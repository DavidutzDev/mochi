# Writing views

A module's views are QML files in its `qml/` directory. They import the core library with `import qs.island`, which has the theme, the built-in controls and the connection to the daemon.

## Conventions

- Every view declares `property var payload`. Island views get their activity's payload; control center cards and pages get their module's published state.
- Island views size themselves with `implicitWidth` and `implicitHeight`, and the island follows. Control center cards and pages fill the space the control center gives them.
- Colors, sizes and timings come from `Theme`, never from literals: `Theme.surface` for a card, `Theme.textBody` for text, `Theme.move` for movement. A change to `theme.toml` then restyles every module.
- Actions go through `Daemon.command(module, action, args)`; arguments are strings.
- Values that change many times a second, like the audio meters' levels, come through `Daemon`'s `live(module, value)` signal rather than the published state, from `ModuleCtx::publish_live`. Listen with `Connections { target: Daemon }`. Nothing keeps them: a view sees only those sent while it's open.

## When a view doesn't load

A view with a QML error, or a missing file, logs the error in `mochid`'s log as a `quickshell` warning, with the file and line, and the island keeps what it showed. When the view is a plugin's [override](writing-plugins.md) of a builtin view, the island and the bubbles load the builtin view instead and log `could not load root:/modules/<module>/<view>.qml, trying root:/builtin/<module>/<view>.qml`. The daemon keeps each overridden module's own files in `builtin/<module>/` in the shell tree for this.

## Overlays

An activity can bring an overlay: `ActivitySpec::overlay("Overlay")` draws `qml/Overlay.qml` over every monitor, under the island and the bubbles, while the activity shows. It's for picking something on screen, like the capture module's region. The overlay gets the activity's `payload` and a `screen` property with its monitor, and takes the clicks. Every monitor's overlay asks for the keyboard, since Hyprland only sends the pointer to surfaces that hold it; the compositor gives it to one of them, so handle keys in each. If it declares `property bool ready`, the island waits for it to turn true before it changes, so an overlay can freeze the screen first.

## Virtual screens

A module can make a monitor of its own through the compositor, named `MOCHI-<MODULE>`, like the share module's `MOCHI-SHARE`. The shell gives it no island or bubbles; it shows the module's `qml/Screen.qml` over the whole monitor instead, with the module's published state as `payload`, and takes no clicks. `Daemon.screens` lists the real monitors only, and `Daemon.virtualScreens` the others; use `Daemon.screens` wherever you list monitors for the user.

## Theme roles

| Role | For |
|---|---|
| `background` | the island and the control center panel |
| `surface` | cards and tiles |
| `raised` | controls, tracks and dividers on those |
| `highlight` | hovered controls |
| `foreground`, `muted` | main and secondary text and icons |
| `accent`, `onAccent` | what is active or important, and text on it |
| `danger`, `success` | destructive actions, good news |
| `border`, `shadow` | the hairline around the island and bubbles, and the shadow under them; the core draws both |

## Theme scales

Views take every size from `Theme`'s scales, so modules and plugins look like one design:

| Scale | Tokens | For |
|---|---|---|
| Text | `textCaption` (11), `textBody` (13), `textTitle` (15), `textHeadline` (20), `textDisplay` (42) | labels and fine print; everything else; card, track and section titles; page titles; big numbers |
| Weight | `weightBody`, `weightLabel`, `weightTitle` | running text; labels; titles |
| Spacing | `spaceTiny` (4), `spaceSmall` (8), `spaceMedium` (12), `spaceLarge` (16), `spaceHuge` (24) | inside a group of small things; between an icon and its text or items in a row; between groups; around a card's content; between sections |
| Corners | `radiusSurface`, `radiusField`, `radiusControl`, or `height / 2` for round ends | cards, tiles and panels; rows, fields and buttons; chips, badges and icon buttons |
| Height | `controlHeight` (32), `rowHeight` (44), `tileHeight` (96) | a button, chip or field; a row in a list; a row of the control center's grid |

Fonts are `fontFamily` and `displayFamily`, for clocks: Inter, which Mochi's packages bring, unless `theme.toml` names another. Motion: `fast` for colors and hovers, `move` with the `overshoot` curve for things that move. The text sizes and corners follow `theme.toml`; the rest are fixed. `textLabel` and `textSubtitle`, from before the scale, still give `textCaption` and `textBody` for now.

A test in `mochi-core` (`tests/design.rs`) reads every view and rejects new raw sizes, spacing, corners, margins and colors. A line that needs one anyway says why in a `// design:` comment, like a badge's 9 pixel count that has to fit in its dot.

## Controls

| Control | What it is | Main properties |
|---|---|---|
| `Symbol` | an icon: Mochi's name or any Material Symbols name, else the icon theme | `name`, `size`, `color`, `filled` |
| `Button` | a pill button; round with only an icon | `text`, `icon`, `tone` (`neutral`, `accent`, `danger`, `ghost`), `clicked()` |
| `IconButton` | a round icon button sized from its icon | `icon`, `size`, `tone`, `clicked()` |
| `Slider` | a value from 0 to 1; thick with an icon, or thin as a seek bar | `value`, `icon`, `thickness`, `reset` (where a double click puts it), `level` (a sound's level, drawn in the fill like a meter; off below 0), `moved(value)`, `released(value)` |
| `ProgressBar` | how far along, not interactive | `value`, `fill` |
| `Switch` | on or off | `checked`, `toggled(checked)` |
| `Segmented` | one choice out of a few | `options` (`[{value, label, icon}]`), `current`, `picked(value)` |
| `Tile` | a control-center tile, vertical for actions, horizontal for toggles | `icon`, `title`, `subtitle`, `checked`, `tone`, `vertical`, `clicked()` |
| `ListRow` | a row with something at the start and controls at the end | `icon` or `image` or `leading`, `title`, `subtitle` (`subtitleFormat: Text.StyledText` for markup, with `linkActivated(link)`), `trailing`, `selected`, `flat`, `marker`, `clicked()` |
| `Badge` | a count in an accent circle | `count` |
| `SectionLabel` | the label above a group of controls | `text` |
| `PanelHeader` | the top of a page or panel: a back button, an icon, the title, and items put inside on the right | `title`, `back`, `icon`, `iconColor`, `backClicked()` |
| `SwitchRow` | a setting that is on or off, as a row whose click flips it | `icon`, `title`, `subtitle`, `checked`, `toggled(checked)` |
| `SliderRow` | a level on a tile: an icon to click, a thin slider, the percentage and a chevron | `icon`, `value`, `maximum` (the percentage at 1), `reset`, `level`, `opens`, `moved(value)`, `released(value)`, `iconClicked()`, `opened()` |
| `RollingText` | text whose digits roll when they change, for clocks, percentages and timers | `text`, `pixelSize`, `weight`, `family`, `color`, `animated` |
| `ScrollFade` | fading edges on a list where more scrolls past them; put it inside the list | `view`, `horizontal`, `color` |
| `EdgeLight` | a light along the top edge of a panel or tile: faint, toward the pointer on hover, sweeping while `working`, and `flash()` when something finishes | `radius`, `working`, `color`, `flash()` |

Controls report what the user did and leave the state to the owner: a `Switch` sends `toggled`, and the view sets `checked` once the module confirms. `mochi ipc demo controls` shows them all.

Icons come from Material Symbols Rounded, which Mochi's packages bring. `Symbol` takes Mochi's names: home, bell, music, clock, grid, moon, search, play, pause, next, previous, note, volume, volume-0 to volume-3, volume-muted, mic, mic-muted, headset, speakers, display, caps-lock, num-lock, wifi, wifi-1, wifi-2, wifi-off, ethernet, offline, airplane, bluetooth, power, lock, logout, reboot, snow, chip, memory, gpu, disk, temperature, leaf, bolt, scale, camera, video, record, stop, region, window, copy, clipboard, palette, edit, pin, trash, folder, close, check, chevron, plus, minus, tray, dot. It also takes any [Material Symbols](https://fonts.google.com/icons) name, like `timer` or `wb_sunny`. Without the font, Mochi's names are drawn instead. Any other name, like an app's, is looked up in the icon theme. `filled` fills the outlined ones, for an active state.
