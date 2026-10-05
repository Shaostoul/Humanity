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
        // The plot their home stands on, on each ship of this server: held under their DID,
        // not their key (storage/plots.rs `plot_owner_id`).
        let plot_owner = super::plot_owner_id(key);
        // The fingerprint an earlier erase of this key is remembered under, if one is
        // (storage/erased_accounts.rs, BUG-135).
        let erased_fingerprint = self.erased_account_fingerprint(key);

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
            // The plot their home stands on (ship homes 1b; the third review: it was neither
            // exported nor erased).
            grab("ship_plots", "SELECT world_id, plot_id, assigned_at FROM game_plots WHERE owner_did = ?1", &[&plot_owner]);
            // Their progress in the shared world (their quest, completed quests, XP and
            // reputation there), kept so a returning player resumes (ship homes 1b, round 4 of
            // the review: it was neither exported nor erased).
            grab("game_progress", "SELECT current_quest, completed_quests, xp, reputation, updated_at FROM player_progress WHERE public_key = ?1", &[&key]);
            // Their fleet ledger: what they used from the fleet and gave it (2026-10-04,
            // storage/fleet_ledger.rs), oldest first. Without the row number: it counts every
            // player's lines, so the gaps in one player's numbers would say how much everyone
            // else did in between (the review's finding 8).
            grab("fleet_ledger", "SELECT kind, direction, item_id, quantity, value, game_time, real_day, give_id, home, adjusted FROM fleet_ledger WHERE public_key = ?1 ORDER BY id ASC", &[&key]);
            // That this key erased its account here earlier, while this server still
            // remembers it (BUG-135, 2026-10-04): only the day and the window it is kept for,
            // under a one-way fingerprint of the key. It is listed because it is held about
            // this key. The
            // erase below never deletes it, because the erase is what writes it
            // (handlers/sign_ups.rs `remember_erase`); it goes when the person signs up here
            // again, or when the server's window or cap culls it. So the lint's "everything
            // erased is exported" rule is met in the direction it checks: export is wider.
            grab("erased_here", "SELECT erased_day, ttl_days FROM erased_accounts WHERE fingerprint = ?1", &[&erased_fingerprint]);

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
    /// shown to the user, a failed or deliberately kept table as
    /// `<label>_FAILED`. The registration is kept when a delete by name
    /// fails, and the listings when their images fail, so nothing the erase
    /// leaves is cut off from the key (`erase_left_rows`). The caller
    /// broadcasts roster updates + closes the socket afterwards.
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
        let plot_owner = super::plot_owner_id(key);
        self.with_conn(|conn| {
            // True when the statement ran (whatever it deleted), false when it failed.
            let mut del = |label: &str, sql: &str, binds: &[&dyn rusqlite::types::ToSql]| -> bool {
                match conn.execute(sql, binds) {
                    Ok(n) => {
                        receipt.push((label.to_string(), n));
                        true
                    }
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
                        false
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
            // Rows keyed by the NAME, and the listing images (reached through the listings):
            // once the registration and the listings are gone, nothing keyed by the key points
            // at them. So when one of them fails to delete, the erase keeps the registration
            // (or the listings) as well: what was left then stays tied to this key, where
            // `erase_left_rows` sees it and the next erase finds it (review of BUG-135 option 2,
            // second round, finding 6). Both of these run either way (`&`, not `&&`).
            let name_rows_went = del("profile", "DELETE FROM profiles WHERE name = ?1 COLLATE NOCASE", &[&name])
                & del("statuses", "DELETE FROM user_status WHERE name = ?1 COLLATE NOCASE", &[&name]);
            del("friend_codes", "DELETE FROM friend_codes WHERE public_key = ?1", &[&key]);
            del("listing_reviews", "DELETE FROM listing_reviews WHERE reviewer_key = ?1", &[&key]);
            let images_went = del("listing_images", "DELETE FROM listing_images WHERE listing_id IN (SELECT id FROM marketplace_listings WHERE seller_key = ?1)", &[&key]);
            if images_went {
                del("listings", "DELETE FROM marketplace_listings WHERE seller_key = ?1", &[&key]);
            }
            del("tasks", "DELETE FROM project_tasks WHERE created_by = ?1", &[&key]);
            del("roles", "DELETE FROM user_roles WHERE public_key = ?1", &[&key]);
            del("membership", "DELETE FROM server_members WHERE public_key = ?1", &[&key]);
            if name_rows_went {
                del("registered_name", "DELETE FROM registered_names WHERE public_key = ?1", &[&key]);
            }
            // Their plot on the ship goes back for the next player (ship homes 1b, the third
            // review: an erased account held its plot for good, and nothing in the app could
            // free it, because the key an admin would name was erased with everything else).
            del("ship_plots", "DELETE FROM game_plots WHERE owner_did = ?1", &[&plot_owner]);
            // Their progress in the shared world (round 4 of the 1b review).
            del("game_progress", "DELETE FROM player_progress WHERE public_key = ?1", &[&key]);
            // Their fleet ledger (2026-10-04).
            del("fleet_ledger", "DELETE FROM fleet_ledger WHERE public_key = ?1", &[&key]);
            // Fold the secure_delete-zeroed pages out of the WAL.
            let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
            // What was kept on purpose above is said in the receipt as not erased, the way a
            // failed delete is, so the erase reports `partial` and the person erases again.
            for (kept, went) in [("listings", images_went), ("registered_name", name_rows_went)] {
                if !went {
                    tracing::error!("account erase: kept {kept}, because a delete that depends on it failed");
                    receipt.push((format!("{kept}_FAILED"), 1));
                }
            }
        });
        receipt
    }

    /// Whether anything of the account that `delete_account` answers for is still here: true
    /// after an erase that did not finish (a `<table>_FAILED` in its receipt), or when this
    /// key wrote again since (a device still signed in from before the erase). A device of the
    /// account that was offline during the erase is told what it would have been told then
    /// (handlers/sign_ups.rs `erased_here_frame`, review of BUG-135 option 2, finding 16):
    /// finished, so Connect signs up afresh, or not, so erase again; the erased-accounts entry
    /// keeps only the day, so this is read from what is left.
    ///
    /// Read: every table `delete_account` erases by the key (or by the plot owner made from it),
    /// with the same column, pinned to `delete_account` by
    /// `the_left_rows_check_reads_every_table_the_erase_answers_for`. Not read, because rows
    /// there come back without the erase having left anything (the second round of the review,
    /// finding 6): `dm_mailbox` (sealed mail other people send keeps arriving for the key) and
    /// `signed_profiles` (profile gossip from another server, where the key may still have an
    /// account, writes the cached copy back). The tables erased by NAME (profiles, statuses)
    /// and the listing images (reached through the listings) cannot be found from the key once
    /// the registration and the listings are gone, so `delete_account` keeps those two whenever
    /// a delete that hangs off them fails, and reading those two here covers the rest.
    pub fn erase_left_rows(&self, key: &str) -> bool {
        let plot_owner = super::plot_owner_id(key);
        let q = "SELECT EXISTS(SELECT 1 FROM messages WHERE from_key = ?1)
            OR EXISTS(SELECT 1 FROM reactions WHERE reactor_key = ?1)
            OR EXISTS(SELECT 1 FROM user_uploads WHERE public_key = ?1)
            OR EXISTS(SELECT 1 FROM notification_prefs WHERE public_key = ?1)
            OR EXISTS(SELECT 1 FROM vault_blobs WHERE public_key = ?1)
            OR EXISTS(SELECT 1 FROM push_subscriptions WHERE public_key = ?1)
            OR EXISTS(SELECT 1 FROM friend_codes WHERE public_key = ?1)
            OR EXISTS(SELECT 1 FROM listing_reviews WHERE reviewer_key = ?1)
            OR EXISTS(SELECT 1 FROM marketplace_listings WHERE seller_key = ?1)
            OR EXISTS(SELECT 1 FROM project_tasks WHERE created_by = ?1)
            OR EXISTS(SELECT 1 FROM user_roles WHERE public_key = ?1)
            OR EXISTS(SELECT 1 FROM server_members WHERE public_key = ?1)
            OR EXISTS(SELECT 1 FROM registered_names WHERE public_key = ?1)
            OR EXISTS(SELECT 1 FROM game_plots WHERE owner_did = ?2)
            OR EXISTS(SELECT 1 FROM player_progress WHERE public_key = ?1)
            OR EXISTS(SELECT 1 FROM fleet_ledger WHERE public_key = ?1)";
        self.with_read_conn(|conn| conn.query_row(q, params![key, plot_owner], |r| r.get::<_, bool>(0)))
            .unwrap_or_else(|e| {
                // Unknown is said as "not finished": that note only asks the person to erase
                // again, never promises a fresh start that may not be true.
                tracing::error!("account: could not check what an erase left: {e}");
                true
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::storage::plot_owner_id;

    fn test_storage() -> Storage {
        Storage::open_temp("account")
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

    /// Ship homes 1b, the third review: erasing an account gives back the plot its home stood
    /// on (held under the DID from the key, `plot_owner_id`, which no key- or name-keyed DELETE
    /// could match), so two erased accounts can no longer fill the shipped two-plot ship for
    /// good; and the export lists the plot first. Another player's plot is untouched.
    ///
    /// Seen red 2026-10-03 with neither the grab nor the DELETE for `game_plots` (the a504c5cd9
    /// account.rs): "the export lists the plot their home stands on: []".
    #[test]
    fn erasing_an_account_gives_back_its_plot_and_the_export_lists_it() {
        let db = test_storage();
        let (cara, dev) = ("c0ffee01", "c0ffee02"); // hex keys, held under their DIDs
        db.register_name("Cara", cara).unwrap();
        let plots = ["p1", "p2"];
        assert_eq!(db.claim_plot("mothership-1", &plot_owner_id(cara), &plots).unwrap().as_deref(), Some("p1"));
        assert_eq!(db.claim_plot("mothership-1", &plot_owner_id(dev), &plots).unwrap().as_deref(), Some("p2"));

        let export = db.export_account(cara, "Cara");
        let listed = export["ship_plots"].as_array().cloned().unwrap_or_default();
        assert!(
            listed.len() == 1 && listed[0]["plot_id"] == "p1" && listed[0]["world_id"] == "mothership-1",
            "the export lists the plot their home stands on: {listed:?}"
        );

        let receipt = db.delete_account(cara, "Cara");
        assert!(receipt.iter().any(|(l, n)| l == "ship_plots" && *n == 1), "the erase gives the plot back: {receipt:?}");
        assert_eq!(db.plot_holder("mothership-1", "p1").unwrap(), None, "p1 is free");
        assert_eq!(db.plot_holder("mothership-1", "p2").unwrap(), Some(plot_owner_id(dev)), "the other player keeps theirs");
        // The next player gets the freed plot.
        assert_eq!(db.claim_plot("mothership-1", &plot_owner_id("c0ffee03"), &plots).unwrap().as_deref(), Some("p1"));
    }

    /// BUG-135, 2026-10-04: a key whose erase this server remembers sees that in its export
    /// (only the day; the row is held under a one-way fingerprint, never the key), and
    /// another key sees nothing of it. Neither an erase nor an export deletes it.
    ///
    /// Seen red 2026-10-04 with the `erased_here` grab taken out: "the export lists the
    /// remembered erase: []".
    #[test]
    fn the_export_lists_a_remembered_erase_to_its_own_key_only() {
        let db = test_storage();
        db.register_name("Fay", "fa11").unwrap();
        db.remember_erased_account("fa11").unwrap();
        let export = db.export_account("fa11", "Fay");
        let listed = export["erased_here"].as_array().cloned().unwrap_or_default();
        assert!(
            listed.len() == 1
                && listed[0]["erased_day"].as_i64().is_some()
                && listed[0]["ttl_days"] == 30
                && listed[0].as_object().map_or(0, |o| o.len()) == 2,
            "the export lists the remembered erase: {listed:?}"
        );
        assert!(db.export_account("0ther", "Other")["erased_here"].as_array().unwrap().is_empty(), "another key sees nothing");
        db.delete_account("fa11", "Fay");
        assert!(db.erased_account_remembered("fa11"), "the erase does not delete what records it");
    }

    /// Ship homes 1b, round 4 of the review (finding 11): the player's progress in the shared
    /// world (`player_progress`: their quest, completed quests, XP and reputation, kept by
    /// public key so a returning player resumes) was neither exported nor erased. The export
    /// lists it, the erase deletes it, and another player's row is untouched.
    ///
    /// Seen red 2026-10-03 on db551f530: "the export lists their progress in the shared world:
    /// []".
    #[test]
    fn erasing_an_account_erases_its_progress_in_the_shared_world() {
        let db = test_storage();
        db.register_name("Eli", "e11e").unwrap();
        db.save_player_progress("e11e", Some("survey_storage"), &["explore_ship".to_string()], 300, 15).unwrap();
        db.save_player_progress("f00d", None, &[], 5, 0).unwrap();
        let export = db.export_account("e11e", "Eli");
        let listed = export["game_progress"].as_array().cloned().unwrap_or_default();
        assert!(
            listed.len() == 1 && listed[0]["xp"] == 300 && listed[0]["current_quest"] == "survey_storage",
            "the export lists their progress in the shared world: {listed:?}"
        );
        let receipt = db.delete_account("e11e", "Eli");
        assert!(receipt.iter().any(|(l, n)| l == "game_progress" && *n == 1), "the erase deletes it: {receipt:?}");
        assert!(db.load_player_progress("e11e").unwrap().is_none(), "nothing of it is left");
        assert!(db.load_player_progress("f00d").unwrap().is_some(), "another player keeps theirs");
    }

    /// Review of BUG-135 option 2, finding 16: a device that was offline during an erase is
    /// told whether it finished, read from what is left. A finished erase leaves nothing the
    /// check reads; mail other people send the key afterwards does not count; a row the
    /// erase failed to delete (here a message, put back by hand) does.
    ///
    /// Seen red 2026-10-04 with `erase_left_rows` answering false whatever was left: "an
    /// account that was never erased has rows" (its first assertion).
    #[test]
    fn what_an_erase_left_is_read_from_the_tables_it_erases() {
        let db = test_storage();
        db.register_name("Gil", "9111").unwrap();
        db.join_server("9111", "Gil").unwrap();
        db.save_player_progress("9111", None, &[], 10, 0).unwrap();
        assert!(db.erase_left_rows("9111"), "an account that was never erased has rows");
        let receipt = db.delete_account("9111", "Gil");
        assert!(!receipt.iter().any(|(l, _)| l.ends_with("_FAILED")), "{receipt:?}");
        assert!(!db.erase_left_rows("9111"), "a finished erase was read as unfinished");
        db.mailbox_put("9111", "sealed mail from someone else, after the erase").unwrap();
        assert!(!db.erase_left_rows("9111"), "mail that arrived after the erase counted as left over");
        db.with_conn(|c| {
            c.execute(
                "INSERT INTO messages (msg_type, from_key, from_name, content, timestamp, raw_json) VALUES ('chat', '9111', 'Gil', 'hi', 1, '{}')",
                [],
            )
        })
        .unwrap();
        assert!(db.erase_left_rows("9111"), "a row the erase left was not seen");
    }

    /// Review of BUG-135 option 2 (second round), finding 6: `erase_left_rows` reads only what
    /// the erase itself answers for. A delete that fails on a table keyed by the NAME (profiles,
    /// statuses) or on the listing images (found through the listings) leaves rows that nothing
    /// keyed by the key points at once the registration and the listings are gone, so the erase
    /// keeps those two whenever what hangs off them failed, and the check sees them. Failures
    /// forced with a trigger that refuses the delete (the rows stay, as after a real failure).
    ///
    /// Seen red 2026-10-04 on 8695b08d4 (the erase deleted the registration and the listings
    /// whatever failed before them): "profiles: an erase that left rows of the account was read
    /// as finished: [... (\"profile_FAILED\", 1), ... (\"registered_name\", 1), ...]".
    #[test]
    fn an_erase_that_fails_on_a_name_or_image_row_is_read_as_unfinished() {
        for table in ["profiles", "user_status", "listing_images"] {
            let db = test_storage();
            db.register_name("Hal", "4a11").unwrap();
            db.join_server("4a11", "Hal").unwrap();
            db.save_profile("Hal", "bio", "{}").unwrap();
            db.save_user_status("Hal", "online", "here").unwrap();
            db.create_listing("l1", "4a11", "Hal", "Pump", "", "tools", "used", "5", "", "").unwrap();
            db.add_listing_image("l1", "/uploads/pump.png", 0).unwrap();
            db.with_conn(|c| c.execute_batch(&format!("CREATE TRIGGER held BEFORE DELETE ON {table} BEGIN SELECT RAISE(ABORT, 'held'); END;")))
                .unwrap();
            let receipt = db.delete_account("4a11", "Hal");
            assert!(receipt.iter().any(|(l, _)| l.ends_with("_FAILED")), "{table}: the forced failure did not happen: {receipt:?}");
            assert!(db.erase_left_rows("4a11"), "{table}: an erase that left rows of the account was read as finished: {receipt:?}");
        }
    }

    /// Finding 6, the other way: profile gossip from another server writes `signed_profiles`
    /// for a key that still has an account THERE. It is no part of what this server's erase
    /// left, so a finished erase stays finished.
    ///
    /// Seen red 2026-10-04 on 8695b08d4 (the check read `signed_profiles`): "a profile another
    /// server gossiped back made a finished erase read as unfinished".
    #[test]
    fn a_profile_gossiped_after_a_finished_erase_does_not_make_it_unfinished() {
        let db = test_storage();
        db.register_name("Ivy", "5b22").unwrap();
        let receipt = db.delete_account("5b22", "Ivy");
        assert!(!receipt.iter().any(|(l, _)| l.ends_with("_FAILED")), "{receipt:?}");
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
        db.store_signed_profile("5b22", "Ivy", "from a peer server", "", "", "{}", "", "", "", now, "sig").unwrap();
        assert!(!db.erase_left_rows("5b22"), "a profile another server gossiped back made a finished erase read as unfinished");
    }

    /// Finding 7: the pin between `erase_left_rows` and `delete_account` is read from the
    /// source however a `del(` call is laid out (one line or several, any bind), and every
    /// DELETE in `delete_account` must sit in a `del(` call the scan understood. A table the
    /// erase deletes by key (or by the plot owner) is read by the check with the same column,
    /// unless it is listed below with its reason; a table reached by name or through another
    /// table is anchored (the erase keeps the registration or the listings when it fails, which
    /// `an_erase_that_fails_on_a_name_or_image_row_is_read_as_unfinished` proves).
    ///
    /// Seen red 2026-10-04 with a `del(` call laid out over four lines added to `delete_account`
    /// (`task_comments` by `author_key`), which the one-line scan this replaces passed:
    /// "erase_left_rows does not read what delete_account erases: [\"FROM task_comments WHERE
    /// author_key = ?1\"]". That scan's "\r\n" escape had also been turned into raw line breaks,
    /// so it normalised nothing and, on a CRLF checkout (the operator's), panicked at its find of
    /// the closing brace: "called `Option::unwrap()` on a `None` value" (run 2026-10-04 on a CRLF
    /// copy of this file); this test passes on a CRLF copy.
    #[test]
    fn the_left_rows_check_reads_every_table_the_erase_answers_for() {
        // Not read, with the reason: mail other people send keeps arriving after an erase; a
        // gossiped profile is written back by other servers where the key still has an account.
        const NOT_READ: [&str; 2] = ["dm_mailbox", "signed_profiles"];
        let src = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/relay/storage/account.rs")).unwrap();
        let src = src.replace("\r\n", "\n"); // a Windows checkout
        let erase = &src[src.find("pub fn delete_account").unwrap()..src.find("pub fn erase_left_rows").unwrap()];
        let check = &src[src.find("pub fn erase_left_rows").unwrap()..];
        let check = &check[..check.find("\n    }\n").unwrap()];
        // Every del( call, to its closing parenthesis, skipping string contents.
        let mut calls = Vec::new();
        let mut rest = erase;
        while let Some(at) = rest.find("del(") {
            let before = rest[..at].chars().last();
            let call = &rest[at..];
            if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                rest = &call[4..];
                continue;
            }
            let (mut depth, mut in_str, mut prev, mut end) = (0i32, false, ' ', call.len());
            for (i, c) in call.char_indices() {
                match c {
                    '"' if prev != '\\' => in_str = !in_str,
                    '(' if !in_str => depth += 1,
                    ')' if !in_str => {
                        depth -= 1;
                        if depth == 0 {
                            end = i + 1;
                            break;
                        }
                    }
                    _ => {}
                }
                prev = c;
            }
            calls.push(call[..end].to_string());
            rest = &call[end..];
        }
        let deletes = erase.matches("\"DELETE FROM ").count();
        let parsed: Vec<&String> = calls.iter().filter(|c| c.contains("\"DELETE FROM ")).collect();
        assert_eq!(parsed.len(), deletes, "a DELETE in delete_account is not in a del( call the scan reads: {calls:?}");
        assert!(deletes >= 18, "the scan found only {deletes} deletes");
        let mut missing = Vec::new();
        for call in &parsed {
            let sql = &call[call.find("\"DELETE FROM ").unwrap() + 1..];
            let sql = &sql[..sql.find('"').unwrap()];
            let mut words = sql.split_whitespace();
            let (table, col) = (words.nth(2).unwrap(), words.nth(1).unwrap());
            let binds = &call[call.find(sql).unwrap() + sql.len()..];
            let anchored = binds.contains("&name") || sql.contains("SELECT");
            if anchored || NOT_READ.contains(&table) {
                continue;
            }
            let n = if binds.contains("&plot_owner") { 2 } else { 1 };
            let want = format!("FROM {table} WHERE {col} = ?{n}");
            if !check.contains(&want) {
                missing.push(want);
            }
        }
        assert!(missing.is_empty(), "erase_left_rows does not read what delete_account erases: {missing:?}");
        for t in NOT_READ {
            assert!(!check.contains(&format!("FROM {t} ")), "erase_left_rows reads {t}, which is not the erase's to answer for");
        }
        // The anchors the name-keyed and image rows rely on.
        assert!(
            check.contains("FROM registered_names WHERE public_key = ?1") && check.contains("FROM marketplace_listings WHERE seller_key = ?1"),
            "erase_left_rows no longer reads the registration and the listings the name and image rows are anchored to"
        );
    }
}
