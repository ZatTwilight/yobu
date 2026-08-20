import QtQuick

QtObject {
	id: root

	required property DaemonConnection daemon
	property var catalog: []
	property var entries: []
	property string query: ""
	property int maxResults: 50
	property bool loading: false
	property bool searching: false
	property bool refreshing: false
	property string error: ""
	property real searchGeneration: 0
	property var removeChangedSubscription: null

	function load() {
		if (!daemon.connected) return
		loading = true
		error = ""
		daemon.request(
			"applications.list",
			{},
			result => {
				root.catalog = result.entries ?? []
				root.loading = false
				root.search(root.query)
			},
			(code, message) => {
				root.loading = false
				root.error = code + ": " + message
			}
		)
	}

	function search(query) {
		root.query = query
		const generation = ++root.searchGeneration
		const trimmed = query.trim()
		if (!daemon.connected) return
		if (trimmed === "") {
			root.searching = true
			root.error = ""
			daemon.request(
				"applications.search",
				{ query: "", max_results: root.maxResults },
				result => {
					if (generation !== root.searchGeneration) return
					root.entries = result.entries ?? []
					root.searching = false
				},
				(code, message) => {
					if (generation !== root.searchGeneration) return
					root.searching = false
					root.error = code + ": " + message
				}
			)
			return
		}

		root.searching = true
		root.error = ""
		daemon.request(
			"calculator.evaluate",
			{ query },
			calcResult => {
				if (generation !== root.searchGeneration) return
				if (calcResult.entry) {
					root.entries = [calcResult.entry]
					root.searching = false
					return
				}
				daemon.request(
					"applications.search",
					{ query, max_results: root.maxResults },
					result => {
						if (generation !== root.searchGeneration) return
						root.entries = result.entries ?? []
						root.searching = false
					},
					(code, message) => {
						if (generation !== root.searchGeneration) return
						root.searching = false
						root.error = code + ": " + message
					}
				)
			},
			() => {
				if (generation !== root.searchGeneration) return
				daemon.request(
					"applications.search",
					{ query, max_results: root.maxResults },
					result => {
						if (generation !== root.searchGeneration) return
						root.entries = result.entries ?? []
						root.searching = false
					},
					(code, message) => {
						if (generation !== root.searchGeneration) return
						root.searching = false
						root.error = code + ": " + message
					}
				)
			}
		)
	}

	function launch(id, onLaunched = () => {}) {
		if (!daemon.connected) return
		error = ""
		daemon.request(
			"applications.launch",
			{ id },
			onLaunched,
			(code, message) => root.error = code + ": " + message
		)
	}

	function refresh() {
		if (!daemon.connected) return
		refreshing = true
		error = ""
		daemon.request(
			"applications.refresh",
			{},
			() => root.refreshing = false,
			(code, message) => {
				root.refreshing = false
				root.error = code + ": " + message
			}
		)
	}

	function handleCatalogChanged() {
		load()
	}

	Component.onCompleted: {
		removeChangedSubscription = daemon.subscribe(
			"applications.changed",
			root.handleCatalogChanged
		)
		if (daemon.connected)
			load()
	}

	Component.onDestruction: {
		if (removeChangedSubscription !== null)
			removeChangedSubscription()
	}

	property Connections daemonWatcher: Connections {
		target: root.daemon

		function onConnectedChanged() {
			if (root.daemon.connected)
				root.load()
		}
	}
}
