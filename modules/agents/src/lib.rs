//! Agents: coding agents like Claude Code on the island. Each session says
//! what it's doing through the agent's hooks, which run `mochi agents
//! hook`: working, waiting for the user, or done. One bubble shows every
//! session, a mark each, and a session that starts waiting shows a notice,
//! as does one that finishes, more briefly. A click on the bubble opens the
//! list of sessions, or clears them when they're all done.
//!
//! Settings in `config.toml`, all optional:
//!
//! ```toml
//! [module.agents]
//! stale_minutes = 60     # forget a session nothing was heard from since
//! waiting_notice = true  # a notice when one needs you
//! done_notice = true     # a short one when one finishes
//! waiting_ms = 8000      # how long the first stays, unless answered
//! done_ms = 3000         # how long the second stays
//! ```

mod sessions;
mod tour;

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, Area, ArgSpec, Assets, BoxFuture, BubbleId, BubbleSpec,
    ContributionSpec, Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent, Priority,
    SamePriority,
};
use serde::Deserialize;
use serde_json::Value;

use crate::sessions::{Sessions, State};

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

/// How long the list stays open, hovering aside.
const LIST: Duration = Duration::from_secs(10);
/// How often quiet sessions are looked for.
const PRUNE_EVERY: Duration = Duration::from_secs(30);

/// The settings `mochi reload` applies without a restart, which would
/// forget every session.
const LIVE: [&str; 5] = [
    "stale_minutes",
    "waiting_notice",
    "done_notice",
    "waiting_ms",
    "done_ms",
];

#[derive(Debug, Default)]
pub struct Agents;

#[derive(Debug, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    /// Minutes without news from a session before it goes, like an agent
    /// that was closed without saying so. 0 keeps them until they end.
    #[schemars(range(min = 0, max = 1440))]
    stale_minutes: u64,
    /// A notice when a session needs you: a question, or a permission to
    /// give.
    waiting_notice: bool,
    /// A short notice when a session finishes.
    done_notice: bool,
    /// How long the notice of a session that needs you stays, in
    /// milliseconds, unless you answer first.
    #[schemars(range(min = 1000, max = 60000))]
    waiting_ms: u64,
    /// How long the notice of a finished session stays, in milliseconds.
    #[schemars(range(min = 1000, max = 30000))]
    done_ms: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            stale_minutes: 60,
            waiting_notice: true,
            done_notice: true,
            waiting_ms: 8000,
            done_ms: 3000,
        }
    }
}

impl Module for Agents {
    fn id(&self) -> &'static str {
        "agents"
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

    fn live_settings(&self) -> &'static [&'static str] {
        &LIVE
    }

    fn contributions(&self) -> Vec<ContributionSpec> {
        tour::steps()
    }

    fn actions(&self) -> Vec<ActionSpec> {
        let id = || ArgSpec::string("id", "The session's id, like Claude Code's session_id");
        vec![
            ActionSpec::new(
                "set",
                "Say what a session is doing; `mochi agents hook` sends this",
            )
            .arg(id())
            .arg(ArgSpec::choice("state", "What it's doing", State::NAMES))
            .arg(
                ArgSpec::string(
                    "app",
                    "The agent, like \"Claude Code\"; empty keeps the one it had",
                )
                .optional(),
            )
            .arg(
                ArgSpec::string("title", "What it's about, like the project's folder")
                    .optional()
                    .rest(),
            ),
            ActionSpec::new("clear", "Forget a session, as when it ends").arg(id()),
            ActionSpec::new("clear-done", "Forget the sessions that are done"),
            ActionSpec::new("clear-all", "Forget every session"),
            ActionSpec::new("show", "Open the list of sessions on the island"),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let mut settings: Settings = ctx.settings()?;
            let mut island = Island::default();
            island.sync(&ctx, false);
            let mut prune = tokio::time::interval(PRUNE_EVERY);
            loop {
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => return Ok(()),
                        Some(ModuleEvent::Command(command)) => {
                            island.command(&ctx, &settings, command);
                        }
                        Some(ModuleEvent::BubbleClicked(bubble)) if island.bubble == Some(bubble) => {
                            island.clicked(&ctx);
                        }
                        Some(ModuleEvent::Clicked(activity)) if island.list != Some(activity) => {
                            // A notice: the list says more.
                            ctx.withdraw(activity);
                            island.open_list(&ctx);
                        }
                        Some(ModuleEvent::Ended { activity, .. }) => island.ended(activity),
                        Some(ModuleEvent::Reconfigured(table)) => {
                            match mochi_core::settings::<Settings>(&table) {
                                Ok(new) => settings = new,
                                Err(error) => tracing::warn!(%error, "the agents' new settings"),
                            }
                        }
                        Some(_) => {}
                    },
                    _ = prune.tick() => {
                        if settings.stale_minutes > 0 {
                            let after = Duration::from_secs(settings.stale_minutes * 60);
                            let gone = island.sessions.prune(after, SystemTime::now());
                            island.forget(&ctx, &gone);
                        }
                    }
                }
            }
        })
    }
}

/// The sessions and what of them the island shows.
#[derive(Debug, Default)]
struct Island {
    sessions: Sessions,
    bubble: Option<BubbleId>,
    list: Option<ActivityId>,
    /// The notices of sessions waiting for you, which go once they don't.
    waiting: HashMap<String, ActivityId>,
    /// The payload last published, to skip the island when nothing changed.
    published: Value,
}

impl Island {
    fn command(&mut self, ctx: &ModuleCtx, settings: &Settings, command: ModuleCommand) {
        let args = &command.args;
        let id = args.str("id").unwrap_or_default().trim().to_owned();
        let result = match command.action.as_str() {
            "set" if id.is_empty() => Err("a session needs an id".to_owned()),
            "set" => {
                // The action's choice took anything else.
                let state = args
                    .str("state")
                    .and_then(State::parse)
                    .unwrap_or(State::Working);
                let app = args.str("app").unwrap_or_default();
                let title = args.str("title").unwrap_or_default();
                if let Some(entered) = self.sessions.set(&id, state, app, title, SystemTime::now())
                {
                    self.entered(ctx, settings, &id, entered);
                }
                Ok(())
            }
            "clear" => {
                if self.sessions.clear(&id) {
                    self.forget(ctx, &[id]);
                    Ok(())
                } else {
                    Err(format!("no session {id}"))
                }
            }
            "clear-done" => {
                let gone = self.sessions.clear_done();
                self.forget(ctx, &gone);
                Ok(())
            }
            "clear-all" => {
                let gone = self.sessions.clear_all();
                self.forget(ctx, &gone);
                Ok(())
            }
            "show" if self.sessions.is_empty() => Err("no agent sessions to show".to_owned()),
            "show" => {
                self.open_list(ctx);
                Ok(())
            }
            other => Err(format!("agents has no action {other}")),
        };
        command.reply(result);
    }

    /// A session entered `state`: its notices, and the island.
    fn entered(&mut self, ctx: &ModuleCtx, settings: &Settings, id: &str, state: State) {
        if let Some(notice) = self.waiting.remove(id) {
            ctx.withdraw(notice);
        }
        let Some(session) = self.sessions.get(id) else {
            return;
        };
        match state {
            State::Waiting if settings.waiting_notice => {
                let spec = ActivitySpec::new("Notice")
                    .key(format!("waiting/{id}"))
                    .priority(Priority::HIGH)
                    .timeout(Duration::from_millis(settings.waiting_ms))
                    .payload(sessions::session_payload(session));
                self.waiting.insert(id.to_owned(), ctx.present(spec));
            }
            State::Done if settings.done_notice => {
                ctx.present(
                    ActivitySpec::new("Notice")
                        .key(format!("done/{id}"))
                        .passive()
                        .fleeting()
                        .timeout(Duration::from_millis(settings.done_ms))
                        .payload(sessions::session_payload(session)),
                );
            }
            _ => {}
        }
        // Someone waiting, or finished, is news for the bubble; working
        // again isn't.
        self.sync(ctx, state != State::Working);
    }

    /// Sessions that went: their notices, and the island.
    fn forget(&mut self, ctx: &ModuleCtx, gone: &[String]) {
        for id in gone {
            if let Some(notice) = self.waiting.remove(id) {
                ctx.withdraw(notice);
            }
        }
        self.sync(ctx, false);
    }

    /// Brings the state, the bubble and the open list in line with the
    /// sessions.
    fn sync(&mut self, ctx: &ModuleCtx, news: bool) {
        let payload = self.sessions.payload();
        if payload == self.published && !news {
            return;
        }
        ctx.publish_state(payload.clone());
        self.published = payload.clone();

        let Some(top) = self.sessions.top() else {
            if let Some(bubble) = self.bubble.take() {
                ctx.hide_bubble(bubble);
            }
            if let Some(list) = self.list.take() {
                ctx.withdraw(list);
            }
            return;
        };
        let priority = match top {
            State::Waiting => Priority::HIGH,
            State::Working => Priority::NORMAL,
            State::Done => Priority::LOW,
        };
        let spec = BubbleSpec::new("Bubble")
            .key("sessions")
            .wide("Wide")
            .area(Area::CenterRight)
            .priority(priority)
            .payload(payload.clone());
        let spec = if news { spec.news() } else { spec };
        self.bubble = Some(ctx.show_bubble(spec));
        if let Some(list) = self.list {
            ctx.update(list, payload);
        }
    }

    /// A click on the bubble: sessions that are all done are seen, so they
    /// go; otherwise the list opens, or closes.
    fn clicked(&mut self, ctx: &ModuleCtx) {
        if self.sessions.all_done() {
            let gone = self.sessions.clear_done();
            self.forget(ctx, &gone);
        } else if let Some(list) = self.list.take() {
            ctx.withdraw(list);
        } else {
            self.open_list(ctx);
        }
    }

    fn open_list(&mut self, ctx: &ModuleCtx) {
        if self.sessions.is_empty() {
            return;
        }
        let spec = ActivitySpec::new("List")
            .key("list")
            .same_priority(SamePriority::Stack)
            .timeout(LIST)
            .payload(self.sessions.payload());
        self.list = Some(ctx.present(spec));
    }

    fn ended(&mut self, activity: ActivityId) {
        if self.list == Some(activity) {
            self.list = None;
        }
        self.waiting.retain(|_, notice| *notice != activity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_example_shows_the_defaults() {
        mochi_core::examples::check_module::<Settings>("agents", include_str!("../settings.toml"));
    }

    #[test]
    fn every_action_is_well_formed() {
        for action in Agents.actions() {
            mochi_core::actions::validate(&action).unwrap();
        }
    }

    #[test]
    fn states_parse_by_their_names() {
        for name in State::NAMES {
            let state = State::parse(name).unwrap();
            assert_eq!(serde_json::to_value(state).unwrap(), name);
        }
        assert_eq!(State::parse("sleeping"), None);
    }
}
