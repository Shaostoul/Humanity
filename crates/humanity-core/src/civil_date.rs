//! Calendar dates from day counts: the civil (proleptic Gregorian) date of a count of days
//! since 1970-01-01, by Howard Hinnant's days-from-civil algorithm.
//!
//! WHY IT LIVES IN THE CORE (the crate split's rung 0, 2026-10-31,
//! docs/design/crate-split.md): it was the relay's (`relay::epoch_days_to_ymd`), and the app's
//! household page (net::household, "10 November 2026") reached into the server for it. Std
//! only. Several pages still carry their own copy of the same arithmetic; this is the one the
//! relay and the household page share.

/// Convert epoch days (since 1970-01-01) to (year, month, day).
pub fn epoch_days_to_ymd(days: i64) -> (i64, u32, u32) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
