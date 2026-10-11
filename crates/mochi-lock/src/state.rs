//! What the keys do, and what the face shows. It never unlocks: it hands a
//! typed password over for checking, and only hears back when the check
//! failed. A password PAM accepts goes straight from the check to the lock.

use std::time::{Duration, Instant};

use crate::auth::Password;

/// A key, as far as the lock cares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    Text(String),
    Backspace,
    Enter,
    /// Escape and Ctrl+U: start again.
    Clear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Typing,
    /// PAM has the password; keys wait.
    Checking,
    /// After too many wrong tries: Enter waits until then.
    Waiting(Instant),
}

/// What the caller should do after a change.
#[derive(Debug)]
pub enum Effect {
    Nothing,
    Redraw,
    /// Check this password, then call [`State::refused`] if it's wrong.
    Check(Password),
}

#[derive(Debug)]
pub struct State {
    password: Password,
    pub phase: Phase,
    /// Wrong tries in a row.
    pub failures: u32,
    /// What PAM said last, or why the last try failed.
    pub message: Option<String>,
    pub caps_lock: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            password: Password::default(),
            phase: Phase::Typing,
            failures: 0,
            message: None,
            caps_lock: false,
        }
    }
}

/// How long Enter waits after the `failures`-th wrong try in a row: not at
/// all for the first two, then 2, 4, 8 seconds and so on, up to a minute.
/// PAM's own delays and lockout come on top.
pub fn backoff(failures: u32) -> Duration {
    if failures < 3 {
        return Duration::ZERO;
    }
    Duration::from_secs((1u64 << (failures - 2).min(6)).min(60))
}

impl State {
    /// How many characters are typed, for the dots.
    pub fn typed(&self) -> usize {
        self.password.chars()
    }

    pub fn key(&mut self, key: Key, now: Instant) -> Effect {
        if self.phase == Phase::Checking {
            return Effect::Nothing;
        }
        if let Phase::Waiting(until) = self.phase
            && now >= until
        {
            self.phase = Phase::Typing;
        }
        match key {
            Key::Text(text) => {
                // Control characters aren't part of a password typed here.
                if text.chars().any(char::is_control) || !self.password.push(&text) {
                    return Effect::Nothing;
                }
            }
            Key::Backspace => self.password.pop(),
            Key::Clear => self.password.clear(),
            Key::Enter => {
                if self.password.is_empty() || matches!(self.phase, Phase::Waiting(_)) {
                    return Effect::Nothing;
                }
                self.phase = Phase::Checking;
                self.message = None;
                return Effect::Check(self.password.take());
            }
        }
        Effect::Redraw
    }

    /// The check said no.
    pub fn refused(&mut self, messages: Vec<String>, now: Instant) {
        self.failures += 1;
        self.message = Some(if messages.is_empty() {
            "Wrong password".to_owned()
        } else {
            messages.join(" ")
        });
        let wait = backoff(self.failures);
        self.phase = if wait.is_zero() {
            Phase::Typing
        } else {
            Phase::Waiting(now + wait)
        };
    }

    /// Ends a wait that ran out. `true` when that changed what shows.
    pub fn tick(&mut self, now: Instant) -> bool {
        match self.phase {
            Phase::Waiting(until) if now >= until => {
                self.phase = Phase::Typing;
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(state: &mut State, text: &str, now: Instant) {
        for character in text.chars() {
            state.key(Key::Text(character.to_string()), now);
        }
    }

    #[test]
    fn enter_hands_the_password_over_and_keys_wait_while_it_is_checked() {
        let now = Instant::now();
        let mut state = State::default();
        assert!(
            matches!(state.key(Key::Enter, now), Effect::Nothing),
            "nothing typed"
        );
        typed(&mut state, "hunter2", now);
        state.key(Key::Backspace, now);
        assert_eq!(state.typed(), 6);
        let Effect::Check(password) = state.key(Key::Enter, now) else {
            panic!("Enter should check");
        };
        assert_eq!(password.chars(), 6);
        assert_eq!(state.typed(), 0, "the field is empty once handed over");
        assert_eq!(state.phase, Phase::Checking);
        assert!(matches!(
            state.key(Key::Text("x".into()), now),
            Effect::Nothing
        ));
        assert!(matches!(state.key(Key::Enter, now), Effect::Nothing));
        assert_eq!(state.typed(), 0);
    }

    #[test]
    fn wrong_tries_wait_longer_and_longer() {
        let mut now = Instant::now();
        let mut state = State::default();
        for _ in 0..2 {
            typed(&mut state, "nope", now);
            state.key(Key::Enter, now);
            state.refused(Vec::new(), now);
            assert_eq!(state.phase, Phase::Typing);
        }
        assert_eq!(state.message.as_deref(), Some("Wrong password"));
        typed(&mut state, "nope", now);
        state.key(Key::Enter, now);
        state.refused(vec!["Account locked".into()], now);
        assert_eq!(state.message.as_deref(), Some("Account locked"));
        assert_eq!(state.phase, Phase::Waiting(now + Duration::from_secs(2)));
        // Typing is fine while waiting; Enter isn't.
        typed(&mut state, "again", now);
        assert!(matches!(state.key(Key::Enter, now), Effect::Nothing));
        now += Duration::from_secs(2);
        assert!(state.tick(now));
        assert!(matches!(state.key(Key::Enter, now), Effect::Check(_)));
        assert_eq!(backoff(4), Duration::from_secs(4));
        assert_eq!(backoff(40), Duration::from_secs(60));
    }

    #[test]
    fn control_characters_and_clear() {
        let now = Instant::now();
        let mut state = State::default();
        typed(&mut state, "abc", now);
        assert!(matches!(
            state.key(Key::Text("\u{8}".into()), now),
            Effect::Nothing
        ));
        assert_eq!(state.typed(), 3);
        state.key(Key::Clear, now);
        assert_eq!(state.typed(), 0);
    }
}
