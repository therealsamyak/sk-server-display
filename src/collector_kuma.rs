//! Uptime Kuma collector: service health from a public status page.
//!
//! It reads the status page API that Uptime Kuma serves without sign-in.
//! It never uses the Kuma dashboard API, so it needs no credentials.

use std::collections::HashMap;

use anyhow::Context as _;
use serde::Deserialize;
use time::{OffsetDateTime, PrimitiveDateTime, macros::format_description};

use crate::status_model::{ServiceHealth, ServiceState};

/// Uptime Kuma collector for one status page.
#[derive(Clone, Debug)]
pub struct KumaCollector {
    pub base_url: String,
    pub status_page: String,
    pub agent: ureq::Agent,
}

#[derive(Deserialize)]
pub(crate) struct KumaStatusPage {
    #[serde(rename = "publicGroupList")]
    pub groups: Vec<KumaGroup>,
}

#[derive(Deserialize)]
pub(crate) struct KumaGroup {
    #[serde(rename = "monitorList")]
    pub monitors: Vec<KumaMonitor>,
}

#[derive(Deserialize)]
pub(crate) struct KumaMonitor {
    pub id: u64,
    pub name: String,
}

#[derive(Deserialize)]
pub(crate) struct KumaHeartbeats {
    #[serde(rename = "heartbeatList")]
    pub heartbeats: HashMap<String, Vec<KumaHeartbeat>>,
    #[serde(rename = "uptimeList")]
    pub uptimes: HashMap<String, f32>,
}

#[derive(Deserialize)]
pub(crate) struct KumaHeartbeat {
    pub status: u8,
    pub time: String,
    #[serde(default)]
    pub msg: Option<String>,
}

impl KumaCollector {
    fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        let url = format!(
            "{}{path}{}",
            self.base_url.trim_end_matches('/'),
            self.status_page
        );
        self.agent
            .get(&url)
            .call()
            .context("Uptime Kuma request failed")?
            .body_mut()
            .read_json::<T>()
            .context("Uptime Kuma response is not valid")
    }

    /// Reads service health. This call blocks; run it on a background thread.
    pub fn fetch_kuma_services(&self) -> anyhow::Result<Vec<ServiceHealth>> {
        let page: KumaStatusPage = self.get("/api/status-page/")?;
        let heartbeats: KumaHeartbeats = self.get("/api/status-page/heartbeat/")?;
        Ok(merge_kuma_services(&page, &heartbeats))
    }
}

/// Joins status page monitors with their latest heartbeat and 24-hour uptime.
pub(crate) fn merge_kuma_services(
    page: &KumaStatusPage,
    heartbeats: &KumaHeartbeats,
) -> Vec<ServiceHealth> {
    page.groups
        .iter()
        .flat_map(|group| &group.monitors)
        .map(|monitor| {
            let latest = heartbeats
                .heartbeats
                .get(&monitor.id.to_string())
                .and_then(|beats| beats.last());
            ServiceHealth {
                name: monitor.name.clone(),
                state: match latest.map(|beat| beat.status) {
                    Some(0) => ServiceState::Down,
                    Some(1) => ServiceState::Up,
                    Some(2) => ServiceState::Pending,
                    Some(3) => ServiceState::Maintenance,
                    _ => ServiceState::Unknown,
                },
                uptime_24h: heartbeats
                    .uptimes
                    .get(&format!("{}_24", monitor.id))
                    .copied(),
                last_heartbeat: latest.and_then(|beat| parse_kuma_time(&beat.time)),
                message: latest
                    .and_then(|beat| beat.msg.clone())
                    .filter(|msg| !msg.is_empty()),
            }
        })
        .collect()
}

/// Parses a Kuma heartbeat time, such as `2026-11-23 19:36:12.660`, in UTC.
pub(crate) fn parse_kuma_time(text: &str) -> Option<OffsetDateTime> {
    let format = format_description!(
        "[year]-[month]-[day] [hour]:[minute]:[second][optional [.[subsecond]]]"
    );
    PrimitiveDateTime::parse(text, format)
        .ok()
        .map(PrimitiveDateTime::assume_utc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn merges_status_page_with_heartbeats() {
        let page: KumaStatusPage = serde_json::from_str(
            r#"{"publicGroupList": [{"name": "Services", "monitorList": [
                {"id": 7, "name": "Media server"}, {"id": 9, "name": "Backup"}, {"id": 12, "name": "New"}
            ]}]}"#,
        )
        .unwrap();
        let heartbeats: KumaHeartbeats = serde_json::from_str(
            r#"{"heartbeatList": {
                "7": [{"status": 0, "time": "2026-11-23 19:35:12.100", "msg": "old"},
                      {"status": 1, "time": "2026-11-23 19:36:12.660", "msg": ""}],
                "9": [{"status": 0, "time": "2026-11-23 19:36:00", "msg": "timeout"}]
            }, "uptimeList": {"7_24": 1, "9_24": 0.875}}"#,
        )
        .unwrap();
        let services = merge_kuma_services(&page, &heartbeats);
        assert_eq!(services.len(), 3);
        assert_eq!(services[0].state, ServiceState::Up);
        assert_eq!(services[0].message, None);
        assert_eq!(
            services[0].last_heartbeat,
            Some(datetime!(2026-11-23 19:36:12.66 UTC))
        );
        assert_eq!(services[1].state, ServiceState::Down);
        assert_eq!(services[1].uptime_24h, Some(0.875));
        assert_eq!(services[1].message.as_deref(), Some("timeout"));
        assert_eq!(services[2].state, ServiceState::Unknown);
    }
}
