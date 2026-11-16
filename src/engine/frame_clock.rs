//! The frame's clock (the file-size split, step 1, 2026-11-14), moved out of `window_event` in
//! src/lib.rs as it was: the frame-rate cap's sleep, the frame's steps (the true step for the
//! counters, the clamped step the simulation runs on, the uncapped clock step), the planet's spin
//! and the real clock every body is placed by, each computed once for the whole frame.

use crate::engine::frame_prelude::*;

/// Sleeps off the frame-rate cap, then stamps the frame's time into `f` and `state`.
pub(crate) fn run(state: &mut EngineState, f: &mut FrameVars) {
    // Frame-rate caps (v0.1016, operator request): sleep off
    // the remainder of the frame budget BEFORE stamping dt,
    // so game time simply sees a longer (correct) frame.
    // Foreground cap while the window has focus; background
    // cap while alt-tabbed ("sync" = background follows
    // foreground). Unlimited = no engine pacing (vsync still
    // paces presentation when on). std::thread::sleep is
    // ~1.5 ms-granular on Windows - fine for a cap, not a
    // precision limiter.
    {
        let s = &state.gui_state.settings;
        let cap = if state.window_focused || s.fps_background_sync {
            if s.fps_foreground_unlimited { 0 } else { s.fps_foreground }
        } else {
            s.fps_background
        };
        if cap > 0 && state.movie.is_none() {
            let target =
                std::time::Duration::from_secs_f64(1.0 / cap as f64);
            let elapsed = state.last_frame.elapsed();
            if elapsed < target {
                // Deliberate pacing, recorded as its own
                // stage so it is never mistaken for work
                // inside `cpu.frame_total`.
                let sleep_t0 = Instant::now();
                std::thread::sleep(target - elapsed);
                humanity_core::frame_costs::record_cpu(
                    "cpu.fps_cap_sleep",
                    sleep_t0.elapsed(),
                );
            }
        }
    }
    humanity_core::frame_costs::span_begin("cpu.span.station"); // the frame's spans: frame_costs::span
    let now = Instant::now();
    // raw_dt is the TRUE frame delta, for the perf counters ONLY.
    // The clamped dt below protects the SIMULATION from spiral-of-
    // death on hitches -- but stamping fps/frame_times from the
    // clamped value CENSORS every reading at exactly 10 fps /
    // 100 ms, which hid a true 131 ms forest frame behind "10 fps"
    // in every probe manifest until 2026-07-31 (P2, see
    // docs/design/vegetation-cards-every-species.md).
    let raw_dt = (now - state.last_frame).as_secs_f32();
    let dt = raw_dt.min(0.1);
    // Recording: exactly 1/fps, whatever the frame took (engine::movie).
    let movie_dt = crate::engine::movie::frame_step(state, now);
    let dt = movie_dt.unwrap_or(dt);
    // Real time, uncapped (the movie's step while recording): the
    // shared-world clocks run on it (net::sync, 2026-10-03).
    let clock_dt = movie_dt.unwrap_or(raw_dt);
    state.last_frame = now;
    // The real clock every body but Earth is placed and turned by, and the frame's clock advanced
    // (the one clock the sim reads, `frame_sim::FrameClock`; the soak advances its own).
    let sim_epoch_s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
        - 946_728_000.0;
    state.clock.advance(dt, clock_dt, raw_dt, sim_epoch_s);
    f.now = now;
}
