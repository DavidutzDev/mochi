//! Skin tones. An emoji of a person or a hand has five more forms, one
//! per Fitzpatrick tone, listed in the table. The picker shows each one in
//! a default tone, unless it has a tone of its own, chosen with a long
//! press. Emoji of two people take the same tone for both.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::search::Emoji;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tone {
    /// The yellow one, without a modifier.
    #[default]
    None,
    Light,
    MediumLight,
    Medium,
    MediumDark,
    Dark,
}

impl Tone {
    /// In order, as the swatches show them.
    pub const ALL: [Tone; 6] = [
        Tone::None,
        Tone::Light,
        Tone::MediumLight,
        Tone::Medium,
        Tone::MediumDark,
        Tone::Dark,
    ];

    pub const NAMES: [&str; 6] = [
        "none",
        "light",
        "medium-light",
        "medium",
        "medium-dark",
        "dark",
    ];

    /// Its place in [`Tone::ALL`]: 0 for none, 1 to 5 from light to dark.
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn parse(name: &str) -> Option<Tone> {
        let index = Tone::NAMES.iter().position(|known| *known == name)?;
        Some(Tone::ALL[index])
    }
}

/// The default tone, and the emoji with one of their own.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Tones {
    pub default: Tone,
    /// By the emoji without a tone.
    pub chosen: BTreeMap<String, Tone>,
}

impl Tones {
    /// The tone an emoji shows in.
    pub fn of(&self, glyph: &str) -> Tone {
        self.chosen.get(glyph).copied().unwrap_or(self.default)
    }

    /// The emoji in its tone. Emoji without tones stay as they are.
    pub fn apply(&self, emoji: &Emoji) -> &'static str {
        match (emoji.tones, self.of(emoji.glyph)) {
            (Some(toned), tone) if tone != Tone::None => toned[tone.index() - 1],
            _ => emoji.glyph,
        }
    }
}

/// The emoji a glyph is, in which tone: "👍🏽" is "👍" in medium.
pub fn find<'a>(emoji: &'a [Emoji], glyph: &str) -> Option<(&'a Emoji, Tone)> {
    emoji.iter().find_map(|emoji| {
        if emoji.glyph == glyph {
            return Some((emoji, Tone::None));
        }
        let position = emoji.tones?.iter().position(|toned| *toned == glyph)?;
        Some((emoji, Tone::ALL[position + 1]))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::{TABLE, parse};

    fn table() -> Vec<Emoji> {
        parse(TABLE).unwrap()
    }

    fn get<'a>(emoji: &'a [Emoji], glyph: &str) -> &'a Emoji {
        emoji.iter().find(|emoji| emoji.glyph == glyph).unwrap()
    }

    #[test]
    fn names_round_trip() {
        for tone in Tone::ALL {
            assert_eq!(Tone::parse(Tone::NAMES[tone.index()]), Some(tone));
        }
        assert_eq!(Tone::parse("purple"), None);
        assert_eq!(
            serde_json::to_string(&Tone::MediumDark).unwrap(),
            r#""medium-dark""#
        );
    }

    #[test]
    fn the_default_applies_to_emoji_with_tones() {
        let emoji = table();
        let tones = Tones {
            default: Tone::Medium,
            ..Tones::default()
        };
        assert_eq!(tones.apply(get(&emoji, "👍")), "👍🏽");
        // The variation selector goes: "☝️" is "☝🏽" in medium.
        assert_eq!(tones.apply(get(&emoji, "☝\u{fe0f}")), "☝🏽");
        // A man golfing keeps his joiner and sign.
        assert_eq!(
            tones.apply(get(&emoji, "🏌\u{fe0f}\u{200d}♂\u{fe0f}")),
            "🏌🏽\u{200d}♂\u{fe0f}"
        );
        // Without tones, as is.
        assert_eq!(tones.apply(get(&emoji, "🐱")), "🐱");
        assert_eq!(Tones::default().apply(get(&emoji, "👍")), "👍");
    }

    #[test]
    fn two_people_take_the_same_tone() {
        let emoji = table();
        let tones = Tones {
            default: Tone::Dark,
            ..Tones::default()
        };
        assert_eq!(
            tones.apply(get(&emoji, "🧑\u{200d}🤝\u{200d}🧑")),
            "🧑🏿\u{200d}🤝\u{200d}🧑🏿"
        );
        assert_eq!(tones.apply(get(&emoji, "🤝")), "🤝🏿");
        // Families have no tones in Unicode.
        assert_eq!(tones.apply(get(&emoji, "👪")), "👪");
    }

    #[test]
    fn an_emoji_of_its_own_tone_wins() {
        let emoji = table();
        let mut tones = Tones {
            default: Tone::Light,
            ..Tones::default()
        };
        tones.chosen.insert("👍".into(), Tone::Dark);
        tones.chosen.insert("👋".into(), Tone::None);
        assert_eq!(tones.apply(get(&emoji, "👍")), "👍🏿");
        assert_eq!(tones.apply(get(&emoji, "👋")), "👋");
        assert_eq!(tones.apply(get(&emoji, "👎")), "👎🏻");
    }

    #[test]
    fn a_toned_glyph_finds_its_emoji() {
        let emoji = table();
        let (found, tone) = find(&emoji, "👍🏾").unwrap();
        assert_eq!((found.glyph, tone), ("👍", Tone::MediumDark));
        let (found, tone) = find(&emoji, "🐱").unwrap();
        assert_eq!((found.glyph, tone), ("🐱", Tone::None));
        assert!(find(&emoji, "nope").is_none());
    }

    #[test]
    fn the_table_has_tones_for_people_and_hands() {
        let emoji = table();
        let toned = emoji.iter().filter(|emoji| emoji.tones.is_some()).count();
        assert!(toned > 300, "only {toned} have tones");
        for emoji in &emoji {
            if let Some(toned) = emoji.tones {
                for (index, glyph) in toned.iter().enumerate() {
                    let modifier = char::from_u32(0x1f3fb + index as u32).unwrap();
                    assert!(glyph.contains(modifier), "{} {glyph}", emoji.name);
                }
            }
        }
    }
}
