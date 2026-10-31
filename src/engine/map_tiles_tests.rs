//! The plan tiles' tests (Maps increment 3). Each was seen red first; what broke it and what it
//! said is in its comment.

use super::*;
use crate::gui::pages::maps::tile_grid::keys_covering;
use crate::ship::levels::{level_cuts, ship_levels};

fn shipped() -> ShipStructure {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    ShipStructure::load_and_assemble_shipped(&data, Some("p1")).expect("the shipped ship assembles at p1")
}

fn cuts_at(ship: &ShipStructure, level: &ShipLevel) -> Cuts {
    Cuts(level_cuts(ship, level).into_iter().map(|c| crate::ship::storey_edits::CutAway { min: c.min, max: c.max, cut_y: c.cut_y }).collect())
}

/// AN EDIT DROPS EXACTLY THE TILES IT TOUCHED (the design's proof for plan tiles). On the shipped
/// ship's ground level, with every 64 m tile over the ship baked: a deck added through the build
/// editor's console (`console_this_session`, what the rig's `console` verb runs) in the home at
/// (32.5, 62) drops the tiles under its footprint and no other; a deck added UPSTAIRS (3.2 m up,
/// over the cut) drops nothing on the ground level, which does not show it; a rebuild that
/// changed nothing drops nothing.
///
/// Seen red 2026-10-22 twice: with every rebuild marking every item's footprint (`dirty_rects`
/// counting old and new together), "a rebuild that changed nothing marked a footprint"; and with
/// the items made from the uncut home, "a deck upstairs dropped 2 ground-level
/// tiles".
#[test]
fn an_edit_drops_exactly_the_tiles_it_touched() {
    let ship = shipped();
    let ground = ship_levels(&ship)[0];
    let cuts = cuts_at(&ship, &ground);
    let (lo, hi) = ship.world_bounds();
    let baked = keys_covering(ground.ordinal, 3, [lo.x, lo.z, hi.x, hi.z]);
    let mut gui = crate::gui::GuiState::default();
    gui.construction_zone = ship.home_zone_index();
    gui.ship_structure = Some(ship);
    let items = |g: &crate::gui::GuiState| plan_items(g.ship_structure.as_ref().unwrap(), &ground, &[], &cuts);
    let before = items(&gui);
    assert!(dirty_rects(&before, &items(&gui)).is_empty(), "a rebuild that changed nothing marked a footprint");

    let (said, _) = crate::engine::ipc::console_this_session(&mut gui, "add_structure deck 32.5 0 62 0");
    assert!(said.starts_with("added"), "the console refused the deck: {said}");
    let after = items(&gui);
    let dropped: Vec<(i32, i32)> = touched(baked.iter().copied(), ground.ordinal, &dirty_rects(&before, &after)).iter().map(|k| (k.x, k.y)).collect();
    let under: Vec<(i32, i32)> = baked.iter().filter(|k| rects_touch(k.rect(), [29.5, 59.0, 35.5, 65.0])).map(|k| (k.x, k.y)).collect();
    assert!(!under.is_empty());
    assert_eq!(dropped, under, "the deck dropped {} tiles, want {under:?}", dropped.len());

    let (said, _) = crate::engine::ipc::console_this_session(&mut gui, "add_structure deck 32.5 3.2 62 0");
    assert!(said.starts_with("added"), "the console refused the upstairs deck: {said}");
    let upstairs = items(&gui);
    let dropped = touched(baked.iter().copied(), ground.ordinal, &dirty_rects(&after, &upstairs));
    assert!(dropped.is_empty(), "a deck upstairs dropped {} ground-level tiles", dropped.len());
}

/// Past the 64 MB cap the least recently used tiles go first, and never one used this frame.
///
/// Seen red 2026-10-22 with the most recently used let go: "evicted [68, 67, 66, 65, 64, 63],
/// want the six used longest ago" (the newest, 69, was used this frame and kept).
#[test]
fn the_least_recently_used_tiles_go_first() {
    let used: Vec<(TileKey, u64)> = (0..70).map(|i| (TileKey { level: 0, z: 2, x: 0, y: i }, 100 + i as u64)).collect();
    let gone = to_evict(&used, 169, CACHE_BYTES);
    let ys: Vec<i32> = gone.iter().map(|k| k.y).collect();
    assert_eq!(ys, vec![0, 1, 2, 3, 4, 5], "evicted {ys:?}, want the six used longest ago");
    let all_now: Vec<(TileKey, u64)> = used.iter().map(|(k, _)| (*k, 7)).collect();
    assert!(to_evict(&all_now, 7, CACHE_BYTES).is_empty(), "a tile used this frame was let go");
}
