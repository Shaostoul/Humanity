//! Offline save/load lifecycle (v0.381) -- homes increment 3.
//!
//! Before this, the game persisted NOTHING between sessions: `persistence::*` was
//! wired only in tests, and entering the 3D world regenerates the homestead fresh.
//! This wires the minimal, correct first slice: the player's INVENTORY + SKILLS
//! (their actual progress) are captured into the active offline home on exit +
//! periodically, and applied back on startup, so your homestead progress sticks.
//!
//! Why apply at STARTUP (not on 3D-enter): the ECS player entity is the source of
//! truth and the systems tick every frame -- in the menu-driven loops AND in 3D --
//! so the player accumulates progress always. Applying on startup also makes the
//! exit-save SAFE: the player always carries the loaded state, so closing without
//! playing round-trips the save instead of overwriting it with an empty inventory.
//!
//! Deferred (need new WorldSave fields or extra care -- see docs/design/
//! homes-as-profiles.md): health, position, vitals. So on reload you wake rested
//! at home. Crops, quests, vehicles, wallet and the world clock round-trip; the
//! clock and the offline catch-up are `resume_home` below.

use crate::ecs::components::Controllable;
use crate::persistence::{self, WorldSave};
use crate::systems::inventory::Inventory;
use crate::systems::skills::{PlayerSkills, SkillProgress};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

/// Wall-clock seconds at the last periodic save (0 = not armed yet). A process
/// singleton, fine for a single-instance desktop app.
static LAST_SAVE_SECS: AtomicU64 = AtomicU64::new(0);

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The active offline home's save file. Progressive disclosure: one home for now
/// (the homes-as-profiles model). Multi-home selection comes with multiplayer.
pub fn active_home_path() -> PathBuf {
    persistence::saves_dir().join("offline_home.json")
}

/// Load the active offline home's save, if it exists + parses. None on first run.
pub fn load_active_home() -> Option<WorldSave> {
    let path = active_home_path();
    if !path.exists() {
        return None;
    }
    match persistence::load_world(&path) {
        Ok(s) => Some(s),
        Err(e) => {
            log::warn!("load_active_home: {e}");
            None
        }
    }
}

/// Extract the live player's progress (inventory + skills) into a WorldSave.
pub fn extract_world_save(world: &hecs::World) -> WorldSave {
    let mut save = WorldSave::new_offline("My Homestead", "fibonacci");
    save.timestamp = now_secs();
    // The player is the single Controllable entity.
    for (_e, (inv, skills, name, appearance, outfit, _ctrl)) in world
        .query::<(
            &Inventory,
            &PlayerSkills,
            &crate::ecs::components::Name,
            &crate::ecs::components::Appearance,
            &crate::ecs::components::Outfit,
            &Controllable,
        )>()
        .iter()
    {
        save.character_name = name.0.clone();
        // One (item_id, qty) per occupied slot; apply re-stacks via add_item.
        save.inventory = inv
            .slots
            .iter()
            .filter_map(|s| s.as_ref().map(|st| (st.item_id.clone(), st.quantity)))
            .collect();
        save.inventory_state = inv
            .slots
            .iter()
            .filter_map(|s| s.as_ref().map(|st| (st.wear, st.quality)))
            .collect();
        save.skills = skills
            .skills
            .iter()
            .map(|(id, p)| (id.clone(), (p.level, p.xp)))
            .collect();
        // Avatar appearance + equipped outfit (v0.440).
        save.appearance = appearance.clone();
        save.outfit = outfit.clone();
        break;
    }
    // Deployed vehicles (economy Phase 2 Stage 1, v0.677): every Vehicle entity's
    // kind + pose, so a parked truck is still there after a restart.
    save.deployed_vehicles = world
        .query::<(
            &crate::ecs::components::Vehicle,
            &crate::ecs::components::Transform,
        )>()
        .iter()
        .map(|(_e, (v, t))| crate::persistence::VehicleSave {
            item_id: v.item_id.clone(),
            position: t.position.to_array(),
            yaw: t.rotation.to_euler(glam::EulerRot::YXZ).0,
        })
        .collect();
    // Wallet (v0.747, ladder rung 3): credits survive restarts.
    for (_e, (wallet, _ctrl)) in world
        .query::<(&crate::ecs::components::Wallet, &Controllable)>()
        .iter()
    {
        save.credits = wallet.credits;
        break;
    }
    // Quests (v0.748, ladder rung 4): the tracker round-trips, so progress
    // and completions survive restarts (was: reset fresh every session).
    for (_e, (tracker, _ctrl)) in world
        .query::<(&crate::systems::quests::QuestTracker, &Controllable)>()
        .iter()
    {
        save.quests = Some(tracker.clone());
        break;
    }
    // Crops (v0.863): the whole garden round-trips. planted_at is game-time
    // seconds, so it only means anything next to the clock it was read from:
    // save_active_home stores that clock in save.game_time, and resume_home
    // puts it back on load. (Until 2026-09-25 this comment claimed the clock
    // was saved when nothing wrote it, and every restart rewound the garden.)
    // Each crop with its soil (2026-09-26, N-P-K), and the soil the emptied
    // units remember, so a restart does not refill every unit.
    // And each crop's pollination record (2026-09-26, farming::pollination),
    // and a picked crop's picking state (farming::picking).
    use crate::ecs::components::{CropInstance, CropPicking, CropPollination, CropSoil};
    type Saved = (CropInstance, Option<CropSoil>, Option<CropPollination>, Option<CropPicking>);
    let crops: Vec<Saved> = world
        .query::<(&CropInstance, Option<&CropSoil>, Option<&CropPollination>, Option<&CropPicking>)>()
        .iter()
        .map(|(_e, (c, s, p, k))| (c.clone(), s.cloned(), p.cloned(), k.cloned()))
        .collect();
    save.crop_soil = crops.iter().map(|(_, s, _, _)| s.clone()).collect();
    save.crop_pollination = crops.iter().map(|(_, _, p, _)| p.clone()).collect();
    save.crop_picking = crops.iter().map(|(_, _, _, k)| k.clone()).collect();
    save.crops = crops.into_iter().map(|(c, _, _, _)| c).collect();
    save.soil_memory = world
        .query::<&crate::ecs::components::SoilMemory>()
        .iter()
        .next()
        .map(|(_e, m)| m.clone())
        .unwrap_or_default();
    // Builds (2026-09-25): finished structures AND scaffolds still going up.
    // Until now nothing wrote this field, so everything the player built was
    // gone after a restart even though its materials had been consumed.
    use crate::systems::construction::{Construction, Structure};
    let pose = |t: &crate::ecs::components::Transform| {
        (t.position.to_array(), t.rotation.to_array(), t.scale.to_array())
    };
    for (_e, (s, t)) in world.query::<(&Structure, &crate::ecs::components::Transform)>().iter() {
        let (position, rotation, scale) = pose(t);
        save.constructions.push(crate::persistence::ConstructionSave {
            blueprint_id: s.blueprint_id.clone(),
            position,
            rotation,
            scale,
            health: s.health,
            max_health: s.max_health,
            provides: s.provides.clone(),
            building: None,
        });
    }
    for (_e, (c, t)) in world.query::<(&Construction, &crate::ecs::components::Transform)>().iter() {
        let (position, rotation, scale) = pose(t);
        save.constructions.push(crate::persistence::ConstructionSave {
            blueprint_id: c.blueprint_id.clone(),
            position,
            rotation,
            scale,
            health: 0.0,
            max_health: 0.0,
            provides: None,
            building: Some((c.progress, c.build_time)),
        });
    }
    save
}

/// Apply a loaded WorldSave's inventory + skills + vehicles + crops onto the
/// live world. Health/position/vitals are left fresh -- not yet persisted.
/// Idempotent; called at startup and on character select.
pub fn apply_save_to_world(world: &mut hecs::World, save: &WorldSave) {
    // Only offline homes are supported today.
    if save.kind != "offline" {
        return;
    }
    for (_e, (inv, skills, name, appearance, outfit, _ctrl)) in world.query_mut::<(
        &mut Inventory,
        &mut PlayerSkills,
        &mut crate::ecs::components::Name,
        &mut crate::ecs::components::Appearance,
        &mut crate::ecs::components::Outfit,
        &Controllable,
    )>() {
        if !save.character_name.is_empty() {
            name.0 = save.character_name.clone();
        }
        // Rebuild inventory: clear every slot, then add_item re-stacks.
        for slot in inv.slots.iter_mut() {
            *slot = None;
        }
        // GROW to fit before re-adding (v0.692 review fix): the v0.687 delivery
        // fix legitimately grows the backpack past its base 36 slots, so a save
        // can hold more stacks than Inventory::new(36) offers -- and add_item's
        // discarded overflow here silently ate the excess on the NEXT restart,
        // undoing the never-lose-a-haul guarantee one launch later. Mirror the
        // delivery-site pattern: ensure the slots, then land everything.
        // Each saved stack comes back exactly as it was, with its wear and
        // grade (2026-09-26); a stack an older save has no state for comes
        // back unworn and ungraded.
        inv.ensure_slots(save.inventory.len());
        for (i, (item_id, qty)) in save.inventory.iter().enumerate() {
            let (wear, quality) = save.inventory_state.get(i).copied().unwrap_or((0, 0));
            let mut stack = crate::systems::inventory::ItemStack::new(item_id.clone(), *qty, (*qty).max(99));
            stack.wear = wear;
            stack.quality = quality;
            inv.slots[i] = Some(stack);
        }
        // Rebuild skills.
        skills.skills.clear();
        for (id, (level, xp)) in &save.skills {
            skills
                .skills
                .insert(id.clone(), SkillProgress { level: *level, xp: *xp });
        }
        // Restore avatar appearance + outfit (v0.440).
        *appearance = save.appearance.clone();
        *outfit = save.outfit.clone();
        break;
    }
    // Wallet (v0.747): -1 = a pre-wallet save; keep the fresh-start default.
    if save.credits >= 0 {
        for (_e, (wallet, _ctrl)) in
            world.query_mut::<(&mut crate::ecs::components::Wallet, &Controllable)>()
        {
            wallet.credits = save.credits;
            break;
        }
    }
    // Quests (v0.748): a saved tracker replaces the fresh spawn default
    // (which auto-accepted gs_first_steps); None keeps the fresh start.
    if let Some(saved) = &save.quests {
        for (_e, (tracker, _ctrl)) in
            world.query_mut::<(&mut crate::systems::quests::QuestTracker, &Controllable)>()
        {
            *tracker = saved.clone();
            break;
        }
    }
    // Deployed vehicles (economy Phase 2 Stage 1): the save is AUTHORITATIVE,
    // exactly like inventory above (clear every slot, then rebuild). Despawn
    // every existing Vehicle, then respawn the saved set. This matters because
    // this fn is NOT startup-only: the launcher's character select (lib.rs
    // "launcher_pending_load") re-applies a save onto the live world, so an
    // add-without-clear here would leak vehicles across saves, and the earlier
    // same-pose skip guard silently collapsed two identically-parked vehicles
    // (deploy twice without moving) into one on reload (v0.678 review fix).
    let existing: Vec<hecs::Entity> = world
        .query_mut::<&crate::ecs::components::Vehicle>()
        .into_iter()
        .map(|(e, _)| e)
        .collect();
    for e in existing {
        let _ = world.despawn(e);
    }
    for vs in &save.deployed_vehicles {
        // Same tuple VehicleSystem::handle_deploy spawns, minus Name (the display
        // name lives in the kit registry, which this fn deliberately has no access
        // to; nothing reads a vehicle's Name yet — revisit when nameplates land).
        world.spawn((
            crate::ecs::components::Vehicle { item_id: vs.item_id.clone() },
            crate::ecs::components::Transform {
                position: glam::Vec3::from_array(vs.position),
                rotation: glam::Quat::from_rotation_y(vs.yaw),
                scale: glam::Vec3::ONE,
            },
            crate::ecs::components::Velocity::default(),
            crate::ecs::components::VehicleSeat {
                occupant_key: None,
                seat_type: "pilot".to_string(),
            },
        ));
    }
    // Crops (v0.863): save is AUTHORITATIVE, same clear-then-rebuild rule as
    // vehicles above (this fn re-applies on character select, not just boot).
    let existing: Vec<hecs::Entity> = world
        .query_mut::<&crate::ecs::components::CropInstance>()
        .into_iter()
        .map(|(e, _)| e)
        .collect();
    for e in existing {
        let _ = world.despawn(e);
    }
    for (i, c) in save.crops.iter().enumerate() {
        let e = world.spawn((c.clone(),));
        if let Some(soil) = save.crop_soil.get(i).cloned().flatten() {
            let _ = world.insert_one(e, soil);
        }
        if let Some(pollination) = save.crop_pollination.get(i).cloned().flatten() {
            let _ = world.insert_one(e, pollination);
        }
        if let Some(picking) = save.crop_picking.get(i).cloned().flatten() {
            let _ = world.insert_one(e, picking);
        }
    }
    // One soil memory per world (2026-09-26): the save's replaces the live one.
    let old: Vec<hecs::Entity> = world
        .query::<&crate::ecs::components::SoilMemory>()
        .iter()
        .map(|(e, _)| e)
        .collect();
    for e in old {
        let _ = world.despawn(e);
    }
    world.spawn((save.soil_memory.clone(),));
    // Builds (2026-09-25): authoritative like crops and vehicles. The
    // ConstructionSystem finishes a restored scaffold on its own tick,
    // with the usual completion events, once progress reaches build_time.
    use crate::systems::construction::{Construction, Structure};
    let existing: Vec<hecs::Entity> = world
        .query_mut::<hecs::Or<&Structure, &Construction>>()
        .into_iter()
        .map(|(e, _)| e)
        .collect();
    for e in existing {
        let _ = world.despawn(e);
    }
    for b in &save.constructions {
        let transform = crate::ecs::components::Transform {
            position: glam::Vec3::from_array(b.position),
            rotation: glam::Quat::from_array(b.rotation),
            scale: glam::Vec3::from_array(b.scale),
        };
        match b.building {
            Some((progress, build_time)) => {
                world.spawn((
                    transform,
                    Construction {
                        blueprint_id: b.blueprint_id.clone(),
                        progress,
                        build_time,
                        builder_key: None,
                    },
                ));
            }
            None => {
                world.spawn((
                    transform,
                    Structure {
                        blueprint_id: b.blueprint_id.clone(),
                        health: b.health,
                        max_health: b.max_health,
                        provides: b.provides.clone(),
                    },
                ));
            }
        }
    }
}

/// A NEW player's starting kit: `starting_items` in data/world/player.ron
/// (the data dir first, the embedded copy otherwise). Empty when the file
/// is missing or does not parse, so a broken data file never blocks a boot.
pub fn starting_kit(data_dir: &std::path::Path) -> Vec<(String, u32)> {
    #[derive(serde::Deserialize)]
    struct PlayerDef {
        #[serde(default)]
        starting_items: Vec<(String, u32)>,
    }
    crate::embedded_data::read_data_or_embedded(data_dir, "world/player.ron")
        .and_then(|text| match ron::from_str::<PlayerDef>(&text) {
            Ok(def) => Some(def.starting_items),
            Err(e) => {
                log::warn!("world/player.ron did not parse, starting kit empty: {e}");
                None
            }
        })
        .unwrap_or_default()
}

/// Apply ONLY the character (name, look, outfit) from a save: the path for
/// "Start every session from the default home" and for a save that carries
/// no progress (`progress_saved == false`). The home, inventory, garden,
/// builds and clock stay the default.
pub fn apply_identity(world: &mut hecs::World, save: &WorldSave) {
    for (_e, (name, appearance, outfit, _ctrl)) in world.query_mut::<(
        &mut crate::ecs::components::Name,
        &mut crate::ecs::components::Appearance,
        &mut crate::ecs::components::Outfit,
        &Controllable,
    )>() {
        if !save.character_name.is_empty() {
            name.0 = save.character_name.clone();
        }
        *appearance = save.appearance.clone();
        *outfit = save.outfit.clone();
        break;
    }
}

/// The save to write when progress is NOT being kept: the existing save
/// with only the character replaced, so the progress on disk survives
/// untouched for when the setting is turned off. With no save yet, a
/// character-only save marked `progress_saved: false`.
pub fn identity_only_save(existing: Option<WorldSave>, world: &hecs::World) -> WorldSave {
    let current = extract_world_save(world);
    let mut save = existing.unwrap_or_else(|| {
        let mut s = WorldSave::new_offline("My Homestead", "fibonacci");
        s.progress_saved = false;
        s
    });
    save.character_name = current.character_name;
    save.appearance = current.appearance;
    save.outfit = current.outfit;
    save
}

/// Extract + write the active offline home to disk. Logs on failure. `placed` is the
/// organize-layer container pool (GuiState-owned, not in the ECS world), persisted
/// alongside the world-derived save so container contents + transfers survive a restart.
pub fn save_active_home(
    world: &hecs::World,
    placed: &[crate::gui::PlacedItem],
    data: &crate::hot_reload::data_store::DataStore,
    keep_progress: bool,
) {
    if !keep_progress {
        // "Start every session from the default home": record the character,
        // leave any progress save exactly as it was.
        let save = identity_only_save(load_active_home(), world);
        if let Err(e) = persistence::save_world(&active_home_path(), &save) {
            log::error!("save_active_home (character only) failed: {e}");
        }
        return;
    }
    let mut save = extract_world_save(world);
    save.placed_items = placed.to_vec();
    // The world clock, from the TimeSystem's DataStore export. Crop
    // planted_at values are only meaningful against it.
    save.game_time = crate::systems::time::elapsed_now(data);
    save.progress_saved = true;
    // Craft batches in flight, from the CraftingSystem's export (the list
    // lives inside the system). Their inputs are already spent.
    save.crafts = data
        .get::<std::sync::Mutex<Vec<crate::systems::crafting::CraftSave>>>("active_crafts_export")
        .and_then(|m| m.lock().ok().map(|v| v.clone()))
        .unwrap_or_default();
    let path = active_home_path();
    if let Err(e) = persistence::save_world(&path, &save) {
        log::error!("save_active_home failed: {e}");
    } else {
        log::info!(
            "Saved offline home: {} item stacks, {} skills",
            save.inventory.len(),
            save.skills.len()
        );
    }
}

/// Save the offline home at most once per `interval_secs` of wall-clock time. Call
/// every frame from the main loop; it self-throttles. Robust to ANY exit path
/// (in-app quit, crash, kill) where the graceful close-save would not fire.
pub fn maybe_periodic_save(
    world: &hecs::World,
    placed: &[crate::gui::PlacedItem],
    data: &crate::hot_reload::data_store::DataStore,
    keep_progress: bool,
    interval_secs: u64,
) {
    let now = now_secs();
    let last = LAST_SAVE_SECS.load(Ordering::Relaxed);
    if last == 0 {
        // First call: arm the timer; do NOT save immediately (avoids writing an
        // empty home before any play happens on a fresh first run).
        LAST_SAVE_SECS.store(now, Ordering::Relaxed);
        return;
    }
    if now.saturating_sub(last) >= interval_secs {
        LAST_SAVE_SECS.store(now, Ordering::Relaxed);
        save_active_home(world, placed, data, keep_progress);
    }
}

/// What `resume_home` did, for the log and the "while you were away" notice.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Resumed {
    /// The game clock the world resumes at, in game seconds.
    pub clock: f64,
    /// Game seconds of offline catch-up applied (0 when the toggle is off).
    pub away_secs: f64,
    /// Living crops that were aged by `away_secs`.
    pub crops_aged: usize,
    /// Builds still under construction that were advanced by `away_secs`.
    pub builds_advanced: usize,
    /// Craft batches in flight that were advanced by `away_secs`.
    pub crafts_advanced: usize,
}

/// Offline progression (operator, 2026-09-21; docs/design/offline-progression.md).
/// Pure part of `resume_home`, so it can be tested without a DataStore.
///
/// The CLOCK always resumes where the save left it. It is never jumped forward
/// by the time away, because every system that reads the clock would then
/// advance offline by accident, and the design doc requires the opposite: a
/// system advances offline only by deliberately opting in here. The clock is
/// also never behind the newest crop (a crop cannot have been planted in the
/// future), which heals a save written before the clock was stored.
///
/// Opted in today (crops, and builds under construction):
/// - CROPS: when `offline_progression` is on, every living crop's planted_at
///   moves back by the time away, so its age grows by exactly that much and
///   the growth-speed setting applies to it like any other hour. Water and
///   health are integrated per tick and are NOT advanced: the character keeps
///   the garden watered while you are away (the doc's "offline upkeep"), so
///   nothing can die of thirst while nobody could have prevented it.
///
/// - BUILDS: scaffolds still going up advance by the time away (see below).
/// - CRAFTS: batches in flight count down by the time away and deliver on
///   the CraftingSystem's next tick (see `restored_crafts`).
///
/// Deliberately not advanced: vitals (not persisted; you wake rested), and
/// anything that consumes or destroys. That includes garden PESTS
/// (2026-09-26): their pressure costs crop health and the player could not
/// have answered it while away, so it resumes where the save left it and the
/// character's upkeep kept them down in the meantime. Nor does a picked
/// crop's picking window (2026-09-26, farming::picking): its clock runs on
/// the tick, so produce the player could not have picked while away does
/// not pass over, and the window resumes where the save left it. A crop's soil draw is
/// not in that class: the growth made offline is paid for from its unit on
/// the first tick back (farming's uptake), which is the crop's own feeding.
///
/// Clock source: the device clock, because only offline single-player homes
/// are saved today. Multiplayer and MMO must use the SERVER clock instead
/// (the design doc's cheating section) when their saves exist.
///
/// A game second is a real second at time scale 1 (SECONDS_PER_DAY is
/// defined that way), so real seconds away convert one to one. The time
/// scale is a dev scrubber and is not saved, so it does not stretch the
/// time away. No cap: a returning player finding a finished garden is the
/// point (the doc's open question, unbounded until something misbehaves).
pub fn catch_up_world(
    world: &mut hecs::World,
    save: &WorldSave,
    offline_progression: bool,
    now: u64,
) -> Resumed {
    let newest_planting = world
        .query_mut::<&crate::ecs::components::CropInstance>()
        .into_iter()
        .map(|(_e, c)| c.planted_at)
        .fold(0.0_f64, f64::max);
    let clock = save.game_time.max(newest_planting).max(0.0);
    // timestamp 0 = never stamped by a save, so there is no "away" to measure.
    // A clock set backwards gives zero, never negative growth.
    let away_secs = if offline_progression && save.timestamp > 0 {
        now.saturating_sub(save.timestamp) as f64
    } else {
        0.0
    };
    let mut crops_aged = 0;
    let mut builds_advanced = 0;
    if away_secs > 0.0 {
        for (_e, crop) in world.query_mut::<&mut crate::ecs::components::CropInstance>() {
            if crop.growth_stage == crate::ecs::components::STAGE_DEAD {
                continue;
            }
            crop.planted_at -= away_secs;
            crops_aged += 1;
        }
        // BUILDS: a scaffold's materials were consumed when it started, so
        // finishing it offline consumes nothing (the doc's "reserve inputs up
        // front" rule). Progress is a countdown in seconds; it is capped at
        // build_time so the ConstructionSystem's next tick does the
        // completion itself, quest event, skill XP and all.
        for (_e, c) in world.query_mut::<&mut crate::systems::construction::Construction>() {
            c.progress = (c.progress as f64 + away_secs).min(c.build_time as f64) as f32;
            builds_advanced += 1;
        }
    }
    let crafts_advanced = if away_secs > 0.0 { save.crafts.len() } else { 0 };
    Resumed { clock, away_secs, crops_aged, builds_advanced, crafts_advanced }
}

/// The craft batches a save resumes with, each counted down by the time away
/// (floored at zero, which completes it on the next tick through the normal
/// delivery path, vehicle pad and machine vessel included). Their inputs were
/// spent when they started, so finishing them offline spends nothing; this is
/// the design doc's "reserve inputs up front" rule, already true of crafting.
pub fn restored_crafts(save: &WorldSave, away_secs: f64) -> Vec<crate::systems::crafting::CraftSave> {
    save.crafts
        .iter()
        .map(|c| {
            let mut c = c.clone();
            c.time_remaining = (c.time_remaining as f64 - away_secs).max(0.0) as f32;
            c
        })
        .collect()
}

/// The one-line "while you were away" notice, or None when nothing grew. The
/// catch-up must never be silent: a garden that jumped forward with no word
/// reads as a bug, not as the character having lived the hours.
pub fn away_notice(r: &Resumed) -> Option<String> {
    if r.away_secs < 60.0 || (r.crops_aged == 0 && r.builds_advanced == 0 && r.crafts_advanced == 0) {
        return None;
    }
    let mins = (r.away_secs / 60.0) as u64;
    let span = if mins >= 48 * 60 {
        format!("{} days", mins / (24 * 60))
    } else if mins >= 60 {
        format!("{} h {} min", mins / 60, mins % 60)
    } else {
        format!("{mins} min")
    };
    let count = |n: usize, one: &str, many: &str| {
        if n == 1 { format!("1 {one}") } else { format!("{n} {many}") }
    };
    let mut parts = Vec::new();
    if r.crops_aged > 0 {
        parts.push(format!("{} kept growing", count(r.crops_aged, "plant", "plants")));
    }
    if r.builds_advanced > 0 {
        parts.push(format!("{} kept going up", count(r.builds_advanced, "build", "builds")));
    }
    if r.crafts_advanced > 0 {
        parts.push(format!("{} kept working", count(r.crafts_advanced, "craft", "crafts")));
    }
    let list = match parts.len() {
        1 => parts[0].clone(),
        2 => format!("{} and {}", parts[0], parts[1]),
        _ => format!("{}, {} and {}", parts[0], parts[1], parts[2]),
    };
    Some(format!("While you were away ({span}), {list}."))
}

/// Put the world clock back where `save` left it and apply the offline
/// catch-up. Call right after `apply_save_to_world` with the same save, at
/// startup and on character select, and after the config is loaded (the
/// toggle lives there). Idempotent per save: both inputs come from disk, so
/// re-applying the same save lands in the same place.
pub fn resume_home(
    world: &mut hecs::World,
    data: &crate::hot_reload::data_store::DataStore,
    save: &WorldSave,
    offline_progression: bool,
) -> Resumed {
    let r = catch_up_world(world, save, offline_progression, now_secs());
    crate::systems::time::request_restore_elapsed(data, r.clock);
    // Craft batches go through the CraftingSystem's restore channel, which
    // it consumes after its rewind drop, so a character select replaces the
    // live batches with the saved ones instead of losing both.
    if let Some(slot) = data
        .get::<std::sync::Mutex<Option<Vec<crate::systems::crafting::CraftSave>>>>("restore_active_crafts")
    {
        if let Ok(mut s) = slot.lock() {
            *s = Some(restored_crafts(save, r.away_secs));
        }
    }
    log::info!(
        "Resumed home clock at game second {:.0}; offline catch-up {} ({:.0} s away, {} crops aged, {} builds and {} crafts advanced)",
        r.clock,
        if offline_progression { "on" } else { "off" },
        r.away_secs,
        r.crops_aged,
        r.builds_advanced,
        r.crafts_advanced
    );
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_apply_round_trips_inventory_and_skills() {
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(36);
        inv.add_item("wood_plank_0", 40, 99);
        inv.add_item("steel_ingot_0", 8, 99);
        let mut skills = PlayerSkills::new();
        skills
            .skills
            .insert("farming".to_string(), SkillProgress { level: 3, xp: 450 });
        let mut appearance = crate::ecs::components::Appearance::default();
        appearance.skin_tone = [0.4, 0.3, 0.2];
        appearance.height_scale = 1.2;
        let mut outfit = crate::ecs::components::Outfit::default();
        outfit.equipped.insert("chest".to_string(), "work_jacket".to_string());
        world.spawn((
            Controllable,
            inv,
            skills,
            crate::ecs::components::Name("Astra".to_string()),
            appearance,
            outfit,
        ));

        let save = extract_world_save(&world);
        assert!(save.inventory.iter().any(|(id, q)| id == "wood_plank_0" && *q == 40));
        assert_eq!(save.skills.get("farming").copied(), Some((3, 450)));
        assert_eq!(save.character_name, "Astra");
        assert_eq!(save.appearance.skin_tone, [0.4, 0.3, 0.2]);
        assert_eq!(save.outfit.equipped.get("chest").map(|s| s.as_str()), Some("work_jacket"));

        // Wipe the live state, then apply the save back.
        for (_e, (inv, skills, _c)) in
            world.query_mut::<(&mut Inventory, &mut PlayerSkills, &Controllable)>()
        {
            for s in inv.slots.iter_mut() {
                *s = None;
            }
            skills.skills.clear();
        }
        apply_save_to_world(&mut world, &save);

        // Verify the restore via a fresh extract.
        let restored = extract_world_save(&world);
        let wood: u32 = restored
            .inventory
            .iter()
            .filter(|(id, _)| id == "wood_plank_0")
            .map(|(_, q)| *q)
            .sum();
        assert_eq!(wood, 40);
        assert_eq!(restored.skills.get("farming").copied(), Some((3, 450)));
        // Appearance + outfit survive the round-trip too (v0.440).
        assert_eq!(restored.character_name, "Astra");
        assert_eq!(restored.appearance.skin_tone, [0.4, 0.3, 0.2]);
        assert_eq!(restored.appearance.height_scale, 1.2);
        assert_eq!(restored.outfit.equipped.get("chest").map(|s| s.as_str()), Some("work_jacket"));
    }

    #[test]
    fn apply_ignores_non_offline_kind() {
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(36);
        inv.add_item("wood_plank_0", 5, 99);
        world.spawn((
            Controllable,
            inv,
            PlayerSkills::new(),
            crate::ecs::components::Name("X".to_string()),
            crate::ecs::components::Appearance::default(),
            crate::ecs::components::Outfit::default(),
        ));

        let mut save = WorldSave::new_offline("X", "fibonacci");
        save.kind = "server".to_string();
        save.inventory = vec![("steel_ingot_0".to_string(), 99)];
        apply_save_to_world(&mut world, &save); // should be a no-op

        let after = extract_world_save(&world);
        // Untouched: still the original wood, no injected steel.
        assert!(after.inventory.iter().any(|(id, q)| id == "wood_plank_0" && *q == 5));
        assert!(!after.inventory.iter().any(|(id, _)| id == "steel_ingot_0"));
    }


    /// Range-review fix (v0.692): a GROWN backpack (the v0.687 delivery fix
    /// legitimately expands past 36 slots) must round-trip the save -- the
    /// load path used to rebuild into Inventory::new(36) and discard
    /// add_item's overflow, silently eating the extra stacks one restart
    /// after the delivery rescued them.
    #[test]
    fn grown_backpack_saves_round_trip_without_losing_stacks() {
        let mut world = hecs::World::new();
        // 37 DISTINCT unstackable items: one more than the base 36 slots.
        let mut inv = Inventory::new(36);
        for i in 0..36 {
            inv.add_item(&format!("junk_{i}"), 1, 1);
        }
        inv.ensure_slots(37);
        inv.add_item("iron_ore_0", 1, 1); // the rescued haul
        world.spawn((
            Controllable,
            inv,
            PlayerSkills::new(),
            crate::ecs::components::Name("Hauler".to_string()),
            crate::ecs::components::Appearance::default(),
            crate::ecs::components::Outfit::default(),
        ));

        let save = extract_world_save(&world);
        assert_eq!(save.inventory.len(), 37, "the grown backpack saved all 37 stacks");

        // Fresh world with the BASE 36-slot inventory, like a restart.
        let mut fresh = hecs::World::new();
        let player = fresh.spawn((
            Controllable,
            Inventory::new(36),
            PlayerSkills::new(),
            crate::ecs::components::Name("X".to_string()),
            crate::ecs::components::Appearance::default(),
            crate::ecs::components::Outfit::default(),
        ));
        apply_save_to_world(&mut fresh, &save);
        let inv = fresh.get::<&Inventory>(player).unwrap();
        let stacks = inv.slots.iter().filter(|s| s.is_some()).count();
        assert_eq!(stacks, 37, "all 37 stacks survived the restart");
        assert_eq!(inv.count_item("iron_ore_0"), 1, "the rescued haul survived");
    }

    /// A deployed vehicle survives the full extract -> apply round trip (economy
    /// Phase 2 Stage 1): the truck the player deployed is still parked where they
    /// left it after a restart, and a second apply doesn't stack a duplicate.
    #[test]
    fn deployed_vehicles_survive_the_save_round_trip() {
        use crate::ecs::components::{Transform, Vehicle, VehicleSeat, Velocity};
        let mut world = hecs::World::new();
        world.spawn((
            Controllable,
            Inventory::new(36),
            PlayerSkills::new(),
            crate::ecs::components::Name("Driver".to_string()),
            crate::ecs::components::Appearance::default(),
            crate::ecs::components::Outfit::default(),
        ));
        world.spawn((
            Vehicle { item_id: "truck_pickup_0".to_string() },
            Transform {
                position: glam::Vec3::new(12.0, 0.0, -7.5),
                rotation: glam::Quat::from_rotation_y(1.25),
                scale: glam::Vec3::ONE,
            },
            Velocity::default(),
            VehicleSeat { occupant_key: None, seat_type: "pilot".to_string() },
        ));

        let save = extract_world_save(&world);
        assert_eq!(save.deployed_vehicles.len(), 1);
        assert_eq!(save.deployed_vehicles[0].item_id, "truck_pickup_0");
        assert!((save.deployed_vehicles[0].yaw - 1.25).abs() < 1e-4);

        // Serde round trip (what actually hits disk), then apply to a FRESH world.
        let json = serde_json::to_string(&save).expect("serialize");
        let loaded: WorldSave = serde_json::from_str(&json).expect("deserialize");
        let mut fresh = hecs::World::new();
        fresh.spawn((
            Controllable,
            Inventory::new(36),
            PlayerSkills::new(),
            crate::ecs::components::Name("X".to_string()),
            crate::ecs::components::Appearance::default(),
            crate::ecs::components::Outfit::default(),
        ));
        apply_save_to_world(&mut fresh, &loaded);
        let vehicles: Vec<(String, glam::Vec3)> = fresh
            .query_mut::<(&Vehicle, &Transform)>()
            .into_iter()
            .map(|(_e, (v, t))| (v.item_id.clone(), t.position))
            .collect();
        assert_eq!(vehicles.len(), 1, "the parked truck came back");
        assert_eq!(vehicles[0].0, "truck_pickup_0");
        assert!((vehicles[0].1 - glam::Vec3::new(12.0, 0.0, -7.5)).length() < 1e-4);

        // Applying the same save again must NOT duplicate the vehicle.
        apply_save_to_world(&mut fresh, &loaded);
        let n = fresh.query_mut::<&Vehicle>().into_iter().count();
        assert_eq!(n, 1, "idempotent re-apply");

        // The save is authoritative (v0.678 review fix): applying a DIFFERENT
        // save clears vehicles that aren't in it — the launcher's character
        // switch must not leak one character's trucks into another's world.
        let empty = WorldSave::new_offline("Other", "fibonacci");
        apply_save_to_world(&mut fresh, &empty);
        let n = fresh.query_mut::<&Vehicle>().into_iter().count();
        assert_eq!(n, 0, "vehicles absent from the applied save are despawned");
    }

    /// Two vehicles deployed at the IDENTICAL pose (deploy twice without moving)
    /// must both come back after a restart. The v0.677 same-pose skip guard
    /// collapsed them to one — two kits paid, one truck restored (review fix).
    #[test]
    fn two_identically_parked_vehicles_both_survive_reload() {
        use crate::ecs::components::{Transform, Vehicle, VehicleSeat, Velocity};
        let mut world = hecs::World::new();
        world.spawn((
            Controllable,
            Inventory::new(36),
            PlayerSkills::new(),
            crate::ecs::components::Name("Driver".to_string()),
            crate::ecs::components::Appearance::default(),
            crate::ecs::components::Outfit::default(),
        ));
        let pose = Transform {
            position: glam::Vec3::new(3.0, 0.0, 9.0),
            rotation: glam::Quat::from_rotation_y(0.4),
            scale: glam::Vec3::ONE,
        };
        for _ in 0..2 {
            world.spawn((
                Vehicle { item_id: "rover_0".to_string() },
                pose.clone(),
                Velocity::default(),
                VehicleSeat { occupant_key: None, seat_type: "pilot".to_string() },
            ));
        }

        let save = extract_world_save(&world);
        assert_eq!(save.deployed_vehicles.len(), 2);

        let mut fresh = hecs::World::new();
        fresh.spawn((
            Controllable,
            Inventory::new(36),
            PlayerSkills::new(),
            crate::ecs::components::Name("X".to_string()),
            crate::ecs::components::Appearance::default(),
            crate::ecs::components::Outfit::default(),
        ));
        apply_save_to_world(&mut fresh, &save);
        let n = fresh.query_mut::<&Vehicle>().into_iter().count();
        assert_eq!(n, 2, "both identically-parked rovers restore");

        // And an old save without the field loads with none (serde default).
        let old_json = r#"{"name":"Old","timestamp":0,"game_time":0.0,
            "player_position":[0.0,0.0,0.0],"player_rotation":[0.0,0.0,0.0,1.0],
            "player_health":100.0,"inventory":[],"skills":{},"constructions":[],
            "weather_state":"clear"}"#;
        let old: WorldSave = serde_json::from_str(old_json).expect("old save loads");
        assert!(old.deployed_vehicles.is_empty());
    }

    /// Regression lock (v0.678, found by the pre-commit review): re-applying a
    /// STALE disk save after a deploy must rewind the WHOLE world consistently —
    /// the kit comes back to the inventory AND the truck despawns. The pre-fix
    /// additive vehicle loop let both exist at once (save-scum duplication).
    #[test]
    fn stale_reapply_rewinds_instead_of_duplicating() {
        use crate::ecs::components::{Transform, Vehicle, VehicleSeat, Velocity};
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(36);
        inv.add_item("truck_pickup_kit_0", 1, 1);
        world.spawn((
            Controllable,
            inv,
            PlayerSkills::new(),
            crate::ecs::components::Name("Driver".to_string()),
            crate::ecs::components::Appearance::default(),
            crate::ecs::components::Outfit::default(),
        ));
        // T0: periodic save fires while the kit is still in the backpack.
        let stale_disk_save = extract_world_save(&world);
        assert_eq!(stale_disk_save.deployed_vehicles.len(), 0);
        assert!(stale_disk_save.inventory.iter().any(|(id, _)| id == "truck_pickup_kit_0"));

        // T1: player deploys the kit (what handle_deploy does: consume + spawn).
        for (_e, (inv, _c)) in world.query_mut::<(&mut Inventory, &Controllable)>() {
            inv.remove_item("truck_pickup_kit_0", 1);
        }
        world.spawn((
            Vehicle { item_id: "truck_pickup_0".to_string() },
            Transform { position: glam::Vec3::new(3.0, 0.0, -6.0), rotation: glam::Quat::IDENTITY, scale: glam::Vec3::ONE },
            Velocity::default(),
            VehicleSeat { occupant_key: None, seat_type: "pilot".to_string() },
        ));

        // T2: player clicks their character in the launcher (lib.rs
        // launcher_pending_load) -> apply_save_to_world with the STALE disk
        // save. Pre-v0.678 this DUPLICATED value: the inventory rebuild
        // resurrected the kit while the additive vehicle loop left the truck
        // standing — one kit became kit + truck via save-scumming. The save is
        // now authoritative for vehicles exactly as it is for inventory, so the
        // whole world rewinds consistently: kit back, truck gone.
        apply_save_to_world(&mut world, &stale_disk_save);

        let kit_count: u32 = world
            .query_mut::<(&Inventory, &Controllable)>()
            .into_iter()
            .map(|(_e, (inv, _c))| inv.count_item("truck_pickup_kit_0"))
            .sum();
        let truck_count = world.query_mut::<&Vehicle>().into_iter().count();
        assert_eq!(kit_count, 1, "kit restored from the stale save");
        assert_eq!(
            truck_count, 0,
            "the truck is NOT in the stale save — a consistent rewind removes it; \
             kit + truck coexisting was the save-scum duplication"
        );
    }

    /// Organize-layer container contents survive a save serde round-trip, and a
    /// A carried tool keeps its wear and grade across a save (2026-09-26):
    /// the save used to keep only ids and counts, so every restart renewed
    /// every tool and erased its grade.
    #[test]
    fn carried_tools_keep_their_wear_and_grade_across_a_save() {
        use crate::systems::inventory::Inventory;
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(8);
        inv.add_item_q("hammer_0", 1, 1, 6);
        inv.add_item_q("hammer_0", 1, 1, 1);
        inv.slots[0].as_mut().unwrap().wear = 150;
        world.spawn((inv, Controllable, crate::ecs::components::Name("Tester".into()), PlayerSkills::new(), crate::ecs::components::Appearance::default(), crate::ecs::components::Outfit::default()));
        let save = extract_world_save(&world);
        let back: WorldSave = serde_json::from_str(&serde_json::to_string(&save).unwrap()).unwrap();
        let mut fresh = hecs::World::new();
        fresh.spawn((Inventory::new(8), Controllable, crate::ecs::components::Name("Tester".into()), PlayerSkills::new(), crate::ecs::components::Appearance::default(), crate::ecs::components::Outfit::default()));
        apply_save_to_world(&mut fresh, &back);
        let got: Vec<(u32, u8)> = fresh
            .query::<(&Inventory, &Controllable)>()
            .iter()
            .flat_map(|(_, (i, _))| i.slots.iter().flatten().map(|s| (s.wear, s.quality)).collect::<Vec<_>>())
            .collect();
        assert_eq!(got, vec![(150, 6), (0, 1)], "each hammer as it was");
    }

    /// The garden's soil survives a save (2026-09-26, N-P-K): each crop's
    /// store and what emptied units remember come back after a restart.
    #[test]
    fn crop_soil_and_soil_memory_survive_a_save() {
        use crate::ecs::components::{CropInstance, CropSoil, Npk, SoilMemory};
        let mut world = hecs::World::new();
        let crop = CropInstance {
            crop_def_id: "tomato".into(),
            growth_stage: "seedling".into(),
            planted_at: 0.0,
            water_level: 1.0,
            health: 100.0,
            tower_id: Some("bed_1".into()),
            tower_slot: Some(2),
            health_seconds: 0.0,
            growing_seconds: 0.0,
        };
        world.spawn((crop, CropSoil { store: Npk::new(1.0, 2.0, 3.0), uptake: 0.4 }));
        let mut memory = SoilMemory::default();
        memory.units.entry("bed_1".into()).or_default().insert(5, Npk::new(4.0, 5.0, 6.0));
        world.spawn((memory,));
        let save = extract_world_save(&world);
        let text = serde_json::to_string(&save).unwrap();
        let back: WorldSave = serde_json::from_str(&text).unwrap();
        let mut fresh = hecs::World::new();
        apply_save_to_world(&mut fresh, &back);
        let soils: Vec<CropSoil> = fresh.query::<(&CropInstance, &CropSoil)>().iter().map(|(_, (_, s))| s.clone()).collect();
        assert_eq!(soils.len(), 1);
        assert_eq!(soils[0].store, Npk::new(1.0, 2.0, 3.0));
        assert!((soils[0].uptake - 0.4).abs() < 1e-6);
        let mems: Vec<SoilMemory> = fresh.query::<&SoilMemory>().iter().map(|(_, m)| m.clone()).collect();
        assert_eq!(mems.len(), 1, "one soil memory per world");
        assert_eq!(mems[0].units["bed_1"][&5], Npk::new(4.0, 5.0, 6.0));
    }

    /// Each grow room's air survives a save (2026-09-26, farming::humidity):
    /// its vapour, its fan's speed, what its crops were breathing out,
    /// whether the player was told it is humid, and its humidifier's output,
    /// litres and dry flag come back as they were. A save from before the
    /// field (no `rooms` in the soil memory) still loads, with no rooms, so
    /// each starts from the home's air, and one from before the humidifier
    /// (a room with no humidifier fields) loads with it idle. Seen red by
    /// marking `SoilMemory::rooms` `#[serde(skip)]` (the room came back
    /// empty), and by marking `RoomAir::humidifier` `#[serde(skip)]` (its
    /// output came back 0).
    #[test]
    fn grow_room_air_survives_a_save_and_old_saves_load() {
        use crate::ecs::components::{RoomAir, SoilMemory};
        let mut world = hecs::World::new();
        let mut memory = SoilMemory::default();
        let air = RoomAir {
            vapour_g_m3: 16.25,
            fan_speed: 0.4,
            breathed_l_day: 540.0,
            told: true,
            humidifier: 0.88,
            humidifier_l_day: 27.5,
            humidifier_dry: true,
        };
        memory.rooms.insert("room-greenhouse".into(), air);
        world.spawn((memory,));
        let text = serde_json::to_string(&extract_world_save(&world)).unwrap();
        let back: WorldSave = serde_json::from_str(&text).unwrap();
        let mut fresh = hecs::World::new();
        apply_save_to_world(&mut fresh, &back);
        let mems: Vec<SoilMemory> = fresh.query::<&SoilMemory>().iter().map(|(_, m)| m.clone()).collect();
        assert_eq!(mems[0].rooms.get("room-greenhouse"), Some(&air), "the room's air came back");
        // A save from before the humidifier: the room has none of its fields.
        let mut pre: serde_json::Value = serde_json::from_str(&text).unwrap();
        let r = pre["soil_memory"]["rooms"]["room-greenhouse"].as_object_mut().unwrap();
        for k in ["humidifier", "humidifier_l_day", "humidifier_dry"] {
            assert!(r.remove(k).is_some(), "the save wrote {k}");
        }
        let back: WorldSave = serde_json::from_value(pre).unwrap();
        let mut before = hecs::World::new();
        apply_save_to_world(&mut before, &back);
        let mems: Vec<SoilMemory> = before.query::<&SoilMemory>().iter().map(|(_, m)| m.clone()).collect();
        let want = RoomAir { humidifier: 0.0, humidifier_l_day: 0.0, humidifier_dry: false, ..air };
        assert_eq!(mems[0].rooms.get("room-greenhouse"), Some(&want), "a pre-humidifier room loads idle");
        // An older save: its soil memory has no `rooms` at all.
        let mut old: serde_json::Value = serde_json::from_str(&text).unwrap();
        old["soil_memory"].as_object_mut().unwrap().remove("rooms");
        let back: WorldSave = serde_json::from_value(old).unwrap();
        let mut older = hecs::World::new();
        apply_save_to_world(&mut older, &back);
        let mems: Vec<SoilMemory> = older.query::<&SoilMemory>().iter().map(|(_, m)| m.clone()).collect();
        assert!(mems[0].rooms.is_empty(), "an old save loads with no room air");
    }

    /// A crop's pollination record survives a save (2026-09-26,
    /// farming::pollination): the flowering and pollinated days and what is
    /// left of a hand pollination come back on the same crop, and a crop
    /// that had none still has none. A save from before the field (no
    /// `crop_pollination`) still loads, its crops with no record. Seen red
    /// by not writing `crop_pollination` in `extract_world_save`.
    #[test]
    fn crop_pollination_survives_a_save() {
        use crate::ecs::components::{CropInstance, CropPollination};
        let mut world = hecs::World::new();
        let crop = |plant: &str| CropInstance {
            crop_def_id: plant.into(),
            growth_stage: "flower".into(),
            planted_at: 0.0,
            water_level: 1.0,
            health: 100.0,
            tower_id: Some("ntower_3".into()),
            tower_slot: Some(1),
            health_seconds: 0.0,
            growing_seconds: 0.0,
        };
        let rec = CropPollination { flowering_days: 4.5, pollinated_days: 2.25, hand_days_left: 1.5 };
        world.spawn((crop("tomato"), rec.clone()));
        world.spawn((crop("lettuce"),));
        let save = extract_world_save(&world);
        let text = serde_json::to_string(&save).unwrap();
        let back: WorldSave = serde_json::from_str(&text).unwrap();
        let mut fresh = hecs::World::new();
        apply_save_to_world(&mut fresh, &back);
        let got: Vec<(String, Option<CropPollination>)> = fresh
            .query::<(&CropInstance, Option<&CropPollination>)>()
            .iter()
            .map(|(_, (c, p))| (c.crop_def_id.clone(), p.cloned()))
            .collect();
        assert_eq!(got.len(), 2);
        for (plant, p) in got {
            match plant.as_str() {
                "tomato" => assert_eq!(p, Some(rec.clone()), "the tomato's record came back"),
                _ => assert_eq!(p, None, "the lettuce still has none"),
            }
        }
        // An older save: strip the field, and it still loads.
        let mut old: serde_json::Value = serde_json::from_str(&text).unwrap();
        old.as_object_mut().unwrap().remove("crop_pollination");
        let back: WorldSave = serde_json::from_value(old).unwrap();
        let mut older = hecs::World::new();
        apply_save_to_world(&mut older, &back);
        assert_eq!(older.query::<&CropInstance>().iter().count(), 2);
        assert_eq!(older.query::<&CropPollination>().iter().count(), 0);
    }

    /// A picked crop's picking state survives a save (2026-09-26,
    /// farming::picking): how long it has been ripe, the picks taken, its
    /// season roll and the fraction it carries come back on the same crop,
    /// and a crop that had none still has none. A save from before the field
    /// (no `crop_picking`) still loads, its crops with no state (a ripe picked
    /// crop then starts its window afresh). Seen red by not writing
    /// `crop_picking` in `extract_world_save`.
    #[test]
    fn crop_picking_survives_a_save() {
        use crate::ecs::components::{CropInstance, CropPicking};
        let mut world = hecs::World::new();
        let crop = |plant: &str, stage: &str| CropInstance {
            crop_def_id: plant.into(),
            growth_stage: stage.into(),
            planted_at: 0.0,
            water_level: 1.0,
            health: 100.0,
            tower_id: Some("ntower_3".into()),
            tower_slot: Some(1),
            health_seconds: 0.0,
            growing_seconds: 0.0,
        };
        let rec = CropPicking { days_ripe: 12.25, next_pick: 7, taken: 6, roll: Some(0.625), carry: 0.375 };
        world.spawn((crop("tomato", "ripe"), rec.clone()));
        world.spawn((crop("lettuce", "mature"),));
        let save = extract_world_save(&world);
        let text = serde_json::to_string(&save).unwrap();
        let back: WorldSave = serde_json::from_str(&text).unwrap();
        let mut fresh = hecs::World::new();
        apply_save_to_world(&mut fresh, &back);
        let got: Vec<(String, Option<CropPicking>)> = fresh
            .query::<(&CropInstance, Option<&CropPicking>)>()
            .iter()
            .map(|(_, (c, p))| (c.crop_def_id.clone(), p.cloned()))
            .collect();
        assert_eq!(got.len(), 2);
        for (plant, p) in got {
            match plant.as_str() {
                "tomato" => assert_eq!(p, Some(rec.clone()), "the tomato's picking came back"),
                _ => assert_eq!(p, None, "the lettuce still has none"),
            }
        }
        let mut old: serde_json::Value = serde_json::from_str(&text).unwrap();
        old.as_object_mut().unwrap().remove("crop_picking");
        let back: WorldSave = serde_json::from_value(old).unwrap();
        let mut older = hecs::World::new();
        apply_save_to_world(&mut older, &back);
        assert_eq!(older.query::<&CropInstance>().iter().count(), 2);
        assert_eq!(older.query::<&CropPicking>().iter().count(), 0);
    }

    /// pre-v0.517 save (no `placed_items` field) loads with an empty pool (serde
    /// default) so it then re-seeds from the places spine.
    #[test]
    fn placed_items_persist_and_old_saves_default_empty() {
        let mut save = WorldSave::new_offline("Test", "fibonacci");
        save.placed_items = vec![
            crate::gui::PlacedItem {
                key: "ice_axe_0".into(),
                name: "Ice Axe".into(),
                qty: 1,
                container: "1/0/0".into(),
                wear: 0,
                quality: 0,
            },
            crate::gui::PlacedItem {
                key: "iron_ore_0".into(),
                name: "Iron Ore".into(),
                qty: 5,
                container: "2/0".into(),
                wear: 0,
                quality: 0,
            },
        ];
        let json = serde_json::to_string(&save).expect("serialize");
        let back: WorldSave = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.placed_items.len(), 2);
        assert_eq!(back.placed_items[0].key, "ice_axe_0");
        assert_eq!(back.placed_items[1].qty, 5);
        assert_eq!(back.placed_items[1].container, "2/0");

        // A pre-v0.517 save JSON that lacks the field -> empty pool, no error.
        let old_json = r#"{"name":"Old","timestamp":0,"game_time":0.0,
            "player_position":[0.0,0.0,0.0],"player_rotation":[0.0,0.0,0.0,1.0],
            "player_health":100.0,"inventory":[],"skills":{},"constructions":[],
            "weather_state":"clear"}"#;
        let old: WorldSave = serde_json::from_str(old_json).expect("old save loads");
        assert!(old.placed_items.is_empty(), "old save defaults to an empty pool");
    }

    fn crop(planted_at: f64, stage: &str) -> crate::ecs::components::CropInstance {
        crate::ecs::components::CropInstance {
            crop_def_id: "tomato".to_string(),
            growth_stage: stage.to_string(),
            planted_at,
            water_level: 0.8,
            health: 90.0,
            tower_id: None,
            tower_slot: None,
            health_seconds: 0.0,
            growing_seconds: 0.0,
        }
    }

    /// Offline catch-up ages living crops by exactly the time away, leaves the
    /// dead alone, never touches water or health, and does nothing when off.
    #[test]
    fn offline_catch_up_ages_living_crops_only_when_on() {
        let mut save = WorldSave::new_offline("t", "fibonacci");
        save.game_time = 6000.0;
        save.timestamp = 1_000;
        save.crops = vec![crop(5000.0, "seedling"), crop(4000.0, crate::ecs::components::STAGE_DEAD)];
        let now = 1_000 + 3_600;

        let mut world = hecs::World::new();
        apply_save_to_world(&mut world, &save);
        let r = catch_up_world(&mut world, &save, true, now);
        assert_eq!(r, Resumed { clock: 6000.0, away_secs: 3600.0, crops_aged: 1, builds_advanced: 0, crafts_advanced: 0 });
        let mut got: Vec<(f64, f32, f32)> = world
            .query_mut::<&crate::ecs::components::CropInstance>()
            .into_iter()
            .map(|(_e, c)| (c.planted_at, c.water_level, c.health))
            .collect();
        got.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        assert_eq!(got, vec![(1400.0, 0.8, 90.0), (4000.0, 0.8, 90.0)], "living aged, dead untouched, upkeep untouched");

        let mut world = hecs::World::new();
        apply_save_to_world(&mut world, &save);
        let r = catch_up_world(&mut world, &save, false, now);
        assert_eq!(r, Resumed { clock: 6000.0, away_secs: 0.0, crops_aged: 0, builds_advanced: 0, crafts_advanced: 0 });
    }

    /// A crop cannot have been planted in the future, so the clock never
    /// resumes behind the newest planting (a save from before the clock was
    /// stored carries game_time 0 next to crops planted at game second 5000).
    #[test]
    fn clock_never_resumes_behind_the_newest_crop() {
        let mut save = WorldSave::new_offline("t", "fibonacci");
        save.crops = vec![crop(5000.0, "seedling"), crop(1200.0, "seedling")];
        let mut world = hecs::World::new();
        apply_save_to_world(&mut world, &save);
        assert_eq!(catch_up_world(&mut world, &save, false, 0).clock, 5000.0);
    }

    /// No stamp means no measurable absence; a clock set backwards means zero
    /// growth, never negative.
    #[test]
    fn unstamped_saves_and_backwards_clocks_grant_nothing() {
        let mut save = WorldSave::new_offline("t", "fibonacci");
        save.crops = vec![crop(100.0, "seedling")];
        let mut world = hecs::World::new();
        apply_save_to_world(&mut world, &save);
        assert_eq!(catch_up_world(&mut world, &save, true, 9_999).away_secs, 0.0, "timestamp 0");

        save.timestamp = 5_000;
        let mut world = hecs::World::new();
        apply_save_to_world(&mut world, &save);
        assert_eq!(catch_up_world(&mut world, &save, true, 4_000).away_secs, 0.0, "clock went backwards");
    }

    /// Builds survive a restart (2026-09-25: nothing wrote save.constructions,
    /// so every structure was discarded at exit after its materials were spent).
    /// A finished Structure and a half-built scaffold both round-trip with pose
    /// and state, and re-applying replaces rather than duplicates.
    #[test]
    fn builds_survive_the_save_round_trip() {
        use crate::systems::construction::{Construction, Structure};
        let tf = |x: f32| crate::ecs::components::Transform {
            position: glam::Vec3::new(x, 0.0, 3.0),
            rotation: glam::Quat::from_rotation_y(0.5),
            scale: glam::Vec3::new(2.0, 3.0, 0.2),
        };
        let mut world = hecs::World::new();
        world.spawn((
            tf(1.0),
            Structure {
                blueprint_id: "wooden_wall".to_string(),
                health: 80.0,
                max_health: 100.0,
                provides: Some("shelter".to_string()),
            },
        ));
        world.spawn((
            tf(5.0),
            Construction {
                blueprint_id: "wooden_door".to_string(),
                progress: 4.0,
                build_time: 10.0,
                builder_key: None,
            },
        ));
        let save = extract_world_save(&world);
        assert_eq!(save.constructions.len(), 2);

        let mut fresh = hecs::World::new();
        apply_save_to_world(&mut fresh, &save);
        apply_save_to_world(&mut fresh, &save); // re-apply must not duplicate
        let structures: Vec<(String, f32, f32, Option<String>, glam::Vec3)> = fresh
            .query::<(&Structure, &crate::ecs::components::Transform)>()
            .iter()
            .map(|(_e, (s, t))| (s.blueprint_id.clone(), s.health, s.max_health, s.provides.clone(), t.scale))
            .collect();
        assert_eq!(
            structures,
            vec![("wooden_wall".to_string(), 80.0, 100.0, Some("shelter".to_string()), glam::Vec3::new(2.0, 3.0, 0.2))]
        );
        let scaffolds: Vec<(String, f32, f32, f32)> = fresh
            .query::<(&Construction, &crate::ecs::components::Transform)>()
            .iter()
            .map(|(_e, (c, t))| (c.blueprint_id.clone(), c.progress, c.build_time, t.position.x))
            .collect();
        assert_eq!(scaffolds, vec![("wooden_door".to_string(), 4.0, 10.0, 5.0)]);
        // And through JSON, the way it reaches disk.
        let json = serde_json::to_string(&save).unwrap();
        let back: WorldSave = serde_json::from_str(&json).unwrap();
        assert_eq!(back.constructions, save.constructions);
    }

    /// A scaffold's materials were spent when it started, so time away may
    /// finish it; progress is capped at build_time so the ConstructionSystem's
    /// own tick does the completion (quest event, XP) rather than this pass.
    #[test]
    fn offline_catch_up_finishes_scaffolds_without_skipping_completion() {
        use crate::systems::construction::Construction;
        let mut save = WorldSave::new_offline("t", "fibonacci");
        save.timestamp = 1_000;
        save.constructions = vec![crate::persistence::ConstructionSave {
            blueprint_id: "wooden_door".to_string(),
            position: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
            health: 0.0,
            max_health: 0.0,
            provides: None,
            building: Some((4.0, 10.0)),
        }];
        let mut world = hecs::World::new();
        apply_save_to_world(&mut world, &save);
        let r = catch_up_world(&mut world, &save, true, 1_000 + 3_600);
        assert_eq!(r.builds_advanced, 1);
        let (_e, c) = world.query_mut::<&Construction>().into_iter().next().unwrap();
        assert_eq!(c.progress, 10.0, "capped at build_time, still a Construction for the tick to complete");

        let mut world = hecs::World::new();
        apply_save_to_world(&mut world, &save);
        assert_eq!(catch_up_world(&mut world, &save, false, 1_000 + 3_600).builds_advanced, 0);
        let (_e, c) = world.query_mut::<&Construction>().into_iter().next().unwrap();
        assert_eq!(c.progress, 4.0, "toggle off leaves the scaffold where it was");
    }

    /// Craft batches resume counted down by the time away, floored at zero
    /// (which completes them on the next tick through the normal delivery).
    #[test]
    fn restored_crafts_count_down_by_the_time_away() {
        let mut save = WorldSave::new_offline("t", "fibonacci");
        let batch = |t: f32| crate::systems::crafting::CraftSave {
            recipe_id: "smelt_iron".to_string(),
            time_remaining: t,
            auto: true,
            machine_id: Some("smelter_1".to_string()),
            pad: None,
        };
        save.crafts = vec![batch(7.0), batch(9_000.0)];
        let r: Vec<f32> = restored_crafts(&save, 3_600.0).iter().map(|c| c.time_remaining).collect();
        assert_eq!(r, vec![0.0, 5_400.0]);
        let r: Vec<f32> = restored_crafts(&save, 0.0).iter().map(|c| c.time_remaining).collect();
        assert_eq!(r, vec![7.0, 9_000.0], "no time away, no change");
    }

    /// "Start every session from the default home" (2026-09-25): writing
    /// keeps an existing progress save untouched apart from the character,
    /// and with no save yet writes a character-only save that is marked as
    /// carrying no progress.
    #[test]
    fn identity_only_save_leaves_progress_alone() {
        let mut world = hecs::World::new();
        world.spawn((
            Controllable,
            Inventory::new(8),
            PlayerSkills::new(),
            crate::ecs::components::Name("Astra".to_string()),
            crate::ecs::components::Appearance::default(),
            crate::ecs::components::Outfit::default(),
        ));
        let mut existing = WorldSave::new_offline("t", "fibonacci");
        existing.character_name = "Old Name".into();
        existing.inventory = vec![("wood_plank_0".into(), 40)];
        existing.crops = vec![crop(5.0, "seedling")];
        existing.game_time = 999.0;
        let out = identity_only_save(Some(existing), &world);
        assert_eq!(out.character_name, "Astra", "the character is recorded");
        assert_eq!(out.inventory, vec![("wood_plank_0".to_string(), 40)], "progress untouched");
        assert_eq!(out.crops.len(), 1);
        assert_eq!(out.game_time, 999.0);
        assert!(out.progress_saved);

        let fresh = identity_only_save(None, &world);
        assert_eq!(fresh.character_name, "Astra");
        assert!(!fresh.progress_saved, "a character-only save says so");
        assert!(fresh.inventory.is_empty() && fresh.crops.is_empty());
    }

    /// The shipped starting kit parses and every item in it is a real item,
    /// so a new player never starts with a name that resolves to nothing.
    #[test]
    fn the_starting_kit_is_real_items() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let kit = starting_kit(&dir);
        assert!(kit.iter().any(|(id, _)| id.starts_with("seed_")), "a new player can plant: {kit:?}");
        let items = crate::systems::inventory::ItemRegistry::from_csv(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/items.csv"
        )))
        .unwrap();
        for (id, qty) in &kit {
            assert!(items.items.contains_key(id), "starting item {id} is not in items.csv");
            assert!(*qty > 0);
        }
    }

    #[test]
    fn away_notice_says_how_long_and_how_many() {
        let r = |away_secs, crops_aged| Resumed { clock: 0.0, away_secs, crops_aged, builds_advanced: 0, crafts_advanced: 0 };
        assert_eq!(away_notice(&r(30.0, 5)), None, "under a minute is not worth a word");
        assert_eq!(away_notice(&r(7200.0, 0)), None, "nothing grew");
        assert_eq!(away_notice(&r(600.0, 1)).unwrap(), "While you were away (10 min), 1 plant kept growing.");
        assert_eq!(away_notice(&r(29_520.0, 12)).unwrap(), "While you were away (8 h 12 min), 12 plants kept growing.");
        assert_eq!(away_notice(&r(3.0 * 86_400.0, 2)).unwrap(), "While you were away (3 days), 2 plants kept growing.");
        let both = Resumed { clock: 0.0, away_secs: 600.0, crops_aged: 3, builds_advanced: 1, crafts_advanced: 0 };
        assert_eq!(
            away_notice(&both).unwrap(),
            "While you were away (10 min), 3 plants kept growing and 1 build kept going up."
        );
    }
}
