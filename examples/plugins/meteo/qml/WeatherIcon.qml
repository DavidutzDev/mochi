import QtQuick
import QtQuick.Shapes
import qs.island

// The weather as a line icon on a 24x24 grid, in the style of Mochi's own
// symbols: `kind` is clear, cloudy, fog, rain, snow or storm.
Item {
    id: root

    property string kind: "cloudy"
    property bool day: true
    property color color: Theme.foreground
    property real size: 20

    readonly property string cloud: "M17.5 15H9a6 6 0 1 1 5.8-7.5h2.7a3.75 3.75 0 1 1 0 7.5z"
    readonly property var paths: ({
        "clear": "M12 8a4 4 0 1 0 0 8a4 4 0 1 0 0-8zM12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4",
        "night": "M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z",
        "cloudy": "M17.5 19H9a7 7 0 1 1 6.7-9h1.8a4.5 4.5 0 1 1 0 9z",
        "fog": "M4 9h16M6 13h12M4 17h16M8 5h8",
        "rain": cloud + "M8 18l-1 3M12 18l-1 3M16 18l-1 3",
        "snow": cloud + "M8 19v.01M12 19v.01M16 19v.01M10 22v.01M14 22v.01",
        "storm": cloud + "M13 16l-2 3h3l-2 3"
    })
    readonly property string path: kind === "clear" && !day ? paths.night : paths[kind] ?? paths.cloudy

    implicitWidth: size
    implicitHeight: size

    Shape {
        width: 24
        height: 24
        scale: root.size / 24
        transformOrigin: Item.TopLeft
        preferredRendererType: Shape.CurveRenderer

        ShapePath {
            fillColor: "transparent"
            strokeColor: root.color
            strokeWidth: 2
            capStyle: ShapePath.RoundCap
            joinStyle: ShapePath.RoundJoin
            PathSvg {
                path: root.path
            }
        }
    }
}
