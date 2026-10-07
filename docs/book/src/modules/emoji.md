# Emoji

An emoji picker. `mochi ipc emoji toggle`, bound to a key, grows the island into a grid of emoji: a search box at the top, a tab for the emoji you picked last and one per Unicode group, and the group's emoji below. Typing searches every emoji by name, like "cat face" or "thumbs"; every word has to start a word of the name or of its group.

The arrows move the selection, even while you type, and Tab or Shift+Tab changes the tab. Enter pastes the selected emoji into the window you were in, and Shift+Enter only copies it. A click pastes, and a right click or Shift+click copies. The emoji under the pointer has its name at the bottom. Escape closes.

Pasting and copying go through the [clipboard](clipboard.md) module, which types Ctrl+V for you. Without it, the picker can still copy when `wl-copy` is installed, but it can't paste.

The emoji you pick are kept in `$XDG_STATE_HOME/mochi/emoji.json`, most recent first. They fill the Recent tab, and they rank a little higher in searches.

In Hyprland:

```lua
hl.bind("SUPER + period", hl.dsp.exec_cmd("mochi ipc emoji toggle"))
```

## Skin tones

The six swatches next to the search box set the skin tone of every emoji of a person or a hand: none (the yellow ones), then the five Fitzpatrick tones from light to dark. The tone applies in the grid, in the Recent tab and in the launcher.

To give one emoji a different tone, hold it down for a moment. Its six forms open over it; a click pastes one, a right click copies it, and that emoji keeps that tone from then on, whatever the default. Escape or a click elsewhere closes them without picking. Emoji with tones have a small dot in the corner when the pointer is on them.

Emoji of two people, like "people holding hands" or "kiss: woman, man", take the same tone for both. Unicode also has them with a different tone for each person; the picker leaves those out. Families have no tones in Unicode.

The tones are kept in `emoji.json` with the recent emoji.

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
| `tone <tone> [emoji]` | Sets the default skin tone: `none`, `light`, `medium-light`, `medium`, `medium-dark` or `dark`. With an emoji, sets that emoji's own tone |

The table comes from Unicode's `emoji-test.txt`, without the hair style variants. Each emoji that has skin tones lists its five toned forms. `modules/emoji/data/generate.sh` writes it again for a newer Unicode.
