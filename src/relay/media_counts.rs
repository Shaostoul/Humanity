//! Views and downloads on the Media page, counted once a person a UTC day (Media round 5,
//! 2026-10-12; docs/design/media-round-5-plan.md 3.5, decisions 4 and 5).
//!
//! WHO A PERSON IS, AND WHAT IS KEPT. A person is their key (a ticket's, or a token's), else their
//! network address. Whether they were counted today is a MARK: the first 16 bytes of BLAKE3, keyed
//! with a secret drawn at random for each UTC day and held only in memory, over
//! `hum/media-count/v1\n<view|download>\n<item>\n<m:key | a:address>`. At UTC midnight the marks
//! are forgotten and the secret is replaced, so nothing about who looked outlives the day, and
//! only the numbers (`media_items.views`, `.downloads`) are written to disk. A restart forgets the
//! marks too, so a person can be counted twice on the day of a restart: an overcount by one, never
//! a record of who. At most [`MARKS_PER_DAY`] marks a day (about 32 MB); past that nothing more is
//! counted that day, with one log line.
//!
//! THE ROUTES (`POST /api/media/viewed`, `GET /api/media/download/{id}`) are lane 1B's: until it
//! lands they answer 501 "Not built yet.", with the signatures and bodies they keep.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;

use crate::relay::media::TokenQuery;
use crate::relay::relay::RelayState;

/// The most marks kept in one day, everyone's: a bound on their memory.
pub const MARKS_PER_DAY: usize = 500_000;

/// What is counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Count {
    View,
    Download,
}

impl Count {
    fn word(self) -> &'static str {
        match self {
            Count::View => "view",
            Count::Download => "download",
        }
    }
}

/// Who is counted: a key (a member, through a ticket or the session's token), else a network
/// address (the `X-Real-IP` nginx sets; never `X-Forwarded-For`, which a client can write). A
/// request with neither counts as one shared unknown address: an undercount, never an overcount.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Who<'a> {
    Key(&'a str),
    Address(&'a str),
}

/// One day's marks: the day (days since 1970, UTC), its secret, and the marks.
struct Day {
    day: i64,
    secret: [u8; 32],
    seen: HashSet<[u8; 16]>,
    /// The day's "full" line was logged.
    full_said: bool,
}

/// The day's marks, in memory only (`RelayState::media_marks`).
pub struct Marks {
    today: std::sync::Mutex<Option<Day>>,
    cap: usize,
}

impl Default for Marks {
    fn default() -> Self {
        Marks::with_cap(MARKS_PER_DAY)
    }
}

impl std::fmt::Debug for Marks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Marks").field("cap", &self.cap).finish_non_exhaustive() // never the secret
    }
}

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

impl Marks {
    /// Marks that hold at most `cap` a day (a test sets a small one).
    pub fn with_cap(cap: usize) -> Self {
        Marks { today: std::sync::Mutex::new(None), cap: cap.max(1) }
    }

    /// Whether `who` is counted for `what` on item `item` at `now_ms` (Unix ms): true the first time
    /// today, false after that, and false for everyone new once the day holds its cap. A new UTC day
    /// forgets every mark and draws a new secret first.
    pub fn first_today(&self, what: Count, item: i64, who: Who, now_ms: i64) -> bool {
        let day = now_ms.div_euclid(DAY_MS);
        let mut today = self.today.lock().unwrap_or_else(|p| p.into_inner());
        if today.as_ref().is_none_or(|d| d.day != day) {
            let mut secret = [0u8; 32];
            if getrandom::getrandom(&mut secret).is_err() {
                // No secret, no mark: count nothing rather than keep marks anyone could work out.
                tracing::error!("Media counts: no random secret for the day; nothing is counted");
                return false;
            }
            *today = Some(Day { day, secret, seen: HashSet::new(), full_said: false });
        }
        let Some(d) = today.as_mut() else { return false };
        let mark = mark_of(&d.secret, what, item, who);
        if d.seen.contains(&mark) {
            return false;
        }
        if d.seen.len() >= self.cap {
            if !d.full_said {
                d.full_said = true;
                tracing::warn!("Media counts: {} marks today, the most a day; nothing more is counted until midnight UTC", self.cap);
            }
            return false;
        }
        d.seen.insert(mark)
    }

    /// How many marks today holds (tests).
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.today.lock().unwrap_or_else(|p| p.into_inner()).as_ref().map_or(0, |d| d.seen.len())
    }
}

/// The mark of `who` for `what` on `item` under the day's `secret`.
fn mark_of(secret: &[u8; 32], what: Count, item: i64, who: Who) -> [u8; 16] {
    let mut h = blake3::Hasher::new_keyed(secret);
    h.update(b"hum/media-count/v1\n");
    h.update(what.word().as_bytes());
    h.update(b"\n");
    h.update(item.to_string().as_bytes());
    h.update(b"\n");
    match who {
        Who::Key(k) => {
            h.update(b"m:");
            h.update(k.as_bytes());
        }
        Who::Address(a) => {
            h.update(b"a:");
            h.update(a.as_bytes());
        }
    }
    let mut out = [0u8; 16];
    out.copy_from_slice(&h.finalize().as_bytes()[..16]);
    out
}

/// What a route that waits on its lane answers.
pub const NOT_BUILT: &str = "Not built yet.";

pub(crate) fn not_built() -> Response {
    (StatusCode::NOT_IMPLEMENTED, NOT_BUILT).into_response()
}

/// `POST /api/media/viewed`'s body: the item's number. Unsigned; the session's upload token in
/// `X-Upload-Token` is needed only to count a members-only item. The answer is always
/// `{ok: true}`, so it never says whether an item exists.
#[derive(Debug, Default, Deserialize)]
pub struct ViewedBody {
    pub id: i64,
}

/// `POST /api/media/viewed` (lane 1B).
pub async fn viewed(State(_state): State<Arc<RelayState>>, _headers: HeaderMap, Json(_b): Json<ViewedBody>) -> Response {
    not_built()
}

/// `GET /api/media/download/{id}?file=<n>[&ticket=]` (lane 1B): file `n` of item `id` as an
/// attachment named by its original name, counted once a person a day.
pub async fn download(State(_state): State<Arc<RelayState>>, Path(_id): Path<i64>, Query(_q): Query<TokenQuery>, _req: axum::extract::Request) -> Response {
    not_built()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE DAY'S MARKS COUNT A PERSON ONCE AND START AGAIN AT MIDNIGHT: the first view of an item by
    /// an address counts and the second does not; another item, another kind of count, another
    /// address and a key (even one spelling the same text) each count once; the next UTC day counts
    /// the same person again; and past the day's cap nobody new is counted until midnight.
    ///
    /// Seen red 2026-10-12 with `first_today` keeping yesterday's marks (the day left out of the
    /// reset: `if today.is_none()`): "the next day counts the same person again".
    #[test]
    fn the_day_marks_count_a_person_once_and_start_again_at_midnight() {
        let marks = Marks::default();
        let noon = 20_000 * DAY_MS + DAY_MS / 2;
        let ann = Who::Address("203.0.113.7");
        assert!(marks.first_today(Count::View, 12, ann, noon), "the first view counts");
        assert!(!marks.first_today(Count::View, 12, ann, noon + 60_000), "the second that day does not");
        assert!(marks.first_today(Count::View, 13, ann, noon), "another item counts");
        assert!(marks.first_today(Count::Download, 12, ann, noon), "a download is counted apart from a view");
        assert!(marks.first_today(Count::View, 12, Who::Address("198.51.100.4"), noon), "another address counts");
        assert!(marks.first_today(Count::View, 12, Who::Key("203.0.113.7"), noon), "a key is never taken for an address");
        assert!(!marks.first_today(Count::View, 12, ann, 20_001 * DAY_MS - 1), "the last millisecond of the day is still that day");
        assert_eq!(marks.len(), 5);
        assert!(marks.first_today(Count::View, 12, ann, 20_001 * DAY_MS), "the next day counts the same person again");
        assert_eq!(marks.len(), 1, "yesterday's marks are forgotten");

        let small = Marks::with_cap(2);
        assert!(small.first_today(Count::View, 1, ann, noon));
        assert!(small.first_today(Count::View, 2, ann, noon));
        assert!(!small.first_today(Count::View, 3, ann, noon), "past the cap nobody new is counted");
        assert!(!small.first_today(Count::View, 1, ann, noon), "and those counted are not counted again");
        assert!(small.first_today(Count::View, 3, ann, noon + DAY_MS), "the cap is a day's");
    }
}
