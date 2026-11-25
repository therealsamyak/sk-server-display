//! Heartbeat strips for the Services summary and monitor rows.

use gpui::{Div, Hsla, ParentElement as _, Styled as _, div, relative};

use crate::metrics::{ServiceHealth, ServiceState};

use super::status_theme::{Scale, StatusTheme};

const HISTORY_SLOTS: usize = 24;

pub(super) fn service_state_color(theme: StatusTheme, state: ServiceState) -> Hsla {
    match state {
        ServiceState::Up => theme.ok(),
        ServiceState::Down => theme.bad(),
        ServiceState::Pending => theme.warn(),
        ServiceState::Maintenance => theme.info(),
        ServiceState::Unknown => theme.track(),
    }
}

/// A compact strip of recent check results for one monitor.
pub(super) fn service_history_strip(
    s: Scale,
    theme: StatusTheme,
    service: &ServiceHealth,
    dim: bool,
) -> Div {
    let recent = &service.heartbeats[service.heartbeats.len().saturating_sub(HISTORY_SLOTS)..];
    let mut states = [ServiceState::Unknown; HISTORY_SLOTS];
    for (slot, beat) in states[HISTORY_SLOTS - recent.len()..]
        .iter_mut()
        .zip(recent)
    {
        *slot = beat.state;
    }
    heartbeat_strip(s, theme, states, dim)
}

/// A compact strip that combines all monitor results by minute.
pub(super) fn service_summary_strip(
    s: Scale,
    theme: StatusTheme,
    services: &[ServiceHealth],
    dim: bool,
) -> Div {
    heartbeat_strip(s, theme, summary_history(services), dim)
}

fn heartbeat_strip(
    s: Scale,
    theme: StatusTheme,
    states: [ServiceState; HISTORY_SLOTS],
    dim: bool,
) -> Div {
    div()
        .flex_none()
        .w(relative(0.42))
        .max_w(s(168.))
        .h(s(10.))
        .flex()
        .gap(s(1.5))
        .children(states.map(|state| {
            let color = if dim && state != ServiceState::Unknown {
                theme.faint()
            } else {
                service_state_color(theme, state)
            };
            div().flex_1().min_w_0().h_full().rounded(s(2.)).bg(color)
        }))
}

fn summary_history(services: &[ServiceHealth]) -> [ServiceState; HISTORY_SLOTS] {
    let latest = services
        .iter()
        .flat_map(|service| &service.heartbeats)
        .filter_map(|beat| beat.time)
        .max();
    let Some(latest) = latest else {
        return [ServiceState::Unknown; HISTORY_SLOTS];
    };

    let mut summary = [ServiceState::Up; HISTORY_SLOTS];
    for service in services {
        let mut minutes = [None; HISTORY_SLOTS];
        for beat in &service.heartbeats {
            let Some(at) = beat.time else { continue };
            let minutes_ago = (latest - at).whole_minutes() as usize;
            if minutes_ago < HISTORY_SLOTS {
                let slot = &mut minutes[HISTORY_SLOTS - 1 - minutes_ago];
                *slot = Some(slot.map_or(beat.state, |state| worst_state(state, beat.state)));
            }
        }
        for (state, minute) in summary.iter_mut().zip(minutes) {
            *state = worst_state(*state, minute.unwrap_or(ServiceState::Unknown));
        }
    }
    summary
}

fn worst_state(left: ServiceState, right: ServiceState) -> ServiceState {
    let severity = |state| match state {
        ServiceState::Down => 4,
        ServiceState::Pending => 3,
        ServiceState::Maintenance => 2,
        ServiceState::Unknown => 1,
        ServiceState::Up => 0,
    };
    if severity(left) >= severity(right) {
        left
    } else {
        right
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::ServiceHeartbeat;
    use time::{Duration, macros::datetime};

    #[test]
    fn summary_aligns_checks_by_minute_and_marks_missing_data() {
        use ServiceState::{Down, Maintenance, Unknown, Up};

        let newest = datetime!(2026-11-25 10:00:00 UTC);
        let service = |checks: &[(i64, ServiceState)]| ServiceHealth {
            name: String::new(),
            state: Up,
            heartbeats: checks
                .iter()
                .map(|&(seconds_ago, state)| ServiceHeartbeat {
                    state,
                    time: Some(newest - Duration::seconds(seconds_ago)),
                })
                .collect(),
        };
        let history = summary_history(&[
            service(&[(195, Up), (135, Up), (75, Down), (0, Up)]),
            service(&[(200, Up), (150, Maintenance), (15, Up)]),
        ]);
        assert_eq!(history[..HISTORY_SLOTS - 4], [Unknown; HISTORY_SLOTS - 4]);
        assert_eq!(history[HISTORY_SLOTS - 4..], [Up, Maintenance, Down, Up]);
        let partial = summary_history(&[service(&[(60, Up), (0, Up)]), service(&[(20, Up)])]);
        assert_eq!(partial[HISTORY_SLOTS - 2..], [Unknown, Up]);
    }
}
