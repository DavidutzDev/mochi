import QtQuick
import Quickshell
import Quickshell.Widgets
import qs.island

// A notification's picture if it has one, otherwise its app's icon from the
// icon theme, otherwise a bell.
ClippingRectangle {
    id: root

    property var note: ({})
    property real size: 28

    readonly property string source: {
        const find = name => name ? Quickshell.iconPath(name, true) : "";
        const file = path => path.startsWith("/") ? `file://${path}` : path.startsWith("file://") ? path : "";
        const image = note.image ?? "";
        if (image)
            return file(image) || find(image);
        const icon = note.icon ?? "";
        return file(icon) || find(icon) || find((note.app ?? "").toLowerCase());
    }

    implicitWidth: size
    implicitHeight: size
    // Pictures, like avatars, get rounded corners; icons keep their shape.
    radius: note.image ? size / 4 : 0
    color: "transparent"

    Glyph {
        anchors.centerIn: parent
        visible: picture.status !== Image.Ready
        name: "bell"
        size: root.size * 0.7
        color: Theme.muted
    }

    Image {
        id: picture

        anchors.fill: parent
        source: root.source
        sourceSize.width: root.size * 2
        sourceSize.height: root.size * 2
        fillMode: root.note.image ? Image.PreserveAspectCrop : Image.PreserveAspectFit
        asynchronous: true
    }
}
