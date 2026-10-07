#!/bin/sh
# Writes emoji.tsv from Unicode's emoji-test.txt. To rerun it, from this
# directory:
#
#     sh generate.sh > emoji.tsv
#
# It takes the file from nixpkgs#unicode-emoji, or from the path given as
# its first argument. It keeps fully-qualified emoji and skips the hair
# style variants. Each line is: emoji, name, group, subgroup, tones,
# separated by tabs, in Unicode's order. Tones is empty, or the emoji in
# the five skin tones from light to dark, separated by spaces; the skin
# tone variants have no line of their own.
#
# An emoji of two people, like "people holding hands", also has variants
# with a different tone for each person. Only the ones where both have the
# same tone are kept, so it takes one tone like the others.
set -eu

test_file=${1:-}
if [ -z "$test_file" ]; then
    out=$(nix build --no-link --print-out-paths nixpkgs#unicode-emoji)
    test_file=$(find -L "$out" -name emoji-test.txt | head -n 1)
fi

awk '
# The code points without skin tones (1F3FB to 1F3FF) and variation
# selectors (FE0F), so a variant and its emoji have the same key: "☝️"
# is 261D FE0F, but "☝🏽" is 261D 1F3FD.
function key(points,    count, list, i, out) {
    count = split(points, list, " ")
    out = ""
    for (i = 1; i <= count; i++)
        if (list[i] !~ /^(1F3F[B-F]|FE0F)$/)
            out = out " " list[i]
    return out
}
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
    sub(/ *;.*/, "", points)
    # What follows the first "#": the emoji, its version, its name.
    rest = $0
    sub(/^[^#]*# /, "", rest)
    emoji = rest
    sub(/ .*/, "", emoji)
    name = rest
    sub(/^[^ ]* E[0-9.]+ /, "", name)
    # Hair styles are 1F9B0 to 1F9B3 after a zero width joiner.
    if (points ~ /200D 1F9B[0-3]/) next
    if (points ~ /1F3F[B-F]/) {
        # A skin tone variant: kept when everyone in it has the same tone.
        tone = ""
        count = split(points, list, " ")
        for (i = 1; i <= count; i++) {
            if (list[i] !~ /^1F3F[B-F]$/) continue
            if (tone != "" && tone != list[i]) next
            tone = list[i]
        }
        k = key(points)
        if (!((k, tone) in variant)) variant[k, tone] = emoji
        next
    }
    lines++
    line[lines] = emoji "\t" name "\t" group "\t" subgroup
    keys[lines] = key(points)
}
END {
    split("1F3FB 1F3FC 1F3FD 1F3FE 1F3FF", tones, " ")
    for (n = 1; n <= lines; n++) {
        toned = ""
        for (t = 1; t <= 5; t++) {
            if (!((keys[n], tones[t]) in variant)) {
                toned = ""
                break
            }
            toned = toned (t > 1 ? " " : "") variant[keys[n], tones[t]]
        }
        printf "%s\t%s\n", line[n], toned
    }
}
' "$test_file"
