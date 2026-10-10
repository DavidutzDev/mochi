# Making an SDK

A plugin backend can be written in any language: mochid starts a program and talks JSON lines with it over a socket. A backend can speak the [plugin protocol](protocol.md#plugin-backends) directly, as the `sh` one on the [Writing plugins](writing-plugins.md#a-backend-in-any-language) page does, but a small library makes plugins in a language pleasant to write. This page is a guide to writing that library: what it must do, what it should do, and a complete one in Python to start from.

The Python SDK below is `examples/python/hello/mochi_plugin.py` in the repository, with an example plugin next to it. Mochi's test suite runs that plugin in a real mochid, so the code on this page works.

## What an SDK does

### 1. Connect

mochid starts the backend with these:

| | |
|---|---|
| File descriptor 3 | One end of a Unix stream socket to mochid. `MOCHI_PLUGIN_FD` holds the number, `3`; read it rather than assume it. |
| `MOCHI_PLUGIN_DIR` | The plugin's directory, which is also the working directory. |
| stdout, stderr | Read by mochid line by line into its log, tagged with the plugin's id. |
| stdin | Empty. |

Wrap the descriptor in the language's socket type: `socket.socket(fileno=3)` in Python, `net.FileConn(os.NewFile(3, ""))` in Go, `new net.Socket({ fd: 3 })` in Node. When `MOCHI_PLUGIN_FD` isn't set, the program was run by hand: exit with a message saying mochid starts it, rather than wait forever.

### 2. Read and write lines

Every message is one JSON object on one line, ended by `\n`, at most 1 MiB. Keep a buffer and split it at newlines: a read can return half a line or several. Write each message whole; when several threads or tasks send, write under a lock so lines don't interleave.

### 3. Say hello

mochid speaks first:

```json
{"type":"hello","api":1,"version":"0.1.0","module":"hello","settings":{},"data_dir":"…","session_dir":"…","compositor":{…}}
```

Check `api`: an SDK for API 1 should refuse anything else with a clear message. Keep `module`, `settings`, `data_dir`, `session_dir` and `compositor` for the plugin. Then answer within 5 seconds, or mochid stops the backend:

```json
{"type":"hello","api":1}
```

### 4. Number activities and bubbles

The backend picks the ids of its activities and bubbles: count up from 1 and return the number from `present` and `show_bubble`, so the plugin can update or remove what it showed. Events about them, like `ended` or `bubble_clicked`, carry the same numbers. Activities and bubbles may share one counter.

### 5. Match answers to questions

`call`, `activate_workspace`, `windows` and `pointer_output` carry an `id`, and their answer, `call_result` or `compositor_result`, repeats it. Use a second counter for these. Other messages can come before the answer: keep them for later rather than drop them. An async SDK keeps a table of pending ids with a future or callback each; a blocking SDK can read until the answer comes and queue the rest, as the Python one does.

### 6. Answer every command

A `command` carries an `id`, the `action` and its `args` as an object, already checked against the manifest. The caller waits until the backend sends a `reply` with that `id`: with nothing for success, `error` for a failure, or `output` for `mochi ipc` to print. Make it hard to forget: Rust's SDK fails a command dropped unanswered, and an SDK with destructors or context managers can do the same.

### 7. Keep the compositor's state

`compositor` messages replace the whole state. Keep the latest for the plugin to read, and let it wait for changes if the language has a way.

### 8. Stop when mochid closes

When the socket reaches its end, mochid is stopping the plugin: end the event loop, let the plugin finish, and exit. 1.5 seconds later the backend is killed. Fail pending questions then, so nothing waits forever.

### 9. Let unknown messages through

The protocol only grows: a newer mochid may send message types or fields the SDK doesn't know. Ignore unknown fields, and pass unknown messages on to the plugin or drop them, but don't fail on them.

## What an SDK should have

- One call per message the backend sends, named like the protocol: `publish_state`, `present`, `update`, `withdraw`, `show_bubble`, `update_bubble`, `hide_bubble`, `call`, `reply`, `activate_workspace`, `windows`, `pointer_output`. The Rust SDK's names are the same, which helps people move between them. `call` returns the `output` its `call_result` carries, for actions that answer with something, like a launcher provider's `search`.
- Settings with defaults. `settings` holds only what the user wrote, so the plugin fills in the rest.
- A way to wait for events and a timer at once, because most plugins also do something every second or minute: a timeout on the event wait, an async stream, or a callback API on an event loop.
- Activity and bubble specs that need only the view: in the protocol, only `compact` and `view` are required, and everything else has a default.

## A Python SDK

The whole library, about 200 lines with only the standard library:

```python
{{#include ../../../examples/python/hello/mochi_plugin.py}}
```

How it does each part:

| Part | In the code |
|---|---|
| Connect | `socket.socket(fileno=int(fd))` |
| Lines | `_read` splits a byte buffer at `\n`; `_send` writes `json.dumps(message) + "\n"` |
| Hello | The constructor reads mochid's `hello`, checks `api` and answers |
| Numbers | `_number` counts activities and bubbles; `_next_request` counts questions |
| Answers | `_ask` reads until its answer comes, keeping other messages in `_backlog` |
| Commands | `reply(event, error=…, output=…)` |
| Compositor | `events` keeps `self.compositor` up to date and doesn't yield those messages |
| Stop | `events` returns when the socket ends |
| Unknown messages | `events` yields every message it doesn't handle itself; the plugin skips what it doesn't know |
| Timers | `events(timeout=1.0)` yields `None` when nothing came for a second |

A plugin with it:

```python
{{#include ../../../examples/python/hello/hello.py}}
```

and its manifest:

```toml
{{#include ../../../examples/python/hello/mochi-plugin.toml}}
```

The backend is the script itself, so Python must be installed where the plugin runs. A plugin can ship its SDK file next to its code, as this one does, or depend on a package.

## Testing an SDK

Test the SDK the way mochid uses it: make a socket pair, give one end to the SDK, and play mochid on the other, sending `hello` first. Check that it answers `hello`, numbers what it shows, matches answers to questions with other messages in between, answers commands, and ends when you close your end. Rust's SDK does this in `crates/mochi-sdk/tests/talk.rs`, and the Rust SDK page shows it.

Then run a plugin built with it in a real mochid: list it in plugins.toml with a `path:` source, enable it, and drive it with `mochi ipc`. `mochi status` says whether it runs, and mochid's log has what it printed. `crates/mochid/tests/plugins.rs` does the same automatically for the Python example and for a backend in `sh`.
