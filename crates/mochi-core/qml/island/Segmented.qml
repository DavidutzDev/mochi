import QtQuick

// One choice out of a few, side by side; the current one is a white pill
// that slides to the next. `options` is [{value, label, icon}]. With
// `keyboard` on, Tab reaches it and Left and Right pick the choice beside.
Rectangle {
    id: root

    property var options: []
    property string current: ""
    property bool keyboard: false
    signal picked(string value)

    readonly property int index: options.findIndex(option => option.value === current)
    readonly property real segment: (width - 8) / Math.max(options.length, 1)

    implicitWidth: 360
    implicitHeight: 44
    radius: height / 2
    color: Theme.surface
    activeFocusOnTab: keyboard && enabled && visible
    Keys.onLeftPressed: event => step(-1, event)
    Keys.onRightPressed: event => step(1, event)

    function step(by: int, event: var): void {
        const next = index + by;
        if (!keyboard || next < 0 || next >= options.length)
            return;
        picked(options[next].value);
        event.accepted = true;
    }

    Rectangle {
        visible: root.activeFocus
        anchors.fill: parent
        anchors.margins: -3
        radius: height / 2
        color: "transparent"
        border.width: 2
        border.color: Theme.accent
    }

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
                    spacing: Theme.spaceSmall

                    Symbol {
                        anchors.verticalCenter: parent.verticalCenter
                        visible: (choice.modelData.icon ?? "") !== ""
                        name: choice.modelData.icon ?? ""
                        size: 17
                        color: choice.selected ? Theme.background : Theme.muted
                    }

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        visible: text !== ""
                        text: choice.modelData.label ?? ""
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
