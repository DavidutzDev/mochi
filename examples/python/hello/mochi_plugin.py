"""A small Mochi plugin SDK for Python, in one file and the standard library.

It speaks the plugin protocol: JSON lines over the socket mochid passes as
file descriptor 3. Copy this file next to your backend and import it.

    from mochi_plugin import Plugin

    plugin = Plugin()
    for event in plugin.events():
        if event["type"] == "command":
            plugin.present("Hello", payload={"text": "Hi"}, timeout_ms=3000)
            plugin.reply(event)
"""

import json
import os
import select
import socket
import sys

API = 1


class CallError(Exception):
    """Another module's action failed: `kind` is `not_enabled`, `itself`,
    `unknown_action`, `invalid_args` or `failed`."""

    def __init__(self, error):
        self.kind = error.get("kind", "failed")
        self.detail = error.get("detail")
        super().__init__(f"{self.kind}: {self.detail}")


class Plugin:
    """The connection to mochid. Creating it says hello."""

    def __init__(self):
        fd = os.environ.get("MOCHI_PLUGIN_FD")
        if fd is None:
            sys.exit("MOCHI_PLUGIN_FD isn't set: mochid starts plugin backends")
        self._socket = socket.socket(fileno=int(fd))
        self._buffer = b""
        self._backlog = []  # messages read while waiting for an answer
        self._next_id = 0  # activities and bubbles
        self._next_request = 0  # calls and compositor questions
        self.closed = False

        hello = self._read()
        if hello is None or hello.get("type") != "hello":
            sys.exit("mochid didn't start with hello")
        if hello["api"] != API:
            sys.exit(f"mochid speaks api {hello['api']}, this plugin speaks {API}")
        self.module = hello["module"]
        self.version = hello["version"]
        self.settings = hello.get("settings") or {}
        self.data_dir = hello["data_dir"]
        self.session_dir = hello["session_dir"]
        self.compositor = hello["compositor"]
        self._send({"type": "hello", "api": API})

    # Events

    def events(self, timeout=None):
        """Yields mochid's events as dicts, until it closes the connection.

        With `timeout` in seconds, yields None when nothing came for that
        long, for work on a timer. Answers and compositor changes are taken
        care of here: `self.compositor` is always the latest state."""
        while True:
            message = self._next(timeout)
            if message is None:
                if self.closed:
                    return
                yield None
                continue
            if message["type"] == "compositor":
                self.compositor = message["state"]
                continue
            yield message

    def reply(self, command, error=None, output=None):
        """Answers a `command` event: with `error` for a failure, with
        `output` for `mochi ipc` to print, or with neither."""
        message = {"type": "reply", "id": command["id"]}
        if error is not None:
            message["error"] = error
        if output is not None:
            message["output"] = output
        self._send(message)

    # Requests that need no answer

    def publish_state(self, state):
        """Replaces the plugin's state, which its views and other modules read."""
        self._send({"type": "publish_state", "state": state})

    def present(self, compact, **spec):
        """Submits an activity showing the view `compact`. `spec` takes the
        protocol's fields: payload, timeout_ms, priority, expanded, key...
        Returns its id."""
        number = self._number()
        self._send({"type": "present", "id": number, "spec": {"compact": compact, **spec}})
        return number

    def update(self, activity, payload):
        self._send({"type": "update", "id": activity, "payload": payload})

    def withdraw(self, activity):
        self._send({"type": "withdraw", "id": activity})

    def show_bubble(self, view, **spec):
        """Shows a bubble with the view `view`. `spec` takes payload, area,
        group, order, priority, news, key, wide. Returns its id."""
        number = self._number()
        self._send({"type": "show_bubble", "id": number, "spec": {"view": view, **spec}})
        return number

    def update_bubble(self, bubble, payload):
        self._send({"type": "update_bubble", "id": bubble, "payload": payload})

    def hide_bubble(self, bubble):
        self._send({"type": "hide_bubble", "id": bubble})

    # Requests with an answer. They wait for it; events that come meanwhile
    # wait in the backlog for `events`.

    def call(self, module, action, *args):
        """Runs another module's action. Raises CallError when it fails."""
        answer = self._ask({"type": "call", "module": module, "action": action, "args": list(args)})
        if answer.get("error"):
            raise CallError(answer["error"])

    def activate_workspace(self, workspace):
        self._compositor({"type": "activate_workspace", "workspace": workspace})

    def windows(self):
        return self._compositor({"type": "windows"})

    def pointer_output(self):
        return self._compositor({"type": "pointer_output"})

    def _compositor(self, request):
        answer = self._ask(request)
        if answer.get("error"):
            raise RuntimeError(answer["error"])
        return answer.get("value")

    def _ask(self, request):
        self._next_request += 1
        request["id"] = self._next_request
        self._send(request)
        while True:
            message = self._read()
            if message is None:
                raise ConnectionError("mochid closed the connection")
            if message["type"] in ("call_result", "compositor_result") and message["id"] == request["id"]:
                return message
            self._backlog.append(message)

    # The wire

    def _number(self):
        self._next_id += 1
        return self._next_id

    def _send(self, message):
        try:
            self._socket.sendall(json.dumps(message).encode() + b"\n")
        except OSError:
            self.closed = True

    def _next(self, timeout):
        if self._backlog:
            return self._backlog.pop(0)
        if b"\n" not in self._buffer and not self.closed:
            ready, _, _ = select.select([self._socket], [], [], timeout)
            if not ready:
                return None
        return self._read()

    def _read(self):
        """The next message, or None once mochid closed the connection."""
        while b"\n" not in self._buffer:
            chunk = self._socket.recv(65536)
            if not chunk:
                self.closed = True
                return None
            self._buffer += chunk
        line, self._buffer = self._buffer.split(b"\n", 1)
        try:
            return json.loads(line)
        except ValueError:
            return self._read()
