//! What a change of the battery deserves: a notice when it drops past a
//! level or gets plugged in or out, and the warning bubble while it's low.
//! A peripheral getting low gets a notice too.

use std::collections::HashSet;
use std::time::Duration;

use serde_json::{Value, json};

use crate::upower::{Battery, Device, Snapshot, State};

/// How far over its low level a peripheral climbs before it can be low
/// again, for levels that wobble.
const RECOVERED: u32 = 5;

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
    /// A peripheral dropping to this on battery shows a notice, once until
    /// it charges; 0 never.
    pub peripherals: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub text: String,
    pub level: u32,
    pub charging: bool,
    pub critical: bool,
    /// A peripheral's symbol; `None` for the laptop's battery.
    pub icon: Option<&'static str>,
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
            icon: None,
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

/// Remembers which peripherals were said to be low, by a name that stays
/// when they disconnect, so a mouse waking up isn't news each time.
#[derive(Debug, Default)]
pub struct Peripherals {
    started: bool,
    warned: HashSet<String>,
}

impl Peripherals {
    /// A notice for each peripheral that got low. The first snapshot is only
    /// a baseline; a device that comes later already low gets one.
    pub fn apply(&mut self, devices: &[Device], levels: &Levels) -> Vec<Notice> {
        let first = !self.started;
        self.started = true;
        let mut notices = Vec::new();
        if levels.peripherals == 0 {
            return notices;
        }
        for device in devices {
            let key = key(device);
            let level = device.battery.level();
            if device.battery.state != State::Discharging || level > levels.peripherals + RECOVERED
            {
                self.warned.remove(&key);
                continue;
            }
            if level > levels.peripherals || !self.warned.insert(key) || first {
                continue;
            }
            let (label, icon) = kind(device.kind);
            notices.push(Notice {
                text: format!("{} battery low · {level}%", name(device, label)),
                level,
                charging: false,
                critical: false,
                icon: Some(icon),
            });
        }
        notices
    }
}

/// What stays the same for a device from one connection to the next.
fn key(device: &Device) -> String {
    [&device.serial, &device.native_path, &device.path]
        .into_iter()
        .find(|text| !text.is_empty())
        .cloned()
        .unwrap_or_default()
}

/// What a peripheral is called and drawn as, from UPower's `Type`.
pub fn kind(kind: u32) -> (&'static str, &'static str) {
    match kind {
        5 => ("Mouse", "mouse"),
        6 => ("Keyboard", "keyboard"),
        7 | 8 => ("Phone", "smartphone"),
        9 => ("Media player", "music_note"),
        10 => ("Tablet", "tablet"),
        11 => ("Computer", "computer"),
        12 => ("Controller", "sports_esports"),
        13 => ("Pen", "stylus"),
        14 => ("Touchpad", "touchpad_mouse"),
        17 => ("Headset", "headset_mic"),
        18 | 21 => ("Speaker", "speaker"),
        19 => ("Headphones", "headphones"),
        20 | 25 => ("Camera", "photo_camera"),
        22 => ("Remote", "settings_remote"),
        23 => ("Printer", "print"),
        26 => ("Watch", "watch"),
        27 => ("Toy", "toys"),
        _ => ("Device", "battery_full"),
    }
}

/// Its model, or else what it is.
fn name(device: &Device, label: &str) -> String {
    if device.model.is_empty() {
        label.to_owned()
    } else {
        device.model.clone()
    }
}

/// How a battery is doing, like `Charging · full in 40 min` or `2 h left`.
fn state(battery: &Battery) -> String {
    match battery.state {
        State::Charging => match battery.time_to_full {
            Some(left) => format!("Charging · full in {}", duration(left)),
            None => "Charging".to_owned(),
        },
        State::Full => "Plugged in".to_owned(),
        State::Discharging => match battery.time_to_empty {
            Some(left) => format!("{} left", duration(left)),
            None => "On battery".to_owned(),
        },
    }
}

/// What the control center's card gets: the combined level, each of the
/// laptop's batteries, and the peripherals.
pub fn payload(snapshot: &Snapshot, levels: &Levels) -> Value {
    let mut payload = match &snapshot.display {
        Some(battery) => {
            let level = battery.level();
            json!({
                "present": true,
                "level": level,
                "charging": battery.state == State::Charging,
                "plugged": battery.state != State::Discharging,
                "low": battery.state == State::Discharging && level <= levels.warning,
                "critical": battery.state == State::Discharging && level <= levels.critical,
                "state": state(battery),
                // Until full while charging, or until empty, when UPower
                // knows: the ring look puts it on a line of its own.
                "time": match battery.state {
                    State::Charging => battery.time_to_full.map(duration),
                    State::Discharging => battery.time_to_empty.map(duration),
                    State::Full => None,
                },
            })
        }
        None => json!({ "present": false }),
    };
    payload["batteries"] = snapshot
        .batteries
        .iter()
        .enumerate()
        .map(|(index, device)| {
            json!({
                "name": format!("Battery {}", index + 1),
                "model": device.model,
                "level": device.battery.level(),
                "charging": device.battery.state == State::Charging,
                "state": state(&device.battery),
            })
        })
        .collect();
    payload["devices"] = snapshot
        .peripherals
        .iter()
        .map(|device| {
            let (label, icon) = kind(device.kind);
            let level = device.battery.level();
            json!({
                "id": device.path,
                "name": name(device, label),
                "kind": label,
                "icon": icon,
                "level": level,
                "charging": device.battery.state == State::Charging,
                "low": device.battery.state == State::Discharging && level <= levels.peripherals,
            })
        })
        .collect();
    payload
}

/// What `mochi ipc battery status` prints.
pub fn status(snapshot: &Snapshot) -> String {
    let mut lines = Vec::new();
    if let Some(battery) = &snapshot.display {
        lines.push(format!(
            "battery: {}% ({})",
            battery.level(),
            state(battery)
        ));
    }
    // One battery is the combined level already.
    if snapshot.batteries.len() > 1 {
        for (index, device) in snapshot.batteries.iter().enumerate() {
            lines.push(format!(
                "battery {} ({}): {}% ({})",
                index + 1,
                device.native_path,
                device.battery.level(),
                state(&device.battery)
            ));
        }
    }
    for device in &snapshot.peripherals {
        let (label, _) = kind(device.kind);
        let charging = if device.battery.state == State::Charging {
            ", charging"
        } else {
            ""
        };
        lines.push(format!(
            "{} ({}): {}%{charging}",
            name(device, label),
            label.to_lowercase(),
            device.battery.level()
        ));
    }
    if lines.is_empty() {
        lines.push("no battery, and no device reports one".to_owned());
    }
    lines.join("\n")
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
            peripherals: 15,
        }
    }

    fn mouse(percent: f64, state: State) -> Device {
        Device {
            path: "/org/freedesktop/UPower/devices/mouse_dev_AA".into(),
            kind: 5,
            model: "MX Master 3".into(),
            serial: "AA".into(),
            reported: true,
            battery: Battery {
                state,
                ..on_battery(percent)
            },
            ..Device::default()
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
        let snapshot = Snapshot {
            display: Some(charging),
            ..Snapshot::default()
        };
        assert_eq!(payload(&snapshot, &levels)["state"], "Charging");
        let charging = Battery {
            time_to_full: Some(Duration::from_secs(40 * 60)),
            ..charging
        };
        let snapshot = Snapshot {
            display: Some(charging),
            ..Snapshot::default()
        };
        assert_eq!(payload(&snapshot, &levels)["time"], "40 min");
        assert_eq!(payload(&Snapshot::default(), &levels)["present"], false);
    }

    #[test]
    fn says_once_when_a_peripheral_gets_low() {
        let levels = levels();
        let mut peripherals = Peripherals::default();
        // Low already at the start: only a baseline.
        assert!(
            peripherals
                .apply(&[mouse(10.0, State::Discharging)], &levels)
                .is_empty()
        );
        // Charged, it can be low again.
        assert!(
            peripherals
                .apply(&[mouse(12.0, State::Charging)], &levels)
                .is_empty()
        );
        assert!(
            peripherals
                .apply(&[mouse(16.0, State::Discharging)], &levels)
                .is_empty()
        );
        let notices = peripherals.apply(&[mouse(15.0, State::Discharging)], &levels);
        assert_eq!(notices[0].text, "MX Master 3 battery low · 15%");
        assert_eq!(notices[0].icon, Some("mouse"));
        // Wobbling around the level, or going away and coming back, says
        // nothing more.
        for percent in [16.0, 14.0, 18.0] {
            assert!(
                peripherals
                    .apply(&[mouse(percent, State::Discharging)], &levels)
                    .is_empty()
            );
        }
        assert!(peripherals.apply(&[], &levels).is_empty());
        assert!(
            peripherals
                .apply(&[mouse(13.0, State::Discharging)], &levels)
                .is_empty()
        );
        // A device that comes already low says so.
        let pad = Device {
            path: "/pad".into(),
            kind: 12,
            model: String::new(),
            serial: String::new(),
            ..mouse(5.0, State::Discharging)
        };
        let notices = peripherals.apply(std::slice::from_ref(&pad), &levels);
        assert_eq!(notices[0].text, "Controller battery low · 5%");
        // Off in the settings.
        let quiet = Levels {
            peripherals: 0,
            ..levels
        };
        let mut peripherals = Peripherals::default();
        peripherals.apply(&[], &quiet);
        assert!(peripherals.apply(&[pad], &quiet).is_empty());
    }

    #[test]
    fn lists_batteries_and_peripherals() {
        let levels = levels();
        let laptop = |native: &str, percent: f64| Device {
            native_path: native.into(),
            kind: crate::upower::BATTERY,
            power_supply: true,
            reported: true,
            battery: on_battery(percent),
            ..Device::default()
        };
        let snapshot = Snapshot {
            display: Some(on_battery(60.0)),
            batteries: vec![laptop("BAT0", 70.0), laptop("BAT1", 50.0)],
            peripherals: vec![mouse(12.0, State::Discharging)],
        };
        let payload = payload(&snapshot, &levels);
        assert_eq!(payload["batteries"][1]["name"], "Battery 2");
        assert_eq!(payload["batteries"][1]["level"], 50);
        assert_eq!(payload["devices"][0]["name"], "MX Master 3");
        assert_eq!(payload["devices"][0]["icon"], "mouse");
        assert_eq!(payload["devices"][0]["low"], true);
        assert_eq!(
            status(&snapshot),
            "battery: 60% (On battery)\nbattery 1 (BAT0): 70% (On battery)\nbattery 2 (BAT1): 50% (On battery)\nMX Master 3 (mouse): 12%"
        );
        assert_eq!(
            status(&Snapshot::default()),
            "no battery, and no device reports one"
        );
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
