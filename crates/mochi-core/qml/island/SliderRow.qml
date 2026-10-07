import QtQuick

// A level on a tile, like the volume in a control center: an icon that can
// be clicked (to mute, say), a thin slider, the value as a rolling
// percentage, and a chevron that opens more when `opens` is set.
Rectangle {
    id: root

    property string icon: ""
    // From 0 to 1, shown as a percentage of `maximum`.
    property real value: 0
    property real maximum: 100
    property real reset: -1
    property real level: -1
    property bool opens: false
    signal moved(real value)
    signal released(real value)
    signal iconClicked
    signal opened

    implicitWidth: 280
    implicitHeight: Theme.rowHeight
    radius: Theme.radiusField
    color: Theme.surface

    Row {
        anchors.fill: parent
        anchors.leftMargin: Theme.spaceSmall
        anchors.rightMargin: Theme.spaceSmall
        spacing: Theme.spaceSmall

        IconButton {
            id: button

            anchors.verticalCenter: parent.verticalCenter
            icon: root.icon
            onClicked: root.iconClicked()
        }

        Slider {
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - button.width - percent.width - (chevron.visible ? chevron.width + Theme.spaceSmall : 0) - Theme.spaceSmall * 2
            thickness: 6
            value: root.value
            reset: root.reset
            level: root.level
            onMoved: value => root.moved(value)
            onReleased: value => root.released(value)
        }

        RollingText {
            id: percent

            anchors.verticalCenter: parent.verticalCenter
            width: Math.max(implicitWidth, zeroes.advanceWidth)
            text: `${Math.round(root.value * root.maximum)}%`
            color: Theme.muted
            pixelSize: Theme.textCaption
            weight: Theme.weightLabel

            TextMetrics {
                id: zeroes

                font: percent.font
                text: "100%"
            }
        }

        IconButton {
            id: chevron

            anchors.verticalCenter: parent.verticalCenter
            visible: root.opens
            icon: "chevron"
            size: 14
            onClicked: root.opened()
        }
    }
}
