//! `mochi agents hook`: Claude Code's hooks, passed on to the agents
//! module. Claude Code runs a hook with a JSON object on stdin, which
//! names the session, its folder and the event; this maps the event to
//! what the session is doing and sends it. It must never hold up or fail
//! the agent, so it waits on nothing for long and always exits 0.

use std::io::{IsTerminal, Read};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use mochi_protocol::ClientMessage;
use serde_json::Value;

/// How long it waits for the hook's JSON, then for mochid.
const PATIENCE: Duration = Duration::from_millis(1000);

/// What an event means for its session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// It's in this state now: `working`, `waiting` or `done`.
    Set(&'static str),
    /// It ended.
    Clear,
}

/// The agents module's action for one hook's JSON, or `None` when the
/// event says nothing about the session's state.
pub fn action(input: &Value, app: &str) -> Option<(String, Vec<String>)> {
    let id = input.get("session_id")?.as_str()?.trim();
    if id.is_empty() {
        return None;
    }
    let event = input.get("hook_event_name")?.as_str()?;
    match step(event, input)? {
        Step::Clear => Some(("clear".into(), vec![id.to_owned()])),
        Step::Set(state) => {
            let mut args = vec![id.to_owned(), state.to_owned(), app.to_owned()];
            // The project's folder names the session.
            let folder = input
                .get("cwd")
                .and_then(Value::as_str)
                .and_then(|cwd| Path::new(cwd).file_name())
                .map(|name| name.to_string_lossy().into_owned());
            args.extend(folder);
            Some(("set".into(), args))
        }
    }
}

/// What a Claude Code event means: see its hooks reference. Events not
/// listed say nothing, like SubagentStop, which comes while the session
/// goes on.
pub fn step(event: &str, input: &Value) -> Option<Step> {
    match event {
        "UserPromptSubmit" | "PreToolUse" | "PostToolUse" | "PostToolUseFailure"
        | "PermissionDenied" | "PreCompact" => Some(Step::Set("working")),
        "PermissionRequest" | "Elicitation" => Some(Step::Set("waiting")),
        "Notification" => {
            // Only the ones that wait on the user: not `idle_prompt`, which
            // comes a minute after a session finished, nor
            // `auth_success`. Older releases send no type.
            match input.get("notification_type").and_then(Value::as_str) {
                None
                | Some(
                    "permission_prompt"
                    | "elicitation_dialog"
                    | "elicitation_url_dialog"
                    | "agent_needs_input",
                ) => Some(Step::Set("waiting")),
                Some(_) => None,
            }
        }
        "Stop" | "StopFailure" => Some(Step::Set("done")),
        "SessionEnd" => Some(Step::Clear),
        _ => None,
    }
}

/// The agent's name when `--app` doesn't give one: T3 Code runs Claude
/// Code with variables of its own, which the hook inherits.
pub fn default_app(mut variables: impl Iterator<Item = String>) -> &'static str {
    if variables.any(|name| name.starts_with("T3CODE_")) {
        "T3 Code"
    } else {
        "Claude Code"
    }
}

/// Runs the hook. Problems go to stderr, which Claude Code doesn't show
/// for a hook that exits 0; mochid not running is no problem.
pub fn hook(app: Option<String>) {
    if let Err(error) = try_hook(app)
        && !error.starts_with("cannot reach mochid")
    {
        eprintln!("mochi agents hook: {error}");
    }
}

fn try_hook(app: Option<String>) -> Result<(), String> {
    let mut stdin = std::io::stdin();
    if stdin.is_terminal() {
        return Err("reads a hook's JSON on stdin; Claude Code passes it".into());
    }
    // A thread, so a writer that never closes stdin can't hold the agent up.
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut text = String::new();
        let _ = sender.send(stdin.read_to_string(&mut text).map(|_| text));
    });
    let text = receiver
        .recv_timeout(PATIENCE)
        .map_err(|_| "no JSON on stdin".to_owned())?
        .map_err(|error| error.to_string())?;
    let input: Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let app = app.unwrap_or_else(|| default_app(std::env::vars().map(|(name, _)| name)).into());
    let Some((action, args)) = action(&input, &app) else {
        return Ok(());
    };
    crate::request_within(
        ClientMessage::Command {
            module: "agents".into(),
            action,
            args,
        },
        Some(PATIENCE),
    )
    .map(drop)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn input(event: &str) -> Value {
        json!({
            "session_id": "abc123",
            "transcript_path": "/home/me/.claude/projects/x/abc123.jsonl",
            "cwd": "/home/me/code/mochi-shell",
            "hook_event_name": event,
        })
    }

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_owned()).collect()
    }

    #[test]
    fn events_map_to_states() {
        for (event, state) in [
            ("UserPromptSubmit", "working"),
            ("PreToolUse", "working"),
            ("PostToolUse", "working"),
            ("PermissionRequest", "waiting"),
            ("Notification", "waiting"),
            ("Stop", "done"),
            ("StopFailure", "done"),
        ] {
            assert_eq!(
                action(&input(event), "Claude Code"),
                Some((
                    "set".into(),
                    args(&["abc123", state, "Claude Code", "mochi-shell"])
                )),
                "{event}"
            );
        }
        assert_eq!(
            action(&input("SessionEnd"), "Claude Code"),
            Some(("clear".into(), args(&["abc123"])))
        );
        for event in ["SessionStart", "SubagentStop", "Whatever"] {
            assert_eq!(action(&input(event), "Claude Code"), None, "{event}");
        }
    }

    #[test]
    fn only_notifications_that_wait_on_the_user_count() {
        let mut notification = input("Notification");
        notification["notification_type"] = json!("permission_prompt");
        assert_eq!(
            step("Notification", &notification),
            Some(Step::Set("waiting"))
        );
        notification["notification_type"] = json!("idle_prompt");
        assert_eq!(step("Notification", &notification), None);
    }

    #[test]
    fn a_session_needs_an_id() {
        let mut nameless = input("Stop");
        nameless["session_id"] = json!("");
        assert_eq!(action(&nameless, "Claude Code"), None);
        assert_eq!(
            action(&json!({ "hook_event_name": "Stop" }), "Claude Code"),
            None
        );
        // Without a folder, the session goes untitled.
        assert_eq!(
            action(
                &json!({ "session_id": "s", "hook_event_name": "Stop" }),
                "T3 Code"
            ),
            Some(("set".into(), args(&["s", "done", "T3 Code"])))
        );
    }

    #[test]
    fn t3_code_is_told_apart() {
        let names = |names: &[&str]| {
            names
                .iter()
                .map(|name| (*name).to_owned())
                .collect::<Vec<_>>()
                .into_iter()
        };
        assert_eq!(
            default_app(names(&["HOME", "T3CODE_RESOURCE_MONITOR_PATH"])),
            "T3 Code"
        );
        assert_eq!(default_app(names(&["HOME", "CLAUDECODE"])), "Claude Code");
    }
}
