//! Glances collector: host metrics from the Glances REST API version 4.

use anyhow::Context as _;
use serde::Deserialize;

use crate::{
    metrics::collectors::collector_tailscale::TailnetHostCollector,
    metrics::status_collector::is_virtual_network_interface,
    metrics::status_model::{
        FeedData, HOST_PROCESS_LIMIT, HostMetrics, HostPresence, HostProcess,
        busiest_host_processes,
    },
};

/// Glances collector for one host.
#[derive(Clone, Debug)]
pub struct GlancesCollector {
    pub base_url: String,
    pub agent: ureq::Agent,
    /// Whether the host offers the btop view and needs process readings.
    pub show_btop: bool,
    /// Tailscale presence for the card when Glances does not answer.
    pub presence: Option<TailnetHostCollector>,
}

#[derive(Deserialize)]
struct GlancesCpu {
    total: f32,
}

#[derive(Deserialize)]
struct GlancesCoreCpu {
    cpu_number: usize,
    total: f32,
}

#[derive(Deserialize)]
pub(crate) struct GlancesInterface {
    pub interface_name: String,
    /// Bytes received and sent since Glances started. `bytes_all` counts
    /// only the last update interval, so it changes with each poll.
    pub bytes_all_gauge: u64,
    pub bytes_recv_rate_per_sec: Option<f64>,
    pub bytes_sent_rate_per_sec: Option<f64>,
}

#[derive(Deserialize)]
struct GlancesProcess {
    pid: u32,
    name: String,
    cpu_percent: Option<f32>,
    memory_info: Option<GlancesProcessMemory>,
}

#[derive(Deserialize)]
struct GlancesProcessMemory {
    rss: u64,
}

#[derive(Deserialize)]
struct GlancesMemory {
    total: u64,
    used: u64,
}

#[derive(Deserialize)]
struct GlancesLoad {
    min1: f32,
}

#[derive(Deserialize)]
pub(crate) struct GlancesSensor {
    pub label: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub value: f32,
}

#[derive(Deserialize)]
pub(crate) struct GlancesFilesystem {
    pub size: u64,
    pub used: u64,
}

impl GlancesCollector {
    fn get<T: serde::de::DeserializeOwned>(&self, plugin: &str) -> anyhow::Result<T> {
        let url = format!("{}/api/4/{plugin}", self.base_url.trim_end_matches('/'));
        self.agent
            .get(&url)
            .call()
            .with_context(|| format!("Glances request failed: {plugin}"))?
            .body_mut()
            .read_json::<T>()
            .with_context(|| format!("Glances response is not valid: {plugin}"))
    }

    /// Reads host metrics. When Glances does not answer and Tailscale knows
    /// the host, reads its presence instead: a laptop that sleeps, or a host
    /// without Glances yet, is a normal state and not a failed feed. This
    /// call blocks; run it on a background thread.
    pub fn fetch_glances_reading(&self) -> anyhow::Result<FeedData> {
        let error = match self.fetch_glances_metrics() {
            Ok(metrics) => return Ok(FeedData::Host(metrics)),
            Err(error) => error,
        };
        match self
            .presence
            .as_ref()
            .and_then(|presence| presence.reader.read_host_presence(&presence.name).ok())
        {
            Some(device) => Ok(FeedData::HostPresence(HostPresence {
                device,
                metrics_error: Some(format!("{error:#}")),
            })),
            None => Err(error),
        }
    }

    fn fetch_glances_metrics(&self) -> anyhow::Result<HostMetrics> {
        let cpu: GlancesCpu = self.get("cpu")?;
        let memory: GlancesMemory = self.get("mem")?;
        let load: Option<GlancesLoad> = self.get("load").ok();
        let sensors: Vec<GlancesSensor> = self.get("sensors").unwrap_or_default();
        let filesystems: Vec<GlancesFilesystem> = self.get("fs").unwrap_or_default();
        let uptime: String = self.get("uptime").unwrap_or_default();
        let mut cores: Vec<GlancesCoreCpu> = self.get("percpu").unwrap_or_default();
        cores.sort_by_key(|core| core.cpu_number);
        let interfaces: Vec<GlancesInterface> = self.get("network").unwrap_or_default();
        let interface = main_network_interface(&interfaces);
        // The top endpoint returns only the busiest processes, not the
        // full process list.
        let processes: Vec<GlancesProcess> = if self.show_btop {
            self.get(&format!("processlist/top/{HOST_PROCESS_LIMIT}"))
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        let disk = main_filesystem(&filesystems);
        Ok(HostMetrics {
            cpu_percent: Some(cpu.total),
            core_percents: cores.into_iter().map(|core| core.total).collect(),
            memory_used_bytes: Some(memory.used),
            memory_total_bytes: Some(memory.total),
            disk_used_bytes: disk.map(|fs| fs.used),
            disk_total_bytes: disk.map(|fs| fs.size),
            temperature_celsius: cpu_temperature(&sensors),
            load_1m: load.map(|load| load.min1),
            uptime_seconds: parse_glances_uptime(&uptime),
            network_receive_rate: interface.and_then(|i| i.bytes_recv_rate_per_sec),
            network_send_rate: interface.and_then(|i| i.bytes_sent_rate_per_sec),
            processes: busiest_host_processes(processes.into_iter().map(|process| HostProcess {
                pid: process.pid,
                name: process.name,
                cpu_percent: process.cpu_percent.unwrap_or_default(),
                memory_bytes: process.memory_info.map_or(0, |memory| memory.rss),
            })),
        })
    }
}

/// Picks the main network interface: the physical one with the most traffic.
pub(crate) fn main_network_interface(interfaces: &[GlancesInterface]) -> Option<&GlancesInterface> {
    interfaces
        .iter()
        .filter(|i| !is_virtual_network_interface(&i.interface_name))
        .max_by_key(|i| i.bytes_all_gauge)
}

/// Picks the main filesystem: the largest one. On macOS, the APFS volumes
/// of one container all report the container size, so the volume with the
/// most used space wins: `/System/Volumes/Data`, where user data lives.
pub(crate) fn main_filesystem(filesystems: &[GlancesFilesystem]) -> Option<&GlancesFilesystem> {
    filesystems.iter().max_by_key(|fs| (fs.size, fs.used))
}

/// Picks the CPU temperature from Glances sensors: the CPU package sensor,
/// then the AMD `Tctl` sensor, then the hottest core sensor.
pub(crate) fn cpu_temperature(sensors: &[GlancesSensor]) -> Option<f32> {
    let cores: Vec<&GlancesSensor> = sensors
        .iter()
        .filter(|s| s.kind == "temperature_core")
        .collect();
    cores
        .iter()
        .find(|s| s.label.starts_with("Package id"))
        .or_else(|| cores.iter().find(|s| s.label == "Tctl"))
        .map(|s| s.value)
        .or_else(|| {
            cores
                .iter()
                .filter(|s| s.label.starts_with("Core"))
                .map(|s| s.value)
                .reduce(f32::max)
        })
}

/// Parses a Glances uptime, such as `2:35:25` or `3 days, 2:35:25`.
pub(crate) fn parse_glances_uptime(text: &str) -> Option<u64> {
    let (days, clock) = match text.split_once(", ") {
        Some((days, clock)) => (days.split_whitespace().next()?.parse::<u64>().ok()?, clock),
        None => (0, text),
    };
    let parts: Vec<u64> = clock
        .split(':')
        .map(|part| part.trim().parse().ok())
        .collect::<Option<_>>()?;
    let [hours, minutes, seconds] = parts[..] else {
        return None;
    };
    Some(days * 86_400 + hours * 3_600 + minutes * 60 + seconds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{
        collectors::collector_tailscale::TailscaleStatusReader,
        status_model::{TailnetConnection, TailnetDevice},
    };

    #[test]
    fn glances_falls_back_to_tailscale_presence() {
        let asleep = TailnetDevice {
            name: "laptop".into(),
            os: "macOS".into(),
            online: false,
            is_self: false,
            last_seen: None,
            connection: TailnetConnection::Offline,
        };
        let collector = |presence| GlancesCollector {
            show_btop: false,
            // Port 9 on the loopback address refuses the connection.
            base_url: "http://127.0.0.1:9".into(),
            agent: ureq::Agent::new_with_defaults(),
            presence,
        };
        let device = TailnetHostCollector {
            name: "laptop".into(),
            reader: TailscaleStatusReader::tailscale_reader_with_devices(vec![asleep.clone()]),
        };
        let Ok(FeedData::HostPresence(presence)) = collector(Some(device)).fetch_glances_reading()
        else {
            panic!("expected the Tailscale presence of the host");
        };
        assert_eq!(presence.device, asleep);
        assert!(presence.metrics_error.is_some());
        // A host without a configured fallback keeps the Glances error.
        assert!(collector(None).fetch_glances_reading().is_err());
    }

    fn sensor(label: &str, value: f32) -> GlancesSensor {
        GlancesSensor {
            label: label.into(),
            kind: "temperature_core".into(),
            value,
        }
    }

    fn interface(name: &str, bytes_all_gauge: u64) -> GlancesInterface {
        GlancesInterface {
            interface_name: name.into(),
            bytes_all_gauge,
            bytes_recv_rate_per_sec: Some(1.0),
            bytes_sent_rate_per_sec: Some(2.0),
        }
    }

    #[test]
    fn picks_busiest_physical_interface() {
        let interfaces = [
            interface("lo", 900),
            interface("tailscale0", 800),
            interface("docker0", 700),
            interface("eth0", 500),
            interface("wlan0", 100),
        ];
        let main = main_network_interface(&interfaces).unwrap();
        assert_eq!(main.interface_name, "eth0");
        // macOS: Tailscale and Apple wireless links are not physical.
        let interfaces = [
            interface("utun4", 900),
            interface("awdl0", 800),
            interface("llw0", 700),
            interface("en1", 500),
        ];
        let main = main_network_interface(&interfaces).unwrap();
        assert_eq!(main.interface_name, "en1");
    }

    #[test]
    fn picks_the_apfs_data_volume() {
        let fs = |size: u64, used: u64| GlancesFilesystem { size, used };
        // `/`, `/System/Volumes/Data`, and `/System/Volumes/Update/mnt1`.
        let filesystems = [fs(460, 15), fs(460, 197), fs(460, 15), fs(4, 2)];
        assert_eq!(main_filesystem(&filesystems).unwrap().used, 197);
    }

    #[test]
    fn parses_glances_uptime_formats() {
        assert_eq!(parse_glances_uptime("2:35:25"), Some(9_325));
        assert_eq!(parse_glances_uptime("3 days, 2:35:25"), Some(268_525));
        assert_eq!(parse_glances_uptime("1 day, 0:00:01"), Some(86_401));
        assert_eq!(parse_glances_uptime(""), None);
    }

    #[test]
    fn prefers_package_temperature() {
        let sensors = [sensor("Core 0", 41.0), sensor("Package id 0", 49.0)];
        assert_eq!(cpu_temperature(&sensors), Some(49.0));
        let cores = [
            sensor("Core 0", 41.0),
            sensor("Core 1", 52.0),
            sensor("acpitz 0", 27.0),
        ];
        assert_eq!(cpu_temperature(&cores), Some(52.0));
        assert_eq!(cpu_temperature(&[sensor("acpitz 0", 27.0)]), None);
    }
}
