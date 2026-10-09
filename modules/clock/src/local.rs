//! The local time, through libc: a date and a time of day here as seconds
//! since the epoch, and back. Reminders are kept as those seconds, so one
//! that came due while mochid was down or the computer slept is due the
//! moment either is back.

use std::fmt;

/// A day, as the calendar here shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub year: i32,
    /// 1 to 12.
    pub month: u32,
    /// 1 to 31.
    pub day: u32,
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// A moment, as the clock here shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Local {
    pub date: Date,
    pub hour: u32,
    pub minute: u32,
    /// 0 is Sunday, like JavaScript's getDay().
    pub weekday: u32,
}

/// `seconds` since the epoch, here.
pub fn from_epoch(seconds: i64) -> Option<Local> {
    let time = libc::time_t::try_from(seconds).ok()?;
    // SAFETY: `tm` is plain data that `localtime_r` fills in completely; an
    // all-zero value is a valid starting point.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    // SAFETY: both pointers are valid for the call, and `localtime_r` is the
    // thread-safe variant.
    if unsafe { libc::localtime_r(&time, &mut tm) }.is_null() {
        return None;
    }
    Some(Local {
        date: Date {
            year: tm.tm_year + 1900,
            month: u32::try_from(tm.tm_mon + 1).ok()?,
            day: u32::try_from(tm.tm_mday).ok()?,
        },
        hour: u32::try_from(tm.tm_hour).ok()?,
        minute: u32::try_from(tm.tm_min).ok()?,
        weekday: u32::try_from(tm.tm_wday).ok()?,
    })
}

/// `hour:minute` on `date` here, in seconds since the epoch. `None` for a
/// day that doesn't exist, like 31 February.
pub fn to_epoch(date: Date, hour: u32, minute: u32) -> Option<i64> {
    // SAFETY: as in `from_epoch`, an all-zero `tm` is valid.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    tm.tm_year = date.year - 1900;
    tm.tm_mon = i32::try_from(date.month).ok()? - 1;
    tm.tm_mday = i32::try_from(date.day).ok()?;
    tm.tm_hour = i32::try_from(hour).ok()?;
    tm.tm_min = i32::try_from(minute).ok()?;
    // Whether daylight saving applies then is for mktime to work out.
    tm.tm_isdst = -1;
    // SAFETY: the pointer is valid for the call; mktime only normalizes the
    // fields it was given.
    let seconds = unsafe { libc::mktime(&mut tm) };
    if seconds == -1 {
        return None;
    }
    // time_t is i64 here, and may be narrower elsewhere.
    #[allow(clippy::useless_conversion)]
    let seconds = i64::from(seconds);
    // mktime takes 31 February as 3 March; that's no date.
    let back = from_epoch(seconds)?;
    (back.date == date).then_some(seconds)
}

/// `2026-10-21`, `today` or `tomorrow`, from `today`.
pub fn parse_date(text: &str, today: Date) -> Result<Date, String> {
    let text = text.trim();
    match text {
        "today" => return Ok(today),
        "tomorrow" => {
            // Noon, so a day without midnight still moves on by one.
            let noon = to_epoch(today, 12, 0).ok_or("today isn't a date here")?;
            return Ok(from_epoch(noon + 86_400).ok_or("no tomorrow")?.date);
        }
        _ => {}
    }
    let wrong = || format!("{text} isn't a date like 2026-10-21, today or tomorrow");
    let mut parts = text.split('-');
    let mut next = || parts.next().and_then(|part| part.parse().ok());
    let year: i32 = next().ok_or_else(wrong)?;
    let month = next().ok_or_else(wrong)?;
    let day = next().ok_or_else(wrong)?;
    if parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(wrong());
    }
    let date = Date {
        year,
        month: month as u32,
        day: day as u32,
    };
    to_epoch(date, 12, 0).map(|_| date).ok_or_else(wrong)
}

/// `20:00` as hours and minutes.
pub fn parse_time(text: &str) -> Result<(u32, u32), String> {
    let wrong = || format!("{} isn't a time like 20:00", text.trim());
    let (hours, minutes) = text.trim().split_once(':').ok_or_else(wrong)?;
    let hours: u32 = hours.parse().map_err(|_| wrong())?;
    let minutes: u32 = minutes.parse().map_err(|_| wrong())?;
    if hours > 23 || minutes > 59 {
        return Err(wrong());
    }
    Ok((hours, minutes))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: Date = Date {
        year: 2026,
        month: 10,
        day: 21,
    };

    #[test]
    fn a_time_here_goes_there_and_back() {
        let seconds = to_epoch(DAY, 20, 5).unwrap();
        let back = from_epoch(seconds).unwrap();
        assert_eq!(back.date, DAY);
        assert_eq!((back.hour, back.minute), (20, 5));
        // 21 October 2026 is a Wednesday.
        assert_eq!(back.weekday, 3);
        // A minute later is 60 seconds later.
        assert_eq!(to_epoch(DAY, 20, 6).unwrap() - seconds, 60);
    }

    #[test]
    fn no_such_day() {
        let february = Date {
            year: 2026,
            month: 2,
            day: 31,
        };
        assert_eq!(to_epoch(february, 9, 0), None);
        assert!(parse_date("2026-02-31", DAY).is_err());
    }

    #[test]
    fn reads_dates() {
        assert_eq!(parse_date("2026-10-21", DAY), Ok(DAY));
        assert_eq!(parse_date("today", DAY), Ok(DAY));
        assert_eq!(
            parse_date("tomorrow", DAY).unwrap(),
            Date {
                year: 2026,
                month: 10,
                day: 22
            }
        );
        let new_year = Date {
            year: 2026,
            month: 12,
            day: 31,
        };
        assert_eq!(
            parse_date("tomorrow", new_year).unwrap(),
            Date {
                year: 2027,
                month: 1,
                day: 1
            }
        );
        for wrong in [
            "21/10/2026",
            "2026-13-01",
            "2026-10",
            "2026-10-21-1",
            "soon",
        ] {
            assert!(parse_date(wrong, DAY).is_err(), "{wrong}");
        }
        assert_eq!(DAY.to_string(), "2026-10-21");
    }

    #[test]
    fn reads_times() {
        assert_eq!(parse_time("20:00"), Ok((20, 0)));
        assert_eq!(parse_time(" 7:05 "), Ok((7, 5)));
        for wrong in ["24:00", "12:60", "noon", "12", "-1:00"] {
            assert!(parse_time(wrong).is_err(), "{wrong}");
        }
    }
}
