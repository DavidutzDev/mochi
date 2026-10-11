//! How long the session remembers what lets someone in without a password:
//! sudo's password, the SSH keys in the agent, polkit's admin actions and
//! the keyring's unlocked collections. By default, until the screen locks.
//!
//! What's kept "until locked" is forgotten once the screen is locked. SSH
//! keys and the keyring may instead be kept for a time from when they were
//! entered: [`keep`] looks every minute, notes when each key and each
//! unlocked collection first showed up, and removes or locks it once its
//! time is up, however it got there. A lock leaves those alone.
//!
//! Each runs on its own and is given a few seconds: one that fails or has
//! nothing to forget doesn't hold the others back, and none holds the lock
//! back, which is already on screen.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::Deserialize;
use zbus::proxy;
use zbus::zvariant::{OwnedObjectPath, Value};

/// How long sudo's password or polkit's admin actions stay remembered:
/// Mochi can't keep them longer than they keep themselves.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Kept {
    /// Until the screen locks.
    #[default]
    UntilLocked,
    /// For as long as they keep it themselves, a lock or not: sudo's
    /// `timestamp_timeout`, 5 minutes unless sudoers says otherwise, and
    /// polkit's 5 minutes.
    OwnTimeout,
}

/// How long SSH keys stay in the agent, or the keyring unlocked, from when
/// they were entered.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
pub enum Lifetime {
    /// Until the screen locks.
    #[default]
    #[serde(rename = "until-locked")]
    UntilLocked,
    /// An hour, a lock or not.
    #[serde(rename = "1h")]
    Hour,
    /// Four hours, a lock or not.
    #[serde(rename = "4h")]
    FourHours,
    /// Eight hours, a lock or not.
    #[serde(rename = "8h")]
    EightHours,
    /// Until logging out.
    #[serde(rename = "until-logout")]
    UntilLogout,
}

impl Lifetime {
    /// How long, when it's a time.
    fn time(self) -> Option<Duration> {
        match self {
            Self::Hour => Some(Duration::from_secs(3600)),
            Self::FourHours => Some(Duration::from_secs(4 * 3600)),
            Self::EightHours => Some(Duration::from_secs(8 * 3600)),
            Self::UntilLocked | Self::UntilLogout => None,
        }
    }
}

/// How long each is remembered, from the module's settings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Remember {
    pub sudo: Kept,
    pub polkit: Kept,
    pub ssh_keys: Lifetime,
    pub keyring: Lifetime,
}

/// The longest each one may take.
const WAIT: Duration = Duration::from_secs(5);

/// How often [`keep`] looks.
const EVERY: Duration = Duration::from_secs(60);

#[proxy(
    interface = "org.freedesktop.PolicyKit1.Authority",
    default_service = "org.freedesktop.PolicyKit1",
    default_path = "/org/freedesktop/PolicyKit1/Authority",
    gen_blocking = false
)]
trait Authority {
    /// Drops what `auth_admin_keep` remembered for the subject.
    fn revoke_temporary_authorizations(
        &self,
        subject: &(&str, HashMap<&str, Value<'_>>),
    ) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.freedesktop.Secret.Service",
    default_service = "org.freedesktop.secrets",
    default_path = "/org/freedesktop/secrets",
    gen_blocking = false
)]
trait Secrets {
    #[zbus(property)]
    fn collections(&self) -> zbus::Result<Vec<OwnedObjectPath>>;

    fn lock(
        &self,
        objects: &[OwnedObjectPath],
    ) -> zbus::Result<(Vec<OwnedObjectPath>, OwnedObjectPath)>;
}

#[proxy(
    interface = "org.freedesktop.Secret.Collection",
    default_service = "org.freedesktop.secrets",
    gen_blocking = false
)]
trait Collection {
    #[zbus(property)]
    fn locked(&self) -> zbus::Result<bool>;
}

/// Forgets, now the screen is locked, what's kept until then, for the
/// logind session `session`.
pub async fn on_lock(remember: Remember, session: Option<String>) {
    let sudo = async {
        if remember.sudo == Kept::UntilLocked {
            // Removes this user's remembered password; never asks for one.
            run("sudo", &["-n", "-K"]).await;
        }
    };
    let ssh_keys = async {
        if remember.ssh_keys == Lifetime::UntilLocked && std::env::var_os("SSH_AUTH_SOCK").is_some()
        {
            run("ssh-add", &["-D"]).await;
        }
    };
    let keyring = async {
        if remember.keyring == Lifetime::UntilLocked {
            timed("the keyring", lock_keyring(None)).await;
        }
    };
    let polkit = async {
        if remember.polkit == Kept::UntilLocked
            && let Some(session) = &session
        {
            timed("polkit", revoke_polkit(session)).await;
        }
    };
    tokio::join!(sudo, ssh_keys, keyring, polkit);
}

/// Removes SSH keys and locks keyring collections once they've been there
/// longer than they may be. Runs for as long as the module does; with no
/// time set, it returns at once.
pub async fn keep(remember: Remember) {
    let (keys, collections) = (remember.ssh_keys.time(), remember.keyring.time());
    if keys.is_none() && collections.is_none() {
        return;
    }
    let mut seen = Seen::default();
    let mut every = tokio::time::interval(EVERY);
    loop {
        every.tick().await;
        let now = Instant::now();
        if let Some(time) = keys {
            let listed = list_keys().await;
            for key in seen.keys.expired(listed, now, time) {
                remove_key(&key).await;
            }
        }
        if let Some(time) = collections {
            let unlocked = unlocked_collections().await;
            let expired = seen.collections.expired(unlocked, now, time);
            if !expired.is_empty() {
                timed("the keyring", lock_keyring(Some(expired))).await;
            }
        }
    }
}

/// When each thing was first seen, by what it is.
#[derive(Debug, Default)]
struct Seen {
    keys: Since<String>,
    collections: Since<OwnedObjectPath>,
}

#[derive(Debug)]
struct Since<T>(HashMap<T, Instant>);

impl<T> Default for Since<T> {
    fn default() -> Self {
        Self(HashMap::new())
    }
}

impl<T: std::hash::Hash + Eq + Clone> Since<T> {
    /// Notes what's there `now`, forgets what's gone, and says what's been
    /// there `time` or longer. `None` when it couldn't be listed: then
    /// nothing changes.
    fn expired(&mut self, there: Option<Vec<T>>, now: Instant, time: Duration) -> Vec<T> {
        let Some(there) = there else {
            return Vec::new();
        };
        self.0.retain(|thing, _| there.contains(thing));
        for thing in there {
            self.0.entry(thing).or_insert(now);
        }
        let expired: Vec<T> = self
            .0
            .iter()
            .filter(|(_, since)| now.duration_since(**since) >= time)
            .map(|(thing, _)| thing.clone())
            .collect();
        for thing in &expired {
            self.0.remove(thing);
        }
        expired
    }
}

/// The agent's public keys, one line each, as `ssh-add -L` says them.
async fn list_keys() -> Option<Vec<String>> {
    std::env::var_os("SSH_AUTH_SOCK")?;
    let output = tokio::time::timeout(WAIT, quiet("ssh-add").arg("-L").output())
        .await
        .ok()?
        .ok()?;
    // 1 means no keys; anything else, no agent to ask.
    match output.status.code() {
        Some(0) => Some(
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(str::to_owned)
                .collect(),
        ),
        Some(1) => Some(Vec::new()),
        _ => None,
    }
}

/// Takes one key out of the agent by its public key, which `ssh-add -d`
/// reads from a file.
async fn remove_key(key: &str) {
    let Ok(file) = tempfile_with(key) else {
        return;
    };
    run("ssh-add", &["-d", &file.0.to_string_lossy()]).await;
}

/// A file holding `contents` in the runtime directory, gone when dropped.
fn tempfile_with(contents: &str) -> std::io::Result<Removed> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map_or_else(std::env::temp_dir, std::path::PathBuf::from);
    let path = dir.join(format!("mochi-key-{}.pub", std::process::id()));
    std::fs::write(&path, format!("{contents}\n"))?;
    Ok(Removed(path))
}

struct Removed(std::path::PathBuf);

impl Drop for Removed {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// The keyring's collections that are unlocked now.
async fn unlocked_collections() -> Option<Vec<OwnedObjectPath>> {
    let looked = async {
        let bus = zbus::Connection::session().await?;
        let mut unlocked = Vec::new();
        for path in SecretsProxy::new(&bus).await?.collections().await? {
            let collection = CollectionProxy::builder(&bus)
                .path(path.clone())?
                .build()
                .await?;
            if !collection.locked().await? {
                unlocked.push(path);
            }
        }
        Ok::<_, zbus::Error>(unlocked)
    };
    tokio::time::timeout(WAIT, looked).await.ok()?.ok()
}

/// A program that never waits on a prompt, and whose output is its own.
fn quiet(program: &str) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(program);
    command
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .env_remove("SSH_ASKPASS")
        .env_remove("SUDO_ASKPASS")
        .kill_on_drop(true);
    command
}

async fn run(program: &str, args: &[&str]) {
    let mut command = quiet(program);
    command.args(args).stdout(std::process::Stdio::null());
    match tokio::time::timeout(WAIT, command.status()).await {
        Ok(Ok(status)) if status.success() => tracing::debug!(program, "forgot"),
        Ok(Ok(status)) => tracing::debug!(program, %status, "nothing forgotten"),
        Ok(Err(error)) => tracing::debug!(program, %error, "can't run it to forget"),
        Err(_) => tracing::warn!(program, "took too long to forget"),
    }
}

async fn timed(what: &str, forgetting: impl Future<Output = zbus::Result<()>>) {
    match tokio::time::timeout(WAIT, forgetting).await {
        Ok(Ok(())) => tracing::debug!(what, "forgot"),
        Ok(Err(error)) => tracing::debug!(what, %error, "nothing forgotten"),
        Err(_) => tracing::warn!(what, "took too long to forget"),
    }
}

/// Locks the Secret Service's collections, gnome-keyring's or KeePassXC's,
/// these or all of them: their secrets need the password again.
async fn lock_keyring(these: Option<Vec<OwnedObjectPath>>) -> zbus::Result<()> {
    let bus = zbus::Connection::session().await?;
    let secrets = SecretsProxy::new(&bus).await?;
    let collections = match these {
        Some(these) => these,
        None => secrets.collections().await?,
    };
    if !collections.is_empty() {
        secrets.lock(&collections).await?;
    }
    Ok(())
}

/// Drops polkit's `auth_admin_keep` authorizations for the session, so
/// the next admin action asks again.
async fn revoke_polkit(session: &str) -> zbus::Result<()> {
    let bus = zbus::Connection::system().await?;
    let authority = AuthorityProxy::new(&bus).await?;
    let details = HashMap::from([("session-id", Value::from(session))]);
    authority
        .revoke_temporary_authorizations(&("unix-session", details))
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_stays_its_time_expires_and_what_goes_starts_again() {
        let hour = Duration::from_secs(3600);
        let start = Instant::now();
        let mut since = Since::<&str>::default();
        let some = |things: &[&'static str]| Some(things.to_vec());
        assert!(since.expired(some(&["a"]), start, hour).is_empty());
        let later = start + Duration::from_secs(1800);
        assert!(since.expired(some(&["a", "b"]), later, hour).is_empty());
        // Can't list: nothing changes.
        assert!(since.expired(None, start + hour * 2, hour).is_empty());
        assert_eq!(since.expired(some(&["a", "b"]), start + hour, hour), ["a"]);
        // "a" was removed, then added again: its hour starts over.
        let again = start + hour + Duration::from_secs(60);
        assert!(since.expired(some(&["a", "b"]), again, hour).is_empty());
        // "b" went away before its time, and came back.
        assert!(since.expired(some(&["a"]), again, hour).is_empty());
        assert!(
            since
                .expired(some(&["a", "b"]), later + hour, hour)
                .is_empty()
        );
        assert_eq!(since.expired(some(&["a", "b"]), again + hour, hour), ["a"]);
    }

    #[test]
    fn settings_read_as_the_menus_say() {
        let lifetime: Lifetime = serde_json::from_value(serde_json::json!("4h")).unwrap();
        assert_eq!(lifetime.time(), Some(Duration::from_secs(4 * 3600)));
        let lifetime: Lifetime = serde_json::from_value(serde_json::json!("until-locked")).unwrap();
        assert_eq!(lifetime.time(), None);
        let kept: Kept = serde_json::from_value(serde_json::json!("own-timeout")).unwrap();
        assert_eq!(kept, Kept::OwnTimeout);
        assert_eq!(Remember::default().ssh_keys, Lifetime::UntilLocked);
    }
}
