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
//! (`add_join_fields`): only a join naming the relay's ship holds a plot (a join naming another
//! ship is refused before anything is spawned), and the player arrives at THEIR door, which
//! they may have moved with the build-mode avatar, not at the default design's. A home with no
//! authored door names none, and both sides take the middle of the plot handed out.
//!
//! On every `game_welcome` the game:
//!   1. checks the relay's ship is its own ship (the `ship.hash`, `ShipStructure::ship_hash`):
//!      positions only agree when everyone has the same ship, so on a mismatch it refuses to
//!      join, with one plain sentence;
//!   2. moves its `home` zone to the plot the relay named, through 1a's assembly (the ship
//!      file plus the home design on that plot plus the plot's door corridor), rebuilds the
//!      home through the editor's rebuild path, and carries along what the rebuild does not
//!      redo (`carry_home_things`: the farm animals, the decoration plants, the pieces the
//!      player built aboard, their parked vehicles, the hologram and the showroom stage), and
//!      the Respawn point. A home that cannot stand on that plot refuses to join and gives the
//!      plot back;
//!   3. as a guest (the ship is full, or the join named no ship), makes the Commons its
//!      Respawn point; on its own plot, its own door;
//!   4. stands the player where the relay holds them (their own entry in the welcome's
//!      `world_snapshot`) when `stand_where_held` says so: on an ARRIVAL (the first welcome
//!      since the world loaded, or from another server), on any welcome that spawned them
//!      afresh (`rejoin` false: after stepping out to solo play or Dev travel and back, after
//!      the relay's 90 s grace ran out, after a relay restart), and whenever the game stands
//!      more than `FAR_FROM_HELD_M` from that point. Otherwise (a reconnect inside the grace,
//!      near where the relay kept them) the player keeps walking where they are: snapping them
//!      back to their last update would undo the walk.
//!
//! `plan_welcome` decides, and is pure (tested below); `apply_welcome_home` does it.
//!
//! THE SAVE'S FRAME. Pieces built aboard and parked vehicles are saved at ship positions, and
//! every world entry builds the home on the default plot first. So the save writes the ones in
//! the home as if the home stood on the default plot (save_load.rs `into_default_frame`, from
//! the `HomeFrame` published here), and a welcome that moves the home carries them to its plot.
//! A save loaded into a running world whose home stands elsewhere is carried the same way
//! (`carry_saved_pieces_home`).

use crate::ecs::components::Transform;
use crate::engine::state::EngineState;
use crate::ship::ship_structure::ShipStructure;
use glam::Vec3;

/// The sentence a player reads when the server's ship is not theirs (one copy, shared with
/// the relay, which refuses such a join with it).
pub(crate) const SHIP_MISMATCH: &str = crate::ship::ship_structure::OTHER_SHIP_SENTENCE;

/// How far the game may stand from where the relay holds the player before a welcome stands
/// them back there, metres, whatever else it says. The relay refuses any update more than
/// 100 m from where it holds them, so a game standing farther away than that would be frozen
/// for everyone else; 90 leaves a margin for one update's walk.
pub(crate) const FAR_FROM_HELD_M: f32 = 90.0;

/// What a welcome asks of the home and of where the player stands.
#[derive(Debug)]
pub(crate) enum WelcomeHome {
    /// Do not join: the sentence to show. `give_up_plot`: the relay gave us a plot our home
    /// cannot stand on, so we give it back as we leave (`game_leave` with `give_up_plot`).
    Refuse { sentence: String, give_up_plot: bool },
    /// The home already stands on the plot the relay named: nothing moves. `door` is where its
    /// holder arrives (the Respawn point), `stand_at` where the player goes (None: they stay
    /// where they are).
    Stay { door: Option<Vec3>, stand_at: Option<Vec3> },
    /// Move the home: the ship re-assembled on `plot`. `door` is where the holder of that plot
    /// arrives with this home (the Respawn button's point from now on), `stand_at` where the
    /// player goes now.
    Move { ship: ShipStructure, plot: String, door: Vec3, stand_at: Vec3 },
    /// A guest (the ship is full). `door` is the Commons arrival (the Respawn point), None for a
    /// ship with no Commons; `stand_at` where the player goes (None: they stay).
    Guest { door: Option<Vec3>, stand_at: Option<Vec3> },
}

/// What the game knows when a welcome arrives, besides the welcome: the server whose welcome
/// last stood us where it holds us (`EngineState::home_arrived_on`), the server this welcome
/// comes from, and where the camera stands (ship metres, the frame the game sends).
pub(crate) struct WelcomeContext<'a> {
    pub arrived_on: Option<&'a str>,
    pub server: &'a str,
    pub camera: Vec3,
}

impl<'a> WelcomeContext<'a> {
    /// Read from the running game.
    fn of(state: &'a EngineState) -> Self {
        WelcomeContext {
            arrived_on: state.home_arrived_on.as_deref(),
            server: state.gui_state.server_url.as_str(),
            camera: state.camera.position,
        }
    }
}

/// True when a welcome from `server` is an ARRIVAL: the first since the world loaded
/// (`arrived_on` None, world_load clears it) or from another server than the one whose welcome
/// last stood us where it holds us. Pure.
pub(crate) fn is_arrival(arrived_on: Option<&str>, server: &str) -> bool {
    arrived_on != Some(server)
}

/// Whether a welcome stands the player where the relay holds them. `arriving`: `is_arrival`.
/// `rejoin`: the welcome's own word that the relay found them still in the world (false: it
/// spawned them afresh, at their door or in the Commons, wherever the game stands). `gap_m`:
/// how far the camera stands from where the relay holds them (None: the welcome says nothing
/// about us). Pure; the second review of 1b found the first rule ("only on an arrival") froze
/// every player who stepped out to solo play and back.
pub(crate) fn stand_where_held(arriving: bool, rejoin: bool, gap_m: Option<f32>) -> bool {
    arriving || !rejoin || gap_m.is_some_and(|d| d > FAR_FROM_HELD_M)
}

/// Decide what a `game_welcome` asks of the home, from the ship the game runs now (`None` when
/// no ship assembled and the legacy layout is showing) and the game's `ctx`. Pure.
pub(crate) fn plan_welcome(ship: Option<&ShipStructure>, welcome: &serde_json::Value, ctx: &WelcomeContext) -> WelcomeHome {
    let refuse = |sentence: &str, give_up_plot: bool| WelcomeHome::Refuse { sentence: sentence.to_string(), give_up_plot };
    let theirs = welcome.get("ship").and_then(|s| s.get("hash")).and_then(|h| h.as_str());
    let Some(ship) = ship else { return refuse(SHIP_MISMATCH, false) };
    if theirs != Some(ship.ship_hash().as_str()) {
        return refuse(SHIP_MISMATCH, false);
    }
    // Where the relay holds us right now (our own entry in the snapshot), and whether to stand
    // there.
    let held = relay_holds_us(welcome);
    let rejoin = welcome.get("rejoin").and_then(|r| r.as_bool()).unwrap_or(false);
    let stand = stand_where_held(is_arrival(ctx.arrived_on, ctx.server), rejoin, held.map(|h| h.distance(ctx.camera)));
    let stand_at = |door: Option<Vec3>| if stand { held.or(door) } else { None };
    let Some(hp) = welcome.get("home_plot").filter(|v| !v.is_null()) else {
        let door = ship.guest_spawn();
        return WelcomeHome::Guest { door, stand_at: stand_at(door) };
    };
    let Some(id) = hp.get("id").and_then(|i| i.as_str()) else {
        return refuse(SHIP_MISMATCH, false);
    };
    if ship.home.as_ref().is_some_and(|a| a.plot == id) {
        let door = own_door(ship);
        return WelcomeHome::Stay { door, stand_at: stand_at(door) };
    }
    let (Some(design), Some(plot)) = (ship.home_design(), ship.plots.iter().find(|p| p.id == id)) else {
        return refuse(&format!("Not joining the shared world: your home could not be placed on the plot this server gave you ({id})."), true);
    };
    let door = ShipStructure::plot_spawn(plot, &design);
    match ship.ship_file().assemble(design, id) {
        // The old home is gone from under the player, so they always go somewhere: where the
        // relay holds them, or this home's door on the new plot.
        Ok(moved) => WelcomeHome::Move { ship: moved, plot: id.to_string(), door, stand_at: held.unwrap_or(door) },
        // The plot this server gave us cannot hold our home: give it back (`give_up_plot`) so
        // it is not held for good by someone who never lives there.
        Err(e) => refuse(&format!("Not joining the shared world: your home does not fit the plot this server gave you ({e})."), true),
    }
}

/// Where the holder of the plot this home stands on arrives with it: its door there, else the
/// middle of the plot (`ShipStructure::plot_spawn`, what the relay spawns at too). None for a
/// ship that was not assembled with a home.
fn own_door(ship: &ShipStructure) -> Option<Vec3> {
    Some(ShipStructure::plot_spawn(ship.home_plot()?, &ship.home_design()?))
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
/// ship we draw (empty when none assembled, which can never match, so the relay refuses the
/// join and nothing is spawned), and `home_spawn`, our own home's door in plot-local metres
/// (`ShipStructure::home_arrival_local`), left out when the home has no authored door.
pub(crate) fn add_join_fields(join: &mut serde_json::Value, ship: Option<&ShipStructure>) {
    let Some(obj) = join.as_object_mut() else { return };
    obj.insert("ship_hash".into(), serde_json::json!(ship.map(|s| s.ship_hash()).unwrap_or_default()));
    if let Some((x, z)) = ship.and_then(|s| s.home_arrival_local()) {
        obj.insert("home_spawn".into(), serde_json::json!([x, z]));
    }
}

/// Stop joining the shared world on this server and show `sentence`. `leave`: Some(give_up)
/// when the relay put us in the world (we send `game_leave`, with `give_up_plot` when the plot
/// it gave us cannot hold our home), None when it never did (it refused our join, net_route.rs
/// `game_join_denied` with reason "other_ship"). Not again on this server until the world is
/// loaded afresh (the lib.rs join gate reads `copresence_refused`).
pub(crate) fn refuse_shared_world(state: &mut EngineState, sentence: String, leave: Option<bool>) {
    log::warn!("Co-presence: {sentence} (ours {:?})", state.gui_state.ship_structure.as_ref().map(|s| s.ship_hash()));
    if let (Some(give_up), Some(ws)) = (leave, state.gui_state.ws_client.as_ref()) {
        let msg = if give_up {
            serde_json::json!({ "type": "game_leave", "give_up_plot": true })
        } else {
            serde_json::json!({ "type": "game_leave" })
        };
        ws.send(&msg.to_string());
    }
    state.game_joined = false;
    state.gui_state.copresence_active = false;
    state.copresence_refused = Some(state.gui_state.server_url.clone());
    state.gui_state.pending_notices.push(sentence);
}

/// Act on a `game_welcome` (engine/net_route.rs, before the welcome reaches net_sync). Returns
/// false when the game refused to join: the caller then drops the welcome.
pub(crate) fn apply_welcome_home(state: &mut EngineState, welcome: &serde_json::Value) -> bool {
    let plan = plan_welcome(state.gui_state.ship_structure.as_ref(), welcome, &WelcomeContext::of(state));
    match plan {
        WelcomeHome::Refuse { sentence, give_up_plot } => {
            log::warn!("Co-presence: refusing the welcome (server ship {})", welcome["ship"]);
            refuse_shared_world(state, sentence, Some(give_up_plot));
            return false;
        }
        WelcomeHome::Stay { door, stand_at } => {
            // Respawn brings the player back to their own door on this plot, whatever another
            // server's welcome (a guest's Commons) set it to before.
            if let Some(d) = door {
                state.fps_spawn = d;
            }
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
            publish_home_frame(state);
            // Respawn brings the player back to their own door, on this plot.
            state.fps_spawn = door;
            put_player_at(state, stand_at);
        }
        WelcomeHome::Guest { door, stand_at } => {
            log::info!("Co-presence: no plot of our own here; joining as a guest in the Commons");
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

/// Where the home stands now and where it stands on the ship's default plot, for the save's
/// frame (save_load.rs `HomeFrame`). None for a ship with no home or no default plot.
pub(crate) fn home_frame(ship: &ShipStructure) -> Option<crate::save_load::HomeFrame> {
    let home = home_box(ship)?;
    let default_id = ship.default_plot_id()?;
    let default = ship.plots.iter().find(|p| p.id == default_id)?.aabb();
    Some(crate::save_load::HomeFrame { home, default })
}

/// Put the home's frame in the DataStore for the save (`save_load::HOME_FRAME_KEY`): called
/// whenever the home is placed, by the world load and by a welcome that moves it. Taken away
/// when there is no frame (the legacy layout), so a save then writes positions as they stand.
pub(crate) fn publish_home_frame(state: &mut EngineState) {
    match state.gui_state.ship_structure.as_ref().and_then(home_frame) {
        Some(f) => state.data_store.insert(crate::save_load::HOME_FRAME_KEY, f),
        None => state.data_store.remove(crate::save_load::HOME_FRAME_KEY),
    }
}

/// After a save was loaded into the running world (lib.rs, the launcher's character pick and
/// a restored snapshot): the save holds the home's pieces and vehicles as if the home stood on
/// the default plot, so carry them to the plot it stands on now. Nothing moves while the home
/// stands on the default plot.
pub(crate) fn carry_saved_pieces_home(state: &mut EngineState) {
    let Some(f) = state.gui_state.ship_structure.as_ref().and_then(home_frame) else { return };
    let (pieces, vehicles) = carry_built_pieces(&mut state.game_world.world, f.default, f.offset());
    if pieces + vehicles > 0 {
        log::info!("Loaded save: carried {pieces} built pieces and {vehicles} vehicles to the home's plot");
    }
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

/// True when `p` stands over a plot's floor: inside its box across x and z (height is not
/// bounded, the rule building aboard uses, engine/build_place.rs `outside_own_plot`), with a
/// centimetre of slack for rounding. A built piece may overhang its plot by up to 0.15 m, but
/// its centre, which is what is tested here, is always inside.
pub(crate) fn over_plot(p: Vec3, plot: (Vec3, Vec3)) -> bool {
    let (lo, hi) = plot;
    p.x >= lo.x - 0.01 && p.x <= hi.x + 0.01 && p.z >= lo.z - 0.01 && p.z <= hi.z + 0.01
}

/// Carry the pieces the player built aboard and their parked vehicles standing in the plot
/// `from` by `delta` (the second review of 1b: a chest built at home on p1 stayed in p1, by then
/// someone else's home, with its contents, when the home moved to p2). A piece built on a
/// planet (it carries a `PlanetSite`, its pose is in that site's frame) never moves; neither does
/// anything outside `from` (the ship's own spaces). A vehicle driving to a point in the home
/// drives to the same point of the moved home. Health, contents (filed under the piece's uid)
/// and open doors are kept: the pieces are moved, not built again. Returns (pieces, vehicles)
/// moved. Pure on the world.
pub(crate) fn carry_built_pieces(world: &mut hecs::World, from: (Vec3, Vec3), delta: Vec3) -> (usize, usize) {
    use crate::systems::construction::{Construction, PlanetSite, Structure};
    if delta == Vec3::ZERO {
        return (0, 0);
    }
    let mut pieces = 0;
    for (_e, (t, _built, site)) in world.query_mut::<(&mut Transform, hecs::Or<&Structure, &Construction>, Option<&PlanetSite>)>() {
        if site.is_none() && over_plot(t.position, from) {
            t.position += delta;
            pieces += 1;
        }
    }
    let mut vehicles = 0;
    for (_e, (_v, t, route)) in world.query_mut::<(
        &crate::ecs::components::Vehicle,
        &mut Transform,
        Option<&mut crate::ecs::components::VehicleRoute>,
    )>() {
        if over_plot(t.position, from) {
            t.position += delta;
            vehicles += 1;
            if let Some(r) = route.filter(|r| over_plot(r.dest, from)) {
                r.dest += delta;
            }
        }
    }
    (pieces, vehicles)
}

/// Where the pieces built aboard and the vehicles stand (ship metres), for the rig's report
/// (`home_things_json`). Planet pieces are left out (their poses are in their site's frame).
/// Pure on the world.
pub(crate) fn built_positions(world: &hecs::World) -> (Vec<Vec3>, Vec<Vec3>) {
    use crate::systems::construction::{Construction, PlanetSite, Structure};
    let pieces = world
        .query::<(&Transform, hecs::Or<&Structure, &Construction>, Option<&PlanetSite>)>()
        .iter()
        .filter(|(_, (_, _, site))| site.is_none())
        .map(|(_, (t, _, _))| t.position)
        .collect();
    let vehicles = world
        .query::<(&crate::ecs::components::Vehicle, &Transform)>()
        .iter()
        .map(|(_, (_, t))| t.position)
        .collect();
    (pieces, vehicles)
}

/// Carry along what the world load placed for the home and the rebuild does not redo, after
/// the home moved by `home_delta` from the box `old_home`:
///   - the farm animals (each grazes around a machine, `Creature.anchor`) and the decoration
///     plants (each scattered around one): by their machine's own shift, so an animal kept
///     near a ship machine stays put;
///   - the pieces the player built aboard and their parked vehicles (`carry_built_pieces`);
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
    let (pieces, vehicles) = carry_built_pieces(&mut state.game_world.world, old_home, home_delta);
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
        "Co-presence: carried {animals} animals, {plants} plants, {pieces} built pieces, {vehicles} vehicles, the hologram{} with the home",
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
/// the showroom stage, every farm animal's grazing point, every decoration plant, every piece
/// built aboard and every parked vehicle.
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
    let (pieces, vehicles) = built_positions(&state.game_world.world);
    serde_json::json!({
        "respawn": v(state.fps_spawn),
        "hologram": v(state.hologram_room_center),
        "showroom": v(state.avatar_base),
        "animals": animals,
        "plants": plants,
        "structures": pieces.into_iter().map(v).collect::<Vec<_>>(),
        "vehicles": vehicles.into_iter().map(v).collect::<Vec<_>>(),
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
    /// the relay holding us (player 7) at `held`, and `rejoin` as the relay says.
    fn welcome_full(plot: Option<&str>, hash: &str, held: Option<[f32; 3]>, rejoin: bool) -> serde_json::Value {
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
        serde_json::json!({
            "type": "game_welcome", "player_id": 7, "world_snapshot": snapshot, "home_plot": home_plot,
            "ship": { "id": "mothership-1", "hash": hash }, "rejoin": rejoin,
        })
    }

    /// A fresh spawn (rejoin false), as on every first join.
    fn welcome_at(plot: Option<&str>, hash: &str, held: Option<[f32; 3]>) -> serde_json::Value {
        welcome_full(plot, hash, held, false)
    }

    fn welcome(plot: Option<&str>, hash: &str) -> serde_json::Value {
        welcome_at(plot, hash, None)
    }

    const SERVER: &str = "http://127.0.0.1:3210";

    /// The first welcome since the world loaded, the camera at `camera`.
    fn arriving(camera: Vec3) -> WelcomeContext<'static> {
        WelcomeContext { arrived_on: None, server: SERVER, camera }
    }

    /// A later welcome from the same server (we arrived on it before), the camera at `camera`.
    fn again(camera: Vec3) -> WelcomeContext<'static> {
        WelcomeContext { arrived_on: Some(SERVER), server: SERVER, camera }
    }

    const P1_DOOR: Vec3 = Vec3::new(53.5, 1.7, 40.5);
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
        match plan_welcome(Some(&ship), &welcome_at(Some("p2"), &hash, Some(P2_DOOR.into())), &arriving(P1_DOOR)) {
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
        match plan_welcome(Some(&ship), &welcome(Some("p2"), &hash), &arriving(P1_DOOR)) {
            WelcomeHome::Move { stand_at, .. } => assert!((stand_at - P2_DOOR).length() < 1e-4),
            other => panic!("{other:?}"),
        }
    }

    /// The game restarted inside the relay's 90 s grace: its world loaded afresh with the home
    /// on p1, and the relay kept the player where they were, deep in p2 at z 170 (`rejoin`
    /// true). The player stands THERE, not at the door 30 m away, or the figure the others see
    /// would jump. Seen red 2026-10-03 with `stand_at` set to the door (the first 1b client):
    /// "after a restart in the grace the player stands at Vec3(53.5, 1.7, 139.5), the relay
    /// holds them at Vec3(30.0, 1.7, 170.0)".
    #[test]
    fn a_restart_inside_the_grace_stands_where_the_relay_kept_the_player() {
        let ship = booted();
        let deep = Vec3::new(30.0, 1.7, 170.0);
        let w = welcome_full(Some("p2"), &ship.ship_hash(), Some(deep.into()), true);
        match plan_welcome(Some(&ship), &w, &arriving(P1_DOOR)) {
            WelcomeHome::Move { door, stand_at, .. } => {
                assert!((stand_at - deep).length() < 1e-4, "after a restart in the grace the player stands at {stand_at:?}, the relay holds them at {deep:?}");
                assert!((door - P2_DOOR).length() < 1e-4, "Respawn is still the door: {door:?}");
            }
            other => panic!("{other:?}"),
        }
    }

    /// The home already on the plot the relay named: nothing moves, and Respawn is our own door
    /// there. On ARRIVAL the player stands where the relay holds them (their own door, which may
    /// not be where the world load put the camera: the player may have walked before the
    /// connection came up). On a reconnect near where the relay kept them they keep walking.
    /// Seen red 2026-10-03 with the Stay arm never moving anyone (the first 1b client): "an
    /// arrival on our own plot stands at our door: Stay { stand_at: None }"; and (the second
    /// review's Respawn finding) with the Stay arm naming no door: "a welcome on our own plot
    /// leaves Respawn wherever it was: None", so a guest on another server who then came back
    /// as the p1 holder respawned in the Commons.
    #[test]
    fn a_welcome_naming_our_own_plot_moves_nothing() {
        let ship = booted();
        let w = welcome_at(Some("p1"), &ship.ship_hash(), Some(P1_DOOR.into()));
        match plan_welcome(Some(&ship), &w, &arriving(Vec3::new(20.0, 1.7, 30.0))) {
            WelcomeHome::Stay { door, stand_at: Some(at) } => {
                assert!((at - P1_DOOR).length() < 1e-4, "{at:?}");
                assert!(door.is_some_and(|d| (d - P1_DOOR).length() < 1e-4), "a welcome on our own plot leaves Respawn wherever it was: {door:?}");
            }
            other => panic!("an arrival on our own plot stands at our door: {other:?}"),
        }
        let near = welcome_full(Some("p1"), &ship.ship_hash(), Some(P1_DOOR.into()), true);
        match plan_welcome(Some(&ship), &near, &again(P1_DOOR + Vec3::new(3.0, 0.0, 0.0))) {
            WelcomeHome::Stay { door: Some(d), stand_at: None } => assert!((d - P1_DOOR).length() < 1e-4),
            other => panic!("a reconnect near where the relay kept us keeps walking: {other:?}"),
        }
    }

    /// A full ship: home_plot null, and the player arrives in the Commons, at the point the
    /// relay put them (game_state.rs assign_home uses the same `guest_spawn`); the Commons is
    /// their Respawn point.
    #[test]
    fn a_full_ship_makes_a_guest_in_the_commons() {
        let ship = booted();
        match plan_welcome(Some(&ship), &welcome_at(None, &ship.ship_hash(), Some(COMMONS.into())), &arriving(P1_DOOR)) {
            WelcomeHome::Guest { door: Some(d), stand_at: Some(at) } => {
                assert!((d - COMMONS).length() < 1e-4, "{d:?}");
                assert!((at - COMMONS).length() < 1e-4, "{at:?}");
            }
            other => panic!("a full ship should make a guest: {other:?}"),
        }
    }

    /// A guest who reconnects inside the grace (the socket dropped for a moment, `rejoin` true)
    /// keeps walking where they are: the relay still holds them about there, at the end of
    /// street-1 here, 148 m from the Commons. Sending them back to the Commons would put them
    /// 148 m from where the relay holds them: every update refused, frozen for everyone else.
    /// Seen red 2026-10-03 on the first 1b client (the guest arm always went to the Commons):
    /// "a reconnecting guest is moved to Some(Vec3(82.0, 1.7, 47.5))".
    #[test]
    fn a_reconnecting_guest_keeps_walking_where_they_are() {
        let ship = booted();
        let street_end = Vec3::new(70.0, 1.7, 195.0);
        let w = welcome_full(None, &ship.ship_hash(), Some(street_end.into()), true);
        match plan_welcome(Some(&ship), &w, &again(street_end + Vec3::new(0.5, 0.0, 0.0))) {
            WelcomeHome::Guest { door, stand_at } => {
                assert_eq!(stand_at, None, "a reconnecting guest is moved to {stand_at:?}");
                assert!(door.is_some_and(|d| (d - COMMONS).length() < 1e-4), "Respawn is the Commons: {door:?}");
            }
            other => panic!("{other:?}"),
        }
        // The same guest arriving afresh (their game restarted inside the grace) stands where
        // the relay kept them, not at the Commons.
        match plan_welcome(Some(&ship), &w, &arriving(P1_DOOR)) {
            WelcomeHome::Guest { stand_at: Some(at), .. } => assert!((at - street_end).length() < 1e-4, "{at:?}"),
            other => panic!("{other:?}"),
        }
    }

    /// THE SECOND REVIEW'S FREEZE: a guest at the end of street-1 steps out of the shared world
    /// (the launcher's offline home, Dev travel or fly mode) and back. The relay took them out
    /// at the `game_leave` and spawns them AFRESH in the Commons (`rejoin` false), 148 m from
    /// where the game stands, while the game is not arriving (same server, same world session).
    /// They must stand where the relay now holds them, or every update is refused by the 100 m
    /// rule. Seen red 2026-10-03 on 65b3e2c0c (stand only on an arrival): "a fresh spawn after
    /// stepping out leaves the player 148 m from where the relay holds them: None".
    #[test]
    fn a_fresh_spawn_after_stepping_out_stands_where_the_relay_holds_us() {
        let ship = booted();
        let street_end = Vec3::new(70.0, 1.7, 195.0);
        let w = welcome_full(None, &ship.ship_hash(), Some(COMMONS.into()), false);
        match plan_welcome(Some(&ship), &w, &again(street_end)) {
            WelcomeHome::Guest { stand_at, .. } => assert!(
                stand_at.is_some_and(|at| (at - COMMONS).length() < 1e-4),
                "a fresh spawn after stepping out leaves the player {:.0} m from where the relay holds them: {stand_at:?}",
                street_end.distance(COMMONS)
            ),
            other => panic!("{other:?}"),
        }
        // The same on the player's own plot: the relay spawned them afresh at their door.
        let w = welcome_full(Some("p1"), &ship.ship_hash(), Some(P1_DOOR.into()), false);
        match plan_welcome(Some(&ship), &w, &again(Vec3::new(70.0, 1.7, 190.0))) {
            WelcomeHome::Stay { stand_at: Some(at), .. } => assert!((at - P1_DOOR).length() < 1e-4, "{at:?}"),
            other => panic!("a fresh spawn on our own plot stands at our door: {other:?}"),
        }
    }

    /// A reconnect inside the grace (`rejoin` true, not arriving) where the game nonetheless
    /// stands far from where the relay kept them (its own updates were refused, say): the
    /// backstop stands them back there. Seen red 2026-10-03 on 65b3e2c0c (no backstop): "a
    /// reconnect 120 m from where the relay holds us is left there: None".
    #[test]
    fn a_reconnect_far_from_where_the_relay_holds_us_brings_us_back() {
        let ship = booted();
        let w = welcome_full(Some("p1"), &ship.ship_hash(), Some(P1_DOOR.into()), true);
        let far = P1_DOOR + Vec3::new(0.0, 0.0, 120.0);
        match plan_welcome(Some(&ship), &w, &again(far)) {
            WelcomeHome::Stay { stand_at, .. } => assert!(
                stand_at.is_some_and(|at| (at - P1_DOOR).length() < 1e-4),
                "a reconnect 120 m from where the relay holds us is left there: {stand_at:?}"
            ),
            other => panic!("{other:?}"),
        }
    }

    /// The decision on its own, every case. Seen red 2026-10-03:
    ///  - with the 65b3e2c0c rule (`arriving` alone): "not arriving, spawned afresh, 148 m away:
    ///    expected true" (the second review's freeze);
    ///  - with "always stand where held" (the wrong fix): "a reconnect 3 m away keeps walking:
    ///    expected false".
    #[test]
    fn stand_where_held_decides_from_arrival_rejoin_and_distance() {
        let cases: [(bool, bool, Option<f32>, bool, &str); 9] = [
            (true, false, Some(0.0), true, "arriving, spawned afresh at the camera"),
            (true, true, Some(3.0), true, "arriving (a game restart inside the grace)"),
            (false, false, Some(148.0), true, "not arriving, spawned afresh, 148 m away"),
            (false, false, Some(2.0), true, "not arriving, spawned afresh, 2 m away"),
            (false, true, Some(3.0), false, "a reconnect 3 m away keeps walking"),
            (false, true, Some(89.9), false, "a reconnect inside the backstop keeps walking"),
            (false, true, Some(90.1), true, "a reconnect beyond the backstop comes back"),
            (false, true, None, false, "a reconnect the welcome says nothing about"),
            (false, false, None, true, "spawned afresh, no entry (stands at the door)"),
        ];
        for (arriving, rejoin, gap, want, what) in cases {
            assert_eq!(stand_where_held(arriving, rejoin, gap), want, "{what}: expected {want}");
        }
    }

    /// Which welcome is an arrival. The second review: hard-coding the arrival decision to false
    /// passed every test. Seen red 2026-10-03 with `is_arrival` returning false: "the first
    /// welcome since the world loaded is an arrival".
    #[test]
    fn the_first_welcome_and_another_servers_are_arrivals() {
        assert!(is_arrival(None, SERVER), "the first welcome since the world loaded is an arrival");
        assert!(is_arrival(Some("http://elsewhere:3210"), SERVER), "a welcome from another server is an arrival");
        assert!(!is_arrival(Some(SERVER), SERVER), "a later welcome from the same server is not");
    }

    /// Another ship (a different hash), no ship named at all, or no ship of our own: refuse,
    /// with the one sentence, never move the home, and never give a plot back (a plot that
    /// is not ours to give).
    #[test]
    fn another_ship_is_refused_in_one_plain_sentence() {
        let ship = booted();
        for w in [
            welcome(Some("p2"), "0123456789abcdef"),
            serde_json::json!({ "type": "game_welcome", "player_id": 7, "home_plot": null }),
        ] {
            match plan_welcome(Some(&ship), &w, &arriving(P1_DOOR)) {
                WelcomeHome::Refuse { sentence, give_up_plot } => {
                    assert_eq!(sentence, SHIP_MISMATCH);
                    assert!(!give_up_plot);
                }
                other => panic!("a different ship must be refused: {other:?}"),
            }
        }
        assert!(matches!(plan_welcome(None, &welcome(Some("p1"), &ship.ship_hash()), &arriving(P1_DOOR)), WelcomeHome::Refuse { .. }));
        // One sentence: one full stop, at the end.
        assert_eq!(SHIP_MISMATCH.matches(". ").count(), 0);
        assert!(SHIP_MISMATCH.ends_with('.'));
    }

    /// A plot our home does not fit (here p2 made narrower than the home, on a ship whose hash
    /// the relay shares): refuse, and GIVE THE PLOT BACK as we leave, or it is held for good by
    /// someone who never lives there (the second review). Seen red 2026-10-03 with
    /// `give_up_plot` false on that path: "a home that does not fit its plot gives it back:
    /// false".
    #[test]
    fn a_home_that_does_not_fit_its_plot_gives_it_back() {
        let mut file = ShipStructure::load_ship_file(&data_dir()).unwrap();
        let p2 = file.plots.iter().position(|p| p.id == "p2").unwrap();
        file.plots[p2].size.0 = 30.0;
        let design = booted().home_design().unwrap();
        let ship = file.clone().assemble(design, "p1").expect("the home stands on p1");
        let w = welcome(Some("p2"), &ship.ship_hash());
        match plan_welcome(Some(&ship), &w, &arriving(P1_DOOR)) {
            WelcomeHome::Refuse { sentence, give_up_plot } => {
                assert!(give_up_plot, "a home that does not fit its plot gives it back: {give_up_plot}");
                assert!(sentence.contains("does not fit"), "{sentence}");
            }
            other => panic!("{other:?}"),
        }
    }

    /// What the game puts in its `game_join`: the ship it draws and its own home's door,
    /// plot-local. No ship: an empty hash (never a match: the relay refuses the join) and no
    /// door. A home with no authored door: no door (each side takes the plot's middle). Seen red
    /// 2026-10-03 with `add_join_fields` adding nothing (the first 1b join): "the join names no
    /// ship: null"; and with `home_arrival_local` sending the middle of the plot the home was
    /// built on (65b3e2c0c, the second review's finding 6): "a home with no door names none:
    /// [27.5,44.5]".
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
        ship.zones[home].body.spawn = None;
        let mut doorless = serde_json::json!({ "type": "game_join" });
        add_join_fields(&mut doorless, Some(&ship));
        assert!(doorless.get("home_spawn").is_none(), "a home with no door names none: {}", doorless["home_spawn"]);
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

    /// A world with what the player may have built: on p1 (the home), a finished chest with
    /// its uid, a wall still going up, a parked vehicle driving to a point in the home; in the
    /// Commons, a Dev-built piece and a vehicle; and a piece built on a planet whose site-local
    /// pose happens to read as a point inside p1.
    fn built_world() -> hecs::World {
        use crate::systems::construction::{Construction, PlanetSite, Structure};
        let tf = |x: f32, z: f32| Transform { position: Vec3::new(x, 0.0, z), rotation: glam::Quat::IDENTITY, scale: Vec3::ONE };
        let built = |id: &str, uid: u32| Structure { blueprint_id: id.into(), health: 42.0, max_health: 100.0, provides: None, uid };
        let vehicle = || crate::ecs::components::Vehicle { item_id: "truck_pickup_0".into() };
        let mut w = hecs::World::new();
        w.spawn((tf(20.0, 30.0), built("chest", 7)));
        w.spawn((tf(10.0, 12.0), Construction { blueprint_id: "wood_wall".into(), progress: 3.0, build_time: 10.0, builder_key: None }));
        w.spawn((
            tf(40.0, 70.0),
            vehicle(),
            crate::ecs::components::VehicleRoute { dest: Vec3::new(45.0, 0.0, 80.0), speed_mps: 3.0, arrive_radius: 2.0 },
        ));
        w.spawn((tf(80.0, 40.0), built("wood_wall", 8))); // the Commons, x 65..99
        w.spawn((tf(90.0, 50.0), vehicle()));
        w.spawn((tf(25.0, 35.0), built("wood_wall", 9), PlanetSite { body: "earth".into(), origin: glam::DVec3::new(6.371e6, 0.0, 0.0) }));
        w
    }

    /// FINDING 3 of the second review: the pieces built aboard and the parked vehicles in the
    /// home go with it (99 m to p2); the ship's spaces and a planet site keep theirs; a chest
    /// keeps its uid (its contents are filed under it) and health; a scaffold its progress; a
    /// vehicle's destination in the home moves with it. Seen red 2026-10-03 with
    /// `carry_built_pieces` moving nothing (65b3e2c0c carried neither): "two pieces and one
    /// vehicle were in the home: left: (0, 0), right: (2, 1)".
    #[test]
    fn what_the_player_built_and_parked_in_the_home_goes_with_it() {
        use crate::systems::construction::{Construction, PlanetSite, Structure};
        let p1 = booted().home_plot().unwrap().aabb();
        let mut w = built_world();
        let delta = Vec3::new(0.0, 0.0, 99.0);
        assert_eq!(carry_built_pieces(&mut w, p1, delta), (2, 1), "two pieces and one vehicle were in the home");
        let at = |w: &hecs::World, id: &str| -> Vec3 {
            w.query::<(&Transform, &Structure)>().iter().find(|(_, (_, s))| s.blueprint_id == id).map(|(_, (t, _))| t.position).unwrap()
        };
        let chest = at(&w, "chest");
        assert_eq!(chest, Vec3::new(20.0, 0.0, 129.0), "the chest stayed at {chest:?}, in p1");
        let (uid, health) = w.query::<&Structure>().iter().find(|(_, s)| s.blueprint_id == "chest").map(|(_, s)| (s.uid, s.health)).unwrap();
        assert_eq!((uid, health), (7, 42.0), "moved, not built again");
        let scaffold = w.query::<(&Transform, &Construction)>().iter().map(|(_, (t, c))| (t.position, c.progress)).next().unwrap();
        assert_eq!(scaffold, (Vec3::new(10.0, 0.0, 111.0), 3.0));
        // The ship's own spaces and the planet site keep their pieces.
        let commons_wall = w.query::<(&Transform, &Structure)>().iter().find(|(_, (_, s))| s.uid == 8).map(|(_, (t, _))| t.position).unwrap();
        assert_eq!(commons_wall, Vec3::new(80.0, 0.0, 40.0));
        let planet = w.query::<(&Transform, &PlanetSite)>().iter().map(|(_, (t, _))| t.position).next().unwrap();
        assert_eq!(planet, Vec3::new(25.0, 0.0, 35.0), "a planet piece never moves with the home");
        // Vehicles: the home's goes, with its destination; the Commons one stays.
        let vehicles: Vec<(Vec3, Option<Vec3>)> = w
            .query::<(&Transform, &crate::ecs::components::Vehicle, Option<&crate::ecs::components::VehicleRoute>)>()
            .iter()
            .map(|(_, (t, _, r))| (t.position, r.map(|r| r.dest)))
            .collect();
        assert!(vehicles.contains(&(Vec3::new(40.0, 0.0, 169.0), Some(Vec3::new(45.0, 0.0, 179.0)))), "{vehicles:?}");
        assert!(vehicles.contains(&(Vec3::new(90.0, 0.0, 50.0), None)), "{vehicles:?}");
        // The rig's report lists the aboard pieces (not the planet's) and the vehicles.
        let (pieces, cars) = built_positions(&w);
        assert_eq!((pieces.len(), cars.len()), (3, 2));
        assert!(pieces.contains(&chest));
        // Moving by nothing moves nothing.
        assert_eq!(carry_built_pieces(&mut built_world(), p1, Vec3::ZERO), (0, 0));
    }

    /// THE SAVE'S FRAME (finding 3, followed through): after the home moved to p2, the save
    /// writes its pieces and vehicles as if the home stood on the default plot, since every
    /// world entry builds it there; loading that save into a world whose home stands on p2
    /// carries them back. Round trip: the chest built on p1 at (20, 30), carried to p2, saved
    /// at p1's (20, 30), loaded and carried to p2's (20, 129). Pieces outside the home and on a
    /// planet are saved where they stand. Seen red 2026-10-03 with `into_default_frame` writing
    /// positions as they stand (the 65b3e2c0c save): "saved with the home on p2, the chest is
    /// written at [20.0, 0.0, 129.0]", which the next world entry, with the home on p1, would
    /// have shown 99 m outside the home.
    #[test]
    fn the_save_holds_the_home_on_its_default_plot() {
        let ship1 = booted();
        let ship2 = ShipStructure::load_and_assemble(&data_dir(), Some("p2")).unwrap();
        let frame = home_frame(&ship2).expect("an assembled ship has a frame");
        assert_eq!(frame.offset(), Vec3::new(0.0, 0.0, 99.0));
        assert_eq!(home_frame(&ship1).unwrap().offset(), Vec3::ZERO, "on the default plot the frame is the ship's");
        // The live world after the move to p2.
        let mut w = built_world();
        carry_built_pieces(&mut w, frame.default, frame.offset());
        let mut save = crate::save_load::extract_world_save(&w);
        crate::save_load::into_default_frame(&mut save, &frame);
        let chest = save.constructions.iter().find(|c| c.blueprint_id == "chest").unwrap().position;
        assert_eq!(chest, [20.0, 0.0, 30.0], "saved with the home on p2, the chest is written at {chest:?}");
        let commons = save.constructions.iter().find(|c| c.uid == 8).unwrap().position;
        assert_eq!(commons, [80.0, 0.0, 40.0], "a piece in the Commons is saved where it stands");
        let planet = save.constructions.iter().find(|c| c.site.is_some()).unwrap().position;
        assert_eq!(planet, [25.0, 0.0, 35.0], "a planet piece keeps its site-local pose");
        let cars: Vec<[f32; 3]> = save.deployed_vehicles.iter().map(|v| v.position).collect();
        assert!(cars.contains(&[40.0, 0.0, 70.0]) && cars.contains(&[90.0, 0.0, 50.0]), "{cars:?}");
        // Loaded into a world whose home stands on p2: carried back to where they were.
        let mut fresh = hecs::World::new();
        crate::save_load::apply_save_to_world(&mut fresh, &save);
        carry_built_pieces(&mut fresh, frame.default, frame.offset());
        let back = built_positions(&fresh).0;
        assert!(back.contains(&Vec3::new(20.0, 0.0, 129.0)), "the chest is back on p2: {back:?}");
        assert!(back.contains(&Vec3::new(80.0, 0.0, 40.0)), "the Commons piece did not move: {back:?}");
    }
}
