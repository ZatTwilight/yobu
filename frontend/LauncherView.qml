import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Widgets

Rectangle {
    id: root

    property string prompt: "Search…"
    property var entries: []
    property int maxResults: 50
    readonly property alias dragHandle: dragBar

    signal accepted(var entry)
    signal cancelRequested()

    property var filteredEntries: {
        const needle = input.text.trim().toLowerCase()
        const matches = needle === ""
            ? entries
            : entries.filter(entry => {
                const text = `${entry.label} ${entry.description ?? ""}`.toLowerCase()
                return text.includes(needle)
            })
        return matches.slice(0, maxResults)
    }

    function reset() {
        input.text = ""
        list.currentIndex = filteredEntries.length > 0 ? 0 : -1
    }

    function focusInput() {
        input.forceActiveFocus()
    }

    function acceptCurrent() {
        const entry = filteredEntries[list.currentIndex]
        if (entry)
            accepted(entry)
    }

    radius: 12
    color: "#ee1f1f28"
    clip: true

    // Consume otherwise unhandled clicks inside the launcher.
    MouseArea { anchors.fill: parent }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Rectangle {
            id: dragBar
            Layout.fillWidth: true
            Layout.preferredHeight: 24
            color: "transparent"

            Rectangle {
                anchors.centerIn: parent
                width: 42
                height: 3
                radius: 2
                color: "#54546d"
            }
        }

        TextField {
            id: input
            Layout.fillWidth: true
            Layout.leftMargin: 16
            Layout.rightMargin: 16
            Layout.bottomMargin: 10
            focus: true
            placeholderText: root.prompt

            onTextChanged: list.currentIndex = root.filteredEntries.length > 0 ? 0 : -1
            Keys.onEscapePressed: root.cancelRequested()
            Keys.onUpPressed: list.decrementCurrentIndex()
            Keys.onDownPressed: list.incrementCurrentIndex()
            onAccepted: root.acceptCurrent()
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
            model: root.filteredEntries
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

                    Image {
                        visible: source != ""
                        source: entry.modelData.icon
                            ? Quickshell.iconPath(entry.modelData.icon, true)
                            : ""
                        Layout.preferredWidth: 24
                        Layout.preferredHeight: 24
                        sourceSize.width: 24
                        sourceSize.height: 24
                        fillMode: Image.PreserveAspectFit
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
                        root.acceptCurrent()
                    }
                }
            }
        }
    }
}
