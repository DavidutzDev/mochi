import QtQuick
import qs.island

// The World tab: the time here, then in each zone of the `zones` setting,
// two to a row, each with its city, its day next to this computer's, and
// how far ahead or behind it is. The clock module reads the zones' offsets
// from the system, as the widgets module does for the world clock widget.
// Here is the inverted card, like the picked tab in the navbar, so the
// others read against it. "Add a city" lays a list of common cities over
// the cards, west to east, to add or take off with a click.
Item {
    id: root

    property var clock: ({})
    readonly property bool twelve: clock.hours === "12"
    readonly property var zones: clock.world ?? []
    // The most the clock module takes.
    readonly property int most: 8
    // Cities whose zone has their name, so the card says the same.
    readonly property var cities: ["Pacific/Honolulu", "America/Los_Angeles", "America/Denver", "America/Chicago", "America/Mexico_City", "America/New_York", "America/Toronto", "America/Sao_Paulo", "America/Argentina/Buenos_Aires", "Europe/London", "Europe/Lisbon", "Africa/Lagos", "Europe/Paris", "Europe/Madrid", "Europe/Amsterdam", "Europe/Berlin", "Europe/Rome", "Europe/Stockholm", "Africa/Johannesburg", "Africa/Cairo", "Europe/Athens", "Europe/Istanbul", "Africa/Nairobi", "Europe/Moscow", "Asia/Dubai", "Asia/Kolkata", "Asia/Bangkok", "Asia/Singapore", "Asia/Hong_Kong", "Asia/Shanghai", "Asia/Seoul", "Asia/Tokyo", "Australia/Sydney", "Pacific/Auckland"]
    property bool picking: false

    // Back to the cards, the keyboard on the button that opened the list.
    function closePicker(): void {
        picking = false;
        addCity.forceActiveFocus();
    }
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

        // The list of cities takes its place.
        visible: !root.picking
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

        visible: !root.picking
        anchors.bottom: parent.bottom
        width: parent.width
        height: Theme.controlHeight

        Text {
            anchors.left: parent.left
            anchors.right: change.left
            anchors.rightMargin: Theme.spaceMedium
            anchors.verticalCenter: parent.verticalCenter
            elide: Text.ElideRight
            text: root.zones.length === 0 ? "No time zones yet. Add a city, or any zone in the settings." : root.zones.length === 1 ? "1 time zone" : `${root.zones.length} time zones`
            color: Theme.muted
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }

        Row {
            id: change

            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.spaceSmall

            ActionButton {
                text: "Change the zones"
                icon: "edit"
                tone: "ghost"
                onClicked: Daemon.command("settings", "open", ["config.module.clock.zones"])
            }

            ActionButton {
                id: addCity

                text: "Add a city"
                icon: "add"
                onClicked: {
                    root.picking = true;
                    picker.forceActiveFocus();
                }
            }
        }
    }

    // The cities, over the cards. Escape or Done goes back to them.
    Rectangle {
        id: picker

        anchors.fill: parent
        visible: root.picking
        radius: Theme.radiusSurface
        color: Theme.surface
        Keys.onEscapePressed: root.closePicker()

        // Clicks stay here.
        MouseArea {
            anchors.fill: parent
        }

        Column {
            id: heading

            x: Theme.spaceLarge
            y: Theme.spaceLarge
            width: parent.width - Theme.spaceLarge * 2 - done.width

            Text {
                text: "Add a city"
                color: Theme.foreground
                font.pixelSize: Theme.textTitle
                font.family: Theme.fontFamily
                font.weight: Theme.weightTitle
            }

            Text {
                width: parent.width
                elide: Text.ElideRight
                text: root.zones.length >= root.most ? `The World tab is full, with ${root.most} zones. Take one off to add another.` : `A click adds a city or takes it off. ${root.zones.length} of ${root.most} zones.`
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }
        }

        ActionButton {
            id: done

            anchors.right: parent.right
            anchors.rightMargin: Theme.spaceLarge
            anchors.verticalCenter: heading.verticalCenter
            text: "Done"
            icon: "check"
            onClicked: root.closePicker()
        }

        Flickable {
            id: cityView

            anchors.top: heading.bottom
            anchors.topMargin: Theme.spaceMedium
            anchors.bottom: parent.bottom
            anchors.bottomMargin: Theme.spaceMedium
            x: Theme.spaceLarge
            width: parent.width - Theme.spaceLarge * 2
            contentWidth: width
            contentHeight: flow.height
            interactive: contentHeight > height
            boundsBehavior: Flickable.StopAtBounds
            clip: true

            ScrollFade {
                view: cityView
                color: Theme.surface
            }

            Flow {
                id: flow

                width: parent.width
                spacing: Theme.spaceSmall

                Repeater {
                    model: root.cities

                    // A chip, filled while the city is on the World tab.
                    Rectangle {
                        id: chip

                        required property string modelData
                        readonly property bool chosen: root.zones.includes(modelData)
                        readonly property bool open: chosen || root.zones.length < root.most

                        function pick(): void {
                            if (open)
                                Daemon.command("clock", chosen ? "remove-zone" : "add-zone", [modelData]);
                        }

                        width: label.implicitWidth + Theme.spaceLarge + Theme.spaceSmall
                        height: Theme.controlHeight - Theme.spaceTiny
                        radius: height / 2
                        opacity: open ? 1 : 0.4
                        color: chosen ? Theme.foreground : area.containsMouse && open ? Theme.highlight : Theme.raised
                        activeFocusOnTab: open
                        Keys.onReturnPressed: pick()
                        Keys.onEnterPressed: pick()
                        Keys.onSpacePressed: pick()

                        Row {
                            id: label

                            anchors.centerIn: parent
                            spacing: Theme.spaceTiny

                            Symbol {
                                anchors.verticalCenter: parent.verticalCenter
                                visible: chip.chosen
                                name: "check"
                                size: 14
                                color: Theme.background
                            }

                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                text: chip.modelData.split("/").pop().replace(/_/g, " ")
                                color: chip.chosen ? Theme.background : Theme.foreground
                                font.pixelSize: Theme.textCaption
                                font.family: Theme.fontFamily
                                font.weight: Theme.weightTitle
                            }
                        }

                        MouseArea {
                            id: area

                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: chip.open ? Qt.PointingHandCursor : Qt.ArrowCursor
                            onClicked: chip.pick()
                        }

                        Rectangle {
                            visible: chip.activeFocus
                            anchors.fill: parent
                            anchors.margins: -3
                            radius: height / 2
                            color: "transparent"
                            border.width: 2
                            border.color: Theme.accent
                        }
                    }
                }
            }
        }
    }
}
