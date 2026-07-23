import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import Quickshell.Hyprland

PanelWindow {
    id: window

    visible: false
    color: "transparent"
    exclusiveZone: 0

    // Request the full output explicitly; four-edge anchoring is reduced by
    // the desktop's 40px reserved panel area on this compositor.
    implicitWidth: screen ? screen.width : 0
    implicitHeight: screen ? screen.height : 0

    anchors {
        top: true
        left: true
    }

    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.OnDemand
    WlrLayershell.namespace: "yobu"

    property var uiConfig: ({ width: 600, height: 420, max_results: 50 })
    property var pendingPositions: ({})
    property bool benchmarkDisplay: false
    property real dragStartX: 0
	property real dragStartY: 0

	function focusedScreen() {
		const monitor = Hyprland.focusedMonitor
		if (!monitor)
		return null

		return Quickshell.screens.find(
			screen => screen.name === monitor.name
		) ?? null
	}


    function finish(message) {
        socket.write(JSON.stringify(message) + "\n")
        socket.flush()
        benchmarkDisplay = false
        launcher.visible = false
        visible = false
    }

	function applyPosition(positions) {
        const availableWidth = Math.max(0, window.screen.width - launcher.width)
        const availableHeight = Math.max(0, window.screen.height - launcher.height)
		const saved = positions[screen.name]

        if (saved) {
            launcher.x = Math.round(saved.x * availableWidth)
            launcher.y = Math.round(saved.y * availableHeight)
        } else {
            launcher.x = Math.round(availableWidth / 2)
            launcher.y = Math.round(availableHeight / 2)
        }
    }

    function savePosition() {
        const availableWidth = Math.max(1, width - launcher.width)
        const availableHeight = Math.max(1, height - launcher.height)
        socket.write(JSON.stringify({
            type: "position",
            screen: screen.name,
            x: launcher.x / availableWidth,
            y: launcher.y / availableHeight
        }) + "\n")
        socket.flush()
    }

	function revealLauncher() {
		if (!visible || !backingWindowVisible || width <= 0 || height <= 0) return
		if (screen && (width !== screen.width || height !== screen.height)) return

        launcher.width = Math.min(uiConfig.width, window.width)
        launcher.height = Math.min(uiConfig.height, window.height)
        applyPosition(pendingPositions)
        launcher.visible = true
        launcher.focusInput()
	}

	function parseUiConfig(config) {
		window.uiConfig = config
		if (config.width < 1) {
			window.uiConfig.width = window.width * config.width
		}
		if (config.height < 1) {
			window.uiConfig.height = window.height * config.height
		}
	}

    onBackingWindowVisibleChanged: revealLauncher()
    onWidthChanged: revealLauncher()
    onHeightChanged: revealLauncher()

    Connections {
        target: launcher.Window.window

        function onFrameSwapped() {
            if (window.benchmarkDisplay && launcher.visible)
                window.finish({ type: "presented" })
        }
    }

    Socket {
        id: socket
        path: (Quickshell.env("YOBU_SOCKET")
            ?? ((Quickshell.env("XDG_RUNTIME_DIR") ?? "/tmp") + "/yobu.sock"))
        connected: true

        onConnectedChanged: {
            if (connected) {
                write(JSON.stringify({ type: "hello", role: "frontend" }) + "\n")
                flush()
            }
        }

        parser: SplitParser {
			onRead: data => {
                const message = JSON.parse(data)
				console.log("got message", message.type)
				if (message.type === "show") {
					parseUiConfig(message.config)
                    window.pendingPositions = message.positions
                    window.benchmarkDisplay = message.benchmark === true
                    launcher.prompt = message.prompt
                    launcher.entries = message.entries
                    launcher.maxResults = message.config.max_results
                    launcher.reset()
                    launcher.visible = false
					window.screen = window.focusedScreen()
					window.visible = true
                    window.revealLauncher()
                } else if (message.type === "search_results") {
                    launcher.applySearchResults(message.query, message.entries)
                }
            }
        }
    }

    MouseArea {
        anchors.fill: parent
        onClicked: window.finish({ type: "cancelled" })
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

        onAccepted: entry => window.finish({ type: "selected", value: entry.value })
        onCancelRequested: window.finish({ type: "cancelled" })
        onQueryChanged: query => {
            if (socket.connected) {
                socket.write(JSON.stringify({ type: "search", query: query }) + "\n")
                socket.flush()
            }
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
