import QtQuick

QtObject {
	id: root

	required property DaemonConnection daemon
	property var positions: ({})
	property var config: ({ width: 600, height: 420, max_results: 50 })
	property bool ready: false
	property string error: ""
	property var removePositionsSub: null

	function load() {
		if (!daemon.connected) return
		daemon.request(
			"state.get",
			{},
			result => {
				root.positions = result.positions ?? {}
				if (result.config) root.config = result.config
				root.ready = true
			},
			(code, message) => root.error = code + ": " + message
		)
	}

	function setPosition(screen, x, y) {
		if (!daemon.connected) return
		// optimistic local update for immediate feedback
		const updated = Object.assign({}, positions)
		updated[screen] = { x, y }
		positions = updated
		daemon.request(
			"state.set_position",
			{ screen, x, y },
			() => {},
			(code, message) => root.error = code + ": " + message
		)
	}

	function handlePositionsEvent(payload) {
		if (!payload || !payload.screen) return
		const updated = Object.assign({}, positions)
		updated[payload.screen] = { x: payload.x, y: payload.y }
		positions = updated
	}

	Component.onCompleted: {
		removePositionsSub = daemon.subscribe("state.positions", root.handlePositionsEvent)
		if (daemon.connected) load()
	}

	Component.onDestruction: {
		if (removePositionsSub !== null) removePositionsSub()
	}

	property Connections daemonWatcher: Connections {
		target: root.daemon
		function onConnectedChanged() {
			if (root.daemon.connected) root.load()
		}
	}
}
