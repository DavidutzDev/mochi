import QtQuick
import Quickshell
import qs.island

// A device's icon: the one BlueZ names, like audio-headset, from the icon
// theme, else the Material symbol for its kind, else the Bluetooth symbol.
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
        name: ({
                "audio-headset": "headset_mic",
                "audio-headphones": "headphones",
                "audio-card": "speaker",
                "input-mouse": "mouse",
                "input-keyboard": "keyboard",
                "input-gaming": "sports_esports",
                "input-tablet": "stylus",
                "phone": "smartphone",
                "computer": "computer",
                "video-display": "tv",
                "camera-photo": "photo_camera",
                "printer": "print"
            })[root.icon] ?? "bluetooth"
        size: root.size * 0.85
        color: root.color
    }
}
