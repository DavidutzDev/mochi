import QtQuick
import QtQuick.Effects
import qs.island

// The clock widget's looks with the time in numerals, here or in another
// time zone, growing with the widget, their digits rolling as they change:
// digital, the time big over the date; stacked, the hour over the minutes
// and the date under; shape, the time inside a shape in the accent color,
// a cookie unless the `shape` setting picks another; and minimal, the time
// and the date on one line, without a card. Digital and stacked can sit in
// a shape too, and start without one. The analog and world looks have
// views of their own.
Item {
    id: root

    // The widgets module's state, which has each time zone's offset.
    property var payload: null
    property var settings: ({})
    property string instance: ""
    property string variant: ""

    readonly property bool twelve: settings.hours === "12"
    readonly property bool withDate: settings.date !== false
    readonly property bool digitalLook: variant === "" || variant === "digital"
    // Digital or stacked in a shape: the accent behind, the time in its
    // ink, within the shape's room.
    readonly property bool shaped: (digitalLook || variant === "stacked") && (settings.shape ?? "none") !== "none"
    readonly property color ink: shaped ? Theme.onAccent : Theme.foreground
    readonly property color faint: shaped ? Qt.alpha(Theme.onAccent, 0.75) : Theme.muted

    ExpressiveShape {
        id: backdrop

        anchors.fill: parent
        visible: root.shaped
        shape: root.shaped ? root.settings.shape : "circle"
        color: Theme.accent
    }

    // Where digital and stacked draw: the widget, or the shape's room.
    Item {
        id: area

        anchors.centerIn: parent
        width: root.shaped ? backdrop.roomWidth : root.width
        height: root.shaped ? backdrop.roomHeight : root.height
    }

    ClockTime {
        id: time

        payload: root.payload
        zone: root.settings.timezone ?? ""
        // Only the digital look has them.
        seconds: digital.active && root.settings.seconds === true
    }

    // The date, and the zone's city when it isn't this computer's.
    function dated(short: bool): string {
        const date = time.date(short);
        if (time.unknown)
            return `${time.zone}: no such time zone`;
        return time.zone !== "" ? `${date} · ${time.city}` : date;
    }

    Loader {
        id: digital

        anchors.fill: area
        active: root.digitalLook
        sourceComponent: Column {
            id: block

            readonly property string text: time.time(root.twelve, time.seconds)

            y: (area.height - height) / 2
            width: area.width
            spacing: 2

            Row {
                // Centered in a shape, at the left on a card.
                x: root.shaped ? (block.width - width) / 2 : 0
                spacing: Theme.spaceSmall

                RollingText {
                    id: clock

                    text: block.text
                    color: root.ink
                    family: Theme.displayFamily
                    weight: Theme.weightTitle
                    // As big as the widget allows, by height and by width.
                    // "AM" takes about a digit and a half more, at a third
                    // of the size.
                    pixelSize: Math.max(12, Math.min(area.height * (root.withDate ? 0.55 : 0.8), area.width / (text.length * 0.62 + (root.twelve ? 0.75 : 0))))
                }

                Text {
                    id: half

                    // On the time's baseline.
                    y: clockMetrics.ascent - halfMetrics.ascent
                    visible: root.twelve
                    text: time.half
                    color: root.faint
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightTitle
                    font.pixelSize: Math.max(10, clock.pixelSize * 0.3)

                    FontMetrics {
                        id: halfMetrics

                        font: half.font
                    }

                    FontMetrics {
                        id: clockMetrics

                        font: clock.font
                    }
                }
            }

            Text {
                visible: root.withDate
                width: parent.width
                horizontalAlignment: root.shaped ? Text.AlignHCenter : Text.AlignLeft
                elide: Text.ElideRight
                text: root.dated(false)
                color: root.faint
                font.family: Theme.fontFamily
                font.pixelSize: Math.max(Theme.textCaption, clock.font.pixelSize * 0.24)
            }
        }
    }

    Loader {
        anchors.fill: area
        active: root.variant === "stacked"
        sourceComponent: StackedTime {
            hours: time.hour(root.twelve)
            minutes: time.pad(time.parts.minutes)
            date: root.withDate ? root.dated(true) : ""
            color: root.ink
            dateColor: root.faint
        }
    }

    // The accent is the shape; the time sits in it in the accent's ink.
    // Without a shape, the time is in the text's color, as big as a
    // squircle's room.
    Loader {
        anchors.fill: parent
        active: root.variant === "shape"
        sourceComponent: ExpressiveShape {
            id: cookie

            readonly property string text: time.time(root.twelve, false)
            readonly property bool none: root.settings.shape === "none"
            readonly property color ink: none ? Theme.foreground : Theme.onAccent

            shape: none ? "squircle" : root.settings.shape ?? "cookie"
            color: none ? "transparent" : Theme.accent

            Column {
                anchors.centerIn: parent

                RollingText {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: cookie.text
                    color: cookie.ink
                    family: Theme.displayFamily
                    weight: Theme.weightTitle
                    // Across the shape's room, and up to half its height,
                    // leaving room for AM or PM.
                    pixelSize: Math.max(Theme.textCaption, Math.min(cookie.roomHeight * 0.5, cookie.roomWidth / (cookie.text.length * 0.62)))
                }

                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    visible: root.twelve
                    text: time.half
                    color: cookie.ink
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightTitle
                    font.pixelSize: Math.max(Theme.textCaption, cookie.roomHeight * 0.14)
                }
            }
        }
    }

    // No card: a soft halo in the background's color sets the text apart
    // from the wallpaper, a dark one around light text, or a light one
    // around dark text in the light appearance.
    Loader {
        anchors.fill: parent
        active: root.variant === "minimal"
        sourceComponent: Item {
            id: line

            readonly property string text: time.time(root.twelve, false)
            readonly property string date: root.withDate ? root.dated(false) : ""
            // The time as big as the height allows, and smaller when the
            // line wouldn't fit across: a digit is about 0.6 of the size,
            // and the date's letters about 0.55 of a third of it.
            readonly property int size: Math.max(Theme.textCaption, Math.min(height * 0.8, width / (text.length * 0.6 + (root.twelve ? 0.8 : 0) + (date.length * 0.55 + 1) * 0.3)))

            Row {
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width
                spacing: line.size * 0.3
                // Software rendering has no effects; the text shows there
                // without its halo.
                layer.enabled: GraphicsInfo.api !== GraphicsInfo.Software
                layer.effect: MultiEffect {
                    shadowEnabled: true
                    shadowColor: Theme.background
                    shadowBlur: 0.8
                    blurMax: 16
                    shadowOpacity: 1
                    shadowHorizontalOffset: 0
                    shadowVerticalOffset: 0
                }

                RollingText {
                    id: big

                    text: line.text
                    family: Theme.displayFamily
                    weight: Theme.weightBody
                    pixelSize: line.size
                }

                // The rest on the time's baseline.
                Text {
                    id: rest

                    y: bigMetrics.ascent - smallMetrics.ascent
                    width: Math.min(implicitWidth, parent.width - big.width - parent.spacing)
                    visible: text !== ""
                    elide: Text.ElideRight
                    text: [root.twelve ? time.half : "", line.date].filter(part => part !== "").join("  ")
                    color: Theme.foreground
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightLabel
                    font.pixelSize: Math.max(Theme.textCaption, line.size * 0.3)

                    FontMetrics {
                        id: smallMetrics

                        font: rest.font
                    }

                    FontMetrics {
                        id: bigMetrics

                        font: big.font
                    }
                }
            }
        }
    }
}
