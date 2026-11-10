//! CHARACTERS ON SCREEN (character animation rung 1, 2026-11-08; docs/design/character-animation.md).
//!
//! Every humanoid the frame draws is one skinned, animated body: the player in third person, the
//! other players, the crew, the showroom's mirror figure, and the dev crowd the rigs measure with.
//! Each frame, for each of them, this:
//!
//! 1. reads where the body is and which way it faces (the camera for the player; the interpolated
//!    position for another player, nothing new sent: plan 8.5; the crew's interpolated position
//!    and the facing net/sync turns as a person turns);
//! 2. reads its speed and turn rate from that motion (`humanity_animation::MotionTracker`) and runs
//!    the locomotion state machine (`Animator`) to a pose;
//! 3. writes the pose palette, and draws the body NEAR (compute-skinned, as ordinary objects in the
//!    opaque list, so its sun shadow and every pass come free) or MIDDLE (skinned in the vertex
//!    shader), by distance and the near cap (`renderer::skinning`, plan 8.4);
//! 4. tints each body region (skin, hair, the cosmetic slots) from the person's look and outfit,
//!    or from the wearer kind's defaults (`data/characters/characters.ron`).
//!
//! THE BODY IS DATA (`data/characters/`): the library is read at the first frame that needs it, read
//! again when a file there changes (hot reload), and, with the reuse switch off, read again every
//! few seconds (so a body that smells stale is ruled in or out by one click). The GPU copy of the
//! body is replaced only when the read model differs from the one uploaded. When the data cannot be
//! read, the log says why and the old box figures stand in (lib.rs), so a broken mod shows people,
//! not nothing.
//!
//! What a body keeps from frame to frame (its place in the state machine, its smoothed speed) is the
//! character's state, like a position, not a reuse of worked-out data. Nothing is kept across runs.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use glam::{Mat4, Quat, Vec3};
use humanity_animation::library::Fallback;
use humanity_animation::{pose, tiers, Animator, CharacterLibrary, DrawTier, FootMode, MotionTracker, Pose, Tint};

use crate::engine::feet_ground::{PersonGround, PlanetGround, WorldGround};
use crate::engine::state::EngineState;
use crate::renderer::RenderObject;

/// With the reuse switch off, the character data is read again this often (seconds).
const REUSE_OFF_REREAD_S: f32 = 3.0;
/// While the vertex-shader skinning PSO is still compiling (the first seconds after the first
/// body), middle bodies are compute-skinned too, up to this many places.
const COMPUTE_FALLBACK_CAP: usize = 64;
/// A crew member's position is their standing height, this far over their feet
/// (`net_route`'s figure, net/sync.rs `NPC_LOCAL_STANDING_Y`).
const CREW_POSITION_OVER_FEET_M: f32 = 1.0;
/// The showroom's podium height (`world_load::place_avatar`).
const SHOWROOM_PODIUM_M: f32 = 0.15;

/// Who a figure is, so its state machine and smoothed motion follow it from frame to frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum FigureKey {
    Me,
    Showroom,
    Player(u32),
    Crew(u64),
    Crowd(u32),
}

/// One person to draw this frame.
#[derive(Debug, Clone)]
pub(crate) struct Person {
    pub key: FigureKey,
    /// Where the feet stand.
    pub feet: Vec3,
    /// The turn about +Y taking the body's front (+Z) to the way it faces, radians.
    pub facing: f32,
    /// Look height multiplier (1.0 = the body's own height). A person whose shape is `Varied` takes
    /// their wearer kind's height range instead.
    pub height: f32,
    /// Their build, proportions and ancestry (the character page's sliders).
    pub shape: ShapeValues,
    /// The wearer kind whose defaults fill what the person does not say ("player", "crew").
    pub wearer: &'static str,
    /// The body their look chooses (`Appearance::body_type`: "female", "male"); None or an unknown
    /// id takes one of the wearer kind's bodies, stable per person.
    pub body: Option<String>,
    pub skin: Option<[f32; 3]>,
    pub hair: Option<[f32; 3]>,
    pub eyes: Option<[f32; 3]>,
    pub outfit: Option<humanity_core::cosmetics::OutfitColors>,
    /// The way up where they stand, world (rung 2a): +Y aboard, the local radial on a planet, so a
    /// body there stands upright on the ground and its feet are measured along it.
    pub up: Vec3,
    /// Crouching (the crouch key; another player's network stance).
    pub crouch: bool,
    /// Off the ground (a jump, a fall; another player's network stance).
    pub airborne: bool,
}

/// Where a person's shape slider values come from (2026-11-09).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ShapeValues {
    /// Their look's (`Appearance::shape`, or another player's `PlayerLook::shape`): a slider it does
    /// not name is at its middle, the body as made.
    Look(std::collections::BTreeMap<String, f32>),
    /// A person with no look of their own (crew, the dev crowd): each slider and their height picked
    /// in their wearer kind's ranges (`characters.ron`) by a stable hash of this seed.
    Varied(u64),
}

/// A stable number per figure: what its stagger and its varied shape are hashed from.
fn figure_seed(key: FigureKey) -> u64 {
    match key {
        FigureKey::Me => 0,
        FigureKey::Showroom => 1,
        FigureKey::Player(id) => 2 + id as u64,
        FigureKey::Crew(id) => 7 + id,
        FigureKey::Crowd(i) => 11 + i as u64,
    }
}

/// The shape targets (index, weight) and the uniform scale a person is drawn with on a body: their
/// look's slider values and height, or, for a person with no look, values and a height picked in
/// their wearer kind's ranges.
pub(crate) fn shape_of(lib: &CharacterLibrary, body: &humanity_animation::LoadedBody, p: &Person) -> (Vec<(usize, f32)>, f32) {
    match &p.shape {
        ShapeValues::Look(values) => body.shape_for(p.height, &|id| values.get(id).copied()),
        ShapeValues::Varied(seed) => {
            let wearer = lib.wearer(p.wearer);
            let height = wearer.map_or(p.height, |w| w.varied_height(*seed));
            body.shape_for(height, &|id| wearer.and_then(|w| w.varied(*seed, id)))
        }
    }
}

/// A dev stand-in (the `crowd` and `figure_path` showcase verbs): walks a circle, turns on the spot,
/// or walks a path back and forth, so the rigs can measure bodies without a relay full of people.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum CrowdMotion {
    Circle { centre: Vec3, radius: f32, speed: f32 },
    Turn { at: Vec3, rate: f32 },
    Path { points: Vec<Vec3>, speed: f32 },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CrowdFigure {
    pub motion: CrowdMotion,
    /// Where on its circle or path it starts, 0 to 1.
    pub phase: f32,
    /// The runtime's clock when it was made, so a rig's capture a known time after the verb finds
    /// it at a known place.
    pub born: f32,
    /// A look of its own (the `lineup` verb: a body and slider values), in place of the crew's
    /// varied shape; None for an ordinary stand-in.
    pub look: Option<CrowdLook>,
    /// Crouching (the `crowd_crouch` verb, rung 2a), as another player's network stance makes them.
    pub crouch: bool,
}

/// A stand-in's own body and shape (the `lineup` verb).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CrowdLook {
    pub body: String,
    pub shape: std::collections::BTreeMap<String, f32>,
}

/// What the last frame drew (the Dev page and the rigs read it).
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Report {
    pub people: usize,
    pub near: usize,
    pub middle: usize,
    /// Middle bodies compute-skinned because the vertex-shader PSO was not ready.
    pub fallback: usize,
    /// The bodies read, by id, comma-separated.
    pub body: String,
    /// People drawn with a shape (morph targets) of their own this frame.
    pub shaped: usize,
    /// Bodies whose feet got the full solve (leg IK, locking) and the pelvis offset alone, this
    /// frame (rung 2a), and the ground questions they asked.
    pub feet_full: usize,
    pub feet_pelvis: usize,
    /// Near bodies the feet's budget gave the pelvis offset instead of the full solve this frame.
    pub feet_over_budget: usize,
    pub ground_queries: u32,
    pub error: Option<String>,
}

struct Figure {
    animator: Animator,
    motion: MotionTracker,
    seen: u64,
    /// The person's skeleton reshaped for their shape weights, with the body and weights it was
    /// made for, and its legs and soles (rung 2a). Kept from frame to frame while both stay the
    /// same (a value keyed by its own inputs, so it cannot be stale) and made again every frame
    /// with the reuse switch off.
    shape: Option<(usize, Vec<(usize, f32)>, humanity_animation::Skeleton, Option<humanity_animation::FootRig>)>,
    /// Their feet from frame to frame: the pelvis offset and each foot's lock (rung 2a; state,
    /// like a position).
    feet: humanity_animation::FootPlacer,
    /// Seconds off the ground, while they are (rung 2a; the graph's jump reads it).
    air_s: f32,
}

/// THE FOOT PROBE (rung 2a, the proof's judge; showcase `foot_probe`): for a set time, every frame,
/// each near body's skinned feet (the foot vertices skinned on the CPU with the very palette the GPU
/// skins with, so it reads the drawn mesh, never the setup code) measured against the ground under
/// them; written to `debug/foot_probe.json` for scripts/lib/foot-probe-check.js.
#[derive(Debug, Clone, Default)]
pub(crate) struct FootProbe {
    /// The runtime clock when it stops and writes.
    pub until: f32,
    /// Per person and foot, the planted frames' clearances (the lowest foot vertex over the ground
    /// under it, metres; negative is through the ground) and the swing frames' lowest.
    pub rows: HashMap<FigureKey, ProbeRow>,
    pub frames: u32,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ProbeRow {
    pub body: String,
    pub tier: &'static str,
    pub mode: &'static str,
    pub planted: [Vec<f32>; 2],
    pub swing_low: [f32; 2],
    /// The ground heights seen under the feet (along up, from the first frame's), for the judge's
    /// "is this ground actually uneven here" check.
    pub ground_lo: f32,
    pub ground_hi: f32,
    pub ground_ref: Option<Vec3>,
    pub on_tread: u32,
    /// Each foot's lowest planted reading and where it was.
    pub worst: [Option<crate::engine::foot_probe::Clearance>; 2],
}

pub(crate) struct AnimationRuntime {
    lib: Option<CharacterLibrary>,
    /// Each library body's GPU id, in the library's body order.
    gpu_bodies: Vec<usize>,
    tried: bool,
    reread: bool,
    read_at: Option<std::time::Instant>,
    figures: HashMap<FigureKey, Figure>,
    /// Region materials by colour and roughness (64 steps each): a GPU allocation keyed by its own
    /// value, so it can never be stale; made once each, like the remote looks' materials.
    materials: HashMap<[u8; 4], usize>,
    frame: u64,
    clock: f32,
    /// The A/B switch (showcase `figures`): false draws the old box figures, for a same-boot
    /// comparison with the bodies (rung 0's baseline).
    pub(crate) bodies_on: bool,
    /// The bodies' A/B (showcase `figures` "test_figure", 2026-11-09): draw everyone with
    /// characters.ron's fallback body (the test figure), for a same-boot comparison of the real
    /// bodies' costs against the rung 1 figure's.
    pub(crate) fallback_only: bool,
    /// The body sliders' A/B (showcase `varied_shapes`, 2026-11-09): false draws the crew and the
    /// dev crowd as the body as made (no shape targets), as main did, for a same-boot measure of
    /// what varied shapes cost.
    pub(crate) varied_shapes: bool,
    pub(crate) crowd: Vec<CrowdFigure>,
    pub(crate) report: Report,
    /// The box figure's meshes and the crew's amber material, for the dev crowd in the boxes arm.
    box_meshes: Option<(usize, usize, usize)>,
    /// The skeleton overlay's unit box and its bone and joint materials (made the first time Show
    /// skeletons is on; a GPU allocation of fixed values, so it cannot be stale).
    skeleton_meshes: Option<(usize, usize, usize)>,
    pose: Pose,
    model: Vec<Mat4>,
    rows: Vec<[f32; 4]>,
    /// The foot probe while it runs (showcase `foot_probe`).
    pub(crate) probe: Option<FootProbe>,
    /// The player's height as last measured for the character page, with the inputs it was
    /// measured for (body, look height, shape): a value keyed by its own inputs, measured again
    /// every frame with the reuse switch off.
    my_height: Option<(usize, f32, std::collections::BTreeMap<String, f32>, f32)>,
    /// What one body's full foot solve has been costing on this machine, microseconds (eased over
    /// frames; 0 until the first is timed): the feet's automatic budget (question 5, "simplified
    /// automatically below it") gives the full solve to as many of the nearest bodies as
    /// `FeetDef::full_budget_ms` allows and the pelvis offset to the rest. A measurement, not a
    /// cache: it is taken again every frame.
    full_solve_us: f32,
}

impl Default for AnimationRuntime {
    fn default() -> Self {
        AnimationRuntime {
            lib: None,
            gpu_bodies: Vec::new(),
            tried: false,
            reread: false,
            read_at: None,
            figures: HashMap::new(),
            materials: HashMap::new(),
            frame: 0,
            clock: 0.0,
            bodies_on: true,
            fallback_only: false,
            varied_shapes: true,
            crowd: Vec::new(),
            report: Report::default(),
            box_meshes: None,
            skeleton_meshes: None,
            pose: Pose::default(),
            model: Vec::new(),
            rows: Vec::new(),
            probe: None,
            my_height: None,
            full_solve_us: 0.0,
        }
    }
}

impl AnimationRuntime {
    /// A file under `data/characters/` changed: read the library again next frame.
    pub(crate) fn request_reread(&mut self) {
        self.reread = true;
    }

    /// The runtime's clock, seconds of frames drawn (the dev crowd walks by it).
    pub(crate) fn clock(&self) -> f32 {
        self.clock
    }

    /// Are the bodies what the frame draws (the data read, the switch on)?
    pub(crate) fn draws_bodies(&self) -> bool {
        self.bodies_on && self.lib.as_ref().is_some_and(|l| l.bodies.len() == self.gpu_bodies.len())
    }
}

/// A model path relative to the data folder or, in a checkout, the repo root beside it (the asset
/// manager's rule).
fn resolve_model(data_dir: &Path, rel: &str) -> PathBuf {
    let p = data_dir.join(rel);
    if !p.exists() {
        if let Some(parent) = data_dir.parent() {
            let alt = parent.join(rel);
            if alt.exists() {
                return alt;
            }
        }
    }
    p
}

/// Read the character library from a data folder with the glTF skin reader.
pub(crate) fn read_library(data_dir: &Path) -> Result<CharacterLibrary, String> {
    let mut read = |rel: &str| crate::assets::skin::read_skinned_gltf(&resolve_model(data_dir, rel));
    CharacterLibrary::load(data_dir, &mut read)
}

/// Read the library if it has not been read, a file changed, or (reuse off) it is due again.
pub(crate) fn ensure_loaded(state: &mut EngineState) {
    let rt = &mut state.animation;
    let due = !humanity_core::reuse::enabled() && rt.read_at.is_some_and(|t| t.elapsed().as_secs_f32() > REUSE_OFF_REREAD_S);
    if rt.tried && !rt.reread && !due {
        return;
    }
    rt.tried = true;
    rt.reread = false;
    rt.read_at = Some(std::time::Instant::now());
    match read_library(&state.data_dir) {
        Ok(lib) => {
            let same_models = rt.gpu_bodies.len() == lib.bodies.len()
                && rt.lib.as_ref().is_some_and(|old| old.bodies.len() == lib.bodies.len() && old.bodies.iter().zip(&lib.bodies).all(|(a, b)| a.model == b.model));
            if !same_models {
                state.renderer.skin_clear_bodies();
                rt.gpu_bodies = lib.bodies.iter().map(|b| state.renderer.skin_register_body(&b.model)).collect();
                for (b, &gpu) in lib.bodies.iter().zip(&rt.gpu_bodies) {
                    log::info!(
                        "Characters: body {} as {} ({} joints, {} vertices, {} regions, {} shape targets, {:.2} MB on the GPU), {} clips, graph {}",
                        b.def.id,
                        b.choice,
                        b.skeleton.joint_count(),
                        b.model.vertex_count(),
                        b.model.primitives.len(),
                        b.model.morph_names.len(),
                        state.renderer.skin_body_bytes(gpu) as f64 / 1_048_576.0,
                        b.clips.clips.len(),
                        lib.graph.id
                    );
                }
                for w in &lib.warnings {
                    log::warn!("Characters: {w}");
                }
            }
            // A new graph may have other states: every figure starts again.
            if rt.lib.as_ref().map_or(true, |old| old.graph != lib.graph) {
                rt.figures.clear();
            }
            // A reshaped skeleton belongs to the library it was made from: made again from this one.
            for f in rt.figures.values_mut() {
                f.shape = None;
            }
            rt.report.body = lib.bodies.iter().map(|b| b.def.id.as_str()).collect::<Vec<_>>().join(",");
            rt.report.error = None;
            // The character page's body chooser lists the bodies a look can choose, and draws the
            // chosen body's shape sliders.
            state.gui_state.characters.body_choices = lib.characters.bodies.keys().cloned().collect();
            state.gui_state.characters.body_sliders = lib.bodies.iter().filter(|b| lib.characters.bodies.contains_key(&b.choice)).map(|b| (b.choice.clone(), b.slider_blocks.clone())).collect();
            rt.lib = Some(lib);
        }
        Err(e) => {
            if rt.report.error.as_deref() != Some(e.as_str()) {
                log::error!("Characters: the character data did not load, so the box figures stand in: {e}");
            }
            rt.report.error = Some(e);
            rt.lib = None;
            rt.gpu_bodies.clear();
        }
    }
}

/// The facing angle (turn of +Z about +Y) of a body looking along a camera at `yaw` (which looks
/// along (sin yaw, 0, -cos yaw), renderer/camera.rs `forward_xz`).
pub(crate) fn facing_for_camera_yaw(yaw: f32) -> f32 {
    std::f32::consts::PI - yaw
}

/// The facing angle (turn of +Z about the local up) of a body standing with `up` (world) and looking
/// along `forward` (rung 2a: on a planet, where up is the local radial): the turn, after +Y is turned
/// onto `up` the shortest way, that takes the body's front onto `forward` along the ground.
pub(crate) fn facing_about(up: Vec3, forward: Vec3) -> f32 {
    let local = body_rotation(up, 0.0).inverse() * forward;
    if local.x.abs() + local.z.abs() < 1e-6 {
        0.0
    } else {
        local.x.atan2(local.z)
    }
}

/// A body's turn in the world: +Y onto its up (the shortest way), then its facing about that up.
pub(crate) fn body_rotation(up: Vec3, facing: f32) -> Quat {
    let base = if (up - Vec3::Y).length_squared() < 1e-10 || up == Vec3::ZERO { Quat::IDENTITY } else { Quat::from_rotation_arc(Vec3::Y, up) };
    base * Quat::from_rotation_y(facing)
}

/// The facing angle of a transform's rotation that is a pure turn about +Y.
pub(crate) fn facing_of_turn(q: Quat) -> f32 {
    2.0 * q.y.atan2(q.w)
}

/// Where a dev stand-in is at `clock` seconds, and which way it faces.
pub(crate) fn crowd_place(fig: &CrowdFigure, clock: f32) -> (Vec3, f32) {
    let clock = clock - fig.born;
    match &fig.motion {
        CrowdMotion::Circle { centre, radius, speed } => {
            let r = radius.max(0.1);
            let a = fig.phase * std::f32::consts::TAU + clock * speed / r;
            let at = *centre + Vec3::new(r * a.cos(), 0.0, r * a.sin());
            // Walking counter-clockwise seen from above: the way it goes is the circle's tangent.
            let tangent = Vec3::new(-a.sin(), 0.0, a.cos()) * speed.signum();
            let facing = if speed.abs() > 1e-4 { tangent.x.atan2(tangent.z) } else { 0.0 };
            (at, facing)
        }
        CrowdMotion::Turn { at, rate } => (*at, fig.phase * std::f32::consts::TAU + clock * rate),
        CrowdMotion::Path { points, speed } => path_place(points, *speed, fig.phase, clock),
    }
}

/// Back and forth along a polyline at `speed` (horizontal metres a second; a vertical leg takes the
/// time its height would take): the place and the way it faces (along the leg, kept on a vertical one).
fn path_place(points: &[Vec3], speed: f32, phase: f32, clock: f32) -> (Vec3, f32) {
    if points.len() < 2 || speed <= 0.0 {
        return (points.first().copied().unwrap_or(Vec3::ZERO), 0.0);
    }
    let legs: Vec<f32> = points.windows(2).map(|w| (w[1] - w[0]).length().max(1e-3)).collect();
    let total: f32 = legs.iter().sum();
    let round = 2.0 * total;
    let mut s = (phase * round + clock * speed).rem_euclid(round);
    let back = s > total;
    if back {
        s = round - s;
    }
    let mut facing = 0.0;
    for (i, len) in legs.iter().enumerate() {
        let (a, b) = (points[i], points[i + 1]);
        let flat = Vec3::new(b.x - a.x, 0.0, b.z - a.z);
        if flat.length() > 1e-3 {
            let d = if back { -flat } else { flat };
            facing = d.x.atan2(d.z);
        }
        if s <= *len || i == legs.len() - 1 {
            let t = (s / len).clamp(0.0, 1.0);
            return (a.lerp(b, t), facing);
        }
        s -= len;
    }
    (points[0], facing)
}

/// The showcase `crowd` verb: `"N[,radius_m[,speed_mps[,turn_rate]]]"` stand-ins around `origin`
/// (the feet under the camera). Speed below 0 mixes the speeds (standing, walking, jogging,
/// running); speed 0 with a turn rate turns them on the spot. `"0"` clears the crowd.
pub(crate) fn parse_crowd(spec: &str, origin: Vec3) -> Option<Vec<CrowdFigure>> {
    let v: Vec<f32> = spec.split(',').map(|p| p.trim().parse().ok()).collect::<Option<Vec<f32>>>()?;
    let n = *v.first()? as i64;
    if !(0..=2000).contains(&n) {
        return None;
    }
    let radius = v.get(1).copied().unwrap_or(6.0).max(0.5);
    let speed = v.get(2).copied().unwrap_or(1.4);
    let turn = v.get(3).copied().unwrap_or(0.0);
    let mixed = [0.0, 0.9, 1.4, 2.2, 3.6];
    let golden = 0.618_034_f32;
    Some(
        (0..n as usize)
            .map(|i| {
                let f = i as f32;
                // Rings outward so the figures do not stand inside each other.
                let ring = radius * (0.35 + 0.65 * ((f + 0.5) / n.max(1) as f32).sqrt());
                let phase = (f * golden).fract();
                let s = if speed < 0.0 { mixed[i % mixed.len()] } else { speed };
                let motion = if s == 0.0 {
                    // Standing (turning on the spot when a turn rate is given).
                    let a = phase * std::f32::consts::TAU;
                    CrowdMotion::Turn { at: origin + Vec3::new(ring * a.cos(), 0.0, ring * a.sin()), rate: turn }
                } else {
                    // Alternate directions, so neighbours pass each other.
                    CrowdMotion::Circle { centre: origin, radius: ring, speed: if i % 2 == 0 { s } else { -s } }
                };
                CrowdFigure { motion, phase, born: 0.0, look: None, crouch: false }
            })
            .collect(),
    )
}

/// The showcase `lineup` verb (2026-11-09, the body sliders' proof): `"body,ahead_m,gap_m|spec|spec|..."`
/// stands one person per spec in a row across the view, `ahead_m` in front of `feet` along
/// `forward` (flat), `gap_m` apart, each facing back toward the camera at idle, drawn with `body`
/// and the spec's slider values: `"muscle=0,weight=1"`, or `"-"` for the body as made. Values are
/// clamped to 0 to 1 when drawn, as a look's are.
pub(crate) fn parse_lineup(spec: &str, feet: Vec3, forward: Vec3) -> Option<Vec<CrowdFigure>> {
    let mut parts = spec.split('|').map(str::trim);
    let head: Vec<&str> = parts.next()?.split(',').map(str::trim).collect();
    let body = humanity_core::player_look::clean_body_id(head.first()?);
    if body.is_empty() {
        return None;
    }
    let ahead: f32 = head.get(1).map_or(Some(3.5), |s| s.parse().ok())?;
    let gap: f32 = head.get(2).map_or(Some(0.85), |s| s.parse().ok())?;
    let specs: Vec<&str> = parts.filter(|p| !p.is_empty()).collect();
    if specs.is_empty() || specs.len() > 64 || !ahead.is_finite() || !gap.is_finite() {
        return None;
    }
    let fwd = Vec3::new(forward.x, 0.0, forward.z).try_normalize()?;
    let right = fwd.cross(Vec3::Y);
    // Facing back along the view: the body's front (+Z) turned onto -forward.
    let facing = (-fwd.x).atan2(-fwd.z);
    let phase = (facing / std::f32::consts::TAU).rem_euclid(1.0);
    let n = specs.len() as f32;
    specs
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let mut shape = std::collections::BTreeMap::new();
            if *s != "-" {
                for kv in s.split(',') {
                    let (k, v) = kv.split_once('=')?;
                    let id = humanity_core::player_look::clean_shape_id(k.trim());
                    let v: f32 = v.trim().parse().ok()?;
                    if id.is_empty() || !v.is_finite() {
                        return None;
                    }
                    shape.insert(id, v);
                }
            }
            let at = feet + fwd * ahead + right * ((i as f32 - (n - 1.0) * 0.5) * gap);
            Some(CrowdFigure { motion: CrowdMotion::Turn { at, rate: 0.0 }, phase, born: 0.0, look: Some(CrowdLook { body: body.clone(), shape }), crouch: false })
        })
        .collect()
}

/// The showcase `figure_path` verb: `"x,y,z;x,y,z;...;speed"` one stand-in walking that path back and
/// forth (the rung 0 vantages: a figure on the stairs, at a ladder, to a chair).
pub(crate) fn parse_path(spec: &str) -> Option<CrowdFigure> {
    let parts: Vec<&str> = spec.split(';').map(str::trim).filter(|p| !p.is_empty()).collect();
    let (last, pts) = parts.split_last()?;
    let speed: f32 = last.parse().ok()?;
    let points = pts
        .iter()
        .map(|p| {
            let v: Vec<f32> = p.split(',').map(|x| x.trim().parse().ok()).collect::<Option<Vec<f32>>>()?;
            (v.len() == 3).then(|| Vec3::new(v[0], v[1], v[2]))
        })
        .collect::<Option<Vec<Vec3>>>()?;
    (points.len() >= 2 && speed > 0.0).then_some(CrowdFigure { motion: CrowdMotion::Path { points, speed }, phase: 0.0, born: 0.0, look: None, crouch: false })
}

/// One region's colour for a person.
pub(crate) fn region_colour(lib: &CharacterLibrary, tint: &Tint, p: &Person) -> [f32; 3] {
    let wearer = lib.wearer(p.wearer);
    let skin = p.skin.or(wearer.map(|w| w.skin)).unwrap_or([0.7, 0.55, 0.45]);
    let hair = p.hair.or(wearer.map(|w| w.hair)).unwrap_or([0.2, 0.15, 0.1]);
    match tint {
        Tint::Skin => skin,
        Tint::Hair => hair,
        Tint::Eyes => p.eyes.or(wearer.map(|w| w.eyes)).unwrap_or([0.3, 0.45, 0.55]),
        Tint::Fixed(r, g, b) => [*r, *g, *b],
        Tint::Slot(slot) => {
            let equipped = p.outfit.as_ref().and_then(|o| match slot.as_str() {
                "head" => o.head,
                "chest" => o.chest,
                "legs" => o.legs,
                "feet" => o.feet,
                "hands" => o.hands,
                "back" => o.back,
                _ => None,
            });
            equipped.unwrap_or_else(|| match wearer.and_then(|w| w.clothes.get(slot)) {
                Some(Fallback::Skin) => skin,
                Some(Fallback::Hair) => hair,
                Some(Fallback::Rgb(r, g, b)) => [*r, *g, *b],
                None => [0.5, 0.5, 0.5],
            })
        }
    }
}

/// Everyone to draw this frame.
fn gather(state: &EngineState, showroom: bool) -> Vec<Person> {
    // People with no look of their own take varied shapes, unless the A/B (showcase
    // `varied_shapes` 0) draws them all as the body as made, as before the sliders.
    let varied = state.animation.varied_shapes;
    let no_look = |key: FigureKey| if varied { ShapeValues::Varied(figure_seed(key)) } else { ShapeValues::Look(Default::default()) };
    let mut out = Vec::new();
    let me = |key: FigureKey, feet: Vec3, facing: f32, up: Vec3, crouch: bool, airborne: bool| {
        let app = &state.gui_state.appearance;
        let outfit = humanity_core::cosmetics::resolve_outfit_colors(&state.gui_state.outfit, &state.cosmetics);
        Person {
            key,
            feet,
            facing,
            height: app.height_scale.clamp(0.5, 2.0),
            shape: ShapeValues::Look(app.shape.clone()),
            wearer: "player",
            body: Some(app.body_type.clone()),
            skin: Some(app.skin_tone),
            hair: Some(app.hair_color),
            eyes: Some(app.eye_color),
            outfit: Some(outfit),
            up,
            crouch,
            airborne,
        }
    };
    if showroom {
        out.push(me(FigureKey::Showroom, state.avatar_base + Vec3::new(0.0, SHOWROOM_PODIUM_M, 0.0), 0.0, Vec3::Y, false, false));
        return out;
    }
    if state.camera.mode == crate::renderer::camera::CameraMode::ThirdPerson && !state.gui_state.construction_active {
        // On a planet the way up is the local radial (rung 2a): the body stands on the ground under
        // the eye and faces the way the camera looks along the ground.
        let (up, facing) = if state.camera.surface_mode {
            let up = state.camera.surface_up.normalize_or_zero();
            (up, facing_about(up, state.camera.forward()))
        } else {
            (Vec3::Y, facing_for_camera_yaw(state.camera.yaw))
        };
        let c = &state.controller;
        out.push(me(FigureKey::Me, state.camera.position - up * c.eye_height(), facing, up, c.crouching(), c.airborne()));
    }
    let world = &state.game_world.world;
    for (_e, (t, r)) in world.query::<(&humanity_core::ecs::components::Transform, &crate::net::sync::RemotePlayer)>().iter() {
        if crate::engine::block::hides_player(&state.gui_state, r) {
            continue;
        }
        let yaw = 2.0 * t.rotation.y.atan2(t.rotation.w);
        out.push(Person {
            key: FigureKey::Player(r.player_id),
            feet: t.position - Vec3::new(0.0, humanity_core::ecs::components::EYE_HEIGHT_M as f32, 0.0),
            facing: facing_for_camera_yaw(yaw),
            height: r.look.as_ref().map_or(1.0, |l| l.height),
            // The shape they chose, as their look carried it (clamped by PlayerLook); none for an
            // older app's look, which draws the body as made.
            shape: ShapeValues::Look(r.look.as_ref().map(|l| l.shape.clone()).unwrap_or_default()),
            wearer: "player",
            body: r.look.as_ref().map(|l| l.body.clone()),
            skin: r.look.as_ref().map(|l| l.skin),
            hair: r.look.as_ref().map(|l| l.hair),
            eyes: None,
            outfit: None,
            up: Vec3::Y,
            // How they stand, as their newest update said (rung 2a).
            crouch: r.posture.stance == humanity_core::stance::Stance::Crouch,
            airborne: r.posture.airborne,
        });
    }
    for (_e, (t, n)) in world.query::<(&humanity_core::ecs::components::Transform, &crate::net::sync::RemoteNpc)>().iter() {
        out.push(Person {
            key: FigureKey::Crew(n.entity_id),
            feet: t.position - Vec3::new(0.0, CREW_POSITION_OVER_FEET_M, 0.0),
            facing: facing_of_turn(t.rotation),
            height: 1.0,
            shape: no_look(FigureKey::Crew(n.entity_id)),
            wearer: "crew",
            body: None,
            skin: None,
            hair: None,
            eyes: None,
            outfit: None,
            up: Vec3::Y,
            crouch: false,
            airborne: false,
        });
    }
    let clock = state.animation.clock;
    for (i, c) in state.animation.crowd.iter().enumerate() {
        let (feet, facing) = crowd_place(c, clock);
        let key = FigureKey::Crowd(i as u32);
        let (shape, body) = match &c.look {
            Some(l) => (ShapeValues::Look(l.shape.clone()), Some(l.body.clone())),
            None => (no_look(key), None),
        };
        out.push(Person { key, feet, facing, height: 1.0, shape, wearer: "crew", body, skin: None, hair: None, eyes: None, outfit: None, up: Vec3::Y, crouch: c.crouch, airborne: false });
    }
    out
}

/// The dev crowd as the old box figures (the A/B's boxes arm, and when the character data did not
/// load): the crew's figure, `net_route::crew_figure_parts`, at each stand-in. lib.rs draws the
/// players' and crew's boxes itself.
fn draw_crowd_boxes(state: &mut EngineState, all_objects: &mut Vec<RenderObject>) {
    if state.animation.crowd.is_empty() {
        return;
    }
    if state.animation.box_meshes.is_none() {
        use crate::engine::net_route::{FIGURE_BODY_MESH_H_M, FIGURE_HEAD_MESH_R_M};
        let r = &mut state.renderer;
        let body = r.add_mesh(crate::renderer::mesh::Mesh::box_xyz(&r.device, 0.42, FIGURE_BODY_MESH_H_M, 0.26));
        let (hv, hi) = crate::engine::net_route::figure_head_mesh_data(FIGURE_HEAD_MESH_R_M);
        let head = r.add_mesh(crate::renderer::mesh::Mesh::from_vertices(&r.device, &hv, &hi));
        // The crew's amber marker, as lib.rs makes it.
        let mat = r.add_material_full([0.92, 0.62, 0.18, 1.0], 0.0, 0.55, 1.0, 0.22);
        state.animation.box_meshes = Some((body, head, mat));
    }
    let (body, head, mat) = state.animation.box_meshes.expect("made above");
    let clock = state.animation.clock;
    for c in &state.animation.crowd {
        let (feet, facing) = crowd_place(c, clock);
        let at = feet + Vec3::new(0.0, CREW_POSITION_OVER_FEET_M, 0.0);
        for p in crate::engine::net_route::crew_figure_parts(at, Quat::from_rotation_y(facing)) {
            let mesh = if p.part == crate::engine::net_route::FigurePart::Body { body } else { head };
            all_objects.push(RenderObject { fade: 0.0, position: p.at, rotation: p.rotation, scale: p.scale, mesh, material: mat });
        }
    }
}

/// How much longer a person's stride is than their body's unshaped one: their uniform scale times
/// how much higher their reshaped hips stand (the legs' length, as the retargeter measures it).
pub(crate) fn stride_scale(shaped: &humanity_animation::Skeleton, base: &humanity_animation::Skeleton, humanoid: &humanity_animation::HumanoidDef, scale: f32) -> f32 {
    let s = scale * shaped.hips_height(humanoid) / base.hips_height(humanoid);
    if s.is_finite() && s > 0.05 {
        s
    } else {
        1.0
    }
}

/// A stable stagger per figure, so a crowd does not step in unison.
fn stagger(key: FigureKey) -> f32 {
    ((figure_seed(key).wrapping_mul(2_654_435_761) % 1000) as f32) / 1000.0
}

/// A person's own legs on their reshaped skeleton `skel`, with what the body's mesh measured at load
/// (a foot's reach, its toe cap and heel counter) taken from the body as made. (Copying the reach
/// alone left every person in the game with no toe cap or heel counter: the fourth and fifth rig
/// sweeps read anim-stairs' swinging feet 8 to 12 cm into a step while the tests, which walked the
/// body's own rig, passed; `a_persons_own_rig_carries_what_the_body_measured` now checks it.)
pub(crate) fn person_rig(body: &humanity_animation::LoadedBody, skel: &humanity_animation::Skeleton, humanoid: &humanity_animation::HumanoidDef) -> Option<humanity_animation::FootRig> {
    humanity_animation::FootRig::new(skel, humanoid).map(|r| match &body.feet {
        Some(b) => r.with_measured(b),
        None => r,
    })
}

/// The skeleton overlay's bone thickness and joint marker size, metres.
const SKELETON_BONE_M: f32 = 0.012;
const SKELETON_JOINT_M: f32 = 0.03;

/// Draw every person as a skinned, animated body. Returns false when the bodies are not what the
/// frame draws (data missing, or the A/B switch on boxes): lib.rs then draws the box figures. With
/// the Dev page's Show skeletons on, each body's joints and bones go into `overlay` (drawn on top
/// of everything, as the editor's gizmos are).
///
/// `render_space` takes the player's own body when they are not aboard (on a planet): it stands
/// under the camera, already in render space, while `all_objects` is the HOME frame that lib.rs
/// moves to the station's orbital place (`station_off`) before the scene pass, which drew the body
/// 400 km up, out of sight, until rung 2a (2026-11-09). lib.rs appends it after that shift, as it
/// does the planet piece in hand.
pub(crate) fn draw_people(state: &mut EngineState, all_objects: &mut Vec<RenderObject>, render_space: &mut Vec<RenderObject>, overlay: &mut Vec<RenderObject>, showroom: bool, dt: f32) -> bool {
    let _cost = humanity_core::frame_costs::stage("cpu.animation");
    ensure_loaded(state);
    publish_my_height(state);
    let show_skeletons = state.gui_state.characters.show_skeletons;
    let dt = if dt.is_finite() { dt.clamp(0.0, 0.25) } else { 0.0 };
    state.animation.clock += dt;
    if !state.animation.draws_bodies() {
        state.animation.report.people = 0;
        // Nothing of the last bodies frame may be drawn again (the middle list lives on the
        // renderer until the next frame's start).
        state.renderer.skin_begin_frame();
        if !showroom {
            draw_crowd_boxes(state, all_objects);
        }
        return false;
    }
    state.animation.frame += 1;
    let people = gather(state, showroom);
    let cam = state.camera.effective_position();
    // THE GROUND under everyone's feet this frame (rung 2a, engine::feet_ground): aboard the
    // footing, on a planet the drawn terrain, in the showroom the podium; nothing in open space.
    // With the feet off the ground is still read while the foot probe runs, so the A/B's red arm
    // measures the same drawn feet against the same ground.
    let feet_on = state.gui_state.characters.feet_on_world;
    let need_ground = feet_on || state.animation.probe.is_some();
    let simplified = state.gui_state.settings.anim_simplified;
    let built = if need_ground && !showroom && state.aboard_station {
        crate::engine::build_place::built_piece_boxes(&state.game_world.world, state.data_store.get::<humanity_systems::construction::BlueprintRegistry>("blueprint_registry"), None)
    } else {
        Vec::new()
    };
    let planet_body = if need_ground && !showroom && !state.aboard_station && state.surface_walk_band { state.frame_lock_body.clone() } else { None };
    let planet_rot = planet_body.as_ref().map(|_| crate::engine::planet_build::body_render_rot(state.station_ride, state.station_world_rot, crate::engine::planet_build::frame_body_rot(state)));
    let earth = planet_body.as_deref() == Some("earth");
    let planet_parts = planet_body.as_deref().and_then(|b| Some((state.planet_defs.get(b)?, state.planet_chunk_states.get(b)?, state.planet_heightmaps.get(b))));
    let ocean = if earth { state.ocean_mask.as_ref() } else { None };
    let source = planet_parts.and_then(|(_, cs, hm)| {
        hm.map(|hm| humanity_terrain::planet_chunks::ElevationSource::Heightmap { hm: hm.as_ref(), detail: &cs.detail, tiles: earth.then_some(&state.terrain_tiles), ocean })
    });
    let ground = if !need_ground {
        WorldGround::None
    } else if showroom {
        WorldGround::Flat(state.avatar_base.y + SHOWROOM_PODIUM_M)
    } else if state.aboard_station {
        WorldGround::Ship { rooms: &state.gui_state.room_bounds, ship: state.gui_state.ship_structure.as_ref(), cars: &state.elevator_state, built: &built }
    } else if let (Some((def, cs, hm)), Some(rot)) = (planet_parts, planet_rot) {
        let depth = cs.last_drawn.iter().map(|p| p.depth).max().unwrap_or(0);
        // The walk's own rule (surface_walk::walk_ground_radius): a water world's drawn ground needs
        // its ocean mask, else the field answers.
        let drawn = source.as_ref().filter(|_| depth > 0 && !(def.has_water && ocean.is_none())).map(|s| std::cell::RefCell::new(humanity_terrain::drawn_surface::DrawnPatchSurface::new_single_shot(def, s, depth)));
        WorldGround::Planet(PlanetGround {
            def,
            drawn,
            hm: hm.map(|h| h.as_ref()),
            detail: &cs.detail,
            tiles: earth.then_some(&state.terrain_tiles),
            anchor: state.frame_lock_anchor,
            camera: state.camera.position,
            rot,
        })
    } else {
        WorldGround::None
    };
    // Off the ship (on a planet) the player's own body stands in render space (`render_space`).
    let me_in_render_space = !showroom && !state.aboard_station;
    let rt = &mut state.animation;
    let renderer = &mut state.renderer;
    let lib = rt.lib.as_ref().expect("draws_bodies checked it");
    let distances: Vec<f32> = people.iter().map(|p| (p.feet + Vec3::Y - cam).length()).collect();
    let tier = tiers::assign(&distances, &lib.characters.tiers);
    // THE FEET'S BUDGET (question 5: full realism where the measured budget allows, simplified
    // automatically below it): the nearest bodies get the full solve, as many as the budget buys at
    // what a solve has been costing on this machine; the rest of the near tier the pelvis offset.
    let max_full = if rt.full_solve_us > 0.0 { (lib.characters.feet.full_budget_ms * 1000.0 / rt.full_solve_us).floor().max(1.0) as usize } else { usize::MAX };
    let mut near_rank = vec![usize::MAX; people.len()];
    {
        let mut order: Vec<usize> = (0..people.len()).filter(|&i| tier[i] == DrawTier::Near).collect();
        order.sort_by(|&a, &b| distances[a].total_cmp(&distances[b]));
        for (rank, &i) in order.iter().enumerate() {
            near_rank[i] = rank;
        }
    }
    let (mut full_us, mut full_n) = (0.0_f32, 0usize);
    renderer.skin_begin_frame();
    let vs_ready = renderer.skin_middle_ready();
    let mut report = Report { people: people.len(), body: rt.report.body.clone(), ..Default::default() };
    // Near places are numbered per body, so a place keeps the body it was made for.
    let mut slots = vec![0usize; lib.bodies.len()];
    let mut fallback_used = 0usize;
    let reuse = humanity_core::reuse::enabled();
    // The overlay's unit box (base at its origin, one metre each way) and its two bright materials.
    let skeleton_meshes = show_skeletons.then(|| {
        *rt.skeleton_meshes.get_or_insert_with(|| {
            let unit = renderer.add_mesh(crate::renderer::mesh::Mesh::box_xyz(&renderer.device, 1.0, 1.0, 1.0));
            let bone = renderer.add_material_full([1.0, 0.85, 0.15, 1.0], 0.0, 0.5, 0.0, 1.0);
            let joint = renderer.add_material_full([0.2, 0.9, 1.0, 1.0], 0.0, 0.5, 0.0, 1.0);
            (unit, bone, joint)
        })
    });
    for (i, p) in people.iter().enumerate() {
        let hash = stagger(p.key);
        let bi = if rt.fallback_only {
            lib.body_index(&lib.characters.fallback).unwrap_or(0)
        } else {
            lib.body_for(p.body.as_deref(), p.wearer, hash)
        };
        let body = &lib.bodies[bi];
        let gpu_body = rt.gpu_bodies[bi];
        // The look's sliders through the body's shape targets, its height through the height
        // targets, the rest as a scale (a crew member's picked in their wearer kind's ranges).
        let (weights, scale) = shape_of(lib, body, p);
        let fig = rt.figures.entry(p.key).or_insert_with(|| Figure {
            animator: Animator::with_offset(&lib.graph, hash, hash * 4.0),
            motion: MotionTracker::default(),
            seen: 0,
            shape: None,
            feet: humanity_animation::FootPlacer::default(),
            air_s: 0.0,
        });
        fig.seen = rt.frame;
        let Figure { animator, motion, shape, feet, air_s, .. } = fig;
        let kept = reuse && shape.as_ref().is_some_and(|(b, w, _, _)| *b == bi && *w == weights);
        if !kept {
            *shape = body.reshaped(&weights).map(|s| {
                let rig = person_rig(body, &s, &lib.humanoid);
                (bi, weights.clone(), s, rig)
            });
        }
        // The person's own skeleton and legs: reshaped for their shape, else the body's as made.
        let (skel, rig) = match shape.as_ref() {
            Some((_, _, s, r)) => (s, r.as_ref()),
            None => (&body.skeleton, body.feet.as_ref()),
        };
        if shape.is_some() {
            report.shaped += 1;
        }
        *air_s = if p.airborne { *air_s + dt } else { 0.0 };
        let input = motion.observe(p.feet, p.facing, dt, lib.graph.smoothing_s, lib.graph.teleport_mps);
        // The graph and its stride warping read speed in the body's own size: a body scaled or
        // reshaped longer in the leg covers more ground a stride, so its clips (whose speeds are for
        // the unshaped body) play slower for the same speed and the feet keep still. Crouching and
        // time off the ground are its stance (rung 2a).
        let stride = stride_scale(skel, &body.skeleton, &lib.humanoid, scale);
        let input = humanity_animation::LocomotionInput {
            speed: input.speed / stride,
            side: input.side / stride,
            crouch: if p.crouch { 1.0 } else { 0.0 },
            airborne: *air_s,
            ..input
        };
        animator.update(&lib.graph, &body.clips, skel, &lib.humanoid, &input, dt, &mut rt.pose);
        pose::model_matrices(skel, &rt.pose, &mut rt.model);
        let rotation = body_rotation(p.up, p.facing);
        let world = Mat4::from_scale_rotation_translation(Vec3::splat(scale), rotation, p.feet);
        // FEET ON THE WORLD (rung 2a, humanity_animation::feet): the near bodies in the realistic
        // animation mode get the full solve (leg IK, the foot turned to the slope about its toe,
        // foot locking, stair footsteps), every other body the pelvis offset alone.
        let mode = match rig {
            Some(_) if feet_on && ground.is_some() => {
                if !simplified && tier[i] == DrawTier::Near && near_rank[i] < max_full {
                    FootMode::Full
                } else {
                    if !simplified && tier[i] == DrawTier::Near {
                        report.feet_over_budget += 1;
                    }
                    FootMode::Pelvis
                }
            }
            _ => FootMode::Off,
        };
        let person_ground = PersonGround { world: &ground, reference: p.feet.y };
        let feet_report = match rig {
            Some(rig) => {
                let frame = humanity_animation::FootFrame { world, contacts: animator.contacts(), airborne: p.airborne, dt };
                let t0 = (mode == FootMode::Full).then(std::time::Instant::now);
                let r = feet.place(rig, skel, &lib.humanoid, &mut rt.pose, &mut rt.model, &frame, &person_ground, mode, &lib.characters.feet);
                if let Some(t0) = t0 {
                    full_us += t0.elapsed().as_secs_f32() * 1.0e6;
                    full_n += 1;
                }
                r
            }
            None => humanity_animation::FeetReport::default(),
        };
        match mode {
            FootMode::Full => report.feet_full += 1,
            FootMode::Pelvis => report.feet_pelvis += 1,
            FootMode::Off => {}
        }
        report.ground_queries += feet_report.queries;
        rt.rows.clear();
        pose::palette_rows(skel, &rt.model, &mut rt.rows);
        let morph = pose::morph_rows(&weights, body.model.vertex_count() as u32);
        let index = renderer.skin_push_palette(pose::affine_rows(&world), &morph, &rt.rows);
        // The foot probe (showcase `foot_probe`): the drawn feet against the ground, near bodies.
        if let Some(probe) = rt.probe.as_mut().filter(|_| tier[i] == DrawTier::Near) {
            let clear = crate::engine::foot_probe::clearances(body, &weights, &rt.rows, &world, p.up, &person_ground);
            let mode_name = match mode {
                FootMode::Full => "full",
                FootMode::Pelvis => "pelvis",
                FootMode::Off => "off",
            };
            crate::engine::foot_probe::record(probe, p.key, body, "near", mode_name, clear, &feet_report, animator.contacts(), p.airborne, p.up);
        }
        // The skeleton as it is posed this frame: a bone from each joint to its parent, a marker
        // at each joint, through the same world matrix as the body; and where each foot found the
        // ground (a joint-coloured marker for a foot kept in place, a bone-coloured one otherwise).
        if let Some((unit, bone_mat, joint_mat)) = skeleton_meshes {
            for f in &feet_report.feet {
                if let Some(g) = f.ground {
                    overlay.push(RenderObject {
                        fade: 0.0,
                        position: g - Vec3::new(SKELETON_JOINT_M, 0.0, SKELETON_JOINT_M),
                        rotation: Quat::IDENTITY,
                        scale: Vec3::new(SKELETON_JOINT_M * 2.0, SKELETON_BONE_M, SKELETON_JOINT_M * 2.0),
                        mesh: unit,
                        material: if f.locked { joint_mat } else { bone_mat },
                    });
                }
            }
            let at = |j: usize| world.transform_point3(rt.model[j].w_axis.truncate());
            for j in 0..skel.joint_count() {
                let here = at(j);
                overlay.push(RenderObject {
                    fade: 0.0,
                    position: here - Vec3::new(0.0, SKELETON_JOINT_M * 0.5, 0.0),
                    rotation: Quat::IDENTITY,
                    scale: Vec3::splat(SKELETON_JOINT_M),
                    mesh: unit,
                    material: joint_mat,
                });
                if let Some(parent) = skel.parents[j] {
                    let from = at(parent);
                    let d = here - from;
                    let len = d.length();
                    if len > 1e-4 {
                        overlay.push(RenderObject {
                            fade: 0.0,
                            position: from,
                            rotation: Quat::from_rotation_arc(Vec3::Y, d / len),
                            scale: Vec3::new(SKELETON_BONE_M, len, SKELETON_BONE_M),
                            mesh: unit,
                            material: bone_mat,
                        });
                    }
                }
            }
        }
        let materials: Vec<usize> = body
            .primitive_regions
            .iter()
            .map(|&r| {
                let region = &body.def.regions[r];
                let c = region_colour(lib, &region.tint, p);
                let q = |x: f32| (x.clamp(0.0, 1.0) * 63.0).round() as u8;
                let key = [q(c[0]), q(c[1]), q(c[2]), q(region.roughness)];
                *rt.materials.entry(key).or_insert_with(|| {
                    renderer.add_material_full([c[0], c[1], c[2], 1.0], 0.0, region.roughness, crate::renderer::materials::MATERIAL_TYPE_CHARACTER, 0.0)
                })
            })
            .collect();
        let near = tier[i] == DrawTier::Near || (!vs_ready && fallback_used < COMPUTE_FALLBACK_CAP);
        if near {
            if tier[i] == DrawTier::Near {
                report.near += 1;
            } else {
                report.fallback += 1;
                fallback_used += 1;
            }
            let meshes = renderer.skin_near(gpu_body, slots[bi], index);
            slots[bi] += 1;
            let list = if me_in_render_space && p.key == FigureKey::Me { &mut *render_space } else { &mut *all_objects };
            for (mesh, material) in meshes.into_iter().zip(materials.iter().copied()) {
                list.push(RenderObject { fade: 0.0, position: p.feet, rotation, scale: Vec3::splat(scale), mesh, material });
            }
        } else if vs_ready {
            report.middle += 1;
            for (region, material) in materials.iter().copied().enumerate() {
                renderer.skin_middle(gpu_body, region, material, index);
            }
        }
    }
    let frame = rt.frame;
    rt.figures.retain(|_, f| f.seen == frame);
    // What a full solve cost this frame, eased (one slow frame does not hand the crowd to the
    // pelvis offset); measured again every frame, so a faster frame raises the count back.
    if full_n > 0 {
        let per = full_us / full_n as f32;
        rt.full_solve_us = if rt.full_solve_us > 0.0 { rt.full_solve_us + (per - rt.full_solve_us) * 0.1 } else { per };
    }
    rt.report = report;
    // The foot probe writes its readout once its time is up.
    if let Some(probe) = rt.probe.as_mut() {
        probe.frames += 1;
        if rt.clock >= probe.until {
            let note = crate::engine::foot_probe::write(probe);
            log::info!("Foot probe: {note}");
            rt.probe = None;
        }
    }
    renderer.skin_submit();
    true
}

/// The player's own height for the character page (rung 2a, "1.62 m"): the chosen body measured
/// with the look's height, build and ancestry (`LoadedBody::standing_height_m` through `shape_for`,
/// so the sliders' lesson that ancestry moves the top of the head is in it). Measured again when the
/// look changes, and every frame with the reuse switch off.
fn publish_my_height(state: &mut EngineState) {
    let Some(lib) = state.animation.lib.as_ref() else { return };
    let app = &state.gui_state.appearance;
    let bi = lib.body_for(Some(&app.body_type), "player", stagger(FigureKey::Me));
    let h = app.height_scale.clamp(0.5, 2.0);
    if humanity_core::reuse::enabled() {
        if let Some((b, hh, s, m)) = &state.animation.my_height {
            if *b == bi && *hh == h && *s == app.shape {
                state.gui_state.characters.my_height_m = Some(*m);
                return;
            }
        }
    }
    let body = &lib.bodies[bi];
    let (weights, scale) = body.shape_for(h, &|id| app.shape.get(id).copied());
    let m = body.standing_height_m(&weights, scale);
    let shape = app.shape.clone();
    state.animation.my_height = Some((bi, h, shape, m));
    state.gui_state.characters.my_height_m = Some(m);
}

#[cfg(test)]
#[path = "animation_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "animation_feet_tests.rs"]
mod feet_tests;
