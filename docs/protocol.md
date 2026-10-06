# Mochi protocol

`mochid` talks to the Quickshell UI and to the `mochi` CLI over one Unix socket. The Rust definitions live in `crates/mochi-protocol`, and this page describes the conversation they implement.

## Transport

- The socket is `$XDG_RUNTIME_DIR/mochi/mochi.sock`. The daemon passes the path to Quickshell in `MOCHI_SOCKET`.
- Each message is one JSON object followed by `\n`. Lines longer than 1 MiB are rejected.
- Every object has a `type` field naming the message in snake_case.
- The current API version is `1`.

## Handshake

The first message on every connection is the client's `hello`:

```json
{"type":"hello","api":1,"role":"ui"}
```

`role` is `ui` for Quickshell and `ctl` for the CLI and other control clients. The daemon answers with its own `hello`:

```json
{"type":"hello","api":1,"version":"0.0.5"}
```

If the API versions differ, the daemon answers with an `unsupported_api` error and closes the connection. Any other message before `hello` gets a `hello_first` error and the connection closes.

## UI connections

After the handshake, the daemon sends the UI everything it needs to draw, in this order:

1. `modules`, the enabled modules.
2. `contributions`, what modules offer each other.
3. `theme`, the design tokens.
4. One `state` per module that has published state.
5. `present`, what the island shows now.
6. `bubbles`, every bubble.

After that, the daemon pushes those messages again whenever they change. A reconnecting UI gets the full set again, so it never has to keep state across restarts.

The UI sends `event` messages, which the daemon never answers:

```json
{"type":"event","activity":7,"kind":"click"}
```

`kind` is `click`, `hover_enter`, `hover_leave`, `dismiss` or `outside`, a click outside the island. Events name the activity they happened on. The daemon ignores events for an activity that is no longer shown, because they arrive while the island switches views.

A click on a bubble is a `bubble_click`, also never answered. The daemon passes it to the bubble's module and drops clicks on bubbles that are gone:

```json
{"type":"bubble_click","bubble":12}
```

The UI may also send `command` to run module actions, for example from a button in a view.

## Control connections

Control clients send one request at a time and read exactly one answer before sending the next.

| Request | Answer |
|---|---|
| `{"type":"command","module":"osd","action":"volume","args":["+5"]}` | `ok` or `error` |
| `{"type":"status"}` | `status` |
| `{"type":"reload"}` | `ok` or `error` |
| `{"type":"dismiss"}` | `ok`: closes what the island shows, as a right click does |
| `{"type":"list_actions"}` or `{"type":"list_actions","module":"osd"}` | `actions` |

Command arguments are always strings, as typed on the command line. The daemon checks them against the action's declared arguments before the module sees them, and answers `invalid_args` with a usage line when they don't fit.

## Messages

### Client to daemon

| Type | Fields | Sent by |
|---|---|---|
| `hello` | `api`, `role` | everyone, first |
| `command` | `module`, `action`, `args` (optional, defaults to `[]`) | UI and control |
| `event` | `activity`, `kind` | UI |
| `bubble_click` | `bubble` | UI |
| `status` | | control |
| `reload` | | control |
| `dismiss` | | control |
| `list_actions` | `module` (optional) | control |

### Daemon to client

| Type | Fields | Sent to |
|---|---|---|
| `hello` | `api`, `version` | everyone |
| `modules` | `modules` | UI |
| `contributions` | `contributions` | UI |
| `state` | `module`, `state` (any JSON) | UI |
| `present` | `activity` (object or `null`) | UI |
| `bubbles` | `bubbles`, `overflow` (optional) | UI |
| `theme` | `theme` | UI |
| `ok` | | whoever sent `command` or `reload` |
| `output` | `output`: what the command says, like the share picker's choice | whoever sent `command` |
| `status` | `status`: `version`, `api`, `ui_connected`, `modules`, `compositor` (`backend`, `outputs`, `workspaces`), `plugins` (each `id`, `state` and an optional `message`; left out without plugins) | control |
| `actions` | `modules`: list of `{module, actions}` | control |
| `error` | `code`, `message` | everyone |

### Activity

`present` carries the activity on screen:

```json
{
  "type": "present",
  "activity": {
    "id": 7,
    "module": "notifications",
    "view": "Compact",
    "payload": {"title": "Hello"},
    "expanded": false,
    "expandable": true,
    "key": "group-42"
  }
}
```

The UI loads `root:/modules/<module>/<view>.qml` and passes `payload` to it. `activity` is `null` only when no module has anything to show, not even the idle pill.

`modal` is only present when `true`. A modal activity takes the keyboard while it's shown, for views you type into like the launcher, and the UI dismisses it on a click outside the island or Escape. The daemon also marks an activity modal while the user has it expanded with a click.

`overlay` is only present when the module set one, and implies `modal`. It names a second view, `root:/modules/<module>/<overlay>.qml`, which the UI draws full-screen on every monitor, under the island, while the activity shows. The overlay gets the same `payload` and a `screen` property with its monitor. If it has a `ready` property, the island waits for it to turn `true` before showing the activity, so an overlay can freeze the screen before the island changes. Overlays are for picking something on screen, like the capture module's region.

`output` is only present for a modal activity shown on one monitor, by name, from `[island] panels`. The islands on other monitors keep what they showed.

`outside` is only present when `true`: a click outside the island closes the activity, so the UI catches every click while it shows and sends an `outside` event. `modal` implies it.

`key` is only present when the module set one. A module uses it to replace its own activity, for example a volume OSD on every volume step. When the next `present` has the same `module`, `key` and `view` as the shown activity, the UI updates the view's `payload` in place instead of switching views, even though the `id` is new.

### Bubbles

`bubbles` lists every bubble on screen, in drawing order: the five areas from left to right (`left`, `center-left`, `center`, `center-right`, `right`), and within each area from left to right. The daemon has already applied the user's placement from `config.toml`, the sort order and the per-area maximum.

```json
{
  "type": "bubbles",
  "bubbles": [
    {"id": 12, "module": "media", "key": "media", "view": "BubbleWide", "wide": true, "payload": {"title": "Song"}, "area": "center-left"},
    {"id": 14, "module": "bluetooth", "view": "Battery", "payload": {"percent": 80}, "area": "right", "group": "status"},
    {"id": 15, "module": "volume", "view": "Level", "payload": {"percent": 40}, "area": "right", "group": "status"}
  ],
  "overflow": [{"area": "right", "hidden": 2}]
}
```

Consecutive bubbles in the same area with the same `group` share one pill. A bubble's view is small, about 26 pixels, and drawn in a circle, unless `wide` is `true`: then the user asked for the module's wide view, drawn in a pill. `key` and `group` are only present when set, and `wide` only when true. As with activities, a bubble with the same `module`, `key` and `view` as one already drawn continues it: the UI updates its payload in place. `overflow` names the areas that left bubbles out, and how many; it is left out when nothing was.

### Contributions

`contributions` lists what every enabled module offers other modules, like the hub's cards and pages. Modules declare them up front, so the list doesn't change while the daemon runs.

```json
{
  "type": "contributions",
  "contributions": [
    {"module": "media", "target": "hub", "kind": "card", "id": "now-playing", "view": "Card", "title": "Now playing", "order": 10, "options": {"span": 2}}
  ]
}
```

`target` is the module meant to use it, and `kind` is one of the kinds that module takes. The target's view loads `root:/modules/<module>/<view>.qml` and passes it the offering module's latest `state` as `payload`. `icon` and `options` are only present when set. A contribution to a module that isn't enabled is simply unused.

### Action descriptions

`actions` lists what `mochi ipc` can run:

```json
{
  "name": "volume",
  "help": "Change the volume",
  "args": [
    {"name": "delta", "help": "Percent to add", "kind": {"type": "int"}, "optional": false, "rest": false}
  ]
}
```

`kind.type` is `string`, `int`, `float`, `bool` or `choice`. A `choice` also has `values`. An `optional` argument may be left out, and only trailing arguments can be optional. A `rest` argument takes every remaining word, joined with spaces, and only the last argument can be `rest`.

### Error codes

| Code | Meaning |
|---|---|
| `bad_message` | The line was not a valid message. |
| `hello_first` | Something other than `hello` arrived first. |
| `unsupported_api` | The client speaks another API version. |
| `not_allowed` | The connection's role may not send this message. |
| `unknown_module` | No enabled module has this id. |
| `unknown_action` | The module has no such action. |
| `invalid_args` | The arguments don't match the action. The message includes a usage line. |
| `module_failed` | The module accepted the command and then reported a failure, or isn't running. |
| `invalid_config` | `reload` found an error in a config file. The message names the file and the key. |
| `internal` | A daemon bug. |

## Plugin backends

A plugin's backend talks to `mochid` over a connection of its own, not the main socket. `mochid` starts the backend with one end of a socket pair as file descriptor 3, named in `MOCHI_PLUGIN_FD`, and the plugin's directory in `MOCHI_PLUGIN_DIR`, which is also its working directory. Messages are the same JSON lines. What the backend prints on stdout and stderr goes to mochid's log.

`mochid` speaks first:

```json
{"type":"hello","api":1,"version":"0.0.5","module":"pomodoro","settings":{"focus_minutes":25},"data_dir":"/run/user/1000/mochi/data/pomodoro","session_dir":"/run/user/1000/mochi/session/pomodoro","compositor":{"backend":"wayland","outputs":[],"workspaces":[]}}
```

`settings` is the plugin's `[module.<id>]` table from config.toml. The backend answers within 5 seconds with the API version it speaks, or mochid stops it:

```json
{"type":"hello","api":1}
```

From then on, either side may send at any time. The backend picks the numbers of its activities and bubbles, and events about them use those numbers. The backend should exit when mochid closes the socket; after 1.5 seconds it is killed.

### Backend to daemon

| Type | Fields | Does |
|---|---|---|
| `publish_state` | `state` | Replaces the plugin's state, which views and other modules read |
| `present` | `id`, `spec` | Submits an activity: see below |
| `update` | `id`, `payload` | Replaces an activity's payload |
| `withdraw` | `id` | Removes an activity |
| `show_bubble` | `id`, `spec` | Shows a bubble, or replaces the one with the same key |
| `update_bubble` | `id`, `payload` | Replaces a bubble's payload |
| `hide_bubble` | `id` | Removes a bubble |
| `reply` | `id`, `output` (optional), `error` (optional) | Answers a `command` |
| `call` | `id`, `module`, `action`, `args` | Runs another module's action; answered with `call_result` |
| `activate_workspace` | `id`, `workspace` | Answered with `compositor_result` |
| `windows` | `id` | The windows on visible workspaces; answered with `compositor_result` |
| `pointer_output` | `id` | The output under the pointer; answered with `compositor_result` |

An activity `spec` needs only `compact`, the view's name. The rest are optional: `expanded`, `expand_for_ms`, `payload`, `priority` (0 to 255, 50 by default), `timeout_ms`, `interruptible` (true by default), `same_priority` (`queue` or `stack`), `modal`, `overlay`, `passive`, `fleeting`, `output` and `key`. A bubble `spec` needs only `view`; the rest are `wide`, `payload`, `area`, `group`, `order`, `priority`, `news` and `key`. They mean what the builder methods of the same name in `mochi-sdk` do.

### Daemon to backend

| Type | Fields | Means |
|---|---|---|
| `command` | `id`, `action`, `args` | An action to run, with `args` checked against the manifest, as an object like `{"minutes": 5}`. Answer with `reply` and the same `id` |
| `clicked` | `activity` | A click on an activity without an expanded view |
| `ended` | `activity`, `reason` | An activity is gone: `expired`, `dismissed`, `withdrawn`, `replaced` or `outside` |
| `bubble_clicked` | `bubble` | A click on a bubble |
| `state` | `module`, `state` | A module from the manifest's `[uses] state` published state; `null` when it stopped |
| `compositor` | `state` | The compositor's state changed |
| `offers` | `offers` | What other modules offer this plugin: every contribution whose `target` is its id, like launcher providers for a plugin that is a launcher. Sent at the start, then when a reload changes it |
| `call_result` | `id`, `output` (optional), `error` (optional) | `output` is what the action answered with; `error` is `{"kind": "not_enabled", "detail": "media"}` and the like |
| `compositor_result` | `id`, `value`, `error` (optional) | |

A backend that exits with an error is started again after 250 ms, then after twice as long each time, up to 8 seconds. After 5 crashes within a minute it stays stopped until `mochi reload`, and a desktop notification says so.

## Changing the protocol

The protocol only grows. Adding a message type or adding a field with a default keeps API version `1`. Removing or renaming a message or field, or changing what a field means, requires a new API version.
