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
    serde_json::json!({ "type": "game_fleet_give", "give_id": give_id, "home": "home-test", "entity_id": store, "item_id": item, "quantity": qty })
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
    assert!(data.file.home_service_watts > 0.0, "one home's service is read: {}", data.file.home_service_watts);
    assert_eq!(data.unit_price(data.kind(ITEM_CREATIVE).unwrap(), "bread_0"), Some(0.0), "a gift from Creative mode is worth nothing");
    assert_eq!(data.kind(ITEM_CREATIVE).unwrap().direction, Direction::Contributed);
    assert_eq!(data.meal_item(), Some("ration_basic_0"), "a meal is a Basic Ration");

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
    assert!(LedgerFile::parse(&text.replace("home_service_watts: 48000.0", "home_service_watts: 0.0")).is_err(), "a home must have a service");
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
    // Five game days at 72x are 24,000 ticks; at a new world's 1x, 1,728,000.
    super::super::ship_stores::at_simplified_speed(&mut world);
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
/// still saying it was): "a meal taken is one used line / left: 0 / right: 1". And seen red
/// the same day with `meal_item` left out of the reply (the review's finding 7: a meal charged
/// but never eaten): "the game is told what the meal is, to feed it: {...\"success\":true,...}
/// / left: Null / right: \"ration_basic_0\"".
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
    assert_eq!(r["meal_item"], "ration_basic_0", "the game is told what the meal is, to feed it: {r}");
    let lines = lines_of(&db, "e11e00b1");
    assert_eq!(lines.len(), 1, "a meal taken is one used line / left: {} / right: 1", lines.len());
    assert_eq!((lines[0].kind.as_str(), lines[0].direction.as_str(), lines[0].quantity, lines[0].value), (MEAL, "used", 1.0, 10.0));
    let day_start = (world.game_time / GAME_DAY_S).floor() * GAME_DAY_S;
    assert_eq!(lines[0].game_time, day_start, "it says which game day, on the game clock");
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
    assert_eq!((&again["quantity"], &again["value"]), (&r["quantity"], &r["value"]), "the same line");
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
/// second report sooner than the minimum is refused; a report claiming more than ONE HOME'S
/// electrical service could carry over the time since the last one is held to that (finding 6
/// of the 2026-10-04 review: it used to be the whole 35 MW reactor, about 63,000 CR a report);
/// a negative one is refused.
///
/// Seen red 2026-10-04 with the cap taken out: "a report of a year of power was held to the
/// reactor's output over the window: 3.066e8 kWh recorded, cap 4.2e4". Seen red again the
/// same day with the cap still the reactor's (finding 6): "a report of a year of power was
/// held to one home's service over the window: 4.2e4 kWh recorded, cap 5.76e1".
///
/// Run at the Simplified 72x, the clock these figures were chosen for: a real minute there is
/// 72 game minutes of a home's power. At a new world's 1x (real time since 2026-10-04) the
/// first report's 2 kWh is more than one home's service carries in the window's 120 game
/// seconds, 1.6 kWh, and is rightly clamped: seen 2026-10-04 when the default changed, "left:
/// (Some(true), Some(1.6), Some(true)) / right: (Some(true), Some(2.0), Some(false))".
#[test]
fn power_reports_are_kept_per_day_and_held_to_one_homes_service() {
    let (db, path) = temp_db("power");
    let mut world = GameWorld::new();
    super::super::ship_stores::at_simplified_speed(&mut world);
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
    let year_wh = 35.0e6 * 365.0 * 24.0;
    let r = power_report(&mut world, &db, key, &serde_json::json!({ "drawn_wh": year_wh }));
    let service = world.fleet_ledger.file.home_service_watts;
    let cap_kwh = service * 60.0 * world.time_scale / 3600.0 / 1000.0;
    let got = r["drawn_kwh"].as_f64().unwrap();
    assert!((got - cap_kwh).abs() < 1e-6, "a report of a year of power was held to one home's service over the window: {got:e} kWh recorded, cap {cap_kwh:e}");
    assert!(cap_kwh * 1.5 < 100.0, "one report of the most a home can draw is worth under 100 CR: {} CR", cap_kwh * 1.5);
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
    // With these two players and an admin who has no ledger, the totals are held back (two
    // others is under the three a sum needs: totals_are_withheld_below_the_minimum_players).
    let t = totals_json(&world, &db, "adm1n0");
    assert_eq!((t["players"].as_i64(), t["withheld"].as_bool()), (Some(2), Some(true)), "{t}");
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

/// THE LEDGER'S REPLIES CARRY NO SHARED ROW NUMBER (finding 8 of the 2026-10-04 review). The
/// table's row numbers count every player's lines, so the gap between two of one player's
/// numbers would say how many lines everyone else wrote in between. Two players write lines in
/// turn; no give answer and no ledger of either holds a field named for a row, nor a number
/// equal to a row id the OTHER player's writes took; nor does the account export.
///
/// Seen red 2026-10-04 with the row number put back in the give answer ("entry_id": e.id):
/// "aaaa05's reply carries a row number ([\"entry_id=1\"])"; and with it put back in the
/// export's SELECT: "the export lists no row numbers: [{...\"id\":1,...},{...\"id\":3,...},
/// {...\"id\":5,...}]" (the gaps are the other player's lines).
#[test]
fn ledger_replies_carry_no_shared_row_id() {
    let (db, path) = temp_db("rowid");
    let mut world = GameWorld::new();
    let (store, _) = store_of(&world);
    player_at_store(&mut world, "aaaa05");
    player_at_store(&mut world, "bbbb06");
    let mut replies = Vec::new();
    for round in 0..3 {
        replies.push(("aaaa05", give(&world, &db, "aaaa05", &give_msg(&format!("ga-{round}"), store, "bread_0", 1))));
        replies.push(("bbbb06", give(&world, &db, "bbbb06", &give_msg(&format!("gb-{round}"), store, "bread_0", 1))));
    }
    replies.push(("aaaa05", ledger_json(&world, &db, "aaaa05")));
    replies.push(("bbbb06", ledger_json(&world, &db, "bbbb06")));
    // Every row id each player's writes took.
    let ids_of = |who: &str| -> Vec<i64> {
        db.with_conn(|c| {
            let mut st = c.prepare("SELECT id FROM fleet_ledger WHERE public_key = ?1").unwrap();
            let ids: Vec<i64> = st.query_map([who], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect();
            ids
        })
    };
    // Every field in a reply named like a row number ("id", or a number ending "_id").
    fn row_fields(v: &serde_json::Value, out: &mut Vec<String>) {
        match v {
            serde_json::Value::Object(m) => {
                for (k, x) in m {
                    if k == "id" || (k.ends_with("_id") && x.is_number()) {
                        out.push(format!("{k}={x}"));
                    }
                    row_fields(x, out);
                }
            }
            serde_json::Value::Array(a) => a.iter().for_each(|x| row_fields(x, out)),
            _ => {}
        }
    }
    for (who, r) in &replies {
        let other = if *who == "aaaa05" { "bbbb06" } else { "aaaa05" };
        let mut fields = Vec::new();
        row_fields(r, &mut fields);
        assert!(fields.is_empty(), "{who}'s reply carries a row number ({fields:?}): {r}");
        for id in ids_of(other) {
            assert!(!r.to_string().contains(&format!("\"entry_id\":{id}")), "{who}'s reply carries a row id {other}'s writes moved: {r}");
        }
    }
    let export = db.export_account("aaaa05", "Erased");
    let listed = export["fleet_ledger"].as_array().cloned().unwrap_or_default();
    assert_eq!(listed.len(), 3, "the export lists the ledger: {}", export["fleet_ledger"]);
    assert!(listed.iter().all(|l| l.get("id").is_none()), "the export lists no row numbers: {}", export["fleet_ledger"]);
    drop(db);
    let _ = std::fs::remove_file(&path);
}

/// THE ADMIN'S TOTALS ARE HELD BACK BELOW THE MINIMUM OF OTHER PLAYERS (finding 9 of the
/// 2026-10-04 review). With one player besides the admin, total minus the admin's own lines is
/// that player's ledger, and asking each second shows each meal as it happens. The shipped
/// minimum is three other players: with two the answer says only that it is withheld and how
/// many are needed (no sums in it); with three it carries the sums. The admin's own ledger
/// does not count toward the three.
///
/// Seen red 2026-10-04 with the minimum check taken out of `totals_json`: "with two other
/// players the totals are withheld / left: Some(false) / right: Some(true)".
#[test]
fn totals_are_withheld_below_the_minimum_players() {
    let (db, path) = temp_db("totals");
    let mut world = GameWorld::new();
    let (store, _) = store_of(&world);
    assert_eq!(world.fleet_ledger.file.totals_min_other_players, 3, "the shipped minimum");
    for k in ["adm1n1", "aaaa07", "bbbb08", "cccc09"] {
        player_at_store(&mut world, k);
    }
    take_meal_and_record(&mut world, &db, "adm1n1", store);
    take_meal_and_record(&mut world, &db, "aaaa07", store);
    take_meal_and_record(&mut world, &db, "bbbb08", store);
    let t = totals_json(&world, &db, "adm1n1");
    assert_eq!(t["withheld"].as_bool(), Some(true), "with two other players the totals are withheld / left: {:?} / right: Some(true)", t["withheld"].as_bool());
    assert!(t.get("used_value").is_none() && t.get("contributed_value").is_none(), "and carry no sums: {t}");
    assert_eq!(t["others_needed"], 3, "{t}");
    take_meal_and_record(&mut world, &db, "cccc09", store);
    let t = totals_json(&world, &db, "adm1n1");
    assert_eq!((t["withheld"].as_bool(), t["players"].as_i64(), t["used_value"].as_f64()), (Some(false), Some(4), Some(40.0)), "three others: the sums: {t}");
    drop(db);
    let _ = std::fs::remove_file(&path);
}

/// A LINE STORES NO TIME FINER THAN A GAME DAY (finding 10 of the 2026-10-04 review). Two
/// meals in one game day, and a give between them, are stored with the same game time, the
/// start of that day; a meal the next game day carries the next day's start. Before, each line
/// kept the clock to the second, which with the clock's speed turns back into the real minute
/// the player ate.
///
/// Seen red 2026-10-04 with the rounding taken out of `LedgerData::line`: "two meals in one
/// game day are stored at the same time / left: [129600.0, 151200.0, 172799.0] / right: all
/// 86400".
#[test]
fn a_line_stores_no_time_finer_than_a_day() {
    let (db, path) = temp_db("day");
    let mut world = GameWorld::new();
    let (store, _) = store_of(&world);
    player_at_store(&mut world, "e11e00b6");
    world.game_time = GAME_DAY_S + 12.0 * 3600.0; // day 2, noon
    world.player_next_meal.clear();
    take_meal_and_record(&mut world, &db, "e11e00b6", store);
    world.game_time += 6.0 * 3600.0;
    give(&world, &db, "e11e00b6", &give_msg("g-day", store, "bread_0", 1));
    world.game_time += 6.0 * 3600.0 - 1.0; // 23:59:59 of day 2
    world.player_next_meal.clear();
    take_meal_and_record(&mut world, &db, "e11e00b6", store);
    let mut times: Vec<f64> = lines_of(&db, "e11e00b6").iter().map(|l| l.game_time).collect();
    times.reverse();
    assert_eq!(times.len(), 3);
    assert!(times.iter().all(|t| *t == GAME_DAY_S), "two meals in one game day are stored at the same time / left: {times:?} / right: all {GAME_DAY_S}");
    world.game_time += 1.0; // day 3
    world.player_next_meal.clear();
    take_meal_and_record(&mut world, &db, "e11e00b6", store);
    assert_eq!(lines_of(&db, "e11e00b6")[0].game_time, 2.0 * GAME_DAY_S, "the next day's line carries the next day's start");
    drop(db);
    let _ = std::fs::remove_file(&path);
}

/// A GIVE PAST THE DAY'S CAP IS REFUSED AND WRITES NOTHING (finding 11 of the 2026-10-04
/// review), through the relay's own give: with the cap set to 3 for the test (the shipped
/// file says 500), the fourth give is answered `too_many` and leaves no line, while a repeat
/// of a recorded give is still answered `already`.
///
/// Seen red 2026-10-04 with the kind's cap not passed into the line (`day_cap: None`): "the
/// fourth give of the day / left: null / right: \"too_many\"".
#[test]
fn gives_beyond_the_daily_cap_record_nothing() {
    let (db, path) = temp_db("cap");
    let mut world = GameWorld::new();
    assert_eq!(world.fleet_ledger.kind(ITEM).unwrap().max_lines_per_day, Some(500), "the shipped cap");
    for k in world.fleet_ledger.file.kinds.iter_mut() {
        if k.id == ITEM {
            k.max_lines_per_day = Some(3);
        }
    }
    let (store, _) = store_of(&world);
    player_at_store(&mut world, "e11e00b7");
    for n in 0..3 {
        assert_eq!(give(&world, &db, "e11e00b7", &give_msg(&format!("g-{n}"), store, "bread_0", 1))["success"], true);
    }
    let fourth = give(&world, &db, "e11e00b7", &give_msg("g-3", store, "bread_0", 1));
    assert_eq!(fourth["error"], "too_many", "the fourth give of the day / left: {} / right: \"too_many\"", fourth["error"]);
    assert_eq!(lines_of(&db, "e11e00b7").len(), 3, "it wrote nothing");
    assert_eq!(give(&world, &db, "e11e00b7", &give_msg("g-1", store, "bread_0", 1))["already"], true, "a repeat is still answered");
    drop(db);
    let _ = std::fs::remove_file(&path);
}

/// A GIFT FROM CREATIVE MODE IS RECORDED BUT NOT COUNTED (finding 5 of the 2026-10-04 review):
/// Creative mode makes things from nothing, so a give its game marks `creative` is a line of
/// "item_creative", worth 0 CR, and leaves the balance where it was. An item the fleet has no
/// price for is still refused, as in normal play.
///
/// Seen red 2026-10-04 with `creative` ignored in `give`: "a Creative-mode gift is worth
/// nothing / left: Some(9.0) / right: Some(0.0)" (three loaves at 3 CR).
#[test]
fn a_creative_gift_is_recorded_but_not_counted() {
    let (db, path) = temp_db("creative");
    let mut world = GameWorld::new();
    let (store, _) = store_of(&world);
    player_at_store(&mut world, "e11e00b8");
    let mut msg = give_msg("g-c", store, "bread_0", 3);
    msg["creative"] = serde_json::json!(true);
    let r = give(&world, &db, "e11e00b8", &msg);
    assert_eq!(r["success"].as_bool(), Some(true), "{r}");
    assert_eq!(r["value"].as_f64(), Some(0.0), "a Creative-mode gift is worth nothing / left: {:?} / right: Some(0.0)", r["value"].as_f64());
    assert_eq!(r["kind"].as_str(), Some(ITEM_CREATIVE), "{r}");
    let l = ledger_json(&world, &db, "e11e00b8");
    assert_eq!((l["contributed_value"].as_f64(), l["standing"].as_str()), (Some(0.0), Some("even")), "{l}");
    assert_eq!(l["recent"][0]["label"], "Given in Creative mode (not counted)");
    let mut unpriced = give_msg("g-c2", store, "no_such_item", 1);
    unpriced["creative"] = serde_json::json!(true);
    assert_eq!(give(&world, &db, "e11e00b8", &unpriced)["error"], "no_price");
    drop(db);
    let _ = std::fs::remove_file(&path);
}

/// A HOME'S GIVES COME BACK TO ITS GAME, AND A SHORT ONE IS CORRECTED ONCE (findings 1, 2, 3
/// and 12 of the 2026-10-04 review). Gives from home A and home B: asking for A's gets A's
/// alone, newest first, each with its item and count. The game then found only 1 of a 3-loaf
/// give to take: one adjust message corrects it to 1 loaf (its value with it), and the same
/// correction sent again changes nothing. A give with no home is refused.
///
/// Seen red 2026-10-04 with `fleet_gives_for_home` reading every home's gives (the home left
/// out of its WHERE): "home A's gives only / left: 3 / right: 2".
#[test]
fn a_homes_gives_come_back_and_a_short_one_is_corrected_once() {
    let (db, path) = temp_db("replay");
    let mut world = GameWorld::new();
    let (store, _) = store_of(&world);
    player_at_store(&mut world, "e11e00b9");
    let from = |id: &str, home: &str, qty: u64| {
        let mut m = give_msg(id, store, "bread_0", qty);
        m["home"] = serde_json::json!(home);
        give(&world, &db, "e11e00b9", &m)
    };
    from("g-a1", "home-a", 3);
    from("g-b1", "home-b", 1);
    from("g-a2", "home-a", 2);
    assert_eq!(from("g-x", "", 1)["error"], "bad_home", "a give with no home is refused");
    let a = gives_json(&world, &db, "e11e00b9", &serde_json::json!({ "home": "home-a" }));
    let ids: Vec<&str> = a["gives"].as_array().unwrap().iter().map(|g| g["give_id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), 2, "home A's gives only / left: {} / right: 2", ids.len());
    assert_eq!(ids, vec!["g-a2", "g-a1"], "newest first: {a}");
    assert_eq!((a["gives"][1]["item_id"].as_str(), a["gives"][1]["quantity"].as_f64()), (Some("bread_0"), Some(3.0)));
    let bread = world.fleet_ledger.goods["bread_0"].base_value;
    let fix = serde_json::json!({ "adjustments": [{ "give_id": "g-a1", "delivered": 1 }, { "give_id": "g-nope", "delivered": 0 }] });
    let r = adjust(&db, "e11e00b9", &fix);
    assert_eq!((r["results"][0]["outcome"].as_str(), r["results"][1]["outcome"].as_str()), (Some("changed"), Some("not_found")), "{r}");
    assert_eq!(adjust(&db, "e11e00b9", &fix)["results"][0]["outcome"], "unchanged", "the same correction again changes nothing");
    let l = ledger_json(&world, &db, "e11e00b9");
    assert_eq!(l["contributed_value"].as_f64(), Some((1.0 + 1.0 + 2.0) * bread), "the short give counts 1 loaf: {l}");
    assert_eq!(gives_json(&world, &db, "e11e00b9", &serde_json::json!({ "home": "" }))["error"], "bad_home");
    drop(db);
    let _ = std::fs::remove_file(&path);
}

/// A GIVE TURNED AWAY BY THE RATE LIMIT IS ANSWERED WITH ITS ID (findings 4 and 13 of the
/// 2026-10-04 review), so the game sends it again instead of waiting for an answer that never
/// comes. The relay's own answer for it is a give result, not the bare game_error.
///
/// Seen red 2026-10-04 with `rate_limited` answering without the id: "the refusal names the
/// give / left: null / right: \"g-fast\"".
#[test]
fn a_rate_limited_give_is_answered_with_its_id() {
    let r = rate_limited(&give_msg("g-fast", 12, "bread_0", 1));
    assert_eq!(r["give_id"], "g-fast", "the refusal names the give / left: {} / right: \"g-fast\"", r["give_id"]);
    assert_eq!((r["type"].as_str(), r["success"].as_bool(), r["error"].as_str()), (Some("game_fleet_give_result"), Some(false), Some("rate_limited")));
}

/// THE GIVE LIMIT READS THE TEST'S CLOCK, NOT THE WALL (BUG-152). The 200 ms limit reads
/// `RelayState::perception_now`, which a test can point at a clock it moves by hand, and the
/// end-to-end test (features.rs `the_fleet_ledger_end_to_end`) relies on that to make "two
/// gives sent at once" 0 ms apart however long the relay takes over the first. If the limit
/// went back to reading the wall, that test would still pass on an idle machine and fail again
/// on a busy one; this is the test that says why.
///
/// Seen red 2026-10-05 with `perception_now` reading `Instant::now()` whatever the test set:
/// "250 ms of wall time passed but the limit's clock did not move, so this is still inside the
/// 200 ms".
#[test]
fn the_give_limit_reads_the_test_clock_not_the_wall() {
    use crate::relay::handlers::msg_handlers::perception_rate_allows;
    use std::time::Duration;
    let (db, path) = temp_db("limit_clock");
    let state = Arc::new(RelayState::new(db));
    let clock = crate::test_clock::ManualClock::new();
    assert!(state.perception_clock.set(clock.clone()).is_ok());

    assert!(perception_rate_allows(&state, "k", "fleet_give"), "the first give is let through");
    std::thread::sleep(Duration::from_millis(250));
    assert!(
        !perception_rate_allows(&state, "k", "fleet_give"),
        "250 ms of wall time passed but the limit's clock did not move, so this is still inside the 200 ms"
    );
    clock.advance(Duration::from_millis(199));
    assert!(!perception_rate_allows(&state, "k", "fleet_give"), "199 ms on the limit's clock is inside the limit");
    clock.advance(Duration::from_millis(1));
    assert!(perception_rate_allows(&state, "k", "fleet_give"), "200 ms on the limit's clock is past it");
    drop(state);
    let _ = std::fs::remove_file(&path);
}
