# Capture

Screenshots and screen recordings, picked on the island.

`mochi ipc capture screenshot` freezes every screen and opens the picker in its default mode, a region unless `mode` says otherwise, so you can drag right away. The island shows Region, Window, Screen and All screens: click one, or press Tab or a number, to switch. Over the frozen screens you drag a region, click a window, click a screen, or click anywhere for all of them as one image. A region can run from one screen into the next, and the screenshot joins their parts at the sharpest screen's scale, with any gap between screens left transparent. Recordings take one screen at a time. A region can be moved and resized by its corners before you take it with Enter, a double click or the Capture button. The screenshot goes to `~/Pictures/Screenshots` and to the clipboard, and the island shows it for a few seconds with buttons to copy it again, open it in an editor, open its folder or delete it. Escape or a right click cancels. The picker opens over anything on the island, the launcher or the control center included, and the frozen screen still shows it, so you can capture the shell itself; it comes back afterwards.

`mochi ipc capture record` picks the same way over the live screens, then records. A red dot next to the island shows while it records; `mochi ipc capture stop` or `record` again stops it, and the island shows the file in `~/Videos/Recordings`. Clicking the dot opens the recording's controls: the time, **Stop**, and for a recording of a whole screen a button for each screen. Clicking another screen goes on recording there, and `mochi ipc capture switch [screen]` does the same, to the next screen without one. The recorder records one screen at a time, so each screen makes a part in a hidden folder beside the recording, and ffmpeg joins them once it stops: as they are when the screens have the same size, which takes a moment, or encoded again otherwise, each part scaled into the first screen's size with black bars. Without ffmpeg, the parts stay beside the recording as "part 1", "part 2" and so on. While a recording is being picked, the island also has toggles for the desktop audio (A) and the microphone (M). `record_audio` and `record_microphone` set how they start. Two buttons set the quality: the frame rate (F) steps through 15, 30, 60, 90 and 120 fps, and the resolution (Q) through Native, 480p, 720p, 1080p and 1440p. A resolution scales the video down to that many lines, keeping its shape, and never scales it up. Two more set the encoding: the codec steps through Auto and every codec gpu-screen-recorder lists on your machine, like HEVC (Vulkan) or H.264 (CPU), and the file through MP4, MKV, which survives the recorder stopping hard, and WebM, which only shows when a codec that fits it, AV1, VP9 or VP8, is there. `framerate`, `resolution`, `codec` and `container` set how they start.

Give a mode to open in it instead of the default: `screenshot window`, or `screenshot screen`, which takes the focused monitor at once.

```toml
{{#include ../../../../modules/capture/settings.toml}}
```

## Recording

Recordings go through [gpu-screen-recorder](https://git.dec05eba.com/gpu-screen-recorder/about/), which encodes on the GPU. Install it, and on NixOS enable its capture helper, which records a region or a screen:

```nix
programs.gpu-screen-recorder.enable = true;
```

The module asks gpu-screen-recorder which video codecs work on your GPU and picks one: a hardware encoder when there is one, otherwise Vulkan encoding, which works on cards whose driver is too old for NVENC, and the CPU as a last resort. Set `codec` to force one.

A window recording goes through the screen-cast portal, so it records that window alone, even behind others. The portal's picker chooses the window.

## Windows

No standard Wayland protocol says where windows are, so picking a window needs the compositor's IPC. Hyprland's works today; elsewhere, screenshots offer only Region and Screen.

## Actions

| Action | What it does |
|---|---|
| `screenshot [region\|window\|screen\|all]` | Takes a screenshot, picking in the default mode without one; `screen` and `all` take it at once |
| `record [region\|window\|screen]` | Starts recording, or stops the recording |
| `mode <region\|window\|screen>` | Switches what the open picker captures |
| `stop` | Stops recording |
| `switch [screen]` | Goes on recording a whole screen on another one, the next without one |
| `cancel` | Closes the picker |
| `copy`, `edit`, `delete`, `open` `[path]` | Act on the last capture, or on a file from the history: copy it, open it in the editor, delete it, open its folder |
| `preview <path>` | Shows a file from the history in the preview card |

The picker sends `audio`, `microphone`, `framerate`, `resolution`, `frame`, `region`, `select` and `confirm` itself, and the control center page `history` and `start`. `framerate [fps]`, `resolution [preset]`, `codec [name]` and `container [mp4|mkv|webm]` also take a value, like `mochi ipc capture framerate 30` or `mochi ipc capture codec hevc_vulkan`.

## The Captures page

The control center has a Captures page: the newest 40 screenshots and recordings in their folders, newest first, each with a thumbnail. A recording's is a frame ffmpeg picks among its first, made once in the background and kept in `$XDG_CACHE_HOME/mochi/thumbnails`; the card after a recording shows it too, once it's ready. Without ffmpeg, recordings show a camera instead. Files other tools saved there show up too. Click one to open it in the preview card; each row also copies it, opens a screenshot in the editor, opens its folder or deletes it. Hold a row and drag it onto another app to drop the file there, as from a file manager. Its Screenshot and Record buttons close the control center before the picker opens, so the control center isn't in the capture. `mochi ipc control-center open capture/history` opens the page.

`copy`, `edit`, `delete` and `open` take a file from the history, like `mochi ipc capture delete <path>`; without one they work on the last capture. Only files the history lists can be deleted this way.

The clipboard opens its images in the same card, through the `show <path> <entry> [label]` action: copy and edit work as for a screenshot, there's no folder, and delete removes the clipboard entry.
