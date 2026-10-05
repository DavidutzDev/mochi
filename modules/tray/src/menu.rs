//! An app's tray menu, over DBusMenu (`com.canonical.dbusmenu`): the
//! entries, read when the menu opens, and the clicks.

use std::collections::HashMap;

use serde_json::{Value, json};
use zbus::Connection;
use zbus::zvariant::OwnedValue;

const INTERFACE: &str = "com.canonical.dbusmenu";

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Entry {
    pub id: i32,
    pub label: String,
    pub enabled: bool,
    pub separator: bool,
    /// `checkmark`, `radio`, or empty.
    pub toggle: String,
    pub checked: bool,
    /// Opens more entries. Apps may fill them only when it opens.
    pub submenu: bool,
    /// An icon theme name.
    pub icon: String,
    pub children: Vec<Entry>,
}

impl Entry {
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "label": self.label,
            "enabled": self.enabled,
            "separator": self.separator,
            "toggle": self.toggle,
            "checked": self.checked,
            "submenu": self.submenu,
            "icon": self.icon,
            "children": self.children.iter().map(Entry::to_json).collect::<Vec<_>>(),
        })
    }
}

/// The entries under `parent`, 0 for the whole menu, after telling the app
/// it's about to show, so lazy menus fill themselves first.
pub async fn read(
    connection: &Connection,
    bus: &str,
    path: &str,
    parent: i32,
) -> zbus::Result<Vec<Entry>> {
    let proxy = zbus::Proxy::new(connection, bus, path, INTERFACE).await?;
    // Not every app implements it; a failure changes nothing.
    let _ = proxy.call_method("AboutToShow", &(parent,)).await;
    let reply = proxy
        .call_method("GetLayout", &(parent, -1i32, Vec::<&str>::new()))
        .await?;
    type Node = (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>);
    let (_revision, (_, _, children)): (u32, Node) = reply.body().deserialize()?;
    Ok(entries(children))
}

/// Clicks an entry.
pub async fn click(connection: &Connection, bus: &str, path: &str, id: i32) -> zbus::Result<()> {
    let proxy = zbus::Proxy::new(connection, bus, path, INTERFACE).await?;
    let data = zbus::zvariant::Value::from(0i32);
    proxy
        .call_method("Event", &(id, "clicked", data, 0u32))
        .await?;
    Ok(())
}

/// The visible entries of a level of the layout.
fn entries(children: Vec<OwnedValue>) -> Vec<Entry> {
    children
        .into_iter()
        .filter_map(|child| {
            type Node = (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>);
            // Children are variants: `av`. Some arrive still wrapped.
            let child = match &*child {
                zbus::zvariant::Value::Value(inner) => inner.try_to_owned().ok()?,
                _ => child,
            };
            let (id, properties, children) = Node::try_from(child).ok()?;
            entry(id, &properties, children)
        })
        .collect()
}

fn entry(
    id: i32,
    properties: &HashMap<String, OwnedValue>,
    children: Vec<OwnedValue>,
) -> Option<Entry> {
    let string = |key: &str| {
        properties
            .get(key)
            .and_then(|value| String::try_from(value.clone()).ok())
            .unwrap_or_default()
    };
    let flag = |key: &str, default: bool| {
        properties
            .get(key)
            .and_then(|value| bool::try_from(value).ok())
            .unwrap_or(default)
    };
    if !flag("visible", true) {
        return None;
    }
    let children = entries(children);
    Some(Entry {
        id,
        label: mnemonic_free(&string("label")),
        enabled: flag("enabled", true),
        separator: string("type") == "separator",
        toggle: string("toggle-type"),
        checked: properties
            .get("toggle-state")
            .and_then(|value| i32::try_from(value).ok())
            .is_some_and(|state| state == 1),
        submenu: string("children-display") == "submenu" || !children.is_empty(),
        icon: string("icon-name"),
        children,
    })
}

/// Drops the underscores that mark access keys: `_File` is "File", and
/// `__` an underscore.
fn mnemonic_free(label: &str) -> String {
    let mut text = String::with_capacity(label.len());
    let mut chars = label.chars().peekable();
    while let Some(char) = chars.next() {
        if char == '_' {
            if chars.peek() == Some(&'_') {
                text.push('_');
                chars.next();
            }
        } else {
            text.push(char);
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use zbus::zvariant::{Structure, StructureBuilder, Value};

    use super::*;

    fn node(
        id: i32,
        properties: &[(&str, Value<'static>)],
        children: Vec<Value<'static>>,
    ) -> Value<'static> {
        let properties: HashMap<String, Value<'static>> = properties
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect();
        let children: Vec<Value<'static>> = children
            .into_iter()
            .map(|child| Value::Value(Box::new(child)))
            .collect();
        let structure: Structure<'static> = StructureBuilder::new()
            .add_field(id)
            .add_field(properties)
            .add_field(zbus::zvariant::Array::from(children))
            .build()
            .unwrap();
        Value::Structure(structure)
    }

    fn owned(value: Value<'_>) -> OwnedValue {
        value.try_to_owned().unwrap()
    }

    #[test]
    fn reads_a_menu() {
        let open = node(1, &[("label", Value::from("_Open Discord"))], vec![]);
        let line = node(2, &[("type", Value::from("separator"))], vec![]);
        let mute = node(
            3,
            &[
                ("label", Value::from("Mute")),
                ("toggle-type", Value::from("checkmark")),
                ("toggle-state", Value::from(1i32)),
            ],
            vec![],
        );
        let hidden = node(4, &[("visible", Value::from(false))], vec![]);
        let status = node(
            5,
            &[
                ("label", Value::from("Status")),
                ("children-display", Value::from("submenu")),
            ],
            vec![node(
                6,
                &[
                    ("label", Value::from("Idle")),
                    ("enabled", Value::from(false)),
                ],
                vec![],
            )],
        );
        let entries = entries(
            [open, line, mute, hidden, status]
                .into_iter()
                .map(owned)
                .collect(),
        );

        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].label, "Open Discord");
        assert!(entries[1].separator);
        assert!(entries[2].checked && entries[2].toggle == "checkmark");
        assert!(entries[3].submenu);
        assert_eq!(entries[3].children[0].label, "Idle");
        assert!(!entries[3].children[0].enabled);
        assert_eq!(entries[3].to_json()["children"][0]["id"], 6);
    }

    #[test]
    fn drops_access_key_marks() {
        assert_eq!(mnemonic_free("_Quit"), "Quit");
        assert_eq!(mnemonic_free("snake__case"), "snake_case");
        assert_eq!(mnemonic_free("plain"), "plain");
    }
}
