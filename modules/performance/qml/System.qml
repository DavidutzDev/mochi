import QtQuick
import qs.island

// The system info widget: the distribution's name over a line each for
// the kernel, the uptime, Mochi's version, the compositor, the CPU and the
// memory in use, as many as fit. The names in the theme's font rather than
// a terminal's: it reads like the rest of the desktop. The performance
// module reads what the computer is once, and the memory with its other
// readings; the uptime counts on here.
Item {
    id: root

    property var payload: null
    property var settings: ({})
    property string instance: ""

    readonly property var system: payload?.system ?? null
    readonly property var memory: payload?.memory ?? null

    property real now: Date.now()

    Timer {
        interval: 30000
        running: root.system !== null
        repeat: true
        onTriggered: root.now = Date.now()
    }

    // "45 min", "3 h 12 min", "2 d 4 h".
    function uptime(booted: real): string {
        const minutes = Math.max(0, Math.floor((now / 1000 - booted) / 60));
        const hours = Math.floor(minutes / 60);
        const days = Math.floor(hours / 24);
        if (days > 0)
            return hours % 24 > 0 ? `${days} d ${hours % 24} h` : `${days} d`;
        if (hours > 0)
            return minutes % 60 > 0 ? `${hours} h ${minutes % 60} min` : `${hours} h`;
        return `${minutes} min`;
    }

    readonly property var rows: {
        if (system === null)
            return [];
        const cpu = system.cpu ? (system.cores ? `${system.cpu} (${system.cores})` : system.cpu) : null;
        return [["Kernel", system.kernel ? `Linux ${system.kernel}` : null], ["Uptime", system.booted ? uptime(system.booted) : null], ["Mochi", system.mochi], ["Compositor", system.compositor], ["CPU", cpu], ["Memory", memory ? `${memory.used} of ${memory.total}` : null]].filter(row => row[1]);
    }
    readonly property real lineHeight: Theme.textBody * 1.5
    // As many lines as fit under the name.
    readonly property int fit: Math.max(0, Math.floor((height - name.height - Theme.spaceSmall) / lineHeight))

    Text {
        anchors.centerIn: parent
        visible: root.system === null
        text: "Waiting for readings"
        color: Theme.muted
        font.pixelSize: Theme.textBody
        font.family: Theme.fontFamily
    }

    Text {
        id: name

        width: parent.width
        visible: root.system !== null
        elide: Text.ElideRight
        text: root.system?.os ?? "Linux"
        color: Theme.foreground
        font.pixelSize: Theme.textHeadline
        font.family: Theme.displayFamily
        font.weight: Theme.weightTitle
    }

    // The names' column, as wide as the widest.
    TextMetrics {
        id: widest

        font.family: Theme.fontFamily
        font.weight: Theme.weightLabel
        font.pixelSize: Theme.textBody
        text: "Compositor"
    }

    Column {
        anchors.top: name.bottom
        anchors.topMargin: Theme.spaceSmall
        width: parent.width

        Repeater {
            model: root.rows.slice(0, root.fit)

            Row {
                required property var modelData

                width: parent.width
                height: root.lineHeight
                spacing: Theme.spaceMedium

                Text {
                    width: widest.advanceWidth
                    anchors.verticalCenter: parent.verticalCenter
                    text: parent.modelData[0]
                    color: Theme.muted
                    font: widest.font
                }

                Text {
                    width: parent.width - widest.advanceWidth - parent.spacing
                    anchors.verticalCenter: parent.verticalCenter
                    elide: Text.ElideRight
                    text: parent.modelData[1]
                    color: Theme.foreground
                    font.pixelSize: Theme.textBody
                    font.family: Theme.fontFamily
                }
            }
        }
    }
}
