use serde_json::json;

use super::*;

const SECOND: Duration = Duration::from_secs(1);

/// Builds activities with increasing ids and tracks a fake clock.
struct Bench {
    arbiter: Arbiter,
    start: Instant,
    now: Instant,
    next_id: u64,
}

impl Bench {
    fn new() -> Self {
        let start = Instant::now();
        Self {
            arbiter: Arbiter::new(),
            start,
            now: start,
            next_id: 0,
        }
    }

    fn submit(&mut self, module: &str, spec: ActivitySpec) -> ActivityId {
        self.next_id += 1;
        let id = ActivityId(self.next_id);
        self.arbiter.submit(id, module, spec, self.now);
        id
    }

    fn advance(&mut self, by: Duration) {
        self.now += by;
        self.arbiter.tick(self.now);
    }

    fn at(&self, seconds: u64) -> Instant {
        self.start + Duration::from_secs(seconds)
    }

    fn shown(&self) -> Option<ActivityId> {
        self.arbiter.shown().map(|activity| activity.id)
    }

    fn ended(&mut self) -> Vec<(ActivityId, EndReason)> {
        self.arbiter
            .take_effects()
            .into_iter()
            .filter_map(|effect| match effect {
                Effect::Ended {
                    activity, reason, ..
                } => Some((activity, reason)),
                _ => None,
            })
            .collect()
    }
}

fn idle() -> ActivitySpec {
    ActivitySpec::new("Pill").priority(Priority::IDLE)
}

fn timed(seconds: u64) -> ActivitySpec {
    ActivitySpec::new("Card").timeout(Duration::from_secs(seconds))
}

#[test]
fn empty_arbiter_shows_nothing() {
    let mut bench = Bench::new();
    assert_eq!(bench.shown(), None);
    assert_eq!(bench.arbiter.next_deadline(), None);
    assert!(bench.arbiter.take_effects().is_empty());
}

#[test]
fn first_activity_is_shown_and_its_timer_starts() {
    let mut bench = Bench::new();
    let card = bench.submit("notifications", timed(4));

    assert_eq!(bench.shown(), Some(card));
    assert_eq!(bench.arbiter.next_deadline(), Some(bench.at(4)));
}

#[test]
fn higher_priority_interrupts_and_the_interrupted_one_resumes() {
    let mut bench = Bench::new();
    let card = bench.submit("notifications", timed(4));
    bench.advance(SECOND);

    let osd = bench.submit("osd", timed(2).priority(Priority::HIGH));
    assert_eq!(bench.shown(), Some(osd));

    bench.advance(2 * SECOND);
    assert_eq!(bench.shown(), Some(card));
    // The card's timer was paused for the 2s the OSD was on screen.
    assert_eq!(bench.arbiter.next_deadline(), Some(bench.at(6)));
}

#[test]
fn lower_priority_waits_for_the_shown_one() {
    let mut bench = Bench::new();
    let osd = bench.submit("osd", timed(2).priority(Priority::HIGH));
    let card = bench.submit("notifications", timed(4));

    assert_eq!(bench.shown(), Some(osd));
    bench.advance(2 * SECOND);
    assert_eq!(bench.shown(), Some(card));
    // Its timer only started when it appeared.
    assert_eq!(bench.arbiter.next_deadline(), Some(bench.at(6)));
}

#[test]
fn same_priority_queues_by_default() {
    let mut bench = Bench::new();
    let first = bench.submit("notifications", timed(4));
    let second = bench.submit("notifications", timed(4));

    assert_eq!(bench.shown(), Some(first));
    bench.advance(4 * SECOND);
    assert_eq!(bench.shown(), Some(second));
}

#[test]
fn same_priority_with_stack_interrupts() {
    let mut bench = Bench::new();
    let card = bench.submit("notifications", timed(4));
    let bluetooth = bench.submit("bluetooth", timed(2).same_priority(SamePriority::Stack));

    assert_eq!(bench.shown(), Some(bluetooth));
    bench.advance(2 * SECOND);
    assert_eq!(bench.shown(), Some(card));
}

#[test]
fn queue_keeps_priority_then_arrival_order() {
    let mut bench = Bench::new();
    let _shown = bench.submit("osd", timed(1).priority(Priority::URGENT));
    let low = bench.submit("a", timed(1).priority(Priority::LOW));
    let normal_first = bench.submit("b", timed(1));
    let high = bench.submit("c", timed(1).priority(Priority::HIGH));
    let normal_second = bench.submit("d", timed(1));

    let mut order = Vec::new();
    for _ in 0..4 {
        bench.advance(SECOND);
        order.push(bench.shown().unwrap());
    }
    assert_eq!(order, [high, normal_first, normal_second, low]);
}

#[test]
fn uninterruptible_activities_make_everyone_wait() {
    let mut bench = Bench::new();
    let pill = bench.submit("idle", idle());
    let card = bench.submit("notifications", timed(4).uninterruptible());
    let urgent = bench.submit("power", timed(2).priority(Priority::URGENT));

    assert_eq!(bench.shown(), Some(card));
    bench.advance(4 * SECOND);
    // The urgent one beats the suspended idle pill.
    assert_eq!(bench.shown(), Some(urgent));
    bench.advance(2 * SECOND);
    assert_eq!(bench.shown(), Some(pill));
}

#[test]
fn suspended_activities_win_ties_against_queued_ones() {
    let mut bench = Bench::new();
    let card = bench.submit("notifications", timed(4));
    let _osd = bench.submit("osd", timed(1).priority(Priority::HIGH));
    let other = bench.submit("notifications", timed(4));

    bench.advance(SECOND);
    assert_eq!(bench.shown(), Some(card));
    bench.advance(4 * SECOND);
    assert_eq!(bench.shown(), Some(other));
}

#[test]
fn idle_comes_back_when_everything_ends() {
    let mut bench = Bench::new();
    let pill = bench.submit("idle", idle());
    let card = bench.submit("notifications", timed(4));

    assert_eq!(bench.shown(), Some(card));
    bench.advance(4 * SECOND);
    assert_eq!(bench.shown(), Some(pill));
    assert_eq!(bench.arbiter.next_deadline(), None);
}

#[test]
fn same_key_replaces_in_place() {
    let mut bench = Bench::new();
    let first = bench.submit(
        "osd",
        timed(2).key("volume").payload(json!({ "level": 40 })),
    );
    bench.ended();
    bench.advance(SECOND);

    let second = bench.submit(
        "osd",
        timed(2).key("volume").payload(json!({ "level": 45 })),
    );

    assert_eq!(bench.shown(), Some(second));
    let shown = bench.arbiter.shown().unwrap();
    assert_eq!(shown.payload, json!({ "level": 45 }));
    // The UI uses the key to update the bar in place instead of switching
    // views.
    assert_eq!(shown.key.as_deref(), Some("volume"));
    // A fresh 2s from now, not what was left of the first one.
    assert_eq!(bench.arbiter.next_deadline(), Some(bench.at(3)));
    assert_eq!(bench.ended(), [(first, EndReason::Replaced)]);
}

#[test]
fn replacing_a_queued_activity_keeps_it_queued() {
    let mut bench = Bench::new();
    let shown = bench.submit("osd", timed(2).priority(Priority::HIGH));
    let _track = bench.submit("spotify", timed(4).key("track"));
    let next_track = bench.submit("spotify", timed(4).key("track"));

    assert_eq!(bench.shown(), Some(shown));
    bench.advance(2 * SECOND);
    assert_eq!(bench.shown(), Some(next_track));
    bench.advance(4 * SECOND);
    assert_eq!(bench.shown(), None);
}

#[test]
fn keys_only_match_within_a_module() {
    let mut bench = Bench::new();
    let first = bench.submit("osd", timed(2).key("main"));
    let other = bench.submit("spotify", timed(2).key("main"));

    assert_eq!(bench.shown(), Some(first));
    bench.advance(2 * SECOND);
    assert_eq!(bench.shown(), Some(other));
}

#[test]
fn hover_pauses_the_timer_and_leaving_gives_at_least_a_second() {
    let mut bench = Bench::new();
    let card = bench.submit("notifications", timed(4));

    bench.now = bench.at(3);
    bench.arbiter.hover(card, true, bench.now);
    assert_eq!(bench.arbiter.next_deadline(), None);

    bench.advance(60 * SECOND);
    assert_eq!(bench.shown(), Some(card));

    bench.arbiter.hover(card, false, bench.now);
    // One second was left, which is also the minimum.
    assert_eq!(bench.arbiter.next_deadline(), Some(bench.at(64)));
}

#[test]
fn leaving_just_before_the_deadline_still_gives_a_second() {
    let mut bench = Bench::new();
    let card = bench.submit("notifications", timed(4));

    bench.now = bench.at(4) - Duration::from_millis(10);
    bench.arbiter.hover(card, true, bench.now);
    bench.now = bench.at(4);
    bench.arbiter.hover(card, false, bench.now);
    assert_eq!(bench.arbiter.next_deadline(), Some(bench.at(5)));
}

#[test]
fn resuming_after_an_interruption_gives_at_least_a_second() {
    let mut bench = Bench::new();
    let card = bench.submit("notifications", timed(4));
    bench.now = bench.at(4) - Duration::from_millis(100);
    bench.submit("osd", timed(1).priority(Priority::HIGH));

    bench.advance(SECOND);
    assert_eq!(bench.shown(), Some(card));
    assert_eq!(bench.arbiter.next_deadline(), Some(bench.now + SECOND));
}

#[test]
fn clicking_an_expandable_activity_toggles_its_view() {
    let mut bench = Bench::new();
    let card = bench.submit("notifications", timed(4).expanded("Details"));
    bench.arbiter.take_effects();

    bench.arbiter.click(card, bench.now);
    let shown = bench.arbiter.shown().unwrap();
    assert_eq!((shown.view.as_str(), shown.expanded), ("Details", true));
    assert!(matches!(
        bench.arbiter.take_effects().as_slice(),
        [Effect::Present(Some(_))]
    ));

    bench.arbiter.click(card, bench.now);
    let shown = bench.arbiter.shown().unwrap();
    assert_eq!((shown.view.as_str(), shown.expanded), ("Card", false));
}

#[test]
fn expanded_activities_never_time_out() {
    let mut bench = Bench::new();
    let card = bench.submit("notifications", timed(4).expanded("Details"));
    bench.arbiter.click(card, bench.now);

    bench.advance(60 * SECOND);
    assert_eq!(bench.shown(), Some(card));

    bench.arbiter.click(card, bench.now);
    assert_eq!(bench.arbiter.next_deadline(), Some(bench.now + 4 * SECOND));
}

#[test]
fn interrupted_activities_come_back_collapsed() {
    let mut bench = Bench::new();
    let card = bench.submit("notifications", timed(4).expanded("Details"));
    bench.arbiter.click(card, bench.now);
    bench.submit("osd", timed(1).priority(Priority::HIGH));

    bench.advance(SECOND);
    let shown = bench.arbiter.shown().unwrap();
    assert_eq!((shown.id, shown.expanded), (card, false));
}

fn peeking() -> ActivitySpec {
    timed(4).expanded("Details").expand_for(2 * SECOND)
}

fn expanded(bench: &Bench) -> bool {
    bench.arbiter.shown().is_some_and(|shown| shown.expanded)
}

#[test]
fn auto_expanded_activities_collapse_then_time_out() {
    let mut bench = Bench::new();
    let card = bench.submit("media", peeking());
    assert!(expanded(&bench));
    assert_eq!(bench.arbiter.next_deadline(), Some(bench.at(2)));

    bench.advance(2 * SECOND);
    assert!(!expanded(&bench));
    assert_eq!(bench.arbiter.next_deadline(), Some(bench.at(6)));

    bench.advance(4 * SECOND);
    assert_eq!(bench.ended(), [(card, EndReason::Expired)]);
}

#[test]
fn hovering_keeps_an_auto_expanded_activity_open() {
    let mut bench = Bench::new();
    let card = bench.submit("media", peeking());
    bench.advance(SECOND);
    bench.arbiter.hover(card, true, bench.now);
    assert_eq!(bench.arbiter.next_deadline(), None);

    bench.advance(60 * SECOND);
    assert!(expanded(&bench));

    // One second was left, which is also the minimum.
    bench.arbiter.hover(card, false, bench.now);
    assert_eq!(bench.arbiter.next_deadline(), Some(bench.at(62)));
}

#[test]
fn a_click_takes_over_from_auto_expansion() {
    let mut bench = Bench::new();
    let card = bench.submit("media", peeking());

    bench.arbiter.click(card, bench.now);
    assert!(!expanded(&bench));
    assert_eq!(bench.arbiter.next_deadline(), Some(bench.at(4)));

    bench.arbiter.click(card, bench.now);
    assert!(expanded(&bench));
    assert_eq!(bench.arbiter.next_deadline(), None);
}

#[test]
fn keyed_replacements_expand_again_only_when_asked() {
    let mut bench = Bench::new();
    let compact = ActivitySpec::new("Card").expanded("Details").key("media");
    bench.submit("media", compact.clone());
    assert!(!expanded(&bench));

    bench.submit("media", compact.clone().expand_for(2 * SECOND));
    assert!(expanded(&bench));

    // A plain update keeps the view open and its time running.
    bench.advance(SECOND);
    bench.submit("media", compact.clone());
    assert!(expanded(&bench));
    assert_eq!(bench.arbiter.next_deadline(), Some(bench.at(2)));

    bench.advance(SECOND);
    assert!(!expanded(&bench));
}

#[test]
fn views_the_user_expanded_stay_expanded() {
    let mut bench = Bench::new();
    let compact = ActivitySpec::new("Card").expanded("Details").key("media");
    let card = bench.submit("media", compact.clone());
    bench.arbiter.click(card, bench.now);

    bench.submit("media", compact.expand_for(2 * SECOND));
    assert!(expanded(&bench));
    assert_eq!(bench.arbiter.next_deadline(), None);
}

#[test]
fn waiting_activities_expand_when_they_show() {
    let mut bench = Bench::new();
    bench.submit("osd", timed(1).priority(Priority::HIGH));
    let card = bench.submit("media", peeking());
    assert!(!expanded(&bench));

    bench.advance(SECOND);
    assert_eq!(bench.shown(), Some(card));
    assert!(expanded(&bench));
    assert_eq!(bench.arbiter.next_deadline(), Some(bench.at(3)));
}

#[test]
fn clicks_without_an_expanded_view_go_to_the_module() {
    let mut bench = Bench::new();
    let pill = bench.submit("idle", idle());
    bench.arbiter.take_effects();

    bench.arbiter.click(pill, bench.now);
    assert_eq!(
        bench.arbiter.take_effects(),
        [Effect::Clicked {
            module: "idle".into(),
            activity: pill
        }]
    );
}

#[test]
fn dismissing_ends_the_activity_and_shows_the_next() {
    let mut bench = Bench::new();
    let pill = bench.submit("idle", idle());
    let card = bench.submit("notifications", timed(4));
    bench.ended();

    bench.arbiter.dismiss(card, bench.now);
    assert_eq!(bench.shown(), Some(pill));
    assert_eq!(bench.ended(), [(card, EndReason::Dismissed)]);
}

#[test]
fn events_for_activities_no_longer_shown_are_ignored() {
    let mut bench = Bench::new();
    let card = bench.submit("notifications", timed(4).expanded("Details"));
    let osd = bench.submit("osd", timed(1).priority(Priority::HIGH));
    bench.arbiter.take_effects();

    bench.arbiter.click(card, bench.now);
    bench.arbiter.hover(card, true, bench.now);
    bench.arbiter.dismiss(card, bench.now);

    assert_eq!(bench.shown(), Some(osd));
    assert!(bench.arbiter.take_effects().is_empty());
    bench.advance(SECOND);
    assert!(!bench.arbiter.shown().unwrap().expanded);
}

#[test]
fn withdraw_removes_an_activity_wherever_it_is() {
    let mut bench = Bench::new();
    let card = bench.submit("notifications", timed(4));
    let osd = bench.submit("osd", timed(2).priority(Priority::HIGH));
    let queued = bench.submit("notifications", timed(4));
    bench.ended();

    bench
        .arbiter
        .withdraw("notifications", card, bench.now)
        .unwrap();
    bench
        .arbiter
        .withdraw("notifications", queued, bench.now)
        .unwrap();
    assert_eq!(
        bench.ended(),
        [(card, EndReason::Withdrawn), (queued, EndReason::Withdrawn)]
    );

    bench.arbiter.withdraw("osd", osd, bench.now).unwrap();
    assert_eq!(bench.shown(), None);
}

#[test]
fn modules_can_only_touch_their_own_activities() {
    let mut bench = Bench::new();
    let card = bench.submit("notifications", timed(4));

    assert_eq!(
        bench.arbiter.withdraw("osd", card, bench.now),
        Err(ArbiterError::NotOwner(card))
    );
    assert_eq!(
        bench.arbiter.update("osd", card, json!({})),
        Err(ArbiterError::NotOwner(card))
    );
    assert_eq!(
        bench.arbiter.withdraw("osd", ActivityId(99), bench.now),
        Err(ArbiterError::UnknownActivity(ActivityId(99)))
    );
    assert_eq!(bench.shown(), Some(card));
}

#[test]
fn withdraw_all_clears_a_module() {
    let mut bench = Bench::new();
    let pill = bench.submit("idle", idle());
    bench.submit("notifications", timed(4));
    bench.submit("osd", timed(2).priority(Priority::HIGH));
    bench.submit("notifications", timed(4));

    bench.arbiter.withdraw_all("notifications", bench.now);
    bench.arbiter.withdraw_all("osd", bench.now);
    assert_eq!(bench.shown(), Some(pill));
}

#[test]
fn updating_the_shown_payload_presents_it_again() {
    let mut bench = Bench::new();
    let track = bench.submit("spotify", ActivitySpec::new("Track"));
    let queued = bench.submit("spotify", ActivitySpec::new("Track"));
    bench.arbiter.take_effects();

    bench
        .arbiter
        .update("spotify", queued, json!({ "title": "Later" }))
        .unwrap();
    assert!(bench.arbiter.take_effects().is_empty());

    bench
        .arbiter
        .update("spotify", track, json!({ "title": "Now" }))
        .unwrap();
    let effects = bench.arbiter.take_effects();
    assert!(matches!(
        effects.as_slice(),
        [Effect::Present(Some(activity))] if activity.payload == json!({ "title": "Now" })
    ));
}

#[test]
fn present_is_reported_once_per_batch() {
    let mut bench = Bench::new();
    bench.submit("idle", idle());
    bench.submit("notifications", timed(4));
    bench.submit("osd", timed(1).priority(Priority::HIGH));

    let presents = bench
        .arbiter
        .take_effects()
        .into_iter()
        .filter(|effect| matches!(effect, Effect::Present(_)))
        .count();
    assert_eq!(presents, 1);
}

#[test]
fn the_last_activity_ending_presents_nothing() {
    let mut bench = Bench::new();
    bench.submit("notifications", timed(1));
    bench.arbiter.take_effects();

    bench.advance(SECOND);
    let effects = bench.arbiter.take_effects();
    assert!(effects.contains(&Effect::Present(None)));
}

#[test]
fn an_overlay_makes_the_activity_modal() {
    let mut bench = Bench::new();
    bench.submit("capture", ActivitySpec::new("Choose").overlay("Overlay"));
    let shown = bench.arbiter.shown().unwrap();
    assert!(shown.modal);
    assert_eq!(shown.overlay.as_deref(), Some("Overlay"));

    let mut bench = Bench::new();
    bench.submit("launcher", ActivitySpec::new("Launcher").modal());
    let shown = bench.arbiter.shown().unwrap();
    assert_eq!(shown.overlay, None);
}
