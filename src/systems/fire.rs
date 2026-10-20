//! Fire system — fire ignition, intensity, spread, suppression.
//!
//! Each tick:
//!   1. Each active `Fire` burns for the tick (`burn_step`): its intensity falls at the
//!      strength of the `FireSuppressor`s in range, its fuel goes at `FUEL_BURN_RATE` times its
//!      intensity, and it goes out at the moment its intensity or its fuel reaches zero.
//!   2. Fires that went out are removed.
//!   3. Each fire rolls a spread check against nearby `Flammable` entities
//!      within `Flammable.ignition_dist`. Hotter fires spread faster.
//!   4. Fires hurt co-located entities. A body (anything with `StatusEffects`: the player) is
//!      burned: the fire pushes the heat onto it as an injury, and the food system, which owns
//!      the body, takes the health and gives the burn its depth (first-hour audit S8,
//!      `systems::wounds`, data/medical/wounds.ron). Anything else with `Health` loses
//!      `DAMAGE_PER_SEC` at full intensity.
//!
//! THE SAME SECONDS DO THE SAME HARM AT ANY FRAME RATE (BUG-164's note on this file,
//! 2026-10-20). What a fire does in a tick is its DOSE, the integral of its intensity over the
//! part of the tick it burned, worked out exactly in f64 (`burn_step`): suppression takes its
//! intensity down in a straight line, so the integral is a trapezoid, and a fire whose fuel runs
//! out or whose intensity reaches zero part-way through a tick burns only to that moment. Its
//! harm, its fuel and its chance to spread all follow from the dose. Its intensity and its fuel
//! stay f32 on the `Fire` component; each change goes through `food::change_exactly` with the
//! fire's carry, as Health does for what is not a body, so nothing f32 rounding holds back is
//! lost. Before: a frame took `intensity * dt` from Health and from the fuel in f32 at the
//! frame's starting intensity, and burned its whole last frame past its fuel, so the harm
//! depended on the frame rate (a fire with 3,000 s of fuel burned it 17% too fast at 240 frames
//! a second, because the fuel's f32 step there is 0.000244 and a frame's share 0.000208).

use std::collections::HashMap;
use std::path::Path;

use rand::Rng;
use serde::Deserialize;

use crate::ecs::components::{Fire, FireSuppressor, Flammable, Health, StatusEffects, Transform};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Fuel burned per second at intensity = 1.0.
const FUEL_BURN_RATE: f64 = 1.0;
/// Spread chance per second per nearby flammable, scaled by intensity.
const SPREAD_CHANCE_PER_SEC: f64 = 0.05;
/// Damage per second to an entity that is not a body at the same position as a Fire (at
/// intensity = 1.0). A body's is data/medical/wounds.ron's `burns.health_per_s`.
const DAMAGE_PER_SEC: f64 = 1.0;
/// Distance threshold for "co-located" damage (meters).
const DAMAGE_DIST: f32 = 1.5;

/// Top-level RON schema for `data/fire_system.ron`.
#[derive(Debug, Deserialize)]
pub struct FireData {
    #[serde(default)] pub ignition_sources: Vec<ron::Value>,
    #[serde(default)] pub fire_behaviors: Vec<ron::Value>,
    #[serde(default)] pub suppression_systems: Vec<ron::Value>,
    #[serde(default)] pub fire_damage_effects: Vec<ron::Value>,
}

/// One fire's tick, worked out exactly (`burn_step`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Burn {
    /// Seconds at full intensity this tick: the integral of the intensity over the time it burned.
    pub dose_s: f64,
    /// Its intensity at the end of the tick, or when it went out.
    pub intensity: f64,
    /// Its fuel at the end of the tick.
    pub fuel: f64,
    /// It went out this tick: its intensity or its fuel reached zero.
    pub out: bool,
}

/// Burn a fire of intensity `intensity` and `fuel` seconds of fuel (at full intensity) for `dt`
/// seconds while suppressors take `suppression_per_s` of intensity a second. Exact, so the same
/// seconds give the same dose whether they come in one step or in many: the intensity falls in
/// a straight line to zero at `intensity / suppression_per_s`, the fuel burns at
/// `FUEL_BURN_RATE` times the intensity, and the fire burns until the first of the end of the
/// tick, its intensity reaching zero, or its fuel running out.
pub fn burn_step(intensity: f64, suppression_per_s: f64, fuel: f64, dt: f64) -> Burn {
    let (i0, s, dt) = (intensity.max(0.0), suppression_per_s.max(0.0), dt.max(0.0));
    let fuel = fuel.max(0.0);
    if i0 <= 0.0 || fuel <= 0.0 {
        return Burn { dose_s: 0.0, intensity: i0, fuel, out: true };
    }
    // When the intensity reaches zero.
    let t_zero = if s > 0.0 { i0 / s } else { f64::INFINITY };
    // When the fuel runs out: FUEL_BURN_RATE * (i0 t - s t^2 / 2) = fuel, the smaller root,
    // written so it holds at s = 0 too (it is then fuel / (rate * i0)); none before t_zero
    // when the discriminant is negative.
    let f = fuel / FUEL_BURN_RATE;
    let disc = i0 * i0 - 2.0 * s * f;
    let t_fuel = if disc >= 0.0 { 2.0 * f / (i0 + disc.sqrt()) } else { f64::INFINITY };
    let t_end = dt.min(t_zero).min(t_fuel);
    let dose_s = (i0 * t_end - 0.5 * s * t_end * t_end).max(0.0);
    let intensity = (i0 - s * t_end).max(0.0);
    let fuel_left = if t_end >= t_fuel { 0.0 } else { (fuel - FUEL_BURN_RATE * dose_s).max(0.0) };
    let out = t_end < dt || intensity <= 0.0 || fuel_left <= 0.0;
    Burn { dose_s, intensity, fuel: fuel_left, out }
}

/// What f32 rounding has held back from one fire's intensity and fuel (`food::change_exactly`).
#[derive(Debug, Clone, Copy, Default)]
struct FireCarry {
    intensity: f64,
    fuel: f64,
}

/// Tracks ignition sources, fire spread, and suppression.
pub struct FireSystem {
    pub data: FireData,
    /// Entities scheduled for new Fire components on next tick (queued from spread checks).
    pending_ignitions: Vec<(hecs::Entity, f32)>,
    /// What f32 rounding has held back from each burning non-body's Health (BUG-164,
    /// `food::change_exactly`).
    carry: HashMap<hecs::Entity, f64>,
    /// What it has held back from each fire's intensity and fuel.
    fire_carry: HashMap<hecs::Entity, FireCarry>,
}

impl FireSystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("fire_system.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(ignition_sources:[],fire_behaviors:[],suppression_systems:[],fire_damage_effects:[])".to_string()
        });
        let data: FireData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse fire_system.ron: {e}");
            FireData {
                ignition_sources: vec![],
                fire_behaviors: vec![],
                suppression_systems: vec![],
                fire_damage_effects: vec![],
            }
        });
        log::info!(
            "Loaded fire data: {} ignition sources, {} behaviors",
            data.ignition_sources.len(), data.fire_behaviors.len()
        );
        Self { data, pending_ignitions: Vec::new(), carry: HashMap::new(), fire_carry: HashMap::new() }
    }
}

impl System for FireSystem {
    fn name(&self) -> &str { "FireSystem" }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, _data: &DataStore) {
        if dt <= 0.0 { return; }
        let dt = f64::from(dt);
        let mut rng = rand::thread_rng();

        // Apply pending ignitions from previous tick (deferred to avoid alias).
        for (entity, fuel) in self.pending_ignitions.drain(..) {
            let _ = world.insert_one(entity, Fire { intensity: 0.3, fuel_remaining: fuel });
            self.fire_carry.remove(&entity);
        }

        // Snapshot fire positions, exact intensities and fuel, and entities.
        let active_fires: Vec<(hecs::Entity, glam::Vec3, f64, f64)> = world
            .query::<(&Transform, &Fire)>()
            .iter()
            .map(|(e, (t, f))| {
                let c = self.fire_carry.get(&e).copied().unwrap_or_default();
                (e, t.position, f64::from(f.intensity) + c.intensity, f64::from(f.fuel_remaining) + c.fuel)
            })
            .collect();

        // Snapshot suppressors.
        let suppressors: Vec<(glam::Vec3, f32, f32)> = world
            .query::<(&Transform, &FireSuppressor)>()
            .iter()
            .map(|(_, (t, s))| (t.position, s.range, s.strength))
            .collect();

        // Snapshot flammables (entity, position, ignition_dist, fuel_seconds).
        let flammables: Vec<(hecs::Entity, glam::Vec3, f32, f32)> = world
            .query::<(&Transform, &Flammable)>()
            .iter()
            .filter(|(e, _)| world.get::<&Fire>(*e).is_err())  // not already burning
            .map(|(e, (t, f))| (e, t.position, f.ignition_dist, f.fuel_seconds))
            .collect();

        // Snapshot health-bearers for damage.
        let damageables: Vec<(hecs::Entity, glam::Vec3)> = world
            .query::<(&Transform, &Health)>()
            .iter()
            .map(|(e, (t, _))| (e, t.position))
            .collect();

        // Each fire's tick, worked out exactly.
        let mut burns: Vec<(hecs::Entity, Burn)> = Vec::new();
        for (fire_entity, fire_pos, intensity, fuel) in &active_fires {
            let suppression: f64 = suppressors
                .iter()
                .filter(|(sup_pos, range, _)| fire_pos.distance(*sup_pos) <= *range)
                .map(|(_, _, strength)| f64::from(*strength))
                .sum();
            let burn = burn_step(*intensity, suppression, *fuel, dt);
            burns.push((*fire_entity, burn));

            // Spread check: the chance a flammable in reach catches within the tick, from the
            // dose (exact for any length of tick: 1 - e^(-rate x dose)).
            let chance = 1.0 - (-SPREAD_CHANCE_PER_SEC * burn.dose_s).exp();
            for (target, target_pos, ignition_dist, fuel) in &flammables {
                let dist = fire_pos.distance(*target_pos);
                if dist > *ignition_dist { continue; }
                if rng.gen::<f64>() < chance {
                    self.pending_ignitions.push((*target, *fuel));
                }
            }

            // Hurt co-located health-bearers: a body is burned (its owner, the
            // food system, takes the health and gives the burn), anything else
            // loses its share exactly.
            if burn.dose_s <= 0.0 { continue; }
            for (target, t_pos) in &damageables {
                if fire_pos.distance(*t_pos) > DAMAGE_DIST { continue; }
                if world.get::<&StatusEffects>(*target).is_ok() {
                    crate::systems::wounds::injure(world, *target, crate::systems::wounds::Injury::Heat { dose_s: burn.dose_s });
                } else if let Ok(mut h) = world.get::<&mut Health>(*target) {
                    let carry = self.carry.entry(*target).or_default();
                    let max = h.max;
                    crate::systems::food::change_exactly(&mut h.current, carry, -burn.dose_s * DAMAGE_PER_SEC, 0.0, max);
                }
            }
        }

        // Forget the carry of anything that no longer has health.
        self.carry.retain(|e, _| damageables.iter().any(|(d, _)| d == e));

        // Each fire takes its tick's change exactly; one that went out is removed.
        let mut deaths: Vec<hecs::Entity> = Vec::new();
        for (entity, burn) in burns {
            if burn.out {
                deaths.push(entity);
                continue;
            }
            let carry = self.fire_carry.entry(entity).or_default();
            if let Ok(mut fire) = world.get::<&mut Fire>(entity) {
                let (was_i, was_f) = (f64::from(fire.intensity) + carry.intensity, f64::from(fire.fuel_remaining) + carry.fuel);
                crate::systems::food::change_exactly(&mut fire.intensity, &mut carry.intensity, burn.intensity - was_i, 0.0, 1.0);
                crate::systems::food::change_exactly(&mut fire.fuel_remaining, &mut carry.fuel, burn.fuel - was_f, 0.0, f32::INFINITY);
            }
        }

        // Remove dead fires.
        for entity in deaths {
            let _ = world.remove_one::<Fire>(entity);
            self.fire_carry.remove(&entity);
            log::debug!("Fire: {:?} extinguished", entity);
        }
        self.fire_carry.retain(|e, _| active_fires.iter().any(|(f, ..)| f == e));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tick split into many gives the same dose, intensity and fuel as one, with
    /// suppression, through the moment the fuel runs out, and at none.
    #[test]
    fn a_burn_split_into_many_steps_is_the_same_burn() {
        for (i0, s, fuel) in [(1.0, 0.0, 100.0), (0.5, 0.02, 4.0), (1.0, 0.1, 50.0), (0.05, 0.0, 0.01)] {
            let whole = burn_step(i0, s, fuel, 12.0);
            let (mut i, mut f, mut dose, mut out) = (i0, fuel, 0.0, false);
            for _ in 0..12 * 240 {
                if out {
                    break;
                }
                let b = burn_step(i, s, f, 1.0 / 240.0);
                dose += b.dose_s;
                (i, f, out) = (b.intensity, b.fuel, b.out);
            }
            assert!((dose - whole.dose_s).abs() < 1e-9, "({i0}, {s}, {fuel}): {dose} in steps, {} at once", whole.dose_s);
            assert_eq!(out, whole.out, "({i0}, {s}, {fuel}): goes out the same");
        }
        // Worked by hand: 0.5 falling at 0.02 a second burns 4 s of fuel in exactly 10 s
        // (0.5 t - 0.01 t^2 = 4), and a fire at 1 falling at 0.1 a second is out at 10 s, a dose of 5.
        let b = burn_step(0.5, 0.02, 4.0, 12.0);
        assert!((b.dose_s - 4.0).abs() < 1e-12 && b.out && b.fuel == 0.0, "{b:?}");
        let b = burn_step(1.0, 0.1, 50.0, 12.0);
        assert!((b.dose_s - 5.0).abs() < 1e-12 && b.out && b.intensity == 0.0, "{b:?}");
    }
}
