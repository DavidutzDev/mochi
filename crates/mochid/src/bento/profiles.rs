//! Setups to switch between: your own, and each bento installed. Each
//! keeps its own `changes.toml` and `widgets.toml`, in
//! `$XDG_DATA_HOME/mochi/bentos/<id>/state/`, and yours in `.mine/state/`.
//! Switching saves the setup you leave and loads the other, so what you
//! change while using a bento stays with it. A bento used for the first
//! time starts from your own setup, with its settings laid over it, its
//! widgets in place of yours and its wallpaper.

use std::path::{Path, PathBuf};

use mochi_core::changes::Changes;
use mochi_module_widgets::layout::Layout;

use super::apply::Context;
use super::manifest::Bento;
use super::{client, screens};
use crate::settings::Op;

/// Your own setup's folder, among the bentos'.
const MINE: &str = ".mine";

/// Where a setup keeps its things: a bento's folder, or yours.
pub fn folder(setup: Option<&str>) -> Result<PathBuf, String> {
    mochi_core::config::data_dir()
        .map(|data| data.join("bentos").join(setup.unwrap_or(MINE)))
        .ok_or_else(|| "no data directory: set HOME or XDG_DATA_HOME".into())
}

/// The copy of a bento kept when it was installed.
pub fn stored(id: &str) -> Result<PathBuf, String> {
    folder(Some(id)).map(|folder| folder.join("bento"))
}

/// The files a setup is made of.
fn files(context: &Context) -> [PathBuf; 2] {
    [
        Changes::path(&context.config_file),
        context
            .config_file
            .with_file_name(mochi_module_widgets::FILE),
    ]
}

/// Keeps the files in place as `setup`'s; for your own setup, the
/// wallpaper too, to set it again when you come back.
fn save(context: &Context, setup: Option<&str>) -> Result<(), String> {
    let state = folder(setup)?.join("state");
    let io = |error: std::io::Error| format!("cannot keep {}: {error}", state.display());
    let _ = std::fs::remove_dir_all(&state);
    std::fs::create_dir_all(&state).map_err(io)?;
    for file in files(context) {
        if file.exists() {
            let name = file.file_name().expect("a file name");
            std::fs::copy(&file, state.join(name)).map_err(io)?;
        }
    }
    if setup.is_none()
        && let Ok(mochi_core::palette::Source::Image(path)) = mochi_core::palette::source("auto")
    {
        std::fs::write(state.join("wallpaper"), path.to_string_lossy().as_bytes()).map_err(io)?;
    }
    Ok(())
}

/// Puts `setup`'s kept files in place. `false` when it has none yet.
fn restore(context: &Context, setup: Option<&str>) -> Result<bool, String> {
    let state = folder(setup)?.join("state");
    if !state.is_dir() {
        return Ok(false);
    }
    for file in files(context) {
        let kept = state.join(file.file_name().expect("a file name"));
        let result = if kept.exists() {
            std::fs::copy(&kept, &file).map(drop)
        } else {
            match std::fs::remove_file(&file) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
                _ => Ok(()),
            }
        };
        result.map_err(|error| format!("cannot put back {}: {error}", file.display()))?;
    }
    Ok(true)
}

/// Forgets what a bento's setup became, so its next use starts fresh.
pub fn forget(id: &str) -> Result<(), String> {
    let state = folder(Some(id))?.join("state");
    match std::fs::remove_dir_all(&state) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            Err(format!("cannot remove {}: {error}", state.display()))
        }
        _ => Ok(()),
    }
}

/// Lays a stored bento over the files in place: its settings over the
/// changes, its widgets in place of theirs, and its wallpaper.
pub fn lay(context: &Context, id: &str) -> Result<(), String> {
    let dir = stored(id)?;
    let bento = Bento::load(&dir)?;
    let mut store = context.store()?;
    store.change(&Op::Merge(Changes {
        config: bento.config.clone(),
        theme: bento.theme.clone(),
    }))?;
    if !bento.widgets.is_empty() {
        let screens = screens::connected();
        let mut layout = Layout::default();
        for widget in &bento.widgets {
            let mut widget = widget.clone();
            if let Some(name) =
                screens::parse_role(&widget.output).and_then(|index| screens.get(index))
            {
                widget.output = name.clone();
            }
            layout.widgets.push(widget);
        }
        let file = context
            .config_file
            .with_file_name(mochi_module_widgets::FILE);
        layout
            .save(&file)
            .map_err(|error| format!("cannot write {}: {error}", file.display()))?;
    }
    wallpaper(Some(id))?;
    Ok(())
}

/// Sets a setup's wallpaper: a bento's own, or the one you had.
fn wallpaper(setup: Option<&str>) -> Result<(), String> {
    let image = match setup {
        Some(id) => Bento::load(&stored(id)?)?
            .bento
            .wallpaper
            .map(|file| stored(id).map(|dir| dir.join(file)))
            .transpose()?,
        None => std::fs::read_to_string(folder(None)?.join("state").join("wallpaper"))
            .ok()
            .map(PathBuf::from),
    };
    if let Some(image) = image.filter(|image| image.is_file()) {
        super::apply::set_wallpaper(&image);
    }
    Ok(())
}

/// Switches to a bento, or back to your own setup with `None`.
pub fn switch(context: &Context, target: Option<&str>) -> Result<(), String> {
    let installed = context.installed()?;
    let current = installed.active.clone();
    if current.as_deref() == target {
        return Ok(());
    }
    if let Some(id) = target
        && !installed.bentos.contains_key(id)
    {
        return Err(format!("no bento called {id} is installed"));
    }
    // Whether Bento is on belongs to no setup: it stays as it is.
    let changes_file = Changes::path(&context.config_file);
    let consent = ["bento", mochi_core::config::BENTO_CONSENT];
    let carried = Changes::load(&changes_file)
        .ok()
        .and_then(|changes| mochi_core::changes::get(&changes.config, &consent).cloned());
    save(context, current.as_deref())?;
    let result = (|| -> Result<(), String> {
        if restore(context, target)? {
            wallpaper(target)
        } else if let Some(id) = target {
            // Its first use: your own setup, with it laid over.
            restore(context, None)?;
            lay(context, id)
        } else {
            Ok(())
        }
    })();
    if let Err(error) = result {
        // Back to the setup that was in use.
        let _ = restore(context, current.as_deref());
        return Err(error);
    }
    let mut changes = Changes::load(&changes_file).map_err(|error| error.to_string())?;
    match carried {
        Some(value) => mochi_core::changes::set(&mut changes.config, &consent, value),
        None => mochi_core::changes::remove(&mut changes.config, &consent),
    }
    changes
        .save(&changes_file)
        .map_err(|error| format!("cannot write {}: {error}", changes_file.display()))?;
    let mut installed = context.installed()?;
    installed.active = target.map(str::to_owned);
    context.save(&installed)?;
    client::reload();
    Ok(())
}

/// What a name means to `use`: your own setup, or a bento's id.
pub fn setup_of(name: &str) -> Option<&str> {
    (!matches!(name, "mine" | "yours" | "own")).then_some(name)
}

/// Copies a fetched bento where switching finds it again, without git's
/// files.
pub fn keep(from: &Path, id: &str) -> Result<(), String> {
    let to = stored(id)?;
    let _ = std::fs::remove_dir_all(&to);
    copy_dir(from, &to)
        .map_err(|error| format!("cannot keep the bento in {}: {error}", to.display()))
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".git" {
            continue;
        }
        let kind = entry.file_type()?;
        if kind.is_dir() {
            copy_dir(&entry.path(), &to.join(&name))?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), to.join(&name))?;
        }
    }
    Ok(())
}
