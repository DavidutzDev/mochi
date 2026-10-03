import QtQuick
import QtQuick.Shapes
import qs.island

// Filled icons on a 24x24 grid, in the theme's colors.
Item {
    id: root

    required property string name
    property color color: Theme.foreground
    property real size: 20

    implicitWidth: size
    implicitHeight: size

    readonly property var paths: ({
        "bell": "M12 2a6 6 0 0 0-6 6v3.6l-1.7 3A1 1 0 0 0 5.2 16h13.6a1 1 0 0 0 .9-1.4L18 11.6V8a6 6 0 0 0-6-6zM9.5 18a2.5 2.5 0 0 0 5 0z",
        "moon": "M20.5 14.2A8.5 8.5 0 0 1 9.8 3.5a.6.6 0 0 0-.8-.7A9.5 9.5 0 1 0 21.2 15a.6.6 0 0 0-.7-.8z",
        "close": "M6.3 4.9 12 10.6l5.7-5.7 1.4 1.4-5.7 5.7 5.7 5.7-1.4 1.4-5.7-5.7-5.7 5.7-1.4-1.4 5.7-5.7-5.7-5.7z"
    })

    Shape {
        width: 24
        height: 24
        scale: root.size / 24
        transformOrigin: Item.TopLeft
        preferredRendererType: Shape.CurveRenderer

        ShapePath {
            fillColor: root.color
            strokeColor: "transparent"
            PathSvg {
                path: root.paths[root.name] ?? ""
            }
        }
    }
}
