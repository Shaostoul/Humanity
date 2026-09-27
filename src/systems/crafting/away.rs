//! Automated machines through the time the player was away (offline
//! progression, 2026-09-27; docs/design/offline-progression.md).
//!
//! In a session an automated machine (an `AutoRefine`: the grain mill, the
//! smelter, the workbench) starts a batch whenever its inputs are on hand and
//! it is not resting at its keep target, spends the inputs, runs for the
//! recipe's craft time, files the product in home storage, and starts again.
//! This runs the same machines through the hours away, moment by moment, by
//! the same rules:
//!
//! - a batch starts only on inputs that were really there at that moment:
//!   the backpack, home storage, what an earlier batch made, drone ore from
//!   the moment it landed, tap water from the tanks. Inputs are spent when a
//!   batch STARTS, as in a session, so nothing is made from stock that was
//!   not there and nothing is reserved that was not;
//! - a machine rests at its keep target (the mill's 20 flour);
//! - an electric machine works only on power the home could have spared: the
//!   Usage meter's day average of what the home makes less what it uses
//!   (`day_power_balance`), paid out no faster than the home made it. A home
//!   that does not cover its own day runs no electric machine while away;
//! - a batch still running when the time runs out is left running, where
//!   the player finds it.
//!
//! What this never does is the doc's "must NOT advance offline": a machine
//! only turns stock the player chose to automate into its product.

use std::collections::HashMap;
use std::sync::Mutex;

use super::{ActiveCraft, CraftingSystem, Recipe, RecipeRegistry};
use crate::hot_reload::data_store::DataStore;
use crate::systems::inventory::{Inventory, ItemRegistry};

/// DataStore key for the time away handed to the machines.
pub const AWAY_WORK: &str = "crafting_away_work";

/// The sun-hours the construction page reads the Usage meter at. Only a panel
/// with no site `average_watts` uses it.
const METER_SUN_HOURS: f32 = 4.5;

/// Most batches one return may catch up, across every machine. A real home
/// runs out of inputs or reaches its keep targets long before this; it only
/// bounds a pathological save (a recipe with no inputs and no keep target).
const MAX_AWAY_BATCHES: usize = 200_000;

/// The time away handed to the automated machines (save_load::resume_home).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AwayWork {
    /// Game seconds the player was away.
    pub secs: f64,
    /// Each machine's batch in flight when the game was saved: machine
    /// instance id -> the seconds it still needed then. The machine was busy
    /// with it until that moment of the time away.
    pub busy: HashMap<String, f64>,
    /// The home's power balance averaged over a day, watts: what it makes on
    /// its own less what it uses (`day_power_balance`), with ship life support
    /// Station-supplied `[0]` and Realistic `[1]`.
    pub power_balance_w: [f32; 2],
    /// Ore the drone brought home during the time away, (seconds in, cargo)
    /// (mining::advance_away). It is in the backpack already; a machine may
    /// use it only from the moment it landed.
    pub hauls: Vec<(f64, Vec<(String, u32)>)>,
}

/// The home's power balance averaged over a day, in watts, as the Usage meter
/// counts both sides (`MachineHome::utility_meters`: each panel's yield at the
/// site, a turbine's site average, no backstop genset, every machine's day
/// average draw): `[Station-supplied, Realistic]` ship life support. Negative
/// when the home does not make what it uses. This is the whole of the power an
/// electric machine may draw while the player is away: the doc's rule that
/// nothing runs on power the home could not have supplied, and the batteries
/// are not a source (they only move a day's power from noon to night).
pub fn day_power_balance(home: &crate::machines::MachineHome) -> [f32; 2] {
    let balance = |life_support_on_grid: bool| {
        home.utility_meters(METER_SUN_HOURS, crate::machines::MeterBasis { life_support_on_grid })
            .into_iter()
            .find(|m| m.utility == "power")
            .map_or(0.0, |m| (m.generation - m.demand) * 1000.0 / 24.0)
    };
    [balance(false), balance(true)]
}

/// Hand the machines the time away (save_load::resume_home). Replaces, never
/// adds: resuming the same save twice is the same time away, and None clears
/// a hand-over the machines never took.
pub fn hand_over(data: &DataStore, work: Option<AwayWork>) {
    if let Some(Ok(mut slot)) = data.get::<Mutex<Option<AwayWork>>>(AWAY_WORK).map(|m| m.lock()) {
        *slot = work;
    }
}

/// Take the time away (the CraftingSystem, once the machines exist).
pub fn take(data: &DataStore) -> Option<AwayWork> {
    data.get::<Mutex<Option<AwayWork>>>(AWAY_WORK).and_then(|m| m.lock().ok().and_then(|mut s| s.take()))
}

/// Are the home's automated machines in the world yet? They spawn with the
/// home (the menu spawns them without their bodies, world entry with them).
pub fn machines_present(world: &hecs::World) -> bool {
    world.query::<&crate::ecs::components::AutoRefine>().iter().next().is_some()
}

/// What the machines did while the player was away.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct AwayReport {
    /// Batches finished during the time away.
    pub batches: usize,
    /// Everything they made, (item id, count), sorted by id.
    pub made: Vec<(String, u32)>,
    /// Machine types that could not run for want of power.
    pub unpowered: Vec<String>,
}

/// "a", "a and b", "a, b and c".
pub fn join_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// The "while you were away" line for the machines, or None when they did
/// nothing worth telling. Never silent about a machine that sat idle for
/// power: an idle mill with no word reads as a bug.
pub fn notice(report: &AwayReport, items: Option<&ItemRegistry>) -> Option<String> {
    let name = |id: &str| items.and_then(|r| r.items.get(id).map(|d| d.name.clone())).unwrap_or_else(|| id.to_string());
    let mut parts = Vec::new();
    if !report.made.is_empty() {
        let list: Vec<String> = report.made.iter().map(|(id, q)| format!("{q} {}", name(id))).collect();
        parts.push(format!("While you were away, the home's machines made {}.", join_list(&list)));
    }
    for kind in &report.unpowered {
        parts.push(format!(
            "The {} did not run while you were away: the home makes no more power than it uses, so it had none to spare.",
            kind.replace('_', " ")
        ));
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// One automated machine, as the time away sees it.
struct Machine {
    entity: hecs::Entity,
    id: Option<String>,
    kind: String,
    recipe: String,
    keep: Option<u32>,
    /// On the grid (a PowerConsumer).
    electric: bool,
    /// What it draws over its idle draw while a batch runs, watts. A steady
    /// load's full draw is already in the home's day demand.
    working_extra_w: f32,
    pad: Option<(glam::Vec3, glam::Quat)>,
    /// Seconds into the time away when it may next try to start.
    free_at: f64,
    /// The batch it is running: (finishes at, recipe id).
    batch: Option<(f64, String)>,
}

/// Why a machine did or did not start.
enum Start {
    Now,
    /// Short of power until then (the home had not yet made enough to spare).
    At(f64),
    /// Inputs missing, keep target reached or the pad full: try again when
    /// anything changes.
    Blocked,
    /// The home cannot spare it any power at all.
    NoPower,
}

/// The stock the machines draw on during the time away.
struct Ledger<'a> {
    player: hecs::Entity,
    /// Made during the time away and bound for home storage, not yet filed
    /// (the main loop files it after this tick, from "home_stock_outputs").
    pool: HashMap<String, u32>,
    /// Drone ore in the backpack that has not landed yet at the moment
    /// being run.
    unarrived: HashMap<String, u32>,
    /// Home storage (the Barn), mirrored by the main loop before the tick;
    /// what is taken from it here the main loop takes out of the Barn after.
    home: Option<&'a Mutex<HashMap<String, u32>>>,
    fluids: Option<&'a crate::systems::fluids::FluidTable>,
}

impl Ledger<'_> {
    fn backpack(&self, world: &hecs::World, id: &str) -> u32 {
        let have = world.get::<&Inventory>(self.player).map(|i| i.count_item(id)).unwrap_or(0);
        have.saturating_sub(self.unarrived.get(id).copied().unwrap_or(0))
    }

    fn stored(&self, id: &str) -> u32 {
        self.home.and_then(|m| m.lock().ok().map(|s| s.get(id).copied().unwrap_or(0))).unwrap_or(0)
    }

    /// On hand for a keep target: the session counts the backpack and home storage.
    fn on_hand(&self, world: &hecs::World, id: &str) -> u32 {
        self.backpack(world, id) + self.pool.get(id).copied().unwrap_or(0) + self.stored(id)
    }

    /// Available as an input: on hand, and tap water from the tanks.
    fn available(&self, world: &hecs::World, id: &str) -> u32 {
        self.on_hand(world, id) + self.fluids.map_or(0, |t| crate::systems::fluids::tap_units(t, world, id))
    }

    /// Spend `qty` of `id`: the backpack first, as a session does, then home
    /// storage (what was made while away, then what was already there), then
    /// the tanks for a measure of tap water.
    fn spend(&mut self, world: &mut hecs::World, id: &str, qty: u32) {
        let mut left = qty;
        let from_pack = self.backpack(world, id).min(left);
        if from_pack > 0 {
            if let Ok(mut inv) = world.get::<&mut Inventory>(self.player) {
                inv.remove_item(id, from_pack);
            }
            left -= from_pack;
        }
        if left > 0 {
            if let Some(p) = self.pool.get_mut(id) {
                let take = (*p).min(left);
                *p -= take;
                left -= take;
            }
        }
        if left > 0 {
            if let Some(Ok(mut s)) = self.home.map(|m| m.lock()) {
                if let Some(c) = s.get_mut(id) {
                    let take = (*c).min(left);
                    *c -= take;
                    left -= take;
                }
            }
        }
        if left > 0 {
            if let Some(l) = self.fluids.and_then(|t| t.tap_litres(id)) {
                crate::systems::fluids::draw_from_tanks(world, left as f32 * l);
            }
        }
    }

    /// Move what a delivery filed for home storage into the pool, so a later
    /// batch this same pass can use it and a keep target counts it.
    fn collect_filed(&mut self, data: &DataStore) {
        if let Some(Ok(mut out)) = data.get::<Mutex<Vec<(String, u32)>>>("home_stock_outputs").map(|m| m.lock()) {
            for (id, q) in out.drain(..) {
                *self.pool.entry(id).or_insert(0) += q;
            }
        }
    }

    /// Hand the pool back to the main loop to file in home storage.
    fn file(self, data: &DataStore) {
        if let Some(Ok(mut out)) = data.get::<Mutex<Vec<(String, u32)>>>("home_stock_outputs").map(|m| m.lock()) {
            let mut pool: Vec<(String, u32)> = self.pool.into_iter().filter(|(_, q)| *q > 0).collect();
            pool.sort();
            out.extend(pool);
        }
    }
}

impl CraftingSystem {
    /// Run the automated machines through the time away (see the module
    /// doc), then tell the player what they made. Called once, from the
    /// tick, with home storage present.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn run_away(
        &mut self,
        world: &mut hecs::World,
        data: &DataStore,
        work: &AwayWork,
        player: hecs::Entity,
        recipes: &RecipeRegistry,
        items: Option<&ItemRegistry>,
        kits: Option<&crate::systems::vehicles::VehicleKitRegistry>,
        fluids: Option<&crate::systems::fluids::FluidTable>,
    ) -> AwayReport {
        let report = self.run_away_machines(world, data, work, player, recipes, items, kits, fluids);
        if let Some(msg) = notice(&report, items) {
            if let Some(Ok(mut n)) = data.get::<Mutex<Vec<String>>>("player_notices").map(|m| m.lock()) {
                n.push(msg);
            }
        }
        log::info!(
            "Offline catch-up: the machines ran {} batches in {:.0} s away ({:?})",
            report.batches,
            work.secs,
            report.made
        );
        report
    }

    #[allow(clippy::too_many_arguments)]
    fn run_away_machines(
        &mut self,
        world: &mut hecs::World,
        data: &DataStore,
        work: &AwayWork,
        player: hecs::Entity,
        recipes: &RecipeRegistry,
        items: Option<&ItemRegistry>,
        kits: Option<&crate::systems::vehicles::VehicleKitRegistry>,
        fluids: Option<&crate::systems::fluids::FluidTable>,
    ) -> AwayReport {
        use crate::ecs::components::{AutoRefine, MachineInstanceId, MachineType, PowerConsumer, StationLoad, Transform};
        let mut report = AwayReport::default();
        let window = work.secs;
        if !(window > 0.0) {
            return report;
        }
        let balance_w = work.power_balance_w[usize::from(crate::systems::life_support::is_realistic(data))];
        let spare_w = f64::from(balance_w.max(0.0));
        let mut used_wh = 0.0_f64;

        let mut machines: Vec<Machine> = world
            .query::<(
                &AutoRefine,
                Option<&MachineInstanceId>,
                Option<&MachineType>,
                Option<&PowerConsumer>,
                Option<&StationLoad>,
                Option<&Transform>,
            )>()
            .iter()
            .filter(|(_, (auto, ..))| recipes.recipes.contains_key(&auto.recipe_id))
            .map(|(entity, (auto, id, kind, pc, load, tf))| Machine {
                entity,
                id: id.map(|i| i.0.clone()),
                kind: kind.map_or_else(|| auto.recipe_id.clone(), |k| k.0.clone()),
                recipe: auto.recipe_id.clone(),
                keep: auto.keep,
                electric: pc.is_some(),
                working_extra_w: load.map_or(0.0, |l| (l.active_watts - l.idle_watts).max(0.0)),
                pad: tf.map(|t| (t.position, t.rotation)),
                free_at: 0.0,
                batch: None,
            })
            .collect();

        // The batch each machine had in flight at the save. One that finished
        // within the time away (restored_crafts counted it down to zero) is
        // taken over here, so it lands at the moment it finished and the
        // machine carries on after it. One still running is left to the
        // session, and so is its machine.
        for m in machines.iter_mut() {
            let running = self.active_crafts.iter().position(|c| {
                c.crafter == m.entity || (m.id.is_some() && c.auto && c.machine_id == m.id)
            });
            let saved = m.id.as_ref().and_then(|id| work.busy.get(id)).copied();
            match (running, saved) {
                (Some(i), Some(b)) if self.active_crafts[i].time_remaining <= 0.0 && b <= window => {
                    let c = self.active_crafts.remove(i);
                    m.batch = Some((b.max(0.0), c.recipe_id));
                }
                (Some(_), _) => m.free_at = f64::INFINITY,
                // Its saved batch already finished through the session.
                (None, Some(b)) => m.free_at = b.max(0.0),
                (None, None) => {}
            }
        }

        let mut hauls = work.hauls.clone();
        hauls.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut unarrived: HashMap<String, u32> = HashMap::new();
        for (_, cargo) in &hauls {
            for (id, q) in cargo {
                *unarrived.entry(id.clone()).or_insert(0) += q;
            }
        }
        let mut ledger = Ledger {
            player,
            pool: HashMap::new(),
            unarrived,
            home: data.get::<Mutex<HashMap<String, u32>>>("home_stock"),
            fluids,
        };
        // Already filed this tick (a harvest the pack could not take) counts
        // as home storage from the start.
        ledger.collect_filed(data);

        let mut made: HashMap<String, u32> = HashMap::new();
        let mut unpowered: Vec<String> = Vec::new();
        let mut next_haul = 0;
        let mut now = 0.0_f64;
        let mut started = 0usize;
        loop {
            // 1. What finished by now lands; what the drone brought by now arrives.
            for m in machines.iter_mut() {
                let Some((done, rid)) = m.batch.clone() else { continue };
                if done > now {
                    continue;
                }
                m.batch = None;
                m.free_at = done;
                if let Some(recipe) = recipes.recipes.get(&rid) {
                    Self::deliver_goods(world, data, recipe, player, m.pad, items, kits, Some(m.entity), true);
                    ledger.collect_filed(data);
                    Self::credit_craft(data, recipe);
                    for (id, q) in &recipe.outputs {
                        *made.entry(id.clone()).or_insert(0) += q;
                    }
                    report.batches += 1;
                }
            }
            while let Some((_, cargo)) = hauls.get(next_haul).filter(|(t, _)| *t <= now) {
                for (id, q) in cargo {
                    if let Some(u) = ledger.unarrived.get_mut(id) {
                        *u = u.saturating_sub(*q);
                    }
                }
                next_haul += 1;
            }

            // 2. Every idle machine whose moment has come tries to start, and
            // again while any did: a start that spends a product can put a
            // machine that was resting at its keep target back to work, which
            // a session sees on its very next frame.
            loop {
                let mut any = false;
                for m in machines.iter_mut() {
                    if m.batch.is_some() || m.free_at > now {
                        continue;
                    }
                    let Some(recipe) = recipes.recipes.get(&m.recipe) else { continue };
                    let energy_wh = f64::from(m.working_extra_w) * f64::from(recipe.craft_time.max(0.0)) / 3600.0;
                    match Self::away_start(m, recipe, now, world, &ledger, kits, balance_w, spare_w, used_wh, energy_wh) {
                        Start::Now => {
                            for (id, q) in &recipe.inputs {
                                ledger.spend(world, id, *q);
                            }
                            used_wh += energy_wh;
                            m.batch = Some((now + f64::from(recipe.craft_time.max(0.01)), m.recipe.clone()));
                            started += 1;
                            any = true;
                        }
                        Start::At(t) => m.free_at = t,
                        Start::Blocked => {}
                        Start::NoPower => {
                            m.free_at = f64::INFINITY;
                            if !unpowered.contains(&m.kind) {
                                unpowered.push(m.kind.clone());
                            }
                        }
                    }
                }
                if !any {
                    break;
                }
            }
            if started >= MAX_AWAY_BATCHES {
                log::warn!("Offline catch-up: stopped at {MAX_AWAY_BATCHES} machine batches");
                break;
            }

            // 3. On to the next moment anything changes: a batch finishing,
            // ore landing, or power enough for a machine waiting on it.
            let next = machines
                .iter()
                .filter_map(|m| m.batch.as_ref().map(|b| b.0))
                .chain(hauls.get(next_haul).map(|h| h.0))
                .chain(machines.iter().filter(|m| m.batch.is_none() && m.free_at > now).map(|m| m.free_at))
                .fold(f64::INFINITY, f64::min);
            if !(next <= window) {
                break;
            }
            now = next;
        }

        // A batch still running when the time ran out is where the player
        // finds it: in flight, its inputs spent, the rest of its time to go.
        for m in machines {
            if let Some((done, recipe_id)) = m.batch {
                self.active_crafts.push(ActiveCraft {
                    recipe_id,
                    time_remaining: (done - window).max(0.0) as f32,
                    crafter: m.entity,
                    auto: true,
                    pad: m.pad,
                    machine_id: m.id,
                    waiting_notified: false,
                    pause_notified: false,
                });
            }
        }
        ledger.file(data);

        let mut made: Vec<(String, u32)> = made.into_iter().collect();
        made.sort();
        report.made = made;
        report.unpowered = unpowered;
        report
    }

    /// Can this machine start a batch at `now` (see `Start`)? The session's
    /// start rules, in the session's order, with power judged against the
    /// home's spare power instead of the live grid.
    #[allow(clippy::too_many_arguments)]
    fn away_start(
        m: &Machine,
        recipe: &Recipe,
        now: f64,
        world: &hecs::World,
        ledger: &Ledger,
        kits: Option<&crate::systems::vehicles::VehicleKitRegistry>,
        balance_w: f32,
        spare_w: f64,
        used_wh: f64,
        energy_wh: f64,
    ) -> Start {
        if recipe.inputs.iter().any(|(id, q)| ledger.available(world, id) < *q) {
            return Start::Blocked;
        }
        if let (Some(k), Some((out, _))) = (m.keep, recipe.outputs.first()) {
            if ledger.on_hand(world, out) >= k {
                return Start::Blocked;
            }
        }
        if Self::has_vehicle_output(recipe, kits) {
            let (base, rot) = m.pad.unwrap_or((glam::Vec3::ZERO, glam::Quat::IDENTITY));
            if Self::free_pad_lane(world, base, rot).is_none() {
                return Start::Blocked;
            }
        }
        if m.electric {
            if balance_w < 0.0 {
                return Start::NoPower;
            }
            if energy_wh > 0.0 {
                if spare_w <= 0.0 {
                    return Start::NoPower;
                }
                // Paid out no faster than the home made it: by time t the
                // machines may have drawn at most spare x t.
                let ready_at = (used_wh + energy_wh) * 3600.0 / spare_w;
                if ready_at > now + 1e-9 {
                    return Start::At(ready_at);
                }
            }
        }
        Start::Now
    }
}
