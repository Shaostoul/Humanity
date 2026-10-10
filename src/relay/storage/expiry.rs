//! The expiry pass: everything this relay keeps only for a set time, deleted once that time is
//! up. One function so every caller runs the same list (review of BUG-135 option 2, finding 3:
//! the erased-accounts cull was wired into two places by hand, nothing showed the wiring, and
//! a settings change culled nothing until the next six-hour pass):
//!   - sealed DM envelopes past `dm_mailbox_ttl_days` (storage/dms.rs);
//!   - public messages past `message_retention_days` (0 = keep forever; pins always kept;
//!     storage/channels.rs);
//!   - erased accounts past their window or over the cap (storage/erased_accounts.rs);
//!   - reports decided more than 90 days ago (storage/reports.rs; open reports are kept).
//!
//! Callers (relay/mod.rs, handlers/server_settings_update.rs): relay start, the six-hour
//! maintenance pass (before its backup, so nothing expired rides into the backup), and every
//! successful `server_settings_update`, so a lowered window or cap holds at once on disk too,
//! which is what the Server Settings page tells the admin.

use super::Storage;

impl Storage {
    /// Run every expiry with this server's current settings. Errors are logged, never fatal:
    /// one failing sweep must not stop the others.
    pub fn run_expiry_sweeps(&self) {
        let s = self.get_server_settings().unwrap_or_default();
        match self.mailbox_expire(s.dm_mailbox_ttl_days) {
            Ok(0) => {}
            Ok(n) => tracing::info!("DM mailbox: expired {n} envelope(s) past the {}-day TTL", s.dm_mailbox_ttl_days),
            Err(e) => tracing::error!("DM mailbox TTL sweep failed: {e}"),
        }
        match self.expire_messages(s.message_retention_days) {
            Ok(0) => {}
            Ok(n) => tracing::info!("Messages: expired {n} past the {}-day retention", s.message_retention_days),
            Err(e) => tracing::error!("Message retention sweep failed: {e}"),
        }
        match self.erased_accounts_sweep(s.erased_accounts_ttl_days, s.erased_accounts_cap) {
            Ok((0, 0)) => {}
            Ok((expired, trimmed)) => tracing::info!(
                "Erased accounts: forgot {expired} past their window and {trimmed} over the cap of {}",
                s.erased_accounts_cap
            ),
            Err(e) => tracing::error!("Erased accounts sweep failed: {e}"),
        }
        match self.reports_expire(super::now_millis() as i64) {
            Ok(0) => {}
            Ok(n) => tracing::info!(
                "Reports: deleted {n} decided more than {} days ago",
                super::reports::REPORT_KEEP_DAYS_AFTER_DECISION
            ),
            Err(e) => tracing::error!("Reports sweep failed: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    fn fresh_db(tag: &str) -> Storage {
        Storage::open_temp_dir(&format!("expiry_{tag}"))
    }

    /// The pass the relay runs at start, every six hours and after a settings change culls
    /// the erased accounts: one past its window and one over the cap are both gone after it,
    /// while the newest stays.
    ///
    /// Seen red 2026-10-04 with the erased-accounts sweep taken out of `run_expiry_sweeps`:
    /// "assertion `left == right` failed: the maintenance pass left erased accounts on disk /
    /// left: 3 / right: 1".
    #[test]
    fn the_maintenance_pass_culls_erased_accounts() {
        let db = fresh_db("cull");
        let mut s = db.get_server_settings().unwrap();
        s.erased_accounts_ttl_days = 30;
        s.erased_accounts_cap = 1;
        assert!(db.set_server_settings(&s, "admin_key").unwrap());
        let today = super::super::dms::unix_day_now();
        for (key, day) in [("expired", today - 40), ("over_cap", today - 3), ("newest", today)] {
            let fp = db.erased_account_fingerprint(key);
            db.with_conn(|c| {
                c.execute(
                    "INSERT INTO erased_accounts (fingerprint, erased_day, ttl_days) VALUES (?1, ?2, 30)",
                    params![fp, day],
                )
            })
            .unwrap();
        }
        db.run_expiry_sweeps();
        assert_eq!(db.erased_accounts_count().unwrap(), 1, "the maintenance pass left erased accounts on disk");
        assert!(db.erased_account_remembered("newest"), "the pass dropped the newest instead of the oldest");
    }

    /// The pass is what every caller runs: relay start and the six-hour pass in relay/mod.rs,
    /// and the settings update (handlers/server_settings_update.rs, which relay.rs hands every
    /// `server_settings_update` to), after it saved. (The storage test above shows the pass
    /// culls; this shows the relay runs it.)
    ///
    /// Seen red 2026-10-04 with the call taken out of the settings update: "relay.rs: a saved
    /// settings change does not run the expiry pass". The update moved into its own handler
    /// file in the world-clock merge (2026-10-04); the test follows it there and also checks
    /// relay.rs still hands the message to that handler.
    #[test]
    fn the_relay_runs_the_pass_at_start_on_its_timer_and_after_a_settings_change() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mod_rs = std::fs::read_to_string(root.join("src/relay/mod.rs")).unwrap();
        let start = mod_rs.find("pub async fn run_relay()").expect("run_relay");
        let body = &mod_rs[start..];
        assert!(body.matches("run_expiry_sweeps()").count() >= 2, "relay/mod.rs: the start or the six-hour pass does not run the expiry pass");
        assert!(!mod_rs.contains("mailbox_expire("), "relay/mod.rs expires the mailbox by hand instead of through the pass");
        let relay_rs = std::fs::read_to_string(root.join("src/relay/relay.rs")).unwrap();
        let arm = relay_rs.find("RelayMessage::ServerSettingsUpdate { .. } =>").expect("relay.rs: the settings update arm");
        // The arm's body is one call; it sits within a few lines of the pattern.
        let call = relay_rs[arm..].find("server_settings_update::handle(");
        assert!(
            call.is_some_and(|at| at < 400),
            "relay.rs: the settings update arm no longer hands the message to handlers/server_settings_update.rs"
        );
        let handler = std::fs::read_to_string(root.join("src/relay/handlers/server_settings_update.rs")).unwrap();
        let upd = handler.find("RelayMessage::ServerSettingsUpdate {").expect("the settings update handler");
        let saved = upd + handler[upd..].find("Ok(true) =>").expect("its saved branch");
        let next_arm = saved + handler[saved..].find("Ok(false) =>").expect("its next branch");
        assert!(handler[saved..next_arm].contains("run_expiry_sweeps()"), "server_settings_update.rs: a saved settings change does not run the expiry pass");
    }
}
