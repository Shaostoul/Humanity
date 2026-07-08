//! Combat system — damage processing, hit detection, death, status effects.
//!
//! Handles DamageEvents queued by other systems (spells, weapons, environment).
//! Applies armor/resistance, tracks status effects, processes death.
//!
//! Registered live in v0.760 (combat arc increment 1). Hits arrive through
//! the `damage_events` DataStore channel (entity bits + event, the same
//! Mutex-channel shape every cross-system request uses); deaths roll the
//! entity's LootTable and push the drops onto `loot_drops`, which lib.rs's
//! frame bridge settles into the killer's pack. Dead CREATURES despawn after
//! a short decay so the pasture does not fill with corpses; a dead PLAYER is
//! never despawned (the respawn flow owns that Dead marker).

pub mod damage;
pub mod effects;

use rand::Rng;

use crate::ecs::components::{Armor, Creature, Dead, Health, LootTable, Transform};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use damage::{DamageEvent, DamageType};
use effects::StatusEffect;

/// Game-seconds a dead creature lingers before despawning.
const CREATURE_DECAY_S: f32 = 20.0;

/// Convert a DamageType enum into the lowercase string key used in Armor.resistance.
fn damage_type_key(t: &DamageType) -> &'static str {
    match t {
        DamageType::Kinetic   => "kinetic",
        DamageType::Thermal   => "thermal",
        DamageType::Energy    => "energy",
        DamageType::Chemical  => "chemical",
        DamageType::Radiation => "radiation",
    }
}

/// Combat system: processes damage events, status effects, and death.
pub struct CombatSystem {
    /// Pending damage events to process this frame.
    pub pending_damage: Vec<(hecs::Entity, DamageEvent)>,
    /// Active status effects per entity.
    pub active_effects: std::collections::HashMap<u64, Vec<StatusEffect>>,
}

impl CombatSystem {
    pub fn new() -> Self {
        Self {
            pending_damage: Vec::new(),
            active_effects: std::collections::HashMap::new(),
        }
    }

    /// Queue a damage event to be processed next tick.
    pub fn deal_damage(&mut self, target: hecs::Entity, event: DamageEvent) {
        self.pending_damage.push((target, event));
    }

    /// Apply a status effect to an entity.
    pub fn apply_effect(&mut self, entity: hecs::Entity, effect: StatusEffect) {
        let key = entity.to_bits().into();
        self.active_effects.entry(key).or_default().push(effect);
    }
}

impl System for CombatSystem {
    fn name(&self) -> &str {
        "combat"
    }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, data: &DataStore) {
        // ── Drain the damage_events channel (v0.760) ── the GUI bridge and
        // other systems queue hits here as (entity bits, event).
        if let Some(chan) = data.get::<std::sync::Mutex<Vec<(u64, DamageEvent)>>>("damage_events") {
            if let Ok(mut incoming) = chan.lock() {
                for (bits, event) in incoming.drain(..) {
                    if let Some(entity) = hecs::Entity::from_bits(bits) {
                        self.pending_damage.push((entity, event));
                    }
                }
            }
        }

        // ── Process pending damage events ──
        let events: Vec<_> = self.pending_damage.drain(..).collect();
        let mut deaths_to_handle: Vec<(hecs::Entity, Option<String>, bool, DamageType)> =
            Vec::new();

        for (entity, event) in events {
            // Skip if already dead.
            if world.get::<&Dead>(entity).is_ok() { continue; }

            // Apply armor mitigation: damage *= (1.0 - resistance).
            let key = damage_type_key(&event.damage_type);
            let resistance = world
                .get::<&Armor>(entity)
                .ok()
                .and_then(|a| a.resistance.get(key).copied())
                .unwrap_or(0.0)
                .clamp(0.0, 1.0);
            let mitigated = event.amount * (1.0 - resistance);

            if let Ok(mut health) = world.get::<&mut Health>(entity) {
                health.current = (health.current - mitigated).max(0.0);
                if health.current <= 0.0 {
                    log::info!(
                        "Entity {:?} killed by {:.1} {:?} damage ({:.0}% mitigated by armor)",
                        entity, mitigated, event.damage_type, resistance * 100.0
                    );
                    deaths_to_handle.push((
                        entity,
                        event.source_name,
                        event.source_is_player,
                        event.damage_type,
                    ));
                }
            }
        }

        // ── Trigger death: insert Dead, roll loot into the loot_drops channel ──
        let mut rng = rand::thread_rng();
        for (entity, source_name, source_is_player, killing_type) in deaths_to_handle {
            // Insert Dead marker (no-op if already present).
            let _ = world.insert_one(entity, Dead::default());

            // A dead PLAYER publishes its cause for the death screen (the
            // same slot FoodSystem's environmental deaths use). (v0.761)
            if world.get::<&crate::ecs::components::Controllable>(entity).is_ok() {
                if let Some(slot) =
                    data.get::<std::sync::Mutex<Option<String>>>("player_death")
                {
                    if let Ok(mut s) = slot.lock() {
                        if s.is_none() {
                            *s = Some(match &source_name {
                                Some(n) => format!("killed by a {n}"),
                                None => "killed in combat".to_string(),
                            });
                        }
                    }
                }
                continue; // players drop no loot
            }

            // The player's killing blow trains a combat skill (v0.762) - the
            // first XP source for the combat category. Kinetic kills train
            // melee; everything else (spells, energy) trains ranged.
            if source_is_player {
                let skill = match killing_type {
                    DamageType::Kinetic => "melee",
                    _ => "ranged",
                };
                crate::systems::skills::award_skill_xp(data, skill, 10);
            }

            // Roll loot if the entity has a LootTable: per entry, chance
            // gates the drop, count rolls uniformly in min..=max.
            let drops: Vec<(String, u32)> = world.get::<&LootTable>(entity)
                .map(|table| {
                    table.entries.iter()
                        .filter_map(|(item, chance, min, max)| {
                            if rng.gen::<f32>() < *chance {
                                let count = if max > min {
                                    rng.gen_range(*min..=*max)
                                } else {
                                    *min
                                };
                                (count > 0).then(|| (item.clone(), count))
                            } else {
                                None
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();

            if !drops.is_empty() {
                if let Ok(mut dead) = world.get::<&mut Dead>(entity) {
                    dead.looted = true;
                }
                let position = world.get::<&Transform>(entity).ok().map(|t| t.position);
                log::info!("Loot dropped from {:?} at {:?}: {:?}", entity, position, drops);
                // Deliver to the killer's pack ONLY when the player landed
                // the killing blow - a wolf's kill is the wolf's dinner, not
                // your loot. (v0.761)
                if source_is_player {
                    if let Some(chan) =
                        data.get::<std::sync::Mutex<Vec<(String, u32)>>>("loot_drops")
                    {
                        if let Ok(mut out) = chan.lock() {
                            out.extend(drops);
                        }
                    }
                }
            }
        }

        // ── Age Dead components (consumers can use this for respawn timers / cleanup) ──
        for (_, dead) in world.query_mut::<&mut Dead>() {
            dead.since += dt;
        }

        // ── Despawn decayed creature corpses ── ONLY entities with a
        // Creature component; a dead player is the respawn flow's business.
        let decayed: Vec<hecs::Entity> = world
            .query::<(&Dead, &Creature)>()
            .iter()
            .filter(|(_, (d, _))| d.since > CREATURE_DECAY_S)
            .map(|(e, _)| e)
            .collect();
        for e in decayed {
            let _ = world.despawn(e);
        }

        // ── Tick status effects ──
        let mut expired_keys = Vec::new();
        for (entity_bits, effects) in self.active_effects.iter_mut() {
            effects.retain_mut(|effect| {
                let expired = effect.tick(dt);
                if expired {
                    log::debug!("Effect '{}' expired on entity {}", effect.name, entity_bits);
                }
                !expired
            });
            if effects.is_empty() {
                expired_keys.push(*entity_bits);
            }
        }
        for key in expired_keys {
            self.active_effects.remove(&key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    fn store_with_channels() -> DataStore {
        let mut data = DataStore::new();
        data.insert(
            "damage_events",
            std::sync::Mutex::new(Vec::<(u64, DamageEvent)>::new()),
        );
        data.insert(
            "loot_drops",
            std::sync::Mutex::new(Vec::<(String, u32)>::new()),
        );
        data.insert(
            "xp_grants",
            std::sync::Mutex::new(Vec::<crate::systems::skills::SkillXPEvent>::new()),
        );
        data
    }

    fn spawn_hen(world: &mut hecs::World) -> hecs::Entity {
        world.spawn((
            Creature {
                def_id: "chicken".into(),
                anchor: Vec3::ZERO,
                range: 2.0,
                phase: 0.0,
                speed: 0.5,
                tint: [1.0, 1.0, 1.0],
                body_side: 0.15,
            },
            Transform::default(),
            Health { current: 15.0, max: 15.0 },
            LootTable {
                // Certain drops so the test is deterministic.
                entries: vec![
                    ("raw_poultry_0".into(), 1.0, 2, 2),
                    ("feather_0".into(), 0.0, 1, 3), // never drops
                ],
            },
        ))
    }

    /// THE kill loop: a channel-queued hit damages through to death, the
    /// Dead marker lands, certain loot reaches the loot_drops channel, and
    /// zero-chance entries never do.
    #[test]
    fn channel_hit_kills_and_drops_loot() {
        let mut world = hecs::World::new();
        let data = store_with_channels();
        let hen = spawn_hen(&mut world);

        let push_hit = |amount: f32| {
            data.get::<std::sync::Mutex<Vec<(u64, DamageEvent)>>>("damage_events")
                .unwrap()
                .lock()
                .unwrap()
                .push((hen.to_bits().into(), DamageEvent {
                    damage_type: DamageType::Thermal,
                    amount,
                    source_name: None,
                    source_is_player: true,
                }));
        };

        let mut sys = CombatSystem::new();
        push_hit(10.0);
        sys.tick(&mut world, 0.016, &data);
        assert_eq!(world.get::<&Health>(hen).unwrap().current, 5.0);
        assert!(world.get::<&Dead>(hen).is_err(), "still alive at 5 hp");

        push_hit(10.0);
        sys.tick(&mut world, 0.016, &data);
        assert!(world.get::<&Dead>(hen).is_ok(), "dead at 0 hp");
        let drops = data
            .get::<std::sync::Mutex<Vec<(String, u32)>>>("loot_drops")
            .unwrap()
            .lock()
            .unwrap()
            .clone();
        assert_eq!(drops, vec![("raw_poultry_0".to_string(), 2)]);
        // The player's thermal killing blow trained ranged (v0.762).
        let grants = data
            .get::<std::sync::Mutex<Vec<crate::systems::skills::SkillXPEvent>>>("xp_grants")
            .unwrap()
            .lock()
            .unwrap()
            .iter()
            .map(|g| (g.skill_id.clone(), g.amount))
            .collect::<Vec<_>>();
        assert_eq!(grants, vec![("ranged".to_string(), 10)]);

        // Overkill on a corpse: no double death, no double loot.
        push_hit(50.0);
        sys.tick(&mut world, 0.016, &data);
        let drops = data
            .get::<std::sync::Mutex<Vec<(String, u32)>>>("loot_drops")
            .unwrap()
            .lock()
            .unwrap()
            .clone();
        assert_eq!(drops.len(), 1, "corpses do not drop twice");
    }

    /// Armor mitigation scales the hit, and dead CREATURES despawn after the
    /// decay window while a dead non-creature (the player) never does.
    #[test]
    fn armor_mitigates_and_corpses_decay() {
        let mut world = hecs::World::new();
        let data = store_with_channels();
        let hen = spawn_hen(&mut world);
        let mut resistance = std::collections::HashMap::new();
        resistance.insert("thermal".to_string(), 0.5f32);
        world.insert_one(hen, Armor { resistance }).unwrap();

        // A "player": Health + Dead, no Creature.
        let player = world.spawn((Health { current: 0.0, max: 100.0 }, Dead::default()));

        let mut sys = CombatSystem::new();
        data.get::<std::sync::Mutex<Vec<(u64, DamageEvent)>>>("damage_events")
            .unwrap()
            .lock()
            .unwrap()
            .push((hen.to_bits().into(), DamageEvent {
                damage_type: DamageType::Thermal,
                amount: 10.0,
                source_name: None,
                source_is_player: true,
            }));
        sys.tick(&mut world, 0.016, &data);
        assert_eq!(
            world.get::<&Health>(hen).unwrap().current,
            10.0,
            "50% thermal armor halves the 10 hit"
        );

        // Kill it, then age past the decay window: the hen despawns, the
        // dead player does not.
        data.get::<std::sync::Mutex<Vec<(u64, DamageEvent)>>>("damage_events")
            .unwrap()
            .lock()
            .unwrap()
            .push((hen.to_bits().into(), DamageEvent {
                damage_type: DamageType::Kinetic,
                amount: 100.0,
                source_name: None,
                source_is_player: true,
            }));
        sys.tick(&mut world, 0.016, &data);
        assert!(world.get::<&Dead>(hen).is_ok());
        sys.tick(&mut world, CREATURE_DECAY_S + 1.0, &data);
        sys.tick(&mut world, 0.016, &data);
        assert!(!world.contains(hen), "creature corpse decayed away");
        assert!(world.contains(player), "a dead player is never despawned");
    }

    /// A combat death of the PLAYER publishes its cause to the same
    /// player_death slot environmental deaths use ("killed by a Wolf"),
    /// drops no loot, and a non-player kill delivers no pack loot. (v0.761)
    #[test]
    fn player_combat_death_records_cause_and_wolf_kills_grant_no_loot() {
        let mut world = hecs::World::new();
        let mut data = store_with_channels();
        data.insert(
            "player_death",
            std::sync::Mutex::new(Option::<String>::None),
        );

        let player = world.spawn((
            crate::ecs::components::Controllable,
            Health { current: 8.0, max: 100.0 },
        ));
        let hen = spawn_hen(&mut world);

        let mut sys = CombatSystem::new();
        // A wolf bite kills the player.
        data.get::<std::sync::Mutex<Vec<(u64, DamageEvent)>>>("damage_events")
            .unwrap()
            .lock()
            .unwrap()
            .push((player.to_bits().into(), DamageEvent {
                damage_type: DamageType::Kinetic,
                amount: 10.0,
                source_name: Some("Wolf".to_string()),
                source_is_player: false,
            }));
        // The wolf also kills the hen: no loot for the player's pack.
        data.get::<std::sync::Mutex<Vec<(u64, DamageEvent)>>>("damage_events")
            .unwrap()
            .lock()
            .unwrap()
            .push((hen.to_bits().into(), DamageEvent {
                damage_type: DamageType::Kinetic,
                amount: 100.0,
                source_name: Some("Wolf".to_string()),
                source_is_player: false,
            }));
        sys.tick(&mut world, 0.016, &data);

        assert!(world.get::<&Dead>(player).is_ok(), "player died");
        let cause = data
            .get::<std::sync::Mutex<Option<String>>>("player_death")
            .unwrap()
            .lock()
            .unwrap()
            .clone();
        assert_eq!(cause.as_deref(), Some("killed by a Wolf"));

        assert!(world.get::<&Dead>(hen).is_ok(), "hen died to the wolf");
        let drops = data
            .get::<std::sync::Mutex<Vec<(String, u32)>>>("loot_drops")
            .unwrap()
            .lock()
            .unwrap()
            .clone();
        assert!(drops.is_empty(), "a wolf's kill is not the player's loot");
    }
}
