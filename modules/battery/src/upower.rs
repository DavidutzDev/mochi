//! The battery, from UPower's display device over the system bus: the
//! combined level of every battery, as the desktop shows it.

use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::mpsc::UnboundedSender;
use zbus::zvariant::OwnedValue;
use zbus::{Connection, MatchRule, MessageStream, Proxy};

const SERVICE: &str = "org.freedesktop.UPower";
const DISPLAY: &str = "/org/freedesktop/UPower/devices/DisplayDevice";
const DEVICE: &str = "org.freedesktop.UPower.Device";

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

/// Sends the battery now and after every change, until nobody listens.
/// `None` means no battery, or UPower isn't running.
pub async fn watch(connection: Connection, batteries: UnboundedSender<Option<Battery>>) {
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
    let mut last: Option<Option<Battery>> = None;
    loop {
        let battery = read(&connection).await.ok().flatten();
        if last != Some(battery) {
            if batteries.send(battery).is_err() {
                return;
            }
            last = Some(battery);
        }
        let Some(stream) = signals.as_mut() else {
            return;
        };
        if stream.next().await.is_none() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
        while let Some(Some(_)) = futures_util::FutureExt::now_or_never(stream.next()) {}
        if batteries.is_closed() {
            return;
        }
    }
}

pub async fn read(connection: &Connection) -> zbus::Result<Option<Battery>> {
    let device = Proxy::new(connection, SERVICE, DISPLAY, DEVICE).await?;
    async fn get<T: TryFrom<OwnedValue>>(device: &Proxy<'_>, name: &str) -> Option<T> {
        device
            .get_property::<OwnedValue>(name)
            .await
            .ok()?
            .try_into()
            .ok()
    }
    let present: bool = get(&device, "IsPresent").await.unwrap_or(false);
    // 2 is a battery; the display device of a desktop has none.
    let kind: u32 = get(&device, "Type").await.unwrap_or_default();
    if !present || kind != 2 {
        return Ok(None);
    }
    let seconds = |value: Option<i64>| {
        value
            .filter(|seconds| *seconds > 0)
            .map(|seconds| Duration::from_secs(seconds.unsigned_abs()))
    };
    Ok(Some(Battery {
        percent: get(&device, "Percentage").await.unwrap_or_default(),
        state: state(get(&device, "State").await.unwrap_or_default()),
        time_to_empty: seconds(get(&device, "TimeToEmpty").await),
        time_to_full: seconds(get(&device, "TimeToFull").await),
    }))
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
    use super::*;

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
}
