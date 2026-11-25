//! Feed freshness rules: when a feed's data counts as live, stale, or offline.

use std::time::Duration;

/// Freshness limits derived from a feed's poll interval.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FreshnessPolicy {
    /// Time between two polls.
    pub poll_interval: Duration,
    /// Data older than this is stale.
    pub stale_after: Duration,
    /// Data older than this is offline.
    pub offline_after: Duration,
}

impl FreshnessPolicy {
    /// Builds the freshness policy for one poll interval.
    ///
    /// Stale means three missed polls. Offline means 15 missed polls, and
    /// never less than 30 seconds.
    pub fn from_poll_interval(poll_interval: Duration) -> Self {
        Self {
            poll_interval,
            stale_after: poll_interval * 3,
            offline_after: (poll_interval * 15).max(Duration::from_secs(30)),
        }
    }
}

/// Freshness of one status feed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedFreshness {
    /// No successful poll yet, and the offline limit is not reached.
    Waiting,
    /// The last successful poll is recent.
    Live,
    /// Polls fail, and the last good data is older than `stale_after`.
    Stale,
    /// No good data for longer than `offline_after`.
    Offline,
}

/// Classifies feed freshness from the age of the last successful poll.
pub fn classify_feed_freshness(
    policy: &FreshnessPolicy,
    last_success_age: Option<Duration>,
    running_for: Duration,
) -> FeedFreshness {
    match last_success_age {
        None if running_for >= policy.offline_after => FeedFreshness::Offline,
        None => FeedFreshness::Waiting,
        Some(age) if age >= policy.offline_after => FeedFreshness::Offline,
        Some(age) if age >= policy.stale_after => FeedFreshness::Stale,
        Some(_) => FeedFreshness::Live,
    }
}

/// Time until the freshness of a feed changes next, with the same inputs as
/// [`classify_feed_freshness`]. `None` when the feed is offline and stays
/// offline until a poll succeeds.
pub fn next_freshness_change(
    policy: &FreshnessPolicy,
    last_success_age: Option<Duration>,
    running_for: Duration,
) -> Option<Duration> {
    let (age, limits) = match last_success_age {
        Some(age) => (age, [policy.stale_after, policy.offline_after]),
        None => (running_for, [policy.offline_after, policy.offline_after]),
    };
    limits
        .into_iter()
        .find(|limit| age < *limit)
        .map(|limit| limit - age)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_limits_follow_poll_interval() {
        let fast = FreshnessPolicy::from_poll_interval(Duration::from_secs(2));
        assert_eq!(fast.stale_after, Duration::from_secs(6));
        assert_eq!(fast.offline_after, Duration::from_secs(30));
        let slow = FreshnessPolicy::from_poll_interval(Duration::from_secs(300));
        assert_eq!(slow.stale_after, Duration::from_secs(900));
        assert_eq!(slow.offline_after, Duration::from_secs(4500));
    }

    #[test]
    fn feed_freshness_moves_from_live_to_offline() {
        let policy = FreshnessPolicy::from_poll_interval(Duration::from_secs(2));
        let start = Duration::from_secs(1);
        let f = |age: Option<u64>| {
            classify_feed_freshness(&policy, age.map(Duration::from_secs), start)
        };
        assert_eq!(f(None), FeedFreshness::Waiting);
        assert_eq!(f(Some(1)), FeedFreshness::Live);
        assert_eq!(f(Some(6)), FeedFreshness::Stale);
        assert_eq!(f(Some(30)), FeedFreshness::Offline);
        assert_eq!(
            classify_feed_freshness(&policy, None, Duration::from_secs(31)),
            FeedFreshness::Offline
        );
    }

    #[test]
    fn next_freshness_change_finds_the_next_limit() {
        let policy = FreshnessPolicy::from_poll_interval(Duration::from_secs(2));
        let start = Duration::from_secs(1);
        let f =
            |age: Option<u64>| next_freshness_change(&policy, age.map(Duration::from_secs), start);
        assert_eq!(f(Some(1)), Some(Duration::from_secs(5)));
        assert_eq!(f(Some(10)), Some(Duration::from_secs(20)));
        assert_eq!(f(Some(30)), None);
        assert_eq!(f(None), Some(Duration::from_secs(29)));
        assert_eq!(
            next_freshness_change(&policy, None, Duration::from_secs(31)),
            None
        );
    }
}
