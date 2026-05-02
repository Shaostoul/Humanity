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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Quest progress payload returned by `record_room_visit`. Drives the
/// `game_quest_progress` (or `game_quest_completed` when complete) broadcast
/// so AI agents and humans both learn the quest advanced.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestProgress {
    pub quest_id: String,
    pub room_id: String,
    pub visited_count: usize,
    pub total: usize,
    pub complete: bool,
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
    /// Uses typed deserialization via the existing `ship::layout` schema.
    fn load_starter_ship(&mut self) {
        use crate::ship::layout::ShipDef;

        let path = "data/ships/starter_fleet.ron";
        let contents = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Could not load {}: {} — game world has no ship layout", path, e);
                return;
            }
        };

        let ship: ShipDef = match ron::from_str(&contents) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("Failed to parse {}: {} — game world has no ship layout", path, e);
                return;
            }
        };

        self.ship_name = ship.name.clone();

        for deck in &ship.decks {
            for room in &deck.rooms {
                let room_type = room_type_to_str(&room.room_type);
                let equipment = room_equipment(room_type);

                let doors = room.doors.iter().map(|d| DoorDef {
                    connects_to: d.connects_to.clone(),
                    direction: direction_to_str(&d.direction).to_string(),
                }).collect();

                self.rooms.push(ShipRoom {
                    id: room.id.clone(),
                    name: room.name.clone(),
                    room_type: room_type.to_string(),
                    position: [room.position.x, room.position.y, room.position.z],
                    size: [room.size.x, room.size.y, room.size.z],
                    doors,
                    deck_name: deck.name.clone(),
                    ship_name: ship.name.clone(),
                    equipment,
                });
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

        // Spawn role-specific ambient NPCs per room type. Each has a wander
        // block so they drift around their assigned room, plus a 'role' field
        // that AI agents see in perception responses, plus a 'dialog' array
        // returned by handle_game_interact so AI agents (and humans) get a
        // bit of personality back when they interact. Makes the ship feel
        // crewed even when no humans are connected.
        for room in &rooms {
            let (npc_type, npc_name, role, description, dialog, greetings) = match room.room_type.as_str() {
                "bridge" => (
                    "navigator", "Helm Officer Vex", "navigator",
                    "Charts course at the bridge",
                    vec![
                        "Course laid in. We're holding orbit at 412 km.",
                        "If you see any unscheduled bursts on the console, flag me immediately.",
                        "Earth looks calm from up here. Don't let it fool you.",
                        "I'd kill for a fresh atlas. Real paper. Stars don't move that fast.",
                    ],
                    vec![
                        "Welcome to the bridge. Don't touch the helm.",
                        "Mind the cabling, citizen.",
                    ],
                ),
                "medbay" => (
                    "medic", "Dr. Kel", "medical_officer",
                    "Tending to the medbay",
                    vec![
                        "If you're hurt, sit on the cot. If you're not, don't touch anything.",
                        "Medkits are in the cabinet. They are not snacks.",
                        "Microgravity nausea is normal the first week. Hydrate.",
                        "I patched up worse than you yesterday. Stay still.",
                    ],
                    vec![
                        "Welcome to medical. State your symptom, briefly.",
                        "You look pale. Drink some water.",
                    ],
                ),
                "engineering" => (
                    "engineer", "Chief Tan", "chief_engineer",
                    "Monitoring the reactor",
                    vec![
                        "Reactor's at 84% and humming. Don't tap the glass.",
                        "If a panel is hot, I already know. Walk away.",
                        "Plumbing on Deck 3 is fixed. Try it before you complain again.",
                        "We keep this ship alive on duct tape and hope. Mostly hope.",
                    ],
                    vec![
                        "Engineering. Keep your hands behind the yellow line.",
                        "Welcome. The reactor whines. That's normal.",
                    ],
                ),
                "cargo" => (
                    "maintenance_bot", "CB-7", "maintenance",
                    "Autonomous bot patrolling cargo",
                    vec![
                        "[CB-7] Manifest reconciled. Variance: zero.",
                        "[CB-7] Bay temperature nominal. Resuming patrol.",
                        "[CB-7] Crate 19-A misaligned. Correcting.",
                        "[CB-7] Greetings, citizen. Please do not block the loaders.",
                    ],
                    vec![
                        "[CB-7] Citizen detected. Logging entry.",
                        "[CB-7] Welcome to Cargo. Please mind the loaders.",
                    ],
                ),
                "hydroponics" => (
                    "botanist", "Botanist Yara", "botanist",
                    "Tending the hydroponic racks",
                    vec![
                        "These tomatoes are six weeks ahead of schedule. Look at them.",
                        "Don't touch the green tray. I'm trialing a new nutrient mix.",
                        "Lettuce harvest in three days. Tell the galley.",
                        "Plants do better when you talk to them. I'm not joking.",
                    ],
                    vec![
                        "Welcome! Mind the spore filter at the door.",
                        "Quiet, please. The seedlings are sensitive.",
                    ],
                ),
                "quarters" => (
                    "crewmate", "Crewmate Nia", "off_duty",
                    "Reading on her bunk",
                    vec![
                        "Off-shift. Quiet hour. Whisper if you must.",
                        "Cycled through three novels this rotation. Got a recommendation?",
                        "Lights at 30%. The Earthrise is the best lamp anyway.",
                        "Wake me at 06:00 ship-time. Not earlier.",
                    ],
                    vec![
                        "Hey. Off-shift, but make yourself at home.",
                        "Bunk's free if you need a nap. Just say so.",
                    ],
                ),
                _ => continue,
            };
            let center_x = room.position[0] + room.size[0] / 2.0;
            let center_y = room.position[1] + 1.0;
            let center_z = room.position[2] + room.size[2] / 2.0;
            let id = self.next_entity_id;
            self.next_entity_id += 1;
            self.entities.insert(id, GameEntity {
                entity_type: npc_type.to_string(),
                position: [center_x, center_y, center_z],
                rotation: [0.0, 0.0, 0.0, 1.0],
                owner: None,
                components: serde_json::json!({
                    "interactable": true,
                    "room_id": room.id,
                    "name": npc_name,
                    "role": role,
                    "description": description,
                    "dialog": dialog,
                    "greetings": greetings,
                    "wander": {
                        "min_x": room.position[0] + 1.0,
                        "max_x": room.position[0] + room.size[0] - 1.0,
                        "min_z": room.position[2] + 1.0,
                        "max_z": room.position[2] + room.size[2] - 1.0,
                        "speed": 0.4,
                        "y": center_y,
                    },
                }),
                last_update: 0.0,
            });
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
    /// Spawns in Crew Quarters by default. Grants the explore_ship starter
    /// quest, pre-marking the spawn room as visited.
    pub fn spawn_player(&mut self, owner_key: &str, position: [f32; 3]) -> u64 {
        let id = self.next_entity_id;
        self.next_entity_id += 1;

        let spawn_pos = if position == [0.0_f32, 1.0, 0.0] {
            self.default_spawn_position()
        } else {
            position
        };

        let spawn_room_id = self.room_for_position(spawn_pos)
            .map(|r| r.id)
            .unwrap_or_else(|| "quarters".to_string());
        let total_rooms = self.rooms.len();

        let entity = GameEntity {
            entity_type: "player".to_string(),
            position: spawn_pos,
            rotation: [0.0, 0.0, 0.0, 1.0],
            owner: Some(owner_key.to_string()),
            components: serde_json::json!({
                "health": 100.0,
                "stamina": 100.0,
                "inventory": [],
                "current_quest": {
                    "id": "explore_ship",
                    "title": "Find your bearings",
                    "description": "Visit each room aboard the Pioneer to learn the ship's layout.",
                    "visited": [spawn_room_id],
                    "total_rooms": total_rooms,
                    "complete": false,
                },
            }),
            last_update: self.game_time,
        };
        self.entities.insert(id, entity);
        id
    }

    /// Find the resident NPC in a room (by room_id component) and return
    /// a random greeting line. None if no NPC, no greetings, or empty array.
    /// Used by handle_game_position_update on first room entry to surface
    /// "Welcome to medical, state your symptom" etc.
    pub fn pick_room_greeting(&self, room_id: &str) -> Option<(String, String)> {
        let npc = self.entities.values().find(|e| {
            e.components.get("room_id").and_then(|v| v.as_str()) == Some(room_id)
                && e.components.get("greetings").is_some()
        })?;
        let speaker = npc.components.get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();
        let lines = npc.components.get("greetings")?.as_array()?;
        if lines.is_empty() { return None; }
        use rand::Rng;
        let idx = rand::thread_rng().gen_range(0..lines.len());
        let line = lines[idx].as_str()?.to_string();
        Some((speaker, line))
    }

    /// Mark a room as visited on the player's current explore_ship quest.
    /// Returns Some(progress) if this visit was new (drives the
    /// `game_quest_progress` / `game_quest_completed` broadcast), None if
    /// the room was already visited or the player has no explore_ship quest.
    pub fn record_room_visit(
        &mut self,
        player_id: u64,
        room_id: &str,
    ) -> Option<QuestProgress> {
        let entity = self.entities.get_mut(&player_id)?;
        let quest = entity.components.get_mut("current_quest")?;
        if quest.get("id").and_then(|v| v.as_str()) != Some("explore_ship") {
            return None;
        }
        if quest.get("complete").and_then(|v| v.as_bool()).unwrap_or(false) {
            return None;
        }
        let visited = quest.get_mut("visited")?.as_array_mut()?;
        if visited.iter().any(|v| v.as_str() == Some(room_id)) {
            return None;
        }
        visited.push(serde_json::Value::String(room_id.to_string()));
        let visited_count = visited.len();
        let total = quest.get("total_rooms").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let complete = total > 0 && visited_count >= total;
        if complete {
            quest["complete"] = serde_json::Value::Bool(true);
        }
        Some(QuestProgress {
            quest_id: "explore_ship".to_string(),
            room_id: room_id.to_string(),
            visited_count,
            total,
            complete,
        })
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

    /// Advance the game simulation by dt seconds. Now also drives ambient
    /// NPC wander behavior — any entity with a `wander` block in its
    /// components JSON drifts to a random target within bounds at `speed`
    /// units/sec. This makes the world feel alive for AI agents perceiving
    /// it even when no humans are connected.
    pub fn tick(&mut self, dt: f64) {
        self.game_time += dt;

        // Apply wander to entities that have a `wander` component.
        // Random number generator scoped to this tick.
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let dt_f = dt as f32;

        let entity_ids: Vec<u64> = self.entities.keys().copied().collect();
        for id in entity_ids {
            let Some(entity) = self.entities.get_mut(&id) else { continue };
            let wander = match entity.components.get("wander").cloned() {
                Some(w) => w,
                None => continue,
            };
            let min_x = wander.get("min_x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let max_x = wander.get("max_x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let min_z = wander.get("min_z").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let max_z = wander.get("max_z").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let speed = wander.get("speed").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32;
            let y = wander.get("y").and_then(|v| v.as_f64()).unwrap_or(entity.position[1] as f64) as f32;

            // Brownian-style step in X/Z within bounds.
            let dx = (rng.gen::<f32>() - 0.5) * 2.0 * speed * dt_f;
            let dz = (rng.gen::<f32>() - 0.5) * 2.0 * speed * dt_f;
            entity.position[0] = (entity.position[0] + dx).clamp(min_x, max_x);
            entity.position[1] = y;
            entity.position[2] = (entity.position[2] + dz).clamp(min_z, max_z);
            entity.last_update = self.game_time;
        }
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

    // ── Persistence ────────────────────────────────────────────

    /// Storage key for the persisted world snapshot. Bump the version suffix
    /// when entity spawn logic changes so old snapshots are ignored on load
    /// (otherwise persisted entities would shadow newly-added ambient NPCs).
    pub const PERSIST_KEY: &'static str = "game_world_snapshot_v5";

    /// Save the world to the SQLite `server_state` table as a JSON blob.
    /// Called periodically from the tick loop. Static-ship fields (rooms,
    /// ship_name) are NOT saved — those reload from RON on every startup.
    pub fn save_to_db(&self, db: &crate::relay::storage::Storage) -> Result<(), String> {
        #[derive(Serialize)]
        struct Snapshot<'a> {
            entities: &'a HashMap<u64, GameEntity>,
            next_entity_id: u64,
            game_time: f64,
        }
        let snap = Snapshot {
            entities: &self.entities,
            next_entity_id: self.next_entity_id,
            game_time: self.game_time,
        };
        let json = serde_json::to_string(&snap).map_err(|e| format!("serialize: {e}"))?;
        db.set_state(Self::PERSIST_KEY, &json).map_err(|e| format!("save: {e}"))?;
        Ok(())
    }

    /// Restore world entities from the SQLite snapshot if one exists.
    /// Returns true if a snapshot was found and applied. Replaces freshly
    /// populated ship entities with the persisted set so player movement
    /// and inventory survive relay restarts.
    pub fn restore_from_db(&mut self, db: &crate::relay::storage::Storage) -> bool {
        #[derive(Deserialize)]
        struct Snapshot {
            entities: HashMap<u64, GameEntity>,
            next_entity_id: u64,
            game_time: f64,
        }
        let json = match db.get_state(Self::PERSIST_KEY) {
            Ok(Some(s)) => s,
            Ok(None) => return false,
            Err(e) => {
                tracing::warn!("Could not read game_world_snapshot: {e}");
                return false;
            }
        };
        match serde_json::from_str::<Snapshot>(&json) {
            Ok(snap) => {
                self.entities = snap.entities;
                self.next_entity_id = snap.next_entity_id.max(self.next_entity_id);
                self.game_time = snap.game_time;
                tracing::info!(
                    "Restored game world snapshot: {} entities, game_time={:.2}",
                    self.entities.len(), self.game_time
                );
                true
            }
            Err(e) => {
                tracing::warn!("Could not parse game_world_snapshot: {e}");
                false
            }
        }
    }
}

/// Convert a RoomType enum into the snake_case string used by room_equipment().
fn room_type_to_str(rt: &crate::ship::layout::RoomType) -> &'static str {
    use crate::ship::layout::RoomType::*;
    match rt {
        Bridge => "bridge",
        Quarters => "quarters",
        Cargo => "cargo",
        Engineering => "engineering",
        Medbay => "medbay",
        Hydroponics => "hydroponics",
        Armory => "armory",
        Hangar => "hangar",
    }
}

/// Convert a Direction enum into the snake_case string used in perception responses.
fn direction_to_str(d: &crate::ship::layout::Direction) -> &'static str {
    use crate::ship::layout::Direction::*;
    match d {
        North => "north",
        South => "south",
        East => "east",
        West => "west",
        Up => "up",
        Down => "down",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// GameWorld::new() should load data/ships/starter_fleet.ron and produce
    /// the Pioneer's 6 rooms. If RON parsing silently fails, rooms is empty.
    #[test]
    fn loads_starter_ship_with_six_rooms() {
        let world = GameWorld::new();
        assert_eq!(world.ship_name, "Pioneer", "ship name should parse from RON");
        assert_eq!(world.rooms.len(), 6, "Pioneer has 6 rooms (bridge, quarters, medbay, cargo, engineering, hydroponics)");

        let room_ids: Vec<&str> = world.rooms.iter().map(|r| r.id.as_str()).collect();
        for expected in ["bridge", "quarters", "medbay", "cargo", "engineering", "hydroponics"] {
            assert!(room_ids.contains(&expected), "missing room: {}", expected);
        }
    }

    /// Each room should have a non-zero size — a sanity check that position/size
    /// tuples parsed from RON correctly.
    #[test]
    fn rooms_have_non_zero_size() {
        let world = GameWorld::new();
        for room in &world.rooms {
            assert!(room.size[0] > 0.0, "{} has zero width", room.id);
            assert!(room.size[1] > 0.0, "{} has zero height", room.id);
            assert!(room.size[2] > 0.0, "{} has zero depth", room.id);
        }
    }

    /// The ship should be populated with interactable entities (equipment + windows).
    /// 6 rooms × ~5 equipment + 3 windows ≈ 33 entities.
    #[test]
    fn populates_ship_entities() {
        let world = GameWorld::new();
        assert!(world.entities.len() >= 20,
            "expected at least 20 ship entities, got {}", world.entities.len());

        // At least one window entity (for looking out at Earth).
        let windows = world.entities.values().filter(|e| e.entity_type == "window").count();
        assert!(windows >= 1, "expected at least 1 window entity");
    }

    /// room_for_position should resolve a point inside the Crew Quarters
    /// to the correct RoomInfo.
    #[test]
    fn room_for_position_resolves_quarters() {
        let world = GameWorld::new();
        // Default spawn is Crew Quarters center.
        let spawn = world.default_spawn_position();
        let room = world.room_for_position(spawn);
        assert!(room.is_some(), "spawn position must be inside a room");
        let room = room.unwrap();
        assert_eq!(room.id, "quarters");
        assert_eq!(room.name, "Crew Quarters");
        assert!(!room.exits.is_empty(), "Crew Quarters has 3 exits");
    }

    /// entities_near should return only entities within radius, sorted by distance.
    #[test]
    fn entities_near_filters_and_sorts() {
        let world = GameWorld::new();
        let spawn = world.default_spawn_position();
        let nearby = world.entities_near(spawn, 5.0);

        assert!(!nearby.is_empty(), "expected entities near spawn point");

        // Verify sorted ascending by distance.
        for window in nearby.windows(2) {
            assert!(window[0].distance <= window[1].distance, "entities not sorted");
        }

        // Verify all within radius.
        for e in &nearby {
            assert!(e.distance <= 5.0, "entity {} outside radius", e.entity_id);
        }
    }

    /// Spawning a player should put them in Crew Quarters when given the sentinel.
    #[test]
    fn spawn_player_uses_crew_quarters() {
        let mut world = GameWorld::new();
        let id = world.spawn_player("test_pubkey", [0.0, 1.0, 0.0]);
        let player = &world.entities[&id];

        let room = world.room_for_position(player.position);
        assert!(room.is_some(), "player not in any room");
        assert_eq!(room.unwrap().id, "quarters");
    }
}
