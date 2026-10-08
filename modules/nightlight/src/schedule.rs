//! When night is: between two times of day, or between sunset and sunrise
//! where you are, with a fade at each end.

use std::time::SystemTime;

/// Minutes in a day.
const DAY: f64 = 1440.0;

/// The local time: minutes since midnight, the day of the year from 1, and
/// the offset from UTC in minutes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Now {
    pub minute: f64,
    pub day: u32,
    pub offset: f64,
}

impl Now {
    pub fn local() -> Self {
        let seconds = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |since| since.as_secs());
        let time = libc::time_t::try_from(seconds).unwrap_or_default();
        // SAFETY: `tm` is plain data that `localtime_r` fills in completely;
        // an all-zero value is a valid starting point.
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        // SAFETY: both pointers are valid for the call, and `localtime_r` is
        // the thread-safe variant.
        if unsafe { libc::localtime_r(&time, &mut tm) }.is_null() {
            return Self {
                minute: 0.0,
                day: 1,
                offset: 0.0,
            };
        }
        Self {
            minute: f64::from(tm.tm_hour * 60 + tm.tm_min) + f64::from(tm.tm_sec) / 60.0,
            day: u32::try_from(tm.tm_yday + 1).unwrap_or(1),
            offset: tm.tm_gmtoff as f64 / 60.0,
        }
    }
}

/// A time of day, `HH:MM`, as minutes since midnight.
pub fn parse_time(text: &str) -> Result<f64, String> {
    let wrong = || format!("{text} isn't a time like 20:00");
    let (hours, minutes) = text.trim().split_once(':').ok_or_else(wrong)?;
    let hours: u32 = hours.parse().map_err(|_| wrong())?;
    let minutes: u32 = minutes.parse().map_err(|_| wrong())?;
    if hours > 23 || minutes > 59 {
        return Err(wrong());
    }
    Ok(f64::from(hours * 60 + minutes))
}

/// Where the sun is: sunrise and sunset in local minutes since midnight.
/// When the sun doesn't cross the horizon that day,
/// `Sun::Day` means it stays up and `Sun::Night` that it stays down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sun {
    Crosses { rise: f64, set: f64 },
    Day,
    Night,
}

/// NOAA's approximation of sunrise and sunset, good to a minute or two.
pub fn sun(latitude: f64, longitude: f64, now: Now) -> Sun {
    let year = 2.0 * std::f64::consts::PI / 365.0 * (f64::from(now.day) - 1.0);
    let equation = 229.18
        * (0.000_075 + 0.001_868 * year.cos()
            - 0.032_077 * year.sin()
            - 0.014_615 * (2.0 * year).cos()
            - 0.040_849 * (2.0 * year).sin());
    let declination = 0.006_918 - 0.399_912 * year.cos() + 0.070_257 * year.sin()
        - 0.006_758 * (2.0 * year).cos()
        + 0.000_907 * (2.0 * year).sin()
        - 0.002_697 * (3.0 * year).cos()
        + 0.001_48 * (3.0 * year).sin();
    let latitude = latitude.to_radians();
    let cosine = 90.833_f64.to_radians().cos() / (latitude.cos() * declination.cos())
        - latitude.tan() * declination.tan();
    if cosine > 1.0 {
        return Sun::Night;
    }
    if cosine < -1.0 {
        return Sun::Day;
    }
    let angle = cosine.acos().to_degrees();
    let local = |utc: f64| (utc + now.offset).rem_euclid(DAY);
    Sun::Crosses {
        rise: local(720.0 - 4.0 * (longitude + angle) - equation),
        set: local(720.0 - 4.0 * (longitude - angle) - equation),
    }
}

/// How far into the night `minute` is, from 0 by day to 1 at night, with
/// a linear fade of `fade` minutes after `start` and before `end`. Night
/// may cross midnight.
pub fn night(minute: f64, start: f64, end: f64, fade: f64) -> f64 {
    let length = (end - start).rem_euclid(DAY);
    let since = (minute - start).rem_euclid(DAY);
    if since >= length {
        return 0.0;
    }
    // A fade can't be longer than half the night.
    let fade = fade.min(length / 2.0);
    if fade <= 0.0 {
        return 1.0;
    }
    let until = length - since;
    (since / fade).min(until / fade).min(1.0)
}

/// The temperature between `day` and `night` kelvin, `amount` of the way.
pub fn blend(day: u32, night: u32, amount: f64) -> u32 {
    let amount = amount.clamp(0.0, 1.0);
    (f64::from(day) + (f64::from(night) - f64::from(day)) * amount).round() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_parse() {
        assert_eq!(parse_time("20:00"), Ok(1200.0));
        assert_eq!(parse_time("07:30"), Ok(450.0));
        assert!(parse_time("24:00").is_err());
        assert!(parse_time("8pm").is_err());
    }

    #[test]
    fn night_crosses_midnight_and_fades() {
        let (start, end) = (1200.0, 420.0);
        assert_eq!(night(720.0, start, end, 30.0), 0.0);
        assert_eq!(night(1215.0, start, end, 30.0), 0.5);
        assert_eq!(night(0.0, start, end, 30.0), 1.0);
        assert_eq!(night(405.0, start, end, 30.0), 0.5);
        assert_eq!(night(420.0, start, end, 30.0), 0.0);
        // Without a fade, at once.
        assert_eq!(night(1200.0, start, end, 0.0), 1.0);
        // Within one day.
        assert_eq!(night(60.0, 30.0, 90.0, 0.0), 1.0);
        assert_eq!(night(100.0, 30.0, 90.0, 0.0), 0.0);
    }

    #[test]
    fn blends_between_temperatures() {
        assert_eq!(blend(6500, 4000, 0.0), 6500);
        assert_eq!(blend(6500, 4000, 0.5), 5250);
        assert_eq!(blend(6500, 4000, 2.0), 4000);
    }

    #[test]
    fn the_sun_rises_and_sets_where_you_are() {
        // Paris at the June solstice, in UTC: sunrise near 03:47, sunset
        // near 19:58.
        let now = Now {
            minute: 0.0,
            day: 172,
            offset: 0.0,
        };
        let Sun::Crosses { rise, set } = sun(48.85, 2.35, now) else {
            panic!("the sun crosses the horizon in Paris");
        };
        assert!((rise - 227.0).abs() < 5.0, "{rise}");
        assert!((set - 1198.0).abs() < 5.0, "{set}");
        // The same in Paris's summer time.
        let summer = Now {
            offset: 120.0,
            ..now
        };
        let Sun::Crosses { rise, .. } = sun(48.85, 2.35, summer) else {
            unreachable!()
        };
        assert!((rise - 347.0).abs() < 5.0, "{rise}");
        // Tromsø: midnight sun in June, polar night in December.
        assert_eq!(sun(69.65, 18.96, now), Sun::Day);
        assert_eq!(sun(69.65, 18.96, Now { day: 355, ..now }), Sun::Night);
    }
}
