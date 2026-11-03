//! The helpers other crates' tests share with the food system's wounds and spoilage tests (the
//! engine's and the GUI's crossing tests): apart from the tests so that the crate split can put
//! them behind its `test-support` feature (rung 2, docs/design/crate-split.md: a crate's
//! #[cfg(test)] items never reach another crate's tests). A child of `food`: `super::super::*`
//! below is the food system.

/// From wounds_tests.rs: the shipped wound and fall data, the store as lib.rs wires the food and
/// combat systems, a fed player, and a landing given to a body.
pub mod wounds {
    use super::super::*;
    use humanity_core::ecs::components::{Controllable, Health, StatusEffects, Transform, Vitals};
    use humanity_core::ecs::systems::System;
    use humanity_core::hot_reload::data_store::DataStore;
    use crate::combat::damage::DamageEvent;
    use crate::illness::{Mode, MODE_KEY};
    use crate::inventory::Inventory;
    use crate::status_effects::StatusEffectRegistry;
    use crate::wounds::Wounds;
    use crate::falls::{player_landed, Falls};
    use std::sync::Mutex;

    pub fn data_dir() -> &'static Path {
        Path::new(concat!(env!("HUMANITY_REPO_ROOT"), "/data"))
    }

    pub fn read(rel: &str) -> String {
        std::fs::read_to_string(data_dir().join(rel)).unwrap_or_else(|e| panic!("data/{rel}: {e}"))
    }

    pub fn registry() -> StatusEffectRegistry {
        StatusEffectRegistry::from_csv(read("status_effects.csv").as_bytes()).expect("status_effects.csv")
    }

    /// data/medical/wounds.ron, read here on its own.
    pub fn shipped() -> Wounds {
        Wounds::from_ron(&read("medical/wounds.ron")).expect("wounds.ron parses")
    }

    /// The DataStore as lib.rs wires the food and combat systems, with the needs held still (the
    /// Vitals drain slider at 0) so a long run changes only what a test is about. `mode` is the
    /// Settings > Gameplay > Illness and injury mode (absent, as in a headless test, is Realistic).
    pub fn store(mode: Option<Mode>) -> DataStore {
        let mut data = DataStore::new();
        data.insert("status_effect_registry", registry());
        for slot in ["consume_request", "drink_request", "use_item_request"] {
            data.insert(slot, Mutex::new(Option::<String>::None));
        }
        data.insert("rest_request", Mutex::new(false));
        data.insert("compost_request", Mutex::new(false));
        data.insert("player_death", Mutex::new(Option::<String>::None));
        data.insert("player_notices", Mutex::new(Vec::<String>::new()));
        data.insert("vitals_drain_scale", Mutex::new(0.0_f32));
        data.insert("damage_events", Mutex::new(Vec::<(u64, DamageEvent)>::new()));
        data.insert("loot_drops", Mutex::new(Vec::<(String, u32)>::new()));
        if let Some(mode) = mode {
            data.insert(MODE_KEY, mode);
        }
        data
    }

    /// A fed and watered player standing at the origin.
    pub fn player(world: &mut hecs::World) -> hecs::Entity {
        world.spawn((
            Controllable,
            Inventory::new(36),
            Vitals { satiation: 80.0, hydration: 80.0, ..Vitals::default() },
            StatusEffects::default(),
            Health::default(),
            Transform::default(),
        ))
    }

    pub fn effects(world: &hecs::World, who: hecs::Entity) -> StatusEffects {
        StatusEffects::clone(&world.get::<&StatusEffects>(who).unwrap())
    }

    pub fn ids(fx: &StatusEffects) -> Vec<String> {
        fx.active.iter().map(|e| e.id.clone()).collect()
    }

    pub fn health(world: &hecs::World, who: hecs::Entity) -> f64 {
        f64::from(world.get::<&Health>(who).unwrap().current)
    }

    pub fn notices(data: &DataStore) -> Vec<String> {
        std::mem::take(&mut *data.get::<Mutex<Vec<String>>>("player_notices").unwrap().lock().unwrap())
    }

    /// Tick `steps` steps of `step_s` game seconds.
    pub fn run(sys: &mut FoodSystem, world: &mut hecs::World, data: &DataStore, steps: u32, step_s: f32) {
        for _ in 0..steps {
            sys.tick(world, step_s, data);
        }
    }

    /// The shipped data with every wound that can go bad certain to when nobody cared for it and
    /// never when someone did, so the rule can be seen without dice.
    pub fn certain(sys: &mut FoodSystem) {
        for w in &mut sys.wounds.wounds {
            if let Some(b) = &mut w.goes_bad {
                b.uncared = 1.0;
                b.cared = 0.0;
            }
        }
    }

    /// Earth's gravity as the homestead runs it (data/game.csv `gravity_m_s2`).
    pub const EARTH_G: f32 = 9.81;
    /// The Moon's surface gravity, m/s^2.
    pub const MOON_G: f32 = 1.62;
    /// A frame at 60 a second.
    pub const FRAME_S: f32 = 1.0 / 60.0;
    /// The planet walk's settle rate (`engine::frame_lock::SURFACE_SETTLE_RATE`, native-only).
    pub const SURFACE_SETTLE: f64 = 4.0;

    pub fn shipped_falls() -> Falls {
        Falls::from_ron(&read("medical/falls.ron")).expect("falls.ron parses")
    }

    /// The free-fall speed of a drop of `h` m under `g`: v = sqrt(2 g h).
    pub fn free_fall(h: f32, g: f32) -> f32 {
        (2.0 * g * h).sqrt()
    }

    /// What a landing did to a body.
    pub struct Landed {
        pub fx: StatusEffects,
        pub took: f64,
        pub said: String,
        pub died: Option<String>,
    }

    /// Give `landing` to a fresh, fed player's body the way lib.rs does (`player_landed`) and run
    /// one body pass in `mode`: what it left, the health it took, what the player was told, and the
    /// death screen's words if it killed.
    pub fn land(mode: Mode, landing: Option<f32>) -> Landed {
        let mut sys = FoodSystem::new(data_dir());
        let data = store(Some(mode));
        let mut world = hecs::World::new();
        let p = player(&mut world);
        if let Some(v) = landing {
            player_landed(&mut world, v);
        }
        sys.tick(&mut world, 0.0, &data);
        let died = data.get::<Mutex<Option<String>>>("player_death").unwrap().lock().unwrap().clone();
        Landed { fx: effects(&world, p), took: 100.0 - health(&world, p), said: notices(&data).join(" "), died }
    }
}

/// From food_spoilage_tests.rs: the store, a player, a home vessel and a Barn stack.
pub mod spoilage {
    use super::super::*;
    use humanity_core::ecs::components::{Health, StatusEffects, Vitals};
    use humanity_core::hot_reload::data_store::DataStore;
    use crate::inventory::containers::{Container, ContainerRegistry};
    use crate::inventory::placed::PlacedItem;
    use crate::inventory::Inventory;
    use std::sync::Mutex;

    pub fn data_dir() -> &'static Path {
        Path::new(concat!(env!("HUMANITY_REPO_ROOT"), "/data"))
    }

    /// The DataStore as lib.rs wires it for the FoodSystem, with the needs held still (the Vitals
    /// drain slider at 0), the shipped container types, and the slot home storage's aging rides in.
    pub fn store() -> DataStore {
        let mut data = DataStore::new();
        let reg = crate::status_effects::StatusEffectRegistry::from_csv(include_bytes!(concat!(
            env!("HUMANITY_REPO_ROOT"),
            "/data/status_effects.csv"
        )))
        .expect("status_effects.csv");
        data.insert("status_effect_registry", reg);
        for slot in ["consume_request", "drink_request"] {
            data.insert(slot, Mutex::new(Option::<String>::None));
        }
        data.insert("rest_request", Mutex::new(false));
        data.insert("compost_request", Mutex::new(false));
        data.insert("player_death", Mutex::new(Option::<String>::None));
        data.insert("vitals_drain_scale", Mutex::new(0.0_f32));
        data.insert(STORAGE_AGING_KEY, Mutex::new(0.0_f64));
        let root = data_dir().join("containers");
        let containers = ContainerRegistry::from_bytes(
            &std::fs::read(root.join("types.csv")).unwrap(),
            &std::fs::read(root.join("content_classes.ron")).unwrap(),
        )
        .unwrap();
        data.insert("container_registry", containers);
        data
    }

    pub fn player(world: &mut hecs::World, inv: Inventory) -> hecs::Entity {
        world.spawn((
            humanity_core::ecs::components::Controllable,
            inv,
            Vitals::default(),
            StatusEffects::default(),
            Health::default(),
        ))
    }

    /// A home vessel of the shipped type `container_type`, holding `qty` of `item`.
    pub fn vessel(world: &mut hecs::World, container_type: &str, item: &str, qty: u32) -> hecs::Entity {
        let mut c = Container::new(container_type, 400.0);
        c.current_content_item = Some(item.to_string());
        c.last_content = Some(item.to_string());
        c.current_qty = qty;
        c.used_liters = qty as f32;
        world.spawn((c,))
    }

    pub fn barn(key: &str, qty: u32) -> PlacedItem {
        PlacedItem { key: key.into(), name: key.into(), qty, container: "1/1".into(), ..Default::default() }
    }

    pub fn age_of(world: &hecs::World, p: hecs::Entity, item: &str) -> f64 {
        world.get::<&Inventory>(p).unwrap().slots.iter().flatten().find(|s| s.item_id == item).map(|s| s.age_s).unwrap()
    }
}
