# Changelog

Mochi follows [semantic versioning](https://semver.org). Before 1.0, any minor release may change the config format, the protocol or the module interface; the changelog says when.

## Unreleased

### Fixed

- A screenshot or recording region nearly as tall as the screen put its size and Capture button off the screen or under the island, out of reach. They go in a line under the region or over it, or turned upright right of it or left of it, reading down its side, where they first fit, and inside it near its bottom when the region leaves no room, never under the island.

## 0.1.0 - 2026-10-10

### Added

- Screenshots and recordings on the control center's Captures page, and images on its Clipboard page, drag onto any app as files, as from a file manager. The panel closes as the drag starts, so the apps under it take the drop, and the drag goes on without it. Clipboard images are written with their type's extension, like `12.png`, so the app takes them for pictures.
- Shift+click or Ctrl+click selects several captures, or several clipboard images, and dragging one of them drops them all, with their number on the drag. The launcher's file results drag out too, and a folder button on the selected one, like Shift+Enter, shows it selected in the file manager through `org.freedesktop.FileManager1`, or opens its folder. Script and plugin providers can give a result a `file` the same way. The Captures page's folder button selects the capture as well.
- The clock's World tab adds any time zone the system has, with a search by city, country or offset and the keyboard; common cities come first. The settings and the widget editor use the same picker.
- File settings, like the timer's alarm sound and the wallpaper, have a Choose button that opens the desktop's file chooser through the XDG portal, or zenity or kdialog.
- The clock panel's Timer tab has the alarm: its volume, its sound with Choose and Default, and Play it.
- Custom timers beside the focus timer: several at once, each with a label, typed like `15m Tea`, `90s` or `1h 30m`, with pause, stop and "+1 min"; a bubble each, or one for the soonest (`timer_bubbles`). When one runs out, the island says so with Again and "+1 min", the alarm plays (`sound`, `sound_file`, `volume`, `sound_command`) and the music pauses (`pause_media`). The clock panel's Timer tab shows them and starts new ones.
- `:t 10m Pizza` in the launcher starts a timer; `:t sw` starts or pauses the stopwatch.
- A time zone picker: the clock's `zones`, and the clock widgets' `timezone` and `zones`, list the system's zones with their offset now and a search.
- The clock's Today tab can show the seconds and how far through the day it is, as a line or a ring, and its clock can sit in another shape or none.
- The stopwatch counts in tenths or hundredths, keeps each run that Reset ends, and copies a run with its laps to the clipboard.
- The clock's World tab adds common cities with a click.
- New shapes for clocks and widgets: hexagon, octagon, squircle and pill. The clock widget's looks pick one in their settings.
- The control center's navbar pages can be reordered and hidden: the pencil drags them along the navbar and takes them out, with `pages` and `hidden_pages` settings and `mochi ipc control-center arrange-pages`. A hidden page still opens by name and from its card. `mochi ipc control-center edit` opens it arranging.
- A Weather page in the control center: the weather now with feels like, humidity, wind, UV index, sunrise and sunset, the next 7 days and the next 24 hours as a curve; and a one-column "Weather now" card under More cards.
- A Clock page in the control center with the panel's Today, Calendar, Stopwatch and World, and a Clock card under More cards.
- `[motion] waves = false` in `theme.toml` draws every progress line and ring flat. The media line, the battery ring and the performance rings also have a `wavy` setting of their own.
- Timer and clock: `notice_seconds` sets how long the end of a timer and a due reminder stay on the island. Weather: `prompt = false` keeps the control center's weather cards hidden until a place is set, instead of saying how to set one.
- Media: `line_color` draws the progress line in the accent, and `card_seeks` lets the Now Playing card's line seek. Weather: `stale_minutes` sets how old the forecast gets before the looks say so, never less than twice `refresh_minutes`.
- The clock and the timer credit [mochi-clock](https://github.com/Xonex5/mochi-clock), by Xonex5, whose ideas their stopwatch, custom timers, alarm and `:t` follow, at the bottom of their settings pages, with a link to its repository.
- Widgets can have several looks: modules declare variants with a title, a one-line description and their own sizes, and the widget's settings switch between them. A look can go without its card.
- The widget drawer is a panel at the side while arranging: search, category chips, and tabs for Add (a live preview of every look, with how many are placed), On desktop and Layouts. Click a widget to place it in the first free spot, clear of the drawer and the island.
- Saved widget layouts: keep an arrangement under a name and switch between them, in `widget-layouts/` next to `widgets.toml`.
- New widget looks: the clock stacked, analog, in a cookie, minimal without a card, and as a world clock; the calendar's week strip, with today as a pentagon; performance as rings or meters; the battery as a ring; now playing as a tall artwork card or the cover alone.
- New widgets: system info (distribution, kernel, uptime, Mochi's version, compositor, CPU and memory) and the focus timer.
- A weather module: the weather now, the next 24 hours and 7 days from Open-Meteo, with no account or key. Set `place` (looked up once) or `latitude` and `longitude`; nothing is sent before. The control center gets a card with the next hours, and the desktop a Weather widget in four looks: now, an icon in a cookie, the next days, and the next 12 hours as a curve. `mochi ipc weather refresh` and `status`; settings apply live.
- A clock panel on the island, `mochi ipc clock toggle`: today with the weather and the next reminder, a calendar with reminders, the focus timer, a stopwatch with laps, and world clocks. A click on the control center's Today card opens it.
- Reminders, from the calendar or `mochi ipc clock remind`: the island says when one is due, with Done and Snooze, also after a suspend or a restart.
- Shared pieces for views in the core: `ExpressiveShape` (circle, pentagon, cookie, clover, burst), `WavyRing`, `WavyProgress`, `StackedTime` and `ClockTime`; see docs/views.md.
- A focus timer, the `timer` module: a bubble by the island counts down a focus session, and a click pauses it. When focus ends the island offers a break, every fourth one long, and when the break ends, the next session. The control center has a card with the time left and buttons. `mochi ipc timer start`, `break`, `pause`, `resume`, `toggle`, `stop` and `status` drive it. It carries on through a mochid restart, and its settings apply without stopping it.
- Notifications play the sound an app asks for (`sound-file`, `sound-name`) when they pop up, through pw-play or paplay, and canberra-gtk-play or the sound theme for names. `suppress-sound`, do not disturb and `sounds = false` keep them quiet, and `sound_command` picks the player.
- Players that report a volume of their own, like Spotify, get a volume slider on the island and on the Now Playing card, and `mochi ipc media volume <level>` sets it: `40`, `+5` or `-5`.
- The mixer lists the apps recording from a microphone under Recording, each with a volume and a mute. `recording:` before an app's name or a stream's id names them in `volume` and `mute`, like `mochi ipc audio mute recording:discord`.
- The keyboard's backlight in the brightness module: its own slider on the control center's card, the OSD when a key changes it, and `mochi ipc brightness up keyboard`, `down keyboard` and `set <level> keyboard`, which move it at least one level. `all` still means only the displays.
- The battery card lists each battery of a laptop with two, and every mouse, keyboard, controller, headset or other device that reports its battery to UPower, red when low. One dropping to `[module.battery] peripherals`, 15% by default, shows a notice once until it charges. `mochi ipc battery status` prints them all.
- The OSD shows the keyboard layout when it switches, on Hyprland, niri and Sway; `layout = false` turns it off. Views read the current layout from the OSD's state, plugins from the compositor state, and `mochi status` shows it.
- A palette widget from the colors module: the latest colors as swatches on the desktop; click one to copy it, hover to see it.
- A widget's settings can send it to another monitor, keeping its anchor and offsets, moved in to stay on screen; `widgets edit` takes a monitor and a widget to open.
- `layout = "compact"` in the launcher: one line per result, the description only on the selected one.
- `git-release:` sources install from Forgejo, Gitea (Codeberg) and GitLab, including GitLab subgroups; another host's forge is found through its API.
- Plugins can come from a tar archive over HTTPS with its hash: `https://…/plugin.tar.gz#sha256=<hex>`. A download with another hash is refused.
- The plugin release workflow builds Rust backends as static musl binaries, so `git-release:` plugins run on NixOS without nix-ld.
- `docs/architecture.md`, a map of the code for contributors.

### Changed

- Lists scroll as in a browser: a mouse wheel's step glides instead of jumping, quick steps add up, and a touchpad's scroll coasts on and slows down after the fingers lift, in the settings panel, the control center, the launcher, the pickers and every other list. A step goes 120 pixels; the theme's `[motion] scroll_step` changes it, and `smooth_scroll = false` goes back to Qt's own scrolling.
- The control center's Today card is the clock module's now: it shows with the clock module on, and its heading opens the Clock page in the control center instead of the separate clock panel. Without the clock module the control center has no time card. `control-center/clock` in its `order` or `hidden` becomes `clock/clock`.
- The media progress line waves while a track plays and lies flat when paused, in the island and on the Now Playing card.
- The example weather plugin is now `meteo`, since `weather` is a builtin id; Bento leaves the weather's `place` out of shared setups.
- The mixer's app icons come from the apps' desktop entries, found by the stream's Flatpak id, app id, icon, program or name, so Zen, Chromium and Discord calls show their icons.
- An open tray menu follows the app's changes (`LayoutUpdated`, `ItemsPropertiesUpdated`): checkmarks, labels and new entries update in place.
- The clipboard history works on compositors without `ext-data-control-v1`, through `wlr-data-control`; `MOCHI_DATA_CONTROL=wlr` forces it.
- curl errors from `mochi plugins` name the URL.

### Fixed

- A file search in the launcher said "No results" while the file index was still being built, as it is for a while after mochid starts. It says "Searching", with dots coming one by one, until the index is ready and the results come, and "No results" only after.
- Opening the launcher could crash Quickshell: app icons loaded on Qt's image thread, and Qt's icon themes break when two threads read them at once. Icons from the theme load on the UI thread now, and pictures from files still in the background. File results in the launcher show a document symbol instead of an empty tile.
- Night light said Unavailable while sharing a screen switchably: the share module's virtual screen has no gamma, and its refusal counted as night light failing. Virtual screens are left out now, and a real screen without gamma no longer stops the others from warming.
- The settings panel's segmented choices fit long labels.
- The world clock found no time zones on NixOS without `TZDIR` set; it reads the zone files' full paths now.
- Turning a module on no longer restarts Quickshell: the UI reloads its views in place and keeps its windows, so the island, the bubbles and the widgets stay on screen. Quickshell also stops reloading by itself while mochid writes the views, which it only does now for `mochid --dev`. A UI that doesn't come back from the reload still gets a fresh Quickshell.
- The installer restarts the running mochid only when it's the one it just updated. Installing into another prefix used to restart a mochid from a package or Nix too.
- The settings panel opens at once: a page's options build over a few frames, behind placeholder rows that show if it takes a moment, instead of all at once before the panel appears. Bento, the About page, the Modules list and the time zone picker build only when they show.
- Panels open faster the first time: the UI compiles the settings panel, the control center with its cards and pages, the launcher, the clock, the mixer, the clipboard and emoji pickers and the tray panel a few seconds after it starts, in the background. The first settings panel took about 70 ms before it showed, and takes 20 now. Modules and plugins ask for theirs with a `preload` contribution to `mochi`.
- The first control center and settings panel froze the island for a quarter to three quarters of a second on systems with many data directories, like NixOS: each symbol asked the icon theme for its name while it was being made, before it knew the icon font draws it, and each of those lookups read every icon directory. Symbols ask the theme only once they know the font has no glyph.
- The performance widget made its rows again, graphs and all, with every reading: a 30 to 45 ms stall every two seconds on every screen. Its looks now update their rows in place, and the numbers roll as they change.
- Without a `modules` list in `config.toml`, the settings panel showed only Idle as on, though every default module runs, and turning one on or off there would have written a list with Idle alone.

## 0.0.9 - 2026-10-09

### Added

- An updater: Mochi looks for a new release on GitHub at start and every 12 hours and says so once on the island. Its page in the settings has the changelog of each newer release and the update: the install script runs again and Mochi restarts, and a package manager or Nix gets its command, to copy or run in a terminal. `[module.updater] check = false` turns it off, and `command` sets your own.
- A module can draw on its own settings page, above its options, with a `section` contribution to `settings`.
- An About page at the bottom of the settings: the version and build, Quickshell, the system, the session, the modules and plugins, and the memory and uptime of mochid and what it started. "Copy details" copies it for a bug report, and buttons open the GitHub page, the documentation and a new issue.
- A universal installer: `curl -fsSL https://raw.githubusercontent.com/DavidutzDev/mochi/main/install.sh | sh` installs the latest release into `~/.local`, checked against its sum, and updates it when run again, restarting a running mochid. `--prefix`, `--version`, `--enable` and `--uninstall` change what it does, and `MOCHI_FROM_SOURCE=1` builds the release with cargo instead, or `main` with `MOCHI_FROM_SOURCE=main`. Mochi installed by Nix, pacman, apt or rpm is left to them.
- Dev mode is the default of a debug build, so `cargo run -p mochid` is enough, and release builds refuse `--dev`. `--no-dev` runs a debug build as an installed shell runs. A dev daemon takes over from the shell already running, which stops its island and modules, waits, and starts again by itself when the dev daemon stops. A bubble on the left says the dev shell runs, with its revision and the shell it took over from. The running shell has to be this version or newer to step aside.

### Changed

- The hub is now the control center, with the module id `control-center`: `mochi ipc control-center toggle`, `[module.control-center]`, `target = "control-center"` in plugin manifests. `hub` still works everywhere, with a warning in the log.
- Bento comes before Plugins in the settings.
- One Mochi per Wayland session: a second `mochid` refuses to start, also with another `--runtime-dir`, instead of drawing a second island over the first.
- A preset picked in the settings panel replaces the colors `theme.toml`'s `[colors]` sets, and the ones picked in the panel before it. Picking one used to change nothing when the file set every color. Copy then gives the preset without those colors.

## 0.0.8 - 2026-10-09

### Added

- Releases with Mochi already built: pushing a version tag builds `mochi` and `mochid` for x86_64 and aarch64 Linux and publishes them on GitHub, with the fonts, the systemd unit, the link handler, shell completions and `install.sh`, which installs them into `~/.local` or a prefix. The flake gets `mochi-bin`, the latest release patched for Nix, and Arch gets `packaging/arch/mochi-bin`, so neither compiles anything. `RELEASING.md` has the steps.
- mochid finds its fonts in `share/mochi/fonts` next to its binary's directory too, as a release unpacked into a prefix has them.
- Themes are packages: a `mochi-theme.toml` gives a color for every role in a dark and a light version, and can set fonts, the island's shape and motion with the `[text]`, `[layout]` and `[motion]` of `theme.toml`, which your own file goes over, and `preset = "<id>"` picks one installed in `~/.local/share/mochi/themes/<id>/` as well as the ones Mochi brings, which are now themes too. The settings panel lists them all with their colors. A theme names the oldest Mochi it works with, and a newer one is refused with a message saying so. This is the first part of Bento, for sharing themes, plugins and whole configs.
- Bento, `mochi bento`: share a whole setup as a bento, a directory with a `mochi-bento.toml` holding the settings, the theme, the widgets and the plugins they need. `share` makes one from the setup running now, leaving out this machine's devices, where you are, secrets and paths in the home directory, and listing them; widgets go on screens named by size, `screen-1` being the largest. `add` installs a bento, a theme or a plugin from a directory, a git repository or a gist: a bento's plugins each ask first, its settings go into `changes.toml`, its widgets replace `widgets.toml`, and its wallpaper is set with awww or swww. `try` applies a theme or a bento's look until Keep or Drop, `remove` puts back what a bento replaced, `list` shows what Bento installed and `check` reads a bento as `add` would. What Bento installed is in `bento.toml`, whose plugins count as if `plugins.toml` listed them, and Copy as Nix includes them. The settings module gains a `try` action for whole tables.
- Bento's registry: `mochi bento search`, `info` and `add <id>` install plugins, themes and bentos from an index of releases pinned to reviewed commits, picking the newest one the running Mochi supports. `bento:<id>` sources work in `plugins.toml` and in bentos, `mochi bento update` moves to newer releases, and `[registries]` in `bento.toml` adds other registries. A release the registry withdraws is no longer installed and is flagged where it is; one withdrawn as harmful is refused, and mochid doesn't start it. `mochid bento registry check`, `index` and `diff` are the registry's CI, and `examples/bento-registry` is a template for one.
- Bento in the settings panel: a Bento group with Discover, the registry's packages with a page each; Installed, with updates, withdrawals and Remove; and Share. Installing first shows what it will do, as the terminal does. `mochi://bento/<id>` links open the Discover page on a package: `mochi open-url` reads them, and the packages install `mochi-links.desktop` to receive them. `mochid bento plan` and `catalog` give the same as JSON, and modules get `ModuleCtx::config_file` and `socket` to run such commands.
- Bento is off until you turn it on, in the settings' Bento page, which says what it installs and what can go wrong, or with `[bento] i_really_understand_that_bento_can_harm_and_contain_malicious_content = true`. While it's off, nothing that fetches other people's packages runs; what's installed stays until removed.
- Share only part of a setup: `mochi bento share --only theme,module:osd,widgets`, or a switch per part on the settings' Share page, each module's settings on its own. `--theme`, or A theme on the page, writes your look as a theme package instead. `mochi bento parts` lists what a setup has to share. Whether Bento is on never goes into a share.
- Switch between setups: your own and each bento installed, with `mochi bento use <id>`, `mochi bento use mine`, or Use on the Installed page. Each keeps its own changes and widgets, so what you change while using one stays with it. Adding a bento switches to it, unless `--no-use`; adding a theme puts it on. Removing a bento no longer brings back the files from before it: it switches back to your own setup, and keeps what another bento still needs.
- `mochi bento publish` sends a release to the registry: run in a package's repository, it checks the committed and pushed release as the registry's CI will, writes its `packages/<id>.toml` or adds the release to it, and opens the pull request with `gh` from a fork. `examples/bento-publish.yml` does it on each tag pushed.
- Change screens during a recording of a whole screen: clicking the recording's dot now opens its controls, with the time, a button for each screen and **Stop**, instead of stopping it. `mochi ipc capture switch [screen]` does the same. Each screen records into a part, and ffmpeg joins the parts once it stops, encoding them again into the first screen's size when the screens differ.
- Pick a screen recording's codec and file format in the picker: the codec steps through Auto and every one gpu-screen-recorder lists on the machine, named like "HEVC (Vulkan)", and the file through MP4, MKV and WebM, skipping WebM when no codec fits it. `codec` and the new `container` set how they start; `mochi ipc capture codec` and `container` do the same.
- Pick the video encoder for conversions of dropped videos: Auto, then the encoders ffmpeg has, H.264, HEVC, AV1, VP9, VP8 and NVIDIA's or AMD's when that GPU is there. It applies where it fits the file. `[module.drop] video_encoder` and `mochi ipc drop encoder` set it.
- Menus for options with known values in the settings panel, with a search, and several picks for lists: apps installed, audio outputs and microphones, tray apps, players, hub cards, modules. A command option, like the idle clock's click, picks a module, one of its actions and its arguments. Command lines like the lock, the logout, the terminal and the screenshot editor offer ready-made ones for what's installed, and lists of fixed values, like drop's actions, pick from them. Schemas mark these with `x-source` and `x-suggest`, and the snapshot carries every module's `actions`. The tray publishes its apps in its state.
- A command option's arguments each get a control of their own: a menu for a choice or for values Mochi knows (monitors, hub pages, audio devices and apps, Bluetooth devices, displays, desktop apps, power profiles, settings sections, Wi-Fi networks and VPNs, players, tray apps), a switch, or a field. Actions mark these with `ArgSpec::source`, and plugin manifests with `source` on an argument.
- Bubble tooltips: resting the pointer on a bubble for `[bubbles] tooltip_ms`, 600 ms by default, shows more about it beside it. Every builtin bubble has one, from the track playing to who uses the microphone. Modules set one with `BubbleSpec::tooltip`, or a bubble view with a `tooltip` property; the protocol's `Bubble` and `bubbles` message gain `tooltip` and `tooltip_ms`. The tray's pinned bubbles use it instead of their own popup.
- A progress bubble while an action on dropped files runs, with an icon for what the files are and a ring that fills, from ffmpeg's progress for videos and sound. The panel can close meanwhile; a click on the bubble opens it again, and the island says how it went when it's done.
- Stop an action on dropped files: **Stop** in the panel, or `mochi ipc drop stop`, kills the program it runs and removes what it made, so a long conversion started by mistake leaves nothing behind.
- A click outside the island that closes a panel or a notice reaches the window under it too: the daemon clicks again at the same spot through a virtual pointer once the island lets go, so one click does both. While a panel is open, a click on another monitor closes it as well. `event` gains an optional `click`.
- Scrolls pass through too: scrolling outside a notice lets it go and scrolls the window under it, and scrolling outside a panel closes it and scrolls, on any monitor. A `Click` can carry a `scroll`, and the UI sends `pass_on` for a scroll that closes nothing.
- An island per monitor. Each monitor's island decides what it shows by itself, so a panel open on one doesn't hold back a notice on another, and a click expands what it shows on that monitor only. What's meant for every monitor, like the idle clock, shows on each island and ends on all of them at once. The workspace indicator now shows on the monitor that switched. The daemon keeps an arbiter per monitor (`mochi_core::Islands`), the protocol's `present` names the monitor in a new `output` field, and the UI's `event` names the island it happened on.
- The tray drawer takes the keyboard: arrows, Enter to activate, Shift+Enter or the Menu key for an app's menu, and arrows, Enter and Left in menus. Tooltips show under the drawer's apps and beside pinned bubbles when the pointer rests on them.
- XEmbed tray icons, from old X11 apps: the tray starts KDE's `xembedsniproxy` when it's installed and not running, which turns them into StatusNotifierItems. `xembed = false` turns it off.
- WPA Enterprise networks join from the Network page: the prompt asks for the user name with the password, and joins with PEAP and MSCHAPv2.
- Hidden networks: **Hidden…** on the Network page, or `mochi ipc network hidden`, asks for the network's name and security on the island.
- Mochi is NetworkManager's secret agent: a password it needs later, like a saved network's that changed or a connection started with nmcli, is asked on the island, with a note when the last one didn't work. Wi-Fi and 802.1X passwords only; a VPN's go to another agent.
- Thumbnails for recordings, on the hub's Captures page and in the card after a recording: a frame ffmpeg picks among the first ones, made once in the background and kept in `$XDG_CACHE_HOME/mochi/thumbnails`. `mochi doctor` lists ffmpeg.
- Open windows in the launcher: the `windows` provider lists the windows on every workspace whose title or app matches what you type, above the apps, and Enter focuses one. `Compositor::toplevels` and `activate_toplevel` give modules every open window through wlr-foreign-toplevel-management, on Hyprland, Sway and niri alike.
- Drop files on the island, the `drop` module: while files hover, the island's outline turns to the accent and it says to drop them; once dropped, it names what came and offers what fits: compress, extract, merge PDFs, convert images to PNG, JPEG or WebP, copy the paths, open. Each action shows when a program for it is installed. New files go next to the dropped ones under a free name, and Show opens their folder. `mochi ipc drop files` and `run` do the same from a script. It's on in newly generated configs; add `drop` to `modules` in yours.
- Dropped files convert to more formats, under **Convert to**: images between PNG, JPEG, WebP, GIF, BMP, TIFF and ICO in Mochi itself, with no program, and to SVG with vtracer; videos and sound with ffmpeg; Office documents with LibreOffice; Markdown and HTML with pandoc. The `png`, `jpg` and `webp` actions became one `convert`, and the old names still work.
- Archives convert too, between ZIP, 7z, tar, tar.gz, tar.xz and tar.zst, and from RAR: extracted into a scratch folder and packed again, with bsdtar, or unzip, tar, zip and 7z without it.
- The drop panel names the program to install when one is missing for what was dropped, like "Install ffmpeg to convert videos", and converts the files it can without it.
- The hub's home is editable: the pencil in the navbar outlines the cards, a drag moves one, its minus takes it off and "More cards" puts it back. Done keeps the result in the hub's new `order` and `hidden` settings, through `changes.toml`, and `mochi ipc hub arrange` does the same from a script. The hub stays open while it saves.
- A volume card on the hub's home, from the audio module: the output's volume as a slider, with the icon muting it.
- `Module::live_settings` names the settings a module applies while it runs. A reload that changes only those sends `ModuleEvent::Reconfigured` with the new table instead of restarting the module. The hub's `order` and `hidden` are the first.
- Night light, the `nightlight` module: warmer screens through the compositor's gamma control (`wlr-gamma-control-unstable-v1`), with no other program. It turns on by hand, between two times, or from sunset to sunrise at your latitude and longitude, fading over `fade_minutes`; by hand lasts until the schedule changes next. The hub has a tile, and `mochi ipc nightlight on`, `off`, `toggle`, `auto`, `temperature` and `status` drive it. `mochi doctor` lists the protocol. It's on in newly generated configs; add `nightlight` to `modules` in yours.
- Keep awake, in the power module: a switch on the hub's Power page and `mochi ipc power awake [on|off|toggle]`. It holds a logind inhibitor for idle and sleep, and the island inhibits idle through Wayland's protocol, so hypridle, swayidle and automatic suspend all wait. A cup bubble stays while it's on, and a click on it turns it off.
- A privacy indicator, the `privacy` module: a bubble next to the island while an app records from a microphone, with a microphone icon in orange, or has a camera open, with a camera in green; a muted microphone shows crossed out. A click mutes the microphone or unmutes it, and the wide bubble names the apps. The camera comes from the processes with `/dev/video*` open. It's on in newly generated configs; add `privacy` to `modules` in yours.
- The audio module's state lists the apps recording from an input under `recording`, without Mochi's meters or recordings of an output.
- Brightness, the `brightness` module: the laptop's backlight from `/sys/class/backlight`, set through logind, and external monitors over DDC/CI through ddcutil. The OSD shows the level when it changes, also from a key the firmware handles, and scrolling on it changes it. The hub has a card with a slider for each display. `mochi ipc brightness up`, `down` and `set <level> [display]` change every display or one, by `backlight`, `external`, an output like `DP-1` or a model; `refresh` looks for monitors again and `status` prints the levels. `mochi doctor` says when ddcutil is missing. It's on in newly generated configs; add `brightness` to `modules` in yours.
- Theme presets with a light mode: `preset` is `obsidian`, `catppuccin`, `nord`, `gruvbox`, `rose-pine` or `tokyo-night`, each with a dark and a light palette that `appearance` picks, or `"auto"` to follow the system's preference live through the desktop portal. `[colors]` goes over the preset. The settings panel lists them with their colors.
- `preset = "wallpaper"` makes the palette from the wallpaper, an image or a plain color that awww, swww or hyprpaper shows, or the image `wallpaper` names: the accent from its most colorful hue, the backgrounds from that hue nearly grey, and lightness chosen for readable text.
- Motion settings in `[motion]`: `reduced = true` turns animations off, so views appear and change at once and the island takes its shape without a spring, and `speed` makes every animation faster or slower. Every animation goes through `Theme.duration(ms)`, and the design check rejects a fixed duration.
- niri and Sway through their IPC, like Hyprland: the focused output, exact also when focus moves to an empty workspace, and where windows are, for picking a window to capture or share. On niri, its casts say what's being shared, for the Sharing bubble, from the moment Mochi starts. Switchable shares still need Hyprland, which makes the monitor they share.
- `mochi doctor` checks what Mochi needs around it and says what to install or change: the daemon and the UI, the config, Quickshell's version, the fonts, the compositor's protocols and what each is for, the portal and its share picker, another notification daemon, and the programs the enabled modules and plugins run. Modules say which with `Module::needs`, and plugins with `[backend] needs`. It fails when something is broken.
- A test compiles every QML view, the core's and each module's, in Quickshell with Qt's offscreen platform, so a view with a syntax error, an unknown type or a property that doesn't exist fails the build. The Nix package runs it.
- A guided tour, the `tour` module. The first time Mochi starts, a notice offers it; after an update, it offers a tour of what's new since the last one. The island pauses, every monitor dims and takes every click and key, and each module shows its real views with made-up data and a caption, chapter by chapter, moving on by itself every few seconds with the island's own transitions. The island's border glows in the accent while it runs, and the looks steps cycle through accents, borders and shadows. Space and the arrows move sooner, and Escape asks whether to stop; a stopped tour starts again where it was. `mochi ipc tour start`, "Tour" in the launcher and a button on its Settings page run it again. `install = false` and `updates = false` turn the offers off.
- Modules and plugins offer tour steps as contributions to `tour`: a view, made-up data, a caption and the release in `since`, which the tour of what's new goes by. Every builtin module has its steps, dated from the history.
- A preview layer in the daemon: options tried without being kept, applied at once over the changes and gone on reload. `mochi ipc settings preview` uses it, with Keep and Drop in the panel, and the tour shows the island's looks through it. `SettingsOp::Preview`, `Keep` and `Drop`.
- `ModuleCtx::pause_island` shows only the caller's activities and bubbles; the others wait and come back after, and a fleeting one like a volume change ends instead.
- `mochi_core::config::state_dir`, the `$XDG_STATE_HOME/mochi` modules keep their files in.

### Fixed

- A `mochi reload` that failed lost `mochid --modules`, so the next one ran every module.
- The settings panel no longer goes blank while something is being tried: the page was anchored to the Keep and Drop bar, which Qt refused, so it lost its height.
- Scrolling in a panel, like the settings, closed it when the list under the pointer was already at its end: the scroll fell through to the catch for scrolls outside. Scrolls over the island and the bubbles never count as outside now.

### Changed

- Bento's registry and the publish workflow install Mochi's latest release instead of building it.
- The settings panel has no Preview button: every change already applies live and is kept. The preview layer stays for `mochi ipc settings preview` and the tour.
- A config without `modules` runs every builtin module, not only `idle`, so a new install, or a home-manager config that sets a few options, has the whole shell. List `modules` to pick fewer. The list is `mochi_core::config::DEFAULT_MODULES`.
- A quieter default look. `EdgeLight` no longer draws a hairline along every panel and card, nor a band of the accent that follows the pointer: it shows only while something works and in the flash when it's done, in a neutral grey unless the view gives it a color. The outline around the island, the bubbles and the hub is fainter, 5% white instead of 8%, in the default theme and the dark palette of every preset.
- The demo module moved to `examples/demo`, and the Nix and Arch packages leave it out: it's for working on Mochi, with test views, the `mochi ipc demo` actions the daemon's tests drive, and `mochi ipc demo controls`. Builds from the repository still have it, as mochid's default `demo` feature.
- The daemon sends only the contributions whose target module runs: an offer to a module that's off goes nowhere.

## 0.0.7 - 2026-10-08

### Added

- A settings panel, the `settings` module: every option of the theme, the island, the bubbles, each module and each plugin, applied as you change it. It has a sidebar of sections, a search through every option (`@modified` lists the changed ones), a dot and a reset button on each option changed in the panel, which takes it back to what your files say, a color picker, a menu of the fonts installed, each drawn in itself, and a TOML editor per section. Copy gives everything that isn't a default as the `settings` and `theme` of home-manager's `programs.mochi`, or as TOML. `mochi ipc settings open [section]` opens it, the hub has a gear, and the launcher finds sections and options by name. It's on in newly generated configs; add `settings` to `modules` in yours.
- The panel's changes live in `changes.toml` next to `config.toml`, laid over it and `theme.toml` whenever mochid reads them; it never writes those two. A change goes away once your files say the same, and changes that no longer fit are set aside with a warning so mochid still starts.
- Modules describe their settings with a JSON schema (`Module::settings_schema`, from `schemars`), and plugins with a `[settings.<key>]` table in their manifest: choices, a range, a color, a font. Modules can read and change the settings through `ModuleCtx::settings_op`.
- `Fonts`, a QML singleton with the font families installed on the system and a search through them, for any view that offers a font.
- `CopyButton`, the Copy button with Nix and TOML in its menu, shared by the widgets drawer and the settings panel, and `ModuleCtx::close_other_panels`, which the panels use to close each other.
- The colors module's hub page is a color chooser (#1): a saturation and brightness square and a hue bar, the color in five formats with a click to copy each, and the history beside it. `mochi ipc colors add <color>` adds a color to the history.

- Notifications show markup in the body (`body-markup`, `body-hyperlinks`): bold, italic, underline, line breaks and links to http, https, mailto and file URLs. Other tags show as text, and an image shows its `alt` text. A click on a link opens it with `xdg-open` and closes the notification, in the popup, the history and the hub. `mochi ipc notifications open <id> <url>` does the same.
- Inline replies (`inline-reply`): a notification from an app that takes a reply has a Reply button that opens a text field. Enter sends the text back as `NotificationReplied`, and Escape goes back. `mochi ipc notifications reply <id> <text>` does the same.
- `ListRow` has `subtitleFormat` and `linkActivated`, for a subtitle with markup.
- Clicking an area's "+N" lists its hidden bubbles on the island, each drawn as its module draws it. A click on one does what a click on the bubble does, and Escape or a click outside closes the list. The protocol has a new UI message, `overflow_click`, and no plugin may take the id `mochi`.
- Alignment guides while arranging widgets: accent lines show where a widget's edges or middle meet another widget's or the screen's middle, and it snaps to them within 6 pixels.
- Emoji skin tones: six swatches by the picker's search box set the tone of every emoji of a person or a hand, in the grid, the Recent tab and the launcher's `:`. Holding an emoji opens its tones; the one you pick is pasted and that emoji keeps it. Emoji of two people take the same tone for both. `mochi ipc emoji tone <tone> [emoji]` sets either, and `emoji.json` keeps them with the recents.
- The launcher's file index follows your files through inotify: files and folders you make, delete, rename or move show up in `/` results, or leave them, right away, even with the launcher open. When the system runs out of inotify watches, it logs it once and rebuilds the index on open as before.
- Pinned clipboard entries: `pin` and `unpin`, Ctrl+P in the picker, and a pin button on each row of the picker and the hub page. Pins stay at the top under "Pinned", and clearing the history and its limits leave them. They're kept in `$XDG_STATE_HOME/mochi/clipboard/pins`, encrypted with the history's key from the Secret Service even with `storage = "memory"`, so they last across logouts.
- The clipboard module's `ignore` lists apps whose copies aren't kept, matched against the focused window's app id (the class on Hyprland): KeePassXC, Bitwarden and 1Password by default. `skip_secrets = false` keeps copies marked with `x-kde-passwordManagerHint`.
- The clipboard module's `pause` pauses without an argument, `resume` starts again, and a bubble shows while paused; clicking it resumes.
- The clipboard picker is wider, with a pane showing the selected entry in full: the text, scrolled with Page Up and Page Down, or the image scaled to fit.
- The Performance page shows disk read and write speeds and network download and upload speeds, each pair with a graph of the last two minutes, in B/s up to GB/s. Disks are whole disks only and network cards are real cards only, so partitions, loop, zram, device-mapper, loopback, bridges and VPN tunnels don't count twice. `mochi ipc performance status` prints them.
- The Performance page's process list sorts by CPU, memory or disk use, with a column for each. Disk use is read from `/proc/<pid>/io`, so only your own processes have one.
- End a process from the Performance page: hovering one of yours shows End, a click asks to confirm, the next sends SIGTERM, and after 3 seconds one still running offers Force, for SIGKILL. `mochi ipc performance end <pid>` and `kill <pid>` do the same, and refuse other users' processes.
- The performance widget has a setting per graph, `cpu`, `memory`, `gpu`, `disk` and `network`, in place of `reading`; an older `reading` still picks one graph.
- The mixer shows one row per app: its slider and mute act on every stream of the app, and a chevron opens it to show each stream with its own slider. `volume` and `mute` take an app's name for all its streams.
- A button on each app row in the mixer moves the app to another output, and in an opened row each stream has its own. `mochi ipc audio move <app> <device>` does it from a keybind. PipeWire remembers the output for the app's next streams.
- Peak meters in the mixer: while it shows on the island or as the hub's Sound page, each slider's fill dims and the part the sound reaches stays lit. Mochi asks the audio server for peak levels only while a mixer is open, and the input's meter moves only while an app records. The `Slider` control has an optional `level`.
- The protocol has a `live` message for values a module sends many times a second, like the meters' levels: `ModuleCtx::publish_live` sends one, views get it from `Daemon`'s `live` signal, and the daemon doesn't keep it.

- Plugins without build tools. `programs.mochi.plugins.<id>.src` in home-manager has Nix build a Rust plugin during the switch, from its `Cargo.lock` with no hash to write, and `package` takes one already built. `mochi plugins install` builds a plugin with its `flake.nix` when it has one and Nix is installed, and `mochi.lib.buildPlugin` and `examples/plugins/flake.nix` give plugin authors that flake. A plugin in the Nix store isn't built again.
- Mochi builds plugins in any of its languages with Nix, without their authors writing Nix: Rust, Node, Python and Go (with `vendor/`) from the lock file they already have, and scripts and release archives as they are, with release binaries patched to run on NixOS. The language comes from the plugin's files, or `[backend] kind`. `[backend] needs` lists the programs the backend runs: Nix puts them on its PATH, `mochi plugins install` lists the missing ones, and mochid warns about them. With Nix installed, `mochi plugins install` builds with Nix when the build's tools or the needed programs aren't there. home-manager's `plugins.<id>.runtimeInputs` adds programs.
- A failed plugin build names the tools missing and the ways around it: installing them, home-manager's `src`, a `git-release:` source when the plugin has releases, or a flake.

- The idle module's `hover` runs an action when the pointer rests on the clock for `hover_delay_ms` (350 by default), like `hover = ["workspaces", "show"]`; a quick click still runs `click`. Modules hear the pointer come onto their activities and leave as `ModuleEvent::Hovered`, in `mochi-core` and `mochi-sdk` (the plugin protocol's `hovered` message), and an activity the island stops showing under the pointer hears it leave.
- `mochi ipc workspaces show` shows a monitor's workspace dots on the island, the one under the pointer by default: clicks switch, and they stay while the pointer is on them. Scrolling on the dots switches to the previous or next workspace.

- A design scale in `Theme`: four text sizes and one for big numbers (`textCaption`, `textBody`, `textTitle`, `textHeadline`, `textDisplay`), weights, five spacings (`spaceTiny` to `spaceHuge`), corners by what they round (`radiusSurface`, `radiusField`, `radiusControl`) and heights (`controlHeight`, `rowHeight`, `tileHeight`). A test reads every view and rejects new raw sizes, spacing, corners, margins and colors; a line that needs one says why in a `// design:` comment. The core controls and the hub are on the scale; the modules follow.
- Inter for text and Material Symbols Rounded for icons, brought by the Nix package and the Arch packages, which mochid links into the shell when it finds them (`MOCHI_FONTS`, then `/usr/share/mochi/fonts`). `Symbol` draws Mochi's icon names from the font, and any Material Symbols name too, like `timer`, with `filled` for active states; without the font it draws its own, as before.
- Shared components: `PanelHeader`, `SwitchRow`, `SliderRow`, `RollingText` (digits that roll when they change), `ScrollFade` (fading edges where a list scrolls) and `EdgeLight` (a light along a panel's top edge that leans toward the pointer, sweeps while something works and flashes when it's done). `mochi ipc demo controls` shows them.
- The hub's cards sit in frames the hub draws, with the icon, the title and a chevron inside, on a grid of equal rows: a card spans `options.span` columns and `options.rows` rows, or the one or two rows its view needs, and fills the first free place. The hub takes the height its content needs, up to `height`, and the island's outline follows it between pages, instead of one size for every page.

### Changed

- `mochid` starts with any Quickshell 0.3.x instead of only 0.3.1, so it runs on the 0.3.0 that Debian 13 backports, Ubuntu 26.10 and Guix ship. It's still tested against 0.3.1.
- Every module view and the example plugins use Theme's scales for text, spacing, corners, heights and colors, and the design check's baseline is empty.
- The hub's Home starts with a row of toggles (Network, Bluetooth), then Today, the latest missed notification and the battery, each one row, with Now playing two columns wide and two rows tall beside Clipboard and Colors. Hub cards that set `rows` get a view sized to fill them.
- The Wi-Fi tile says "Ethernet · <connection>" when a cable carries the connection.
- The battery card and widget fit in one row with the power profiles at the end; the widget no longer spills out of its frame.
- Clocks, percentages, timers and counts roll their digits: the idle clock, Today, the widgets clock, the volume OSD, the mixer, battery and Bluetooth levels, the recording time, the pomodoro timer and performance readings.
- Island panels and widget frames have an edge light: it sweeps while a capture saves, a Wi-Fi password is tried or pairing waits, and flashes when a capture is saved, a color picked or copied, an emoji or clipboard entry copied, or the widget layout copied.
- Lists that scroll fade at their edges, and page and panel headers share one `PanelHeader`, which takes an optional `icon` and `iconColor`; the Bluetooth page shows its symbol in the accent color while powered.
- Bluetooth devices show icons by kind, and the power profiles use the eco, balance and speed icons.
- The type scale has four sizes and one for big numbers: `label` and `subtitle` are gone from `[text]` in `theme.toml`, and `title` is 15 instead of 16. Old files still work: `label` is read as `caption` and `subtitle` as `body`, with a note from `mochi config check`. `Theme.textLabel` and `Theme.textSubtitle` give `textCaption` and `textBody` for views and plugins that still use them.

### Fixed

- A restart of mochid, or a reload that restarted the share module, froze a running switchable share: Mochi removed `MOCHI-SHARE` under the app, which had to stop sharing and share again. Mochi now leaves the monitor while the app captures it, and the next start carries on with the same source and quality.
- After a restart, Mochi didn't know a share was running, since Hyprland only reports captures as they start and stop: no bubble, and a switchable share it couldn't switch. The share module writes down what's captured and gives it back to the compositor when mochid starts again. `Compositor::assume_captures` takes captures from before Mochi started, and the first IPC connection no longer clears them.

- When `mochi reload` restarted the idle module, it left a second clock waiting behind the shown one, and its `hover` then watched the hidden one.
- Discord's notifications showed their Markdown as typed, like `**TEST**`. The notifications module reads it now: `**bold**`, `*italic*`, `__underline__`, `~~strike~~`, `` `code` ``, `[text](url)` links, and `||spoilers||`, shown as "spoiler". Plain `https://` links in any app's notifications become links, and `<s>`, `<strong>`, `<em>`, `<del>` and `<strike>` are kept. `markdown = false` in `[module.notifications]` turns it off.
- A plugin's override that fails to load gives way to the builtin view, on the island and in bubbles, and `mochid`'s log has the error.
- A widget in the middle or far third of a screen that isn't a whole number of cells no longer jumps a few pixels when you let go of it.

## 0.0.6 - 2026-10-07

### Added

- Bubbles animate out: a pill shrinks and fades when its last bubble goes, and a bubble leaving a group fades while its pill narrows. The other pills slide into place instead of jumping, when one comes or goes, in a row or a stack.
- `show = false` in `[bubbles.<module>]` hides a module's bubbles.
- Widgets module: views on the desktop, under the windows, from any module or plugin, placed as often as you like with settings each. `mochi ipc widgets edit` raises them on the focused monitor to arrange: drag to move on a grid, drag the corner to resize, click for a settings form made from what the widget declares, drag new ones from a drawer that a click on the island opens, with a search and a filter per module. Widgets have layers, `z`, for which is on top where they overlap. Every change rewrites `widgets.toml` next to `config.toml` at once, and hand edits to it apply when saved; `mochi config check` checks it. Positions are anchors and grid cells, so layouts survive other screen sizes. "Copy as Nix" and `widgets export` print the layout for home-manager. The module offers a clock with a time zone, 12 or 24 hours, seconds and the date, and a calendar. Media and battery offer their hub cards as widgets, performance a widget of graphs, and the weather example plugin its card.
- Notes module: to-do lists and notes as desktop widgets, typed into on the desktop, each with its own content in `$XDG_STATE_HOME/mochi/notes.json`. It's on in newly generated configs. The home-manager module's `widgets` declares a layout, as Nix or TOML; a rebuild writes it only when it changed, so arranging keeps working. Modules offer widgets with `widgets` contributions, see Writing widgets. It's on in newly generated configs.
- The core shell has a desktop layer on each monitor, under the windows, for the widgets module's view. Modules get the directory `config.toml` is in from `ModuleCtx::config_dir`.
- Emoji module: an emoji grid on the island (`mochi ipc emoji toggle`) with a search, a tab per group and recents; Enter or a click types the emoji into the window you were in, Shift+Enter or a right click copies it. In the launcher, `:` searches emoji the same way. It's on in newly generated configs.
- Colors module: `mochi ipc colors pick` freezes the screen with a magnifier, and a click copies the color under the pointer, read exactly from the screen at any scale and rotation. A card on the island shows it as HEX, RGB, HSL and OKLCH, each a click to copy. A hub card and page keep the history. In the launcher, `#` takes a color in any of those forms or a CSS name and shows it in every format, or picks one from the screen. It's on in newly generated configs.
- The launcher's file search and web searches are built in: `/` searches an index of your home folder in memory, Enter opens a file and Shift+Enter its folder; `!w`, `!g`, `!gh`, `!yt`, `!nix` and `!wiki` search the web, and `engines` adds more. Results can say what Shift+Enter does (`alt`) and show a color swatch (`color`). Modules' providers are asked at once instead of after a pause. `mochi ipc launcher open :` opens it with something typed already. The example emoji plugin is now `emoji-example`, with `;`.
- The clipboard module's `copy-text` leaves the hub open; only pasting closes it.
- A double click on a mixer slider puts the volume back to 100%. The `Slider` control has a `reset` value for it.
- `[island] notices`: `"focus"` or `"pointer"` shows notifications, the volume and every other notice on one monitor, and the other monitors keep the idle island; `"all"`, the default, keeps them on every monitor. The protocol's `present` message has a new `resting` field, the activity other monitors show meanwhile; an island whose monitor a panel isn't meant for now shows it too, instead of what it showed before.
- `mochi dismiss` closes what the island shows, for a keybind: notices that leave the keyboard to your app, like the volume, can't hear Escape. The protocol has a matching `dismiss` message.
- Launcher providers. Results come from providers: the apps, a calculator (`=`, or plain math like `2+2` without it; Enter copies the result), and commands (`>`; Enter runs one, Shift+Enter in a terminal, and ones you ran come back first). A query starting with a provider's prefix asks only it; the others answer together, under headings, as their results come in. `[module.launcher.providers.<name>]` changes a provider's prefix, order and heading, turns it off, or adds a script provider: a command that prints results as JSON lines, each saying what Enter does (`copy`, `type`, `open` or `run`). `examples/launcher` has a web search and a file search script.
- Plugins can be launcher providers, with a `provider` contribution whose `search` action answers queries. `examples/plugins/emoji` is an emoji picker built this way. A module receives the contributions offered to it as `ModuleEvent::Offers`, in `mochi-core` and `mochi-sdk` (the plugin protocol's `offers` message), and a contribution's `view` is optional.
- `ModuleCtx::ask` in `mochi-core` and `mochi-sdk`: like `call`, and returns what the action answered with. The plugin protocol's `call_result` has a new `output` field, and the Python example SDK's `call` returns it.
- The clipboard module's `copy-text` and `paste-text` put text from elsewhere on the clipboard, and keep it in the history.
- Hub cards open their module's page: a chevron by the heading marks the ones that have one, and clicking the heading, or the card beside its controls, opens it. `options.page` picks the page when a module has several.
- Scrolling on the volume OSD changes the volume, 5% a notch, through the audio module.
- The battery card has the power profiles under the level, like the power page.
- The notification history lasts across restarts, in `$XDG_STATE_HOME/mochi/notifications.json`, readable only by you, with the images from raw pixels next to it. Restored notifications have no buttons, since their apps can't answer anymore. `save_history = false` turns it off.
- `display_family` in the theme's `[text]`: a font for the clocks on the idle island and in the hub.
- A plugin's settings are checked against its `settings.toml`, like a builtin's: an unknown key or a value of the wrong type fails `mochi config check`, startup and reload, naming the key.
- `examples/plugins/release.yml`, a GitHub Actions workflow for plugin repositories: on a tag it builds the backend on x86_64 and aarch64 and attaches the archive `git-release:` downloads.
- The power module finds power-profiles-daemon under its older name, `net.hadess.PowerProfiles`, for versions before 0.20.
- A switchable screen share's monitor takes the shape of the screen or area shared first, so it fills the copy without bars.
- `mochi completions <shell>` prints shell completions; the Nix and Arch packages install them for bash, zsh and fish. `--json` makes `mochi status`, `mochi ipc list`, `mochi ipc <module>` and `mochi plugins list` print JSON.
- Keybind examples for Hyprland, in Lua and `hyprland.conf`, Sway and niri on the Getting started page.

### Fixed

- Escape closes a modal view whatever inside it has the focus: the island takes the keyboard's focus for each one that doesn't take it itself, and Escape a view doesn't use comes up to the island.
- A plugin without views no longer makes Quickshell warn about an import it can't find.
- `mochi ipc hub open <page>` switches pages while the hub is open, also after a click on a tab.
- A notice that closes on a click outside, like the media notice, took every scroll until a click closed it, so a page under it couldn't scroll. A scroll outside now lets go of the screen, and the notice stays until it times out.

## 0.0.5 - 2026-10-06

### Added

- Plugins. A plugin is a module from outside Mochi, with the same powers as a builtin: activities, bubbles, actions in `mochi ipc`, hub cards and pages, settings under `[module.<id>]`, the compositor, and calls to other modules. Its manifest, `mochi-plugin.toml`, names its backend, actions, contributions, the builtin views it replaces and the modules whose state it reads. `~/.config/mochi/plugins.toml` lists plugins with a `git:` source built from a branch, tag or commit, a `git-release:` source downloaded from a GitHub release, or a `path:`. `mochi plugins install`, `update`, `remove` and `list` manage them: install shows what a plugin runs and asks first, and `plugins.lock` pins each one's commit or release. A backend runs as its own process over a socket, supervised: after 5 crashes within a minute it stays stopped until `mochi reload`, with a notification. `mochi status` lists plugins. The home-manager module has `plugins`.
- `mochi-sdk`, for plugin backends in Rust: `ModuleCtx` and `ModuleEvent` work like a builtin's. Two example plugins in `examples/plugins`: a pomodoro timer and the weather from Open-Meteo.
- Plugin documentation: the Plugins page for installing them, and Writing plugins, Plugin manifest, The Rust SDK and Making an SDK for writing them and libraries for other languages. The documentation site has the SDK's API reference under `api/` (`nix build .#sdk-docs`). `examples/python/hello` is a plugin in Python with a small SDK of its own, which the Making an SDK page walks through; the test suite runs it.
- Modules can read each other's state: `ModuleCtx::watch_state` sends another module's state as `ModuleEvent::State`, the latest at once and every change after.
- The plugin protocol, in `docs/protocol.md` and `mochi_protocol::plugin`. `ActivitySpec`, `BubbleSpec`, `Priority`, `Args` and `CallError` moved to `mochi_protocol::spec` and serialize to JSON; `mochi-core` re-exports them where they were.
- Stacked bubbles, with `stack = true` in `[bubbles]`: each area shows its most important bubble, the others peeking out behind it, and fans out on hover. A bubble with news comes to the front for `news_ms`, then goes back. Modules mark news with `BubbleSpec::news()`; notifications, network, Bluetooth, battery, tray and performance do for what matters. The protocol's bubbles carry `priority` and `news`, and the bubbles message `stack`.
- Performance module: CPU, memory and GPU use and the CPU's and GPU's temperatures. A Performance page in the hub with graphs of the last two minutes and the busiest processes; a notice when a reading stays over its level, naming the busiest process; and a red bubble while one stays critical. Levels per reading, and how long a reading must stay up, are settings. NVIDIA GPUs are read through `nvidia-smi`, AMD ones from sysfs. It's on in newly generated configs.
- Symbols: `memory`, `gpu` and `temperature`.
- Battery module, from UPower: a short notice on the island when the battery drops past a level on battery, at 80, 50, 20 and 10% by default, once per discharge; a warning bubble at or under 50%, red and breathing at 10%, where the notice also stays longer; notices on plugging in or out with the time left; and a hub card with the level. `notices`, `warning`, `critical` and `plugged` set it. It shows nothing without a battery. It's on in newly generated configs.
- Network module, from NetworkManager: a bubble with the connection (Wi-Fi strength, Ethernet or offline, a lock while a VPN runs), a Network page with the Wi-Fi networks, wired devices and VPNs, a home card with Wi-Fi, VPN and airplane tiles, and a notice on connecting or disconnecting. Joining a new secured network asks for its password on the island; a network that doesn't come up is forgotten, so a wrong password isn't kept. `wifi`, `airplane`, `scan`, `connect`, `disconnect`, `forget` and `vpn` actions. It's on in newly generated configs.
- Bluetooth module, from BlueZ: a bubble with the connected device's battery, a Bluetooth page to connect, pair and forget devices and scan for new ones, a home tile, and a notice when a device connects or disconnects. Mochi is BlueZ's pairing agent, so codes to confirm or type show on the island. It's on in newly generated configs.
- Symbols: `wifi-1`, `wifi-2`, `wifi-off`, `ethernet`, `offline` and `airplane`.
- Tray module: apps' tray icons, through StatusNotifierItem. A tray bubble opens a drawer on the island with every app's icon and name; a click activates the app, a right click shows its menu in Mochi's style, with submenus as pages, and a middle click does its second action. Apps in `pinned` get a bubble of their own, which also takes the scroll wheel. An app asking for attention makes its bubble breathe. Mochi serves the StatusNotifierWatcher, or shows another tray's icons when one already does. `mochi ipc tray list`, `activate`, `menu` and the others work from keybinds. It's on in newly generated configs.
- Symbols: `tray` and `dot`.
- Capture: a Captures page in the hub, with the newest screenshots and recordings in their folders, whatever made them. A click opens one in the preview card, and each row copies it, edits a screenshot, opens its folder or deletes it. `copy`, `edit`, `delete` and `open` take a file from the history; `preview <path>` shows one.
- Screenshots across monitors: a region can run from one screen into the next, and the screenshot joins their parts at the sharpest screen's scale, transparent where no screen is. A new All screens mode, `mochi ipc capture screenshot all`, takes every screen as one image.
- `[island] panels` in `config.toml`: the launcher, the hub, the clipboard and other views you type into open on the monitor with keyboard focus (the default), the one under the pointer, or all of them. The protocol's activity has a new `output` field.
- `[island] click_outside`: a click outside the island now closes whatever it shows, a notification popup, the media card, a screenshot's preview, instead of only views you opened. A popup you never opened goes to the missed ones. The volume and workspace notices let clicks through (`ActivitySpec::passive`). `"expanded"` goes back to closing only views you opened. The protocol has a new `outside` field and event.
- Clicking a clipboard image opens it in the screenshot preview card, with copy, edit and delete. The capture module has a `show` action for it.
- Audio module: a volume mixer, as the hub's Sound page or on the island with `mochi ipc audio toggle`. It has the output and the input with their volumes and a list of devices to switch to, and a volume and mute for each app playing sound. `volume <target> <level>` (with `+5` and `-5`), `mute`, `output` and `input` do the same from keybinds. `max_volume` sets the loudest the sliders and `volume` go, up to 300%, and the OSD's volume bar follows it, with a mark at 100%. It's on in newly generated configs.
- Media: arrows next to the player's name, on the island and the hub card, switch between players when several have a track, like Spotify and a browser video. The one you pick stays shown until it stops, even when another starts a new track. `next-player`, `previous-player` and `player <name>` do the same from keybinds.
- Share: clicking the sharing bubble changes what you share, without the app asking again. The portal can't change a running share, so with the picker's new Switchable switch on, the default, the app shares a monitor of Mochi's own, `MOCHI-SHARE`, and Mochi draws a live copy of the screen, window or region you picked on it. The bubble, or `mochi ipc share switch`, opens the picker to change the copy. The monitor goes a few seconds after the app stops. `switchable = false` in `[module.share]` shares the choice itself, as before. Hyprland only.
- Quality presets for recordings and switchable screen shares: the pickers step the frame rate through 15, 30, 60, 90 and 120 fps (F in the capture picker) and the resolution through Native, 480p, 720p, 1080p and 1440p (Q), which scales down keeping the shape and never up. Recordings pass them to gpu-screen-recorder; a switchable share makes its monitor at that size and refresh rate, and the picker the bubble opens changes them during the share. `resolution` is a new setting next to `framerate` in `[module.capture]`, and both are new in `[module.share]`. `mochi_core::quality` has the presets.
- Views: a module can show `Screen.qml` on a monitor of its own, named `MOCHI-<MODULE>`. `Daemon.screens` lists the real monitors and `Daemon.virtualScreens` these. The compositor state has each output's size, and what is being captured (`captured`, from Hyprland's `screencastv2` events).
- `ActivitySpec::fleeting`: an activity that shows at once or not at all, never queued or suspended. The volume and workspace notices are fleeting, so they no longer show late after the hub, the launcher or the mixer closes. The volume notice now replaces a workspace notice on screen instead of waiting for it.

### Fixed

- `mochi reload` restarts a module that stopped or failed, instead of leaving it out until mochid restarts.
- A stack of bubbles left of the island stood a bubble's width away from it: the layout read the deck's width while it animated.
- The hub has one size for every page, `width` and `height` in `[module.hub]`, so it no longer changes size when switching pages.
- The hub no longer grows past the screen, which pushed the navbar off it: its cards or page scroll past most of the screen's height. It's wider, with three columns packed without gaps, and cards with nothing to show, like Bluetooth without an adapter or Battery on a desktop, hide, as do the pages of modules that aren't available.
- Stopping mochid gives modules up to 2 seconds to clean up, so the share module removes its switchable monitor instead of leaving it behind.
- A module added by `mochi reload` showed nothing when its views use each other, like the audio mixer: Quickshell only finds a directory's QML types when it starts. The daemon now restarts Quickshell when a reload adds a module.
- Each monitor's island sends events for the activity it shows, not the daemon's current one.
- The capture overlay no longer reports itself ready before it knows its screen, which could let the island change before the screen froze.

## 0.0.4 - 2026-10-05

### Added

- Clipboard module: a clipboard history on the island. `mochi ipc clipboard toggle`, bound to SUPER+V, searches what you copied, text and images with thumbnails; Enter pastes the entry into the window you were in by typing Ctrl+V, or Ctrl+Shift+V in a terminal, and Shift+Enter only copies it. It reads the clipboard through `ext-data-control-v1` and types through `zwp-virtual-keyboard-v1`, so nothing else needs to run. The history is a log file of its own, in the runtime directory until you log out, or with `storage = "disk"` in `$XDG_STATE_HOME/mochi`, encrypted with XChaCha20-Poly1305 under a key kept in the Secret Service. Copies password managers mark as secret are never read, and removing an entry rewrites the file. A hub card shows the count and pauses or clears the history. It's on in newly generated configs.
- The hub has a Clipboard page: the history with a search box, pause and clear; a click pastes an entry, and each row can copy or remove it.
- `Priority::TOP`, which interrupts any activity, even an uninterruptible one.
- `ModuleCtx::session_dir`, a module directory in the runtime directory that survives a daemon restart, unlike `data_dir`.
- The compositor state has `focused_app`, the app id of the window that had focus last.
- Share module: the screen-share picker for xdg-desktop-portal-hyprland. The island shows the screens and windows with live pictures, or lets you draw a region, with a Remember switch; `mochi share-pick` is the program the portal runs, packaged as `mochi-share-picker`, and the home-manager module writes `xdph.conf` for it (`portalPicker.enable`). A bubble shows while the screen is shared. It's on in newly generated configs.
- Commands can answer with output, a new `output` message, which `mochi ipc` prints.
- The compositor state says when the screen is captured, from Hyprland's `screencast` events.
- Recording can leave out the desktop audio: a speaker toggle next to the microphone one in the picker, the A key, the `audio` action, and `record_audio` for the default.
- Arch Linux packages in `packaging/arch`: `mochi` builds the latest release, `mochi-git` the latest commit. Until they are on the AUR, the installing page shows how to build them from a clone with `makepkg -si`.
- A hairline border and a soft shadow around the island and the bubbles, as the theme colors `border` and `shadow`; transparent turns either off. The border skips any edge the island is attached to in notch mode.

### Changed

- Screenshots and recordings open over anything on the island, the launcher, the hub or the clipboard included, instead of waiting for it to close. The frozen screen still shows it, so the shell itself can be captured, and it comes back afterwards.
- A notification or the media card you expanded with a click now takes the keyboard: Escape or a click outside closes it. Views that open on their own never take the keyboard.

### Fixed

- Apps started from the launcher no longer close when Mochi stops. uwsm gave them a scope of their own, but they stayed children of mochid, and an app that ends with its parent, like Discord in bubblewrap with `--die-with-parent`, closed with it. Apps, the screenshot editor, the folder opener and the power commands now start through a double fork, adopted by systemd.
- A light 1px line around the island and the bubbles: their blur reached past the drawn outline. It now stays a pixel inside.

## 0.0.3 - 2026-10-04

### Fixed

- Opening a screenshot no longer stretches the frozen screen. The island's window is now always as tall as the screen, so the compositor never animates a resize: Hyprland's `layers` animation stretched it while the window grew. `layout.surface_height` now caps the island itself.
- Recording works when the GPU's hardware encoder is unusable, as with NVENC on a GTX 1060, whose last driver (580) is older than the NVENC version nixpkgs' FFmpeg needs. The module asks `gpu-screen-recorder --info` which codecs work and picks a Vulkan one when no hardware codec is listed, with CPU encoding as the last resort. The `codec` setting forces one. Regions use gpu-screen-recorder's `-w WxH+X+Y`, as `-region` is deprecated.

## 0.0.2 - 2026-10-04

### Added

- Capture module: screenshots and recordings from the island. `mochi ipc capture screenshot` freezes the screens and opens in a default mode, a region unless set otherwise, with the island switching to a window or a screen; the capture is saved, copied, and shown with buttons to copy, edit, delete or open its folder. A screenshot takes under a tenth of a second after the pick. `record` records through gpu-screen-recorder, with a red dot next to the island while it runs; when it can't start, the island says why. It's on in newly generated configs.
- Overlays: an activity can draw a full-screen view on every monitor, under the island, with `ActivitySpec::overlay`. The protocol's activity has a new `overlay` field.
- Window positions from Hyprland's IPC, for modules that pick a window on screen.

### Changed

- The island's window no longer reserves the edge itself: a separate invisible strip does. The island now always starts at the screen edge, even with another bar there, and can cover the screen without moving windows.
- The Nix package brings gpu-screen-recorder, unless the home-manager settings name another recorder. The NixOS module also enables `programs.gpu-screen-recorder`, which recording a region or a screen needs.

### Fixed

- The Nix package's description, which broke its build in the commit after 0.0.1.

## 0.0.1 - 2026-10-04

The first release.

### The shell

- `mochid`, the daemon. It owns all state, runs the modules, and starts and supervises Quickshell 0.3.1. It restarts the UI if it crashes.
- `mochi`, the command-line client: `status`, `reload`, `ipc <module> <action>` and `config`.
- The island, at the top or bottom edge, sizes itself to what it shows and animates between views. Clicking expands or collapses it; right-clicking closes it.
- The island picks what to show by priority. A new activity can interrupt or queue behind the current one, and replaces any earlier one with the same key.
- Island and notch layouts. The notch touches the screen edge and has curved ears.
- Bubbles: small round status items in five areas along the edge. Modules choose where they go; `[bubbles.<module>]` in `config.toml` moves them, groups them or makes them wide pills.
- Modules can add cards and pages to other modules, such as the hub, and call each other's actions.
- The Obsidian theme, with built-in controls for views: buttons, sliders, switches, tiles, list rows and icons.

### Modules

- Idle: the clock, shown when nothing else is. A click opens the hub.
- OSD: volume, mute, the output device, microphone mute, Caps Lock and Num Lock, from PipeWire or PulseAudio.
- Workspaces: the workspace indicator, from `ext-workspace-v1`, on any compositor that supports it.
- Media: what's playing in any MPRIS player, with controls. A new track shows the player, then the music becomes a bubble.
- Notifications: a freedesktop notification server. A new message from an app replaces its previous one. Missed notifications become a bubble, and do not disturb silences them. Mochi waits behind swaync, mako or dunst and takes over when they stop.
- Launcher: fuzzy app search, most used apps first, and app actions. Starts apps through uwsm or a systemd scope.
- Hub: a panel with pages and cards from other modules.
- Power: lock, log out, suspend, hibernate, reboot, reboot to firmware and shut down, plus power profiles. It shows as a hub page. Actions that end the session ask for a second click.

### Configuration

- The first start writes a commented `config.toml` and `theme.toml` with every option and its default.
- `mochi config init`, `check` and `path`. Errors name the file, the section and the key.
- `mochi reload` applies both files without a restart. It starts, stops or restarts only the modules that changed, and keeps the running config if the new one has an error.

### Packaging

- A Nix flake with the package, an overlay, a home-manager module and a NixOS module, in `packaging/nix`. The home-manager module writes the config from Nix, checks it at build time, and reloads the shell when it changes.
- A systemd user unit, `mochid.service`, bound to `graphical-session.target`.
- A documentation site, built with mdBook from `docs/book`.
