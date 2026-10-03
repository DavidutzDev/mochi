//! Ranks apps and their actions for a query.
//!
//! - No query: apps the user launches, most used first, then the rest from
//!   A to Z. Actions only show when searched for.
//! - A query: fuzzy matches on the name, and on the generic name, keywords
//!   and program, which count for less. An app's actions match on their own
//!   name and rank just below a match on the app. Often used apps get a
//!   boost.

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use crate::entries::App;
use crate::history::History;

/// One line in the list.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    /// What `launch` takes: the app id, or `app id:action id`.
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub action: bool,
}

pub fn rank(apps: &[App], history: &History, query: &str, now: u64, limit: usize) -> Vec<Hit> {
    let query = query.trim();
    if query.is_empty() {
        let mut sorted: Vec<&App> = apps.iter().collect();
        sorted.sort_by(|a, b| {
            let (a_score, b_score) = (history.score(&a.id, now), history.score(&b.id, now));
            b_score
                .total_cmp(&a_score)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        return sorted.into_iter().take(limit).map(app_hit).collect();
    }

    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
    let mut buffer = Vec::new();
    let mut score = |text: &str| {
        pattern
            .score(Utf32Str::new(text, &mut buffer), &mut matcher)
            .map(f64::from)
    };

    let mut scored: Vec<(f64, Hit)> = Vec::new();
    for app in apps {
        let boost = history.score(&app.id, now).ln_1p() * 20.0;
        let program = app.exec.split_whitespace().next().unwrap_or_default();
        let program = program.rsplit('/').next().unwrap_or(program);
        let others = [
            app.generic.as_deref().unwrap_or_default(),
            &app.keywords.join(" "),
            program,
        ]
        .into_iter()
        .filter_map(&mut score)
        .fold(None, |best: Option<f64>, s| {
            Some(best.map_or(s, |b| b.max(s)))
        });
        let best = match (score(&app.name), others) {
            (Some(name), Some(other)) => Some(name.max(other * 0.6)),
            (name, other) => name.or(other.map(|other| other * 0.6)),
        };
        if let Some(best) = best {
            scored.push((best + boost, app_hit(app)));
        }

        for action in &app.actions {
            if let Some(found) = score(&action.name) {
                let hit = Hit {
                    id: format!("{}:{}", app.id, action.id),
                    name: action.name.clone(),
                    description: Some(app.name.clone()),
                    icon: action.icon.clone().or_else(|| app.icon.clone()),
                    action: true,
                };
                scored.push((found * 0.8 + boost, hit));
            }
        }
    }
    scored.sort_by(|(a_score, a), (b_score, b)| {
        b_score
            .total_cmp(a_score)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    scored.into_iter().take(limit).map(|(_, hit)| hit).collect()
}

fn app_hit(app: &App) -> Hit {
    Hit {
        id: app.id.clone(),
        name: app.name.clone(),
        description: app.generic.clone().or_else(|| app.comment.clone()),
        icon: app.icon.clone(),
        action: false,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::entries::AppAction;

    fn app(id: &str, name: &str, generic: Option<&str>) -> App {
        App {
            id: id.into(),
            name: name.into(),
            generic: generic.map(str::to_owned),
            comment: None,
            icon: None,
            exec: format!("/usr/bin/{}", id.trim_end_matches(".desktop")),
            terminal: false,
            keywords: Vec::new(),
            path: None,
            actions: Vec::new(),
            file: PathBuf::new(),
        }
    }

    fn apps() -> Vec<App> {
        let mut firefox = app("firefox.desktop", "Firefox", Some("Web Browser"));
        firefox.actions.push(AppAction {
            id: "new-private-window".into(),
            name: "New Private Window".into(),
            icon: None,
            exec: "firefox --private-window".into(),
        });
        vec![
            app("codium.desktop", "VSCodium", Some("Text Editor")),
            app("alacritty.desktop", "Alacritty", Some("Terminal")),
            firefox,
            app("nvim.desktop", "Neovim", Some("Text Editor")),
        ]
    }

    fn names(hits: &[Hit]) -> Vec<&str> {
        hits.iter().map(|hit| hit.name.as_str()).collect()
    }

    #[test]
    fn empty_query_puts_used_apps_first_then_a_to_z() {
        let mut history = History::default();
        history.record("nvim.desktop", 0);
        let hits = rank(&apps(), &history, "", 0, 10);
        assert_eq!(names(&hits), ["Neovim", "Alacritty", "Firefox", "VSCodium"]);
        assert!(hits.iter().all(|hit| !hit.action));
    }

    #[test]
    fn names_beat_descriptions_and_actions_follow_their_app() {
        let history = History::default();
        let hits = rank(&apps(), &history, "vsc", 0, 10);
        assert_eq!(hits[0].name, "VSCodium");

        // "editor" only matches generic names.
        let hits = rank(&apps(), &history, "editor", 0, 10);
        assert_eq!(names(&hits), ["Neovim", "VSCodium"]);

        let hits = rank(&apps(), &history, "private", 0, 10);
        assert_eq!(hits[0].id, "firefox.desktop:new-private-window");
        assert_eq!(hits[0].description.as_deref(), Some("Firefox"));
        assert!(hits[0].action);

        assert!(rank(&apps(), &history, "zzzz", 0, 10).is_empty());
    }

    #[test]
    fn often_used_apps_win_close_matches() {
        let mut history = History::default();
        assert_eq!(rank(&apps(), &history, "editor", 0, 10)[0].name, "Neovim");
        for _ in 0..5 {
            history.record("codium.desktop", 0);
        }
        assert_eq!(rank(&apps(), &history, "editor", 0, 10)[0].name, "VSCodium");
    }
}
