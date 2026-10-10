# Releasing

A release is a version tag on GitHub, like `v0.2.0`. `.github/workflows/release.yml` builds and publishes it.

Mochi has two kinds:

- **Patches**, like 0.1.1 and 0.1.2: fixes and small changes. CI cuts them from your pushes; you only write the changelog.
- **Minor and major releases**, like 0.2.0 and 1.0.0: new features, the tour's new steps, and, before 1.0, changes to the config format, the protocol or the module interface. You make them by hand.

## Patches

Each fix or small change adds its line to `## Unreleased` in `CHANGELOG.md`, under `### Added`, `### Changed` or `### Fixed`. Push to `main`, and `.github/workflows/patch.yml`:

1. Runs `packaging/release/patch.sh`, which takes the patch after the latest tag, like `0.1.2` after `v0.1.1`, writes it everywhere with `packaging/release/bump.sh`, and renames `## Unreleased` to `## 0.1.2 - <date>`.
2. Commits that as `Release 0.1.2`, tags it `v0.1.2` and pushes both.
3. Runs `release.yml` for the tag, as below.

Everything pushed since the last release goes into one patch, however many commits it is: nothing needs squashing. A push without entries under `## Unreleased` releases nothing, so docs, tests and the TODO can go out on their own. `git pull --rebase` before your next push, for the release's two commits.

A patch brings no tour steps: a step's `since` is always a minor release, like `0.2.0`, and `patch.sh` stops if one was added since the last tag. After a patch update, the tour shows its changelog entries instead, one card each. After an update to a new series, like 0.1.2 to 0.3.1, it shows only the new series' steps, and a first tour shows neither.

## Minor and major releases

### Before tagging

1. Set the version everywhere, `Cargo.toml`, `Cargo.lock`, the README, the docs, the protocol's examples and the example plugins' `mochi-sdk` lines, with `packaging/release/bump.sh <version>`.
2. Rename `## Unreleased` in `CHANGELOG.md` to `## <version> - <date>`. The release's notes are that section.
3. Tag the steps a new feature brings in the tour with `"since": "<version>"`, so people who update see them.
4. Add a line for the release to `TODO.md`.
5. Check it: `nix build .#mochi`, which runs the tests, and `nix build .#docs`.
6. Commit as `Release <version>: <what it brings>`, then tag: `git tag -a v<version> -m "Mochi <version>"`.

While `Cargo.toml` says a version that isn't tagged yet, `patch.sh` leaves the push alone.

### Publishing

```sh
git push origin main v<version>
```

## What release.yml does

For a tag pushed by hand, or one `patch.yml` made:

1. Checks that the tag is the version `Cargo.toml` says, and that `CHANGELOG.md` has its section.
2. Builds `mochi` and `mochid` for x86_64 and aarch64 on Ubuntu 22.04, so they run with glibc 2.35 or newer.
3. Packs each with `packaging/release/archive.sh`: the stripped binaries, the fonts, the systemd unit, the link handler, the completions, the licenses and `install.sh`, as `mochi-<arch>-linux.tar.gz` with a `.sha256`.
4. Creates the GitHub release, titled `Mochi <version>`, with the changelog's section as its notes.
5. Runs `packaging/release/record.sh`, which writes `packaging/nix/release.json` for the flake's `mochi-bin` and sets the version and sums in the Arch `PKGBUILD`s, and commits that to `main` as `Record release <version> for the packages`.

`git pull` afterwards to get that commit. If the last step failed, run `packaging/release/record.sh <version>` yourself once the release is on GitHub, and commit what it changed.

## After

- Start the next section of `CHANGELOG.md` with `## Unreleased` when the next change comes.
- The AUR packages, once they're published there, take the new `PKGBUILD`s.
