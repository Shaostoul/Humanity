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

            grab("registered_names", "SELECT name, public_key, kyber_public, registered_at FROM registered_names WHERE public_key = ?1", &[&key]);
            grab("membership", "SELECT public_key, name, role, joined_at, last_seen, hide_presence FROM server_members WHERE public_key = ?1", &[&key]);
            grab("profile", "SELECT * FROM profiles WHERE name = ?1 COLLATE NOCASE", &[&name]);
            grab("signed_profiles", "SELECT * FROM signed_profiles WHERE public_key = ?1", &[&key]);
            grab("messages_authored", "SELECT id, channel_id, content, timestamp FROM messages WHERE from_key = ?1 ORDER BY timestamp ASC", &[&key]);
            // (follows removed 2026-08-24: the server stores no social
            // graph to export — following lives in the user's own client
            // store.)
            // `user_uploads`, NOT `uploads`. There is no table called `uploads`
            // and there never was: this query named one, `grab` swallowed the
            // prepare error into an empty array, and the export has been telling
            // every user "uploads": [] as though they had none. Real columns
            // per the schema: public_key, filename, uploaded_at, shared,
            // original_name, size_bytes.
            grab("uploads", "SELECT id, filename, original_name, size_bytes, shared, uploaded_at FROM user_uploads WHERE public_key = ?1", &[&key]);
            grab("notification_prefs", "SELECT * FROM notification_prefs WHERE public_key = ?1", &[&key]);
            // `project_tasks`, NOT `tasks`. Same phantom-table bug as uploads above.
            grab("tasks_created", "SELECT id, title, description, status, priority, created_by FROM project_tasks WHERE created_by = ?1", &[&key]);
            grab("listings", "SELECT * FROM marketplace_listings WHERE seller_key = ?1", &[&key]);
            grab("reviews_written", "SELECT * FROM listing_reviews WHERE reviewer_key = ?1", &[&key]);
            // The vault blob is the user's own client-encrypted data.
            grab("vault", "SELECT public_key, length(blob) AS blob_bytes, updated_at FROM vault_blobs WHERE public_key = ?1", &[&key]);
            // Sealed mail queued for them (counts only; contents are sealed
            // envelopes their own client decrypts via dm_fetch).
            grab("dm_mailbox_queued", "SELECT COUNT(*) AS envelopes FROM dm_mailbox WHERE to_key = ?1", &[&key]);

            // ── Things erase deletes but export never offered ──
            // These six were in delete_account() with no matching grab here.
            // That asymmetry is the signature of two hand-maintained lists, and
            // it means the server was erasing data it had never let the user
            // download. tests/account_sql_lint.rs now fails the build if a table
            // is deleted without being exported.
            grab("role", "SELECT public_key, role FROM user_roles WHERE public_key = ?1", &[&key]);
            grab("status", "SELECT name, status, status_text FROM user_status WHERE name = ?1 COLLATE NOCASE", &[&name]);
            grab("friend_codes", "SELECT code, public_key, created_at, expires_at, uses_remaining FROM friend_codes WHERE public_key = ?1", &[&key]);
            grab("push_subscriptions", "SELECT id, endpoint, p256dh, auth, created_at FROM push_subscriptions WHERE public_key = ?1", &[&key]);
            grab("reactions", "SELECT id, target_from, target_timestamp, emoji, channel, created_at FROM reactions WHERE reactor_key = ?1", &[&key]);
            grab("listing_images", "SELECT i.id, i.listing_id, i.url, i.position, i.created_at FROM listing_images i JOIN marketplace_listings l ON l.id = i.listing_id WHERE l.seller_key = ?1", &[&key]);

            // ── Moderation and reputation: visible to you, never deletable by you ──
            // A sanction whose existence is hidden from the person it constrains
            // is the opaque moderation the Accord objects to, so these ARE
            // exported. None of them is ever deleted: a record that exists to
            // constrain someone cannot be erasable by that someone, or account
            // deletion becomes a way out of a ban. web/pages/rules.html says so
            // in the same words.
            //
            // reporter_key and reputation_events.source_key are deliberately NOT
            // selected in the about-me direction. Handing someone the identity of
            // whoever reported or penalised them is a retaliation vector, and it
            // is the reporter's data, not theirs. Do not "simplify" these to
            // SELECT * later; the omission is the point.
            //
            // Reports ABOUT you are also deliberately absent. Their only handle is
            // reported_name, free text typed by a reporter, and a display name is
            // released on ban, kick and account deletion. Registering a departed
            // member's name would otherwise return every accusation ever filed
            // against them, to a stranger. That needs a reported_key column first.
            grab("reports_filed", "SELECT id, reported_name, reason, created_at FROM reports WHERE reporter_key = ?1", &[&key]);
            grab("chat_ban", "SELECT public_key, name, banned_at FROM banned_keys WHERE public_key = ?1", &[&key]);
            grab("chat_mute", "SELECT public_key, name, muted_at FROM muted_members WHERE public_key = ?1", &[&key]);
            grab("reputation", "SELECT public_key, score, level, updated_at FROM reputation WHERE public_key = ?1", &[&key]);
            grab("reputation_events_about_me", "SELECT id, event_type, points, reason, created_at FROM reputation_events WHERE public_key = ?1 ORDER BY created_at ASC", &[&key]);
            grab("bug_reports_filed", "SELECT id, title, severity, category, status, votes, created_at FROM bug_reports WHERE reporter_key = ?1 ORDER BY id ASC", &[&key]);
            grab("bug_votes_cast", "SELECT bug_id, voted_at FROM bug_votes WHERE voter_key = ?1", &[&key]);
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
                // This named a table that does not exist, so with_read_conn
                // returned Err, unwrap_or_default() turned it into an empty
                // list, and NO uploaded file was ever removed from disk by
                // "erase everything". files_removed was structurally always 0.
                // user_uploads stores the on-disk filename directly, not a URL.
                let mut stmt = conn.prepare("SELECT filename FROM user_uploads WHERE public_key = ?1")?;
                let v = stmt
                    .query_map(params![key], |r| r.get::<_, String>(0))?
                    .filter_map(|r| r.ok())
                    .collect();
                Ok(v)
            })
            .unwrap_or_default();
        let mut files_removed = 0usize;
        for url in &upload_files {
            // user_uploads.filename is the on-disk name; files live in
            // data/uploads/. The rsplit is kept so a stored value that happens
            // to carry a path prefix still resolves to its basename.
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
                    Err(e) => {
                        // A failed statement stays non-fatal (one broken table
                        // must not abandon the rest of an erasure), but it no
                        // longer stays SILENT. Previously this only warned to
                        // the log, and the caller's receipt filters out zero
                        // counts, so "the statement errored", "you had no rows"
                        // and "we never ran it" were indistinguishable to the
                        // user, who was told their data was erased either way.
                        // That is how two DELETEs against tables that do not
                        // exist survived in here unnoticed.
                        tracing::error!("account erase FAILED for {label}: {e}");
                        receipt.push((format!("{label}_FAILED"), 1));
                    }
                }
            };
            del("messages", "DELETE FROM messages WHERE from_key = ?1", &[&key]);
            del("reactions", "DELETE FROM reactions WHERE reactor_key = ?1", &[&key]);
            del("uploads", "DELETE FROM user_uploads WHERE public_key = ?1", &[&key]);
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
            del("tasks", "DELETE FROM project_tasks WHERE created_by = ?1", &[&key]);
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

    /// The load-bearing safety property of this whole feature.
    ///
    /// A ban and a mute MUST be visible to the person they constrain, because a
    /// sanction hidden from its subject is the opaque moderation the Accord
    /// objects to and web/pages/rules.html promises against. And they MUST
    /// survive erasure, because a record that exists to hold someone to account
    /// cannot be erasable by that person: otherwise "delete my account" is a
    /// self-service unban, and register_name is a bare INSERT OR IGNORE with no
    /// ban check, so the same key would simply walk back in.
    ///
    /// Both halves are asserted here so neither can be lost to a later tidy-up.
    #[test]
    fn sanctions_are_visible_in_the_export_and_survive_erasure() {
        let db = test_storage();
        db.register_name("Rowan", "rowan_key").unwrap();
        db.ban_user("rowan_key", "Rowan").unwrap();
        db.mute_user("rowan_key", "Rowan").unwrap();

        // Visible before erasing, so the decision is informed.
        let export = db.export_account("rowan_key", "Rowan");
        assert_eq!(
            export["chat_ban"].as_array().expect("chat_ban array").len(),
            1,
            "a banned member must be able to see their own ban"
        );
        assert_eq!(
            export["chat_mute"].as_array().expect("chat_mute array").len(),
            1,
            "and their own mute"
        );

        // Erase, then confirm the sanctions are STILL there.
        db.delete_account("rowan_key", "Rowan");
        assert!(
            db.is_banned("rowan_key").unwrap_or(false),
            "erasing the account must NOT lift the ban: that would make account \
             deletion a self-service unban"
        );
        assert!(
            db.is_muted("rowan_key").unwrap_or(false),
            "nor the mute, which is the one a muted member can still reach, since \
             mute leaves the socket working"
        );
    }

    /// Uploads and tasks specifically, because those two were the ones that
    /// silently did nothing. The queries named `uploads` and `tasks`, neither of
    /// which is a table in this schema (the real names are `user_uploads` and
    /// `project_tasks`), and both helpers swallow a bad table name: `grab`
    /// returns an empty array and `del` only warned. So the export told the user
    /// they had no uploads, the erase receipt never mentioned them, and every
    /// uploaded row and every file on disk stayed exactly where it was.
    ///
    /// The test below passed throughout, because it never touched either table.
    /// That is the shape of the bug: a green suite over the half that worked.
    #[test]
    fn uploads_and_tasks_are_actually_exported_and_actually_erased() {
        let db = test_storage();
        db.register_name("Ann", "ann_key").unwrap();
        db.register_name("Bea", "bea_key").unwrap();
        db.record_upload("ann_key", "ann-photo.png", 100, false, "photo.png", 4096).unwrap();
        db.record_upload("bea_key", "bea-photo.png", 100, false, "photo.png", 2048).unwrap();
        db.create_task("Ann task", "", "backlog", "medium", None, "ann_key", "").unwrap();
        db.create_task("Bea task", "", "backlog", "medium", None, "bea_key", "").unwrap();

        // Export must SEE them. Before the fix both arrays came back empty,
        // which reads to a user as "you have none", not "we did not look".
        let export = db.export_account("ann_key", "Ann");
        let uploads = export["uploads"].as_array().expect("uploads array");
        assert_eq!(uploads.len(), 1, "the export must contain the upload");
        assert_eq!(uploads[0]["filename"], "ann-photo.png");
        assert_eq!(export["tasks_created"].as_array().unwrap().len(), 1);

        // Erase must actually delete them, and say so in the receipt.
        let receipt = db.delete_account("ann_key", "Ann");
        let count = |label: &str| {
            receipt.iter().find(|(l, _)| l == label).map(|(_, n)| *n).unwrap_or(0)
        };
        assert_eq!(count("uploads"), 1, "the upload row must be deleted");
        assert_eq!(count("tasks"), 1, "the task row must be deleted");
        assert!(
            !receipt.iter().any(|(l, _)| l.ends_with("_FAILED")),
            "no statement may fail: {receipt:?}"
        );

        // And they are really gone, not merely reported gone.
        let after = db.export_account("ann_key", "Ann");
        assert!(after["uploads"].as_array().unwrap().is_empty());
        assert!(after["tasks_created"].as_array().unwrap().is_empty());

        // The other member is untouched.
        let bea = db.export_account("bea_key", "Bea");
        assert_eq!(bea["uploads"].as_array().unwrap().len(), 1, "Bea keeps her upload");
        assert_eq!(bea["tasks_created"].as_array().unwrap().len(), 1, "Bea keeps her task");
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
