//! AI system — behavior state machine for NPCs and creatures.
//!
//! Queries entities with `AIBehavior + Transform + Health` and runs a simple
//! state machine driven by `behavior_type`. No pathfinding yet — movement is
//! direct toward/away from targets.
//!
//! Registered live in v0.761 (combat arc increment 2). Predators now count
//! the PLAYER as prey, the `attacking` state lands real bites through the
//! `damage_events` channel (CombatSystem resolves mitigation/death/loot),
//! and the system integrates its own movement (position += velocity * dt)
//! so hostiles actually close distance.
//!
//! Behavior tree node types live in `behavior.rs` (data-driven RON format).
//! Flow field pathfinding in `flow_field.rs` (future integration).
//! Off-screen autonomy in `autonomy.rs`.

pub mod behavior;
pub mod flow_field;

use glam::Vec3;
use rand::Rng;

use crate::ecs::components::{AIBehavior, Controllable, Faction, Health, Transform, Velocity};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// How close an entity must be to a target to count as "in range" (meters).
const ATTACK_RANGE: f32 = 5.0;
/// Detection radius for finding targets (meters).
const DETECT_RANGE: f32 = 30.0;
/// Wander radius from the entity's idle origin (meters).
const WANDER_RADIUS: f32 = 10.0;
/// Health fraction below which passive/herd entities flee.
const FLEE_THRESHOLD: f32 = 0.3;
/// Herd cohesion radius — how close herd members try to stay to each other.
const HERD_RADIUS: f32 = 15.0;
/// Movement speed multiplier for normal movement (units/s).
const MOVE_SPEED: f32 = 4.0;
/// Movement speed multiplier for fleeing (units/s).
const FLEE_SPEED: f32 = 6.0;
/// Rest duration after a predator makes a kill (seconds).
const REST_DURATION: f32 = 5.0;
/// Seconds between bites while in the attacking state. (v0.761)
const BITE_COOLDOWN_S: f32 = 1.5;
/// Bite damage = max health * this (a 65 hp wolf bites for ~10), capped.
const BITE_DAMAGE_FRACTION: f32 = 0.15;
const BITE_DAMAGE_CAP: f32 = 25.0;

/// Behavior state machine runner for all AI-controlled entities.
pub struct AISystem {
    /// Per-entity timers keyed by entity id bits. Used for wander cooldowns,
    /// rest timers, etc. Cleaned up lazily when entities despawn.
    timers: std::collections::HashMap<u64, f32>,
    /// Per-entity wander targets.
    wander_targets: std::collections::HashMap<u64, Vec3>,
    /// Per-entity seconds until the next bite lands. (v0.761)
    bite_timers: std::collections::HashMap<u64, f32>,
}

impl AISystem {
    pub fn new() -> Self {
        Self {
            timers: std::collections::HashMap::new(),
            wander_targets: std::collections::HashMap::new(),
            bite_timers: std::collections::HashMap::new(),
        }
    }
}

impl System for AISystem {
    fn name(&self) -> &str {
        "AISystem"
    }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore) {
        // ── 1. Snapshot all positions, factions, health, and AI state ────
        // We need to read other entities' data while deciding what to do,
        // so collect everything up front to avoid borrow conflicts.
        let snapshots: Vec<EntitySnapshot> = world
            .query::<(
                &Transform,
                &Health,
                Option<&Faction>,
                Option<&AIBehavior>,
                Option<&Controllable>,
                Option<&crate::ecs::components::Name>,
            )>()
            .iter()
            .map(|(entity, (tf, hp, faction, ai, player, name))| EntitySnapshot {
                id: entity.to_bits().into(),
                position: tf.position,
                health_frac: if hp.max > 0.0 { hp.current / hp.max } else { 0.0 },
                max_health: hp.max,
                faction: faction.map(|f| f.id.clone()),
                behavior_type: ai.map(|a| a.behavior_type.clone()),
                state: ai.map(|a| a.state.clone()),
                alive: hp.current > 0.0,
                is_player: player.is_some(),
                name: name.map(|n| n.0.clone()),
            })
            .collect();

        // ── 2. Decide new state + velocity for each AI entity ───────────
        let mut decisions: Vec<(u64, String, Option<u64>, Vec3)> = Vec::new();

        for snap in &snapshots {
            let Some(ref btype) = snap.behavior_type else { continue };
            if !snap.alive { continue; }

            let (new_state, target, desired_vel) = match btype.as_str() {
                "passive" => self.tick_passive(snap, &snapshots, dt),
                "aggressive" => self.tick_aggressive(snap, &snapshots, dt),
                "herd" => self.tick_herd(snap, &snapshots, dt),
                "predator" => self.tick_predator(snap, &snapshots, dt),
                "guard" => self.tick_guard(snap, &snapshots, dt),
                _ => ("idle".to_string(), None, Vec3::ZERO),
            };

            decisions.push((snap.id, new_state, target, desired_vel));
        }

        // ── 3. Apply decisions back to components ───────────────────────
        // Bite bookkeeping: cooldowns tick down for everyone. (v0.761)
        for t in self.bite_timers.values_mut() {
            *t -= dt;
        }
        self.bite_timers.retain(|_, t| *t > 0.0);

        for (id, new_state, target, desired_vel) in decisions {
            let entity = hecs::Entity::from_bits(id).expect("valid entity bits");

            let attacking = new_state == "attacking";
            if let Ok(mut ai) = world.get::<&mut AIBehavior>(entity) {
                ai.state = new_state;
                ai.target = target;
            }

            if let Ok(mut vel) = world.get::<&mut Velocity>(entity) {
                vel.linear = desired_vel;
            }

            // Integrate movement + face travel direction: nothing else moves
            // AI entities, so the system owns its own locomotion. (v0.761)
            if desired_vel.length_squared() > 0.001 {
                if let Ok(mut tf) = world.get::<&mut Transform>(entity) {
                    tf.position += desired_vel * dt;
                    tf.rotation =
                        glam::Quat::from_rotation_y(f32::atan2(desired_vel.x, desired_vel.z));
                }
            }

            // A landed attack BITES through the one damage pipeline. (v0.761)
            if attacking {
                if let Some(target_id) = target {
                    let cooling = self.bite_timers.contains_key(&id);
                    if !cooling {
                        self.bite_timers.insert(id, BITE_COOLDOWN_S);
                        let (damage, my_name) = snapshots
                            .iter()
                            .find(|s| s.id == id)
                            .map(|s| {
                                (
                                    (s.max_health * BITE_DAMAGE_FRACTION).min(BITE_DAMAGE_CAP),
                                    s.name.clone(),
                                )
                            })
                            .unwrap_or((5.0, None));
                        if let Some(chan) = data.get::<std::sync::Mutex<
                            Vec<(u64, crate::systems::combat::damage::DamageEvent)>,
                        >>("damage_events")
                        {
                            if let Ok(mut q) = chan.lock() {
                                q.push((
                                    target_id,
                                    crate::systems::combat::damage::DamageEvent {
                                        damage_type:
                                            crate::systems::combat::damage::DamageType::Kinetic,
                                        amount: damage,
                                        source_name: my_name,
                                        source_is_player: false,
                                    },
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Internal helpers ────────────────────────────────────────────────────

/// Read-only snapshot of an entity used for target selection.
struct EntitySnapshot {
    id: u64,
    position: Vec3,
    health_frac: f32,
    max_health: f32,
    faction: Option<String>,
    behavior_type: Option<String>,
    state: Option<String>,
    alive: bool,
    /// The player (Controllable) - predators count them as prey. (v0.761)
    is_player: bool,
    name: Option<String>,
}

impl AISystem {
    // ── Passive: wander, flee when hurt ──────────────────────────────

    fn tick_passive(
        &mut self,
        me: &EntitySnapshot,
        others: &[EntitySnapshot],
        dt: f32,
    ) -> (String, Option<u64>, Vec3) {
        // Flee if health is low and an aggressor is nearby
        if me.health_frac < FLEE_THRESHOLD {
            if let Some(threat) = nearest_hostile(me, others, DETECT_RANGE) {
                let away = flee_direction(me.position, threat.position);
                return ("fleeing".to_string(), Some(threat.id), away * FLEE_SPEED);
            }
        }

        // Wander randomly
        self.wander(me, dt)
    }

    // ── Aggressive: patrol, attack different-faction entities ────────

    fn tick_aggressive(
        &mut self,
        me: &EntitySnapshot,
        others: &[EntitySnapshot],
        dt: f32,
    ) -> (String, Option<u64>, Vec3) {
        // Look for nearest different-faction entity
        if let Some(target) = nearest_different_faction(me, others, DETECT_RANGE) {
            let dist = me.position.distance(target.position);
            if dist <= ATTACK_RANGE {
                // Close enough — attack (face target, zero velocity)
                return ("attacking".to_string(), Some(target.id), Vec3::ZERO);
            }
            // Move toward target
            let dir = (target.position - me.position).normalize_or_zero();
            return ("patrolling".to_string(), Some(target.id), dir * MOVE_SPEED);
        }

        // No targets — wander/patrol
        self.wander(me, dt)
    }

    // ── Herd: stay near same-type entities, flee from aggressors ────

    fn tick_herd(
        &mut self,
        me: &EntitySnapshot,
        others: &[EntitySnapshot],
        dt: f32,
    ) -> (String, Option<u64>, Vec3) {
        // Flee from nearby aggressive entities
        if let Some(threat) = nearest_with_behavior(me, others, "aggressive", DETECT_RANGE) {
            let away = flee_direction(me.position, threat.position);
            return ("fleeing".to_string(), Some(threat.id), away * FLEE_SPEED);
        }
        if let Some(threat) = nearest_with_behavior(me, others, "predator", DETECT_RANGE) {
            let away = flee_direction(me.position, threat.position);
            return ("fleeing".to_string(), Some(threat.id), away * FLEE_SPEED);
        }

        // Move toward herd center if too far
        let herd_center = compute_herd_center(me, others);
        if let Some(center) = herd_center {
            let to_center = center - me.position;
            if to_center.length() > HERD_RADIUS {
                let dir = to_center.normalize_or_zero();
                return ("wandering".to_string(), None, dir * MOVE_SPEED * 0.6);
            }
        }

        // Gentle wander
        self.wander(me, dt)
    }

    // ── Predator: hunt passive/herd, rest after kill ────────────────

    fn tick_predator(
        &mut self,
        me: &EntitySnapshot,
        others: &[EntitySnapshot],
        dt: f32,
    ) -> (String, Option<u64>, Vec3) {
        let timer = self.timers.entry(me.id).or_insert(0.0);

        // If resting, count down
        if me.state.as_deref() == Some("resting") {
            *timer -= dt;
            if *timer > 0.0 {
                return ("resting".to_string(), None, Vec3::ZERO);
            }
            // Done resting
        }

        // Hunt the nearest prey: passive/herd animals or the PLAYER (v0.761).
        let prey = others
            .iter()
            .filter(|o| {
                o.id != me.id
                    && o.alive
                    && (o.is_player
                        || matches!(o.behavior_type.as_deref(), Some("passive") | Some("herd")))
            })
            .min_by(|a, b| {
                let da = me.position.distance_squared(a.position);
                let db = me.position.distance_squared(b.position);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            });

        if let Some(target) = prey {
            let dist = me.position.distance(target.position);
            if dist > DETECT_RANGE {
                // Too far, idle
                return self.wander(me, dt);
            }
            if dist <= ATTACK_RANGE {
                // Check if prey is dead — if so, rest
                if !target.alive {
                    *timer = REST_DURATION;
                    return ("resting".to_string(), None, Vec3::ZERO);
                }
                return ("attacking".to_string(), Some(target.id), Vec3::ZERO);
            }
            // Chase
            let dir = (target.position - me.position).normalize_or_zero();
            return ("hunting".to_string(), Some(target.id), dir * MOVE_SPEED * 1.3);
        }

        self.wander(me, dt)
    }

    // ── Guard: stand ground, attack hostile faction in range ─────────

    fn tick_guard(
        &mut self,
        me: &EntitySnapshot,
        others: &[EntitySnapshot],
        _dt: f32,
    ) -> (String, Option<u64>, Vec3) {
        // Attack nearest hostile-faction entity within detect range
        if let Some(target) = nearest_different_faction(me, others, DETECT_RANGE) {
            let dist = me.position.distance(target.position);
            if dist <= ATTACK_RANGE {
                return ("attacking".to_string(), Some(target.id), Vec3::ZERO);
            }
            // Move toward threat but stay close to guard position
            let dir = (target.position - me.position).normalize_or_zero();
            return ("patrolling".to_string(), Some(target.id), dir * MOVE_SPEED * 0.8);
        }

        // Stand ground
        ("idle".to_string(), None, Vec3::ZERO)
    }

    // ── Shared wander logic ─────────────────────────────────────────

    fn wander(
        &mut self,
        me: &EntitySnapshot,
        dt: f32,
    ) -> (String, Option<u64>, Vec3) {
        let timer = self.timers.entry(me.id).or_insert(0.0);
        *timer -= dt;

        if *timer <= 0.0 {
            // Pick a new wander target
            let mut rng = rand::thread_rng();
            let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
            let dist: f32 = rng.gen_range(2.0..WANDER_RADIUS);
            let target_pos = me.position + Vec3::new(angle.cos() * dist, 0.0, angle.sin() * dist);
            self.wander_targets.insert(me.id, target_pos);
            *timer = rng.gen_range(3.0..8.0); // wander for 3-8 seconds
        }

        if let Some(&target_pos) = self.wander_targets.get(&me.id) {
            let to_target = target_pos - me.position;
            if to_target.length() > 0.5 {
                let dir = to_target.normalize_or_zero();
                return ("wandering".to_string(), None, dir * MOVE_SPEED * 0.4);
            }
        }

        ("idle".to_string(), None, Vec3::ZERO)
    }
}

// ── Free functions for target selection ─────────────────────────────────

/// Find nearest entity with a different faction (and alive).
fn nearest_different_faction<'a>(
    me: &EntitySnapshot,
    others: &'a [EntitySnapshot],
    range: f32,
) -> Option<&'a EntitySnapshot> {
    let my_faction = me.faction.as_deref()?;
    let range_sq = range * range;

    others
        .iter()
        .filter(|o| {
            o.id != me.id
                && o.alive
                && o.faction.as_deref().map_or(false, |f| f != my_faction)
                && me.position.distance_squared(o.position) <= range_sq
        })
        .min_by(|a, b| {
            let da = me.position.distance_squared(a.position);
            let db = me.position.distance_squared(b.position);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
}

/// Find nearest entity that could be a threat (aggressive/predator with different faction or no faction).
fn nearest_hostile<'a>(
    me: &EntitySnapshot,
    others: &'a [EntitySnapshot],
    range: f32,
) -> Option<&'a EntitySnapshot> {
    let range_sq = range * range;

    others
        .iter()
        .filter(|o| {
            o.id != me.id
                && o.alive
                && matches!(
                    o.behavior_type.as_deref(),
                    Some("aggressive") | Some("predator")
                )
                && me.position.distance_squared(o.position) <= range_sq
        })
        .min_by(|a, b| {
            let da = me.position.distance_squared(a.position);
            let db = me.position.distance_squared(b.position);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
}

/// Find nearest entity with a specific behavior_type.
fn nearest_with_behavior<'a>(
    me: &EntitySnapshot,
    others: &'a [EntitySnapshot],
    behavior: &str,
    range: f32,
) -> Option<&'a EntitySnapshot> {
    let range_sq = range * range;

    others
        .iter()
        .filter(|o| {
            o.id != me.id
                && o.alive
                && o.behavior_type.as_deref() == Some(behavior)
                && me.position.distance_squared(o.position) <= range_sq
        })
        .min_by(|a, b| {
            let da = me.position.distance_squared(a.position);
            let db = me.position.distance_squared(b.position);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
}

/// Compute average position of all same-behavior_type entities (herd center).
fn compute_herd_center(me: &EntitySnapshot, others: &[EntitySnapshot]) -> Option<Vec3> {
    let my_btype = me.behavior_type.as_deref()?;
    let mut sum = Vec3::ZERO;
    let mut count = 0u32;

    for o in others {
        if o.id != me.id && o.alive && o.behavior_type.as_deref() == Some(my_btype) {
            sum += o.position;
            count += 1;
        }
    }

    if count > 0 {
        Some(sum / count as f32)
    } else {
        None
    }
}

/// Direction vector pointing away from a threat.
fn flee_direction(my_pos: Vec3, threat_pos: Vec3) -> Vec3 {
    (my_pos - threat_pos).normalize_or_zero()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::Name;

    /// THE hostile loop (v0.761): a predator wolf sees the player as prey,
    /// closes the distance under its own locomotion, and once in range its
    /// bites land on the damage_events channel with its name attached and
    /// respect the bite cooldown.
    #[test]
    fn predator_hunts_the_player_and_bites() {
        let mut world = hecs::World::new();
        let mut data = DataStore::new();
        data.insert(
            "damage_events",
            std::sync::Mutex::new(
                Vec::<(u64, crate::systems::combat::damage::DamageEvent)>::new(),
            ),
        );

        let player = world.spawn((
            Controllable,
            Transform {
                position: Vec3::ZERO,
                ..Default::default()
            },
            Health { current: 100.0, max: 100.0 },
        ));
        let wolf = world.spawn((
            Name("Wolf".to_string()),
            AIBehavior {
                behavior_type: "predator".to_string(),
                state: "idle".to_string(),
                target: None,
            },
            Transform {
                position: Vec3::new(15.0, 0.0, 0.0),
                ..Default::default()
            },
            Health { current: 65.0, max: 65.0 },
            Velocity::default(),
        ));

        let mut sys = AISystem::new();
        // First tick: the wolf spots the player and starts hunting.
        sys.tick(&mut world, 0.1, &data);
        {
            let ai = world.get::<&AIBehavior>(wolf).unwrap();
            assert_eq!(ai.state, "hunting", "wolf hunts the player");
            assert_eq!(ai.target, Some(player.to_bits().into()));
        }
        let d0 = world.get::<&Transform>(wolf).unwrap().position.length();
        assert!(d0 < 15.0, "the wolf moved closer under its own locomotion");

        // Run until it closes to attack range and bites.
        for _ in 0..600 {
            sys.tick(&mut world, 0.1, &data);
        }
        {
            let ai = world.get::<&AIBehavior>(wolf).unwrap();
            assert_eq!(ai.state, "attacking", "wolf reached attack range");
        }
        let bites = data
            .get::<std::sync::Mutex<Vec<(u64, crate::systems::combat::damage::DamageEvent)>>>(
                "damage_events",
            )
            .unwrap()
            .lock()
            .unwrap()
            .clone();
        assert!(!bites.is_empty(), "bites landed");
        let (target_bits, bite) = &bites[0];
        assert_eq!(*target_bits, u64::from(player.to_bits()));
        assert_eq!(bite.source_name.as_deref(), Some("Wolf"));
        assert!(!bite.source_is_player);
        assert!((bite.amount - 65.0 * BITE_DAMAGE_FRACTION).abs() < 0.01);
        // Cooldown: 60 seconds of ticks can land at most ~60/1.5 = 40 bites.
        assert!(
            bites.len() <= 1 + (60.0 / BITE_COOLDOWN_S) as usize,
            "bite cooldown respected ({} bites)",
            bites.len()
        );
    }
}
