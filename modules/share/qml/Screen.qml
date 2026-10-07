import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland

// The switchable screen the app shares: a live copy of the chosen screen,
// window or area, as large as fits, centered on black. The pointer shows
// in it. Picking another source in the picker changes it here.
Item {
    id: root

    property var payload: null
    readonly property var source: payload?.source ?? null
    readonly property string kind: source?.kind ?? ""

    readonly property var screen: kind === "window" ? null : Quickshell.screens.find(screen => screen.name === source?.output) ?? null
    readonly property var window: {
        if (kind !== "window")
            return null;
        const bare = value => (value ?? "").replace(/^0x/, "").replace(/^0+/, "").toLowerCase();
        const wanted = bare(source.address);
        return Hyprland.toplevels.values.find(toplevel => bare(toplevel.address) === wanted)?.wayland ?? null;
    }

    // The part of the capture shown, in the capture's logical pixels: all
    // of it, or the area.
    readonly property rect part: {
        if (kind === "area")
            return Qt.rect(source.x, source.y, source.width, source.height);
        if (kind === "screen" && screen)
            return Qt.rect(0, 0, screen.width, screen.height);
        const size = copy.sourceSize;
        return Qt.rect(0, 0, Math.max(size.width, 1), Math.max(size.height, 1));
    }
    // The whole capture, in the same pixels.
    readonly property size whole: kind === "window" ? copy.sourceSize : Qt.size(screen?.width ?? 1, screen?.height ?? 1)
    readonly property real scale: Math.min(width / Math.max(part.width, 1), height / Math.max(part.height, 1))

    clip: true

    ScreencopyView {
        id: copy

        captureSource: root.kind === "window" ? root.window : root.screen
        live: true
        paintCursor: true
        x: (root.width - root.part.width * root.scale) / 2 - root.part.x * root.scale
        y: (root.height - root.part.height * root.scale) / 2 - root.part.y * root.scale
        width: root.whole.width * root.scale
        height: root.whole.height * root.scale
    }

    // What's shared went away, like a closed window.
    Text {
        anchors.centerIn: parent
        visible: root.source !== null && copy.captureSource === null
        text: "Nothing to show: pick something else from Mochi's sharing bubble"
        color: "#8a8a8a" // design: drawn into the shared video, always on black
        font.pixelSize: 28 // design: read from a whole screen shared in a call
    }
}
