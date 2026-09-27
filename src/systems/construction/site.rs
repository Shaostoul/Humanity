//! Built pieces on a planet's surface: the BUILD SITE (2026-09-27, BUG-102).
//!
//! Pieces built aboard live in the HOME frame, the ship's own coordinates,
//! and are drawn wherever the station is. On a planet the camera stays put
//! while the ship frame moves under it, so a piece placed from the surface in
//! the home frame was drawn at the station, hundreds of km up, and the
//! shelter test followed the player through the rain (BUG-102). A piece built
//! on a planet stands in a build site instead: a flat frame pinned to the
//! ground of the body it stands on.
//!
//! A site is its body and an ORIGIN in that body's UNROTATED local frame (the
//! frame the terrain is built in and the frame lock's anchor lives in), in
//! f64. At 6,371 km an f32 is good to half a metre, so the origin never
//! passes through f32 (CLAUDE.md, "f32 at planet scale"); only offsets from
//! it, a few hundred metres at most, are narrowed. The site's axes follow
//! from the origin alone ([`tangent_basis`]: up is the radial there, +X runs
//! the way the ground turns), so every piece of a site shares them bit for
//! bit and a save needs nothing but the body and the origin.
//!
//! Inside a site everything works exactly as it does aboard: the metre grid,
//! turning, resting on top, the shelter rule, the look ray. A planet piece is
//! a `Structure` (or a `Construction`) with a SITE-LOCAL `Transform`, metres
//! from the origin with Y up, plus a [`PlanetSite`] component; a home piece
//! has no `PlanetSite`. Pieces are only ever compared with pieces in the same
//! frame ([`in_frame`]), which is what keeps a wall on Earth from holding up a
//! roof on the ship. Across a site's reach ([`SITE_JOIN_M`]) Earth's
//! curvature drops the ground by 8 cm, so the flat frame is honest.
//!
//! Each frame the renderer draws a site piece at `render_off + rot * p`,
//! where `render_off` and `rot` are the body's centre and orientation in
//! render space that frame and `p` the piece in the body's frame: the same
//! transform the terrain patches, the near trees and the OSM buildings use
//! (`engine::region_meshes`), narrowed to f32 only at the end
//! ([`PlanetSite::render_pose`]).

use super::{Construction, Structure};
use crate::ecs::components::Transform;
use glam::{DMat3, DQuat, DVec3, Quat, Vec3};
use serde::{Deserialize, Serialize};

/// How far from an existing site's origin a new piece joins that site (and
/// its grid) rather than starting a site of its own, metres. A kilometre
/// holds a homestead and its fields; curvature across it is 8 cm on Earth.
pub const SITE_JOIN_M: f64 = 1_000.0;

/// The planet-fixed frame a group of built pieces stands in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanetSite {
    /// The body it stands on (`cosmos` id, e.g. "earth", "moon").
    pub body: String,
    /// The site origin in the body's unrotated local frame, metres from the
    /// body's centre. On the ground where the first piece was placed from.
    pub origin: DVec3,
}

impl PlanetSite {
    /// The site's axes in the body's frame (see [`tangent_basis`]).
    pub fn basis(&self) -> DQuat {
        tangent_basis(self.origin)
    }

    /// A site-local point (metres, Y up) in the body's frame, in f64.
    pub fn to_body(&self, local: Vec3) -> DVec3 {
        self.origin + self.basis() * local.as_dvec3()
    }

    /// A body-frame point in site-local metres. The subtraction runs in f64
    /// on planet-radius numbers; only the small result is narrowed.
    pub fn to_local(&self, body: DVec3) -> Vec3 {
        (self.basis().inverse() * (body - self.origin)).as_vec3()
    }

    /// A body-frame direction in site-local axes.
    pub fn dir_to_local(&self, dir: DVec3) -> Vec3 {
        (self.basis().inverse() * dir).as_vec3()
    }

    /// Where a site-local transform draws this frame: its position and
    /// rotation in render space, given the body's centre (`render_off`) and
    /// orientation (`rot`) in render space. f64 until the final narrowing,
    /// exactly like `region_meshes`' `render_off + rot_d * anchor_local`.
    pub fn render_pose(&self, tf: &Transform, render_off: DVec3, rot: DQuat) -> (Vec3, Quat) {
        let p = render_off + rot * self.to_body(tf.position);
        let r = (rot * self.basis()).as_quat() * tf.rotation;
        (p.as_vec3(), r.normalize())
    }
}

/// A site's axes for an origin in a body's unrotated frame, as the rotation
/// taking site-local axes to body axes: +Y is the local up (the radial at
/// the origin), +X runs the way the ground turns (the body spins about its
/// +Y, so this is east on a planet turning like Earth), and +Z = X x Y, so
/// -Z is north. Near a pole, where the spin axis and the up nearly agree,
/// +X is taken from the body's X axis instead so the frame stays
/// well-conditioned.
pub fn tangent_basis(origin: DVec3) -> DQuat {
    let up = origin.normalize_or_zero();
    if up.length_squared() < 0.5 {
        return DQuat::IDENTITY;
    }
    let reference = if up.dot(DVec3::Y).abs() > 0.999 { DVec3::X } else { DVec3::Y };
    let east = reference.cross(up).normalize();
    let south = east.cross(up);
    DQuat::from_mat3(&DMat3::from_cols(east, up, south)).normalize()
}

/// Are a piece and a frame the same frame? `None` is the home frame. Every
/// query that compares pieces (what a piece rests on, the shelter, the look
/// ray, a duplicate) runs over pieces in one frame only.
pub fn in_frame(piece: Option<&PlanetSite>, frame: Option<&PlanetSite>) -> bool {
    piece == frame
}

/// The site a piece placed from `feet` (a point on the ground, in `body`'s
/// frame) goes into: the nearest site already standing on that body within
/// [`SITE_JOIN_M`], so a new wall lines up on the same grid as the last one,
/// or a new site with its origin at `feet`.
pub fn site_for(world: &hecs::World, body: &str, feet: DVec3) -> PlanetSite {
    nearest_site(world, body, feet, SITE_JOIN_M).unwrap_or_else(|| PlanetSite { body: body.to_string(), origin: feet })
}

/// The nearest site on `body` with pieces in it (finished or going up) whose
/// origin is within `within` metres of `at`, or None.
pub fn nearest_site(world: &hecs::World, body: &str, at: DVec3, within: f64) -> Option<PlanetSite> {
    let mut best: Option<(f64, PlanetSite)> = None;
    let mut consider = |s: &PlanetSite| {
        if s.body != body {
            return;
        }
        let d = (s.origin - at).length();
        if d <= within && best.as_ref().map_or(true, |(bd, _)| d < *bd) {
            best = Some((d, s.clone()));
        }
    };
    for (_e, (_, s)) in world.query::<(&Structure, &PlanetSite)>().iter() {
        consider(s);
    }
    for (_e, (_, s)) in world.query::<(&Construction, &PlanetSite)>().iter() {
        consider(s);
    }
    best.map(|(_, s)| s)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EARTH_R: f64 = 6_371_000.0;

    /// A point on the ground at a latitude and longitude, degrees.
    fn ground(lat: f64, lon: f64) -> DVec3 {
        let (la, lo) = (lat.to_radians(), lon.to_radians());
        DVec3::new(la.cos() * lo.cos(), la.sin(), -la.cos() * lo.sin()) * EARTH_R
    }

    /// The site frame is a proper right-handed frame with +Y the local up,
    /// everywhere including at the poles, and a site-local point survives
    /// the round trip to the body frame and back to well under a millimetre
    /// at planet scale (the f32 rule: the origin never passes through f32).
    /// Red check, run: computing `to_local` as `body.as_vec3() -
    /// origin.as_vec3()` (narrowing first) misses by decimetres and the
    /// round-trip assertion fails.
    #[test]
    fn the_site_frame_is_up_at_the_origin_and_exact_at_planet_scale() {
        for (lat, lon) in [(47.6, -122.7), (23.0, 13.0), (0.0, 0.0), (89.99, 10.0), (-90.0, 0.0)] {
            let site = PlanetSite { body: "earth".into(), origin: ground(lat, lon) };
            let q = site.basis();
            let up = q * DVec3::Y;
            assert!((up - site.origin.normalize()).length() < 1e-9, "up at {lat},{lon}: {up}");
            let (x, y, z) = (q * DVec3::X, q * DVec3::Y, q * DVec3::Z);
            assert!((x.cross(y) - z).length() < 1e-9, "right-handed at {lat},{lon}");
            for local in [Vec3::new(3.25, 0.125, -7.5), Vec3::new(-400.0, 12.0, 250.0)] {
                let back = site.to_local(site.to_body(local));
                assert!((back - local).length() < 1e-3, "{local} -> {back} at {lat},{lon}");
            }
        }
    }

    /// A new piece joins the nearest site within reach on the same body, and
    /// starts its own site beyond it or on another body.
    #[test]
    fn a_piece_joins_the_nearest_site_on_its_body() {
        let a = PlanetSite { body: "earth".into(), origin: ground(47.6, -122.7) };
        let b = PlanetSite { body: "earth".into(), origin: ground(47.6, -122.69) };
        let mut world = hecs::World::new();
        let wall = || Structure { blueprint_id: "wood_wall".into(), health: 1.0, max_health: 1.0, provides: None, uid: 0 };
        world.spawn((Transform::default(), wall(), a.clone()));
        world.spawn((Transform::default(), wall(), b.clone()));
        let near_a = a.to_body(Vec3::new(20.0, 0.0, 5.0));
        assert_eq!(site_for(&world, "earth", near_a), a);
        let near_b = b.to_body(Vec3::new(-30.0, 0.0, 0.0));
        assert_eq!(site_for(&world, "earth", near_b), b);
        let far = ground(10.0, 10.0);
        assert_eq!(site_for(&world, "earth", far), PlanetSite { body: "earth".into(), origin: far });
        assert_eq!(site_for(&world, "moon", near_a).origin, near_a, "another body's site is never joined");
    }
}
