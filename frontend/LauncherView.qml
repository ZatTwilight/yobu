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
    signal queryChanged(string query)

    property var filteredEntries: entries.slice(0, maxResults)

    function applySearchResults(query, results) {
        if (input.text === query)
            entries = results
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

    radius: 7
    color: "#f21f1f28"
    border.width: 1
    border.color: "#54546d"
    clip: true

    // Soft inner atmosphere: restrained, warm, and less like a stock dialog.
    Rectangle {
        anchors.fill: parent
        radius: root.radius
        opacity: 0.22
        gradient: Gradient {
            GradientStop { position: 0.0; color: "#363646" }
            GradientStop { position: 0.48; color: "#1f1f28" }
            GradientStop { position: 1.0; color: "#16161d" }
        }
    }

    // Consume otherwise unhandled clicks inside the launcher.
    MouseArea { anchors.fill: parent }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Rectangle {
            id: dragBar
            Layout.fillWidth: true
            Layout.preferredHeight: 22
            color: "transparent"

            Row {
                anchors.centerIn: parent
                spacing: 4
                Repeater {
                    model: 3
                    Rectangle {
                        width: 4
                        height: 4
                        radius: 2
                        color: "#727169"
                        opacity: 0.75
                    }
                }
            }
        }

        Rectangle {
            id: searchFrame
            Layout.fillWidth: true
            Layout.preferredHeight: 42
            Layout.leftMargin: 14
            Layout.rightMargin: 14
            radius: 5
            color: input.activeFocus ? "#2a2a37" : "#252535"
            border.width: 1
            border.color: input.activeFocus ? "#e6c384" : "#54546d"

            Behavior on color { ColorAnimation { duration: 120 } }
            Behavior on border.color { ColorAnimation { duration: 120 } }

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 12
                anchors.rightMargin: 9
                spacing: 9

                Item {
                    Layout.preferredWidth: 17
                    Layout.preferredHeight: 17

                    Rectangle {
                        x: 1
                        y: 1
                        width: 10
                        height: 10
                        radius: 5
                        color: "transparent"
                        border.width: 1.7
                        border.color: input.activeFocus ? "#e6c384" : "#7e9cd8"
                    }
                    Rectangle {
                        x: 10
                        y: 11
                        width: 6
                        height: 1.7
                        radius: 1
                        rotation: 45
                        transformOrigin: Item.Left
                        color: input.activeFocus ? "#e6c384" : "#7e9cd8"
                    }
                }

                TextField {
                    id: input
                    Layout.fillWidth: true
                    focus: true
                    color: "#dcd7ba"
                    placeholderText: root.prompt
                    placeholderTextColor: "#727169"
                    selectionColor: "#49443c"
                    selectedTextColor: "#e6c384"
                    font.pixelSize: 14
                    font.weight: Font.Medium
                    leftPadding: 0
                    rightPadding: 0
                    topPadding: 0
                    bottomPadding: 0
                    background: Item {}

                    onTextChanged: {
                        list.currentIndex = root.filteredEntries.length > 0 ? 0 : -1
                        root.queryChanged(text)
                    }
                    Keys.onEscapePressed: root.cancelRequested()
                    Keys.onUpPressed: list.decrementCurrentIndex()
                    Keys.onDownPressed: list.incrementCurrentIndex()
                    onAccepted: root.acceptCurrent()
                }

                Rectangle {
                    Layout.preferredWidth: 30
                    Layout.preferredHeight: 20
                    radius: 3
                    color: "#1f1f28"
                    border.width: 1
                    border.color: "#54546d"

                    Text {
                        anchors.centerIn: parent
                        text: "esc"
                        color: "#727169"
                        font.pixelSize: 9
                        font.letterSpacing: 0.5
                    }
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.topMargin: 10
            Layout.bottomMargin: 5
            Layout.leftMargin: 21
            Layout.rightMargin: 21

            Text {
                text: input.text.length > 0 ? "MATCHES" : "SUGGESTIONS"
                color: "#7e9cd8"
                font.pixelSize: 10
                font.weight: Font.DemiBold
                font.letterSpacing: 1.4
            }

            Item { Layout.fillWidth: true }

            Text {
                text: root.filteredEntries.length + (root.filteredEntries.length === 1 ? " result" : " results")
                color: "#727169"
                font.pixelSize: 11
            }
        }

        ListView {
            id: list
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.leftMargin: 12
            Layout.rightMargin: 8
            Layout.bottomMargin: 10
            rightMargin: 8
            clip: true
            spacing: 3
            model: root.filteredEntries
            currentIndex: count > 0 ? 0 : -1
            keyNavigationWraps: true
            boundsBehavior: Flickable.StopAtBounds

            ScrollBar.vertical: ScrollBar {
                policy: ScrollBar.AsNeeded
                width: 5
                contentItem: Rectangle {
                    radius: 3
                    color: "#54546d"
                    opacity: 0.65
                }
                background: Item {}
            }

            delegate: Rectangle {
                id: entry
                required property var modelData
                required property int index
                property bool hovered: entryMouse.containsMouse

                width: ListView.view.width
                height: entry.modelData.description ? 46 : 38
                radius: 4
                color: ListView.isCurrentItem
                    ? "#363646"
                    : (hovered ? "#2a2a37" : "transparent")
                border.width: ListView.isCurrentItem ? 1 : 0
                border.color: "#54546d"

                Behavior on color { ColorAnimation { duration: 90 } }

                Rectangle {
                    visible: entry.ListView.isCurrentItem
                    anchors.left: parent.left
                    anchors.verticalCenter: parent.verticalCenter
                    width: 3
                    height: parent.height - 20
                    radius: 2
                    color: "#e6c384"
                }

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 11
                    anchors.rightMargin: 10
                    spacing: 9

                    Item {
                        Layout.preferredWidth: 25
                        Layout.preferredHeight: 25

                        Image {
                            id: iconImage
                            anchors.centerIn: parent
                            width: 22
                            height: 22
                            source: entry.modelData.icon
                                ? Quickshell.iconPath(entry.modelData.icon, true)
                                : ""
                            sourceSize.width: 52
                            sourceSize.height: 52
                            fillMode: Image.PreserveAspectFit
                            visible: status === Image.Ready
                        }

                        Rectangle {
                            anchors.centerIn: parent
                            width: 22
                            height: 22
                            radius: 3
                            visible: iconImage.status !== Image.Ready
                            color: "#2a2a37"
                            border.width: 1
                            border.color: "#54546d"

                            Text {
                                anchors.centerIn: parent
                                text: (entry.modelData.label ?? "?").charAt(0).toUpperCase()
                                color: "#7e9cd8"
                                font.pixelSize: 10
                                font.weight: Font.DemiBold
                            }
                        }
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 1

                        Text {
                            Layout.fillWidth: true
                            text: entry.modelData.label
                            color: entry.ListView.isCurrentItem ? "#e6c384" : "#dcd7ba"
                            font.pixelSize: 13
                            font.weight: entry.ListView.isCurrentItem ? Font.DemiBold : Font.Normal
                            elide: Text.ElideRight
                        }

                        Text {
                            Layout.fillWidth: true
                            visible: text !== ""
                            text: entry.modelData.description ?? ""
                            color: "#727169"
                            font.pixelSize: 10
                            elide: Text.ElideRight
                        }
                    }

                    Text {
                        visible: entry.ListView.isCurrentItem
                        text: "↵"
                        color: "#e6c384"
                        font.pixelSize: 17
                    }
                }

                MouseArea {
                    id: entryMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    onEntered: list.currentIndex = entry.index
                    onClicked: {
                        list.currentIndex = entry.index
                        root.acceptCurrent()
                    }
                }
            }

            Text {
                anchors.centerIn: parent
                visible: list.count === 0
                text: input.text.length > 0 ? "No matches found" : "Nothing to show yet"
                color: "#727169"
                font.pixelSize: 13
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 28
            color: "#16161d"
            border.width: 0

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 18
                anchors.rightMargin: 18
                spacing: 8

                Text { text: "↑↓"; color: "#7e9cd8"; font.pixelSize: 11 }
                Text { text: "navigate"; color: "#727169"; font.pixelSize: 10 }
                Item { Layout.fillWidth: true }
                Text { text: "↵"; color: "#e6c384"; font.pixelSize: 12 }
                Text { text: "open"; color: "#727169"; font.pixelSize: 10 }
            }
        }
    }
}
