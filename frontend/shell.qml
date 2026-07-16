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

    anchors {
        top: true
        right: true
        bottom: true
        left: true
    }

    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.OnDemand
    WlrLayershell.namespace: "qmenu"

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

        launcher.width = Math.min(uiConfig.width, width)
        launcher.height = Math.min(uiConfig.height, height)
        applyPosition(pendingPositions)
        launcher.visible = true
        launcher.focusInput()
    }

    onBackingWindowVisibleChanged: revealLauncher()

    Connections {
        target: launcher.Window.window

        function onFrameSwapped() {
            if (window.benchmarkDisplay && launcher.visible)
                window.finish({ type: "presented" })
        }
    }

    Socket {
        id: socket
        path: (Quickshell.env("QMENU_SOCKET")
            ?? ((Quickshell.env("XDG_RUNTIME_DIR") ?? "/tmp") + "/qmenu.sock"))
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
                    window.uiConfig = message.config
                    window.pendingPositions = message.positions
                    window.benchmarkDisplay = message.benchmark === true
                    launcher.prompt = message.prompt
                    launcher.entries = message.entries
                    launcher.maxResults = message.config.max_results
                    launcher.reset()
                    launcher.visible = false
					window.visible = true
					window.screen = window.focusedScreen()
                    window.revealLauncher()
                }
            }
        }
    }

    MouseArea {
        anchors.fill: parent
        onClicked: window.finish({ type: "cancelled" })
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
