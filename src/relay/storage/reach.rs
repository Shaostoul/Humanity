//! "Who can reach me" (2026-10-09, docs/design/blocking-and-safe-mode.md 10c): the audience
//! each person chose for each kind of contact, `reach_settings (public_key, kind, audience)`.
//!
//! A missing row means the safe default for that kind (handlers/reach.rs `Kind::default_audience`),
//! so someone who never opened Settings > Safety has no rows at all and is still protected, and
//! nothing has to be written when a person signs up. The words are checked by the handler before
//! they are written (handlers/reach.rs `handle_reach_set`); this module stores and reads them as
//! given, and the handler reads a word it does not know as the default.
//!
//! Per server for now. Sharing the setting across federated servers as a signed setting (10a)
//! comes with the federation work.
//!
//! Listed by the account export and deleted by the account erase (storage/account.rs).

use rusqlite::{params, OptionalExtension};

use super::Storage;

impl Storage {
    /// Every (kind, audience) row `key` saved, sorted by kind. Empty means every kind is at its
    /// default. A read that fails is logged and read as no rows, which is the safe defaults.
    pub fn reach_settings_of(&self, key: &str) -> Vec<(String, String)> {
        self.with_read_conn(|conn| {
            let mut stmt = conn.prepare("SELECT kind, audience FROM reach_settings WHERE public_key = ?1 ORDER BY kind ASC")?;
            let rows = stmt.query_map(params![key], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            rows.collect()
        })
        .unwrap_or_else(|e| {
            tracing::error!("reach settings: could not read a person's settings, using the defaults: {e}");
            Vec::new()
        })
    }

    /// The audience `key` saved for `kind`, if they saved one. A failed read is logged and read
    /// as none, which is the default for that kind.
    pub fn reach_audience_of(&self, key: &str, kind: &str) -> Option<String> {
        self.with_read_conn(|conn| {
            conn.query_row(
                "SELECT audience FROM reach_settings WHERE public_key = ?1 AND kind = ?2",
                params![key, kind],
                |r| r.get::<_, String>(0),
            )
            .optional()
        })
        .unwrap_or_else(|e| {
            tracing::error!("reach settings: could not read an audience, using the default: {e}");
            None
        })
    }

    /// Save `rows` (kind, audience) for `key`, all of them or none: one transaction, each row
    /// replacing what that kind held before. Kinds not named keep what they had.
    pub fn set_reach_settings(&self, key: &str, rows: &[(&str, &str)]) -> Result<(), rusqlite::Error> {
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            for (kind, audience) in rows {
                tx.execute(
                    "INSERT OR REPLACE INTO reach_settings (public_key, kind, audience) VALUES (?1, ?2, ?3)",
                    params![key, kind, audience],
                )?;
            }
            tx.commit()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Saved rows read back for their own key only, a second save of a kind replaces it, kinds
    /// not named keep what they had, and nothing saved reads as no rows (the defaults).
    ///
    /// Seen red 2026-10-09 with the WHERE clause of `reach_settings_of` reading `kind = ?1`
    /// instead of `public_key = ?1`: "Ada's rows: []".
    #[test]
    fn settings_save_read_back_and_replace_per_kind() {
        let db = Storage::open_temp("reach_rows");
        assert!(db.reach_settings_of("ada_key").is_empty(), "nothing saved reads as the defaults");
        assert_eq!(db.reach_audience_of("ada_key", "call"), None);

        db.set_reach_settings("ada_key", &[("message", "anyone"), ("call", "nobody")]).unwrap();
        db.set_reach_settings("bo_key", &[("trade", "groups")]).unwrap();
        let ada = db.reach_settings_of("ada_key");
        assert_eq!(ada, vec![("call".to_string(), "nobody".to_string()), ("message".to_string(), "anyone".to_string())], "Ada's rows: {ada:?}");

        db.set_reach_settings("ada_key", &[("call", "chosen")]).unwrap();
        assert_eq!(db.reach_audience_of("ada_key", "call").as_deref(), Some("chosen"), "a second save replaces the kind");
        assert_eq!(db.reach_audience_of("ada_key", "message").as_deref(), Some("anyone"), "a kind not named keeps what it had");
        assert_eq!(db.reach_audience_of("ada_key", "trade"), None);
        assert_eq!(db.reach_settings_of("bo_key"), vec![("trade".to_string(), "groups".to_string())], "another person is untouched");
    }

    /// The table is a plain `CREATE TABLE IF NOT EXISTS` in a batch of its own, so a database
    /// made before it existed gains it on the next start (the BUG-046 shape: the live relay's
    /// database is never a fresh one), with the columns the handlers read.
    ///
    /// Seen red 2026-10-09 with the reach_settings batch removed from `Storage::open`: "a
    /// database from before step B has no reach_settings table after a restart: no such table:
    /// reach_settings".
    #[test]
    fn a_database_from_before_the_table_gains_it_on_open() {
        let dir = crate::test_temp::dir("reach_premigration");
        let path = dir.join("relay.db");
        let db = Storage::open(&path).expect("open");
        db.with_conn(|c| c.execute_batch("DROP TABLE IF EXISTS reach_settings;")).unwrap();
        db.join_server("old_key", "Old").unwrap();
        drop(db);

        let reopened = Storage::open(&path).expect("a database from before step B opens");
        let cols: Result<Vec<String>, rusqlite::Error> = reopened.with_read_conn(|c| {
            c.prepare("SELECT kind, audience FROM reach_settings WHERE public_key = 'x'")?;
            let mut st = c.prepare("PRAGMA table_info(reach_settings)")?;
            let cols = st.query_map([], |r| r.get::<_, String>(1))?.filter_map(|r| r.ok()).collect();
            Ok(cols)
        });
        let cols = cols.unwrap_or_else(|e| panic!("a database from before step B has no reach_settings table after a restart: {e}"));
        assert_eq!(cols, ["public_key", "kind", "audience"]);
        assert!(reopened.is_member("old_key"), "and what it held is still there");
        reopened.set_reach_settings("old_key", &[("message", "anyone")]).unwrap();
        assert_eq!(reopened.reach_audience_of("old_key", "message").as_deref(), Some("anyone"));
    }

    /// The account export lists the person's settings, the erase deletes them and says so in its
    /// receipt, a row written again afterwards counts as left over (`erase_left_rows`), and
    /// another person's settings are untouched.
    ///
    /// Seen red 2026-10-09 with the reach_settings line taken out of `export_account`: "the
    /// export lists who can reach them: []"; and with the DELETE taken out of `delete_account`:
    /// "the erase deletes them and says so" (no reach_settings in the receipt).
    #[test]
    fn the_export_lists_the_settings_and_the_erase_deletes_them() {
        let db = Storage::open_temp("reach_account");
        db.register_name("Cy", "c1c1").unwrap();
        db.set_reach_settings("c1c1", &[("call", "friends"), ("message", "anyone")]).unwrap();
        db.set_reach_settings("d0d0", &[("call", "nobody")]).unwrap();

        let export = db.export_account("c1c1", "Cy");
        let listed = export["reach_settings"].as_array().cloned().unwrap_or_default();
        assert!(
            listed.len() == 2
                && listed[0]["kind"] == "call"
                && listed[0]["audience"] == "friends"
                && listed[1]["kind"] == "message"
                && listed[0].as_object().map_or(0, |o| o.len()) == 2,
            "the export lists who can reach them: {listed:?}"
        );

        let receipt = db.delete_account("c1c1", "Cy");
        assert!(receipt.iter().any(|(l, n)| l == "reach_settings" && *n == 2), "the erase deletes them and says so: {receipt:?}");
        assert!(db.reach_settings_of("c1c1").is_empty());
        assert!(!db.erase_left_rows("c1c1"), "nothing of the account is left");
        assert_eq!(db.reach_settings_of("d0d0").len(), 1, "another person's settings stay");

        // A device still signed in from before the erase writes a setting again: it is seen.
        db.set_reach_settings("c1c1", &[("trade", "nobody")]).unwrap();
        assert!(db.erase_left_rows("c1c1"), "a setting written after the erase was not seen as left over");
    }
}
