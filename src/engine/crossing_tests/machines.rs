//! From machines.rs's tests: the drone dock shows while the drone is out, by the engine's
//! `drone_dock_visible`.


#[test]
fn drone_dock_visibility_tracks_drone_active_flag() {
    // Calls the REAL crate-level fn the render call site in src/lib.rs also calls (moved to
    // crate root and out of the native-only module 2026-07-01 specifically so this test can
    // reach it without pulling in rendering/ECS deps; a hand-copied duplicate here previously
    // could not have caught a regression in the real gate, this can).
    use crate::drone_dock_visible;
    // Docked: no drone out, in gameplay, machines not decluttered away.
    assert!(drone_dock_visible(false, false, false), "no drone in flight -> docked model shows");
    // Undocked: a drone launched (Outbound/Mining/Returning) -> gui_state.drone_active is true.
    assert!(!drone_dock_visible(false, false, true), "a drone is out -> the pad reads empty");
    // Never shown in the character showroom, regardless of drone state.
    assert!(!drone_dock_visible(true, false, false));
    // Never shown while the "Machine" type is decluttered away in the build editor.
    assert!(!drone_dock_visible(false, true, false));
}
