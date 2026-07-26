//! Per-ITEM-TYPE LOD category registry (v0.971 generalization of the
//! vegetation rung-2a registry): every kind of thing with detail stages -
//! vegetation, creatures, water, planet terrain - gets a row in
//! `data/lod/categories.ron` (embedded-registry pattern, like lock_types).
//! Rows whose stages are live carry real distances; rows whose controls
//! live elsewhere carry a controls_note the Settings block renders, so the
//! "Detail distances by item type" section lists EVERY type honestly.
//! Ungated (pure serde) so every feature set compiles it.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct LodCategory {
    pub id: String,
    pub label: String,
    /// Full 3D models within this range (metres). 0 = category has no model stage.
    pub model_m: f32,
    /// Flat silhouette cards out to this range. 0 = no card stage.
    pub card_m: f32,
    /// Alpha-billboard mid-stage ceiling (rung 2c). 0 = none yet.
    pub billboard_m: f32,
    /// Crossfade seconds between stages (rides RenderObject.fade).
    pub fade_s: f32,
    /// Where this type's live controls are, when they are NOT the generic
    /// model/card sliders (e.g. water's mesh-detail slider, the planet
    /// terrain sliders). Empty = the stage distances above are the story.
    #[serde(default)]
    pub controls_note: String,
}

/// The category table, parsed once from the embedded RON.
pub fn categories() -> &'static [LodCategory] {
    static REG: std::sync::OnceLock<Vec<LodCategory>> = std::sync::OnceLock::new();
    REG.get_or_init(|| {
        const SRC: &str = include_str!("../data/lod/categories.ron");
        match ron::from_str::<Vec<LodCategory>>(SRC) {
            Ok(v) if !v.is_empty() => v,
            Ok(_) | Err(_) => {
                log::error!("data/lod/categories.ron missing/invalid; using a tree-only fallback");
                vec![LodCategory {
                    id: "tree".into(),
                    label: "Trees".into(),
                    model_m: 120.0,
                    card_m: 1500.0,
                    billboard_m: 0.0,
                    fade_s: 0.3,
                    controls_note: String::new(),
                }]
            }
        }
    })
}

/// Lookup by id.
pub fn category(id: &str) -> Option<&'static LodCategory> {
    categories().iter().find(|c| c.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The registry must parse, carry the live categories, and the TREE row
    /// must match the pre-registry settings defaults exactly (rung 2a's
    /// zero-visual-change contract: moving the defaults into data must not
    /// move the picture).
    #[test]
    fn registry_parses_and_tree_matches_the_legacy_defaults() {
        let cats = categories();
        assert!(cats.len() >= 7, "expected the seven categories (five vegetation/creature + water + planet), got {}", cats.len());
        for c in cats {
            assert!(!c.label.is_empty() && c.fade_s > 0.0, "category {:?} incomplete", c.id);
        }
        let tree = category("tree").expect("tree category present");
        assert_eq!(tree.model_m, 120.0, "tree model distance must match the legacy default");
        assert_eq!(tree.card_m, 1500.0, "tree card distance must match the legacy default");
        assert!(category("grass").is_some(), "grass category present (rung 2b consumes it)");
        // v0.971: the non-vegetation types are listed with their control
        // locations so the Settings section covers everything honestly.
        for id in ["water", "planet"] {
            let c = category(id).expect("generalized category present");
            assert!(
                !c.controls_note.is_empty(),
                "{id} must say where its live controls are"
            );
        }
    }
}
