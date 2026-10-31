//! Quest system — data-driven quest progression loaded from RON files.
//!
//! Quest definitions live in `data/quests/*.ron` and are deserialized into `QuestDef`.
//! The `QuestSystem` checks active quest objectives each tick, advances steps when
//! objectives are met, and awards item rewards on completion.

pub mod objectives;
/// A quest's chain for the Quests page's chain window (2026-10-30).
pub mod chain;
/// The opening, played on the shipped data with the real systems (2026-10-04).
#[cfg(all(test, feature = "native"))]
mod opening_tests;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::inventory::Inventory;

pub use objectives::{QuestObjective, QuestStep};

// ── Quest definition (deserialized from RON) ────────────────

/// A complete quest definition loaded from data files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestDef {
    /// Unique quest identifier (e.g., "tutorial_first_habitat").
    pub id: String,
    /// Human-readable quest name.
    pub name: String,
    /// Longer description shown in the quest journal.
    pub description: String,
    /// Ordered list of steps the player must complete.
    pub steps: Vec<QuestStep>,
    /// Item rewards granted on quest completion: (item_id, quantity).
    pub rewards: Vec<(String, u32)>,
    /// Skill XP granted on completion: (skill_id, amount). serde-default so
    /// existing quest files load unchanged (v0.747.x, ladder rung 4 — the
    /// Quests page always promised XP; QuestDef finally carries it).
    #[serde(default)]
    pub xp_rewards: Vec<(String, u32)>,
    /// Quest ID that must be completed before this quest can be accepted.
    pub prerequisite: Option<String>,
}

/// Registry of all quest definitions, keyed by quest ID.
/// Stored in DataStore under key "quest_registry".
#[derive(Debug, Clone, Default)]
pub struct QuestRegistry {
    pub quests: HashMap<String, QuestDef>,
}

impl QuestRegistry {
    pub fn get(&self, id: &str) -> Option<&QuestDef> {
        self.quests.get(id)
    }

    /// The game's quests, as `DataStore["quest_registry"]` holds them (BUG-163).
    /// Each quest file the game ships comes from `data_dir`'s `quests/` when this
    /// version can read it, else from the copy built into the game, and the log
    /// says which file and why (`embedded_data::load_data_or_embedded`, the rule
    /// every registry loads by). Then every other `.ron` file in `quests/`, a
    /// modder's own, is merged in name order, and one that does not parse is
    /// skipped with a warning. A later file's quest replaces an earlier one of
    /// the same id.
    pub fn load(data_dir: &std::path::Path) -> Self {
        let shipped: Vec<&str> = crate::embedded_data::EMBEDDED_KEYS
            .iter()
            .filter_map(|k| k.strip_prefix("quests/"))
            .collect();
        let mut quests = HashMap::new();
        for name in &shipped {
            let rel = format!("quests/{name}");
            match crate::embedded_data::load_data_or_embedded(data_dir, &rel, crate::assets::loader::parse_ron::<Vec<QuestDef>>) {
                Ok(defs) => quests.extend(defs.into_iter().map(|d| (d.id.clone(), d))),
                Err(e) => log::warn!("{e}; its quests are missing"),
            }
        }
        let mut own: Vec<std::path::PathBuf> = std::fs::read_dir(data_dir.join("quests"))
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|p| p.extension().is_some_and(|x| x == "ron"))
            .filter(|p| p.file_name().and_then(|n| n.to_str()).map_or(true, |n| !shipped.contains(&n)))
            .collect();
        own.sort();
        for path in own {
            match std::fs::read(&path)
                .map_err(|e| e.to_string())
                .and_then(|bytes| crate::assets::loader::parse_ron::<Vec<QuestDef>>(&bytes))
            {
                Ok(defs) => quests.extend(defs.into_iter().map(|d| (d.id.clone(), d))),
                Err(e) => log::warn!("Quest file {} skipped: {e}", path.display()),
            }
        }
        log::info!("Loaded {} quest definitions", quests.len());
        Self { quests }
    }

    /// Load every `*.ron` quest file in a directory (each a `Vec<QuestDef>`) and
    /// merge them into one registry. Data-driven (infinite-of-X): drop a new
    /// `.ron` into `data/quests/` and its quests appear. Malformed or unreadable
    /// files are logged + skipped — never panics. Exactly the files in one
    /// folder, for tests that read the shipped quests; the game loads through
    /// `load`, which also falls back to the built-in copy of a shipped quest
    /// file this version cannot read (BUG-163).
    pub fn from_ron_dir(dir: &std::path::Path) -> Self {
        let mut quests = HashMap::new();
        match std::fs::read_dir(dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map(|x| x == "ron").unwrap_or(false) {
                        match std::fs::read_to_string(&path) {
                            Ok(text) => match ron::from_str::<Vec<QuestDef>>(&text) {
                                Ok(defs) => {
                                    for def in defs {
                                        quests.insert(def.id.clone(), def);
                                    }
                                }
                                Err(e) => {
                                    log::warn!("Quest file {} parse error: {e}", path.display())
                                }
                            },
                            Err(e) => {
                                log::warn!("Quest file {} read error: {e}", path.display())
                            }
                        }
                    }
                }
            }
            Err(e) => log::warn!("Quests dir {} unreadable: {e}", dir.display()),
        }
        log::info!("Loaded {} quest definitions", quests.len());
        Self { quests }
    }
}

// ── Travel destinations (v0.979; where the player stands, 2026-10-04; the
//    ship's shared places, 2026-10-12) ──────

/// One named world destination a Travel objective can point at. A fixed place's
/// `pos` is XZ in the homestead-world frame (the same frame
/// `entities/wild_spawns.ron` uses), so a destination can mark a spawn cluster,
/// a field, or any landmark: it has no height, and is reached from any height
/// over it. A destination that is no fixed place (the player's own front door,
/// the nearest mess hall) names what it resolves to in `at` instead
/// (2026-10-04); those places have a floor, and are reached only on its storey
/// (ship homes increment 7, `DestinationDef::distance_from`).
#[derive(Debug, Clone, Deserialize)]
pub struct DestinationDef {
    /// The id Travel(destination: ...) references.
    pub id: String,
    /// Player-facing name (quest journal copy can reference it).
    pub label: String,
    /// World XZ centre. Unused when `at` names where the place is.
    #[serde(default)]
    pub pos: (f32, f32),
    /// How near counts as being there, metres: within this radius of `pos`
    /// or of a doorstep; for a volume aboard (`NearestVolume`), within this
    /// far of its floor, so anywhere on the floor counts and so does its edge.
    pub radius: f32,
    /// Where the place is when it is not a fixed point: resolved every tick
    /// from what the game publishes (`DestinationAnchor`). None: `pos`.
    #[serde(default)]
    pub at: Option<DestinationAnchor>,
}

/// A destination that moves with the player's own circumstances (2026-10-04,
/// the opening): never a fixed coordinate.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub enum DestinationAnchor {
    /// The doorstep of the player's OWN front door: `out_m` metres out of the
    /// door, along the corridor its plot's door makes. A family home's door
    /// opens on the Commons, a First Street plot's on the street; whichever
    /// plot the player was given, offline or on a server, the game publishes
    /// its door every frame (`FrontDoor`, `publish_front_door`). A guest,
    /// whose home is put away, has no front door aboard, and the place does
    /// not resolve.
    OwnFrontDoor { out_m: f32 },
    /// The ship's labelled volume of zone type `zone_type`
    /// (data/blueprints/zone_types.ron: a row of a zone body's `zones` in the
    /// ship file, like the Commons' "mess-hall") NEAREST the player's own
    /// front door, its whole floor (2026-10-12, ship homes increment 6, the
    /// Cabin's opening). So each home is sent to the mess hall that serves it:
    /// the cabin row by First Street's north end to the Commons', a homestead
    /// far down the street to the South Commons'. A player with no front door
    /// aboard (a guest) is sent to the one nearest where they stand. Never a
    /// room of the player's own home: these are the ship's shared places
    /// (`ShipPlaces`). With none of that type aboard, it does not resolve.
    ///
    /// Nearest as the crow flies, to the edge of the floor. On this ship every
    /// home reaches every shared place along First Street, so the nearest by
    /// line is the nearest on foot; a ship where the two differ needs the walk
    /// along its corridors here instead.
    ///
    /// ON THE PLAYER'S STOREY (ship homes increment 7): the distance is
    /// measured on the storey of the point it is measured from (the front
    /// door's floor, else the player's feet). Of the volumes of the type, the
    /// nearest of those on that storey is taken (`ShipVolume::on_its_storey`);
    /// only when none is on it, the nearest across the floor on any storey. And
    /// a volume is reached only standing on its storey: one on the floor above
    /// is never reached from under it, however near across the floor.
    NearestVolume { zone_type: String },
}

impl DestinationDef {
    /// The volume aboard this place is now, for a `NearestVolume` place: the
    /// nearest of its type to the player's own front door (on the door's
    /// floor), or to the player (`player`, where their eyes are, ship metres)
    /// when there is none, measured on that storey (`ShipPlaces::nearest`).
    /// None for any other kind of place, and when no volume of the type is
    /// aboard.
    pub fn volume<'a>(&self, aboard: Aboard<'a>, player: glam::Vec3) -> Option<&'a ShipVolume> {
        match &self.at {
            Some(DestinationAnchor::NearestVolume { zone_type }) => {
                let feet = glam::Vec3::new(player.x, crate::ship::moves::feet_y(player.y), player.z);
                let from = aboard.front_door.map_or(feet, |d| glam::Vec3::new(d.door.0, d.floor, d.door.1));
                aboard.places?.nearest(zone_type, from)
            }
            _ => None,
        }
    }

    /// How far the player (`player`, where their eyes are, ship metres) stands
    /// from this place now, metres across the floor: from `pos` or the
    /// doorstep, or from the edge of a volume's floor (0 on it). A place with a
    /// floor (the front door, a volume) is measured only on its storey (ship
    /// homes increment 7: the feet from half a metre under its floor to half a
    /// metre under its top, src/ship/moves.rs `stands_in_storey_of`): from any
    /// other storey it is infinitely far, so a place on the floor above is
    /// never reached by standing under it. A fixed place (`pos`) has no height.
    /// None when the place does not resolve (a guest's front door; a kind of
    /// room the ship does not have), so nobody stands in it.
    pub fn distance_from(&self, player: glam::Vec3, aboard: Aboard<'_>) -> Option<f32> {
        let to = |c: (f32, f32)| (player.x - c.0).hypot(player.z - c.1);
        let feet = crate::ship::moves::feet_y(player.y);
        let on_storey = |d: f32, there: bool| if there { d } else { f32::INFINITY };
        match &self.at {
            None => Some(to(self.pos)),
            Some(DestinationAnchor::OwnFrontDoor { out_m }) => aboard.front_door.map(|d| on_storey(to(d.doorstep(*out_m)), d.on_its_storey(feet))),
            Some(DestinationAnchor::NearestVolume { .. }) => self.volume(aboard, player).map(|v| on_storey(v.distance_from(player), v.on_its_storey(feet))),
        }
    }
}

/// What the places that are no fixed points resolve against: the player's own
/// front door and the ship's labelled volumes, as the game publishes them
/// every frame (`publish_front_door`). A guest has no front door aboard; a
/// headless world or a test may publish neither.
#[derive(Debug, Clone, Copy, Default)]
pub struct Aboard<'a> {
    pub front_door: Option<&'a FrontDoor>,
    pub places: Option<&'a ShipPlaces>,
}

impl<'a> Aboard<'a> {
    /// What `data` holds of them.
    pub fn of(data: &'a DataStore) -> Aboard<'a> {
        Aboard { front_door: data.get::<FrontDoor>(FRONT_DOOR_KEY), places: data.get::<ShipPlaces>(SHIP_PLACES_KEY) }
    }
}

/// The destination list. DataStore: `"quest_destinations"`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DestinationList {
    pub destinations: Vec<DestinationDef>,
}

impl DestinationList {
    pub fn from_ron(bytes: &[u8]) -> Result<Self, String> {
        let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
        ron::from_str(text).map_err(|e| e.to_string())
    }
}

/// The ids of the destinations the player stands in now, at `player` (where
/// their eyes are, ship metres; `player_position`) (pure, for tests). A place
/// with a floor counts only on its storey (`DestinationDef::distance_from`).
/// A Travel step is done while the player stands in its
/// place and it is the step they are on (2026-10-04). It used to be an event
/// fired on ENTERING the place and counted from the moment the quest was
/// accepted, so the opening's last step, out of your own front door, would
/// already have been done by anyone who looked out of the door at the start,
/// and would have finished the moment the step before it did, wherever the
/// player was.
pub fn destinations_here(
    player: glam::Vec3,
    dests: &DestinationList,
    aboard: Aboard<'_>,
) -> std::collections::HashSet<String> {
    dests
        .destinations
        .iter()
        .filter(|d| d.distance_from(player, aboard).is_some_and(|m| m <= d.radius))
        .map(|d| d.id.clone())
        .collect()
}

// ── The ship's shared places (2026-10-12, ship homes increment 6) ───────

/// One labelled volume of the ship as the quests see it: a row of a zone
/// body's `zones` in the ship file (the Commons' "mess-hall"), with its floor
/// and its height in ship metres, the frame "camera_position" is in. It keeps
/// its height since ship homes increment 7: a place on an upper storey is
/// reached only on that storey (`on_its_storey`).
#[derive(Debug, Clone, PartialEq)]
pub struct ShipVolume {
    /// The volume's id, also a room id of the relay's world ("mess-hall").
    pub id: String,
    /// Its zone type (data/blueprints/zone_types.ron): "mess_hall".
    pub type_id: String,
    /// Its own label ("Mess hall"); may be empty.
    pub label: String,
    /// The label of the ship zone it stands in ("The Commons"), its id when
    /// it has none.
    pub zone_label: String,
    /// The floor's corners across the floor, (x, z): the smaller x and z,
    /// then the larger.
    pub min: (f32, f32),
    pub max: (f32, f32),
    /// The floor's height, y, ship metres (the zone's origin plus the
    /// volume's own).
    pub floor: f32,
    /// How tall the volume is, metres.
    pub height: f32,
}

impl ShipVolume {
    /// How far `p` is from this floor across the floor (x and z), metres: 0
    /// anywhere over it. Its height is not looked at (`on_its_storey` does).
    pub fn distance_from(&self, p: glam::Vec3) -> f32 {
        let dx = (self.min.0 - p.x).max(p.x - self.max.0).max(0.0);
        let dz = (self.min.1 - p.z).max(p.z - self.max.1).max(0.0);
        dx.hypot(dz)
    }

    /// True when feet at height `feet_y` stand on this volume's storey (ship
    /// homes increment 7): from half a metre under its floor to half a metre
    /// under its top, or under one storey's top when it is lower than a storey
    /// (src/ship/moves.rs `stands_in_storey_of`, the rule every place with a
    /// floor uses).
    pub fn on_its_storey(&self, feet_y: f32) -> bool {
        crate::ship::moves::stands_in_storey_of(feet_y, self.floor, self.height)
    }

    /// The zone it stands in as it reads inside a sentence: "the Commons".
    pub fn zone_in_a_sentence(&self) -> String {
        in_a_sentence(&self.zone_label)
    }
}

/// The ship's labelled volumes, every zone's but the player's own home's.
/// DataStore: `SHIP_PLACES_KEY`, published by the game every frame with the
/// front door (`publish_front_door`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShipPlaces {
    pub volumes: Vec<ShipVolume>,
}

impl ShipPlaces {
    /// Every labelled volume of every zone of `ship`, in the ship file's
    /// order, but none of the player's own home (its rooms are no shared
    /// place, and a home put away stands nowhere aboard).
    pub fn of_ship(ship: &crate::ship::ship_structure::ShipStructure) -> ShipPlaces {
        use crate::ship::ship_structure::HOME_ZONE_ID;
        let volumes = ship
            .zones
            .iter()
            .filter(|z| z.id != HOME_ZONE_ID)
            .flat_map(|z| {
                let zone_label = if z.label.is_empty() { z.id.clone() } else { z.label.clone() };
                z.body.zones.iter().map(move |v| {
                    let min = (z.origin.0 + v.origin.0, z.origin.2 + v.origin.2);
                    ShipVolume {
                        id: v.id.clone(),
                        type_id: v.type_id.clone(),
                        label: v.label.clone(),
                        zone_label: zone_label.clone(),
                        min,
                        max: (min.0 + v.size.0, min.1 + v.size.2),
                        floor: z.origin.1 + v.origin.1,
                        height: v.size.1,
                    }
                })
            })
            .collect();
        ShipPlaces { volumes }
    }

    /// The volume of type `zone_type` nearest `from` (a floor point: the
    /// front door's, or the player's feet), to the edge of its floor, ON THE
    /// STOREY OF `from` (ship homes increment 7): of the volumes on that
    /// storey (`ShipVolume::on_its_storey`) the nearest across the floor, and
    /// only when none is on it the nearest across the floor on any storey; of
    /// two as near, the first in the ship file. None when the ship has none of
    /// that type.
    pub fn nearest(&self, zone_type: &str, from: glam::Vec3) -> Option<&ShipVolume> {
        self.volumes
            .iter()
            .filter(|v| v.type_id == zone_type)
            .min_by(|a, b| b.on_its_storey(from.y).cmp(&a.on_its_storey(from.y)).then(a.distance_from(from).total_cmp(&b.distance_from(from))))
    }
}

/// The DataStore key of the published `ShipPlaces`.
pub const SHIP_PLACES_KEY: &str = "ship_places";

/// A place's label as it reads inside a sentence: a leading "The" lowered
/// ("The Commons" is "the Commons"), anything else as it is ("First Street").
pub fn in_a_sentence(label: &str) -> String {
    match label.strip_prefix("The ") {
        Some(rest) => format!("the {rest}"),
        None => label.to_string(),
    }
}

// ── The player's own front door (2026-10-04, the opening) ──────────────

/// The player's own front door, as the quests see it: published by the game
/// every frame under `FRONT_DOOR_KEY` from the ship it runs, with the home on
/// whichever plot the player was given (`front_door_of`). Ship metres, the
/// frame "camera_position" is in.
#[derive(Debug, Clone, PartialEq)]
pub struct FrontDoor {
    /// The door across the floor, (x, z): where the plot's door corridor
    /// leaves the home's shell.
    pub door: (f32, f32),
    /// The door's floor, y, ship metres: the corridor's deck (ship homes
    /// increment 7: the doorstep is reached only on its storey).
    pub floor: f32,
    /// The way out, a unit XZ direction along the corridor, away from home.
    pub outward: (f32, f32),
    /// What the door opens on: the label of the zone its corridor leads to
    /// ("The Commons", "First Street").
    pub opens_on: String,
}

impl FrontDoor {
    /// The doorstep `out_m` metres out of the door, across the floor (x, z).
    pub fn doorstep(&self, out_m: f32) -> (f32, f32) {
        (self.door.0 + self.outward.0 * out_m, self.door.1 + self.outward.1 * out_m)
    }

    /// True when feet at height `feet_y` stand on the door's storey: its
    /// floor, by the rule every place with a floor uses (src/ship/moves.rs
    /// `stands_in_storey_of`, one storey tall).
    pub fn on_its_storey(&self, feet_y: f32) -> bool {
        crate::ship::moves::stands_in_storey_of(feet_y, self.floor, crate::ship::moves::STOREY_M)
    }

    /// What the door opens on, as it reads inside a sentence: "the Commons"
    /// (a leading "The" lowered), "First Street".
    pub fn opens_on_in_a_sentence(&self) -> String {
        in_a_sentence(&self.opens_on)
    }
}

/// The DataStore key of the published `FrontDoor`.
pub const FRONT_DOOR_KEY: &str = "own_front_door";

/// The front door of the home `ship` was assembled with: its plot's door
/// corridor, from the home (`ShipStructure::assemble` checks that the
/// corridor leaves through the design's door) to the zone it opens on. None
/// for a ship with no home on a plot: a guest's home put away, the ship file
/// on its own, or a corridor that does not resolve.
pub fn front_door_of(ship: &crate::ship::ship_structure::ShipStructure) -> Option<FrontDoor> {
    use crate::ship::ship_structure::HOME_ZONE_ID;
    ship.home_plot()?;
    let row = ship.corridors.iter().find(|c| c.from_zone == HOME_ZONE_ID)?;
    let g = ship.corridor_geometry(row).ok()?;
    let out = glam::Vec2::new(g.end_to.x - g.end_from.x, g.end_to.z - g.end_from.z).normalize_or_zero();
    if out == glam::Vec2::ZERO {
        return None;
    }
    let opens_on = ship.zones.get(g.to_zone_idx)?.label.clone();
    Some(FrontDoor { door: (g.end_from.x, g.end_from.z), floor: g.floor_y, outward: (out.x, out.y), opens_on })
}

/// Publish where the quests' places aboard are, from the ship the game runs
/// (it calls this every frame, engine::quest_hooks): the player's own front
/// door (`FRONT_DOOR_KEY`), and since 2026-10-12 the ship's labelled volumes
/// a `NearestVolume` place is found among (`SHIP_PLACES_KEY`), so "walk to the
/// mess hall" finds the mess hall that serves this home. Each is taken away
/// when there is none: no ship, or (the door) a guest whose home is put away.
/// A guest keeps the ship's places: the mess hall is theirs too.
pub fn publish_front_door(data: &mut DataStore, ship: Option<&crate::ship::ship_structure::ShipStructure>) {
    match ship.and_then(front_door_of) {
        Some(door) => {
            if data.get::<FrontDoor>(FRONT_DOOR_KEY) != Some(&door) {
                data.insert(FRONT_DOOR_KEY, door);
            }
        }
        None => {
            if data.contains(FRONT_DOOR_KEY) {
                data.remove(FRONT_DOOR_KEY);
            }
        }
    }
    match ship.map(ShipPlaces::of_ship).filter(|p| !p.volumes.is_empty()) {
        Some(places) => {
            if data.get::<ShipPlaces>(SHIP_PLACES_KEY) != Some(&places) {
                data.insert(SHIP_PLACES_KEY, places);
            }
        }
        None => {
            if data.contains(SHIP_PLACES_KEY) {
                data.remove(SHIP_PLACES_KEY);
            }
        }
    }
}

/// The placeholder a step's text may carry for what the player's own front
/// door opens on (2026-10-04): "the Commons" for a family home's plot, "First
/// Street" for a plot on the street (`step_text`).
pub const FRONT_DOOR_OPENS_ON: &str = "{front_door_opens_on}";

/// What the player reads for a step whose text is `description`: the text,
/// with what their own front door opens on filled in (`FRONT_DOOR_OPENS_ON`),
/// "the ship" while there is no front door to name. The HUD's quest line, the
/// Quests page and the step and quest notices all read it from here.
pub fn step_text(description: &str, data: &DataStore) -> String {
    if !description.contains(FRONT_DOOR_OPENS_ON) {
        return description.to_string();
    }
    let place = data
        .get::<FrontDoor>(FRONT_DOOR_KEY)
        .map(FrontDoor::opens_on_in_a_sentence)
        .unwrap_or_else(|| "the ship".to_string());
    description.replace(FRONT_DOOR_OPENS_ON, &place)
}

/// Where the player is, their eyes (the camera) in the frame destinations use,
/// for the Travel emitter (2026-10-04, the first-hour audit's B5; all three
/// coordinates since ship homes increment 7, so a place with a floor is
/// reached only on its storey). In first person the
/// camera IS the player: walking moves the camera, and the player entity's
/// Transform keeps wherever spawning or a teleport last put it, so reading the
/// Transform meant a Travel step never fired however far the player walked.
/// The game publishes "camera_position" every frame (lib.rs, the home-local
/// camera its proximity checks read, the walk-up interaction among them), and
/// this reads it; the controlled entity's Transform is the fallback only where
/// nothing publishes one (a headless world, a test).
pub fn player_position(world: &hecs::World, data: &DataStore) -> Option<glam::Vec3> {
    if let Some(p) = data.get::<glam::Vec3>("camera_position") {
        return Some(*p);
    }
    world
        .query::<(&crate::ecs::components::Transform, &crate::ecs::components::Controllable)>()
        .iter()
        .next()
        .map(|(_, (tf, _))| tf.position)
}

/// Stable quest key for an NPC display name: lowercase, every non-alphanumeric
/// run collapsed to one underscore, trimmed. "Mira Chen" -> "mira_chen", so a
/// quest authors Talk(npc_id: "mira_chen") no matter how the relay styles the
/// name. The talk emitter (lib.rs dialogue-open) fires "talk_<this>".
pub fn npc_talk_key(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut last_us = true; // suppress a leading underscore
    for c in name.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
            last_us = false;
        } else if !last_us {
            out.push('_');
            last_us = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    out
}

/// The quest event one unit of `item_id` made by a recipe reports, read by
/// `Make` objectives (2026-10-02).
pub fn make_event_key(item_id: &str) -> String {
    format!("make_{item_id}")
}

/// The quest event one `item_id` eaten reports (FoodSystem), read by `Eat`
/// objectives (2026-10-04).
pub fn eat_event_key(item_id: &str) -> String {
    format!("{EAT_EVENT}{item_id}")
}

/// The quest event one crop of `plant_id` planted reports (FarmingSystem),
/// read by `Plant` objectives (2026-10-04).
pub fn plant_event_key(plant_id: &str) -> String {
    format!("{PLANT_EVENT}{plant_id}")
}

/// The quest event a view coming on screen reports (the interface, through
/// the game's frame), read by `View` objectives (2026-10-04).
pub fn view_event_key(view: &str) -> String {
    format!("view_{view}")
}

/// The prefixes of the eat and plant events: an `Eat` or `Plant` step that
/// names no item counts every event with its prefix.
const EAT_EVENT: &str = "eat_";
const PLANT_EVENT: &str = "plant_";

/// How many events a count step has seen: of `prefix` + `id` when it names
/// one, else of every event with that prefix (any food, any crop).
fn counted(progress: &HashMap<String, u32>, prefix: &str, id: Option<&str>) -> u32 {
    match id {
        Some(id) => progress.get(&format!("{prefix}{id}")).copied().unwrap_or(0),
        None => progress.iter().filter(|(k, _)| k.starts_with(prefix)).map(|(_, n)| *n).sum(),
    }
}

/// Push a quest-progress event key (e.g. `"craft_smelt_iron"`, `"harvest_potato"`)
/// onto the shared `"quest_events"` DataStore channel. Action systems call this on
/// completion; [`QuestSystem`] drains it each tick and bumps matching progress
/// counters so count-based objectives (Craft/Harvest/…) advance. No-ops cleanly if
/// the channel is absent (e.g. a headless/test world that never registered it).
pub fn push_quest_event(data: &DataStore, key: String) {
    if let Some(lock) = data.get::<std::sync::Mutex<Vec<String>>>("quest_events") {
        if let Ok(mut events) = lock.lock() {
            events.push(key);
        }
    }
}

// ── Telling the player (2026-10-04, the first-hour audit's F3) ──────
//
// A finished quest was written only to the log: the rewards arrived and the
// next quest began with nothing on screen. These lines go on "player_notices",
// the channel the main loop shows as a toast that stays up long enough to read.

/// "Quest complete: First Steps. You received 2 Iron Ingot and 30 Metalworking
/// XP." Names come from the item and skill registries, falling back to the id.
pub fn completion_notice(
    name: &str,
    rewards: &[(String, u32)],
    xp_rewards: &[(String, u32)],
    items: Option<&crate::systems::inventory::ItemRegistry>,
    skills: Option<&crate::systems::skills::SkillRegistry>,
) -> String {
    let item = |id: &str| items.and_then(|r| r.items.get(id)).map(|d| d.name.clone()).unwrap_or_else(|| id.to_string());
    let skill = |id: &str| skills.and_then(|r| r.get(id)).map(|d| d.name.clone()).unwrap_or_else(|| id.replace('_', " "));
    let mut got: Vec<String> = rewards.iter().map(|(id, q)| format!("{q} {}", item(id))).collect();
    got.extend(xp_rewards.iter().map(|(id, xp)| format!("{xp} {} XP", skill(id))));
    if got.is_empty() {
        format!("Quest complete: {name}.")
    } else {
        format!("Quest complete: {name}. You received {}.", crate::systems::crafting::away::join_list(&got))
    }
}

/// The line for the quests a finished one started: "New quest: Toolsmith.
/// Forge a hammer." with the first step, or "New quests: A, B and C." for
/// several. None when it started none. `text` turns a step's description into
/// what the player reads (`step_text`).
pub fn next_quest_notice(started: &[&QuestDef], text: impl Fn(&str) -> String) -> Option<String> {
    match started {
        [] => None,
        [one] => Some(match one.steps.first() {
            Some(step) => format!("New quest: {}. {}.", one.name, text(&step.description).trim_end_matches('.')),
            None => format!("New quest: {}.", one.name),
        }),
        many => {
            let names: Vec<String> = many.iter().map(|q| q.name.clone()).collect();
            Some(format!("New quests: {}.", crate::systems::crafting::away::join_list(&names)))
        }
    }
}

/// The line when a step is done and the quest goes on (2026-10-04, the
/// opening): "First Steps: step 1 of 8 done. Next: Eat something: press I,
/// click a Basic Ration, then Eat." `done` counts from 1; `next` is the next
/// step as the player reads it. The HUD's quest line changes to the next step
/// at the same moment, and this says that it did. The last step is the quest's
/// own notice (`completion_notice`).
pub fn step_notice(quest: &str, done: usize, total: usize, next: &str) -> String {
    format!("{quest}: step {done} of {total} done. Next: {}.", next.trim_end_matches('.'))
}

fn post_notice(data: &DataStore, line: String) {
    if let Some(slot) = data.get::<std::sync::Mutex<Vec<String>>>("player_notices") {
        if let Ok(mut n) = slot.lock() {
            n.push(line);
        }
    }
}

// ── Player quest state (ECS component) ──────────────────────

/// Tracks a single active quest's progress for a player entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveQuest {
    /// Which quest definition this tracks.
    pub quest_id: String,
    /// Index into QuestDef::steps for the current step (0-based).
    pub current_step: usize,
    /// Progress counters keyed by objective description (for count-based objectives).
    pub progress: HashMap<String, u32>,
}

/// Attach to the player entity to track quest state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QuestTracker {
    /// Quests currently in progress.
    pub active_quests: Vec<ActiveQuest>,
    /// IDs of completed quests.
    pub completed_quests: Vec<String>,
}

impl QuestTracker {
    /// Whether a quest has been completed.
    pub fn is_completed(&self, quest_id: &str) -> bool {
        self.completed_quests.iter().any(|id| id == quest_id)
    }

    /// Whether a quest is currently active.
    pub fn is_active(&self, quest_id: &str) -> bool {
        self.active_quests.iter().any(|q| q.quest_id == quest_id)
    }

    /// Start tracking a new quest (no-op if already active or completed).
    pub fn accept_quest(&mut self, quest_id: &str) {
        if self.is_active(quest_id) || self.is_completed(quest_id) {
            return;
        }
        self.active_quests.push(ActiveQuest {
            quest_id: quest_id.to_string(),
            current_step: 0,
            progress: HashMap::new(),
        });
        log::info!("Quest accepted: {}", quest_id);
    }
}

// ── Reward granting ─────────────────────────────────────────

/// Pending quest reward — queued for the inventory system to process.
/// Stored in DataStore under "quest_rewards" as Vec<PendingReward>.
#[derive(Debug, Clone)]
pub struct PendingReward {
    pub entity: hecs::Entity,
    pub item_id: String,
    pub quantity: u32,
}

// ── Quest system ────────────────────────────────────────────

/// Checks active quest objectives each tick, advances steps, awards rewards.
pub struct QuestSystem;

impl Default for QuestSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl QuestSystem {
    pub fn new() -> Self {
        Self
    }

    /// Check if a single objective is met given the player's inventory, what
    /// the home holds (`at_home`, by item id), the progress map and the
    /// destinations the player stands in now (`here`, `destinations_here`).
    fn check_objective(
        objective: &QuestObjective,
        inventory: Option<&Inventory>,
        at_home: &dyn Fn(&str) -> u32,
        progress: &HashMap<String, u32>,
        here: &std::collections::HashSet<String>,
    ) -> bool {
        match objective {
            QuestObjective::Gather { item_id, quantity } => {
                // Acquired = carried, or kept at home (2026-10-04, with
                // BUG-150): the drone unloads its haul into home storage and
                // the automated machines file their goods there, so ore the
                // drone brought home counts whether or not it was carried.
                let carried = inventory.map_or(0, |inv| inv.count_item(item_id));
                carried + at_home(item_id) >= *quantity
            }
            QuestObjective::Craft { recipe_id, quantity } => {
                // Track via progress counter (crafting system increments this)
                let key = format!("craft_{}", recipe_id);
                progress.get(&key).copied().unwrap_or(0) >= *quantity
            }
            QuestObjective::Make { item_id, quantity } => {
                // Units made by any recipe (crafting pushes one event per unit)
                progress.get(&make_event_key(item_id)).copied().unwrap_or(0) >= *quantity
            }
            QuestObjective::Harvest { crop_id, quantity } => {
                // Track via progress counter (farming system increments this)
                let key = format!("harvest_{}", crop_id);
                progress.get(&key).copied().unwrap_or(0) >= *quantity
            }
            QuestObjective::Build { blueprint_id } => {
                // Track via progress counter (construction system sets this)
                let key = format!("build_{}", blueprint_id);
                progress.get(&key).copied().unwrap_or(0) >= 1
            }
            // Standing in the place while this is the step (2026-10-04): only
            // the current step is ever checked, so an earlier pass does not
            // count (`destinations_here`).
            QuestObjective::Travel { destination } => here.contains(destination),
            QuestObjective::Talk { npc_id } => {
                // Track via progress counter (interaction system sets this)
                let key = format!("talk_{}", npc_id);
                progress.get(&key).copied().unwrap_or(0) >= 1
            }
            // What the player ate, planted and opened (2026-10-04, the
            // opening), from the events the food and farming systems and the
            // interface report where it really happens.
            QuestObjective::Eat { item_id, quantity } => counted(progress, EAT_EVENT, item_id.as_deref()) >= *quantity,
            QuestObjective::Plant { crop_id, quantity } => counted(progress, PLANT_EVENT, crop_id.as_deref()) >= *quantity,
            QuestObjective::View { view } => progress.get(&view_event_key(view)).copied().unwrap_or(0) >= 1,
        }
    }
}

impl System for QuestSystem {
    fn name(&self) -> &str {
        "QuestSystem"
    }

    fn tick(&mut self, world: &mut hecs::World, _dt: f32, data: &DataStore) {
        let registry = match data.get::<QuestRegistry>("quest_registry") {
            Some(r) => r,
            None => return, // No quests loaded yet
        };

        // Where the player stands now, for Travel steps (2026-10-04): the
        // destinations (data/entities/destinations.ron) they are inside this
        // tick, the player's own front door and the ship's shared places
        // among them when the game published them (`Aboard`). Where the player
        // is comes from the walking camera (`player_position`): the entity's
        // Transform never moved when they walked.
        let here = match (data.get::<DestinationList>("quest_destinations"), player_position(world, data)) {
            (Some(dests), Some(at)) => destinations_here(at, dests, Aboard::of(data)),
            _ => std::collections::HashSet::new(),
        };

        // Drain quest-progress events the action systems pushed this frame
        // ("craft_<recipe>", "harvest_<crop>", ...). Applied to every active
        // quest's progress map below so count-based objectives advance. (Gather
        // objectives are checked against live inventory and need no events.)
        let events: Vec<String> = data
            .get::<std::sync::Mutex<Vec<String>>>("quest_events")
            .and_then(|m| m.lock().ok().map(|mut e| e.drain(..).collect()))
            .unwrap_or_default();

        // What the home holds, for Gather steps: home storage as mirrored for
        // this tick ("home_stock", engine::stock_piles::publish_home_stock),
        // after what the systems ahead of this one took from it, plus what
        // landed in it this tick and is not put away yet ("home_stock_outputs":
        // a drone haul, a machine's batch; the main loop files it after the
        // tick). Read per item, only when a Gather step asks. None of it
        // without home storage (tests, headless).
        let at_home = |id: &str| -> u32 {
            let stored = data
                .get::<std::sync::Mutex<HashMap<String, u32>>>("home_stock")
                .and_then(|m| m.lock().ok().map(|s| s.get(id).copied().unwrap_or(0)))
                .unwrap_or(0);
            let landed: u32 = data
                .get::<std::sync::Mutex<Vec<(String, u32)>>>("home_stock_outputs")
                .and_then(|m| m.lock().ok().map(|v| v.iter().filter(|(i, _)| i == id).map(|(_, q)| q).sum::<u32>()))
                .unwrap_or(0);
            stored + landed
        };

        // Collect entities with QuestTracker to process. Completed tuples carry
        // (quest_id, item rewards, xp rewards).
        #[allow(clippy::type_complexity)]
        let mut updates: Vec<(
            hecs::Entity,
            QuestTracker,
            Vec<(String, Vec<(String, u32)>, Vec<(String, u32)>)>,
        )> = Vec::new();

        for (entity, (tracker, inventory)) in
            world.query_mut::<(&QuestTracker, Option<&Inventory>)>()
        {
            let mut tracker = tracker.clone();
            // Apply this frame's progress events to every active quest's counters
            // (e.g. a "craft_smelt_iron" event bumps progress["craft_smelt_iron"]).
            let events_applied = !events.is_empty() && !tracker.active_quests.is_empty();
            if events_applied {
                for active in tracker.active_quests.iter_mut() {
                    for key in &events {
                        *active.progress.entry(key.clone()).or_insert(0) += 1;
                    }
                }
            }
            let mut completed_this_tick: Vec<(String, Vec<(String, u32)>, Vec<(String, u32)>)> =
                Vec::new();
            let mut quests_to_advance: Vec<(usize, usize)> = Vec::new(); // (quest_index, new_step)
            let mut quests_to_complete: Vec<usize> = Vec::new();

            for (qi, active) in tracker.active_quests.iter().enumerate() {
                let quest_def = match registry.get(&active.quest_id) {
                    Some(def) => def,
                    None => continue, // Quest definition not found
                };

                // Check if current step is within bounds
                if active.current_step >= quest_def.steps.len() {
                    // All steps done — mark for completion
                    quests_to_complete.push(qi);
                    completed_this_tick.push((
                        active.quest_id.clone(),
                        quest_def.rewards.clone(),
                        quest_def.xp_rewards.clone(),
                    ));
                    continue;
                }

                let step = &quest_def.steps[active.current_step];
                if Self::check_objective(&step.objective, inventory, &at_home, &active.progress, &here) {
                    let next_step = active.current_step + 1;
                    if next_step >= quest_def.steps.len() {
                        // Final step completed
                        quests_to_complete.push(qi);
                        completed_this_tick.push((
                            active.quest_id.clone(),
                            quest_def.rewards.clone(),
                            quest_def.xp_rewards.clone(),
                        ));
                    } else {
                        quests_to_advance.push((qi, next_step));
                    }
                }
            }

            // Apply step advances (do this before removals to keep indices valid)
            for (qi, new_step) in &quests_to_advance {
                tracker.active_quests[*qi].current_step = *new_step;
                log::info!(
                    "Quest '{}': advanced to step {}",
                    tracker.active_quests[*qi].quest_id,
                    new_step
                );
                // Tell the player the step is done and what is next
                // (2026-10-04, the opening): the HUD's quest line changes at
                // this moment, and a change nothing points at is easy to miss.
                if let Some(def) = registry.get(&tracker.active_quests[*qi].quest_id) {
                    if let Some(next) = def.steps.get(*new_step) {
                        post_notice(data, step_notice(&def.name, *new_step, def.steps.len(), &step_text(&next.description, data)));
                    }
                }
            }

            // Complete quests (remove in reverse order to preserve indices)
            quests_to_complete.sort_unstable();
            for qi in quests_to_complete.into_iter().rev() {
                let quest_id = tracker.active_quests[qi].quest_id.clone();
                tracker.active_quests.remove(qi);
                tracker.completed_quests.push(quest_id.clone());
                log::info!("Quest completed: {}", quest_id);
            }

            // Prerequisite chaining: completing a quest auto-accepts any quest whose
            // prerequisite it satisfies (and that isn't already active or completed).
            let mut started: Vec<&QuestDef> = Vec::new();
            for (completed_id, _, _) in &completed_this_tick {
                for def in registry.quests.values() {
                    if def.prerequisite.as_deref() == Some(completed_id.as_str())
                        && !tracker.is_active(&def.id)
                        && !tracker.is_completed(&def.id)
                    {
                        tracker.accept_quest(&def.id);
                        started.push(def);
                    }
                }
            }

            // Tell the player (2026-10-04, F3): what finished and what it gave,
            // then what started. Only the player carries a QuestTracker.
            let items = data.get::<crate::systems::inventory::ItemRegistry>("item_registry");
            let skills = data.get::<crate::systems::skills::SkillRegistry>("skill_registry");
            for (completed_id, rewards, xp_rewards) in &completed_this_tick {
                let name = registry.get(completed_id).map_or(completed_id.as_str(), |d| d.name.as_str());
                post_notice(data, completion_notice(name, rewards, xp_rewards, items, skills));
            }
            started.sort_by(|a, b| a.name.cmp(&b.name)); // the registry's order is a hash map's
            if let Some(line) = next_quest_notice(&started, |d| step_text(d, data)) {
                post_notice(data, line);
            }

            if events_applied
                || !completed_this_tick.is_empty()
                || !quests_to_advance.is_empty()
            {
                updates.push((entity, tracker, completed_this_tick));
            }
        }

        // Apply tracker updates and grant rewards directly to inventory
        for (entity, tracker, completed) in updates {
            if let Ok(mut t) = world.get::<&mut QuestTracker>(entity) {
                *t = tracker;
            }

            // Grant item + XP rewards for completed quests (XP via the shared
            // xp_grants channel, drained by SkillSystem later this same frame).
            for (_quest_id, rewards, xp_rewards) in completed {
                for (item_id, quantity) in rewards {
                    if let Ok(mut inv) = world.get::<&mut Inventory>(entity) {
                        let overflow = inv.add_item(&item_id, quantity, 99);
                        if overflow > 0 {
                            log::warn!(
                                "Quest reward overflow: {} of {} could not fit in inventory",
                                overflow,
                                item_id
                            );
                        }
                    }
                }
                for (skill_id, amount) in xp_rewards {
                    crate::systems::skills::award_skill_xp(data, &skill_id, amount);
                }
            }
        }

    }
}

#[cfg(test)]
mod quest_tests {
    use super::*;

    fn quest(id: &str, obj: QuestObjective, reward: Vec<(String, u32)>, prereq: Option<&str>) -> QuestDef {
        QuestDef {
            id: id.to_string(),
            name: id.to_string(),
            description: String::new(),
            steps: vec![QuestStep {
                description: String::new(),
                objective: obj,
            }],
            rewards: reward,
            xp_rewards: Vec::new(),
            prerequisite: prereq.map(|s| s.to_string()),
        }
    }

    /// v0.748 (ladder rung 4): completing a quest grants its xp_rewards through
    /// the shared xp_grants channel (drained by SkillSystem the same frame).
    #[test]
    fn completion_grants_xp_rewards() {
        let mut reg = QuestRegistry::default();
        let mut q = quest(
            "xp_quest",
            QuestObjective::Gather { item_id: "stick_0".into(), quantity: 1 },
            vec![],
            None,
        );
        q.xp_rewards = vec![("farming".to_string(), 25)];
        reg.quests.insert(q.id.clone(), q);

        let mut data = DataStore::new();
        data.insert("quest_registry", reg);
        data.insert(
            "xp_grants",
            std::sync::Mutex::new(Vec::<crate::systems::skills::SkillXPEvent>::new()),
        );
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(4);
        inv.add_item("stick_0", 1, 99);
        let mut tracker = QuestTracker::default();
        tracker.accept_quest("xp_quest");
        world.spawn((inv, tracker));

        let mut sys = QuestSystem::new();
        sys.tick(&mut world, 0.1, &data);

        let grants = data
            .get::<std::sync::Mutex<Vec<crate::systems::skills::SkillXPEvent>>>("xp_grants")
            .unwrap()
            .lock()
            .unwrap()
            .clone();
        assert!(
            grants.iter().any(|g| g.skill_id == "farming" && g.amount == 25),
            "quest completion pushed the XP grant, got {grants:?}"
        );
    }

    #[test]
    fn from_ron_dir_loads_the_real_quests() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/quests");
        let reg = QuestRegistry::from_ron_dir(&dir);
        assert!(reg.get("gs_first_steps").is_some(), "the getting-started chain loads");
        assert!(reg.quests.len() >= 4, "all quest files merged, got {}", reg.quests.len());
    }

    /// The game's quest loader (BUG-163): a shipped quest file this version cannot read
    /// comes from its built-in copy, a modder's own file is added beside the shipped
    /// ones, and a broken file of their own is skipped without costing anything else.
    ///
    /// Seen red with `load` reading the folder alone (`from_ron_dir`):
    ///   the opening quest is missing when its file cannot be read
    #[test]
    fn a_shipped_quest_file_the_game_cannot_read_comes_from_its_built_in_copy() {
        let root = crate::test_temp::dir("quests");
        let data = root.join("data");
        let quests = data.join("quests");
        std::fs::create_dir_all(&quests).expect("make the folder");
        std::fs::write(quests.join("getting_started.ron"), "[ (id: \"gs_first_steps\", name: ").expect("write");
        let own = r#"[(id: "my_quest", name: "Mine", description: "A modder's quest", steps: [], rewards: [], prerequisite: None)]"#;
        std::fs::write(quests.join("zz_mine.ron"), own).expect("write");
        std::fs::write(quests.join("zz_broken.ron"), "[(").expect("write");
        let reg = QuestRegistry::load(&data);
        assert!(reg.get("gs_first_steps").is_some(), "the opening quest is missing when its file cannot be read");
        assert!(reg.get("my_quest").is_some(), "the modder's own quest is added");
        let shipped = QuestRegistry::from_ron_dir(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/quests"));
        assert_eq!(reg.quests.len(), shipped.quests.len() + 1, "every shipped quest, and the modder's");
    }

    /// Where the player stands (2026-10-04, replacing v0.979's edge trigger):
    /// the destinations whose radius holds the player's XZ, a fixed place by
    /// its `pos`, the own front door by the door the game published, and
    /// nothing for a front door that is not published (a guest).
    #[test]
    fn destinations_here_are_the_places_the_player_stands_in() {
        let dests = DestinationList {
            destinations: vec![
                DestinationDef { id: "fields".into(), label: "the fields".into(), pos: (10.0, 10.0), radius: 5.0, at: None },
                DestinationDef {
                    id: "front_door".into(),
                    label: "your front door".into(),
                    pos: (0.0, 0.0),
                    radius: 1.5,
                    at: Some(DestinationAnchor::OwnFrontDoor { out_m: 1.5 }),
                },
            ],
        };
        let door = FrontDoor { door: (55.0, 139.0), floor: 0.0, outward: (1.0, 0.0), opens_on: "First Street".into() };
        let aboard = Aboard { front_door: Some(&door), places: None };
        // Where the player's eyes are, standing on the deck.
        let here = |xz: (f32, f32)| destinations_here(glam::Vec3::new(xz.0, 1.7, xz.1), &dests, aboard);
        assert!(here((30.0, 30.0)).is_empty(), "out in the open");
        assert_eq!(here((11.0, 11.0)), ["fields".to_string()].into_iter().collect(), "in the fields");
        assert_eq!(here((56.5, 139.0)), ["front_door".to_string()].into_iter().collect(), "on the doorstep, 1.5 m out of the door");
        assert!(here((53.5, 139.5)).is_empty(), "just inside the door is not out of it");
        assert!(here((0.0, 0.0)).is_empty(), "the front door's unused pos is no place");
        assert!(destinations_here(glam::Vec3::new(56.5, 1.7, 139.0), &dests, Aboard::default()).is_empty(), "no door published, no doorstep");
        assert_eq!(door.opens_on_in_a_sentence(), "First Street");
        let commons = FrontDoor { opens_on: "The Commons".into(), ..door };
        assert_eq!(commons.opens_on_in_a_sentence(), "the Commons", "\"The\" is lowered inside a sentence");
    }

    /// A SHARED PLACE ABOARD IS ITS WHOLE FLOOR (2026-10-12, ship homes
    /// increment 6): standing anywhere on the floor of the nearest volume of
    /// its type counts, and so does standing within the radius of its edge.
    /// A circle round the middle would not do: the mess hall is a 30 x 8 m
    /// strip, and its stores stand at its west end, 13 m from the middle,
    /// where a player taking a meal stands. Nearest to the player's own front
    /// door, so the other hall does not count; a guest, with no door, is sent
    /// to the one nearest where they stand. No places published, no place.
    ///
    /// Seen red 2026-10-12 with the place taken as a 4 m circle round the
    /// floor's middle (the plan's wording, docs/design/ship-homes-increment-6-
    /// plan.md 3.10: "the centre of the nearest labelled volume", radius 4.0):
    /// "the stores' end of the mess hall is in it".
    #[test]
    fn a_shared_place_aboard_is_its_whole_floor() {
        let hall = |id: &str, zone: &str, min: (f32, f32), max: (f32, f32)| ShipVolume {
            id: id.into(),
            type_id: "mess_hall".into(),
            label: "Mess hall".into(),
            zone_label: zone.into(),
            min,
            max,
            floor: 0.0,
            height: 3.0,
        };
        let places = ShipPlaces {
            volumes: vec![
                hall("mess-hall", "The Commons", (65.0, 20.0), (95.0, 28.0)),
                hall("mess-hall-2", "The South Commons", (77.0, 770.0), (101.0, 778.0)),
            ],
        };
        let dests = DestinationList {
            destinations: vec![DestinationDef {
                id: "mess_hall".into(),
                label: "the mess hall".into(),
                pos: (0.0, 0.0),
                radius: 1.0,
                at: Some(DestinationAnchor::NearestVolume { zone_type: "mess_hall".into() }),
            }],
        };
        let cabin_door = FrontDoor { door: (80.5, 89.25), floor: 0.0, outward: (-1.0, 0.0), opens_on: "Cabin Block A".into() };
        let aboard = Aboard { front_door: Some(&cabin_door), places: Some(&places) };
        let here = |xz: (f32, f32), aboard: Aboard<'_>| destinations_here(glam::Vec3::new(xz.0, 1.7, xz.1), &dests, aboard).contains("mess_hall");
        assert!(here((67.0, 22.0), aboard), "the stores' end of the mess hall is in it");
        assert!(here((80.0, 24.0), aboard), "its middle");
        assert!(here((80.0, 28.9), aboard), "within a metre of its edge");
        assert!(!here((80.0, 29.5), aboard), "a metre and a half out is not");
        assert!(!here((89.0, 774.0), aboard), "the cabin's mess hall is the Commons', not the South Commons'");
        let guest = Aboard { front_door: None, places: Some(&places) };
        assert!(here((89.0, 774.0), guest), "a guest is sent to the one nearest where they stand");
        assert_eq!(dests.destinations[0].volume(guest, glam::Vec3::new(89.0, 1.7, 790.0)).map(|v| v.zone_in_a_sentence()), Some("the South Commons".into()));
        assert!(!here((80.0, 24.0), Aboard { front_door: Some(&cabin_door), places: None }), "no places published, no place");
    }

    /// A PLACE ON THE UPPER FLOOR IS NOT REACHED FROM UNDER IT (ship homes
    /// increment 7): the quests' places keep their height. A recreation room on
    /// a storey at 3.0 m is reached standing on its floor (eyes at 4.7 m), not
    /// from the floor under it (eyes at 1.7 m, 0 m from it across the floor)
    /// nor from the floor over it. The front door's doorstep is on the door's
    /// floor, so the floor over the doorstep is not out of the door. And a
    /// NearestVolume is chosen on the storey it is measured from: a guest on
    /// the ground floor is sent to the hall on their floor 20 m away, not the
    /// one right over their head; upstairs, to the one upstairs.
    ///
    /// Seen red 2026-10-20 with `stands_in_storey_of` answering true (every
    /// height counts, the rule before storeys): "the upper floor's recreation
    /// room was reached from under it".
    #[test]
    fn a_quest_place_on_the_upper_floor_is_not_reached_from_under_it() {
        let room = |id: &str, type_id: &str, min: (f32, f32), max: (f32, f32), floor: f32| ShipVolume {
            id: id.into(),
            type_id: type_id.into(),
            label: id.into(),
            zone_label: "The Commons".into(),
            min,
            max,
            floor,
            height: 3.0,
        };
        let places = ShipPlaces {
            volumes: vec![
                room("rec-up", "recreation", (10.0, 10.0), (20.0, 20.0), 3.0),
                room("hall-up", "mess_hall", (40.0, 10.0), (50.0, 20.0), 3.0),
                room("hall-down", "mess_hall", (40.0, 30.0), (50.0, 40.0), 0.0),
            ],
        };
        let place = |id: &str, ty: &str| DestinationDef { id: id.into(), label: id.into(), pos: (0.0, 0.0), radius: 1.0, at: Some(DestinationAnchor::NearestVolume { zone_type: ty.into() }) };
        let dests = DestinationList {
            destinations: vec![
                place("recreation", "recreation"),
                DestinationDef { id: "front_door".into(), label: "your front door".into(), pos: (0.0, 0.0), radius: 1.5, at: Some(DestinationAnchor::OwnFrontDoor { out_m: 1.5 }) },
            ],
        };
        let door = FrontDoor { door: (55.0, 40.0), floor: 0.0, outward: (1.0, 0.0), opens_on: "The Commons".into() };
        let aboard = Aboard { front_door: Some(&door), places: Some(&places) };
        let here = |x: f32, eye_y: f32, z: f32| destinations_here(glam::Vec3::new(x, eye_y, z), &dests, aboard);
        assert!(!here(15.0, 1.7, 15.0).contains("recreation"), "the upper floor's recreation room was reached from under it");
        assert!(here(15.0, 4.7, 15.0).contains("recreation"), "on its floor");
        assert!(here(15.0, 4.7 + 1.2, 15.0).contains("recreation"), "jumping in it");
        assert!(!here(15.0, 7.7, 15.0).contains("recreation"), "from the floor over it");
        assert!(here(56.5, 1.7, 40.0).contains("front_door"), "on the doorstep");
        assert!(!here(56.5, 4.7, 40.0).contains("front_door"), "the floor over the doorstep is not out of the door");
        // Nearest on the storey it is measured from.
        let halls = place("mess_hall", "mess_hall");
        let guest = Aboard { front_door: None, places: Some(&places) };
        let nearest = |eye_y: f32| halls.volume(guest, glam::Vec3::new(45.0, eye_y, 15.0)).map(|v| v.id.clone());
        assert_eq!(nearest(1.7).as_deref(), Some("hall-down"), "on the ground floor, the hall on your floor, not the one over your head");
        assert_eq!(nearest(4.7).as_deref(), Some("hall-up"), "upstairs, the one upstairs");
        assert_eq!(halls.volume(aboard, glam::Vec3::new(45.0, 4.7, 15.0)).map(|v| v.id.as_str()), Some("hall-down"), "from the front door, on its floor");
    }

    #[test]
    fn npc_talk_key_slugs_names_stably() {
        assert_eq!(npc_talk_key("Mira Chen"), "mira_chen");
        assert_eq!(npc_talk_key("  D'Arcy-Lou  "), "d_arcy_lou");
        assert_eq!(npc_talk_key("R2"), "r2");
        assert_eq!(npc_talk_key("___"), "");
    }

    /// THE quest-data lockstep (v0.981, post-audit item 1 made permanent):
    /// every id any shipped quest objective references must exist in its
    /// source-of-truth data file. The 2026-07-20 audit found quests ~80%
    /// dead-id; the rewrite fixed them, and this test keeps them fixed - a
    /// quest naming a nonexistent item/recipe/blueprint/crop/destination can
    /// never advance and now fails the build instead of failing the player.
    /// (Talk ids are exempt: crew names come from the RELAY at runtime, not
    /// from static data - the npc_talk_key convention is their contract.)
    #[test]
    fn shipped_quest_objective_ids_all_resolve() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let csv_ids = |rel: &str| -> std::collections::HashSet<String> {
            std::fs::read_to_string(root.join(rel))
                .unwrap_or_else(|e| panic!("{rel} readable: {e}"))
                .lines()
                .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
                .filter_map(|l| l.split(',').next().map(|s| s.trim().to_string()))
                .collect()
        };
        let items = csv_ids("data/items.csv");
        let recipes = csv_ids("data/recipes.csv");
        // Every item some recipe outputs (column 5, "id:qty|id:qty").
        let made: std::collections::HashSet<String> = std::fs::read_to_string(root.join("data/recipes.csv"))
            .expect("recipes.csv readable")
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .filter_map(|l| l.split(',').nth(4))
            .flat_map(|outs| outs.split('|').filter_map(|o| o.split(':').next()).map(|s| s.trim().to_string()))
            .collect();
        let plants = csv_ids("data/plants.csv");
        let blueprints = crate::systems::construction::BlueprintRegistry::from_ron(
            &std::fs::read(root.join("data/blueprints/basic.ron")).unwrap(),
        )
        .expect("blueprints parse");
        let dests = DestinationList::from_ron(
            &std::fs::read(root.join("data/entities/destinations.ron")).unwrap(),
        )
        .expect("destinations parse");
        let dest_ids: std::collections::HashSet<&str> =
            dests.destinations.iter().map(|d| d.id.as_str()).collect();

        let reg = QuestRegistry::from_ron_dir(&root.join("data/quests"));
        assert!(reg.quests.len() >= 10, "quest chains load, got {}", reg.quests.len());
        for q in reg.quests.values() {
            for s in &q.steps {
                match &s.objective {
                    QuestObjective::Gather { item_id, .. } => assert!(
                        items.contains(item_id),
                        "quest {}: Gather names unknown item {item_id}",
                        q.id
                    ),
                    QuestObjective::Craft { recipe_id, .. } => assert!(
                        recipes.contains(recipe_id),
                        "quest {}: Craft names unknown recipe {recipe_id}",
                        q.id
                    ),
                    // A Make item must exist AND some recipe must produce it,
                    // or the step can never count a unit.
                    QuestObjective::Make { item_id, .. } => {
                        assert!(items.contains(item_id), "quest {}: Make names unknown item {item_id}", q.id);
                        assert!(made.contains(item_id), "quest {}: no recipe makes {item_id}", q.id);
                    }
                    QuestObjective::Harvest { crop_id, .. } => assert!(
                        plants.contains(crop_id),
                        "quest {}: Harvest names unknown crop {crop_id}",
                        q.id
                    ),
                    QuestObjective::Build { blueprint_id } => assert!(
                        blueprints.blueprints.contains_key(blueprint_id),
                        "quest {}: Build names unknown blueprint {blueprint_id}",
                        q.id
                    ),
                    QuestObjective::Travel { destination } => assert!(
                        dest_ids.contains(destination.as_str()),
                        "quest {}: Travel names unknown destination {destination}",
                        q.id
                    ),
                    QuestObjective::Talk { .. } => {} // relay-runtime names, see doc
                    // An Eat item must be food the player can eat (not drink).
                    QuestObjective::Eat { item_id, .. } => {
                        if let Some(id) = item_id {
                            assert!(items.contains(id), "quest {}: Eat names unknown item {id}", q.id);
                            assert_eq!(
                                crate::systems::food::consume_kinds().get(id),
                                Some(&false),
                                "quest {}: Eat names {id}, which is not eaten",
                                q.id
                            );
                        }
                    }
                    QuestObjective::Plant { crop_id, .. } => {
                        if let Some(id) = crop_id {
                            assert!(plants.contains(id), "quest {}: Plant names unknown crop {id}", q.id);
                        }
                    }
                    // The views the interface reports (GuiState::on_screen).
                    QuestObjective::View { view } => assert!(
                        ["vitals"].contains(&view.as_str()),
                        "quest {}: View names {view}, which no page reports",
                        q.id
                    ),
                }
                // Reward items must exist too - a completed quest that grants
                // a phantom item would vanish the reward silently.
            }
            for (item_id, _) in &q.rewards {
                assert!(
                    items.contains(item_id),
                    "quest {}: reward names unknown item {item_id}",
                    q.id
                );
            }
        }
    }

    /// Lockstep guard: every Travel(destination) referenced by any shipped
    /// quest exists in data/entities/destinations.ron - a quest pointing at a
    /// nonexistent place could never advance and would fail here, not in play.
    #[test]
    fn shipped_travel_destinations_all_exist() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let dests = DestinationList::from_ron(
            &std::fs::read(root.join("data/entities/destinations.ron")).unwrap(),
        )
        .unwrap();
        assert!(dests.destinations.len() >= 4, "shipped destination list loads");
        let ids: std::collections::HashSet<&str> =
            dests.destinations.iter().map(|d| d.id.as_str()).collect();
        let reg = QuestRegistry::from_ron_dir(&root.join("data/quests"));
        let mut travel_steps = 0;
        for q in reg.quests.values() {
            for s in &q.steps {
                if let QuestObjective::Travel { destination } = &s.objective {
                    travel_steps += 1;
                    assert!(
                        ids.contains(destination.as_str()),
                        "quest {} references unknown destination {destination}",
                        q.id
                    );
                }
            }
        }
        assert!(travel_steps >= 3, "the restored Travel steps load, got {travel_steps}");
    }

    #[test]
    fn gather_quest_completes_from_inventory_and_grants_reward() {
        let mut reg = QuestRegistry::default();
        reg.quests.insert(
            "q_gather".into(),
            quest(
                "q_gather",
                QuestObjective::Gather { item_id: "iron_ore_0".into(), quantity: 3 },
                vec![("iron_ingot_0".into(), 2)],
                None,
            ),
        );
        let mut data = DataStore::new();
        data.insert("quest_registry", reg);

        let mut world = hecs::World::new();
        let mut tracker = QuestTracker::default();
        tracker.accept_quest("q_gather");
        let mut inv = Inventory::new(16);
        inv.add_item("iron_ore_0", 2, 99); // one short
        let player = world.spawn((tracker, inv));
        let mut sys = QuestSystem::new();

        sys.tick(&mut world, 0.0, &data);
        assert!(
            !world.get::<&QuestTracker>(player).unwrap().is_completed("q_gather"),
            "2 < 3 ore → incomplete"
        );

        world.get::<&mut Inventory>(player).unwrap().add_item("iron_ore_0", 1, 99);
        sys.tick(&mut world, 0.0, &data);
        let t = world.get::<&QuestTracker>(player).unwrap();
        assert!(t.is_completed("q_gather"), "3 ore → quest completes");
        assert_eq!(
            world.get::<&Inventory>(player).unwrap().count_item("iron_ingot_0"),
            2,
            "completion granted the reward"
        );
    }

    /// WHAT THE HOME HOLDS IS ACQUIRED (2026-10-04, with BUG-150). The drone
    /// now unloads its haul into home storage, the store the home's machines
    /// are fed from, not into the backpack, so the first quest's "Acquire 3
    /// iron ore (mine it with a drone, or stock it)" counts what the home
    /// holds as well as what is carried: home storage as mirrored for the
    /// tick, and what landed in it this tick and is not put away yet (the
    /// drone files its haul during the tick; the main loop puts it away
    /// after). 1 carried + 1 stored is 2, short; the haul's 1 makes 3.
    ///
    /// Seen red before the fix: "1 carried + 1 stored + 1 just landed is 3:
    /// complete" (the Gather step counted the backpack's 1 alone).
    #[test]
    fn gather_counts_what_the_home_holds_as_well_as_the_backpack() {
        use std::sync::Mutex;
        let mut reg = QuestRegistry::default();
        reg.quests.insert(
            "q_gather".into(),
            quest("q_gather", QuestObjective::Gather { item_id: "iron_ore_0".into(), quantity: 3 }, vec![], None),
        );
        let mut data = DataStore::new();
        data.insert("quest_registry", reg);
        data.insert("home_stock", Mutex::new(HashMap::from([("iron_ore_0".to_string(), 1u32)])));
        data.insert("home_stock_outputs", Mutex::new(Vec::<(String, u32)>::new()));
        let mut world = hecs::World::new();
        let mut tracker = QuestTracker::default();
        tracker.accept_quest("q_gather");
        let mut inv = Inventory::new(16);
        inv.add_item("iron_ore_0", 1, 99);
        let player = world.spawn((tracker, inv));
        let mut sys = QuestSystem::new();
        sys.tick(&mut world, 0.0, &data);
        assert!(
            !world.get::<&QuestTracker>(player).unwrap().is_completed("q_gather"),
            "1 carried + 1 stored is 2 < 3: incomplete"
        );
        data.get::<Mutex<Vec<(String, u32)>>>("home_stock_outputs").unwrap().lock().unwrap().push(("iron_ore_0".into(), 1));
        sys.tick(&mut world, 0.0, &data);
        assert!(
            world.get::<&QuestTracker>(player).unwrap().is_completed("q_gather"),
            "1 carried + 1 stored + 1 just landed is 3: complete"
        );
    }

    /// MAKE COUNTS EVERY ROUTE TO THE ITEM (2026-10-02). The first quest's
    /// iron step named `smelt_iron` (coal), so a player who smelted with
    /// graphite made the ingot and the quest did not move. A Make step counts
    /// units of the item from any recipe. Red check, run: with the starter
    /// quest's step as `Craft(smelt_iron)`, the graphite event below left it
    /// incomplete.
    #[test]
    fn make_objective_counts_any_recipe_that_produces_the_item() {
        let mut reg = QuestRegistry::default();
        reg.quests.insert(
            "q_make".into(),
            quest("q_make", QuestObjective::Make { item_id: "iron_ingot_0".into(), quantity: 2 }, vec![], None),
        );
        let mut data = DataStore::new();
        data.insert("quest_registry", reg);
        data.insert("quest_events", std::sync::Mutex::new(Vec::<String>::new()));
        let mut world = hecs::World::new();
        let mut tracker = QuestTracker::default();
        tracker.accept_quest("q_make");
        let player = world.spawn((tracker, Inventory::new(16)));
        let mut sys = QuestSystem::new();

        // Slag is made alongside; it must not count toward iron.
        push_quest_event(&data, make_event_key("slag_0"));
        push_quest_event(&data, make_event_key("iron_ingot_0"));
        sys.tick(&mut world, 0.0, &data);
        assert!(!world.get::<&QuestTracker>(player).unwrap().is_completed("q_make"), "1 of 2 ingots");
        push_quest_event(&data, make_event_key("iron_ingot_0"));
        sys.tick(&mut world, 0.0, &data);
        assert!(world.get::<&QuestTracker>(player).unwrap().is_completed("q_make"), "2 of 2 ingots");

        // The shipped starter quest's smelting step takes the graphite route
        // (since 2026-10-04 it is the sixth of the opening's eight steps).
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let shipped = QuestRegistry::from_ron_dir(&root.join("data/quests"));
        let smelt = shipped
            .get("gs_first_steps")
            .expect("gs_first_steps ships")
            .steps
            .iter()
            .position(|s| matches!(&s.objective, QuestObjective::Make { item_id, .. } if item_id == "iron_ingot_0"))
            .expect("First Steps smelts an iron ingot");
        let mut data = DataStore::new();
        data.insert("quest_registry", shipped);
        data.insert("quest_events", std::sync::Mutex::new(Vec::<String>::new()));
        let mut world = hecs::World::new();
        let mut tracker = QuestTracker::default();
        tracker.accept_quest("gs_first_steps");
        tracker.active_quests[0].current_step = smelt;
        let player = world.spawn((tracker, Inventory::new(16)));
        sys.tick(&mut world, 0.0, &data);
        // What crafting reports for one smelt_iron_graphite batch.
        push_quest_event(&data, "craft_smelt_iron_graphite".to_string());
        push_quest_event(&data, make_event_key("iron_ingot_0"));
        push_quest_event(&data, make_event_key("slag_0"));
        sys.tick(&mut world, 0.0, &data);
        assert_eq!(
            world.get::<&QuestTracker>(player).unwrap().active_quests[0].current_step,
            smelt + 1,
            "smelting with graphite finishes First Steps' smelting step"
        );
    }

    #[test]
    fn craft_event_completes_quest_and_chains_prerequisite() {
        let mut reg = QuestRegistry::default();
        reg.quests.insert(
            "q_craft".into(),
            quest(
                "q_craft",
                QuestObjective::Craft { recipe_id: "smelt_iron".into(), quantity: 1 },
                vec![],
                None,
            ),
        );
        reg.quests.insert(
            "q_next".into(),
            quest(
                "q_next",
                QuestObjective::Gather { item_id: "iron_ingot_0".into(), quantity: 1 },
                vec![],
                Some("q_craft"),
            ),
        );
        let mut data = DataStore::new();
        data.insert("quest_registry", reg);
        data.insert("quest_events", std::sync::Mutex::new(Vec::<String>::new()));

        let mut world = hecs::World::new();
        let mut tracker = QuestTracker::default();
        tracker.accept_quest("q_craft");
        let player = world.spawn((tracker, Inventory::new(16)));
        let mut sys = QuestSystem::new();

        sys.tick(&mut world, 0.0, &data);
        assert!(!world.get::<&QuestTracker>(player).unwrap().is_completed("q_craft"));

        // Emit the craft event CraftingSystem would push on completing smelt_iron.
        push_quest_event(&data, "craft_smelt_iron".to_string());
        sys.tick(&mut world, 0.0, &data);

        let t = world.get::<&QuestTracker>(player).unwrap();
        assert!(t.is_completed("q_craft"), "craft event completed the Craft quest");
        assert!(
            t.is_active("q_next"),
            "completing q_craft auto-accepts its dependent q_next"
        );
    }

    /// TRAVEL STEPS READ WHERE THE PLAYER IS (2026-10-04, the first-hour audit's
    /// B5). The emitter read the player entity's Transform, which only spawning
    /// and a teleport set: walking moves the camera (in first person the camera
    /// IS the player, lib.rs's walk), so a Travel step never fired however far
    /// the player walked. It now reads "camera_position", the home-local
    /// position the game publishes every frame for its proximity checks (the
    /// walk-up interaction reads the same). Here the player entity stays at the
    /// front door, where spawning left it, while the camera walks to the shipped
    /// outdoor fields.
    ///
    /// Red, run on the emitter before this: "walking to the outdoor fields
    /// completes Initial Survey's Travel step" (left: 1, right: 2).
    #[test]
    fn walking_to_a_destination_completes_its_travel_step() {
        use crate::ecs::components::{Controllable, Transform};
        use glam::Vec3;
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let reg = QuestRegistry::from_ron_dir(&root.join("data/quests"));
        let travel = reg
            .get("exploration_first_survey")
            .expect("Initial Survey ships")
            .steps
            .iter()
            .position(|s| matches!(&s.objective, QuestObjective::Travel { destination } if destination == "outdoor_fields"))
            .expect("Initial Survey walks to the outdoor fields");
        let dests = DestinationList::from_ron(&std::fs::read(root.join("data/entities/destinations.ron")).unwrap()).unwrap();
        let fields = dests.destinations.iter().find(|d| d.id == "outdoor_fields").expect("the fields are a destination").pos;
        let mut data = DataStore::new();
        data.insert("quest_registry", reg);
        data.insert("quest_destinations", dests);
        data.insert("quest_events", std::sync::Mutex::new(Vec::<String>::new()));

        let door = Vec3::new(53.5, 1.7, 40.5);
        let mut tracker = QuestTracker::default();
        tracker.accept_quest("exploration_first_survey");
        tracker.active_quests[0].current_step = travel; // the compass is made
        let mut world = hecs::World::new();
        let player = world.spawn((Transform { position: door, ..Default::default() }, Controllable, tracker, Inventory::new(8)));
        let mut sys = QuestSystem::new();

        data.insert("camera_position", door);
        sys.tick(&mut world, 0.1, &data);
        assert_eq!(world.get::<&QuestTracker>(player).unwrap().active_quests[0].current_step, travel, "nothing fires at the door");
        // Walk there: only the camera moves, as in play.
        data.insert("camera_position", Vec3::new(fields.0, 1.7, fields.1));
        sys.tick(&mut world, 0.1, &data);
        assert_eq!(
            world.get::<&QuestTracker>(player).unwrap().active_quests[0].current_step,
            travel + 1,
            "walking to the outdoor fields completes Initial Survey's Travel step"
        );
    }

    /// The ore two exploration quests gather comes from a node a player can
    /// walk to (2026-10-04, B5). "Initial Survey" asks for 5 ore samples and
    /// "Distant Expeditions" for 3 rare ore, which only two creatures dropped
    /// that nothing spawns (tests/recipe_sources_lint.rs now fails on any
    /// Gather item with no source). Their sources are resource nodes in
    /// data/entities/wild_spawns.ron; this holds each such placement, scatter
    /// radius and all, inside a room of the home or a zone of the ship (ship
    /// metres, the home on plot p1 where offline play puts it), so the source
    /// is somewhere a player can stand, not in a wall or the void between.
    ///
    /// Red, run on the shipped data before this: "no placed node yields
    /// ore_sample_0" (nothing in wild_spawns.ron gave either ore).
    #[test]
    fn quest_ore_comes_from_nodes_a_player_can_walk_to() {
        use crate::systems::livestock::{CreatureRegistry, WildSpawnList};
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let creatures = CreatureRegistry::from_csv(&std::fs::read(root.join("data/creatures.csv")).unwrap()).unwrap();
        let spawns = WildSpawnList::from_ron(&std::fs::read(root.join("data/entities/wild_spawns.ron")).unwrap()).unwrap();
        let ship = crate::ship::ship_structure::ShipStructure::load_and_assemble_shipped(&root.join("data"), None)
            .expect("the shipped ship assembles");
        // Walkable boxes in ship metres, (id, x0, x1, z0, z1): the home's rooms and every other zone.
        let mut boxes: Vec<(String, f32, f32, f32, f32)> = Vec::new();
        let home_i = ship.home_zone_index();
        for (i, z) in ship.zones.iter().enumerate() {
            if i == home_i {
                for r in z.body.zones.iter().filter(|r| r.origin.0 >= 0.0 && r.origin.2 >= 0.0 && r.origin.0 + r.size.0 <= z.body.width && r.origin.2 + r.size.2 <= z.body.depth) {
                    let (x0, z0) = (z.origin.0 + r.origin.0, z.origin.2 + r.origin.2);
                    boxes.push((r.id.clone(), x0, x0 + r.size.0, z0, z0 + r.size.2));
                }
            } else {
                boxes.push((z.id.clone(), z.origin.0, z.origin.0 + z.body.width, z.origin.2, z.origin.2 + z.body.depth));
            }
        }
        assert!(boxes.len() > 20, "the home's rooms and the ship's zones, got {}", boxes.len());

        for item in ["ore_sample_0", "rare_ore_0"] {
            let nodes: Vec<_> = spawns
                .spawns
                .iter()
                .filter(|s| creatures.get(&s.creature).and_then(|d| d.renewable()).is_some_and(|p| p.item == item))
                .collect();
            assert!(!nodes.is_empty(), "no placed node yields {item}");
            for s in nodes {
                let inside = boxes.iter().find(|(_, x0, x1, z0, z1)| {
                    s.pos.0 - s.radius >= *x0 && s.pos.0 + s.radius <= *x1 && s.pos.1 - s.radius >= *z0 && s.pos.1 + s.radius <= *z1
                });
                assert!(
                    inside.is_some(),
                    "{} ({item}) at ({}, {}) with radius {} is not inside any room of the home or zone of the ship",
                    s.creature,
                    s.pos.0,
                    s.pos.1,
                    s.radius
                );
            }
        }
    }

    // ── The starter quests say where and what next (2026-10-04, the
    //    first-hour audit's F3) ──

    /// The shipped quests, items and skills, and the event and notice
    /// channels the game registers.
    fn shipped_quest_data() -> DataStore {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut data = DataStore::new();
        data.insert("quest_registry", QuestRegistry::from_ron_dir(&root.join("data/quests")));
        data.insert(
            "item_registry",
            crate::systems::inventory::ItemRegistry::from_csv(&std::fs::read(root.join("data/items.csv")).unwrap()).unwrap(),
        );
        data.insert(
            "skill_registry",
            crate::systems::skills::SkillRegistry::from_csv(&std::fs::read(root.join("data/skills/skills.csv")).unwrap()).unwrap(),
        );
        data.insert("quest_events", std::sync::Mutex::new(Vec::<String>::new()));
        data.insert("player_notices", std::sync::Mutex::new(Vec::<String>::new()));
        data
    }

    fn notices(data: &DataStore) -> Vec<String> {
        std::mem::take(&mut *data.get::<std::sync::Mutex<Vec<String>>>("player_notices").unwrap().lock().unwrap())
    }

    /// THE IRON STEP SAYS WHERE. It read "Acquire 3 iron ore (mine it with a
    /// drone, or stock it)": the drone is on the Inventory page, which it never
    /// said, and "stock it" is a Dev-mode button. It now names the two places a
    /// new player gets ore, and this holds them true: the Inventory page has a
    /// Mining section, and every shipped home places a trading post. (It was
    /// the first step; since 2026-10-04 it is the fifth of the opening's, and
    /// every step's fit in the HUD's line is held by
    /// opening_tests::every_step_of_every_shipped_quest_fits_the_hud_line.)
    ///
    /// Red, run on the shipped quest before this: "the first step names the
    /// Inventory page's Mining section: Acquire 3 iron ore (mine it with a
    /// drone, or stock it)".
    #[test]
    fn the_iron_step_says_where_to_get_iron_ore() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let reg = QuestRegistry::from_ron_dir(&root.join("data/quests"));
        let q = reg.get("gs_first_steps").expect("First Steps ships");
        let iron = &q
            .steps
            .iter()
            .find(|s| matches!(&s.objective, QuestObjective::Gather { item_id, .. } if item_id == "iron_ore_0"))
            .expect("First Steps gathers iron ore")
            .description;
        assert!(iron.contains("Inventory > Mining"), "the iron step names the Inventory page's Mining section: {iron}");
        assert!(iron.contains("trading post"), "the iron step names the trading post: {iron}");
        assert!(!iron.contains("stock"), "the iron step points at no Dev-mode button: {iron}");
        // The places it names exist.
        let inventory = std::fs::read_to_string(root.join("src/gui/pages/inventory.rs")).expect("the Inventory page source");
        assert!(inventory.contains("\"Mining\", tree_force"), "the Inventory page has a Mining section");
        for file in ["home.ron", "home_solo.ron"] {
            let home = crate::machines::MachineHome::load(&root.join("data/machines").join(file)).unwrap_or_else(|| panic!("{file} parses"));
            assert!(
                home.all_instances().iter().any(|i| i.machine == "trading_post" && i.zone == "home"),
                "{file} places a trading post in the home, where the iron step sends the player"
            );
        }
    }

    /// A FINISHED QUEST TELLS THE PLAYER. Completion was written only to the
    /// log, so First Steps finished, its two ingots arrived and Toolsmith began
    /// with nothing on screen to say so. It now posts on "player_notices", the
    /// channel the main loop shows as a toast: what finished and what it gave,
    /// then the quest the data says comes next. Since 2026-10-04 a step done
    /// on the way posts its own line too (`step_notice`).
    ///
    /// Red, run on the system before this: "finishing First Steps tells the
    /// player" (left: [], right: the two lines below).
    #[test]
    fn a_finished_quest_tells_the_player_what_it_gave_and_what_is_next() {
        let mut data = shipped_quest_data();
        let mut tracker = QuestTracker::default();
        tracker.accept_quest("gs_first_steps");
        let steps = &data.get::<QuestRegistry>("quest_registry").unwrap().get("gs_first_steps").unwrap().steps;
        let last = steps.len() - 1;
        let smelt = steps
            .iter()
            .position(|s| matches!(&s.objective, QuestObjective::Make { item_id, .. } if item_id == "iron_ingot_0"))
            .expect("First Steps smelts an iron ingot");
        tracker.active_quests[0].current_step = smelt;
        let mut world = hecs::World::new();
        world.spawn((tracker, Inventory::new(16)));
        let mut sys = QuestSystem::new();
        push_quest_event(&data, make_event_key("iron_ingot_0"));
        sys.tick(&mut world, 0.0, &data); // the ingot: a step done, the quest goes on
        let said = notices(&data);
        assert_eq!(said.len(), 1, "a step done says so once: {said:?}");
        assert!(said[0].starts_with(&format!("First Steps: step {} of {} done. Next: ", smelt + 1, last + 1)), "{said:?}");
        // The last step, out of the front door (the opening's own test walks there).
        data.insert(
            FRONT_DOOR_KEY,
            FrontDoor { door: (55.0, 40.0), floor: 0.0, outward: (1.0, 0.0), opens_on: "The Commons".into() },
        );
        data.insert(
            "quest_destinations",
            DestinationList::from_ron(&std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/entities/destinations.ron")).unwrap()).unwrap(),
        );
        push_quest_event(&data, "build_storage_chest".to_string());
        sys.tick(&mut world, 0.0, &data);
        let _ = notices(&data);
        data.insert("camera_position", glam::Vec3::new(56.5, 1.7, 40.0));
        sys.tick(&mut world, 0.0, &data);
        assert_eq!(
            notices(&data),
            vec![
                "Quest complete: First Steps. You received 2 Iron Ingot and 30 Metalworking XP.".to_string(),
                "New quest: Toolsmith. Forge a hammer: Esc > Crafting > Craft Hammer > Craft.".to_string(),
            ],
            "finishing First Steps tells the player"
        );
    }

    /// THE STARTER QUESTS RUN ON. After Toolsmith the HUD's quest line went
    /// blank: Build First Habitat named no prerequisite, so the chaining never
    /// reached it and a new player had to find it on the Quests page. The data
    /// now says it follows Toolsmith, and the chaining accepts it the moment
    /// Toolsmith is done.
    ///
    /// Red, run on the shipped quests before this: "finishing Toolsmith
    /// accepts Build First Habitat".
    #[test]
    fn finishing_toolsmith_starts_build_first_habitat() {
        let data = shipped_quest_data();
        let mut tracker = QuestTracker::default();
        tracker.accept_quest("gs_toolsmith");
        let mut world = hecs::World::new();
        let player = world.spawn((tracker, Inventory::new(16)));
        let mut sys = QuestSystem::new();
        push_quest_event(&data, "craft_craft_hammer".to_string());
        sys.tick(&mut world, 0.0, &data);
        let t = world.get::<&QuestTracker>(player).unwrap();
        assert!(t.is_completed("gs_toolsmith"), "the hammer finishes Toolsmith");
        assert!(t.is_active("tutorial_first_habitat"), "finishing Toolsmith accepts Build First Habitat");
    }
}
