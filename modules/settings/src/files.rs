//! A file for an option whose source is `file`, like the timer's
//! `sound_file`, from the desktop's own file chooser: the XDG desktop
//! portal's `FileChooser` over the session bus, which opens the dialog of
//! GNOME, KDE or whichever backend the desktop has. Without a portal, or
//! one without a file chooser, `zenity` or `kdialog` opens one. Without
//! any of them, the answer says so, and the path can still be typed.
//!
//! The portal answers in two steps: `OpenFile` returns a request's object
//! path at once, and the request sends `Response` when the dialog closes,
//! with a code and the files as `file://` URIs. The request's path is
//! known before the call, from the bus name and a token of our own, so the
//! answer can't come before Mochi listens for it.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use futures_util::StreamExt;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
use zbus::{Connection, MatchRule, MessageStream};

const PORTAL: &str = "org.freedesktop.portal.Desktop";
const DESKTOP: &str = "/org/freedesktop/portal/desktop";
const CHOOSER: &str = "org.freedesktop.portal.FileChooser";
const REQUEST: &str = "org.freedesktop.portal.Request";
/// The dialogs tried without a portal, in order.
pub const DIALOGS: [&str; 2] = ["zenity", "kdialog"];
/// What the answer says when nothing can open a file chooser.
pub const NOTHING: &str = "no file chooser: install a desktop portal backend like \
                           xdg-desktop-portal-gtk, or zenity or kdialog; or type the path";

/// The files a chooser shows: the portal goes by the MIME type, the
/// dialogs by the names' endings.
#[derive(Debug, PartialEq, Eq)]
pub struct Filter {
    pub name: &'static str,
    pub mime: &'static str,
    pub globs: &'static [&'static str],
}

const FILTERS: [(&str, Filter); 2] = [
    (
        "audio",
        Filter {
            name: "Sound files",
            mime: "audio/*",
            globs: &[
                "*.oga", "*.ogg", "*.opus", "*.wav", "*.flac", "*.mp3", "*.m4a",
            ],
        },
    ),
    (
        "image",
        Filter {
            name: "Images",
            mime: "image/*",
            globs: &["*.png", "*.jpg", "*.jpeg", "*.webp", "*.gif", "*.bmp"],
        },
    ),
];

/// The filter an option's `x-filter` names.
pub fn filter(name: &str) -> Option<&'static Filter> {
    FILTERS
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, filter)| filter)
}

/// What to ask for.
#[derive(Debug, Default)]
pub struct Request {
    /// The dialog's title, like "Choose the sound file".
    pub title: String,
    pub filter: Option<&'static Filter>,
    /// Where the dialog opens, like the folder of the file chosen before.
    pub folder: Option<PathBuf>,
}

/// Asks for a file: through the portal on `bus`, or else the first of
/// [`DIALOGS`] in `path`, a `PATH`-like list of folders. `None` when the
/// dialog was closed without one.
pub async fn choose(
    request: &Request,
    bus: Option<&Connection>,
    path: Option<&OsStr>,
) -> Result<Option<PathBuf>, String> {
    if let Some(bus) = bus {
        match portal(bus, request).await {
            Ok(answer) => return answer,
            // No portal, or one without a file chooser.
            Err(error) => tracing::info!(%error, "no file chooser portal, trying a dialog"),
        }
    }
    dialog(request, path).await
}

/// The folder a chooser opens in for an option set to `value`: the one the
/// file is in, when it's there.
pub fn folder_of(value: &str) -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let path = match (value.strip_prefix("~/"), home) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => PathBuf::from(value),
    };
    let folder = path.parent()?;
    (path.is_absolute() && folder.is_dir()).then(|| folder.to_owned())
}

/// Asks the portal. The outer error means there's no portal to ask, the
/// inner one that it answered with a problem.
async fn portal(
    bus: &Connection,
    request: &Request,
) -> Result<Result<Option<PathBuf>, String>, zbus::Error> {
    static TOKENS: AtomicU32 = AtomicU32::new(0);
    let token = format!(
        "mochi{}_{}",
        std::process::id(),
        TOKENS.fetch_add(1, Ordering::Relaxed)
    );
    let sender = bus
        .unique_name()
        .ok_or_else(|| zbus::Error::Failure("no name on the bus".into()))?;
    let handle = request_path(sender.as_str(), &token);
    let mut responses = listen(bus, &handle).await?;

    let mut options: HashMap<&str, Value<'_>> = HashMap::new();
    options.insert("handle_token", Value::from(token.as_str()));
    options.insert("modal", Value::from(true));
    if let Some(filter) = request.filter {
        let only = (filter.name, vec![(1u32, filter.mime)]);
        let all = ("All files", vec![(0u32, "*")]);
        options.insert("filters", Value::from(vec![only.clone(), all]));
        options.insert("current_filter", Value::from(only));
    }
    if let Some(folder) = &request.folder {
        // A byte string with its NUL, as the portal takes paths.
        let mut bytes = folder.as_os_str().as_encoded_bytes().to_vec();
        bytes.push(0);
        options.insert("current_folder", Value::from(bytes));
    }
    let reply = bus
        .call_method(
            Some(PORTAL),
            DESKTOP,
            Some(CHOOSER),
            "OpenFile",
            &("", request.title.as_str(), options),
        )
        .await?;
    let given: OwnedObjectPath = reply.body().deserialize()?;
    // Portals before version 0.9 pick the path themselves.
    if given.as_str() != handle {
        responses = listen(bus, given.as_str()).await?;
    }
    let Some(message) = responses.next().await else {
        return Ok(Err("the file chooser went away".to_owned()));
    };
    let (code, results): (u32, HashMap<String, OwnedValue>) = message?.body().deserialize()?;
    Ok(answer(code, &results))
}

/// The object path the portal gives a request: the caller's unique name
/// without its colon, dots as underscores, then the token.
fn request_path(sender: &str, token: &str) -> String {
    let sender = sender.trim_start_matches(':').replace('.', "_");
    format!("{DESKTOP}/request/{sender}/{token}")
}

async fn listen(bus: &Connection, path: &str) -> Result<MessageStream, zbus::Error> {
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface(REQUEST)?
        .member("Response")?
        .path(path)?
        .build();
    MessageStream::for_match_rule(rule, bus, None).await
}

/// A request's `Response`: 0 with the files' URIs, 1 when the dialog was
/// closed, 2 when it ended some other way.
fn answer(code: u32, results: &HashMap<String, OwnedValue>) -> Result<Option<PathBuf>, String> {
    match code {
        0 => {}
        1 => return Ok(None),
        _ => return Err("the file chooser closed without a file".to_owned()),
    }
    let uris: Vec<String> = results
        .get("uris")
        .and_then(|uris| Vec::<String>::try_from(uris.try_clone().ok()?).ok())
        .unwrap_or_default();
    let Some(uri) = uris.first() else {
        return Ok(None);
    };
    path_of(uri)
        .map(Some)
        .ok_or_else(|| format!("the file chooser gave {uri}, which isn't a file on this computer"))
}

/// The path a `file://` URI names, its `%20`s and the like decoded.
fn path_of(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    // An empty host, or this one by name.
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    if !rest.starts_with('/') {
        return None;
    }
    let mut bytes = Vec::with_capacity(rest.len());
    let mut chars = rest.bytes();
    while let Some(byte) = chars.next() {
        if byte == b'%' {
            let high = chars.next()?;
            let low = chars.next()?;
            let hex = [high, low];
            let text = std::str::from_utf8(&hex).ok()?;
            bytes.push(u8::from_str_radix(text, 16).ok()?);
        } else {
            bytes.push(byte);
        }
    }
    use std::os::unix::ffi::OsStringExt;
    Some(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
}

/// The first of [`DIALOGS`] in `path`, with where it is.
fn find_dialog(path: Option<&OsStr>) -> Option<(&'static str, PathBuf)> {
    let dirs: Vec<PathBuf> = path.map(|path| std::env::split_paths(path).collect())?;
    DIALOGS.into_iter().find_map(|program| {
        dirs.iter()
            .map(|dir| dir.join(program))
            .find(|file| file.is_file())
            .map(|file| (program, file))
    })
}

/// The arguments that make `program` ask for one file.
fn dialog_args(program: &str, request: &Request) -> Vec<String> {
    let folder = request.folder.as_deref().map(Path::display);
    match program {
        "zenity" => {
            let mut args = vec![
                "--file-selection".to_owned(),
                format!("--title={}", request.title),
            ];
            if let Some(folder) = folder {
                // With a slash, it opens in the folder rather than at a
                // file of that name.
                args.push(format!("--filename={folder}/"));
            }
            if let Some(filter) = request.filter {
                args.push(format!(
                    "--file-filter={} | {}",
                    filter.name,
                    filter.globs.join(" ")
                ));
                args.push("--file-filter=All files | *".to_owned());
            }
            args
        }
        _ => {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_owned());
            let mut args = vec![
                format!("--title={}", request.title),
                "--getopenfilename".to_owned(),
                folder.map_or(home, |folder| folder.to_string()),
            ];
            if let Some(filter) = request.filter {
                args.push(format!("{}|{}", filter.globs.join(" "), filter.name));
            }
            args
        }
    }
}

/// Asks with a dialog program. Both print the path and exit with 0, or exit
/// with 1 when closed.
async fn dialog(request: &Request, path: Option<&OsStr>) -> Result<Option<PathBuf>, String> {
    let (program, file) = find_dialog(path).ok_or_else(|| NOTHING.to_owned())?;
    let output = tokio::process::Command::new(file)
        .args(dialog_args(program, request))
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|error| format!("can't start {program}: {error}"))?;
    match output.status.code() {
        Some(0) => {
            let text = String::from_utf8_lossy(&output.stdout);
            let chosen = text.lines().next().unwrap_or_default().trim_end();
            if chosen.is_empty() {
                return Ok(None);
            }
            let chosen = PathBuf::from(chosen);
            if !chosen.is_absolute() {
                return Err(format!(
                    "{program} gave {}, not a whole path",
                    chosen.display()
                ));
            }
            Ok(Some(chosen))
        }
        Some(1) => Ok(None),
        _ => Err(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use super::*;

    #[test]
    fn reads_the_portals_answer() {
        let uris = |list: &[&str]| {
            let list: Vec<String> = list.iter().map(|uri| (*uri).to_owned()).collect();
            HashMap::from([(
                "uris".to_owned(),
                OwnedValue::try_from(Value::from(list)).unwrap(),
            )])
        };
        assert_eq!(
            answer(0, &uris(&["file:///home/ana/Sounds/Bell%20one.oga"])),
            Ok(Some(PathBuf::from("/home/ana/Sounds/Bell one.oga")))
        );
        // Closed: nothing, and no complaint.
        assert_eq!(answer(1, &HashMap::new()), Ok(None));
        assert!(answer(2, &HashMap::new()).is_err());
        assert_eq!(answer(0, &uris(&[])), Ok(None));
        assert!(answer(0, &uris(&["https://example.com/bell.oga"])).is_err());
    }

    #[test]
    fn file_uris_become_paths() {
        assert_eq!(
            path_of("file:///tmp/a%C3%A9%25.wav"),
            Some(PathBuf::from("/tmp/aé%.wav"))
        );
        assert_eq!(
            path_of("file://localhost/tmp/x.oga"),
            Some(PathBuf::from("/tmp/x.oga"))
        );
        assert_eq!(path_of("file://server/tmp/x.oga"), None);
        assert_eq!(path_of("file:///tmp/broken%2"), None);
        assert_eq!(path_of("/tmp/x.oga"), None);
    }

    #[test]
    fn request_paths_follow_the_bus_name() {
        assert_eq!(
            request_path(":1.42", "mochi7_0"),
            "/org/freedesktop/portal/desktop/request/1_42/mochi7_0"
        );
    }

    #[test]
    fn knows_its_filters() {
        assert_eq!(filter("audio").unwrap().mime, "audio/*");
        assert!(filter("image").is_some());
        assert!(filter("video").is_none());
    }

    /// A folder with programs that pretend to be the dialogs.
    struct Fakes(PathBuf);

    impl Fakes {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("mochi-files-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        /// A program that writes its arguments next to itself, prints
        /// `out` and exits with `code`.
        fn add(&self, program: &str, out: &str, code: i32) {
            use std::os::unix::fs::PermissionsExt;
            let file = self.0.join(program);
            let script = format!(
                "#!/bin/sh\nfor arg in \"$@\"; do echo \"$arg\"; done > {}\nprintf '%s\\n' '{out}'\nexit {code}\n",
                self.0.join(format!("{program}.args")).display()
            );
            std::fs::write(&file, script).unwrap();
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        fn args(&self, program: &str) -> Vec<String> {
            std::fs::read_to_string(self.0.join(format!("{program}.args")))
                .unwrap_or_default()
                .lines()
                .map(str::to_owned)
                .collect()
        }

        fn path(&self) -> &OsStr {
            self.0.as_os_str()
        }
    }

    impl Drop for Fakes {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn sound_request() -> Request {
        Request {
            title: "Choose the sound file".into(),
            filter: filter("audio"),
            folder: Some(PathBuf::from("/tmp")),
        }
    }

    #[tokio::test]
    async fn zenity_first_then_kdialog_then_nothing() {
        let fakes = Fakes::new("order");
        // Nothing there.
        assert_eq!(
            choose(&sound_request(), None, Some(fakes.path())).await,
            Err(NOTHING.to_owned())
        );
        assert_eq!(
            choose(&sound_request(), None, None).await,
            Err(NOTHING.to_owned())
        );

        fakes.add("kdialog", "/tmp/from kdialog.oga", 0);
        assert_eq!(
            choose(&sound_request(), None, Some(fakes.path())).await,
            Ok(Some(PathBuf::from("/tmp/from kdialog.oga")))
        );
        assert_eq!(
            fakes.args("kdialog"),
            [
                "--title=Choose the sound file",
                "--getopenfilename",
                "/tmp",
                "*.oga *.ogg *.opus *.wav *.flac *.mp3 *.m4a|Sound files",
            ]
        );

        // zenity goes first.
        fakes.add("zenity", "/tmp/from zenity.oga", 0);
        assert_eq!(
            choose(&sound_request(), None, Some(fakes.path())).await,
            Ok(Some(PathBuf::from("/tmp/from zenity.oga")))
        );
        assert_eq!(
            fakes.args("zenity"),
            [
                "--file-selection",
                "--title=Choose the sound file",
                "--filename=/tmp/",
                "--file-filter=Sound files | *.oga *.ogg *.opus *.wav *.flac *.mp3 *.m4a",
                "--file-filter=All files | *",
            ]
        );

        // Closed without a file, and a dialog that failed.
        fakes.add("zenity", "", 1);
        assert_eq!(
            choose(&sound_request(), None, Some(fakes.path())).await,
            Ok(None)
        );
        fakes.add("zenity", "", 5);
        assert!(
            choose(&sound_request(), None, Some(fakes.path()))
                .await
                .is_err()
        );
        fakes.add("zenity", "relative.oga", 0);
        assert!(
            choose(&sound_request(), None, Some(fakes.path()))
                .await
                .is_err()
        );
    }

    /// A bus of the test's own, from a config that starts no services, so
    /// no real portal comes up on it.
    struct Bus {
        dir: PathBuf,
        daemon: std::process::Child,
        address: String,
    }

    impl Bus {
        /// `None` where there's no `dbus-daemon`, like a build sandbox.
        fn start(name: &str) -> Option<Self> {
            if !mochi_core::process::installed("dbus-daemon") {
                return None;
            }
            let dir =
                std::env::temp_dir().join(format!("mochi-files-bus-{name}-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let socket = dir.join("bus");
            let config = dir.join("bus.conf");
            std::fs::write(
                &config,
                format!(
                    "<busconfig>\n<type>session</type>\n<listen>unix:path={}</listen>\n\
                     <policy context=\"default\">\n<allow send_destination=\"*\" eavesdrop=\"true\"/>\n\
                     <allow eavesdrop=\"true\"/>\n<allow own=\"*\"/>\n</policy>\n</busconfig>\n",
                    socket.display()
                ),
            )
            .unwrap();
            let daemon = std::process::Command::new("dbus-daemon")
                .arg(format!("--config-file={}", config.display()))
                .arg("--nofork")
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap();
            let started = std::time::Instant::now();
            while !socket.exists() && started.elapsed() < Duration::from_secs(5) {
                std::thread::sleep(Duration::from_millis(20));
            }
            Some(Self {
                address: format!("unix:path={}", socket.display()),
                dir,
                daemon,
            })
        }

        async fn connect(&self) -> Connection {
            zbus::connection::Builder::address(self.address.as_str())
                .unwrap()
                .build()
                .await
                .unwrap()
        }
    }

    impl Drop for Bus {
        fn drop(&mut self) {
            let _ = self.daemon.kill();
            let _ = self.daemon.wait();
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// The title and the options a portal was asked with.
    type Asked = (String, HashMap<String, OwnedValue>);

    /// A portal that answers every `OpenFile` with `code` and `uris`, and
    /// keeps what it was asked.
    struct FakePortal {
        code: u32,
        uris: Vec<String>,
        asked: Arc<Mutex<Option<Asked>>>,
    }

    #[zbus::interface(name = "org.freedesktop.portal.FileChooser")]
    impl FakePortal {
        async fn open_file(
            &self,
            #[zbus(header)] header: zbus::message::Header<'_>,
            #[zbus(connection)] connection: &Connection,
            _parent: String,
            title: String,
            options: HashMap<String, OwnedValue>,
        ) -> OwnedObjectPath {
            let token = options
                .get("handle_token")
                .and_then(|token| String::try_from(token.try_clone().ok()?).ok())
                .unwrap_or_default();
            *self.asked.lock().unwrap() = Some((title, options));
            let sender = header.sender().unwrap().to_string();
            let path = request_path(&sender, &token);
            let (code, uris, connection) = (self.code, self.uris.clone(), connection.clone());
            let reply = path.clone();
            // The answer comes after the call returns, as when someone
            // picks a file.
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(50)).await;
                let results = HashMap::from([("uris", Value::from(uris))]);
                connection
                    .emit_signal(
                        None::<()>,
                        reply.as_str(),
                        REQUEST,
                        "Response",
                        &(code, results),
                    )
                    .await
                    .unwrap();
            });
            OwnedObjectPath::try_from(path).unwrap()
        }
    }

    #[tokio::test]
    async fn asks_the_portal_and_falls_back_without_one() {
        let Some(bus) = Bus::start("portal") else {
            return;
        };
        let us = bus.connect().await;
        let fakes = Fakes::new("portal");
        fakes.add("zenity", "/tmp/from zenity.oga", 0);

        // No portal on this bus: zenity asks.
        assert_eq!(
            choose(&sound_request(), Some(&us), Some(fakes.path())).await,
            Ok(Some(PathBuf::from("/tmp/from zenity.oga")))
        );

        let asked = Arc::new(Mutex::new(None));
        let portal = FakePortal {
            code: 0,
            uris: vec!["file:///tmp/Bell%20one.oga".into()],
            asked: asked.clone(),
        };
        let _server = zbus::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .name(PORTAL)
            .unwrap()
            .serve_at(DESKTOP, portal)
            .unwrap()
            .build()
            .await
            .unwrap();
        // The portal answers, so zenity isn't asked.
        std::fs::remove_file(fakes.0.join("zenity.args")).unwrap();
        assert_eq!(
            choose(&sound_request(), Some(&us), Some(fakes.path())).await,
            Ok(Some(PathBuf::from("/tmp/Bell one.oga")))
        );
        assert!(fakes.args("zenity").is_empty());
        let (title, options) = asked.lock().unwrap().take().unwrap();
        assert_eq!(title, "Choose the sound file");
        let current = options["current_filter"].to_string();
        assert!(
            current.contains("Sound files") && current.contains("audio/*"),
            "{current}"
        );
        let folder = Vec::<u8>::try_from(options["current_folder"].try_clone().unwrap()).unwrap();
        assert_eq!(folder, b"/tmp\0");
    }

    #[tokio::test]
    async fn a_closed_portal_dialog_chooses_nothing() {
        let Some(bus) = Bus::start("closed") else {
            return;
        };
        let us = bus.connect().await;
        let portal = FakePortal {
            code: 1,
            uris: Vec::new(),
            asked: Arc::default(),
        };
        let _server = zbus::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .name(PORTAL)
            .unwrap()
            .serve_at(DESKTOP, portal)
            .unwrap()
            .build()
            .await
            .unwrap();
        let request = Request {
            title: "Choose".into(),
            ..Request::default()
        };
        // Closed: no dialog after it, though zenity would answer.
        let fakes = Fakes::new("closed");
        fakes.add("zenity", "/tmp/x.oga", 0);
        assert_eq!(
            choose(&request, Some(&us), Some(fakes.path())).await,
            Ok(None)
        );
        assert!(fakes.args("zenity").is_empty());
    }
}
