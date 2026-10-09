import QtQuick
import qs.island

// What every look of the widget shows without a forecast: why, in a line or
// three, and the button that fixes it, which sets the place, changes one
// Open-Meteo doesn't know, or tries again after a failure.
Column {
    id: root

    property var payload: null
    // The height it has, so a long error takes only the lines that fit
    // over the button.
    property real room: 0
    readonly property bool configured: payload?.configured ?? false
    readonly property string fix: {
        if (!configured)
            return "set";
        if (payload?.loading || payload?.error == null)
            return "";
        return payload?.next == null ? "change" : "retry";
    }

    spacing: Theme.spaceSmall

    FontMetrics {
        id: metrics

        font.pixelSize: Theme.textBody
        font.family: Theme.fontFamily
    }

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
        maximumLineCount: {
            if (root.room <= 0)
                return 3;
            const left = root.room - (action.visible ? action.height + root.spacing : 0);
            return Math.max(1, Math.min(3, Math.floor(left / metrics.lineSpacing)));
        }
        wrapMode: Text.Wrap
        color: Theme.muted
        font.pixelSize: Theme.textBody
        font.family: Theme.fontFamily
    }

    Button {
        id: action

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
