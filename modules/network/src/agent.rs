//! NetworkManager's secret agent: when a connection needs a password it
//! doesn't have, like a saved network whose password changed, or one
//! started with nmcli, NetworkManager asks the agents of the user on
//! screen. Mochi asks on the island and answers with what was typed.
//!
//! It answers for Wi-Fi passwords and the password of 802.1X; other
//! secrets, like a VPN plugin's, say there are none, so another agent can
//! answer them.

use std::collections::HashMap;

use tokio::sync::{mpsc, oneshot};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
use zbus::{Connection, interface};

const PATH: &str = "/org/freedesktop/NetworkManager/SecretAgent";
/// How Mochi names itself to NetworkManager.
const IDENTIFIER: &str = "org.mochi.shell";

/// `GetSecrets` flags.
const ALLOW_INTERACTION: u32 = 0x1;
const REQUEST_NEW: u32 = 0x2;

type Settings = HashMap<String, HashMap<String, OwnedValue>>;

/// A password NetworkManager asks for.
#[derive(Debug)]
pub struct Request {
    /// The network's name, or the connection's.
    pub name: String,
    /// For 802.1X, the user name the connection has.
    pub identity: Option<String>,
    /// The last password didn't work.
    pub retry: bool,
    /// The password, or `None` when the user cancelled.
    pub reply: oneshot::Sender<Option<String>>,
}

/// What kind of secret a setting holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Secret {
    Psk,
    Eap,
}

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.freedesktop.NetworkManager.SecretAgent")]
enum AgentError {
    #[zbus(error)]
    ZBus(zbus::Error),
    UserCanceled(String),
    NoSecrets(String),
}

struct Agent {
    requests: mpsc::UnboundedSender<Request>,
}

/// What a request needs from the connection's settings: which secret, the
/// name to show and, for 802.1X, the user name.
fn wanted(connection: &Settings, setting: &str) -> Option<(Secret, String, Option<String>)> {
    let text = |group: &str, key: &str| {
        connection
            .get(group)?
            .get(key)
            .and_then(|value| <&str>::try_from(value).ok().map(str::to_owned))
    };
    let secret = match setting {
        "802-11-wireless-security" => {
            let management = text("802-11-wireless-security", "key-mgmt").unwrap_or_default();
            if !matches!(management.as_str(), "wpa-psk" | "sae") {
                return None;
            }
            Secret::Psk
        }
        "802-1x" => Secret::Eap,
        _ => return None,
    };
    let ssid = connection
        .get("802-11-wireless")
        .and_then(|wireless| wireless.get("ssid"))
        .and_then(|value| <Vec<u8>>::try_from(value.try_clone().ok()?).ok())
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
    let name = ssid
        .or_else(|| text("connection", "id"))
        .unwrap_or_else(|| "the network".to_owned());
    let identity = (secret == Secret::Eap)
        .then(|| text("802-1x", "identity"))
        .flatten();
    Some((secret, name, identity))
}

/// The answer, in the shape NetworkManager reads.
fn answer(secret: Secret, password: &str) -> HashMap<String, HashMap<String, OwnedValue>> {
    let (setting, key) = match secret {
        Secret::Psk => ("802-11-wireless-security", "psk"),
        Secret::Eap => ("802-1x", "password"),
    };
    let value = Value::from(password)
        .try_to_owned()
        .expect("a string has no file descriptors");
    HashMap::from([(setting.to_owned(), HashMap::from([(key.to_owned(), value)]))])
}

#[interface(name = "org.freedesktop.NetworkManager.SecretAgent")]
impl Agent {
    async fn get_secrets(
        &self,
        connection: Settings,
        _connection_path: OwnedObjectPath,
        setting_name: String,
        _hints: Vec<String>,
        flags: u32,
    ) -> Result<HashMap<String, HashMap<String, OwnedValue>>, AgentError> {
        let Some((secret, name, identity)) = wanted(&connection, &setting_name) else {
            return Err(AgentError::NoSecrets(format!(
                "Mochi can't ask for {setting_name}"
            )));
        };
        if flags & ALLOW_INTERACTION == 0 {
            return Err(AgentError::NoSecrets("Mochi keeps no secrets".into()));
        }
        let (reply, answered) = oneshot::channel();
        let request = Request {
            name,
            identity,
            retry: flags & REQUEST_NEW != 0,
            reply,
        };
        self.requests
            .send(request)
            .map_err(|_| AgentError::NoSecrets("the network module stopped".into()))?;
        match answered.await {
            Ok(Some(password)) => Ok(answer(secret, &password)),
            _ => Err(AgentError::UserCanceled("cancelled on the island".into())),
        }
    }

    /// NetworkManager gave up waiting; the prompt closes when its request
    /// is dropped.
    async fn cancel_get_secrets(&self, _connection_path: OwnedObjectPath, _setting_name: String) {}

    /// NetworkManager keeps the secrets in the connection itself.
    async fn save_secrets(&self, _connection: Settings, _connection_path: OwnedObjectPath) {}

    async fn delete_secrets(&self, _connection: Settings, _connection_path: OwnedObjectPath) {}
}

/// Serves the agent on `connection` and registers it with NetworkManager.
/// Requests come to `requests`.
pub async fn start(
    connection: &Connection,
    requests: mpsc::UnboundedSender<Request>,
) -> zbus::Result<()> {
    let agent = Agent { requests };
    connection.object_server().at(PATH, agent).await?;
    register(connection).await
}

/// Registers again, as NetworkManager forgets agents when it restarts.
pub async fn register(connection: &Connection) -> zbus::Result<()> {
    connection
        .call_method(
            Some("org.freedesktop.NetworkManager"),
            "/org/freedesktop/NetworkManager/AgentManager",
            Some("org.freedesktop.NetworkManager.AgentManager"),
            "Register",
            &(IDENTIFIER,),
        )
        .await
        .map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(value: Value<'_>) -> OwnedValue {
        value.try_to_owned().unwrap()
    }

    fn settings(groups: &[(&str, &[(&str, Value<'_>)])]) -> Settings {
        groups
            .iter()
            .map(|(group, keys)| {
                (
                    (*group).to_owned(),
                    keys.iter()
                        .map(|(key, value)| ((*key).to_owned(), owned(value.clone())))
                        .collect(),
                )
            })
            .collect()
    }

    #[test]
    fn asks_for_wifi_and_eap_passwords_only() {
        let home = settings(&[
            ("connection", &[("id", Value::from("Home"))]),
            (
                "802-11-wireless",
                &[("ssid", Value::from(b"Home 5G".to_vec()))],
            ),
            (
                "802-11-wireless-security",
                &[("key-mgmt", Value::from("wpa-psk"))],
            ),
        ]);
        assert_eq!(
            wanted(&home, "802-11-wireless-security"),
            Some((Secret::Psk, "Home 5G".into(), None))
        );
        let work = settings(&[
            ("connection", &[("id", Value::from("Work"))]),
            ("802-1x", &[("identity", Value::from("ada"))]),
        ]);
        assert_eq!(
            wanted(&work, "802-1x"),
            Some((Secret::Eap, "Work".into(), Some("ada".into())))
        );
        // WEP, and a VPN plugin's secrets, are someone else's.
        let wep = settings(&[(
            "802-11-wireless-security",
            &[("key-mgmt", Value::from("none"))],
        )]);
        assert_eq!(wanted(&wep, "802-11-wireless-security"), None);
        assert_eq!(wanted(&home, "vpn"), None);
    }

    #[test]
    fn answers_in_the_setting_asked() {
        let psk = answer(Secret::Psk, "hunter2");
        assert_eq!(
            psk["802-11-wireless-security"]["psk"],
            owned(Value::from("hunter2"))
        );
        let eap = answer(Secret::Eap, "pw");
        assert_eq!(eap["802-1x"]["password"], owned(Value::from("pw")));
    }
}
