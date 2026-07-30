//! Procedural planet-surface trees (v0.1066).
//!
//! Operator ask: "Can we see about adding a variety of trees? I'd love to see
//! cherry blossoms in Japan near mount fuji."
//!
//! Before this module the whole planet had exactly TWO tree species, chosen by
//! a hardcoded bit: `species_fir = ((r5 >> 9) & 1) == 0`, fir at 22 m and pine
//! at 16 m, everywhere from the tropics to the treeline. Both were photoscanned
//! glTF, and the release bundle does not ship `assets/models/`, so a downloaded
//! build had no near trees at all.
//!
//! Now `data/vegetation/trees.ron` is the species list (infinite-of-X: adding a
//! tree is a data row), and any species with an empty `model` field is built
//! HERE out of numbers instead of art. Procedural species therefore ship.
//!
//! Geometry follows the same rule the crop generator uses: flat-shaded
//! triangles with per-face colour packed into the UV, plus the organ tag in
//! spare bits, so trees render through material type 20 and pick up the
//! close-range leaf shading (venation, micro-relief, waxy cuticle, backlit
//! transmission) for free.
//!
//! ARCHITECTURE is annual-increment recursion, the same shape the design doc
//! (`docs/design/procedural-plants.md`) settles on: a limb extends, then spawns
//! children with less vigour, and leaves live on the OUTERMOST twigs only, so
//! the canopy is a shell rather than a solid block of foliage.

use super::plant_mesh::{Organ, PlantMeshBuilder};
use serde::Deserialize;

// ── Species data (deserialized from data/vegetation/trees.ron) ───────────

#[derive(Debug, Clone, Deserialize)]
pub struct TreeDef {
    pub id: String,
    pub display: String,
    /// glTF base name under `assets/models/plants/`. EMPTY = procedural.
    pub model: String,
    pub variants: u32,
    /// Tile index into the baked billboard atlas (model species only).
    pub sprite_tile: u8,
    pub height_m: f32,
    pub height_jitter: f32,
    pub weight: f32,
    pub lat_min_deg: f32,
    pub lat_max_deg: f32,
    pub elev_min_m: f32,
    pub elev_max_m: f32,
    /// Region lock. `region_radius_km` of 0 disables it (species is global).
    pub region_lat_deg: f32,
    pub region_lon_deg: f32,
    pub region_radius_km: f32,
    /// conifer | broadleaf | umbrella | palm. Unknown falls back to broadleaf.
    pub form: String,
    pub trunk_color: [f32; 3],
    pub leaf_color: [f32; 3],
    pub blossom_color: [f32; 3],
    /// Fraction of leaf clusters replaced by blossom clusters (0 = never).
    pub blossom_frac: f32,
}

impl TreeDef {
    pub fn is_procedural(&self) -> bool {
        self.model.is_empty()
    }

    /// Unit direction of this species' region centre, or None when global.
    fn region_dir(&self) -> Option<[f64; 3]> {
        if self.region_radius_km <= 0.0 {
            return None;
        }
        let lat = (self.region_lat_deg as f64).to_radians();
        let lon = (self.region_lon_deg as f64).to_radians();
        let cl = lat.cos();
        // Matches the spawn site's dir construction in planet_chunks.rs:
        // (cos(lat)cos(lon), sin(lat), -cos(lat)sin(lon)).
        Some([cl * lon.cos(), lat.sin(), -cl * lon.sin()])
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TreeRegistry {
    pub trees: Vec<TreeDef>,
}

impl TreeRegistry {
    pub fn from_ron(text: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(text)
    }

    pub fn get(&self, idx: usize) -> Option<&TreeDef> {
        self.trees.get(idx)
    }

    pub fn len(&self) -> usize {
        self.trees.len()
    }

    pub fn is_empty(&self) -> bool {
        self.trees.is_empty()
    }

    /// Index of a species id, for callers that need a stable handle.
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.trees.iter().position(|t| t.id == id)
    }

    /// Pick a species for a spawn cell. `dir` is the outward unit direction of
    /// the cell (already computed by the caller), `elev_m` its height above sea
    /// level, `planet_radius_m` the body radius so a region radius in km means
    /// the same thing on any planet, and `roll` a deterministic random word
    /// from the cell stream.
    ///
    /// Returns the registry index, so the picked species survives into
    /// `NearTree.species` and the caller can look up geometry later.
    ///
    /// Weighting is intentional: a region-locked species (sakura) carries a
    /// high weight so that inside its region it DOMINATES rather than showing
    /// up as one pink tree in a thousand firs.
    pub fn pick(
        &self,
        dir: [f64; 3],
        lat_deg: f32,
        elev_m: f32,
        planet_radius_m: f64,
        roll: u32,
    ) -> Option<usize> {
        let mut total = 0.0f32;
        // Two passes rather than an allocation: this runs per candidate cell.
        for t in &self.trees {
            if Self::gates_pass(t, dir, lat_deg, elev_m, planet_radius_m) {
                total += t.weight.max(0.0);
            }
        }
        if total <= 0.0 {
            return None;
        }
        let mut pick = (roll % 100_000) as f32 / 100_000.0 * total;
        for (i, t) in self.trees.iter().enumerate() {
            if !Self::gates_pass(t, dir, lat_deg, elev_m, planet_radius_m) {
                continue;
            }
            pick -= t.weight.max(0.0);
            if pick <= 0.0 {
                return Some(i);
            }
        }
        // Float drift only; fall back to the last species that passed.
        self.trees
            .iter()
            .enumerate()
            .filter(|(_, t)| Self::gates_pass(t, dir, lat_deg, elev_m, planet_radius_m))
            .map(|(i, _)| i)
            .next_back()
    }

    fn gates_pass(
        t: &TreeDef,
        dir: [f64; 3],
        lat_deg: f32,
        elev_m: f32,
        planet_radius_m: f64,
    ) -> bool {
        if lat_deg < t.lat_min_deg || lat_deg > t.lat_max_deg {
            return false;
        }
        if elev_m < t.elev_min_m || elev_m > t.elev_max_m {
            return false;
        }
        if let Some(c) = t.region_dir() {
            // Great-circle angle between the cell and the region centre.
            let d = (dir[0] * c[0] + dir[1] * c[1] + dir[2] * c[2]).clamp(-1.0, 1.0);
            let ang = d.acos();
            let max_ang = (t.region_radius_km as f64 * 1000.0) / planet_radius_m.max(1.0);
            if ang > max_ang {
                return false;
            }
        }
        true
    }
}

/// The shipped species list, compiled in so a build with no `data/` directory
/// still grows trees. The on-disk copy wins when present, which keeps the file
/// editable in a dev checkout.
const EMBEDDED_TREES: &str = include_str!("../../data/vegetation/trees.ron");

static REGISTRY: std::sync::OnceLock<TreeRegistry> = std::sync::OnceLock::new();

/// Process-wide species registry.
///
/// A OnceLock rather than a DataStore entry because the two species-picking
/// sites live in free functions in `terrain::planet_chunks` (the card bake and
/// the near-model mirror) with different signatures, and those two MUST agree
/// exactly or a tree changes species as you walk toward it.
pub fn registry() -> &'static TreeRegistry {
    REGISTRY.get_or_init(|| {
        let disk = std::fs::read_to_string("data/vegetation/trees.ron")
            .ok()
            .and_then(|t| TreeRegistry::from_ron(&t).ok());
        match disk {
            Some(r) if !r.is_empty() => r,
            _ => TreeRegistry::from_ron(EMBEDDED_TREES).unwrap_or_default(),
        }
    })
}

// ── Deterministic RNG (xorshift64*, same shape as plant_mesh) ────────────

struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).max(1))
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (self.next() >> 40) as f32 / ((1u64 << 24) as f32) * (hi - lo)
    }
}

// ── vector helpers ───────────────────────────────────────────────────────

fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
    [v[0] / l, v[1] / l, v[2] / l]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn add(a: [f32; 3], b: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] + b[0] * s, a[1] + b[1] * s, a[2] + b[2] * s]
}

/// Rotate `v` away from `axis` by `deg`, around a stable perpendicular chosen
/// from `phase` so successive children fan out instead of stacking.
fn tilt(v: [f32; 3], deg: f32, phase: f32) -> [f32; 3] {
    let up = if v[1].abs() > 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let t1 = norm(cross(v, up));
    let t2 = cross(v, t1);
    let side = norm([
        t1[0] * phase.cos() + t2[0] * phase.sin(),
        t1[1] * phase.cos() + t2[1] * phase.sin(),
        t1[2] * phase.cos() + t2[2] * phase.sin(),
    ]);
    let a = deg.to_radians();
    norm([
        v[0] * a.cos() + side[0] * a.sin(),
        v[1] * a.cos() + side[1] * a.sin(),
        v[2] * a.cos() + side[2] * a.sin(),
    ])
}

// ── Geometry ─────────────────────────────────────────────────────────────

/// Triangle ceiling per tree. Every triangle costs 108 bytes (faces are
/// unshared, the flat-shading contract), but a mesh is built ONCE PER
/// (species, variant) and then instanced by transform, so the resident cost is
/// ~18 meshes, not one per tree on screen.
///
/// The ceiling is a backstop, not a target: when it used to bite, it stopped
/// whole subtrees mid-recursion and the first branches ate the entire budget
/// while the rest of the crown came out bare. Branch tube cost is kept low
/// (see `sides_for`) precisely so foliage, not scaffolding, fills this.
const MAX_TRIS: usize = 6800;

/// Radial segments for a limb at `depth`. A twig does not need the 8 sides a
/// trunk does, and halving them here is what buys the crown its leaves.
fn sides_for(depth: u32) -> u32 {
    match depth {
        0 => 6,
        1 => 5,
        _ => 4,
    }
}

/// Build one tree into `b`, centred on the origin with +Y up.
///
/// `height_m` is the FINAL height (the caller already applied per-instance
/// jitter), so the same species reads as a stand of different-aged trees.
pub fn build_tree(b: &mut PlantMeshBuilder, def: &TreeDef, height_m: f32, seed: u32) {
    let mut rng = Rng::new(seed as u64 ^ 0x7ee_5eed);
    let h = height_m.max(0.5);
    match def.form.as_str() {
        "conifer" => conifer(b, def, h, &mut rng),
        "umbrella" => umbrella(b, def, h, &mut rng),
        "palm" => palm(b, def, h, &mut rng),
        // Unknown forms fall back to broadleaf so a new data row always renders.
        _ => broadleaf(b, def, h, &mut rng),
    }
}

/// A leaf cluster: several blades fanned around a twig tip. Individual leaves
/// on an 18 m tree would be sub-pixel and cost thousands of triangles, so one
/// "leaf" here stands for a clump of foliage, which is what every foliage
/// renderer does at this scale.
fn leaf_cluster(
    b: &mut PlantMeshBuilder,
    at: [f32; 3],
    dir: [f32; 3],
    size: f32,
    color: [f32; 3],
    n: u32,
    rng: &mut Rng,
) {
    for k in 0..n {
        let phase = k as f32 * 2.399_963 + rng.range(-0.4, 0.4);
        let d = tilt(dir, rng.range(38.0, 82.0), phase);
        // Blades hang slightly: gravity plus the weight of the clump.
        let d = norm([d[0], d[1] - 0.35, d[2]]);
        b.leaf(at, d, size, size * 0.62, 0.5, color);
    }
}

/// Recursive limb. Emits a tapered segment, then either children or foliage.
#[allow(clippy::too_many_arguments)]
fn limb(
    b: &mut PlantMeshBuilder,
    def: &TreeDef,
    from: [f32; 3],
    dir: [f32; 3],
    len: f32,
    r0: f32,
    depth: u32,
    max_depth: u32,
    leaf_size: f32,
    rng: &mut Rng,
) {
    if b.indices.len() / 3 > MAX_TRIS || len < 0.05 {
        return;
    }
    let r1 = r0 * 0.68;
    let to = add(from, dir, len);
    b.tube(from, to, r0, r1, sides_for(depth), def.trunk_color);

    // Foliage on the outer TWO generations, not just the tips: one generation
    // of leaf clumps leaves a crown you can see straight through.
    if depth + 1 >= max_depth {
        let blossom = def.blossom_frac > 0.0 && rng.range(0.0, 1.0) < def.blossom_frac;
        let color = if blossom { def.blossom_color } else { def.leaf_color };
        let tip = depth >= max_depth;
        let at = if tip { to } else { add(from, dir, len * 0.72) };
        let size = if tip { leaf_size } else { leaf_size * 0.78 };
        leaf_cluster(b, at, dir, size, color, if tip { 6 } else { 3 }, rng);
        if tip {
            return;
        }
    }

    // 2-3 children, fanned by the golden angle so they do not stack, with a
    // dominant leader early on (apical dominance) that eases off with depth.
    let n = if rng.range(0.0, 1.0) < 0.45 { 3 } else { 2 };
    for k in 0..n {
        let phase = k as f32 * 2.399_963 + rng.range(-0.5, 0.5) + depth as f32;
        let spread = rng.range(22.0, 42.0) - if depth == 0 { 6.0 } else { 0.0 };
        let d = tilt(dir, spread, phase);
        // Phototropism: every generation bends back toward vertical, which is
        // what rounds a crown instead of leaving it a flat fan.
        let d = norm([d[0], d[1] + 0.22, d[2]]);
        // Twigs shorten faster than limbs, so foliage clumps sit close together
        // instead of leaving long bare runs between them.
        let child_len = len * if depth + 2 >= max_depth { rng.range(0.42, 0.56) } else { rng.range(0.62, 0.78) };
        limb(b, def, to, d, child_len, r1, depth + 1, max_depth, leaf_size * 0.86, rng);
    }
}

fn broadleaf(b: &mut PlantMeshBuilder, def: &TreeDef, h: f32, rng: &mut Rng) {
    // A clear bole, then the crown. Cherry and maple branch low; oak higher.
    let bole = h * rng.range(0.26, 0.36);
    let r_base = h * 0.030;
    let lean = norm([rng.range(-0.06, 0.06), 1.0, rng.range(-0.06, 0.06)]);
    let top = add([0.0, 0.0, 0.0], lean, bole);
    b.tube([0.0, 0.0, 0.0], top, r_base, r_base * 0.74, 8, def.trunk_color);
    // 3 primary limbs off the bole top. Three rather than four: each primary
    // costs a whole subtree, and the budget buys more by spending it on
    // foliage than on a fourth scaffold.
    let n = 3;
    // Clumps are ~10% of tree height. Individual leaves at true scale would be
    // sub-pixel and cost thousands of triangles, so one blade here stands for
    // a clump of foliage, which is what every foliage renderer does.
    let leaf_size = h * 0.105;
    for k in 0..n {
        let phase = k as f32 * 2.399_963 + rng.range(-0.3, 0.3);
        let d = tilt(lean, rng.range(26.0, 46.0), phase);
        limb(b, def, top, d, (h - bole) * rng.range(0.40, 0.52), r_base * 0.74, 0, 3, leaf_size, rng);
    }
}

fn conifer(b: &mut PlantMeshBuilder, def: &TreeDef, h: f32, rng: &mut Rng) {
    // A single straight leader with whorls of short, steeply drooping branches
    // that shorten toward the top: the classic conical silhouette.
    let r_base = h * 0.022;
    let top = [0.0, h, 0.0];
    b.tube([0.0, 0.0, 0.0], top, r_base, r_base * 0.16, 8, def.trunk_color);
    let whorls = 9;
    let leaf_size = h * 0.055;
    for w in 0..whorls {
        let f = 0.22 + 0.74 * (w as f32 / (whorls - 1) as f32);
        let y = h * f;
        // Branch length tapers linearly to the apex.
        let blen = h * 0.30 * (1.0 - f) + h * 0.03;
        let per = 5;
        for k in 0..per {
            let phase = (w * per + k) as f32 * 2.399_963;
            let d = tilt([0.0, 1.0, 0.0], rng.range(74.0, 96.0), phase);
            let d = norm([d[0], d[1] - 0.30, d[2]]);
            let tip = add([0.0, y, 0.0], d, blen);
            b.tube([0.0, y, 0.0], tip, r_base * 0.22, r_base * 0.07, 4, def.trunk_color);
            leaf_cluster(b, tip, d, leaf_size, def.leaf_color, 3, rng);
            // A second clump midway keeps the branch from reading as a bare stick.
            let mid = add([0.0, y, 0.0], d, blen * 0.55);
            leaf_cluster(b, mid, d, leaf_size * 0.8, def.leaf_color, 2, rng);
        }
    }
}

fn umbrella(b: &mut PlantMeshBuilder, def: &TreeDef, h: f32, rng: &mut Rng) {
    // Acacia: a tall bare bole, then limbs that flatten hard into a wide,
    // level crown. The giveaway is that the crown is WIDER than the tree is
    // tall and its underside is flat.
    let bole = h * rng.range(0.52, 0.62);
    let r_base = h * 0.034;
    let top = [0.0, bole, 0.0];
    b.tube([0.0, 0.0, 0.0], top, r_base, r_base * 0.66, 8, def.trunk_color);
    let n = 5;
    let leaf_size = h * 0.085;
    for k in 0..n {
        let phase = k as f32 * 2.399_963 + rng.range(-0.3, 0.3);
        // Steeply out, barely up.
        let d = tilt([0.0, 1.0, 0.0], rng.range(58.0, 74.0), phase);
        let seg = h * rng.range(0.30, 0.40);
        let mid = add(top, d, seg);
        b.tube(top, mid, r_base * 0.66, r_base * 0.34, 5, def.trunk_color);
        // The crown layer: near-horizontal fans of foliage.
        for j in 0..3 {
            let p2 = phase + j as f32 * 1.9;
            let d2 = tilt([0.0, 1.0, 0.0], rng.range(80.0, 94.0), p2);
            let tip = add(mid, d2, h * rng.range(0.16, 0.26));
            b.tube(mid, tip, r_base * 0.30, r_base * 0.12, 4, def.trunk_color);
            leaf_cluster(b, tip, [0.0, 1.0, 0.0], leaf_size, def.leaf_color, 4, rng);
        }
    }
}

fn palm(b: &mut PlantMeshBuilder, def: &TreeDef, h: f32, rng: &mut Rng) {
    // No branches at all and no secondary thickening: a palm is a single
    // unbranched stem with a crown of fronds at the top, and an old palm is
    // not a fatter palm. Modelled as a gently curved stack of segments.
    let segs = 7;
    let r_base = h * 0.028;
    let curve = rng.range(-0.10, 0.10);
    let mut p = [0.0f32, 0.0, 0.0];
    let mut d = norm([curve, 1.0, rng.range(-0.08, 0.08)]);
    for i in 0..segs {
        let f = i as f32 / segs as f32;
        let seg = h / segs as f32;
        let to = add(p, d, seg);
        b.tube(p, to, r_base * (1.0 - f * 0.35), r_base * (1.0 - (f + 0.15) * 0.35), 7, def.trunk_color);
        p = to;
        d = norm([d[0] + curve * 0.06, d[1], d[2]]);
    }
    // Crown: long fronds arching out and down.
    let frond = h * 0.34;
    for k in 0..9 {
        let phase = k as f32 * 2.399_963;
        let dd = tilt([0.0, 1.0, 0.0], rng.range(52.0, 88.0), phase);
        let dd = norm([dd[0], dd[1] - 0.28, dd[2]]);
        b.leaf(p, dd, frond, frond * 0.26, 0.9, def.leaf_color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> TreeRegistry {
        let text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/vegetation/trees.ron"),
        )
        .expect("data/vegetation/trees.ron exists");
        TreeRegistry::from_ron(&text).expect("trees.ron parses")
    }

    const EARTH_R: f64 = 6_371_000.0;

    fn dir_of(lat_deg: f64, lon_deg: f64) -> [f64; 3] {
        let lat = lat_deg.to_radians();
        let lon = lon_deg.to_radians();
        let cl = lat.cos();
        [cl * lon.cos(), lat.sin(), -cl * lon.sin()]
    }

    #[test]
    fn shipped_registry_parses_and_covers_both_kinds() {
        let r = registry();
        assert!(r.len() >= 6, "expected a real species list, got {}", r.len());
        assert!(r.trees.iter().any(|t| !t.is_procedural()), "no model-backed species");
        assert!(r.trees.iter().any(|t| t.is_procedural()), "no procedural species");
        assert!(r.index_of("sakura").is_some(), "sakura missing");
    }

    /// The whole point of the region gate: cherry blossom near Fuji, nowhere else.
    #[test]
    fn sakura_is_local_to_fuji() {
        let r = registry();
        let sakura = r.index_of("sakura").unwrap();
        let fuji = dir_of(35.36, 138.73);
        // Sample many rolls at Fuji: sakura must be reachable there.
        let hit = (0..400).any(|i| r.pick(fuji, 35.36, 700.0, EARTH_R, i * 7919) == Some(sakura));
        assert!(hit, "sakura never picked at Fuji");

        // ...and unreachable at similar latitude/elevation on other continents.
        for (name, lat, lon) in [
            ("Oregon", 45.0, -122.0),
            ("Alps", 46.5, 8.0),
            ("Patagonia", -45.0, -71.0),
        ] {
            let d = dir_of(lat, lon);
            let any = (0..400).any(|i| r.pick(d, lat as f32, 700.0, EARTH_R, i * 7919) == Some(sakura));
            assert!(!any, "sakura leaked to {name}");
        }
    }

    /// Gates must actually restrict: no palms in the arctic, no birch at the equator.
    #[test]
    fn latitude_gates_hold() {
        let r = registry();
        let palm = r.index_of("palm").unwrap();
        let birch = r.index_of("birch").unwrap();
        let arctic = dir_of(68.0, 20.0);
        let equator = dir_of(2.0, 20.0);
        assert!(
            !(0..300).any(|i| r.pick(arctic, 68.0, 200.0, EARTH_R, i * 7919) == Some(palm)),
            "palm grew in the arctic"
        );
        assert!(
            !(0..300).any(|i| r.pick(equator, 2.0, 200.0, EARTH_R, i * 7919) == Some(birch)),
            "birch grew on the equator"
        );
    }

    /// Above every species' elevation ceiling nothing may be picked, which is
    /// what keeps the treeline a treeline.
    #[test]
    fn nothing_grows_above_every_ceiling() {
        let r = registry();
        let d = dir_of(45.0, 10.0);
        assert_eq!(r.pick(d, 45.0, 9000.0, EARTH_R, 12345), None);
    }

    /// Every procedural form must produce real, finite, budgeted geometry.
    #[test]
    fn every_procedural_form_builds_finite_geometry() {
        let r = registry();
        for t in r.trees.iter().filter(|t| t.is_procedural()) {
            for seed in [1u32, 2, 99] {
                let mut b = PlantMeshBuilder::new();
                build_tree(&mut b, t, t.height_m, seed);
                let tris = b.indices.len() / 3;
                assert!(tris > 40, "{} seed {seed}: only {tris} triangles", t.id);
                assert!(tris <= MAX_TRIS + 400, "{} seed {seed}: {tris} triangles blows the budget", t.id);
                assert!(
                    b.vertices.iter().all(|v| v.position.iter().all(|c| c.is_finite())),
                    "{}: non-finite vertex",
                    t.id
                );
                // Trees grow UP from the origin and must stay near their height.
                let top = b.vertices.iter().map(|v| v.position[1]).fold(f32::MIN, f32::max);
                assert!(
                    top > t.height_m * 0.35 && top < t.height_m * 1.8,
                    "{}: top {top} vs height {}",
                    t.id,
                    t.height_m
                );
            }
        }
    }

    /// DEV AID (not run by default). Dumps a side-on SVG of every species so
    /// the silhouettes can be eyeballed without booting the game:
    ///   cargo test --features native --lib tree_mesh::tests::dump -- --ignored
    /// Writes to $TREE_DUMP or ./tree_dump.svg. Uses the REAL generator, so
    /// what you see is the geometry the planet gets.
    #[test]
    #[ignore]
    fn dump_species_svg() {
        let r = registry();
        let procedural: Vec<&TreeDef> = r.trees.iter().filter(|t| t.is_procedural()).collect();
        let (cw, ch) = (260.0f32, 340.0f32);
        let mut svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" \
             viewBox=\"0 0 {} {}\"><rect width=\"100%\" height=\"100%\" fill=\"#12160f\"/>",
            cw * procedural.len() as f32,
            ch + 40.0,
            cw * procedural.len() as f32,
            ch + 40.0
        );
        for (i, t) in procedural.iter().enumerate() {
            let mut b = PlantMeshBuilder::new();
            build_tree(&mut b, t, t.height_m, 11);
            let scale = (ch - 60.0) / t.height_m;
            let ox = cw * i as f32 + cw * 0.5;
            let oy = ch - 20.0;
            // Painter's algorithm: farthest (most negative z) drawn first.
            let mut faces: Vec<(f32, String)> = Vec::new();
            for f in b.indices.chunks(3) {
                let p: Vec<[f32; 3]> = f.iter().map(|&i| b.vertices[i as usize].position).collect();
                let z = (p[0][2] + p[1][2] + p[2][2]) / 3.0;
                let (c, _) = crate::terrain::planet_surface::unpack_uv_to_color(
                    b.vertices[f[0] as usize].uv,
                );
                // Cheap lambert against a fixed key so the form reads.
                let n = b.vertices[f[0] as usize].normal;
                let l = (n[0] * 0.4 + n[1] * 0.75 + n[2] * 0.5).max(0.0) * 0.75 + 0.35;
                let col = format!(
                    "rgb({},{},{})",
                    (c[0] * l * 255.0).min(255.0) as u32,
                    (c[1] * l * 255.0).min(255.0) as u32,
                    (c[2] * l * 255.0).min(255.0) as u32
                );
                let pts: Vec<String> = p
                    .iter()
                    .map(|v| {
                        format!("{:.1},{:.1}", ox + v[0] * scale + v[2] * scale * 0.25, oy - v[1] * scale)
                    })
                    .collect();
                faces.push((z, format!("<polygon points=\"{}\" fill=\"{}\"/>", pts.join(" "), col)));
            }
            faces.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            for (_, s) in faces {
                svg.push_str(&s);
            }
            svg.push_str(&format!(
                "<text x=\"{ox}\" y=\"{}\" fill=\"#cfe3bd\" font-family=\"monospace\" \
                 font-size=\"13\" text-anchor=\"middle\">{} ({:.0} m, {} tris)</text>",
                ch + 12.0,
                t.id,
                t.height_m,
                b.indices.len() / 3
            ));
        }
        svg.push_str("</svg>");
        let path = std::env::var("TREE_DUMP").unwrap_or_else(|_| "tree_dump.svg".to_string());
        std::fs::write(&path, svg).expect("write dump");
        eprintln!("wrote {path}");
    }

    #[test]
    fn same_seed_is_deterministic_and_seeds_differ() {
        let r = registry();
        let t = r.get(r.index_of("sakura").unwrap()).unwrap();
        let gen = |s: u32| {
            let mut b = PlantMeshBuilder::new();
            build_tree(&mut b, t, t.height_m, s);
            b.vertices.len()
        };
        assert_eq!(gen(7), gen(7), "same seed must rebuild identically");
        assert_ne!(gen(7), gen(8), "different seeds must differ");
    }

    /// Blossom species must actually emit petal-coloured faces, or "cherry
    /// blossom" is just a green tree with a pink entry in a data file.
    #[test]
    fn sakura_emits_blossom_coloured_faces() {
        let r = registry();
        let t = r.get(r.index_of("sakura").unwrap()).unwrap();
        let mut b = PlantMeshBuilder::new();
        build_tree(&mut b, t, t.height_m, 5);
        // Unpack each face colour and look for the pink.
        let mut pink = 0;
        for f in b.indices.chunks(3) {
            let uv = b.vertices[f[0] as usize].uv;
            let (c, _) = crate::terrain::planet_surface::unpack_uv_to_color(uv);
            if c[0] > 0.85 && c[1] > 0.55 && c[1] < 0.9 && c[2] > 0.6 {
                pink += 1;
            }
        }
        assert!(pink > 30, "only {pink} blossom faces on a sakura");
    }
}
