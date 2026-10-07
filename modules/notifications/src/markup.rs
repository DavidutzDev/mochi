//! The body's markup (`body-markup`), made safe for Qt's StyledText.
//!
//! Apps may put a little HTML in the body. The views show it as StyledText,
//! which also understands images, fonts and colors, so only this subset
//! passes:
//!
//! - `<b>`, `<i>`, `<u>`, `<s>` and `<br>`, with `<strong>`, `<em>`, `<del>`
//!   and `<strike>` read as their short forms. A newline becomes `<br>`,
//!   since StyledText folds it into a space.
//! - `<a href>` to an `http`, `https`, `mailto` or `file` URL. Any other link,
//!   like `javascript:`, loses its tag and keeps its text.
//! - `<img>` becomes its `alt` text: StyledText would load the picture from
//!   anywhere, and doesn't show `alt` when it can't.
//! - Entities stay, as long as they are well formed.
//!
//! Every other tag shows as text, so a plain body like `Ada <ada@example.org>`
//! reads as written. Attributes other than `href` go, tags close in the
//! right order, and the ones left open close at the end.
//!
//! With `markdown`, the body's Markdown is read first, see `markdown`.

use crate::markdown;

/// The whole body as StyledText.
pub fn markup(body: &str, markdown: bool) -> String {
    render(&read(body, markdown), false).text
}

/// The body's first line with text in it, as StyledText.
pub fn first_line(body: &str, markdown: bool) -> String {
    render(&read(body, markdown), true).text
}

/// The links the markup keeps, as they appear in its `href`s: what a view
/// hands back when one is clicked.
pub fn links(body: &str, markdown: bool) -> Vec<String> {
    render(&read(body, markdown), false).links
}

fn read(body: &str, markdown: bool) -> std::borrow::Cow<'_, str> {
    if markdown {
        markdown::to_markup(body).into()
    } else {
        body.into()
    }
}

/// The length of a well-formed tag at the start of `text`, whatever its
/// name, for the Markdown pass to leave alone.
pub(crate) fn is_tag(text: &str) -> Option<usize> {
    parse_tag(text).map(|tag| tag.len)
}

/// The scheme of a link worth keeping, when `url` has one and more after it.
pub(crate) fn scheme(url: &str) -> Option<&'static str> {
    let lower = url.trim().to_ascii_lowercase();
    SCHEMES
        .into_iter()
        .find(|scheme| lower.starts_with(scheme) && lower.len() > scheme.len())
}

const SCHEMES: [&str; 4] = ["http://", "https://", "mailto:", "file://"];

/// Entities StyledText reads by name.
const NAMED: [&str; 6] = ["lt", "gt", "amp", "quot", "nbsp", "apos"];

#[derive(Debug, Default)]
struct Rendered {
    text: String,
    links: Vec<String>,
}

/// A tag as written, before deciding what to do with it.
#[derive(Debug)]
struct Tag {
    closing: bool,
    name: String,
    attributes: Vec<(String, String)>,
    /// Bytes from the `<` through the `>`.
    len: usize,
}

/// An element left open: `b`, `i`, `u`, `s` or `a`. A link without a usable
/// `href` is open but wasn't written.
#[derive(Debug)]
struct Open {
    name: &'static str,
    written: bool,
}

fn render(body: &str, one_line: bool) -> Rendered {
    let mut out = Rendered::default();
    let mut open: Vec<Open> = Vec::new();
    // Whether anything but whitespace was written, so a first line skips
    // blank ones.
    let mut seen = false;
    let mut rest = body;

    while let Some(ch) = rest.chars().next() {
        let mut advance = ch.len_utf8();
        let mut line_break = false;
        match ch {
            '<' => match parse_tag(rest) {
                Some(tag) => {
                    advance = tag.len;
                    match canonical(&tag.name) {
                        "br" if !tag.closing => line_break = true,
                        "img" if !tag.closing => {
                            if let Some(alt) = attribute(&tag, "alt") {
                                seen |= !alt.trim().is_empty();
                                escape_into(&mut out.text, &alt);
                            }
                        }
                        name @ ("b" | "i" | "u" | "s" | "a") if tag.closing => {
                            close(&mut out.text, &mut open, name)
                        }
                        name @ ("b" | "i" | "u" | "s") => {
                            out.text.push_str(&format!("<{name}>"));
                            open.push(Open {
                                name,
                                written: true,
                            });
                        }
                        "a" => {
                            // Links don't nest: a new one ends the last.
                            close(&mut out.text, &mut open, "a");
                            let href = attribute(&tag, "href").and_then(|href| link(&href));
                            if let Some(href) = &href {
                                out.text.push_str(&format!("<a href=\"{href}\">"));
                                out.links.push(href.clone());
                            }
                            open.push(Open {
                                name: "a",
                                written: href.is_some(),
                            });
                        }
                        // A closing `</br>` or `</img>` means nothing.
                        "br" | "img" => {}
                        _ => {
                            out.text.push_str("&lt;");
                            advance = 1;
                        }
                    }
                }
                None => out.text.push_str("&lt;"),
            },
            '>' => out.text.push_str("&gt;"),
            '&' => match entity(rest) {
                Some((kept, len)) => {
                    seen = true;
                    out.text.push_str(&kept);
                    advance = len;
                }
                None => out.text.push_str("&amp;"),
            },
            '\n' => line_break = true,
            '\r' => {}
            other => {
                seen |= !other.is_whitespace();
                out.text.push(other);
            }
        }
        rest = &rest[advance..];

        if line_break {
            if one_line && seen {
                break;
            }
            if !one_line {
                out.text.push_str("<br>");
            }
        }
    }

    for element in open.into_iter().rev() {
        if element.written {
            out.text.push_str(&format!("</{}>", element.name));
        }
    }
    if one_line {
        out.text = out.text.trim().to_owned();
    }
    out
}

/// A known tag's name, with long forms like `strong` read as the short
/// one; empty for the others.
fn canonical(name: &str) -> &'static str {
    match name {
        "b" | "strong" => "b",
        "i" | "em" => "i",
        "u" => "u",
        "s" | "del" | "strike" => "s",
        "a" => "a",
        "br" => "br",
        "img" => "img",
        _ => "",
    }
}

/// Closes `name` and everything opened inside it. A closing tag with
/// nothing to close goes.
fn close(out: &mut String, open: &mut Vec<Open>, name: &str) {
    let Some(index) = open.iter().rposition(|element| element.name == name) else {
        return;
    };
    for element in open.drain(index..).rev() {
        if element.written {
            out.push_str(&format!("</{}>", element.name));
        }
    }
}

/// `<name attr="value" ...>` or `</name>` at the start of `text`. None when
/// it isn't a well-formed tag, so the `<` shows as text.
fn parse_tag(text: &str) -> Option<Tag> {
    let mut chars = text.char_indices().skip(1).peekable();
    let closing = chars.next_if(|&(_, ch)| ch == '/').is_some();
    let mut name = String::new();
    while let Some((_, ch)) = chars.next_if(|&(_, ch)| ch.is_ascii_alphanumeric()) {
        name.push(ch.to_ascii_lowercase());
    }
    if name.is_empty() {
        return None;
    }
    // The name ends at a space, a `/` or the `>`.
    if !chars
        .peek()
        .is_some_and(|&(_, ch)| ch.is_whitespace() || ch == '/' || ch == '>')
    {
        return None;
    }

    let mut attributes = Vec::new();
    loop {
        while chars
            .next_if(|&(_, ch)| ch.is_whitespace() || ch == '/')
            .is_some()
        {}
        let (index, ch) = chars.next()?;
        if ch == '>' {
            return Some(Tag {
                closing,
                name,
                attributes,
                len: index + 1,
            });
        }
        let mut key = String::from(ch.to_ascii_lowercase());
        while let Some((_, ch)) = chars
            .next_if(|&(_, ch)| !ch.is_whitespace() && !matches!(ch, '=' | '>' | '/' | '"' | '\''))
        {
            key.push(ch.to_ascii_lowercase());
        }
        while chars.next_if(|&(_, ch)| ch.is_whitespace()).is_some() {}
        let mut value = String::new();
        if chars.next_if(|&(_, ch)| ch == '=').is_some() {
            while chars.next_if(|&(_, ch)| ch.is_whitespace()).is_some() {}
            match chars.next_if(|&(_, ch)| ch == '"' || ch == '\'') {
                Some((_, quote)) => loop {
                    let (_, ch) = chars.next()?;
                    if ch == quote {
                        break;
                    }
                    value.push(ch);
                },
                None => {
                    while let Some((_, ch)) =
                        chars.next_if(|&(_, ch)| !ch.is_whitespace() && ch != '>')
                    {
                        value.push(ch);
                    }
                }
            }
        }
        attributes.push((key, value));
    }
}

/// An attribute's value with its entities read.
fn attribute(tag: &Tag, name: &str) -> Option<String> {
    tag.attributes
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| decode(value))
}

/// The `href` to write, when the link is one to keep. StyledText hands an
/// `href` back as written, without reading its entities, so the characters
/// that would end the attribute are percent-encoded instead.
fn link(href: &str) -> Option<String> {
    let href = href.trim();
    scheme(href)?;
    let mut written = String::with_capacity(href.len());
    for ch in href.chars() {
        match ch {
            '"' => written.push_str("%22"),
            '<' => written.push_str("%3C"),
            '>' => written.push_str("%3E"),
            ' ' => written.push_str("%20"),
            ch if ch.is_control() => {}
            ch => written.push(ch),
        }
    }
    Some(written)
}

/// A well-formed entity at the start of `text`, as StyledText should get
/// it, and its length.
fn entity(text: &str) -> Option<(String, usize)> {
    // The longest is `&#x10FFFF;`.
    let (end, _) = text.char_indices().take(12).find(|&(_, ch)| ch == ';')?;
    let name = &text[1..end];
    let kept = if NAMED.contains(&name) {
        format!("&{name};")
    } else {
        let number = name.strip_prefix('#')?;
        let code = match number.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => number.parse().ok()?,
        };
        let ch = char::from_u32(code).filter(|ch| !ch.is_control() || *ch == '\t')?;
        format!("&#{};", u32::from(ch))
    };
    Some((kept, end + 1))
}

/// Reads the entities in an attribute's value.
fn decode(value: &str) -> String {
    let mut decoded = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(at) = rest.find('&') {
        decoded.push_str(&rest[..at]);
        rest = &rest[at..];
        let read = entity(rest).and_then(|(kept, len)| {
            let ch = match &kept[1..kept.len() - 1] {
                "lt" => '<',
                "gt" => '>',
                "amp" => '&',
                "quot" => '"',
                "apos" => '\'',
                "nbsp" => '\u{a0}',
                number => char::from_u32(number.strip_prefix('#')?.parse().ok()?)?,
            };
            Some((ch, len))
        });
        match read {
            Some((ch, len)) => {
                decoded.push(ch);
                rest = &rest[len..];
            }
            None => {
                decoded.push('&');
                rest = &rest[1..];
            }
        }
    }
    decoded.push_str(rest);
    decoded
}

/// Plain text, like an image's `alt`, made safe to put in the markup.
fn escape_into(out: &mut String, text: &str) {
    for ch in text.chars() {
        match ch {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '\n' | '\r' => out.push(' '),
            ch => out.push(ch),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_safe_tags() {
        assert_eq!(
            markup("<b>Ada</b> sent <i>a</i> <u>file</u>", false),
            "<b>Ada</b> sent <i>a</i> <u>file</u>"
        );
        assert_eq!(markup("<B>loud</B>", false), "<b>loud</b>");
        assert_eq!(
            markup("one\ntwo<br/>three<br >", false),
            "one<br>two<br>three<br>"
        );
    }

    #[test]
    fn nests_and_closes_in_order() {
        assert_eq!(
            markup("<b>bo<i>ld</b> it</i>", false),
            "<b>bo<i>ld</i></b> it"
        );
        assert_eq!(markup("<b><i>deep</i></b>", false), "<b><i>deep</i></b>");
        assert_eq!(
            markup("<b>never closed <u>either", false),
            "<b>never closed <u>either</u></b>"
        );
        assert_eq!(markup("stray</b> close", false), "stray close");
    }

    #[test]
    fn escapes_everything_else() {
        assert_eq!(
            markup("Ada <ada@example.org>", false),
            "Ada &lt;ada@example.org&gt;"
        );
        assert_eq!(
            markup("<span style=\"color:red\">hi</span>", false),
            "&lt;span style=\"color:red\"&gt;hi&lt;/span&gt;"
        );
        assert_eq!(
            markup("<font color=red>x", false),
            "&lt;font color=red&gt;x"
        );
        assert_eq!(markup("1 < 2 > 0", false), "1 &lt; 2 &gt; 0");
        assert_eq!(markup("<b unclosed", false), "&lt;b unclosed");
        assert_eq!(
            markup("<b title=\"no end>x", false),
            "&lt;b title=\"no end&gt;x"
        );
    }

    #[test]
    fn drops_attributes_but_href() {
        assert_eq!(
            markup("<b class=\"x\" onclick='evil()'>hi</b>", false),
            "<b>hi</b>"
        );
        assert_eq!(
            markup(
                "<a target=_blank href='https://example.org' style=x>site</a>",
                false
            ),
            "<a href=\"https://example.org\">site</a>"
        );
    }

    #[test]
    fn keeps_only_safe_links() {
        for href in [
            "javascript:alert(1)",
            " JavaScript:alert(1)",
            "data:text/html,hi",
            "ftp://example.org",
            "vbscript:x",
            "/etc/passwd",
            "https://",
            "",
        ] {
            assert_eq!(
                markup(&format!("<a href=\"{href}\">x</a>"), false),
                "x",
                "{href:?}"
            );
        }
        assert_eq!(markup("<a>no href</a>", false), "no href");
        for href in [
            "https://example.org",
            "HTTP://example.org",
            "mailto:ada@example.org",
            "file:///home/ada/a.pdf",
        ] {
            assert_eq!(
                links(&format!("<a href=\"{href}\">x</a>"), false),
                [href],
                "{href:?}"
            );
        }
        // A dropped link's closing tag doesn't close an outer element.
        assert_eq!(
            markup("<b><a href=\"javascript:x\">x</a> y</b>", false),
            "<b>x y</b>"
        );
        // Links don't nest.
        assert_eq!(
            markup(
                "<a href=\"https://a.org\">a<a href=\"https://b.org\">b</a>",
                false
            ),
            "<a href=\"https://a.org\">a</a><a href=\"https://b.org\">b</a>"
        );
    }

    #[test]
    fn hrefs_come_back_as_written() {
        let body = "<a href=\"https://example.org/?a=1&amp;b=&quot;2&quot; c\">x</a>";
        assert_eq!(
            markup(body, false),
            "<a href=\"https://example.org/?a=1&b=%222%22%20c\">x</a>"
        );
        assert_eq!(
            links(body, false),
            ["https://example.org/?a=1&b=%222%22%20c"]
        );
    }

    #[test]
    fn keeps_entities() {
        assert_eq!(
            markup(
                "&lt;b&gt; &amp; &quot; &apos; &nbsp; &#9731; &#x263A;",
                false
            ),
            "&lt;b&gt; &amp; &quot; &apos; &nbsp; &#9731; &#9786;"
        );
        assert_eq!(markup("fish & chips", false), "fish &amp; chips");
        assert_eq!(
            markup("&bogus; &#xZZ; &#0; &", false),
            "&amp;bogus; &amp;#xZZ; &amp;#0; &amp;"
        );
    }

    #[test]
    fn images_become_their_alt_text() {
        assert_eq!(
            markup(
                "look <img src=\"https://tracker.example/p.png\" alt=\"a <cat>\"/> here",
                false
            ),
            "look a &lt;cat&gt; here"
        );
        assert_eq!(markup("<img src=x.png>", false), "");
    }

    #[test]
    fn reads_long_forms_and_strikes() {
        assert_eq!(
            markup(
                "<strong>a</strong> <em>b</em> <del>c</del> <strike>d</s>",
                false
            ),
            "<b>a</b> <i>b</i> <s>c</s> <s>d</s>"
        );
    }

    #[test]
    fn reads_markdown_when_asked() {
        assert_eq!(markup("**TEST**", false), "**TEST**");
        assert_eq!(markup("**TEST**", true), "<b>TEST</b>");
        // What it makes is checked like any markup: a bad link stays text,
        // and a tag inside code shows as written.
        assert_eq!(
            markup("[x](javascript:alert(1)) `<img src=x>`", true),
            "[x](javascript:alert(1)) &lt;img src=x&gt;"
        );
        let body = "see https://example.org/a_b?x=1&y=2, now";
        assert_eq!(links(body, true), ["https://example.org/a_b?x=1&y=2"]);
        assert!(links(body, false).is_empty());
        assert_eq!(first_line("**a\nb** c", true), "<b>a</b>");
    }

    #[test]
    fn the_first_line_skips_blank_ones_and_closes_its_tags() {
        assert_eq!(
            first_line("\n  \n<b>Ada:</b> hi\nsecond", false),
            "<b>Ada:</b> hi"
        );
        assert_eq!(first_line("<i>one<br>two</i>", false), "<i>one</i>");
        assert_eq!(first_line("plain", false), "plain");
        assert_eq!(first_line("", false), "");
    }
}
