#!/bin/sh
# Installs Mochi from this release archive: the binaries, the fonts, the
# systemd unit, the mochi:// link handler and the shell completions.
#
#   ./install.sh                           into ~/.local, for you
#   sudo ./install.sh --prefix /usr/local  for everyone
#   ./install.sh --uninstall               removes what it installed
#
# Mochi also needs Quickshell 0.3 and PulseAudio's library (libpulse, which
# PipeWire systems have too), from your distribution.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
# The copy kept in <prefix>/share/mochi knows its prefix.
case "$here" in
*/share/mochi) prefix=${here%/share/mochi} ;;
*) prefix="$HOME/.local" ;;
esac
action=install
while [ $# -gt 0 ]; do
    case "$1" in
    --prefix)
        prefix="$2"
        shift 2
        ;;
    --prefix=*)
        prefix="${1#--prefix=}"
        shift
        ;;
    --uninstall)
        action=uninstall
        shift
        ;;
    -h | --help)
        sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'
        exit 0
        ;;
    *)
        echo "install.sh: unknown option $1" >&2
        exit 2
        ;;
    esac
done

version=$(cat "$here/VERSION")
# A user's units live in their config; a system prefix has its own.
if [ "$prefix" = "$HOME/.local" ]; then
    units="$HOME/.config/systemd/user"
else
    units="$prefix/lib/systemd/user"
fi

files="bin/mochi bin/mochid
share/mochi/fonts/InterVariable.ttf share/mochi/fonts/MaterialSymbolsRounded.ttf
share/applications/mochi-links.desktop
share/bash-completion/completions/mochi share/fish/vendor_completions.d/mochi.fish
share/zsh/site-functions/_mochi
share/licenses/mochi/Inter-OFL.txt share/licenses/mochi/MaterialSymbols-Apache-2.0.txt
share/mochi/install.sh share/mochi/VERSION"

if [ "$action" = uninstall ]; then
    for file in $files; do
        rm -f "$prefix/$file"
    done
    rm -f "$units/mochid.service"
    rmdir "$prefix/share/mochi/fonts" "$prefix/share/mochi" "$prefix/share/licenses/mochi" 2>/dev/null || true
    echo "Removed Mochi from $prefix."
    exit 0
fi

# This script and the version go along, so the universal installer can
# update what's here, and `share/mochi/install.sh --uninstall` removes it.
mkdir -p "$here/share/mochi"
[ -f "$here/share/mochi/install.sh" ] || cp "$0" "$here/share/mochi/install.sh"
[ -f "$here/share/mochi/VERSION" ] || cp "$here/VERSION" "$here/share/mochi/VERSION"

for file in $files; do
    mode=644
    case "$file" in bin/*) mode=755 ;; esac
    case "$file" in share/mochi/install.sh) mode=755 ;; esac
    mkdir -p "$(dirname "$prefix/$file")"
    install -m"$mode" "$here/$file" "$prefix/$file"
done
# The unit and the link handler name the binaries by their full path.
mkdir -p "$units"
sed "s|/usr/bin/|$prefix/bin/|g" "$here/lib/systemd/user/mochid.service" >"$units/mochid.service"
sed "s|/usr/bin/|$prefix/bin/|g" "$here/share/applications/mochi-links.desktop" \
    >"$prefix/share/applications/mochi-links.desktop"
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q "$prefix/share/applications" || true
fi

echo "Installed Mochi $version in $prefix."
if ! command -v quickshell >/dev/null 2>&1; then
    echo "Mochi draws its shell with Quickshell 0.3: install it from your distribution."
fi
case ":$PATH:" in
*":$prefix/bin:"*) ;;
*) echo "Add $prefix/bin to your PATH to run mochi." ;;
esac
echo "Start it with the session:  systemctl --user daemon-reload && systemctl --user enable --now mochid"
echo "or run mochid from your compositor's autostart. \`mochi doctor\` checks the rest."
