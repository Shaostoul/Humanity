//! THE FLEET LEDGER's table: what each player used from the fleet and what they gave it
//! (the operator's decision of 2026-10-04 on question 12 of
//! docs/design/ship-homes-and-logistics.md: "track what they player uses and contributes.
//! That way they can be in the red or black").
//!
//! ```text
//! fleet_ledger (id, public_key, kind, direction, item_id, quantity, value,
//!               game_time, real_day, give_id)
//!   INDEX        (public_key, id)                       a player's lines, newest last
//!   UNIQUE INDEX (public_key, give_id) WHERE give_id    a give is recorded once
//! ```
//!
//! One row is one line of a player's ledger: `kind` is an id of data/ship/fleet_ledger.ron
//! ("meal", "item", "power_drawn", "power_returned"), `direction` is "used" or
//! "contributed", copied from the kind when the line is written, as is `value` (CR): a later
//! edit to the data file changes what new lines are worth, never what old ones said.
//! `game_time` is the shared world's clock (game seconds) and `real_day` the coarse real
//! date (unix days, the sealed mailbox's granularity): enough to say when, and no finer.
//!
//! `give_id` is chosen by the game for each give, so a give it sends again (after a
//! reconnect, the answer lost on the way) is recorded once: the unique index refuses the
//! second row and `record_fleet_entry` hands back the first. A line of a "per day" kind
//! (power, a flow that never stops) is added to the player's line of that kind for that
//! real day instead of becoming a new row each report.
//!
//! The relay holds no inventories yet (holdings are increment 8), so what a give says the
//! player handed over is what their game says; the game takes the items out of its own
//! backpack once this table has the line (gui/pages/fleet_ledger.rs).
//!
//! Erasing an account deletes its lines and exporting it lists them (storage/account.rs).

use super::{now_millis, Storage};
use rusqlite::{params, OptionalExtension};

/// Unix days, the coarse real date a line carries.
pub fn real_day_now() -> i64 {
    (now_millis() / 86_400_000) as i64
}

/// One line about to be written.
#[derive(Debug, Clone, PartialEq)]
pub struct NewFleetEntry {
    pub kind: String,
    /// "used" or "contributed".
    pub direction: String,
    /// The item the line is about ("" for a meal or power).
    pub item_id: String,
    pub quantity: f64,
    /// What the line is worth, CR, at the price of the moment.
    pub value: f64,
    pub game_time: f64,
    pub real_day: i64,
    /// The game's id for a give; None for anything else.
    pub give_id: Option<String>,
    /// Add to this player's line of the same kind for the same real day (a flow).
    pub per_day: bool,
}

/// One line as stored.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FleetEntry {
    pub id: i64,
    pub kind: String,
    pub direction: String,
    pub item_id: String,
    pub quantity: f64,
    pub value: f64,
    pub game_time: f64,
    pub real_day: i64,
    pub give_id: Option<String>,
}

/// What writing a line did.
#[derive(Debug, Clone, PartialEq)]
pub enum Recorded {
    /// A new line (or, for a per-day kind, today's line grown by it).
    New(FleetEntry),
    /// A give whose id this player had already recorded: the line from then, unchanged.
    Already(FleetEntry),
}

/// A player's balance, or the whole fleet's: everything given minus everything used, CR.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize)]
pub struct FleetBalance {
    pub used: f64,
    pub contributed: f64,
}

impl FleetBalance {
    /// Given minus used: above 0 is in the black, below 0 in the red.
    pub fn balance(&self) -> f64 {
        self.contributed - self.used
    }

    /// "black", "red" or "even" (within a hundredth of a credit, so rounding noise in
    /// summed prices never reads as a debt).
    pub fn standing(&self) -> &'static str {
        let b = self.balance();
        if b > 0.005 {
            "black"
        } else if b < -0.005 {
            "red"
        } else {
            "even"
        }
    }
}

/// One kind's totals in a player's ledger.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FleetKindTotal {
    pub kind: String,
    pub direction: String,
    pub quantity: f64,
    pub value: f64,
}

/// The whole fleet's totals, for an admin: how many players have a ledger, and their sums.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize)]
pub struct FleetTotals {
    pub players: i64,
    pub used: f64,
    pub contributed: f64,
}

fn map_entry(r: &rusqlite::Row) -> rusqlite::Result<FleetEntry> {
    Ok(FleetEntry {
        id: r.get(0)?,
        kind: r.get(1)?,
        direction: r.get(2)?,
        item_id: r.get(3)?,
        quantity: r.get(4)?,
        value: r.get(5)?,
        game_time: r.get(6)?,
        real_day: r.get(7)?,
        give_id: r.get(8)?,
    })
}

const ENTRY_COLUMNS: &str = "id, kind, direction, item_id, quantity, value, game_time, real_day, give_id";

impl Storage {
    /// Write one line of `key`'s ledger, in one transaction: a give whose id is already
    /// recorded for this key is not written again (`Recorded::Already`); a per-day line is
    /// added to today's line of its kind when there is one.
    pub fn record_fleet_entry(&self, key: &str, e: &NewFleetEntry) -> Result<Recorded, rusqlite::Error> {
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            if let Some(gid) = e.give_id.as_deref() {
                let had = tx
                    .query_row(
                        &format!("SELECT {ENTRY_COLUMNS} FROM fleet_ledger WHERE public_key = ?1 AND give_id = ?2"),
                        params![key, gid],
                        map_entry,
                    )
                    .optional()?;
                if let Some(had) = had {
                    tx.commit()?;
                    return Ok(Recorded::Already(had));
                }
            }
            let today: Option<i64> = if e.per_day {
                tx.query_row(
                    "SELECT id FROM fleet_ledger
                     WHERE public_key = ?1 AND kind = ?2 AND real_day = ?3 AND give_id IS NULL
                     ORDER BY id DESC LIMIT 1",
                    params![key, e.kind, e.real_day],
                    |r| r.get(0),
                )
                .optional()?
            } else {
                None
            };
            let id = match today {
                Some(id) => {
                    tx.execute(
                        "UPDATE fleet_ledger SET quantity = quantity + ?1, value = value + ?2, game_time = ?3 WHERE id = ?4",
                        params![e.quantity, e.value, e.game_time, id],
                    )?;
                    id
                }
                None => {
                    tx.execute(
                        "INSERT INTO fleet_ledger
                            (public_key, kind, direction, item_id, quantity, value, game_time, real_day, give_id)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                        params![key, e.kind, e.direction, e.item_id, e.quantity, e.value, e.game_time, e.real_day, e.give_id],
                    )?;
                    tx.last_insert_rowid()
                }
            };
            let row = tx.query_row(&format!("SELECT {ENTRY_COLUMNS} FROM fleet_ledger WHERE id = ?1"), params![id], map_entry)?;
            tx.commit()?;
            Ok(Recorded::New(row))
        })
    }

    /// The line `key`'s give `give_id` wrote, if it was recorded.
    pub fn fleet_give_of(&self, key: &str, give_id: &str) -> Result<Option<FleetEntry>, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                &format!("SELECT {ENTRY_COLUMNS} FROM fleet_ledger WHERE public_key = ?1 AND give_id = ?2"),
                params![key, give_id],
                map_entry,
            )
            .optional()
        })
    }

    /// `key`'s balance, its totals per kind, and its newest `recent` lines (newest first).
    /// Only ever `key`'s own: there is no way here to read another player's.
    pub fn fleet_ledger_of(
        &self,
        key: &str,
        recent: usize,
    ) -> Result<(FleetBalance, Vec<FleetKindTotal>, Vec<FleetEntry>), rusqlite::Error> {
        self.with_conn(|conn| {
            let mut kinds = Vec::new();
            {
                let mut stmt = conn.prepare(
                    "SELECT kind, direction, SUM(quantity), SUM(value) FROM fleet_ledger
                     WHERE public_key = ?1 GROUP BY kind, direction ORDER BY MIN(id)",
                )?;
                let rows = stmt.query_map(params![key], |r| {
                    Ok(FleetKindTotal { kind: r.get(0)?, direction: r.get(1)?, quantity: r.get(2)?, value: r.get(3)? })
                })?;
                for r in rows {
                    kinds.push(r?);
                }
            }
            let mut bal = FleetBalance::default();
            for k in &kinds {
                match k.direction.as_str() {
                    "used" => bal.used += k.value,
                    "contributed" => bal.contributed += k.value,
                    _ => {}
                }
            }
            let mut lines = Vec::new();
            {
                let mut stmt = conn.prepare(&format!(
                    "SELECT {ENTRY_COLUMNS} FROM fleet_ledger WHERE public_key = ?1 ORDER BY id DESC LIMIT ?2"
                ))?;
                for r in stmt.query_map(params![key, recent as i64], map_entry)? {
                    lines.push(r?);
                }
            }
            Ok((bal, kinds, lines))
        })
    }

    /// The whole fleet's totals over every player (an admin's view: sums only, no names).
    pub fn fleet_totals(&self) -> Result<FleetTotals, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT COUNT(DISTINCT public_key),
                        COALESCE(SUM(CASE WHEN direction = 'used' THEN value END), 0.0),
                        COALESCE(SUM(CASE WHEN direction = 'contributed' THEN value END), 0.0)
                 FROM fleet_ledger",
                [],
                |r| Ok(FleetTotals { players: r.get(0)?, used: r.get(1)?, contributed: r.get(2)? }),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db(tag: &str) -> (Storage, std::path::PathBuf) {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_fleet_{tag}_{}_{nanos}.db", std::process::id()));
        (Storage::open(&path).expect("open test db"), path)
    }

    fn line(kind: &str, direction: &str, value: f64) -> NewFleetEntry {
        NewFleetEntry {
            kind: kind.into(),
            direction: direction.into(),
            item_id: String::new(),
            quantity: 1.0,
            value,
            game_time: 100.0,
            real_day: 20_000,
            give_id: None,
            per_day: false,
        }
    }

    /// THE BALANCE'S SIGN: given minus used, in the black above 0, in the red below, even at
    /// 0 (and within a hundredth of a credit, so summed prices never show a debt of noise).
    ///
    /// Seen red 2026-10-04 with `balance` written used - contributed: "used 30, gave 12: in
    /// the red / left: \"black\" / right: \"red\"".
    #[test]
    fn the_balance_is_in_the_black_or_the_red() {
        let b = FleetBalance { used: 30.0, contributed: 12.0 };
        assert_eq!(b.standing(), "red", "used 30, gave 12: in the red");
        assert_eq!(b.balance(), -18.0);
        assert_eq!(FleetBalance { used: 3.0, contributed: 10.0 }.standing(), "black");
        assert_eq!(FleetBalance { used: 3.0, contributed: 3.0 }.standing(), "even");
        assert_eq!(FleetBalance { used: 0.1 + 0.2, contributed: 0.3 }.standing(), "even", "no debt of rounding noise");
        assert_eq!(FleetBalance::default().standing(), "even");
    }

    /// ONE PLAYER'S LINES ARE ONLY THEIRS: two players' lines in one table, and each one's
    /// ledger holds its own and sums its own.
    ///
    /// Seen red 2026-10-04 with the key left out of the per-kind query's WHERE: "B's ledger
    /// holds only B's lines: B used 10, gave 0 / left: 50 / right: 10.0".
    #[test]
    fn one_players_ledger_holds_only_their_lines() {
        let (db, path) = temp_db("own");
        db.record_fleet_entry("aaaa", &line("meal", "used", 10.0)).unwrap();
        db.record_fleet_entry("aaaa", &line("item", "contributed", 30.0)).unwrap();
        db.record_fleet_entry("bbbb", &line("meal", "used", 10.0)).unwrap();
        let (bal, kinds, lines) = db.fleet_ledger_of("bbbb", 20).unwrap();
        assert_eq!(bal.used + bal.contributed, 10.0, "B's ledger holds only B's lines: B used 10, gave 0 / left: {} / right: 10.0", bal.used + bal.contributed);
        assert_eq!(lines.len(), 1);
        assert_eq!(kinds.len(), 1);
        let (bal, _, lines) = db.fleet_ledger_of("aaaa", 20).unwrap();
        assert_eq!((bal.used, bal.contributed, bal.standing()), (10.0, 30.0, "black"));
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].kind, "item", "newest first");
        let t = db.fleet_totals().unwrap();
        assert_eq!((t.players, t.used, t.contributed), (2, 20.0, 30.0));
        drop(db);
        let _ = std::fs::remove_file(&path);
    }

    /// A GIVE IS RECORDED ONCE: the same give id sent twice is one line, and the second
    /// write hands back the first; another player may use the same id.
    ///
    /// Seen red 2026-10-04 with the give-id lookup removed (the unique index then refused
    /// the second row, so the write failed instead of answering): "the second write of a
    /// give answers with the first: Err(SqliteFailure(Error { code: ConstraintViolation,
    /// extended_code: 2067 }, Some(\"UNIQUE constraint failed: fleet_ledger.public_key,
    /// fleet_ledger.give_id\")))".
    #[test]
    fn a_give_is_recorded_once() {
        let (db, path) = temp_db("give_once");
        let mut give = line("item", "contributed", 6.0);
        give.item_id = "bread_0".into();
        give.quantity = 2.0;
        give.give_id = Some("g-1".into());
        let first = match db.record_fleet_entry("aaaa", &give).unwrap() {
            Recorded::New(e) => e,
            other => panic!("the first write is new: {other:?}"),
        };
        let again = db.record_fleet_entry("aaaa", &give);
        assert!(matches!(again, Ok(Recorded::Already(ref e)) if *e == first), "the second write of a give answers with the first: {again:?}");
        assert!(matches!(db.record_fleet_entry("bbbb", &give), Ok(Recorded::New(_))), "another player's give with the same id is theirs");
        assert_eq!(db.fleet_ledger_of("aaaa", 20).unwrap().2.len(), 1, "one line");
        drop(db);
        let _ = std::fs::remove_file(&path);
    }

    /// A PER-DAY KIND (power) IS ONE LINE A DAY: reports on one real day grow that day's
    /// line, a new day starts a new one, and a meal is never merged.
    ///
    /// Seen red 2026-10-04 with `per_day` ignored: "three power reports on one day are one
    /// line / left: 3 / right: 1".
    #[test]
    fn a_flow_is_one_line_a_day() {
        let (db, path) = temp_db("per_day");
        let mut p = line("power_drawn", "used", 1.5);
        p.per_day = true;
        for _ in 0..3 {
            db.record_fleet_entry("aaaa", &p).unwrap();
        }
        let lines = db.fleet_ledger_of("aaaa", 20).unwrap().2;
        assert_eq!(lines.len(), 1, "three power reports on one day are one line / left: {} / right: 1", lines.len());
        assert_eq!((lines[0].quantity, lines[0].value), (3.0, 4.5));
        p.real_day += 1;
        db.record_fleet_entry("aaaa", &p).unwrap();
        db.record_fleet_entry("aaaa", &line("meal", "used", 10.0)).unwrap();
        db.record_fleet_entry("aaaa", &line("meal", "used", 10.0)).unwrap();
        assert_eq!(db.fleet_ledger_of("aaaa", 20).unwrap().2.len(), 4, "a new day, and two meals");
        drop(db);
        let _ = std::fs::remove_file(&path);
    }
}
