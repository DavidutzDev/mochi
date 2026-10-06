# Launcher

Grows the island into a search box. Your most used apps come first; typing searches names, descriptions and keywords, and app actions like Firefox's "New Private Window" when you search for them. Enter or a click picks a result, Escape or a click elsewhere closes the launcher.

Apps and commands start through `uwsm app` in a uwsm session, otherwise through `systemd-run`, so they never belong to `mochid`.

## Providers

Results come from providers. These are built in:

| Provider | Prefix | Answers |
|---|---|---|
| `apps` | none | Your apps and their actions |
| `calculator` | `=` | Math, like `=2^10` or `=sqrt(2)*pi`. Plain math like `2+2` works without the prefix, above the apps. Enter copies the result |
| `commands` | `>` | A shell command, like `> htop`, with the ones you ran before. Enter runs it, Shift+Enter runs it in the terminal from `terminal` |
| `files` | `/` | Files and folders in your home by name, from an index in memory. Enter opens one, Shift+Enter the folder it's in |
| web searches | `!w`, `!g`, `!gh`, `!yt`, `!nix`, `!wiki` | `!w rust` opens a DuckDuckGo search for "rust"; the others search Google, GitHub, YouTube, NixOS packages and Wikipedia. `engines` adds more |
| [emoji](emoji.md) | `:` | Emoji by name. Enter types it into the window you were in, Shift+Enter copies it |
| [colors](colors.md) | `#` | A color you type, in every format, or a pick from the screen |

The emoji and colors providers come from their modules, so they're there when those modules run.

A query that starts with a provider's prefix asks only that provider, with the rest of the query. Providers without a prefix answer every query, and their results show together under headings, in the providers' `order`. The built-ins and modules answer at once; scripts a moment after you stop typing, and their results come in as they're ready.

`mochi ipc launcher open :` opens the launcher with `:` typed, so a key can go straight to a provider.

The file index covers your home folder without hidden files and folders and build folders like `node_modules` and `target`, up to 200 000 entries. It's built when the launcher starts, and again in the background when you open the launcher and the index is more than 15 seconds old; the results update when it's ready.

The calculator knows `+ - * / % ^`, parentheses, `!`, `pi`, `e`, `tau`, and the functions `sqrt cbrt abs floor ceil round sin cos tan asin acos atan sinh cosh tanh ln log log2 exp min max pow`, with trig in radians. A number before a name or a parenthesis multiplies, so `2pi` works.

### Script providers

A section with a `command` adds a provider of your own. The launcher runs the command with the query as its last argument, and in `MOCHI_QUERY`, and reads one result per line of what it prints:

```json
{"title": "Search the web for rust", "subtitle": "https://duckduckgo.com/?q=rust", "icon": "web-browser", "open": "https://duckduckgo.com/?q=rust"}
```

| Key | |
|---|---|
| `title` | Required |
| `subtitle` | A second line |
| `icon` | An icon theme name, or a file path |
| `glyph` | A short text shown big in place of the icon, like an emoji |
| `color` | A CSS color, shown as a round swatch in place of the icon |
| `copy` | Enter copies this text, through the clipboard module, or `wl-copy` without it |
| `type` | Enter pastes this text into the window you were in, through the clipboard module |
| `open` | Enter opens this URL or file with `xdg-open` |
| `run` | Enter runs this shell command, Shift+Enter in a terminal |
| `alt` | What Shift+Enter does, as an object with one of the four: `{"copy": "😀"}`. A `run` result runs in a terminal on Shift+Enter without one |
| `id` | Passed to the provider's `pick` command after Enter |

A result has at most one of `copy`, `type`, `open` and `run`. A newer query stops a script that hasn't finished, and so does `timeout_ms`, 2 seconds by default. A script without a prefix isn't asked for an empty query.

Two example scripts are in the repository's `examples/launcher`, and the test suite runs both. The launcher has both built in now, faster, so they're here to copy from. Web search:

```sh
{{#include ../../../../examples/launcher/web-search.sh}}
```

Files and folders in your home, through `fd` when installed:

```sh
{{#include ../../../../examples/launcher/files.sh}}
```

Plugins can offer providers too, with results from their backend: see [Launcher providers](../launcher-providers.md). The example emoji picker in `examples/plugins/emoji` is one, with `;`.

```toml
{{#include ../../../../modules/launcher/settings.toml}}
```

| Action | What it does |
|---|---|
| `toggle`, `open`, `close` | Shows or hides the launcher |
| `launch <id>` | Starts an app by desktop id, like `firefox.desktop` |

The launcher sends `search` and `activate` itself.
