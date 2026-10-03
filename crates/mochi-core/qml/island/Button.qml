import QtQuick

// A pill button with text, an icon, or both; with only an icon it's round.
// `tone` sets its colors: "neutral" (raised), "accent", "danger", or "ghost"
// (no fill until hovered).
Rectangle {
    id: root

    property string text: ""
    property string icon: ""
    property string tone: "neutral"
    property real iconSize: 15
    readonly property bool hovered: area.containsMouse
    signal clicked

    readonly property color ink: tone === "accent" ? Theme.onAccent : Theme.foreground

    implicitHeight: 30
    implicitWidth: text === "" ? implicitHeight : content.implicitWidth + 24
    radius: height / 2
    opacity: enabled ? 1 : 0.4
    scale: area.pressed ? 0.95 : 1
    color: {
        switch (tone) {
        case "accent":
            return hovered ? Qt.lighter(Theme.accent, 1.12) : Theme.accent;
        case "danger":
            return hovered ? Qt.lighter(Theme.danger, 1.12) : Theme.danger;
        case "ghost":
            return hovered ? Theme.raised : "transparent";
        default:
            return hovered ? Theme.highlight : Theme.raised;
        }
    }

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

    Row {
        id: content

        anchors.centerIn: parent
        spacing: 6

        Symbol {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.icon !== ""
            name: root.icon
            size: root.iconSize
            color: root.ink
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.text !== ""
            text: root.text
            color: root.ink
            font.pixelSize: Theme.textLabel
            font.family: Theme.fontFamily
            font.weight: Font.DemiBold
        }
    }

    MouseArea {
        id: area

        anchors.fill: parent
        enabled: root.enabled
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
