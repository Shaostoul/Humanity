//! humanity-animation: one body and one animation system for the player, the other players and the
//! crew (docs/design/character-animation.md; its card is docs/crates/humanity-animation.md).
//!
//! Rung 1 of the plan, built 2026-11-08: the humanoid skeleton standard (`humanoid`, `role_map`,
//! `skeleton`), poses and the GPU's pose palette (`pose`), clips sampled from imported keys and baked
//! onto a body by role (`clip`, `retarget`), and the locomotion state machine with its idle, walk,
//! run and turn blend spaces (`graph`), fed by speed and turn rate read from positions (`motion`),
//! with the near and middle drawing tiers (`tiers`) and the data files that tie it together
//! (`library`).
//!
//! Rung 2a, 2026-11-09: feet on the world (`feet`): contacts labelled on every clip at load, the
//! ground under each foot asked of the engine through a trait, two-bone analytic leg IK on each
//! person's reshaped skeleton, the pelvis lowered by the larger drop, the foot turned to the slope
//! about a toe pivot, foot locking and stair footsteps; and the graph's crouch and jump inputs.
//!
//! WHAT IT IS NOT: no GPU code, no egui, no glTF reader, no network. The engine reads the files
//! (`assets::skin`) into `humanity_core::skin_data` and draws the palettes this crate writes
//! (`renderer::skinning`); the relay never compiles this crate, because animation is cosmetic and
//! never crosses the network (plan section 8.5: "Never poses").
//!
//! THE BODY IS DATA. Nothing here names a body, a rig, a clip or a state: the body, the role map
//! that turns its bone names into humanoid roles, the clips and the graph are files under
//! `data/characters/` (CLAUDE.md, Infinite-of-X), so a MakeHuman, a SOMA or a Quaternius rig is a
//! new role map and a new body file, not code (the operator, 2026-11-08: build it body-agnostic so
//! "the body is an asset choice, not a code choice").
//!
//! NOTHING KEPT ACROSS RUNS. Clips are retargeted in memory at load from the source files and never
//! written to disk (plan section 8.7); the engine rebuilds the library when the files change and,
//! with the reuse switch off, every few seconds (`engine::animation`).

pub mod clip;
pub mod feet;
pub mod graph;
pub mod humanoid;
pub mod joint_map;
pub mod library;
pub mod look;
pub mod motion;
pub mod pose;
pub mod retarget;
pub mod role_map;
pub mod skeleton;
pub mod tiers;

pub use clip::{Clip, ClipSet, SourceClip};
pub use feet::{FeetDef, FeetReport, FootFrame, FootMode, FootPlacer, FootRig, Ground, GroundHit, StepInfo};
pub use graph::{Animator, Graph, GraphDef, LocomotionInput};
pub use humanoid::{HumanoidDef, Tier};
pub use library::{BodyDef, CharacterLibrary, CharactersDef, ClipDef, DressChoice, LoadedBody, Tint};
pub use look::{GarmentKind, LookLibrary, Part, PersonInputs, PersonMesh};
pub use motion::MotionTracker;
pub use pose::{Pose, Trs, MAX_ACTIVE_MORPHS, MORPH_SLOTS, PALETTE_ROWS_PER_JOINT};
pub use role_map::RoleMap;
pub use skeleton::Skeleton;
pub use tiers::{DrawTier, Tiers};

/// Reads a RON data file with the house's options (comments allowed, `Some(...)` written out).
pub(crate) fn parse_ron<T: serde::de::DeserializeOwned>(text: &str, what: &str) -> Result<T, String> {
    ron::from_str(text).map_err(|e| format!("{what}: {e}"))
}

#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod tests;
