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

/// Formats a network rate with decimal units, such as `12.4 MB/s`.
pub fn format_status_rate(bytes_per_second: u64) -> String {
    let value = bytes_per_second as f64;
    if value >= 1e9 {
        format!("{:.1} GB/s", value / 1e9)
    } else if value >= 1e6 {
        format!("{:.1} MB/s", value / 1e6)
    } else if value >= 1e3 {
        format!("{:.0} kB/s", value / 1e3)
    } else {
        format!("{bytes_per_second} B/s")
    }
}

/// Formats an uptime, such as `41 d 6 h` or `3 h 12 m`.
pub fn format_status_uptime(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;
    if days > 0 {
        format!("{days} d {hours} h")
    } else if hours > 0 {
        format!("{hours} h {minutes} m")
    } else {
        format!("{minutes} m")
    }
}

/// Formats a data age, such as `4 s`, `2 m 5 s`, or `3 h 10 m`.
pub fn format_status_age(age: Duration) -> String {
    let seconds = age.as_secs();
    if seconds < 60 {
        format!("{seconds} s")
    } else if seconds < 3_600 {
        format!("{} m {} s", seconds / 60, seconds % 60)
    } else {
        format!("{} h {} m", seconds / 3_600, (seconds % 3_600) / 60)
    }
}

/// Formats a wall-clock time in the local offset, such as `14:02:11`.
pub fn format_status_clock(at: OffsetDateTime, offset: UtcOffset) -> String {
    at.to_offset(offset)
        .format(format_description!("[hour]:[minute]:[second]"))
        .unwrap_or_default()
}

/// Formats a calendar date in the local offset, such as `Mon 23 Nov 2026`.
pub fn format_status_date(at: OffsetDateTime, offset: UtcOffset) -> String {
    at.to_offset(offset)
        .format(format_description!(
            "[weekday repr:short] [day padding:none] [month repr:short] [year]"
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
        assert_eq!(format_status_rate(12_400_000), "12.4 MB/s");
        assert_eq!(format_status_rate(950), "950 B/s");
        assert_eq!(format_status_uptime(3_564_000), "41 d 6 h");
        assert_eq!(format_status_uptime(11_520), "3 h 12 m");
        assert_eq!(format_status_age(Duration::from_secs(125)), "2 m 5 s");
    }

    #[test]
    fn formats_clock_in_local_offset() {
        let at = datetime!(2026-11-23 22:02:11 UTC);
        assert_eq!(format_status_clock(at, offset!(-8)), "14:02:11");
        assert_eq!(format_status_date(at, offset!(-8)), "Mon 23 Nov 2026");
    }
}
