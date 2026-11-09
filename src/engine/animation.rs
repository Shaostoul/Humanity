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
use humanity_animation::{pose, tiers, Animator, CharacterLibrary, DrawTier, MotionTracker, Pose, Tint};

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
    /// Look height multiplier (1.0 = the body's own height).
    pub height: f32,
    /// The wearer kind whose defaults fill what the person does not say ("player", "crew").
    pub wearer: &'static str,
    pub skin: Option<[f32; 3]>,
    pub hair: Option<[f32; 3]>,
    pub outfit: Option<humanity_core::cosmetics::OutfitColors>,
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
}

/// What the last frame drew (the Dev page and the rigs read it).
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Report {
    pub people: usize,
    pub near: usize,
    pub middle: usize,
    /// Middle bodies compute-skinned because the vertex-shader PSO was not ready.
    pub fallback: usize,
    pub body: String,
    pub error: Option<String>,
}

struct Figure {
    animator: Animator,
    motion: MotionTracker,
    seen: u64,
}

pub(crate) struct AnimationRuntime {
    lib: Option<CharacterLibrary>,
    body: Option<usize>,
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
    pub(crate) crowd: Vec<CrowdFigure>,
    pub(crate) report: Report,
    /// The box figure's meshes and the crew's amber material, for the dev crowd in the boxes arm.
    box_meshes: Option<(usize, usize, usize)>,
    pose: Pose,
    model: Vec<Mat4>,
    rows: Vec<[f32; 4]>,
}

impl Default for AnimationRuntime {
    fn default() -> Self {
        AnimationRuntime {
            lib: None,
            body: None,
            tried: false,
            reread: false,
            read_at: None,
            figures: HashMap::new(),
            materials: HashMap::new(),
            frame: 0,
            clock: 0.0,
            bodies_on: true,
            crowd: Vec::new(),
            report: Report::default(),
            box_meshes: None,
            pose: Pose::default(),
            model: Vec::new(),
            rows: Vec::new(),
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
        self.bodies_on && self.lib.is_some() && self.body.is_some()
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
            let same_model = rt.lib.as_ref().is_some_and(|old| old.model == lib.model) && rt.body.is_some();
            if !same_model {
                state.renderer.skin_clear_bodies();
                rt.body = Some(state.renderer.skin_register_body(&lib.model));
                log::info!(
                    "Characters: body {} ({} joints, {} vertices, {} regions), {} clips, graph {}",
                    lib.body.id,
                    lib.skeleton.joint_count(),
                    lib.model.vertex_count(),
                    lib.model.primitives.len(),
                    lib.clips.clips.len(),
                    lib.graph.id
                );
            }
            // A new graph may have other states: every figure starts again.
            if rt.lib.as_ref().map_or(true, |old| old.graph != lib.graph) {
                rt.figures.clear();
            }
            rt.report.body = lib.body.id.clone();
            rt.report.error = None;
            rt.lib = Some(lib);
        }
        Err(e) => {
            if rt.report.error.as_deref() != Some(e.as_str()) {
                log::error!("Characters: the character data did not load, so the box figures stand in: {e}");
            }
            rt.report.error = Some(e);
            rt.lib = None;
            rt.body = None;
        }
    }
}

/// The facing angle (turn of +Z about +Y) of a body looking along a camera at `yaw` (which looks
/// along (sin yaw, 0, -cos yaw), renderer/camera.rs `forward_xz`).
pub(crate) fn facing_for_camera_yaw(yaw: f32) -> f32 {
    std::f32::consts::PI - yaw
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
                CrowdFigure { motion, phase, born: 0.0 }
            })
            .collect(),
    )
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
    (points.len() >= 2 && speed > 0.0).then_some(CrowdFigure { motion: CrowdMotion::Path { points, speed }, phase: 0.0, born: 0.0 })
}

/// One region's colour for a person.
pub(crate) fn region_colour(lib: &CharacterLibrary, tint: &Tint, p: &Person) -> [f32; 3] {
    let wearer = lib.wearer(p.wearer);
    let skin = p.skin.or(wearer.map(|w| w.skin)).unwrap_or([0.7, 0.55, 0.45]);
    let hair = p.hair.or(wearer.map(|w| w.hair)).unwrap_or([0.2, 0.15, 0.1]);
    match tint {
        Tint::Skin => skin,
        Tint::Hair => hair,
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
    let mut out = Vec::new();
    let me_look = || {
        let app = &state.gui_state.appearance;
        let outfit = humanity_core::cosmetics::resolve_outfit_colors(&state.gui_state.outfit, &state.cosmetics);
        (app.height_scale.clamp(0.5, 2.0), Some(app.skin_tone), Some(app.hair_color), Some(outfit))
    };
    if showroom {
        let (height, skin, hair, outfit) = me_look();
        out.push(Person {
            key: FigureKey::Showroom,
            feet: state.avatar_base + Vec3::new(0.0, SHOWROOM_PODIUM_M, 0.0),
            facing: 0.0,
            height,
            wearer: "player",
            skin,
            hair,
            outfit,
        });
        return out;
    }
    if state.camera.mode == crate::renderer::camera::CameraMode::ThirdPerson && !state.gui_state.construction_active {
        let (height, skin, hair, outfit) = me_look();
        out.push(Person {
            key: FigureKey::Me,
            feet: state.camera.position - Vec3::new(0.0, state.controller.eye_height(), 0.0),
            facing: facing_for_camera_yaw(state.camera.yaw),
            height,
            wearer: "player",
            skin,
            hair,
            outfit,
        });
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
            height: r.look.map_or(1.0, |l| l.height),
            wearer: "player",
            skin: r.look.map(|l| l.skin),
            hair: r.look.map(|l| l.hair),
            outfit: None,
        });
    }
    for (_e, (t, n)) in world.query::<(&humanity_core::ecs::components::Transform, &crate::net::sync::RemoteNpc)>().iter() {
        out.push(Person {
            key: FigureKey::Crew(n.entity_id),
            feet: t.position - Vec3::new(0.0, CREW_POSITION_OVER_FEET_M, 0.0),
            facing: facing_of_turn(t.rotation),
            height: 1.0,
            wearer: "crew",
            skin: None,
            hair: None,
            outfit: None,
        });
    }
    let clock = state.animation.clock;
    for (i, c) in state.animation.crowd.iter().enumerate() {
        let (feet, facing) = crowd_place(c, clock);
        out.push(Person { key: FigureKey::Crowd(i as u32), feet, facing, height: 1.0, wearer: "crew", skin: None, hair: None, outfit: None });
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

/// A stable stagger per figure, so a crowd does not step in unison.
fn stagger(key: FigureKey) -> f32 {
    let n: u64 = match key {
        FigureKey::Me => 0,
        FigureKey::Showroom => 1,
        FigureKey::Player(id) => 2 + id as u64,
        FigureKey::Crew(id) => 7 + id,
        FigureKey::Crowd(i) => 11 + i as u64,
    };
    ((n.wrapping_mul(2_654_435_761) % 1000) as f32) / 1000.0
}

/// Draw every person as a skinned, animated body. Returns false when the bodies are not what the
/// frame draws (data missing, or the A/B switch on boxes): lib.rs then draws the box figures.
pub(crate) fn draw_people(state: &mut EngineState, all_objects: &mut Vec<RenderObject>, showroom: bool, dt: f32) -> bool {
    let _cost = humanity_core::frame_costs::stage("cpu.animation");
    ensure_loaded(state);
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
    let rt = &mut state.animation;
    let renderer = &mut state.renderer;
    let lib = rt.lib.as_ref().expect("draws_bodies checked it");
    let body = rt.body.expect("draws_bodies checked it");
    let distances: Vec<f32> = people.iter().map(|p| (p.feet + Vec3::Y - cam).length()).collect();
    let tier = tiers::assign(&distances, &lib.characters.tiers);
    renderer.skin_begin_frame();
    let vs_ready = renderer.skin_middle_ready();
    let mut report = Report { people: people.len(), body: lib.body.id.clone(), ..Default::default() };
    let mut slot = 0usize;
    for (i, p) in people.iter().enumerate() {
        let fig = rt.figures.entry(p.key).or_insert_with(|| {
            let s = stagger(p.key);
            Figure { animator: Animator::with_offset(&lib.graph, s, s * 4.0), motion: MotionTracker::default(), seen: 0 }
        });
        fig.seen = rt.frame;
        let input = fig.motion.observe(p.feet, p.facing, dt, lib.graph.smoothing_s, lib.graph.teleport_mps);
        fig.animator.update(&lib.graph, &lib.clips, &lib.skeleton, &lib.humanoid, &input, dt, &mut rt.pose);
        pose::model_matrices(&lib.skeleton, &rt.pose, &mut rt.model);
        rt.rows.clear();
        pose::palette_rows(&lib.skeleton, &rt.model, &mut rt.rows);
        let scale = lib.scale_for(p.height);
        let rotation = Quat::from_rotation_y(p.facing);
        let world = Mat4::from_scale_rotation_translation(Vec3::splat(scale), rotation, p.feet);
        let index = renderer.skin_push_palette(pose::affine_rows(&world), &rt.rows);
        let materials: Vec<usize> = lib
            .primitive_regions
            .iter()
            .map(|&r| {
                let region = &lib.body.regions[r];
                let c = region_colour(lib, &region.tint, p);
                let q = |x: f32| (x.clamp(0.0, 1.0) * 63.0).round() as u8;
                let key = [q(c[0]), q(c[1]), q(c[2]), q(region.roughness)];
                *rt.materials.entry(key).or_insert_with(|| {
                    renderer.add_material_full([c[0], c[1], c[2], 1.0], 0.0, region.roughness, crate::renderer::materials::MATERIAL_TYPE_CHARACTER, 0.0)
                })
            })
            .collect();
        let near = tier[i] == DrawTier::Near || (!vs_ready && slot < COMPUTE_FALLBACK_CAP);
        if near {
            if tier[i] == DrawTier::Near {
                report.near += 1;
            } else {
                report.fallback += 1;
            }
            let meshes = renderer.skin_near(body, slot, index);
            slot += 1;
            for (mesh, material) in meshes.into_iter().zip(materials.iter().copied()) {
                all_objects.push(RenderObject { fade: 0.0, position: p.feet, rotation, scale: Vec3::splat(scale), mesh, material });
            }
        } else if vs_ready {
            report.middle += 1;
            for (region, material) in materials.iter().copied().enumerate() {
                renderer.skin_middle(body, region, material, index);
            }
        }
    }
    let frame = rt.frame;
    rt.figures.retain(|_, f| f.seen == frame);
    rt.report = report;
    renderer.skin_submit();
    true
}

#[cfg(test)]
#[path = "animation_tests.rs"]
mod tests;
