# Settings

A panel with every option: the theme, the island, the bubbles, which modules run, each module's settings and each plugin's. A change applies as you make it. Theme options apply while you drag. A module's own options apply 400 ms after you stop, since the module restarts to read them.

`mochi ipc settings open` opens it, and `mochi ipc settings open osd` opens a section: a module's id, `colors`, `text`, `layout`, `motion`, `island`, `bubbles` or `modules`. An option's path, like `theme.colors.accent`, opens its section and points at it. The hub's gear opens the panel at the page you're on, and the launcher finds "Settings", each section and each option by name.

## Using it

- A dot marks an option that isn't at its default, and the section in the sidebar. Hovering the option shows a button back to the default, and Reset in the header does it for the whole section.
- Typing in the search box looks through every option's name and description. `@modified` lists the ones that aren't at their default. Up and Down move through the sidebar, Ctrl+F goes back to the search box, and Escape closes the menu, the editor, the search, then the panel.
- The font options list the fonts installed, each drawn in itself, with a search; a font that isn't installed shows in red.
- TOML shows the section as TOML, every option at its value, to edit by hand. Save checks it first and shows what's wrong instead of changing anything. Options for which the panel has no control, like the launcher's providers, are edited there.
- Copy gives everything that isn't a default as Nix, the `settings` and `theme` attributes of home-manager's `programs.mochi`, ready to paste. Copy as TOML gives `config.toml` and `theme.toml` instead.

## Where changes go

The panel never writes `config.toml` or `theme.toml`, which home-manager keeps read-only. It keeps its changes in `changes.toml` next to them, and `mochid` lays that file over both every time it reads them. A change goes away by itself once your files say the same, so pasting what Copy gave you and switching leaves nothing behind. Until then the changes survive restarts. "Undo all changes", at the bottom of the sidebar, drops them all.

When `changes.toml` has an option that no longer fits, after an update renamed it for example, `mochid` starts without the changes and logs why.

## Actions

| Action | |
|---|---|
| `open [section]`, `toggle`, `close` | The panel. |
| `set <path> <json>` | Sets an option, like `set theme.colors.accent '"#30d158"'`. `null` unsets it. |
| `reset <path>` | Puts an option, or each option of a section, back to its default. |
| `discard` | Drops every change. |
| `text <path>`, `edit <path> <toml>` | A section as TOML, and replacing it. |
| `export [nix\|toml]`, `copy [nix\|toml]` | Prints, or copies, every option that isn't at its default. |

The settings module has no settings of its own.
