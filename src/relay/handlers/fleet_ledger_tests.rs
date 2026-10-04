//! Tests of the fleet ledger (fleet_ledger.rs). Each was seen to fail before it passed; the
//! failure text is in its comment.

use super::*;
use crate::relay::storage::Storage;

fn temp_db(tag: &str) -> (Storage, std::path::PathBuf) {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let path = std::env::temp_dir().join(format!("hum_fleet_h_{tag}_{}_{nanos}.db", std::process::id()));
    (Storage::open(&path).expect("open test db"), path)
}

fn store_of(world: &GameWorld) -> (u64, [f32; 3]) {
    let mut s: Vec<(u64, [f32; 3])> = world.entities.iter().filter(|(_, e)| e.entity_type == "food_store").map(|(id, e)| (*id, e.position)).collect();
    s.sort_by_key(|(id, _)| *id);
    s[0]
}

/// A player standing 1 m from the mess hall's stores.
fn player_at_store(world: &mut GameWorld, key: &str) -> u64 {
    let (_, at) = store_of(world);
    world.spawn_player(key, [at[0] + 1.0, 1.7, at[2]])
}

fn give_msg(give_id: &str, store: u64, item: &str, qty: u64) -> serde_json::Value {
    serde_json::json!({ "type": "game_fleet_give", "give_id": give_id, "entity_id": store, "item_id": item, "quantity": qty })
}

fn lines_of(db: &Storage, key: &str) -> Vec<crate::relay::storage::FleetEntry> {
    db.fleet_ledger_of(key, 1000).unwrap().2
}

fn count(world: &GameWorld, k: &str) -> u64 {
    world.entities.values().filter_map(|e| e.components.get(k).and_then(|v| v.as_u64())).sum()
}

/// THE SHIPPED LEDGER FILE has every kind this relay records, each with a price: a meal is a
/// Basic Ration's base value in data/trade_goods.ron (10 CR), a kWh 1.5 CR, an item its own
/// base value; an item that is no trade good has no price. A missing or broken file is
/// replaced by the built-in copy, which has the same kinds.
///
/// Seen red 2026-10-04 with the "power_returned" row taken out of the file: "the shipped
/// fleet_ledger.ron has no kind \"power_returned\", which the relay records".
#[test]
fn the_shipped_ledger_file_prices_every_kind_the_relay_records() {
    let data = LedgerData::load();
    for id in RECORDED_KINDS {
        let k = data.kind(id).unwrap_or_else(|| panic!("the shipped fleet_ledger.ron has no kind {id:?}, which the relay records"));
        if !matches!(k.price, Price::OfItem) {
            assert!(data.unit_price(k, "").is_some(), "{id} has no price");
        }
    }
    let ration = data.goods.get("ration_basic_0").expect("trade_goods.ron prices a Basic Ration").base_value;
    assert_eq!(data.unit_price(data.kind(MEAL).unwrap(), ""), Some(ration), "a meal is priced as a Basic Ration");
    assert_eq!(ration, 10.0);
    assert_eq!(data.unit_price(data.kind(POWER_DRAWN).unwrap(), ""), Some(1.5));
    assert_eq!(data.unit_price(data.kind(ITEM).unwrap(), "bread_0"), Some(data.goods["bread_0"].base_value));
    assert_eq!(data.unit_price(data.kind(ITEM).unwrap(), "no_such_item"), None, "no trade good, no price");
    assert_eq!(data.kind(MEAL).unwrap().direction, Direction::Used);
    assert_eq!(data.kind(ITEM).unwrap().direction, Direction::Contributed);
    assert!(data.reactor_watts > 0.0, "the reactor's output is read: {}", data.reactor_watts);

    // A missing file and a broken one both fall back to the built-in copy.
    let dir = std::env::temp_dir().join(format!("hum_fleet_file_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let missing = LedgerData::load_file(&dir.join("nope.ron"));
    assert_eq!(missing.kinds.len(), data.file.kinds.len(), "a missing file serves the built-in copy");
    let broken = dir.join("broken.ron");
    std::fs::write(&broken, "( kinds: [ oops").unwrap();
    assert_eq!(LedgerData::load_file(&broken).kinds.len(), data.file.kinds.len(), "a broken file serves the built-in copy");
    let text = std::fs::read_to_string("data/ship/fleet_ledger.ron").unwrap();
    assert!(LedgerFile::parse(&text.replace("price: Fixed(1.5)", "price: Fixed(NaN)")).is_err(), "a NaN price is refused");
    assert!(LedgerFile::parse(&text.replace("power_report_window_s: 120.0", "power_report_window_s: 1.0")).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

/// AN UNLIMITED FLEET NEVER RUNS DRY (the operator, 2026-10-04: "the fleet has unlimited of
/// everything"). A new world is unlimited; with its store EMPTY and no farms filling it, five
/// game days of meals every 4 game hours: no crew member misses one, every player meal is
/// served, and the stock is never drawn on.
///
/// Seen red 2026-10-04 with `draw_meal` ignoring the mode (the stocked path always): "a player
/// meal was refused: {\"action\":\"take_meal\",\"entity_id\":12,\"error\":\"empty\",
/// \"meals_left\":0.0,\"success\":false,...} / left: String(\"empty\") / right: \"not_yet\"".
#[test]
fn an_unlimited_fleet_never_runs_dry_and_nobody_misses_a_meal() {
    let mut world = GameWorld::new();
    assert_eq!(world.fleet_supply, FleetSupply::Unlimited, "a new world's fleet is unlimited");
    super::super::ship_stores::meals_every(&mut world, 4.0);
    world.provisions.ship_farms_meals_per_day = 0.0;
    let (store, _) = store_of(&world);
    world.entities.get_mut(&store).unwrap().components["meals"] = serde_json::json!(0.0);
    player_at_store(&mut world, "e11e00b0");
    let mut player_meals = 0;
    let end = world.game_time + 5.0 * 86_400.0;
    while world.game_time < end {
        world.tick(0.25);
        let r = super::super::ship_stores::take_meal(&mut world, "e11e00b0", store);
        if r["success"] == true {
            player_meals += 1;
            assert!(r["meals_left"].is_null() && r["supply"] == "unlimited", "{r}");
        } else {
            assert_eq!(r["error"], "not_yet", "a player meal was refused: {r}");
        }
    }
    let missed = count(&world, "meals_missed");
    assert_eq!(missed, 0, "with the fleet unlimited, {missed} crew meals were missed in five game days");
    let eaten = count(&world, "meals_eaten");
    assert!(eaten >= 100, "the crew ate: {eaten} meals");
    assert!(player_meals >= 29, "a player meal every 4 game hours for five days: {player_meals}");
    assert_eq!(world.entities[&store].components["meals"].as_f64(), Some(0.0), "the stock was never drawn on");
}

/// A MEAL TAKEN IS A "USED" LINE WITH ITS VALUE: one meal, at the Basic Ration's 10 CR, and
/// the taker is 10 CR in the red. A refused meal writes nothing.
///
/// Seen red 2026-10-04 with the line's write taken out of `take_meal_and_record` (the reply
/// still saying it was): "a meal taken is one used line / left: 0 / right: 1".
#[test]
fn a_meal_taken_is_a_used_line_with_its_value() {
    let (db, path) = temp_db("meal");
    let mut world = GameWorld::new();
    let (store, at) = store_of(&world);
    let far = world.spawn_player("e11e00b1", [at[0] + 20.0, 1.7, at[2]]);
    assert_eq!(take_meal_and_record(&mut world, &db, "e11e00b1", store)["error"], "too_far");
    assert!(lines_of(&db, "e11e00b1").is_empty(), "a refused meal writes nothing");
    world.entities.get_mut(&far).unwrap().position = [at[0] + 1.0, 1.7, at[2]];
    let r = take_meal_and_record(&mut world, &db, "e11e00b1", store);
    assert_eq!(r["success"], true, "{r}");
    assert_eq!(r["ledger"]["value"].as_f64(), Some(10.0), "{r}");
    let lines = lines_of(&db, "e11e00b1");
    assert_eq!(lines.len(), 1, "a meal taken is one used line / left: {} / right: 1", lines.len());
    assert_eq!((lines[0].kind.as_str(), lines[0].direction.as_str(), lines[0].quantity, lines[0].value), (MEAL, "used", 1.0, 10.0));
    assert!((lines[0].game_time - world.game_time).abs() < 1e-9, "it says when, on the game clock");
    assert_eq!(lines[0].real_day, crate::relay::storage::fleet_ledger::real_day_now(), "and the real day");
    let l = ledger_json(&world, &db, "e11e00b1");
    assert_eq!((l["used_value"].as_f64(), l["standing"].as_str()), (Some(10.0), Some("red")), "{l}");
    // The second meal before its time is refused, and writes nothing.
    assert_eq!(take_meal_and_record(&mut world, &db, "e11e00b1", store)["error"], "not_yet");
    assert_eq!(lines_of(&db, "e11e00b1").len(), 1);
    drop(db);
    let _ = std::fs::remove_file(&path);
}

/// A GIVE IS A "CONTRIBUTED" LINE, RECORDED ONCE: three loaves of bread at the store are one
/// line worth three times bread's base value; the same give sent again (its answer lost) is
/// the same line, answered `already`, never a second. That the game takes the loaves out of
/// the backpack exactly once on that answer is the game's half (gui/pages/fleet_ledger.rs).
///
/// Seen red 2026-10-04 with the give-id look-ups taken out of `give` and of
/// `record_fleet_entry` (only the unique index left to stop a second line): the repeat was
/// refused instead of answered, "{\"error\":\"failed\",\"give_id\":\"g-0001\",...
/// \"success\":false,...} / left: (Some(false), None) / right: (Some(true), Some(true))".
#[test]
fn a_give_is_a_contributed_line_recorded_once() {
    let (db, path) = temp_db("give");
    let mut world = GameWorld::new();
    let (store, _) = store_of(&world);
    player_at_store(&mut world, "e11e00b2");
    let bread = world.fleet_ledger.goods["bread_0"].base_value;
    let r = give(&world, &db, "e11e00b2", &give_msg("g-0001", store, "bread_0", 3));
    assert_eq!((r["success"].as_bool(), r["already"].as_bool()), (Some(true), Some(false)), "{r}");
    assert_eq!(r["value"].as_f64(), Some(3.0 * bread), "{r}");
    assert_eq!(r["item_name"], "Bread", "{r}");
    let again = give(&world, &db, "e11e00b2", &give_msg("g-0001", store, "bread_0", 3));
    assert_eq!((again["success"].as_bool(), again["already"].as_bool()), (Some(true), Some(true)), "{again}");
    assert_eq!(again["entry_id"], r["entry_id"], "the same line");
    let lines = lines_of(&db, "e11e00b2");
    assert_eq!(lines.len(), 1, "the same give sent twice is one line / left: {} / right: 1", lines.len());
    assert_eq!((lines[0].kind.as_str(), lines[0].direction.as_str(), lines[0].item_id.as_str(), lines[0].quantity), (ITEM, "contributed", "bread_0", 3.0));
    assert_eq!(lines[0].give_id.as_deref(), Some("g-0001"));
    let l = ledger_json(&world, &db, "e11e00b2");
    assert_eq!(l["standing"], "black", "given, nothing used: in the black: {l}");
    assert_eq!(l["recent"][0]["item_name"], "Bread");
    drop(db);
    let _ = std::fs::remove_file(&path);
}

/// A REFUSED GIVE RECORDS NOTHING (so the game takes nothing): not in the world, no such
/// store, not a store, too far, no item, a quantity of 0 or past the cap, an item the fleet has
/// no price for, a give id that is no id. After all of them, the ledger is empty.
///
/// Seen red 2026-10-04 with the reach check taken out of `give`: "a give from 20 m away was
/// refused: {\"already\":false,\"entry_id\":1,\"give_id\":\"g-2\",...\"success\":true,...}
/// / left: Null / right: \"too_far\"".
#[test]
fn a_refused_give_records_nothing() {
    let (db, path) = temp_db("refused");
    let mut world = GameWorld::new();
    let (store, at) = store_of(&world);
    let key = "e11e00b3";
    assert_eq!(give(&world, &db, key, &give_msg("g-1", store, "bread_0", 1))["error"], "not_in_game");
    let p = world.spawn_player(key, [at[0] + 20.0, 1.7, at[2]]);
    let far = give(&world, &db, key, &give_msg("g-2", store, "bread_0", 1));
    assert_eq!(far["error"], "too_far", "a give from 20 m away was refused: {far}");
    world.entities.get_mut(&p).unwrap().position = [at[0] + 1.0, 1.7, at[2]];
    let table = *world.entities.iter().find(|(_, e)| e.entity_type == "dining_table").map(|(id, _)| id).expect("a dining table");
    let max = u64::from(world.fleet_ledger.file.max_give_quantity);
    for (msg, want) in [
        (give_msg("g-3", 999_999, "bread_0", 1), "entity_not_found"),
        (give_msg("g-4", table, "bread_0", 1), "not_a_store"),
        (give_msg("g-5", store, "", 1), "bad_item"),
        (give_msg("g-6", store, "bread_0", 0), "bad_quantity"),
        (give_msg("g-7", store, "bread_0", max + 1), "bad_quantity"),
        (give_msg("g-8", store, "no_such_item", 1), "no_price"),
        (give_msg("", store, "bread_0", 1), "bad_give_id"),
        (give_msg("g 9 has spaces", store, "bread_0", 1), "bad_give_id"),
    ] {
        let r = give(&world, &db, key, &msg);
        assert_eq!(r["error"], want, "{msg} -> {r}");
        assert_eq!(r["success"], false);
    }
    assert!(lines_of(&db, key).is_empty(), "nothing was recorded: {:?}", lines_of(&db, key));
    drop(db);
    let _ = std::fs::remove_file(&path);
}

/// POWER, USED AND RETURNED: a report is written to today's lines in kWh at 1.5 CR each; a
/// second report sooner than the minimum is refused; a report claiming more than the reactor
/// could deliver over the time since the last one is held to that; a negative one is refused.
///
/// Seen red 2026-10-04 with the cap taken out: "a report of a year of power was held to the
/// reactor's output over the window: 3.066e8 kWh recorded, cap 4.2e4".
#[test]
fn power_reports_are_kept_per_day_and_held_to_the_reactor() {
    let (db, path) = temp_db("power");
    let mut world = GameWorld::new();
    let key = "e11e00b4";
    player_at_store(&mut world, key);
    let r = power_report(&mut world, &db, key, &serde_json::json!({ "drawn_wh": 2000.0, "returned_wh": 500.0 }));
    assert_eq!((r["success"].as_bool(), r["drawn_kwh"].as_f64(), r["clamped"].as_bool()), (Some(true), Some(2.0), Some(false)), "{r}");
    assert_eq!(power_report(&mut world, &db, key, &serde_json::json!({ "drawn_wh": 1.0 }))["error"], "too_soon");
    world.game_time += 60.0 * world.time_scale; // a real minute on the game clock
    power_report(&mut world, &db, key, &serde_json::json!({ "drawn_wh": 1000.0, "returned_wh": 0.0 }));
    let l = ledger_json(&world, &db, key);
    let kinds: Vec<(String, f64, f64)> = l["kinds"].as_array().unwrap().iter().map(|k| (k["kind"].as_str().unwrap().to_string(), k["quantity"].as_f64().unwrap(), k["value"].as_f64().unwrap())).collect();
    assert!(kinds.contains(&(POWER_DRAWN.to_string(), 3.0, 4.5)), "two reports on one day, one line of 3 kWh at 1.5 CR: {kinds:?}");
    assert!(kinds.contains(&(POWER_RETURNED.to_string(), 0.5, 0.75)), "{kinds:?}");
    assert_eq!(lines_of(&db, key).len(), 2, "one drawn line and one returned line today");
    // A year of power in one report, a real minute after the last: held to what the reactor
    // makes in that minute on the game clock.
    world.game_time += 60.0 * world.time_scale;
    let year_wh = world.fleet_ledger.reactor_watts * 365.0 * 24.0;
    let r = power_report(&mut world, &db, key, &serde_json::json!({ "drawn_wh": year_wh }));
    let cap_kwh = world.fleet_ledger.reactor_watts * 60.0 * world.time_scale / 3600.0 / 1000.0;
    let got = r["drawn_kwh"].as_f64().unwrap();
    assert!((got - cap_kwh).abs() < 1e-6, "a report of a year of power was held to the reactor's output over the window: {got:e} kWh recorded, cap {cap_kwh:e}");
    assert_eq!(r["clamped"], true);
    world.game_time += 60.0 * world.time_scale;
    assert_eq!(power_report(&mut world, &db, key, &serde_json::json!({ "drawn_wh": -5.0 }))["error"], "bad_report");
    assert_eq!(power_report(&mut world, &db, "e11e00ff", &serde_json::json!({ "drawn_wh": 5.0 }))["error"], "not_in_game");
    drop(db);
    let _ = std::fs::remove_file(&path);
}

/// ONE PLAYER CANNOT SEE ANOTHER'S LEDGER: the ledger message is built from the asker's key
/// alone (relay.rs passes the socket's own key; no field of the request names anyone), and it
/// holds their lines only. An admin's totals sum everyone without naming anyone.
///
/// Seen red 2026-10-04 with the newest-lines query reading every player's lines (its key
/// left out): "B sees only B's meal / left: 3 / right: 1".
#[test]
fn one_player_cannot_see_another_players_ledger() {
    let (db, path) = temp_db("private");
    let mut world = GameWorld::new();
    let (store, _) = store_of(&world);
    player_at_store(&mut world, "aaaa01");
    player_at_store(&mut world, "bbbb02");
    take_meal_and_record(&mut world, &db, "aaaa01", store);
    take_meal_and_record(&mut world, &db, "bbbb02", store);
    give(&world, &db, "aaaa01", &give_msg("g-a", store, "bread_0", 2));
    let b = ledger_json(&world, &db, "bbbb02");
    let n = b["recent"].as_array().unwrap().len();
    assert_eq!(n, 1, "B sees only B's meal / left: {n} / right: 1");
    assert_eq!(b["contributed_value"].as_f64(), Some(0.0), "A's give is not in B's ledger: {b}");
    let t = totals_json(&world, &db);
    assert_eq!(t["players"], 2, "{t}");
    assert!(!t.to_string().contains("aaaa01") && !t.to_string().contains("bbbb02"), "the totals name nobody: {t}");
    drop(db);
    let _ = std::fs::remove_file(&path);
}

/// THE LEDGER AND THE SETTING SURVIVE A RESTART, AND A CHANGE OF MODE TOUCHES NO LEDGER: lines
/// written, the admin's "stocked" saved; the relay starts again on the same file and its world
/// runs stocked, the lines all there.
///
/// Seen red 2026-10-04 with the line in `RelayState::new` that reads the mode taken out:
/// "after a restart the fleet runs as saved / left: Unlimited / right: Stocked".
#[test]
fn the_ledger_and_the_setting_survive_a_restart() {
    let (db, path) = temp_db("restart");
    let mut world = GameWorld::new();
    let (store, _) = store_of(&world);
    player_at_store(&mut world, "e11e00b5");
    take_meal_and_record(&mut world, &db, "e11e00b5", store);
    give(&world, &db, "e11e00b5", &give_msg("g-r", store, "bread_0", 1));
    let mut s = db.get_server_settings().unwrap();
    s.fleet_supply_mode = "stocked".into();
    assert!(db.set_server_settings(&s, "admin").unwrap());
    drop(db);
    let state = crate::relay::relay::RelayState::new(Storage::open(&path).expect("reopen"));
    let w = state.game_world.try_read().expect("nothing else holds the world");
    assert_eq!(w.fleet_supply, FleetSupply::Stocked, "after a restart the fleet runs as saved / left: {:?} / right: Stocked", w.fleet_supply);
    let l = ledger_json(&w, &state.db, "e11e00b5");
    assert_eq!(l["recent"].as_array().unwrap().len(), 2, "both lines came back: {l}");
    assert_eq!(l["supply"], "stocked");
    drop(w);
    drop(state);
    let _ = std::fs::remove_file(&path);
}

/// A DATABASE THE PREVIOUS CODE WROTE OPENS (the BUG-046 rule). tests/fixtures/relay/
/// relay_v0_1456.sql is EXACTLY the database the code before the fleet ledger wrote (main at
/// 86eafead9, v0.1456.0), dumped to SQL text so it can be read in review and is not a 1 MB
/// binary: `Storage::open` on a new file, an admin's settings (the clock at 24x, public
/// messages kept 7 days), a player "e11e00f0"'s progress, and the shared world saved after
/// `GameWorld::new()`, that player taking one meal (stocked then, the only mode: 90 meals to
/// 89) and ten real minutes of 20 Hz ticks (the farms brought the store to 93). Dumped by
/// node:sqlite (every schema object as SQLite stored it, then every row), no row edited.
///
/// Loaded into a fresh file and opened by this code: the settings keep their values and gain
/// the fleet's "unlimited"; the ledger table is there and takes lines; the stored world
/// restores with its store's stock carried over; and the player's next meal is a ledger line.
///
/// Seen red 2026-10-04 with the guarded ALTER for fleet_supply_mode switched off:
/// "get_server_settings on the previous code's database: SqlInputError { error: Error { code:
/// Unknown, extended_code: 1 }, msg: \"no such column: fleet_supply_mode\", ...".
#[test]
fn a_database_from_the_previous_code_opens_with_its_world() {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let path = std::env::temp_dir().join(format!("hum_fleet_prev_{}_{nanos}.db", std::process::id()));
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(include_str!("../../../tests/fixtures/relay/relay_v0_1456.sql")).expect("the previous code's database loads from its dump");
        let has: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name = 'fleet_ledger')", [], |r| r.get(0)).unwrap();
        assert!(!has, "the fixture is from before the fleet ledger");
    }
    let db = Storage::open(&path).expect("the previous code's database opens");
    let s = db.get_server_settings().unwrap_or_else(|e| panic!("get_server_settings on the previous code's database: {e:?}"));
    assert_eq!((s.world_time_scale, s.message_retention_days), (24.0, 7), "the owner's settings are kept");
    assert_eq!(s.fleet_supply_mode, "unlimited", "and the fleet is unlimited");
    drop(db);
    let state = crate::relay::relay::RelayState::new(Storage::open(&path).unwrap());
    {
        let mut world = state.game_world.try_write().unwrap();
        assert_eq!((world.time_scale, world.fleet_supply), (24.0, FleetSupply::Unlimited));
        assert!(world.restore_from_db(&state.db), "the stored world restores");
        let (store, _) = store_of(&world);
        let stock = world.entities[&store].components["meals"].as_f64().unwrap();
        assert!((stock - 93.0).abs() < 0.01, "the store's stock carried over: {stock}");
        player_at_store(&mut world, "e11e00f0");
        let r = take_meal_and_record(&mut world, &state.db, "e11e00f0", store);
        assert_eq!(r["success"], true, "{r}");
        assert_eq!(world.entities[&store].components["meals"].as_f64(), Some(stock), "unlimited: the stock is not drawn on");
    }
    assert_eq!(lines_of(&state.db, "e11e00f0").len(), 1, "the meal is a line of the new ledger");
    drop(state);
    let _ = std::fs::remove_file(&path);
}

/// ERASING AN ACCOUNT DELETES ITS LEDGER (and the export lists it first): the erase leaves no
/// line of the key, says so in its receipt, and the left-rows check finds none; another
/// player's lines stay.
///
/// Seen red 2026-10-04 with the fleet_ledger DELETE left out of `delete_account`: "the receipt
/// says the ledger went: [(\"upload_files_removed\", 0), (\"messages\", 0), ... (\"ship_plots\",
/// 0), (\"game_progress\", 0)]" (no fleet_ledger line in it, and both lines still stored).
#[test]
fn erasing_an_account_deletes_its_ledger() {
    let (db, path) = temp_db("erase");
    let mut world = GameWorld::new();
    let (store, _) = store_of(&world);
    player_at_store(&mut world, "aaaa03");
    player_at_store(&mut world, "bbbb04");
    take_meal_and_record(&mut world, &db, "aaaa03", store);
    give(&world, &db, "aaaa03", &give_msg("g-e", store, "bread_0", 1));
    take_meal_and_record(&mut world, &db, "bbbb04", store);
    let export = db.export_account("aaaa03", "Erased");
    assert_eq!(export["fleet_ledger"].as_array().map(|a| a.len()), Some(2), "the export lists the ledger: {}", export["fleet_ledger"]);
    let receipt = db.delete_account("aaaa03", "Erased");
    assert!(receipt.iter().any(|(t, n)| t == "fleet_ledger" && *n == 2), "the receipt says the ledger went: {receipt:?}");
    let left = lines_of(&db, "aaaa03").len();
    assert_eq!(left, 0, "the erased account's ledger is gone / left: {left} / right: 0");
    assert!(!db.erase_left_rows("aaaa03"), "nothing of the account is left");
    assert_eq!(lines_of(&db, "bbbb04").len(), 1, "another player's ledger stays");
    drop(db);
    let _ = std::fs::remove_file(&path);
}
