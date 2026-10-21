//! The Maps page's state: which view is open, the pan and zoom of the 2D
//! views, the Solar System view's camera and selection, the page's own clock,
//! the overlay switches, and the cached forward scan of sky events.
//!
//! These were twenty `cosmos_*` fields on `GuiState` (the page was
//! `gui/pages/cosmos.rs`, v0.203.0 onward) until Maps increment 0
//! (2026-10-20, docs/design/maps-zoom-to-anything.md) moved them here as one
//! `GuiState::maps`, under the page's own name. None of them is saved in
//! config.json, so nothing on disk changed name.

use crate::gui::pages::maps::{MapCamera, MapView, UpcomingSkyEvent};

/// Everything the Maps page keeps between frames (`GuiState::maps`).
pub struct MapsState {
    /// Which view the page is currently rendering.
    pub view: MapView,
    /// Pan offset (screen pixels). Updated by click-drag on the canvas.
    pub pan: egui::Vec2,
    /// Zoom factor — 1.0 = default, > 1.0 = zoomed in, < 1.0 = zoomed out.
    /// Updated by mouse wheel scroll. Clamped in allocate_canvas.
    pub zoom: f32,
    /// Currently selected body id in the System view (for the right-side
    /// details panel + map highlight). v0.203.2 — populated by clicking
    /// a body in the left-side browser sidebar OR clicking it on the map.
    pub selected_body: Option<String>,
    /// Which planet groups are expanded in the body browser sidebar.
    /// Stored as ids of planets whose moon list is expanded. v0.203.2.
    pub expanded_planets: std::collections::HashSet<String>,
    /// Pending focus request — when Some, the next System view render
    /// computes the pan needed to center this body on screen, then
    /// clears the request. Set by sidebar click or "Focus" button.
    /// v0.205.0 (operator pushback on zoom always centering on Sun).
    pub focus_request: Option<String>,
    /// Body id the camera continuously follows across frames as it moves
    /// along its orbit, or `None` for no auto-follow. Unlike
    /// `focus_request` (a one-shot snap-to), this re-centers every
    /// frame. Set by the body detail card's "Track"/"Stop Tracking" action;
    /// cleared automatically by any other Focus request (see
    /// `gui/pages/maps/system_view.rs`'s focus-consumption site).
    pub tracked_body: Option<String>,
    /// 3D camera state for System view (Phase 4, v0.206.0).
    /// Yaw + pitch + distance + look-at target define the camera; mouse
    /// drag rotates, scroll zooms, sidebar click re-centers the target.
    pub camera: MapCamera,

    /// The page's sim-time in seconds since the J2000.0 epoch
    /// (2000-01-01 12:00:00 UTC). Drives Kepler-evolved body positions.
    /// Initialized to "current real-world time" on the first open of the
    /// page so the user immediately sees today's planetary configuration.
    /// v0.208.0.
    pub sim_time_seconds: f64,
    /// Sim speed multiplier — 0 = paused, 1 = real-time (1 second sim =
    /// 1 second real), 86400 = 1 day per real second, etc. Negative
    /// values rewind. v0.208.0.
    pub sim_speed: f64,
    /// Wall-clock Instant of the previous frame, used to compute dt for
    /// sim_time advancement. None on first frame. v0.208.0.
    #[allow(clippy::type_complexity)]
    pub last_real_instant: Option<std::time::Instant>,
    /// Whether the page's sim_time has been initialized (sets to "now"
    /// on the first draw of the page). v0.208.0.
    pub sim_time_initialized: bool,
    /// Which body's pill is currently expanded into an info card on the
    /// 3D System view. Only one body can be expanded at a time — clicking
    /// a different body's pill swaps the expansion; clicking the same
    /// pill collapses it. Independent of `selected_body` which
    /// drives the right-side details panel. v0.209.0.
    pub expanded_body: Option<String>,
    /// Whether to render Lagrange-point overlay markers (L1-L5 for
    /// Sun-Earth, Earth-Moon, Sun-Mars, Sun-Jupiter, Sun-Saturn pairs).
    /// Off by default to keep the wide view clean. Toggled from the
    /// System view's overlay button. v0.211.0.
    pub show_lagrange: bool,
    /// Whether to render reference-orbit rings (LEO/MEO/GEO/etc) around
    /// supported planets when zoomed close enough. Off by default —
    /// rings only appear when the user explicitly enables AND the camera
    /// is close enough that they're not microscopic on screen. v0.212.0.
    pub show_reference_orbits: bool,
    /// Cached forward Sky-Events scan (Phase 4d-quad, v0.248). The scan
    /// is O(days × bodies²) so it must NOT run every frame like the
    /// instant detector — it's recomputed lazily when sim_time drifts
    /// far from `upcoming_scan_origin`, throttled by
    /// `upcoming_last_scan`. Each entry: when (sim seconds since
    /// J2000), human label, severity (0 info / 1 notable / 2 major).
    pub upcoming_events: Vec<UpcomingSkyEvent>,
    /// sim_time the cached forward scan was computed at. Recompute when
    /// the live sim_time moves more than ~12h away from this.
    pub upcoming_scan_origin: f64,
    /// Wall-clock instant of the last forward scan — throttles recompute
    /// so fast-forward / scrubbing can't trigger a scan every frame.
    pub upcoming_last_scan: Option<std::time::Instant>,
}

impl Default for MapsState {
    fn default() -> Self {
        Self {
            view: MapView::System,
            pan: egui::Vec2::ZERO,
            zoom: 1.0,
            selected_body: None,
            expanded_planets: std::collections::HashSet::new(),
            focus_request: None,
            tracked_body: None,
            camera: MapCamera::default(),
            sim_time_seconds: 0.0,
            sim_speed: 0.0, // Paused by default — operator scrubs / plays.
            last_real_instant: None,
            sim_time_initialized: false,
            upcoming_events: Vec::new(),
            upcoming_scan_origin: f64::NAN, // forces first scan
            upcoming_last_scan: None,
            expanded_body: None,
            show_lagrange: false,
            show_reference_orbits: false,
        }
    }
}
