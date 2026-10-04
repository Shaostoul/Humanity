//! THE FLEET LEDGER's table: what each player used from the fleet and what they gave it
//! (the operator's decision of 2026-10-04 on question 12 of
//! docs/design/ship-homes-and-logistics.md: "track what they player uses and contributes.
//! That way they can be in the red or black").
//!
//! ```text
//! fleet_ledger (id, public_key, kind, direction, item_id, quantity, value,
//!               game_time, real_day, give_id, home, adjusted)
//!   INDEX        (public_key, id)                       a player's lines, newest last
//!   UNIQUE INDEX (public_key, give_id) WHERE give_id    a give is recorded once
//! ```
//!
//! One row is one line of a player's ledger: `kind` is an id of data/ship/fleet_ledger.ron
//! ("meal", "item", "item_creative", "power_drawn", "power_returned"), `direction` is "used"
//! or "contributed", copied from the kind when the line is written, as is `value` (CR): a
//! later edit to the data file changes what new lines are worth, never what old ones said.
//!
//! WHEN, AND HOW FINELY (the review of 2026-10-04, finding 10). `real_day` is the real date
//! (unix days, the sealed mailbox's granularity). `game_time` is the shared world's clock
//! ROUNDED DOWN TO THE START OF ITS GAME DAY (handlers/fleet_ledger.rs `LedgerData::line`), so
//! two lines written in one game day carry the same time. That is still finer than a real
//! day: at the default clock of 72 game seconds to the real second a game day is 20 real
//! minutes, so anyone holding this file can say when a line was written to within about 20
//! minutes (at a slower clock, less finely: at 1x a game day is a real day). It is no longer
//! the minute and second the player ate, which the unrounded clock gave away.
//!
//! `give_id` is chosen by the game for each give, so a give it sends again (after a
//! reconnect, the answer lost on the way) is recorded once: the unique index refuses the
//! second row and `record_fleet_entry` hands back the first. `home` is the id the game keeps
//! with the backpack the give came out of (saved with it), so the game can ask for the gives
//! of ITS home and settle again any its loaded save has not (`fleet_gives_for_home`); no other
//! home's gives are ever handed back to it. `adjusted` is set once, when the game said fewer
//! of a give's items were there to take than the give claimed (`adjust_fleet_give`).
//!
//! A line of a "per day" kind (power, a flow that never stops) is added to the player's line
//! of that kind for that real day instead of becoming a new row each report. A kind with a
//! daily cap (gives) refuses a line past it (`Recorded::OverCap`, nothing written), so no game
//! can write rows without end (finding 11).
//!
//! The relay holds no inventories yet (holdings are increment 8), so what a give says the
//! player handed over is what their game says; the game takes the items out of its own
//! backpack when it sends the give (engine/fleet.rs).
//!
//! Erasing an account deletes its lines and exporting it lists them (storage/account.rs).

use super::{now_millis, Storage};
use rusqlite::{params, OptionalExtension};

/// Unix days, the coarse real date a line carries.
pub fn real_day_now() -> i64 {
    (now_millis() / 86_400_000) as i64
}

/// One line about to be written.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NewFleetEntry {
    pub kind: String,
    /// "used" or "contributed".
    pub direction: String,
    /// The item the line is about ("" for a meal or power).
    pub item_id: String,
    pub quantity: f64,
    /// What the line is worth, CR, at the price of the moment.
    pub value: f64,
    /// The shared world's clock at the start of the game day (see the module doc).
    pub game_time: f64,
    pub real_day: i64,
    /// The game's id for a give; None for anything else.
    pub give_id: Option<String>,
    /// The home a give came from (the game's id for it); "" for anything else.
    pub home: String,
    /// Add to this player's line of the same kind for the same real day (a flow).
    pub per_day: bool,
    /// The most lines of this kind one player may write in one real day; None for no cap.
    pub day_cap: Option<u32>,
}

/// One line as stored.
#[derive(Debug, Clone, PartialEq)]
pub struct FleetEntry {
    /// The row's number. Shared by every player's lines, so it is never sent to a game: the
    /// gap between two of one player's numbers would say how many lines everyone else wrote
    /// in between (finding 8 of the 2026-10-04 review).
    pub id: i64,
    pub kind: String,
    pub direction: String,
    pub item_id: String,
    pub quantity: f64,
    pub value: f64,
    pub game_time: f64,
    pub real_day: i64,
    pub give_id: Option<String>,
    pub home: String,
}

/// What writing a line did.
#[derive(Debug, Clone, PartialEq)]
pub enum Recorded {
    /// A new line (or, for a per-day kind, today's line grown by it).
    New(FleetEntry),
    /// A give whose id this player had already recorded: the line from then, unchanged.
    Already(FleetEntry),
    /// Past this kind's daily cap: nothing was written.
    OverCap,
}

/// A give as the game needs it to settle one again (`fleet_gives_for_home`).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FleetGiveRecord {
    pub give_id: String,
    pub item_id: String,
    /// How many the fleet counts: the give's quantity, or fewer once the game said fewer
    /// were there to take (`adjust_fleet_give`).
    pub quantity: f64,
}

/// What `adjust_fleet_give` did.
#[derive(Debug, Clone, PartialEq)]
pub enum Adjusted {
    /// The line now counts fewer, at the same price each.
    Changed(FleetEntry),
    /// Nothing changed: the line was adjusted before, or the game said as many or more were
    /// there (a correction only ever lowers a give, once).
    Unchanged(FleetEntry),
    /// This player recorded no give with that id.
    NotFound,
}

/// A player's balance, or the whole fleet's: everything given minus everything used, CR.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize)]
pub struct FleetBalance {
    pub used: f64,
    pub contributed: f64,
}

/// How far from even a balance must be before it reads as in the black or in the red: the
/// panel shows credits to one decimal, so a difference it would print as 0 CR is even here
/// too (finding 17 of the 2026-10-04 review: a balance of +0.02 CR read "In the black: you
/// have given the fleet 0 CR more than you have used from it.").
pub const STANDING_THRESHOLD_CR: f64 = 0.05;

impl FleetBalance {
    /// Given minus used: above 0 is in the black, below 0 in the red.
    pub fn balance(&self) -> f64 {
        self.contributed - self.used
    }

    /// "black", "red" or "even" (within `STANDING_THRESHOLD_CR`, so rounding noise in summed
    /// prices never reads as a debt or a credit the panel would print as 0).
    pub fn standing(&self) -> &'static str {
        let b = self.balance();
        if b > STANDING_THRESHOLD_CR {
            "black"
        } else if b < -STANDING_THRESHOLD_CR {
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

/// The whole fleet's totals, for an admin: how many players have a ledger, how many of them
/// are someone other than the admin asking, and their sums.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize)]
pub struct FleetTotals {
    pub players: i64,
    pub others: i64,
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
        home: r.get(9)?,
    })
}

const ENTRY_COLUMNS: &str = "id, kind, direction, item_id, quantity, value, game_time, real_day, give_id, home";

impl Storage {
    /// Write one line of `key`'s ledger, in one transaction: a give whose id is already
    /// recorded for this key is not written again (`Recorded::Already`); a line past its
    /// kind's daily cap is not written (`Recorded::OverCap`); a per-day line is added to
    /// today's line of its kind when there is one.
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
            if let Some(cap) = e.day_cap {
                let today: i64 = tx.query_row(
                    "SELECT COUNT(*) FROM fleet_ledger WHERE public_key = ?1 AND kind = ?2 AND real_day = ?3",
                    params![key, e.kind, e.real_day],
                    |r| r.get(0),
                )?;
                if today >= i64::from(cap) {
                    tx.commit()?;
                    return Ok(Recorded::OverCap);
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
                            (public_key, kind, direction, item_id, quantity, value, game_time, real_day, give_id, home)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                        params![key, e.kind, e.direction, e.item_id, e.quantity, e.value, e.game_time, e.real_day, e.give_id, e.home],
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

    /// `key`'s newest `limit` gives from the home `home` (newest first): what the game needs
    /// to settle again a give its loaded save does not list (a crash before the save, or a
    /// snapshot from before the give put back). Only that player's, and only that home's.
    pub fn fleet_gives_for_home(&self, key: &str, home: &str, limit: usize) -> Result<Vec<FleetGiveRecord>, rusqlite::Error> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT give_id, item_id, quantity FROM fleet_ledger
                 WHERE public_key = ?1 AND home = ?2 AND give_id IS NOT NULL
                 ORDER BY id DESC LIMIT ?3",
            )?;
            let rows = stmt.query_map(params![key, home, limit as i64], |r| {
                Ok(FleetGiveRecord { give_id: r.get(0)?, item_id: r.get(1)?, quantity: r.get(2)? })
            })?;
            rows.collect()
        })
    }

    /// The game found only `delivered` of give `give_id`'s items to take (they had left the
    /// backpack some other way first): count the give as that many, at the same price each,
    /// ONCE. A correction only ever lowers a give; a second one, or one claiming as many or
    /// more, changes nothing.
    pub fn adjust_fleet_give(&self, key: &str, give_id: &str, delivered: f64) -> Result<Adjusted, rusqlite::Error> {
        self.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            let found = tx
                .query_row(
                    &format!("SELECT {ENTRY_COLUMNS}, adjusted FROM fleet_ledger WHERE public_key = ?1 AND give_id = ?2"),
                    params![key, give_id],
                    |r| Ok((map_entry(r)?, r.get::<_, bool>(10)?)),
                )
                .optional()?;
            let Some((line, adjusted)) = found else {
                tx.commit()?;
                return Ok(Adjusted::NotFound);
            };
            let delivered = delivered.max(0.0);
            if adjusted || !delivered.is_finite() || delivered >= line.quantity {
                tx.commit()?;
                return Ok(Adjusted::Unchanged(line));
            }
            let each = if line.quantity > 0.0 { line.value / line.quantity } else { 0.0 };
            tx.execute(
                "UPDATE fleet_ledger SET quantity = ?1, value = ?2, adjusted = 1 WHERE id = ?3",
                params![delivered, each * delivered, line.id],
            )?;
            let row = tx.query_row(&format!("SELECT {ENTRY_COLUMNS} FROM fleet_ledger WHERE id = ?1"), params![line.id], map_entry)?;
            tx.commit()?;
            Ok(Adjusted::Changed(row))
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

    /// The whole fleet's totals over every player (an admin's view: sums only, no names), and
    /// how many of the players are someone other than `asker` (the handler holds the sums back
    /// when that is too few to hide any one person in them).
    pub fn fleet_totals(&self, asker: &str) -> Result<FleetTotals, rusqlite::Error> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT COUNT(DISTINCT public_key),
                        COUNT(DISTINCT CASE WHEN public_key != ?1 THEN public_key END),
                        COALESCE(SUM(CASE WHEN direction = 'used' THEN value END), 0.0),
                        COALESCE(SUM(CASE WHEN direction = 'contributed' THEN value END), 0.0)
                 FROM fleet_ledger",
                params![asker],
                |r| Ok(FleetTotals { players: r.get(0)?, others: r.get(1)?, used: r.get(2)?, contributed: r.get(3)? }),
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
            quantity: 1.0,
            value,
            game_time: 100.0,
            real_day: 20_000,
            ..Default::default()
        }
    }

    /// THE BALANCE'S SIGN: given minus used, in the black above 0, in the red below, even at
    /// 0 (and within a twentieth of a credit, so summed prices never show a debt of noise).
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

    /// A DIFFERENCE THE PANEL WOULD PRINT AS 0 CR IS EVEN (finding 17 of the 2026-10-04
    /// review): one meal used (10 CR) against 6.68 kWh of power returned (10.02 CR) is +0.02,
    /// which the panel's credits() prints "0 CR"; it must not read as in the black. And a
    /// difference the panel prints as 0.1 CR is in the black.
    ///
    /// Seen red 2026-10-04 with the old threshold of a hundredth of a credit (0.005): "+0.02
    /// CR is even / left: \"black\" / right: \"even\"".
    #[test]
    fn a_difference_the_panel_prints_as_nothing_is_even() {
        let b = FleetBalance { used: 10.0, contributed: 10.02 };
        assert_eq!(b.standing(), "even", "+0.02 CR is even / left: {:?} / right: \"even\"", b.standing());
        assert_eq!(FleetBalance { used: 10.0, contributed: 9.98 }.standing(), "even");
        assert_eq!(FleetBalance { used: 10.0, contributed: 10.1 }.standing(), "black");
        assert_eq!(FleetBalance { used: 10.1, contributed: 10.0 }.standing(), "red");
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
        let t = db.fleet_totals("aaaa").unwrap();
        assert_eq!((t.players, t.others, t.used, t.contributed), (2, 1, 20.0, 30.0));
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
        give.home = "home-a".into();
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

    /// A HOME'S GIVES ARE ITS OWN: the game asking for the gives of home A gets A's (newest
    /// first, as many as it asks for), never home B's and never another player's.
    ///
    /// Seen red 2026-10-04 with the home left out of the query's WHERE: "home A's gives only /
    /// left: [\"g-b1\", \"g-a2\", \"g-a1\"] / right: [\"g-a2\", \"g-a1\"]".
    #[test]
    fn a_homes_gives_are_its_own() {
        let (db, path) = temp_db("home");
        let give = |id: &str, home: &str| NewFleetEntry {
            item_id: "bread_0".into(),
            give_id: Some(id.into()),
            home: home.into(),
            ..line("item", "contributed", 3.0)
        };
        db.record_fleet_entry("aaaa", &give("g-a1", "home-a")).unwrap();
        db.record_fleet_entry("aaaa", &give("g-a2", "home-a")).unwrap();
        db.record_fleet_entry("aaaa", &give("g-b1", "home-b")).unwrap();
        db.record_fleet_entry("bbbb", &give("g-x", "home-a")).unwrap();
        let ids: Vec<String> = db.fleet_gives_for_home("aaaa", "home-a", 10).unwrap().into_iter().map(|g| g.give_id).collect();
        assert_eq!(ids, vec!["g-a2".to_string(), "g-a1".to_string()], "home A's gives only / left: {ids:?} / right: [\"g-a2\", \"g-a1\"]");
        assert_eq!(db.fleet_gives_for_home("aaaa", "home-a", 1).unwrap().len(), 1, "as many as asked for");
        assert!(db.fleet_gives_for_home("aaaa", "home-c", 10).unwrap().is_empty());
        drop(db);
        let _ = std::fs::remove_file(&path);
    }

    /// A GIVE IS CORRECTED DOWN, ONCE: the game found 1 of a give's 3 loaves to take, so the
    /// give counts 1, at the same price each; a second correction, and one claiming more,
    /// change nothing; an id never given is not found.
    ///
    /// Seen red 2026-10-04 with the `adjusted` check taken out: "a second correction changes
    /// nothing / left: Changed(FleetEntry { id: 1, kind: \"item\", ..., quantity: 0.0, value:
    /// 0.0, ... }) / right: Unchanged" (the second correction took the give to 0).
    #[test]
    fn a_give_is_corrected_down_once() {
        let (db, path) = temp_db("adjust");
        let give = NewFleetEntry { item_id: "bread_0".into(), quantity: 3.0, give_id: Some("g-1".into()), home: "h".into(), ..line("item", "contributed", 9.0) };
        db.record_fleet_entry("aaaa", &give).unwrap();
        match db.adjust_fleet_give("aaaa", "g-1", 1.0).unwrap() {
            Adjusted::Changed(e) => assert_eq!((e.quantity, e.value), (1.0, 3.0)),
            other => panic!("the correction lowers the give: {other:?}"),
        }
        let second = db.adjust_fleet_give("aaaa", "g-1", 0.0).unwrap();
        assert!(matches!(second, Adjusted::Unchanged(_)), "a second correction changes nothing / left: {second:?} / right: Unchanged");
        assert_eq!(db.fleet_ledger_of("aaaa", 5).unwrap().0.contributed, 3.0);
        assert_eq!(db.adjust_fleet_give("aaaa", "g-404", 0.0).unwrap(), Adjusted::NotFound);
        assert_eq!(db.fleet_gives_for_home("aaaa", "h", 5).unwrap()[0].quantity, 1.0, "the home's list counts what was delivered");
        // A correction claiming as many as recorded changes nothing either.
        let more = NewFleetEntry { give_id: Some("g-2".into()), ..give.clone() };
        db.record_fleet_entry("aaaa", &more).unwrap();
        assert!(matches!(db.adjust_fleet_give("aaaa", "g-2", 3.0).unwrap(), Adjusted::Unchanged(_)));
        drop(db);
        let _ = std::fs::remove_file(&path);
    }

    /// GIVES PAST THE DAILY CAP RECORD NOTHING (finding 11 of the 2026-10-04 review): with a
    /// cap of 3 a day, the fourth give that day is refused and no row is written; a repeat of
    /// a recorded give is still answered; the next real day takes gives again; other kinds,
    /// with no cap, are not counted against it.
    ///
    /// Seen red 2026-10-04 with the cap check taken out of `record_fleet_entry`: "the fourth
    /// give of the day is refused / left: New(FleetEntry { id: 5, kind: \"item\", ..., give_id:
    /// Some(\"g-4\"), home: \"h\" }) / right: OverCap".
    #[test]
    fn gives_beyond_the_daily_cap_record_nothing() {
        let (db, path) = temp_db("cap");
        let give = |id: &str, day: i64| NewFleetEntry {
            item_id: "bread_0".into(),
            give_id: Some(id.into()),
            home: "h".into(),
            real_day: day,
            day_cap: Some(3),
            ..line("item", "contributed", 3.0)
        };
        db.record_fleet_entry("aaaa", &line("meal", "used", 10.0)).unwrap();
        for id in ["g-1", "g-2", "g-3"] {
            assert!(matches!(db.record_fleet_entry("aaaa", &give(id, 20_000)).unwrap(), Recorded::New(_)));
        }
        let fourth = db.record_fleet_entry("aaaa", &give("g-4", 20_000)).unwrap();
        assert_eq!(fourth, Recorded::OverCap, "the fourth give of the day is refused / left: {fourth:?} / right: OverCap");
        assert_eq!(db.fleet_ledger_of("aaaa", 50).unwrap().2.len(), 4, "no row was written for it");
        assert!(matches!(db.record_fleet_entry("aaaa", &give("g-2", 20_000)).unwrap(), Recorded::Already(_)), "a repeat is still answered");
        assert!(matches!(db.record_fleet_entry("aaaa", &give("g-5", 20_001)).unwrap(), Recorded::New(_)), "the next day takes gives again");
        assert!(matches!(db.record_fleet_entry("bbbb", &give("g-1", 20_000)).unwrap(), Recorded::New(_)), "another player has a cap of their own");
        drop(db);
        let _ = std::fs::remove_file(&path);
    }
}
