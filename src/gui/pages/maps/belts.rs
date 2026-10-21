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

/// Append the dust to `mesh`: one small triangle per point on the canvas, in
/// the theme's rock or ice colour. Returns how many were drawn.
pub fn add_dust(mesh: &mut Mesh, set: &BeltSet, cache: &BeltCache, proj: &MapProjector, scene: &MapScene, rock: Color32, ice: Color32) -> u32 {
    if cache.pos_au.len() != set.orbits.len() {
        return 0;
    }
    // The map frame is metres from Earth's (display) centre: one offset for
    // every point, then each is projected in f64.
    let earth_au = scene.earth_display_au();
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
mod tests {
    use super::*;

    /// The shipped belts are real: 20,000 orbits in four classes, with the
    /// MPC's credit and its header shipped beside them; the main belt's
    /// orbits lie between Mars and Jupiter, the trojans at Jupiter's distance,
    /// the Kuiper belt beyond Neptune.
    ///
    /// Seen red 2026-10-21 with the header file left out of the data folder:
    /// "data/maps/MPCORB-header.txt does not ship beside the belts".
    #[test]
    fn the_shipped_belts_are_the_real_minor_planets_with_their_credit() {
        let set = belt_set();
        assert!(set.len() >= 19_000, "only {} belt orbits", set.len());
        assert!(set.credit.contains("Minor Planet Center"), "no MPC credit: {:?}", set.credit);
        let header = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/data/maps/MPCORB-header.txt"));
        let header = header.expect("data/maps/MPCORB-header.txt does not ship beside the belts");
        assert!(header.contains("Software programs may include this datafile"), "the header is not MPCORB's");
        let class = |id: &str| set.classes.iter().position(|c| c.id == id).unwrap() as u8;
        let a_of = |c: u8| set.orbits.iter().filter(|o| o.class == c).map(|o| o.a).collect::<Vec<_>>();
        let median = |mut v: Vec<f64>| {
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v[v.len() / 2]
        };
        let (main, trojan, distant) = (median(a_of(class("main"))), median(a_of(class("trojan"))), median(a_of(class("distant"))));
        assert!((2.2..3.3).contains(&main), "the main belt's median orbit is {main:.2} AU");
        assert!((5.0..5.4).contains(&trojan), "the trojans' median orbit is {trojan:.2} AU");
        assert!(distant > 30.0, "the distant objects' median orbit is {distant:.1} AU");
    }

    /// The trojans gather at Jupiter's L4 and L5: on the page's date, most of
    /// them stand 40 to 80 degrees from Jupiter round the Sun, the two swarms
    /// either side of it.
    ///
    /// Seen red 2026-10-21 with each orbit's mean anomaly left at its epoch
    /// value plus a full extra half turn (`m0 + PI`): "only 1% of the trojans
    /// are 40 to 80 degrees from Jupiter".
    #[test]
    fn the_trojans_swarm_sixty_degrees_from_jupiter() {
        let set = belt_set();
        let t = 843_782_400.0;
        let mut cache = BeltCache::default();
        cache.update(set, t, false);
        let jupiter = crate::cosmos::body_world_position_3d_au(crate::cosmos::find_body("jupiter").unwrap(), t);
        let tro = set.classes.iter().position(|c| c.id == "trojan").unwrap() as u8;
        let flat = |v: DVec3| DVec3::new(v.x, 0.0, v.z).normalize();
        let (mut near60, mut all, mut ahead, mut behind) = (0, 0, 0, 0);
        for (o, p) in set.orbits.iter().zip(&cache.pos_au) {
            if o.class != tro {
                continue;
            }
            all += 1;
            let ang = flat(*p).dot(flat(jupiter)).clamp(-1.0, 1.0).acos().to_degrees();
            if (40.0..80.0).contains(&ang) {
                near60 += 1;
                // Ahead of Jupiter: the turn from Jupiter to it is the way the
                // planets go round, counter-clockwise seen from ecliptic north (+Y).
                if flat(jupiter).cross(flat(*p)).y > 0.0 { ahead += 1 } else { behind += 1 }
            }
        }
        let share = near60 as f64 / all as f64;
        assert!(share > 0.6, "only {:.0}% of the trojans are 40 to 80 degrees from Jupiter", share * 100.0);
        assert!(ahead > all / 5 && behind > all / 5, "the two swarms: {ahead} ahead, {behind} behind of {all}");
    }

    /// Seen from far out, the inner belts thin with the square of the zoom so
    /// their points never pile up into a solid blob: at the page's 60 AU
    /// overview no main-belt point adds more than 3% of its colour, while at
    /// 11 AU (the belts 165 px across) they draw at full strength.
    ///
    /// Seen red 2026-10-21 with the dust premultiplied by egui's
    /// `from_rgba_unmultiplied` (in linear light): "at 60 AU a main-belt point
    /// adds 33 of 255 of its colour, over 8".
    #[test]
    fn far_out_the_belts_thin_instead_of_piling_up() {
        use crate::gui::pages::maps::camera::{MapCamera, MapProjector, MapScene};
        let set = belt_set();
        let t = 843_782_400.0;
        let mut cache = BeltCache::default();
        cache.update(set, t, false);
        let scene = MapScene::new(None, t, false);
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(755.0, 745.0));
        let rock = Color32::from_rgba_unmultiplied(209, 189, 158, 140); // theme-exempt: fixed test colours, the theme token values
        let ice = Color32::from_rgba_unmultiplied(158, 189, 224, 140); // theme-exempt: fixed test colours, the theme token values
        let main = set.classes.iter().position(|c| c.id == "main").unwrap() as u8;
        // The brightest premultiplied red a main-belt point adds, at a view.
        let brightest = |cam: MapCamera| {
            let proj = MapProjector::new(cam.pose(&scene), rect);
            let mut mesh = Mesh::default();
            add_dust(&mut mesh, set, &cache, &proj, &scene, rock, ice);
            let mut i = 0usize;
            let mut best = 0u8;
            for (o, p) in set.orbits.iter().zip(&cache.pos_au) {
                let Some((at, _)) = proj.project((*p - scene.earth_display_au()) * M_PER_AU) else { continue };
                if !rect.contains(at) {
                    continue;
                }
                if o.class == main {
                    best = best.max(mesh.vertices[i * 3].color.r());
                }
                i += 1;
            }
            best
        };
        let far = brightest(MapCamera::default());
        assert!(far <= 8, "at 60 AU a main-belt point adds {far} of 255 of its colour, over 8");
        let near = brightest(MapCamera::overview(11.0 * M_PER_AU));
        assert!(near >= 60, "at 11 AU a main-belt point adds only {near} of 255");
    }

    /// The dust moves only when the clock has moved a visible amount (or
    /// Diagram is switched), so a live map does not solve 20,000 Kepler
    /// equations every frame.
    ///
    /// Seen red 2026-10-21 with the cache never reused: "assertion `left ==
    /// right` failed: re-propagated within 21600 s".
    #[test]
    fn the_dust_is_propagated_only_when_the_clock_moves_far_enough() {
        let set = belt_set();
        let mut cache = BeltCache::default();
        cache.update(set, 0.0, false);
        cache.update(set, 60.0, false);
        cache.update(set, REPROPAGATE_S, false);
        assert_eq!(cache.propagations, 1, "re-propagated within {REPROPAGATE_S} s");
        cache.update(set, REPROPAGATE_S + 1.0, false);
        assert_eq!(cache.propagations, 2);
        cache.update(set, REPROPAGATE_S + 1.0, true);
        assert_eq!(cache.propagations, 3, "switching Diagram did not move the dust");
    }
}
