import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import Quickshell.Widgets

PanelWindow {
    id: window

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

    property string prompt: "Search…"
    property var entries: []
    property string query: ""
    property var uiConfig: ({ width: 600, height: 420, max_results: 50 })
    property var filteredEntries: {
        const needle = query.trim().toLowerCase()
        const matches = needle === ""
            ? entries
            : entries.filter(entry => {
                const text = `${entry.label} ${entry.description ?? ""}`.toLowerCase()
                return text.includes(needle)
            })
        return matches.slice(0, uiConfig.max_results)
    }

    function finish(message) {
        socket.write(JSON.stringify(message) + "\n")
        socket.flush()
        Qt.callLater(Qt.quit)
    }

    function selectCurrent() {
        const entry = filteredEntries[list.currentIndex]
        if (entry)
            finish({ type: "selected", value: entry.value })
    }

    function applyPosition(positions) {
        const saved = positions[screen.name]
        if (!saved) {
            card.x = (width - card.width) / 2
            card.y = (height - card.height) / 2
            return
        }
        card.x = saved.x * Math.max(0, width - card.width)
        card.y = saved.y * Math.max(0, height - card.height)
    }

    function savePosition() {
        const availableWidth = Math.max(1, width - card.width)
        const availableHeight = Math.max(1, height - card.height)
        socket.write(JSON.stringify({
            type: "position",
            screen: screen.name,
            x: card.x / availableWidth,
            y: card.y / availableHeight
        }) + "\n")
        socket.flush()
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
                if (message.type === "show") {
                    window.prompt = message.prompt
                    window.entries = message.entries
                    window.uiConfig = message.config
                    card.width = Math.min(message.config.width, window.width)
                    card.height = Math.min(message.config.height, window.height)
                    window.applyPosition(message.positions)
                    input.text = ""
                    list.currentIndex = message.entries.length > 0 ? 0 : -1
                    Qt.callLater(() => input.forceActiveFocus())
                }
            }
        }
    }

    MouseArea {
        anchors.fill: parent
        onClicked: window.finish({ type: "cancelled" })
    }

    Rectangle {
        id: card
        width: 600
        height: 420
        x: (window.width - width) / 2
        y: (window.height - height) / 2
        radius: 12
        color: "#ee1f1f28"
        clip: true

        MouseArea { anchors.fill: parent }

        ColumnLayout {
            anchors.fill: parent
            spacing: 0

            Rectangle {
                id: dragBar
                Layout.fillWidth: true
                Layout.preferredHeight: 24
                color: "transparent"

                property real startX
                property real startY

                Rectangle {
                    anchors.centerIn: parent
                    width: 42
                    height: 3
                    radius: 2
                    color: "#54546d"
                }

                DragHandler {
                    target: null

                    onActiveChanged: {
                        if (active) {
                            dragBar.startX = card.x
                            dragBar.startY = card.y
                        } else {
                            window.savePosition()
                        }
                    }

                    onActiveTranslationChanged: {
                        card.x = Math.max(0, Math.min(
                            window.width - card.width,
                            dragBar.startX + activeTranslation.x
                        ))
                        card.y = Math.max(0, Math.min(
                            window.height - card.height,
                            dragBar.startY + activeTranslation.y
                        ))
                    }
                }
            }

            TextField {
                id: input
                Layout.fillWidth: true
                Layout.leftMargin: 16
                Layout.rightMargin: 16
                Layout.bottomMargin: 10
                focus: true
                placeholderText: window.prompt

                onTextChanged: {
                    window.query = text
                    list.currentIndex = window.filteredEntries.length > 0 ? 0 : -1
                }

                Keys.onEscapePressed: window.finish({ type: "cancelled" })
                Keys.onUpPressed: list.decrementCurrentIndex()
                Keys.onDownPressed: list.incrementCurrentIndex()
                onAccepted: window.selectCurrent()
            }

            ListView {
                id: list
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.leftMargin: 12
                Layout.rightMargin: 12
                Layout.bottomMargin: 12
                clip: true
                spacing: 2
                model: window.filteredEntries
                currentIndex: count > 0 ? 0 : -1
                keyNavigationWraps: true

                delegate: Rectangle {
                    id: entry
                    required property var modelData
                    required property int index
                    width: ListView.view.width
                    height: entry.modelData.description ? 48 : 38
                    radius: 5
                    color: ListView.isCurrentItem ? "#363646" : "transparent"

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 10
                        anchors.rightMargin: 10
                        spacing: 10

                        IconImage {
                            visible: source != ""
                            source: entry.modelData.icon
                                ? Quickshell.iconPath(entry.modelData.icon, true)
                                : ""
                            Layout.preferredWidth: 24
                            Layout.preferredHeight: 24
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 0

                            Text {
                                Layout.fillWidth: true
                                text: entry.modelData.label
                                color: "#dcd7ba"
                                elide: Text.ElideRight
                            }

                            Text {
                                Layout.fillWidth: true
                                visible: text !== ""
                                text: entry.modelData.description ?? ""
                                color: "#727169"
                                font.pixelSize: 11
                                elide: Text.ElideRight
                            }
                        }
                    }

                    MouseArea {
                        anchors.fill: parent
                        onClicked: {
                            list.currentIndex = entry.index
                            window.selectCurrent()
                        }
                    }
                }
            }
        }
    }
}
