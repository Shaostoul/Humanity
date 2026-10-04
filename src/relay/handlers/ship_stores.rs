//! THE SHIP'S FOOD STORES (increment 3 of docs/design/ship-homes-and-logistics.md).
//!
//! The operator decided on 2026-10-04 (open question 12 of that design): NPCs eat from the
//! same food stores as players, and NPC homesteads mostly provide for themselves. So the relay
//! keeps the ship's stores (data/food/ship_stores.ron): each a stock of MEALS at a place
//! aboard (the mess hall's, today). Everyone eats from the same stock:
//!
//!   - a CREW member who eats (crew.ron `eats`) walks to its seat in the mess hall when a meal
//!     is due (every `meal_interval_hours` of the world's GAME clock, which runs at the server's
//!     `world_time_scale`, 72x by default: 8 game hours is 400 real seconds), eats for the length
//!     of its meal chore (chores.ron, `meal: true`, in real seconds like every chore), and takes
//!     ONE meal from the store at that place. An empty store means a missed meal
//!     (`meals_missed`) and another try in `empty_store_retry_minutes` (game minutes);
//!   - a PLAYER takes one with the `take_meal` action on the store, standing within 5 m of it
//!     (`handle_take_meal`, the same reach as every interaction), one draw on the same stock,
//!     and one a meal interval, by their key (`GameWorld::player_next_meal`): the "free,
//!     metered" meal of the design's question 11, so no one person can empty the crew's stock.
//!     The game's own hunger does not read this yet: serving meals into the food system is
//!     increment 6 ("the mess hall serves free, metered crew meals"), and until then a taken
//!     meal is counted on the player (`meals_taken`), which is what an AI agent sees;
//!   - an NPC of a HOMESTEAD household (crew.ron `household: "homestead"`) eats the share
//!     `npc_homestead_self_provided` of its meals from its own homestead and the rest from
//!     the stores, and puts `npc_homestead_fleet_meals_per_day` into them. The share is
//!     credited once per MEAL TIME: a homestead NPC that finds the store empty misses that meal
//!     and comes again at its next meal time (it has its own food; it does not come back every
//!     half hour, and a retry never credits the share twice). No shipped NPC lives on a
//!     homestead yet (the crew are the ship's company, household "crew", all of whose meals
//!     come from the stores), so these two values wait for the first NPC homesteads; how much
//!     those give the fleet against what human players give is the operator's open question,
//!     and its value is a placeholder named as one in the file.
//!
//! What fills the stores today is `ship_farms_meals_per_day`, per GAME day (the same clock the
//! meals run on): a stand-in for the ship's farms until goods are carried aboard for real
//! (increment 9, shipments). Players put nothing in yet.
//!
//! The stock lives on the store's entity (`food_store`, components `meals` and `capacity`),
//! and the stored world carries it across restarts (ship_world.rs `carry_over_ship_state`),
//! while the store itself is stood where this file says at every start.

use serde::Deserialize;
use std::sync::Arc;

use super::game_state::GameWorld;
use super::ship_world::{plot_at, room_point};
use crate::relay::relay::RelayState;

/// One store of the ship (data/food/ship_stores.ron).
#[derive(Debug, Clone, Deserialize)]
pub struct StoreDef {
    pub id: String,
    pub label: String,
    /// The room it stands in (a room id of the relay's world: a ship zone or a volume in one).
    pub place: String,
    /// Where in that room, room-local (x, z) metres from its min corner.
    pub spot: (f32, f32),
    /// The meals a new world's store starts with.
    pub meals: f64,
    /// The most it holds; restocking stops there.
    pub capacity: f64,
}

/// data/food/ship_stores.ron (see the top of this file for what each value does).
#[derive(Debug, Clone, Deserialize)]
pub struct Provisions {
    pub stores: Vec<StoreDef>,
    pub meal_interval_hours: f64,
    pub empty_store_retry_minutes: f64,
    pub ship_farms_meals_per_day: f64,
    pub npc_homestead_self_provided: f64,
    pub npc_homestead_fleet_meals_per_day: f64,
}

impl Default for Provisions {
    /// No file and no built-in copy: no stores, and the crew never come to eat.
    fn default() -> Self {
        Provisions {
            stores: Vec::new(),
            meal_interval_hours: 8.0,
            empty_store_retry_minutes: 30.0,
            ship_farms_meals_per_day: 0.0,
            npc_homestead_self_provided: 0.0,
            npc_homestead_fleet_meals_per_day: 0.0,
        }
    }
}

impl Provisions {
    /// data/food/ship_stores.ron from the relay's working folder, else the copy built into
    /// the exe (logged as such, the rigs refuse a run that served one). One that does not parse
    /// leaves no stores, loudly.
    pub fn load() -> Provisions {
        let text = std::fs::read_to_string("data/food/ship_stores.ron").unwrap_or_else(|e| {
            crate::embedded_data::note_builtin_copy("food/ship_stores.ron", format_args!("data/food/ship_stores.ron could not be read ({e})"));
            include_str!("../../../data/food/ship_stores.ron").to_string()
        });
        Self::parse(&text).unwrap_or_else(|e| {
            tracing::error!("ship_stores.ron does not parse ({e}); the ship has no food stores and the crew never eat");
            Provisions::default()
        })
    }

    /// Parse a stores file. Every number must be finite (RON reads `NaN` and `inf`, and NaN
    /// passes every `< 0.0` check); the clocks positive; the share between 0 and 1; the rates
    /// not negative; and each store's capacity not negative, with its starting meals between 0
    /// and that capacity (a negative capacity would put a negative stock in the store, which
    /// `stock_stores` keeps each store to and the stored world then keeps).
    pub fn parse(text: &str) -> Result<Provisions, String> {
        let p: Provisions = ron::from_str(text).map_err(|e| e.to_string())?;
        let numbers = [
            ("meal_interval_hours", p.meal_interval_hours),
            ("empty_store_retry_minutes", p.empty_store_retry_minutes),
            ("ship_farms_meals_per_day", p.ship_farms_meals_per_day),
            ("npc_homestead_self_provided", p.npc_homestead_self_provided),
            ("npc_homestead_fleet_meals_per_day", p.npc_homestead_fleet_meals_per_day),
        ];
        if let Some((name, v)) = numbers.iter().find(|(_, v)| !v.is_finite()) {
            return Err(format!("{name} must be a number; it is {v}"));
        }
        if !(p.meal_interval_hours > 0.0 && p.empty_store_retry_minutes > 0.0) {
            return Err("meal_interval_hours and empty_store_retry_minutes must be more than 0".into());
        }
        if !(0.0..=1.0).contains(&p.npc_homestead_self_provided) {
            return Err(format!("npc_homestead_self_provided is a share, 0 to 1; it is {}", p.npc_homestead_self_provided));
        }
        if p.ship_farms_meals_per_day < 0.0 || p.npc_homestead_fleet_meals_per_day < 0.0 {
            return Err("meals per day cannot be negative".into());
        }
        for s in &p.stores {
            if ![s.meals, s.capacity, f64::from(s.spot.0), f64::from(s.spot.1)].iter().all(|v| v.is_finite()) {
                return Err(format!("store {}: its meals, capacity and spot must be numbers", s.id));
            }
            if !(s.capacity >= 0.0 && s.meals >= 0.0 && s.meals <= s.capacity) {
                return Err(format!("store {}: capacity {} must not be negative, and its meals {} between 0 and it", s.id, s.capacity, s.meals));
            }
        }
        Ok(p)
    }

    /// Seconds of the world's GAME clock between one person's meals.
    pub fn meal_interval_s(&self) -> f64 {
        self.meal_interval_hours * 3600.0
    }
}

/// What a meal draw found.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Draw {
    /// One meal taken; this many are left.
    Taken(f64),
    /// None left.
    Empty,
}

const SECONDS_PER_DAY: f64 = 86_400.0;

impl GameWorld {
    /// Stand each store of the ship (data/food/ship_stores.ron) in its place, with its starting
    /// stock. A store whose place is not a room of this world, or whose spot is outside that
    /// room or on a plot, is left out with a warning (a typo must not put food in someone's
    /// home).
    pub(crate) fn spawn_stores(&mut self) {
        for def in self.provisions.stores.clone() {
            let Some(room) = self.rooms.iter().find(|r| r.id == def.place).cloned() else {
                tracing::warn!("Store '{}' names unknown place '{}'; left out", def.id, def.place);
                continue;
            };
            let at = room_point(&room, Some(def.spot));
            if def.spot.0 < 0.0 || def.spot.1 < 0.0 || def.spot.0 > room.size[0] || def.spot.1 > room.size[2] || plot_at(&self.ship_plots.plots, at).is_some() {
                tracing::warn!("Store '{}' stands outside {} or on a plot; left out", def.id, def.place);
                continue;
            }
            let id = self.next_entity_id;
            self.next_entity_id += 1;
            self.entities.insert(
                id,
                super::game_state::GameEntity {
                    entity_type: "food_store".to_string(),
                    position: at,
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    owner: None,
                    components: serde_json::json!({
                        "interactable": true,
                        // Built from the files at every start; a stored world carries over only
                        // its stock (ship_world.rs `carry_over_ship_state`).
                        "ship_built": true,
                        // Counts toward the survey_storage quest like any locker or bin.
                        "storage": true,
                        "store_id": def.id,
                        "name": def.label,
                        "room_id": def.place,
                        "description": format!("{}: take a meal with the take_meal action", def.label),
                        "meals": def.meals.min(def.capacity).max(0.0),
                        "capacity": def.capacity,
                    }),
                    last_update: 0.0,
                },
            );
        }
    }

    /// The store entity in `place` (the first, by id), else any store aboard.
    pub fn store_for_place(&self, place: &str) -> Option<u64> {
        let mut stores: Vec<(u64, bool)> = self
            .entities
            .iter()
            .filter(|(_, e)| e.entity_type == "food_store")
            .map(|(id, e)| (*id, e.components.get("room_id").and_then(|v| v.as_str()) == Some(place)))
            .collect();
        stores.sort();
        stores.iter().find(|(_, here)| *here).or(stores.first()).map(|(id, _)| *id)
    }

    /// Take one meal from a store, for anyone: a crew member's meal and a player's `take_meal`
    /// both come here, so they share one stock.
    pub fn draw_meal(&mut self, store: u64) -> Option<Draw> {
        let e = self.entities.get_mut(&store).filter(|e| e.entity_type == "food_store")?;
        let meals = e.components.get("meals").and_then(|v| v.as_f64()).unwrap_or(0.0);
        if meals < 1.0 {
            return Some(Draw::Empty);
        }
        e.components["meals"] = serde_json::json!(meals - 1.0);
        Some(Draw::Taken(meals - 1.0))
    }

    /// What `dt` GAME seconds put into the stores (the tick passes its real seconds times the
    /// clock's speed, the same clock meals come on): the ship's farms, and every NPC of a
    /// homestead household's gift to the fleet, per game day, shared evenly across the stores,
    /// each kept to its capacity.
    pub(crate) fn stock_stores(&mut self, dt: f64) {
        let homesteads = self
            .entities
            .values()
            .filter(|e| e.components.get("household").and_then(|v| v.as_str()) == Some("homestead"))
            .count() as f64;
        let per_day = self.provisions.ship_farms_meals_per_day + homesteads * self.provisions.npc_homestead_fleet_meals_per_day;
        let ids: Vec<u64> = self.entities.iter().filter(|(_, e)| e.entity_type == "food_store").map(|(id, _)| *id).collect();
        if per_day <= 0.0 || ids.is_empty() {
            return;
        }
        let each = per_day * dt / SECONDS_PER_DAY / ids.len() as f64;
        for id in ids {
            let Some(e) = self.entities.get_mut(&id) else { continue };
            let meals = e.components.get("meals").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let cap = e.components.get("capacity").and_then(|v| v.as_f64()).unwrap_or(f64::INFINITY);
            e.components["meals"] = serde_json::json!((meals + each).min(cap));
        }
    }

    /// Give a crew member who eats its meal clock: the first meal comes `stagger` of the way
    /// through an interval from now, so the crew do not all come to eat at once.
    pub(crate) fn start_meal_clock(components: &mut serde_json::Value, game_time: f64, interval_s: f64, stagger: f64) {
        components["next_meal_at"] = serde_json::json!(game_time + interval_s * stagger);
        components["meals_eaten"] = serde_json::json!(0);
        components["meals_at_home"] = serde_json::json!(0);
        components["meals_missed"] = serde_json::json!(0);
        components["home_meal_credit"] = serde_json::json!(0.0);
    }

    /// Start every eater's meal clock from the world's clock NOW: the k-th of n eaters (by
    /// entity id) first eats k/(n+1) of a meal interval from now, so they come one at a time.
    /// Called when the crew are stood up (`populate_ship_entities`, at clock 0) and again by
    /// anything that then moves the clock: the upgrade of a stored Pioneer world carries its
    /// clock over (a day and more, so clocks started at 0 were all overdue at once, and the
    /// crew ate in lockstep for good: the review of increment 3, finding 2), and a restore
    /// starts the clocks of crew its snapshot does not know.
    pub(crate) fn restart_meal_clocks(&mut self) {
        let interval = self.provisions.meal_interval_s();
        let now = self.game_time;
        let mut eaters: Vec<u64> = self
            .entities
            .iter()
            .filter(|(_, e)| e.components.get("eats").and_then(|v| v.as_bool()) == Some(true))
            .map(|(id, _)| *id)
            .collect();
        eaters.sort();
        let n = eaters.len() as f64;
        for (k, id) in eaters.iter().enumerate() {
            if let Some(e) = self.entities.get_mut(id) {
                Self::start_meal_clock(&mut e.components, now, interval, (k as f64 + 1.0) / (n + 1.0));
            }
        }
    }

    /// When a crew member is between chores: the index of the meal chore (in `self.chores`) it
    /// should do now, or None (no meal due, it does not eat, or this meal comes from its own
    /// homestead, which this counts and moves its clock on for).
    pub(crate) fn meal_due(&mut self, id: u64, role: &str) -> Option<usize> {
        let now = self.game_time;
        let interval = self.provisions.meal_interval_s();
        let self_share = self.provisions.npc_homestead_self_provided;
        let e = self.entities.get_mut(&id)?;
        let c = &mut e.components;
        if !c.get("eats").and_then(|v| v.as_bool()).unwrap_or(false) {
            return None;
        }
        let due_at = c.get("next_meal_at").and_then(|v| v.as_f64())?;
        if now < due_at {
            return None;
        }
        if c.get("household").and_then(|v| v.as_str()) == Some("homestead") {
            // Mostly from their own homestead: of every meal, `self_share` is credited to home,
            // and a meal is eaten at home whenever a whole one has been credited.
            let credit = c.get("home_meal_credit").and_then(|v| v.as_f64()).unwrap_or(0.0) + self_share;
            if credit >= 1.0 - 1e-9 {
                c["home_meal_credit"] = serde_json::json!(credit - 1.0);
                let n = c.get("meals_at_home").and_then(|v| v.as_u64()).unwrap_or(0);
                c["meals_at_home"] = serde_json::json!(n + 1);
                c["next_meal_at"] = serde_json::json!(now + interval);
                return None;
            }
            c["home_meal_credit"] = serde_json::json!(credit);
        }
        self.meal_chore_for(role)
    }

    /// The meal chore (an index into `self.chores`) a crew member of `role` eats at: the first
    /// meal chore naming its role, else the first open to everyone. Each eater's own seat in
    /// the mess hall (data/npc/chores.ron), so the crew are never drawn as one figure at one
    /// spot (the review of increment 3, finding 9).
    pub(crate) fn meal_chore_for(&self, role: &str) -> Option<usize> {
        let own = self.chores.iter().position(|ch| ch.meal && ch.roles.iter().any(|r| r == role));
        own.or_else(|| self.chores.iter().position(|ch| ch.meal && ch.roles.is_empty()))
    }

    /// A crew member finished its meal chore in `place`: one meal from the store there (or any
    /// store aboard). Counted as eaten, the next one due an interval on; or, with none left,
    /// counted as missed: the ship's company (household "crew", every meal from the stores)
    /// tries again sooner, while a homestead NPC, which has its own food, waits for its next
    /// meal time, so its home share is credited once per meal time and never again on a retry
    /// (the review of increment 3, finding 6).
    pub(crate) fn finish_meal(&mut self, id: u64, place: &str) {
        let now = self.game_time;
        let interval = self.provisions.meal_interval_s();
        let retry = self.provisions.empty_store_retry_minutes * 60.0;
        let drawn = self.store_for_place(place).and_then(|s| self.draw_meal(s));
        let Some(e) = self.entities.get_mut(&id) else { return };
        let c = &mut e.components;
        let bump = |c: &mut serde_json::Value, k: &str| {
            let n = c.get(k).and_then(|v| v.as_u64()).unwrap_or(0);
            c[k] = serde_json::json!(n + 1);
        };
        match drawn {
            Some(Draw::Taken(_)) => {
                bump(c, "meals_eaten");
                c["next_meal_at"] = serde_json::json!(now + interval);
            }
            _ => {
                bump(c, "meals_missed");
                let homestead = c.get("household").and_then(|v| v.as_str()) == Some("homestead");
                c["next_meal_at"] = serde_json::json!(now + if homestead { interval } else { retry });
                tracing::info!("Game: the ship's stores in {place} are empty; a crew member missed a meal");
            }
        }
    }
}

/// A player's `take_meal` on a store (game_interact with that action, msg_handlers.rs
/// `handle_game_interact`): one meal from it if they stand within 5 m, it holds one, and their
/// meal time has come (one a meal interval per person, like the crew). The reply is a
/// `game_interact_result` with `meals_left`; a refusal names why (`not_in_game`,
/// `entity_not_found`, `not_a_store`, `too_far`, `not_yet` with `next_meal_in_s` game seconds,
/// `empty`).
pub async fn handle_take_meal(state: &Arc<RelayState>, my_key: &str, entity_id: u64) {
    let reply = {
        let mut world = state.game_world.write().await;
        take_meal(&mut world, my_key, entity_id)
    };
    super::msg_handlers::send_game_private(state, my_key, &reply).await;
}

/// The world side of `handle_take_meal`: the reply it sends.
pub fn take_meal(world: &mut GameWorld, my_key: &str, entity_id: u64) -> serde_json::Value {
    let fail = |error: &str| serde_json::json!({ "type": "game_interact_result", "entity_id": entity_id, "action": "take_meal", "success": false, "error": error });
    let Some(player) = world.find_player_entity(my_key) else { return fail("not_in_game") };
    let Some(store) = world.entities.get(&entity_id) else { return fail("entity_not_found") };
    if store.entity_type != "food_store" {
        return fail("not_a_store");
    }
    let (p, s) = (world.entities[&player].position, store.position);
    let dist = ((p[0] - s[0]).powi(2) + (p[1] - s[1]).powi(2) + (p[2] - s[2]).powi(2)).sqrt();
    if dist > 5.0 {
        let mut r = fail("too_far");
        r["distance"] = serde_json::json!(dist);
        return r;
    }
    // One meal a meal interval per person (the review of increment 3, finding 4): the crew are
    // held to it by their meal clocks, and without it one player could empty the shared stock in
    // seconds. Spent times are dropped as they pass, so the map holds only people still waiting.
    let now = world.game_time;
    world.player_next_meal.retain(|_, at| *at > now);
    if let Some(&at) = world.player_next_meal.get(my_key) {
        let mut r = fail("not_yet");
        r["next_meal_in_s"] = serde_json::json!(at - now);
        return r;
    }
    match world.draw_meal(entity_id) {
        Some(Draw::Taken(left)) => {
            let interval = world.provisions.meal_interval_s();
            world.player_next_meal.insert(my_key.to_string(), now + interval);
            if let Some(e) = world.entities.get_mut(&player) {
                let n = e.components.get("meals_taken").and_then(|v| v.as_u64()).unwrap_or(0);
                e.components["meals_taken"] = serde_json::json!(n + 1);
            }
            serde_json::json!({ "type": "game_interact_result", "entity_id": entity_id, "action": "take_meal", "success": true, "meals_left": left })
        }
        _ => {
            let mut r = fail("empty");
            r["meals_left"] = serde_json::json!(0.0);
            r
        }
    }
}

/// TESTS: meals every `hours` GAME hours from now on, every eater's clock restarted with the
/// same stagger the world gives them at its start (a world is built with the shipped 8 hours).
#[cfg(test)]
pub(crate) fn meals_every(world: &mut GameWorld, hours: f64) {
    world.provisions.meal_interval_hours = hours;
    world.restart_meal_clocks();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stores(world: &GameWorld) -> Vec<u64> {
        let mut v: Vec<u64> = world.entities.iter().filter(|(_, e)| e.entity_type == "food_store").map(|(id, _)| *id).collect();
        v.sort();
        v
    }

    fn meals_in(world: &GameWorld, store: u64) -> f64 {
        world.entities[&store].components["meals"].as_f64().unwrap()
    }

    fn count(world: &GameWorld, key: &str) -> u64 {
        world.entities.values().filter_map(|e| e.components.get(key).and_then(|v| v.as_u64())).sum()
    }

    /// The shipped stores file parses, and its one store stands in the mess hall with its
    /// starting stock, on no plot.
    #[test]
    fn the_shipped_stores_file_parses_and_its_store_stands_in_the_mess_hall() {
        let text = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/food/ship_stores.ron")).unwrap();
        let p = Provisions::parse(&text).expect("ship_stores.ron parses");
        assert!(!p.stores.is_empty());
        let world = GameWorld::new();
        let s = stores(&world);
        assert_eq!(s.len(), p.stores.len(), "every shipped store stands in the world");
        let e = &world.entities[&s[0]];
        assert_eq!(world.room_for_position(e.position).map(|r| r.id).as_deref(), Some(p.stores[0].place.as_str()));
        assert!(super::super::ship_world::plot_at(&world.ship_plots.plots, e.position).is_none());
        assert_eq!(meals_in(&world, s[0]), p.stores[0].meals);
        // Out-of-range values are refused, not used.
        assert!(Provisions::parse(&text.replace("npc_homestead_self_provided: 0.8", "npc_homestead_self_provided: 1.5")).is_err());
        assert!(Provisions::parse(&text.replace("meal_interval_hours: 8.0", "meal_interval_hours: 0.0")).is_err());
        // A store's own numbers too (the review of increment 3, finding 6): a negative capacity
        // would put a negative stock in the store (stock_stores keeps each to its capacity), and
        // NaN passes every `< 0.0` check. Seen red 2026-10-04 before the store checks: "a store
        // with capacity -1 parsed".
        for (what, from, to) in [
            ("a store with capacity -1", "capacity: 300.0", "capacity: -1.0"),
            ("a store with NaN meals", "meals: 90.0", "meals: NaN"),
            ("a store starting fuller than it holds", "meals: 90.0", "meals: 301.0"),
            ("a store with negative meals", "meals: 90.0", "meals: -1.0"),
            ("farms giving NaN a day", "ship_farms_meals_per_day: 18.0", "ship_farms_meals_per_day: NaN"),
            ("a meal every inf hours", "meal_interval_hours: 8.0", "meal_interval_hours: inf"),
        ] {
            let bad = text.replace(from, to);
            assert_ne!(bad, text, "{what}: the shipped file has no '{from}' to change");
            assert!(Provisions::parse(&bad).is_err(), "{what} parsed");
        }
    }

    /// NPCS EAT FROM THE SAME STORES AS PLAYERS (the operator, 2026-10-04): the crew's meals and
    /// a player's `take_meal` come off ONE stock, and every meal is accounted for: what the
    /// store holds at the end is what it started with, plus what the ship's farms put in,
    /// minus the crew's meals, minus the player's.
    ///
    /// Seen red 2026-10-04 with a crew member's meal counted without being taken from the store
    /// (finish_meal not calling draw_meal): "the crew's meals came off the store / left: 90.0 /
    /// right: 71.0".
    ///
    /// In GAME time since main's 72x shared clock (the review of increment 3, finding 1): a meal
    /// every 4 game hours and a game day of it, whatever the clock's speed. Written in real
    /// seconds (a meal every 15 minutes, an hour of ticks) it failed on the merged tree with
    /// "nobody missed a meal with a full store / left: 105 / right: 0": at 72x that hour was three
    /// game days, and the 90 meals ran out.
    #[test]
    fn the_crew_and_the_players_eat_from_the_same_store() {
        let mut world = GameWorld::new();
        meals_every(&mut world, 4.0); // a meal every 4 game hours, to see several in a day
        world.provisions.ship_farms_meals_per_day = 0.0; // nothing in, so every meal shows
        let store = stores(&world)[0];
        let start = meals_in(&world, store);
        run_game_hours(&mut world, 24.0);
        let crew_meals = count(&world, "meals_eaten");
        assert!(crew_meals >= 10, "the crew came to eat: {crew_meals} meals in a game day");
        assert_eq!(count(&world, "meals_missed"), 0, "nobody missed a meal with a full store");
        assert_eq!(meals_in(&world, store), start - crew_meals as f64, "the crew's meals came off the store");

        // A player in the mess hall takes one from the same store.
        let at = world.entities[&store].position;
        let p = world.spawn_player("e11e00aa", [at[0] + 1.0, 1.7, at[2]]);
        let reply = take_meal(&mut world, "e11e00aa", store);
        assert_eq!(reply["success"], true, "{reply}");
        assert_eq!(reply["meals_left"].as_f64(), Some(start - crew_meals as f64 - 1.0), "{reply}");
        assert_eq!(world.entities[&p].components["meals_taken"], 1);
        assert_eq!(meals_in(&world, store), start - crew_meals as f64 - 1.0, "the player's meal came off the same store");
    }

    /// An empty store is a missed meal, tried again sooner, and the store is not driven below
    /// zero; a player finds it empty too.
    ///
    /// Seen red 2026-10-04 with the same break: "two meals in the store, two eaten / left: 19 /
    /// right: 2".
    #[test]
    fn an_empty_store_is_a_missed_meal_for_everyone() {
        let mut world = GameWorld::new();
        meals_every(&mut world, 4.0);
        world.provisions.ship_farms_meals_per_day = 0.0;
        let store = stores(&world)[0];
        world.entities.get_mut(&store).unwrap().components["meals"] = serde_json::json!(2.0);
        run_game_hours(&mut world, 24.0);
        assert_eq!(count(&world, "meals_eaten"), 2, "two meals in the store, two eaten");
        assert!(count(&world, "meals_missed") >= 3, "the rest of the crew found it empty");
        assert_eq!(meals_in(&world, store), 0.0, "never below zero");
        let at = world.entities[&store].position;
        world.spawn_player("e11e00ab", [at[0], 1.7, at[2] + 1.0]);
        assert_eq!(take_meal(&mut world, "e11e00ab", store)["error"], "empty");
    }

    /// A player takes a meal only within reach, only from a store, only in the world.
    #[test]
    fn take_meal_needs_reach_a_store_and_the_world() {
        let mut world = GameWorld::new();
        let store = stores(&world)[0];
        assert_eq!(take_meal(&mut world, "e11e00ac", store)["error"], "not_in_game");
        let at = world.entities[&store].position;
        world.spawn_player("e11e00ac", [at[0] + 20.0, 1.7, at[2]]);
        assert_eq!(take_meal(&mut world, "e11e00ac", store)["error"], "too_far");
        let other = *world.entities.iter().find(|(_, e)| e.entity_type == "dining_table").map(|(id, _)| id).expect("a dining table");
        assert_eq!(take_meal(&mut world, "e11e00ac", other)["error"], "not_a_store");
        assert_eq!(take_meal(&mut world, "e11e00ac", 999_999)["error"], "entity_not_found");
    }

    /// NPC HOMESTEADS MOSTLY PROVIDE FOR THEMSELVES (the operator, 2026-10-04): an NPC of a
    /// homestead household eats `npc_homestead_self_provided` of its meals at home and the rest
    /// from the stores, and gives the stores `npc_homestead_fleet_meals_per_day` (the open
    /// question; set here to a test value). The shipped crew are the ship's company (every
    /// meal from the stores) and no homestead NPC ships yet, so this makes one.
    ///
    /// Seen red 2026-10-04 with the homestead branch of meal_due taken out: "four in five at
    /// home (0.8): 0 at home, 10 from the stores / left: 10 / right: 2".
    #[test]
    fn an_npc_homestead_mostly_feeds_itself_and_gives_what_it_is_set_to() {
        let mut world = GameWorld::new();
        world.provisions.ship_farms_meals_per_day = 0.0;
        world.provisions.npc_homestead_self_provided = 0.8;
        world.provisions.npc_homestead_fleet_meals_per_day = 0.0;
        let homesteader = one_homesteader(&mut world);
        meals_every(&mut world, 4.0);
        let store = stores(&world)[0];
        let start = meals_in(&world, store);
        // A meal every 4 game hours: two game days is twelve meal times, ten or more of them
        // eaten (a meal waits for the chore in hand to finish, which at 72x is game minutes).
        run_game_hours(&mut world, 48.0);
        let c = &world.entities[&homesteader].components;
        let (home, stored) = (c["meals_at_home"].as_u64().unwrap(), c["meals_eaten"].as_u64().unwrap());
        assert!(home + stored >= 10, "ten meals or more: {home} at home, {stored} from the stores");
        // 0.8 a meal is credited to home, and a whole credit is a meal at home: the stores give
        // the 1st, 6th, 11th... meal, one in five, rounded up.
        assert_eq!(stored, (home + stored).div_ceil(5), "four in five at home (0.8): {home} at home, {stored} from the stores");
        assert_eq!(meals_in(&world, store), start - stored as f64, "only the stores' share came off the store");

        // What it gives the fleet's stores is the open question's value, per day.
        world.provisions.npc_homestead_fleet_meals_per_day = 2.0;
        let before = meals_in(&world, store);
        world.stock_stores(86_400.0);
        assert!((meals_in(&world, store) - before - 2.0).abs() < 1e-6, "one homestead NPC gives 2 a day");
    }

    /// The ship's farms fill the stores at their rate, and no store goes past its capacity.
    #[test]
    fn the_farms_restock_the_stores_up_to_capacity() {
        let mut world = GameWorld::new();
        world.provisions.ship_farms_meals_per_day = 24.0;
        let store = stores(&world)[0];
        world.entities.get_mut(&store).unwrap().components["meals"] = serde_json::json!(10.0);
        world.stock_stores(43_200.0);
        assert!((meals_in(&world, store) - 22.0).abs() < 1e-6, "half a day at 24 a day: {}", meals_in(&world, store));
        world.stock_stores(86_400.0 * 100.0);
        let cap = world.entities[&store].components["capacity"].as_f64().unwrap();
        assert_eq!(meals_in(&world, store), cap, "never past capacity");
    }

    /// Tick `world` at 0.25 real seconds a tick until its clock has moved `hours` GAME hours:
    /// the same game time at any clock speed (72x by default since main's shared clock).
    fn run_game_hours(world: &mut GameWorld, hours: f64) {
        let end = world.game_time + hours * 3600.0;
        while world.game_time < end {
            world.tick(0.25);
        }
    }

    /// Every eater but the first stops eating, and that one lives on a homestead (no shipped
    /// NPC does yet, so the tests make one). Returns its id. Call before `meals_every`, which
    /// staggers only the eaters.
    fn one_homesteader(world: &mut GameWorld) -> u64 {
        let mut eaters: Vec<u64> = world.entities.iter().filter(|(_, e)| e.components.get("eats").and_then(|v| v.as_bool()) == Some(true)).map(|(id, _)| *id).collect();
        eaters.sort();
        for id in &eaters[1..] {
            world.entities.get_mut(id).unwrap().components["eats"] = serde_json::json!(false);
        }
        world.entities.get_mut(&eaters[0]).unwrap().components["household"] = serde_json::json!("homestead");
        eaters[0]
    }

    /// THE STORES HOLD STEADY AT THE SHARED CLOCK'S SPEED (the review of increment 3, findings 1
    /// and 7). Meals come on the world's clock, which runs at its time scale (72x by default;
    /// an admin can set 1x to 1000x at runtime), so the farms must restock on the same clock,
    /// or at any speed but 1x the crew eat many times faster than the stores fill. A fresh
    /// world, nobody else aboard, three REAL hours at the relay's 20 Hz tick: nobody misses a
    /// meal and the store ends no lower than it began (18 a day in against the five eaters' 15),
    /// at the shipped 72x, at 1x and at 500x.
    ///
    /// Seen red 2026-10-04 on the merged tree (restocking on real seconds): "at 72x: 91 meals
    /// missed in three real hours".
    #[test]
    fn the_stores_hold_steady_at_the_shared_clocks_speed() {
        for scale in [72.0, 1.0, 500.0] {
            let mut world = GameWorld::new();
            world.time_scale = scale;
            let store = stores(&world)[0];
            let start = meals_in(&world, store);
            for _ in 0..(3 * 3600 * 20) {
                world.tick(0.05);
            }
            let missed = count(&world, "meals_missed");
            assert_eq!(missed, 0, "at {scale}x: {missed} meals missed in three real hours");
            let end = meals_in(&world, store);
            assert!(end >= start, "at {scale}x the store fell from {start} to {end:.1} in three real hours ({} eaten)", count(&world, "meals_eaten"));
        }
    }

    /// A PLAYER TAKES ONE MEAL PER MEAL TIME (the review of increment 3, finding 4: "the free,
    /// metered basic crew meal" of the design's question 11). The crew eat one meal a meal
    /// interval; a player is metered the same way, on the world's clock, and per PERSON (their
    /// key), so stepping out and back in does not reset it. Otherwise one player could empty the
    /// shared stock in seconds and every crew member would go hungry until the farms refilled it.
    ///
    /// Seen red 2026-10-04 before the player's meal clock: "a second meal straight after the
    /// first was given: {\"action\":\"take_meal\",\"entity_id\":12,\"meals_left\":88.0,
    /// \"success\":true,\"type\":\"game_interact_result\"}".
    #[test]
    fn a_player_takes_one_meal_per_meal_time() {
        let mut world = GameWorld::new();
        let store = stores(&world)[0];
        let start = meals_in(&world, store);
        let at = world.entities[&store].position;
        world.spawn_player("e11e00ad", [at[0] + 1.0, 1.7, at[2]]);
        let first = take_meal(&mut world, "e11e00ad", store);
        assert_eq!(first["success"], true, "{first}");
        let second = take_meal(&mut world, "e11e00ad", store);
        assert_eq!(second["error"], "not_yet", "a second meal straight after the first was given: {second}");
        assert!(second["next_meal_in_s"].as_f64().unwrap_or(0.0) > 0.0, "the refusal says when: {second}");
        assert_eq!(meals_in(&world, store), start - 1.0, "the store is down by exactly one");

        // Out of the world and back in (a new entity, the same person): still not yet.
        world.despawn_player("e11e00ad");
        world.spawn_player("e11e00ad", [at[0] + 1.0, 1.7, at[2]]);
        assert_eq!(take_meal(&mut world, "e11e00ad", store)["error"], "not_yet", "stepping out and back in reset the meal clock");

        // Someone else is not held up by it.
        world.spawn_player("e11e00ae", [at[0], 1.7, at[2] + 1.0]);
        assert_eq!(take_meal(&mut world, "e11e00ae", store)["success"], true);

        // A meal interval later, the first player eats again.
        world.game_time += world.provisions.meal_interval_s();
        assert_eq!(take_meal(&mut world, "e11e00ad", store)["success"], true, "the next meal time came");
        assert_eq!(meals_in(&world, store), start - 3.0);
    }

    /// A HOMESTEAD NPC FINDING THE STORE EMPTY GAINS NO EXTRA HOME MEALS (the review of
    /// increment 3, finding 6). Its home share is credited once per MEAL TIME: a missed store
    /// meal is missed, and its next meal comes at the next meal time like any other. When a
    /// miss was retried half an hour later, every retry credited the 0.8 again, so missed fleet
    /// meals turned into extra home meals and the share drifted above 0.8.
    ///
    /// Seen red 2026-10-04 with the retry crediting again: "within two game hours of its missed
    /// store meal it ate at home: 1 home meal(s)" (its retry, half an hour on, credited 0.8 a
    /// second time).
    #[test]
    fn a_homestead_npc_finding_the_store_empty_gains_no_extra_home_meals() {
        let mut world = GameWorld::new();
        world.provisions.ship_farms_meals_per_day = 0.0;
        world.provisions.npc_homestead_self_provided = 0.8;
        let homesteader = one_homesteader(&mut world);
        meals_every(&mut world, 4.0);
        let store = stores(&world)[0];
        world.entities.get_mut(&store).unwrap().components["meals"] = serde_json::json!(0.0);
        let get = |w: &GameWorld, k: &str| w.entities[&homesteader].components[k].as_u64().unwrap_or(0);
        // Its first meal time is a store meal (0.8 credited, less than one whole meal), and the
        // store is empty: run until it has missed it.
        let end = world.game_time + 12.0 * 3600.0;
        while get(&world, "meals_missed") == 0 && world.game_time < end {
            world.tick(0.25);
        }
        assert_eq!(get(&world, "meals_missed"), 1, "its first store meal was missed");
        // Two game hours on (half its meal interval): no meal time has come, so nothing at home.
        run_game_hours(&mut world, 2.0);
        assert_eq!(get(&world, "meals_at_home"), 0, "within two game hours of its missed store meal it ate at home: {} home meal(s)", get(&world, "meals_at_home"));
        // Two game days of meal times on an empty store: every meal time ends one way (at home or
        // missed), and the store's share is still one in five, rounded up.
        run_game_hours(&mut world, 48.0);
        let (home, missed) = (get(&world, "meals_at_home"), get(&world, "meals_missed"));
        assert_eq!(get(&world, "meals_eaten"), 0, "nothing came off an empty store");
        assert!(home + missed >= 10, "ten meal times or more: {home} at home, {missed} missed");
        assert_eq!(missed, (home + missed).div_ceil(5), "four in five at home (0.8), the rest missed: {home} at home, {missed} missed");
        let credit = world.entities[&homesteader].components["home_meal_credit"].as_f64().unwrap();
        assert!((0.0..1.0).contains(&credit), "the home credit stays under one whole meal: {credit}");
    }

    /// INSPECTING THE STORE IS NOT MEETING THE CREW (the review of increment 3, finding 8). The
    /// store has a `name` (agents read it in perception), and the interaction handler used to
    /// count ANY named thing on the meet-the-crew quest: one look at the store was one crew
    /// member met. Only a crew member (a name AND lines to say, the test `crew_npc_count`
    /// counts by) counts; the store still counts on the survey-the-storage quest.
    ///
    /// Seen red 2026-10-04 with the handler's old rule: "inspecting the store counted as meeting
    /// a crew member: [\"The mess hall's stores\"]".
    #[test]
    fn inspecting_the_store_is_not_meeting_the_crew() {
        let mut world = GameWorld::new();
        let store = stores(&world)[0];
        let at = world.entities[&store].position;
        let p = world.spawn_player("e11e00af", [at[0] + 1.0, 1.7, at[2]]);
        // Straight onto the meet-the-crew quest, as a guest's last room would leave them.
        let rooms: Vec<String> = world.rooms.iter().map(|r| r.id.clone()).collect();
        for r in &rooms {
            world.record_room_visit(p, r);
        }
        world.apply_quest_reward(p).expect("the explore quest finished");
        assert_eq!(world.chain_next_quest(p).expect("a next quest")["id"], "meet_the_crew");

        let got = world.record_interaction_quest(p, store);
        let talked = world.entities[&p].components["current_quest"]["talked_to"].clone();
        assert!(got.is_none() && talked.as_array().is_some_and(|a| a.is_empty()), "inspecting the store counted as meeting a crew member: {talked}");

        // A crew member does count.
        let crew = *world.entities.iter().find(|(_, e)| e.components.get("dialog").is_some()).map(|(id, _)| id).expect("a crew member");
        let name = world.entities[&crew].components["name"].as_str().unwrap().to_string();
        let met = world.record_interaction_quest(p, crew).expect("meeting a crew member counts");
        assert_eq!(met.step_id, name);
    }
}
