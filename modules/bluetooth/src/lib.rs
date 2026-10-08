//! Bluetooth: power, paired devices with their battery, scanning and
//! pairing, from BlueZ.
//!
//! The hub has a Bluetooth page, with the paired devices to connect or
//! forget and the devices in range to pair, and a tile on its home. A
//! bubble shows while a device is connected. Pairing asks its questions on
//! the island, like whether the code on a phone matches. Devices that come
//! or go show a short notice.
//!
//! Settings in `config.toml`, all optional:
//!
//! ```toml
//! [module.bluetooth]
//! bubble = true      # a bubble while a device is connected
//! notices = true     # a notice when a device connects or disconnects
//! scan_seconds = 30  # how long a scan looks for devices
//! ```

mod agent;
mod bluez;
mod model;

use std::time::{Duration, Instant};

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, Area, ArgSpec, Assets, BoxFuture, BubbleId, BubbleSpec,
    CallError, ContributionSpec, Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent,
    Priority,
};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::{mpsc, oneshot};
use zbus::Connection;

use crate::agent::Question;
use crate::bluez::Snapshot;

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

const NOTICE: Duration = Duration::from_millis(2500);

#[derive(Debug, Default)]
pub struct Bluetooth;

#[derive(Debug, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    bubble: bool,
    notices: bool,
    scan_seconds: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            bubble: true,
            notices: true,
            scan_seconds: 30,
        }
    }
}

impl Module for Bluetooth {
    fn id(&self) -> &'static str {
        "bluetooth"
    }

    fn assets(&self) -> Assets {
        Assets::new(&QML, concat!(env!("CARGO_MANIFEST_DIR"), "/qml"))
    }

    fn settings_example(&self) -> &'static str {
        include_str!("../settings.toml")
    }

    fn settings_schema(&self) -> Option<serde_json::Value> {
        Some(mochi_core::options::schema_of::<Settings>())
    }

    fn check_settings(&self, table: &mochi_core::toml::Table) -> Result<(), String> {
        mochi_core::settings::<Settings>(table)
            .map(drop)
            .map_err(|error| error.to_string())
    }

    fn contributions(&self) -> Vec<ContributionSpec> {
        vec![
            ContributionSpec::new("hub", "card", "status", "Card", "Bluetooth")
                .icon("bluetooth")
                .order(2)
                .options(json!({ "span": 1, "rows": 1 })),
            ContributionSpec::new("hub", "page", "page", "Page", "Bluetooth")
                .icon("bluetooth")
                .order(16),
        ]
    }

    fn actions(&self) -> Vec<ActionSpec> {
        let state =
            || ArgSpec::choice("state", "On, off, or flip it", ["on", "off", "toggle"]).optional();
        let device = || ArgSpec::string("device", "The device's name or address").rest();
        vec![
            ActionSpec::new("power", "Turn Bluetooth on or off").arg(state()),
            ActionSpec::new("powered", "Succeed when Bluetooth is on, for scripts"),
            ActionSpec::new("scan", "Look for devices to pair").arg(state()),
            ActionSpec::new("connect", "Connect a paired device").arg(device()),
            ActionSpec::new("disconnect", "Disconnect a device").arg(device()),
            ActionSpec::new("pair", "Pair a device in range, then connect it").arg(device()),
            ActionSpec::new("forget", "Forget a paired device").arg(device()),
            ActionSpec::new(
                "answer",
                "Answer the pairing question; the island sends this",
            )
            .arg(ArgSpec::choice("answer", "Yes or no", ["yes", "no"])),
            ActionSpec::new(
                "pin",
                "Give the code pairing asks for; the island sends this",
            )
            .arg(ArgSpec::string("code", "The code").rest()),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let settings: Settings = ctx.settings()?;
            let connection = Connection::system().await?;
            let (questions_sender, mut questions) = mpsc::unbounded_channel();
            agent::serve(&connection, questions_sender).await?;
            let (snapshots_sender, mut snapshots) = mpsc::unbounded_channel();
            tokio::spawn(bluez::watch(connection.clone(), snapshots_sender));
            let (failures_sender, mut failures) = mpsc::unbounded_channel();
            let mut state = State {
                settings,
                connection,
                snapshot: None,
                bubble: None,
                shown_devices: None,
                question: None,
                scan_ends: None,
                failures: failures_sender,
            };
            state.publish(&ctx);
            loop {
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => {
                            state.stop_scan().await;
                            return Ok(());
                        }
                        Some(ModuleEvent::Command(command)) => state.command(&ctx, command).await,
                        Some(ModuleEvent::Ended { activity, .. }) => state.ended(activity),
                        Some(ModuleEvent::BubbleClicked(_)) => {
                            let open = ctx.call("hub", "open", &["bluetooth/page"]);
                            tokio::spawn(async move {
                                if let Err(error) = open.await {
                                    tracing::debug!(%error, "can't open the hub");
                                }
                            });
                        }
                        Some(_) => {}
                    },
                    Some(snapshot) = snapshots.recv() => state.changed(&ctx, snapshot),
                    Some(question) = questions.recv() => state.ask(&ctx, question),
                    Some(failure) = failures.recv() => notice(&ctx, &failure),
                    () = sleep_until(state.scan_ends) => state.stop_scan().await,
                }
            }
        })
    }
}

/// A pairing question on the island, and where its answer goes.
#[derive(Debug)]
enum Asking {
    YesNo(oneshot::Sender<bool>),
    Code(oneshot::Sender<Option<String>>),
    /// A code to type on the device: nothing to answer.
    Showing,
}

#[derive(Debug)]
struct State {
    settings: Settings,
    connection: Connection,
    /// `None` while there's no adapter or BlueZ isn't running.
    snapshot: Option<Snapshot>,
    bubble: Option<BubbleId>,
    /// The bubble's device and count, to tell news from a refresh.
    shown_devices: Option<(serde_json::Value, serde_json::Value)>,
    question: Option<(ActivityId, Asking)>,
    /// When Mochi's scan stops.
    scan_ends: Option<Instant>,
    failures: mpsc::UnboundedSender<String>,
}

impl State {
    fn changed(&mut self, ctx: &ModuleCtx, snapshot: Option<Snapshot>) {
        if snapshot.is_some() && self.snapshot.is_none() {
            // BlueZ came, or came back: it forgot the agent.
            let connection = self.connection.clone();
            tokio::spawn(async move {
                if let Err(error) = agent::register(&connection).await {
                    tracing::warn!(%error, "can't answer pairing questions");
                }
            });
        }
        if self.settings.notices
            && let (Some(before), Some(after)) = (&self.snapshot, &snapshot)
        {
            for line in model::notices(before, after) {
                notice(ctx, &line);
            }
        }
        self.snapshot = snapshot;
        self.publish(ctx);
    }

    fn publish(&mut self, ctx: &ModuleCtx) {
        ctx.publish_state(model::payload(
            self.snapshot.as_ref(),
            self.scan_ends.is_some(),
        ));
        let payload = self
            .snapshot
            .as_ref()
            .filter(|_| self.settings.bubble)
            .and_then(model::bubble);
        match payload {
            Some(payload) => {
                // Another device connected or gone is news; a battery
                // reading isn't.
                let devices = (payload["name"].clone(), payload["count"].clone());
                let news = self.bubble.is_some() && Some(&devices) != self.shown_devices.as_ref();
                self.shown_devices = Some(devices);
                let spec = BubbleSpec::new("Bubble")
                    .key("connected")
                    .area(Area::Right)
                    .group("status")
                    .payload(payload);
                let spec = if news { spec.news() } else { spec };
                self.bubble = Some(ctx.show_bubble(spec));
            }
            None => {
                if let Some(bubble) = self.bubble.take() {
                    ctx.hide_bubble(bubble);
                }
            }
        }
    }

    fn snapshot(&self) -> Result<&Snapshot, String> {
        self.snapshot
            .as_ref()
            .ok_or_else(|| "there's no Bluetooth adapter, or BlueZ isn't running".to_owned())
    }

    fn device(&self, name: &str) -> Result<bluez::Device, String> {
        self.snapshot()?
            .devices
            .iter()
            .find(|device| {
                device.name.eq_ignore_ascii_case(name) || device.address.eq_ignore_ascii_case(name)
            })
            .cloned()
            .ok_or_else(|| format!("no Bluetooth device called {name}"))
    }

    async fn command(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        let args = &command.args;
        let device = args.str("device").unwrap_or_default().to_owned();
        let result = match command.action.as_str() {
            "power" => match self.snapshot() {
                Ok(snapshot) => {
                    let on = match args.str("state") {
                        Some("on") => true,
                        Some("off") => false,
                        _ => !snapshot.powered,
                    };
                    bluez::set_powered(&self.connection, &snapshot.adapter, on)
                        .await
                        .map_err(|error| error.to_string())
                }
                Err(error) => Err(error),
            },
            "powered" => match self.snapshot() {
                Ok(snapshot) if snapshot.powered => Ok(()),
                Ok(_) => Err("Bluetooth is off".into()),
                Err(error) => Err(error),
            },
            "scan" => {
                let on = match args.str("state") {
                    Some("on") => true,
                    Some("off") => false,
                    _ => self.scan_ends.is_none(),
                };
                if on {
                    self.start_scan(ctx).await
                } else {
                    self.stop_scan().await;
                    self.publish(ctx);
                    Ok(())
                }
            }
            "connect" => self.device(&device).map(|found| {
                self.run(found.name.clone(), move |connection| async move {
                    bluez::connect(&connection, &found.path).await
                });
            }),
            "disconnect" => match self.device(&device) {
                Ok(found) => bluez::disconnect(&self.connection, &found.path)
                    .await
                    .map_err(|error| error.to_string()),
                Err(error) => Err(error),
            },
            "pair" => self.device(&device).map(|found| {
                self.run(found.name.clone(), move |connection| async move {
                    bluez::pair(&connection, &found.path).await
                });
            }),
            "forget" => match (self.device(&device), self.snapshot()) {
                (Ok(found), Ok(snapshot)) => {
                    bluez::forget(&self.connection, &snapshot.adapter, &found.path)
                        .await
                        .map_err(|error| error.to_string())
                }
                (Err(error), _) | (_, Err(error)) => Err(error),
            },
            "answer" => {
                let yes = args.str("answer") == Some("yes");
                self.answer(ctx, |asking| match asking {
                    Asking::YesNo(answer) => {
                        let _ = answer.send(yes);
                    }
                    Asking::Code(answer) => {
                        let _ = answer.send(None);
                    }
                    Asking::Showing => {}
                })
            }
            "pin" => {
                let code = args.str("code").unwrap_or_default().to_owned();
                self.answer(ctx, |asking| match asking {
                    Asking::Code(answer) => {
                        let _ = answer.send(Some(code));
                    }
                    Asking::YesNo(answer) => {
                        let _ = answer.send(false);
                    }
                    Asking::Showing => {}
                })
            }
            other => Err(format!("bluetooth has no action {other}")),
        };
        command.reply(result);
    }

    /// Runs a call that may take a while, like pairing, which asks its
    /// questions meanwhile. A failure shows as a notice.
    fn run<F, Fut>(&self, name: String, call: F)
    where
        F: FnOnce(Connection) -> Fut + Send + 'static,
        Fut: Future<Output = zbus::Result<()>> + Send,
    {
        let connection = self.connection.clone();
        let failures = self.failures.clone();
        tokio::spawn(async move {
            if let Err(error) = call(connection).await {
                let reason = match &error {
                    zbus::Error::MethodError(_, Some(message), _) => message.clone(),
                    other => other.to_string(),
                };
                let _ = failures.send(format!("{name}: {reason}"));
            }
        });
    }

    async fn start_scan(&mut self, ctx: &ModuleCtx) -> Result<(), String> {
        let snapshot = self.snapshot()?;
        if !snapshot.powered {
            return Err("Bluetooth is off".into());
        }
        if self.scan_ends.is_none() {
            bluez::discover(&self.connection, &snapshot.adapter, true)
                .await
                .map_err(|error| error.to_string())?;
        }
        self.scan_ends = Some(Instant::now() + Duration::from_secs(self.settings.scan_seconds));
        self.publish(ctx);
        Ok(())
    }

    async fn stop_scan(&mut self) {
        if self.scan_ends.take().is_some()
            && let Some(snapshot) = &self.snapshot
        {
            let _ = bluez::discover(&self.connection, &snapshot.adapter, false).await;
        }
    }

    /// Puts a pairing question on the island.
    fn ask(&mut self, ctx: &ModuleCtx, question: Question) {
        let name = |device: &str| {
            self.snapshot
                .as_ref()
                .and_then(|snapshot| {
                    snapshot
                        .devices
                        .iter()
                        .find(|candidate| candidate.path == device)
                })
                .map_or_else(|| "A device".to_owned(), |device| device.name.clone())
        };
        let (payload, asking) = match question {
            Question::Confirm {
                device,
                code,
                answer,
            } => (
                json!({ "kind": "confirm", "device": name(&device), "code": format!("{code:06}") }),
                Asking::YesNo(answer),
            ),
            Question::Allow { device, answer } => (
                json!({ "kind": "allow", "device": name(&device) }),
                Asking::YesNo(answer),
            ),
            Question::Pin { device, answer } => (
                json!({ "kind": "pin", "device": name(&device) }),
                Asking::Code(answer),
            ),
            Question::Show { device, code } => (
                json!({ "kind": "show", "device": name(&device), "code": code }),
                Asking::Showing,
            ),
            Question::Cancel => {
                if let Some((activity, _)) = self.question.take() {
                    ctx.withdraw(activity);
                }
                return;
            }
        };
        // Pairing takes the keyboard, as the hub does; only one can.
        let close = ctx.call("hub", "close", &[]);
        tokio::spawn(async move {
            match close.await {
                Ok(()) | Err(CallError::NotEnabled(_)) => {}
                Err(error) => tracing::warn!(%error, "could not close the hub"),
            }
        });
        let spec = ActivitySpec::new("Prompt")
            .key("pairing")
            .priority(Priority::URGENT)
            .uninterruptible()
            .modal()
            .payload(payload);
        // A newer question replaces an older one, which then goes unanswered.
        self.question = Some((ctx.present(spec), asking));
    }

    fn answer(&mut self, ctx: &ModuleCtx, give: impl FnOnce(Asking)) -> Result<(), String> {
        let (activity, asking) = self.question.take().ok_or("nothing is being asked")?;
        give(asking);
        ctx.withdraw(activity);
        Ok(())
    }

    /// The question went away unanswered, by Escape or a click outside: no.
    fn ended(&mut self, activity: ActivityId) {
        if self
            .question
            .as_ref()
            .is_some_and(|(asked, _)| *asked == activity)
            && let Some((_, asking)) = self.question.take()
        {
            match asking {
                Asking::YesNo(answer) => {
                    let _ = answer.send(false);
                }
                Asking::Code(answer) => {
                    let _ = answer.send(None);
                }
                Asking::Showing => {}
            }
        }
    }
}

/// A short line on the island, gone by itself.
fn notice(ctx: &ModuleCtx, text: &str) {
    ctx.present(
        ActivitySpec::new("Notice")
            .key("notice")
            .priority(Priority::HIGH)
            .same_priority(mochi_core::SamePriority::Stack)
            .passive()
            .fleeting()
            .timeout(NOTICE)
            .payload(json!({ "text": text })),
    );
}

/// Waits until `deadline`, or forever without one.
async fn sleep_until(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline.into()).await,
        None => std::future::pending().await,
    }
}

#[cfg(test)]
mod settings_example {
    #[test]
    fn shows_the_defaults() {
        mochi_core::examples::check_module::<super::Settings>(
            "bluetooth",
            include_str!("../settings.toml"),
        );
    }
}
