import QtQuick
import qs.island

// The hub page: every color picked, newest first. Each row has the color
// in every format; clicking one copies it, the trash removes the color.
Item {
    id: root

    property var payload: null
    readonly property var colors: payload?.history ?? []
    // "color/format" copied last, for the check mark.
    property string copied: ""

    implicitHeight: toolbar.height + 12 + (colors.length === 0 ? 80 : list.implicitHeight)

    Timer {
        id: forget

        interval: 1500
        onTriggered: root.copied = ""
    }

    Item {
        id: toolbar

        width: parent.width
        height: 32

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.colors.length === 1 ? "1 color" : `${root.colors.length} colors`
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }

        Row {
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: 8

            Button {
                tone: "accent"
                text: "Pick a color"
                onClicked: Daemon.command("colors", "start", [])
            }

            Button {
                visible: root.colors.length > 0
                text: "Clear"
                onClicked: Daemon.command("colors", "clear", [])
            }
        }
    }

    Text {
        anchors.top: toolbar.bottom
        anchors.topMargin: 40
        anchors.horizontalCenter: parent.horizontalCenter
        visible: root.colors.length === 0
        text: "No colors yet"
        color: Theme.muted
        font.pixelSize: Theme.textSubtitle
        font.family: Theme.fontFamily
    }

    Column {
        id: list

        anchors.top: toolbar.bottom
        anchors.topMargin: 12
        width: parent.width
        spacing: 8

        Repeater {
            model: root.colors

            Rectangle {
                id: row

                required property var modelData

                width: list.width
                height: 56
                radius: Theme.radiusMedium
                color: Theme.surface

                Rectangle {
                    id: swatch

                    x: 12
                    anchors.verticalCenter: parent.verticalCenter
                    width: 36
                    height: 36
                    radius: Theme.radiusSmall
                    color: row.modelData.swatch
                    border.color: Theme.border
                    border.width: 1
                }

                Flow {
                    anchors.left: swatch.right
                    anchors.leftMargin: 12
                    anchors.right: remove.left
                    anchors.rightMargin: 8
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 6

                    Repeater {
                        model: row.modelData.formats

                        Button {
                            required property var modelData
                            readonly property string key: `${row.modelData.color}/${modelData.format}`

                            text: modelData.text
                            icon: root.copied === key ? "check" : ""
                            iconSize: 12
                            onClicked: {
                                Daemon.command("colors", "copy", [row.modelData.color, modelData.format]);
                                root.copied = key;
                                forget.restart();
                            }
                        }
                    }
                }

                IconButton {
                    id: remove

                    anchors.right: parent.right
                    anchors.rightMargin: 8
                    anchors.verticalCenter: parent.verticalCenter
                    icon: "trash"
                    size: 14
                    tone: "neutral"
                    onClicked: Daemon.command("colors", "remove", [row.modelData.color])
                }
            }
        }
    }
}
