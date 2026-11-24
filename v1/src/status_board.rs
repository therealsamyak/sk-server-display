//! Status board: polls every status feed on the GPUI background executor.

use std::time::{Duration, Instant};

use gpui::{App, AppContext as _, Context, Entity, Subscription, Task};
use server_status_core::{
    status_config::StatusFeedSpec,
    status_feed::StatusFeed,
    status_freshness::FeedFreshness,
    status_startup::{StartupMilestone, log_startup_milestone},
};

/// Interval of the redraw tick that keeps clocks and age labels current.
const STATUS_TICK_INTERVAL: Duration = Duration::from_secs(1);

/// Poll state of one feed: when its last poll started, and the running poll.
#[derive(Default)]
struct FeedPoll {
    started_at: Option<Instant>,
    task: Option<Task<()>>,
}

/// Status board model: all feeds and the poll loop.
pub struct StatusBoard {
    feeds: Vec<StatusFeed>,
    polls: Vec<FeedPoll>,
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
                polls: specs.iter().map(|_| FeedPoll::default()).collect(),
                feeds: specs.into_iter().map(StatusFeed::new_status_feed).collect(),
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
        let interval = self.feeds[index].policy.poll_interval;
        self.polls[index]
            .started_at
            .is_none_or(|started| started.elapsed() >= interval)
    }

    fn start_feed_poll(&mut self, index: usize, cx: &mut Context<Self>) {
        let poll = &mut self.polls[index];
        if poll.task.is_some() {
            return;
        }
        poll.started_at = Some(Instant::now());
        let collector = self.feeds[index].spec.collector.clone();
        let fetch = cx.background_spawn(async move { collector.collect_status_reading() });
        poll.task = Some(cx.spawn(async move |this, cx| {
            let result = fetch.await;
            this.update(cx, |board, cx| {
                board.polls[index].task = None;
                board.feeds[index].record_feed_poll(result);
                if board.feeds.iter().all(StatusFeed::has_poll_result) {
                    log_startup_milestone(StartupMilestone::AllFeedsPolled);
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
        feed.feed_freshness(self.started_at.elapsed())
    }
}
