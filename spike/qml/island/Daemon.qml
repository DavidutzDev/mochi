pragma Singleton

import QtQuick
import Quickshell
import Quickshell.Io

// Connection to mochid. Holds the activity the island should show and sends
// UI events back. Reconnects on its own when the daemon restarts.
Singleton {
    id: root

    readonly property int api: 1
    readonly property bool connected: socket.connected

    // {id, module, view, payload}, or null before the first message.
    property var activity: null

    function event(kind: string): void {
        if (activity)
            send({ type: "event", activity: activity.id, kind: kind });
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
            console.warn(`mochi: malformed message from daemon: ${line}`);
            return;
        }

        switch (message.type) {
        case "hello":
            if (message.api !== api)
                console.warn(`mochi: daemon speaks api ${message.api}, UI speaks ${api}`);
            break;
        case "present":
            // The daemon re-sends the current activity on every reconnect.
            if (!activity || activity.id !== message.activity.id)
                activity = message.activity;
            break;
        case "error":
            console.warn(`mochi: daemon error: ${message.message}`);
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
            if (connected)
                root.send({ type: "hello", api: root.api, role: "ui" });
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
