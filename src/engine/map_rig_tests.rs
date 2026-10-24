//! The Maps rig verbs' tests (Maps increments 1 and 2; moved out of map_rig.rs in increment 3 to
//! keep its line budget). Each was seen red first; what broke it and what it said is in its comment.

use super::*;

fn get(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    move |k: &str| pairs.iter().find(|(key, _)| *key == k).map(|(_, v)| v.to_string())
}

/// The rig's verbs parse, and a typo is refused with the reason.
///
/// Seen red 2026-10-21 with "unfollow" read as follow: left `follow:
/// Some(true)`, right `follow: Some(false)`.
#[test]
fn the_map_verbs_parse() {
    assert_eq!(parse_map_verb(get(&[])), Ok(None));
    assert_eq!(
        parse_map_verb(get(&[("map", "unfollow"), ("map_distance", "1500")])),
        Ok(Some(MapVerb { follow: Some(false), distance_m: Some(1500.0), ..Default::default() }))
    );
    assert_eq!(
        parse_map_verb(get(&[("map_ship", "0"), ("map_report", "1")])),
        Ok(Some(MapVerb { ship: Some(false), report: true, ..Default::default() }))
    );
    assert!(parse_map_verb(get(&[("map", "hoem")])).is_err());
    assert!(parse_map_verb(get(&[("map_distance", "-3")])).is_err());
}

/// Increment 2's verbs: the overview, a layer switched by its id in
/// layers.ron (an id the file does not have is refused, so a rig vantage
/// cannot switch a layer that does not exist and pass), and the Diagram.
///
/// Seen red 2026-10-21 with any layer id accepted: "map_layer accepted
/// \"belt\", which layers.ron does not have".
#[test]
fn the_overview_layer_and_diagram_verbs_parse() {
    assert_eq!(
        parse_map_verb(get(&[("map", "overview"), ("map_layer", "belts=0, names=1"), ("map_diagram", "1")])),
        Ok(Some(MapVerb { overview: true, layers: vec![("belts".into(), false), ("names".into(), true)], diagram: Some(true), ..Default::default() }))
    );
    assert!(parse_map_verb(get(&[("map_layer", "belt=0")])).is_err(), "map_layer accepted \"belt\", which layers.ron does not have");
    assert!(parse_map_verb(get(&[("map_layer", "belts")])).is_err());
    assert!(parse_map_verb(get(&[("map_diagram", "yes")])).is_err());
}
