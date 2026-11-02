//! From systems/wounds_tests.rs: wounds and their care come back with a save, saved and loaded by
//! the engine (`save_load`).

use crate::systems::food::*;
use humanity_core::ecs::components::StatusEffects;
use crate::systems::food::wounds_tests::certain;
use crate::systems::food::wounds_tests::data_dir;
use crate::systems::food::wounds_tests::effects;
use crate::systems::food::wounds_tests::player;
use crate::systems::food::wounds_tests::registry;
use crate::systems::food::wounds_tests::run;
use crate::systems::food::wounds_tests::store;
use crate::systems::illness::Mode;

/// WOUNDS COME BACK WITH A SAVE. They are status effects, so the body's save (`BodySave`)
/// carries each with the time it has left, the care with them; a load puts them back, and the
/// cared-for bite, its day about to pass, still does not go bad after the load.
///
/// Seen red with `restore_body` leaving out the saved effects: "after the load: bleeding None,
/// want Some(300.0)"; and, with the care ignored when the dice are rolled, "the saved care
/// still counts after the load".
#[test]
fn wounds_and_their_care_come_back_with_a_save() {
    use crate::save_load::{apply_save_to_world, extract_world_save};
    // A player as lib.rs spawns one, for the save.
    fn saved_player(world: &mut hecs::World) -> hecs::Entity {
        let p = player(world);
        world
            .insert(
                p,
                (
                    crate::systems::skills::PlayerSkills::new(),
                    humanity_core::ecs::components::Name("Astra".into()),
                    humanity_core::ecs::components::Appearance::default(),
                    humanity_core::ecs::components::Outfit::default(),
                ),
            )
            .unwrap();
        p
    }
    // The bite 23 hours old, an hour before it can go bad.
    let bite_left = registry().duration("bite_wound") as f64 - 23.0 * 3600.0;
    let mut world = hecs::World::new();
    let p = saved_player(&mut world);
    {
        let mut fx = world.get::<&mut StatusEffects>(p).unwrap();
        fx.apply("bleeding", 300.0);
        fx.apply("bite_wound", bite_left);
        fx.apply("burn_partial", 1_000_000.0);
        fx.apply("wound_care", 1_000_000.0);
    }
    let save: crate::persistence::WorldSave =
        serde_json::from_str(&serde_json::to_string(&extract_world_save(&world)).unwrap()).unwrap();
    let mut fresh = hecs::World::new();
    let q = saved_player(&mut fresh);
    apply_save_to_world(&mut fresh, &save);
    let fx = effects(&fresh, q);
    for (id, want) in [("bleeding", 300.0), ("bite_wound", bite_left), ("burn_partial", 1_000_000.0), ("wound_care", 1_000_000.0)] {
        assert_eq!(fx.remaining(id), Some(want), "after the load: {id} {:?}, want Some({want:?})", fx.remaining(id));
    }
    // Its day passes in the next hour: cared for, it does not go bad.
    let mut sys = FoodSystem::new(data_dir());
    certain(&mut sys);
    let data = store(Some(Mode::Realistic));
    run(&mut sys, &mut fresh, &data, 3, 3600.0);
    assert!(!effects(&fresh, q).has("infected_wound"), "the saved care still counts after the load");
}
