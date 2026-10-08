//! The tour's steps. Each module offers its own as contributions to the
//! tour, with the view to show, made-up data for it, a caption and the
//! release that brought it:
//!
//! ```ignore
//! ContributionSpec::new("tour", "step", "player", "Expanded", "Now playing")
//!     .order(10)
//!     .options(json!({
//!         "chapter": "notices",
//!         "since": "0.0.1",
//!         "caption": "What's playing, from any player.",
//!         "payload": { "title": "Teardrop", ... },
//!     }))
//! ```
//!
//! `place` says where it shows: on the island (the default), as a
//! `bubble`, or framed like a hub `card` or a desktop `widget`, `size`
//! pixels wide and tall. `properties` are more the view takes, like a
//! widget's `settings`. The tour
//! adds its own: a welcome, the island's looks, a card for each module
//! that's off, and the end.

use std::cmp::Ordering;
use std::fmt;

use mochi_core::Contribution;
use serde::Serialize;
use serde_json::{Value, json};

/// A release, like `0.0.7`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Version(pub u32, pub u32, pub u32);

impl Version {
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = text.trim().trim_start_matches('v').split('.');
        let mut next = || parts.next()?.parse().ok();
        Some(Self(next()?, next()?, next().unwrap_or(0)))
    }

    /// This build's.
    pub fn current() -> Self {
        Self::parse(env!("CARGO_PKG_VERSION")).expect("the crate version parses")
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// The tour's chapters, in order, with their titles.
pub const CHAPTERS: [(&str, &str); 8] = [
    ("welcome", "Welcome"),
    ("island", "The island"),
    ("panels", "Panels"),
    ("notices", "Notices"),
    ("desktop", "The desktop"),
    ("capture", "Capture and share"),
    ("settings", "Make it yours"),
    ("end", "That's Mochi"),
];

fn chapter_index(chapter: &str) -> usize {
    CHAPTERS
        .iter()
        .position(|(id, _)| *id == chapter)
        .unwrap_or(CHAPTERS.len() - 2)
}

/// One step of the tour.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Step {
    /// `module/id`.
    pub id: String,
    /// Whose view shows; the tour's own for text cards.
    pub module: String,
    /// The view's file name, empty for a text card.
    pub view: String,
    pub title: String,
    pub icon: String,
    pub caption: String,
    pub chapter: String,
    #[serde(skip)]
    pub order: i32,
    #[serde(skip)]
    pub since: Version,
    /// `island`, `bubble`, `card`, `widget` or `text`.
    pub place: String,
    pub payload: Value,
    /// Options the step tries while it shows, by path: a look of the
    /// island, say. Never kept. With several frames, the step cycles
    /// through them.
    #[serde(skip)]
    pub frames: Vec<Vec<(String, Value)>>,
    /// A card's or a widget's size, `[width, height]` in pixels.
    pub size: Option<[u32; 2]>,
    /// More properties the view takes, like a widget's `settings`.
    pub properties: Value,
}

impl Step {
    fn text(
        id: &str,
        chapter: &str,
        since: Version,
        title: &str,
        icon: &str,
        caption: &str,
    ) -> Self {
        Self {
            id: format!("tour/{id}"),
            module: "tour".to_owned(),
            view: String::new(),
            title: title.to_owned(),
            icon: icon.to_owned(),
            caption: caption.to_owned(),
            chapter: chapter.to_owned(),
            order: 0,
            since,
            place: "text".to_owned(),
            payload: Value::Null,
            frames: Vec::new(),
            size: None,
            properties: Value::Null,
        }
    }

    /// A step a module offers. `None` for one without a caption or with a
    /// release that doesn't parse: those are bugs in the module, logged.
    pub fn offered(offer: &Contribution) -> Option<Self> {
        let options = &offer.options;
        let text = |key: &str| options.get(key).and_then(Value::as_str).unwrap_or("");
        let since = Version::parse(text("since"));
        let caption = text("caption");
        if since.is_none() || caption.is_empty() {
            tracing::warn!(module = %offer.module, id = %offer.id, "a tour step needs a caption and a `since`");
            return None;
        }
        let size = options.get("size").and_then(|size| {
            let size = size.as_array()?;
            Some([
                size.first()?.as_u64()? as u32,
                size.get(1)?.as_u64()? as u32,
            ])
        });
        Some(Self {
            id: format!("{}/{}", offer.module, offer.id),
            module: offer.module.clone(),
            view: offer.view.clone(),
            title: offer.title.clone(),
            icon: offer.icon.clone().unwrap_or_default(),
            caption: caption.to_owned(),
            chapter: Some(text("chapter"))
                .filter(|chapter| !chapter.is_empty())
                .unwrap_or("panels")
                .to_owned(),
            order: offer.order,
            since: since?,
            place: Some(text("place"))
                .filter(|place| !place.is_empty())
                .unwrap_or("island")
                .to_owned(),
            payload: options.get("payload").cloned().unwrap_or(Value::Null),
            frames: Vec::new(),
            size,
            properties: options.get("properties").cloned().unwrap_or(Value::Null),
        })
    }
}

/// A module the settings know, for the card a module that's off gets.
#[derive(Debug, Clone, PartialEq)]
pub struct Known {
    pub id: String,
    pub title: String,
    pub icon: String,
    pub description: String,
    pub enabled: bool,
}

/// The release each builtin module came with, for the card of one that's
/// off in a tour of what's new.
fn arrived(module: &str) -> Version {
    match module {
        "capture" => Version(0, 0, 2),
        "clipboard" | "share" => Version(0, 0, 4),
        "audio" | "battery" | "bluetooth" | "network" | "performance" | "tray" => Version(0, 0, 5),
        "colors" | "emoji" | "notes" | "widgets" => Version(0, 0, 6),
        "settings" => Version(0, 0, 7),
        "tour" => Version(0, 0, 8),
        _ => Version(0, 0, 1),
    }
}

impl Step {
    /// How long it shows, from the tour's pace: cards of the tour's own
    /// are quick to read, a module's view gets the full time.
    pub fn seconds(&self, pace: f64) -> f64 {
        match self.place.as_str() {
            "text" | "off" | "look" => pace * 0.6,
            _ => pace,
        }
    }
}

/// The island's looks, tried for a moment each.
fn looks() -> Vec<Step> {
    let look = |id: &str,
                since: Version,
                title: &str,
                icon: &str,
                caption: &str,
                frames: Vec<Vec<(&str, Value)>>| {
        let mut step = Step::text(id, "island", since, title, icon, caption);
        step.place = "look".to_owned();
        step.frames = frames
            .into_iter()
            .map(|frame| {
                frame
                    .into_iter()
                    .map(|(path, value)| (path.to_owned(), value))
                    .collect()
            })
            .collect();
        step
    };
    let accents = ["#30d158", "#0a84ff", "#bf5af2", "#ff375f", "#ff9f0a"];
    vec![
        look(
            "notch",
            Version(0, 0, 1),
            "Notch",
            "crop_16_9",
            "Island or notch: the notch sits against the edge with curved ears. Settings › Layout › Mode.",
            vec![vec![("theme.layout.mode", json!("notch"))]],
        ),
        look(
            "bottom",
            Version(0, 0, 1),
            "Top or bottom",
            "vertical_align_bottom",
            "Everything can sit at the bottom edge instead. Settings › Layout › Anchor.",
            vec![vec![("theme.layout.anchor", json!("bottom"))]],
        ),
        look(
            "accent",
            Version(0, 0, 1),
            "Colors",
            "palette",
            "Every color is yours to pick, live, with the accent used sparingly. Settings › Colors.",
            accents
                .iter()
                .map(|accent| vec![("theme.colors.accent", json!(accent))])
                .collect(),
        ),
        look(
            "border",
            Version(0, 0, 4),
            "Border and shadow",
            "border_style",
            "A hairline around the island and the bubbles, and a soft shadow under them, in any color, or none.",
            accents
                .iter()
                .map(|accent| {
                    let rgb = accent.trim_start_matches('#');
                    vec![
                        ("theme.colors.border", json!(format!("#ff{rgb}"))),
                        ("theme.colors.shadow", json!(format!("#99{rgb}"))),
                    ]
                })
                .collect(),
        ),
        look(
            "stack",
            Version(0, 0, 5),
            "Stacked bubbles",
            "bubble_chart",
            "Bubbles can stack, the most important in front; hovering fans them out. Settings › Bubbles.",
            vec![vec![("config.bubbles.stack", json!(true))]],
        ),
    ]
}

/// Every step, in order. `since` keeps only what came after a release,
/// for a tour of what's new; `None` is the whole tour.
pub fn build(offers: &[Contribution], known: &[Known], since: Option<Version>) -> Vec<Step> {
    let mut steps = vec![Step::text(
        "welcome",
        "welcome",
        Version(0, 0, 1),
        "Welcome to Mochi",
        "waving_hand",
        "A minute or two through what Mochi does, with made-up data. Space goes on, ← goes back, Esc stops.",
    )];
    steps.extend(looks());
    steps.extend(
        offers
            .iter()
            .filter(|offer| offer.kind == "step")
            .filter_map(Step::offered),
    );
    for module in known
        .iter()
        .filter(|module| !module.enabled && module.id != "tour")
    {
        let sentence = module
            .description
            .split_inclusive(". ")
            .next()
            .unwrap_or_default()
            .trim();
        let mut step = Step::text(
            &format!("off/{}", module.id),
            "settings",
            arrived(&module.id),
            &module.title,
            &module.icon,
            format!("{sentence} It's off: turn it on in Settings › Modules.").trim(),
        );
        step.place = "off".to_owned();
        step.order = 50;
        steps.push(step);
    }
    steps.push(Step::text(
        "end",
        "end",
        Version(0, 0, 1),
        "That's Mochi",
        "check_circle",
        "Run the tour again from Settings › Tour, or search \"tour\" in the launcher.",
    ));

    if let Some(seen) = since {
        let news: Vec<Step> = steps
            .iter()
            .filter(|step| step.since > seen && step.chapter != "welcome" && step.chapter != "end")
            .cloned()
            .collect();
        if news.is_empty() {
            return Vec::new();
        }
        let mut welcome = Step::text(
            "whats-new",
            "welcome",
            Version::current(),
            &format!("New in Mochi {}", Version::current()),
            "new_releases",
            &format!("What changed since {seen}. Space goes on, ← goes back, Esc stops."),
        );
        welcome.order = -1;
        let end = steps.last().cloned().expect("ends with the end");
        steps = std::iter::once(welcome)
            .chain(news)
            .chain(std::iter::once(end))
            .collect();
    }

    steps.sort_by(|a, b| {
        chapter_index(&a.chapter)
            .cmp(&chapter_index(&b.chapter))
            .then(a.order.cmp(&b.order))
            .then_with(|| a.module.cmp(&b.module))
            .then(Ordering::Equal)
    });
    steps
}

/// A chapter's title.
pub fn chapter_title(chapter: &str) -> &'static str {
    CHAPTERS
        .iter()
        .find(|(id, _)| *id == chapter)
        .map_or("", |(_, title)| title)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offer(module: &str, id: &str, options: Value) -> Contribution {
        mochi_core::ContributionSpec::new("tour", "step", id, "View", "Title")
            .options(options)
            .into_contribution(module)
    }

    #[test]
    fn versions_compare_as_releases() {
        assert!(Version::parse("0.0.10").unwrap() > Version::parse("0.0.9").unwrap());
        assert_eq!(Version::parse("v0.1"), Some(Version(0, 1, 0)));
        assert_eq!(Version::parse("soon"), None);
    }

    #[test]
    fn the_whole_tour_runs_in_chapter_order() {
        let offers = [
            offer(
                "media",
                "player",
                json!({ "chapter": "notices", "since": "0.0.1", "caption": "Music." }),
            ),
            offer(
                "launcher",
                "search",
                json!({ "chapter": "panels", "since": "0.0.1", "caption": "Search." }),
            ),
            offer("broken", "step", json!({ "chapter": "panels" })),
        ];
        let known = [Known {
            id: "emoji".into(),
            title: "Emoji".into(),
            icon: "mood".into(),
            description: "An emoji picker. More words.".into(),
            enabled: false,
        }];
        let steps = build(&offers, &known, None);
        let ids: Vec<&str> = steps.iter().map(|step| step.id.as_str()).collect();
        assert_eq!(ids.first(), Some(&"tour/welcome"));
        assert_eq!(ids.last(), Some(&"tour/end"));
        let position = |id: &str| ids.iter().position(|step| *step == id).unwrap();
        assert!(position("tour/notch") < position("launcher/search"));
        assert!(position("launcher/search") < position("media/player"));
        assert!(position("media/player") < position("tour/off/emoji"));
        assert!(!ids.contains(&"broken/step"));
        let off = &steps[position("tour/off/emoji")];
        assert_eq!(
            off.caption,
            "An emoji picker. It's off: turn it on in Settings › Modules."
        );
    }

    #[test]
    fn whats_new_keeps_only_later_steps() {
        let offers = [
            offer(
                "media",
                "player",
                json!({ "chapter": "notices", "since": "0.0.1", "caption": "Music." }),
            ),
            offer(
                "audio",
                "meters",
                json!({ "chapter": "panels", "since": "0.0.7", "caption": "Meters." }),
            ),
        ];
        let steps = build(&offers, &[], Some(Version(0, 0, 6)));
        let ids: Vec<&str> = steps.iter().map(|step| step.id.as_str()).collect();
        assert_eq!(ids, ["tour/whats-new", "audio/meters", "tour/end"]);
        assert!(build(&offers, &[], Some(Version(0, 0, 7))).is_empty());
    }
}
