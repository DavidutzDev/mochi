import QtQuick
import qs.island

// The desktop widget's looks of the weather now. `current`: the sky as a
// large icon, the temperature beside it, and the sky in words with the
// place and today's low and high under it. `icon`: the sky's icon in a
// cookie, the temperature big beside it over the sky in words. Everything
// grows with the widget. Until a place is set, they say so. The forecast
// and the hours have views of their own.
Item {
    id: root

    property var payload: null
    // What the desktop gives a widget; this one has no settings.
    property var settings: ({})
    property string instance: ""
    property string variant: ""
    readonly property bool cookie: variant === "icon"
    readonly property var current: payload?.current ?? null
    readonly property var today: payload?.daily?.[0] ?? null
    readonly property string unit: payload?.unit?.temperature ?? "°"
    // The icon and the temperature follow the widget's height, and its
    // width so they fit side by side.
    readonly property real big: Math.max(18, Math.min(height * 0.45, width / 5))

    implicitWidth: 256
    implicitHeight: 96

    // No place yet, or no forecast yet.
    Missing {
        anchors.verticalCenter: parent.verticalCenter
        width: parent.width
        visible: root.current === null
        payload: root.payload
        room: root.height
    }

    Row {
        anchors.verticalCenter: parent.verticalCenter
        width: parent.width
        visible: root.current !== null && !root.cookie
        spacing: root.big * 0.3

        Symbol {
            id: icon

            anchors.verticalCenter: parent.verticalCenter
            name: root.current?.icon ?? "cloud"
            size: root.big * 1.4
            color: Theme.accent
        }

        Column {
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - icon.width - parent.spacing

            RollingText {
                text: `${Math.round(root.current?.temperature ?? 0)}${root.unit}`
                pixelSize: root.big
                family: Theme.displayFamily
                weight: Theme.weightTitle
            }

            Text {
                width: parent.width
                text: root.current?.text ?? ""
                elide: Text.ElideRight
                color: Theme.foreground
                font.pixelSize: Math.max(Theme.textBody, root.big * 0.36)
                font.family: Theme.fontFamily
                font.weight: Theme.weightLabel
            }

            // Left out when the widget is too low for three lines, and
            // how old the forecast is while it's stale.
            Text {
                width: parent.width
                visible: root.height >= 60 && !age.stale
                text: {
                    const range = root.today ? `${Math.round(root.today.min)}° to ${Math.round(root.today.max)}°` : "";
                    return [root.payload?.place?.name, range].filter(part => part).join(" · ");
                }
                elide: Text.ElideRight
                color: Theme.muted
                font.pixelSize: Math.max(Theme.textCaption, root.big * 0.3)
                font.family: Theme.fontFamily
            }

            Age {
                id: age

                width: parent.width
                visible: root.height >= 60 && stale
                payload: root.payload
                pixelSize: Math.max(Theme.textCaption, root.big * 0.3)
            }
        }
    }

    // The icon look: the cookie as high as the widget, up to a third of its
    // width, and the temperature at about half the cookie.
    Row {
        id: blob

        readonly property real side: Math.min(root.height, root.width * 0.36)

        anchors.verticalCenter: parent.verticalCenter
        width: parent.width
        visible: root.current !== null && root.cookie
        spacing: Math.max(Theme.spaceMedium, side * 0.16)

        ExpressiveShape {
            id: shape

            anchors.verticalCenter: parent.verticalCenter
            width: blob.side
            height: blob.side
            shape: "cookie"

            Symbol {
                anchors.centerIn: parent
                name: root.current?.icon ?? "cloud"
                size: Math.round(blob.side * 0.5)
                color: Theme.onAccent
                filled: true
            }
        }

        Column {
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - shape.width - parent.spacing

            RollingText {
                text: `${Math.round(root.current?.temperature ?? 0)}°`
                pixelSize: Math.max(18, Math.min(blob.side * 0.46, parent.width / 2.6))
                family: Theme.displayFamily
                weight: Theme.weightTitle
            }

            Text {
                width: parent.width
                text: root.current?.text ?? ""
                elide: Text.ElideRight
                color: Theme.foreground
                font.pixelSize: Math.max(Theme.textBody, blob.side * 0.16)
                font.family: Theme.fontFamily
                font.weight: Theme.weightLabel
            }

            Age {
                width: parent.width
                visible: stale && root.height >= 72
                payload: root.payload
                pixelSize: Math.max(Theme.textCaption, blob.side * 0.12)
            }
        }
    }
}
