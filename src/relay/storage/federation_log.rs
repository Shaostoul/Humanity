//! Federation's stored half (2026-10-22, docs/design/federation-activation.md Phases 2 and 3):
//! the outbox this server sends its links from, where it is in each linked server's outbox, the
//! bans linked servers told it about, and the federated copies of messages their origin edits,
//! deletes or erases. The logic is handlers/federation_log.rs.
//!
//! THE OUTBOX (`federation_outbox`) is every frame this server sends its links, numbered: a chat
//! line, an edit, a delete, a ban, a profile copy, a signed object. A link that was down, or that
//! paused this server for sending too fast, asks for everything after the last number it applied
//! and gets exactly that, so nothing is lost and nothing arrives twice. It is bounded: at most
//! [`OUTBOX_ROWS_KEPT`] rows and [`OUTBOX_DAYS_KEPT`] days (less when the server keeps its own
//! messages for less). A deleted message's frames are SCRUBBED at once (their text replaced by
//! nothing, the number kept), so a link that catches up later is never sent words their author or
//! a moderator took back.
//!
//! THE CURSORS (`federation_cursors`) are this server's place in each linked server's outbox: the
//! outbox's own random id (so a linked server whose database was replaced starts again from
//! nothing rather than being ignored) and the last number applied.
//!
//! FEDERATED BANS (`federated_bans`) are the bans each linked server told this one about. While
//! that server is linked at tier 2 or above, a key it banned cannot post in this server's
//! federated rooms (the rooms are shared). This server's own rooms are untouched: each server's
//! moderation of its own rooms is its own.

use rusqlite::{params, OptionalExtension};

use super::Storage;

/// The most rows the outbox keeps.
pub const OUTBOX_ROWS_KEPT: i64 = 10_000;
/// The most days the outbox keeps a row (fewer when message expiry is shorter).
pub const OUTBOX_DAYS_KEPT: u64 = 7;
/// The `server_state` key of this server's outbox id.
const LOG_ID_KEY: &str = "federation_log_id";

/// One row of the outbox, as a link is sent it.
#[derive(Debug, Clone, PartialEq)]
pub struct OutboxRow {
    pub seq: i64,
    pub kind: String,
    /// The link this row is never sent to (the one it came from), or empty.
    pub not_to: String,
    /// The frame as sent; empty when scrubbed.
    pub frame: String,
}

/// A ban a linked server told this one about.
#[derive(Debug, Clone, PartialEq)]
pub struct FederatedBan {
    pub origin_key: String,
    pub public_key: String,
    pub name: String,
    pub banned_at: i64,
}

impl Storage {
    /// This server's outbox id: random, made once, kept in `server_state`.
    pub fn fed_log_id(&self) -> Result<String, rusqlite::Error> {
        if let Some(id) = self.get_state(LOG_ID_KEY)?.filter(|id| !id.is_empty()) {
            return Ok(id);
        }
        let id = hex::encode(rand::random::<[u8; 8]>());
        self.set_state(LOG_ID_KEY, &id)?;
        Ok(id)
    }

    /// Add a frame to the outbox; its number. `author` and `stamp` say whose message it carries
    /// and the message's own time (for scrubbing); `not_to` is the link it is never sent to.
    pub fn fed_outbox_append(&self, kind: &str, author: &str, stamp: u64, not_to: &str, frame: &str) -> Result<i64, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO federation_outbox (kind, author, stamp, not_to, frame, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![kind, author, stamp as i64, not_to, frame, super::now_millis() as i64],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// Up to `limit` rows after `after`, in order.
    pub fn fed_outbox_after(&self, after: i64, limit: usize) -> Result<Vec<OutboxRow>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT seq, kind, not_to, frame FROM federation_outbox WHERE seq > ?1 ORDER BY seq ASC LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![after, limit as i64], |r| {
                Ok(OutboxRow { seq: r.get(0)?, kind: r.get(1)?, not_to: r.get(2)?, frame: r.get(3)? })
            })?;
            rows.collect()
        })
    }

    /// The oldest and newest numbers the outbox holds, or (0, newest ever) when it holds none:
    /// numbers are never reused (AUTOINCREMENT), so the newest ever is known even when empty.
    pub fn fed_outbox_bounds(&self) -> Result<(i64, i64), rusqlite::Error> {
        self.with_conn(|conn| {
            let (oldest, newest): (Option<i64>, Option<i64>) =
                conn.query_row("SELECT MIN(seq), MAX(seq) FROM federation_outbox", [], |r| Ok((r.get(0)?, r.get(1)?)))?;
            let ever: Option<i64> = conn
                .query_row("SELECT seq FROM sqlite_sequence WHERE name = 'federation_outbox'", [], |r| r.get(0))
                .optional()?;
            Ok((oldest.unwrap_or(0), newest.or(ever).unwrap_or(0)))
        })
    }

    /// Drop the outbox rows past [`OUTBOX_ROWS_KEPT`] and older than `days` days. Returns how many.
    pub fn fed_outbox_prune(&self, days: u64) -> Result<usize, rusqlite::Error> {
        let cutoff = super::now_millis().saturating_sub(days.min(OUTBOX_DAYS_KEPT) * 86_400_000) as i64;
        self.with_conn(|conn| {
            let mut n = conn.execute("DELETE FROM federation_outbox WHERE created_at < ?1", params![cutoff])?;
            n += conn.execute(
                "DELETE FROM federation_outbox WHERE seq <= (SELECT MAX(seq) FROM federation_outbox) - ?1",
                params![OUTBOX_ROWS_KEPT],
            )?;
            Ok(n)
        })
    }

    /// Scrub the frames carrying `author`'s words: all of them, or those of the message stamped
    /// `stamp`. The rows and their numbers stay (a link is sent a skip for them). Returns how many.
    pub fn fed_outbox_scrub(&self, author: &str, stamp: Option<u64>) -> Result<usize, rusqlite::Error> {
        if author.is_empty() {
            return Ok(0);
        }
        self.with_conn(|conn| match stamp {
            Some(ts) => conn.execute(
                "UPDATE federation_outbox SET frame = '', kind = 'scrubbed' WHERE author = ?1 AND stamp = ?2 AND kind IN ('chat', 'edit')",
                params![author, ts as i64],
            ),
            None => conn.execute(
                "UPDATE federation_outbox SET frame = '', kind = 'scrubbed' WHERE author = ?1 AND kind IN ('chat', 'edit')",
                params![author],
            ),
        })
    }

    /// The outbox's chat frame for `author`'s message stamped `stamp`, with its number.
    pub fn fed_outbox_chat(&self, author: &str, stamp: u64) -> Result<Option<(i64, String)>, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT seq, frame FROM federation_outbox WHERE kind = 'chat' AND author = ?1 AND stamp = ?2 ORDER BY seq DESC LIMIT 1",
                params![author, stamp as i64],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
        })
    }

    /// Replace one outbox row's frame (an edited chat line, signed again).
    pub fn fed_outbox_set_frame(&self, seq: i64, frame: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| Ok(conn.execute("UPDATE federation_outbox SET frame = ?1 WHERE seq = ?2", params![frame, seq])? > 0))
    }

    /// This server's place in `peer`'s outbox: its id and the last number applied.
    pub fn fed_cursor(&self, peer: &str) -> Result<Option<(String, i64)>, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row("SELECT log_id, last_seq FROM federation_cursors WHERE peer_key = ?1", params![peer], |r| Ok((r.get(0)?, r.get(1)?)))
                .optional()
        })
    }

    /// Record this server's place in `peer`'s outbox.
    pub fn set_fed_cursor(&self, peer: &str, log_id: &str, last: i64) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO federation_cursors (peer_key, log_id, last_seq, updated_at) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(peer_key) DO UPDATE SET log_id = ?2, last_seq = ?3, updated_at = ?4",
                params![peer, log_id, last, super::now_millis() as i64],
            )?;
            Ok(())
        })
    }

    /// Keep a ban `origin` told this server about (a newer one replaces the older).
    pub fn fed_ban_put(&self, origin: &str, key: &str, name: &str, at: i64) -> Result<(), rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO federated_bans (origin_key, public_key, name, banned_at) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(origin_key, public_key) DO UPDATE SET name = ?3, banned_at = ?4",
                params![origin, key, name, at],
            )?;
            Ok(())
        })
    }

    /// Forget a ban `origin` lifted. True when there was one.
    pub fn fed_ban_remove(&self, origin: &str, key: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            Ok(conn.execute("DELETE FROM federated_bans WHERE origin_key = ?1 AND public_key = ?2", params![origin, key])? > 0)
        })
    }

    /// The bans `origin` told this server about, newest first.
    pub fn fed_bans_of(&self, origin: &str) -> Result<Vec<FederatedBan>, rusqlite::Error> {
        self.with_read_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT origin_key, public_key, name, banned_at FROM federated_bans WHERE origin_key = ?1 ORDER BY banned_at DESC",
            )?;
            let rows = stmt.query_map(params![origin], |r| {
                Ok(FederatedBan { origin_key: r.get(0)?, public_key: r.get(1)?, name: r.get(2)?, banned_at: r.get(3)? })
            })?;
            rows.collect()
        })
    }

    /// The name of a server linked at tier 2 or above that banned `key`, if one did: the key then
    /// cannot post in this server's federated rooms. A ban from a server no longer linked, or
    /// lowered below tier 2, no longer counts (removing a link lifts its bans here).
    pub fn fed_banned_by(&self, key: &str) -> Result<Option<String>, rusqlite::Error> {
        self.with_read_conn(|conn| {
            conn.query_row(
                "SELECT s.name FROM federated_bans b JOIN federated_servers s ON s.public_key = b.origin_key
                 WHERE b.public_key = ?1 AND s.trust_tier >= 2 ORDER BY b.banned_at DESC LIMIT 1",
                params![key],
                |r| r.get(0),
            )
            .optional()
        })
    }

    /// Whether this server holds `origin`'s copy of `author`'s message stamped `stamp`.
    pub fn fed_message_held(&self, origin: &str, author: &str, stamp: u64) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let n: i64 = conn.query_row(
                "SELECT COUNT(*) FROM messages WHERE msg_type = 'federated_chat' AND from_key = ?1 AND timestamp = ?2
                 AND json_extract(raw_json, '$.from_key') = ?3",
                params![origin, stamp as i64, author],
                |r| r.get(0),
            )?;
            Ok(n > 0)
        })
    }

    /// Apply `origin`'s edit to its copy of `author`'s message stamped `stamp`: the text, in the
    /// row and in the frame kept for history. The channel it is in, when there was one to edit.
    pub fn fed_message_edit(&self, origin: &str, author: &str, stamp: u64, new_content: &str) -> Result<Option<String>, rusqlite::Error> {
        self.with_conn(|conn| {
            let found: Option<(i64, String, Option<String>)> = conn
                .query_row(
                    "SELECT id, raw_json, channel_id FROM messages WHERE msg_type = 'federated_chat' AND from_key = ?1
                     AND timestamp = ?2 AND json_extract(raw_json, '$.from_key') = ?3 LIMIT 1",
                    params![origin, stamp as i64, author],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            let Some((id, raw, channel)) = found else { return Ok(None) };
            let mut val: serde_json::Value = serde_json::from_str(&raw).unwrap_or(serde_json::Value::Null);
            if val.is_object() {
                val["content"] = serde_json::Value::String(new_content.to_string());
            }
            let new_raw = if val.is_object() { val.to_string() } else { raw };
            conn.execute("UPDATE messages SET content = ?1, raw_json = ?2 WHERE id = ?3", params![new_content, new_raw, id])?;
            Ok(Some(channel.unwrap_or_else(|| "general".to_string())))
        })
    }

    /// Delete `origin`'s copy of `author`'s message stamped `stamp`. True when there was one.
    pub fn fed_message_delete(&self, origin: &str, author: &str, stamp: u64) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            Ok(conn.execute(
                "DELETE FROM messages WHERE msg_type = 'federated_chat' AND from_key = ?1 AND timestamp = ?2
                 AND json_extract(raw_json, '$.from_key') = ?3",
                params![origin, stamp as i64, author],
            )? > 0)
        })
    }

    /// Delete every copy of `author`'s messages `origin` sent (their account there was erased).
    /// The stamps of what went, so the apps can be told.
    pub fn fed_messages_erase(&self, origin: &str, author: &str) -> Result<Vec<u64>, rusqlite::Error> {
        self.with_conn(|conn| {
            let stamps: Vec<u64> = {
                let mut stmt = conn.prepare(
                    "SELECT timestamp FROM messages WHERE msg_type = 'federated_chat' AND from_key = ?1
                     AND json_extract(raw_json, '$.from_key') = ?2",
                )?;
                let rows = stmt.query_map(params![origin, author], |r| r.get::<_, i64>(0))?;
                rows.filter_map(|r| r.ok()).map(|t| t as u64).collect()
            };
            conn.execute(
                "DELETE FROM messages WHERE msg_type = 'federated_chat' AND from_key = ?1 AND json_extract(raw_json, '$.from_key') = ?2",
                params![origin, author],
            )?;
            Ok(stamps)
        })
    }

    /// Whether `key` has a message here in a federated room (asked before an erase, so the linked
    /// servers are told to remove their copies only when there are any).
    pub fn posted_in_federated_rooms(&self, key: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM messages m JOIN channels c ON c.id = COALESCE(m.channel_id, 'general')
                 WHERE m.from_key = ?1 AND m.msg_type = 'chat' AND c.federated = 1)",
                params![key],
                |r| r.get::<_, bool>(0),
            )
        })
    }

    /// The channel of this server's own message from `from_key` stamped `stamp`, if it holds one.
    pub fn message_channel(&self, from_key: &str, stamp: u64) -> Result<Option<String>, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT COALESCE(channel_id, 'general') FROM messages WHERE from_key = ?1 AND timestamp = ?2 AND msg_type = 'chat' LIMIT 1",
                params![from_key, stamp as i64],
                |r| r.get(0),
            )
            .optional()
        })
    }

    /// The channel and author of this server's own message with row id `id`, if it holds one.
    pub fn message_by_id(&self, id: i64) -> Result<Option<(String, String, u64)>, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT from_key, COALESCE(channel_id, 'general'), timestamp FROM messages WHERE id = ?1 AND msg_type = 'chat'",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? as u64)),
            )
            .optional()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE OUTBOX KEEPS ITS NUMBERS: rows come back after a number in order; pruned and emptied,
    /// the next number still follows the last ever given (so a link's place stays meaningful);
    /// a scrubbed row keeps its number and loses its words.
    #[test]
    fn the_outbox_keeps_its_numbers_and_scrubs_words() {
        let db = Storage::open_temp("fed_outbox");
        let a = db.fed_outbox_append("chat", "ann", 100, "", "{\"words\":\"hello\"}").unwrap();
        let b = db.fed_outbox_append("edit", "ann", 100, "", "{\"words\":\"hello again\"}").unwrap();
        let c = db.fed_outbox_append("profile", "", 0, "peer-b", "{}").unwrap();
        assert_eq!((b, c), (a + 1, a + 2));
        let rows = db.fed_outbox_after(a, 10).unwrap();
        assert_eq!(rows.iter().map(|r| r.seq).collect::<Vec<_>>(), vec![b, c]);
        assert_eq!(rows[1].not_to, "peer-b");
        assert_eq!(db.fed_outbox_scrub("ann", Some(100)).unwrap(), 2);
        let rows = db.fed_outbox_after(0, 10).unwrap();
        assert_eq!((rows[0].kind.as_str(), rows[0].frame.as_str()), ("scrubbed", ""));
        assert_eq!(db.fed_outbox_bounds().unwrap(), (a, c));
        db.with_conn(|conn| conn.execute("DELETE FROM federation_outbox", [])).unwrap();
        assert_eq!(db.fed_outbox_bounds().unwrap(), (0, c), "the newest number ever given survives an empty outbox");
        assert_eq!(db.fed_outbox_append("chat", "", 0, "", "{}").unwrap(), c + 1, "and the next one follows it");
        let id = db.fed_log_id().unwrap();
        assert_eq!(id.len(), 16);
        assert_eq!(db.fed_log_id().unwrap(), id, "made once");
    }

    /// A LINKED SERVER'S BAN COUNTS WHILE IT IS LINKED AT TIER 2: kept, it names the server; at
    /// tier 1, or removed, it no longer counts; lifted, it is gone.
    #[test]
    fn a_linked_servers_ban_counts_while_it_is_linked() {
        let db = Storage::open_temp("fed_bans");
        db.add_federated_server("https://a.example", "Server A", "https://a.example").unwrap();
        db.update_federated_server_info("https://a.example", "Server A", Some("key-a"), false).unwrap();
        db.fed_ban_put("key-a", "mallory", "Mallory", 1).unwrap();
        assert_eq!(db.fed_banned_by("mallory").unwrap(), None, "tier 0: not linked");
        db.set_server_trust_tier("https://a.example", 2).unwrap();
        assert_eq!(db.fed_banned_by("mallory").unwrap().as_deref(), Some("Server A"));
        assert_eq!(db.fed_bans_of("key-a").unwrap().len(), 1);
        db.set_server_trust_tier("https://a.example", 1).unwrap();
        assert_eq!(db.fed_banned_by("mallory").unwrap(), None, "lowered below tier 2");
        db.set_server_trust_tier("https://a.example", 2).unwrap();
        assert!(db.fed_ban_remove("key-a", "mallory").unwrap());
        assert_eq!(db.fed_banned_by("mallory").unwrap(), None, "lifted");
    }
}
