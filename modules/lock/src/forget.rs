//! What the lock makes the session forget once the screen is locked:
//! sudo's remembered password, the SSH keys in the agent, the keyring's
//! unlocked collections and polkit's remembered authorizations. Whoever
//! sits down at a locked computer, or unlocks it, types them again.
//!
//! Each runs on its own and is given a few seconds: one that fails or has
//! nothing to forget doesn't hold the others back, and none of them holds
//! the lock back, which is already on screen.

use std::collections::HashMap;
use std::time::Duration;

use zbus::proxy;
use zbus::zvariant::{OwnedObjectPath, Value};

/// What to forget, from the module's settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Forget {
    pub sudo: bool,
    pub ssh_keys: bool,
    pub keyring: bool,
    pub polkit: bool,
}

/// The longest each one may take.
const WAIT: Duration = Duration::from_secs(5);

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

/// Forgets what `what` says, for the logind session `session`.
pub async fn forget(what: Forget, session: Option<String>) {
    let sudo = async {
        if what.sudo {
            // Removes this user's remembered password; never asks for one.
            run("sudo", &["-n", "-K"]).await;
        }
    };
    let ssh_keys = async {
        if what.ssh_keys && std::env::var_os("SSH_AUTH_SOCK").is_some() {
            run("ssh-add", &["-D"]).await;
        }
    };
    let keyring = async {
        if what.keyring {
            timed("the keyring", lock_keyring()).await;
        }
    };
    let polkit = async {
        if what.polkit
            && let Some(session) = &session
        {
            timed("polkit", revoke_polkit(session)).await;
        }
    };
    tokio::join!(sudo, ssh_keys, keyring, polkit);
}

/// Runs a program with nothing to read, so it can never wait on a prompt.
async fn run(program: &str, args: &[&str]) {
    let mut command = tokio::process::Command::new(program);
    command
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .env_remove("SSH_ASKPASS")
        .env_remove("SUDO_ASKPASS")
        .kill_on_drop(true);
    match tokio::time::timeout(WAIT, command.status()).await {
        Ok(Ok(status)) if status.success() => {
            tracing::debug!(program, "forgot on locking");
        }
        Ok(Ok(status)) => tracing::debug!(program, %status, "nothing forgotten on locking"),
        Ok(Err(error)) => tracing::debug!(program, %error, "can't run it on locking"),
        Err(_) => tracing::warn!(program, "took too long on locking"),
    }
}

async fn timed(what: &str, forgetting: impl Future<Output = zbus::Result<()>>) {
    match tokio::time::timeout(WAIT, forgetting).await {
        Ok(Ok(())) => tracing::debug!(what, "forgot on locking"),
        Ok(Err(error)) => tracing::debug!(what, %error, "nothing forgotten on locking"),
        Err(_) => tracing::warn!(what, "took too long on locking"),
    }
}

/// Locks every collection of the Secret Service, gnome-keyring's or
/// KeePassXC's: their secrets need the password again.
async fn lock_keyring() -> zbus::Result<()> {
    let bus = zbus::Connection::session().await?;
    let secrets = SecretsProxy::new(&bus).await?;
    let collections = secrets.collections().await?;
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
