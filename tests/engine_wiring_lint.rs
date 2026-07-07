//! Engine-wiring lint — the `theme_token_lint` analog for game systems + runtime data.
//!
//! The 2026-05-29 game-code audit found the engine was extensively WRITTEN but
//! barely WIRED: ~40 `impl System for X` types existed, but only a handful were
//! ever registered into the runtime `SystemRunner`, and the item/recipe/plant
//! registries were loaded then DISCARDED so the DataStore stayed empty. Systems
//! and data that never run are invisible — nothing fails, they just silently do
//! nothing.
//!
//! This lint makes that gap IMPOSSIBLE to reintroduce silently:
//!   * Every `impl System for X` under `src/systems/` must be EITHER registered in
//!     `src/lib.rs` (the native runtime runner) OR listed in `DEFERRED_SYSTEMS`
//!     with a written reason. A new system that's neither fails the build.
//!   * The core runtime registries + game_time must stay wired into `src/lib.rs`.
//!
//! As deferred systems get genuinely wired (data loaded, output exported, live
//! behaviour verified), DELETE them from `DEFERRED_SYSTEMS`. The list should
//! shrink, never grow without a real reason — same discipline as
//! `theme_token_lint`'s `LEGACY_OFFENDERS`. The whole point is to drive it to
//! empty, one honest increment at a time.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Systems that have an `impl System` but are intentionally NOT yet registered in
/// the runtime, each with the reason. Shrinks as systems are genuinely wired.
///
/// (Registered as of v0.342.0: TimeSystem, PlayerControllerSystem,
/// InteractionSystem, FarmingSystem, InventorySystem, ContainerCompatibilitySystem,
/// CraftingSystem, FoodSystem, DroneSystem, WeatherSystem, SkillSystem, QuestSystem.)
const DEFERRED_SYSTEMS: &[(&str, &str)] = &[
    // Simulation systems — implemented, tick safely, read game_time (now exported),
    // but operate on world entities/outputs not yet spawned/consumed. Register each
    // alongside the content increment that spawns its entities + verifies it does
    // not misbehave against the live player/NPC entities.
    ("EcologySystem", "operates on disease/population entities not yet spawned; verify no player/NPC component collisions before registering"),
    ("HydrologySystem", "operates on WaterBody entities not yet spawned + needs Weather exported"),
    // AtmosphereSystem is now REGISTERED (v0.617): ticks the home's sealed EnclosedSpace (spawned with
    // the home) + publishes the live AirStatus. Left this allowlist.
    ("DisasterSystem", "spawn is intentionally manual; operates on Disaster entities not yet spawned"),
    // Gameplay systems — implemented but need their content/UI/data layer wired.
    ("CombatSystem", "needs combat encounters + live-behaviour verification vs player/NPC"),
    ("AISystem", "behaviour trees/autonomy; needs NPC spawn + dialogue wiring (note: off-screen autonomy is currently relay-side)"),
    // ConstructionSystem is REGISTERED in src/lib.rs as of 2026-07-01 (afternoon audit:
    // fully-coded blueprint->build->Structure loop, just never turned on) plus
    // BlueprintRegistry now actually loads data/blueprints/basic.ron -- nothing calls
    // queue_build() yet (no GUI/economy-automation caller wired), so it's a live,
    // correct no-op today, not deferred.
    ("PlacementSystem", "paired with ConstructionSystem; same build-mode gating"),
    // EconomySystem entry removed 2026-07-07 (v0.747, closure ladder rung 3):
    // REGISTERED in src/lib.rs — passive income pays real credits into the
    // player's Wallet component; vendor trade runs via the frame bridge.
    // VehicleSystem entry removed 2026-07-02: REGISTERED in src/lib.rs (economy
    // Phase 2 Stage 1) — its deploy arm drains deploy_kit_request live; the
    // enter/exit/mech arms stay dormant until Stage 3 publishes their commands.
    // ElectricalSystem + SolarSystem are REGISTERED in src/lib.rs (the live home power
    // sim: solar scales by time of day, electrical sums supply/demand + battery SoC),
    // so they are intentionally NOT deferred — the lint detects their path-qualified
    // register() calls directly.
    // PsychologySystem entry removed 2026-07-02: the whole psychology.rs scaffold
    // was deleted in the dead-code sweep (operator-approved; zero callers).
    // Self-loading scaffolds (new(data_dir)) — real data, thin behaviour; register
    // each when its entity layer + consumers exist.
    ("AgingSystem", "scaffold; needs living entities with age"),
    ("AstronomySystem", "scaffold; celestial data drives holograms, not an ECS tick yet"),
    ("DockingSystem", "scaffold; needs ship/dock entities"),
    ("CreativeArtsSystem", "scaffold; needs art/creation entities"),
    ("FireSystem", "scaffold; needs flammable entities + ignition"),
    ("GeneticsSystem", "scaffold; needs breeding entities"),
    ("GovernanceSystem", "scaffold; governance is currently relay-side"),
    ("GeologySystem", "scaffold; needs terrain/mining integration"),
    ("WasteSystem", "scaffold; needs waste/sanitation entities"),
    ("OfflineSystem", "scaffold; off-screen autonomy is relay-side"),
    ("OceanographySystem", "scaffold; needs ocean bodies"),
    // ManufacturingSystem is REGISTERED in src/lib.rs as of 2026-07-01 (afternoon audit:
    // fully-coded ProductionFacility->timed recipe->output_count loop, just never turned
    // on). Safe no-op until something spawns a ProductionFacility entity.
    ("MedicalSystem", "scaffold; needs health/injury entities"),
    ("TransportationSystem", "scaffold; needs a transit network"),
    ("HvacSystem", "scaffold; needs enclosed-space climate entities"),
    // PlumbingSystem is now REGISTERED (v0.608): the live home water sim, coupled to power. It ticks
    // against WaterTank/WaterProducer/WaterConsumer/PlumbingCircuit entities, so it left this allowlist.
];

/// Systems wired through the multiplayer/net path rather than the single-player
/// runtime runner — excluded from the "must be in src/lib.rs" check.
const NET_PATH_SYSTEMS: &[&str] = &["NetSyncSystem"];

/// Runtime data the engine MUST keep loading into the DataStore — these are the
/// keys whose absence silently no-ops a registered system (the audit's core bug).
/// Each must appear in src/lib.rs (the native runtime wiring).
const REQUIRED_RUNTIME_DATA: &[&str] = &[
    "item_registry",
    "recipe_registry",
    "plant_registry",
    "container_registry",
    "status_effect_registry",
    "skill_registry",
    "quest_registry",
    "game_time",
];

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                collect_rs_files(&p, out);
            } else if p.extension().map(|x| x == "rs").unwrap_or(false) {
                out.push(p);
            }
        }
    }
}

/// Take the leading Rust identifier from a string slice.
fn leading_ident(s: &str) -> String {
    s.chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

/// All `impl System for X` type names found under src/systems/.
fn implemented_systems() -> BTreeSet<String> {
    let mut files = Vec::new();
    collect_rs_files(&manifest_dir().join("src").join("systems"), &mut files);
    let mut set = BTreeSet::new();
    for f in files {
        let src = fs::read_to_string(&f).unwrap_or_default();
        for line in src.lines() {
            if let Some(rest) = line.trim().strip_prefix("impl System for ") {
                let name = leading_ident(rest);
                if !name.is_empty() {
                    set.insert(name);
                }
            }
        }
    }
    set
}

/// System type names registered in src/lib.rs (the native runtime runner) via
/// `register(X::new())` or `register(X)`.
fn registered_systems() -> BTreeSet<String> {
    let src = fs::read_to_string(manifest_dir().join("src").join("lib.rs")).unwrap_or_default();
    let mut set = BTreeSet::new();
    for (i, _) in src.match_indices("register(") {
        let after = &src[i + "register(".len()..];
        // Take the constructor PATH (identifier + `::` segments) up to the first
        // non-path char, e.g. `crate::systems::solar::SolarSystem::new`,
        // `FarmingSystem::new`, or just `PlayerControllerSystem`.
        let path: String = after
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == ':')
            .collect();
        // The system TYPE is the LAST UpperCamelCase segment — this skips the module
        // path and the lowercase `new`, so a fully-qualified `register(crate::..::X::new())`
        // is detected just like the short `register(X::new())` form. (Before this, the
        // leading-ident approach read `crate` from a path-qualified call and silently
        // missed the registration — a false positive that kept this lint red.)
        if let Some(name) = path
            .split("::")
            .filter(|seg| seg.chars().next().map(|c| c.is_uppercase()).unwrap_or(false))
            .last()
        {
            set.insert(name.to_string());
        }
    }
    set
}

#[test]
fn every_implemented_system_is_registered_or_deferred() {
    let implemented = implemented_systems();
    let registered = registered_systems();
    let deferred: BTreeSet<String> = DEFERRED_SYSTEMS.iter().map(|(n, _)| n.to_string()).collect();
    let net: BTreeSet<String> = NET_PATH_SYSTEMS.iter().map(|s| s.to_string()).collect();

    assert!(
        !implemented.is_empty(),
        "scan found no `impl System for` under src/systems/ — the scanner path is wrong"
    );

    // (1) Every implemented system must be registered, deferred, or net-path.
    let unclassified: Vec<&String> = implemented
        .iter()
        .filter(|s| !registered.contains(*s) && !deferred.contains(*s) && !net.contains(*s))
        .collect();
    assert!(
        unclassified.is_empty(),
        "These systems have `impl System` but are neither registered in src/lib.rs nor listed in \
         DEFERRED_SYSTEMS with a reason. Register them, or defer them on purpose:\n  {}",
        unclassified
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );

    // (2) A deferred system that IS now registered must leave the allowlist (keep it shrinking).
    let stale: Vec<&str> = DEFERRED_SYSTEMS
        .iter()
        .filter(|(n, _)| registered.contains(*n))
        .map(|(n, _)| *n)
        .collect();
    assert!(
        stale.is_empty(),
        "These systems are registered but still listed in DEFERRED_SYSTEMS — remove them:\n  {}",
        stale.join("\n  ")
    );

    // (3) A deferred entry with no matching `impl System` is stale (renamed/removed).
    let nonexistent: Vec<&str> = DEFERRED_SYSTEMS
        .iter()
        .filter(|(n, _)| !implemented.contains(*n))
        .map(|(n, _)| *n)
        .collect();
    assert!(
        nonexistent.is_empty(),
        "These DEFERRED_SYSTEMS no longer have an `impl System` (renamed/removed?) — clean up:\n  {}",
        nonexistent.join("\n  ")
    );
}

#[test]
fn core_runtime_data_stays_wired() {
    // The registries + game_time must keep being loaded into the runtime DataStore.
    // This locks the Wiring-1/Wiring-2 fixes: deleting a load would empty the
    // DataStore again and silently no-op the consuming system — exactly the bug
    // the audit found. (We check src/lib.rs references the key; the load helpers +
    // the from_csv constructors are covered by their own unit/integration tests.)
    let src = fs::read_to_string(manifest_dir().join("src").join("lib.rs")).unwrap_or_default();
    let missing: Vec<&str> = REQUIRED_RUNTIME_DATA
        .iter()
        .filter(|key| !src.contains(&format!("\"{key}\"")))
        .copied()
        .collect();
    assert!(
        missing.is_empty(),
        "src/lib.rs no longer references these runtime DataStore keys (a registry load was \
         removed? the consuming system will silently no-op):\n  {}",
        missing.join("\n  ")
    );
}
