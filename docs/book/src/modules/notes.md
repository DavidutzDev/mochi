# Notes

To-do lists and notes, as [desktop widgets](widgets.md). Each one you place has its own content, kept in `$XDG_STATE_HOME/mochi/notes.json` under the widget's id, and removing the widget drops it.

The widgets are typed into on the desktop: click the to-do list's bottom field or the note, and type. The to-do list adds an item on Enter; a click on an item ticks it, its cross removes it, and "Clear done" removes the ticked ones. The note saves a moment after you stop typing.

```toml
{{#include ../../../../modules/notes/settings.toml}}
```

| Action | What it does |
|---|---|
| `add <widget> <text>` | Adds an item to a to-do list |
| `toggle <widget> <index>`, `delete <widget> <index>` | Ticks or removes an item, counting from 0 |
| `clear <widget>` | Removes the ticked items |
| `write <widget> [text]` | Replaces a note's text |
| `forget <widget>` | Drops what a widget kept; the widgets module sends it when one is removed |
