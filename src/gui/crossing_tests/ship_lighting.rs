//! From ship/ship_structure.rs (`lighting_watts_tests`): the shipped lights draw real watts through
//! the renderer's light types.

use crate::ship::ship_structure::*;

/// v0.967 (homestead increment 5): the shipped structure's switched-on
/// lights must sum to a sane, NONZERO wattage through the shipped
/// light-type table - the house lighting is real load on the power
/// meter, and a light-type rename that silently zeroes the bill fails
/// here instead of in the HUD.
#[test]
fn shipped_lights_draw_real_watts() {
    // The ship the game runs (the ship file + the homestead on its default plot).
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let s = ShipStructure::load_and_assemble_shipped(&data, None).expect("the shipped ship assembles");
    let watts = s.lighting_watts(|id| {
        crate::renderer::light::light_type(id).map(|t| t.watts).unwrap_or(0.0)
    });
    assert!(
        (100.0..2000.0).contains(&watts),
        "shipped lighting should draw a realistic LED-household load, got {watts} W"
    );
    // Every shipped light type must carry a wattage (a new entry with
    // watts omitted defaults to 0 = free power, which is a lie).
    for t in crate::renderer::light::light_types() {
        assert!(t.watts > 0.0, "light type {} has no wattage", t.id);
    }
}
