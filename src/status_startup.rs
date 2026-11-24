//! Startup timing: logs how long the status display takes to reach each
//! startup milestone. The benchmark in `bench/` reads these log lines.

use std::{
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

static PROCESS_START: OnceLock<Instant> = OnceLock::new();

/// One point in the startup of a status display app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupMilestone {
    /// The view function ran for the first time.
    FirstViewBuilt,
    /// The window presented its first frame.
    FirstFramePresented,
    /// Every feed has its first poll result, data or error.
    AllFeedsPolled,
}

impl StartupMilestone {
    fn milestone_index(self) -> usize {
        match self {
            Self::FirstViewBuilt => 0,
            Self::FirstFramePresented => 1,
            Self::AllFeedsPolled => 2,
        }
    }

    fn milestone_text(self) -> &'static str {
        match self {
            Self::FirstViewBuilt => "first view built",
            Self::FirstFramePresented => "first frame presented",
            Self::AllFeedsPolled => "all feeds polled",
        }
    }
}

static LOGGED: [AtomicBool; 3] = [
    AtomicBool::new(false),
    AtomicBool::new(false),
    AtomicBool::new(false),
];

/// Records the start of the process. Call it first in `main`.
pub fn record_process_start() {
    PROCESS_START.get_or_init(Instant::now);
}

/// Logs the time from process start to a milestone. Only the first call for
/// each milestone logs, so callers can call it on every frame.
pub fn log_startup_milestone(milestone: StartupMilestone) {
    if LOGGED[milestone.milestone_index()].swap(true, Ordering::Relaxed) {
        return;
    }
    let Some(start) = PROCESS_START.get() else {
        return;
    };
    log::info!(
        "startup: {} after {:.1} ms",
        milestone.milestone_text(),
        start.elapsed().as_secs_f64() * 1000.0
    );
}
