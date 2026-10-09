import QtQuick
import qs.island

// The control center's home card and a desktop widget: the battery's level,
// how long until empty or full, and the power module's profiles at the end
// when it has some. Under it, a line for each battery of a laptop with two,
// and for each mouse, keyboard, controller or other device that reports its
// battery. Without any of them, it steps aside.
Item {
    id: root

    property var payload: null
    // What the desktop gives a widget; this one has no settings.
    property var settings: ({})
    property string instance: ""
    readonly property bool present: payload?.present ?? false
    // The laptop's batteries, when there are two or more, then the devices.
    readonly property var lines: {
        const batteries = payload?.batteries ?? [];
        const each = batteries.length > 1 ? batteries.map(battery => ({
                    "id": battery.name,
                    "name": battery.name,
                    "icon": "",
                    "level": battery.level,
                    "charging": battery.charging,
                    "low": false
                })) : [];
        return each.concat(payload?.devices ?? []);
    }
    // A desktop without peripherals: nothing worth a card.
    readonly property bool hidden: !present && lines.length === 0
    readonly property color tint: payload?.critical ? Theme.danger : payload?.low ? Theme.accent : Theme.foreground
    readonly property var power: Daemon.state("power")
    readonly property var profiles: power?.profiles ?? []
    readonly property var profileIcons: ({
            "power-saver": "eco",
            "balanced": "balance",
            "performance": "speed"
        })
    readonly property int lineHeight: Theme.textBody + Theme.spaceSmall
    readonly property int lineSpacing: Theme.spaceTiny
    // A card's two rows at most, less its padding and its heading, as the
    // control center lays them out; a widget's size.
    readonly property real tallest: instance === "" ? Theme.tileHeight * 2 + Theme.spaceMedium - Theme.spaceMedium * 2 - (Theme.textCaption + Theme.spaceSmall * 2) : height
    // As many lines as there's room for. The last says how many more there
    // are.
    readonly property int room: {
        const left = tallest - (present ? Theme.rowHeight + lineSpacing : 0);
        return Math.max(0, Math.floor((left + lineSpacing) / (lineHeight + lineSpacing)));
    }
    // Without room for all, the devices go before the laptop's own
    // batteries, which the level above sums up, and low ones first.
    readonly property var shown: {
        if (lines.length <= room)
            return lines;
        const devices = payload?.devices ?? [];
        if (devices.length <= room)
            return devices;
        const ranked = devices.filter(device => device.low).concat(devices.filter(device => !device.low));
        const kept = ranked.slice(0, Math.max(0, room - 1));
        return room > 0 ? kept.concat([
            {
                "id": "more",
                "name": `${devices.length - kept.length} more`,
                "more": true
            }
        ]) : kept;
    }

    implicitWidth: 240
    implicitHeight: column.implicitHeight

    Column {
        id: column

        width: parent.width
        // Centered in a widget taller than it needs.
        y: Math.max(0, (root.height - implicitHeight) / 2)
        spacing: root.lineSpacing

        Item {
            width: parent.width
            height: Theme.rowHeight
            visible: root.present

            Gauge {
                id: gauge

                anchors.verticalCenter: parent.verticalCenter
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
                visible: root.profiles.length > 0
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

        Repeater {
            model: root.shown

            // A battery or a device: its symbol, its name, and its level,
            // red when low.
            Item {
                id: line

                required property var modelData
                readonly property color tint: modelData.low ? Theme.danger : Theme.muted

                width: column.width
                height: root.lineHeight

                Item {
                    id: mark

                    width: Theme.textBody * 1.5
                    height: parent.height

                    Gauge {
                        anchors.centerIn: parent
                        visible: !line.modelData.more && line.modelData.icon === ""
                        level: line.modelData.level ?? 0
                        charging: line.modelData.charging ?? false
                        color: line.tint
                        size: Theme.textBody * 0.9
                    }

                    Symbol {
                        anchors.centerIn: parent
                        visible: (line.modelData.icon ?? "") !== ""
                        name: line.modelData.icon ?? ""
                        size: Theme.textBody + 2
                        color: line.tint
                    }
                }

                Text {
                    anchors.left: mark.right
                    anchors.leftMargin: Theme.spaceSmall
                    anchors.right: level.left
                    anchors.rightMargin: Theme.spaceSmall
                    anchors.verticalCenter: parent.verticalCenter
                    text: line.modelData.name
                    elide: Text.ElideRight
                    color: line.modelData.more ? Theme.muted : Theme.foreground
                    font.pixelSize: Theme.textCaption
                    font.family: Theme.fontFamily
                }

                Row {
                    id: level

                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    visible: !line.modelData.more
                    spacing: 2

                    Symbol {
                        anchors.verticalCenter: parent.verticalCenter
                        visible: line.modelData.charging ?? false
                        name: "bolt"
                        size: Theme.textCaption
                        color: Theme.accent
                    }

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: `${line.modelData.level ?? 0}%`
                        color: line.modelData.low ? Theme.danger : Theme.muted
                        font.pixelSize: Theme.textCaption
                        font.family: Theme.fontFamily
                        font.weight: Theme.weightLabel
                    }
                }
            }
        }
    }
}
