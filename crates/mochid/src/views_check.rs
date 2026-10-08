//! A test that compiles every QML view: the core's, and each builtin
//! module's, in a shell written as mochid writes it. A view with a syntax
//! error, an unknown type or a property that doesn't exist fails the build,
//! instead of failing on someone's screen.
//!
//! Quickshell runs with Qt's offscreen platform, so it needs no display and
//! touches none, and quits once it has compiled them all.

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

#[test]
fn every_view_compiles() {
    let Some(quickshell) = std::env::var_os("MOCHI_QUICKSHELL")
        .map(Into::into)
        .or_else(|| which("quickshell"))
    else {
        panic!("quickshell isn't on the PATH; set MOCHI_QUICKSHELL to run this test");
    };
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

    let runtime = dir.join("runtime");
    std::fs::create_dir_all(&runtime).unwrap();
    let mut child = Command::new(quickshell)
        .arg("-p")
        .arg(shell.join("shell.qml"))
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("XDG_RUNTIME_DIR", &runtime)
        .env("NO_COLOR", "1")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DISPLAY")
        .env_remove("MOCHI_SOCKET")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("quickshell starts");

    // Both streams, line by line, until the checker says it's done.
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
    let mut errors = Vec::new();
    let mut checked = None;
    while let Ok(line) = received.recv_timeout(Duration::from_secs(60)) {
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
            break;
        }
    }
    let _ = child.kill();
    let _ = child.wait();
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
