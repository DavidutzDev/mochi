//! The laptop's backlight, in `/sys/class/backlight`. Reading needs nothing;
//! writing goes through logind, which lets the user of the active session
//! set it without a udev rule, and falls back to the file for systems that
//! grant it.

use std::path::{Path, PathBuf};

use zbus::Connection;

/// The directory the kernel lists backlights in.
pub const SYSFS: &str = "/sys/class/backlight";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backlight {
    /// The device's name, like `intel_backlight`.
    pub name: String,
    pub path: PathBuf,
    pub value: u32,
    pub max: u32,
}

impl Backlight {
    pub fn percent(&self) -> u32 {
        percent(self.value, self.max)
    }

    /// The raw value for a percent; never 0, which turns some panels off.
    pub fn value_for(&self, percent: u32) -> u32 {
        let value = (f64::from(percent.min(100)) * f64::from(self.max) / 100.0).round() as u32;
        value.clamp(1, self.max.max(1))
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

/// The backlight to use, from the devices in `root`: the firmware's
/// interface over the platform's over the raw one, as systemd picks.
pub fn find(root: &Path) -> Option<Backlight> {
    let mut found: Vec<(u8, Backlight)> = std::fs::read_dir(root)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let max = read_number(&path.join("max_brightness")).filter(|max| *max > 0)?;
            let value = read_number(&path.join("brightness"))?;
            let rank = match std::fs::read_to_string(path.join("type")).ok()?.trim() {
                "firmware" => 0,
                "platform" => 1,
                _ => 2,
            };
            Some((
                rank,
                Backlight {
                    name: entry.file_name().to_string_lossy().into_owned(),
                    path,
                    value,
                    max,
                },
            ))
        })
        .collect();
    found.sort_by(|a, b| (a.0, &a.1.name).cmp(&(b.0, &b.1.name)));
    found.into_iter().next().map(|(_, backlight)| backlight)
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
                &("backlight", backlight.name.as_str(), value),
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

    fn device(root: &Path, name: &str, kind: &str, value: u32, max: u32) {
        let path = root.join(name);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("type"), format!("{kind}\n")).unwrap();
        std::fs::write(path.join("brightness"), format!("{value}\n")).unwrap();
        std::fs::write(path.join("max_brightness"), format!("{max}\n")).unwrap();
    }

    #[test]
    fn the_firmware_backlight_wins() {
        let root = std::env::temp_dir().join(format!("mochi-backlight-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
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
    fn percents_map_to_raw_values() {
        let backlight = Backlight {
            name: "panel".into(),
            path: PathBuf::new(),
            value: 0,
            max: 255,
        };
        assert_eq!(backlight.value_for(100), 255);
        assert_eq!(backlight.value_for(50), 128);
        // Never off.
        assert_eq!(backlight.value_for(0), 1);
        assert_eq!(percent(128, 255), 50);
        assert_eq!(percent(5, 0), 0);
    }
}
