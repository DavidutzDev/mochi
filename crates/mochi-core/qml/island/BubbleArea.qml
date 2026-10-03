import QtQuick
import "Lists.js" as Lists

// One of the five areas: a row of pills, left to right, with room for the
// island when it sits here. The island stays a child of the window; it is
// placed over `slot`, which takes its size, so the pills move as it grows.
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

    onPillsChanged: Lists.sync(pillModel, pills.map(pill => pill.key))
    Component.onCompleted: Lists.sync(pillModel, pills.map(pill => pill.key))

    spacing: Theme.spacing

    // In notch mode, whatever is outermost in a side area touches that side
    // of the screen too.
    function touchesSide(item: Item): real {
        if (!onSide)
            return 0;
        const outermost = area === "left" ? item.x < 0.5 : item.x + item.width > width - 0.5;
        return outermost ? attached : 0;
    }

    Item {
        id: startSlot

        visible: root.hostsIsland && !root.islandAtEnd && root.island.shown
        width: root.island.width
        height: root.island.height
        y: root.atBottom ? root.height - height : 0
    }

    Repeater {
        model: ListModel {
            id: pillModel
        }

        Pill {
            id: pill

            required property string key

            bubbles: root.pillsByKey[key] ?? []
            y: root.atBottom ? root.height - height : 0
            attached: root.attached
            sideAttached: root.touchesSide(pill)
            atBottom: root.atBottom
            atRight: root.area === "right"

            Component.onCompleted: root.window.addPill(pill)
            Component.onDestruction: root.window.removePill(pill)
        }
    }

    // What didn't fit, as a count.
    Pill {
        id: more

        visible: root.hidden > 0
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
            font.pixelSize: 13
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
