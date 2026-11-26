use std::{collections::HashMap, time::Duration};

use anyhow::{Context as _, Result, bail};
use futures_lite::{StreamExt as _, future};
use gpui::{BackgroundExecutor, Task, Window};
use zbus::{
    Connection, Proxy,
    proxy::{Builder, CacheProperties},
    zvariant::{OwnedObjectPath, Value},
};

use super::{
    AC_ONLY_BATTERY_LABEL, BatteryPowerState, IDLE_INHIBITION_REASON, format_battery_status,
    update_idle_inhibition,
};

const UPOWER: &str = "org.freedesktop.UPower";
const UPOWER_PATH: &str = "/org/freedesktop/UPower";
const ON_BATTERY: &str = "OnBattery";
const PORTAL_IDLE: u32 = 8;
const PORTAL_SUSPEND: u32 = 4;

/// Reads the system battery from UPower's combined display device.
pub(super) async fn read_battery_status(system_bus: &mut Option<Connection>) -> Option<String> {
    match read_upower_battery(system_bus).await {
        Ok(status) => status,
        Err(error) => {
            log::debug!("could not read local battery status: {error}");
            None
        }
    }
}

async fn read_upower_battery(system_bus: &mut Option<Connection>) -> zbus::Result<Option<String>> {
    const UPOWER_CHARGING: u32 = 1;
    let connection = match system_bus {
        Some(connection) if !connection.is_closed() => connection,
        _ => system_bus.insert(
            zbus::connection::Builder::system()?
                .method_timeout(Duration::from_secs(2))
                .build()
                .await?,
        ),
    };
    let power = Builder::<Proxy<'_>>::new(connection)
        .destination(UPOWER)?
        .path(UPOWER_PATH)?
        .interface(UPOWER)?
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    let display = Builder::<Proxy<'_>>::new(connection)
        .destination(UPOWER)?
        .path("/org/freedesktop/UPower/devices/DisplayDevice")?
        .interface("org.freedesktop.UPower.Device")?
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    let on_battery = power.get_property::<bool>(ON_BATTERY).await?;
    if !display.get_property::<bool>("IsPresent").await? {
        return Ok((!on_battery).then(|| AC_ONLY_BATTERY_LABEL.into()));
    }
    let percent = display.get_property::<f64>("Percentage").await?;
    let charging = display.get_property::<u32>("State").await? == UPOWER_CHARGING;
    let state = match (charging, on_battery) {
        (true, _) => BatteryPowerState::Charging,
        (false, true) => BatteryPowerState::OnBattery,
        (false, false) => BatteryPowerState::PluggedIn,
    };
    Ok(format_battery_status(Some(percent), Some(state)))
}

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
    let power = Proxy::new(&system, UPOWER, UPOWER_PATH, UPOWER).await?;
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
