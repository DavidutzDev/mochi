# Clipboard

Keeps what you copy, text and images, and pastes it back. `mochi ipc clipboard toggle`, bound to a key like SUPER+V, grows the island into a search over the history, newest first. Typing searches the start of each text; images match "image". Enter puts the entry back on the clipboard and pastes it into the window you were in, by typing Ctrl+V for you, or Ctrl+Shift+V in a terminal. Shift+Enter only copies it. Shift+Delete, or the trash button, removes it. Escape closes.

Mochi reads the clipboard through the `ext-data-control-v1` Wayland protocol, which Hyprland and Sway support, and pastes through `zwp-virtual-keyboard-v1`. It replaces `cliphist` and `wl-paste --watch`; nothing else needs to run.

## Where the history lives

By default it stays in memory: a file in `$XDG_RUNTIME_DIR/mochi`, which is a tmpfs only you can read. It survives a restart of `mochid` and goes when you log out.

With `storage = "disk"`, it's kept in `$XDG_STATE_HOME/mochi/clipboard/history`, encrypted with XChaCha20-Poly1305. The key is made on the first start and kept in the Secret Service, as "Mochi clipboard history key": gnome-keyring, KeePassXC, or anything else that implements it. Without the key the file can't be read. When no Secret Service answers, Mochi keeps the history in memory instead and says so on the hub card. It never writes it to disk unencrypted.

Either way:

- Copies that password managers mark as secret, with `x-kde-passwordManagerHint`, are never read. KeePassXC marks passwords this way.
- Copying the same thing again moves its entry to the top instead of keeping it twice.
- Removing an entry, or clearing the history, rewrites the file at once, so nothing of it stays behind.
- The file is readable by you only, and text is compressed with zstd.

The hub has a card with the number of entries, a button to pause the history and one to clear it, and a Clipboard page with the history itself: search it, click an entry to paste it, or copy or remove it from its row. `mochi ipc hub open clipboard/history` opens the page.

```toml
{{#include ../../../../modules/clipboard/settings.toml}}
```

## Actions

| Action | What it does |
|---|---|
| `toggle`, `open`, `close` | Shows or hides the history |
| `pick <id>` | Copies an entry and pastes it into the window you were in |
| `copy <id>` | Copies an entry without pasting it |
| `delete <id>` | Removes an entry |
| `clear` | Removes everything |
| `pause on\|off\|toggle` | Stops keeping what you copy, or starts again |

The picker sends `search` itself.
