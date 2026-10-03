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

    // On a 24x24 grid: a filled path, holes by the even-odd rule, or an
    // object with a `fill` and a 2px `stroke` path.
    readonly property var icons: ({
        "home": "M12 3.2 3 10.4V20a1 1 0 0 0 1 1h5.5v-6h5v6H20a1 1 0 0 0 1-1v-9.6z",
        "bell": "M12 2a6 6 0 0 0-6 6v3.6l-1.7 3A1 1 0 0 0 5.2 16h13.6a1 1 0 0 0 .9-1.4L18 11.6V8a6 6 0 0 0-6-6zM9.5 18a2.5 2.5 0 0 0 5 0z",
        "music": "M18.5 3.1a1 1 0 0 1 .5.86V15.5a3.5 3.5 0 1 1-2-3.16V7.3l-7 1.75v8.45a3.5 3.5 0 1 1-2-3.16V6.5a1 1 0 0 1 .76-.97l9-2.25a1 1 0 0 1 .74-.18z",
        "clock": "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm0 2a8 8 0 1 1 0 16 8 8 0 0 1 0-16zm-1 3v6.2l4.9 2.9 1-1.7-3.9-2.3V7z",
        "grid": "M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z",
        "moon": "M20.5 14.2A8.5 8.5 0 0 1 9.8 3.5a.6.6 0 0 0-.8-.7A9.5 9.5 0 1 0 21.2 15a.6.6 0 0 0-.7-.8z",
        "volume": "M3 9h4l5-4v14l-5-4H3zM15 8.5a5 5 0 0 1 0 7l-1.4-1.4a3 3 0 0 0 0-4.2zM17.8 5.7a9 9 0 0 1 0 12.6l-1.4-1.4a7 7 0 0 0 0-9.8z",
        "power": { "fill": "", "stroke": "M12 3v9M6.5 6.5a8 8 0 1 0 11 0" },
        "lock": {
            "fill": "M6 11h12a1 1 0 0 1 1 1v8a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1v-8a1 1 0 0 1 1-1z",
            "stroke": "M8 11V7.5a4 4 0 0 1 8 0V11"
        },
        "logout": { "fill": "", "stroke": "M13 4H6v16h7M10 12h10M16.5 8.5 20 12l-3.5 3.5" },
        "reboot": { "fill": "", "stroke": "M20 12a8 8 0 1 1-2.34-5.66M20 4v4.5h-4.5" },
        "snow": { "fill": "", "stroke": "M12 3v18M4.2 7.5l15.6 9M4.2 16.5l15.6-9" },
        "chip": { "fill": "", "stroke": "M8 8h8v8H8zM10 3v3M14 3v3M10 18v3M14 18v3M3 10h3M3 14h3M18 10h3M18 14h3" },
        "leaf": "M20 4C10 4 4 9 4 15c0 2 .6 3.6 1.5 5l1.4-1.4C9 15 12 12.5 16 11c-3.3 2-6 4.6-7.4 8.1.9.6 2.1.9 3.4.9 6 0 8-7 8-16z",
        "bolt": "M13 2 4 14h7l-1 8 9-12h-7z",
        "scale": { "fill": "", "stroke": "M12 4v16M7 20h10M5 7h14M5 7l-2.5 6a2.5 2.5 0 0 0 5 0zM19 7l-2.5 6a2.5 2.5 0 0 0 5 0z" }
    })
    readonly property var icon: icons[name] ?? null
    readonly property string fill: typeof icon === "string" ? icon : icon?.fill ?? ""
    readonly property string stroke: typeof icon === "string" ? "" : icon?.stroke ?? ""

    Shape {
        visible: root.icon !== null
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
                path: root.fill
            }
        }

        ShapePath {
            fillColor: "transparent"
            strokeColor: root.color
            strokeWidth: 2
            capStyle: ShapePath.RoundCap
            joinStyle: ShapePath.RoundJoin
            PathSvg {
                path: root.stroke
            }
        }
    }

    Image {
        anchors.fill: parent
        visible: root.icon === null && status === Image.Ready
        source: root.icon === null && root.name ? Quickshell.iconPath(root.name, true) : ""
        sourceSize.width: root.size * 2
        sourceSize.height: root.size * 2
    }
}
