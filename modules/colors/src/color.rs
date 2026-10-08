//! Colors: reading them from text, and writing them in each format.
//!
//! [`Color::parse`] reads hex (`1e1e2e`, `#1e1e2e`, `#abc`, `#1e1e2e80`),
//! `rgb()`, `hsl()` and `oklch()` in the old comma form or the new space
//! form, and CSS color names. A closing parenthesis may be left out, so a
//! color half typed into the launcher already shows. The formats round the
//! way color pickers usually do: whole degrees and percents, and chroma to
//! two places.

use serde::Deserialize;

/// An sRGB color, 8 bits a channel, with alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// The ways a color can be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Hex,
    Rgb,
    Hsl,
    Oklch,
}

impl Format {
    pub const ALL: [Self; 4] = [Self::Hex, Self::Rgb, Self::Hsl, Self::Oklch];
    pub const NAMES: [&'static str; 4] = ["hex", "rgb", "hsl", "oklch"];

    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|format| format.name() == word)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Hex => "hex",
            Self::Rgb => "rgb",
            Self::Hsl => "hsl",
            Self::Oklch => "oklch",
        }
    }

    /// What the island and the launcher call it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Hex => "HEX",
            Self::Rgb => "RGB",
            Self::Hsl => "HSL",
            Self::Oklch => "OKLCH",
        }
    }
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// The color some text names, or `None`.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim().to_ascii_lowercase();
        if let Some(color) = parse_hex(&text) {
            return Some(color);
        }
        if let Some(color) = named(&text) {
            return Some(color);
        }
        let (name, inside) = text.split_once('(')?;
        let inside = inside.strip_suffix(')').unwrap_or(inside);
        if inside.contains(['(', ')']) {
            return None;
        }
        // `rgb(30, 30, 46, 0.5)` and `rgb(30 30 46 / 50%)` alike.
        let parts: Vec<&str> = inside
            .split(|c: char| c == ',' || c == '/' || c.is_whitespace())
            .filter(|part| !part.is_empty())
            .collect();
        let (channels, alpha) = match parts.as_slice() {
            [a, b, c] => ([*a, *b, *c], None),
            [a, b, c, alpha] => ([*a, *b, *c], Some(parse_alpha(alpha)?)),
            _ => return None,
        };
        let mut color = match name.trim() {
            "rgb" | "rgba" => Self::from_rgb_parts(channels)?,
            "hsl" | "hsla" => Self::from_hsl_parts(channels)?,
            "oklch" => Self::from_oklch_parts(channels)?,
            _ => return None,
        };
        color.a = alpha.unwrap_or(255);
        Some(color)
    }

    fn from_rgb_parts(parts: [&str; 3]) -> Option<Self> {
        let channel = |part: &str| -> Option<u8> {
            let value = match part.strip_suffix('%') {
                Some(percent) => number(percent)? * 255.0 / 100.0,
                None => number(part)?,
            };
            Some(to_byte(value / 255.0))
        };
        Some(Self::rgb(
            channel(parts[0])?,
            channel(parts[1])?,
            channel(parts[2])?,
        ))
    }

    fn from_hsl_parts(parts: [&str; 3]) -> Option<Self> {
        let hue = angle(parts[0])?;
        let fraction = |part: &str| Some(number(part.strip_suffix('%').unwrap_or(part))? / 100.0);
        let (r, g, b) = hsl_to_rgb(hue, fraction(parts[1])?, fraction(parts[2])?);
        Some(Self::rgb(to_byte(r), to_byte(g), to_byte(b)))
    }

    fn from_oklch_parts(parts: [&str; 3]) -> Option<Self> {
        let lightness = match parts[0].strip_suffix('%') {
            Some(percent) => number(percent)? / 100.0,
            None => number(parts[0])?,
        };
        // 100% chroma is 0.4.
        let chroma = match parts[1].strip_suffix('%') {
            Some(percent) => number(percent)? * 0.4 / 100.0,
            None => number(parts[1])?,
        };
        let hue = angle(parts[2])?.to_radians();
        let (r, g, b) = oklab_to_srgb(
            lightness.clamp(0.0, 1.0),
            chroma.max(0.0) * hue.cos(),
            chroma.max(0.0) * hue.sin(),
        );
        Some(Self::rgb(to_byte(r), to_byte(g), to_byte(b)))
    }

    /// `#1e1e2e`, or `#1e1e2e80` with some transparency. It names the color
    /// in the history and in actions.
    pub fn id(self) -> String {
        self.hex(false)
    }

    /// The color as QML reads it: `#rrggbb`, or `#aarrggbb`.
    pub fn qml(self) -> String {
        match self.a {
            255 => self.hex(false),
            a => format!("#{a:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b),
        }
    }

    pub fn format(self, format: Format, uppercase: bool) -> String {
        match format {
            Format::Hex => self.hex(uppercase),
            Format::Rgb => self.rgb_text(),
            Format::Hsl => self.hsl_text(),
            Format::Oklch => self.oklch_text(),
        }
    }

    fn hex(self, uppercase: bool) -> String {
        let text = match self.a {
            255 => format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b),
            a => format!("#{:02x}{:02x}{:02x}{a:02x}", self.r, self.g, self.b),
        };
        if uppercase {
            text.to_ascii_uppercase()
        } else {
            text
        }
    }

    fn rgb_text(self) -> String {
        match self.a {
            255 => format!("rgb({}, {}, {})", self.r, self.g, self.b),
            _ => format!(
                "rgba({}, {}, {}, {})",
                self.r,
                self.g,
                self.b,
                self.alpha_text()
            ),
        }
    }

    fn hsl_text(self) -> String {
        let (hue, saturation, lightness) = self.hsl();
        let hue = hue.round() as u32 % 360;
        let saturation = (saturation * 100.0).round();
        let lightness = (lightness * 100.0).round();
        match self.a {
            255 => format!("hsl({hue}, {saturation}%, {lightness}%)"),
            _ => format!(
                "hsla({hue}, {saturation}%, {lightness}%, {})",
                self.alpha_text()
            ),
        }
    }

    fn oklch_text(self) -> String {
        let (lightness, chroma, hue) = self.oklch();
        let lightness = (lightness * 100.0).round();
        let chroma = (chroma * 100.0).round() / 100.0;
        // A gray has no hue.
        let hue = if chroma == 0.0 {
            0
        } else {
            hue.round() as u32 % 360
        };
        let chroma = trim(chroma);
        match self.a {
            255 => format!("oklch({lightness}% {chroma} {hue})"),
            _ => format!("oklch({lightness}% {chroma} {hue} / {})", self.alpha_text()),
        }
    }

    /// Alpha from 0 to 1, to two places: `0.5`.
    fn alpha_text(self) -> String {
        trim((f64::from(self.a) / 255.0 * 100.0).round() / 100.0)
    }

    /// Hue in degrees, saturation and lightness from 0 to 1.
    fn hsl(self) -> (f64, f64, f64) {
        let [r, g, b] = [self.r, self.g, self.b].map(|channel| f64::from(channel) / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let lightness = (max + min) / 2.0;
        let delta = max - min;
        if delta == 0.0 {
            return (0.0, 0.0, lightness);
        }
        let saturation = delta / (1.0 - (2.0 * lightness - 1.0).abs());
        let hue = if max == r {
            60.0 * ((g - b) / delta).rem_euclid(6.0)
        } else if max == g {
            60.0 * ((b - r) / delta + 2.0)
        } else {
            60.0 * ((r - g) / delta + 4.0)
        };
        (hue, saturation, lightness)
    }

    /// Lightness from 0 to 1, chroma, and hue in degrees.
    fn oklch(self) -> (f64, f64, f64) {
        let [r, g, b] = [self.r, self.g, self.b].map(|channel| linear(f64::from(channel) / 255.0));
        let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
        let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
        let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
        let lightness = 0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s;
        let a = 1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s;
        let b = 0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s;
        let hue = b.atan2(a).to_degrees().rem_euclid(360.0);
        (lightness, a.hypot(b), hue)
    }
}

fn parse_hex(text: &str) -> Option<Color> {
    let digits = text.strip_prefix('#').unwrap_or(text);
    if !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let value = |range: &str| u8::from_str_radix(range, 16).ok();
    // `#abc` is `#aabbcc`.
    let short = |index: usize| value(&digits[index..=index]).map(|nibble| nibble * 17);
    let long = |index: usize| value(&digits[index * 2..index * 2 + 2]);
    let (r, g, b, a) = match digits.len() {
        3 => (short(0)?, short(1)?, short(2)?, 255),
        4 => (short(0)?, short(1)?, short(2)?, short(3)?),
        6 => (long(0)?, long(1)?, long(2)?, 255),
        8 => (long(0)?, long(1)?, long(2)?, long(3)?),
        _ => return None,
    };
    Some(Color { r, g, b, a })
}

fn named(text: &str) -> Option<Color> {
    if text == "transparent" {
        return Some(Color {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        });
    }
    NAMED
        .iter()
        .find(|(name, _)| *name == text)
        .map(|(_, value)| from_value(*value))
}

/// CSS color names that start with `prefix`, for the launcher.
pub fn names_starting(prefix: &str) -> impl Iterator<Item = (&'static str, Color)> + '_ {
    NAMED
        .iter()
        .filter(move |(name, _)| name.starts_with(prefix))
        .map(|(name, value)| (*name, from_value(*value)))
}

/// The CSS name of a color, when it has one.
pub fn name_of(color: Color) -> Option<&'static str> {
    NAMED
        .iter()
        .find(|(_, value)| from_value(*value) == color)
        .map(|(name, _)| *name)
}

fn from_value(value: u32) -> Color {
    let [_, r, g, b] = value.to_be_bytes();
    Color::rgb(r, g, b)
}

fn number(text: &str) -> Option<f64> {
    text.parse::<f64>().ok().filter(|value| value.is_finite())
}

/// Degrees, with or without `deg`.
fn angle(text: &str) -> Option<f64> {
    number(text.strip_suffix("deg").unwrap_or(text))
}

/// `0.5` or `50%`, as 0 to 255.
fn parse_alpha(text: &str) -> Option<u8> {
    let value = match text.strip_suffix('%') {
        Some(percent) => number(percent)? / 100.0,
        None => number(text)?,
    };
    Some(to_byte(value))
}

/// A channel from 0 to 1, as 0 to 255, clamped.
fn to_byte(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// A number without trailing zeros: `0.5`, `0.03`, `0`.
fn trim(value: f64) -> String {
    let text = format!("{value:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    match text {
        "" | "-0" => "0".to_owned(),
        text => text.to_owned(),
    }
}

/// sRGB to linear light.
fn linear(channel: f64) -> f64 {
    if channel <= 0.040_45 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear light to sRGB.
fn gamma(channel: f64) -> f64 {
    if channel <= 0.003_130_8 {
        channel * 12.92
    } else {
        1.055 * channel.powf(1.0 / 2.4) - 0.055
    }
}

/// Hue in degrees, saturation and lightness from 0 to 1, to sRGB from 0 to
/// 1.
fn hsl_to_rgb(hue: f64, saturation: f64, lightness: f64) -> (f64, f64, f64) {
    let saturation = saturation.clamp(0.0, 1.0);
    let lightness = lightness.clamp(0.0, 1.0);
    let hue = hue.rem_euclid(360.0) / 60.0;
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let x = chroma * (1.0 - (hue.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match hue as u32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let m = lightness - chroma / 2.0;
    (r + m, g + m, b + m)
}

/// OKLab to sRGB from 0 to 1; colors outside sRGB clamp later.
fn oklab_to_srgb(lightness: f64, a: f64, b: f64) -> (f64, f64, f64) {
    let l = (lightness + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
    let m = (lightness - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
    let s = (lightness - 0.089_484_177_5 * a - 1.291_485_548_0 * b).powi(3);
    let r = 4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s;
    let g = -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s;
    let b = -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701_0 * s;
    (gamma(r), gamma(g), gamma(b))
}

/// The CSS color names, without `transparent`.
const NAMED: [(&str, u32); 148] = [
    ("aliceblue", 0xf0f8ff),
    ("antiquewhite", 0xfaebd7),
    ("aqua", 0x00ffff),
    ("aquamarine", 0x7fffd4),
    ("azure", 0xf0ffff),
    ("beige", 0xf5f5dc),
    ("bisque", 0xffe4c4),
    ("black", 0x000000),
    ("blanchedalmond", 0xffebcd),
    ("blue", 0x0000ff),
    ("blueviolet", 0x8a2be2),
    ("brown", 0xa52a2a),
    ("burlywood", 0xdeb887),
    ("cadetblue", 0x5f9ea0),
    ("chartreuse", 0x7fff00),
    ("chocolate", 0xd2691e),
    ("coral", 0xff7f50),
    ("cornflowerblue", 0x6495ed),
    ("cornsilk", 0xfff8dc),
    ("crimson", 0xdc143c),
    ("cyan", 0x00ffff),
    ("darkblue", 0x00008b),
    ("darkcyan", 0x008b8b),
    ("darkgoldenrod", 0xb8860b),
    ("darkgray", 0xa9a9a9),
    ("darkgreen", 0x006400),
    ("darkgrey", 0xa9a9a9),
    ("darkkhaki", 0xbdb76b),
    ("darkmagenta", 0x8b008b),
    ("darkolivegreen", 0x556b2f),
    ("darkorange", 0xff8c00),
    ("darkorchid", 0x9932cc),
    ("darkred", 0x8b0000),
    ("darksalmon", 0xe9967a),
    ("darkseagreen", 0x8fbc8f),
    ("darkslateblue", 0x483d8b),
    ("darkslategray", 0x2f4f4f),
    ("darkslategrey", 0x2f4f4f),
    ("darkturquoise", 0x00ced1),
    ("darkviolet", 0x9400d3),
    ("deeppink", 0xff1493),
    ("deepskyblue", 0x00bfff),
    ("dimgray", 0x696969),
    ("dimgrey", 0x696969),
    ("dodgerblue", 0x1e90ff),
    ("firebrick", 0xb22222),
    ("floralwhite", 0xfffaf0),
    ("forestgreen", 0x228b22),
    ("fuchsia", 0xff00ff),
    ("gainsboro", 0xdcdcdc),
    ("ghostwhite", 0xf8f8ff),
    ("gold", 0xffd700),
    ("goldenrod", 0xdaa520),
    ("gray", 0x808080),
    ("green", 0x008000),
    ("greenyellow", 0xadff2f),
    ("grey", 0x808080),
    ("honeydew", 0xf0fff0),
    ("hotpink", 0xff69b4),
    ("indianred", 0xcd5c5c),
    ("indigo", 0x4b0082),
    ("ivory", 0xfffff0),
    ("khaki", 0xf0e68c),
    ("lavender", 0xe6e6fa),
    ("lavenderblush", 0xfff0f5),
    ("lawngreen", 0x7cfc00),
    ("lemonchiffon", 0xfffacd),
    ("lightblue", 0xadd8e6),
    ("lightcoral", 0xf08080),
    ("lightcyan", 0xe0ffff),
    ("lightgoldenrodyellow", 0xfafad2),
    ("lightgray", 0xd3d3d3),
    ("lightgreen", 0x90ee90),
    ("lightgrey", 0xd3d3d3),
    ("lightpink", 0xffb6c1),
    ("lightsalmon", 0xffa07a),
    ("lightseagreen", 0x20b2aa),
    ("lightskyblue", 0x87cefa),
    ("lightslategray", 0x778899),
    ("lightslategrey", 0x778899),
    ("lightsteelblue", 0xb0c4de),
    ("lightyellow", 0xffffe0),
    ("lime", 0x00ff00),
    ("limegreen", 0x32cd32),
    ("linen", 0xfaf0e6),
    ("magenta", 0xff00ff),
    ("maroon", 0x800000),
    ("mediumaquamarine", 0x66cdaa),
    ("mediumblue", 0x0000cd),
    ("mediumorchid", 0xba55d3),
    ("mediumpurple", 0x9370db),
    ("mediumseagreen", 0x3cb371),
    ("mediumslateblue", 0x7b68ee),
    ("mediumspringgreen", 0x00fa9a),
    ("mediumturquoise", 0x48d1cc),
    ("mediumvioletred", 0xc71585),
    ("midnightblue", 0x191970),
    ("mintcream", 0xf5fffa),
    ("mistyrose", 0xffe4e1),
    ("moccasin", 0xffe4b5),
    ("navajowhite", 0xffdead),
    ("navy", 0x000080),
    ("oldlace", 0xfdf5e6),
    ("olive", 0x808000),
    ("olivedrab", 0x6b8e23),
    ("orange", 0xffa500),
    ("orangered", 0xff4500),
    ("orchid", 0xda70d6),
    ("palegoldenrod", 0xeee8aa),
    ("palegreen", 0x98fb98),
    ("paleturquoise", 0xafeeee),
    ("palevioletred", 0xdb7093),
    ("papayawhip", 0xffefd5),
    ("peachpuff", 0xffdab9),
    ("peru", 0xcd853f),
    ("pink", 0xffc0cb),
    ("plum", 0xdda0dd),
    ("powderblue", 0xb0e0e6),
    ("purple", 0x800080),
    ("rebeccapurple", 0x663399),
    ("red", 0xff0000),
    ("rosybrown", 0xbc8f8f),
    ("royalblue", 0x4169e1),
    ("saddlebrown", 0x8b4513),
    ("salmon", 0xfa8072),
    ("sandybrown", 0xf4a460),
    ("seagreen", 0x2e8b57),
    ("seashell", 0xfff5ee),
    ("sienna", 0xa0522d),
    ("silver", 0xc0c0c0),
    ("skyblue", 0x87ceeb),
    ("slateblue", 0x6a5acd),
    ("slategray", 0x708090),
    ("slategrey", 0x708090),
    ("snow", 0xfffafa),
    ("springgreen", 0x00ff7f),
    ("steelblue", 0x4682b4),
    ("tan", 0xd2b48c),
    ("teal", 0x008080),
    ("thistle", 0xd8bfd8),
    ("tomato", 0xff6347),
    ("turquoise", 0x40e0d0),
    ("violet", 0xee82ee),
    ("wheat", 0xf5deb3),
    ("white", 0xffffff),
    ("whitesmoke", 0xf5f5f5),
    ("yellow", 0xffff00),
    ("yellowgreen", 0x9acd32),
];

#[cfg(test)]
mod tests {
    use super::*;

    const MOCHA: Color = Color::rgb(0x1e, 0x1e, 0x2e);

    fn every(color: Color, uppercase: bool) -> [String; 4] {
        Format::ALL.map(|format| color.format(format, uppercase))
    }

    #[test]
    fn writes_every_format() {
        assert_eq!(
            every(MOCHA, false),
            [
                "#1e1e2e",
                "rgb(30, 30, 46)",
                "hsl(240, 21%, 15%)",
                "oklch(24% 0.03 284)"
            ]
        );
        assert_eq!(MOCHA.format(Format::Hex, true), "#1E1E2E");
        assert_eq!(
            every(Color::rgb(255, 0, 0), false),
            [
                "#ff0000",
                "rgb(255, 0, 0)",
                "hsl(0, 100%, 50%)",
                "oklch(63% 0.26 29)"
            ]
        );
        assert_eq!(
            every(Color::rgb(255, 255, 255), false),
            [
                "#ffffff",
                "rgb(255, 255, 255)",
                "hsl(0, 0%, 100%)",
                "oklch(100% 0 0)"
            ]
        );
        assert_eq!(
            every(Color::rgb(0, 0, 0), false)[2..],
            ["hsl(0, 0%, 0%)", "oklch(0% 0 0)"]
        );
        assert_eq!(
            Color::rgb(0x66, 0x33, 0x99).format(Format::Hsl, false),
            "hsl(270, 50%, 40%)"
        );
        assert_eq!(
            Color::rgb(0, 0, 255).format(Format::Oklch, false),
            "oklch(45% 0.31 264)"
        );
        assert_eq!(
            Color::rgb(0, 255, 0).format(Format::Oklch, false),
            "oklch(87% 0.29 142)"
        );
    }

    #[test]
    fn writes_transparency() {
        let half = Color { a: 128, ..MOCHA };
        assert_eq!(
            every(half, false),
            [
                "#1e1e2e80",
                "rgba(30, 30, 46, 0.5)",
                "hsla(240, 21%, 15%, 0.5)",
                "oklch(24% 0.03 284 / 0.5)"
            ]
        );
        assert_eq!(half.id(), "#1e1e2e80");
        assert_eq!(half.qml(), "#801e1e2e");
        assert_eq!(MOCHA.qml(), "#1e1e2e");
        let clear = Color { a: 0, ..MOCHA };
        assert_eq!(clear.format(Format::Rgb, false), "rgba(30, 30, 46, 0)");
    }

    #[test]
    fn reads_hex() {
        for text in ["1e1e2e", "#1e1e2e", "#1E1E2E", "  #1e1e2e "] {
            assert_eq!(Color::parse(text), Some(MOCHA), "{text}");
        }
        assert_eq!(Color::parse("#abc"), Some(Color::rgb(0xaa, 0xbb, 0xcc)));
        assert_eq!(
            Color::parse("#abcd"),
            Some(Color {
                r: 0xaa,
                g: 0xbb,
                b: 0xcc,
                a: 0xdd
            })
        );
        assert_eq!(Color::parse("#1e1e2e80"), Some(Color { a: 0x80, ..MOCHA }));
        for text in ["", "#", "#12", "1e1e2", "#1e1e2e8", "#1e1e2g", "##1e1e2e"] {
            assert_eq!(Color::parse(text), None, "{text}");
        }
    }

    #[test]
    fn reads_rgb() {
        for text in [
            "rgb(30, 30, 46)",
            "rgb(30 30 46)",
            "RGB(30,30,46)",
            "rgb(30 30 46",
            "rgba(30, 30, 46, 1)",
            "rgb(30 30 46 / 100%)",
            "rgb(11.77% 11.77% 18.04%)",
        ] {
            assert_eq!(Color::parse(text), Some(MOCHA), "{text}");
        }
        assert_eq!(
            Color::parse("rgb(30 30 46 / 50%)"),
            Some(Color { a: 128, ..MOCHA })
        );
        assert_eq!(
            Color::parse("rgba(30, 30, 46, 0.5)"),
            Some(Color { a: 128, ..MOCHA })
        );
        // Out of range clamps, as in CSS.
        assert_eq!(Color::parse("rgb(300 -4 46)"), Some(Color::rgb(255, 0, 46)));
        for text in [
            "rgb(30 30)",
            "rgb(a b c)",
            "rgb(1 2 3 4 5)",
            "rgb((1 2 3)",
            "rgx(1 2 3)",
        ] {
            assert_eq!(Color::parse(text), None, "{text}");
        }
    }

    #[test]
    fn reads_hsl() {
        assert_eq!(
            Color::parse("hsl(240 21% 15%)"),
            Some(Color::rgb(30, 30, 46))
        );
        assert_eq!(
            Color::parse("hsl(240deg, 21%, 15%)"),
            Some(Color::rgb(30, 30, 46))
        );
        assert_eq!(Color::parse("hsl(0 100% 50%)"), Some(Color::rgb(255, 0, 0)));
        assert_eq!(
            Color::parse("hsl(120, 100%, 25%)"),
            Some(Color::rgb(0, 128, 0))
        );
        assert_eq!(
            Color::parse("hsl(-120 100% 50%)"),
            Some(Color::rgb(0, 0, 255))
        );
        assert_eq!(
            Color::parse("hsla(270, 50%, 40%, 0.5)"),
            Some(Color {
                a: 128,
                ..Color::rgb(0x66, 0x33, 0x99)
            })
        );
        assert_eq!(Color::parse("hsl(1 2)"), None);
    }

    #[test]
    fn reads_oklch() {
        assert_eq!(
            Color::parse("oklch(62.8% 0.2577 29.23)"),
            Some(Color::rgb(255, 0, 0))
        );
        assert_eq!(
            Color::parse("oklch(0.628 0.2577 29.23deg)"),
            Some(Color::rgb(255, 0, 0))
        );
        assert_eq!(
            Color::parse("oklch(100% 0 0)"),
            Some(Color::rgb(255, 255, 255))
        );
        assert_eq!(Color::parse("oklch(0% 0 0)"), Some(Color::rgb(0, 0, 0)));
        assert_eq!(
            Color::parse("oklch(50% 0.1 180 / 0.5)").map(|color| color.a),
            Some(128)
        );
        // Outside sRGB clamps.
        assert_eq!(
            Color::parse("oklch(90% 0.4 30)").map(|color| color.r),
            Some(255)
        );
    }

    #[test]
    fn reads_names() {
        assert_eq!(
            Color::parse("rebeccapurple"),
            Some(Color::rgb(0x66, 0x33, 0x99))
        );
        assert_eq!(Color::parse("Red"), Some(Color::rgb(255, 0, 0)));
        assert_eq!(Color::parse("transparent").map(|color| color.a), Some(0));
        assert_eq!(Color::parse("reb"), None);
        assert_eq!(name_of(Color::rgb(0x66, 0x33, 0x99)), Some("rebeccapurple"));
        assert_eq!(name_of(MOCHA), None);
        let names: Vec<&str> = names_starting("lightg").map(|(name, _)| name).collect();
        assert_eq!(
            names,
            [
                "lightgoldenrodyellow",
                "lightgray",
                "lightgreen",
                "lightgrey"
            ]
        );
        // Sorted, so the launcher lists them in order, and no name twice.
        assert!(NAMED.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }

    #[test]
    fn formats_read_back() {
        // Hex and rgb() are exact; the rounded hsl() and oklch() land within
        // a step or two.
        let mut off = 0;
        for value in (0..=0xff_ffff_u32).step_by(0x01_0305) {
            let color = from_value(value);
            for format in Format::ALL {
                let text = color.format(format, false);
                let back = Color::parse(&text).unwrap_or_else(|| panic!("{text}"));
                let distance = [
                    back.r.abs_diff(color.r),
                    back.g.abs_diff(color.g),
                    back.b.abs_diff(color.b),
                ]
                .into_iter()
                .max()
                .unwrap_or(0);
                match format {
                    Format::Hex | Format::Rgb => assert_eq!(back, color, "{text}"),
                    Format::Hsl => assert!(distance <= 3, "{text} is {back:?}, not {color:?}"),
                    Format::Oklch => off = off.max(distance),
                }
            }
        }
        // Chroma to two places is coarse for dark, saturated colors.
        assert!(off <= 40, "oklch() was {off} off");
    }

    #[test]
    fn formats_by_name() {
        assert_eq!(Format::parse("oklch"), Some(Format::Oklch));
        assert_eq!(Format::parse("cmyk"), None);
        assert_eq!(Format::NAMES, Format::ALL.map(Format::name));
        assert_eq!(Format::Hsl.label(), "HSL");
    }
}
