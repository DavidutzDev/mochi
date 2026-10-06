import QtQuick
import qs.island

// The hub card: the latest colors as dots and a button to pick one.
// Clicking a dot copies it in the default format; hovering shows its text.
Item {
    id: root

    property var payload: null
    readonly property var colors: (payload?.history ?? []).slice(0, 8)
    property var hovered: null
    property bool copied: false

    implicitHeight: 64

    Timer {
        id: forget

        interval: 1500
        onTriggered: root.copied = false
    }

    Row {
        id: dots

        anchors.left: parent.left
        anchors.top: parent.top
        spacing: 6

        Repeater {
            model: root.colors

            Rectangle {
                id: dot

                required property var modelData

                width: 24
                height: 24
                radius: 12
                color: modelData.swatch
                border.color: area.containsMouse ? Theme.foreground : Theme.border
                border.width: area.containsMouse ? 2 : 1
                scale: area.pressed ? 0.9 : 1

                Behavior on scale {
                    NumberAnimation {
                        duration: Theme.fast
                    }
                }

                MouseArea {
                    id: area

                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onContainsMouseChanged: {
                        if (containsMouse)
                            root.hovered = dot.modelData;
                        else if (root.hovered === dot.modelData)
                            root.hovered = null;
                    }
                    onClicked: {
                        Daemon.command("colors", "copy", [dot.modelData.color]);
                        root.copied = true;
                        forget.restart();
                    }
                }
            }
        }
    }

    Text {
        anchors.left: parent.left
        anchors.right: pick.left
        anchors.rightMargin: 8
        anchors.verticalCenter: pick.verticalCenter
        text: {
            if (root.copied)
                return "Copied";
            if (root.hovered)
                return root.hovered.text;
            const count = root.payload?.history?.length ?? 0;
            return count === 0 ? "No colors yet" : count === 1 ? "1 color" : `${count} colors`;
        }
        elide: Text.ElideRight
        color: root.hovered || root.copied ? Theme.foreground : Theme.muted
        font.pixelSize: Theme.textLabel
        font.family: Theme.fontFamily
    }

    Button {
        id: pick

        anchors.right: parent.right
        anchors.bottom: parent.bottom
        tone: "accent"
        text: "Pick"
        onClicked: Daemon.command("colors", "start", [])
    }
}
