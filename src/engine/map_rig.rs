//! The Maps page's dev verbs and the rig's report (Maps increments 1 and 2):
//! permanent dev tooling the probe rig's map vantages drive the page with,
//! moved out of `engine/map_view.rs` in increment 2 when the overview, the
//! layer switches and the Diagram joined them.
//!
//! The verbs (`engine/ipc.rs` `poll_showcase_request`):
//! `{"map":"home"|"overview"|"follow"|"unfollow"|"report"}`,
//! `{"map_distance":"<m>"}`, `{"map_ship":"0"|"1"}`, `{"map_turn":"<deg/s>"}`,
//! `{"map_layer":"belts=0,names=1"}`, `{"map_diagram":"0"|"1"}`, and
//! `{"map_report":"1"}` riding along with any of them. They are queued for the
//! feed (`engine/map_feed.rs`), which applies them after its page-open handling.
//!
//! The report (`debug/map_done.json`) says where the canvas is, where the ship
//! landed, what the world layer drew and what the symbols were (increment 2's
//! `MapSymbology`: bodies, orbits, names placed and dropped, belt points, the
//! layers drawn, the band, the time the symbols took).

use crate::engine::state::EngineState;
use crate::gui::pages::maps::camera::{MapScene, MAX_DISTANCE_M, MIN_DISTANCE_M};
use crate::gui::pages::maps::you_layer::WorldLayerWant;
use crate::gui::pages::maps::MapView;
use crate::gui::GuiPage;
use crate::gui::{MapVerb, MapsState};

/// Where the rig's report goes.
const REPORT_PATH: &str = "debug/map_done.json";

/// Queue the map verbs of a showcase request.
pub(crate) fn showcase(state: &mut EngineState, text: &str) {
    match parse_map_verb(|k| crate::engine::ipc::showcase_value(text, k)) {
        Ok(Some(verb)) => {
            log::info!("Showcase: map -> {verb:?}");
            state.gui_state.maps.pending.push(verb);
        }
        Ok(None) => {}
        Err(why) => log::warn!("Showcase: {why}"),
    }
}

fn zero_or_one(key: &str, v: &str) -> Result<bool, String> {
    match v.trim() {
        "1" => Ok(true),
        "0" => Ok(false),
        other => Err(format!("{key} wants 0 or 1, not {other:?}")),
    }
}

/// Read the map verbs out of a showcase request. None when it has none.
pub(crate) fn parse_map_verb(get: impl Fn(&str) -> Option<String>) -> Result<Option<MapVerb>, String> {
    let mut verb = MapVerb::default();
    let mut any = false;
    if let Some(m) = get("map") {
        any = true;
        match m.trim() {
            "home" => verb.home = true,
            "overview" => verb.overview = true,
            "follow" => verb.follow = Some(true),
            "unfollow" => verb.follow = Some(false),
            "report" => verb.report = true,
            other => return Err(format!("map wants home, overview, follow, unfollow or report, not {other:?}")),
        }
    }
    if let Some(d) = get("map_distance") {
        any = true;
        match d.trim().parse::<f64>() {
            Ok(m) if m.is_finite() && m > 0.0 => verb.distance_m = Some(m),
            _ => return Err(format!("map_distance wants metres above 0, not {d:?}")),
        }
    }
    if let Some(s) = get("map_ship") {
        any = true;
        verb.ship = Some(zero_or_one("map_ship", &s)?);
    }
    if let Some(t) = get("map_turn") {
        any = true;
        match t.trim().parse::<f64>() {
            Ok(dps) if dps.is_finite() && dps.abs() <= 360.0 => verb.turn_dps = Some(dps),
            _ => return Err(format!("map_turn wants degrees a second, -360 to 360, not {t:?}")),
        }
    }
    if let Some(l) = get("map_layer") {
        any = true;
        for part in l.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let (id, on) = part.split_once('=').ok_or_else(|| format!("map_layer wants id=0 or id=1, not {part:?}"))?;
            let id = id.trim();
            if !crate::gui::pages::maps::overlays::layers().iter().any(|layer| layer.id == id) {
                return Err(format!("map_layer: data/maps/layers.ron has no layer {id:?}"));
            }
            verb.layers.push((id.to_string(), zero_or_one("map_layer", on)?));
        }
    }
    if let Some(d) = get("map_diagram") {
        any = true;
        verb.diagram = Some(zero_or_one("map_diagram", &d)?);
    }
    // A report rides along with any other verb when asked for.
    if get("map_report").as_deref() == Some("1") {
        any = true;
        verb.report = true;
    }
    Ok(any.then_some(verb))
}

/// Write `debug/map_done.json` once the rig's report request has been
/// drawn: the page drew at or after the request, and with the world layer
/// wanted (the whole world or the sky), it rendered at or after it too.
/// `want` is None when the page is not open (written at once, saying so).
pub(crate) fn write_report_if_due(state: &mut EngineState, want: Option<WorldLayerWant>) {
    let maps = &state.gui_state.maps;
    let Some(after) = maps.report_after_seq else { return };
    if let Some(w) = want {
        let drawn = maps.drawn_seq >= after;
        let renders = state.world_loaded && (w == WorldLayerWant::Draw || w.draws_sky());
        let rendered = !renders || maps.world.rendered_seq >= after;
        if !(drawn && rendered) {
            return;
        }
    }
    let report = report_json(&state.gui_state.maps, want, state.gui_state.active_page == GuiPage::Maps);
    let _ = std::fs::create_dir_all("debug");
    let _ = std::fs::write(REPORT_PATH, report.to_string());
    state.gui_state.maps.report_after_seq = None;
}

/// The report's body (pure, so its shape is pinned without an engine).
pub(crate) fn report_json(maps: &crate::gui::MapsState, want: Option<WorldLayerWant>, open: bool) -> serde_json::Value {
    use crate::gui::pages::maps::camera::MapAnchor;
    let anchor = match &maps.camera.anchor {
        MapAnchor::Me => "me".to_string(),
        MapAnchor::Body(id) => format!("body:{id}"),
        MapAnchor::Point { body, .. } => format!("point:{body}"),
    };
    let s = &maps.symbology;
    serde_json::json!({
        "ok": true,
        "page_open": open,
        "follow_me": maps.camera.follows_me(),
        "anchor": anchor,
        "distance_m": maps.camera.distance(),
        "live": maps.live,
        "diagram": maps.diagram,
        "aboard": maps.me.as_ref().map(|m| m.aboard),
        "world": {
            "want": want.map(|w| format!("{w:?}")),
            "renders": maps.world.renders,
            "size": maps.world.size,
            "half": maps.world.half,
            "ship_drawn": maps.world.ship_drawn,
            "sky_only": maps.world.sky_only,
        },
        "symbology": {
            "band": s.band,
            "bodies": s.bodies,
            "orbits": s.orbits,
            "orbit_vertices": s.orbit_vertices,
            "labels_placed": s.labels_placed,
            "labels_dropped": s.labels_dropped,
            "belt_points": s.belt_points,
            "layers": s.layers,
            "ms": s.ms,
        },
        "canvas_px": maps.canvas_px,
        "ship_px": maps.ship_px,
        "frame_seq": maps.frame_seq,
    })
}

/// How far the overview verb stands from the Sun: out past Jupiter, so the
/// inner planets, the main belt, the Hildas and the trojans are all in view.
const OVERVIEW_VERB_M: f64 = 11.0 * crate::cosmos::M_PER_AU;

/// One rig request (`MapVerb`), applied to the page's state. Called by the
/// feed (`map_feed::frame_update`) after its page-open handling, so a request
/// that opens the page and stops following is not undone by the opening.
pub(crate) fn apply_verb(maps: &mut MapsState, verb: &MapVerb) {
    let scene_me = maps.me.clone();
    let scene = MapScene::new(scene_me.as_ref(), maps.sim_time_seconds, maps.live).with_diagram(maps.diagram);
    if verb.home {
        // A fresh start on you, the dev switches included: the page can stay
        // open from an earlier request (a rig vantage), so the reset on
        // opening never ran (the turning vantage's 20 degrees a second turned
        // the next vantage's camera between its two captures, 2026-10-21).
        maps.view = MapView::System;
        maps.live = true;
        maps.turn_dps = 0.0;
        maps.draw_ship = true;
        maps.layers_on.clear();
        maps.diagram = false;
        maps.camera.open_on_me(scene_me.as_ref());
    }
    if verb.overview {
        // The solar-system overview (Maps increment 2's rig vantage): the
        // Sun's family from ecliptic north, a little tilted so it reads as 3D.
        // A fresh start too: a layer a control vantage switched off is back
        // at its default (the switches below then apply on top).
        maps.view = MapView::System;
        maps.live = true;
        maps.turn_dps = 0.0;
        maps.layers_on.clear();
        maps.diagram = false;
        maps.camera = crate::gui::pages::maps::MapCamera::overview(OVERVIEW_VERB_M);
    }
    if let Some(on) = verb.diagram {
        maps.diagram = on;
    }
    for (id, on) in &verb.layers {
        maps.layers_on.insert(id.clone(), *on);
    }
    match verb.follow {
        Some(true) if !maps.camera.follows_me() => {
            let (d, yaw, pitch) = (maps.camera.distance(), maps.camera.yaw, maps.camera.pitch);
            maps.camera.open_on_me(scene_me.as_ref());
            maps.camera.set_distance(d);
            maps.camera.yaw = yaw;
            maps.camera.pitch = pitch;
        }
        Some(false) => maps.camera.unfollow(&scene),
        _ => {}
    }
    if let Some(d) = verb.distance_m {
        maps.camera.set_distance(d.clamp(MIN_DISTANCE_M, MAX_DISTANCE_M));
    }
    if let Some(ship) = verb.ship {
        maps.draw_ship = ship;
    }
    if verb.report {
        maps.report_after_seq = Some(maps.frame_seq);
    }
    if let Some(t) = verb.turn_dps {
        maps.turn_dps = t;
    }
}

#[cfg(test)]
mod tests {
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
}
