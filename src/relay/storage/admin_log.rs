//! The admin log (2026-10-22, docs/design/in-app-ops.md): every change an admin makes through the
//! action registry (src/relay/admin/), so the admins of a server can see who changed what and
//! when, from inside the app, instead of reading the relay's journal over SSH.
//!
//! WHAT A ROW HOLDS. When (milliseconds), which action (its registry id), a one-line summary the
//! action wrote, and whether it worked. Never a password, token or web address: the alert-channel
//! actions log the channel's kind and label, never its address. Never a person's key or name:
//! an action on a member logs that it happened, not to whom (the banned list keeps that, and it
//! goes with the person's account).
//!
//! WHO DID IT. The admin is a keyed one-way fingerprint ([`admin_log_fingerprint`]) under the
//! erased-accounts secret with a domain of its own, never the key, the same house pattern as
//! `erased_accounts` and `friend_cert_revocations`. The log is shown with names by matching the
//! fingerprints of the people who hold the admin or moderator role NOW
//! ([`Storage::admin_log_names`]); a row whose admin no longer holds a role, or whose account
//! was erased, shows as a former admin, so the log keeps its accountability without keeping a
//! key the account erase would have to answer for.
//!
//! HOW LONG. The newest [`ADMIN_LOG_KEEP`] rows; older ones are deleted as new ones arrive.

use std::collections::HashMap;

use rusqlite::params;

use super::Storage;

/// How many rows the log keeps, newest first.
pub const ADMIN_LOG_KEEP: i64 = 2_000;

/// The longest summary kept, in characters (a summary is one line, written by the action).
pub const ADMIN_LOG_SUMMARY_MAX: usize = 300;

/// The domain the fingerprint is taken under, so it never equals the erased-accounts one for the
/// same key.
const ADMIN_LOG_FINGERPRINT_CONTEXT: &[u8] = b"hum/admin-log/v1\n";

/// One row of the admin log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminLogRow {
    pub id: i64,
    pub at_ms: i64,
    /// The keyed fingerprint of the admin who ran it.
    pub admin_fingerprint: String,
    /// The action's registry id, e.g. `alerts.add`.
    pub action: String,
    pub summary: String,
    pub ok: bool,
}

/// The fingerprint `key` is logged under on this relay: 64 hex characters.
pub fn admin_log_fingerprint(secret: &[u8; 32], key: &str) -> String {
    let mut h = blake3::Hasher::new_keyed(secret);
    h.update(ADMIN_LOG_FINGERPRINT_CONTEXT);
    h.update(key.trim().to_ascii_lowercase().as_bytes());
    h.finalize().to_hex().to_string()
}

/// The summary as it is kept: one line, at most [`ADMIN_LOG_SUMMARY_MAX`] characters.
pub fn kept_summary(summary: &str) -> String {
    let line: String = summary.split(['\n', '\r']).filter(|p| !p.trim().is_empty()).collect::<Vec<_>>().join(" ");
    line.chars().take(ADMIN_LOG_SUMMARY_MAX).collect()
}

impl Storage {
    /// The fingerprint `key` is logged under here.
    pub fn admin_log_fingerprint(&self, key: &str) -> String {
        admin_log_fingerprint(&self.erase_secret, key)
    }

    /// Add a row for `key` running `action`, then keep only the newest [`ADMIN_LOG_KEEP`].
    pub fn admin_log_append(&self, key: &str, action: &str, summary: &str, ok: bool) -> Result<(), rusqlite::Error> {
        let fp = self.admin_log_fingerprint(key);
        let at = super::now_millis() as i64;
        let summary = kept_summary(summary);
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO admin_log (at_ms, admin_fingerprint, action, summary, ok) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![at, fp, action, summary, ok as i32],
            )?;
            conn.execute(
                "DELETE FROM admin_log WHERE id NOT IN (SELECT id FROM admin_log ORDER BY id DESC LIMIT ?1)",
                params![ADMIN_LOG_KEEP],
            )?;
            Ok(())
        })
    }

    /// The newest `limit` rows, newest first.
    pub fn admin_log_recent(&self, limit: i64) -> Result<Vec<AdminLogRow>, rusqlite::Error> {
        self.with_read_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, at_ms, admin_fingerprint, action, summary, ok FROM admin_log ORDER BY id DESC LIMIT ?1",
            )?;
            let rows = stmt
                .query_map(params![limit.clamp(1, ADMIN_LOG_KEEP)], |r| {
                    Ok(AdminLogRow {
                        id: r.get(0)?,
                        at_ms: r.get(1)?,
                        admin_fingerprint: r.get(2)?,
                        action: r.get(3)?,
                        summary: r.get(4)?,
                        ok: r.get::<_, i64>(5)? != 0,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(rows)
        })
    }

    /// The names to show the log with: the fingerprint of everyone who holds the owner, admin or
    /// moderator role now, mapped to their registered name (their key's first characters when
    /// they have none). A fingerprint missing from this map belongs to someone who no longer
    /// holds a role here.
    pub fn admin_log_names(&self) -> HashMap<String, String> {
        let people: Vec<(String, Option<String>)> = self
            .with_read_conn(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT ur.public_key, (SELECT rn.name FROM registered_names rn WHERE rn.public_key = ur.public_key LIMIT 1)
                     FROM user_roles ur WHERE ur.role IN ('owner', 'admin', 'mod', 'moderator')",
                )?;
                let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.filter_map(|r| r.ok()).collect();
                Ok(rows)
            })
            .unwrap_or_default();
        people
            .into_iter()
            .map(|(key, name)| {
                let shown = name.unwrap_or_else(|| format!("{}...", key.chars().take(8).collect::<String>()));
                (self.admin_log_fingerprint(&key), shown)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The log never holds the admin's key, and a row is shown with the admin's name only while
    /// they hold a role. Seen red 2026-10-22 with `admin_log_append` writing `key` in place of
    /// `fp`: "the log never holds the admin's key".
    #[test]
    fn the_log_names_by_role_and_never_holds_the_key() {
        let db = Storage::open_temp("admin_log_names");
        let key = "ab".repeat(1952);
        db.register_name("Ada", &key).unwrap();
        db.set_role(&key, "admin").unwrap();
        db.admin_log_append(&key, "alerts.add", "Added ntfy channel \"my phone\".", true).unwrap();
        let rows = db.admin_log_recent(10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].action, "alerts.add");
        assert!(rows[0].ok);
        let all: String = db
            .with_conn(|c| c.query_row("SELECT admin_fingerprint || action || summary FROM admin_log", [], |r| r.get(0)))
            .unwrap();
        assert!(!all.contains(&key), "the log never holds the admin's key");
        assert_eq!(db.admin_log_names().get(&rows[0].admin_fingerprint).map(String::as_str), Some("Ada"));
        db.set_role(&key, "user").unwrap();
        assert!(db.admin_log_names().get(&rows[0].admin_fingerprint).is_none(), "a former admin is not named");
    }

    /// A summary is kept as one line and cut to its length; the log keeps only its newest rows.
    #[test]
    fn a_summary_is_one_line_and_the_log_is_trimmed() {
        assert_eq!(kept_summary("one\ntwo\r\n\nthree"), "one two three");
        assert_eq!(kept_summary(&"x".repeat(900)).chars().count(), ADMIN_LOG_SUMMARY_MAX);
        let db = Storage::open_temp("admin_log_trim");
        db.with_conn(|c| {
            for i in 0..(ADMIN_LOG_KEEP + 5) {
                c.execute(
                    "INSERT INTO admin_log (at_ms, admin_fingerprint, action, summary, ok) VALUES (?1, 'f', 'a', 's', 1)",
                    params![i],
                )?;
            }
            Ok::<_, rusqlite::Error>(())
        })
        .unwrap();
        db.admin_log_append("k", "log.test", "the newest", true).unwrap();
        let n: i64 = db.with_conn(|c| c.query_row("SELECT COUNT(*) FROM admin_log", [], |r| r.get(0))).unwrap();
        assert_eq!(n, ADMIN_LOG_KEEP);
        assert_eq!(db.admin_log_recent(1).unwrap()[0].summary, "the newest");
    }
}
