//! The laptop's backlight, in `/sys/class/backlight`, and the keyboard's, an
//! LED in `/sys/class/leds`. Reading needs nothing; writing goes through
//! logind, which lets the user of the active session set them without a udev
//! rule, and falls back to the file for systems that grant it.

use std::path::{Path, PathBuf};

use zbus::Connection;

/// The directory the kernel lists backlights in.
pub const SYSFS: &str = "/sys/class/backlight";
/// The directory the kernel lists LEDs in, keyboard backlights among them.
pub const LEDS: &str = "/sys/class/leds";

/// What a light is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Screen,
    Keyboard,
}

impl Kind {
    /// The subsystem logind's `SetBrightness` takes.
    fn subsystem(self) -> &'static str {
        match self {
            Self::Screen => "backlight",
            Self::Keyboard => "leds",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backlight {
    /// The device's name, like `intel_backlight` or `tpacpi::kbd_backlight`.
    pub name: String,
    pub path: PathBuf,
    pub value: u32,
    pub max: u32,
    pub kind: Kind,
}

impl Backlight {
    pub fn percent(&self) -> u32 {
        percent(self.value, self.max)
    }

    /// The raw value for a percent. A screen never gets 0, which turns some
    /// panels off; a keyboard does.
    pub fn value_for(&self, percent: u32) -> u32 {
        let value = (f64::from(percent.min(100)) * f64::from(self.max) / 100.0).round() as u32;
        let least = match self.kind {
            Kind::Screen => 1,
            Kind::Keyboard => 0,
        };
        value.clamp(least, self.max.max(least))
    }

    /// The raw value for a percent reached by moving from the current one,
    /// at least a level away: a keyboard has two or three levels, and a step
    /// of 5% would never reach the next.
    pub fn value_moved_to(&self, percent: u32) -> u32 {
        let value = self.value_for(percent);
        let now = self.percent();
        if value != self.value || percent == now {
            return value;
        }
        if percent > now {
            (self.value + 1).min(self.max)
        } else {
            self.value_for(0).max(self.value.saturating_sub(1))
        }
    }

    /// Reads the value again; `None` when the device went away.
    pub fn read(&self) -> Option<u32> {
        read_number(&self.path.join("brightness"))
    }
}

pub fn percent(value: u32, max: u32) -> u32 {
    if max == 0 {
        return 0;
    }
    (f64::from(value) * 100.0 / f64::from(max)).round() as u32
}

fn read_number(path: &Path) -> Option<u32> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// A light in `path`, when it has a level to read.
fn light(path: PathBuf, kind: Kind) -> Option<Backlight> {
    let max = read_number(&path.join("max_brightness")).filter(|max| *max > 0)?;
    let value = read_number(&path.join("brightness"))?;
    Some(Backlight {
        name: path.file_name()?.to_string_lossy().into_owned(),
        path,
        value,
        max,
        kind,
    })
}

/// The backlight to use, from the devices in `root`: the firmware's
/// interface over the platform's over the raw one, as systemd picks.
pub fn find(root: &Path) -> Option<Backlight> {
    let mut found: Vec<(u8, Backlight)> = std::fs::read_dir(root)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let rank = match std::fs::read_to_string(path.join("type")).ok()?.trim() {
                "firmware" => 0,
                "platform" => 1,
                _ => 2,
            };
            Some((rank, light(path, Kind::Screen)?))
        })
        .collect();
    found.sort_by(|a, b| (a.0, &a.1.name).cmp(&(b.0, &b.1.name)));
    found.into_iter().next().map(|(_, backlight)| backlight)
}

/// The keyboard's backlight, from the LEDs in `root`: the first named like
/// `tpacpi::kbd_backlight`, as UPower picks.
pub fn find_keyboard(root: &Path) -> Option<Backlight> {
    let mut found: Vec<Backlight> = std::fs::read_dir(root)
        .ok()?
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .contains("kbd_backlight")
        })
        .filter_map(|entry| light(entry.path(), Kind::Keyboard))
        .collect();
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found.into_iter().next()
}

/// Sets the raw value, through logind or else the file.
pub async fn set(
    connection: Option<&Connection>,
    backlight: &Backlight,
    value: u32,
) -> Result<(), String> {
    let mut problem = String::from("logind isn't reachable");
    if let Some(connection) = connection {
        let reply = connection
            .call_method(
                Some("org.freedesktop.login1"),
                "/org/freedesktop/login1/session/auto",
                Some("org.freedesktop.login1.Session"),
                "SetBrightness",
                &(backlight.kind.subsystem(), backlight.name.as_str(), value),
            )
            .await;
        match reply {
            Ok(_) => return Ok(()),
            Err(error) => problem = error.to_string(),
        }
    }
    tokio::fs::write(backlight.path.join("brightness"), value.to_string())
        .await
        .map_err(|error| format!("can't set {}: {problem}, and {error}", backlight.name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("mochi-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        root
    }

    fn device(root: &Path, name: &str, kind: &str, value: u32, max: u32) {
        let path = root.join(name);
        std::fs::create_dir_all(&path).unwrap();
        if !kind.is_empty() {
            std::fs::write(path.join("type"), format!("{kind}\n")).unwrap();
        }
        std::fs::write(path.join("brightness"), format!("{value}\n")).unwrap();
        std::fs::write(path.join("max_brightness"), format!("{max}\n")).unwrap();
    }

    fn keyboard(value: u32, max: u32) -> Backlight {
        Backlight {
            name: "kbd".into(),
            path: PathBuf::new(),
            value,
            max,
            kind: Kind::Keyboard,
        }
    }

    #[test]
    fn the_firmware_backlight_wins() {
        let root = scratch("backlight");
        assert_eq!(find(&root), None);
        device(&root, "intel_backlight", "raw", 9600, 19200);
        device(&root, "acpi_video0", "firmware", 30, 100);
        let found = find(&root).unwrap();
        assert_eq!(found.name, "acpi_video0");
        assert_eq!(found.percent(), 30);
        std::fs::remove_dir_all(root.join("acpi_video0")).unwrap();
        let found = find(&root).unwrap();
        assert_eq!(
            (found.name.as_str(), found.percent()),
            ("intel_backlight", 50)
        );
        std::fs::write(found.path.join("brightness"), "4800").unwrap();
        assert_eq!(found.read(), Some(4800));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn finds_the_keyboard_among_the_leds() {
        let root = scratch("leds");
        assert_eq!(find_keyboard(&root), None);
        device(&root, "input3::capslock", "", 0, 1);
        device(&root, "input3::numlock", "", 1, 1);
        assert_eq!(find_keyboard(&root), None);
        device(&root, "tpacpi::kbd_backlight", "", 1, 2);
        // One without levels doesn't count.
        device(&root, "asus::kbd_backlight", "", 0, 0);
        let found = find_keyboard(&root).unwrap();
        assert_eq!(found.name, "tpacpi::kbd_backlight");
        assert_eq!((found.kind, found.percent()), (Kind::Keyboard, 50));
        std::fs::write(found.path.join("brightness"), "2\n").unwrap();
        assert_eq!(found.read(), Some(2));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn percents_map_to_raw_values() {
        let backlight = Backlight {
            name: "panel".into(),
            path: PathBuf::new(),
            value: 0,
            max: 255,
            kind: Kind::Screen,
        };
        assert_eq!(backlight.value_for(100), 255);
        assert_eq!(backlight.value_for(50), 128);
        // Never off.
        assert_eq!(backlight.value_for(0), 1);
        assert_eq!(percent(128, 255), 50);
        assert_eq!(percent(5, 0), 0);
        // A keyboard turns off.
        assert_eq!(keyboard(1, 3).value_for(0), 0);
        assert_eq!(keyboard(1, 3).value_for(60), 2);
    }

    #[test]
    fn a_step_reaches_the_next_keyboard_level() {
        // Two levels: off, 50% and 100%.
        assert_eq!(keyboard(1, 2).value_moved_to(55), 2);
        assert_eq!(keyboard(1, 2).value_moved_to(45), 0);
        assert_eq!(keyboard(2, 2).value_moved_to(100), 2);
        assert_eq!(keyboard(0, 2).value_moved_to(0), 0);
        // A big step goes as far as it says.
        assert_eq!(keyboard(0, 2).value_moved_to(100), 2);
        // Many levels move as usual.
        assert_eq!(keyboard(50, 100).value_moved_to(55), 55);
    }
}
