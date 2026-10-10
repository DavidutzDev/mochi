import QtQuick

// Compiles the views modules offer as `preload`, like the settings panel
// and the control center, a few seconds after the shell starts, one at a
// time and off the UI thread. Opening one the first time then only builds
// it, instead of compiling a few thousand lines of QML before the island
// can move. A preload's `offered` option names kinds of what other modules
// offer its module, like the control center's cards, whose views compile
// with it. The compiled views stay in memory while the shell runs.
QtObject {
    id: root

    // The views to compile, by URL.
    readonly property var wanted: {
        const urls = [];
        const add = (module, view) => {
            if (view !== "")
                urls.push(`root:/modules/${module}/${view}.qml`);
        };
        for (const entry of Daemon.offered("mochi", "preload")) {
            add(entry.module, entry.view);
            for (const kind of entry.options?.offered ?? [])
                for (const offer of Daemon.offered(entry.module, kind))
                    add(offer.module, offer.view);
        }
        return [...new Set(urls)];
    }

    // What was compiled, by URL, kept so the engine keeps it. Changed in
    // place: nothing binds to it.
    property var compiled: ({})
    property bool busy: false

    onWantedChanged: root.wait.restart()

    // After the views the shell starts with have loaded.
    property Timer wait: Timer {
        interval: 3000
        running: Daemon.ready
        onTriggered: root.next()
    }

    function next(): void {
        if (root.busy)
            return;
        const url = root.wanted.find(url => root.compiled[url] === undefined);
        if (url === undefined)
            return;
        const component = Qt.createComponent(url, Component.Asynchronous);
        root.compiled[url] = component;
        if (component.status !== Component.Loading) {
            root.done(url, component);
            return;
        }
        root.busy = true;
        component.statusChanged.connect(() => {
            if (component.status === Component.Loading)
                return;
            root.busy = false;
            root.done(url, component);
        });
    }

    function done(url: string, component: var): void {
        if (component.status === Component.Error)
            console.warn(`mochi: could not preload ${url}: ${component.errorString()}`);
        Qt.callLater(root.next);
    }
}
