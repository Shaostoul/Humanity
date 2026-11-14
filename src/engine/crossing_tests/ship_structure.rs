//! From ship/ship_structure.rs (`storey_fields_tests`): an older game is refused as another ship, by
//! the engine's welcome check (`home_plot::plan_welcome`).

use humanity_ship::ship_structure::*;
use glam::Vec3;

/// AN OLDER GAME IS REFUSED AS ANOTHER SHIP (ship homes increment 7): growing the homestead
/// plots changed the ship's hash, and the game's welcome check (engine/home_plot.rs
/// `plan_welcome`) refuses any relay whose hash is not its own, in the one plain sentence,
/// without giving up a plot. Both ways round: a game built before the plots grew (its ship is
/// this one with 3 m homestead plots, whose hash is the Wave 0 pin, the hash of the ship file
/// before increment 7) meeting a relay running this file, and this game meeting an older relay.
/// The same ship is never refused as another.
///
/// Proven green 2026-10-20 (the existing behaviour, held with the new hash): the refusal does
/// not depend on which way the hash moved.
#[cfg(feature = "native")]
#[test]
fn an_older_game_is_refused_as_another_ship() {
    use crate::engine::home_plot::{plan_welcome, WelcomeContext, WelcomeHome};
    let data = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data");
    let now = ShipStructure::load_and_assemble_shipped(&data, None).expect("the shipped ship assembles");
    let mut older = now.clone();
    for p in older.plots.iter_mut().filter(|p| p.kind == "homestead") {
        p.size.1 = 3.0;
    }
    // The room program's plots and the residential district grown to hold them came later
    // (2026-11-13): an older game's ship has neither.
    older.plots.retain(|p| p.kind == "homestead" || p.kind == "cabin");
    if let Some(d) = older.districts.iter_mut().find(|d| d.id == "res-1") {
        d.size.0 = 100.0;
    }
    assert_eq!(older.ship_hash(), "effbdfbec3360115", "the older game's ship is the ship file before increment 7");
    let welcome = |hash: &str| serde_json::json!({ "type": "game_welcome", "player_id": 7, "world_snapshot": [], "home_plot": null, "ship": { "id": now.id, "hash": hash }, "rejoin": false });
    let ctx = WelcomeContext { arrived_on: None, server: "http://127.0.0.1:3210", camera: Vec3::new(53.5, 1.7, 40.5), data_dir: None, other_kind: None };
    for (what, game, relay) in [("an older game, this relay", &older, now.ship_hash()), ("this game, an older relay", &now, older.ship_hash())] {
        match plan_welcome(Some(game), &welcome(&relay), &ctx) {
            WelcomeHome::Refuse { sentence, give_up_plot } => {
                assert_eq!(sentence, OTHER_SHIP_SENTENCE, "{what}");
                assert!(!give_up_plot, "{what}: a plot that is not ours to give");
            }
            other => panic!("{what}: another ship must be refused: {other:?}"),
        }
    }
    assert!(!matches!(plan_welcome(Some(&now), &welcome(&now.ship_hash()), &ctx), WelcomeHome::Refuse { .. }), "the same ship is not refused");
}
