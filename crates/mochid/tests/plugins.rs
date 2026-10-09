//! Plugins in a real mochid: a backend written in plain `sh`, to show the
//! protocol needs no SDK, loaded from plugins.toml with `path:` sources.

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

use common::{Client, Daemon};
use mochi_protocol::{ClientMessage, DaemonMessage, PluginState, PluginStatus, Role};
use serde_json::json;

/// Shows an activity and publishes state on `shout`, exits on `crash`,
/// and repeats when the `beta` plugin publishes state. With a `crash` file
/// in its directory, it exits right after mochid's hello.
const BACKEND: &str = r#"#!/bin/sh
read -r hello <&3
if [ -e "$MOCHI_PLUGIN_DIR/crash" ]; then exit 1; fi
printf '{"type":"hello","api":1}\n' >&3
while read -r line <&3; do
    case "$line" in
    *'"type":"command"'*)
        id=${line#*\"id\":}
        id=${id%%,*}
        case "$line" in
        *'"action":"shout"'*)
            printf '{"type":"present","id":1,"spec":{"compact":"Shout","payload":{"text":"hi"}}}\n' >&3
            printf '{"type":"publish_state","state":{"shouted":true}}\n' >&3
            printf '{"type":"reply","id":%s,"output":"shouted"}\n' "$id" >&3 ;;
        *'"action":"crash"'*) exit 3 ;;
        *) printf '{"type":"reply","id":%s,"error":"no such action"}\n' "$id" >&3 ;;
        esac ;;
    *'"type":"state"'*'"module":"beta"'*)
        printf '{"type":"publish_state","state":{"saw":"beta"}}\n' >&3 ;;
    esac
done
"#;

fn plugin(dir: &Path, id: &str, extra: &str) {
    let root = dir.join("plugins").join(id);
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir_all(root.join("qml/overrides/idle")).unwrap();
    fs::write(
        root.join("mochi-plugin.toml"),
        format!(
            r#"[plugin]
id = "{id}"
name = "Echo"
version = "1.0.0"
api = 1

[backend]
exec = "bin/backend"
{extra}
[[actions]]
name = "shout"
description = "Show something"

[[actions]]
name = "crash"
description = "Exit with an error"

[[contributions]]
target = "control-center"
kind = "card"
id = "{id}"
view = "Shout"
title = "Echo"
"#
        ),
    )
    .unwrap();
    let backend = root.join("bin/backend");
    fs::write(&backend, BACKEND).unwrap();
    fs::set_permissions(&backend, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(root.join("qml/Shout.qml"), "import QtQuick\nItem {}\n").unwrap();
    fs::write(
        root.join("qml/overrides/idle/Pill.qml"),
        "import QtQuick\nItem {}\n",
    )
    .unwrap();
}

fn start(name: &str) -> Daemon {
    Daemon::start_prepared(
        name,
        "idle,control-center,echo,beta,gone",
        None,
        |dir, command| {
            plugin(
                dir,
                "echo",
                "\n[uses]\nstate = [\"beta\"]\n\n[views]\noverrides = [\"idle/Pill\"]\n",
            );
            plugin(dir, "beta", "");
            let config = dir.join("config/mochi");
            fs::create_dir_all(&config).unwrap();
            fs::write(
            config.join("plugins.toml"),
            format!(
                "[plugins.echo]\nsource = \"path:{0}/plugins/echo\"\n\n[plugins.beta]\nsource = \"path:{0}/plugins/beta\"\n\n[plugins.gone]\nsource = \"git:github.com/example/gone\"\n",
                dir.display()
            ),
        )
        .unwrap();
            // A plugin that gives up sends a desktop notification; not to the
            // session running the tests.
            command.env("DBUS_SESSION_BUS_ADDRESS", "unix:path=/nonexistent");
        },
    )
}

fn plugins(ctl: &mut Client) -> Vec<PluginStatus> {
    ctl.send(&ClientMessage::Status);
    match ctl.recv() {
        DaemonMessage::Status { status } => status.plugins,
        other => panic!("expected a status, got {other:?}"),
    }
}

fn state_of(plugins: &[PluginStatus], id: &str) -> PluginState {
    plugins.iter().find(|plugin| plugin.id == id).unwrap().state
}

/// Reads until `found` picks a message.
fn wait<T>(client: &mut Client, mut found: impl FnMut(DaemonMessage) -> Option<T>) -> T {
    loop {
        if let Some(value) = found(client.recv()) {
            return value;
        }
    }
}

#[test]
fn plugins_run_like_modules() {
    let daemon = start("plugins");
    let mut ui = daemon.client(Role::Ui);
    let mut ctl = daemon.client(Role::Ctl);

    let modules = wait(&mut ui, |message| match message {
        DaemonMessage::Modules { modules } => Some(modules),
        _ => None,
    });
    assert_eq!(modules, ["idle", "control-center", "echo", "beta"]);
    let contributions = wait(&mut ui, |message| match message {
        DaemonMessage::Contributions { contributions } => Some(contributions),
        _ => None,
    });
    assert!(
        contributions
            .iter()
            .any(|offer| offer.module == "echo" && offer.view == "Shout")
    );

    daemon.wait_for(
        || {
            let plugins = plugins(&mut ctl);
            state_of(&plugins, "echo") == PluginState::Running
                && state_of(&plugins, "beta") == PluginState::Running
        },
        "the plugins to run",
    );
    let listed = plugins(&mut ctl);
    let gone = listed.iter().find(|plugin| plugin.id == "gone").unwrap();
    assert_eq!(gone.state, PluginState::Missing);
    assert!(
        gone.message
            .as_deref()
            .unwrap()
            .contains("mochi plugins install gone")
    );

    // An action: an activity on the island, state, and output for the CLI.
    assert_eq!(
        ctl.command("echo", "shout", &[]),
        DaemonMessage::Output {
            output: "shouted".into()
        }
    );
    let shown = ui.wait_for_view("echo", "Shout");
    assert_eq!(shown.payload, json!({ "text": "hi" }));

    // Watching: beta's state reaches echo, which says so in its own.
    assert!(matches!(
        ctl.command("beta", "shout", &[]),
        DaemonMessage::Output { .. }
    ));
    wait(&mut ui, |message| match message {
        DaemonMessage::State { module, state }
            if module == "echo" && state == json!({ "saw": "beta" }) =>
        {
            Some(())
        }
        _ => None,
    });

    // The override sits in the idle module's directory.
    let pill = daemon.dir.join("run/mochi/shell/modules/idle/Pill.qml");
    assert_eq!(
        fs::read_link(&pill).unwrap(),
        daemon.dir.join("plugins/echo/qml/overrides/idle/Pill.qml")
    );

    // `mochi ipc echo` lists the manifest's actions.
    ctl.send(&ClientMessage::ListActions {
        module: Some("echo".into()),
    });
    let DaemonMessage::Actions { modules } = ctl.recv() else {
        panic!("expected actions");
    };
    let names: Vec<&str> = modules[0]
        .actions
        .iter()
        .map(|action| action.name.as_str())
        .collect();
    assert_eq!(names, ["shout", "crash"]);
}

#[test]
fn a_crashing_backend_restarts_then_gives_up_until_a_reload() {
    let daemon = start("plugin-crash");
    let mut ctl = daemon.client(Role::Ctl);
    daemon.wait_for(
        || state_of(&plugins(&mut ctl), "echo") == PluginState::Running,
        "echo to run",
    );

    // Every restart crashes too, until it gives up.
    let crash = daemon.dir.join("plugins/echo/crash");
    fs::write(&crash, "").unwrap();
    assert!(matches!(
        ctl.command("echo", "crash", &[]),
        DaemonMessage::Error { .. }
    ));
    daemon.wait_long(
        || state_of(&plugins(&mut ctl), "echo") == PluginState::Failed,
        "echo to give up",
        Duration::from_secs(15),
    );
    assert!(daemon.log().contains("crashed 5 times within a minute"));
    let failed = plugins(&mut ctl);
    assert!(
        failed
            .iter()
            .find(|plugin| plugin.id == "echo")
            .unwrap()
            .message
            .as_deref()
            .unwrap()
            .contains("mochi reload")
    );
    // The other plugin carries on.
    assert_eq!(state_of(&failed, "beta"), PluginState::Running);

    fs::remove_file(&crash).unwrap();
    ctl.send(&ClientMessage::Reload);
    assert_eq!(ctl.recv(), DaemonMessage::Ok);
    daemon.wait_for(
        || state_of(&plugins(&mut ctl), "echo") == PluginState::Running,
        "echo to run again",
    );
    assert!(matches!(
        ctl.command("echo", "shout", &[]),
        DaemonMessage::Output { .. }
    ));
}

/// `examples/python/hello`: a backend in Python, with the small SDK the
/// "Making an SDK" page walks through.
#[test]
fn the_python_example_runs() {
    let python = std::env::var_os("PATH")
        .and_then(|path| {
            std::env::split_paths(&path)
                .map(|dir| dir.join("python3"))
                .find(|candidate| candidate.is_file())
        })
        .expect("python3 is in PATH, for the Python example plugin");
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/python/hello");

    let daemon = Daemon::start_prepared(
        "python",
        "idle,control-center,hello",
        None,
        |dir, command| {
            let plugin = dir.join("hello");
            copy_dir(&example, &plugin);
            // `#!/usr/bin/env` isn't there in every build sandbox.
            let script = plugin.join("hello.py");
            let text = fs::read_to_string(&script).unwrap();
            let (_, rest) = text.split_once('\n').unwrap();
            fs::write(&script, format!("#!{}\n{rest}", python.display())).unwrap();
            fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

            let config = dir.join("config/mochi");
            fs::create_dir_all(&config).unwrap();
            fs::write(
                config.join("plugins.toml"),
                format!("[plugins.hello]\nsource = \"path:{}\"\n", plugin.display()),
            )
            .unwrap();
            command.env("DBUS_SESSION_BUS_ADDRESS", "unix:path=/nonexistent");
        },
    );
    let mut ui = daemon.client(Role::Ui);
    let mut ctl = daemon.client(Role::Ctl);
    daemon.wait_for(
        || state_of(&plugins(&mut ctl), "hello") == PluginState::Running,
        "the Python plugin to run",
    );

    assert_eq!(
        ctl.command("hello", "say", &["hi", "there"]),
        DaemonMessage::Output {
            output: "said 'hi there'".into()
        }
    );
    // The activity and the bubble, in whichever order they come.
    let (mut shown, mut bubble) = (None, None);
    while shown.is_none() || bubble.is_none() {
        match ui.recv() {
            DaemonMessage::Present {
                activity: Some(activity),
                ..
            } if activity.module == "hello" => shown = Some(activity),
            DaemonMessage::Bubbles { bubbles, .. } => {
                if let Some(found) = bubbles.into_iter().find(|bubble| bubble.module == "hello") {
                    bubble = Some(found);
                }
            }
            _ => {}
        }
    }
    let (shown, bubble) = (shown.unwrap(), bubble.unwrap());
    assert_eq!(shown.view, "Hello");
    assert_eq!(shown.payload, json!({ "text": "hi there" }));
    assert_eq!(bubble.payload, json!({ "said": 1 }));
    assert_eq!(
        ctl.command("hello", "count", &[]),
        DaemonMessage::Output { output: "1".into() }
    );

    // The bubble calls the control center, which opens.
    ui.send(&ClientMessage::BubbleClick { bubble: bubble.id });
    ui.wait_for_view("control-center", "ControlCenter");
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}
