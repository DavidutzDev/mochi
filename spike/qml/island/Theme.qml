pragma Singleton

import QtQuick
import Quickshell

Singleton {
    // Colors
    readonly property color background: "#e60c0c0f"
    readonly property color surface: "#26ffffff"
    readonly property color foreground: "#f5f5f7"
    readonly property color muted: "#98989f"
    readonly property color accent: "#ff9f0a"

    // Layout
    readonly property int topMargin: 6
    readonly property int idleHeight: 34
    readonly property int padding: 14
    readonly property int maxRadius: 26

    // The layer surface never resizes. The island animates inside it, so
    // this is the largest size any view can take.
    readonly property int surfaceHeight: 640

    // Morph springs
    readonly property real spring: 4.0
    readonly property real damping: 0.32
    readonly property int fadeIn: 220
    readonly property int fadeOut: 120
    readonly property int fadeDelay: 90
}
