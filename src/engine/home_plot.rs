//! Your home on the shared ship (increment 1b of docs/design/ship-homes-and-logistics.md).
//!
//! WHY: a relay now hands every player a plot of the mothership and spawns them on it
//! (src/relay/handlers/game_state.rs `assign_home`). The game first builds the home on the
//! ship's default plot (p1, increment 1a), because it does not know its plot until it joins.
//! If the relay then says p2 and the game kept drawing its home at p1, the player would stand
//! in p1 while the relay holds them at p2's front door, 99 to 139 m away: every update they
//! sent would be refused by the relay's 100 m rule, and everyone else would see them frozen.
//! So the two halves land together, and this file is the game's half.
//!
//! On joining, the game tells the relay which ship it draws and where its own home's door is
//! (`add_join_fields`): a game drawing another ship then takes no plot (it is about to refuse
//! the welcome, and the plot would be held for good by someone who never lives there), and
//! the player arrives at THEIR door, which they may have moved with the build-mode avatar,
//! not at the default design's.
//!
//! On every `game_welcome` the game:
//!   1. checks the relay's ship is its own ship (the `ship.hash`, `ShipStructure::ship_hash`):
//!      positions only agree when everyone has the same ship, so on a mismatch it refuses to
//!      join, with one plain sentence;
//!   2. moves its `home` zone to the plot the relay named, through 1a's assembly (the ship
//!      file plus the home design on that plot plus the plot's door corridor), rebuilds the
//!      home through the editor's rebuild path, and carries along what the world load placed
//!      for the home that the rebuild does not redo (`carry_home_things`: the farm animals,
//!      the decoration plants, the hologram and the showroom stage), and the Respawn point;
//!   3. as a guest (the ship is full, `home_plot` null), makes the Commons its Respawn point;
//!   4. stands the player where the relay holds them (their own entry in the welcome's
//!      `world_snapshot`) when this welcome is an ARRIVAL: the first since the world loaded,
//!      or from another server. On a fresh join that is their door on their plot; after the
//!      game restarted inside the relay's 90 s grace it is where they were. A later welcome
//!      from the same server is a reconnect, and the player keeps walking where they are
//!      (snapping them back to their last update would undo the walk, or pull a player who
//!      went off the ship back aboard), unless the home itself moved from under them.
//!
//! `plan_welcome` decides, and is pure (tested below); `apply_welcome_home` does it.
//!
//! What follows the home on a move: walls, floors, collision, room lights, the sealed bounds,
//! machines (with their grow plots, screens and pipes) and door panels through the rebuild;
//! the livestock, decorations, hologram, showroom stage and Respawn point here. The next world
//! entry still builds the home on the default plot first (the remembered plot is increment 2),
//! and the welcome moves it again.

use crate::engine::state::EngineState;
use crate::ship::ship_structure::ShipStructure;
use glam::Vec3;

/// The sentence a player reads when the server's ship is not theirs (the design: "one plain
/// sentence: positions only agree when everyone has the same ship").
pub(crate) const SHIP_MISMATCH: &str =
    "Not joining the shared world: this server has a different ship from yours, and positions only agree when everyone has the same ship.";

/// What a welcome asks of the home and of where the player stands.
#[derive(Debug)]
pub(crate) enum WelcomeHome {
    /// Do not join: the sentence to show.
    Refuse(String),
    /// The home already stands on the plot the relay named: nothing moves. `stand_at` is where
    /// the player goes (None: they stay where they are).
    Stay { stand_at: Option<Vec3> },
    /// Move the home: the ship re-assembled on `plot`. `door` is where the holder of that plot
    /// arrives with this home (the Respawn button's point from now on), `stand_at` where the
    /// player goes now.
    Move { ship: ShipStructure, plot: String, door: Vec3, stand_at: Vec3 },
    /// The ship is full: a guest. `door` is the Commons arrival (the Respawn point), None for a
    /// ship with no Commons; `stand_at` where the player goes (None: they stay).
    Guest { door: Option<Vec3>, stand_at: Option<Vec3> },
}

/// Decide what a `game_welcome` asks of the home, from the ship the game runs now (`None` when
/// no ship assembled and the legacy layout is showing). `arriving`: this welcome is the first
/// since the world loaded, or from another server (see the module notes). Pure.
pub(crate) fn plan_welcome(ship: Option<&ShipStructure>, welcome: &serde_json::Value, arriving: bool) -> WelcomeHome {
    let theirs = welcome.get("ship").and_then(|s| s.get("hash")).and_then(|h| h.as_str());
    let Some(ship) = ship else { return WelcomeHome::Refuse(SHIP_MISMATCH.to_string()) };
    if theirs != Some(ship.ship_hash().as_str()) {
        return WelcomeHome::Refuse(SHIP_MISMATCH.to_string());
    }
    // Where the relay holds us right now: our own entry in the snapshot.
    let held = relay_holds_us(welcome);
    let Some(hp) = welcome.get("home_plot").filter(|v| !v.is_null()) else {
        let door = ship.guest_spawn();
        return WelcomeHome::Guest { door, stand_at: if arriving { held.or(door) } else { None } };
    };
    let Some(id) = hp.get("id").and_then(|i| i.as_str()) else {
        return WelcomeHome::Refuse(SHIP_MISMATCH.to_string());
    };
    if ship.home.as_ref().is_some_and(|a| a.plot == id) {
        return WelcomeHome::Stay { stand_at: if arriving { held } else { None } };
    }
    let (Some(design), Some(plot)) = (ship.home_design(), ship.plots.iter().find(|p| p.id == id)) else {
        return WelcomeHome::Refuse(SHIP_MISMATCH.to_string());
    };
    let door = ShipStructure::plot_spawn(plot, &design);
    match ship.ship_file().assemble(design, id) {
        // The old home is gone from under the player, so they always go somewhere: where the
        // relay holds them, or this home's door on the new plot.
        Ok(moved) => WelcomeHome::Move { ship: moved, plot: id.to_string(), door, stand_at: held.unwrap_or(door) },
        Err(e) => WelcomeHome::Refuse(format!("Not joining the shared world: your home does not fit the plot this server gave you ({e}).")),
    }
}

/// Our own position in a welcome's `world_snapshot` (the entry whose `entity_id` is the
/// welcome's `player_id`): where the relay holds us. None when the welcome carries no such
/// entry. Ship metres at eye height, the frame the game sends its camera in.
fn relay_holds_us(welcome: &serde_json::Value) -> Option<Vec3> {
    let me = welcome.get("player_id")?.as_u64()?;
    let entry = welcome
        .get("world_snapshot")?
        .as_array()?
        .iter()
        .find(|e| e.get("entity_id").and_then(|i| i.as_u64()) == Some(me))?;
    let p = entry.get("position")?.as_array()?;
    let n = |i: usize| p.get(i).and_then(|v| v.as_f64()).map(|v| v as f32);
    let at = Vec3::new(n(0)?, n(1)?, n(2)?);
    at.is_finite().then_some(at)
}

/// The two fields our `game_join` carries for the plot (lib.rs, the join): `ship_hash`, the
/// ship we draw (empty when none assembled, which can never match, so a game that will refuse
/// claims nothing), and `home_spawn`, our own home's door in plot-local metres
/// (`ShipStructure::home_arrival_local`).
pub(crate) fn add_join_fields(join: &mut serde_json::Value, ship: Option<&ShipStructure>) {
    let Some(obj) = join.as_object_mut() else { return };
    obj.insert("ship_hash".into(), serde_json::json!(ship.map(|s| s.ship_hash()).unwrap_or_default()));
    if let Some((x, z)) = ship.and_then(|s| s.home_arrival_local()) {
        obj.insert("home_spawn".into(), serde_json::json!([x, z]));
    }
}

/// Act on a `game_welcome` (engine/net_route.rs, before the welcome reaches net_sync). Returns
/// false when the game refused to join: the caller then drops the welcome.
pub(crate) fn apply_welcome_home(state: &mut EngineState, welcome: &serde_json::Value) -> bool {
    let arriving = state.home_arrived_on.as_deref() != Some(state.gui_state.server_url.as_str());
    match plan_welcome(state.gui_state.ship_structure.as_ref(), welcome, arriving) {
        WelcomeHome::Refuse(sentence) => {
            log::warn!("Co-presence: {sentence} (server ship {}, ours {:?})", welcome["ship"], state.gui_state.ship_structure.as_ref().map(|s| s.ship_hash()));
            if let Some(ref ws) = state.gui_state.ws_client {
                ws.send(&serde_json::json!({ "type": "game_leave" }).to_string());
            }
            state.game_joined = false;
            state.gui_state.copresence_active = false;
            // Not again on this server until the world is loaded afresh (lib.rs join gate).
            state.copresence_refused = Some(state.gui_state.server_url.clone());
            state.gui_state.pending_notices.push(sentence);
            return false;
        }
        WelcomeHome::Stay { stand_at } => {
            if let Some(at) = stand_at {
                put_player_at(state, at);
            }
        }
        WelcomeHome::Move { ship, plot, door, stand_at } => {
            log::info!("Co-presence: the server gave us plot {plot}; moving the home there");
            // Where things stood before the move, to carry what the rebuild does not redo.
            let before = crate::engine::home_meshes::current_placements(state).unwrap_or_default();
            let old = state.gui_state.ship_structure.as_ref().and_then(home_box);
            let new = home_box(&ship);
            state.gui_state.construction_zone = ship.home_zone_index();
            state.gui_state.ship_structure = Some(ship);
            // The editor's own rebuild, called directly rather than through the dirty flag: a
            // plot assignment is not an edit, so it must not arm the autosave or the undo history.
            crate::engine::home_meshes::rebuild_homestead(state);
            let after = crate::engine::home_meshes::current_placements(state).unwrap_or_default();
            if let (Some(old), Some(new)) = (old, new) {
                carry_home_things(state, &machine_shifts(&before, &after), new.0 - old.0, old);
            }
            // Respawn brings the player back to their own door, on this plot.
            state.fps_spawn = door;
            put_player_at(state, stand_at);
        }
        WelcomeHome::Guest { door, stand_at } => {
            log::info!("Co-presence: the ship is full; joining as a guest in the Commons");
            // A guest's own home is still drawn on the default plot, which is somebody else's:
            // Respawn brings them back to the Commons, where the relay put them.
            if let Some(d) = door {
                state.fps_spawn = d;
            }
            if let Some(at) = stand_at {
                put_player_at(state, at);
            }
        }
    }
    state.game_welcomed = true;
    state.home_arrived_on = Some(state.gui_state.server_url.clone());
    true
}

/// The box of the plot the home stands on, (min, max) in ship metres.
fn home_box(ship: &ShipStructure) -> Option<(Vec3, Vec3)> {
    ship.home_plot().map(|p| p.aabb())
}

/// One machine the move carried: where it stood (x, z) and how far it went.
pub(crate) type MachineShift = ((f32, f32), Vec3);

/// Every machine whose position changed between two placements of the same machines (by id),
/// with where it stood and how far it went. The home's machines go with the home; the ship's
/// (the Commons rows) stay, and so are not listed. Pure.
pub(crate) fn machine_shifts(before: &[crate::machines::PlacedMachine], after: &[crate::machines::PlacedMachine]) -> Vec<MachineShift> {
    let after: std::collections::HashMap<&str, &crate::machines::PlacedMachine> = after.iter().map(|m| (m.id.as_str(), m)).collect();
    before
        .iter()
        .filter_map(|b| {
            let a = after.get(b.id.as_str())?;
            let d = Vec3::new(a.pos.0 - b.pos.0, a.pos.1 - b.pos.1, a.pos.2 - b.pos.2);
            (d != Vec3::ZERO).then_some(((b.pos.0, b.pos.2), d))
        })
        .collect()
}

/// How far something anchored at machine position `at` went: that machine's shift, matched on
/// x and z (a sphere's placement is lifted by its radius, the world load's anchor is not, and
/// two machines never share a footprint point across zones, which may not overlap). Zero for
/// an anchor no moved machine stands at. Pure.
pub(crate) fn shift_of(shifts: &[MachineShift], at: Vec3) -> Vec3 {
    shifts
        .iter()
        .find(|((x, z), _)| (x - at.x).abs() < 1e-3 && (z - at.z).abs() < 1e-3)
        .map_or(Vec3::ZERO, |(_, d)| *d)
}

/// Carry along what the world load placed for the home and the rebuild does not redo, after
/// the home moved by `home_delta` from the box `old_home`:
///   - the farm animals (each grazes around a machine, `Creature.anchor`) and the decoration
///     plants (each scattered around one): by their machine's own shift, so an animal kept
///     near a ship machine stays put;
///   - the solar-system hologram: on an assembled ship its centre is the world load's fallback
///     point at the home's corner (no ship room is a hologram room), and it is part of the home
///     (lib.rs draws it "in the home"), so it moves with the home's origin;
///   - the showroom stage and the avatar standing on it (`avatar_base`, the respawner or
///     wardrobe room), when that room is in the home.
/// Health, yield timers and which animals are still alive are kept: they are moved, not
/// spawned again.
fn carry_home_things(state: &mut EngineState, shifts: &[MachineShift], home_delta: Vec3, old_home: (Vec3, Vec3)) {
    let mut animals = 0;
    for (_e, (c, t)) in state
        .game_world
        .world
        .query_mut::<(&mut crate::ecs::components::Creature, &mut crate::ecs::components::Transform)>()
    {
        let d = shift_of(shifts, c.anchor);
        if d != Vec3::ZERO {
            c.anchor += d;
            t.position += d;
            animals += 1;
        }
    }
    let mut plants = 0;
    for deco in state.decoration_objects.iter_mut() {
        let d = shift_of(shifts, deco.5);
        if d != Vec3::ZERO {
            deco.2 += d;
            deco.5 += d;
            plants += 1;
        }
    }
    state.hologram_room_center += home_delta;
    let b = state.avatar_base;
    let stage_in_home = b.cmpge(old_home.0).all() && b.cmple(old_home.1).all();
    if stage_in_home {
        state.avatar_base += home_delta;
        let start = state.avatar_obj_start.min(state.placeholder_objects.len());
        for o in state.placeholder_objects[start..].iter_mut() {
            o.2 += home_delta;
        }
    }
    log::info!(
        "Co-presence: carried {animals} animals, {plants} plants, the hologram{} with the home",
        if stage_in_home { " and the showroom stage" } else { "" }
    );
}

/// Stand the player at `at` (ship metres, eye height): the camera and the walking body, the
/// way the showcase request's `cam` verb does it (engine/ipc.rs), since in first person the
/// camera follows the body each frame.
fn put_player_at(state: &mut EngineState, at: Vec3) {
    for (_e, (t, _c)) in state
        .game_world
        .world
        .query_mut::<(&mut crate::ecs::components::Transform, &crate::ecs::components::Controllable)>()
    {
        t.position = at;
    }
    state.camera.position = at;
}

/// For the recorder's probe (engine/ipc.rs): the plot the home stands on, every plot of the
/// ship, and where the home's own things are (`home_things_json`), so a rig can check where the
/// game put its camera, its home and the other players.
pub(crate) fn probe_json(ship: Option<&ShipStructure>) -> (serde_json::Value, serde_json::Value) {
    let plot_json = |p: &crate::ship::ship_structure::Plot| {
        serde_json::json!({
            "id": p.id,
            "kind": p.kind,
            "origin": [p.origin.0, p.origin.1, p.origin.2],
            "size": [p.size.0, p.size.1, p.size.2],
        })
    };
    let Some(ship) = ship else { return (serde_json::Value::Null, serde_json::json!([])) };
    let home = ship.home_plot().map(plot_json).unwrap_or(serde_json::Value::Null);
    (home, serde_json::Value::Array(ship.plots.iter().map(plot_json).collect()))
}

/// Where the things that belong to the home stand now, for the rig (verify-copresence --plots
/// judges each one against the plot the game should hold): the Respawn point, the hologram,
/// the showroom stage, every farm animal's grazing point and every decoration plant.
pub(crate) fn home_things_json(state: &EngineState) -> serde_json::Value {
    let v = |p: Vec3| serde_json::json!([p.x, p.y, p.z]);
    let animals: Vec<serde_json::Value> = state
        .game_world
        .world
        .query::<(&crate::ecs::components::Creature, &crate::systems::livestock::HerdSlot)>()
        .iter()
        .map(|(_, (c, _))| v(c.anchor))
        .collect();
    let plants: Vec<serde_json::Value> = state.decoration_objects.iter().map(|d| v(d.2)).collect();
    serde_json::json!({
        "respawn": v(state.fps_spawn),
        "hologram": v(state.hologram_room_center),
        "showroom": v(state.avatar_base),
        "animals": animals,
        "plants": plants,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
    }

    /// The game as it boots: the home on the default plot, p1.
    fn booted() -> ShipStructure {
        ShipStructure::load_and_assemble(&data_dir(), None).expect("the shipped ship assembles")
    }

    /// A welcome as the relay sends it (relay/handlers/msg_handlers.rs handle_game_join), with
    /// the relay holding us (player 7) at `held`.
    fn welcome_at(plot: Option<&str>, hash: &str, held: Option<[f32; 3]>) -> serde_json::Value {
        let ship = booted();
        let home_plot = match plot {
            Some(id) => {
                let p = ship.plots.iter().find(|p| p.id == id).unwrap();
                serde_json::json!({ "id": id, "kind": p.kind, "origin": [p.origin.0, p.origin.1, p.origin.2], "size": [p.size.0, p.size.1, p.size.2] })
            }
            None => serde_json::Value::Null,
        };
        let mut snapshot = vec![serde_json::json!({ "entity_id": 3, "entity_type": "npc", "position": [1.0, 1.0, 1.0] })];
        if let Some(h) = held {
            snapshot.push(serde_json::json!({ "entity_id": 7, "entity_type": "player", "position": h }));
        }
        serde_json::json!({ "type": "game_welcome", "player_id": 7, "world_snapshot": snapshot, "home_plot": home_plot, "ship": { "id": "mothership-1", "hash": hash } })
    }

    fn welcome(plot: Option<&str>, hash: &str) -> serde_json::Value {
        welcome_at(plot, hash, None)
    }

    const P2_DOOR: Vec3 = Vec3::new(53.5, 1.7, 139.5);
    const COMMONS: Vec3 = Vec3::new(82.0, 1.7, 47.5);

    /// THE CASE 1b EXISTS FOR: the relay says p2. The home moves to p2 (every room and the
    /// spawn by exactly p2's offset, through 1a's assembly), Respawn becomes p2's door, and the
    /// player stands where the relay holds them, which on a fresh join is that door. Seen red
    /// 2026-10-03 with the Move arm of `plan_welcome` replaced by `Stay` (the 1a client, which
    /// never moved its home): "the relay said p2 and the home did not move: Stay".
    #[test]
    fn a_welcome_naming_another_plot_moves_the_home_there() {
        let ship = booted();
        let hash = ship.ship_hash();
        match plan_welcome(Some(&ship), &welcome_at(Some("p2"), &hash, Some(P2_DOOR.into())), true) {
            WelcomeHome::Move { ship: moved, plot, door, stand_at } => {
                assert_eq!(plot, "p2");
                assert_eq!(moved.home.as_ref().map(|a| a.plot.as_str()), Some("p2"));
                let dz = 99.0; // p2 sits 99 m south of p1 (design section 2.4)
                let home = |s: &ShipStructure| s.zones[s.home_zone_index()].origin;
                assert_eq!(home(&moved), (0.0, 0.0, dz));
                assert_eq!(moved.zones.len(), ship.zones.len());
                // The door: what the relay spawns at, and what the game's own path gives.
                assert!((door - P2_DOOR).length() < 1e-4, "door {door:?}");
                assert!((moved.home_spawn_world().unwrap() - door).length() < 1e-4);
                assert!((stand_at - P2_DOOR).length() < 1e-4, "stand_at {stand_at:?}");
                // The door lies inside p2's box, not p1's.
                let (lo, hi) = moved.home_plot().unwrap().aabb();
                assert!(door.cmpge(lo).all() && door.cmple(hi).all());
                // Moving is not a new ship: the fingerprint holds.
                assert_eq!(moved.ship_hash(), hash);
            }
            other => panic!("the relay said p2 and the home did not move: {other:?}"),
        }
        // A welcome with no entry for us (never sent by the relay, but possible): the door.
        match plan_welcome(Some(&ship), &welcome(Some("p2"), &hash), true) {
            WelcomeHome::Move { stand_at, .. } => assert!((stand_at - P2_DOOR).length() < 1e-4),
            other => panic!("{other:?}"),
        }
    }

    /// The game restarted inside the relay's 90 s grace: its world loaded afresh with the home
    /// on p1, and the relay kept the player where they were, deep in p2 at z 170. The player
    /// stands THERE, not at the door 30 m away, or the figure the others see would jump. Seen
    /// red 2026-10-03 with `stand_at` set to the door (the first 1b client): "after a restart in
    /// the grace the player stands at Vec3(53.5, 1.7, 139.5), the relay holds them at
    /// Vec3(30.0, 1.7, 170.0)".
    #[test]
    fn a_restart_inside_the_grace_stands_where_the_relay_kept_the_player() {
        let ship = booted();
        let deep = Vec3::new(30.0, 1.7, 170.0);
        match plan_welcome(Some(&ship), &welcome_at(Some("p2"), &ship.ship_hash(), Some(deep.into())), true) {
            WelcomeHome::Move { door, stand_at, .. } => {
                assert!((stand_at - deep).length() < 1e-4, "after a restart in the grace the player stands at {stand_at:?}, the relay holds them at {deep:?}");
                assert!((door - P2_DOOR).length() < 1e-4, "Respawn is still the door: {door:?}");
            }
            other => panic!("{other:?}"),
        }
    }

    /// The home already on the plot the relay named: nothing moves. On ARRIVAL the player
    /// stands where the relay holds them (their own door, which may not be where the world
    /// load put the camera: the player may have walked before the connection came up). On a
    /// reconnect they keep walking where they are. Seen red 2026-10-03 with the Stay arm never
    /// moving anyone (the first 1b client): "an arrival on our own plot stands at our door:
    /// Stay { stand_at: None }".
    #[test]
    fn a_welcome_naming_our_own_plot_moves_nothing() {
        let ship = booted();
        let door = Vec3::new(53.5, 1.7, 40.5);
        let w = welcome_at(Some("p1"), &ship.ship_hash(), Some(door.into()));
        match plan_welcome(Some(&ship), &w, true) {
            WelcomeHome::Stay { stand_at: Some(at) } => assert!((at - door).length() < 1e-4, "{at:?}"),
            other => panic!("an arrival on our own plot stands at our door: {other:?}"),
        }
        assert!(matches!(plan_welcome(Some(&ship), &w, false), WelcomeHome::Stay { stand_at: None }));
    }

    /// A full ship: home_plot null, and the player arrives in the Commons, at the point the
    /// relay put them (game_state.rs assign_home uses the same `guest_spawn`); the Commons is
    /// their Respawn point.
    #[test]
    fn a_full_ship_makes_a_guest_in_the_commons() {
        let ship = booted();
        match plan_welcome(Some(&ship), &welcome_at(None, &ship.ship_hash(), Some(COMMONS.into())), true) {
            WelcomeHome::Guest { door: Some(d), stand_at: Some(at) } => {
                assert!((d - COMMONS).length() < 1e-4, "{d:?}");
                assert!((at - COMMONS).length() < 1e-4, "{at:?}");
            }
            other => panic!("a full ship should make a guest: {other:?}"),
        }
    }

    /// A guest who reconnects (the socket dropped for a moment) keeps walking where they are:
    /// the relay still holds them about there, at the end of street-1 here, 148 m from the
    /// Commons. Sending them back to the Commons would put them 148 m from where the relay
    /// holds them: every update refused, frozen for everyone else. Seen red 2026-10-03 on the
    /// first 1b client (the guest arm always went to the Commons): "a reconnecting guest is
    /// moved to Some(Vec3(82.0, 1.7, 47.5))".
    #[test]
    fn a_reconnecting_guest_keeps_walking_where_they_are() {
        let ship = booted();
        let street_end = Vec3::new(70.0, 1.7, 195.0);
        match plan_welcome(Some(&ship), &welcome_at(None, &ship.ship_hash(), Some(street_end.into())), false) {
            WelcomeHome::Guest { door, stand_at } => {
                assert_eq!(stand_at, None, "a reconnecting guest is moved to {stand_at:?}");
                assert!(door.is_some_and(|d| (d - COMMONS).length() < 1e-4), "Respawn is the Commons: {door:?}");
            }
            other => panic!("{other:?}"),
        }
        // The same guest arriving afresh (their game restarted inside the grace) stands where
        // the relay kept them, not at the Commons.
        match plan_welcome(Some(&ship), &welcome_at(None, &ship.ship_hash(), Some(street_end.into())), true) {
            WelcomeHome::Guest { stand_at: Some(at), .. } => assert!((at - street_end).length() < 1e-4, "{at:?}"),
            other => panic!("{other:?}"),
        }
    }

    /// Another ship (a different hash), no ship named at all, or no ship of our own: refuse,
    /// with the one sentence, and never move the home.
    #[test]
    fn another_ship_is_refused_in_one_plain_sentence() {
        let ship = booted();
        for w in [
            welcome(Some("p2"), "0123456789abcdef"),
            serde_json::json!({ "type": "game_welcome", "player_id": 7, "home_plot": null }),
        ] {
            match plan_welcome(Some(&ship), &w, true) {
                WelcomeHome::Refuse(s) => assert_eq!(s, SHIP_MISMATCH),
                other => panic!("a different ship must be refused: {other:?}"),
            }
        }
        assert!(matches!(plan_welcome(None, &welcome(Some("p1"), &ship.ship_hash()), true), WelcomeHome::Refuse(_)));
        // One sentence: one full stop, at the end.
        assert_eq!(SHIP_MISMATCH.matches(". ").count(), 0);
        assert!(SHIP_MISMATCH.ends_with('.'));
    }

    /// What the game puts in its `game_join`: the ship it draws and its own home's door,
    /// plot-local. No ship: an empty hash (never a match, so no plot is claimed) and no door.
    /// Seen red 2026-10-03 with `add_join_fields` adding nothing (the first 1b join): "the join
    /// names no ship: null".
    #[test]
    fn the_join_names_our_ship_and_our_door() {
        let mut ship = booted();
        let home = ship.home_zone_index();
        ship.zones[home].body.spawn = Some((12.5, 30.0));
        let mut join = serde_json::json!({ "type": "game_join" });
        add_join_fields(&mut join, Some(&ship));
        assert_eq!(join["ship_hash"], ship.ship_hash().as_str(), "the join names no ship: {}", join["ship_hash"]);
        assert_eq!(join["home_spawn"], serde_json::json!([12.5, 30.0]));
        let mut bare = serde_json::json!({ "type": "game_join" });
        add_join_fields(&mut bare, None);
        assert_eq!(bare["ship_hash"], "");
        assert!(bare.get("home_spawn").is_none());
    }

    /// What the world load anchored to a machine goes with that machine. Placing the shipped
    /// machines with the home on p1 and again on p2: an anchor at a home machine (the grain
    /// field the chickens graze around) moves by exactly p2's 99 m, a sphere's lifted
    /// placement still matches its unlifted anchor, and an anchor at a ship machine (a Commons
    /// row) or at no machine at all does not move. Seen red 2026-10-03 with `shift_of`
    /// returning zero (the first 1b client, which carried nothing): "the chickens' grain
    /// field moved by Vec3(0.0, 0.0, 0.0)".
    #[test]
    fn what_was_anchored_to_a_home_machine_goes_with_it() {
        let home = crate::machines::MachineHome::load(&crate::machines::home_ron_path(&data_dir())).expect("the machines load");
        let place = |plot: &str| {
            let ship = ShipStructure::load_and_assemble(&data_dir(), Some(plot)).unwrap();
            home.placements(&std::collections::HashMap::new(), Some(&ship.zone_rects()))
        };
        let (on_p1, on_p2) = (place("p1"), place("p2"));
        let shifts = machine_shifts(&on_p1, &on_p2);
        let g = on_p1.iter().find(|m| m.id == "grain_field_1").expect("the grain field is placed");
        let d = shift_of(&shifts, Vec3::new(g.pos.0, g.pos.1, g.pos.2));
        assert!((d - Vec3::new(0.0, 0.0, 99.0)).length() < 1e-4, "the chickens' grain field moved by {d:?}");
        // A sphere's placement is lifted by its radius; the world load anchors at the floor.
        if let Some(s) = on_p1.iter().find(|m| m.shape == "sphere") {
            let floor = Vec3::new(s.pos.0, s.pos.1 - s.size.0, s.pos.2);
            assert!((shift_of(&shifts, floor) - Vec3::new(0.0, 0.0, 99.0)).length() < 1e-4, "{} (a sphere)", s.id);
        }
        // A machine outside the home (a Commons row, the ship's) stays, and so does what
        // grazes around it; so does an anchor at no machine at all.
        let (lo, hi) = booted().home_plot().unwrap().aabb();
        let c = on_p1
            .iter()
            .find(|m| {
                let p = Vec3::new(m.pos.0, m.pos.1, m.pos.2);
                !(p.cmpge(lo).all() && p.cmple(hi).all())
            })
            .expect("the ship has machines outside the home");
        assert_eq!(shift_of(&shifts, Vec3::new(c.pos.0, 0.0, c.pos.2)), Vec3::ZERO, "{} is not the home's", c.id);
        assert_eq!(shift_of(&shifts, Vec3::new(-500.0, 0.0, -500.0)), Vec3::ZERO);
    }
}
