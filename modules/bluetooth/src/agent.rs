//! The pairing agent: BlueZ asks it the questions pairing needs, like
//! whether a code matches the one on the device, and the module asks them
//! on the island.

use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;
use zbus::Connection;
use zbus::zvariant::{ObjectPath, OwnedObjectPath};

pub const PATH: &str = "/org/mochi/BluetoothAgent";

/// A question for the user. Each waits for its answer, or for the user to
/// go away.
#[derive(Debug)]
pub enum Question {
    /// Does the device show this six-digit code?
    Confirm {
        device: String,
        code: u32,
        answer: oneshot::Sender<bool>,
    },
    /// Type the code the device shows or expects, for older devices.
    Pin {
        device: String,
        answer: oneshot::Sender<Option<String>>,
    },
    /// Show a code to type on the device, like a keyboard.
    Show { device: String, code: String },
    /// May this device pair, with no code at all?
    Allow {
        device: String,
        answer: oneshot::Sender<bool>,
    },
    /// BlueZ gave up: close whatever is asked.
    Cancel,
}

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.bluez.Error")]
enum Refusal {
    #[zbus(error)]
    ZBus(zbus::Error),
    Rejected(String),
    Canceled(String),
}

#[derive(Debug)]
struct Agent {
    questions: UnboundedSender<Question>,
}

impl Agent {
    /// Asks, and waits for the answer.
    async fn ask<T>(
        &self,
        question: impl FnOnce(oneshot::Sender<T>) -> Question,
    ) -> Result<T, Refusal> {
        let (answer, answered) = oneshot::channel();
        self.questions
            .send(question(answer))
            .map_err(|_| Refusal::Canceled("Mochi isn't listening".into()))?;
        answered
            .await
            .map_err(|_| Refusal::Canceled("no answer".into()))
    }
}

#[zbus::interface(name = "org.bluez.Agent1")]
impl Agent {
    fn release(&self) {}

    async fn request_pin_code(&self, device: OwnedObjectPath) -> Result<String, Refusal> {
        let device = device.to_string();
        self.ask(|answer| Question::Pin { device, answer })
            .await?
            .ok_or_else(|| Refusal::Rejected("no code".into()))
    }

    async fn request_passkey(&self, device: OwnedObjectPath) -> Result<u32, Refusal> {
        let device = device.to_string();
        let code = self
            .ask(|answer| Question::Pin { device, answer })
            .await?
            .ok_or_else(|| Refusal::Rejected("no code".into()))?;
        code.trim()
            .parse()
            .map_err(|_| Refusal::Rejected("the code is a number".into()))
    }

    fn display_pin_code(&self, device: OwnedObjectPath, code: String) {
        let _ = self.questions.send(Question::Show {
            device: device.to_string(),
            code,
        });
    }

    fn display_passkey(&self, device: OwnedObjectPath, passkey: u32, _entered: u16) {
        let _ = self.questions.send(Question::Show {
            device: device.to_string(),
            code: format!("{passkey:06}"),
        });
    }

    async fn request_confirmation(
        &self,
        device: OwnedObjectPath,
        passkey: u32,
    ) -> Result<(), Refusal> {
        let device = device.to_string();
        if self
            .ask(|answer| Question::Confirm {
                device,
                code: passkey,
                answer,
            })
            .await?
        {
            Ok(())
        } else {
            Err(Refusal::Rejected("the codes don't match".into()))
        }
    }

    async fn request_authorization(&self, device: OwnedObjectPath) -> Result<(), Refusal> {
        let device = device.to_string();
        if self
            .ask(|answer| Question::Allow { device, answer })
            .await?
        {
            Ok(())
        } else {
            Err(Refusal::Rejected("not allowed".into()))
        }
    }

    /// A paired device using a service, like audio: BlueZ only asks for
    /// devices that aren't trusted, and Mochi trusts what it pairs.
    fn authorize_service(&self, _device: OwnedObjectPath, _uuid: String) {}

    fn cancel(&self) {
        let _ = self.questions.send(Question::Cancel);
    }
}

/// Serves the agent on the connection. [`register`] makes BlueZ use it.
pub async fn serve(
    connection: &Connection,
    questions: UnboundedSender<Question>,
) -> zbus::Result<()> {
    connection
        .object_server()
        .at(PATH, Agent { questions })
        .await?;
    Ok(())
}

/// Tells BlueZ to ask Mochi, as the default agent. Again after BlueZ
/// restarts.
pub async fn register(connection: &Connection) -> zbus::Result<()> {
    let manager = zbus::Proxy::new(
        connection,
        "org.bluez",
        "/org/bluez",
        "org.bluez.AgentManager1",
    )
    .await?;
    let path = ObjectPath::try_from(PATH)?;
    // Already registered on this connection: fine.
    let _ = manager
        .call_method("RegisterAgent", &(&path, "KeyboardDisplay"))
        .await;
    manager
        .call_method("RequestDefaultAgent", &(&path,))
        .await?;
    Ok(())
}
