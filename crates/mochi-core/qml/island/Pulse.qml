pragma Singleton

import QtQuick
import Quickshell

// The one clock every Ticker follows: 30 times a second while one of them
// runs, and never otherwise. Shared, so a wave and the bars beside it move
// in the same frame rather than each drawing frames of its own.
Singleton {
    id: root

    // How many Tickers run now.
    property int wanted: 0
    // The time, in milliseconds, at the last beat.
    property real now: 0

    Timer {
        interval: 33
        repeat: true
        triggeredOnStart: true
        running: root.wanted > 0
        onTriggered: root.now = Date.now()
    }
}
