import QtQuick

// What a drag out of an island window carries; see DragOut. One per
// window, outside the panels: a drag's data must outlive the panel it
// started from, which closes as soon as the drag starts, or the app it's
// dropped on would read it after it's gone.
Item {
    id: root

    readonly property var host: Window.window
    // The files, as file:// URLs.
    property var urls: []
    // An ItemGrabResult of what the row shows, kept while the drag is out.
    property var image: null
    property bool carrying: false
    property point hotSpot: Qt.point(0, 0)

    width: 1
    height: 1

    Drag.active: carrying
    Drag.dragType: Drag.Automatic
    Drag.supportedActions: Qt.CopyAction
    Drag.imageSource: image ? image.url : ""
    Drag.hotSpot: hotSpot
    Drag.mimeData: ({
            "text/uri-list": root.urls.map(url => `${url}\r\n`).join("")
        })
    Drag.onDragStarted: {
        DragOut.active = true;
        // Out of the way, so the apps under the panel take the drop.
        Daemon.event("dismiss");
    }
    Drag.onDragFinished: {
        root.carrying = false;
        root.image = null;
        DragOut.active = false;
    }

    function carry(urls: var, image: var, hotSpot: point): void {
        root.urls = urls;
        root.image = image;
        root.hotSpot = hotSpot;
        root.carrying = true;
    }

    Component.onCompleted: DragOut.carriers.push(root)
    Component.onDestruction: DragOut.carriers = Array.from(DragOut.carriers).filter(entry => entry !== root)
}
