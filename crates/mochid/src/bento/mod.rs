//! `mochid bento`, which `mochi bento` runs: sharing setups, themes and
//! plugins.
//!
//! A bento is a directory with a `mochi-bento.toml` (see [`manifest`]):
//! a setup's settings, theme, widgets and the plugins it needs. `share`
//! makes one from the setup running now; `add` installs a bento, a theme
//! or a plugin from a directory or a git repository; `try` applies a
//! theme or a bento's look until Keep or Drop; `remove` takes one out
//! again. What Bento installed is in `bento.toml` next to config.toml.

// A command for a terminal: it talks to the user.
#![allow(clippy::print_stdout, clippy::print_stderr)]

mod apply;
mod client;
mod fetch;
mod manifest;
mod screens;
mod share;

use std::path::{Path, PathBuf};

use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum Action {
    /// Make a bento of your setup: its settings, theme, widgets and
    /// plugins, without what belongs to this machine or to you.
    Share {
        /// The directory to write it in; its name is the bento's id.
        dir: PathBuf,
        /// What it's called; the directory's name without one.
        #[arg(long)]
        name: Option<String>,
        /// Bring the wallpaper along.
        #[arg(long)]
        wallpaper: bool,
        /// Write into a directory that isn't empty.
        #[arg(long)]
        force: bool,
        /// Print the manifest instead, to paste somewhere like a gist.
        #[arg(long)]
        print: bool,
    },
    /// Install a bento, a theme or a plugin: a directory, a git repository
    /// like `github.com/<user>/<repo>`, a gist's URL, or any source
    /// plugins.toml takes.
    Add {
        source: String,
        /// Don't ask.
        #[arg(long, short)]
        yes: bool,
    },
    /// Apply a theme, or a bento's settings and theme, without keeping
    /// them: Keep or Drop them in the settings.
    Try { source: String },
    /// Take out a bento, a theme or a plugin Bento installed. A bento puts
    /// back the settings and widgets it replaced.
    Remove {
        id: String,
        /// Don't ask.
        #[arg(long, short)]
        yes: bool,
    },
    /// Show what Bento installed.
    List,
    /// Check a bento, a theme or a plugin in a directory, as `add` would.
    Check {
        #[arg(default_value = ".")]
        dir: PathBuf,
    },
}

pub fn run(action: &Action, config_file: &Path) -> Result<(), String> {
    match action {
        Action::Share {
            dir,
            name,
            wallpaper,
            force,
            print,
        } => share::share(
            config_file,
            dir,
            &share::Options {
                name: name.clone(),
                wallpaper: *wallpaper,
                force: *force,
                print: *print,
            },
        ),
        Action::Add { source, yes } => apply::add(config_file, source, *yes),
        Action::Try { source } => apply::try_it(config_file, source),
        Action::Remove { id, yes } => apply::remove(config_file, id, *yes),
        Action::List => apply::list(config_file),
        Action::Check { dir } => apply::check_dir(dir),
    }
}
