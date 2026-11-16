//! A person's daily routine for the soak (the headless sim runner's increment 3, 2026-11-15;
//! docs/design/headless-sim-runner.md). The routine is data (data/sim/routines/<id>.ron): acts at
//! hours of the home's day (drink, eat, sleep, rest). This file turns each act into the requests a
//! player's clicks make, on the same `GuiState` fields the pages set (the Drink and Eat buttons, the
//! water tank card's Fill, the Garden page's "Harvest N ready", "Take to backpack", the Rest button)
//! and the bed prompt's sleep request, so the frame's bridges (`frame_sim::bridges_slice`, the
//! requests in engine/frame_sim_requests.rs) carry them into the systems exactly as they carry a
//! player's. Nothing here changes a vital, a tank or a backpack itself.
//!
//! What each act did is measured after the step from what the systems hold (the backpack's counts,
//! the vessels filled), never from what was asked: a request the bridges drop counts nothing, and
//! the soak's report says so (`RoutineLog`).
//!
//! An act takes two steps: the first fetches (fills the bottles at the tank, harvests what is ripe,
//! takes a food from storage), the second eats or drinks what the first fetched. A step is the
//! soak's (600 s at the safe step), so a meal is fetched and eaten within ten game minutes.
//!
//! No GPU, window, painter or `EngineState` in here (tests/sim_gpu_free_lint.rs).

use std::collections::BTreeMap;
use std::path::Path;

use humanity_core::ecs::components::{Controllable, CropInstance, MachineInstanceId, WaterTank};
use humanity_core::hot_reload::data_store::DataStore;
use humanity_systems::inventory::Inventory;
use serde::Deserialize;

use crate::gui::GuiState;

/// The routines' folder, under the data folder.
pub(crate) const ROUTINES_DIR: &str = "sim/routines";

/// What one act asks for (the routine file's `act`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub(crate) enum ActKind {
    /// A drink: the first carried drink, the bottles filled at the home's tank first when none is.
    Drink,
    /// A meal: the ripe crops harvested, then the first carried food, one taken from home storage
    /// first when none is carried.
    Eat,
    /// The night in the home's bed.
    Sleep,
    /// A ten-minute nap (the Rest button).
    Rest,
}

/// One act at an hour of the home's day.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Act {
    /// The hour of the home's day (the HUD's), 0 to the day's hours.
    pub hour: f64,
    pub act: ActKind,
}

/// data/sim/routines/<id>.ron.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Routine {
    pub id: String,
    pub name: String,
    /// The foods reached for, first first.
    pub foods: Vec<String>,
    /// The drinks reached for, first first.
    pub drinks: Vec<String>,
    /// The day's acts; their order in the file does not matter.
    pub acts: Vec<Act>,
}

impl Routine {
    /// Read routine `id` from the game's data folder.
    pub(crate) fn load(data_dir: &Path, id: &str) -> Result<Self, String> {
        let path = data_dir.join(ROUTINES_DIR).join(format!("{id}.ron"));
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut r: Routine = ron::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        r.acts.sort_by(|a, b| a.hour.total_cmp(&b.hour));
        Ok(r)
    }
}

/// What the routine's acts did over a run, measured from the systems' state.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct RoutineLog {
    /// Drinks the body took (a carried drink's count fell after the Drink request).
    pub drinks: u32,
    /// Meals eaten (a carried food's count fell after the Eat request).
    pub meals: u32,
    /// Of those, produce the garden's harvest put in the backpack.
    pub crops_eaten: u32,
    /// Ripe crops harvested for a meal (the harvest's produce in the backpack, items).
    pub produce_harvested: u32,
    /// Foods taken from home storage for a meal.
    pub taken_from_storage: u32,
    /// Litres of water the tank's Fill put in the backpack's bottles.
    pub water_drawn_l: f64,
    /// Nights lain down for, and naps.
    pub sleeps: u32,
    pub rests: u32,
    /// Acts that found nothing to do or whose request did nothing, by what was missing.
    pub missed: BTreeMap<String, u32>,
    /// What was eaten and drunk, by item.
    pub eaten: BTreeMap<String, u32>,
    pub drunk: BTreeMap<String, u32>,
}

/// An act in progress (it takes two steps, see the file's header).
#[derive(Debug, Clone)]
enum Pending {
    /// Fetched; eat or drink on the next step. `pack` is the backpack before the fetch.
    Consume { kind: ActKind, pack: BTreeMap<String, u32> },
    /// Asked to eat or drink `item`, carried `before` of it.
    Check { kind: ActKind, item: String, before: u32, from_garden: bool },
    /// Asked for the tank's Fill: the full vessels carried before.
    Filled { full_before: f64 },
}

/// A routine running through a soak.
pub(crate) struct RoutineRun {
    pub routine: Routine,
    /// The home's local time, hours since the clock's start, as the last step began (None before
    /// the first step: acts due at the very start are not run backwards).
    last_h: Option<f64>,
    /// Acts due but not yet started (one act starts per step).
    queue: Vec<ActKind>,
    pending: Vec<Pending>,
    /// Produce the garden's harvest put in the backpack and not yet eaten, by item.
    garden: BTreeMap<String, u32>,
    pub log: RoutineLog,
}

impl RoutineRun {
    pub(crate) fn new(routine: Routine) -> Self {
        RoutineRun { routine, last_h: None, queue: Vec::new(), pending: Vec::new(), garden: BTreeMap::new(), log: RoutineLog::default() }
    }

    /// Before a step: the acts that came due since the last step queue up, and the next act's
    /// requests (or the eating and drinking of what the last step fetched) go on `gui`.
    pub(crate) fn before_step(&mut self, world: &hecs::World, data: &DataStore, gui: &mut GuiState) {
        // A dead person clicks nothing (the windowed game shows its death screen): the routine
        // stops with them, so nothing is eaten or drunk by a body (the food system would let it).
        if gui.player_death_cause.is_some() {
            self.queue.clear();
            self.pending.clear();
            return;
        }
        let (now_h, day_h) = local_hours(data);
        if let Some(last) = self.last_h {
            for (at, kind) in due_between(&self.routine.acts, day_h, last, now_h) {
                let _ = at;
                self.queue.push(kind);
            }
        }
        self.last_h = Some(now_h);
        // What the last step fetched is eaten or drunk now.
        let fetched: Vec<Pending> = std::mem::take(&mut self.pending);
        for p in fetched {
            if let Pending::Consume { kind, pack } = p {
                self.consume(world, gui, kind, &pack);
            } else {
                self.pending.push(p);
            }
        }
        // One new act a step, so two acts due in one coarse step do not ask for the same thing twice.
        if self.pending.iter().all(|p| !matches!(p, Pending::Check { .. })) && !self.queue.is_empty() {
            let kind = self.queue.remove(0);
            self.start(world, data, gui, kind);
        }
    }

    /// Game seconds from now to the next act's hour. The soak shortens the step that would pass it
    /// to land on it (engine/sim_soak.rs), as it lands on its marks, so an act happens at its hour
    /// whatever the step: at a 600 s step it would otherwise come up to ten minutes late, and the
    /// soak's measured error would be the script's, not the sim's.
    pub(crate) fn seconds_to_next_act(&self, data: &DataStore) -> Option<f64> {
        let (now_h, day_h) = local_hours(data);
        let day = (now_h / day_h).floor();
        let next = [day, day + 1.0]
            .iter()
            .flat_map(|d| self.routine.acts.iter().map(move |a| d * day_h + a.hour))
            .filter(|&at| at > now_h + 1e-9)
            .fold(f64::INFINITY, f64::min);
        next.is_finite().then(|| (next - now_h) * humanity_systems::time::SECONDS_PER_HOUR)
    }

    /// After a step: what the requests did, from the backpack.
    pub(crate) fn after_step(&mut self, world: &hecs::World, data: &DataStore) {
        let still: Vec<Pending> = std::mem::take(&mut self.pending);
        for p in still {
            match p {
                Pending::Check { kind, item, before, from_garden } => {
                    let now = carried(world, &item);
                    if now < before {
                        match kind {
                            ActKind::Drink => {
                                self.log.drinks += 1;
                                *self.log.drunk.entry(item).or_default() += 1;
                            }
                            _ => {
                                self.log.meals += 1;
                                if from_garden {
                                    self.log.crops_eaten += 1;
                                    if let Some(n) = self.garden.get_mut(&item) {
                                        *n = n.saturating_sub(1);
                                    }
                                }
                                *self.log.eaten.entry(item).or_default() += 1;
                            }
                        }
                    } else {
                        self.miss(format!("{} request did nothing ({item})", act_word(kind)));
                    }
                }
                Pending::Filled { full_before } => {
                    let drawn = full_vessel_litres(world, data) - full_before;
                    if drawn > 1e-6 {
                        self.log.water_drawn_l += drawn;
                    } else {
                        self.miss("the tank's Fill did nothing".to_string());
                    }
                }
                other => self.pending.push(other),
            }
        }
    }

    /// Start one act: fetch what it needs (its requests on `gui`), to eat or drink next step.
    fn start(&mut self, world: &hecs::World, data: &DataStore, gui: &mut GuiState, kind: ActKind) {
        let pack = pack_counts(world);
        match kind {
            ActKind::Drink => {
                if self.routine.drinks.iter().any(|d| pack.get(d).copied().unwrap_or(0) > 0) {
                    self.consume(world, gui, kind, &pack);
                    return;
                }
                // Carrying no drink: fill the empty bottles carried at the home's water tank (its card's Fill).
                let Some((tank, key)) = fill_at_tank(world, data) else {
                    self.miss("nothing to drink and no water to fill a bottle from".to_string());
                    return;
                };
                gui.machine_card_fluid_pending = Some((tank, key));
                self.pending.push(Pending::Filled { full_before: full_vessel_litres(world, data) });
                self.pending.push(Pending::Consume { kind, pack });
            }
            ActKind::Eat => {
                // The Garden page's "Harvest N ready": every ripe crop.
                let ripe = ripe_crops(world, data);
                if !ripe.is_empty() {
                    gui.pending_harvest_many = ripe;
                } else if !self.routine.foods.iter().any(|f| pack.get(f).copied().unwrap_or(0) > 0) {
                    // Nothing ripe and no food carried: "Take to backpack" on one from home storage.
                    if !take_from_storage(gui, &self.routine.foods) {
                        self.miss("no food carried, ripe or in home storage".to_string());
                        return;
                    }
                    self.log.taken_from_storage += 1;
                }
                self.pending.push(Pending::Consume { kind, pack });
            }
            ActKind::Sleep => {
                // The bed's prompt (engine/built_uses.rs) asks the same: the night in the home's bed.
                humanity_systems::sleep::request(data, "Bed");
                self.log.sleeps += 1;
            }
            ActKind::Rest => {
                gui.pending_rest = true;
                self.log.rests += 1;
            }
        }
    }

    /// Eat or drink the first carried item from the routine's list (the harvest's produce first).
    fn consume(&mut self, world: &hecs::World, gui: &mut GuiState, kind: ActKind, before_fetch: &BTreeMap<String, u32>) {
        let pack = pack_counts(world);
        let has = |id: &str| pack.get(id).copied().unwrap_or(0) > 0;
        if kind == ActKind::Eat {
            // What the harvest just put in the pack is the garden's.
            for f in &self.routine.foods {
                let gained = pack.get(f).copied().unwrap_or(0).saturating_sub(before_fetch.get(f).copied().unwrap_or(0));
                if gained > 0 {
                    *self.garden.entry(f.clone()).or_default() += gained;
                    self.log.produce_harvested += gained;
                }
            }
        }
        let list = if kind == ActKind::Drink { &self.routine.drinks } else { &self.routine.foods };
        let garden_first = list.iter().find(|f| has(f) && self.garden.get(*f).copied().unwrap_or(0) > 0);
        let Some(item) = garden_first.or_else(|| list.iter().find(|f| has(f))).cloned() else {
            self.miss(format!("nothing to {} after fetching", act_word(kind)));
            return;
        };
        let from_garden = self.garden.get(&item).copied().unwrap_or(0) > 0;
        if kind == ActKind::Drink {
            gui.pending_drink_item = Some(item.clone());
        } else {
            gui.pending_consume_item = Some(item.clone());
        }
        let before = pack.get(&item).copied().unwrap_or(0);
        self.pending.push(Pending::Check { kind, item, before, from_garden });
    }

    fn miss(&mut self, why: String) {
        *self.log.missed.entry(why).or_default() += 1;
    }
}

fn act_word(kind: ActKind) -> &'static str {
    match kind {
        ActKind::Drink => "drink",
        ActKind::Eat => "eat",
        ActKind::Sleep => "sleep",
        ActKind::Rest => "rest",
    }
}

/// The home's local time in hours since the clock's start (the HUD's hour, `time::local_hour`,
/// counted on), and the hours in its day.
pub(crate) fn local_hours(data: &DataStore) -> (f64, f64) {
    let (elapsed, hpd) = data
        .get::<std::sync::Mutex<humanity_systems::time::GameTime>>("game_time")
        .and_then(|m| m.lock().ok().map(|g| (g.elapsed_seconds, g.hours_per_day)))
        .unwrap_or((0.0, humanity_systems::time::DEFAULT_HOURS_PER_DAY));
    let day_h = f64::from(hpd.max(1));
    let lon = humanity_systems::time::home_longitude_deg(data);
    (elapsed / humanity_systems::time::SECONDS_PER_HOUR + lon / 360.0 * day_h, day_h)
}

/// The acts due in the local time span (last, now], each with the local hour it fell at.
pub(crate) fn due_between(acts: &[Act], day_h: f64, last: f64, now: f64) -> Vec<(f64, ActKind)> {
    let mut out = Vec::new();
    if !(now > last) {
        return out;
    }
    let mut day = (last / day_h).floor();
    while day * day_h <= now {
        for a in acts {
            let at = day * day_h + a.hour;
            if at > last && at <= now {
                out.push((at, a.act));
            }
        }
        day += 1.0;
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

/// The player's backpack, item by item.
pub(crate) fn pack_counts(world: &hecs::World) -> BTreeMap<String, u32> {
    let mut out = BTreeMap::new();
    if let Some((_, (inv, _))) = world.query::<(&Inventory, &Controllable)>().iter().next() {
        for s in inv.slots.iter().flatten() {
            *out.entry(s.item_id.clone()).or_default() += s.quantity;
        }
    }
    out
}

fn carried(world: &hecs::World, item: &str) -> u32 {
    world.query::<(&Inventory, &Controllable)>().iter().next().map_or(0, |(_, (inv, _))| inv.count_item(item))
}

/// Litres of water carried in full vessels and tap-water items (data/containers/fluids.ron).
pub(crate) fn full_vessel_litres(world: &hecs::World, data: &DataStore) -> f64 {
    let Some(t) = data.get::<humanity_systems::fluids::FluidTable>("fluid_table") else { return 0.0 };
    let pack = pack_counts(world);
    let vessels: f64 = t.fillables.iter().filter(|f| f.fluid == "water").map(|f| f64::from(pack.get(&f.full).copied().unwrap_or(0)) * f64::from(f.litres)).sum();
    let tap: f64 = t.tap.keys().map(|id| f64::from(pack.get(id).copied().unwrap_or(0)) * f64::from(t.tap_litres(id).unwrap_or(0.0))).sum();
    vessels + tap
}

/// The water tank card's Fill for the empty bottles carried: the fullest tank holding enough
/// water for one, and the action's key (`fluids::card_actions`). None: no empty vessel carried, or
/// no tank with water.
fn fill_at_tank(world: &hecs::World, data: &DataStore) -> Option<(String, String)> {
    let t = data.get::<humanity_systems::fluids::FluidTable>("fluid_table")?;
    let pack = pack_counts(world);
    let vessel = t.fillables.iter().filter(|f| f.fluid == "water").find(|f| pack.get(&f.empty).copied().unwrap_or(0) > 0)?;
    let tank = world
        .query::<(&WaterTank, &MachineInstanceId)>()
        .iter()
        .filter(|(_, (w, _))| w.liters >= vessel.litres)
        .max_by(|a, b| a.1 .0.liters.total_cmp(&b.1 .0.liters))
        .map(|(_, (_, id))| id.0.clone())?;
    Some((tank, format!("fill:{}", vessel.empty)))
}

/// Every ripe crop the Garden page's "Harvest N ready" would harvest: the last growth stage of its
/// plant, alive, with a share ready to pick (garden_mirror.rs reads them the same way).
pub(crate) fn ripe_crops(world: &hecs::World, data: &DataStore) -> Vec<u64> {
    use humanity_systems::farming;
    let plants = data.get::<farming::PlantRegistry>("plant_registry");
    let crops: Vec<(hecs::Entity, bool)> = world
        .query::<&CropInstance>()
        .iter()
        .map(|(e, c)| {
            let stages: Vec<&str> = plants
                .and_then(|r| r.get(&c.crop_def_id))
                .map(|d| d.stages())
                .unwrap_or_else(|| humanity_core::ecs::components::DEFAULT_GROWTH_STAGES.iter().copied().collect());
            let dead = c.growth_stage.as_str() == humanity_core::ecs::components::STAGE_DEAD;
            (e, !dead && stages.last().is_some_and(|l| *l == c.growth_stage.as_str()))
        })
        .collect();
    let ents: Vec<hecs::Entity> = crops.iter().map(|c| c.0).collect();
    let picking = farming::picking::GuiView::for_crops(world, data, &ents);
    crops.into_iter().filter(|(e, mature)| picking.ready(*e, *mature)).map(|(e, _)| e.to_bits().into()).collect()
}

/// "Take to backpack" on one of the first food in `foods` home storage holds, as the Inventory
/// page sends it (a transfer into the backpack, the stack in storage one smaller, where it came
/// from kept for an overflow's return). False when storage holds none of them.
fn take_from_storage(gui: &mut GuiState, foods: &[String]) -> bool {
    let Some(idx) = foods.iter().find_map(|f| gui.placed_items.iter().position(|p| &p.key == f && p.qty > 0)) else { return false };
    let mut one = gui.placed_items[idx].clone();
    one.qty = 1;
    gui.pending_inventory_transfers.push(humanity_systems::inventory::TransferOp {
        item_id: one.key.clone(),
        qty: 1,
        add: true,
        wear: one.wear,
        quality: one.quality,
        age_s: one.age_s,
    });
    gui.pending_take_origins.push(one);
    if gui.placed_items[idx].qty <= 1 {
        gui.placed_items.remove(idx);
    } else {
        gui.placed_items[idx].qty -= 1;
    }
    true
}
