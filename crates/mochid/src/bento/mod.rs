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
mod profiles;
mod publish;
mod registry;
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
        /// Only these parts, by comma: theme, shell (the island and the
        /// bubbles), modules (which run, with their plugins), settings (every
        /// module's), module:<id> (one module's), widgets, wallpaper.
        /// Everything but the wallpaper without it.
        #[arg(long, value_name = "PARTS")]
        only: Option<String>,
        /// Bring the wallpaper along too.
        #[arg(long)]
        wallpaper: bool,
        /// Write the look in use as a theme package instead: colors in
        /// both versions, fonts, shape and motion.
        #[arg(long)]
        theme: bool,
        /// Write into a directory that isn't empty.
        #[arg(long)]
        force: bool,
        /// Print the manifest instead, to paste somewhere like a gist.
        #[arg(long)]
        print: bool,
        /// Say what was written and left out as JSON.
        #[arg(long)]
        json: bool,
    },
    /// What `share` can bring from this setup, as JSON.
    Parts,
    /// Install a bento, a theme or a plugin: a directory, a git repository
    /// like `github.com/<user>/<repo>`, a gist's URL, or any source
    /// plugins.toml takes.
    Add {
        source: String,
        /// Install a theme or a bento without switching to it.
        #[arg(long)]
        no_use: bool,
        /// Install this commit of a git or registry source, the one `plan`
        /// showed.
        #[arg(long)]
        at: Option<String>,
        /// Don't ask.
        #[arg(long, short)]
        yes: bool,
    },
    /// Switch to a bento, back to your own setup with `mine`, or put a
    /// theme on. Each setup keeps what you change while using it.
    Use { name: String },
    /// Say what `add` would do, as JSON, changing nothing.
    Plan { source: String },
    /// The registry's packages and what Bento installed, as JSON.
    Catalog {
        /// Download the registry's index again, however recent the copy.
        #[arg(long)]
        refresh: bool,
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
    /// Look through the registry: packages whose id, name, description or
    /// tags match, every one without a query.
    Search {
        query: Vec<String>,
        /// Only `plugin`, `theme` or `bento`.
        #[arg(long)]
        kind: Option<String>,
    },
    /// Show a package in the registry: who looks after it, where its code
    /// is, and its releases.
    Info { id: String },
    /// Move plugins to their newest release, and themes and bentos whose
    /// source moved on.
    Update {
        /// Only these; everything from a repository or the registry
        /// without any.
        ids: Vec<String>,
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
    /// Send a release of the package in a directory to the registry: check
    /// it, write its `packages/<id>.toml`, and open the pull request with
    /// `gh`. The release is the commit checked out, pushed to `origin`.
    Publish {
        #[arg(default_value = ".")]
        dir: PathBuf,
        /// The registry's repository, as `owner/name`.
        #[arg(long)]
        registry: Option<String>,
        /// A tag to find it by; repeat for more.
        #[arg(long = "tag")]
        tags: Vec<String>,
        /// Its SPDX license, when the LICENSE file doesn't say.
        #[arg(long)]
        license: Option<String>,
        /// The oldest Mochi a plugin works with; this one without.
        #[arg(long)]
        mochi: Option<String>,
        /// Print the package file instead, to send by hand.
        #[arg(long)]
        print: bool,
        /// Don't ask.
        #[arg(long, short)]
        yes: bool,
    },
    /// Tools for a registry's repository, which its CI runs.
    #[command(subcommand)]
    Registry(RegistryAction),
}

#[derive(Debug, Subcommand)]
pub enum RegistryAction {
    /// Check every `packages/<id>.toml`, and with `--fetch` clone each
    /// package's newest release and read it as `mochi bento add` would.
    Check {
        #[arg(default_value = ".")]
        dir: PathBuf,
        #[arg(long)]
        fetch: bool,
        /// Only these packages, with `--fetch`.
        #[arg(long, value_delimiter = ',')]
        only: Vec<String>,
    },
    /// Write `index.json` from the package files.
    Index {
        #[arg(default_value = ".")]
        dir: PathBuf,
        #[arg(long, default_value = "index.json")]
        out: PathBuf,
    },
    /// Say what changed between two checkouts, and whether a person must
    /// review it.
    Diff {
        base: PathBuf,
        head: PathBuf,
        /// Who opened the pull request: changes to a package they don't
        /// look after need a review.
        #[arg(long)]
        author: Option<String>,
        /// Print JSON, for scripts.
        #[arg(long)]
        json: bool,
    },
}

/// Why Bento refuses while it's off, and how to turn it on.
pub(super) const OFF: &str = "Bento is off. It installs what other people made for Mochi, and a plugin among those runs with your rights, so it could read your files or send them somewhere. The Bento page in the settings explains more and turns it on, or set\n\n  [bento]\n  i_really_understand_that_bento_can_harm_and_contain_malicious_content = true\n\nin config.toml.";

/// Whether the user turned Bento on, in their files or the settings.
pub fn on(config_file: &Path) -> Result<bool, String> {
    let (_, loaded) =
        crate::settings::Store::load(config_file, None).map_err(|error| error.to_string())?;
    Ok(loaded.config.bento.on)
}

pub fn run(action: &Action, config_file: &Path) -> Result<(), String> {
    // What fetches other people's packages waits for consent; listing,
    // removing, sharing and the registry's own tools don't.
    let fetches = matches!(
        action,
        Action::Add { .. }
            | Action::Try { .. }
            | Action::Plan { .. }
            | Action::Search { .. }
            | Action::Info { .. }
            | Action::Update { .. }
    );
    if fetches && !on(config_file)? {
        return Err(OFF.to_owned());
    }
    match action {
        Action::Share {
            dir,
            name,
            only,
            wallpaper,
            theme,
            force,
            print,
            json,
        } => {
            let mut parts = match only {
                Some(only) => share::Parts::only(only)?,
                None => share::Parts::default(),
            };
            parts.wallpaper |= *wallpaper;
            let options = share::Options {
                name: name.clone(),
                parts,
                force: *force,
                print: *print,
                json: *json,
            };
            if *theme {
                share::share_theme(config_file, dir, &options)
            } else {
                share::share(config_file, dir, &options)
            }
        }
        Action::Parts => share::parts(config_file),
        Action::Add {
            source,
            at,
            no_use,
            yes,
        } => apply::add(config_file, source, at.as_deref(), !*no_use, *yes),
        Action::Use { name } => apply::use_it(config_file, name, on(config_file)?),
        Action::Plan { source } => apply::plan(config_file, source),
        Action::Publish {
            dir,
            registry,
            tags,
            license,
            mochi,
            print,
            yes,
        } => publish::publish(
            dir,
            &publish::Options {
                registry: registry.clone(),
                tags: tags.clone(),
                license: license.clone(),
                mochi: mochi.clone(),
                print: *print,
                yes: *yes,
            },
        ),
        Action::Catalog { refresh } => {
            let (_, loaded) = crate::settings::Store::load(config_file, None)
                .map_err(|error| error.to_string())?;
            registry::catalog(
                config_file,
                *refresh,
                loaded.config.bento.on,
                &loaded.theme.preset,
            )
        }
        Action::Try { source } => apply::try_it(config_file, source),
        Action::Remove { id, yes } => apply::remove(config_file, id, *yes),
        Action::List => apply::list(config_file),
        Action::Check { dir } => apply::check_dir(dir),
        Action::Search { query, kind } => {
            registry::search(config_file, &query.join(" "), kind.as_deref())
        }
        Action::Info { id } => registry::info(config_file, id),
        Action::Update { ids, yes } => apply::update(config_file, ids, *yes),
        Action::Registry(RegistryAction::Check { dir, fetch, only }) => {
            registry::check(dir, *fetch, only)
        }
        Action::Registry(RegistryAction::Index { dir, out }) => registry::index(dir, out),
        Action::Registry(RegistryAction::Diff {
            base,
            head,
            author,
            json,
        }) => registry::diff(base, head, author.as_deref(), *json),
    }
}
