import QtQuick
import QtQuick.Shapes
import Quickshell

// An icon by name: one of the built-in symbols below, drawn in the theme's
// colors so every module's icons match, or else an icon from the system
// theme. Contributions name their icon this way.
Item {
    id: root

    required property string name
    property color color: Theme.foreground
    property real size: 20

    implicitWidth: size
    implicitHeight: size

    // Filled paths on a 24x24 grid; holes use the even-odd rule.
    readonly property var paths: ({
        "home": "M12 3.2 3 10.4V20a1 1 0 0 0 1 1h5.5v-6h5v6H20a1 1 0 0 0 1-1v-9.6z",
        "bell": "M12 2a6 6 0 0 0-6 6v3.6l-1.7 3A1 1 0 0 0 5.2 16h13.6a1 1 0 0 0 .9-1.4L18 11.6V8a6 6 0 0 0-6-6zM9.5 18a2.5 2.5 0 0 0 5 0z",
        "music": "M18.5 3.1a1 1 0 0 1 .5.86V15.5a3.5 3.5 0 1 1-2-3.16V7.3l-7 1.75v8.45a3.5 3.5 0 1 1-2-3.16V6.5a1 1 0 0 1 .76-.97l9-2.25a1 1 0 0 1 .74-.18z",
        "clock": "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm0 2a8 8 0 1 1 0 16 8 8 0 0 1 0-16zm-1 3v6.2l4.9 2.9 1-1.7-3.9-2.3V7z",
        "grid": "M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z",
        "moon": "M20.5 14.2A8.5 8.5 0 0 1 9.8 3.5a.6.6 0 0 0-.8-.7A9.5 9.5 0 1 0 21.2 15a.6.6 0 0 0-.7-.8z",
        "volume": "M3 9h4l5-4v14l-5-4H3zM15 8.5a5 5 0 0 1 0 7l-1.4-1.4a3 3 0 0 0 0-4.2zM17.8 5.7a9 9 0 0 1 0 12.6l-1.4-1.4a7 7 0 0 0 0-9.8z",
        "power": "M11 2h2v10h-2zM6.3 5.6l1.4 1.4a7 7 0 1 0 8.6 0l1.4-1.4a9 9 0 1 1-11.4 0z"
    })
    readonly property string path: paths[name] ?? ""

    Shape {
        visible: root.path !== ""
        width: 24
        height: 24
        scale: root.size / 24
        transformOrigin: Item.TopLeft
        preferredRendererType: Shape.CurveRenderer

        ShapePath {
            fillColor: root.color
            strokeColor: "transparent"
            fillRule: ShapePath.OddEvenFill
            PathSvg {
                path: root.path
            }
        }
    }

    Image {
        anchors.fill: parent
        visible: root.path === "" && status === Image.Ready
        source: root.path === "" && root.name ? Quickshell.iconPath(root.name, true) : ""
        sourceSize.width: root.size * 2
        sourceSize.height: root.size * 2
    }
}
