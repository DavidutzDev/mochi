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
mod history;
mod record;
mod thumbs;
mod tour;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant, SystemTime};

use include_dir::{Dir, include_dir};
use mochi_core::compositor::Window;
use mochi_core::quality::{self, Resolution};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, Area, ArgSpec, Args, Assets, BoxFuture, BubbleId,
    BubbleSpec, ContributionSpec, Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent,
    Priority,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::crop::Rect;
use crate::record::{Options, Recording, Target};

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

const MODES: [&str; 4] = ["region", "window", "screen", "all"];
const RESOLUTIONS: [&str; 5] = ["native", "480p", "720p", "1080p", "1440p"];
/// How long the hub takes to close, before a capture started from its page.
const HUB_CLOSES: Duration = Duration::from_millis(350);
/// The highest frame rate a recording may ask for.
const MAX_FRAMERATE: i64 = 240;

#[derive(Debug, Default)]
pub struct Capture;

#[derive(Debug, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    screenshots: String,
    recordings: String,
    screenshot_name: String,
    recording_name: String,
    #[schemars(extend("x-suggest" = [["satty", "--filename"], ["swappy", "-f"], ["gimp"], ["krita"]]))]
    editor: Vec<String>,
    #[schemars(extend("x-suggest" = [["gpu-screen-recorder"]]))]
    recorder: Vec<String>,
    framerate: u32,
    resolution: Resolution,
    /// The file a recording goes in: mp4, mkv or webm.
    container: record::Container,
    #[schemars(extend("x-source" = "audio-device"))]
    audio: String,
    #[schemars(extend("x-source" = "audio-device"))]
    microphone: String,
    record_audio: bool,
    record_microphone: bool,
    copy: bool,
    preview_ms: u64,
    mode: Mode,
    #[schemars(extend("enum" = ["", "h264", "hevc", "av1", "vp8", "vp9", "hevc_hdr", "av1_hdr", "hevc_10bit", "av1_10bit", "h264_vulkan", "hevc_vulkan", "av1_vulkan", "h264_software"]))]
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
            resolution: Resolution::Native,
            container: record::Container::Mp4,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
enum Mode {
    /// A rectangle you drag out.
    Region,
    /// The window you click.
    Window,
    /// The screen you click.
    Screen,
    /// Every screen as one image. Screenshots only.
    All,
}

impl Mode {
    fn parse(word: &str) -> Option<Self> {
        match word {
            "region" => Some(Self::Region),
            "window" => Some(Self::Window),
            "screen" => Some(Self::Screen),
            "all" => Some(Self::All),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Region => "region",
            Self::Window => "window",
            Self::Screen => "screen",
            Self::All => "all",
        }
    }
}

/// What a screenshot takes, once the overlay has said where the screens are.
#[derive(Debug, Clone, PartialEq)]
enum Pick {
    /// One whole screen.
    Output(String),
    /// An area in the global layout, on one screen or across several.
    Area(Rect),
    /// Every screen.
    All,
}

impl Module for Capture {
    fn id(&self) -> &'static str {
        "capture"
    }

    fn assets(&self) -> Assets {
        Assets::new(&QML, concat!(env!("CARGO_MANIFEST_DIR"), "/qml"))
    }

    fn contributions(&self) -> Vec<ContributionSpec> {
        let mut offers = vec![
            ContributionSpec::new("hub", "page", "history", "Page", "Captures")
                .icon("camera")
                .order(35),
        ];
        offers.extend(tour::steps());
        offers
    }

    fn settings_example(&self) -> &'static str {
        include_str!("../settings.toml")
    }

    fn needs(&self, table: &mochi_core::toml::Table) -> Vec<mochi_core::Need> {
        let settings: Settings = mochi_core::settings(table).unwrap_or_default();
        let mut needs = Vec::new();
        if let Some(recorder) = settings.recorder.first() {
            needs.push(mochi_core::Need::new(recorder, "Recording the screen"));
        }
        if let Some(editor) = settings.editor.first() {
            needs.push(mochi_core::Need::new(editor, "Edit on a screenshot"));
        }
        if settings.copy {
            needs.push(mochi_core::Need::new(
                "wl-copy",
                "Copying captures to the clipboard",
            ));
        }
        needs.push(mochi_core::Need::new("xdg-open", "Open on a capture"));
        needs.push(mochi_core::Need::new("ffmpeg", "Thumbnails of recordings"));
        needs
    }

    fn settings_schema(&self) -> Option<Value> {
        Some(mochi_core::options::schema_of::<Settings>())
    }

    fn check_settings(&self, table: &mochi_core::toml::Table) -> Result<(), String> {
        mochi_core::settings::<Settings>(table)
            .map(drop)
            .map_err(|error| error.to_string())
    }

    fn actions(&self) -> Vec<ActionSpec> {
        let file = || {
            ArgSpec::string(
                "path",
                "A file from the history; the last capture when left out",
            )
            .optional()
            .rest()
        };
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
            ActionSpec::new(
                "framerate",
                "Set the recording's frame rate, or step to the next preset",
            )
            .arg(
                ArgSpec::int(
                    "fps",
                    "Frames per second; the next of 15, 30, 60, 90 and 120 when left out",
                )
                .optional(),
            ),
            ActionSpec::new(
                "resolution",
                "Set the recording's resolution, or step to the next preset",
            )
            .arg(
                ArgSpec::choice(
                    "resolution",
                    "The most lines; the next preset when left out",
                    RESOLUTIONS,
                )
                .optional(),
            ),
            ActionSpec::new(
                "codec",
                "Set the recording's video codec, or step to the next the recorder has",
            )
            .arg(
                ArgSpec::string(
                    "codec",
                    "Like h264, hevc, av1 or auto; the next when left out",
                )
                .optional(),
            ),
            ActionSpec::new(
                "container",
                "Set the recording's file format, or step to the next",
            )
            .arg(
                ArgSpec::choice(
                    "container",
                    "mp4, mkv or webm; the next when left out",
                    ["mp4", "mkv", "webm"],
                )
                .optional(),
            ),
            ActionSpec::new("layout", "Where the screens are; the overlay sends this")
                .arg(ArgSpec::int("session", "The picker it belongs to"))
                .arg(ArgSpec::string("screens", "Each screen as name x y width height").rest()),
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
            ActionSpec::new("copy", "Copy the last capture, or one from the history").arg(file()),
            ActionSpec::new(
                "edit",
                "Open the last screenshot, or one from the history, in the editor",
            )
            .arg(file()),
            ActionSpec::new("delete", "Delete the last capture, or one from the history")
                .arg(file()),
            ActionSpec::new(
                "open",
                "Open the folder of the last capture, or of one from the history",
            )
            .arg(file()),
            ActionSpec::new(
                "preview",
                "Show a capture from the history in the preview card",
            )
            .arg(ArgSpec::string("path", "The file, as the history lists it").rest()),
            ActionSpec::new("history", "Look for new captures; the hub page sends this"),
            ActionSpec::new(
                "start",
                "Close the hub, then open the picker; the hub page sends this",
            )
            .arg(ArgSpec::choice(
                "kind",
                "What to make",
                ["screenshot", "record"],
            )),
            ActionSpec::new(
                "show",
                "Show an image in the preview card; the clipboard sends this for its images",
            )
            .arg(ArgSpec::string("path", "The image file"))
            .arg(ArgSpec::int("entry", "The clipboard entry it came from"))
            .arg(
                ArgSpec::string("label", "What the card says under the title")
                    .optional()
                    .rest(),
            ),
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
            let (thumbs, mut thumbnails) = mpsc::unbounded_channel();
            let mut state = State::new(settings, ctx.data_dir().to_owned(), thumbs);
            state.publish_history(&ctx);
            loop {
                tokio::select! {
                    found = probed(&mut probe) => {
                        state.codec = found.pick;
                        state.codecs = found.codecs;
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
                    Some(thumb) = thumbnails.recv() => state.thumbnailed(&ctx, thumb),
                }
            }
        })
    }
}

/// Waits for the codec probe, or forever once it answered.
async fn probed(probe: &mut Option<JoinHandle<record::Probe>>) -> record::Probe {
    match probe {
        Some(probe) => probe.await.unwrap_or_default(),
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
    setup: Setup,
    /// The monitor with the keyboard.
    output: Option<String>,
    /// For the window picker. `None` when the compositor doesn't say where
    /// windows are.
    windows: Option<Vec<Window>>,
    /// Every screen's place in the global layout, as the overlay reports
    /// it once it opens.
    layout: Vec<(String, Rect)>,
    /// Each output's place, once its frame is saved.
    frames: HashMap<String, Rect>,
    /// A region drawn and not taken yet, in the global layout. It may cross
    /// from one screen into the next.
    region: Option<Rect>,
    /// What to capture, waiting for the frames of the screens it touches.
    picked: Option<Pick>,
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
        // Every screen at once is for screenshots, with more than one screen.
        let modes: Vec<&str> = MODES
            .into_iter()
            .filter(|mode| match *mode {
                "window" => self.kind == Kind::Recording || self.windows.is_some(),
                "all" => self.kind == Kind::Screenshot && self.layout.len() != 1,
                _ => true,
            })
            .collect();
        json!({
            "session": self.number,
            "stage": stage,
            // The overlays of these screens save their frames, once picked.
            "picked": self.targets().map(|(outputs, _)| outputs),
            "kind": self.kind.as_str(),
            "mode": self.mode.as_str(),
            "modes": modes,
            "audio": self.setup.desktop,
            "microphone": self.setup.microphone,
            "framerate": self.setup.framerate,
            "resolution": self.setup.resolution.as_str(),
            "codec": record::codec_label(&self.setup.codec),
            "container": self.setup.container.as_str(),
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
            "region": self.region.as_ref().map(rect),
        })
    }

    /// The screens a pick touches and the area it takes, once the layout is
    /// known: `None` before that, or when the area misses every screen.
    fn targets(&self) -> Option<(Vec<String>, Rect)> {
        let area = match self.picked.as_ref()? {
            Pick::Output(output) => {
                let (_, place) = self.layout.iter().find(|(name, _)| name == output)?;
                *place
            }
            Pick::Area(area) => *area,
            Pick::All => self
                .layout
                .iter()
                .map(|(_, place)| *place)
                .reduce(|union, place| union.union(place))?,
        };
        let outputs: Vec<String> = self
            .layout
            .iter()
            .filter(|(_, place)| place.intersects(&area))
            .map(|(name, _)| name.clone())
            .collect();
        (!outputs.is_empty()).then_some((outputs, area))
    }
}

/// What a recording hears, and its quality.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Setup {
    /// The desktop audio, `audio` in the settings.
    desktop: bool,
    microphone: bool,
    framerate: u32,
    resolution: Resolution,
    /// The video codec; empty for the recorder's pick.
    codec: String,
    container: record::Container,
}

/// A file the island shows after a capture.
#[derive(Debug)]
struct Saved {
    kind: Kind,
    path: PathBuf,
    /// The clipboard entry it shows, for an image from the clipboard: its
    /// file is the clipboard's, and deleting removes the entry instead.
    clipboard: Option<i64>,
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
    /// The newest captures in the folders, for the hub page.
    history: Vec<history::Entry>,
    preview: Option<ActivityId>,
    /// The video codec the probe picked, when it picked one.
    codec: Option<&'static str>,
    /// Every codec the recorder lists here, for the picker to step through.
    codecs: Vec<String>,
    /// What the card shows, to add a recording's thumbnail once it's made.
    preview_payload: Value,
    /// Where thumbnails go; `None` without a home.
    thumbnails: Option<PathBuf>,
    /// Thumbnails are being made.
    thumbnailing: bool,
    /// Videos ffmpeg made no thumbnail of, not to try again.
    no_thumbnail: HashSet<PathBuf>,
    thumbs: mpsc::UnboundedSender<Thumb>,
}

/// News from the thumbnail maker.
#[derive(Debug)]
enum Thumb {
    Failed(PathBuf),
    Done,
}

impl State {
    fn new(settings: Settings, frames: PathBuf, thumbs: mpsc::UnboundedSender<Thumb>) -> Self {
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
            history: Vec::new(),
            preview: None,
            codec: None,
            codecs: Vec::new(),
            preview_payload: Value::Null,
            thumbnails: thumbs::folder(),
            thumbnailing: false,
            no_thumbnail: HashSet::new(),
            thumbs,
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
            "audio" => self.change_setup(ctx, |setup| setup.desktop = !setup.desktop),
            "microphone" => self.change_setup(ctx, |setup| setup.microphone = !setup.microphone),
            "framerate" => match args.int("fps") {
                Some(fps) if !(1..=MAX_FRAMERATE).contains(&fps) => Err(format!(
                    "{fps} fps is out of range; it goes from 1 to {MAX_FRAMERATE}"
                )),
                fps => self.change_setup(ctx, |setup| {
                    setup.framerate = fps.map_or_else(
                        || quality::next_framerate(setup.framerate),
                        |fps| fps as u32,
                    );
                }),
            },
            "resolution" => {
                let asked = args.str("resolution").and_then(Resolution::parse);
                self.change_setup(ctx, |setup| {
                    setup.resolution = asked.unwrap_or_else(|| setup.resolution.next());
                })
            }
            "codec" => {
                let container = self
                    .session
                    .as_ref()
                    .map_or(self.settings.container, |session| session.setup.container);
                let choices = self.codec_choices(container);
                let asked = args.str("codec").map(str::to_owned);
                match asked {
                    Some(codec) if codec != "auto" && !self.codecs.contains(&codec) => {
                        Err(format!("{codec} isn't one the recorder lists here"))
                    }
                    asked => self.change_setup(ctx, |setup| {
                        setup.codec = match asked {
                            Some(codec) if codec == "auto" => String::new(),
                            Some(codec) => codec,
                            None => {
                                let at = choices.iter().position(|codec| *codec == setup.codec);
                                choices
                                    .get(at.map_or(0, |at| (at + 1) % choices.len().max(1)))
                                    .cloned()
                                    .unwrap_or_default()
                            }
                        };
                    }),
                }
            }
            "container" => {
                let asked = args.str("container").and_then(record::Container::parse);
                let choices: Vec<String> = self.codecs.clone();
                // A container no codec here fits is skipped, once the
                // recorder said which it has.
                let fits = |container: record::Container| {
                    choices.is_empty() || choices.iter().any(|codec| container.takes(codec))
                };
                self.change_setup(ctx, |setup| {
                    setup.container = asked.unwrap_or_else(|| {
                        let mut next = setup.container.next();
                        while !fits(next) && next != setup.container {
                            next = next.next();
                        }
                        next
                    });
                    // WebM takes only some codecs: the first that fits.
                    if !setup.container.takes(&setup.codec) {
                        setup.codec = choices
                            .iter()
                            .find(|codec| setup.container.takes(codec))
                            .cloned()
                            .unwrap_or_default();
                    }
                })
            }
            "layout" => self.layout(ctx, args).await,
            "frame" => self.frame(ctx, args).await,
            "region" => self.region(ctx, args),
            "select" => match area(args) {
                Some((output, area)) => self.select(ctx, output, area).await,
                None => Err("bad area".into()),
            },
            "confirm" => match self.session.as_ref().and_then(|session| session.region) {
                Some(area) => self.select(ctx, String::new(), area).await,
                None => Err("draw a region first".into()),
            },
            "copy" => match self.target(args) {
                Ok(saved) => copy(&saved).await,
                Err(error) => Err(error),
            },
            "edit" => self.target(args).and_then(|saved| self.edit(&saved)),
            "delete" => self.delete(ctx, args),
            "open" => self.target(args).and_then(|saved| {
                if saved.clipboard.is_some() {
                    return Err("a clipboard image has no folder".into());
                }
                spawn(&[
                    "xdg-open".into(),
                    folder_of(&saved.path).display().to_string(),
                ])
            }),
            "preview" => self.preview_file(ctx, args),
            "history" => {
                self.publish_history(ctx);
                Ok(())
            }
            "start" => {
                let _ = ctx.call("hub", "close", &[]).await;
                // Long enough for the hub to shrink away before the screen
                // freezes.
                tokio::time::sleep(HUB_CLOSES).await;
                let kind = match args.str("kind") {
                    Some("record") => Kind::Recording,
                    _ => Kind::Screenshot,
                };
                self.open(ctx, kind, None).await
            }
            "show" => self.show(ctx, &command.args),
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
            return self.start_recording(ctx, Target::Portal, self.default_setup());
        }

        let output = compositor.state().focused_output;
        self.sessions += 1;
        let mut session = Session {
            number: self.sessions,
            kind,
            mode,
            setup: self.default_setup(),
            output: output.clone(),
            windows,
            layout: Vec::new(),
            frames: HashMap::new(),
            region: None,
            picked: None,
            picked_at: None,
            activity: ActivityId(0),
        };
        if kind == Kind::Recording && mode == Mode::All {
            // Recordings take one screen at a time; a default of every
            // screen falls back to a region.
            if asked.is_some() {
                return Err("recordings take one screen at a time".into());
            }
            session.mode = Mode::Region;
        }
        // Asked for outright, these skip the picking.
        if kind == Kind::Screenshot {
            match asked {
                Some(Mode::Screen) => {
                    let output = output.ok_or("no monitor has focus")?;
                    session.picked = Some(Pick::Output(output));
                }
                Some(Mode::All) => session.picked = Some(Pick::All),
                _ => {}
            }
            if session.picked.is_some() {
                session.picked_at = Some(Instant::now());
            }
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
            let setup = session.setup.clone();
            self.close(ctx);
            return self.start_recording(ctx, Target::Portal, setup);
        }
        session.mode = mode;
        session.region = None;
        self.refresh(ctx);
        Ok(())
    }

    fn default_setup(&self) -> Setup {
        Setup {
            desktop: self.settings.record_audio,
            microphone: self.settings.record_microphone,
            framerate: self.settings.framerate,
            resolution: self.settings.resolution,
            // WebM takes only some codecs, so it names one.
            codec: if self.settings.container.takes(&self.settings.codec) {
                self.settings.codec.clone()
            } else {
                self.codec_choices(self.settings.container)
                    .into_iter()
                    .find(|codec| !codec.is_empty())
                    .unwrap_or_default()
            },
            container: self.settings.container,
        }
    }

    /// The codecs to step through: the recorder's pick, then each it lists
    /// that goes in the container.
    fn codec_choices(&self, container: record::Container) -> Vec<String> {
        std::iter::once(String::new())
            .chain(self.codecs.iter().cloned())
            .filter(|codec| container.takes(codec))
            .collect()
    }

    fn change_setup(
        &mut self,
        ctx: &ModuleCtx,
        toggle: impl FnOnce(&mut Setup),
    ) -> Result<(), String> {
        let session = self.session.as_mut().ok_or("the picker isn't open")?;
        toggle(&mut session.setup);
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
        let (_, region) = area(args).ok_or("bad region")?;
        let session = self.session.as_mut().ok_or("the picker isn't open")?;
        session.region = Some(region);
        self.refresh(ctx);
        Ok(())
    }

    /// Where the screens are, from the overlay: `name x y width height` for
    /// each.
    async fn layout(&mut self, ctx: &ModuleCtx, args: &Args) -> Result<(), String> {
        let number = args.int("session").unwrap_or_default();
        let layout = parse_layout(args.str("screens").unwrap_or_default()).ok_or("bad layout")?;
        match &mut self.session {
            Some(session) if u64::try_from(number) == Ok(session.number) => {
                tracing::debug!(screens = layout.len(), "the overlay sent the layout");
                session.layout = layout;
            }
            _ => return Ok(()),
        }
        self.refresh(ctx);
        self.finish(ctx).await;
        Ok(())
    }

    /// The overlay picked `area` on `output`: a region, a window or a whole
    /// screen, depending on the mode.
    async fn select(&mut self, ctx: &ModuleCtx, output: String, area: Rect) -> Result<(), String> {
        let session = self.session.as_mut().ok_or("the picker isn't open")?;
        if session.picked.is_some() {
            return Ok(());
        }
        if session.kind == Kind::Recording {
            let setup = session.setup.clone();
            // A whole screen records the output itself, which follows a
            // change of resolution.
            let target = match session.mode {
                Mode::Screen => Target::Output(output),
                _ => Target::Region(area),
            };
            self.close(ctx);
            // Lets the shade leave the screen before the first frame.
            tokio::time::sleep(Duration::from_millis(150)).await;
            return self.start_recording(ctx, target, setup);
        }
        let pick = match session.mode {
            Mode::Screen => Pick::Output(output),
            Mode::All => Pick::All,
            Mode::Region | Mode::Window => Pick::Area(area),
        };
        tracing::debug!(?pick, "picked");
        session.picked = Some(pick);
        session.picked_at = Some(Instant::now());
        session.region = None;
        self.refresh(ctx);
        self.finish(ctx).await;
        Ok(())
    }

    /// Takes the screenshot once every screen it touches has saved its
    /// frame.
    async fn finish(&mut self, ctx: &ModuleCtx) {
        let Some(session) = &self.session else { return };
        let Some((outputs, area)) = session.targets() else {
            return;
        };
        let Some(frames) = outputs
            .iter()
            .map(|output| {
                let place = session.frames.get(output).copied()?;
                Some((self.frames.join(format!("frame-{output}.ppm")), place))
            })
            .collect::<Option<Vec<_>>>()
        else {
            return;
        };
        let picked_at = session.picked_at;
        self.close(ctx);

        let name = files::timestamp(&self.settings.screenshot_name, SystemTime::now());
        let folder = self.screenshots.clone();
        let result = tokio::task::spawn_blocking(move || {
            std::fs::create_dir_all(&folder)
                .map_err(|error| format!("cannot create {}: {error}", folder.display()))?;
            let path = files::unused(&folder, &name, "png");
            match frames.as_slice() {
                [(frame, place)] => crop::crop(frame, *place, area, &path).map(|_| path),
                _ => crop::join(&frames, area, &path).map(|_| path),
            }
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
                        clipboard: None,
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
        setup: Setup,
    ) -> Result<(), String> {
        // The size of what's recorded, to scale it down, never up.
        let size = match &target {
            Target::Region(area) => Some((area.width.round() as u32, area.height.round() as u32)),
            Target::Output(name) => ctx
                .compositor()
                .state()
                .outputs
                .iter()
                .find(|output| output.name == *name && output.height > 0)
                .map(|output| (output.width, output.height)),
            Target::Portal => None,
        };
        let recording = match self.spawn_recorder(&target, setup, size) {
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

    fn spawn_recorder(
        &self,
        target: &Target,
        setup: Setup,
        size: Option<(u32, u32)>,
    ) -> Result<Recording, String> {
        std::fs::create_dir_all(&self.recordings)
            .map_err(|error| format!("cannot create {}: {error}", self.recordings.display()))?;
        let name = files::timestamp(&self.settings.recording_name, SystemTime::now());
        let file = files::unused(&self.recordings, &name, setup.container.as_str());
        let mut audio = Vec::new();
        if setup.desktop {
            audio.push(self.settings.audio.as_str());
        }
        if setup.microphone {
            audio.push(self.settings.microphone.as_str());
        }
        let options = Options {
            recorder: &self.settings.recorder,
            framerate: setup.framerate,
            limit: record::limit(setup.resolution, size),
            audio,
            codec: if setup.codec.is_empty() {
                self.codec
            } else {
                Some(setup.codec.as_str())
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
                        clipboard: None,
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
                clipboard: None,
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
            "thumbnail": self.thumbnail_of(&saved.path).filter(|_| saved.kind == Kind::Recording),
        });
        self.present_preview(ctx, payload);
        self.last = Some(saved);
        self.publish_history(ctx);
    }

    /// Looks at the folders again and tells the hub page.
    fn publish_history(&mut self, ctx: &ModuleCtx) {
        self.history = history::scan(&[&self.screenshots, &self.recordings], history::SHOWN);
        let entries: Vec<Value> = self
            .history
            .iter()
            .map(|entry| {
                let mut value = entry.to_json();
                if !entry.screenshot {
                    value["thumbnail"] = json!(self.thumbnail_of(&entry.path));
                }
                value
            })
            .collect();
        self.make_thumbnails();
        ctx.publish_state(json!({
            "captures": entries,
            "editable": !self.settings.editor.is_empty(),
        }));
    }

    /// What a file action works on: the file given, which must be in the
    /// history, or the last capture.
    fn target(&self, args: &Args) -> Result<Saved, String> {
        match args.str("path") {
            Some(path) => {
                let entry = self
                    .history
                    .iter()
                    .find(|entry| entry.path == Path::new(path))
                    .ok_or_else(|| format!("{path} isn't in the captures history"))?;
                Ok(Saved {
                    kind: if entry.screenshot {
                        Kind::Screenshot
                    } else {
                        Kind::Recording
                    },
                    path: entry.path.clone(),
                    clipboard: None,
                })
            }
            None => {
                let saved = self.last.as_ref().ok_or("nothing captured yet")?;
                Ok(Saved {
                    kind: saved.kind,
                    path: saved.path.clone(),
                    clipboard: saved.clipboard,
                })
            }
        }
    }

    /// Shows a capture from the history in the preview card, where its
    /// buttons work on it.
    fn preview_file(&mut self, ctx: &ModuleCtx, args: &Args) -> Result<(), String> {
        if args.str("path").is_none() {
            return Err("which capture?".into());
        }
        let saved = self.target(args)?;
        let payload = json!({
            "kind": saved.kind.as_str(),
            "title": match saved.kind {
                Kind::Screenshot => "Screenshot",
                Kind::Recording => "Recording",
            },
            "path": saved.path.display().to_string(),
            "name": saved.path.file_name().map(|name| name.to_string_lossy().into_owned()),
            "folder": folder_of(&saved.path).display().to_string(),
            "copied": false,
            "editable": saved.kind == Kind::Screenshot && !self.settings.editor.is_empty(),
            "thumbnail": self.thumbnail_of(&saved.path).filter(|_| saved.kind == Kind::Recording),
        });
        // Opened from the hub's page: the hub holds the keyboard.
        let close = ctx.call("hub", "close", &[]);
        tokio::spawn(async move {
            match close.await {
                Ok(()) | Err(mochi_core::CallError::NotEnabled(_)) => {}
                Err(error) => tracing::warn!(%error, "could not close the hub"),
            }
        });
        self.present_preview(ctx, payload);
        self.last = Some(saved);
        Ok(())
    }

    /// Shows an image from the clipboard in the card a screenshot gets.
    fn show(&mut self, ctx: &ModuleCtx, args: &Args) -> Result<(), String> {
        let path = PathBuf::from(args.str("path").ok_or("no image")?);
        if !path.is_file() {
            return Err(format!("no image at {}", path.display()));
        }
        let entry = args.int("entry").ok_or("no clipboard entry")?;
        let payload = json!({
            "kind": Kind::Screenshot.as_str(),
            "title": "From the clipboard",
            "path": path.display().to_string(),
            "name": args.str("label").unwrap_or_default(),
            "copied": false,
            "editable": !self.settings.editor.is_empty(),
            "folder": null,
        });
        self.present_preview(ctx, payload);
        self.last = Some(Saved {
            kind: Kind::Screenshot,
            path,
            clipboard: Some(entry),
        });
        Ok(())
    }

    fn failed(&mut self, ctx: &ModuleCtx, kind: Kind, message: &str) {
        tracing::warn!(kind = kind.as_str(), %message, "capture failed");
        let payload = json!({ "kind": kind.as_str(), "error": message });
        self.present_preview(ctx, payload);
    }

    /// Shows the card, and keeps what it shows to add a thumbnail later.
    fn present_preview(&mut self, ctx: &ModuleCtx, payload: Value) {
        self.preview = Some(ctx.present(self.preview_spec(payload.clone())));
        self.preview_payload = payload;
    }

    /// The thumbnail of a recording, once it's made.
    fn thumbnail_of(&self, path: &Path) -> Option<String> {
        let folder = self.thumbnails.as_ref()?;
        let modified = std::fs::metadata(path)
            .and_then(|file| file.modified())
            .ok()?;
        let thumbnail = thumbs::path(folder, path, modified);
        thumbnail.is_file().then(|| thumbnail.display().to_string())
    }

    /// Makes the recordings' missing thumbnails, one after the other in
    /// the background; each comes back as a [`Thumb`].
    fn make_thumbnails(&mut self) {
        if self.thumbnailing || !mochi_core::process::installed("ffmpeg") {
            return;
        }
        let Some(folder) = self.thumbnails.clone() else {
            return;
        };
        let missing: Vec<(PathBuf, PathBuf)> = self
            .history
            .iter()
            .filter(|entry| !entry.screenshot && !self.no_thumbnail.contains(&entry.path))
            .map(|entry| {
                (
                    entry.path.clone(),
                    thumbs::path(&folder, &entry.path, entry.modified),
                )
            })
            .filter(|(_, thumbnail)| !thumbnail.is_file())
            .collect();
        if missing.is_empty() {
            return;
        }
        self.thumbnailing = true;
        let made = self.thumbs.clone();
        tokio::spawn(async move {
            for (video, thumbnail) in missing {
                if let Err(error) = thumbs::make(&video, &thumbnail).await {
                    tracing::debug!(video = %video.display(), %error, "no thumbnail");
                    let _ = made.send(Thumb::Failed(video));
                }
            }
            let _ = made.send(Thumb::Done);
        });
    }

    /// A batch of thumbnails is made: the page and the card get them.
    fn thumbnailed(&mut self, ctx: &ModuleCtx, thumb: Thumb) {
        match thumb {
            Thumb::Failed(video) => {
                self.no_thumbnail.insert(video);
            }
            Thumb::Done => {
                self.thumbnailing = false;
                self.publish_history(ctx);
                if let (Some(preview), Some(path)) =
                    (self.preview, self.preview_payload["path"].as_str())
                    && self.preview_payload["kind"] == "recording"
                    && self.preview_payload["thumbnail"].is_null()
                    && let Some(thumbnail) = self.thumbnail_of(Path::new(path))
                {
                    self.preview_payload["thumbnail"] = json!(thumbnail);
                    ctx.update(preview, self.preview_payload.clone());
                }
            }
        }
    }

    fn preview_spec(&self, payload: Value) -> ActivitySpec {
        ActivitySpec::new("Preview")
            .key("preview")
            .priority(Priority::HIGH)
            .timeout(Duration::from_millis(self.settings.preview_ms))
            .payload(payload)
    }

    fn edit(&self, saved: &Saved) -> Result<(), String> {
        if saved.kind != Kind::Screenshot {
            return Err("only screenshots open in the editor".into());
        }
        let mut argv = self.settings.editor.clone();
        argv.push(saved.path.display().to_string());
        spawn(&argv)
    }

    fn delete(&mut self, ctx: &ModuleCtx, args: &Args) -> Result<(), String> {
        let saved = self.target(args)?;
        // The card shows the last capture: it goes with it.
        let shown = self
            .last
            .as_ref()
            .is_some_and(|last| last.path == saved.path);
        if shown {
            self.last = None;
        }
        if let Some(entry) = saved.clipboard {
            let id = entry.to_string();
            let remove = ctx.call("clipboard", "delete", &[&id]);
            tokio::spawn(async move {
                if let Err(error) = remove.await {
                    tracing::warn!(%error, "could not remove the clipboard entry");
                }
            });
            if let Some(preview) = self.preview.take() {
                ctx.withdraw(preview);
            }
            return Ok(());
        }
        std::fs::remove_file(&saved.path)
            .map_err(|error| format!("cannot delete {}: {error}", saved.path.display()))?;
        if shown && let Some(preview) = self.preview.take() {
            ctx.withdraw(preview);
        }
        self.publish_history(ctx);
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

/// `name x y width height` for each screen, one after another.
fn parse_layout(text: &str) -> Option<Vec<(String, Rect)>> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() || !words.len().is_multiple_of(5) {
        return None;
    }
    words
        .chunks(5)
        .map(|screen| {
            let number = |index: usize| screen[index].parse::<f64>().ok();
            let place = Rect::new(number(1)?, number(2)?, number(3)?, number(4)?);
            (place.width > 0.0 && place.height > 0.0).then(|| (screen[0].to_owned(), place))
        })
        .collect()
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
            setup: Setup {
                desktop: true,
                microphone: false,
                framerate: 60,
                resolution: Resolution::Native,
                codec: String::new(),
                container: record::Container::Mp4,
            },
            output: Some("DP-3".into()),
            windows,
            layout: Vec::new(),
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
        assert_eq!(
            payload["modes"],
            json!(["region", "window", "screen", "all"])
        );
        assert_eq!(payload["frames"], "/run/mochi/capture");

        picker.region = Some(Rect::new(1.0, 2.0, 3.0, 4.0));
        assert_eq!(
            picker.payload(frames)["region"],
            json!({ "x": 1.0, "y": 2.0, "width": 3.0, "height": 4.0 })
        );

        picker.layout = vec![("DP-3".into(), Rect::new(0.0, 0.0, 1920.0, 1080.0))];
        picker.picked = Some(Pick::Output("DP-3".into()));
        let payload = picker.payload(frames);
        assert_eq!(payload["stage"], "saving");
        assert_eq!(payload["picked"], json!(["DP-3"]));
        // One screen: nothing to join.
        assert_eq!(payload["modes"], json!(["region", "window", "screen"]));
    }

    #[test]
    fn windows_only_show_where_the_compositor_says_where_they_are() {
        let frames = Path::new("/tmp");
        let screenshot = session(Kind::Screenshot, None).payload(frames);
        assert_eq!(screenshot["modes"], json!(["region", "screen", "all"]));
        assert_eq!(screenshot["windows"], Value::Null);
        // Recordings pick windows through the portal.
        let recording = session(Kind::Recording, None).payload(frames);
        assert_eq!(recording["modes"], json!(["region", "window", "screen"]));
    }

    #[test]
    fn picks_take_the_screens_they_touch() {
        let mut picker = session(Kind::Screenshot, None);
        picker.picked = Some(Pick::Area(Rect::new(1800.0, 100.0, 300.0, 200.0)));
        // Not before the overlay says where the screens are.
        assert_eq!(picker.targets(), None);

        picker.layout = parse_layout("HDMI-A-1 0 0 1920 1080 DP-3 1920 0 2560 1440").unwrap();
        let (outputs, area) = picker.targets().unwrap();
        assert_eq!(outputs, ["HDMI-A-1", "DP-3"]);
        assert_eq!(area, Rect::new(1800.0, 100.0, 300.0, 200.0));

        picker.picked = Some(Pick::All);
        assert_eq!(
            picker.targets().unwrap().1,
            Rect::new(0.0, 0.0, 4480.0, 1440.0)
        );
        picker.picked = Some(Pick::Output("DP-3".into()));
        assert_eq!(picker.targets().unwrap().0, ["DP-3"]);
        picker.picked = Some(Pick::Area(Rect::new(9000.0, 0.0, 10.0, 10.0)));
        assert_eq!(picker.targets(), None);

        assert_eq!(parse_layout("DP-3 0 0 1920"), None);
        assert_eq!(parse_layout(""), None);
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
