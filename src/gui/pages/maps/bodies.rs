//! How the Maps page draws a BODY (Maps increment 2, docs/design/maps-zoom-to-anything.md
//! 5.3): "true scale and true distance ... A body whose disc would be under
//! ~6 px is drawn as an ICON of a minimum size: a disc of its real colour lit
//! on the side facing the Sun (a crescent, quarter or gibbous shape from the
//! real Sun-body-camera angle), so even a 4 px Mars reads as a lit world, not
//! a dot. Over ~6 px the icon fades out and the world layer's real globe takes
//! over."
//!
//! Until this increment every body was one flat `circle_filled` in a colour
//! from a hard-coded ten-name match, and every moon and asteroid the same grey.
//!
//! * The disc is a small mesh of rings, each vertex shaded by Lambert's law
//!   against the Sun's direction in the camera's frame, so the lit side, the
//!   terminator and the phase come out of the real geometry at any size: a
//!   crescent Venus near the Sun's line, a gibbous Mars, a half-lit Moon at
//!   quarter. The night side is kept at a tenth of the day side's light, a map
//!   convention (a real night side is black against black space) so the
//!   disc's outline still reads; the edge is feathered a pixel for smoothness.
//! * The colour is DATA: sol.json's `map_color` (`SolBody::map_color`, the
//!   mean of the body's albedo map where it has one). It is shown through an
//!   exposure curve, `icon_lit_linear`, the way a camera shows a scene: the
//!   true reflectances run from 4% (a comet) to 99% (Enceladus), and a linear
//!   picture would draw most of the family as near-black dots. The curve
//!   keeps their order and their hue.
//! * The Sun is a disc of its own light, darkened toward the limb as the real
//!   photosphere is (`I(mu) = 1 - u (1 - mu)`, u = 0.6, the common linear
//!   limb-darkening law for visible light), with GLARE round it: a
//!   point-spread function falling off as the inverse square and cube of the
//!   angle from the Sun, the shape Spencer, Shirley, Zimmerman and Greenberg
//!   fitted to the eye's own glare ("Physically-Based Glare Effects for
//!   Digital Images", SIGGRAPH 1995). It is drawn in the symbol layer because
//!   in the solar-system band the world layer draws the star sky only (the
//!   celestial pass cannot be run from a camera an AU away: `you_layer.rs`);
//!   where the world layer draws everything, the real Sun is in it and this
//!   glare is left out.
//!
//! Everything is appended to ONE mesh per frame in back-to-front order, so the
//! whole family costs one draw and no tessellation.

use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{Color32, Pos2, Vec2};
use glam::DVec3;

use super::camera::MapPose;

/// The smallest radius, px, a body is drawn at, by kind: the icon floor.
pub fn icon_floor_px(id: &str, body_type: &str) -> f32 {
    match (id, body_type) {
        ("sun", _) => 5.0,
        (_, "terrestrial" | "gas_giant" | "ice_giant") => 3.5,
        (_, "dwarf_planet") => 3.0,
        (_, "moon") => 2.4,
        _ => 2.0,
    }
}

/// Radius, px, where the icon begins to give way to the real globe, and where
/// it is gone: 5 to 8 px across, the design's "~6 px".
pub const HANDOFF_FROM_PX: f32 = 2.5;
pub const HANDOFF_TO_PX: f32 = 4.0;

/// How much of a body's icon to draw (1 all, 0 none) when its true disc is
/// `true_r_px` in radius and the world layer is drawing real globes.
pub fn icon_alpha(true_r_px: f32, globe_drawn: bool) -> f32 {
    if !globe_drawn {
        return 1.0;
    }
    1.0 - smoothstep(HANDOFF_FROM_PX, HANDOFF_TO_PX, true_r_px)
}

pub fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// sRGB byte to linear light.
pub fn lin(b: u8) -> f32 {
    let c = b as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear light to an sRGB byte.
pub fn srgb(l: f32) -> u8 {
    let c = l.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0).round() as u8
}

/// The colour a body's lit side is painted in, linear RGB: its map colour
/// through the exposure curve (module doc). The brightest channel goes from
/// `m` to `m^0.4`, the others scale with it, so the hue is kept and a 4%
/// comet still shows.
pub fn icon_lit_linear(c: [u8; 3]) -> [f32; 3] {
    let l = [lin(c[0]), lin(c[1]), lin(c[2])];
    let m = l[0].max(l[1]).max(l[2]);
    if m <= 1e-6 {
        return [0.25; 3];
    }
    let k = m.powf(0.4) / m;
    [l[0] * k, l[1] * k, l[2] * k]
}

/// `c` (a theme colour, read unmultiplied) at `f` times its own opacity,
/// premultiplied in GAMMA space. egui's `from_rgba_unmultiplied`
/// premultiplies in linear light, so a faint point keeps most of its
/// brightness on screen (sRGB 209 at 8% opacity adds 63 of 255, a quarter);
/// thousands of faint dust points or the faded end of a trail then add up to
/// a solid blob (the 60 AU overview's main belt, 2026-10-21). Premultiplied
/// here, 8% opacity adds 8% of the colour, as a fade is meant to.
pub fn faded(c: Color32, f: f32) -> Color32 {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    let k = (a as f32 / 255.0 * f).clamp(0.0, 1.0);
    let m = |v: u8| (v as f32 * k).round() as u8;
    Color32::from_rgba_premultiplied(m(r), m(g), m(b), (k * 255.0).round() as u8)
}

/// A body's colour as an egui colour, its map colour unchanged (lists, dots).
pub fn map_color32(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// The direction to the Sun from a body, in the camera's frame: x right, y
/// up, z toward the viewer. `body` and `sun` are map-frame positions.
pub fn sun_in_view(pose: &MapPose, body: DVec3, sun: DVec3) -> DVec3 {
    let s = (sun - body).normalize_or_zero();
    DVec3::new(s.dot(pose.right), s.dot(pose.up), -s.dot(pose.forward))
}

/// The fraction of the visible disc that is lit, from the Sun's direction in
/// the camera's frame: (1 + cos of the phase angle) / 2. Full face-on is 1, a
/// new moon 0, a quarter one half.
pub fn illuminated_fraction(sun_view: DVec3) -> f64 {
    (1.0 + sun_view.z.clamp(-1.0, 1.0)) * 0.5
}

/// The night side's share of the day side's light (module doc).
const NIGHT: f32 = 0.10;
/// The feathered edge, px.
const FEATHER: f32 = 0.8;

fn color(l: [f32; 3], a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(srgb(l[0]), srgb(l[1]), srgb(l[2]), (a.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// Rings and segments for a disc of radius `r` px: more for a big one.
fn tessellation(r: f32) -> (usize, usize) {
    let rings = ((r / 3.0).ceil() as usize).clamp(3, 12);
    let segs = ((r * 1.6).ceil() as usize).clamp(18, 72);
    (rings, segs)
}

/// Append a disc of radius `r` at `centre` whose vertex colours come from
/// `shade(u, v, w)` (u right, v up, w toward the viewer, on the unit sphere's
/// visible half), with a feathered rim.
fn add_disc(mesh: &mut Mesh, centre: Pos2, r: f32, alpha: f32, shade: impl Fn(f32, f32, f32) -> [f32; 3]) {
    let (rings, segs) = tessellation(r);
    let base = mesh.vertices.len() as u32;
    let push = |mesh: &mut Mesh, u: f32, v: f32, a: f32, scale: f32| {
        let w = (1.0 - u * u - v * v).max(0.0).sqrt();
        mesh.vertices.push(Vertex { pos: centre + Vec2::new(u * r * scale, -v * r * scale), uv: WHITE_UV, color: color(shade(u, v, w), a) });
    };
    push(mesh, 0.0, 0.0, alpha, 1.0);
    for k in 1..=rings {
        let rr = k as f32 / rings as f32;
        for s in 0..segs {
            let th = s as f32 / segs as f32 * std::f32::consts::TAU;
            push(mesh, rr * th.cos(), rr * th.sin(), alpha, 1.0);
        }
    }
    // The feather: the rim's colour at no opacity, a pixel out.
    let feather_scale = (r + FEATHER) / r.max(0.01);
    for s in 0..segs {
        let th = s as f32 / segs as f32 * std::f32::consts::TAU;
        push(mesh, th.cos(), th.sin(), 0.0, feather_scale);
    }
    let ring_start = |k: usize| base + 1 + ((k - 1) * segs) as u32;
    for s in 0..segs {
        let s1 = (s + 1) % segs;
        mesh.add_triangle(base, ring_start(1) + s as u32, ring_start(1) + s1 as u32);
    }
    for k in 1..=rings {
        let (a, b) = (ring_start(k), ring_start(k + 1));
        for s in 0..segs {
            let s1 = (s + 1) % segs;
            let (a0, a1, b0, b1) = (a + s as u32, a + s1 as u32, b + s as u32, b + s1 as u32);
            mesh.add_triangle(a0, b0, b1);
            mesh.add_triangle(a0, b1, a1);
        }
    }
}

/// Append a body's phase-lit disc: `lit` (linear) where the Sun shines
/// straight down, Lambert's falloff to the terminator, `NIGHT` beyond it.
pub fn add_phase_disc(mesh: &mut Mesh, centre: Pos2, r: f32, lit: [f32; 3], sun_view: DVec3, alpha: f32) {
    let s = sun_view.normalize_or_zero().as_vec3();
    add_disc(mesh, centre, r, alpha, |u, v, w| {
        let lambert = (u * s.x + v * s.y + w * s.z).max(0.0);
        let k = NIGHT + (1.0 - NIGHT) * lambert;
        [lit[0] * k, lit[1] * k, lit[2] * k]
    });
}

/// Append the Sun's disc: its own light, limb-darkened (module doc).
pub fn add_sun_disc(mesh: &mut Mesh, centre: Pos2, r: f32, light: [f32; 3]) {
    const U: f32 = 0.6;
    add_disc(mesh, centre, r, 1.0, |_, _, w| {
        let k = 1.0 - U * (1.0 - w);
        [light[0] * k, light[1] * k, light[2] * k]
    });
}

/// The glare's brightness at `deg` degrees from the Sun's centre, before
/// exposure: the two far terms of Spencer et al.'s point-spread function (the
/// near core is the disc itself).
pub fn glare_psf(deg: f64) -> f64 {
    let t = deg.max(0.0) + 0.02;
    0.478 * 20.91 / (t * t * t) + 0.138 * 72.37 / (t * t)
}

/// How bright the glare is drawn: the exposure the PSF is seen at.
const GLARE_EXPOSURE: f64 = 0.10;
/// How far out the glare is drawn, degrees.
const GLARE_MAX_DEG: f64 = 30.0;

/// The glare as drawn at `deg` from the Sun: 0 to 1.
pub fn glare_at(deg: f64) -> f32 {
    (1.0 - (-GLARE_EXPOSURE * glare_psf(deg)).exp()) as f32
}

/// Append the Sun's glare round a disc of `r_disc` px at `centre`, the view
/// showing `deg_per_px` degrees a pixel: rings spaced evenly in log radius,
/// each ring's light added to what is under it (premultiplied colour with no
/// alpha, which egui's blend adds).
pub fn add_sun_glare(mesh: &mut Mesh, centre: Pos2, r_disc: f32, deg_per_px: f32, light: [f32; 3]) {
    if deg_per_px <= 0.0 {
        return;
    }
    const RINGS: usize = 28;
    const SEGS: usize = 64;
    let r0 = r_disc.max(1.0);
    let r1 = (GLARE_MAX_DEG / deg_per_px as f64) as f32;
    if r1 <= r0 * 1.05 {
        return;
    }
    let base = mesh.vertices.len() as u32;
    for k in 0..=RINGS {
        let rr = r0 * (r1 / r0).powf(k as f32 / RINGS as f32);
        // Measured from the disc's edge outward: inside the disc is the disc.
        let g = if k == RINGS { 0.0 } else { glare_at(((rr - r_disc).max(0.0) * deg_per_px) as f64 + 0.25) };
        let c = Color32::from_rgba_premultiplied(srgb(light[0] * g), srgb(light[1] * g), srgb(light[2] * g), 0);
        for s in 0..SEGS {
            let th = s as f32 / SEGS as f32 * std::f32::consts::TAU;
            mesh.vertices.push(Vertex { pos: centre + Vec2::new(th.cos(), th.sin()) * rr, uv: WHITE_UV, color: c });
        }
    }
    for k in 0..RINGS {
        let (a, b) = (base + (k * SEGS) as u32, base + ((k + 1) * SEGS) as u32);
        for s in 0..SEGS {
            let s1 = (s + 1) % SEGS;
            mesh.add_triangle(a + s as u32, b + s as u32, b + s1 as u32);
            mesh.add_triangle(a + s as u32, b + s1 as u32, a + s1 as u32);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The phase comes from the real geometry: with the Sun behind the
    /// camera the whole face is lit, beside it half, behind the body none;
    /// and the drawn disc agrees, its brightest vertex on the Sun's side.
    ///
    /// Seen red 2026-10-21 with `sun_in_view` returning the Sun's direction in
    /// world axes, not the camera's (world +Z read as "toward the viewer", a
    /// full face): "with the Sun to the right the lit side is at x 0.00".
    #[test]
    fn the_lit_side_faces_the_sun_and_the_phase_follows_the_angle() {
        assert!((illuminated_fraction(DVec3::Z) - 1.0).abs() < 1e-12);
        assert!((illuminated_fraction(DVec3::X) - 0.5).abs() < 1e-12);
        assert!(illuminated_fraction(-DVec3::Z).abs() < 1e-12);
        // A camera looking along world +X (its right is world +Z), the Sun
        // off along world +Z: in the camera's frame the Sun is to the right.
        let pose = MapPose { look_at: DVec3::ZERO, eye: DVec3::new(-10.0, 0.0, 0.0), forward: DVec3::X, right: DVec3::Z, up: DVec3::Y, distance: 10.0, fov_y: 1.0 };
        let sun_view = sun_in_view(&pose, DVec3::ZERO, DVec3::new(0.0, 0.0, 1.0e11));
        let mut mesh = Mesh::default();
        add_phase_disc(&mut mesh, Pos2::new(100.0, 100.0), 20.0, [0.8, 0.4, 0.2], sun_view, 1.0);
        let brightest = mesh.vertices.iter().max_by_key(|v| v.color.r()).expect("a mesh");
        let x = (brightest.pos.x - 100.0) / 20.0;
        assert!(x > 0.5, "with the Sun to the right the lit side is at x {x:.2}");
        let darkest = mesh.vertices.iter().filter(|v| v.color.a() > 0).map(|v| v.color.r()).min().expect("a mesh");
        assert!(
            mesh.vertices.iter().filter(|v| v.color.a() > 0 && v.color.r() == darkest).all(|v| v.pos.x <= 100.001),
            "the night side is not all on the left"
        );
    }

    /// The exposure keeps every body visible and in its order of brightness:
    /// a 4% comet still has a fifth of full light, Enceladus is brighter than
    /// the Moon, and Mars keeps its red.
    ///
    /// Seen red 2026-10-21 with the map colour painted straight (no curve):
    /// "a 4% comet's icon is lit at 0.040, under 0.2".
    #[test]
    fn the_exposure_keeps_dark_bodies_visible_and_in_order() {
        let comet = icon_lit_linear(crate::cosmos::grey_at_albedo(0.04));
        assert!(comet[0] >= 0.2, "a 4% comet's icon is lit at {:.3}, under 0.2", comet[0]);
        let moon = icon_lit_linear([0x9c, 0x99, 0x98]);
        let enceladus = icon_lit_linear(crate::cosmos::grey_at_albedo(0.99));
        assert!(enceladus[0] > moon[0] && moon[0] > comet[0]);
        let mars = icon_lit_linear([0xb8, 0x5d, 0x41]);
        assert!(mars[0] > 2.0 * mars[1] && mars[0] > 2.0 * mars[2], "Mars lost its red: {mars:?}");
    }

    /// The icon gives way to the real globe between 5 and 8 px across, and
    /// only where the world layer is drawing globes.
    ///
    /// Seen red 2026-10-21 with the hand-off ignoring the world layer: "with no
    /// globe drawn, a 10 px Earth's icon is drawn at 0.00".
    #[test]
    fn the_icon_hands_off_to_the_globe_at_about_six_pixels() {
        assert_eq!(icon_alpha(1.5, true), 1.0);
        assert!(icon_alpha(3.25, true) > 0.0 && icon_alpha(3.25, true) < 1.0);
        assert_eq!(icon_alpha(5.0, true), 0.0);
        assert_eq!(icon_alpha(5.0, false), 1.0, "with no globe drawn, a 10 px Earth's icon is drawn at {:.2}", icon_alpha(5.0, false));
    }

    /// The glare falls off steeply away from the Sun: bright beside the disc,
    /// faint by Earth's orbit at the overview, gone by 30 degrees.
    ///
    /// Seen red 2026-10-21 with the exposure at 1.0 instead of 0.10 (a glare
    /// that washes out the inner system): "glare 1 0.3783099 0.016455019".
    #[test]
    fn the_glare_falls_off_with_the_angle() {
        let near = glare_at(0.5);
        let mid = glare_at(5.0);
        let far = glare_at(25.0);
        assert!(near > 0.9 && mid < 0.1 && mid > 0.02 && far < 0.003, "glare {near} {mid} {far}");
    }
}
