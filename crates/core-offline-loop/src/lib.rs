use core_lifeform_model::{LifeformState, LifeformTick, TickInput};
use core_session_orchestrator::{
    can_transition, validate_config, FidelityPreset, NetworkScope, ProgressionPolicy, SessionConfig,
    SessionMode, TransitionReason,
};
use core_skill_progression::{
    award_xp, capability_index, ProgressionProfile, SkillBook,
};
use core_teaching_graph::{
    add_node, add_prereq, recommend_next, CompetencyGraph, CompetencyNode,
};
use module_crop_systems::{
    harvest_report, tick_growth, CropInstance, EnvironmentInput, GrowthStage,
};
use module_soil_ecology::{simulate_season, SeasonInput, SoilCell, SoilTexture};
use module_water_systems::{
    treat_water, Potability, TreatmentStep, WaterNode, WaterQuality, WaterSourceKind,
};
use serde::{Deserialize, Serialize};
use std::fs;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridPos {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSnapshot {
    pub tick: u64,
    pub player: LifeformState,
    pub player_pos: GridPos,
    pub water: WaterNode,
    pub soil: SoilCell,
    pub crop: CropInstance,
    pub skills: SkillBook,
    pub progression: ProgressionProfile,
    pub teaching: CompetencyGraph,
    pub session: SessionConfig,
}

impl WorldSnapshot {
    pub fn new_default() -> Self {
        Self {
            tick: 0,
            player: LifeformState::baseline_human(),
            player_pos: GridPos { x: 0, y: 0 },
            water: WaterNode {
                source_kind: WaterSourceKind::Stored,
                liters: 40.0,
                quality: WaterQuality {
                    contamination_index: 25.0,
                    potability: Potability::NonPotable,
                },
            },
            soil: SoilCell {
                texture: SoilTexture::Loam,
                moisture: 55.0,
                nutrient_index: 50.0,
                compaction: 35.0,
                biology: 48.0,
            },
            crop: CropInstance {
                stage: GrowthStage::Seed,
                vitality: 60.0,
                stress: 15.0,
                growth_progress: 0.0,
            },
            skills: SkillBook::default(),
            progression: ProgressionProfile::default(),
            teaching: default_teaching_graph(),
            session: SessionConfig {
                mode: SessionMode::Offline,
                policy: ProgressionPolicy::OpenProfile,
                network: NetworkScope::Offline,
                fidelity: FidelityPreset::Medium,
            },
        }
    }
}

fn default_teaching_graph() -> CompetencyGraph {
    let mut graph = CompetencyGraph::default();
    let _ = add_node(
        &mut graph,
        CompetencyNode {
            id: "water".to_string(),
            label: "Water Safety".to_string(),
            target_level: 2,
        },
    );
    let _ = add_node(
        &mut graph,
        CompetencyNode {
            id: "farming".to_string(),
            label: "Crop Systems".to_string(),
            target_level: 2,
        },
    );
    let _ = add_node(
        &mut graph,
        CompetencyNode {
            id: "health".to_string(),
            label: "First Aid".to_string(),
            target_level: 2,
        },
    );
    let _ = add_node(
        &mut graph,
        CompetencyNode {
            id: "plumbing".to_string(),
            label: "Plumbing Basics".to_string(),
            target_level: 2,
        },
    );
    let _ = add_prereq(&mut graph, "plumbing", "water");
    graph
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Help,
    Status,
    Look,
    Move(char),
    Rest,
    Drink,
    TreatWater,
    FarmTick,
    Practice(String),
    Lesson,
    SetDifficulty(FidelityPreset),
    Transition(SessionMode),
    Save(String),
    Load(String),
    Quit,
}

#[derive(Debug, Error)]
pub enum LoopError {
    #[error("unknown command")]
    UnknownCommand,
    #[error("invalid args")]
    InvalidArgs,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("session transition failed: {0}")]
    Transition(String),
    #[error("simulation failed: {0}")]
    Simulation(String),
}

pub fn parse_command(input: &str) -> Result<Command, LoopError> {
    let mut parts = input.split_whitespace();
    let cmd = parts.next().ok_or(LoopError::UnknownCommand)?;
    match cmd.to_lowercase().as_str() {
        "help" => Ok(Command::Help),
        "status" => Ok(Command::Status),
        "look" => Ok(Command::Look),
        "move" => {
            let dir = parts.next().ok_or(LoopError::InvalidArgs)?;
            let c = dir.chars().next().ok_or(LoopError::InvalidArgs)?;
            if !matches!(c, 'n' | 's' | 'e' | 'w') {
                return Err(LoopError::InvalidArgs);
            }
            Ok(Command::Move(c))
        }
        "rest" => Ok(Command::Rest),
        "drink" => Ok(Command::Drink),
        "treat_water" => Ok(Command::TreatWater),
        "farm_tick" => Ok(Command::FarmTick),
        "practice" => {
            let skill = parts.next().ok_or(LoopError::InvalidArgs)?;
            Ok(Command::Practice(skill.to_string()))
        }
        "lesson" => Ok(Command::Lesson),
        "set_difficulty" => {
            let d = parts.next().ok_or(LoopError::InvalidArgs)?;
            let preset = match d {
                "baby" | "creative" => FidelityPreset::BabyCreative,
                "easy" => FidelityPreset::Easy,
                "medium" => FidelityPreset::Medium,
                "hard" => FidelityPreset::Hard,
                "realistic" => FidelityPreset::Realistic,
                _ => return Err(LoopError::InvalidArgs),
            };
            Ok(Command::SetDifficulty(preset))
        }
        "transition" => {
            let m = parts.next().ok_or(LoopError::InvalidArgs)?;
            let mode = match m {
                "offline" => SessionMode::Offline,
                "host" => SessionMode::HostP2p,
                "join" => SessionMode::JoinP2p,
                "dedicated" => SessionMode::Dedicated,
                _ => return Err(LoopError::InvalidArgs),
            };
            Ok(Command::Transition(mode))
        }
        "save" => Ok(Command::Save(parts.next().ok_or(LoopError::InvalidArgs)?.to_string())),
        "load" => Ok(Command::Load(parts.next().ok_or(LoopError::InvalidArgs)?.to_string())),
        "quit" => Ok(Command::Quit),
        _ => Err(LoopError::UnknownCommand),
    }
}

pub fn apply_command(world: &mut WorldSnapshot, cmd: Command) -> Result<String, LoopError> {
    match cmd {
        Command::Help => Ok("commands: help status look move <n|s|e|w> rest drink treat_water farm_tick practice <skill> lesson set_difficulty <baby|easy|medium|hard|realistic> transition <offline|host|join|dedicated> save <path> load <path> quit".to_string()),
        Command::Status => Ok(format!(
            "tick={} pos=({}, {}) energy={:.1} hydration={:.1} stress={:.1} water_liters={:.1} water_contam={:.1} crop_stage={:?} capability={:.1} mode={:?}/{:?}/{:?}",
            world.tick,
            world.player_pos.x,
            world.player_pos.y,
            world.player.physiology.energy,
            world.player.physiology.hydration,
            world.player.affect.stress,
            world.water.liters,
            world.water.quality.contamination_index,
            world.crop.stage,
            capability_index(&world.skills),
            world.session.mode,
            world.session.network,
            world.session.fidelity,
        )),
        Command::Look => Ok(format!(
            "You are at grid ({}, {}). Soil nutrients {:.1}, moisture {:.1}.",
            world.player_pos.x, world.player_pos.y, world.soil.nutrient_index, world.soil.moisture
        )),
        Command::Move(dir) => {
            match dir {
                'n' => world.player_pos.y += 1,
                's' => world.player_pos.y -= 1,
                'e' => world.player_pos.x += 1,
                'w' => world.player_pos.x -= 1,
                _ => return Err(LoopError::InvalidArgs),
            }
            tick_player(world, 1, 1.05, 0.0, 0.0)?;
            world.tick += 1;
            Ok("moved".to_string())
        }
        Command::Rest => {
            tick_player(world, 8, 0.6, 0.5, 0.5)?;
            world.tick += 8;
            Ok("rested for 8h".to_string())
        }
        Command::Drink => {
            if world.water.liters <= 0.0 {
                return Ok("no water left".to_string());
            }
            world.water.liters = (world.water.liters - 1.0).max(0.0);
            let water_intake = if world.water.quality.potability == Potability::Potable {
                1.0
            } else {
                0.5
            };
            tick_player(world, 1, 1.0, 0.0, water_intake)?;
            world.tick += 1;
            Ok("drank water".to_string())
        }
        Command::TreatWater => {
            treat_water(&mut world.water, TreatmentStep { efficacy: 0.45 })
                .map_err(|e| LoopError::Simulation(e.to_string()))?;
            world.tick += 1;
            Ok(format!(
                "treated water; contamination now {:.1}",
                world.water.quality.contamination_index
            ))
        }
        Command::FarmTick => {
            simulate_season(
                &mut world.soil,
                SeasonInput {
                    rainfall: 58.0,
                    heat: 45.0,
                    tillage_intensity: 20.0,
                    amendment_boost: 15.0,
                },
            )
            .map_err(|e| LoopError::Simulation(e.to_string()))?;

            tick_growth(
                &mut world.crop,
                EnvironmentInput {
                    moisture: world.soil.moisture,
                    nutrient_index: world.soil.nutrient_index,
                    temperature_suitability: 72.0,
                    pollination_support: 66.0,
                },
            )
            .map_err(|e| LoopError::Simulation(e.to_string()))?;

            let _ = award_xp(
                &mut world.skills,
                &world.progression,
                "farming",
                55,
                world.session.fidelity,
            )
            .map_err(|e| LoopError::Simulation(e.to_string()))?;

            world.tick += 24;
            let harvest = harvest_report(&world.crop)
                .map(|h| format!(" harvest(yield={:.1}, quality={:.1})", h.yield_score, h.quality_score))
                .unwrap_or_default();
            Ok(format!("farm tick complete; stage={:?}{}", world.crop.stage, harvest))
        }
        Command::Practice(skill) => {
            let rec = award_xp(
                &mut world.skills,
                &world.progression,
                &skill,
                40,
                world.session.fidelity,
            )
            .map_err(|e| LoopError::Simulation(e.to_string()))?;
            world.tick += 2;
            Ok(format!("practiced {skill}: xp={} level={}", rec.xp, rec.level))
        }
        Command::Lesson => {
            let recs = recommend_next(&world.teaching, &world.skills, 3)
                .map_err(|e| LoopError::Simulation(e.to_string()))?;
            if recs.is_empty() {
                Ok("no recommendations; current competencies meet tracked targets".to_string())
            } else {
                Ok(format!("next lessons: {}", recs.join(", ")))
            }
        }
        Command::SetDifficulty(preset) => {
            world.session.fidelity = preset;
            Ok(format!("difficulty set to {:?}", preset))
        }
        Command::Transition(to_mode) => {
            can_transition(world.session.mode, to_mode, TransitionReason::UserRequested)
                .map_err(|e| LoopError::Transition(e.to_string()))?;

            world.session.mode = to_mode;
            world.session.network = match to_mode {
                SessionMode::Offline => NetworkScope::Offline,
                SessionMode::HostP2p => NetworkScope::Lan,
                SessionMode::JoinP2p => NetworkScope::Lan,
                SessionMode::Dedicated => NetworkScope::DirectInternet,
            };
            if to_mode != SessionMode::Offline {
                world.session.policy = ProgressionPolicy::ClosedProfile;
            }

            validate_config(&world.session).map_err(|e| LoopError::Transition(e.to_string()))?;
            Ok(format!("transitioned to {:?}", to_mode))
        }
        Command::Save(path) => {
            let data = serde_json::to_string_pretty(world)?;
            fs::write(&path, data)?;
            Ok(format!("saved {}", path))
        }
        Command::Load(path) => {
            let data = fs::read_to_string(&path)?;
            let loaded: WorldSnapshot = serde_json::from_str(&data)?;
            *world = loaded;
            Ok(format!("loaded {}", path))
        }
        Command::Quit => Ok("quit".to_string()),
    }
}

fn tick_player(
    world: &mut WorldSnapshot,
    elapsed_hours: u64,
    environment_stress_multiplier: f32,
    food_intake_units: f32,
    water_intake_units: f32,
) -> Result<(), LoopError> {
    world
        .player
        .tick(TickInput {
            elapsed_hours,
            environment_stress_multiplier,
            food_intake_units,
            water_intake_units,
        })
        .map_err(|e| LoopError::Simulation(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_command_advances_tick_and_position() {
        let mut world = WorldSnapshot::new_default();
        let _ = apply_command(&mut world, Command::Move('n')).expect("move should work");
        assert_eq!(world.tick, 1);
        assert_eq!(world.player_pos, GridPos { x: 0, y: 1 });
    }

    #[test]
    fn transition_offline_to_host_is_allowed() {
        let mut world = WorldSnapshot::new_default();
        let _ = apply_command(&mut world, Command::Transition(SessionMode::HostP2p))
            .expect("transition should work");
        assert_eq!(world.session.mode, SessionMode::HostP2p);
    }

    #[test]
    fn parse_set_difficulty_realistic() {
        let cmd = parse_command("set_difficulty realistic").expect("parse should work");
        assert_eq!(cmd, Command::SetDifficulty(FidelityPreset::Realistic));
    }

    #[test]
    fn practice_increases_skill_xp() {
        let mut world = WorldSnapshot::new_default();
        let _ = apply_command(&mut world, Command::Practice("water".to_string())).unwrap();
        let rec = world.skills.skills.get("water").unwrap();
        assert!(rec.xp > 0);
    }

    #[test]
    fn lesson_returns_recommendations() {
        let mut world = WorldSnapshot::new_default();
        let out = apply_command(&mut world, Command::Lesson).unwrap();
        assert!(out.contains("next lessons") || out.contains("no recommendations"));
    }
}
