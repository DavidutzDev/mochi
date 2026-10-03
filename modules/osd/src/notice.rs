//! Turns state changes into notices worth showing.
//!
//! Sources report the current state, not deltas, and often report the same
//! state several times in a row. The tracker compares each report with the
//! previous one. The first report of each kind only sets the baseline, so
//! starting up or reconnecting to the audio server never shows anything.

use serde::Serialize;
use serde_json::{Value, json};

/// The default output device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    /// Stable identifier, used to tell a device switch from a volume change.
    pub name: String,
    pub description: String,
    pub kind: DeviceKind,
    /// Average of the channels, in percent. Can go above 100.
    pub volume: u32,
    pub muted: bool,
}

/// The default input device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Input {
    pub name: String,
    pub muted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Locks {
    pub caps: bool,
    pub num: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    Headset,
    Speakers,
    Display,
}

impl DeviceKind {
    /// Guesses the kind from what the audio server reports: the form factor
    /// property, then the active port and device names.
    pub fn detect(form_factor: Option<&str>, port: &str, name: &str) -> Self {
        match form_factor {
            Some("headset" | "headphone" | "hands-free") => return Self::Headset,
            Some("speaker" | "internal" | "computer") => return Self::Speakers,
            _ => {}
        }
        let words = format!("{port} {name}").to_ascii_lowercase();
        if ["hdmi", "displayport", "iec958-hdmi"]
            .iter()
            .any(|word| words.contains(word))
        {
            Self::Display
        } else if ["headphone", "headset", "bluez"]
            .iter()
            .any(|word| words.contains(word))
        {
            Self::Headset
        } else {
            Self::Speakers
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LockKey {
    Caps,
    Num,
}

/// A state report from one of the sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Output(Option<Output>),
    Input(Option<Input>),
    Locks(Locks),
    /// The audio connection was lost. The next reports set a new baseline.
    AudioReset,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    Volume {
        percent: u32,
        muted: bool,
    },
    Device {
        description: String,
        kind: DeviceKind,
    },
    Microphone {
        muted: bool,
    },
    Lock {
        key: LockKey,
        on: bool,
    },
}

impl Notice {
    /// The QML view that shows it.
    pub fn view(&self) -> &'static str {
        match self {
            Self::Volume { .. } => "Volume",
            Self::Device { .. } => "Device",
            Self::Microphone { .. } => "Microphone",
            Self::Lock { .. } => "Lock",
        }
    }

    pub fn payload(&self) -> Value {
        match self {
            Self::Volume { percent, muted } => json!({ "percent": percent, "muted": muted }),
            Self::Device { description, kind } => {
                json!({ "description": description, "kind": kind })
            }
            Self::Microphone { muted } => json!({ "muted": muted }),
            Self::Lock { key, on } => json!({ "key": key, "on": on }),
        }
    }
}

#[derive(Debug, Default)]
pub struct Tracker {
    /// `None` until the first report: no baseline yet.
    output: Option<Option<Output>>,
    input: Option<Option<Input>>,
    locks: Option<Locks>,
}

impl Tracker {
    pub fn apply(&mut self, change: Change) -> Option<Notice> {
        match change {
            Change::Output(next) => {
                let previous = self.output.replace(next.clone())?;
                output_notice(previous.as_ref(), next.as_ref())
            }
            Change::Input(next) => {
                let previous = self.input.replace(next.clone())?;
                match (previous, next) {
                    (Some(previous), Some(next))
                        if previous.name == next.name && previous.muted != next.muted =>
                    {
                        Some(Notice::Microphone { muted: next.muted })
                    }
                    _ => None,
                }
            }
            Change::Locks(next) => {
                let previous = self.locks.replace(next)?;
                if previous.caps != next.caps {
                    Some(Notice::Lock {
                        key: LockKey::Caps,
                        on: next.caps,
                    })
                } else if previous.num != next.num {
                    Some(Notice::Lock {
                        key: LockKey::Num,
                        on: next.num,
                    })
                } else {
                    None
                }
            }
            Change::AudioReset => {
                self.output = None;
                self.input = None;
                None
            }
        }
    }
}

fn output_notice(previous: Option<&Output>, next: Option<&Output>) -> Option<Notice> {
    let next = next?;
    let switched = previous.is_none_or(|previous| previous.name != next.name);
    if switched {
        // A switch usually changes the volume too; the device is the news.
        return Some(Notice::Device {
            description: next.description.clone(),
            kind: next.kind,
        });
    }
    let previous = previous?;
    if previous.volume != next.volume || previous.muted != next.muted {
        return Some(Notice::Volume {
            percent: next.volume,
            muted: next.muted,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headset(volume: u32, muted: bool) -> Output {
        Output {
            name: "alsa_output.usb-arctis".into(),
            description: "Arctis Nova 7".into(),
            kind: DeviceKind::Headset,
            volume,
            muted,
        }
    }

    fn monitor() -> Output {
        Output {
            name: "alsa_output.pci-hdmi".into(),
            description: "C27F390".into(),
            kind: DeviceKind::Display,
            volume: 40,
            muted: false,
        }
    }

    fn mic(muted: bool) -> Input {
        Input {
            name: "alsa_input.usb-arctis".into(),
            muted,
        }
    }

    #[test]
    fn first_reports_are_silent() {
        let mut tracker = Tracker::default();
        assert_eq!(
            tracker.apply(Change::Output(Some(headset(57, false)))),
            None
        );
        assert_eq!(tracker.apply(Change::Input(Some(mic(false)))), None);
        assert_eq!(tracker.apply(Change::Locks(Locks::default())), None);
    }

    #[test]
    fn volume_and_mute_changes_show_the_volume() {
        let mut tracker = Tracker::default();
        tracker.apply(Change::Output(Some(headset(57, false))));

        assert_eq!(
            tracker.apply(Change::Output(Some(headset(62, false)))),
            Some(Notice::Volume {
                percent: 62,
                muted: false
            })
        );
        assert_eq!(
            tracker.apply(Change::Output(Some(headset(62, true)))),
            Some(Notice::Volume {
                percent: 62,
                muted: true
            })
        );
    }

    #[test]
    fn repeated_reports_show_nothing() {
        let mut tracker = Tracker::default();
        tracker.apply(Change::Output(Some(headset(57, false))));
        assert_eq!(
            tracker.apply(Change::Output(Some(headset(57, false)))),
            None
        );

        tracker.apply(Change::Locks(Locks::default()));
        assert_eq!(tracker.apply(Change::Locks(Locks::default())), None);
    }

    #[test]
    fn volume_above_100_is_reported_as_is() {
        let mut tracker = Tracker::default();
        tracker.apply(Change::Output(Some(headset(100, false))));
        assert_eq!(
            tracker.apply(Change::Output(Some(headset(130, false)))),
            Some(Notice::Volume {
                percent: 130,
                muted: false
            })
        );
    }

    #[test]
    fn a_device_switch_wins_over_its_volume_change() {
        let mut tracker = Tracker::default();
        tracker.apply(Change::Output(Some(headset(57, false))));
        assert_eq!(
            tracker.apply(Change::Output(Some(monitor()))),
            Some(Notice::Device {
                description: "C27F390".into(),
                kind: DeviceKind::Display
            })
        );
    }

    #[test]
    fn a_device_appearing_is_a_switch_and_disappearing_is_silent() {
        let mut tracker = Tracker::default();
        tracker.apply(Change::Output(None));
        assert!(matches!(
            tracker.apply(Change::Output(Some(headset(57, false)))),
            Some(Notice::Device { .. })
        ));
        assert_eq!(tracker.apply(Change::Output(None)), None);
    }

    #[test]
    fn microphone_mute_shows_only_for_the_same_device() {
        let mut tracker = Tracker::default();
        tracker.apply(Change::Input(Some(mic(false))));
        assert_eq!(
            tracker.apply(Change::Input(Some(mic(true)))),
            Some(Notice::Microphone { muted: true })
        );

        let other = Input {
            name: "alsa_input.webcam".into(),
            muted: false,
        };
        assert_eq!(tracker.apply(Change::Input(Some(other))), None);
    }

    #[test]
    fn lock_toggles_show_which_key_and_its_state() {
        let mut tracker = Tracker::default();
        tracker.apply(Change::Locks(Locks::default()));

        let caps_on = Locks {
            caps: true,
            num: false,
        };
        assert_eq!(
            tracker.apply(Change::Locks(caps_on)),
            Some(Notice::Lock {
                key: LockKey::Caps,
                on: true
            })
        );
        let num_on = Locks {
            caps: true,
            num: true,
        };
        assert_eq!(
            tracker.apply(Change::Locks(num_on)),
            Some(Notice::Lock {
                key: LockKey::Num,
                on: true
            })
        );
    }

    #[test]
    fn a_reconnect_sets_a_new_baseline() {
        let mut tracker = Tracker::default();
        tracker.apply(Change::Output(Some(headset(57, false))));
        tracker.apply(Change::AudioReset);
        // After a reconnect, the first report is the new baseline even though
        // the volume differs.
        assert_eq!(
            tracker.apply(Change::Output(Some(headset(80, false)))),
            None
        );
        assert!(
            tracker
                .apply(Change::Output(Some(headset(81, false))))
                .is_some()
        );
    }

    #[test]
    fn a_reconnect_keeps_the_lock_baseline() {
        let mut tracker = Tracker::default();
        tracker.apply(Change::Locks(Locks::default()));
        tracker.apply(Change::AudioReset);
        assert!(
            tracker
                .apply(Change::Locks(Locks {
                    caps: true,
                    num: false
                }))
                .is_some()
        );
    }

    #[test]
    fn device_kinds() {
        assert_eq!(
            DeviceKind::detect(Some("headset"), "analog-output", "x"),
            DeviceKind::Headset
        );
        assert_eq!(
            DeviceKind::detect(None, "hdmi-output-0", "x"),
            DeviceKind::Display
        );
        assert_eq!(
            DeviceKind::detect(None, "", "alsa_output.pci-0000.hdmi-stereo"),
            DeviceKind::Display
        );
        assert_eq!(
            DeviceKind::detect(None, "analog-output-headphones", "x"),
            DeviceKind::Headset
        );
        assert_eq!(
            DeviceKind::detect(None, "", "bluez_output.AA_BB.1"),
            DeviceKind::Headset
        );
        assert_eq!(
            DeviceKind::detect(None, "analog-output-speaker", "x"),
            DeviceKind::Speakers
        );
        assert_eq!(
            DeviceKind::detect(Some("speaker"), "hdmi", "x"),
            DeviceKind::Speakers
        );
    }

    #[test]
    fn payloads_match_what_the_views_read() {
        let notice = Notice::Device {
            description: "Arctis Nova 7".into(),
            kind: DeviceKind::Headset,
        };
        assert_eq!(notice.view(), "Device");
        assert_eq!(
            notice.payload(),
            json!({ "description": "Arctis Nova 7", "kind": "headset" })
        );
        assert_eq!(
            Notice::Lock {
                key: LockKey::Caps,
                on: true
            }
            .payload(),
            json!({ "key": "caps", "on": true })
        );
    }
}
