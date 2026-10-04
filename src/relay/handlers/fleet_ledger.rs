//! THE FLEET LEDGER: what each player used from the fleet and what they gave it, in the red or
//! in the black (the operator's decision of 2026-10-04 on question 12 of
//! docs/design/ship-homes-and-logistics.md, verbatim: "For the sake of simplicity during early
//! development we'll say the fleet has unlimited of everything and just track what they player
//! uses and contributes. That way they can be in the red or black so they can gauge what
//! they're actually using/contributing.").
//!
//! WHAT IS RECORDED, per player key (storage/fleet_ledger.rs holds the lines):
//!   - USED: every meal they take from the ship's food stores (`take_meal_and_record`, the one
//!     way a player draws on the stores), and the electricity the ship's reactor supplies their
//!     home while they are in the shared world (`power_report`). Those two are everything the
//!     code has the fleet supply a player today: the reactor feeds every home past what it
//!     makes (src/systems/ship_power.rs, "essentially provide unlimited (at least to start)",
//!     metered), and the mess hall's stores feed everyone aboard. Air and water are made in
//!     each home by its own machines (the reactor powering them is the power line); nothing
//!     else is handed to a player by the fleet.
//!   - CONTRIBUTED: items they give the fleet at one of its stores (`give`), and the surplus
//!     electricity their home sends back to the ship (`power_report`).
//! What a line is worth comes from data, never from here: data/ship/fleet_ledger.ron names the
//! kinds and how each is priced (a meal as a Basic Ration, power per kWh, an item at its base
//! value in data/trade_goods.ron, the comments there say why).
//!
//! HOW A GIVE KEEPS ITEMS STRAIGHT. The relay holds no inventories yet (each player's backpack
//! lives in their own game and their own save; holdings on the server are increment 8 of the
//! design), exactly as for P2P trades (gui/pages/trade.rs). So a give is two halves:
//!   1. the game asks with `game_fleet_give` {give_id, entity_id (the store), item_id,
//!      quantity}, having checked that its backpack holds them;
//!   2. this relay checks the player is in the world, at a fleet store within reach, and that
//!      the fleet has a price for the item, then writes ONE line, keyed by the give's id. It
//!      answers `game_fleet_give_result`; only a success makes the game take the items out of
//!      its backpack, once, recorded by the give's id beside the backpack (the trade
//!      settlements, saved with it). A give the game sends again (the answer was lost to a
//!      reconnect) is the same line, answered `already: true`, so it is never counted twice.
//!   A refused give writes nothing and the game takes nothing. What the relay cannot do yet is
//!   see that the backpack really held the items: an altered game could give what it does not
//!   have, which only moves its own gauge until the server holds inventories (increment 8).
//!
//! SEEING IT. `game_fleet_ledger_request` answers `game_fleet_ledger` with the asker's OWN
//! ledger: no message here names another player, so nobody can read someone else's. An admin
//! may ask `game_fleet_totals_request` for the whole fleet's sums (`game_fleet_totals`), which
//! names nobody. Erasing an account deletes its lines (storage/account.rs).
//!
//! THE SETTING. `set_fleet_supply` puts an admin's choice of supply mode (ship_stores.rs
//! `FleetSupply`) into the running world. It never touches a ledger: unlimited or stocked,
//! every player's lines stay as they are.

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

use super::game_state::GameWorld;
use super::ship_stores::{distance, FleetSupply, STORE_REACH_M};
use crate::relay::relay::RelayState;
use crate::relay::storage::{NewFleetEntry, Recorded, Storage};

/// The kind ids this code records (each must be a kind of data/ship/fleet_ledger.ron: a test
/// checks the shipped file has every one).
pub const MEAL: &str = "meal";
pub const ITEM: &str = "item";
pub const POWER_DRAWN: &str = "power_drawn";
pub const POWER_RETURNED: &str = "power_returned";
pub const RECORDED_KINDS: [&str; 4] = [MEAL, ITEM, POWER_DRAWN, POWER_RETURNED];

/// Which side of the balance a kind is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Direction {
    Used,
    Contributed,
}

impl Direction {
    pub fn as_str(self) -> &'static str {
        match self {
            Direction::Used => "used",
            Direction::Contributed => "contributed",
        }
    }
}

/// How one unit of a kind is priced (see data/ship/fleet_ledger.ron).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub enum Price {
    /// The base_value of this trade good.
    OfGood(String),
    /// The base_value of the item the line is about.
    OfItem,
    /// This many CR a unit.
    Fixed(f64),
}

/// One kind of ledger line.
#[derive(Debug, Clone, Deserialize)]
pub struct KindDef {
    pub id: String,
    pub direction: Direction,
    pub label: String,
    pub unit: String,
    pub price: Price,
    #[serde(default)]
    pub per_day: bool,
}

/// data/ship/fleet_ledger.ron.
#[derive(Debug, Clone, Deserialize)]
pub struct LedgerFile {
    pub kinds: Vec<KindDef>,
    pub max_give_quantity: u32,
    pub recent_entries: usize,
    pub power_report_min_interval_s: f64,
    pub power_report_window_s: f64,
}

impl Default for LedgerFile {
    /// Neither the file nor the built-in copy: no kinds, so nothing is recorded.
    fn default() -> Self {
        LedgerFile { kinds: Vec::new(), max_give_quantity: 0, recent_entries: 20, power_report_min_interval_s: 30.0, power_report_window_s: 120.0 }
    }
}

impl LedgerFile {
    /// Parse a ledger file: every number finite and not negative, the windows positive, and
    /// every fixed price a number of credits that is not negative (RON reads NaN and inf).
    pub fn parse(text: &str) -> Result<LedgerFile, String> {
        let f: LedgerFile = ron::from_str(text).map_err(|e| e.to_string())?;
        if !(f.power_report_min_interval_s.is_finite() && f.power_report_min_interval_s > 0.0) {
            return Err("power_report_min_interval_s must be a number above 0".into());
        }
        if !(f.power_report_window_s.is_finite() && f.power_report_window_s >= f.power_report_min_interval_s) {
            return Err("power_report_window_s must be a number at least power_report_min_interval_s".into());
        }
        for k in &f.kinds {
            if let Price::Fixed(v) = k.price {
                if !(v.is_finite() && v >= 0.0) {
                    return Err(format!("kind {}: a fixed price must be a number not below 0; it is {v}", k.id));
                }
            }
        }
        Ok(f)
    }
}

/// A trade good's name and base value (data/trade_goods.ron).
#[derive(Debug, Clone, PartialEq)]
pub struct Good {
    pub name: String,
    pub base_value: f64,
}

/// What the ledger records and what things are worth: the ledger file, the trade goods'
/// values, and the reactor's output (data/ship_power.ron), read once when the world is made.
#[derive(Debug, Clone, Default)]
pub struct LedgerData {
    pub file: LedgerFile,
    pub goods: HashMap<String, Good>,
    /// The ship's reactor, watts: the most one power report can be counted at.
    pub reactor_watts: f64,
}

impl LedgerData {
    /// The three files from the relay's data folder, disk first; a missing or broken file is
    /// replaced by the copy built into the exe and the log says so (the same rule as every
    /// relay data file; the rigs refuse a run that served one).
    pub fn load() -> LedgerData {
        LedgerData { file: Self::load_file(std::path::Path::new("data/ship/fleet_ledger.ron")), goods: Self::load_goods(), reactor_watts: crate::systems::ship_power::ShipPowerData::load().electric_watts.max(0.0) }
    }

    /// data/ship/fleet_ledger.ron from `path` (the tests point it at a missing one and a broken
    /// one), else the built-in copy, else nothing.
    pub fn load_file(path: &std::path::Path) -> LedgerFile {
        let shown = path.display().to_string();
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
            crate::embedded_data::note_builtin_copy("ship/fleet_ledger.ron", format_args!("{shown} could not be read ({e})"));
            include_str!("../../../data/ship/fleet_ledger.ron").to_string()
        });
        match LedgerFile::parse(&text) {
            Ok(f) => f,
            Err(e) => {
                crate::embedded_data::note_builtin_copy("ship/fleet_ledger.ron", format_args!("{shown} does not parse ({e})"));
                LedgerFile::parse(include_str!("../../../data/ship/fleet_ledger.ron")).unwrap_or_else(|e| {
                    tracing::error!("The built-in fleet_ledger.ron does not parse either ({e}); the fleet ledger records nothing");
                    LedgerFile::default()
                })
            }
        }
    }

    /// Every trade good's name and base value (data/trade_goods.ron, disk first).
    fn load_goods() -> HashMap<String, Good> {
        let Some(text) = crate::embedded_data::read_data_or_embedded(std::path::Path::new("data"), "trade_goods.ron") else {
            tracing::error!("trade_goods.ron is missing; the fleet has no prices, so it takes no gives");
            return HashMap::new();
        };
        match crate::systems::economy::TradeGoodsRegistry::from_ron(text.as_bytes()) {
            Ok(reg) => reg.goods.into_iter().map(|(id, g)| (id, Good { name: g.name, base_value: f64::from(g.base_value) })).collect(),
            Err(e) => {
                tracing::error!("trade_goods.ron does not parse ({e}); the fleet has no prices, so it takes no gives");
                HashMap::new()
            }
        }
    }

    pub fn kind(&self, id: &str) -> Option<&KindDef> {
        self.file.kinds.iter().find(|k| k.id == id)
    }

    /// What one unit of `kind` (about `item_id`, for an item) is worth now, CR; None when it
    /// has no price (an item that is no trade good, or a good the file names that is gone).
    pub fn unit_price(&self, kind: &KindDef, item_id: &str) -> Option<f64> {
        match &kind.price {
            Price::Fixed(v) => Some(*v),
            Price::OfGood(id) => self.goods.get(id).map(|g| g.base_value),
            Price::OfItem => self.goods.get(item_id).map(|g| g.base_value),
        }
    }

    /// A line of `kind_id`, `quantity` units of it, at the price of the moment; None when the
    /// kind is not in the file or has no price.
    pub fn line(&self, kind_id: &str, item_id: &str, quantity: f64, game_time: f64, give_id: Option<String>) -> Option<NewFleetEntry> {
        let kind = self.kind(kind_id)?;
        let price = self.unit_price(kind, item_id)?;
        Some(NewFleetEntry {
            kind: kind.id.clone(),
            direction: kind.direction.as_str().to_string(),
            item_id: item_id.to_string(),
            quantity,
            value: price * quantity,
            game_time,
            real_day: crate::relay::storage::fleet_ledger::real_day_now(),
            give_id,
            per_day: kind.per_day,
        })
    }

    fn item_name(&self, id: &str) -> Option<&str> {
        self.goods.get(id).map(|g| g.name.as_str())
    }
}

/// Is this entity one of the fleet's stores, where a player may give? The mess hall's food
/// stores are the fleet's only stores today, so food and materials alike are handed in there.
fn is_fleet_store(e: &super::game_state::GameEntity) -> bool {
    e.entity_type == "food_store"
}

/// A give id the game chose: 1 to 64 letters, digits, '-' or '_'.
fn valid_give_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// `take_meal` (ship_stores.rs) and, when a meal was taken, its line in the taker's ledger:
/// one meal, used, at the meal's price. The reply carries `ledger` {kind, value} (or
/// `ledger_error` when the line could not be written: the meal was still taken).
pub fn take_meal_and_record(world: &mut GameWorld, db: &Storage, key: &str, entity_id: u64) -> serde_json::Value {
    let mut reply = super::ship_stores::take_meal(world, key, entity_id);
    if reply["success"] != true {
        return reply;
    }
    let Some(line) = world.fleet_ledger.line(MEAL, "", 1.0, world.game_time, None) else {
        tracing::error!("Fleet ledger: no priced \"meal\" kind in data/ship/fleet_ledger.ron; a meal went unrecorded");
        reply["ledger_error"] = serde_json::json!("no_meal_kind");
        return reply;
    };
    match db.record_fleet_entry(key, &line) {
        Ok(_) => reply["ledger"] = serde_json::json!({ "kind": MEAL, "direction": line.direction, "value": line.value }),
        Err(e) => {
            tracing::error!("Fleet ledger: a meal could not be recorded: {e}");
            reply["ledger_error"] = serde_json::json!("failed");
        }
    }
    reply
}

/// A player's give to the fleet (`game_fleet_give`): the reply, a `game_fleet_give_result`.
/// Success writes exactly one line (or finds the one an earlier send of the same give wrote);
/// every refusal writes nothing: `bad_give_id`, `not_in_game`, `entity_not_found`,
/// `not_a_store`, `too_far` (with `distance`), `bad_item`, `bad_quantity` (with `max`),
/// `no_price` (the fleet has no price for it), `failed` (the database said no).
pub fn give(world: &GameWorld, db: &Storage, key: &str, raw: &serde_json::Value) -> serde_json::Value {
    let give_id = raw.get("give_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let item_id = raw.get("item_id").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    let fail = |error: &str| {
        serde_json::json!({ "type": "game_fleet_give_result", "give_id": give_id, "item_id": item_id, "success": false, "error": error })
    };
    let done = |e: &crate::relay::storage::FleetEntry, already: bool| {
        serde_json::json!({
            "type": "game_fleet_give_result", "give_id": give_id, "success": true, "already": already,
            "entry_id": e.id, "item_id": e.item_id, "item_name": world.fleet_ledger.item_name(&e.item_id),
            "quantity": e.quantity, "value": e.value,
        })
    };
    if !valid_give_id(&give_id) {
        return fail("bad_give_id");
    }
    // Sent again after a lost answer: the line it wrote then, whatever has changed since.
    match db.fleet_give_of(key, &give_id) {
        Ok(Some(e)) => return done(&e, true),
        Ok(None) => {}
        Err(e) => {
            tracing::error!("Fleet ledger: could not look up give {give_id}: {e}");
            return fail("failed");
        }
    }
    let Some(player) = world.find_player_entity(key) else { return fail("not_in_game") };
    let Some(store) = raw.get("entity_id").and_then(|v| v.as_u64()).and_then(|id| world.entities.get(&id)) else {
        return fail("entity_not_found");
    };
    if !is_fleet_store(store) {
        return fail("not_a_store");
    }
    let dist = distance(world.entities[&player].position, store.position);
    if dist > STORE_REACH_M {
        let mut r = fail("too_far");
        r["distance"] = serde_json::json!(dist);
        return r;
    }
    if item_id.is_empty() || item_id.len() > 64 {
        return fail("bad_item");
    }
    let max = world.fleet_ledger.file.max_give_quantity;
    let quantity = raw.get("quantity").and_then(|v| v.as_u64()).unwrap_or(0);
    if quantity == 0 || quantity > u64::from(max) {
        let mut r = fail("bad_quantity");
        r["max"] = serde_json::json!(max);
        return r;
    }
    let Some(line) = world.fleet_ledger.line(ITEM, &item_id, quantity as f64, world.game_time, Some(give_id.clone())) else {
        return fail("no_price");
    };
    match db.record_fleet_entry(key, &line) {
        Ok(Recorded::New(e)) => done(&e, false),
        Ok(Recorded::Already(e)) => done(&e, true),
        Err(e) => {
            tracing::error!("Fleet ledger: give {give_id} could not be recorded: {e}");
            fail("failed")
        }
    }
}

/// A game's report of its home's power (`game_fleet_power` {drawn_wh, returned_wh}, watt-hours
/// since its last report): the reply, a `game_fleet_power_result`. Each is held to what the
/// ship's reactor could deliver over the time since this player's last report, at most
/// `power_report_window_s` real seconds of it (`clamped` says when that cut one down), and
/// written to today's power lines in kWh. Refused: `not_in_game`, `bad_report` (not numbers,
/// or below 0), `too_soon` (under `power_report_min_interval_s` real seconds since the last).
pub fn power_report(world: &mut GameWorld, db: &Storage, key: &str, raw: &serde_json::Value) -> serde_json::Value {
    let fail = |error: &str| serde_json::json!({ "type": "game_fleet_power_result", "success": false, "error": error });
    if world.find_player_entity(key).is_none() {
        return fail("not_in_game");
    }
    let wh = |k: &str| raw.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0);
    let (drawn, returned) = (wh("drawn_wh"), wh("returned_wh"));
    if !(drawn.is_finite() && returned.is_finite() && drawn >= 0.0 && returned >= 0.0) {
        return fail("bad_report");
    }
    let now = world.game_time;
    let scale = world.time_scale.max(1e-9);
    let file = &world.fleet_ledger.file;
    let since = world.fleet_power_last.get(key).map(|t| now - t);
    if since.is_some_and(|s| s < file.power_report_min_interval_s * scale) {
        return fail("too_soon");
    }
    let window_s = since.unwrap_or(f64::INFINITY).min(file.power_report_window_s * scale);
    let cap_wh = world.fleet_ledger.reactor_watts * window_s / 3600.0;
    let clamped = drawn > cap_wh || returned > cap_wh;
    let (drawn, returned) = (drawn.min(cap_wh), returned.min(cap_wh));
    world.fleet_power_last.insert(key.to_string(), now);
    for (kind, value) in [(POWER_DRAWN, drawn), (POWER_RETURNED, returned)] {
        if value <= 0.0 {
            continue;
        }
        let Some(line) = world.fleet_ledger.line(kind, "", value / 1000.0, now, None) else { continue };
        if let Err(e) = db.record_fleet_entry(key, &line) {
            tracing::error!("Fleet ledger: a power report could not be recorded: {e}");
            return fail("failed");
        }
    }
    serde_json::json!({ "type": "game_fleet_power_result", "success": true, "drawn_kwh": drawn / 1000.0, "returned_kwh": returned / 1000.0, "clamped": clamped })
}

/// `key`'s own ledger, the `game_fleet_ledger` message: the supply mode, the totals and the
/// balance (`standing` black, red or even), each kind's totals and the newest lines.
pub fn ledger_json(world: &GameWorld, db: &Storage, key: &str) -> serde_json::Value {
    let data = &world.fleet_ledger;
    let recent = data.file.recent_entries;
    let label = |kind: &str| data.kind(kind).map_or(kind.to_string(), |k| k.label.clone());
    let unit = |kind: &str| data.kind(kind).map_or(String::new(), |k| k.unit.clone());
    match db.fleet_ledger_of(key, recent) {
        Ok((bal, kinds, lines)) => serde_json::json!({
            "type": "game_fleet_ledger",
            "supply": world.fleet_supply.as_str(),
            "currency": "CR",
            "used_value": bal.used,
            "contributed_value": bal.contributed,
            "balance": bal.balance(),
            "standing": bal.standing(),
            "game_time": world.game_time,
            "kinds": kinds.iter().map(|k| serde_json::json!({
                "kind": k.kind, "label": label(&k.kind), "unit": unit(&k.kind),
                "direction": k.direction, "quantity": k.quantity, "value": k.value,
            })).collect::<Vec<_>>(),
            "recent": lines.iter().map(|l| serde_json::json!({
                "id": l.id, "kind": l.kind, "label": label(&l.kind), "unit": unit(&l.kind),
                "direction": l.direction, "item_id": l.item_id, "item_name": data.item_name(&l.item_id),
                "quantity": l.quantity, "value": l.value, "game_time": l.game_time, "real_day": l.real_day,
            })).collect::<Vec<_>>(),
        }),
        Err(e) => {
            tracing::error!("Fleet ledger: could not read {key}'s ledger: {e}");
            serde_json::json!({ "type": "game_fleet_ledger", "error": "failed" })
        }
    }
}

/// The whole fleet's sums for an admin, the `game_fleet_totals` message (no names in it).
pub fn totals_json(world: &GameWorld, db: &Storage) -> serde_json::Value {
    match db.fleet_totals() {
        Ok(t) => {
            let bal = crate::relay::storage::FleetBalance { used: t.used, contributed: t.contributed };
            serde_json::json!({
                "type": "game_fleet_totals", "supply": world.fleet_supply.as_str(), "currency": "CR",
                "players": t.players, "used_value": t.used, "contributed_value": t.contributed,
                "balance": bal.balance(), "standing": bal.standing(),
            })
        }
        Err(e) => {
            tracing::error!("Fleet ledger: could not read the fleet's totals: {e}");
            serde_json::json!({ "type": "game_admin_error", "message": "The fleet's totals could not be read." })
        }
    }
}

/// Put an admin's supply mode into the running world (from `server_settings_update`, after the
/// setting is saved, so a restart keeps it). No ledger is touched.
pub async fn set_fleet_supply(state: &Arc<RelayState>, mode: &str) {
    let supply = FleetSupply::from_setting(mode);
    let mut world = state.game_world.write().await;
    if world.fleet_supply != supply {
        world.fleet_supply = supply;
        tracing::info!("The fleet's stores are now {}", supply.as_str());
    }
}

/// The four ledger messages (relay.rs routes them here). Replies go to the sender alone.
pub async fn handle(state: &Arc<RelayState>, my_key: &str, kind: &str, raw: &serde_json::Value) {
    let send = |v: serde_json::Value| async move { super::msg_handlers::send_game_private(state, my_key, &v).await };
    match kind {
        "game_fleet_ledger_request" => {
            if !super::msg_handlers::check_perception_rate(state, my_key, "fleet_ledger") {
                return;
            }
            let v = { ledger_json(&*state.game_world.read().await, &state.db, my_key) };
            send(v).await;
        }
        "game_fleet_give" => {
            if !super::msg_handlers::check_perception_rate(state, my_key, "fleet_give") {
                return;
            }
            let (reply, ledger) = {
                let world = state.game_world.read().await;
                let reply = give(&world, &state.db, my_key, raw);
                // The ledger after it, so the game shows the new line at once.
                let ledger = (reply["success"] == true).then(|| ledger_json(&world, &state.db, my_key));
                (reply, ledger)
            };
            send(reply).await;
            if let Some(l) = ledger {
                send(l).await;
            }
        }
        "game_fleet_power" => {
            let v = { power_report(&mut *state.game_world.write().await, &state.db, my_key, raw) };
            send(v).await;
        }
        "game_fleet_totals_request" => {
            let v = if super::msg_handlers::is_game_admin(state, my_key) {
                totals_json(&*state.game_world.read().await, &state.db)
            } else {
                serde_json::json!({ "type": "game_admin_error", "message": "Only admins can see the fleet's totals." })
            };
            send(v).await;
        }
        _ => {}
    }
}

#[cfg(test)]
#[path = "fleet_ledger_tests.rs"]
mod tests;
