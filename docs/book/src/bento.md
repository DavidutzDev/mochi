# Bento

Bento shares setups, themes and plugins. A bento is a whole setup in one directory: the settings, the theme, the widgets and the plugins they need. You make one from what you run with one command, push it to a git repository, and anyone installs it with another.

```sh
mochi bento search clock                 # look through the registry
mochi bento add pomodoro                 # install from it
mochi bento share ~/cozy                 # make a bento of your setup
mochi bento add github.com/someone/cozy  # install one from anywhere
mochi bento try github.com/someone/dusk  # try a theme or a bento, then Keep or Drop
mochi bento remove cozy                  # take it out again
mochi bento list                         # what Bento installed
mochi bento update                       # move to newer releases
```

`add`, `try` and `remove` work for all three kinds: a directory with a `mochi-bento.toml` is a bento, one with a `mochi-theme.toml` a [theme](theme.md#theme-packages), and one with a `mochi-plugin.toml` a [plugin](plugins.md). Themes and bentos hold only TOML and images, so they run nothing; a plugin is code, and asks before it installs, as `mochi plugins install` does.

## Where from

`add` and `try` take:

| Given | Means |
|---|---|
| `pomodoro`, `bento:pomodoro`, `bento:pomodoro:0.3.0` | A package in the [registry](#the-registry): the newest release this Mochi runs, or the one named. |
| `~/cozy`, `./cozy`, `path:~/cozy` | A directory, used where it is. |
| `github.com/<user>/<repo>`, `gh:<user>/<repo>` | A GitHub repository, cloned at its default branch. |
| `https://gist.github.com/<user>/<id>` | A gist, which is a git repository too: one with only a `mochi-bento.toml` is a whole bento. |
| `git:<url>:<ref>`, `git-release:…` | A source as `plugins.toml` takes it, with a branch, a tag or a commit. |
| `bento:friends/pomodoro` | A package in another registry, see [Other registries](#other-registries). |

A repository is cloned to a scratch directory and read before anything happens; cloning runs none of its code.

## Making one

```sh
mochi bento share ~/cozy --name "Cozy"
```

`share` writes `~/cozy/mochi-bento.toml` from the setup running now: what your files and the settings panel set that isn't a default, the theme, the widgets and the plugins they use. The directory's name is the bento's id. A theme you installed goes along in `themes/<id>/`, and `--wallpaper` brings the wallpaper awww, swww or hyprpaper shows. `--print` prints the manifest instead, to paste into a gist.

It leaves out what belongs to your machine or to you, and lists each thing it left out:

- Options whose values are this machine's devices, like a recording's audio output.
- Where you are, like night light's `latitude` and `longitude`, and keys that hold secrets, like `token`, `password` or `api_key`.
- Paths in your home directory. A path written with `~/` stays, since it works on any machine.
- Values that look like tokens: long runs of letters and digits.
- Plugins in a local directory (`path:`), which nobody else can install, and the widgets they offer.
- Widgets on a screen that isn't connected.

Read the list, add a description and screenshots to the manifest, then push the directory to a git repository.

## Installing one

```
$ mochi bento add github.com/someone/cozy
Fetching git:github.com/someone/cozy…

Cozy 1.0.0 (cozy)
  Warm colors, a clock and notes on the big screen
  by someone
  from      git:github.com/someone/cozy at d34ba43578
  settings  12 options, 2 of the theme
  themes    1 brought along
  plugins   pomodoro
  widgets   3 on 2 screens
  needs     pomodoro from git:github.com/someone/mochi-pomodoro, which asks before it installs
  Its settings go over yours, in changes.toml, and its widgets replace yours. `mochi bento remove cozy`
  puts back what it replaces.
Add cozy? [y/N]
```

Before installing anything, `add` checks the bento's settings the way `mochi config check` would, so a typo or an option this Mochi doesn't know changes nothing. Then it installs the themes it brings and the plugins it needs, each plugin showing what it runs and asking. Its settings go into `changes.toml`, over your files, like changes from the [settings panel](modules/settings.md): a dot marks each one there, Copy hands them to your Nix or TOML config, and they work while home-manager owns your files. Its widgets replace `widgets.toml`, and its wallpaper is set with awww or swww when one runs. If any step fails, what it installed is removed and nothing changes.

`mochi bento remove cozy` puts `changes.toml` and `widgets.toml` back as they were before the bento, and removes the themes and plugins it brought. Changes you made after adding it go too, and `remove` says so before it asks. Adding a bento again updates it, and `remove` still goes back to before the first time.

`--yes` skips the questions, plugins' included.

## Trying

`mochi bento try` applies a theme, or a bento's settings and theme, without keeping them, through the settings panel's preview. It opens the panel, whose bar at the bottom has **Keep**, which makes them changes like any other, and **Drop**, which goes back; `mochi ipc settings keep` and `drop` do the same, and a reload drops them too. It needs the settings module running. A bento's widgets, wallpaper and plugins only come with `add`; `try` leaves out the modules whose plugins aren't installed, and says which. The themes it needs stay installed after a Drop, and `mochi bento remove <theme>` takes one out.

## The registry

Bento's registry lists plugins, themes and bentos whose releases someone checked. Each release is pinned to a commit: installing clones the package's own repository at that commit, the code a reviewer read, never a branch that moved since. A person reads the code of every plugin release before it's listed; themes and bentos, which run nothing, are listed once checks pass.

```
$ mochi bento search purple
dusk                 theme   0.2.0      Purple evening
$ mochi bento info dusk
Dusk (dusk), a theme
  Purple evening
  by someone, MIT
  repository https://github.com/someone/mochi-dusk
  0.3.0      1c522779b3 needs Mochi 0.0.9
  0.2.0      6c2755f1dc what `add` installs
Install it with `mochi bento add dusk`.
```

`add` installs the newest release that the running Mochi supports, so an older Mochi still gets something that works. A name alone means the registry, unless a directory by that name is where you run it. Plugins from the registry are recorded as `bento:<id>` sources, so `mochi plugins install` on another machine installs the same release, and `plugins.lock` keeps its commit.

`mochi bento update` moves what Bento installed to newer releases: plugins to the newest the registry lists, and themes and bentos from a repository to its newest commit. A bento asks again before laying its settings over yours. With ids, it updates only those; a directory is updated only when named.

Mochi downloads the registry's `index.json` when it needs it and keeps it in `~/.cache/mochi/bento/` for ten minutes, or longer when the registry can't be reached.

### Withdrawn releases

The registry can withdraw a release. A yanked one is no longer installed, and `mochi bento list` and mochid's log say so where it is. One withdrawn as harmful is refused, and mochid doesn't start it: `mochi status` names it and says to remove it. mochid only reads the index Mochi last downloaded, so a search, an install or an update brings the news.

### Other registries

`[registries]` in `bento.toml` adds registries, by name, with their index's URL:

```toml
[registries]
friends = "https://friends.example/index.json"
```

`mochi bento add bento:friends/pomodoro` installs from it. A `bento` entry points the default registry elsewhere. Anyone can run one: the registry's repository is a template in Mochi's source, in `examples/bento-registry`, with its checks.

### Publishing

The registry is a git repository with a file per package, `packages/<id>.toml`, naming its repository, its maintainers, its license and each release's version, commit and oldest Mochi. A pull request adds one. Its README says what's checked and who merges. `mochid bento registry check`, `index` and `diff` are the tools its CI runs, and `mochi bento check` runs the same checks on your package before you send it.

## bento.toml

What Bento installed is in `bento.toml`, next to `config.toml`. Bento writes it, never home-manager, so installing works while home-manager owns your config:

```toml
[plugins.pomodoro]
source = "git:github.com/someone/mochi-pomodoro"
version = "0.3.0"
by = "cozy"

[themes.mint]
source = "git:github.com/someone/cozy"
revision = "d34ba435786f27e4d50f7ac6feaf488914513d77"
version = "1.0.0"
by = "cozy"

[bentos.cozy]
source = "git:github.com/someone/cozy"
revision = "d34ba435786f27e4d50f7ac6feaf488914513d77"
version = "1.0.0"

[registries]
friends = "https://friends.example/index.json"
```

Its plugins count as if `plugins.toml` listed them: `mochi plugins install` installs them on a new machine, `mochi plugins list` shows them, and `plugins.toml` wins when both list an id. `by` names the bento that brought something, which removing the bento removes too. The settings panel's Copy as Nix includes its plugins as `plugins`, for `programs.mochi.plugins`. Themes go in `~/.local/share/mochi/themes/`, and what a bento replaced in `~/.local/share/mochi/bentos/<id>/`.

`[registries]` is the one part you edit yourself: see [Other registries](#other-registries).

## mochi-bento.toml

```toml
[bento]
id = "cozy"
name = "Cozy"
version = "1.0.0"
mochi = "0.0.7"
description = "Warm colors, a clock and notes on the big screen"
authors = ["someone"]
homepage = "https://github.com/someone/cozy"
screenshots = ["screens/desk.png"]
wallpaper = "wallpaper.jpg"

[plugins]
pomodoro = "git:github.com/someone/mochi-pomodoro"

[config]
modules = ["idle", "osd", "hub", "widgets", "notes", "pomodoro"]

[config.module.osd]
timeout_ms = 2500

[theme]
preset = "mint"
appearance = "auto"

[[widget]]
id = "w1"
module = "widgets"
widget = "clock"
output = "screen-1"
anchor = "top-right"
x = -2
y = 3
width = 14
height = 7
```

`[bento]` says what it is. `id` is lowercase letters, digits, `-` and `_`, starting with a letter. `mochi` is the oldest Mochi it works with: an older one refuses it and says why. `screenshots` and `wallpaper` are paths inside the bento.

`[plugins]` lists the plugins it needs, by id, with a source as `plugins.toml` takes it, like `bento:pomodoro` for one in the registry. A `path:` source can't be shared, so it isn't allowed.

`[config]` goes over `config.toml` and `[theme]` over `theme.toml`, with the same keys. Tables merge key by key, and anything else replaces what was there, so `modules` is the whole list. `preset` can name a theme the bento brings in `themes/<id>/mochi-theme.toml`.

`[[widget]]` is a widget as `widgets.toml` places it, except `output`: `screen-1` is the largest screen, `screen-2` the next, and so on, so widgets land on the same kind of screen whatever the outputs are called. A widget for a screen the machine doesn't have keeps `screen-2` or the like as its `output`, which matches no monitor, so it stays hidden until you set `output` to a monitor's name in `widgets.toml`.

`mochi bento check <dir>` reads a bento, a theme or a plugin as `add` would, without installing anything.
