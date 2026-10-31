//! From ship/moves.rs's tests: the relay's on-foot limit covers the fastest legitimate walk, the
//! renderer's walk and sprint times every speed buff and piece of gear the simulation knows.

use crate::ship::moves::tests::shipped;

/// The fastest a person can walk on these data files, folded by the very functions lib.rs
/// moves the player with (the review of increment 4, R2: this read `speed:<v>:multiply` from
/// the text and missed every other kind of modifier the folds apply): the walk and the sprint
/// the movement code uses, times the status effects' `net_stat_multiplier` over every effect
/// that speeds a person up, each as often as it stacks (the CSV's `max_stacks`, which the
/// registry reads; nothing stacks an effect yet), times the gear's `net_stat_multiplier` over every piece of gear
/// that does, all worn at once (more than any one outfit can). The folds run in the order
/// they are given, and an `add` counts for most before the multiplies, so those come first.
#[cfg(feature = "native")]
fn fastest_walk_mps(effects: &str, equipment: &str) -> f32 {
    use crate::renderer::camera::{SPRINT_FACTOR, WALK_SPEED_MPS};
    use crate::systems::economy::EquipmentRegistry;
    use crate::systems::status_effects::StatusEffectRegistry;
    let rows = |csv: &str| -> Vec<Vec<String>> {
        csv.lines().filter(|l| !l.starts_with('#') && !l.starts_with("id,") && !l.trim().is_empty()).map(|l| l.split(',').map(str::to_string).collect()).collect()
    };
    let fx = StatusEffectRegistry::from_csv(effects.as_bytes()).expect("the status effects parse");
    // status_effects.csv: id,name,type,duration_s,stackable,max_stacks,tick_interval_s,stat_modifier,...
    let mut buffs: Vec<(bool, String, usize)> = rows(effects)
        .into_iter()
        .filter(|c| fx.net_stat_multiplier([c[0].as_str()], "speed") > 1.0)
        .map(|c| {
            let adds = fx.get(&c[0]).and_then(|d| d.modifier()).is_some_and(|(_, _, op)| op == "add");
            (adds, c[0].clone(), fx.get(&c[0]).map_or(1, |d| d.max_stacks as usize).max(1))
        })
        .collect();
    buffs.sort_by_key(|(adds, _, _)| !*adds);
    let ids: Vec<&str> = buffs.iter().flat_map(|(_, id, n)| std::iter::repeat(id.as_str()).take(*n)).collect();
    let from_effects = fx.net_stat_multiplier(ids, "speed");
    let gear = EquipmentRegistry::from_csv(equipment.as_bytes()).expect("the gear parses");
    let mut worn: Vec<(bool, String)> = rows(equipment)
        .into_iter()
        .filter(|c| gear.net_stat_multiplier([c[0].as_str()], "speed") > 1.0)
        .map(|c| (gear.get(&c[0]).is_some_and(|d| d.stat_modifiers.contains(":add")), c[0].clone()))
        .collect();
    worn.sort_by_key(|(adds, _)| !*adds);
    let from_gear = gear.net_stat_multiplier(worn.iter().map(|(_, id)| id.as_str()), "speed");
    WALK_SPEED_MPS * SPRINT_FACTOR * from_effects * from_gear
}

/// THE ON-FOOT LIMIT COVERS THE FASTEST LEGITIMATE WALK: the walk and the sprint the
/// movement code uses, times every speed buff of data/status_effects.csv at its most stacks,
/// times every speed modifier of data/equipment.csv above 1. A new buff or a faster walk that
/// passes the limit would have the relay correct honest players, so it fails here first.
/// Seen red 2026-10-04 with the limit at 9.5 (the plain sprint): "the fastest legitimate walk
/// is 24.11 m/s, over the on-foot limit of 9.50".
#[cfg(feature = "native")]
#[test]
fn the_on_foot_limit_covers_the_fastest_legitimate_walk() {
    use crate::renderer::camera::{SPRINT_FACTOR, WALK_SPEED_MPS};
    let effects = std::fs::read_to_string("data/status_effects.csv").expect("data/status_effects.csv");
    let equipment = std::fs::read_to_string("data/equipment.csv").expect("data/equipment.csv");
    let fastest = fastest_walk_mps(&effects, &equipment);
    let limit = shipped().moving.on_foot_mps;
    assert!(fastest > WALK_SPEED_MPS * SPRINT_FACTOR, "the buffs were found: {fastest}");
    assert!(fastest <= limit, "the fastest legitimate walk is {fastest:.2} m/s, over the on-foot limit of {limit:.2}");
}

/// EVERY KIND OF SPEED BUFF COUNTS TOWARD THE FASTEST WALK (the review of increment 4, R2).
/// Both registries the game moves the player with (status_effects.rs and economy's
/// `net_stat_multiplier`, lib.rs) fold an `add` modifier as well as a `multiply` one, but the
/// on-foot limit's test read `speed:<v>:multiply` only: a new buff written as `add` would have
/// passed it while the relay corrected every honest player who had it. A buff of a half
/// again, written as `add`, must raise the fastest walk by half.
///
/// Seen red 2026-10-04 on the code before the fix (the bound read `speed:<v>:multiply` from the
/// text): "an `add` speed buff was not counted: 24.11 m/s with it, 24.11 without".
#[cfg(feature = "native")]
#[test]
fn every_kind_of_speed_buff_counts_toward_the_fastest_walk() {
    let effects = std::fs::read_to_string("data/status_effects.csv").expect("data/status_effects.csv");
    let equipment = std::fs::read_to_string("data/equipment.csv").expect("data/equipment.csv");
    let without = fastest_walk_mps(&effects, &equipment);
    let tonic = format!("{}\nsprint_tonic,Sprint Tonic,buff,60,false,1,0,speed:0.5:add,0,none,0,movement,Half again as fast\n", effects.trim_end());
    let with = fastest_walk_mps(&tonic, &equipment);
    assert!(with >= without * 1.49, "an `add` speed buff was not counted: {with:.2} m/s with it, {without:.2} without");
    let boots = format!("{}\nboots_sprint_0,feet,0.08,0,0,0,0,0,0,,0,speed:0.5:add,Racing spikes\n", equipment.trim_end());
    let with_gear = fastest_walk_mps(&effects, &boots);
    assert!(with_gear >= without * 1.49, "an `add` speed modifier on gear was not counted: {with_gear:.2} m/s with it, {without:.2} without");
}
