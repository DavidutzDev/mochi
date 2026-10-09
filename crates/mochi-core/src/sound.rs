//! Plays a sound: a file, or a name from the sound theme, like
//! `message-new-instant`. The notifications module plays the sounds apps
//! ask for with it, and the timer its alarm.
//!
//! Files play through the first of `pw-play` and `paplay` installed. Names
//! play through `canberra-gtk-play`, which knows the desktop's sound theme,
//! and without it Mochi finds the file in the theme itself:
//! `sounds/<theme>/stereo/<name>.oga` in the XDG data directories, then in
//! the `freedesktop` theme. A module's own command, like the notifications'
//! `sound_command`, replaces all of these: it gets the file, names
//! included. A volume goes to the players Mochi knows, not to a command of
//! the user's own.
//!
//! A sound never holds the module up: the player runs on its own, gets
//! [`LONGEST`] at most, and a new sound stops the one still playing, so a
//! burst of messages doesn't play over itself.

use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::task::JoinHandle;

/// The players for files, in the order they're tried.
pub const PLAYERS: [&str; 2] = ["pw-play", "paplay"];
/// Plays sound theme names the way GTK apps do.
const CANBERRA: &str = "canberra-gtk-play";
/// The theme every other one falls back to.
const FALLBACK_THEME: &str = "freedesktop";
const EXTENSIONS: [&str; 3] = ["oga", "ogg", "wav"];
/// Long enough for any notification sound; a player still going after this
/// is stuck.
const LONGEST: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sound {
    File(PathBuf),
    Name(String),
}

impl Sound {
    /// From `sound-file`: a path, or a `file://` URL. Only absolute paths,
    /// so the player can't take one for an option.
    pub fn file(hint: &str) -> Option<Self> {
        let path = hint.strip_prefix("file://").unwrap_or(hint);
        path.starts_with('/')
            .then(|| Self::File(PathBuf::from(path)))
    }

    /// From `sound-name`. Names are words and dashes, so one can't climb
    /// out of the theme's folder or pass for an option.
    pub fn name(hint: &str) -> Option<Self> {
        let fine = hint
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || matches!(char, '-' | '_' | '.'));
        (fine && hint.starts_with(|char: char| char.is_ascii_alphanumeric()))
            .then(|| Self::Name(hint.to_owned()))
    }
}

/// The first player for files that's installed.
pub fn player() -> Option<&'static str> {
    PLAYERS
        .into_iter()
        .find(|program| crate::process::installed(program))
}

/// Plays sounds, one at a time.
#[derive(Debug)]
pub struct Speaker {
    /// A command of the user's own, like `sound_command`: the program and
    /// its options, the file goes last. Empty for the defaults.
    command: Vec<String>,
    theme: String,
    /// From 0 to 1, for the players Mochi knows. `None` keeps theirs.
    volume: Option<f64>,
    playing: Option<JoinHandle<()>>,
}

impl Speaker {
    pub fn new(command: Vec<String>, theme: String) -> Self {
        Self {
            command,
            theme,
            volume: None,
            playing: None,
        }
    }

    /// Plays at `volume`, from 0 to 1, rather than at the players' own.
    pub fn with_volume(mut self, volume: f64) -> Self {
        self.volume = Some(volume.clamp(0.0, 1.0));
        self
    }

    /// Starts playing `sound` and returns at once. Stops the sound before.
    /// False when nothing could play it: no player, or a name the sound
    /// theme doesn't have.
    pub fn play(&mut self, sound: &Sound) -> bool {
        let Some(argv) = command(
            sound,
            &self.command,
            &self.theme,
            self.volume,
            crate::process::installed,
            &data_dirs(),
        ) else {
            tracing::debug!(?sound, "nothing to play the sound with");
            return false;
        };
        let (program, args) = argv.split_first().expect("a command has a program");
        let child = tokio::process::Command::new(program)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn();
        let mut child = match child {
            Ok(child) => child,
            Err(error) => {
                tracing::warn!(%error, program, "can't play a sound");
                return false;
            }
        };
        // Stopping the task drops the child, which kills it.
        let task = tokio::spawn(async move {
            let _ = tokio::time::timeout(LONGEST, child.wait()).await;
        });
        if let Some(before) = self.playing.replace(task) {
            before.abort();
        }
        true
    }
}

impl Drop for Speaker {
    fn drop(&mut self) {
        if let Some(playing) = self.playing.take() {
            playing.abort();
        }
    }
}

/// What plays `sound`: `custom` with the file, or the defaults at `volume`.
/// `installed` says which programs are there, and `dirs` are the data
/// directories to look for sound themes in. `None` when nothing can play
/// it.
fn command(
    sound: &Sound,
    custom: &[String],
    theme: &str,
    volume: Option<f64>,
    installed: impl Fn(&str) -> bool,
    dirs: &[PathBuf],
) -> Option<Vec<String>> {
    let file = match sound {
        Sound::File(path) => path.clone(),
        Sound::Name(name) if custom.is_empty() && installed(CANBERRA) => {
            let mut argv = vec![CANBERRA.into(), "-i".into(), name.clone()];
            // In decibels, down from the full volume.
            if let Some(volume) = volume {
                let decibels = 20.0 * volume.max(0.001).log10();
                argv.extend(["-V".into(), format!("{decibels:.1}")]);
            }
            return Some(argv);
        }
        Sound::Name(name) => in_theme(name, theme, dirs)?,
    };
    let mut argv = if custom.is_empty() {
        let player = PLAYERS.into_iter().find(|program| installed(program))?;
        let mut argv = vec![player.to_owned()];
        match (player, volume) {
            ("pw-play", Some(volume)) => argv.push(format!("--volume={volume:.2}")),
            // paplay's full volume is 65536.
            (_, Some(volume)) => argv.push(format!("--volume={}", (volume * 65536.0).round())),
            (_, None) => {}
        }
        argv
    } else {
        custom.to_vec()
    };
    argv.push(file.display().to_string());
    Some(argv)
}

/// The file for a sound name, as the sound theme spec finds it: the whole
/// name, then shorter ones (`message-new-instant`, `message-new`,
/// `message`), each in `theme` and then in `freedesktop`.
fn in_theme(name: &str, theme: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    let mut themes = vec![theme];
    if theme != FALLBACK_THEME {
        themes.push(FALLBACK_THEME);
    }
    let mut name = name;
    loop {
        for theme in &themes {
            for dir in dirs {
                let folder = dir.join("sounds").join(theme).join("stereo");
                if let Some(file) = EXTENSIONS
                    .iter()
                    .map(|extension| folder.join(format!("{name}.{extension}")))
                    .find(|file| file.is_file())
                {
                    return Some(file);
                }
            }
        }
        name = &name[..name.rfind('-')?];
    }
}

/// The user's data directory, then the system ones.
fn data_dirs() -> Vec<PathBuf> {
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
        .collect()
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    fn only<'a>(programs: &'a [&'a str]) -> impl Fn(&str) -> bool + 'a {
        move |program| programs.contains(&program)
    }

    fn strings(argv: &[&str]) -> Vec<String> {
        argv.iter().map(|arg| (*arg).to_owned()).collect()
    }

    /// A data directory with a sound theme, removed when dropped.
    struct Themes(PathBuf);

    impl Themes {
        fn new(name: &str, files: &[&str]) -> Self {
            let dir =
                std::env::temp_dir().join(format!("mochi-sounds-{name}-{}", std::process::id()));
            for file in files {
                let path = dir.join("sounds").join(file);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, b"").unwrap();
            }
            Self(dir)
        }
    }

    impl Drop for Themes {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn reads_the_hints_safely() {
        assert_eq!(
            Sound::file("/usr/share/sounds/ding.oga"),
            Some(Sound::File("/usr/share/sounds/ding.oga".into()))
        );
        assert_eq!(
            Sound::file("file:///tmp/ding.wav"),
            Some(Sound::File("/tmp/ding.wav".into()))
        );
        assert_eq!(Sound::file("ding.wav"), None);
        assert_eq!(Sound::file("--volume=9"), None);
        assert_eq!(
            Sound::name("message-new-instant"),
            Some(Sound::Name("message-new-instant".into()))
        );
        assert_eq!(Sound::name("../../etc/passwd"), None);
        assert_eq!(Sound::name("-x"), None);
        assert_eq!(Sound::name(""), None);
    }

    #[test]
    fn files_play_through_the_first_player_installed() {
        let ding = Sound::File("/tmp/ding.oga".into());
        assert_eq!(
            command(
                &ding,
                &[],
                "freedesktop",
                None,
                only(&["paplay", "pw-play"]),
                &[]
            ),
            Some(strings(&["pw-play", "/tmp/ding.oga"]))
        );
        assert_eq!(
            command(&ding, &[], "freedesktop", None, only(&["paplay"]), &[]),
            Some(strings(&["paplay", "/tmp/ding.oga"]))
        );
        assert_eq!(
            command(&ding, &[], "freedesktop", None, only(&[]), &[]),
            None
        );
    }

    #[test]
    fn names_go_to_canberra_when_it_is_there() {
        let name = Sound::Name("message-new-instant".into());
        assert_eq!(
            command(
                &name,
                &[],
                "freedesktop",
                None,
                only(&["canberra-gtk-play", "pw-play"]),
                &[]
            ),
            Some(strings(&["canberra-gtk-play", "-i", "message-new-instant"]))
        );
    }

    #[test]
    fn a_volume_goes_to_each_player_its_own_way() {
        let ding = Sound::File("/tmp/ding.oga".into());
        let half = Some(0.5);
        assert_eq!(
            command(&ding, &[], "freedesktop", half, only(&["pw-play"]), &[]),
            Some(strings(&["pw-play", "--volume=0.50", "/tmp/ding.oga"]))
        );
        assert_eq!(
            command(&ding, &[], "freedesktop", half, only(&["paplay"]), &[]),
            Some(strings(&["paplay", "--volume=32768", "/tmp/ding.oga"]))
        );
        // Canberra takes decibels: half is about 6 below the full volume.
        let name = Sound::Name("alarm-clock-elapsed".into());
        assert_eq!(
            command(
                &name,
                &[],
                "freedesktop",
                half,
                only(&["canberra-gtk-play"]),
                &[]
            ),
            Some(strings(&[
                "canberra-gtk-play",
                "-i",
                "alarm-clock-elapsed",
                "-V",
                "-6.0"
            ]))
        );
    }

    #[test]
    fn names_are_found_in_the_theme_otherwise() {
        let themes = Themes::new(
            "lookup",
            &[
                "freedesktop/stereo/message.oga",
                "freedesktop/stereo/bell.oga",
                "ocean/stereo/bell.wav",
            ],
        );
        let dirs = [PathBuf::from("/nonexistent"), themes.0.clone()];
        let sounds = themes.0.join("sounds");
        let play = |name: &str, theme: &str| {
            command(
                &Sound::Name(name.into()),
                &[],
                theme,
                None,
                only(&["pw-play"]),
                &dirs,
            )
        };
        // The theme first, then freedesktop.
        assert_eq!(
            play("bell", "ocean"),
            Some(vec![
                "pw-play".into(),
                sounds.join("ocean/stereo/bell.wav").display().to_string()
            ])
        );
        // Shorter names when the whole one isn't there.
        assert_eq!(
            play("message-new-instant", "ocean"),
            Some(vec![
                "pw-play".into(),
                sounds
                    .join("freedesktop/stereo/message.oga")
                    .display()
                    .to_string()
            ])
        );
        assert_eq!(play("phone-incoming-call", "ocean"), None);
    }

    #[test]
    fn the_own_command_gets_the_file_names_too() {
        let themes = Themes::new("custom", &["freedesktop/stereo/bell.oga"]);
        let custom = strings(&["mpv", "--really-quiet"]);
        let everything = only(&["canberra-gtk-play", "pw-play"]);
        assert_eq!(
            command(
                &Sound::File("/tmp/ding.oga".into()),
                &custom,
                "freedesktop",
                Some(0.5),
                &everything,
                &[]
            ),
            Some(strings(&["mpv", "--really-quiet", "/tmp/ding.oga"]))
        );
        let bell = themes.0.join("sounds/freedesktop/stereo/bell.oga");
        assert_eq!(
            command(
                &Sound::Name("bell".into()),
                &custom,
                "freedesktop",
                Some(0.5),
                &everything,
                std::slice::from_ref(&themes.0)
            ),
            Some(vec![
                "mpv".into(),
                "--really-quiet".into(),
                bell.display().to_string()
            ])
        );
    }

    /// Plays through a script that writes its pid and arguments, then waits
    /// as a long sound would: `play` returns at once, and the next sound
    /// stops it.
    #[tokio::test]
    async fn plays_without_waiting_and_one_at_a_time() {
        let dir = std::env::temp_dir().join(format!("mochi-speaker-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("player");
        std::fs::write(
            &script,
            "#!/bin/sh\necho $$ \"$@\" > \"$(dirname \"$0\")/$(basename \"$2\")\"\nexec sleep 30\n",
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&script).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
        std::fs::set_permissions(&script, permissions).unwrap();

        let mut speaker = Speaker::new(
            vec![script.display().to_string(), "--quiet".into()],
            "freedesktop".into(),
        );
        let started = Instant::now();
        assert!(speaker.play(&Sound::File("/tmp/first.oga".into())));
        assert!(started.elapsed() < Duration::from_secs(1));
        let wait_for = |file: &str| {
            let path = dir.join(file);
            let started = Instant::now();
            while !path.exists() && started.elapsed() < Duration::from_secs(5) {
                std::thread::sleep(Duration::from_millis(20));
            }
            std::thread::sleep(Duration::from_millis(50));
            std::fs::read_to_string(path).unwrap()
        };
        let first = wait_for("first.oga");
        let (pid, args) = first.trim().split_once(' ').unwrap();
        assert_eq!(args, "--quiet /tmp/first.oga");

        assert!(speaker.play(&Sound::File("/tmp/second.oga".into())));
        let second = wait_for("second.oga");
        assert!(second.ends_with("--quiet /tmp/second.oga\n"), "{second}");
        // Let the aborted task drop the first child.
        tokio::task::yield_now().await;
        let stopped = |pid: &str| {
            std::fs::read_to_string(format!("/proc/{pid}/stat"))
                .map_or(true, |stat| stat.split(' ').nth(2) == Some("Z"))
        };
        let started = Instant::now();
        while !stopped(pid) && started.elapsed() < Duration::from_secs(5) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(stopped(pid), "the first sound still plays");

        let (second_pid, _) = second.trim().split_once(' ').unwrap();
        drop(speaker);
        tokio::task::yield_now().await;
        let started = Instant::now();
        while !stopped(second_pid) && started.elapsed() < Duration::from_secs(5) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(stopped(second_pid), "the sound outlived the module");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
