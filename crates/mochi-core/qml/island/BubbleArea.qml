import QtQuick
import "Lists.js" as Lists

// One of the five areas: a row of pills, left to right, with room for the
// island when it sits here. The island stays a child of the window; it is
// placed over `slot`, which takes its size, so the pills move as it grows.
//
// With `[bubbles] stack`, the pills stack instead: the most important in
// front, the next two peeking out behind it, away from the island. A pill
// with news comes to the front for a while. Hovering fans the stack out
// into the row.
Row {
    id: root

    required property string area
    required property Item island
    // The window, which keeps the list of pills for the input mask and blur.
    required property var window
    property real attached: 0
    property bool atBottom: false

    readonly property bool hostsIsland: Theme.islandArea === area
    // The island sits at the screen edge in the side areas and next to the
    // center otherwise.
    readonly property bool islandAtEnd: area === "center-left" || area === "right"
    readonly property Item slot: islandAtEnd ? endSlot : startSlot
    readonly property bool onSide: area === "left" || area === "right"

    // Consecutive bubbles with the same group share a pill.
    readonly property var pills: {
        const pills = [];
        for (const bubble of Daemon.bubbles) {
            if (bubble.area !== area)
                continue;
            const last = pills[pills.length - 1];
            if (bubble.group != null && last && last.group === bubble.group) {
                last.bubbles.push(bubble);
            } else {
                const key = bubble.group != null ? `group/${bubble.group}` : Lists.bubbleKey(bubble);
                pills.push({ key: key, group: bubble.group ?? null, bubbles: [bubble] });
            }
        }
        return pills;
    }
    readonly property var pillsByKey: {
        const map = {};
        for (const pill of pills)
            map[pill.key] = pill.bubbles;
        return map;
    }
    readonly property int hidden: Daemon.overflow.find(entry => entry.area === area)?.hidden ?? 0

    // Stacking: which pill is in front, and whether it's fanned out.
    readonly property bool stacking: Daemon.bubbleStack !== null
    readonly property bool fanned: !stacking || hover.hovered || folding.running
    // The pill with fresh news, in front until `news` runs out.
    property string newsKey: ""
    property real seenNews: -1
    readonly property string frontKey: {
        if (newsKey !== "" && pillsByKey[newsKey])
            return newsKey;
        let best = null;
        for (const pill of pills) {
            const priority = Math.max(...pill.bubbles.map(bubble => bubble.priority ?? 0));
            if (best === null || priority > best.priority)
                best = { "key": pill.key, "priority": priority };
        }
        return best?.key ?? "";
    }
    // Peeks spread away from the island.
    readonly property bool peekLeft: area === "left" || area === "center-left"

    onPillsChanged: {
        Lists.sync(pillModel, pills.map(pill => pill.key));
        // The pill whose news went up since the last look comes forward.
        let latest = seenNews;
        let key = "";
        for (const pill of pills) {
            for (const bubble of pill.bubbles) {
                if ((bubble.news ?? 0) > latest) {
                    latest = bubble.news;
                    key = pill.key;
                }
            }
        }
        if (key !== "" && seenNews >= 0 && stacking) {
            newsKey = key;
            news.restart();
        }
        seenNews = Math.max(seenNews, latest, 0);
        Qt.callLater(deck.layout);
    }
    onFannedChanged: Qt.callLater(deck.layout)
    onFrontKeyChanged: Qt.callLater(deck.layout)

    Timer {
        id: news

        interval: Daemon.bubbleStack?.news_ms ?? 4000
        onTriggered: root.newsKey = ""
    }

    // Leaving the stack folds it a moment later, so crossing a gap between
    // fanned pills doesn't fold it under the pointer.
    Timer {
        id: folding

        interval: 400
    }

    Component.onCompleted: Lists.sync(pillModel, pills.map(pill => pill.key))

    spacing: Theme.spacing

    // In notch mode, whatever is outermost in a side area touches that side
    // of the screen too.
    function touchesSide(item: Item): real {
        if (!onSide)
            return 0;
        // Pills sit in the deck; the count sits in the row.
        const x = item.x + (item.parent === deck ? deck.x : 0);
        const outermost = area === "left" ? x < 0.5 : x + item.width > width - 0.5;
        return outermost ? attached : 0;
    }

    Item {
        id: startSlot

        visible: root.hostsIsland && !root.islandAtEnd && root.island.shown
        width: root.island.width
        height: root.island.height
        y: root.atBottom ? root.height - height : 0
    }

    // The pills, placed by `layout`: in a row, or stacked.
    Item {
        id: deck

        // Tells PillRegion its pills sit one level deeper.
        readonly property bool isDeck: true

        width: 0
        // A size of its own: from the row's, it would be 0, and draw nothing.
        height: Theme.idleHeight
        // Shrinks to nothing as the last pill leaves, then takes no spacing.
        visible: width > 0.5

        HoverHandler {
            id: hover

            enabled: root.stacking
            onHoveredChanged: {
                if (!hovered)
                    folding.restart();
            }
        }

        // Moves a pill to `x`: at once the first time, so a new pill grows
        // in where it belongs, and sliding after that.
        function place(item: Item, x: real): void {
            item.x = x;
            item.placed = true;
        }

        function layout(): void {
            // Leaving pills fade out where they are, and the rest close up.
            const items = [];
            for (let index = 0; index < placed.count; index++) {
                const item = placed.itemAt(index);
                if (item && !item.leaving)
                    items.push(item);
            }
            if (root.fanned) {
                let x = 0;
                for (const item of items) {
                    place(item, x);
                    item.z = 0;
                    item.stackScale = 1;
                    item.opacity = 1;
                    x += item.implicitWidth + root.spacing;
                }
                width = Math.max(0, x - root.spacing);
                return;
            }
            const front = items.find(item => item.key === root.frontKey) ?? items[0];
            if (!front)
                return;
            const behind = items.filter(item => item !== front);
            const peek = 8;
            const peeking = Math.min(behind.length, 2);
            // From the target, not `width`, which animates and still reads
            // the old value here.
            const total = front.implicitWidth + peek * peeking;
            width = total;
            const sign = root.peekLeft ? -1 : 1;
            const centre = root.peekLeft ? total - front.implicitWidth / 2 : front.implicitWidth / 2;
            place(front, centre - front.implicitWidth / 2);
            front.z = behind.length + 1;
            front.stackScale = 1;
            front.opacity = 1;
            behind.forEach((item, index) => {
                const depth = Math.min(index + 1, 2);
                place(item, centre + sign * peek * depth - item.implicitWidth / 2);
                item.z = behind.length - index;
                item.stackScale = 1 - 0.12 * depth;
                item.opacity = index < 2 ? 1 - 0.3 * depth : 0;
            });
        }

        Behavior on width {
            NumberAnimation {
                duration: Theme.move
                easing.type: Easing.BezierSpline
                easing.bezierCurve: Theme.overshoot
            }
        }

        Repeater {
            id: placed

            model: ListModel {
                id: pillModel
            }
            onItemAdded: Qt.callLater(deck.layout)
            onItemRemoved: Qt.callLater(deck.layout)

            Pill {
                id: pill

                required property string key
                // Its bubbles are gone: it animates out, then its row goes.
                required property bool leaving
                // Set by the deck's layout.
                property real stackScale: 1
                property bool placed: false
                // The last bubbles it had, shown while it leaves.
                property var held: []

                bubbles: root.pillsByKey[key] ?? held
                onBubblesChanged: {
                    if (bubbles.length > 0 && bubbles !== held)
                        held = bubbles;
                }
                exiting: leaving
                // Out of the input mask and blur at once. In a row, it goes
                // under the pills that slide over its place; in a stack, it
                // stays on top while it shrinks, so it is seen to go.
                onLeavingChanged: {
                    if (leaving) {
                        root.window.removePill(pill);
                        if (root.fanned)
                            z = -1;
                    } else {
                        root.window.addPill(pill);
                    }
                    Qt.callLater(deck.layout);
                }
                onGone: Lists.removeLeft(pillModel, key)
                y: root.atBottom ? root.height - height : 0
                attached: root.attached
                sideAttached: root.touchesSide(pill)
                atBottom: root.atBottom
                atRight: root.area === "right"
                transform: Scale {
                    origin.x: pill.width / 2
                    origin.y: pill.height / 2
                    xScale: pill.stackScale
                    yScale: pill.stackScale

                    Behavior on xScale {
                        NumberAnimation {
                            duration: Theme.move
                            easing.type: Easing.OutCubic
                        }
                    }

                    Behavior on yScale {
                        NumberAnimation {
                            duration: Theme.move
                            easing.type: Easing.OutCubic
                        }
                    }
                }

                Behavior on x {
                    enabled: pill.placed
                    NumberAnimation {
                        duration: Theme.move
                        easing.type: Easing.BezierSpline
                        easing.bezierCurve: Theme.overshoot
                    }
                }

                onImplicitWidthChanged: Qt.callLater(deck.layout)
                Component.onCompleted: root.window.addPill(pill)
                Component.onDestruction: root.window.removePill(pill)
            }
        }
    }

    // What didn't fit, as a count.
    Pill {
        id: more

        visible: root.hidden > 0 && !root.stacking
        bubbles: []
        implicitWidth: Math.max(implicitHeight, count.implicitWidth + 16)
        y: root.atBottom ? root.height - height : 0
        attached: root.attached
        sideAttached: root.touchesSide(more)
        atBottom: root.atBottom
        atRight: root.area === "right"

        Text {
            id: count

            anchors.centerIn: parent
            text: `+${root.hidden}`
            color: Theme.muted
            font.pixelSize: Theme.textBody
            font.family: Theme.fontFamily
            font.weight: Font.DemiBold
        }

        Component.onCompleted: root.window.addPill(more)
    }

    Item {
        id: endSlot

        visible: root.hostsIsland && root.islandAtEnd && root.island.shown
        width: root.island.width
        height: root.island.height
        y: root.atBottom ? root.height - height : 0
    }
}
