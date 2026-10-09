import QtQuick
import qs.island

// The control center's home card and a desktop widget, in one row: the
// battery's level, how long until empty or full, and the power module's
// profiles at the end when it has some. Without a battery, it steps aside.
Item {
    id: root

    property var payload: null
    // What the desktop gives a widget; this one has no settings.
    property var settings: ({})
    property string instance: ""
    readonly property bool present: payload?.present ?? false
    // A desktop: nothing worth a card.
    readonly property bool hidden: !present
    readonly property color tint: payload?.critical ? Theme.danger : payload?.low ? Theme.accent : Theme.foreground
    readonly property var power: Daemon.state("power")
    readonly property var profiles: power?.profiles ?? []
    readonly property var profileIcons: ({
            "power-saver": "eco",
            "balanced": "balance",
            "performance": "speed"
        })

    implicitWidth: 240
    implicitHeight: Theme.rowHeight

    Gauge {
        id: gauge

        anchors.verticalCenter: parent.verticalCenter
        visible: root.present
        level: root.payload?.level ?? 0
        charging: root.payload?.charging ?? false
        color: root.tint
        size: 26
    }

    Column {
        anchors.left: gauge.right
        anchors.leftMargin: Theme.spaceMedium
        anchors.right: switcher.visible ? switcher.left : parent.right
        anchors.rightMargin: switcher.visible ? Theme.spaceSmall : 0
        anchors.verticalCenter: parent.verticalCenter
        visible: root.present

        RollingText {
            text: `${root.payload?.level ?? 0}%`
            color: root.tint
            pixelSize: Theme.textTitle
            weight: Theme.weightTitle
        }

        Text {
            width: parent.width
            text: root.payload?.state ?? ""
            elide: Text.ElideRight
            color: Theme.muted
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }
    }

    Segmented {
        id: switcher

        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        width: Theme.controlHeight * root.profiles.length
        height: Theme.controlHeight
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
