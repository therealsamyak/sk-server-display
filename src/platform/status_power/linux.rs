use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use ashpd::{
    desktop::inhibit::{InhibitFlags, InhibitOptions, InhibitProxy},
    zbus::{
        self, Connection, Proxy,
        proxy::{Builder, CacheProperties},
    },
};
use futures_lite::{StreamExt as _, future::race};
use gpui::{BackgroundExecutor, Context, Task};

use super::{
    AC_ONLY_BATTERY_LABEL, BatteryPowerState, BatteryReading, IDLE_INHIBITION_REASON,
    IdlePowerState, StatusPower, format_battery_status,
};

const UPOWER: &str = "org.freedesktop.UPower";
const UPOWER_PATH: &str = "/org/freedesktop/UPower";
const ON_BATTERY: &str = "OnBattery";

/// Reads the system battery from UPower's combined display device.
pub(super) async fn read_battery_status(
    system_bus: &mut Option<Connection>,
) -> Option<BatteryReading> {
    match read_upower_battery(system_bus).await {
        Ok(status) => status,
        Err(error) => {
            log::debug!("could not read local battery status: {error}");
            None
        }
    }
}

async fn read_upower_battery(
    system_bus: &mut Option<Connection>,
) -> zbus::Result<Option<BatteryReading>> {
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
        return Ok((!on_battery).then(|| BatteryReading::new(AC_ONLY_BATTERY_LABEL.into())));
    }
    let percent = display.get_property::<f64>("Percentage").await?;
    let charging = display.get_property::<u32>("State").await? == UPOWER_CHARGING;
    let state = match (charging, on_battery) {
        (true, _) => BatteryPowerState::Charging,
        (false, true) => BatteryPowerState::OnBattery,
        (false, false) => BatteryPowerState::PluggedIn,
    };
    let Some(label) = format_battery_status(Some(percent), Some(state)) else {
        return Ok(None);
    };
    let mut reading = BatteryReading::new(label);
    reading.on_battery = on_battery;
    reading.net_watts = display
        .get_property::<f64>("EnergyRate")
        .await
        .ok()
        .filter(|rate| rate.is_finite() && *rate >= 0.0)
        .and_then(|rate| match state {
            BatteryPowerState::Charging => Some(rate),
            BatteryPowerState::OnBattery => Some(-rate),
            BatteryPowerState::PluggedIn => (rate == 0.0).then_some(0.0),
        });
    if on_battery {
        reading.remaining_seconds = display
            .get_property::<i64>("TimeToEmpty")
            .await
            .ok()
            .and_then(|seconds| u64::try_from(seconds).ok())
            .filter(|seconds| *seconds > 0);
    }
    Ok(Some(reading))
}

/// Owns the desktop idle inhibition request.
pub(super) struct DisplayPowerMonitor {
    _monitor: Task<()>,
}

impl DisplayPowerMonitor {
    /// Requests idle inhibition and reports its state to the toolbar.
    pub(super) fn start(cx: &Context<StatusPower>) -> Self {
        let executor = cx.background_executor().clone();
        let monitor = cx.spawn(async move |power, cx| {
            let error = match request_idle_inhibition(&executor).await {
                Ok((session, mut owner_changes)) => {
                    if power
                        .update(cx, |power, cx| {
                            power.state = IdlePowerState::Active;
                            cx.notify();
                        })
                        .is_err()
                    {
                        return;
                    }
                    race(session.closed(), async {
                        owner_changes.next().await;
                    })
                    .await;
                    anyhow::anyhow!("desktop idle inhibition connection closed")
                }
                Err(error) => error,
            };
            log::warn!("idle inhibition unavailable: {error:#}");
            if let Err(error) = power.update(cx, |power, cx| {
                power.monitor = None;
                power.state = IdlePowerState::Failed;
                cx.notify();
            }) {
                log::debug!("idle inhibition view closed: {error}");
            }
        });
        Self { _monitor: monitor }
    }
}

async fn request_idle_inhibition(
    executor: &BackgroundExecutor,
) -> Result<(Connection, zbus::proxy::OwnerChangedStream<'static>)> {
    // The dedicated connection releases the request when the task is cancelled.
    let acquire = async {
        let session = Connection::session()
            .await
            .context("could not open the desktop session bus")?;
        let portal = InhibitProxy::with_connection(session.clone()).await?;
        let owner_changes = portal.receive_owner_changed().await?;
        portal
            .inhibit(
                None,
                InhibitFlags::Idle | InhibitFlags::Suspend,
                InhibitOptions::default().set_reason(IDLE_INHIBITION_REASON),
            )
            .await?
            .response()
            .context("desktop portal rejected idle inhibition")?;
        Ok((session, owner_changes))
    };
    race(acquire, async {
        executor.timer(Duration::from_secs(5)).await;
        bail!("desktop portal did not confirm idle inhibition")
    })
    .await
}
