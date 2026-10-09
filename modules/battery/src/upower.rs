//! Batteries from UPower over the system bus: the display device, the
//! combined level of every battery as the desktop shows it, and each device
//! UPower lists, the laptop's batteries and the peripherals that report one,
//! like mice, keyboards and controllers.

use std::collections::HashMap;
use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::mpsc::UnboundedSender;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};
use zbus::{Connection, MatchRule, MessageStream};

const SERVICE: &str = "org.freedesktop.UPower";
const PATH: &str = "/org/freedesktop/UPower";
const UPOWER: &str = "org.freedesktop.UPower";
const DISPLAY: &str = "/org/freedesktop/UPower/devices/DisplayDevice";
const DEVICE: &str = "org.freedesktop.UPower.Device";

/// UPower's `Type` for a battery.
pub const BATTERY: u32 = 2;
/// UPower's `Type` for a charger, which has no level.
const LINE_POWER: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum State {
    Charging,
    #[default]
    Discharging,
    /// Plugged in and full, or not charging at a set limit.
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Battery {
    /// 0 to 100.
    pub percent: f64,
    pub state: State,
    /// How long until empty or full, when UPower knows.
    pub time_to_empty: Option<Duration>,
    pub time_to_full: Option<Duration>,
}

impl Battery {
    /// The level as the views show it.
    pub fn level(&self) -> u32 {
        self.percent.clamp(0.0, 100.0).round() as u32
    }
}

/// One of UPower's devices with a battery.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Device {
    /// UPower's object path, which names it while it stays.
    pub path: String,
    /// UPower's `Type`: 2 a battery, 5 a mouse, 12 a controller.
    pub kind: u32,
    /// The kernel's or BlueZ's name for it, like `BAT0` or
    /// `/org/bluez/hci0/dev_AA_BB_CC_DD_EE_FF`.
    pub native_path: String,
    pub model: String,
    /// Often the Bluetooth address, the same for one device UPower finds
    /// twice, through the kernel and through BlueZ.
    pub serial: String,
    /// Powers the computer: the laptop's own batteries.
    pub power_supply: bool,
    pub battery: Battery,
    /// Whether it said how full it is yet; until then UPower gives 0% in an
    /// unknown state.
    pub reported: bool,
}

impl Device {
    /// One of the laptop's own batteries.
    pub fn is_laptop(&self) -> bool {
        self.power_supply && self.kind == BATTERY
    }

    /// A mouse, a keyboard, a controller and such, once it has said how full
    /// it is.
    pub fn is_peripheral(&self) -> bool {
        !self.power_supply && self.kind != LINE_POWER && self.reported
    }

    /// Whether it comes from BlueZ rather than the kernel.
    pub fn is_bluez(&self) -> bool {
        self.native_path.starts_with("/org/bluez/")
    }
}

/// Everything UPower says, at one time.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Snapshot {
    /// The combined level; `None` without a battery, as on a desktop.
    pub display: Option<Battery>,
    /// The laptop's batteries, in the kernel's order.
    pub batteries: Vec<Device>,
    /// The peripherals with a level, each once, by name.
    pub peripherals: Vec<Device>,
}

/// Sends what UPower says now and after every change, until nobody listens.
/// Without UPower, the snapshot is empty.
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
    let mut last: Option<Snapshot> = None;
    loop {
        let snapshot = read(&connection).await.unwrap_or_default();
        if last.as_ref() != Some(&snapshot) {
            if snapshots.send(snapshot.clone()).is_err() {
                return;
            }
            last = Some(snapshot);
        }
        let Some(stream) = signals.as_mut() else {
            return;
        };
        if stream.next().await.is_none() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
        while let Some(Some(_)) = futures_util::FutureExt::now_or_never(stream.next()) {}
        if snapshots.is_closed() {
            return;
        }
    }
}

type Properties = HashMap<String, OwnedValue>;

async fn properties(connection: &Connection, path: &str) -> zbus::Result<Properties> {
    connection
        .call_method(
            Some(SERVICE),
            path,
            Some("org.freedesktop.DBus.Properties"),
            "GetAll",
            &(DEVICE,),
        )
        .await?
        .body()
        .deserialize()
}

pub async fn read(connection: &Connection) -> zbus::Result<Snapshot> {
    let display = properties(connection, DISPLAY).await?;
    let paths: Vec<OwnedObjectPath> = connection
        .call_method(Some(SERVICE), PATH, Some(UPOWER), "EnumerateDevices", &())
        .await?
        .body()
        .deserialize()?;
    let mut devices = Vec::new();
    for path in paths {
        // A device gone between the list and the read is left out.
        if let Ok(found) = properties(connection, path.as_str()).await {
            devices.push((path.to_string(), found));
        }
    }
    Ok(snapshot(&display, devices))
}

/// The snapshot from the properties of the display device and of each
/// device, by path.
pub fn snapshot(display: &Properties, devices: Vec<(String, Properties)>) -> Snapshot {
    // The display device of a desktop has no battery.
    let display = (get(display, "IsPresent").unwrap_or(false)
        && get::<u32>(display, "Type").unwrap_or_default() == BATTERY)
        .then(|| battery(display));
    let mut batteries = Vec::new();
    let mut peripherals: Vec<Device> = Vec::new();
    for (path, properties) in devices {
        let device = device(path, &properties);
        if device.is_laptop() {
            if get(&properties, "IsPresent").unwrap_or(false) {
                batteries.push(device);
            }
            continue;
        }
        if !device.is_peripheral() {
            continue;
        }
        // One device found twice, through the kernel and through BlueZ,
        // shows once, as the kernel's.
        let twin = (!device.serial.is_empty())
            .then(|| {
                peripherals
                    .iter()
                    .position(|other| other.serial.eq_ignore_ascii_case(&device.serial))
            })
            .flatten();
        match twin {
            Some(index) if peripherals[index].is_bluez() && !device.is_bluez() => {
                peripherals[index] = device;
            }
            Some(_) => {}
            None => peripherals.push(device),
        }
    }
    batteries.sort_by(|a, b| a.native_path.cmp(&b.native_path));
    peripherals.sort_by(|a, b| (&a.model, &a.path).cmp(&(&b.model, &b.path)));
    Snapshot {
        display,
        batteries,
        peripherals,
    }
}

fn get<T: TryFrom<OwnedValue>>(properties: &Properties, name: &str) -> Option<T> {
    properties.get(name)?.try_clone().ok()?.try_into().ok()
}

fn device(path: String, properties: &Properties) -> Device {
    let text = |name| {
        get::<String>(properties, name)
            .map(|text| text.trim().to_owned())
            .unwrap_or_default()
    };
    let battery = battery(properties);
    let state: u32 = get(properties, "State").unwrap_or_default();
    Device {
        path,
        kind: get(properties, "Type").unwrap_or_default(),
        native_path: text("NativePath"),
        model: text("Model"),
        serial: text("Serial"),
        power_supply: get(properties, "PowerSupply").unwrap_or(false),
        reported: state != 0 || battery.percent > 0.0,
        battery,
    }
}

fn battery(properties: &Properties) -> Battery {
    let seconds = |name| {
        get::<i64>(properties, name)
            .filter(|seconds| *seconds > 0)
            .map(|seconds| Duration::from_secs(seconds.unsigned_abs()))
    };
    Battery {
        percent: get(properties, "Percentage").unwrap_or_default(),
        state: state(get(properties, "State").unwrap_or_default()),
        time_to_empty: seconds("TimeToEmpty"),
        time_to_full: seconds("TimeToFull"),
    }
}

/// UPower's states: 1 charging, 2 discharging, 3 empty, 4 fully charged,
/// 5 pending charge, 6 pending discharge.
fn state(value: u32) -> State {
    match value {
        1 => State::Charging,
        4 | 5 => State::Full,
        _ => State::Discharging,
    }
}

#[cfg(test)]
mod tests {
    use zbus::zvariant::Value;

    use super::*;

    /// Properties as UPower sends them.
    fn properties(entries: &[(&str, Value<'_>)]) -> Properties {
        entries
            .iter()
            .map(|(name, value)| {
                (
                    (*name).to_owned(),
                    OwnedValue::try_from(value.try_clone().unwrap()).unwrap(),
                )
            })
            .collect()
    }

    fn peripheral(kind: u32, model: &str, native: &str, serial: &str, percent: f64) -> Properties {
        properties(&[
            ("Type", Value::from(kind)),
            ("Model", Value::from(model)),
            ("NativePath", Value::from(native)),
            ("Serial", Value::from(serial)),
            ("PowerSupply", Value::from(false)),
            ("IsPresent", Value::from(true)),
            ("Percentage", Value::from(percent)),
            ("State", Value::from(2u32)),
        ])
    }

    #[test]
    fn reads_upowers_states() {
        assert_eq!(state(1), State::Charging);
        assert_eq!(state(2), State::Discharging);
        assert_eq!(state(4), State::Full);
        assert_eq!(state(6), State::Discharging);
        let battery = Battery {
            percent: 49.6,
            ..Battery::default()
        };
        assert_eq!(battery.level(), 50);
    }

    #[test]
    fn sorts_batteries_from_peripherals() {
        let laptop = |native: &str, percent: f64| {
            properties(&[
                ("Type", Value::from(BATTERY)),
                ("NativePath", Value::from(native)),
                ("Model", Value::from("5B10W13930")),
                ("PowerSupply", Value::from(true)),
                ("IsPresent", Value::from(true)),
                ("Percentage", Value::from(percent)),
                ("State", Value::from(1u32)),
                ("TimeToFull", Value::from(1800i64)),
            ])
        };
        let display = properties(&[
            ("Type", Value::from(BATTERY)),
            ("IsPresent", Value::from(true)),
            ("Percentage", Value::from(61.0)),
            ("State", Value::from(1u32)),
        ]);
        let found = snapshot(
            &display,
            vec![
                ("/ac".into(), properties(&[("Type", Value::from(1u32))])),
                ("/bat1".into(), laptop("BAT1", 40.0)),
                ("/bat0".into(), laptop("BAT0", 82.0)),
                (
                    "/mouse".into(),
                    peripheral(5, "MX Master 3", "hidpp_battery_0", "", 40.0),
                ),
                (
                    "/pad".into(),
                    peripheral(
                        12,
                        "Wireless Controller",
                        "ps-controller-battery-aa",
                        "",
                        70.0,
                    ),
                ),
                // Not heard from yet.
                (
                    "/keyboard".into(),
                    properties(&[
                        ("Type", Value::from(6u32)),
                        ("PowerSupply", Value::from(false)),
                        ("Percentage", Value::from(0.0)),
                        ("State", Value::from(0u32)),
                    ]),
                ),
            ],
        );
        assert_eq!(found.display.unwrap().level(), 61);
        let names: Vec<&str> = found
            .batteries
            .iter()
            .map(|b| b.native_path.as_str())
            .collect();
        assert_eq!(names, ["BAT0", "BAT1"]);
        assert_eq!(found.batteries[0].battery.state, State::Charging);
        assert_eq!(
            found.batteries[0].battery.time_to_full,
            Some(Duration::from_secs(1800))
        );
        let names: Vec<&str> = found.peripherals.iter().map(|d| d.model.as_str()).collect();
        assert_eq!(names, ["MX Master 3", "Wireless Controller"]);
        assert_eq!(found.peripherals[1].kind, 12);
    }

    #[test]
    fn a_device_found_twice_shows_once() {
        let address = "AA:BB:CC:DD:EE:FF";
        let found = snapshot(
            &Properties::new(),
            vec![
                (
                    "/bluez".into(),
                    peripheral(
                        5,
                        "Mouse",
                        "/org/bluez/hci0/dev_AA_BB_CC_DD_EE_FF",
                        address,
                        50.0,
                    ),
                ),
                (
                    "/kernel".into(),
                    peripheral(
                        5,
                        "Mouse",
                        "hid-aa:bb:cc:dd:ee:ff-battery",
                        &address.to_lowercase(),
                        55.0,
                    ),
                ),
                (
                    "/buds".into(),
                    peripheral(17, "Buds", "/org/bluez/hci0/dev_11", "11", 80.0),
                ),
            ],
        );
        assert_eq!(found.display, None);
        let paths: Vec<&str> = found.peripherals.iter().map(|d| d.path.as_str()).collect();
        assert_eq!(paths, ["/buds", "/kernel"]);
    }
}
