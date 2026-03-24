//! Server-side game world authority.
//!
//! Maintains the canonical game state: all entities, positions, and components.
//! The server is the single source of truth — clients send intents, the server
//! validates and broadcasts the authoritative result.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Snapshot of a single entity for initial world state transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySnapshot {
    pub entity_id: u64,
    pub entity_type: String,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub owner: Option<String>,
    pub components: serde_json::Value,
}

/// A game entity tracked by the server.
#[derive(Debug, Clone)]
pub struct GameEntity {
    pub entity_type: String,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub owner: Option<String>,
    pub components: serde_json::Value,
    pub last_update: f64,
}

/// Server-authoritative game world state.
pub struct GameWorld {
    pub entities: HashMap<u64, GameEntity>,
    pub next_entity_id: u64,
    pub game_time: f64,
    pub tick_rate: f32,
}

impl GameWorld {
    /// Initialize an empty game world.
    pub fn new() -> Self {
        Self {
            entities: HashMap::new(),
            next_entity_id: 1,
            game_time: 0.0,
            tick_rate: 20.0, // 20 ticks per second
        }
    }

    /// Create a new entity in the world and return its ID.
    pub fn spawn_entity(&mut self, entity_type: &str, position: [f32; 3]) -> u64 {
        let id = self.next_entity_id;
        self.next_entity_id += 1;
        let entity = GameEntity {
            entity_type: entity_type.to_string(),
            position,
            rotation: [0.0, 0.0, 0.0, 1.0], // identity quaternion
            owner: None,
            components: serde_json::Value::Object(serde_json::Map::new()),
            last_update: self.game_time,
        };
        self.entities.insert(id, entity);
        id
    }

    /// Spawn a player entity owned by the given public key.
    pub fn spawn_player(&mut self, owner_key: &str, position: [f32; 3]) -> u64 {
        let id = self.next_entity_id;
        self.next_entity_id += 1;
        let entity = GameEntity {
            entity_type: "player".to_string(),
            position,
            rotation: [0.0, 0.0, 0.0, 1.0],
            owner: Some(owner_key.to_string()),
            components: serde_json::json!({
                "health": 100.0,
                "stamina": 100.0,
            }),
            last_update: self.game_time,
        };
        self.entities.insert(id, entity);
        id
    }

    /// Remove an entity from the world. Returns true if it existed.
    pub fn despawn_entity(&mut self, id: u64) -> bool {
        self.entities.remove(&id).is_some()
    }

    /// Remove the player entity owned by the given key. Returns the entity ID if found.
    pub fn despawn_player(&mut self, owner_key: &str) -> Option<u64> {
        let id = self.entities.iter()
            .find(|(_, e)| e.owner.as_deref() == Some(owner_key) && e.entity_type == "player")
            .map(|(id, _)| *id);
        if let Some(id) = id {
            self.entities.remove(&id);
        }
        id
    }

    /// Find the entity ID for a player by their owner key.
    pub fn find_player_entity(&self, owner_key: &str) -> Option<u64> {
        self.entities.iter()
            .find(|(_, e)| e.owner.as_deref() == Some(owner_key) && e.entity_type == "player")
            .map(|(id, _)| *id)
    }

    /// Update an entity's position and rotation. Returns false if entity not found.
    pub fn update_position(&mut self, id: u64, position: [f32; 3], rotation: [f32; 4]) -> bool {
        if let Some(entity) = self.entities.get_mut(&id) {
            entity.position = position;
            entity.rotation = rotation;
            entity.last_update = self.game_time;
            true
        } else {
            false
        }
    }

    /// Get a full snapshot of the world for new joiners.
    pub fn snapshot(&self) -> Vec<EntitySnapshot> {
        self.entities.iter().map(|(id, e)| EntitySnapshot {
            entity_id: *id,
            entity_type: e.entity_type.clone(),
            position: e.position,
            rotation: e.rotation,
            owner: e.owner.clone(),
            components: e.components.clone(),
        }).collect()
    }

    /// Advance the game simulation by dt seconds.
    /// This is where server-side NPC AI, physics, and world updates run.
    pub fn tick(&mut self, dt: f64) {
        self.game_time += dt;

        // Future: NPC AI, server-side physics, resource respawn, etc.
        // For now, just advance game time.
    }

    /// Get the number of player entities currently in the world.
    pub fn player_count(&self) -> usize {
        self.entities.values().filter(|e| e.entity_type == "player").count()
    }

    /// Get the total entity count.
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }
}
