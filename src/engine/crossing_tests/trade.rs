//! From gui/pages/trade.rs (`trade_moves_tests`): a trade settles once into the home a save load
//! puts back, through the engine's save and load (`save_load`).

use crate::gui::pages::trade::*;
use crate::gui::GuiState;
use crate::gui::pages::trade::trade_moves_tests::carry;
use crate::gui::pages::trade::trade_moves_tests::frames;
use crate::gui::pages::trade::trade_moves_tests::full_player;
use crate::gui::pages::trade::trade_moves_tests::hammer_for_wheat;
use crate::gui::pages::trade::trade_moves_tests::held;
use crate::gui::pages::trade::trade_moves_tests::op;
use crate::gui::pages::trade::trade_moves_tests::queued;
use crate::systems::inventory::Inventory;
use crate::systems::inventory::TradeSettlements;

/// BLOCKER (2026-10-02 review): the relay connects at the main menu, so a
/// trade settles into the home loaded at startup; clicking Play then loads
/// the save from disk (lib.rs `launcher_pending_load` ->
/// `save_load::apply_save_to_world`), which puts back the backpack AND the
/// settled set from before the trade, and nothing settled it again: inside
/// the 120 s autosave interval the items were lost. This runs that sequence
/// through the real functions in the main loop's order. Seen red,
/// 2026-10-02: with the re-settle taken out of `tick` (the old behaviour),
/// the loaded backpack still held (5 wheat, 0 hammers) after Play.
#[test]
fn a_trade_undone_by_a_save_load_settles_once_into_the_loaded_home() {
    let (mut world, p) = full_player();
    carry(&mut world, p, "wheat_0", 5);
    // The save on disk: the last autosave, from before the trade.
    let disk = crate::save_load::extract_world_save(&world);
    let mut inv = crate::systems::inventory::InventorySystem::new();
    let mut gs = GuiState::default();
    gs.profile_public_key = "bob".into();
    let known = |_: &str| true;

    // Main menu: the list fetched on connect settles the trade.
    assert!(route_trade_frame(&mut gs, &mut world, known, &hammer_for_wheat()));
    frames(1, &mut gs, &mut world, &mut inv);
    assert_eq!(held(&world, p), (0, 1));
    // Play: the save from disk is loaded onto the live world.
    crate::save_load::apply_save_to_world(&mut world, &disk);
    assert_eq!(held(&world, p), (5, 0), "the load put back the backpack from before the trade");
    // The frames after Play settle it into the loaded home.
    frames(3, &mut gs, &mut world, &mut inv);
    assert_eq!(held(&world, p), (0, 1), "the trade lands in the home that will be saved");
    {
        let ts = world.get::<&TradeSettlements>(p).unwrap();
        assert!(ts.pending.is_empty() && ts.settled.contains("t-1"), "{:?}", *ts);
    }
    // Exactly once: the list again (a reconnect) and more frames move nothing.
    assert!(route_trade_frame(&mut gs, &mut world, known, &hammer_for_wheat()));
    frames(3, &mut gs, &mut world, &mut inv);
    assert_eq!(held(&world, p), (0, 1));
    // Announced once: settling again after the load is quiet.
    assert_eq!(gs.pending_notices.len(), 1, "{:?}", gs.pending_notices);
    // The next save carries the trade with its items.
    let save = crate::save_load::extract_world_save(&world);
    assert_eq!(save.settled_trades, vec!["t-1".to_string()]);
    assert!(save.inventory.iter().any(|(id, q)| id == "hammer_0" && *q == 1), "{:?}", save.inventory);
}

/// A save load DROPS trade moves queued before it (2026-10-02, round 2):
/// they were sized against the backpack the load replaced, and `tick`
/// queues them again against the loaded one. Here the completion arrives
/// and Play loads the save in the same frame, before the systems tick.
/// Seen red, 2026-10-02: with `ts.pending.clear()` taken out of
/// `save_load::restore_settled_trades`, the move sized against the 2 wheat
/// carried before the load took 2 of the loaded 5, so 3 wheat the other
/// player also receives stayed behind: (3, 1).
#[test]
fn moves_queued_before_a_save_load_are_sized_again_against_the_loaded_backpack() {
    let (mut world, p) = full_player();
    carry(&mut world, p, "wheat_0", 5);
    let disk = crate::save_load::extract_world_save(&world);
    // Since that save, 3 of the wheat were eaten.
    world.get::<&mut Inventory>(p).unwrap().remove_item("wheat_0", 3);
    let mut inv = crate::systems::inventory::InventorySystem::new();
    let mut gs = GuiState::default();
    gs.profile_public_key = "bob".into();

    assert!(route_trade_frame(&mut gs, &mut world, |_: &str| true, &hammer_for_wheat()));
    assert_eq!(queued(&world, p)[0].1, vec![op("wheat_0", 2, false), op("hammer_0", 1, true)]);
    crate::save_load::apply_save_to_world(&mut world, &disk);
    frames(3, &mut gs, &mut world, &mut inv);
    assert_eq!(held(&world, p), (0, 1), "all 5 wheat the loaded home holds go, as the other player receives 5");
}
