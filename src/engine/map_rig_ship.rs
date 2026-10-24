//! The Maps page's ship verbs and the ship block of the rig's report (Maps increment 3, "into
//! the ship, deck by deck"): permanent dev tooling the probe rig's ship vantages drive the page
//! with, beside `engine/map_rig.rs` (whose line budget they would not fit).
//!
//! The verbs: `{"map_level":"<ordinal>"|"you"|"whole"}` shows a level cut open (as the level
//! picker does, `ship_view::pick_level`) and `{"map_plan":"1"|"0"}` turns the straight-down
//! plan on or off (`ship_view::set_plan`). The report's `ship` block says which levels the
//! picker lists, the one shown, the rooms named and filled on it, whether your dot was drawn, and
//! the plan tiles (wanted, kept, baked, the last bake's time, and the tiles an edit dropped with
//! the footprints that made it drop them).

use crate::gui::pages::maps::ship_view;
use crate::gui::{LevelVerb, MapVerb, MapsState};

/// Read Maps increment 3's verbs into `verb`; true when there were any.
pub(crate) fn parse(get: &dyn Fn(&str) -> Option<String>, verb: &mut MapVerb) -> Result<bool, String> {
    let mut any = false;
    if let Some(l) = get("map_level") {
        any = true;
        verb.level = Some(match l.trim() {
            "you" => LevelVerb::You,
            "whole" => LevelVerb::Whole,
            n => LevelVerb::Ordinal(n.parse::<i32>().map_err(|_| format!("map_level wants a level's ordinal, you or whole, not {l:?}"))?),
        });
    }
    if let Some(p) = get("map_plan") {
        any = true;
        verb.plan = Some(match p.trim() {
            "1" => true,
            "0" => false,
            other => return Err(format!("map_plan wants 0 or 1, not {other:?}")),
        });
    }
    Ok(any)
}

/// Apply them (after `map_rig::apply_verb`'s own): the home and overview verbs start on the
/// whole ship; a level is shown as the picker shows it, the plan switched as its switch does.
pub(crate) fn apply(maps: &mut MapsState, verb: &MapVerb) {
    if verb.home || verb.overview {
        maps.level_pick = None;
        maps.plan = false;
    }
    match verb.level {
        Some(LevelVerb::Ordinal(o)) => ship_view::pick_level(maps, Some(o)),
        Some(LevelVerb::You) => {
            let you = maps.me.as_ref().and_then(|m| m.level);
            ship_view::pick_level(maps, you);
        }
        Some(LevelVerb::Whole) => ship_view::pick_level(maps, None),
        None => {}
    }
    if let Some(on) = verb.plan {
        ship_view::set_plan(maps, on);
    }
}

/// The report's `ship` block (see the module doc).
pub(crate) fn report(maps: &MapsState) -> serde_json::Value {
    let s = &maps.ship_drawn;
    let t = &maps.tiles;
    let key = |k: &crate::engine::map_tiles::TileKey| serde_json::json!({ "level": k.level, "z": k.z, "x": k.x, "y": k.y, "rect": k.rect() });
    let mut have: Vec<_> = t.have.keys().copied().collect();
    have.sort();
    serde_json::json!({
        "levels": s.levels,
        "level": s.level,
        "level_pick": maps.level_pick,
        "you_level": maps.me.as_ref().and_then(|m| m.level),
        "you_room": maps.me.as_ref().and_then(|m| m.you_room.as_ref().map(|r| r.1.clone())),
        "plan": maps.plan,
        "rooms_named": s.rooms_named,
        "rooms_filled": s.rooms_filled,
        "you_drawn": s.you_drawn,
        "tiles": {
            "want": t.want.iter().map(key).collect::<Vec<_>>(),
            "have": have.iter().map(key).collect::<Vec<_>>(),
            "baked": t.baked,
            "bytes": t.bytes,
            "last_bake_ms": t.last_bake_ms,
            "dropped": t.dropped.iter().map(key).collect::<Vec<_>>(),
            "dirty": t.dirty,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |k: &str| pairs.iter().find(|(key, _)| *key == k).map(|(_, v)| v.to_string())
    }

    /// The ship verbs parse through the map's own (`map_rig::parse_map_verb`), and a typo is
    /// refused with the reason, so a rig vantage cannot ask for a level that is not a level.
    ///
    /// Seen red 2026-10-22 with `parse` never called from `parse_map_verb`: left `Ok(None)`,
    /// right `Ok(Some(MapVerb { .., level: Some(Ordinal(1)), plan: Some(true) }))`.
    #[test]
    fn the_ship_verbs_parse() {
        let parsed = crate::engine::map_rig::parse_map_verb(get(&[("map_level", "1"), ("map_plan", "1")]));
        assert_eq!(parsed, Ok(Some(MapVerb { level: Some(LevelVerb::Ordinal(1)), plan: Some(true), ..Default::default() })));
        assert_eq!(crate::engine::map_rig::parse_map_verb(get(&[("map_level", "you")])), Ok(Some(MapVerb { level: Some(LevelVerb::You), ..Default::default() })));
        assert_eq!(crate::engine::map_rig::parse_map_verb(get(&[("map_level", "whole")])), Ok(Some(MapVerb { level: Some(LevelVerb::Whole), ..Default::default() })));
        assert!(crate::engine::map_rig::parse_map_verb(get(&[("map_level", "upstairs")])).is_err());
        assert!(crate::engine::map_rig::parse_map_verb(get(&[("map_plan", "yes")])).is_err());
    }
}
