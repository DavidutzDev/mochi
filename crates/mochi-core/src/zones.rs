//! Time zones' offsets from UTC, for clocks in other zones. QML has no time
//! zone database, so a module reads each zone's offset from the system with
//! `date` and publishes them; a view moves UTC by the offset. The core's
//! `ClockTime` reads them as `zones` and `unknownZones` from the state it's
//! given. Offsets change twice a year at most, so reading them again every
//! [`REFRESH`] keeps clocks right across daylight saving.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::process::Command;

/// How often a module reads its zones again, for daylight saving.
pub const REFRESH: Duration = Duration::from_secs(600);

/// The zones a world clock shows until it's told others.
pub const DEFAULT: [&str; 3] = ["Europe/London", "America/New_York", "Asia/Tokyo"];

/// What the system said of some zones: the offsets of those it has, and
/// the names of those it doesn't.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Offsets {
    /// Seconds east of UTC, by zone.
    pub known: BTreeMap<String, i64>,
    pub unknown: BTreeSet<String>,
}

impl Offsets {
    /// Asks the system for each zone's offset now.
    pub async fn read(zones: impl IntoIterator<Item = String>) -> Self {
        let mut offsets = Self::default();
        for zone in zones {
            match utc_offset(&zone).await {
                Some(offset) => {
                    offsets.known.insert(zone, offset);
                }
                None => {
                    tracing::warn!(%zone, "unknown time zone");
                    // Not asked again every turn, and the clocks say so.
                    offsets.unknown.insert(zone);
                }
            }
        }
        offsets
    }

    /// Whether one of `wanted` was never asked about, like a zone just
    /// added to the settings.
    pub fn missing<'a>(&self, wanted: impl IntoIterator<Item = &'a String>) -> bool {
        wanted
            .into_iter()
            .any(|zone| !self.known.contains_key(zone) && !self.unknown.contains(zone))
    }

    /// The two fields `ClockTime` reads: `zones`, the offsets by zone, and
    /// `unknownZones`.
    pub fn fields(&self) -> (Value, Value) {
        (json!(self.known), json!(self.unknown))
    }
}

/// The zones a setting names: a text with commas or spaces between them,
/// or a list. Empty names go.
pub fn list(value: &Value) -> Vec<String> {
    let names: Vec<&str> = match value {
        Value::String(text) => text.split([',', ' ']).collect(),
        Value::Array(zones) => zones.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    };
    names
        .into_iter()
        .map(str::trim)
        .filter(|zone| !zone.is_empty())
        .map(str::to_owned)
        .collect()
}

/// A zone's offset from UTC now, in seconds, from `date`. `None` for a
/// zone the system doesn't have, which `date` would quietly take as UTC.
pub async fn utc_offset(zone: &str) -> Option<i64> {
    if zone.contains("..") {
        return None;
    }
    let file = zone_dirs()
        .map(|dir| dir.join(zone))
        .find(|file| file.is_file())?;
    // The file's whole path, so `date` finds it also where the C library
    // looks elsewhere, like NixOS without TZDIR set.
    let output = Command::new("date")
        .arg("+%z")
        .env("TZ", format!(":{}", file.display()))
        .output()
        .await
        .ok()?;
    parse_offset(String::from_utf8(output.stdout).ok()?.trim())
}

/// Where the system keeps its time zones: `TZDIR`, or the usual places.
fn zone_dirs() -> impl Iterator<Item = PathBuf> {
    std::env::var_os("TZDIR")
        .map(PathBuf::from)
        .into_iter()
        .chain(["/usr/share/zoneinfo", "/etc/zoneinfo"].map(PathBuf::from))
}

/// `+0200` or `-0330` as seconds.
fn parse_offset(text: &str) -> Option<i64> {
    let (sign, digits) = match text.as_bytes().first()? {
        b'+' => (1, &text[1..]),
        b'-' => (-1, &text[1..]),
        _ => return None,
    };
    if digits.len() != 4 {
        return None;
    }
    let hours: i64 = digits[..2].parse().ok()?;
    let minutes: i64 = digits[2..].parse().ok()?;
    Some(sign * (hours * 3600 + minutes * 60))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_utc_offsets() {
        assert_eq!(parse_offset("+0200"), Some(7200));
        assert_eq!(parse_offset("-0330"), Some(-12600));
        assert_eq!(parse_offset("0200"), None);
        assert_eq!(parse_offset("+02"), None);
    }

    #[tokio::test]
    async fn asks_the_system_for_offsets() {
        assert_eq!(utc_offset("Not/A_Zone").await, None);
        assert_eq!(utc_offset("../../etc/passwd").await, None);
        // Where the system has a time zone database, as a desktop does.
        if zone_dirs().any(|dir| dir.join("Asia/Kolkata").is_file()) {
            assert_eq!(utc_offset("UTC").await, Some(0));
            // No daylight saving there, and half an hour off the hour.
            assert_eq!(utc_offset("Asia/Kolkata").await, Some(5 * 3600 + 1800));
            let read = Offsets::read(["Asia/Kolkata".to_owned(), "Not/A_Zone".to_owned()]).await;
            assert_eq!(read.known["Asia/Kolkata"], 5 * 3600 + 1800);
            assert!(read.unknown.contains("Not/A_Zone"));
        }
    }

    #[test]
    fn a_zone_never_asked_about_is_missing() {
        let mut offsets = Offsets::default();
        let wanted = ["UTC".to_owned(), "Not/A_Zone".to_owned()];
        assert!(offsets.missing(&wanted));
        offsets.known.insert("UTC".into(), 0);
        assert!(offsets.missing(&wanted));
        // An unknown zone isn't asked again every turn.
        offsets.unknown.insert("Not/A_Zone".into());
        assert!(!offsets.missing(&wanted));
        let (zones, unknown) = offsets.fields();
        assert_eq!(zones, json!({ "UTC": 0 }));
        assert_eq!(unknown, json!(["Not/A_Zone"]));
    }

    #[test]
    fn reads_a_list_of_zones() {
        assert_eq!(
            list(&json!(" Europe/London,America/New_York  Asia/Kolkata, ")),
            ["Europe/London", "America/New_York", "Asia/Kolkata"]
        );
        assert_eq!(
            list(&json!(["UTC", " ", "Asia/Tokyo"])),
            ["UTC", "Asia/Tokyo"]
        );
        assert!(list(&json!(null)).is_empty());
    }
}
