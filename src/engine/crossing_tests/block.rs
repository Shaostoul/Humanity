//! From engine/block.rs's tests (the GUI's blocking): a blocked player has no figure, name or
//! place in the HUD's list, read through the engine's routing of the relay's frames (`net_route`).

use crate::engine::block::tests::{app, identity, tidy};
use crate::engine::block::*;

/// IN THE GAME (section 4.5, "Game figure and name"): the relay's snapshot entry and join
/// carry each player's key, and someone we blocked has no name over them and no figure drawn
/// (both places read `hides_player`); anyone else, and a player whose key has not come yet,
/// is drawn. Seen red 2026-10-10 two ways: `hides_player` answering false, "no name over the
/// blocked player"; and the figure pass in lib.rs without its check, "the figure pass asks it".
/// 10m R9, the HUD's co-presence list too: seen red 2026-10-10 with `copresence_names`
/// ignoring `hidden`, "the blocked player is not in the HUD's list" (Cy listed); and with
/// lib.rs handing it `&|_| false`, the count assertion (left 2, right 3).
#[test]
fn a_blocked_players_figure_and_name_are_not_drawn() {
    use crate::net::protocol::NetMessage;
    let (seed, me) = identity(148);
    let mut gs = app(&me, &seed, "block-game");
    block(&mut gs, "cy");

    let entry = serde_json::json!({ "entity_id": 7, "entity_type": "player", "position": [1.0, 1.7, 0.0], "rotation": [0.0, 0.0, 0.0, 1.0], "components": { "name": "Cy", "key": "cy" } });
    let keys: Vec<Option<String>> = crate::engine::net_route::snapshot_entry_messages(&entry, None)
        .into_iter()
        .filter_map(|m| match m { NetMessage::PlayerJoined { key, .. } => Some(key), _ => None })
        .collect();
    assert_eq!(keys, vec![Some("cy".to_string())], "a snapshot entry's key reaches the player record");

    let mut world = hecs::World::new();
    for (id, name, key) in [(7u32, "Cy", Some("cy")), (8, "Dee", Some("dee")), (9, "Player 9", None)] {
        let at = glam::Vec3::new(id as f32, 1.7, 0.0);
        world.spawn((
            humanity_core::ecs::components::Transform { position: at, rotation: glam::Quat::IDENTITY, scale: glam::Vec3::ONE },
            crate::net::sync::RemotePlayer {
                player_id: id,
                name: name.into(),
                look: None,
                key: key.map(str::to_string),
                last_position: at,
                target_position: at,
                last_rotation: glam::Quat::IDENTITY,
                target_rotation: glam::Quat::IDENTITY,
                velocity: glam::Vec3::ZERO,
                interpolation_t: 1.0,
                last_update_time: 0.0,
                posture: Default::default(),
                doing: Default::default(),
            },
        ));
    }
    let labels = crate::engine::net_route::nameplate_labels(&world, glam::Vec3::ZERO, &|p| hides_player(&gs, p));
    let mut names: Vec<String> = labels.into_iter().map(|l| l.name).collect();
    names.sort();
    assert!(!names.contains(&"Cy".to_string()), "no name over the blocked player: {names:?}");
    assert_eq!(names, vec!["Dee".to_string(), "Player 9".to_string()], "everyone else is named, a player with no key yet too");

    // 10m R9: nor in the HUD's "N here: ..." list, which counts every other player.
    let here = crate::engine::net_route::copresence_names(&world, &|p| hides_player(&gs, p));
    assert_eq!(here, vec!["Dee".to_string(), "Player 9".to_string()], "the blocked player is not in the HUD's list");

    let lib = std::fs::read_to_string(concat!(env!("HUMANITY_REPO_ROOT"), "/src/lib.rs")).unwrap();
    assert_eq!(
        lib.matches("crate::engine::block::hides_player(&state.gui_state").count(),
        3,
        "the figure pass asks it, and so do the nameplates and the HUD's co-presence list"
    );
    tidy(&gs);
}
