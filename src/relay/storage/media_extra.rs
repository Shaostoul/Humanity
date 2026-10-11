//! The Media page's storage, the increment 2 follow-ups (2026-10-11; docs/design/media-showcase.md,
//! "Increment 2 follow-ups, exact" and "The operator's answers (2026-10-11)"). A child of
//! storage/media.rs (`#[path]`), so it reads that file's private helpers; the routes are in
//! relay/media.rs.
//!
//! - [`Storage::set_media_kept`]: an admin or a moderator keeps an item on the server (A7).
//! - [`Storage::picked_among`]: which of some items a person picked (A3, `picked_by_me`).
//! - [`Storage::media_waiting_of`]: a person's Media uploads not yet published (A4).

use super::MEDIA_SHELF;
use crate::relay::storage::Storage;
use rusqlite::{params, OptionalExtension};

/// One of a person's own Media uploads waiting to be published, as `GET /api/media/mine` gives it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct MyWaitingUpload {
    pub filename: String,
    /// The name the file had on the person's device.
    pub original_name: String,
    pub size_bytes: i64,
    /// Unix ms.
    pub uploaded_at: i64,
}

impl Storage {
    /// Keep item `id` on the server (`by` an admin or moderator's key) or stop keeping it (`None`).
    /// A kept item counts against the Media budget but not against its uploader's share, and is
    /// left out of the prune lists (A7). Stopping puts it back in the uploader's share even past
    /// it: nothing is deleted for that, the share only stops new uploads. Refused: no such item.
    pub fn set_media_kept(&self, id: i64, by: Option<&str>, now_ms: i64) -> Result<(), super::MediaError> {
        self.with_conn(|conn| {
            let n = match by {
                Some(key) => conn.execute("UPDATE media_items SET kept_by = ?1, kept_at = ?2 WHERE id = ?3", params![key, now_ms, id])?,
                None => conn.execute("UPDATE media_items SET kept_by = NULL, kept_at = NULL WHERE id = ?1", params![id])?,
            };
            if n == 0 {
                return Err(super::MediaError::Refused(super::Refusal::NotFound, "There is no Media item with that number.".into()));
            }
            Ok(())
        })
    }

    /// Which of `ids` `picker` picked (A3: every item of a member's list carries `picked_by_me`).
    pub fn picked_among(&self, picker: &str, ids: &[i64]) -> Result<std::collections::HashSet<i64>, rusqlite::Error> {
        if ids.is_empty() {
            return Ok(Default::default());
        }
        self.with_read_conn(|conn| {
            let mut stmt = conn.prepare("SELECT 1 FROM media_picks WHERE picker_key = ?1 AND item_id = ?2")?;
            let mut out = std::collections::HashSet::new();
            for id in ids {
                if stmt.query_row(params![picker, id], |_| Ok(())).optional()?.is_some() {
                    out.insert(*id);
                }
            }
            Ok(out)
        })
    }

    /// `key`'s Media uploads not yet published, oldest first (A4), with the names they had on the
    /// person's device.
    pub fn media_waiting_of(&self, key: &str) -> Result<Vec<MyWaitingUpload>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT filename, original_name, size_bytes, uploaded_at FROM user_uploads
                 WHERE public_key = ?1 AND shared = ?2 AND filename NOT IN (SELECT upload_filename FROM media_items)
                 ORDER BY uploaded_at ASC, id ASC LIMIT 500",
            )?;
            let rows = stmt
                .query_map(params![key, MEDIA_SHELF], |r| {
                    Ok(MyWaitingUpload { filename: r.get(0)?, original_name: r.get(1)?, size_bytes: r.get(2)?, uploaded_at: r.get(3)? })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }
}
