import QtQuick
import qs.island

// The World tab: the time here, then in each zone of the `zones` setting,
// two to a row, each with its city, its day next to this computer's, and
// how far ahead or behind it is. The clock module reads the zones' offsets
// from the system, as the widgets module does for the world clock widget.
// Here is the inverted card, like the picked tab in the navbar, so the
// others read against it.
Item {
    id: root

    property var clock: ({})
    readonly property bool twelve: clock.hours === "12"
    readonly property var zones: clock.world ?? []
    // Here first, as an empty zone.
    readonly property var cards: [""].concat(zones)
    readonly property int columns: 2
    readonly property real cardWidth: (width - Theme.spaceMedium * (columns - 1)) / columns
    readonly property real cardHeight: Theme.rowHeight * 2 - Theme.spaceMedium

    ClockTime {
        id: here
    }

    Flickable {
        id: view

        width: parent.width
        anchors.top: parent.top
        anchors.bottom: footer.top
        anchors.bottomMargin: Theme.spaceMedium
        contentWidth: width
        contentHeight: grid.height
        interactive: contentHeight > height
        boundsBehavior: Flickable.StopAtBounds
        clip: true

        ScrollFade {
            view: view
        }

        Grid {
            id: grid

            columns: root.columns
            spacing: Theme.spaceMedium

            Repeater {
                model: root.cards

                Rectangle {
                    id: card

                    required property string modelData
                    readonly property bool home: modelData === ""

                    width: root.cardWidth
                    height: root.cardHeight
                    radius: Theme.radiusSurface
                    color: home ? Theme.foreground : Theme.surface
                    // The text on it.
                    readonly property color ink: home ? Theme.background : Theme.foreground
                    readonly property color faint: home ? Qt.alpha(Theme.background, 0.7) : Theme.muted

                    ClockTime {
                        id: there

                        payload: root.clock
                        zone: card.modelData
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

                    Column {
                        anchors.left: parent.left
                        anchors.leftMargin: Theme.spaceLarge
                        anchors.right: time.left
                        anchors.rightMargin: Theme.spaceSmall
                        anchors.verticalCenter: parent.verticalCenter

                        Text {
                            width: parent.width
                            elide: Text.ElideRight
                            text: card.home ? "Here" : there.city
                            color: card.ink
                            font.pixelSize: Theme.textTitle
                            font.family: Theme.fontFamily
                            font.weight: Theme.weightTitle
                        }

                        Text {
                            width: parent.width
                            elide: Text.ElideRight
                            text: {
                                if (card.home)
                                    return here.date(true);
                                if (there.unknown)
                                    return "No such time zone";
                                if (!there.known)
                                    return "Reading the time zone…";
                                return `${card.day} · ${card.ahead}`;
                            }
                            // A day that isn't today here is what's easiest
                            // to get wrong.
                            color: card.days !== 0 ? card.ink : card.faint
                            font.pixelSize: Theme.textCaption
                            font.family: Theme.fontFamily
                            font.weight: card.days !== 0 ? Theme.weightTitle : Theme.weightBody
                        }
                    }

                    Row {
                        id: time

                        anchors.right: parent.right
                        anchors.rightMargin: Theme.spaceLarge
                        anchors.verticalCenter: parent.verticalCenter
                        visible: there.known
                        spacing: Theme.spaceTiny

                        RollingText {
                            id: digits

                            text: there.time(root.twelve, false)
                            color: card.ink
                            family: Theme.displayFamily
                            weight: Theme.weightTitle
                            pixelSize: Theme.textDisplay * 0.75
                        }

                        Text {
                            anchors.baseline: digits.bottom
                            visible: root.twelve
                            text: there.half
                            color: card.faint
                            font.pixelSize: Theme.textCaption
                            font.family: Theme.fontFamily
                            font.weight: Theme.weightTitle
                        }
                    }
                }
            }
        }
    }

    Item {
        id: footer

        anchors.bottom: parent.bottom
        width: parent.width
        height: Theme.controlHeight

        Text {
            anchors.left: parent.left
            anchors.right: change.left
            anchors.rightMargin: Theme.spaceMedium
            anchors.verticalCenter: parent.verticalCenter
            elide: Text.ElideRight
            text: root.zones.length === 0 ? "No time zones yet. Add some, like Europe/Paris." : root.zones.length === 1 ? "1 time zone" : `${root.zones.length} time zones`
            color: Theme.muted
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }

        ActionButton {
            id: change

            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            text: root.zones.length === 0 ? "Choose time zones" : "Change the zones"
            icon: "edit"
            onClicked: Daemon.command("settings", "open", ["config.module.clock.zones"])
        }
    }
}
