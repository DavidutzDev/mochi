# Capture

Screenshots and screen recordings, picked on the island.

`mochi ipc capture screenshot` freezes every screen and opens the picker in its default mode, a region unless `mode` says otherwise, so you can drag right away. The island shows Region, Window, Screen and All screens: click one, or press Tab or a number, to switch. Over the frozen screens you drag a region, click a window, click a screen, or click anywhere for all of them as one image. A region can run from one screen into the next, and the screenshot joins their parts at the sharpest screen's scale, with any gap between screens left transparent. Recordings take one screen at a time. A region can be moved and resized by its corners before you take it with Enter, a double click or the Capture button. The screenshot goes to `~/Pictures/Screenshots` and to the clipboard, and the island shows it for a few seconds with buttons to copy it again, open it in an editor, open its folder or delete it. Escape or a right click cancels. The picker opens over anything on the island, the launcher or the hub included, and the frozen screen still shows it, so you can capture the shell itself; it comes back afterwards.

`mochi ipc capture record` picks the same way over the live screens, then records. A red dot next to the island shows while it records; clicking it, `mochi ipc capture stop` or `record` again stops it, and the island shows the file in `~/Videos/Recordings`. While a recording is being picked, the island also has toggles for the desktop audio (A) and the microphone (M). `record_audio` and `record_microphone` set how they start.

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
| `cancel` | Closes the picker |
| `copy`, `edit`, `delete`, `open` | Act on the last capture: copy it, open it in the editor, delete it, open its folder |

The picker sends `audio`, `microphone`, `frame`, `region`, `select` and `confirm` itself.

The clipboard opens its images in the same card, through the `show <path> <entry> [label]` action: copy and edit work as for a screenshot, there's no folder, and delete removes the clipboard entry.
