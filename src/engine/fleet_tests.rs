//! Tests of the fleet ledger's engine half (fleet.rs). Each was seen to fail before it passed;
//! the failure text is in its comment.

use super::*;
use crate::gui::pages::fleet_ledger::FleetGive;
use crate::systems::inventory::{FleetHeld, Inventory, TradeSettlements};

fn world_with_bread(n: u32) -> (hecs::World, hecs::Entity) {
    let mut world = hecs::World::new();
    let mut inv = Inventory::new(20);
    inv.add_item("bread_0", n, 50);
    let p = world.spawn((inv, crate::ecs::components::Controllable));
    (world, p)
}

fn bread(world: &hecs::World, p: hecs::Entity) -> u32 {
    world.get::<&Inventory>(p).unwrap().count_item("bread_0")
}

fn ts(world: &hecs::World, p: hecs::Entity) -> TradeSettlements {
    world.get::<&TradeSettlements>(p).map(|t| (*t).clone()).unwrap_or_default()
}

fn ask(gs: &mut GuiState, id: &str, qty: u32) {
    gs.fleet.outbox.push(FleetGive { give_id: id.into(), store: 12, item_id: "bread_0".into(), name: "Bread".into(), qty, wear: 0, quality: 0 });
}

/// A game connected to "wss://here", NOT in Creative mode (GuiState's default is the Dev play
/// mode's, Creative on, whose gifts the fleet records without counting).
fn here() -> GuiState {
    let mut gs = GuiState::default();
    gs.connected_server_url = "wss://here".into();
    gs.creative_mode = false;
    gs
}

fn answer(id: &str, ok: bool, error: &str) -> serde_json::Value {
    serde_json::json!({ "type": "game_fleet_give_result", "give_id": id, "success": ok, "error": error })
}

/// The InventorySystem's frame: it applies the queued moves (a refused give's items going back).
fn inventory_frame(world: &mut hecs::World) {
    use crate::ecs::systems::System;
    crate::systems::inventory::InventorySystem::new().tick(world, 0.016, &crate::hot_reload::data_store::DataStore::new());
}

/// A GIVE'S ITEMS ARE IN ONE PLACE AT A TIME (finding 1 of the 2026-10-04 review): asked, the
/// loaves leave the backpack at once and are HELD with the give (so nothing else, a trade, a
/// move to storage, a meal, another give, can take them too); the server's yes keeps them given
/// and records the give beside the backpack, and the same yes again takes nothing; a refusal
/// puts them back in the backpack.
///
/// Seen red 2026-10-04 with `process_outbox` holding the give without taking the loaves out
/// (the old way: out only on the server's yes, so in flight they were still in the backpack
/// for a trade or storage to take): "a held give's loaves are out of the backpack / left: 6 /
/// right: 3".
#[test]
fn a_gives_items_are_in_one_place_at_a_time() {
    let (mut world, p) = world_with_bread(6);
    let mut gs = here();
    ask(&mut gs, "give-a", 3);
    process_outbox(&mut gs, &mut world);
    assert_eq!(bread(&world, p), 3, "a held give's loaves are out of the backpack / left: {} / right: 3", bread(&world, p));
    assert_eq!(ts(&world, p).fleet_held.len(), 1, "and held with the give");
    assert_eq!(gs.fleet.held[0].server, "wss://here", "for this server");
    on_give_result(&mut gs, &mut world, &answer("give-a", true, ""));
    on_give_result(&mut gs, &mut world, &answer("give-a", true, ""));
    inventory_frame(&mut world);
    assert_eq!(bread(&world, p), 3, "the yes, twice, kept three given");
    let t = ts(&world, p);
    assert!(t.fleet_held.is_empty() && t.settled.contains(&settled_id("give-a")), "{t:?}");
    assert_eq!(gs.fleet.status, "You gave the fleet 3 Bread.");

    // A refusal puts them back.
    ask(&mut gs, "give-b", 2);
    process_outbox(&mut gs, &mut world);
    assert_eq!(bread(&world, p), 1);
    on_give_result(&mut gs, &mut world, &answer("give-b", false, "too_far"));
    inventory_frame(&mut world);
    assert_eq!(bread(&world, p), 3, "a refused give's loaves are back");
    assert!(gs.fleet.status.contains("going back into your backpack"), "{}", gs.fleet.status);
    assert!(gs.fleet.held.is_empty());
}

/// A GIVE OF LOAVES PROMISED TO A CONFIRMED TRADE IS REFUSED BEFORE ANYTHING MOVES (finding 1
/// of the 2026-10-04 review): the player confirmed a trade offering 5 of their 5 loaves, so
/// when the other player confirms, their game receives 5; a give of the same loaves would have
/// the fleet credit what the trade hands over.
///
/// Seen red 2026-10-04 with the trade promise left out of `process_outbox`'s count: "the
/// promised loaves stay in the backpack / left: 0 / right: 5".
#[test]
fn a_give_of_loaves_promised_to_a_trade_is_refused() {
    let (mut world, p) = world_with_bread(5);
    let mut gs = here();
    gs.profile_public_key = "me".into();
    let offer = crate::gui::GuiTradeItem { item_type: "item".into(), name: "Bread".into(), quantity: 5, reference_id: Some("bread_0".into()), ..Default::default() };
    gs.trades = vec![crate::gui::GuiTrade { id: "t-1".into(), initiator_key: "me".into(), recipient_key: "them".into(), status: "active".into(), initiator_items: vec![offer], initiator_confirmed: true, ..Default::default() }];
    ask(&mut gs, "give-t", 5);
    process_outbox(&mut gs, &mut world);
    assert_eq!(bread(&world, p), 5, "the promised loaves stay in the backpack / left: {} / right: 5", bread(&world, p));
    assert!(ts(&world, p).fleet_held.is_empty());
    assert!(gs.fleet.status.starts_with("Only 0 Bread are free to give"), "{}", gs.fleet.status);
}

/// A GIVE FROM ONE HOME NEVER SETTLES INTO ANOTHER (findings 2 and 12 of the 2026-10-04
/// review). Home A gives 3 loaves and the server records it. Then home B's save is loaded (its
/// own backpack of 6 and its own settled set, under its own home id), and the server's list of
/// home A's gives arrives: nothing is taken from B. Asking for gives now asks for B's.
///
/// Seen red 2026-10-04 with the home check taken out of `settle_listed`: "home B keeps its
/// loaves / left: 3 / right: 6".
#[test]
fn a_give_from_one_home_never_settles_into_another() {
    let (mut world, p) = world_with_bread(6);
    let mut gs = here();
    let home_a = home_id(&mut world).unwrap();
    ask(&mut gs, "give-a", 3);
    process_outbox(&mut gs, &mut world);
    on_give_result(&mut gs, &mut world, &answer("give-a", true, ""));
    // Home B's save put back over the live backpack, as save_load does with `rewound`.
    *world.get::<&mut Inventory>(p).unwrap() = {
        let mut inv = Inventory::new(20);
        inv.add_item("bread_0", 6, 50);
        inv
    };
    *world.get::<&mut TradeSettlements>(p).unwrap() = TradeSettlements { home_id: "home-b".into(), fleet_recheck: true, ..Default::default() };
    let list = vec![serde_json::json!({ "give_id": "give-a", "item_id": "bread_0", "quantity": 3.0 })];
    let (taken, _) = settle_listed(&mut world, &home_a, &list);
    assert_eq!(bread(&world, p), 6, "home B keeps its loaves / left: {} / right: 6", bread(&world, p));
    assert_eq!(taken, 0);
    let asked = ask_for_gives(&mut gs, &mut world).expect("the gives of the home just loaded are asked for");
    assert_eq!(asked["home"], "home-b", "{asked}");
}

/// AN ANSWERED GIVE THE SAVE LOST IS SETTLED AGAIN, ONCE (finding 3 of the 2026-10-04 review).
/// Through the relay's own functions: home A gives 3 loaves, the relay records it and the game
/// settles it. Then the game closes before saving (or a snapshot from before the give is put
/// back): the backpack is 6 again and its settled set empty, same home. The save load asks for
/// the home's gives again, the relay's list arrives, and 3 loaves leave once; the list again
/// takes nothing. And when only 1 of a give's loaves is there, the correction sent back makes
/// the relay count 1.
///
/// Seen red 2026-10-04 with `settle_listed` marking a listed give settled without taking its
/// loaves: "the reloaded backpack gives its loaves again / left: 6 / right: 3".
#[test]
fn an_answered_give_the_save_lost_is_settled_again_once() {
    use crate::relay::handlers::{fleet_ledger as relay_fleet, game_state::GameWorld};
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let path = std::env::temp_dir().join(format!("hum_fleet_replay_{}_{nanos}.db", std::process::id()));
    let db = crate::relay::storage::Storage::open(&path).unwrap();
    let mut rw = GameWorld::new();
    let snapshot: Vec<serde_json::Value> = rw.snapshot().iter().map(|e| serde_json::to_value(e).unwrap()).collect();
    let store = stores_in(&serde_json::json!({ "world_snapshot": snapshot }))[0].clone();
    rw.spawn_player("e11e00c1", [store.position[0] + 1.0, 1.7, store.position[2]]);

    let (mut world, p) = world_with_bread(6);
    let mut gs = here();
    let home = home_id(&mut world).unwrap();
    gs.fleet.outbox.push(FleetGive { give_id: "give-r".into(), store: store.entity_id, item_id: "bread_0".into(), name: "Bread".into(), qty: 3, wear: 0, quality: 0 });
    process_outbox(&mut gs, &mut world);
    let sent = send_due(&mut gs, &mut world, 1.0).expect("the held give is sent");
    on_give_result(&mut gs, &mut world, &relay_fleet::give(&rw, &db, "e11e00c1", &sent));
    assert_eq!(bread(&world, p), 3);

    // The save from before the give comes back: six loaves, nothing settled, same home.
    *world.get::<&mut Inventory>(p).unwrap() = {
        let mut inv = Inventory::new(20);
        inv.add_item("bread_0", 6, 50);
        inv
    };
    *world.get::<&mut TradeSettlements>(p).unwrap() = TradeSettlements { home_id: home.clone(), fleet_recheck: true, ..Default::default() };
    gs.fleet.gives_asked_for = Some(home.clone());
    let req = ask_for_gives(&mut gs, &mut world).expect("a save put back asks again even for the same home");
    let list = relay_fleet::gives_json(&rw, &db, "e11e00c1", &req);
    let gives = list["gives"].as_array().cloned().unwrap();
    settle_listed(&mut world, &home, &gives);
    assert_eq!(bread(&world, p), 3, "the reloaded backpack gives its loaves again / left: {} / right: 3", bread(&world, p));
    settle_listed(&mut world, &home, &gives);
    assert_eq!(bread(&world, p), 3, "once");

    // Only 1 loaf there this time: the correction makes the fleet count 1.
    *world.get::<&mut Inventory>(p).unwrap() = {
        let mut inv = Inventory::new(20);
        inv.add_item("bread_0", 1, 50);
        inv
    };
    world.get::<&mut TradeSettlements>(p).unwrap().settled.clear();
    let (taken, adjustments) = settle_listed(&mut world, &home, &gives);
    assert_eq!((taken, adjustments.len()), (1, 1));
    assert_eq!(adjustments[0]["delivered"], 1, "{adjustments:?}");
    relay_fleet::adjust(&db, "e11e00c1", &serde_json::json!({ "adjustments": adjustments }));
    let l = relay_fleet::ledger_json(&rw, &db, "e11e00c1");
    assert_eq!(l["contributed_value"].as_f64(), Some(rw.fleet_ledger.goods["bread_0"].base_value), "the fleet counts the 1 loaf: {l}");
    drop(db);
    let _ = std::fs::remove_file(&path);
}

/// A GIVE TURNED AWAY FOR COMING TOO SOON IS SENT AGAIN (findings 4 and 13 of the 2026-10-04
/// review): the relay answers it `rate_limited` with its id; the give stays held, is no longer
/// counted as sent, and goes again after a short wait, the same give.
///
/// Seen red 2026-10-04 with `rate_limited` treated as any refusal: "a give turned away for
/// coming too soon is still held / left: 0 / right: 1".
#[test]
fn a_give_turned_away_for_coming_too_soon_is_sent_again() {
    let (mut world, _) = world_with_bread(6);
    let mut gs = here();
    ask(&mut gs, "give-f", 1);
    process_outbox(&mut gs, &mut world);
    assert!(send_due(&mut gs, &mut world, 1.0).is_some());
    on_give_result(&mut gs, &mut world, &answer("give-f", false, "rate_limited"));
    assert_eq!(gs.fleet.held.len(), 1, "a give turned away for coming too soon is still held / left: {} / right: 1", gs.fleet.held.len());
    assert!(gs.fleet.sent.is_empty());
    assert!(send_due(&mut gs, &mut world, 0.1).is_none(), "not at once");
    let again = send_due(&mut gs, &mut world, GIVE_RETRY_S).expect("sent again after the wait");
    assert_eq!(again["give_id"], "give-f");
}

/// HELD GIVES GO OUT ONE AT A TIME, SPACED PAST THE RELAY'S 200 MS, AND ONLY TO THEIR OWN
/// SERVER (findings 13 and 16 of the 2026-10-04 review): after a reconnect two held gives are
/// sent one per `GIVE_SPACING_S`, not back to back; a give held for another server is never
/// sent here.
///
/// Seen red 2026-10-04 with the server filter taken out of `send_due` (the give held for
/// another server, first in the list, went first): "the first held for this server / left:
/// String(\"give-x\") / right: \"give-1\"".
#[test]
fn held_gives_go_out_one_at_a_time_and_only_to_their_server() {
    let (mut world, p) = world_with_bread(6);
    let mut gs = here();
    {
        let mut t = TradeSettlements::default();
        let held = |id: &str, server: &str| FleetHeld { give_id: id.into(), server: server.into(), store: 12, item_id: "bread_0".into(), name: "Bread".into(), qty: 1, ..Default::default() };
        t.fleet_held = vec![held("give-x", "wss://there"), held("give-1", "wss://here"), held("give-2", "wss://here")];
        world.insert_one(p, t).unwrap();
    }
    mirror_held(&mut gs, &world);
    let first = send_due(&mut gs, &mut world, GIVE_SPACING_S).expect("the first goes");
    assert_eq!(first["give_id"], "give-1", "the first held for this server");
    assert!(send_due(&mut gs, &mut world, 0.0).is_none(), "not two back to back");
    let second = send_due(&mut gs, &mut world, GIVE_SPACING_S).expect("the second after the spacing");
    assert_eq!(second["give_id"], "give-2");
    let third = send_due(&mut gs, &mut world, GIVE_SPACING_S);
    assert_eq!(third, None, "a give held for another server is never sent here / left: {third:?} / right: None");
}

/// A MEAL TAKEN IS EATEN (finding 7 of the 2026-10-04 review): the server's yes names the good
/// a meal is, and the game puts one in the backpack and asks for it to be eaten (the Eat
/// button's path), so the meal charged to the ledger feeds the player; the panel says so.
///
/// Seen red 2026-10-04 with `feed` not asking for the meal to be eaten: "the meal is eaten /
/// left: None / right: Some(\"ration_basic_0\")".
#[test]
fn a_meal_taken_is_eaten() {
    let (mut world, p) = world_with_bread(0);
    let mut gs = here();
    assert!(feed(&mut world, &mut gs, "ration_basic_0", 20));
    assert_eq!(gs.pending_consume_item.as_deref(), Some("ration_basic_0"), "the meal is eaten / left: {:?} / right: Some(\"ration_basic_0\")", gs.pending_consume_item);
    assert_eq!(world.get::<&Inventory>(p).unwrap().count_item("ration_basic_0"), 1, "from the backpack, where the Eat path takes it");
    let yes = serde_json::json!({ "success": true, "meal_item": "ration_basic_0" });
    assert_eq!(meal_sentence(&yes, true), "You ate a meal from the ship's stores.");
}

/// POWER IS REPORTED AS WHAT CHANGED SINCE THE LAST REPORT: nothing at the baseline, the
/// difference after, and a tally that went down (a save load put back an older one: here the
/// drawn side fell while the returned side rose) reports nothing and starts from it, never a
/// negative report.
///
/// Seen red 2026-10-04 with the went-down check taken out: "a tally that went down reports
/// nothing: Some(Object {\"drawn_wh\": Number(-1400.0), \"returned_wh\": Number(40.0),
/// \"type\": String(\"game_fleet_power\")})".
#[test]
fn power_is_reported_since_the_last_report() {
    let mut base = Some((1000.0, 200.0));
    assert!(power_report(&mut base, 1000.0, 200.0).is_none(), "nothing new");
    let m = power_report(&mut base, 1500.0, 260.0).expect("a report");
    assert_eq!((m["drawn_wh"].as_f64(), m["returned_wh"].as_f64()), (Some(500.0), Some(60.0)));
    let down = power_report(&mut base, 100.0, 300.0);
    assert!(down.is_none(), "a tally that went down reports nothing: {down:?}");
    assert_eq!(base, Some((100.0, 300.0)), "and starts again from it");
    let m = power_report(&mut base, 150.0, 300.0).unwrap();
    assert_eq!(m["drawn_wh"].as_f64(), Some(50.0));
}

/// THE GAME READS WHAT THE RELAY REALLY SENDS (not JSON typed for the test): a relay world
/// and database in this process, a meal taken and a give made through the relay's own
/// functions, the relay's welcome snapshot, its give answer and its ledger message handed
/// to the game's readers. The stores are found, the meal names its good, the give's loaves
/// leave the backpack once (a Creative-mode give marked so), and the ledger reads as the relay
/// wrote it.
///
/// Seen red 2026-10-04 with the game reading the ledger's totals under "used" and
/// "contributed" (the relay writes "used_value" and "contributed_value"): "the game reads
/// the relay's totals / left: (0.0, 0.0) / right: (10.0, 6.0)".
#[test]
fn the_game_reads_what_the_relay_really_sends() {
    use crate::relay::handlers::{fleet_ledger as relay_fleet, game_state::GameWorld};
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let path = std::env::temp_dir().join(format!("hum_fleet_native_{}_{nanos}.db", std::process::id()));
    let db = crate::relay::storage::Storage::open(&path).unwrap();
    let mut rw = GameWorld::new();
    let snapshot: Vec<serde_json::Value> = rw.snapshot().iter().map(|e| serde_json::to_value(e).unwrap()).collect();
    let stores = stores_in(&serde_json::json!({ "world_snapshot": snapshot }));
    assert_eq!(stores.len(), 1, "the game finds the relay's store in its welcome: {stores:?}");
    let store = stores[0].clone();
    rw.spawn_player("e11e00c0", [store.position[0] + 1.0, 1.7, store.position[2]]);
    let meal = relay_fleet::take_meal_and_record(&mut rw, &db, "e11e00c0", store.entity_id);
    assert_eq!(meal["meal_item"], "ration_basic_0", "the meal names its good: {meal}");
    assert_eq!(meal_sentence(&meal, true), "You ate a meal from the ship's stores.");

    // The give, as the game sends it, answered by the relay, settled by the game.
    let (mut world, p) = world_with_bread(6);
    let mut gs = here();
    gs.fleet.outbox.push(FleetGive { give_id: "give-real".into(), store: store.entity_id, item_id: "bread_0".into(), name: "Bread".into(), qty: 2, wear: 0, quality: 0 });
    process_outbox(&mut gs, &mut world);
    let msg = send_due(&mut gs, &mut world, 1.0).expect("sent");
    assert_eq!(msg["home"].as_str().map(|h| h.starts_with("home-")), Some(true), "with its home: {msg}");
    let answer = relay_fleet::give(&rw, &db, "e11e00c0", &msg);
    on_give_result(&mut gs, &mut world, &answer);
    let again = relay_fleet::give(&rw, &db, "e11e00c0", &msg);
    on_give_result(&mut gs, &mut world, &again);
    assert_eq!(bread(&world, p), 4, "two loaves left the backpack, once: {answer}");

    // A Creative-mode give is sent marked, and the relay records it at 0.
    gs.creative_mode = true;
    gs.fleet.outbox.push(FleetGive { give_id: "give-c".into(), store: store.entity_id, item_id: "bread_0".into(), name: "Bread".into(), qty: 1, wear: 0, quality: 0 });
    process_outbox(&mut gs, &mut world);
    let msg = send_due(&mut gs, &mut world, 1.0).expect("sent");
    assert_eq!(msg["creative"], true, "{msg}");
    let answer = relay_fleet::give(&rw, &db, "e11e00c0", &msg);
    on_give_result(&mut gs, &mut world, &answer);
    assert!(gs.fleet.status.contains("not counted"), "{}", gs.fleet.status);

    let l = crate::gui::pages::fleet_ledger::FleetLedger::from_json(&relay_fleet::ledger_json(&rw, &db, "e11e00c0")).expect("the ledger reads");
    assert_eq!((l.used, l.contributed), (10.0, 6.0), "the game reads the relay's totals / left: {:?} / right: (10.0, 6.0)", (l.used, l.contributed));
    assert_eq!((l.standing.as_str(), l.supply.as_str()), ("red", "unlimited"));
    assert_eq!(l.recent.len(), 3);
    assert_eq!(l.recent[1].item_name.as_deref(), Some("Bread"));
    drop(db);
    let _ = std::fs::remove_file(&path);
}

/// The welcome's snapshot names the fleet's stores, and a take_meal answer reads in words.
///
/// Seen red 2026-10-04 with `stores_in` looking for an entity type the relay does not send
/// ("fleet_store"): "left: [] / right: [FleetStore { entity_id: 12, ... }]".
#[test]
fn the_welcome_names_the_stores_and_a_meal_answer_reads() {
    let w = serde_json::json!({ "world_snapshot": [
        { "entity_id": 12, "entity_type": "food_store", "position": [67.0, 1.0, 22.0], "components": { "name": "The mess hall's stores" } },
        { "entity_id": 13, "entity_type": "dining_table", "position": [70.0, 1.0, 22.0], "components": {} },
    ]});
    let s = stores_in(&w);
    assert_eq!(s, vec![FleetStore { entity_id: 12, name: "The mess hall's stores".into(), position: [67.0, 1.0, 22.0] }]);
    assert_eq!(meal_sentence(&serde_json::json!({ "success": true }), false), "The ship's stores gave you a meal.");
    assert_eq!(meal_sentence(&serde_json::json!({ "success": false, "error": "not_yet", "next_meal_in_s": 7100.0 }), false), "Your next meal is in 119 game minutes.");
}
