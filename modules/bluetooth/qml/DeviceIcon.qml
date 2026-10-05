import QtQuick
import Quickshell
import qs.island

// A device's icon: the one BlueZ names, like audio-headset, from the icon
// theme, else Mochi's own for headsets and the Bluetooth symbol.
Item {
    id: root

    property string icon: ""
    property real size: 20
    property color color: Theme.foreground

    implicitWidth: size
    implicitHeight: size

    Image {
        id: picture

        anchors.fill: parent
        source: root.icon === "" ? "" : Quickshell.iconPath(`${root.icon}-symbolic`, true) || Quickshell.iconPath(root.icon, true)
        sourceSize.width: root.size * 2
        sourceSize.height: root.size * 2
        fillMode: Image.PreserveAspectFit
        asynchronous: true
    }

    Symbol {
        anchors.centerIn: parent
        visible: picture.status !== Image.Ready
        name: root.icon.startsWith("audio-head") ? "headset" : root.icon === "audio-card" ? "speakers" : "bluetooth"
        size: root.size * 0.85
        color: root.color
    }
}
