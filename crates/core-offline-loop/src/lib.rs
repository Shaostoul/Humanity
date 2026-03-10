use core_firstperson_controller::{
    apply_look, apply_move, ControllerInput, ControllerState, MoveDir,
};
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
pub struct Inventory {
    pub food_rations: u32,
    pub wood: u32,
    pub fiber: u32,
    pub scrap: u32,
    pub filter_kits: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestones {
    pub crafted_filter: bool,
    pub purified_water: bool,
    pub planted_cycle: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSnapshot {
    pub tick: u64,
    pub player: LifeformState,
    pub player_pos: GridPos,
    pub controller: ControllerState,
    pub water: WaterNode,
    pub soil: SoilCell,
    pub crop: CropInstance,
    pub inventory: Inventory,
    pub milestones: Milestones,
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
            controller: ControllerState::baseline(),
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
            inventory: Inventory {
                food_rations: 2,
                wood: 0,
                fiber: 0,
                scrap: 0,
                filter_kits: 0,
            },
            milestones: Milestones {
                crafted_filter: false,
                purified_water: false,
                planted_cycle: false,
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

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Help,
    Status,
    Look,
    Move(char),
    LookDir(f32, f32),
    Rest,
    Drink,
    TreatWater,
    FarmTick,
    Gather(String),
    CraftFilter,
    Eat,
    Inventory,
    Objective,
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
        "look_dir" => {
            let yaw = parts
                .next()
                .ok_or(LoopError::InvalidArgs)?
                .parse::<f32>()
                .map_err(|_| LoopError::InvalidArgs)?;
            let pitch = parts
                .next()
                .ok_or(LoopError::InvalidArgs)?
                .parse::<f32>()
                .map_err(|_| LoopError::InvalidArgs)?;
            Ok(Command::LookDir(yaw, pitch))
        }
        "rest" => Ok(Command::Rest),
        "drink" => Ok(Command::Drink),
        "treat_water" => Ok(Command::TreatWater),
        "farm_tick" => Ok(Command::FarmTick),
        "gather" => {
            let item = parts.next().ok_or(LoopError::InvalidArgs)?;
            Ok(Command::Gather(item.to_string()))
        }
        "craft_filter" => Ok(Command::CraftFilter),
        "eat" => Ok(Command::Eat),
        "inventory" => Ok(Command::Inventory),
        "objective" => Ok(Command::Objective),
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
        Command::Help => Ok("commands: help status look move <n|s|e|w> look_dir <yaw_delta> <pitch_delta> rest drink eat gather <wood|fiber|scrap|food> craft_filter treat_water farm_tick inventory objective practice <skill> lesson set_difficulty <baby|easy|medium|hard|realistic> transition <offline|host|join|dedicated> save <path> load <path> quit".to_string()),
        Command::Status => Ok(format!(
            "tick={} pos=({}, {}) fp=({:.1},{:.1},{:.1}) yaw={:.1} pitch={:.1} energy={:.1} hydration={:.1} stress={:.1} water_liters={:.1} water_contam={:.1} crop_stage={:?} inv(food={},wood={},fiber={},scrap={},filters={}) milestones({}/{}/{}) capability={:.1} mode={:?}/{:?}/{:?}",
            world.tick,
            world.player_pos.x,
            world.player_pos.y,
            world.controller.position.x,
            world.controller.position.y,
            world.controller.position.z,
            world.controller.yaw_deg,
            world.controller.pitch_deg,
            world.player.physiology.energy,
            world.player.physiology.hydration,
            world.player.affect.stress,
            world.water.liters,
            world.water.quality.contamination_index,
            world.crop.stage,
            world.inventory.food_rations,
            world.inventory.wood,
            world.inventory.fiber,
            world.inventory.scrap,
            world.inventory.filter_kits,
            world.milestones.crafted_filter,
            world.milestones.purified_water,
            world.milestones.planted_cycle,
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
            let move_dir = match dir {
                'n' => MoveDir::Forward,
                's' => MoveDir::Backward,
                'e' => MoveDir::Right,
                'w' => MoveDir::Left,
                _ => return Err(LoopError::InvalidArgs),
            };

            apply_move(
                &mut world.controller,
                ControllerInput {
                    dir: move_dir,
                    dt_seconds: 1.0,
                    sprint: false,
                },
            );

            world.player_pos.x = world.controller.position.x.round() as i32;
            world.player_pos.y = world.controller.position.z.round() as i32;

            tick_player(world, 1, 1.05, 0.0, 0.0)?;
            world.tick += 1;
            Ok("moved".to_string())
        }
        Command::LookDir(yaw, pitch) => {
            apply_look(&mut world.controller, yaw, pitch);
            world.tick += 1;
            Ok(format!(
                "look updated yaw={:.1} pitch={:.1}",
                world.controller.yaw_deg, world.controller.pitch_deg
            ))
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
            let efficacy = if world.inventory.filter_kits > 0 {
                world.inventory.filter_kits -= 1;
                0.75
            } else {
                0.45
            };

            treat_water(&mut world.water, TreatmentStep { efficacy })
                .map_err(|e| LoopError::Simulation(e.to_string()))?;
            if world.water.quality.potability == Potability::Potable {
                world.milestones.purified_water = true;
            }
            world.tick += 1;
            Ok(format!(
                "treated water (eff={:.2}); contamination now {:.1}",
                efficacy,
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

            world.milestones.planted_cycle = true;
            world.tick += 24;
            let harvest = harvest_report(&world.crop)
                .map(|h| format!(" harvest(yield={:.1}, quality={:.1})", h.yield_score, h.quality_score))
                .unwrap_or_default();
            Ok(format!("farm tick complete; stage={:?}{}", world.crop.stage, harvest))
        }
        Command::Gather(item) => {
            let (skill, msg) = match item.as_str() {
                "wood" => {
                    world.inventory.wood += 2;
                    ("carpentry", "gathered wood +2")
                }
                "fiber" => {
                    world.inventory.fiber += 2;
                    ("farming", "gathered fiber +2")
                }
                "scrap" => {
                    world.inventory.scrap += 1;
                    ("plumbing", "gathered scrap +1")
                }
                "food" => {
                    world.inventory.food_rations += 1;
                    ("farming", "foraged food ration +1")
                }
                _ => return Err(LoopError::InvalidArgs),
            };
            let _ = award_xp(
                &mut world.skills,
                &world.progression,
                skill,
                20,
                world.session.fidelity,
            )
            .map_err(|e| LoopError::Simulation(e.to_string()))?;
            world.tick += 1;
            Ok(msg.to_string())
        }
        Command::CraftFilter => {
            if world.inventory.wood < 1 || world.inventory.fiber < 1 || world.inventory.scrap < 1 {
                return Ok("need at least wood=1 fiber=1 scrap=1 to craft filter".to_string());
            }
            world.inventory.wood -= 1;
            world.inventory.fiber -= 1;
            world.inventory.scrap -= 1;
            world.inventory.filter_kits += 1;
            world.milestones.crafted_filter = true;
            let _ = award_xp(
                &mut world.skills,
                &world.progression,
                "water",
                35,
                world.session.fidelity,
            )
            .map_err(|e| LoopError::Simulation(e.to_string()))?;
            world.tick += 2;
            Ok("crafted filter kit +1".to_string())
        }
        Command::Eat => {
            if world.inventory.food_rations == 0 {
                return Ok("no food rations available".to_string());
            }
            world.inventory.food_rations -= 1;
            tick_player(world, 1, 0.9, 1.2, 0.0)?;
            world.tick += 1;
            Ok("ate one food ration".to_string())
        }
        Command::Inventory => Ok(format!(
            "inventory => food={} wood={} fiber={} scrap={} filters={}",
            world.inventory.food_rations,
            world.inventory.wood,
            world.inventory.fiber,
            world.inventory.scrap,
            world.inventory.filter_kits
        )),
        Command::Objective => {
            let score = [
                world.milestones.crafted_filter,
                world.milestones.purified_water,
                world.milestones.planted_cycle,
            ]
            .iter()
            .filter(|x| **x)
            .count();
            Ok(format!(
                "offline milestone progress: {}/3 (craft_filter={}, purified_water={}, farm_cycle={})",
                score,
                world.milestones.crafted_filter,
                world.milestones.purified_water,
                world.milestones.planted_cycle,
            ))
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
        assert!(world.player_pos.y > 0);
        assert!(world.controller.position.z > 0.0);
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

    #[test]
    fn look_dir_changes_orientation() {
        let mut world = WorldSnapshot::new_default();
        let _ = apply_command(&mut world, Command::LookDir(15.0, -10.0)).unwrap();
        assert_eq!(world.controller.yaw_deg, 15.0);
        assert_eq!(world.controller.pitch_deg, -10.0);
    }

    #[test]
    fn non_combat_offline_loop_reaches_milestone() {
        let mut world = WorldSnapshot::new_default();

        let _ = apply_command(&mut world, Command::Gather("wood".into())).unwrap();
        let _ = apply_command(&mut world, Command::Gather("fiber".into())).unwrap();
        let _ = apply_command(&mut world, Command::Gather("scrap".into())).unwrap();
        let _ = apply_command(&mut world, Command::CraftFilter).unwrap();
        let _ = apply_command(&mut world, Command::TreatWater).unwrap();
        let _ = apply_command(&mut world, Command::FarmTick).unwrap();

        assert!(world.milestones.crafted_filter);
        assert!(world.milestones.purified_water);
        assert!(world.milestones.planted_cycle);
    }
}
