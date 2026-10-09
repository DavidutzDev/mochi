# Clipboard

Keeps what you copy, text and images, and pastes it back. `mochi ipc clipboard toggle`, bound to a key like SUPER+V, grows the island into a search over the history, pinned entries first, then the rest newest first. Typing searches the start of each text; images match "image". A pane on the right shows the selected entry in full: the whole text, which Page Up and Page Down scroll, or the image scaled to fit. Enter puts the entry back on the clipboard and pastes it into the window you were in, by typing Ctrl+V for you, or Ctrl+Shift+V in a terminal. Shift+Enter only copies it. Ctrl+P, or the pin button on the selected row, pins or unpins it. Shift+Delete, or the trash button, removes it. Escape closes.

Mochi reads the clipboard through the `ext-data-control-v1` Wayland protocol, which Hyprland and Sway support, and pastes through `zwp-virtual-keyboard-v1`. It replaces `cliphist` and `wl-paste --watch`; nothing else needs to run.

## Where the history lives

By default it stays in memory: a file in `$XDG_RUNTIME_DIR/mochi`, which is a tmpfs only you can read. It survives a restart of `mochid` and goes when you log out.

With `storage = "disk"`, it's kept in `$XDG_STATE_HOME/mochi/clipboard/history`, encrypted with XChaCha20-Poly1305. The key is made on the first start and kept in the Secret Service, as "Mochi clipboard history key": gnome-keyring, KeePassXC, or anything else that implements it. Without the key the file can't be read. When no Secret Service answers, Mochi keeps the history in memory instead and says so on the control center card. It never writes it to disk unencrypted.

Either way:

- Copying the same thing again moves its entry to the top instead of keeping it twice.
- Removing an entry, or clearing the history, rewrites the file at once, so nothing of it stays behind.
- The file is readable by you only, and text is compressed with zstd.

## Pins

A pinned entry stays at the top of the picker and of the control center page, under "Pinned". Clearing the history leaves it, and it doesn't count toward `max_entries` or `max_age_hours`. Pins are kept in `$XDG_STATE_HOME/mochi/clipboard/pins`, encrypted with the same key as the history on disk, even with `storage = "memory"`, so they last across logouts. Mochi asks the Secret Service for the key when you pin the first entry, or at startup once pins exist. When no Secret Service answers, pins stay in `$XDG_RUNTIME_DIR` until you log out, and the control center card says so. Copying a pinned entry again doesn't add it to the history.

## What isn't kept

- Copies that password managers mark as secret, with the `x-kde-passwordManagerHint` type, are never read. KeePassXC marks passwords this way. `skip_secrets = false` keeps them.
- Copies made while a window listed in `ignore` has the keyboard are never read. The list holds app ids, matched in any case; on Hyprland the app id is the window's class, which `hyprctl activewindow` shows. By default it lists KeePassXC, Bitwarden and 1Password. Mochi learns which window has the keyboard through `wlr-foreign-toplevel-management`, which Hyprland, Sway and niri support; on other compositors `ignore` has no effect.
- Nothing is kept while the history is paused. A bubble shows on the island meanwhile; clicking it resumes.

The control center has a card with the number of entries and pins, a button to pause the history and one to clear it, and a Clipboard page with the pins and the history: search them, click an entry to paste it, or pin, copy or remove it from its row. The pin button shows on a pinned row, and on the others when you point at them. Clicking an image, here or in the picker, opens it in the card a screenshot gets, with copy, edit and delete; Enter still pastes it. `mochi ipc control-center open clipboard/history` opens the page.

```toml
{{#include ../../../../modules/clipboard/settings.toml}}
```

## Actions

| Action | What it does |
|---|---|
| `toggle`, `open`, `close` | Shows or hides the history |
| `pick <id>` | Copies an entry and pastes it into the window you were in |
| `copy <id>` | Copies an entry without pasting it |
| `copy-text <text>` | Copies some text, and keeps it in the history; the launcher's calculator and providers copy through it |
| `paste-text <text>` | Copies some text and pastes it into the window you were in |
| `show <id>` | Opens an image entry in the capture module's preview card |
| `pin <id>`, `unpin <id>` | Pins an entry, or puts a pin back in the history |
| `delete <id>` | Removes an entry, pinned or not |
| `clear` | Removes everything but the pins |
| `pause [on\|off\|toggle]` | Stops keeping what you copy, or starts again; `on` without an argument |
| `resume` | Starts keeping what you copy again |

The picker sends `search` and `preview` itself.
