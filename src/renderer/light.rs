//! Light TYPES registry (v0.571): the data-driven catalog of placeable lights, loaded from
//! `data/lighting/light_types.ron`. Mirrors the `wall_materials` / `lock_types` pattern
//! (`include_str!` + `OnceLock` + lookup-by-id). Pure serde/data -- no GPU types -- so it parses
//! everywhere.
//!
//! The renderer's PBR shader already evaluates up to 8 POINT lights (pos + colour + intensity +
//! range), plus a directional sun + fill. Stage 1 places lights as DATA and resolves them into that
//! existing point-light path; `kind` carries Spot/Bar/Emissive for later stages (the shader gains the
//! cone/length maths then), but today every placed light is uploaded as a point light.

use serde::{Deserialize, Serialize};

/// What the light is (a fixed shader capability, so a closed enum -- adding a kind needs shader work,
/// per infinite-of-X's "closed set with code cost" exception). Stage 1 renders all as Point.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum LightKind {
    #[default]
    Point,
    /// A cone light (cone_*_deg used). Shader support: a later stage.
    Spot,
    /// A linear/area light (length_m used). Shader support: a later stage.
    Bar,
    /// A glowing surface that also lights the room (a TV). Synthesized from an emissive surface later.
    Emissive,
}

/// One entry in the light catalog. Add a type by adding a line to `light_types.ron`.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct LightType {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: LightKind,
    /// Linear RGB colour.
    pub color: (f32, f32, f32),
    /// Brightness multiplier (the shader's inverse-square term scales from here).
    pub intensity: f32,
    /// Falloff radius in metres (the light fades to nothing by here).
    pub range: f32,
    /// Spot cone (degrees) -- inner = full bright, outer = edge. Unused for Point/Bar. (later stage)
    #[serde(default)]
    pub cone_inner_deg: f32,
    #[serde(default)]
    pub cone_outer_deg: f32,
    /// Bar length in metres. Unused for Point/Spot. (later stage)
    #[serde(default)]
    pub length_m: f32,
    #[serde(default)]
    pub note: String,
}

/// The light catalog, parsed once from the embedded RON.
pub fn light_types() -> &'static [LightType] {
    static REG: std::sync::OnceLock<Vec<LightType>> = std::sync::OnceLock::new();
    REG.get_or_init(|| {
        const SRC: &str = include_str!("../../data/lighting/light_types.ron");
        match ron::from_str::<Vec<LightType>>(SRC) {
            Ok(v) => v,
            Err(e) => {
                log::error!("light_types.ron parse error: {e}");
                Vec::new()
            }
        }
    })
}

/// Look up a light type by its `id` (what a placed light stores).
pub fn light_type(id: &str) -> Option<&'static LightType> {
    light_types().iter().find(|t| t.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_type_registry_parses_and_has_a_point_preset() {
        let types = light_types();
        assert!(!types.is_empty(), "expected the seeded catalog");
        let p = types.iter().find(|t| t.kind == LightKind::Point).expect("a Point preset exists");
        assert!(p.intensity > 0.0 && p.range > 0.0, "a usable point light");
        assert!(light_type(&p.id).is_some());
        assert!(light_type("nope").is_none());
    }
}
