//! Paused federation peers (2026-10-22, the Federation panel of docs/design/in-app-ops.md).
//!
//! Pausing keeps a peer in the registry, with its trust and its pinned key, while federation with
//! it stops. It works through the one read every federation path already makes: while a peer's
//! row has a pause here, `list_federated_servers` (misc.rs) leaves it out, so the outbound dialer
//! skips it, an inbound hello from it finds no row and is refused, and a link already open closes
//! within `REGISTRY_CHECK_SECS` because `until_removed` (handlers/federation.rs) no longer finds
//! it. None of that code needed to change. The admin views read
//! [`Storage::list_federated_servers_all`], which keeps every row and says which are paused.

use std::collections::HashSet;

use rusqlite::params;

use super::{FederatedServer, Storage};

impl Storage {
    /// Pause federation with `server_id`. False when no such peer is registered.
    pub fn pause_federated_server(&self, server_id: &str) -> Result<bool, rusqlite::Error> {
        let now = super::now_millis() as i64;
        self.with_conn(|conn| {
            let known: i64 = conn.query_row(
                "SELECT COUNT(*) FROM federated_servers WHERE server_id = ?1",
                params![server_id],
                |r| r.get(0),
            )?;
            if known == 0 {
                return Ok(false);
            }
            conn.execute(
                "INSERT OR IGNORE INTO federation_paused (server_id, paused_at) VALUES (?1, ?2)",
                params![server_id, now],
            )?;
            Ok(true)
        })
    }

    /// Resume federation with `server_id`. False when it was not paused.
    pub fn resume_federated_server(&self, server_id: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            Ok(conn.execute("DELETE FROM federation_paused WHERE server_id = ?1", params![server_id])? > 0)
        })
    }

    /// The ids of every paused peer.
    pub fn paused_federated_servers(&self) -> Result<HashSet<String>, rusqlite::Error> {
        self.with_read_conn(|conn| {
            let mut stmt = conn.prepare("SELECT server_id FROM federation_paused")?;
            let ids = stmt.query_map([], |r| r.get::<_, String>(0))?.filter_map(|r| r.ok()).collect();
            Ok(ids)
        })
    }

    /// Every registered peer, paused ones included, each with whether it is paused: what the
    /// admin views show. Federation itself reads `list_federated_servers`, which leaves the
    /// paused ones out.
    pub fn list_federated_servers_all(&self) -> Result<Vec<(FederatedServer, bool)>, rusqlite::Error> {
        let paused = self.paused_federated_servers()?;
        let rows = self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT server_id, name, url, public_key, trust_tier, accord_compliant, status, last_seen, added_at
                 FROM federated_servers
                 ORDER BY trust_tier DESC, name ASC",
            )?;
            let rows: Vec<FederatedServer> = stmt
                .query_map([], |row| {
                    Ok(FederatedServer {
                        server_id: row.get(0)?,
                        name: row.get(1)?,
                        url: row.get(2)?,
                        public_key: row.get(3)?,
                        trust_tier: row.get(4)?,
                        accord_compliant: row.get::<_, i32>(5)? != 0,
                        status: row.get(6)?,
                        last_seen: row.get(7)?,
                        added_at: row.get(8)?,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok::<_, rusqlite::Error>(rows)
        })?;
        Ok(rows.into_iter().map(|s| { let p = paused.contains(&s.server_id); (s, p) }).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A paused peer is left out of the list federation reads and kept in the admins' list, with
    /// its trust; resuming brings it back; removing it removes the pause, so adding it again does
    /// not find it paused. Seen red 2026-10-22 with `list_federated_servers` reading every row:
    /// "a paused peer is left out of what federation reads".
    #[test]
    fn a_paused_peer_is_kept_but_left_out_of_federation() {
        let db = Storage::open_temp("federation_pause");
        db.add_federated_server("https://a.example", "A", "https://a.example").unwrap();
        db.add_federated_server("https://b.example", "B", "https://b.example").unwrap();
        db.set_server_trust_tier("https://a.example", 2).unwrap();
        assert!(db.pause_federated_server("https://a.example").unwrap());
        assert!(!db.pause_federated_server("https://nobody.example").unwrap(), "no such peer");
        let read: Vec<String> = db.list_federated_servers().unwrap().into_iter().map(|s| s.server_id).collect();
        assert_eq!(read, vec!["https://b.example".to_string()], "a paused peer is left out of what federation reads");
        let all = db.list_federated_servers_all().unwrap();
        let a = all.iter().find(|(s, _)| s.server_id == "https://a.example").expect("the admins still see it");
        assert!(a.1, "and see that it is paused");
        assert_eq!(a.0.trust_tier, 2, "with its trust kept");
        assert!(db.resume_federated_server("https://a.example").unwrap());
        assert_eq!(db.list_federated_servers().unwrap().len(), 2, "resumed");
        db.pause_federated_server("https://a.example").unwrap();
        db.remove_federated_server("https://a.example").unwrap();
        db.add_federated_server("https://a.example", "A", "https://a.example").unwrap();
        assert_eq!(db.list_federated_servers().unwrap().len(), 2, "removing a peer removes its pause");
    }
}
