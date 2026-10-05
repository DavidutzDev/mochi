import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.island

// The picker over one monitor, under the island. For a screenshot it shows
// the screen frozen the moment the picker opened, and saves that frame for
// the module to cut from; a recording picks over the live screen. Outside
// what's picked, the screen dims.
//
// Positions sent to the module are in the global layout, in logical
// pixels: this screen's own position plus the point on it.
Item {
    id: root

    property var payload: ({})
    property var screen: null

    readonly property string output: screen?.name ?? ""
    readonly property real originX: screen?.x ?? 0
    readonly property real originY: screen?.y ?? 0
    readonly property bool screenshot: payload.kind === "screenshot"
    readonly property string stage: payload.stage ?? ""
    readonly property string mode: payload.mode ?? ""
    readonly property bool picking: stage === "select"
    // The window grows to cover the screen when the picker opens; nothing is
    // drawn before it does, or the frozen frame would stretch with it.
    readonly property bool covering: screen !== null && height >= screen.height
    // The island waits for the frozen frame, so the frame shows the screen
    // as it was, not the picker.
    // Not before it knows its screen and its picker either: until then it
    // can't tell whether it has a frame to freeze.
    readonly property bool ready: covering && output !== "" && payload.session !== undefined && (!screenshot || frozen.hasContent)

    // The region drawn, in this screen's coordinates, or null. It's global:
    // it may lie partly or wholly on another screen.
    readonly property var region: {
        const region = payload.region;
        if (!region)
            return null;
        return Qt.rect(region.x - originX, region.y - originY, region.width, region.height);
    }
    // Where a drag may go, in this screen's coordinates: every screen for a
    // screenshot, which can join them, only this one for a recording.
    readonly property rect bounds: {
        if (!screenshot)
            return Qt.rect(0, 0, width, height);
        const screens = Daemon.screens;
        let left = 0, top = 0, right = width, bottom = height;
        for (const other of screens) {
            left = Math.min(left, other.x - originX);
            top = Math.min(top, other.y - originY);
            right = Math.max(right, other.x - originX + other.width);
            bottom = Math.max(bottom, other.y - originY + other.height);
        }
        return Qt.rect(left, top, right - left, bottom - top);
    }
    // Whether the region's middle is on this screen: its size and button
    // show there only.
    readonly property bool holdsRegion: {
        const area = shownRegion;
        if (!area)
            return false;
        const x = area.x + area.width / 2;
        const y = area.y + area.height / 2;
        return x >= 0 && x < width && y >= 0 && y < height;
    }

    // The first screen's overlay tells the module where every screen is,
    // once per picker.
    property int layoutSent: -1
    onPayloadChanged: sendLayout()
    onOutputChanged: sendLayout()
    Component.onCompleted: sendLayout()
    function sendLayout(): void {
        const session = payload.session;
        if (session === undefined || layoutSent === session || Daemon.screens[0]?.name !== output)
            return;
        layoutSent = session;
        const words = [String(session)];
        for (const screen of Daemon.screens)
            words.push(screen.name, String(screen.x), String(screen.y), String(screen.width), String(screen.height));
        Daemon.command("capture", "layout", words);
    }
    // While dragging: the region being drawn or moved, before the module
    // hears of it.
    property var dragged: null
    readonly property var shownRegion: dragged ?? region

    // The topmost window under the pointer, in local coordinates, or null.
    readonly property var hoveredWindow: {
        if (mode !== "window" || !pointer.containsMouse)
            return null;
        const x = pointer.mouseX + originX;
        const y = pointer.mouseY + originY;
        const window = (payload.windows ?? []).find(w => x >= w.x && x < w.x + w.width && y >= w.y && y < w.y + w.height);
        return window ? Object.assign({}, window, {
            "x": window.x - originX,
            "y": window.y - originY
        }) : null;
    }

    // What stays bright.
    readonly property var hole: {
        if (!picking)
            return null;
        switch (mode) {
        case "region":
            return shownRegion;
        case "window":
            return hoveredWindow ? Qt.rect(hoveredWindow.x, hoveredWindow.y, hoveredWindow.width, hoveredWindow.height) : null;
        case "screen":
            return pointer.containsMouse ? Qt.rect(0, 0, width, height) : null;
        case "all":
            return Qt.rect(0, 0, width, height);
        }
        return null;
    }

    function send(action: string, rect: rect): void {
        Daemon.command("capture", action, [output, String(rect.x + originX), String(rect.y + originY), String(rect.width), String(rect.height)]);
    }

    // While dragging, the region goes to the module every 30 ms, so the
    // other screens draw their part of it too.
    property real lastSent: 0
    function share(region: rect): void {
        const now = Date.now();
        if (now - lastSent < 30 || region.width < 4 || region.height < 4)
            return;
        lastSent = now;
        send("region", region);
    }

    // `value` kept between `low` and `high`.
    function clamp(value: real, low: real, high: real): real {
        return Math.max(low, Math.min(value, high));
    }

    function cancel(): void {
        Daemon.command("capture", "cancel", []);
    }

    // Every monitor's overlay asks for the keyboard; the compositor gives it
    // to one. The island swapping its view takes focus with it, so the
    // overlay takes it back whenever it moves elsewhere while picking.
    readonly property bool wantsKeys: picking
    readonly property Item focused: Window.activeFocusItem
    onWantsKeysChanged: takeKeys()
    onFocusedChanged: takeKeys()
    function takeKeys(): void {
        if (wantsKeys && !keys.activeFocus)
            Qt.callLater(() => {
                if (wantsKeys)
                    keys.forceActiveFocus();
            });
    }

    ScreencopyView {
        id: frozen

        // The monitor's size, never the window's, so it can't stretch.
        width: root.screen?.width ?? 0
        height: root.screen?.height ?? 0
        visible: root.screenshot && root.covering
        captureSource: root.screenshot ? root.screen : null
        live: false

        // Nothing is saved while the picker opens, so its animation runs
        // smoothly: only the picked monitor's frame, once it's picked.
        readonly property bool wanted: hasContent && root.stage === "saving" && (root.payload.picked ?? []).includes(root.output)
        property int saved: -1

        onWantedChanged: {
            const session = root.payload.session;
            if (!wanted || saved === session)
                return;
            saved = session;
            const path = `${root.payload.frames}/frame-${root.output}.ppm`;
            const started = Date.now();
            grabToImage(result => {
                // PPM is raw pixels: quick to write. The module compresses.
                if (result.saveToFile(path))
                    Daemon.command("capture", "frame", [String(session), root.output, String(root.originX), String(root.originY), String(width), String(height)]);
                else
                    console.warn(`capture: could not save ${path}`);
                console.debug(`capture: saved the frame in ${Date.now() - started} ms`);
            }, Qt.size(sourceSize.width, sourceSize.height));
        }
    }

    // The screen just stops: no shade until something is picked. Then the
    // rest dims at once, as four strips around it. A whole screen gets only
    // the outline.
    Item {
        anchors.fill: parent
        visible: root.ready && root.hole !== null && root.mode !== "screen"
        opacity: 0.5

        readonly property rect cut: root.hole ?? Qt.rect(0, 0, 0, 0)

        Rectangle {
            width: parent.width
            height: parent.cut.y
            color: "black"
        }

        Rectangle {
            y: parent.cut.y + parent.cut.height
            width: parent.width
            height: parent.height - y
            color: "black"
        }

        Rectangle {
            y: parent.cut.y
            width: parent.cut.x
            height: parent.cut.height
            color: "black"
        }

        Rectangle {
            x: parent.cut.x + parent.cut.width
            y: parent.cut.y
            width: parent.width - x
            height: parent.cut.height
            color: "black"
        }
    }

    // The outline of what's picked.
    Rectangle {
        visible: root.ready && root.hole !== null
        x: root.hole?.x ?? 0
        y: root.hole?.y ?? 0
        width: root.hole?.width ?? 0
        height: root.hole?.height ?? 0
        color: "transparent"
        border.color: Theme.accent
        border.width: root.mode === "screen" ? 4 : 2
    }

    // Takes the clicks and drags.
    MouseArea {
        id: pointer

        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        cursorShape: root.mode === "region" && root.picking ? Qt.CrossCursor : Qt.PointingHandCursor

        // A region drag: where it started, and whether it moves the region.
        property point start
        property var moving: null

        onPressed: mouse => {
            if (mouse.button === Qt.RightButton) {
                root.cancel();
                return;
            }
            if (!root.picking || root.mode !== "region")
                return;
            start = Qt.point(mouse.x, mouse.y);
            const region = root.region;
            const inside = region && mouse.x >= region.x && mouse.x < region.x + region.width && mouse.y >= region.y && mouse.y < region.y + region.height;
            moving = inside ? region : null;
            root.dragged = inside ? region : Qt.rect(mouse.x, mouse.y, 0, 0);
        }

        // The pointer stays with this screen while the button is down, even
        // over the next one, so a drag can cross into it.
        onPositionChanged: mouse => {
            if (!pressed || root.dragged === null)
                return;
            const bounds = root.bounds;
            if (moving) {
                const x = root.clamp(moving.x + mouse.x - start.x, bounds.x, bounds.x + bounds.width - moving.width);
                const y = root.clamp(moving.y + mouse.y - start.y, bounds.y, bounds.y + bounds.height - moving.height);
                root.dragged = Qt.rect(x, y, moving.width, moving.height);
            } else {
                const x = root.clamp(mouse.x, bounds.x, bounds.x + bounds.width);
                const y = root.clamp(mouse.y, bounds.y, bounds.y + bounds.height);
                root.dragged = Qt.rect(Math.min(start.x, x), Math.min(start.y, y), Math.abs(x - start.x), Math.abs(y - start.y));
            }
            root.share(root.dragged);
        }

        onReleased: mouse => {
            if (mouse.button !== Qt.LeftButton || root.dragged === null)
                return;
            const region = root.dragged;
            // A click without a drag keeps the region there was.
            if (region.width >= 4 && region.height >= 4)
                root.send("region", region);
            root.dragged = null;
            moving = null;
        }

        onClicked: mouse => {
            if (mouse.button !== Qt.LeftButton)
                return;
            if (!root.picking)
                return;
            if (root.mode === "window" && root.hoveredWindow)
                root.send("select", Qt.rect(root.hoveredWindow.x, root.hoveredWindow.y, root.hoveredWindow.width, root.hoveredWindow.height));
            else if (root.mode === "screen" || root.mode === "all")
                root.send("select", Qt.rect(0, 0, root.width, root.height));
        }

        onDoubleClicked: mouse => {
            if (root.mode === "region" && root.region)
                Daemon.command("capture", "confirm", []);
        }
    }

    // Handles on the region's corners resize it.
    Repeater {
        model: root.mode === "region" && root.picking && root.shownRegion ? 4 : 0

        Rectangle {
            id: handle

            required property int index
            readonly property bool onRight: index % 2 === 1
            readonly property bool onBottom: index >= 2
            readonly property rect area: root.shownRegion ?? Qt.rect(0, 0, 0, 0)

            x: (onRight ? area.x + area.width : area.x) - width / 2
            y: (onBottom ? area.y + area.height : area.y) - height / 2
            width: 12
            height: 12
            radius: 6
            color: Theme.foreground
            border.color: Theme.accent
            border.width: 2

            MouseArea {
                anchors.fill: parent
                anchors.margins: -6
                cursorShape: handle.onRight === handle.onBottom ? Qt.SizeFDiagCursor : Qt.SizeBDiagCursor

                // The corner opposite stays put.
                property point anchor

                onPressed: {
                    const area = root.region;
                    anchor = Qt.point(handle.onRight ? area.x : area.x + area.width, handle.onBottom ? area.y : area.y + area.height);
                    root.dragged = area;
                }

                onPositionChanged: mouse => {
                    if (!pressed)
                        return;
                    const point = mapToItem(root, mouse.x, mouse.y);
                    const bounds = root.bounds;
                    const x = root.clamp(point.x, bounds.x, bounds.x + bounds.width);
                    const y = root.clamp(point.y, bounds.y, bounds.y + bounds.height);
                    root.dragged = Qt.rect(Math.min(anchor.x, x), Math.min(anchor.y, y), Math.abs(x - anchor.x), Math.abs(y - anchor.y));
                    root.share(root.dragged);
                }

                onReleased: {
                    if (root.dragged.width >= 4 && root.dragged.height >= 4)
                        root.send("region", root.dragged);
                    root.dragged = null;
                }
            }
        }
    }

    // Under the region: its size and the button that takes it.
    Row {
        readonly property rect area: root.shownRegion ?? Qt.rect(0, 0, 0, 0)

        visible: root.mode === "region" && root.picking && root.holdsRegion
        spacing: 8
        x: Math.max(8, Math.min(root.width - width - 8, area.x + (area.width - width) / 2))
        y: area.y + area.height + 12 + height > root.height ? area.y - height - 12 : area.y + area.height + 12

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: size.implicitWidth + 20
            height: 30
            radius: 15
            color: Theme.background

            Text {
                id: size

                anchors.centerIn: parent
                text: `${Math.round(parent.parent.area.width)} × ${Math.round(parent.parent.area.height)}`
                color: Theme.foreground
                font.pixelSize: Theme.textLabel
                font.family: Theme.fontFamily
                font.features: {
                    "tnum": 1
                }
            }
        }

        Button {
            visible: root.dragged === null
            tone: "accent"
            icon: root.screenshot ? "camera" : "record"
            text: root.screenshot ? "Capture" : "Record"
            onClicked: Daemon.command("capture", "confirm", [])
        }
    }

    // The window's name under the pointer.
    Rectangle {
        visible: root.picking && root.hoveredWindow !== null
        x: Math.max(8, Math.min(root.width - width - 8, (root.hoveredWindow?.x ?? 0) + ((root.hoveredWindow?.width ?? 0) - width) / 2))
        y: Math.max(8, Math.min(root.height - height - 8, (root.hoveredWindow?.y ?? 0) + ((root.hoveredWindow?.height ?? 0) - height) / 2))
        width: Math.min(title.implicitWidth + 28, 420)
        height: 34
        radius: 17
        color: Theme.background

        Text {
            id: title

            anchors.fill: parent
            anchors.leftMargin: 14
            anchors.rightMargin: 14
            verticalAlignment: Text.AlignVCenter
            text: root.hoveredWindow?.title || root.hoveredWindow?.app_id || ""
            elide: Text.ElideRight
            color: Theme.foreground
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
        }
    }

    function switchMode(mode: string): void {
        if (mode && mode !== root.mode)
            Daemon.command("capture", "mode", [mode]);
    }

    // Keys, on whichever monitor has the keyboard: Enter takes the region,
    // Tab or a number switch modes, A the desktop audio, M the microphone, F
    // the frame rate, Q the resolution, Escape cancels.
    Item {
        id: keys

        readonly property var modes: root.payload.modes ?? []

        Keys.onEscapePressed: root.cancel()
        Keys.onReturnPressed: Daemon.command("capture", "confirm", [])
        Keys.onEnterPressed: Daemon.command("capture", "confirm", [])
        Keys.onTabPressed: root.switchMode(modes[(modes.indexOf(root.mode) + 1) % modes.length])
        Keys.onBacktabPressed: root.switchMode(modes[(modes.indexOf(root.mode) - 1 + modes.length) % modes.length])
        Keys.onPressed: event => {
            if (event.key >= Qt.Key_1 && event.key < Qt.Key_1 + modes.length) {
                root.switchMode(modes[event.key - Qt.Key_1]);
                event.accepted = true;
            } else if (event.key === Qt.Key_A && !root.screenshot) {
                Daemon.command("capture", "audio", []);
                event.accepted = true;
            } else if (event.key === Qt.Key_M && !root.screenshot) {
                Daemon.command("capture", "microphone", []);
                event.accepted = true;
            } else if (event.key === Qt.Key_F && !root.screenshot) {
                Daemon.command("capture", "framerate", []);
                event.accepted = true;
            } else if (event.key === Qt.Key_Q && !root.screenshot) {
                Daemon.command("capture", "resolution", []);
                event.accepted = true;
            }
        }
    }
}
