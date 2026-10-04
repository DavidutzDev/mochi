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

    // {id, module, view, payload, expanded, expandable}, or null.
    property var activity: null

    // Every bubble in drawing order: [{id, module, key, view, payload, area,
    // group}]. Consecutive bubbles with the same area and group share a pill.
    property var bubbles: []
    // [{area, hidden}] for areas with more bubbles than fit.
    property var overflow: []

    // Design tokens from theme.toml, or null until the daemon sends them.
    property var theme: null

    function state(module: string): var {
        return states[module] ?? null;
    }

    // Reports something that happened to the shown activity.
    function event(kind: string): void {
        if (activity)
            send({ type: "event", activity: activity.id, kind: kind });
    }

    // Reports something that happened to one activity, which the daemon
    // ignores unless it's still the shown one.
    function eventFor(activity: var, kind: string): void {
        if (activity)
            send({ type: "event", activity: activity.id, kind: kind });
    }

    function bubbleClick(id: int): void {
        send({ type: "bubble_click", bubble: id });
    }

    // Runs a module action, for example from a button in a view.
    function command(module: string, action: string, args: var): void {
        send({ type: "command", module: module, action: action, args: args ?? [] });
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
        case "modules":
            modules = message.modules;
            break;
        case "contributions":
            contributions = message.contributions;
            break;
        case "state": {
            const next = Object.assign({}, states);
            next[message.module] = message.state;
            states = next;
            break;
        }
        case "present":
            activity = message.activity;
            break;
        case "bubbles":
            bubbles = message.bubbles;
            overflow = message.overflow ?? [];
            break;
        case "theme":
            theme = message.theme;
            break;
        case "ok":
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
                root.send({ type: "hello", api: root.api, role: "ui" });
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
