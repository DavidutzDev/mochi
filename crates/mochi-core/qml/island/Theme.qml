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
// Sizes come from the scales below: four text sizes and one for big
// numbers, five spacings, three radii and three heights. A check in the
// test suite rejects raw ones in module views.
Singleton {
    readonly property var tokens: Daemon.theme

    // The fonts Mochi's packages bring, linked into the shell's `fonts`
    // directory by mochid. Without them, Inter from the system, or the
    // system font, and the drawn symbols.
    readonly property FontLoader inter: FontLoader {
        source: "root:/fonts/InterVariable.ttf"
    }
    readonly property FontLoader symbols: FontLoader {
        source: "root:/fonts/MaterialSymbolsRounded.ttf"
    }
    readonly property bool hasSymbols: symbols.status === FontLoader.Ready
    readonly property string symbolFamily: hasSymbols ? symbols.name : ""

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
    readonly property color border: tokens?.colors.border ?? "#0dffffff"
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

    // The radii by what they round: cards, tiles and panels; rows, fields
    // and buttons; chips, badges and icon buttons. Round ends use
    // `height / 2`.
    readonly property int radiusSurface: radiusLarge
    readonly property int radiusField: radiusMedium
    readonly property int radiusControl: radiusSmall

    // The spacing scale: inside a group of small things, between an icon
    // and its text or items in a row, between groups, around a card's
    // content, and between sections.
    readonly property int spaceTiny: 4
    readonly property int spaceSmall: 8
    readonly property int spaceMedium: 12
    readonly property int spaceLarge: 16
    readonly property int spaceHuge: 24

    // Heights: a button, chip or field; a row in a list; one row of the
    // hub's grid of cards.
    readonly property int controlHeight: 32
    readonly property int rowHeight: 44
    readonly property int tileHeight: 96

    // Motion. `reduced` turns animations off: every duration is 0, and the
    // spring settles at once without a bounce. `speed` scales the rest:
    // durations divide by it, and the spring's stiffness grows with its
    // square, so the island settles that much faster too.
    readonly property bool reducedMotion: tokens?.motion.reduced ?? false
    readonly property real speed: Math.max(tokens?.motion.speed ?? 1, 0.05)
    function duration(ms: real): int {
        return reducedMotion ? 0 : Math.round(ms / speed);
    }
    readonly property real spring: reducedMotion ? 100 : (tokens?.motion.spring ?? 4.0) * speed * speed
    readonly property real damping: reducedMotion ? 1 : tokens?.motion.damping ?? 0.32
    readonly property int fadeIn: duration(tokens?.motion.fade_in_ms ?? 220)
    readonly property int fadeOut: duration(tokens?.motion.fade_out_ms ?? 120)
    readonly property int fadeDelay: duration(tokens?.motion.fade_delay_ms ?? 90)
    readonly property int fast: duration(tokens?.motion.fast_ms ?? 150)
    readonly property int move: duration(tokens?.motion.move_ms ?? 350)
    // Material 3's expressive spatial curve: a slight overshoot, for things
    // that move or resize. Use with `easing.type: Easing.BezierSpline`.
    // Without motion, no overshoot either.
    readonly property var overshoot: reducedMotion ? [0.2, 0, 0, 1, 1, 1] : [0.38, 1.21, 0.22, 1.0, 1, 1]

    readonly property string fontFamily: tokens?.text.family || (inter.status === FontLoader.Ready ? inter.name : "Inter")
    // The clocks.
    readonly property string displayFamily: tokens?.text.display_family || fontFamily
    // The type scale: labels and fine print, everything else, titles, page
    // titles, and big numbers.
    readonly property int textCaption: tokens?.text.caption ?? 11
    readonly property int textBody: tokens?.text.body ?? 13
    readonly property int textTitle: tokens?.text.title ?? 15
    readonly property int textHeadline: tokens?.text.headline ?? 20
    readonly property int textDisplay: tokens?.text.display ?? 42
    // Weights: running text, labels, titles.
    readonly property int weightBody: Font.Normal
    readonly property int weightLabel: Font.Medium
    readonly property int weightTitle: Font.DemiBold

    // Before the scale had four sizes. Views and plugins still using them
    // get the nearest size; they go in a later release.
    readonly property int textLabel: textCaption
    readonly property int textSubtitle: textBody
}
