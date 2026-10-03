import QtQuick
import QtQuick.Shapes
import qs.island

// Line icons drawn on a 24x24 grid in the theme's colors, so they follow
// theme.toml instead of the system icon theme.
Item {
    id: root

    required property string name
    property color color: Theme.foreground
    property real size: 20

    implicitWidth: size
    implicitHeight: size

    readonly property string speaker: "M3 9h4l5-4v14l-5-4H3z"
    readonly property string wave1: "M15.5 9.5a3.5 3.5 0 0 1 0 5"
    readonly property string wave2: "M18 7a7 7 0 0 1 0 10"
    readonly property string wave3: "M20.5 4.5a10.5 10.5 0 0 1 0 15"
    readonly property string micBody: "M9 5a3 3 0 0 1 6 0v6a3 3 0 0 1-6 0z"
    readonly property string micStand: "M5.5 11a6.5 6.5 0 0 0 13 0M12 17.5V21M8.5 21h7"

    // Each icon is a filled part and a stroked part, both SVG path data.
    readonly property var icons: ({
        "volume-0": { fill: speaker, stroke: "" },
        "volume-1": { fill: speaker, stroke: wave1 },
        "volume-2": { fill: speaker, stroke: `${wave1}${wave2}` },
        "volume-3": { fill: speaker, stroke: `${wave1}${wave2}${wave3}` },
        "volume-muted": { fill: speaker, stroke: "M16 9.5l5 5M21 9.5l-5 5" },
        "mic": { fill: micBody, stroke: micStand },
        "mic-muted": { fill: micBody, stroke: `${micStand}M4 3l16 18` },
        "headset": {
            fill: "M3 14h4v7H5a2 2 0 0 1-2-2zM17 14h4v5a2 2 0 0 1-2 2h-2z",
            stroke: "M4 15v-3a8 8 0 0 1 16 0v3"
        },
        "speakers": {
            fill: "M10.75 7a1.25 1.25 0 1 0 2.5 0a1.25 1.25 0 1 0-2.5 0z",
            stroke: "M7 2.5h10a1.5 1.5 0 0 1 1.5 1.5v16a1.5 1.5 0 0 1-1.5 1.5H7a1.5 1.5 0 0 1-1.5-1.5V4A1.5 1.5 0 0 1 7 2.5zM8.5 15a3.5 3.5 0 1 0 7 0a3.5 3.5 0 1 0-7 0"
        },
        "display": {
            fill: "",
            stroke: "M4 4.5h16a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1v-10a1 1 0 0 1 1-1zM9 20.5h6M12 16.5v4"
        },
        "caps-lock": { fill: "", stroke: "M12 4l-7 7h4v5h6v-5h4zM9 20h6" },
        "num-lock": {
            fill: "",
            stroke: "M6 3.5h12a2.5 2.5 0 0 1 2.5 2.5v12a2.5 2.5 0 0 1-2.5 2.5H6A2.5 2.5 0 0 1 3.5 18V6A2.5 2.5 0 0 1 6 3.5zM10.5 9l2-1.5V17M10 17h5"
        }
    })
    readonly property var icon: icons[name] ?? { fill: "", stroke: "" }

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
                path: root.icon.fill
            }
        }

        ShapePath {
            fillColor: "transparent"
            strokeColor: root.color
            strokeWidth: 2
            capStyle: ShapePath.RoundCap
            joinStyle: ShapePath.RoundJoin
            PathSvg {
                path: root.icon.stroke
            }
        }
    }
}
