#!/usr/bin/env python3
"""An example plugin backend in Python, with the SDK in mochi_plugin.py.

`mochi ipc hello say [text...]` shows the text on the island and counts it
in a bubble. A click on the bubble opens the hub, when it's enabled.
"""

from mochi_plugin import CallError, Plugin

plugin = Plugin()
greeting = plugin.settings.get("greeting", "Hello from Python")
said = 0
bubble = None

plugin.publish_state({"said": said})

for event in plugin.events():
    kind = event["type"]
    if kind == "command" and event["action"] == "say":
        text = event["args"].get("text") or greeting
        plugin.present("Hello", payload={"text": text}, timeout_ms=3000)
        said += 1
        plugin.publish_state({"said": said})
        if bubble is None:
            bubble = plugin.show_bubble("Bubble", payload={"said": said}, area="right", news=True)
        else:
            plugin.update_bubble(bubble, {"said": said})
        plugin.reply(event, output=f"said {text!r}")
    elif kind == "command" and event["action"] == "count":
        plugin.reply(event, output=str(said))
    elif kind == "command":
        plugin.reply(event, error=f"no action {event['action']}")
    elif kind == "bubble_clicked":
        try:
            plugin.call("hub", "open")
        except CallError as error:
            # Without the hub there's nothing to open: fine.
            if error.kind != "not_enabled":
                print(f"opening the hub failed: {error}", flush=True)
