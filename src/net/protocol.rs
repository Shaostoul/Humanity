//! Multiplayer message protocol for ECS state synchronization.

use serde::{Deserialize, Serialize};

/// All multiplayer message types exchanged between client and server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NetMessage {
    // ── Connection ──
    /// Client requests to join the game world.
    Join {
        player_name: String,
        public_key: String,
    },
    /// Server welcomes client with their ID and a world snapshot.
    Welcome {
        player_id: u32,
        world_snapshot: Vec<EntitySnapshot>,
    },
    /// Another player joined the world.
    PlayerJoined {
        player_id: u32,
        name: String,
        position: [f32; 3],
    },
    /// A player left the world.
    PlayerLeft {
        player_id: u32,
    },

    // ── State sync ──
    /// Position/rotation update for a player (sent frequently).
    PositionUpdate {
        player_id: u32,
        position: [f32; 3],
        rotation: [f32; 4],
        velocity: [f32; 3],
        timestamp: f64,
    },
    /// Server spawns a new entity in the world.
    EntitySpawn {
        entity_id: u64,
        entity_type: String,
        position: [f32; 3],
        data: serde_json::Value,
    },
    /// Server removes an entity from the world.
    EntityDespawn {
        entity_id: u64,
    },
    /// A component on an entity changed.
    EntityUpdate {
        entity_id: u64,
        component: String,
        data: serde_json::Value,
    },

    // ── Actions ──
    /// Player interacts with an entity.
    InteractWith {
        entity_id: u64,
        action: String,
    },
    /// In-game chat message.
    ChatMessage {
        sender: String,
        content: String,
        channel: String,
    },

    // ── World streaming ──
    /// Client requests a terrain chunk.
    ChunkRequest {
        x: i32,
        y: i32,
        z: i32,
    },
    /// Server sends terrain chunk data.
    ChunkData {
        x: i32,
        y: i32,
        z: i32,
        data: Vec<u8>,
    },
    /// Server synchronizes game clock.
    TimeSync {
        game_time: f64,
        server_time: f64,
    },

    // ── Heartbeat ──
    Ping { timestamp: f64 },
    Pong { timestamp: f64 },
}

/// Snapshot of a single entity for initial world state transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySnapshot {
    pub entity_id: u64,
    pub entity_type: String,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub components: serde_json::Value,
}
