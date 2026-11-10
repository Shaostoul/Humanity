//! THE GROUND UNDER A PERSON'S FEET (character animation rung 2a, 2026-11-09;
//! docs/design/character-animation.md section 4).
//!
//! humanity-animation's feet (`humanity_animation::feet`) ask "what is the ground under this
//! point?" through a trait, and this file answers it from what the game already stands bodies on,
//! so a drawn foot meets the same floor the walk stands the body on:
//!
//! - ABOARD, [`ShipGround`]: `engine::footing`, asked per sole point (the room's floor, a floor
//!   slab, a stair tread, a ramp, a deck, a platform, an elevator car, a finished built piece),
//!   with the person's own root as the height a step up is measured from, so a foot finds the tread
//!   a body there could step onto and not the deck over its head. A stair tread also says where its
//!   middle is (`Footing::tread_at`, from the stair piece's own riser and tread), which the footstep
//!   planner puts the foot on.
//! - ON A PLANET, [`PlanetGround`]: the drawn ground the planet walk stands the player on (the
//!   patch drawn there this frame, map packs' road beds and building pads included; the elevation
//!   field where none is drawn yet), with every position AND direction in f64 all the way to the
//!   sampler (CLAUDE.md's planet-scale rule: an f32 unit vector quantizes ground position at
//!   0.4 to 0.8 m). A render-space point near the camera becomes a point in the body's unrotated
//!   frame as `anchor + rot^-1 (p - camera)` (the camera stays parked while the planet moves under
//!   it, `planet_build::PlayerView`), and the ground point comes back the same way. Its slope is
//!   read from two more samples a quarter metre away in the ground's tangent plane.

use std::cell::RefCell;

use glam::{DQuat, DVec3, Vec3};
use humanity_animation::{Ground, GroundHit, StepInfo};
use humanity_terrain::drawn_surface::DrawnPatchSurface;
use humanity_terrain::planet::PlanetDef;

use crate::engine::footing::Footing;

/// Aboard: the footing, with the person's root (feet) height as the reference a step up is measured
/// from.
pub(crate) struct ShipGround<'a> {
    pub footing: Footing<'a>,
    /// Where the person's feet stand, world Y.
    pub reference: f32,
}

impl Ground for ShipGround<'_> {
    fn ground(&self, at: Vec3) -> Option<GroundHit> {
        let eye = Vec3::new(at.x, self.reference + humanity_ship::wall_collision::EYE_HEIGHT_M, at.z);
        let y = self.footing.floor_under(eye, self.reference)?;
        let step = self.footing.tread_at(at, y).map(|(axis, centre, depth)| StepInfo { axis, centre, depth });
        Some(GroundHit { point: Vec3::new(at.x, y, at.z), normal: Vec3::Y, step })
    }
}

/// The ground of a planet as the player stands on it this frame, asked by render-space points.
pub(crate) struct PlanetGround<'s, 'a, 'b> {
    pub def: &'a PlanetDef,
    /// The drawn ground (one surface for the frame's questions; None before any patch is drawn,
    /// when the field below answers).
    pub drawn: Option<RefCell<DrawnPatchSurface<'s, 'b>>>,
    pub hm: Option<&'a humanity_terrain::planet_heightmap::PlanetHeightmap>,
    pub detail: &'a humanity_terrain::planet_chunks::DetailNoise,
    pub tiles: Option<&'a humanity_terrain::terrain_tiles::TerrainTiles>,
    /// The eye in the body's unrotated frame (`frame_lock_anchor`), the camera's render position
    /// it is drawn at, and the body's rotation into render space this frame.
    pub anchor: DVec3,
    pub camera: Vec3,
    pub rot: DQuat,
}

impl PlanetGround<'_, '_, '_> {
    /// The ground's radius under a unit direction of the body's frame, f64 end to end.
    pub(crate) fn radius(&self, dir: DVec3) -> f64 {
        if let Some(d) = &self.drawn {
            let r = d.borrow_mut().radius_at(dir);
            if r.is_finite() && r > 0.0 {
                return r;
            }
        }
        crate::engine::frame_lock::ground_radius_m(Some(self.def), self.hm, Some(self.detail), self.tiles, dir)
    }

    /// A render-space point in the body's unrotated frame.
    pub(crate) fn to_body(&self, p: Vec3) -> DVec3 {
        self.anchor + self.rot.inverse() * (p - self.camera).as_dvec3()
    }

    /// A body-frame point in render space.
    pub(crate) fn to_render(&self, b: DVec3) -> Vec3 {
        self.camera + (self.rot * (b - self.anchor)).as_vec3()
    }
}

/// The tangent offset the slope is read across, metres.
const PLANET_SLOPE_M: f64 = 0.25;

impl Ground for PlanetGround<'_, '_, '_> {
    fn ground(&self, at: Vec3) -> Option<GroundHit> {
        let dir = self.to_body(at).normalize_or_zero();
        if dir == DVec3::ZERO {
            return None;
        }
        let r = self.radius(dir);
        let g = dir * r;
        let t1 = dir.any_orthonormal_vector();
        let t2 = dir.cross(t1);
        let at_offset = |t: DVec3| {
            let d = (dir + t * (PLANET_SLOPE_M / r.max(1.0))).normalize();
            d * self.radius(d)
        };
        let (ga, gb) = (at_offset(t1), at_offset(t2));
        let mut n = (ga - g).cross(gb - g).normalize_or_zero();
        if n.dot(dir) < 0.0 {
            n = -n;
        }
        if n == DVec3::ZERO {
            n = dir;
        }
        Some(GroundHit { point: self.to_render(g), normal: (self.rot * n).as_vec3().normalize_or_zero(), step: None })
    }
}

/// The ground a person stands on this frame: aboard, on a planet, or nowhere the game knows.
pub(crate) enum WorldGround<'s, 'a, 'b> {
    Ship { rooms: &'a [humanity_ship::room_types::RoomBounds], ship: Option<&'a humanity_ship::ship_structure::ShipStructure>, cars: &'a crate::engine::footing::ElevatorCars, built: &'a [(Vec3, Vec3)] },
    Planet(PlanetGround<'s, 'a, 'b>),
    /// A level floor at this height (the character showroom's podium).
    Flat(f32),
    None,
}

impl WorldGround<'_, '_, '_> {
    /// Is there any ground to put feet on?
    pub(crate) fn is_some(&self) -> bool {
        !matches!(self, WorldGround::None)
    }
}

/// One person's question to the frame's ground: the world's, with their own feet as the reference.
pub(crate) struct PersonGround<'g, 's, 'a, 'b> {
    pub world: &'g WorldGround<'s, 'a, 'b>,
    pub reference: f32,
}

impl Ground for PersonGround<'_, '_, '_, '_> {
    fn ground(&self, at: Vec3) -> Option<GroundHit> {
        match self.world {
            WorldGround::Ship { rooms, ship, cars, built } => ShipGround { footing: Footing { rooms, ship: *ship, cars, built }, reference: self.reference }.ground(at),
            WorldGround::Planet(p) => p.ground(at),
            WorldGround::Flat(y) => Some(GroundHit { point: Vec3::new(at.x, *y, at.z), normal: Vec3::Y, step: None }),
            WorldGround::None => None,
        }
    }
}

/// A flat floor at a height, and a stair of treads on it (the engine's feet tests, without a ship).
#[cfg(test)]
pub(crate) struct TestGround {
    pub floor: f32,
    /// A stair climbing +X from `x0`: `n` treads of `tread` metres, each `riser` higher.
    pub stair: Option<(f32, f32, f32, usize)>,
    /// A ramp: the floor rises `slope` metres per metre along +X past x = 0.
    pub slope: f32,
}

#[cfg(test)]
impl Ground for TestGround {
    fn ground(&self, at: Vec3) -> Option<GroundHit> {
        if let Some((x0, tread, riser, n)) = self.stair {
            if at.x >= x0 && at.x < x0 + tread * n as f32 {
                let i = ((at.x - x0) / tread).floor();
                let y = self.floor + (i + 1.0) * riser;
                let centre = x0 + (i + 0.5) * tread;
                return Some(GroundHit { point: Vec3::new(at.x, y, at.z), normal: Vec3::Y, step: Some(StepInfo { axis: Vec3::X, centre, depth: tread }) });
            }
            if at.x >= x0 + tread * n as f32 {
                return Some(GroundHit { point: Vec3::new(at.x, self.floor + riser * n as f32, at.z), normal: Vec3::Y, step: None });
            }
        }
        let y = self.floor + self.slope * at.x.max(0.0);
        let normal = if at.x > 0.0 { Vec3::new(-self.slope, 1.0, 0.0).normalize() } else { Vec3::Y };
        Some(GroundHit { point: Vec3::new(at.x, y, at.z), normal, step: None })
    }
}
