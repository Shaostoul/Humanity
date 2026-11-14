//! HASHED ALPHA under temporal anti-aliasing: rung 1b of the renderer roadmap (2026-11-13; the plan
//! of record is docs/design/renderer-roadmap.md sections 3.3 and 12).
//!
//! WHAT IT DOES. Before temporal anti-aliasing (rung 1a) a partly transparent cutout could only be
//! cut hard (a leaf's alpha under 0.5 discarded) or blended (which needs sorting, writes no depth
//! or motion, and casts no true shadow). So card hair drew twice (Scheuermann 2004: an opaque core
//! at alpha 0.8 and up, a blended fringe in the transparent list), the player's own fringe could not
//! show on a planet (their body stands in render space, which has no transparent list), the far
//! tree cards stair-stepped and shimmered at their hard cut, and a level-of-detail swap dissolved
//! through a fixed 4 x 4 Bayer screen door that stood still on the screen while it played.
//!
//! With the resolve averaging frames, a cutout can instead keep or drop each pixel by comparing its
//! alpha with a threshold that changes per pixel and per frame (HASHED ALPHA TESTING, Wyman and
//! McGuire 2017, "Hashed Alpha Testing", I3D; Unreal's dithered opacity mask is the same idea): the
//! history averages the kept and the dropped into the authored coverage, so the edge is soft, the
//! surface writes depth and motion like any opaque one, and nothing needs sorting. Under Temporal:
//! - CARD HAIR draws in ONE opaque pass: at or over the cut (0.8) a fragment is kept as before, under
//!   it kept where its alpha beats the threshold, so the expected coverage is exactly what the two
//!   passes blended (alpha under the cut, solid over it). The fringe pass is not drawn in that view
//!   (`Renderer::hashed_skips`), and the player's own fringe now shows on a planet.
//! - The FAR TREE CARDS (type 12, the baked atlas) keep a pixel where the sprite's alpha beats the
//!   threshold instead of cutting at 0.5, and the card stage's far end dissolves over its last
//!   [`CARD_FAR_BAND`] of distance instead of ending at a hard ring.
//! - Every LEVEL-OF-DETAIL CROSSFADE (terrain patches, the near trees fading in) takes the threshold
//!   in place of the Bayer table, so the dissolve the history resolves is a smooth blend. A rising
//!   level keeps where the threshold is under its fade and the falling one where it is not, with the
//!   same threshold, so the two cover each pixel exactly once ([`fade_kept`]).
//!
//! THE THRESHOLD ([`threshold`], twinned in 00-bindings-vertex.wgsl `taa_hash_threshold`): Jimenez's
//! interleaved gradient noise (2014, "Next Generation Post Processing in Call of Duty: Advanced
//! Warfare", SIGGRAPH Advances) gives each pixel a value spread evenly over its 3 x 3 neighbours, the
//! neighbourhood the resolve clips its history to, so a half-covered patch reads as half covered in
//! every neighbourhood and the history survives the clip. The jitter phase (0 to 7) adds
//! `(phase * 5 mod 8) / 8`, so over the eight phases a still pixel takes ONE value in each eighth of
//! [0, 1): its coverage averaged over the eight frames is the authored alpha to within an eighth,
//! and over its neighbours exactly. The pattern depends only on the pixel and the phase, so a still
//! pixel repeats it every eight frames: stable, not noise.
//!
//! WHEN IT IS OFF (today's paths, untouched): anti-aliasing Off; the showcase `taa_hashed` "0" (the
//! red twin: Temporal with the two passes and the hard cuts); the reuse switch off (the history is
//! discarded every frame, so nothing would average the pattern); the off-screen views (camera
//! screens and the hi-res capture, which have no resolve) and the shadow passes, which keep the hard
//! cuts and the Bayer table (a shadow texel is filtered by PCF, not by a history).

use super::*;

/// Interleaved gradient noise's constants (Jimenez 2014): `fract(C.z * fract(dot(pixel, C.xy)))`.
pub const IGN: [f32; 3] = [0.067_110_56, 0.005_837_15, 52.982_918_9];

/// The phases a still pixel's threshold cycles through: the jitter's eight (taa::JITTER_PHASES).
pub const PHASES: u32 = JITTER_PHASES;

/// The step through the eight strata from one phase to the next (5 is coprime with 8, so the eight
/// phases visit every stratum once, and consecutive frames sit far apart).
pub const STRATUM_STEP: u32 = 5;

/// The hair's floor: under this alpha a card fragment is never drawn (the fringe pass's own floor,
/// so the faint haze at a card's outer edge does not speckle).
pub const HAIR_FLOOR: f32 = 0.03;

/// The share of the far tree cards' range over which their stage dissolves out under hashed alpha
/// (the last tenth before the "Tree silhouette distance" cutoff).
pub const CARD_FAR_BAND: f32 = 0.1;

fn fract(x: f32) -> f32 {
    x - x.floor()
}

/// Interleaved gradient noise at a pixel, in [0, 1).
pub fn ign(px: u32, py: u32) -> f32 {
    fract(IGN[2] * fract(IGN[0] * px as f32 + IGN[1] * py as f32))
}

/// The hashed-alpha threshold of a pixel at a jitter phase, in [0, 1). A fragment with alpha `a` is
/// kept where `a > threshold`.
pub fn threshold(px: u32, py: u32, phase: u32) -> f32 {
    let stratum = (phase % PHASES) * STRATUM_STEP % PHASES;
    fract(ign(px, py) + stratum as f32 / PHASES as f32)
}

/// A plain cutout under hashed alpha (the far tree cards): kept where the alpha beats the threshold.
pub fn foliage_kept(alpha: f32, h: f32) -> bool {
    alpha > h
}

/// The hair card's one pass under hashed alpha: solid at or over the cut, as the core always was;
/// under it, kept where the alpha beats the threshold (what the blended fringe drew, on average);
/// never under [`HAIR_FLOOR`].
pub fn hair_kept(alpha: f32, cut: f32, h: f32) -> bool {
    alpha >= cut || (alpha >= HAIR_FLOOR && alpha > h)
}

/// A level-of-detail crossfade: a rising level (`fade` > 0) keeps where the threshold is under its
/// fade, the falling partner (`-fade`) where it is not. 0 = not fading.
pub fn fade_kept(fade: f32, h: f32) -> bool {
    if fade > 0.0 {
        h < fade
    } else if fade < 0.0 {
        h >= -fade
    } else {
        true
    }
}

/// The far tree cards' coverage at a distance under hashed alpha: 1 inside the stage, falling to 0
/// over its last [`CARD_FAR_BAND`] before the cutoff `far_m`. The shader tests the sprite's alpha
/// times this against the threshold, so a card's coverage is the two multiplied.
pub fn card_far_coverage(dist_m: f32, far_m: f32) -> f32 {
    let band = (far_m * CARD_FAR_BAND).max(1.0e-3);
    ((far_m - dist_m) / band).clamp(0.0, 1.0)
}

impl Renderer {
    /// Whether hashed alpha replaces the hard cutouts and the hair's two passes this frame (the
    /// main view under Temporal, the showcase `taa_hashed` not "0", the reuse switch on).
    pub fn taa_hashed(&self) -> bool {
        self.taa_on() && self.taa.hashed && humanity_core::reuse::enabled()
    }

    /// Whether a transparent-list draw is left out because hashed alpha draws it in the opaque pass
    /// instead: the hair's blended fringe (type 26 with a negative cut), in the main view's passes
    /// while this frame's camera says hashed alpha is on (`taa_camera` stamps the same flag, so the
    /// core's shader and this skip always agree; an off-screen view draws the fringe as before).
    pub(crate) fn hashed_skips(&self, material: &Material) -> bool {
        let f = self.taa.now.get();
        f.live && f.hashed && !self.offscreen_view_active() && is_hair_fringe(material.material_type, material.params2)
    }
}

/// A material is a hair card's blended fringe pass (renderer/materials.rs MATERIAL_TYPE_HAIR, its
/// `params2.z` negative).
pub fn is_hair_fringe(material_type: f32, params2: [f32; 4]) -> bool {
    (material_type - materials::MATERIAL_TYPE_HAIR).abs() < 0.5 && params2[2] < 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALPHAS: [f32; 7] = [0.05, 0.1, 0.25, 0.5, 0.62, 0.73, 0.9];

    /// Red first (2026-11-13, the twin built standalone with the break behind a cfg): against
    /// today's dither, a threshold of the pixel alone (the 4 x 4 Bayer table, no phase), this failed
    /// with "pixel (0, 0) alpha 0.05: covered 8 of 8 frames, wants 0.05 within an eighth", since a
    /// still pixel then keeps or drops on every frame alike and the history has nothing to average
    /// (the next three tests failed against it too).
    #[test]
    fn a_still_pixels_eight_frame_coverage_is_the_authored_alpha() {
        for &a in &ALPHAS {
            let (mut sum, mut n) = (0.0f64, 0u32);
            for py in 0..64u32 {
                for px in 0..64u32 {
                    let kept = (0..PHASES).filter(|&k| foliage_kept(a, threshold(px, py, k))).count();
                    let cov = kept as f32 / PHASES as f32;
                    assert!(
                        (cov - a).abs() <= 1.0 / PHASES as f32 + 1.0e-6,
                        "pixel ({px}, {py}) alpha {a}: covered {kept} of 8 frames, wants {a} within an eighth"
                    );
                    sum += cov as f64;
                    n += 1;
                }
            }
            let mean = sum / n as f64;
            assert!((mean - a as f64).abs() < 0.01, "alpha {a}: a 64 x 64 patch over eight frames covers {mean}");
        }
    }

    /// The pattern is the pixel's and the phase's alone: the same still pixel at the same phase eight
    /// frames later has the same threshold, and the eight phases put it once in each eighth of [0, 1).
    /// Red first: with the phase step 4 (not coprime with 8) this failed with "pixel (0, 0): the
    /// eight phases fill strata [4, 0, 0, 0, 4, 0, 0, 0] twice and miss others" (and the coverage
    /// test above with "covered 4 of 8 frames").
    #[test]
    fn a_still_pixels_threshold_repeats_with_the_jitter_phase_and_fills_every_stratum() {
        for py in 0..16u32 {
            for px in 0..16u32 {
                let mut strata = [0u32; PHASES as usize];
                for k in 0..PHASES {
                    let h = threshold(px, py, k);
                    assert!((0.0..1.0).contains(&h), "threshold {h} out of [0, 1)");
                    assert_eq!(h.to_bits(), threshold(px, py, k + PHASES).to_bits(), "pixel ({px}, {py}) phase {k}");
                    strata[((h * PHASES as f32) as usize).min(PHASES as usize - 1)] += 1;
                }
                assert!(strata.iter().all(|&c| c == 1), "pixel ({px}, {py}): the eight phases fill strata {strata:?} twice and miss others");
            }
        }
    }

    /// A crossfade's two levels cover every pixel exactly once in every frame, and the rising one
    /// covers its fade on average. Red first: with the falling level keeping `h > -fade` (a strict
    /// compare), a pixel whose threshold equals the fade was covered by neither level: "fade 0.125,
    /// pixel (0, 0) phase 5: covered 0 times" (its noise is 0 and phase 5 adds one eighth).
    #[test]
    fn a_crossfades_coverage_sums_to_one_across_the_two_levels() {
        for fi in 1..32 {
            let t = fi as f32 / 32.0;
            let mut rising = 0u32;
            let mut n = 0u32;
            for py in 0..32u32 {
                for px in 0..32u32 {
                    for k in 0..PHASES {
                        let h = threshold(px, py, k);
                        let both = fade_kept(t, h) as u32 + fade_kept(-t, h) as u32;
                        assert_eq!(both, 1, "fade {t}, pixel ({px}, {py}) phase {k}: covered {both} times");
                        rising += fade_kept(t, h) as u32;
                        n += 1;
                    }
                }
            }
            let cov = rising as f32 / n as f32;
            assert!((cov - t).abs() < 0.01, "fade {t}: the rising level covers {cov}");
        }
        // Both thresholds the stage exactly at the stratum edges too.
        assert!(fade_kept(0.5, 0.49) && !fade_kept(-0.5, 0.49));
        assert!(!fade_kept(0.5, 0.5) && fade_kept(-0.5, 0.5));
    }

    /// The hair's one pass covers on average exactly what the two passes drew: solid at or over the
    /// cut, the alpha under it, nothing under the floor.
    #[test]
    fn the_hair_pass_covers_what_the_two_passes_blended() {
        let cut = materials::HAIR_CUTOFF;
        for &a in &[0.01f32, 0.02, 0.05, 0.3, 0.5, 0.79, 0.8, 0.85, 1.0] {
            let mut kept = 0u32;
            let mut n = 0u32;
            for py in 0..64u32 {
                for px in 0..64u32 {
                    for k in 0..PHASES {
                        kept += hair_kept(a, cut, threshold(px, py, k)) as u32;
                        n += 1;
                    }
                }
            }
            let cov = kept as f32 / n as f32;
            let want = if a >= cut { 1.0 } else if a < HAIR_FLOOR { 0.0 } else { a };
            assert!((cov - want).abs() < 0.01, "alpha {a}: one pass covers {cov}, the two passes {want}");
        }
    }

    #[test]
    fn the_far_cards_dissolve_over_the_last_tenth_of_their_range() {
        assert_eq!(card_far_coverage(500.0, 1000.0), 1.0);
        assert_eq!(card_far_coverage(900.0, 1000.0), 1.0);
        assert!((card_far_coverage(950.0, 1000.0) - 0.5).abs() < 1.0e-6);
        assert_eq!(card_far_coverage(1000.0, 1000.0), 0.0);
    }

    #[test]
    fn only_the_hair_fringe_is_left_to_the_opaque_pass() {
        let hair = materials::MATERIAL_TYPE_HAIR;
        assert!(is_hair_fringe(hair, [1.0, 1.0, -materials::HAIR_CUTOFF, 0.4]));
        assert!(!is_hair_fringe(hair, [1.0, 1.0, materials::HAIR_CUTOFF, 0.4]), "the core is not skipped");
        assert!(!is_hair_fringe(materials::MATERIAL_TYPE_CHARACTER, [1.0, 1.0, -0.8, 0.4]), "cloth is not hair");
    }

    /// The shader's threshold, the hair's one pass, the far cards and the crossfade are the twins
    /// above (the WGSL is read the way the compiler reads it).
    #[test]
    fn the_shader_hash_is_the_twin() {
        let flat = |s: &str| -> String { s.replace("\r\n", "\n").split_whitespace().collect() };
        let bindings = flat(include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/assets/shaders/pbr/00-bindings-vertex.wgsl")));
        assert!(bindings.contains("taa_hash:vec4<f32>,"), "the camera carries the hashed-alpha row");
        assert!(bindings.contains("fract(52.9829189*fract(dot(p,vec2<f32>(0.06711056,0.00583715))))"), "interleaved gradient noise, with the twin's constants");
        assert!(bindings.contains("fract(ign+f32((k*5u)%8u)*0.125)"), "the phase's stratum step, 5 of 8");
        let shared = flat(include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/assets/shaders/pbr/80-fragment-shared.wgsl")));
        assert!(shared.contains("if(h>=lod_fade){discard;}") && shared.contains("if(h<-lod_fade){discard;}"), "the crossfade's two levels share one threshold");
        let main = flat(include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/assets/shaders/pbr/90-fragment-main.wgsl")));
        assert!(main.contains("if(ch_col.a<ch_cut&&(ch_col.a<0.03||ch_col.a<=taa_hash_threshold(in.clip_position.xy))){"), "the hair's one pass");
        assert!(main.contains("if(spr.a*card_far<=taa_hash_threshold(in.clip_position.xy)){"), "the far cards' hashed cut, the sprite's alpha times the far band's coverage");
        assert!(main.contains("card_dist)/max(shadow_u.params2.x*0.1,1.0e-3)"), "the far cards' dissolve band, CARD_FAR_BAND");
        assert_eq!(CARD_FAR_BAND, 0.1);
        assert_eq!(HAIR_FLOOR, 0.03);
    }
}
