//! The building pieces the server keeps in its shared world (ship homes increment 5, "building
//! only on your own plot", 2026-10-05; docs/design/ship-homes-increment-5-plan.md section 3.3).
//!
//! In a server's shared world, the shell pieces a player builds aboard (the blueprints marked
//! `shared: true` in data/blueprints/basic.ron: foundations, walls, the window wall, the roof)
//! are kept here instead of in the player's own save, so everyone near sees them and they are
//! still there after the relay restarts. One row is one piece:
//!
//! ```text
//! world_pieces (piece_id, world_id, frame, blueprint_id,
//!               pos_x, pos_y, pos_z, rot_x, rot_y, rot_z, rot_w, scale_x, scale_y, scale_z,
//!               owner_did, placed_at, state_json)
//!   piece_id INTEGER PRIMARY KEY AUTOINCREMENT     a number is never given to a second piece
//! ```
//!
//! - `world_id` is the ship's id from the ship file, as `game_plots` keeps it, so a later fleet
//!   of ships keeps one set of pieces per ship.
//! - `frame` is where the piece is kept (src/ship/build_frames.rs): a plot, `plot:p3`, or one of
//!   the ship's shared spaces, `zone:commons`. The pose is measured from that frame's corner.
//! - The pose columns hold what the relay kept after its checks
//!   (systems/construction/shared.rs `pose_in_frame`): the canonical pose, on the grid, turned a
//!   whole quarter. A REAL holds an f32 exactly, so a piece comes back to the bit.
//! - `owner_did` is the builder's `did:hum:` (`plot_owner_id`, the id plots are held under). It
//!   answers who may take the piece down, and it is how an erase finds the account's pieces. It
//!   is NEVER sent anywhere: nobody is told who built what (shared.rs, after increment 4's rule
//!   that nobody is told who lives where). [`StoredPiece::to_piece_for`], the one way from a row
//!   to the wire, tells a player only whether the piece is theirs.
//! - `placed_at` is when the relay accepted the piece, Unix MILLISECONDS on the relay's clock
//!   (the unit every time in this database is kept in); the wire carries it as seconds.
//! - `state_json` is room for a piece's own state later (wear, a door left open), `{}` today:
//!   nothing writes it and nothing reads it yet.
//!
//! The relay keeps its own copy of the pieces in memory and writes each change here BEFORE it
//! tells anyone, so a piece nobody could get back after a restart is never shown. There is no
//! index: a ship holds at most `shared::MAX_PIECES_PER_WORLD` (4,096), read whole when the relay
//! starts, and every other statement here finds one row by its number or deletes a group.
//!
//! Erasing an account deletes every piece it built, on every ship, and exporting it lists them
//! (storage/account.rs, found under its `did:hum:` like its plot).

use super::Storage;
use crate::systems::construction::shared::Piece;
use rusqlite::params;

/// One piece about to be kept: what the relay checked and accepted, in its frame's terms.
#[derive(Debug, Clone, PartialEq)]
pub struct NewPiece {
    /// The ship's id (the ship file's), as `game_plots` keeps it.
    pub world_id: String,
    /// The frame it is kept in: `plot:p3`, `zone:commons` (src/ship/build_frames.rs).
    pub frame: String,
    /// Which blueprint (data/blueprints/basic.ron).
    pub blueprint_id: String,
    /// Metres from the frame's corner, y up: the bottom centre of the piece's box.
    pub position: [f32; 3],
    /// Its turn as a quaternion (x, y, z, w).
    pub rotation: [f32; 4],
    /// Its size, metres.
    pub scale: [f32; 3],
    /// Who built it: their id as plots are held under it (`plot_owner_id`). Never sent anywhere.
    pub owner_did: String,
    /// When the relay accepted it, Unix milliseconds on the relay's clock.
    pub placed_at_ms: i64,
}

impl NewPiece {
    /// This piece as it is kept once the database gave it `piece_id`
    /// ([`Storage::insert_world_piece`]), for the relay's own copy in memory.
    pub fn kept_as(self, piece_id: u64) -> StoredPiece {
        StoredPiece {
            piece_id,
            world_id: self.world_id,
            frame: self.frame,
            blueprint_id: self.blueprint_id,
            position: self.position,
            rotation: self.rotation,
            scale: self.scale,
            owner_did: self.owner_did,
            placed_at_ms: self.placed_at_ms,
        }
    }
}

/// One piece as kept.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredPiece {
    /// Its number: never given to another piece, even after this one is taken down or the
    /// relay restarts (AUTOINCREMENT).
    pub piece_id: u64,
    pub world_id: String,
    pub frame: String,
    pub blueprint_id: String,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    /// Who built it. Never sent anywhere (see the module doc).
    pub owner_did: String,
    /// When the relay accepted it, Unix milliseconds on the relay's clock.
    pub placed_at_ms: i64,
}

impl StoredPiece {
    /// This piece as it travels to the player whose id (as plots are held under it,
    /// `plot_owner_id`) is `viewer_did`: `mine` when they built it, and nothing else about who
    /// did. The time goes as seconds, the wire's unit.
    pub fn to_piece_for(&self, viewer_did: &str) -> Piece {
        Piece {
            piece_id: self.piece_id,
            blueprint_id: self.blueprint_id.clone(),
            position: self.position,
            rotation: self.rotation,
            scale: self.scale,
            placed_at: self.placed_at_ms as f64 / 1000.0,
            mine: !viewer_did.is_empty() && viewer_did == self.owner_did,
        }
    }
}

/// A piece's number as SQLite keeps it. None for a number no piece can have (past i64::MAX:
/// a game can send any u64).
fn row_id(piece_id: u64) -> Option<i64> {
    i64::try_from(piece_id).ok()
}

impl Storage {
    /// Keep a new piece. Returns its number, never given to another piece (AUTOINCREMENT).
    pub fn insert_world_piece(&self, p: &NewPiece) -> Result<u64, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO world_pieces (world_id, frame, blueprint_id,
                     pos_x, pos_y, pos_z, rot_x, rot_y, rot_z, rot_w, scale_x, scale_y, scale_z,
                     owner_did, placed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                params![
                    p.world_id,
                    p.frame,
                    p.blueprint_id,
                    p.position[0] as f64,
                    p.position[1] as f64,
                    p.position[2] as f64,
                    p.rotation[0] as f64,
                    p.rotation[1] as f64,
                    p.rotation[2] as f64,
                    p.rotation[3] as f64,
                    p.scale[0] as f64,
                    p.scale[1] as f64,
                    p.scale[2] as f64,
                    p.owner_did,
                    p.placed_at_ms,
                ],
            )?;
            // The writer connection that did the INSERT, so this is that row's number.
            Ok(conn.last_insert_rowid() as u64)
        })
    }

    /// Take down piece `piece_id` of `world`: true when it was there.
    pub fn delete_world_piece(&self, world: &str, piece_id: u64) -> Result<bool, rusqlite::Error> {
        let Some(id) = row_id(piece_id) else { return Ok(false) };
        self.with_conn(|conn| {
            let n = conn.execute("DELETE FROM world_pieces WHERE world_id = ?1 AND piece_id = ?2", params![world, id])?;
            Ok(n > 0)
        })
    }

    /// Every piece kept on `world`, oldest first (by number). What the relay loads when it
    /// starts. A piece whose frame has left the ship file is still returned (the relay skips
    /// it and keeps the row, so putting the frame back brings it back). A row that cannot be
    /// read is left out with an error in the log, so one damaged row never hides the rest.
    pub fn load_world_pieces(&self, world: &str) -> Result<Vec<StoredPiece>, rusqlite::Error> {
        self.with_read_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT piece_id, world_id, frame, blueprint_id,
                        pos_x, pos_y, pos_z, rot_x, rot_y, rot_z, rot_w, scale_x, scale_y, scale_z,
                        owner_did, placed_at
                 FROM world_pieces WHERE world_id = ?1 ORDER BY piece_id ASC",
            )?;
            let rows = stmt.query_map(params![world], |r| {
                let f = |i: usize| -> Result<f32, rusqlite::Error> { Ok(r.get::<_, f64>(i)? as f32) };
                Ok(StoredPiece {
                    piece_id: r.get::<_, i64>(0)? as u64,
                    world_id: r.get(1)?,
                    frame: r.get(2)?,
                    blueprint_id: r.get(3)?,
                    position: [f(4)?, f(5)?, f(6)?],
                    rotation: [f(7)?, f(8)?, f(9)?, f(10)?],
                    scale: [f(11)?, f(12)?, f(13)?],
                    owner_did: r.get(14)?,
                    placed_at_ms: r.get(15)?,
                })
            })?;
            Ok(rows
                .filter_map(|row| match row {
                    Ok(piece) => Some(piece),
                    Err(e) => {
                        tracing::error!("world_pieces: a piece of {world} could not be read, left out: {e}");
                        None
                    }
                })
                .collect())
        })
    }

    /// Take down every piece in frame `frame` of `world`: how many went. For a plot given back
    /// (an admin's release, a home that does not fit, an erased account): its pieces come down
    /// with it, so the next household never moves in among a stranger's walls (decision 2 of
    /// the increment 5 plan).
    pub fn delete_world_pieces_in_frame(&self, world: &str, frame: &str) -> Result<usize, rusqlite::Error> {
        self.with_conn(|conn| conn.execute("DELETE FROM world_pieces WHERE world_id = ?1 AND frame = ?2", params![world, frame]))
    }

    /// Take down every piece `owner_did` built, on every ship: how many went. An account's erase
    /// does this itself, inside its one pass over the tables (storage/account.rs
    /// `delete_account`); this is the same delete for any other caller.
    pub fn delete_world_pieces_of_owner(&self, owner_did: &str) -> Result<usize, rusqlite::Error> {
        self.with_conn(|conn| conn.execute("DELETE FROM world_pieces WHERE owner_did = ?1", params![owner_did]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A wood wall on p1 of the first ship, built by `owner`, turned a quarter (x and w of the
    /// quaternion are 0.70710677, a number with no short decimal form, to prove the pose comes
    /// back to the bit).
    fn wall(world: &str, frame: &str, owner: &str) -> NewPiece {
        let s = std::f32::consts::FRAC_1_SQRT_2;
        NewPiece {
            world_id: world.into(),
            frame: frame.into(),
            blueprint_id: "wood_wall".into(),
            position: [48.5, 0.1, 36.0],
            rotation: [0.0, s, 0.0, s],
            scale: [2.0, 3.1, 0.2],
            owner_did: owner.into(),
            placed_at_ms: 1_791_000_000_123,
        }
    }

    fn ids(db: &Storage, world: &str) -> Vec<u64> {
        db.load_world_pieces(world).unwrap().iter().map(|p| p.piece_id).collect()
    }

    /// A PIECE COMES BACK EXACTLY AS IT WAS KEPT, and A NUMBER IS NEVER GIVEN TO A SECOND PIECE:
    /// every field returns (the pose to the bit), and after the newest piece is taken down and
    /// the relay restarts, the next piece gets a higher number than any piece ever had. A game
    /// still holding a taken-down piece's number (its take-down crossed the new build on the
    /// way) must never find that number on another piece.
    ///
    /// Seen red 2026-10-05 with AUTOINCREMENT dropped from the CREATE in storage/mod.rs: "a
    /// taken-down piece's number is never given again / left: 2 / right: more than 2".
    #[test]
    fn pieces_round_trip_and_ids_are_never_reused() {
        let path = crate::test_temp::db("world_pieces_ids");
        let first_wall = wall("mothership-1", "plot:p1", "did:hum:ann");
        let second_wall = NewPiece { position: [50.5, 0.1, 36.0], ..first_wall.clone() };
        let (first, second) = {
            let db = Storage::open(&path).expect("open");
            let first = db.insert_world_piece(&first_wall).unwrap();
            let second = db.insert_world_piece(&second_wall).unwrap();
            assert!(second > first, "numbers go up: {first}, then {second}");
            let kept = db.load_world_pieces("mothership-1").unwrap();
            assert_eq!(kept, vec![first_wall.clone().kept_as(first), second_wall.clone().kept_as(second)], "every field comes back");
            let s = std::f32::consts::FRAC_1_SQRT_2;
            assert_eq!(kept[0].rotation[1].to_bits(), s.to_bits(), "the turn comes back to the bit");
            assert!(db.delete_world_piece("mothership-1", second).unwrap(), "the newest piece is taken down");
            assert!(!db.delete_world_piece("mothership-1", second).unwrap(), "and is not there to take down again");
            assert!(!db.delete_world_piece("mothership-1", u64::MAX).unwrap(), "a number no piece can have is no piece");
            (first, second)
        };
        // The relay restarts.
        let db = Storage::open(&path).expect("reopen");
        assert_eq!(ids(&db, "mothership-1"), vec![first], "the piece still standing is still there");
        let third = db.insert_world_piece(&second_wall).unwrap();
        assert!(third > second, "a taken-down piece's number is never given again / left: {third} / right: more than {second}");
    }

    /// WHAT STANDS ON A PLOT GOES WITH THE PLOT, AND WHAT AN ACCOUNT BUILT GOES WITH THE
    /// ACCOUNT: giving back p1 of one ship takes down every piece on it, whoever built it, and
    /// nothing else (not p1 of another ship, not the next plot); erasing an account takes down
    /// what it built on every ship, and nobody else's pieces.
    ///
    /// Seen red 2026-10-05 with the WHERE on world_id removed from
    /// `delete_world_pieces_in_frame`: "giving back p1 of one ship takes down the three pieces on
    /// it / left: 4 / right: 3" (p1 of the other ship went too).
    #[test]
    fn pieces_of_a_frame_and_of_an_owner_go_together() {
        let db = Storage::open_temp("world_pieces_groups");
        let (one, two) = ("mothership-1", "mothership-2");
        let insert = |world: &str, frame: &str, owner: &str| db.insert_world_piece(&wall(world, frame, owner)).unwrap();
        let ann_1 = insert(one, "plot:p1", "did:hum:ann");
        let ann_2 = insert(one, "plot:p1", "did:hum:ann");
        // Bea's piece on Ann's plot (a household permit), and one on her own.
        let bea_on_ann = insert(one, "plot:p1", "did:hum:bea");
        let bea_own = insert(one, "plot:p2", "did:hum:bea");
        // Ann holds p1 on the second ship too; Cal built in its Commons.
        let ann_other_ship = insert(two, "plot:p1", "did:hum:ann");
        let cal = insert(two, "zone:commons", "did:hum:cal");
        assert_eq!(ids(&db, one), vec![ann_1, ann_2, bea_on_ann, bea_own]);

        let went = db.delete_world_pieces_in_frame(one, "plot:p1").unwrap();
        assert_eq!(went, 3, "giving back p1 of one ship takes down the three pieces on it / left: {went} / right: 3");
        assert_eq!(ids(&db, one), vec![bea_own], "the next plot keeps its piece");
        assert_eq!(ids(&db, two), vec![ann_other_ship, cal], "p1 of the other ship keeps its piece");

        let went = db.delete_world_pieces_of_owner("did:hum:ann").unwrap();
        assert_eq!(went, 1, "Ann's erase takes her last piece, on the other ship");
        assert_eq!(ids(&db, two), vec![cal], "Cal's piece stays");
        assert_eq!(ids(&db, one), vec![bea_own], "Bea's piece stays");
    }

    /// A DATABASE THE PREVIOUS CODE WROTE OPENS (the BUG-046 rule). tests/fixtures/relay/
    /// relay_v0_1456.sql is the database the code before the fleet ledger wrote (see
    /// relay/handlers/fleet_ledger_tests.rs for how it was made); its `roles` table has no
    /// `can_edit_ship` and it has no `world_pieces`. Opened by this code: the built-in Admin role
    /// has the ship-editing rank and Verified does not, and `world_pieces` is there and keeps a
    /// piece.
    ///
    /// Seen red 2026-10-05 with the guarded ALTER for can_edit_ship switched off: "the previous
    /// code's database opens: SqliteFailure(Error { code: Unknown, extended_code: 1 },
    /// Some(\"table roles has no column named can_edit_ship\"))" (the seed's INSERT names it).
    #[test]
    fn a_database_from_before_the_pieces_opens_and_admin_has_the_ship_rank() {
        let path = crate::test_temp::db("world_pieces_prev");
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.execute_batch(include_str!("../../../tests/fixtures/relay/relay_v0_1456.sql")).expect("the previous code's database loads from its dump");
            let has: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name = 'world_pieces')", [], |r| r.get(0)).unwrap();
            assert!(!has, "the fixture is from before the pieces");
            assert!(conn.prepare("SELECT can_edit_ship FROM roles LIMIT 0").is_err(), "and before the rank");
        }
        let db = Storage::open(&path).unwrap_or_else(|e| panic!("the previous code's database opens: {e:?}"));
        assert!(db.role_def("admin").can_edit_ship, "the built-in Admin role has the ship-editing rank");
        assert!(!db.role_def("verified").can_edit_ship, "and Verified does not");
        let id = db.insert_world_piece(&wall("mothership-1", "plot:p1", "did:hum:ann")).expect("world_pieces takes a piece");
        assert_eq!(ids(&db, "mothership-1"), vec![id], "and keeps it");
    }
}
