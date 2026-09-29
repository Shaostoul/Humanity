//! Placing a built piece in the world (2026-09-27): the ghost, the keys, the
//! hint.
//!
//! Build on the Crafting page used to drop the piece 4 m ahead on the floor,
//! unturned, with no preview, so walls only ran east-west and a roof could
//! never go overhead. Now Build puts the blueprint IN HAND: the page closes,
//! a see-through scaffold (the ghost) follows the crosshair, the Toggle roof
//! key (R by default) turns it a quarter, Interact (E) builds it where the
//! ghost stands and keeps the piece in hand for the next one, and Esc puts
//! it down. Where it lands is `construction::placement::placement_pose`: x
//! and z on the metre grid, y on the floor, or on top of the walls for a
//! roof (`mount: OnTop` in the blueprint data). The ghost's pose is kept on
//! the placing state every frame and E builds exactly that pose, so the
//! piece goes where the ghost stood.
//!
//! WHERE (2026-09-27, the real fix for BUG-102): aboard, in the home frame,
//! on the deck; on a planet's ground, in the build site the player stands
//! in, on the ground under the crosshair (`engine::planet_build::ghost`).
//! Where a piece cannot go (open space, a vehicle, flying, the sea) the hint
//! says so. A piece whose box already stands there is shown but not built
//! again (no double spend).
//!
//! R is the roof toggle when nothing is in hand; while placing it turns the
//! piece (the Controls page says so), because one key holds one action and R
//! is where every building game puts rotate. lib.rs calls `frame` once a
//! frame and `key` from its key handler, ahead of the E chain and the menu
//! Escape.

use crate::engine::planet_build;
use crate::engine::state::EngineState;
use crate::gui::{BuildPlacing, GuiPage, GuiState};
use crate::input::bindings::{pretty_key_name, GameAction};
use crate::ecs::components::Transform;
use crate::ship::wall_collision::WallSegment;
use crate::systems::construction::{doorway, placement, BlueprintRegistry, BuildRequest, DoorOpen, PlanetSite, Structure};
use glam::Vec3;

/// What a key press does to the piece in hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlaceKey {
    /// Build it where the ghost stands (Interact).
    Build,
    /// Turn it a quarter (the Toggle roof key).
    Turn,
    /// Put it down (Esc).
    Stop,
    /// Take down the finished piece in view (the Swing tool key, F).
    TakeDown,
}

/// The placement meaning of a key press, or None when nothing is in hand, a
/// page is open, or the key is not one of placing's (so it keeps its usual
/// meaning). Pure over the GUI state, so it is tested without a window.
pub(crate) fn key_action(gui: &GuiState, key_name: &str, escape: bool) -> Option<PlaceKey> {
    if gui.build_placing.is_none() || gui.active_page != GuiPage::None {
        return None;
    }
    if escape {
        Some(PlaceKey::Stop)
    } else if gui.keybinds.is(GameAction::ToggleRoof, key_name) {
        Some(PlaceKey::Turn)
    } else if gui.keybinds.is(GameAction::Interact, key_name) {
        Some(PlaceKey::Build)
    } else if gui.keybinds.is(GameAction::AttackSwing, key_name) {
        Some(PlaceKey::TakeDown)
    } else {
        None
    }
}

/// A key press (lib.rs's key handler, presses only). Returns true when
/// placing used the key, so the handler stops there. A held key's repeats
/// are swallowed without acting, so holding E cannot build a stack of walls
/// in one spot.
pub(crate) fn key(state: &mut EngineState, key_name: &str, escape: bool, repeat: bool) -> bool {
    let Some(action) = key_action(&state.gui_state, key_name, escape) else { return false };
    if repeat {
        return true;
    }
    match action {
        PlaceKey::Stop => state.gui_state.build_placing = None,
        PlaceKey::TakeDown => take_down(state),
        PlaceKey::Turn => {
            if let Some(p) = state.gui_state.build_placing.as_mut() {
                p.quarter_turns = (p.quarter_turns + 1) % 4;
            }
        }
        PlaceKey::Build => {
            // Only where a ghost stands (first person, on foot, aboard or on
            // a planet's ground), and never twice in one spot.
            let Some(p) = state.gui_state.build_placing.as_ref() else { return false };
            let Some(pose) = p.ghost.clone() else { return false };
            if p.occupied || p.short {
                return true;
            }
            let request = BuildRequest::new(p.blueprint_id.clone(), pose).on(p.site.clone());
            if let Some(chan) = state.data_store.get::<std::sync::Mutex<Vec<BuildRequest>>>("build_request") {
                if let Ok(mut c) = chan.lock() {
                    c.push(request);
                }
            }
        }
    }
    true
}

/// Once a frame: pick up what Build was clicked on, drop it when the player
/// can no longer place (the build editor, the showroom, death), and move the
/// ghost and the hint.
pub(crate) fn frame(state: &mut EngineState) {
    if let Some(id) = state.gui_state.pending_build.take() {
        let name = state
            .data_store
            .get::<BlueprintRegistry>("blueprint_registry")
            .and_then(|r| r.get(&id))
            .map_or_else(|| id.clone(), |bp| bp.name.clone());
        // Picking up another piece keeps the turn: the next wall of a room
        // usually runs the way the last one did.
        let quarter_turns = state.gui_state.build_placing.as_ref().map_or(0, |p| p.quarter_turns);
        state.gui_state.build_placing = Some(BuildPlacing {
            blueprint_id: id,
            name,
            quarter_turns,
            ghost: None,
            site: None,
            occupied: false,
            short: false,
            hint: String::new(),
        });
        state.gui_state.active_page = GuiPage::None;
    }
    let g = &state.gui_state;
    if g.construction_active || g.showroom_active || g.player_death_cause.is_some() {
        state.gui_state.build_placing = None;
        return;
    }
    let Some(p) = state.gui_state.build_placing.as_ref() else { return };
    // Where the piece would go, in the frame the player is in: the home
    // aboard, the build site they stand in on a planet (planet_build).
    let (name, turns) = (p.name.clone(), p.quarter_turns);
    let placed = planet_build::ghost(state, &p.blueprint_id, turns);
    // On a planet only the pack counts (the home's storage is in orbit).
    let short = match &placed {
        Ok(g) if g.site.is_some() => carried_short(state, &p.blueprint_id),
        _ => None,
    };
    let keys = &state.gui_state.keybinds;
    let hint = match (&placed, &short) {
        (Ok(_), Some((item, more))) => short_hint(&name, item, *more),
        (Ok(g), None) => placing_hint(
            &name,
            turns,
            g.above_floor,
            g.occupied,
            &pretty_key_name(keys.pair(GameAction::Interact).0),
            &pretty_key_name(keys.pair(GameAction::ToggleRoof).0),
            &pretty_key_name(keys.pair(GameAction::AttackSwing).0),
        ),
        (Err(why), _) => planet_build::cannot_build_hint(&name, *why),
    };
    if let Some(p) = state.gui_state.build_placing.as_mut() {
        p.hint = hint;
        p.short = short.is_some();
        match placed {
            Ok(g) => {
                p.ghost = Some(g.pose);
                p.site = g.site;
                p.occupied = g.occupied;
            }
            Err(_) => {
                p.ghost = None;
                p.site = None;
                p.occupied = false;
            }
        }
    }
}

/// What the player's pack is short of for `blueprint_id`: the item's name
/// and how many more, or None when they carry enough
/// (`construction::materials_short` with no home storage, the rule the
/// ConstructionSystem applies to a planet build).
fn carried_short(state: &EngineState, blueprint_id: &str) -> Option<(String, u32)> {
    use crate::ecs::components::Controllable;
    use crate::systems::inventory::{Inventory, ItemRegistry};
    let bp = state.data_store.get::<BlueprintRegistry>("blueprint_registry")?.get(blueprint_id)?;
    let world = &state.game_world.world;
    let mut q = world.query::<(&Inventory, &Controllable)>();
    let (_e, (inv, _)) = q.iter().next()?;
    let (id, more) = crate::systems::construction::materials_short(bp, |id| inv.count_item(id), None)?;
    let name = state
        .data_store
        .get::<ItemRegistry>("item_registry")
        .and_then(|r| r.items.get(&id).map(|d| d.name.clone()))
        .unwrap_or(id);
    Some((name, more))
}

/// The line under the crosshair on a planet when the pack holds too little:
/// what is missing, and why the home's storage does not count.
pub(crate) fn short_hint(name: &str, item: &str, more: u32) -> String {
    format!("Placing {name}: carry {more} more {item} to build it here (on a planet you build from what you carry)   [Esc] done")
}

/// The line under the crosshair while placing. `above_floor` is how high the
/// ghost rests above the floor it is aimed at (a roof on walls); `occupied`
/// says the same piece already stands there, so E will not build it again.
/// `take_down_key` takes down the finished piece in view (`take_down`).
pub(crate) fn placing_hint(
    name: &str,
    quarter_turns: u8,
    above_floor: f32,
    occupied: bool,
    build_key: &str,
    turn_key: &str,
    take_down_key: &str,
) -> String {
    let turned = match quarter_turns % 4 {
        0 => String::new(),
        q => format!(", turned {} degrees", u32::from(q) * 90),
    };
    let on_top = if above_floor > 0.01 { format!(", on top at {above_floor:.1} m") } else { String::new() };
    if occupied {
        return format!(
            "Placing {name}{turned}{on_top}: already built here   [{turn_key}] turn   [{take_down_key}] take down   [Esc] done"
        );
    }
    format!("Placing {name}{turned}{on_top}   [{build_key}] build here   [{turn_key}] turn   [{take_down_key}] take down   [Esc] done")
}

/// How far away a piece can be taken down from, metres: the reach a piece is
/// placed within (the ghost stands 1.5 to 8 m ahead).
const TAKE_DOWN_REACH_M: f32 = 8.0;

/// What taking down the piece in view would do, or why it will not: the
/// piece, its name and the materials that go back. A store that still holds
/// anything is refused, so taking down a chest can never lose what is in it.
/// Pure over the world and the placed items, so it is tested without a window.
pub(crate) fn take_down_plan(
    world: &hecs::World,
    placed: &[crate::gui::PlacedItem],
    registry: Option<&BlueprintRegistry>,
    eye: Vec3,
    forward: Vec3,
    frame: Option<&PlanetSite>,
    walls: &[WallSegment],
) -> Result<(hecs::Entity, String, Vec<(String, u32)>), String> {
    let Some(e) = crate::systems::construction::uses::first_in_view(world, registry, eye, forward, TAKE_DOWN_REACH_M, frame, walls) else {
        return Err("Nothing built in reach to take down".to_string());
    };
    let (id, uid) = match world.get::<&Structure>(e) {
        Ok(s) => (s.blueprint_id.clone(), s.uid),
        Err(_) => return Err("Nothing built in reach to take down".to_string()),
    };
    let bp = registry.and_then(|r| r.get(&id));
    let name = bp.map_or_else(|| id.clone(), |b| b.name.clone());
    let path = crate::systems::construction::uses::storage_path(uid);
    if uid != 0 && placed.iter().any(|it| it.container == path && it.qty > 0) {
        return Err(format!("Empty the {name} before taking it down"));
    }
    Ok((e, name, bp.map(|b| b.materials.clone()).unwrap_or_default()))
}

/// Take down the finished piece the player looks at (the Swing tool key with a
/// piece in hand, 2026-09-28). Its materials go back into the pack through the
/// same channel "Take to backpack" uses, so what does not fit goes back to
/// storage and the player is told; the piece is gone. Built pieces are solid
/// (`built_piece_segments`), so without this four walls could shut a player in.
/// Every material comes back: a game choice, since nothing models what
/// dismantling breaks.
fn take_down(state: &mut EngineState) {
    let Some(f) = planet_build::player_frame(state) else {
        set_placing_note(state, "Nothing built in reach to take down".to_string());
        return;
    };
    let plan = take_down_plan(
        &state.game_world.world,
        &state.gui_state.placed_items,
        state.data_store.get::<BlueprintRegistry>("blueprint_registry"),
        f.eye,
        f.forward,
        f.site.as_ref(),
        // Aboard, the home's own walls hide what is behind them.
        if f.site.is_none() { state.wall_colliders.as_slice() } else { &[] },
    );
    match plan {
        Err(why) => set_placing_note(state, why),
        Ok((e, name, materials)) => {
            let msg = apply_take_down(&mut state.game_world.world, &state.data_store, e, &name, &materials);
            state.gui_state.pending_notices.push(msg);
        }
    }
}

/// What a take-down does, once `take_down_plan` has chosen the piece: its
/// materials go back through the "Take to backpack" channel (so what does
/// not fit returns to storage, and the player is told), the piece is gone,
/// and the returned line names what came back by the items' names. Split out
/// of `take_down` so it is tested without a window (the review of
/// 2026-09-28 found it untested; the line also printed item ids).
pub(crate) fn apply_take_down(
    world: &mut hecs::World,
    data: &crate::hot_reload::data_store::DataStore,
    e: hecs::Entity,
    name: &str,
    materials: &[(String, u32)],
) -> String {
    use crate::systems::inventory::{ItemRegistry, TransferOp};
    if let Some(chan) = data.get::<std::sync::Mutex<Vec<TransferOp>>>("inventory_transfer_ops") {
        if let Ok(mut c) = chan.lock() {
            for (id, qty) in materials {
                c.push(TransferOp { item_id: id.clone(), qty: *qty, add: true, wear: 0, quality: 0 });
            }
        }
    }
    let _ = world.despawn(e);
    let items = data.get::<ItemRegistry>("item_registry");
    let got: Vec<String> = materials
        .iter()
        .map(|(id, q)| {
            let item = items.and_then(|r| r.items.get(id).map(|d| d.name.clone())).unwrap_or_else(|| id.clone());
            format!("{q} {item}")
        })
        .collect();
    if got.is_empty() {
        format!("Took down the {name}")
    } else {
        format!("Took down the {name}: {} back", got.join(", "))
    }
}

/// A refusal shown in the placing hint's place for a moment: through the
/// notice toasts, which is where a player already looks for "why not".
fn set_placing_note(state: &mut EngineState, msg: String) {
    state.gui_state.pending_notices.push(msg);
}

/// Built pieces are solid (2026-09-28, arc C Tier B: "built pieces have no
/// collision, you walk through them"). Every finished piece in `frame` (the
/// home aboard, `None`; a planet build site on the ground) whose box reaches from above the knee to the eye becomes a blocking
/// segment for `ship::wall_collision::resolve`, beside the home's own walls:
/// a wall, a bed, a chest or a machine stops you like a wall of the home.
/// Roofs overhead and floors underfoot sit outside that band, and scaffolds are
/// not `Structure`s yet, so a piece going up can still be walked through.
///
/// `eye` is in that frame's metres. On a planet the walk calls this through
/// `planet_build::collide_on_site`, because there the player moves by the frame
/// lock's anchor rather than the camera. A wall with a door in it blocks as
/// its parts (`doorway::parts`): the wall either side, and the door while it
/// is shut; the lintel is over the head.
pub(crate) fn built_piece_segments(
    world: &hecs::World,
    registry: Option<&BlueprintRegistry>,
    frame: Option<&PlanetSite>,
    eye: Vec3,
    eye_height: f32,
) -> Vec<WallSegment> {
    let feet_y = eye.y - eye_height;
    let mut out = Vec::new();
    for (_e, (s, tf, site, open)) in world.query::<(&Structure, &Transform, Option<&PlanetSite>, Option<&DoorOpen>)>().iter() {
        if !crate::systems::construction::site::in_frame(site, frame) {
            continue;
        }
        let parts = registry.and_then(|r| r.get(&s.blueprint_id)).and_then(|bp| doorway::piece_parts(bp, tf, open.is_some()));
        let boxes: Vec<(Vec3, Vec3)> = match parts {
            Some(parts) => parts.iter().map(|(p, _)| placement::world_aabb(p)).collect(),
            None => vec![placement::world_aabb(tf)],
        };
        for (lo, hi) in boxes {
            if lo.y < eye.y && hi.y > feet_y + STEP_OVER_M {
                out.push(segment_of_box(lo, hi));
            }
        }
    }
    out
}

/// A piece lower than this (m above the feet) is stepped over, not walked into:
/// a foundation or a floor tile.
const STEP_OVER_M: f32 = 0.4;

/// The blocking segment for a box on the ground: along its longer side, inset
/// by half its thickness at each end, with that half thickness as the radius,
/// so the rounded ends stay inside the box's footprint. A square box becomes a
/// point with its half width as the radius.
fn segment_of_box(lo: Vec3, hi: Vec3) -> WallSegment {
    let (cx, cz) = ((lo.x + hi.x) * 0.5, (lo.z + hi.z) * 0.5);
    let (half_x, half_z) = ((hi.x - lo.x) * 0.5, (hi.z - lo.z) * 0.5);
    if half_x >= half_z {
        let run = half_x - half_z;
        WallSegment { a: (cx - run, cz), b: (cx + run, cz), half_thickness: half_z }
    } else {
        let run = half_z - half_x;
        WallSegment { a: (cx, cz - run), b: (cx, cz + run), half_thickness: half_x }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn holding() -> GuiState {
        let mut g = GuiState::default();
        g.active_page = GuiPage::None;
        g.build_placing = Some(BuildPlacing {
            blueprint_id: "wood_wall".into(),
            name: "Wood Wall".into(),
            quarter_turns: 0,
            ghost: None,
            site: None,
            occupied: false,
            short: false,
            hint: String::new(),
        });
        g
    }

    /// With a piece in hand and no page open, E builds, R turns and Esc
    /// stops; a rebound roof key turns instead of R; with nothing in hand or
    /// a page open, no key is placing's. Red check, run: dropping the
    /// `build_placing.is_none()` guard makes R turn nothing-in-hand and the
    /// last assertion fails.
    #[test]
    fn placing_keys_follow_the_binds_and_only_while_placing() {
        let mut g = holding();
        assert_eq!(key_action(&g, "KeyE", false), Some(PlaceKey::Build));
        assert_eq!(key_action(&g, "KeyR", false), Some(PlaceKey::Turn));
        assert_eq!(key_action(&g, "Escape", true), Some(PlaceKey::Stop));
        assert_eq!(key_action(&g, "KeyF", false), Some(PlaceKey::TakeDown), "the Swing tool key takes down");
        assert_eq!(key_action(&g, "KeyW", false), None, "walking still walks");
        g.keybinds.force_bind(GameAction::ToggleRoof, false, "KeyT");
        assert_eq!(key_action(&g, "KeyT", false), Some(PlaceKey::Turn));
        assert_eq!(key_action(&g, "KeyR", false), None);
        g.active_page = GuiPage::Inventory;
        assert_eq!(key_action(&g, "KeyE", false), None, "a page open: keys are the page's");
        let mut empty = holding();
        empty.build_placing = None;
        assert_eq!(key_action(&empty, "KeyR", false), None, "nothing in hand: R toggles the roof");
    }

    /// The hint names the piece, its turn, whether it rests on top, and the
    /// live keys; over a piece already built it says so and offers no build
    /// key; and each place a piece cannot go has its own plain reason.
    #[test]
    fn the_hint_says_the_turn_and_the_keys() {
        assert_eq!(
            placing_hint("Wood Wall", 1, 0.0, false, "E", "R", "F"),
            "Placing Wood Wall, turned 90 degrees   [E] build here   [R] turn   [F] take down   [Esc] done"
        );
        assert_eq!(
            placing_hint("Wood Roof", 4, 3.0, false, "E", "T", "F"),
            "Placing Wood Roof, on top at 3.0 m   [E] build here   [T] turn   [F] take down   [Esc] done"
        );
        let twice = placing_hint("Wood Wall", 0, 0.0, true, "E", "R", "F");
        assert!(twice.contains("already built here") && !twice.contains("[E]"), "{twice}");
        use planet_build::{cannot_build_hint, CannotBuild};
        assert!(cannot_build_hint("Bed", CannotBuild::NotFirstPerson).contains("first person"));
        assert!(cannot_build_hint("Bed", CannotBuild::Driving).contains("vehicle"));
        assert!(cannot_build_hint("Bed", CannotBuild::OpenSpace).contains("open space"));
        assert!(cannot_build_hint("Bed", CannotBuild::NotOnGround).contains("stand on the ground"));
        assert!(cannot_build_hint("Bed", CannotBuild::OnWater).contains("water"));
        let short = short_hint("Wood Wall", "Wood Plank", 4);
        assert!(short.contains("carry 4 more Wood Plank") && short.contains("what you carry") && !short.contains("build here"), "{short}");
    }
}

#[cfg(test)]
mod collision_tests {
    use super::*;
    use crate::ship::wall_collision::{resolve, PLAYER_RADIUS};
    use glam::Quat;

    fn place(world: &mut hecs::World, reg: &BlueprintRegistry, id: &str, x: f32, z: f32, turns: u8) {
        let bp = reg.get(id).unwrap();
        let tf = placement::placement_pose(bp, Vec3::new(x, 0.0, z), turns, world, reg, None);
        world.spawn((tf, Structure { blueprint_id: id.into(), health: bp.health, max_health: bp.health, provides: bp.provides.clone(), uid: 0 }));
    }

    /// A BUILT WALL IS SOLID (2026-09-28). Walking east into a wall built
    /// across the way stops the player at the wall; a roof overhead does not
    /// block the way under it; a piece on a planet site is not in the home's
    /// list; and with no pieces the walk goes through. Red check, run:
    /// returning an empty list from `built_piece_segments` lets the walk pass
    /// through the wall and fails the first assertion.
    #[test]
    fn a_built_wall_stops_the_player_and_a_roof_overhead_does_not() {
        let reg = BlueprintRegistry::from_ron(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/blueprints/basic.ron"))).unwrap();
        let eye_h = 1.7;
        let start = Vec3::new(-2.0, eye_h, 0.0);
        let goal = Vec3::new(2.0, eye_h, 0.0);
        let walk = |world: &hecs::World| {
            let segs = built_piece_segments(world, None, None, start, eye_h);
            resolve(start, goal, PLAYER_RADIUS, &[], &segs)
        };
        // A wall running north-south at x = 0 (one quarter turn).
        let mut world = hecs::World::new();
        place(&mut world, &reg, "wood_wall", 0.0, 0.0, 1);
        let end = walk(&world);
        assert!(end.x < -PLAYER_RADIUS + 0.05, "stopped west of the wall, at {end}");

        // A roof alone, overhead: the way under it is open.
        let mut roofed = hecs::World::new();
        let roof = reg.get("roof").unwrap();
        roofed.spawn((
            Transform { position: Vec3::new(0.0, 3.0, 0.0), rotation: Quat::IDENTITY, scale: Vec3::from_array(roof.size) },
            Structure { blueprint_id: "roof".into(), health: 1.0, max_health: 1.0, provides: roof.provides.clone(), uid: 0 },
        ));
        assert!(built_piece_segments(&roofed, None, None, start, eye_h).is_empty(), "a roof overhead is not in the way");
        assert!((walk(&roofed).x - goal.x).abs() < 1e-4);

        // The same wall on a planet site is not a home piece.
        let mut site = hecs::World::new();
        let bp = reg.get("wood_wall").unwrap();
        let tf = placement::placement_pose(bp, Vec3::ZERO, 1, &site, &reg, None);
        site.spawn((
            tf,
            Structure { blueprint_id: "wood_wall".into(), health: 1.0, max_health: 1.0, provides: bp.provides.clone(), uid: 0 },
            PlanetSite { body: "earth".into(), origin: glam::DVec3::new(6.371e6, 0.0, 0.0) },
        ));
        assert!(built_piece_segments(&site, None, None, start, eye_h).is_empty(), "planet sites are not the home frame");
        assert!((walk(&hecs::World::new()).x - goal.x).abs() < 1e-4, "nothing built, nothing in the way");
    }

    /// A DOOR IN A WALL (2026-09-28). A shut door blocks the doorway, an open
    /// one lets the player through it, and the wall beside the gap blocks
    /// either way. Red check, run: collecting the whole piece's box instead
    /// of its parts fails the open-door walk.
    #[test]
    fn a_doorway_lets_you_through_only_when_the_door_is_open() {
        let reg = BlueprintRegistry::from_ron(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/blueprints/basic.ron"))).unwrap();
        assert!(reg.get("wood_wall_door").and_then(|b| b.doorway).is_some(), "the shipped doorway wall");
        let eye_h = 1.7;
        let mut world = hecs::World::new();
        // Running north-south at x = 0, the gap at z -0.5 .. 0.5.
        place(&mut world, &reg, "wood_wall_door", 0.0, 0.0, 1);
        let e = world.query::<&Structure>().iter().next().map(|(e, _)| e).unwrap();
        let walk = |world: &hecs::World, z: f32| {
            let (start, goal) = (Vec3::new(-2.0, eye_h, z), Vec3::new(2.0, eye_h, z));
            let segs = built_piece_segments(world, Some(&reg), None, start, eye_h);
            resolve(start, goal, PLAYER_RADIUS, &[], &segs)
        };
        assert!(walk(&world, 0.0).x < -0.25, "a shut door blocks the doorway");
        world.insert_one(e, DoorOpen).unwrap();
        assert!((walk(&world, 0.0).x - 2.0).abs() < 1e-3, "an open door lets you through: {}", walk(&world, 0.0));
        assert!(walk(&world, 1.5).x < -0.25, "the wall beside the gap still blocks");

        // A window is glass: walking at it stops you, like the wall. The wall
        // under the sill would stop you on its own, so the glass is checked
        // as a segment of its own: the two sides, the wall under the sill and
        // the pane (the lintel is over the head). Red check, run: leaving the
        // glass out of collision fails the count.
        let mut glazed = hecs::World::new();
        place(&mut glazed, &reg, "wood_wall_window", 0.0, 0.0, 1);
        assert!(walk(&glazed, 0.0).x < -0.25, "a window stops you");
        let segs = built_piece_segments(&glazed, Some(&reg), None, Vec3::new(-2.0, eye_h, 0.0), eye_h);
        assert_eq!(segs.len(), 4, "the sides, the wall under the sill and the pane");
    }

    /// The segment stays inside the box's footprint: a long thin box runs
    /// along its length, a square one is a point with its half width.
    #[test]
    fn a_box_becomes_a_segment_inside_its_footprint() {
        let s = segment_of_box(Vec3::new(-1.0, 0.0, -0.1), Vec3::new(1.0, 3.0, 0.1));
        assert_eq!((s.a, s.b, s.half_thickness), ((-0.9, 0.0), (0.9, 0.0), 0.1));
        let s = segment_of_box(Vec3::new(2.0, 0.0, 2.0), Vec3::new(2.8, 1.0, 2.8));
        assert!((s.a.0 - 2.4).abs() < 1e-6 && s.a == s.b && (s.half_thickness - 0.4).abs() < 1e-6);
        let s = segment_of_box(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.2, 3.0, 2.0));
        assert!((s.a.1 - 0.1).abs() < 1e-6 && (s.b.1 - 1.9).abs() < 1e-6 && (s.a.0 - 0.1).abs() < 1e-6);
    }
}

#[cfg(test)]
mod take_down_tests {
    use super::*;
    use crate::systems::construction::placement;

    fn place(world: &mut hecs::World, reg: &BlueprintRegistry, id: &str, x: f32, z: f32, turns: u8, uid: u32) -> hecs::Entity {
        let bp = reg.get(id).unwrap();
        let tf = placement::placement_pose(bp, Vec3::new(x, 0.0, z), turns, world, reg, None);
        world.spawn((tf, Structure { blueprint_id: id.into(), health: bp.health, max_health: bp.health, provides: bp.provides.clone(), uid }))
    }

    /// THE TAKE-DOWN ITSELF (2026-09-28): the piece is gone, every material
    /// is queued back to the pack as an add, and the line says what came
    /// back. Red check, run: leaving out the despawn fails the first
    /// assertion.
    #[test]
    fn a_take_down_removes_the_piece_and_queues_its_materials() {
        use crate::systems::inventory::TransferOp;
        let reg = BlueprintRegistry::from_ron(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/blueprints/basic.ron"))).unwrap();
        let mut world = hecs::World::new();
        let wall = place(&mut world, &reg, "wood_wall", 0.0, 0.0, 1, 1);
        let mut data = crate::hot_reload::data_store::DataStore::new();
        data.insert("inventory_transfer_ops", std::sync::Mutex::new(Vec::<TransferOp>::new()));
        let materials = vec![("wood_plank_0".to_string(), 6)];
        let line = apply_take_down(&mut world, &data, wall, "Wood Wall", &materials);
        assert!(!world.contains(wall), "the wall is gone");
        let ops = data.get::<std::sync::Mutex<Vec<TransferOp>>>("inventory_transfer_ops").unwrap().lock().unwrap().clone();
        assert_eq!(ops, vec![TransferOp { item_id: "wood_plank_0".into(), qty: 6, add: true, wear: 0, quality: 0 }]);
        assert_eq!(line, "Took down the Wood Wall: 6 wood_plank_0 back", "no item registry here: the id stands in for the name");
    }

    /// TAKE DOWN (2026-09-28). Looking at a wall within reach names it and
    /// gives back its materials; looking away, or at a wall past the reach,
    /// finds nothing; a chest that still holds something is refused until it
    /// is empty, and an empty one comes down. Red check, run: dropping the
    /// store check lets the full chest come down and fails the refusal.
    #[test]
    fn take_down_finds_the_piece_in_view_and_keeps_a_full_chest() {
        let reg = BlueprintRegistry::from_ron(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/blueprints/basic.ron"))).unwrap();
        let mut world = hecs::World::new();
        let wall = place(&mut world, &reg, "wood_wall", 0.0, 0.0, 1, 1);
        let eye = Vec3::new(-2.0, 1.5, 0.0);
        let (e, name, materials) = take_down_plan(&world, &[], Some(&reg), eye, Vec3::X, None, &[]).expect("a wall in view");
        assert_eq!((e, name.as_str()), (wall, "Wood Wall"));
        assert_eq!(materials, vec![("wood_plank_0".to_string(), 6)], "every material comes back");
        assert!(take_down_plan(&world, &[], Some(&reg), eye, Vec3::NEG_X, None, &[]).is_err(), "looking away");
        assert!(take_down_plan(&world, &[], Some(&reg), Vec3::new(-20.0, 1.5, 0.0), Vec3::X, None, &[]).is_err(), "out of reach");

        let mut stores = hecs::World::new();
        let chest = place(&mut stores, &reg, "storage_chest", 0.0, 0.0, 0, 7);
        let held = vec![crate::gui::PlacedItem {
            key: "wood_plank_0".into(),
            name: "Wood Plank".into(),
            qty: 3,
            container: crate::systems::construction::uses::storage_path(7),
            wear: 0,
            quality: 0,
        }];
        let at_chest = Vec3::new(0.0, 0.5, -2.0);
        let refused = take_down_plan(&stores, &held, Some(&reg), at_chest, Vec3::Z, None, &[]).unwrap_err();
        assert_eq!(refused, "Empty the Storage Chest before taking it down");
        let (e, _, _) = take_down_plan(&stores, &[], Some(&reg), at_chest, Vec3::Z, None, &[]).expect("an empty chest comes down");
        assert_eq!(e, chest);
    }
}
