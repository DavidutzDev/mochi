#!/bin/sh
# Records a published release where the packages read it:
#
#   packaging/release/record.sh <version> [dist-dir]
#
# - packaging/nix/release.json: each system's archive and its hash, for the
#   flake's `mochi-bin`.
# - packaging/arch/mochi/PKGBUILD: the version and the source tarball's sum.
# - packaging/arch/mochi-bin/PKGBUILD: the version and the archives' sums.
#
# The archives' sums come from the .sha256 files in [dist-dir] when given,
# or from downloading them. The release workflow runs this and commits the
# result; it works by hand too, once the release is on GitHub.
set -eu

version=$1
dist=${2:-}
repo=https://github.com/DavidutzDev/mochi
root=$(cd "$(dirname "$0")/../.." && pwd)

# An archive's sum: from its .sha256 file, or by downloading it.
sum() {
    name="mochi-$1-linux.tar.gz"
    if [ -n "$dist" ] && [ -f "$dist/$name.sha256" ]; then
        cut -d' ' -f1 "$dist/$name.sha256"
    else
        curl -fsSL --retry 3 "$repo/releases/download/v$version/$name" | sha256sum | cut -d' ' -f1
    fi
}

x86=$(sum x86_64)
arm=$(sum aarch64)
source=$(curl -fsSL --retry 3 "$repo/archive/refs/tags/v$version.tar.gz" | sha256sum | cut -d' ' -f1)

cat >"$root/packaging/nix/release.json" <<JSON
{
  "version": "$version",
  "assets": {
    "x86_64-linux": {
      "url": "$repo/releases/download/v$version/mochi-x86_64-linux.tar.gz",
      "sha256": "$x86"
    },
    "aarch64-linux": {
      "url": "$repo/releases/download/v$version/mochi-aarch64-linux.tar.gz",
      "sha256": "$arm"
    }
  }
}
JSON

arch="$root/packaging/arch"
sed -i -e "s/^pkgver=.*/pkgver=$version/" -e "s/^pkgrel=.*/pkgrel=1/" \
    "$arch/mochi/PKGBUILD" "$arch/mochi-bin/PKGBUILD"
# The source tarball's sum is the first in the list; the font's stays.
sed -i "s/^sha256sums=('[0-9a-f]*'/sha256sums=('$source'/" "$arch/mochi/PKGBUILD"
sed -i -e "s/^sha256sums_x86_64=.*/sha256sums_x86_64=('$x86')/" \
    -e "s/^sha256sums_aarch64=.*/sha256sums_aarch64=('$arm')/" "$arch/mochi-bin/PKGBUILD"

echo "Recorded $version: x86_64 $x86, aarch64 $arm, source $source"
