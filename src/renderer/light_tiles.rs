//! Screen-tile light binning (light clustering L1, v0.951): CPU-side
//! Forward+ first rung. The screen is a fixed 16x9 tile grid; each frame,
//! every light's influence sphere is conservatively projected to a screen
//! rect and its index appended to each overlapped tile's list. The fragment
//! shader then loops ONLY its tile's list, which bounds per-pixel cost by
//! local overlap instead of the global light count - that is what lifts the
//! old 256 truncate to 2048 (the flat-cost ceiling the operator asked for:
//! "could we have 1,000 or more?").
//!
//! Pure math + owned buffers, no wgpu here: unit-tested below, uploaded by
//! the renderer. Conservative over-inclusion is always allowed (costs a few
//! ALU on a few tiles); EXCLUSION of a light from a tile it lights is the
//! only correctness bug, and the tests pin against it.

use glam::{Mat4, Vec3, Vec4Swizzles};

pub const TILE_COLS: usize = 16;
pub const TILE_ROWS: usize = 9;
pub const TILE_COUNT: usize = TILE_COLS * TILE_ROWS;
/// Per-tile light cap. 64 x 144 tiles x 4 bytes = 36 KB of indices.
pub const TILE_CAP: usize = 64;

/// Minimal light view for binning (position + influence range; line lights
/// pass both endpoints as two entries).
#[derive(Debug, Clone, Copy)]
pub struct BinLight {
    pub pos: Vec3,
    pub range: f32,
}

/// Bin light spheres into the tile grid. Returns (counts[TILE_COUNT],
/// indices[TILE_COUNT * TILE_CAP]) ready for storage-buffer upload. `screen`
/// is the render-target pixel size; `view_proj` the camera matrix the frame
/// renders with.
pub fn bin_lights(
    lights: &[BinLight],
    view_proj: &Mat4,
    cam_pos: Vec3,
    screen: (u32, u32),
) -> (Vec<u32>, Vec<u32>) {
    let mut counts = vec![0u32; TILE_COUNT];
    let mut indices = vec![0u32; TILE_COUNT * TILE_CAP];
    let (sw, sh) = (screen.0.max(1) as f32, screen.1.max(1) as f32);

    let mut push = |tx: usize, ty: usize, li: usize, counts: &mut Vec<u32>, indices: &mut Vec<u32>| {
        let t = ty * TILE_COLS + tx;
        let c = counts[t] as usize;
        if c < TILE_CAP {
            indices[t * TILE_CAP + c] = li as u32;
            counts[t] += 1;
        }
        // Tile full: drop. 64 overlapping influence spheres on one tile is
        // beyond any authored scene today; revisit with L2 depth slicing.
    };

    for (li, l) in lights.iter().enumerate() {
        let clip = *view_proj * l.pos.extend(1.0);
        // Camera actually inside the influence sphere: the light can wrap
        // the whole view. Conservative: all tiles. (Distance test, NOT
        // clip.w - a behind-camera light has negative w, which must SKIP,
        // not wrap; the first test run caught exactly that smear.)
        if cam_pos.distance(l.pos) <= l.range {
            for ty in 0..TILE_ROWS {
                for tx in 0..TILE_COLS {
                    push(tx, ty, li, &mut counts, &mut indices);
                }
            }
            continue;
        }
        if clip.w <= 0.0 {
            continue; // behind the camera and not enclosing: invisible
        }
        let ndc = clip.xyz() / clip.w;
        // Projected SILHOUETTE radius in NDC, exact tangent form: for a
        // camera at euclidean distance d OUTSIDE the sphere (the wrap test
        // above guarantees d > range), the silhouette's angular tangent is
        // range / sqrt(d^2 - range^2). Two wrong versions preceded this:
        // dividing by clip.w (center depth) under-included near lights (hard
        // tile seams cutting light pools in the first lit capture), and
        // clip.w - range exploded for lights BESIDE the camera (small axis
        // depth, large lateral distance), flooding every tile past TILE_CAP
        // and dropping the visible lights (the second capture's black frame).
        let d = cam_pos.distance(l.pos);
        let tangent = l.range / (d * d - l.range * l.range).max(1e-4).sqrt();
        let rx = (tangent * view_proj.col(0).x.abs().max(1e-4) * 1.2).min(2.5);
        let ry = (tangent * view_proj.col(1).y.abs().max(1e-4) * 1.2).min(2.5);
        // NDC -> pixel rect (y flips).
        let x0 = (ndc.x - rx) * 0.5 + 0.5;
        let x1 = (ndc.x + rx) * 0.5 + 0.5;
        let y0 = 0.5 - (ndc.y + ry) * 0.5;
        let y1 = 0.5 - (ndc.y - ry) * 0.5;
        if x1 < 0.0 || x0 > 1.0 || y1 < 0.0 || y0 > 1.0 {
            continue; // fully off screen
        }
        let tx0 = ((x0 * sw) / (sw / TILE_COLS as f32)).floor().max(0.0) as usize;
        let tx1 = ((x1 * sw) / (sw / TILE_COLS as f32)).floor().min(TILE_COLS as f32 - 1.0) as usize;
        let ty0 = ((y0 * sh) / (sh / TILE_ROWS as f32)).floor().max(0.0) as usize;
        let ty1 = ((y1 * sh) / (sh / TILE_ROWS as f32)).floor().min(TILE_ROWS as f32 - 1.0) as usize;
        for ty in ty0..=ty1.min(TILE_ROWS - 1) {
            for tx in tx0..=tx1.min(TILE_COLS - 1) {
                push(tx, ty, li, &mut counts, &mut indices);
            }
        }
    }
    (counts, indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn look_at_proj() -> Mat4 {
        let proj = Mat4::perspective_rh(60f32.to_radians(), 16.0 / 9.0, 0.1, 10_000.0);
        let view = Mat4::look_at_rh(Vec3::ZERO, Vec3::NEG_Z, Vec3::Y);
        proj * view
    }

    /// A small light dead ahead lands in the center tiles and ONLY around
    /// the center; a light behind the camera lands nowhere.
    #[test]
    fn center_light_bins_centrally_and_behind_bins_nowhere() {
        let vp = look_at_proj();
        let ahead = BinLight { pos: Vec3::new(0.0, 0.0, -50.0), range: 2.0 };
        let behind = BinLight { pos: Vec3::new(0.0, 0.0, 50.0), range: 2.0 };
        let (counts, indices) = bin_lights(&[ahead, behind], &vp, Vec3::ZERO, (1920, 1080));
        let center = (TILE_ROWS / 2) * TILE_COLS + TILE_COLS / 2;
        assert!(counts[center] >= 1, "center tile must contain the ahead light");
        assert_eq!(indices[center * TILE_CAP], 0, "and it is light index 0");
        let total: u32 = counts.iter().sum();
        assert!(total < 12, "a tiny distant light must not smear far ({total} entries)");
        // The behind light contributed nothing anywhere.
        for t in 0..TILE_COUNT {
            for c in 0..counts[t] as usize {
                assert_ne!(indices[t * TILE_CAP + c], 1, "behind-camera light leaked into tile {t}");
            }
        }
    }

    /// The camera INSIDE a big light's range = every tile lists it (the
    /// conservative wrap case - exclusion here would black out the room the
    /// player is standing in).
    #[test]
    fn enclosing_light_reaches_every_tile() {
        let vp = look_at_proj();
        let room = BinLight { pos: Vec3::new(0.5, 1.0, 0.5), range: 8.0 };
        let (counts, _) = bin_lights(&[room], &vp, Vec3::ZERO, (1920, 1080));
        assert!(counts.iter().all(|&c| c >= 1), "enclosing light must be in every tile");
    }

    /// Off-to-the-side light touches edge tiles, not the far side.
    #[test]
    fn side_light_stays_on_its_side() {
        let vp = look_at_proj();
        // To the right of the view axis, ahead of the camera.
        let side = BinLight { pos: Vec3::new(30.0, 0.0, -60.0), range: 5.0 };
        let (counts, _) = bin_lights(&[side], &vp, Vec3::ZERO, (1920, 1080));
        let left_half: u32 = (0..TILE_ROWS)
            .flat_map(|ty| (0..TILE_COLS / 2).map(move |tx| ty * TILE_COLS + tx))
            .map(|t| counts[t])
            .sum();
        let right_half: u32 = (0..TILE_ROWS)
            .flat_map(|ty| (TILE_COLS / 2..TILE_COLS).map(move |tx| ty * TILE_COLS + tx))
            .map(|t| counts[t])
            .sum();
        assert!(right_half >= 1, "the right-side light must reach right tiles");
        assert_eq!(left_half, 0, "and must not leak to the left half");
    }
}
