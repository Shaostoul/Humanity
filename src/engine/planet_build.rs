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
//! - WHICH FRAME the player is in this frame ([`build_frame`] for placing,
//!   [`player_frame`] for the shelter and the bed and chest look ray): the
//!   home frame aboard, or on a planet's ground the build site they stand in
//!   (`construction::site`), with their eye and look direction converted
//!   into that site's flat Y-up frame from the frame lock's anchor (the eye
//!   in the body's frame, f64) and the camera's look.
//! - THE GROUND in that frame ([`Ground`]): the drawn terrain the player
//!   stands on (`surface_walk::walk_ground_radius`, the same floor the walk
//!   clamp, the grass and the near trees use), so a piece is set on the
//!   ground you can see and the aim meets the ground under the crosshair.
//! - THE DRAW ([`push_render_objects`], moved here from lib.rs): home pieces
//!   into the home-frame list as before; site pieces at `render_off + rot *
//!   p` in the CELESTIAL list with the terrain, the near trees and the OSM
//!   buildings, so the ground hides them behind a hill and they cast the
//!   sun's shadow; and a second copy of any piece within 2 m of the
//!   eye in the scene list, whose near plane (unlike the celestial pass's
//!   1 m) does not cut into a wall you stand beside.
//!
//! `render_off` and `rot` are the body's centre and orientation in render
//! space THIS frame, recorded by [`note_body`] from inside the celestial loop,
//! so a piece is placed with exactly the numbers its ground was.

use crate::ecs::components::Transform;
use crate::engine::state::EngineState;
use crate::renderer::camera::CameraMode;
use crate::renderer::RenderObject;
use crate::systems::construction::site::{self, PlanetSite};
use crate::systems::construction::{placement, BlueprintRegistry, Construction, Structure};
use crate::terrain::planet::PlanetDef;
use glam::{DQuat, DVec3, Quat, Vec3};

/// How high the eye may be over the ground and still place a piece, metres:
/// standing (1.7 m plus the walk clamp's clearance, up to 2.5 m on the
/// elevation field) with a little air for a hop. Higher is flying.
const MAX_EYE_OVER_GROUND_M: f64 = 4.5;
/// A site piece whose box comes within this of the eye is also drawn in the
/// scene pass: the celestial pass's near plane is 1 m, and its frustum
/// corners reach about 1.8 m at a wide window, so a wall closer than that
/// would be cut open and show the outside through it. Kept this tight on
/// purpose: the scene pass lights everything as if indoors (its uniforms
/// carry no local up, so there is no sky ambient, `sky_ambient` in the
/// shader), so the copy of a face turned from the sun reads black where the
/// celestial draw reads dark (measured at the planet-built-shelter hut,
/// 2026-09-27). Inside a roofed hut that is closer to the truth than the
/// sky light the celestial pass would give it; outside, it is a small step
/// in shade within arm's reach. Giving planet pieces one lighting in both
/// passes is the follow-up (docs/FEATURES.md, "Building on a planet").
const NEAR_COPY_M: f32 = 2.0;
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

/// Called from inside the celestial loop for every body it places. Keeps the
/// placement of the one the player is locked to, for [`push_render_objects`]
/// later in the same frame (which takes it, so it is never a frame stale).
pub(crate) fn note_body(state: &mut EngineState, body: &str, render_off: DVec3) {
    if state.frame_lock_body.as_deref() == Some(body) {
        state.planet_body_frame = Some(BodyFrame { body: body.to_string(), render_off, rot: body_rot(state) });
    }
}

/// The rotation taking a direction in the locked body's unrotated frame to
/// render space: the planet's spin, composed with the hull frame when riding
/// the station (the same product the celestial loop builds as `rot_d`).
fn body_rot(state: &EngineState) -> DQuat {
    crate::station::hull_frame_rot(state.station_ride, state.station_world_rot) * DQuat::from_rotation_y(state.current_spin)
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
    let (at, site) = if state.aboard_station {
        let floor = state.controller.ground_floor();
        (placement::aim_point(state.camera.position, state.camera.forward(), floor), None)
    } else {
        let body = state.frame_lock_body.as_deref().ok_or(CannotBuild::OpenSpace)?;
        if !state.surface_walk_band {
            return Err(CannotBuild::NotOnGround);
        }
        let ground = Ground::for_body(state, body).ok_or(CannotBuild::NotOnGround)?;
        let eye_body = state.frame_lock_anchor;
        let up = eye_body.normalize_or_zero();
        let under = ground.radius(up);
        if eye_body.length() - under > MAX_EYE_OVER_GROUND_M {
            return Err(CannotBuild::NotOnGround);
        }
        if ground.under_water(up, under) {
            return Err(CannotBuild::OnWater);
        }
        let site = site::site_for(world, body, up * under);
        let eye = site.to_local(eye_body);
        let look = site.dir_to_local(body_rot(state).inverse() * state.camera.forward().as_dvec3());
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
/// on a planet, the nearest build site within reach (`site::SITE_JOIN_M`);
/// None anywhere else (open space, or a planet with nothing built near).
pub(crate) fn player_frame(state: &EngineState) -> Option<PlayerFrame> {
    if state.aboard_station {
        let eye = state.camera.position;
        let feet = eye - Vec3::Y * state.controller.eye_height();
        return Some(PlayerFrame { site: None, eye, forward: state.camera.forward(), feet });
    }
    let body = state.frame_lock_body.as_deref()?;
    let eye_body = state.frame_lock_anchor;
    let look_body = body_rot(state).inverse() * state.camera.forward().as_dvec3();
    let ground_r = || Ground::for_body(state, body).map(|g| g.radius(eye_body.normalize_or_zero()));
    player_at_site(&state.game_world.world, body, eye_body, look_body, ground_r)
}

/// The player in the nearest build site on `body`, from their eye and look
/// direction in the body's frame and the ground radius under them
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

/// Push this frame's built pieces and the ghost (moved from lib.rs's
/// "Blueprint structures render" block, v0.746). A scaffold shows as an
/// amber box that rises with build progress; a finished piece is a solid box
/// tinted by its blueprint category. Home pieces go to `home` (shifted to
/// the station with the rest of the home frame); site pieces go to
/// `celestial` in render space, plus a copy in `near` when within
/// [`NEAR_COPY_M`] of the eye; a planet ghost goes to `near` only (it never
/// casts a shadow). `near` is drawn in the scene pass AFTER the home frame's
/// station shift, because it is already in render space.
pub(crate) fn push_render_objects(
    state: &mut EngineState,
    home: &mut Vec<RenderObject>,
    celestial: &mut Vec<RenderObject>,
    near: &mut Vec<RenderObject>,
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
        state.structure_mats = Some([scaffold, wood, stone, metal]);
    }
    let (Some(unit_box), Some([scaffold_mat, wood_mat, stone_mat, metal_mat])) = (state.structure_mesh, state.structure_mats) else {
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
    let cam = state.camera.position;
    // One piece, in its frame, into the right list(s).
    let mut push = |tf: &Transform, scale: Vec3, site: Option<&PlanetSite>, material: usize, fade: f32, ghost: bool| {
        let Some(site) = site else {
            home.push(RenderObject { fade, position: tf.position, rotation: tf.rotation, scale, mesh: unit_box, material });
            return;
        };
        let Some(bf) = body_frame.as_ref().filter(|f| f.body == site.body) else { return };
        let (position, rotation) = site.render_pose(tf, bf.render_off, bf.rot);
        let centre = position + rotation * Vec3::new(0.0, scale.y * 0.5, 0.0);
        let gap = (centre - cam).length() - scale.length() * 0.5;
        if gap > DRAW_RANGE_M {
            return;
        }
        let obj = RenderObject { fade, position, rotation, scale, mesh: unit_box, material };
        if gap < NEAR_COPY_M || ghost {
            near.push(obj.clone());
        }
        if !ghost {
            celestial.push(obj);
        }
    };
    let world = &state.game_world.world;
    for (_e, (c, tf, site)) in world.query::<(&Construction, &Transform, Option<&PlanetSite>)>().iter() {
        // A scaffold rises from 15% to full height with progress.
        let frac = (c.progress / c.build_time.max(0.01)).clamp(0.0, 1.0) * 0.85 + 0.15;
        push(tf, Vec3::new(tf.scale.x, tf.scale.y * frac, tf.scale.z), site, scaffold_mat, 0.0, false);
    }
    for (_e, (s, tf, site)) in world.query::<(&Structure, &Transform, Option<&PlanetSite>)>().iter() {
        push(tf, tf.scale, site, mat_for(&s.blueprint_id), 0.0, false);
    }
    // The piece in hand (engine/build_place.rs): a half-dithered scaffold where it would go.
    if let Some(p) = state.gui_state.build_placing.as_ref() {
        if let Some(g) = p.ghost.as_ref() {
            push(g, g.scale, p.site.as_ref(), scaffold_mat, 0.5, true);
        }
    }
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
        let Some(body) = state.frame_lock_body.clone() else { return "not aboard and not over a body: nothing built".into() };
        let Some(ground) = Ground::for_body(state, &body) else { return format!("{body} has no surface to build on") };
        let base_dir = match at.map(|a| a.split(',').filter_map(|v| v.trim().parse::<f64>().ok()).collect::<Vec<_>>()) {
            Some(ll) if ll.len() == 2 => crate::terrain::osm_region::latlon_to_dir_f64(ll[0], ll[1]),
            _ => {
                // The crosshair: march the look ray to the ground, up to 3 km.
                let eye_body = state.frame_lock_anchor;
                let up = eye_body.normalize_or_zero();
                let probe = PlanetSite { body: body.clone(), origin: up * ground.radius(up) };
                let look = probe.dir_to_local(body_rot(state).inverse() * state.camera.forward().as_dvec3());
                let hit = placement::ray_ground_hit(probe.to_local(eye_body), look, &|x, z| ground.height(&probe, x, z), 3_000.0);
                let Some(hit) = hit else { return "the crosshair meets no ground within 3 km: nothing built".into() };
                probe.to_body(hit).normalize()
            }
        };
        let base_ground = base_dir * ground.radius(base_dir);
        let site = site::site_for(&state.game_world.world, &body, base_ground);
        let base = site.to_local(base_ground);
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
            let rot = DQuat::from_rotation_y(spin);
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
}
