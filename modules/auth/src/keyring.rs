//! gnome-keyring's prompter: gnome-keyring, and gcr's SSH agent, ask the
//! session bus's `org.gnome.keyring.SystemPrompter` for passwords, like the
//! one that unlocks a keyring, a new keyring's, or an SSH key's passphrase.
//! GNOME's `gcr-prompter` answers there by default; Mochi takes the name so
//! they ask on the island instead.
//!
//! The protocol is gcr's, a private one: a client calls `BeginPrompting`
//! with a callback object of its own; the prompter calls `PromptReady` on it
//! when its turn comes; the client calls `PerformPrompt` with what to ask;
//! the prompter answers with `PromptReady` again, the password encrypted
//! with [`Exchange`]; `StopPrompting` ends it, and the prompter says
//! `PromptDone`. One client prompts at a time.

use std::collections::{HashMap, VecDeque};

use mochi_core::ModuleCtx;
use tokio::sync::{mpsc, oneshot};
use zbus::message::Header;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
use zbus::{Connection, interface};

use crate::exchange::Exchange;
use crate::prompt::{KeyringAsk, KeyringReply, Prompts};

const NAME: &str = "org.gnome.keyring.SystemPrompter";
const PATH: &str = "/org/gnome/keyring/Prompter";
const CALLBACK: &str = "org.gnome.keyring.internal.Prompter.Callback";

/// A client's callback object: its bus name and path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caller {
    sender: String,
    path: OwnedObjectPath,
}

#[derive(Debug)]
pub enum Event {
    Begin(Caller),
    Perform {
        caller: Caller,
        confirm: bool,
        properties: HashMap<String, OwnedValue>,
        exchange: String,
        checked: oneshot::Sender<Result<(), String>>,
    },
    Stop(Caller),
    /// The client didn't take a `PromptReady`: it's gone.
    Gone(Caller),
}

struct Service {
    events: mpsc::UnboundedSender<Event>,
}

fn caller(header: &Header<'_>, path: OwnedObjectPath) -> zbus::fdo::Result<Caller> {
    let sender = header
        .sender()
        .ok_or_else(|| zbus::fdo::Error::Failed("no sender".into()))?;
    Ok(Caller {
        sender: sender.to_string(),
        path,
    })
}

#[interface(name = "org.gnome.keyring.internal.Prompter")]
impl Service {
    async fn begin_prompting(
        &self,
        #[zbus(header)] header: Header<'_>,
        callback: OwnedObjectPath,
    ) -> zbus::fdo::Result<()> {
        let _ = self.events.send(Event::Begin(caller(&header, callback)?));
        Ok(())
    }

    async fn perform_prompt(
        &self,
        #[zbus(header)] header: Header<'_>,
        callback: OwnedObjectPath,
        r#type: String,
        properties: HashMap<String, OwnedValue>,
        exchange: String,
    ) -> zbus::fdo::Result<()> {
        let confirm = match r#type.as_str() {
            "confirm" => true,
            "password" => false,
            _ => {
                return Err(zbus::fdo::Error::InvalidArgs(
                    "Invalid type argument".into(),
                ));
            }
        };
        let (checked, answer) = oneshot::channel();
        let _ = self.events.send(Event::Perform {
            caller: caller(&header, callback)?,
            confirm,
            properties,
            exchange,
            checked,
        });
        match answer.await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(zbus::fdo::Error::Failed(error)),
            Err(_) => Err(zbus::fdo::Error::Failed("the auth module stopped".into())),
        }
    }

    async fn stop_prompting(
        &self,
        #[zbus(header)] header: Header<'_>,
        callback: OwnedObjectPath,
    ) -> zbus::fdo::Result<()> {
        let _ = self.events.send(Event::Stop(caller(&header, callback)?));
        Ok(())
    }
}

/// The client on its turn.
struct Active {
    caller: Caller,
    exchange: Exchange,
    properties: HashMap<String, OwnedValue>,
    /// The prompt on the island for it, by serial, while there's one.
    asking: Option<u64>,
}

pub struct Prompter {
    connection: Connection,
    events: mpsc::UnboundedSender<Event>,
    waiting: VecDeque<Caller>,
    active: Option<Active>,
    serials: u64,
}

impl Prompter {
    /// Serves the prompter on the session bus and asks for its name. While
    /// gcr-prompter holds it, Mochi waits in the bus's queue and gets it
    /// once that one exits, as it does after a few idle seconds.
    pub async fn start(events: mpsc::UnboundedSender<Event>) -> Result<Self, String> {
        let connection = Connection::session()
            .await
            .map_err(|error| error.to_string())?;
        connection
            .object_server()
            .at(
                PATH,
                Service {
                    events: events.clone(),
                },
            )
            .await
            .map_err(|error| error.to_string())?;
        let reply = connection
            .request_name_with_flags(NAME, enumflags2::BitFlags::empty())
            .await
            .map_err(|error| error.to_string())?;
        if reply == zbus::fdo::RequestNameReply::InQueue {
            tracing::info!(
                "gcr-prompter has the keyring's prompts for now; Mochi takes them when it exits"
            );
        }
        Ok(Self {
            connection,
            events,
            waiting: VecDeque::new(),
            active: None,
            serials: 0,
        })
    }

    pub fn event(&mut self, ctx: &ModuleCtx, prompts: &mut Prompts, event: Event) {
        match event {
            Event::Begin(caller) => {
                self.waiting.push_back(caller);
                self.next();
            }
            Event::Perform {
                caller,
                confirm,
                properties,
                exchange,
                checked,
            } => {
                let result = self.perform(ctx, prompts, &caller, confirm, properties, &exchange);
                let _ = checked.send(result);
            }
            Event::Stop(caller) | Event::Gone(caller) => {
                self.waiting.retain(|waiting| waiting != &caller);
                if self
                    .active
                    .as_ref()
                    .is_some_and(|active| active.caller == caller)
                    && let Some(active) = self.active.take()
                {
                    if let Some(serial) = active.asking {
                        prompts.withdraw_keyring(ctx, serial);
                    }
                    self.call(&active.caller, "PromptDone", ());
                }
                self.next();
            }
        }
    }

    fn perform(
        &mut self,
        ctx: &ModuleCtx,
        prompts: &mut Prompts,
        caller: &Caller,
        confirm: bool,
        properties: HashMap<String, OwnedValue>,
        exchange: &str,
    ) -> Result<(), String> {
        let active = self
            .active
            .as_mut()
            .filter(|active| &active.caller == caller)
            .ok_or("Not begun prompting for this prompt callback")?;
        if active.asking.is_some() {
            return Err("Already performing a prompt for this prompt callback".into());
        }
        // Properties come as they change: keep the ones before.
        active.properties.extend(properties);
        active
            .exchange
            .receive(exchange)
            .map_err(|_| "Invalid secret exchange received")?;
        self.serials += 1;
        active.asking = Some(self.serials);
        tracing::info!("the keyring asks for a password");
        prompts.ask_keyring(ctx, ask(self.serials, confirm, &active.properties));
        Ok(())
    }

    /// The island's answer for the prompt `serial`.
    pub fn answered(&mut self, serial: u64, reply: KeyringReply) {
        let Some(active) = self
            .active
            .as_mut()
            .filter(|active| active.asking == Some(serial))
        else {
            return;
        };
        active.asking = None;
        let mut properties: HashMap<&str, Value<'_>> = HashMap::new();
        if active.properties.contains_key("choice-label") {
            // As gcr's prompter sends them: each value in a variant of its
            // own, inside the dictionary's.
            properties.insert(
                "choice-chosen",
                Value::Value(Box::new(Value::from(reply.choice))),
            );
        }
        let password = reply.password.as_ref().filter(|_| reply.accepted);
        // For a new password, how strong it is: gnome-keyring asks whether
        // to store passwords unencrypted when it's 0, as for a blank one.
        if unwrapped_flag(&active.properties, "password-new") {
            let strength = password.map_or(0, |password| strength(password));
            properties.insert(
                "password-strength",
                Value::Value(Box::new(Value::from(strength))),
            );
        }
        let exchange = match active
            .exchange
            .send(password.map(|password| password.as_bytes()))
        {
            Ok(exchange) => exchange,
            Err(error) => {
                tracing::warn!(%error, "can't answer the keyring");
                return;
            }
        };
        let response = if reply.accepted { "yes" } else { "no" };
        let caller = active.caller.clone();
        self.call(&caller, "PromptReady", (response, properties, exchange));
    }

    /// The next client's turn, if none has it.
    fn next(&mut self) {
        if self.active.is_some() {
            return;
        }
        let Some(caller) = self.waiting.pop_front() else {
            return;
        };
        let exchange = match Exchange::new() {
            Ok(exchange) => exchange,
            Err(error) => {
                tracing::warn!(%error, "can't start the keyring's secret exchange");
                return;
            }
        };
        let begin = exchange.begin();
        self.active = Some(Active {
            caller: caller.clone(),
            exchange,
            properties: HashMap::new(),
            asking: None,
        });
        let none: HashMap<&str, Value<'_>> = HashMap::new();
        self.call(&caller, "PromptReady", ("", none, begin));
    }

    /// Calls the client's callback without waiting on it; a client that
    /// doesn't answer is gone.
    fn call<B>(&self, caller: &Caller, method: &'static str, body: B)
    where
        B: serde::Serialize + zbus::zvariant::DynamicType + Send + Sync + 'static,
    {
        let connection = self.connection.clone();
        let events = self.events.clone();
        let caller = caller.clone();
        tokio::spawn(async move {
            let sent = connection
                .call_method(
                    Some(caller.sender.as_str()),
                    caller.path.as_str(),
                    Some(CALLBACK),
                    method,
                    &body,
                )
                .await;
            if let Err(error) = sent
                && method == "PromptReady"
            {
                tracing::debug!(%error, "the keyring's client is gone");
                let _ = events.send(Event::Gone(caller));
            }
        });
    }
}

/// A value out of the variants around it: gcr wraps each property's value
/// in one of its own.
fn unwrap<'a>(value: &'a Value<'a>) -> &'a Value<'a> {
    match value {
        Value::Value(inner) => unwrap(inner),
        other => other,
    }
}

fn unwrapped_flag(properties: &HashMap<String, OwnedValue>, name: &str) -> bool {
    matches!(
        properties.get(name).map(|value| unwrap(value)),
        Some(Value::Bool(true))
    )
}

/// A password's strength as gcr's dialog reports it, 0 for a blank one
/// and up to 5: one for being there, one each for 8 and 12 characters, and
/// one each for mixing letters with digits, and with anything else.
fn strength(password: &str) -> i32 {
    if password.is_empty() {
        return 0;
    }
    let length = password.chars().count();
    let letters = password.chars().any(char::is_alphabetic);
    let digits = password.chars().any(|c| c.is_ascii_digit());
    let others = password.chars().any(|c| !c.is_alphanumeric());
    1 + i32::from(length >= 8)
        + i32::from(length >= 12)
        + i32::from(letters && digits)
        + i32::from(others)
}

/// What the island shows, from the prompt's properties.
fn ask(serial: u64, confirm: bool, properties: &HashMap<String, OwnedValue>) -> KeyringAsk {
    let text = |name: &str| match properties.get(name).map(|value| unwrap(value)) {
        Some(Value::Str(text)) if !text.is_empty() => Some(text.to_string()),
        _ => None,
    };
    let flag = |name: &str| unwrapped_flag(properties, name);
    KeyringAsk {
        serial,
        confirm,
        title: text("title"),
        message: text("message"),
        description: text("description"),
        warning: text("warning"),
        choice_label: text("choice-label"),
        choice: flag("choice-chosen"),
        password_new: flag("password-new"),
        continue_label: text("continue-label"),
        cancel_label: text("cancel-label"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blank_password_is_the_only_one_without_strength() {
        assert_eq!(strength(""), 0);
        assert_eq!(strength("a"), 1);
        assert_eq!(strength("correct horse battery"), 4);
        assert_eq!(strength("tr0ub4dor&3xyz"), 5);
    }

    #[test]
    fn reads_what_gnome_keyring_sends() {
        let properties = HashMap::from([
            (
                "title".to_owned(),
                OwnedValue::from(zbus::zvariant::Str::from("Unlock Keyring")),
            ),
            (
                "choice-label".to_owned(),
                OwnedValue::from(zbus::zvariant::Str::from("Automatically unlock")),
            ),
            (
                "choice-chosen".to_owned(),
                // Wrapped, as gcr sends it.
                Value::Value(Box::new(Value::from(true)))
                    .try_into()
                    .unwrap(),
            ),
            (
                "warning".to_owned(),
                OwnedValue::from(zbus::zvariant::Str::from("")),
            ),
        ]);
        let asked = ask(7, false, &properties);
        assert_eq!(asked.title.as_deref(), Some("Unlock Keyring"));
        assert_eq!(asked.choice_label.as_deref(), Some("Automatically unlock"));
        assert!(asked.choice);
        assert_eq!(asked.warning, None, "an empty warning is none");
        assert!(!asked.password_new);
    }
}
