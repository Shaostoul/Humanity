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
use crate::renderer::mesh_pool::Step;
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
    /// Their face and body detail slider values by id (the look, 2026-11-09; none: the body as made).
    pub detail: std::collections::BTreeMap<String, f32>,
    /// What their look says they wear (hair, brows, lashes, the outfit's cosmetics); what it does
    /// not say is their wearer kind's default.
    pub dress: humanity_animation::DressChoice,
    /// What they are using (rung 2b, engine::interact): an anchor and, when gameplay times it (the
    /// player's grant, a stand-in's), its phase and time; None walks.
    pub act: Option<ActIn>,
    /// Where they look, world (rung 2b): the player's aim, another player's pitch and yaw, a crew
    /// member's point of interest; None looks ahead.
    pub look: Option<Vec3>,
    /// The player's own body drawn for the first-person eye (rung 2b): the head and neck folded
    /// away, the camera riding the head.
    pub first_person: bool,
    /// What each hand holds, as the grip it is held in (left, right; an index into the library's
    /// grips; rung 2b: the two grip sockets), and how far along one shaft the second hand closes
    /// when the main hand's thing takes both hands.
    pub held: [Option<usize>; 2],
    pub second_hand: Option<f32>,
}

/// What a person is using this frame, as gather found it (rung 2b).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ActIn {
    /// The anchor (an index into the library's anchors).
    pub anchor: usize,
    /// The phase and the seconds into it when gameplay times it (the player's grant, a stand-in's);
    /// None for one only known to be on (a ladder, a workbench, a door), whose phases the figure times
    /// (in when it starts, out when it stops), or another player's (their phase from the network,
    /// timed here).
    pub phase: Option<humanity_animation::Phase>,
    pub t: Option<f32>,
    /// Where it stands in the world, when known (the player's, a stand-in's, a ladder, a workbench, a
    /// door found near the person); None for another player's seat, posed from the anchor's own row.
    pub placed: Option<crate::engine::interact::Placed>,
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
    /// Using an anchor (the `crowd_act` verb, rung 2b): where it stands, when the stand-in began, and
    /// when it gets up again (None: stays).
    pub act: Option<CrowdAct>,
}

/// A stand-in's interaction (the `crowd_act` verb, rung 2b): it walks in from the approach point,
/// holds, and gets up after `hold_s` seconds when given.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CrowdAct {
    pub placed: crate::engine::interact::Placed,
    pub hold_s: Option<f32>,
}

/// A stand-in's own body and shape (the `lineup` verb).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CrowdLook {
    pub body: String,
    pub shape: std::collections::BTreeMap<String, f32>,
    /// Detail slider values (the look, 2026-11-09: a spec's `d:<id>=v`).
    pub detail: std::collections::BTreeMap<String, f32>,
    /// What it wears (a spec's `hair=`, `brows=`, `lashes=`, `wear:<slot>=<cosmetic id>`).
    pub dress: humanity_animation::DressChoice,
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
    /// The look (2026-11-09): people drawn with their own mesh this frame, how many were built, what
    /// the builds cost (milliseconds, all of them), the slowest one, and the own meshes' GPU bytes.
    pub own: usize,
    pub own_built: usize,
    pub own_build_ms: f32,
    pub own_build_max_ms: f32,
    pub own_bytes: u64,
    /// The own-mesh pool this frame (2026-11-10): kept meshes let go to make room, people moved
    /// down to pack it, and finished meshes left waiting for room.
    pub own_evicted: usize,
    pub own_packed: usize,
    pub own_waiting: usize,
    /// People in an interaction this frame (rung 2b: seated, lying, climbing, at a station, at a
    /// door's handle).
    pub acting: usize,
    pub error: Option<String>,
}

/// What became of a finished own mesh that did not go into the own-mesh pool this frame.
enum OwnOutcome {
    /// No pool yet (the frame before it is made): it waits.
    NoPool,
    /// The pool refused a placement it had said would fit (not expected): it waits.
    Refused(crate::renderer::mesh_pool::Refusal),
    /// This frame's upload packed the pool to make room: it goes next frame.
    Packing,
    /// The pool is full of people near: it waits, drawn with the shared body.
    Waiting,
    /// Bigger than the pool was planned for: drawn with the shared body (logged).
    TooBig,
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
    /// Their own shaped and dressed mesh and its GPU body (the look, 2026-11-09), while they are near
    /// (and for a few seconds after, so walking the near line does not build them again and again).
    own: Option<OwnBody>,
    /// Seconds since they last wanted their own mesh (near, the player, the showroom).
    away_s: f32,
    /// Their own mesh building on a worker thread, off the frame (2026-11-10).
    building: Option<Building>,
    /// A finished own mesh waiting for room in the own-mesh pool (2026-11-10: its upload went to
    /// packing the pool, or the pool is full of people near), with what it took and whether it was
    /// built on a worker; tried again on a later frame, drawn with the shared body (or their last
    /// own mesh) meanwhile.
    pending: Option<(humanity_animation::PersonMesh, f32, bool)>,
    /// The version of their look whose mesh was bigger than the own-mesh pool is planned for: drawn
    /// with the shared body and not built again until the look or the pool's plan changes (logged
    /// once).
    too_big: Option<u64>,
    /// Their interaction from frame to frame (rung 2b): the anchor, its phase and time, and where it
    /// stands; it plays its exit out after the act stops. State, like a position.
    track: Option<Track>,
    /// Their head's turn toward what they look at (rung 2b; state, like a position).
    look: humanity_animation::LookState,
    /// How far under the hips joint their seat flesh stands in an anchor's pose (the warp's measure),
    /// with the anchor and the version of their mesh it was measured on (a value keyed by its own
    /// inputs; measured again every frame with the reuse switch off).
    seat_drop: Option<(usize, u64, f32)>,
    /// The first-person view's ride on the head, low-passed (the player's figure only).
    view_offset: Option<Vec3>,
}

/// A figure's interaction from frame to frame.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Track {
    anchor: usize,
    phase: humanity_animation::Phase,
    t: f32,
    placed: Option<crate::engine::interact::Placed>,
    /// Timed by gameplay (the player's grant, a stand-in's): it ends when the act does, its exit
    /// already played.
    timed: bool,
}

/// One frame of a figure's interaction track from the person's act: a timed act is copied; one
/// whose phase comes from the network is timed here within that phase; one only known to be on (a
/// ladder, a workbench, a door) enters, holds while on, and plays its exit out once off.
fn update_track(track: &mut Option<Track>, act: Option<&ActIn>, lib: &CharacterLibrary, clips: &humanity_animation::ClipSet, dt: f32) {
    use humanity_animation::Phase;
    let len = |anchor: usize, phase: Phase| humanity_animation::interact::phase_len(&lib.anchors.anchors[anchor], phase, clips);
    match act {
        Some(a) => {
            let same = track.as_ref().is_some_and(|t| t.anchor == a.anchor);
            match (a.phase, a.t) {
                (Some(phase), Some(t)) => *track = Some(Track { anchor: a.anchor, phase, t, placed: a.placed, timed: true }),
                (Some(phase), None) => match track.as_mut() {
                    Some(tr) if same && tr.phase == phase => {
                        tr.t += dt;
                        if phase != Phase::Hold {
                            tr.t = tr.t.min(len(a.anchor, phase));
                        }
                        tr.placed = a.placed.or(tr.placed);
                    }
                    _ => *track = Some(Track { anchor: a.anchor, phase, t: 0.0, placed: a.placed, timed: false }),
                },
                (None, _) => match track.as_mut() {
                    Some(tr) if same => {
                        tr.t += dt;
                        if tr.phase == Phase::Exit {
                            // On again while leaving: in again from the start of its way in.
                            tr.phase = Phase::Enter;
                            tr.t = 0.0;
                        } else if tr.phase == Phase::Enter && tr.t >= len(a.anchor, Phase::Enter) {
                            tr.phase = Phase::Hold;
                            tr.t = 0.0;
                        }
                        tr.placed = a.placed.or(tr.placed);
                    }
                    _ => *track = Some(Track { anchor: a.anchor, phase: Phase::Enter, t: 0.0, placed: a.placed, timed: false }),
                },
            }
        }
        None => {
            let done = match track.as_mut() {
                None => false,
                Some(tr) if tr.timed => true,
                Some(tr) if tr.phase != Phase::Exit => {
                    tr.phase = Phase::Exit;
                    tr.t = 0.0;
                    false
                }
                Some(tr) => {
                    tr.t += dt;
                    tr.t >= len(tr.anchor, Phase::Exit)
                }
            };
            if done {
                *track = None;
            }
        }
    }
}

/// A person's own mesh building on a worker thread: where the finished mesh and what it took
/// (milliseconds) arrive.
struct Building {
    rx: std::sync::mpsc::Receiver<(humanity_animation::PersonMesh, f32)>,
}

/// A person's own mesh (`humanity_animation::look::build_person`) and the renderer's id for it (a
/// handle of the own-mesh pool, 2026-11-10).
struct OwnBody {
    mesh: humanity_animation::PersonMesh,
    /// The body (index into the library's bodies) it was built on.
    body: usize,
    gpu: usize,
    /// Its GPU bytes in the pool (vertices, their skinned copy and indices), for the report.
    bytes: u64,
}

/// A person's own mesh is released once they have not wanted it for this long (seconds).
const OWN_KEEP_S: f32 = 5.0;

/// A garment's texture gives its slot back once nobody near has worn it for this long (seconds):
/// the pool holds the textures of what people around you wear, not of every garment seen this
/// session (2026-11-10: kept for the session, a rig's sweep reached 300 MB of garment textures, and
/// the library only grows).
const GARMENT_KEEP_S: f32 = 30.0;

/// A garment nobody near has worn for this long (seconds) is idle: when its pool is full, a garment
/// someone near wants may take its slot before its `GARMENT_KEEP_S` is up.
const GARMENT_IDLE_S: f32 = 1.0;

/// A worn garment's texture (the look; engine::look_textures, renderer::texture_pool): read off the
/// frame on a thread of its own the first time anyone wears it, written into a slot of its size's
/// pool, then drawn through its template material.
enum GarmentTex {
    /// Being read. `template` is the material it had before it was released (that material and the
    /// garment's others get the new slot, so their indices stay), None the first time.
    Loading { rx: std::sync::mpsc::Receiver<Result<crate::engine::look_textures::Loaded, String>>, template: Option<usize> },
    /// Read, waiting for a slot: its pool is full of garments someone near is wearing (drawn in its
    /// mean colour meanwhile, the middle tier's tint), or this frame's upload went to another.
    Waiting { loaded: crate::engine::look_textures::Loaded, template: Option<usize> },
    /// In a slot of pool `pool` (an index into `AnimationRuntime::pools`, which knows the slot);
    /// `used` is the runtime clock when someone near last wore it.
    Ready { template: usize, pool: usize, used: f32 },
    /// Let go after `GARMENT_KEEP_S` unworn (or its idle slot taken): its materials remain, without
    /// a texture, until it is worn again.
    Released { template: usize },
    Failed,
}

/// What the garment textures are loaded at: each half's most texels (Settings > Graphics > Garment
/// textures) and whether the GPU takes BC7.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TexSize {
    max_px: u32,
    compressed: bool,
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
    /// Shared with the look's build workers (each holds it while it builds one person).
    lib: Option<std::sync::Arc<CharacterLibrary>>,
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
    /// The look's A/B (showcase `own_mesh`, 2026-11-09): false draws everyone with the shared body
    /// as main did (shape on the GPU, clothes as tinted regions), for a same-boot measure.
    pub(crate) look_on: bool,
    /// The poke-through check's red arm (showcase `covers` 0): garments hide nothing under them.
    pub(crate) covers_on: bool,
    /// The foot probe's red arm for shoes (showcase `soles` 0): the soles of the body as made, so a
    /// shod foot sinks by its sole.
    pub(crate) soles_on: bool,
    /// The textures' A/B (showcase `garment_textures` 0): garments drawn in their mean colour, for a
    /// same-boot measure of what the textured and hair materials cost.
    pub(crate) textures_on: bool,
    /// Each worn garment's texture, by garment index (forgotten when the library is read again).
    garment_tex: HashMap<usize, GarmentTex>,
    /// Materials over the garments' textures, by (garment, colour, pass: 0 cloth or hair core, 1
    /// hair fringe): a GPU allocation keyed by its own value, made once each, like `materials`.
    garment_materials: HashMap<(usize, [u8; 3], u8), usize>,
    /// The garment texture pools, one per slot shape (renderer::texture_pool), each allocated on
    /// the GPU once, when a garment of its shape first arrives.
    pools: Vec<crate::renderer::texture_pool::SlotPool>,
    /// What the garment textures are loaded at; a change (the setting) lets every one go and the
    /// pools with them, to be read again at the new size.
    tex_size: Option<TexSize>,
    /// What the player's body shows this frame (rung 2b): the anchor's id and its phase, for the
    /// movement message.
    me_act: Option<(String, humanity_animation::Phase)>,
    pose2: Pose,
    scratch: Pose,
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
            look_on: true,
            covers_on: true,
            soles_on: true,
            textures_on: true,
            garment_tex: HashMap::new(),
            garment_materials: HashMap::new(),
            pools: Vec::new(),
            tex_size: None,
            me_act: None,
            pose2: Pose::default(),
            scratch: Pose::default(),
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

    /// The character library, when it has been read (rung 2b: the interactions read its anchors).
    pub(crate) fn library(&self) -> Option<std::sync::Arc<CharacterLibrary>> {
        self.lib.clone()
    }

    /// What the player's body shows this frame (rung 2b): the anchor's id and its phase.
    pub(crate) fn my_act(&self) -> Option<(String, humanity_animation::Phase)> {
        self.me_act.clone()
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
    // The look's binary files (the base meshes, the detail targets, the garments), read whole.
    let mut read_bytes = |rel: &str| {
        let p = resolve_model(data_dir, rel);
        std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))
    };
    CharacterLibrary::load(data_dir, &mut read, &mut read_bytes)
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
            // A new graph may have other states: every figure starts again (their own meshes' GPU
            // bodies released first, unless every body was just cleared).
            if rt.lib.as_ref().map_or(true, |old| old.graph != lib.graph) {
                for f in rt.figures.values_mut() {
                    if let (Some(own), true) = (f.own.take(), same_models) {
                        state.renderer.skin_release_body(own.gpu);
                    }
                }
                rt.figures.clear();
            }
            // A reshaped skeleton and a person's own mesh belong to the library they were made from:
            // made again from this one (an own mesh's GPU body is released, unless every body was
            // just cleared with the old models).
            for f in rt.figures.values_mut() {
                f.shape = None;
                f.building = None;
                f.pending = None;
                f.too_big = None;
                if let Some(own) = f.own.take() {
                    if same_models {
                        state.renderer.skin_release_body(own.gpu);
                    }
                }
            }
            // The own-mesh pool was planned for the old library (its largest dressed person): let
            // go, and planned again from this one on the next frame that draws bodies.
            state.renderer.skin_drop_own_pool();
            // Garment indices belong to the library they were read from: every texture lets its slot
            // go, and the pools go with them (sized from the old library); the new library's pools
            // are planned for the Settings page.
            let_go_of_garment_textures(rt, &mut state.renderer);
            rt.garment_tex.clear();
            rt.garment_materials.clear();
            if let Some(look) = lib.look.as_ref() {
                crate::gui::garment_textures::note_plan(pool_plan(look, lib.characters.tiers.near_cap, state.renderer.block_compression()));
            }
            // What the own-mesh pool will take (Settings shows it beside the garment textures).
            if let Some(plan) = own_pool_plan(&lib) {
                crate::gui::garment_textures::note_mesh_plan(crate::gui::garment_textures::MeshPlan { bytes: plan.bytes(&lib), people: plan.people });
            }
            rt.report.body = lib.bodies.iter().map(|b| b.def.id.as_str()).collect::<Vec<_>>().join(",");
            rt.report.error = None;
            // The character page's body chooser lists the bodies a look can choose, and draws the
            // chosen body's shape sliders.
            state.gui_state.characters.body_choices = lib.characters.bodies.keys().cloned().collect();
            state.gui_state.characters.body_sliders = lib.bodies.iter().filter(|b| lib.characters.bodies.contains_key(&b.choice)).map(|b| (b.choice.clone(), b.slider_blocks.clone())).collect();
            // The look (2026-11-09): each body's detail sliders in the page's groups, and the hair
            // styles, brows and lashes a character can wear.
            state.gui_state.characters.detail_sliders = match lib.look.as_ref() {
                Some(look) => lib.characters.bodies.keys().map(|choice| (choice.clone(), detail_blocks(look, choice))).collect(),
                None => Default::default(),
            };
            state.gui_state.characters.wearables = match lib.look.as_ref() {
                Some(look) => [("hair", humanity_animation::GarmentKind::Hair), ("brows", humanity_animation::GarmentKind::Brows), ("lashes", humanity_animation::GarmentKind::Lashes)]
                    .into_iter()
                    .map(|(name, kind)| (name.to_string(), look.garments.iter().filter(|g| g.def.kind == kind).map(|g| g.def.id.clone()).collect()))
                    .collect(),
                None => Default::default(),
            };
            rt.lib = Some(std::sync::Arc::new(lib));
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

/// A body's detail sliders as the character page draws them: in details.ron's group order, each
/// group's sliders in the file's order, only those for this body.
pub(crate) fn detail_blocks(look: &humanity_animation::LookLibrary, choice: &str) -> Vec<humanity_core::player_look::DetailBlock> {
    use humanity_core::player_look::{DetailBlock, DetailSliderView};
    look.details_def
        .groups
        .iter()
        .map(|g| DetailBlock {
            group: g.clone(),
            sliders: look
                .sliders_for(choice)
                .filter(|s| &s.group == g)
                .map(|s| DetailSliderView { id: s.id.clone(), low: s.ends.0.clone(), high: s.ends.1.clone(), single: s.single() })
                .collect(),
        })
        .filter(|b| !b.sliders.is_empty())
        .collect()
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
                CrowdFigure { motion, phase, born: 0.0, look: None, crouch: false, act: None }
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
            let mut detail = std::collections::BTreeMap::new();
            let mut dress = humanity_animation::DressChoice::default();
            if *s != "-" {
                for kv in s.split(',') {
                    let (k, v) = kv.split_once('=')?;
                    let (k, v) = (k.trim(), v.trim());
                    // The look (2026-11-09): what it wears by id, and its detail sliders.
                    let plain = |x: &str| humanity_core::player_look::clean_shape_id(x);
                    match k {
                        "hair" => dress.hair = Some(plain(v)),
                        "brows" => dress.brows = Some(plain(v)),
                        "lashes" => dress.lashes = Some(plain(v)),
                        _ if k.starts_with("wear:") => {
                            dress.outfit.insert(plain(&k[5..]), plain(v));
                        }
                        _ => {
                            let (detailed, name) = match k.strip_prefix("d:") {
                                Some(rest) => (true, rest),
                                None => (false, k),
                            };
                            let id = plain(name);
                            let v: f32 = v.parse().ok()?;
                            if id.is_empty() || !v.is_finite() {
                                return None;
                            }
                            if detailed {
                                detail.insert(id, v);
                            } else {
                                shape.insert(id, v);
                            }
                        }
                    }
                }
            }
            let at = feet + fwd * ahead + right * ((i as f32 - (n - 1.0) * 0.5) * gap);
            Some(CrowdFigure { motion: CrowdMotion::Turn { at, rate: 0.0 }, phase, born: 0.0, look: Some(CrowdLook { body: body.clone(), shape, detail, dress }), crouch: false, act: None })
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
    (points.len() >= 2 && speed > 0.0).then_some(CrowdFigure { motion: CrowdMotion::Path { points, speed }, phase: 0.0, born: 0.0, look: None, crouch: false, act: None })
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

/// A colour as a material key (64 steps a channel).
fn colour_key(c: [f32; 3]) -> [u8; 3] {
    c.map(|x| (x.clamp(0.0, 1.0) * 63.0).round() as u8)
}

/// A plain character material of this colour and roughness: made once each, keyed by its value.
fn tinted_material(materials: &mut HashMap<[u8; 4], usize>, renderer: &mut crate::renderer::Renderer, c: [f32; 3], roughness: f32) -> usize {
    let q = colour_key(c);
    let key = [q[0], q[1], q[2], (roughness.clamp(0.0, 1.0) * 63.0).round() as u8];
    *materials.entry(key).or_insert_with(|| renderer.add_material_full([c[0], c[1], c[2], 1.0], 0.0, roughness, crate::renderer::materials::MATERIAL_TYPE_CHARACTER, 0.0))
}

/// A worn garment's texture (the look; engine::look_textures, renderer::texture_pool): its template
/// material (white, the garment's own type), whether it has a normal map and its colour's mean
/// luminance, once it sits in a slot; None while it is read (it starts reading here, off the frame,
/// the first time), while it waits for a slot, or when it failed (logged once). `uploads`: how many
/// slots this frame may still write (one: a texture is up to 2.7 MB of BC7 with its levels).
#[allow(clippy::too_many_arguments)]
fn garment_texture(
    tex: &mut HashMap<usize, GarmentTex>,
    shared: &HashMap<(usize, [u8; 3], u8), usize>,
    pools: &mut Vec<crate::renderer::texture_pool::SlotPool>,
    renderer: &mut crate::renderer::Renderer,
    data_dir: &Path,
    look: &humanity_animation::LookLibrary,
    near_cap: usize,
    g: usize,
    size: TexSize,
    clock: f32,
    uploads: &mut usize,
) -> Option<(usize, bool, f32)> {
    use crate::renderer::texture_pool::{SlotPool, Take};
    let def = &look.garments[g].def;
    let start = |template: Option<usize>| {
        let path = resolve_model(data_dir, &def.texture);
        let (texture_px, id) = (def.texture_px, def.id.clone());
        let (tx, rx) = std::sync::mpsc::channel();
        let spawned = std::thread::Builder::new().name("garment-texture".into()).spawn(move || {
            let _ = tx.send(crate::engine::look_textures::read(&path, texture_px, size.max_px, size.compressed));
        });
        match spawned {
            Ok(_) => GarmentTex::Loading { rx, template },
            Err(e) => {
                log::warn!("Characters: could not start reading garment {id}'s texture ({e}); drawn in its colour");
                GarmentTex::Failed
            }
        }
    };
    let entry = tex.entry(g).or_insert_with(|| start(None));
    // Worn again after it was let go: read it again, into the materials it had.
    if let GarmentTex::Released { template } = *entry {
        *entry = start(Some(template));
    }
    // A finished read waits for a slot.
    if let GarmentTex::Loading { rx, template } = entry {
        match rx.try_recv() {
            Ok(Ok(loaded)) => *entry = GarmentTex::Waiting { loaded, template: *template },
            Ok(Err(e)) => {
                log::warn!("Characters: garment {}'s texture did not read ({e}); drawn in its colour", def.id);
                *entry = GarmentTex::Failed;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => *entry = GarmentTex::Failed,
        }
    }
    // Into a slot while this frame's upload is free: a free slot, else an idle garment's (nobody
    // near has worn it for `GARMENT_IDLE_S`), else it waits in its mean colour. Never an allocation
    // on the frame, but the pool's own, once, the first time a garment of its shape arrives.
    if *uploads > 0 && matches!(tex.get(&g), Some(GarmentTex::Waiting { .. })) {
        let idle: Vec<(usize, f32)> = tex
            .iter()
            .filter_map(|(&k, t)| match t {
                GarmentTex::Ready { used, .. } if clock - *used > GARMENT_IDLE_S => Some((k, clock - *used)),
                _ => None,
            })
            .collect();
        let Some(GarmentTex::Waiting { loaded, template }) = tex.remove(&g) else { return None };
        let pi = match pools.iter().position(|p| p.class == loaded.class) {
            Some(i) => i,
            None => {
                pools.push(SlotPool::new(loaded.class, pool_capacity(look, near_cap, loaded.class, size.max_px)));
                pools.len() - 1
            }
        };
        match pools[pi].take(g, |h| idle.iter().find(|(k, _)| *k == h).map(|(_, s)| *s)) {
            Take::Full => {
                tex.insert(g, GarmentTex::Waiting { loaded, template });
            }
            Take::Slot { layer, evicted } => {
                *uploads -= 1;
                // What the frame pays for the look (with the meshes' uploads): the texture into its slot.
                let _cost = humanity_core::frame_costs::stage("cpu.look_build");
                let t0 = std::time::Instant::now();
                // The idle garment whose slot this is lets it go first.
                if let Some(e) = evicted {
                    if let Some(&GarmentTex::Ready { template: et, .. }) = tex.get(&e) {
                        drop_garment_materials(renderer, shared, e, et);
                        tex.insert(e, GarmentTex::Released { template: et });
                    }
                }
                let pool = &mut pools[pi];
                // Allocated when bodies were first drawn (`allocate_pools`); a shape the library did
                // not plan (none today) is allocated here, once.
                if pool.needs_texture() {
                    let ta = std::time::Instant::now();
                    pool.gpu = Some(renderer.create_texture_pool(pool.class, pool.capacity));
                    log_pool(pool, ta.elapsed().as_secs_f32() * 1000.0);
                }
                // Always set by now (allocated just above when it was not); a pool without its
                // texture leaves the garment in its mean colour rather than a stale entry.
                let Some(gpu) = pool.gpu else {
                    pool.give_back(g);
                    tex.insert(g, GarmentTex::Failed);
                    return None;
                };
                if let Err(e) = renderer.texture_pool_write(gpu, layer, &loaded.levels) {
                    log::warn!("Characters: garment {}'s texture did not go into its slot ({e}); drawn in its colour", def.id);
                    pool.give_back(g);
                    tex.insert(g, GarmentTex::Failed);
                    return None;
                }
                use crate::renderer::materials::{HAIR_CUTOFF, MATERIAL_TYPE_CHARACTER, MATERIAL_TYPE_HAIR};
                let hn = if def.normal_map { 1.0 } else { 0.0 };
                let (ty, params2) = if def.alpha { (MATERIAL_TYPE_HAIR, [1.0, hn, HAIR_CUTOFF, def.luma]) } else { (MATERIAL_TYPE_CHARACTER, [1.0, hn, 0.0, 0.0]) };
                let template = match template {
                    Some(t) => {
                        renderer.set_material_pool_slot(t, gpu, layer, params2);
                        for (&(of, _, _), &m) in shared.iter() {
                            if of == g && m != t {
                                renderer.share_material_texture(m, t);
                            }
                        }
                        t
                    }
                    None => renderer.add_character_material_on_slot([1.0; 4], def.roughness, ty, params2, gpu, layer),
                };
                let bytes: usize = loaded.levels.iter().map(Vec::len).sum();
                log::info!(
                    "Characters: garment {} textured, {} by {} texels ({:.1} MB with its levels, {}), into slot {} of {} ({} held), onto the GPU in {:.2} ms",
                    def.id,
                    loaded.class.width,
                    loaded.class.height,
                    bytes as f64 / 1_048_576.0,
                    if loaded.class.compressed { "BC7" } else { "RGBA8" },
                    layer,
                    pool.capacity,
                    pool.held(),
                    t0.elapsed().as_secs_f32() * 1000.0
                );
                tex.insert(g, GarmentTex::Ready { template, pool: pi, used: clock });
            }
        }
    }
    match tex.get_mut(&g) {
        Some(GarmentTex::Ready { template, used, .. }) => {
            *used = clock;
            Some((*template, def.normal_map, def.luma))
        }
        _ => None,
    }
}

/// Let a garment's materials go of their slot: its template and every material sharing its texture
/// (other colours, hair's fringe pass).
fn drop_garment_materials(renderer: &mut crate::renderer::Renderer, shared: &HashMap<(usize, [u8; 3], u8), usize>, g: usize, template: usize) {
    for (&(of, _, _), &m) in shared.iter() {
        if of == g {
            renderer.drop_material_texture(m);
        }
    }
    renderer.drop_material_texture(template);
}

/// Let go of the textures of garments nobody near has worn for `GARMENT_KEEP_S`: their materials
/// let the slot go and the slot goes back to its pool.
fn release_unworn_garments(tex: &mut HashMap<usize, GarmentTex>, shared: &HashMap<(usize, [u8; 3], u8), usize>, pools: &mut [crate::renderer::texture_pool::SlotPool], renderer: &mut crate::renderer::Renderer, clock: f32) {
    for (&g, entry) in tex.iter_mut() {
        if let GarmentTex::Ready { template, pool, used, .. } = *entry {
            if clock - used > GARMENT_KEEP_S {
                drop_garment_materials(renderer, shared, g, template);
                if let Some(p) = pools.get_mut(pool) {
                    p.give_back(g);
                }
                *entry = GarmentTex::Released { template };
            }
        }
    }
}

/// Let every garment texture go, and the pools with them (the garment texture size changed, or the
/// library is read again): each garment keeps its template material, textureless, to be read again
/// into it when worn.
fn let_go_of_garment_textures(rt: &mut AnimationRuntime, renderer: &mut crate::renderer::Renderer) {
    for &m in rt.garment_materials.values() {
        renderer.drop_material_texture(m);
    }
    let mut kept = HashMap::new();
    for (g, t) in rt.garment_tex.drain() {
        let template = match t {
            GarmentTex::Ready { template, .. } | GarmentTex::Released { template } => Some(template),
            GarmentTex::Loading { template, .. } | GarmentTex::Waiting { template, .. } => template,
            GarmentTex::Failed => None,
        };
        if let Some(template) = template {
            renderer.drop_material_texture(template);
            kept.insert(g, GarmentTex::Released { template });
        }
    }
    rt.garment_tex = kept;
    for p in rt.pools.drain(..) {
        if let Some(gpu) = p.gpu {
            renderer.release_texture_pool(gpu);
        }
    }
}

/// How many slots a pool of `class` holds: enough for the near tier (its cap and the player) each
/// wearing one garment of every kind that comes in this shape, and never more than the library has
/// garments of it. A pool full of garments someone near wears makes a newcomer wait in its mean
/// colour, so this is the near tier's worst case, not a limit people meet.
fn pool_capacity(look: &humanity_animation::LookLibrary, near_cap: usize, class: crate::renderer::texture_pool::SlotClass, max_px: u32) -> u32 {
    let mut kinds: Vec<humanity_animation::GarmentKind> = Vec::new();
    let mut count = 0usize;
    for g in &look.garments {
        if crate::engine::look_textures::class_for(g.def.texture_px, max_px, class.compressed) == class {
            count += 1;
            if !kinds.contains(&g.def.kind) {
                kinds.push(g.def.kind);
            }
        }
    }
    count.min((near_cap + 1) * kinds.len()).max(1) as u32
}

/// The own-mesh pool's plan (2026-11-10, renderer::mesh_pool): sized for the largest dressed person
/// the library can make (`CharacterLibrary::largest_own_mesh`), the near tier (its cap and the
/// player) at that size at once, as the garment texture pools are; a place in its books for every
/// own mesh the engine keeps (the near tier and `own_keep_cap` people who just left it); and a full
/// near tier's near places for each shared body (people near waiting for their own builds are drawn
/// with it), made with the pool.
pub(crate) struct OwnPoolPlan {
    pub class: crate::renderer::mesh_pool::MeshClass,
    pub people: u32,
    pub handles: u32,
    pub shared_places: usize,
}

impl OwnPoolPlan {
    /// The GPU bytes it sets aside: the pool's buffers, its places' parameters and the shared
    /// bodies' near places (their skinned copy and parameters), as the renderer makes them.
    pub fn bytes(&self, lib: &CharacterLibrary) -> u64 {
        let pool = crate::renderer::mesh_pool::MeshPool::new(self.class, self.people, self.handles);
        let shared: u64 = lib.bodies.iter().map(|b| (b.model.vertex_count().max(1) as u64 * 32 + 16) * self.shared_places as u64).sum();
        pool.bytes() + pool.handles() as u64 * 16 + shared
    }
}

/// The environment variable that plans the own-mesh pool smaller than the library's largest dressed
/// person (`own_pool_plan`): the oversized path's arm in a rig.
pub(crate) const OWN_POOL_VERTICES_VAR: &str = "HUMANITY_OWN_POOL_VERTICES";

/// Own meshes kept for people who left the near tier stay at most this many (the oldest go first).
fn own_keep_cap(near_cap: usize) -> usize {
    near_cap * 2
}

/// The own-mesh pool's plan for this library; None without the look.
pub(crate) fn own_pool_plan(lib: &CharacterLibrary) -> Option<OwnPoolPlan> {
    let (vertices, indices) = lib.largest_own_mesh()?;
    // The oversized arm in the running game (dev tooling, 2026-11-10): `HUMANITY_OWN_POOL_VERTICES=n`
    // plans the pool for people of at most n vertices, so everyone dressed past it is drawn with the
    // shared body and logged, the path a garment larger than the plan takes.
    let vertices = std::env::var(OWN_POOL_VERTICES_VAR).ok().and_then(|v| v.trim().parse::<u32>().ok()).map_or(vertices, |cap| cap.min(vertices));
    let near_cap = lib.characters.tiers.near_cap;
    let people = near_cap as u32 + 1;
    Some(OwnPoolPlan { class: crate::renderer::mesh_pool::MeshClass { vertices, indices }, people, handles: people + own_keep_cap(near_cap) as u32, shared_places: people as usize })
}

/// Make and allocate a pool for every slot shape the library's garments fall in at `size` (each at
/// its capacity), each logged with its size and the time its allocation took.
fn allocate_pools(pools: &mut Vec<crate::renderer::texture_pool::SlotPool>, renderer: &mut crate::renderer::Renderer, look: &humanity_animation::LookLibrary, near_cap: usize, size: TexSize) {
    for g in &look.garments {
        let class = crate::engine::look_textures::class_for(g.def.texture_px, size.max_px, size.compressed);
        if pools.iter().any(|p| p.class == class) {
            continue;
        }
        let mut pool = crate::renderer::texture_pool::SlotPool::new(class, pool_capacity(look, near_cap, class, size.max_px));
        if pool.needs_texture() {
            let t0 = std::time::Instant::now();
            pool.gpu = Some(renderer.create_texture_pool(pool.class, pool.capacity));
            log_pool(&pool, t0.elapsed().as_secs_f32() * 1000.0);
        }
        pools.push(pool);
    }
}

fn log_pool(pool: &crate::renderer::texture_pool::SlotPool, ms: f32) {
    log::info!(
        "Characters: garment texture pool of {} slots of {} by {} texels ({} levels, {}) allocated: {:.1} MB, in {:.2} ms",
        pool.capacity,
        pool.class.width,
        pool.class.height,
        pool.class.levels,
        if pool.class.compressed { "BC7" } else { "RGBA8: no block compression" },
        (pool.class.slot_bytes() * pool.capacity as u64) as f64 / 1_048_576.0,
        ms
    );
}

/// What the pools take at each garment texture level (Settings > Graphics > Garment textures), on a
/// GPU that does (`compressed`) or does not take BC7: every pool shape the library's garments fall
/// in at that level, at its capacity.
fn pool_plan(look: &humanity_animation::LookLibrary, near_cap: usize, compressed: bool) -> crate::gui::garment_textures::Plan {
    let bytes = crate::gui::garment_textures::levels()
        .iter()
        .map(|level| {
            let mut classes: Vec<crate::renderer::texture_pool::SlotClass> = Vec::new();
            for g in &look.garments {
                let c = crate::engine::look_textures::class_for(g.def.texture_px, level.max_px, compressed);
                if !classes.contains(&c) {
                    classes.push(c);
                }
            }
            let total = classes.iter().map(|&c| c.slot_bytes() * pool_capacity(look, near_cap, c, level.max_px) as u64).sum();
            (level.id.clone(), total)
        })
        .collect();
    crate::gui::garment_textures::Plan { bytes, compressed }
}

/// The outfit slots a worn garment of this kind fills: the body's regions painted as those slots'
/// clothes are the body under it (the look, 2026-11-09). MakeHuman's clothes are whole outfits, top
/// and trousers in one.
pub(crate) fn garment_slots(kind: humanity_animation::GarmentKind) -> &'static [&'static str] {
    use humanity_animation::GarmentKind as K;
    match kind {
        K::Clothes => &["chest", "legs"],
        K::Shoes => &["feet"],
        K::Hat => &["head"],
        K::Hair | K::Brows | K::Lashes => &[],
    }
}

/// A garment's colour for a person: hair, brows and lashes in their hair colour (lashes darker, as
/// lashes read), anything else its texture's mean colour (`garments.ron`).
pub(crate) fn garment_colour(lib: &CharacterLibrary, def: &humanity_animation::look::GarmentDef, p: &Person) -> [f32; 3] {
    use humanity_animation::GarmentKind as K;
    let hair = p.hair.or(lib.wearer(p.wearer).map(|w| w.hair)).unwrap_or([0.2, 0.15, 0.1]);
    match def.kind {
        K::Hair | K::Brows => hair,
        K::Lashes => hair.map(|c| c * 0.35),
        _ => [def.colour.0, def.colour.1, def.colour.2],
    }
}

/// Everyone to draw this frame.
fn gather(state: &EngineState, showroom: bool) -> Vec<Person> {
    // People with no look of their own take varied shapes, unless the A/B (showcase
    // `varied_shapes` 0) draws them all as the body as made, as before the sliders.
    let varied = state.animation.varied_shapes;
    let no_look = |key: FigureKey| if varied { ShapeValues::Varied(figure_seed(key)) } else { ShapeValues::Look(Default::default()) };
    let lib = state.animation.lib.clone();
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
            detail: app.detail.clone(),
            dress: my_dress(app, &state.gui_state.outfit),
            act: None,
            look: None,
            first_person: false,
            held: [None, None],
            second_hand: None,
        }
    };
    if showroom {
        out.push(me(FigureKey::Showroom, state.avatar_base + Vec3::new(0.0, SHOWROOM_PODIUM_M, 0.0), 0.0, Vec3::Y, false, false));
        return out;
    }
    let third = state.camera.mode == crate::renderer::camera::CameraMode::ThirdPerson;
    // TRUE FIRST PERSON (rung 2b): the same body for the first-person eye, its head and neck folded
    // away, unless Settings' "Show my body in first person" is off (the old first-person view).
    let first = state.camera.mode == crate::renderer::camera::CameraMode::FirstPerson && state.gui_state.settings.fp_body && state.interact.rung2b_on;
    if (third || first) && !state.gui_state.construction_active {
        // On a planet the way up is the local radial (rung 2a): the body stands on the ground under
        // the eye and faces the way the camera looks along the ground.
        let (up, facing) = if state.camera.surface_mode {
            let up = state.camera.surface_up.normalize_or_zero();
            (up, facing_about(up, state.camera.forward()))
        } else {
            (Vec3::Y, facing_for_camera_yaw(state.camera.yaw))
        };
        let c = &state.controller;
        let mut p = me(FigureKey::Me, state.camera.position - up * c.eye_height(), facing, up, c.crouching(), c.airborne());
        p.first_person = first;
        // What they hold (rung 2b: the two grip sockets), each in its grip; a thing that takes both
        // hands closes the left hand on its shaft too.
        if let Some(lib) = lib.as_deref() {
            let equip = state.data_store.get::<humanity_systems::economy::EquipmentRegistry>("equipment_registry");
            let grip_of = |slot: &str| state.gui_state.outfit.equipped.get(slot).and_then(|id| equip.and_then(|r| r.get(id))).and_then(|d| lib.grips.index(&d.grip));
            let (r, l) = (grip_of("grip_r"), grip_of("grip_l"));
            p.second_hand = r.and_then(|g| lib.grips.grips[g].second_hand_m);
            p.held = [if p.second_hand.is_some() { r } else { l }, r];
        }
        // Their head follows the camera's aim (rung 2b): the point a few metres along the view.
        p.look = Some(state.camera.position + state.camera.forward() * 6.0);
        if let Some(lib) = lib.as_deref().filter(|_| state.interact.rung2b_on) {
            p.act = my_act_in(state, lib, &mut p);
        }
        out.push(p);
    }
    let world = &state.game_world.world;
    for (_e, (t, r)) in world.query::<(&humanity_core::ecs::components::Transform, &crate::net::sync::RemotePlayer)>().iter() {
        if crate::engine::block::hides_player(&state.gui_state, r) {
            continue;
        }
        let yaw = 2.0 * t.rotation.y.atan2(t.rotation.w);
        let feet = t.position - Vec3::new(0.0, humanity_core::ecs::components::EYE_HEIGHT_M as f32, 0.0);
        let mut p = Person {
            key: FigureKey::Player(r.player_id),
            feet,
            facing: facing_for_camera_yaw(yaw),
            height: r.look.as_ref().map_or(1.0, |l| l.height),
            // The shape they chose, as their look carried it (clamped by PlayerLook); none for an
            // older app's look, which draws the body as made.
            shape: ShapeValues::Look(r.look.as_ref().map(|l| l.shape.clone()).unwrap_or_default()),
            wearer: "player",
            body: r.look.as_ref().map(|l| l.body.clone()),
            skin: r.look.as_ref().map(|l| l.skin),
            hair: r.look.as_ref().map(|l| l.hair),
            eyes: r.look.as_ref().and_then(|l| l.eyes),
            outfit: None,
            up: Vec3::Y,
            // How they stand, as their newest update said (rung 2a).
            crouch: r.posture.stance == humanity_core::stance::Stance::Crouch,
            airborne: r.posture.airborne,
            // Their face and body details and what they wear, as their look carried it (the look,
            // 2026-11-09; cleaned by PlayerLook's one reader, so only known ids arrive).
            detail: r.look.as_ref().map(|l| l.detail.clone()).unwrap_or_default(),
            dress: r.look.as_ref().map(|l| dress_of_wear(&l.wear)).unwrap_or_default(),
            act: None,
            // Where they look (rung 2b): their yaw and the pitch their update carried.
            look: Some(t.position + Vec3::new(yaw.sin() * r.doing.pitch.cos(), r.doing.pitch.sin(), -yaw.cos() * r.doing.pitch.cos()) * 6.0),
            first_person: false,
            held: [None, None],
            second_hand: None,
        };
        if let Some(lib) = lib.as_deref().filter(|_| state.interact.rung2b_on) {
            p.act = remote_act_in(state, lib, &mut p, r.doing.act.as_ref());
        }
        out.push(p);
    }
    // A crew member looks at the player when the player is near and in front of them (rung 2b: a
    // point of interest; the crowd's timetable will give them others).
    let my_head = state.camera.position;
    for (_e, (t, n)) in world.query::<(&humanity_core::ecs::components::Transform, &crate::net::sync::RemoteNpc)>().iter() {
        let facing = facing_of_turn(t.rotation);
        let feet = t.position - Vec3::new(0.0, CREW_POSITION_OVER_FEET_M, 0.0);
        let to_me = my_head - (feet + Vec3::Y * 1.5);
        let ahead = Vec3::new(facing.sin(), 0.0, facing.cos());
        let look = (to_me.length() < CREW_LOOK_M && to_me.normalize_or_zero().dot(ahead) > 0.2).then_some(my_head);
        let mut p = Person {
            key: FigureKey::Crew(n.entity_id),
            feet,
            facing,
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
            detail: Default::default(),
            dress: Default::default(),
            act: None,
            look,
            first_person: false,
            held: [None, None],
            second_hand: None,
        };
        if let Some(lib) = lib.as_deref().filter(|_| state.interact.rung2b_on) {
            p.act = door_act_in(state, lib, &p);
        }
        out.push(p);
    }
    let clock = state.animation.clock;
    for (i, c) in state.animation.crowd.iter().enumerate() {
        let (feet, facing) = crowd_place(c, clock);
        let key = FigureKey::Crowd(i as u32);
        let (shape, body, detail, dress) = match &c.look {
            Some(l) => (ShapeValues::Look(l.shape.clone()), Some(l.body.clone()), l.detail.clone(), l.dress.clone()),
            None => (no_look(key), None, Default::default(), Default::default()),
        };
        let mut p = Person { key, feet, facing, height: 1.0, shape, wearer: "other", body, skin: None, hair: None, eyes: None, outfit: None, up: Vec3::Y, crouch: c.crouch, airborne: false, detail, dress, act: None, look: None, first_person: false, held: [None, None], second_hand: None };
        if let Some(lib) = lib.as_deref().filter(|_| state.interact.rung2b_on) {
            p.act = crowd_act_in(state, lib, c, clock, &mut p);
        }
        out.push(p);
    }
    out
}

/// How near the player must be for a crew member to look at them (metres).
const CREW_LOOK_M: f32 = 4.0;

/// The player's act this frame (rung 2b), in the order gameplay grants them: their granted seat or
/// bed (its root and facing put where the grant has eased them, the capsule's place), else the
/// ladder they are on, else the workbench whose card they have open, else a door opening at their
/// hand.
fn my_act_in(state: &EngineState, lib: &CharacterLibrary, p: &mut Person) -> Option<ActIn> {
    if let Some(e) = state.interact.me.filter(|e| e.placed.anchor < lib.anchors.anchors.len()) {
        let bi = lib.body_for(p.body.as_deref(), p.wearer, stagger(p.key));
        let (feet, facing) = crate::engine::interact::engaged_root(lib, &e, bi);
        p.feet = feet;
        p.facing = facing;
        return Some(ActIn { anchor: e.placed.anchor, phase: Some(e.phase), t: Some(e.clip_t()), placed: Some(e.placed) });
    }
    if state.aboard_station && state.controller.on_ladder(state.camera.position.y) {
        if let Some(placed) = crate::engine::interact::ladder_near(state, lib, p.feet) {
            face_anchor(lib, &placed, p, true);
            return Some(ActIn { anchor: placed.anchor, phase: None, t: None, placed: Some(placed) });
        }
    }
    if let Some(sel) = state.gui_state.selected_machine {
        for placed in crate::engine::interact::machine_anchors(state, lib, sel) {
            let def = &lib.anchors.anchors[placed.anchor];
            if def.kind != humanity_animation::AnchorKind::Station {
                continue;
            }
            // From whichever side of it they stand.
            let flip = (Quat::from_rotation_y(-placed.yaw) * (p.feet - placed.origin)).z > 0.0;
            let placed = crate::engine::interact::Placed { yaw: placed.yaw + if flip { std::f32::consts::PI } else { 0.0 }, ..placed };
            if crate::engine::interact::at_station(def, &placed, p.feet) {
                face_anchor(lib, &placed, p, false);
                return Some(ActIn { anchor: placed.anchor, phase: None, t: None, placed: Some(placed) });
            }
        }
    }
    door_act_in(state, lib, p)
}

/// Another player's act (rung 2b), as their update named it: a seat or a bed posed from the
/// anchor's own row where they stand (their capsule is at its root), a ladder or a workbench found by
/// its id near them; else a door opening at their hand, worked out here.
fn remote_act_in(state: &EngineState, lib: &CharacterLibrary, p: &mut Person, act: Option<&humanity_core::stance::Act>) -> Option<ActIn> {
    let Some(act) = act else { return door_act_in(state, lib, p) };
    let anchor = lib.anchors.index(&act.anchor)?;
    let def = &lib.anchors.anchors[anchor];
    let phase = humanity_animation::Phase::from_code(act.phase)?;
    let placed = match def.kind {
        humanity_animation::AnchorKind::Climb => crate::engine::interact::ladder_near(state, lib, p.feet),
        humanity_animation::AnchorKind::Station => crate::engine::interact::machine_anchor_near(state, lib, anchor, p.feet, def.reach_m),
        _ => None,
    };
    if let Some(pl) = placed.as_ref() {
        face_anchor(lib, pl, p, def.kind == humanity_animation::AnchorKind::Climb);
    }
    Some(ActIn { anchor, phase: Some(phase), t: None, placed })
}

/// A dev stand-in's act (rung 2b): one the `crowd_act` verb gave it, timed by the runtime's clock
/// (in from the approach point, held, out after its hold), else the ladder it is on (a `figure_path`
/// up a ladder), else a door opening at its hand.
fn crowd_act_in(state: &EngineState, lib: &CharacterLibrary, c: &CrowdFigure, clock: f32, p: &mut Person) -> Option<ActIn> {
    if let Some(a) = c.act.filter(|a| a.placed.anchor < lib.anchors.anchors.len()) {
        let def = &lib.anchors.anchors[a.placed.anchor];
        let bi = lib.body_for(p.body.as_deref(), p.wearer, stagger(p.key));
        let clips = &lib.bodies[bi.min(lib.bodies.len() - 1)].clips;
        let enter = humanity_animation::interact::phase_len(def, humanity_animation::Phase::Enter, clips);
        let exit = humanity_animation::interact::phase_len(def, humanity_animation::Phase::Exit, clips);
        let t = (clock - c.born).max(0.0);
        let hold = a.hold_s.unwrap_or(f32::INFINITY);
        let (phase, pt) = if t < enter {
            (humanity_animation::Phase::Enter, t)
        } else if t < enter + hold {
            (humanity_animation::Phase::Hold, t - enter)
        } else if t < enter + hold + exit {
            (humanity_animation::Phase::Exit, t - enter - hold)
        } else {
            return None;
        };
        let start = (a.placed.approach(def), a.placed.root(def).1);
        let e = crate::engine::interact::Engaged { placed: a.placed, phase, t: pt, from: start, leave: false, approach_s: 0.0 };
        if def.root_driven {
            let (feet, facing) = crate::engine::interact::engaged_root(lib, &e, bi);
            p.feet = feet;
            p.facing = facing;
        } else {
            face_anchor(lib, &a.placed, p, def.kind == humanity_animation::AnchorKind::Climb);
        }
        return Some(ActIn { anchor: a.placed.anchor, phase: Some(phase), t: Some(pt), placed: Some(a.placed) });
    }
    // A stand-in walking a path up a ladder is on it once it is off the ladder's floor.
    if let Some(placed) = crate::engine::interact::ladder_near(state, lib, p.feet) {
        if p.feet.y > placed.origin.y + 0.25 && matches!(placed.object, crate::engine::interact::ObjectRef::Ladder { height, .. } if p.feet.y < placed.origin.y + height - 0.05) {
            face_anchor(lib, &placed, p, true);
            return Some(ActIn { anchor: placed.anchor, phase: None, t: None, placed: Some(placed) });
        }
    }
    door_act_in(state, lib, p)
}

/// A swing door opening within reach of a person's hand (rung 2b, worked out on every screen): its
/// handle as their act.
fn door_act_in(state: &EngineState, lib: &CharacterLibrary, p: &Person) -> Option<ActIn> {
    if !state.aboard_station {
        return None;
    }
    let shoulder = p.feet + p.up * 1.35;
    let h = crate::engine::interact::opening_handle_near(state, lib, shoulder, DOOR_REACH_M)?;
    // Only a door ahead of them (one swinging open beside or behind them is not theirs to hold).
    let ahead = Vec3::new(p.facing.sin(), 0.0, p.facing.cos());
    let to = h.at - shoulder;
    if Vec3::new(to.x, 0.0, to.z).normalize_or_zero().dot(ahead) < 0.1 {
        return None;
    }
    // Within this body's own reach from its nearer shoulder, at the scale it is drawn: the first rig
    // sweep took a handle 85 cm from a fixed shoulder height, and the hand hung 30 cm short of it,
    // the arm straight out at a handle it could not reach.
    let bi = lib.body_for(p.body.as_deref(), p.wearer, stagger(p.key));
    let body = lib.bodies.get(bi)?;
    let rig = body.hands.as_ref()?;
    let (_, scale) = shape_of(lib, body, p);
    let rotation = body_rotation(p.up, p.facing);
    let reaches = (0..2).any(|side| {
        let at = p.feet + rotation * (rig.shoulders[side] * scale);
        (h.at - at).length() <= rig.reach[side] * scale * DOOR_REACH_SHARE
    });
    if !reaches {
        return None;
    }
    let placed = crate::engine::interact::Placed { anchor: h.anchor, origin: h.at, yaw: 0.0, object: crate::engine::interact::ObjectRef::Door(h.door) };
    Some(ActIn { anchor: h.anchor, phase: None, t: None, placed: Some(placed) })
}

/// How far from a person's shoulders' height a door's handle may be for it to be looked at further
/// (metres): more than any arm reaches; each body's own reach then decides (`DOOR_REACH_SHARE`).
const DOOR_REACH_M: f32 = 0.85;
/// The share of an arm's full length (shoulder to the palm's middle) a hand reaches for a handle at:
/// short of the elbow locked straight.
const DOOR_REACH_SHARE: f32 = 0.95;

/// Turn a person to face an anchor while it holds them (a ladder, a workbench): the drawn body
/// faces it (the capsule and the camera are untouched); a climber is also drawn at the ladder's face,
/// at the height their capsule climbs to.
fn face_anchor(lib: &CharacterLibrary, placed: &crate::engine::interact::Placed, p: &mut Person, at_face: bool) {
    let def = &lib.anchors.anchors[placed.anchor];
    let (root, facing) = placed.root(def);
    p.facing = facing;
    if at_face {
        p.feet = Vec3::new(root.x, p.feet.y, root.z);
    }
}

/// What another player's look says they wear ("" is the viewer's default for that part).
pub(crate) fn dress_of_wear(w: &humanity_core::player_look::LookWear) -> humanity_animation::DressChoice {
    let id = |s: &str| (!s.is_empty()).then(|| s.to_string());
    humanity_animation::DressChoice { hair: id(&w.hair), brows: id(&w.brows), lashes: id(&w.lashes), outfit: w.outfit.clone() }
}

/// What the player's look says they wear: the hair, brows and lashes chosen on the character page
/// (an id nothing knows, as the old default "short", reads as the wearer kind's default), and the
/// wardrobe's cosmetics by slot.
pub(crate) fn my_dress(app: &humanity_core::ecs::components::Appearance, outfit: &humanity_core::ecs::components::Outfit) -> humanity_animation::DressChoice {
    let id = |s: &str| (!s.is_empty()).then(|| s.to_string());
    humanity_animation::DressChoice {
        hair: id(&app.hair_style),
        brows: id(&app.brows),
        lashes: id(&app.lashes),
        outfit: outfit.equipped.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
    }
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

/// The joint the first-person body folds away from the eye (rung 2b): the neck, and so everything
/// under it (the head, the eyes, the jaw); the upper neck or the head on a rig without a neck.
pub(crate) fn first_person_fold(h: &humanity_animation::HandRig) -> Option<usize> {
    h.neck.or(h.upper_neck).or(h.head)
}

/// How fast the first-person eye follows the drawn head (seconds, an exponential follow): a step's
/// small jolts are smoothed while a sit, a crouch or a climb is followed at once (rung 2b, the
/// stabilization; yaw and pitch stay the player's).
const HEAD_RIDE_FOLLOW_S: f32 = 0.06;

/// A version of a shared-body person's shape (their body and shape weights), for the seat's drop
/// measured on them (a person's own mesh has its own version, `PersonMesh::key`). FNV-1a.
fn weights_key(body: usize, weights: &[(usize, f32)]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ 0x5eed;
    let mut eat = |x: u64| {
        for b in x.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    eat(body as u64);
    for &(t, w) in weights {
        eat(t as u64);
        eat(w.to_bits() as u64);
    }
    h
}

/// The hands' (and on a ladder the feet's) targets for an interaction this frame, in the person's
/// model space (rung 2b): the ladder's rungs from the piece's own size and rung count, a workbench's
/// surface in front of the person along its length, a door's handle for the nearer hand. Which
/// climbing hands are on a rung (not moving between rungs) goes in `hands_on`.
#[allow(clippy::too_many_arguments)]
fn contact_limbs(
    lib: &CharacterLibrary,
    def: &humanity_animation::AnchorDef,
    placed: Option<&crate::engine::interact::Placed>,
    to_model: &Mat4,
    p: &Person,
    scale: f32,
    legs: Option<&humanity_animation::FootRig>,
    hand_rig: Option<&humanity_animation::HandRig>,
    reach: f32,
    hands_on: &mut [bool; 2],
) -> Option<humanity_animation::Limbs> {
    use humanity_animation::interact::{ClimbInput, ContactPart, HandTarget};
    use humanity_animation::AnchorKind;
    let placed = placed?;
    let grip = |part: ContactPart| def.contact(part).and_then(|c| c.grip.as_deref()).and_then(|g| lib.grips.index(g));
    let up_m = to_model.transform_vector3(p.up).normalize_or_zero();
    match def.kind {
        AnchorKind::Climb => {
            let crate::engine::interact::ObjectRef::Ladder { height, rungs, width } = placed.object else { return None };
            let n = rungs.max(2);
            let s = height / n as f32;
            // The ladder's foot in model space: its rung line ahead, its middle across, its foot
            // under the root by the height the capsule has climbed.
            let foot = to_model.transform_point3(placed.origin);
            let input = ClimbInput { z: foot.z, x: foot.x, first_rung_y: 0.5 * s / scale, spacing: s / scale, rungs: n as usize, half_width: (width * 0.5 - 0.04) / scale, rung_half_m: RUNG_HALF_M / scale, reach_over_root: reach, palm_depth: hand_rig.map_or([0.012; 2], |h| [h.arms[0].palm_depth / scale, h.arms[1].palm_depth / scale]) };
            let limbs = humanity_animation::interact::climb_limbs(&input, -foot.y, grip(ContactPart::Hands), legs);
            for (side, t) in limbs.hands.iter().enumerate() {
                if let Some(t) = t {
                    let k = (t.at.y - foot.y - input.first_rung_y) / input.spacing;
                    hands_on[side] = (k - k.round()).abs() < 0.01 && k.round() >= 0.0 && k.round() < n as f32;
                }
            }
            Some(limbs)
        }
        AnchorKind::Station => {
            let c = def.contact(ContactPart::Hands)?;
            let span = def.span_m.unwrap_or(0.0);
            // In front of the person along the bench's length.
            let local = Quat::from_rotation_y(-placed.yaw) * (p.feet - placed.origin);
            let slide = local.x.clamp(-span, span);
            let palm = to_model.transform_vector3(-p.up).normalize_or_zero();
            let fingers = to_model.transform_vector3(Quat::from_rotation_y(placed.yaw) * Vec3::Z).normalize_or_zero();
            let mut l = humanity_animation::Limbs { weight: 1.0, ..Default::default() };
            for side in 0..2 {
                // The person's left (model +X) is the bench frame's +X when they face its +Z.
                let across = if side == 0 { STATION_HANDS_APART_M * 0.5 } else { -STATION_HANDS_APART_M * 0.5 };
                // The palm's point stands its hand's palm-side depth over the top, so the deepest skin
                // (the thumb's ball, the heel of the hand) rests on it (measured from the drawn hand).
                let depth = hand_rig.map_or(0.012, |h| h.arms[side].palm_depth);
                let at = to_model.transform_point3(placed.point([slide + across, c.at[1], c.at[2]])) + up_m * ((depth + 0.002) / scale);
                l.hands[side] = Some(HandTarget { at, palm, fingers, grip: grip(ContactPart::Hands) });
            }
            Some(l)
        }
        AnchorKind::Door => {
            // The handle (a door's placed origin is its handle this frame), for the nearer hand,
            // the palm toward it, the fingers down its face.
            let at = to_model.transform_point3(placed.origin);
            let side = if at.x >= 0.0 { 0 } else { 1 };
            let part = def.contacts.first().map(|c| c.part).unwrap_or(ContactPart::RightHand);
            let palm = Vec3::new(at.x * 0.3, 0.0, at.z).normalize_or_zero();
            let fingers = -up_m;
            let mut l = humanity_animation::Limbs { weight: 1.0, ..Default::default() };
            l.hands[side] = Some(HandTarget { at, palm, fingers, grip: grip(part) });
            Some(l)
        }
        _ => None,
    }
}

/// A ladder rung's half thickness (metres): the shipped ladder's rungs are 8 cm boxes
/// (crates/humanity-ship/src/structure.rs, `MeshShape::Ladder`).
const RUNG_HALF_M: f32 = 0.04;
/// How far apart a person's hands rest on a work surface (metres): about their shoulders' width.
const STATION_HANDS_APART_M: f32 = 0.40;

/// Measure one person's drawn contacts for the interaction probe (rung 2b, engine::anchor_probe):
/// the hips on the seat or the mattress while holding, each climbing hand on its rung while it grips
/// one, each hand on a work surface, the hand at a door's handle.
#[allow(clippy::too_many_arguments)]
fn probe_contacts(
    probe: &mut crate::engine::anchor_probe::AnchorProbe,
    p: &Person,
    body: &humanity_animation::LoadedBody,
    def: &humanity_animation::AnchorDef,
    tr: Track,
    world: &Mat4,
    model: &humanity_core::skin_data::SkinnedModelData,
    seat: &[(usize, usize)],
    hands: &[Vec<(usize, usize)>; 2],
    weights: &[(usize, f32)],
    rows: &[[f32; 4]],
    hands_on: [bool; 2],
    engage_w: f32,
    hips_world: Vec3,
) {
    use crate::engine::anchor_probe::{hips_clearance, nearest, rung_gap, skinned, surface_clearance};
    use humanity_animation::interact::ContactPart;
    use humanity_animation::{AnchorKind, Phase};
    let kind = format!("{:?}", def.kind);
    let phase = tr.phase.code();
    probe.phase(p.key, phase);
    let note = |probe: &mut crate::engine::anchor_probe::AnchorProbe, name: &'static str, v: Option<f32>| probe.note(p.key, &body.choice, &def.id, &kind, phase, name, v);
    let holding = tr.phase == Phase::Hold && engage_w > 0.999;
    match def.kind {
        AnchorKind::Sit | AnchorKind::Lie if holding => {
            let Some(c) = def.contact(ContactPart::Hips) else { return };
            // The seat's contact in the world: on the placed object, or from the anchor's row where
            // the person stands at its root (another player's seat).
            let contact = match tr.placed {
                Some(pl) => pl.point(c.at),
                None => p.feet + body_rotation(p.up, p.facing) * def.in_person_frame(c.at),
            };
            let pts = skinned(model, seat, weights, rows, world, crate::engine::anchor_probe::VERTEX_STRIDE);
            let forward = (def.kind == AnchorKind::Sit).then(|| body_rotation(p.up, p.facing) * Vec3::Z);
            note(probe, "hips_m", hips_clearance(&pts, hips_world, contact, p.up, forward));
        }
        AnchorKind::Climb if engage_w > 0.999 => {
            let Some(pl) = tr.placed else { return };
            let crate::engine::interact::ObjectRef::Ladder { height, rungs, width } = pl.object else { return };
            let n = rungs.max(2);
            for side in 0..2 {
                if !hands_on[side] {
                    continue;
                }
                let pts = skinned(model, &hands[side], weights, rows, world, crate::engine::anchor_probe::VERTEX_STRIDE);
                let gap = (0..n)
                    .filter_map(|r| {
                        let y = (r as f32 + 0.5) * height / n as f32;
                        rung_gap(&pts, pl.point([-width * 0.5 + 0.04, y, 0.0]), pl.point([width * 0.5 - 0.04, y, 0.0]), RUNG_HALF_M)
                    })
                    .reduce(f32::min);
                note(probe, if side == 0 { "left_hand_m" } else { "right_hand_m" }, gap);
            }
        }
        AnchorKind::Station if holding => {
            let (Some(c), Some(pl)) = (def.contact(ContactPart::Hands), tr.placed) else { return };
            let y = pl.point(c.at).y;
            for side in 0..2 {
                let pts = skinned(model, &hands[side], weights, rows, world, crate::engine::anchor_probe::VERTEX_STRIDE);
                note(probe, if side == 0 { "left_hand_m" } else { "right_hand_m" }, surface_clearance(&pts, y));
            }
        }
        AnchorKind::Door if holding => {
            let Some(pl) = tr.placed else { return };
            let gap = (0..2).filter_map(|side| nearest(&skinned(model, &hands[side], weights, rows, world, crate::engine::anchor_probe::VERTEX_STRIDE), pl.origin)).reduce(f32::min);
            note(probe, "handle_m", gap);
        }
        _ => {}
    }
}

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
pub(crate) fn draw_people(
    state: &mut EngineState,
    all_objects: &mut Vec<RenderObject>,
    render_space: &mut Vec<RenderObject>,
    transparent: &mut Vec<RenderObject>,
    overlay: &mut Vec<RenderObject>,
    showroom: bool,
    dt: f32,
) -> bool {
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
    // A cosmetic's garment (cosmetics.csv's `model`): None for an id the catalog does not know.
    let data_dir = state.data_dir.clone();
    let cosmetics = &state.cosmetics;
    let model_of = |id: &str| -> Option<Option<String>> { cosmetics.iter().find(|c| c.id == id).map(|c| c.model.clone()) };
    // Settings > Graphics > Garment textures (Full or Low memory) and the GPU's block compression:
    // a change lets every garment texture go, and the pools with them, to be read again at the new
    // size as people wear them (each drawn in its mean colour for the frames that takes).
    let tex_size = TexSize { max_px: crate::gui::garment_textures::max_px(&state.gui_state.settings.garment_textures), compressed: state.renderer.block_compression() };
    if state.animation.tex_size != Some(tex_size) {
        if state.animation.tex_size.is_some() {
            log::info!("Characters: garment textures now {} texels a half ({}); every one is read again as it is worn", tex_size.max_px, if tex_size.compressed { "BC7" } else { "uncompressed" });
            let_go_of_garment_textures(&mut state.animation, &mut state.renderer);
        }
        state.animation.tex_size = Some(tex_size);
    }
    // The pools, each allocated once, on the first frame that draws bodies (entering the world), and
    // again after the setting changes or the library is read again: never when a crowd walks in.
    // Measured 2026-11-10: allocated as the first garment of its shape arrived, the small pool took
    // 9.0 ms on that frame (the large one 0.1 ms), the crowd's one remaining spike.
    if state.animation.textures_on && state.animation.pools.is_empty() {
        if let Some(lib) = state.animation.lib.clone() {
            if let Some(look) = lib.look.as_ref() {
                allocate_pools(&mut state.animation.pools, &mut state.renderer, look, lib.characters.tiers.near_cap, tex_size);
            }
        }
    }
    // The own-mesh pool (2026-11-10, renderer::mesh_pool), the same way: allocated once, on the
    // first frame that draws bodies, and again after the library is read again; never when a crowd
    // walks in. Measured before it: one own-mesh upload in about 38 took 9.9 to 11.5 ms, each a new
    // allocation, in three of five crowd runs.
    if state.renderer.skin_own_pool_stats().is_none() {
        if let Some(lib) = state.animation.lib.clone() {
            if let Some(plan) = own_pool_plan(&lib) {
                match state.renderer.skin_create_own_pool(plan.class, plan.people, plan.handles, plan.shared_places, COMPUTE_FALLBACK_CAP as u32) {
                    Some(ms) => {
                        let stats = state.renderer.skin_own_pool_stats();
                        log::info!(
                            "Characters: own-mesh pool for {} people of up to {} vertices and {} indices ({}), {} places, and {} near places for each shared body: {:.1} MB allocated in {:.2} ms",
                            plan.people,
                            plan.class.vertices,
                            plan.class.indices,
                            if std::env::var_os(OWN_POOL_VERTICES_VAR).is_some() { "planned smaller by HUMANITY_OWN_POOL_VERTICES" } else { "the largest dressed person" },
                            plan.handles,
                            plan.shared_places,
                            stats.map_or(0.0, |s| s.reserved_bytes as f64 / 1_048_576.0),
                            ms
                        );
                    }
                    None => log::warn!("Characters: the own-mesh pool could not be made (no skinned body registered); people near are drawn with the shared body"),
                }
            }
        }
    }
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
    // Own meshes building on workers now (at most `look_threads`).
    let mut in_flight = rt.figures.values().filter(|f| f.building.is_some()).count();
    // What a frame puts on the GPU for the look (2026-11-10): one finished own mesh (about 1 ms, its
    // buffers) and one garment texture into its slot (up to 2.7 MB of BC7 with its levels, 10.7 MB
    // decoded on a GPU without block compression); the rest wait a frame each, so
    // a crowd arriving never piles every upload into one frame (the sweep's worst frame did).
    let mut mesh_uploads = 1usize;
    let mut texture_uploads = 1usize;
    // Own meshes kept for people who left the near tier stay at most this many (the oldest go
    // first), so a large crowd milling around the near line holds a bounded set; the own-mesh pool
    // has a place in its books for each (`own_pool_plan`), and when its room runs short the longest
    // out gives way sooner.
    let keep_cap = own_keep_cap(lib.characters.tiers.near_cap);
    // The interactions' switches (rung 2b; the rigs' red twins) and the probe while it runs.
    let (warp_on, hand_ik_on, fp_head_hidden, rung2b_on) = (state.interact.warp_on, state.interact.hand_ik_on, state.interact.fp_head_hidden, state.interact.rung2b_on);
    let mut aprobe = state.interact.probe.take();
    let mut me_act: Option<(String, humanity_animation::Phase)> = None;
    let mut my_eye: Option<Vec3> = None;
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
        // Taken out of the map while they are drawn (and put back at the end of this person), so a
        // newcomer's mesh can make room in the own-mesh pool by letting another person's kept mesh go.
        let mut fig = rt.figures.remove(&p.key).unwrap_or_else(|| Figure {
            animator: Animator::with_offset(&lib.graph, hash, hash * 4.0),
            motion: MotionTracker::default(),
            seen: 0,
            shape: None,
            feet: humanity_animation::FootPlacer::default(),
            air_s: 0.0,
            own: None,
            away_s: 0.0,
            building: None,
            pending: None,
            too_big: None,
            track: None,
            look: Default::default(),
            seat_drop: None,
            view_offset: None,
        });
        fig.seen = rt.frame;
        let Figure { animator, motion, shape, feet, air_s, own, away_s, building, pending, too_big, track, look, seat_drop, .. } = &mut fig;
        // THE LOOK (2026-11-09; humanity_animation::look): a near person, the player and the
        // showroom's figure are drawn with their OWN mesh, built once when their look changes (a
        // version made from everything it is built from), on a worker thread off the frame (at most
        // `look_threads` at once, the player first: they come first in the list), and every frame ON
        // the frame with the reuse switch off; everyone else, and anyone still waiting for their
        // first build, with the shared body and its shape on the GPU.
        let wants_own = rt.look_on
            && !rt.fallback_only
            && body.base.is_some()
            && (tier[i] == DrawTier::Near || matches!(p.key, FigureKey::Me | FigureKey::Showroom));
        let mut own_key = None;
        if let (true, Some(look), Some(base)) = (wants_own, lib.look.as_ref(), body.base.as_ref()) {
            *away_s = 0.0;
            let garments = lib.dress_for(bi, p.wearer, figure_seed(p.key), &p.dress, &model_of);
            let inputs = humanity_animation::PersonInputs { shape: &weights, detail: &p.detail, garments: &garments, covers: rt.covers_on, soles: rt.soles_on };
            let key = humanity_animation::look::look_key(bi, &inputs);
            own_key = Some(key);
            // A build that finished on its worker comes onto the GPU (even if their look moved on
            // while it built: it is newer than what they had, and the next one is due at once).
            let mut arrived: Option<(humanity_animation::PersonMesh, f32, bool)> = None;
            // A finished mesh that waited for room in the own-mesh pool goes first.
            if mesh_uploads > 0 {
                if let Some(waiting) = pending.take() {
                    arrived = Some(waiting);
                    mesh_uploads -= 1;
                }
            }
            if let (None, Some(b), true) = (arrived.as_ref(), building.as_ref(), mesh_uploads > 0) {
                match b.rx.try_recv() {
                    Ok((mesh, ms)) => {
                        arrived = Some((mesh, ms, true));
                        *building = None;
                        in_flight = in_flight.saturating_sub(1);
                        mesh_uploads -= 1;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {}
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        log::warn!("Characters: {:?}'s own mesh build stopped without a mesh; trying again", p.key);
                        *building = None;
                        in_flight = in_flight.saturating_sub(1);
                    }
                }
            }
            let have = arrived.as_ref().or(pending.as_ref()).map(|(m, _, _)| m.key).or(own.as_ref().map(|o| o.mesh.key));
            // A look whose mesh was too big for the own-mesh pool is not built again until it
            // changes (or the pool is planned again); with the reuse switch off, every frame anyway.
            let due = humanity_animation::look::rebuild_due(have, key, reuse) && !(reuse && *too_big == Some(key));
            if due && !reuse {
                // The reuse switch off: built again on the frame, every frame, so nothing drawn is
                // older than this frame's look.
                let _cost = humanity_core::frame_costs::stage("cpu.look_build");
                let t0 = std::time::Instant::now();
                let mesh = humanity_animation::look::build_person(body, base, look, &lib.humanoid, &inputs, key);
                arrived = Some((mesh, t0.elapsed().as_secs_f32() * 1000.0, false));
            } else if due && building.is_none() && in_flight < lib.characters.look_threads.max(1) {
                let (tx, rx) = std::sync::mpsc::channel();
                let job_lib = std::sync::Arc::clone(lib);
                let (job_shape, job_detail, job_garments) = (weights.clone(), p.detail.clone(), garments.clone());
                let (covers, soles) = (rt.covers_on, rt.soles_on);
                let spawned = std::thread::Builder::new().name("look-build".into()).spawn(move || {
                    let body = &job_lib.bodies[bi];
                    let (Some(look), Some(base)) = (job_lib.look.as_ref(), body.base.as_ref()) else { return };
                    let inputs = humanity_animation::PersonInputs { shape: &job_shape, detail: &job_detail, garments: &job_garments, covers, soles };
                    let t0 = std::time::Instant::now();
                    let mesh = humanity_animation::look::build_person(body, base, look, &job_lib.humanoid, &inputs, key);
                    let _ = tx.send((mesh, t0.elapsed().as_secs_f32() * 1000.0));
                });
                match spawned {
                    Ok(_) => {
                        *building = Some(Building { rx });
                        in_flight += 1;
                    }
                    Err(e) => {
                        // No thread to be had: built on the frame instead, as before workers.
                        log::warn!("Characters: could not start a build worker ({e}); building {:?} on the frame", p.key);
                        let _cost = humanity_core::frame_costs::stage("cpu.look_build");
                        let t0 = std::time::Instant::now();
                        let mesh = humanity_animation::look::build_person(body, base, look, &lib.humanoid, &inputs, key);
                        arrived = Some((mesh, t0.elapsed().as_secs_f32() * 1000.0, false));
                    }
                }
            }
            if let Some((mesh, ms, worker)) = arrived {
                // What the frame pays for a person's new mesh: two writes into their range of the
                // own-mesh pool (and, with the reuse switch off, the build itself, above).
                let _cost = humanity_core::frame_costs::stage("cpu.look_build");
                let t0 = std::time::Instant::now();
                let (nv, ni) = (mesh.model.vertex_count() as u32, mesh.model.primitives.iter().map(|q| q.indices.len()).sum::<usize>() as u32);
                let mut placed = None;
                let mut outcome = OwnOutcome::NoPool;
                // Each round either ends or lets one kept mesh go, so it is bounded by the people.
                for _ in 0..rt.figures.len() + 2 {
                    let kept: Vec<(f32, usize)> = rt.figures.values().filter_map(|f| f.own.as_ref().filter(|_| f.away_s > 0.0).map(|o| (f.away_s, o.gpu))).collect();
                    match renderer.skin_own_next_step(own.as_ref().map(|o| o.gpu), nv, ni, &kept) {
                        None => break,
                        Some(Step::Place(b)) => {
                            match renderer.skin_own_place(b, &mesh.model) {
                                Ok(()) => placed = Some(b),
                                Err(e) => outcome = OwnOutcome::Refused(e),
                            }
                            break;
                        }
                        Some(Step::Evict(b)) => {
                            // The person out of the near tier longest gives their room up (their mesh
                            // is built again if they come back).
                            match rt.figures.iter_mut().find(|(_, f)| f.own.as_ref().is_some_and(|o| o.gpu == b)) {
                                Some((k, f)) => {
                                    log::info!("Characters: {k:?}'s kept own mesh (out of the near tier {:.1} s) let go to make room for {:?}", f.away_s, p.key);
                                    f.own = None;
                                    renderer.skin_release_body(b);
                                }
                                None => renderer.skin_release_body(b),
                            }
                            report.own_evicted += 1;
                        }
                        Some(Step::Pack(side)) => {
                            // The free room is in pieces: this frame's upload moves one person down
                            // (their mesh written again from its CPU copy), and the newcomer tries
                            // again next frame.
                            let moved = renderer.skin_own_pack_candidate(side);
                            let model = moved.and_then(|b| match own.as_ref().filter(|o| o.gpu == b) {
                                Some(o) => Some(&o.mesh.model),
                                None => rt.figures.values().find_map(|f| f.own.as_ref().filter(|o| o.gpu == b).map(|o| &o.mesh.model)),
                            });
                            if let (Some(b), Some(m)) = (moved, model) {
                                if renderer.skin_own_pack(side, b, m) {
                                    report.own_packed += 1;
                                    log::info!("Characters: the own-mesh pool's {side:?} packed: one person moved down to make room for {:?}", p.key);
                                }
                            }
                            outcome = OwnOutcome::Packing;
                            break;
                        }
                        Some(Step::Wait) => {
                            outcome = OwnOutcome::Waiting;
                            break;
                        }
                        Some(Step::TooBig) => {
                            outcome = OwnOutcome::TooBig;
                            break;
                        }
                    }
                }
                let upload_ms = t0.elapsed().as_secs_f32() * 1000.0;
                report.own_built += 1;
                report.own_build_ms += ms;
                report.own_build_max_ms = report.own_build_max_ms.max(ms);
                match placed {
                    Some(gpu) => {
                        if reuse {
                            // Rare with reuse on (a look changed, someone came near): each build is
                            // logged with its cost and size, the measurement section 16 quotes.
                            log::info!(
                                "Characters: built {:?}'s own mesh on {} in {ms:.2} ms {} ({} vertices, {} indices, {} parts, lift {:.3} m), onto the GPU in {upload_ms:.2} ms (the own-mesh pool: {})",
                                p.key,
                                body.choice,
                                if worker { "on a worker" } else { "on the frame" },
                                nv,
                                ni,
                                mesh.parts.len(),
                                mesh.lift,
                                renderer.skin_own_pool_stats().map_or_else(String::new, |s| format!("{} people, {:.1} of {:.1} MB", s.holders, s.used_bytes as f64 / 1_048_576.0, s.reserved_bytes as f64 / 1_048_576.0))
                            );
                        }
                        *too_big = None;
                        let bytes = renderer.skin_body_bytes(gpu);
                        *own = Some(OwnBody { mesh, body: bi, gpu, bytes });
                    }
                    None => match outcome {
                        OwnOutcome::TooBig => {
                            // Bigger than the pool was planned for: the shared body, until the look
                            // or the pool's plan changes. Logged once a look.
                            if *too_big != Some(mesh.key) {
                                log::warn!(
                                    "Characters: {:?}'s own mesh ({nv} vertices, {ni} indices) is bigger than the own-mesh pool is planned for ({}); drawn with the shared body",
                                    p.key,
                                    renderer.skin_own_pool_stats().map_or_else(String::new, |s| format!("up to {} vertices and {} indices", s.class.vertices, s.class.indices))
                                );
                            }
                            *too_big = Some(mesh.key);
                            if let Some(o) = own.take() {
                                renderer.skin_release_body(o.gpu);
                            }
                        }
                        OwnOutcome::Refused(e) => {
                            log::warn!("Characters: {:?}'s own mesh was refused by the own-mesh pool ({e:?}); trying again", p.key);
                            *pending = Some((mesh, ms, worker));
                        }
                        OwnOutcome::NoPool | OwnOutcome::Packing | OwnOutcome::Waiting => {
                            report.own_waiting += 1;
                            *pending = Some((mesh, ms, worker));
                        }
                    },
                }
            }
        } else {
            *away_s += dt;
        }
        // Their own mesh when it was built for the look they have now; with reuse on, also the last
        // one built for them on this body while the next builds (a few frames), so a slider dragged
        // on the character page never flickers to the shared body. With the reuse switch off every
        // mesh drawn is this frame's.
        let mine = own.as_ref().filter(|o| own_key == Some(o.mesh.key) || (reuse && own_key.is_some() && o.body == bi));
        if let Some(o) = mine {
            report.own += 1;
            report.own_bytes += o.bytes;
        }
        let gpu_body = mine.map_or(gpu_body, |o| o.gpu);
        let kept = reuse && shape.as_ref().is_some_and(|(b, w, _, _)| *b == bi && *w == weights);
        if !kept && mine.is_none() {
            *shape = body.reshaped(&weights).map(|s| {
                let rig = person_rig(body, &s, &lib.humanoid);
                (bi, weights.clone(), s, rig)
            });
        }
        // The person's own skeleton and legs: from their own mesh, else reshaped for their shape,
        // else the body's as made.
        let (skel, rig) = match (mine, shape.as_ref()) {
            (Some(o), _) => (&o.mesh.skeleton, o.mesh.feet.as_ref()),
            (None, Some((_, _, s, r))) => (s, r.as_ref()),
            (None, None) => (&body.skeleton, body.feet.as_ref()),
        };
        if mine.is_some() || shape.is_some() {
            report.shaped += 1;
        }
        // A shoe's sole stands the person this much higher (their own mesh's lift, model metres).
        let lift = mine.map_or(0.0, |o| o.mesh.lift);
        // The shape targets the GPU adds: none for an own mesh (its shape is in its vertices).
        let gpu_weights: &[(usize, f32)] = if mine.is_some() { &[] } else { &weights };
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
        let rotation = body_rotation(p.up, p.facing);
        let origin = p.feet + p.up * (lift * scale);
        let world = Mat4::from_scale_rotation_translation(Vec3::splat(scale), rotation, origin);
        // INTERACTIONS (rung 2b, humanity_animation::interact, engine::interact): the figure's track
        // follows the person's act; its phase's clip or target pose is blended in from the walk, the
        // hips warped onto THIS seat (motion warping, from the enter clip's contact frame), and the
        // hands and feet put on their contacts below, after the feet.
        // An anchor index from a library read before this one is not this library's (a hot reload).
        if track.is_some_and(|t| t.anchor >= lib.anchors.anchors.len()) {
            *track = None;
        }
        let act_in = p.act.filter(|a| a.anchor < lib.anchors.anchors.len());
        update_track(track, act_in.as_ref(), lib, &body.clips, dt);
        let hand_rig = body.hands.as_ref();
        let mut engage_w = 0.0_f32;
        let mut act_contacts: Option<[f32; 2]> = None;
        let act = track.and_then(|tr| hand_rig.map(|h| (tr, h)));
        if let Some((tr, h)) = act {
            let def = &lib.anchors.anchors[tr.anchor];
            report.acting += 1;
            let idle = body.clips.index("idle");
            let contact_s = humanity_animation::interact::contact_time(def, &body.clips, skel, h.hips);
            let release_s = humanity_animation::interact::release_time(def, &body.clips, skel, h.hips);
            // THE SEAT'S DROP: how far under the hips joint this person's seat flesh (trousers on)
            // stands in the anchor's own pose, measured on the mesh they are drawn with, so the warp
            // rests the flesh, not the joint, on the seat. Measured when the anchor or their mesh
            // changes, and every frame with the reuse switch off.
            let hips_c = def.contact(humanity_animation::interact::ContactPart::Hips);
            let mesh_key = mine.map_or_else(|| weights_key(bi, &weights), |o| o.mesh.key);
            let drop = match (*seat_drop, hips_c) {
                (_, None) => 0.0,
                (Some((a, k, d)), Some(_)) if reuse && a == tr.anchor && k == mesh_key => d,
                (_, Some(_)) => {
                    let probe = humanity_animation::Engagement { def, phase: humanity_animation::Phase::Hold, t: 0.0, hips_target: Some(Vec3::new(0.0, 1.0, 0.0)), warp: false, contact_s, release_s };
                    humanity_animation::interact::pose(&probe, &body.clips, skel, &lib.humanoid, h, idle, &rt.pose, &mut rt.pose2, &mut rt.scratch);
                    pose::model_matrices(skel, &rt.pose2, &mut rt.model);
                    rt.rows.clear();
                    pose::palette_rows(skel, &rt.model, &mut rt.rows);
                    let hips_at = rt.model[h.hips].w_axis.truncate();
                    let (model, seat) = mine.map_or((&body.model, &body.seat_vertices), |o| (&o.mesh.model, &o.mesh.seat_vertices));
                    let forward = (def.kind == humanity_animation::AnchorKind::Sit).then_some(Vec3::Z);
                    let d = crate::engine::anchor_probe::seat_drop(model, seat, gpu_weights, &rt.rows, hips_at, forward).unwrap_or(humanity_animation::interact::default_back_drop());
                    *seat_drop = Some((tr.anchor, mesh_key, d));
                    d
                }
            };
            // Where the hips joint goes (model space, the person standing at the anchor's root): over
            // the seat's contact by the drop. Model space stands on the shoes' soles (`lift`), so the
            // seat's height over the floor is that much less in it.
            let hips_target = hips_c.map(|c| {
                let local = def.in_person_frame(c.at) / scale;
                Vec3::new(local.x, local.y - lift + drop, local.z)
            });
            let e = humanity_animation::Engagement { def, phase: tr.phase, t: tr.t, hips_target, warp: warp_on, contact_s, release_s };
            let w = humanity_animation::interact::pose(&e, &body.clips, skel, &lib.humanoid, h, idle, &rt.pose, &mut rt.pose2, &mut rt.scratch);
            std::mem::swap(&mut rt.pose, &mut rt.pose2);
            engage_w = w.engage;
            // The feet's contacts: the phase clip's labels, blended in with the pose.
            if let Some(ci) = def.phase(tr.phase).clip.as_deref().and_then(|c| body.clips.index(c)) {
                let c = body.clips.get(ci).contacts_at(tr.t);
                let a = animator.contacts();
                act_contacts = Some([a[0] + (c[0] - a[0]) * engage_w, a[1] + (c[1] - a[1]) * engage_w]);
            }
            if p.key == FigureKey::Me {
                me_act = Some((def.id.clone(), tr.phase));
            }
        }
        pose::model_matrices(skel, &rt.pose, &mut rt.model);
        let act_kind = act.map(|(tr, _)| lib.anchors.anchors[tr.anchor].kind);
        // FEET ON THE WORLD (rung 2a, humanity_animation::feet): the near bodies in the realistic
        // animation mode get the full solve (leg IK, the foot turned to the slope about its toe,
        // foot locking, stair footsteps), every other body the pelvis offset alone.
        let mode = match rig {
            // Lying, or on a ladder (rung 2b): no ground under the feet; the climb's own limbs put
            // them on its rungs.
            Some(_) if matches!(act_kind, Some(humanity_animation::AnchorKind::Lie | humanity_animation::AnchorKind::Climb)) && engage_w > 0.0 => FootMode::Off,
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
                // Seated (rung 2b), the hips are the seat's: the pelvis is not moved for the feet.
                let pelvis_fixed = act_kind == Some(humanity_animation::AnchorKind::Sit) && engage_w > 0.0;
                let frame = humanity_animation::FootFrame { world, contacts: act_contacts.unwrap_or_else(|| animator.contacts()), airborne: p.airborne, dt, pelvis_fixed };
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
        // HANDS AND FEET ON THEIR CONTACTS (rung 2b): the ladder's rungs, a workbench's surface, a
        // door's handle, by two-bone arm (and, on the ladder, leg) IK with the grips' fingers.
        let to_model = world.inverse();
        let mut climb_hands_on: [bool; 2] = [false; 2];
        // The targets are worked out with the IK off too (the red twin), so the probe still knows
        // which hands the climb has on a rung and measures them where they hang.
        if let Some((tr, h)) = act {
            let def = &lib.anchors.anchors[tr.anchor];
            let reach = humanity_animation::interact::climb_reach(h, &rt.model);
            let limbs = contact_limbs(lib, def, tr.placed.as_ref(), &to_model, p, scale, rig, Some(h), reach, &mut climb_hands_on);
            if let (Some(mut limbs), true) = (limbs, hand_ik_on) {
                limbs.weight *= engage_w;
                humanity_animation::interact::apply_limbs(h, rig, skel, &lib.humanoid, &lib.grips, &mut rt.pose, &mut rt.model, &limbs);
            }
        }
        // WHAT THEY HOLD (rung 2b): each held thing's grip on its hand's fingers; a thing that takes both
        // hands puts both on its shaft, carried in front (the prop itself is not drawn yet).
        if let (None, Some(h), true) = (act, hand_rig, p.held.iter().any(Option::is_some)) {
            match p.second_hand {
                Some(second) if hand_ik_on => {
                    let limbs = humanity_animation::interact::two_handed_limbs(h, &rt.model, p.held[1], second);
                    humanity_animation::interact::apply_limbs(h, rig, skel, &lib.humanoid, &lib.grips, &mut rt.pose, &mut rt.model, &limbs);
                }
                _ => {
                    for side in 0..2 {
                        if let Some(g) = p.held[side].and_then(|g| lib.grips.grips.get(g)) {
                            humanity_animation::interact::apply_grip(&h.arms[side], skel, &mut rt.pose, g, 1.0);
                        }
                    }
                    pose::model_matrices(skel, &rt.pose, &mut rt.model);
                }
            }
        }
        // LOOK-AT (rung 2b): the head, then the eyes, toward what they look at, at a person's turn rate
        // within the head's limits (near bodies and the player).
        if let (Some(h), true) = (hand_rig, rung2b_on && (tier[i] == DrawTier::Near || p.key == FigureKey::Me)) {
            let target = p.look.map(|w| to_model.transform_point3(w));
            humanity_animation::interact::look_at(h, skel, &mut rt.pose, &mut rt.model, look, target, 1.0, dt);
        }
        // TRUE FIRST PERSON (rung 2b): the eye rides the drawn head (its eyes' midpoint), low-passed, and
        // the head and neck are folded away for the eye camera.
        if p.first_person {
            if let Some(h) = hand_rig {
                let eye_model = match h.eyes {
                    [Some(l), Some(r)] => (rt.model[l].w_axis.truncate() + rt.model[r].w_axis.truncate()) * 0.5,
                    _ => h.head.map_or(Vec3::new(0.0, 1.6, 0.1), |j| rt.model[j].w_axis.truncate() + Vec3::new(0.0, 0.08, 0.1)),
                };
                my_eye = Some(world.transform_point3(eye_model));
                if fp_head_hidden {
                    if let Some(neck) = first_person_fold(h) {
                        humanity_animation::interact::collapse_subtree(skel, &mut rt.model, neck);
                    }
                }
            }
        }
        rt.rows.clear();
        pose::palette_rows(skel, &rt.model, &mut rt.rows);
        let morph = pose::morph_rows(gpu_weights, body.model.vertex_count() as u32);
        let index = renderer.skin_push_palette(pose::affine_rows(&world), &morph, &rt.rows);
        // The interaction probe (showcase `anchor_probe`, rung 2b): the drawn body against what it
        // touches, near bodies in an interaction, and the first-person eye.
        if let Some(aprobe) = aprobe.as_mut() {
            let (model, seat_v, hand_v) = mine.map_or((&body.model, &body.seat_vertices, &body.hand_vertices), |o| (&o.mesh.model, &o.mesh.seat_vertices, &o.mesh.hand_vertices));
            if let (Some((tr, h)), true) = (act, tier[i] == DrawTier::Near) {
                let def = &lib.anchors.anchors[tr.anchor];
                let hips_world = world.transform_point3(rt.model[h.hips].w_axis.truncate());
                probe_contacts(aprobe, p, body, def, tr, &world, model, seat_v, hand_v, gpu_weights, &rt.rows, climb_hands_on, engage_w, hips_world);
            }
            if p.first_person {
                let eye = my_eye.unwrap_or(p.feet + p.up * 1.6);
                let pts = crate::engine::anchor_probe::skinned_drawn(model, gpu_weights, &rt.rows, &world, 3);
                aprobe.note(p.key, &body.choice, "first_person", "FirstPerson", 1, "eye_nearest_m", crate::engine::anchor_probe::nearest(&pts, eye));
                aprobe.note(p.key, &body.choice, "first_person", "FirstPerson", 1, "body_below_m", pts.iter().map(|q| (eye - *q).dot(p.up)).reduce(f32::max));
            }
        }
        // The foot probe (showcase `foot_probe`): the drawn feet against the ground, near bodies.
        if let Some(probe) = rt.probe.as_mut().filter(|_| tier[i] == DrawTier::Near) {
            let (model, foot_verts) = mine.map_or((&body.model, &body.foot_vertices), |o| (&o.mesh.model, &o.mesh.foot_vertices));
            let clear = crate::engine::foot_probe::clearances(model, foot_verts, gpu_weights, &rt.rows, &world, p.up, &person_ground);
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
        // The hair's fringe materials (Scheuermann's second pass), by primitive, for the transparent
        // list once the near meshes are known.
        let mut fringes: Vec<(usize, usize)> = Vec::new();
        let materials: Vec<usize> = match (mine, lib.look.as_ref()) {
            // Their own mesh: the body's regions (a region painted as the clothes of a slot a worn
            // garment fills takes the skin's colour: it is the body under the garment, showing only
            // at its edges), then each garment, textured once its texture has loaded (its mean
            // colour until then, and with the textures' A/B off).
            (Some(o), Some(look)) => {
                let worn: Vec<humanity_animation::GarmentKind> = o.mesh.parts.iter().filter_map(|part| match part {
                    humanity_animation::Part::Garment(g) => look.garments.get(*g).map(|x| x.def.kind),
                    _ => None,
                }).collect();
                let mut out = Vec::with_capacity(o.mesh.parts.len());
                for (k, part) in o.mesh.parts.iter().enumerate() {
                    let m = match *part {
                        humanity_animation::Part::Region(r) => {
                            let region = &body.def.regions[r];
                            let under = matches!(&region.tint, Tint::Slot(s) if worn.iter().any(|kind| garment_slots(*kind).contains(&s.as_str())));
                            let c = if under { region_colour(lib, &Tint::Skin, p) } else { region_colour(lib, &region.tint, p) };
                            tinted_material(&mut rt.materials, renderer, c, region.roughness)
                        }
                        humanity_animation::Part::Garment(g) => {
                            let def = &look.garments[g].def;
                            let c = garment_colour(lib, def, p);
                            let ready = if rt.textures_on {
                                garment_texture(&mut rt.garment_tex, &rt.garment_materials, &mut rt.pools, renderer, &data_dir, look, lib.characters.tiers.near_cap, g, tex_size, rt.clock, &mut texture_uploads)
                            } else {
                                None
                            };
                            match ready {
                                Some((template, has_normal, luma)) if def.alpha => {
                                    use crate::renderer::materials::{HAIR_CUTOFF, MATERIAL_TYPE_HAIR};
                                    let q = colour_key(c);
                                    let hn = if has_normal { 1.0 } else { 0.0 };
                                    let mut like = |pass: u8, cut: f32| {
                                        *rt.garment_materials.entry((g, q, pass)).or_insert_with(|| {
                                            renderer.add_material_like(template, [c[0], c[1], c[2], 1.0], def.roughness, MATERIAL_TYPE_HAIR, [1.0, hn, cut, luma]).unwrap_or(template)
                                        })
                                    };
                                    let core = like(0, HAIR_CUTOFF);
                                    fringes.push((k, like(1, -HAIR_CUTOFF)));
                                    core
                                }
                                Some((template, _, _)) => template,
                                None => tinted_material(&mut rt.materials, renderer, c, def.roughness),
                            }
                        }
                    };
                    out.push(m);
                }
                out
            }
            // The shared body (middle bodies, and anyone waiting for their own mesh): a region painted
            // as a slot's clothes takes the colour of the garment that fills that slot (its
            // texture's mean colour), so a person keeps their colours across the near line.
            _ => {
                let worn: Vec<&humanity_animation::look::GarmentDef> = match lib.look.as_ref() {
                    Some(look) if !rt.fallback_only => lib.dress_for(bi, p.wearer, figure_seed(p.key), &p.dress, &model_of).into_iter().filter_map(|g| look.garments.get(g).map(|x| &x.def)).collect(),
                    _ => Vec::new(),
                };
                body.primitive_regions
                    .iter()
                    .map(|&r| {
                        let region = &body.def.regions[r];
                        let filled = match &region.tint {
                            Tint::Slot(s) => worn.iter().find(|d| garment_slots(d.kind).contains(&s.as_str())).map(|d| [d.colour.0, d.colour.1, d.colour.2]),
                            _ => None,
                        };
                        tinted_material(&mut rt.materials, renderer, filled.unwrap_or_else(|| region_colour(lib, &region.tint, p)), region.roughness)
                    })
                    .collect()
            }
        };
        let near = tier[i] == DrawTier::Near || (!vs_ready && fallback_used < COMPUTE_FALLBACK_CAP);
        if near {
            if tier[i] == DrawTier::Near {
                report.near += 1;
            } else {
                report.fallback += 1;
                fallback_used += 1;
            }
            // Near places are numbered per shared body; a person's own body has its own place.
            let meshes = if mine.is_some() {
                renderer.skin_near(gpu_body, 0, index)
            } else {
                slots[bi] += 1;
                renderer.skin_near(gpu_body, slots[bi] - 1, index)
            };
            let in_render_space = me_in_render_space && p.key == FigureKey::Me;
            let list = if in_render_space { &mut *render_space } else { &mut *all_objects };
            for (mesh, material) in meshes.iter().copied().zip(materials.iter().copied()) {
                list.push(RenderObject { fade: 0.0, position: origin, rotation, scale: Vec3::splat(scale), mesh, material });
            }
            // The hair's soft fringe, blended over the solid core in the transparent list (home
            // frame only: the player's own body on a planet keeps the core alone, a known limit).
            if !in_render_space {
                for &(k, material) in &fringes {
                    if let Some(&mesh) = meshes.get(k) {
                        transparent.push(RenderObject { fade: 0.0, position: origin, rotation, scale: Vec3::splat(scale), mesh, material });
                    }
                }
            }
        } else if vs_ready {
            report.middle += 1;
            for (region, material) in materials.iter().copied().enumerate() {
                renderer.skin_middle(gpu_body, region, material, index);
            }
        }
        rt.figures.insert(p.key, fig);
    }
    // Garments nobody near has worn for a while give their textures back.
    release_unworn_garments(&mut rt.garment_tex, &rt.garment_materials, &mut rt.pools, renderer, rt.clock);
    let frame = rt.frame;
    // Beyond the cap, the own meshes of the people longest out of the near tier go now.
    let mut kept: Vec<(f32, FigureKey)> = rt.figures.iter().filter(|(_, f)| f.own.is_some() && f.away_s > 0.0).map(|(k, f)| (f.away_s, *k)).collect();
    if kept.len() > keep_cap {
        kept.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (_, k) in kept.iter().take(kept.len() - keep_cap) {
            if let Some(f) = rt.figures.get_mut(k) {
                f.away_s = OWN_KEEP_S + 1.0;
            }
        }
    }
    // People gone from the frame, and own meshes not wanted for a while, give their room in the
    // own-mesh pool back.
    for f in rt.figures.values_mut() {
        if f.seen != frame || f.away_s > OWN_KEEP_S {
            f.building = None;
            f.pending = None;
            if let Some(own) = f.own.take() {
                renderer.skin_release_body(own.gpu);
            }
        }
    }
    rt.figures.retain(|_, f| f.seen == frame);
    // What a full solve cost this frame, eased (one slow frame does not hand the crowd to the
    // pelvis offset); measured again every frame, so a faster frame raises the count back.
    if full_n > 0 {
        let per = full_us / full_n as f32;
        rt.full_solve_us = if rt.full_solve_us > 0.0 { rt.full_solve_us + (per - rt.full_solve_us) * 0.1 } else { per };
    }
    rt.report = report;
    rt.me_act = me_act;
    // The first-person eye rides the drawn head (rung 2b), low-passed so a step does not shake the
    // picture (yaw and pitch stay the player's); Reduce motion keeps the eye where the walk puts it.
    let ride = my_eye.filter(|_| !state.gui_state.settings.reduce_motion);
    let want = ride.map(|eye| eye - state.camera.position);
    let me_fig = rt.figures.get_mut(&FigureKey::Me);
    match (want, me_fig) {
        (Some(w), Some(fig)) => {
            let k = 1.0 - (-dt / HEAD_RIDE_FOLLOW_S).exp();
            let now = fig.view_offset.map_or(w, |v| v + (w - v) * k);
            fig.view_offset = Some(now);
            state.camera.view_offset = now;
            state.controller.view_from_head = true;
        }
        (_, fig) => {
            if let Some(fig) = fig {
                fig.view_offset = None;
            }
            state.camera.view_offset = Vec3::ZERO;
            state.controller.view_from_head = false;
        }
    }
    // The interaction probe writes its readout once its time is up.
    if let Some(mut probe) = aprobe {
        probe.frames += 1;
        if rt.clock >= probe.until {
            let note = crate::engine::anchor_probe::write(&probe);
            log::info!("Interaction probe: {note}");
        } else {
            state.interact.probe = Some(probe);
        }
    }
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

#[cfg(test)]
#[path = "animation_look_tests.rs"]
mod look_tests;

#[cfg(test)]
#[path = "animation_interact_tests.rs"]
mod interact_tests;
