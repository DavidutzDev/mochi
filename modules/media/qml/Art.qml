import QtQuick
import Quickshell.Widgets
import qs.island

// The cover, or a note while there is none or it doesn't load.
Item {
    id: root

    property string source: ""
    property real size: 24
    property real radius: Math.round(size / 5)

    // Qt's software renderer, which draws the lock screen, has no shaders,
    // and ClippingRectangle needs them: there, a canvas rounds the cover.
    readonly property bool software: GraphicsInfo.api === GraphicsInfo.Software
    readonly property bool loaded: cover.status === Image.Ready

    implicitWidth: size
    implicitHeight: size

    Rectangle {
        anchors.fill: parent
        radius: root.radius
        color: Theme.raised
    }

    Symbol {
        anchors.centerIn: parent
        visible: !root.loaded
        name: "note"
        size: root.size * 0.5
        color: Theme.muted
    }

    ClippingRectangle {
        anchors.fill: parent
        visible: !root.software
        radius: root.radius
        color: "transparent"

        Image {
            id: cover

            anchors.fill: parent
            source: root.source
            sourceSize.width: root.size * 2
            sourceSize.height: root.size * 2
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            opacity: status === Image.Ready ? 1 : 0

            Behavior on opacity {
                NumberAnimation {
                    duration: Theme.fast
                }
            }
        }
    }

    Canvas {
        id: rounded

        anchors.fill: parent
        visible: root.software && root.loaded

        // The canvas only draws images it loaded itself: the same cover,
        // which Qt's image cache already has.
        property string wanted: root.software ? root.source : ""

        onWantedChanged: {
            if (wanted !== "" && !isImageLoaded(wanted))
                loadImage(wanted);
            requestPaint();
        }
        onImageLoaded: requestPaint()
        onPaint: {
            const context = getContext("2d");
            context.reset();
            if (wanted === "" || !isImageLoaded(wanted) || width <= 0 || height <= 0)
                return;
            // Filling the square, its middle, as the image crops it.
            const aspect = cover.implicitHeight > 0 ? cover.implicitWidth / cover.implicitHeight : 1;
            const drawnWidth = aspect >= 1 ? height * aspect : width;
            const drawnHeight = aspect >= 1 ? height : width / aspect;
            const radius = Math.min(root.radius, width / 2, height / 2);
            context.beginPath();
            context.roundedRect(0, 0, width, height, radius, radius);
            context.clip();
            context.drawImage(wanted, (width - drawnWidth) / 2, (height - drawnHeight) / 2, drawnWidth, drawnHeight);
        }

        Connections {
            target: root

            function onRadiusChanged() {
                rounded.requestPaint();
            }

            function onLoadedChanged() {
                rounded.requestPaint();
            }
        }

        onWidthChanged: requestPaint()
        onHeightChanged: requestPaint()
    }
}
