import QtQuick
import qs.island

// Rows in the shape of options, standing in while a page builds. They
// show only once it takes a moment, so a page that builds at once doesn't
// flash them.
Item {
    id: root

    property bool loading: false
    property bool shown: false

    clip: true
    opacity: 0

    onLoadingChanged: {
        if (loading) {
            wait.restart();
        } else {
            wait.stop();
            shown = false;
        }
    }
    Component.onCompleted: {
        if (loading)
            wait.start();
    }
    onShownChanged: {
        if (shown)
            fade.restart();
        else
            opacity = 0;
    }

    Timer {
        id: wait

        interval: 120
        onTriggered: root.shown = true
    }

    NumberAnimation {
        id: fade

        target: root
        property: "opacity"
        from: 0
        to: 1
        duration: Theme.fast
    }

    Column {
        id: rows

        width: parent.width
        visible: root.shown
        spacing: 2

        SequentialAnimation on opacity {
            running: root.shown && !Theme.reducedMotion
            loops: Animation.Infinite

            NumberAnimation {
                to: 0.5
                duration: Theme.duration(700)
                easing.type: Easing.InOutSine
            }

            NumberAnimation {
                to: 1
                duration: Theme.duration(700)
                easing.type: Easing.InOutSine
            }
        }

        Repeater {
            model: Math.ceil(root.height / (Theme.rowHeight + Theme.spaceSmall))

            Item {
                id: row

                required property int index
                // Titles and descriptions of different lengths, like real
                // options.
                readonly property real title: [0.3, 0.42, 0.24, 0.36, 0.28, 0.4][index % 6]
                readonly property real description: [0.55, 0.4, 0.62, 0.48, 0.58, 0.44][index % 6]

                width: rows.width
                height: Theme.rowHeight + Theme.spaceSmall

                Column {
                    x: Theme.spaceMedium
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: Theme.spaceSmall

                    Rectangle {
                        width: rows.width * row.title
                        height: 10
                        radius: height / 2
                        color: Theme.surface
                    }

                    Rectangle {
                        width: rows.width * row.description
                        height: 8
                        radius: height / 2
                        color: Theme.surface
                    }
                }

                // Where the control would be.
                Rectangle {
                    anchors.right: parent.right
                    anchors.rightMargin: Theme.spaceMedium
                    anchors.verticalCenter: parent.verticalCenter
                    width: 44
                    height: 24
                    radius: height / 2
                    color: Theme.surface
                }
            }
        }
    }
}
