//! Glances collector: host metrics from the Glances REST API version 4.

use anyhow::Context as _;
use serde::Deserialize;

use crate::{
    collector_tailscale::TailscaleStatusReader,
    status_collector::is_virtual_network_interface,
    status_model::{FeedData, HostMetrics, HostPresence},
};

/// Glances collector for one host.
#[derive(Clone, Debug)]
pub struct GlancesCollector {
    pub name: String,
    pub base_url: String,
    pub agent: ureq::Agent,
    /// Tailscale presence for the card when Glances does not answer.
    pub tailscale: TailscaleStatusReader,
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
    pub bytes_all: u64,
    pub bytes_recv_rate_per_sec: Option<f64>,
    pub bytes_sent_rate_per_sec: Option<f64>,
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
        match self.tailscale.read_host_presence(&self.name) {
            Ok(device) => Ok(FeedData::HostPresence(HostPresence {
                device,
                metrics_error: Some(format!("{error:#}")),
            })),
            Err(_) => Err(error),
        }
    }

    fn fetch_glances_metrics(&self) -> anyhow::Result<HostMetrics> {
        let cpu: GlancesCpu = self.get("cpu")?;
        let memory: GlancesMemory = self.get("mem")?;
        let load: GlancesLoad = self.get("load")?;
        let sensors: Vec<GlancesSensor> = self.get("sensors").unwrap_or_default();
        let filesystems: Vec<GlancesFilesystem> = self.get("fs").unwrap_or_default();
        let uptime: String = self.get("uptime").unwrap_or_default();
        let mut cores: Vec<GlancesCoreCpu> = self.get("percpu").unwrap_or_default();
        cores.sort_by_key(|core| core.cpu_number);
        let interfaces: Vec<GlancesInterface> = self.get("network").unwrap_or_default();
        let interface = main_network_interface(&interfaces);

        let disk = filesystems.iter().max_by_key(|fs| fs.size);
        Ok(HostMetrics {
            name: self.name.clone(),
            cpu_percent: Some(cpu.total),
            core_percents: cores.into_iter().map(|core| core.total).collect(),
            memory_used_bytes: Some(memory.used),
            memory_total_bytes: Some(memory.total),
            disk_used_bytes: disk.map(|fs| fs.used),
            disk_total_bytes: disk.map(|fs| fs.size),
            temperature_celsius: cpu_temperature(&sensors),
            load_1m: Some(load.min1),
            uptime_seconds: parse_glances_uptime(&uptime),
            network_receive_rate: interface.and_then(|i| i.bytes_recv_rate_per_sec),
            network_send_rate: interface.and_then(|i| i.bytes_sent_rate_per_sec),
        })
    }
}

/// Picks the main network interface: the physical one with the most traffic.
pub(crate) fn main_network_interface(interfaces: &[GlancesInterface]) -> Option<&GlancesInterface> {
    interfaces
        .iter()
        .filter(|i| !is_virtual_network_interface(&i.interface_name))
        .max_by_key(|i| i.bytes_all)
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
    use crate::status_model::{TailnetConnection, TailnetDevice};

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
        let collector = |name: &str| GlancesCollector {
            name: name.into(),
            // Port 9 on the loopback address refuses the connection.
            base_url: "http://127.0.0.1:9".into(),
            agent: ureq::Agent::new_with_defaults(),
            tailscale: TailscaleStatusReader::tailscale_reader_with_devices(vec![asleep.clone()]),
        };
        let Ok(FeedData::HostPresence(presence)) = collector("laptop").fetch_glances_reading()
        else {
            panic!("expected the Tailscale presence of the host");
        };
        assert_eq!(presence.device, asleep);
        assert!(presence.metrics_error.is_some());
        // A host that Tailscale does not know keeps the Glances error.
        assert!(collector("missing").fetch_glances_reading().is_err());
    }

    fn sensor(label: &str, value: f32) -> GlancesSensor {
        GlancesSensor {
            label: label.into(),
            kind: "temperature_core".into(),
            value,
        }
    }

    fn interface(name: &str, bytes_all: u64) -> GlancesInterface {
        GlancesInterface {
            interface_name: name.into(),
            bytes_all,
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
