//! Status poller: one background thread for each status feed. Each thread
//! polls its collector, sends the result to the UI thread, and waits for the
//! next poll or a refresh request.

use std::{
    sync::mpsc::{self, Receiver, RecvTimeoutError, Sender},
    time::Duration,
};

use rune::AppHandle;
use server_status_core::status_collector::StatusCollector;

use crate::status_state::StatusState;

/// Refresh senders, one for each feed. A message starts a poll now.
#[derive(Default)]
pub struct StatusRefresh {
    senders: Vec<Sender<()>>,
}

impl StatusRefresh {
    /// Asks every feed thread to poll now.
    pub fn refresh_all_feeds(&self) {
        for sender in &self.senders {
            sender.send(()).ok();
        }
    }
}

/// One feed's poll job, before its thread starts.
pub struct StatusPollJob {
    index: usize,
    collector: StatusCollector,
    interval: Duration,
    refresh: Receiver<()>,
}

/// Creates the refresh senders and the poll jobs for the feeds.
pub fn new_status_poll_jobs(
    feeds: &[(StatusCollector, Duration)],
) -> (StatusRefresh, Vec<StatusPollJob>) {
    let mut refresh = StatusRefresh::default();
    let jobs = feeds
        .iter()
        .enumerate()
        .map(|(index, (collector, interval))| {
            let (sender, receiver) = mpsc::channel();
            refresh.senders.push(sender);
            StatusPollJob {
                index,
                collector: collector.clone(),
                interval: *interval,
                refresh: receiver,
            }
        })
        .collect();
    (refresh, jobs)
}

/// Starts one poll thread for each job. A thread stops when the app quits.
pub fn start_status_pollers(handle: &AppHandle<StatusState>, jobs: Vec<StatusPollJob>) {
    for job in jobs {
        let handle = handle.clone();
        let name = format!("status-poll-{}", job.index);
        let spawned = std::thread::Builder::new()
            .name(name)
            .spawn(move || run_status_poll_loop(handle, job));
        if let Err(error) = spawned {
            log::error!("cannot start a status poll thread: {error}");
        }
    }
}

fn run_status_poll_loop(handle: AppHandle<StatusState>, job: StatusPollJob) {
    loop {
        let result = job.collector.collect_status_reading();
        let index = job.index;
        handle.update(move |state, _| state.record_feed_poll(index, result));
        match job.refresh.recv_timeout(job.interval) {
            Ok(()) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        // Several refresh requests in a row start one poll.
        while job.refresh.try_recv().is_ok() {}
    }
}
