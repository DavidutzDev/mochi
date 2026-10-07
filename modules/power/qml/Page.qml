import QtQuick
import qs.island

// The hub page: one tile per power action, then the power profiles. Tiles
// that end the session ask first: the first click turns "Shut down" into
// "Shut down?", a second click within a few seconds acts. Acting closes the
// hub.
Item {
    id: root

    property var payload: null
    readonly property var buttons: payload?.buttons ?? []
    readonly property var profiles: payload?.profiles ?? []
    // The action waiting for its second click, or "".
    property string armed: ""

    implicitHeight: column.implicitHeight

    readonly property var profileLooks: ({
            "power-saver": {
                "label": "Power saver",
                "icon": "eco"
            },
            "balanced": {
                "label": "Balanced",
                "icon": "balance"
            },
            "performance": {
                "label": "Performance",
                "icon": "speed"
            }
        })

    Timer {
        id: disarm

        interval: 3000
        onTriggered: root.armed = ""
    }

    function press(button: var): void {
        if (button.confirm && armed !== button.action) {
            armed = button.action;
            disarm.restart();
            return;
        }
        armed = "";
        Daemon.command("power", button.action, []);
        Daemon.event("dismiss");
    }

    Column {
        id: column

        width: parent.width
        spacing: Theme.spaceSmall

        SectionLabel {
            text: "Session"
        }

        Row {
            id: tiles

            width: parent.width
            spacing: Theme.spaceSmall

            Repeater {
                model: root.buttons

                Tile {
                    required property var modelData
                    readonly property bool asking: root.armed === modelData.action

                    width: (tiles.width - tiles.spacing * (root.buttons.length - 1)) / Math.max(root.buttons.length, 1)
                    icon: modelData.icon
                    title: asking ? `${modelData.label}?` : modelData.label
                    checked: asking
                    tone: "danger"
                    onClicked: root.press(modelData)
                }
            }
        }

        Item {
            width: 1
            height: Theme.spaceSmall
        }

        SectionLabel {
            visible: root.profiles.length > 0
            text: "Power profile"
        }

        Segmented {
            visible: root.profiles.length > 0
            width: parent.width
            height: 52
            options: root.profiles.map(name => Object.assign({
                    "value": name
                }, root.profileLooks[name] ?? {
                    "label": name,
                    "icon": ""
                }))
            current: root.payload?.profile ?? ""
            onPicked: value => Daemon.command("power", "profile", [value])
        }
    }
}
