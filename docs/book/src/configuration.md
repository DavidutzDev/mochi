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
