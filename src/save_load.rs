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
    // Respawn deployed vehicles (economy Phase 2 Stage 1, v0.677). Idempotence
    // guard: this fn is documented as called once at startup, but if a vehicle
    // with the same pose already exists (e.g. a future second call), don't stack
    // a duplicate on top of it.
    for vs in &save.deployed_vehicles {
        let pos = glam::Vec3::from_array(vs.position);
        let already_there = world
            .query_mut::<(
                &crate::ecs::components::Vehicle,
                &crate::ecs::components::Transform,
            )>()
            .into_iter()
            .any(|(_e, (v, t))| {
                v.item_id == vs.item_id && (t.position - pos).length_squared() < 0.01
            });
        if already_there {
            continue;
        }
        // Same tuple VehicleSystem::handle_deploy spawns, minus Name (the display
        // name lives in the kit registry, which this fn deliberately has no access
        // to; nothing reads a vehicle's Name yet — revisit when nameplates land).
        world.spawn((
            crate::ecs::components::Vehicle { item_id: vs.item_id.clone() },
            crate::ecs::components::Transform {
                position: pos,
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
}

/// Extract + write the active offline home to disk. Logs on failure. `placed` is the
/// organize-layer container pool (GuiState-owned, not in the ECS world), persisted
/// alongside the world-derived save so container contents + transfers survive a restart.
pub fn save_active_home(world: &hecs::World, placed: &[crate::gui::PlacedItem]) {
    let mut save = extract_world_save(world);
    save.placed_items = placed.to_vec();
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
        save_active_home(world, placed);
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

        // And an old save without the field loads with none (serde default).
        let old_json = r#"{"name":"Old","timestamp":0,"game_time":0.0,
            "player_position":[0.0,0.0,0.0],"player_rotation":[0.0,0.0,0.0,1.0],
            "player_health":100.0,"inventory":[],"skills":{},"constructions":[],
            "weather_state":"clear"}"#;
        let old: WorldSave = serde_json::from_str(old_json).expect("old save loads");
        assert!(old.deployed_vehicles.is_empty());
    }

    /// Organize-layer container contents survive a save serde round-trip, and a
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
            },
            crate::gui::PlacedItem {
                key: "iron_ore_0".into(),
                name: "Iron Ore".into(),
                qty: 5,
                container: "2/0".into(),
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
}
