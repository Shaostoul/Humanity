//! The app's timestamps: the display format and time zone (Settings), the formatter every page's
//! times go through, and the always-full one the message pill's hover shows. Moved out of
//! `gui/pages/chat.rs` (2026-10-24) under its line budget in tests/file_size_ratchet.rs, and
//! re-exported there (`pub use timestamps::*`), so every `crate::gui::pages::chat::format_timestamp`
//! and the rest still resolve.
//!
//! Times are shown in this computer's own time by default, as the website shows them, or in UTC
//! (Settings > Time zone, 2026-10-24). Until then the app wrote every time in UTC (19:48) while
//! the website wrote the same message's time locally (12:48). The offset comes from the system
//! for each time shown, daylight saving included, so a time from last winter shows in last
//! winter's offset: Windows' own `SystemTimeToTzSpecificLocalTime`, `localtime_r` elsewhere. No
//! time library, and nothing kept between calls (the no-caches rule).

/// Format a UNIX millisecond timestamp as "YYYY-MM-DD HH:MM:SS" plus its zone: "(UTC-07:00)"
/// in this computer's time, "UTC" in UTC. Uses Howard Hinnant's days-from-civil algorithm so we
/// don't need chrono as a dependency. Accurate for any proleptic-Gregorian year.
///
/// Used by the Þ-hover timestamp-expansion overlay in the message
/// pill (operator feedback 2026-05-12 — "when we interact with the
/// timestamp ... on the left the timestamp expands to include the
/// YEAR:MONTH:DAY:HOUR:MINUTE:SECOND").
/// Always-full timestamp (the pill-expand hover popup), independent of the
/// user's display-format setting — the popup is the "show me everything" view.
pub fn format_full_timestamp(ts_ms: u64) -> String {
    full_in(ts_ms, timestamp_zone())
}

/// `format_full_timestamp` in a given zone.
fn full_in(ts_ms: u64, zone: TimestampZone) -> String {
    let (shown_ms, offset) = shown_in(ts_ms, zone);
    let (year, month, day, hour, minute, second, _ms) = ts_parts(shown_ms);
    let zone = match offset {
        Some(off) => format!("(UTC{}{:02}:{:02})", if off < 0 { '-' } else { '+' }, off.abs() / 3600, off.abs() % 3600 / 60),
        None => "UTC".to_string(),
    };
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02} {zone}", year, month, day, hour, minute, second)
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

/// User-selectable timestamp display granularity (operator request), in the zone
/// `timestamp_zone()` says. Drives `format_timestamp` app-wide via a process-global
/// so the pure formatter doesn't need GuiState threaded through every call site.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TimestampFormat {
    HourMin,        // 17:42   (default)
    HourMinSec,     // 17:42:09
    DateHourMin,    // 2026-05-29 17:42
    DateHourMinSec, // 2026-05-29 17:42:09
    Full,           // 2026-05-29 17:42:09.123
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
    /// Human label for the settings dropdown (with a live example), from the word source; the
    /// website's Settings names the same five with the same keys.
    pub fn label(self) -> &'static str {
        use crate::i18n::t;
        match self {
            TimestampFormat::HourMin => t("settings.timestamp_format_hour_min"),
            TimestampFormat::HourMinSec => t("settings.timestamp_format_hour_min_sec"),
            TimestampFormat::DateHourMin => t("settings.timestamp_format_date_hour_min"),
            TimestampFormat::DateHourMinSec => t("settings.timestamp_format_date_hour_min_sec"),
            TimestampFormat::Full => t("settings.timestamp_format_full"),
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

/// Whether times are shown in this computer's own time (the default, as the website shows them)
/// or in UTC, the one time used everywhere (Settings > Time zone; config `timestamp_zone`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TimestampZone {
    Local,
    Utc,
}
impl TimestampZone {
    /// Stable string for config persistence.
    pub fn as_str(self) -> &'static str {
        match self {
            TimestampZone::Local => "local",
            TimestampZone::Utc => "utc",
        }
    }
    /// "utc" or "local"; anything else (an empty field) is the default.
    pub fn from_config_str(s: &str) -> Self {
        match s {
            "utc" => TimestampZone::Utc,
            "local" => TimestampZone::Local,
            _ => Self::DEFAULT,
        }
    }
    /// This computer's time. In the lib tests, UTC: a test that checks a written time must not
    /// depend on where the machine running it is (CI is in UTC, the operator's PC is not), and a
    /// config applied by one test must not move another's clock.
    pub const DEFAULT: TimestampZone = if cfg!(test) { TimestampZone::Utc } else { TimestampZone::Local };
    /// Its name in the Settings menu.
    pub fn label(self) -> &'static str {
        match self {
            TimestampZone::Local => crate::i18n::t("settings.timestamp_zone_local"),
            TimestampZone::Utc => crate::i18n::t("settings.timestamp_zone_utc"),
        }
    }
    pub const ALL: [TimestampZone; 2] = [TimestampZone::Local, TimestampZone::Utc];
}

/// 0 = this computer's time, 1 = UTC; starts at `TimestampZone::DEFAULT`.
static TS_ZONE: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(if matches!(TimestampZone::DEFAULT, TimestampZone::Utc) { 1 } else { 0 });
/// Set the app-wide time zone times are shown in (call on config load + settings change).
pub fn set_timestamp_zone(z: TimestampZone) {
    TS_ZONE.store(if z == TimestampZone::Utc { 1 } else { 0 }, std::sync::atomic::Ordering::Relaxed);
}
/// The app-wide time zone times are shown in.
pub fn timestamp_zone() -> TimestampZone {
    if TS_ZONE.load(std::sync::atomic::Ordering::Relaxed) == 1 { TimestampZone::Utc } else { TimestampZone::Local }
}

/// This computer's offset from UTC at `unix_secs`, in seconds east of UTC (California in summer
/// is -25200), daylight saving included by the system's own rules for that date; 0 when the
/// system cannot say.
pub fn local_offset_secs(unix_secs: i64) -> i64 {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::SYSTEMTIME;
        use windows::Win32::System::Time::SystemTimeToTzSpecificLocalTime;
        let (y, m, d) = crate::persistence::civil_from_days(unix_secs.div_euclid(86_400));
        let s = unix_secs.rem_euclid(86_400);
        let utc = SYSTEMTIME {
            wYear: y as u16,
            wMonth: m as u16,
            wDayOfWeek: 0,
            wDay: d as u16,
            wHour: (s / 3600) as u16,
            wMinute: (s % 3600 / 60) as u16,
            wSecond: (s % 60) as u16,
            wMilliseconds: 0,
        };
        let mut local = SYSTEMTIME::default();
        // SAFETY: both pointers are to live locals of the right type; None asks for the zone the
        // computer is set to.
        if unsafe { SystemTimeToTzSpecificLocalTime(None, &utc, &mut local) }.is_err() {
            return 0;
        }
        let local_secs = crate::persistence::days_from_civil(local.wYear as i64, local.wMonth as i64, local.wDay as i64) * 86_400
            + local.wHour as i64 * 3600
            + local.wMinute as i64 * 60
            + local.wSecond as i64;
        local_secs - unix_secs
    }
    #[cfg(unix)]
    {
        let t = unix_secs as libc::time_t;
        // SAFETY: a zeroed `tm` is a valid value of that plain C struct, and localtime_r (the
        // thread-safe form) writes only into it.
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        if unsafe { libc::localtime_r(&t, &mut tm) }.is_null() {
            return 0;
        }
        tm.tm_gmtoff as i64
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = unix_secs;
        0
    }
}

/// The time to write for `ts_ms` in `zone`, and the offset applied (None in UTC): `ts_parts`
/// of the first is the clock reading a person in that zone saw.
fn shown_in(ts_ms: u64, zone: TimestampZone) -> (u64, Option<i64>) {
    match zone {
        TimestampZone::Utc => (ts_ms, None),
        TimestampZone::Local => {
            let off = local_offset_secs((ts_ms / 1000) as i64);
            ((ts_ms as i64 + off * 1000).max(0) as u64, Some(off))
        }
    }
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
    let (year, month, day) = crate::persistence::civil_from_days(days);
    (year as i32, month, day, hour, minute, second, millis)
}

/// Format an epoch-ms timestamp per the user's chosen `timestamp_format()` and
/// `timestamp_zone()`: used for the message-pill across the whole chat, so the
/// settings apply everywhere at once. In UTC the time says so (" UTC"); in this
/// computer's time it is written bare, as the website writes it.
pub fn format_timestamp(ts: u64) -> String {
    timestamp_in(ts, timestamp_format(), timestamp_zone())
}

/// `format_timestamp` in a given format and zone.
fn timestamp_in(ts: u64, format: TimestampFormat, zone: TimestampZone) -> String {
    let (shown_ms, offset) = shown_in(ts, zone);
    let (y, mo, d, h, mi, s, ms) = ts_parts(shown_ms);
    let zone = if offset.is_some() { "" } else { " UTC" };
    match format {
        TimestampFormat::HourMin => format!("{:02}:{:02}{zone}", h, mi),
        TimestampFormat::HourMinSec => format!("{:02}:{:02}:{:02}{zone}", h, mi, s),
        TimestampFormat::DateHourMin => format!("{:04}-{:02}-{:02} {:02}:{:02}{zone}", y, mo, d, h, mi),
        TimestampFormat::DateHourMinSec => {
            format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}{zone}", y, mo, d, h, mi, s)
        }
        TimestampFormat::Full => {
            format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}{zone}", y, mo, d, h, mi, s, ms)
        }
    }
}

/// A date and time to the minute ("2026-10-17 14:30"), in the zone the setting says, for a file's
/// date changed and the like: the same rule as the chat's times.
pub fn format_date_minute(ts_ms: u64) -> String {
    date_minute_in(ts_ms, timestamp_zone())
}

/// `format_date_minute` in a given zone.
fn date_minute_in(ts_ms: u64, zone: TimestampZone) -> String {
    let (shown_ms, offset) = shown_in(ts_ms, zone);
    let (y, mo, d, h, mi, _, _) = ts_parts(shown_ms);
    format!("{:04}-{:02}-{:02} {:02}:{:02}{}", y, mo, d, h, mi, if offset.is_some() { "" } else { " UTC" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_are_written_in_the_zone_asked_with_the_systems_own_offset() {
        // 2026-10-24 19:48:24 UTC. Through the pure forms: the settings are process-wide, and the
        // lib tests run side by side.
        let ts = 1_792_871_304_000u64;
        let utc = TimestampZone::Utc;
        assert_eq!(timestamp_in(ts, TimestampFormat::DateHourMinSec, utc), "2026-10-24 19:48:24 UTC");
        assert_eq!(timestamp_in(ts, TimestampFormat::HourMin, utc), "19:48 UTC");
        assert_eq!(full_in(ts, utc), "2026-10-24 19:48:24 UTC");
        assert_eq!(date_minute_in(ts, utc), "2026-10-24 19:48 UTC");
        let local = TimestampZone::Local;
        let off = local_offset_secs((ts / 1000) as i64);
        let (y, mo, d, h, mi, s, _) = ts_parts((ts as i64 + off * 1000) as u64);
        assert_eq!(timestamp_in(ts, TimestampFormat::DateHourMinSec, local), format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}"), "this computer's time, written bare");
        assert_eq!(timestamp_in(ts, TimestampFormat::HourMin, local), format!("{h:02}:{mi:02}"));
        let sign = if off < 0 { '-' } else { '+' };
        assert_eq!(full_in(ts, local), format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02} (UTC{sign}{:02}:{:02})", off.abs() / 3600, off.abs() % 3600 / 60));
    }

    #[test]
    fn an_empty_setting_is_the_default_and_the_tests_default_is_utc() {
        assert_eq!(TimestampZone::from_config_str(""), TimestampZone::DEFAULT);
        assert_eq!(TimestampZone::DEFAULT, TimestampZone::Utc, "under cfg(test)");
        assert_eq!(TimestampZone::from_config_str("local"), TimestampZone::Local);
        assert_eq!(TimestampZone::from_config_str(TimestampZone::Utc.as_str()), TimestampZone::Utc);
    }

    #[test]
    fn the_offset_is_a_real_time_zones() {
        // Every zone on Earth is within 14 hours of UTC and a whole number of quarter hours.
        for ts in [0i64, 1_767_225_600, 1_782_864_000, 1_792_871_304] {
            let off = local_offset_secs(ts);
            eprintln!("local offset at {ts}: {off} s"); // to compare by hand with another source (--nocapture)
            assert!(off.abs() <= 14 * 3600, "{off}");
            assert_eq!(off % 900, 0, "{off}");
        }
    }

    #[test]
    fn the_date_arithmetic_round_trips() {
        for days in [-719_468i64, -1, 0, 19_000, 20_385, 2_932_896] {
            let (y, m, d) = crate::persistence::civil_from_days(days);
            assert_eq!(crate::persistence::days_from_civil(y, m as i64, d as i64), days);
        }
    }
}
