import QtQuick
import qs.island

// The control center's card: the sky now with its icon, the temperature and
// the place, and the next hours in a strip on the right, as many as fit.
// Until a place is set, it says so and opens the setting.
Item {
    id: root

    property var payload: null
    readonly property bool configured: payload?.configured ?? false
    // Hidden until a place is set, when `prompt` is off.
    readonly property bool hidden: !configured && !(payload?.prompt ?? true)
    readonly property var current: payload?.current ?? null
    readonly property var hours: payload?.hourly ?? []
    readonly property string unit: payload?.unit?.temperature ?? "°"
    // What the button under a missing forecast does: set the place, change
    // one Open-Meteo doesn't know, or try again after a failure.
    readonly property string fix: {
        if (!configured)
            return "set";
        if (payload?.loading || payload?.error == null)
            return "";
        return payload?.next == null ? "change" : "retry";
    }
    // Room for the hours beside the weather now, each this wide.
    readonly property int hourWidth: 44
    readonly property int shownHours: Math.max(0, Math.min(hours.length, 8, Math.floor((width - now.width - Theme.spaceLarge) / hourWidth)))

    function degrees(value: real): string {
        return `${Math.round(value)}°`;
    }

    implicitHeight: Theme.rowHeight

    // No place yet, or no forecast yet: one line, and what to do about it.
    Item {
        anchors.fill: parent
        visible: root.current === null

        Symbol {
            id: waiting

            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            name: root.configured ? "cloud" : "location_on"
            size: 22
            color: Theme.muted
        }

        Text {
            anchors.left: waiting.right
            anchors.leftMargin: Theme.spaceMedium
            anchors.right: action.visible ? action.left : parent.right
            anchors.rightMargin: Theme.spaceSmall
            anchors.verticalCenter: parent.verticalCenter
            text: {
                if (!root.configured)
                    return "Set a place to see its weather here.";
                if (root.payload?.loading)
                    return "Fetching the weather…";
                return root.payload?.error ?? "No forecast yet.";
            }
            elide: Text.ElideRight
            maximumLineCount: 2
            wrapMode: Text.Wrap
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        Button {
            id: action

            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            visible: root.fix !== ""
            text: ({
                    "set": "Set a place",
                    "change": "Change the place",
                    "retry": "Try again"
                })[root.fix] ?? ""
            icon: root.fix === "retry" ? "refresh" : "edit"
            onClicked: {
                if (root.fix === "retry")
                    Daemon.command("weather", "refresh", []);
                else
                    Daemon.command("settings", "open", ["config.module.weather.place"]);
            }
        }
    }

    // The weather now.
    Row {
        id: now

        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        visible: root.current !== null
        spacing: Theme.spaceMedium

        Symbol {
            anchors.verticalCenter: parent.verticalCenter
            name: root.current?.icon ?? "cloud"
            size: 34
            color: Theme.accent
        }

        Column {
            anchors.verticalCenter: parent.verticalCenter
            width: Math.max(120, Math.min(200, root.width * 0.32))

            RollingText {
                text: `${Math.round(root.current?.temperature ?? 0)}${root.unit}`
                pixelSize: Theme.textHeadline
                family: Theme.displayFamily
                weight: Theme.weightTitle
            }

            Text {
                width: parent.width
                text: [root.current?.text, root.payload?.place?.name].filter(part => part).join(" · ")
                elide: Text.ElideRight
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }
        }
    }

    // The next hours, from the one that runs.
    Row {
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        visible: root.current !== null

        Repeater {
            model: root.hours.slice(0, root.shownHours)

            Column {
                required property var modelData
                required property int index

                width: root.hourWidth
                spacing: 1

                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: parent.index === 0 ? "Now" : `${String(parent.modelData.hour).padStart(2, "0")}:00`
                    color: Theme.muted
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                }

                Symbol {
                    anchors.horizontalCenter: parent.horizontalCenter
                    name: parent.modelData.icon
                    size: 16
                    color: Theme.foreground
                }

                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: root.degrees(parent.modelData.temperature)
                    color: Theme.foreground
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                    font.weight: Theme.weightLabel
                }
            }
        }
    }
}
