//! Server-side game world authority.
//!
//! Maintains the canonical game state: all entities, positions, and components.
//! The server is the single source of truth — clients send intents, the server
//! validates and broadcasts the authoritative result.
//!
//! Includes ship layout awareness and spatial queries so AI agents (and future
//! headless clients) can perceive the world through structured data instead of
//! rendered frames.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Snapshot / entity types ──

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

// ── Ship layout types (loaded from data/ships/) ──

/// A ship in the game world.
#[derive(Debug, Clone)]
pub struct ShipLayout {
    pub name: String,
    pub class: String,
    pub length: f32,
    pub width: f32,
    pub height: f32,
    pub decks: Vec<DeckLayout>,
}

/// A deck within a ship.
#[derive(Debug, Clone)]
pub struct DeckLayout {
    pub deck_index: u32,
    pub name: String,
    pub rooms: Vec<ShipRoom>,
}

/// A room within a ship deck, with AABB bounds for spatial lookup.
#[derive(Debug, Clone)]
pub struct ShipRoom {
    pub id: String,
    pub name: String,
    pub room_type: String,
    pub position: [f32; 3],
    pub size: [f32; 3],
    pub doors: Vec<DoorDef>,
    pub deck_name: String,
    pub ship_name: String,
    pub equipment: Vec<String>,
}

/// A door connecting two rooms.
#[derive(Debug, Clone)]
pub struct DoorDef {
    pub connects_to: String,
    pub direction: String,
}

// ── Perception response types ──

/// Room info returned by spatial queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomInfo {
    pub id: String,
    pub name: String,
    pub room_type: String,
    pub deck: String,
    pub ship: String,
    pub exits: Vec<DoorInfo>,
}

/// Door/exit info for perception responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoorInfo {
    pub direction: String,
    pub connects_to: String,
    pub room_name: String,
}

/// Brief entity info for nearby-entity queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityBrief {
    pub entity_id: u64,
    pub entity_type: String,
    pub name: Option<String>,
    pub distance: f32,
    pub position: [f32; 3],
    pub interactable: bool,
}

// ── Room equipment definitions (from data/rooms.ron) ──

fn room_equipment(room_type: &str) -> Vec<String> {
    match room_type {
        "bridge" => vec![
            "command_chair", "helm_console", "tactical_display",
            "comms_station", "viewscreen",
        ],
        "quarters" => vec![
            "bunk_bed", "locker", "desk_fold", "curtain_divider",
        ],
        "medbay" => vec![
            "medical_bed", "surgical_table", "medicine_cabinet",
            "diagnostic_scanner", "defibrillator",
        ],
        "cargo" => vec![
            "cargo_net", "mag_clamp_floor", "inventory_terminal",
            "freight_elevator",
        ],
        "engineering" => vec![
            "engineering_console", "power_junction", "tool_cabinet",
            "diagnostic_panel", "spare_parts_bin",
        ],
        "hydroponics" => vec![
            "hydro_rack", "nutrient_tank", "uv_grow_panel",
            "water_pump", "harvest_bin",
        ],
        _ => vec![],
    }.into_iter().map(String::from).collect()
}

// ── GameWorld ──

/// Server-authoritative game world state.
pub struct GameWorld {
    pub entities: HashMap<u64, GameEntity>,
    pub next_entity_id: u64,
    pub game_time: f64,
    pub tick_rate: f32,
    pub rooms: Vec<ShipRoom>,
    pub ship_name: String,
}

impl GameWorld {
    /// Initialize game world and load the starter ship layout.
    pub fn new() -> Self {
        let mut world = Self {
            entities: HashMap::new(),
            next_entity_id: 1,
            game_time: 0.0,
            tick_rate: 20.0,
            rooms: Vec::new(),
            ship_name: String::new(),
        };
        world.load_starter_ship();
        world.populate_ship_entities();
        world
    }

    /// Load the Pioneer frigate layout from data/ships/starter_fleet.ron.
    fn load_starter_ship(&mut self) {
        let path = "data/ships/starter_fleet.ron";
        let contents = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Could not load {}: {} — game world has no ship layout", path, e);
                return;
            }
        };

        let parsed: ron::Value = match ron::from_str(&contents) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("Failed to parse {}: {} — game world has no ship layout", path, e);
                return;
            }
        };

        let ship_name = parsed.clone().into_rust::<HashMap<String, ron::Value>>()
            .ok()
            .and_then(|m| m.get("name").and_then(|v| v.clone().into_rust::<String>().ok()))
            .unwrap_or_else(|| "Pioneer".to_string());

        self.ship_name = ship_name.clone();

        if let Ok(map) = parsed.into_rust::<HashMap<String, ron::Value>>() {
            if let Some(decks_val) = map.get("decks") {
                if let Ok(decks) = decks_val.clone().into_rust::<Vec<HashMap<String, ron::Value>>>() {
                    for deck in &decks {
                        let deck_name = deck.get("name")
                            .and_then(|v| v.clone().into_rust::<String>().ok())
                            .unwrap_or_default();

                        if let Some(rooms_val) = deck.get("rooms") {
                            if let Ok(rooms) = rooms_val.clone().into_rust::<Vec<HashMap<String, ron::Value>>>() {
                                for room in &rooms {
                                    let id = room.get("id")
                                        .and_then(|v| v.clone().into_rust::<String>().ok())
                                        .unwrap_or_default();
                                    let name = room.get("name")
                                        .and_then(|v| v.clone().into_rust::<String>().ok())
                                        .unwrap_or_default();
                                    let room_type = room.get("room_type")
                                        .and_then(|v| v.clone().into_rust::<String>().ok())
                                        .unwrap_or_default();

                                    let position = extract_f32_3(room.get("position"));
                                    let size = extract_f32_3(room.get("size"));

                                    let doors = room.get("doors")
                                        .and_then(|v| v.clone().into_rust::<Vec<HashMap<String, ron::Value>>>().ok())
                                        .unwrap_or_default()
                                        .iter()
                                        .map(|d| DoorDef {
                                            connects_to: d.get("connects_to")
                                                .and_then(|v| v.clone().into_rust::<String>().ok())
                                                .unwrap_or_default(),
                                            direction: d.get("direction")
                                                .and_then(|v| v.clone().into_rust::<String>().ok())
                                                .unwrap_or_default(),
                                        })
                                        .collect();

                                    let equipment = room_equipment(&room_type);

                                    self.rooms.push(ShipRoom {
                                        id,
                                        name,
                                        room_type,
                                        position,
                                        size,
                                        doors,
                                        deck_name: deck_name.clone(),
                                        ship_name: ship_name.clone(),
                                        equipment,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        tracing::info!(
            "Loaded ship '{}' with {} rooms",
            self.ship_name,
            self.rooms.len()
        );
    }

    /// Spawn static entities for each room's equipment (furniture, terminals, windows).
    fn populate_ship_entities(&mut self) {
        let rooms: Vec<ShipRoom> = self.rooms.clone();
        for room in &rooms {
            let room_center = [
                room.position[0] + room.size[0] / 2.0,
                room.position[1] + 1.0,
                room.position[2] + room.size[2] / 2.0,
            ];

            // Spread equipment around the room.
            for (i, equip) in room.equipment.iter().enumerate() {
                let angle = (i as f32) * std::f32::consts::TAU / room.equipment.len().max(1) as f32;
                let spread = room.size[0].min(room.size[2]) * 0.3;
                let pos = [
                    room_center[0] + angle.cos() * spread,
                    room_center[1],
                    room_center[2] + angle.sin() * spread,
                ];

                let id = self.next_entity_id;
                self.next_entity_id += 1;
                self.entities.insert(id, GameEntity {
                    entity_type: equip.clone(),
                    position: pos,
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    owner: None,
                    components: serde_json::json!({
                        "interactable": true,
                        "room_id": room.id,
                        "description": format!("{} in {}", equip, room.name),
                    }),
                    last_update: 0.0,
                });
            }

            // Add a window entity to rooms that would have external views.
            let has_window = matches!(
                room.room_type.as_str(),
                "bridge" | "quarters" | "medbay"
            );
            if has_window {
                let window_pos = [
                    room.position[0] + room.size[0] - 0.2,
                    room.position[1] + 1.5,
                    room.position[2] + room.size[2] / 2.0,
                ];
                let id = self.next_entity_id;
                self.next_entity_id += 1;
                self.entities.insert(id, GameEntity {
                    entity_type: "window".to_string(),
                    position: window_pos,
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    owner: None,
                    components: serde_json::json!({
                        "interactable": true,
                        "room_id": room.id,
                        "description": format!("Viewport in {} — look outside", room.name),
                        "view": {
                            "celestial_body": "Earth",
                            "distance_km": 400,
                            "orbit": "LEO",
                        },
                    }),
                    last_update: 0.0,
                });
            }
        }

        tracing::info!("Populated {} ship entities across {} rooms",
            self.entities.len(), self.rooms.len());
    }

    // ── Entity management ──

    /// Create a new entity in the world and return its ID.
    pub fn spawn_entity(&mut self, entity_type: &str, position: [f32; 3]) -> u64 {
        let id = self.next_entity_id;
        self.next_entity_id += 1;
        let entity = GameEntity {
            entity_type: entity_type.to_string(),
            position,
            rotation: [0.0, 0.0, 0.0, 1.0],
            owner: None,
            components: serde_json::Value::Object(serde_json::Map::new()),
            last_update: self.game_time,
        };
        self.entities.insert(id, entity);
        id
    }

    /// Spawn a player entity owned by the given public key.
    /// Spawns in Crew Quarters by default.
    pub fn spawn_player(&mut self, owner_key: &str, position: [f32; 3]) -> u64 {
        let id = self.next_entity_id;
        self.next_entity_id += 1;

        let spawn_pos = if position == [0.0_f32, 1.0, 0.0] {
            self.default_spawn_position()
        } else {
            position
        };

        let entity = GameEntity {
            entity_type: "player".to_string(),
            position: spawn_pos,
            rotation: [0.0, 0.0, 0.0, 1.0],
            owner: Some(owner_key.to_string()),
            components: serde_json::json!({
                "health": 100.0,
                "stamina": 100.0,
                "inventory": [],
            }),
            last_update: self.game_time,
        };
        self.entities.insert(id, entity);
        id
    }

    /// Default spawn position: center of Crew Quarters, 1m above floor.
    fn default_spawn_position(&self) -> [f32; 3] {
        self.rooms.iter()
            .find(|r| r.id == "quarters")
            .map(|r| [
                r.position[0] + r.size[0] / 2.0,
                r.position[1] + 1.0,
                r.position[2] + r.size[2] / 2.0,
            ])
            .unwrap_or([0.0, 1.0, 0.0])
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
    pub fn tick(&mut self, dt: f64) {
        self.game_time += dt;
    }

    /// Get the number of player entities currently in the world.
    pub fn player_count(&self) -> usize {
        self.entities.values().filter(|e| e.entity_type == "player").count()
    }

    /// Get the total entity count.
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    // ── Spatial queries (perception API) ──

    /// Find all entities within `radius` meters of `position`, sorted by distance.
    pub fn entities_near(&self, position: [f32; 3], radius: f32) -> Vec<EntityBrief> {
        let r2 = radius * radius;
        let mut results: Vec<EntityBrief> = self.entities.iter()
            .filter_map(|(id, e)| {
                let dx = e.position[0] - position[0];
                let dy = e.position[1] - position[1];
                let dz = e.position[2] - position[2];
                let dist_sq = dx * dx + dy * dy + dz * dz;
                if dist_sq > r2 { return None; }
                let distance = dist_sq.sqrt();

                let name = e.components.get("description")
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .or_else(|| e.owner.clone());

                let interactable = e.components.get("interactable")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                Some(EntityBrief {
                    entity_id: *id,
                    entity_type: e.entity_type.clone(),
                    name,
                    distance,
                    position: e.position,
                    interactable,
                })
            })
            .collect();
        results.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    /// Determine which room a position falls within (AABB containment).
    pub fn room_for_position(&self, position: [f32; 3]) -> Option<RoomInfo> {
        for room in &self.rooms {
            let min = room.position;
            let max = [
                room.position[0] + room.size[0],
                room.position[1] + room.size[1],
                room.position[2] + room.size[2],
            ];
            if position[0] >= min[0] && position[0] <= max[0]
                && position[1] >= min[1] && position[1] <= max[1]
                && position[2] >= min[2] && position[2] <= max[2]
            {
                return Some(self.room_info(room));
            }
        }
        None
    }

    /// Build RoomInfo with resolved door target names.
    fn room_info(&self, room: &ShipRoom) -> RoomInfo {
        let exits = room.doors.iter().map(|d| {
            let target_name = self.rooms.iter()
                .find(|r| r.id == d.connects_to)
                .map(|r| r.name.clone())
                .unwrap_or_else(|| d.connects_to.clone());
            DoorInfo {
                direction: d.direction.clone(),
                connects_to: d.connects_to.clone(),
                room_name: target_name,
            }
        }).collect();

        RoomInfo {
            id: room.id.clone(),
            name: room.name.clone(),
            room_type: room.room_type.clone(),
            deck: room.deck_name.clone(),
            ship: room.ship_name.clone(),
            exits,
        }
    }

    /// Get info for a room by ID.
    pub fn room_by_id(&self, room_id: &str) -> Option<RoomInfo> {
        self.rooms.iter()
            .find(|r| r.id == room_id)
            .map(|r| self.room_info(r))
    }
}

/// Extract [f32; 3] from a RON tuple value like (1.0, 2.0, 3.0).
fn extract_f32_3(val: Option<&ron::Value>) -> [f32; 3] {
    val.and_then(|v| {
        if let ron::Value::Seq(seq) = v {
            if seq.len() >= 3 {
                let x = seq[0].clone().into_rust::<f64>().ok()? as f32;
                let y = seq[1].clone().into_rust::<f64>().ok()? as f32;
                let z = seq[2].clone().into_rust::<f64>().ok()? as f32;
                return Some([x, y, z]);
            }
        }
        None
    })
    .unwrap_or([0.0, 0.0, 0.0])
}
