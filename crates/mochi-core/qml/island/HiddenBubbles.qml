import QtQuick

// The bubbles an area leaves out, which the daemon shows when the user
// clicks its "+N". Each row draws the bubble's own view, the wide one when
// the module has one, and a click on it does what a click on the bubble
// does. Escape or a click outside closes it, as with any panel.
Item {
    id: root

    // {area, bubbles: [{id, module, view, wide, payload}]}
    property var payload
    readonly property var bubbles: payload?.bubbles ?? []

    implicitWidth: Math.max(280, list.implicitWidth) + Theme.padding * 2
    implicitHeight: list.implicitHeight + Theme.padding * 2

    Column {
        id: list

        x: Theme.padding
        y: Theme.padding
        width: root.width - Theme.padding * 2
        spacing: 4

        SectionLabel {
            text: root.bubbles.length === 1 ? "1 more bubble" : `${root.bubbles.length} more bubbles`
            bottomPadding: 4
        }

        Repeater {
            model: root.bubbles

            Rectangle {
                id: row

                required property var modelData

                width: list.width
                implicitWidth: view.width + label.implicitWidth + 3 * 12
                height: Math.max(Theme.idleHeight, view.height) + 8
                radius: Theme.radiusMedium
                color: area.containsMouse ? Theme.raised : Theme.surface

                Behavior on color {
                    ColorAnimation {
                        duration: Theme.fast
                    }
                }

                // Under the view, so buttons in it get their own clicks.
                MouseArea {
                    id: area

                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: Daemon.bubbleClick(row.modelData.id)
                }

                Loader {
                    id: view

                    readonly property string url: `root:/modules/${row.modelData.module}/${row.modelData.view}.qml`

                    x: 12
                    anchors.verticalCenter: parent.verticalCenter
                    width: item ? item.implicitWidth : 0
                    height: item ? item.implicitHeight : 0
                    Component.onCompleted: {
                        setSource(url, {
                            payload: row.modelData.payload
                        });
                        if (status !== Loader.Ready)
                            console.warn(`mochi: could not load ${url}`);
                    }
                }

                Binding {
                    target: view.item
                    property: "payload"
                    value: row.modelData.payload
                    when: view.item !== null
                }

                Text {
                    id: label

                    anchors.right: parent.right
                    anchors.rightMargin: 12
                    anchors.verticalCenter: parent.verticalCenter
                    text: row.modelData.module
                    color: Theme.muted
                    font.pixelSize: Theme.textLabel
                    font.family: Theme.fontFamily
                }
            }
        }
    }
}
