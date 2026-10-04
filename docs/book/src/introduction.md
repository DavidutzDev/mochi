# Mochi

Mochi is a desktop shell for Wayland built around a central island, like the Dynamic Island on a phone. The island shows what matters right now: a notification, the volume you just changed, the song that started. Small round bubbles next to it keep things in view, like the music playing or missed notifications. A hub panel grows out of the island with cards and pages, and a launcher finds your apps.

It has two parts:

- `mochid`, a daemon written in Rust. It owns every piece of state and talks to the system: D-Bus, PipeWire, logind, the compositor. It decides what the island shows.
- A Quickshell UI that `mochid` starts and keeps running. It only draws what the daemon tells it to.

`mochi` is the command-line client. `mochi ipc <module> <action>` runs any module's action, which is how you bind keys to Mochi.

Everything is a module: the clock, the notifications, the launcher. Each one can be turned off, and the island makes room for what's left. Mochi uses standard Wayland protocols, so it works on any compositor that supports them, with extras where a compositor offers more.

Mochi is early work in progress.
