//! Which bodies are skinned how (plan section 8.4): the nearest, up to a budgeted count, are skinned
//! once a frame by a compute pass (their skinned vertices then serve the depth, shadow and colour
//! passes alike); the rest out to the middle distance skin in the vertex shader, which costs no
//! memory. Rung 4 adds the far tier's baked animation; until then a body past the middle distance
//! is drawn as a middle one.

use serde::Deserialize;

/// The tier limits: `data/characters/characters.ron`'s `tiers`.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Tiers {
    /// Within this many metres a body may be near (compute-skinned).
    pub near_m: f32,
    /// Out to this many metres a body is middle (vertex-shader skinned); rung 4's far tier begins
    /// here (`CREW_NAME_DIST`'s 40 m, where the nameplate retires too).
    pub middle_m: f32,
    /// At most this many near bodies (plan 8.3: about 1 MB of skinned vertices each at full
    /// detail, so the near tier is capped).
    pub near_cap: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawTier {
    Near,
    Middle,
}

/// The tier of each body, by its distance from the camera: the nearest first take the near
/// places while they last and are within `near_m`; every other body is middle.
pub fn assign(distances: &[f32], tiers: &Tiers) -> Vec<DrawTier> {
    let mut order: Vec<usize> = (0..distances.len()).collect();
    order.sort_by(|&a, &b| distances[a].total_cmp(&distances[b]));
    let mut out = vec![DrawTier::Middle; distances.len()];
    for (taken, &i) in order.iter().enumerate() {
        if taken >= tiers.near_cap || !(distances[i] <= tiers.near_m) {
            break;
        }
        out[i] = DrawTier::Near;
    }
    out
}
