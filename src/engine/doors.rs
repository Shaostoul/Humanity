//! The doors' easing, a step of the simulation (the headless sim runner's increment 3, 2026-11-15;
//! docs/design/headless-sim-runner.md, the inversions). Each door eases toward open or shut by who is
//! near it (the player, the other players, the dev crowd's walkers, the animals), its style, its locks,
//! a manual opening and a lift's landing; a door a hand opens swings away from whoever opens it and is
//! pushed ahead of anyone walking through. The walk collides against the open fraction this leaves
//! (built_uses.rs `shut_door_segments`), the next frame.
//!
//! Moved verbatim out of `home_meshes::render_door_panels`, which only draws the doors now: the
//! easing ran inside the home's draw list, so it ran only while the home was drawn and paused while
//! the showroom was open (a door half open stayed half open behind it). It runs every frame now,
//! from the frame's doors stage, right before the home's draw list, where it ran before; the draw
//! list reads the open fractions and the swings it leaves. Still on `EngineState` (the camera, the
//! controller's gait, the animation library's handle): increment 4 hands it plain values.

use crate::engine::frame_prelude::*;
use crate::engine::home_meshes::{door_locked_now, door_target};

/// Ease every door one step (the frame clock's `dt`) toward where its actors, locks and lift want it.
pub(crate) fn tick(state: &mut EngineState) {
    let dt = state.clock.dt;
    if state.door_panels.is_empty() {
        return;
    }
    // Auto-doors open for ANY nearby actor, not just the local player (v0.795, with the
    // corridor doors): a REMOTE player must be able to walk through a corridor mouth on your
    // screen (you SEE the door part for them, and its collider clears for you both), and a
    // wandering animal should not phase through a shut door's visual. Gathered once per frame:
    // the camera (the local player) + every RemotePlayer + every Creature transform. The
    // per-door check below takes the NEAREST actor, so one loiterer holds the door open.
    // v0.1005 (operator: "I was hearing doors open as my character moved
    // around the home despite my camera being in some far off distance"):
    // while the player is AWAY (surface-flying a planet, FTL), the local
    // camera coordinates are frozen at their last home-frame values - a
    // stale point usually INSIDE the house - so the "camera" kept counting
    // as a door actor and kept passing the earshot gate below. The camera
    // is only a door actor, and door sounds only reach the ears, while
    // actually aboard (the same gate the wall-collision system uses).
    // Kept apart by kind (ship homes increment 2 review, finding 8): a neighbour's corridor door
    // opens for the other players only (`door_panels::door_actor_distance`).
    // Each actor by its FEET (ship homes increment 7: a door opens only for a body on its storey,
    // `door_panels::door_actor_distance`): the camera and the other players are eyes (the camera's
    // own eye height; another player's position is the eye their game sends), an animal's position
    // is where it stands.
    let camera_actor = state.aboard_station.then_some(state.camera.position - Vec3::Y * state.controller.eye_height());
    let remote_eye = humanity_core::ecs::components::EYE_HEIGHT_M as f32;
    // With how fast each moves across the floor (rung 2b's follow-up: a sprinting player opens a
    // hinged door at its open_dist, a walking one by hand).
    let others_moving: Vec<(Vec3, f32)> = state
        .game_world
        .world
        .query::<(&humanity_core::ecs::components::Transform, &crate::net::sync::RemotePlayer)>()
        .iter()
        .map(|(_e, (t, r))| (t.position - Vec3::Y * remote_eye, Vec3::new(r.velocity.x, 0.0, r.velocity.z).length()))
        // The dev crowd's walkers open doors as they come to them, as another player does (crowd
        // navigation rung 1, engine/crowd_walk.rs): the mesh walks them through automatic doors.
        .chain(crate::engine::crowd_walk::door_actors(state))
        .collect();
    let others: Vec<Vec3> = others_moving.iter().map(|o| o.0).collect();
    let animals: Vec<Vec3> = state
        .game_world
        .world
        .query::<(&humanity_core::ecs::components::Transform, &humanity_core::ecs::components::Creature)>()
        .iter()
        .map(|(_e, (t, _))| t.position)
        .collect();
    // Snapshot the per-door manual-open flags (v0.567) so the loop can read them while it holds a
    // &mut on door_panels (a disjoint-field borrow the checker won't always see through).
    let manual = state.door_manual_open.clone();
    // Local SFX buffer: the panel loop holds &mut door_panels, so edge sounds
    // collect here and land on state.pending_sfx after the borrow ends.
    let mut sfx: Vec<(&'static str, &'static str)> = Vec::new();
    let state_cam = state.camera.position;
    let locks_live = state.door_locks.clone();
    // The elevators' landing doors (ship homes increment 7 review): one at a landing opens only
    // while the car is there (`door_target`). Only the doors marked when they were placed ask.
    let ship = state.gui_state.ship_structure.as_ref();
    let cars = &state.elevator_state;
    let landing_doors = &state.home_draw.landing_doors;
    // A HAND opens a hinged door (rung 2b's follow-up, the operator 2026-11-10: "Sure."): the local
    // player sprinting (Shift with a walk key) opens it at its open_dist as before; the handle's place
    // on the panel is the door anchor's row (data/characters/interactions.ron) when the bodies are
    // read, else the row's own numbers.
    let me_sprinting = state.controller.gait().1;
    let handle_local = crate::engine::interact::handle_local(state.animation.library().as_deref());
    let mut swings = std::mem::take(&mut state.door_swing);
    swings.resize(state.door_panels.len(), Default::default());
    for (di, (p, open)) in state.door_panels.iter_mut().enumerate() {
        // An operable DOOR opens on approach; a window or a "fixed"-styled opening stays shut
        // (v0.538: consult door_anim::is_operable so a door explicitly styled "fixed" does not
        // chase an open target it can never animate to).
        let operable = !p.is_window && humanity_systems::door_anim::is_operable(&p.style);
        // Is the door LOCKED right now (v0.570)? Live lock states if present, else the legacy bool.
        let locked_now = door_locked_now(p, locks_live.get(di));
        // Nearest actor's HORIZONTAL distance -- eye/body height must not count, or a tall
        // camera would never trigger a short door.
        let dist = humanity_ship::door_panels::door_actor_distance(p, camera_actor, &others, &animals);
        let landing = match ship {
            Some(ship) if landing_doors.get(di).copied().unwrap_or(false) => {
                crate::engine::footing::lift_landing_open(ship, cars, p.center)
            }
            _ => None,
        };
        // A door a hand opens (`door_anim::HandOpen`, its style's row): walking people open it by
        // hand or body, fast ones and animals at its open_dist; a locked one, a manual one and a
        // landing door with no car stay `door_target`'s.
        let hand = humanity_systems::door_anim::style_hand(&p.style)
            .filter(|_| operable && p.auto_open && !p.others_only && !locked_now && landing != Some(false));
        let (target, opener) = match hand {
            Some(h) => {
                let near_hand = hand_door_actors(p, camera_actor, me_sprinting, &others_moving, &animals, &h, handle_local);
                let t = humanity_systems::door_anim::hand_door_target(dist, near_hand.hand.0, near_hand.touch.0, near_hand.fast.0, p.open_dist, &h, *open);
                (t, near_hand.opener(&h))
            }
            None => (door_target(p, *open, locked_now, manual.get(di).copied().unwrap_or(false), landing, dist), None),
        };
        // Starting to open from shut: it swings away from whoever opens it (a hand door), and at the
        // push's pace when a hand did.
        if *open <= 0.02 && target > 0.5 {
            if let (Some(h), Some((at, by_hand))) = (hand, opener) {
                swings[di] = humanity_systems::door_anim::Swing { side: humanity_systems::door_anim::swing_away_from(p.hinge, p.center, at), by_hand };
                let _ = h;
            } else {
                swings[di] = Default::default();
            }
        }
        // Frame-rate-independent exponential ease toward the target: smooth open/close, ~0.4 s
        // to settle, no snapping (v0.540, pure + tested in door_anim since v0.795); a door a hand is
        // opening at the push's pace.
        let open_before = *open;
        *open = match hand {
            Some(h) if swings[di].by_hand && target > 0.5 => humanity_systems::door_anim::ease_open_at(*open, target, dt, h.swing_rate),
            _ => humanity_systems::door_anim::ease_open(*open, target, dt),
        };
        // And pushed ahead of anyone walking through it: never so little open that the panel passes
        // through a body near it (a door a hand opened swings at the push's pace, slower than a walk
        // through it near the hinge).
        if hand.is_some() && swings[di].by_hand && target > 0.5 {
            for a in camera_actor.into_iter().chain(others.iter().copied()) {
                if (a - p.hinge).length_squared() < 2.25 && humanity_ship::door_panels::feet_reach_door(a.y, p.center.y, p.center.y + p.size.y) {
                    *open = open.max(humanity_systems::door_anim::push_open(p.hinge, p.center, swings[di].side, a, PUSH_CLEAR_M));
                }
            }
        }
        // Door SFX (v0.983): fire on the swing's START edges - rising off
        // fully-closed plays the open sound, dropping off fully-open plays
        // the close. Mid-swing reversals stay quiet (no re-trigger spam when
        // a player hovers at an auto-door's radius), and doors beyond
        // earshot stay silent (a REMOTE actor can open doors far from you;
        // play_sound is non-spatial, so gate by distance until the spatial
        // path is wired).
        let within_earshot = state.aboard_station
            && (p.center - state_cam).length_squared() < 25.0 * 25.0;
        if within_earshot {
            if open_before <= 0.02 && *open > 0.02 {
                sfx.push(("sfx.door_open", "audio/sfx/door_open.ogg"));
            } else if open_before >= 0.98 && *open < 0.98 {
                sfx.push(("sfx.door_close", "audio/sfx/door_close.ogg"));
            }
        }
    }
    state.door_swing = swings;
    state.pending_sfx.extend(sfx);
}

/// How far a door a hand opened keeps its panel from a body walking through (metres, across the
/// floor): about the drawn body's half width at the shoulders and hips, inside the walk's 0.3 m.
const PUSH_CLEAR_M: f32 = 0.28;

/// Who is at a hand-opened door this frame (rung 2b's follow-up): the nearest walking person's feet
/// to its handle on their side and to its face, and the nearest fast actor (a sprinting person, an
/// animal) to its middle, each with where they stand, all across the floor and only those on the
/// door's storey (`door_panels::feet_reach_door`).
struct HandDoorActors {
    hand: (f32, Vec3),
    touch: (f32, Vec3),
    fast: (f32, Vec3),
}

impl HandDoorActors {
    /// Who opens it now, if anyone does, and whether by hand (a hand at the handle or a body at the
    /// face) or as a door opens for something fast.
    fn opener(&self, h: &humanity_systems::door_anim::HandOpen) -> Option<(Vec3, bool)> {
        if self.hand.0 <= h.reach_m {
            Some((self.hand.1, true))
        } else if self.touch.0 <= h.touch_m {
            Some((self.touch.1, true))
        } else if self.fast.0 < f32::MAX {
            Some((self.fast.1, false))
        } else {
            None
        }
    }
}

/// `HandDoorActors` for door `p`: the camera (the local player, sprinting or not), the other
/// players with their speed across the floor (faster than the row's `sprint_mps` is a sprint) and
/// the animals; `handle` is the handle's place on the panel, (along from its free edge, out from its
/// face), as `interact::handle_local` reads it.
fn hand_door_actors(
    p: &humanity_ship::door_panels::PanelPlacement,
    camera: Option<Vec3>,
    me_sprinting: bool,
    others: &[(Vec3, f32)],
    animals: &[Vec3],
    h: &humanity_systems::door_anim::HandOpen,
    handle: (f32, f32),
) -> HandDoorActors {
    let (sill, head) = (p.center.y, p.center.y + p.size.y);
    let on_storey = |a: &Vec3| humanity_ship::door_panels::feet_reach_door(a.y, sill, head);
    let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z).length();
    let normal = p.rotation * Vec3::Z;
    let along = p.rotation * Vec3::X;
    let mut out = HandDoorActors { hand: (f32::MAX, Vec3::ZERO), touch: (f32::MAX, Vec3::ZERO), fast: (f32::MAX, Vec3::ZERO) };
    let walkers = camera.filter(|_| !me_sprinting).into_iter().chain(others.iter().filter(|o| o.1 <= h.sprint_mps).map(|o| o.0));
    for a in walkers.filter(on_storey) {
        let d = a - p.center;
        let side = if d.dot(normal) >= 0.0 { 1.0 } else { -1.0 };
        let at = p.center + p.rotation * Vec3::new(p.size.x * 0.5 + handle.0, 0.0, side * (p.size.z * 0.5 + handle.1));
        let to_handle = flat(a - at);
        if to_handle < out.hand.0 {
            out.hand = (to_handle, a);
        }
        // The face, anywhere across the door's width (and the walk's radius beyond each side).
        if d.dot(along).abs() <= p.size.x * 0.5 + humanity_ship::wall_collision::PLAYER_RADIUS {
            let to_face = d.dot(normal).abs();
            if to_face < out.touch.0 {
                out.touch = (to_face, a);
            }
        }
    }
    let fast = camera.filter(|_| me_sprinting).into_iter().chain(others.iter().filter(|o| o.1 > h.sprint_mps).map(|o| o.0)).chain(animals.iter().copied());
    for a in fast.filter(on_storey) {
        let d = flat(a - p.center);
        if d < out.fast.0 {
            out.fast = (d, a);
        }
    }
    out
}
