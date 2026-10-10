import QtQuick

// Scrolling for a Flickable or a ListView, as in a browser. A mouse
// wheel's step glides to where it leads instead of jumping, and steps that
// come quickly add up. A touchpad follows the fingers, and the list coasts
// on and slows down once they lift. Put it inside the view, with `view`
// set to it, next to its ScrollFade. The theme's `[motion] smooth_scroll`
// turns it off, and `scroll_step` sets how far a wheel's step goes.
WheelHandler {
    id: root

    required property Flickable view
    property real step: Theme.wheelStep

    // The touchpad's speed, in pixels a second down the content, and when
    // it last moved.
    property real speed: 0
    property real stamp: 0

    enabled: Theme.smoothScroll
    target: null
    orientation: Qt.Vertical
    acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad

    // How far the view scrolls each way.
    function clamp(y: real): real {
        const top = root.view.originY - root.view.topMargin;
        const bottom = Math.max(top, root.view.originY + root.view.contentHeight + root.view.bottomMargin - root.view.height);
        return Math.max(top, Math.min(bottom, y));
    }

    onWheel: event => {
        // A touchpad sends pixels, in phases from the touch to the lift.
        if (event.pixelDelta.y !== 0 || event.phase !== Qt.NoScrollPhase)
            root.touchpad(event.pixelDelta.y, event.phase);
        else
            root.turn(event.angleDelta.y);
    }

    // A wheel turned by `angle`, 120 a step, up positive: on from where
    // the last step was going.
    function turn(angle: real): void {
        root.view.cancelFlick();
        const from = root.glide.running ? root.glide.to : root.view.contentY;
        root.glide.to = root.clamp(from - angle / 120 * root.step);
        root.glide.from = root.view.contentY;
        root.glide.restart();
    }

    // Fingers moved by `pixels` on a touchpad, up positive, in `phase`.
    function touchpad(pixels: real, phase: int): void {
        root.glide.stop();
        if (phase === Qt.ScrollBegin) {
            root.view.cancelFlick();
            root.speed = 0;
        }
        if (phase === Qt.ScrollEnd) {
            // A flick's velocity counts up the content.
            if (Math.abs(root.speed) > 60)
                root.view.flick(0, -root.speed);
            root.speed = 0;
            return;
        }
        const now = Date.now();
        const elapsed = Math.max(1, now - root.stamp);
        root.stamp = now;
        const move = -pixels;
        root.view.contentY = root.clamp(root.view.contentY + move);
        const current = move / elapsed * 1000;
        root.speed = elapsed > 100 ? current : root.speed * 0.6 + current * 0.4;
    }

    property NumberAnimation glide: NumberAnimation {
        target: root.view
        property: "contentY"
        duration: Theme.duration(280)
        easing.type: Easing.OutCubic
    }
}
