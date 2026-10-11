//! logind, on the system bus: the session's Lock signal, `LockedHint`, and
//! the sleep delay that gives the lock time to show before the computer
//! sleeps.

use zbus::zvariant::{OwnedFd, OwnedObjectPath};
use zbus::{Connection, proxy};

#[proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1",
    gen_blocking = false
)]
pub trait Manager {
    /// Holds off `what` while the returned file stays open.
    fn inhibit(&self, what: &str, who: &str, why: &str, mode: &str) -> zbus::Result<OwnedFd>;

    /// `true` just before sleeping, `false` after waking.
    #[zbus(signal)]
    fn prepare_for_sleep(&self, start: bool) -> zbus::Result<()>;
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
    fn set_locked_hint(&self, locked: bool) -> zbus::Result<()>;

    /// Its id, like `2`, as polkit names the session.
    #[zbus(property)]
    fn id(&self) -> zbus::Result<String>;

    /// Someone asked the session to lock: `loginctl lock-session`, an idle
    /// daemon, the power module.
    #[zbus(signal)]
    fn lock(&self) -> zbus::Result<()>;
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
