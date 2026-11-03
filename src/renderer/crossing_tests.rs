//! TESTS OF TERRAIN, TOGETHER WITH THE RENDERER (the crate split, 2026-10-31,
//! docs/design/crate-split.md): written beside terrain's code, they reach into this crate to do it,
//! so each moved up WHOLE to the lowest crate that may use everything it reaches. Nothing in a test
//! changed but its paths. From terrain/far_trees.rs: the canopy constants match the shader.

use humanity_terrain::far_trees::*;
use humanity_terrain::far_trees::FAR_CARD_SHADE_BIT;

/// The tint and the tone range live in TWO languages. The shader applies
/// them to the imagery fetch, this module bakes them into the fallback
/// colour, and a drift makes a planet with imagery and a planet without
/// grow visibly different forests.
#[test]
fn canopy_constants_match_the_shader() {
    let src = crate::renderer::shader_loader::assembled_pbr_source();
    let want_tint = format!(
        "const CANOPY_GROUND_TINT: vec3<f32> = vec3<f32>({:.2}, {:.2}, {:.2});",
        FAR_CARD_CANOPY_TINT[0], FAR_CARD_CANOPY_TINT[1], FAR_CARD_CANOPY_TINT[2]
    );
    assert!(
        src.contains(&want_tint),
        "the shader's CANOPY_GROUND_TINT does not match far_trees::FAR_CARD_CANOPY_TINT \
         (expected `{want_tint}`)"
    );
    for (name, v) in [
        ("CANOPY_CARD_SHADE_LO", FAR_CARD_SHADE_LO),
        ("CANOPY_CARD_SHADE_HI", FAR_CARD_SHADE_HI),
    ] {
        let want = format!("const {name}: f32 = {v:.2};");
        assert!(src.contains(&want), "the shader lost `{want}`");
    }
    // The shader decodes the tone from bits 18..21; this is the weight.
    assert_eq!(FAR_CARD_SHADE_BIT, 262_144.0);
    assert!(src.contains("(packed >> 18u) & 15u"));
    // And the two shade decoders agree at every code.
    for code in 0..=FAR_CARD_SHADE_LEVELS {
        let t = code as f32 / FAR_CARD_SHADE_LEVELS as f32;
        let want = FAR_CARD_SHADE_LO + (FAR_CARD_SHADE_HI - FAR_CARD_SHADE_LO) * t;
        assert!((far_card_shade(code) - want).abs() < 1e-6);
    }
}
