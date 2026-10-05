# Plugin manifest

`mochi-plugin.toml` sits at the root of a plugin and says what it is, what it runs and what it offers. mochid reads it when it starts and on `mochi reload`; `mochi plugins install` reads it before installing, to show you. A key the manifest doesn't know is an error, so typos don't go unnoticed.

A complete one, from the pomodoro example:

```toml
{{#include ../../../examples/plugins/pomodoro/mochi-plugin.toml}}
```

## `[plugin]`

| Key | | |
|---|---|---|
| `id` | required | Its name in `config.toml`, `[module.<id>]`, `mochi ipc <id>` and `Daemon.state("<id>")`. Lowercase letters, digits, `-` and `_`, starting with a letter. It must match the id in plugins.toml, and can't be a builtin module's. |
| `name` | required | What `mochi plugins` shows. |
| `version` | required | The plugin's own version, any text. plugins.lock records it, and `{version}` in a release asset's name is it. |
| `api` | required | The protocol version the backend speaks: `1`. mochid refuses a plugin with another. |
| `description` | | One line about it. |
| `authors` | | A list of names. |
| `homepage` | | A URL. |

## `[backend]`

Leave it out for a plugin of views only, like one that only replaces a builtin view.

| Key | | |
|---|---|---|
| `exec` | required | The program mochid starts, relative to the plugin's directory. |
| `args` | | Its arguments. |
| `build` | | A shell command that builds `exec` from source. `mochi plugins install` runs it with `sh` in the plugin's directory for `git:` and `path:` sources, with `MOCHI_PLUGIN_DIR` set; never for `git-release:`. |

The backend starts in the plugin's directory, with the socket to mochid as file descriptor 3. See [the plugin protocol](protocol.md#plugin-backends).

## `[release]`

For `git-release:` sources.

| Key | | |
|---|---|---|
| `asset` | required | The release asset's file name. `{id}`, `{version}`, `{tag}` and `{arch}` are filled in; `{arch}` is `x86_64` or `aarch64`. |

The asset is a tar archive, compressed or not, holding the plugin as it should be installed, built `exec` included: at the archive's root, or in its only directory.

## `[views]`

| Key | Default | |
|---|---|---|
| `dir` | `"qml"` | Where the views are, relative to the plugin's directory. |
| `overrides` | `[]` | Builtin views it replaces, as `"<module>/<view>"`. The plugin's `<dir>/overrides/<module>/<view>.qml` takes the place of the module's `<view>.qml` while both are enabled. |

## `[uses]`

| Key | Default | |
|---|---|---|
| `state` | `[]` | Modules whose published state the backend receives, as `state` messages: once at the start, then on every change, and `null` when the module stops. A module that isn't enabled sends nothing. |

## `[[actions]]`

One table per action `mochi ipc <id> <action>` runs. mochid checks the words typed against the arguments before the backend sees them, and `mochi ipc <id>` lists the actions with their help.

| Key | | |
|---|---|---|
| `name` | required | One word. |
| `description` | required | One line, for `mochi ipc`'s list. |
| `args` | | A list of arguments, in order. |

An argument:

| Key | Default | |
|---|---|---|
| `name` | required | The name the backend reads it by. |
| `description` | `""` | One line. |
| `kind` | `"string"` | `string`, `int`, `float`, `bool` (`true`/`false`, `on`/`off`, `yes`/`no`) or `choice`. |
| `choices` | `[]` | The words a `choice` takes. |
| `optional` | `false` | May be left out; only trailing arguments can be optional. |
| `rest` | `false` | Takes every remaining word, joined with spaces; only the last argument. |

The backend gets the arguments as an object: `{"minutes": 25, "label": "Write"}`. A left-out optional argument isn't there.

## `[[contributions]]`

What the plugin offers other modules, like a card or a page for the hub. The view gets the plugin's published state as its `payload`.

| Key | | |
|---|---|---|
| `target` | required | The module it's for, like `hub`. |
| `kind` | required | What it is to the target: the hub takes `card` and `page`. |
| `id` | required | Unique among the plugin's contributions. |
| `view` | required | The view's file name, without `.qml`. |
| `title` | required | Its heading. |
| `icon` | | A Mochi symbol, like `clock`, or an icon theme name. |
| `order` | `0` | Lower comes first. |
| `options` | | Anything else the target reads, like `{ span = 2 }` for a hub card two columns wide. |

## `settings.toml`

Not part of the manifest, but next to it by convention: the plugin's `[module.<id>]` section with every setting commented out at its default, for users to copy into `config.toml`. The backend gets the section in `hello`.
