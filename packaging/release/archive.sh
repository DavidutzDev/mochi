#!/bin/sh
# Packs a release archive from binaries already built:
#
#   packaging/release/archive.sh <bin-dir> <out-dir>
#
# <bin-dir> holds `mochi` and `mochid`. The archive, mochi-<arch>-linux.tar.gz,
# unpacks to mochi-<version>/ with the binaries, the fonts the shell loads,
# the systemd unit, the mochi:// link handler, shell completions, the
# licenses and install.sh. Its name has no version, so
# https://github.com/DavidutzDev/mochi/releases/latest/download/<name> always
# gives the newest. A .sha256 file goes next to it.
set -eu

bin=$(cd "$1" && pwd)
mkdir -p "$2"
out=$(cd "$2" && pwd)
root=$(cd "$(dirname "$0")/../.." && pwd)

version=$("$bin/mochi" --version | awk '{ print $2 }')
arch=$(uname -m)
name="mochi-$arch-linux"
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
dir="$stage/mochi-$version"

# The fonts, pinned: Inter for text, Material Symbols Rounded for icons.
inter_url=https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip
inter_sha=9883fdd4a49d4fb66bd8177ba6625ef9a64aa45899767dde3d36aa425756b11e
symbols_commit=bd8cb85bd4bad964fe6918f79665bb40c3a8efef
symbols_url="https://raw.githubusercontent.com/google/material-design-icons/$symbols_commit/variablefont/MaterialSymbolsRounded%5BFILL%2CGRAD%2Copsz%2Cwght%5D.ttf"
symbols_sha=c2182b6337495e64cc9e2311c52522567ac277d25a842bd027f9c9e3a5cc6d86
symbols_license="https://raw.githubusercontent.com/google/material-design-icons/$symbols_commit/LICENSE"

fetch() {
    curl -fsSL --retry 3 -o "$1" "$2"
    if [ -n "${3:-}" ]; then
        echo "$3  $1" | sha256sum -c --quiet
    fi
}

mkdir -p "$dir/bin" "$dir/share/mochi/fonts" "$dir/share/applications" \
    "$dir/lib/systemd/user" "$dir/share/licenses/mochi" \
    "$dir/share/bash-completion/completions" "$dir/share/fish/vendor_completions.d" \
    "$dir/share/zsh/site-functions"

install -m755 "$bin/mochi" "$bin/mochid" "$dir/bin/"
install -m644 "$root/systemd/mochid.service" "$dir/lib/systemd/user/"
install -m644 "$root/share/applications/mochi-links.desktop" "$dir/share/applications/"
install -m755 "$root/packaging/release/install.sh" "$dir/"
install -m644 "$root/LICENSE" "$root/README.md" "$root/CHANGELOG.md" "$dir/"
echo "$version" >"$dir/VERSION"

fetch "$stage/inter.zip" "$inter_url" "$inter_sha"
unzip -q -o -j "$stage/inter.zip" InterVariable.ttf LICENSE.txt -d "$stage"
install -m644 "$stage/InterVariable.ttf" "$dir/share/mochi/fonts/"
install -m644 "$stage/LICENSE.txt" "$dir/share/licenses/mochi/Inter-OFL.txt"
fetch "$dir/share/mochi/fonts/MaterialSymbolsRounded.ttf" "$symbols_url" "$symbols_sha"
fetch "$dir/share/licenses/mochi/MaterialSymbols-Apache-2.0.txt" "$symbols_license"

"$bin/mochi" completions bash >"$dir/share/bash-completion/completions/mochi"
"$bin/mochi" completions fish >"$dir/share/fish/vendor_completions.d/mochi.fish"
"$bin/mochi" completions zsh >"$dir/share/zsh/site-functions/_mochi"

tar -C "$stage" -czf "$out/$name.tar.gz" "mochi-$version"
(cd "$out" && sha256sum "$name.tar.gz" >"$name.tar.gz.sha256")
echo "$out/$name.tar.gz"
