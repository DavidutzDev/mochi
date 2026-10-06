# Configuration

Mochi reads two files in `~/.config/mochi/` (or `$XDG_CONFIG_HOME/mochi/`):

- `config.toml`: which modules run, and their settings.
- `theme.toml`: how everything looks. See [Theme](theme.md).

Both are written on the first start, with every option commented. A line like `# timeout_ms = 1500` shows a default: remove the `# ` to change it. Anything left out keeps its default, so a file can be as short as the one line you change.

## Checking and applying

`mochi config check` checks both files with the same rules `mochid` uses, without a running daemon. Every error names the file, the section and the key:

```text
config.toml: [module.osd]: unknown field `timeot_ms`, expected one of `timeout_ms`, `volume`, `device`, `microphone`, `locks`
```

`mochi reload` applies both files to the running daemon. Modules you added start, modules you removed stop, and modules whose settings changed restart; the others keep running. A file with an error changes nothing, and `mochi reload` prints the error.

`mochi config init` writes the example files where they're missing, and `mochi config init --print` shows them without writing.

## Modules

```toml
modules = ["idle", "osd", "workspaces", "media", "notifications", "launcher", "hub", "power"]
```

The modules to run, in this order. Each module's settings live in a `[module.<id>]` section; the [module pages](modules/idle.md) list them. A module left out of the list doesn't run, and its section is ignored.

`mochid --modules idle,osd` overrides the list for one run, which helps when trying things.

## The island

```toml
{{#include ../../../crates/mochi-core/defaults/island.toml}}
```

`panels` decides where the views you type into open. On Hyprland, `"pointer"` asks the compositor where the mouse is; elsewhere it opens them where the keyboard is.

`notices` does the same for everything else the island shows: notifications, the volume, workspace switches, the media card, any module's notice. With `"focus"` or `"pointer"`, a notice shows on that one monitor, picked when it comes, and the other monitors keep the idle island. The default, `"all"`, shows it on every monitor's island.

`click_outside` decides what a click outside the island closes. With `"all"`, a notification you never opened goes to the missed ones, as if its time had run out, and the media card goes back to its bubble. The volume and workspace notices never catch clicks, since they show while you're busy elsewhere. While a notice you didn't open catches clicks, it also takes the scroll wheel, since a Wayland surface can't pass events on to the window under it. So scrolling outside it lets go: that scroll is lost, the ones after it reach the window, and the notice stays until its time runs out. Some compositors, like Sway, only see the change once the pointer moves. A view you opened with a click, or one you type into, is closed for good, and takes the keyboard while it's open so Escape closes it too.
