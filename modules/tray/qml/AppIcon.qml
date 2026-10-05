import QtQuick
import Quickshell
import qs.island

// An app's tray icon: a picture file, or a name from the icon theme, or the
// tray symbol when neither is found.
Item {
    id: root

    property string icon: ""
    property real size: 22

    readonly property string source: {
        if (icon === "")
            return "";
        if (icon.startsWith("/"))
            return `file://${icon.split("/").map(encodeURIComponent).join("/")}`;
        return Quickshell.iconPath(icon, true);
    }

    implicitWidth: size
    implicitHeight: size

    Image {
        id: picture

        anchors.fill: parent
        source: root.source
        sourceSize.width: root.size * 2
        sourceSize.height: root.size * 2
        fillMode: Image.PreserveAspectFit
        asynchronous: true
        smooth: true
    }

    Symbol {
        anchors.centerIn: parent
        visible: picture.status !== Image.Ready
        name: "tray"
        size: root.size * 0.8
        color: Theme.muted
    }
}
