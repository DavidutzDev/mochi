//! Whether the system prefers light colors, from the desktop portal's
//! `color-scheme` setting, for themes with `appearance = "auto"`.

use futures_util::StreamExt;
use tokio::sync::mpsc::UnboundedSender;
use zbus::proxy;
use zbus::zvariant::OwnedValue;

#[proxy(
    interface = "org.freedesktop.portal.Settings",
    default_service = "org.freedesktop.portal.Desktop",
    default_path = "/org/freedesktop/portal/desktop"
)]
trait Settings {
    fn read_one(&self, namespace: &str, key: &str) -> zbus::Result<OwnedValue>;

    #[zbus(signal)]
    fn setting_changed(&self, namespace: &str, key: &str, value: OwnedValue) -> zbus::Result<()>;
}

const NAMESPACE: &str = "org.freedesktop.appearance";
const KEY: &str = "color-scheme";

/// `color-scheme`: 1 prefers dark, 2 light, 0 says nothing, which counts
/// as dark.
fn light(value: &OwnedValue) -> bool {
    u32::try_from(value).is_ok_and(|scheme| scheme == 2)
}

/// Sends the preference now, then each time it changes. Ends quietly
/// without a portal: themes then stay dark.
pub async fn watch(sender: UnboundedSender<bool>) {
    if let Err(error) = follow(&sender).await {
        tracing::debug!(%error, "no color scheme from the portal");
    }
}

async fn follow(sender: &UnboundedSender<bool>) -> zbus::Result<()> {
    let bus = zbus::Connection::session().await?;
    let settings = SettingsProxy::new(&bus).await?;
    let mut changes = settings.receive_setting_changed().await?;
    if let Ok(value) = settings.read_one(NAMESPACE, KEY).await
        && sender.send(light(&value)).is_err()
    {
        return Ok(());
    }
    while let Some(change) = changes.next().await {
        let Ok(args) = change.args() else {
            continue;
        };
        if args.namespace() == &NAMESPACE
            && args.key() == &KEY
            && sender.send(light(args.value())).is_err()
        {
            return Ok(());
        }
    }
    Ok(())
}
