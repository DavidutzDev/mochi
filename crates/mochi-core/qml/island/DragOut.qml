pragma Singleton

import Quickshell

// Whether something is being dragged out of the island onto another app,
// like a screenshot from the control center. A panel catches every click
// on its screen to close on one outside; while a drag is out, the islands
// let the pointer through instead, so the app under it takes the drop.
Singleton {
    property bool active: false
}
