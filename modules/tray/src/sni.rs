//! Which apps have a tray icon, from the StatusNotifierItem protocol over
//! D-Bus.
//!
//! Apps register their icon with the `org.kde.StatusNotifierWatcher`. Mochi
//! serves the watcher itself, and follows each app's bus name so an icon
//! goes when its app quits. Only one program owns the watcher's name: when
//! another tray already does, like Waybar's, Mochi reads the icons from it
//! instead. Either way it registers as a host, the program that shows the
//! icons, which apps check before they make one.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, PoisonError};

use enumflags2::BitFlags;
use futures_util::StreamExt;
use tokio::sync::mpsc::UnboundedSender;
use zbus::fdo::{DBusProxy, RequestNameReply};
use zbus::message::Header;
use zbus::object_server::SignalEmitter;
use zbus::{Connection, interface, proxy};

const WATCHER: &str = "org.kde.StatusNotifierWatcher";
const WATCHER_PATH: &str = "/StatusNotifierWatcher";
/// Where an item lives when it gives only its bus name.
const ITEM_PATH: &str = "/StatusNotifierItem";

/// An icon came or went, by its address: the app's bus name and the object
/// path of its item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Added(Address),
    Removed(Address),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Address {
    pub bus: String,
    pub path: String,
}

impl Address {
    /// From what an app registers: a bus name, an object path on the
    /// sender's connection (as libappindicator sends), or both run
    /// together, as watchers list them.
    pub fn parse(service: &str, sender: Option<&str>) -> Option<Self> {
        if service.starts_with('/') {
            return Some(Self {
                bus: sender?.to_owned(),
                path: service.to_owned(),
            });
        }
        let (bus, path) = match service.find('/') {
            Some(slash) => (&service[..slash], &service[slash..]),
            None => (service, ITEM_PATH),
        };
        (!bus.is_empty()).then(|| Self {
            bus: bus.to_owned(),
            path: path.to_owned(),
        })
    }

    /// How watchers list it: the bus name, then the path.
    pub fn to_service(&self) -> String {
        format!("{}{}", self.bus, self.path)
    }
}

/// The watcher Mochi serves.
#[derive(Debug)]
struct Watcher {
    items: Arc<Mutex<BTreeSet<Address>>>,
    changes: UnboundedSender<Change>,
}

#[interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    async fn register_status_notifier_item(
        &self,
        service: String,
        #[zbus(header)] header: Header<'_>,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) {
        let sender = header.sender().map(|sender| sender.to_string());
        let Some(address) = Address::parse(&service, sender.as_deref()) else {
            tracing::debug!(service, "ignored a tray icon without a bus name");
            return;
        };
        let added = self
            .items
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(address.clone());
        if added {
            let _ = Self::status_notifier_item_registered(&emitter, &address.to_service()).await;
            let _ = self.changes.send(Change::Added(address));
        }
    }

    /// Mochi is the host; others may register too.
    fn register_status_notifier_host(&self, _service: String) {}

    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        self.items
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(Address::to_service)
            .collect()
    }

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        0
    }

    #[zbus(signal)]
    async fn status_notifier_item_registered(
        emitter: &SignalEmitter<'_>,
        service: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn status_notifier_item_unregistered(
        emitter: &SignalEmitter<'_>,
        service: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn status_notifier_host_registered(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}

/// Another program's watcher, when it holds the name.
#[proxy(
    interface = "org.kde.StatusNotifierWatcher",
    default_service = "org.kde.StatusNotifierWatcher",
    default_path = "/StatusNotifierWatcher"
)]
trait OtherWatcher {
    fn register_status_notifier_host(&self, service: &str) -> zbus::Result<()>;

    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> zbus::Result<Vec<String>>;

    #[zbus(signal)]
    fn status_notifier_item_registered(&self, service: String) -> zbus::Result<()>;

    #[zbus(signal)]
    fn status_notifier_item_unregistered(&self, service: String) -> zbus::Result<()>;
}

/// Starts following tray icons, sending every icon there is, then each one
/// that comes or goes, until nobody listens.
pub async fn start(changes: UnboundedSender<Change>) -> zbus::Result<Connection> {
    let items = Arc::new(Mutex::new(BTreeSet::new()));
    let watcher = Watcher {
        items: items.clone(),
        changes: changes.clone(),
    };
    let connection = zbus::connection::Builder::session()?
        .serve_at(WATCHER_PATH, watcher)?
        .build()
        .await?;
    // A name of our own as the host, as the protocol asks.
    let host = format!("org.kde.StatusNotifierHost-{}", std::process::id());
    connection.request_name(host.as_str()).await?;

    let reply = connection
        .request_name_with_flags(WATCHER, BitFlags::empty())
        .await?;
    if matches!(
        reply,
        RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner
    ) {
        tracing::info!("serving the tray");
        let emitter = SignalEmitter::new(&connection, WATCHER_PATH)?;
        let _ = Watcher::status_notifier_host_registered(&emitter).await;
        tokio::spawn(forget_quitters(connection.clone(), items, changes));
    } else {
        // Don't wait in line for the name: two watchers would split the
        // icons between them.
        let _ = connection.release_name(WATCHER).await;
        tracing::info!("another tray serves the icons; showing its icons");
        tokio::spawn(follow_other(connection.clone(), host, changes));
    }
    Ok(connection)
}

/// Drops the icons of apps that quit, telling apps and the module.
async fn forget_quitters(
    connection: Connection,
    items: Arc<Mutex<BTreeSet<Address>>>,
    changes: UnboundedSender<Change>,
) {
    let Ok(bus) = DBusProxy::new(&connection).await else {
        return;
    };
    let Ok(mut owners) = bus.receive_name_owner_changed().await else {
        return;
    };
    while let Some(signal) = owners.next().await {
        let Ok(args) = signal.args() else { continue };
        if args.new_owner().is_some() {
            continue;
        }
        let name = args.name().to_string();
        let gone: Vec<Address> = {
            let mut items = items.lock().unwrap_or_else(PoisonError::into_inner);
            let gone: Vec<Address> = items
                .iter()
                .filter(|address| address.bus == name)
                .cloned()
                .collect();
            for address in &gone {
                items.remove(address);
            }
            gone
        };
        for address in gone {
            if let Ok(emitter) = SignalEmitter::new(&connection, WATCHER_PATH) {
                let _ = Watcher::status_notifier_item_unregistered(&emitter, &address.to_service())
                    .await;
            }
            if changes.send(Change::Removed(address)).is_err() {
                return;
            }
        }
    }
}

/// Follows the icons another watcher collects.
async fn follow_other(connection: Connection, host: String, changes: UnboundedSender<Change>) {
    let result = async {
        let watcher = OtherWatcherProxy::new(&connection).await?;
        let mut added = watcher.receive_status_notifier_item_registered().await?;
        let mut removed = watcher.receive_status_notifier_item_unregistered().await?;
        watcher.register_status_notifier_host(&host).await?;
        for service in watcher.registered_status_notifier_items().await? {
            if let Some(address) = Address::parse(&service, None) {
                let _ = changes.send(Change::Added(address));
            }
        }
        loop {
            tokio::select! {
                Some(signal) = added.next() => {
                    if let Some(address) = signal.args().ok().and_then(|args| Address::parse(&args.service, None)) {
                        let _ = changes.send(Change::Added(address));
                    }
                }
                Some(signal) = removed.next() => {
                    if let Some(address) = signal.args().ok().and_then(|args| Address::parse(&args.service, None)) {
                        let _ = changes.send(Change::Removed(address));
                    }
                }
                else => return zbus::Result::Ok(()),
            }
            if changes.is_closed() {
                return Ok(());
            }
        }
    }
    .await;
    if let Err(error) = result {
        tracing::warn!(%error, "lost the other tray's icons");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_every_way_apps_register() {
        assert_eq!(
            Address::parse("org.kde.StatusNotifierItem-4077-1", Some(":1.80")),
            Some(Address {
                bus: "org.kde.StatusNotifierItem-4077-1".into(),
                path: "/StatusNotifierItem".into(),
            })
        );
        // libappindicator sends a path on its own connection.
        assert_eq!(
            Address::parse("/org/ayatana/NotificationItem/discord", Some(":1.92")),
            Some(Address {
                bus: ":1.92".into(),
                path: "/org/ayatana/NotificationItem/discord".into(),
            })
        );
        assert_eq!(Address::parse("/StatusNotifierItem", None), None);
        // As watchers list them.
        let listed = Address::parse(":1.92/org/ayatana/NotificationItem/discord", None).unwrap();
        assert_eq!(listed.bus, ":1.92");
        assert_eq!(
            listed.to_service(),
            ":1.92/org/ayatana/NotificationItem/discord"
        );
        assert_eq!(Address::parse("", None), None);
    }
}
