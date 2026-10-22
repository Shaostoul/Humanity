//! The asteroid BELTS on the Maps page (Maps increment 2,
//! docs/design/maps-zoom-to-anything.md 5.10): "The belts are point clouds
//! sampled from the real orbit distribution, propagated only when the clock
//! moves a visible amount."
//!
//! The dust is 20,000 REAL minor planets (`data/maps/belts.json`, built by
//! `scripts/build-belts.js` from the Minor Planet Center's MPCORB; its terms
//! in docs/reference/findings/2026-10-21-mpc-orbit-data.md): the 13,000
//! largest of the main belt, 1,500 Hildas, 2,500 Jupiter trojans and 3,000 of
//! the Kuiper belt and beyond, each moved along its own osculating ellipse to
//! the page's date. So the picture is the real one: the Hildas' triangle
//! with Jupiter at a corner, the two trojan swarms 60 degrees ahead of and
//! behind Jupiter, the Kirkwood thinning, the Kuiper belt's lumpy ring. Until
//! this increment the belts were not drawn at all (and the web page drew a
//! random ring).
//!
//! Cost: propagating 20,000 orbits (Kepler's equation each) happens only when
//! the clock has moved more than `REPROPAGATE_S` (six hours: a main-belt
//! asteroid moves a third of a pixel at the overview in that time) or Diagram
//! is switched; every frame only projects the cached positions and appends one
//! small triangle each to one mesh.

use std::sync::OnceLock;

use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{Color32, Vec2};
use glam::DVec3;
use serde::Deserialize;

use crate::cosmos::{world_from_ecliptic, M_PER_AU};

use super::camera::{MapProjector, MapScene};

/// How far the clock may move before the dust is moved with it, seconds.
pub const REPROPAGATE_S: f64 = 6.0 * 3600.0;
/// The Gaussian gravitational constant, radians a day for a = 1 AU: the Sun's
/// mean motion law the MPC's own elements use (n = k / a^1.5).
const GAUSS_K: f64 = 0.017_202_098_95;
/// J2000.0 as a Julian Date (the page's clock counts seconds from it).
const JD_J2000: f64 = 2_451_545.0;

/// One class of the belts file (main belt, Hildas, trojans, distant).
#[derive(Debug, Clone, Deserialize)]
pub struct BeltClass {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub about: String,
    /// "rock" or "ice": which of the theme's two dust colours it is drawn in.
    #[serde(default)]
    pub tone: String,
    #[serde(default)]
    pub count: usize,
}

#[derive(Debug, Deserialize)]
struct BeltsFile {
    format: String,
    credit: String,
    classes: Vec<BeltClass>,
    rows: Vec<(String, usize, f64, f64, f64, f64, f64, f64, f64, f64, String, String)>,
}

/// One minor planet's orbit, ready to move: its perifocal axes in the
/// world's axes, its size and shape, and its mean anomaly's clock.
#[derive(Debug, Clone)]
struct BeltOrbit {
    class: u8,
    /// How bright it is drawn, 0 to 1 (from its absolute magnitude H).
    weight: f32,
    p: DVec3,
    q: DVec3,
    a: f64,
    b: f64,
    e: f64,
    m0: f64,
    n_per_s: f64,
    epoch_s: f64,
}

/// The belts file, read once.
pub struct BeltSet {
    pub classes: Vec<BeltClass>,
    pub credit: String,
    orbits: Vec<BeltOrbit>,
    /// Each class's median semi-major axis, AU: how big the class looks on
    /// screen, which thins its dust when it is small (`add_dust`).
    class_a_au: Vec<f64>,
}

impl BeltSet {
    pub fn len(&self) -> usize {
        self.orbits.len()
    }

    pub fn is_empty(&self) -> bool {
        self.orbits.is_empty()
    }
}

/// Parse a belts file (`format` humanityos-belts-1).
fn parse(text: &str) -> Result<BeltSet, String> {
    let f: BeltsFile = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if f.format != "humanityos-belts-1" {
        return Err(format!("unknown belts format {:?}", f.format));
    }
    let mut orbits = Vec::with_capacity(f.rows.len());
    for (desig, class, h, epoch_jd, a, e, inc, node, peri, m, _ref, _computer) in &f.rows {
        if *class >= f.classes.len() || !(*a > 0.0) || !(0.0..1.0).contains(e) {
            return Err(format!("row {desig}: class {class}, a {a}, e {e} is not an orbit this page can draw"));
        }
        // The perifocal axes in the ecliptic frame of J2000 (the 3-1-3
        // rotation node, inclination, argument of perihelion, as in
        // `cosmos::perifocal_axes_ecliptic`), then into the world.
        let (so, co) = node.to_radians().sin_cos();
        let (si, ci) = inc.to_radians().sin_cos();
        let (sw, cw) = peri.to_radians().sin_cos();
        let p = DVec3::new(co * cw - so * sw * ci, so * cw + co * sw * ci, sw * si);
        let q = DVec3::new(-co * sw - so * cw * ci, -so * sw + co * cw * ci, cw * si);
        orbits.push(BeltOrbit {
            class: *class as u8,
            weight: (0.55 + 0.45 * ((15.0 - h) / 7.0).clamp(0.0, 1.0)) as f32,
            p: world_from_ecliptic(p),
            q: world_from_ecliptic(q),
            a: *a,
            b: a * (1.0 - e * e).sqrt(),
            e: *e,
            m0: m.to_radians(),
            n_per_s: GAUSS_K / a.powf(1.5) / 86_400.0,
            epoch_s: (epoch_jd - JD_J2000) * 86_400.0,
        });
    }
    let class_a_au = (0..f.classes.len())
        .map(|c| {
            let mut a: Vec<f64> = orbits.iter().filter(|o| o.class as usize == c).map(|o| o.a).collect();
            a.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
            a.get(a.len() / 2).copied().unwrap_or(3.0)
        })
        .collect();
    Ok(BeltSet { classes: f.classes, credit: f.credit, orbits, class_a_au })
}

/// The belts, from the data folder or the copy built into the exe; empty
/// (and said so in the log) when neither can be read.
pub fn belt_set() -> &'static BeltSet {
    static B: OnceLock<BeltSet> = OnceLock::new();
    B.get_or_init(|| match crate::embedded_data::load_text_or_embedded(&crate::data_dir(), "maps/belts.json", parse) {
        Ok(set) => {
            log::info!("Maps: {} belt orbits ({})", set.len(), set.credit);
            set
        }
        Err(e) => {
            log::warn!("Maps: {e}; the belts are left out");
            BeltSet { classes: Vec::new(), credit: String::new(), orbits: Vec::new(), class_a_au: Vec::new() }
        }
    })
}

/// The dust where the last propagation put it (`MapsState::belts`).
#[derive(Debug, Clone, Default)]
pub struct BeltCache {
    /// The clock the positions are for, and whether they are the Diagram's.
    t: f64,
    diagram: bool,
    /// Heliocentric AU, already through the Diagram when it is on.
    pos_au: Vec<DVec3>,
    /// How many propagations so far (tests and the report).
    pub propagations: u64,
    /// The dust's last mesh and what it was drawn for, so a map that has not
    /// moved reuses it: rebuilding 17,000 points (a projection and a faded
    /// colour each) cost 3.5 ms a frame at the overview at rest, against the
    /// symbols' 1 ms budget (the release sweep of 2026-10-21).
    mesh: std::sync::Arc<Mesh>,
    mesh_for: Option<DustKey>,
    mesh_drawn: u32,
    /// How many times the dust's mesh was built (tests and the report).
    pub mesh_builds: u64,
}

/// What the dust's mesh depends on (`BeltCache::mesh`): the camera, the canvas,
/// the propagated positions, Diagram, the two colours, and the map frame's
/// origin (Earth's display centre, which moves with the clock).
#[derive(Debug, Clone, Copy, PartialEq)]
struct DustKey {
    pose: super::camera::MapPose,
    rect: egui::Rect,
    propagations: u64,
    diagram: bool,
    rock: Color32,
    ice: Color32,
    earth_au: DVec3,
}

impl DustKey {
    /// The same picture, to a quarter of a pixel. The map frame is centred on
    /// Earth, so with a live clock a camera held over the Sun moves in it every
    /// frame, and so does every point (by minus Earth's step): what reaches the
    /// screen is the points' shift relative to the eye, `-(d_earth + d_eye)`,
    /// and the camera's turn. Compared exactly, the pose never matched twice and
    /// the dust was rebuilt every frame (3.7 ms at the overview, 2026-10-21).
    fn same_picture(&self, other: &DustKey, metres_per_px: f64) -> bool {
        let (a, b) = (&self.pose, &other.pose);
        let mpp = metres_per_px.max(f64::MIN_POSITIVE);
        let shift_px = ((self.earth_au - other.earth_au) * M_PER_AU + (a.eye - b.eye)).length() / mpp;
        let px_per_rad = f64::from(self.rect.height()) / a.fov_y.max(1e-9);
        let turn_px = (a.forward - b.forward).length().max((a.up - b.up).length()) * px_per_rad;
        self.rect == other.rect
            && self.propagations == other.propagations
            && self.diagram == other.diagram
            && self.rock == other.rock
            && self.ice == other.ice
            && a.fov_y == b.fov_y
            && (a.distance - b.distance).abs() <= a.distance.abs() * 1e-9
            && shift_px < 0.25
            && turn_px < 0.25
    }
}

impl BeltCache {
    /// Move every orbit to `t` if the clock has moved more than
    /// `REPROPAGATE_S` since the last time, or Diagram was switched.
    pub fn update(&mut self, set: &BeltSet, t: f64, diagram: bool) {
        let fresh = self.pos_au.len() == set.orbits.len() && self.diagram == diagram && (t - self.t).abs() <= REPROPAGATE_S;
        if fresh {
            return;
        }
        self.pos_au.clear();
        self.pos_au.extend(set.orbits.iter().map(|o| {
            let p = position_au(o, t);
            if diagram { super::diagram::helio_au(p) } else { p }
        }));
        self.t = t;
        self.diagram = diagram;
        self.propagations += 1;
    }
}

fn position_au(o: &BeltOrbit, t: f64) -> DVec3 {
    let m = (o.m0 + o.n_per_s * (t - o.epoch_s)).rem_euclid(std::f64::consts::TAU);
    let ea = crate::cosmos::kepler_solve(m, o.e);
    o.p * (o.a * (ea.cos() - o.e)) + o.q * (o.b * ea.sin())
}

/// Append the dust to `mesh` (tests; the page paints `dust_mesh`'s shared
/// mesh without copying it). Returns how many points were drawn.
pub fn add_dust(mesh: &mut Mesh, set: &BeltSet, cache: &mut BeltCache, proj: &MapProjector, scene: &MapScene, rock: Color32, ice: Color32) -> u32 {
    let (dust, drawn) = dust_mesh(set, cache, proj, scene, rock, ice);
    mesh.append_ref(&dust);
    drawn
}

/// The dust: one small triangle per point on the canvas, in the theme's rock
/// or ice colour, and how many were drawn. The mesh is built again only when
/// the picture would change (`DustKey`); otherwise the last one is handed back
/// as it is (egui holds a mesh shape by `Arc`, so nothing is copied).
pub fn dust_mesh(set: &BeltSet, cache: &mut BeltCache, proj: &MapProjector, scene: &MapScene, rock: Color32, ice: Color32) -> (std::sync::Arc<Mesh>, u32) {
    if cache.pos_au.len() != set.orbits.len() {
        return (std::sync::Arc::default(), 0);
    }
    // The map frame is metres from Earth's (display) centre: one offset for
    // every point, then each is projected in f64.
    let earth_au = scene.earth_display_au();
    let key = DustKey { pose: proj.pose, rect: proj.rect, propagations: cache.propagations, diagram: scene.diagram, rock, ice, earth_au };
    if cache.mesh_for.is_some_and(|k| k.same_picture(&key, proj.metres_per_px())) {
        return (cache.mesh.clone(), cache.mesh_drawn);
    }
    let mut built = Mesh::default();
    let drawn = build_dust(&mut built, set, cache, proj, scene, rock, ice, earth_au);
    cache.mesh = std::sync::Arc::new(built);
    cache.mesh_for = Some(key);
    cache.mesh_drawn = drawn;
    cache.mesh_builds += 1;
    (cache.mesh.clone(), drawn)
}

#[allow(clippy::too_many_arguments)]
fn build_dust(mesh: &mut Mesh, set: &BeltSet, cache: &BeltCache, proj: &MapProjector, scene: &MapScene, rock: Color32, ice: Color32, earth_au: DVec3) -> u32 {
    let tones: Vec<Color32> = set.classes.iter().map(|c| if c.tone == "ice" { ice } else { rock }).collect();
    // A class seen small is thinned, so its points do not pile up into a
    // solid blob over the inner planets (at the 60 AU overview the main belt
    // is 30 px in radius and its points lay six deep): the points per pixel
    // grow as the square of the zoom out, so each is drawn fainter by the
    // same square, full strength from 150 px of median orbit radius on screen.
    let thin: Vec<f32> = set
        .class_a_au
        .iter()
        .map(|&a| {
            let a_disp = if scene.diagram { super::diagram::radial_au(a) } else { a };
            let k = (a_disp * M_PER_AU / proj.metres_per_px()) as f32 / 150.0;
            (k * k).clamp(0.02, 1.0)
        })
        .collect();
    let view = proj.rect;
    const R: f32 = 0.9;
    let corners = [Vec2::new(0.0, -R * 1.15), Vec2::new(R, R * 0.6), Vec2::new(-R, R * 0.6)];
    mesh.reserve_triangles(cache.pos_au.len());
    mesh.reserve_vertices(cache.pos_au.len() * 3);
    let mut drawn = 0u32;
    for (o, p) in set.orbits.iter().zip(&cache.pos_au) {
        let Some((at, _)) = proj.project((*p - earth_au) * M_PER_AU) else { continue };
        if !view.contains(at) {
            continue;
        }
        let c = super::bodies::faded(tones[o.class as usize], o.weight * thin[o.class as usize]);
        let i = mesh.vertices.len() as u32;
        for k in corners {
            mesh.vertices.push(Vertex { pos: at + k, uv: WHITE_UV, color: c });
        }
        mesh.add_triangle(i, i + 1, i + 2);
        drawn += 1;
    }
    drawn
}

#[cfg(test)]
#[path = "belts_tests.rs"]
mod tests;
