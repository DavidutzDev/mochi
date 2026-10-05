# Writing plugins

A plugin is a directory, usually a git repository, with a manifest, views and a backend:

```
my-plugin/
  mochi-plugin.toml     what it is, what it runs, what it offers
  settings.toml         its commented [module.<id>] section, for users
  qml/                  its views
  src/, Cargo.toml      the backend's source, for a Rust backend
```

The repository has example plugins to copy:

| Example | Shows |
|---|---|
| `examples/plugins/pomodoro` | A Rust backend: a countdown bubble updated every second, island notices, actions with arguments, a hub card with buttons, watching the media module's state |
| `examples/plugins/weather` | A Rust backend fetching from the web on a timer, settings with a place and units, a hub card and a forecast view |
| `examples/python/hello` | A Python backend with a small SDK of its own: see [Making an SDK](custom-sdks.md) |

To work on a plugin, list it with a `path:` source and install it, which runs its build:

```toml
# ~/.config/mochi/plugins.toml
[plugins.pomodoro]
source = "path:~/code/mochi-pomodoro"
```

```sh
mochi plugins install pomodoro   # builds it; mochid reloads
```

Views hot-reload as you save them. After rebuilding the backend, `mochi reload` restarts it: mochid restarts a plugin whose manifest or backend changed.

## The manifest

`mochi-plugin.toml` at the plugin's root names the plugin, its backend, its actions and what it offers. The smallest one, for a plugin of views only:

```toml
[plugin]
id = "clock"
name = "Big clock"
version = "0.1.0"
api = 1

[views]
overrides = ["idle/Pill"]
```

A plugin with a backend adds `[backend]` with the program and the command that builds it, and usually `[[actions]]` and `[[contributions]]`. [Plugin manifest](plugin-manifest.md) lists every key.

## Views

Views are QML files in the views directory, loaded the same way as a builtin module's: see [Writing views](views.md). An activity or bubble names a view by its file name without `.qml`, and a view gets the `payload` property. `import qs.island` gives the theme, `Daemon` and the controls. A view reads its plugin's state with `Daemon.state("<id>")` and runs its actions with `Daemon.command("<id>", "<action>", [args])`.

A hub card or page gets the plugin's published state as its `payload`, and a card can set `hidden: true` to step aside.

### Replacing builtin views

`overrides = ["idle/Pill"]` puts the plugin's `qml/overrides/idle/Pill.qml` in place of the idle module's `Pill.qml`, while both modules are enabled. The file sits in the idle module's directory, so it can use that module's other files and gets the same payload as the view it replaces. Without the plugin, the builtin view is back. When two plugins replace the same view, the one whose id sorts first wins.

## The backend in Rust

Depend on `mochi-sdk`, and hand your function to `mochi_sdk::run`. It gets a `ModuleCtx` with the same calls as a builtin module's:

```rust
use mochi_sdk::{ActivitySpec, ModuleCtx, ModuleEvent, json};

fn main() -> std::process::ExitCode {
    mochi_sdk::run(run)
}

async fn run(mut ctx: ModuleCtx) -> Result<(), mochi_sdk::Error> {
    while let Some(event) = ctx.next_event().await {
        match event {
            ModuleEvent::Command(command) if command.action == "say" => {
                ctx.present(
                    ActivitySpec::new("Hello")
                        .timeout(std::time::Duration::from_secs(3))
                        .payload(json!({ "text": "Hi" })),
                );
                command.reply(Ok(()));
            }
            ModuleEvent::Command(command) => command.reply(Err("no such action".into())),
            _ => {}
        }
    }
    // mochid closed the connection: time to go.
    Ok(())
}
```

[The Rust SDK](sdk.md) covers settings, activities, bubbles, state, calls, the compositor, logging and testing.

## A backend in any language

A backend is any program that speaks the [plugin protocol](protocol.md#plugin-backends): JSON lines over file descriptor 3. This one, in `sh`, answers an action with an activity. For more than a few lines, a small library helps: [Making an SDK](custom-sdks.md) explains how to write one, with a complete one in Python.

```sh
#!/bin/sh
read -r hello <&3
printf '{"type":"hello","api":1}\n' >&3
while read -r line <&3; do
    case "$line" in
    *'"type":"command"'*)
        id=${line#*\"id\":}
        id=${id%%,*}
        printf '{"type":"present","id":1,"spec":{"compact":"Hello","timeout_ms":3000}}\n' >&3
        printf '{"type":"reply","id":%s}\n' "$id" >&3 ;;
    esac
done
```

## Publishing a release

For a `git-release:` source, attach an archive to a GitHub release, named as `[release] asset` says with `{id}`, `{version}`, `{tag}` and `{arch}` (`x86_64` or `aarch64`) filled in. The archive holds the plugin as it should be installed: the manifest, the views and the built `exec`, at its root or in one directory. `mochi plugins install` reads the manifest from the tagged commit first, to show what it will install, then downloads the asset.
