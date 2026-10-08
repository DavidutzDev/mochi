//! External monitors over DDC/CI, through `ddcutil`. It needs the `i2c-dev`
//! kernel module and access to `/dev/i2c-*`, which the `i2c` group or
//! ddcutil's udev rule gives.
//!
//! Each command takes from a tenth of a second to a few, so finding the
//! monitors runs in the background, and each monitor has a writer that
//! sets only the latest value asked for: a dragged slider sends many, and
//! the monitor gets the last.

use tokio::process::Command;
use tokio::sync::watch;

/// VCP feature 0x10, the luminance.
const BRIGHTNESS: &str = "10";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// The I2C bus, `/dev/i2c-<bus>`.
    pub bus: u32,
    /// The model, like `DELL U2720Q`.
    pub model: String,
    /// The compositor's name for its output, like `DP-1`, when ddcutil knows
    /// the connector.
    pub output: Option<String>,
    pub value: u32,
    pub max: u32,
}

/// The monitors that answer DDC/CI, with their brightness.
pub async fn detect() -> Result<Vec<Found>, String> {
    let output = run(&["detect", "--brief"]).await?;
    let mut found = Vec::new();
    for monitor in parse_detect(&output) {
        match run(&[
            "--bus",
            &monitor.bus.to_string(),
            "getvcp",
            BRIGHTNESS,
            "--brief",
        ])
        .await
        {
            Ok(reply) => match parse_vcp(&reply) {
                Some((value, max)) => found.push(Found {
                    value,
                    max,
                    ..monitor
                }),
                None => {
                    tracing::debug!(bus = monitor.bus, reply, "no brightness in ddcutil's reply")
                }
            },
            Err(error) => {
                tracing::debug!(bus = monitor.bus, %error, "can't read a monitor's brightness")
            }
        }
    }
    Ok(found)
}

/// Starts the writer for the monitor on `bus`: the value sent last is the
/// one it sets.
pub fn writer(bus: u32, value: u32) -> watch::Sender<u32> {
    let (sender, mut receiver) = watch::channel(value);
    tokio::spawn(async move {
        while receiver.changed().await.is_ok() {
            let value = *receiver.borrow_and_update();
            let args = [
                "--bus",
                &bus.to_string(),
                "--noverify",
                "setvcp",
                BRIGHTNESS,
                &value.to_string(),
            ];
            if let Err(error) = run(&args).await {
                tracing::warn!(bus, %error, "can't set a monitor's brightness");
            }
        }
    });
    sender
}

async fn run(args: &[&str]) -> Result<String, String> {
    let output = Command::new("ddcutil")
        .args(args)
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|error| format!("can't run ddcutil: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let message = if error.trim().is_empty() {
            stdout
        } else {
            error
        };
        Err(format!("ddcutil {}: {}", args.join(" "), message.trim()))
    }
}

/// The displays in `ddcutil detect --brief`, without their brightness yet.
/// Blocks start at an unindented line; only `Display N` ones work, not
/// `Invalid display`. Versions differ on `DRM connector` and
/// `DRM_connector`.
fn parse_detect(text: &str) -> Vec<Found> {
    let mut found = Vec::new();
    let mut current: Option<Found> = None;
    for line in text.lines() {
        if !line.starts_with(char::is_whitespace) {
            found.extend(current.take());
            if line.starts_with("Display ") {
                current = Some(Found {
                    bus: u32::MAX,
                    model: String::new(),
                    output: None,
                    value: 0,
                    max: 0,
                });
            }
            continue;
        }
        let Some(monitor) = current.as_mut() else {
            continue;
        };
        let Some((key, value)) = line.trim().split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "I2C bus" => {
                if let Some(bus) = value
                    .strip_prefix("/dev/i2c-")
                    .and_then(|bus| bus.parse().ok())
                {
                    monitor.bus = bus;
                }
            }
            "DRM connector" | "DRM_connector" => {
                // `card1-DP-1`: the card, then the connector.
                monitor.output = value.split_once('-').map(|(_, output)| output.to_owned());
            }
            "Monitor" => {
                // `maker:model:serial`.
                let mut parts = value.split(':');
                let maker = parts.next().unwrap_or_default().trim();
                let model = parts.next().unwrap_or_default().trim();
                monitor.model = if model.is_empty() { maker } else { model }.to_owned();
            }
            _ => {}
        }
    }
    found.extend(current);
    found.retain(|monitor| monitor.bus != u32::MAX);
    found
}

/// `VCP 10 C 50 100`: a continuous value, now and its top.
fn parse_vcp(text: &str) -> Option<(u32, u32)> {
    let words: Vec<&str> = text.split_whitespace().collect();
    match words.as_slice() {
        ["VCP", _, "C", value, max, ..] => Some((value.parse().ok()?, max.parse().ok()?)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays_come_from_detect() {
        let text = "\
Display 1
   I2C bus:  /dev/i2c-6
   DRM_connector:           card1-DP-1
   Monitor:                 DEL:DELL U2720Q:7XYZ
\x20
Invalid display
   I2C bus:  /dev/i2c-8
   DRM_connector:           card1-eDP-1
   Monitor:                 BOE::
\x20
Display 2
   I2C bus:  /dev/i2c-7
   DRM connector:           card1-HDMI-A-1
   Monitor:                 GSM::
";
        let found = parse_detect(text);
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(found[0].bus, 6);
        assert_eq!(found[0].model, "DELL U2720Q");
        assert_eq!(found[0].output.as_deref(), Some("DP-1"));
        // Without a model, the maker.
        assert_eq!(found[1].model, "GSM");
        assert_eq!(found[1].output.as_deref(), Some("HDMI-A-1"));
    }

    #[test]
    fn brightness_comes_from_getvcp() {
        assert_eq!(parse_vcp("VCP 10 C 50 100\n"), Some((50, 100)));
        assert_eq!(parse_vcp("VCP 10 ERR\n"), None);
        assert_eq!(parse_vcp(""), None);
    }
}
