# Settings

A panel with every option: the theme, the island, the bubbles, which modules run, each module's settings and each plugin's. A change applies as you make it. Theme options apply while you drag. A module's own options apply 400 ms after you stop, since the module restarts to read them. Its Bento pages install plugins, themes and whole setups from the registry, and share yours: see [Bento](../bento.md#in-the-settings).

`mochi ipc settings open` opens it, and `mochi ipc settings open osd` opens a section: a module's id, `colors`, `text`, `layout`, `motion`, `island`, `bubbles`, `modules` or `about`. An option's path, like `theme.colors.accent`, opens its section and points at it. The control center's gear opens the panel at the page you're on, and the launcher finds "Settings", each section and each option by name.

## Using it

- A dot marks an option you changed in the panel, and its section in the sidebar. Hovering the option shows a button that undoes the change, back to what your files say, and Reset in the header does it for the whole section. Double-clicking a slider does the same.
- Typing in the search box looks through every option's name and description. `@modified` lists the ones you changed in the panel. Up and Down move through the sidebar, Ctrl+F goes back to the search box, and Escape closes the menu, the editor, the search, then the panel.
- The font options list the fonts installed, each drawn in itself, with a search; a font that isn't installed shows in red.
- Time zones, like the clock's `zones`, pick from every zone the system has, with a search that finds a city, a country or an offset like `utc+9`, the same picker as the clock's World tab. Up, Down and Enter work in it, and a zone's name the list doesn't have can be typed.
- A file, like the timer's `sound_file` or the theme's `wallpaper`, has a field to type its path and **Choose…**, which opens the desktop's own file chooser through the XDG desktop portal, showing only sound files or images where the option says so. Without a portal, or one without a file chooser, `zenity` or `kdialog` opens one; with none of them, the option says so and the field still takes a path.
- Options whose values come from somewhere pick them from a menu with a search, and lists pick several: the apps installed, with their icons, for the clipboard's ignored apps and terminals; the outputs and microphones for a recording's audio; the tray's apps for `pinned` and `hidden`; the players for media's `ignore`; the control center's cards for its order. Typing a value that isn't there works too where it makes sense. A command, like the idle clock's click, picks a module, then one of its actions with what it does, then each of its arguments on a row of its own: a menu for a choice, or for what Mochi knows, like the monitors, the control center's pages, the audio devices, the apps or the settings' sections; a switch for on and off; a field for the rest. Command lines, like the lock or the terminal, offer ready-made ones for the programs installed.
- Every change applies at once and is kept, so what you see is what you get. When something else tries settings without keeping them, like `mochi ipc settings preview`, a bar offers **Keep**, which makes them changes like any other, or **Drop**. A reload drops them too, and Copy leaves them out.
- TOML shows the section as TOML, every option at its value, to edit by hand. Save checks it first and shows what's wrong instead of changing anything. Options for which the panel has no control, like the launcher's providers, are edited there.
- Copy gives everything that isn't a default as Nix, the `settings` and `theme` attributes of home-manager's `programs.mochi`, ready to paste. Copy as TOML gives `config.toml` and `theme.toml` instead.

## About

The About page, at the bottom of the sidebar, says which Mochi runs and where: the version and build, the revision for a build from the source tree, Quickshell's version, the distribution, kernel, architecture, processor and memory, the desktop and compositor with its screens, the modules and plugins that run, and the memory and uptime of mochid and of the processes it started. "Copy details" puts all of it on the clipboard as text, to paste into a bug report; "Report a bug" opens a new issue on GitHub. A value in red says something is wrong, like a plugin that failed or a Quickshell version Mochi doesn't support.

## Where changes go

The panel never writes `config.toml` or `theme.toml`, which home-manager keeps read-only. It keeps its changes in `changes.toml` next to them, and `mochid` lays that file over both every time it reads them. A change goes away by itself once your files say the same, so pasting what Copy gave you and switching leaves nothing behind. Until then the changes survive restarts. "Undo all changes", at the bottom of the sidebar, drops them all.

When `changes.toml` has an option that no longer fits, after an update renamed it for example, `mochid` starts without the changes and logs why.

## Actions

| Action | |
|---|---|
| `open [section]`, `toggle`, `close` | The panel. |
| `set <path> <json>` | Sets an option, like `set theme.colors.accent '"#30d158"'`. `null` unsets it. |
| `reset <path>` | Undoes the change to an option, or to each option of a section, back to what the files say. |
| `discard` | Drops every change. |
| `preview <path> <json>`, `keep`, `drop` | Tries an option without keeping it, then keeps or drops what's being tried. |
| `try <json>` | Tries whole tables of settings, like `{"config": {...}, "theme": {...}}`, in place of what was being tried; `mochi bento try` sends it. |
| `bento-catalog [refresh]`, `bento-plan <source>`, `bento-show <source>` | Read Bento's registry and what it installed, or say what installing something does; `show` opens the Discover page on it, as `mochi://bento/` links do. See [Bento](../bento.md#in-the-settings). |
| `bento-add <source> [commit]`, `bento-remove <id>`, `bento-update [id]`, `bento-try <source>`, `bento-use <id\|mine>`, `bento-share <dir> [theme or parts] [name]`, `bento-parts` | What the Bento pages' buttons run, with `mochid bento`. `bento-forget` closes what a plan or a share shows. |
| `text <path>`, `edit <path> <toml>` | A section as TOML, and replacing it. |
| `export [nix\|toml]`, `copy [nix\|toml]` | Prints, or copies, every option that isn't at its default. |
| `about`, `about-copy` | Read what the About page shows, and copy it as text for a bug report. |
| `open-link <page>` | Opens `repository`, `documentation`, `issue` (a new issue on GitHub) or `mochi-clock` (credited by the clock and the timer) in the browser, with `xdg-open`. Nothing else opens. |
| `choose-file <path>` | Opens a file chooser for a file option, like `config.module.timer.sound_file`, and sets the option to the file picked; prints the path, or nothing when the chooser was closed. Another one while it's open takes its place. |

The settings module has no settings of its own.
