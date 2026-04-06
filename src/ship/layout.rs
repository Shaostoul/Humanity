//! Ship layout — data-driven definitions loaded from RON files.
//!
//! Each ship has decks containing rooms connected by doors. The `ShipLayout`
//! wrapper provides queries (room lookup, pathfinding) over a parsed `ShipDef`.

use glam::Vec3;
use serde::Deserialize;
use std::collections::{HashMap, VecDeque};

/// Cardinal + vertical directions for door placement.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    North,
    South,
    East,
    West,
    Up,
    Down,
}

/// Room purpose — determines default furnishing and systems.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RoomType {
    Bridge,
    Quarters,
    Cargo,
    Engineering,
    Medbay,
    Hydroponics,
    Armory,
    Hangar,
}

/// Ship size class — affects available tonnage, crew cap, and jump range.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ShipClass {
    Frigate,
    Cruiser,
    Carrier,
    Station,
}

/// A door connecting two rooms.
#[derive(Clone, Debug, Deserialize)]
pub struct DoorDef {
    /// Room id this door leads to.
    pub connects_to: String,
    /// Wall the door is placed on.
    pub direction: Direction,
}

/// A single room within a deck.
#[derive(Clone, Debug, Deserialize)]
pub struct RoomDef {
    /// Unique identifier within the ship (e.g. "bridge", "cargo_1").
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Functional category.
    pub room_type: RoomType,
    /// Position of room center relative to deck origin (meters).
    pub position: Vec3,
    /// Room dimensions in meters (width, height, depth).
    pub size: Vec3,
    /// Doors leading out of this room.
    pub doors: Vec<DoorDef>,
}

/// One horizontal deck of the ship.
#[derive(Clone, Debug, Deserialize)]
pub struct DeckDef {
    /// Deck number (0 = lowest, ascending upward).
    pub deck_index: i32,
    /// Display name (e.g. "Main Deck", "Lower Engineering").
    pub name: String,
    /// Rooms on this deck.
    pub rooms: Vec<RoomDef>,
}

/// Top-level ship definition deserialized from a RON file.
#[derive(Clone, Debug, Deserialize)]
pub struct ShipDef {
    pub name: String,
    pub class: ShipClass,
    /// Overall hull dimensions in meters.
    pub length: f32,
    pub width: f32,
    pub height: f32,
    /// Decks from bottom to top.
    pub decks: Vec<DeckDef>,
}

/// Runtime wrapper providing queries over a parsed `ShipDef`.
pub struct ShipLayout {
    pub def: ShipDef,
    /// room_id -> (deck_index, index within deck's room vec)
    room_index: HashMap<String, (usize, usize)>,
}

impl ShipLayout {
    /// Build the lookup index from a parsed definition.
    pub fn new(def: ShipDef) -> Self {
        let mut room_index = HashMap::new();
        for (di, deck) in def.decks.iter().enumerate() {
            for (ri, room) in deck.rooms.iter().enumerate() {
                room_index.insert(room.id.clone(), (di, ri));
            }
        }
        Self { def, room_index }
    }

    /// Look up a room by id.
    pub fn room(&self, id: &str) -> Option<&RoomDef> {
        let &(di, ri) = self.room_index.get(id)?;
        Some(&self.def.decks[di].rooms[ri])
    }

    /// All rooms across every deck.
    pub fn all_rooms(&self) -> Vec<&RoomDef> {
        self.def
            .decks
            .iter()
            .flat_map(|d| d.rooms.iter())
            .collect()
    }

    /// BFS shortest path between two rooms (by door connections).
    /// Returns the ordered list of room ids from `start` to `end` inclusive,
    /// or `None` if no path exists.
    pub fn find_path(&self, start: &str, end: &str) -> Option<Vec<String>> {
        if start == end {
            return Some(vec![start.to_string()]);
        }
        if !self.room_index.contains_key(start) || !self.room_index.contains_key(end) {
            return None;
        }

        let mut visited: HashMap<String, String> = HashMap::new();
        let mut queue: VecDeque<String> = VecDeque::new();
        queue.push_back(start.to_string());
        visited.insert(start.to_string(), String::new());

        while let Some(current) = queue.pop_front() {
            if let Some(room) = self.room(&current) {
                for door in &room.doors {
                    if visited.contains_key(&door.connects_to) {
                        continue;
                    }
                    visited.insert(door.connects_to.clone(), current.clone());
                    if door.connects_to == end {
                        // Reconstruct path
                        let mut path = vec![end.to_string()];
                        let mut at = end.to_string();
                        while at != start {
                            at = visited[&at].clone();
                            path.push(at.clone());
                        }
                        path.reverse();
                        return Some(path);
                    }
                    queue.push_back(door.connects_to.clone());
                }
            }
        }
        None
    }

    /// World-space position of a door opening on a room's wall.
    /// Returns the center of the wall face offset toward the door direction.
    pub fn door_position(room: &RoomDef, door: &DoorDef) -> Vec3 {
        let half = room.size * 0.5;
        let offset = match door.direction {
            Direction::North => Vec3::new(0.0, 0.0, -half.z),
            Direction::South => Vec3::new(0.0, 0.0, half.z),
            Direction::East => Vec3::new(half.x, 0.0, 0.0),
            Direction::West => Vec3::new(-half.x, 0.0, 0.0),
            Direction::Up => Vec3::new(0.0, half.y, 0.0),
            Direction::Down => Vec3::new(0.0, -half.y, 0.0),
        };
        room.position + offset
    }
}
