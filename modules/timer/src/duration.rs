//! Lengths as people type them, for custom timers: `5m`, `90s`, `1h 30m`,
//! `1h30`, `1.5h`, `10:00`, `1:30:00`, or a bare number of minutes, with a
//! label after it: `15m Tea`. The label is whatever follows the length.

/// The longest timer, a day.
pub const LONGEST_MS: u64 = 24 * 3600 * 1000;
/// The longest label, in characters.
const LABEL: usize = 60;

const HOUR: f64 = 3_600_000.0;
const MINUTE: f64 = 60_000.0;
const SECOND: f64 = 1000.0;

/// A length and its label, maybe empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    pub ms: u64,
    pub label: String,
}

/// Reads a length and an optional label, or says what's wrong.
pub fn parse(input: &str) -> Result<Parsed, String> {
    let text = input.trim();
    let first = text.split_whitespace().next().unwrap_or_default();
    let (ms, rest) = if first.contains(':') {
        (clock(first)?, &text[first.len()..])
    } else {
        units(text)?
    };
    if ms == 0 {
        return Err("a timer needs a length above zero".to_owned());
    }
    if ms > LONGEST_MS {
        return Err("a timer runs a day at most".to_owned());
    }
    let label: String = rest.trim().chars().take(LABEL).collect();
    Ok(Parsed {
        ms,
        label: label.trim_end().to_owned(),
    })
}

/// `m:ss` or `h:mm:ss`.
fn clock(text: &str) -> Result<u64, String> {
    let wrong = || format!("{text} isn't a length like 10:00 or 1:30:00");
    let parts: Vec<u64> = text
        .split(':')
        .map(|part| part.parse().map_err(|_| wrong()))
        .collect::<Result<_, _>>()?;
    let seconds = match parts[..] {
        [minutes, seconds] if seconds < 60 => minutes * 60 + seconds,
        [hours, minutes, seconds] if minutes < 60 && seconds < 60 => {
            hours * 3600 + minutes * 60 + seconds
        }
        _ => return Err(wrong()),
    };
    Ok(seconds.saturating_mul(1000))
}

/// Numbers with units from the start of `text`, and the rest after them.
/// A number without a unit takes the unit below the one before it, so
/// `1h 30` is an hour and a half, or minutes when it comes first; it ends
/// the length.
fn units(text: &str) -> Result<(u64, &str), String> {
    let mut rest = text;
    let mut total = 0.0;
    let mut last: Option<f64> = None;
    loop {
        let trimmed = rest.trim_start();
        let digits = trimmed
            .find(|char: char| !char.is_ascii_digit() && char != '.')
            .unwrap_or(trimmed.len());
        let Ok(number) = trimmed[..digits].parse::<f64>() else {
            break;
        };
        let after = &trimmed[digits..];
        let spaced = after.trim_start();
        let letters = spaced
            .find(|char: char| !char.is_alphabetic())
            .unwrap_or(spaced.len());
        match unit(&spaced[..letters]) {
            Some(unit) => {
                total += number * unit;
                last = Some(unit);
                rest = &spaced[letters..];
            }
            None => {
                let unit = match last {
                    Some(unit) if unit == HOUR => MINUTE,
                    Some(_) => SECOND,
                    None => MINUTE,
                };
                total += number * unit;
                last = Some(unit);
                rest = after;
                break;
            }
        }
    }
    if last.is_none() {
        let what = text.split_whitespace().next().unwrap_or(text);
        return Err(format!(
            "{what:?} isn't a length; try 5m, 90s, 1h 30m or 10:00"
        ));
    }
    Ok((total.round() as u64, rest))
}

fn unit(word: &str) -> Option<f64> {
    Some(match word.to_lowercase().as_str() {
        "h" | "hr" | "hrs" | "hour" | "hours" => HOUR,
        "m" | "min" | "mins" | "minute" | "minutes" => MINUTE,
        "s" | "sec" | "secs" | "second" | "seconds" => SECOND,
        _ => return None,
    })
}

/// A length in words: `10 min`, `1 h 30 min`, `45 s`, `2 min 30 s`.
pub fn words(ms: u64) -> String {
    let seconds = ms.div_ceil(1000);
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    let mut parts = Vec::new();
    if hours > 0 {
        parts.push(format!("{hours} h"));
    }
    if minutes > 0 {
        parts.push(format!("{minutes} min"));
    }
    if seconds > 0 || parts.is_empty() {
        parts.push(format!("{seconds} s"));
    }
    parts.join(" ")
}

/// A length as `parse` reads it back: `10m`, `1h30m`, `90s`.
pub fn short(ms: u64) -> String {
    let seconds = ms.div_ceil(1000);
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    let mut text = String::new();
    if hours > 0 {
        text.push_str(&format!("{hours}h"));
    }
    if minutes > 0 {
        text.push_str(&format!("{minutes}m"));
    }
    if seconds > 0 || text.is_empty() {
        text.push_str(&format!("{seconds}s"));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: u64 = 60_000;

    fn read(input: &str) -> (u64, String) {
        let parsed = parse(input).unwrap_or_else(|error| panic!("{input}: {error}"));
        (parsed.ms, parsed.label)
    }

    #[test]
    fn reads_lengths_with_units() {
        assert_eq!(read("5m"), (5 * MINUTE, String::new()));
        assert_eq!(read("90s"), (90_000, String::new()));
        assert_eq!(read("1h 30m"), (90 * MINUTE, String::new()));
        assert_eq!(read("1h30m"), (90 * MINUTE, String::new()));
        assert_eq!(read("1.5h"), (90 * MINUTE, String::new()));
        assert_eq!(read("2 min 30 sec"), (150_000, String::new()));
        assert_eq!(read("1 hour"), (60 * MINUTE, String::new()));
        assert_eq!(read("10 Minutes"), (10 * MINUTE, String::new()));
    }

    #[test]
    fn a_bare_number_takes_the_next_unit_down() {
        assert_eq!(read("25"), (25 * MINUTE, String::new()));
        assert_eq!(read("1h 30"), (90 * MINUTE, String::new()));
        assert_eq!(read("1h30"), (90 * MINUTE, String::new()));
        assert_eq!(read("5m 30"), (330_000, String::new()));
    }

    #[test]
    fn reads_clock_lengths() {
        assert_eq!(read("10:00"), (10 * MINUTE, String::new()));
        assert_eq!(read("0:45"), (45_000, String::new()));
        assert_eq!(read("1:30:00 Bread"), (90 * MINUTE, "Bread".into()));
        assert!(parse("10:75").is_err());
        assert!(parse("1:2:3:4").is_err());
        assert!(parse("ten:00").is_err());
    }

    #[test]
    fn the_rest_is_the_label() {
        assert_eq!(read("15m Tea"), (15 * MINUTE, "Tea".into()));
        assert_eq!(
            read("10m  Pizza  oven "),
            (10 * MINUTE, "Pizza  oven".into())
        );
        assert_eq!(read("3 eggs"), (3 * MINUTE, "eggs".into()));
        assert_eq!(read("1h 30 Bread"), (90 * MINUTE, "Bread".into()));
        // A word that isn't a unit starts the label, even one that begins
        // like one.
        assert_eq!(read("5 mangoes"), (5 * MINUTE, "mangoes".into()));
        assert_eq!(read("20s squats"), (20_000, "squats".into()));
        let long = format!("1m {}", "x".repeat(100));
        assert_eq!(read(&long).1.len(), 60);
    }

    #[test]
    fn says_what_is_wrong() {
        assert!(parse("").is_err());
        assert!(parse("Tea").unwrap_err().contains("\"Tea\" isn't a length"));
        assert!(parse("0m").unwrap_err().contains("above zero"));
        assert!(parse("25h").unwrap_err().contains("a day at most"));
        assert_eq!(read("24h").0, LONGEST_MS);
    }

    #[test]
    fn writes_lengths_back() {
        assert_eq!(words(10 * MINUTE), "10 min");
        assert_eq!(words(90 * MINUTE), "1 h 30 min");
        assert_eq!(words(150_000), "2 min 30 s");
        assert_eq!(words(45_000), "45 s");
        assert_eq!(words(0), "0 s");
        for ms in [10 * MINUTE, 90 * MINUTE, 150_000, 45_000, 3_600_000 + 1000] {
            assert_eq!(parse(&short(ms)).unwrap().ms, ms, "{}", short(ms));
        }
    }
}
