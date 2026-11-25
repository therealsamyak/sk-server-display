//! Status value formatting: bytes, rates, uptime, ages, and clock text.

use std::time::Duration;

use time::{OffsetDateTime, UtcOffset, macros::format_description};

const KIB: f64 = 1024.0;

/// Formats a byte count with binary units, such as `11.2 GiB`.
pub fn format_status_bytes(bytes: u64) -> String {
    let units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= KIB && unit < units.len() - 1 {
        value /= KIB;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else if value >= 100.0 {
        format!("{value:.0} {}", units[unit])
    } else {
        format!("{value:.1} {}", units[unit])
    }
}

/// Formats a used and total byte pair, such as `11.2 / 32.0 GiB`.
pub fn format_status_usage(used: u64, total: u64) -> String {
    let total_text = format_status_bytes(total);
    let unit = total_text.split(' ').nth(1).unwrap_or("B");
    let divisor = match unit {
        "KiB" => KIB,
        "MiB" => KIB.powi(2),
        "GiB" => KIB.powi(3),
        "TiB" => KIB.powi(4),
        "PiB" => KIB.powi(5),
        _ => 1.0,
    };
    let used_value = used as f64 / divisor;
    let total_value = total as f64 / divisor;
    format!("{used_value:.1} / {total_value:.1} {unit}")
}

/// Formats a transfer rate, such as `38 B/s` or `1.2 MiB/s`.
pub fn format_status_rate(bytes_per_second: f64) -> String {
    format!(
        "{}/s",
        format_status_bytes(bytes_per_second.max(0.0).round() as u64)
    )
}

/// Formats an uptime, such as `41d 6h` or `3h 12m`.
pub fn format_status_uptime(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

/// Formats a data age, such as `4s`, `2m 5s`, or `3h 10m`.
pub fn format_status_age(age: Duration) -> String {
    let seconds = age.as_secs();
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3_600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else if seconds < 86_400 {
        format!("{}h {}m", seconds / 3_600, (seconds % 3_600) / 60)
    } else {
        format!("{}d {}h", seconds / 86_400, (seconds % 86_400) / 3_600)
    }
}

/// Time until the text of [`format_status_age`] changes, as the age grows.
pub fn status_age_text_valid_for(age: Duration) -> Duration {
    let step = match age.as_secs() {
        0..3_600 => 1,
        3_600..86_400 => 60,
        _ => 3_600,
    };
    let step = Duration::from_secs(step);
    step - Duration::from_nanos((age.as_nanos() % step.as_nanos()) as u64)
}

/// Time until the text of [`format_status_clock`] changes: the time to the
/// next minute.
pub fn status_clock_text_valid_for(at: OffsetDateTime) -> Duration {
    let into_minute = Duration::from_secs(u64::from(at.second()))
        + Duration::from_nanos(u64::from(at.nanosecond()));
    Duration::from_secs(60).saturating_sub(into_minute)
}

/// Formats a wall-clock time in the local offset, such as `14:02`.
pub fn format_status_clock(at: OffsetDateTime, offset: UtcOffset) -> String {
    at.to_offset(offset)
        .format(format_description!("[hour]:[minute]"))
        .unwrap_or_default()
}

/// Formats a calendar date in the local offset, such as `Monday 23 November`.
pub fn format_status_date(at: OffsetDateTime, offset: UtcOffset) -> String {
    at.to_offset(offset)
        .format(format_description!(
            "[weekday] [day padding:none] [month repr:long]"
        ))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::{datetime, offset};

    #[test]
    fn formats_bytes_and_usage() {
        assert_eq!(format_status_bytes(512), "512 B");
        assert_eq!(format_status_bytes(12_025_908_428), "11.2 GiB");
        assert_eq!(
            format_status_usage(12_025_908_428, 34_359_738_368),
            "11.2 / 32.0 GiB"
        );
    }

    #[test]
    fn formats_rates_uptime_and_age() {
        assert_eq!(format_status_rate(38.0), "38 B/s");
        assert_eq!(format_status_rate(1_258_291.0), "1.2 MiB/s");
        assert_eq!(format_status_uptime(3_564_000), "41d 6h");
        assert_eq!(format_status_uptime(11_520), "3h 12m");
        assert_eq!(format_status_age(Duration::from_secs(125)), "2m 5s");
        assert_eq!(format_status_age(Duration::from_secs(185_000)), "2d 3h");
    }

    #[test]
    fn age_and_clock_text_change_at_the_next_step() {
        let ms = Duration::from_millis;
        assert_eq!(status_age_text_valid_for(ms(4_250)), ms(750));
        assert_eq!(status_age_text_valid_for(ms(125_000)), ms(1_000));
        assert_eq!(status_age_text_valid_for(ms(3_630_000)), ms(30_000));
        assert_eq!(status_age_text_valid_for(ms(90_600_000)), ms(3_000_000));
        let at = datetime!(2026-11-23 22:02:11.5 UTC);
        assert_eq!(status_clock_text_valid_for(at), ms(48_500));
    }

    #[test]
    fn formats_clock_in_local_offset() {
        let at = datetime!(2026-11-23 22:02:11 UTC);
        assert_eq!(format_status_clock(at, offset!(-8)), "14:02");
        assert_eq!(format_status_date(at, offset!(-8)), "Monday 23 November");
    }
}
