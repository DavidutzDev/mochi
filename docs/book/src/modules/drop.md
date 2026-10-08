# Drop

Drag files onto the island. While they hover, its outline turns to the accent and it says to drop them. Once dropped, it names what came, like "3 PDFs" or "report.pdf", and offers what fits:

| Action | For | With |
|---|---|---|
| Compress | anything | `zip`, or `bsdtar` |
| Extract | zip, tar, tar.gz and other archives, 7z, rar | `bsdtar`, or `unzip`, `tar` and `7z` |
| Merge PDFs | two PDFs or more, in the order dropped | `pdfunite` (poppler), or `qpdf` |
| To PNG, To JPEG, To WebP | images not already in that format | `magick` or `convert` (ImageMagick) |
| Copy paths | anything | `wl-copy` |
| Open | anything, up to 10 files | `xdg-open` |

An action shows only when a program for it is installed; `mochi doctor` lists the ones missing. New files go next to the first dropped one, and never over an old one: a second `Archive.zip` becomes `Archive 2.zip`, and an archive extracts into a new folder named after it. After an action, the island says what it made, with Show to open its folder, and closes a few seconds later. Escape closes it sooner.

```toml
{{#include ../../../../modules/drop/settings.toml}}
```

`mochi ipc drop files` takes paths or `file://` URIs, one a line, to do the same from a script:

```sh
mochi ipc drop files "$(printf '%s\n' "$PWD"/*.pdf)"
mochi ipc drop run merge
```
