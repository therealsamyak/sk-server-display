//! Status freshness rules: when data counts as live, stale, or offline.

use std::time::Duration;

use time::OffsetDateTime;

use crate::status_snapshot::HostStatus;

/// Freshness thresholds derived from the poll interval.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FreshnessPolicy {
    /// Time between two status polls.
    pub poll_interval: Duration,
    /// Data older than this is stale.
    pub stale_after: Duration,
    /// Data older than this is offline.
    pub offline_after: Duration,
    /// Time limit for one status request.
    pub request_timeout: Duration,
}

impl FreshnessPolicy {
    /// Builds the freshness policy for one poll interval.
    ///
    /// Stale means three missed polls. Offline means 15 missed polls, and
    /// never less than 30 seconds.
    pub fn from_poll_interval(poll_interval: Duration) -> Self {
        let poll_interval = poll_interval.max(Duration::from_millis(500));
        Self {
            poll_interval,
            stale_after: poll_interval * 3,
            offline_after: (poll_interval * 15).max(Duration::from_secs(30)),
            request_timeout: poll_interval
                .mul_f32(0.75)
                .clamp(Duration::from_millis(500), Duration::from_secs(10)),
        }
    }
}

/// Freshness of the status source as a whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceFreshness {
    /// No successful poll yet, and the offline limit is not reached.
    Waiting,
    /// The last successful poll is recent.
    Live,
    /// Polls fail, and the last good data is older than `stale_after`.
    Stale,
    /// No good data for longer than `offline_after`.
    Offline,
}

/// Classifies source freshness from the age of the last successful poll.
pub fn classify_source_freshness(
    policy: &FreshnessPolicy,
    last_success_age: Option<Duration>,
    running_for: Duration,
) -> SourceFreshness {
    match last_success_age {
        None if running_for >= policy.offline_after => SourceFreshness::Offline,
        None => SourceFreshness::Waiting,
        Some(age) if age >= policy.offline_after => SourceFreshness::Offline,
        Some(age) if age >= policy.stale_after => SourceFreshness::Stale,
        Some(_) => SourceFreshness::Live,
    }
}

/// Freshness of one host inside a snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostFreshness {
    /// The host sample is recent.
    Live,
    /// The host sample is older than `stale_after`.
    Stale,
    /// The host sample is older than `offline_after`.
    Offline,
    /// The status source reports the host as unreachable.
    Down,
}

/// Classifies host freshness from the host sample time and the snapshot time.
pub fn classify_host_freshness(
    policy: &FreshnessPolicy,
    host: &HostStatus,
    generated_at: OffsetDateTime,
) -> HostFreshness {
    if !host.reachable {
        return HostFreshness::Down;
    }
    let age = (generated_at - host.sampled_at).max(time::Duration::ZERO);
    let age = Duration::try_from(age).unwrap_or(Duration::ZERO);
    if age >= policy.offline_after {
        HostFreshness::Offline
    } else if age >= policy.stale_after {
        HostFreshness::Stale
    } else {
        HostFreshness::Live
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn policy() -> FreshnessPolicy {
        FreshnessPolicy::from_poll_interval(Duration::from_secs(2))
    }

    fn host(reachable: bool, sampled_at: OffsetDateTime) -> HostStatus {
        HostStatus {
            name: "nas-01".into(),
            role: "Storage".into(),
            reachable,
            sampled_at,
            uptime_seconds: None,
            cpu_percent: None,
            memory_used_bytes: None,
            memory_total_bytes: None,
            disk_used_bytes: None,
            disk_total_bytes: None,
            temperature_celsius: None,
            load_1m: None,
            network_rx_bytes_per_second: None,
            network_tx_bytes_per_second: None,
        }
    }

    #[test]
    fn policy_thresholds_follow_poll_interval() {
        let policy = policy();
        assert_eq!(policy.stale_after, Duration::from_secs(6));
        assert_eq!(policy.offline_after, Duration::from_secs(30));
        assert_eq!(policy.request_timeout, Duration::from_millis(1500));
        let slow = FreshnessPolicy::from_poll_interval(Duration::from_secs(10));
        assert_eq!(slow.offline_after, Duration::from_secs(150));
    }

    #[test]
    fn source_freshness_moves_from_live_to_offline() {
        let policy = policy();
        let start = Duration::from_secs(1);
        let s = |age: Option<u64>| {
            classify_source_freshness(&policy, age.map(Duration::from_secs), start)
        };
        assert_eq!(s(None), SourceFreshness::Waiting);
        assert_eq!(s(Some(1)), SourceFreshness::Live);
        assert_eq!(s(Some(6)), SourceFreshness::Stale);
        assert_eq!(s(Some(29)), SourceFreshness::Stale);
        assert_eq!(s(Some(30)), SourceFreshness::Offline);
        assert_eq!(
            classify_source_freshness(&policy, None, Duration::from_secs(31)),
            SourceFreshness::Offline
        );
    }

    #[test]
    fn host_freshness_uses_sample_age_and_reachability() {
        let policy = policy();
        let now = datetime!(2026-11-23 22:02:11 UTC);
        let h = |seconds: i64| host(true, now - time::Duration::seconds(seconds));
        assert_eq!(
            classify_host_freshness(&policy, &h(1), now),
            HostFreshness::Live
        );
        assert_eq!(
            classify_host_freshness(&policy, &h(14), now),
            HostFreshness::Stale
        );
        assert_eq!(
            classify_host_freshness(&policy, &h(45), now),
            HostFreshness::Offline
        );
        assert_eq!(
            classify_host_freshness(&policy, &h(-3), now),
            HostFreshness::Live
        );
        assert_eq!(
            classify_host_freshness(&policy, &host(false, now), now),
            HostFreshness::Down
        );
    }
}
