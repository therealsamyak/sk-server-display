//! Status board: polls the status source and keeps the latest snapshot.

use std::time::{Duration, Instant};

use gpui::{App, AppContext as _, Context, Entity, Subscription, Task};
use time::OffsetDateTime;

use crate::{
    status_freshness::{FreshnessPolicy, SourceFreshness, classify_source_freshness},
    status_snapshot::StatusSnapshot,
    status_source::StatusSource,
};

/// Interval of the redraw tick that keeps clocks and age labels current.
const STATUS_TICK_INTERVAL: Duration = Duration::from_secs(1);

/// Status board model: latest snapshot, poll errors, and freshness.
pub struct StatusBoard {
    source: StatusSource,
    policy: FreshnessPolicy,
    started_at: Instant,
    latest: Option<StatusSnapshot>,
    last_success_at: Option<Instant>,
    last_success_wall: Option<OffsetDateTime>,
    last_error: Option<String>,
    consecutive_failures: u32,
    last_poll_started_at: Option<Instant>,
    poll_task: Option<Task<()>>,
    _tick_task: Task<()>,
    _wake_subscription: Subscription,
}

impl StatusBoard {
    /// Creates the status board entity and starts the poll loop.
    pub fn new_status_board(
        source: StatusSource,
        policy: FreshnessPolicy,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| {
            let tick_task = cx.spawn(async move |this, cx| {
                loop {
                    let keep_running = this.update(cx, |board: &mut StatusBoard, cx| {
                        if board.poll_is_due() {
                            board.start_status_poll(cx);
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
                    this.update(cx, |board, cx| board.refresh_status_now(cx))
                        .ok();
                }
            });
            Self {
                source,
                policy,
                started_at: Instant::now(),
                latest: None,
                last_success_at: None,
                last_success_wall: None,
                last_error: None,
                consecutive_failures: 0,
                last_poll_started_at: None,
                poll_task: None,
                _tick_task: tick_task,
                _wake_subscription: wake_subscription,
            }
        })
    }

    /// Starts a poll now, unless one is already running.
    pub fn refresh_status_now(&mut self, cx: &mut Context<Self>) {
        self.start_status_poll(cx);
    }

    fn poll_is_due(&self) -> bool {
        self.last_poll_started_at
            .is_none_or(|started| started.elapsed() >= self.policy.poll_interval)
    }

    fn start_status_poll(&mut self, cx: &mut Context<Self>) {
        if self.poll_task.is_some() {
            return;
        }
        self.last_poll_started_at = Some(Instant::now());
        let source = self.source.clone();
        let fetch = cx.background_spawn(async move { source.fetch_status_snapshot() });
        self.poll_task = Some(cx.spawn(async move |this, cx| {
            let result = fetch.await;
            this.update(cx, |board, cx| {
                board.poll_task = None;
                board.apply_poll_result(result);
                cx.notify();
            })
            .ok();
        }));
    }

    fn apply_poll_result(&mut self, result: anyhow::Result<StatusSnapshot>) {
        match result {
            Ok(snapshot) => {
                self.latest = Some(snapshot);
                self.last_success_at = Some(Instant::now());
                self.last_success_wall = Some(OffsetDateTime::now_utc());
                self.last_error = None;
                self.consecutive_failures = 0;
            }
            Err(error) => {
                self.last_error = Some(format!("{error:#}"));
                self.consecutive_failures += 1;
            }
        }
    }

    /// Latest good snapshot, if any poll has succeeded.
    pub fn latest_snapshot(&self) -> Option<&StatusSnapshot> {
        self.latest.as_ref()
    }

    /// Freshness of the status source now.
    pub fn source_freshness(&self) -> SourceFreshness {
        classify_source_freshness(
            &self.policy,
            self.last_success_age(),
            self.started_at.elapsed(),
        )
    }

    /// Time since the last successful poll.
    pub fn last_success_age(&self) -> Option<Duration> {
        self.last_success_at.map(|at| at.elapsed())
    }

    /// Wall-clock time of the last successful poll.
    pub fn last_success_wall(&self) -> Option<OffsetDateTime> {
        self.last_success_wall
    }

    /// Error text of the last failed poll, cleared by the next success.
    pub fn last_poll_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Number of failed polls since the last success.
    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }

    /// Freshness thresholds in use.
    pub fn freshness_policy(&self) -> &FreshnessPolicy {
        &self.policy
    }

    /// Footer label of the status source.
    pub fn source_label(&self) -> String {
        self.source.status_source_label()
    }
}
