//! Who holds which plot of the ship (increment 1b of
//! docs/design/ship-homes-and-logistics.md, section 5.10).
//!
//! A PLOT is where one household's home goes on the mothership. Its geometry
//! (origin, size, door) lives in data, in the ship file
//! (data/blueprints/ship_structure.ron), never here: one source of truth, the
//! same file every client draws from. This table holds ownership only:
//!
//! ```text
//! game_plots (world_id, plot_id, owner_did, assigned_at)
//!   PRIMARY KEY (world_id, plot_id)   one holder per plot
//!   UNIQUE      (world_id, owner_did) one plot per player
//! ```
//!
//! `world_id` is the ship's id from the ship file, so a later fleet of ships
//! keeps one row set per ship. `owner_did` is the player's `did:hum:` (from
//! their Dilithium key), so a plot follows the person, not a socket.
//!
//! The two constraints are what make sharing impossible rather than unlikely:
//! two joins at the same moment can never both hold one plot (the primary
//! key), and one player can never hold two (the unique pair). Every claim also
//! runs on the single writer connection, inside one transaction.
//!
//! Privacy (design section 5.10): the owner is used for the claim only. No
//! endpoint lists who lives where.

use super::{now_millis, Storage};
use rusqlite::{params, OptionalExtension};

impl Storage {
    /// The plot `owner` holds on `world`, or else the first of `plots` (in the
    /// ship file's order) that nobody holds, claimed for them now. None when
    /// every plot is held: a full ship, and the caller makes them a guest.
    ///
    /// A held row whose plot is no longer in `plots` (it left the ship file)
    /// is dropped and the player claims afresh, so the data stays the one
    /// source of truth for which plots exist.
    pub fn claim_plot(&self, world: &str, owner: &str, plots: &[&str]) -> Result<Option<String>, rusqlite::Error> {
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            let held: Option<String> = tx
                .query_row(
                    "SELECT plot_id FROM game_plots WHERE world_id = ?1 AND owner_did = ?2",
                    params![world, owner],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(id) = held {
                if plots.contains(&id.as_str()) {
                    tx.commit()?;
                    return Ok(Some(id));
                }
                tx.execute(
                    "DELETE FROM game_plots WHERE world_id = ?1 AND owner_did = ?2",
                    params![world, owner],
                )?;
            }
            let now = now_millis() as i64;
            let mut claimed = None;
            for id in plots {
                // OR IGNORE: a plot someone holds is skipped by the primary key.
                let n = tx.execute(
                    "INSERT OR IGNORE INTO game_plots (world_id, plot_id, owner_did, assigned_at)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![world, id, owner, now],
                )?;
                if n == 1 {
                    claimed = Some(id.to_string());
                    break;
                }
            }
            tx.commit()?;
            Ok(claimed)
        })
    }

    /// Give a plot back (nothing calls this in increment 1b: plots are kept
    /// for good until the moving and inactivity rules of later increments).
    /// Used by the tests to prove a freed plot is claimed again.
    #[allow(dead_code)]
    pub fn release_plot(&self, world: &str, owner: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let n = conn.execute(
                "DELETE FROM game_plots WHERE world_id = ?1 AND owner_did = ?2",
                params![world, owner],
            )?;
            Ok(n > 0)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db(tag: &str) -> (Storage, std::path::PathBuf) {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_plots_store_{tag}_{}_{nanos}.db", std::process::id()));
        (Storage::open(&path).expect("open test db"), path)
    }

    const PLOTS: [&str; 2] = ["p1", "p2"];

    #[test]
    fn claims_in_order_keeps_what_it_gave_and_fills_up() {
        let (db, path) = temp_db("order");
        assert_eq!(db.claim_plot("ship", "did:hum:a", &PLOTS).unwrap().as_deref(), Some("p1"));
        assert_eq!(db.claim_plot("ship", "did:hum:b", &PLOTS).unwrap().as_deref(), Some("p2"));
        // Asked again, each keeps their own.
        assert_eq!(db.claim_plot("ship", "did:hum:b", &PLOTS).unwrap().as_deref(), Some("p2"));
        assert_eq!(db.claim_plot("ship", "did:hum:a", &PLOTS).unwrap().as_deref(), Some("p1"));
        // A third player: the ship is full.
        assert_eq!(db.claim_plot("ship", "did:hum:c", &PLOTS).unwrap(), None);
        // A freed plot goes to the next who asks.
        assert!(db.release_plot("ship", "did:hum:a").unwrap());
        assert_eq!(db.claim_plot("ship", "did:hum:c", &PLOTS).unwrap().as_deref(), Some("p1"));
        // Another ship is another row set.
        assert_eq!(db.claim_plot("other", "did:hum:a", &PLOTS).unwrap().as_deref(), Some("p1"));
        let _ = std::fs::remove_file(&path);
    }

    /// A plot that left the ship file no longer holds anyone: its row is
    /// dropped and the player claims a plot that exists.
    #[test]
    fn a_plot_that_left_the_ship_file_is_dropped() {
        let (db, path) = temp_db("left");
        assert_eq!(db.claim_plot("ship", "did:hum:a", &PLOTS).unwrap().as_deref(), Some("p1"));
        assert_eq!(db.claim_plot("ship", "did:hum:a", &["p2", "p3"]).unwrap().as_deref(), Some("p2"));
        let rows: i64 = db
            .with_conn(|c| c.query_row("SELECT COUNT(*) FROM game_plots WHERE owner_did = 'did:hum:a'", [], |r| r.get(0)))
            .unwrap();
        assert_eq!(rows, 1, "the old row went");
        let _ = std::fs::remove_file(&path);
    }

    /// The table itself refuses a shared plot and a second plot for one
    /// player, whatever the code above does: the constraints are the guard.
    /// Seen red 2026-10-03 with the UNIQUE (world_id, owner_did) line taken
    /// out of the CREATE: "a second plot for one player is refused by the
    /// table" failed (the insert succeeded).
    #[test]
    fn the_table_refuses_a_shared_plot_and_a_second_plot_for_one_player() {
        let (db, path) = temp_db("constraints");
        let insert = |plot: &str, owner: &str| {
            db.with_conn(|c| {
                c.execute(
                    "INSERT INTO game_plots (world_id, plot_id, owner_did, assigned_at) VALUES ('ship', ?1, ?2, 0)",
                    params![plot, owner],
                )
            })
        };
        insert("p1", "did:hum:a").expect("the first claim");
        assert!(insert("p1", "did:hum:b").is_err(), "a plot already held is refused by the table");
        assert!(insert("p2", "did:hum:a").is_err(), "a second plot for one player is refused by the table");
        let _ = std::fs::remove_file(&path);
    }

    /// Many joins at once, from many threads: every player gets a different
    /// plot, never two the same, and the extras are guests.
    #[test]
    fn simultaneous_claims_never_share_a_plot() {
        let (db, path) = temp_db("race");
        let db = std::sync::Arc::new(db);
        let plots: Vec<String> = (1..=8).map(|n| format!("p{n}")).collect();
        let handles: Vec<_> = (0..16)
            .map(|i| {
                let db = db.clone();
                let plots = plots.clone();
                std::thread::spawn(move || {
                    let refs: Vec<&str> = plots.iter().map(|s| s.as_str()).collect();
                    db.claim_plot("ship", &format!("did:hum:{i}"), &refs).unwrap()
                })
            })
            .collect();
        let got: Vec<Option<String>> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let held: Vec<&String> = got.iter().flatten().collect();
        let unique: std::collections::HashSet<&&String> = held.iter().collect();
        assert_eq!(held.len(), 8, "eight plots, eight holders: {got:?}");
        assert_eq!(unique.len(), 8, "no plot given twice: {got:?}");
        let _ = std::fs::remove_file(&path);
    }
}
