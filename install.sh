#!/bin/sh
# Installs Mochi, or updates it, from its latest release:
#
#   curl -fsSL https://raw.githubusercontent.com/DavidutzDev/mochi/main/install.sh | sh
#
# Run it again to update. Options go after `sh -s --`:
#
#   --prefix DIR    install there instead of ~/.local (with sudo for /usr/local)
#   --version X     a given release, like 0.0.8, instead of the latest
#   --enable        start mochid with the session, through systemd
#   --uninstall     remove what this installed
#   --force         install even when the same version is there, or when a
#                   package manager installed Mochi
#
# MOCHI_FROM_SOURCE=1 builds the release from source instead of downloading
# it, with cargo; MOCHI_FROM_SOURCE=main builds the latest commit. The other
# options can come from MOCHI_PREFIX and MOCHI_VERSION too.
#
# Mochi installed by a package manager, like Nix or pacman, is left to it.
set -eu

repo=${MOCHI_REPO:-https://github.com/DavidutzDev/mochi}
releases=${MOCHI_RELEASES:-$repo/releases}
prefix=${MOCHI_PREFIX:-$HOME/.local}
version=${MOCHI_VERSION:-}
source=${MOCHI_FROM_SOURCE:-}
enable=
uninstall=
force=

say() { printf '%s\n' "$*"; }
fail() {
    printf 'mochi installer: %s\n' "$*" >&2
    exit 1
}
has() { command -v "$1" >/dev/null 2>&1; }

while [ $# -gt 0 ]; do
    case "$1" in
    --prefix) prefix=$2 && shift 2 ;;
    --prefix=*) prefix=${1#--prefix=} && shift ;;
    --version) version=$2 && shift 2 ;;
    --version=*) version=${1#--version=} && shift ;;
    --enable) enable=1 && shift ;;
    --uninstall) uninstall=1 && shift ;;
    --force) force=1 && shift ;;
    -h | --help)
        sed -n '2,22p' "$0" 2>/dev/null | sed 's/^# \{0,1\}//' ||
            say "See https://github.com/DavidutzDev/mochi/blob/main/install.sh"
        exit 0
        ;;
    *) fail "unknown option $1; --help lists them" ;;
    esac
done
version=${version#v}

# What this installer put there before, if anything.
installed=
if [ -f "$prefix/share/mochi/VERSION" ] && [ -x "$prefix/share/mochi/install.sh" ]; then
    installed=$(cat "$prefix/share/mochi/VERSION")
fi

if [ -n "$uninstall" ]; then
    [ -n "$installed" ] || fail "this installer put nothing in $prefix"
    if has systemctl && systemctl --user is-active --quiet mochid 2>/dev/null; then
        systemctl --user disable --now mochid || true
    fi
    sh "$prefix/share/mochi/install.sh" --prefix "$prefix" --uninstall
    if has systemctl; then systemctl --user daemon-reload 2>/dev/null || true; fi
    exit 0
fi

# Mochi from a package manager stays with it: two copies would fight over
# the session.
if [ -z "$installed" ] && [ -z "$force" ] && has mochi; then
    found=$(command -v mochi)
    real=$(readlink -f "$found" 2>/dev/null || printf '%s' "$found")
    case "$real" in
    /nix/store/*) fail "Nix installed Mochi ($found): update it with your flake or profile, or pass --force" ;;
    esac
    if has pacman && owner=$(pacman -Qqo "$real" 2>/dev/null); then
        fail "pacman's $owner package installed Mochi: update it with pacman, or pass --force"
    fi
    if has dpkg && owner=$(dpkg -S "$real" 2>/dev/null); then
        fail "the ${owner%%:*} package installed Mochi: update it with apt, or pass --force"
    fi
    if has rpm && owner=$(rpm -qf "$real" 2>/dev/null); then
        fail "the $owner package installed Mochi: update it with your package manager, or pass --force"
    fi
    fail "Mochi is already at $found, from somewhere this installer didn't put it; pass --force to install anyway"
fi

for tool in curl tar uname; do
    has "$tool" || fail "it needs $tool"
done
if has sha256sum; then
    check_sum() { sha256sum -c --quiet "$1"; }
elif has shasum; then
    check_sum() { shasum -a 256 -c --quiet "$1"; }
else
    fail "it needs sha256sum or shasum, to check what it downloads"
fi

arch=$(uname -m)
case "$arch" in
x86_64 | aarch64) ;;
arm64) arch=aarch64 ;;
*) [ -n "$source" ] || fail "no release is built for $arch: try MOCHI_FROM_SOURCE=1" ;;
esac

# The release asked for, or the latest: GitHub redirects
# releases/latest to the newest tag.
if [ -z "$version" ] && [ "$source" != main ]; then
    latest=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "$releases/latest") ||
        fail "can't reach $releases"
    version=${latest##*/v}
    case "$version" in
    [0-9]*) ;;
    *) fail "$releases has no release yet" ;;
    esac
fi

if [ -n "$installed" ] && [ "$installed" = "$version" ] && [ -z "$force" ]; then
    say "Mochi $version is installed in $prefix, the latest. --force installs it again."
    exit 0
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT INT TERM

if [ -n "$source" ]; then
    # Built here, then packed like a release, so it installs the same way.
    for tool in git cargo pkg-config unzip; do
        has "$tool" || fail "building from source needs $tool"
    done
    pkg-config --exists libpulse ||
        fail "building from source needs libpulse's headers: libpulse-dev, pulseaudio-libs-devel or libpulse"
    ref=$([ "$source" = main ] && echo main || echo "v$version")
    say "Building Mochi $ref from source…"
    git clone --quiet --depth 1 --branch "$ref" "$repo" "$work/src" ||
        fail "can't clone $repo at $ref"
    (cd "$work/src" && cargo build --release --locked --no-default-features -p mochid -p mochi) ||
        fail "the build failed"
    archive=$(sh "$work/src/packaging/release/archive.sh" "$work/src/target/release" "$work/dist" | tail -1)
    version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$work/src/Cargo.toml" | head -1)
else
    # Prebuilt for glibc 2.35 and newer.
    if has ldd; then
        glibc=$(ldd --version 2>&1 | head -1 | sed -n 's/.* \([0-9][0-9]*\.[0-9][0-9]*\)$/\1/p')
        case "$glibc" in
        "") fail "the releases are built against glibc; on this system, try MOCHI_FROM_SOURCE=1" ;;
        2.[0-9] | 2.[0-2][0-9] | 2.3[0-4]) fail "the releases need glibc 2.35 or newer, this has $glibc: try MOCHI_FROM_SOURCE=1" ;;
        esac
    fi
    name="mochi-$arch-linux.tar.gz"
    say "Downloading Mochi $version for $arch…"
    curl -fsSL --retry 3 -o "$work/$name" "$releases/download/v$version/$name" ||
        fail "can't download $releases/download/v$version/$name"
    curl -fsSL --retry 3 -o "$work/$name.sha256" "$releases/download/v$version/$name.sha256" ||
        fail "can't download the archive's sum"
    (cd "$work" && check_sum "$name.sha256") || fail "the archive doesn't match its sum"
    archive="$work/$name"
fi

tar -xzf "$archive" -C "$work"
sh "$work"/mochi-*/install.sh --prefix "$prefix"
# What's installed, for the next run to update, and its uninstaller, also
# when an older archive's install.sh didn't keep them.
mkdir -p "$prefix/share/mochi"
[ -x "$prefix/share/mochi/install.sh" ] || install -m755 "$work"/mochi-*/install.sh "$prefix/share/mochi/install.sh"
printf '%s\n' "$version" >"$prefix/share/mochi/VERSION"

if has systemctl && systemctl --user daemon-reload 2>/dev/null; then
    if [ -n "$enable" ]; then
        systemctl --user enable --now mochid
    elif systemctl --user is-active --quiet mochid; then
        # An update: the running shell takes the new binaries, when it's
        # the one from this prefix. Another one, like a package's, stays.
        pid=$(systemctl --user show -p MainPID --value mochid 2>/dev/null || true)
        running=$(readlink -f "/proc/$pid/exe" 2>/dev/null || true)
        here=$(cd "$prefix/bin" 2>/dev/null && pwd -P)
        if [ -n "$here" ] && [ "${running%/*}" = "$here" ]; then
            systemctl --user restart mochid
            say "Restarted mochid."
        else
            say "The running mochid comes from ${running:-somewhere else}: restart it from this prefix to use $version."
        fi
    fi
fi
if [ -n "$installed" ]; then
    say "Updated Mochi from $installed to $version."
fi
