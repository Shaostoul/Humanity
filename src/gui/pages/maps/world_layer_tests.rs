//! The world layer's tests (Maps increment 2; moved out of world_layer.rs in increment 3 to keep
//! its line budget). Each was seen red first; what broke it and what it said is in its comment.

use super::*;
use crate::gui::pages::maps::camera::MapCamera;

/// The world layer's bands (Maps increment 2): aboard and live, the whole
/// world from the ship out to the edge of near space; beyond it, on the
/// Diagram, off Live or away from the ship, the star sky alone; nothing
/// with no feed or before the world is loaded.
///
/// Seen red 2026-10-21 with `draws_sky` left false for the solar-system
/// band: "11 AU out the world layer draws no star sky (TooFar)".
#[test]
fn the_whole_world_to_near_space_the_star_sky_beyond() {
    let me = crate::gui::pages::maps::camera::tests::feed_at(3600.0, glam::DVec3::ZERO);
    let mut maps = MapsState::default();
    maps.me = Some(me.clone());
    maps.sim_time_seconds = me.sky_seconds;
    maps.camera.open_on_me(Some(&me));
    assert_eq!(world_layer_want(&maps), WorldLayerWant::Draw, "at the ship");
    maps.camera.set_distance(1.0e9);
    assert_eq!(world_layer_want(&maps), WorldLayerWant::Draw, "a million km out is near space: the whole world");
    maps.camera = MapCamera::overview(11.0 * crate::cosmos::M_PER_AU);
    let far = world_layer_want(&maps);
    assert!(far == WorldLayerWant::TooFar && far.draws_sky(), "11 AU out the world layer draws no star sky ({far:?})");
    maps.camera.open_on_me(Some(&me));
    maps.diagram = true;
    assert!(world_layer_want(&maps).draws_sky(), "the Diagram keeps the star sky");
    maps.diagram = false;
    maps.live = false;
    assert!(world_layer_want(&maps).draws_sky(), "off Live keeps the star sky");
    maps.me.as_mut().unwrap().world_loaded = false;
    assert!(!world_layer_want(&maps).draws_sky(), "no world, no sky");
    maps.me = None;
    assert_eq!(world_layer_want(&maps), WorldLayerWant::NoFeed);
}
