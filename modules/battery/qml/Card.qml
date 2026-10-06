import QtQuick
import qs.island

// The hub's home card: the battery's level, and how long until empty or
// full, and the power module's profiles under them when it has some.
// Without a battery, it says so.
Item {
    id: root

    property var payload: null
    readonly property bool present: payload?.present ?? false
    // A desktop: nothing worth a card.
    readonly property bool hidden: !present
    readonly property color tint: payload?.critical ? Theme.danger : payload?.low ? Theme.accent : Theme.foreground
    readonly property var power: Daemon.state("power")
    readonly property var profiles: power?.profiles ?? []
    readonly property var profileIcons: ({
            "power-saver": "leaf",
            "balanced": "scale",
            "performance": "bolt"
        })

    implicitHeight: profiles.length > 0 ? 64 + 10 + switcher.height : 64

    Text {
        anchors.centerIn: parent
        visible: !root.present
        text: "No battery"
        color: Theme.muted
        font.pixelSize: Theme.textBody
        font.family: Theme.fontFamily
    }

    Row {
        y: (64 - height) / 2
        visible: root.present
        spacing: 14

        Gauge {
            anchors.verticalCenter: parent.verticalCenter
            level: root.payload?.level ?? 0
            charging: root.payload?.charging ?? false
            color: root.tint
            size: 26
        }

        Column {
            anchors.verticalCenter: parent.verticalCenter
            spacing: 2

            Text {
                text: `${root.payload?.level ?? 0}%`
                color: root.tint
                font.pixelSize: Theme.textTitle
                font.family: Theme.fontFamily
                font.weight: Font.DemiBold
                font.features: { "tnum": 1 }
            }

            Text {
                text: root.payload?.state ?? ""
                color: Theme.muted
                font.pixelSize: Theme.textLabel
                font.family: Theme.fontFamily
            }
        }
    }

    Segmented {
        id: switcher

        y: 64 + 10
        width: parent.width
        height: 36
        visible: root.present && root.profiles.length > 0
        color: Theme.raised
        options: root.profiles.map(name => ({
                    "value": name,
                    "label": "",
                    "icon": root.profileIcons[name] ?? "dot"
                }))
        current: root.power?.profile ?? ""
        onPicked: value => Daemon.command("power", "profile", [value])
    }
}
