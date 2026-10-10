pragma Singleton

import QtQuick
import Quickshell

// Dragging files out of the island onto another app, like screenshots
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

    // Drags `paths`, files, out of `window`, drawn as `image`, an
    // ItemGrabResult or null, with the pointer at `hotSpot` in it.
    function start(window: var, paths: var, image: var, hotSpot: point): void {
        if (root.active || paths.length === 0)
            return;
        const carrier = Array.from(root.carriers).find(entry => entry.host === window);
        if (!carrier)
            return;
        carrier.carry(paths.map(root.url), image, hotSpot);
    }

    // A path as a file:// URL.
    function url(path: string): string {
        return `file://${path.split("/").map(encodeURIComponent).join("/")}`;
    }
}
