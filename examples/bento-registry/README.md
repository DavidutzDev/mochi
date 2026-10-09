# Bento

Plugins, themes and whole setups for [Mochi](https://github.com/DavidutzDev/mochi), each release pinned to the commit someone checked. Mochi reads this registry:

```sh
mochi bento search clock
mochi bento info pomodoro
mochi bento add pomodoro
```

The code stays in each package's own repository. This one only lists them, one file each in `packages/`, and publishes them together as `index.json`.

## Adding a package

In your package's repository, with the release tagged and pushed, `mochi bento publish` checks it, writes its file and opens the pull request; `examples/bento-publish.yml` in Mochi's source does it on each tag. By hand:

1. Make it work with `mochi bento add <your repository>`, and check it with `mochi bento check` in its directory.
2. Tag the release you want listed, and note its full commit: `git rev-parse v1.0.0`.
3. Add `packages/<id>.toml`, where `<id>` is the id in its manifest:

   ```toml
   kind = "theme"            # plugin, theme or bento
   name = "Dusk"
   description = "Purple evenings, with a dawn for light mode"
   repository = "https://github.com/you/mochi-dusk"
   maintainers = ["you"]     # GitHub names
   license = "MIT"
   tags = ["dark", "purple"]
   screenshots = ["screens/control-center.png"]   # paths in your repository

   [[release]]
   version = "1.0.0"         # what its manifest says
   commit = "4f1c2a9d0e7b5a3c1d2e3f4a5b6c7d8e9f0a1b2c"
   mochi = "0.0.8"           # the oldest Mochi it works with
   ```

4. Open a pull request. A new release is one more `[[release]]`.

## What gets checked

Every pull request runs `mochid bento registry check`: each file must read, with a full commit, an `https://` repository and someone to look after it. Then each changed package's newest release is cloned and read as `mochi bento add` would. Its id, version and `mochi` must match this file. A theme or a bento may hold only TOML, Markdown, text and pictures, and a theme's text must be readable on its cards in both versions. A bento's settings must be ones Mochi knows. A plugin's release is built with Nix, from its lock file.

## Who merges

- **Plugins are reviewed by a person.** Each new plugin, and each new release of one, waits for a maintainer of this registry to read its code. The pull request links to the code between the last release and the new one.
- **Themes and bentos merge on their own** once the checks pass, when they're new or their maintainers send them.
- **Everything else waits for a review:** a package that moves to another repository, changes maintainers or leaves, and changes to a package from someone who doesn't look after it.

## Withdrawing a release

A release that turns out broken gets `yanked = "why"`: Mochi stops installing it and warns where it's installed. One that does harm gets `malicious = "why"`: Mochi refuses to install it, and mochid stops running it. Keep the release in the file, so Mochi knows what it's looking at.

## Running a registry of your own

Copy this directory into a repository, then in its settings allow auto-merge, require the Check workflow on `main`, and publish Pages from GitHub Actions. Users add it to `bento.toml` next to their `config.toml`:

```toml
[registries]
friends = "https://<you>.github.io/<repository>/index.json"
```

and install from it with `mochi bento add bento:friends/<id>`.
