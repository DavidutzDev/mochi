# Changelog

Mochi follows [semantic versioning](https://semver.org). Before 1.0, any minor release may change the config format, the protocol or the module interface; the changelog says when.

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
- `ActivitySpec::fleeting`: an activity that shows at once or not at all, never queued or suspended. The volume and workspace notices are fleeting, so they no longer show late after the hub, the launcher or the mixer closes. The volume notice now replaces a workspace notice on screen instead of waiting for it.

### Fixed

- Escape closes a modal view whatever inside it has the focus: the island takes the keyboard's focus for each one that doesn't take it itself, and Escape a view doesn't use comes up to the island.
- A plugin without views no longer makes Quickshell warn about an import it can't find.
- `mochi ipc hub open <page>` switches pages while the hub is open, also after a click on a tab.
- A notice that closes on a click outside, like the media notice, took every scroll until a click closed it, so a page under it couldn't scroll. A scroll outside now lets go of the screen, and the notice stays until it times out.
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
