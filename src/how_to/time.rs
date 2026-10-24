//! UTC times for runs, as text a person can read in the file: YYYY-MM-DDTHH:MM:SSZ. No time
//! library: the civil-date arithmetic is persistence::civil_from_days (Howard Hinnant's algorithm)
//! and its inverse persistence::days_from_civil.

/// Milliseconds since 1970 as YYYY-MM-DDTHH:MM:SSZ.
pub fn iso(ms: u64) -> String {
    let secs = ms / 1000;
    let (y, m, d) = crate::persistence::civil_from_days((secs / 86_400) as i64);
    let t = secs % 86_400;
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", t / 3600, t / 60 % 60, t % 60)
}

/// Milliseconds since 1970 as YYYYMMDD-HHMMSS (a run's id).
pub fn compact(ms: u64) -> String {
    iso(ms).replace(['-', ':'], "").replace('T', "-").trim_end_matches('Z').to_string()
}

/// The day, YYYY-MM-DD, of a time.
pub fn day(ms: u64) -> String {
    iso(ms)[..10].to_string()
}

/// YYYY-MM-DDTHH:MM:SSZ back to milliseconds since 1970.
pub fn ms_from_iso(s: &str) -> Option<u64> {
    let b = s.as_bytes();
    if b.len() != 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' || b[19] != b'Z' {
        return None;
    }
    let n = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, m, d) = (n(0..4)?, n(5..7)?, n(8..10)?);
    let (hh, mm, ss) = (n(11..13)?, n(14..16)?, n(17..19)?);
    let days = crate::persistence::days_from_civil(y, m, d);
    let secs = days * 86_400 + hh * 3600 + mm * 60 + ss;
    u64::try_from(secs).ok().map(|s| s * 1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_goes_to_text_and_back() {
        // 2026-10-23T21:02:11Z
        let ms = 1_792_789_331_000;
        assert_eq!(iso(ms), "2026-10-23T21:02:11Z");
        assert_eq!(ms_from_iso("2026-10-23T21:02:11Z"), Some(ms));
        assert_eq!(compact(ms), "20261023-210211");
        assert_eq!(day(ms), "2026-10-23");
        assert_eq!(ms_from_iso("2026-10-23 21:02:11"), None);
        assert_eq!(ms_from_iso(&iso(0)), Some(0));
        assert_eq!(iso(951_782_400_000), "2000-02-29T00:00:00Z", "a leap day");
    }
}
