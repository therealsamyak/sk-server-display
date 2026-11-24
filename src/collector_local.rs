//! Local collector: host metrics of the machine that runs the display.

use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Instant,
};

use sysinfo::{Components, DiskRefreshKind, Disks, Networks, System};

use crate::{status_collector::is_virtual_network_interface, status_model::HostMetrics};

/// Local collector state. CPU usage and network rates need the previous
/// reading for a time delta. The disk and sensor lists stay loaded, so each
/// poll refreshes values only. On Windows this keeps one WMI connection for
/// the temperature sensors instead of one connection for each poll.
struct LocalSampler {
    system: System,
    networks: Networks,
    networks_read_at: Instant,
    disks: Disks,
    components: Option<Components>,
    host_name: String,
}

/// Local collector for the machine that runs the display.
#[derive(Clone)]
pub struct LocalCollector {
    sampler: Arc<Mutex<LocalSampler>>,
}

impl std::fmt::Debug for LocalCollector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LocalCollector")
    }
}

impl LocalCollector {
    /// Creates the local collector.
    pub fn new_local_collector() -> anyhow::Result<Self> {
        let mut system = System::new()?;
        system.refresh_cpu_all();
        Ok(Self {
            sampler: Arc::new(Mutex::new(LocalSampler {
                system,
                networks: Networks::new_with_refreshed_list()?,
                networks_read_at: Instant::now(),
                disks: Disks::new_with_refreshed_list_specifics(
                    DiskRefreshKind::nothing().with_storage(),
                )?,
                components: Components::new_with_refreshed_list().ok(),
                host_name: System::host_name().unwrap_or_else(|_| "this machine".into()),
            })),
        })
    }

    /// Reads local host metrics. This call blocks; run it on a background thread.
    pub fn fetch_local_metrics(&self) -> anyhow::Result<HostMetrics> {
        let mut sampler = self
            .sampler
            .lock()
            .map_err(|_| anyhow::anyhow!("local metrics lock is poisoned"))?;
        let sampler = &mut *sampler;
        sampler.system.refresh_cpu_usage();
        sampler.system.refresh_memory();
        sampler.networks.refresh(true);
        let seconds = sampler.networks_read_at.elapsed().as_secs_f64().max(0.001);
        sampler.networks_read_at = Instant::now();
        let interface = sampler
            .networks
            .iter()
            .filter(|(name, _)| !is_virtual_network_interface(&name.to_string_lossy()))
            .max_by_key(|(_, data)| data.total_received() + data.total_transmitted())
            .map(|(_, data)| data);

        // macOS keeps user data on the APFS Data volume. Its `/` volume is the
        // sealed system volume with a small fixed size in use.
        let system_disk = ["/System/Volumes/Data", "/", "C:\\"]
            .into_iter()
            .find_map(|mount| {
                let mount = Path::new(mount);
                sampler
                    .disks
                    .iter()
                    .position(|disk| disk.mount_point() == mount)
            })
            .map(|index| &mut sampler.disks[index]);
        let system_disk = system_disk.map(|disk| {
            disk.refresh_specifics(DiskRefreshKind::nothing().with_storage());
            (disk.total_space(), disk.available_space())
        });
        let temperature = sampler.components.as_mut().and_then(|components| {
            components.refresh(false);
            components
                .iter()
                .filter(|c| {
                    let label = c.label().to_lowercase();
                    label.contains("package") || label.contains("tctl") || label.contains("cpu")
                })
                .filter_map(|c| c.temperature())
                .reduce(f32::max)
        });
        // Windows has no load average.
        let load = System::load_average().ok().map(|load| load.one as f32);
        let system = &sampler.system;

        Ok(HostMetrics {
            name: sampler.host_name.clone(),
            cpu_percent: Some(system.global_cpu_usage()),
            core_percents: system.cpus().iter().map(|cpu| cpu.usage()).collect(),
            memory_used_bytes: Some(system.used_memory()),
            memory_total_bytes: Some(system.total_memory()),
            disk_used_bytes: system_disk.map(|(total, available)| total - available),
            disk_total_bytes: system_disk.map(|(total, _)| total),
            temperature_celsius: temperature,
            load_1m: load.filter(|load| cfg!(not(target_os = "windows")) || *load > 0.0),
            uptime_seconds: System::uptime().ok(),
            network_receive_rate: interface.map(|data| data.received() as f64 / seconds),
            network_send_rate: interface.map(|data| data.transmitted() as f64 / seconds),
        })
    }
}
