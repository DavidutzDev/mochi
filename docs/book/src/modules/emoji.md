# Emoji

An emoji picker. `mochi ipc emoji toggle`, bound to a key, grows the island into a grid of emoji: a search box at the top, a tab for the emoji you picked last and one per Unicode group, and the group's emoji below. Typing searches every emoji by name, like "cat face" or "thumbs"; every word has to start a word of the name or of its group.

The arrows move the selection, even while you type, and Tab or Shift+Tab changes the tab. Enter pastes the selected emoji into the window you were in, and Shift+Enter only copies it. A click pastes, and a right click or Shift+click copies. The emoji under the pointer has its name at the bottom. Escape closes.

Pasting and copying go through the [clipboard](clipboard.md) module, which types Ctrl+V for you. Without it, the picker can still copy when `wl-copy` is installed, but it can't paste.

The emoji you pick are kept in `$XDG_STATE_HOME/mochi/emoji.json`, most recent first. They fill the Recent tab, and they rank a little higher in searches.

In Hyprland:

```lua
hl.bind("SUPER + period", hl.dsp.exec_cmd("mochi ipc emoji toggle"))
```

## In the launcher

The module adds a provider to the [launcher](launcher.md): type `:` and a few words, like `:fire`. Enter pastes the emoji, and Shift+Enter copies it. With nothing after the `:`, the emoji you picked last come first. Change the prefix in a `[module.launcher.providers.emoji]` section.

```toml
{{#include ../../../../modules/emoji/settings.toml}}
```

## Actions

| Action | What it does |
|---|---|
| `toggle`, `open`, `close` | Shows or hides the picker |
| `paste <emoji>` | Pastes an emoji into the window you were in, and remembers it |
| `copy <emoji>` | Copies an emoji, and remembers it |
| `search [words]` | Answers with the matching emoji as launcher results, one JSON line each |
| `pick <emoji>` | Remembers an emoji as picked; the launcher sends this |

The table comes from Unicode's `emoji-test.txt`, without the skin tone and hair style variants. `modules/emoji/data/generate.sh` writes it again for a newer Unicode.
