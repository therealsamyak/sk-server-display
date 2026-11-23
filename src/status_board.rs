//! Status board: polls every status feed and keeps its latest reading.

use std::time::{Duration, Instant};

use crate::{
    status_config::StatusFeedSpec,
    status_freshness::{FeedFreshness, FreshnessPolicy, classify_feed_freshness},
    status_model::FeedData,
};
use gpui::{App, AppContext as _, Context, Entity, Subscription, Task};

/// Interval of the redraw tick that keeps clocks and age labels current.
const STATUS_TICK_INTERVAL: Duration = Duration::from_secs(1);

/// Status feed: one collector, its latest reading, and its poll state.
pub struct StatusFeed {
    pub spec: StatusFeedSpec,
    pub policy: FreshnessPolicy,
    data: Option<FeedData>,
    last_success_at: Option<Instant>,
    last_error: Option<String>,
    consecutive_failures: u32,
    last_poll_started_at: Option<Instant>,
    poll_task: Option<Task<()>>,
}

impl StatusFeed {
    /// Latest good reading, if any poll has succeeded.
    pub fn latest_reading(&self) -> Option<&FeedData> {
        self.data.as_ref()
    }

    /// Time since the last successful poll.
    pub fn last_success_age(&self) -> Option<Duration> {
        self.last_success_at.map(|at| at.elapsed())
    }

    /// Error text of the last failed poll, cleared by the next success.
    pub fn last_poll_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Number of failed polls since the last success.
    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }
}

/// Status board model: all feeds and the poll loop.
pub struct StatusBoard {
    feeds: Vec<StatusFeed>,
    started_at: Instant,
    _tick_task: Task<()>,
    _wake_subscription: Subscription,
}

impl StatusBoard {
    /// Creates the status board entity and starts the poll loop.
    pub fn new_status_board(specs: Vec<StatusFeedSpec>, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let tick_task = cx.spawn(async move |this, cx| {
                loop {
                    let keep_running = this.update(cx, |board: &mut StatusBoard, cx| {
                        for index in 0..board.feeds.len() {
                            if board.poll_is_due(index) {
                                board.start_feed_poll(index, cx);
                            }
                        }
                        cx.notify();
                    });
                    if keep_running.is_err() {
                        return;
                    }
                    cx.background_executor().timer(STATUS_TICK_INTERVAL).await;
                }
            });
            let wake_subscription = cx.on_system_wake({
                let this = cx.weak_entity();
                move |cx| {
                    this.update(cx, |board, cx| board.refresh_all_feeds(cx))
                        .ok();
                }
            });
            Self {
                feeds: specs
                    .into_iter()
                    .map(|spec| StatusFeed {
                        policy: FreshnessPolicy::from_poll_interval(spec.poll_interval),
                        spec,
                        data: None,
                        last_success_at: None,
                        last_error: None,
                        consecutive_failures: 0,
                        last_poll_started_at: None,
                        poll_task: None,
                    })
                    .collect(),
                started_at: Instant::now(),
                _tick_task: tick_task,
                _wake_subscription: wake_subscription,
            }
        })
    }

    /// Starts a poll of every feed now, unless one is already running.
    pub fn refresh_all_feeds(&mut self, cx: &mut Context<Self>) {
        for index in 0..self.feeds.len() {
            self.start_feed_poll(index, cx);
        }
    }

    fn poll_is_due(&self, index: usize) -> bool {
        let feed = &self.feeds[index];
        feed.last_poll_started_at
            .is_none_or(|started| started.elapsed() >= feed.policy.poll_interval)
    }

    fn start_feed_poll(&mut self, index: usize, cx: &mut Context<Self>) {
        let feed = &mut self.feeds[index];
        if feed.poll_task.is_some() {
            return;
        }
        feed.last_poll_started_at = Some(Instant::now());
        let collector = feed.spec.collector.clone();
        let fetch = cx.background_spawn(async move { collector.collect_status_reading() });
        feed.poll_task = Some(cx.spawn(async move |this, cx| {
            let result = fetch.await;
            this.update(cx, |board, cx| {
                let feed = &mut board.feeds[index];
                feed.poll_task = None;
                match result {
                    Ok(data) => {
                        feed.data = Some(data);
                        feed.last_success_at = Some(Instant::now());
                        feed.last_error = None;
                        feed.consecutive_failures = 0;
                    }
                    Err(error) => {
                        log::warn!("{} poll failed: {error:#}", feed.spec.title);
                        feed.last_error = Some(format!("{error:#}"));
                        feed.consecutive_failures += 1;
                    }
                }
                cx.notify();
            })
            .ok();
        }));
    }

    /// All feeds in display order.
    pub fn status_feeds(&self) -> &[StatusFeed] {
        &self.feeds
    }

    /// Freshness of one feed now.
    pub fn feed_freshness(&self, feed: &StatusFeed) -> FeedFreshness {
        classify_feed_freshness(
            &feed.policy,
            feed.last_success_age(),
            self.started_at.elapsed(),
        )
    }
}
