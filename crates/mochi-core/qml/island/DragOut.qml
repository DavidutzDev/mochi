pragma Singleton

import QtQuick
import Quickshell

// Dragging a file out of the island onto another app, like a screenshot
// from the control center. A row asks `start()`; the drag itself belongs
// to its island window's carrier (DragCarrier), which lives as long as the
// window: the panel closes once the drag starts, so the apps under it take
// the drop, and the drag goes on without the row it came from. While a
// drag is out, the islands let the pointer through.
Singleton {
    id: root

    property bool active: false
    // Every island window's carrier.
    property list<Item> carriers

    // Drags `url`, a file:// URL, out of `window`, drawn as `image`, an
    // ItemGrabResult or null, with the pointer at `hotSpot` in it.
    function start(window: var, url: string, image: var, hotSpot: point): void {
        if (root.active || url === "")
            return;
        const carrier = Array.from(root.carriers).find(entry => entry.host === window);
        if (!carrier)
            return;
        carrier.carry(url, image, hotSpot);
    }
}
