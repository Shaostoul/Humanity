//! Multiplayer co-presence (the file-size split, step 1, 2026-11-14), moved out of
//! `window_event` in src/lib.rs as it was: joining the shared world while in it and connected to a
//! relay, and syncing the players around you.

use crate::engine::frame_prelude::*;

/// Syncs the shared world and the players around you for this frame.
pub(crate) fn run(state: &mut EngineState) {
    let dt = state.clock.dt;
    let clock_dt = state.clock.clock_dt;
    humanity_core::frame_costs::span("cpu.span.copresence");
    // ── Multiplayer co-presence (v0.472) ──────────────────────────────────────
    // While actually in the 3D world AND connected to a relay, join the shared game
    // world once (over the existing authenticated chat socket), stream our position,
    // and apply remote players. The relay validates + broadcasts; net_sync spawns /
    // moves / interpolates RemotePlayer entities, which the render pass draws.
    {
        let in_world = state.gui_state.active_page == GuiPage::None
            && !state.gui_state.showroom_active
            && !state.gui_state.construction_active;
        let connected = state
            .gui_state
            .ws_client
            .as_ref()
            .map_or(false, |w| w.is_connected());
        // A switch to another server while joined leaves the one we joined
        // on, and a fresh connection tries a server that refused us again
        // (ship homes 1b, engine/home_plot.rs `follow_server`).
        crate::engine::home_plot::follow_server(state);
        // Then the save follows the pairing: your own home's, or this server's
        // (ship homes increment 6, engine/save_slot.rs `follow`).
        crate::engine::save_slot::follow(state);
        if connected {
            // SOLO step-out (v0.801): joined but the player switched to
            // solo (offline home entry, or Dev travel engaging) -- leave
            // the shared WORLD deliberately: the relay despawns our
            // entity + broadcasts game_player_left (others see us leave,
            // honestly), the chat socket stays up. Rejoining is just the
            // normal join below once solo clears: a fresh join, progress
            // restored from storage (since 2026-10-03 a leave despawns
            // at once; it used to be held as a reconnect). The host's
            // clock and the other players go with it (forget_shared_world).
            if state.game_joined && state.gui_state.copresence_solo {
                if let Some(ref ws) = state.gui_state.ws_client {
                    ws.send(&serde_json::json!({"type": "game_leave"}).to_string());
                }
                crate::engine::home_plot::forget_shared_world(state);
                log::info!("Co-presence: stepped out of the shared world (solo)");
            }
            // Join once, on first entering the world while connected --
            // and only after the identify handshake BOUND the socket
            // (v0.794): the relay's pre-bind loop silently discards
            // every non-identify message, so a game_join racing the
            // Dilithium challenge simply vanished (client showed
            // "Shared world", server counted nobody -- caught by the
            // v0.793 autopilot two-instance test). Only ABOARD (ship homes
            // 1b): from a planet or a Dev trip the camera is not in ship
            // metres, and a welcome there moved the player by ship
            // coordinates in the planet's frame (home_plot.rs `aboard`).
            // And only once the world has LOADED with its ship (1b, round 4):
            // this block runs before load_world in the frame Enter World or
            // Play is pressed, and a returning player's socket is identified
            // by then, so the join went out naming no ship and was refused
            // as another ship (home_plot.rs `join_step`).
            let gate = crate::engine::home_plot::JoinGate {
                in_world,
                joined: state.game_joined,
                identified: state.gui_state.ws_identified,
                solo: state.gui_state.copresence_solo,
                aboard: crate::engine::home_plot::aboard(state),
                refused_here: state.copresence_refused.as_deref()
                    == Some(state.gui_state.active_server_key().as_str()),
                world_loaded: state.world_loaded,
                has_ship: state.gui_state.ship_structure.is_some(),
                slot_ready: crate::engine::save_slot::slot_ready(&state.gui_state),
            };
            match crate::engine::home_plot::join_step(&gate) {
                crate::engine::home_plot::JoinStep::Wait => {}
                crate::engine::home_plot::JoinStep::RefuseOwnShip => {
                    // The world loaded on the legacy layout: no ship to stand on
                    // or to name. Say so under the HUD instead of joining.
                    crate::engine::home_plot::refuse_shared_world(
                        state,
                        crate::engine::home_plot::OWN_SHIP.to_string(),
                        None,
                    );
                }
                crate::engine::home_plot::JoinStep::Join => {
                    let name = if state.gui_state.character_name.trim().is_empty() {
                        "Wanderer".to_string()
                    } else {
                        state.gui_state.character_name.clone()
                    };
                    // character_mode is reserved for the open/closed-server model
                    // (relay ignores extra fields today; envelope right from day one).
                    let mut join = serde_json::json!({
                        "type": "game_join",
                        "player_name": name,
                        "character_mode": "local",
                        // How this player looks to the others (2026-09-29), with what
                        // they wear (2026-11-09, the look).
                        "appearance": humanity_core::player_look::PlayerLook::from_appearance_wearing(&state.gui_state.appearance, &state.gui_state.outfit.equipped).to_json(),
                    });
                    // Our ship and our own door, for the plot (increment 1b).
                    // Never a join without a ship (round 4).
                    if crate::engine::home_plot::add_join_fields(&mut join, state.gui_state.ship_structure.as_ref(), &humanity_systems::persistence::live_save::active_home().start) {
                        // After a death: the respawner to wake at, so the relay stands us
                        // there (game-setting-and-intro.md 5.6, rung 2, engine/wake.rs).
                        crate::engine::wake::add_to_join(&mut join, state);
                        if let Some(ref ws) = state.gui_state.ws_client {
                            ws.send(&join.to_string());
                        }
                        state.game_joined = true;
                        state.game_welcomed = false; // our plot is not known yet (1b)
                        state.game_pos_timer = 0.0;
                    } else {
                        crate::engine::home_plot::refuse_shared_world(
                            state,
                            crate::engine::home_plot::OWN_SHIP.to_string(),
                            None,
                        );
                    }
                }
            }
            // While joined, KEEP the session alive across menu
            // round-trips (v0.779): the old code tore down every
            // RemotePlayer on any page open, then re-sent game_join
            // on return -- which the relay silently ignored as a
            // duplicate, so the roster degraded to "Player N"
            // placeholders and stationary players went invisible.
            // Now: net_sync keeps applying remote updates in menus
            // (avatars/names survive); only position SENDING is
            // limited to actually being in the world.
            if state.game_joined {
                // 15 updates a second in the world, stamped on real time
                // (clock_dt), plus one standing-still update the frame we
                // leave it (engine::net_route, net::sync, 2026-10-03).
                drive_position_send(state, in_world, dt, clock_dt);
                crate::engine::fleet::tick(state, clock_dt); // the fleet ledger (2026-10-04)
                state.net_sync.set_clock_step(clock_dt);
                // `tick` is the System trait method; call it fully-qualified.
                humanity_core::ecs::systems::System::tick(
                    &mut state.net_sync,
                    &mut state.game_world.world,
                    dt,
                    &state.data_store,
                );
                // Mirror the shared-world roster into GuiState so the
                // paint-only HUD can show co-presence (v0.774): who else
                // is here right now, from the RemotePlayer entities
                // net_sync maintains, a blocked player left out (10m R9).
                let names = crate::engine::net_route::copresence_names(
                    &state.game_world.world,
                    &|r| crate::engine::block::hides_player(&state.gui_state, r),
                );
                state.gui_state.copresence_active = true;
                // Compare-before-assign: the roster only changes on
                // join/leave/rename, not every frame.
                if names != state.gui_state.copresence_names {
                    state.gui_state.copresence_names = names;
                }
            }
        } else if state.game_joined {
            // DISCONNECTED: allow a fresh join on reconnect + clear
            // remote avatars and the relay's crew (they are stale
            // without a live feed; a rejoin starts from fresh updates).
            crate::engine::home_plot::forget_shared_world(state);
        }
        crate::engine::shared_build::tick(state, clock_dt); // pieces the server keeps: sent, or given back out of the shared world (increment 5)
    }

}
