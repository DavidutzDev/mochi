# Releasing

A release is a version tag pushed to GitHub. `.github/workflows/release.yml` does the rest.

## Before tagging

1. Set the version in `Cargo.toml`'s `[workspace.package]`, and run `cargo build` so `Cargo.lock` follows.
2. Change the other places that name it: `README.md` (its summary), `docs/book/src/installing.md`, `docs/book/src/sdk.md`, `docs/book/src/writing-plugins.md`, `docs/book/src/custom-sdks.md`, `docs/protocol.md`, the `hello` examples in `crates/mochi-protocol/src/messages.rs` and `plugin.rs`, and the `mochi-sdk` lines in `examples/plugins/*/Cargo.toml`. `grep -rn "<old version>"` finds them.
3. Rename `## Unreleased` in `CHANGELOG.md` to `## <version> - <date>`. The release's notes are that section.
4. Tag the steps a new feature brings in the tour with `"since": "<version>"`, so people who update see them.
5. Add a line for the release to `TODO.md`.
6. Check it: `nix build .#mochi`, which runs the tests, and `nix build .#docs`.
7. Commit as `Release <version>: <what it brings>`, then tag: `git tag -a v<version> -m "Mochi <version>"`.

## Publishing

```sh
git push origin main v<version>
```

The workflow then:

1. Checks that the tag is the version `Cargo.toml` says, and that `CHANGELOG.md` has its section.
2. Builds `mochi` and `mochid` for x86_64 and aarch64 on Ubuntu 22.04, so they run with glibc 2.35 or newer.
3. Packs each with `packaging/release/archive.sh`: the stripped binaries, the fonts, the systemd unit, the link handler, the completions, the licenses and `install.sh`, as `mochi-<arch>-linux.tar.gz` with a `.sha256`.
4. Creates the GitHub release, titled `Mochi <version>`, with the changelog's section as its notes.
5. Runs `packaging/release/record.sh`, which writes `packaging/nix/release.json` for the flake's `mochi-bin` and sets the version and sums in the Arch `PKGBUILD`s, and commits that to `main` as `Record release <version> for the packages`.

`git pull` afterwards to get that commit. If the last step failed, run `packaging/release/record.sh <version>` yourself once the release is on GitHub, and commit what it changed.

## After

- Start the next section of `CHANGELOG.md` with `## Unreleased`.
- The AUR packages, once they're published there, take the new `PKGBUILD`s.
