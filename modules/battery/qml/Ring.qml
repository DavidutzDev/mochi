import QtQuick
import qs.island

// The battery widget's ring look: the level as a ring with the percent in
// the middle, and under it what the battery is doing, fully charged,
// charging or on battery, and how long until full or empty. The ring
// waves while it charges, as power comes in, and lies flat on battery; it
// turns red when the battery is low. Without a battery, it steps aside.
Item {
    id: root

    property var payload: null
    property var settings: ({})
    property string instance: ""
    property string variant: ""

    readonly property bool present: payload?.present ?? false
    readonly property bool hidden: !present
    readonly property int level: payload?.level ?? 0
    readonly property bool charging: payload?.charging ?? false
    readonly property bool low: (payload?.low ?? false) || (payload?.critical ?? false)
    readonly property string doing: {
        if (charging)
            return "Charging";
        if (payload?.plugged ?? false)
            // Held at a charge limit, it's plugged in rather than full.
            return level >= 99 ? "Fully charged" : "Plugged in";
        return "On battery";
    }
    // How long until full or empty, when UPower knows.
    readonly property string time: {
        const time = payload?.time ?? "";
        if (time === "")
            return "";
        return charging ? `Full in ${time}` : `${time} left`;
    }
    readonly property real size: Math.max(0, Math.min(width, height - lines.height - Theme.spaceSmall))

    Column {
        anchors.centerIn: parent
        visible: root.present
        spacing: Theme.spaceSmall

        WavyRing {
            anchors.horizontalCenter: parent.horizontalCenter
            width: root.size
            height: root.size
            size: root.size
            thickness: Math.max(4, root.size * 0.07)
            value: root.level / 100
            wavy: root.charging
            color: root.low ? Theme.danger : Theme.accent

            Row {
                anchors.centerIn: parent
                spacing: 2

                Symbol {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: root.charging
                    name: "bolt"
                    size: percent.pixelSize * 0.8
                    color: Theme.foreground
                }

                RollingText {
                    id: percent

                    text: `${root.level}%`
                    color: root.low ? Theme.danger : Theme.foreground
                    pixelSize: Math.max(Theme.textBody, Math.min(Theme.textDisplay, root.size * 0.22))
                    weight: Theme.weightTitle
                }
            }
        }

        Column {
            id: lines

            anchors.horizontalCenter: parent.horizontalCenter

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                width: Math.min(implicitWidth, root.width)
                elide: Text.ElideRight
                text: root.doing
                color: Theme.foreground
                font.pixelSize: Theme.textBody
                font.family: Theme.fontFamily
                font.weight: Theme.weightLabel
            }

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                width: Math.min(implicitWidth, root.width)
                visible: root.time !== ""
                elide: Text.ElideRight
                text: root.time
                color: Theme.muted
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }
        }
    }
}
