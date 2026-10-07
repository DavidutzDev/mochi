import QtQuick

// A control-center tile: an icon and a title, with an optional subtitle.
// Vertical tiles stack them, for actions; horizontal ones put a round icon
// beside the text, for toggles like Wi-Fi. `checked` fills it with its
// `tone`: "accent" or "danger".
Rectangle {
    id: root

    property string icon: ""
    property string title: ""
    property string subtitle: ""
    property bool checked: false
    property string tone: "accent"
    property bool vertical: true
    readonly property bool hovered: area.containsMouse
    signal clicked

    readonly property color fill: tone === "danger" ? Theme.danger : Theme.accent
    readonly property color ink: checked && tone === "accent" ? Theme.onAccent : Theme.foreground

    implicitWidth: vertical ? 120 : 220
    implicitHeight: vertical ? 112 : 64
    radius: Theme.radiusSurface
    color: checked ? fill : hovered ? Theme.raised : Theme.surface
    scale: area.pressed ? 0.97 : 1

    Behavior on color {
        ColorAnimation {
            duration: Theme.fast
        }
    }

    Behavior on scale {
        NumberAnimation {
            duration: Theme.fast
        }
    }

    Column {
        anchors.centerIn: parent
        visible: root.vertical
        spacing: Theme.spaceMedium

        Symbol {
            anchors.horizontalCenter: parent.horizontalCenter
            name: root.icon
            size: 26
            color: root.ink
        }

        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: root.title
            color: root.ink
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
            font.weight: Font.DemiBold
        }
    }

    Row {
        anchors.left: parent.left
        anchors.leftMargin: Theme.spaceMedium
        anchors.right: parent.right
        anchors.rightMargin: Theme.spaceMedium
        anchors.verticalCenter: parent.verticalCenter
        visible: !root.vertical
        spacing: Theme.spaceMedium

        // Smaller in a tile shorter than its usual 64 pixels, like a hub
        // card's one row.
        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: Math.min(40, root.height - Theme.spaceSmall)
            height: width
            radius: height / 2
            color: root.checked ? Qt.alpha(Theme.onAccent, 0.15) : Theme.raised

            Symbol {
                anchors.centerIn: parent
                name: root.icon
                size: 20
                color: root.ink
            }
        }

        Column {
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - 52

            Text {
                width: parent.width
                text: root.title
                elide: Text.ElideRight
                color: root.ink
                font.pixelSize: Theme.textBody
                font.family: Theme.fontFamily
                font.weight: Font.DemiBold
            }

            Text {
                width: parent.width
                visible: text !== ""
                text: root.subtitle
                elide: Text.ElideRight
                color: root.checked ? root.ink : Theme.muted
                opacity: root.checked ? 0.75 : 1
                font.pixelSize: Theme.textCaption
                font.family: Theme.fontFamily
            }
        }
    }

    MouseArea {
        id: area

        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
