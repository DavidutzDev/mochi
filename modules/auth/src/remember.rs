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
    /// As the settings say it.
    pub fn name(self) -> &'static str {
        match self {
            Self::UntilLocked => "until-locked",
            Self::Hour => "1h",
            Self::FourHours => "4h",
            Self::EightHours => "8h",
            Self::UntilLogout => "until-logout",
        }
    }

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

/// The time a prompt picked for an SSH key, by [`key_id`].
pub type KeyTimes = std::sync::Arc<std::sync::Mutex<HashMap<String, Lifetime>>>;

/// A public key without its comment: its type and its base64, as both
/// `ssh-add -L` and a `.pub` file say them.
pub fn key_id(line: &str) -> String {
    line.split_whitespace()
        .take(2)
        .collect::<Vec<_>>()
        .join(" ")
}

fn picked(times: &KeyTimes) -> HashMap<String, Lifetime> {
    times.lock().map(|times| times.clone()).unwrap_or_default()
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
/// logind session `session`. An SSH key goes with the time its prompt
/// picked, in `times`, or else `remember_ssh_keys`'s.
pub async fn on_lock(remember: Remember, session: Option<String>, times: KeyTimes) {
    let sudo = async {
        if remember.sudo == Kept::UntilLocked {
            // Removes this user's remembered password; never asks for one.
            run("sudo", &["-n", "-K"]).await;
        }
    };
    let ssh_keys = async {
        if std::env::var_os("SSH_AUTH_SOCK").is_none() {
            return;
        }
        let picked = picked(&times);
        if picked.is_empty() {
            if remember.ssh_keys == Lifetime::UntilLocked {
                run("ssh-add", &["-D"]).await;
            }
            return;
        }
        // Key by key: some stay through the lock.
        for key in list_keys().await.unwrap_or_default() {
            let lifetime = picked
                .get(&key_id(&key))
                .copied()
                .unwrap_or(remember.ssh_keys);
            if lifetime == Lifetime::UntilLocked {
                remove_key(&key).await;
            }
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
/// longer than they may be: a key the time its prompt picked, in `times`,
/// or else `remember_ssh_keys`'s. Runs for as long as the module does.
pub async fn keep(remember: Remember, times: KeyTimes) {
    let collections = remember.keyring.time();
    let mut seen = Seen::default();
    let mut every = tokio::time::interval(EVERY);
    loop {
        every.tick().await;
        let now = Instant::now();
        let picked = picked(&times);
        let time_of = |key: &String| {
            picked
                .get(&key_id(key))
                .copied()
                .unwrap_or(remember.ssh_keys)
                .time()
        };
        let timed_keys = remember.ssh_keys.time().is_some()
            || picked.values().any(|lifetime| lifetime.time().is_some());
        if timed_keys {
            let listed = list_keys().await;
            for key in seen.keys.expired(listed, now, time_of) {
                remove_key(&key).await;
            }
        }
        if let Some(time) = collections {
            let unlocked = unlocked_collections().await;
            let expired = seen.collections.expired(unlocked, now, |_| Some(time));
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
    /// there its `time` or longer; one without a time stays. `None` when it
    /// couldn't be listed: then nothing changes.
    fn expired(
        &mut self,
        there: Option<Vec<T>>,
        now: Instant,
        time: impl Fn(&T) -> Option<Duration>,
    ) -> Vec<T> {
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
            .filter(|(thing, since)| {
                time(thing).is_some_and(|time| now.duration_since(**since) >= time)
            })
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
/// Each has a name of its own, as a lock and [`keep`] may remove keys at
/// once, and is new: one already there, or a link, isn't written through.
fn tempfile_with(contents: &str) -> std::io::Result<Removed> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    static FILES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map_or_else(std::env::temp_dir, std::path::PathBuf::from);
    let number = FILES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = dir.join(format!("mochi-key-{}-{number}.pub", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)?;
    let removed = Removed(path);
    file.write_all(format!("{contents}\n").as_bytes())?;
    Ok(removed)
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
        let all = |_: &&str| Some(hour);
        let some = |things: &[&'static str]| Some(things.to_vec());
        assert!(since.expired(some(&["a"]), start, all).is_empty());
        let later = start + Duration::from_secs(1800);
        assert!(since.expired(some(&["a", "b"]), later, all).is_empty());
        // Can't list: nothing changes.
        assert!(since.expired(None, start + hour * 2, all).is_empty());
        assert_eq!(since.expired(some(&["a", "b"]), start + hour, all), ["a"]);
        // "a" was removed, then added again: its hour starts over.
        let again = start + hour + Duration::from_secs(60);
        assert!(since.expired(some(&["a", "b"]), again, all).is_empty());
        // "b" went away before its time, and came back.
        assert!(since.expired(some(&["a"]), again, all).is_empty());
        assert!(
            since
                .expired(some(&["a", "b"]), later + hour, all)
                .is_empty()
        );
        assert_eq!(since.expired(some(&["a", "b"]), again + hour, all), ["a"]);
    }

    #[test]
    fn a_key_with_its_own_time_and_one_without() {
        let hour = Duration::from_secs(3600);
        let start = Instant::now();
        let mut since = Since::<&str>::default();
        let some = |things: &[&'static str]| Some(things.to_vec());
        // "kept" was picked to stay until logout: it never expires here.
        let time = |key: &&str| (*key != "kept").then_some(hour);
        assert!(
            since
                .expired(some(&["kept", "timed"]), start, time)
                .is_empty()
        );
        assert_eq!(
            since.expired(some(&["kept", "timed"]), start + hour, time),
            ["timed"]
        );
        assert!(
            since
                .expired(some(&["kept"]), start + hour * 9, time)
                .is_empty()
        );
    }

    #[test]
    fn a_key_is_known_by_its_type_and_base64() {
        assert_eq!(
            key_id("ssh-ed25519 AAAAC3Nza me@laptop"),
            key_id("ssh-ed25519 AAAAC3Nza other comment")
        );
        assert_eq!(key_id("ssh-ed25519 AAAAC3Nza"), "ssh-ed25519 AAAAC3Nza");
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
