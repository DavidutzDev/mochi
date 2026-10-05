//! BlueZ over the system bus: a snapshot of the adapter and the devices,
//! read again after its signals, and the calls that change them.

use std::collections::HashMap;
use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::mpsc::UnboundedSender;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};
use zbus::{Connection, MatchRule, MessageStream, Proxy};

const SERVICE: &str = "org.bluez";
const ADAPTER: &str = "org.bluez.Adapter1";
const DEVICE: &str = "org.bluez.Device1";
const BATTERY: &str = "org.bluez.Battery1";

/// Everything the module shows, as BlueZ last said. `None` from [`read`]
/// when there is no adapter.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Snapshot {
    pub adapter: String,
    pub powered: bool,
    pub discovering: bool,
    pub devices: Vec<Device>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub path: String,
    pub address: String,
    /// The name to show: the alias the user gave, else the device's own.
    pub name: String,
    /// A freedesktop icon name, like `audio-headset` or `input-mouse`.
    pub icon: String,
    pub paired: bool,
    pub connected: bool,
    /// Percent, when the device reports it.
    pub battery: Option<u8>,
}

type Objects = HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedValue>>>;

/// Sends a snapshot now and after every change, until nobody listens.
/// `None` means no adapter, or BlueZ isn't running.
pub async fn watch(connection: Connection, snapshots: UnboundedSender<Option<Snapshot>>) {
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender(SERVICE)
        .map(|rule| rule.build());
    let mut signals = match rule {
        Ok(rule) => MessageStream::for_match_rule(rule, &connection, None)
            .await
            .ok(),
        Err(_) => None,
    };
    // BlueZ starting later only shows as its name appearing.
    let owners = zbus::fdo::DBusProxy::new(&connection).await.ok();
    let mut started = match &owners {
        Some(bus) => bus.receive_name_owner_changed().await.ok(),
        None => None,
    };
    let mut last: Option<Option<Snapshot>> = None;
    loop {
        let snapshot = read(&connection).await.ok().flatten();
        if last.as_ref() != Some(&snapshot) {
            if snapshots.send(snapshot.clone()).is_err() {
                return;
            }
            last = Some(snapshot);
        }
        tokio::select! {
            signal = async {
                match signals.as_mut() {
                    Some(stream) => stream.next().await.map(drop),
                    None => std::future::pending().await,
                }
            } => if signal.is_none() { return },
            owner = async {
                match started.as_mut() {
                    Some(stream) => loop {
                        let signal = stream.next().await?;
                        if signal.args().is_ok_and(|args| args.name() == SERVICE) {
                            return Some(());
                        }
                    },
                    None => std::future::pending().await,
                }
            } => if owner.is_none() { started = None },
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
        if let Some(stream) = signals.as_mut() {
            while let Some(Some(_)) = futures_util::FutureExt::now_or_never(stream.next()) {}
        }
        if snapshots.is_closed() {
            return;
        }
    }
}

/// The first adapter and the devices it knows.
pub async fn read(connection: &Connection) -> zbus::Result<Option<Snapshot>> {
    let manager = zbus::fdo::ObjectManagerProxy::builder(connection)
        .destination(SERVICE)?
        .path("/")?
        .build()
        .await?;
    let objects: Objects = manager
        .get_managed_objects()
        .await?
        .into_iter()
        .map(|(path, interfaces)| {
            let interfaces = interfaces
                .into_iter()
                .map(|(name, properties)| (name.to_string(), properties))
                .collect();
            (path, interfaces)
        })
        .collect();
    Ok(snapshot(&objects))
}

fn snapshot(objects: &Objects) -> Option<Snapshot> {
    let mut adapters: Vec<(&OwnedObjectPath, &HashMap<String, OwnedValue>)> = objects
        .iter()
        .filter_map(|(path, interfaces)| Some((path, interfaces.get(ADAPTER)?)))
        .collect();
    adapters.sort_by_key(|(path, _)| path.as_str());
    let (adapter_path, adapter) = adapters.first()?;
    let flag = |properties: &HashMap<String, OwnedValue>, key: &str| {
        properties
            .get(key)
            .and_then(|value| bool::try_from(value).ok())
            .unwrap_or(false)
    };
    let text = |properties: &HashMap<String, OwnedValue>, key: &str| {
        properties
            .get(key)
            .and_then(|value| String::try_from(value.clone()).ok())
            .unwrap_or_default()
    };
    let mut devices: Vec<Device> = objects
        .iter()
        .filter_map(|(path, interfaces)| {
            let device = interfaces.get(DEVICE)?;
            let owner = device
                .get("Adapter")
                .and_then(|value| OwnedObjectPath::try_from(value.clone()).ok());
            if owner.as_ref() != Some(*adapter_path) {
                return None;
            }
            let address = text(device, "Address");
            let alias = text(device, "Alias");
            let name = if alias.is_empty() {
                text(device, "Name")
            } else {
                alias
            };
            Some(Device {
                path: path.to_string(),
                name: if name.is_empty() {
                    address.clone()
                } else {
                    name
                },
                address,
                icon: text(device, "Icon"),
                paired: flag(device, "Paired"),
                connected: flag(device, "Connected"),
                battery: interfaces
                    .get(BATTERY)
                    .and_then(|battery| battery.get("Percentage"))
                    .and_then(|value| u8::try_from(value).ok()),
            })
        })
        .collect();
    // Connected first, then paired, then by name.
    devices.sort_by(|a, b| {
        (!a.connected, !a.paired, a.name.to_lowercase(), &a.address).cmp(&(
            !b.connected,
            !b.paired,
            b.name.to_lowercase(),
            &b.address,
        ))
    });
    Some(Snapshot {
        adapter: adapter_path.to_string(),
        powered: flag(adapter, "Powered"),
        discovering: flag(adapter, "Discovering"),
        devices,
    })
}

async fn proxy<'a>(
    connection: &'a Connection,
    path: &'a str,
    interface: &'a str,
) -> zbus::Result<Proxy<'a>> {
    Proxy::new(connection, SERVICE, path, interface).await
}

pub async fn set_powered(connection: &Connection, adapter: &str, on: bool) -> zbus::Result<()> {
    proxy(connection, adapter, ADAPTER)
        .await?
        .set_property("Powered", on)
        .await
        .map_err(zbus::Error::from)
}

pub async fn discover(connection: &Connection, adapter: &str, on: bool) -> zbus::Result<()> {
    let adapter = proxy(connection, adapter, ADAPTER).await?;
    adapter
        .call_method(
            if on {
                "StartDiscovery"
            } else {
                "StopDiscovery"
            },
            &(),
        )
        .await?;
    Ok(())
}

pub async fn connect(connection: &Connection, device: &str) -> zbus::Result<()> {
    proxy(connection, device, DEVICE)
        .await?
        .call_method("Connect", &())
        .await?;
    Ok(())
}

pub async fn disconnect(connection: &Connection, device: &str) -> zbus::Result<()> {
    proxy(connection, device, DEVICE)
        .await?
        .call_method("Disconnect", &())
        .await?;
    Ok(())
}

/// Pairs, which runs the agent's questions, then trusts the device so it
/// may connect by itself, and connects it.
pub async fn pair(connection: &Connection, device: &str) -> zbus::Result<()> {
    let proxy = proxy(connection, device, DEVICE).await?;
    proxy.call_method("Pair", &()).await?;
    proxy
        .set_property("Trusted", true)
        .await
        .map_err(zbus::Error::from)?;
    proxy.call_method("Connect", &()).await?;
    Ok(())
}

/// Forgets a device: unpaired, and gone from the list.
pub async fn forget(connection: &Connection, adapter: &str, device: &str) -> zbus::Result<()> {
    let adapter = proxy(connection, adapter, ADAPTER).await?;
    let device = zbus::zvariant::ObjectPath::try_from(device)?;
    adapter.call_method("RemoveDevice", &(device,)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use zbus::zvariant::{ObjectPath, Value};

    use super::*;

    fn owned(value: Value<'_>) -> OwnedValue {
        value.try_to_owned().unwrap()
    }

    fn path(path: &str) -> OwnedObjectPath {
        OwnedObjectPath::try_from(path).unwrap()
    }

    fn device(alias: &str, paired: bool, connected: bool) -> HashMap<String, OwnedValue> {
        HashMap::from([
            ("Alias".to_owned(), owned(Value::from(alias))),
            (
                "Address".to_owned(),
                owned(Value::from("AA:BB:CC:DD:EE:FF")),
            ),
            ("Icon".to_owned(), owned(Value::from("audio-headset"))),
            ("Paired".to_owned(), owned(Value::from(paired))),
            ("Connected".to_owned(), owned(Value::from(connected))),
            (
                "Adapter".to_owned(),
                owned(Value::from(
                    ObjectPath::try_from("/org/bluez/hci0").unwrap(),
                )),
            ),
        ])
    }

    #[test]
    fn reads_the_adapter_and_its_devices() {
        let objects: Objects = HashMap::from([
            (
                path("/org/bluez/hci0"),
                HashMap::from([(
                    ADAPTER.to_owned(),
                    HashMap::from([("Powered".to_owned(), owned(Value::from(true)))]),
                )]),
            ),
            (
                path("/org/bluez/hci0/dev_1"),
                HashMap::from([
                    (DEVICE.to_owned(), device("Mouse", true, false)),
                    (
                        BATTERY.to_owned(),
                        HashMap::from([("Percentage".to_owned(), owned(Value::from(80u8)))]),
                    ),
                ]),
            ),
            (
                path("/org/bluez/hci0/dev_2"),
                HashMap::from([(DEVICE.to_owned(), device("Headphones", true, true))]),
            ),
            (
                path("/org/bluez/hci0/dev_3"),
                HashMap::from([(DEVICE.to_owned(), device("Speaker", false, false))]),
            ),
        ]);
        let found = snapshot(&objects).unwrap();
        assert!(found.powered);
        let names: Vec<&str> = found
            .devices
            .iter()
            .map(|device| device.name.as_str())
            .collect();
        assert_eq!(names, ["Headphones", "Mouse", "Speaker"]);
        assert_eq!(found.devices[1].battery, Some(80));
        assert_eq!(snapshot(&HashMap::new()), None);
    }
}
