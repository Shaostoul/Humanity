//! The plan tiles' tests (Maps increment 3). Each was seen red first; what broke it and what it
//! said is in its comment.

use super::*;
use crate::ship::levels::{level_cuts, ship_levels};

fn shipped() -> ShipStructure {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    ShipStructure::load_and_assemble_shipped(&data, Some("p1")).expect("the shipped ship assembles at p1")
}

fn cuts_at(ship: &ShipStructure, level: &ShipLevel) -> Cuts {
    Cuts(level_cuts(ship, level).into_iter().map(|c| crate::engine::editor::CutAway { min: c.min, max: c.max, cut_y: c.cut_y }).collect())
}

/// A tile knows its footprint (zoom 0 is 512 m, each zoom halves it), the tile one zoom coarser
/// that holds it, and where it sits in that tile's image.
///
/// Seen red 2026-10-22 with the parent found by plain division (`x / 2`), which rounds a
/// negative x toward zero: "tile (-1, -3) at zoom 2 has parent (0, -1), want (-1, -2)".
#[test]
fn a_tile_knows_its_footprint_and_its_parent() {
    assert_eq!(TileKey::size_m(0), 512.0);
    assert_eq!(TileKey::size_m(6), 8.0);
    let k = TileKey { level: 1, z: 2, x: 3, y: -2 };
    assert_eq!(k.rect(), [384.0, -256.0, 512.0, -128.0]);
    let p = TileKey { level: 1, z: 2, x: -1, y: -3 }.parent().unwrap();
    assert_eq!((p.x, p.y), (-1, -2), "tile (-1, -3) at zoom 2 has parent ({}, {}), want (-1, -2)", p.x, p.y);
    let parent = k.parent().unwrap();
    assert_eq!((parent.z, parent.x, parent.y), (1, 1, -1));
    assert_eq!(k.uv_in(&parent), [0.5, 0.0, 1.0, 0.5]);
    assert_eq!(TileKey { level: 0, z: 0, x: 0, y: 0 }.parent(), None);
}

/// The zoom keeps a tile's pixel no bigger than a screen pixel: 1 m a pixel is zoom 0, half a
/// metre zoom 1, 0.3 m zoom 2 (a 0.25 m tile pixel), never past the finest or under the
/// coarsest; and the tiles cover the view.
///
/// Seen red 2026-10-22 with the zoom rounded down: "0.3 m a screen pixel takes zoom 1, whose
/// 0.50 m pixels are bigger".
#[test]
fn the_zoom_keeps_a_tile_pixel_no_bigger_than_a_screen_pixel() {
    for (m, want) in [(1.0, 0u8), (0.5, 1), (0.3, 2), (5.0, 0), (0.001, MAX_ZOOM)] {
        let z = zoom_for(m);
        let px = TileKey::size_m(z) as f64 / TILE_PX as f64;
        assert_eq!(z, want, "{m} m a screen pixel takes zoom {z}, whose {px:.2} m pixels are bigger");
    }
    let keys = keys_covering(0, 3, [10.0, -5.0, 140.0, 60.0]);
    let xs: Vec<i32> = keys.iter().map(|k| k.x).collect();
    assert_eq!((xs.iter().min(), xs.iter().max()), (Some(&0), Some(&2)));
    assert_eq!(keys.len(), 3 * 2, "64 m tiles over 10..140 by -5..60: three across, two down");
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
