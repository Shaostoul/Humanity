//! Tests of home_meshes.rs's pure parts: what an OPEN plot lights (ship homes increment 5b,
//! docs/design/ship-homes-increment-5b-plan.md section 3.7). Each was seen to fail before it
//! passed; the failure is in its comment.

use super::*;
use crate::ship::ship_structure::{HomeDesign, OpenPlots};

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

/// The shipped ship with the home on `plot`.
fn ship(plot: &str) -> ShipStructure {
    ShipStructure::load_and_assemble_shipped(&data_dir(), Some(plot)).expect("the shipped ship assembles")
}

/// `plots` open, `lit` the one lit.
fn opened(mut s: ShipStructure, plots: &[&str], lit: Option<&str>) -> ShipStructure {
    s.open = OpenPlots { plots: plots.iter().map(|p| p.to_string()).collect(), lit: lit.map(str::to_string) };
    s
}

/// THE OPEN PLOT YOU STAND IN IS LIT, AND ON NOBODY'S METER (plan section 5, lane 1B; section 3.7's
/// table, "Light inside"). On a ship whose home is on p2, p1 open and lit: the room lights grow by
/// exactly the lights of p1's shipped design (every switched-on one, a strip as its segments), all
/// of them standing on p1, and the home's own lights are unchanged and first. The lighting meter
/// (`ShipStructure::lighting_watts`, what the house lighting consumer draws) reads the same: those
/// lights are no zone's. Open but not lit, lit but not open, and the home's own plot "lit" all
/// add nothing: a visited plot is lit only while its permit holder stands in it.
/// Seen red 2026-10-12 with the lights not added (`home_lights` dropping `open_plot_lights`):
/// "the lit open plot p1 adds 0 lights; its shipped design lights it with 49" failed.
#[test]
fn the_open_plot_you_stand_in_is_lit_and_on_nobodys_meter() {
    let watts_of = |id: &str| crate::renderer::light::light_type(id).map(|t| t.watts).unwrap_or(0.0);
    let closed = ship("p2");
    let before = home_lights(Some(&closed), Vec::new(), true);
    assert!(!before.is_empty(), "the home on p2 has lights of its own");

    let lit = opened(closed.clone(), &["p1"], Some("p1"));
    let after = home_lights(Some(&lit), Vec::new(), true);
    let design = HomeDesign::built_in_ref("homestead").expect("the homestead is built in");
    let p1 = lit.plots.iter().find(|p| p.id == "p1").expect("p1");
    let want = placed_room_lights(&design.body.lights, Vec3::new(p1.origin.0, p1.origin.1, p1.origin.2));
    let switched_on = design.body.lights.iter().filter(|l| l.on).count();
    assert!(switched_on > 10 && want.len() >= switched_on, "the shipped homestead's lights: {switched_on} on, {} drawn", want.len());
    let added = after.len() - before.len();
    assert_eq!(added, want.len(), "the lit open plot p1 adds {added} lights; its shipped design lights it with {}", want.len());
    // The home's own lights come first, unchanged; the added ones all stand on p1.
    for (a, b) in before.iter().zip(&after) {
        assert!((a.pos - b.pos).length() < 1e-5 && a.intensity == b.intensity, "the home's own light moved: {:?} vs {:?}", a.pos, b.pos);
    }
    let (lo, hi) = p1.aabb();
    for l in &after[before.len()..] {
        assert!(
            l.pos.x >= lo.x - 0.01 && l.pos.x <= hi.x + 0.01 && l.pos.z >= lo.z - 0.01 && l.pos.z <= hi.z + 0.01,
            "a light of the open plot stands off p1 ({lo:?} to {hi:?}): {:?}",
            l.pos
        );
    }
    // Nobody's meter: the zones' lights, which the meter reads, are the same.
    assert_eq!(lit.lighting_watts(watts_of), closed.lighting_watts(watts_of), "the open plot's lights are charged to a meter");

    // Only where you stand, only what is open, only a neighbour's plot.
    for (plots, lit_plot, why) in [
        (&["p1"][..], None, "p1 open, nothing lit"),
        (&["p3"][..], Some("p1"), "p1 lit but not open"),
        (&["p2"][..], Some("p2"), "the home's own plot"),
    ] {
        let s = opened(closed.clone(), plots, lit_plot);
        assert_eq!(home_lights(Some(&s), Vec::new(), true).len(), before.len(), "{why}: lights were added");
    }
    // The open plot's lights never decide the auto fill: with the auto fill in use (a ship whose
    // zones have no placed lights), it stays, and the plot's lights come after it.
    let mut dark = lit.clone();
    for z in dark.zones.iter_mut() {
        z.body.lights.clear();
    }
    let auto = auto_room_lights(std::iter::once((Vec3::new(80.0, 2.0, 40.0), Vec3::new(10.0, 4.0, 10.0))));
    let filled = home_lights(Some(&dark), auto, true);
    assert_eq!(filled.len(), 1 + want.len(), "the auto fill went out when a neighbour's plot was lit");
}
