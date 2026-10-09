import QtQuick
import qs.island

// The clock panel: one tab at a time over a navbar, like the control
// center's. Every tab takes the same room, so the island keeps its size from
// tab to tab. The tabs read the clock module's state, and Today the weather
// module's; the tour passes made-up ones as `payload.state` and
// `payload.weather`.
//
// The keyboard: Tab goes through the controls and the navbar, 1 to 5 pick a
// tab, Left and Right on the navbar move along it, and Escape closes the
// panel.
Item {
    id: root

    property var payload: ({})
    readonly property var clock: payload.state ?? Daemon.state("clock") ?? ({})
    readonly property var weather: payload.weather ?? Daemon.state("weather")
    readonly property bool weatherOn: payload.weather !== undefined || Daemon.modules.includes("weather")
    property string tab: payload.tab ?? "today"
    // `mochi ipc clock open <tab>` while it's open switches tabs, after a
    // click on one replaced the binding above.
    onPayloadChanged: tab = payload.tab ?? "today"
    // Today's "Add a reminder" opens the calendar on today, ready to type.
    property bool adding: false

    readonly property var tabs: [
        {
            "id": "today",
            "title": "Today",
            "icon": "light_mode"
        },
        {
            "id": "calendar",
            "title": "Calendar",
            "icon": "calendar_month"
        },
        {
            "id": "timer",
            "title": "Timer",
            "icon": "hourglass_top"
        },
        {
            "id": "stopwatch",
            "title": "Stopwatch",
            "icon": "timer"
        },
        {
            "id": "world",
            "title": "World",
            "icon": "public"
        }
    ]

    readonly property int margin: Theme.spaceLarge
    // The room every tab gets: the calendar's six weeks and the day beside
    // them, the tallest of the five.
    readonly property int room: 340

    implicitWidth: 720
    implicitHeight: margin + room + Theme.spaceMedium + 1 + navbar.height

    function pick(id: string): void {
        if (id !== "calendar")
            adding = false;
        tab = id;
    }

    focus: true
    Keys.onEscapePressed: Daemon.event("dismiss")
    Keys.onPressed: event => {
        const index = event.key - Qt.Key_1;
        if (event.modifiers === Qt.NoModifier && index >= 0 && index < tabs.length) {
            pick(tabs[index].id);
            event.accepted = true;
        }
    }
    Component.onCompleted: Qt.callLater(() => root.forceActiveFocus())

    Loader {
        id: page

        x: root.margin
        y: root.margin
        width: parent.width - root.margin * 2
        height: root.room
        sourceComponent: ({
                "today": today,
                "calendar": calendar,
                "timer": timer,
                "stopwatch": stopwatch,
                "world": world
            })[root.tab] ?? today
    }

    Component {
        id: today

        Today {
            clock: root.clock
            weather: root.weather
            weatherOn: root.weatherOn
            onAddReminder: {
                root.adding = true;
                root.pick("calendar");
            }
        }
    }

    Component {
        id: calendar

        Calendar {
            clock: root.clock
            adding: root.adding
        }
    }

    Component {
        id: timer

        Focus {}
    }

    Component {
        id: stopwatch

        Stopwatch {
            clock: root.clock
        }
    }

    Component {
        id: world

        World {
            clock: root.clock
        }
    }

    Rectangle {
        id: divider

        x: root.margin
        anchors.top: page.bottom
        anchors.topMargin: Theme.spaceMedium
        width: parent.width - root.margin * 2
        height: 1
        color: Theme.raised
    }

    // Like the control center's: icons in circles, the current tab
    // stretched into a pill with its name. Each one takes the keyboard too.
    Item {
        id: navbar

        anchors.top: divider.bottom
        width: parent.width
        height: 52

        Row {
            id: tabRow

            anchors.centerIn: parent
            spacing: Theme.spaceSmall

            Repeater {
                id: tabItems

                model: root.tabs

                Rectangle {
                    id: tabItem

                    required property var modelData
                    required property int index
                    readonly property bool selected: root.tab === modelData.id

                    width: selected ? content.implicitWidth + 28 : height
                    height: 34
                    radius: height / 2
                    color: selected ? Theme.foreground : area.containsMouse ? Theme.raised : "transparent"
                    activeFocusOnTab: true
                    Keys.onReturnPressed: root.pick(modelData.id)
                    Keys.onEnterPressed: root.pick(modelData.id)
                    Keys.onSpacePressed: root.pick(modelData.id)
                    Keys.onLeftPressed: tabItems.itemAt((index + root.tabs.length - 1) % root.tabs.length).forceActiveFocus()
                    Keys.onRightPressed: tabItems.itemAt((index + 1) % root.tabs.length).forceActiveFocus()

                    Behavior on width {
                        NumberAnimation {
                            duration: Theme.move
                            easing.type: Easing.BezierSpline
                            easing.bezierCurve: Theme.overshoot
                        }
                    }

                    Behavior on color {
                        ColorAnimation {
                            duration: Theme.fast
                        }
                    }

                    Row {
                        id: content

                        anchors.centerIn: parent
                        spacing: Theme.spaceSmall

                        Symbol {
                            anchors.verticalCenter: parent.verticalCenter
                            name: tabItem.modelData.icon
                            size: 18
                            color: tabItem.selected ? Theme.background : Theme.muted
                        }

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            visible: tabItem.selected
                            text: tabItem.modelData.title
                            color: Theme.background
                            font.pixelSize: Theme.textBody
                            font.family: Theme.fontFamily
                            font.weight: Theme.weightTitle
                        }
                    }

                    Rectangle {
                        visible: tabItem.activeFocus
                        anchors.fill: parent
                        anchors.margins: -3
                        radius: height / 2
                        color: "transparent"
                        border.width: 2
                        border.color: Theme.accent
                    }

                    MouseArea {
                        id: area

                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.pick(tabItem.modelData.id)
                    }
                }
            }
        }

        // The clock's settings.
        ActionButton {
            anchors.right: parent.right
            anchors.rightMargin: root.margin
            anchors.verticalCenter: parent.verticalCenter
            visible: Daemon.modules.includes("settings")
            icon: "settings"
            tone: "ghost"
            onClicked: Daemon.command("settings", "open", ["clock"])
        }
    }
}
