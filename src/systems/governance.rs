//! Governance system — resolves active votes when their deadlines expire.
//!
//! Per-tick: every `ActiveVote` has its `deadline_seconds_remaining` decremented.
//! When it hits zero, the vote is marked `resolved` and a result event is logged.
//! Game code reads `resolved` votes to apply effects (pass/fail laws, elect officials).

use std::path::Path;

use serde::Deserialize;

use crate::ecs::components::ActiveVote;
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Top-level RON schema for `data/governance.ron`.
#[derive(Debug, Deserialize)]
pub struct GovernanceData {
    #[serde(default)] pub government_types: Vec<ron::Value>,
    #[serde(default)] pub laws: Vec<ron::Value>,
    #[serde(default)] pub civic_roles: Vec<ron::Value>,
    #[serde(default)] pub dispute_resolution: Vec<ron::Value>,
}

/// Tracks laws, roles, votes, and settlement governance.
pub struct GovernanceSystem {
    pub data: GovernanceData,
    /// Total votes resolved since startup (for civilization stats).
    pub lifetime_votes_resolved: u64,
}

impl GovernanceSystem {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("governance.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            log::warn!("Failed to read {}: {e}", path.display());
            "(government_types:[],laws:[],civic_roles:[],dispute_resolution:[])".to_string()
        });
        let data: GovernanceData = ron::from_str(&text).unwrap_or_else(|e| {
            log::warn!("Failed to parse governance.ron: {e}");
            GovernanceData { government_types: vec![], laws: vec![], civic_roles: vec![], dispute_resolution: vec![] }
        });
        log::info!("Loaded governance data: {} gov types, {} laws", data.government_types.len(), data.laws.len());
        Self { data, lifetime_votes_resolved: 0 }
    }

    /// Cast a vote on an active proposal. Idempotent per vote — game code is
    /// responsible for tracking who's voted to prevent double-voting (use
    /// the relay's signed-object substrate for that).
    pub fn cast_vote(world: &mut hecs::World, vote: hecs::Entity, yes: bool) {
        if let Ok(mut v) = world.get::<&mut ActiveVote>(vote) {
            if v.resolved { return; }
            if yes { v.yes += 1; } else { v.no += 1; }
        }
    }
}

impl System for GovernanceSystem {
    fn name(&self) -> &str { "GovernanceSystem" }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, _data: &DataStore) {
        if dt <= 0.0 { return; }

        let mut just_resolved: Vec<(hecs::Entity, String, u32, u32, bool)> = Vec::new();
        for (entity, vote) in world.query_mut::<&mut ActiveVote>() {
            if vote.resolved { continue; }
            vote.deadline_seconds_remaining -= dt;
            if vote.deadline_seconds_remaining <= 0.0 {
                vote.resolved = true;
                let passed = vote.yes > vote.no;
                just_resolved.push((entity, vote.proposal.clone(), vote.yes, vote.no, passed));
            }
        }

        for (entity, prop, yes, no, passed) in just_resolved {
            log::info!(
                "Governance vote {:?} resolved: '{}' — {} yes / {} no — {}",
                entity, prop, yes, no, if passed { "PASSED" } else { "REJECTED" }
            );
            self.lifetime_votes_resolved += 1;
        }
    }
}
