import QtQuick

// The top of a page or panel: a back button when `back` is set, the title,
// and anything put inside on the right, like icon buttons.
Item {
    id: root

    property string title: ""
    property bool back: false
    default property alias actions: trailing.data
    signal backClicked

    implicitWidth: lead.implicitWidth + trailing.implicitWidth + Theme.spaceSmall
    implicitHeight: Theme.controlHeight

    Row {
        id: lead

        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.spaceSmall

        IconButton {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.back
            icon: "chevron"
            rotation: 180
            onClicked: root.backClicked()
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.title
            color: Theme.foreground
            font.pixelSize: Theme.textTitle
            font.family: Theme.fontFamily
            font.weight: Theme.weightTitle
        }
    }

    Row {
        id: trailing

        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.spaceTiny
    }
}
