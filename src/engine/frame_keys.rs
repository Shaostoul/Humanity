//! The frame's keyboard input (the file-size split, step 1, 2026-11-14): the window's
//! `KeyboardInput` event, moved out of `window_event` in src/lib.rs as it was. In-game keys,
//! the menus' keys and build mode's keys are read here; the held movement keys feed
//! `InputState`, which the frame's movement stages read.

use crate::engine::frame_prelude::*;

/// One key pressed or released. `egui_consumed` is true when the GUI took the key (Tab in game
/// never reaches the GUI, see `window_event`).
pub(crate) fn run(state: &mut EngineState, event: winit::event::KeyEvent, egui_consumed: bool) {
    if let PhysicalKey::Code(key) = event.physical_key {
        let pressed = event.state.is_pressed();

        // Rebindable controls (2026-08-12): gameplay keys are
        // identified by their KeyCode debug name ("KeyW",
        // "Space", "ShiftLeft" -- the same convention the
        // voice push key established in v0.490) and resolved
        // through GuiState::keybinds, so Settings > Controls
        // rebinds take effect the same frame. Resolved once
        // per event here. Fixed shortcuts (Esc, the F row,
        // build-mode Ctrl combos, Ctrl+V paste) still compare
        // raw KeyCodes below by design -- see
        // input::bindings::FIXED_BINDS.
        let key_name = format!("{:?}", key);
        use humanity_core::input::bindings::GameAction as GA;

        // Track Ctrl/Cmd modifier state from raw winit input.
        // (egui-winit doesn't expose this in a way we can
        // read for our pre-egui Ctrl+V detection.)
        if matches!(key, KeyCode::ControlLeft | KeyCode::ControlRight
            | KeyCode::SuperLeft | KeyCode::SuperRight)
        {
            state.ctrl_held = pressed;
        }
        if matches!(key, KeyCode::ShiftLeft | KeyCode::ShiftRight) {
            state.shift_held = pressed; // for Ctrl+Shift+Z redo (v0.575)
        }
        if matches!(key, KeyCode::AltLeft | KeyCode::AltRight) {
            // Hold-Alt free-look cursor (v0.735) — reconcile_cursor
            // reads this every frame.
            state.alt_held = pressed;
        }

        // Keybind CAPTURE (Settings > Controls, 2026-08-12):
        // while a bind cell is armed, the next key press
        // becomes that bind (Esc cancels, Delete clears,
        // reserved keys refuse, taken keys raise a confirm).
        // Full flow + tests: engine::keybind_capture.
        if crate::engine::keybind_capture::handle_keybind_capture(
            &mut state.gui_state,
            &key_name,
            pressed,
        ) {
            return;
        }

        // Escape closes the F10 Cloud dev sidebar FIRST, and does
        // nothing else that frame (operator, 2026-09-18: "when I
        // press esc it closes the F10 menu only instead of also
        // opening up the menu, but only while the F10 menu is
        // up"). It is first in the Escape ordering because the
        // topmost thing on screen is what Escape means: an
        // expanded sidebar is the overlay the operator opened
        // most recently and the one under their freed cursor.
        // Ahead of the modal and screen gates below on purpose:
        // the sidebar never takes the keyboard, so closing it
        // interrupts no typing, and the next press still reaches
        // whichever of those is open. Only an EXPANDED sidebar
        // returns true here; closed or collapsed, Escape keeps
        // its usual meaning (help panel, then the menu cascade).
        // The cursor re-grabs through reconcile_cursor, which
        // reads the flag this cleared (single authority, v0.460).
        // A HELD Escape delivers a second press about half a
        // second later (winit auto-repeat). No Escape rule below
        // wants that: after this press closed the sidebar the
        // repeat would open the menu, and a held Escape on the
        // menu would toggle it open and shut. Only the first
        // press of a hold counts (review finding, 2026-09-18).
        if key == KeyCode::Escape && pressed && event.repeat {
            return;
        }
        if key == KeyCode::Escape
            && pressed
            && crate::engine::input::escape_closes_cloud_dev(&mut state.gui_state)
        {
            return;
        }

        // In-world modal panels own the keyboard (v0.773 chat,
        // v0.778 creature editor). egui already received this event
        // above (on_window_event), so text still types into the
        // panel's fields; here we must run NO gameplay key handler
        // (inventory 'i', the 1-9 hotbar, tool/build keys, the
        // menu-Esc) -- that was the "typing 'i' opens the inventory"
        // bug. Esc closes whichever panel is open so the global
        // menu-Esc below never fires. ONLY PRESSES are swallowed
        // (v0.779): key RELEASES fall through so the held-state
        // trackers below (voice push-to-talk, Tab reveal, F1
        // keymap, movement keys) clear correctly -- swallowing
        // releases left the mic stuck TRANSMITTING when the PTT
        // key was released while a panel was open. No action
        // handler below fires on a release (all check `pressed`).
        if state.gui_state.in_world_modal_open() {
            if key == KeyCode::Escape && pressed {
                state.gui_state.close_in_world_modals();
            }
            // Repeat-E cycles the open NPC dialogue card's line
            // (v0.797) -- the "press E to keep talking" flow. Only
            // when the talk card is the open modal: in the chat
            // panel a typed 'e' is just a letter, and the creature
            // editor has a Name text field.
            if state.gui_state.keybinds.is(GA::Interact, &key_name)
                && pressed
                && state.gui_state.npc_talk_target.is_some()
                && !state.gui_state.chat_input_active
                && state.gui_state.dev_edit_target.is_none()
            {
                state.gui_state.npc_talk_advance();
            }
            if pressed {
                return;
            }
        }

        // In-world screens (rung 1): while the last-clicked
        // screen's page has a text field focused, keys and
        // typed text go to THAT page and no gameplay key
        // handler runs (typing "i" into a wall screen's search
        // box must not open the inventory, the same class of
        // bug the modal gate above fixed). Escape releases the
        // screen's focus instead of opening the menu. Presses
        // only, as above: releases fall through so the held-key
        // trackers clear. The modifiers come from the raw
        // trackers this handler already keeps.
        if state.gui_state.active_page == GuiPage::None
            && (state.screens.keyboard_captured() || (key == KeyCode::Escape && state.screens.focused.is_some()))
        {
            let mods = egui::Modifiers {
                alt: state.alt_held,
                ctrl: state.ctrl_held,
                shift: state.shift_held,
                mac_cmd: false,
                command: state.ctrl_held,
            };
            let typed = event.text.as_ref().map(|t| t.as_str());
            if state.screens.route_key(&key_name, pressed, typed, mods) && pressed {
                return;
            }
        }

        // A built piece in hand (engine/build_place.rs): E builds, R turns, Esc stops.
        if pressed && crate::engine::build_place::key(state, &key_name, key == KeyCode::Escape, event.repeat) {
            return;
        }
        // Undo/redo in the construction editor (v0.575): Ctrl+Z undo, Ctrl+Shift+Z (or
        // Ctrl+Y) redo. Gated to build mode so it never fights the chat Ctrl+V path.
        if pressed && state.ctrl_held && state.gui_state.construction_active {
            if key == KeyCode::KeyZ && state.shift_held {
                construction_redo(state);
            } else if key == KeyCode::KeyZ {
                construction_undo(state);
            } else if key == KeyCode::KeyY {
                construction_redo(state);
            } else if key == KeyCode::KeyD {
                construction_duplicate(state); // v0.600: stamp a copy of the selection
            }
        }
        // Rotate the held STRUCTURE piece with [ and ] (v0.583): 15-degree steps. Only
        // while a structure is held for placement, so it never fights other keys.
        if pressed
            && state.gui_state.construction_active
            && state.gui_state.construction_structure_type.is_some()
        {
            if key == KeyCode::BracketLeft {
                state.gui_state.construction_structure_yaw =
                    (state.gui_state.construction_structure_yaw - 15.0).rem_euclid(360.0);
            } else if key == KeyCode::BracketRight {
                state.gui_state.construction_structure_yaw =
                    (state.gui_state.construction_structure_yaw + 15.0).rem_euclid(360.0);
            }
        }

        // F1 (hold) shows the keymap for the current screen/mode. Works on every
        // page so you can always see what keys do something here. (v0.465)
        if key == KeyCode::F1 {
            state.gui_state.keymap_visible = pressed;
            if pressed && state.gui_state.keymaps.is_empty() {
                state.gui_state.keymaps =
                    crate::gui::pages::keymap::load_keymaps(&state.data_dir);
            }
            return;
        }
        // Diagnostics dev-HUD overlays (v0.482): F2 perf, F3 network,
        // F4 system. Toggle on press; stack in the top-right corner.
        if key == KeyCode::F2 && pressed {
            state.gui_state.show_perf_overlay = !state.gui_state.show_perf_overlay;
            return;
        }
        if key == KeyCode::F3 && pressed {
            state.gui_state.show_network_overlay = !state.gui_state.show_network_overlay;
            return;
        }
        if key == KeyCode::F4 && pressed {
            state.gui_state.show_system_overlay = !state.gui_state.show_system_overlay;
            return;
        }
        // F11 (v0.1050, operator request): the live WEATHER
        // panel. Presets from glassy calm to hurricane, plus
        // condition buttons and wind/intensity sliders. Wind is
        // what the ocean spectrum is built from, so this is
        // also the sea-state control.
        if key == KeyCode::F11 && pressed {
            state.gui_state.show_weather_panel =
                !state.gui_state.show_weather_panel;
            return;
        }
        // F10 (v0.1254.4, operator request): the cloud dev
        // panel - GUI buttons for the bisect toggles that
        // previously required showcase file drops.
        // 2026-09-05 (operator: "Pressing F10 should free the
        // cursor and bring up the menu so I don't have to hold
        // alt"): the panel is a left sidebar with a collapse
        // tab. F10 on an EXPANDED sidebar closes it (the cursor
        // re-grabs through reconcile_cursor, same as before it
        // opened); F10 on a closed OR collapsed sidebar opens
        // it expanded, so the key always works even when the
        // slim tab cannot be clicked because the cursor is
        // grabbed. The cursor itself is freed by
        // reconcile_cursor reading cloud_dev_sidebar_expanded,
        // never here (the single-authority rule, v0.460).
        // The rule itself is engine::input::toggle_cloud_dev_panel
        // (2026-09-18): the same function the dev IPC's "f10"
        // verb calls, so the key and the rig cannot diverge.
        if key == KeyCode::F10 && pressed {
            crate::engine::input::toggle_cloud_dev_panel(&mut state.gui_state);
            return;
        }
        // F6 (v0.890): save a LOCATION BOOKMARK - the exact
        // camera placement + aim, appended to
        // debug/bookmarks.json. Restored by name via
        // debug/camera_request.json {"bookmark":"bm-N"}, so a
        // screenshot teleport lands exactly where the operator
        // stood and looked, instead of staring into the dirt.
        if key == KeyCode::F6 && pressed {
            state.bookmark_save_requested = true;
            return;
        }
        // F8 (2026-10-24): Mark this moment for the play journal, on any page: a
        // numbered mark with this frame's picture, taken at the end of the frame
        // (engine/play_journal_frame.rs `take_mark`, docs/design/play-journal.md).
        if key == KeyCode::F8 && pressed {
            state.gui_state.play_mark_request = Some(humanity_core::play_journal::MarkAsk::by_hand());
            return;
        }
        // F9 (v0.1109, operator request): toggle DEV FLIGHT
        // without leaving the world for the Dev page. OFF is
        // the normal walk/run model (gravity + ground clamp);
        // ON removes both and HOLDS the altitude when nothing
        // is held, so you can park at "5,000 feet or whatever"
        // for a scenery shot and press F9 again to fall
        // normally. Same flag the Dev page checkbox and the
        // HUD's FLY indicator use - there is no second mode.
        // Cheats-gated like every other dev-travel affordance
        // (the per-frame sync at the top of the loop would
        // force it back off anyway; refusing here says why).
        if key == KeyCode::F9 && pressed {
            if state.gui_state.dev_cheats_active(&state.theme) {
                let on = !state.gui_state.dev_hover;
                state.gui_state.dev_hover = on;
                state.gui_state.dev_fly_mode = on;
                state.controller.fly_mode = on; // effective this frame
                // Leaving flight starts a FRESH fall rather
                // than inheriting whatever the last airborne
                // moment left behind.
                state.surface_vr = 0.0;
                log::info!(
                    "[DevFlight] F9: {}",
                    if on { "ON (no gravity, altitude held)" } else { "OFF (walking)" }
                );
            } else {
                log::info!(
                    "[DevFlight] F9 ignored: dev tools are off (Settings > \
                                     Gameplay > Play mode, plus the cheats switch)"
                );
            }
            return;
        }

        // Voice push key (v0.490): tracked from RAW winit input so
        // it works in-game (where egui isn't focused) AND supports
        // keys egui lacks, like CapsLock (the default push key).
        // The stored name is the KeyCode debug string (e.g.
        // "CapsLock", "KeyV"). Also captures a new binding when the
        // Settings UI is waiting for one.
        {
            let name = key_name.clone();
            if state.gui_state.voice_binding_key {
                if pressed {
                    if key != KeyCode::Escape {
                        // Escape cancels; any other key binds.
                        state.gui_state.voice_ptt_key = name;
                        state.gui_state.settings_dirty = true;
                    }
                    state.gui_state.voice_binding_key = false;
                    return;
                }
            } else if name == state.gui_state.voice_ptt_key {
                state.gui_state.voice_ptt_held = pressed;
            }
        }

        // Ctrl+V clipboard image paste — detected HERE at the
        // raw winit layer because egui-winit intercepts the
        // paste shortcut, reads clipboard TEXT only, and
        // returns early without emitting a V key event. For
        // an image clipboard egui therefore sees no signal
        // at all. We set a flag the Chat page consumes next
        // frame (the actual clipboard read + upload lives in
        // chat.rs so the networking/state code stays there).
        // Operator-reported 2026-05-15 (3rd attempt).
        if key == KeyCode::KeyV && pressed && state.ctrl_held
            && state.gui_state.active_page == GuiPage::Chat
        {
            state.gui_state.pending_clipboard_paste = true;
        }

        // Escape behavior (docs/design/navigation.md section 10.4, the rules
        // in gui/history.rs `GuiState::escape`): the nearest thing open closes
        // first (here: the help panel, the menu's preview, a popup); then a page
        // opened FROM inside another goes back to it (Chat → cog →
        // ServerSettings → Esc returns to Chat, v0.195.0); a page reached from
        // the menu goes into the world; the world brings back the page left; the
        // title screen stays put.
        // The nearest thing open closes first (section 10.4 rule 1), on every
        // screen, the world and the title screen included: an egui drop-down, else
        // the popup opened last (gui/layers.rs, the one close path; a popup asking
        // for a decision keeps the press and nothing else moves). A popup is nearer
        // than the pinned help panel: its shade covers the panel (navigation rung 3).
        if key == KeyCode::Escape && pressed && crate::gui::history::close_popup(&state.egui_ctx) {
            return;
        }
        // Then the pinned help panel (v0.1212): dismissing it is what Esc means
        // while it is up; everything below (showroom cancel, nav back, drop to
        // FPS) happens on the next press, unchanged.
        if key == KeyCode::Escape && pressed && state.gui_state.help_panel_pinned {
            state.gui_state.help_panel_pinned = false;
            return;
        }
        // Then the top menu's preview (navigation rung 2, section 10.4 rule 1): the
        // second row goes back to the page's own row at once.
        if key == KeyCode::Escape && pressed
            && !matches!(state.gui_state.active_page, GuiPage::None | GuiPage::MainMenu)
            && crate::gui::pages::escape_menu::end_preview(&state.egui_ctx)
        {
            return;
        }

        if key == KeyCode::Escape && pressed {
            // Esc cancels the showroom cleanly. Leaving showroom_active
            // set would make the next Play render the showroom instead
            // of the world. The character picker (mode 0, opened from a
            // menu via Play/Characters) returns to that menu; the
            // in-world mirror/wardrobe (modes 1/2, opened from FPS)
            // returns to first-person. (v0.476.1)
            if state.gui_state.showroom_active {
                // "Edit look" borrows the appearance editor FROM the
                // picker, so Esc there backs out to the picker rather
                // than dropping into the world past the Enter bar.
                if state.gui_state.showroom_return_to_picker
                    && state.gui_state.showroom_mode != 0
                {
                    state.gui_state.showroom_return_to_picker = false;
                    state.gui_state.showroom_mode = 0;
                    return;
                }
                let was_picker = state.gui_state.showroom_mode == 0;
                state.gui_state.showroom_active = false;
                state.gui_state.showroom_confirm = false;
                state.controller.showroom_lock = false;
                state.camera.switch_mode(crate::renderer::camera::CameraMode::FirstPerson);
                state.camera.position = state.showroom_return_pos;
                if was_picker {
                    state.gui_state.leave_world();
                }
                return;
            }

            state.gui_state.escape();
            // Cursor grab is handled by the single per-frame reconciliation
            // (keys off active_page/showroom/construction). Manipulating it
            // here too desynced `cursor_free` and broke later panels. (v0.460)
            return;
        }

        // Enter opens the in-world chat panel (v0.772): frees
        // the cursor + disables look/move so you can read + type
        // in the same relay chat as the Chat page. Only fires
        // when the panel is NOT already open (once open its input
        // has egui focus, so egui_consumed is true and this is
        // skipped; the panel handles Enter-to-send + Esc-to-close).
        if state.gui_state.keybinds.is(GA::OpenChat, &key_name) && pressed
            && state.gui_state.active_page == GuiPage::None
            && !state.gui_state.chat_input_active
            && !state.gui_state.showroom_active
            // Build mode has its own UI + click ownership; the death
            // screen must keep the keyboard (Enter opened a hidden
            // panel UNDER the Foreground death dim and stole focus).
            // (v0.779)
            && !state.gui_state.construction_active
            && state.gui_state.player_death_cause.is_none()
            && !egui_consumed
        {
            state.gui_state.chat_input_active = true;
            state.gui_state.chat_input_focus_pending = true;
            // View modes (increment 1c): the panel reopens in the
            // last mode used this session. A private conversation
            // left active (from the Chat page or a previous panel
            // session) opens under its OWN tab -- DMs or Groups --
            // so it can never render inside the public Channels
            // view. This replaces the v0.779 force-to-#general
            // guard: the panel now serves DMs/groups deliberately
            // (same stores + send paths as the Chat page, and the
            // E2EE confirm modal renders from the panel too). The
            // PASSIVE feed keeps its own public-only guard in
            // hud.rs -- it is unattended; the panel is not.
            let ch = state.gui_state.chat_active_channel.clone();
            if ch.starts_with("dm:") {
                state.gui_state.ingame_chat_mode =
                    crate::gui::IngameChatMode::Dms;
            } else if ch.starts_with("p2pgroup:") || ch.starts_with("group:") {
                state.gui_state.ingame_chat_mode =
                    crate::gui::IngameChatMode::Groups;
            }
            // Clear held movement so opening the panel mid-run doesn't
            // leave the player drifting (the panel's early-return above
            // swallows the key-release events). Zero BOTH movers: the
            // camera controller (FPS look/walk) AND the InputState the
            // ECS PlayerController reads to drive the rigid body. (v0.773)
            state.controller.stop_movement();
            state.data_store.insert("input_state", InputState::default());
        }

        // Tab = HOLD to reveal hidden labels (v0.429): peek markers
        // through walls across owned/explored rooms at x3 distance.
        // Tracks held state (true on press, false on release).
        if state.gui_state.keybinds.is(GA::RevealPeek, &key_name) {
            state.gui_state.reveal_held = pressed;
            if state.gui_state.active_page == GuiPage::None {
                return; // consume in-game (Tab was inventory pre-v0.429)
            }
        }
        // I opens inventory (took over from Tab when Tab became the reveal peek).
        if state.gui_state.keybinds.is(GA::Inventory, &key_name) && pressed
            && state.gui_state.active_page == GuiPage::None
        {
            state.gui_state.go(GuiPage::Inventory, crate::gui::How::Beside);
            // Cursor freed by the per-frame reconciliation (active_page != None).
            return;
        }
        // E opens/pins the targeted machine's info card (walk-up
        // interaction, v0.431). No early return: E still drives
        // input.interact for the ECS interaction system below.
        if state.gui_state.keybinds.is(GA::Interact, &key_name) && pressed
            && state.gui_state.active_page == GuiPage::None
        {
            // Vehicle take-over (Stage 3, v0.690) -- checked FIRST:
            // driving exits; facing a parked vehicle enters. Entering
            // cancels any self-drive route (the driver wins).
            if let Some(veh) = state.driving_vehicle.take() {
                // EXIT: step out beside the vehicle at ground level.
                let (pos, rot) = state
                    .game_world
                    .world
                    .get::<&humanity_core::ecs::components::Transform>(veh)
                    .map(|t| (t.position, t.rotation))
                    .unwrap_or((state.camera.position, Quat::IDENTITY));
                if let Ok(mut seat) = state
                    .game_world
                    .world
                    .get::<&mut humanity_core::ecs::components::VehicleSeat>(veh)
                {
                    seat.occupant_key = None;
                }
                let side = rot * Vec3::new(0.0, 0.0, 2.2);
                state.camera.position =
                    Vec3::new(pos.x + side.x, pos.y + 1.7, pos.z + side.z);
            } else if let Some(veh) = state.targeted_vehicle {
                // ENTER: seat the player, cancel self-drive.
                if state.game_world.world.contains(veh) {
                    let _ = state
                        .game_world
                        .world
                        .remove_one::<humanity_core::ecs::components::VehicleRoute>(veh);
                    if let Ok(mut seat) = state
                        .game_world
                        .world
                        .get::<&mut humanity_core::ecs::components::VehicleSeat>(veh)
                    {
                        seat.occupant_key = Some("player".to_string());
                    }
                    state.driving_vehicle = Some(veh);
                }
            } else if let Some(animal) = state.targeted_livestock {
                // Walk-up collect from a farm animal (v0.751):
                // the frame bridge settles it against the pack.
                state.pending_livestock_harvest = Some(animal);
            } else if let Some(npc_id) = state.gui_state.targeted_npc {
                // Walk-up TALK to a crew NPC (v0.797, operator: "I
                // can't interact with NPCs at all"). Opens the
                // dialogue card with a random greeting; repeat E /
                // the More button cycles the dialog lines. All text
                // arrived from the relay (welcome-snapshot
                // NpcProfile) -- the client only displays it.
                // Note: targeting (below, per frame) clears
                // targeted_livestock while an NPC is faced, so this
                // branch is what E does when a person is in front
                // of you.
                let mut opened = false;
                for (_e, npc) in state
                    .game_world
                    .world
                    .query::<&crate::net::sync::RemoteNpc>()
                    .iter()
                {
                    if npc.entity_id != npc_id {
                        continue;
                    }
                    // The relay's fields, and the card's choices and AI from the data
                    // (gui/talk/, embeddable-ai.md rung 3).
                    crate::gui::talk::open_for(&mut state.gui_state, npc, &state.data_store, &state.game_world.world);
                    // Wall-clock nanos as the greeting seed: cheap
                    // variety, no RNG state to carry (helper wraps).
                    let seed = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| u64::from(d.subsec_nanos()))
                        .unwrap_or(0);
                    state.gui_state.npc_talk_line =
                        crate::net::sync::pick_greeting(&npc.greetings, seed)
                            .unwrap_or_default()
                            .to_string();
                    opened = true;
                    break;
                }
                if opened {
                    state.gui_state.npc_talk_target = Some(npc_id);
                    // Talk quest emitter (v0.981, the second half of
                    // the v0.979 Travel pair): opening the dialogue
                    // IS "talked to them". Keyed by the stable name
                    // slug so quests reference people, not
                    // session-scoped entity ids. Re-opens re-fire,
                    // which is harmless (Talk objectives latch at 1)
                    // and lets a later-accepted quest complete on
                    // the next chat.
                    humanity_systems::quests::push_quest_event(
                        &state.data_store,
                        format!(
                            "talk_{}",
                            humanity_systems::quests::npc_talk_key(
                                &state.gui_state.npc_talk_name
                            )
                        ),
                    );
                    // No greetings authored -> lead with the first
                    // dialog line instead of an empty card.
                    if state.gui_state.npc_talk_line.is_empty() {
                        state.gui_state.npc_talk_advance();
                    }
                    // Same open-a-modal hygiene as the chat panel /
                    // creature editor: clear held movement in BOTH
                    // movers so the player doesn't drift while the
                    // modal guard swallows the key releases.
                    state.controller.stop_movement();
                    state.data_store.insert("input_state", InputState::default());
                }
            } else
 if let Some(cp) = state.gui_state.targeted_control_panel {
                // Looking at a door control panel (v0.567 + v0.570 locks): if the door
                // is LOCKED, E UNLOCKS it (Stage 1 unlocks every lock -- key/code/skill
                // enforcement is a follow-up); once unlocked, E opens/closes a manual
                // door. So a locked door takes two presses: unlock, then open.
                let locked = state
                    .door_panels
                    .get(cp)
                    .map_or(false, |panel| door_locked_now(&panel.0, state.door_locks.get(cp)));
                if locked {
                    if let Some(live) = state.door_locks.get_mut(cp) {
                        for s in live.iter_mut() {
                            if *s == humanity_ship::lock_types::LockState::Locked {
                                *s = humanity_ship::lock_types::LockState::Unlocked;
                            }
                        }
                    }
                } else if let Some(panel) = state.door_panels.get(cp) {
                    if !panel.0.auto_open {
                        if let Some(m) = state.door_manual_open.get_mut(cp) {
                            *m = !*m;
                        }
                    }
                }
            } else if state.interact.me.is_some() {
                // Seated or lying (rung 2b): E gets up, as a movement key does.
                crate::engine::interact::press_e(state);
            } else if crate::engine::built_uses::use_machine(state) {
                // The home's bed (its def `provides: "rest"`): E sleeps, as at a built bed,
                // and lies the player down in it (character animation rung 2b).
                crate::engine::interact::press_e(state);
            } else if crate::engine::interact::press_e(state) {
                // A seat (a chair, the couch): E sits the player on it (rung 2b,
                // engine/interact.rs); a movement key gets up.
            } else if let Some(t) = state.gui_state.targeted_machine {
                // Looking at a machine: toggle its card open/closed.
                state.gui_state.selected_machine =
                    if state.gui_state.selected_machine == Some(t) {
                        None
                    } else {
                        Some(t)
                    };
            } else if crate::engine::built_uses::activate(state) {
                // A built bed (sleep) or chest (open): engine/built_uses.rs.
            } else if crate::engine::death_pack::activate(state) {
                // A pack left where you fell: take back what fits (engine/death_pack.rs).
            } else if state.gui_state.selected_machine.is_some() {
                // Not looking at any machine but a card is pinned: E closes it
                // (so "[E] close" works from anywhere, not just at the machine).
                state.gui_state.selected_machine = None;
            } else if !state.gui_state.showroom_active {
                // Walk-up to a character station (v0.442, rewritten 2026-09-19).
                //
                // This used to match the room's ID against the literal strings
                // "wetroom" and "bedroom". Room ids in the live home come from the
                // zone that covers them (`room-bedroom`, `room-wetroom`, ...), so
                // neither string ever matched and the whole path was dead: there
                // was no way to reach the look editor from inside the world.
                //
                // It now reads the PAGE each of the room's actions opens, joined
                // from data/rooms.ron + data/rooms/room_actions.ron at load. A
                // room with an action that opens "appearance" is a mirror (the
                // bedroom's standing mirror); one that opens "wardrobe" is a
                // wardrobe (the dressing room). Any home anyone authors gets the
                // same stations for free by naming a room type, no code change.
                //
                // The page and not the label: `RoomBounds::actions` holds human
                // labels ("Change Outfit"), which exist to be read and can be
                // reworded, and the first attempt at this compared against the
                // raw action ids, so it matched nothing and stayed just as dead
                // as the room-id version it replaced.
                //
                // The room is the one the HUD's room card names (`hud::room_at`),
                // so the station works wherever the card says "Here: Change
                // Outfit", against a wall included (review of ship homes
                // increment 7: a room box stops short of its walls).
                let pages = crate::gui::pages::hud::room_at(&state.gui_state.room_bounds, state.camera.position)
                    .map(|r| r.action_pages.clone())
                    .unwrap_or_default();
                let opens = |page: &str| pages.iter().any(|x| x == page);
                if opens("appearance") {
                    open_showroom(state, 1);
                } else if opens("wardrobe") {
                    open_showroom(state, 2);
                }
            }
        }

        // 1-9 cast from the HUD ability bar (v0.754): the first
        // nine castable abilities, in the same order the bar
        // draws them. Ready-check here is a courtesy; the
        // AbilitySystem re-validates cost + cooldown anyway.
        if pressed
            && state.gui_state.active_page == GuiPage::None
            && !state.gui_state.showroom_active
        {
            let slot = state.gui_state.keybinds.hotbar_slot(&key_name);
            if let Some(i) = slot {
                let id = state
                    .gui_state
                    .abilities
                    .iter()
                    .filter(|a| a.castable_now)
                    .nth(i)
                    .filter(|a| a.cooldown_remaining <= 0.0)
                    .map(|a| a.id.clone());
                if let Some(id) = id {
                    // Offensive casts hit the creature the
                    // player is facing (v0.760).
                    let target = state
                        .targeted_livestock
                        .map(|e| u64::from(e.to_bits()));
                    state.gui_state.pending_cast = Some((id, target));
                }
            }
        }

        // F swings the held tool (or bare hands) at the faced
        // creature (v0.765). The frame bridge settles it with
        // the worn hands-slot weapon's damage + reach.
        if state.gui_state.keybinds.is(GA::AttackSwing, &key_name)
            && pressed
            && state.gui_state.active_page == GuiPage::None
            && !state.gui_state.showroom_active
        {
            state.gui_state.pending_swing = true;
        }

        // G opens the walk-up creature EDITOR for the creature
        // you're facing (dev tool, v0.778; Dev play mode +
        // cheats switch since task #50). The actual snapshot of
        // its values into the edit buffers runs next frame in
        // the main loop (queries live there); opening frees the
        // cursor + disables look/move via the modal guard above.
        if state.gui_state.keybinds.is(GA::CreatureEditor, &key_name)
            && pressed
            && state.gui_state.active_page == GuiPage::None
            && !state.gui_state.showroom_active
            // Same gates as the chat panel (v0.779): build mode owns
            // its own clicks, and the death screen keeps the keyboard.
            && !state.gui_state.construction_active
            && state.gui_state.player_death_cause.is_none()
            && state.gui_state.dev_cheats_active(&state.theme)
        {
            if let Some(ent) = state.targeted_livestock {
                state.gui_state.dev_edit_target = Some(u64::from(ent.to_bits()));
                state.gui_state.dev_edit_snapshot_pending = true;
                state.controller.stop_movement();
                state.data_store.insert("input_state", InputState::default());
            }
        }

        // R toggles the home roof/ceiling (construction mode, v0.453). Off by
        // default so the sky (stars + the real solar system) shows through the
        // open top; on for a sealed look or atmosphere tests. Also exposed as a
        // checkbox on the Settings page (GUI-first).
        if state.gui_state.keybinds.is(GA::ToggleRoof, &key_name) && pressed
            && state.gui_state.active_page == GuiPage::None
            && !state.gui_state.showroom_active
        {
            state.gui_state.show_roof = !state.gui_state.show_roof;
            return;
        }

        // H toggles the HULL WRAP (ship-superstructure increment D): the
        // generated exterior shell around the zone cluster. On by default so
        // the ship reads as a vessel from outside; off for unobstructed
        // interior screenshots or a clear top-down build view. Also exposed
        // as a Settings checkbox next to Show roof (GUI-first).
        if state.gui_state.keybinds.is(GA::ToggleHull, &key_name) && pressed
            && state.gui_state.active_page == GuiPage::None
            && !state.gui_state.showroom_active
        {
            state.gui_state.show_hull = !state.gui_state.show_hull;
            return;
        }

        // B toggles the construction editor (v0.455): craft each room's walls
        // live. On open, mirror the live layout into the editable GuiState.
        if state.gui_state.keybinds.is(GA::BuildEditor, &key_name) && pressed
            && state.gui_state.active_page == GuiPage::None
            && !state.gui_state.showroom_active
        {
            crate::engine::editor::toggle_build_editor(state);
            return;
        }

        // Don't pass input to the game when egui consumed it or a menu is open.
        // During construction we DO pass it: WASD flies the orbit focal point,
        // Space/Shift change level (egui_consumed still wins, so typing in a
        // field never moves the camera). (v0.464)
        if egui_consumed || state.gui_state.active_page != GuiPage::None {
            return;
        }

        // Camera + movement route through the rebindable keymap
        // (2026-08-12): resolve the key to its single action and
        // forward it. Replaces process_keyboard's hardcoded
        // W/A/S/D/F/V/M/O/Q/E match in camera.rs.
        let key_action = state.gui_state.keybinds.action_for(&key_name);
        if let Some(act) = key_action {
            // The controller takes it, and a key going down
            // releases the probe hold (the pin exists only for
            // scripted captures; a human at the keys wins at
            // once). One function with the rig's held keys
            // (engine/rig_walk.rs), so the two cannot drift.
            crate::engine::input::apply_game_action(&mut state.controller, &mut state.probe_hold, act, pressed);
        }

        // Update InputState in DataStore for game systems
        let mut input = state.data_store
            .get::<InputState>("input_state")
            .cloned()
            .unwrap_or_default();
        match key_action {
            Some(GA::MoveForward) => input.forward = pressed,
            Some(GA::MoveBack) => input.backward = pressed,
            Some(GA::MoveLeft) => input.left = pressed,
            Some(GA::MoveRight) => input.right = pressed,
            Some(GA::Jump) => input.jump = pressed,
            Some(GA::Interact) => input.interact = pressed,
            _ => {}
        }
        // Dead players do not walk: freeze movement input while
        // the death screen is up (v0.745). Held keys clear too.
        if state.gui_state.player_death_cause.is_some() {
            input = InputState::default();
        }
        state.data_store.insert("input_state", input);
    }
}
