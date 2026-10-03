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
{"type":"hello","api":1,"version":"0.1.0"}
```

If the API versions differ, the daemon answers with an `unsupported_api` error and closes the connection. Any other message before `hello` gets a `hello_first` error and the connection closes.

## UI connections

After the handshake, the daemon sends the UI everything it needs to draw, in this order:

1. `modules`, the enabled modules.
2. `theme`, the design tokens.
3. One `state` per module that has published state.
4. `present`, what the island shows now.

After that, the daemon pushes those messages again whenever they change. A reconnecting UI gets the full set again, so it never has to keep state across restarts.

The UI sends `event` messages, which the daemon never answers:

```json
{"type":"event","activity":7,"kind":"click"}
```

`kind` is `click`, `hover_enter`, `hover_leave` or `dismiss`. Events name the activity they happened on. The daemon ignores events for an activity that is no longer shown, because they arrive while the island switches views.

The UI may also send `command` to run module actions, for example from a button in a view.

## Control connections

Control clients send one request at a time and read exactly one answer before sending the next.

| Request | Answer |
|---|---|
| `{"type":"command","module":"osd","action":"volume","args":["+5"]}` | `ok` or `error` |
| `{"type":"status"}` | `status` |
| `{"type":"reload"}` | `ok` or `error` |
| `{"type":"list_actions"}` or `{"type":"list_actions","module":"osd"}` | `actions` |

Command arguments are always strings, as typed on the command line. The daemon checks them against the action's declared arguments before the module sees them, and answers `invalid_args` with a usage line when they don't fit.

## Messages

### Client to daemon

| Type | Fields | Sent by |
|---|---|---|
| `hello` | `api`, `role` | everyone, first |
| `command` | `module`, `action`, `args` (optional, defaults to `[]`) | UI and control |
| `event` | `activity`, `kind` | UI |
| `status` | | control |
| `reload` | | control |
| `list_actions` | `module` (optional) | control |

### Daemon to client

| Type | Fields | Sent to |
|---|---|---|
| `hello` | `api`, `version` | everyone |
| `modules` | `modules` | UI |
| `state` | `module`, `state` (any JSON) | UI |
| `present` | `activity` (object or `null`) | UI |
| `theme` | `theme` | UI |
| `ok` | | whoever sent `command` or `reload` |
| `status` | `status`: `version`, `api`, `ui_connected`, `modules` | control |
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
    "expandable": true
  }
}
```

The UI loads `root:/modules/<module>/<view>.qml` and passes `payload` to it. `activity` is `null` only when no module has anything to show, not even the idle pill.

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

## Changing the protocol

The protocol only grows. Adding a message type or adding a field with a default keeps API version `1`. Removing or renaming a message or field, or changing what a field means, requires a new API version.
