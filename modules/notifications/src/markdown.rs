//! Markdown in a body, as the tags `markup` keeps.
//!
//! Some apps, like Discord, send their messages' Markdown as it was typed,
//! so `**hi**` would show with its stars. This reads the inline part of
//! Discord's flavor before the markup is made safe:
//!
//! - `**bold**`, `*italic*` and `_italic_`, `***both***`, `__underline__`,
//!   `~~strike~~`.
//! - `` `code` `` shows its text as written, without the backticks.
//! - `||spoilers||` show as "spoiler", so a notification doesn't give them
//!   away.
//! - `[text](https://…)` becomes a link, and so does a bare `https://…` or
//!   `<https://…>`, for every app.
//! - A backslash shows the character after it as written.
//!
//! Tags and entities already in the body pass through untouched, for apps
//! that send markup. `_` only counts at a word's edge, so `snake_case`
//! stays, and a mark needs text right inside it, so `2 * 3 * 4` stays too.

use crate::markup::{is_tag, scheme};

/// The marks, longest first so `**` isn't read as two `*`.
const MARKS: [&str; 7] = ["***", "**", "__", "~~", "||", "*", "_"];

/// Characters a backslash keeps as written.
const ESCAPABLE: &str = "\\*_~|`[]()<>#-";

/// The body with its Markdown as tags.
pub fn to_markup(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    inline(text, &mut out);
    out
}

fn inline(text: &str, out: &mut String) {
    let mut at = 0;
    'scan: while let Some(ch) = text[at..].chars().next() {
        let rest = &text[at..];

        if ch == '\\'
            && let Some(next) = rest[1..].chars().next()
            && ESCAPABLE.contains(next)
        {
            escape_into(out, &next.to_string());
            at += 1 + next.len_utf8();
            continue;
        }

        if ch == '<' {
            // `<https://…>`: a link Discord shows without a preview.
            if let Some(end) = rest.find('>')
                && scheme(&rest[1..end]).is_some()
                && !rest[1..end].contains(char::is_whitespace)
            {
                link_into(out, &rest[1..end], None);
                at += end + 1;
                continue;
            }
            // A tag the app wrote stays as it is.
            if let Some(len) = is_tag(rest) {
                out.push_str(&rest[..len]);
                at += len;
                continue;
            }
        }

        if ch == '&'
            && let Some(end) = rest[..rest.len().min(12)].find(';')
            && rest[1..end]
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '#')
        {
            out.push_str(&rest[..=end]);
            at += end + 1;
            continue;
        }

        if ch == '`'
            && let Some((code, len)) = code_span(rest)
        {
            escape_into(out, code);
            at += len;
            continue;
        }

        if ch == '['
            && let Some((label, url, len)) = masked_link(rest)
        {
            link_into(out, url, Some(label));
            at += len;
            continue;
        }

        if matches!(ch, 'h' | 'H')
            && !text[..at]
                .chars()
                .next_back()
                .is_some_and(char::is_alphanumeric)
            && let Some(len) = bare_url(rest)
        {
            link_into(out, &rest[..len], None);
            at += len;
            continue;
        }

        for mark in MARKS {
            if rest.starts_with(mark)
                && opens(text, at, mark)
                && let Some(end) = closing(text, at + mark.len(), mark)
            {
                let inside = &text[at + mark.len()..end];
                if mark == "||" {
                    out.push_str("<i>spoiler</i>");
                } else {
                    let tags = tags(mark);
                    for tag in tags {
                        out.push_str(&format!("<{tag}>"));
                    }
                    inline(inside, out);
                    for tag in tags.iter().rev() {
                        out.push_str(&format!("</{tag}>"));
                    }
                }
                at = end + mark.len();
                continue 'scan;
            }
        }

        out.push(ch);
        at += ch.len_utf8();
    }
}

fn tags(mark: &str) -> &'static [&'static str] {
    match mark {
        "***" => &["b", "i"],
        "**" => &["b"],
        "__" => &["u"],
        "~~" => &["s"],
        _ => &["i"],
    }
}

/// Whether `mark` at `at` can open: text right after it, and for `_`, not
/// inside a word.
fn opens(text: &str, at: usize, mark: &str) -> bool {
    let after = text[at + mark.len()..].chars().next();
    if after.is_none_or(char::is_whitespace) {
        return false;
    }
    !mark.starts_with('_')
        || !text[..at]
            .chars()
            .next_back()
            .is_some_and(char::is_alphanumeric)
}

/// Where `mark` closes, from `from` on: text right before it, and for `_`,
/// not inside a word. A run that opens a span of its own inside, like the
/// `**` in `*a **b** c*`, is skipped to that span's end. A longer run that
/// only closes, like the `***` ending `**bold *and italic***`, closes the
/// inner span with its first marks and leaves the rest to the outer one.
fn closing(text: &str, from: usize, mark: &str) -> Option<usize> {
    let first = mark.chars().next()?;
    let mut at = from;
    while at < text.len() {
        let rest = &text[at..];
        if rest.starts_with('`')
            && let Some((_, len)) = code_span(rest)
        {
            at += len;
            continue;
        }
        let run = rest.chars().take_while(|&ch| ch == first).count();
        if run == 0 {
            at += rest.chars().next()?.len_utf8();
            continue;
        }
        let before = text[..at].chars().next_back();
        let after = rest[run..].chars().next();
        let can_open = after.is_some_and(|ch| !ch.is_whitespace())
            && !before.is_some_and(char::is_alphanumeric);
        let can_close = at > from
            && before.is_some_and(|ch| !ch.is_whitespace())
            && (first != '_' || !after.is_some_and(char::is_alphanumeric));
        if can_close && (run == mark.len() || (run > mark.len() && !can_open)) {
            return Some(at);
        }
        if can_open
            && let Some(inner) = MARKS
                .into_iter()
                .find(|inner| inner.starts_with(first) && inner.len() <= run)
            && let Some(end) = closing(text, at + inner.len(), inner)
        {
            at = end + inner.len();
            continue;
        }
        at += run;
    }
    None
}

/// A span between runs of as many backticks, and its whole length.
fn code_span(text: &str) -> Option<(&str, usize)> {
    let ticks = text.chars().take_while(|&ch| ch == '`').count();
    let fence = &text[..ticks];
    let end = text[ticks..].find(fence)? + ticks;
    let code = &text[ticks..end];
    // A longer run isn't the end of this span.
    if text[end + ticks..].starts_with('`') || code.is_empty() {
        return None;
    }
    Some((code.trim_matches('\n'), end + ticks))
}

/// `[label](url)`, and its whole length.
fn masked_link(text: &str) -> Option<(&str, &str, usize)> {
    let close = text.find("](")?;
    let label = &text[1..close];
    if label.is_empty() || label.contains(['\n', '[']) {
        return None;
    }
    let url_start = close + 2;
    let end = text[url_start..].find(')')? + url_start;
    let url = text[url_start..end].trim();
    if url.contains(char::is_whitespace) || scheme(url).is_none() {
        return None;
    }
    Some((label, url, end + 1))
}

/// A bare `http://` or `https://` link's length. Punctuation at its end,
/// and a closing bracket it didn't open, belong to the sentence.
fn bare_url(text: &str) -> Option<usize> {
    let lower = text.get(..8)?.to_ascii_lowercase();
    if !lower.starts_with("http://") && !lower.starts_with("https://") {
        return None;
    }
    let mut end = text
        .find(|ch: char| ch.is_whitespace() || matches!(ch, '<' | '>' | '"'))
        .unwrap_or(text.len());
    loop {
        let url = &text[..end];
        let last = url.chars().next_back()?;
        let unopened = last == ')' && url.matches('(').count() < url.matches(')').count();
        if matches!(
            last,
            '.' | ',' | ';' | ':' | '!' | '?' | '\'' | '*' | '_' | '~' | '|'
        ) || unopened
        {
            end -= last.len_utf8();
        } else {
            break;
        }
    }
    let scheme_len = if lower.starts_with("https") { 8 } else { 7 };
    (end > scheme_len).then_some(end)
}

/// A link as a tag `markup` reads: the label's Markdown is read too.
fn link_into(out: &mut String, url: &str, label: Option<&str>) {
    out.push_str("<a href=\"");
    escape_into(out, url);
    out.push_str("\">");
    match label {
        Some(label) => inline(label, out),
        None => escape_into(out, url),
    }
    out.push_str("</a>");
}

/// Text that must show as written.
fn escape_into(out: &mut String, text: &str) {
    for ch in text.chars() {
        match ch {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            ch => out.push(ch),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_discord_marks() {
        assert_eq!(to_markup("**TEST**"), "<b>TEST</b>");
        assert_eq!(to_markup("*a* and _b_"), "<i>a</i> and <i>b</i>");
        assert_eq!(to_markup("***both***"), "<b><i>both</i></b>");
        assert_eq!(to_markup("__under__ ~~gone~~"), "<u>under</u> <s>gone</s>");
        assert_eq!(
            to_markup("**bold *and italic***"),
            "<b>bold <i>and italic</i></b>"
        );
        assert_eq!(to_markup("*a **b** c*"), "<i>a <b>b</b> c</i>");
        assert_eq!(to_markup("**over\ntwo lines**"), "<b>over\ntwo lines</b>");
    }

    #[test]
    fn leaves_what_isnt_markdown() {
        for text in [
            "snake_case_name",
            "2 * 3 * 4",
            "** spaced **",
            "**never closed",
            "a lone _",
            "5*",
            "~~",
        ] {
            assert_eq!(to_markup(text), text, "{text:?}");
        }
        assert_eq!(to_markup(r"\*not italic\*"), "*not italic*");
        assert_eq!(to_markup(r"a \<b>"), "a &lt;b>");
    }

    #[test]
    fn code_shows_as_written() {
        assert_eq!(to_markup("run `cargo **test**`"), "run cargo **test**");
        assert_eq!(to_markup("``a ` b``"), "a ` b");
        assert_eq!(to_markup("`<b>`"), "&lt;b&gt;");
        assert_eq!(to_markup("**a `*` b**"), "<b>a * b</b>");
    }

    #[test]
    fn hides_spoilers() {
        assert_eq!(to_markup("it was ||the butler||"), "it was <i>spoiler</i>");
    }

    #[test]
    fn makes_links() {
        assert_eq!(
            to_markup("see https://example.org/a_b_c."),
            "see <a href=\"https://example.org/a_b_c\">https://example.org/a_b_c</a>."
        );
        assert_eq!(
            to_markup("(https://en.wikipedia.org/wiki/Rust_(language))"),
            "(<a href=\"https://en.wikipedia.org/wiki/Rust_(language)\">https://en.wikipedia.org/wiki/Rust_(language)</a>)"
        );
        assert_eq!(
            to_markup("<https://example.org>"),
            "<a href=\"https://example.org\">https://example.org</a>"
        );
        assert_eq!(
            to_markup("[the **docs**](https://example.org/?a=1&b=2)"),
            "<a href=\"https://example.org/?a=1&amp;b=2\">the <b>docs</b></a>"
        );
        assert_eq!(
            to_markup("**https://example.org**"),
            "<b><a href=\"https://example.org\">https://example.org</a></b>"
        );
        // Not a scheme it opens, or not a link at all.
        assert_eq!(
            to_markup("[x](javascript:alert(1))"),
            "[x](javascript:alert(1))"
        );
        assert_eq!(to_markup("xhttps://example.org"), "xhttps://example.org");
        assert_eq!(to_markup("https://"), "https://");
    }

    #[test]
    fn markup_passes_through() {
        assert_eq!(
            to_markup("<b>Ada</b> &amp; <a href=\"https://a.org\">a_b_c</a>"),
            "<b>Ada</b> &amp; <a href=\"https://a.org\">a_b_c</a>"
        );
    }
}
