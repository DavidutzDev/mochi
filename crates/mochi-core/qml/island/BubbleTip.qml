import QtQuick
import Quickshell

// A bubble's tooltip: a line or two beside it, below the bubble, or above
// when the island sits at the bottom, once the pointer has rested on it
// for `[bubbles] tooltip_ms`. Empty text shows nothing.
PopupWindow {
    id: root

    // The bubble it's for.
    required property Item target
    property string text: ""
    // The pointer is over the bubble.
    property bool hovered: false
    readonly property bool below: Theme.anchor !== "bottom"
    readonly property bool wanted: hovered && text !== "" && Daemon.bubbleTooltipMs > 0

    anchor.item: target
    anchor.rect.x: target.width / 2
    anchor.rect.y: below ? target.height + Theme.spaceSmall : -Theme.spaceSmall
    anchor.edges: below ? Edges.Bottom : Edges.Top
    anchor.gravity: below ? Edges.Bottom : Edges.Top
    visible: wanted && !rest.running
    implicitWidth: Math.min(tip.implicitWidth, 300) + Theme.padding * 2
    implicitHeight: tip.implicitHeight + Theme.spaceSmall * 2
    color: "transparent"

    // The pointer rests a moment first, so passing over shows nothing.
    Timer {
        id: rest

        interval: Daemon.bubbleTooltipMs
        running: root.wanted
    }

    Rectangle {
        anchors.fill: parent
        radius: Theme.radiusField
        color: Theme.background
        border.color: Theme.border
        border.width: 1

        Text {
            id: tip

            x: Theme.padding
            y: Theme.spaceSmall
            width: Math.min(implicitWidth, 300)
            text: root.text
            wrapMode: Text.Wrap
            textFormat: Text.PlainText
            color: Theme.foreground
            font.pixelSize: Theme.textCaption
            font.family: Theme.fontFamily
        }
    }
}
