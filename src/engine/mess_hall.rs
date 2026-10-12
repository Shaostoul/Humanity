//! THE MESS HALL SERVES (ship homes increment 6, lane 1D, docs/design/ship-homes-increment-6-plan.md
//! 3.8): E at the mess hall's serving counter opens its card, which offers a free crew meal and
//! drinking water.
//!
//! TAKE A MEAL. In a server's shared world the meal is the server's: the counter asks the
//! nearest fleet store in reach (`game_interact take_meal`, the same request Inventory > The
//! fleet sends, `fleet_ledger::take_meal_at_nearest`), the server counts it on the player's
//! fleet ledger, one a meal time, and its answer is eaten by engine/fleet.rs `on_meal`. While
//! joined the counter NEVER serves a meal of its own: a meal not on the ledger would be food
//! from nowhere. Away from a server (your own home, or stepped out of the shared world) the
//! counter serves the ship's stores on this computer (`systems::ship_meals`): one a meal time,
//! counted on the save's meter, the meal put in the backpack and eaten through
//! `fleet::feed`, the Eat button's path, so the food system eats it and reports the Eat event
//! the Cabin's opening counts. Decision 4 of the plan: meals any time, one a meal time.
//!
//! FILL YOUR BOTTLES fills every empty water vessel the backpack holds from the ship's water,
//! the same online and offline, and counts the litres on the same meter.
//!
//! Both are done at the counter: within the reach of every interaction (5 m, the walk-up
//! card's own reach) of a serving counter. In the shared world the server checks its own reach
//! from the store, which stands just in front of the counter.
//!
//! The card's buttons (gui/pages/hud.rs) only ask (`FleetView::counter_ask`); `frame`, called
//! each frame from lib.rs beside the trading post's flag, sets the card's flag, carries out the
//! ask and mirrors the meter for Inventory > The fleet.

use std::path::Path;

use crate::engine::state::EngineState;
use crate::gui::pages::fleet_ledger::{self, CounterAsk, STORE_REACH_M};
use crate::gui::GuiState;
use crate::hot_reload::data_store::DataStore;
use crate::systems::ship_meals::{self, MealRules, Served};

/// The machine type a serving counter is (data/machines/ship.ron's catalog, increment 6 lane 1A).
pub const COUNTER_TYPE: &str = "serving_counter";

/// What the counter says when asked from too far away.
pub const WALK_TO_COUNTER: &str = "Walk up to the mess hall's serving counter first.";

/// True when the machine `selected_id` (the pinned card's) is a serving counter: its card offers
/// a meal and water. Nothing else does, however much it looks like furniture.
pub fn card_offers_meal(machines: Option<&crate::machines::MachineHome>, selected_id: Option<&str>) -> bool {
    let (Some(m), Some(id)) = (machines, selected_id) else { return false };
    m.instances.iter().any(|i| i.id == id && i.machine == COUNTER_TYPE)
}

/// How far `from` is from the nearest serving counter's walk-up label, metres; None with no
/// counter in this home's machines.
pub fn nearest_counter_m(machines: Option<&crate::machines::MachineHome>, labels: &[crate::gui::MachineLabel], from: glam::Vec3) -> Option<f32> {
    let m = machines?;
    labels
        .iter()
        .filter(|l| m.instances.iter().any(|i| i.id == l.machine_id && i.machine == COUNTER_TYPE))
        .map(|l| l.pos.distance(from))
        .min_by(|a, b| a.total_cmp(b))
}

/// Everything the counter reads and changes, borrowed from the engine (the tests build one
/// without an engine).
pub struct Counter<'a> {
    pub gs: &'a mut GuiState,
    pub world: &'a mut hecs::World,
    pub data: &'a mut DataStore,
    pub data_dir: &'a Path,
    /// How far the player stands from the nearest serving counter, metres (None: none here).
    pub counter_m: Option<f32>,
}

impl<'a> Counter<'a> {
    /// The engine's counter, measured from where the player stands now.
    pub fn of(state: &'a mut EngineState) -> Counter<'a> {
        let counter_m = nearest_counter_m(state.gui_state.home_machines.as_ref(), &state.gui_state.machine_labels, state.camera.position);
        Counter { gs: &mut state.gui_state, world: &mut state.game_world.world, data: &mut state.data_store, data_dir: &state.data_dir, counter_m }
    }

    fn at_counter(&self) -> bool {
        self.counter_m.is_some_and(|d| d <= STORE_REACH_M)
    }
}

/// What Take a meal did.
#[derive(Debug, Clone, PartialEq)]
pub enum Meal {
    /// In the shared world: the server was asked (this message); its answer is eaten later.
    Asked(serde_json::Value),
    /// Away from a server: one meal of this item from the ship's stores, eaten.
    Served(String),
    /// Nothing now, and why.
    No(String),
}

impl Meal {
    pub fn sentence(&self) -> String {
        match self {
            Meal::Asked(_) => "Asking the ship's stores for a meal...".to_string(),
            Meal::Served(_) => "You ate a crew meal from the ship's stores.".to_string(),
            Meal::No(why) => why.clone(),
        }
    }
}

/// Take a meal at the counter (its card's button, and the probe's verb): the sentence it said,
/// also left on the card.
pub fn take_meal(state: &mut EngineState) -> String {
    let said = take_meal_at(&mut Counter::of(state)).sentence();
    state.gui_state.fleet.counter_status = said.clone();
    said
}

/// Fill your bottles at the counter (its card's button, and the probe's verb): the sentence it
/// said, also left on the card.
pub fn fill_bottles(state: &mut EngineState) -> String {
    let said = fill_bottles_at(&mut Counter::of(state));
    state.gui_state.fleet.counter_status = said.clone();
    said
}

/// Take a meal: the server's in the shared world, the ship's stores on this computer elsewhere
/// (the module doc).
pub fn take_meal_at(c: &mut Counter) -> Meal {
    if c.gs.copresence_active {
        return match fleet_ledger::take_meal_at_nearest(c.gs) {
            Ok(msg) => Meal::Asked(msg),
            Err(why) => Meal::No(why),
        };
    }
    if !c.at_counter() {
        return Meal::No(WALK_TO_COUNTER.into());
    }
    let rules = MealRules::load(c.data_dir);
    let now = crate::systems::time::elapsed_now(c.data);
    let mut after = meter(c.data);
    match ship_meals::serve(&mut after, now, &rules) {
        Served::NotYet { ready_in_s } => Meal::No(ship_meals::not_yet_sentence(ready_in_s)),
        Served::Meal { item } => {
            let max_stack = c.data.get::<crate::systems::inventory::ItemRegistry>("item_registry").map_or(99, |r| r.max_stack_for(&item));
            if !crate::engine::fleet::feed(c.world, c.gs, &item, max_stack) {
                return Meal::No("No meal: you are not in the world yet.".into());
            }
            crate::save_load::put_ship_meals(c.data, &after);
            Meal::Served(item)
        }
    }
}

/// Fill your bottles: every empty water vessel in the backpack, from the ship's water, counted on
/// the meter. The same in the shared world and away from it.
pub fn fill_bottles_at(c: &mut Counter) -> String {
    if !c.at_counter() {
        return WALK_TO_COUNTER.into();
    }
    let table = c.data.get::<crate::systems::fluids::FluidTable>("fluid_table").cloned().unwrap_or_else(|| crate::systems::fluids::FluidTable::load(c.data_dir));
    let filled = ship_meals::fill_vessels(&table, c.world, c.data.get::<crate::systems::inventory::ItemRegistry>("item_registry"));
    if filled.litres > 0.0 {
        let mut m = meter(c.data);
        m.water_l += filled.litres;
        crate::save_load::put_ship_meals(c.data, &m);
    }
    filled.sentence()
}

/// The meter as it stands. Until startup registers it (the increment's wiring), the first use
/// takes up the live save's own meter, as a save with none registered keeps the one it had, so
/// nothing counted before is lost.
fn meter(data: &mut DataStore) -> crate::persistence::ShipMeals {
    if let Some(m) = crate::save_load::ship_meals(data) {
        return m;
    }
    let had = crate::save_load::load_active_home().map(|s| s.ship_meals).unwrap_or_default();
    crate::save_load::put_ship_meals(data, &had);
    had
}

/// Each frame (lib.rs, beside the trading post's flag): whether the pinned card is a serving
/// counter, the button its card asked for carried out, and the meter mirrored for Inventory >
/// The fleet.
pub fn frame(state: &mut EngineState) {
    let gs = &mut state.gui_state;
    let pinned = gs.selected_machine.and_then(|i| gs.machine_labels.get(i)).map(|l| l.machine_id.clone());
    let was_open = gs.machine_card_meal;
    gs.machine_card_meal = card_offers_meal(gs.home_machines.as_ref(), pinned.as_deref());
    if gs.machine_card_meal && !was_open {
        // A card opened afresh starts without the last visit's result.
        gs.fleet.counter_status.clear();
    }
    match gs.fleet.counter_ask.take() {
        Some(CounterAsk::Meal) => {
            take_meal(state);
        }
        Some(CounterAsk::Water) => {
            fill_bottles(state);
        }
        None => {}
    }
    let here = crate::save_load::ship_meals(&state.data_store);
    if state.gui_state.fleet.meals_here != here {
        state.gui_state.fleet.meals_here = here;
    }
}

#[cfg(test)]
#[path = "mess_hall_tests.rs"]
mod tests;
