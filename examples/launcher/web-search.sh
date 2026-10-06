#!/bin/sh
# A launcher provider: searches the web for what you type. In config.toml:
#
#   [module.launcher.providers.web]
#   prefix = "!w"
#   title = "Web"
#   command = ["sh", "/path/to/web-search.sh"]
#
# The launcher runs it with the query as its last argument and reads one
# JSON result per line. Set SEARCH_URL to use another engine; the query
# goes at its end.

query=$1
[ -n "$query" ] || exit 0
engine=${SEARCH_URL:-https://duckduckgo.com/?q=}

# Every byte but letters, digits and -._~ becomes %XX.
encode() {
    printf '%s' "$1" | od -An -v -tx1 | tr -s ' \n' '  ' | {
        read -r bytes
        for byte in $bytes; do
            case $byte in
            3[0-9] | 4[1-9a-f] | 5[0-9a] | 6[1-9a-f] | 7[0-9a] | 2d | 2e | 5f | 7e)
                printf "\\$(printf '%03o' "0x$byte")" ;;
            *) printf '%%%s' "$(printf '%s' "$byte" | tr 'a-f' 'A-F')" ;;
            esac
        done
    }
}

# A JSON string's inside: backslashes and quotes escaped.
json() {
    printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

url=$engine$(encode "$query")
printf '{"title": "Search the web for %s", "subtitle": "%s", "icon": "web-browser", "open": "%s"}\n' \
    "$(json "$query")" "$(json "$url")" "$(json "$url")"
