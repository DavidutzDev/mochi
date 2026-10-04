//! The key for the history on disk, kept in the Secret Service:
//! gnome-keyring, KeePassXC or anything else that implements it. The key is
//! made on first use and stored as an item with the attributes
//! `application = mochi` and `purpose = clipboard`.
//!
//! The session uses the `plain` algorithm: the key crosses the session bus
//! unencrypted, which only the user's own processes can listen to, and those
//! could read mochid's memory anyway.

use std::collections::HashMap;

use futures_util::StreamExt;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};
use zbus::{Connection, proxy};
use zeroize::Zeroizing;

use crate::store::Key;

/// `(session, parameters, value, content type)`.
type Secret = (OwnedObjectPath, Vec<u8>, Vec<u8>, String);

#[proxy(
    interface = "org.freedesktop.Secret.Service",
    default_service = "org.freedesktop.secrets",
    default_path = "/org/freedesktop/secrets"
)]
trait Service {
    fn open_session(
        &self,
        algorithm: &str,
        input: &Value<'_>,
    ) -> zbus::Result<(OwnedValue, OwnedObjectPath)>;

    fn search_items(
        &self,
        attributes: HashMap<&str, &str>,
    ) -> zbus::Result<(Vec<OwnedObjectPath>, Vec<OwnedObjectPath>)>;

    fn unlock(
        &self,
        objects: &[ObjectPath<'_>],
    ) -> zbus::Result<(Vec<OwnedObjectPath>, OwnedObjectPath)>;

    fn get_secrets(
        &self,
        items: &[ObjectPath<'_>],
        session: &ObjectPath<'_>,
    ) -> zbus::Result<HashMap<OwnedObjectPath, Secret>>;

    fn read_alias(&self, name: &str) -> zbus::Result<OwnedObjectPath>;
}

#[proxy(
    interface = "org.freedesktop.Secret.Collection",
    default_service = "org.freedesktop.secrets"
)]
trait Collection {
    fn create_item(
        &self,
        properties: HashMap<&str, Value<'_>>,
        secret: &Secret,
        replace: bool,
    ) -> zbus::Result<(OwnedObjectPath, OwnedObjectPath)>;
}

#[proxy(
    interface = "org.freedesktop.Secret.Prompt",
    default_service = "org.freedesktop.secrets"
)]
trait Prompt {
    fn prompt(&self, window_id: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn completed(&self, dismissed: bool, result: Value<'_>) -> zbus::Result<()>;
}

/// The history's key: the one stored, or a new one stored now. Asks the
/// user to unlock the keyring when it's locked.
pub async fn key() -> Result<Key, String> {
    let connection = Connection::session()
        .await
        .map_err(|error| format!("cannot reach the session bus: {error}"))?;
    let service = ServiceProxy::new(&connection)
        .await
        .map_err(|error| format!("no Secret Service: {error}"))?;
    let (_, session) = service
        .open_session("plain", &Value::from(""))
        .await
        .map_err(|error| format!("no Secret Service: {error}"))?;

    let attributes = HashMap::from([("application", "mochi"), ("purpose", "clipboard")]);
    let (unlocked, locked) = service
        .search_items(attributes.clone())
        .await
        .map_err(|error| format!("cannot search the keyring: {error}"))?;
    let item = match (unlocked.first(), locked.first()) {
        (Some(item), _) => Some(item.clone()),
        (None, Some(item)) => {
            unlock(&connection, &service, item).await?;
            Some(item.clone())
        }
        (None, None) => None,
    };

    if let Some(item) = item {
        let mut secrets = service
            .get_secrets(&[item.as_ref()], &session.as_ref())
            .await
            .map_err(|error| format!("cannot read the key: {error}"))?;
        let (_, _, value, _) = secrets.remove(&item).ok_or("the keyring returned no key")?;
        let value = Zeroizing::new(value);
        let key: [u8; 32] = value
            .as_slice()
            .try_into()
            .map_err(|_| "the stored key isn't 32 bytes".to_owned())?;
        return Ok(Zeroizing::new(key));
    }

    // None yet: make one, and keep it in the default collection.
    let mut key = Zeroizing::new([0; 32]);
    getrandom::fill(key.as_mut()).map_err(|error| format!("no randomness: {error}"))?;
    let collection = service
        .read_alias("default")
        .await
        .map_err(|error| format!("cannot find the default keyring: {error}"))?;
    if collection.as_str() == "/" {
        return Err("the Secret Service has no default keyring".into());
    }
    unlock(&connection, &service, &collection).await?;
    let collection = CollectionProxy::builder(&connection)
        .path(collection)
        .map_err(|error| error.to_string())?
        .build()
        .await
        .map_err(|error| error.to_string())?;
    let properties = HashMap::from([
        (
            "org.freedesktop.Secret.Item.Label",
            Value::from("Mochi clipboard history key"),
        ),
        (
            "org.freedesktop.Secret.Item.Attributes",
            Value::from(attributes),
        ),
    ]);
    let secret: Secret = (
        session,
        Vec::new(),
        key.to_vec(),
        "application/octet-stream".into(),
    );
    let (_, prompt) = collection
        .create_item(properties, &secret, false)
        .await
        .map_err(|error| format!("cannot store the key: {error}"))?;
    let (_, _, mut value, _) = secret;
    zeroize::Zeroize::zeroize(&mut value);
    run_prompt(&connection, &prompt).await?;
    tracing::info!("made a new key for the clipboard history");
    Ok(key)
}

/// Unlocks an item or a collection, asking the user when needed.
async fn unlock(
    connection: &Connection,
    service: &ServiceProxy<'_>,
    object: &OwnedObjectPath,
) -> Result<(), String> {
    let (_, prompt) = service
        .unlock(&[object.as_ref()])
        .await
        .map_err(|error| format!("cannot unlock the keyring: {error}"))?;
    run_prompt(connection, &prompt).await
}

/// Shows a prompt and waits for the user. `/` means there's none.
async fn run_prompt(connection: &Connection, prompt: &OwnedObjectPath) -> Result<(), String> {
    if prompt.as_str() == "/" {
        return Ok(());
    }
    let prompt = PromptProxy::builder(connection)
        .path(prompt.clone())
        .map_err(|error| error.to_string())?
        .build()
        .await
        .map_err(|error| error.to_string())?;
    let mut completed = prompt
        .receive_completed()
        .await
        .map_err(|error| error.to_string())?;
    prompt
        .prompt("")
        .await
        .map_err(|error| format!("cannot ask to unlock the keyring: {error}"))?;
    let signal = completed
        .next()
        .await
        .ok_or("the keyring prompt went away")?;
    let args = signal.args().map_err(|error| error.to_string())?;
    if args.dismissed {
        return Err("the keyring stayed locked".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    /// Run against a keyring of its own, never the user's. gnome-keyring
    /// keeps keyrings in `$XDG_DATA_HOME`, so that has to move too, and
    /// without a display it can't open a dialog:
    ///
    /// ```sh
    /// dbus-run-session -- env -u WAYLAND_DISPLAY -u DISPLAY sh -c 'export
    ///   XDG_RUNTIME_DIR=$(mktemp -d) XDG_DATA_HOME=$(mktemp -d) HOME=$(mktemp -d);
    ///   echo test | gnome-keyring-daemon --unlock --components=secrets >/dev/null;
    ///   cargo test -p mochi-module-clipboard -- --ignored secret'
    /// ```
    #[tokio::test]
    #[ignore = "needs a Secret Service of its own; see the comment"]
    async fn makes_a_key_once_and_reads_it_back() {
        let first = super::key().await.unwrap();
        let second = super::key().await.unwrap();
        assert_eq!(*first, *second);
        assert_ne!(*first, [0; 32]);
    }
}
