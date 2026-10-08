//! Network: Wi-Fi, Ethernet, VPNs and airplane mode, from NetworkManager.
//!
//! A bubble shows the connection: the Wi-Fi's strength, Ethernet, or
//! offline, with a lock while a VPN runs. The hub has a Network page, with
//! the Wi-Fi networks in range, the wired devices and the VPNs, and a card
//! on its home with Wi-Fi, VPN and airplane mode tiles. Joining a new
//! secured network asks for its password on the island. Connecting and
//! disconnecting show a short notice.
//!
//! Settings in `config.toml`, all optional:
//!
//! ```toml
//! [module.network]
//! bubble = true    # the connection's bubble
//! notices = true   # a notice on the island when the connection changes
//! ```

mod model;
mod nm;
mod tour;

use std::time::Duration;

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, Area, ArgSpec, Assets, BoxFuture, BubbleId, BubbleSpec,
    CallError, ContributionSpec, Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent,
    Priority,
};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc;
use zbus::Connection;

use crate::nm::Snapshot;

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

/// How long a connection may take to come up before it counts as failed.
const CONNECT_LIMIT: Duration = Duration::from_secs(30);
/// How long a notice stays.
const NOTICE: Duration = Duration::from_millis(2500);

#[derive(Debug, Default)]
pub struct Network;

#[derive(Debug, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    bubble: bool,
    notices: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            bubble: true,
            notices: true,
        }
    }
}

impl Module for Network {
    fn id(&self) -> &'static str {
        "network"
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
        let mut offers = vec![
            ContributionSpec::new("hub", "card", "status", "Card", "Network")
                .icon("wifi")
                .order(1)
                .options(json!({ "span": 2, "rows": 1 })),
            ContributionSpec::new("hub", "page", "page", "Page", "Network")
                .icon("wifi")
                .order(15),
        ];
        offers.extend(tour::steps());
        offers
    }

    fn actions(&self) -> Vec<ActionSpec> {
        let state =
            || ArgSpec::choice("state", "On, off, or flip it", ["on", "off", "toggle"]).optional();
        let name = |what: &'static str| ArgSpec::string("name", what).rest();
        vec![
            ActionSpec::new("wifi", "Turn Wi-Fi on or off").arg(state()),
            ActionSpec::new("airplane", "Turn every radio off, or back on").arg(state()),
            ActionSpec::new("scan", "Look for Wi-Fi networks"),
            ActionSpec::new(
                "connect",
                "Join a Wi-Fi network or start a VPN; a new secured network asks for its password",
            )
            .arg(name("A Wi-Fi network's name, or a saved connection's")),
            ActionSpec::new(
                "password",
                "Join a new Wi-Fi network with its password; the prompt sends this",
            )
            .arg(ArgSpec::string("ssid", "The network's name"))
            .arg(ArgSpec::string("password", "Its password").rest()),
            ActionSpec::new(
                "disconnect",
                "Leave a Wi-Fi network, stop a VPN, or unplug a wired device in software",
            )
            .arg(name("The network, connection or device; the Wi-Fi when left out").optional()),
            ActionSpec::new("forget", "Forget a saved Wi-Fi network")
                .arg(name("The network's name")),
            ActionSpec::new("vpn", "Start or stop a VPN")
                .arg(ArgSpec::string("name", "The VPN's name"))
                .arg(state()),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let settings: Settings = ctx.settings()?;
            let connection = Connection::system().await?;
            let (snapshots_sender, mut snapshots) = mpsc::unbounded_channel();
            tokio::spawn(nm::watch(connection.clone(), snapshots_sender));
            let (results_sender, mut results) = mpsc::unbounded_channel();
            let mut state = State {
                settings,
                connection,
                snapshot: None,
                bubble: None,
                shown_status: serde_json::Value::Null,
                prompt: None,
                airplane: None,
                results: results_sender,
            };
            state.publish(&ctx);
            loop {
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => state.command(&ctx, command).await,
                        Some(ModuleEvent::Ended { activity, .. }) if state.prompt.as_ref().is_some_and(|prompt| prompt.1 == activity) => {
                            state.prompt = None;
                        }
                        Some(ModuleEvent::BubbleClicked(_)) => {
                            let open = ctx.call("hub", "open", &["network/page"]);
                            tokio::spawn(async move {
                                if let Err(error) = open.await {
                                    tracing::debug!(%error, "can't open the hub");
                                }
                            });
                        }
                        Some(_) => {}
                    },
                    Some(snapshot) = snapshots.recv() => state.changed(&ctx, snapshot),
                    Some(result) = results.recv() => state.joined(&ctx, result).await,
                }
            }
        })
    }
}

/// How joining a new network went.
#[derive(Debug)]
struct Joined {
    ssid: String,
    /// The connection NetworkManager saved for it.
    saved: String,
    up: bool,
}

#[derive(Debug)]
struct State {
    settings: Settings,
    connection: Connection,
    snapshot: Option<Snapshot>,
    bubble: Option<BubbleId>,
    /// What the bubble shows, to tell news from a refresh.
    shown_status: serde_json::Value,
    /// The password prompt on the island: the network's name, the activity.
    prompt: Option<(String, ActivityId)>,
    /// While in airplane mode, the radios that were on: Wi-Fi, mobile,
    /// Bluetooth.
    airplane: Option<(bool, bool, bool)>,
    results: mpsc::UnboundedSender<Joined>,
}

impl State {
    fn changed(&mut self, ctx: &ModuleCtx, snapshot: Snapshot) {
        if self.settings.notices
            && let Some(before) = &self.snapshot
        {
            for (icon, text) in model::notices(before, &snapshot) {
                notice(ctx, icon, &text);
            }
        }
        self.snapshot = Some(snapshot);
        self.publish(ctx);
    }

    fn publish(&mut self, ctx: &ModuleCtx) {
        let payload = model::payload(self.snapshot.as_ref(), self.airplane.is_some());
        ctx.publish_state(payload.clone());
        match (self.settings.bubble, &self.snapshot) {
            (true, Some(_)) => {
                let status = payload["status"].clone();
                let spec = BubbleSpec::new("Bubble")
                    .key("status")
                    .area(Area::Right)
                    .group("status")
                    .payload(status.clone());
                // Another network, or none, is news; the same one isn't.
                let spec = if self.bubble.is_some() && status != self.shown_status {
                    spec.news()
                } else {
                    spec
                };
                self.shown_status = status;
                self.bubble = Some(ctx.show_bubble(spec));
            }
            _ => {
                if let Some(bubble) = self.bubble.take() {
                    ctx.hide_bubble(bubble);
                }
            }
        }
    }

    async fn command(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        let args = &command.args;
        let name = args.str("name").unwrap_or_default().to_owned();
        let state = args.str("state");
        let result = match command.action.as_str() {
            "wifi" => match &self.snapshot {
                Some(snapshot) => {
                    let on = flip(state, snapshot.wireless_enabled);
                    nm::set_radio(&self.connection, true, on)
                        .await
                        .map_err(|error| error.to_string())
                }
                None => Err("NetworkManager isn't running".into()),
            },
            "airplane" => {
                let on = flip(state, self.airplane.is_some());
                self.set_airplane(ctx, on).await
            }
            "scan" => self.scan().await,
            "connect" => self.connect(ctx, &name).await,
            "password" => {
                let ssid = args.str("ssid").unwrap_or_default().to_owned();
                let password = args.str("password").unwrap_or_default().to_owned();
                self.close_prompt(ctx);
                self.join(&ssid, Some(&password)).await
            }
            "disconnect" => self.disconnect(&name).await,
            "forget" => self.forget(&name).await,
            "vpn" => self.vpn(&name, state).await,
            other => Err(format!("network has no action {other}")),
        };
        command.reply(result);
    }

    fn snapshot(&self) -> Result<&Snapshot, String> {
        self.snapshot
            .as_ref()
            .ok_or_else(|| "NetworkManager isn't running".to_owned())
    }

    async fn scan(&self) -> Result<(), String> {
        let snapshot = self.snapshot()?;
        for device in snapshot.devices.iter().filter(|device| device.wifi) {
            // A scan asked too soon after the last fails; the old list stays.
            if let Err(error) = nm::scan(&self.connection, &device.path).await {
                tracing::debug!(%error, "scan");
            }
        }
        Ok(())
    }

    /// Joins a network by name, or starts a saved connection.
    async fn connect(&mut self, ctx: &ModuleCtx, name: &str) -> Result<(), String> {
        let snapshot = self.snapshot()?;
        let saved = snapshot
            .saved
            .iter()
            .find(|saved| saved.ssid.as_deref() == Some(name) || saved.id == name)
            .cloned();
        if let Some(saved) = saved {
            let device = (!saved.is_vpn())
                .then(|| {
                    snapshot
                        .access_points
                        .iter()
                        .find(|point| saved.ssid.as_deref() == Some(point.ssid.as_str()))
                        .map(|point| point.device.clone())
                })
                .flatten();
            return nm::activate(&self.connection, &saved.path, device.as_deref())
                .await
                .map(drop)
                .map_err(|error| format!("{name}: {error}"));
        }
        let network = model::networks(snapshot)
            .into_iter()
            .find(|network| network.ssid == name)
            .ok_or_else(|| format!("no Wi-Fi network called {name} is in range"))?;
        if network.enterprise {
            return Err(format!(
                "{name} asks for a user name; set it up once with nmcli or nm-connection-editor"
            ));
        }
        if network.secure {
            self.ask_password(ctx, name);
            return Ok(());
        }
        self.join(name, None).await
    }

    /// Joins a network not saved yet. The outcome comes back as [`Joined`].
    async fn join(&self, ssid: &str, password: Option<&str>) -> Result<(), String> {
        let snapshot = self.snapshot()?;
        let point = snapshot
            .access_points
            .iter()
            .filter(|point| point.ssid == ssid)
            .max_by_key(|point| point.strength)
            .ok_or_else(|| format!("no Wi-Fi network called {ssid} is in range"))?
            .clone();
        let (saved, active) = nm::join(&self.connection, &point, password)
            .await
            .map_err(|error| format!("{ssid}: {error}"))?;
        let connection = self.connection.clone();
        let results = self.results.clone();
        let ssid = ssid.to_owned();
        tokio::spawn(async move {
            let up = nm::settled(&connection, &active, CONNECT_LIMIT).await;
            let _ = results.send(Joined { ssid, saved, up });
        });
        Ok(())
    }

    /// A new network failed to come up: forget it, so a wrong password
    /// isn't kept, and say so.
    async fn joined(&mut self, ctx: &ModuleCtx, joined: Joined) {
        if joined.up {
            return;
        }
        if let Err(error) = nm::forget(&self.connection, &joined.saved).await {
            tracing::debug!(%error, "can't forget the failed network");
        }
        notice(
            ctx,
            "wifi-off",
            &format!("Couldn't join {}: is the password right?", joined.ssid),
        );
    }

    fn ask_password(&mut self, ctx: &ModuleCtx, ssid: &str) {
        // The prompt takes the keyboard, as the hub does; only one can.
        let close = ctx.call("hub", "close", &[]);
        tokio::spawn(async move {
            match close.await {
                Ok(()) | Err(CallError::NotEnabled(_)) => {}
                Err(error) => tracing::warn!(%error, "could not close the hub"),
            }
        });
        let spec = ActivitySpec::new("Password")
            .key("password")
            .priority(Priority::URGENT)
            .uninterruptible()
            .modal()
            .payload(json!({ "ssid": ssid }));
        self.prompt = Some((ssid.to_owned(), ctx.present(spec)));
    }

    fn close_prompt(&mut self, ctx: &ModuleCtx) {
        if let Some((_, activity)) = self.prompt.take() {
            ctx.withdraw(activity);
        }
    }

    async fn disconnect(&self, name: &str) -> Result<(), String> {
        let snapshot = self.snapshot()?;
        if name.is_empty() {
            let wifi = snapshot
                .devices
                .iter()
                .find(|device| device.wifi && device.active.is_some())
                .ok_or("no Wi-Fi network is connected")?;
            return nm::disconnect(&self.connection, &wifi.path)
                .await
                .map_err(|error| error.to_string());
        }
        if let Some(device) = snapshot
            .devices
            .iter()
            .find(|device| device.interface == name)
        {
            return nm::disconnect(&self.connection, &device.path)
                .await
                .map_err(|error| error.to_string());
        }
        let saved_ssid = |active: &nm::Active| {
            snapshot
                .saved
                .iter()
                .find(|saved| saved.path == active.connection)
                .and_then(|saved| saved.ssid.clone())
        };
        let active = snapshot
            .active
            .iter()
            .find(|active| active.id == name || saved_ssid(active).as_deref() == Some(name))
            .ok_or_else(|| format!("{name} isn't connected"))?;
        nm::deactivate(&self.connection, &active.path)
            .await
            .map_err(|error| error.to_string())
    }

    async fn forget(&self, name: &str) -> Result<(), String> {
        let snapshot = self.snapshot()?;
        let saved = snapshot
            .saved
            .iter()
            .find(|saved| {
                saved.ssid.as_deref() == Some(name) || (saved.id == name && saved.ssid.is_some())
            })
            .ok_or_else(|| format!("no saved Wi-Fi network called {name}"))?;
        nm::forget(&self.connection, &saved.path)
            .await
            .map_err(|error| error.to_string())
    }

    async fn vpn(&self, name: &str, state: Option<&str>) -> Result<(), String> {
        let snapshot = self.snapshot()?;
        let saved = snapshot
            .saved
            .iter()
            .find(|saved| saved.is_vpn() && saved.id == name)
            .ok_or_else(|| format!("no VPN called {name}"))?;
        let active = snapshot
            .active
            .iter()
            .find(|active| active.connection == saved.path);
        match (flip(state, active.is_some()), active) {
            (true, None) => nm::activate(&self.connection, &saved.path, None)
                .await
                .map(drop)
                .map_err(|error| format!("{name}: {error}")),
            (false, Some(active)) => nm::deactivate(&self.connection, &active.path)
                .await
                .map_err(|error| error.to_string()),
            _ => Ok(()),
        }
    }

    /// Turns Wi-Fi, the mobile radio and Bluetooth off, remembering which
    /// were on to turn them back on.
    async fn set_airplane(&mut self, ctx: &ModuleCtx, on: bool) -> Result<(), String> {
        let snapshot = self.snapshot()?.clone();
        if on == self.airplane.is_some() {
            return Ok(());
        }
        if on {
            let bluetooth_was_on = matches!(ctx.call("bluetooth", "powered", &[]).await, Ok(()));
            self.airplane = Some((
                snapshot.wireless_enabled,
                snapshot.wwan_enabled,
                bluetooth_was_on,
            ));
            nm::set_radio(&self.connection, true, false)
                .await
                .map_err(|error| error.to_string())?;
            let _ = nm::set_radio(&self.connection, false, false).await;
            let _ = ctx.call("bluetooth", "power", &["off"]).await;
        } else if let Some((wifi, wwan, bluetooth_on)) = self.airplane.take() {
            if wifi {
                nm::set_radio(&self.connection, true, true)
                    .await
                    .map_err(|error| error.to_string())?;
            }
            if wwan {
                let _ = nm::set_radio(&self.connection, false, true).await;
            }
            if bluetooth_on {
                let _ = ctx.call("bluetooth", "power", &["on"]).await;
            }
        }
        self.publish(ctx);
        Ok(())
    }
}

/// `on`, `off`, or the other of `now`.
fn flip(state: Option<&str>, now: bool) -> bool {
    match state {
        Some("on") => true,
        Some("off") => false,
        _ => !now,
    }
}

/// A short line on the island, gone by itself.
fn notice(ctx: &ModuleCtx, icon: &str, text: &str) {
    ctx.present(
        ActivitySpec::new("Notice")
            .key("notice")
            .priority(Priority::HIGH)
            .same_priority(mochi_core::SamePriority::Stack)
            .passive()
            .fleeting()
            .timeout(NOTICE)
            .payload(json!({ "icon": icon, "text": text })),
    );
}

#[cfg(test)]
mod settings_example {
    #[test]
    fn shows_the_defaults() {
        mochi_core::examples::check_module::<super::Settings>(
            "network",
            include_str!("../settings.toml"),
        );
    }

    #[test]
    fn flips() {
        assert!(super::flip(Some("on"), true));
        assert!(!super::flip(Some("off"), false));
        assert!(super::flip(None, false));
        assert!(!super::flip(Some("toggle"), true));
    }
}
