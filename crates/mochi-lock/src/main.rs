//! `mochi-lock`: locks the screen until the right password, and draws the
//! lock screen. mochid starts it; run by hand, from another TTY, it takes
//! over a lock whose locker died.
//!
//! ```text
//! mochi-lock [--config <file.json>] [--marker <file>]
//! ```
//!
//! It says what happens on stdout, one JSON object a line: `locked` once the
//! compositor confirmed it, `unlocked`, `refused` with a reason, `broken`.
//! It exits 0 only once unlocked; see `lock::exit` for the rest. With
//! `--marker`, it writes its process id to that file once locked and
//! removes it once unlocked, so mochid finds a lock that outlived it.

mod auth;
mod config;
mod display;
mod face;
mod lock;
mod renderer;
mod state;

use std::path::PathBuf;

use config::Config;

fn main() {
    // A panic ends the process at once, unwinding nothing: whatever went
    // wrong, the screen stays locked, and mochid starts the lock again.
    std::panic::set_hook(Box::new(|info| {
        lock::report("broken", Some(&format!("mochi-lock panicked: {info}")));
        std::process::abort();
    }));

    // Day and month names in the user's language.
    // SAFETY: an empty locale name picks the environment's; called before
    // any other thread starts.
    unsafe { libc::setlocale(libc::LC_TIME, c"".as_ptr()) };

    let mut args = std::env::args().skip(1);
    let mut config = Config::default();
    let mut marker = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                let Some(path) = args.next().map(PathBuf::from) else {
                    lock::report("broken", Some("--config needs a file"));
                    std::process::exit(lock::exit::BROKEN);
                };
                // A config that doesn't read still locks, with the defaults:
                // a typo mustn't leave the screen unlocked.
                match Config::load(&path) {
                    Ok(loaded) => config = loaded,
                    Err(error) => lock::report("warning", Some(&error)),
                }
            }
            "--marker" => marker = args.next().map(PathBuf::from),
            "--help" | "-h" => {
                lock::report(
                    "help",
                    Some("mochi-lock [--config <file.json>] [--marker <file>]"),
                );
                std::process::exit(0);
            }
            other => {
                lock::report("broken", Some(&format!("unknown argument {other:?}")));
                std::process::exit(lock::exit::BROKEN);
            }
        }
    }

    std::process::exit(lock::run(config, marker));
}
