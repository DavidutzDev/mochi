//! The agent sessions the module knows, in the order they first spoke,
//! and what each is doing. Nothing here touches the island: `lib.rs` turns
//! what changed into notices and the bubble.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{Value, json};

/// What a session is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Working,
    /// Waiting for the user: a question, or a permission to give.
    Waiting,
    /// Finished its turn.
    Done,
}

impl State {
    pub const NAMES: [&str; 3] = ["working", "waiting", "done"];

    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "working" => Some(Self::Working),
            "waiting" => Some(Self::Waiting),
            "done" => Some(Self::Done),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub id: String,
    /// The agent, like "Claude Code".
    pub app: String,
    /// What the session is about, like its project's folder. May be empty.
    pub title: String,
    pub state: State,
    /// When it entered its state.
    pub since: SystemTime,
    /// When it last said anything, for `stale_minutes`.
    pub heard: SystemTime,
}

/// The app a session gets when nobody named one.
pub const UNNAMED: &str = "Agent";

#[derive(Debug, Default)]
pub struct Sessions {
    list: Vec<Session>,
}

impl Sessions {
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn get(&self, id: &str) -> Option<&Session> {
        self.list.iter().find(|session| session.id == id)
    }

    /// Records what a session says it's doing. An empty `app` or `title`
    /// keeps the one it had. Returns the state it entered, or `None` when
    /// it was already in it.
    pub fn set(
        &mut self,
        id: &str,
        state: State,
        app: &str,
        title: &str,
        now: SystemTime,
    ) -> Option<State> {
        let Some(session) = self.list.iter_mut().find(|session| session.id == id) else {
            let app = match app.trim() {
                "" => UNNAMED,
                app => app,
            };
            self.list.push(Session {
                id: id.to_owned(),
                app: app.to_owned(),
                title: title.trim().to_owned(),
                state,
                since: now,
                heard: now,
            });
            return Some(state);
        };
        if !app.trim().is_empty() {
            app.trim().clone_into(&mut session.app);
        }
        if !title.trim().is_empty() {
            title.trim().clone_into(&mut session.title);
        }
        session.heard = now;
        if session.state == state {
            return None;
        }
        session.state = state;
        session.since = now;
        Some(state)
    }

    /// Forgets one session. Whether it was there.
    pub fn clear(&mut self, id: &str) -> bool {
        let before = self.list.len();
        self.list.retain(|session| session.id != id);
        self.list.len() != before
    }

    /// Forgets the sessions that are done, and returns their ids.
    pub fn clear_done(&mut self) -> Vec<String> {
        self.drain(|session| session.state == State::Done)
    }

    /// Forgets every session, and returns their ids.
    pub fn clear_all(&mut self) -> Vec<String> {
        self.drain(|_| true)
    }

    /// Forgets the sessions nothing was heard from for `after`, and returns
    /// their ids.
    pub fn prune(&mut self, after: Duration, now: SystemTime) -> Vec<String> {
        self.drain(|session| {
            now.duration_since(session.heard)
                .is_ok_and(|quiet| quiet >= after)
        })
    }

    fn drain(&mut self, mut leaving: impl FnMut(&Session) -> bool) -> Vec<String> {
        let mut gone = Vec::new();
        self.list.retain(|session| {
            let leaves = leaving(session);
            if leaves {
                gone.push(session.id.clone());
            }
            !leaves
        });
        gone
    }

    /// Whether every session is done: a click on the bubble then clears
    /// them instead of opening the list.
    pub fn all_done(&self) -> bool {
        self.list.iter().all(|session| session.state == State::Done)
    }

    /// The most pressing state: someone waiting, then working, then done.
    pub fn top(&self) -> Option<State> {
        let has = |state| self.list.iter().any(|session| session.state == state);
        [State::Waiting, State::Working, State::Done]
            .into_iter()
            .find(|state| has(*state))
    }

    /// What the views get: the sessions, oldest first, with `since` in
    /// milliseconds since the epoch.
    pub fn payload(&self) -> Value {
        let sessions: Vec<Value> = self.list.iter().map(session_payload).collect();
        json!({ "sessions": sessions })
    }
}

pub fn session_payload(session: &Session) -> Value {
    json!({
        "id": session.id,
        "app": session.app,
        "title": session.title,
        "state": session.state,
        "since": millis(session.since),
    })
}

fn millis(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).map_or(0, |since| {
        u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(1_000_000 + seconds)
    }

    #[test]
    fn a_session_goes_through_its_states() {
        let mut sessions = Sessions::default();
        assert_eq!(
            sessions.set("a", State::Working, "Claude Code", "mochi", at(0)),
            Some(State::Working)
        );
        // Every tool call says working again: nothing new.
        assert_eq!(sessions.set("a", State::Working, "", "", at(5)), None);
        assert_eq!(
            sessions.set("a", State::Waiting, "", "", at(10)),
            Some(State::Waiting)
        );
        assert_eq!(
            sessions.set("a", State::Working, "", "", at(20)),
            Some(State::Working)
        );
        assert_eq!(
            sessions.set("a", State::Done, "", "", at(30)),
            Some(State::Done)
        );
        let session = sessions.get("a").unwrap();
        // Empty names keep the earlier ones.
        assert_eq!(
            (session.app.as_str(), session.title.as_str()),
            ("Claude Code", "mochi")
        );
        assert_eq!(session.since, at(30));
    }

    #[test]
    fn a_new_session_enters_whatever_it_says() {
        let mut sessions = Sessions::default();
        assert_eq!(
            sessions.set("a", State::Done, "", "", at(0)),
            Some(State::Done)
        );
        assert_eq!(
            sessions.set("b", State::Waiting, "", "", at(0)),
            Some(State::Waiting)
        );
        assert_eq!(sessions.get("a").unwrap().app, UNNAMED);
    }

    #[test]
    fn the_most_pressing_state_leads() {
        let mut sessions = Sessions::default();
        assert_eq!(sessions.top(), None);
        sessions.set("a", State::Done, "", "", at(0));
        assert!(sessions.all_done());
        assert_eq!(sessions.top(), Some(State::Done));
        sessions.set("b", State::Working, "", "", at(0));
        assert!(!sessions.all_done());
        assert_eq!(sessions.top(), Some(State::Working));
        sessions.set("c", State::Waiting, "", "", at(0));
        assert_eq!(sessions.top(), Some(State::Waiting));
    }

    #[test]
    fn clearing() {
        let mut sessions = Sessions::default();
        sessions.set("a", State::Done, "", "", at(0));
        sessions.set("b", State::Working, "", "", at(0));
        sessions.set("c", State::Done, "", "", at(0));
        assert_eq!(sessions.clear_done(), ["a", "c"]);
        assert!(sessions.clear("b"));
        assert!(!sessions.clear("b"));
        assert!(sessions.is_empty());

        sessions.set("d", State::Waiting, "", "", at(0));
        assert_eq!(sessions.clear_all(), ["d"]);
    }

    #[test]
    fn quiet_sessions_go() {
        let mut sessions = Sessions::default();
        sessions.set("old", State::Waiting, "", "", at(0));
        sessions.set("new", State::Working, "", "", at(0));
        // Saying the same again still counts as news from it.
        sessions.set("new", State::Working, "", "", at(50 * 60));
        assert_eq!(
            sessions.prune(Duration::from_secs(60 * 60), at(60 * 60)),
            ["old"]
        );
        assert!(sessions.get("new").is_some());
    }

    #[test]
    fn the_payload_keeps_the_order_they_came_in() {
        let mut sessions = Sessions::default();
        sessions.set("b", State::Working, "Claude Code", "shell", at(0));
        sessions.set("a", State::Done, "T3 Code", "", at(1));
        sessions.set("b", State::Waiting, "", "", at(2));
        let payload = sessions.payload();
        assert_eq!(payload["sessions"][0]["id"], "b");
        assert_eq!(payload["sessions"][0]["state"], "waiting");
        assert_eq!(payload["sessions"][0]["since"], 1_000_002_000_u64);
        assert_eq!(payload["sessions"][1]["app"], "T3 Code");
    }
}
