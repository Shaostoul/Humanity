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

/// The id a plot is held under: the player's `did:hum:` from their Dilithium key (hex),
/// so a plot follows the person. Hex in either case reads as the same key (a key pasted in
/// upper case is the same person). A key that is not hex (a server bot's `bot_` key) is
/// used as it stands, marked so it can never collide with a DID. Here, beside the table,
/// because an account's erasure and export (storage/account.rs) find its rows by it too.
pub fn plot_owner_id(key: &str) -> String {
    match hex::decode(key) {
        Ok(bytes) if !bytes.is_empty() => crate::relay::core::did::did_for_pubkey(&bytes),
        _ => format!("key:{key}"),
    }
}

impl Storage {
    /// The plot `owner` holds on `world` when it is one of `all` (every plot of
    /// the ship, whatever its kind), or else the first of `free_order` (in the
    /// ship file's order) that nobody holds, claimed for them now. None when
    /// none of `free_order` is free: the caller makes them a guest.
    ///
    /// Two lists, because a person asks for a KIND of home (ship homes
    /// increment 6: a join's `start`, docs/design/ship-homes-increment-6-plan.md
    /// section 3.5) and the claim looks only among plots of that kind, while a
    /// plot they already hold is theirs whatever they ask for: asking for a
    /// cabin never gives up the homestead you live on. Moving to another kind
    /// is not built (question 2 of that plan).
    ///
    /// A held row whose plot is no longer in `all` (it left the ship file)
    /// is dropped and the player claims afresh, so the data stays the one
    /// source of truth for which plots exist.
    pub fn claim_plot(&self, world: &str, owner: &str, all: &[&str], free_order: &[&str]) -> Result<Option<String>, rusqlite::Error> {
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
                if all.contains(&id.as_str()) {
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
            for id in free_order {
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

    /// The plot `owner` holds on `world`, None when they hold none. Read only: what a person's
    /// own `game_home_request` is answered with (handlers/home_plots.rs `handle_home`), and only
    /// ever their own row.
    pub fn held_by(&self, world: &str, owner: &str) -> Result<Option<String>, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT plot_id FROM game_plots WHERE world_id = ?1 AND owner_did = ?2",
                params![world, owner],
                |r| r.get(0),
            )
            .optional()
        })
    }

    /// How many of `ids` nobody holds on `world`: a count, never who holds the others (design
    /// section 5.10, and plan 3.11 of increment 6: "the free counts are counts").
    pub fn free_of(&self, world: &str, ids: &[&str]) -> Result<u32, rusqlite::Error> {
        let held: std::collections::HashSet<String> = self.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT plot_id FROM game_plots WHERE world_id = ?1")?;
            let rows = stmt.query_map(params![world], |r| r.get::<_, String>(0))?;
            rows.collect::<Result<_, _>>()
        })?;
        let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
        Ok(unique.iter().filter(|id| !held.contains(**id)).count() as u32)
    }

    /// Give back the plot `owner` holds on `world`: Some(its id) when they
    /// held one, None when they held none. The next player without a plot
    /// claims it. Called when an admin releases a plot (Server Settings,
    /// `game_release_plot`) and when a game gives up a plot its home cannot
    /// stand on (`game_leave` with `give_up_plot`). Nothing releases a plot
    /// on its own: when an idle plot should go back is an open question for
    /// the operator (docs/design/ship-homes-and-logistics.md section 9).
    pub fn release_plot(&self, world: &str, owner: &str) -> Result<Option<String>, rusqlite::Error> {
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            let held: Option<String> = tx
                .query_row(
                    "SELECT plot_id FROM game_plots WHERE world_id = ?1 AND owner_did = ?2",
                    params![world, owner],
                    |r| r.get(0),
                )
                .optional()?;
            tx.execute(
                "DELETE FROM game_plots WHERE world_id = ?1 AND owner_did = ?2",
                params![world, owner],
            )?;
            tx.commit()?;
            Ok(held)
        })
    }

    /// Who holds plot `plot` on `world` (their owner id, `plot_owner_id`), None when nobody
    /// does. For the admin's release by plot id: the relay checks that holder is not in the
    /// world before giving the plot back. Never sent anywhere: no endpoint lists who lives
    /// where (design section 5.10).
    pub fn plot_holder(&self, world: &str, plot: &str) -> Result<Option<String>, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT owner_did FROM game_plots WHERE world_id = ?1 AND plot_id = ?2",
                params![world, plot],
                |r| r.get(0),
            )
            .optional()
        })
    }

    /// Give back plot `plot` on `world`, whoever holds it: true when someone did. The
    /// admin's release by plot id (Server Settings, `game_release_plot` naming a plot), for
    /// a plot whose holder's key nobody has any more: after their account was erased the
    /// key is gone from every list, and a plot id is all an admin can name (the third review
    /// of 1b).
    pub fn release_plot_by_id(&self, world: &str, plot: &str) -> Result<bool, rusqlite::Error> {
        self.with_conn(|conn| {
            let n = conn.execute(
                "DELETE FROM game_plots WHERE world_id = ?1 AND plot_id = ?2",
                params![world, plot],
            )?;
            Ok(n > 0)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db(tag: &str) -> Storage {
        Storage::open_temp(&format!("plots_store_{tag}"))
    }

    const PLOTS: [&str; 2] = ["p1", "p2"];

    #[test]
    fn claims_in_order_keeps_what_it_gave_and_fills_up() {
        let db = temp_db("order");
        assert_eq!(db.claim_plot("ship", "did:hum:a", &PLOTS, &PLOTS).unwrap().as_deref(), Some("p1"));
        assert_eq!(db.claim_plot("ship", "did:hum:b", &PLOTS, &PLOTS).unwrap().as_deref(), Some("p2"));
        // Asked again, each keeps their own.
        assert_eq!(db.claim_plot("ship", "did:hum:b", &PLOTS, &PLOTS).unwrap().as_deref(), Some("p2"));
        assert_eq!(db.claim_plot("ship", "did:hum:a", &PLOTS, &PLOTS).unwrap().as_deref(), Some("p1"));
        // A third player: the ship is full.
        assert_eq!(db.claim_plot("ship", "did:hum:c", &PLOTS, &PLOTS).unwrap(), None);
        // A freed plot goes to the next who asks; releasing names what was freed,
        // and releasing again frees nothing.
        assert_eq!(db.release_plot("ship", "did:hum:a").unwrap().as_deref(), Some("p1"));
        assert_eq!(db.release_plot("ship", "did:hum:a").unwrap(), None);
        assert_eq!(db.claim_plot("ship", "did:hum:c", &PLOTS, &PLOTS).unwrap().as_deref(), Some("p1"));
        // Another ship is another row set.
        assert_eq!(db.claim_plot("other", "did:hum:a", &PLOTS, &PLOTS).unwrap().as_deref(), Some("p1"));
    }

    /// A plot that left the ship file no longer holds anyone: its row is
    /// dropped and the player claims a plot that exists.
    #[test]
    fn a_plot_that_left_the_ship_file_is_dropped() {
        let db = temp_db("left");
        assert_eq!(db.claim_plot("ship", "did:hum:a", &PLOTS, &PLOTS).unwrap().as_deref(), Some("p1"));
        assert_eq!(db.claim_plot("ship", "did:hum:a", &["p2", "p3"], &["p2", "p3"]).unwrap().as_deref(), Some("p2"));
        let rows: i64 = db
            .with_conn(|c| c.query_row("SELECT COUNT(*) FROM game_plots WHERE owner_did = 'did:hum:a'", [], |r| r.get(0)))
            .unwrap();
        assert_eq!(rows, 1, "the old row went");
    }

    /// The table itself refuses a shared plot and a second plot for one
    /// player, whatever the code above does: the constraints are the guard.
    /// Seen red 2026-10-03 with the UNIQUE (world_id, owner_did) line taken
    /// out of the CREATE: "a second plot for one player is refused by the
    /// table" failed (the insert succeeded).
    #[test]
    fn the_table_refuses_a_shared_plot_and_a_second_plot_for_one_player() {
        let db = temp_db("constraints");
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
    }

    /// The ship of these tests: two homesteads and three cabins, in the ship file's order.
    const SHIP: [&str; 5] = ["p1", "p2", "c1", "c2", "c3"];
    const HOMESTEADS: [&str; 2] = ["p1", "p2"];
    const CABINS: [&str; 3] = ["c1", "c2", "c3"];

    /// ASKING FOR A CABIN NEVER GIVES UP THE HOMESTEAD YOU HOLD (ship homes increment 6, plan
    /// 3.5, and section 7's "never cut: keeping a held plot whatever is asked"). Ann holds p2;
    /// her join asks for a cabin and is answered p2, her row unchanged, and no cabin is taken for
    /// her. Bo, holding nothing, asking for a cabin, gets the first cabin though p1 is free; Cy,
    /// asking for a homestead, gets p1. A plot that left the ship file still goes (it is not in
    /// `all`), and a claim then looks only among the kind asked for.
    ///
    /// Seen red 2026-10-12 with the keep checking `free_order` instead of `all` (the kind's ids
    /// as the only list): "assertion `left == right` failed: Ann's homestead p2 was given up when
    /// she asked for a cabin / left: Some(\"c1\") / right: Some(\"p2\")".
    #[test]
    fn a_held_plot_is_kept_whatever_kind_is_asked() {
        let db = temp_db("kept_any_kind");
        let (ann, bo, cy) = ("did:hum:ann", "did:hum:bo", "did:hum:cy");
        db.with_conn(|c| c.execute("INSERT INTO game_plots (world_id, plot_id, owner_did, assigned_at) VALUES ('ship', 'p2', ?1, 0)", params![ann]))
            .expect("Ann holds p2");
        assert_eq!(
            db.claim_plot("ship", ann, &SHIP, &CABINS).unwrap().as_deref(),
            Some("p2"),
            "Ann's homestead p2 was given up when she asked for a cabin"
        );
        assert_eq!(db.held_by("ship", ann).unwrap().as_deref(), Some("p2"), "her row is unchanged");
        assert_eq!(db.free_of("ship", &CABINS).unwrap(), 3, "no cabin was taken for her");
        assert_eq!(db.claim_plot("ship", bo, &SHIP, &CABINS).unwrap().as_deref(), Some("c1"), "Bo asks for a cabin: the first cabin, though p1 is free");
        assert_eq!(db.claim_plot("ship", cy, &SHIP, &HOMESTEADS).unwrap().as_deref(), Some("p1"), "Cy asks for a homestead: p1");
        assert_eq!(db.claim_plot("ship", bo, &SHIP, &HOMESTEADS).unwrap().as_deref(), Some("c1"), "Bo asking for a homestead keeps his cabin");
        // Every homestead is held now: one more asking for a homestead is a guest, and nothing of
        // another kind is handed out in its place.
        assert_eq!(db.claim_plot("ship", "did:hum:dee", &SHIP, &HOMESTEADS).unwrap(), None, "no homestead free: a guest");
        assert_eq!(db.held_by("ship", "did:hum:dee").unwrap(), None);
        // A plot that left the ship file is let go, and the claim looks among the kind asked for.
        assert_eq!(db.claim_plot("ship", ann, &["p1", "c1", "c2", "c3"], &CABINS).unwrap().as_deref(), Some("c2"), "p2 left the ship: Ann gets a cabin");
        assert_eq!(db.plot_holder("ship", "p2").unwrap(), None, "and her old row went");
    }

    /// THE FREE COUNTS COUNT ONLY PLOTS NOBODY HOLDS, on this ship, among the ids asked about: a
    /// homestead held changes no cabin count, a row on another ship changes nothing here, an id
    /// named twice counts once, and an id the table has never seen is free. A count is all it
    /// says (plan 3.11).
    ///
    /// Seen red 2026-10-12 with `free_of` counting every id it was given, held or not
    /// (`unique.len()`): "assertion `left == right` failed: one cabin held, two free / left: 3 /
    /// right: 2".
    #[test]
    fn free_counts_count_only_plots_nobody_holds() {
        let db = temp_db("free_counts");
        assert_eq!(db.free_of("ship", &CABINS).unwrap(), 3, "an empty ship: every cabin free");
        db.claim_plot("ship", "did:hum:a", &SHIP, &CABINS).unwrap();
        assert_eq!(db.free_of("ship", &CABINS).unwrap(), 2, "one cabin held, two free");
        db.claim_plot("ship", "did:hum:b", &SHIP, &HOMESTEADS).unwrap();
        assert_eq!(db.free_of("ship", &CABINS).unwrap(), 2, "a homestead held changes no cabin count");
        assert_eq!(db.free_of("ship", &HOMESTEADS).unwrap(), 1);
        db.claim_plot("other", "did:hum:c", &SHIP, &CABINS).unwrap();
        assert_eq!(db.free_of("ship", &CABINS).unwrap(), 2, "a cabin held on another ship is not held here");
        assert_eq!(db.free_of("ship", &["c2", "c2", "c3"]).unwrap(), 2, "an id named twice counts once");
        assert_eq!(db.free_of("ship", &[]).unwrap(), 0, "no ids, none free");
        db.claim_plot("ship", "did:hum:d", &SHIP, &CABINS).unwrap();
        db.claim_plot("ship", "did:hum:e", &SHIP, &CABINS).unwrap();
        assert_eq!(db.free_of("ship", &CABINS).unwrap(), 0, "every cabin held");
        assert_eq!(db.claim_plot("ship", "did:hum:f", &SHIP, &CABINS).unwrap(), None, "and the next asking for one is a guest");
    }

    /// Many joins at once, from many threads: every player gets a different
    /// plot, never two the same, and the extras are guests.
    #[test]
    fn simultaneous_claims_never_share_a_plot() {
        let db = temp_db("race");
        let db = std::sync::Arc::new(db);
        let plots: Vec<String> = (1..=8).map(|n| format!("p{n}")).collect();
        let handles: Vec<_> = (0..16)
            .map(|i| {
                let db = db.clone();
                let plots = plots.clone();
                std::thread::spawn(move || {
                    let refs: Vec<&str> = plots.iter().map(|s| s.as_str()).collect();
                    db.claim_plot("ship", &format!("did:hum:{i}"), &refs, &refs).unwrap()
                })
            })
            .collect();
        let got: Vec<Option<String>> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let held: Vec<&String> = got.iter().flatten().collect();
        let unique: std::collections::HashSet<&&String> = held.iter().collect();
        assert_eq!(held.len(), 8, "eight plots, eight holders: {got:?}");
        assert_eq!(unique.len(), 8, "no plot given twice: {got:?}");
    }
}
