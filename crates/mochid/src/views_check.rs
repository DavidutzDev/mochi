//! A test that compiles every QML view: the core's, and each builtin
//! module's, in a shell written as mochid writes it. A view with a syntax
//! error, an unknown type or a property that doesn't exist fails the build,
//! instead of failing on someone's screen.
//!
//! Quickshell runs with Qt's offscreen platform, so it needs no display and
//! touches none, and quits once it has compiled them all. The same way, a
//! second test runs the island's Zones.js, the time zone search the zone
//! pickers share, on a few zones.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use mochi_core::assets::{self, Mode, ShellModule};

use crate::modules::builtin;

/// What Quickshell says of a window without Wayland.
const OFFSCREEN: &str = "No PanelWindow backend loaded";

/// Every view's URL under the shell: `root:/island/Pill.qml`, `root:/modules/osd/Volume.qml`.
fn urls(modules: &[Box<dyn mochi_core::Module>]) -> Vec<String> {
    let mut urls = Vec::new();
    fn walk(dir: &include_dir::Dir<'_>, found: &mut Vec<String>) {
        for entry in dir.entries() {
            match entry {
                include_dir::DirEntry::Dir(inner) => walk(inner, found),
                include_dir::DirEntry::File(file) => {
                    let path = file.path().to_string_lossy().into_owned();
                    if path.ends_with(".qml") {
                        found.push(path);
                    }
                }
            }
        }
    }
    let qml = |assets: &mochi_core::Assets| -> Vec<String> {
        let mut found = Vec::new();
        if let mochi_core::Assets::Embedded { dir, .. } = assets {
            walk(dir, &mut found);
        }
        found
    };
    for path in qml(&mochi_core::QML) {
        if path.starts_with("island/") {
            urls.push(format!("root:/{path}"));
        }
    }
    for module in modules {
        for path in qml(&module.assets()) {
            urls.push(format!("root:/modules/{}/{path}", module.id()));
        }
    }
    urls.sort();
    urls
}

/// The checker that replaces `shell.qml`: compiles each view and prints
/// what's wrong with it.
fn checker(urls: &[String]) -> String {
    let list = serde_json::to_string(urls).expect("strings serialize");
    format!(
        r#"import QtQuick
import Quickshell

ShellRoot {{
    Component.onCompleted: {{
        const urls = {list};
        for (const url of urls) {{
            const component = Qt.createComponent(url);
            if (component.status === Component.Error)
                console.warn(`VIEW-ERROR ${{url}}: ${{component.errorString().trim().replace(/\n/g, " | ")}}`);
        }}
        console.warn(`VIEWS-CHECKED ${{urls.length}}`);
    }}
}}
"#
    )
}

/// Runs `shell` in Quickshell, offscreen with a runtime directory and a bus
/// address in `dir` that lead nowhere, and gives what it prints until a line
/// with `done`, that one too.
fn run_until(dir: &Path, shell: &Path, done: &str) -> Vec<String> {
    let Some(quickshell) = std::env::var_os("MOCHI_QUICKSHELL")
        .map(Into::into)
        .or_else(|| which("quickshell"))
    else {
        panic!("quickshell isn't on the PATH; set MOCHI_QUICKSHELL to run this test");
    };
    let runtime = dir.join("runtime");
    std::fs::create_dir_all(&runtime).unwrap();
    let mut child = Command::new(quickshell)
        .arg("-p")
        .arg(shell)
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("XDG_RUNTIME_DIR", &runtime)
        // No session bus: nothing a lookup could start, like a portal.
        .env(
            "DBUS_SESSION_BUS_ADDRESS",
            format!("unix:path={}", dir.join("nobus").display()),
        )
        .env("NO_COLOR", "1")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DISPLAY")
        .env_remove("MOCHI_SOCKET")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("quickshell starts");

    // Both streams, line by line.
    let (lines, received) = mpsc::channel();
    let stdout = child.stdout.take().expect("piped");
    let stderr = child.stderr.take().expect("piped");
    for stream in [
        Box::new(stdout) as Box<dyn std::io::Read + Send>,
        Box::new(stderr),
    ] {
        let lines = lines.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                let _ = lines.send(line);
            }
        });
    }
    let mut out = Vec::new();
    while let Ok(line) = received.recv_timeout(Duration::from_secs(60)) {
        let last = line.contains(done);
        out.push(line);
        if last {
            break;
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    out
}

#[test]
fn every_view_compiles() {
    let dir = std::env::temp_dir().join(format!("mochi-views-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let shell = dir.join("shell");
    let modules = builtin();
    let assets: Vec<mochi_core::Assets> = modules.iter().map(|module| module.assets()).collect();
    let views: Vec<ShellModule<'_>> = modules
        .iter()
        .zip(&assets)
        .map(|(module, assets)| ShellModule {
            id: module.id(),
            assets,
            overrides: &[],
        })
        .collect();
    assets::write_shell(&shell, &mochi_core::QML, &views, &[], Mode::Copy).unwrap();
    let urls = urls(&modules);
    assert!(urls.len() > 100, "only {} views", urls.len());
    std::fs::write(shell.join("shell.qml"), checker(&urls)).unwrap();

    let mut errors = Vec::new();
    let mut checked = None;
    for line in run_until(&dir, &shell.join("shell.qml"), "VIEWS-CHECKED ") {
        if let Some(at) = line.find("VIEW-ERROR ") {
            let error = &line[at + "VIEW-ERROR ".len()..];
            // The windows need Wayland's layer shell, which the offscreen
            // platform doesn't have: that says nothing about the file.
            if !error.contains(OFFSCREEN) {
                errors.push(error.to_owned());
            }
        }
        if let Some(at) = line.find("VIEWS-CHECKED ") {
            checked = line[at + "VIEWS-CHECKED ".len()..]
                .trim()
                .parse::<usize>()
                .ok();
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(
        checked,
        Some(urls.len()),
        "quickshell didn't check the views"
    );
    assert!(
        errors.is_empty(),
        "views that don't compile:\n{}",
        errors.join("\n")
    );
}

fn which(program: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("PATH")?
        .to_str()?
        .split(':')
        .map(|dir| Path::new(dir).join(program))
        .find(|path| path.is_file())
}

/// The zones Zones.js searches in the test, as mochi_core::zones lists
/// them: west to east, then by name. Noumea's offset is made up, to put a
/// "New" country west of New York.
fn zone_fixture() -> serde_json::Value {
    use mochi_core::zones::Zone;
    let zone = |name: &str, country: &str, hours: f64| Zone {
        name: name.to_owned(),
        country: country.to_owned(),
        offset: (hours * 3600.0) as i64,
    };
    [
        zone("Pacific/Noumea", "New Caledonia", -11.0),
        zone("America/Los_Angeles", "United States", -7.0),
        zone("America/New_York", "United States", -4.0),
        zone("America/Argentina/Buenos_Aires", "Argentina", -3.0),
        zone("America/Sao_Paulo", "Brazil", -3.0),
        zone("UTC", "", 0.0),
        zone("Europe/London", "Britain (UK)", 1.0),
        zone("Europe/Paris", "France", 2.0),
        zone("Asia/Kolkata", "India", 5.5),
        zone("Asia/Seoul", "Korea (South)", 9.0),
        zone("Asia/Tokyo", "Japan", 9.0),
    ]
    .iter()
    .map(Zone::choice)
    .collect()
}

/// A shell that runs Zones.js on the fixture and prints what it found.
fn zone_checker(zones: &serde_json::Value) -> String {
    format!(
        r#"import QtQuick
import Quickshell
import "Zones.js" as Zones

ShellRoot {{
    Component.onCompleted: {{
        const zones = {zones};
        const names = list => list.map(zone => zone.value);
        const queries = ["tok", "japan", "utc+9", "São", "buenos aires", "new", "york new", "united states", "  ", "zzz"];
        const found = {{}};
        for (const query of queries)
            found[query] = names(Zones.search(zones, query));
        const titles = {{ "chosen": "C", "suggested": "S", "all": "A" }};
        const brief = rows => rows.map(row => row.kind === "heading" ? `# ${{row.label}}` : row.value);
        const rows = {{
            "empty": brief(Zones.rows(zones, "", ["Asia/Tokyo", "Mars/Olympus"], ["Europe/London", "Asia/Tokyo"], titles, false)),
            "nothing chosen": brief(Zones.rows(zones, "", [], [], titles, false)),
            "typed": brief(Zones.rows(zones, "Etc/GMT+3", [], [], titles, true)),
            "typed off": brief(Zones.rows(zones, "Etc/GMT+3", [], [], titles, false)),
            "known": brief(Zones.rows(zones, "Asia/Tokyo", [], [], titles, true)),
            "words": brief(Zones.rows(zones, "tokyo please", [], [], titles, true))
        }};
        console.warn(`ZONES-DONE ${{JSON.stringify({{ "found": found, "rows": rows }})}}`);
    }}
}}
"#
    )
}

#[test]
fn zone_search_finds_cities_countries_and_offsets() {
    let dir = std::env::temp_dir().join(format!("mochi-zones-js-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mochi_core::Assets::Embedded { dir: qml, .. } = &mochi_core::QML else {
        panic!("the core's QML is embedded");
    };
    let script = qml
        .get_file("island/Zones.js")
        .expect("island/Zones.js")
        .contents();
    std::fs::write(dir.join("Zones.js"), script).unwrap();
    std::fs::write(dir.join("shell.qml"), zone_checker(&zone_fixture())).unwrap();
    let lines = run_until(&dir, &dir.join("shell.qml"), "ZONES-DONE ");
    let _ = std::fs::remove_dir_all(&dir);
    let result: serde_json::Value = lines
        .iter()
        .find_map(|line| {
            let at = line.find("ZONES-DONE ")?;
            serde_json::from_str(&line[at + "ZONES-DONE ".len()..]).ok()
        })
        .unwrap_or_else(|| panic!("Zones.js said nothing:\n{}", lines.join("\n")));

    let found = |query: &str| -> Vec<String> {
        serde_json::from_value(result["found"][query].clone()).unwrap()
    };
    // The city first, then a country, then an offset.
    assert_eq!(found("tok"), ["Asia/Tokyo"]);
    assert_eq!(found("japan"), ["Asia/Tokyo"]);
    assert_eq!(found("utc+9"), ["Asia/Seoul", "Asia/Tokyo"]);
    // Without its accent, and with a space for the underscore.
    assert_eq!(found("São"), ["America/Sao_Paulo"]);
    assert_eq!(found("buenos aires"), ["America/Argentina/Buenos_Aires"]);
    // A city that starts with it comes before a country that has it, even
    // one further west.
    assert_eq!(found("new"), ["America/New_York", "Pacific/Noumea"]);
    assert_eq!(found("york new"), ["America/New_York"]);
    assert_eq!(
        found("united states"),
        ["America/Los_Angeles", "America/New_York"]
    );
    assert_eq!(found("  ").len(), 11);
    assert!(found("zzz").is_empty());

    let rows = |case: &str| -> Vec<String> {
        serde_json::from_value(result["rows"][case].clone()).unwrap()
    };
    // Before a search: the chosen ones, one the list doesn't have too,
    // then the suggested ones not chosen, then the rest.
    let empty = rows("empty");
    assert_eq!(
        empty[..6],
        [
            "# C",
            "Asia/Tokyo",
            "Mars/Olympus",
            "# S",
            "Europe/London",
            "# A"
        ]
    );
    assert_eq!(empty.len(), 6 + 9);
    assert!(!empty[6..].contains(&"Asia/Tokyo".to_owned()));
    assert_eq!(rows("nothing chosen")[0], "# A");
    // A name typed by hand is offered when the list doesn't have it, and
    // only where the picker takes one.
    assert_eq!(rows("typed"), ["Etc/GMT+3"]);
    assert!(rows("typed off").is_empty());
    assert_eq!(rows("known"), ["Asia/Tokyo"]);
    assert!(rows("words").is_empty());
}
