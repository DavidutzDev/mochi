//! The weather through a real `mochid`, with a curl that answers with the
//! module's fixtures and writes down every URL it's asked for: nothing is
//! asked while no place is set, and a place is looked up, then its
//! forecast fetched.

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use common::Daemon;
use mochi_protocol::{DaemonMessage, ErrorCode, Role};

/// Stands in for curl: the URL is the last argument.
const FAKE_CURL: &str = r#"#!/bin/sh
for url; do :; done
printf '%s\n' "$url" >> "$(dirname "$0")/asked"
case "$url" in
    https://geocoding-api.open-meteo.com/*) cat "$FIXTURES/geocoding.json" ;;
    https://api.open-meteo.com/*) cat "$FIXTURES/forecast.json" ;;
    *) exit 6 ;;
esac
"#;

fn output(message: DaemonMessage) -> String {
    match message {
        DaemonMessage::Output { output } => output,
        other => panic!("expected output, got {other:?}"),
    }
}

/// Starts mochid with the fake curl first on the PATH, and `settings` as
/// `[module.weather]`.
fn start(name: &str, settings: &str) -> Daemon {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../modules/weather/fixtures");
    Daemon::start_prepared(name, "idle,weather", None, |dir, command| {
        let bin = dir.join("bin");
        fs::create_dir_all(&bin).unwrap();
        let curl = bin.join("curl");
        fs::write(&curl, FAKE_CURL).unwrap();
        fs::set_permissions(&curl, fs::Permissions::from_mode(0o755)).unwrap();
        let path = std::env::var("PATH").unwrap_or_default();
        command.env("PATH", format!("{}:{path}", bin.display()));
        command.env("FIXTURES", &fixtures);

        let config = dir.join("config/mochi");
        fs::create_dir_all(&config).unwrap();
        fs::write(
            config.join("config.toml"),
            format!("[module.weather]\n{settings}\n"),
        )
        .unwrap();
    })
}

fn asked(daemon: &Daemon) -> Vec<String> {
    fs::read_to_string(daemon.dir.join("bin/asked"))
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn no_place_sends_nothing() {
    let daemon = start("weather-unset", "");
    let mut ctl = daemon.client(Role::Ctl);
    let status = output(ctl.command("weather", "status", &[]));
    assert!(status.starts_with("no place set"), "{status}");
    let refused = ctl.command("weather", "refresh", &[]);
    assert!(
        matches!(
            refused,
            DaemonMessage::Error {
                code: ErrorCode::ModuleFailed,
                ..
            }
        ),
        "{refused:?}"
    );
    // Past the moment a first fetch would go.
    thread::sleep(Duration::from_secs(4));
    assert_eq!(asked(&daemon), Vec::<String>::new());
    assert!(!state_file(&daemon).exists());
}

#[test]
fn a_place_is_looked_up_then_its_forecast_fetched() {
    let daemon = start("weather-lyon", "place = \"Lyon, France\"");
    let mut ctl = daemon.client(Role::Ctl);
    let mut status = String::new();
    daemon.wait_for(
        || {
            status = output(ctl.command("weather", "status", &[]));
            status.starts_with("Lyon:")
        },
        "the forecast",
    );
    assert!(status.contains("°C, overcast"), "{status}");
    let asked = asked(&daemon);
    assert_eq!(asked.len(), 2, "{asked:?}");
    assert!(asked[0].ends_with("&name=Lyon"), "{asked:?}");
    assert!(asked[1].contains("latitude=45.74906"), "{asked:?}");
    assert!(state_file(&daemon).is_file());
}

fn state_file(daemon: &Daemon) -> PathBuf {
    daemon.dir.join("state/mochi/weather.json")
}
