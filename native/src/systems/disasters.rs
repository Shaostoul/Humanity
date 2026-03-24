//! Disaster system -- natural and artificial catastrophic events.
//!
//! Spawns disasters based on conditions and location, applies damage in an
//! area of effect, and triggers chain reactions (earthquake -> tsunami,
//! volcano -> ash cloud + lava, meteor -> shockwave + crater).

use glam::Vec3;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

use crate::ecs::components::{Health, Transform};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

// ── Disaster types ─────────────────────────────────────────

/// All disaster categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DisasterType {
    // Weather
    Lightning,
    Tornado,
    Hurricane,
    Blizzard,
    Heatwave,
    Flood,
    // Geological
    Earthquake,
    Volcano,
    Landslide,
    Tsunami,
    Sinkhole,
    // Cosmic
    SolarFlare,
    MeteorImpact,
    AsteroidImpact,
    GammaRayBurst,
    // Artificial
    NuclearExplosion,
    ChemicalSpill,
    ReactorMeltdown,
    // Extreme
    SuperVolcano,
    PlanetaryCollision,
    BlackHoleApproach,
}

/// What kind of damage a disaster inflicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamageType {
    Heat,
    Force,
    Radiation,
    Chemical,
    Cold,
    Electric,
    Gravitational,
}

/// An active disaster event in the world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveDisaster {
    /// Type of disaster.
    pub disaster_type: DisasterType,
    /// World-space center of the disaster.
    pub position: [f32; 3],
    /// Radius of effect in meters.
    pub radius: f32,
    /// Intensity (0.0-1.0, affects damage and visual strength).
    pub intensity: f32,
    /// Seconds remaining before the disaster ends.
    pub duration: f32,
    /// Damage applied per second to entities in the radius.
    pub damage_per_second: f32,
    /// Type of damage inflicted.
    pub damage_type: DamageType,
    /// Visual effect identifier (for the renderer to look up).
    pub vfx_id: String,
}

// ── Disaster parameter defaults ────────────────────────────

impl DisasterType {
    /// Default parameters for each disaster type.
    fn defaults(self) -> (f32, f32, f32, DamageType, &'static str) {
        // (radius, duration, dps, damage_type, vfx_id)
        match self {
            // Weather
            DisasterType::Lightning => (5.0, 0.5, 500.0, DamageType::Electric, "vfx_lightning"),
            DisasterType::Tornado => (200.0, 120.0, 30.0, DamageType::Force, "vfx_tornado"),
            DisasterType::Hurricane => (5000.0, 600.0, 10.0, DamageType::Force, "vfx_hurricane"),
            DisasterType::Blizzard => (3000.0, 300.0, 5.0, DamageType::Cold, "vfx_blizzard"),
            DisasterType::Heatwave => (10000.0, 600.0, 2.0, DamageType::Heat, "vfx_heatwave"),
            DisasterType::Flood => (1000.0, 300.0, 8.0, DamageType::Force, "vfx_flood"),
            // Geological
            DisasterType::Earthquake => (5000.0, 30.0, 15.0, DamageType::Force, "vfx_earthquake"),
            DisasterType::Volcano => (3000.0, 600.0, 40.0, DamageType::Heat, "vfx_volcano"),
            DisasterType::Landslide => (500.0, 15.0, 50.0, DamageType::Force, "vfx_landslide"),
            DisasterType::Tsunami => (8000.0, 60.0, 35.0, DamageType::Force, "vfx_tsunami"),
            DisasterType::Sinkhole => (100.0, 10.0, 200.0, DamageType::Force, "vfx_sinkhole"),
            // Cosmic
            DisasterType::SolarFlare => (100000.0, 120.0, 20.0, DamageType::Radiation, "vfx_solar_flare"),
            DisasterType::MeteorImpact => (500.0, 5.0, 300.0, DamageType::Force, "vfx_meteor"),
            DisasterType::AsteroidImpact => (50000.0, 30.0, 500.0, DamageType::Force, "vfx_asteroid"),
            DisasterType::GammaRayBurst => (1000000.0, 10.0, 1000.0, DamageType::Radiation, "vfx_grb"),
            // Artificial
            DisasterType::NuclearExplosion => (15000.0, 30.0, 200.0, DamageType::Radiation, "vfx_nuke"),
            DisasterType::ChemicalSpill => (500.0, 300.0, 10.0, DamageType::Chemical, "vfx_chemical"),
            DisasterType::ReactorMeltdown => (5000.0, 600.0, 50.0, DamageType::Radiation, "vfx_meltdown"),
            // Extreme
            DisasterType::SuperVolcano => (100000.0, 1800.0, 80.0, DamageType::Heat, "vfx_supervolcano"),
            DisasterType::PlanetaryCollision => (1e7, 60.0, 10000.0, DamageType::Force, "vfx_collision"),
            DisasterType::BlackHoleApproach => (1e8, 300.0, 0.0, DamageType::Gravitational, "vfx_blackhole"),
        }
    }

    /// Chain reactions triggered when this disaster occurs.
    fn chain_reactions(self) -> &'static [DisasterType] {
        match self {
            DisasterType::Earthquake => &[DisasterType::Tsunami, DisasterType::Landslide],
            DisasterType::Volcano => &[], // handled specially (lava + ash)
            DisasterType::MeteorImpact => &[DisasterType::Earthquake],
            DisasterType::AsteroidImpact => &[DisasterType::Earthquake, DisasterType::Tsunami],
            DisasterType::NuclearExplosion => &[DisasterType::Earthquake],
            DisasterType::SuperVolcano => &[DisasterType::Earthquake],
            DisasterType::PlanetaryCollision => &[DisasterType::Earthquake],
            _ => &[],
        }
    }
}

// ── System ─────────────────────────────────────────────────

/// Manages active disasters, applies damage, and triggers chain reactions.
pub struct DisasterSystem {
    /// How often to check for new random disasters (seconds).
    tick_interval: f32,
    /// Accumulated time since last check.
    elapsed: f32,
    /// Currently active disasters.
    active_disasters: Vec<ActiveDisaster>,
    /// Random number generator.
    rng: StdRng,
    /// Pending chain-reaction disasters to spawn next tick.
    pending_chains: Vec<ActiveDisaster>,
}

impl DisasterSystem {
    pub fn new() -> Self {
        Self {
            tick_interval: 5.0,
            elapsed: 0.0,
            active_disasters: Vec::new(),
            rng: StdRng::from_entropy(),
            pending_chains: Vec::new(),
        }
    }

    /// Manually spawn a disaster at a specific location.
    pub fn spawn_disaster(
        &mut self,
        disaster_type: DisasterType,
        position: Vec3,
        intensity: f32,
    ) {
        let intensity = intensity.clamp(0.0, 1.0);
        let (radius, duration, dps, damage_type, vfx_id) = disaster_type.defaults();

        let disaster = ActiveDisaster {
            disaster_type,
            position: position.to_array(),
            radius: radius * intensity,
            intensity,
            duration,
            damage_per_second: dps * intensity,
            damage_type,
            vfx_id: vfx_id.to_string(),
        };

        log::info!(
            "DISASTER: {:?} at ({:.0}, {:.0}, {:.0}), intensity {:.2}, radius {:.0}m",
            disaster_type,
            position.x, position.y, position.z,
            intensity, disaster.radius,
        );

        // Queue chain reactions.
        for &chain_type in disaster_type.chain_reactions() {
            let (c_radius, c_duration, c_dps, c_dmg_type, c_vfx) = chain_type.defaults();
            let chain_intensity = (intensity * 0.6).clamp(0.1, 1.0);
            // Offset position slightly for chain reactions.
            let offset = Vec3::new(
                self.rng.gen_range(-500.0..500.0),
                0.0,
                self.rng.gen_range(-500.0..500.0),
            );
            let chain_pos = position + offset;

            self.pending_chains.push(ActiveDisaster {
                disaster_type: chain_type,
                position: chain_pos.to_array(),
                radius: c_radius * chain_intensity,
                intensity: chain_intensity,
                duration: c_duration,
                damage_per_second: c_dps * chain_intensity,
                damage_type: c_dmg_type,
                vfx_id: c_vfx.to_string(),
            });

            log::info!(
                "  -> Chain reaction: {:?} at ({:.0}, {:.0}, {:.0})",
                chain_type,
                chain_pos.x, chain_pos.y, chain_pos.z,
            );
        }

        self.active_disasters.push(disaster);
    }

    /// Get all currently active disasters.
    pub fn active_disasters(&self) -> &[ActiveDisaster] {
        &self.active_disasters
    }

    /// Apply damage from all active disasters to nearby entities.
    fn apply_damage(&self, world: &mut hecs::World, dt: f32) {
        if self.active_disasters.is_empty() {
            return;
        }

        // Collect entities with position and health.
        let entities: Vec<(hecs::Entity, Vec3)> = world
            .query_mut::<&Transform>()
            .into_iter()
            .map(|(e, t)| (e, t.position))
            .collect();

        for (entity, pos) in &entities {
            let mut total_damage = 0.0_f32;

            for disaster in &self.active_disasters {
                let center = Vec3::from_array(disaster.position);
                let dist = pos.distance(center);

                if dist > disaster.radius {
                    continue;
                }

                // Damage falls off linearly with distance.
                let falloff = 1.0 - (dist / disaster.radius);
                let mut dmg = disaster.damage_per_second * falloff * dt;

                // Special: Black hole gravitational damage scales with inverse square.
                if disaster.disaster_type == DisasterType::BlackHoleApproach {
                    let event_horizon = 100.0; // meters
                    if dist < event_horizon {
                        // Spaghettification: instant death.
                        dmg = 100000.0;
                        log::warn!("Entity {:?} crossed event horizon — spaghettification!", entity);
                    } else {
                        // Tidal forces: inverse square.
                        let tidal = (event_horizon / dist).powi(2) * 100.0;
                        dmg = tidal * dt;
                    }
                }

                // Special: Nuclear radiation fallout persists.
                if disaster.disaster_type == DisasterType::NuclearExplosion {
                    // EMP effect logged (electronics disruption would be handled by another system).
                    if dist < disaster.radius * 0.5 {
                        dmg *= 2.0; // inner blast zone gets double damage.
                    }
                }

                // Special: Solar flare — radiation only damages unshielded entities.
                // (Shield check would go here; for now all entities take damage.)
                if disaster.disaster_type == DisasterType::SolarFlare {
                    dmg *= 0.5; // atmosphere provides some shielding.
                }

                total_damage += dmg;
            }

            if total_damage > 0.0 {
                if let Ok(mut health) = world.get::<&mut Health>(*entity) {
                    health.current = (health.current - total_damage).max(0.0);
                }
            }
        }
    }
}

impl System for DisasterSystem {
    fn name(&self) -> &str {
        "DisasterSystem"
    }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, _data: &DataStore) {
        // Drain pending chain reactions into active list.
        self.active_disasters.append(&mut self.pending_chains);

        // Update durations and remove expired disasters.
        self.active_disasters.retain_mut(|d| {
            d.duration -= dt;
            if d.duration <= 0.0 {
                log::info!("DISASTER ended: {:?}", d.disaster_type);
                false
            } else {
                true
            }
        });

        // Apply damage from active disasters.
        self.apply_damage(world, dt);

        // Periodic random disaster check (placeholder — real implementation would
        // check terrain features, fault lines, weather conditions, etc.).
        self.elapsed += dt;
        if self.elapsed >= self.tick_interval {
            self.elapsed = 0.0;
            // Random disaster generation is intentionally NOT automatic.
            // Disasters should be triggered by game events, terrain conditions,
            // or explicit spawn calls. This tick is reserved for future condition
            // checks (e.g., "if near volcano AND seismic activity > threshold").
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hot_reload::data_store::DataStore;

    #[test]
    fn test_disaster_defaults() {
        let (radius, duration, dps, _, _) = DisasterType::Lightning.defaults();
        assert!(radius > 0.0);
        assert!(duration > 0.0);
        assert!(dps > 0.0);
    }

    #[test]
    fn test_spawn_disaster() {
        let mut system = DisasterSystem::new();
        system.spawn_disaster(DisasterType::Earthquake, Vec3::new(100.0, 0.0, 200.0), 0.7);

        assert_eq!(system.active_disasters().len(), 1);
        assert_eq!(system.active_disasters()[0].disaster_type, DisasterType::Earthquake);

        // Earthquake should trigger chain reactions (tsunami + landslide).
        assert_eq!(system.pending_chains.len(), 2);
    }

    #[test]
    fn test_disaster_expires() {
        let mut system = DisasterSystem::new();
        system.spawn_disaster(DisasterType::Lightning, Vec3::ZERO, 1.0);

        let mut world = hecs::World::new();
        let data = DataStore::new();

        // Lightning has 0.5s duration. Tick past it.
        system.tick(&mut world, 1.0, &data);

        // Lightning should be gone, but chain reactions (none for lightning) are empty.
        // The disaster had 0.5s duration minus 1.0s dt = expired.
        let lightning_count = system
            .active_disasters()
            .iter()
            .filter(|d| d.disaster_type == DisasterType::Lightning)
            .count();
        assert_eq!(lightning_count, 0);
    }

    #[test]
    fn test_damage_applied_to_entity() {
        let mut system = DisasterSystem::new();
        system.spawn_disaster(DisasterType::Heatwave, Vec3::ZERO, 1.0);

        let mut world = hecs::World::new();
        let data = DataStore::new();

        // Spawn an entity at the disaster center.
        let entity = world.spawn((
            Transform {
                position: Vec3::ZERO,
                ..Default::default()
            },
            Health {
                current: 100.0,
                max: 100.0,
            },
        ));

        // Tick the system.
        system.tick(&mut world, 1.0, &data);

        let health = world.get::<&Health>(entity).unwrap();
        assert!(health.current < 100.0, "Entity should take damage from heatwave");
    }

    #[test]
    fn test_no_damage_outside_radius() {
        let mut system = DisasterSystem::new();
        system.spawn_disaster(DisasterType::Lightning, Vec3::ZERO, 1.0);

        let mut world = hecs::World::new();
        let data = DataStore::new();

        // Spawn entity far away from the disaster.
        let entity = world.spawn((
            Transform {
                position: Vec3::new(100000.0, 0.0, 0.0),
                ..Default::default()
            },
            Health {
                current: 100.0,
                max: 100.0,
            },
        ));

        system.tick(&mut world, 0.1, &data);

        let health = world.get::<&Health>(entity).unwrap();
        assert!(
            (health.current - 100.0).abs() < f32::EPSILON,
            "Entity outside radius should take no damage"
        );
    }

    #[test]
    fn test_chain_reactions() {
        let mut system = DisasterSystem::new();
        system.spawn_disaster(DisasterType::AsteroidImpact, Vec3::ZERO, 0.8);

        // Asteroid should chain into Earthquake + Tsunami.
        assert_eq!(system.pending_chains.len(), 2);

        let chain_types: Vec<DisasterType> = system.pending_chains.iter().map(|d| d.disaster_type).collect();
        assert!(chain_types.contains(&DisasterType::Earthquake));
        assert!(chain_types.contains(&DisasterType::Tsunami));
    }

    #[test]
    fn test_system_ticks_without_panic() {
        let mut system = DisasterSystem::new();
        let mut world = hecs::World::new();
        let data = DataStore::new();

        for _ in 0..200 {
            system.tick(&mut world, 0.05, &data);
        }
    }
}
