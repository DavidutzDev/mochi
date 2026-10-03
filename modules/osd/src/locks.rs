//! Caps Lock and Num Lock, read from the kernel's keyboard LEDs.
//!
//! The kernel doesn't notify changes to an LED's `brightness` file, so this
//! polls. Each poll reads a few one-byte files, which costs nothing
//! measurable. A lock counts as on when any keyboard's LED is on, since every
//! keyboard has its own set of LEDs.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::sync::mpsc::UnboundedSender;

use crate::notice::{Change, LockKey, Locks};

const LEDS: &str = "/sys/class/leds";
const POLL: Duration = Duration::from_millis(100);
/// Keyboards come and go, so the LED list is refreshed every few seconds.
const POLLS_PER_SCAN: u32 = 30;

pub async fn watch(changes: UnboundedSender<Change>) {
    let mut interval = tokio::time::interval(POLL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let mut leds = Vec::new();
    let mut polls = POLLS_PER_SCAN;
    let mut last = None;

    loop {
        interval.tick().await;
        if changes.is_closed() {
            return;
        }
        if polls == POLLS_PER_SCAN {
            leds = scan(Path::new(LEDS));
            polls = 0;
        }
        polls += 1;

        let locks = read(&leds);
        if last != Some(locks) {
            last = Some(locks);
            let _ = changes.send(Change::Locks(locks));
        }
    }
}

/// The `brightness` files of every Caps Lock and Num Lock LED in `dir`.
fn scan(dir: &Path) -> Vec<(LockKey, PathBuf)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let key = if name.ends_with("::capslock") {
                LockKey::Caps
            } else if name.ends_with("::numlock") {
                LockKey::Num
            } else {
                return None;
            };
            Some((key, entry.path().join("brightness")))
        })
        .collect()
}

fn read(leds: &[(LockKey, PathBuf)]) -> Locks {
    let on = |wanted: LockKey| {
        leds.iter()
            .filter(|(key, _)| *key == wanted)
            .any(|(_, path)| fs::read_to_string(path).is_ok_and(|value| value.trim() != "0"))
    };
    Locks {
        caps: on(LockKey::Caps),
        num: on(LockKey::Num),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leds(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mochi-leds-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for (led, brightness) in files {
            fs::create_dir_all(dir.join(led)).unwrap();
            fs::write(dir.join(led).join("brightness"), brightness).unwrap();
        }
        dir
    }

    #[test]
    fn finds_lock_leds_and_ignores_others() {
        let dir = leds(
            "scan",
            &[
                ("input15::capslock", "0\n"),
                ("input15::numlock", "1\n"),
                ("input15::scrolllock", "0\n"),
                ("input21::charging", "1\n"),
            ],
        );
        let found = scan(&dir);
        assert_eq!(found.len(), 2);
        assert_eq!(
            read(&found),
            Locks {
                caps: false,
                num: true
            }
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn any_keyboard_with_the_led_on_counts() {
        let dir = leds(
            "keyboards",
            &[("input15::capslock", "0\n"), ("input21::capslock", "1\n")],
        );
        assert!(read(&scan(&dir)).caps);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_missing_directory_means_no_locks() {
        let found = scan(Path::new("/nonexistent/leds"));
        assert!(found.is_empty());
        assert_eq!(read(&found), Locks::default());
    }
}
