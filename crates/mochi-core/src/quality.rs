//! Quality presets for recordings and screen shares: a frame rate and a
//! resolution, which the pickers cycle through.

use serde::{Deserialize, Serialize};

/// The frame rates the pickers offer, in frames per second.
pub const FRAMERATES: [u32; 5] = [15, 30, 60, 90, 120];

/// The next preset after `framerate`, wrapping around. A rate between
/// presets goes to the next one up.
pub fn next_framerate(framerate: u32) -> u32 {
    FRAMERATES
        .into_iter()
        .find(|preset| *preset > framerate)
        .unwrap_or(FRAMERATES[0])
}

/// How tall a video is at most. A source no taller keeps its size: nothing
/// is ever scaled up.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Resolution {
    /// The source's own size.
    #[default]
    #[serde(rename = "native")]
    Native,
    #[serde(rename = "480p")]
    P480,
    #[serde(rename = "720p")]
    P720,
    #[serde(rename = "1080p")]
    P1080,
    #[serde(rename = "1440p")]
    P1440,
}

impl Resolution {
    pub const ALL: [Self; 5] = [
        Self::Native,
        Self::P480,
        Self::P720,
        Self::P1080,
        Self::P1440,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::P480 => "480p",
            Self::P720 => "720p",
            Self::P1080 => "1080p",
            Self::P1440 => "1440p",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|resolution| resolution.as_str() == text)
    }

    /// The most lines, or `None` for the source's own.
    pub fn height(self) -> Option<u32> {
        match self {
            Self::Native => None,
            Self::P480 => Some(480),
            Self::P720 => Some(720),
            Self::P1080 => Some(1080),
            Self::P1440 => Some(1440),
        }
    }

    /// The next preset, wrapping around.
    pub fn next(self) -> Self {
        let index = Self::ALL.iter().position(|resolution| *resolution == self);
        Self::ALL[index.map_or(0, |index| (index + 1) % Self::ALL.len())]
    }

    /// [`Resolution::fit`], for a size as a pair.
    pub fn fit_within(self, (width, height): (u32, u32)) -> (u32, u32) {
        self.fit(width, height)
    }

    /// The size a `width` × `height` source gets: scaled down to this many
    /// lines with its shape kept, in even numbers as encoders want, or kept
    /// as it is when it's no taller.
    pub fn fit(self, width: u32, height: u32) -> (u32, u32) {
        match self.height() {
            Some(lines) if height > lines && height > 0 => {
                let scaled = u64::from(width) * u64::from(lines) / u64::from(height);
                let even = |value: u64| u32::try_from(value.max(2) & !1).unwrap_or(u32::MAX);
                (even(scaled), even(u64::from(lines)))
            }
            _ => (width, height),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycles_through_the_framerates() {
        assert_eq!(next_framerate(15), 30);
        assert_eq!(next_framerate(60), 90);
        assert_eq!(next_framerate(120), 15);
        assert_eq!(next_framerate(144), 15);
        assert_eq!(next_framerate(50), 60);
    }

    #[test]
    fn cycles_through_the_resolutions() {
        assert_eq!(Resolution::Native.next(), Resolution::P480);
        assert_eq!(Resolution::P1440.next(), Resolution::Native);
        assert_eq!(Resolution::parse("720p"), Some(Resolution::P720));
        assert_eq!(Resolution::parse("4k"), None);
    }

    #[test]
    fn scales_down_only_keeping_the_shape() {
        assert_eq!(Resolution::P720.fit(1920, 1080), (1280, 720));
        assert_eq!(Resolution::P480.fit(1920, 1080), (852, 480));
        // An ultrawide keeps its shape.
        assert_eq!(Resolution::P1080.fit(3440, 1440), (2580, 1080));
        // Never scaled up.
        assert_eq!(Resolution::P1440.fit(1920, 1080), (1920, 1080));
        assert_eq!(Resolution::Native.fit(1920, 1080), (1920, 1080));
    }

    #[test]
    fn reads_and_writes_the_settings_names() {
        #[derive(Deserialize)]
        struct Settings {
            resolution: Resolution,
        }
        let settings: Settings = toml::from_str(r#"resolution = "1080p""#).unwrap();
        assert_eq!(settings.resolution, Resolution::P1080);
        assert!(toml::from_str::<Settings>(r#"resolution = "8k""#).is_err());
    }
}
