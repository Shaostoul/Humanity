//! Quiet hours: a daily window, by the device's own clock, in which nothing notifies. The two ends
//! are the person's and synced through their server (`notification_prefs.dnd_start`/`dnd_end`).
//!
//! The server stores them as text. The desktop writes "22:00"; the website wrote "22" (an hour)
//! until 2026-10-30, and a stored row may still hold that, so [`parse_clock`] reads both. A
//! window whose end comes before its start runs past midnight (22:00 to 07:00); one whose ends are
//! equal is empty (nothing is silenced), since "all day" is what Mute everything is for.

/// "22", "22:00", "7:30" (24-hour) to minutes after midnight. `None` for anything else.
pub fn parse_clock(s: &str) -> Option<u16> {
    let s = s.trim();
    let (h, m) = match s.split_once(':') {
        Some((h, m)) => (h, m),
        None => (s, "0"),
    };
    let digits = |p: &str| !p.is_empty() && p.len() <= 2 && p.bytes().all(|b| b.is_ascii_digit());
    if !digits(h) || !digits(m) {
        return None;
    }
    let h: u16 = h.parse().ok()?;
    let m: u16 = m.parse().ok()?;
    (h < 24 && m < 60).then_some(h * 60 + m)
}

/// Minutes after midnight as "HH:MM", the shape both apps now write.
pub fn format_clock(minutes: u16) -> String {
    let m = minutes % (24 * 60);
    format!("{:02}:{:02}", m / 60, m % 60)
}

/// The window, when both ends are set and read.
pub fn window(start: Option<&str>, end: Option<&str>) -> Option<(u16, u16)> {
    Some((parse_clock(start?)?, parse_clock(end?)?))
}

/// Is `minute` (after midnight) inside the window from `start` to `end`?
pub fn in_quiet(start: u16, end: u16, minute: u16) -> bool {
    if start == end {
        false
    } else if start < end {
        minute >= start && minute < end
    } else {
        minute >= start || minute < end
    }
}

/// The device's local minute after midnight at Unix second `secs` (its own time zone, summer time
/// included, from the same clock the chat's timestamps read).
pub fn local_minute(secs: i64) -> u16 {
    let local = secs + crate::gui::pages::chat::local_offset_secs(secs);
    (local.rem_euclid(86_400) / 60) as u16
}
