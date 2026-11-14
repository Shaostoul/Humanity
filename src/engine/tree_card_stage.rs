//! The tree card stage in the frame loop (BUG-301, 2026-11-14; the terrain half is
//! `humanity_terrain::tree_card_stage`).
//!
//! Three things live here:
//! - [`floor_for`]: the depth floor the planet selection asks for, so the stage has its depth-15
//!   ground without the tile tier (before this, no release and, since BUG-287, no dev boot drew a
//!   single tree card).
//! - The showcase switch `card_floor` ("0" off, "1" on): the red twin (the stage as it was, empty
//!   without the tile tier) and the same-boot arm the stage's frame cost is measured against.
//! - The counter: [`note`] reads each frame's draw list once the selection is made, logs a
//!   `[TreeCards]` line once a second, and keeps the reading that every screenshot's done file
//!   carries as `tree_cards` ([`done_json`]). probe-sweep fails a vantage whose `min_tree_cards`
//!   that reading misses (scripts/lib/tree-cards-check.js), which is the check that would have
//!   caught this bug.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use humanity_terrain::planet::PlanetDef;
use humanity_terrain::planet_chunks::ChunkState;
use humanity_terrain::tree_card_stage::{tally, CardFloor, CardTally};

/// The showcase `card_floor` pin: true while it is "0" (the floor off).
static FLOOR_OFF: AtomicBool = AtomicBool::new(false);

/// Set the showcase `card_floor` pin (`on` false = the red twin).
pub(crate) fn set_floor_on(on: bool) {
    FLOOR_OFF.store(!on, Ordering::Relaxed);
}

/// Is the card floor on (the showcase pin not "0")?
pub(crate) fn floor_on() -> bool {
    !FLOOR_OFF.load(Ordering::Relaxed)
}

/// The floor for this planet this frame: the card depth out to the requested "Tree silhouette
/// distance" (`veg_tree_card_m`, with the same clamp the renderer's cutoff takes), on a body whose
/// air is breathable with the forest slider above zero.
pub(crate) fn floor_for(def: &PlanetDef, settings: &crate::gui::SettingsState) -> CardFloor {
    if !floor_on() {
        return CardFloor::default();
    }
    CardFloor::for_stage(
        def.breathable,
        settings.tree_density,
        settings.veg_tree_card_m.clamp(100.0, crate::config::TREE_CARD_MAX_M),
    )
}

/// One frame's reading, with the window it was counted against.
#[derive(Clone, Copy)]
struct Reading {
    at: Instant,
    tally: CardTally,
    hide_m: f32,
    far_m: f32,
    atlas_ready: bool,
    floor: bool,
}

static LAST: Mutex<Option<Reading>> = Mutex::new(None);

/// Count this frame's draw list (call once the selection's `last_drawn` is in place). `hide_m` and
/// `far_m` are the renderer's card window, `atlas_ready` whether the card sprites are baked.
pub(crate) fn note(cs: &ChunkState, radius_m: f64, hide_m: f32, far_m: f32, atlas_ready: bool) {
    let r = Reading {
        at: Instant::now(),
        tally: tally(cs, radius_m, hide_m, far_m),
        hide_m,
        far_m,
        atlas_ready,
        floor: floor_on(),
    };
    if let Ok(mut last) = LAST.lock() {
        *last = Some(r);
    }
    static LAST_LOG_S: AtomicU64 = AtomicU64::new(0);
    let now_s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if LAST_LOG_S.swap(now_s, Ordering::Relaxed) != now_s {
        let t = r.tally;
        log::info!(
            "[TreeCards] drawn={} card_patches={} cards={} in_window={} max_depth={} hide={:.0}m far={:.0}m atlas={} floor={}",
            t.drawn_patches,
            t.card_patches,
            t.cards,
            t.cards_in_window,
            t.max_drawn_depth,
            hide_m,
            far_m,
            atlas_ready,
            r.floor,
        );
    }
}

/// The last reading as the done file's `tree_cards` object, or null when no planet selection has
/// run in the last two seconds (in space, aboard, on the menu).
pub(crate) fn done_json() -> serde_json::Value {
    let Some(r) = LAST.lock().ok().and_then(|l| *l) else {
        return serde_json::Value::Null;
    };
    if r.at.elapsed().as_secs_f32() > 2.0 {
        return serde_json::Value::Null;
    }
    let t = r.tally;
    serde_json::json!({
        "drawn_patches": t.drawn_patches,
        "card_patches": t.card_patches,
        "cards": t.cards,
        "in_window": t.cards_in_window,
        "max_drawn_depth": t.max_drawn_depth,
        "hide_m": r.hide_m,
        "far_m": r.far_m,
        "atlas_ready": r.atlas_ready,
        "floor": r.floor,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The showcase pin turns the floor off and back on; with it off the floor is the default
    /// (the red twin: the selection's own cap, no card ground without the tile tier).
    #[test]
    fn the_showcase_pin_turns_the_floor_off_and_on() {
        let def: PlanetDef = ron::from_str(
            r#"(name: "T", radius: 6371000.0, gravity: 9.81, terrain_seed: 1, ore_seed: 1, breathable: true)"#,
        )
        .expect("def parses");
        let settings = crate::gui::SettingsState::default();
        assert!(floor_for(&def, &settings).depth > 0, "the shipped settings grow a card floor on a breathable world");
        set_floor_on(false);
        assert_eq!(floor_for(&def, &settings), CardFloor::default());
        set_floor_on(true);
        assert!(floor_for(&def, &settings).depth > 0);
        let airless: PlanetDef = ron::from_str(
            r#"(name: "M", radius: 1737000.0, gravity: 1.62, terrain_seed: 1, ore_seed: 1)"#,
        )
        .expect("def parses");
        assert_eq!(floor_for(&airless, &settings), CardFloor::default(), "no floor where no tree grows");
    }
}
