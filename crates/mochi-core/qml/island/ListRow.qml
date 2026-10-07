import QtQuick

// A row in a list: something at the start (an `icon`, an `image`, or any
// item put in `leading`), a title and subtitle, and anything put in
// `trailing`, like a close button. `flat` rows have no fill until hovered or
// `selected`; `marker` draws the accent bar on the selected one.
Rectangle {
    id: root

    property string icon: ""
    property string image: ""
    property string title: ""
    property string subtitle: ""
    // Text.StyledText for a subtitle with markup; its links emit
    // `linkActivated` instead of `clicked`.
    property int subtitleFormat: Text.PlainText
    property bool selected: false
    property bool flat: false
    property bool marker: false
    property real leadingSize: 34
    property alias leading: leadingSlot.data
    property alias trailing: trailingSlot.data
    readonly property bool hovered: area.containsMouse
    signal clicked
    signal linkActivated(string link)

    implicitWidth: 360
    implicitHeight: 56
    radius: Theme.radiusMedium
    color: selected ? Theme.raised : hovered ? (flat ? Theme.surface : Theme.raised) : flat ? "transparent" : Theme.surface

    Behavior on color {
        ColorAnimation {
            duration: Theme.fast
        }
    }

    MouseArea {
        id: area

        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }

    Rectangle {
        x: 0
        anchors.verticalCenter: parent.verticalCenter
        visible: root.marker && root.selected
        width: 3
        height: parent.height - 20
        radius: 1.5
        color: Theme.accent
    }

    Item {
        id: leadingSlot

        x: 14
        anchors.verticalCenter: parent.verticalCenter
        width: root.leadingSize
        height: root.leadingSize

        Symbol {
            anchors.centerIn: parent
            visible: root.icon !== ""
            name: root.icon
            size: root.leadingSize * 0.6
            color: Theme.foreground
        }

        Image {
            anchors.fill: parent
            visible: root.image !== ""
            source: root.image
            sourceSize.width: root.leadingSize * 2
            sourceSize.height: root.leadingSize * 2
            fillMode: Image.PreserveAspectFit
            asynchronous: true
        }
    }

    Column {
        anchors.left: leadingSlot.right
        anchors.leftMargin: Theme.spaceMedium
        anchors.right: trailingSlot.left
        anchors.rightMargin: Theme.spaceMedium
        anchors.verticalCenter: parent.verticalCenter

        Text {
            width: parent.width
            text: root.title
            elide: Text.ElideRight
            textFormat: Text.PlainText
            color: Theme.foreground
            font.pixelSize: Theme.textSubtitle
            font.family: Theme.fontFamily
            font.weight: Font.DemiBold
        }

        Text {
            width: parent.width
            visible: text !== ""
            text: root.subtitle
            elide: Text.ElideRight
            textFormat: root.subtitleFormat
            color: Theme.muted
            linkColor: Theme.accent
            onLinkActivated: link => root.linkActivated(link)
            font.pixelSize: Theme.textLabel
            font.family: Theme.fontFamily
        }
    }

    Row {
        id: trailingSlot

        anchors.right: parent.right
        anchors.rightMargin: Theme.spaceMedium
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.spaceSmall
    }
}
