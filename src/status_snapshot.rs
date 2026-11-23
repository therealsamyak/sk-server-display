//! Status snapshot model: the JSON contract that the status display polls.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// One status snapshot: every host and service at one point in time.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct StatusSnapshot {
    /// Time when the status source produced this snapshot.
    #[serde(with = "time::serde::rfc3339")]
    pub generated_at: OffsetDateTime,
    /// Hosts in display order.
    pub hosts: Vec<HostStatus>,
    /// Services in display order.
    #[serde(default)]
    pub services: Vec<ServiceStatus>,
}

/// Host status: reachability and resource metrics for one server.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct HostStatus {
    /// Short host name, such as `nas-01`.
    pub name: String,
    /// Plain-words role, such as `Storage`.
    #[serde(default)]
    pub role: String,
    /// `false` when the status source cannot reach the host.
    pub reachable: bool,
    /// Time of the last metric sample for this host.
    #[serde(with = "time::serde::rfc3339")]
    pub sampled_at: OffsetDateTime,
    #[serde(default)]
    pub uptime_seconds: Option<u64>,
    #[serde(default)]
    pub cpu_percent: Option<f32>,
    #[serde(default)]
    pub memory_used_bytes: Option<u64>,
    #[serde(default)]
    pub memory_total_bytes: Option<u64>,
    #[serde(default)]
    pub disk_used_bytes: Option<u64>,
    #[serde(default)]
    pub disk_total_bytes: Option<u64>,
    #[serde(default)]
    pub temperature_celsius: Option<f32>,
    #[serde(default)]
    pub load_1m: Option<f32>,
    #[serde(default)]
    pub network_rx_bytes_per_second: Option<u64>,
    #[serde(default)]
    pub network_tx_bytes_per_second: Option<u64>,
}

/// Service status: the health of one service on one host.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ServiceStatus {
    /// Service name, such as `jellyfin`.
    pub name: String,
    /// Host that runs the service.
    #[serde(default)]
    pub host: String,
    /// Service health state.
    pub state: ServiceState,
    /// Short reason for a degraded or down state.
    #[serde(default)]
    pub detail: Option<String>,
}

/// Service health state reported by the status source.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceState {
    Ok,
    Degraded,
    Down,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_snapshot_json() {
        let json = r#"{
            "generated_at": "2026-11-23T22:02:11Z",
            "hosts": [
                {"name": "nas-01", "reachable": true, "sampled_at": "2026-11-23T22:02:10Z", "cpu_percent": 23.5}
            ],
            "services": [
                {"name": "samba", "host": "nas-01", "state": "degraded", "detail": "1 share read-only"}
            ]
        }"#;
        let snapshot: StatusSnapshot = serde_json::from_str(json).expect("snapshot parses");
        assert_eq!(snapshot.hosts[0].name, "nas-01");
        assert_eq!(snapshot.hosts[0].cpu_percent, Some(23.5));
        assert_eq!(snapshot.hosts[0].memory_total_bytes, None);
        assert_eq!(snapshot.services[0].state, ServiceState::Degraded);
    }
}
