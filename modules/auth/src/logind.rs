//! logind, on the system bus: the session's Lock signal and its id.

use zbus::zvariant::OwnedObjectPath;
use zbus::{Connection, proxy};

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
    /// Its id, like `2`, as polkit names the session.
    #[zbus(property)]
    fn id(&self) -> zbus::Result<String>;

    /// Someone asked the session to lock: `loginctl lock-session`, an idle
    /// daemon, the power module.
    #[zbus(signal)]
    fn lock(&self) -> zbus::Result<()>;
}

/// The user's session on screen. `XDG_SESSION_ID` is often unset in a
/// daemon, so ask logind which session is the user's display.
pub async fn session(system: &Connection) -> zbus::Result<SessionProxy<'static>> {
    let (_, path) = UserProxy::new(system).await?.display().await?;
    SessionProxy::builder(system).path(path)?.build().await
}
