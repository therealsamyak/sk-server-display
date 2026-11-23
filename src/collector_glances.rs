//! Glances collector: host metrics from the Glances REST API version 4.

use anyhow::Context as _;
use serde::Deserialize;

use crate::status_model::HostMetrics;

/// Glances collector for one host.
#[derive(Clone, Debug)]
pub struct GlancesCollector {
    pub name: String,
    pub base_url: String,
    pub agent: ureq::Agent,
}

#[derive(Deserialize)]
struct GlancesCpu {
    total: f32,
    cpucore: Option<usize>,
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

#[derive(Deserialize)]
struct GlancesQuicklook {
    cpu_name: Option<String>,
}

#[derive(Deserialize)]
struct GlancesSystem {
    hr_name: Option<String>,
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

    /// Reads host metrics. This call blocks; run it on a background thread.
    pub fn fetch_glances_metrics(&self) -> anyhow::Result<HostMetrics> {
        let cpu: GlancesCpu = self.get("cpu")?;
        let memory: GlancesMemory = self.get("mem")?;
        let load: GlancesLoad = self.get("load")?;
        let sensors: Vec<GlancesSensor> = self.get("sensors").unwrap_or_default();
        let filesystems: Vec<GlancesFilesystem> = self.get("fs").unwrap_or_default();
        let uptime: String = self.get("uptime").unwrap_or_default();
        let quicklook: Option<GlancesQuicklook> = self.get("quicklook").ok();
        let system: Option<GlancesSystem> = self.get("system").ok();

        let disk = filesystems.iter().max_by_key(|fs| fs.size);
        let detail = [
            system.and_then(|s| s.hr_name),
            quicklook.and_then(|q| q.cpu_name),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
        Ok(HostMetrics {
            name: self.name.clone(),
            detail,
            cpu_percent: Some(cpu.total),
            cpu_cores: cpu.cpucore,
            memory_used_bytes: Some(memory.used),
            memory_total_bytes: Some(memory.total),
            disk_used_bytes: disk.map(|fs| fs.used),
            disk_total_bytes: disk.map(|fs| fs.size),
            temperature_celsius: cpu_temperature(&sensors),
            load_1m: Some(load.min1),
            uptime_seconds: parse_glances_uptime(&uptime),
        })
    }
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

    fn sensor(label: &str, value: f32) -> GlancesSensor {
        GlancesSensor {
            label: label.into(),
            kind: "temperature_core".into(),
            value,
        }
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
