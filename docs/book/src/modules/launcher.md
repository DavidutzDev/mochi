# Launcher

Grows the island into a search box. Your most used apps come first; typing searches names, descriptions and keywords, and app actions like Firefox's "New Private Window" when you search for them. Enter or a click picks a result, Escape or a click elsewhere closes the launcher.

Apps and commands start through `uwsm app` in a uwsm session, otherwise through `systemd-run`, so they never belong to `mochid`.

## Providers

Results come from providers. Three are built in:

| Provider | Prefix | Answers |
|---|---|---|
| `apps` | none | Your apps and their actions |
| `calculator` | `=` | Math, like `=2^10` or `=sqrt(2)*pi`. Plain math like `2+2` works without the prefix, above the apps. Enter copies the result |
| `commands` | `>` | A shell command, like `> htop`, with the ones you ran before. Enter runs it, Shift+Enter runs it in the terminal from `terminal` |

A query that starts with a provider's prefix asks only that provider, with the rest of the query. Providers without a prefix answer every query, and their results show together under headings, in the providers' `order`. The built-ins answer at once; scripts and plugins a moment after you stop typing, and their results come in as they're ready.

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
| `copy` | Enter copies this text, through the clipboard module, or `wl-copy` without it |
| `type` | Enter pastes this text into the window you were in, through the clipboard module |
| `open` | Enter opens this URL or file with `xdg-open` |
| `run` | Enter runs this shell command, Shift+Enter in a terminal |
| `id` | Passed to the provider's `pick` command after Enter |

A result has at most one of `copy`, `type`, `open` and `run`. A newer query stops a script that hasn't finished, and so does `timeout_ms`, 2 seconds by default. A script without a prefix isn't asked for an empty query.

Two example scripts are in the repository's `examples/launcher`, and the test suite runs both. Web search, with `!w`:

```sh
{{#include ../../../../examples/launcher/web-search.sh}}
```

Files and folders in your home, with `/`, through `fd` when installed:

```sh
{{#include ../../../../examples/launcher/files.sh}}
```

Plugins can offer providers too, with results from their backend: see [Launcher providers](../launcher-providers.md). The emoji picker in `examples/plugins/emoji` is one, with `:`.

```toml
{{#include ../../../../modules/launcher/settings.toml}}
```

| Action | What it does |
|---|---|
| `toggle`, `open`, `close` | Shows or hides the launcher |
| `launch <id>` | Starts an app by desktop id, like `firefox.desktop` |

The launcher sends `search` and `activate` itself.
