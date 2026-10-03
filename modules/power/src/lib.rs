//! Power: lock, log out, suspend, hibernate, reboot, reboot to firmware and
//! shut down, plus power profiles. It has no island view of its own: it
//! offers the hub a page, and the CLI runs the same actions. Buttons only
//! show for what logind allows; profiles only when power-profiles-daemon
//! runs.
//!
//! Settings in `config.toml`, all optional:
//!
//! ```toml
//! [module.power]
//! # Commands that replace the defaults: logind locks the session (your
//! # locker answers), and logging out is `uwsm stop` under uwsm, otherwise
//! # logind ends the session.
//! lock = ["hyprlock"]
//! logout = ["hyprctl", "dispatch", "exit"]
//! ```

mod system;

use std::process::Stdio;

use futures_util::StreamExt;
use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ArgSpec, Assets, BoxFuture, ContributionSpec, Module, ModuleCommand, ModuleCtx,
    ModuleError, ModuleEvent,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::process::Command;
use zbus::Connection;

use crate::system::{Abilities, ManagerProxy, ProfilesProxy};

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

#[derive(Debug, Default)]
pub struct Power;

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    lock: Vec<String>,
    logout: Vec<String>,
}

/// A button on the page, in page order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Button {
    action: &'static str,
    label: &'static str,
    icon: &'static str,
    /// Needs a second click: it ends the session.
    confirm: bool,
}

const BUTTONS: [Button; 7] = [
    Button {
        action: "lock",
        label: "Lock",
        icon: "lock",
        confirm: false,
    },
    Button {
        action: "logout",
        label: "Log out",
        icon: "logout",
        confirm: true,
    },
    Button {
        action: "suspend",
        label: "Suspend",
        icon: "moon",
        confirm: false,
    },
    Button {
        action: "hibernate",
        label: "Hibernate",
        icon: "snow",
        confirm: false,
    },
    Button {
        action: "reboot",
        label: "Reboot",
        icon: "reboot",
        confirm: true,
    },
    Button {
        action: "firmware",
        label: "Firmware",
        icon: "chip",
        confirm: true,
    },
    Button {
        action: "shutdown",
        label: "Shut down",
        icon: "power",
        confirm: true,
    },
];

/// The buttons logind allows. Lock and log out always show.
fn buttons(abilities: Abilities) -> Vec<Button> {
    BUTTONS
        .into_iter()
        .filter(|button| match button.action {
            "suspend" => abilities.suspend,
            "hibernate" => abilities.hibernate,
            "reboot" => abilities.reboot,
            "firmware" => abilities.firmware,
            "shutdown" => abilities.shutdown,
            _ => true,
        })
        .collect()
}

impl Module for Power {
    fn id(&self) -> &'static str {
        "power"
    }

    fn assets(&self) -> Assets {
        Assets::new(&QML, concat!(env!("CARGO_MANIFEST_DIR"), "/qml"))
    }

    fn actions(&self) -> Vec<ActionSpec> {
        vec![
            ActionSpec::new("lock", "Lock the screen"),
            ActionSpec::new("logout", "End the session"),
            ActionSpec::new("suspend", "Suspend to memory"),
            ActionSpec::new("hibernate", "Hibernate to disk"),
            ActionSpec::new("reboot", "Restart the machine"),
            ActionSpec::new("firmware", "Restart into the firmware setup"),
            ActionSpec::new("shutdown", "Turn the machine off"),
            ActionSpec::new("profile", "Switch the power profile").arg(ArgSpec::string(
                "name",
                "power-saver, balanced or performance",
            )),
        ]
    }

    fn contributions(&self) -> Vec<ContributionSpec> {
        vec![
            ContributionSpec::new("hub", "page", "power", "Page", "Power")
                .icon("power")
                .order(90),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let settings: Settings = ctx.settings()?;
            let system = Connection::system().await?;
            let manager = system::manager(&system).await?;
            let buttons = buttons(Abilities::read(&manager).await);
            let uwsm = succeeds(Command::new("uwsm").args(["check", "is-active"])).await;

            // Profiles are optional: without the service, the page has none.
            let profiles = match ProfilesProxy::new(&system).await {
                Ok(proxy) => match proxy.profiles().await {
                    Ok(list) => Some((proxy, system::profile_names(&list))),
                    Err(error) => {
                        tracing::info!(%error, "no power profiles");
                        None
                    }
                },
                Err(_) => None,
            };
            let mut changes = match &profiles {
                Some((proxy, _)) => Some(proxy.receive_active_profile_changed().await),
                None => None,
            };

            let names = profiles.as_ref().map(|(_, names)| names.clone());
            let active = match &profiles {
                Some((proxy, _)) => proxy.active_profile().await.ok(),
                None => None,
            };
            ctx.publish_state(state(&buttons, names.as_ref(), active));

            loop {
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => {
                            let target = Target {
                                system: &system,
                                manager: &manager,
                                profiles: profiles.as_ref().map(|(proxy, _)| proxy),
                                settings: &settings,
                                uwsm,
                            };
                            run(&target, command).await;
                        }
                        Some(_) => {}
                    },
                    Some(change) = next_change(&mut changes) => {
                        let active = change.get().await.ok();
                        ctx.publish_state(state(&buttons, names.as_ref(), active));
                    }
                }
            }
        })
    }
}

/// What the page shows.
fn state(buttons: &[Button], profiles: Option<&Vec<String>>, active: Option<String>) -> Value {
    json!({
        "buttons": buttons
            .iter()
            .map(|button| json!({
                "action": button.action,
                "label": button.label,
                "icon": button.icon,
                "confirm": button.confirm,
            }))
            .collect::<Vec<_>>(),
        "profiles": profiles.cloned().unwrap_or_default(),
        "profile": active,
    })
}

type ProfileChanges = zbus::proxy::PropertyStream<'static, String>;

async fn next_change(
    changes: &mut Option<ProfileChanges>,
) -> Option<zbus::proxy::PropertyChanged<'static, String>> {
    match changes {
        Some(changes) => changes.next().await,
        None => std::future::pending().await,
    }
}

/// Everything an action may need.
struct Target<'a> {
    system: &'a Connection,
    manager: &'a ManagerProxy<'static>,
    profiles: Option<&'a ProfilesProxy<'static>>,
    settings: &'a Settings,
    uwsm: bool,
}

async fn run(target: &Target<'_>, command: ModuleCommand) {
    // `interactive` lets polkit ask for a password when the policy wants one.
    let result = match command.action.as_str() {
        "lock" if !target.settings.lock.is_empty() => spawn(&target.settings.lock),
        "lock" => session_call(target, |session| async move { session.lock().await }).await,
        "logout" if !target.settings.logout.is_empty() => spawn(&target.settings.logout),
        "logout" if target.uwsm => spawn(&["uwsm".into(), "stop".into()]),
        "logout" => session_call(target, |session| async move { session.terminate().await }).await,
        "suspend" => target.manager.suspend(true).await.map_err(describe),
        "hibernate" => target.manager.hibernate(true).await.map_err(describe),
        "reboot" => target.manager.reboot(true).await.map_err(describe),
        "firmware" => match target.manager.set_reboot_to_firmware_setup(true).await {
            Ok(()) => target.manager.reboot(true).await.map_err(describe),
            Err(error) => Err(describe(error)),
        },
        "shutdown" => target.manager.power_off(true).await.map_err(describe),
        "profile" => match target.profiles {
            Some(profiles) => {
                let name = command.args.str("name").unwrap_or_default();
                profiles.set_active_profile(name).await.map_err(describe)
            }
            None => Err("no power profiles service is running".into()),
        },
        other => Err(format!("power has no action {other}")),
    };
    if let Err(message) = &result {
        tracing::warn!(action = %command.action, %message, "power action failed");
    }
    command.reply(result);
}

async fn session_call<F, Fut>(target: &Target<'_>, call: F) -> Result<(), String>
where
    F: FnOnce(system::SessionProxy<'static>) -> Fut,
    Fut: Future<Output = zbus::Result<()>>,
{
    let session = system::session(target.system).await.map_err(describe)?;
    call(session).await.map_err(describe)
}

fn describe(error: zbus::Error) -> String {
    match error {
        zbus::Error::MethodError(name, Some(message), _) => format!("{message} ({name})"),
        other => other.to_string(),
    }
}

/// Starts a command and lets it run on its own.
fn spawn(argv: &[String]) -> Result<(), String> {
    let (program, args) = argv.split_first().ok_or("empty command")?;
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|error| format!("cannot start {program}: {error}"))?;
    tokio::spawn(async move {
        let _ = child.wait().await;
    });
    Ok(())
}

async fn succeeds(command: &mut Command) -> bool {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .is_ok_and(|status| status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_follow_what_logind_allows() {
        let abilities = Abilities {
            shutdown: true,
            reboot: true,
            suspend: true,
            hibernate: false,
            firmware: true,
        };
        let actions: Vec<&str> = buttons(abilities)
            .iter()
            .map(|button| button.action)
            .collect();
        assert_eq!(
            actions,
            [
                "lock", "logout", "suspend", "reboot", "firmware", "shutdown"
            ]
        );
        let none: Vec<&str> = buttons(Abilities::default())
            .iter()
            .map(|button| button.action)
            .collect();
        assert_eq!(none, ["lock", "logout"]);
    }

    #[test]
    fn only_session_enders_ask_twice() {
        let asking: Vec<&str> = BUTTONS
            .iter()
            .filter(|button| button.confirm)
            .map(|button| button.action)
            .collect();
        assert_eq!(asking, ["logout", "reboot", "firmware", "shutdown"]);
    }

    #[test]
    fn state_lists_buttons_and_profiles() {
        let names = vec!["power-saver".to_owned(), "balanced".to_owned()];
        let state = state(&BUTTONS[..1], Some(&names), Some("balanced".into()));
        assert_eq!(state["buttons"][0]["label"], "Lock");
        assert_eq!(state["profiles"], json!(["power-saver", "balanced"]));
        assert_eq!(state["profile"], "balanced");

        let without = state_without_profiles();
        assert_eq!(without["profiles"], json!([]));
        assert_eq!(without["profile"], Value::Null);
    }

    fn state_without_profiles() -> Value {
        state(&BUTTONS, None, None)
    }
}
