//! The terrain patches and the reuse switch (2026-11-13, part B): `planet_chunks::reuse_off_rebuilds`,
//! `selection_kept` and `ChunkState::take_for_rebuild`.

use super::*;

/// THE REUSE SWITCH on the terrain's patches (2026-11-13, part B): with reuse on nothing is
/// built again and the parked selection is kept; with reuse off the walk runs every frame and
/// the drawn patches built longest ago are built again, oldest first, a few a frame, so every
/// drawn patch takes its turn. Seen red with `reuse_off_rebuilds` ignoring `reuse`:
/// "reuse on: nothing built again".
#[test]
fn with_reuse_off_the_drawn_patches_built_longest_ago_are_built_again() {
    let ids: Vec<PatchId> = (0..6u8).map(PatchId::root).collect();
    // Built at frames 50, 10, 40, 20, 60, 30.
    let drawn: Vec<(PatchId, u64)> = ids.iter().zip([50u64, 10, 40, 20, 60, 30]).map(|(id, at)| (*id, at)).collect();
    assert!(reuse_off_rebuilds(drawn.clone(), true, 3).is_empty(), "reuse on: nothing built again");
    assert_eq!(reuse_off_rebuilds(drawn.clone(), false, 3), vec![ids[1], ids[3], ids[5]], "reuse off: the three oldest, oldest first");
    assert_eq!(reuse_off_rebuilds(drawn.clone(), false, 10).len(), 6, "fewer drawn than the share: all of them");
    // Every drawn patch takes its turn: rebuilding stamps it newest.
    let mut book: HashMap<PatchId, u64> = drawn.iter().cloned().collect();
    let mut seen = std::collections::HashSet::new();
    for frame in 100..102u64 {
        for id in reuse_off_rebuilds(book.iter().map(|(i, a)| (*i, *a)), false, 3) {
            book.insert(id, frame);
            seen.insert(id);
        }
    }
    assert_eq!(seen.len(), 6, "two frames of three: every patch built again once");
    assert!(selection_kept(true, true), "reuse on, parked: the selection is kept");
    assert!(!selection_kept(false, true), "reuse off: walked again every frame");
    assert!(!selection_kept(true, false), "not parked: walked again");
    // New builds first: no rebuilds while a descent still wants patches (seen red 2026-11-14 with
    // the share fixed at TERRAIN_REUSE_OFF_REBUILDS: "new builds wanted: no rebuilds", left 8 right 0).
    assert_eq!(reuse_off_rebuild_share(0), TERRAIN_REUSE_OFF_REBUILDS, "nothing new: the share");
    assert_eq!(reuse_off_rebuild_share(3), 0, "new builds wanted: no rebuilds");
}

/// A patch taken out to be built again leaves the byte count, and the rebuilt one comes back in
/// stamped with the frame it was built.
#[test]
fn a_patch_built_again_in_place_keeps_the_byte_count_honest() {
    let mut cs = ChunkState::new(7);
    let id = PatchId::root(2);
    let band = RadialBand { min_r_m: 1.0, max_r_m: 2.0 };
    cs.frame = 5;
    cs.insert_built(id, 3, None, 1000, DVec3::ZERO, band, 0.0);
    assert_eq!(cs.cache[&id].built_frame, 5);
    assert_eq!(cs.take_for_rebuild(&id), Some((3, None)));
    assert_eq!(cs.total_bytes, 0);
    cs.frame = 9;
    cs.insert_built(id, 4, None, 1200, DVec3::ZERO, band, 0.0);
    assert_eq!((cs.total_bytes, cs.cache[&id].built_frame), (1200, 9));
    assert_eq!(cs.take_for_rebuild(&PatchId::root(3)), None, "a patch not built is not taken");
}
