//! Scrutiny collector: SMART disk health from the Scrutiny summary API.

use anyhow::Context as _;
use serde::Deserialize;
use std::collections::HashMap;
use time::OffsetDateTime;

use crate::status_model::{DiskHealth, DiskState};

/// Scrutiny collector for one Scrutiny web server.
#[derive(Clone, Debug)]
pub struct ScrutinyCollector {
    pub base_url: String,
    pub agent: ureq::Agent,
}

#[derive(Deserialize)]
pub(crate) struct ScrutinySummaryResponse {
    pub data: ScrutinySummaryData,
}

#[derive(Deserialize)]
pub(crate) struct ScrutinySummaryData {
    pub summary: HashMap<String, ScrutinyDeviceSummary>,
}

#[derive(Deserialize)]
pub(crate) struct ScrutinyDeviceSummary {
    pub device: ScrutinyDevice,
    #[serde(default)]
    pub smart: Option<ScrutinySmart>,
}

#[derive(Deserialize)]
pub(crate) struct ScrutinyDevice {
    pub device_name: String,
    #[serde(default)]
    pub model_name: String,
    #[serde(default)]
    pub capacity: u64,
    #[serde(default)]
    pub device_status: u8,
}

#[derive(Deserialize)]
pub(crate) struct ScrutinySmart {
    #[serde(default)]
    pub temp: Option<f32>,
    #[serde(default)]
    pub power_on_hours: Option<u64>,
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub collector_date: Option<OffsetDateTime>,
}

impl ScrutinyCollector {
    /// Reads disk health. This call blocks; run it on a background thread.
    pub fn fetch_scrutiny_disks(&self) -> anyhow::Result<Vec<DiskHealth>> {
        let url = format!("{}/api/summary", self.base_url.trim_end_matches('/'));
        let response: ScrutinySummaryResponse = self
            .agent
            .get(&url)
            .call()
            .context("Scrutiny request failed")?
            .body_mut()
            .read_json()
            .context("Scrutiny response is not valid")?;
        Ok(scrutiny_disks(response))
    }
}

/// Converts a Scrutiny summary to disks, sorted by device name.
pub(crate) fn scrutiny_disks(response: ScrutinySummaryResponse) -> Vec<DiskHealth> {
    let mut disks: Vec<DiskHealth> = response
        .data
        .summary
        .into_values()
        .map(|summary| DiskHealth {
            device: summary.device.device_name,
            model: summary.device.model_name,
            capacity_bytes: summary.device.capacity,
            state: match summary.device.device_status {
                0 => DiskState::Passed,
                1 => DiskState::FailedSmart,
                2 => DiskState::FailedScrutiny,
                _ => DiskState::FailedBoth,
            },
            temperature_celsius: summary.smart.as_ref().and_then(|s| s.temp),
            power_on_hours: summary.smart.as_ref().and_then(|s| s.power_on_hours),
            collected_at: summary.smart.and_then(|s| s.collector_date),
        })
        .collect();
    disks.sort_by(|a, b| a.device.cmp(&b.device));
    disks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_summary_and_status_codes() {
        let response: ScrutinySummaryResponse = serde_json::from_str(
            r#"{"success": true, "data": {"summary": {
                "wwn-b": {"device": {"device_name": "sdb", "model_name": "HDD B", "capacity": 1000, "device_status": 2},
                          "smart": {"temp": 38, "power_on_hours": 25407, "collector_date": "2026-11-23T19:00:22Z"}},
                "wwn-a": {"device": {"device_name": "sda", "model_name": "HDD A", "capacity": 2000, "device_status": 0}}
            }}}"#,
        )
        .unwrap();
        let disks = scrutiny_disks(response);
        assert_eq!(disks[0].device, "sda");
        assert_eq!(disks[0].state, DiskState::Passed);
        assert_eq!(disks[0].temperature_celsius, None);
        assert_eq!(disks[1].state, DiskState::FailedScrutiny);
        assert_eq!(disks[1].power_on_hours, Some(25_407));
    }
}
