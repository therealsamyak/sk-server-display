use std::collections::HashMap;

use anyhow::{Context as _, Result, bail};
use futures_lite::{StreamExt as _, future};
use gpui::{BackgroundExecutor, Task, Window};
use zbus::{
    Connection, Proxy,
    zvariant::{OwnedObjectPath, Value},
};

use super::{IDLE_INHIBITION_REASON, update_idle_inhibition};

const UPOWER: &str = "org.freedesktop.UPower";
const ON_BATTERY: &str = "OnBattery";
const PORTAL_IDLE: u32 = 8;
const PORTAL_SUSPEND: u32 = 4;

/// Owns the desktop idle requests and the UPower subscription.
pub(super) struct DisplayPowerMonitor {
    _monitor: Task<()>,
}

impl DisplayPowerMonitor {
    /// Watches external power while the status window is in display mode.
    pub(super) fn start(_: &Window, executor: &BackgroundExecutor) -> Result<Self> {
        let background = executor.clone();
        let monitor = executor.spawn(async move {
            if let Err(error) = monitor_power(&background).await {
                log::warn!("could not monitor external power: {error:#}");
            }
        });
        Ok(Self { _monitor: monitor })
    }
}

async fn monitor_power(executor: &BackgroundExecutor) -> Result<()> {
    let system = Connection::system()
        .await
        .context("could not open the system bus")?;
    let power = Proxy::new(&system, UPOWER, "/org/freedesktop/UPower", UPOWER).await?;
    let mut owners = power.receive_owner_changed().await?;
    let mut changes = power.receive_property_changed::<bool>(ON_BATTERY).await;
    let watch_power = async {
        let mut inhibitor = None;
        let mut on_battery = power.get_property::<bool>(ON_BATTERY).await?;
        loop {
            update_idle_inhibition(&mut inhibitor, !on_battery, || {
                Ok(executor.spawn(async {
                    if let Err(error) = hold_idle_inhibition().await {
                        log::warn!("idle inhibition unavailable: {error:#}");
                    }
                }))
            });
            let Some(change) = changes.next().await else {
                bail!("UPower stopped reporting power changes");
            };
            on_battery = change.get().await?;
        }
    };
    let watch_owner = async {
        while let Some(Some(_)) = owners.next().await {}
        bail!("UPower disconnected");
    };
    let watch_bus = async {
        system.closed().await;
        bail!("system bus disconnected");
    };
    future::race(watch_power, future::race(watch_owner, watch_bus)).await
}

async fn hold_idle_inhibition() -> Result<()> {
    // A dedicated connection owns this request. The portal releases the inhibition when
    // the connection drops, even when fullscreen ends before the D-Bus call returns.
    let session = Connection::session()
        .await
        .context("could not open the desktop session bus")?;
    let portal = Proxy::new(
        &session,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.Inhibit",
    )
    .await?;
    let options = HashMap::from([("reason", Value::from(IDLE_INHIBITION_REASON))]);
    let _request: OwnedObjectPath = portal
        .call("Inhibit", &("", PORTAL_IDLE | PORTAL_SUSPEND, options))
        .await
        .context("desktop portal could not inhibit display and system idle")?;
    session.closed().await;
    bail!("desktop session bus disconnected");
}
