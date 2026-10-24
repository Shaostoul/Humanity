//! The map view's tests (Maps increment 1; moved out of map_view.rs in increment 3 to keep its line
//! budget). Each was seen red first; what broke it and what it said is in its comment.

use super::*;
use crate::gui::pages::maps::camera::{MapCamera, MapProjector};

/// A point on the hull lands where the symbols say it is: the renderer
/// camera built from the map's pose projects a ship-frame point (the
/// render frame while riding) onto the same canvas pixel the page's f64
/// projector puts its world position, within half a pixel, at the
/// opening framing and turned and zoomed round, at four hours of the day.
///
/// Seen red 2026-10-21 with `render_camera` leaving out `station::to_hull`
/// (the world's axes, not the hull's): "hour 0, view 0: the hull corner is
/// 24.2 px from where the symbols put it".
#[test]
fn a_point_on_the_hull_lands_where_the_symbols_say() {
    let rect = egui::Rect::from_min_size(egui::pos2(260.0, 60.0), egui::vec2(1400.0, 900.0));
    for hour in [0.0, 5.0, 11.0, 19.5] {
        let me = crate::gui::pages::maps::camera::tests::feed_at(hour * 3600.0, DVec3::ZERO);
        let scene = MapScene::new(Some(&me), me.sky_seconds, true);
        for view in 0..3 {
            let mut cam = MapCamera::default();
            cam.open_on_me(Some(&me));
            if view >= 1 {
                cam.rotate_by(1.3, 0.4, &scene);
            }
            if view == 2 {
                cam.set_distance(320.0);
            }
            let pose = cam.pose(&scene);
            let proj = MapProjector::new(pose, rect);
            let rcam = render_camera(&pose, &me, rect.width() / rect.height());
            let vp = rcam.view_projection_matrix();
            let (lo, hi) = me.ship_box_m.expect("the feed carries the ship's box");
            let corners = (0..8).map(|i| DVec3::new(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z }));
            let mut checked = 0;
            for local in corners.chain([me.you_ship_m]) {
                // Only what is drawn: a point tens of thousands of pixels off the
                // canvas (one a few metres in front of the eye at 320 m) carries
                // f32 noise magnified by its angle, and nobody sees it.
                let Some((want, _)) = proj.project(me.ship_to_world(local)) else { continue };
                if !rect.expand(rect.height()).contains(want) {
                    continue;
                }
                checked += 1;
                let clip = vp * local.as_vec3().extend(1.0);
                let ndc = clip.truncate() / clip.w;
                let got = egui::pos2(
                    rect.left() + (ndc.x + 1.0) * 0.5 * rect.width(),
                    rect.top() + (1.0 - ndc.y) * 0.5 * rect.height(),
                );
                let off = (got - want).length();
                assert!(off < 0.5, "hour {hour}, view {view}: the hull corner is {off:.1} px from where the symbols put it");
            }
            assert!(checked >= 3, "hour {hour}, view {view}: only {checked} hull points were on the canvas to compare");
        }
    }
}
