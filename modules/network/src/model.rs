//! What the views show, from a [`Snapshot`]: the connection's status, the
//! Wi-Fi networks by name, the wired devices and the VPNs, and the notices
//! a change deserves.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::nm::{ACTIVATED, DEVICE_ACTIVATED, Snapshot};

/// How the connection looks: an icon and a few words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub icon: &'static str,
    pub label: String,
    /// The VPN on, if any.
    pub vpn: Option<String>,
}

/// One Wi-Fi network, by name: the strongest of its access points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Network {
    pub ssid: String,
    pub strength: u8,
    pub secure: bool,
    pub enterprise: bool,
    pub saved: bool,
    pub connected: bool,
}

fn is_vpn(kind: &str) -> bool {
    matches!(kind, "vpn" | "wireguard")
}

/// The Wi-Fi network a device is on.
fn connected_ssid(snapshot: &Snapshot) -> Option<(&str, u8)> {
    snapshot
        .devices
        .iter()
        .filter(|device| device.wifi && device.state == DEVICE_ACTIVATED)
        .find_map(|device| {
            let point = device.access_point.as_ref()?;
            snapshot
                .access_points
                .iter()
                .find(|candidate| &candidate.path == point)
                .map(|point| (point.ssid.as_str(), point.strength))
        })
}

pub fn wifi_icon(strength: u8) -> &'static str {
    match strength {
        0..=34 => "wifi-1",
        35..=69 => "wifi-2",
        _ => "wifi",
    }
}

pub fn status(snapshot: &Snapshot) -> Status {
    let vpn = snapshot
        .active
        .iter()
        .find(|active| is_vpn(&active.kind) && active.state == ACTIVATED)
        .map(|active| active.id.clone());
    let primary = snapshot
        .primary
        .as_ref()
        .and_then(|primary| {
            snapshot
                .active
                .iter()
                .find(|active| &active.path == primary)
        })
        .filter(|active| !is_vpn(&active.kind));
    let (icon, label) = match primary {
        Some(active) if active.kind == "802-11-wireless" => match connected_ssid(snapshot) {
            Some((ssid, strength)) => (wifi_icon(strength), ssid.to_owned()),
            None => ("wifi", active.id.clone()),
        },
        Some(active) => ("ethernet", active.id.clone()),
        None if snapshot
            .active
            .iter()
            .any(|active| active.state < ACTIVATED) =>
        {
            ("wifi-1", "Connecting…".to_owned())
        }
        None if !snapshot.wireless_enabled && snapshot.devices.iter().any(|device| device.wifi) => {
            ("wifi-off", "Wi-Fi off".to_owned())
        }
        None => ("offline", "Offline".to_owned()),
    };
    Status { icon, label, vpn }
}

/// The Wi-Fi networks in range, by name: the one in use first, then the
/// saved ones, then by strength.
pub fn networks(snapshot: &Snapshot) -> Vec<Network> {
    let connected = connected_ssid(snapshot).map(|(ssid, _)| ssid.to_owned());
    let mut by_name: BTreeMap<&str, Network> = BTreeMap::new();
    for point in &snapshot.access_points {
        let network = by_name.entry(&point.ssid).or_insert_with(|| Network {
            ssid: point.ssid.clone(),
            strength: 0,
            secure: false,
            enterprise: false,
            saved: snapshot
                .saved
                .iter()
                .any(|saved| saved.ssid.as_deref() == Some(point.ssid.as_str())),
            connected: connected.as_deref() == Some(point.ssid.as_str()),
        });
        network.strength = network.strength.max(point.strength);
        network.secure |= point.secure;
        network.enterprise |= point.enterprise;
    }
    let mut networks: Vec<Network> = by_name.into_values().collect();
    networks.sort_by(|a, b| {
        (b.connected, b.saved, b.strength, &a.ssid).cmp(&(
            a.connected,
            a.saved,
            a.strength,
            &b.ssid,
        ))
    });
    networks
}

/// What the views get.
pub fn payload(snapshot: Option<&Snapshot>, airplane: bool) -> Value {
    let Some(snapshot) = snapshot else {
        return json!({ "available": false });
    };
    let status = status(snapshot);
    let wifi = snapshot.devices.iter().any(|device| device.wifi);
    let networks: Vec<Value> = networks(snapshot)
        .into_iter()
        .map(|network| {
            json!({
                "ssid": network.ssid,
                "strength": network.strength,
                "icon": wifi_icon(network.strength),
                "secure": network.secure,
                "enterprise": network.enterprise,
                "saved": network.saved,
                "connected": network.connected,
            })
        })
        .collect();
    let wired: Vec<Value> = snapshot
        .devices
        .iter()
        .filter(|device| !device.wifi)
        .map(|device| {
            let name = device
                .active
                .as_ref()
                .and_then(|active| {
                    snapshot
                        .active
                        .iter()
                        .find(|candidate| &candidate.path == active)
                })
                .map(|active| active.id.clone());
            json!({
                "interface": device.interface,
                "connected": device.state == DEVICE_ACTIVATED,
                "connection": name,
            })
        })
        .collect();
    let vpns: Vec<Value> = snapshot
        .saved
        .iter()
        .filter(|saved| saved.is_vpn())
        .map(|saved| {
            let state = snapshot
                .active
                .iter()
                .find(|active| active.connection == saved.path)
                .map(|active| active.state);
            json!({
                "id": saved.id,
                "active": state == Some(ACTIVATED),
                "connecting": state.is_some_and(|state| state < ACTIVATED),
            })
        })
        .collect();
    json!({
        "available": true,
        "status": { "icon": status.icon, "label": status.label, "vpn": status.vpn },
        "wifi": {
            "available": wifi,
            "enabled": snapshot.wireless_enabled,
            "hardware": snapshot.wireless_hardware,
        },
        "airplane": airplane,
        "networks": networks,
        "wired": wired,
        "vpns": vpns,
    })
}

/// A short line for the island about what changed, with its icon.
pub fn notices(before: &Snapshot, after: &Snapshot) -> Vec<(&'static str, String)> {
    let mut notices = Vec::new();
    let (old, new) = (status(before), status(after));
    let online = |status: &Status| {
        !matches!(status.icon, "offline" | "wifi-off") && status.label != "Connecting…"
    };
    if online(&new) && (!online(&old) || old.label != new.label) {
        notices.push((new.icon, format!("Connected to {}", new.label)));
    } else if online(&old) && !online(&new) {
        notices.push((new.icon, format!("Disconnected from {}", old.label)));
    }
    match (old.vpn, new.vpn) {
        (None, Some(vpn)) => notices.push(("lock", format!("{vpn} is on"))),
        (Some(vpn), None) => notices.push(("lock", format!("{vpn} is off"))),
        _ => {}
    }
    notices
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nm::{AccessPoint, Active, Device, Saved};

    fn point(path: &str, ssid: &str, strength: u8, secure: bool) -> AccessPoint {
        AccessPoint {
            path: path.into(),
            device: "/d/wifi".into(),
            ssid: ssid.into(),
            strength,
            secure,
            enterprise: false,
            sae: false,
        }
    }

    fn on_wifi() -> Snapshot {
        Snapshot {
            wireless_enabled: true,
            wireless_hardware: true,
            devices: vec![
                Device {
                    path: "/d/wifi".into(),
                    interface: "wlan0".into(),
                    wifi: true,
                    state: DEVICE_ACTIVATED,
                    active: Some("/a/1".into()),
                    access_point: Some("/ap/2".into()),
                },
                Device {
                    path: "/d/eth".into(),
                    interface: "enp4s0".into(),
                    wifi: false,
                    state: 30,
                    active: None,
                    access_point: None,
                },
            ],
            access_points: vec![
                point("/ap/1", "Cafe", 80, false),
                point("/ap/2", "Home", 40, true),
                point("/ap/3", "Home", 60, true),
                point("/ap/4", "Neighbour", 90, true),
            ],
            active: vec![Active {
                path: "/a/1".into(),
                id: "Home".into(),
                kind: "802-11-wireless".into(),
                state: ACTIVATED,
                connection: "/s/home".into(),
            }],
            saved: vec![
                Saved {
                    path: "/s/home".into(),
                    id: "Home".into(),
                    kind: "802-11-wireless".into(),
                    ssid: Some("Home".into()),
                },
                Saved {
                    path: "/s/work".into(),
                    id: "Work VPN".into(),
                    kind: "wireguard".into(),
                    ssid: None,
                },
            ],
            primary: Some("/a/1".into()),
            ..Snapshot::default()
        }
    }

    #[test]
    fn shows_the_wifi_in_use() {
        let status = status(&on_wifi());
        assert_eq!(status.label, "Home");
        assert_eq!(status.icon, "wifi-2");
        assert_eq!(status.vpn, None);
    }

    #[test]
    fn lists_networks_once_each_in_use_first() {
        let networks = networks(&on_wifi());
        let names: Vec<&str> = networks
            .iter()
            .map(|network| network.ssid.as_str())
            .collect();
        assert_eq!(names, ["Home", "Neighbour", "Cafe"]);
        assert!(networks[0].connected && networks[0].saved);
        // The strongest access point of a name counts.
        assert_eq!(networks[0].strength, 60);
        assert!(!networks[2].secure);
    }

    #[test]
    fn says_when_offline_or_off() {
        let mut snapshot = on_wifi();
        snapshot.primary = None;
        snapshot.active.clear();
        assert_eq!(status(&snapshot).icon, "offline");
        snapshot.wireless_enabled = false;
        assert_eq!(status(&snapshot).label, "Wi-Fi off");
    }

    #[test]
    fn tells_what_changed() {
        let before = on_wifi();
        let mut after = before.clone();
        after.primary = None;
        after.active.clear();
        assert_eq!(
            notices(&before, &after),
            [("offline", "Disconnected from Home".to_owned())]
        );
        assert_eq!(
            notices(&after, &before),
            [("wifi-2", "Connected to Home".to_owned())]
        );
        assert!(notices(&before, &before).is_empty());

        let mut vpn = before.clone();
        vpn.active.push(Active {
            path: "/a/2".into(),
            id: "Work VPN".into(),
            kind: "wireguard".into(),
            state: ACTIVATED,
            connection: "/s/work".into(),
        });
        assert_eq!(
            notices(&before, &vpn),
            [("lock", "Work VPN is on".to_owned())]
        );
        let payload = payload(Some(&vpn), false);
        assert_eq!(payload["vpns"][0]["active"], true);
        assert_eq!(payload["wired"][0]["interface"], "enp4s0");
        assert_eq!(payload["networks"][0]["icon"], "wifi-2");
    }
}
