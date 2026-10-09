pragma Singleton

import QtQuick
import Quickshell
import Quickshell.Io

// The connection to mochid. Everything the UI shows comes from here, and the
// daemon sends all of it again after every reconnect, so nothing in QML has to
// survive a restart. See docs/protocol.md.
Singleton {
    id: root

    readonly property int api: 1

    // True once the daemon answered our hello.
    property bool ready: false

    // The real monitors. Mochi makes monitors of its own, named MOCHI-<module>,
    // like the share module's switchable screen; they get no island.
    readonly property var screens: Quickshell.screens.filter(screen => !isVirtual(screen))
    readonly property var virtualScreens: Quickshell.screens.filter(isVirtual)

    function isVirtual(screen: var): bool {
        return (screen?.name ?? "").startsWith("MOCHI-");
    }

    // Enabled module ids.
    property var modules: []

    // What modules offer each other: [{module, target, kind, id, view,
    // title, icon, order, options}].
    property var contributions: []

    // The contributions for one module, of one kind, in order.
    function offered(target: string, kind: string): var {
        return contributions.filter(entry => entry.target === target && entry.kind === kind).sort((a, b) => a.order - b.order);
    }

    // Latest published state per module id. Replaced, never mutated, so
    // bindings on it update.
    property var states: ({})

    // Each monitor's island shows an activity of its own: {id, module,
    // view, payload, expanded, expandable} or null, by monitor name. The
    // empty name stands for every monitor, before the compositor names
    // them. Replaced, never mutated, so bindings on it update.
    property var activities: ({})

    // Where a view's own events go: the island holding the keyboard, for a
    // panel, or else the one under the pointer, by monitor; null for none.
    property var focusedOutput: null
    property var pointedOutput: null

    function activityFor(output: string): var {
        return activities[output] ?? activities[""] ?? null;
    }

    // Every bubble in drawing order: [{id, module, key, view, payload, area,
    // group}]. Consecutive bubbles with the same area and group share a pill.
    property var bubbles: []
    // [{area, hidden}] for areas with more bubbles than fit.
    property var overflow: []
    // {news_ms} when each area stacks its bubbles, or null.
    property var bubbleStack: null
    // How long the pointer rests on a bubble before its tooltip shows; 0
    // for never.
    property int bubbleTooltipMs: 0

    // Design tokens from theme.toml, or null until the daemon sends them.
    property var theme: null

    // A passing value from a module, like the audio meters' levels, many
    // times a second. Not kept: views that want it listen for it.
    signal live(string module, var value)

    function state(module: string): var {
        return states[module] ?? null;
    }

    // Reports something that happened to the activity a view belongs to:
    // the panel with the keyboard, or the one under the pointer.
    function event(kind: string): void {
        const output = focusedOutput ?? pointedOutput;
        if (output !== null) {
            eventFor(activityFor(output), kind, output);
            return;
        }
        const any = Object.keys(activities).map(output => ({
                    "activity": activities[output],
                    "output": output
                })).find(entry => entry.activity !== null);
        if (any)
            eventFor(any.activity, kind, any.output);
    }

    // Reports something that happened to one activity on the island of
    // `output`, which the daemon ignores unless it's still shown there.
    function eventFor(activity: var, kind: string, output: string): void {
        if (!activity)
            return;
        const message = {
            "type": "event",
            "activity": activity.id,
            "kind": kind
        };
        if (output)
            message.output = output;
        send(message);
    }

    // A click outside the island closed `activity` on the island of
    // `output`; the daemon passes the click on to the window under it.
    // `click` is {output, x, y, width, height, button}.
    function outsideClick(activity: var, output: string, click: var): void {
        if (!activity)
            return;
        send({
            "type": "event",
            "activity": activity.id,
            "kind": "outside",
            "output": output,
            "click": click
        });
    }

    // A scroll the island caught that belongs to the window under it,
    // without closing anything; the daemon scrolls there once the island
    // lets go. `click` is as in outsideClick, with `scroll` {x, y}.
    function passOn(click: var): void {
        send({
            "type": "pass_on",
            "click": click
        });
    }

    function bubbleClick(id: int): void {
        send({
            type: "bubble_click",
            bubble: id
        });
    }

    // A click on an area's "+N": the daemon lists its hidden bubbles.
    function overflowClick(area: string): void {
        send({
            type: "overflow_click",
            area: area
        });
    }

    // Runs a module action, for example from a button in a view.
    function command(module: string, action: string, args: var): void {
        send({
            type: "command",
            module: module,
            action: action,
            args: args ?? []
        });
    }

    function send(message: var): void {
        if (!socket.connected)
            return;
        socket.write(JSON.stringify(message) + "\n");
        socket.flush();
    }

    function handle(line: string): void {
        let message;
        try {
            message = JSON.parse(line);
        } catch (error) {
            console.warn(`mochi: malformed message from the daemon: ${line}`);
            return;
        }

        switch (message.type) {
        case "hello":
            if (message.api !== api)
                console.warn(`mochi: the daemon speaks api ${message.api}, the UI speaks ${api}`);
            ready = true;
            break;
        // New modules: their views load in place, the windows stay, and
        // this connection starts over with the new generation.
        case "reload_views":
            Quickshell.reload(false);
            break;
        case "modules":
            modules = message.modules;
            break;
        case "contributions":
            contributions = message.contributions;
            break;
        case "state":
            {
                const next = Object.assign({}, states);
                next[message.module] = message.state;
                states = next;
                break;
            }
        case "live":
            live(message.module, message.value);
            break;
        case "present":
            {
                // The monitor's island, or every one's before they have names.
                const next = Object.assign({}, activities);
                next[message.output ?? ""] = message.activity;
                activities = next;
                break;
            }
        case "bubbles":
            bubbles = message.bubbles;
            overflow = message.overflow ?? [];
            bubbleStack = message.stack ?? null;
            bubbleTooltipMs = message.tooltip_ms ?? 0;
            break;
        case "theme":
            theme = message.theme;
            break;
        // Answers to commands views send; views don't wait for them.
        case "ok":
        case "output":
            break;
        case "error":
            console.warn(`mochi: ${message.code}: ${message.message}`);
            break;
        default:
            console.warn(`mochi: unknown message type ${message.type}`);
        }
    }

    Socket {
        id: socket

        path: Quickshell.env("MOCHI_SOCKET") ?? ""
        connected: true

        onConnectionStateChanged: {
            if (connected) {
                root.send({
                    type: "hello",
                    api: root.api,
                    role: "ui"
                });
            } else {
                root.ready = false;
            }
        }

        parser: SplitParser {
            onRead: line => root.handle(line)
        }
    }

    // A failed connect leaves the socket idle, so keep asking until it works.
    Timer {
        interval: 500
        repeat: true
        running: !socket.connected
        onTriggered: socket.connected = true
    }
}
