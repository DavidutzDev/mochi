//! Authentication: how long the session remembers what lets someone in
//! without a password: sudo's password, the SSH keys in the agent, the admin
//! actions polkit remembers and the keyring's unlocked secrets. By default
//! everything is forgotten once the screen locks; SSH keys and the keyring
//! may be kept for a time instead.
//!
//! The screen locks in several ways, and this module hears of each: logind's
//! Lock (`loginctl lock-session`, hypridle, the power module's Lock), and
//! the `locked` action, which the lock module calls once Mochi's lock is
//! on screen and the power module after starting a lock command of its own,
//! like hyprlock. A keybinding that starts a locker directly can run
//! `mochi ipc auth locked` with it.
//!
//! Settings in `config.toml`, all optional:
//!
//! ```toml
//! [module.auth]
//! remember_sudo = "until-locked"      # or "own-timeout"
//! remember_ssh_keys = "until-locked"  # "1h", "4h", "8h", "until-logout"
//! remember_keyring = "until-locked"   # "1h", "4h", "8h", "until-logout"
//! remember_polkit = "until-locked"    # or "own-timeout"
//! ```

mod helper;
mod logind;
mod polkit;
mod prompt;
mod remember;

use futures_util::StreamExt;
use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ArgSpec, Assets, BoxFuture, Module, ModuleCtx, ModuleError, ModuleEvent,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::mpsc;

use remember::{Kept, Lifetime, Remember};

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

#[derive(Debug, Default)]
pub struct Auth;

#[derive(Debug, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    /// How long sudo remembers its password: until the screen locks, or
    /// its own timeout, a lock or not.
    remember_sudo: Kept,
    /// How long SSH keys stay in the agent from when they're added: until
    /// the screen locks, a time, or until logging out.
    remember_ssh_keys: Lifetime,
    /// How long the keyring stays unlocked from when it's unlocked: until
    /// the screen locks, a time, or until logging out.
    remember_keyring: Lifetime,
    /// How long polkit remembers an admin action it may keep: until the
    /// screen locks, or its own timeout, a lock or not.
    remember_polkit: Kept,
    /// Ask for polkit's admin passwords on the island, as the session's
    /// polkit agent, in place of hyprpolkitagent or another.
    polkit: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            remember_sudo: Kept::default(),
            remember_ssh_keys: Lifetime::default(),
            remember_keyring: Lifetime::default(),
            remember_polkit: Kept::default(),
            polkit: true,
        }
    }
}

impl Settings {
    fn remember(&self) -> Remember {
        Remember {
            sudo: self.remember_sudo,
            polkit: self.remember_polkit,
            ssh_keys: self.remember_ssh_keys,
            keyring: self.remember_keyring,
        }
    }
}

impl Module for Auth {
    fn id(&self) -> &'static str {
        "auth"
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
        mochi_core::settings::<Settings>(table)
            .map(drop)
            .map_err(|error| error.to_string())
    }

    fn actions(&self) -> Vec<ActionSpec> {
        vec![
            ActionSpec::new(
                "locked",
                "The screen just locked: forget what's kept until then",
            ),
            ActionSpec::new(
                "forget",
                "Forget everything now: sudo, SSH keys, polkit and the keyring",
            ),
            ActionSpec::new(
                "answer",
                "Answer the prompt on the island; the prompt sends this",
            )
            .arg(ArgSpec::string(
                "answer",
                "JSON: the prompt's id, the user and the password",
            )),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let settings: Settings = ctx.settings()?;
            let remember = settings.remember();

            let system = zbus::Connection::system().await.ok();
            let session = match &system {
                Some(system) => logind::session(system).await.ok(),
                None => None,
            };
            let id = match &session {
                Some(session) => session.id().await.ok(),
                None => None,
            };
            let mut locks = match &session {
                Some(session) => session.receive_lock().await.ok(),
                None => None,
            };

            // SSH keys and the keyring kept for a time, taken back once
            // it's up; stopped with the module.
            let _keep = Aborts(tokio::spawn(remember::keep(remember)));

            // polkit's agent for the session, on a connection of its own:
            // polkitd forgets the agent when it closes, as the module stops.
            let (polkit_events, mut polkit_asks) = mpsc::unbounded_channel();
            let mut problem = None;
            let _agent = match (settings.polkit, &id) {
                (true, Some(id)) => match zbus::Connection::system().await {
                    Ok(agent) => match polkit::start(&agent, id, polkit_events).await {
                        Ok(()) => {
                            tracing::info!("answering polkit for the session");
                            Some(agent)
                        }
                        Err(error) => {
                            tracing::warn!(%error, "can't answer polkit");
                            problem = Some(error);
                            None
                        }
                    },
                    Err(error) => {
                        problem = Some(error.to_string());
                        None
                    }
                },
                (true, None) => {
                    problem = Some("no logind session to answer polkit for".to_owned());
                    None
                }
                (false, _) => None,
            };
            ctx.publish_state(json!({ "polkit": { "on": settings.polkit, "problem": problem } }));
            let mut prompts = prompt::Prompts::default();
            let (checked, mut checks) = mpsc::unbounded_channel();

            loop {
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => match command.action.as_str() {
                            "locked" => {
                                forget(remember, id.clone());
                                command.reply(Ok(()));
                            }
                            "forget" => {
                                forget(Remember::default(), id.clone());
                                command.reply(Ok(()));
                            }
                            "answer" => {
                                let answer = command.args.str("answer").unwrap_or_default();
                                let result = prompts.answer(&ctx, answer, &checked);
                                command.reply(result);
                            }
                            other => {
                                let error = format!("auth has no action {other}");
                                command.reply(Err(error));
                            }
                        },
                        Some(ModuleEvent::Ended { activity, .. }) => prompts.ended(&ctx, activity),
                        Some(_) => {}
                    },
                    Some(event) = polkit_asks.recv() => match event {
                        polkit::Event::Begin(request) => prompts.begin(&ctx, request),
                        polkit::Event::Cancel(cookie) => prompts.cancel(&ctx, &cookie),
                    },
                    Some(done) = checks.recv() => prompts.checked(&ctx, done),
                    Some(_) = async {
                        match &mut locks {
                            Some(locks) => locks.next().await,
                            None => std::future::pending().await,
                        }
                    } => {
                        tracing::debug!("logind asked to lock");
                        forget(remember, id.clone());
                    }
                }
            }
        })
    }
}

/// Forgets what's kept until locked, without waiting on it.
fn forget(remember: Remember, session: Option<String>) {
    tokio::spawn(remember::on_lock(remember, session));
}

/// Stops a task when dropped, as the module's run ends.
struct Aborts(tokio::task::JoinHandle<()>);

impl Drop for Aborts {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everything_goes_on_locking_by_default() {
        let table: mochi_core::toml::Table = "".parse().unwrap();
        let remember = mochi_core::settings::<Settings>(&table).unwrap().remember();
        assert_eq!(remember, Remember::default());
        assert_eq!(remember.ssh_keys, Lifetime::UntilLocked);
        assert_eq!(remember.sudo, Kept::UntilLocked);
    }

    #[test]
    fn the_example_shows_the_defaults() {
        mochi_core::examples::check_module::<Settings>("auth", include_str!("../settings.toml"));
    }
}
