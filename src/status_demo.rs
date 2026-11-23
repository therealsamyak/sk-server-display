//! Demo status source: realistic sample metrics without a real server.

use std::time::{Duration, Instant};

use anyhow::bail;
use time::OffsetDateTime;

use crate::status_snapshot::{HostStatus, ServiceState, ServiceStatus, StatusSnapshot};

const GIB: u64 = 1024 * 1024 * 1024;
const TIB: u64 = 1024 * GIB;

/// Demo scenario that selects which states the sample data shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum DemoScenario {
    /// All hosts are live and all services are healthy.
    Healthy,
    /// One host is stale, one host is down, and one service is degraded.
    Mixed,
}

/// Demo status source that generates sample snapshots in process.
#[derive(Clone, Debug)]
pub struct DemoStatusSource {
    started_at: Instant,
    scenario: DemoScenario,
    outage_after: Option<Duration>,
}

impl DemoStatusSource {
    /// Creates a demo source. With `outage_after`, polls fail after that time.
    pub fn new(scenario: DemoScenario, outage_after: Option<Duration>) -> Self {
        Self {
            started_at: Instant::now(),
            scenario,
            outage_after,
        }
    }

    /// Returns the next demo snapshot, or an error during a simulated outage.
    pub fn fetch_demo_snapshot(&self) -> anyhow::Result<StatusSnapshot> {
        let elapsed = self.started_at.elapsed();
        if let Some(outage_after) = self.outage_after
            && elapsed >= outage_after
        {
            bail!("demo outage: connection refused");
        }
        Ok(build_demo_snapshot(
            self.scenario,
            OffsetDateTime::now_utc(),
            elapsed.as_secs_f64(),
        ))
    }
}

/// Builds one demo snapshot for a scenario at time `now`.
pub fn build_demo_snapshot(scenario: DemoScenario, now: OffsetDateTime, t: f64) -> StatusSnapshot {
    let wave = |period: f64, phase: f64| ((t / period + phase) * std::f64::consts::TAU).sin();
    let mixed = scenario == DemoScenario::Mixed;

    let mut hosts = vec![
        host("nas-01", "Storage", now, 3_564_000)
            .cpu(23.0 + 6.0 * wave(37.0, 0.1))
            .memory(11.2, 32)
            .disk(7.1, 12.0)
            .temperature(48.0 + 1.5 * wave(90.0, 0.3))
            .load(0.82 + 0.2 * wave(53.0, 0.0))
            .network(12_400_000.0 + 3_000_000.0 * wave(11.0, 0.2), 1_100_000.0),
        host("media-01", "Media", now, 1_046_000)
            .cpu(61.0 + 12.0 * wave(23.0, 0.4))
            .memory(9.8, 16)
            .disk(3.4, 4.0)
            .temperature(67.0 + 3.0 * wave(41.0, 0.6))
            .load(3.41 + 0.5 * wave(29.0, 0.2))
            .network(2_300_000.0, 38_600_000.0 + 6_000_000.0 * wave(13.0, 0.5)),
        host("build-01", "CI runner", now, 266_000)
            .cpu(88.0 + 8.0 * wave(17.0, 0.8))
            .memory(27.5, 32)
            .disk(0.41, 0.93)
            .temperature(79.0 + 4.0 * wave(19.0, 0.1))
            .load(7.95 + 0.8 * wave(21.0, 0.3))
            .network(4_800_000.0, 600_000.0),
        host("gateway-01", "Router", now, 8_294_000)
            .cpu(7.0 + 2.0 * wave(31.0, 0.9))
            .memory(1.3, 4)
            .disk(0.006, 0.028)
            .temperature(52.0)
            .load(0.11)
            .network(54_200_000.0 + 9_000_000.0 * wave(7.0, 0.0), 6_300_000.0),
        host("dns-01", "DNS", now, 1_901_000)
            .cpu(3.0 + 1.0 * wave(43.0, 0.2))
            .memory(0.6, 2)
            .disk(0.011, 0.029)
            .temperature(44.0)
            .load(0.04)
            .network(180_000.0, 210_000.0),
        host("backup-01", "Backup", now, 604_000)
            .cpu(12.0)
            .memory(2.1, 8)
            .disk(14.2, 16.0)
            .temperature(41.0)
            .load(0.35)
            .network(900_000.0, 80_000.0),
    ];

    if mixed {
        let dns = &mut hosts[4];
        dns.0.sampled_at = now - time::Duration::seconds(14);
        let backup = &mut hosts[5];
        backup.0.reachable = false;
        backup.0.sampled_at = now - time::Duration::minutes(192);
    }

    let mut services = vec![
        service("samba", "nas-01", ServiceState::Ok, None),
        service("jellyfin", "media-01", ServiceState::Ok, None),
        service("forgejo-runner", "build-01", ServiceState::Ok, None),
        service("wireguard", "gateway-01", ServiceState::Ok, None),
        service("unbound", "dns-01", ServiceState::Ok, None),
        service("restic-nightly", "backup-01", ServiceState::Ok, None),
    ];
    if mixed {
        services[2] = service(
            "forgejo-runner",
            "build-01",
            ServiceState::Degraded,
            Some("queue 7 jobs"),
        );
        services[5] = service(
            "restic-nightly",
            "backup-01",
            ServiceState::Down,
            Some("host unreachable"),
        );
    }

    StatusSnapshot {
        generated_at: now,
        hosts: hosts.into_iter().map(|host| host.0).collect(),
        services,
    }
}

struct DemoHost(HostStatus);

fn host(name: &str, role: &str, now: OffsetDateTime, uptime_seconds: u64) -> DemoHost {
    DemoHost(HostStatus {
        name: name.into(),
        role: role.into(),
        reachable: true,
        sampled_at: now - time::Duration::milliseconds(400),
        uptime_seconds: Some(uptime_seconds),
        cpu_percent: None,
        memory_used_bytes: None,
        memory_total_bytes: None,
        disk_used_bytes: None,
        disk_total_bytes: None,
        temperature_celsius: None,
        load_1m: None,
        network_rx_bytes_per_second: None,
        network_tx_bytes_per_second: None,
    })
}

impl DemoHost {
    fn cpu(mut self, percent: f64) -> Self {
        self.0.cpu_percent = Some(percent.clamp(0.0, 100.0) as f32);
        self
    }
    fn memory(mut self, used_gib: f64, total_gib: u64) -> Self {
        self.0.memory_used_bytes = Some((used_gib * GIB as f64) as u64);
        self.0.memory_total_bytes = Some(total_gib * GIB);
        self
    }
    fn disk(mut self, used_tib: f64, total_tib: f64) -> Self {
        self.0.disk_used_bytes = Some((used_tib * TIB as f64) as u64);
        self.0.disk_total_bytes = Some((total_tib * TIB as f64) as u64);
        self
    }
    fn temperature(mut self, celsius: f64) -> Self {
        self.0.temperature_celsius = Some(celsius as f32);
        self
    }
    fn load(mut self, load: f64) -> Self {
        self.0.load_1m = Some(load.max(0.0) as f32);
        self
    }
    fn network(mut self, rx: f64, tx: f64) -> Self {
        self.0.network_rx_bytes_per_second = Some(rx.max(0.0) as u64);
        self.0.network_tx_bytes_per_second = Some(tx.max(0.0) as u64);
        self
    }
}

fn service(name: &str, host: &str, state: ServiceState, detail: Option<&str>) -> ServiceStatus {
    ServiceStatus {
        name: name.into(),
        host: host.into(),
        state,
        detail: detail.map(Into::into),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status_freshness::{FreshnessPolicy, HostFreshness, classify_host_freshness};
    use time::macros::datetime;

    #[test]
    fn mixed_demo_shows_stale_and_down_hosts() {
        let now = datetime!(2026-11-23 22:02:11 UTC);
        let snapshot = build_demo_snapshot(DemoScenario::Mixed, now, 0.0);
        let policy = FreshnessPolicy::from_poll_interval(Duration::from_secs(2));
        let states: Vec<_> = snapshot
            .hosts
            .iter()
            .map(|host| classify_host_freshness(&policy, host, now))
            .collect();
        assert_eq!(
            states.iter().filter(|s| **s == HostFreshness::Live).count(),
            4
        );
        assert_eq!(states[4], HostFreshness::Stale);
        assert_eq!(states[5], HostFreshness::Down);
    }

    #[test]
    fn demo_source_fails_after_outage_starts() {
        let source = DemoStatusSource::new(DemoScenario::Healthy, Some(Duration::ZERO));
        assert!(source.fetch_demo_snapshot().is_err());
    }
}
