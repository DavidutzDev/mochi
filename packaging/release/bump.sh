#!/bin/sh
# Sets Mochi's version to the one given everywhere it's written: Cargo.toml
# and Cargo.lock, the README, the docs, the protocol's examples and the
# example plugins' mochi-sdk lines. The patch release workflow runs it, and
# so does a minor release by hand (RELEASING.md).
#
#   packaging/release/bump.sh 0.2.0
set -eu

new=${1:?usage: bump.sh <version>}
cd "$(dirname "$0")/../.."

old=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
if [ "$old" = "$new" ]; then
    exit 0
fi
# The old version as a pattern: its dots match only dots.
was=$(printf '%s' "$old" | sed 's/\./\\./g')

# The workspace's version is the first one in Cargo.toml.
sed -i "0,/^version = \"$was\"/s//version = \"$new\"/" Cargo.toml
sed -i "s/at version $was,/at version $new,/" README.md
sed -i "s/\"version\":\"$was\"/\"version\":\"$new\"/g" docs/protocol.md docs/book/src/custom-sdks.md
sed -i "s|DavidutzDev/mochi/v$was|DavidutzDev/mochi/v$new|; s/--version $was\`/--version $new\`/" docs/book/src/installing.md
sed -i "s/tag = \"v$was\"/tag = \"v$new\"/g" docs/book/src/sdk.md docs/book/src/writing-plugins.md examples/plugins/*/Cargo.toml
sed -i "s/version: \"$was\".into(),/version: \"$new\".into(),/" \
    crates/mochi-protocol/src/messages.rs crates/mochi-protocol/src/plugin.rs crates/mochid/src/session.rs

# Only the workspace's own crates move in the lock file.
cargo update --workspace --offline --quiet 2>/dev/null || cargo update --workspace --quiet
