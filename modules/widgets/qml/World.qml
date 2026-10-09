import QtQuick
import qs.island

// The clock widget's world look: a line for each zone in `zones`, up to
// four, with the city, how far ahead or behind this computer it is, and its
// time. The widgets module reads each zone's offset from the system, as
// for the other looks. A day that isn't today here stands out in the
// text's color and weight: it's what's easiest to get wrong.
Item {
    id: root

    property var payload: null
    property var settings: ({})
    property string instance: ""
    property string variant: ""

    readonly property bool twelve: settings.hours === "12"
    // Read like the module reads them: commas or spaces between, or a list.
    readonly property var zones: {
        const value = settings.zones ?? "";
        const list = Array.isArray(value) ? value : `${value}`.split(/[,\s]+/);
        return list.map(zone => `${zone}`.trim()).filter(zone => zone !== "").slice(0, 4);
    }
    readonly property real rowHeight: zones.length > 0 ? (height - Theme.spaceSmall * (zones.length - 1)) / zones.length : 0

    // This computer's day, to tell the zones' apart from it.
    ClockTime {
        id: here
    }

    Text {
        anchors.fill: parent
        visible: root.zones.length === 0
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        wrapMode: Text.Wrap
        text: "Add time zones, like Europe/London, in its settings"
        color: Theme.muted
        font.family: Theme.fontFamily
        font.pixelSize: Theme.textBody
    }

    Column {
        anchors.fill: parent
        spacing: Theme.spaceSmall

        Repeater {
            model: root.zones

            Item {
                id: row

                required property string modelData

                width: parent.width
                height: root.rowHeight

                ClockTime {
                    id: there

                    payload: root.payload
                    zone: row.modelData
                    ticking: false
                    now: here.now
                }

                // Days from here's date to the zone's: -1, 0 or 1.
                readonly property int days: {
                    const theirs = Date.UTC(there.parts.year, there.parts.month, there.parts.date);
                    const ours = Date.UTC(here.parts.year, here.parts.month, here.parts.date);
                    return Math.round((theirs - ours) / 86400000);
                }
                // Hours and minutes ahead of here, like "+9 h" or "−4 h 30".
                readonly property string ahead: {
                    const local = -here.now.getTimezoneOffset() * 60;
                    const difference = Math.round(((there.offset ?? local) - local) / 60);
                    if (difference === 0)
                        return "Same time";
                    const hours = Math.floor(Math.abs(difference) / 60);
                    const minutes = Math.abs(difference) % 60;
                    const sign = difference > 0 ? "+" : "−";
                    return minutes > 0 ? `${sign}${hours} h ${minutes}` : `${sign}${hours} h`;
                }
                readonly property string day: days > 0 ? "Tomorrow" : days < 0 ? "Yesterday" : "Today"
                // The time's size, from the row's height, and narrower rows.
                readonly property int size: Math.max(Theme.textBody, Math.min(height * 0.62, Theme.textDisplay, width * 0.16))

                Column {
                    anchors.left: parent.left
                    anchors.right: clock.left
                    anchors.rightMargin: Theme.spaceSmall
                    anchors.verticalCenter: parent.verticalCenter

                    Text {
                        width: parent.width
                        elide: Text.ElideRight
                        text: there.city
                        color: Theme.foreground
                        font.family: Theme.fontFamily
                        font.weight: Theme.weightTitle
                        font.pixelSize: Math.max(Theme.textBody, Math.min(Theme.textHeadline, row.size * 0.42))
                    }

                    Row {
                        width: parent.width
                        spacing: Theme.spaceTiny

                        Text {
                            visible: there.known
                            text: row.day
                            color: row.days !== 0 ? Theme.foreground : Theme.muted
                            font.family: Theme.fontFamily
                            font.weight: row.days !== 0 ? Theme.weightTitle : Theme.weightBody
                            font.pixelSize: Theme.textCaption
                        }

                        Text {
                            width: Math.min(implicitWidth, parent.width)
                            elide: Text.ElideRight
                            text: there.unknown ? "No such time zone" : !there.known ? "Reading the time zone" : `· ${row.ahead}`
                            color: Theme.muted
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.textCaption
                        }
                    }
                }

                Row {
                    id: clock

                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    visible: there.known
                    spacing: Theme.spaceTiny

                    RollingText {
                        id: digits

                        text: there.time(root.twelve, false)
                        family: Theme.displayFamily
                        weight: Theme.weightTitle
                        pixelSize: row.size
                    }

                    Text {
                        id: half

                        y: digitsMetrics.ascent - halfMetrics.ascent
                        visible: root.twelve
                        text: there.half
                        color: Theme.muted
                        font.family: Theme.fontFamily
                        font.weight: Theme.weightTitle
                        font.pixelSize: Math.max(Theme.textCaption, row.size * 0.3)

                        FontMetrics {
                            id: halfMetrics

                            font: half.font
                        }

                        FontMetrics {
                            id: digitsMetrics

                            font: digits.font
                        }
                    }
                }
            }
        }
    }
}
