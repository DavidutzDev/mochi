import QtQuick

// One choice out of a few, side by side; the current one is a white pill
// that slides to the next. `options` is [{value, label, icon}].
Rectangle {
    id: root

    property var options: []
    property string current: ""
    signal picked(string value)

    readonly property int index: options.findIndex(option => option.value === current)
    readonly property real segment: (width - 8) / Math.max(options.length, 1)

    implicitWidth: 360
    implicitHeight: 44
    radius: height / 2
    color: Theme.surface

    Rectangle {
        visible: root.index >= 0
        x: 4 + root.segment * root.index
        y: 4
        width: root.segment
        height: root.height - 8
        radius: height / 2
        color: Theme.foreground

        Behavior on x {
            NumberAnimation {
                duration: Theme.move
                easing.type: Easing.BezierSpline
                easing.bezierCurve: Theme.overshoot
            }
        }
    }

    Row {
        x: 4
        y: 4

        Repeater {
            model: root.options

            Item {
                id: choice

                required property var modelData
                required property int index
                readonly property bool selected: index === root.index

                width: root.segment
                height: root.height - 8

                Row {
                    anchors.centerIn: parent
                    spacing: 8

                    Symbol {
                        anchors.verticalCenter: parent.verticalCenter
                        visible: (choice.modelData.icon ?? "") !== ""
                        name: choice.modelData.icon ?? ""
                        size: 17
                        color: choice.selected ? Theme.background : Theme.muted
                    }

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: choice.modelData.label
                        color: choice.selected ? Theme.background : Theme.foreground
                        font.pixelSize: Theme.textBody
                        font.family: Theme.fontFamily
                        font.weight: Font.DemiBold
                    }
                }

                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: root.picked(choice.modelData.value)
                }
            }
        }
    }
}
