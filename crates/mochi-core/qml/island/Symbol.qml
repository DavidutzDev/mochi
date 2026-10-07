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
    readonly property string speaker: "M3 9h4l5-4v14l-5-4H3z"
    readonly property string wave1: "M15.5 9.5a3.5 3.5 0 0 1 0 5"
    readonly property string wave2: "M18 7a7 7 0 0 1 0 10"
    readonly property string wave3: "M20.5 4.5a10.5 10.5 0 0 1 0 15"
    readonly property string micBody: "M9 5a3 3 0 0 1 6 0v6a3 3 0 0 1-6 0z"
    readonly property string micStand: "M5.5 11a6.5 6.5 0 0 0 13 0M12 17.5V21M8.5 21h7"

    readonly property var icons: ({
        // Places and things.
        "home": "M12 3.2 3 10.4V20a1 1 0 0 0 1 1h5.5v-6h5v6H20a1 1 0 0 0 1-1v-9.6z",
        "bell": "M12 2a6 6 0 0 0-6 6v3.6l-1.7 3A1 1 0 0 0 5.2 16h13.6a1 1 0 0 0 .9-1.4L18 11.6V8a6 6 0 0 0-6-6zM9.5 18a2.5 2.5 0 0 0 5 0z",
        "music": "M18.5 3.1a1 1 0 0 1 .5.86V15.5a3.5 3.5 0 1 1-2-3.16V7.3l-7 1.75v8.45a3.5 3.5 0 1 1-2-3.16V6.5a1 1 0 0 1 .76-.97l9-2.25a1 1 0 0 1 .74-.18z",
        "clock": "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm0 2a8 8 0 1 1 0 16 8 8 0 0 1 0-16zm-1 3v6.2l4.9 2.9 1-1.7-3.9-2.3V7z",
        "grid": "M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z",
        "moon": "M20.5 14.2A8.5 8.5 0 0 1 9.8 3.5a.6.6 0 0 0-.8-.7A9.5 9.5 0 1 0 21.2 15a.6.6 0 0 0-.7-.8z",
        "search": "M10 3a7 7 0 1 0 4.2 12.6l4.6 4.6 1.4-1.4-4.6-4.6A7 7 0 0 0 10 3zm0 2a5 5 0 1 1 0 10 5 5 0 0 1 0-10z",

        // Media.
        "play": "M8 5.5v13a1 1 0 0 0 1.5.86l10.5-6.5a1 1 0 0 0 0-1.72L9.5 4.64A1 1 0 0 0 8 5.5z",
        "pause": "M6 5a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1v14a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1zM13 5a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1v14a1 1 0 0 1-1 1h-3a1 1 0 0 1-1-1z",
        "next": "M4 6.4v11.2a1 1 0 0 0 1.55.83L14 12.8a1 1 0 0 0 0-1.6L5.55 5.57A1 1 0 0 0 4 6.4zM17 5h2a1 1 0 0 1 1 1v12a1 1 0 0 1-1 1h-2a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1z",
        "previous": "M20 6.4v11.2a1 1 0 0 1-1.55.83L10 12.8a1 1 0 0 1 0-1.6l8.45-5.63A1 1 0 0 1 20 6.4zM7 5H5a1 1 0 0 0-1 1v12a1 1 0 0 0 1 1h2a1 1 0 0 0 1-1V6a1 1 0 0 0-1-1z",
        "note": "M18.5 3.1a1 1 0 0 1 .5.86V15.5a3.5 3.5 0 1 1-2-3.16V7.3l-7 1.75v8.45a3.5 3.5 0 1 1-2-3.16V6.5a1 1 0 0 1 .76-.97l9-2.25a1 1 0 0 1 .74-.18z",

        // Audio and devices.
        "volume": { "fill": speaker, "stroke": `${wave1}${wave2}` },
        "volume-0": { "fill": speaker, "stroke": "" },
        "volume-1": { "fill": speaker, "stroke": wave1 },
        "volume-2": { "fill": speaker, "stroke": `${wave1}${wave2}` },
        "volume-3": { "fill": speaker, "stroke": `${wave1}${wave2}${wave3}` },
        "volume-muted": { "fill": speaker, "stroke": "M16 9.5l5 5M21 9.5l-5 5" },
        "mic": { "fill": micBody, "stroke": micStand },
        "mic-muted": { "fill": micBody, "stroke": `${micStand}M4 3l16 18` },
        "headset": { "fill": "M3 14h4v7H5a2 2 0 0 1-2-2zM17 14h4v5a2 2 0 0 1-2 2h-2z", "stroke": "M4 15v-3a8 8 0 0 1 16 0v3" },
        "speakers": {
            "fill": "M10.75 7a1.25 1.25 0 1 0 2.5 0a1.25 1.25 0 1 0-2.5 0z",
            "stroke": "M7 2.5h10a1.5 1.5 0 0 1 1.5 1.5v16a1.5 1.5 0 0 1-1.5 1.5H7a1.5 1.5 0 0 1-1.5-1.5V4A1.5 1.5 0 0 1 7 2.5zM8.5 15a3.5 3.5 0 1 0 7 0a3.5 3.5 0 1 0-7 0"
        },
        "display": { "fill": "", "stroke": "M4 4.5h16a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1v-10a1 1 0 0 1 1-1zM9 20.5h6M12 16.5v4" },
        "caps-lock": { "fill": "", "stroke": "M12 4l-7 7h4v5h6v-5h4zM9 20h6" },
        "num-lock": {
            "fill": "",
            "stroke": "M6 3.5h12a2.5 2.5 0 0 1 2.5 2.5v12a2.5 2.5 0 0 1-2.5 2.5H6A2.5 2.5 0 0 1 3.5 18V6A2.5 2.5 0 0 1 6 3.5zM10.5 9l2-1.5V17M10 17h5"
        },

        // Connections.
        "wifi": { "fill": "M10.5 18.5a1.5 1.5 0 1 0 3 0 1.5 1.5 0 1 0-3 0", "stroke": "M3 9a13 13 0 0 1 18 0M6 12.5a8.5 8.5 0 0 1 12 0M9 15.8a4 4 0 0 1 6 0" },
        "wifi-2": { "fill": "M10.5 18.5a1.5 1.5 0 1 0 3 0 1.5 1.5 0 1 0-3 0", "stroke": "M6 12.5a8.5 8.5 0 0 1 12 0M9 15.8a4 4 0 0 1 6 0" },
        "wifi-1": { "fill": "M10.5 18.5a1.5 1.5 0 1 0 3 0 1.5 1.5 0 1 0-3 0", "stroke": "M9 15.8a4 4 0 0 1 6 0" },
        "wifi-off": { "fill": "M10.5 18.5a1.5 1.5 0 1 0 3 0 1.5 1.5 0 1 0-3 0", "stroke": "M3 9a13 13 0 0 1 18 0M6 12.5a8.5 8.5 0 0 1 12 0M9 15.8a4 4 0 0 1 6 0M4 3l16 18" },
        "ethernet": { "fill": "", "stroke": "M9.5 3.5h5v5h-5zM3.5 15.5h5v5h-5zM15.5 15.5h5v5h-5zM12 8.5V12M6 15.5V12h12v3.5" },
        "offline": { "fill": "", "stroke": "M12 3.5a8.5 8.5 0 1 0 0 17 8.5 8.5 0 1 0 0-17zM3.5 12h17M12 3.5c2.3 2.4 3.5 5.3 3.5 8.5s-1.2 6.1-3.5 8.5M12 3.5C9.7 5.9 8.5 8.8 8.5 12s1.2 6.1 3.5 8.5M4 4l16 16" },
        "airplane": "M21 16v-2l-8-5V3.5a1.5 1.5 0 0 0-3 0V9l-8 5v2l8-2.5V19l-2 1.5V22l3.5-1 3.5 1v-1.5L13 19v-5.5z",
        "bluetooth": { "fill": "", "stroke": "M7 7.5l10 9-5 4.5V3l5 4.5-10 9" },

        // Power.
        "power": { "fill": "", "stroke": "M12 3v9M6.5 6.5a8 8 0 1 0 11 0" },
        "lock": {
            "fill": "M6 11h12a1 1 0 0 1 1 1v8a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1v-8a1 1 0 0 1 1-1z",
            "stroke": "M8 11V7.5a4 4 0 0 1 8 0V11"
        },
        "logout": { "fill": "", "stroke": "M13 4H6v16h7M10 12h10M16.5 8.5 20 12l-3.5 3.5" },
        "reboot": { "fill": "", "stroke": "M20 12a8 8 0 1 1-2.34-5.66M20 4v4.5h-4.5" },
        "snow": { "fill": "", "stroke": "M12 3v18M4.2 7.5l15.6 9M4.2 16.5l15.6-9" },
        "chip": { "fill": "", "stroke": "M8 8h8v8H8zM10 3v3M14 3v3M10 18v3M14 18v3M3 10h3M3 14h3M18 10h3M18 14h3" },
        "memory": { "fill": "", "stroke": "M3.5 7.5h17v8h-17zM7 7.5v-2M10.5 7.5v-2M14 7.5v-2M17.5 7.5v-2M6 18.5v-3M9.5 18.5v-3M14.5 18.5v-3M18 18.5v-3M7 10.5v2M10.5 10.5v2M14 10.5v2M17.5 10.5v2" },
        "gpu": { "fill": "", "stroke": "M3 6.5h18v11H3zM6 17.5v2.5M9.5 17.5v2.5M13 12a3 3 0 1 0 6 0a3 3 0 1 0-6 0M6.5 10h3M6.5 13.5h3" },
        "disk": { "fill": "M16.5 15a1.2 1.2 0 1 0 0 2.4 1.2 1.2 0 1 0 0-2.4z", "stroke": "M3.5 12.5l2.5-7h12l2.5 7v6h-17zM3.5 12.5h17M6.5 16.2h5" },
        "temperature": { "fill": "M12 15.2a2.3 2.3 0 1 0 0 4.6 2.3 2.3 0 1 0 0-4.6z", "stroke": "M10 14.3V5a2 2 0 0 1 4 0v9.3a4.3 4.3 0 1 1-4 0zM12 9v6.5" },
        "leaf": "M20 4C10 4 4 9 4 15c0 2 .6 3.6 1.5 5l1.4-1.4C9 15 12 12.5 16 11c-3.3 2-6 4.6-7.4 8.1.9.6 2.1.9 3.4.9 6 0 8-7 8-16z",
        "bolt": "M13 2 4 14h7l-1 8 9-12h-7z",
        "scale": { "fill": "", "stroke": "M12 4v16M7 20h10M5 7h14M5 7l-2.5 6a2.5 2.5 0 0 0 5 0zM19 7l-2.5 6a2.5 2.5 0 0 0 5 0z" },

        // Capture.
        "camera": "M9.2 4h5.6a1 1 0 0 1 .83.45L16.9 6.5H19a2 2 0 0 1 2 2V18a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8.5a2 2 0 0 1 2-2h2.1l1.27-2.05A1 1 0 0 1 9.2 4zM12 9a4 4 0 1 0 0 8 4 4 0 0 0 0-8z",
        "video": "M5 6h9a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2zM17.5 10.2l3.5-2.2v8l-3.5-2.2z",
        "record": "M12 5a7 7 0 1 0 0 14 7 7 0 0 0 0-14z",
        "stop": "M7.5 6h9A1.5 1.5 0 0 1 18 7.5v9a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 6 16.5v-9A1.5 1.5 0 0 1 7.5 6z",
        "region": { "fill": "", "stroke": "M4 9V5a1 1 0 0 1 1-1h4M15 4h4a1 1 0 0 1 1 1v4M20 15v4a1 1 0 0 1-1 1h-4M9 20H5a1 1 0 0 1-1-1v-4" },
        "window": { "fill": "", "stroke": "M5 4.5h14a1.5 1.5 0 0 1 1.5 1.5v12a1.5 1.5 0 0 1-1.5 1.5H5A1.5 1.5 0 0 1 3.5 18V6A1.5 1.5 0 0 1 5 4.5zM3.5 9h17" },
        "copy": { "fill": "", "stroke": "M9.5 9h9a1.5 1.5 0 0 1 1.5 1.5v9a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 8 19.5v-9A1.5 1.5 0 0 1 9.5 9zM16 9V5.5A1.5 1.5 0 0 0 14.5 4h-9A1.5 1.5 0 0 0 4 5.5v9A1.5 1.5 0 0 0 5.5 16H8" },
        "clipboard": { "fill": "", "stroke": "M9 4.5H7A1.5 1.5 0 0 0 5.5 6v13.5A1.5 1.5 0 0 0 7 21h10a1.5 1.5 0 0 0 1.5-1.5V6A1.5 1.5 0 0 0 17 4.5h-2M9.5 3h5a.5.5 0 0 1 .5.5v2a.5.5 0 0 1-.5.5h-5a.5.5 0 0 1-.5-.5v-2a.5.5 0 0 1 .5-.5z" },
        "palette": { "fill": "M6.25 12a1.25 1.25 0 1 0 2.5 0a1.25 1.25 0 1 0 -2.5 0zM8.25 7.75a1.25 1.25 0 1 0 2.5 0a1.25 1.25 0 1 0 -2.5 0zM13.25 7.75a1.25 1.25 0 1 0 2.5 0a1.25 1.25 0 1 0 -2.5 0z", "stroke": "M12 3a9 9 0 0 0 0 18c.9 0 1.5-.6 1.5-1.4 0-.4-.1-.7-.4-1a1.5 1.5 0 0 1 1.1-2.6H16a5 5 0 0 0 5-5c0-4.4-4-8-9-8z" },
        "edit": { "fill": "", "stroke": "M4 20h4L19 9a2.83 2.83 0 0 0-4-4L4 16zM13.5 6.5l4 4" },
        "pin": { "fill": "", "stroke": "M8.5 3.5h7M10 3.5v5L7 12.5h10l-3-4v-5M12 12.5v8" },
        "trash": { "fill": "", "stroke": "M4 7h16M10 3.5h4M6 7l1 12a1.5 1.5 0 0 0 1.5 1.5h7A1.5 1.5 0 0 0 17 19l1-12M10 11v5.5M14 11v5.5" },
        "folder": "M4.5 4.5h4.4a1.5 1.5 0 0 1 1.06.44L11.5 6.5h8A1.5 1.5 0 0 1 21 8v10a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 18V6a1.5 1.5 0 0 1 1.5-1.5z",

        // Controls.
        "close": { "fill": "", "stroke": "M6.5 6.5l11 11M17.5 6.5l-11 11" },
        "check": { "fill": "", "stroke": "M5 12.5l4.5 4.5L19 7.5" },
        "chevron": { "fill": "", "stroke": "M9.5 6l6 6-6 6" },
        "plus": { "fill": "", "stroke": "M12 5v14M5 12h14" },
        "minus": { "fill": "", "stroke": "M5 12h14" },
        "tray": { "fill": "", "stroke": "M4 13.5h4.5l1.5 2.5h4l1.5-2.5H20M4 13.5V18a1.5 1.5 0 0 0 1.5 1.5h13A1.5 1.5 0 0 0 20 18v-4.5M4 13.5 6.5 6h11l2.5 7.5" },
        "dot": "M12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 1 0 0-7z"
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
