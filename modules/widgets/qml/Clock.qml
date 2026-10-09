import QtQuick
import QtQuick.Effects
import qs.island

// The clock widget's looks with the time in numerals, here or in another
// time zone, growing with the widget, their digits rolling as they change:
// digital, the time big over the date; stacked, the hour over the minutes
// and the date under; shape, the time inside a cookie in the accent color;
// and minimal, the time and the date on one line, without a card. The
// analog and world looks have views of their own.
Item {
    id: root

    // The widgets module's state, which has each time zone's offset.
    property var payload: null
    property var settings: ({})
    property string instance: ""
    property string variant: ""

    readonly property bool twelve: settings.hours === "12"
    readonly property bool withDate: settings.date !== false

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

        anchors.fill: parent
        active: root.variant === "" || root.variant === "digital"
        sourceComponent: Column {
            id: block

            readonly property string text: time.time(root.twelve, time.seconds)

            y: (root.height - height) / 2
            width: root.width
            spacing: 2

            Row {
                spacing: Theme.spaceSmall

                RollingText {
                    id: clock

                    text: block.text
                    family: Theme.displayFamily
                    weight: Theme.weightTitle
                    // As big as the widget allows, by height and by width.
                    // "AM" takes about a digit and a half more, at a third
                    // of the size.
                    pixelSize: Math.max(12, Math.min(root.height * (root.withDate ? 0.55 : 0.8), root.width / (text.length * 0.62 + (root.twelve ? 0.75 : 0))))
                }

                Text {
                    id: half

                    // On the time's baseline.
                    y: clockMetrics.ascent - halfMetrics.ascent
                    visible: root.twelve
                    text: time.half
                    color: Theme.muted
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
                elide: Text.ElideRight
                text: root.dated(false)
                color: Theme.muted
                font.family: Theme.fontFamily
                font.pixelSize: Math.max(Theme.textCaption, clock.font.pixelSize * 0.24)
            }
        }
    }

    Loader {
        anchors.fill: parent
        active: root.variant === "stacked"
        sourceComponent: StackedTime {
            hours: time.hour(root.twelve)
            minutes: time.pad(time.parts.minutes)
            date: root.withDate ? root.dated(true) : ""
        }
    }

    // The accent is the cookie; the time sits in it in the accent's ink.
    Loader {
        anchors.fill: parent
        active: root.variant === "shape"
        sourceComponent: ExpressiveShape {
            id: cookie

            readonly property string text: time.time(root.twelve, false)

            shape: "cookie"
            color: Theme.accent

            Column {
                anchors.centerIn: parent

                RollingText {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: cookie.text
                    color: Theme.onAccent
                    family: Theme.displayFamily
                    weight: Theme.weightTitle
                    // Within the cookie's middle, about seven tenths of it.
                    pixelSize: Math.max(Theme.textCaption, Math.min(cookie.side * 0.3, cookie.side * 0.7 / (cookie.text.length * 0.62)))
                }

                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    visible: root.twelve
                    text: time.half
                    color: Theme.onAccent
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightTitle
                    font.pixelSize: Math.max(Theme.textCaption, cookie.side * 0.09)
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
