#!/bin/sh
# Writes emoji.tsv from Unicode's emoji-test.txt. To rerun it, from this
# directory:
#
#     sh generate.sh > emoji.tsv
#
# It takes the file from nixpkgs#unicode-emoji, or from the path given as
# its first argument. It keeps fully-qualified emoji and skips the skin tone
# and hair style variants. Each line is: emoji, name, group, subgroup,
# separated by tabs, in Unicode's order.
set -eu

test_file=${1:-}
if [ -z "$test_file" ]; then
    out=$(nix build --no-link --print-out-paths nixpkgs#unicode-emoji)
    test_file=$(find -L "$out" -name emoji-test.txt | head -n 1)
fi

awk '
/^# Version:/ {
    printf "# Unicode emoji %s, from emoji-test.txt by generate.sh\n", $3
}
/^# group:/ {
    group = tolower(substr($0, 10))
}
/^# subgroup:/ {
    subgroup = substr($0, 13)
}
/; fully-qualified / {
    points = $0
    sub(/;.*/, "", points)
    # Skin tones are 1F3FB to 1F3FF; hair styles are 1F9B0 to 1F9B3 after
    # a zero width joiner.
    if (points ~ /1F3F[B-F]/ || points ~ /200D 1F9B[0-3]/) next
    # What follows the first "#": the emoji, its version, its name.
    rest = $0
    sub(/^[^#]*# /, "", rest)
    emoji = rest
    sub(/ .*/, "", emoji)
    name = rest
    sub(/^[^ ]* E[0-9.]+ /, "", name)
    if (name ~ /skin tone/) next
    printf "%s\t%s\t%s\t%s\n", emoji, name, group, subgroup
}
' "$test_file"
