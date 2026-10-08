# Drop

Drag files onto the island. While they hover, its outline turns to the accent and it says to drop them. Once dropped, it names what came, like "3 PDFs" or "report.pdf", and offers what fits:

| Action | For | With |
|---|---|---|
| Compress | anything | `zip`, or `bsdtar` |
| Extract | zip, tar, tar.gz and other archives, 7z, rar | `bsdtar`, or `unzip`, `tar` and `7z` |
| Merge PDFs | two PDFs or more, in the order dropped | `pdfunite` (poppler), or `qpdf` |
| Copy paths | anything | `wl-copy` |
| Open | anything, up to 10 files | `xdg-open` |

Under **Convert to**, a button for each format the files convert to:

| From | To | With |
|---|---|---|
| PNG, JPEG, WebP, GIF, BMP, TIFF | PNG, JPEG, WebP, GIF, BMP, TIFF, ICO | nothing: Mochi converts them itself |
| HEIC, AVIF | the same | ImageMagick (`magick`) |
| images | SVG, traced into shapes | `vtracer` |
| images | PDF | ImageMagick |
| videos | MP4, WebM, MKV, GIF, MP3 (the sound) | `ffmpeg` |
| sound | MP3, Ogg, FLAC, WAV | `ffmpeg` |
| Word, Writer, Excel, PowerPoint files | PDF, Word, ODT, HTML | LibreOffice (`soffice`) |
| documents, Markdown, HTML | Markdown, Word, ODT, HTML | `pandoc` |

A JPEG has no transparency, so transparent pixels land on white; an icon is at most 256 pixels a side.

When something dropped needs a program that isn't installed, the panel says which, like "Install ffmpeg to convert videos", and the other files still convert: an image and a video dropped together still offer GIF for the image.

An action shows only when a program for it is installed; `mochi doctor` lists the ones missing. New files go next to the first dropped one, and never over an old one: a second `Archive.zip` becomes `Archive 2.zip`, and an archive extracts into a new folder named after it. After an action, the island says what it made, with Show to open its folder, and closes a few seconds later. Escape closes it sooner.

```toml
{{#include ../../../../modules/drop/settings.toml}}
```

`mochi ipc drop files` takes paths or `file://` URIs, one a line, to do the same from a script:

```sh
mochi ipc drop files "$(printf '%s\n' "$PWD"/*.pdf)"
mochi ipc drop run merge
```
