//! The icon of each app in the mixer. A stream names an icon of its own, or
//! only its program, and browsers name icons the theme lacks: Chromium says
//! `chromium-browser`, Zen says nothing and runs `zen`. The app's desktop
//! entry has the icon the launcher shows, so the mixer looks the app up
//! there, by what its stream says it is, and keeps the answer.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use libpulse_binding::proplist::{Proplist, properties};
use mochi_core::desktop::{self, App, Locale};

/// How long the desktop entries read stay. An app seen for the first time
/// after that reads them again, so one installed since gets its icon.
const FRESH: Duration = Duration::from_secs(60);

/// What a stream says about its app, as the PulseAudio properties name it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Identity {
    /// `pipewire.access.portal.app_id`: a Flatpak's id, like
    /// `org.chromium.Chromium`.
    pub portal_id: Option<String>,
    /// `application.id`.
    pub app_id: Option<String>,
    /// `application.icon_name`.
    pub icon: Option<String>,
    /// `application.process.binary`.
    pub binary: Option<String>,
    /// `application.name`.
    pub name: Option<String>,
}

impl Identity {
    pub fn of(proplist: &Proplist) -> Self {
        let property = |key: &str| {
            proplist
                .get_str(key)
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
        };
        Self {
            portal_id: property("pipewire.access.portal.app_id"),
            app_id: property(properties::APPLICATION_ID),
            icon: property(properties::APPLICATION_ICON_NAME),
            binary: property(properties::APPLICATION_PROCESS_BINARY),
            name: property(properties::APPLICATION_NAME),
        }
    }
}

/// The icons found so far, by what the streams said.
pub struct Icons {
    read: fn() -> Vec<App>,
    apps: Vec<App>,
    read_at: Option<Instant>,
    found: HashMap<Identity, Option<String>>,
}

impl std::fmt::Debug for Icons {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Icons")
            .field("apps", &self.apps.len())
            .field("found", &self.found)
            .finish_non_exhaustive()
    }
}

impl Default for Icons {
    fn default() -> Self {
        Self::reading(installed)
    }
}

impl Icons {
    /// Icons from the apps `read` lists, read the first time an app needs
    /// one.
    fn reading(read: fn() -> Vec<App>) -> Self {
        Self {
            read,
            apps: Vec::new(),
            read_at: None,
            found: HashMap::new(),
        }
    }

    /// The icon of a stream's app: its desktop entry's, or else the icon the
    /// stream names, or its program's name.
    pub fn icon(&mut self, identity: &Identity) -> Option<String> {
        if let Some(icon) = self.found.get(identity) {
            return icon.clone();
        }
        if self.read_at.is_none_or(|read_at| read_at.elapsed() > FRESH) {
            self.apps = (self.read)();
            self.read_at = Some(Instant::now());
        }
        let names: Vec<&str> = [
            &identity.portal_id,
            &identity.app_id,
            &identity.icon,
            &identity.binary,
            &identity.name,
        ]
        .into_iter()
        .flatten()
        .map(String::as_str)
        .collect();
        let icon = desktop::find(&self.apps, &names)
            .and_then(|app| app.icon.clone())
            .or_else(|| identity.icon.clone())
            .or_else(|| identity.binary.as_ref().map(|binary| binary.to_lowercase()));
        self.found.insert(identity.clone(), icon.clone());
        icon
    }
}

/// The apps the launcher would list.
fn installed() -> Vec<App> {
    desktop::scan(
        &desktop::directories(),
        &Locale::from_env(),
        &desktop::current_desktops(),
    )
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::path::PathBuf;

    use super::*;

    fn app(id: &str, name: &str, exec: &str, icon: &str) -> App {
        App {
            id: id.into(),
            name: name.into(),
            generic: None,
            comment: None,
            icon: Some(icon.into()),
            exec: exec.into(),
            terminal: false,
            keywords: Vec::new(),
            path: None,
            wm_class: None,
            actions: Vec::new(),
            file: PathBuf::new(),
        }
    }

    thread_local! {
        static READS: Cell<u32> = const { Cell::new(0) };
    }

    fn apps() -> Vec<App> {
        READS.with(|reads| reads.set(reads.get() + 1));
        vec![
            app(
                "zen-beta.desktop",
                "Zen Browser (Beta)",
                "zen-beta %U",
                "zen-browser",
            ),
            app(
                "chromium-browser.desktop",
                "Chromium",
                "chromium %U",
                "chromium",
            ),
            app("discord.desktop", "Discord", "Discord", "discord"),
        ]
    }

    fn identity(binary: &str, name: &str, icon: Option<&str>) -> Identity {
        Identity {
            binary: Some(binary.into()),
            name: Some(name.into()),
            icon: icon.map(str::to_owned),
            ..Identity::default()
        }
    }

    #[test]
    fn finds_icons_the_streams_dont_name() {
        let mut icons = Icons::reading(apps);
        // As Zen and Chromium report themselves.
        assert_eq!(
            icons.icon(&identity("zen", "Zen", None)).as_deref(),
            Some("zen-browser")
        );
        assert_eq!(
            icons
                .icon(&identity("chromium", "Chromium", Some("chromium-browser")))
                .as_deref(),
            Some("chromium")
        );
        // A WebRTC stream in Discord.
        assert_eq!(
            icons
                .icon(&identity("Discord", "WEBRTC VoiceEngine", None))
                .as_deref(),
            Some("discord")
        );
        // Without an entry, what the stream says, or its program.
        assert_eq!(
            icons
                .icon(&identity("mpv", "mpv Media Player", Some("mpv")))
                .as_deref(),
            Some("mpv")
        );
        assert_eq!(
            icons.icon(&identity("Totem", "Videos", None)).as_deref(),
            Some("totem")
        );
        assert_eq!(icons.icon(&Identity::default()), None);
        // The entries were read once, and each answer is kept.
        assert_eq!(
            icons.icon(&identity("zen", "Zen", None)).as_deref(),
            Some("zen-browser")
        );
        assert_eq!(READS.with(Cell::get), 1);
    }
}
