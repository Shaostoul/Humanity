//! Hand tools a craft needs (2026-09-26; crafting depth, rung 1 of the
//! gameplay arc, docs/design/gameplay-gaps-2026-09-25.md).
//!
//! A tool is not consumed. Each manual craft wears every tool it names by one
//! use, and a tool breaks when its uses reach its items.csv `durability`. The
//! tools must be in the backpack (you carry the hammer to the bench). An
//! automated machine needs none: the machine is the tool.
//!
//! Which recipes need which tools is data (`data/crafting/tools.ron`): rules
//! by station and category, and per-recipe lists that override them. The
//! same resolution serves the crafting system and the Crafting page, so the
//! page never offers a craft the system would refuse.

use std::collections::HashMap;

use serde::Deserialize;

/// One rule: recipes made at `station` (and, when `category` is not empty,
/// of that category) need `tools`.
#[derive(Debug, Clone, Deserialize)]
pub struct ToolRule {
    pub station: String,
    #[serde(default)]
    pub category: String,
    pub tools: Vec<String>,
}

/// `data/crafting/tools.ron`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ToolRules {
    #[serde(default)]
    pub rules: Vec<ToolRule>,
    /// Recipe id to its tools, overriding the rules (an empty list = none).
    #[serde(default)]
    pub recipes: HashMap<String, Vec<String>>,
}

impl ToolRules {
    pub const FILE: &'static str = "crafting/tools.ron";

    pub fn from_ron(bytes: &[u8]) -> Result<Self, String> {
        let s = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
        ron::from_str(s).map_err(|e| format!("{}: {e}", Self::FILE))
    }

    /// The tools recipe `id`, made at `station` (empty = by hand) in
    /// `category`, needs. A recipe that MAKES one of the tools never needs
    /// that tool itself, so the first one can always be made.
    pub fn tools_for(&self, id: &str, station: &str, category: &str, outputs: &[(String, u32)]) -> Vec<String> {
        let listed = self.recipes.get(id).cloned().or_else(|| {
            self.rules
                .iter()
                .find(|r| !station.is_empty() && r.station == station && (r.category.is_empty() || r.category == category))
                .map(|r| r.tools.clone())
        });
        listed
            .unwrap_or_default()
            .into_iter()
            .filter(|t| !outputs.iter().any(|(o, _)| o == t))
            .collect()
    }
}

/// Read the rules from the data folder (or the copy built into the exe). A
/// missing or broken file means no tool requirements, never a broken game.
pub fn load(data_dir: &std::path::Path) -> ToolRules {
    match crate::embedded_data::read_data_or_embedded(data_dir, ToolRules::FILE).map(|s| ToolRules::from_ron(s.as_bytes())) {
        Some(Ok(r)) => r,
        Some(Err(e)) => {
            log::warn!("{e}; crafting needs no tools this session");
            ToolRules::default()
        }
        None => ToolRules::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(rel: &str) -> Vec<u8> {
        std::fs::read(format!("{}/data/{rel}", env!("CARGO_MANIFEST_DIR"))).expect(rel)
    }

    fn rules() -> ToolRules {
        ToolRules::from_ron(&data(ToolRules::FILE)).expect("tools.ron parses")
    }

    #[test]
    fn rules_resolve_by_station_category_and_override() {
        let r = rules();
        let none: Vec<(String, u32)> = Vec::new();
        assert_eq!(r.tools_for("build_chair", "workbench_0", "construction", &none), vec!["hammer_0", "saw_hand_0"]);
        assert!(r.tools_for("x", "", "construction", &none).is_empty(), "by hand: no station rule applies");
        assert!(!r.tools_for("craft_chest_wood", "workbench_0", "crafting", &none).is_empty(), "a per-recipe list applies");
        // A recipe that makes a tool never needs that tool.
        let makes_hammer = vec![("hammer_0".to_string(), 1)];
        assert!(!r.tools_for("x", "forge_0", "crafting", &makes_hammer).contains(&"hammer_0".to_string()));
    }

    /// The web Crafting page reads data/recipes.json, which
    /// scripts/gen-recipes-json.js resolves from the same tools.ron with its
    /// own reader. Native and web must name the same tools for every recipe;
    /// re-run the script after editing tools.ron or recipes.csv.
    #[test]
    fn the_web_mirror_names_the_same_tools() {
        #[derive(Deserialize)]
        struct Tool {
            id: String,
        }
        #[derive(Deserialize)]
        struct WebRecipe {
            id: String,
            #[serde(default)]
            tools: Vec<Tool>,
        }
        #[derive(Deserialize)]
        struct Web {
            recipes: Vec<WebRecipe>,
        }
        let web: Web = serde_json::from_slice(&data("recipes.json")).expect("recipes.json");
        let native = crate::systems::crafting::RecipeRegistry::from_csv(&data("recipes.csv")).unwrap().with_tools(&rules());
        assert_eq!(web.recipes.len(), native.recipes.len(), "recipes.json is stale: run node scripts/gen-recipes-json.js");
        for w in &web.recipes {
            let n = native.recipes.get(&w.id).unwrap_or_else(|| panic!("{} only on the web", w.id));
            let wt: Vec<&str> = w.tools.iter().map(|t| t.id.as_str()).collect();
            assert_eq!(wt, n.tools, "{}: web and native disagree on tools (re-run gen-recipes-json.js)", w.id);
        }
    }

    /// Every tool any recipe needs is a real item with a durability, and can
    /// be had without already owning it: sold by the vendor, in the starter
    /// kit, or made by a recipe whose own tools can be had (followed to a
    /// fixed point). Without this a rule could lock recipes behind a tool
    /// nobody can get.
    #[test]
    fn every_required_tool_can_be_had() {
        use crate::assets::loader::parse_csv;
        #[derive(Deserialize)]
        struct Item {
            id: String,
            #[serde(default)]
            durability: u32,
        }
        #[derive(Deserialize)]
        struct Row {
            id: String,
            #[serde(default)]
            category: String,
            #[serde(default)]
            outputs: String,
            #[serde(default)]
            station_required: String,
        }
        let r = rules();
        let items: HashMap<String, u32> =
            parse_csv::<Item>(&data("items.csv")).unwrap().into_iter().map(|i| (i.id, i.durability)).collect();
        let recipes: Vec<Row> = parse_csv(&data("recipes.csv")).unwrap();
        let vendor = String::from_utf8(data("trade_goods.ron")).unwrap();
        let kit = String::from_utf8(data("world/player.ron")).unwrap();

        let per_recipe: Vec<(Vec<String>, Vec<String>)> = recipes
            .iter()
            .map(|row| {
                let outs = crate::systems::crafting::Recipe::parse_ingredients(&row.outputs);
                let tools = r.tools_for(&row.id, row.station_required.trim(), &row.category, &outs);
                (tools, outs.into_iter().map(|(o, _)| o).collect())
            })
            .collect();
        let needed: std::collections::BTreeSet<String> = per_recipe.iter().flat_map(|(t, _)| t.clone()).collect();
        assert!(!needed.is_empty());
        for t in &needed {
            let d = items.get(t).unwrap_or_else(|| panic!("tool {t} is not in items.csv"));
            assert!(*d > 0, "tool {t} has no durability in items.csv, so it could never wear out");
        }
        let mut have: std::collections::HashSet<String> = needed
            .iter()
            .filter(|t| vendor.contains(&format!("id: \"{t}\"")) || kit.contains(&format!("(\"{t}\"")))
            .cloned()
            .collect();
        loop {
            let before = have.len();
            for (tools, outs) in &per_recipe {
                if tools.iter().all(|t| have.contains(t)) {
                    for o in outs {
                        if needed.contains(o) {
                            have.insert(o.clone());
                        }
                    }
                }
            }
            if have.len() == before {
                break;
            }
        }
        let missing: Vec<&String> = needed.iter().filter(|t| !have.contains(*t)).collect();
        assert!(missing.is_empty(), "tools no player can get: {missing:?}");
    }
}
