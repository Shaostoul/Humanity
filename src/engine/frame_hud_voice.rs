//! The performance counters, voice keys and the updater (the file-size split, step 1, 2026-11-14), moved out of
//! `window_event` in src/lib.rs as they were: the FPS counter and the F2 overlay's frame-time ring
//! (from the true step), the push-to-talk keys, and the updater's background results.

use crate::engine::frame_prelude::*;

/// Updates the frame counters, the voice keys and the updater's results.
pub(crate) fn run(state: &mut EngineState) {
    let raw_dt = state.clock.raw_dt;
    let dt = state.clock.dt;
    // Update FPS counter -- from raw_dt, NOT the sim-clamped dt,
    // or everything slower than 10 fps reads exactly 10 fps (P2).
    humanity_core::frame_costs::span("cpu.span.hud_voice");
    state.gui_state.fps = if raw_dt > 0.0 { 1.0 / raw_dt } else { 0.0 };
    // Frame-time ring buffer for the F2 performance overlay sparkline.
    {
        let ft = &mut state.gui_state.frame_times;
        ft.push(raw_dt * 1000.0);
        if ft.len() > 120 {
            let excess = ft.len() - 120;
            ft.drain(0..excess);
        }
    }
    // Sample EngineState diagnostics for the F2/F4 overlays, but ONLY
    // while the relevant overlay is open so the syscall/entity-walk
    // cost nothing when hidden. (v0.482)
    if state.gui_state.show_perf_overlay || state.gui_state.show_system_overlay {
        state.gui_state.diag_uptime_secs = state.start_time.elapsed().as_secs();
    }
    if state.gui_state.show_perf_overlay {
        state.gui_state.diag_entity_count = state.game_world.world.iter().count();
        state.gui_state.diag_light_count = gpu!(state).renderer.light_count();
    }
    if state.gui_state.show_system_overlay {
        if let Some(u) = memory_stats::memory_stats() {
            state.gui_state.diag_mem_mb = u.physical_mem as f32 / 1_048_576.0;
        }
    }
    // Settings audio "Test microphone" toggle: keep the mic loopback
    // running while mic_test_active, on the chosen devices (v0.485).
    {
        let want = state.gui_state.mic_test_active;
        // Start/stop only on the toggle EDGE, never every frame: a
        // failing start drops MIC_RUNNING back to false within a frame,
        // so a per-frame "start if not running" would spin-retry and
        // flicker the status between "Starting..." and "Failed".
        if want && !state.gui_state.mic_test_prev {
            crate::net::voice::start_mic_test(
                state.gui_state.audio_input_device.clone(),
                state.gui_state.audio_output_device.clone(),
            );
        } else if !want && state.gui_state.mic_test_prev {
            crate::net::voice::stop_mic_test();
        }
        state.gui_state.mic_test_prev = want;
        // If the start failed (async, in the worker thread), flip the
        // toggle back off so the button resets to "Test microphone" and
        // does not look stuck on. The "Failed: ..." status stays visible.
        if want
            && !crate::net::voice::mic_test_running()
            && crate::net::voice::mic_status().starts_with("Failed")
        {
            state.gui_state.mic_test_active = false;
            state.gui_state.mic_test_prev = false;
        }
        // Decayed peak-hold meter for the UI.
        let lvl = crate::net::voice::mic_level();
        state.gui_state.mic_meter = (state.gui_state.mic_meter * 0.85).max(lvl);
    }
    // v0.488/490: push the live voice input params (gain / filter /
    // transmit mode / activation threshold) to the worker every frame, so
    // changes apply without restarting the test. `voice_ptt_held` is
    // maintained by the raw winit key handler above (works in-game + for
    // CapsLock); it only matters for the push-to-talk / push-to-mute modes.
    {
        crate::net::voice::set_input_params(
            state.gui_state.voice_gain,
            state.gui_state.voice_filter_mode,
            state.gui_state.voice_transmit_mode,
            state.gui_state.voice_vad_threshold,
            state.gui_state.voice_ptt_held,
        );
    }
    // v0.494 Phase D: run the live voice session while joined to a
    // voice room (capture + encode + decode/play), and pump captured
    // Opus to every connected peer. Edge-triggered start/stop.
    {
        // Outgoing-call ring-out timeout (v0.705): the callee has
        // 30 s to accept (matching the web). On expiry, cancel the
        // ring (tell them to stop) and clear.
        if let Some(deadline) = state.gui_state.call_outgoing_deadline {
            if std::time::Instant::now() >= deadline {
                if let Some((k, _)) = state.gui_state.call_outgoing.take() {
                    if let Some(ref client) = state.gui_state.ws_client {
                        let _ = client.send(&serde_json::json!({
                            "type": "voice_call",
                            "from": state.gui_state.profile_public_key,
                            "to": k,
                            "action": "hangup",
                        }).to_string());
                    }
                }
                state.gui_state.call_outgoing_deadline = None;
            }
        }

        // Voice rooms AND 1:1 calls run the same live session +
        // Opus pump; a call peer lands in voice_connected_peers
        // via the same VoiceConnected event as room peers (v0.703).
        let want_session = crate::gui::pages::chat::voice_wanted(&state.gui_state);
        if want_session && !state.gui_state.voice_session_prev {
            crate::net::voice::start_voice_session(
                state.gui_state.audio_input_device.clone(),
                state.gui_state.audio_output_device.clone(),
            );
        } else if !want_session && state.gui_state.voice_session_prev {
            crate::net::voice::stop_voice_session();
            state.gui_state.voice_connected_peers.clear();
        }
        state.gui_state.voice_session_prev = want_session;
        // Send captured mic frames to each connected voice peer.
        // Skipped while muted in a 1:1 call (v0.705): we stop
        // transmitting but still receive + play the peer's audio.
        let muted_in_call =
            state.gui_state.call_active.is_some() && state.gui_state.call_muted;
        if want_session
            && !muted_in_call
            && !state.gui_state.voice_connected_peers.is_empty()
        {
            let frames = crate::net::voice::drain_voice_send();
            if !frames.is_empty() {
                let peers: Vec<String> = state.gui_state.voice_connected_peers.iter().cloned().collect();
                if let Some(ref webrtc) = state.gui_state.webrtc {
                    for opus in frames {
                        for peer in &peers {
                            webrtc.send_voice(peer.clone(), opus.clone());
                        }
                    }
                }
            }
        }
    }

    // v0.707: a fresh machine resolves data_dir to the CWD
    // fallback while the storage mode is still Undecided. Once
    // the first-boot chooser picks a home (and extraction runs),
    // re-resolve so reads + editor saves target the chosen dir.
    if state.data_dir == std::path::Path::new("data")
        && humanity_core::storage::mode() != humanity_core::storage::StorageMode::Undecided
    {
        state.data_dir = find_data_dir();
    }

    // Poll updater for background thread results
    if state.gui_state.updater.poll(dt as f64) {
        // An update just became available -- show toast
        state.gui_state.update_toast_visible = true;
    }

}
