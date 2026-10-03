//! Where a grow machine's plants stand (2026-09-26).
//!
//! Since v0.1367.0 a bed, tray or field plot holds as many plants as its floor
//! fits at the crop's spacing (`systems::farming::units::plants_in_plot`), and
//! the simulation harvests, feeds and waters that many. Until this module the
//! renderer still drew ONE plant per crop entity, so a plot of wheat that the
//! farm fed as 480 plants showed a single stalk. This lays out what the plot
//! really holds:
//!
//! - [`plot_rects`]: a grow machine's footprint divided into its medium's
//!   plots (`plots` in data/garden/grow_media.ron), as near square as the
//!   count allows. A stacked medium's plots are shelves, each the whole
//!   footprint, one above another.
//! - [`plot_plants`]: one plot's plants in rows whose pitch is the crop's real
//!   spacing, up to the per-plot visual cap (`plot_visual_cap` in
//!   data/plants_visual.ron). Above the cap the plot is drawn as `cap` clumps
//!   instead, each standing for the plants whose floor it covers and drawn
//!   that much wider (never taller), so a cereal plot reads as a stand.
//! - [`plot_draw_cap`]: that cap lowered for a heavy model, so no plot
//!   draws more vertices than `plot_vertex_budget` in the same file.
//! - [`bake_copy`]: one placed copy of a stage model appended to a merged
//!   mesh, which is how a plot of hero models costs one draw, not one per
//!   plant.
//! - [`shelf_rack_geometry`] and [`tent_geometry`] (2026-09-27): the body a
//!   stacked machine is drawn with, open shelving with a deck under each of
//!   its shelf plots, and the clear fruiting tent a grow medium's
//!   `enclosure` puts around it.
//!
//! Pure geometry with no engine state, so the rules are unit tested here and
//! `home_meshes::rebuild_plant_meshes` only places meshes.

use glam::{Quat, Vec3};

use crate::renderer::mesh::Vertex;

/// One plot of a grow machine, in the machine's own frame (before its yaw).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PlotRect {
    /// Centre, metres from the footprint centre: x across the machine's
    /// width (`size.0`), z across its depth (`size.2`).
    pub center: [f32; 2],
    /// Width (x) and depth (z) of the plot, metres.
    pub size: [f32; 2],
    /// Height of the plot's growing surface above the machine's base, metres.
    pub floor: f32,
}

/// Divide a machine footprint `size` (width, height, depth, metres, as in
/// data/machines/home.ron) into `plots` plots (0 is read as 1). Flat plots
/// tile the footprint on the most nearly square grid whose cell count is
/// exactly `plots`, all at the machine's top; plot `u` is column `u % across`,
/// row `u / across`. Stacked plots are shelves: each the whole footprint,
/// spaced evenly up the height, the bottom one just clear of the floor (a
/// quarter of the spacing, at most 10 cm) and every one with its whole
/// spacing free above it, so what grows on the top shelf stays under the
/// frame. (Until 2026-09-27 the top shelf WAS the machine's top and the
/// bottom spacing stood empty: the oysters on a rack's top shelf reached
/// 1.96 m, through the roof of its 1.9 m tent.)
pub(crate) fn plot_rects(size: (f32, f32, f32), plots: u32, stacked: bool) -> Vec<PlotRect> {
    let n = plots.max(1);
    let (w, h, d) = (size.0.max(0.0), size.1.max(0.0), size.2.max(0.0));
    if stacked {
        let step = h / n as f32;
        let base = (step * 0.25).min(0.1);
        return (0..n)
            .map(|k| PlotRect { center: [0.0, 0.0], size: [w, d], floor: base + step * k as f32 })
            .collect();
    }
    let (across, deep) = plot_grid(w, d, n);
    let (pw, pd) = (w / across as f32, d / deep as f32);
    (0..n)
        .map(|u| {
            let (ix, iz) = (u % across, u / across);
            PlotRect {
                center: [-w * 0.5 + (ix as f32 + 0.5) * pw, -d * 0.5 + (iz as f32 + 0.5) * pd],
                size: [pw, pd],
                floor: h,
            }
        })
        .collect()
}

/// The (across, deep) factor pair of `n` whose cells on a `w` x `d`
/// footprint are closest to square: a 2 x 1 m bed in 2 plots is two 1 m
/// squares side by side, not two 2 x 0.5 m strips.
fn plot_grid(w: f32, d: f32, n: u32) -> (u32, u32) {
    let mut best = (n, 1);
    let mut best_err = f32::INFINITY;
    for across in 1..=n {
        if n % across != 0 {
            continue;
        }
        let deep = n / across;
        let cell_w = (w / across as f32).max(1e-6);
        let cell_d = (d / deep as f32).max(1e-6);
        let err = (cell_w / cell_d).ln().abs();
        if err < best_err {
            best_err = err;
            best = (across, deep);
        }
    }
    best
}

/// One plot's drawn plants.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PlotPlants {
    /// Where each plant (or clump) stands, metres from the plot centre, in
    /// the plot's frame: [x across its width, z across its depth].
    pub spots: Vec<[f32; 2]>,
    /// How much wider than one plant each spot is drawn: 1 for real plants;
    /// for clumps the square root of the plants each stands for, so a clump
    /// covers the floor its plants would. Height is never scaled.
    pub widen: f32,
}

/// How far a plant may sit off its row position, as a fraction of the pitch
/// each way. Sown rows are regular, but a perfect lattice reads as a
/// computer; 15 percent breaks it while no plant can reach a neighbour's
/// place, so the spacing stays the crop's.
const JITTER: f32 = 0.15;

/// Lay `plants` plants (0 is read as 1) over a plot of `size` (width, depth,
/// metres), drawing at most `cap` of them.
///
/// The rule: `n = min(plants, cap)` spots in `rows = round(sqrt(n x depth /
/// width))` rows, the plants dealt to the rows as evenly as whole numbers
/// allow and each row spread across the whole width. Every spot is the
/// centre of its own cell, nudged by at most [`JITTER`] of a pitch, so it
/// never leaves the plot, and the rows reach every edge. When `plants` is the
/// plot area over the crop's `area_per_plant_m2` (how the farm counts them),
/// the pitch is the crop's real spacing, give or take the rounding of a row.
///
/// Above the cap the same grid carries `cap` clumps and `widen` becomes
/// `sqrt(plants / cap)`: each clump stands for `plants / cap` plants and
/// covers their floor. `cap == 0` means the data set no cap: one plant at the
/// plot centre, unwidened, the look before plots were drawn in full.
///
/// `seed` makes the nudges stable, so a rebuild never shuffles a bed.
pub(crate) fn plot_plants(size: [f32; 2], plants: u32, cap: u32, seed: u64) -> PlotPlants {
    let (w, d) = (size[0].max(0.0), size[1].max(0.0));
    let plants = plants.max(1);
    let (n, widen) = if cap == 0 {
        return PlotPlants { spots: vec![[0.0, 0.0]], widen: 1.0 };
    } else if plants <= cap {
        (plants, 1.0)
    } else {
        (cap, (plants as f32 / cap as f32).sqrt())
    };
    let rows = ((n as f32 * d / w.max(1e-6)).sqrt().round() as u32).clamp(1, n);
    let row_pitch = d / rows as f32;
    let mut rng = seed ^ 0x9E37_79B9_7F4A_7C15;
    let mut spots = Vec::with_capacity(n as usize);
    for r in 0..rows {
        // Row r takes the plants between two even cut points, so row counts
        // differ by at most one and every row is non-empty (rows <= n).
        let in_row = (r + 1) * n / rows - r * n / rows;
        let pitch = w / in_row as f32;
        let z = -d * 0.5 + (r as f32 + 0.5) * row_pitch;
        for c in 0..in_row {
            let x = -w * 0.5 + (c as f32 + 0.5) * pitch;
            let jx = unit_noise(&mut rng) * JITTER * pitch;
            let jz = unit_noise(&mut rng) * JITTER * row_pitch;
            spots.push([x + jx, z + jz]);
        }
    }
    PlotPlants { spots, widen }
}

/// The most plants or clumps one plot is drawn with: the plant `cap`
/// (`plot_visual_cap`), lowered so a plot of plants of `verts_per_plant`
/// vertices stays inside `vertex_budget` (`plot_vertex_budget`), and never
/// below 1. A widened clump has its one plant's vertices, so the budget is
/// spots x vertices. `cap == 0` stays 0 (no cap in the data: one plant per
/// plot, see [`plot_plants`]); `vertex_budget == 0` leaves the plant cap alone.
pub(crate) fn plot_draw_cap(cap: u32, vertex_budget: u32, verts_per_plant: usize) -> u32 {
    if cap == 0 || vertex_budget == 0 {
        return cap;
    }
    let fits = (vertex_budget as usize / verts_per_plant.max(1)).max(1);
    cap.min(u32::try_from(fits).unwrap_or(u32::MAX))
}

/// A stable value in [-1, 1] from a splitmix64 step.
fn unit_noise(state: &mut u64) -> f32 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 / (1u64 << 24) as f32 * 2.0 - 1.0
}

/// Append one copy of a model (`src_vertices`, `src_indices`, in the model's
/// own metres with its base at the origin) to a merged mesh, standing at
/// `at`, scaled `widen` across and `tall` up, then turned by `rot` about its
/// base: a plot passes a yaw about +Y, a tower's net cup also tips the plant
/// out from the column (2026-10-03). Normals follow the inverse-transpose of
/// that scale, so a widened clump still lights like its model; UVs are
/// copied as they are, so the copy samples the same palette texels.
pub(crate) fn bake_copy(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    src_vertices: &[Vertex],
    src_indices: &[u32],
    at: Vec3,
    rot: Quat,
    widen: f32,
    tall: f32,
) {
    let base = vertices.len() as u32;
    let (sx, sy) = (widen.max(1e-6), tall.max(1e-6));
    vertices.extend(src_vertices.iter().map(|v| {
        let p = Vec3::new(v.position[0] * sx, v.position[1] * sy, v.position[2] * sx);
        let n = Vec3::new(v.normal[0] / sx, v.normal[1] / sy, v.normal[2] / sx).normalize_or_zero();
        Vertex { position: (at + rot * p).to_array(), normal: (rot * n).to_array(), uv: v.uv }
    }));
    indices.extend(src_indices.iter().map(|i| base + i));
}

/// Radius of a shelving upright, metres: the 1 inch (25.4 mm) round post of
/// the chrome wire shelving a mushroom fruiting tent is built around.
pub(crate) const SHELF_POST_R: f32 = 0.0127;
/// Thickness of a shelf deck, metres.
pub(crate) const SHELF_DECK_M: f32 = 0.02;
/// Sides of a drawn upright: eight reads as round at arm's length.
const SHELF_POST_SIDES: u32 = 8;

/// Open shelving for a stacked grow machine (a mushroom rack, 2026-09-27):
/// a round upright at each corner, the machine's full height, and one deck
/// under every shelf, its top at the shelf's floor, so what grows on each
/// shelf is in plain sight. The decks are this machine's [`plot_rects`], so
/// a deck is drawn exactly where its plants stand. In the machine's own
/// frame, as its box was: centred on x and z, base at y = 0, `size` (width,
/// height, depth, metres) the machine's size in data/machines/home.ron.
pub(crate) fn shelf_rack_geometry(size: (f32, f32, f32), plots: u32) -> (Vec<Vertex>, Vec<u32>) {
    let (w, h, d) = (size.0.max(0.05), size.1.max(0.05), size.2.max(0.05));
    let (mut v, mut ix) = (Vec::new(), Vec::new());
    let r = SHELF_POST_R.min(w * 0.25).min(d * 0.25);
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        push_post(&mut v, &mut ix, [sx * (w * 0.5 - r), sz * (d * 0.5 - r)], r, h);
    }
    for shelf in plot_rects((w, h, d), plots, true) {
        let top = shelf.floor;
        push_box(&mut v, &mut ix, [-w * 0.5, (top - SHELF_DECK_M).max(0.0), -d * 0.5], [w * 0.5, top, d * 0.5]);
    }
    (v, ix)
}

/// A grow machine's enclosure (a mushroom rack's fruiting tent, 2026-09-27):
/// a sheet on each of its four sides and one over the top, `size` (width,
/// height, depth, metres; `Enclosure::size` in data/garden/grow_media.ron)
/// centred on the machine with its hem on the floor. No floor sheet, since
/// the tent stands on the floor. It draws through the transparent pass,
/// which shades both sides of a sheet, so no face is doubled for the inside.
pub(crate) fn tent_geometry(size: (f32, f32, f32)) -> (Vec<Vertex>, Vec<u32>) {
    let (x, h, z) = (size.0.max(0.05) * 0.5, size.1.max(0.05), size.2.max(0.05) * 0.5);
    let (mut v, mut ix) = (Vec::new(), Vec::new());
    for (n, corners) in box_faces([-x, 0.0, -z], [x, h, z]) {
        if n[1] < -0.5 {
            continue; // no floor sheet
        }
        push_quad(&mut v, &mut ix, corners, n);
    }
    (v, ix)
}

/// The six faces of an axis-aligned box from `lo` to `hi`: each outward
/// normal with its corners counter-clockwise seen from outside, the winding
/// `Mesh::box_xyz` uses and the back-face cull keeps.
fn box_faces(lo: [f32; 3], hi: [f32; 3]) -> [([f32; 3], [[f32; 3]; 4]); 6] {
    let [x0, y0, z0] = lo;
    let [x1, y1, z1] = hi;
    [
        ([0.0, 0.0, 1.0], [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]]),
        ([0.0, 0.0, -1.0], [[x1, y0, z0], [x0, y0, z0], [x0, y1, z0], [x1, y1, z0]]),
        ([1.0, 0.0, 0.0], [[x1, y0, z1], [x1, y0, z0], [x1, y1, z0], [x1, y1, z1]]),
        ([-1.0, 0.0, 0.0], [[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]]),
        ([0.0, 1.0, 0.0], [[x0, y1, z1], [x1, y1, z1], [x1, y1, z0], [x0, y1, z0]]),
        ([0.0, -1.0, 0.0], [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]]),
    ]
}

/// One flat quad, corners counter-clockwise from its outside.
fn push_quad(v: &mut Vec<Vertex>, ix: &mut Vec<u32>, corners: [[f32; 3]; 4], normal: [f32; 3]) {
    let base = v.len() as u32;
    for (p, uv) in corners.into_iter().zip([[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]) {
        v.push(Vertex { position: p, normal, uv });
    }
    ix.extend([base, base + 1, base + 2, base + 2, base + 3, base]);
}

fn push_box(v: &mut Vec<Vertex>, ix: &mut Vec<u32>, lo: [f32; 3], hi: [f32; 3]) {
    for (n, corners) in box_faces(lo, hi) {
        push_quad(v, ix, corners, n);
    }
}

/// A round upright standing on the floor at `c` (x, z), `r` in radius and
/// `h` tall: smooth-shaded sides and a flat cap (its foot is on the floor).
fn push_post(v: &mut Vec<Vertex>, ix: &mut Vec<u32>, c: [f32; 2], r: f32, h: f32) {
    let n = SHELF_POST_SIDES;
    let ring = |k: u32| {
        let (s, co) = (k as f32 / n as f32 * std::f32::consts::TAU).sin_cos();
        (co, s)
    };
    let base = v.len() as u32;
    for k in 0..=n {
        let (co, s) = ring(k);
        let u = k as f32 / n as f32;
        v.push(Vertex { position: [c[0] + co * r, 0.0, c[1] + s * r], normal: [co, 0.0, s], uv: [u, 1.0] });
        v.push(Vertex { position: [c[0] + co * r, h, c[1] + s * r], normal: [co, 0.0, s], uv: [u, 0.0] });
    }
    for k in 0..n {
        let (b0, t0, b1, t1) = (base + 2 * k, base + 2 * k + 1, base + 2 * k + 2, base + 2 * k + 3);
        ix.extend([b0, t0, b1, b1, t0, t1]);
    }
    let cap = v.len() as u32;
    v.push(Vertex { position: [c[0], h, c[1]], normal: [0.0, 1.0, 0.0], uv: [0.5, 0.5] });
    for k in 0..n {
        let (co, s) = ring(k);
        v.push(Vertex { position: [c[0] + co * r, h, c[1] + s * r], normal: [0.0, 1.0, 0.0], uv: [0.5, 0.5] });
    }
    for k in 0..n {
        ix.extend([cap, cap + 1 + (k + 1) % n, cap + 1 + k]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inside(spots: &[[f32; 2]], size: [f32; 2]) -> bool {
        spots.iter().all(|s| s[0].abs() <= size[0] * 0.5 && s[1].abs() <= size[1] * 0.5)
    }

    /// The largest distance from any point of the plot to its nearest spot,
    /// sampled on a 60 x 60 grid that includes the corners.
    fn worst_gap(spots: &[[f32; 2]], size: [f32; 2]) -> f32 {
        let mut worst = 0.0f32;
        for i in 0..=60 {
            for j in 0..=60 {
                let p = [size[0] * (i as f32 / 60.0 - 0.5), size[1] * (j as f32 / 60.0 - 0.5)];
                let near = spots
                    .iter()
                    .map(|s| ((s[0] - p[0]).powi(2) + (s[1] - p[1]).powi(2)).sqrt())
                    .fold(f32::INFINITY, f32::min);
                worst = worst.max(near);
            }
        }
        worst
    }

    fn mean_neighbour(spots: &[[f32; 2]]) -> f32 {
        let sum: f32 = spots
            .iter()
            .enumerate()
            .map(|(i, a)| {
                spots
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != i)
                    .map(|(_, b)| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt())
                    .fold(f32::INFINITY, f32::min)
            })
            .sum();
        sum / spots.len() as f32
    }

    /// N plants at spacing s fill the plot's rectangle without leaving it:
    /// every plant drawn, every one inside, no point of the plot farther than
    /// 1.25 spacings from a plant (half a cell's diagonal plus the nudge),
    /// and neighbours about one spacing apart.
    /// Cases are the showcase's real plots: a 1 m2 potato bed plot, a 1.44 m2
    /// bean field plot, a 1 m2 sunflower plot, a 2 x 1 m plot of seven.
    ///
    /// Seen red on 2026-09-26 by breaking the column position to
    /// `(c + 0.5) * w - w / 2` (the pitch left out): the spots ran off the
    /// plot and the inside check failed.
    #[test]
    fn plant_layout_draws_every_plant_at_its_spacing_inside_the_plot() {
        for (size, area_per_plant) in [
            ([1.0, 1.0], 0.234_f32),  // potato
            ([1.2, 1.2], 0.045),      // bean
            ([1.0, 1.0], 0.193),      // sunflower
            ([2.0, 1.0], 0.28),       // seven in a long plot
            ([1.2, 1.2], 0.0145),     // 99 plants, still under the cap
        ] {
            let plants = ((size[0] * size[1]) / area_per_plant * (1.0 + 1e-6)).floor() as u32;
            let got = plot_plants(size, plants, 128, 7);
            let spacing = (size[0] * size[1] / plants as f32).sqrt();
            assert_eq!(got.spots.len() as u32, plants, "{size:?}: one spot per plant");
            assert_eq!(got.widen, 1.0, "{size:?}: real plants are drawn at their own size");
            assert!(inside(&got.spots, size), "{size:?}: a plant left the plot: {:?}", got.spots);
            let gap = worst_gap(&got.spots, size);
            assert!(gap <= 1.25 * spacing, "{size:?}: a point {gap} m from any plant, spacing {spacing}");
            let nn = mean_neighbour(&got.spots);
            assert!(
                (0.75 * spacing..=1.3 * spacing).contains(&nn),
                "{size:?}: neighbours {nn} m apart at a spacing of {spacing}"
            );
        }
    }

    /// Above the cap a plot draws exactly `cap` clumps that still cover it:
    /// 480 wheat plants on a 1.44 m2 field plot at a cap of 128 are 128
    /// clumps, each sqrt(480 / 128) wider so their floor adds up to the
    /// plot's, all inside, no point farther than 1.25 clump spacings from one.
    ///
    /// Seen red on 2026-09-26 by drawing `plants` spots and ignoring the cap:
    /// 480 spots, not 128.
    #[test]
    fn plant_layout_above_the_cap_draws_cap_clumps_covering_the_plot() {
        let size = [1.2, 1.2];
        let got = plot_plants(size, 480, 128, 3);
        assert_eq!(got.spots.len(), 128, "the clump count is the cap");
        assert!((got.widen - (480.0f32 / 128.0).sqrt()).abs() < 1e-5, "widen {}", got.widen);
        assert!(
            (got.widen * got.widen * got.spots.len() as f32 - 480.0).abs() < 1e-2,
            "the clumps stand for every plant"
        );
        assert!(inside(&got.spots, size), "a clump left the plot");
        let clump_spacing = (size[0] * size[1] / 128.0).sqrt();
        let gap = worst_gap(&got.spots, size);
        assert!(gap <= 1.25 * clump_spacing, "a point {gap} m from any clump, spacing {clump_spacing}");
        // And the stated fallback: no cap in the data is one plant per plot.
        let one = plot_plants(size, 480, 0, 3);
        assert_eq!(one, PlotPlants { spots: vec![[0.0, 0.0]], widen: 1.0 });
    }

    /// A heavy model lowers the cap so the plot stays in its vertex budget:
    /// a rice plot at its last stage (3,954 vertices a plant) in 65,536 is 16
    /// clumps; a wheat stalk (196) is held by the plant cap instead; no
    /// budget, or a budget smaller than one plant, never draws nothing.
    ///
    /// Seen red on 2026-09-26 by returning `cap` unchanged: rice drew 128.
    #[test]
    fn plant_layout_draw_cap_keeps_a_plot_inside_its_vertex_budget() {
        assert_eq!(plot_draw_cap(128, 65_536, 3_954), 16);
        assert_eq!(plot_draw_cap(128, 65_536, 196), 128);
        assert_eq!(plot_draw_cap(128, 0, 3_954), 128, "no budget: the plant cap alone");
        assert_eq!(plot_draw_cap(128, 1_000, 3_954), 1, "never fewer than one");
        assert_eq!(plot_draw_cap(0, 65_536, 3_954), 0, "no cap stays the one-plant fallback");
        assert_eq!(plot_draw_cap(128, 65_536, 0), 128, "an empty model is not a division by zero");
    }

    /// The same seed lays the same bed (a rebuild must not shuffle plants).
    #[test]
    fn plant_layout_is_stable_for_a_seed() {
        assert_eq!(plot_plants([1.0, 1.0], 33, 128, 11), plot_plants([1.0, 1.0], 33, 128, 11));
        assert_ne!(plot_plants([1.0, 1.0], 33, 128, 11), plot_plants([1.0, 1.0], 33, 128, 12));
    }

    /// Flat plots tile the footprint exactly in near-square cells; stacked
    /// plots are shelves of the whole footprint up the machine's height.
    #[test]
    fn plant_layout_plot_rects_tile_the_footprint() {
        // A 2 x 1 m bed in two plots: two 1 m squares side by side.
        let bed = plot_rects((2.0, 0.4, 1.0), 2, false);
        assert_eq!(bed.len(), 2);
        for p in &bed {
            assert_eq!(p.size, [1.0, 1.0]);
            assert_eq!(p.floor, 0.4);
        }
        assert_eq!(bed[0].center, [-0.5, 0.0]);
        assert_eq!(bed[1].center, [0.5, 0.0]);
        // A 2.4 m field in four: 1.2 m squares, their areas summing to the field's.
        let field = plot_rects((2.4, 0.18, 2.4), 4, false);
        let area: f32 = field.iter().map(|p| p.size[0] * p.size[1]).sum();
        assert!((area - 2.4 * 2.4).abs() < 1e-4);
        for p in &field {
            assert!((p.size[0] - 1.2).abs() < 1e-6 && (p.size[1] - 1.2).abs() < 1e-6);
            for (c, s, full) in [(p.center[0], p.size[0], 2.4), (p.center[1], p.size[1], 2.4)] {
                assert!(c - s * 0.5 >= -full * 0.5 - 1e-5 && c + s * 0.5 <= full * 0.5 + 1e-5);
            }
        }
        // A 1.8 m rack of four shelves: whole footprint each, 0.45 m apart,
        // the bottom one 10 cm off the floor, the top one 0.35 m under the
        // frame's top with its whole spacing free above it.
        let rack = plot_rects((1.2, 1.8, 0.6), 4, true);
        for (p, want) in rack.iter().zip([0.1, 0.55, 1.0, 1.45]) {
            assert!((p.floor - want).abs() < 1e-5, "shelf at {} m, want {want}", p.floor);
        }
        assert_eq!(rack.len(), 4);
        assert!(rack.iter().all(|p| p.size == [1.2, 0.6] && p.center == [0.0, 0.0]));
        // Zero plots is one.
        assert_eq!(plot_rects((1.0, 1.0, 1.0), 0, false).len(), 1);
    }

    /// A baked copy stands where it is put, widens across but not up, and
    /// keeps its indices pointing at its own vertices.
    #[test]
    fn plant_layout_bake_copy_places_and_widens_a_model() {
        let v = |p: [f32; 3]| Vertex { position: p, normal: [1.0, 0.0, 0.0], uv: [0.0, 0.0] };
        let src = [v([0.1, 0.0, 0.0]), v([0.0, 1.0, 0.0]), v([0.0, 0.0, 0.1])];
        let (mut verts, mut idx) = (vec![v([9.0, 9.0, 9.0])], vec![0u32]);
        bake_copy(&mut verts, &mut idx, &src, &[0, 1, 2], Vec3::new(5.0, 2.0, -3.0), Quat::IDENTITY, 2.0, 1.0);
        assert_eq!(idx, vec![0, 1, 2, 3], "the copy's indices follow the existing vertex");
        let near = |a: [f32; 3], b: [f32; 3]| (Vec3::from(a) - Vec3::from(b)).length() < 1e-5;
        assert!(near(verts[1].position, [5.2, 2.0, -3.0]), "x doubled, then placed");
        assert!(near(verts[2].position, [5.0, 3.0, -3.0]), "height untouched");
        assert!(near(verts[3].position, [5.0, 2.0, -2.8]), "z doubled");
        assert!((Vec3::from(verts[1].normal).length() - 1.0).abs() < 1e-5, "normals stay unit");
    }

    /// Every triangle turns the way its vertices' normals say, so the
    /// back-face cull keeps the outside of every face.
    fn wound_outward(v: &[Vertex], ix: &[u32]) -> bool {
        ix.chunks(3).all(|t| {
            let p = |i: u32| Vec3::from(v[i as usize].position);
            let facing = (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0]));
            let said: Vec3 = t.iter().map(|&i| Vec3::from(v[i as usize].normal)).sum();
            facing.dot(said) > 0.0
        })
    }

    /// The triangles that cross the height `y`, as (min x, max x, min z,
    /// max z) of each: what a level cut through the mesh meets.
    fn cut_at(v: &[Vertex], ix: &[u32], y: f32) -> Vec<[f32; 4]> {
        ix.chunks(3)
            .filter_map(|t| {
                let ps: Vec<[f32; 3]> = t.iter().map(|&i| v[i as usize].position).collect();
                let (lo, hi) = ps.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p[1]), b.max(p[1])));
                if !(lo < y && y < hi) {
                    return None;
                }
                let e = ps.iter().fold([f32::MAX, f32::MIN, f32::MAX, f32::MIN], |e, p| {
                    [e[0].min(p[0]), e[1].max(p[0]), e[2].min(p[2]), e[3].max(p[2])]
                });
                Some(e)
            })
            .collect()
    }

    /// What is wrong with `(v, ix)` as open shelving of `size` in `plots`
    /// shelves: empty when a deck with its top at each shelf's floor covers
    /// the footprint, the space between the first two decks holds only the
    /// corner uprights, nothing leaves the rack, and every face is turned out.
    fn shelving_problems(v: &[Vertex], ix: &[u32], size: (f32, f32, f32), plots: u32) -> Vec<String> {
        let (hw, h, hd) = (size.0 * 0.5, size.1, size.2 * 0.5);
        let mut out = Vec::new();
        if ix.is_empty() || ix.len() % 3 != 0 {
            out.push(format!("{} indices", ix.len()));
        }
        if !wound_outward(v, ix) {
            out.push("a face is turned inside out".into());
        }
        for p in v {
            let [x, y, z] = p.position;
            if x.abs() > hw + 1e-5 || z.abs() > hd + 1e-5 || !(0.0..=h + 1e-5).contains(&y) {
                out.push(format!("outside the rack: {:?}", p.position));
            }
        }
        let shelves = plot_rects(size, plots, true);
        for shelf in &shelves {
            for corner in [[-hw, -hd], [hw, -hd], [hw, hd], [-hw, hd]] {
                let top = v.iter().any(|p| {
                    (p.position[1] - shelf.floor).abs() < 1e-5
                        && (p.position[0] - corner[0]).abs() < 1e-5
                        && (p.position[2] - corner[1]).abs() < 1e-5
                        && p.normal[1] > 0.99
                });
                if !top {
                    out.push(format!("no deck top reaching {corner:?} at the shelf floor {} m", shelf.floor));
                }
            }
        }
        // Halfway between the first two shelves a level cut meets only the
        // uprights: pieces no wider than a post, at a corner.
        if shelves.len() >= 2 {
            let y = (shelves[0].floor + shelves[1].floor - SHELF_DECK_M) * 0.5;
            let cut = cut_at(v, ix, y);
            if cut.is_empty() {
                out.push("nothing holds the decks up".into());
            }
            for e in &cut {
                let slim = e[1] - e[0] <= 2.0 * SHELF_POST_R + 1e-5 && e[3] - e[2] <= 2.0 * SHELF_POST_R + 1e-5;
                let cornered = e[0].abs().max(e[1].abs()) > hw - 2.0 * SHELF_POST_R - 1e-5;
                if !(slim && cornered) {
                    out.push(format!("between the shelves at {y} m: {e:?}"));
                }
            }
        }
        out
    }

    /// The mushroom rack is open shelving (see `shelving_problems`), and the
    /// check is not one that cannot fail: the solid box of the same size, the
    /// way the rack was drawn until 2026-09-27, is rejected by it.
    #[test]
    fn shelf_rack_is_open_shelving_with_a_deck_under_every_shelf() {
        let size = (1.2, 1.8, 0.6);
        let (v, ix) = shelf_rack_geometry(size, 4);
        let problems = shelving_problems(&v, &ix, size, 4);
        assert!(problems.is_empty(), "{problems:#?}");
        let (mut bv, mut bix) = (Vec::new(), Vec::new());
        push_box(&mut bv, &mut bix, [-0.6, 0.0, -0.3], [0.6, 1.8, 0.3]);
        let solid = shelving_problems(&bv, &bix, size, 4);
        assert!(solid.iter().any(|p| p.starts_with("no deck top")), "{solid:#?}");
        assert!(solid.iter().any(|p| p.starts_with("between the shelves")), "{solid:#?}");
    }

    /// What is wrong with `(v, ix)` as a tent of `size`: empty when it is
    /// four walls and a top (no floor sheet) spanning exactly the box
    /// centred on the machine with its hem on the floor, every sheet out.
    fn tent_problems(v: &[Vertex], ix: &[u32], size: (f32, f32, f32)) -> Vec<String> {
        let mut out = Vec::new();
        if ix.len() != 5 * 6 {
            out.push(format!("{} indices, not four walls and a top", ix.len()));
        }
        if !wound_outward(v, ix) {
            out.push("a sheet is turned inside out".into());
        }
        let lo = v.iter().fold(Vec3::splat(f32::MAX), |a, p| a.min(Vec3::from(p.position)));
        let hi = v.iter().fold(Vec3::splat(f32::MIN), |a, p| a.max(Vec3::from(p.position)));
        let (want_lo, want_hi) = (Vec3::new(-size.0 * 0.5, 0.0, -size.2 * 0.5), Vec3::new(size.0 * 0.5, size.1, size.2 * 0.5));
        if (lo - want_lo).length() > 1e-5 || (hi - want_hi).length() > 1e-5 {
            out.push(format!("spans {lo} to {hi}, not {want_lo} to {want_hi}"));
        }
        if v.iter().any(|p| p.normal[1] < -0.5) {
            out.push("a floor sheet".into());
        }
        out
    }

    /// The fruiting tent is the enclosure's box (see `tent_problems`), and
    /// the check rejects a tent built with width and depth swapped.
    #[test]
    fn tent_is_the_enclosure_box_without_a_floor() {
        let (v, ix) = tent_geometry((1.3, 1.9, 0.7));
        let problems = tent_problems(&v, &ix, (1.3, 1.9, 0.7));
        assert!(problems.is_empty(), "{problems:#?}");
        let (sv, six) = tent_geometry((0.7, 1.9, 1.3));
        assert!(tent_problems(&sv, &six, (1.3, 1.9, 0.7)).iter().any(|p| p.starts_with("spans")));
    }
}
