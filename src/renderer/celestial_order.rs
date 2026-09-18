//! The compositing order of the celestial TRANSPARENT list (2026-09-18).
//!
//! Alpha-blended objects without depth sorting composite in the order they
//! are drawn, so the order of `celestial_transparent` in lib.rs is a
//! picture-affecting rule, not a detail. The rules the frame loop authors:
//!
//! - The planet's LAYER STACK, bottom to top as pushed: the ocean backstop
//!   and the ocean shell (type 16), then the cloud deck (15) and the
//!   atmosphere (14 scattering, or 13, the Fresnel fallback dome when
//!   Settings > Graphics > Planets > "Scattering atmosphere" is off), in the
//!   cloud/atmosphere order the v0.997 approach-vanish fix chose per
//!   altitude (atmosphere first when the camera is inside the air, cloud
//!   first when it is outside). That relative order must NEVER change.
//! - Water sinks to the very end whenever the camera is inside the air or
//!   underwater (`water_over_sky`), because the depth-writing water pipeline
//!   (v0.1060) is only correct when water is drawn last.
//!
//! Since increment P2 of the frame-cost arc the list is also drawn through a
//! pipeline picked per object by material class (`pipeline::shader_class`),
//! and grouping the classes keeps the pipeline switches to a handful per
//! frame. P2 first grouped on the CLASS itself, which put the type-13
//! fallback dome (General class: its branch is in the general surface
//! chain) in front of the cloud and water shells, so with scattering off the
//! haze composited UNDER the sea and the deck (found by the P2 review). The
//! grouping key is therefore the planet LAYER BAND, not the class: every
//! object of types 13..16 keeps the authored stack order, everything else
//! (the sun's blended core and halo, gas giant bands) draws before the
//! stack. The draw loop still switches pipelines only when the class
//! changes, so a General dome inside the stack costs one extra switch and
//! nothing else.

/// True for the planet layer stack: 13 fallback dome, 14 scattering
/// atmosphere, 15 cloud deck, 16 ocean shell and backstop. Banded at
/// +-0.5 like every material-type test in the shader.
#[inline]
pub fn is_planet_layer(material_type: f32) -> bool {
    (12.5..16.5).contains(&material_type)
}

/// The stable-sort key for `celestial_transparent`: layers after
/// non-layers, and (when water goes last) water after the rest. A stable
/// sort on this key never reorders two objects with equal keys, which is
/// what keeps the authored cloud/atmosphere order intact.
#[inline]
pub fn celestial_transparent_key(material_type: f32, is_water: bool) -> (bool, bool) {
    (is_planet_layer(material_type), is_water)
}

/// The key for one object of the real list: reads the material's CPU-side
/// type copy (a private field of `Material`, visible here because this
/// module is a child of `renderer`) and answers General-shaped (not a
/// layer) for a missing index, which every draw loop skips anyway.
#[inline]
pub fn key_for(renderer: &super::Renderer, material: usize, is_water: bool) -> (bool, bool) {
    let mt = renderer.materials.get(material).map_or(0.0, |m| m.material_type);
    celestial_transparent_key(mt, is_water)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny stand-in for the real list: (name, material type, is water).
    fn order(list: &[(&'static str, f32, bool)]) -> Vec<&'static str> {
        let mut v = list.to_vec();
        v.sort_by_key(|(_, t, w)| celestial_transparent_key(*t, *w));
        v.into_iter().map(|(n, _, _)| n).collect()
    }

    #[test]
    fn the_band_is_exactly_the_four_layer_types() {
        for t in [13.0, 14.0, 15.0, 16.0] {
            assert!(is_planet_layer(t), "type {t} is a planet layer");
        }
        for t in [0.0, 11.0, 12.0, 17.0, 18.0, 19.0, 24.0] {
            assert!(!is_planet_layer(t), "type {t} is not a planet layer");
        }
        // The half-step edges, the same +-0.5 banding the shader uses.
        assert!(is_planet_layer(12.5) && !is_planet_layer(12.49));
        assert!(!is_planet_layer(16.5) && is_planet_layer(16.49));
    }

    /// The P2 review's failure: scattering OFF (type-13 dome), camera
    /// outside the atmosphere, clouds on. The authored order is sun,
    /// backstop, water, cloud, dome; the grouping must not lift the dome
    /// (a General-class object) above the sea and the clouds.
    #[test]
    fn a_fallback_dome_keeps_its_place_above_the_clouds_and_the_sea() {
        let list = [
            ("sun", 17.0, false),
            ("backstop", 16.0, false),
            ("water", 16.0, false),
            ("cloud", 15.0, false),
            ("dome13", 13.0, false),
        ];
        assert_eq!(order(&list), vec!["sun", "backstop", "water", "cloud", "dome13"]);
    }

    /// Inside the air (atmosphere pushed before the cloud) with water going
    /// last: water and its backstop sink to the end, the atmosphere/cloud
    /// order survives, the sun stays first. This is the pre-P2 result.
    #[test]
    fn water_last_sinks_water_and_keeps_the_stack_order() {
        let list = [
            ("sun", 17.0, false),
            ("backstop", 16.0, true),
            ("water", 16.0, true),
            ("atmo14", 14.0, false),
            ("cloud", 15.0, false),
        ];
        assert_eq!(order(&list), vec!["sun", "atmo14", "cloud", "backstop", "water"]);
    }

    /// A non-layer object pushed AFTER the stack (a gas giant's bands, a
    /// later body's halo) is lifted in front of it, which is the grouping
    /// P2 wanted; the stack's own order is untouched by that lift.
    #[test]
    fn non_layers_group_before_the_stack_without_reordering_it() {
        let list = [
            ("cloud", 15.0, false),
            ("atmo14", 14.0, false),
            ("bands", 17.0, false),
        ];
        assert_eq!(order(&list), vec!["bands", "cloud", "atmo14"]);
    }
}
