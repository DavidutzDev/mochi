# Plugins

Plugins add modules to Mochi. A plugin can do anything a builtin module does: show activities on the island and bubbles next to it, offer cards and pages to the hub, answer `mochi ipc` actions, publish state for its views, read other modules' state, and replace builtin views. A plugin is views in QML, usually with a backend: a program `mochid` starts and talks to. [Writing plugins](writing-plugins.md) shows how to make one: Mochi's [SDK](sdk.md) writes backends in Rust, and [any language](custom-sdks.md) works.

Plugins are trusted code, like the apps you install. Their views can run commands and their backends run as you, so install plugins you trust.

## Listing plugins

Plugins are listed in `plugins.toml`, next to `config.toml` in `~/.config/mochi/`, by id:

```toml
[plugins.pomodoro]
source = "git:github.com/User/mochi-pomodoro:main"

[plugins.weather]
source = "git-release:github.com/User/mochi-weather:v0.2.0"

[plugins.mine]
source = "path:~/code/my-plugin"
```

The source says where the plugin comes from and how it's installed:

| Source | Installs |
|---|---|
| `git:<host>/<user>/<repo>:<ref>` | Clones the repository and builds the plugin with the command in its manifest. `<ref>` is a branch, a tag or a commit; without `:<ref>`, the default branch. A URL with a scheme works too: `git:https://codeberg.org/User/repo:v1`. |
| `git-release:github.com/<user>/<repo>:<tag>` | Downloads the release asset the plugin's manifest names, already built: nothing to compile. Without `:<tag>`, the latest release. GitHub only for now. |
| `path:<dir>` | Uses the directory where it is, building it in place. For writing a plugin: its views hot-reload. `~` is your home directory, and a relative path starts next to plugins.toml. |

Building from `git:` needs what the plugin builds with, usually `cargo`, plus `git`. The Nix package brings `git`, `curl` and `tar`, not a Rust toolchain.

## Installing

```sh
mochi plugins install            # every plugin in plugins.toml that isn't installed
mochi plugins install pomodoro   # just this one
```

Before anything runs, `install` shows what the plugin is and what it will do, and asks:

```
Pomodoro 0.1.0 (pomodoro)
  A focus timer: a bubble counts down, the island says when to rest
  from    git:github.com/User/mochi-pomodoro:main at 4f1c2a9d0e
  runs    cargo build --release --locked --target-dir target && install -Dm755 target/release/pomodoro bin/pomodoro
          in /home/you/.local/share/mochi/plugins/pomodoro
  starts  bin/pomodoro with mochid
  reads   the state of media
  actions start, break, pause, stop, status
Install pomodoro? [y/N]
```

`--yes` skips the question. Installed plugins live in `~/.local/share/mochi/plugins/<id>/`, and a running `mochid` reloads to pick them up.

Then enable the plugin like a builtin module, in `config.toml`:

```toml
modules = ["idle", "osd", "media", "hub", "pomodoro"]

[module.pomodoro]
focus_minutes = 50
```

A plugin's settings go in its `[module.<id>]` section, like a builtin's. The plugin's `settings.toml` lists them.

### Without build tools

A `git:` plugin builds on your machine, with whatever its `build` command runs, often `cargo`. When the plugin's repository has a `flake.nix` and Nix is installed, `install` runs `nix build` on the flake instead, and nothing else needs to be installed. The build stays in the Nix store, kept from garbage collection by a link in `~/.local/share/mochi/plugins/.nix/`.

When a build fails, `install` names the tools missing and the ways around it that apply to the plugin:

```
mochi: chrono: the build failed (exit status: 127): cargo build --release ...
  `cargo` isn't installed. Ways around it:
  - install it, then run `mochi plugins install chrono` again
  - with home-manager, let Nix build it during the switch, with no tools here:
      programs.mochi.plugins.chrono.src = <a flake input of its repository>;
  - use its prebuilt releases, if it publishes them:
      chrono = { source = "git-release:github.com/Someone/mochi-clock" }
  - ask its author for a flake.nix: with Nix installed, Mochi builds a plugin's flake instead
```

On NixOS, a prebuilt release only runs if it's a static binary or `programs.nix-ld` is on, since a usual Linux binary looks for its loader in `/lib64`.

## Pinning and updating

`install` records what each plugin resolved to in `plugins.lock`, next to plugins.toml: the commit for `git:`, the tag and the asset's hash for `git-release:`. From then on, `install` installs exactly that, on this machine or another one with the same files, until you update:

```sh
mochi plugins update             # fetch every plugin's branch or latest release again
mochi plugins update pomodoro
```

A release asset that changed since the lock recorded it is refused. Changing a plugin's source in plugins.toml makes the next `install` resolve it again.

## Checking and removing

```sh
mochi plugins list
mochi status
```

`list` shows each plugin's source, what it's pinned at, its version, and whether it runs. A plugin is `running`, `disabled` when it's installed but not in `modules`, `missing` when it isn't installed or its manifest has an error, or `failed`. A plugin's backend that crashes starts again after a moment; after 5 crashes within a minute it stays stopped until `mochi reload`, and a notification says so.

```sh
mochi plugins remove pomodoro
```

deletes the installed files and the lock entry. Remove the plugin from plugins.toml too, or the next `install` brings it back.

A plugin's id can't be a builtin module's, and two plugins can't share an id. A plugin listed in plugins.toml counts as a module in `config.toml` even before it's installed, so the file stays valid on a new machine; it just doesn't run until it's installed.

## With home-manager

`programs.mochi.plugins` writes plugins.toml, so `config.toml` can enable plugins and still pass the build-time check:

```nix
programs.mochi = {
  enable = true;
  plugins.pomodoro = "git:github.com/User/mochi-pomodoro:main";
  settings.modules = [ "idle" "osd" "media" "hub" "pomodoro" ];
};
```

With a string, installing stays your step: run `mochi plugins install` after switching.

Nix can build the plugin during the switch instead, so the machine needs no build tools and there's no install step. Add the plugin's repository as a flake input and give it as `src`:

```nix
# flake.nix
inputs.mochi-pomodoro = {
  url = "github:User/mochi-pomodoro";
  flake = false;
};

# home-manager
programs.mochi.plugins.pomodoro = {
  src = inputs.mochi-pomodoro;
  # Native libraries its backend links, if any.
  buildInputs = [ pkgs.alsa-lib ];
};
```

This works for Rust plugins with a `Cargo.lock`, git dependencies included, and needs no hash. plugins.toml then points at the build in the Nix store, and `nix flake update mochi-pomodoro` moves it to the latest commit. A plugin with its own flake can be given as `package` instead: `programs.mochi.plugins.pomodoro.package = inputs.mochi-pomodoro.packages.${pkgs.system}.default;`.
