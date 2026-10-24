//! The chat's timestamps: the app-wide display format (Settings), the formatter every page's
//! times go through, and the always-full one the message pill's hover shows. All UTC. Moved
//! VERBATIM out of `gui/pages/chat.rs` (2026-10-24) under its line budget in
//! tests/file_size_ratchet.rs, and re-exported there (`pub use timestamps::*`), so every
//! `crate::gui::pages::chat::format_timestamp` and the rest still resolve.

/// Format a UNIX millisecond timestamp as "YYYY-MM-DD HH:MM:SS UTC".
/// Uses Howard Hinnant's days-from-civil algorithm so we don't need
/// chrono as a dependency. Accurate for any proleptic-Gregorian year.
///
/// Used by the Þ-hover timestamp-expansion overlay in the message
/// pill (operator feedback 2026-05-12 — "when we interact with the
/// timestamp ... on the left the timestamp expands to include the
/// YEAR:MONTH:DAY:HOUR:MINUTE:SECOND").
/// Always-full timestamp (the pill-expand hover popup), independent of the
/// user's display-format setting — the popup is the "show me everything" view.
pub fn format_full_timestamp(ts_ms: u64) -> String {
    let (year, month, day, hour, minute, second, _ms) = ts_parts(ts_ms);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
        year, month, day, hour, minute, second
    )
}

/// Return a human-readable timestamp string for "now".
pub(crate) fn chrono_now_str() -> String {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    // Route through the setting-aware formatter so a local send-echo matches the
    // user's chosen timestamp format (and the relay-reconciled copy).
    format_timestamp(now_ms)
}

/// Format a Unix-millis timestamp to HH:MM UTC.
/// User-selectable timestamp display granularity (operator request). All UTC.
/// Drives `format_timestamp` app-wide via a process-global so the pure formatter
/// doesn't need GuiState threaded through every call site.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TimestampFormat {
    HourMin,        // 17:42 UTC   (default)
    HourMinSec,     // 17:42:09 UTC
    DateHourMin,    // 2026-05-29 17:42 UTC
    DateHourMinSec, // 2026-05-29 17:42:09 UTC
    Full,           // 2026-05-29 17:42:09.123 UTC
}
impl TimestampFormat {
    /// Stable string for config persistence.
    pub fn as_str(self) -> &'static str {
        match self {
            TimestampFormat::HourMin => "hour_min",
            TimestampFormat::HourMinSec => "hour_min_sec",
            TimestampFormat::DateHourMin => "date_hour_min",
            TimestampFormat::DateHourMinSec => "date_hour_min_sec",
            TimestampFormat::Full => "full",
        }
    }
    pub fn from_config_str(s: &str) -> Self {
        match s {
            "hour_min_sec" => TimestampFormat::HourMinSec,
            "date_hour_min" => TimestampFormat::DateHourMin,
            "date_hour_min_sec" => TimestampFormat::DateHourMinSec,
            "full" => TimestampFormat::Full,
            _ => TimestampFormat::HourMin,
        }
    }
    /// Human label for the settings dropdown (with a live example).
    pub fn label(self) -> &'static str {
        match self {
            TimestampFormat::HourMin => "Time, 17:42",
            TimestampFormat::HourMinSec => "Time + seconds, 17:42:09",
            TimestampFormat::DateHourMin => "Date + time, 2026-05-29 17:42",
            TimestampFormat::DateHourMinSec => "Date + time + seconds, 2026-05-29 17:42:09",
            TimestampFormat::Full => "Full + milliseconds, 2026-05-29 17:42:09.123",
        }
    }
    pub const ALL: [TimestampFormat; 5] = [
        TimestampFormat::HourMin,
        TimestampFormat::HourMinSec,
        TimestampFormat::DateHourMin,
        TimestampFormat::DateHourMinSec,
        TimestampFormat::Full,
    ];
    fn discriminant(self) -> usize {
        TimestampFormat::ALL.iter().position(|&f| f == self).unwrap_or(0)
    }
    fn from_discriminant(d: usize) -> Self {
        TimestampFormat::ALL.get(d).copied().unwrap_or(TimestampFormat::HourMin)
    }
}

static TS_FORMAT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// Set the app-wide timestamp display format (call on config load + settings change).
pub fn set_timestamp_format(f: TimestampFormat) {
    TS_FORMAT.store(f.discriminant(), std::sync::atomic::Ordering::Relaxed);
}
/// The current app-wide timestamp display format.
pub fn timestamp_format() -> TimestampFormat {
    TimestampFormat::from_discriminant(TS_FORMAT.load(std::sync::atomic::Ordering::Relaxed))
}

/// Break an epoch-ms timestamp into UTC (year, month, day, hour, minute, second, millis).
/// Howard Hinnant days-from-civil → calendar date (no chrono dependency).
fn ts_parts(ts_ms: u64) -> (i32, u32, u32, i64, i64, i64, u64) {
    let unix_s = (ts_ms / 1000) as i64;
    let millis = ts_ms % 1000;
    let days = unix_s.div_euclid(86_400);
    let secs_in_day = unix_s.rem_euclid(86_400);
    let hour = secs_in_day / 3_600;
    let minute = (secs_in_day % 3_600) / 60;
    let second = secs_in_day % 60;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { (mp + 3) as u32 } else { (mp - 9) as u32 };
    let year = if month <= 2 { (y + 1) as i32 } else { y as i32 };
    (year, month, day, hour, minute, second, millis)
}

/// Format an epoch-ms timestamp per the user's chosen `timestamp_format()` —
/// used for the message-pill across the whole chat, so the setting applies
/// everywhere at once. Always UTC.
pub fn format_timestamp(ts: u64) -> String {
    let (y, mo, d, h, mi, s, ms) = ts_parts(ts);
    match timestamp_format() {
        TimestampFormat::HourMin => format!("{:02}:{:02} UTC", h, mi),
        TimestampFormat::HourMinSec => format!("{:02}:{:02}:{:02} UTC", h, mi, s),
        TimestampFormat::DateHourMin => format!("{:04}-{:02}-{:02} {:02}:{:02} UTC", y, mo, d, h, mi),
        TimestampFormat::DateHourMinSec => {
            format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC", y, mo, d, h, mi, s)
        }
        TimestampFormat::Full => {
            format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03} UTC", y, mo, d, h, mi, s, ms)
        }
    }
}
