# Agents

Coding agents on the island: whether each session is working, waiting for you, or done. Claude Code says so through its hooks, which run `mochi agents hook`. T3 Code runs Claude Code underneath, so the same hooks cover its sessions too, named "T3 Code" on the island.

The module is off by default. Add `"agents"` to `modules` in `config.toml`, or turn it on in the settings panel.

## The island

One bubble next to the island holds a mark for each session, in the order they started, so a session keeps its place:

- working: the agent's icon with a short arc turning round it;
- waiting for you: a raised hand on the accent;
- done: a green check.

Past four sessions, the rest are counted. Resting the pointer on the bubble lists them, like "mochi-shell needs you". A click opens the list of sessions, with the agent, the project and how long each has been in its state; the cross forgets one, and Clear done forgets the ones that finished. When every session is done, a click on the bubble clears them instead. With `wide = true` in `[bubbles.agents]`, the bubble says what the one session does, or how many do what.

When a session starts waiting, a notice says so, like "Claude Code needs you · mochi-shell", and goes once you answer. When one finishes, a shorter notice says it's done. A click on either opens the list. `waiting_ms` and `done_ms` set how long each stays, 8 and 3 seconds by default, and `waiting_notice` and `done_notice` turn them off.

A session goes when the agent ends it, and after `stale_minutes` without news from it, for an agent that was closed without saying so.

```toml
{{#include ../../../../modules/agents/settings.toml}}
```

## Claude Code's hooks

Add these to `~/.claude/settings.json`, next to what's there:

```json
{
  "hooks": {
    "UserPromptSubmit": [{ "hooks": [{ "type": "command", "command": "mochi agents hook" }] }],
    "PreToolUse": [{ "hooks": [{ "type": "command", "command": "mochi agents hook" }] }],
    "PostToolUse": [{ "hooks": [{ "type": "command", "command": "mochi agents hook" }] }],
    "PermissionRequest": [{ "hooks": [{ "type": "command", "command": "mochi agents hook" }] }],
    "Notification": [{ "hooks": [{ "type": "command", "command": "mochi agents hook" }] }],
    "Stop": [{ "hooks": [{ "type": "command", "command": "mochi agents hook" }] }],
    "StopFailure": [{ "hooks": [{ "type": "command", "command": "mochi agents hook" }] }],
    "SessionEnd": [{ "hooks": [{ "type": "command", "command": "mochi agents hook" }] }]
  }
}
```

`mochi agents hook` reads the JSON Claude Code passes on stdin and sends the session's state to the module:

| Event | State |
|---|---|
| `UserPromptSubmit`, `PreToolUse`, `PostToolUse` | working |
| `PermissionRequest`, `Notification` | waiting for you |
| `Stop`, `StopFailure` | done |
| `SessionEnd` | the session goes |

`Notification` counts only when it waits on you, like a permission prompt or a question; the one Claude Code sends a minute after a session finished doesn't. The session is named after its project's folder, the `cwd` Claude Code passes.

The hook prints nothing, waits at most a second for mochid, and always exits 0, also when mochid isn't running or the module is off, so it never holds up the agent. `--app` names the agent on the island; without it, the hook says "T3 Code" when T3 Code runs it and "Claude Code" otherwise. If Claude Code can't find `mochi`, give its full path in `command`.

## Other agents

Any agent with hooks of its own can say the same with `mochi ipc`:

```sh
mochi ipc agents set "$SESSION" working Codex my-project
mochi ipc agents set "$SESSION" done
mochi ipc agents clear "$SESSION"
```

| Action | What it does |
|---|---|
| `set <id> <state> [app] [title]` | Says what a session does: `working`, `waiting` or `done`. An empty app or title keeps the one it had |
| `clear <id>` | Forgets a session, as when it ends |
| `clear-done`, `clear-all` | Forgets the sessions that are done, or every one |
| `show` | Opens the list on the island |
