//! Views use `Theme`'s scales, not raw sizes and colors, so they don't drift
//! apart. This reads every view in `modules/`, `examples/` and the core QML,
//! and counts
//! the lines that set, by hand:
//!
//! - a text size (`pixelSize: 13`);
//! - spacing or a radius above 2 pixels (`spacing: 10`, `radius: 12`);
//! - a margin above 0 (`anchors.margins: 6`); negative ones, which widen a
//!   click target, are fine;
//! - a color (`"#ff0000"`, `Qt.rgba(...)`);
//! - an animation's duration (`duration: 300`), which then ignores the
//!   theme's speed and reduced motion: `Theme.duration(300)` follows them.
//!
//! A line that needs one says why in a `// design:` comment on it. The views
//! that broke these rules before the scales existed are listed with their
//! counts in `design-baseline.txt`: none may get worse, and one that gets
//! better must lower its count there, so the list only shrinks.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const BASELINE: &str = "tests/design-baseline.txt";

/// The files whose job is defining or drawing the scales themselves.
const EXEMPT: [&str; 2] = [
    "crates/mochi-core/qml/island/Theme.qml",
    "crates/mochi-core/qml/island/Symbol.qml",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn views(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            views(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "qml") {
            found.push(path);
        }
    }
}

/// The value after `key:` on the line, when it's a plain number.
fn number_after(line: &str, key: &str) -> Option<f64> {
    let at = line.find(&format!("{key}:"))?;
    let before = line[..at].chars().next_back();
    if before.is_some_and(|ch| ch.is_ascii_alphanumeric()) {
        return None;
    }
    let value = line[at + key.len() + 1..].trim_start();
    let end = value
        .find(|ch: char| !(ch.is_ascii_digit() || ch == '.' || ch == '-'))
        .unwrap_or(value.len());
    let rest = value[end..].trim_start();
    // Only a literal: `spacing: 4 + Theme.spaceSmall` isn't one.
    if !(rest.is_empty() || rest.starts_with("//") || rest.starts_with(';')) {
        return None;
    }
    value[..end].parse().ok()
}

/// Whether a line sets a raw size or color.
fn breaks_scale(line: &str) -> bool {
    let code = line.split("//").next().unwrap_or_default();
    if line.contains("// design:") {
        return false;
    }
    if number_after(code, "pixelSize").is_some() {
        return true;
    }
    for key in ["spacing", "radius"] {
        if number_after(code, key).is_some_and(|value| value > 2.0) {
            return true;
        }
    }
    for key in [
        "margins",
        "leftMargin",
        "rightMargin",
        "topMargin",
        "bottomMargin",
        "horizontalCenterOffset",
    ] {
        if number_after(code, key).is_some_and(|value| value > 0.0) {
            return true;
        }
    }
    if number_after(code, "duration").is_some() {
        return true;
    }
    let hex = code.match_indices("\"#").any(|(at, _)| {
        let digits: String = code[at + 2..]
            .chars()
            .take_while(char::is_ascii_hexdigit)
            .collect();
        matches!(digits.len(), 3 | 6 | 8) && code[at + 2 + digits.len()..].starts_with('"')
    });
    hex || code.contains("Qt.rgba(")
}

fn counts() -> BTreeMap<String, usize> {
    let root = root();
    let mut found = Vec::new();
    views(&root.join("modules"), &mut found);
    views(&root.join("examples"), &mut found);
    views(&root.join("crates/mochi-core/qml"), &mut found);
    let mut counts = BTreeMap::new();
    for path in found {
        let name = path
            .strip_prefix(&root)
            .expect("found under the root")
            .to_string_lossy()
            .into_owned();
        if EXEMPT.contains(&name.as_str()) {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("views are readable");
        let count = text.lines().filter(|line| breaks_scale(line)).count();
        if count > 0 {
            counts.insert(name, count);
        }
    }
    counts
}

fn baseline() -> BTreeMap<String, usize> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(BASELINE);
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .map(|line| {
            let (count, name) = line
                .trim()
                .split_once(' ')
                .expect("baseline lines are `<count> <path>`");
            (name.to_owned(), count.parse().expect("a count"))
        })
        .collect()
}

#[test]
fn views_use_the_theme_scales() {
    let counts = counts();
    // `MOCHI_WRITE_BASELINE=1 cargo test -p mochi-core --test design`
    // writes the current counts, after a migration lowered them.
    if std::env::var_os("MOCHI_WRITE_BASELINE").is_some() {
        let mut text = String::from(
            "# Views that set raw sizes or colors, from before Theme's scales: `<count> <path>`.\n# Counts may only go down; see tests/design.rs.\n",
        );
        for (name, count) in &counts {
            text.push_str(&format!("{count} {name}\n"));
        }
        std::fs::write(Path::new(env!("CARGO_MANIFEST_DIR")).join(BASELINE), text)
            .expect("the baseline is writable");
    }
    let baseline = baseline();
    let mut problems = Vec::new();
    for (name, &count) in &counts {
        let allowed = baseline.get(name).copied().unwrap_or(0);
        if count > allowed {
            problems.push(format!(
                "{name}: {count} raw sizes or colors, {allowed} allowed; use Theme's scales, or say why in a `// design:` comment"
            ));
        }
    }
    for (name, &allowed) in &baseline {
        let count = counts.get(name).copied().unwrap_or(0);
        if count < allowed {
            problems.push(format!(
                "{name}: down to {count} from {allowed}; lower its count in crates/mochi-core/{BASELINE}"
            ));
        }
    }
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

#[test]
fn finds_raw_values() {
    for line in [
        "font.pixelSize: 13",
        "spacing: 10",
        "radius: 12",
        "anchors.margins: 6",
        "color: \"#ff0000\"",
        "color: Qt.rgba(1, 1, 1, 0.5)",
        "duration: 300",
    ] {
        assert!(breaks_scale(line), "{line}");
    }
    for line in [
        "font.pixelSize: Theme.textBody",
        "spacing: Theme.spaceSmall",
        "spacing: 2",
        "radius: height / 2",
        "radius: 1.5",
        "anchors.margins: -6",
        "x: 12",
        "font.pixelSize: 9 // design: the badge fits in a 14 pixel dot",
        "text: \"#1 in charts\"",
        "spacing: 4 + Theme.spaceSmall",
        "duration: Theme.duration(300)",
    ] {
        assert!(!breaks_scale(line), "{line}");
    }
}
