//! Genetics system — diploid trait inheritance with mutation chance.
//!
//! Event-driven (no per-tick simulation). Game code calls
//! `GeneticsSystem::breed(parent_a, parent_b, rng)` to produce a child Genome.
//! Each trait pair contributes one allele (50/50 random) to the child;
//! a small mutation chance flips each child allele to a random one drawn
//! from the trait's defined allele pool in `data/genetics.ron`.

use std::collections::HashMap;
use std::path::Path;

use rand::Rng;
use serde::Deserialize;

use crate::ecs::components::Genome;
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Default mutation chance per allele if not specified per-trait. Tunable.
const DEFAULT_MUTATION_RATE: f32 = 0.001;

/// Top-level RON schema for `data/genetics.ron`.
/// (Real schema uses `plant_traits` / `animal_traits` rather than a single
/// `traits` field — accept several variants and merge.)
#[derive(Debug, Deserialize, Default)]
pub struct GeneticsData {
    #[serde(default)] pub plant_traits: Vec<ron::Value>,
    #[serde(default)] pub animal_traits: Vec<ron::Value>,
    #[serde(default)] pub mutations: Vec<ron::Value>,
    #[serde(default)] pub diseases: Vec<ron::Value>,
    #[serde(default)] pub breeding_methods: Vec<ron::Value>,
}

/// Manages breeding, trait inheritance, mutations, and genetic diseases.
pub struct GeneticsSystem {
    pub data: GeneticsData,
    /// Mapping `trait_id -> Vec<allele_name>` extracted from the data file
    /// for use in mutation rolls. Empty traits fall back to copying parents.
    allele_pools: HashMap<String, Vec<String>>,
}

impl GeneticsSystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("genetics.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(plant_traits:[],animal_traits:[],mutations:[],diseases:[],breeding_methods:[])".to_string()
        });
        let data: GeneticsData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse genetics.ron: {e}");
            GeneticsData::default()
        });
        log::info!(
            "Loaded genetics data: {} plant traits, {} animal traits",
            data.plant_traits.len(), data.animal_traits.len()
        );

        let mut allele_pools: HashMap<String, Vec<String>> = HashMap::new();
        for v in data.plant_traits.iter().chain(data.animal_traits.iter()) {
            if let Some((id, alleles)) = Self::parse_trait_alleles(v) {
                allele_pools.insert(id, alleles);
            }
        }

        Self { data, allele_pools }
    }

    /// Try to extract `(trait_id, [allele_name...])` from a trait definition.
    /// Accepts both `alleles: ["a", "b"]` and `variants: [(id: "a", ...)]` shapes.
    fn parse_trait_alleles(v: &ron::Value) -> Option<(String, Vec<String>)> {
        let map = v.clone().into_rust::<HashMap<String, ron::Value>>().ok()?;
        let id = map.get("id")?.clone().into_rust::<String>().ok()?;

        // Try the simple `alleles: [String, ...]` shape first.
        if let Some(allele_list) = map.get("alleles").and_then(|a| match a {
            ron::Value::Seq(s) => Some(s.clone()),
            _ => None,
        }) {
            let names: Vec<String> = allele_list.into_iter()
                .filter_map(|v| v.into_rust::<String>().ok())
                .collect();
            if !names.is_empty() {
                return Some((id, names));
            }
        }

        // Fall back to `variants: [(id: "x"), ...]`.
        if let Some(variants_seq) = map.get("variants").and_then(|v| match v {
            ron::Value::Seq(s) => Some(s.clone()),
            _ => None,
        }) {
            let names: Vec<String> = variants_seq.into_iter()
                .filter_map(|v| {
                    v.into_rust::<HashMap<String, ron::Value>>().ok()
                        .and_then(|m| m.get("id").and_then(|i| i.clone().into_rust::<String>().ok()))
                })
                .collect();
            if !names.is_empty() {
                return Some((id, names));
            }
        }

        None
    }

    /// Combine two parent genomes into a child. Each trait pair in the union
    /// of parent traits contributes one allele picked at random; a small
    /// `DEFAULT_MUTATION_RATE` chance per allele swaps it for a random one
    /// from the trait's allele pool (if known).
    pub fn breed<R: Rng + ?Sized>(
        &self,
        parent_a: &Genome,
        parent_b: &Genome,
        rng: &mut R,
    ) -> Genome {
        let mut alleles = HashMap::new();
        let trait_ids: std::collections::HashSet<&String> = parent_a.alleles.keys()
            .chain(parent_b.alleles.keys())
            .collect();

        for trait_id in trait_ids {
            let a_pair = parent_a.alleles.get(trait_id);
            let b_pair = parent_b.alleles.get(trait_id);

            // Pick one allele from each parent (random within the parent's pair).
            let from_a = a_pair
                .map(|(a1, a2)| if rng.gen_bool(0.5) { a1.clone() } else { a2.clone() })
                .or_else(|| b_pair.map(|(b1, _)| b1.clone()))
                .unwrap_or_default();
            let from_b = b_pair
                .map(|(b1, b2)| if rng.gen_bool(0.5) { b1.clone() } else { b2.clone() })
                .or_else(|| a_pair.map(|(a1, _)| a1.clone()))
                .unwrap_or_default();

            // Apply mutations.
            let pool = self.allele_pools.get(trait_id);
            let mutated_a = self.maybe_mutate(from_a, pool, rng);
            let mutated_b = self.maybe_mutate(from_b, pool, rng);
            alleles.insert(trait_id.clone(), (mutated_a, mutated_b));
        }

        Genome { alleles }
    }

    fn maybe_mutate<R: Rng + ?Sized>(
        &self,
        allele: String,
        pool: Option<&Vec<String>>,
        rng: &mut R,
    ) -> String {
        if rng.gen::<f32>() >= DEFAULT_MUTATION_RATE { return allele; }
        match pool {
            Some(p) if !p.is_empty() => {
                let idx = rng.gen_range(0..p.len());
                p[idx].clone()
            }
            _ => allele,
        }
    }
}

impl System for GeneticsSystem {
    fn name(&self) -> &str { "GeneticsSystem" }

    fn tick(&mut self, _world: &mut hecs::World, _dt: f32, _data: &DataStore) {
        // Genetics is event-driven — `breed()` is called by the breeding
        // interaction handler, not on every frame. Nothing to do per tick.
    }
}
