//! polkit's authentication agent for the session: when an app asks for an
//! admin action, like pkexec, systemctl, run0 or a settings app, polkitd
//! asks the session's agent to authenticate someone who may allow it. Mochi
//! asks on the island; polkit's own helper checks the password with PAM and
//! tells polkitd, so nothing here decides who's allowed.
//!
//! Only one agent serves a session: another one, like hyprpolkitagent,
//! keeps Mochi from registering, and [`start`] says so.

use std::collections::HashMap;

use tokio::sync::{mpsc, oneshot};
use zbus::message::Header;
use zbus::zvariant::{OwnedValue, Value};
use zbus::{Connection, interface};

const PATH: &str = "/org/mochi/shell/PolicyKit1/AuthenticationAgent";

/// What polkitd asks for.
#[derive(Debug)]
pub enum Event {
    Begin(Request),
    /// polkitd no longer needs the one with this cookie: the app gave up.
    Cancel(String),
}

/// An authentication polkitd wants: someone of `identities` allowing
/// `action`.
#[derive(Debug)]
pub struct Request {
    pub action: String,
    /// Why, as polkit says it, in the user's language.
    pub message: String,
    pub icon: String,
    /// More about it: for pkexec, `program` and `command_line`; for every
    /// action, the asking process as `polkit.caller-pid`.
    pub details: HashMap<String, String>,
    pub cookie: String,
    /// The users who may allow it, by name.
    pub users: Vec<String>,
    /// `true` once someone allowed it, `false` when cancelled or failed.
    pub done: oneshot::Sender<bool>,
}

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.freedesktop.PolicyKit1.Error")]
enum PolkitError {
    #[zbus(error)]
    ZBus(zbus::Error),
    Cancelled(String),
    Failed(String),
}

struct Agent {
    events: mpsc::UnboundedSender<Event>,
}

/// An identity's user name: polkit sends users, having turned admin groups
/// into their members already; anything else is left out.
fn user_name(identity: &(String, HashMap<String, OwnedValue>)) -> Option<String> {
    let (kind, details) = identity;
    if kind != "unix-user" {
        return None;
    }
    let uid = u32::try_from(details.get("uid")?).ok()?;
    name_of(uid)
}

/// The name of the user with `uid`, from the system's user database.
pub fn name_of(uid: u32) -> Option<String> {
    let mut buffer = vec![0u8; 4096];
    // SAFETY: getpwuid_r fills `entry` with pointers into `buffer`, both of
    // which outlive the reads below; `found` is null when there's no user.
    unsafe {
        let mut entry: libc::passwd = std::mem::zeroed();
        let mut found: *mut libc::passwd = std::ptr::null_mut();
        let status = libc::getpwuid_r(
            uid,
            &raw mut entry,
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &raw mut found,
        );
        if status != 0 || found.is_null() || entry.pw_name.is_null() {
            return None;
        }
        Some(
            std::ffi::CStr::from_ptr(entry.pw_name)
                .to_string_lossy()
                .into_owned(),
        )
    }
}

/// Whether polkitd sent the call: the bus's policy lets only it call an
/// agent, and this checks again, so nothing else puts a prompt up or takes
/// one down.
async fn from_polkitd(connection: &Connection, header: &Header<'_>) -> bool {
    let Some(sender) = header.sender() else {
        return false;
    };
    let Ok(bus) = zbus::fdo::DBusProxy::new(connection).await else {
        return false;
    };
    let Ok(polkitd) = zbus::names::BusName::from_static_str("org.freedesktop.PolicyKit1") else {
        return false;
    };
    bus.get_name_owner(polkitd)
        .await
        .is_ok_and(|owner| owner.as_str() == sender.as_str())
}

#[interface(name = "org.freedesktop.PolicyKit1.AuthenticationAgent")]
impl Agent {
    /// Answers once it's over: allowed, cancelled, or failed too often.
    #[allow(clippy::too_many_arguments)]
    async fn begin_authentication(
        &self,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
        action_id: String,
        message: String,
        icon_name: String,
        details: HashMap<String, String>,
        cookie: String,
        identities: Vec<(String, HashMap<String, OwnedValue>)>,
    ) -> Result<(), PolkitError> {
        if !from_polkitd(connection, &header).await {
            return Err(PolkitError::Failed("only polkitd asks".into()));
        }
        let users: Vec<String> = identities.iter().filter_map(user_name).collect();
        if users.is_empty() {
            return Err(PolkitError::Failed("Mochi can only ask users".into()));
        }
        let (done, finished) = oneshot::channel();
        let request = Request {
            action: action_id,
            message,
            icon: icon_name,
            details,
            cookie,
            users,
            done,
        };
        self.events
            .send(Event::Begin(request))
            .map_err(|_| PolkitError::Failed("the auth module stopped".into()))?;
        match finished.await {
            Ok(true) => Ok(()),
            _ => Err(PolkitError::Cancelled("cancelled on the island".into())),
        }
    }

    async fn cancel_authentication(
        &self,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
        cookie: String,
    ) {
        if from_polkitd(connection, &header).await {
            let _ = self.events.send(Event::Cancel(cookie));
        }
    }
}

/// The subject Mochi answers for: the session on screen. A debug build
/// answers for one process instead with `MOCHI_POLKIT_PROCESS=<pid>`, so it
/// can be tried beside the session's own agent.
fn subject(session: &str) -> (&'static str, HashMap<&'static str, Value<'static>>) {
    #[cfg(debug_assertions)]
    if let Some(pid) = std::env::var("MOCHI_POLKIT_PROCESS")
        .ok()
        .and_then(|pid| pid.parse::<u32>().ok())
    {
        return (
            "unix-process",
            HashMap::from([
                ("pid", Value::from(pid)),
                ("start-time", Value::from(start_time(pid).unwrap_or(0))),
            ]),
        );
    }
    (
        "unix-session",
        HashMap::from([("session-id", Value::from(session.to_owned()))]),
    )
}

/// When a process started, in clock ticks since boot, as polkit names it.
#[cfg(debug_assertions)]
fn start_time(pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // The fields after the command, whose name may hold spaces.
    let rest = &stat[stat.rfind(')')? + 2..];
    rest.split(' ').nth(19)?.parse().ok()
}

/// Serves the agent on `system` and registers it for the session
/// `session`. polkitd forgets it when the connection closes.
pub async fn start(
    system: &Connection,
    session: &str,
    events: mpsc::UnboundedSender<Event>,
) -> Result<(), String> {
    system
        .object_server()
        .at(PATH, Agent { events })
        .await
        .map_err(|error| error.to_string())?;
    let locale = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_else(|_| "C".into());
    system
        .call_method(
            Some("org.freedesktop.PolicyKit1"),
            "/org/freedesktop/PolicyKit1/Authority",
            Some("org.freedesktop.PolicyKit1.Authority"),
            "RegisterAuthenticationAgent",
            &(subject(session), locale.as_str(), PATH),
        )
        .await
        .map(drop)
        .map_err(|error| match error {
            zbus::Error::MethodError(_, Some(message), _)
                if message.contains("already exists") =>
            {
                "another polkit agent answers for this session, like hyprpolkitagent: turn it off for Mochi's".to_owned()
            }
            error => error.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_users_are_asked() {
        let root = (
            "unix-user".to_owned(),
            HashMap::from([("uid".to_owned(), OwnedValue::from(0u32))]),
        );
        assert_eq!(user_name(&root).as_deref(), Some("root"));
        let group = (
            "unix-group".to_owned(),
            HashMap::from([("gid".to_owned(), OwnedValue::from(0u32))]),
        );
        assert_eq!(user_name(&group), None);
    }
}
