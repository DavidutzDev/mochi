//! WMO weather codes, as Open-Meteo gives them: a few words for each, an
//! icon from Material Symbols for the `Symbol` view, by day and by night,
//! and a broad kind for views that pick colors or pictures by the sky.

use serde::Serialize;

/// What a code says, ready for the views.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Sky {
    /// Like "Partly cloudy".
    pub text: &'static str,
    /// A Material Symbols name, like `partly_cloudy_night`.
    pub icon: &'static str,
    /// `clear`, `partly`, `cloudy`, `fog`, `drizzle`, `rain`, `sleet`,
    /// `snow`, `storm`, or `unknown`.
    pub kind: &'static str,
}

/// The sky for `code`, by day or by night: only the clear and partly
/// cloudy icons differ.
pub fn sky(code: i64, day: bool) -> Sky {
    let (text, kind) = match code {
        0 => ("Clear", "clear"),
        1 => ("Mostly clear", "partly"),
        2 => ("Partly cloudy", "partly"),
        3 => ("Overcast", "cloudy"),
        45 => ("Fog", "fog"),
        48 => ("Freezing fog", "fog"),
        51 => ("Light drizzle", "drizzle"),
        53 => ("Drizzle", "drizzle"),
        55 => ("Heavy drizzle", "drizzle"),
        56 | 57 => ("Freezing drizzle", "sleet"),
        61 => ("Light rain", "rain"),
        63 => ("Rain", "rain"),
        65 => ("Heavy rain", "rain"),
        66 | 67 => ("Freezing rain", "sleet"),
        71 => ("Light snow", "snow"),
        73 => ("Snow", "snow"),
        75 => ("Heavy snow", "snow"),
        77 => ("Snow grains", "snow"),
        80 => ("Light showers", "rain"),
        81 => ("Showers", "rain"),
        82 => ("Heavy showers", "rain"),
        85 => ("Snow showers", "snow"),
        86 => ("Heavy snow showers", "snow"),
        95 => ("Thunderstorm", "storm"),
        96 | 99 => ("Thunderstorm with hail", "storm"),
        _ => ("Unknown", "unknown"),
    };
    let icon = match code {
        0 if day => "clear_day",
        0 => "clear_night",
        1 | 2 if day => "partly_cloudy_day",
        1 | 2 => "partly_cloudy_night",
        45 | 48 => "foggy",
        51 | 53 | 55 | 61 | 80 => "rainy_light",
        63 | 81 => "rainy",
        65 | 82 => "rainy_heavy",
        56 | 57 | 66 | 67 => "weather_mix",
        71 | 73 | 77 | 85 => "weather_snowy",
        75 | 86 => "snowing_heavy",
        95 | 96 | 99 => "thunderstorm",
        _ => "cloud",
    };
    Sky { text, icon, kind }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_read_as_words_and_icons() {
        assert_eq!(
            sky(2, true),
            Sky {
                text: "Partly cloudy",
                icon: "partly_cloudy_day",
                kind: "partly",
            }
        );
        assert_eq!(sky(2, false).icon, "partly_cloudy_night");
        assert_eq!(sky(0, true).icon, "clear_day");
        assert_eq!(sky(0, false).icon, "clear_night");
        // Rain, snow and storms look the same at night.
        assert_eq!(sky(63, false).icon, sky(63, true).icon);
        assert_eq!(sky(63, true).icon, "rainy");
        assert_eq!(sky(75, true).icon, "snowing_heavy");
        assert_eq!(sky(66, true).kind, "sleet");
        assert_eq!(sky(99, true).text, "Thunderstorm with hail");
        assert_eq!(sky(99, true).icon, "thunderstorm");
        assert_eq!(sky(45, true).icon, "foggy");
        let unknown = sky(42, true);
        assert_eq!((unknown.text, unknown.icon), ("Unknown", "cloud"));
    }

    #[test]
    fn every_code_has_words() {
        let codes = [
            0, 1, 2, 3, 45, 48, 51, 53, 55, 56, 57, 61, 63, 65, 66, 67, 71, 73, 75, 77, 80, 81, 82,
            85, 86, 95, 96, 99,
        ];
        for code in codes {
            assert_ne!(sky(code, true).kind, "unknown", "{code}");
        }
    }
}
