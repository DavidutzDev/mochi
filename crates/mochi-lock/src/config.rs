//! What mochid tells the lock: the PAM service and how the face looks. It's
//! a JSON file named with `--config`; without one, run by hand from a TTY,
//! the defaults lock just the same.

use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    /// The PAM service, a file in `/etc/pam.d`.
    pub pam_service: String,
    /// The font for the clock and the text: Inter, which Mochi's packages
    /// bring. Without one, the face has no text, only the dots.
    pub font: Option<PathBuf>,
    pub colors: Colors,
    /// "24" or "12".
    pub hours: String,
    /// The lock screen's renderer; without it, the lock draws its own face.
    pub renderer: Option<Renderer>,
    /// The theme, as mochid sends it to the shell, for the renderer.
    pub theme: serde_json::Value,
    /// The wallpaper, blurred, for the renderer's background.
    pub background: Option<PathBuf>,
}

/// Where the renderer is: Quickshell, and the lock screen's shell for it.
#[derive(Debug, Clone, Deserialize)]
pub struct Renderer {
    pub quickshell: PathBuf,
    pub shell: PathBuf,
}

/// The theme's roles the face uses, as `#rrggbb` or `#aarrggbb`.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Colors {
    pub background: String,
    pub raised: String,
    pub foreground: String,
    pub muted: String,
    pub accent: String,
    pub danger: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            pam_service: "mochi".into(),
            font: None,
            colors: Colors::default(),
            hours: "24".into(),
            renderer: None,
            theme: serde_json::Value::Null,
            background: None,
        }
    }
}

impl Default for Colors {
    // Obsidian's.
    fn default() -> Self {
        Self {
            background: "#000000".into(),
            raised: "#2c2c2e".into(),
            foreground: "#ffffff".into(),
            muted: "#8e8e93".into(),
            accent: "#ff9f0a".into(),
            danger: "#ff453a".into(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
    }

    /// The font, or Mochi's Inter where mochid links the shell's fonts.
    pub fn font_bytes(&self) -> Option<Vec<u8>> {
        let runtime = std::env::var_os("XDG_RUNTIME_DIR")
            .map(|dir| PathBuf::from(dir).join("mochi/shell/fonts/InterVariable.ttf"));
        self.font
            .iter()
            .cloned()
            .chain(runtime)
            .find_map(|path| std::fs::read(path).ok())
    }
}

/// An `#rrggbb` or `#aarrggbb` color as (r, g, b, a), or `fallback`.
pub fn rgba(text: &str, fallback: [u8; 4]) -> [u8; 4] {
    let hex = text.trim().trim_start_matches('#');
    let byte = |at: usize| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok();
    let parsed = match hex.len() {
        6 => (|| Some([byte(0)?, byte(2)?, byte(4)?, 255]))(),
        8 => (|| Some([byte(2)?, byte(4)?, byte(6)?, byte(0)?]))(),
        _ => None,
    };
    parsed.unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_colors_and_falls_back() {
        assert_eq!(rgba("#ff9f0a", [0; 4]), [255, 159, 10, 255]);
        assert_eq!(rgba("#80112233", [0; 4]), [0x11, 0x22, 0x33, 0x80]);
        assert_eq!(rgba("orange", [1, 2, 3, 4]), [1, 2, 3, 4]);
        let config: Config = serde_json::from_str(r#"{"pam_service":"login"}"#).unwrap();
        assert_eq!(config.pam_service, "login");
        assert_eq!(config.colors.accent, "#ff9f0a");
    }
}
