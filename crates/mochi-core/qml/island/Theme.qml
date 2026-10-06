pragma Singleton

import QtQuick
import Quickshell

// Design tokens from theme.toml. The daemon sends them right after connecting;
// the fallbacks here only cover the moment before that and match the defaults
// in crates/mochi-protocol/src/theme.rs.
//
// Views use roles, never raw colors or sizes: the island is `background`,
// cards sit on it in `surface`, controls on those in `raised`, and hovered
// controls in `highlight`. `accent` marks what is active or important.
Singleton {
    readonly property var tokens: Daemon.theme

    readonly property color background: tokens?.colors.background ?? "#f5000000"
    readonly property color surface: tokens?.colors.surface ?? "#1c1c1e"
    readonly property color raised: tokens?.colors.raised ?? "#2c2c2e"
    readonly property color highlight: tokens?.colors.highlight ?? "#3a3a3c"
    readonly property color foreground: tokens?.colors.foreground ?? "#ffffff"
    readonly property color muted: tokens?.colors.muted ?? "#8e8e93"
    readonly property color accent: tokens?.colors.accent ?? "#ff9f0a"
    readonly property color onAccent: tokens?.colors.on_accent ?? "#000000"
    readonly property color danger: tokens?.colors.danger ?? "#ff453a"
    readonly property color success: tokens?.colors.success ?? "#30d158"
    readonly property color border: tokens?.colors.border ?? "#14ffffff"
    readonly property color shadow: tokens?.colors.shadow ?? "#59000000"

    // "island" or "notch".
    readonly property string mode: tokens?.layout.mode ?? "island"
    // "top" or "bottom".
    readonly property string anchor: tokens?.layout.anchor ?? "top"
    // The area the island sits in: left, center-left, center, center-right
    // or right.
    readonly property string islandArea: tokens?.layout.island ?? "center"
    readonly property int margin: tokens?.layout.margin ?? 6
    readonly property int spacing: tokens?.layout.spacing ?? 8
    readonly property int earRadius: tokens?.layout.notch?.ear_radius ?? 10
    readonly property int idleHeight: tokens?.layout.idle_height ?? 34
    readonly property int padding: tokens?.layout.padding ?? 14
    readonly property int maxRadius: tokens?.layout.max_radius ?? 34
    readonly property int radiusSmall: tokens?.layout.radius_small ?? 8
    readonly property int radiusMedium: tokens?.layout.radius_medium ?? 14
    readonly property int radiusLarge: tokens?.layout.radius_large ?? 20
    readonly property int surfaceHeight: tokens?.layout.surface_height ?? 640

    readonly property real spring: tokens?.motion.spring ?? 4.0
    readonly property real damping: tokens?.motion.damping ?? 0.32
    readonly property int fadeIn: tokens?.motion.fade_in_ms ?? 220
    readonly property int fadeOut: tokens?.motion.fade_out_ms ?? 120
    readonly property int fadeDelay: tokens?.motion.fade_delay_ms ?? 90
    readonly property int fast: tokens?.motion.fast_ms ?? 150
    readonly property int move: tokens?.motion.move_ms ?? 350
    // Material 3's expressive spatial curve: a slight overshoot, for things
    // that move or resize. Use with `easing.type: Easing.BezierSpline`.
    readonly property var overshoot: [0.38, 1.21, 0.22, 1.0, 1, 1]

    readonly property string fontFamily: tokens?.text.family || Qt.application.font.family
    // The clocks.
    readonly property string displayFamily: tokens?.text.display_family || fontFamily
    readonly property int textCaption: tokens?.text.caption ?? 11
    readonly property int textLabel: tokens?.text.label ?? 12
    readonly property int textBody: tokens?.text.body ?? 13
    readonly property int textSubtitle: tokens?.text.subtitle ?? 14
    readonly property int textTitle: tokens?.text.title ?? 16
    readonly property int textHeadline: tokens?.text.headline ?? 20
    readonly property int textDisplay: tokens?.text.display ?? 42
}
