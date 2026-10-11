import QtQuick
import qs.island

// The lock screen's password field, a widget only the lock screen has. It
// never holds the password: `payload` is the lock's state from mochi-lock,
// how many characters are typed and what went wrong, and while arranging,
// the module's state, which has none of that, so it shows its hint.
Item {
    id: root

    property var payload: null
    property var settings: ({})

    readonly property int typed: payload?.typed ?? 0
    readonly property string phase: payload?.phase ?? "typing"
    readonly property var message: payload?.message ?? null

    implicitWidth: 360
    implicitHeight: column.implicitHeight

    Column {
        id: column

        anchors.verticalCenter: parent.verticalCenter
        width: parent.width
        spacing: Theme.spaceSmall

        Text {
            text: root.payload?.user ? `Locked · ${root.payload.user}` : "Locked"
            color: Theme.muted
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
            font.weight: Theme.weightLabel
        }

        Rectangle {
            readonly property bool failed: root.message !== null && root.typed === 0

            width: parent.width
            height: Theme.controlHeight + Theme.spaceLarge + Theme.spaceTiny
            radius: height / 2
            color: Theme.raised
            border.width: failed ? 1 : 0
            border.color: Theme.danger

            Symbol {
                x: Theme.spaceLarge
                anchors.verticalCenter: parent.verticalCenter
                name: "lock"
                size: Theme.textTitle
                color: Theme.muted
            }

            Row {
                anchors.centerIn: parent
                spacing: Theme.spaceSmall
                visible: root.typed > 0 && root.phase !== "checking"

                Repeater {
                    model: Math.min(root.typed, 18)

                    Rectangle {
                        width: 9
                        height: 9
                        radius: width / 2
                        color: Theme.foreground
                    }
                }
            }

            Text {
                anchors.centerIn: parent
                visible: root.typed === 0 || root.phase === "checking"
                text: {
                    switch (root.phase) {
                    case "checking":
                        return "Checking…";
                    case "waiting":
                        return `Try again in ${root.payload?.wait ?? 0} s`;
                    default:
                        return "Type your password";
                    }
                }
                color: Theme.muted
                font.pixelSize: Theme.textBody
                font.family: Theme.fontFamily
            }
        }

        Text {
            width: parent.width
            visible: text !== ""
            wrapMode: Text.Wrap
            text: root.message ?? ""
            color: Theme.danger
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }

        Text {
            visible: (root.payload?.capsLock ?? false) && root.message === null
            text: "Caps Lock is on"
            color: Theme.accent
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
            font.weight: Theme.weightLabel
        }
    }
}
