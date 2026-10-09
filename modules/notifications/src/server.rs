//! `org.freedesktop.Notifications` on the session bus.
//!
//! Only one program owns that name at a time. If another notification daemon
//! has it, Mochi queues for it and takes over when that daemon stops, without
//! taking it away.
//!
//! Calls arrive on zbus's own tasks, so the server only hands out ids and
//! forwards everything to the module as [`Incoming`].

use std::collections::HashMap;
use std::time::SystemTime;

use enumflags2::BitFlags;
use futures_util::StreamExt;
use tokio::sync::mpsc::UnboundedSender;
use zbus::fdo::{DBusProxy, RequestNameReply};
use zbus::object_server::SignalEmitter;
use zbus::zvariant::OwnedValue;
use zbus::{Connection, connection, interface};

use crate::center::Reason;
use crate::note::{Note, Request};

const NAME: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";

#[derive(Debug)]
pub enum Incoming {
    Notify(Box<Note>),
    Close(u32),
}

#[derive(Debug)]
struct Server {
    last_id: u32,
    incoming: UnboundedSender<Incoming>,
}

#[interface(name = "org.freedesktop.Notifications")]
impl Server {
    fn get_capabilities(&self) -> Vec<&str> {
        vec![
            "actions",
            "body",
            "body-hyperlinks",
            "body-markup",
            "icon-static",
            "inline-reply",
            "persistence",
            "sound",
        ]
    }

    // The spec fixes these arguments.
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &mut self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> u32 {
        let id = if replaces_id == 0 {
            // 0 means "none", so skip it when the counter wraps.
            self.last_id = self.last_id.checked_add(1).unwrap_or(1);
            self.last_id
        } else {
            replaces_id
        };
        let request = Request {
            app_name,
            app_icon,
            summary,
            body,
            actions,
            hints,
            expire_timeout,
        };
        let note = Note::new(id, request, SystemTime::now());
        let _ = self.incoming.send(Incoming::Notify(Box::new(note)));
        id
    }

    fn close_notification(&self, id: u32) {
        let _ = self.incoming.send(Incoming::Close(id));
    }

    fn get_server_information(&self) -> (&str, &str, &str, &str) {
        ("Mochi", "Mochi", env!("CARGO_PKG_VERSION"), "1.2")
    }

    #[zbus(signal)]
    async fn notification_closed(
        emitter: &SignalEmitter<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn action_invoked(
        emitter: &SignalEmitter<'_>,
        id: u32,
        action_key: &str,
    ) -> zbus::Result<()>;

    /// Not in the spec: KDE added it with the `inline-reply` capability,
    /// and apps that send an `inline-reply` action listen for it.
    #[zbus(signal)]
    async fn notification_replied(
        emitter: &SignalEmitter<'_>,
        id: u32,
        text: &str,
    ) -> zbus::Result<()>;
}

/// Serves the interface and asks for the name, queueing behind another
/// daemon that holds it. New notifications are numbered after `last_id`.
pub async fn start(incoming: UnboundedSender<Incoming>, last_id: u32) -> zbus::Result<Connection> {
    let server = Server { last_id, incoming };
    let connection = connection::Builder::session()?
        .serve_at(PATH, server)?
        .build()
        .await?;

    // Subscribe first, so the moment we get the name can't slip by.
    let bus = DBusProxy::new(&connection).await?;
    let mut acquired = bus.receive_name_acquired().await?;
    // No flags: queue behind the current owner instead of replacing it, and
    // keep the name once we have it. zbus's default flags do the opposite.
    match connection
        .request_name_with_flags(NAME, BitFlags::empty())
        .await?
    {
        RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner => {
            tracing::info!("serving notifications");
        }
        RequestNameReply::InQueue | RequestNameReply::Exists => {
            let owner = bus
                .get_name_owner(NAME.try_into()?)
                .await
                .map(|owner| owner.to_string())
                .unwrap_or_default();
            tracing::warn!(
                %owner,
                "another notification daemon is running; notifications go to it until it stops"
            );
            tokio::spawn(async move {
                while let Some(signal) = acquired.next().await {
                    if signal.args().is_ok_and(|args| args.name() == NAME) {
                        tracing::info!(
                            "the other notification daemon stopped; serving notifications"
                        );
                        return;
                    }
                }
            });
        }
    }
    Ok(connection)
}

pub async fn closed(connection: &Connection, id: u32, reason: Reason) -> zbus::Result<()> {
    let emitter = SignalEmitter::new(connection, PATH)?;
    Server::notification_closed(&emitter, id, reason as u32).await
}

pub async fn invoked(connection: &Connection, id: u32, action: &str) -> zbus::Result<()> {
    let emitter = SignalEmitter::new(connection, PATH)?;
    Server::action_invoked(&emitter, id, action).await
}

pub async fn replied(connection: &Connection, id: u32, text: &str) -> zbus::Result<()> {
    let emitter = SignalEmitter::new(connection, PATH)?;
    Server::notification_replied(&emitter, id, text).await
}
