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

// ── Crew chores (loaded from data/npc/chores.ron) ──

/// One chore an ambient crew NPC can perform: walk to `room_id`, dwell there
/// "working" for `duration_secs`, then rotate to the next allowed chore.
/// Data-driven per the infinite-of-X rule; see schemas/chore.toml.
#[derive(Debug, Clone, Deserialize)]
pub struct ChoreDef {
    pub id: String,
    pub label: String,
    pub room_id: String,
    pub duration_secs: f32,
    /// Roles allowed to do this chore; empty = any crew NPC.
    pub roles: Vec<String>,
}

/// Event emitted by `GameWorld::tick` when a crew NPC's chore state changes
/// (or, throttled, while it travels). The relay broadcasts these as
/// `game_npc_update` messages so clients can render crew actually doing
/// things: position + the human-readable chore label.
#[derive(Debug, Clone, Serialize)]
pub struct NpcChoreEvent {
    pub entity_id: u64,
    pub name: String,
    pub position: [f32; 3],
    pub chore_id: String,
    pub chore_label: String,
    /// "traveling" | "working" | "completed"
    pub chore_state: String,
    pub room_id: String,
}

/// Walking speed for a crew NPC traveling to a chore site (m/s). Faster than
/// the old wander drift (0.4) so travel reads as purposeful walking.
pub const CHORE_WALK_SPEED: f32 = 1.1;

/// How often traveling NPC positions are broadcast to clients (seconds).
/// State transitions (assigned / working / completed) always broadcast.
pub const NPC_POSITION_BROADCAST_INTERVAL: f64 = 0.5;

/// Deterministic chore rotation: which entry of an NPC's ALLOWED chore list
/// it does next. `npc_seq` staggers crew so they don't all start on the same
/// chore; `chores_done` advances the rotation one step per completed chore.
/// Pure logic (no world, no RNG) so it is directly unit-testable.
pub fn next_chore_index(npc_seq: usize, chores_done: u64, chore_count: usize) -> Option<usize> {
    if chore_count == 0 {
        return None;
    }
    Some(((npc_seq as u64 + chores_done) % chore_count as u64) as usize)
}

/// Straight-line movement step toward a target (no pathfinding exists in the
/// engine yet -- an accepted, honest limitation of this first slice). Returns
/// the new position and whether the mover arrived this step. Never overshoots:
/// when the remaining distance fits inside this step, snaps to the target.
/// Pure logic so travel timing is directly unit-testable.
pub fn step_toward(pos: [f32; 3], target: [f32; 3], speed: f32, dt: f32) -> ([f32; 3], bool) {
    let d = [target[0] - pos[0], target[1] - pos[1], target[2] - pos[2]];
    let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let step = (speed * dt).max(1e-6);
    if dist <= step {
        (target, true)
    } else {
        let k = step / dist;
        ([pos[0] + d[0] * k, pos[1] + d[1] * k, pos[2] + d[2] * k], false)
    }
}

/// Indices into `chores` that a crew member with `role` may perform
/// (a chore with an empty roles list is open to everyone).
pub fn allowed_chore_indices(chores: &[ChoreDef], role: &str) -> Vec<usize> {
    chores
        .iter()
        .enumerate()
        .filter(|(_, c)| c.roles.is_empty() || c.roles.iter().any(|r| r == role))
        .map(|(i, _)| i)
        .collect()
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

/// Quest progress payload returned by `record_room_visit` or
/// `record_npc_talk`. `step_id` is the room id (for explore_ship) or NPC
/// name (for meet_the_crew). `room_id` is kept as an alias of step_id for
/// backward compatibility with AI clients that consumed the v0.167 shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestProgress {
    pub quest_id: String,
    pub step_id: String,
    /// Alias of step_id; preserved for v0.167 compatibility.
    pub room_id: String,
    pub visited_count: usize,
    pub total: usize,
    pub complete: bool,
}

/// Reward payload returned by `apply_quest_reward` when a quest finishes.
/// Drives the `game_quest_reward` private event so the player sees what
/// they earned. xp_total / reputation_total are the post-application stats
/// so AI agents don't need a follow-up query to read them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestReward {
    pub quest_id: String,
    pub xp: u64,
    pub reputation: u64,
    pub message: String,
    pub xp_total: u64,
    pub reputation_total: u64,
}

// ── Room equipment definitions (from data/rooms.ron) ──

/// Storage-class equipment that the survey_storage quest cares about.
/// Anything matching gets `"storage": true` in its components and counts
/// toward the quest goal when interacted with.
fn is_storage_equipment(equip: &str) -> bool {
    matches!(
        equip,
        "locker" | "medicine_cabinet" | "tool_cabinet"
        | "spare_parts_bin" | "harvest_bin" | "inventory_terminal"
    )
}

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
    /// Crew chore catalog from data/npc/chores.ron. Empty when the file is
    /// missing/unparseable -- crew then fall back to the old wander drift.
    pub chores: Vec<ChoreDef>,
    /// Accumulator throttling traveling-NPC position broadcasts (not persisted).
    npc_broadcast_accum: f64,
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
            chores: Vec::new(),
            npc_broadcast_accum: 0.0,
        };
        world.load_starter_ship();
        world.load_chores();
        world.populate_ship_entities();
        world
    }

    /// Load the crew chore catalog from data/npc/chores.ron. Chores whose
    /// room_id doesn't resolve against the loaded ship layout are dropped
    /// with a warning (a typo'd room must not strand an NPC walking forever
    /// toward nowhere). Missing/unparseable file -> empty catalog -> crew
    /// keep the legacy wander behavior.
    fn load_chores(&mut self) {
        let path = "data/npc/chores.ron";
        let contents = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Could not load {}: {} -- crew NPCs will wander instead of doing chores", path, e);
                return;
            }
        };
        let chores: Vec<ChoreDef> = match ron::from_str(&contents) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("Failed to parse {}: {} -- crew NPCs will wander instead of doing chores", path, e);
                return;
            }
        };
        let (valid, dropped): (Vec<ChoreDef>, Vec<ChoreDef>) = chores
            .into_iter()
            .partition(|c| self.rooms.iter().any(|r| r.id == c.room_id));
        for c in &dropped {
            tracing::warn!("Chore '{}' references unknown room '{}' -- dropped", c.id, c.room_id);
        }
        tracing::info!("Loaded {} crew chores from {}", valid.len(), path);
        self.chores = valid;
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
                        // Storage flag for the survey_storage quest (v0.172).
                        // True for cabinets / bins / lockers across rooms.
                        "storage": is_storage_equipment(equip),
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
        //
        // Chore AI (data-driven, see data/npc/chores.ron): each crew NPC is
        // also a `chore_agent` with a stable `npc_seq`. When the chore catalog
        // is loaded, tick() replaces the wander drift with a real task loop
        // (walk to a chore site, dwell "working", rotate to the next chore);
        // the wander block remains only as a fallback for an empty catalog.
        let mut npc_seq: u64 = 0;
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
                    // Chore AI wiring: stable per-crew rotation offset + the
                    // live activity label clients show ("Taking reactor
                    // readings"). `chores_done` advances the rotation.
                    "chore_agent": true,
                    "npc_seq": npc_seq,
                    "chores_done": 0,
                    "activity": description,
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
            npc_seq += 1;
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
                // RPG-ish progression. Earned by completing quests.
                "xp": 0,
                "reputation": 0,
                "completed_quests": [],
                "current_quest": {
                    "id": "explore_ship",
                    "title": "Find your bearings",
                    "description": "Visit each room aboard the Pioneer to learn the ship's layout.",
                    "visited": [spawn_room_id],
                    "total_rooms": total_rooms,
                    "complete": false,
                    "reward": {
                        "xp": 100,
                        "reputation": 5,
                        "message": "You're getting your bearings. The crew nods as you pass.",
                    },
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
    /// Does NOT chain to the next quest — caller should call
    /// `apply_quest_reward` then `chain_next_quest` after a completion so
    /// the reward fires for the right quest.
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
            step_id: room_id.to_string(),
            room_id: room_id.to_string(),
            visited_count,
            total,
            complete,
        })
    }

    /// Promote the player to the next quest in the chain after the current
    /// one completes. Returns the new quest payload if a chain exists, None
    /// if the chain has ended. Idempotent: only fires when current_quest is
    /// complete and no follow-up has been set.
    pub fn chain_next_quest(&mut self, player_id: u64) -> Option<serde_json::Value> {
        let total_npcs = self.crew_npc_count();
        let total_storage = self.storage_entity_count();
        let entity = self.entities.get_mut(&player_id)?;
        let current_id = entity.components.get("current_quest")
            .and_then(|q| q.get("id"))
            .and_then(|v| v.as_str())?
            .to_string();
        let complete = entity.components.get("current_quest")
            .and_then(|q| q.get("complete"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !complete { return None; }

        let next = match current_id.as_str() {
            "explore_ship" => Some(serde_json::json!({
                "id": "meet_the_crew",
                "title": "Meet the crew",
                "description": "Talk to every named crew member to learn who's aboard.",
                "talked_to": [],
                "total_npcs": total_npcs,
                "complete": false,
                "reward": {
                    "xp": 200,
                    "reputation": 10,
                    "message": "You know the crew now. They know you.",
                },
            })),
            "meet_the_crew" => Some(serde_json::json!({
                "id": "survey_storage",
                "title": "Survey the storage",
                "description": "Inspect every storage container aboard the ship — lockers, cabinets, and bins.",
                "scanned": [],
                "total": total_storage,
                "complete": false,
                "reward": {
                    "xp": 300,
                    "reputation": 15,
                    "message": "Inventory's tracked. Chief Tan would approve.",
                },
            })),
            // survey_storage is the current end of the starter chain. Future
            // quests can be added here without touching the handlers.
            _ => None,
        };
        if let Some(ref q) = next {
            entity.components["current_quest"] = q.clone();
        }
        next
    }

    /// Total number of named crew NPCs (entities with both a `name` and a
    /// `dialog` field). Drives the meet_the_crew quest goal.
    fn crew_npc_count(&self) -> usize {
        self.entities.values().filter(|e| {
            e.components.get("name").is_some()
                && e.components.get("dialog").is_some()
        }).count()
    }

    /// Total number of storage entities in the world (lockers, cabinets,
    /// bins, etc. — anything with `components.storage == true`). Drives
    /// the survey_storage quest goal.
    fn storage_entity_count(&self) -> usize {
        self.entities.values().filter(|e| {
            e.components.get("storage").and_then(|v| v.as_bool()).unwrap_or(false)
        }).count()
    }

    /// Record that the player scanned a storage entity (interacted with a
    /// locker / cabinet / bin while on the survey_storage quest). Returns
    /// Some(progress) on a new scan, None otherwise.
    pub fn record_storage_scan(
        &mut self,
        player_id: u64,
        entity_id: u64,
    ) -> Option<QuestProgress> {
        // Verify the target is actually storage before doing anything.
        let is_storage = self.entities.get(&entity_id)
            .and_then(|e| e.components.get("storage"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !is_storage { return None; }

        let entity = self.entities.get_mut(&player_id)?;
        let quest = entity.components.get_mut("current_quest")?;
        if quest.get("id").and_then(|v| v.as_str()) != Some("survey_storage") {
            return None;
        }
        if quest.get("complete").and_then(|v| v.as_bool()).unwrap_or(false) {
            return None;
        }
        let scanned = quest.get_mut("scanned")?.as_array_mut()?;
        let id_value = serde_json::Value::Number(entity_id.into());
        if scanned.iter().any(|v| v == &id_value) {
            return None;
        }
        scanned.push(id_value);
        let scanned_count = scanned.len();
        let total = quest.get("total").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let complete = total > 0 && scanned_count >= total;
        if complete {
            quest["complete"] = serde_json::Value::Bool(true);
        }
        Some(QuestProgress {
            quest_id: "survey_storage".to_string(),
            step_id: entity_id.to_string(),
            room_id: entity_id.to_string(),
            visited_count: scanned_count,
            total,
            complete,
        })
    }

    /// Record that the player has spoken with an NPC. Returns Some(progress)
    /// if this NPC was new for the meet_the_crew quest, None otherwise.
    /// Auto-completes the quest when all NPCs have been spoken with.
    pub fn record_npc_talk(
        &mut self,
        player_id: u64,
        npc_name: &str,
    ) -> Option<QuestProgress> {
        let entity = self.entities.get_mut(&player_id)?;
        let quest = entity.components.get_mut("current_quest")?;
        if quest.get("id").and_then(|v| v.as_str()) != Some("meet_the_crew") {
            return None;
        }
        if quest.get("complete").and_then(|v| v.as_bool()).unwrap_or(false) {
            return None;
        }
        let talked = quest.get_mut("talked_to")?.as_array_mut()?;
        if talked.iter().any(|v| v.as_str() == Some(npc_name)) {
            return None;
        }
        talked.push(serde_json::Value::String(npc_name.to_string()));
        let talked_count = talked.len();
        let total = quest.get("total_npcs").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let complete = total > 0 && talked_count >= total;
        if complete {
            quest["complete"] = serde_json::Value::Bool(true);
        }
        Some(QuestProgress {
            quest_id: "meet_the_crew".to_string(),
            step_id: npc_name.to_string(),
            room_id: npc_name.to_string(), // alias for backward-compat
            visited_count: talked_count,
            total,
            complete,
        })
    }

    /// Apply the reward block from a player's just-completed current_quest:
    /// adds xp + reputation to the player's stats, appends the quest id to
    /// `completed_quests`, and returns the QuestReward (with running totals).
    /// Returns None if the player has no current_quest or it isn't complete.
    pub fn apply_quest_reward(&mut self, player_id: u64) -> Option<QuestReward> {
        let entity = self.entities.get_mut(&player_id)?;

        // Snapshot the quest reward before mutating anything else.
        let (quest_id, xp, rep, message) = {
            let quest = entity.components.get("current_quest")?;
            if !quest.get("complete").and_then(|v| v.as_bool()).unwrap_or(false) {
                return None;
            }
            let quest_id = quest.get("id")?.as_str()?.to_string();
            let reward = quest.get("reward")?;
            let xp = reward.get("xp").and_then(|v| v.as_u64()).unwrap_or(0);
            let rep = reward.get("reputation").and_then(|v| v.as_u64()).unwrap_or(0);
            let message = reward.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string();
            (quest_id, xp, rep, message)
        };

        // Apply to player stats.
        let cur_xp = entity.components.get("xp").and_then(|v| v.as_u64()).unwrap_or(0);
        let cur_rep = entity.components.get("reputation").and_then(|v| v.as_u64()).unwrap_or(0);
        let xp_total = cur_xp + xp;
        let reputation_total = cur_rep + rep;
        entity.components["xp"] = serde_json::json!(xp_total);
        entity.components["reputation"] = serde_json::json!(reputation_total);

        // Track completion. completed_quests is a Vec<String>.
        if let Some(list) = entity.components.get_mut("completed_quests")
            .and_then(|v| v.as_array_mut())
        {
            if !list.iter().any(|v| v.as_str() == Some(quest_id.as_str())) {
                list.push(serde_json::Value::String(quest_id.clone()));
            }
        }

        Some(QuestReward {
            quest_id,
            xp,
            reputation: rep,
            message,
            xp_total,
            reputation_total,
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

    /// Advance the game simulation by dt seconds.
    ///
    /// Crew NPCs (`chore_agent` entities) run a real task loop when the chore
    /// catalog is loaded: pick the next chore in their role's deterministic
    /// rotation, walk straight-line to its room, dwell there "working" for
    /// the chore's duration, then rotate to the next one. State transitions
    /// (plus throttled travel positions) are returned as events for the relay
    /// to broadcast as `game_npc_update`, so clients can show crew actually
    /// completing tasks instead of an endless wander loop.
    ///
    /// Entities with only a `wander` block (or when no chores loaded) keep
    /// the legacy Brownian drift within bounds.
    pub fn tick(&mut self, dt: f64) -> Vec<NpcChoreEvent> {
        self.game_time += dt;
        let mut events: Vec<NpcChoreEvent> = Vec::new();

        // Throttle traveling-position broadcasts to NPC_POSITION_BROADCAST_INTERVAL.
        self.npc_broadcast_accum += dt;
        let emit_travel_positions = self.npc_broadcast_accum >= NPC_POSITION_BROADCAST_INTERVAL;
        if emit_travel_positions {
            self.npc_broadcast_accum = 0.0;
        }

        // Random number generator scoped to this tick (wander fallback only;
        // the chore loop is fully deterministic).
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let dt_f = dt as f32;

        let entity_ids: Vec<u64> = self.entities.keys().copied().collect();
        for id in entity_ids {
            let Some(entity) = self.entities.get(&id) else { continue };

            // Chore-driven crew take the task loop; everything else falls
            // through to the legacy wander drift.
            let is_chore_agent = entity.components.get("chore_agent")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if is_chore_agent && !self.chores.is_empty() {
                self.tick_chore_agent(id, dt_f, emit_travel_positions, &mut events);
                continue;
            }

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
        events
    }

    /// The chore-site position for a room: its center at standing height.
    fn chore_site(&self, room_id: &str) -> Option<[f32; 3]> {
        self.rooms.iter().find(|r| r.id == room_id).map(|r| [
            r.position[0] + r.size[0] / 2.0,
            r.position[1] + 1.0,
            r.position[2] + r.size[2] / 2.0,
        ])
    }

    /// One simulation step for a single chore-driven crew NPC. State machine:
    ///
    ///   (no chore) --assign--> traveling --arrive--> working --timer--> done
    ///        ^                                                            |
    ///        +------------------------------------------------------------+
    ///
    /// Chore state lives in the entity's components JSON (`chore` block +
    /// `chores_done` counter + flat `activity` label) so it is included in
    /// world snapshots (AI perception + persistence) automatically.
    fn tick_chore_agent(
        &mut self,
        id: u64,
        dt: f32,
        emit_travel_positions: bool,
        events: &mut Vec<NpcChoreEvent>,
    ) {
        // Snapshot what we need before borrowing mutably.
        let (name, role, npc_seq, chores_done, chore_block) = {
            let Some(e) = self.entities.get(&id) else { return };
            (
                e.components.get("name").and_then(|v| v.as_str()).unwrap_or("Crew").to_string(),
                e.components.get("role").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                e.components.get("npc_seq").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                e.components.get("chores_done").and_then(|v| v.as_u64()).unwrap_or(0),
                e.components.get("chore").cloned(),
            )
        };

        match chore_block {
            // ── No current chore: assign the next one in this crew's rotation ──
            None => {
                let allowed = allowed_chore_indices(&self.chores, &role);
                let Some(slot) = next_chore_index(npc_seq, chores_done, allowed.len()) else {
                    return; // role has no allowed chores -> stays idle (wander won't run; acceptable)
                };
                let def = self.chores[allowed[slot]].clone();
                let Some(target) = self.chore_site(&def.room_id) else {
                    // load_chores validates room ids, so this is unreachable in
                    // practice; skip the entry defensively rather than loop on it.
                    if let Some(e) = self.entities.get_mut(&id) {
                        e.components["chores_done"] = serde_json::json!(chores_done + 1);
                    }
                    return;
                };
                let game_time = self.game_time;
                let Some(e) = self.entities.get_mut(&id) else { return };
                e.components["chore"] = serde_json::json!({
                    "id": def.id,
                    "label": def.label,
                    "room_id": def.room_id,
                    "state": "traveling",
                    "target": target,
                    "remaining": def.duration_secs,
                });
                e.components["activity"] = serde_json::json!(def.label);
                e.last_update = game_time;
                events.push(NpcChoreEvent {
                    entity_id: id,
                    name,
                    position: e.position,
                    chore_id: def.id,
                    chore_label: def.label,
                    chore_state: "traveling".to_string(),
                    room_id: def.room_id,
                });
            }
            // ── Has a chore: travel to it, then dwell "working" ──
            Some(c) => {
                let chore_id = c.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let label = c.get("label").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let room_id = c.get("room_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let state = c.get("state").and_then(|v| v.as_str()).unwrap_or("traveling").to_string();
                let game_time = self.game_time;

                if state == "traveling" {
                    let target: [f32; 3] = c.get("target")
                        .and_then(|t| {
                            let a = t.as_array()?;
                            if a.len() != 3 { return None; }
                            Some([
                                a[0].as_f64()? as f32,
                                a[1].as_f64()? as f32,
                                a[2].as_f64()? as f32,
                            ])
                        })
                        .unwrap_or_else(|| self.entities.get(&id).map(|e| e.position).unwrap_or([0.0; 3]));
                    let Some(e) = self.entities.get_mut(&id) else { return };
                    let (new_pos, arrived) = step_toward(e.position, target, CHORE_WALK_SPEED, dt);
                    e.position = new_pos;
                    e.last_update = game_time;
                    if arrived {
                        e.components["chore"]["state"] = serde_json::json!("working");
                        events.push(NpcChoreEvent {
                            entity_id: id,
                            name,
                            position: new_pos,
                            chore_id,
                            chore_label: label,
                            chore_state: "working".to_string(),
                            room_id,
                        });
                    } else if emit_travel_positions {
                        events.push(NpcChoreEvent {
                            entity_id: id,
                            name,
                            position: new_pos,
                            chore_id,
                            chore_label: label,
                            chore_state: "traveling".to_string(),
                            room_id,
                        });
                    }
                } else {
                    // "working": count the dwell timer down; complete at zero.
                    let remaining = c.get("remaining").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32 - dt;
                    let Some(e) = self.entities.get_mut(&id) else { return };
                    if remaining <= 0.0 {
                        e.components["chores_done"] = serde_json::json!(chores_done + 1);
                        if let Some(obj) = e.components.as_object_mut() {
                            obj.remove("chore");
                        }
                        e.components["activity"] = serde_json::json!("Between tasks");
                        e.last_update = game_time;
                        events.push(NpcChoreEvent {
                            entity_id: id,
                            name,
                            position: e.position,
                            chore_id,
                            chore_label: label,
                            chore_state: "completed".to_string(),
                            room_id,
                        });
                    } else {
                        e.components["chore"]["remaining"] = serde_json::json!(remaining);
                        e.last_update = game_time;
                    }
                }
            }
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
    //
    // The live world is in-memory; these methods make it durable across relay
    // restarts via two dedicated SQLite tables (see
    // storage/game_persistence.rs):
    //   * game_world_snapshots — the whole entity set + game_time + next id.
    //   * player_progress       — per-player quest/XP/reputation.
    // Static-ship fields (rooms, ship_name) are NEVER persisted; they reload
    // from data/ships/*.ron on every boot so layout edits always propagate.

    /// `world_id` used for the single persisted world snapshot row.
    ///
    /// The version suffix is load-bearing: bump it whenever entity *spawn*
    /// logic changes (new ambient NPCs, new equipment, etc.). On the next boot
    /// the old `world_id` row is simply not found, so the relay rebuilds the
    /// world fresh from RON instead of restoring a stale snapshot that would
    /// shadow the newly-added entities. Player *progress* is keyed separately
    /// (by pubkey) and is NOT discarded by a world version bump — returning
    /// players keep their XP/quests even when the shared world is rebuilt.
    pub const PERSIST_KEY: &'static str = "game_world_snapshot_v9";

    /// Save the world to the `game_world_snapshots` table as a JSON blob.
    /// Called periodically from the relay tick loop (and on graceful shutdown).
    /// Static-ship fields (rooms, ship_name) are NOT saved — they reload from
    /// RON on every startup.
    pub fn save_to_db(&self, db: &crate::relay::storage::Storage) -> Result<(), String> {
        // Serialize ONLY the dynamic world state. The shape here is the
        // `snapshot_json` blob stored in game_world_snapshots; game_time and
        // next_entity_id are also passed as their own columns for cheap
        // inspection, but the blob remains the authoritative restore source.
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
        db.save_game_world(Self::PERSIST_KEY, &json, self.game_time, self.next_entity_id)
            .map_err(|e| format!("save: {e}"))?;
        Ok(())
    }

    /// Restore world entities from the SQLite snapshot if one exists.
    /// Returns true if a snapshot was found and applied. Replaces the freshly
    /// populated ship entities with the persisted set so player movement,
    /// inventory, and quest state survive relay restarts.
    pub fn restore_from_db(&mut self, db: &crate::relay::storage::Storage) -> bool {
        #[derive(Deserialize)]
        struct Snapshot {
            entities: HashMap<u64, GameEntity>,
            next_entity_id: u64,
            game_time: f64,
        }
        let snapshot = match db.load_game_world(Self::PERSIST_KEY) {
            Ok(Some(s)) => s,
            Ok(None) => return false, // fresh relay / world version bumped → rebuild from RON
            Err(e) => {
                tracing::warn!("Could not read game_world_snapshot: {e}");
                return false;
            }
        };
        match serde_json::from_str::<Snapshot>(&snapshot.snapshot_json) {
            Ok(snap) => {
                self.entities = snap.entities;
                // Never hand out an id below either the snapshot's high-water
                // mark OR the freshly-populated world's — avoids id reuse.
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

    // ── Player progress bridge ──────────────────────────────────
    //
    // Player progression (current quest id, completed quests, xp, reputation)
    // lives inside the player entity's in-memory `components`. These two helpers
    // bridge that in-memory shape to/from the durable `player_progress` table so
    // a returning player keeps what they earned even if the world snapshot was
    // invalidated by a spawn-logic version bump.

    /// Extract a player's persistable progression from their entity, by entity
    /// id. Returns `(current_quest_id, completed_quests, xp, reputation)`, or
    /// `None` if the entity is missing. `current_quest_id` is `None` once the
    /// player has finished the whole starter chain (no `current_quest` block).
    pub fn extract_player_progress(
        &self,
        player_id: u64,
    ) -> Option<(Option<String>, Vec<String>, u64, u64)> {
        let entity = self.entities.get(&player_id)?;
        let current_quest = entity
            .components
            .get("current_quest")
            .and_then(|q| q.get("id"))
            .and_then(|v| v.as_str())
            .map(String::from);
        let completed_quests = entity
            .components
            .get("completed_quests")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let xp = entity.components.get("xp").and_then(|v| v.as_u64()).unwrap_or(0);
        let reputation = entity
            .components
            .get("reputation")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        Some((current_quest, completed_quests, xp, reputation))
    }

    /// Seed a freshly-spawned player entity with previously-persisted
    /// progression: restores xp, reputation, and the completed-quests list, and
    /// fast-forwards the quest chain so the player resumes on the quest they
    /// were last on (rather than restarting at explore_ship).
    ///
    /// `spawn_player` always grants the explore_ship starter quest; this is
    /// called immediately after, on `game_join`, ONLY when the DB had a saved
    /// row for the player. Returns true if anything was applied.
    pub fn seed_player_progress(
        &mut self,
        player_id: u64,
        current_quest: Option<&str>,
        completed_quests: &[String],
        xp: u64,
        reputation: u64,
    ) -> bool {
        // Restore the flat stats + completed list on the entity.
        {
            let Some(entity) = self.entities.get_mut(&player_id) else { return false };
            entity.components["xp"] = serde_json::json!(xp);
            entity.components["reputation"] = serde_json::json!(reputation);
            entity.components["completed_quests"] =
                serde_json::Value::Array(
                    completed_quests
                        .iter()
                        .map(|q| serde_json::Value::String(q.clone()))
                        .collect(),
                );
        }

        // Fast-forward the quest chain to the saved current quest. We walk the
        // chain by repeatedly marking the current quest complete and chaining,
        // WITHOUT re-applying rewards (apply_quest_reward is intentionally not
        // called — the xp/reputation we just restored already include past
        // rewards). This reuses the single source of truth for quest shapes
        // (chain_next_quest) instead of duplicating quest JSON here.
        if let Some(target) = current_quest {
            // explore_ship is what spawn_player granted; if that's the saved
            // quest there's nothing to advance.
            // Bounded loop: the starter chain is 3 long; cap iterations so a
            // future cycle in the chain can never hang the join handler.
            for _ in 0..8 {
                let cur = self
                    .entities
                    .get(&player_id)
                    .and_then(|e| e.components.get("current_quest"))
                    .and_then(|q| q.get("id"))
                    .and_then(|v| v.as_str())
                    .map(String::from);
                match cur {
                    Some(ref id) if id == target => break, // arrived
                    Some(_) => {
                        // Mark current complete so chain_next_quest advances,
                        // then chain. If chaining yields nothing (end of chain)
                        // we stop regardless.
                        if let Some(e) = self.entities.get_mut(&player_id) {
                            if let Some(q) = e.components.get_mut("current_quest") {
                                q["complete"] = serde_json::Value::Bool(true);
                            }
                        }
                        if self.chain_next_quest(player_id).is_none() {
                            break;
                        }
                    }
                    None => break, // no current quest at all
                }
            }
        }
        true
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

    // ── Persistence integration (save → restore through SQLite) ──

    /// Throwaway on-disk DB with the full relay schema (mirrors the storage
    /// modules' per-test helper).
    fn make_test_storage() -> crate::relay::storage::Storage {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_gameworld_test_{pid}_{nanos}.db"));
        crate::relay::storage::Storage::open(&path).expect("open test db")
    }

    /// save_to_db → restore_from_db must reconstruct the dynamic world:
    /// the entity set, game_time, and next_entity_id all survive a round-trip
    /// through SQLite (simulating a relay restart). A spawned player's moved
    /// position must be preserved too.
    #[test]
    fn world_save_restore_preserves_entities_time_and_next_id() {
        let db = make_test_storage();

        // Build a world, spawn a player, advance the clock, move the player.
        let mut world = GameWorld::new();
        let player_id = world.spawn_player("pk_persist", [0.0, 1.0, 0.0]);
        world.tick(5.0); // advance game_time
        world.update_position(player_id, [12.0, 1.0, 34.0], [0.0, 0.0, 0.0, 1.0]);
        let saved_entity_count = world.entities.len();
        let saved_game_time = world.game_time;
        let saved_next_id = world.next_entity_id;

        // Persist it.
        world.save_to_db(&db).expect("save_to_db");

        // A FRESH world (as if the relay restarted) restores from the snapshot.
        let mut restored = GameWorld::new();
        let did_restore = restored.restore_from_db(&db);
        assert!(did_restore, "restore_from_db should find the saved snapshot");

        assert_eq!(restored.entities.len(), saved_entity_count, "entity count preserved");
        assert_eq!(restored.game_time, saved_game_time, "game_time preserved");
        assert!(restored.next_entity_id >= saved_next_id, "next_entity_id not regressed");

        // The player's moved position survived.
        let p = restored.entities.get(&player_id).expect("player entity restored");
        assert_eq!(p.position, [12.0, 1.0, 34.0], "player position preserved across restart");
        assert_eq!(p.owner.as_deref(), Some("pk_persist"));
    }

    /// restore_from_db on an empty DB returns false (no snapshot) and leaves
    /// the freshly-built world intact — the relay then keeps the RON-built one.
    #[test]
    fn world_restore_on_empty_db_is_a_noop() {
        let db = make_test_storage();
        let mut world = GameWorld::new();
        let before = world.entities.len();
        assert!(!world.restore_from_db(&db), "no snapshot → returns false");
        assert_eq!(world.entities.len(), before, "world untouched when nothing to restore");
    }

    /// extract_player_progress must read the progression fields off the
    /// player entity (current quest id, completed list, xp, reputation), and
    /// seed_player_progress must fast-forward a fresh player to a saved quest
    /// while restoring xp/reputation/completed — the restore-on-join flow.
    #[test]
    fn player_progress_extract_and_seed_round_trip() {
        let mut world = GameWorld::new();
        let player_id = world.spawn_player("pk_prog", [0.0, 1.0, 0.0]);

        // Fresh player: explore_ship, zeroed stats.
        let (q, completed, xp, rep) =
            world.extract_player_progress(player_id).expect("progress");
        assert_eq!(q.as_deref(), Some("explore_ship"));
        assert!(completed.is_empty());
        assert_eq!(xp, 0);
        assert_eq!(rep, 0);

        // Simulate a returning player who finished explore_ship + meet_the_crew,
        // is now on survey_storage, with earned stats.
        let saved_completed = vec!["explore_ship".to_string(), "meet_the_crew".to_string()];
        let ok = world.seed_player_progress(
            player_id,
            Some("survey_storage"),
            &saved_completed,
            300,
            15,
        );
        assert!(ok, "seed should apply");

        // The entity now reflects the restored progress.
        let (q2, completed2, xp2, rep2) =
            world.extract_player_progress(player_id).expect("progress");
        assert_eq!(q2.as_deref(), Some("survey_storage"), "fast-forwarded to saved quest");
        assert_eq!(completed2, saved_completed, "completed list restored");
        assert_eq!(xp2, 300, "xp restored");
        assert_eq!(rep2, 15, "reputation restored");
    }

    // ── Crew chore AI (v0.663) ──

    /// The rotation is deterministic: one slot forward per completed chore,
    /// wrapping, staggered by npc_seq so crew don't all start on chore 0.
    #[test]
    fn next_chore_index_rotates_deterministically() {
        assert_eq!(next_chore_index(0, 0, 0), None, "empty catalog yields no chore");
        assert_eq!(next_chore_index(0, 0, 4), Some(0));
        assert_eq!(next_chore_index(0, 1, 4), Some(1));
        assert_eq!(next_chore_index(0, 4, 4), Some(0), "wraps after a full cycle");
        assert_eq!(next_chore_index(0, 5, 4), Some(1));
        assert_eq!(next_chore_index(2, 0, 4), Some(2), "npc_seq staggers the start");
        assert_eq!(next_chore_index(3, 2, 4), Some(1));
        // Every chore in the catalog is eventually visited by any single NPC.
        let visited: std::collections::HashSet<usize> =
            (0..4).map(|done| next_chore_index(1, done, 4).unwrap()).collect();
        assert_eq!(visited.len(), 4, "a full rotation covers the whole catalog");
    }

    /// Straight-line travel covers distance at the requested speed, arrives
    /// on the expected step, and never overshoots the target.
    #[test]
    fn step_toward_travels_at_speed_and_never_overshoots() {
        let target = [3.0_f32, 0.0, 0.0];
        let mut pos = [0.0_f32, 0.0, 0.0];
        let mut steps = 0;
        loop {
            let (p, arrived) = step_toward(pos, target, 1.5, 0.5); // 0.75 m per step
            assert!(p[0] <= 3.0 + 1e-4, "must never overshoot the target");
            pos = p;
            steps += 1;
            if arrived { break; }
            assert!(steps < 100, "must converge");
        }
        assert_eq!(pos, target, "arrival snaps exactly onto the target");
        assert_eq!(steps, 4, "3.0 m at 0.75 m/step arrives on the 4th step");
        // Degenerate case: already at the target.
        let (p, arrived) = step_toward(target, target, 1.5, 0.5);
        assert!(arrived);
        assert_eq!(p, target);
    }

    /// data/npc/chores.ron parses, ids are unique, labels/durations are sane,
    /// and every referenced room resolves against the loaded ship layout.
    #[test]
    fn chores_file_parses_and_room_ids_resolve() {
        let world = GameWorld::new();
        assert!(!world.chores.is_empty(), "data/npc/chores.ron must load and validate");
        let mut seen = std::collections::HashSet::new();
        for c in &world.chores {
            assert!(seen.insert(c.id.clone()), "duplicate chore id {}", c.id);
            assert!(!c.label.trim().is_empty(), "chore {} needs a label", c.id);
            assert!(!c.label.contains('\u{2014}'), "chore {} label contains an em dash", c.id);
            assert!(c.duration_secs > 0.0, "chore {} needs a positive duration", c.id);
            assert!(
                world.rooms.iter().any(|r| r.id == c.room_id),
                "chore {} references unknown room {}", c.id, c.room_id
            );
        }
    }

    /// Every spawned crew NPC's role has at least one allowed chore (so nobody
    /// idles forever), and every role's chores span at least two rooms (so
    /// crew visibly walk across the ship between tasks -- the design goal).
    #[test]
    fn every_crew_role_has_chores_spanning_rooms() {
        let world = GameWorld::new();
        let agents: Vec<&GameEntity> = world.entities.values()
            .filter(|e| e.components.get("chore_agent").and_then(|v| v.as_bool()).unwrap_or(false))
            .collect();
        assert!(!agents.is_empty(), "expected crew chore agents in the world");
        for e in &agents {
            let role = e.components.get("role").and_then(|v| v.as_str()).unwrap_or("");
            let allowed = allowed_chore_indices(&world.chores, role);
            assert!(!allowed.is_empty(), "role {role} has no allowed chores");
            let rooms: std::collections::HashSet<&str> = allowed.iter()
                .map(|&i| world.chores[i].room_id.as_str())
                .collect();
            assert!(rooms.len() >= 2, "role {role} chores span only {rooms:?} -- crew should travel between rooms");
        }
    }

    /// Live loop: over 120 simulated seconds every crew NPC completes at least
    /// one chore, the event stream shows the full traveling -> working ->
    /// completed lifecycle in order, and the synced `activity` label is kept.
    #[test]
    fn crew_complete_chores_and_rotate() {
        let mut world = GameWorld::new();
        assert!(!world.chores.is_empty());
        let agent_id = *world.entities.iter()
            .find(|(_, e)| e.components.get("chore_agent").and_then(|v| v.as_bool()).unwrap_or(false))
            .map(|(id, _)| id)
            .expect("at least one crew agent");

        let mut all_events: Vec<NpcChoreEvent> = Vec::new();
        for _ in 0..2400 { // 120 s at the live 50 ms tick
            all_events.extend(world.tick(0.05));
        }

        for (id, e) in &world.entities {
            let is_agent = e.components.get("chore_agent").and_then(|v| v.as_bool()).unwrap_or(false);
            if !is_agent { continue; }
            let done = e.components.get("chores_done").and_then(|v| v.as_u64()).unwrap_or(0);
            assert!(done >= 1, "crew entity {id} completed no chores in 120 s");
            let activity = e.components.get("activity").and_then(|v| v.as_str()).unwrap_or("");
            assert!(!activity.is_empty(), "crew entity {id} lost its activity label");
            // While a chore is in flight, the flat activity label mirrors it.
            if let Some(chore) = e.components.get("chore") {
                assert_eq!(
                    chore.get("label").and_then(|v| v.as_str()),
                    Some(activity),
                    "activity label must match the current chore"
                );
            }
        }

        let states: Vec<&str> = all_events.iter()
            .filter(|ev| ev.entity_id == agent_id)
            .map(|ev| ev.chore_state.as_str())
            .collect();
        let first_travel = states.iter().position(|s| *s == "traveling");
        let first_work = states.iter().position(|s| *s == "working");
        let first_done = states.iter().position(|s| *s == "completed");
        assert!(first_travel.is_some(), "agent never broadcast traveling");
        assert!(first_work.is_some(), "agent never broadcast working");
        assert!(first_done.is_some(), "agent never broadcast completed");
        assert!(first_travel < first_work, "traveling must precede working");
        assert!(first_work < first_done, "working must precede completed");
    }

    /// The dwell at a chore site matches the chore's declared duration_secs
    /// (within one tick of quantization).
    #[test]
    fn working_dwell_matches_chore_duration() {
        let mut world = GameWorld::new();
        assert!(!world.chores.is_empty());
        let mut working_at: Option<(u64, String, f64)> = None;
        let mut completed_at: Option<f64> = None;
        'sim: for _ in 0..20_000 { // up to 1000 simulated seconds
            for ev in world.tick(0.05) {
                match (&working_at, ev.chore_state.as_str()) {
                    (None, "working") => {
                        working_at = Some((ev.entity_id, ev.chore_id.clone(), world.game_time));
                    }
                    (Some((id, _, _)), "completed") if ev.entity_id == *id => {
                        completed_at = Some(world.game_time);
                        break 'sim;
                    }
                    _ => {}
                }
            }
        }
        let (_, chore_id, started) = working_at.expect("an NPC reached working");
        let finished = completed_at.expect("the same NPC completed its chore");
        let duration = world.chores.iter()
            .find(|c| c.id == chore_id)
            .map(|c| c.duration_secs as f64)
            .expect("chore def exists");
        let dwell = finished - started;
        assert!(
            dwell >= duration - 1e-6 && dwell <= duration + 0.15,
            "dwell {dwell:.2}s should match duration {duration:.2}s within one tick"
        );
    }

    /// End-to-end via SQLite: a player's progress saved to player_progress and
    /// re-applied to a fresh player entity (the exact game_join restore path)
    /// keeps them on the right quest with the right stats.
    #[test]
    fn player_progress_persists_and_restores_via_db() {
        let db = make_test_storage();

        // Save a returning player's progress through the storage layer.
        let completed = vec!["explore_ship".to_string()];
        db.save_player_progress("pk_join", Some("meet_the_crew"), &completed, 100, 5)
            .expect("save_player_progress");

        // New world + fresh spawn (relay restarted; player rejoins).
        let mut world = GameWorld::new();
        let player_id = world.spawn_player("pk_join", [0.0, 1.0, 0.0]);

        // Load + seed, exactly like handle_game_join does.
        let loaded = db.load_player_progress("pk_join").unwrap().expect("row present");
        world.seed_player_progress(
            player_id,
            loaded.current_quest.as_deref(),
            &loaded.completed_quests,
            loaded.xp,
            loaded.reputation,
        );

        let (q, completed_out, xp, rep) =
            world.extract_player_progress(player_id).expect("progress");
        assert_eq!(q.as_deref(), Some("meet_the_crew"), "resumed on saved quest");
        assert_eq!(completed_out, completed);
        assert_eq!(xp, 100);
        assert_eq!(rep, 5);
    }
}
