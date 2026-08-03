//! WHICH MATERIALS CAN DISCARD IN THE SUN SHADOW PASS (v0.1108).
//!
//! One predicate and the test that keeps it honest. It lives in its own file
//! rather than in `materials.rs` because it is not material registration: it
//! is the CPU half of a contract with `fs_shadow` in
//! `assets/shaders/pbr/90-fragment-main.wgsl`, and its sibling half (fs_main
//! vs fs_shadow) is already guarded next door in
//! `renderer::pipeline::shadow_cutout_tests`.
//!
//! THE FAILURE THIS EXISTS TO CATCH is the one v0.1106-v0.1107 shipped twice:
//! a discard that is written, reviewed and merged, and can never execute. A
//! cutout branch in fs_shadow is dead unless (a) the pass binds a texture it
//! can actually read - see `AlbedoBindGroup` - and (b) the caster draws with
//! the PSO that HAS a fragment stage, which is what this predicate decides.
//! Both halves are silent when wrong: the shadows just stay solid.

/// Does `fs_shadow` have a DISCARD for this material type?
///
/// The classic sun-shadow caster loop uses this to choose between the
/// depth-only PSO and the alpha-cutout one. Through v0.1107 it keyed on "the
/// material has an albedo texture", which is not the same question, and was
/// wrong in BOTH directions: baked bark (type 22) and textured planet meshes
/// are fully opaque and paid for a fragment stage that can never discard,
/// while an UNTEXTURED terrain-patch material (type 12 - the arena-overflow
/// patches that carry sprite tree cards) took the depth-only path even though
/// its cards discard on the atlas alpha.
///
/// The bands are the shader's own, written the way the shader writes them.
/// `cutout_type_bands_match_the_shader` FAILS if the two ever disagree,
/// including when someone adds a FIFTH cutout to fs_shadow - which would
/// otherwise be inert for every classic caster.
pub(super) fn type_casts_cutout_shadow(t: f32) -> bool {
    // 12 = planet surface / terrain patch (its sprite tree cards discard on
    // the atlas alpha), 19 = photoscanned foliage, 21 = baked cluster card.
    (t >= 11.5 && t < 12.5) || (t >= 18.5 && t < 19.5) || (t >= 20.5 && t < 21.5)
}

#[cfg(test)]
mod tests {
    use super::super::shader_loader::assembled_pbr_source;
    use super::type_casts_cutout_shadow;

    /// Every `material_type >= A && material_type < B` band inside fs_shadow.
    /// Read out of the SHADER rather than kept as a hand-maintained list here,
    /// so the test cannot agree with a stale copy of the answer.
    fn shader_bands() -> Vec<(f32, f32)> {
        let src = assembled_pbr_source();
        let at = src.find("fn fs_shadow").expect("fs_shadow entry point missing");
        let mut out = Vec::new();
        for part in src[at..].split("material_type >= ").skip(1) {
            let Some((lo, rest)) = part.split_once(" && material_type < ") else {
                continue;
            };
            let hi: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            let (Ok(lo), Ok(hi)) = (lo.trim().parse::<f32>(), hi.parse::<f32>()) else {
                continue;
            };
            out.push((lo, hi));
        }
        out
    }

    #[test]
    fn cutout_type_bands_match_the_shader() {
        let bands = shader_bands();
        assert!(
            bands.len() >= 3,
            "parsed {} type bands out of fs_shadow - the shader's band syntax \
             changed and this test is no longer reading it",
            bands.len()
        );
        // Every band the shader gates a discard on must select the cutout PSO.
        for (lo, hi) in &bands {
            let mid = (lo + hi) * 0.5;
            assert!(
                type_casts_cutout_shadow(mid),
                "fs_shadow discards for material_type in [{lo}, {hi}) but the \
                 classic caster loop would draw type {mid} with the DEPTH-ONLY \
                 shadow PSO, so that cutout can never run. Add the band to \
                 shadow_cutout::type_casts_cutout_shadow."
            );
        }
        // And nothing else may: a type paying for a fragment stage that cannot
        // discard is the v0.1107 cost (30 -> 23.8 fps) charged for nothing.
        // These are shipped opaque types - 0 panel, 1 metal, 3 wood, 18 gas
        // giant, 20 near-tree foliage mesh, 22 baked bark.
        for t in [0.0_f32, 1.0, 3.0, 18.0, 20.0, 22.0] {
            let in_band = bands.iter().any(|(lo, hi)| t >= *lo && t < *hi);
            assert_eq!(
                type_casts_cutout_shadow(t),
                in_band,
                "material type {t} disagrees: shader-has-a-discard = {in_band}, \
                 CPU selector = {}",
                type_casts_cutout_shadow(t)
            );
        }
    }
}
