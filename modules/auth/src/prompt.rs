//! The prompts on the island: one at a time, the others waiting their turn.
//! A prompt shows who asks and why, takes a password, and has polkit's
//! helper check it. Three wrong tries, Escape or Cancel end it.

use std::collections::VecDeque;

use mochi_core::{ActivityId, ActivitySpec, ModuleCtx, Priority};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::mpsc;
use zeroize::Zeroizing;

use crate::{helper, polkit};

/// Wrong tries before a prompt gives up.
const TRIES: u32 = 3;

/// What the prompt sends.
#[derive(Deserialize)]
struct Answer {
    id: u64,
    user: String,
    password: String,
}

impl Drop for Answer {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.password);
    }
}

/// How a check came out, for the prompt `id`.
#[derive(Debug)]
pub struct Checked {
    id: u64,
    result: Result<(), Vec<String>>,
}

struct Shown {
    id: u64,
    activity: ActivityId,
    request: polkit::Request,
    /// The user picked to authenticate as.
    user: String,
    failures: u32,
    checking: bool,
    error: Option<String>,
}

#[derive(Default)]
pub struct Prompts {
    waiting: VecDeque<polkit::Request>,
    shown: Option<Shown>,
    ids: u64,
}

impl Prompts {
    /// polkitd asks: now, or after the prompts before it.
    pub fn begin(&mut self, ctx: &ModuleCtx, request: polkit::Request) {
        tracing::info!(action = %request.action, "polkit asks to authenticate");
        self.waiting.push_back(request);
        self.show_next(ctx);
    }

    /// polkitd no longer needs it: the app gave up.
    pub fn cancel(&mut self, ctx: &ModuleCtx, cookie: &str) {
        self.waiting.retain(|request| request.cookie != cookie);
        if self
            .shown
            .as_ref()
            .is_some_and(|shown| shown.request.cookie == cookie)
        {
            self.finish(ctx, false);
        }
    }

    /// The prompt left the island, by Escape, Cancel or a click outside.
    pub fn ended(&mut self, ctx: &ModuleCtx, activity: ActivityId) {
        if self
            .shown
            .as_ref()
            .is_some_and(|shown| shown.activity == activity)
        {
            self.finish(ctx, false);
        }
    }

    /// The prompt's answer, as JSON: has polkit's helper check it, and says
    /// so to `checked` when it's done.
    pub fn answer(
        &mut self,
        ctx: &ModuleCtx,
        answer: &str,
        checked: &mpsc::UnboundedSender<Checked>,
    ) -> Result<(), String> {
        let answer: Answer =
            serde_json::from_str(answer).map_err(|_| "the answer isn't what the prompt sends")?;
        let shown = self.shown.as_mut().ok_or("nothing is being asked")?;
        if shown.id != answer.id {
            return Err("that prompt is gone".into());
        }
        if shown.checking {
            return Err("still checking".into());
        }
        if !shown.request.users.contains(&answer.user) {
            return Err("that user may not allow it".into());
        }
        shown.user.clone_from(&answer.user);
        shown.checking = true;
        shown.error = None;
        ctx.update(shown.activity, payload(shown));
        let (id, cookie) = (shown.id, shown.request.cookie.clone());
        let user = answer.user.clone();
        let password = Zeroizing::new(answer.password.clone());
        let checked = checked.clone();
        tokio::spawn(async move {
            let result = helper::check(&user, &cookie, &password).await;
            let _ = checked.send(Checked { id, result });
        });
        Ok(())
    }

    /// A check is done: allowed, or another try.
    pub fn checked(&mut self, ctx: &ModuleCtx, checked: Checked) {
        let Some(shown) = self.shown.as_mut().filter(|shown| shown.id == checked.id) else {
            return;
        };
        shown.checking = false;
        match checked.result {
            Ok(()) => {
                tracing::info!(action = %shown.request.action, "polkit authenticated");
                self.finish(ctx, true);
            }
            Err(said) => {
                shown.failures += 1;
                tracing::info!(failures = shown.failures, "polkit's check failed");
                if shown.failures >= TRIES {
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
            let _ = shown.request.done.send(allowed);
        }
        self.show_next(ctx);
    }

    fn show_next(&mut self, ctx: &ModuleCtx) {
        if self.shown.is_some() {
            return;
        }
        let Some(request) = self.waiting.pop_front() else {
            return;
        };
        // The prompt takes the keyboard, as the control center does; only
        // one can.
        let close = ctx.call("control-center", "close", &[]);
        tokio::spawn(async move {
            let _ = close.await;
        });
        self.ids += 1;
        let me = polkit::name_of(
            // SAFETY: getuid has no preconditions and can't fail.
            unsafe { libc::getuid() },
        );
        let user = me
            .filter(|me| request.users.contains(me))
            .or_else(|| request.users.first().cloned())
            .unwrap_or_default();
        let mut shown = Shown {
            id: self.ids,
            // Until it's presented, just below.
            activity: ActivityId(0),
            request,
            user,
            failures: 0,
            checking: false,
            error: None,
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
    let request = &shown.request;
    json!({
        "id": shown.id,
        "kind": "polkit",
        "message": request.message,
        "action": request.action,
        "icon": request.icon,
        "app": asker(request),
        "command": request.details.get("command_line"),
        "users": request.users,
        "user": shown.user,
        "checking": shown.checking,
        "error": shown.error,
        "triesLeft": TRIES - shown.failures,
    })
}
