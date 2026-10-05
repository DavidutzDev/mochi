//! One app's tray icon: what it shows, read from its
//! `org.kde.StatusNotifierItem` object, and the clicks it takes.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use serde_json::{Value, json};
use tokio::sync::mpsc::UnboundedSender;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};
use zbus::{Connection, MatchRule, MessageStream};

use crate::sni::Address;

const INTERFACE: &str = "org.kde.StatusNotifierItem";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Status {
    /// Nothing worth showing now: dimmed in the drawer.
    Passive,
    #[default]
    Active,
    NeedsAttention,
}

/// What an icon shows, as its app last said.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Item {
    /// The app's own name for it, like "discord" or "nm-applet".
    pub id: String,
    pub title: String,
    pub status: Status,
    /// An icon theme name, or a PNG file Mochi wrote from the app's pixels
    /// or found in the app's own icon folder.
    pub icon: String,
    pub attention_icon: String,
    pub tooltip: String,
    /// The DBusMenu object, if the app has a menu.
    pub menu: Option<String>,
    /// Clicking should open the menu: the app does nothing on Activate.
    pub only_menu: bool,
}

impl Item {
    /// What the views get. `key` names the item in actions.
    pub fn to_json(&self, key: &str) -> Value {
        let icon = match self.status {
            Status::NeedsAttention if !self.attention_icon.is_empty() => &self.attention_icon,
            _ => &self.icon,
        };
        json!({
            "key": key,
            "id": self.id,
            "title": self.name(),
            "icon": icon,
            "tooltip": self.tooltip,
            "attention": self.status == Status::NeedsAttention,
            "passive": self.status == Status::Passive,
            "menu": self.menu.is_some(),
        })
    }

    /// What to call it: the title, else the id.
    pub fn name(&self) -> &str {
        if self.title.is_empty() {
            &self.id
        } else {
            &self.title
        }
    }
}

/// What the module hears about an icon.
#[derive(Debug)]
pub struct Update {
    pub address: Address,
    pub item: Item,
}

/// Reads an icon now and after every change its app announces, until the
/// task is stopped. `icons` is where pictures from the app's pixels go.
pub async fn follow(
    connection: Connection,
    address: Address,
    icons: PathBuf,
    updates: UnboundedSender<Update>,
) {
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender(address.bus.as_str())
        .and_then(|rule| rule.path(address.path.as_str()))
        .and_then(|rule| rule.interface(INTERFACE))
        .map(|rule| rule.build());
    let mut signals = match rule {
        Ok(rule) => MessageStream::for_match_rule(rule, &connection, None)
            .await
            .ok(),
        Err(_) => None,
    };
    loop {
        match read(&connection, &address, &icons).await {
            Ok(item) => {
                let update = Update {
                    address: address.clone(),
                    item,
                };
                if updates.send(update).is_err() {
                    return;
                }
            }
            Err(error) => tracing::debug!(%error, bus = address.bus, "can't read a tray icon"),
        }
        let Some(stream) = signals.as_mut() else {
            return;
        };
        if stream.next().await.is_none() {
            return;
        }
        // Apps send a burst of signals for one change: read once after it.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        while let Some(Some(_)) = futures_util::FutureExt::now_or_never(stream.next()) {}
    }
}

async fn read(connection: &Connection, address: &Address, icons: &Path) -> zbus::Result<Item> {
    let properties = zbus::fdo::PropertiesProxy::builder(connection)
        .destination(address.bus.as_str())?
        .path(address.path.as_str())?
        .build()
        .await?;
    let values = properties.get_all(INTERFACE.try_into()?).await?;
    Ok(parse(&values, icons))
}

fn parse(values: &HashMap<String, OwnedValue>, icons: &Path) -> Item {
    let string = |key: &str| {
        values
            .get(key)
            .and_then(|value| String::try_from(value.clone()).ok())
            .unwrap_or_default()
    };
    let theme_path = string("IconThemePath");
    let icon = |name: &str, pixmap: &str| {
        let name = string(name);
        if !name.is_empty() {
            if name.starts_with('/') {
                return name;
            }
            if let Some(file) = in_theme_path(&theme_path, &name) {
                return file.display().to_string();
            }
            return name;
        }
        values
            .get(pixmap)
            .and_then(|value| Vec::<(i32, i32, Vec<u8>)>::try_from(value.clone()).ok())
            .and_then(|pixmaps| write_pixmap(&pixmaps, icons))
            .unwrap_or_default()
    };
    let tooltip = values
        .get("ToolTip")
        .and_then(|value| {
            <(String, Vec<(i32, i32, Vec<u8>)>, String, String)>::try_from(value.clone()).ok()
        })
        .map(|(_, _, title, body)| {
            [title, body]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    Item {
        id: string("Id"),
        title: string("Title"),
        status: match string("Status").as_str() {
            "Passive" => Status::Passive,
            "NeedsAttention" => Status::NeedsAttention,
            _ => Status::Active,
        },
        icon: icon("IconName", "IconPixmap"),
        attention_icon: icon("AttentionIconName", "AttentionIconPixmap"),
        tooltip,
        menu: values
            .get("Menu")
            .and_then(|value| OwnedObjectPath::try_from(value.clone()).ok())
            .map(|path| path.to_string())
            .filter(|path| path != "/"),
        only_menu: values
            .get("ItemIsMenu")
            .and_then(|value| bool::try_from(value).ok())
            .unwrap_or(false),
    }
}

/// An icon in the app's own folder, which some apps ship outside any theme.
fn in_theme_path(folder: &str, name: &str) -> Option<PathBuf> {
    if folder.is_empty() {
        return None;
    }
    let mut stack = vec![(PathBuf::from(folder), 0)];
    while let Some((dir, depth)) = stack.pop() {
        for entry in std::fs::read_dir(&dir).ok()?.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if depth < 4 {
                    stack.push((path, depth + 1));
                }
            } else if path.file_stem().and_then(|stem| stem.to_str()) == Some(name)
                && matches!(
                    path.extension().and_then(|extension| extension.to_str()),
                    Some("png" | "svg")
                )
            {
                return Some(path);
            }
        }
    }
    None
}

/// Writes the largest of an app's pictures as a PNG, named after its
/// pixels so a picture is written once.
fn write_pixmap(pixmaps: &[(i32, i32, Vec<u8>)], folder: &Path) -> Option<String> {
    let (width, height, argb) = pixmaps
        .iter()
        .filter(|(width, height, data)| {
            *width > 0 && *height > 0 && data.len() == (*width as usize) * (*height as usize) * 4
        })
        .max_by_key(|(width, height, _)| width * height)?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (width, height, argb).hash(&mut hasher);
    let path = folder.join(format!("{:016x}.png", hasher.finish()));
    if !path.exists() {
        let rgba = argb_to_rgba(argb);
        if let Err(error) = write_png(&path, *width as u32, *height as u32, &rgba) {
            tracing::debug!(%error, "can't write a tray icon");
            return None;
        }
    }
    Some(path.display().to_string())
}

/// The protocol's pixels are ARGB in network byte order: A, R, G, B.
fn argb_to_rgba(argb: &[u8]) -> Vec<u8> {
    argb.as_chunks::<4>()
        .0
        .iter()
        .flat_map(|[a, r, g, b]| [*r, *g, *b, *a])
        .collect()
}

fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> std::io::Result<()> {
    let file = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(std::io::Error::other)?;
    writer
        .write_image_data(rgba)
        .map_err(std::io::Error::other)?;
    writer.finish().map_err(std::io::Error::other)
}

/// A click or scroll for the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Activate,
    Secondary,
    Scroll { delta: i32, vertical: bool },
}

/// Sends `input` to the app.
pub async fn send(connection: &Connection, address: &Address, input: Input) -> zbus::Result<()> {
    let proxy = zbus::Proxy::new(
        connection,
        address.bus.as_str(),
        address.path.as_str(),
        INTERFACE,
    )
    .await?;
    match input {
        Input::Activate => proxy.call_method("Activate", &(0i32, 0i32)).await?,
        Input::Secondary => {
            proxy
                .call_method("SecondaryActivate", &(0i32, 0i32))
                .await?
        }
        Input::Scroll { delta, vertical } => {
            let orientation = if vertical { "vertical" } else { "horizontal" };
            proxy.call_method("Scroll", &(delta, orientation)).await?
        }
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::zvariant::Value;

    fn owned(value: Value<'_>) -> OwnedValue {
        value.try_to_owned().unwrap()
    }

    #[test]
    fn reads_an_icon() {
        let folder = std::env::temp_dir().join(format!("mochi-tray-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let pixels = vec![(2i32, 1i32, vec![255u8, 10, 20, 30, 128, 40, 50, 60])];
        let values: HashMap<String, OwnedValue> = [
            ("Id", owned(Value::from("discord"))),
            ("Title", owned(Value::from("Discord"))),
            ("Status", owned(Value::from("NeedsAttention"))),
            ("IconName", owned(Value::from(""))),
            ("IconPixmap", owned(Value::from(pixels))),
            ("AttentionIconName", owned(Value::from("discord-unread"))),
            (
                "Menu",
                owned(Value::from(
                    zbus::zvariant::ObjectPath::try_from("/MenuBar").unwrap(),
                )),
            ),
            ("ItemIsMenu", owned(Value::from(false))),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect();

        let item = parse(&values, &folder);
        assert_eq!(item.name(), "Discord");
        assert_eq!(item.status, Status::NeedsAttention);
        assert!(item.icon.ends_with(".png"), "{}", item.icon);
        assert_eq!(item.menu.as_deref(), Some("/MenuBar"));
        // The attention icon shows while the app asks for attention.
        let json = item.to_json("discord");
        assert_eq!(json["icon"], "discord-unread");
        assert_eq!(json["attention"], true);
        std::fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn turns_argb_into_rgba() {
        assert_eq!(argb_to_rgba(&[255, 1, 2, 3]), [1, 2, 3, 255]);
    }

    #[test]
    fn names_fall_back_to_the_id() {
        let item = Item {
            id: "nm-applet".into(),
            ..Item::default()
        };
        assert_eq!(item.name(), "nm-applet");
    }
}
