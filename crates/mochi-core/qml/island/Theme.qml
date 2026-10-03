pragma Singleton

import QtQuick
import Quickshell

// Design tokens from theme.toml. The daemon sends them right after connecting;
// the fallbacks here only cover the moment before that and match the defaults
// in crates/mochi-protocol/src/theme.rs.
Singleton {
    readonly property var tokens: Daemon.theme

    readonly property color background: tokens?.colors.background ?? "#e60c0c0f"
    readonly property color surface: tokens?.colors.surface ?? "#26ffffff"
    readonly property color foreground: tokens?.colors.foreground ?? "#f5f5f7"
    readonly property color muted: tokens?.colors.muted ?? "#98989f"
    readonly property color accent: tokens?.colors.accent ?? "#ff9f0a"

    readonly property int topMargin: tokens?.layout.top_margin ?? 6
    readonly property int idleHeight: tokens?.layout.idle_height ?? 34
    readonly property int padding: tokens?.layout.padding ?? 14
    readonly property int maxRadius: tokens?.layout.max_radius ?? 26
    readonly property int surfaceHeight: tokens?.layout.surface_height ?? 640

    readonly property real spring: tokens?.motion.spring ?? 4.0
    readonly property real damping: tokens?.motion.damping ?? 0.32
    readonly property int fadeIn: tokens?.motion.fade_in_ms ?? 220
    readonly property int fadeOut: tokens?.motion.fade_out_ms ?? 120
    readonly property int fadeDelay: tokens?.motion.fade_delay_ms ?? 90
}
