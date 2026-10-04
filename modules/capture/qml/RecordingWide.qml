import QtQuick
import qs.island

// The wide recording bubble: the red dot and how long it has recorded.
Row {
    id: root

    property var payload: ({})
    property real now: Date.now()

    readonly property int seconds: Math.max(0, Math.floor((now - (payload.started_ms ?? now)) / 1000))
    readonly property string time: {
        const minutes = Math.floor(seconds / 60);
        const rest = String(seconds % 60).padStart(2, "0");
        return minutes >= 60 ? `${Math.floor(minutes / 60)}:${String(minutes % 60).padStart(2, "0")}:${rest}` : `${minutes}:${rest}`;
    }

    spacing: 8

    Timer {
        interval: 1000
        repeat: true
        running: true
        onTriggered: root.now = Date.now()
    }

    Recording {
        anchors.verticalCenter: parent.verticalCenter
        width: 12
        implicitWidth: 12
    }

    Text {
        anchors.verticalCenter: parent.verticalCenter
        text: root.time
        color: Theme.foreground
        font.pixelSize: Theme.textBody
        font.family: Theme.fontFamily
        font.features: {
            "tnum": 1
        }
    }
}
