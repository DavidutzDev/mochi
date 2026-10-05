//! NetworkManager over the system bus: a snapshot of the devices, Wi-Fi
//! networks and connections, read again after its signals, and the requests
//! that change them.

use std::collections::HashMap;
use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::mpsc::UnboundedSender;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
use zbus::{Connection, MatchRule, MessageStream, Proxy};

const SERVICE: &str = "org.freedesktop.NetworkManager";
const PATH: &str = "/org/freedesktop/NetworkManager";
const SETTINGS: &str = "/org/freedesktop/NetworkManager/Settings";
const DEVICE: &str = "org.freedesktop.NetworkManager.Device";
const WIRELESS: &str = "org.freedesktop.NetworkManager.Device.Wireless";
const ACCESS_POINT: &str = "org.freedesktop.NetworkManager.AccessPoint";
const ACTIVE: &str = "org.freedesktop.NetworkManager.Connection.Active";
const CONNECTION: &str = "org.freedesktop.NetworkManager.Settings.Connection";

/// NetworkManager's device types worth showing.
const ETHERNET: u32 = 1;
const WIFI: u32 = 2;
/// A device that's up and connected.
pub const DEVICE_ACTIVATED: u32 = 100;
/// An active connection's states.
pub const ACTIVATED: u32 = 2;
pub const DEACTIVATED: u32 = 4;

/// Everything the module shows, as NetworkManager last said.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Snapshot {
    pub wireless_enabled: bool,
    /// The Wi-Fi radio isn't switched off by a key or a switch.
    pub wireless_hardware: bool,
    pub wwan_enabled: bool,
    pub devices: Vec<Device>,
    pub access_points: Vec<AccessPoint>,
    pub active: Vec<Active>,
    pub saved: Vec<Saved>,
    /// The active connection that carries the default route.
    pub primary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub path: String,
    pub interface: String,
    pub wifi: bool,
    pub state: u32,
    /// Its active connection's path.
    pub active: Option<String>,
    /// For Wi-Fi, the access point it's on.
    pub access_point: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessPoint {
    pub path: String,
    /// The Wi-Fi device that sees it.
    pub device: String,
    pub ssid: String,
    /// Percent.
    pub strength: u8,
    pub secure: bool,
    /// 802.1X, with a user name: Mochi can't ask for that.
    pub enterprise: bool,
    /// WPA3: SAE instead of a pre-shared key.
    pub sae: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Active {
    pub path: String,
    pub id: String,
    /// `802-11-wireless`, `802-3-ethernet`, `vpn`, `wireguard`, …
    pub kind: String,
    pub state: u32,
    /// The saved connection it runs.
    pub connection: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saved {
    pub path: String,
    pub id: String,
    pub kind: String,
    /// For Wi-Fi, the network's name.
    pub ssid: Option<String>,
}

impl Saved {
    pub fn is_vpn(&self) -> bool {
        matches!(self.kind.as_str(), "vpn" | "wireguard")
    }
}

/// Sends a snapshot now and after every change, until nobody listens.
pub async fn watch(connection: Connection, snapshots: UnboundedSender<Snapshot>) {
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
    let mut last = None;
    loop {
        match read(&connection).await {
            Ok(snapshot) => {
                if last.as_ref() != Some(&snapshot) {
                    if snapshots.send(snapshot.clone()).is_err() {
                        return;
                    }
                    last = Some(snapshot);
                }
            }
            Err(error) => tracing::debug!(%error, "can't read NetworkManager"),
        }
        let Some(stream) = signals.as_mut() else {
            return;
        };
        if stream.next().await.is_none() {
            return;
        }
        // Signal storms, like a scan's: read once after them.
        tokio::time::sleep(Duration::from_millis(300)).await;
        while let Some(Some(_)) = futures_util::FutureExt::now_or_never(stream.next()) {}
        if snapshots.is_closed() {
            return;
        }
    }
}

async fn proxy<'a>(
    connection: &'a Connection,
    path: &'a str,
    interface: &'a str,
) -> zbus::Result<Proxy<'a>> {
    Proxy::new(connection, SERVICE, path, interface).await
}

async fn get<T>(proxy: &Proxy<'_>, name: &str) -> Option<T>
where
    T: TryFrom<OwnedValue>,
{
    proxy
        .get_property::<OwnedValue>(name)
        .await
        .ok()?
        .try_into()
        .ok()
}

fn path(value: Option<OwnedObjectPath>) -> Option<String> {
    value
        .map(|path| path.to_string())
        .filter(|path| path != "/")
}

pub async fn read(connection: &Connection) -> zbus::Result<Snapshot> {
    let manager = proxy(connection, PATH, SERVICE).await?;
    let mut snapshot = Snapshot {
        wireless_enabled: get(&manager, "WirelessEnabled").await.unwrap_or(false),
        wireless_hardware: get(&manager, "WirelessHardwareEnabled")
            .await
            .unwrap_or(false),
        wwan_enabled: get(&manager, "WwanEnabled").await.unwrap_or(false),
        primary: path(get(&manager, "PrimaryConnection").await),
        ..Snapshot::default()
    };

    let devices: Vec<OwnedObjectPath> = manager.call("GetDevices", &()).await?;
    for device_path in devices {
        let device_path = device_path.to_string();
        let device = proxy(connection, &device_path, DEVICE).await?;
        let kind: u32 = get(&device, "DeviceType").await.unwrap_or_default();
        let managed: bool = get(&device, "Managed").await.unwrap_or(false);
        if !managed || !(kind == ETHERNET || kind == WIFI) {
            continue;
        }
        let mut entry = Device {
            interface: get(&device, "Interface").await.unwrap_or_default(),
            wifi: kind == WIFI,
            state: get(&device, "State").await.unwrap_or_default(),
            active: path(get(&device, "ActiveConnection").await),
            access_point: None,
            path: device_path.clone(),
        };
        if entry.wifi {
            let wireless = proxy(connection, &device_path, WIRELESS).await?;
            entry.access_point = path(get(&wireless, "ActiveAccessPoint").await);
            let points: Vec<OwnedObjectPath> =
                get(&wireless, "AccessPoints").await.unwrap_or_default();
            for point in points {
                if let Some(point) = access_point(connection, point.as_str(), &device_path).await {
                    snapshot.access_points.push(point);
                }
            }
        }
        snapshot.devices.push(entry);
    }

    let active: Vec<OwnedObjectPath> = get(&manager, "ActiveConnections").await.unwrap_or_default();
    for active_path in active {
        let active_path = active_path.to_string();
        let proxy = proxy(connection, &active_path, ACTIVE).await?;
        snapshot.active.push(Active {
            id: get(&proxy, "Id").await.unwrap_or_default(),
            kind: get(&proxy, "Type").await.unwrap_or_default(),
            state: get(&proxy, "State").await.unwrap_or_default(),
            connection: path(get(&proxy, "Connection").await).unwrap_or_default(),
            path: active_path,
        });
    }

    let settings = proxy(
        connection,
        SETTINGS,
        "org.freedesktop.NetworkManager.Settings",
    )
    .await?;
    let saved: Vec<OwnedObjectPath> = settings.call("ListConnections", &()).await?;
    for saved_path in saved {
        let saved_path = saved_path.to_string();
        let proxy = proxy(connection, &saved_path, CONNECTION).await?;
        let Ok(values) = proxy
            .call::<_, _, HashMap<String, HashMap<String, OwnedValue>>>("GetSettings", &())
            .await
        else {
            continue;
        };
        if let Some(saved) = saved_connection(&saved_path, &values) {
            snapshot.saved.push(saved);
        }
    }
    Ok(snapshot)
}

async fn access_point(connection: &Connection, point: &str, device: &str) -> Option<AccessPoint> {
    let proxy = proxy(connection, point, ACCESS_POINT).await.ok()?;
    let ssid: Vec<u8> = get(&proxy, "Ssid").await?;
    let ssid = String::from_utf8_lossy(&ssid).trim().to_owned();
    if ssid.is_empty() {
        return None;
    }
    let flags: u32 = get(&proxy, "Flags").await.unwrap_or_default();
    let wpa: u32 = get(&proxy, "WpaFlags").await.unwrap_or_default();
    let rsn: u32 = get(&proxy, "RsnFlags").await.unwrap_or_default();
    Some(AccessPoint {
        path: point.to_owned(),
        device: device.to_owned(),
        ssid,
        strength: get(&proxy, "Strength").await.unwrap_or_default(),
        secure: flags & 1 != 0 || wpa != 0 || rsn != 0,
        // NM_802_11_AP_SEC_KEY_MGMT_802_1X and _SAE.
        enterprise: (wpa | rsn) & 0x200 != 0,
        sae: rsn & 0x400 != 0 && rsn & 0x100 == 0,
    })
}

fn saved_connection(
    path: &str,
    values: &HashMap<String, HashMap<String, OwnedValue>>,
) -> Option<Saved> {
    let section = values.get("connection")?;
    let text = |value: Option<&OwnedValue>| {
        value
            .and_then(|value| String::try_from(value.clone()).ok())
            .unwrap_or_default()
    };
    let ssid = values
        .get("802-11-wireless")
        .and_then(|wifi| wifi.get("ssid"))
        .and_then(|ssid| Vec::<u8>::try_from(ssid.clone()).ok())
        .map(|ssid| String::from_utf8_lossy(&ssid).into_owned());
    Some(Saved {
        path: path.to_owned(),
        id: text(section.get("id")),
        kind: text(section.get("type")),
        ssid,
    })
}

/// Turns Wi-Fi, or the mobile radio, on or off.
pub async fn set_radio(connection: &Connection, wifi: bool, on: bool) -> zbus::Result<()> {
    let manager = proxy(connection, PATH, SERVICE).await?;
    let name = if wifi {
        "WirelessEnabled"
    } else {
        "WwanEnabled"
    };
    manager
        .set_property(name, on)
        .await
        .map_err(zbus::Error::from)
}

/// Asks a Wi-Fi device to look for networks.
pub async fn scan(connection: &Connection, device: &str) -> zbus::Result<()> {
    let wireless = proxy(connection, device, WIRELESS).await?;
    let options: HashMap<&str, Value<'_>> = HashMap::new();
    wireless.call_method("RequestScan", &(options,)).await?;
    Ok(())
}

/// Starts a saved connection, on `device` or wherever it fits. Returns the
/// active connection.
pub async fn activate(
    connection: &Connection,
    saved: &str,
    device: Option<&str>,
) -> zbus::Result<String> {
    let manager = proxy(connection, PATH, SERVICE).await?;
    let saved = zbus::zvariant::ObjectPath::try_from(saved)?;
    let device = zbus::zvariant::ObjectPath::try_from(device.unwrap_or("/"))?;
    let root = zbus::zvariant::ObjectPath::try_from("/")?;
    let active: OwnedObjectPath = manager
        .call("ActivateConnection", &(saved, device, root))
        .await?;
    Ok(active.to_string())
}

/// Joins a Wi-Fi network for the first time, with its password when it has
/// one. NetworkManager saves it. Returns the saved connection and the active
/// one.
pub async fn join(
    connection: &Connection,
    point: &AccessPoint,
    password: Option<&str>,
) -> zbus::Result<(String, String)> {
    let manager = proxy(connection, PATH, SERVICE).await?;
    let mut settings: HashMap<&str, HashMap<&str, Value<'_>>> = HashMap::new();
    settings.insert(
        "802-11-wireless",
        HashMap::from([
            ("ssid", Value::from(point.ssid.as_bytes().to_vec())),
            ("mode", Value::from("infrastructure")),
        ]),
    );
    if let Some(password) = password {
        let key_mgmt = if point.sae { "sae" } else { "wpa-psk" };
        settings.insert(
            "802-11-wireless-security",
            HashMap::from([
                ("key-mgmt", Value::from(key_mgmt)),
                ("psk", Value::from(password.to_owned())),
            ]),
        );
    }
    let device = zbus::zvariant::ObjectPath::try_from(point.device.as_str())?;
    let point_path = zbus::zvariant::ObjectPath::try_from(point.path.as_str())?;
    let (saved, active): (OwnedObjectPath, OwnedObjectPath) = manager
        .call("AddAndActivateConnection", &(settings, device, point_path))
        .await?;
    Ok((saved.to_string(), active.to_string()))
}

/// Stops an active connection.
pub async fn deactivate(connection: &Connection, active: &str) -> zbus::Result<()> {
    let manager = proxy(connection, PATH, SERVICE).await?;
    let active = zbus::zvariant::ObjectPath::try_from(active)?;
    manager
        .call_method("DeactivateConnection", &(active,))
        .await?;
    Ok(())
}

/// Disconnects a device, which then stays off until asked again.
pub async fn disconnect(connection: &Connection, device: &str) -> zbus::Result<()> {
    let device = proxy(connection, device, DEVICE).await?;
    device.call_method("Disconnect", &()).await?;
    Ok(())
}

/// Deletes a saved connection, like a Wi-Fi network to forget.
pub async fn forget(connection: &Connection, saved: &str) -> zbus::Result<()> {
    let saved = proxy(connection, saved, CONNECTION).await?;
    saved.call_method("Delete", &()).await?;
    Ok(())
}

/// Waits for an active connection to come up or fail, for at most `limit`.
/// `true` when it's up.
pub async fn settled(connection: &Connection, active: &str, limit: Duration) -> bool {
    let started = tokio::time::Instant::now();
    while started.elapsed() < limit {
        let state: Option<u32> = match proxy(connection, active, ACTIVE).await {
            Ok(proxy) => get(&proxy, "State").await,
            Err(_) => None,
        };
        match state {
            Some(ACTIVATED) => return true,
            // Gone, or given up.
            Some(DEACTIVATED) | None => return false,
            Some(_) => tokio::time::sleep(Duration::from_millis(500)).await,
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(value: Value<'_>) -> OwnedValue {
        value.try_to_owned().unwrap()
    }

    #[test]
    fn reads_saved_connections() {
        let wifi: HashMap<String, HashMap<String, OwnedValue>> = HashMap::from([
            (
                "connection".to_owned(),
                HashMap::from([
                    ("id".to_owned(), owned(Value::from("Home"))),
                    ("type".to_owned(), owned(Value::from("802-11-wireless"))),
                ]),
            ),
            (
                "802-11-wireless".to_owned(),
                HashMap::from([("ssid".to_owned(), owned(Value::from(b"Home 5G".to_vec())))]),
            ),
        ]);
        let saved = saved_connection("/s/1", &wifi).unwrap();
        assert_eq!(saved.id, "Home");
        assert_eq!(saved.ssid.as_deref(), Some("Home 5G"));
        assert!(!saved.is_vpn());

        let vpn: HashMap<String, HashMap<String, OwnedValue>> = HashMap::from([(
            "connection".to_owned(),
            HashMap::from([
                ("id".to_owned(), owned(Value::from("Work"))),
                ("type".to_owned(), owned(Value::from("wireguard"))),
            ]),
        )]);
        assert!(saved_connection("/s/2", &vpn).unwrap().is_vpn());
        assert!(saved_connection("/s/3", &HashMap::new()).is_none());
    }
}
