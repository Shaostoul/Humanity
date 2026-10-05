//! Building where you stand, on the ship or on a planet (2026-09-27, the real
//! fix for BUG-102).
//!
//! Built pieces used to live only in the HOME frame and be drawn at the
//! station's offset. Aboard that is right; on a planet the camera stays put
//! while the ship frame moves under it, so a piece placed from the surface
//! was drawn at the station, hundreds of km up, and the shelter test
//! followed the player through the rain. v0.1390.0 gated both to the ship.
//! This module is what lets a player build a shelter on a planet's ground,
//! where the weather is:
//!
//! - WHICH FRAME the player is in this frame ([`ghost`] for placing,
//!   [`player_frame`] for the shelter, the stations and the bed and chest
//!   look ray): the home frame aboard, or on a planet's ground the build
//!   site they stand in (`construction::site`), with their eye and look
//!   direction converted into that site's flat Y-up frame from the frame
//!   lock's anchor (the eye in the body's frame, f64) and the camera's look.
//!   Everything the frame reads from the engine is read in ONE place,
//!   [`PlayerView::of`], so the rules are tested without a window.
//! - THE GROUND in that frame ([`Ground`]): the drawn terrain the player
//!   stands on (`surface_walk::walk_ground_radius`, the same floor the walk
//!   clamp, the grass and the near trees use), so a piece is set on the
//!   ground you can see and the aim meets the ground under the crosshair.
//! - THE DRAW ([`push_render_objects`], moved here from lib.rs): home pieces
//!   into the home-frame list as before; site pieces at `render_off + rot *
//!   p` in the CELESTIAL list with the terrain, the near trees and the OSM
//!   buildings, at every distance, so the ground hides them behind a hill,
//!   they cast the sun's shadow, and anything else in front of them (a
//!   chest inside a hut, the grass on a slope) stays in front.
//!
//! THE NEAR COPY IS GONE (2026-09-27, the review of this module). Site pieces
//! within 2 m of the eye used to be drawn a second time in the scene pass,
//! because the celestial pass's near plane was 1 m and its corner reached
//! 2.27 m at the widest view, so a wall beside you was cut open. But the
//! scene pass CLEARS depth, so the copy was painted over everything drawn
//! only in the celestial pass: a chest inside the hut vanished behind its own
//! wall, and a wall's buried base painted over the grass on a slope. The
//! celestial near plane is now 5 cm (`renderer::camera::CELESTIAL_NEAR_M`),
//! which costs no depth precision anywhere (the tests there prove it), and
//! every site piece is drawn once, in the celestial pass.
//!
//! `render_off` and `rot` are the body's centre and orientation in render
//! space THIS frame, handed to [`note_body`] by the celestial loop: the very
//! values it places that body's terrain with (lib.rs `rot_d`, computed by
//! [`body_render_rot`]), so a piece stands exactly on the ground it was set
//! on.

use crate::ecs::components::Transform;
use crate::engine::state::EngineState;
use crate::renderer::camera::CameraMode;
use crate::renderer::RenderObject;
use crate::systems::construction::site::{self, PlanetSite};
use crate::systems::construction::{placement, BlueprintRegistry, Construction, Structure};
use crate::terrain::planet::PlanetDef;
use glam::{DQuat, DVec3, Vec3};

/// How high the eye may be over the ground and still place a piece, metres:
/// standing (1.7 m plus the walk clamp's clearance, up to 2.5 m on the
/// elevation field) with a little air for a hop. Higher is flying.
const MAX_EYE_OVER_GROUND_M: f64 = 4.5;
/// Site pieces farther than this from the eye are not drawn (sub-pixel).
const DRAW_RANGE_M: f32 = 30_000.0;
/// Ground over connected ocean deeper than this under the sea's surface is
/// sea floor (the same 60 m backstop the walk clamp uses for Earth).
const SEA_FLOOR_M: f64 = 60.0;

/// This frame's render placement of the body the player is locked to: its
/// centre in render space and its orientation, exactly as the celestial
/// loop placed its terrain.
pub(crate) struct BodyFrame {
    pub body: String,
    pub render_off: DVec3,
    pub rot: DQuat,
}

/// Called from inside the celestial loop for every body it places, with the
/// SAME render offset and rotation that loop places the body's terrain with
/// (its `rot_d`: one binding, never a recomputation here). Keeps the
/// placement of the one the player is locked to, for [`push_render_objects`]
/// later in the same frame (which takes it, so it is never a frame stale).
pub(crate) fn note_body(state: &mut EngineState, body: &str, render_off: DVec3, rot: DQuat) {
    if state.frame_lock_body.as_deref() == Some(body) {
        state.planet_body_frame = Some(BodyFrame { body: body.to_string(), render_off, rot });
    }
    // A pack left on this body's ground is drawn and marked in the same frame (2026-10-04).
    crate::engine::death_pack::note_body(state, body, render_off, rot);
}

/// THE rotation taking a direction in a body's unrotated frame to render
/// space: the planet's spin, composed with the hull frame when riding the
/// station. The celestial loop places the terrain with this (lib.rs
/// `rot_d`), and the look conversions below use it, so there is one formula.
pub(crate) fn body_render_rot(station_ride: bool, station_world_rot: DQuat, spin: f64) -> DQuat {
    crate::station::hull_frame_rot(station_ride, station_world_rot) * DQuat::from_rotation_y(spin)
}

/// What the frame needs to know about the player, read from the engine in
/// this ONE place so everything that follows is pure (and tested without a
/// window). On a planet the camera stays parked while the ship frame moves
/// under it, so the player's place there is the frame lock's ANCHOR (the eye
/// in the body's unrotated frame, f64) and never the camera's position,
/// which is only the player's place aboard.
#[derive(Debug, Clone)]
pub(crate) struct PlayerView {
    pub aboard: bool,
    /// The camera in the home frame, and its look in render space.
    pub camera_pos: Vec3,
    pub forward: Vec3,
    pub eye_height: f32,
    /// The body the frame lock holds, and the eye in its frame.
    pub body: Option<String>,
    pub anchor: DVec3,
    /// That body's rotation into render space this frame ([`body_render_rot`]).
    pub rot: DQuat,
}

impl PlayerView {
    pub(crate) fn of(state: &EngineState) -> Self {
        PlayerView {
            aboard: state.aboard_station,
            camera_pos: state.camera.position,
            forward: state.camera.forward(),
            eye_height: state.controller.eye_height(),
            body: state.frame_lock_body.clone(),
            anchor: state.frame_lock_anchor,
            rot: body_render_rot(state.station_ride, state.station_world_rot, state.current_spin),
        }
    }

    /// The look direction in the locked body's unrotated frame.
    fn look_body(&self) -> DVec3 {
        self.rot.inverse() * self.forward.as_dvec3()
    }
}

/// The ground of one body, as the player stands on it this frame.
pub(crate) struct Ground<'a> {
    def: &'a PlanetDef,
    hm: Option<&'a crate::terrain::planet_heightmap::PlanetHeightmap>,
    detail: crate::terrain::planet_chunks::DetailNoise,
    tiles: Option<&'a crate::terrain::terrain_tiles::TerrainTiles>,
    ocean: Option<&'a crate::terrain::ocean_mask::OceanMask>,
    drawn_depth: u8,
    earth: bool,
}

impl<'a> Ground<'a> {
    /// The ground of `body`, or None when it has no planet definition (no
    /// known surface to stand a piece on).
    pub(crate) fn for_body(state: &'a EngineState, body: &str) -> Option<Self> {
        let def = state.planet_defs.get(body)?;
        let earth = body == "earth";
        Some(Ground {
            def,
            hm: state.planet_heightmaps.get(body).map(|a| a.as_ref()),
            detail: crate::terrain::planet_chunks::DetailNoise::new(def.terrain_seed),
            tiles: earth.then_some(&state.terrain_tiles),
            ocean: if earth { state.ocean_mask.as_ref() } else { None },
            drawn_depth: state
                .planet_chunk_states
                .get(body)
                .and_then(|cs| cs.last_drawn.iter().map(|p| p.depth).max())
                .unwrap_or(0),
            earth,
        })
    }

    /// The ground's radius under a unit direction of the body's frame: the
    /// drawn patch surface when the renderer has drawn one here, else the
    /// elevation field (`surface_walk::walk_ground_radius`, the floor the
    /// player's own feet are clamped to). f64 end to end.
    pub(crate) fn radius(&self, dir: DVec3) -> f64 {
        let field = crate::engine::frame_lock::ground_radius_m(Some(self.def), self.hm, Some(&self.detail), self.tiles, dir);
        crate::surface_walk::walk_ground_radius(
            field,
            Some(self.def),
            self.hm,
            Some(&self.detail),
            self.tiles,
            self.ocean,
            self.drawn_depth,
            0.0,
            dir,
        )
        .0
    }

    /// The ground's height at the site-local point (x, z), in site-local
    /// metres.
    pub(crate) fn height(&self, site: &PlanetSite, x: f32, z: f32) -> f32 {
        let dir = site.to_body(Vec3::new(x, 0.0, z)).normalize();
        site.to_local(dir * self.radius(dir)).y
    }

    /// Is the ground under `dir` (radius `r`) the floor of a sea? Earth's
    /// connected-ocean mask, or any water world's ground far under its sea.
    pub(crate) fn under_water(&self, dir: DVec3, r: f64) -> bool {
        (self.earth && self.ocean.is_some_and(|m| m.is_ocean(dir.as_vec3())))
            || (self.def.has_water && r < self.def.radius - SEA_FLOOR_M)
    }
}

/// Why a piece in hand cannot be placed from where the player is. Each has
/// its own line under the crosshair ([`cannot_build_hint`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CannotBuild {
    NotFirstPerson,
    Driving,
    /// Neither aboard nor on a body: open space.
    OpenSpace,
    /// Over a body but flying, not standing on its ground.
    NotOnGround,
    /// On the sea, or aiming at it.
    OnWater,
}

/// The line under the crosshair when a piece in hand cannot be placed.
pub(crate) fn cannot_build_hint(name: &str, why: CannotBuild) -> String {
    let what = match why {
        CannotBuild::NotFirstPerson => "go to first person, on foot, to place it",
        CannotBuild::Driving => "get out of the vehicle to place it",
        CannotBuild::OpenSpace => "pieces are built on the ship or on a planet's ground, not in open space",
        CannotBuild::NotOnGround => "stand on the ground to place it",
        CannotBuild::OnWater => "pieces cannot be built on water",
    };
    format!("Placing {name}: {what}   [Esc] done")
}

/// Where the piece in hand would be built this frame.
pub(crate) struct Ghost {
    pub pose: Transform,
    /// The frame `pose` is in: None aboard, the build site on a planet.
    pub site: Option<PlanetSite>,
    /// How high it rests over the floor it was aimed at (a roof on walls).
    pub above_floor: f32,
    /// The same box already stands there: E would be a double spend.
    pub occupied: bool,
}

/// The ghost of `blueprint_id`, turned `quarter_turns`, where the crosshair
/// is: aboard on the deck (the home frame, as before), on a planet on the
/// ground under the crosshair in the build site the player stands in.
pub(crate) fn ghost(state: &EngineState, blueprint_id: &str, quarter_turns: u8) -> Result<Ghost, CannotBuild> {
    if state.camera.mode != CameraMode::FirstPerson {
        return Err(CannotBuild::NotFirstPerson);
    }
    if state.driving_vehicle.is_some() {
        return Err(CannotBuild::Driving);
    }
    let Some(reg) = state.data_store.get::<BlueprintRegistry>("blueprint_registry") else {
        return Err(CannotBuild::OpenSpace);
    };
    let Some(bp) = reg.get(blueprint_id) else { return Err(CannotBuild::OpenSpace) };
    let world = &state.game_world.world;
    let v = PlayerView::of(state);
    let (at, site) = if v.aboard {
        let floor = state.controller.ground_floor();
        (placement::aim_point(v.camera_pos, v.forward, floor), None)
    } else {
        let body = v.body.as_deref().ok_or(CannotBuild::OpenSpace)?;
        if !state.surface_walk_band {
            return Err(CannotBuild::NotOnGround);
        }
        let ground = Ground::for_body(state, body).ok_or(CannotBuild::NotOnGround)?;
        let up = v.anchor.normalize_or_zero();
        let under = ground.radius(up);
        if v.anchor.length() - under > MAX_EYE_OVER_GROUND_M {
            return Err(CannotBuild::NotOnGround);
        }
        if ground.under_water(up, under) {
            return Err(CannotBuild::OnWater);
        }
        let site = site::site_for(world, body, up * under);
        let eye = site.to_local(v.anchor);
        let look = site.dir_to_local(v.look_body());
        let at = placement::aim_point_on_ground(eye, look, |x, z| ground.height(&site, x, z));
        let dir = site.to_body(at).normalize();
        if ground.under_water(dir, ground.radius(dir)) {
            return Err(CannotBuild::OnWater);
        }
        (at, Some(site))
    };
    let pose = placement::placement_pose(bp, at, quarter_turns, world, reg, site.as_ref());
    let occupied = placement::occupied(world, &pose, site.as_ref());
    Ok(Ghost { above_floor: pose.position.y - at.y, pose, site, occupied })
}

/// The player in one frame: where the shelter test and the look ray run.
#[derive(Debug, Clone)]
pub(crate) struct PlayerFrame {
    /// None = the home frame; else the build site on a planet.
    pub site: Option<PlanetSite>,
    pub eye: Vec3,
    pub forward: Vec3,
    pub feet: Vec3,
}

/// The frame the player's built surroundings are in: the home frame aboard;
/// on a planet, the site of the nearest piece within reach
/// (`site::SITE_JOIN_M`); None anywhere else (open space, or a planet with
/// nothing built near).
pub(crate) fn player_frame(state: &EngineState) -> Option<PlayerFrame> {
    let v = PlayerView::of(state);
    let ground_r = || {
        let g = Ground::for_body(state, v.body.as_deref()?)?;
        Some(g.radius(v.anchor.normalize_or_zero()))
    };
    frame_of(&state.game_world.world, &v, ground_r)
}

/// [`player_frame`] over a [`PlayerView`]: aboard, the camera in the home
/// frame; on a planet, the anchor and the look in the nearest site
/// ([`player_at_site`]); the camera's position is never read there.
pub(crate) fn frame_of(world: &hecs::World, v: &PlayerView, ground_r: impl FnOnce() -> Option<f64>) -> Option<PlayerFrame> {
    if v.aboard {
        let feet = v.camera_pos - Vec3::Y * v.eye_height;
        return Some(PlayerFrame { site: None, eye: v.camera_pos, forward: v.forward, feet });
    }
    let body = v.body.as_deref()?;
    player_at_site(world, body, v.anchor, v.look_body(), ground_r)
}

/// The player in the site of the nearest piece on `body`, from their eye and
/// look direction in the body's frame and the ground radius under them
/// (`ground_r`, asked only when there is a site; None when unknown). Their
/// feet are on that ground while they stand on it, else a standing height
/// under the eye (flying over a roof is not being under it). Pure, so the
/// planet shelter chain is tested without a window.
pub(crate) fn player_at_site(
    world: &hecs::World,
    body: &str,
    eye_body: DVec3,
    look_body: DVec3,
    ground_r: impl FnOnce() -> Option<f64>,
) -> Option<PlayerFrame> {
    let site = site::nearest_site(world, body, eye_body, site::SITE_JOIN_M)?;
    let up = eye_body.normalize_or_zero();
    let eye_h = crate::surface_walk::EYE_HEIGHT_M;
    let feet_body = match ground_r() {
        Some(g) if eye_body.length() - g <= MAX_EYE_OVER_GROUND_M => up * g,
        _ => eye_body - up * eye_h,
    };
    Some(PlayerFrame {
        eye: site.to_local(eye_body),
        forward: site.dir_to_local(look_body),
        feet: site.to_local(feet_body),
        site: Some(site),
    })
}

/// One piece to draw this frame: its transform in its frame, its drawn scale
/// (a scaffold is shorter), the frame, and how it looks.
struct Piece<'a> {
    tf: Transform,
    scale: Vec3,
    site: Option<&'a PlanetSite>,
    material: usize,
    fade: f32,
    ghost: bool,
    /// A window's pane: drawn in the transparent pass (2026-09-28).
    glass: bool,
}

/// The lists a frame's pieces are drawn in.
#[derive(Default)]
struct Drawn {
    /// The home frame's scene list (shifted to the station with the home).
    home: Vec<RenderObject>,
    /// The celestial list, in render space: EVERY site piece, at every
    /// distance, so what is in front of it stays in front.
    celestial: Vec<RenderObject>,
    /// The scene list after the station shift, in render space: only the
    /// planet ghost.
    ghost: Vec<RenderObject>,
    /// Window glass in the home frame (the scene's transparent list).
    home_glass: Vec<RenderObject>,
    /// Window glass at a site, in render space (the celestial transparent
    /// list, sorted after everything else: `celestial_order::key_for`).
    celestial_glass: Vec<RenderObject>,
}

/// Route the frame's pieces into their lists. Home pieces go to the home
/// list. A site piece is placed with the body's frame at `render_off + rot *
/// p` and goes to the CELESTIAL list only, however close to the eye `cam`
/// it is. The planet ghost goes to the ghost list, drawn in the scene pass:
/// a preview is meant to be seen where it would stand even when something is
/// in front of it, and it must cast no shadow, which the celestial list
/// would give it. A site piece with no body frame this frame (the loop did
/// not place its body) or past [`DRAW_RANGE_M`] is not drawn.
fn route<'a>(pieces: impl Iterator<Item = Piece<'a>>, body_frame: Option<&BodyFrame>, cam: Vec3, mesh: usize) -> Drawn {
    let mut out = Drawn::default();
    for p in pieces {
        let obj = |position, rotation| RenderObject { fade: p.fade, position, rotation, scale: p.scale, mesh, material: p.material };
        let Some(site) = p.site else {
            if p.glass {
                out.home_glass.push(obj(p.tf.position, p.tf.rotation));
            } else {
                out.home.push(obj(p.tf.position, p.tf.rotation));
            }
            continue;
        };
        let Some(bf) = body_frame.filter(|f| f.body == site.body) else { continue };
        let (position, rotation) = site.render_pose(&p.tf, bf.render_off, bf.rot);
        let centre = position + rotation * Vec3::new(0.0, p.scale.y * 0.5, 0.0);
        if (centre - cam).length() - p.scale.length() * 0.5 > DRAW_RANGE_M {
            continue;
        }
        if p.ghost {
            out.ghost.push(obj(position, rotation));
        } else if p.glass {
            out.celestial_glass.push(obj(position, rotation));
        } else {
            out.celestial.push(obj(position, rotation));
        }
    }
    out
}

/// Push this frame's built pieces and the ghost (moved from lib.rs's
/// "Blueprint structures render" block, v0.746). A scaffold shows as an
/// amber box that rises with build progress; a finished piece is a solid box
/// tinted by its blueprint category. Home pieces go to `home` (shifted to
/// the station with the rest of the home frame); site pieces go to
/// `celestial` in render space ([`route`]); a planet ghost goes to
/// `ghost_out`, drawn in the scene pass AFTER the home frame's station
/// shift, because it is already in render space.
pub(crate) fn push_render_objects(
    state: &mut EngineState,
    home: &mut Vec<RenderObject>,
    celestial: &mut Vec<RenderObject>,
    ghost_out: &mut Vec<RenderObject>,
    home_transparent: &mut Vec<RenderObject>,
    celestial_transparent: &mut Vec<RenderObject>,
) {
    if state.structure_mesh.is_none() {
        let mesh = crate::renderer::mesh::Mesh::box_xyz(&state.renderer.device, 1.0, 1.0, 1.0);
        state.structure_mesh = Some(state.renderer.add_mesh(mesh));
    }
    if state.structure_mats.is_none() {
        // World-object placeholder palette (scaffold amber, wood, stone, metal).
        let scaffold = state.renderer.add_material_typed([0.85, 0.62, 0.25, 1.0], 0.0, 0.85, 0.0);
        let wood = state.renderer.add_material_typed([0.48, 0.33, 0.20, 1.0], 0.0, 0.8, 0.0);
        let stone = state.renderer.add_material_typed([0.55, 0.55, 0.58, 1.0], 0.05, 0.9, 0.0);
        let metal = state.renderer.add_material_typed([0.45, 0.30, 0.25, 1.0], 0.6, 0.45, 0.0);
        // A door leaf is darker wood, so a shut door reads against its wall (2026-09-28).
        let door = state.renderer.add_material_typed([0.30, 0.19, 0.11, 1.0], 0.0, 0.75, 0.0); // theme-exempt: world material, not UI
        // Window glass: the home's own window glass (door panels), transparent pass.
        let glass = state.renderer.add_material_full([0.55, 0.78, 0.92, 0.34], 0.0, 0.08, 1.0, 0.10); // theme-exempt: world material, not UI
        state.structure_mats = Some([scaffold, wood, stone, metal, door, glass]);
    }
    let (Some(unit_box), Some([scaffold_mat, wood_mat, stone_mat, metal_mat, door_mat, glass_mat])) = (state.structure_mesh, state.structure_mats) else {
        return;
    };
    let body_frame = state.planet_body_frame.take();
    let registry = state.data_store.get::<BlueprintRegistry>("blueprint_registry");
    let mat_for = |bp_id: &str| -> usize {
        match registry.and_then(|r| r.get(bp_id)).map(|bp| bp.category.as_str()).unwrap_or("") {
            "foundation" => stone_mat,
            "wall" | "roof" | "door" | "window" | "furniture" => wood_mat,
            _ => metal_mat,
        }
    };
    let world = &state.game_world.world;
    let mut scaffolds = world.query::<(&Construction, &Transform, Option<&PlanetSite>)>();
    let mut finished = world.query::<(&Structure, &Transform, Option<&PlanetSite>, Option<&crate::systems::construction::DoorOpen>)>();
    // A scaffold rises from 15% to full height with progress.
    let rising = scaffolds.iter().map(|(_e, (c, tf, site))| {
        let frac = (c.progress / c.build_time.max(0.01)).clamp(0.0, 1.0) * 0.85 + 0.15;
        Piece { tf: tf.clone(), scale: Vec3::new(tf.scale.x, tf.scale.y * frac, tf.scale.z), site, material: scaffold_mat, fade: 0.0, ghost: false, glass: false }
    });
    // A wall with a door in it is drawn as its parts (`doorway::parts`), the
    // same boxes that block the walk.
    let standing = finished.iter().flat_map(|(_e, (s, tf, site, open))| {
        let material = mat_for(&s.blueprint_id);
        use crate::systems::construction::doorway::{piece_parts, Part};
        let parts: Vec<(Transform, usize, bool)> = match registry.and_then(|r| r.get(&s.blueprint_id)).and_then(|bp| piece_parts(bp, tf, open.is_some())) {
            Some(parts) => parts
                .into_iter()
                .map(|(p, kind)| match kind {
                    Part::Leaf => (p, door_mat, false),
                    Part::Glass => (p, glass_mat, true),
                    Part::Wall => (p, material, false),
                })
                .collect(),
            None => vec![(tf.clone(), material, false)],
        };
        parts.into_iter().map(move |(p, material, glass)| Piece { scale: p.scale, tf: p, site, material, fade: 0.0, ghost: false, glass })
    });
    // The piece in hand (engine/build_place.rs): a half-dithered scaffold where it would go.
    let in_hand = state.gui_state.build_placing.as_ref().and_then(|p| {
        let g = p.ghost.as_ref()?;
        Some(Piece { tf: g.clone(), scale: g.scale, site: p.site.as_ref(), material: scaffold_mat, fade: 0.5, ghost: true, glass: false })
    });
    let drawn = route(rising.chain(standing).chain(in_hand), body_frame.as_ref(), state.camera.position, unit_box);
    home.extend(drawn.home);
    celestial.extend(drawn.celestial);
    ghost_out.extend(drawn.ghost);
    home_transparent.extend(drawn.home_glass);
    celestial_transparent.extend(drawn.celestial_glass);
}

/// Dev verb for the probe rig (showcase_request `{"build":"...",
/// "build_at":"lat,lon"}`): stand built pieces up FINISHED around a ground
/// point, the way a player's builds would end up, so a capture can see them.
/// `spec` is `id@dx,dz,turns` items joined by `;` (offsets in metres in the
/// site's frame, east and south; turns 0 to 3). The point is `build_at`
/// (latitude, longitude on the body the player is locked to) or, without
/// it, the ground under the crosshair. Placement is the real one
/// (`placement_pose`, so a roof lands on the walls built just before it,
/// in the build site that point belongs to); what is skipped is the
/// materials, the build time and the stand-on-the-ground rule, which is
/// what a dev verb is for. A piece whose box already stands is skipped, so
/// running the same request twice builds nothing new. Aboard it builds in
/// the home frame around the aimed deck point. Returns a line for the log.
///
/// NO DEV GATE, on purpose: like every other showcase verb, it is reached
/// only by dropping `debug/showcase_request.json` next to the running exe,
/// which is the dev channel itself; none of the showcase verbs (time,
/// weather, the camera, the sea) checks a dev flag, and this one would be
/// the odd one out if it did (the planet-build review, 2026-09-27).
pub(crate) fn dev_build(state: &mut EngineState, spec: &str, at: Option<&str>) -> String {
    let items: Vec<(String, f32, f32, u8)> = spec
        .split(';')
        .filter_map(|item| {
            let item = item.trim();
            let (id, rest) = item.split_once('@').unwrap_or((item, ""));
            let n: Vec<f32> = rest.split(',').filter_map(|v| v.trim().parse().ok()).collect();
            (!id.is_empty()).then(|| (id.to_string(), n.first().copied().unwrap_or(0.0), n.get(1).copied().unwrap_or(0.0), n.get(2).map_or(0, |t| (*t as i32).rem_euclid(4) as u8)))
        })
        .collect();
    // The ground point and the frame, then each piece's aimed point on the
    // ground (all read while the terrain is borrowed).
    let (site, targets): (Option<PlanetSite>, Vec<Vec3>) = if state.aboard_station {
        let base = placement::aim_point(state.camera.position, state.camera.forward(), state.controller.ground_floor());
        (None, items.iter().map(|(_, dx, dz, _)| base + Vec3::new(*dx, 0.0, *dz)).collect())
    } else {
        let Some((site, base, ground)) = dev_ground_point(state, at) else {
            return "not aboard, and no ground point on a body: nothing built".into();
        };
        let targets = items
            .iter()
            .map(|(_, dx, dz, _)| {
                let (x, z) = (base.x + dx, base.z + dz);
                Vec3::new(x, ground.height(&site, x, z), z)
            })
            .collect();
        (Some(site), targets)
    };
    let Some(reg) = state.data_store.get::<BlueprintRegistry>("blueprint_registry") else { return "no blueprint registry".into() };
    let world = &mut state.game_world.world;
    let mut built = 0;
    for ((id, _, _, turns), at) in items.iter().zip(targets) {
        let Some(bp) = reg.get(id) else { continue };
        let pose = placement::placement_pose(bp, at, *turns, world, reg, site.as_ref());
        if placement::occupied(world, &pose, site.as_ref()) {
            continue;
        }
        let piece = world.spawn((
            pose,
            Structure { blueprint_id: bp.id.clone(), health: bp.health, max_health: bp.health, provides: bp.provides.clone(), uid: 0 },
        ));
        if let Some(s) = site.clone() {
            let _ = world.insert_one(piece, s);
        }
        built += 1;
    }
    match &site {
        Some(s) => {
            let (lat, lon) = crate::terrain::osm_region::dir_to_latlon_f64(s.origin.normalize());
            format!("{built} of {} pieces stood up at the {} site at {lat:.6}, {lon:.6}", items.len(), s.body)
        }
        None => format!("{built} of {} pieces stood up in the home", items.len()),
    }
}

/// The dev verbs' ground point on the locked body: `at` ("lat,lon") or the
/// ground under the crosshair (marched up to 3 km), with the build site it
/// belongs to (`site_for`: the site of the nearest piece, or a new one) and
/// the point in that site's frame. None off a body or when the crosshair
/// meets no ground.
fn dev_ground_point<'a>(state: &'a EngineState, at: Option<&str>) -> Option<(PlanetSite, Vec3, Ground<'a>)> {
    let v = PlayerView::of(state);
    let body = v.body.clone()?;
    let ground = Ground::for_body(state, &body)?;
    let base_dir = match at.map(|a| a.split(',').filter_map(|v| v.trim().parse::<f64>().ok()).collect::<Vec<_>>()) {
        Some(ll) if ll.len() == 2 => crate::terrain::osm_region::latlon_to_dir_f64(ll[0], ll[1]),
        _ => {
            let up = v.anchor.normalize_or_zero();
            let probe = PlanetSite { body: body.clone(), origin: up * ground.radius(up) };
            let look = probe.dir_to_local(v.look_body());
            let hit = placement::ray_ground_hit(probe.to_local(v.anchor), look, &|x, z| ground.height(&probe, x, z), 3_000.0)?;
            probe.to_body(hit).normalize()
        }
    };
    let base_ground = base_dir * ground.radius(base_dir);
    let site = site::site_for(&state.game_world.world, &body, base_ground);
    let base = site.to_local(base_ground);
    Some((site, base, ground))
}

/// Dev verb for the probe rig (showcase_request `{"stand":"dx,h,dz,heading,
/// pitch","stand_at":"lat,lon"}`, 2026-09-27): put the player's eye `h`
/// metres over the ground at `dx`, `dz` (east and south, metres) from the
/// ground point (`stand_at`, or the crosshair's), in the build site that
/// point belongs to, looking toward `heading` (degrees from north, east
/// positive) and `pitch` (degrees, up positive). This is how a capture
/// stands INSIDE a hut the `build` verb stood up (the planet-built-inside
/// vantage), which the camera park cannot do: it parks tens of metres over
/// the ground. The eye moves through the frame lock's anchor, exactly where
/// walking would have put it. Send it after the park (probe-sweep.js
/// `final_showcase`). No dev gate, like every showcase verb (see
/// [`dev_build`]). Returns a line for the log.
pub(crate) fn dev_stand(state: &mut EngineState, spec: &str, at: Option<&str>) -> String {
    let n: Vec<f32> = spec.split(',').filter_map(|v| v.trim().parse().ok()).collect();
    let [dx, h, dz, heading, pitch] = [0, 1, 2, 3, 4].map(|i| n.get(i).copied().unwrap_or(if i == 1 { 1.7 } else { 0.0 }));
    if state.aboard_station {
        return "aboard: stand is for a planet's ground".into();
    }
    let Some((site, base, ground)) = dev_ground_point(state, at) else {
        return "no ground point on a body: nobody moved".into();
    };
    let (x, z) = (base.x + dx, base.z + dz);
    let eye_local = Vec3::new(x, ground.height(&site, x, z) + h, z);
    let (hr, pr) = (heading.to_radians(), pitch.to_radians());
    // Site axes: +X east, -Z north, +Y up.
    let look_local = Vec3::new(hr.sin() * pr.cos(), pr.sin(), -hr.cos() * pr.cos());
    let v = PlayerView::of(state);
    let look_render = v.rot * (site.basis() * look_local.as_dvec3());
    let eye = site.to_body(eye_local);
    drop(ground);
    // Move the eye the way walking would: the anchor, and the ship frame by
    // the same step in render space, so the camera's world position agrees
    // with the anchor whichever way the frame lock reads it next frame (it
    // re-captures the anchor from the camera on the frame surface mode
    // engages, and keeps the stored one after).
    let step = DQuat::from_rotation_y(state.current_spin) * (eye - state.frame_lock_anchor);
    state.ship_world_pos += step;
    state.frame_lock_anchor = eye;
    // The look in the camera's CURRENT basis, the surface one when engaged
    // (clearing it would make the frame lock re-capture the anchor).
    let look = look_render.as_vec3();
    let (cam_yaw, cam_pitch) = if state.camera.surface_mode {
        crate::surface_walk::surface_look_angles(state.camera.up, look)
    } else {
        crate::surface_walk::world_look_angles(look)
    };
    state.camera.yaw = cam_yaw;
    state.camera.pitch = cam_pitch;
    format!("eye at {eye_local:?} in the {} site (heading {heading} deg, pitch {pitch} deg)", site.body)
}

/// Built pieces are solid on a planet's ground too (2026-09-28). On a planet
/// the player moves by the frame lock's anchor (the eye's point in the body's
/// unrotated frame), not by the camera, so the walk's step, from the anchor
/// before it to the anchor after, is resolved in the frame of the build site
/// the player stands in, against that site's finished pieces, with the same
/// resolver and segments the home uses aboard
/// (`build_place::built_piece_segments`). Site-local metres are small numbers
/// (a site reaches 1 km), so the round trip through f32 costs well under a
/// millimetre, and it is only taken when a piece actually moved the step.
/// Outside any site, or with nothing in the way, the anchor is returned as is.
pub(crate) fn collide_on_site(
    world: &hecs::World,
    registry: Option<&BlueprintRegistry>,
    body: &str,
    before: DVec3,
    after: DVec3,
    eye_height: f32,
) -> DVec3 {
    let Some(site) = site::nearest_site(world, body, after, site::SITE_JOIN_M) else {
        return after;
    };
    let (from, to) = (site.to_local(before), site.to_local(after));
    let segments = crate::engine::build_place::built_piece_segments(world, registry, Some(&site), to, eye_height);
    if segments.is_empty() {
        return after;
    }
    let resolved = crate::ship::wall_collision::resolve(from, to, crate::ship::wall_collision::PLAYER_RADIUS, &[], &segments);
    if (resolved - to).length_squared() < 1.0e-10 {
        return after;
    }
    site.to_body(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::construction::uses;

    const EARTH_R: f64 = 6_371_000.0;

    fn shipped() -> BlueprintRegistry {
        BlueprintRegistry::from_ron(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/blueprints/basic.ron"))).unwrap()
    }

    /// A site on flat ground (a sphere of Earth's radius) at a latitude and
    /// longitude.
    fn site_at(lat: f64, lon: f64) -> PlanetSite {
        PlanetSite { body: "earth".into(), origin: crate::terrain::osm_region::latlon_to_dir_f64(lat, lon) * EARTH_R }
    }

    /// Build `pieces` in `site` the way the ghost places them.
    fn build_in(world: &mut hecs::World, reg: &BlueprintRegistry, site: &PlanetSite, pieces: &[(&str, f32, f32, u8)]) {
        for (id, x, z, t) in pieces {
            let bp = reg.get(id).unwrap();
            let pose = placement::placement_pose(bp, Vec3::new(*x, 0.0, *z), *t, world, reg, Some(site));
            world.spawn((pose, Structure { blueprint_id: (*id).into(), health: 1.0, max_health: 1.0, provides: bp.provides.clone(), uid: 0 }, site.clone()));
        }
    }

    /// THE BUG-102 PROPERTY. On a planet the camera stays put while the ship
    /// frame moves under it: walking moves `ship_world_pos`, not the camera,
    /// and the planet spins. Simulated here frame by frame exactly as the
    /// frame lock does it (`dev_travel::frame_lock_ship_pos` with the camera
    /// parked at a fixed local offset), a wall built in a site near Silverdale
    /// is drawn each frame at a render position whose WORLD position,
    /// taken back into the planet's frame, is the same planet-fixed point to
    /// under a millimetre, standing upright on the local up, and at the true
    /// distance from the walking player. Red check, run: dropping the spin
    /// from the draw (`render_off + self.to_body(..)` in `render_pose`, the
    /// planet's orientation ignored) moves the wall with the planet's turn
    /// and the first assertion fails by kilometres.
    #[test]
    fn a_planet_piece_keeps_its_planet_fixed_place_while_the_ship_frame_moves() {
        let site = site_at(47.645, -122.6925);
        let wall = Transform {
            position: Vec3::new(2.0, 0.31, -2.0),
            rotation: placement::quarter_turn(1),
            scale: Vec3::new(4.0, 3.0, 0.2),
        };
        let fixed = site.to_body(wall.position);
        let cam_local = DVec3::new(0.0, 50.0, 0.0); // the camera never moves
        let body_center = DVec3::ZERO; // Earth is the frame origin
        for k in 0..6 {
            let spin = 0.4 + 0.35 * k as f64;
            // The player walks 3 m east and 2 m north each frame.
            let eye = site.to_body(Vec3::new(3.0 * k as f32, 1.7, -2.0 * k as f32));
            let ship = crate::dev_travel::frame_lock_ship_pos(body_center, spin, eye, cam_local);
            let rot = body_render_rot(false, DQuat::IDENTITY, spin);
            let (p, r) = site.render_pose(&wall, body_center - ship, rot);
            let world = ship + p.as_dvec3();
            let back = rot.inverse() * (world - body_center);
            assert!((back - fixed).length() < 1e-3, "frame {k}: drawn {:.4} m from its planet-fixed place", (back - fixed).length());
            let up_render = (r * Vec3::Y).as_dvec3();
            let radial = (rot * site.origin).normalize();
            assert!((up_render - radial).length() < 1e-5, "frame {k}: upright on the local up");
            let seen = (p.as_dvec3() - cam_local).length();
            assert!((seen - (fixed - eye).length()).abs() < 1e-3, "frame {k}: at the true distance from the player");
        }
    }

    /// THE SHELTER ON A PLANET uses the planet frame. Three walls and a roof
    /// built in a site: a player standing inside, given by their eye in the
    /// planet's frame (the frame lock's anchor) and the ground under them,
    /// is sheltered; walked 40 m away, they are not; and the room shelters
    /// nobody in the home frame. Red check, run: making `player_at_site`
    /// ignore the eye and put the feet at the site origin (a fixed spot,
    /// what testing the parked camera's position amounted to) keeps the
    /// walked-away player sheltered and the second assertion fails.
    #[test]
    fn the_shelter_on_a_planet_follows_the_player_in_the_planet_frame() {
        let reg = shipped();
        let site = site_at(23.0, 13.0);
        let mut world = hecs::World::new();
        build_in(&mut world, &reg, &site, &[("wood_wall", 0.0, -2.0, 0), ("wood_wall", -2.0, 0.0, 1), ("wood_wall", 2.0, 0.0, 1), ("roof", 0.0, 0.0, 0)]);
        let stand = |x: f32, z: f32| {
            let ground = site.to_body(Vec3::new(x, 0.0, z));
            let up = ground.normalize();
            let eye = up * (ground.length() + 1.75);
            let f = player_at_site(&world, "earth", eye, DVec3::X, || Some(ground.length())).expect("a site within reach");
            uses::shelter_at(&world, f.feet, f.site.as_ref())
        };
        assert!(stand(0.0, 0.5).sheltered(), "inside: {:?}", stand(0.0, 0.5));
        assert_eq!(stand(40.0, 0.0), uses::ShelterCheck::default(), "walked away");
        assert_eq!(uses::shelter_at(&world, Vec3::new(0.0, 0.0, 0.5), None), uses::ShelterCheck::default(), "no roof in the home frame");
        // Beyond the site's reach there is no build frame at all.
        let far = site.to_body(Vec3::new(5_000.0, 1.7, 0.0));
        assert!(player_at_site(&world, "earth", far, DVec3::X, || None).is_none());
    }

    /// A view of a player on a planet whose eye (the anchor) stands at `local`
    /// in `site`, looking along the site's -Z (north), with the parked camera
    /// at `camera_pos` in the home frame.
    fn on_planet(site: &PlanetSite, local: Vec3, camera_pos: Vec3, spin: f64) -> PlayerView {
        let rot = body_render_rot(false, DQuat::IDENTITY, spin);
        PlayerView {
            aboard: false,
            camera_pos,
            forward: (rot * (site.basis() * DVec3::NEG_Z)).as_vec3(),
            eye_height: 1.7,
            body: Some(site.body.clone()),
            anchor: site.to_body(local),
            rot,
        }
    }

    /// THE PLAYER'S FRAME IS WIRED TO THE ANCHOR, NOT THE CAMERA (the review's
    /// missing test). A hut stands in the home frame around (0, 0, 0) and
    /// another in a site on Earth. On the planet the parked camera's LOCAL
    /// position sits inside the home hut by coincidence (it is where the
    /// park leaves it) while the anchor is 60 m from the Earth hut: the
    /// player is in the Earth site, not sheltered, and the look is the
    /// site's north. With the anchor moved inside the Earth hut they are
    /// sheltered, whatever the camera says. Aboard, the camera is the place
    /// and the home hut shelters. Red check, run: making `frame_of` place a
    /// planet player at `camera_pos` in the site (what reading the raw
    /// camera amounted to) shelters the player 60 m out, and the first
    /// shelter assertion fails.
    #[test]
    fn the_players_frame_follows_the_anchor_not_the_parked_camera() {
        let reg = shipped();
        let site = site_at(23.5, 13.0);
        let hut = [("wood_wall", 0.0, -2.0, 0), ("wood_wall", -2.0, 0.0, 1), ("wood_wall", 2.0, 0.0, 1), ("roof", 0.0, 0.0, 0)];
        let mut world = hecs::World::new();
        build_in(&mut world, &reg, &site, &hut);
        for (id, x, z, t) in hut {
            let bp = reg.get(id).unwrap();
            let pose = placement::placement_pose(bp, Vec3::new(x, 0.0, z), t, &world, &reg, None);
            world.spawn((pose, Structure { blueprint_id: id.into(), health: 1.0, max_health: 1.0, provides: bp.provides.clone(), uid: 0 }));
        }
        let parked = Vec3::new(0.0, 1.7, 0.5); // inside the HOME hut
        let ground = |v: &PlayerView| {
            let g = site.to_body(site.to_local(v.anchor) * Vec3::new(1.0, 0.0, 1.0)).length();
            move || Some(g)
        };
        let out = on_planet(&site, Vec3::new(60.0, 1.7, 0.0), parked, 0.8);
        let f = frame_of(&world, &out, ground(&out)).expect("within the site's reach");
        assert_eq!(f.site.as_ref(), Some(&site), "in the Earth site");
        assert!(!uses::shelter_at(&world, f.feet, f.site.as_ref()).sheltered(), "60 m from the hut: out in the weather");
        assert!((f.forward - Vec3::NEG_Z).length() < 1e-5, "the look is the site's north: {}", f.forward);
        assert!((f.eye - Vec3::new(60.0, 1.7, 0.0)).length() < 1e-3, "the eye is the anchor's: {}", f.eye);
        let inside = on_planet(&site, Vec3::new(0.0, 1.7, 0.5), Vec3::new(500.0, 40.0, 0.0), 2.1);
        let f = frame_of(&world, &inside, ground(&inside)).unwrap();
        assert!(uses::shelter_at(&world, f.feet, f.site.as_ref()).sheltered(), "inside the Earth hut, wherever the camera is");
        let aboard = PlayerView { aboard: true, ..on_planet(&site, Vec3::new(60.0, 1.7, 0.0), parked, 0.8) };
        let f = frame_of(&world, &aboard, || None).unwrap();
        assert_eq!(f.site, None, "aboard: the home frame");
        assert!(uses::shelter_at(&world, f.feet, None).sheltered(), "aboard, the home hut shelters the camera's place");
    }

    /// NOTE_BODY RECORDS THE TERRAIN'S OWN ROTATION, never a recomputation
    /// (the review's missing test). The frame loop has exactly one `rot_d`
    /// binding, made by [`body_render_rot`], with which it places the
    /// terrain patches, the near trees and the OSM buildings, and it hands
    /// that same binding to `note_body`; note_body stores what it is given.
    /// A piece is then drawn with the numbers its ground was: the drawn
    /// piece's base and the terrain's own placement of the same planet point
    /// (`render_off + rot_d * p`, region_meshes' form) agree to the bit.
    /// Red check, run: note_body computing its own rotation from the spin
    /// again (the old `body_rot(state)`) fails the source assertions.
    #[test]
    fn note_body_is_handed_the_terrains_own_rot_d() {
        let src = crate::terrain::planet_chunks::frame_loop_source();
        let bindings = src.matches("let rot_d =").count();
        assert_eq!(bindings, 1, "one rot_d binding in the frame loop, got {bindings}");
        let at = src.find("let rot_d =").unwrap();
        assert!(src[at..].starts_with("let rot_d = crate::engine::planet_build::body_render_rot("), "rot_d comes from body_render_rot");
        let call = src.find("planet_build::note_body(state, &b.id, render_off, rot_d)").expect("note_body is handed rot_d");
        assert!(call > at, "after the binding it is handed");
        let this = include_str!("planet_build.rs");
        let body = &this[this.find("pub(crate) fn note_body").unwrap()..this.find("/// THE rotation taking").unwrap()];
        assert!(!body.contains("spin") && !body.contains("body_render_rot"), "note_body recomputes nothing: {body}");
        // What it stores draws a piece exactly where the terrain puts the point.
        let site = site_at(47.645, -122.6925);
        let rot_d = body_render_rot(true, DQuat::from_rotation_z(0.3), 1.234);
        let render_off = DVec3::new(-120.0, -6_371_010.0, 35.0);
        let base = Transform { position: Vec3::new(3.0, 0.2, -1.0), ..Transform::default() };
        let (p, _) = site.render_pose(&base, render_off, rot_d);
        let terrain = (render_off + rot_d * site.to_body(base.position)).as_vec3();
        assert_eq!(p, terrain);
    }

    /// THE DRAW'S REAL FAILURE MODE (the review's missing test). An open hut
    /// (three walls and a roof) with a chest 0.6 m in front of its north
    /// wall, and the eye standing in its open south side looking north: the
    /// geometry the old near copy got wrong, because the north wall (1.0 m
    /// by the copy's measure, centre distance less half its diagonal) was
    /// copied into the scene pass, which clears depth, while the chest (2.1
    /// m) was not, so the wall was painted over the chest. Now every site
    /// piece is routed to the CELESTIAL list only, and the ghost alone to
    /// the ghost list. In that one depth-tested pass, along a look ray
    /// through the chest into the wall, the chest is met first and writes
    /// the greater reverse-Z depth, both inside the near plane (depth at
    /// most 1), so the chest stays in front of its wall. Red check, run:
    /// putting back the near copy (`gap < 2.0` also pushed to the scene
    /// list) sends the walls there and the ghost-list assertion fails.
    #[test]
    fn a_chest_in_front_of_its_wall_stays_in_front() {
        let reg = shipped();
        let site = site_at(23.5, 13.02);
        let mut world = hecs::World::new();
        build_in(
            &mut world,
            &reg,
            &site,
            &[("wood_wall", 0.0, -2.0, 0), ("wood_wall", -2.0, 0.0, 1), ("wood_wall", 2.0, 0.0, 1), ("roof", 0.0, 0.0, 0), ("storage_chest", 0.0, -1.0, 0)],
        );
        let spin = 0.9;
        let rot = body_render_rot(false, DQuat::IDENTITY, spin);
        let eye = site.to_body(Vec3::new(0.0, 1.7, 1.5));
        let render_off = -(rot * eye); // the eye at the render origin
        let bf = BodyFrame { body: "earth".into(), render_off, rot };
        let ghost_tf = Transform { position: Vec3::new(1.0, 0.0, 1.0), ..Transform::default() };
        let mut q = world.query::<(&Structure, &Transform, &PlanetSite)>();
        let pieces: Vec<(String, Transform)> = q.iter().map(|(_e, (s, tf, _))| (s.blueprint_id.clone(), tf.clone())).collect();
        let items = pieces
            .iter()
            .map(|(_, tf)| Piece { tf: tf.clone(), scale: tf.scale, site: Some(&site), material: 1, fade: 0.0, ghost: false, glass: false })
            .chain(std::iter::once(Piece { tf: ghost_tf.clone(), scale: Vec3::ONE, site: Some(&site), material: 0, fade: 0.5, ghost: true, glass: false }));
        let drawn = route(items, Some(&bf), Vec3::ZERO, 0);
        assert_eq!(drawn.celestial.len(), pieces.len(), "every site piece is in the celestial pass");
        assert!(drawn.home.is_empty());
        assert_eq!(drawn.ghost.len(), 1, "only the ghost goes to the scene pass");
        // A look ray from the eye through the chest's front face, near its
        // top, on into the north wall 0.2 m over the floor.
        let from = Vec3::new(0.0, 1.7, 1.5);
        let dir = Vec3::new(0.0, -1.0, -2.25).normalize();
        let hit = |id: &str| pieces.iter().filter(|(b, _)| b == id).filter_map(|(_, tf)| uses::ray_hits_box(from, dir, tf)).fold(f32::MAX, f32::min);
        let (chest, wall) = (hit("storage_chest"), hit("wood_wall"));
        assert!(chest < wall && wall < 10.0, "the chest ({chest} m) is in front of the wall ({wall} m)");
        let depth = |d: f32| crate::renderer::camera::celestial_depth_at(d);
        assert!(depth(chest) > depth(wall) && depth(chest) <= 1.0, "one pass, nearer wins: {} vs {}", depth(chest), depth(wall));
    }
}

#[cfg(test)]
mod site_collision_tests {
    use super::*;
    use crate::systems::construction::{placement, BlueprintRegistry, Structure};

    /// A BUILT WALL IS SOLID ON THE GROUND (2026-09-28). A site on Earth's
    /// equator with one wall standing north-south: a step east into it from
    /// the west is stopped short of the wall, in the site's own metres; a step
    /// that misses the wall, or a step taken far from any site, comes back
    /// exactly as it went in. Red check, run: returning `after` at the top of
    /// `collide_on_site` walks through the wall and fails the first assertion.
    #[test]
    fn a_wall_on_a_planet_site_stops_the_walk() {
        let reg = BlueprintRegistry::from_ron(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/blueprints/basic.ron"))).unwrap();
        let site = PlanetSite { body: "earth".into(), origin: DVec3::new(6.371e6, 0.0, 0.0) };
        let mut world = hecs::World::new();
        let bp = reg.get("wood_wall").unwrap();
        let tf = placement::placement_pose(bp, Vec3::ZERO, 1, &world, &reg, Some(&site));
        world.spawn((
            tf,
            Structure { blueprint_id: "wood_wall".into(), health: bp.health, max_health: bp.health, provides: bp.provides.clone(), uid: 0 },
            site.clone(),
        ));
        let eye_h = 1.7;
        let body = |x: f32, z: f32| site.to_body(Vec3::new(x, eye_h, z));

        let out = collide_on_site(&world, None, "earth", body(-2.0, 0.0), body(2.0, 0.0), eye_h);
        let local = site.to_local(out);
        assert!(local.x < -0.25, "stopped west of the wall, at {local}");
        assert!(local.z.abs() < 1e-3, "no sideways slide on a square hit: {local}");

        // Past the wall's end: nothing in the way, the anchor is untouched.
        let clear = body(2.0, 5.0);
        assert_eq!(collide_on_site(&world, None, "earth", body(-2.0, 5.0), clear, eye_h), clear);
        // Far from any site (3 km away): untouched.
        let far = body(3_000.0, 0.0);
        assert_eq!(collide_on_site(&world, None, "earth", body(2_990.0, 0.0), far, eye_h), far);
        // Another body's site is not this one.
        assert_eq!(collide_on_site(&world, None, "moon", body(-2.0, 0.0), body(2.0, 0.0), eye_h), body(2.0, 0.0));
    }
}
