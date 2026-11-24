//! Status redraw timer: draws a view again when its time-based text changes.
//!
//! GPUI draws a view again only after `cx.notify()`. Clock text, data ages,
//! and freshness change with time and not with new data. Each view records
//! the earliest of these changes while it renders, and this timer notifies
//! the view at that time. Views with no time-based text never wake up.

use std::{cell::Cell, time::Duration};

use gpui::{Context, Task};
use time::OffsetDateTime;

use crate::status_format::{format_status_age, status_age_text_valid_for};

/// Extra wait after a text change, so the redraw sees the new text and not
/// the last moment of the old text.
const STATUS_REDRAW_MARGIN: Duration = Duration::from_millis(5);

/// Redraw timer of one view.
#[derive(Default)]
pub struct StatusRedrawTimer {
    next_in: Cell<Option<Duration>>,
    task: Option<Task<()>>,
}

impl StatusRedrawTimer {
    /// Asks for a redraw after `delay`. The earliest request wins.
    pub fn redraw_after(&self, delay: Duration) {
        let next = self.next_in.get().map_or(delay, |next| next.min(delay));
        self.next_in.set(Some(next));
    }

    /// Formats a data age, such as `4s`, and asks for a redraw when the text
    /// changes.
    pub fn status_age_text(&self, age: Duration) -> String {
        self.redraw_after(status_age_text_valid_for(age));
        format_status_age(age)
    }

    /// Formats the age of a wall-clock time, such as `4s ago`. `—` when there
    /// is no time.
    pub fn status_ago_text(&self, now: OffsetDateTime, at: Option<OffsetDateTime>) -> String {
        at.map(|at| {
            let age = Duration::try_from(now - at).unwrap_or_default();
            format!("{} ago", self.status_age_text(age))
        })
        .unwrap_or_else(|| "—".into())
    }

    /// Starts the timer for the earliest redraw request of this render, and
    /// cancels the timer of the last render. Call it at the end of `render`.
    pub fn schedule_status_redraw<V: 'static>(&mut self, cx: &mut Context<V>) {
        self.task = self.next_in.take().map(|delay| {
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(delay + STATUS_REDRAW_MARGIN)
                    .await;
                this.update(cx, |_, cx| cx.notify()).ok();
            })
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redraw_timer_keeps_the_earliest_request() {
        let timer = StatusRedrawTimer::default();
        timer.redraw_after(Duration::from_secs(30));
        assert_eq!(timer.status_age_text(Duration::from_millis(4_250)), "4s");
        timer.redraw_after(Duration::from_secs(10));
        assert_eq!(timer.next_in.get(), Some(Duration::from_millis(750)));
    }
}
