//! The fleet ledger's engine half (2026-10-04): the server's answers into the Inventory page's
//! fleet panel, a give's items out of the backpack once the server recorded it, and the home's
//! reactor power reported while the player is in the shared world. The panel and its words are
//! gui/pages/fleet_ledger.rs; the server's side is relay/handlers/fleet_ledger.rs.
//!
//! ITEMS LEAVE ONCE. The backpack lives in this game, so a give is settled here: when the
//! server answers a give this game sent with success, its items are taken out of the backpack
//! and the give's id (`fleet-give:<id>`) is written into the backpack's settled set
//! (systems::inventory::TradeSettlements, saved with the backpack in the same WorldSave) IN THE
//! SAME STEP. A second answer for the same give (the server answers a repeated give `already`)
//! finds the id there and takes nothing. A refused give takes nothing. A save load that puts
//! back a backpack from before a give is settled again by `tick` (the give stays in `confirmed`
//! for the session), the way the Trade page settles a completed trade again (trade.rs `tick`).
//!
//! KNOWN GAPS: a give in flight when the game closes is not saved, so if the server recorded it
//! but the answer never arrived, its items stay in the backpack (the fleet counts the gift, the
//! player keeps the items: no one loses anything). And the server cannot see the backpack, so it
//! takes the game's word for what was given, until it holds inventories itself (increment 8 of
//! docs/design/ship-homes-and-logistics.md).
//!
//! POWER. The ship's reactor meters every watt-hour a home draws from it and returns to it
//! (systems::ship_power, `ShipSupplyLedger`). While the player is in a server's shared world,
//! `tick` sends what was drawn and returned since the last report, once a minute
//! (`game_fleet_power`), and the server writes it into the fleet ledger. The baseline is set at
//! each welcome, so power used while playing alone is never charged to a server's fleet.

use crate::engine::state::EngineState;
use crate::gui::pages::fleet_ledger::{FleetGive, FleetLedger, FleetStore, FleetTotals};
use crate::gui::GuiState;

/// Real seconds between power reports (the server refuses one under its own minimum, 30 s in
/// data/ship/fleet_ledger.ron).
pub const POWER_REPORT_INTERVAL_S: f32 = 60.0;

/// The settled-set id a give is recorded under beside the backpack.
pub fn settled_id(give_id: &str) -> String {
    format!("fleet-give:{give_id}")
}

/// A server's `game_*` message about the fleet; true when it was one (net_route.rs hands each
/// message here before its own match).
pub(crate) fn on_game_message(state: &mut EngineState, v: &serde_json::Value) -> bool {
    match v.get("type").and_then(|t| t.as_str()) {
        Some("game_fleet_ledger") => {
            if let Some(l) = FleetLedger::from_json(v) {
                state.gui_state.fleet.ledger = Some(l);
            }
            true
        }
        Some("game_fleet_give_result") => {
            on_give_result(&mut state.gui_state, &mut state.game_world.world, v);
            true
        }
        Some("game_fleet_totals") => {
            let f = |k: &str| v.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0);
            state.gui_state.fleet.totals = Some(FleetTotals { players: v.get("players").and_then(|x| x.as_i64()).unwrap_or(0), used: f("used_value"), contributed: f("contributed_value") });
            true
        }
        Some("game_fleet_power_result") => true,
        // A meal the panel asked for (an AI agent's take_meal answers the same way): say what
        // happened, and fetch the ledger with its new line.
        Some("game_interact_result") if v.get("action").and_then(|a| a.as_str()) == Some("take_meal") => {
            state.gui_state.fleet.status = meal_sentence(v);
            crate::gui::pages::fleet_ledger::request_ledger(&state.gui_state);
            true
        }
        _ => false,
    }
}

/// What a take_meal answer means, in a sentence.
pub fn meal_sentence(v: &serde_json::Value) -> String {
    if v.get("success").and_then(|s| s.as_bool()) == Some(true) {
        return "You took a meal from the ship's stores.".into();
    }
    match v.get("error").and_then(|e| e.as_str()).unwrap_or("") {
        "not_yet" => {
            let s = v.get("next_meal_in_s").and_then(|x| x.as_f64()).unwrap_or(0.0);
            format!("Your next meal is in {:.0} game minutes.", (s / 60.0).ceil())
        }
        "too_far" => "Walk closer to the stores to take a meal.".into(),
        "empty" => "The stores are empty: no meal until the farms fill them.".into(),
        "not_in_game" => "Join the shared world to take a meal.".into(),
        other => format!("No meal: {other}."),
    }
}

/// The server's answer to a give: on success its items leave the backpack once, and it is kept
/// as confirmed for the session; on a refusal nothing leaves. Either way it is no longer in
/// flight, and the panel says what happened.
pub(crate) fn on_give_result(gs: &mut GuiState, world: &mut hecs::World, v: &serde_json::Value) {
    let Some(give_id) = v.get("give_id").and_then(|x| x.as_str()) else { return };
    let ok = v.get("success").and_then(|x| x.as_bool()) == Some(true);
    let sent = gs.fleet.in_flight.iter().position(|g| g.give_id == give_id).map(|i| gs.fleet.in_flight.remove(i));
    let give = sent.or_else(|| gs.fleet.confirmed.iter().find(|g| g.give_id == give_id).cloned());
    let Some(give) = give else {
        // Not a give this game sent this session (another device of the same player): nothing
        // of this backpack is in it.
        return;
    };
    if !ok {
        let why = v.get("error").and_then(|x| x.as_str()).unwrap_or("refused");
        gs.fleet.status = format!("The fleet did not take {} {}: {}. Nothing left your backpack.", give.qty, give.name, refusal_words(why));
        return;
    }
    if !gs.fleet.confirmed.iter().any(|g| g.give_id == give.give_id) {
        gs.fleet.confirmed.push(give.clone());
    }
    gs.fleet.status = match settle_give(world, &give) {
        Some(0) => format!("You gave the fleet {} {}.", give.qty, give.name),
        Some(short) => format!("The fleet recorded {} {}; {short} of them had already left your backpack.", give.qty, give.name),
        None => gs.fleet.status.clone(),
    };
}

/// Plain words for a refusal code.
fn refusal_words(code: &str) -> &'static str {
    match code {
        "too_far" => "you were too far from the store",
        "no_price" => "the fleet has no price for it",
        "bad_quantity" => "that is more than one give can carry",
        "not_in_game" => "you are not in the shared world",
        "not_a_store" | "entity_not_found" => "that is not one of the fleet's stores",
        _ => "the server could not record it",
    }
}

/// Take a give's items out of the player's backpack, once: None when this backpack already
/// settled it (or there is no player), else how many were short (0 when all were there).
pub(crate) fn settle_give(world: &mut hecs::World, give: &FleetGive) -> Option<u32> {
    use crate::systems::inventory::{Inventory, TradeSettlements};
    let player = world.query::<(&Inventory, &crate::ecs::components::Controllable)>().iter().next().map(|(e, _)| e)?;
    if world.get::<&TradeSettlements>(player).is_err() {
        world.insert_one(player, TradeSettlements::default()).ok()?;
    }
    let id = settled_id(&give.give_id);
    let mut q = world.query_one::<(&mut Inventory, &mut TradeSettlements)>(player).ok()?;
    let (inv, ts) = q.get()?;
    if ts.knows(&id) {
        return None;
    }
    let short = inv.remove_worn(&give.item_id, give.qty, give.wear, give.quality);
    ts.settled.insert(id);
    Some(short)
}

/// A welcome to a shared world: its fleet stores, a fresh power baseline (nothing used before
/// this join is charged to this fleet), the ledger, and any give still waiting for an answer
/// sent again (the server answers a repeat with the line it already has).
pub(crate) fn on_welcome(state: &mut EngineState, v: &serde_json::Value) {
    let fleet = &mut state.gui_state.fleet;
    fleet.stores = stores_in(v);
    let t = crate::systems::ship_power::tally(&state.data_store, crate::systems::ship_power::PLAYER_HOME, crate::systems::ship_power::POWER);
    fleet.power_baseline = Some((t.drawn, t.returned));
    fleet.power_timer = 0.0;
    fleet.ledger = None;
    let resend: Vec<serde_json::Value> = fleet.in_flight.iter().map(crate::gui::pages::fleet_ledger::give_message).collect();
    if let Some(ws) = state.gui_state.ws_client.as_ref() {
        ws.send(&serde_json::json!({ "type": "game_fleet_ledger_request" }).to_string());
        for m in resend {
            ws.send(&m.to_string());
        }
    }
}

/// The fleet's stores in a welcome's world snapshot.
pub fn stores_in(v: &serde_json::Value) -> Vec<FleetStore> {
    let Some(snap) = v.get("world_snapshot").and_then(|s| s.as_array()) else { return Vec::new() };
    snap.iter()
        .filter(|e| e.get("entity_type").and_then(|t| t.as_str()) == Some("food_store"))
        .filter_map(|e| {
            let p = e.get("position")?.as_array()?;
            Some(FleetStore {
                entity_id: e.get("entity_id")?.as_u64()?,
                name: e.get("components").and_then(|c| c.get("name")).and_then(|n| n.as_str()).unwrap_or("The fleet's store").to_string(),
                position: [p.first()?.as_f64()? as f32, p.get(1)?.as_f64()? as f32, p.get(2)?.as_f64()? as f32],
            })
        })
        .collect()
}

/// Each frame while joined (lib.rs, beside the position send): where the player stands, the
/// fleet's prices once, gives the backpack must settle (again after a save load), and the power
/// report once a minute.
pub(crate) fn tick(state: &mut EngineState, real_dt: f32) {
    let p = state.camera.position;
    state.gui_state.fleet.my_position = Some([p.x, p.y, p.z]);
    if state.gui_state.fleet.prices.is_empty() {
        if let Some(reg) = state.data_store.get::<crate::systems::economy::TradeGoodsRegistry>("trade_goods_registry") {
            state.gui_state.fleet.prices = reg.goods.iter().map(|(id, g)| (id.clone(), f64::from(g.base_value))).collect();
        }
    }
    settle_confirmed(&state.gui_state, &mut state.game_world.world);
    if !state.game_welcomed {
        return;
    }
    let fleet = &mut state.gui_state.fleet;
    fleet.power_timer += real_dt;
    if fleet.power_timer < POWER_REPORT_INTERVAL_S {
        return;
    }
    fleet.power_timer = 0.0;
    let t = crate::systems::ship_power::tally(&state.data_store, crate::systems::ship_power::PLAYER_HOME, crate::systems::ship_power::POWER);
    let Some(msg) = power_report(&mut fleet.power_baseline, t.drawn, t.returned) else { return };
    if let Some(ws) = state.gui_state.ws_client.as_ref() {
        ws.send(&msg.to_string());
    }
}

/// Settle every give the server recorded this session into the backpack as it is now: a no-op
/// for each the backpack has settled, and the items out again for one a save load undid.
pub(crate) fn settle_confirmed(gs: &GuiState, world: &mut hecs::World) {
    for give in &gs.fleet.confirmed {
        settle_give(world, give);
    }
}

/// The power report since `baseline` (drawn and returned watt-hours already reported), moving
/// the baseline on; None when there is nothing to report. A tally that went DOWN (a save load
/// put back an older one) reports nothing and starts again from it.
pub fn power_report(baseline: &mut Option<(f64, f64)>, drawn: f64, returned: f64) -> Option<serde_json::Value> {
    let (d0, r0) = baseline.unwrap_or((drawn, returned));
    *baseline = Some((drawn, returned));
    if drawn < d0 || returned < r0 {
        return None;
    }
    let (d, r) = (drawn - d0, returned - r0);
    (d > 0.0 || r > 0.0).then(|| serde_json::json!({ "type": "game_fleet_power", "drawn_wh": d, "returned_wh": r }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::inventory::{Inventory, TradeSettlements};

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

    fn in_flight(gs: &mut GuiState, id: &str, qty: u32) {
        gs.fleet.in_flight.push(FleetGive { give_id: id.into(), store: 12, item_id: "bread_0".into(), name: "Bread".into(), qty, wear: 0, quality: 0 });
    }

    /// A GIVE'S ITEMS LEAVE THE BACKPACK EXACTLY ONCE: the server's yes takes them; the same
    /// yes again (the server answers a repeated give `already`), and the settling every frame
    /// does, take nothing more. A refusal takes nothing at all.
    ///
    /// Seen red 2026-10-04 with `settle_give` not checking the settled set: "the second answer
    /// took nothing more / left: 0 / right: 3" (the repeat and the next frame each took three
    /// more, down to none).
    #[test]
    fn a_gives_items_leave_the_backpack_exactly_once() {
        let (mut world, p) = world_with_bread(6);
        let mut gs = GuiState::default();
        in_flight(&mut gs, "give-a", 3);
        let yes = serde_json::json!({ "type": "game_fleet_give_result", "give_id": "give-a", "success": true, "already": false });
        on_give_result(&mut gs, &mut world, &yes);
        assert_eq!(bread(&world, p), 3, "the yes took three");
        assert!(gs.fleet.in_flight.is_empty() && gs.fleet.confirmed.len() == 1);
        let mut again = yes.clone();
        again["already"] = serde_json::json!(true);
        on_give_result(&mut gs, &mut world, &again);
        settle_confirmed(&gs, &mut world); // what tick does every frame
        assert_eq!(bread(&world, p), 3, "the second answer took nothing more / left: {} / right: 3", bread(&world, p));
        assert!(world.get::<&TradeSettlements>(p).unwrap().settled.contains(&settled_id("give-a")), "recorded beside the backpack");

        // A refusal takes nothing.
        in_flight(&mut gs, "give-b", 2);
        on_give_result(&mut gs, &mut world, &serde_json::json!({ "type": "game_fleet_give_result", "give_id": "give-b", "success": false, "error": "too_far" }));
        assert_eq!(bread(&world, p), 3, "a refused give took nothing");
        assert!(gs.fleet.in_flight.is_empty(), "and is no longer in flight");
        assert!(gs.fleet.status.contains("Nothing left your backpack"), "{}", gs.fleet.status);
        // An answer for a give this game never sent takes nothing either.
        on_give_result(&mut gs, &mut world, &serde_json::json!({ "type": "game_fleet_give_result", "give_id": "give-z", "success": true }));
        assert_eq!(bread(&world, p), 3);
    }

    /// A SAVE LOAD THAT PUTS THE GIVEN ITEMS BACK is settled again: the backpack and its
    /// settled set from before the give come back, and the next settle takes the items once.
    ///
    /// Seen red 2026-10-04 with `settle_confirmed` (tick's settling every frame) emptied, so a
    /// give was settled only on the server's answer: "the reloaded backpack keeps the loaves it
    /// gave / left: 6 / right: 3".
    #[test]
    fn a_save_load_that_puts_given_items_back_settles_again() {
        let (mut world, p) = world_with_bread(6);
        let mut gs = GuiState::default();
        in_flight(&mut gs, "give-c", 3);
        on_give_result(&mut gs, &mut world, &serde_json::json!({ "type": "game_fleet_give_result", "give_id": "give-c", "success": true }));
        assert_eq!(bread(&world, p), 3);
        // The save from before: six loaves, nothing settled.
        *world.get::<&mut Inventory>(p).unwrap() = {
            let mut inv = Inventory::new(20);
            inv.add_item("bread_0", 6, 50);
            inv
        };
        world.get::<&mut TradeSettlements>(p).unwrap().settled.clear();
        // Two frames.
        settle_confirmed(&gs, &mut world);
        settle_confirmed(&gs, &mut world);
        assert_eq!(bread(&world, p), 3, "the reloaded backpack keeps the loaves it gave / left: {} / right: 3", bread(&world, p));
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
        assert_eq!(meal_sentence(&serde_json::json!({ "success": true })), "You took a meal from the ship's stores.");
        assert_eq!(meal_sentence(&serde_json::json!({ "success": false, "error": "not_yet", "next_meal_in_s": 7100.0 })), "Your next meal is in 119 game minutes.");
    }
}
