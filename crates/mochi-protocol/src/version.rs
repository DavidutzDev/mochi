//! Mochi's version, and the oldest one a theme, bento or plugin says it
//! works with.

/// The version running, like `0.0.7`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A version's numbers: `0.1` is `0.1.0`. What follows a `-` or `+`, like
/// `-rc1`, doesn't count.
pub fn parse(text: &str) -> Option<[u64; 3]> {
    let core = text.trim().split(['-', '+']).next()?;
    let mut numbers = [0; 3];
    let mut parts = core.split('.');
    for (index, slot) in numbers.iter_mut().enumerate() {
        match parts.next() {
            Some(part) => *slot = part.parse().ok()?,
            None if index > 0 => break,
            None => return None,
        }
    }
    parts.next().is_none().then_some(numbers)
}

/// Whether this Mochi is `minimum` or newer, or why not.
pub fn supports(minimum: &str) -> Result<(), String> {
    let wanted = parse(minimum)
        .ok_or_else(|| format!("`mochi = {minimum:?}` isn't a version like \"0.0.8\""))?;
    if parse(VERSION).is_some_and(|running| running >= wanted) {
        Ok(())
    } else {
        Err(format!(
            "it needs Mochi {minimum} or newer, this is {VERSION}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_their_numbers() {
        assert_eq!(parse("0.0.7"), Some([0, 0, 7]));
        assert_eq!(parse("1.2"), Some([1, 2, 0]));
        assert_eq!(parse("0.1.0-rc1"), Some([0, 1, 0]));
        assert_eq!(parse("one"), None);
        assert_eq!(parse("1.2.3.4"), None);
        assert_eq!(parse(""), None);
        assert!(supports("0.0.1").is_ok());
        assert!(supports(VERSION).is_ok());
        assert!(supports("999.0").unwrap_err().contains("needs Mochi 999.0"));
        assert!(supports("soon").is_err());
    }
}
