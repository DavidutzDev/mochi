import QtQuick
import qs.island

// The progress line, as the media settings draw it: wavy while the track
// plays unless `wavy` is off, in the text's color or the accent. The
// settings come in the payload's `line`; a payload without it, like the
// tour's, gets the defaults.
WavyProgress {
    property var payload: null

    playing: payload?.status === "playing"
    wavy: payload?.line?.wavy ?? true
    fill: payload?.line?.color === "accent" ? Theme.accent : Theme.foreground
}
