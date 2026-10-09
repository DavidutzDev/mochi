import QtQuick
import qs.island

// The control center's Clock page: the clock panel's Today, Calendar,
// Stopwatch and World tabs, the same views, picked on a row at the top.
// The timer has a card of its own in the control center, so it's left out.
Item {
    id: root

    property var payload: ({})
    readonly property var clock: payload ?? ({})
    readonly property var weather: Daemon.state("weather")
    readonly property bool weatherOn: Daemon.modules.includes("weather")
    property string tab: "today"
    // Today's "Add a reminder" opens the calendar on today, ready to type.
    property bool adding: false

    readonly property var tabs: [
        {
            "value": "today",
            "label": "Today",
            "icon": "light_mode"
        },
        {
            "value": "calendar",
            "label": "Calendar",
            "icon": "calendar_month"
        },
        {
            "value": "stopwatch",
            "label": "Stopwatch",
            "icon": "timer"
        },
        {
            "value": "world",
            "label": "World",
            "icon": "public"
        }
    ]
    // The room the panel gives every tab.
    readonly property int room: 340

    implicitHeight: picker.height + Theme.spaceMedium + room

    function pick(id: string): void {
        if (id !== "calendar")
            adding = false;
        tab = id;
    }

    Segmented {
        id: picker

        anchors.horizontalCenter: parent.horizontalCenter
        width: Math.min(parent.width, 520)
        height: 36
        color: Theme.surface
        options: root.tabs
        current: root.tab
        keyboard: true
        onPicked: value => root.pick(value)
    }

    Loader {
        anchors.top: picker.bottom
        anchors.topMargin: Theme.spaceMedium
        width: parent.width
        height: root.room
        sourceComponent: ({
                "today": today,
                "calendar": calendar,
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
}
