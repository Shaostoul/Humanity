//! Atmospheric system -- gas composition, pressure, and breathability simulation.
//!
//! Simulates planetary atmospheres and enclosed spaces (ship rooms, buildings).
//! Tracks gas composition, checks for explosive mixtures, monitors oxygen levels
//! for suffocation, detects toxic gases, and handles sealed/unsealed room equalization.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::ecs::components::{Health, PowerConsumer, Transform};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

// ── Data types ─────────────────────────────────────────────

/// Atmospheric state for a planet region or enclosed space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Atmosphere {
    /// Gas composition: gas name -> percentage (0-100).
    pub composition: HashMap<String, f32>,
    /// Total atmospheric pressure in atmospheres.
    pub pressure_atm: f32,
    /// Temperature in Kelvin.
    pub temperature_k: f32,
    /// Relative humidity (0.0-1.0).
    pub humidity: f32,
    /// Wind speed in m/s (0 in enclosed spaces).
    pub wind_speed: f32,
    /// Normalized wind direction vector [x, y, z].
    pub wind_direction: [f32; 3],
    /// Whether this atmosphere contains enough O2 to breathe.
    pub breathable: bool,
    /// Whether toxic gases are present above safe thresholds.
    pub toxic: bool,
    /// Whether any flammable gas is within its explosive concentration range.
    pub flammable: bool,
}

impl Default for Atmosphere {
    /// Earth-like atmosphere.
    fn default() -> Self {
        let mut composition = HashMap::new();
        composition.insert("N2".to_string(), 78.0);
        composition.insert("O2".to_string(), 21.0);
        composition.insert("Ar".to_string(), 0.93);
        composition.insert("CO2".to_string(), 0.04);

        Self {
            composition,
            pressure_atm: 1.0,
            temperature_k: 293.0,
            humidity: 0.4,
            wind_speed: 0.0,
            wind_direction: [1.0, 0.0, 0.0],
            breathable: true,
            toxic: false,
            flammable: false,
        }
    }
}

impl Atmosphere {
    /// Create a vacuum (space).
    pub fn vacuum() -> Self {
        Self {
            composition: HashMap::new(),
            pressure_atm: 0.0,
            temperature_k: 2.7, // cosmic microwave background
            humidity: 0.0,
            wind_speed: 0.0,
            wind_direction: [0.0, 0.0, 0.0],
            breathable: false,
            toxic: false,
            flammable: false,
        }
    }

    /// Get the percentage of a specific gas, defaulting to 0.
    pub fn gas_percent(&self, gas: &str) -> f32 {
        self.composition.get(gas).copied().unwrap_or(0.0)
    }
}

/// An enclosed space with its own atmosphere (ship room, building, habitat).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnclosedSpace {
    /// Interior volume in cubic meters.
    pub volume_m3: f32,
    /// Current atmosphere inside the space.
    pub atmosphere: Atmosphere,
    /// Whether the space is fully sealed (airtight).
    pub sealed: bool,
    /// Gas exchange rate with outside atmosphere (m3/s). 0 if perfectly sealed.
    pub leak_rate: f32,
}

impl EnclosedSpace {
    /// Create a sealed room with Earth-like atmosphere.
    pub fn new_sealed(volume_m3: f32) -> Self {
        Self {
            volume_m3,
            atmosphere: Atmosphere::default(),
            sealed: true,
            leak_rate: 0.0,
        }
    }

    /// Create an unsealed room that exchanges gas with the outside.
    pub fn new_unsealed(volume_m3: f32) -> Self {
        Self {
            volume_m3,
            atmosphere: Atmosphere::default(),
            sealed: false,
            leak_rate: 0.1,
        }
    }
}

/// ECS component marking an entity as being in an enclosed space.
#[derive(Debug, Clone)]
pub struct InEnclosedSpace {
    /// Entity ID of the EnclosedSpace this entity is inside.
    pub space_entity: hecs::Entity,
}

/// ECS component marking an entity as an ignition source (sparks, fire, weapons).
#[derive(Debug, Clone)]
pub struct IgnitionSource;

/// Marks THE home's enclosed space (v0.617): the one whose atmosphere the live "Air" readout reflects.
/// Spawned with the home (alongside the HomeMachine marker so re-entering the world re-spawns it once).
#[derive(Debug, Clone, Copy)]
pub struct HomeAir;

/// An air handler on the home's life support (v0.618): while POWERED it regenerates O2 + scrubs CO2 in
/// the home space. `needs_power` gates it on the SAME entity's PowerConsumer -- cut the grid and the
/// scrubber stops, so occupancy slowly makes the air unbreathable (the power -> air -> Vitals chain).
/// Rates are percentage-points per second.
#[derive(Debug, Clone, Copy)]
pub struct AirScrubber {
    pub o2_regen_per_s: f32,
    pub co2_scrub_per_s: f32,
    pub needs_power: bool,
}

/// Home life-support tuning (v0.618). Occupancy (a household of ~3) steadily consumes O2 + emits CO2;
/// powered scrubbers offset it. Picked so one recycler covers the household with margin, and a full
/// power loss drains 21% -> ~19.5% (unbreathable) over a couple of minutes -- visible, not instant.
pub const HOME_OCCUPANTS: f32 = 3.0;
pub const O2_DRAIN_PER_PERSON_PER_S: f32 = 0.004;
pub const CO2_RISE_PER_PERSON_PER_S: f32 = 0.0008;

/// Live air readout, published to the DataStore each tick (key `air_status`) so the GUI can show the
/// home's running life-support state -- mirrors PowerStatus / WaterStatus. (v0.617)
#[derive(Debug, Clone, Copy, Default)]
pub struct AirStatus {
    /// Oxygen, percent of the mix (Earth ~21).
    pub o2_pct: f32,
    /// Carbon dioxide, percent (Earth ~0.04; rises with occupancy if scrubbing stops).
    pub co2_pct: f32,
    /// Total pressure, atmospheres.
    pub pressure_atm: f32,
    /// Temperature, Celsius (converted from the model's Kelvin for display).
    pub temp_c: f32,
    /// Whether the mix is breathable right now.
    pub breathable: bool,
}

// ── Flammable gas data ─────────────────────────────────────

/// Lower explosive limit (percentage in air) for common flammable gases.
fn explosive_lower_limit(gas: &str) -> Option<f32> {
    match gas {
        "H2" => Some(4.0),
        "CH4" => Some(5.0),
        "C3H8" => Some(2.1),   // propane
        "C2H6" => Some(3.0),   // ethane
        "C2H4" => Some(2.7),   // ethylene
        "CO" => Some(12.5),
        "C2H2" => Some(2.5),   // acetylene
        "NH3" => Some(15.0),
        _ => None,
    }
}

/// Upper explosive limit (percentage in air) for common flammable gases.
fn explosive_upper_limit(gas: &str) -> Option<f32> {
    match gas {
        "H2" => Some(75.0),
        "CH4" => Some(15.0),
        "C3H8" => Some(9.5),
        "C2H6" => Some(12.4),
        "C2H4" => Some(36.0),
        "CO" => Some(74.0),
        "C2H2" => Some(81.0),
        "NH3" => Some(28.0),
        _ => None,
    }
}

/// Toxic threshold (percentage) for harmful gases.
fn toxic_threshold(gas: &str) -> Option<f32> {
    match gas {
        "CO" => Some(0.005),      // 50 ppm
        "CO2" => Some(5.0),       // 5% is dangerous
        "H2S" => Some(0.005),     // 50 ppm
        "SO2" => Some(0.0005),    // 5 ppm
        "Cl2" => Some(0.0001),    // 1 ppm
        "NH3" => Some(0.005),     // 50 ppm
        "NO2" => Some(0.0003),    // 3 ppm
        "HCN" => Some(0.001),     // 10 ppm
        _ => None,
    }
}

// ── Constants ──────────────────────────────────────────────

/// O2 percentage below which entities start taking suffocation damage.
const O2_DAMAGE_THRESHOLD: f32 = 10.0;

/// O2 percentage that triggers a warning (but no damage yet).
const O2_WARNING_THRESHOLD: f32 = 16.0;

/// Health damage per second from suffocation (at 0% O2).
const SUFFOCATION_DAMAGE_MAX: f32 = 25.0;

/// Health damage per second from toxic gas exposure (at 10x threshold).
const TOXIC_DAMAGE_MAX: f32 = 15.0;

/// Decompression damage per second in vacuum.
const DECOMPRESSION_DAMAGE: f32 = 50.0;

/// Rate at which unsealed rooms equalize with outside (fraction per second).
const EQUALIZATION_RATE: f32 = 0.05;

// ── System ─────────────────────────────────────────────────

/// Simulates atmospheric conditions for planets and enclosed spaces.
pub struct AtmosphereSystem {
    /// How often to run a full simulation step (seconds).
    tick_interval: f32,
    /// Accumulated time since last step.
    elapsed: f32,
    /// The planet's outside atmosphere (default: Earth-like).
    pub outside_atmosphere: Atmosphere,
}

impl AtmosphereSystem {
    pub fn new() -> Self {
        Self {
            tick_interval: 1.0,
            elapsed: 0.0,
            outside_atmosphere: Atmosphere::default(),
        }
    }

    /// Set the planetary atmosphere (for alien worlds, vacuum, etc.).
    pub fn set_outside_atmosphere(&mut self, atmo: Atmosphere) {
        self.outside_atmosphere = atmo;
    }

    /// Evaluate flammability, breathability, and toxicity for an atmosphere.
    fn evaluate_atmosphere(atmo: &mut Atmosphere) {
        // Check breathability.
        let o2 = atmo.gas_percent("O2");
        atmo.breathable = o2 >= O2_WARNING_THRESHOLD && atmo.pressure_atm > 0.3;

        // Check flammability: any gas between its explosive limits.
        atmo.flammable = false;
        for (gas, &pct) in &atmo.composition {
            if let (Some(low), Some(high)) = (explosive_lower_limit(gas), explosive_upper_limit(gas)) {
                if pct >= low && pct <= high {
                    atmo.flammable = true;
                    break;
                }
            }
        }

        // Check toxicity: any gas above its toxic threshold.
        atmo.toxic = false;
        for (gas, &pct) in &atmo.composition {
            if let Some(threshold) = toxic_threshold(gas) {
                if pct > threshold {
                    atmo.toxic = true;
                    break;
                }
            }
        }
    }

    /// Equalize an enclosed space's atmosphere toward the outside atmosphere.
    fn equalize(space: &mut EnclosedSpace, outside: &Atmosphere, dt: f32) {
        if space.sealed && space.leak_rate <= 0.0 {
            return;
        }

        let rate = if space.sealed {
            space.leak_rate * 0.1 * dt // slow leak
        } else {
            EQUALIZATION_RATE * dt // fast equalization
        };

        let rate = rate.clamp(0.0, 1.0);

        // Lerp pressure and temperature toward outside.
        space.atmosphere.pressure_atm +=
            (outside.pressure_atm - space.atmosphere.pressure_atm) * rate;
        space.atmosphere.temperature_k +=
            (outside.temperature_k - space.atmosphere.temperature_k) * rate;
        space.atmosphere.humidity +=
            (outside.humidity - space.atmosphere.humidity) * rate;

        // Lerp gas composition toward outside.
        // Collect all gas names from both atmospheres.
        let mut all_gases: Vec<String> = space.atmosphere.composition.keys().cloned().collect();
        for gas in outside.composition.keys() {
            if !all_gases.contains(gas) {
                all_gases.push(gas.clone());
            }
        }

        for gas in all_gases {
            let inside_pct = space.atmosphere.gas_percent(&gas);
            let outside_pct = outside.gas_percent(&gas);
            let new_pct = inside_pct + (outside_pct - inside_pct) * rate;
            if new_pct > 0.001 {
                space.atmosphere.composition.insert(gas, new_pct);
            } else {
                space.atmosphere.composition.remove(&gas);
            }
        }
    }
}

impl System for AtmosphereSystem {
    fn name(&self) -> &str {
        "AtmosphereSystem"
    }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore) {
        self.elapsed += dt;
        if self.elapsed < self.tick_interval {
            return;
        }
        let step_dt = self.elapsed;
        self.elapsed = 0.0;

        // Evaluate the outside atmosphere.
        Self::evaluate_atmosphere(&mut self.outside_atmosphere);

        // Check if any ignition sources exist in the world.
        let ignition_exists = world.query_mut::<&IgnitionSource>().into_iter().next().is_some();

        // Phase 1: Update enclosed spaces.
        // Collect spaces to process (to avoid borrow conflicts).
        let space_entities: Vec<hecs::Entity> = world
            .query_mut::<&EnclosedSpace>()
            .into_iter()
            .map(|(e, _)| e)
            .collect();

        for space_entity in &space_entities {
            // Get the space, equalize, evaluate.
            if let Ok(mut space) = world.get::<&mut EnclosedSpace>(*space_entity) {
                Self::equalize(&mut space, &self.outside_atmosphere, step_dt);
                Self::evaluate_atmosphere(&mut space.atmosphere);
            }
        }

        // Phase 1b -- HOME LIFE SUPPORT (v0.618): occupancy drains O2 + raises CO2 in the home space;
        // POWERED scrubbers offset it. Cut the grid -> scrubbers shed -> O2 falls -> unbreathable ->
        // (the inside-homestead EnvironmentContext flips oxygenated off -> FoodSystem drains blood O2).
        let (mut o2_regen, mut co2_scrub) = (0.0f32, 0.0f32);
        for (_, (sc, power)) in world.query::<(&AirScrubber, Option<&PowerConsumer>)>().iter() {
            let powered = !sc.needs_power || power.map(|c| c.enabled).unwrap_or(false);
            if powered {
                o2_regen += sc.o2_regen_per_s;
                co2_scrub += sc.co2_scrub_per_s;
            }
        }
        let home_space = world.query::<(&HomeAir, &EnclosedSpace)>().iter().next().map(|(e, _)| e);
        if let Some(e) = home_space {
            if let Ok(mut sp) = world.get::<&mut EnclosedSpace>(e) {
                let o2 = sp.atmosphere.gas_percent("O2");
                let new_o2 = (o2 - HOME_OCCUPANTS * O2_DRAIN_PER_PERSON_PER_S * step_dt + o2_regen * step_dt).clamp(0.0, 21.0);
                sp.atmosphere.composition.insert("O2".to_string(), new_o2);
                let co2 = sp.atmosphere.gas_percent("CO2");
                let new_co2 = (co2 + HOME_OCCUPANTS * CO2_RISE_PER_PERSON_PER_S * step_dt - co2_scrub * step_dt).max(0.04);
                sp.atmosphere.composition.insert("CO2".to_string(), new_co2);
                Self::evaluate_atmosphere(&mut sp.atmosphere);
            }
        }

        // Phase 2: Apply atmospheric effects to entities.
        // Collect entities with health that are in enclosed spaces.
        let entities_in_spaces: Vec<(hecs::Entity, hecs::Entity)> = world
            .query_mut::<&InEnclosedSpace>()
            .into_iter()
            .map(|(e, ies)| (e, ies.space_entity))
            .collect();

        for (entity, space_entity) in &entities_in_spaces {
            // Read the atmosphere from the enclosed space.
            let atmo = match world.get::<&EnclosedSpace>(*space_entity) {
                Ok(space) => space.atmosphere.clone(),
                Err(_) => continue,
            };

            // Apply damage based on atmospheric conditions.
            let mut damage = 0.0_f32;

            // Vacuum / decompression.
            if atmo.pressure_atm < 0.1 {
                damage += DECOMPRESSION_DAMAGE * step_dt;
            }

            // Suffocation: O2 below damage threshold.
            let o2 = atmo.gas_percent("O2");
            if o2 < O2_DAMAGE_THRESHOLD {
                let severity = 1.0 - (o2 / O2_DAMAGE_THRESHOLD);
                damage += SUFFOCATION_DAMAGE_MAX * severity * step_dt;
            }

            // Toxic gas damage.
            if atmo.toxic {
                for (gas, &pct) in &atmo.composition {
                    if let Some(threshold) = toxic_threshold(gas) {
                        if pct > threshold {
                            let ratio = (pct / threshold).min(10.0);
                            damage += TOXIC_DAMAGE_MAX * (ratio / 10.0) * step_dt;
                        }
                    }
                }
            }

            // Explosion check: flammable atmosphere + ignition source.
            if atmo.flammable && ignition_exists {
                // Massive instantaneous damage from explosion.
                damage += 200.0;
                log::warn!("EXPLOSION in enclosed space {:?}!", space_entity);
            }

            // Apply damage.
            if damage > 0.0 {
                if let Ok(mut health) = world.get::<&mut Health>(*entity) {
                    health.current = (health.current - damage).max(0.0);
                }
            }
        }

        // Publish the HOME space's air to the GUI (v0.617), mirroring PowerStatus / WaterStatus. Clone
        // the atmosphere first so the immutable query borrow releases before we lock + write.
        if let Some(status) = data.get::<std::sync::Mutex<AirStatus>>("air_status") {
            let home_atmo = world
                .query::<(&HomeAir, &EnclosedSpace)>()
                .iter()
                .next()
                .map(|(_, (_, sp))| sp.atmosphere.clone());
            if let (Ok(mut s), Some(atmo)) = (status.lock(), home_atmo) {
                s.o2_pct = atmo.gas_percent("O2");
                s.co2_pct = atmo.gas_percent("CO2");
                s.pressure_atm = atmo.pressure_atm;
                s.temp_c = atmo.temperature_k - 273.15;
                s.breathable = atmo.breathable;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_atmosphere_is_breathable() {
        let mut atmo = Atmosphere::default();
        AtmosphereSystem::evaluate_atmosphere(&mut atmo);
        assert!(atmo.breathable);
        assert!(!atmo.toxic);
        assert!(!atmo.flammable);
    }

    #[test]
    fn test_vacuum_not_breathable() {
        let mut atmo = Atmosphere::vacuum();
        AtmosphereSystem::evaluate_atmosphere(&mut atmo);
        assert!(!atmo.breathable);
    }

    #[test]
    fn test_flammable_hydrogen() {
        let mut atmo = Atmosphere::default();
        atmo.composition.insert("H2".to_string(), 10.0); // between 4-75%
        AtmosphereSystem::evaluate_atmosphere(&mut atmo);
        assert!(atmo.flammable);
    }

    #[test]
    fn test_toxic_co() {
        let mut atmo = Atmosphere::default();
        atmo.composition.insert("CO".to_string(), 0.01); // above 0.005% threshold
        AtmosphereSystem::evaluate_atmosphere(&mut atmo);
        assert!(atmo.toxic);
    }

    #[test]
    fn test_low_oxygen_not_breathable() {
        let mut atmo = Atmosphere::default();
        atmo.composition.insert("O2".to_string(), 12.0);
        AtmosphereSystem::evaluate_atmosphere(&mut atmo);
        assert!(!atmo.breathable);
    }

    /// v0.617: the registered AtmosphereSystem publishes the HOME space's air to `air_status` for the
    /// Live air card. An Earth-like sealed home reads ~21% O2 and breathable.
    #[test]
    fn publishes_home_air_status() {
        use crate::ecs::systems::System;
        use crate::hot_reload::data_store::DataStore;
        let mut data = DataStore::new();
        data.insert("air_status", std::sync::Mutex::new(AirStatus::default()));
        let mut world = hecs::World::new();
        world.spawn((HomeAir, EnclosedSpace::new_sealed(14_000.0)));
        let mut sys = AtmosphereSystem::new();
        sys.tick(&mut world, 1.0, &data); // dt >= tick_interval so the step runs
        let s = *data.get::<std::sync::Mutex<AirStatus>>("air_status").unwrap().lock().unwrap();
        assert!((s.o2_pct - 21.0).abs() < 1.0, "home O2 ~21%, got {}", s.o2_pct);
        assert!((s.pressure_atm - 1.0).abs() < 0.01, "1 atm, got {}", s.pressure_atm);
        assert!(s.breathable, "Earth-like home air is breathable");
    }

    /// v0.618: a POWERED air scrubber holds the home's O2 against occupancy; cut its power and occupancy
    /// drains it -- the power -> air consequence.
    #[test]
    fn powered_scrubber_holds_air_then_power_loss_drains_it() {
        use crate::ecs::components::PowerConsumer;
        use crate::ecs::systems::System;
        use crate::hot_reload::data_store::DataStore;
        let mut data = DataStore::new();
        data.insert("air_status", std::sync::Mutex::new(AirStatus::default()));
        let mut world = hecs::World::new();
        world.spawn((HomeAir, EnclosedSpace::new_sealed(14_000.0)));
        let scrubber = world.spawn((
            AirScrubber { o2_regen_per_s: 0.02, co2_scrub_per_s: 0.006, needs_power: true },
            PowerConsumer { draw_watts: 25.0, priority: 1, enabled: true },
        ));
        let mut sys = AtmosphereSystem::new();
        let o2 = |d: &DataStore| d.get::<std::sync::Mutex<AirStatus>>("air_status").unwrap().lock().unwrap().o2_pct;

        // Powered: regen >= occupancy drain, so O2 stays ~21%.
        for _ in 0..5 { sys.tick(&mut world, 1.0, &data); }
        let powered = o2(&data);
        assert!(powered > 20.9, "powered scrubber holds O2, got {powered}");

        // Cut power: occupancy keeps consuming with no regen -> O2 falls.
        world.get::<&mut PowerConsumer>(scrubber).unwrap().enabled = false;
        for _ in 0..30 { sys.tick(&mut world, 1.0, &data); }
        let cut = o2(&data);
        assert!(cut < powered - 0.2, "power loss drains the home air ({powered} -> {cut})");
    }

    #[test]
    fn test_equalization() {
        let mut space = EnclosedSpace::new_unsealed(100.0);
        space.atmosphere.pressure_atm = 2.0;
        let outside = Atmosphere::default();

        AtmosphereSystem::equalize(&mut space, &outside, 10.0);

        // Pressure should move toward 1.0.
        assert!(space.atmosphere.pressure_atm < 2.0);
        assert!(space.atmosphere.pressure_atm > 1.0);
    }

    #[test]
    fn test_sealed_room_no_leak() {
        let mut space = EnclosedSpace::new_sealed(100.0);
        space.atmosphere.pressure_atm = 2.0;
        let outside = Atmosphere::default();

        AtmosphereSystem::equalize(&mut space, &outside, 10.0);

        // Perfectly sealed: pressure should not change.
        assert!((space.atmosphere.pressure_atm - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_system_ticks_without_panic() {
        let mut system = AtmosphereSystem::new();
        let mut world = hecs::World::new();
        let data = DataStore::new();

        for _ in 0..100 {
            system.tick(&mut world, 0.05, &data);
        }
    }
}
