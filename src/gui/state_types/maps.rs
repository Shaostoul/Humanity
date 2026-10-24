//! The Maps page's state: which view is open, the pan and zoom of the 2D
//! views, the Solar System view's camera and selection, the page's own clock,
//! the overlay switches, and the cached forward scan of sky events.
//!
//! These were twenty `cosmos_*` fields on `GuiState` (the page was
//! `gui/pages/cosmos.rs`, v0.203.0 onward) until Maps increment 0
//! (2026-10-20, docs/design/maps-zoom-to-anything.md) moved them here as one
//! `GuiState::maps`, under the page's own name. None of them is saved in
//! config.json, so nothing on disk changed name.
//!
//! Maps increment 1 (2026-10-21) added what the engine says about where you
//! are (`MapSelf`, written by `engine/map_feed.rs` every frame), the clock's
//! Live mode, the world layer the engine renders for the page
//! (`MapWorldLayer`, `engine/map_view.rs`), and the dev requests the rig sends
//! (`MapVerb`, `engine/ipc.rs`).

use glam::{DQuat, DVec3};

use crate::gui::pages::maps::{MapCamera, MapView, StationRow, UpcomingSkyEvent};

/// Where you are, as the engine computed it THIS frame (Maps increment 1,
/// `engine/map_feed.rs`). The page reads it and never recomputes it: the
/// ship's pose, the Sun and Earth's spin come from one source, the engine,
/// so the ship can never drift off the place it hangs over on the map while
/// the game has it somewhere else (docs/design/maps-zoom-to-anything.md,
/// section 8, "Two truths").
///
/// Every position is in the engine's world frame: metres, Earth's centre at
/// the origin, Y up, the ecliptic in XZ (the frame `crate::cosmos` and the
/// frame loop place every body in).
#[derive(Debug, Clone, PartialEq)]
pub struct MapSelf {
    /// The ship you live on: its station id (`data/stations/<id>.ron`) and
    /// its name ("UHS Mothership").
    pub ship_id: String,
    pub ship_name: String,
    /// The body it orbits ("earth") and that body's gravitational
    /// parameter, m^3/s^2, so its orbit can be drawn from its own state.
    pub parent: String,
    pub parent_mu: f64,
    /// The ship frame's origin: exactly `EngineState::station_world_pos`.
    pub ship_pos_m: DVec3,
    /// Its velocity, m/s, from the same propagation that placed it.
    pub ship_vel_mps: DVec3,
    /// Its attitude, body to world: exactly `EngineState::station_world_rot`
    /// (body +Y is up, away from Earth; body +Z along the track).
    pub ship_rot: DQuat,
    /// The ship's box in its own frame (`ShipStructure::aboard_bounds`, the
    /// zones, plots, corridors and districts grown by the aboard margin), or
    /// None before a ship has assembled.
    pub ship_box_m: Option<(DVec3, DVec3)>,
    /// Earth's turn this frame, radians (`EngineState::current_spin`).
    pub earth_spin_rad: f64,
    /// The Sun: exactly `EngineState::sun_world_pos`.
    pub sun_pos_m: DVec3,
    /// Riding the ship (`EngineState::station_ride`).
    pub aboard: bool,
    /// You, in the ship's own frame (its body axes, origin at the frame
    /// origin) and in the world frame.
    pub you_ship_m: DVec3,
    pub you_world_m: DVec3,
    /// The game clock: elapsed game seconds and the length of a game day.
    pub game_seconds: f64,
    pub game_day_s: f64,
    /// The sky clock the engine places the planets by, seconds since
    /// J2000.0 (the wall clock until increment 6 gives the sky one clock).
    pub sky_seconds: f64,
    /// Which level of the ship you are on, its ordinal (Maps increment 3, ship/levels.rs
    /// `level_at` of your feet, the rule the build editor finds your storey by), while aboard
    /// with the page open; None otherwise.
    pub level: Option<i32>,
    /// The room the HUD's room card names for you (`hud::room_card_at`, its id and the name it
    /// shows), so the map and the game never disagree about which room you are in.
    pub you_room: Option<(String, String)>,
    /// Which way you face, a unit vector in the ship's own frame (the camera's look).
    pub you_facing_ship: DVec3,
    /// Whether the 3D world is loaded (the world layer needs it).
    pub world_loaded: bool,
}

impl MapSelf {
    /// The centre of the ship's box in its own frame, or the frame origin
    /// before a ship has assembled.
    pub fn ship_centre_local(&self) -> DVec3 {
        self.ship_box_m.map(|(lo, hi)| (lo + hi) * 0.5).unwrap_or(DVec3::ZERO)
    }

    /// The centre of the ship in the world frame.
    pub fn ship_centre_m(&self) -> DVec3 {
        self.ship_pos_m + self.ship_rot * self.ship_centre_local()
    }

    /// The ship's longest side, metres (its box's), or 1 km before a ship
    /// has assembled.
    pub fn ship_length_m(&self) -> f64 {
        self.ship_box_m
            .map(|(lo, hi)| {
                let d = hi - lo;
                d.x.max(d.y).max(d.z)
            })
            .filter(|l| l.is_finite() && *l > 1.0)
            .unwrap_or(1_000.0)
    }

    /// A point given in the ship's own frame, in the world frame.
    pub fn ship_to_world(&self, local: DVec3) -> DVec3 {
        self.ship_pos_m + self.ship_rot * local
    }
}

/// What the engine rendered for the page's backdrop (Maps increment 1,
/// `engine/map_view.rs`): the real world from the map's camera, the ship
/// drawn from its own meshes. The page draws `texture` under its symbols.
#[derive(Debug, Clone, Default)]
pub struct MapWorldLayer {
    /// The texture, registered with the window's egui renderer, or None
    /// before the first render (and after the page closes).
    pub texture: Option<egui::TextureId>,
    /// Its size in pixels.
    pub size: [u32; 2],
    /// Rendered at half resolution because the camera was moving.
    pub half: bool,
    /// How many renders so far this opening of the page.
    pub renders: u64,
    /// Whether the last render drew the ship (the rig's centred check turns
    /// it off once to see which pixels are the ship).
    pub ship_drawn: bool,
    /// The frame (`MapsState::frame_seq`) the last render happened in.
    pub rendered_seq: u64,
    /// The last render was the star sky only (Maps increment 2: the
    /// solar-system band, the Diagram, away from the ship), not the world.
    pub sky_only: bool,
}

/// One dev request for the map, from the rig (`engine/ipc.rs` showcase
/// verbs `map`, `map_distance`, `map_ship`). Queued here and applied by the
/// feed after its page-open handling, so a request that opens the page and
/// stops following in one go is not undone by the opening framing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MapVerb {
    /// Open framing on you, following ("map":"home").
    pub home: bool,
    /// Follow on or off ("map":"follow" / "map":"unfollow").
    pub follow: Option<bool>,
    /// Camera distance from its anchor, metres ("map_distance").
    pub distance_m: Option<f64>,
    /// Draw the ship in the world layer or not ("map_ship":"1" / "0").
    pub ship: Option<bool>,
    /// Write `debug/map_done.json` once the request has been drawn
    /// ("map":"report").
    pub report: bool,
    /// Turn the camera round its anchor at this many degrees a second, the
    /// way a drag does ("map_turn"; 0 stops): the rig measures the world
    /// layer while the camera moves with it.
    pub turn_dps: Option<f64>,
    /// The solar-system overview: the camera on the Sun, looking down from
    /// ecliptic north ("map":"overview"; `map_distance` sets how far).
    pub overview: bool,
    /// Overlay layers switched on or off by id ("map_layer":"belts=0,names=1").
    pub layers: Vec<(String, bool)>,
    /// Diagram mode on or off ("map_diagram":"1" / "0").
    pub diagram: Option<bool>,
    /// The level shown (Maps increment 3, "map_level": an ordinal, "you" or "whole").
    pub level: Option<LevelVerb>,
    /// The straight-down plan on or off ("map_plan":"1" / "0").
    pub plan: Option<bool>,
}

/// Which level a rig asks the map to show (`MapVerb::level`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LevelVerb {
    /// The level with this ordinal.
    Ordinal(i32),
    /// The level you are on.
    You,
    /// The whole ship, no level.
    Whole,
}

/// What one draw of the Solar System view put on the map (Maps increment 2):
/// the rig's overview check reads it from `debug/map_done.json`, so a map
/// that lost its belts or its names fails there, not only in a picture.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MapSymbology {
    /// Bodies drawn (an icon, a disc or the Sun).
    pub bodies: u32,
    /// Orbit lines drawn, and the vertices of all of them.
    pub orbits: u32,
    pub orbit_vertices: u32,
    /// Names placed, and offered but dropped for want of a clear spot.
    pub labels_placed: u32,
    pub labels_dropped: u32,
    /// Belt dust points drawn this frame.
    pub belt_points: u32,
    /// The layer ids drawn this frame (on, and in their band).
    pub layers: Vec<String>,
    /// The band the camera is in ("Ship", "NearSpace", "SolarSystem").
    pub band: String,
    /// The time the view's symbols took to build, milliseconds.
    pub ms: f32,
}

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
    /// flies the camera to this body and anchors it there. Set by sidebar
    /// click or "Focus" button. v0.205.0 (operator pushback on zoom always
    /// centering on Sun); a fly-to since Maps increment 1.
    pub focus_request: Option<String>,
    /// Body id the camera continuously follows across frames as it moves
    /// along its orbit, or `None` for no auto-follow. Set by the body detail
    /// card's "Track"/"Stop Tracking" action; cleared automatically by any
    /// other Focus request (see `gui/pages/maps/system_view.rs`'s
    /// focus-consumption site).
    pub tracked_body: Option<String>,
    /// The System view's camera (Maps increment 1, `gui/pages/maps/camera.rs`):
    /// an anchor (you, a body, a point) plus an f64 offset, 10 m to 1e14 m.
    pub camera: MapCamera,

    /// The page's sim-time in seconds since the J2000.0 epoch
    /// (2000-01-01 12:00:00 UTC). Drives Kepler-evolved body positions.
    /// While `live` it is the engine's own sky clock (`MapSelf::sky_seconds`),
    /// set every frame by the feed; scrubbing detaches it. v0.208.0.
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
    /// The clock is LIVE: the map shows the ship and the planets exactly
    /// where the game has them, on the game's own sky clock (Maps increment
    /// 1, docs/design/maps-zoom-to-anything.md 5.9). On by default; any
    /// scrub, skip or speed change detaches it, and "Live" puts it back.
    /// The world layer is drawn only while live (until increment 6 gives
    /// the engine one sky clock the map can move).
    pub live: bool,
    /// Which body's pill is currently expanded into an info card on the
    /// 3D System view. Only one body can be expanded at a time — clicking
    /// a different body's pill swaps the expansion; clicking the same
    /// pill collapses it. Independent of `selected_body` which
    /// drives the right-side details panel. v0.209.0.
    pub expanded_body: Option<String>,
    /// The overlay switches a person has flipped, by layer id (Maps
    /// increment 2: every overlay is a row of `data/maps/layers.ron`, the
    /// Lagrange points and the reference orbits among them, which were two
    /// `show_*` fields until then). A layer not in here is at its default.
    pub layers_on: std::collections::HashMap<String, bool>,
    /// Diagram mode (decision 5, Maps increment 2): log-spaced orbits and
    /// enlarged bodies, labelled not to scale; true scale is the default.
    pub diagram: bool,
    /// What the last draw of the Solar System view put on the map, for the
    /// rig's report (`debug/map_done.json`).
    pub symbology: MapSymbology,
    /// The belts' dust where the last propagation put it (`maps/belts.rs`).
    pub belts: crate::gui::pages::maps::BeltCache,
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

    // ── Maps increment 1 ──
    /// What the engine says about where you are this frame, or None when
    /// nothing has said (a snapshot, the page drawn with no engine).
    pub me: Option<MapSelf>,
    /// The page was open last frame (the feed's page-open detector).
    pub was_open: bool,
    /// Counts the feed's frames; the world layer and the rig's report use
    /// it to tell "this frame" from "an earlier one".
    pub frame_seq: u64,
    /// The frame the page last drew in (`frame_seq` at the time).
    pub drawn_seq: u64,
    /// The world layer the engine rendered for the page.
    pub world: MapWorldLayer,
    /// Draw the ship in the world layer (a dev switch for the rig's
    /// "ship centred" check; always on for people).
    pub draw_ship: bool,
    /// The System view's canvas on the window, physical pixels
    /// `[left, top, right, bottom]`, as last drawn.
    pub canvas_px: Option<[f32; 4]>,
    /// The canvas's logical size, for the engine's world-layer aspect.
    pub canvas_points: Option<[f32; 2]>,
    /// The window's pixels per point when the canvas was last drawn.
    pub pixels_per_point: f32,
    /// Where the ship's centre landed on the canvas at the last draw,
    /// physical pixels (None off-screen or with no feed).
    pub ship_px: Option<[f32; 2]>,
    /// Dev requests waiting for the feed (`MapVerb`).
    pub pending: Vec<MapVerb>,
    /// A dev turn of the camera, degrees a second (`MapVerb::turn_dps`).
    pub turn_dps: f64,
    /// A report the rig asked for: written once a draw at or after this
    /// frame has happened (and, with the world layer up, a render).
    pub report_after_seq: Option<u64>,
    /// Every `data/stations/*.ron`, with its computed altitude, for the
    /// body list's Stations rows (loaded on first use).
    pub stations: Option<Vec<StationRow>>,

    // ── Maps increment 3: into the ship, deck by deck ──
    /// The level a person picked on the level picker (its ordinal, ship/levels.rs), shown cut
    /// open; None is the whole ship.
    pub level_pick: Option<i32>,
    /// The straight-down plan of the level shown (your level when none is picked), drawn from
    /// baked plan tiles (engine/map_tiles.rs).
    pub plan: bool,
    /// The plan tiles: the ones the page wants this frame and the ones the engine has baked.
    pub tiles: MapTiles,
    /// What the ship view drew last frame, for the rig's report.
    pub ship_drawn: MapShipDrawn,
}

/// The plan tiles between the page and the engine (Maps increment 3, engine/map_tiles.rs): the
/// page says which it wants, the engine bakes the missing ones (one a frame) and says which it
/// has.
#[derive(Debug, Clone, Default)]
pub struct MapTiles {
    /// The tiles the plan's view covers this frame, finest first.
    pub want: Vec<crate::engine::map_tiles::TileKey>,
    /// The tiles baked and kept, each with its texture as registered with the window's egui.
    pub have: std::collections::HashMap<crate::engine::map_tiles::TileKey, egui::TextureId>,
    /// What the engine did: tiles baked this opening of the page, the last one's cost, the bytes
    /// kept, and the tiles an edit made it drop (each with the reason's footprint).
    pub baked: u32,
    pub last_bake_ms: f32,
    pub bytes: usize,
    pub dropped: Vec<crate::engine::map_tiles::TileKey>,
    pub dirty: Vec<[f32; 4]>,
}

/// What the ship view put on the map in its last draw (Maps increment 3), for the rig's report.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MapShipDrawn {
    /// The ship's levels as the picker lists them (top to bottom), and the one shown.
    pub levels: Vec<String>,
    pub level: Option<String>,
    /// The rooms named on the level shown, by the names the HUD gives them.
    pub rooms_named: Vec<String>,
    /// Rooms filled on the level shown.
    pub rooms_filled: u32,
    /// Whether your dot was drawn (you are on the level shown).
    pub you_drawn: bool,
}

impl MapsState {
    /// The scene this frame draws: the feed, the page's clock and Diagram,
    /// one builder so the camera, the page and the world layer never
    /// disagree about where things are.
    pub fn scene(&self) -> crate::gui::pages::maps::camera::MapScene<'_> {
        crate::gui::pages::maps::camera::MapScene::new(self.me.as_ref(), self.sim_time_seconds, self.live).with_diagram(self.diagram)
    }

    /// Whether an overlay layer is switched on (its row's default until a
    /// person flips it).
    pub fn layer_on(&self, id: &str) -> bool {
        self.layers_on.get(id).copied().unwrap_or_else(|| crate::gui::pages::maps::overlays::layer_default(id))
    }
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
            sim_speed: 0.0, // Paused when detached; Live runs on the engine's clock.
            last_real_instant: None,
            sim_time_initialized: false,
            live: true,
            upcoming_events: Vec::new(),
            upcoming_scan_origin: f64::NAN, // forces first scan
            upcoming_last_scan: None,
            expanded_body: None,
            layers_on: std::collections::HashMap::new(),
            diagram: false,
            symbology: MapSymbology::default(),
            belts: Default::default(),
            me: None,
            was_open: false,
            frame_seq: 0,
            drawn_seq: 0,
            world: MapWorldLayer::default(),
            draw_ship: true,
            canvas_px: None,
            canvas_points: None,
            pixels_per_point: 1.0,
            ship_px: None,
            pending: Vec::new(),
            turn_dps: 0.0,
            report_after_seq: None,
            stations: None,
            level_pick: None,
            plan: false,
            tiles: MapTiles::default(),
            ship_drawn: MapShipDrawn::default(),
        }
    }
}
