//! Status feed: one collector's latest reading, host history, and poll errors.

use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use crate::{
    metrics::status_config::StatusFeedSpec,
    metrics::status_freshness::{
        FeedFreshness, FreshnessPolicy, classify_feed_freshness, next_freshness_change,
    },
    metrics::status_model::{FeedData, HostMetrics},
};

/// Number of host samples that the CPU and network graphs show. At the
/// 2-second host poll interval this is 3 minutes.
pub const HOST_HISTORY_SAMPLES: usize = 90;

/// Recent host samples for the CPU and network graphs, oldest first.
#[derive(Debug, Default)]
pub struct HostHistory {
    pub cpu_percent: VecDeque<f32>,
    pub network_receive_rate: VecDeque<f64>,
    pub network_send_rate: VecDeque<f64>,
}

impl HostHistory {
    fn record_host_sample(&mut self, host: &HostMetrics) {
        fn push<T>(samples: &mut VecDeque<T>, value: T) {
            if samples.len() == HOST_HISTORY_SAMPLES {
                samples.pop_front();
            }
            samples.push_back(value);
        }
        if let Some(cpu) = host.cpu_percent {
            push(&mut self.cpu_percent, cpu);
        }
        push(
            &mut self.network_receive_rate,
            host.network_receive_rate.unwrap_or_default(),
        );
        push(
            &mut self.network_send_rate,
            host.network_send_rate.unwrap_or_default(),
        );
    }
}

/// Status feed: one collector, its latest reading, and its last poll error.
/// The UI crates own the poll scheduling.
#[derive(Debug)]
pub struct StatusFeed {
    pub spec: StatusFeedSpec,
    pub policy: FreshnessPolicy,
    data: Option<FeedData>,
    history: HostHistory,
    last_success_at: Option<Instant>,
    last_error: Option<String>,
}

impl StatusFeed {
    /// Creates a status feed with no reading yet.
    pub fn new_status_feed(spec: StatusFeedSpec) -> Self {
        Self {
            policy: FreshnessPolicy::from_poll_interval(spec.poll_interval),
            spec,
            data: None,
            history: HostHistory::default(),
            last_success_at: None,
            last_error: None,
        }
    }

    /// `true` after the first poll result, data or error.
    pub fn has_poll_result(&self) -> bool {
        self.data.is_some() || self.last_error.is_some()
    }

    /// Stores the result of one poll: a new reading, or the poll error.
    pub fn record_feed_poll(&mut self, result: anyhow::Result<FeedData>) {
        match result {
            Ok(data) => {
                if let FeedData::Host(host) = &data {
                    self.history.record_host_sample(host);
                }
                self.data = Some(data);
                self.last_success_at = Some(Instant::now());
                self.last_error = None;
            }
            Err(error) => {
                log::warn!("{} poll failed: {error:#}", self.spec.title);
                self.last_error = Some(format!("{error:#}"));
            }
        }
    }

    /// Latest good reading, if any poll has succeeded.
    pub fn latest_reading(&self) -> Option<&FeedData> {
        self.data.as_ref()
    }

    /// Recent CPU and network samples. Empty for feeds without host metrics.
    pub fn host_history(&self) -> &HostHistory {
        &self.history
    }

    /// Time since the last successful poll.
    pub fn last_success_age(&self) -> Option<Duration> {
        self.last_success_at.map(|at| at.elapsed())
    }

    /// Error text of the last failed poll, cleared by the next success.
    pub fn last_poll_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Freshness of this feed now. `running_for` is the time since the
    /// display started, so a feed that never answers becomes offline.
    pub fn feed_freshness(&self, running_for: Duration) -> FeedFreshness {
        classify_feed_freshness(&self.policy, self.last_success_age(), running_for)
    }

    /// Time until [`Self::feed_freshness`] changes without a new poll result.
    pub fn next_freshness_change(&self, running_for: Duration) -> Option<Duration> {
        next_freshness_change(&self.policy, self.last_success_age(), running_for)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{
        collectors::collector_local::LocalCollector, status_collector::StatusCollector,
    };

    fn host_feed() -> StatusFeed {
        StatusFeed::new_status_feed(StatusFeedSpec {
            title: "server".into(),
            source: "local",
            collector: StatusCollector::Local(LocalCollector::default()),
            poll_interval: Duration::from_secs(2),
            btop: None,
        })
    }

    #[test]
    fn keeps_last_reading_after_a_failed_poll() {
        let mut feed = host_feed();
        let host = HostMetrics {
            cpu_percent: Some(12.0),
            ..HostMetrics::default()
        };
        feed.record_feed_poll(Ok(FeedData::Host(host.clone())));
        feed.record_feed_poll(Err(anyhow::anyhow!("connection refused")));
        assert_eq!(feed.latest_reading(), Some(&FeedData::Host(host)));
        assert_eq!(feed.last_poll_error(), Some("connection refused"));
        assert_eq!(
            feed.feed_freshness(Duration::from_secs(1)),
            FeedFreshness::Live
        );
    }

    #[test]
    fn host_history_keeps_the_newest_samples() {
        let mut feed = host_feed();
        for index in 0..HOST_HISTORY_SAMPLES + 5 {
            let host = HostMetrics {
                cpu_percent: Some(index as f32),
                ..HostMetrics::default()
            };
            feed.record_feed_poll(Ok(FeedData::Host(host)));
        }
        let cpu = &feed.host_history().cpu_percent;
        assert_eq!(cpu.len(), HOST_HISTORY_SAMPLES);
        assert_eq!(cpu.front(), Some(&5.0));
    }
}
