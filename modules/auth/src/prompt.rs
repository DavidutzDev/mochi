//! The prompts on the island: one at a time, the others waiting their turn.
//! polkit's show who asks and why and have polkit's helper check the
//! password; three wrong tries, Escape or Cancel end them. The keyring's
//! show what gnome-keyring says and hand the answer back to it, which
//! checks it and asks again when it's wrong.

use std::collections::VecDeque;

use mochi_core::{ActivityId, ActivitySpec, ModuleCommand, ModuleCtx, Priority};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::mpsc;
use zeroize::Zeroizing;

use crate::remember::Lifetime;
use crate::{helper, polkit};

/// Wrong tries before a polkit prompt gives up.
const TRIES: u32 = 3;

/// What the prompt sends.
#[derive(Deserialize)]
struct Answer {
    id: u64,
    #[serde(default)]
    user: String,
    #[serde(default)]
    password: String,
    /// The keyring's choice, like unlocking it at login.
    #[serde(default)]
    choice: bool,
    /// `false` for No on a keyring's question.
    #[serde(default = "yes")]
    accept: bool,
    /// For an SSH key, how long it stays, as `remember_ssh_keys` says it.
    #[serde(default)]
    keep: Option<String>,
}

fn yes() -> bool {
    true
}

impl Drop for Answer {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.password);
    }
}

/// How a polkit check came out, for the prompt `id`.
#[derive(Debug)]
pub struct Checked {
    id: u64,
    result: Result<(), Vec<String>>,
}

/// What gnome-keyring asks, as its prompt's properties say.
#[derive(Debug, Clone, Default)]
pub struct KeyringAsk {
    pub serial: u64,
    /// A yes or no question rather than a password.
    pub confirm: bool,
    pub title: Option<String>,
    pub message: Option<String>,
    pub description: Option<String>,
    /// Why it asks again, like a wrong password.
    pub warning: Option<String>,
    /// A choice to tick, like unlocking the keyring at login.
    pub choice_label: Option<String>,
    pub choice: bool,
    /// A new password, typed twice.
    pub password_new: bool,
    pub continue_label: Option<String>,
    pub cancel_label: Option<String>,
}

/// The island's answer to a keyring prompt.
#[derive(Debug)]
pub struct KeyringReply {
    pub accepted: bool,
    pub password: Option<Zeroizing<String>>,
    pub choice: bool,
}

/// What `mochi askpass` asks, for sudo -A or ssh: the command waits until
/// it's answered.
pub struct AskpassAsk {
    pub command: ModuleCommand,
    /// A yes or no question, like ssh confirming the use of a key.
    pub confirm: bool,
    /// The program asking, by name.
    pub app: String,
    /// What it asks, in its own words.
    pub message: String,
    /// For an SSH key's passphrase, the key, by [`remember::key_id`], and
    /// how long it stays in the agent unless the prompt picks another time.
    pub key: Option<(String, Lifetime)>,
}

enum Ask {
    Polkit {
        request: polkit::Request,
        /// The user picked to authenticate as.
        user: String,
        failures: u32,
    },
    Keyring(KeyringAsk),
    Askpass(AskpassAsk),
}

/// What an answer leaves to do.
#[derive(Debug)]
pub enum Answered {
    Nothing,
    /// Hand this to the keyring.
    Keyring(u64, KeyringReply),
    /// Keep this SSH key as long as the prompt picked.
    Keep(String, Lifetime),
}

struct Shown {
    id: u64,
    activity: ActivityId,
    ask: Ask,
    checking: bool,
    error: Option<String>,
}

#[derive(Default)]
pub struct Prompts {
    waiting: VecDeque<Ask>,
    shown: Option<Shown>,
    ids: u64,
}

impl Prompts {
    /// polkitd asks: now, or after the prompts before it.
    pub fn begin(&mut self, ctx: &ModuleCtx, request: polkit::Request) {
        tracing::info!(action = %request.action, "polkit asks to authenticate");
        let me = polkit::name_of(
            // SAFETY: getuid has no preconditions and can't fail.
            unsafe { libc::getuid() },
        );
        let user = me
            .filter(|me| request.users.contains(me))
            .or_else(|| request.users.first().cloned())
            .unwrap_or_default();
        self.waiting.push_back(Ask::Polkit {
            request,
            user,
            failures: 0,
        });
        self.show_next(ctx);
    }

    /// The keyring asks: now, or after the prompts before it.
    pub fn ask_keyring(&mut self, ctx: &ModuleCtx, ask: KeyringAsk) {
        self.waiting.push_back(Ask::Keyring(ask));
        self.show_next(ctx);
    }

    /// sudo or ssh asks, through `mochi askpass`.
    pub fn askpass(&mut self, ctx: &ModuleCtx, ask: AskpassAsk) {
        tracing::info!(app = %ask.app, "askpass asks");
        self.waiting.push_back(Ask::Askpass(ask));
        self.show_next(ctx);
    }

    /// polkitd no longer needs it: the app gave up.
    pub fn cancel(&mut self, ctx: &ModuleCtx, cookie: &str) {
        let polkit =
            |ask: &Ask| matches!(ask, Ask::Polkit { request, .. } if request.cookie == cookie);
        self.waiting.retain(|ask| !polkit(ask));
        if self.shown.as_ref().is_some_and(|shown| polkit(&shown.ask)) {
            self.finish(ctx, false);
        }
    }

    /// The keyring stopped asking.
    pub fn withdraw_keyring(&mut self, ctx: &ModuleCtx, serial: u64) {
        let keyring = |ask: &Ask| matches!(ask, Ask::Keyring(asked) if asked.serial == serial);
        self.waiting.retain(|ask| !keyring(ask));
        if self.shown.as_ref().is_some_and(|shown| keyring(&shown.ask)) {
            self.finish(ctx, false);
        }
    }

    /// The prompt left the island, by Escape, Cancel or a click outside: a
    /// keyring prompt's No, to hand back.
    pub fn ended(&mut self, ctx: &ModuleCtx, activity: ActivityId) -> Option<(u64, KeyringReply)> {
        let shown = self
            .shown
            .as_ref()
            .filter(|shown| shown.activity == activity)?;
        let declined = match &shown.ask {
            Ask::Keyring(asked) => Some((
                asked.serial,
                KeyringReply {
                    accepted: false,
                    password: None,
                    choice: asked.choice,
                },
            )),
            Ask::Polkit { .. } | Ask::Askpass(_) => None,
        };
        self.finish(ctx, false);
        declined
    }

    /// The prompt's answer, as JSON. polkit's goes to its helper, which
    /// says how it went to `checked`; the keyring's comes back, to hand on.
    pub fn answer(
        &mut self,
        ctx: &ModuleCtx,
        answer: &str,
        checked: &mpsc::UnboundedSender<Checked>,
    ) -> Result<Answered, String> {
        let answer: Answer =
            serde_json::from_str(answer).map_err(|_| "the answer isn't what the prompt sends")?;
        let current = self.shown.as_ref().ok_or("nothing is being asked")?;
        if current.id != answer.id {
            return Err("that prompt is gone".into());
        }
        if current.checking {
            return Err("still checking".into());
        }
        // askpass's command goes with the prompt: the password is its output.
        if matches!(current.ask, Ask::Askpass(_))
            && let Some(Shown {
                activity,
                ask: Ask::Askpass(asked),
                ..
            }) = self.shown.take()
        {
            ctx.withdraw(activity);
            let keep = asked.key.map(|(key, default)| {
                let picked = answer
                    .keep
                    .as_deref()
                    .and_then(|keep| serde_json::from_value(json!(keep)).ok());
                (key, picked.unwrap_or(default))
            });
            match (asked.confirm, answer.accept) {
                (true, true) => asked.command.reply(Ok(())),
                (true, false) => asked.command.reply(Err("declined".into())),
                (false, _) => asked.command.answer(Ok(answer.password.clone())),
            }
            self.show_next(ctx);
            return Ok(keep.map_or(Answered::Nothing, |(key, lifetime)| {
                Answered::Keep(key, lifetime)
            }));
        }
        let Some(shown) = self.shown.as_mut() else {
            return Err("nothing is being asked".into());
        };
        match &mut shown.ask {
            Ask::Askpass(_) => Ok(Answered::Nothing),
            Ask::Keyring(asked) => {
                let reply = KeyringReply {
                    accepted: answer.accept,
                    password: (!asked.confirm).then(|| Zeroizing::new(answer.password.clone())),
                    choice: answer.choice,
                };
                let serial = asked.serial;
                // The keyring may ask again, as a new prompt.
                self.finish(ctx, answer.accept);
                Ok(Answered::Keyring(serial, reply))
            }
            Ask::Polkit { request, user, .. } => {
                if !request.users.contains(&answer.user) {
                    return Err("that user may not allow it".into());
                }
                user.clone_from(&answer.user);
                let (id, cookie) = (shown.id, request.cookie.clone());
                shown.checking = true;
                shown.error = None;
                ctx.update(shown.activity, payload(shown));
                let user = answer.user.clone();
                let password = Zeroizing::new(answer.password.clone());
                let checked = checked.clone();
                tokio::spawn(async move {
                    let result = helper::check(&user, &cookie, &password).await;
                    let _ = checked.send(Checked { id, result });
                });
                Ok(Answered::Nothing)
            }
        }
    }

    /// A polkit check is done: allowed, or another try.
    pub fn checked(&mut self, ctx: &ModuleCtx, checked: Checked) {
        let Some(shown) = self.shown.as_mut().filter(|shown| shown.id == checked.id) else {
            return;
        };
        let Ask::Polkit {
            request, failures, ..
        } = &mut shown.ask
        else {
            return;
        };
        shown.checking = false;
        match checked.result {
            Ok(()) => {
                tracing::info!(action = %request.action, "polkit authenticated");
                self.finish(ctx, true);
            }
            Err(said) => {
                *failures += 1;
                tracing::info!(failures = *failures, "polkit's check failed");
                if *failures >= TRIES {
                    self.finish(ctx, false);
                    return;
                }
                shown.error = Some(if said.is_empty() {
                    "Wrong password. Try again.".to_owned()
                } else {
                    said.join(" ")
                });
                ctx.update(shown.activity, payload(shown));
            }
        }
    }

    fn finish(&mut self, ctx: &ModuleCtx, allowed: bool) {
        if let Some(shown) = self.shown.take() {
            ctx.withdraw(shown.activity);
            match shown.ask {
                Ask::Polkit { request, .. } => {
                    let _ = request.done.send(allowed);
                }
                Ask::Askpass(asked) => asked.command.reply(Err("cancelled on the island".into())),
                Ask::Keyring(_) => {}
            }
        }
        self.show_next(ctx);
    }

    fn show_next(&mut self, ctx: &ModuleCtx) {
        if self.shown.is_some() {
            return;
        }
        let Some(ask) = self.waiting.pop_front() else {
            return;
        };
        // The prompt takes the keyboard, as the control center does; only
        // one can.
        let close = ctx.call("control-center", "close", &[]);
        tokio::spawn(async move {
            let _ = close.await;
        });
        self.ids += 1;
        let error = match &ask {
            Ask::Keyring(asked) => asked.warning.clone(),
            Ask::Polkit { .. } | Ask::Askpass(_) => None,
        };
        let mut shown = Shown {
            id: self.ids,
            // Until it's presented, just below.
            activity: ActivityId(0),
            ask,
            checking: false,
            error,
        };
        let spec = ActivitySpec::new("Prompt")
            .key("prompt")
            .priority(Priority::URGENT)
            .uninterruptible()
            .modal()
            .payload(payload(&shown));
        shown.activity = ctx.present(spec);
        self.shown = Some(shown);
    }
}

/// The program that asks, by its name, from the process polkit says.
fn asker(request: &polkit::Request) -> Option<String> {
    let pid = request
        .details
        .get("polkit.subject-pid")
        .or_else(|| request.details.get("polkit.caller-pid"))?;
    let name = std::fs::read_to_string(format!("/proc/{}/comm", pid.trim())).ok()?;
    Some(name.trim().to_owned()).filter(|name| !name.is_empty())
}

/// What the view shows.
fn payload(shown: &Shown) -> Value {
    match &shown.ask {
        Ask::Polkit {
            request,
            user,
            failures,
        } => json!({
            "id": shown.id,
            "kind": "polkit",
            "message": request.message,
            "action": request.action,
            "icon": request.icon,
            "app": asker(request),
            "command": request.details.get("command_line"),
            "users": request.users,
            "user": user,
            "checking": shown.checking,
            "error": shown.error,
            "triesLeft": TRIES - failures,
        }),
        Ask::Askpass(asked) => json!({
            "id": shown.id,
            "kind": "askpass",
            "app": asked.app,
            "message": asked.message,
            "confirm": asked.confirm,
            "keep": asked.key.as_ref().map(|(_, lifetime)| lifetime.name()),
            "checking": false,
        }),
        Ask::Keyring(asked) => json!({
            "id": shown.id,
            "kind": "keyring",
            "title": asked.title,
            "message": asked.message,
            "description": asked.description,
            "error": shown.error,
            "confirm": asked.confirm,
            "passwordNew": asked.password_new,
            "choiceLabel": asked.choice_label,
            "choice": asked.choice,
            "continueLabel": asked.continue_label,
            "cancelLabel": asked.cancel_label,
            "checking": false,
        }),
    }
}
