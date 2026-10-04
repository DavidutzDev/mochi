//! Recording through gpu-screen-recorder, which encodes on the GPU. It runs
//! as a child of the daemon until `stop` sends it SIGINT, which makes it
//! finish the file.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::oneshot;

use crate::crop::Rect;

/// What to record.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    /// An area in the global layout, in logical pixels.
    Region(Rect),
    /// A whole output, by name.
    Output(String),
    /// Whatever the user picks in the screen-cast portal: the way to record
    /// a single window.
    Portal,
}

/// How to record, from the settings.
#[derive(Debug, Clone, PartialEq)]
pub struct Options<'a> {
    /// The program and any arguments of its own.
    pub recorder: &'a [String],
    pub framerate: u32,
    /// Audio sources, mixed into one track.
    pub audio: Vec<&'a str>,
}

/// The recorder's command line.
pub fn command(options: &Options<'_>, target: &Target, file: &Path) -> Vec<String> {
    let mut argv: Vec<String> = options.recorder.to_vec();
    match target {
        Target::Region(area) => {
            let round = |value: f64| value.round() as i64;
            argv.extend([
                "-w".into(),
                "region".into(),
                "-region".into(),
                format!(
                    "{}x{}+{}+{}",
                    round(area.width),
                    round(area.height),
                    round(area.x),
                    round(area.y)
                ),
            ]);
        }
        Target::Output(name) => argv.extend(["-w".into(), name.clone()]),
        Target::Portal => argv.extend(["-w".into(), "portal".into()]),
    }
    argv.extend(["-f".into(), options.framerate.to_string()]);
    let sources: Vec<&str> = options
        .audio
        .iter()
        .copied()
        .filter(|source| !source.is_empty())
        .collect();
    if !sources.is_empty() {
        argv.extend(["-a".into(), sources.join("|")]);
    }
    argv.extend(["-o".into(), file.display().to_string()]);
    argv
}

/// A recording in progress.
#[derive(Debug)]
pub struct Recording {
    pid: u32,
    pub file: PathBuf,
    pub started: SystemTime,
    /// The recorder's exit: `Err` with what it said when it failed.
    done: oneshot::Receiver<Result<(), String>>,
}

impl Recording {
    pub fn start(argv: &[String], file: PathBuf) -> Result<Self, String> {
        let (program, args) = argv.split_first().ok_or("no recorder is set")?;
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| format!("cannot start {program}: {error}"))?;
        let pid = child.id().ok_or("the recorder exited at once")?;
        // Keeps the last line it says, read while it runs so a full pipe never
        // blocks it.
        let said = Arc::new(Mutex::new(None));
        if let Some(stderr) = child.stderr.take() {
            let said = Arc::clone(&said);
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if let Some(line) = last_line(&line) {
                        *said.lock().unwrap_or_else(PoisonError::into_inner) = Some(line);
                    }
                }
            });
        }

        let (sender, done) = oneshot::channel();
        tokio::spawn(async move {
            let status = child.wait().await;
            // Its last words may still be in the pipe.
            tokio::time::sleep(Duration::from_millis(100)).await;
            let said = said.lock().unwrap_or_else(PoisonError::into_inner).take();
            let result = match status {
                Ok(status) if status.success() => Ok(()),
                Ok(status) => Err(said.unwrap_or(format!("the recorder {status}"))),
                Err(error) => Err(error.to_string()),
            };
            let _ = sender.send(result);
        });
        Ok(Self {
            pid,
            file,
            started: SystemTime::now(),
            done,
        })
    }

    /// Asks the recorder to finish the file and exit.
    pub fn stop(&self) {
        let Ok(pid) = libc::pid_t::try_from(self.pid) else {
            return;
        };
        // SAFETY: `kill` takes plain integers. The pid is our own child's,
        // which can't be reaped and reused until the task above waits for it,
        // and that only happens after it exits.
        unsafe {
            libc::kill(pid, libc::SIGINT);
        }
    }

    /// Waits for the recorder to exit.
    pub async fn finished(&mut self) -> Result<(), String> {
        (&mut self.done)
            .await
            .unwrap_or_else(|_| Err("lost track of the recorder".into()))
    }
}

/// gpu-screen-recorder's usual failures, said short enough for the island
/// and with what to do.
pub fn explain(message: &str) -> String {
    if message.contains("kms server") {
        return "gpu-screen-recorder can't capture the screen without its helper. \
                On NixOS, set programs.gpu-screen-recorder.enable = true"
            .into();
    }
    let missing = message
        .strip_prefix("cannot start ")
        .and_then(|rest| rest.split_once(": "))
        .filter(|(_, error)| error.contains("No such file"));
    match missing {
        Some((program, _)) => {
            format!("{program} isn't installed. Install it, or set recorder in [module.capture]")
        }
        None => message.to_owned(),
    }
}

/// The last line worth showing from a program's error output.
fn last_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(audio: Vec<&str>) -> Options<'_> {
        static RECORDER: std::sync::LazyLock<Vec<String>> =
            std::sync::LazyLock::new(|| vec!["gpu-screen-recorder".into()]);
        Options {
            recorder: &RECORDER,
            framerate: 60,
            audio,
        }
    }

    #[test]
    fn records_a_region_in_global_coordinates() {
        let area = Rect::new(1942.4, 62.0, 640.0, 359.6);
        let argv = command(
            &options(vec!["default_output"]),
            &Target::Region(area),
            Path::new("/tmp/a.mp4"),
        );
        assert_eq!(
            argv.join(" "),
            "gpu-screen-recorder -w region -region 640x360+1942+62 -f 60 -a default_output -o /tmp/a.mp4"
        );
    }

    #[test]
    fn mixes_the_microphone_in_and_skips_empty_sources() {
        let argv = command(
            &options(vec!["default_output", "default_input"]),
            &Target::Output("DP-3".into()),
            Path::new("/tmp/a.mp4"),
        );
        assert_eq!(
            argv.join(" "),
            "gpu-screen-recorder -w DP-3 -f 60 -a default_output|default_input -o /tmp/a.mp4"
        );

        let argv = command(&options(vec![""]), &Target::Portal, Path::new("/tmp/a.mp4"));
        assert_eq!(
            argv.join(" "),
            "gpu-screen-recorder -w portal -f 60 -o /tmp/a.mp4"
        );
    }

    #[test]
    fn explains_the_usual_failures() {
        let kms = "gsr error: gsr_kms_client_init: kms server died or never started";
        assert!(explain(kms).ends_with("programs.gpu-screen-recorder.enable = true"));
        let missing = "cannot start gpu-screen-recorder: No such file or directory (os error 2)";
        assert_eq!(
            explain(missing),
            "gpu-screen-recorder isn't installed. Install it, or set recorder in [module.capture]"
        );
        assert_eq!(explain("something else"), "something else");
    }

    #[test]
    fn keeps_the_last_thing_said() {
        assert_eq!(
            last_line("gsr info: starting\ngsr error: no permission\n\n").as_deref(),
            Some("gsr error: no permission")
        );
        assert_eq!(last_line("  \n"), None);
    }

    #[tokio::test]
    async fn stops_on_sigint_and_reports_failures() {
        let sleeper = [
            "sh".into(),
            "-c".into(),
            "trap 'exit 0' INT; sleep 10 & wait".into(),
        ];
        let mut recording = Recording::start(&sleeper, PathBuf::from("/tmp/x")).unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
        recording.stop();
        assert_eq!(recording.finished().await, Ok(()));

        let failing = [
            "sh".into(),
            "-c".into(),
            "echo 'gsr error: nope' >&2; exit 1".into(),
        ];
        let mut recording = Recording::start(&failing, PathBuf::from("/tmp/x")).unwrap();
        assert_eq!(recording.finished().await, Err("gsr error: nope".into()));
    }
}
