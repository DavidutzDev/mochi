//! What a change of the battery deserves: a notice when it drops past a
//! level or gets plugged in or out, and the warning bubble while it's low.

use std::time::Duration;

use serde_json::{Value, json};

use crate::upower::{Battery, State};

/// When to say what, from the settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Levels {
    /// Dropping past each of these on battery shows a notice, once per
    /// discharge.
    pub notices: Vec<u32>,
    /// At or under this on battery, the warning bubble shows.
    pub warning: u32,
    /// At or under this, the bubble and the notice turn red, and the
    /// notice stays longer.
    pub critical: u32,
    /// A notice when the charger is plugged in or out.
    pub plugged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub text: String,
    pub level: u32,
    pub charging: bool,
    pub critical: bool,
}

/// Remembers the last battery, to tell what changed.
#[derive(Debug, Default)]
pub struct Tracker {
    last: Option<Battery>,
}

impl Tracker {
    pub fn apply(&mut self, battery: Battery, levels: &Levels) -> Option<Notice> {
        let last = self.last.replace(battery)?;
        let level = battery.level();
        let notice = |text: String| Notice {
            text,
            level,
            charging: battery.state == State::Charging,
            critical: battery.state == State::Discharging && level <= levels.critical,
        };
        let on_battery = battery.state == State::Discharging;
        let was_on_battery = last.state == State::Discharging;
        if levels.plugged && on_battery != was_on_battery {
            return Some(notice(if on_battery {
                match battery.time_to_empty {
                    Some(left) => format!("On battery · {level}% · {} left", duration(left)),
                    None => format!("On battery · {level}%"),
                }
            } else if battery.state == State::Charging {
                format!("Charging · {level}%")
            } else {
                format!("Plugged in · {level}%")
            }));
        }
        if !on_battery || !was_on_battery {
            return None;
        }
        // A jump past several levels says it once.
        let crossed = levels
            .notices
            .iter()
            .any(|mark| level <= *mark && last.level() > *mark);
        if !crossed {
            return None;
        }
        Some(notice(match battery.time_to_empty {
            Some(left) if level <= levels.critical => {
                format!(
                    "Battery critical · {level}% · {} left · plug in",
                    duration(left)
                )
            }
            None if level <= levels.critical => format!("Battery critical · {level}% · plug in"),
            Some(left) => format!("Battery at {level}% · {} left", duration(left)),
            None => format!("Battery at {level}%"),
        }))
    }
}

/// The warning bubble's payload, while on battery at or under the warning
/// level.
pub fn bubble(battery: &Battery, levels: &Levels) -> Option<Value> {
    let level = battery.level();
    (battery.state == State::Discharging && level <= levels.warning).then(|| {
        json!({
            "level": level,
            "critical": level <= levels.critical,
            "left": battery.time_to_empty.map(duration),
        })
    })
}

/// What the control center's card gets.
pub fn payload(battery: Option<&Battery>, levels: &Levels) -> Value {
    let Some(battery) = battery else {
        return json!({ "present": false });
    };
    let level = battery.level();
    let state = match battery.state {
        State::Charging => match battery.time_to_full {
            Some(left) => format!("Charging · full in {}", duration(left)),
            None => "Charging".to_owned(),
        },
        State::Full => "Plugged in".to_owned(),
        State::Discharging => match battery.time_to_empty {
            Some(left) => format!("{} left", duration(left)),
            None => "On battery".to_owned(),
        },
    };
    json!({
        "present": true,
        "level": level,
        "charging": battery.state == State::Charging,
        "plugged": battery.state != State::Discharging,
        "low": battery.state == State::Discharging && level <= levels.warning,
        "critical": battery.state == State::Discharging && level <= levels.critical,
        "state": state,
    })
}

/// `3 h 20 min`, `45 min`.
pub fn duration(time: Duration) -> String {
    let minutes = time.as_secs() / 60;
    match (minutes / 60, minutes % 60) {
        (0, minutes) => format!("{minutes} min"),
        (hours, 0) => format!("{hours} h"),
        (hours, minutes) => format!("{hours} h {minutes} min"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn levels() -> Levels {
        Levels {
            notices: vec![80, 50, 20, 10],
            warning: 50,
            critical: 10,
            plugged: true,
        }
    }

    fn on_battery(percent: f64) -> Battery {
        Battery {
            percent,
            state: State::Discharging,
            time_to_empty: None,
            time_to_full: None,
        }
    }

    #[test]
    fn says_once_per_level_crossed() {
        let mut tracker = Tracker::default();
        let levels = levels();
        // The first reading is only a baseline.
        assert_eq!(tracker.apply(on_battery(85.0), &levels), None);
        assert_eq!(tracker.apply(on_battery(81.0), &levels), None);
        let notice = tracker.apply(on_battery(80.0), &levels).unwrap();
        assert_eq!(notice.text, "Battery at 80%");
        assert!(!notice.critical);
        assert_eq!(tracker.apply(on_battery(79.0), &levels), None);
        // A jump past two levels says it once.
        assert!(tracker.apply(on_battery(19.0), &levels).is_some());
        let critical = tracker.apply(on_battery(9.0), &levels).unwrap();
        assert!(critical.critical);
        assert_eq!(critical.text, "Battery critical · 9% · plug in");
    }

    #[test]
    fn says_when_plugged_in_or_out() {
        let mut tracker = Tracker::default();
        let levels = levels();
        tracker.apply(on_battery(45.0), &levels);
        let charging = Battery {
            state: State::Charging,
            ..on_battery(45.0)
        };
        assert_eq!(
            tracker.apply(charging, &levels).unwrap().text,
            "Charging · 45%"
        );
        let unplugged = Battery {
            time_to_empty: Some(Duration::from_secs(12_000)),
            ..on_battery(46.0)
        };
        assert_eq!(
            tracker.apply(unplugged, &levels).unwrap().text,
            "On battery · 46% · 3 h 20 min left"
        );
        // Charging past a level says nothing.
        let mut quiet = Levels {
            plugged: false,
            ..levels
        };
        quiet.notices = vec![80];
        let mut tracker = Tracker::default();
        tracker.apply(
            Battery {
                state: State::Charging,
                ..on_battery(79.0)
            },
            &quiet,
        );
        assert_eq!(
            tracker.apply(
                Battery {
                    state: State::Charging,
                    ..on_battery(81.0)
                },
                &quiet
            ),
            None
        );
    }

    #[test]
    fn warns_while_low_on_battery() {
        let levels = levels();
        assert!(bubble(&on_battery(51.0), &levels).is_none());
        assert_eq!(
            bubble(&on_battery(50.0), &levels).unwrap()["critical"],
            false
        );
        assert_eq!(bubble(&on_battery(8.0), &levels).unwrap()["critical"], true);
        let charging = Battery {
            state: State::Charging,
            ..on_battery(8.0)
        };
        assert!(bubble(&charging, &levels).is_none());
        assert_eq!(payload(Some(&charging), &levels)["state"], "Charging");
        assert_eq!(payload(None, &levels)["present"], false);
    }

    #[test]
    fn writes_durations() {
        assert_eq!(duration(Duration::from_secs(45 * 60)), "45 min");
        assert_eq!(duration(Duration::from_secs(2 * 3600)), "2 h");
        assert_eq!(
            duration(Duration::from_secs(3 * 3600 + 20 * 60)),
            "3 h 20 min"
        );
    }
}
