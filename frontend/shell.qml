import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import Quickshell.Wayland

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
    property var filteredEntries: {
        const needle = query.trim().toLowerCase()
        if (needle === "")
            return entries
        return entries.filter(entry => entry.label.toLowerCase().includes(needle))
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
                    window.query = ""
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
        width: Math.min(600, window.width / 2)
        height: Math.min(420, window.height / 2)
        x: (window.width - width) / 2
        y: (window.height - height) / 2
        radius: 12
        color: "#ee1f1f28"

        MouseArea { anchors.fill: parent }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 16
            spacing: 10

            TextField {
                id: input
                Layout.fillWidth: true
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
                    height: 36
                    radius: 5
                    color: ListView.isCurrentItem ? "#363646" : "transparent"

                    Text {
                        anchors.fill: parent
                        anchors.leftMargin: 10
                        anchors.rightMargin: 10
                        text: entry.modelData.label
                        color: "#dcd7ba"
                        verticalAlignment: Text.AlignVCenter
                        elide: Text.ElideRight
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
