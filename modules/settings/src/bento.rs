//! The Bento pages of the settings: the registry, what Bento installed,
//! and sharing a setup. The work runs as `mochid bento` commands, the same
//! the terminal runs, against the daemon's own config and socket: they
//! clone, build and install, which mochid itself never does. Each one's
//! answer comes back on a channel, so the panel stays live meanwhile.

use std::path::PathBuf;
use std::process::Stdio;

use mochi_core::ModuleCtx;
use serde_json::{Value, json};
use tokio::process::Command;
use tokio::sync::mpsc::UnboundedSender;

/// A command that finished.
#[derive(Debug)]
pub enum Done {
    Catalog(Result<Value, String>),
    /// What installing `source` would do.
    Plan(String, Result<Value, String>),
    /// An install, removal, update or try: its action, what it was for,
    /// and how it went.
    Job(String, String, Result<(), String>),
    Shared(Result<Value, String>),
}

#[derive(Debug, Default)]
pub struct Bento {
    catalog: Value,
    loading: bool,
    /// The source being planned, and its plan once it came.
    planning: Option<String>,
    plan: Value,
    /// `{action, target, running, error}`.
    job: Value,
    sharing: bool,
    shared: Value,
}

impl Bento {
    pub fn state(&self) -> Value {
        json!({
            "catalog": self.catalog,
            "loading": self.loading,
            "planning": self.planning,
            "plan": self.plan,
            "job": self.job,
            "sharing": self.sharing,
            "shared": self.shared,
        })
    }

    /// Runs one of the `bento-*` actions. `Err` for a bad request; what
    /// the command says comes later, as a [`Done`].
    pub fn command(
        &mut self,
        ctx: &ModuleCtx,
        done: &UnboundedSender<Done>,
        action: &str,
        words: &[String],
    ) -> Result<(), String> {
        let word = |index: usize| words.get(index).cloned().unwrap_or_default();
        match action {
            "bento-catalog" => {
                self.loading = true;
                let mut args = vec!["catalog".to_owned()];
                if word(0) == "refresh" {
                    args.push("--refresh".into());
                }
                spawn(ctx, done, args, |result| Done::Catalog(parse(result)));
            }
            "bento-plan" => {
                let source = word(0);
                if source.trim().is_empty() {
                    return Err("say what to install".into());
                }
                self.planning = Some(source.clone());
                self.plan = Value::Null;
                let args = vec!["plan".to_owned(), source.clone()];
                spawn(ctx, done, args, move |result| {
                    Done::Plan(source, parse(result))
                });
            }
            "bento-forget" => {
                self.planning = None;
                self.plan = Value::Null;
                if self.job["running"] != json!(true) {
                    self.job = Value::Null;
                }
                self.shared = Value::Null;
            }
            "bento-add" | "bento-remove" | "bento-update" | "bento-try" | "bento-use" => {
                if self.job["running"] == json!(true) {
                    return Err("Bento is busy with something else".into());
                }
                let target = word(0);
                let mut args = match action {
                    "bento-add" => {
                        let mut args = vec!["add".to_owned(), target.clone(), "--yes".into()];
                        if !word(1).is_empty() {
                            args.extend(["--at".to_owned(), word(1)]);
                        }
                        args
                    }
                    "bento-remove" => vec!["remove".to_owned(), target.clone(), "--yes".into()],
                    "bento-use" => vec!["use".to_owned(), target.clone()],
                    "bento-update" => vec!["update".to_owned(), "--yes".into()],
                    _ => vec!["try".to_owned(), target.clone()],
                };
                if action == "bento-update" && !target.is_empty() {
                    args.push(target.clone());
                }
                self.job = json!({
                    "action": action.trim_start_matches("bento-"),
                    "target": target,
                    "running": true,
                });
                let name = action.to_owned();
                spawn(ctx, done, args, move |result| {
                    Done::Job(name, target, result.map(drop))
                });
            }
            "bento-share" => {
                let dir = word(0);
                if dir.trim().is_empty() {
                    return Err("say which directory to write".into());
                }
                let mut args = vec!["share".to_owned(), expand(&dir), "--json".into()];
                if word(1) == "true" {
                    args.push("--wallpaper".into());
                }
                let name = words
                    .get(2..)
                    .map(|rest| rest.join(" "))
                    .unwrap_or_default();
                if !name.trim().is_empty() {
                    args.extend(["--name".to_owned(), name]);
                }
                self.sharing = true;
                self.shared = Value::Null;
                spawn(ctx, done, args, |result| Done::Shared(parse(result)));
            }
            other => return Err(format!("settings has no action {other}")),
        }
        Ok(())
    }

    /// A command finished. Installs and removals bring a new catalog.
    pub fn finished(&mut self, ctx: &ModuleCtx, sender: &UnboundedSender<Done>, done: Done) {
        match done {
            Done::Catalog(result) => {
                self.loading = false;
                self.catalog = match result {
                    Ok(catalog) => catalog,
                    Err(error) => json!({ "error": error, "packages": [], "installed": [] }),
                };
            }
            Done::Plan(source, result) => {
                if self.planning.as_deref() != Some(source.as_str()) {
                    return;
                }
                self.plan = match result {
                    Ok(plan) => plan,
                    Err(error) => json!({ "problem": error }),
                };
            }
            Done::Job(action, target, result) => {
                self.job = json!({
                    "action": action.trim_start_matches("bento-"),
                    "target": target,
                    "running": false,
                    "error": result.err(),
                });
                if action != "bento-try" {
                    self.planning = None;
                    self.plan = Value::Null;
                    let _ = self.command(ctx, sender, "bento-catalog", &[]);
                }
            }
            Done::Shared(result) => {
                self.sharing = false;
                self.shared = match result {
                    Ok(shared) => shared,
                    Err(error) => json!({ "error": error }),
                };
            }
        }
    }
}

/// Runs `mochid bento <args>` for the daemon this module runs in, and
/// sends what it printed, or the error it ended with.
fn spawn(
    ctx: &ModuleCtx,
    done: &UnboundedSender<Done>,
    args: Vec<String>,
    wrap: impl FnOnce(Result<String, String>) -> Done + Send + 'static,
) {
    // The daemon's own binary; after an upgrade replaced it, Linux names it
    // "... (deleted)", and the new one is at the same path, or on the PATH.
    let program = std::env::current_exe()
        .ok()
        .map(|exe| {
            let text = exe.to_string_lossy();
            PathBuf::from(text.strip_suffix(" (deleted)").unwrap_or(&text))
        })
        .filter(|exe| exe.is_file())
        .unwrap_or_else(|| PathBuf::from("mochid"));
    let mut command = Command::new(program);
    if let Some(config) = ctx.config_file() {
        command.arg("--config").arg(config);
    }
    if let Some(socket) = ctx.socket() {
        command.env(mochi_core::SOCKET_ENV, socket);
    }
    command
        .arg("bento")
        .args(args)
        .stdin(Stdio::null())
        .kill_on_drop(true);
    let done = done.clone();
    tokio::spawn(async move {
        let result = match command.output().await {
            Ok(output) if output.status.success() => {
                Ok(String::from_utf8_lossy(&output.stdout).into_owned())
            }
            Ok(output) => {
                let said = String::from_utf8_lossy(&output.stderr);
                let last = said
                    .lines()
                    .rev()
                    .find(|line| line.starts_with("mochi:"))
                    .or_else(|| said.lines().rev().find(|line| !line.trim().is_empty()))
                    .unwrap_or("it failed");
                Err(last.trim_start_matches("mochi:").trim().to_owned())
            }
            Err(error) => Err(format!("cannot run mochid: {error}")),
        };
        let _ = done.send(wrap(result));
    });
}

fn parse(result: Result<String, String>) -> Result<Value, String> {
    let text = result?;
    serde_json::from_str(text.trim()).map_err(|error| format!("mochid answered oddly: {error}"))
}

/// `~/` is the home directory, as the panel's users type it.
fn expand(path: &str) -> String {
    match (path.strip_prefix("~/"), std::env::var("HOME")) {
        (Some(rest), Ok(home)) => format!("{home}/{rest}"),
        _ => path.to_owned(),
    }
}
