//! The frame's mouse input (the file-size split, step 1, 2026-11-14): the window's mouse button
//! and wheel events, moved out of `window_event` in src/lib.rs as they were. Picking, grabbing
//! and placing in build mode start here; the camera's look comes from `device_event`.

use crate::engine::frame_prelude::*;

/// A mouse button pressed or released: the window's `MouseInput` event. `egui_consumed` is true
/// when the GUI took the event.
pub(crate) fn button(state: &mut EngineState, button: winit::event::MouseButton, btn_state: winit::event::ElementState, egui_consumed: bool) {
    use winit::event::{ElementState, MouseButton};
    let left = button == MouseButton::Left;
    let right = button == MouseButton::Right;
    let pressed = btn_state == ElementState::Pressed;
    // A CLICK in the window is the one honest signal that the
    // operator deliberately entered a HUMANITY_NO_FOCUS
    // instance (v0.1072). This used to clear on Focused(true),
    // but Windows can deliver a focus event during creation
    // without any user action, which disarmed the guard before
    // the boot-frame settings_dirty ran apply_window_mode, and
    // the maximize there activated the window: every rig boot
    // still stole focus a second in, measured at t+1s.
    if pressed && state.background_no_cursor {
        state.background_no_cursor = false;
        crate::engine::launch_focus::open_audio_on_click(&mut state.audio);
        // ...and the saved relay node a script launch held back.
        crate::gui::pages::host_node::autostart_on_first_click(&state.gui_state);
    }
    // In-world modal panels own clicks (v0.773 chat, v0.778 creature
    // editor). egui still receives the click (on_window_event, above)
    // so the panel's buttons work; but the game must NOT also swing a
    // tool / place a machine / interact while a panel is up. ONLY
    // PRESSES are swallowed (v0.779): releases fall through so
    // lmb_held, the controller's button state, and the construction
    // release-cleanup all clear -- swallowing releases left a
    // build-mode drag chasing the freed cursor (the stuck-grab class
    // v0.593 fixed).
    if state.gui_state.in_world_modal_open() && pressed {
        return;
    }
    if left {
        state.lmb_held = pressed; // undo drag-coalescing (v0.575)
    }
    // Construction astral editor: LEFT grabs/drops a room (left is a no-op in the
    // orbit cam, so we own it). Gated on !egui_consumed so panel clicks never
    // start a grab. (v0.466)
    if state.gui_state.construction_active && left && !egui_consumed {
        // Per-type LOCK (v0.614): a locked type can't be picked/grabbed in the viewport.
        // Precomputed as bools (no borrow held across the mutable try_pick_* calls below).
        // A type that's LOCKED or HIDDEN (v0.636) can't be picked/grabbed in the viewport.
        let locked = &state.gui_state.construction_locked_types;
        let hidden = &state.gui_state.construction_hidden_types;
        let blocked = |t: &str| locked.contains(t) || hidden.contains(t);
        let (lock_wall, lock_struct, lock_machine, lock_light, lock_road, lock_pipe, hide_zone) = (
            blocked("Wall"),
            blocked("Struct"),
            blocked("Machine"),
            blocked("Light"),
            blocked("Road"),
            blocked("Pipe"),
            blocked("Zone"),
        );
        if pressed {
            // Any viewport press re-decides the selection, so drop a stale PIPE / ZONE
            // selection up front (v0.626/v0.634); the try_pick_* below re-set them on a hit.
            // GUI clicks (egui_consumed) never reach here, so the right-panel Remove is safe.
            state.gui_state.construction_connection_selected = None;
            state.gui_state.construction_zone_selected = None;
            // Wall-drawing mode owns the click first (v0.534): drop a corner node.
            // Else holding a palette item -> drop it; else grab a room.
            if state.gui_state.construction_place_conduit_node {
                // Place-a-conduit-node mode (v0.629): drop a pipe-graph junction at the
                // cursor floor point -- a "main line" node you then drag ports onto. The
                // tool stays held (place several); right-click cancels.
                if let Some((_, hx, hz)) = cursor_floor_hit(state) {
                    if let Some(home) = state.gui_state.home_machines.as_mut() {
                        home.add_conduit_node((hx, 0.35, hz), "power");
                    }
                    state.gui_state.construction_machines_dirty = true;
                }
            } else if state.gui_state.construction_wall_mode {
                try_place_wall_node(state);
            } else if state.gui_state.construction_place_type.is_some() {
                try_place_held_machine(state);
            } else if state.gui_state.construction_place_light.is_some() {
                // Holding a LIGHT from the palette (v0.784) -> drop it at the
                // cursor; ceiling types hang under the zone ceiling, lamps sit
                // low. The tool stays held; right-click cancels.
                try_place_held_light(state);
            } else if state.gui_state.construction_structure_type.is_some() {
                // Holding a STRUCTURAL piece (v0.583) -> drop it on the floor.
                try_place_structure(state);
            } else if state.gui_state.ship_structure.is_some() && try_grab_opening_resize(state) {
                // Grabbed an opening RESIZE handle (v0.578): the per-frame drag resizes
                // the door/window (width via left/right, height via top/bottom).
            } else if state.gui_state.ship_structure.is_some() && try_grab_opening(state) {
                // Grabbed a door/window opening gizmo (v0.546): the per-frame drag
                // slides it along its wall.
            } else if state.gui_state.ship_structure.is_some() && try_grab_node(state) {
                // Grabbed a wall corner-node gizmo (v0.541): the per-frame drag moves
                // it (+ any walls sharing it) with snapping.
            } else if try_grab_char(state) {
                // Grabbed the build-mode avatar (v0.557): drag it across the floor; you
                // spawn right there when you leave build mode.
            } else if (!lock_road || !lock_pipe) && try_pick_node(state) {
                // Clicked a ROAD / CONDUIT node orb (v0.599): select + drag it.
            } else if !lock_light && state.gui_state.ship_structure.is_some() && try_pick_strip_point(state) {
                // Grabbed a STRIP control-point handle (v0.790): drag steers that
                // path point across the floor. Before try_pick_light so the small
                // handles beat the body diamond.
            } else if !lock_light && state.gui_state.ship_structure.is_some() && try_pick_light_ring(state) {
                // Grabbed a light ROTATION ring (v0.790, viewport-fixed v0.792):
                // sweeping the cursor around the ring rotates the aim about that
                // ring's axis.
            } else if state.gui_state.ship_structure.is_some() && try_pick_corridor_mouth(state) {
                // Grabbed a corridor DOOR-MOUTH handle (v0.790): drag slides the
                // corridor along the wall (its world `lat`).
            } else if !lock_light && state.gui_state.ship_structure.is_some() && try_pick_light(state) {
                // Clicked a placed-LIGHT diamond gizmo (v0.576) -> its detail shows on
                // the right panel, like a wall.
            } else if !lock_struct && state.gui_state.ship_structure.is_some() && try_pick_structure(state) {
                // Clicked a placed STRUCTURE (v0.583) -> its detail shows on the right.
            } else if !lock_machine && crate::engine::screens::edit::try_pick(state) {
                // An in-world screen's picture (2026-11-06): its cog opens its
                // settings panel, an edge or a corner resizes it, the middle moves
                // it along its wall. Before the machine pick, whose body it is.
            } else if !lock_machine && state.gui_state.ship_structure.is_some() && try_pick_port(state) {
                // Grabbed a machine PORT (v0.625): drag onto another machine to WIRE
                // them. Runs before the machine pick so a port beats moving the body.
            } else if !lock_machine && state.gui_state.ship_structure.is_some() && try_pick_machine(state) {
                // Selected a machine in the viewport (v0.553) -> its detail shows on
                // the right panel. Click only; machines are not dragged here. Gated to
                // the box-home path so it never shadows the legacy room-grab.
            } else if state.gui_state.ship_structure.is_some() && try_pick_connection(state) {
                // Clicked a PIPE/WIRE (v0.626): select that connection -> its detail +
                // a Remove button show on the right panel. After machines (so a machine
                // under the cursor wins) but before walls (pipes are in the foreground).
            } else if state.gui_state.ship_structure.is_some() && try_pick_slab(state) {
                // Clicked a FLOOR SLAB's outline (ship homes increment 7): select it;
                // its detail shows on the right and its corner handles appear.
            } else if !lock_wall && state.gui_state.ship_structure.is_some() && try_pick_wall(state) {
                // Clicked a WALL SURFACE (v0.573): select that wall for editing --
                // unambiguous vs hunting for the right corner orb at an intersection.
            } else if !hide_zone && state.gui_state.ship_structure.is_some() && try_pick_zone(state) {
                // Clicked a ZONE box (v0.634): select + drag it. LAST before the room grab
                // so the big macro volumes never steal a click from anything in front.
            } else {
                try_begin_room_grab(state);
            }
        } else {
            // Release. A gizmo grab that never crossed the drag threshold (grab_press
            // still set) is a TAP -> SELECT its wall + show it on the right panel,
            // instead of moving it (v0.549). Click to inspect, click-and-hold to move.
            if state.construction_grab_press.is_some() {
                if let Some(c) = state.construction_node_grab {
                    // A wall of the storey being edited (ship homes increment 7).
                    let chosen = state.gui_state.construction_storeys.chosen;
                    let sel = zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone)
                        .and_then(|hs| storey_wall_at_corner(hs, EditStorey::of(hs, chosen).index, c));
                    if let Some(i) = sel {
                        state.gui_state.construction_wall_selected = Some(i);
                        state.gui_state.construction_machine_selected = None;
                    }
                } else if let Some((wi, _)) = state.construction_opening_grab {
                    state.gui_state.construction_wall_selected = Some(wi);
                    state.gui_state.construction_machine_selected = None;
                }
            }
            // Port DRAG-TO-CONNECT release (v0.625): if we were dragging a machine port,
            // drop it on a machine that has a compatible (same-utility) port to wire them.
            // Orient supply->demand so `from` is the source (the data convention).
            if let Some((src_id, util, pdir, _)) = state.construction_port_drag.take() {
                if let Some(node_id) = port_drop_node_target(state) {
                    // Dropped on a CONDUIT NODE (v0.629): branch this machine onto the
                    // main-line node (a graph edge machine -> node, routed as a real pipe).
                    if let Some(home) = state.gui_state.home_machines.as_mut() {
                        use humanity_ship::machines::ConduitEnd;
                        if home.add_conduit_edge(ConduitEnd::Machine(src_id.clone()), ConduitEnd::Node(node_id), util.id()) {
                            state.gui_state.construction_machines_dirty = true;
                        }
                    }
                } else if let Some(target) = port_drop_target(state, &src_id) {
                    if machine_has_port(state, &target, util) {
                        if let Some(home) = state.gui_state.home_machines.as_mut() {
                            let (from, to) = match pdir {
                                humanity_core::utilities::PortDir::In => (target.clone(), src_id.clone()),
                                _ => (src_id.clone(), target.clone()),
                            };
                            if home.add_connection(&from, &to, util.id()) {
                                state.gui_state.construction_machines_dirty = true;
                            }
                        }
                    }
                }
            }
            state.construction_grab = None; // release; keep the selection highlighted
            state.construction_gizmo_grab = None; // release a slid handle too
            state.construction_node_grab = None; // release a dragged corner node
            // A screen's handle let go (2026-11-06): one more rebuild, which makes a
            // stream's or a clip's surface anew at the screen's final shape (it is
            // kept through the drag, `screens::sync_screens`).
            if matches!(state.construction_object_grab, Some(ObjectGrab::Screen(_))) {
                state.gui_state.construction_machines_dirty = true;
            }
            state.construction_object_grab = None; // release a dragged object (v0.593)
            state.construction_opening_grab = None; // release a dragged opening
            state.construction_opening_resize = None; // release a resize handle (v0.578)
            state.construction_char_grab = false; // release a dragged avatar (v0.557)
            state.construction_grab_press = None;
        }
    } else if state.gui_state.construction_active
        && right
        && pressed
        && (state.gui_state.construction_place_type.is_some()
            || state.gui_state.construction_structure_type.is_some()
            || state.gui_state.construction_place_light.is_some()
            || state.gui_state.construction_place_conduit_node
            || state.gui_state.construction_wall_mode)
    {
        // Right-click cancels the held placement item / structure / light /
        // conduit-node tool OR exits wall-drawing and clears the pending
        // corner (v0.529/v0.534/v0.583/v0.629/v0.784).
        state.gui_state.construction_place_type = None;
        state.gui_state.construction_structure_type = None;
        state.gui_state.construction_place_light = None;
        state.gui_state.construction_place_conduit_node = false;
        state.gui_state.construction_wall_mode = false;
        state.gui_state.construction_wall_start = None;
    } else if !egui_consumed && state.gui_state.active_page == GuiPage::None {
        // In-world screens (rung 1): a primary press or release
        // while the look ray (or the free cursor) rests on a
        // screen goes to that screen's page instead of the
        // game. Only the PRESS is withheld from the controller
        // (the same rule as the in-world modals, v0.779): the
        // release still reaches it so a held button state can
        // never stick. `screens.hover` is the hit computed by
        // screens::update this frame, so the click lands on
        // exactly what the player is looking at.
        let screen_took_it = left && state.screens.route_button(pressed);
        if !(screen_took_it && pressed) {
            state.controller.process_mouse_button(button, btn_state);
        }
    }
}

/// The mouse wheel: the window's `MouseWheel` event.
pub(crate) fn wheel(state: &mut EngineState, delta: winit::event::MouseScrollDelta, egui_consumed: bool) {
    // Wheel joins the modal gates (v0.779): scrolling the chat
    // history / editor must not also zoom the game camera.
    if !egui_consumed
        && state.gui_state.active_page == GuiPage::None
        && !state.gui_state.in_world_modal_open()
    {
        let scroll = match delta {
            winit::event::MouseScrollDelta::LineDelta(_, y) => y,
            winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 100.0,
        };
        // In-world screens (rung 1): a notch over a screen
        // scrolls that screen's page and nothing else. The
        // sign is winit's line delta, which is also egui's
        // wheel convention, so it passes through untouched.
        if state.screens.route_scroll(scroll) {
            return;
        }
        // Dev FTL speed on the wheel (v0.791.x): while fly mode
        // is on in first person the wheel is otherwise unused
        // (FP ignores scroll_delta), so it steps the fly speed
        // multiplier one decade per notch, clamped 1x..1e9x.
        // dev_fly_mode is forced off when dev tooling is off
        // (Dev play mode + cheats switch, task #50), so this
        // path is inert for normal players.
        if state.gui_state.dev_fly_mode
            && state.camera.mode == crate::renderer::camera::CameraMode::FirstPerson
        {
            let exp = (state.gui_state.dev_fly_speed_mult.max(1.0).log10()
                + scroll)
                .clamp(0.0, 9.0);
            state.gui_state.dev_fly_speed_mult = 10f32.powf(exp);
        } else {
            state.controller.process_scroll(scroll);
        }
    }
}
