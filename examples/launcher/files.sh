#!/bin/sh
# A launcher provider: finds files and folders in your home by name, and
# opens the one you pick. In config.toml:
#
#   [module.launcher.providers.files]
#   prefix = "/"
#   title = "Files"
#   command = ["sh", "/path/to/files.sh"]
#
# It uses fd when installed, find otherwise, and skips hidden files. Set
# FILES_ROOT to search somewhere else than your home.

query=$1
[ -n "$query" ] || exit 0
root=${FILES_ROOT:-$HOME}

json() {
    printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

if command -v fd >/dev/null 2>&1; then
    fd --max-results 30 --fixed-strings --ignore-case -- "$query" "$root"
else
    find "$root" -name '.*' -prune -o -iname "*$query*" -print 2>/dev/null | head -n 30
fi | while IFS= read -r path; do
    path=${path%/}
    [ "$path" = "$root" ] && continue
    if [ -d "$path" ]; then icon=folder; else icon=text-x-generic; fi
    printf '{"title": "%s", "subtitle": "%s", "icon": "%s", "open": "%s"}\n' \
        "$(json "${path##*/}")" "$(json "${path%/*}")" "$icon" "$(json "$path")"
done
