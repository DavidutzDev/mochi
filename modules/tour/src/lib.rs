//! Tour: a guided tour of Mochi. The first time Mochi starts, a notice on
//! the island offers it; after an update, it offers a tour of what's new
//! since the last one. `mochi ipc tour start` runs it any time.
//!
//! While it runs, the island is paused for every other module: their
//! notices and bubbles wait and show after. Both monitors dim, and the dim
//! layer takes every click and key. Each step shows a module's real view
//! with made-up data, from the steps modules offer (see [`steps`]), and
//! moves on by itself; Space and the arrows move sooner, and Escape asks
//! whether to stop. A stopped tour starts again where it stopped.
//!
//! Steps that show a look of the island try it through the daemon's
//! preview layer, which applies options without saving them, and drop it
//! after.

mod state;
mod steps;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use include_dir::{Dir, include_dir};
use mochi_core::{
    ActionSpec, ActivityId, ActivitySpec, Area, ArgSpec, Assets, BoxFuture, BubbleId, BubbleSpec,
    Contribution, ContributionSpec, Module, ModuleCommand, ModuleCtx, ModuleError, ModuleEvent,
    Priority, SettingsOp,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::state::{Offer, Saved};
use crate::steps::{Known, Step, Version};

static QML: Dir = include_dir!("$CARGO_MANIFEST_DIR/qml");

/// How long after starting the offer waits, so it doesn't come with
/// everything else.
const OFFER_AFTER: Duration = Duration::from_secs(3);

#[derive(Debug, Default)]
pub struct Tour;

#[derive(Debug, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    install: bool,
    updates: bool,
    #[schemars(range(min = 2, max = 30))]
    seconds_per_step: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            install: true,
            updates: true,
            seconds_per_step: 5,
        }
    }
}

impl Module for Tour {
    fn id(&self) -> &'static str {
        "tour"
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
            ActionSpec::new("start", "Take the tour, from where it stopped if it did").arg(
                ArgSpec::choice(
                    "tour",
                    "all of Mochi, or what's new since the last tour",
                    ["all", "new"],
                )
                .optional(),
            ),
            ActionSpec::new("next", "Go on to the next step"),
            ActionSpec::new("back", "Go back a step"),
            ActionSpec::new("stop", "Ask whether to stop the tour"),
            ActionSpec::new("end", "Stop the tour now"),
            ActionSpec::new("continue", "Carry on after asking to stop"),
            ActionSpec::new("later", "Not now: offer the tour again next time"),
            ActionSpec::new("never", "Never offer the tour again"),
            ActionSpec::new(
                "search",
                "List the tours a query names, as JSON lines; the launcher sends this",
            )
            .arg(
                ArgSpec::string("query", "What to look for")
                    .optional()
                    .rest(),
            ),
            ActionSpec::new("pick-result", "Start what the launcher listed")
                .arg(ArgSpec::string("id", "all or new")),
        ]
    }

    fn contributions(&self) -> Vec<ContributionSpec> {
        vec![
            ContributionSpec::new("launcher", "provider", "tour", "", "Tour").options(json!({
                "search": "search",
                "pick": "pick-result",
            })),
        ]
    }

    fn run(self: Box<Self>, mut ctx: ModuleCtx) -> BoxFuture<'static, Result<(), ModuleError>> {
        Box::pin(async move {
            let path = Saved::path();
            let mut tour = State {
                settings: ctx.settings()?,
                saved: path.as_deref().map(Saved::load).unwrap_or_default(),
                path,
                ..State::default()
            };
            let offer_at = Instant::now() + OFFER_AFTER;
            let mut offered = false;
            loop {
                let due = tour.run.as_ref().and_then(|run| run.due);
                tokio::select! {
                    event = ctx.next_event() => match event {
                        None => {
                            // Mochi is stopping: carry on next time.
                            tour.finish(&ctx, true).await;
                            return Ok(());
                        }
                        Some(ModuleEvent::Command(command)) => tour.command(&ctx, command).await,
                        Some(ModuleEvent::Offers(offers)) => tour.offers = offers,
                        Some(ModuleEvent::Ended { activity, .. }) => tour.ended(&ctx, activity).await,
                        Some(_) => {}
                    },
                    () = sleep_until(Some(offer_at)), if !offered => {
                        offered = true;
                        tour.offer(&ctx);
                    }
                    () = sleep_until(due) => tour.step_by(&ctx, 1).await,
                }
            }
        })
    }
}

async fn sleep_until(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline.into()).await,
        None => std::future::pending().await,
    }
}

/// A tour running.
#[derive(Debug)]
struct Run {
    steps: Vec<Step>,
    index: usize,
    /// A tour of what's new.
    news: bool,
    stage: ActivityId,
    bubble: Option<BubbleId>,
    /// Asking whether to stop: it waits.
    confirming: bool,
    /// When it moves on by itself.
    due: Option<Instant>,
    /// The step tried options through the preview layer.
    previewing: bool,
}

#[derive(Debug, Default)]
struct State {
    settings: Settings,
    saved: Saved,
    path: Option<PathBuf>,
    offers: Vec<Contribution>,
    /// The notice offering the tour, while it shows.
    offer: Option<(ActivityId, Offer)>,
    run: Option<Run>,
}

impl State {
    async fn command(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        let result = match command.action.as_str() {
            "start" | "pick-result" => {
                let which = command
                    .args
                    .str("tour")
                    .or_else(|| command.args.str("id"))
                    .map(str::to_owned);
                self.start(ctx, which.as_deref()).await
            }
            "next" => {
                self.step_by(ctx, 1).await;
                Ok(())
            }
            "back" => {
                self.step_by(ctx, -1).await;
                Ok(())
            }
            "stop" => self.confirm(ctx, true),
            "continue" => self.confirm(ctx, false),
            "end" => {
                self.finish(ctx, true).await;
                Ok(())
            }
            "later" => {
                self.close_offer(ctx);
                Ok(())
            }
            "never" => {
                self.close_offer(ctx);
                self.saved.never = true;
                self.save();
                Ok(())
            }
            "search" => {
                let lines: Vec<String> = self
                    .search(command.args.str("query").unwrap_or_default())
                    .iter()
                    .map(Value::to_string)
                    .collect();
                command.answer(Ok(lines.join("\n")));
                return;
            }
            other => Err(format!("tour has no action {other}")),
        };
        command.reply(result);
    }

    fn save(&self) {
        if let Some(path) = &self.path {
            self.saved.save(path);
        }
    }

    /// Offers the tour, when there's one to offer.
    fn offer(&mut self, ctx: &ModuleCtx) {
        let Some(offer) = state::offer(
            &self.saved,
            self.settings.install,
            self.settings.updates,
            Version::current(),
        ) else {
            return;
        };
        let (kind, seen) = match offer {
            Offer::Whole => ("all", String::new()),
            Offer::News(seen) => {
                // Nothing to show since then, nothing to offer.
                if steps::build(&self.offers, &[], Some(seen)).is_empty() {
                    self.saved.seen = Version::current().to_string();
                    self.save();
                    return;
                }
                ("new", seen.to_string())
            }
        };
        let spec = ActivitySpec::new("Offer")
            .key("offer")
            .priority(Priority::HIGH)
            .uninterruptible()
            .payload(json!({
                "tour": kind,
                "seen": seen,
                "version": Version::current().to_string(),
            }));
        self.offer = Some((ctx.present(spec), offer));
    }

    fn close_offer(&mut self, ctx: &ModuleCtx) {
        if let Some((id, _)) = self.offer.take() {
            ctx.withdraw(id);
        }
    }

    /// The modules there are, from the settings, for the cards of the ones
    /// that are off.
    async fn known(ctx: &ModuleCtx) -> Vec<Known> {
        let Ok(snapshot) = ctx.settings_op(SettingsOp::Snapshot).await else {
            return Vec::new();
        };
        let sections = snapshot["sections"].as_array().cloned().unwrap_or_default();
        snapshot["modules"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|module| {
                let id = module["id"].as_str().unwrap_or_default().to_owned();
                let description = sections
                    .iter()
                    .find(|section| section["module"] == id)
                    .and_then(|section| section["description"].as_str())
                    .unwrap_or_default()
                    .to_owned();
                Known {
                    title: module["title"].as_str().unwrap_or(&id).to_owned(),
                    icon: module["icon"].as_str().unwrap_or_default().to_owned(),
                    enabled: module["enabled"] == true,
                    description,
                    id,
                }
            })
            .collect()
    }

    /// Starts the tour: `all`, `new`, or by default the one offered, or
    /// where a stopped tour was.
    async fn start(&mut self, ctx: &ModuleCtx, which: Option<&str>) -> Result<(), String> {
        if self.run.is_some() {
            return Ok(());
        }
        let offered = self.offer.as_ref().map(|(_, offer)| *offer);
        self.close_offer(ctx);
        let news = match which {
            Some("all") => None,
            Some("new") => Some(self.saved.seen().unwrap_or_default()),
            _ => match offered {
                Some(Offer::News(seen)) => Some(seen),
                Some(Offer::Whole) => None,
                None if self.saved.resume.is_some() && self.saved.resume_news => self.saved.seen(),
                None => None,
            },
        };
        let known = Self::known(ctx).await;
        let steps = steps::build(&self.offers, &known, news);
        if steps.is_empty() {
            return Err("nothing new since the last tour".into());
        }
        // A stopped tour of the same kind carries on where it was.
        let index = match (&self.saved.resume, which) {
            (Some(id), None) if self.saved.resume_news == news.is_some() => steps
                .iter()
                .position(|step| &step.id == id)
                .unwrap_or_default(),
            _ => 0,
        };
        tracing::info!(
            steps = steps.len(),
            news = news.is_some(),
            "starting the tour"
        );
        ctx.close_other_panels();
        ctx.pause_island(true);
        let spec = ActivitySpec::new("Stage")
            .key("stage")
            .priority(Priority::TOP)
            .uninterruptible()
            .overlay("Dim")
            .payload(Value::Null);
        let stage = ctx.present(spec);
        self.run = Some(Run {
            steps,
            index,
            news: news.is_some(),
            stage,
            bubble: None,
            confirming: false,
            due: None,
            previewing: false,
        });
        self.show(ctx).await;
        Ok(())
    }

    /// Moves `by` steps; past the last one, the tour ends.
    async fn step_by(&mut self, ctx: &ModuleCtx, by: isize) {
        let Some(run) = &mut self.run else {
            return;
        };
        if run.confirming {
            return;
        }
        let next = run.index as isize + by;
        if next >= run.steps.len() as isize {
            self.finish(ctx, false).await;
            return;
        }
        run.index = next.max(0) as usize;
        self.show(ctx).await;
    }

    /// Shows the current step: its view, its bubble, the look it tries.
    async fn show(&mut self, ctx: &ModuleCtx) {
        let seconds = self.settings.seconds_per_step;
        let Some(run) = &mut self.run else {
            return;
        };
        let step = run.steps[run.index].clone();

        if let Some(bubble) = run.bubble.take() {
            ctx.hide_bubble(bubble);
        }
        if step.place == "bubble" {
            let area = step.payload["area"].as_str().unwrap_or("center-right");
            let area = serde_json::from_value(json!(area)).unwrap_or(Area::CenterRight);
            run.bubble = Some(ctx.show_bubble(
                BubbleSpec::new("Bubble").key("step").area(area).payload(
                    json!({ "module": step.module, "view": step.view, "payload": step.payload }),
                ),
            ));
        }

        let op = if !step.preview.is_empty() {
            run.previewing = true;
            Some(SettingsOp::Preview {
                values: step.preview.clone(),
                replace: true,
            })
        } else if run.previewing {
            run.previewing = false;
            Some(SettingsOp::Drop)
        } else {
            None
        };
        run.due = Some(Instant::now() + Duration::from_secs(seconds));
        ctx.update(run.stage, run.payload(seconds));
        if let Some(op) = op
            && let Err(error) = ctx.settings_op(op).await
        {
            tracing::warn!(%error, step = %step.id, "can't show this look");
        }
    }

    fn confirm(&mut self, ctx: &ModuleCtx, asking: bool) -> Result<(), String> {
        let seconds = self.settings.seconds_per_step;
        let run = self.run.as_mut().ok_or("no tour is running")?;
        run.confirming = asking;
        run.due = (!asking).then(|| Instant::now() + Duration::from_secs(seconds));
        ctx.update(run.stage, run.payload(seconds));
        Ok(())
    }

    /// Ends the tour: the island and the look come back. `stopped` keeps
    /// the step to carry on from next time.
    async fn finish(&mut self, ctx: &ModuleCtx, stopped: bool) {
        let Some(run) = self.run.take() else {
            return;
        };
        ctx.withdraw(run.stage);
        if let Some(bubble) = run.bubble {
            ctx.hide_bubble(bubble);
        }
        if run.previewing
            && let Err(error) = ctx.settings_op(SettingsOp::Drop).await
        {
            tracing::warn!(%error, "can't put the look back");
        }
        ctx.pause_island(false);
        let at_end = run.index + 1 >= run.steps.len();
        self.saved.resume = (stopped && !at_end).then(|| run.steps[run.index].id.clone());
        self.saved.resume_news = run.news;
        if !run.news || !stopped {
            self.saved.seen = Version::current().to_string();
        }
        self.save();
        tracing::info!(stopped, "the tour ended");
    }

    async fn ended(&mut self, ctx: &ModuleCtx, activity: ActivityId) {
        if self.offer.as_ref().is_some_and(|(id, _)| *id == activity) {
            // Closed without an answer: offer it again next time.
            self.offer = None;
        }
        if self.run.as_ref().is_some_and(|run| run.stage == activity) {
            // `mochi dismiss`: as Escape, then Enter.
            self.finish(ctx, true).await;
        }
    }

    fn search(&self, query: &str) -> Vec<Value> {
        let query = query.trim().to_lowercase();
        if query.len() < 2 {
            return Vec::new();
        }
        let mut out = Vec::new();
        let matches = |words: &[&str]| {
            words
                .iter()
                .any(|word| word.starts_with(&query) || query.starts_with(word))
        };
        if matches(&["tour", "welcome", "onboarding", "showcase", "help"]) {
            out.push(json!({
                "title": "Tour",
                "subtitle": "A guided tour of Mochi",
                "icon": "tour",
                "id": "all",
            }));
        }
        if matches(&["what's new", "whats new", "new", "changes", "tour"]) {
            out.push(json!({
                "title": "What's new in Mochi",
                "subtitle": format!("A tour of what changed in {}", Version::current()),
                "icon": "new_releases",
                "id": "new",
            }));
        }
        out
    }
}

impl Run {
    fn payload(&self, seconds: u64) -> Value {
        let step = &self.steps[self.index];
        json!({
            "step": step,
            "chapter": steps::chapter_title(&step.chapter),
            "index": self.index,
            "count": self.steps.len(),
            "news": self.news,
            "confirming": self.confirming,
            "seconds": seconds,
        })
    }
}

#[cfg(test)]
mod settings_example {
    #[test]
    fn shows_the_defaults() {
        mochi_core::examples::check_module::<super::Settings>(
            "tour",
            include_str!("../settings.toml"),
        );
    }
}
