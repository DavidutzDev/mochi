# Mochi

A desktop shell for Wayland, built around one island at the top of your screen.

Instead of a bar full of icons, Mochi has an island, like the Dynamic Island on a phone. It shows what matters right now, a notification, the volume you just changed, the song that started, then gets out of the way. Small bubbles beside it keep an eye on what's ongoing, like the music playing or a recording. Open it for a launcher, a control center with your quick settings, and a settings panel for everything else.

Mochi works on Hyprland, niri and Sway. It's young, at version 0.0.9, and already the whole shell its author uses every day.

## Why Mochi

### More than a Quickshell config

Mochi draws its interface with [Quickshell](https://quickshell.org), like many shells. Most of them keep everything in QML: the looks, but also the logic, the timers and the talking to the system. Mochi moves all of that into `mochid`, a daemon written in Rust, and leaves QML only the drawing.

- **It stays smooth.** The work happens in compiled code, away from what draws, so a busy moment doesn't make an animation stutter.
- **It's tested.** The rules for what shows and each feature's behavior run in a test suite, end to end.
- **A crash loses nothing.** If the interface crashes, `mochid` restarts it and puts everything back where it was.
- **Add-ons are simple.** Plugins can be written in any language. Themes and whole setups install in one command with Bento. Your settings live in a file or a settings panel, not in code.

### A shell to install, not a dotfiles repository to copy

Most shells you see on [r/unixporn](https://www.reddit.com/r/unixporn/) are someone's own setup: you clone it, edit their code to make it yours, and updating means merging their changes into yours. Mochi is made to be installed.

- **It has releases.** One command installs Mochi and updates it.
- **Your settings stay yours.** They live apart from the code, in files and a panel, so an update never undoes them.
- **It isn't tied to one machine.** Mochi uses standard Wayland protocols, so it runs on several compositors and screen layouts.
- **Every option is documented**, and checked when it loads, so a typo says where it is.

[How it works](https://davidutzdev.github.io/mochi/how-it-works.html) explains the design in more detail.

## Features

- 🏝️ An island on every screen that shows one thing at a time, and bubbles beside it for what's ongoing
- 🔔 Notifications, with replies, a history and do not disturb
- 🎵 What's playing in any player, with the cover and controls
- 🔊 Volume and brightness as you change them, and a mixer with a slider per app
- 🚀 A launcher for apps, open windows, files, calculations, web searches and emoji
- 🧭 A control center with your quick toggles, cards and pages
- 📸 Screenshots and screen recordings, even switching screens while recording
- 🖥️ A screen-share picker that changes what you share without the app asking again
- 📋 A clipboard history, with text and images
- 📶 Wi-Fi, Ethernet and VPNs
- 🔵 Bluetooth devices, with their battery
- 🔋 Your laptop's battery, and your mouse's or controller's
- 🌙 Night light, and ☕ keeping the screen awake
- 🎙️ A sign when the microphone or the camera is in use, with a mute
- 📊 CPU, memory, GPU and temperatures, with a warning when one stays high
- 🧩 Tray icons for apps like Discord and Steam
- 📦 Files dropped on the island, converted, compressed or extracted
- 🎨 A color picker for anything on screen, and 😀 an emoji picker
- 🗒️ Desktop widgets in many looks, from a drawer with live previews: clocks, calendars, weather, notes, to-do lists, what's playing, system rings, and layouts to switch between
- 🌤️ The weather, from Open-Meteo, with no account
- 🕰️ A clock panel: today with the weather, a calendar with reminders, a stopwatch and world clocks
- ⏱️ A focus timer, with breaks
- 🤖 Your coding agents on the island: working, waiting for you, or done
- 🔒 Lock, log out, suspend, reboot and shut down
- ⚙️ A settings panel for every option, applied as you change it
- 🍱 Bento, to install and share themes, plugins and whole setups
- 🔄 Updates: a notice when a release is out, its changelog, and the update the way you installed Mochi
- 🔌 Plugins, in any language

## Install

On most Linux systems, one command installs the latest release, and updates it when you run it again:

```sh
curl -fsSL https://raw.githubusercontent.com/DavidutzDev/mochi/main/install.sh | sh -s -- --enable
```

Mochi needs [Quickshell](https://quickshell.org) 0.3 and PulseAudio's library, `libpulse`, from your distribution. `--enable` starts it with your session.

- **Nix and NixOS:** the flake has a package, a home-manager module and a NixOS module.
- **Arch Linux:** `packaging/arch` has `mochi`, `mochi-bin` and `mochi-git`.
- **Anything else:** `MOCHI_FROM_SOURCE=1` before `sh` builds it from source.

[Installing](https://davidutzdev.github.io/mochi/installing.html) has the details for each.

## Learn more

The [documentation](https://davidutzdev.github.io/mochi/) covers everything:

- [Getting started](https://davidutzdev.github.io/mochi/getting-started.html): your first keybinds, and where things are
- [Configuration](https://davidutzdev.github.io/mochi/configuration.html) and [Theme](https://davidutzdev.github.io/mochi/theme.html): every option
- [How it works](https://davidutzdev.github.io/mochi/how-it-works.html): the daemon, the island and modules
- [Plugins](https://davidutzdev.github.io/mochi/plugins.html) and [Bento](https://davidutzdev.github.io/mochi/bento.html): adding to Mochi, and sharing
- [Writing plugins](https://davidutzdev.github.io/mochi/writing-plugins.html): making your own

## Working on Mochi

Everything runs from the dev shell, `nix develop`:

```sh
cargo run -p mochid        # the shell, with views that reload as you edit them
cargo test --workspace     # the tests
```

A debug build like this one runs in dev mode. Only one Mochi runs per session, so the dev shell takes over from the one already running, which starts again by itself when you stop the dev one. A bubble on the left shows the revision the dev shell runs. `--no-dev` runs a debug build as an installed shell runs, and release builds never run in dev mode.

`TODO.md` has the plan, `CHANGELOG.md` what each release brought, and `RELEASING.md` how to make one.

## Credits

- [Quickshell](https://quickshell.org), by outfoxxed and its contributors, draws Mochi's interface. Mochi wouldn't exist without it.
- The [r/unixporn](https://www.reddit.com/r/unixporn/) community, whose desktops showed what a Linux desktop can look like, and inspired this one.
- [Inter](https://rsms.me/inter/), by Rasmus Andersson, for the text, and [Material Symbols](https://fonts.google.com/icons), by Google, for the icons.
- The palettes of [Catppuccin](https://catppuccin.com), [Nord](https://www.nordtheme.com), [Gruvbox](https://github.com/morhetz/gruvbox), [Rosé Pine](https://rosepinetheme.com) and [Tokyo Night](https://github.com/enkia/tokyo-night-vscode-theme), which Mochi's themes follow.

## License

[MIT](LICENSE).
