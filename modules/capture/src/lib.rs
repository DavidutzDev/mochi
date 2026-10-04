//! Capture: screenshots and screen recordings from the island.
//!
//! `mochi ipc capture screenshot` freezes every screen and opens the picker
//! in the default mode, a region unless set otherwise; the island switches
//! to a window or a whole screen. An overlay over the frozen screens picks
//! it, the module cuts it out of the frozen
//! frame, saves it and copies it, and the island shows it with buttons to
//! copy, edit, delete or open its folder. `record` picks the same way over
//! the live screens and records through gpu-screen-recorder; a bubble with a
//! timer shows while it runs, and clicking it, `stop` or `record` again ends
//! it. A window recording goes through the screen-cast portal, whose picker
//! chooses the window.
//!
//! The overlay saves each output's frozen frame to the module's data
//! directory and reports where the output sits in the global layout; picks
//! come back in that same space, in logical pixels.
//!
//! Settings in `config.toml`: see `settings.toml`.

mod crop;
mod files;
mod record;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant, SystemTime};

use include_dir::{Dir, include_dir};
use mochi_core::compositor::Window;
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, Area, ArgSpec, Args, Assets, BoxFuture, BubbleId,
    BubbleSpec, Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent, Priority,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::process::Command;
use tokio::task::JoinHandle;

use crate::crop::Rect;
use crate::record::{Options, Recording, Target};

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

const MODES: [&str; 3] = ["region", "window", "screen"];

#[derive(Debug, Default)]
pub struct Capture;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    screenshots: String,
    recordings: String,
    screenshot_name: String,
    recording_name: String,
    editor: Vec<String>,
    recorder: Vec<String>,
    framerate: u32,
    audio: String,
    microphone: String,
    record_audio: bool,
    record_microphone: bool,
    copy: bool,
    preview_ms: u64,
    mode: Mode,
    codec: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            screenshots: String::new(),
            recordings: String::new(),
            screenshot_name: "Screenshot from %Y-%m-%d %H-%M-%S".into(),
            recording_name: "Recording from %Y-%m-%d %H-%M-%S".into(),
            editor: vec!["satty".into(), "--filename".into()],
            recorder: vec!["gpu-screen-recorder".into()],
            framerate: 60,
            audio: "default_output".into(),
            microphone: "default_input".into(),
            record_audio: true,
            record_microphone: false,
            copy: true,
            preview_ms: 6000,
            mode: Mode::Region,
            codec: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Screenshot,
    Recording,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Screenshot => "screenshot",
            Self::Recording => "recording",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Mode {
    Region,
    Window,
    Screen,
}

impl Mode {
    fn parse(word: &str) -> Option<Self> {
        match word {
            "region" => Some(Self::Region),
            "window" => Some(Self::Window),
            "screen" => Some(Self::Screen),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Region => "region",
            Self::Window => "window",
            Self::Screen => "screen",
        }
    }
}

impl Module for Capture {
    fn id(&self) -> &'static str {
        "capture"
    }

    fn assets(&self) -> Assets {
        Assets::new(&QML, concat!(env!("CARGO_MANIFEST_DIR"), "/qml"))
    }

    fn settings_example(&self) -> &'static str {
        include_str!("../settings.toml")
    }

    fn check_settings(&self, table: &mochi_core::toml::Table) -> Result<(), String> {
        mochi_core::settings::<Settings>(table)
            .map(drop)
            .map_err(|error| error.to_string())
    }

    fn actions(&self) -> Vec<ActionSpec> {
        let mode = || {
            ArgSpec::choice(
                "mode",
                "What to capture; the `mode` setting when left out",
                MODES,
            )
            .optional()
        };
        let area = |spec: ActionSpec| {
            spec.arg(ArgSpec::string("output", "The monitor it's on"))
                .arg(ArgSpec::float("x", "Left edge, in the global layout"))
                .arg(ArgSpec::float("y", "Top edge, in the global layout"))
                .arg(ArgSpec::float("width", "In logical pixels"))
                .arg(ArgSpec::float("height", "In logical pixels"))
        };
        vec![
            ActionSpec::new("screenshot", "Take a screenshot").arg(mode()),
            ActionSpec::new("record", "Start recording, or stop the recording").arg(mode()),
            ActionSpec::new("stop", "Stop recording"),
            ActionSpec::new("cancel", "Close the picker"),
            ActionSpec::new("mode", "Switch what the picker captures").arg(ArgSpec::choice(
                "mode",
                "What to capture",
                MODES,
            )),
            ActionSpec::new("audio", "Turn recording the desktop audio on or off"),
            ActionSpec::new("microphone", "Turn recording the microphone on or off"),
            area(
                ActionSpec::new("frame", "A frozen screen is saved; the overlay sends this")
                    .arg(ArgSpec::int("session", "The picker it belongs to")),
            ),
            area(ActionSpec::new(
                "region",
                "A region is drawn; the overlay sends this",
            )),
            area(ActionSpec::new(
                "select",
                "Capture this area; the overlay sends this",
            )),
            ActionSpec::new("confirm", "Capture the region drawn"),
            ActionSpec::new("copy", "Copy the last capture"),
            ActionSpec::new("edit", "Open the last screenshot in the editor"),
            ActionSpec::new("delete", "Delete the last capture"),
            ActionSpec::new("open", "Open the last capture's folder"),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let settings: Settings = ctx.settings()?;
            // Which video codec works here, asked once in the background:
            // gpu-screen-recorder takes a quarter of a second to answer.
            let mut probe = settings.codec.is_empty().then(|| {
                let program = settings.recorder.first().cloned().unwrap_or_default();
                tokio::spawn(async move { record::probe(&program).await })
            });
            let mut state = State::new(settings, ctx.data_dir().to_owned());
            loop {
                tokio::select! {
                    codec = probed(&mut probe) => {
                        state.codec = codec;
                        probe = None;
                    }
                    event = ctx.next_event() => match event {
                        None => {
                            state.shut_down().await;
                            return Ok(());
                        }
                        Some(ModuleEvent::Command(command)) => state.command(&ctx, command).await,
                        Some(ModuleEvent::Ended { activity, .. }) => state.ended(activity),
                        Some(ModuleEvent::BubbleClicked(bubble)) if state.bubble == Some(bubble) => {
                            state.stop_recording();
                        }
                        Some(_) => {}
                    },
                    result = finished(&mut state.recording) => state.recorded(&ctx, result),
                }
            }
        })
    }
}

/// Waits for the codec probe, or forever once it answered.
async fn probed(probe: &mut Option<JoinHandle<Option<&'static str>>>) -> Option<&'static str> {
    match probe {
        Some(probe) => probe.await.ok().flatten(),
        None => std::future::pending().await,
    }
}

/// Waits for the recording to end, or forever without one.
async fn finished(recording: &mut Option<Recording>) -> Result<(), String> {
    match recording {
        Some(recording) => recording.finished().await,
        None => std::future::pending().await,
    }
}

/// A picker on screen: the overlay over every monitor and a view on the
/// island.
#[derive(Debug)]
struct Session {
    /// Tells this picker's frames from a closed one's.
    number: u64,
    kind: Kind,
    mode: Mode,
    sound: Sound,
    /// The monitor with the keyboard.
    output: Option<String>,
    /// For the window picker. `None` when the compositor doesn't say where
    /// windows are.
    windows: Option<Vec<Window>>,
    /// Each output's place in the global layout, once its frame is saved.
    frames: HashMap<String, Rect>,
    /// A region drawn and not taken yet, with its output.
    region: Option<(String, Rect)>,
    /// What to capture, waiting for its output's frame: an area, or the
    /// whole output.
    picked: Option<(String, Option<Rect>)>,
    /// When it was picked, to log how long the screenshot took.
    picked_at: Option<Instant>,
    activity: ActivityId,
}

impl Session {
    fn payload(&self, frames: &Path) -> Value {
        let stage = if self.picked.is_some() {
            "saving"
        } else {
            "select"
        };
        let rect = |area: &Rect| json!({ "x": area.x, "y": area.y, "width": area.width, "height": area.height });
        // Window recordings go through the portal, so only screenshots need
        // to know where windows are.
        let modes: Vec<&str> = MODES
            .into_iter()
            .filter(|mode| {
                *mode != "window" || self.kind == Kind::Recording || self.windows.is_some()
            })
            .collect();
        json!({
            "session": self.number,
            "stage": stage,
            // The overlay saves only this output's frame, once it's picked.
            "picked": self.picked.as_ref().map(|(output, _)| output),
            "kind": self.kind.as_str(),
            "mode": self.mode.as_str(),
            "modes": modes,
            "audio": self.sound.desktop,
            "microphone": self.sound.microphone,
            "output": self.output,
            "frames": frames.display().to_string(),
            "windows": self.windows.as_ref().map(|windows| windows
                .iter()
                .map(|window| json!({
                    "title": window.title,
                    "app_id": window.app_id,
                    "x": window.x,
                    "y": window.y,
                    "width": window.width,
                    "height": window.height,
                }))
                .collect::<Vec<_>>()),
            "region": self.region.as_ref().map(|(output, area)| {
                let mut region = rect(area);
                region["output"] = json!(output);
                region
            }),
        })
    }
}

/// What a recording hears.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Sound {
    /// The desktop audio, `audio` in the settings.
    desktop: bool,
    microphone: bool,
}

/// A file the island shows after a capture.
#[derive(Debug)]
struct Saved {
    kind: Kind,
    path: PathBuf,
}

#[derive(Debug)]
struct State {
    settings: Settings,
    screenshots: PathBuf,
    recordings: PathBuf,
    /// Where the overlay saves frozen frames.
    frames: PathBuf,
    sessions: u64,
    session: Option<Session>,
    recording: Option<Recording>,
    bubble: Option<BubbleId>,
    last: Option<Saved>,
    preview: Option<ActivityId>,
    /// The video codec the probe picked, when it picked one.
    codec: Option<&'static str>,
}

impl State {
    fn new(settings: Settings, frames: PathBuf) -> Self {
        Self {
            screenshots: files::folder(
                &settings.screenshots,
                "XDG_PICTURES_DIR",
                "Pictures",
                "Screenshots",
            ),
            recordings: files::folder(
                &settings.recordings,
                "XDG_VIDEOS_DIR",
                "Videos",
                "Recordings",
            ),
            settings,
            frames,
            sessions: 0,
            session: None,
            recording: None,
            bubble: None,
            last: None,
            preview: None,
            codec: None,
        }
    }

    async fn command(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        let args = &command.args;
        let mode = || args.str("mode").and_then(Mode::parse);
        let result = match command.action.as_str() {
            "screenshot" => self.open(ctx, Kind::Screenshot, mode()).await,
            "record" if self.recording.is_some() => {
                self.stop_recording();
                Ok(())
            }
            "record" => self.open(ctx, Kind::Recording, mode()).await,
            "stop" => match &self.recording {
                Some(_) => {
                    self.stop_recording();
                    Ok(())
                }
                None => Err("nothing is recording".into()),
            },
            "cancel" => {
                self.close(ctx);
                Ok(())
            }
            "mode" => match mode() {
                Some(mode) => self.choose(ctx, mode).await,
                None => Err("no such mode".into()),
            },
            "audio" => self.toggle_sound(ctx, |sound| sound.desktop = !sound.desktop),
            "microphone" => self.toggle_sound(ctx, |sound| sound.microphone = !sound.microphone),
            "frame" => self.frame(ctx, args).await,
            "region" => self.region(ctx, args),
            "select" => match area(args) {
                Some((output, area)) => self.pick(ctx, output, Some(area)).await,
                None => Err("bad area".into()),
            },
            "confirm" => match self
                .session
                .as_mut()
                .and_then(|session| session.region.take())
            {
                Some((output, area)) => self.pick(ctx, output, Some(area)).await,
                None => Err("draw a region first".into()),
            },
            "copy" => match &self.last {
                Some(saved) => copy(saved).await,
                None => Err("nothing captured yet".into()),
            },
            "edit" => self.edit(),
            "delete" => self.delete(ctx),
            "open" => match &self.last {
                Some(saved) => spawn(&[
                    "xdg-open".into(),
                    folder_of(&saved.path).display().to_string(),
                ]),
                None => Err("nothing captured yet".into()),
            },
            other => Err(format!("capture has no action {other}")),
        };
        if let Err(message) = &result {
            tracing::debug!(action = %command.action, %message, "capture");
        }
        command.reply(result);
    }

    /// Opens the picker in `mode`, or the default mode; the island's bar
    /// switches it. A screenshot of the screen skips the picking.
    async fn open(
        &mut self,
        ctx: &ModuleCtx,
        kind: Kind,
        mode: Option<Mode>,
    ) -> Result<(), String> {
        if self.recording.is_some() {
            return Err("a recording is running; stop it first".into());
        }
        self.close(ctx);
        let compositor = ctx.compositor();
        let windows = match kind {
            Kind::Screenshot if compositor.knows_windows() => compositor.windows().await.ok(),
            _ => None,
        };
        let asked = mode;
        let mut mode = mode.unwrap_or(self.settings.mode);
        if kind == Kind::Screenshot && mode == Mode::Window && windows.is_none() {
            // Without window positions a default of window falls back to a
            // region; asking for it outright is an error.
            if asked.is_some() {
                return Err("this compositor doesn't say where windows are".into());
            }
            mode = Mode::Region;
        }
        if kind == Kind::Recording && mode == Mode::Window {
            return self.start_recording(ctx, Target::Portal, self.default_sound());
        }

        let output = compositor.state().focused_output;
        self.sessions += 1;
        let mut session = Session {
            number: self.sessions,
            kind,
            mode,
            sound: self.default_sound(),
            output: output.clone(),
            windows,
            frames: HashMap::new(),
            region: None,
            picked: None,
            picked_at: None,
            activity: ActivityId(0),
        };
        if kind == Kind::Screenshot && asked == Some(Mode::Screen) {
            let output = output.ok_or("no monitor has focus")?;
            session.picked = Some((output, None));
            session.picked_at = Some(Instant::now());
        }
        session.activity = ctx.present(self.spec(&session));
        self.session = Some(session);
        Ok(())
    }

    fn spec(&self, session: &Session) -> ActivitySpec {
        ActivitySpec::new("Picker")
            .key("picker")
            // Over anything, even the launcher or the hub, which the frozen
            // screen still shows, so they can be captured too.
            .priority(Priority::TOP)
            .uninterruptible()
            .overlay("Overlay")
            .payload(session.payload(&self.frames))
    }

    /// Sends the picker's new state to its views.
    fn refresh(&self, ctx: &ModuleCtx) {
        if let Some(session) = &self.session {
            ctx.update(session.activity, session.payload(&self.frames));
        }
    }

    fn close(&mut self, ctx: &ModuleCtx) {
        if let Some(session) = self.session.take() {
            ctx.withdraw(session.activity);
        }
    }

    fn ended(&mut self, activity: ActivityId) {
        if self
            .session
            .as_ref()
            .is_some_and(|session| session.activity == activity)
        {
            self.session = None;
        }
        if self.preview == Some(activity) {
            self.preview = None;
        }
    }

    async fn choose(&mut self, ctx: &ModuleCtx, mode: Mode) -> Result<(), String> {
        let session = self.session.as_mut().ok_or("the picker isn't open")?;
        if !session.payload(&self.frames)["modes"]
            .as_array()
            .is_some_and(|modes| modes.iter().any(|known| known == mode.as_str()))
        {
            return Err(format!("can't pick a {} here", mode.as_str()));
        }
        if session.kind == Kind::Recording && mode == Mode::Window {
            let sound = session.sound;
            self.close(ctx);
            return self.start_recording(ctx, Target::Portal, sound);
        }
        session.mode = mode;
        session.region = None;
        self.refresh(ctx);
        Ok(())
    }

    fn default_sound(&self) -> Sound {
        Sound {
            desktop: self.settings.record_audio,
            microphone: self.settings.record_microphone,
        }
    }

    fn toggle_sound(
        &mut self,
        ctx: &ModuleCtx,
        toggle: impl FnOnce(&mut Sound),
    ) -> Result<(), String> {
        let session = self.session.as_mut().ok_or("the picker isn't open")?;
        toggle(&mut session.sound);
        self.refresh(ctx);
        Ok(())
    }

    async fn frame(&mut self, ctx: &ModuleCtx, args: &Args) -> Result<(), String> {
        let (output, place) = area(args).ok_or("bad frame")?;
        let number = args.int("session").unwrap_or_default();
        match &mut self.session {
            Some(session) if u64::try_from(number) == Ok(session.number) => {
                session.frames.insert(output, place);
                tracing::debug!(
                    after_ms = session.picked_at.map_or(0, |at| at.elapsed().as_millis()),
                    "the overlay saved the frame"
                );
            }
            // A closed picker's overlay finishing late.
            _ => return Ok(()),
        }
        self.finish(ctx).await;
        Ok(())
    }

    fn region(&mut self, ctx: &ModuleCtx, args: &Args) -> Result<(), String> {
        let region = area(args).ok_or("bad region")?;
        let session = self.session.as_mut().ok_or("the picker isn't open")?;
        session.region = Some(region);
        self.refresh(ctx);
        Ok(())
    }

    async fn pick(
        &mut self,
        ctx: &ModuleCtx,
        output: String,
        area: Option<Rect>,
    ) -> Result<(), String> {
        let session = self.session.as_mut().ok_or("the picker isn't open")?;
        if session.picked.is_some() {
            return Ok(());
        }
        if session.kind == Kind::Recording {
            let sound = session.sound;
            // A whole screen records the output itself, which follows a
            // change of resolution.
            let target = match area {
                Some(area) if session.mode != Mode::Screen => Target::Region(area),
                _ => Target::Output(output),
            };
            self.close(ctx);
            // Lets the shade leave the screen before the first frame.
            tokio::time::sleep(Duration::from_millis(150)).await;
            return self.start_recording(ctx, target, sound);
        }
        session.picked = Some((output, area));
        session.picked_at = Some(Instant::now());
        tracing::debug!(output = %session.picked.as_ref().map_or("", |(output, _)| output.as_str()), "picked");
        session.region = None;
        self.refresh(ctx);
        self.finish(ctx).await;
        Ok(())
    }

    /// Takes the screenshot once its output's frame is saved.
    async fn finish(&mut self, ctx: &ModuleCtx) {
        let Some(session) = &self.session else { return };
        let Some((output, area)) = &session.picked else {
            return;
        };
        let Some(place) = session.frames.get(output).copied() else {
            return;
        };
        let picked_at = session.picked_at;
        let frame = self.frames.join(format!("frame-{output}.ppm"));
        let area = area.unwrap_or(place);
        self.close(ctx);

        let name = files::timestamp(&self.settings.screenshot_name, SystemTime::now());
        let folder = self.screenshots.clone();
        let result = tokio::task::spawn_blocking(move || {
            std::fs::create_dir_all(&folder)
                .map_err(|error| format!("cannot create {}: {error}", folder.display()))?;
            let path = files::unused(&folder, &name, "png");
            crop::crop(&frame, place, area, &path).map(|_| path)
        })
        .await
        .unwrap_or_else(|error| Err(error.to_string()));
        match result {
            Ok(path) => {
                tracing::info!(
                    path = %path.display(),
                    after_ms = picked_at.map_or(0, |at| at.elapsed().as_millis()),
                    "took a screenshot"
                );
                self.saved(
                    ctx,
                    Saved {
                        kind: Kind::Screenshot,
                        path,
                    },
                );
            }
            Err(message) => self.failed(ctx, Kind::Screenshot, &message),
        }
    }

    /// Starts recording, with the bubble. When the recorder can't start, the
    /// island says why: the action may have come from a click on it, whose
    /// error would only reach the log.
    fn start_recording(
        &mut self,
        ctx: &ModuleCtx,
        target: Target,
        sound: Sound,
    ) -> Result<(), String> {
        let recording = match self.spawn_recorder(&target, sound) {
            Ok(recording) => recording,
            Err(message) => {
                self.failed(ctx, Kind::Recording, &message);
                return Err(message);
            }
        };
        let started_ms = recording
            .started
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |since| since.as_millis() as u64);
        self.bubble = Some(
            ctx.show_bubble(
                BubbleSpec::new("Recording")
                    .wide("RecordingWide")
                    .key("recording")
                    .area(Area::CenterRight)
                    .order(-10)
                    .payload(json!({ "started_ms": started_ms })),
            ),
        );
        self.recording = Some(recording);
        Ok(())
    }

    fn spawn_recorder(&self, target: &Target, sound: Sound) -> Result<Recording, String> {
        std::fs::create_dir_all(&self.recordings)
            .map_err(|error| format!("cannot create {}: {error}", self.recordings.display()))?;
        let name = files::timestamp(&self.settings.recording_name, SystemTime::now());
        let file = files::unused(&self.recordings, &name, "mp4");
        let mut audio = Vec::new();
        if sound.desktop {
            audio.push(self.settings.audio.as_str());
        }
        if sound.microphone {
            audio.push(self.settings.microphone.as_str());
        }
        let options = Options {
            recorder: &self.settings.recorder,
            framerate: self.settings.framerate,
            audio,
            codec: if self.settings.codec.is_empty() {
                self.codec
            } else {
                Some(self.settings.codec.as_str())
            },
        };
        let argv = record::command(&options, target, &file);
        tracing::info!(command = %argv.join(" "), "recording");
        Recording::start(&argv, file).map_err(|error| record::explain(&error))
    }

    fn stop_recording(&self) {
        if let Some(recording) = &self.recording {
            recording.stop();
        }
    }

    /// The recorder exited, after `stop` or on its own.
    fn recorded(&mut self, ctx: &ModuleCtx, result: Result<(), String>) {
        let Some(recording) = self.recording.take() else {
            return;
        };
        if let Some(bubble) = self.bubble.take() {
            ctx.hide_bubble(bubble);
        }
        let written = std::fs::metadata(&recording.file).is_ok_and(|file| file.len() > 0);
        match (result, written) {
            (Ok(()), true) | (Err(_), true) => {
                tracing::info!(path = %recording.file.display(), "recorded");
                self.saved(
                    ctx,
                    Saved {
                        kind: Kind::Recording,
                        path: recording.file,
                    },
                );
            }
            (Err(message), false) => {
                self.failed(ctx, Kind::Recording, &record::explain(&message));
            }
            (Ok(()), false) => self.failed(ctx, Kind::Recording, "the recorder wrote nothing"),
        }
    }

    fn saved(&mut self, ctx: &ModuleCtx, saved: Saved) {
        if self.settings.copy {
            let copied = Saved {
                kind: saved.kind,
                path: saved.path.clone(),
            };
            tokio::spawn(async move {
                if let Err(message) = copy(&copied).await {
                    tracing::warn!(%message, "could not copy the capture");
                }
            });
        }
        let payload = json!({
            "kind": saved.kind.as_str(),
            "path": saved.path.display().to_string(),
            "name": saved.path.file_name().map(|name| name.to_string_lossy().into_owned()),
            "folder": folder_of(&saved.path).display().to_string(),
            "copied": self.settings.copy,
            "editable": saved.kind == Kind::Screenshot && !self.settings.editor.is_empty(),
        });
        self.preview = Some(ctx.present(self.preview_spec(payload)));
        self.last = Some(saved);
    }

    fn failed(&mut self, ctx: &ModuleCtx, kind: Kind, message: &str) {
        tracing::warn!(kind = kind.as_str(), %message, "capture failed");
        let payload = json!({ "kind": kind.as_str(), "error": message });
        self.preview = Some(ctx.present(self.preview_spec(payload)));
    }

    fn preview_spec(&self, payload: Value) -> ActivitySpec {
        ActivitySpec::new("Preview")
            .key("preview")
            .priority(Priority::HIGH)
            .timeout(Duration::from_millis(self.settings.preview_ms))
            .payload(payload)
    }

    fn edit(&self) -> Result<(), String> {
        let saved = self.last.as_ref().ok_or("nothing captured yet")?;
        if saved.kind != Kind::Screenshot {
            return Err("only screenshots open in the editor".into());
        }
        let mut argv = self.settings.editor.clone();
        argv.push(saved.path.display().to_string());
        spawn(&argv)
    }

    fn delete(&mut self, ctx: &ModuleCtx) -> Result<(), String> {
        let saved = self.last.take().ok_or("nothing captured yet")?;
        std::fs::remove_file(&saved.path)
            .map_err(|error| format!("cannot delete {}: {error}", saved.path.display()))?;
        if let Some(preview) = self.preview.take() {
            ctx.withdraw(preview);
        }
        Ok(())
    }

    /// Finishes a recording when the daemon stops, so the file stays
    /// playable.
    async fn shut_down(&mut self) {
        if let Some(recording) = &mut self.recording {
            recording.stop();
            let _ = tokio::time::timeout(Duration::from_secs(5), recording.finished()).await;
        }
    }
}

/// `output x y width height`, from the overlay.
fn area(args: &Args) -> Option<(String, Rect)> {
    let output = args.str("output")?.to_owned();
    let area = Rect::new(
        args.float("x")?,
        args.float("y")?,
        args.float("width")?,
        args.float("height")?,
    );
    (area.width > 0.0 && area.height > 0.0).then_some((output, area))
}

fn folder_of(path: &Path) -> &Path {
    path.parent().unwrap_or(path)
}

/// Puts a capture on the clipboard: a screenshot as an image, a recording
/// as a file to paste.
async fn copy(saved: &Saved) -> Result<(), String> {
    let (kind, stdin) = match saved.kind {
        Kind::Screenshot => {
            let file = std::fs::File::open(&saved.path)
                .map_err(|error| format!("cannot read {}: {error}", saved.path.display()))?;
            ("image/png", Stdio::from(file))
        }
        Kind::Recording => ("text/uri-list", Stdio::piped()),
    };
    let mut child = Command::new("wl-copy")
        .args(["--type", kind])
        .stdin(stdin)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("cannot start wl-copy: {error}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        let uri = format!("file://{}\n", saved.path.display());
        stdin
            .write_all(uri.as_bytes())
            .await
            .map_err(|error| error.to_string())?;
    }
    // wl-copy forks to serve the clipboard and returns at once.
    match child.wait().await {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!("wl-copy {status}")),
        Err(error) => Err(error.to_string()),
    }
}

/// Starts a program on its own, in its own scope and adopted by systemd,
/// so stopping Mochi leaves it running.
fn spawn(argv: &[String]) -> Result<(), String> {
    mochi_core::process::spawn_detached(&mochi_core::process::in_app_scope(argv), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(kind: Kind, windows: Option<Vec<Window>>) -> Session {
        Session {
            number: 3,
            kind,
            mode: Mode::Region,
            sound: Sound {
                desktop: true,
                microphone: false,
            },
            output: Some("DP-3".into()),
            windows,
            frames: HashMap::new(),
            region: None,
            picked: None,
            picked_at: None,
            activity: ActivityId(1),
        }
    }

    #[test]
    fn the_payload_follows_the_stages() {
        let frames = Path::new("/run/mochi/capture");
        let mut picker = session(Kind::Screenshot, Some(Vec::new()));
        let payload = picker.payload(frames);
        assert_eq!(payload["stage"], "select");
        assert_eq!(payload["mode"], "region");
        assert_eq!(payload["modes"], json!(["region", "window", "screen"]));
        assert_eq!(payload["frames"], "/run/mochi/capture");

        picker.region = Some(("DP-3".into(), Rect::new(1.0, 2.0, 3.0, 4.0)));
        assert_eq!(
            picker.payload(frames)["region"],
            json!({ "output": "DP-3", "x": 1.0, "y": 2.0, "width": 3.0, "height": 4.0 })
        );

        picker.picked = Some(("DP-3".into(), None));
        let payload = picker.payload(frames);
        assert_eq!(payload["stage"], "saving");
        assert_eq!(payload["picked"], "DP-3");
    }

    #[test]
    fn windows_only_show_where_the_compositor_says_where_they_are() {
        let frames = Path::new("/tmp");
        let screenshot = session(Kind::Screenshot, None).payload(frames);
        assert_eq!(screenshot["modes"], json!(["region", "screen"]));
        assert_eq!(screenshot["windows"], Value::Null);
        // Recordings pick windows through the portal.
        let recording = session(Kind::Recording, None).payload(frames);
        assert_eq!(recording["modes"], json!(["region", "window", "screen"]));
    }

    #[test]
    fn areas_need_a_size() {
        let spec = ActionSpec::new("select", "")
            .arg(ArgSpec::string("output", ""))
            .arg(ArgSpec::float("x", ""))
            .arg(ArgSpec::float("y", ""))
            .arg(ArgSpec::float("width", ""))
            .arg(ArgSpec::float("height", ""));
        let words = |text: &str| text.split(' ').map(str::to_owned).collect::<Vec<_>>();
        let args = mochi_core::actions::parse(&spec, &words("DP-3 -20 5.5 100 50")).unwrap();
        assert_eq!(
            area(&args),
            Some(("DP-3".into(), Rect::new(-20.0, 5.5, 100.0, 50.0)))
        );
        let args = mochi_core::actions::parse(&spec, &words("DP-3 0 0 0 50")).unwrap();
        assert_eq!(area(&args), None);
    }
}

#[cfg(test)]
mod settings_example {
    #[test]
    fn shows_the_defaults() {
        mochi_core::examples::check_module::<super::Settings>(
            "capture",
            include_str!("../settings.toml"),
        );
    }
}
