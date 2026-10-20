//! What the dev IPC reports of the player's body (2026-10-20): health, the wounds on it, and the
//! landings the walk handed to it. Permanent dev tooling.
//!
//! WHY IT EXISTS. Falls (src/systems/falls.rs) shipped with unit tests and nothing that looked at
//! the running game: `player_landed` wrote no log line and no probe said what health or wounds
//! the player had, so a rig could not see a fall reach the body at all. This is what
//! scripts/verify-falls.js reads (judged by scripts/lib/falls-judge.js):
//!   - the home session's `vitals` (src/engine/ipc.rs `home_session_core`) carries `health`,
//!     `health_max` and `wounds` beside the satiation and hydration it always had;
//!   - every recorder frame carries `health`, so a rig sees WHEN health fell against the camera's
//!     drop on the same frame clock;
//!   - the recorder's done JSON carries `falls`: how many landings lib.rs took from the walk and
//!     handed to the body (`CameraController::take_landing`), the last one's speed, the Illness
//!     and injury mode, the walk's gravity, and whether dev flight was on;
//!   - the showcase `illness` verb sets Settings > Gameplay > Illness and injury, as its radio does.
//!
//! The ECS is read, not the HUD's copy: the HUD can lag a frame or be fed by another path, and
//! the question a rig asks is what the body itself holds.

use crate::ecs::components::{Controllable, Dead, Health, StatusEffects};
use crate::systems::status_effects::StatusEffectRegistry;

/// The player's health now: the one Controllable with a Health that is not dead. None with no
/// living player (before the world loads, on the death screen).
pub(crate) fn health(world: &hecs::World) -> Option<(f32, f32)> {
    world
        .query::<(&Controllable, &Health, Option<&Dead>)>()
        .iter()
        .find(|(_, (_, _, dead))| dead.is_none())
        .map(|(_, (_, h, _))| (h.current, h.max))
}

/// The wounds on the player now (`wound` rows of status_effects.csv, src/systems/wounds.rs): each
/// one's id, the name the game shows for it and the seconds it has left (game seconds, as a
/// wound counts them). Without a registry nothing can be told to be a wound, so the list is
/// empty and `registry` in `body_json` says so.
pub(crate) fn wounds(world: &hecs::World, registry: Option<&StatusEffectRegistry>) -> Vec<serde_json::Value> {
    let Some(reg) = registry else { return Vec::new() };
    let mut q = world.query::<(&Controllable, &StatusEffects, Option<&Dead>)>();
    let Some((_, (_, effects, _))) = q.iter().find(|(_, (_, _, dead))| dead.is_none()) else {
        return Vec::new();
    };
    effects
        .active
        .iter()
        .filter_map(|a| {
            let def = reg.get(&a.id)?;
            (def.kind == crate::systems::wounds::WOUND).then(|| {
                serde_json::json!({ "id": a.id, "name": def.name, "remaining_s": a.remaining })
            })
        })
        .collect()
}

/// The home session's `vitals`: the satiation and hydration it has always carried (the HUD's,
/// unchanged), and the body's health and wounds from the ECS.
pub(crate) fn vitals_json(
    satiation: f32,
    hydration: f32,
    world: &hecs::World,
    data: &crate::hot_reload::data_store::DataStore,
) -> serde_json::Value {
    let registry = data.get::<StatusEffectRegistry>("status_effect_registry");
    let hp = health(world);
    serde_json::json!({
        "satiation": satiation,
        "hydration": hydration,
        "health": hp.map(|h| h.0),
        "health_max": hp.map(|h| h.1),
        "wounds": wounds(world, registry),
        // False when no status effect registry loaded: then `wounds` is empty because nothing
        // could be named a wound, not because there are none.
        "registry": registry.is_some(),
    })
}

/// The recorder's `falls` block (pure): `landings` is `CameraController::landings_taken`.
pub(crate) fn falls_core(landings: (u32, Option<f32>), realistic: bool, gravity_m_s2: f32, flying: bool, aboard: bool) -> serde_json::Value {
    serde_json::json!({
        "landings": landings.0,
        "last_m_s": landings.1,
        "mode": if realistic { "realistic" } else { "forgiving" },
        "gravity_m_s2": gravity_m_s2,
        "flying": flying,
        "aboard": aboard,
    })
}

/// The recorder's `falls` block for the running game (see the module doc).
pub(crate) fn falls_json(state: &crate::engine::state::EngineState) -> serde_json::Value {
    let flying = state.controller.fly_mode || state.gui_state.dev_fly_mode || state.gui_state.dev_hover;
    falls_core(
        state.controller.landings_taken(),
        state.gui_state.settings.illness_realistic,
        state.controller.interior_gravity(),
        flying,
        state.aboard_station,
    )
}

/// Read a showcase `illness` value: `realistic` or `forgiving` (Settings > Gameplay > Illness and
/// injury), any case, spaces allowed. None for anything else, so a typo changes nothing.
pub(crate) fn parse_illness_mode(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "realistic" => Some(true),
        "forgiving" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> StatusEffectRegistry {
        StatusEffectRegistry::from_csv(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/status_effects.csv")))
            .expect("status_effects.csv")
    }

    /// THE PROBE SAYS WHAT THE BODY HOLDS. A player at 90.3 of 100 with a sprained ankle and a
    /// well-fed buff: health and its max as the ECS has them, and the ankle (by its id and the
    /// name the game shows) as the one wound, the buff left out. A dead body and no player report
    /// no health. Seen red 2026-10-20 with `wounds` filtering on `kind == "buff"`: the list held
    /// Well Fed and not the ankle.
    #[test]
    fn the_probe_reports_health_and_the_wounds_by_name() {
        let reg = registry();
        let mut world = hecs::World::new();
        assert_eq!(health(&world), None, "no player, no health");
        let mut effects = StatusEffects::default();
        effects.apply("sprained_ankle", 1_209_600.0);
        effects.apply("well_fed", 600.0);
        let me = world.spawn((Controllable, Health { current: 90.3, max: 100.0 }, effects));
        assert_eq!(health(&world), Some((90.3, 100.0)));
        let w = wounds(&world, Some(&reg));
        assert_eq!(w.len(), 1, "{w:?}");
        assert_eq!(w[0]["id"], "sprained_ankle");
        assert_eq!(w[0]["name"], reg.get("sprained_ankle").unwrap().name.as_str());
        assert!(wounds(&world, None).is_empty(), "no registry: nothing can be named a wound");

        let mut data = crate::hot_reload::data_store::DataStore::new();
        data.insert("status_effect_registry", reg);
        let v = vitals_json(70.0, 80.0, &world, &data);
        assert_eq!((v["satiation"].as_f64(), v["hydration"].as_f64()), (Some(70.0), Some(80.0)), "the old fields unchanged");
        assert!((v["health"].as_f64().unwrap() - 90.3).abs() < 1e-4 && v["health_max"] == 100.0, "{v}");
        assert_eq!((v["wounds"][0]["id"].as_str(), v["registry"].as_bool()), (Some("sprained_ankle"), Some(true)));

        world.insert_one(me, Dead::default()).unwrap();
        assert_eq!(health(&world), None, "a dead body reports no health");
        assert!(wounds(&world, Some(&registry())).is_empty());
    }

    /// THE PROBE IS WIRED INTO THE DEV IPC: the home session's vitals come from here, every
    /// recorder frame carries the health, its done JSON the `falls` block, and the showcase poll
    /// reads the `illness` verb. Without these the functions above are tested and never run.
    #[test]
    fn the_probe_is_wired_into_the_dev_ipc() {
        let src = include_str!("ipc.rs").replace("\r\n", "\n");
        for wired in [
            "\"vitals\": crate::engine::body_probe::vitals_json(gs.vitals.satiation, gs.vitals.hydration, world, data)",
            "\"health\": crate::engine::body_probe::health(&state.game_world.world)",
            "\"falls\": crate::engine::body_probe::falls_json(state)",
            "grab(\"illness\")",
            "crate::engine::body_probe::parse_illness_mode(&raw)",
        ] {
            assert!(src.contains(wired), "src/engine/ipc.rs never has {wired}");
        }
    }

    /// The `falls` block and the `illness` verb's words.
    #[test]
    fn the_falls_block_and_the_illness_words() {
        let v = falls_core((2, Some(7.67)), true, 9.81, false, true);
        assert_eq!((v["landings"].as_u64(), v["mode"].as_str(), v["aboard"].as_bool()), (Some(2), Some("realistic"), Some(true)));
        assert!((v["last_m_s"].as_f64().unwrap() - 7.67).abs() < 1e-4);
        assert_eq!(falls_core((0, None), false, 9.81, true, false)["last_m_s"], serde_json::Value::Null);
        assert_eq!(falls_core((0, None), false, 9.81, true, false)["mode"], "forgiving");
        assert_eq!(parse_illness_mode(" Realistic "), Some(true));
        assert_eq!(parse_illness_mode("forgiving"), Some(false));
        for bad in ["", "real", "1", "on"] {
            assert_eq!(parse_illness_mode(bad), None, "{bad:?}");
        }
    }
}
