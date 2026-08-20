import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Wayland

PanelWindow {
	id: window

	// Keep the layer-shell surface mapped so opening only toggles QML items.
	visible: true
	color: "transparent"
	exclusiveZone: 0
	mask: Region {
		width: window.launcherActive ? window.width : 0
		height: window.launcherActive ? window.height : 0
	}
	implicitWidth: screen ? screen.width : 0
	implicitHeight: screen ? screen.height : 0

	anchors {
		top: true
		left: true
	}

	WlrLayershell.layer: WlrLayer.Overlay
	WlrLayershell.keyboardFocus: launcherActive ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
	WlrLayershell.namespace: "yobu"

	property var uiConfig: stateModule.config
	property var positions: stateModule.positions
	property real dragStartX: 0
	property real dragStartY: 0
	property var pipedEntries: []
	property var filteredPipedEntries: []
	property bool isPiped: false
	property bool launcherActive: false
	property string menuKey: ""

	function focusedScreen() {
		const monitor = Hyprland.focusedMonitor
		if (monitor) {
			const focused = Quickshell.screens.find(screen => screen.name === monitor.name)
			if (focused) return focused
		}
		return Quickshell.screens[0] ?? null
	}

	function showLauncher() {
		isPiped = false
		pipedEntries = []
		filteredPipedEntries = []
		const targetScreen = focusedScreen()
		if (targetScreen)
			window.screen = targetScreen
		applications.search("")
		launcher.prompt = "Search..."
		launcher.reset()
		window.launcherActive = true
		window.revealLauncher()
	}

	function showMenu(key) {
		daemon.request("menus.get", { key }, result => {
			isPiped = true
			menuKey = key
			pipedEntries = result.entries ?? []
			filteredPipedEntries = pipedEntries.slice(0, stateModule.config.max_results)
			const targetScreen = focusedScreen()
			if (targetScreen) window.screen = targetScreen
			launcher.prompt = result.prompt ?? "Pick one"
			launcher.reset()
			window.launcherActive = true
			window.revealLauncher()
			launcher.prompt = result.prompt ?? "Pick one"
		}, (code, message) => console.error("menus.get failed:", code, message))
	}

	function completeMenu(value) {
		const key = menuKey
		menuKey = ""
		if (key !== "")
			daemon.request("menus.complete", { key, value }, () => {},
				(code, message) => console.error("menus.complete failed:", code, message))
	}

	function fuzzyScore(text, query) {
		text = text.toLowerCase()
		let score = 0
		let previous = -2
		let position = 0
		for (let i = 0; i < query.length; i++) {
			const found = text.indexOf(query[i], position)
			if (found === -1) return null
			score += 10
			if (found === previous + 1) score += 14
			if (found === 0 || " /_-".includes(text[found - 1])) score += 9
			score -= found - position
			previous = found
			position = found + 1
		}
		if (text.includes(query)) score += 40
		return score - text.length * 0.01
	}

	function filterPiped(query) {
		if (!isPiped) return
		const q = query.trim().toLowerCase()
		if (q === "") {
			filteredPipedEntries = pipedEntries.slice(0, stateModule.config.max_results)
			return
		}
		filteredPipedEntries = pipedEntries.map((entry, index) => {
			const text = [entry.label, entry.description].filter(Boolean).join(" ")
			return { entry, index, score: fuzzyScore(text, q) }
		}).filter(match => match.score !== null)
			.sort((a, b) => b.score - a.score || a.index - b.index)
			.slice(0, stateModule.config.max_results)
			.map(match => match.entry)
	}

	function hideLauncher() {
		launcher.visible = false
		launcherActive = false
	}

	function toggleLauncher() {
		if (launcherActive)
			hideLauncher()
		else
			showLauncher()
	}

	function applyPosition() {
		if (!window.screen) return
		const availableWidth = Math.max(0, window.width - launcher.width)
		const availableHeight = Math.max(0, window.height - launcher.height)
		const saved = stateModule.positions[window.screen.name]

		if (saved) {
			launcher.x = Math.round(saved.x * availableWidth)
			launcher.y = Math.round(saved.y * availableHeight)
		} else {
			launcher.x = Math.round(availableWidth / 2)
			launcher.y = Math.round(availableHeight / 2)
		}
	}

	function savePosition() {
		if (!window.screen) return
		const availableWidth = Math.max(1, width - launcher.width)
		const availableHeight = Math.max(1, height - launcher.height)
		stateModule.setPosition(
			window.screen.name,
			launcher.x / availableWidth,
			launcher.y / availableHeight
		)
	}

	function revealLauncher() {
		if (!launcherActive || !backingWindowVisible || width <= 0 || height <= 0) return
		if (screen && (width !== screen.width || height !== screen.height)) return

		const cfg = stateModule.config ?? { width: 600, height: 420 }
		const targetW = cfg.width < 1 ? Math.round(window.width * cfg.width) : cfg.width
		const targetH = cfg.height < 1 ? Math.round(window.height * cfg.height) : cfg.height
		launcher.width = Math.min(targetW, window.width)
		launcher.height = Math.min(targetH, window.height)
		applyPosition()
		launcher.visible = true
		launcher.focusInput()
	}

	onBackingWindowVisibleChanged: revealLauncher()
	onWidthChanged: revealLauncher()
	onHeightChanged: revealLauncher()

	Connections {
		target: stateModule
		function onPositionsChanged() {
			if (window.launcherActive && launcher.visible) window.applyPosition()
		}
		function onConfigChanged() {
			if (window.launcherActive) window.revealLauncher()
		}
	}

	DaemonConnection {
		id: daemon
		clientName: "quickshell.launcher"
		capabilities: ["applications.consumer"]

		onProtocolError: (code, message) => console.error("daemon protocol error:", code, message)
	}

	StateModule {
		id: stateModule
		daemon: daemon
	}

	ApplicationsModule {
		id: applications
		daemon: daemon
		maxResults: stateModule.config.max_results

		onErrorChanged: {
			if (error !== "")
				console.error("applications provider failed:", error)
		}
	}

	IpcHandler {
		id: yobuIpc
		target: "yobu"

		function setVisible(visible: bool): void {
			if (visible) {
				window.showLauncher()
			} else {
				const wasPiped = window.isPiped
				window.hideLauncher()
				if (wasPiped) {
					window.isPiped = false
					window.completeMenu(null)
				}
			}
		}

		function toggle(): void {
			window.toggleLauncher()
		}

		function showMenu(key: string): void {
			window.showMenu(key)
		}

		function refresh(): void {
			applications.refresh()
		}

		function isVisible(): bool {
			return window.launcherActive && launcher.visible
		}
	}

	MouseArea {
		anchors.fill: parent
		onClicked: {
			const wasPiped = window.isPiped
			window.hideLauncher()
			if (wasPiped) {
				window.isPiped = false
				window.completeMenu(null)
			}
		}
	}

	FileView {
		path: Quickshell.env("YOBU_LAUNCHER_VIEW") ?? ""
		watchChanges: path !== ""
		onFileChanged: Quickshell.reload(false)
	}

	LauncherView {
		id: launcher
		visible: false
		width: 600
		height: 420
		x: (window.width - width) / 2
		y: (window.height - height) / 2
		entries: window.isPiped ? window.filteredPipedEntries : applications.entries
		maxResults: stateModule.config.max_results

		onAccepted: entry => {
			const wasPiped = window.isPiped
			const val = entry.value
			window.hideLauncher()
			if (wasPiped) {
				window.isPiped = false
				window.completeMenu(val)
				return
			}
			if (entry.id === "calculation") {
				Quickshell.clipboardText = val
				return
			}
			applications.launch(entry.id)
		}
		onCancelRequested: {
			const wasPiped = window.isPiped
			window.hideLauncher()
			if (wasPiped) {
				window.isPiped = false
				window.completeMenu(null)
			}
		}
		onQueryChanged: query => {
			if (window.isPiped) window.filterPiped(query)
			else applications.search(query)
		}
	}

	DragHandler {
		parent: launcher.dragHandle
		target: null

		onActiveChanged: {
			if (active) {
				window.dragStartX = launcher.x
				window.dragStartY = launcher.y
			} else {
				window.savePosition()
			}
		}

		onActiveTranslationChanged: {
			launcher.x = Math.max(0, Math.min(
				window.width - launcher.width,
				window.dragStartX + activeTranslation.x
			))
			launcher.y = Math.max(0, Math.min(
				window.height - launcher.height,
				window.dragStartY + activeTranslation.y
			))
		}
	}
}
