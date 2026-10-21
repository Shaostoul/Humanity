//! The Maps page's camera (Maps increment 1, docs/design/maps-zoom-to-anything.md
//! section 5.1): an ANCHOR (you, a body, or a fixed point) and the camera's
//! offset from it, a zoom from 10 m to 1e14 m, and the frame its yaw and pitch
//! are measured in.
//!
//! Why an anchor and an f64 offset: the map goes from a 10 m view of your hull
//! to a 1e14 m view of the whole Sun's family in one zoom. A camera that kept
//! one absolute position would need sub-metre precision at a hundred AU, which
//! no float has once it is narrowed for drawing. So every position here is
//! f64, in metres, and the projection subtracts the eye from a point BEFORE it
//! does anything else (the dynamic reference frame of Axelsson et al., 2017):
//! the ship 10 m away projects to the same pixel whether it is 42,000 km or
//! 1 AU from the origin (`camera_tests.rs`, the f64-stability test).
//!
//! The frame is the ENGINE's world frame: metres, Earth's centre at the
//! origin, Y up, the ecliptic in XZ, which is how `crate::cosmos` and the
//! frame loop place every body. The ship's pose, the Sun and Earth's spin come
//! from the engine through `MapSelf` (`engine/map_feed.rs`) and are never
//! recomputed here ("Two truths", section 8 of the design).
//!
//! Which way is up (decision 3, accepted 2026-10-20): at ship scale the
//! camera turns with the ship's own frame, so Earth stays below the ship the
//! way the residents see it from the deck; by near space it has blended to
//! the fixed stars (`ship_frame_weight`).
//!
//! Follow (decision 2): anchored on you, the camera keeps its offset from you
//! however the ship moves. Rotating keeps follow; panning or picking another
//! body ends it; "Back to me" (and the Home key) flies back and follows again.
//! Letting go keeps the view exactly where it was: the anchor becomes a fixed
//! point and the frame is held at the orientation it had (`MapFrame::Held`).
//!
//! Fly-to: the zoom-out, travel, zoom-in path of van Wijk and Nuij ("Smooth
//! and efficient zooming and panning", 2003), the one Mapbox's flyTo cites,
//! cut short the moment the person grabs the camera (Reach and North, 2018,
//! made such paths interruptible).

use glam::{DQuat, DVec3};

use crate::cosmos::{body_world_position_3d_au, find_body, M_PER_AU};
use crate::gui::state_types::MapSelf;

/// The closest the camera comes to its anchor, metres.
pub const MIN_DISTANCE_M: f64 = 10.0;
/// The farthest, metres (about 670 AU: the whole Sun's family and its belts).
pub const MAX_DISTANCE_M: f64 = 1.0e14;
/// The map's vertical field of view, radians (the 60 degrees it has always had).
pub const FOV_Y_RAD: f64 = std::f64::consts::FRAC_PI_3;
/// How far up or down the camera may look, radians: just inside straight down
/// or up, where yaw stops meaning anything.
pub const PITCH_LIMIT_RAD: f64 = 89.0 * std::f64::consts::PI / 180.0;
/// Up to this distance the camera turns with the ship's own frame entirely.
pub const SHIP_FRAME_FULL_M: f64 = 60_000.0;
/// From this distance on the camera holds the fixed stars entirely.
pub const STARS_FRAME_FULL_M: f64 = 2_000_000.0;
/// The world layer (the ship from its real meshes) is drawn up to this far
/// from the anchor: the "Ship" band of the design's table (10 m to ~60 km).
/// Beyond it the ship is an icon on its orbit line.
pub const WORLD_LAYER_MAX_M: f64 = 60_000.0;
/// The opening view: this many ship lengths from the ship's centre.
pub const OPEN_SHIP_LENGTHS: f64 = 1.5;
/// The opening view's yaw and pitch in the ship's frame: looking down past
/// the hull at Earth (a pitch below about -60 degrees keeps Earth's disc,
/// straight below the ship, inside the 60 degree view).
pub const OPEN_YAW_RAD: f64 = 20.0 * std::f64::consts::PI / 180.0;
pub const OPEN_PITCH_RAD: f64 = -68.0 * std::f64::consts::PI / 180.0;
/// The overview the page opens on when nothing says where you are.
pub const OVERVIEW_DISTANCE_M: f64 = 60.0 * M_PER_AU;
pub const OVERVIEW_PITCH_RAD: f64 = -70.0 * std::f64::consts::PI / 180.0;
/// van Wijk and Nuij's rho: the trade between zooming and panning. 1.42 is
/// the value their user study preferred (and Mapbox's default curve).
const FLY_RHO: f64 = 1.42;
/// Fly-to speed, in the path's own units per second, and its time limits.
const FLY_SPEED: f64 = 3.0;
const FLY_MIN_S: f64 = 0.6;
const FLY_MAX_S: f64 = 3.0;

/// What the camera is anchored to.
#[derive(Debug, Clone, PartialEq)]
pub enum MapAnchor {
    /// You: your ship while you are aboard it (its centre), else wherever you are.
    Me,
    /// A body of the Sun's family, by its `sol.json` id.
    Body(String),
    /// A fixed point: `offset_m` from a body's centre, in world axes. What a
    /// pan, a stop-following or a grabbed flight leaves the camera on.
    Point { body: String, offset_m: DVec3 },
}

/// The frame the camera's yaw and pitch are measured in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MapFrame {
    /// The ship's own frame at ship scale, the fixed stars by near space
    /// (decision 3), blended between by `ship_frame_weight`.
    Auto,
    /// Held at this orientation: what the view had when it stopped following,
    /// so letting go never turns it.
    Held(DQuat),
}

/// A flight in progress (see the module doc).
#[derive(Debug, Clone, PartialEq)]
pub struct FlyTo {
    pub to: MapAnchor,
    pub to_distance: f64,
    pub to_yaw: f64,
    pub to_pitch: f64,
    /// Where the camera looked and how it was turned when the flight began.
    pub from_look: DVec3,
    pub from_distance: f64,
    pub from_rot: DQuat,
    pub elapsed: f64,
    pub duration: f64,
}

/// The Maps page's camera.
#[derive(Debug, Clone, PartialEq)]
pub struct MapCamera {
    pub anchor: MapAnchor,
    /// ln(distance from the anchor, metres): zoom is multiplicative, so the
    /// distance is kept as its logarithm.
    pub log_distance: f64,
    /// Look direction in the camera's frame: yaw about the frame's up (0
    /// looks along -Z, positive turns toward +X) and pitch above (+) or
    /// below (-) the frame's horizontal. The game camera's own convention.
    pub yaw: f64,
    pub pitch: f64,
    pub frame: MapFrame,
    pub fly: Option<FlyTo>,
}

impl Default for MapCamera {
    /// The whole Sun's family from above, the view the page opens on when
    /// nothing says where you are.
    fn default() -> Self {
        Self {
            anchor: MapAnchor::Body("sun".to_string()),
            log_distance: OVERVIEW_DISTANCE_M.ln(),
            yaw: 0.0,
            pitch: OVERVIEW_PITCH_RAD,
            frame: MapFrame::Auto,
            fly: None,
        }
    }
}

/// Where everything is, for one draw: the engine's feed (if any) and the
/// page's clock.
#[derive(Debug, Clone, Copy)]
pub struct MapScene<'a> {
    pub me: Option<&'a MapSelf>,
    /// Seconds since J2000.0 the bodies are placed at.
    pub sim_time: f64,
    /// The clock is on the engine's own sky clock (the Sun then comes from
    /// the engine, exactly; off it, from the page's clock).
    pub live: bool,
}

impl<'a> MapScene<'a> {
    pub fn new(me: Option<&'a MapSelf>, sim_time: f64, live: bool) -> Self {
        Self { me, sim_time, live }
    }

    /// Earth's place around the Sun, AU (`cosmos`), at the scene's time.
    pub fn earth_helio_au(&self) -> DVec3 {
        find_body("earth").map(|e| body_world_position_3d_au(e, self.sim_time)).unwrap_or(DVec3::ZERO)
    }

    /// A heliocentric AU position (`cosmos`) in the map's frame: metres from
    /// Earth's centre.
    pub fn au_to_m(&self, p_au: DVec3) -> DVec3 {
        (p_au - self.earth_helio_au()) * M_PER_AU
    }

    /// A body's centre in the map's frame. The Sun comes from the engine
    /// while live (`MapSelf::sun_pos_m`), exactly as the game placed it.
    pub fn body_m(&self, id: &str) -> Option<DVec3> {
        if id == "earth" {
            return Some(DVec3::ZERO);
        }
        if id == "sun" && self.live {
            // Not before the world has placed it: until then the engine's
            // sun sits at the origin, which is Earth's centre.
            if let Some(me) = self.me.filter(|m| m.sun_pos_m.length() > 1.0e9) {
                return Some(me.sun_pos_m);
            }
        }
        find_body(id).map(|b| self.au_to_m(body_world_position_3d_au(b, self.sim_time)))
    }

    /// Where "you" are for the camera: your ship's centre while aboard it,
    /// else your own position. None with no feed.
    pub fn me_m(&self) -> Option<DVec3> {
        self.me.map(|m| if m.aboard { m.ship_centre_m() } else { m.you_world_m })
    }

    /// The anchor's position, if it can be found.
    pub fn anchor_m(&self, anchor: &MapAnchor) -> Option<DVec3> {
        match anchor {
            MapAnchor::Me => self.me_m(),
            MapAnchor::Body(id) => self.body_m(id),
            MapAnchor::Point { body, offset_m } => self.body_m(body).map(|b| b + *offset_m),
        }
    }
}

/// How much of the ship's own frame the camera turns with at `distance`
/// metres: 1 up to `SHIP_FRAME_FULL_M`, 0 from `STARS_FRAME_FULL_M`, a
/// smoothstep in log distance between, so zooming out turns the view to the
/// stars gradually instead of with a jump.
pub fn ship_frame_weight(distance: f64) -> f64 {
    let (a, b) = (SHIP_FRAME_FULL_M.ln(), STARS_FRAME_FULL_M.ln());
    let t = ((b - distance.max(1.0).ln()) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The rotation for a look direction of (`yaw`, `pitch`) in a frame: camera
/// space (looking along -Z, up +Y) to the frame's axes. `forward` is
/// `(sin yaw cos pitch, sin pitch, -cos yaw cos pitch)`, the game camera's.
pub fn yaw_pitch_rot(yaw: f64, pitch: f64) -> DQuat {
    DQuat::from_rotation_y(-yaw) * DQuat::from_rotation_x(pitch)
}

/// The camera's pose for one draw: where it looks from and how it is turned,
/// in the world frame, f64 throughout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapPose {
    pub look_at: DVec3,
    pub eye: DVec3,
    pub forward: DVec3,
    pub right: DVec3,
    pub up: DVec3,
    pub distance: f64,
    pub fov_y: f64,
}

impl MapPose {
    fn from_rot(look_at: DVec3, rot: DQuat, distance: f64) -> Self {
        let forward = (rot * DVec3::NEG_Z).normalize();
        let up = (rot * DVec3::Y).normalize();
        let right = forward.cross(up).normalize();
        Self { look_at, eye: look_at - forward * distance, forward, right, up, distance, fov_y: FOV_Y_RAD }
    }
}

impl MapCamera {
    /// Distance from the anchor, metres.
    pub fn distance(&self) -> f64 {
        self.log_distance.exp()
    }

    pub fn set_distance(&mut self, metres: f64) {
        self.log_distance = metres.clamp(MIN_DISTANCE_M, MAX_DISTANCE_M).ln();
    }

    /// Whether the camera follows something that moves (you or a body)
    /// rather than sitting on a fixed point.
    pub fn follows(&self) -> bool {
        !matches!(self.anchor, MapAnchor::Point { .. })
    }

    /// Whether it follows YOU.
    pub fn follows_me(&self) -> bool {
        self.anchor == MapAnchor::Me && self.fly.is_none()
    }

    /// The frame's rotation (frame axes to world) at `distance` for `anchor`.
    fn frame_rot_for(&self, anchor: &MapAnchor, distance: f64, scene: &MapScene) -> DQuat {
        match self.frame {
            MapFrame::Held(q) => q,
            MapFrame::Auto => match (anchor, scene.me) {
                (MapAnchor::Me, Some(me)) if me.aboard => {
                    DQuat::IDENTITY.slerp(me.ship_rot, ship_frame_weight(distance)).normalize()
                }
                _ => DQuat::IDENTITY,
            },
        }
    }

    /// The camera's orientation (camera space to world) when settled on
    /// `anchor` at `distance` with this yaw and pitch.
    fn settled_rot(&self, anchor: &MapAnchor, distance: f64, yaw: f64, pitch: f64, scene: &MapScene) -> DQuat {
        (self.frame_rot_for(anchor, distance, scene) * yaw_pitch_rot(yaw, pitch)).normalize()
    }

    /// The pose for this draw. An anchor that cannot be found (you, with no
    /// feed) is read as the Sun, so the map never loses itself.
    pub fn pose(&self, scene: &MapScene) -> MapPose {
        if let Some(fly) = &self.fly {
            return fly_pose(self, fly, scene);
        }
        let d = self.distance();
        let look = scene.anchor_m(&self.anchor).or_else(|| scene.body_m("sun")).unwrap_or(DVec3::ZERO);
        MapPose::from_rot(look, self.settled_rot(&self.anchor, d, self.yaw, self.pitch, scene), d)
    }

    // ── What the person does ──

    /// Open on you, following, framing the whole hull with Earth behind it
    /// (decision 2): what opening the page from the game does. With no feed,
    /// the overview.
    pub fn open_on_me(&mut self, me: Option<&MapSelf>) {
        match me {
            Some(me) => {
                let d = if me.aboard { me.ship_length_m() * OPEN_SHIP_LENGTHS } else { 50.0 };
                *self = Self {
                    anchor: MapAnchor::Me,
                    log_distance: d.clamp(MIN_DISTANCE_M, MAX_DISTANCE_M).ln(),
                    yaw: OPEN_YAW_RAD,
                    pitch: OPEN_PITCH_RAD,
                    frame: MapFrame::Auto,
                    fly: None,
                };
            }
            None => *self = Self::default(),
        }
    }

    /// "Back to me" and the Home key: fly back to you, following, at the
    /// opening framing.
    pub fn back_to_me(&mut self, scene: &MapScene) {
        let Some(me) = scene.me else {
            *self = Self::default();
            return;
        };
        let mut target = self.clone();
        target.open_on_me(Some(me));
        self.fly_to(MapAnchor::Me, target.distance(), target.yaw, target.pitch, scene);
    }

    /// Fly to a body and stay with it.
    pub fn focus_body(&mut self, id: &str, distance: f64, scene: &MapScene) {
        self.fly_to(MapAnchor::Body(id.to_string()), distance, self.yaw, self.pitch.min(-0.35), scene);
    }

    /// Stop following: the anchor becomes the point the camera looks at, and
    /// the frame is held where it is, so nothing on screen moves.
    pub fn unfollow(&mut self, scene: &MapScene) {
        let pose = self.pose(scene);
        let rot = self.current_rot(scene);
        self.fly = None;
        self.anchor = point_anchor(pose.look_at, scene);
        self.frame = MapFrame::Held((rot * yaw_pitch_rot(self.yaw, self.pitch).inverse()).normalize());
        self.log_distance = pose.distance.clamp(MIN_DISTANCE_M, MAX_DISTANCE_M).ln();
    }

    /// Zoom by `factor` (under 1 comes closer), toward the anchor.
    pub fn zoom_by(&mut self, factor: f64, scene: &MapScene) {
        self.grab(scene);
        self.log_distance = (self.log_distance + factor.max(1e-9).ln())
            .clamp(MIN_DISTANCE_M.ln(), MAX_DISTANCE_M.ln());
    }

    /// Turn around the anchor (a drag): keeps following.
    pub fn rotate_by(&mut self, d_yaw: f64, d_pitch: f64, scene: &MapScene) {
        self.grab(scene);
        self.yaw = (self.yaw + d_yaw).rem_euclid(std::f64::consts::TAU);
        self.pitch = (self.pitch + d_pitch).clamp(-PITCH_LIMIT_RAD, PITCH_LIMIT_RAD);
    }

    /// Slide the look point across the view by (`dx`, `dy`) pixels of a view
    /// `view_h_px` tall (a shift or middle drag): ends following.
    pub fn pan_by(&mut self, dx: f64, dy: f64, view_h_px: f64, scene: &MapScene) {
        self.unfollow(scene);
        let pose = self.pose(scene);
        let m_per_px = 2.0 * pose.distance * (pose.fov_y * 0.5).tan() / view_h_px.max(1.0);
        let shift = (-pose.right * dx + pose.up * dy) * m_per_px;
        if let MapAnchor::Point { offset_m, .. } = &mut self.anchor {
            *offset_m += shift;
        }
    }

    /// Advance a flight by `dt` seconds, landing it when it is done.
    pub fn tick(&mut self, dt: f64) {
        let Some(fly) = self.fly.as_mut() else { return };
        fly.elapsed += dt.max(0.0);
        if fly.elapsed >= fly.duration {
            let fly = self.fly.take().expect("checked above");
            self.anchor = fly.to;
            self.log_distance = fly.to_distance.clamp(MIN_DISTANCE_M, MAX_DISTANCE_M).ln();
            self.yaw = fly.to_yaw;
            self.pitch = fly.to_pitch;
            self.frame = MapFrame::Auto;
        }
    }

    /// The person took hold of the camera mid-flight: stop exactly where it
    /// is (Reach and North's interruptible path), on a fixed point.
    pub fn grab(&mut self, scene: &MapScene) {
        if self.fly.is_none() {
            return;
        }
        let pose = self.pose(scene);
        let rot = self.current_rot(scene);
        self.fly = None;
        self.anchor = point_anchor(pose.look_at, scene);
        self.log_distance = pose.distance.clamp(MIN_DISTANCE_M, MAX_DISTANCE_M).ln();
        self.yaw = 0.0;
        self.pitch = 0.0;
        self.frame = MapFrame::Held(rot);
    }

    /// The camera's current orientation, camera space to world.
    fn current_rot(&self, scene: &MapScene) -> DQuat {
        let pose = self.pose(scene);
        DQuat::from_mat3(&glam::DMat3::from_cols(pose.right, pose.up, -pose.forward)).normalize()
    }

    /// Start a flight to `to`, ending `distance` from it at this yaw and pitch.
    pub fn fly_to(&mut self, to: MapAnchor, distance: f64, yaw: f64, pitch: f64, scene: &MapScene) {
        let pose = self.pose(scene);
        let from_rot = self.current_rot(scene);
        let to_look = scene.anchor_m(&to).unwrap_or(pose.look_at);
        let to_distance = distance.clamp(MIN_DISTANCE_M, MAX_DISTANCE_M);
        let w0 = view_width(pose.distance);
        let w1 = view_width(to_distance);
        let s = fly_path(w0, w1, (to_look - pose.look_at).length()).length;
        self.fly = Some(FlyTo {
            to,
            to_distance,
            to_yaw: yaw,
            to_pitch: pitch.clamp(-PITCH_LIMIT_RAD, PITCH_LIMIT_RAD),
            from_look: pose.look_at,
            from_distance: pose.distance,
            from_rot,
            elapsed: 0.0,
            duration: (s / FLY_SPEED).clamp(FLY_MIN_S, FLY_MAX_S),
        });
        // From an anchor that turned with the ship, the flight's settled end
        // is measured in the frame it lands in.
        self.frame = MapFrame::Auto;
    }
}

/// A fixed point at `p`, kept against the body it is nearest in the sense
/// that matters for a map: Earth inside Earth's sphere of influence (about
/// 925,000 km), else the Sun. A point near the ship then stays put relative
/// to Earth while Earth goes round the Sun.
fn point_anchor(p: DVec3, scene: &MapScene) -> MapAnchor {
    const EARTH_SOI_M: f64 = 9.25e8;
    let body = if p.length() < EARTH_SOI_M { "earth" } else { "sun" };
    let base = scene.body_m(body).unwrap_or(DVec3::ZERO);
    MapAnchor::Point { body: body.to_string(), offset_m: p - base }
}

/// The width of the view at `distance`, metres: the path's own unit.
fn view_width(distance: f64) -> f64 {
    2.0 * distance * (FOV_Y_RAD * 0.5).tan()
}

/// One van Wijk and Nuij path: from a view `w0` wide to one `w1` wide, `u1`
/// apart. `at(s)` gives (fraction of the way across, view width) at path
/// length `s` from 0 to `length`.
#[derive(Debug, Clone, Copy)]
pub struct FlyPath {
    w0: f64,
    w1: f64,
    u1: f64,
    r0: f64,
    pub length: f64,
    /// The two ends are (nearly) the same place: a pure zoom.
    zoom_only: bool,
}

/// Build the path (van Wijk and Nuij 2003, section 3). `r` uses
/// `-asinh(b)`, which equals the paper's `ln(-b + sqrt(b^2 + 1))` without its
/// cancellation when `b` is large (a 1e12 m flight from a 1 km view).
pub fn fly_path(w0: f64, w1: f64, u1: f64) -> FlyPath {
    let rho = FLY_RHO;
    let (w0, w1) = (w0.max(1e-9), w1.max(1e-9));
    if u1 < 1e-9 * w0.max(w1) {
        return FlyPath { w0, w1, u1, r0: 0.0, length: (w1 / w0).ln().abs() / rho, zoom_only: true };
    }
    let rho2 = rho * rho;
    let b0 = (w1 * w1 - w0 * w0 + rho2 * rho2 * u1 * u1) / (2.0 * w0 * rho2 * u1);
    let b1 = (w1 * w1 - w0 * w0 - rho2 * rho2 * u1 * u1) / (2.0 * w1 * rho2 * u1);
    let r0 = -b0.asinh();
    let r1 = -b1.asinh();
    FlyPath { w0, w1, u1, r0, length: (r1 - r0) / rho, zoom_only: false }
}

impl FlyPath {
    /// (fraction of the way across in 0..=1, view width) at path length `s`.
    pub fn at(&self, s: f64) -> (f64, f64) {
        let rho = FLY_RHO;
        let s = s.clamp(0.0, self.length);
        if self.zoom_only {
            // No way to go across, only in or out: the fraction just tracks progress.
            let k = if self.w1 >= self.w0 { 1.0 } else { -1.0 };
            let along = if self.length > 0.0 { s / self.length } else { 1.0 };
            return (along, self.w0 * (k * rho * s).exp());
        }
        let c0 = self.r0.cosh();
        let u = self.w0 / (rho * rho) * c0 * ((rho * s + self.r0).tanh() - self.r0.tanh());
        let w = self.w0 * c0 / (rho * s + self.r0).cosh();
        ((u / self.u1).clamp(0.0, 1.0), w)
    }
}

/// The pose part way through a flight. The far end is read live, so a
/// flight to the ship lands on the ship even though it moved meanwhile.
fn fly_pose(cam: &MapCamera, fly: &FlyTo, scene: &MapScene) -> MapPose {
    let to_look = scene.anchor_m(&fly.to).unwrap_or(fly.from_look);
    let path = fly_path(view_width(fly.from_distance), view_width(fly.to_distance), (to_look - fly.from_look).length());
    let t = (fly.elapsed / fly.duration.max(1e-6)).clamp(0.0, 1.0);
    let (u, w) = path.at(t * path.length);
    let look = fly.from_look + (to_look - fly.from_look) * u;
    let distance = (w / (2.0 * (FOV_Y_RAD * 0.5).tan())).clamp(MIN_DISTANCE_M, MAX_DISTANCE_M);
    let to_rot = cam.settled_rot(&fly.to, fly.to_distance, fly.to_yaw, fly.to_pitch, scene);
    let ease = t * t * (3.0 - 2.0 * t);
    MapPose::from_rot(look, fly.from_rot.slerp(to_rot, ease).normalize(), distance)
}

/// Projects world points onto the canvas for one draw. The world layer's
/// renderer camera is built from the same pose (`engine/map_view.rs`
/// `render_camera`), so a symbol lands on the thing it names.
#[derive(Debug, Clone, Copy)]
pub struct MapProjector {
    pub pose: MapPose,
    pub rect: egui::Rect,
    /// Pixels per unit of tan(angle) off the view axis.
    px_per_tan: f64,
}

impl MapProjector {
    pub fn new(pose: MapPose, rect: egui::Rect) -> Self {
        let px_per_tan = (rect.height() as f64 * 0.5) / (pose.fov_y * 0.5).tan();
        Self { pose, rect, px_per_tan }
    }

    /// The canvas point for a world point, and its depth along the view, or
    /// None behind (or at) the eye. The eye is subtracted in f64 first.
    pub fn project(&self, p: DVec3) -> Option<(egui::Pos2, f64)> {
        let v = p - self.pose.eye;
        let z = v.dot(self.pose.forward);
        if z <= self.pose.distance * 1e-9 {
            return None;
        }
        let x = v.dot(self.pose.right) / z * self.px_per_tan;
        let y = v.dot(self.pose.up) / z * self.px_per_tan;
        let c = self.rect.center();
        Some((egui::Pos2::new((c.x as f64 + x) as f32, (c.y as f64 - y) as f32), z))
    }

    /// A sphere of radius `r` metres at depth `depth`, in pixels.
    pub fn radius_px(&self, r: f64, depth: f64) -> f32 {
        if depth <= 0.0 {
            return 0.0;
        }
        let s = (r / depth).min(1e6);
        (s * self.px_per_tan) as f32
    }

    /// Metres per pixel at the anchor's distance.
    pub fn metres_per_px(&self) -> f64 {
        self.pose.distance / self.px_per_tan
    }
}

#[cfg(test)]
#[path = "camera_tests.rs"]
pub(crate) mod tests;
