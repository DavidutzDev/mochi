//! A time zone's offset from UTC at some moment, read from its file in the
//! system's zone database, the TZif format of RFC 8536. The file lists the
//! moments the offset changed, and ends with a POSIX TZ rule, like
//! `CET-1CEST,M3.5.0,M10.5.0/3`, for the years after its list. Some
//! systems trim the list to the last change of rules, so the rule matters.
//!
//! Reading a file is quick and needs no other program, so the settings can
//! show the offset of every zone the system has.

/// Seconds east of UTC at `at`, seconds since the epoch, from a TZif
/// file's bytes. `None` for a file that isn't one.
pub fn offset_at(data: &[u8], at: i64) -> Option<i64> {
    let header = Header::read(data)?;
    // Version 1 has 32-bit times; later ones repeat the data with 64-bit
    // times after it, then the rule.
    let (header, block, size) = if header.version >= b'2' {
        let rest = data.get(header.length(4)..)?;
        (Header::read(rest)?, rest.get(HEADER..)?, 8)
    } else {
        (header, data.get(HEADER..)?, 4)
    };
    let times: Vec<i64> = block
        .get(..header.times * size)?
        .chunks(size)
        .map(|bytes| match size {
            4 => i64::from(i32::from_be_bytes(bytes.try_into().unwrap_or_default())),
            _ => i64::from_be_bytes(bytes.try_into().unwrap_or_default()),
        })
        .collect();
    let kinds = block.get(header.times * size..header.times * (size + 1))?;
    let types = block.get(header.times * (size + 1)..)?;
    let offset_of = |index: u8| -> Option<i64> {
        let start = usize::from(index) * 6;
        let bytes = types.get(start..start + 4)?;
        Some(i64::from(i32::from_be_bytes(bytes.try_into().ok()?)))
    };
    let after = times.partition_point(|time| *time <= at);
    // The type of the last change by `at`; before the first, the first type.
    let listed = match after.checked_sub(1) {
        Some(last) => offset_of(*kinds.get(last)?),
        None => offset_of(0),
    };
    if after < times.len() || size == 4 {
        return listed;
    }
    // After the list: the rule at the end, else the last type listed.
    let footer = block
        .get(header.length(size) - HEADER..)
        .unwrap_or_default();
    let rule = std::str::from_utf8(footer).ok().and_then(|footer| {
        let footer = footer.strip_prefix('\n')?;
        Rule::parse(&footer[..footer.find('\n')?])
    });
    match rule {
        Some(rule) => Some(rule.offset_at(at)),
        None => listed,
    }
}

/// The size of a TZif header.
const HEADER: usize = 44;

/// The counts in a TZif header.
#[derive(Debug, Clone, Copy)]
struct Header {
    version: u8,
    ut: usize,
    std: usize,
    leaps: usize,
    times: usize,
    types: usize,
    chars: usize,
}

impl Header {
    fn read(data: &[u8]) -> Option<Self> {
        if data.get(..4)? != b"TZif" {
            return None;
        }
        let count = |at: usize| -> Option<usize> {
            let bytes = data.get(20 + at * 4..24 + at * 4)?;
            usize::try_from(u32::from_be_bytes(bytes.try_into().ok()?)).ok()
        };
        Some(Self {
            version: *data.get(4)?,
            ut: count(0)?,
            std: count(1)?,
            leaps: count(2)?,
            times: count(3)?,
            types: count(4)?,
            chars: count(5)?,
        })
    }

    /// The header and its data, with times `size` bytes long.
    fn length(&self, size: usize) -> usize {
        HEADER
            + self.times * (size + 1)
            + self.types * 6
            + self.chars
            + self.leaps * (size + 4)
            + self.std
            + self.ut
    }
}

/// A POSIX TZ rule: the standard offset, and daylight saving's offset and
/// when it starts and ends, if the zone has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rule {
    /// Seconds east of UTC.
    standard: i64,
    saving: Option<Saving>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Saving {
    offset: i64,
    start: Change,
    end: Change,
}

/// A day of the year and a time of that day, local, in seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Change {
    day: Day,
    time: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Day {
    /// `Jn`: 1 to 365, never counting 29 February.
    Julian(u32),
    /// `n`: 0 to 365, counting 29 February.
    Ordinal(u32),
    /// `Mm.w.d`: day `d` (0 is Sunday) of week `w` (5 is the last) of month
    /// `m`.
    Month { month: u32, week: u32, weekday: u32 },
}

impl Rule {
    fn parse(text: &str) -> Option<Self> {
        let mut rest = text;
        name(&mut rest)?;
        // POSIX offsets are west of UTC.
        let standard = -duration(&mut rest)?;
        if rest.is_empty() {
            return Some(Self {
                standard,
                saving: None,
            });
        }
        name(&mut rest)?;
        let offset = if rest.starts_with(',') {
            standard + 3600
        } else {
            -duration(&mut rest)?
        };
        // Without dates, the United States' rule, as POSIX says.
        let (start, end) = match rest.strip_prefix(',') {
            Some(dates) => {
                let (start, end) = dates.split_once(',')?;
                (change(start)?, change(end)?)
            }
            None if rest.is_empty() => (change("M3.2.0")?, change("M11.1.0")?),
            None => return None,
        };
        Some(Self {
            standard,
            saving: Some(Saving { offset, start, end }),
        })
    }

    fn offset_at(&self, at: i64) -> i64 {
        let Some(saving) = self.saving else {
            return self.standard;
        };
        let year = civil(days_floor(at + self.standard)).0;
        // When each change happens, in UTC: the start in standard time and
        // the end in daylight time.
        let start = saving.start.at(year) - self.standard;
        let end = saving.end.at(year) - saving.offset;
        let saving_now = if start < end {
            start <= at && at < end
        } else {
            // South of the equator, the saving spans the new year.
            !(end <= at && at < start)
        };
        if saving_now {
            saving.offset
        } else {
            self.standard
        }
    }
}

impl Change {
    /// Seconds since the epoch, local, of this change in `year`.
    fn at(&self, year: i64) -> i64 {
        let leap = is_leap(year);
        let first = days_from_civil(year, 1, 1);
        let day = match self.day {
            Day::Julian(day) => {
                let day = i64::from(day) - 1;
                first + day + i64::from(leap && day >= 59)
            }
            Day::Ordinal(day) => first + i64::from(day),
            Day::Month {
                month,
                week,
                weekday,
            } => {
                let start = days_from_civil(year, month, 1);
                // 1 January 1970 was a Thursday.
                let first_weekday = (start + 4).rem_euclid(7);
                let mut day = start + (i64::from(weekday) - first_weekday).rem_euclid(7);
                day += 7 * (i64::from(week) - 1);
                let next = if month == 12 {
                    days_from_civil(year + 1, 1, 1)
                } else {
                    days_from_civil(year, month + 1, 1)
                };
                while day >= next {
                    day -= 7;
                }
                day
            }
        };
        day * 86_400 + self.time
    }
}

/// Takes a zone's abbreviation off the front: letters, or anything in
/// angle brackets, like `<+0530>`.
fn name(rest: &mut &str) -> Option<()> {
    let length = if let Some(quoted) = rest.strip_prefix('<') {
        quoted.find('>')? + 2
    } else {
        rest.find(|char: char| !char.is_ascii_alphabetic())
            .unwrap_or(rest.len())
    };
    if length < 3 {
        return None;
    }
    *rest = &rest[length..];
    Some(())
}

/// Takes `[+-]hh[:mm[:ss]]` off the front, as seconds.
fn duration(rest: &mut &str) -> Option<i64> {
    let sign = match rest.as_bytes().first()? {
        b'-' => -1,
        _ => 1,
    };
    let text = rest.trim_start_matches(['+', '-']);
    let length = text
        .find(|char: char| !char.is_ascii_digit() && char != ':')
        .unwrap_or(text.len());
    let mut seconds = 0;
    for (index, part) in text[..length].split(':').enumerate() {
        if index > 2 {
            return None;
        }
        let value: i64 = part.parse().ok()?;
        seconds += value * [3600, 60, 1][index];
    }
    *rest = &text[length..];
    Some(sign * seconds)
}

/// A date and an optional `/time`, 2:00 by default.
fn change(text: &str) -> Option<Change> {
    let (date, time) = match text.split_once('/') {
        Some((date, time)) => {
            let mut time = time;
            let seconds = duration(&mut time)?;
            if !time.is_empty() {
                return None;
            }
            (date, seconds)
        }
        None => (text, 7200),
    };
    let day = if let Some(day) = date.strip_prefix('J') {
        Day::Julian(day.parse().ok().filter(|day| (1..=365).contains(day))?)
    } else if let Some(rule) = date.strip_prefix('M') {
        let mut parts = rule.split('.').map(str::parse::<u32>);
        let (Some(Ok(month)), Some(Ok(week)), Some(Ok(weekday)), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return None;
        };
        if !(1..=12).contains(&month) || !(1..=5).contains(&week) || weekday > 6 {
            return None;
        }
        Day::Month {
            month,
            week,
            weekday,
        }
    } else {
        Day::Ordinal(date.parse().ok().filter(|day| *day <= 365)?)
    };
    Some(Change { day, time })
}

fn is_leap(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_floor(seconds: i64) -> i64 {
    seconds.div_euclid(86_400)
}

/// Days since 1 January 1970 of a date, after Howard Hinnant's
/// `days_from_civil`.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let of_era = year - era * 400;
    let month = i64::from(month);
    let of_year =
        (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + i64::from(day) - 1;
    let of_cycle = of_era * 365 + of_era / 4 - of_era / 100 + of_year;
    era * 146_097 + of_cycle - 719_468
}

/// The year, month and day of a day since 1 January 1970.
fn civil(days: i64) -> (i64, u32, u32) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let of_era = days - era * 146_097;
    let year_of_era = (of_era - of_era / 1460 + of_era / 36_524 - of_era / 146_096) / 365;
    let of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted = (5 * of_year + 2) / 153;
    let day = of_year - (153 * shifted + 2) / 5 + 1;
    let month = if shifted < 10 {
        shifted + 3
    } else {
        shifted - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Seconds since the epoch of a UTC date and hour.
    fn utc(year: i64, month: u32, day: u32, hour: i64) -> i64 {
        days_from_civil(year, month, day) * 86_400 + hour * 3600
    }

    #[test]
    fn counts_days_like_the_calendar() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
        assert_eq!(civil(11_017), (2000, 3, 1));
        assert_eq!(civil(-1), (1969, 12, 31));
        assert_eq!(civil(days_from_civil(2026, 10, 9)), (2026, 10, 9));
    }

    #[test]
    fn reads_rules() {
        let london = Rule::parse("GMT0BST,M3.5.0/1,M10.5.0").unwrap();
        assert_eq!(london.standard, 0);
        assert_eq!(london.saving.unwrap().offset, 3600);
        let india = Rule::parse("IST-5:30").unwrap();
        assert_eq!((india.standard, india.saving), (19_800, None));
        let quoted = Rule::parse("<-03>3").unwrap();
        assert_eq!(quoted.standard, -10_800);
        assert_eq!(Rule::parse(""), None);
        assert_eq!(Rule::parse("X1"), None);
        assert_eq!(Rule::parse("CET-1CEST,M13.5.0,M10.5.0/3"), None);
    }

    #[test]
    fn follows_daylight_saving_both_ways() {
        // Europe: the last Sundays of March and October, at 1:00 UTC.
        let paris = Rule::parse("CET-1CEST,M3.5.0,M10.5.0/3").unwrap();
        assert_eq!(paris.offset_at(utc(2026, 1, 15, 12)), 3600);
        assert_eq!(paris.offset_at(utc(2026, 3, 29, 0)), 3600);
        assert_eq!(paris.offset_at(utc(2026, 3, 29, 1)), 7200);
        assert_eq!(paris.offset_at(utc(2026, 10, 25, 0)), 7200);
        assert_eq!(paris.offset_at(utc(2026, 10, 25, 1)), 3600);
        // New York: the second Sunday of March to the first of November.
        let new_york = Rule::parse("EST5EDT,M3.2.0,M11.1.0").unwrap();
        assert_eq!(new_york.offset_at(utc(2026, 3, 8, 6)), -18_000);
        assert_eq!(new_york.offset_at(utc(2026, 3, 8, 7)), -14_400);
        assert_eq!(new_york.offset_at(utc(2026, 11, 1, 5)), -14_400);
        assert_eq!(new_york.offset_at(utc(2026, 11, 1, 6)), -18_000);
        // Sydney saves over the southern summer, across the new year.
        let sydney = Rule::parse("AEST-10AEDT,M10.1.0,M4.1.0/3").unwrap();
        assert_eq!(sydney.offset_at(utc(2026, 1, 1, 0)), 39_600);
        assert_eq!(sydney.offset_at(utc(2026, 7, 1, 0)), 36_000);
        assert_eq!(sydney.offset_at(utc(2026, 12, 1, 0)), 39_600);
        // Days by number: J60 is always 1 March.
        let julian = Rule::parse("AAA0BBB,J60/0,J305/0").unwrap();
        assert_eq!(julian.offset_at(utc(2028, 2, 29, 12)), 0);
        assert_eq!(julian.offset_at(utc(2028, 3, 1, 12)), 3600);
    }

    /// A version 2 file with two changes and a rule, as `zic -b slim`
    /// writes them.
    fn file(times: &[i64], kinds: &[u8], types: &[i32], rule: &str) -> Vec<u8> {
        let mut data = Vec::new();
        // An empty version 1 part.
        let header = |data: &mut Vec<u8>, times: usize, types: usize| {
            data.extend(b"TZif2");
            data.extend([0; 15]);
            for count in [0, 0, 0, times, types, 4] {
                data.extend(u32::try_from(count).unwrap().to_be_bytes());
            }
        };
        header(&mut data, 0, 1);
        data.extend([0, 0, 0, 0, 0, 0]);
        data.extend(b"UTC\0");
        header(&mut data, times.len(), types.len());
        for time in times {
            data.extend(time.to_be_bytes());
        }
        data.extend(kinds);
        for offset in types {
            data.extend(offset.to_be_bytes());
            data.extend([0, 0]);
        }
        data.extend(b"ABC\0");
        data.extend(format!("\n{rule}\n").bytes());
        data
    }

    #[test]
    fn reads_the_list_then_the_rule() {
        let changed = utc(2000, 1, 1, 0);
        let data = file(&[0, changed], &[0, 1], &[3600, 7200], "XYZ-3");
        assert_eq!(offset_at(&data, -1), Some(3600));
        assert_eq!(offset_at(&data, 10), Some(3600));
        // Past the list, the rule: a fat file would have listed this too.
        assert_eq!(offset_at(&data, changed + 10), Some(10_800));
        // A rule Mochi can't read leaves the last type.
        let data = file(&[0, changed], &[0, 1], &[3600, 7200], "");
        assert_eq!(offset_at(&data, changed + 10), Some(7200));
        assert_eq!(offset_at(b"not a zone", 0), None);
    }

    #[test]
    fn reads_the_system_zones() {
        let Some(dir) = ["/usr/share/zoneinfo", "/etc/zoneinfo"]
            .into_iter()
            .map(std::path::Path::new)
            .find(|dir| dir.join("Asia/Kolkata").is_file())
        else {
            return;
        };
        let read = |zone: &str, at| offset_at(&std::fs::read(dir.join(zone)).unwrap(), at);
        assert_eq!(read("Asia/Kolkata", utc(2026, 1, 1, 0)), Some(19_800));
        assert_eq!(read("Europe/London", utc(2026, 1, 1, 0)), Some(0));
        assert_eq!(read("Europe/London", utc(2026, 7, 1, 0)), Some(3600));
        assert_eq!(read("America/New_York", utc(2050, 7, 1, 0)), Some(-14_400));
        assert_eq!(read("Australia/Sydney", utc(2026, 1, 1, 0)), Some(39_600));
    }
}
