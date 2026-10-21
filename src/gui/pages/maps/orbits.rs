//! The Maps page's ORBIT LINES (Maps increment 2, docs/design/maps-zoom-to-anything.md
//! 5.4), the "bunch of lines" of the operator's 2026-10-20 note made into
//! something that reads as motion:
//!
//! * TRAIL. Bright where the body is and fading to a faint line half an orbit
//!   behind it, so every orbit shows which way its body goes and where it is.
//!   The rest of the ellipse stays faint (you can see where it is going).
//!   The fade is measured in MEAN anomaly, which is time, so an eccentric
//!   comet's trail is half its period, not half its ellipse.
//! * THE BODY IS A VERTEX. The line starts and ends at the body's own
//!   eccentric anomaly (`cosmos::eccentric_anomaly_at`), the same point the
//!   body is drawn at, so a body never sits off its line however coarse the
//!   rest of the sampling.
//! * ADAPTIVE SAMPLING. 64 even steps round the ellipse, then any piece whose
//!   middle strays more than a third of a pixel from its chord is split, but
//!   only where the piece is on or near the canvas. Zoomed in on Earth its
//!   1 AU orbit is sampled finely beside Earth and not at all on the far side,
//!   instead of the old 96 fixed chords, each 10 million km long.
//! * SIZE FADE. An orbit under about 8 px across fades out (a far planet's
//!   moons stop cluttering), and one far bigger than the screen thins and
//!   fades (Earth's orbit passing straight through the view at ship scale).
//!
//! Drawn as ONE anti-aliased triangle mesh with a colour per vertex (the
//! trail), not thousands of line shapes for egui to tessellate.

use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{Color32, Pos2, Rect, Vec2};
use glam::DVec3;

use super::camera::MapProjector;

/// Even steps the first pass takes round an ellipse.
const BASE_STEPS: usize = 64;
/// A piece is split when its middle strays this far from its chord, px.
const TOLERANCE_PX: f32 = 0.35;
/// The most an orbit is split (2^DEPTH pieces per base step at most).
const MAX_DEPTH: u32 = 12;
/// The most points one orbit gets.
const MAX_POINTS: usize = 2048;
/// The faint line the rest of the orbit keeps, as a share of the trail's head.
pub const TRAIL_FLOOR: f32 = 0.22;

/// One sample of an orbit line: where on the ellipse (eccentric anomaly,
/// radians, unwrapped from the body's own) and where on the canvas (None
/// behind the camera).
#[derive(Debug, Clone, Copy)]
pub struct OrbitSample {
    pub ea: f64,
    pub at: Option<Pos2>,
}

/// Sample an orbit from the body round to the body again. `eval` gives the
/// map-frame point at an eccentric anomaly; `ea_body` is the body's own, the
/// first and last sample.
pub fn sample_adaptive(eval: &dyn Fn(f64) -> DVec3, proj: &MapProjector, ea_body: f64) -> Vec<OrbitSample> {
    let view = proj.rect.expand(proj.rect.width().max(proj.rect.height()) * 0.25);
    let at = |ea: f64| OrbitSample { ea, at: proj.project(eval(ea)).map(|(p, _)| p) };
    let mut out = Vec::with_capacity(BASE_STEPS * 2);
    let step = std::f64::consts::TAU / BASE_STEPS as f64;
    let mut prev = at(ea_body);
    out.push(prev);
    for k in 1..=BASE_STEPS {
        let next = at(ea_body + step * k as f64);
        refine(&at, prev, next, 0, view, &mut out);
        out.push(next);
        prev = next;
    }
    out
}

/// Split the piece from `a` to `b` while it strays from its chord (pushing
/// every point strictly between them, in order).
fn refine(at: &dyn Fn(f64) -> OrbitSample, a: OrbitSample, b: OrbitSample, depth: u32, view: Rect, out: &mut Vec<OrbitSample>) {
    if depth >= MAX_DEPTH || out.len() >= MAX_POINTS {
        return;
    }
    let mid = at((a.ea + b.ea) * 0.5);
    let split = match (a.at, b.at, mid.at) {
        (Some(pa), Some(pb), Some(pm)) => {
            let bounds = Rect::from_two_pos(pa, pb).union(Rect::from_min_max(pm, pm));
            bounds.intersects(view) && distance_to_chord(pm, pa, pb) > TOLERANCE_PX
        }
        // A piece crossing behind the camera: split it to find where the
        // line leaves the view, a few levels deep.
        (Some(_), None, _) | (None, Some(_), _) => depth < 6,
        _ => false,
    };
    if split {
        refine(at, a, mid, depth + 1, view, out);
        out.push(mid);
        refine(at, mid, b, depth + 1, view, out);
    }
}

fn distance_to_chord(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_sq();
    if len2 <= 1e-6 {
        return (p - a).length();
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

/// The trail's brightness a fraction `behind` (0 at the body, 1 a whole
/// period on) of an orbit behind the body: full at the body, easing to the
/// floor half an orbit back, the floor after.
pub fn trail_alpha(behind: f64) -> f32 {
    let t = (1.0 - behind.clamp(0.0, 1.0) / 0.5).max(0.0) as f32;
    TRAIL_FLOOR + (1.0 - TRAIL_FLOOR) * t.powf(1.3)
}

/// How much of a whole orbit to draw (0 none, 1 all) when its semi-major
/// axis is `a_px` on a canvas `view_px` tall: gone under about 8 px across,
/// thinning from four screens wide, gone at two hundred (by then the visible
/// piece is a straight line through its own body, Earth's orbit across the
/// Earth-Moon view: the first near-space snapshot, 2026-10-21).
pub fn size_fade(a_px: f32, view_px: f32) -> f32 {
    let small = super::bodies::smoothstep(2.0, 6.0, a_px);
    let screens = a_px / view_px.max(1.0);
    let big = if screens <= 4.0 { 1.0 } else { (1.0 - (screens / 4.0).ln() / (200.0_f32 / 4.0).ln()).clamp(0.0, 1.0) };
    small * big
}

/// How much of an orbit `a_m` across (half its long axis) to draw with the
/// camera `distance_m` from what it looks at: all of it up to 300 times the
/// distance, none from 3,000 (log between). Zoomed onto the ship 1.5 km away,
/// Venus's and the Moon's orbits are seventy million and a quarter million
/// times further: lines across the sky that say nothing about what is in view
/// (the first ship-scale snapshot, 2026-10-21, was crossed by a dozen).
pub fn scale_fade(a_m: f64, distance_m: f64) -> f32 {
    let ratio = a_m / distance_m.max(1.0);
    (1.0 - (ratio / 300.0).max(1.0).ln() / 10.0_f64.ln()).clamp(0.0, 1.0) as f32
}

/// Append an anti-aliased polyline of `pts` (canvas point, brightness 0 to
/// 1), `width` px wide in `colour` (its alpha is the line's at full
/// brightness), to `mesh`. Thinner than a pixel draws a pixel wide and fainter.
pub fn add_polyline(mesh: &mut Mesh, pts: &[(Pos2, f32)], colour: Color32, width: f32) {
    if pts.len() < 2 {
        return;
    }
    let (w, a_scale) = if width < 1.0 { (1.0, width.max(0.0)) } else { (width, 1.0) };
    let hw = w * 0.5;
    const FEATHER: f32 = 0.8;
    let base = mesh.vertices.len() as u32;
    let n = pts.len();
    for i in 0..n {
        let prev = pts[i.saturating_sub(1)].0;
        let next = pts[(i + 1).min(n - 1)].0;
        let t = next - prev;
        let t = if t.length_sq() > 1e-9 { t.normalized() } else { Vec2::X };
        let nrm = Vec2::new(-t.y, t.x);
        let (p, bright) = pts[i];
        // Faded in gamma space (`bodies::faded`), so the trail's tail really
        // is a fifth as bright as its head.
        let core = super::bodies::faded(colour, bright.clamp(0.0, 1.0) * a_scale);
        let clear = Color32::TRANSPARENT;
        for (off, c) in [(hw + FEATHER, clear), (hw, core), (-hw, core), (-(hw + FEATHER), clear)] {
            mesh.vertices.push(Vertex { pos: p + nrm * off, uv: WHITE_UV, color: c });
        }
    }
    for i in 0..n as u32 - 1 {
        let (a0, b0) = (base + 4 * i, base + 4 * (i + 1));
        for s in 0..3 {
            mesh.add_triangle(a0 + s, b0 + s, b0 + s + 1);
            mesh.add_triangle(a0 + s, b0 + s + 1, a0 + s + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cosmos::{eccentric_anomaly_at, find_body, orbit_point_au, M_PER_AU};
    use crate::gui::pages::maps::camera::{MapCamera, MapScene};

    fn overview(t: f64) -> MapProjector {
        let scene = MapScene::new(None, t, false);
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0));
        MapProjector::new(MapCamera::default().pose(&scene), rect)
    }

    /// Every orbit starts and ends EXACTLY at its body: the first and last
    /// samples are the body's own place, to the millimetre, for planets,
    /// moons and a comet, on several dates.
    ///
    /// Seen red 2026-10-21 with the line started at eccentric anomaly 0
    /// (perihelion) as the old sampler did: "mercury on day 0: the line starts
    /// 115787233.2 km from the body".
    #[test]
    fn the_body_is_a_vertex_of_its_own_orbit() {
        for day in [0.0, 137.0, 3000.0] {
            let t = 843_782_400.0 + day * 86_400.0;
            let scene = MapScene::new(None, t, false);
            let proj = overview(t);
            for id in ["mercury", "mars", "moon", "io", "halley"] {
                let b = find_body(id).unwrap();
                let parent = b.parent.clone().unwrap();
                let base = scene.body_m(&parent).unwrap();
                let eval = |ea: f64| base + orbit_point_au(b, ea, t) * M_PER_AU;
                let s = sample_adaptive(&eval, &proj, eccentric_anomaly_at(b, t));
                let body = scene.body_m(id).unwrap();
                for end in [s.first().unwrap(), s.last().unwrap()] {
                    let miss = eval(end.ea).distance(body);
                    assert!(miss < 1e-3 * 1.0e3, "{id} on day {day}: the line starts {:.1} km from the body", miss / 1000.0);
                }
            }
        }
    }

    /// Zoomed in close to Earth, its orbit is sampled finely beside it (no
    /// chord strays a third of a pixel) and coarsely where it is off the
    /// canvas; at the overview it needs few points.
    ///
    /// Seen red 2026-10-21 with the splitting switched off (the 64 even steps
    /// alone, like the old 96 fixed ones): "beside Earth a chord strays 8.2 px
    /// from the orbit".
    #[test]
    fn sampling_adapts_to_the_view() {
        let t = 843_782_400.0;
        let scene = MapScene::new(None, t, false);
        let earth = find_body("earth").unwrap();
        let sun = scene.body_m("sun").unwrap();
        let eval = |ea: f64| sun + orbit_point_au(earth, ea, t) * M_PER_AU;
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0));
        let mut cam = MapCamera::default();
        cam.anchor = crate::gui::pages::maps::camera::MapAnchor::Body("earth".into());
        cam.set_distance(5.0e9);
        cam.pitch = -0.6;
        let proj = MapProjector::new(cam.pose(&scene), rect);
        let s = sample_adaptive(&eval, &proj, eccentric_anomaly_at(earth, t));
        let mut worst = 0.0_f32;
        for w in s.windows(2) {
            let (Some(a), Some(b)) = (w[0].at, w[1].at) else { continue };
            if !rect.contains(a) && !rect.contains(b) {
                continue;
            }
            let (_, mid) = (0, proj.project(eval((w[0].ea + w[1].ea) * 0.5)).map(|(p, _)| p));
            if let Some(m) = mid {
                worst = worst.max(distance_to_chord(m, a, b));
            }
        }
        assert!(worst < 1.0, "beside Earth a chord strays {worst:.1} px from the orbit");
        let few = sample_adaptive(&eval, &overview(t), eccentric_anomaly_at(earth, t)).len();
        assert!(few < 200, "at the overview Earth's orbit took {few} points");
    }

    /// The trail is brightest at the body, at the floor half an orbit back,
    /// and an orbit fades out when small and when far bigger than the screen.
    ///
    /// Seen red 2026-10-21 with the size fade's small end left out: "assertion
    /// `left == right` failed: a 2 px orbit is drawn".
    #[test]
    fn the_trail_and_the_size_fade() {
        assert!((trail_alpha(0.0) - 1.0).abs() < 1e-6);
        assert!(trail_alpha(0.1) > trail_alpha(0.3));
        assert!((trail_alpha(0.5) - TRAIL_FLOOR).abs() < 1e-6 && (trail_alpha(0.9) - TRAIL_FLOOR).abs() < 1e-6);
        assert_eq!(size_fade(1.0, 900.0), 0.0, "a 2 px orbit is drawn");
        assert_eq!(size_fade(100.0, 900.0), 1.0);
        assert!(size_fade(900.0 * 30.0, 900.0) < 0.5 && size_fade(900.0 * 250.0, 900.0) == 0.0);
    }

    /// Zoomed onto the ship, the planets' and the Moon's orbits are gone; in
    /// near space the Moon's is whole; at the overview every planet's is.
    ///
    /// Seen red 2026-10-21 with no scale fade (1.0 always): "assertion `left ==
    /// right` failed: 1.5 km from the ship the Moon's orbit is drawn at 1.00".
    #[test]
    fn orbits_far_bigger_than_the_view_fade_out() {
        let moon = scale_fade(384_400_000.0, 1_500.0);
        assert_eq!(moon, 0.0, "1.5 km from the ship the Moon's orbit is drawn at {moon:.2}");
        assert_eq!(scale_fade(1.08e11, 1_500.0), 0.0);
        assert_eq!(scale_fade(384_400_000.0, 4.0e8), 1.0);
        assert_eq!(scale_fade(30.07 * M_PER_AU, 11.0 * M_PER_AU), 1.0);
        assert!(scale_fade(1.0e12, 1.0e9) > 0.0 && scale_fade(1.0e12, 1.0e9) < 1.0);
    }
}
