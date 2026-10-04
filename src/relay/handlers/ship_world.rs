//! THE RELAY'S WORLD IS THE SHIP (increment 3 of docs/design/ship-homes-and-logistics.md).
//!
//! Until increment 3 the relay simulated the Pioneer, a six-room frigate read from
//! data/ships/starter_fleet.ron, while every game drew the mothership from
//! data/blueprints/ship_structure.ron. Both used the same numbers as positions, so the crew,
//! walking the Pioneer's rooms at x 0..18 and z 0..20, were drawn inside the home on plot p1
//! (x 0..55, z 0..89), and a player who walked into the Commons was in "no room" as far as the
//! relay knew. Now the relay builds its rooms from the ship file itself:
//!
//!   - every ZONE of the ship file is a room. The ship file holds only the shared spaces (the
//!     Commons, the streets): homes are not zones there, each player's home is put on their plot
//!     by the game (`ShipStructure::assemble`), so "every zone" is exactly "every place outside
//!     the plots";
//!   - every labelled VOLUME inside a zone's body (`HomeStructure::zones`, the same records that
//!     name a home's rooms) is a room of its own, inside its zone: the mess hall in the Commons.
//!     A point in both is in the smaller one (`GameWorld::room_for_position`).
//!
//! The ship's DISTRICTS (the hangar, the Concourse, the reactor, ...) are not rooms: they are
//! labelled areas with no floor and no corridor in (ship_structure.ron's header), so no person
//! can walk into one yet, and a quest step or a chore there could never be reached.
//!
//! THE CREW (data/npc/crew.ron) each have a post, a room, and work through their chores
//! (data/npc/chores.ron), each at a place and a spot in it. No chore site may lie on a plot: a
//! chore there is dropped at load with a warning, and the relay tests check the shipped files
//! put none there. The crew walk in straight lines between sites (no pathfinding yet), so the
//! shipped sites all stand in the Commons, a single box no plot touches: a straight walk
//! between two points in a box never leaves it.
//!
//! THE EXPLORE QUEST visits every room, then asks the player to find their own home: the step
//! reads the plot the relay holds for them (`set_home_plot`), and counts only once every shared
//! place has been visited, because every player arrives at their own door and would otherwise
//! finish the step on the spot.
//!
//! THE STORED WORLD of the Pioneer (`PREVIOUS_PERSIST_KEY`) is upgraded once at startup
//! (`upgrade_previous_world`): what it says about players is kept, the Pioneer-built crew and
//! furniture are not.

use super::game_state::{DoorDef, GameEntity, GameWorld, QuestProgress, RoomInfo, ShipRoom};
use crate::ship::ship_structure::{PlotArrival, ShipStructure};

/// Height over the floor where the relay stands a figure (the crew, the furniture): their
/// centre, a metre up. The game grounds every crew member at the same height over its floor
/// (net/sync.rs `NPC_LOCAL_STANDING_Y`, engine/net_route.rs `CREW_POSITION_OVER_FEET_M`).
pub const STANDING_Y_M: f32 = 1.0;

/// The id of the explore quest's last step, "find your home". `visited` holds it once done, and
/// a `game_quest_progress` names it as its `step_id`.
pub const HOME_STEP: &str = "home";

/// A compass word for a move of (dx, dz) across the ship's floor, by its larger part. The
/// game's compass (gui/pages/hud.rs) puts north at yaw 0, and a camera at yaw 0 looks along -z
/// (renderer/camera.rs `forward_xz`: (sin yaw, 0, -cos yaw)), so north is -z, east +x, south
/// +z and west -x. The Pioneer's doors named their own compass words in its data; the ship's
/// corridors carry none, so they are worked out here.
pub fn compass(dx: f32, dz: f32) -> &'static str {
    if dx.abs() > dz.abs() {
        if dx > 0.0 { "east" } else { "west" }
    } else if dz > 0.0 {
        "south"
    } else {
        "north"
    }
}

/// The deck words for a floor at height `y`. The ship has one deck today (every zone's floor is
/// at y 0); a second deck reads by its height until the ship file names decks.
fn deck_name(y: f32) -> String {
    if y.abs() < 0.01 { "Main deck".to_string() } else { format!("Deck at {y:.1} m") }
}

/// The centre of a room's floor plan, ship metres (x, z).
fn centre_xz(r: &ShipRoom) -> (f32, f32) {
    (r.position[0] + r.size[0] * 0.5, r.position[2] + r.size[2] * 0.5)
}

/// Every room of the ship file (see the top of this file): its zones, then the labelled
/// volumes inside each zone. Each zone's exits are its corridors to other zones (both ways)
/// and to the plots whose doors open on it ("plot:<id>"); a volume inside a zone has one exit,
/// to its zone, and its zone one to it. `equipment` is left empty: the world furnishes rooms
/// (`GameWorld::populate_ship_entities`) by `room_type`.
///
/// A room's `room_type` is its zone's purpose ("commons", "street"), or a volume's zone type
/// ("mess_hall"): the key data/ships/room_equipment.ron furnishes it by.
pub fn shared_rooms(ship: &ShipStructure) -> Vec<ShipRoom> {
    let mut rooms: Vec<ShipRoom> = Vec::new();
    for z in &ship.zones {
        let b = &z.body;
        rooms.push(ShipRoom {
            id: z.id.clone(),
            name: if z.label.is_empty() { z.id.clone() } else { z.label.clone() },
            room_type: z.purpose.clone(),
            position: [z.origin.0, z.origin.1, z.origin.2],
            size: [b.width, b.height, b.depth],
            doors: Vec::new(),
            deck_name: deck_name(z.origin.1),
            ship_name: ship.id.clone(),
            equipment: Vec::new(),
        });
    }
    // Corridors between zones, both ways, named by which way they run.
    let zone_ids: Vec<String> = ship.zones.iter().map(|z| z.id.clone()).collect();
    for c in &ship.corridors {
        let (Some(a), Some(b)) = (zone_ids.iter().position(|i| *i == c.from_zone), zone_ids.iter().position(|i| *i == c.to_zone)) else {
            continue;
        };
        let (ca, cb) = (centre_xz(&rooms[a]), centre_xz(&rooms[b]));
        let there = compass(cb.0 - ca.0, cb.1 - ca.1);
        let back = compass(ca.0 - cb.0, ca.1 - cb.1);
        rooms[a].doors.push(DoorDef { connects_to: c.to_zone.clone(), direction: there.to_string() });
        rooms[b].doors.push(DoorDef { connects_to: c.from_zone.clone(), direction: back.to_string() });
    }
    // The plots' doors: a home's corridor opens on a shared zone.
    for p in &ship.plots {
        let Some(zi) = zone_ids.iter().position(|i| *i == p.door.zone) else { continue };
        let cz = centre_xz(&rooms[zi]);
        let cp = (p.origin.0 + p.size.0 * 0.5, p.origin.2 + p.size.2 * 0.5);
        rooms[zi].doors.push(DoorDef { connects_to: format!("plot:{}", p.id), direction: compass(cp.0 - cz.0, cp.1 - cz.1).to_string() });
    }
    // The labelled volumes inside each zone's body: rooms of their own, after every zone.
    for z in &ship.zones {
        for v in z.body.zones.iter().filter(|v| !v.label.trim().is_empty()) {
            let room = ShipRoom {
                id: v.id.clone(),
                name: v.label.clone(),
                room_type: v.type_id.clone(),
                position: [z.origin.0 + v.origin.0, z.origin.1 + v.origin.1, z.origin.2 + v.origin.2],
                size: [v.size.0, v.size.1, v.size.2],
                doors: Vec::new(),
                deck_name: deck_name(z.origin.1),
                ship_name: ship.id.clone(),
                equipment: Vec::new(),
            };
            let Some(zi) = rooms.iter().position(|r| r.id == z.id) else { continue };
            let (cz, cv) = (centre_xz(&rooms[zi]), centre_xz(&room));
            let mut room = room;
            room.doors.push(DoorDef { connects_to: z.id.clone(), direction: compass(cz.0 - cv.0, cz.1 - cv.1).to_string() });
            rooms[zi].doors.push(DoorDef { connects_to: v.id.clone(), direction: compass(cv.0 - cz.0, cv.1 - cz.1).to_string() });
            rooms.push(room);
        }
    }
    rooms
}

/// True when `p` lies inside `room`'s box (its faces included).
pub fn room_contains(room: &ShipRoom, p: [f32; 3]) -> bool {
    (0..3).all(|k| p[k] >= room.position[k] && p[k] <= room.position[k] + room.size[k])
}

/// The plot whose floor plan holds `p` (x and z, edges included), if any. Across the floor
/// only: a figure standing over a plot at any height is on that plot.
pub fn plot_at(plots: &[PlotArrival], p: [f32; 3]) -> Option<&PlotArrival> {
    plots.iter().find(|pl| {
        p[0] >= pl.origin.0 && p[0] <= pl.origin.0 + pl.size.0 && p[2] >= pl.origin.2 && p[2] <= pl.origin.2 + pl.size.2
    })
}

/// A point of a room at standing height, from room-local (x, z) metres (None: its middle).
pub fn room_point(room: &ShipRoom, spot: Option<(f32, f32)>) -> [f32; 3] {
    let (x, z) = spot.unwrap_or((room.size[0] * 0.5, room.size[2] * 0.5));
    [room.position[0] + x, room.position[1] + STANDING_Y_M, room.position[2] + z]
}

impl GameWorld {
    /// Build the world's rooms from the ship file (see the top of this file); the ship file's
    /// id names the world (`ship_name`). The ship the relay loaded its plots from (the same
    /// `ShipStructure::ship_for_relay`), so the rooms and the plots can never disagree.
    pub(crate) fn load_ship_rooms(&mut self, ship: &ShipStructure) {
        self.rooms = shared_rooms(ship);
        self.ship_name = ship.id.clone();
        tracing::info!(
            "Game: the world is the ship {} with {} room(s): {}",
            self.ship_name,
            self.rooms.len(),
            self.rooms.iter().map(|r| r.id.as_str()).collect::<Vec<_>>().join(", ")
        );
    }

    /// The room a point is in: the SMALLEST room whose box holds it, so a point in the mess
    /// hall is in the mess hall, not just in the Commons around it.
    pub fn room_for_position(&self, position: [f32; 3]) -> Option<RoomInfo> {
        self.rooms
            .iter()
            .filter(|r| room_contains(r, position))
            .min_by(|a, b| {
                let va = a.size[0] * a.size[1] * a.size[2];
                let vb = b.size[0] * b.size[1] * b.size[2];
                va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|r| self.room_info(r))
    }

    /// Where someone with no plot and no named spawn arrives: the ship's guest spot (the
    /// Commons, `ShipStructure::guest_spawn`, at eye height), else the middle of the first room
    /// at standing height, else the old [0, 1, 0] for a relay with no ship at all.
    pub(crate) fn default_spawn_position(&self) -> [f32; 3] {
        if let Some(g) = self.ship_plots.guest_spawn {
            return [g.x, g.y, g.z];
        }
        self.rooms.first().map(|r| room_point(r, None)).unwrap_or([0.0, 1.0, 0.0])
    }

    /// The explore quest a fresh player starts on (`spawn_player`): visit every room of the
    /// ship, then find your home. `start_room` (the room they spawn in, if any) counts as
    /// visited. The home step is added by `set_home_plot`, once the relay knows their plot.
    pub(crate) fn explore_quest(&self, start_room: Option<&str>) -> serde_json::Value {
        let places: Vec<serde_json::Value> = self.rooms.iter().map(|r| serde_json::json!({ "id": r.id, "name": r.name })).collect();
        serde_json::json!({
            "id": "explore_ship",
            "title": "Find your bearings",
            "description": "Visit every shared place aboard the ship, then find your way back to your own home.",
            "visited": start_room.map(|r| vec![r.to_string()]).unwrap_or_default(),
            "places": places,
            "home_plot": null,
            "total_rooms": self.rooms.len(),
            "complete": false,
            "reward": {
                "xp": 100,
                "reputation": 5,
                "message": "You know your way around the ship now, and your way home.",
            },
        })
    }

    /// Record the plot the relay holds for this player (None for a guest) on their entity
    /// (`home_plot`: id, origin, size) and, while their explore quest is unfinished, give it
    /// its "find your home" step, or take the step away from a guest. Called on every join
    /// (handle_game_join), so a plot handed out afresh or given back is what the quest reads.
    pub fn set_home_plot(&mut self, player_id: u64, plot: Option<&PlotArrival>) {
        let rooms = self.rooms.len();
        let Some(e) = self.entities.get_mut(&player_id) else { return };
        let Some(obj) = e.components.as_object_mut() else { return };
        let plot_json = plot.map(|p| {
            serde_json::json!({
                "id": p.id,
                "origin": [p.origin.0, p.origin.1, p.origin.2],
                "size": [p.size.0, p.size.1, p.size.2],
            })
        });
        obj.insert("home_plot".to_string(), plot_json.unwrap_or(serde_json::Value::Null));
        let Some(q) = obj.get_mut("current_quest") else { return };
        if q.get("id").and_then(|v| v.as_str()) != Some("explore_ship") || q.get("complete").and_then(|v| v.as_bool()).unwrap_or(false) {
            return;
        }
        q["home_plot"] = plot.map(|p| serde_json::json!(p.id)).unwrap_or(serde_json::Value::Null);
        q["total_rooms"] = serde_json::json!(rooms + usize::from(plot.is_some()));
    }

    /// What a player's move to `position` did to their quest, and the greeting it earns: the
    /// first visit to a room counts (and its crew member says hello); once every room is
    /// visited, stepping onto their own plot finishes "find your home". Called by
    /// handle_game_position_update for every accepted move.
    pub fn quest_step_at(&mut self, player_id: u64, position: [f32; 3]) -> (Option<QuestProgress>, Option<(String, String)>) {
        if let Some(room) = self.room_for_position(position) {
            let progress = self.record_room_visit(player_id, &room.id);
            let greeting = if progress.is_some() { self.pick_room_greeting(&room.id) } else { None };
            return (progress, greeting);
        }
        (self.record_home_found(player_id, position), None)
    }

    /// The "find your home" step: Some(progress) the first time the player stands on the plot
    /// their entity names (`home_plot`) after visiting every room on their explore quest.
    fn record_home_found(&mut self, player_id: u64, position: [f32; 3]) -> Option<QuestProgress> {
        let rooms: Vec<String> = self.rooms.iter().map(|r| r.id.clone()).collect();
        let e = self.entities.get_mut(&player_id)?;
        let plot = e.components.get("home_plot").filter(|p| !p.is_null())?.clone();
        let num = |v: &serde_json::Value, k: usize| v.get(k).and_then(|x| x.as_f64()).unwrap_or(f64::NAN) as f32;
        let (o, s) = (plot.get("origin")?, plot.get("size")?);
        let on_plot = position[0] >= num(o, 0) && position[0] <= num(o, 0) + num(s, 0) && position[2] >= num(o, 2) && position[2] <= num(o, 2) + num(s, 2);
        if !on_plot {
            return None;
        }
        let quest = e.components.get_mut("current_quest")?;
        if quest.get("id").and_then(|v| v.as_str()) != Some("explore_ship") || quest.get("complete").and_then(|v| v.as_bool()).unwrap_or(false) {
            return None;
        }
        let visited = quest.get_mut("visited")?.as_array_mut()?;
        let seen = |id: &str| visited.iter().any(|v| v.as_str() == Some(id));
        if seen(HOME_STEP) || !rooms.iter().all(|r| seen(r)) {
            return None;
        }
        visited.push(serde_json::json!(HOME_STEP));
        let visited_count = visited.len();
        let total = quest.get("total_rooms").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let complete = total > 0 && visited_count >= total;
        if complete {
            quest["complete"] = serde_json::Value::Bool(true);
        }
        let plot_id = plot.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        Some(QuestProgress {
            quest_id: "explore_ship".to_string(),
            step_id: HOME_STEP.to_string(),
            room_id: format!("plot:{plot_id}"),
            visited_count,
            total,
            complete,
        })
    }

    /// Upgrade the world the PREVIOUS code stored (`GameWorld::PREVIOUS_PERSIST_KEY`, the
    /// Pioneer's), once, at startup: called by `restore_from_db` when no world of this
    /// version is stored. True when there was one to upgrade.
    ///
    /// What carries over: every player in it is a ghost (no socket survives a restart), so
    /// each one's progress is written to `player_progress` exactly as a restore does for its
    /// own ghosts, and they rejoin with it; the world clock; and the entity-id high-water
    /// mark, so no id is handed out twice. What does not: everything the Pioneer built (its
    /// crew walking its rooms, which the game drew inside plot p1, and the furniture and
    /// windows of its rooms): the freshly built ship's crew, furniture and stores stay. The
    /// old row is then deleted and this world written in its place, so the upgrade runs once.
    ///
    /// The old blob is read loosely (a JSON value, entity by entity), so a field the old code
    /// wrote that this one does not know, or one entity of a shape it cannot read, skips that
    /// entity instead of stopping the relay.
    pub(crate) fn upgrade_previous_world(&mut self, db: &crate::relay::storage::Storage) -> bool {
        let old = match db.load_game_world(GameWorld::PREVIOUS_PERSIST_KEY) {
            Ok(Some(s)) => s,
            Ok(None) => return false,
            Err(e) => {
                tracing::warn!("Could not read the previous stored world: {e}");
                return false;
            }
        };
        let blob: serde_json::Value = serde_json::from_str(&old.snapshot_json).unwrap_or_else(|e| {
            tracing::warn!("The previous stored world does not parse ({e}); only its clock carries over");
            serde_json::Value::Null
        });
        let mut ghosts = 0usize;
        if let Some(entities) = blob.get("entities").and_then(|v| v.as_object()) {
            for (id, raw) in entities {
                let Ok(e) = serde_json::from_value::<GameEntity>(raw.clone()) else {
                    tracing::warn!("Previous stored world: entity {id} is of a shape this relay cannot read; skipped");
                    continue;
                };
                if e.entity_type != "player" {
                    continue; // built by the Pioneer; the ship's own crew and furniture stand instead
                }
                let Some(owner) = e.owner.clone() else { continue };
                let (quest, completed, xp, rep) = progress_of(&e);
                if let Err(err) = db.save_player_progress(&owner, quest.as_deref(), &completed, xp, rep) {
                    tracing::warn!("Could not keep the progress of {owner} from the previous stored world: {err}");
                }
                ghosts += 1;
            }
        }
        let time = blob.get("game_time").and_then(|v| v.as_f64()).unwrap_or(old.game_time);
        let next = blob.get("next_entity_id").and_then(|v| v.as_u64()).unwrap_or(old.next_entity_id);
        if time.is_finite() && time > self.game_time {
            self.game_time = time;
        }
        self.next_entity_id = self.next_entity_id.max(next).max(old.next_entity_id);
        match self.save_to_db(db) {
            Ok(()) => {
                if let Err(e) = db.delete_game_world(GameWorld::PREVIOUS_PERSIST_KEY) {
                    tracing::warn!("Could not delete the previous stored world after upgrading it: {e}");
                }
            }
            Err(e) => tracing::warn!("Could not store the upgraded world ({e}); the previous one is kept and upgraded again next start"),
        }
        tracing::info!(
            "Game: upgraded the previous stored world (the Pioneer's) to the ship: kept the progress of {ghosts} player(s), the clock ({:.0} s) and the id mark ({})",
            self.game_time,
            self.next_entity_id
        );
        true
    }
}

/// A player entity's progress, as `GameWorld::extract_player_progress` reads it, from an
/// entity that is not in the world (the previous stored world's).
fn progress_of(e: &GameEntity) -> (Option<String>, Vec<String>, u64, u64) {
    let c = &e.components;
    let quest = c.get("current_quest").and_then(|q| q.get("id")).and_then(|v| v.as_str()).map(String::from);
    let completed = c
        .get("completed_quests")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let xp = c.get("xp").and_then(|v| v.as_u64()).unwrap_or(0);
    let rep = c.get("reputation").and_then(|v| v.as_u64()).unwrap_or(0);
    (quest, completed, xp, rep)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ship::ship_structure::ShipStructure;

    fn data() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
    }

    /// The shipped ship file, as the relay loads it.
    fn ship() -> ShipStructure {
        ShipStructure::ship_for_relay(&data()).expect("the shipped ship file loads")
    }

    fn temp_db(tag: &str) -> (crate::relay::storage::Storage, std::path::PathBuf) {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_shipworld_{tag}_{}_{nanos}.db", std::process::id()));
        (crate::relay::storage::Storage::open(&path).expect("open test db"), path)
    }

    fn fmt3(p: [f32; 3]) -> String {
        format!("({:.1}, {:.1}, {:.1})", p[0], p[1], p[2])
    }

    /// THE WORLD IS BUILT FROM THE SHIP FILE: one room for every zone of it (its box, its
    /// label, its purpose) and one for every labelled volume in a zone's body, and no other.
    /// No room stands on a plot, and the world is named by the ship file's id.
    ///
    /// Seen red 2026-10-04: SEE_RED_ROOMS
    #[test]
    fn the_world_is_built_from_the_ship_file() {
        let ship = ship();
        let world = GameWorld::new();
        let mut want: Vec<String> = ship.zones.iter().map(|z| z.id.clone()).collect();
        for z in &ship.zones {
            want.extend(z.body.zones.iter().filter(|v| !v.label.trim().is_empty()).map(|v| v.id.clone()));
        }
        want.sort();
        let mut got: Vec<String> = world.rooms.iter().map(|r| r.id.clone()).collect();
        got.sort();
        assert_eq!(got, want, "the world's rooms are not the ship file's places");
        assert!(want.iter().any(|w| w == "mess-hall"), "the shipped Commons names its mess hall: {want:?}");
        assert_eq!(world.ship_name, ship.id, "the world is named by the ship file's id");
        for z in &ship.zones {
            let r = world.rooms.iter().find(|r| r.id == z.id).unwrap();
            assert_eq!(r.position, [z.origin.0, z.origin.1, z.origin.2], "{} stands where the ship file puts it", z.id);
            assert_eq!(r.size, [z.body.width, z.body.height, z.body.depth], "{} is the size of its zone", z.id);
            assert_eq!(r.room_type, z.purpose, "{} is of its zone's purpose", z.id);
        }
        let plots = &world.ship_plots.plots;
        assert!(plots.len() >= 2, "the shipped ship has plots");
        for r in &world.rooms {
            for pl in plots {
                let apart = r.position[0] + r.size[0] <= pl.origin.0
                    || pl.origin.0 + pl.size.0 <= r.position[0]
                    || r.position[2] + r.size[2] <= pl.origin.2
                    || pl.origin.2 + pl.size.2 <= r.position[2];
                assert!(apart, "room {} overlaps plot {}", r.id, pl.id);
            }
        }
        // Doors: the Commons opens on First Street (south, +z), on plot p1's corridor (west),
        // and on its own mess hall.
        let commons = world.room_by_id("commons").expect("a Commons");
        assert!(commons.exits.iter().any(|e| e.connects_to == "street-1" && e.direction == "south"), "{:?}", commons.exits);
        assert!(commons.exits.iter().any(|e| e.connects_to == "plot:p1" && e.direction == "west"), "{:?}", commons.exits);
        assert!(commons.exits.iter().any(|e| e.connects_to == "mess-hall"), "{:?}", commons.exits);
    }

    /// A point is in the SMALLEST room holding it: the mess hall inside the Commons wins, the
    /// rest of the Commons is the Commons, and a home's floor is in no room.
    ///
    /// Seen red 2026-10-04: SEE_RED_SMALLEST
    #[test]
    fn a_point_is_in_the_smallest_room_holding_it() {
        let world = GameWorld::new();
        assert_eq!(world.room_for_position([80.0, 1.7, 24.0]).map(|r| r.id).as_deref(), Some("mess-hall"));
        assert_eq!(world.room_for_position([90.0, 1.7, 50.0]).map(|r| r.id).as_deref(), Some("commons"));
        assert_eq!(world.room_for_position([70.0, 1.7, 150.0]).map(|r| r.id).as_deref(), Some("street-1"));
        assert_eq!(world.room_for_position([53.5, 1.7, 40.5]).map(|r| r.id), None, "p1's door is in no room of the ship");
    }

    /// NO CHORE TARGET LIES INSIDE A PLOT BOX (the design's proof for increment 3), read from
    /// the shipped chores file itself, before `load_chores` filters anything (it drops a chore
    /// that stands on a plot, which would hide one here): every chore's site is in its own
    /// place and on no plot, and every chore loaded. The crew's starting spots and the food
    /// stores, and everything else the world stands up, too.
    ///
    /// Seen red 2026-10-04: SEE_RED_CHORES
    #[test]
    fn no_chore_target_lies_inside_a_plot_box() {
        let world = GameWorld::new();
        let text = std::fs::read_to_string(data().join("npc/chores.ron")).expect("chores.ron");
        let chores: Vec<super::super::game_state::ChoreDef> = ron::from_str(&text).expect("chores.ron parses");
        assert!(!chores.is_empty());
        let plots = &world.ship_plots.plots;
        for c in &chores {
            let room = world.rooms.iter().find(|r| r.id == c.place).unwrap_or_else(|| panic!("chore {} names unknown place {}", c.id, c.place));
            let site = room_point(room, Some(c.spot));
            assert!(room_contains(room, site), "chore {}'s site {} is outside {}", c.id, fmt3(site), c.place);
            if let Some(p) = plot_at(plots, site) {
                panic!("chore {}'s site {} is on plot {}", c.id, fmt3(site), p.id);
            }
        }
        assert_eq!(world.chores.len(), chores.len(), "load_chores dropped a shipped chore");
        for (id, e) in &world.entities {
            if let Some(p) = plot_at(plots, e.position) {
                panic!("entity {id} ({}) stands on plot {} at {}", e.entity_type, p.id, fmt3(e.position));
            }
        }
    }

    /// NO CREW FIGURE IS EVER ON A PLOT, and the crew stay in the Commons: two hours of the
    /// relay's clock with a meal due every 15 minutes (so every eater walks to the mess hall and
    /// back several times), every crew position after every second and every position the
    /// relay sends the games (`game_npc_update`), on no plot and inside the Commons' box.
    ///
    /// Seen red 2026-10-04: SEE_RED_CREW
    #[test]
    fn no_crew_figure_is_ever_on_a_plot() {
        let mut world = GameWorld::new();
        super::super::ship_stores::meals_every(&mut world, 0.25);
        let commons = world.rooms.iter().find(|r| r.id == "commons").cloned().expect("a Commons");
        let plots = world.ship_plots.plots.clone();
        let crew: Vec<u64> = world.entities.iter().filter(|(_, e)| e.components.get("chore_agent").is_some()).map(|(id, _)| *id).collect();
        assert!(crew.len() >= 5, "the shipped crew stand in the world: {}", crew.len());
        for step in 0..(2 * 3600 * 4) {
            for ev in world.tick(0.25) {
                if let Some(p) = plot_at(&plots, ev.position) {
                    panic!("crew {} sent at {} is on plot {} (t {:.1} s)", ev.name, fmt3(ev.position), p.id, world.game_time);
                }
                assert!(room_contains(&commons, ev.position), "crew {} sent at {} is outside the Commons", ev.name, fmt3(ev.position));
            }
            if step % 4 == 0 {
                for id in &crew {
                    let e = &world.entities[id];
                    let name = e.components["name"].as_str().unwrap_or("?");
                    if let Some(p) = plot_at(&plots, e.position) {
                        panic!("crew {name} at {} is on plot {} (t {:.1} s)", fmt3(e.position), p.id, world.game_time);
                    }
                    assert!(room_contains(&commons, e.position), "crew {name} at {} is outside the Commons", fmt3(e.position));
                }
            }
        }
        let meals: u64 = crew.iter().map(|id| world.entities[id].components.get("meals_eaten").and_then(|v| v.as_u64()).unwrap_or(0)).sum();
        assert!(meals >= 10, "the crew walked to their meals and back ({meals} eaten in two hours)");
    }

    /// The walls of the shipped ship as everyone walks against them (the rig's own list,
    /// `wall_collision::everyones_walls`, with the home on its default plot).
    fn walls() -> Vec<crate::ship::wall_collision::WallSegment> {
        let ship = ShipStructure::load_and_assemble_shipped(&data(), None).expect("the shipped ship assembles");
        crate::ship::wall_collision::everyones_walls(&ship)
    }

    /// Closest distance between two segments on the floor (x, z); 0 when they cross.
    fn seg_dist(p: (f32, f32), q: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
        let cross = |o: (f32, f32), u: (f32, f32), v: (f32, f32)| (u.0 - o.0) * (v.1 - o.1) - (u.1 - o.1) * (v.0 - o.0);
        let (d1, d2, d3, d4) = (cross(a, b, p), cross(a, b, q), cross(p, q, a), cross(p, q, b));
        if ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0)) {
            return 0.0;
        }
        let pt = |x: (f32, f32), s: (f32, f32), e: (f32, f32)| {
            let (dx, dz) = (e.0 - s.0, e.1 - s.1);
            let l2 = dx * dx + dz * dz;
            let t = if l2 < 1e-9 { 0.0 } else { (((x.0 - s.0) * dx + (x.1 - s.1) * dz) / l2).clamp(0.0, 1.0) };
            ((x.0 - s.0 - dx * t).powi(2) + (x.1 - s.1 - dz * t).powi(2)).sqrt()
        };
        pt(p, a, b).min(pt(q, a, b)).min(pt(a, p, q)).min(pt(b, p, q))
    }

    /// EVERY CREW WALK BETWEEN ITS SITES IS CLEAR OF WALLS: the crew walk in straight lines
    /// (no pathfinding yet), so every pair of sites one crew member goes between (its role's
    /// chores, and its meal when it eats) must have no wall across it, with a body's width
    /// (0.3 m) of room either side. Walls are everyone's (the rig's list).
    ///
    /// Seen red 2026-10-04: SEE_RED_WALLS
    #[test]
    fn every_crew_walk_between_its_sites_is_clear_of_walls() {
        let world = GameWorld::new();
        let walls = walls();
        assert!(walls.len() > 20, "the ship's walls are known: {}", walls.len());
        for e in world.entities.values().filter(|e| e.components.get("chore_agent").is_some()) {
            let name = e.components["name"].as_str().unwrap_or("?");
            let role = e.components["role"].as_str().unwrap_or("");
            let eats = e.components.get("eats").and_then(|v| v.as_bool()).unwrap_or(false);
            let sites: Vec<(&str, [f32; 3])> = world
                .chores
                .iter()
                .filter(|c| (c.roles.is_empty() || c.roles.iter().any(|r| r == role)) && (!c.meal || eats))
                .map(|c| (c.id.as_str(), world.chore_site_of(c).expect("a valid site")))
                .collect();
            assert!(sites.len() >= 2, "{name} has at least two sites: {sites:?}");
            for (i, (ia, a)) in sites.iter().enumerate() {
                for (ib, b) in sites.iter().skip(i + 1) {
                    for w in &walls {
                        let d = seg_dist((a[0], a[2]), (b[0], b[2]), w.a, w.b);
                        assert!(
                            d > w.half_thickness + 0.3,
                            "{name} walks from {ia} {} to {ib} {} through the wall ({:.1}, {:.1})-({:.1}, {:.1})",
                            fmt3(*a),
                            fmt3(*b),
                            w.a.0,
                            w.a.1,
                            w.b.0,
                            w.b.1
                        );
                    }
                }
            }
        }
    }

    /// AN OLD STORED WORLD UPGRADES: a database holding the world the PREVIOUS code stored
    /// (the Pioneer's, `game_world_snapshot_v9`) opens on this code without trouble, and its
    /// world becomes the ship's: the players' progress is kept (written to player_progress, as
    /// a restore does for its own ghosts), the clock and the id mark carry over, nothing the
    /// Pioneer built survives (no entity of a Pioneer room, none on a plot), the old row is
    /// deleted and this version's written, and the next start restores that one.
    ///
    /// tests/fixtures/relay/pioneer_world_v9.json is EXACTLY what the previous code wrote,
    /// unedited: produced 2026-10-04 on 1297cde96 (v0.1451.0) by `GameWorld::new()`, a player
    /// "e11e0001" spawned in the Crew Quarters and moved to (4, 5, 12), a visit to the bridge,
    /// 30 s of ticks, then `save_to_db`, and the stored blob copied out.
    ///
    /// Seen red 2026-10-04: SEE_RED_UPGRADE
    #[test]
    fn an_old_stored_world_upgrades_to_the_ship() {
        let (db, path) = temp_db("upgrade");
        let blob = include_str!("../../../tests/fixtures/relay/pioneer_world_v9.json");
        let old: serde_json::Value = serde_json::from_str(blob).expect("the fixture parses");
        let old_time = old["game_time"].as_f64().unwrap();
        let old_next = old["next_entity_id"].as_u64().unwrap();
        db.save_game_world(GameWorld::PREVIOUS_PERSIST_KEY, blob, old_time, old_next).expect("store the old world");
        drop(db);

        // The relay starts on that database (Storage::open runs every migration on it).
        let db = crate::relay::storage::Storage::open(&path).expect("a database holding the Pioneer's world opens");
        let mut world = GameWorld::new();
        let upgraded = world.restore_from_db(&db);
        let progress = db.load_player_progress("e11e0001").expect("progress query");
        let progress = progress.expect("the previous world's player kept no progress");
        assert!(upgraded, "the old world was found and upgraded");
        assert_eq!(progress.current_quest.as_deref(), Some("explore_ship"), "their quest carried over");
        assert!((world.game_time - old_time).abs() < 1e-6, "the clock carried over: {} vs {old_time}", world.game_time);
        assert!(world.next_entity_id >= old_next, "the id mark carried over: {} < {old_next}", world.next_entity_id);
        let pioneer = ["bridge", "quarters", "medbay", "cargo", "engineering", "hydroponics"];
        for (id, e) in &world.entities {
            let room = e.components.get("room_id").and_then(|v| v.as_str()).unwrap_or("");
            assert!(!pioneer.contains(&room), "entity {id} ({}) still names the Pioneer's {room}", e.entity_type);
            assert!(plot_at(&world.ship_plots.plots, e.position).is_none(), "entity {id} ({}) stands on a plot at {}", e.entity_type, fmt3(e.position));
            assert_ne!(e.entity_type, "player", "a ghost player came back into the world");
        }
        assert!(world.entities.values().any(|e| e.entity_type == "food_store"), "the ship's stores stand in the upgraded world");
        assert!(db.load_game_world(GameWorld::PREVIOUS_PERSIST_KEY).unwrap().is_none(), "the old world was deleted after the upgrade");
        assert!(db.load_game_world(GameWorld::PERSIST_KEY).unwrap().is_some(), "the upgraded world was stored");

        // The next start restores this version's world, crew and all.
        let mut again = GameWorld::new();
        assert!(again.restore_from_db(&db), "the upgraded world restores");
        let crew: Vec<&str> = again.entities.values().filter(|e| e.components.get("chore_agent").is_some()).filter_map(|e| e.components["room_id"].as_str()).collect();
        assert!(!crew.is_empty() && crew.iter().all(|r| !pioneer.contains(r)), "the restored crew are the ship's: {crew:?}");
        drop(db);
        let _ = std::fs::remove_file(&path);
    }

    /// A stored world of the previous version that is not JSON at all upgrades too: only its
    /// clock and id mark (its row's own columns) carry over, and the relay starts.
    #[test]
    fn an_unreadable_old_world_still_upgrades() {
        let (db, path) = temp_db("upgrade_bad");
        db.save_game_world(GameWorld::PREVIOUS_PERSIST_KEY, "{ not json", 12.5, 400).unwrap();
        let mut world = GameWorld::new();
        assert!(world.restore_from_db(&db));
        assert!((world.game_time - 12.5).abs() < 1e-9, "the row's clock: {}", world.game_time);
        assert!(world.next_entity_id >= 400, "the row's id mark: {}", world.next_entity_id);
        assert!(db.load_game_world(GameWorld::PREVIOUS_PERSIST_KEY).unwrap().is_none());
        drop(db);
        let _ = std::fs::remove_file(&path);
    }

    /// The plot of the shipped ship with this id, as the relay hands it out.
    fn plot(world: &GameWorld, id: &str) -> PlotArrival {
        world.ship_plots.plot(id).cloned().unwrap_or_else(|| panic!("no plot {id}"))
    }

    /// A point in this room and in no smaller one (the Commons' own floor is outside its mess
    /// hall, which is its north strip).
    fn inside(world: &GameWorld, id: &str) -> [f32; 3] {
        let room = world.rooms.iter().find(|x| x.id == id).cloned().unwrap();
        room_point(&room, if id == "commons" { Some((25.0, 40.0)) } else { None })
    }

    /// "FIND YOUR HOME": the explore quest visits every room of the ship, then counts the
    /// player's own plot, and only then. A player who arrives at their own door does not finish
    /// it on the spot; someone else's plot does not count; a guest's quest has no home step.
    ///
    /// Seen red 2026-10-04: SEE_RED_HOME
    #[test]
    fn the_explore_quest_ends_by_finding_your_home() {
        let mut world = GameWorld::new();
        let p2 = plot(&world, "p2");
        let door = p2.arrival(Some((53.5, 40.5)));
        let home = [door.x, door.y, door.z];
        let id = world.spawn_player("e11e0002", home);
        world.set_home_plot(id, Some(&p2));
        let q = &world.entities[&id].components["current_quest"];
        assert_eq!(q["home_plot"], "p2");
        assert_eq!(q["total_rooms"].as_u64(), Some(world.rooms.len() as u64 + 1), "every room and their home");
        assert_eq!(q["visited"].as_array().map(|v| v.len()), Some(0), "arriving at their own door visits no room");

        let (none, _) = world.quest_step_at(id, home);
        assert!(none.is_none(), "standing on their own plot before visiting the ship counted: {none:?}");

        let rooms: Vec<String> = world.rooms.iter().map(|r| r.id.clone()).collect();
        for (n, r) in rooms.iter().enumerate() {
            let at = inside(&world, r);
            let (got, _) = world.quest_step_at(id, at);
            let got = got.unwrap_or_else(|| panic!("the first visit to {r} counted nothing"));
            assert_eq!(&got.step_id, r);
            assert_eq!(got.visited_count, n + 1);
            assert!(!got.complete, "the quest finished at {r}, before home");
        }

        let p1 = plot(&world, "p1");
        let (theirs, _) = world.quest_step_at(id, [p1.origin.0 + 10.0, 1.7, p1.origin.2 + 10.0]);
        assert!(theirs.is_none(), "someone else's plot counted as home: {theirs:?}");

        let (done, _) = world.quest_step_at(id, home);
        let done = done.expect("their own plot, after every room, finds their home");
        assert_eq!(done.step_id, HOME_STEP);
        assert_eq!(done.room_id, "plot:p2");
        assert!(done.complete, "finding their home finishes the quest: {done:?}");
        assert!(world.quest_step_at(id, home).0.is_none(), "home counts once");

        // A guest has no home step: the last room finishes it.
        let g = world.spawn_player("e11e0003", [0.0, 1.0, 0.0]);
        world.set_home_plot(g, None);
        assert_eq!(world.entities[&g].components["current_quest"]["total_rooms"].as_u64(), Some(rooms.len() as u64));
        let mut finished = false;
        for r in &rooms {
            let at = inside(&world, r);
            if let (Some(p), _) = world.quest_step_at(g, at) {
                finished = p.complete;
            }
        }
        assert!(finished, "a guest's quest finishes with the last room");
    }

    /// A player with no place named arrives in the Commons (the guest spot), which counts as
    /// visited; one who arrives at their own door is in no room.
    ///
    /// Seen red 2026-10-04: SEE_RED_SPAWN
    #[test]
    fn a_player_with_no_place_named_arrives_in_the_commons() {
        let mut world = GameWorld::new();
        let id = world.spawn_player("e11e0004", [0.0, 1.0, 0.0]);
        let at = world.entities[&id].position;
        assert_eq!(world.room_for_position(at).map(|r| r.id).as_deref(), Some("commons"));
        assert_eq!(world.entities[&id].components["current_quest"]["visited"], serde_json::json!(["commons"]));
        let p1 = plot(&world, "p1");
        let door = p1.arrival(Some((53.5, 40.5)));
        let home = world.spawn_player("e11e0005", [door.x, door.y, door.z]);
        assert_eq!(world.entities[&home].components["current_quest"]["visited"], serde_json::json!([]));
    }
}
