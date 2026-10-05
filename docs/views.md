# Writing views

A module's views are QML files in its `qml/` directory. They import the core library with `import qs.island`, which has the theme, the built-in controls and the connection to the daemon.

## Conventions

- Every view declares `property var payload`. Island views get their activity's payload; hub cards and pages get their module's published state.
- Island views size themselves with `implicitWidth` and `implicitHeight`, and the island follows. Hub cards and pages fill the space the hub gives them.
- Colors, sizes and timings come from `Theme`, never from literals: `Theme.surface` for a card, `Theme.textBody` for text, `Theme.move` for movement. A change to `theme.toml` then restyles every module.
- Actions go through `Daemon.command(module, action, args)`; arguments are strings.

## Overlays

An activity can bring an overlay: `ActivitySpec::overlay("Overlay")` draws `qml/Overlay.qml` over every monitor, under the island and the bubbles, while the activity shows. It's for picking something on screen, like the capture module's region. The overlay gets the activity's `payload` and a `screen` property with its monitor, and takes the clicks. Every monitor's overlay asks for the keyboard, since Hyprland only sends the pointer to surfaces that hold it; the compositor gives it to one of them, so handle keys in each. If it declares `property bool ready`, the island waits for it to turn true before it changes, so an overlay can freeze the screen first.

## Virtual screens

A module can make a monitor of its own through the compositor, named `MOCHI-<MODULE>`, like the share module's `MOCHI-SHARE`. The shell gives it no island or bubbles; it shows the module's `qml/Screen.qml` over the whole monitor instead, with the module's published state as `payload`, and takes no clicks. `Daemon.screens` lists the real monitors only, and `Daemon.virtualScreens` the others; use `Daemon.screens` wherever you list monitors for the user.

## Theme roles

| Role | For |
|---|---|
| `background` | the island and the hub panel |
| `surface` | cards and tiles |
| `raised` | controls, tracks and dividers on those |
| `highlight` | hovered controls |
| `foreground`, `muted` | main and secondary text and icons |
| `accent`, `onAccent` | what is active or important, and text on it |
| `danger`, `success` | destructive actions, good news |
| `border`, `shadow` | the hairline around the island and bubbles, and the shadow under them; the core draws both |

Text sizes: `textCaption`, `textLabel`, `textBody`, `textSubtitle`, `textTitle`, `textHeadline`, `textDisplay`, with `fontFamily`. Corners: `radiusSmall`, `radiusMedium`, `radiusLarge`. Motion: `fast` for colors and hovers, `move` with the `overshoot` curve for things that move.

## Controls

| Control | What it is | Main properties |
|---|---|---|
| `Symbol` | an icon from the built-in set, or the icon theme | `name`, `size`, `color` |
| `Button` | a pill button; round with only an icon | `text`, `icon`, `tone` (`neutral`, `accent`, `danger`, `ghost`), `clicked()` |
| `IconButton` | a round icon button sized from its icon | `icon`, `size`, `tone`, `clicked()` |
| `Slider` | a value from 0 to 1; thick with an icon, or thin as a seek bar | `value`, `icon`, `thickness`, `moved(value)`, `released(value)` |
| `ProgressBar` | how far along, not interactive | `value`, `fill` |
| `Switch` | on or off | `checked`, `toggled(checked)` |
| `Segmented` | one choice out of a few | `options` (`[{value, label, icon}]`), `current`, `picked(value)` |
| `Tile` | a control-center tile, vertical for actions, horizontal for toggles | `icon`, `title`, `subtitle`, `checked`, `tone`, `vertical`, `clicked()` |
| `ListRow` | a row with something at the start and controls at the end | `icon` or `image` or `leading`, `title`, `subtitle`, `trailing`, `selected`, `flat`, `marker`, `clicked()` |
| `Badge` | a count in an accent circle | `count` |
| `SectionLabel` | the label above a group of controls | `text` |

Controls report what the user did and leave the state to the owner: a `Switch` sends `toggled`, and the view sets `checked` once the module confirms. `mochi ipc demo controls` shows them all.

The built-in icons: home, bell, music, clock, grid, moon, search, play, pause, next, previous, note, volume, volume-0 to volume-3, volume-muted, mic, mic-muted, headset, speakers, display, caps-lock, num-lock, wifi, wifi-1, wifi-2, wifi-off, ethernet, offline, airplane, bluetooth, power, lock, logout, reboot, snow, chip, memory, gpu, temperature, leaf, bolt, scale, clipboard, close, check, chevron, plus, minus, tray, dot. Any other name is looked up in the icon theme.
