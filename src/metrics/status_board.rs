//! Status board: polls every status feed on the GPUI background executor.
//!
//! The board emits [`StatusFeedUpdated`] when a reading or its freshness
//! changes, so only views that use that feed need to redraw.

use std::time::{Duration, Instant};

use crate::{
    metrics::status_config::StatusFeedSpec,
    metrics::status_feed::StatusFeed,
    metrics::status_freshness::FeedFreshness,
    status_startup::{StartupMilestone, log_startup_milestone},
};
use gpui::{App, AppContext as _, Context, Entity, EventEmitter, Subscription, Task};

/// Event: a feed's reading, history, error, or freshness changed.
#[derive(Clone, Copy, Debug)]
pub struct StatusFeedUpdated {
    /// Index of the feed in [`StatusBoard::status_feeds`].
    pub index: usize,
}

/// Poll state of one feed: when it next becomes due, and the running poll.
struct FeedPoll {
    due_at: Instant,
    task: Option<Task<()>>,
}

/// Status board model: all feeds and the poll loop.
pub struct StatusBoard {
    feeds: Vec<StatusFeed>,
    polls: Vec<FeedPoll>,
    started_at: Instant,
    poll_timer: Option<(Instant, Task<()>)>,
    _wake_subscription: Subscription,
}

impl StatusBoard {
    /// Creates the status board entity and starts the poll loop.
    pub fn new_status_board(specs: Vec<StatusFeedSpec>, cx: &mut App) -> Entity<Self> {
        let board = cx.new(|cx: &mut Context<Self>| {
            let started_at = Instant::now();
            let wake_subscription = cx.on_system_wake({
                let this = cx.weak_entity();
                move |cx| {
                    this.update(cx, |board, cx| board.refresh_all_feeds(cx))
                        .ok();
                }
            });
            Self {
                polls: specs
                    .iter()
                    .map(|_| FeedPoll {
                        due_at: started_at,
                        task: None,
                    })
                    .collect(),
                feeds: specs.into_iter().map(StatusFeed::new_status_feed).collect(),
                started_at,
                poll_timer: None,
                _wake_subscription: wake_subscription,
            }
        });
        board.update(cx, |board, cx| board.poll_due_feeds(cx));
        board
    }

    /// Starts a poll of every feed now, unless one is already running.
    pub fn refresh_all_feeds(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        for index in 0..self.feeds.len() {
            self.start_feed_poll(index, now, cx);
        }
        self.schedule_next_poll(cx);
    }

    fn poll_due_feeds(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        for index in 0..self.polls.len() {
            if self.polls[index].due_at <= now {
                self.start_feed_poll(index, now, cx);
            }
        }
        self.schedule_next_poll(cx);
    }

    fn schedule_next_poll(&mut self, cx: &mut Context<Self>) {
        let due = self
            .polls
            .iter()
            .filter(|poll| poll.task.is_none())
            .map(|poll| poll.due_at)
            .min();
        if due == self.poll_timer.as_ref().map(|(at, _)| *at) {
            return;
        }
        self.poll_timer = due.map(|at| {
            let timer = cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(at.saturating_duration_since(Instant::now()))
                    .await;
                if let Err(error) = this.update(cx, |board, cx| {
                    board.poll_timer = None;
                    board.poll_due_feeds(cx);
                }) {
                    log::debug!("poll timer ended with its status board: {error}");
                }
            });
            (at, timer)
        });
    }

    fn start_feed_poll(&mut self, index: usize, now: Instant, cx: &mut Context<Self>) {
        let poll = &mut self.polls[index];
        if poll.task.is_some() {
            return;
        }
        poll.due_at = now + self.feeds[index].policy.poll_interval;
        let collector = self.feeds[index].spec.collector.clone();
        let fetch = cx.background_spawn(async move { collector.collect_status_reading() });
        poll.task = Some(cx.spawn(async move |this, cx| {
            let result = fetch.await;
            this.update(cx, |board, cx| {
                board.polls[index].task = None;
                let feed = &mut board.feeds[index];
                let running_for = board.started_at.elapsed();
                let before = feed.feed_freshness(running_for);
                let changed = feed.record_feed_poll(result);
                if changed || before != feed.feed_freshness(running_for) {
                    cx.emit(StatusFeedUpdated { index });
                }
                if board.feeds.iter().all(StatusFeed::has_poll_result) {
                    log_startup_milestone(StartupMilestone::AllFeedsPolled);
                }
                board.schedule_next_poll(cx);
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

    /// Time until the freshness of one feed changes without a new poll.
    pub fn next_freshness_change(&self, feed: &StatusFeed) -> Option<Duration> {
        feed.next_freshness_change(self.started_at.elapsed())
    }
}

impl EventEmitter<StatusFeedUpdated> for StatusBoard {}
