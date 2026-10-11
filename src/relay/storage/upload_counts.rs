//! Upload allowances, the counting half (2026-10-11; docs/design/media-showcase.md, follow-up B3;
//! the operator's answers of 2026-10-11, number 3: "a limit of number of uploads per user and a max
//! upload bandwidth per tier of user"). The rules that read these numbers are in
//! relay/upload_rules.rs (`allowance`), shared by both upload routes.
//!
//! One table, `upload_counts` (created in storage/mod.rs): for each person, how many bytes they
//! uploaded in the current UTC day (`span` 'day') and how many uploads they started in the current
//! clock hour ('hour'). A window is fixed, not rolling, so the relay can say exactly when it
//! resets: midnight UTC, or the top of the hour. Rows of windows that have ended are deleted
//! whenever a new amount is added, so the table holds at most two rows per person who uploaded
//! today.

use super::Storage;
use rusqlite::{params, OptionalExtension};

/// The day's window, in Unix milliseconds: midnight UTC to midnight UTC.
pub const DAY_MS: i64 = 24 * 60 * 60 * 1000;
/// The hour's window: the clock hour.
pub const HOUR_MS: i64 = 60 * 60 * 1000;

/// The two kinds of allowance: bytes in a day, uploads started in an hour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Span {
    Day,
    Hour,
}

impl Span {
    /// The word stored in `upload_counts.span`.
    pub fn word(self) -> &'static str {
        match self {
            Span::Day => "day",
            Span::Hour => "hour",
        }
    }
    /// How long one window is, in milliseconds.
    pub fn length_ms(self) -> i64 {
        match self {
            Span::Day => DAY_MS,
            Span::Hour => HOUR_MS,
        }
    }
    /// When the window holding `now_ms` began.
    pub fn starts_at(self, now_ms: i64) -> i64 {
        now_ms - now_ms.rem_euclid(self.length_ms())
    }
    /// When the window holding `now_ms` ends (the next one begins): when the allowance resets.
    pub fn resets_at(self, now_ms: i64) -> i64 {
        self.starts_at(now_ms) + self.length_ms()
    }
}

impl Storage {
    /// How much `key` has used of `span`'s allowance in the window holding `now_ms`: bytes for the
    /// day, uploads for the hour. 0 when nothing is counted (or the count cannot be read: the
    /// error is logged, and the upload is not stopped for the relay's own fault).
    pub fn upload_count(&self, key: &str, span: Span, now_ms: i64) -> i64 {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT amount FROM upload_counts WHERE public_key = ?1 AND span = ?2 AND starts_at = ?3",
                params![key, span.word(), span.starts_at(now_ms)],
                |r| r.get::<_, i64>(0),
            )
            .optional()
        })
        .unwrap_or_else(|e| {
            tracing::error!("Upload allowance: could not read a count ({e})");
            None
        })
        .unwrap_or(0)
    }

    /// Add `amount` to `key`'s count of `span` in the window holding `now_ms`, and delete every
    /// row of a window that has ended (anyone's).
    pub fn add_upload_count(&self, key: &str, span: Span, now_ms: i64, amount: i64) -> Result<(), rusqlite::Error> {
        if amount <= 0 {
            return Ok(());
        }
        let starts = span.starts_at(now_ms);
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO upload_counts (public_key, span, starts_at, amount) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (public_key, span, starts_at) DO UPDATE SET amount = amount + excluded.amount",
                params![key, span.word(), starts, amount],
            )?;
            conn.execute("DELETE FROM upload_counts WHERE span = ?1 AND starts_at < ?2", params![span.word(), starts])?;
            Ok(())
        })
    }

    /// Take `amount` back off `key`'s count of `span` in the window holding `now_ms`, never below 0:
    /// a resumable piece counted before it was written, whose write failed (R3-1, 2026-10-11).
    pub fn take_back_upload_count(&self, key: &str, span: Span, now_ms: i64, amount: i64) -> Result<(), rusqlite::Error> {
        if amount <= 0 {
            return Ok(());
        }
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE upload_counts SET amount = MAX(amount - ?4, 0) WHERE public_key = ?1 AND span = ?2 AND starts_at = ?3",
                params![key, span.word(), span.starts_at(now_ms), amount],
            )
            .map(|_| ())
        })
    }
}
