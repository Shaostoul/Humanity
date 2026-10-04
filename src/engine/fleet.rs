//! The fleet ledger's engine half (2026-10-04): the server's answers into the Inventory page's
//! fleet panel, a give's items out of the backpack, a meal eaten, and the home's reactor power
//! reported while the player is in the shared world. The panel and its words are
//! gui/pages/fleet_ledger.rs; the server's side is relay/handlers/fleet_ledger.rs.
//!
//! A GIVE'S ITEMS ARE IN ONE PLACE AT A TIME (reworked after the review of 2026-10-04,
//! findings 1, 2, 3, 4, 12, 13 and 16). The backpack lives in this game, so a give is settled
//! here, in three steps:
//!   1. ASKED: the panel puts the give in `fleet.outbox`; nothing has moved yet.
//!   2. HELD: the next frame (`process_outbox`) takes the items OUT of the backpack and keeps
//!      them with the give in the player's `TradeSettlements.fleet_held`, saved with the
//!      backpack. Held items are not in the backpack, so nothing else (a trade, a move to
//!      storage, eating, another give) can take them as well; a give of items that are
//!      promised to a confirmed trade or already on their way out is refused before they move.
//!      The held give goes to the server it was given at, one give at a time, spaced past the
//!      relay's 200 ms between gives (`send_due`).
//!   3. ANSWERED: a yes drops the held items for good and records the give's id
//!      (`fleet-give:<id>`) in the backpack's settled set in the same step; a refusal puts them
//!      back in the backpack (queued like a trade's moves, so a full backpack sends the rest to
//!      home storage); "too soon" (the relay's rate limit, answered with the give's id) leaves
//!      it held and sends it again a moment later.
//! A held give belongs to one server (`FleetHeld.server`) and is never sent to another.
//!
//! SETTLED AGAIN AFTER A SAVE LOAD (`on_gives`). Every give carries the id of the home its
//! backpack belongs to (`TradeSettlements.home_id`, saved with it). After each welcome, and
//! whenever a save is put back over the backpack (`fleet_recheck`, set by save_load), the game
//! asks the server for THIS home's recorded gives, and any the loaded backpack has not settled
//! (the game closed before it saved, or a snapshot from before the give was restored) is
//! taken out of the backpack again, once. When fewer were there to take, one
//! `game_fleet_give_adjust` tells the server, which then counts only what was delivered. A
//! different home (another character's save) has another id, so another home's gives are
//! never taken from it.
//!
//! WHAT IS STILL TAKEN ON TRUST: the server cannot see the backpack, so it takes this game's
//! word for what was given, until it holds inventories itself (increment 8 of
//! docs/design/ship-homes-and-logistics.md). Gifts made in Creative mode are sent marked
//! `creative`, and the server records them without counting them.
//!
//! A MEAL. The server's answer to `take_meal` names the good a meal is (`meal_item`, a Basic
//! Ration); the game puts one in the backpack and eats it at once (the Eat button's path), so a
//! meal charged to the ledger is a meal eaten.
//!
//! POWER. The ship's reactor meters every watt-hour a home draws from it and returns to it
//! (systems::ship_power, `ShipSupplyLedger`). While the player is in a server's shared world,
//! `tick` sends what was drawn and returned since the last report, once a minute
//! (`game_fleet_power`), and the server writes it into the fleet ledger. The baseline is set at
//! each welcome, so power used while playing alone is never charged to a server's fleet.

use crate::engine::state::EngineState;
use crate::gui::pages::fleet_ledger::{FleetLedger, FleetStore, FleetTotals};
use crate::gui::GuiState;
use crate::systems::inventory::{FleetHeld, Inventory, TradeSettlements, TransferOp};

/// Real seconds between power reports (the server refuses one under its own minimum, 30 s in
/// data/ship/fleet_ledger.ron).
pub const POWER_REPORT_INTERVAL_S: f32 = 60.0;
/// Real seconds between two gives sent to the server: past the relay's 200 ms between two of
/// one player's gives, so a backlog (several held gives after a reconnect) is not turned away.
pub const GIVE_SPACING_S: f32 = 0.3;
/// Real seconds before sending again a give the relay turned away for coming too soon.
pub const GIVE_RETRY_S: f32 = 0.5;

/// The settled-set id a give is recorded under beside the backpack.
pub fn settled_id(give_id: &str) -> String {
    format!("fleet-give:{give_id}")
}

/// The settled-set id of a refused give's items going back into the backpack (queued like a
/// trade's moves, systems::inventory::TradeSettlements).
fn return_id(give_id: &str) -> String {
    format!("fleet-return:{give_id}")
}

/// The player: the first entity with a backpack and player control (the one trades settle into).
fn player(world: &hecs::World) -> Option<hecs::Entity> {
    world.query::<(&Inventory, &crate::ecs::components::Controllable)>().iter().next().map(|(e, _)| e)
}

/// The player, with a TradeSettlements beside the backpack (made when missing).
fn player_with_settlements(world: &mut hecs::World) -> Option<hecs::Entity> {
    let p = player(world)?;
    if world.get::<&TradeSettlements>(p).is_err() {
        world.insert_one(p, TradeSettlements::default()).ok()?;
    }
    Some(p)
}

/// This home's id with the fleet, made the first time it is needed and saved with the backpack.
pub fn home_id(world: &mut hecs::World) -> Option<String> {
    let p = player_with_settlements(world)?;
    let mut ts = world.get::<&mut TradeSettlements>(p).ok()?;
    if ts.home_id.is_empty() {
        ts.home_id = format!("home-{:016x}", rand::random::<u64>());
    }
    Some(ts.home_id.clone())
}

/// The held gives as they stand, mirrored into the panel's view.
fn mirror_held(gs: &mut GuiState, world: &hecs::World) {
    let held = player(world).and_then(|p| world.get::<&TradeSettlements>(p).ok().map(|ts| ts.fleet_held.clone())).unwrap_or_default();
    if gs.fleet.held != held {
        gs.fleet.held = held;
    }
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
        Some("game_fleet_gives") => {
            on_gives(&mut state.gui_state, &mut state.game_world.world, v);
            true
        }
        Some("game_fleet_totals") => {
            state.gui_state.fleet.totals = Some(FleetTotals::from_json(v));
            true
        }
        // The answers that need no word of their own (the ledger follows an adjust).
        Some("game_fleet_power_result" | "game_fleet_give_adjusted") => true,
        // A meal the panel asked for (an AI agent's take_meal answers the same way): eat it, say
        // what happened, and fetch the ledger with its new line.
        Some("game_interact_result") if v.get("action").and_then(|a| a.as_str()) == Some("take_meal") => {
            on_meal(state, v);
            crate::gui::pages::fleet_ledger::request_ledger(&state.gui_state);
            true
        }
        _ => false,
    }
}

/// A take_meal answer: on success the meal (the good it names, a Basic Ration) goes into the
/// backpack and is eaten at once through the Eat button's path (`pending_consume_item`), so a
/// meal charged to the ledger feeds the player (finding 7 of the 2026-10-04 review).
pub(crate) fn on_meal(state: &mut EngineState, v: &serde_json::Value) {
    let item = v.get("meal_item").and_then(|x| x.as_str()).filter(|_| v.get("success").and_then(|s| s.as_bool()) == Some(true));
    let eaten = item.is_some_and(|item| {
        let max_stack = state.data_store.get::<crate::systems::inventory::ItemRegistry>("item_registry").map_or(99, |r| r.max_stack_for(item));
        feed(&mut state.game_world.world, &mut state.gui_state, item, max_stack)
    });
    state.gui_state.fleet.status = meal_sentence(v, eaten);
}

/// Put one `item` in the player's backpack (a slot is made for it, as a drink's empty vessel
/// gets one) and ask for it to be eaten; false with no player.
pub(crate) fn feed(world: &mut hecs::World, gs: &mut GuiState, item: &str, max_stack: u32) -> bool {
    let Some(p) = player(world) else { return false };
    let Ok(mut inv) = world.get::<&mut Inventory>(p) else { return false };
    let occupied = inv.slots.iter().filter(|s| s.is_some()).count();
    inv.ensure_slots(occupied + 1);
    if inv.add_item(item, 1, max_stack.max(1)) > 0 {
        return false;
    }
    gs.pending_consume_item = Some(item.to_string());
    true
}

/// What a take_meal answer means, in a sentence (`eaten`: the game fed it to the player).
pub fn meal_sentence(v: &serde_json::Value, eaten: bool) -> String {
    if v.get("success").and_then(|s| s.as_bool()) == Some(true) {
        return if eaten { "You ate a meal from the ship's stores.".into() } else { "The ship's stores gave you a meal.".into() };
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

/// The server's answer to a give. A yes drops its held items for good and records the give
/// beside the backpack; a refusal puts its held items back in the backpack; "too soon" (the
/// rate limit) leaves it held and sends it again a moment later. A give not held here (another
/// device's, or one a save load put back from before) changes nothing: the fleet's list of
/// this home's gives settles that one (`on_gives`).
pub(crate) fn on_give_result(gs: &mut GuiState, world: &mut hecs::World, v: &serde_json::Value) {
    let Some(give_id) = v.get("give_id").and_then(|x| x.as_str()) else { return };
    gs.fleet.sent.retain(|id| id != give_id);
    let ok = v.get("success").and_then(|x| x.as_bool()) == Some(true);
    let why = v.get("error").and_then(|x| x.as_str()).unwrap_or("refused");
    if !ok && why == "rate_limited" {
        gs.fleet.next_send_in = gs.fleet.next_send_in.max(GIVE_RETRY_S);
        return;
    }
    let Some(p) = player_with_settlements(world) else { return };
    let held = {
        let Ok(mut ts) = world.get::<&mut TradeSettlements>(p) else { return };
        let held = ts.fleet_held.iter().position(|h| h.give_id == give_id).map(|i| ts.fleet_held.remove(i));
        match &held {
            Some(_) if ok => {
                ts.settled.insert(settled_id(give_id));
            }
            Some(h) => {
                let back = TransferOp { item_id: h.item_id.clone(), qty: h.qty, add: true, wear: h.wear, quality: h.quality };
                ts.pending.push((return_id(give_id), vec![back]));
            }
            None => {}
        }
        held
    };
    if let Some(h) = held {
        gs.fleet.status = if !ok {
            format!("The fleet did not take {} {}: {}. They are going back into your backpack.", h.qty, h.name, refusal_words(why))
        } else if h.creative || v.get("kind").and_then(|k| k.as_str()) == Some("item_creative") {
            format!("The fleet recorded {} {} from Creative mode. Gifts made in Creative mode are not counted, because it makes things from nothing.", h.qty, h.name)
        } else {
            format!("You gave the fleet {} {}.", h.qty, h.name)
        };
    }
    mirror_held(gs, world);
}

/// Plain words for a refusal code.
fn refusal_words(code: &str) -> &'static str {
    match code {
        "too_far" => "you were too far from the store",
        "no_price" => "the fleet has no price for it",
        "bad_quantity" => "that is more than one give can carry",
        "not_in_game" => "you are not in the shared world",
        "not_a_store" | "entity_not_found" => "that is not one of the fleet's stores",
        "too_many" => "you have given as many times as the fleet takes in one day; try again tomorrow",
        _ => "the server could not record it",
    }
}

/// The server's list of this home's recorded gives (`game_fleet_gives`): any the backpack has
/// not settled is settled now (`settle_listed`), and one correction goes back for those whose
/// items were not all there.
pub(crate) fn on_gives(gs: &mut GuiState, world: &mut hecs::World, v: &serde_json::Value) {
    let home = v.get("home").and_then(|x| x.as_str()).unwrap_or("");
    let gives = v.get("gives").and_then(|x| x.as_array()).cloned().unwrap_or_default();
    let (taken, adjustments) = settle_listed(world, home, &gives);
    if !adjustments.is_empty() {
        if let Some(ws) = gs.ws_client.as_ref() {
            ws.send(&serde_json::json!({ "type": "game_fleet_give_adjust", "adjustments": adjustments }).to_string());
        }
    }
    if taken > 0 {
        gs.fleet.status = if adjustments.is_empty() {
            format!("The fleet had recorded {taken} gift(s) from this home that your loaded save did not list, so those items left your backpack again.")
        } else {
            format!(
                "The fleet had recorded {taken} gift(s) from this home that your loaded save did not list. Some of those items were no longer in your backpack, so the fleet now counts only what was there."
            )
        };
    }
    mirror_held(gs, world);
}

/// Settle into the backpack the listed gives of home `home` that it has not settled: a give
/// still held here (its answer was lost) only drops its hold; any other takes its items out of
/// the backpack once. Returns how many took items, and the corrections ({give_id, delivered})
/// for those that found fewer than the fleet counts. Nothing at all when `home` is not this
/// backpack's home: another home's gives are never taken from it.
pub(crate) fn settle_listed(world: &mut hecs::World, home: &str, gives: &[serde_json::Value]) -> (usize, Vec<serde_json::Value>) {
    let mut adjustments = Vec::new();
    let mut taken = 0;
    let Some(p) = player_with_settlements(world) else { return (0, adjustments) };
    let Ok(mut q) = world.query_one::<(&mut Inventory, &mut TradeSettlements)>(p) else { return (0, adjustments) };
    let Some((inv, ts)) = q.get() else { return (0, adjustments) };
    if home.is_empty() || ts.home_id != home {
        return (0, adjustments);
    }
    for g in gives {
        let Some(id) = g.get("give_id").and_then(|x| x.as_str()) else { continue };
        let sid = settled_id(id);
        if ts.knows(&sid) {
            continue;
        }
        if let Some(i) = ts.fleet_held.iter().position(|h| h.give_id == id) {
            // Recorded while its answer was lost: its items left the backpack when it was held.
            ts.fleet_held.remove(i);
            ts.settled.insert(sid);
            continue;
        }
        let item = g.get("item_id").and_then(|x| x.as_str()).unwrap_or("");
        let qty = g.get("quantity").and_then(|x| x.as_f64()).unwrap_or(0.0).max(0.0).round() as u32;
        ts.settled.insert(sid);
        if item.is_empty() || qty == 0 {
            continue;
        }
        let short = inv.remove_item(item, qty);
        taken += 1;
        if short > 0 {
            adjustments.push(serde_json::json!({ "give_id": id, "delivered": qty - short }));
        }
    }
    (taken, adjustments)
}

/// A welcome to a shared world: its fleet stores, a fresh power baseline (nothing used before
/// this join is charged to this fleet), the ledger asked for, and every held give for this
/// server to be sent again from the start, one at a time (`send_due`), and this home's recorded
/// gives asked for again (`tick`).
pub(crate) fn on_welcome(state: &mut EngineState, v: &serde_json::Value) {
    let fleet = &mut state.gui_state.fleet;
    fleet.stores = stores_in(v);
    let t = crate::systems::ship_power::tally(&state.data_store, crate::systems::ship_power::PLAYER_HOME, crate::systems::ship_power::POWER);
    fleet.power_baseline = Some((t.drawn, t.returned));
    fleet.power_timer = 0.0;
    fleet.ledger = None;
    fleet.sent.clear();
    fleet.gives_asked_for = None;
    fleet.next_send_in = GIVE_SPACING_S;
    if let Some(ws) = state.gui_state.ws_client.as_ref() {
        ws.send(&serde_json::json!({ "type": "game_fleet_ledger_request" }).to_string());
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

/// Take the asked gives' items out of the backpack and hold them (step 2 of the module doc).
/// A give is refused, in words, when fewer of its items are free than it asks for: free means
/// in the backpack, less what is promised to a confirmed trade and what is already on its way
/// out (a trade's moves, a move to storage this frame).
pub(crate) fn process_outbox(gs: &mut GuiState, world: &mut hecs::World) {
    let outbox = std::mem::take(&mut gs.fleet.outbox);
    if outbox.is_empty() {
        return;
    }
    let Some(p) = player_with_settlements(world) else { return };
    for give in outbox {
        let promised = crate::gui::pages::trade::promised_to_trades(gs, &give.item_id);
        let to_storage: u32 = gs.pending_inventory_transfers.iter().filter(|o| !o.add && o.item_id == give.item_id).map(|o| o.qty).sum();
        let Ok(mut q) = world.query_one::<(&mut Inventory, &mut TradeSettlements)>(p) else { return };
        let Some((inv, ts)) = q.get() else { return };
        let queued: u32 = ts.pending.iter().flat_map(|(_, m)| m.iter()).filter(|o| !o.add && o.item_id == give.item_id).map(|o| o.qty).sum();
        let free = inv.count_item(&give.item_id).saturating_sub(promised + to_storage + queued);
        if free < give.qty {
            gs.fleet.status = format!(
                "Only {free} {} are free to give: the rest are promised to a trade you confirmed or already on their way out of your backpack.",
                give.name
            );
            continue;
        }
        let taken = give.qty - inv.remove_worn(&give.item_id, give.qty, give.wear, give.quality);
        if taken == 0 {
            gs.fleet.status = "That is no longer in your backpack.".into();
            continue;
        }
        ts.fleet_held.push(FleetHeld {
            give_id: give.give_id.clone(),
            server: gs.connected_server_url.clone(),
            store: give.store,
            item_id: give.item_id.clone(),
            name: give.name.clone(),
            qty: taken,
            wear: give.wear,
            quality: give.quality,
            creative: gs.creative_mode,
        });
        gs.fleet.status = format!("Giving {taken} {} to the fleet...", give.name);
    }
    mirror_held(gs, world);
}

/// Send the next held give for this server, one at a time and spaced (`GIVE_SPACING_S`), never
/// one held for another server. Returns the message sent (the tests read it).
pub(crate) fn send_due(gs: &mut GuiState, world: &mut hecs::World, real_dt: f32) -> Option<serde_json::Value> {
    gs.fleet.next_send_in -= real_dt;
    if gs.fleet.next_send_in > 0.0 {
        return None;
    }
    let server = gs.connected_server_url.clone();
    let next = gs.fleet.held.iter().find(|h| h.server == server && !gs.fleet.sent.contains(&h.give_id)).cloned()?;
    let home = home_id(world)?;
    let msg = crate::gui::pages::fleet_ledger::give_message(&next, &home);
    if let Some(ws) = gs.ws_client.as_ref() {
        ws.send(&msg.to_string());
    }
    gs.fleet.sent.push(next.give_id);
    gs.fleet.next_send_in = GIVE_SPACING_S;
    Some(msg)
}

/// Each frame while joined (lib.rs, beside the position send): where the player stands, the
/// fleet's prices once, asked gives held, this home's recorded gives asked for (after a welcome
/// and after a save is put back), held gives sent, and the power report once a minute.
pub(crate) fn tick(state: &mut EngineState, real_dt: f32) {
    let p = state.camera.position;
    state.gui_state.fleet.my_position = Some([p.x, p.y, p.z]);
    if state.gui_state.fleet.prices.is_empty() {
        if let Some(reg) = state.data_store.get::<crate::systems::economy::TradeGoodsRegistry>("trade_goods_registry") {
            state.gui_state.fleet.prices = reg.goods.iter().map(|(id, g)| (id.clone(), f64::from(g.base_value))).collect();
        }
    }
    process_outbox(&mut state.gui_state, &mut state.game_world.world);
    mirror_held(&mut state.gui_state, &state.game_world.world);
    if !state.game_welcomed {
        return;
    }
    ask_for_gives(&mut state.gui_state, &mut state.game_world.world);
    send_due(&mut state.gui_state, &mut state.game_world.world, real_dt);
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

/// Ask the server for this home's recorded gives when it has not been asked on this connection
/// for this home, or a save was just put back over the backpack. Returns the request (the tests
/// read it).
pub(crate) fn ask_for_gives(gs: &mut GuiState, world: &mut hecs::World) -> Option<serde_json::Value> {
    let p = player_with_settlements(world)?;
    let recheck = world.get::<&mut TradeSettlements>(p).map(|mut ts| std::mem::take(&mut ts.fleet_recheck)).unwrap_or(false);
    let home = home_id(world)?;
    if !recheck && gs.fleet.gives_asked_for.as_deref() == Some(home.as_str()) {
        return None;
    }
    let msg = serde_json::json!({ "type": "game_fleet_gives_request", "home": home });
    if let Some(ws) = gs.ws_client.as_ref() {
        ws.send(&msg.to_string());
    }
    gs.fleet.gives_asked_for = Some(home);
    Some(msg)
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
#[path = "fleet_tests.rs"]
mod tests;
