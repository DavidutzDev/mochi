//! What the views show, from a [`Snapshot`], and the notices a change
//! deserves.

use serde_json::{Value, json};

use crate::bluez::{Device, Snapshot};

fn device_json(device: &Device) -> Value {
    json!({
        "name": device.name,
        "address": device.address,
        "icon": device.icon,
        "paired": device.paired,
        "connected": device.connected,
        "battery": device.battery,
    })
}

/// What the views get. `scanning` is Mochi's scan, which may differ from
/// BlueZ's `Discovering` while another program scans.
pub fn payload(snapshot: Option<&Snapshot>, scanning: bool) -> Value {
    let Some(snapshot) = snapshot else {
        return json!({ "available": false });
    };
    let paired: Vec<Value> = snapshot
        .devices
        .iter()
        .filter(|device| device.paired)
        .map(device_json)
        .collect();
    // Devices in range that aren't paired, with a name worth showing.
    let found: Vec<Value> = snapshot
        .devices
        .iter()
        .filter(|device| !device.paired && device.name != device.address)
        .map(device_json)
        .collect();
    let connected: Vec<&Device> = snapshot
        .devices
        .iter()
        .filter(|device| device.connected)
        .collect();
    json!({
        "available": true,
        "powered": snapshot.powered,
        "scanning": scanning || snapshot.discovering,
        "paired": paired,
        "found": found,
        "connected": connected.iter().map(|device| device.name.clone()).collect::<Vec<_>>(),
    })
}

/// The bubble's payload, while something is connected.
pub fn bubble(snapshot: &Snapshot) -> Option<Value> {
    let connected: Vec<&Device> = snapshot
        .devices
        .iter()
        .filter(|device| device.connected)
        .collect();
    let first = connected.first()?;
    Some(json!({
        "count": connected.len(),
        "name": first.name,
        "icon": first.icon,
        "battery": first.battery,
    }))
}

/// A line for the island about each device that came or went.
pub fn notices(before: &Snapshot, after: &Snapshot) -> Vec<String> {
    let mut notices = Vec::new();
    for device in after.devices.iter().filter(|device| device.paired) {
        let was = before
            .devices
            .iter()
            .find(|old| old.path == device.path)
            .is_some_and(|old| old.connected);
        if device.connected && !was {
            match device.battery {
                Some(battery) => notices.push(format!("{} connected · {battery}%", device.name)),
                None => notices.push(format!("{} connected", device.name)),
            }
        } else if !device.connected && was {
            notices.push(format!("{} disconnected", device.name));
        }
    }
    notices
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(name: &str, paired: bool, connected: bool) -> Device {
        Device {
            path: format!("/dev/{name}"),
            address: format!("{name}-address"),
            name: name.into(),
            icon: "audio-headset".into(),
            paired,
            connected,
            battery: None,
        }
    }

    fn snapshot(devices: Vec<Device>) -> Snapshot {
        Snapshot {
            adapter: "/org/bluez/hci0".into(),
            powered: true,
            discovering: false,
            devices,
        }
    }

    #[test]
    fn splits_paired_devices_from_found_ones() {
        let mut nameless = device("x", false, false);
        nameless.name = nameless.address.clone();
        let snapshot = snapshot(vec![
            device("Buds", true, true),
            device("Speaker", false, false),
            nameless,
        ]);
        let payload = payload(Some(&snapshot), true);
        assert_eq!(payload["paired"][0]["name"], "Buds");
        assert_eq!(payload["found"].as_array().map(Vec::len), Some(1));
        assert_eq!(payload["scanning"], true);
        assert_eq!(bubble(&snapshot).unwrap()["name"], "Buds");
        assert_eq!(super::payload(None, false)["available"], false);
    }

    #[test]
    fn tells_who_came_and_went() {
        let before = snapshot(vec![
            device("Buds", true, false),
            device("Mouse", true, true),
        ]);
        let mut buds = device("Buds", true, true);
        buds.battery = Some(70);
        let after = snapshot(vec![buds, device("Mouse", true, false)]);
        assert_eq!(
            notices(&before, &after),
            ["Buds connected · 70%", "Mouse disconnected"]
        );
        assert!(bubble(&before).is_some());
        assert!(notices(&after, &after).is_empty());
    }
}
