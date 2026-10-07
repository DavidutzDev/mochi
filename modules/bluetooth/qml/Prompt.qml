import QtQuick
import qs.island

// A question pairing asks: does the device show this code, may it pair,
// what code it expects, or a code to type on it. Escape says no.
Item {
    id: root

    property var payload: ({})
    readonly property string kind: payload.kind ?? "confirm"
    readonly property string device: payload.device ?? "A device"

    implicitWidth: 400
    implicitHeight: column.implicitHeight + Theme.padding * 2

    focus: true
    Keys.onEscapePressed: Daemon.command("bluetooth", "answer", ["no"])
    Keys.onReturnPressed: root.accept()
    Keys.onEnterPressed: root.accept()

    Component.onCompleted: {
        if (kind === "pin")
            Qt.callLater(() => input.forceActiveFocus());
    }

    function accept(): void {
        if (kind === "pin") {
            if (input.text.length > 0)
                Daemon.command("bluetooth", "pin", [input.text]);
        } else if (kind === "show") {
            Daemon.event("dismiss");
        } else {
            Daemon.command("bluetooth", "answer", ["yes"]);
        }
    }

    // Pairing waits on the answer.
    EdgeLight {
        radius: Theme.radiusSurface
        working: true
    }

    Column {
        id: column

        x: Theme.padding
        y: Theme.padding
        width: root.width - Theme.padding * 2
        spacing: Theme.spaceMedium

        PanelHeader {
            width: parent.width
            title: `Pair ${root.device}`
        }

        Text {
            width: parent.width
            wrapMode: Text.WordWrap
            text: {
                switch (root.kind) {
                case "confirm":
                    return "Does the device show this code?";
                case "allow":
                    return "It wants to pair without a code. Allow it?";
                case "pin":
                    return "Type the code the device shows, or the one it expects, like 0000.";
                default:
                    return "Type this code on the device, then press Enter on it.";
                }
            }
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        Text {
            visible: root.kind === "confirm" || root.kind === "show"
            anchors.horizontalCenter: parent.horizontalCenter
            text: root.payload.code ?? ""
            color: Theme.foreground
            font.pixelSize: Theme.textDisplay
            font.family: Theme.fontFamily
            font.weight: Theme.weightTitle
            font.letterSpacing: 6
            font.features: {
                "tnum": 1
            }
        }

        Rectangle {
            visible: root.kind === "pin"
            width: parent.width
            height: 40
            radius: height / 2
            color: Theme.surface

            TextInput {
                id: input

                anchors.left: parent.left
                anchors.leftMargin: Theme.spaceLarge
                anchors.right: parent.right
                anchors.rightMargin: Theme.spaceLarge
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.foreground
                selectionColor: Theme.accent
                font.pixelSize: Theme.textBody
                font.family: Theme.fontFamily
                clip: true

                Keys.onReturnPressed: root.accept()
                Keys.onEnterPressed: root.accept()
                Keys.onEscapePressed: Daemon.command("bluetooth", "answer", ["no"])
            }
        }

        Row {
            anchors.right: parent.right
            spacing: Theme.spaceSmall

            Button {
                visible: root.kind !== "show"
                text: root.kind === "pin" ? "Cancel" : "No"
                tone: "ghost"
                onClicked: Daemon.command("bluetooth", "answer", ["no"])
            }

            Button {
                text: root.kind === "show" ? "Done" : root.kind === "pin" ? "Pair" : "Yes"
                tone: "accent"
                onClicked: root.accept()
            }
        }
    }
}
