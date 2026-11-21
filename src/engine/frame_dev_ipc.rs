//! The dev control link's frame stage (the file-size split, step 1, 2026-11-14), moved out of
//! `window_event` in src/lib.rs as it was: the dev autopilot's zero-click world entry, the dev IPC
//! requests, and the Travel list's bookmark edits.

use crate::engine::frame_prelude::*;

/// Runs the dev autopilot and the dev control link for this frame.
pub(crate) fn run(state: &mut EngineState) {
    let clock_dt = state.clock.clock_dt;
    humanity_core::frame_costs::span("cpu.span.dev_ipc");
    // Dev autopilot (v0.793): zero-click world entry for scripted
    // co-presence tests. Runs immediately BEFORE the auto-connect
    // block so a dropped request is honored the same frame.
    poll_autopilot_request(state);
    crate::engine::mcp::frame_tick(state); // the dev MCP endpoint: off unless HUMANITY_DEV_MCP=1 (docs/ai/mcp.md)
    // Megashader hot-reload poll (v0.924 dev-aid): lives with
    // the other unconditional per-frame polls - effectively
    // free when idle (one try_recv); a saved shader edit
    // swaps the PSOs in seconds with world state intact.
    if let Some(r) = gpu!(state).renderer.world_mut() { r.poll_shader_reload(); } // the world's (engine/renderer_slot.rs)
    // The two cloud PSOs compile in the background at boot
    // (2026-10-18): installed on the first frame after they
    // finish, and recorded as BACKGROUND spans, which the boot
    // timer never counts toward the time to playable.
    if let Some(c) = gpu!(state).renderer.world_mut().and_then(|r| r.poll_deferred_pipelines()) {
        let since_open = state.boot_timer.boot_start.elapsed();
        state.boot_timer.record_background("cloud_psos_compile", c.compile);
        state.boot_timer.record_background("clouds_ready_after_open", since_open);
    }

    // Dev camera request (v0.813): the file-request sibling of the
    // Dev page's Travel tool, for scripted visual verification --
    // the operator's field-report screenshots reproduced without a
    // pilot. Drop debug/camera_request.json while the app runs:
    //   {"body":"earth","altitude_km":400,"look_offset_deg":0}
    // and the next frame teleports the local frame to the SUNLIT
    // side of that body at that altitude, aims at the body center
    // (look_offset_deg rotates the aim toward the horizon; ~85 at
    // low altitude frames the limb), engages fly mode, and writes
    // debug/camera_done.json. Composes with autopilot_request +
    // screenshot_request into a complete look-anywhere capture
    // recipe. Runs HERE (the every-frame update path, same as the
    // autopilot poll) and not in the GUI-action section, which
    // stops running once a world is loaded. Permanent dev tooling.
    poll_camera_request(state);
    crate::engine::ipc::poll_remote_players_request(state, clock_dt); // drawn remote players for verify-copresence
    crate::engine::ipc::poll_door_points_request(state); // the ship's door points, for verify-copresence (increment 2)
    crate::engine::talk::frame(state); // the talk card: its dev IPC, its AI box, the way it shows (engine/talk.rs)
    crate::engine::movie::poll_request(state);
    crate::engine::movie::steer(state);
    crate::engine::ipc::poll_cloudmap_request(state);
    crate::engine::ipc::poll_cloud_profile_dump_request(state);
    // In-world screen dev IPC (debug/screen_request.json): parsed
    // here in the update phase; the event is queued after the
    // look ray runs and completed after the surface draws, both
    // in the screens block just before the scene passes.
    crate::engine::ipc::poll_screen_request(state);
    // Main-UI dev IPC (debug/ui_request.json, 2026-09-18): the
    // F10 / Escape rules and synthetic pointer events against
    // the MAIN egui context. Parsed and advanced here, before
    // take_egui_input runs below, so a queued event lands in
    // THIS frame's egui pass; answered after the frame, next to
    // the post-present reconcile_cursor, so the done file
    // reports the cursor as the authority left it.
    crate::engine::ipc::poll_ui_request(state);

    // F6 location bookmark save (v0.890): runs here so
    // current_spin + frame-lock state are fresh this frame.
    save_location_bookmark(state);
    // Bookmark GUI bridge (v0.913): reload the Dev > Travel
    // list when the file changed, and run GUI-triggered
    // bookmark teleports through the SAME restore path the
    // dev camera_request uses.
    if state.gui_state.location_bookmarks_dirty {
        state.gui_state.location_bookmarks_dirty = false;
        state.gui_state.location_bookmarks = std::fs::read_to_string(
            "debug/bookmarks.json",
        )
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|root| {
            root.get("bookmarks").and_then(|b| b.as_array()).map(|arr| {
                arr.iter()
                    .filter_map(|e| {
                        let name = e.get("name")?.as_str()?.to_string();
                        let cat = e
                            .get("category")
                            .and_then(|c| c.as_str())
                            .unwrap_or("Uncategorized")
                            .to_string();
                        let body = e
                            .get("body")
                            .and_then(|b| b.as_str())
                            .unwrap_or("space")
                            .to_string();
                        Some((name, cat, body))
                    })
                    .collect()
            })
        })
        .unwrap_or_default();
    }
    if let Some(id) = state.gui_state.pending_bookmark_teleport.take() {
        let req = serde_json::json!({ "bookmark": id });
        if !restore_location_bookmark(state, &req) {
            state.gui_state.pending_toasts.push((
                format!("Bookmark {id} could not be restored"),
                crate::gui::ToastKind::Error,
            ));
        }
    }
    // Bookmark file edits from the Travel list (v0.914):
    // delete a bookmark or move it to another category.
    let delete = state.gui_state.pending_bookmark_delete.take();
    let recat = state.gui_state.pending_bookmark_recat.take();
    if delete.is_some() || recat.is_some() {
        const BM_PATH: &str = "debug/bookmarks.json";
        if let Some(mut root) = std::fs::read_to_string(BM_PATH)
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        {
            if let Some(arr) = root
                .get_mut("bookmarks")
                .and_then(|b| b.as_array_mut())
            {
                if let Some(id) = &delete {
                    arr.retain(|e| {
                        e.get("name").and_then(|n| n.as_str()) != Some(id)
                    });
                }
                if let Some((id, cat)) = &recat {
                    for e in arr.iter_mut() {
                        if e.get("name").and_then(|n| n.as_str())
                            == Some(id.as_str())
                        {
                            e["category"] =
                                serde_json::json!(cat.trim().to_string());
                        }
                    }
                }
            }
            let _ = std::fs::write(
                BM_PATH,
                serde_json::to_string_pretty(&root).unwrap_or_default(),
            );
            state.gui_state.location_bookmarks_dirty = true;
            if let Some(id) = delete {
                state.gui_state.pending_toasts.push((
                    format!("Bookmark {id} deleted"),
                    crate::gui::ToastKind::Info,
                ));
            }
        }
    }

}
