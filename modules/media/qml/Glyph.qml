import QtQuick
import QtQuick.Shapes
import qs.island

// Filled media icons on a 24x24 grid, in the theme's colors.
Item {
    id: root

    required property string name
    property color color: Theme.foreground
    property real size: 20

    implicitWidth: size
    implicitHeight: size

    readonly property var paths: ({
        "play": "M8 5.5v13a1 1 0 0 0 1.5.86l10.5-6.5a1 1 0 0 0 0-1.72L9.5 4.64A1 1 0 0 0 8 5.5z",
        "pause": "M6 5a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1v14a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1zM13 5a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1v14a1 1 0 0 1-1 1h-3a1 1 0 0 1-1-1z",
        "next": "M4 6.4v11.2a1 1 0 0 0 1.55.83L14 12.8a1 1 0 0 0 0-1.6L5.55 5.57A1 1 0 0 0 4 6.4zM17 5h2a1 1 0 0 1 1 1v12a1 1 0 0 1-1 1h-2a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1z",
        "previous": "M20 6.4v11.2a1 1 0 0 1-1.55.83L10 12.8a1 1 0 0 1 0-1.6l8.45-5.63A1 1 0 0 1 20 6.4zM7 5H5a1 1 0 0 0-1 1v12a1 1 0 0 0 1 1h2a1 1 0 0 0 1-1V6a1 1 0 0 0-1-1z",
        "note": "M18.5 3.1a1 1 0 0 1 .5.86V15.5a3.5 3.5 0 1 1-2-3.16V7.3l-7 1.75v8.45a3.5 3.5 0 1 1-2-3.16V6.5a1 1 0 0 1 .76-.97l9-2.25a1 1 0 0 1 .74-.18z"
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
