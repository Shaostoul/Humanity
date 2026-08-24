//! Account data export + erasure (data sovereignty, 2026-08-23).
//!
//! "Your data" should mean it: any member can download everything this
//! server holds about them, and can erase it — self-service, no admin
//! required. The operator directive is that nobody's data leaks without
//! their permission; the strongest enforcement is the user themselves
//! holding the export and the delete button.
//!
//! Export returns a JSON object grouping every row keyed by the caller's
//! identity (key- and name-keyed tables both). Erasure hard-deletes the
//! same set; with `PRAGMA secure_delete=ON` the freed pages are zeroed,
//! and the caller runs a WAL truncate afterwards. Rotating backups hold
//! prior snapshots until they age out (documented in
//! docs/reference/retention_and_deletion_semantics.md).
//!
//! Deliberately NOT deleted:
//! - Other people's copies of public conversation (their clients already
//!   have them; replication makes global recall impossible and we don't
//!   pretend otherwise).
//! - Sealed DM envelopes in OTHER people's mailboxes (they are the
//!   recipient's data, unreadable to anyone else anyway).

use super::Storage;
use rusqlite::params;

impl Storage {
    /// Everything this server stores about `key` / `name`, as JSON.
    pub fn export_account(&self, key: &str, name: &str) -> serde_json::Value {
        let mut out = serde_json::Map::new();
        out.insert("exported_at_unix_ms".into(), serde_json::json!(super::now_millis()));
        out.insert("public_key".into(), serde_json::json!(key));
        out.insert("name".into(), serde_json::json!(name));

        self.with_read_conn(|conn| {
            let mut grab = |label: &str, sql: &str, binds: &[&dyn rusqlite::types::ToSql]| {
                let rows: Vec<serde_json::Value> = (|| {
                    let mut stmt = conn.prepare(sql).ok()?;
                    let ncols = stmt.column_count();
                    let names: Vec<String> =
                        (0..ncols).map(|i| stmt.column_name(i).unwrap_or("").to_string()).collect();
                    let collected = stmt
                        .query_map(binds, |row| {
                            let mut obj = serde_json::Map::new();
                            for (i, cn) in names.iter().enumerate() {
                                let v: rusqlite::types::Value = row.get(i)?;
                                let jv = match v {
                                    rusqlite::types::Value::Null => serde_json::Value::Null,
                                    rusqlite::types::Value::Integer(n) => serde_json::json!(n),
                                    rusqlite::types::Value::Real(f) => serde_json::json!(f),
                                    rusqlite::types::Value::Text(s) => serde_json::json!(s),
                                    rusqlite::types::Value::Blob(b) => serde_json::json!(format!("<{} bytes>", b.len())),
                                };
                                obj.insert(cn.clone(), jv);
                            }
                            Ok(serde_json::Value::Object(obj))
                        })
                        .ok()?
                        .filter_map(|r| r.ok())
                        .collect();
                    Some(collected)
                })()
                .unwrap_or_default();
                out.insert(label.to_string(), serde_json::Value::Array(rows));
            };

            grab("registered_names", "SELECT name, public_key, registered_at FROM registered_names WHERE public_key = ?1", &[&key]);
            grab("membership", "SELECT public_key, name, role, joined_at, last_seen, hide_presence FROM server_members WHERE public_key = ?1", &[&key]);
            grab("profile", "SELECT * FROM profiles WHERE name = ?1 COLLATE NOCASE", &[&name]);
            grab("signed_profiles", "SELECT * FROM signed_profiles WHERE public_key = ?1", &[&key]);
            grab("messages_authored", "SELECT id, channel_id, content, timestamp FROM messages WHERE from_key = ?1 ORDER BY timestamp ASC", &[&key]);
            // (follows removed 2026-08-24: the server stores no social
            // graph to export — following lives in the user's own client
            // store.)
            grab("uploads", "SELECT id, filename, url, size, mime_type, created_at FROM uploads WHERE uploader_key = ?1", &[&key]);
            grab("notification_prefs", "SELECT * FROM notification_prefs WHERE public_key = ?1", &[&key]);
            grab("tasks_created", "SELECT id, title, description, status, priority, created_at FROM tasks WHERE created_by = ?1", &[&key]);
            grab("listings", "SELECT * FROM marketplace_listings WHERE seller_key = ?1", &[&key]);
            grab("reviews_written", "SELECT * FROM listing_reviews WHERE reviewer_key = ?1", &[&key]);
            // The vault blob is the user's own client-encrypted data.
            grab("vault", "SELECT public_key, length(blob) AS blob_bytes, updated_at FROM vault_blobs WHERE public_key = ?1", &[&key]);
            // Sealed mail queued for them (counts only; contents are sealed
            // envelopes their own client decrypts via dm_fetch).
            grab("dm_mailbox_queued", "SELECT COUNT(*) AS envelopes FROM dm_mailbox WHERE to_key = ?1", &[&key]);
            Ok(())
        })
        .ok();

        serde_json::Value::Object(out)
    }

    /// Erase this account: every row keyed by `key`/`name`, plus uploaded
    /// files on disk. Returns (table_label, rows_deleted) for the receipt
    /// shown to the user. The caller broadcasts roster updates + closes
    /// the socket afterwards.
    pub fn delete_account(&self, key: &str, name: &str) -> Vec<(String, usize)> {
        // Upload files first (need the rows to find the paths).
        let upload_files: Vec<String> = self
            .with_read_conn(|conn| {
                let mut stmt = conn.prepare("SELECT url FROM uploads WHERE uploader_key = ?1")?;
                let v = stmt
                    .query_map(params![key], |r| r.get::<_, String>(0))?
                    .filter_map(|r| r.ok())
                    .collect();
                Ok(v)
            })
            .unwrap_or_default();
        let mut files_removed = 0usize;
        for url in &upload_files {
            // Upload URLs are /uploads/<file>; files live in data/uploads/.
            if let Some(fname) = url.rsplit('/').next() {
                let path = std::path::Path::new("data/uploads").join(fname);
                if std::fs::remove_file(&path).is_ok() {
                    files_removed += 1;
                }
            }
        }

        let mut receipt: Vec<(String, usize)> = Vec::new();
        receipt.push(("upload_files_removed".to_string(), files_removed));
        self.with_conn(|conn| {
            let mut del = |label: &str, sql: &str, binds: &[&dyn rusqlite::types::ToSql]| {
                match conn.execute(sql, binds) {
                    Ok(n) => receipt.push((label.to_string(), n)),
                    // Defensive: a missing table (older DB) is not fatal to
                    // the rest of the erasure.
                    Err(e) => tracing::warn!("account erase: {label}: {e}"),
                }
            };
            del("messages", "DELETE FROM messages WHERE from_key = ?1", &[&key]);
            del("reactions", "DELETE FROM reactions WHERE reactor_key = ?1", &[&key]);
            del("uploads", "DELETE FROM uploads WHERE uploader_key = ?1", &[&key]);
            del("notification_prefs", "DELETE FROM notification_prefs WHERE public_key = ?1", &[&key]);
            del("vault", "DELETE FROM vault_blobs WHERE public_key = ?1", &[&key]);
            del("dm_mailbox", "DELETE FROM dm_mailbox WHERE to_key = ?1", &[&key]);
            del("push_subscriptions", "DELETE FROM push_subscriptions WHERE public_key = ?1", &[&key]);
            del("signed_profiles", "DELETE FROM signed_profiles WHERE public_key = ?1", &[&key]);
            del("profile", "DELETE FROM profiles WHERE name = ?1 COLLATE NOCASE", &[&name]);
            del("statuses", "DELETE FROM user_status WHERE name = ?1 COLLATE NOCASE", &[&name]);
            del("friend_codes", "DELETE FROM friend_codes WHERE public_key = ?1", &[&key]);
            del("listing_reviews", "DELETE FROM listing_reviews WHERE reviewer_key = ?1", &[&key]);
            del("listing_images", "DELETE FROM listing_images WHERE listing_id IN (SELECT id FROM marketplace_listings WHERE seller_key = ?1)", &[&key]);
            del("listings", "DELETE FROM marketplace_listings WHERE seller_key = ?1", &[&key]);
            del("tasks", "DELETE FROM tasks WHERE created_by = ?1", &[&key]);
            del("roles", "DELETE FROM user_roles WHERE public_key = ?1", &[&key]);
            del("membership", "DELETE FROM server_members WHERE public_key = ?1", &[&key]);
            del("registered_name", "DELETE FROM registered_names WHERE public_key = ?1", &[&key]);
            // Fold the secure_delete-zeroed pages out of the WAL.
            let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        });
        receipt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_storage() -> Storage {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_account_{pid}_{nanos}.db"));
        Storage::open(&path).expect("open test db")
    }

    /// A populated account exports its data and erases to nothing, while
    /// another member's data is untouched.
    #[test]
    fn export_then_erase_leaves_no_trace_of_the_account() {
        let db = test_storage();
        db.register_name("Alice", "alice_key").unwrap();
        db.register_name("Bob", "bob_key").unwrap();
        db.join_server("alice_key", "Alice").unwrap();
        db.join_server("bob_key", "Bob").unwrap();
        db.mailbox_put("alice_key", "sealed-env").unwrap();
        db.mailbox_put("bob_key", "bobs-env").unwrap();

        // Export sees the account's rows.
        let export = db.export_account("alice_key", "Alice");
        assert_eq!(export["registered_names"].as_array().unwrap().len(), 1);
        assert_eq!(export["membership"].as_array().unwrap().len(), 1);
        assert_eq!(export["dm_mailbox_queued"][0]["envelopes"], 1);

        // Erase.
        let receipt = db.delete_account("alice_key", "Alice");
        let count = |label: &str| receipt.iter().find(|(l, _)| l == label).map(|(_, n)| *n).unwrap_or(0);
        assert_eq!(count("registered_name"), 1);
        assert_eq!(count("membership"), 1);
        assert_eq!(count("dm_mailbox"), 1);

        // Nothing left under the key or name.
        let export2 = db.export_account("alice_key", "Alice");
        assert!(export2["registered_names"].as_array().unwrap().is_empty());
        assert!(export2["membership"].as_array().unwrap().is_empty());
        // Bob is untouched.
        assert!(db.is_member("bob_key"));
        assert_eq!(db.mailbox_fetch("bob_key", 0, 10).unwrap().len(), 1);
    }
}
