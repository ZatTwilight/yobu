import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
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

    Component.onCompleted: Qt.callLater(() => input.forceActiveFocus())

    MouseArea {
        anchors.fill: parent
        onClicked: Qt.quit()
    }

    Rectangle {
        id: card
        width: Math.min(600, window.width / 2)
        height: 160
        x: (window.width - width) / 2
        y: (window.height - height) / 2
        radius: 12
        color: "#ee1f1f28"

        MouseArea { anchors.fill: parent }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 20
            spacing: 12

            TextField {
                id: input
                Layout.fillWidth: true
                focus: true
                placeholderText: "Search…"
                Keys.onEscapePressed: Qt.quit()
            }

            Item { Layout.fillHeight: true }
        }
    }
}
