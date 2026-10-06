//! logind and the power profiles service, on the system bus.

use std::collections::HashMap;

use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
use zbus::{Connection, proxy};

#[proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1",
    gen_blocking = false
)]
pub trait Manager {
    fn can_power_off(&self) -> zbus::Result<String>;
    fn can_reboot(&self) -> zbus::Result<String>;
    fn can_suspend(&self) -> zbus::Result<String>;
    fn can_hibernate(&self) -> zbus::Result<String>;
    fn can_reboot_to_firmware_setup(&self) -> zbus::Result<String>;
    fn power_off(&self, interactive: bool) -> zbus::Result<()>;
    fn reboot(&self, interactive: bool) -> zbus::Result<()>;
    fn suspend(&self, interactive: bool) -> zbus::Result<()>;
    fn hibernate(&self, interactive: bool) -> zbus::Result<()>;
    fn set_reboot_to_firmware_setup(&self, enable: bool) -> zbus::Result<()>;
}

/// The calling user: `self` resolves to whoever asks.
#[proxy(
    interface = "org.freedesktop.login1.User",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1/user/self",
    gen_blocking = false
)]
trait User {
    /// The user's graphical session, the one on screen.
    #[zbus(property)]
    fn display(&self) -> zbus::Result<(String, OwnedObjectPath)>;
}

#[proxy(
    interface = "org.freedesktop.login1.Session",
    default_service = "org.freedesktop.login1",
    gen_blocking = false
)]
pub trait Session {
    fn lock(&self) -> zbus::Result<()>;
    fn terminate(&self) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.freedesktop.UPower.PowerProfiles",
    default_service = "org.freedesktop.UPower.PowerProfiles",
    default_path = "/org/freedesktop/UPower/PowerProfiles",
    gen_blocking = false
)]
pub trait Profiles {
    #[zbus(property)]
    fn active_profile(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn set_active_profile(&self, profile: &str) -> zbus::Result<()>;
    #[zbus(property)]
    fn profiles(&self) -> zbus::Result<Vec<HashMap<String, OwnedValue>>>;
}

/// power-profiles-daemon and its names, under the name it has had since 0.20
/// or, for older versions, under `net.hadess.PowerProfiles`. `None` when
/// neither answers.
pub async fn profiles(system: &Connection) -> Option<(ProfilesProxy<'static>, Vec<String>)> {
    let mut last = None;
    for proxy in [ProfilesProxy::new(system).await, older(system).await] {
        match proxy {
            Ok(proxy) => match proxy.profiles().await {
                Ok(list) => {
                    let names = profile_names(&list);
                    return Some((proxy, names));
                }
                Err(error) => last = Some(error),
            },
            Err(error) => last = Some(error),
        }
    }
    if let Some(error) = last {
        tracing::info!(%error, "no power profiles");
    }
    None
}

/// power-profiles-daemon under its name before 0.20.
async fn older(system: &Connection) -> zbus::Result<ProfilesProxy<'static>> {
    ProfilesProxy::builder(system)
        .destination("net.hadess.PowerProfiles")?
        .path("/net/hadess/PowerProfiles")?
        .interface("net.hadess.PowerProfiles")?
        .build()
        .await
}

/// What logind lets this user do, read once at startup.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Abilities {
    pub shutdown: bool,
    pub reboot: bool,
    pub suspend: bool,
    pub hibernate: bool,
    pub firmware: bool,
}

impl Abilities {
    pub async fn read(manager: &ManagerProxy<'_>) -> Self {
        Self {
            shutdown: allowed(manager.can_power_off().await),
            reboot: allowed(manager.can_reboot().await),
            suspend: allowed(manager.can_suspend().await),
            hibernate: allowed(manager.can_hibernate().await),
            firmware: allowed(manager.can_reboot_to_firmware_setup().await),
        }
    }
}

/// `yes`, or `challenge`: allowed after a password prompt. `no` and `na`
/// hide the button.
fn allowed(answer: zbus::Result<String>) -> bool {
    matches!(answer.as_deref(), Ok("yes" | "challenge"))
}

pub async fn manager(system: &Connection) -> zbus::Result<ManagerProxy<'static>> {
    ManagerProxy::new(system).await
}

/// The user's session on screen. `XDG_SESSION_ID` is often unset in a
/// daemon, so ask logind which session is the user's display.
pub async fn session(system: &Connection) -> zbus::Result<SessionProxy<'static>> {
    let (_, path) = UserProxy::new(system).await?.display().await?;
    SessionProxy::builder(system).path(path)?.build().await
}

/// The profile names on offer, in the service's order.
pub fn profile_names(profiles: &[HashMap<String, OwnedValue>]) -> Vec<String> {
    profiles
        .iter()
        .filter_map(
            |profile| match profile.get("Profile").map(|value| &**value) {
                Some(Value::Str(name)) => Some(name.to_string()),
                _ => None,
            },
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reads, never sets: `cargo test -p mochi-module-power -- --ignored`
    /// on a system whose power-profiles-daemon still answers to the old name.
    #[tokio::test]
    #[ignore = "needs power-profiles-daemon on the system bus"]
    async fn the_older_name_answers() {
        let system = Connection::system().await.unwrap();
        let proxy = older(&system).await.unwrap();
        let names = profile_names(&proxy.profiles().await.unwrap());
        assert!(names.contains(&"balanced".to_owned()), "{names:?}");
        assert!(!proxy.active_profile().await.unwrap().is_empty());
    }

    #[test]
    fn only_yes_and_challenge_show_a_button() {
        assert!(allowed(Ok("yes".into())));
        assert!(allowed(Ok("challenge".into())));
        assert!(!allowed(Ok("no".into())));
        assert!(!allowed(Ok("na".into())));
        assert!(!allowed(Err(zbus::Error::Failure("no logind".into()))));
    }

    #[test]
    fn reads_profile_names() {
        let profile = |name: &str| {
            HashMap::from([
                (
                    "Profile".to_owned(),
                    Value::from(name).try_to_owned().unwrap(),
                ),
                (
                    "Driver".to_owned(),
                    Value::from("platform").try_to_owned().unwrap(),
                ),
            ])
        };
        let profiles = vec![profile("power-saver"), profile("balanced"), HashMap::new()];
        assert_eq!(profile_names(&profiles), ["power-saver", "balanced"]);
    }
}
