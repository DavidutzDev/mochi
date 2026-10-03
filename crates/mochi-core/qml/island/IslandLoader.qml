import QtQuick

// One of the island's two view slots. The current slot fades in after a short
// delay so the shape moves first; the other fades out and unloads.
Loader {
    id: root

    required property bool current

    anchors.centerIn: parent
    asynchronous: false
    opacity: current ? 1 : 0
    scale: current ? 1 : 0.94

    Behavior on opacity {
        SequentialAnimation {
            PauseAnimation {
                duration: root.current ? Theme.fadeDelay : 0
            }
            NumberAnimation {
                duration: root.current ? Theme.fadeIn : Theme.fadeOut
                easing.type: Easing.OutCubic
            }
        }
    }

    Behavior on scale {
        NumberAnimation {
            duration: Theme.fadeIn
            easing.type: Easing.OutCubic
        }
    }

    onOpacityChanged: {
        if (!current && opacity === 0)
            source = "";
    }
}
