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
//! homes-as-profiles.md): health, position, game_time (TimeSystem owns its own
//! clock), vitals, crops, quests. So on reload you wake rested at home with your
//! inventory + skills intact.

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
    for (_e, (inv, skills, appearance, outfit, _ctrl)) in world
        .query::<(
            &Inventory,
            &PlayerSkills,
            &crate::ecs::components::Appearance,
            &crate::ecs::components::Outfit,
            &Controllable,
        )>()
        .iter()
    {
        // One (item_id, qty) per occupied slot; apply re-stacks via add_item.
        save.inventory = inv
            .slots
            .iter()
            .filter_map(|s| s.as_ref().map(|st| (st.item_id.clone(), st.quantity)))
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
    save
}

/// Apply a loaded WorldSave's inventory + skills onto the live player entity.
/// Other state (health/position/vitals/crops/quests) is left fresh -- not yet
/// persisted. Idempotent; called once at startup.
pub fn apply_save_to_world(world: &mut hecs::World, save: &WorldSave) {
    // Only offline homes are supported today.
    if save.kind != "offline" {
        return;
    }
    for (_e, (inv, skills, appearance, outfit, _ctrl)) in world.query_mut::<(
        &mut Inventory,
        &mut PlayerSkills,
        &mut crate::ecs::components::Appearance,
        &mut crate::ecs::components::Outfit,
        &Controllable,
    )>() {
        // Rebuild inventory: clear every slot, then add_item re-stacks.
        for slot in inv.slots.iter_mut() {
            *slot = None;
        }
        for (item_id, qty) in &save.inventory {
            inv.add_item(item_id, *qty, 99);
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
}

/// Extract + write the active offline home to disk. Logs on failure.
pub fn save_active_home(world: &hecs::World) {
    let save = extract_world_save(world);
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
pub fn maybe_periodic_save(world: &hecs::World, interval_secs: u64) {
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
        save_active_home(world);
    }
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
        world.spawn((Controllable, inv, skills, appearance, outfit));

        let save = extract_world_save(&world);
        assert!(save.inventory.iter().any(|(id, q)| id == "wood_plank_0" && *q == 40));
        assert_eq!(save.skills.get("farming").copied(), Some((3, 450)));
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
}
