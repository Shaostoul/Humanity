//! Combat system — damage processing, hit detection, death, status effects.
//!
//! Handles DamageEvents queued by other systems (spells, weapons, environment).
//! Applies armor/resistance, tracks status effects, processes death.

pub mod damage;
pub mod effects;

use crate::ecs::systems::System;
use crate::ecs::components::Health;
use crate::hot_reload::data_store::DataStore;
use damage::DamageEvent;
use effects::StatusEffect;

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

    fn tick(&mut self, world: &mut hecs::World, dt: f32, _data: &DataStore) {
        // ── Process pending damage events ──
        let events: Vec<_> = self.pending_damage.drain(..).collect();
        for (entity, event) in events {
            if let Ok(mut health) = world.get::<&mut Health>(entity) {
                // TODO: apply armor/resistance based on damage type
                health.current = (health.current - event.amount).max(0.0);

                if health.current <= 0.0 {
                    log::info!(
                        "Entity {:?} killed by {:.1} {:?} damage",
                        entity, event.amount, event.damage_type
                    );
                    // TODO: trigger death (drop loot, animation, respawn timer)
                }
            }
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
