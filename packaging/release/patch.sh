#!/bin/sh
# Prepares a patch release when CHANGELOG.md has entries under
# "## Unreleased": the patch after the latest version tag, like 0.1.2 after
# v0.1.1, written everywhere by bump.sh, and "Unreleased" dated as that
# version. Prints the version, or nothing when there's nothing to release.
# It commits nothing: .github/workflows/patch.yml commits, tags and
# releases.
#
# Minor and major releases are made by hand (RELEASING.md); patches only
# follow them. A patch brings no tour steps: the tour shows its changelog.
set -eu
cd "$(dirname "$0")/../.."

# Nothing written down, nothing to release.
entries=$(awk '/^## Unreleased/ { inside = 1; next } /^## / { inside = 0 } inside && /^- /' CHANGELOG.md | wc -l)
if [ "$entries" -eq 0 ]; then
    exit 0
fi

last=$(git describe --tags --abbrev=0 --match 'v[0-9]*')
current=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
# A release being made by hand: Cargo.toml already says a newer version.
if [ "v$current" != "$last" ]; then
    echo "Cargo.toml says $current but the latest tag is $last: leaving it to the release by hand" >&2
    exit 0
fi

# Tour steps come with minor releases.
if git diff "$last" HEAD -- 'modules/*/src/tour.rs' 'examples/*/src/tour.rs' | grep -q '^+.*"since"'; then
    echo "error: tour steps were added since $last; they come with a minor release, not a patch" >&2
    exit 1
fi

major=${current%%.*}
rest=${current#*.}
minor=${rest%%.*}
patch=${rest#*.}
next="$major.$minor.$((patch + 1))"

sh packaging/release/bump.sh "$next"
sed -i "s/^## Unreleased\$/## $next - $(date -u +%Y-%m-%d)/" CHANGELOG.md
echo "$next"
