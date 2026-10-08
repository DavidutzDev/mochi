# The Rust SDK

`mochi-sdk` writes plugin backends in Rust. It speaks the [plugin protocol](protocol.md#plugin-backends) for you, and its `ModuleCtx` has the same calls as a builtin module's, so code moves between a plugin and a builtin with few changes. The [API documentation](api/mochi_sdk/index.html) lists every type and method; this page shows how they fit together.

## Setting up

A plugin's backend is a binary crate:

```toml
[package]
name = "mochi-plugin-pomodoro"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "pomodoro"
path = "src/main.rs"

[dependencies]
mochi-sdk = { git = "https://github.com/DavidutzDev/mochi", tag = "v0.0.7" }
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["macros", "time"] }
```

Pin the tag of the Mochi release you build against. The SDK speaks protocol API 1, which every Mochi 0.0.x speaks: a plugin built with an older SDK keeps working with a newer Mochi until the API changes.

The manifest's `build` compiles it and puts the binary where `exec` says:

```toml
[backend]
exec = "bin/pomodoro"
build = "cargo build --release --locked --target-dir target && install -Dm755 target/release/pomodoro bin/pomodoro"
```

## Running

`mochi_sdk::run` connects to mochid, says hello, runs your function on a single-threaded tokio runtime, and turns its result into the exit code:

```rust
fn main() -> std::process::ExitCode {
    mochi_sdk::run(run)
}

async fn run(mut ctx: mochi_sdk::ModuleCtx) -> Result<(), mochi_sdk::Error> {
    while let Some(event) = ctx.next_event().await {
        // ...
    }
    Ok(())
}
```

`next_event` returns `None` when mochid closes the connection: when it stops, reloads with the plugin disabled or changed, or shuts down. Return then; a backend still running 1.5 seconds later is killed. An `Err` from your function, or a panic, counts as a crash, and mochid starts the backend again.

For another runtime, like a multi-threaded one, call `ModuleCtx::connect().await` from your own `main`. It fails when the program wasn't started by mochid, so running the binary by hand says so instead of hanging.

## Settings

The plugin's `[module.<id>]` table from `config.toml` arrives with the connection. Read it into a type of your own:

```rust
#[derive(serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    focus_minutes: u64,
    area: mochi_sdk::Area,
}

impl Default for Settings {
    fn default() -> Self {
        Self { focus_minutes: 25, area: mochi_sdk::Area::CenterRight }
    }
}

let settings: Settings = ctx.settings()?;
```

With `deny_unknown_fields`, a typo in `config.toml` makes the backend exit with an error naming the key, which mochid logs. It fails the same way after each restart, so it soon stays stopped: `mochi status` says it failed, and the log has why. Fix the file and run `mochi reload`. Settings only change with a reload, which restarts the plugin, so read them once.

## Events

| `ModuleEvent` | Comes when |
|---|---|
| `Command(command)` | `mochi ipc <id> <action>`, a button in a view running `Daemon.command`, or another module's `call` |
| `Clicked(activity)` | A click on one of its activities that has no expanded view |
| `Hovered { activity, hovered }` | The pointer came onto one of its activities on the island, or left it |
| `Ended { activity, reason }` | One of its activities is gone: `Expired`, `Dismissed`, `Withdrawn`, `Replaced` or `Outside` |
| `BubbleClicked(bubble)` | A click on one of its bubbles |
| `State { module, state }` | A module from `[uses] state` published state |
| `Offers(contributions)` | What other modules offer this one, like launcher providers to a launcher: at the start, then after each reload that changes it |

To do work on a timer as well, wait on both with `tokio::select!`:

```rust
let mut tick = tokio::time::interval(std::time::Duration::from_secs(1));
loop {
    tokio::select! {
        event = ctx.next_event() => match event {
            None => return Ok(()),
            Some(event) => handle(&ctx, event),
        },
        _ = tick.tick() => refresh(&ctx),
    }
}
```

## Commands

A command has the action's `name` and its `args`, already checked against the manifest. Read arguments by name with the getter for their kind: `args.str("label")`, `args.int("minutes")`, `args.float(..)`, `args.bool(..)`. An optional argument that was left out is `None`.

Answer every command, once:

```rust
command.reply(Ok(()));                       // done
command.reply(Err("no timer runs".into()));  // mochi ipc prints it and fails
command.answer(Ok(format!("{left} left")));  // mochi ipc prints the output
```

A command can wait for its answer, held in a variable or moved into a task, while the plugin goes on with other events: the caller waits with it. Dropping a command without answering fails it for the caller.

## Activities

`present` submits an activity to the island and returns its id at once; the arbiter decides when it shows, by priority. `ActivitySpec::new("Done")` shows the view `Done.qml`, with these on top:

| Builder | Does |
|---|---|
| `.payload(json!({..}))` | What the views get as `payload` |
| `.timeout(duration)` | Ends it after this long on screen; the time stops while hovered or expanded |
| `.priority(Priority::HIGH)` | `LOW`, `NORMAL` (the default), `HIGH`, `URGENT`, or `Priority(n)` for anything between |
| `.key("timer")` | Replaces the plugin's activity with the same key instead of adding one; with the same view, it updates in place |
| `.expanded("Big")` | The view a click opens; without one, clicks come as `Clicked` |
| `.expand_for(duration)` | Opens on the expanded view, then collapses after this long |
| `.uninterruptible()` | Higher priorities wait instead of interrupting it |
| `.same_priority(SamePriority::Stack)` | Interrupts a shown activity of the same priority instead of waiting |
| `.modal()` | Takes the keyboard, for views you type into |
| `.overlay("Pick")` | A full-screen view on every monitor under the island, for picking something on screen |
| `.passive()` | A click outside doesn't close it |
| `.fleeting()` | Shows at once or not at all, never waiting in the queue |
| `.output("DP-3")` | A modal activity on this monitor only |

`update(id, payload)` changes what it shows, and `withdraw(id)` removes it. Each activity ends exactly once, with an `Ended` event.

## Bubbles

`show_bubble` puts a small view next to the island and returns its id. `BubbleSpec::new("Bubble")` shows `Bubble.qml`, a view about 26 pixels tall:

| Builder | Does |
|---|---|
| `.payload(json!({..}))` | What the view gets |
| `.area(Area::Right)` | `Left`, `CenterLeft`, `CenterRight` (the default) or `Right`; the user's `[bubbles.<id>]` settings win |
| `.key("timer")` | Replaces the plugin's bubble with the same key, keeping its place |
| `.wide("BubbleWide")` | A wider view with text, for users who set `wide = true` |
| `.group("status")` | Bubbles with the same group next to each other share a pill |
| `.order(n)` | Lower goes further left |
| `.priority(..)` | Breaks ties, decides who stays when an area is full and who is in front of a stack |
| `.news()` | With stacked bubbles, brings it to the front for a while |

`update_bubble(id, payload)` changes it in place, and `hide_bubble(id)` removes it.

## State

`publish_state(json!({..}))` replaces the plugin's state. Views read it with `Daemon.state("<id>")`, hub cards and pages get it as their `payload`, and other modules can watch it. Publish the whole state each time, not a change.

To read another module's state, name it in the manifest's `[uses] state`, then match `ModuleEvent::State`:

```rust
Some(ModuleEvent::State { module, state }) if module == "media" => {
    playing = state["status"] == "playing";
}
```

It comes once at the start when the module has published something, then on every change, and as `Value::Null` when that module stops.

## Calling other modules

`call` runs another module's action, with the same checks as `mochi ipc`, and returns a future that doesn't borrow the context:

```rust
let call = ctx.call("media", "pause", &[]);
tokio::spawn(async move {
    match call.await {
        Ok(()) | Err(CallError::NotEnabled(_)) => {}
        Err(error) => eprintln!("pausing failed: {error}"),
    }
});
```

`ask` does the same and returns what the action answered with, like the output `mochi ipc` prints: `Ok(Some(text))`, or `Ok(None)` for an action that answered nothing. The launcher asks [providers](launcher-providers.md) this way.

`CallError::NotEnabled` means the module isn't enabled: the usual case for using another module when it's there and carrying on when it isn't. Spawn the call rather than awaiting it in your event loop when the other module might call back: each would wait for the other.

## The compositor

`ctx.compositor()` is the latest state: `outputs`, `workspaces`, `focused_output`, `focused_app`, and `screencast` with what's `captured`. `compositor_changes()` is a `tokio::sync::watch::Receiver` that wakes on every new state. Check `backend`: without a supported compositor it's `"unsupported"` and the lists stay empty.

`activate_workspace(id)`, `windows()` and `pointer_output()` ask the compositor and return futures, like `call`.

## Files

`data_dir()` is a directory only this plugin writes to, emptied when mochid starts: for files its views load, like an image it downloaded. `session_dir()` keeps its files across mochid restarts, until you log out. Both are in the runtime directory. For anything that should last longer, use `$XDG_DATA_HOME` or `$XDG_CACHE_HOME` yourself.

The backend starts in the plugin's directory, which `MOCHI_PLUGIN_DIR` also names.

## Logging

What the backend prints goes to mochid's log, tagged with the plugin's id: stdout at the info level, stderr as warnings. `journalctl --user -u mochid` shows them under systemd. `println!` is fine: the protocol has a socket of its own.

## Testing

Logic that doesn't need mochid tests like any Rust code. To test the plugin's conversation, play mochid: `ModuleCtx::over` takes one end of a socket pair, and the test sends mochid's `hello` and reads what the plugin sends, with the message types from `mochi_sdk::protocol`:

```rust
{{#include ../../../crates/mochi-sdk/tests/talk.rs:test}}
```

To try it for real, list the plugin with a `path:` source and run `mochi plugins install <id>`, which builds it; `mochi reload` restarts it after each rebuild.

## Between a plugin and a builtin

`ModuleCtx`, `ModuleEvent`, `ModuleCommand`, `ActivitySpec`, `BubbleSpec`, `Priority`, `Area`, `Args` and `CallError` have the same names and calls in `mochi-sdk` and in `mochi-core`, so a plugin can become a builtin module, or the reverse, with few changes. The differences:

| | Builtin (`mochi-core`) | Plugin (`mochi-sdk`) |
|---|---|---|
| Entry point | `impl Module`, with `run(self, ctx)` | `mochi_sdk::run(function)` |
| Actions, contributions, views | Declared in Rust | Declared in the manifest |
| `settings::<T>()` errors | `toml::de::Error` | `serde_json::Error` |
| Watching state | `ctx.watch_state("media")` | `[uses] state` in the manifest |
| Compositor | `ctx.compositor()` returns a handle with `state()` and `subscribe()` | `compositor()` returns the state; `compositor_changes()` subscribes |
| Virtual monitors | `create_virtual_output` and the like | Not yet |
