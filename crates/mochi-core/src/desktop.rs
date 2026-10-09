//! Desktop entries: the `.desktop` files that list installed apps.
//!
//! They live in `applications/` under `$XDG_DATA_HOME` and every
//! `$XDG_DATA_DIRS` entry. The first file with a given id wins, so a user's
//! own copy overrides the system one, and a hidden copy hides it.
//!
//! The launcher lists them, and [`find`] tells which app a process or a
//! name belongs to, for its icon.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct App {
    /// The desktop file id, like `firefox.desktop`.
    pub id: String,
    pub name: String,
    /// "Web Browser".
    pub generic: Option<String>,
    pub comment: Option<String>,
    pub icon: Option<String>,
    pub exec: String,
    pub terminal: bool,
    pub keywords: Vec<String>,
    /// The directory to run it in.
    pub path: Option<PathBuf>,
    /// `StartupWMClass`: the class or app id its windows have, which is
    /// often its program's name.
    pub wm_class: Option<String>,
    pub actions: Vec<AppAction>,
    /// Where it came from, for `%k`.
    pub file: PathBuf,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppAction {
    /// Its key in the file, like `new-private-window`.
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub exec: String,
}

/// Which translations to prefer, from `LC_ALL`, `LC_MESSAGES` or `LANG`.
#[derive(Debug, Clone, Default)]
pub struct Locale {
    /// Most specific first: `fr_FR@euro`, `fr_FR`, `fr@euro`, `fr`.
    candidates: Vec<String>,
}

impl Locale {
    pub fn from_env() -> Self {
        let value = ["LC_ALL", "LC_MESSAGES", "LANG"]
            .into_iter()
            .filter_map(|name| std::env::var(name).ok())
            .find(|value| !value.is_empty())
            .unwrap_or_default();
        Self::parse(&value)
    }

    /// `lang_COUNTRY.ENCODING@MODIFIER`, every part but `lang` optional.
    pub fn parse(value: &str) -> Self {
        let (rest, modifier) = match value.split_once('@') {
            Some((rest, modifier)) => (rest, Some(modifier)),
            None => (value, None),
        };
        let rest = rest.split('.').next().unwrap_or_default();
        let (lang, country) = match rest.split_once('_') {
            Some((lang, country)) => (lang, Some(country)),
            None => (rest, None),
        };
        if lang.is_empty() || lang == "C" || lang == "POSIX" {
            return Self::default();
        }
        let mut candidates = Vec::new();
        if let (Some(country), Some(modifier)) = (country, modifier) {
            candidates.push(format!("{lang}_{country}@{modifier}"));
        }
        if let Some(country) = country {
            candidates.push(format!("{lang}_{country}"));
        }
        if let Some(modifier) = modifier {
            candidates.push(format!("{lang}@{modifier}"));
        }
        candidates.push(lang.to_owned());
        Self { candidates }
    }
}

/// The desktops `OnlyShowIn` and `NotShowIn` name, from
/// `XDG_CURRENT_DESKTOP`.
pub fn current_desktops() -> Vec<String> {
    std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .filter(|desktop| !desktop.is_empty())
        .map(str::to_owned)
        .collect()
}

/// `applications/` in the user's data directory, then in the system ones.
pub fn directories() -> Vec<PathBuf> {
    let home = std::env::var_os("XDG_DATA_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".local/share")));
    let system = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    home.into_iter()
        .chain(
            system
                .split(':')
                .filter(|dir| !dir.is_empty())
                .map(PathBuf::from),
        )
        .map(|dir| dir.join("applications"))
        .collect()
}

/// Every app to show, from `directories` in order.
pub fn scan(directories: &[PathBuf], locale: &Locale, desktops: &[String]) -> Vec<App> {
    let mut seen = HashSet::new();
    let mut apps = Vec::new();
    for directory in directories {
        let mut files = Vec::new();
        collect(directory, directory, &mut files);
        files.sort();
        for (id, file) in files {
            // The first file with an id decides, even one that hides the app.
            if !seen.insert(id.clone()) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            if let Some(app) = parse(&id, &file, &text, locale, desktops) {
                apps.push(app);
            }
        }
    }
    apps
}

/// Desktop files under `directory`, with their ids: the path below `root`
/// with `/` turned into `-`.
fn collect(root: &Path, directory: &Path, files: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, files);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "desktop")
            && let Ok(relative) = path.strip_prefix(root)
        {
            let id = relative.to_string_lossy().replace('/', "-");
            files.push((id, path));
        }
    }
}

/// One file's `[Desktop Entry]` and `[Desktop Action …]` groups. `None` for
/// anything that isn't an app to show here.
pub fn parse(
    id: &str,
    file: &Path,
    text: &str,
    locale: &Locale,
    desktops: &[String],
) -> Option<App> {
    let groups = groups(text);
    let entry = groups.iter().find(|(name, _)| name == "Desktop Entry")?;
    let get = |key: &str| value(&entry.1, key, locale);
    let flag = |key: &str| get(key).is_some_and(|value| value == "true");

    if get("Type").as_deref() != Some("Application") || flag("NoDisplay") || flag("Hidden") {
        return None;
    }
    let names = |key: &str| get(key).map(|value| list(&value)).unwrap_or_default();
    let only = names("OnlyShowIn");
    if !only.is_empty() && !only.iter().any(|desktop| desktops.contains(desktop)) {
        return None;
    }
    if names("NotShowIn")
        .iter()
        .any(|desktop| desktops.contains(desktop))
    {
        return None;
    }
    if let Some(program) = get("TryExec")
        && !installed(&program)
    {
        return None;
    }

    let actions = names("Actions")
        .into_iter()
        .filter_map(|action| {
            let (_, keys) = groups
                .iter()
                .find(|(name, _)| *name == format!("Desktop Action {action}"))?;
            Some(AppAction {
                name: value(keys, "Name", locale)?,
                icon: value(keys, "Icon", locale),
                exec: value(keys, "Exec", locale)?,
                id: action,
            })
        })
        .collect();

    Some(App {
        id: id.to_owned(),
        name: get("Name")?,
        generic: get("GenericName"),
        comment: get("Comment"),
        icon: get("Icon"),
        exec: get("Exec")?,
        terminal: flag("Terminal"),
        keywords: names("Keywords"),
        path: get("Path").map(PathBuf::from),
        wm_class: get("StartupWMClass"),
        actions,
        file: file.to_owned(),
    })
}

type Group = (String, Vec<(String, String)>);

fn groups(text: &str) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            groups.push((name.to_owned(), Vec::new()));
        } else if let Some((key, value)) = line.split_once('=')
            && let Some((_, keys)) = groups.last_mut()
        {
            keys.push((key.trim().to_owned(), value.trim().to_owned()));
        }
    }
    groups
}

/// A key's value, translated when the locale has a translation, with the
/// general escapes undone.
fn value(keys: &[(String, String)], key: &str, locale: &Locale) -> Option<String> {
    let find = |wanted: &str| {
        keys.iter()
            .find(|(name, _)| name == wanted)
            .map(|(_, value)| value)
    };
    locale
        .candidates
        .iter()
        .find_map(|candidate| find(&format!("{key}[{candidate}]")))
        .or_else(|| find(key))
        .map(|value| unescape(value))
}

/// `\s`, `\n`, `\t`, `\r` and `\\`. Other escapes, like `\;` in lists, stay
/// for the next step.
fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// A `;`-separated list, where `\;` is a literal semicolon.
fn list(value: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut item = String::new();
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&';') => {
                item.push(';');
                chars.next();
            }
            ';' => items.push(std::mem::take(&mut item)),
            other => item.push(other),
        }
    }
    items.push(item);
    items.retain(|item| !item.is_empty());
    items
}

/// Whether a program exists: a path, or a name found in `PATH`.
pub fn installed(program: &str) -> bool {
    if program.contains('/') {
        return Path::new(program).is_file();
    }
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(program).is_file()))
}

/// Splits `Exec=` on spaces outside double quotes. Inside quotes, a
/// backslash makes the next `"`, `` ` ``, `$` or `\` literal. `None` for an
/// unclosed quote.
pub fn exec_words(exec: &str) -> Option<Vec<String>> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut quoted = false;
    let mut chars = exec.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            '\\' if quoted => word.push(chars.next()?),
            ' ' | '\t' if !quoted => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            other => {
                word.push(other);
                started = true;
            }
        }
    }
    if quoted {
        return None;
    }
    if started {
        words.push(word);
    }
    Some(words)
}

/// The program an entry runs, without its directory: `firefox` for
/// `Exec=/usr/bin/firefox %u`, and past `env` and its variables.
fn program(exec: &str) -> Option<String> {
    let words = exec_words(exec)?;
    let mut words = words.iter().map(String::as_str);
    let base = |word: &str| {
        Path::new(word)
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned)
    };
    let first = words.next()?;
    if base(first).as_deref() != Some("env") {
        return base(first);
    }
    words
        .find(|word| !word.contains('=') && !word.starts_with('-'))
        .and_then(base)
}

/// The app that one of `names` belongs to, the first name first: what a
/// process says it is, like a sound stream's app id, program and app name.
/// Each name is tried, ignoring case, as an entry's id, the last part of a
/// reverse-DNS id, its `StartupWMClass`, its program and its name. When none
/// of them matches, each is tried as their first word, so `zen` finds
/// `zen-beta.desktop` and `chromium` finds `chromium-browser.desktop`.
pub fn find<'a>(apps: &'a [App], names: &[&str]) -> Option<&'a App> {
    let names: Vec<String> = names
        .iter()
        .map(|name| name.trim().to_lowercase())
        .filter(|name| !name.is_empty())
        .collect();
    let keys: Vec<(&App, Keys)> = apps.iter().map(|app| (app, Keys::of(app))).collect();
    for exact in [true, false] {
        for name in &names {
            if let Some((app, _)) = keys.iter().find(|(_, keys)| keys.matches(name, exact)) {
                return Some(app);
            }
        }
    }
    None
}

/// What [`find`] compares names with, in lowercase.
struct Keys {
    /// The id without `.desktop`.
    id: String,
    wm_class: Option<String>,
    program: Option<String>,
    name: String,
}

impl Keys {
    fn of(app: &App) -> Self {
        Self {
            id: app
                .id
                .strip_suffix(".desktop")
                .unwrap_or(&app.id)
                .to_lowercase(),
            wm_class: app.wm_class.as_deref().map(str::to_lowercase),
            program: program(&app.exec).map(|program| program.to_lowercase()),
            name: app.name.to_lowercase(),
        }
    }

    fn matches(&self, name: &str, exact: bool) -> bool {
        let keys = [
            Some(self.id.as_str()),
            self.wm_class.as_deref(),
            self.program.as_deref(),
            Some(self.name.as_str()),
        ];
        let mut keys = keys.into_iter().flatten();
        if exact {
            let last = self.id.rsplit('.').next().filter(|_| self.id.contains('.'));
            last == Some(name) || keys.any(|key| key == name)
        } else {
            keys.any(|key| {
                key.strip_prefix(name)
                    .is_some_and(|rest| rest.starts_with(['-', '_', '.', ' ']))
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIREFOX: &str = "\
# A comment
[Desktop Entry]
Type=Application
Name=Firefox
Name[fr]=Firefox (fr)
GenericName=Web Browser
GenericName[fr_FR]=Navigateur web
Icon=firefox
Exec=firefox --name firefox %U
Keywords=web;internet\\;stuff;
Actions=new-private-window;missing;

[Desktop Action new-private-window]
Name=New Private Window
Exec=firefox --private-window %U
";

    fn firefox(locale: &str) -> Option<App> {
        parse(
            "firefox.desktop",
            Path::new("/x/firefox.desktop"),
            FIREFOX,
            &Locale::parse(locale),
            &["Hyprland".into()],
        )
    }

    #[test]
    fn reads_an_entry_with_its_actions() {
        let app = firefox("en_US.UTF-8").unwrap();
        assert_eq!(app.name, "Firefox");
        assert_eq!(app.generic.as_deref(), Some("Web Browser"));
        assert_eq!(app.keywords, ["web", "internet;stuff"]);
        assert_eq!(app.exec, "firefox --name firefox %U");
        // An action listed without its group is skipped.
        assert_eq!(app.actions.len(), 1);
        assert_eq!(app.actions[0].id, "new-private-window");
        assert_eq!(app.actions[0].name, "New Private Window");
    }

    #[test]
    fn prefers_the_most_specific_translation() {
        let app = firefox("fr_FR.UTF-8").unwrap();
        assert_eq!(app.name, "Firefox (fr)");
        assert_eq!(app.generic.as_deref(), Some("Navigateur web"));
        assert_eq!(
            Locale::parse("fr_FR.UTF-8@euro").candidates,
            ["fr_FR@euro", "fr_FR", "fr@euro", "fr"]
        );
        assert!(Locale::parse("C.UTF-8").candidates.is_empty());
    }

    #[test]
    fn hides_what_should_not_show_here() {
        let entry = |extra: &str| {
            let text = format!("[Desktop Entry]\nType=Application\nName=A\nExec=a\n{extra}");
            parse(
                "a.desktop",
                Path::new("/a"),
                &text,
                &Locale::default(),
                &["Hyprland".into()],
            )
        };
        assert!(entry("").is_some());
        assert!(entry("NoDisplay=true").is_none());
        assert!(entry("Hidden=true").is_none());
        assert!(entry("OnlyShowIn=GNOME;KDE;").is_none());
        assert!(entry("OnlyShowIn=Hyprland;").is_some());
        assert!(entry("NotShowIn=Hyprland;").is_none());
        assert!(entry("TryExec=/nonexistent/program").is_none());
        let link = parse(
            "l.desktop",
            Path::new("/l"),
            "[Desktop Entry]\nType=Link\nName=L\nURL=https://example.com",
            &Locale::default(),
            &[],
        );
        assert!(link.is_none());
    }

    #[test]
    fn the_first_directory_wins_even_when_it_hides() {
        let root = std::env::temp_dir().join(format!("mochi-entries-{}", std::process::id()));
        let user = root.join("user");
        let system = root.join("system");
        std::fs::create_dir_all(user.join("sub")).unwrap();
        std::fs::create_dir_all(&system).unwrap();
        let app = |name: &str| format!("[Desktop Entry]\nType=Application\nName={name}\nExec=x\n");
        std::fs::write(user.join("a.desktop"), app("User A")).unwrap();
        std::fs::write(system.join("a.desktop"), app("System A")).unwrap();
        std::fs::write(user.join("b.desktop"), "[Desktop Entry]\nHidden=true\n").unwrap();
        std::fs::write(system.join("b.desktop"), app("System B")).unwrap();
        std::fs::write(user.join("sub/c.desktop"), app("C")).unwrap();

        let apps = scan(&[user, system], &Locale::default(), &[]);
        let found: Vec<(&str, &str)> = apps
            .iter()
            .map(|app| (app.id.as_str(), app.name.as_str()))
            .collect();
        assert_eq!(found, [("a.desktop", "User A"), ("sub-c.desktop", "C")]);
        std::fs::remove_dir_all(root).unwrap();
    }

    fn entry(id: &str, name: &str, exec: &str, wm_class: Option<&str>) -> App {
        let text = format!("[Desktop Entry]\nType=Application\nName={name}\nExec={exec}\n");
        App {
            wm_class: wm_class.map(str::to_owned),
            ..parse(id, Path::new("/a"), &text, &Locale::default(), &[]).unwrap()
        }
    }

    #[test]
    fn finds_the_app_a_process_names() {
        let apps = [
            entry(
                "zen-beta.desktop",
                "Zen Browser (Beta)",
                "zen-beta --name zen-beta %U",
                Some("zen-beta"),
            ),
            entry("chromium-browser.desktop", "Chromium", "chromium %U", None),
            entry("discord.desktop", "Discord", "Discord", Some("discord")),
            entry("com.obsproject.Studio.desktop", "OBS Studio", "obs", None),
            entry(
                "com.spotify.Client.desktop",
                "Spotify",
                "env LD_PRELOAD=x /opt/spotify/spotify %U",
                None,
            ),
        ];
        let found = |names: &[&str]| find(&apps, names).map(|app| app.id.as_str());
        // Zen's streams say `zen` and "Zen": the first word of its id.
        assert_eq!(found(&["zen", "Zen"]), Some("zen-beta.desktop"));
        // Chromium names an icon after its id; its program is `chromium`.
        assert_eq!(
            found(&["chromium-browser", "chromium"]),
            Some("chromium-browser.desktop")
        );
        assert_eq!(found(&["chromium"]), Some("chromium-browser.desktop"));
        // A WebRTC stream's name says nothing; its program does.
        assert_eq!(
            found(&["", "Discord", "WEBRTC VoiceEngine"]),
            Some("discord.desktop")
        );
        assert_eq!(found(&["obs"]), Some("com.obsproject.Studio.desktop"));
        assert_eq!(found(&["studio"]), Some("com.obsproject.Studio.desktop"));
        // Past `env`, and without the directory.
        assert_eq!(found(&["spotify"]), Some("com.spotify.Client.desktop"));
        assert_eq!(found(&["firefox", "Firefox"]), None);
        // A name that matches exactly beats one that starts another.
        assert_eq!(found(&["zen", "discord"]), Some("discord.desktop"));
    }

    #[test]
    fn reads_the_program_an_entry_runs() {
        assert_eq!(program("/usr/bin/firefox %u").as_deref(), Some("firefox"));
        assert_eq!(program("env A=1 B=2 foot").as_deref(), Some("foot"));
        assert_eq!(program(r#""/opt/My App/run" --x"#).as_deref(), Some("run"));
        assert_eq!(program(""), None);
    }
}
