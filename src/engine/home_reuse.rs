//! THE HOME'S REUSES AND THE REUSE SWITCH (2026-11-13; CLAUDE.md, the reuse rule;
//! `humanity_core::reuse`).
//!
//! The home is built once and kept: its shell, its machines and their pipes are rebuilt when an
//! edit raises a dirty flag (lib.rs, the construction flags), the garden when a crop's look moves
//! (`home_meshes::rebuild_plant_meshes`), the stock piles when the stored volume changes
//! (`stock_piles`). Each of those is a reuse, and with the switch off (the Dev page's "Turn off all
//! reuse", `HUMANITY_NO_REUSE=1`) each is worked out again. Not every frame for the two big ones,
//! because that is far over a frame:
//!
//! * the home: an edit's rebuild is 23 to 100 ms (stutter survey 2, 2026-10-31), and this one,
//!   which also draws every machine body and model afresh, measured 301 to 637 ms, median 526 ms,
//!   of which the machines 283 to 594 ms (19 rebuilds, home-racks-noon with the switch off,
//!   2026-11-13, run.log's "Homestead rebuilt" lines). So it runs on a schedule, once every
//!   [`HOME_REUSE_OFF_REBUILD_S`] while aboard (`home_rebuild_due`, `frame`);
//! * the garden: a whole garden is about 400 ms of the worker and 3 million vertices to upload, so
//!   a full rebuild starts once the last has finished and [`GARDEN_REUSE_OFF_REBUILD_S`] after it
//!   began (`garden_full_rebuild_due`).
//!
//! What does run every frame with the switch off: the stock piles' layout (`stock_layout_kept`).
//! These are pure so the tests never flip the process-wide switch (`ai::client_tests` does).

use crate::engine::state::EngineState;

/// With the reuse switch off, how often the home is rebuilt while aboard, seconds.
/// Five seconds, not two: at half a second a rebuild, two would hold the game still a quarter of
/// the time.
pub(crate) const HOME_REUSE_OFF_REBUILD_S: f32 = 5.0;

/// Is the home rebuilt this frame for the reuse switch? Never with it on (the edit flags decide);
/// with it off, while aboard (the only place the home is drawn), once every
/// [`HOME_REUSE_OFF_REBUILD_S`] since the last.
pub(crate) fn home_rebuild_due(reuse: bool, aboard: bool, since_last_s: f32) -> bool {
    !reuse && aboard && since_last_s >= HOME_REUSE_OFF_REBUILD_S
}

/// With the reuse switch off, the least time between two full rebuilds of the garden, seconds.
pub(crate) const GARDEN_REUSE_OFF_REBUILD_S: f32 = 2.0;

/// Does a FULL garden rebuild start this frame (every machine's plants, the data files read
/// again, the models found missing tried again)? Never with reuse on (the per-machine
/// signatures, the data files' stamps and the misses decide what is built again); with it off,
/// once the pass is idle and [`GARDEN_REUSE_OFF_REBUILD_S`] has passed since the last one began.
pub(crate) fn garden_full_rebuild_due(reuse: bool, idle: bool, since_last_s: f32) -> bool {
    !reuse && idle && since_last_s >= GARDEN_REUSE_OFF_REBUILD_S
}

/// What the stock piles' layout was made from: the stored volume of each kind, and the home's
/// edit count (`HomeDraw::edits`, bumped by every shell upload and machine rebuild), because the
/// piles stand in the storage zones' free floor, which an edit moves.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct StockKey {
    pub volumes: [f32; 3],
    pub edits: u64,
}

/// Is the kept stock-pile layout drawn again as it is? Only with reuse on, and only while neither
/// the stored volume (to 0.01 L) nor the home changed since it was laid out.
pub(crate) fn stock_layout_kept(have: &StockKey, now: &StockKey, reuse: bool) -> bool {
    reuse && have.edits == now.edits && (0..3).all(|k| (now.volumes[k] - have.volumes[k]).abs() <= 0.01)
}

/// A machine model's shared textured material, drawn at Furniture detail `drawn_at`: is it used
/// as it is for a body built at `divisor`? Only with reuse on and at the same level; otherwise
/// its picture is uploaded again from the model just read.
pub(crate) fn model_material_kept(drawn_at: u32, divisor: u32, reuse: bool) -> bool {
    humanity_core::reuse::keeps(Some(drawn_at), divisor, reuse)
}

/// Once a frame, after the edit flags (lib.rs): with the reuse switch off, rebuild the home on
/// its schedule. The doors keep their state across it: who opened a door and its lock states are
/// play, not data built from the ship, so a rebuild that leaves the doors where they are puts them
/// back (a real edit's rebuild resets them, `home_meshes::rebuild_door_panels`).
pub(crate) fn frame(state: &mut EngineState) {
    let since = state.home_draw.reuse_off_at.map_or(f32::MAX, |t| t.elapsed().as_secs_f32());
    let aboard = state.aboard_station && state.gui_state.ship_structure.is_some();
    if !home_rebuild_due(humanity_core::reuse::enabled(), aboard, since) {
        return;
    }
    state.home_draw.reuse_off_at = Some(std::time::Instant::now());
    let doors = (state.door_panels.len(), state.door_manual_open.clone(), state.door_locks.clone());
    // An empty list takes the machines down their full path: every body and model drawn afresh.
    state.machine_objects.clear();
    crate::engine::home_meshes::rebuild_homestead(state);
    if state.door_panels.len() == doors.0 {
        state.door_manual_open = doors.1;
        state.door_locks = doors.2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With the reuse switch off the home is rebuilt on its schedule while aboard, and only then;
    /// with it on, never (the edit flags decide). Seen red 2026-11-13 with the home kept whatever
    /// the switch said: "reuse off, aboard, 2 s: no rebuild" (the schedule was 2 s then).
    #[test]
    fn with_reuse_off_the_home_is_rebuilt_on_its_schedule() {
        assert!(home_rebuild_due(false, true, HOME_REUSE_OFF_REBUILD_S), "reuse off, aboard, its time come: no rebuild");
        assert!(home_rebuild_due(false, true, f32::MAX), "the first one at once");
        assert!(!home_rebuild_due(false, true, HOME_REUSE_OFF_REBUILD_S - 0.1), "not before its time");
        assert!(!home_rebuild_due(false, false, f32::MAX), "not while the home is out of sight");
        assert!(!home_rebuild_due(true, true, f32::MAX), "reuse on: only an edit rebuilds");
    }

    /// The same for the garden: a full rebuild with reuse off, once the last has finished and its
    /// time has come. Seen red 2026-11-13: "reuse off, idle, 2 s: no full garden rebuild".
    #[test]
    fn with_reuse_off_the_garden_is_built_again_whole_on_its_schedule() {
        assert!(garden_full_rebuild_due(false, true, GARDEN_REUSE_OFF_REBUILD_S), "reuse off, idle, 2 s: no full garden rebuild");
        assert!(!garden_full_rebuild_due(false, false, f32::MAX), "not over one still in flight");
        assert!(!garden_full_rebuild_due(false, true, 0.5), "not before its time");
        assert!(!garden_full_rebuild_due(true, true, f32::MAX), "reuse on: the signatures decide");
    }

    /// THE STOCK PILES REAL BUG (2026-11-13): the piles' layout was keyed by the stored volume
    /// alone, but the piles stand in the storage zones' floor. An edit that moved or shrank a
    /// storage zone and kept the same volume left the piles where the zone used to be. Seen red
    /// 2026-11-13 with the volume as the whole key: "an edit kept the old layout".
    #[test]
    fn the_stock_piles_follow_an_edit_that_keeps_the_volume() {
        let have = StockKey { volumes: [120.0, 40.0, 0.0], edits: 7 };
        assert!(stock_layout_kept(&have, &have, true), "nothing changed: kept");
        let edited = StockKey { edits: 8, ..have };
        assert!(!stock_layout_kept(&have, &edited, true), "an edit kept the old layout");
        let more = StockKey { volumes: [121.0, 40.0, 0.0], ..have };
        assert!(!stock_layout_kept(&have, &more, true), "more stock: laid out again");
        assert!(!stock_layout_kept(&have, &have, false), "reuse off: laid out every frame");
    }

    /// A machine model's material keeps its picture only with reuse on and at the same Furniture
    /// detail. Seen red 2026-11-13: "reuse off kept the model's picture".
    #[test]
    fn with_reuse_off_a_model_picture_is_uploaded_again() {
        assert!(model_material_kept(2, 2, true));
        assert!(!model_material_kept(1, 2, true), "a new detail level repaints");
        assert!(!model_material_kept(2, 2, false), "reuse off kept the model's picture");
    }
}
