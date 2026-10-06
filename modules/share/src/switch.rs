//! The switchable share. The portal can't change what a running share
//! captures, so the app shares a monitor of Mochi's own instead, `OUTPUT`,
//! and Mochi draws a live copy of the chosen screen, window or area on it.
//! Picking another source changes the copy, and the app never knows.
//!
//! Hyprland reports what each capture takes. Mochi's copy takes the real
//! screens, so a capture of `OUTPUT` is the app's. Once none has run for a
//! few seconds, the share is over and the monitor goes.

use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// The monitor the app shares.
pub const OUTPUT: &str = "MOCHI-SHARE";

/// How long `OUTPUT` may go uncaptured before the share counts as over, in
/// case the app restarts its capture.
const GRACE: Duration = Duration::from_secs(3);
/// How long the app has to start capturing once it got the monitor.
const START: Duration = Duration::from_secs(30);
/// How long a capture of `OUTPUT` must last to be the app's: a
/// screenshot's is far shorter.
const BELIEVE: Duration = Duration::from_millis(1500);

/// What the copy shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Screen(String),
    /// A window, by Hyprland's address, `0x…`.
    Window {
        address: String,
        title: String,
    },
    /// Part of a monitor, in logical pixels.
    Area {
        output: String,
        x: i64,
        y: i64,
        width: i64,
        height: i64,
    },
}

impl Source {
    pub fn to_json(&self) -> Value {
        match self {
            Self::Screen(output) => json!({ "kind": "screen", "output": output }),
            Self::Window { address, title } => {
                json!({ "kind": "window", "address": address, "title": title })
            }
            Self::Area {
                output,
                x,
                y,
                width,
                height,
            } => json!({
                "kind": "area",
                "output": output,
                "x": x,
                "y": y,
                "width": width,
                "height": height,
            }),
        }
    }
}

/// A switchable share, from the app's answer until it stops.
#[derive(Debug)]
pub struct Session {
    pub source: Source,
    /// The monitor's refresh rate and resolution now.
    pub framerate: u32,
    pub resolution: mochi_core::quality::Resolution,
    /// The monitor's size at native resolution, from the first source.
    pub size: (u32, u32),
    started: Instant,
    /// The app has captured the monitor at some point.
    shared: bool,
    /// Since when `OUTPUT` is captured, while it is.
    busy_since: Option<Instant>,
    /// Since when it isn't, while it isn't.
    quiet_since: Option<Instant>,
}

impl Session {
    pub fn new(source: Source, now: Instant) -> Self {
        Self {
            source,
            framerate: 60,
            resolution: mochi_core::quality::Resolution::Native,
            size: (1920, 1080),
            started: now,
            shared: false,
            busy_since: None,
            quiet_since: Some(now),
        }
    }

    /// Follows whether something captures `OUTPUT`.
    pub fn captured(&mut self, captured: bool, now: Instant) {
        if captured {
            self.busy_since.get_or_insert(now);
            self.quiet_since = None;
        } else {
            if self
                .busy_since
                .take()
                .is_some_and(|since| now.duration_since(since) >= BELIEVE)
            {
                self.shared = true;
            }
            self.quiet_since.get_or_insert(now);
        }
    }

    /// When the share may be over, if the count stays as it is.
    pub fn deadline(&self) -> Option<Instant> {
        let quiet = self.quiet_since?;
        Some(if self.shared {
            quiet + GRACE
        } else {
            self.started + START
        })
    }

    pub fn ended(&self, now: Instant) -> bool {
        self.deadline().is_some_and(|deadline| deadline <= now)
    }

    pub fn payload(&self) -> Value {
        json!({ "output": OUTPUT, "source": self.source.to_json() })
    }
}

/// The size of the switchable monitor, in pixels: the shape of what's
/// shared first, as large as fits in the largest real monitor, so it fills
/// the copy without bars and keeps its sharpness. A window changes shape
/// while it's shared, so it gets the largest monitor's.
pub fn size(outputs: &[(String, u32, u32)], first: &Source) -> (u32, u32) {
    let (width, height) = largest(outputs);
    let shape = match first {
        Source::Screen(name) => outputs
            .iter()
            .find(|(output, ..)| output == name)
            .map(|(_, width, height)| (u64::from(*width), u64::from(*height))),
        Source::Area { width, height, .. } => Some((width.unsigned_abs(), height.unsigned_abs())),
        Source::Window { .. } => None,
    };
    let Some((shape_width, shape_height)) =
        shape.filter(|(width, height)| *width > 0 && *height > 0)
    else {
        return (width, height);
    };
    let (width, height) = (u64::from(width), u64::from(height));
    let (width, height) = if shape_width * height > shape_height * width {
        (width, width * shape_height / shape_width)
    } else {
        (height * shape_width / shape_height, height)
    };
    // Even sizes, which video encoders want.
    let even = |side: u64| u32::try_from(side).unwrap_or(u32::MAX).max(2) & !1;
    (even(width), even(height))
}

/// The largest real monitor, in pixels.
fn largest(outputs: &[(String, u32, u32)]) -> (u32, u32) {
    outputs
        .iter()
        .filter(|(name, width, height)| {
            !name.starts_with(mochi_core::compositor::VIRTUAL_PREFIX) && *width > 0 && *height > 0
        })
        .map(|(_, width, height)| (*width, *height))
        .max_by_key(|(width, height)| u64::from(*width) * u64::from(*height))
        .unwrap_or((1920, 1080))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND: Duration = Duration::from_secs(1);

    fn session() -> (Session, Instant) {
        let now = Instant::now();
        (Session::new(Source::Screen("DP-3".into()), now), now)
    }

    #[test]
    fn ends_once_the_app_stops_capturing() {
        let (mut session, start) = session();
        // Nothing captures it yet: the app has time to start.
        session.captured(false, start);
        assert!(!session.ended(start + 10 * SECOND));

        session.captured(true, start + SECOND);
        assert_eq!(session.deadline(), None);

        // The app restarts its capture for a moment.
        session.captured(false, start + 2 * SECOND);
        session.captured(true, start + 2 * SECOND + SECOND / 2);
        assert!(!session.ended(start + 10 * SECOND));

        session.captured(false, start + 20 * SECOND);
        assert!(!session.ended(start + 22 * SECOND));
        assert!(session.ended(start + 23 * SECOND));
    }

    #[test]
    fn a_screenshot_is_not_the_app() {
        let (mut session, start) = session();
        session.captured(false, start);
        session.captured(true, start + SECOND);
        session.captured(false, start + SECOND + SECOND / 10);
        assert!(!session.ended(start + 10 * SECOND));
        assert_eq!(session.deadline(), Some(start + 30 * SECOND));
    }

    #[test]
    fn ends_when_the_app_never_captures() {
        let (mut session, start) = session();
        session.captured(false, start);
        assert!(!session.ended(start + 29 * SECOND));
        assert!(session.ended(start + 30 * SECOND));
    }

    #[test]
    fn takes_the_first_sources_shape_within_the_largest_monitor() {
        let outputs = [
            ("HDMI-A-1".to_owned(), 1920, 1200),
            ("DP-3".to_owned(), 2560, 1440),
            ("MOCHI-SHARE".to_owned(), 3840, 2160),
        ];
        let window = Source::Window {
            address: "0x1".into(),
            title: String::new(),
        };
        assert_eq!(size(&outputs, &window), (2560, 1440));
        assert_eq!(size(&[], &window), (1920, 1080));
        // A 16:10 screen, as tall as the largest.
        assert_eq!(
            size(&outputs, &Source::Screen("HDMI-A-1".into())),
            (2304, 1440)
        );
        assert_eq!(size(&outputs, &Source::Screen("DP-3".into())), (2560, 1440));
        let area = |width, height| Source::Area {
            output: "DP-3".into(),
            x: 0,
            y: 0,
            width,
            height,
        };
        // Wide and short, and tall and narrow; even sides.
        assert_eq!(size(&outputs, &area(1000, 200)), (2560, 512));
        assert_eq!(size(&outputs, &area(301, 1000)), (432, 1440));
        assert_eq!(size(&outputs, &area(0, 100)), (2560, 1440));
    }

    #[test]
    fn tells_the_view_what_to_copy() {
        let area = Source::Area {
            output: "DP-3".into(),
            x: 10,
            y: 20,
            width: 640,
            height: 360,
        };
        assert_eq!(area.to_json()["kind"], "area");
        assert_eq!(area.to_json()["width"], 640);
        let (session, _) = session();
        assert_eq!(session.payload()["output"], OUTPUT);
        assert_eq!(session.payload()["source"]["output"], "DP-3");
    }
}
