import QtQuick
import qs.island

// The control center's small card, one column wide: the sky's icon, the
// temperature, and the sky in words with the place. Without a forecast, a
// line on why and the button that fixes it, like the wide card.
Item {
    id: root

    property var payload: null
    readonly property bool configured: payload?.configured ?? false
    readonly property var current: payload?.current ?? null
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

    implicitHeight: Theme.rowHeight

    // No place yet, or no forecast yet.
    Item {
        anchors.fill: parent
        visible: root.current === null

        Text {
            anchors.left: parent.left
            anchors.right: action.visible ? action.left : parent.right
            anchors.rightMargin: Theme.spaceSmall
            anchors.verticalCenter: parent.verticalCenter
            text: {
                if (!root.configured)
                    return "No place set yet.";
                if (root.payload?.loading)
                    return "Fetching the weather…";
                return root.payload?.error ?? "No forecast yet.";
            }
            elide: Text.ElideRight
            maximumLineCount: 2
            wrapMode: Text.Wrap
            color: Theme.muted
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }

        Button {
            id: action

            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            visible: root.fix !== ""
            text: ({
                    "set": "Set a place",
                    "change": "Change",
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

    Row {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        visible: root.current !== null
        spacing: Theme.spaceMedium

        Symbol {
            id: sky

            anchors.verticalCenter: parent.verticalCenter
            name: root.current?.icon ?? "cloud"
            size: 34
            color: Theme.accent
        }

        Column {
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - sky.width - parent.spacing

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
}
