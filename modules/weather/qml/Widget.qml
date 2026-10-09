import QtQuick
import qs.island

// The desktop widget: the sky now as a large icon, the temperature beside
// it, and the sky in words with the place and today's low and high under
// it. Everything grows with the widget. Until a place is set, it says so.
Item {
    id: root

    property var payload: null
    // What the desktop gives a widget; this one has no settings.
    property var settings: ({})
    property string instance: ""
    readonly property bool configured: payload?.configured ?? false
    readonly property var current: payload?.current ?? null
    readonly property var today: payload?.daily?.[0] ?? null
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
    // The icon and the temperature follow the widget's height, and its
    // width so they fit side by side.
    readonly property real big: Math.max(18, Math.min(height * 0.45, width / 5))

    implicitWidth: 256
    implicitHeight: 96

    // No place yet, or no forecast yet.
    Column {
        anchors.verticalCenter: parent.verticalCenter
        width: parent.width
        visible: root.current === null
        spacing: Theme.spaceSmall

        Text {
            width: parent.width
            text: {
                if (!root.configured)
                    return "Set a place to see its weather here.";
                if (root.payload?.loading)
                    return "Fetching the weather…";
                return root.payload?.error ?? "No forecast yet.";
            }
            elide: Text.ElideRight
            maximumLineCount: 3
            wrapMode: Text.Wrap
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        Button {
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

    Row {
        anchors.verticalCenter: parent.verticalCenter
        width: parent.width
        visible: root.current !== null
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

            // Left out when the widget is too low for three lines.
            Text {
                width: parent.width
                visible: root.height >= 60
                text: {
                    const range = root.today ? `${Math.round(root.today.min)}° to ${Math.round(root.today.max)}°` : "";
                    return [root.payload?.place?.name, range].filter(part => part).join(" · ");
                }
                elide: Text.ElideRight
                color: Theme.muted
                font.pixelSize: Math.max(Theme.textCaption, root.big * 0.3)
                font.family: Theme.fontFamily
            }
        }
    }
}
