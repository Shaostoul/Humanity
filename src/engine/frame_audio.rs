//! The frame's audio (the file-size split, step 1, 2026-11-14), moved out of
//! `window_event` in src/lib.rs as it was: sound previews, the music and effects the GUI asked for,
//! and the click sound for the GUI's events.

use crate::engine::frame_prelude::*;

/// Plays this frame's sounds; `full_output` is the egui pass's output.
pub(crate) fn run(state: &mut EngineState, full_output: &egui::FullOutput) {
    humanity_core::frame_costs::span("cpu.span.audio");
    // The live music and the ship's hum (2026-11-06, engine/music.rs).
    crate::engine::music::frame(state);
    if let Some(audio) = state.audio.as_mut() {
        // The Media page's video (and its window over the chat) and a video posted
        // in chat play their sound here.
        crate::gui::pages::files::sync_sound(&mut state.gui_state, audio);
        crate::gui::pages::chat::sync_video_sound(&mut state.gui_state.chat_video, audio);
        // Settings > Audio > Even out loudness (2026-11-06): read as each
        // sound starts (humanity_core::audio::levels).
        audio.set_even_loudness(state.gui_state.settings.even_loudness);
    }
    // Honest volume sliders: every bus's slider into kira, only when one
    // moved (engine/sfx.rs: master, music, sound effects, interface,
    // notifications, ambience).
    crate::engine::sfx::sync_volumes(state);
    if let Some(audio) = state.audio.as_mut() {
        // Settings > Audio > Sound levels' Play (2026-11-06): the sound at
        // the person's level for it now, after the volumes above.
        crate::gui::pages::settings::sound_levels::play_preview(&state.egui_ctx, audio, &state.sound_catalog);
        // ── Footsteps (v0.968, audio increment 2):
        // a stride meter over whichever channel is
        // actually walking - the planet walk band
        // moves the frame-lock anchor, home walking
        // moves the camera. Fly mode is silent. ──
        {
            let on_planet = state.surface_owns_translation
                && state.surface_walk_band
                && !state.controller.fly_mode;
            let aboard = state.aboard_station
                && !state.controller.fly_mode
                && state.camera.mode
                    == crate::renderer::camera::CameraMode::FirstPerson;
            let d_anchor = (state.frame_lock_anchor
                - state.prev_stride_anchor)
                .length() as f32;
            let d_cam =
                (state.camera.position - state.prev_stride_cam).length();
            state.prev_stride_anchor = state.frame_lock_anchor;
            state.prev_stride_cam = state.camera.position;
            // v0.996: airborne is SILENT - a jump or a
            // thrust arc used to keep clicking heels
            // (the anchor still moves horizontally).
            // The accumulator also resets so landing
            // doesn't fire an instant catch-up step.
            let step = if on_planet {
                if state.on_ground_planet { d_anchor } else { 0.0 }
            } else if aboard {
                d_cam
            } else {
                0.0
            };
            if on_planet && !state.on_ground_planet {
                state.stride_accum = 0.0;
            }
            // Teleports and band handoffs produce huge
            // one-frame deltas - never a footstep.
            if step > 0.001 && step < 2.0 {
                state.stride_accum += step;
                // 1.5 m per step (v0.996, was 0.75:
                // "the footsteps sound way too fast" -
                // a real walking stride, half the rate).
                if state.stride_accum >= 1.5 {
                    state.stride_accum = 0.0;
                    let surface = if on_planet {
                        humanity_core::audio::sounds::SurfaceType::Grass
                    } else {
                        humanity_core::audio::sounds::SurfaceType::Metal
                    };
                    let rel =
                        surface.footstep_sound_from(&state.sound_catalog);
                    let vol = state
                        .sound_catalog
                        .volume_or_default(surface.footstep_id());
                    let path = humanity_core::audio::sounds::asset_path(&humanity_core::storage::assets_dir(), &rel);
                    let _ = audio.play_sound_vol(&path, vol);
                }
            } else if step >= 2.0 {
                state.stride_accum = 0.0;
            }
        }
        // UI click (v0.1112): fire ONLY when egui
        // reported a REAL widget activation this frame -
        // a button/link/toggle was actually clicked -
        // not merely because the pointer was pressed
        // over some egui area. The old gate keyed on
        // wants_pointer_input(), true whenever the
        // pointer sat over ANY egui panel, so clicking
        // empty background clicked-sounded ("clicking on
        // the screen makes a sound even if I don't click
        // anything"). platform_output.events carries an
        // OutputEvent::Clicked only on genuine activation.
    }
    // The sounds actions, notices and the ECS systems queued this frame,
    // each on its bus (engine/sfx.rs, BUG-265).
    crate::engine::sfx::play_queued(state);
    // UI click (v0.1112): fire ONLY when egui reported a REAL widget
    // activation this frame (a button, link or toggle actually clicked),
    // not merely a press over some egui area ("clicking on the screen
    // makes a sound even if I don't click anything"):
    // platform_output.events carries an OutputEvent::Clicked only on
    // genuine activation. Read it HERE: handle_platform_output consumes
    // full_output.platform_output below. The catalogue drives the file and
    // its "ui" bus the interface volume.
    if crate::gui::ui_click_should_sound(&full_output.platform_output.events, state.gui_state.settings.ui_sounds_enabled) {
        crate::engine::sfx::play_click(state);
    }

    // Handle egui platform output (cursor changes, clipboard, etc.)
}
