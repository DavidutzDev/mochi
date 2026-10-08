//! Theme presets: named palettes, each in a dark and a light version, and
//! one made from the wallpaper. A preset gives every color role; what
//! `theme.toml` sets in `[colors]` goes over it.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use toml::{Table, Value};

/// The presets, in the order the settings list them. `wallpaper` is made
/// from the current wallpaper instead.
pub const PRESETS: [&str; 7] = [
    "obsidian",
    "catppuccin",
    "nord",
    "gruvbox",
    "rose-pine",
    "tokyo-night",
    "wallpaper",
];

/// The roles, in the order of a palette below.
const ROLES: [&str; 12] = [
    "background",
    "surface",
    "raised",
    "highlight",
    "foreground",
    "muted",
    "accent",
    "on_accent",
    "danger",
    "success",
    "border",
    "shadow",
];

type Palette = [&'static str; 12];

/// Each preset's dark and light palettes, in the order of [`ROLES`]. The
/// background is slightly see-through, like the default.
fn palettes(preset: &str) -> Option<(Palette, Palette)> {
    Some(match preset {
        "obsidian" => (
            [
                "#f5000000",
                "#1c1c1e",
                "#2c2c2e",
                "#3a3a3c",
                "#ffffff",
                "#8e8e93",
                "#ff9f0a",
                "#000000",
                "#ff453a",
                "#30d158",
                "#0dffffff",
                "#59000000",
            ],
            [
                "#f5f5f5f7",
                "#ffffff",
                "#e5e5ea",
                "#d1d1d6",
                "#000000",
                "#6e6e73",
                "#ff9500",
                "#000000",
                "#ff3b30",
                "#34c759",
                "#1a000000",
                "#26000000",
            ],
        ),
        "catppuccin" => (
            [
                "#f511111b",
                "#1e1e2e",
                "#313244",
                "#45475a",
                "#cdd6f4",
                "#7f849c",
                "#cba6f7",
                "#11111b",
                "#f38ba8",
                "#a6e3a1",
                "#0dffffff",
                "#59000000",
            ],
            [
                "#f5eff1f5",
                "#e6e9ef",
                "#ccd0da",
                "#bcc0cc",
                "#4c4f69",
                "#8c8fa1",
                "#8839ef",
                "#eff1f5",
                "#d20f39",
                "#40a02b",
                "#1a000000",
                "#26000000",
            ],
        ),
        "nord" => (
            [
                "#f52e3440",
                "#3b4252",
                "#434c5e",
                "#4c566a",
                "#eceff4",
                "#a3abb9",
                "#88c0d0",
                "#2e3440",
                "#bf616a",
                "#a3be8c",
                "#0dffffff",
                "#59000000",
            ],
            [
                "#f5eceff4",
                "#e5e9f0",
                "#d8dee9",
                "#c8d0dc",
                "#2e3440",
                "#4c566a",
                "#5e81ac",
                "#eceff4",
                "#bf616a",
                "#6f8f55",
                "#1a000000",
                "#26000000",
            ],
        ),
        "gruvbox" => (
            [
                "#f51d2021",
                "#282828",
                "#3c3836",
                "#504945",
                "#ebdbb2",
                "#928374",
                "#fe8019",
                "#1d2021",
                "#fb4934",
                "#b8bb26",
                "#0dffffff",
                "#59000000",
            ],
            [
                "#f5f9f5d7",
                "#fbf1c7",
                "#ebdbb2",
                "#d5c4a1",
                "#3c3836",
                "#7c6f64",
                "#af3a03",
                "#fbf1c7",
                "#9d0006",
                "#79740e",
                "#1a000000",
                "#26000000",
            ],
        ),
        "rose-pine" => (
            [
                "#f5191724",
                "#1f1d2e",
                "#26233a",
                "#403d52",
                "#e0def4",
                "#908caa",
                "#c4a7e7",
                "#191724",
                "#eb6f92",
                "#9ccfd8",
                "#0dffffff",
                "#59000000",
            ],
            [
                "#f5faf4ed",
                "#fffaf3",
                "#f2e9e1",
                "#dfdad9",
                "#575279",
                "#797593",
                "#907aa9",
                "#faf4ed",
                "#b4637a",
                "#56949f",
                "#1a000000",
                "#26000000",
            ],
        ),
        "tokyo-night" => (
            [
                "#f516161e",
                "#1a1b26",
                "#292e42",
                "#414868",
                "#c0caf5",
                "#787c99",
                "#7aa2f7",
                "#16161e",
                "#f7768e",
                "#9ece6a",
                "#0dffffff",
                "#59000000",
            ],
            [
                "#f5e1e2e7",
                "#d5d6db",
                "#c4c8da",
                "#b6bbd2",
                "#343b58",
                "#6a6f8e",
                "#2e7de9",
                "#e1e2e7",
                "#f52a65",
                "#587539",
                "#1a000000",
                "#26000000",
            ],
        ),
        _ => return None,
    })
}

fn table(palette: &[String]) -> Table {
    ROLES
        .iter()
        .zip(palette)
        .map(|(role, color)| ((*role).to_owned(), Value::String(color.clone())))
        .collect()
}

/// A preset's colors, as `[colors]` in `theme.toml` would set them. For
/// `wallpaper`, `wallpaper` names the image, or `auto` for the one the
/// wallpaper daemon shows; it fails when there's none to read.
pub fn colors(preset: &str, light: bool, wallpaper: &str) -> Result<Table, String> {
    if preset == "wallpaper" {
        let source = cached_source(wallpaper)?;
        return Ok(table(&cached_palette(&source, light)?));
    }
    let (dark, bright) = palettes(preset).ok_or_else(|| {
        format!(
            "unknown preset {preset:?}, expected one of {}",
            PRESETS.join(", ")
        )
    })?;
    let palette = if light { bright } else { dark };
    Ok(table(&palette.map(str::to_owned)))
}

/// Each preset's swatches, for the settings: its background, surface,
/// accent and foreground.
pub fn swatches(preset: &str, light: bool) -> Vec<String> {
    palettes(preset)
        .map(|(dark, bright)| if light { bright } else { dark })
        .map(|palette| [palette[0], palette[1], palette[6], palette[4]])
        .map(|colors| colors.iter().map(|color| (*color).to_owned()).collect())
        .unwrap_or_default()
}

/// How long a wallpaper found with `auto` counts as current: settings
/// changes read the theme again, and asking the daemon each time is slow.
const FRESH: Duration = Duration::from_secs(5);

static SOURCE: Mutex<Option<(String, Instant, Source)>> = Mutex::new(None);
/// A palette made from a wallpaper: the source, its modification time,
/// whether it's the light version, and the colors.
type Made = (Source, Option<SystemTime>, bool, Vec<String>);

/// The last palette made.
static PALETTE: Mutex<Option<Made>> = Mutex::new(None);

fn cached_source(wallpaper: &str) -> Result<Source, String> {
    let mut cache = SOURCE.lock().expect("the lock is never poisoned");
    if let Some((spec, at, source)) = cache.as_ref()
        && spec == wallpaper
        && at.elapsed() < FRESH
    {
        return Ok(source.clone());
    }
    let found = source(wallpaper)?;
    *cache = Some((wallpaper.to_owned(), Instant::now(), found.clone()));
    Ok(found)
}

fn cached_palette(source: &Source, light: bool) -> Result<Vec<String>, String> {
    let modified = match source {
        Source::Image(path) => std::fs::metadata(path)
            .and_then(|meta| meta.modified())
            .ok(),
        Source::Color(_) => None,
    };
    let mut cache = PALETTE.lock().expect("the lock is never poisoned");
    if let Some((cached, at, cached_light, palette)) = cache.as_ref()
        && cached == source
        && *at == modified
        && *cached_light == light
    {
        return Ok(palette.clone());
    }
    let palette = from_source(source, light)?;
    *cache = Some((source.clone(), modified, light, palette.clone()));
    Ok(palette)
}

/// What the wallpaper shows: an image, or a plain color.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    Image(PathBuf),
    Color([u8; 3]),
}

/// The wallpaper: the path given, or with `auto`, the one awww, swww or
/// hyprpaper shows.
pub fn source(wallpaper: &str) -> Result<Source, String> {
    if wallpaper != "auto" && !wallpaper.is_empty() {
        let path = expand(wallpaper);
        return if path.is_file() {
            Ok(Source::Image(path))
        } else {
            Err(format!("no wallpaper at {}", path.display()))
        };
    }
    let run = |program: &str, args: &[&str]| -> Option<String> {
        let output = Command::new(program).args(args).output().ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
    };
    for program in ["awww", "swww"] {
        if let Some(found) = run(program, &["query"]).as_deref().and_then(from_query) {
            return Ok(found);
        }
    }
    if let Some(found) = run("hyprctl", &["hyprpaper", "listactive"])
        .as_deref()
        .and_then(from_hyprpaper)
    {
        return Ok(found);
    }
    Err(
        "no wallpaper found: awww, swww and hyprpaper show none; set `wallpaper` to an image"
            .into(),
    )
}

fn expand(path: &str) -> PathBuf {
    match path.strip_prefix("~/") {
        Some(rest) => std::env::var_os("HOME")
            .map_or_else(|| PathBuf::from(path), |home| Path::new(&home).join(rest)),
        None => PathBuf::from(path),
    }
}

/// `awww query` or `swww query`: `: DP-3: 1920x1080, scale: 1, currently
/// displaying: image: /path` or `... color: 1e1e2e`. The first monitor's.
fn from_query(text: &str) -> Option<Source> {
    let line = text
        .lines()
        .find(|line| line.contains("currently displaying:"))?;
    let shown = line.split("currently displaying:").nth(1)?.trim();
    if let Some(path) = shown.strip_prefix("image:") {
        let path = PathBuf::from(path.trim());
        return path.is_file().then_some(Source::Image(path));
    }
    let hex = shown.strip_prefix("color:")?.trim().trim_start_matches('#');
    let value = u32::from_str_radix(hex.get(..6)?, 16).ok()?;
    Some(Source::Color([
        (value >> 16) as u8,
        (value >> 8) as u8,
        value as u8,
    ]))
}

/// `hyprctl hyprpaper listactive`: `DP-3 = /path` per monitor.
fn from_hyprpaper(text: &str) -> Option<Source> {
    text.lines().find_map(|line| {
        let path = PathBuf::from(line.split_once(" = ")?.1.trim());
        path.is_file().then_some(Source::Image(path))
    })
}

/// A palette from the wallpaper, in the order of [`ROLES`].
pub fn from_source(source: &Source, light: bool) -> Result<Vec<String>, String> {
    let pixels: Vec<[u8; 3]> = match source {
        Source::Color(color) => vec![*color],
        Source::Image(path) => {
            let image = image::open(path).map_err(|error| {
                format!("cannot read the wallpaper {}: {error}", path.display())
            })?;
            image
                .thumbnail(64, 64)
                .to_rgb8()
                .pixels()
                .map(|pixel| pixel.0)
                .collect()
        }
    };
    Ok(generate(&pixels, light))
}

/// The palette for a set of pixels: the accent from the hue that's most
/// colorful and most present, the backgrounds from the same hue nearly
/// grey, and lightness chosen for contrast, not taken from the image.
pub fn generate(pixels: &[[u8; 3]], light: bool) -> Vec<String> {
    // Weighted hue histogram: colorful, mid-light pixels count most.
    let mut bins = [0.0f64; 36];
    let mut chroma_in = [0.0f64; 36];
    let mut hue_x = 0.0;
    let mut hue_y = 0.0;
    for pixel in pixels {
        let (l, c, h) = oklch(*pixel);
        let weight = c * (1.0 - (l - 0.6).abs());
        hue_x += h.to_radians().cos() * c;
        hue_y += h.to_radians().sin() * c;
        if c < 0.03 {
            continue;
        }
        let bin = ((h / 10.0) as usize).min(35);
        bins[bin] += weight;
        chroma_in[bin] = chroma_in[bin].max(c);
    }
    let best = (0..36)
        .max_by(|a, b| bins[*a].total_cmp(&bins[*b]))
        .unwrap_or(0);
    let colorful = bins[best] > 0.0;
    // A grey wallpaper keeps a quiet blue accent.
    let accent_hue = if colorful {
        best as f64 * 10.0 + 5.0
    } else {
        250.0
    };
    let accent_chroma = if colorful {
        chroma_in[best].clamp(0.09, 0.17)
    } else {
        0.06
    };
    let mean_hue = hue_y.atan2(hue_x).to_degrees().rem_euclid(360.0);
    let neutral_hue = if colorful { accent_hue } else { mean_hue };
    let tint = if colorful { 0.02 } else { 0.0 };

    let color = |l: f64, c: f64, h: f64| hex(lch_to_rgb(l, c, h));
    let (background, surface, raised, highlight, foreground, muted, accent_l) = if light {
        (0.97, 0.94, 0.89, 0.84, 0.22, 0.50, 0.55)
    } else {
        (0.15, 0.21, 0.27, 0.33, 0.97, 0.70, 0.76)
    };
    let accent = lch_to_rgb(accent_l, accent_chroma, accent_hue);
    // Text on the accent: black or white, whichever reads better.
    let on_accent = if luminance(accent) > 0.4 {
        "#000000".to_owned()
    } else {
        "#ffffff".to_owned()
    };
    vec![
        format!("#f5{}", &color(background, tint, neutral_hue)[1..]),
        color(surface, tint, neutral_hue),
        color(raised, tint * 1.2, neutral_hue),
        color(highlight, tint * 1.4, neutral_hue),
        color(foreground, tint * 0.5, neutral_hue),
        color(muted, tint, neutral_hue),
        hex(accent),
        on_accent,
        color(if light { 0.55 } else { 0.70 }, 0.17, 25.0),
        color(if light { 0.55 } else { 0.78 }, 0.15, 145.0),
        if light { "#1a000000" } else { "#0dffffff" }.to_owned(),
        if light { "#26000000" } else { "#59000000" }.to_owned(),
    ]
}

fn hex([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn to_linear(channel: u8) -> f64 {
    let value = f64::from(channel) / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn from_linear(value: f64) -> u8 {
    let value = value.clamp(0.0, 1.0);
    let encoded = if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0).round() as u8
}

fn luminance([r, g, b]: [u8; 3]) -> f64 {
    0.2126 * to_linear(r) + 0.7152 * to_linear(g) + 0.0722 * to_linear(b)
}

/// OKLCH: lightness 0 to 1, chroma, hue in degrees.
fn oklch([r, g, b]: [u8; 3]) -> (f64, f64, f64) {
    let (r, g, b) = (to_linear(r), to_linear(g), to_linear(b));
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    let lightness = 0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s;
    let a = 1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s;
    let bb = 0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s;
    let chroma = (a * a + bb * bb).sqrt();
    (
        lightness,
        chroma,
        bb.atan2(a).to_degrees().rem_euclid(360.0),
    )
}

/// An OKLCH color in sRGB, with the chroma lowered until it fits.
fn lch_to_rgb(lightness: f64, chroma: f64, hue: f64) -> [u8; 3] {
    let mut chroma = chroma;
    loop {
        let (a, b) = (
            chroma * hue.to_radians().cos(),
            chroma * hue.to_radians().sin(),
        );
        let l = (lightness + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
        let m = (lightness - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
        let s = (lightness - 0.089_484_177_5 * a - 1.291_485_548 * b).powi(3);
        let r = 4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s;
        let g = -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s;
        let bl = -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701 * s;
        let inside = [r, g, bl]
            .iter()
            .all(|value| (-0.001..=1.001).contains(value));
        if inside || chroma <= 0.0 {
            return [from_linear(r), from_linear(g), from_linear(bl)];
        }
        chroma = (chroma - 0.005).max(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contrast(a: &str, b: &str) -> f64 {
        let rgb = |text: &str| {
            let digits = text.trim_start_matches('#');
            let digits = &digits[digits.len() - 6..];
            let value = u32::from_str_radix(digits, 16).unwrap();
            [(value >> 16) as u8, (value >> 8) as u8, value as u8]
        };
        let (x, y) = (luminance(rgb(a)), luminance(rgb(b)));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn every_preset_has_both_palettes_and_reads_well() {
        for preset in PRESETS.iter().filter(|preset| **preset != "wallpaper") {
            for light in [false, true] {
                let colors = colors(preset, light, "").unwrap();
                assert_eq!(colors.len(), 12, "{preset}");
                let get = |role: &str| colors[role].as_str().unwrap().to_owned();
                let text = contrast(&get("foreground"), &get("surface"));
                assert!(text >= 4.5, "{preset} light={light}: text {text:.1}");
                let accent = contrast(&get("accent"), &get("on_accent"));
                assert!(accent >= 2.5, "{preset} light={light}: accent {accent:.1}");
            }
        }
        assert!(colors("teal", false, "").is_err());
    }

    #[test]
    fn the_default_is_obsidian_dark() {
        let colors = colors("obsidian", false, "").unwrap();
        let theme = crate::config::parse_theme("", Path::new("theme.toml")).unwrap();
        assert_eq!(
            colors["accent"].as_str(),
            Some(theme.colors.accent.as_str())
        );
        assert_eq!(
            colors["background"].as_str(),
            Some(theme.colors.background.as_str())
        );
    }

    #[test]
    fn a_wallpaper_gives_its_hue_and_readable_text() {
        // A teal image, a little noisy.
        let pixels: Vec<[u8; 3]> = (0..400)
            .map(|index| [20 + (index % 7) as u8, 140, 150 + (index % 11) as u8])
            .collect();
        for light in [false, true] {
            let palette = generate(&pixels, light);
            let (_, _, hue) = oklch({
                let digits = &palette[6][1..];
                let value = u32::from_str_radix(digits, 16).unwrap();
                [(value >> 16) as u8, (value >> 8) as u8, value as u8]
            });
            assert!((170.0..230.0).contains(&hue), "accent hue {hue}");
            assert!(contrast(&palette[4], &palette[1]) >= 7.0, "{palette:?}");
            assert!(palette[0].starts_with("#f5"));
        }
        // A plain black wallpaper: grey, with a quiet accent.
        let palette = generate(&[[0, 0, 0]], false);
        assert!(contrast(&palette[4], &palette[1]) >= 7.0);
    }

    #[test]
    fn wallpaper_daemons_are_read() {
        let awww = ": DP-3: 1920x1080, scale: 1, currently displaying: color: 1e1e2e\n";
        assert_eq!(from_query(awww), Some(Source::Color([0x1e, 0x1e, 0x2e])));
        assert_eq!(from_hyprpaper("DP-3 = /nonexistent.png\n"), None);
    }
}
