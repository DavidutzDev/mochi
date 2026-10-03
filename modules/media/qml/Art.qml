import QtQuick
import Quickshell.Widgets
import qs.island

// The cover, or a note while there is none or it doesn't load.
ClippingRectangle {
    id: root

    property string source: ""
    property real size: 24

    implicitWidth: size
    implicitHeight: size
    radius: Math.round(size / 5)
    color: Theme.surface

    Glyph {
        anchors.centerIn: parent
        visible: cover.status !== Image.Ready
        name: "note"
        size: root.size * 0.5
        color: Theme.muted
    }

    Image {
        id: cover

        anchors.fill: parent
        source: root.source
        sourceSize.width: root.size * 2
        sourceSize.height: root.size * 2
        fillMode: Image.PreserveAspectCrop
        asynchronous: true
        opacity: status === Image.Ready ? 1 : 0

        Behavior on opacity {
            NumberAnimation {
                duration: 150
            }
        }
    }
}
