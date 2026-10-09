//! An example Mochi plugin: a focus timer.
//!
//! While it runs, a bubble counts down. When focus ends, the island says so and
//! a break starts; when the break ends, it says that too. The control center
//! gets a card with the time left and buttons, from the state this publishes.

use std::time::Duration;

use mochi_sdk::{
    ActivitySpec, Area, BubbleId, BubbleSpec, ModuleCommand, ModuleCtx, ModuleEvent, Priority,
    Value, json,
};
use serde::Deserialize;
use tokio::time::{Instant, interval_at};

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    focus_minutes: u64,
    break_minutes: u64,
    auto_break: bool,
    pause_media: bool,
    area: Area,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            focus_minutes: 25,
            break_minutes: 5,
            auto_break: true,
            pause_media: false,
            area: Area::CenterRight,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Focus,
    Break,
}

impl Phase {
    fn name(self) -> &'static str {
        match self {
            Self::Focus => "focus",
            Self::Break => "break",
        }
    }
}

/// A running timer.
#[derive(Debug)]
struct Timer {
    phase: Phase,
    total: Duration,
    left: Duration,
    paused: bool,
    bubble: BubbleId,
}

struct Pomodoro {
    settings: Settings,
    timer: Option<Timer>,
    /// Focus sessions finished since the plugin started.
    sessions: u32,
    /// Whether the media player is playing, from the media module.
    playing: bool,
}

fn main() -> std::process::ExitCode {
    mochi_sdk::run(run)
}

async fn run(mut ctx: ModuleCtx) -> Result<(), mochi_sdk::Error> {
    let settings: Settings = ctx.settings()?;
    let mut pomodoro = Pomodoro {
        settings,
        timer: None,
        sessions: 0,
        playing: false,
    };
    pomodoro.publish(&ctx);

    let second = Duration::from_secs(1);
    let mut tick = interval_at(Instant::now() + second, second);
    loop {
        tokio::select! {
            event = ctx.next_event() => match event {
                // mochid is stopping the plugin.
                None => return Ok(()),
                Some(ModuleEvent::Command(command)) => pomodoro.command(&ctx, command),
                Some(ModuleEvent::BubbleClicked(_)) => pomodoro.toggle_pause(&ctx),
                Some(ModuleEvent::State { module, state }) if module == "media" => {
                    pomodoro.playing = state["status"] == "playing";
                }
                Some(_) => {}
            },
            _ = tick.tick() => pomodoro.tick(&ctx),
        }
    }
}

impl Pomodoro {
    fn command(&mut self, ctx: &ModuleCtx, command: ModuleCommand) {
        let minutes = command
            .args
            .int("minutes")
            .map(|minutes| minutes.max(1) as u64);
        let action = command.action.clone();
        match action.as_str() {
            "start" => {
                self.start(
                    ctx,
                    Phase::Focus,
                    minutes.unwrap_or(self.settings.focus_minutes),
                );
                command.reply(Ok(()));
            }
            "break" => {
                self.start(
                    ctx,
                    Phase::Break,
                    minutes.unwrap_or(self.settings.break_minutes),
                );
                command.reply(Ok(()));
            }
            "pause" => {
                if self.timer.is_some() {
                    self.toggle_pause(ctx);
                    command.reply(Ok(()));
                } else {
                    command.reply(Err("no timer is running".into()));
                }
            }
            "stop" => {
                self.stop(ctx);
                command.reply(Ok(()));
            }
            "status" => command.answer(Ok(self.describe())),
            other => command.reply(Err(format!("no action {other}"))),
        }
    }

    fn start(&mut self, ctx: &ModuleCtx, phase: Phase, minutes: u64) {
        self.stop(ctx);
        let total = Duration::from_secs(minutes * 60);
        let bubble = ctx.show_bubble(
            BubbleSpec::new("Bubble")
                .key("timer")
                .area(self.settings.area)
                .priority(Priority::LOW)
                .payload(self.bubble_payload(phase, total, total, false))
                .news(),
        );
        self.timer = Some(Timer {
            phase,
            total,
            left: total,
            paused: false,
            bubble,
        });
        if phase == Phase::Focus && self.settings.pause_media && self.playing {
            let call = ctx.call("media", "pause", &[]);
            tokio::spawn(async move {
                // Not playing anymore, or no media module: fine either way.
                let _ = call.await;
            });
        }
        self.publish(ctx);
    }

    fn stop(&mut self, ctx: &ModuleCtx) {
        if let Some(timer) = self.timer.take() {
            ctx.hide_bubble(timer.bubble);
        }
        self.publish(ctx);
    }

    fn toggle_pause(&mut self, ctx: &ModuleCtx) {
        let Some(timer) = &mut self.timer else {
            return;
        };
        timer.paused = !timer.paused;
        self.refresh(ctx);
    }

    fn tick(&mut self, ctx: &ModuleCtx) {
        let Some(timer) = &mut self.timer else {
            return;
        };
        if timer.paused {
            return;
        }
        timer.left = timer.left.saturating_sub(Duration::from_secs(1));
        if !timer.left.is_zero() {
            self.refresh(ctx);
            return;
        }

        let finished = timer.phase;
        self.stop(ctx);
        let text = match finished {
            Phase::Focus => {
                self.sessions += 1;
                if self.settings.auto_break {
                    self.start(ctx, Phase::Break, self.settings.break_minutes);
                    format!(
                        "Focus done. {} minutes of rest",
                        self.settings.break_minutes
                    )
                } else {
                    "Focus done. Time for a break".to_owned()
                }
            }
            Phase::Break => "Break over. Back to it".to_owned(),
        };
        ctx.present(
            ActivitySpec::new("Done")
                .priority(Priority::HIGH)
                .timeout(Duration::from_secs(6))
                .payload(json!({ "text": text, "phase": finished.name() })),
        );
    }

    /// After the time or the pause changed.
    fn refresh(&self, ctx: &ModuleCtx) {
        if let Some(timer) = &self.timer {
            let payload = self.bubble_payload(timer.phase, timer.left, timer.total, timer.paused);
            ctx.update_bubble(timer.bubble, payload);
        }
        self.publish(ctx);
    }

    fn bubble_payload(&self, phase: Phase, left: Duration, total: Duration, paused: bool) -> Value {
        json!({
            "phase": phase.name(),
            "left": left.as_secs(),
            "total": total.as_secs(),
            "paused": paused,
        })
    }

    /// The state the control center card shows.
    fn publish(&self, ctx: &ModuleCtx) {
        let state = match &self.timer {
            Some(timer) => json!({
                "phase": timer.phase.name(),
                "left": timer.left.as_secs(),
                "total": timer.total.as_secs(),
                "paused": timer.paused,
                "sessions": self.sessions,
            }),
            None => json!({
                "phase": "idle",
                "focus_minutes": self.settings.focus_minutes,
                "sessions": self.sessions,
            }),
        };
        ctx.publish_state(state);
    }

    fn describe(&self) -> String {
        match &self.timer {
            Some(timer) => {
                let left = timer.left.as_secs();
                let paused = if timer.paused { ", paused" } else { "" };
                format!(
                    "{} {}:{:02} left{paused}",
                    timer.phase.name(),
                    left / 60,
                    left % 60
                )
            }
            None => format!("idle, {} sessions done", self.sessions),
        }
    }
}
