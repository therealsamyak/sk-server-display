//! Local collector: host metrics of the machine that runs the display.

use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use sysinfo::{Components, Disks, System};

use crate::status_model::HostMetrics;

/// Local collector. It keeps one `System` so CPU usage has a time delta.
#[derive(Clone)]
pub struct LocalCollector {
    system: Arc<Mutex<System>>,
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
            system: Arc::new(Mutex::new(system)),
        })
    }

    /// Reads local host metrics. This call blocks; run it on a background thread.
    pub fn fetch_local_metrics(&self) -> anyhow::Result<HostMetrics> {
        let mut system = self
            .system
            .lock()
            .map_err(|_| anyhow::anyhow!("local metrics lock is poisoned"))?;
        system.refresh_cpu_usage();
        system.refresh_memory();
        let cpu_model = system
            .cpus()
            .first()
            .map(|cpu| cpu.brand().trim().to_string())
            .unwrap_or_default();

        let disks = Disks::new_with_refreshed_list()?;
        let system_disk = disks.iter().find(|disk| {
            let mount = disk.mount_point();
            mount == Path::new("/") || mount == Path::new("C:\\")
        });
        let temperature = Components::new_with_refreshed_list()
            .ok()
            .and_then(|components| {
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

        Ok(HostMetrics {
            name: System::host_name().unwrap_or_else(|_| "this machine".into()),
            detail: [System::long_os_version().ok(), Some(cpu_model)]
                .into_iter()
                .flatten()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" · "),
            cpu_percent: Some(system.global_cpu_usage()),
            cpu_cores: Some(system.cpus().len()),
            memory_used_bytes: Some(system.used_memory()),
            memory_total_bytes: Some(system.total_memory()),
            disk_used_bytes: system_disk.map(|d| d.total_space() - d.available_space()),
            disk_total_bytes: system_disk.map(|d| d.total_space()),
            temperature_celsius: temperature,
            load_1m: load.filter(|load| cfg!(not(target_os = "windows")) || *load > 0.0),
            uptime_seconds: System::uptime().ok(),
        })
    }
}
