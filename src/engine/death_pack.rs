//! The engine's half of the Death setting (2026-10-04; the rules and the arithmetic are
//! `systems::death_pack`, the design docs/design/death-and-your-pack.md).
//!
//! What the frame does with a death and the packs it leaves:
//!   - `after_tick`, right after the systems tick: surfaces a death the systems recorded
//!     (the "player_death" slot FoodSystem and CombatSystem fill) as the death screen and,
//!     in Realistic, leaves the backpack's contents in a pack where the player fell
//!     (`spot_for_death`: the floor under them aboard, the ground under them on a planet,
//!     the nearest place that can be walked to from open space or deep water). Done before
//!     the periodic save in the same frame, so a save never holds a death with the backpack
//!     still full. Then each pack's clock counts the frame, only in the world and alive.
//!   - `frame`, with the other walk-up prompts: the crosshair prompt at a pack in reach.
//!   - `activate`, from the E chain in lib.rs: take back as much as fits.
//!   - `note_body`, from the celestial loop: where a body with a pack on it is drawn.
//!   - `push_render_objects`, with the built pieces: the packs, and their markers on the HUD
//!     ("Your pack · 120 m", the direction-placed marker of BUG-148).

use glam::{DQuat, DVec3, Quat, Vec3};

use crate::engine::state::EngineState;
use crate::renderer::RenderObject;
use crate::systems::death_pack::{self as dp, items_words, minutes_words, DeathMode, DeathNote, Landing, LeftPack, PackEvent, PackPlace, PackSpot};

/// What the engine keeps for the packs (`EngineState::death_packs`).
#[derive(Default)]
pub(crate) struct PackEngine {
    /// The frames the celestial loop placed the bodies with packs on in, this frame (body,
    /// centre in render space, orientation): noted by `note_body`, taken by
    /// `push_render_objects`, so never a frame stale.
    frames: Vec<BodyAt>,
    /// The unit box every part of a pack is drawn with, and the canvas and strap materials.
    mesh: Option<usize>,
    mats: Option<[usize; 2]>,
}

/// One body's placement in render space this frame: its centre and its orientation.
pub(crate) type BodyAt = (String, DVec3, DQuat);

/// The Death setting as the player has it (Settings > Gameplay > Death).
fn mode(state: &EngineState) -> DeathMode {
    DeathMode::from_realistic(state.gui_state.settings.death_realistic)
}

/// Right after the systems tick, every frame: a new death, a death that came back with a
/// save, and the packs' clocks. See the file's header.
pub(crate) fn after_tick(state: &mut EngineState, dt: f32) {
    // A new frame: the celestial loop notes this frame's body frames after this.
    state.death_packs.frames.clear();
    let cause = state
        .data_store
        .get::<std::sync::Mutex<Option<String>>>("player_death")
        .and_then(|m| m.lock().ok().and_then(|mut s| s.take()));
    if let Some(cause) = cause {
        state.gui_state.player_death_cause = Some(cause);
        let mode = mode(state);
        // Out of the world (the menus tick the systems too) there is nowhere to leave it.
        let spot = if mode == DeathMode::Realistic && state.world_loaded { spot_for_death(state) } else { None };
        let note = dp::on_death(&mut state.game_world.world, mode, spot);
        if let DeathNote::Left { items, place_words, .. } = &note {
            log::info!("[Death] Realistic: {items} items left in a pack {place_words}");
        }
        state.gui_state.death_pack.note = Some(note);
    }
    if state.gui_state.player_death_cause.is_none() {
        // Alive (respawned, or a living save loaded): no death on screen.
        state.gui_state.death_pack.note = None;
        dp::settle(&mut state.game_world.world);
    } else if state.gui_state.death_pack.note.is_none() {
        // A death that came back with a save: quit on the death screen, loaded again.
        state.gui_state.death_pack.note = Some(dp::note_for_loaded_death(&state.game_world.world, mode(state)));
    }
    // The packs' clocks count play: in the world and alive, never the menus before it.
    if state.world_loaded && state.gui_state.player_death_cause.is_none() {
        let events = dp::count_down(&mut state.game_world.world, f64::from(dt), &state.gui_state.death_pack.rules);
        let keep = state.gui_state.death_pack.rules.keep_minutes_of_play;
        for e in events {
            state.gui_state.pending_notices.push(pack_notice(&e, keep));
        }
    }
}

/// The showcase `die` verb (engine/ipc.rs): the player dies of `cause` the way the systems
/// kill them, health to nothing, `Dead` with the cause, and the cause in the "player_death"
/// slot, so `after_tick` surfaces it with what the Death mode costs. Returns what it did.
pub(crate) fn dev_die(state: &mut EngineState, cause: &str) -> String {
    let world = &mut state.game_world.world;
    let Some(p) = dp::player(world) else { return "no player to die".to_string() };
    if world.get::<&crate::ecs::components::Dead>(p).is_ok() {
        return "already dead".to_string();
    }
    if let Ok(mut h) = world.get::<&mut crate::ecs::components::Health>(p) {
        h.current = 0.0;
    }
    let cause = if cause.trim().is_empty() { "injuries" } else { cause.trim() };
    let _ = world.insert_one(p, crate::ecs::components::Dead { cause: cause.to_string(), ..Default::default() });
    if let Some(slot) = state.data_store.get::<std::sync::Mutex<Option<String>>>("player_death") {
        if let Ok(mut s) = slot.lock() {
            *s = Some(cause.to_string());
        }
    }
    format!("died of {cause} (Death mode {:?})", mode(state))
}

/// The notice a pack's clock gives (`count_down`).
pub(crate) fn pack_notice(e: &PackEvent, keep_minutes: f64) -> String {
    match e {
        PackEvent::Warn { place_words, items, minutes_left } => format!(
            "Your pack {place_words} will be gone in {} of play. It holds {}.",
            minutes_words(*minutes_left),
            items_words(*items)
        ),
        PackEvent::Gone { place_words, items } => format!(
            "Your pack {place_words} is gone: it lay there for {} of play. The {} in it {} lost.",
            minutes_words(keep_minutes),
            items_words(*items),
            if *items == 1 { "is" } else { "are" }
        ),
    }
}

/// Where a Realistic death leaves the pack, from where the player fell: on a planet's
/// ground (the frame lock holds a body with a known surface) the ground under them, or the
/// nearest dry ground from deep water; aboard the floor under them, or from open space
/// (outside every room, or off the ship near no body) the nearest floor that can be walked
/// to. On or over a body with no ground to walk to near them (no dry ground within the shore
/// search, or no known surface) the ship's nearest floor too, said as such
/// (`aboard_landing`). None only with no rooms at all (nothing to stand on has loaded).
fn spot_for_death(state: &EngineState) -> Option<PackSpot> {
    let rules = &state.gui_state.death_pack.rules;
    // Set when the frame lock holds a body but no ground on it could take the pack.
    let mut no_ground = false;
    if let Some(body) = state.frame_lock_body.as_deref() {
        if let Some(g) = crate::engine::planet_build::Ground::for_body(state, body) {
            let eye_h = f64::from(state.controller.eye_height());
            let found = dp::ground_spot(state.frame_lock_anchor, eye_h, rules, |d| {
                let r = g.radius(d);
                (r, g.under_water(d, r))
            });
            if let Some(s) = found {
                return Some(PackSpot {
                    place: PackPlace::Ground { body: body.to_string(), at: s.at.to_array() },
                    place_words: format!("on {}", body_name(body)),
                    landing: s.landing,
                });
            }
        }
        // No dry ground within the search, or no known surface: the ship's nearest floor.
        no_ground = true;
    }
    // The camera in the ship's own metres (the home is drawn at home-local + station_off,
    // aboard and off the station alike: ipc.rs `camera_home_position`).
    let feet = crate::engine::ipc::camera_home_position(state.camera.position, state.station_off)
        - Vec3::Y * state.controller.eye_height();
    let rooms: Vec<(Vec3, Vec3)> = state.gui_state.room_bounds.iter().map(|r| (r.min, r.max)).collect();
    let s = dp::aboard_spot(
        &rooms,
        feet,
        state.controller.ground_floor(),
        rules.wall_clearance_m,
        rules.room_slack_m,
        rules.open_space_m,
    )?;
    let name = state.gui_state.room_bounds.get(s.room).map_or("", |r| r.display_name.as_str());
    Some(PackSpot {
        place: PackPlace::Aboard { at: s.at.to_array() },
        place_words: room_words(name),
        landing: aboard_landing(s.moved_m, no_ground),
    })
}

/// Why a pack left aboard lies where it does (`spot_for_death`): `moved_m` is how far the
/// nearest floor was when the player was outside every room (`dp::aboard_spot`), and
/// `no_ground` says they died on or over a body that had no ground to walk to near them.
pub(crate) fn aboard_landing(moved_m: Option<f32>, no_ground: bool) -> Landing {
    match moved_m.map(f64::from) {
        None => Landing::WhereYouFell,
        Some(dist_m) if no_ground => Landing::FromNoGround { dist_m },
        Some(dist_m) => Landing::FromOpenSpace { dist_m },
    }
}

/// "in the Kitchen"; "aboard the ship" for a room with no name of its own (an anonymous
/// room is named by its id, "room_7").
pub(crate) fn room_words(name: &str) -> String {
    let n = name.trim();
    if n.is_empty() || n.contains('_') {
        "aboard the ship".to_string()
    } else if n.get(..4).is_some_and(|p| p.eq_ignore_ascii_case("the ")) {
        format!("in {n}")
    } else {
        format!("in the {n}")
    }
}

/// A body's name ("Earth"), from the solar system's data; its id when it has none.
fn body_name(id: &str) -> String {
    crate::cosmos::find_body(id).map_or_else(|| id.to_string(), |b| b.name.clone())
}

/// The pack in reach in front of the player, and how many items it holds; None while
/// anything the E chain in lib.rs tries first is targeted (a vehicle, an animal, a person, a
/// door panel, a home machine, a built bed, chest or door), or a page, the editor or the
/// death screen is up. The pack comes after all of them in the chain, so its prompt never
/// promises an E another target takes.
fn target(state: &EngineState) -> Option<(hecs::Entity, u32)> {
    let g = &state.gui_state;
    let busy = g.active_page != crate::gui::GuiPage::None
        || g.construction_active
        || g.build_placing.is_some()
        || g.showroom_active
        || g.player_death_cause.is_some()
        || state.camera.mode != crate::renderer::camera::CameraMode::FirstPerson
        || state.driving_vehicle.is_some()
        || state.targeted_vehicle.is_some()
        || state.targeted_livestock.is_some()
        || g.targeted_npc.is_some()
        || g.targeted_control_panel.is_some()
        || g.targeted_machine.is_some()
        || !g.structure_prompt.is_empty();
    if busy {
        return None;
    }
    let rules = &g.death_pack.rules;
    let half = f64::from(rules.look.size_m.1) * 0.5;
    let (eye, look) = (state.camera.position.as_dvec3(), state.camera.forward().as_dvec3());
    // On a planet the camera stays put while the frame moves under it, so the eye there is the
    // frame lock's anchor in the body's frame, and the look turned into it (planet_build.rs).
    let rot = crate::engine::planet_build::body_render_rot(state.station_ride, state.station_world_rot, state.current_spin);
    let off = state.station_off.as_dvec3();
    let mut best: Option<(hecs::Entity, f64, u32)> = None;
    for (e, p) in state.game_world.world.query::<&LeftPack>().iter() {
        let dist = match &p.place {
            PackPlace::Aboard { at } => {
                let mid = Vec3::from_array(*at).as_dvec3() + off + DVec3::Y * half;
                faces(eye, look, mid, rules.reach_m, rules.facing_cos)
            }
            PackPlace::Ground { body, at } if state.frame_lock_body.as_deref() == Some(body.as_str()) => {
                let at = DVec3::from_array(*at);
                let mid = at + at.normalize_or_zero() * half;
                faces(state.frame_lock_anchor, rot.inverse() * look, mid, rules.reach_m, rules.facing_cos)
            }
            PackPlace::Ground { .. } => None,
        };
        if let Some(d) = dist {
            if best.as_ref().map_or(true, |b| d < b.1) {
                best = Some((e, d, p.count()));
            }
        }
    }
    best.map(|(e, _, n)| (e, n))
}

/// How far `target` is from `eye` when it is within `reach` and no wider than `facing_cos`
/// (a cosine) off the unit `look`; None otherwise. f64, so it holds at planet scale.
pub(crate) fn faces(eye: DVec3, look: DVec3, target: DVec3, reach: f64, facing_cos: f64) -> Option<f64> {
    let to = target - eye;
    let d = to.length();
    if d > reach {
        return None;
    }
    if d < 1e-6 {
        return Some(0.0);
    }
    (to / d).dot(look.normalize_or_zero()).ge(&facing_cos).then_some(d)
}

/// Once a frame, with the other walk-up prompts: the crosshair prompt at a pack in reach.
pub(crate) fn frame(state: &mut EngineState) {
    let prompt = target(state).map(|(_, n)| {
        let key = crate::input::bindings::pretty_key_name(state.gui_state.keybinds.pair(crate::input::bindings::GameAction::Interact).0);
        format!("[{key}] Take back your pack ({})", items_words(n))
    });
    state.gui_state.death_pack.prompt = prompt.unwrap_or_default();
}

/// The E press at a pack in reach (the E chain in lib.rs, after the built pieces): as much
/// as the backpack holds goes back in, the rest stays in the pack. False when no pack is in
/// reach, so the chain carries on.
pub(crate) fn activate(state: &mut EngineState) -> bool {
    if state.gui_state.death_pack.prompt.is_empty() {
        return false;
    }
    let Some((pack, _)) = target(state) else { return false };
    let items = state.data_store.get::<crate::systems::inventory::ItemRegistry>("item_registry");
    let Some(r) = dp::take_back(&mut state.game_world.world, pack, items) else { return false };
    state.gui_state.pending_toasts.push((take_back_words(r), crate::gui::ToastKind::Info));
    if r.taken > 0 {
        state.pending_sfx.push(("sfx.inventory_pickup", "audio/ui/inventory_pickup.ogg"));
    }
    state.gui_state.death_pack.prompt.clear();
    true
}

/// What an E press at a pack says it did.
pub(crate) fn take_back_words(r: dp::TakeBack) -> String {
    match (r.taken, r.left) {
        (t, 0) => format!("You took back everything in your pack: {}.", items_words(t)),
        (0, 1) => "Your backpack has no room: the 1 item is still in your pack.".to_string(),
        (0, l) => format!("Your backpack has no room: all {} are still in your pack.", items_words(l)),
        (t, l) => format!(
            "You took back {}. {} still in your pack: your backpack has no room for them.",
            items_words(t),
            if l == 1 { "1 item is".to_string() } else { format!("{l} items are") }
        ),
    }
}

/// From the celestial loop (planet_build.rs `note_body`), for every body it places: keep
/// the frame of a body with a pack on it, for `push_render_objects` later in the frame.
pub(crate) fn note_body(state: &mut EngineState, body: &str, render_off: DVec3, rot: DQuat) {
    let has = state
        .game_world
        .world
        .query::<&LeftPack>()
        .iter()
        .any(|(_e, p)| matches!(&p.place, PackPlace::Ground { body: b, .. } if b == body));
    if has {
        state.death_packs.frames.push((body.to_string(), render_off, rot));
    }
}

/// Where each pack is in render space and how far from the camera, with its marker label:
/// "Your pack" for one, "Your pack 1" (the newest), "Your pack 2", ... for several. A pack
/// aboard is at its ship-metre place plus `station_off` (where the home frame is drawn); one
/// on a body at that body's centre plus its orientation times the place, from this frame's
/// `frames` (a body not placed this frame gives no marker). Each is aimed at the pack's middle,
/// `half` metres over where it lies. Pure, so the marker is tested without a window.
pub(crate) fn pack_markers(packs: &[LeftPack], station_off: Vec3, frames: &[BodyAt], camera: Vec3, half: f32) -> Vec<(String, Vec3, f64)> {
    let mut order: Vec<&LeftPack> = packs.iter().collect();
    order.sort_by(|a, b| a.played_s.total_cmp(&b.played_s));
    let several = order.len() > 1;
    order
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let at = render_mid(p, station_off, frames, half)?;
            let label = if several { format!("Your pack {}", i + 1) } else { "Your pack".to_string() };
            Some((label, at.as_vec3(), (at - camera.as_dvec3()).length()))
        })
        .collect()
}

/// A pack's middle in render space (`pack_markers`), f64 until the end.
fn render_mid(p: &LeftPack, station_off: Vec3, frames: &[BodyAt], half: f32) -> Option<DVec3> {
    match &p.place {
        PackPlace::Aboard { at } => Some((Vec3::from_array(*at) + station_off + Vec3::Y * half).as_dvec3()),
        PackPlace::Ground { body, at } => {
            let (_, off, rot) = frames.iter().find(|f| f.0 == *body)?;
            let at = DVec3::from_array(*at);
            Some(*off + *rot * (at + at.normalize_or_zero() * f64::from(half)))
        }
    }
}

/// Draw the packs and mark them on the HUD (lib.rs, with the built pieces, after the
/// celestial loop has noted this frame's body frames). A pack aboard goes into the home
/// frame's list (`home`, shifted to the station with the rest of the home); one on a
/// planet into the celestial list at the body's frame, standing up from its ground. The
/// markers join the tracked markers the HUD draws (`target_markers`, cleared each frame by
/// the station block, which runs before this).
pub(crate) fn push_render_objects(state: &mut EngineState, home: &mut Vec<RenderObject>, celestial: &mut Vec<RenderObject>) {
    let frames = std::mem::take(&mut state.death_packs.frames);
    let packs = dp::packs(&state.game_world.world);
    if packs.is_empty() {
        return;
    }
    let look = state.gui_state.death_pack.rules.look.clone();
    if state.death_packs.mesh.is_none() {
        let mesh = crate::renderer::mesh::Mesh::box_xyz(&state.renderer.device, 1.0, 1.0, 1.0);
        state.death_packs.mesh = Some(state.renderer.add_mesh(mesh));
    }
    if state.death_packs.mats.is_none() {
        let (c, s) = (look.canvas, look.straps);
        // World materials, not UI: the pack's canvas and its straps (data/world/death.ron).
        let canvas = state.renderer.add_material_typed([c.0, c.1, c.2, 1.0], 0.0, 0.85, 0.0);
        let straps = state.renderer.add_material_typed([s.0, s.1, s.2, 1.0], 0.0, 0.7, 0.0);
        state.death_packs.mats = Some([canvas, straps]);
    }
    let (Some(mesh), Some([canvas, straps])) = (state.death_packs.mesh, state.death_packs.mats) else { return };
    for p in &packs {
        let (base, rot, list) = match &p.place {
            PackPlace::Aboard { at } => (Vec3::from_array(*at), Quat::IDENTITY, &mut *home),
            PackPlace::Ground { body, at } => {
                let Some((_, off, rot)) = frames.iter().find(|f| f.0 == *body) else { continue };
                let at = DVec3::from_array(*at);
                let up = (*rot * at.normalize_or_zero()).as_vec3().normalize_or_zero();
                ((*off + *rot * at).as_vec3(), Quat::from_rotation_arc(Vec3::Y, up), &mut *celestial)
            }
        };
        for (offset, scale, strap) in pack_parts(look.size_m) {
            list.push(RenderObject {
                fade: 0.0,
                position: base + rot * offset,
                rotation: rot,
                scale,
                mesh,
                material: if strap { straps } else { canvas },
            });
        }
    }
    let half = look.size_m.1 * 0.5;
    let markers = pack_markers(&packs, state.station_off, &frames, state.camera.position, half);
    state.gui_state.target_markers.extend(markers);
}

/// A pack's parts as unit boxes (the box is bottom-origin): (offset of its base from the
/// pack's base, size, strap or canvas). The main bag `size` (width, height, depth), a lid
/// on top, a pocket on the front (+Z) and two shoulder straps down the back (-Z), all in
/// proportion to the bag, so data/world/death.ron's size scales the whole pack.
pub(crate) fn pack_parts(size: (f32, f32, f32)) -> [(Vec3, Vec3, bool); 5] {
    let (w, h, d) = size;
    [
        (Vec3::ZERO, Vec3::new(w, h, d), false),
        (Vec3::new(0.0, h, 0.0), Vec3::new(w * 1.06, h * 0.14, d * 1.08), true),
        (Vec3::new(0.0, h * 0.08, d * 0.5), Vec3::new(w * 0.76, h * 0.42, d * 0.3), false),
        (Vec3::new(-w * 0.26, h * 0.12, -d * 0.52), Vec3::new(w * 0.14, h * 0.8, d * 0.1), true),
        (Vec3::new(w * 0.26, h * 0.12, -d * 0.52), Vec3::new(w * 0.14, h * 0.8, d * 0.1), true),
    ]
}

#[cfg(test)]
#[path = "death_pack_tests.rs"]
mod tests;
