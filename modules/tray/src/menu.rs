//! An app's tray menu, over DBusMenu (`com.canonical.dbusmenu`): the
//! entries, read when the menu opens and again whenever the app changes
//! them while it's open, and the clicks.

use std::collections::HashMap;
use std::time::Duration;

use futures_util::StreamExt;
use serde_json::{Value, json};
use tokio::sync::mpsc::UnboundedSender;
use zbus::zvariant::OwnedValue;
use zbus::{Connection, MatchRule, MessageStream};

const INTERFACE: &str = "com.canonical.dbusmenu";
/// The signals that say the entries changed: some came or went, or some
/// changed their label, state or the like.
const CHANGES: [&str; 2] = ["LayoutUpdated", "ItemsPropertiesUpdated"];

/// An entry in the layout: its id, properties and children.
type Node = (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>);

/// A menu read again after its app changed it. `menu` tells the module
/// which open menu it is for.
#[derive(Debug)]
pub struct Relayout {
    pub menu: u64,
    pub entries: Vec<Entry>,
}

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
    layout(&proxy, parent).await
}

/// The entries under `parent` as the app has them now.
async fn layout(proxy: &zbus::Proxy<'_>, parent: i32) -> zbus::Result<Vec<Entry>> {
    let reply = proxy
        .call_method("GetLayout", &(parent, -1i32, Vec::<&str>::new()))
        .await?;
    let (_revision, (_, _, children)): (u32, Node) = reply.body().deserialize()?;
    Ok(entries(children))
}

/// Reads the whole menu again after every change its app announces, and
/// sends it as menu `menu`, until the task is stopped. It never calls
/// `AboutToShow`: apps may answer that with `LayoutUpdated`, and the two
/// would go back and forth for good.
pub async fn follow(
    connection: Connection,
    bus: String,
    path: String,
    menu: u64,
    relayouts: UnboundedSender<Relayout>,
) {
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender(bus.as_str())
        .and_then(|rule| rule.path(path.as_str()))
        .and_then(|rule| rule.interface(INTERFACE))
        .map(|rule| rule.build());
    let stream = match rule {
        Ok(rule) => MessageStream::for_match_rule(rule, &connection, None).await,
        Err(error) => Err(error),
    };
    let proxy = zbus::Proxy::new(&connection, bus.as_str(), path.as_str(), INTERFACE).await;
    let (mut stream, proxy) = match (stream, proxy) {
        (Ok(stream), Ok(proxy)) => (stream, proxy),
        (Err(error), _) | (_, Err(error)) => {
            tracing::debug!(%error, bus, "can't follow a tray menu");
            return;
        }
    };
    let changed = |message: &zbus::Message| {
        message
            .header()
            .member()
            .is_some_and(|member| CHANGES.contains(&member.as_str()))
    };
    while let Some(message) = stream.next().await {
        if !message.as_ref().is_ok_and(changed) {
            continue;
        }
        // Apps send a burst of signals for one change: read once after it.
        tokio::time::sleep(Duration::from_millis(50)).await;
        while let Some(Some(_)) = futures_util::FutureExt::now_or_never(stream.next()) {}
        match layout(&proxy, 0).await {
            Ok(entries) => {
                if relayouts.send(Relayout { menu, entries }).is_err() {
                    return;
                }
            }
            Err(error) => tracing::debug!(%error, bus, "can't read a tray menu again"),
        }
    }
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

    /// A bus of the test's own, from a config that starts no services.
    struct Bus {
        dir: std::path::PathBuf,
        daemon: std::process::Child,
        address: String,
    }

    impl Bus {
        /// `None` where there's no `dbus-daemon`, like a build sandbox.
        fn start() -> Option<Self> {
            if !mochi_core::process::installed("dbus-daemon") {
                return None;
            }
            let dir = std::env::temp_dir().join(format!("mochi-tray-bus-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let socket = dir.join("bus");
            let config = dir.join("bus.conf");
            std::fs::write(
                &config,
                format!(
                    "<busconfig>\n<type>session</type>\n<listen>unix:path={}</listen>\n\
                     <policy context=\"default\">\n<allow send_destination=\"*\" eavesdrop=\"true\"/>\n\
                     <allow eavesdrop=\"true\"/>\n<allow own=\"*\"/>\n</policy>\n</busconfig>\n",
                    socket.display()
                ),
            )
            .unwrap();
            let daemon = std::process::Command::new("dbus-daemon")
                .arg(format!("--config-file={}", config.display()))
                .arg("--nofork")
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap();
            let started = std::time::Instant::now();
            while !socket.exists() && started.elapsed() < Duration::from_secs(5) {
                std::thread::sleep(Duration::from_millis(20));
            }
            Some(Self {
                address: format!("unix:path={}", socket.display()),
                dir,
                daemon,
            })
        }

        async fn connect(&self) -> Connection {
            zbus::connection::Builder::address(self.address.as_str())
                .unwrap()
                .build()
                .await
                .unwrap()
        }
    }

    impl Drop for Bus {
        fn drop(&mut self) {
            let _ = self.daemon.kill();
            let _ = self.daemon.wait();
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// An app's menu with one entry, whose label the test changes.
    struct Menu {
        label: std::sync::Arc<std::sync::Mutex<String>>,
        about_to_show: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    #[zbus::interface(name = "com.canonical.dbusmenu")]
    impl Menu {
        fn about_to_show(&self, _id: i32) -> bool {
            self.about_to_show
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            false
        }

        fn get_layout(&self, _parent: i32, _depth: i32, _properties: Vec<String>) -> (u32, Node) {
            let label = self.label.lock().unwrap().clone();
            let child = owned(node(1, &[("label", Value::from(label))], vec![]));
            (1, (0, HashMap::new(), vec![child]))
        }

        #[zbus(signal)]
        async fn layout_updated(
            emitter: &zbus::object_server::SignalEmitter<'_>,
            revision: u32,
            parent: i32,
        ) -> zbus::Result<()>;

        #[zbus(signal)]
        async fn items_properties_updated(
            emitter: &zbus::object_server::SignalEmitter<'_>,
            updated: Vec<(i32, HashMap<String, OwnedValue>)>,
            removed: Vec<(i32, Vec<String>)>,
        ) -> zbus::Result<()>;
    }

    /// Sends `signal` until the menu is read again: the task subscribes on
    /// its own time, so the first ones may go unheard.
    async fn heard<F: Future<Output = zbus::Result<()>>>(
        relayouts: &mut tokio::sync::mpsc::UnboundedReceiver<Relayout>,
        signal: impl Fn() -> F,
    ) -> Relayout {
        for _ in 0..50 {
            signal().await.unwrap();
            if let Ok(Some(relayout)) =
                tokio::time::timeout(Duration::from_millis(200), relayouts.recv()).await
            {
                return relayout;
            }
        }
        panic!("the menu was never read again");
    }

    #[tokio::test]
    async fn follows_the_menu_as_the_app_changes_it() {
        let Some(bus) = Bus::start() else {
            return;
        };
        let label = std::sync::Arc::new(std::sync::Mutex::new("Mute".to_owned()));
        let about_to_show = std::sync::Arc::default();
        let app = bus.connect().await;
        app.object_server()
            .at(
                "/MenuBar",
                Menu {
                    label: label.clone(),
                    about_to_show: std::sync::Arc::clone(&about_to_show),
                },
            )
            .await
            .unwrap();
        let emitter = zbus::object_server::SignalEmitter::new(&app, "/MenuBar").unwrap();
        let mochi = bus.connect().await;
        let (sender, mut relayouts) = tokio::sync::mpsc::unbounded_channel();
        let task = tokio::spawn(follow(
            mochi,
            app.unique_name().unwrap().to_string(),
            "/MenuBar".into(),
            7,
            sender,
        ));

        *label.lock().unwrap() = "Unmute".into();
        let relayout = heard(&mut relayouts, || Menu::layout_updated(&emitter, 2, 0)).await;
        assert_eq!(relayout.menu, 7);
        assert_eq!(relayout.entries[0].label, "Unmute");

        while relayouts.try_recv().is_ok() {}
        *label.lock().unwrap() = "Mute".into();
        let relayout = heard(&mut relayouts, || {
            Menu::items_properties_updated(&emitter, vec![], vec![])
        })
        .await;
        assert_eq!(relayout.entries[0].label, "Mute");
        // Reading again never asks the app to get ready to show.
        assert_eq!(about_to_show.load(std::sync::atomic::Ordering::SeqCst), 0);
        task.abort();
    }

    #[test]
    fn drops_access_key_marks() {
        assert_eq!(mnemonic_free("_Quit"), "Quit");
        assert_eq!(mnemonic_free("snake__case"), "snake_case");
        assert_eq!(mnemonic_free("plain"), "plain");
    }
}
