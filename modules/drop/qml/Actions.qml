import QtQuick
import qs.island

// What was dropped and what to do with it: a button for each action that
// fits. While one runs, its button says so; after, the result shows, with
// a way to its folder. Escape closes it.
Item {
    id: root

    property var payload: ({})
    readonly property var actions: payload.actions ?? []
    readonly property bool busy: payload.running != null

    implicitWidth: Math.max(360, buttons.implicitWidth + Theme.padding * 2)
    implicitHeight: column.implicitHeight + Theme.padding * 2

    focus: true
    Keys.onEscapePressed: Daemon.command("drop", "close", [])

    Column {
        id: column

        x: Theme.padding
        y: Theme.padding
        width: parent.width - Theme.padding * 2
        spacing: Theme.spaceMedium

        Row {
            spacing: Theme.spaceSmall

            Symbol {
                anchors.verticalCenter: parent.verticalCenter
                name: "place_item"
                color: Theme.accent
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                width: column.width - Theme.controlHeight
                text: root.payload.summary ?? ""
                elide: Text.ElideMiddle
                color: Theme.foreground
                font.pixelSize: Theme.textTitle
                font.family: Theme.fontFamily
                font.weight: Theme.weightTitle
            }
        }

        Text {
            visible: root.actions.length === 0
            text: "Nothing to do with these: install zip, ImageMagick or poppler for more"
            color: Theme.muted
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }

        Flow {
            id: buttons

            width: column.width
            spacing: Theme.spaceSmall

            Repeater {
                model: root.actions

                Button {
                    required property var modelData
                    readonly property bool running: root.payload.running === modelData.id

                    icon: running ? "hourglass_top" : modelData.icon
                    text: running ? "Working…" : modelData.label
                    enabled: !root.busy
                    onClicked: Daemon.command("drop", "run", [modelData.id])
                }
            }
        }

        Row {
            visible: (root.payload.message ?? null) !== null
            spacing: Theme.spaceSmall

            Symbol {
                anchors.verticalCenter: parent.verticalCenter
                name: root.payload.failed ? "error" : "check_circle"
                size: Theme.textBody
                color: root.payload.failed ? Theme.danger : Theme.success
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                width: Math.min(implicitWidth, column.width - Theme.controlHeight * 3)
                text: root.payload.message ?? ""
                elide: Text.ElideRight
                color: root.payload.failed ? Theme.danger : Theme.foreground
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }

            Button {
                anchors.verticalCenter: parent.verticalCenter
                visible: root.payload.result ?? false
                tone: "ghost"
                text: "Show"
                onClicked: Daemon.command("drop", "show", [])
            }
        }
    }
}
