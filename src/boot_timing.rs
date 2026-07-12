//! Boot-phase timing (2026-07-12 dev tooling).
//!
//! Replaces guessing about startup cost with MEASURED per-phase spans. A
//! `BootTimer` is created at the top of `resumed()` (window-open), accumulates
//! named spans as each boot phase completes (data extraction, window + renderer
//! init, then the lazy world-load sub-phases: homestead meshes, orrery
//! hologram, star catalog + Milky Way glow, planet surface bake, ...), and at
//! the moment the 3D world is fully loaded emits ONE consolidated summary:
//!   * to the log (`=== BOOT TIMING ===`, one line per phase), and
//!   * to `debug/boot_timing.json` -- the same machine-readable JSON-drop
//!     convention as `debug/screenshot_done.json` / `debug/camera_done.json`,
//!     so a scripted or AI dev session can `Read` the real numbers instead of
//!     eyeballing the wall clock.
//!
//! Std-only (plus serde_json, already a dependency); imports nothing from the
//! renderer/GUI/winit, so it is trivially safe and the module is native-gated
//! (only the native boot path uses it -- keeps the relay build lean).

use std::time::{Duration, Instant};

/// Accumulates named boot-phase spans and emits a one-shot summary once the
/// world is loaded. Cost is a `Vec` push per phase plus a single file write.
pub struct BootTimer {
    /// Set the instant `resumed()` begins -- the wall-clock origin for the
    /// "time to playable" total.
    pub boot_start: Instant,
    spans: Vec<(String, Duration)>,
    /// Guards `emit` so the summary fires exactly once even if `load_world`
    /// were ever re-entered.
    emitted: bool,
}

impl BootTimer {
    pub fn new(boot_start: Instant) -> Self {
        Self {
            boot_start,
            spans: Vec::new(),
            emitted: false,
        }
    }

    /// Record a pre-measured span under `name`.
    pub fn record(&mut self, name: impl Into<String>, dur: Duration) {
        self.spans.push((name.into(), dur));
    }

    /// Record `start.elapsed()` under `name` -- the common case (call it right
    /// after the phase's block closes).
    pub fn since(&mut self, name: impl Into<String>, start: Instant) {
        let d = start.elapsed();
        self.record(name, d);
    }

    /// Log a per-phase summary and drop `debug/boot_timing.json`. `total` is
    /// the wall-clock window-open -> world-ready duration. Idempotent.
    pub fn emit(&mut self, total: Duration) {
        if self.emitted {
            return;
        }
        self.emitted = true;
        let work_ms: f64 = self.spans.iter().map(|(_, d)| d.as_secs_f64() * 1000.0).sum();
        log::info!(
            "=== BOOT TIMING: {:.0} ms wall to playable ({:.0} ms measured work) ===",
            total.as_secs_f64() * 1000.0,
            work_ms,
        );
        for (name, d) in &self.spans {
            log::info!("  boot {:<22} {:>8.0} ms", name, d.as_secs_f64() * 1000.0);
        }
        let phases: Vec<serde_json::Value> = self
            .spans
            .iter()
            .map(|(n, d)| serde_json::json!({ "name": n, "ms": d.as_secs_f64() * 1000.0 }))
            .collect();
        let body = serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "total_ms": total.as_secs_f64() * 1000.0,
            "work_ms": work_ms,
            "phases": phases,
        });
        let _ = std::fs::create_dir_all("debug");
        let _ = std::fs::write("debug/boot_timing.json", body.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_spans_and_sums_work() {
        let mut bt = BootTimer::new(Instant::now());
        bt.record("a", Duration::from_millis(10));
        bt.record("b", Duration::from_millis(20));
        assert_eq!(bt.spans.len(), 2);
        let work: f64 = bt.spans.iter().map(|(_, d)| d.as_secs_f64() * 1000.0).sum();
        assert!((work - 30.0).abs() < 0.5, "work sum {work}");
    }

    #[test]
    fn emit_is_one_shot() {
        let mut bt = BootTimer::new(Instant::now());
        bt.record("x", Duration::from_millis(1));
        bt.emit(Duration::from_millis(5));
        assert!(bt.emitted);
        // Second emit is a no-op (no panic, guard holds).
        bt.emit(Duration::from_millis(5));
        assert!(bt.emitted);
    }
}
