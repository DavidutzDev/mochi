//! Drop: files dragged onto the island. While they hover, the island says
//! to drop them; once dropped, it names what came and offers what fits:
//! compress, extract, merge PDFs, convert images, copy the paths, open.
//! New files go next to the first dropped one, never over an old one.
//!
//! The island sends `hover on`, `hover off` and `files`; nothing else needs
//! to call it. `mochi ipc drop files` takes paths too, one a line.
//!
//! Settings in `config.toml`, all optional:
//!
//! ```toml
//! [module.drop]
//! actions = ["zip", "extract", "merge", "png", "jpg", "webp", "copy", "open"]
//! ```

mod actions;
mod files;
mod tour;

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, Area, ArgSpec, Assets, BoxFuture, BubbleId, BubbleSpec,
    ContributionSpec, Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent, Priority,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::actions::{Plan, Step};
use crate::files::Dropped;

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

/// The hint goes on its own if the drag never says it left.
const HINT_TIMEOUT: Duration = Duration::from_secs(10);
/// How long the panel stays after an action is done.
const DONE_TIMEOUT: Duration = Duration::from_secs(4);

#[derive(Debug, Default)]
pub struct DropModule;

#[derive(Debug, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    /// The actions to offer, when they fit: zip, extract, merge, convert,
    /// copy and open.
    #[schemars(extend("items" = { "type": "string", "enum": ["zip", "extract", "merge", "convert", "copy", "open"] }))]
    actions: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            actions: actions::IDS.iter().map(|id| (*id).to_owned()).collect(),
        }
    }
}

/// Earlier names for the conversions, which `convert` replaced.
const OLD_CONVERSIONS: [&str; 3] = ["png", "jpg", "webp"];

impl Settings {
    fn load(table: &mochi_core::toml::Table) -> Result<Self, String> {
        let mut settings: Self = mochi_core::settings(table).map_err(|error| error.to_string())?;
        if let Some(unknown) = settings.actions.iter().find(|id| {
            !actions::IDS.contains(&id.as_str()) && !OLD_CONVERSIONS.contains(&id.as_str())
        }) {
            return Err(format!(
                "actions has {unknown}; the actions are {}",
                actions::IDS.join(", ")
            ));
        }
        // The old names still turn the conversions on.
        if settings
            .actions
            .iter()
            .any(|id| OLD_CONVERSIONS.contains(&id.as_str()))
        {
            settings
                .actions
                .retain(|id| !OLD_CONVERSIONS.contains(&id.as_str()));
            if !settings.actions.iter().any(|id| id == "convert") {
                settings.actions.push("convert".to_owned());
            }
        }
        Ok(settings)
    }
}

impl Module for DropModule {
    fn id(&self) -> &'static str {
        "drop"
    }

    fn assets(&self) -> Assets {
        Assets::new(&QML, concat!(env!("CARGO_MANIFEST_DIR"), "/qml"))
    }

    fn settings_example(&self) -> &'static str {
        include_str!("../settings.toml")
    }

    fn settings_schema(&self) -> Option<Value> {
        Some(mochi_core::options::schema_of::<Settings>())
    }

    fn check_settings(&self, table: &mochi_core::toml::Table) -> Result<(), String> {
        Settings::load(table).map(drop)
    }

    fn needs(&self, table: &mochi_core::toml::Table) -> Vec<mochi_core::Need> {
        let settings = Settings::load(table).unwrap_or_default();
        let wants = |id: &str| settings.actions.iter().any(|action| action == id);
        let mut needs = Vec::new();
        for (id, program, purpose) in [
            ("zip", "zip", "Compressing dropped files"),
            ("extract", "bsdtar", "Extracting dropped archives"),
            ("merge", "pdfunite", "Merging dropped PDFs"),
            ("convert", "ffmpeg", "Converting dropped videos and sound"),
            (
                "convert",
                "soffice",
                "Converting dropped documents, with LibreOffice",
            ),
            ("convert", "pandoc", "Converting dropped Markdown and HTML"),
            ("convert", "vtracer", "Converting dropped images to SVG"),
            (
                "convert",
                "magick",
                "Converting dropped HEIC and AVIF images, and images to PDF",
            ),
            ("copy", "wl-copy", "Copying dropped files' paths"),
            ("open", "xdg-open", "Opening dropped files"),
        ] {
            if wants(id) {
                needs.push(mochi_core::Need::new(program, purpose));
            }
        }
        needs
    }

    fn contributions(&self) -> Vec<ContributionSpec> {
        tour::steps()
    }

    fn actions(&self) -> Vec<ActionSpec> {
        vec![
            ActionSpec::new("files", "Offer what to do with these files").arg(ArgSpec::string(
                "files",
                "file:// URIs or paths, one a line",
            )),
            ActionSpec::new("hover", "Say to drop, or stop; the island sends this").arg(
                ArgSpec::choice(
                    "state",
                    "on while files hover over the island",
                    ["on", "off"],
                ),
            ),
            ActionSpec::new("run", "Do an action to the dropped files").arg(ArgSpec::string(
                "action",
                "zip, extract, merge, copy, open, or to- and a format, like to-png",
            )),
            ActionSpec::new("stop", "Stop the action running, and remove what it made"),
            ActionSpec::new("show", "Show where the result went"),
            ActionSpec::new("close", "Close the panel"),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            // check_settings refused unknown actions; load renames the old.
            let table: mochi_core::toml::Table = ctx.settings()?;
            let settings = Settings::load(&table).unwrap_or_default();
            let (finished, mut results) = mpsc::unbounded_channel();
            let (progress, mut progressed) = mpsc::unbounded_channel();
            let mut state = State {
                enabled: settings.actions,
                hint: None,
                panel: None,
                files: Vec::new(),
                running: None,
                message: None,
                failed: false,
                result: None,
                finished,
                closing: None,
                progress,
                bubble: None,
                working: Value::Null,
                task: None,
                making: Vec::new(),
            };
            loop {
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => state.command(&ctx, command),
                        Some(ModuleEvent::Ended { activity, .. }) => {
                            if state.hint == Some(activity) {
                                state.hint = None;
                            }
                            if state.panel == Some(activity) {
                                state.panel = None;
                                state.closing = None;
                            }
                        }
                        // The progress bubble opens the panel again.
                        Some(ModuleEvent::BubbleClicked(bubble)) if state.bubble == Some(bubble) => {
                            state.open(&ctx);
                        }
                        Some(_) => {}
                    },
                    Some((action, outcome)) = results.recv() => state.finish(&ctx, action, outcome),
                    Some(fraction) = progressed.recv() => state.progressed(&ctx, fraction),
                    _ = sleep_until(state.closing), if state.closing.is_some() => {
                        state.closing = None;
                        state.close(&ctx);
                    }
                }
            }
        })
    }
}

async fn sleep_until(deadline: Option<tokio::time::Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}

type Outcome = Result<Plan, String>;

#[derive(Debug)]
struct State {
    enabled: Vec<String>,
    hint: Option<ActivityId>,
    panel: Option<ActivityId>,
    files: Vec<Dropped>,
    /// The action running now.
    running: Option<String>,
    /// What the last action did, or why it failed.
    message: Option<String>,
    failed: bool,
    result: Option<PathBuf>,
    finished: mpsc::UnboundedSender<(String, Outcome)>,
    /// When the panel closes, after an action is done.
    closing: Option<tokio::time::Instant>,
    /// How far the running action is, from 0 to 1.
    progress: mpsc::UnboundedSender<f64>,
    /// The bubble while an action runs, and what it shows.
    bubble: Option<BubbleId>,
    working: Value,
    /// The running action's task, which stopping drops, and what it makes.
    task: Option<tokio::task::JoinHandle<()>>,
    making: Vec<PathBuf>,
}

impl State {
    fn command(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        let result = match command.action.as_str() {
            "hover" => {
                if command.args.str("state") == Some("on") {
                    if self.hint.is_none() {
                        let spec = ActivitySpec::new("Hint")
                            .key("hint")
                            .priority(Priority::URGENT)
                            .timeout(HINT_TIMEOUT);
                        self.hint = Some(ctx.present(spec));
                    }
                } else if let Some(hint) = self.hint.take() {
                    ctx.withdraw(hint);
                }
                Ok(())
            }
            "files" => {
                if let Some(hint) = self.hint.take() {
                    ctx.withdraw(hint);
                }
                let files = files::parse(command.args.str("files").unwrap_or_default());
                if files.is_empty() {
                    Err("none of those are files here".to_owned())
                } else {
                    self.files = files;
                    self.message = None;
                    self.failed = false;
                    self.result = None;
                    self.closing = None;
                    self.open(ctx);
                    Ok(())
                }
            }
            "run" => self.start(ctx, command.args.str("action").unwrap_or_default()),
            "show" => match &self.result {
                Some(result) => {
                    let folder = if result.is_dir() {
                        result.clone()
                    } else {
                        result.parent().map(PathBuf::from).unwrap_or_default()
                    };
                    let argv = ["xdg-open".to_owned(), folder.display().to_string()];
                    mochi_core::process::spawn_detached(
                        &mochi_core::process::in_app_scope(&argv),
                        None,
                    )
                }
                None => Err("nothing to show yet".to_owned()),
            },
            "close" => {
                self.close(ctx);
                Ok(())
            }
            "stop" => self.stop(ctx),
            other => Err(format!("drop has no action {other}")),
        };
        command.reply(result);
    }

    fn start(&mut self, ctx: &ModuleCtx, action: &str) -> Result<(), String> {
        if self.running.is_some() {
            return Err("an action is still running".into());
        }
        let setting = if action.starts_with("to-") {
            "convert"
        } else {
            action
        };
        if !self.enabled.iter().any(|id| id == setting) {
            return Err(format!("{setting} is off in the settings"));
        }
        let plan = actions::plan(action, &self.files, &mochi_core::process::installed)?;
        self.running = Some(action.to_owned());
        self.message = None;
        self.closing = None;
        self.update(ctx);
        // A bubble follows it, so the panel can close meanwhile.
        let label = actions::offered(&self.files, &self.enabled, &mochi_core::process::installed)
            .into_iter()
            .find(|offered| offered.id == action)
            .map(|offered| offered.label)
            .unwrap_or_else(|| action.to_owned());
        self.working = json!({
            "icon": actions::icon(action, &self.files),
            "doing": if action.starts_with("to-") { format!("Converting to {label}") } else { label },
            "files": files::summary(&self.files),
            "progress": 0.0,
        });
        self.bubble = Some(ctx.show_bubble(self.bubble_spec()));
        self.making = plan.made();
        let finished = self.finished.clone();
        let progress = self.progress.clone();
        let action = action.to_owned();
        self.task = Some(tokio::spawn(async move {
            let outcome = execute(&plan, &progress).await.map(|()| plan);
            let _ = finished.send((action, outcome));
        }));
        Ok(())
    }

    /// Stops the running action: dropping its task kills the program it
    /// runs, then what it made goes, half written or not.
    fn stop(&mut self, ctx: &ModuleCtx) -> Result<(), String> {
        let task = self.task.take().ok_or("nothing is running")?;
        task.abort();
        let action = self.running.take().unwrap_or_default();
        for path in std::mem::take(&mut self.making) {
            let removed = if path.is_dir() {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            };
            match removed {
                Ok(()) => {
                    tracing::debug!(path = %path.display(), "removed what a stopped action made")
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    tracing::warn!(path = %path.display(), %error, "can't remove what a stopped action made")
                }
            }
        }
        // LibreOffice's folder of its own.
        if let Some(parent) = self.files.first().and_then(|file| file.path.parent())
            && let Ok(entries) = std::fs::read_dir(parent)
        {
            for entry in entries.flatten() {
                if entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".mochi-convert-")
                {
                    let _ = std::fs::remove_dir_all(entry.path());
                }
            }
        }
        if let Some(bubble) = self.bubble.take() {
            ctx.hide_bubble(bubble);
        }
        tracing::info!(action, "stopped");
        self.message = Some("Stopped".to_owned());
        self.failed = false;
        self.result = None;
        self.update(ctx);
        Ok(())
    }

    fn bubble_spec(&self) -> BubbleSpec {
        BubbleSpec::new("Progress")
            .key("progress")
            .area(Area::CenterRight)
            .order(-7)
            .payload(self.working.clone())
    }

    /// The running action moved on.
    fn progressed(&mut self, ctx: &ModuleCtx, fraction: f64) {
        if let Some(bubble) = self.bubble {
            self.working["progress"] = json!(fraction.clamp(0.0, 1.0));
            ctx.update_bubble(bubble, self.working.clone());
        }
    }

    fn finish(&mut self, ctx: &ModuleCtx, action: String, outcome: Outcome) {
        self.running = None;
        self.task = None;
        self.making.clear();
        if let Some(bubble) = self.bubble.take() {
            ctx.hide_bubble(bubble);
        }
        // Closed meanwhile: say how it went on the island.
        if self.panel.is_none() {
            let (icon, text) = match &outcome {
                Ok(plan) => ("check_circle", plan.done.clone()),
                Err(error) => ("error", error.clone()),
            };
            ctx.present(
                ActivitySpec::new("Notice")
                    .key("notice")
                    .priority(Priority::HIGH)
                    .timeout(DONE_TIMEOUT)
                    .payload(json!({ "icon": icon, "text": text, "failed": outcome.is_err() })),
            );
        }
        match outcome {
            Ok(plan) => {
                tracing::info!(action, done = %plan.done, "drop");
                self.message = Some(plan.done);
                self.failed = false;
                self.result = plan.result;
                self.closing = Some(tokio::time::Instant::now() + DONE_TIMEOUT);
            }
            Err(error) => {
                tracing::warn!(action, %error, "drop action failed");
                self.message = Some(error);
                self.failed = true;
            }
        }
        self.update(ctx);
    }

    fn payload(&self) -> Value {
        let offered = actions::offered(&self.files, &self.enabled, &mochi_core::process::installed);
        json!({
            "summary": files::summary(&self.files),
            "count": self.files.len(),
            "actions": offered
                .iter()
                .map(|action| json!({
                    "id": action.id,
                    "label": action.label,
                    "icon": action.icon,
                    "convert": action.convert,
                }))
                .collect::<Vec<_>>(),
            "missing": actions::missing(&self.files, &self.enabled, &mochi_core::process::installed),
            "running": self.running,
            "message": self.message,
            "failed": self.failed,
            "result": self.result.is_some(),
        })
    }

    fn open(&mut self, ctx: &ModuleCtx) {
        if let Some(panel) = self.panel {
            ctx.update(panel, self.payload());
            return;
        }
        ctx.close_other_panels();
        let spec = ActivitySpec::new("Actions")
            .key("drop")
            .priority(Priority::URGENT)
            .uninterruptible()
            .modal()
            .payload(self.payload());
        self.panel = Some(ctx.present(spec));
    }

    fn update(&self, ctx: &ModuleCtx) {
        if let Some(panel) = self.panel {
            ctx.update(panel, self.payload());
        }
    }

    fn close(&mut self, ctx: &ModuleCtx) {
        if let Some(panel) = self.panel.take() {
            ctx.withdraw(panel);
        }
    }
}

/// Runs a plan's steps in turn; the first to fail stops it, with what it
/// printed.
async fn execute(plan: &Plan, progress: &mpsc::UnboundedSender<f64>) -> Result<(), String> {
    let total = plan.steps.len().max(1) as f64;
    for (index, step) in plan.steps.iter().enumerate() {
        let done = index as f64;
        let _ = progress.send(done / total);
        match step {
            Step::MakeDir(folder) => tokio::fs::create_dir_all(folder)
                .await
                .map_err(|error| format!("can't make {}: {error}", folder.display()))?,
            Step::Move { from, to } => {
                tokio::fs::rename(from, to)
                    .await
                    .map_err(|error| format!("can't move {}: {error}", from.display()))?;
                // The folder the program wrote in, now empty.
                if let Some(folder) = from.parent() {
                    let _ = tokio::fs::remove_dir(folder).await;
                }
            }
            Step::Image { input, output } => {
                let (input, output) = (input.clone(), output.clone());
                tokio::task::spawn_blocking(move || actions::convert_image(&input, &output))
                    .await
                    .map_err(|error| error.to_string())??;
            }
            // Apps outlive Mochi, in their own scope.
            Step::Run { program, args, .. } if program == "xdg-open" => {
                let argv: Vec<String> = std::iter::once(program.clone())
                    .chain(args.iter().map(|arg| arg.to_string_lossy().into_owned()))
                    .collect();
                mochi_core::process::spawn_detached(
                    &mochi_core::process::in_app_scope(&argv),
                    None,
                )?;
            }
            Step::Run { program, args, cwd } if program == "ffmpeg" => {
                ffmpeg(args, cwd, |fraction| {
                    let _ = progress.send((done + fraction) / total);
                })
                .await?;
            }
            Step::Run { program, args, cwd } => {
                let output = tokio::process::Command::new(program)
                    .args(args)
                    .current_dir(cwd)
                    .stdin(Stdio::null())
                    .kill_on_drop(true)
                    .output()
                    .await
                    .map_err(|error| format!("can't run {program}: {error}"))?;
                if !output.status.success() {
                    let said = String::from_utf8_lossy(&output.stderr);
                    let last = said
                        .lines()
                        .rev()
                        .find(|line| !line.trim().is_empty())
                        .unwrap_or("");
                    return Err(format!("{program} failed: {}", last.trim()));
                }
            }
        }
    }
    Ok(())
}

/// Runs ffmpeg, reporting how far into the input it is, from its
/// `-progress` lines against the length ffprobe reads.
async fn ffmpeg(
    args: &[std::ffi::OsString],
    cwd: &std::path::Path,
    report: impl Fn(f64),
) -> Result<(), String> {
    use tokio::io::{AsyncBufReadExt, BufReader};

    let input = args
        .iter()
        .position(|arg| arg == "-i")
        .and_then(|index| args.get(index + 1));
    let length = match input {
        Some(input) => duration(input).await,
        None => None,
    };
    let mut child = tokio::process::Command::new("ffmpeg")
        .args(["-progress", "pipe:1", "-nostats"])
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("can't run ffmpeg: {error}"))?;
    let mut said = String::new();
    let mut errors = child.stderr.take().map(BufReader::new);
    if let Some(stdout) = child.stdout.take() {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            // Microseconds, whatever the name says.
            if let (Some(length), Some(time)) = (length, line.strip_prefix("out_time_us="))
                && let Ok(micros) = time.trim().parse::<f64>()
            {
                report((micros / 1_000_000.0 / length).clamp(0.0, 1.0));
            }
        }
    }
    if let Some(errors) = &mut errors {
        let _ = tokio::io::AsyncReadExt::read_to_string(errors, &mut said).await;
    }
    let status = child
        .wait()
        .await
        .map_err(|error| format!("ffmpeg stopped: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        let last = said
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("");
        Err(format!("ffmpeg failed: {}", last.trim()))
    }
}

/// How long a video or a sound lasts, in seconds, from ffprobe.
async fn duration(input: &std::ffi::OsStr) -> Option<f64> {
    let output = tokio::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "csv=p=0",
        ])
        .arg(input)
        .stdin(Stdio::null())
        .output()
        .await
        .ok()?;
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|length| *length > 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_the_defaults() {
        mochi_core::examples::check_module::<Settings>("drop", include_str!("../settings.toml"));
    }

    #[test]
    fn actions_must_exist() {
        let table = |text: &str| mochi_core::toml::from_str(text).unwrap();
        assert!(Settings::load(&table("actions = [\"zip\", \"merge\"]")).is_ok());
        assert!(Settings::load(&table("actions = [\"shred\"]")).is_err());
        // The old names of the conversions still turn them on.
        let old = Settings::load(&table("actions = [\"zip\", \"png\", \"webp\"]")).unwrap();
        assert_eq!(old.actions, ["zip", "convert"]);
    }

    #[tokio::test]
    async fn plans_run_their_programs() {
        let dir = std::env::temp_dir().join(format!("mochi-drop-run-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let plan = Plan {
            steps: vec![
                Step::MakeDir(dir.join("made")),
                Step::Run {
                    program: "sh".into(),
                    args: vec!["-c".into(), "echo hi > made/out.txt".into()],
                    cwd: dir.clone(),
                },
            ],
            done: "Done".into(),
            result: None,
        };
        execute(&plan, &mpsc::unbounded_channel().0).await.unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("made/out.txt")).unwrap(),
            "hi\n"
        );
        let failing = Plan {
            steps: vec![Step::Run {
                program: "sh".into(),
                args: vec!["-c".into(), "echo broken >&2; exit 1".into()],
                cwd: dir.clone(),
            }],
            ..plan
        };
        assert_eq!(
            execute(&failing, &mpsc::unbounded_channel().0).await,
            Err("sh failed: broken".into())
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
