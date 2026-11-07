//! Aim assist over a real screen: a `ScreenCore` (the GPU-free half of a wall screen, the same
//! egui context and synthetic-event API the look ray drives) showing two buttons, Play and Stop,
//! 100 pixels apart on a 1.6 x 0.9 m picture seen from 5 m. Every check reads egui's own widget
//! list ([`targets`]) and clicks through the core's own event API, so "the assist picked Play" is
//! proven by Play reporting a click, not by the setup.

use super::*;
use crate::gui::screen_surface::ScreenCore;
use crate::gui::theme::load_theme;
use crate::gui::GuiState;

const PX: (u32, u32) = (1280, 720);
const PLAY: Rect = Rect { min: egui::pos2(400.0, 330.0), max: egui::pos2(480.0, 390.0) };
const STOP: Rect = Rect { min: egui::pos2(580.0, 330.0), max: egui::pos2(660.0, 390.0) };
const OFF: Rect = Rect { min: egui::pos2(100.0, 100.0), max: egui::pos2(180.0, 160.0) };

/// The picture: 1.6 m wide, 0.9 m high, facing +Z, its top-left corner at (-0.8, 1.9, 0).
fn quad() -> QuadGeom {
    QuadGeom { origin: Vec3::new(-0.8, 1.9, 0.0), u_axis: Vec3::new(1.6, 0.0, 0.0), v_axis: Vec3::new(0.0, -0.9, 0.0), normal: Vec3::Z }
}

const EYE: Vec3 = Vec3::new(0.0, 1.45, 5.0);

/// The look direction from the eye to surface pixel `p`.
fn look_at(p: Pos2) -> Vec3 {
    (px_to_world(&quad(), PX, p) - EYE).normalize()
}

/// What one run of the page draws: Play and Stop (enabled), Off (disabled) and a selectable label
/// (which senses a click, for selecting its words, and must not be offered). Returns
/// the names of the buttons that reported a click this run.
fn draw(ctx: &egui::Context) -> Vec<&'static str> {
    let mut clicked = Vec::new();
    egui::CentralPanel::default().show(ctx, |ui| {
        if ui.put(PLAY, egui::Button::new("Play")).clicked() {
            clicked.push("play");
        }
        if ui.put(STOP, egui::Button::new("Stop")).clicked() {
            clicked.push("stop");
        }
        ui.add_enabled_ui(false, |ui| {
            if ui.put(OFF, egui::Button::new("Off")).clicked() {
                clicked.push("off");
            }
        });
        ui.put(Rect::from_min_size(egui::pos2(700.0, 500.0), egui::vec2(200.0, 40.0)), egui::Label::new("Now playing").selectable(true));
    });
    clicked
}

/// A core showing the scene, run twice so its widgets are recorded and settled.
fn scene() -> (ScreenCore, GuiState, Vec<&'static str>) {
    let theme = load_theme();
    let mut state = GuiState::default();
    let mut core = ScreenCore::new("aim_test", "tasks", PX.0, PX.1, &theme);
    let mut clicks = Vec::new();
    for _ in 0..2 {
        core.run_with(&mut state, |ctx, _| clicks.extend(draw(ctx)));
    }
    (core, state, clicks)
}

/// Click at surface pixel `p` the way a wall screen is clicked: hover, press, release, a run each.
fn click_at(core: &mut ScreenCore, state: &mut GuiState, p: Pos2) -> Vec<&'static str> {
    let uv = (p.x / PX.0 as f32, p.y / PX.1 as f32);
    let mut clicks = Vec::new();
    core.pointer_moved(uv);
    core.run_with(state, |ctx, _| clicks.extend(draw(ctx)));
    core.button(uv, true);
    core.run_with(state, |ctx, _| clicks.extend(draw(ctx)));
    core.button(uv, false);
    core.run_with(state, |ctx, _| clicks.extend(draw(ctx)));
    clicks
}

/// The id of the target whose rectangle is `r` (egui's own ids, looked up by where it drew).
fn id_of(targets: &[Target], r: Rect) -> Id {
    targets.iter().find(|t| t.rect == r).unwrap_or_else(|| panic!("no target at {r:?}: {targets:?}")).id
}

/// The nearest clickable widget within the cone is chosen, the pointer snaps onto it, and a click
/// there clicks IT (Play reports the click, Stop does not). The disabled button and the label are
/// not targets. The crosshair 20 px right of Play (0.29 degrees from it at 5 m) and 80 px left of
/// Stop picks Play; ON Stop picks Stop at once, whatever was chosen before.
///
/// Seen red 2026-11-06 (one run, three breaks in `pick`: the farthest target instead of the
/// nearest, the cone ignored, the hysteresis ignored): "the nearest widget is chosen" failed with
/// Stop's id where Play's was expected.
#[test]
fn the_nearest_widget_in_the_cone_is_chosen_and_clicked() {
    let (mut core, mut state, _) = scene();
    let tuning = AimTuning::default();
    let found = targets(core.ctx());
    assert_eq!(found.len(), 2, "Play and Stop, not the disabled button or the label: {found:?}");
    let (play, stop) = (id_of(&found, PLAY), id_of(&found, STOP));

    let aim = egui::pos2(500.0, 360.0);
    let (raw, t) = plane_px(EYE, look_at(aim), &quad(), PX).expect("the ray meets the screen");
    assert!((raw - aim).length() < 0.05 && (t - 5.0).abs() < 0.1, "the plane point is the aimed pixel: {raw:?} at {t} m");
    let p = pick(EYE, look_at(aim), &quad(), PX, raw, &found, None, tuning.cone_deg_default, &tuning);
    assert_eq!(p.target.map(|t| t.id), Some(play), "the nearest widget is chosen");
    assert!(p.snapped && PLAY.contains(p.pointer), "the pointer snaps inside Play: {:?}", p.pointer);
    assert!((p.pointer.x - (PLAY.max.x - tuning.inset_px)).abs() < 1e-3, "at its point nearest the crosshair, inset");
    assert!(p.angle_deg > 0.1 && p.angle_deg < 0.5, "20 px at 5 m is about a third of a degree: {}", p.angle_deg);

    let clicks = click_at(&mut core, &mut state, p.pointer);
    assert_eq!(clicks, vec!["play"], "the click went to Play and only Play");

    // The crosshair ON Stop: Stop at once, even with Play chosen last frame, pointer unmoved.
    let on_stop = egui::pos2(600.0, 350.0);
    let (raw, _) = plane_px(EYE, look_at(on_stop), &quad(), PX).unwrap();
    let p = pick(EYE, look_at(on_stop), &quad(), PX, raw, &found, Some(play), tuning.cone_deg_default, &tuning);
    assert_eq!(p.target.map(|t| t.id), Some(stop), "a widget under the crosshair wins at once");
    assert!(!p.snapped && (p.pointer - raw).length() < 1e-4, "and the pointer stays on the crosshair");
}

/// Nothing outside the cone is chosen: the crosshair on an empty part of the screen, 6.9 degrees
/// from the nearest button, picks nothing and a click there clicks nothing; a ray that misses the
/// picture far off picks nothing; and a smaller cone refuses what a larger one takes.
///
/// Seen red 2026-11-06 (the same run as the test above, the cone ignored): "nothing outside the
/// cone" failed with Play chosen from 6.9 degrees away.
#[test]
fn no_widget_outside_the_cone_is_chosen() {
    let (mut core, mut state, _) = scene();
    let tuning = AimTuning::default();
    let found = targets(core.ctx());
    let far = egui::pos2(1100.0, 600.0);
    let (raw, _) = plane_px(EYE, look_at(far), &quad(), PX).unwrap();
    let p = pick(EYE, look_at(far), &quad(), PX, raw, &found, None, tuning.cone_deg_default, &tuning);
    assert!(p.target.is_none(), "nothing outside the cone: {:?}", p.target);
    assert!(!p.snapped && (p.pointer - raw).length() < 1e-4, "the pointer stays where the crosshair is");
    assert!(click_at(&mut core, &mut state, p.pointer).is_empty(), "a click on empty screen clicks nothing");

    // Off the picture entirely (the plane 840 px right of Stop): nothing.
    let off = egui::pos2(1500.0, 360.0);
    let (raw, _) = plane_px(EYE, look_at(off), &quad(), PX).unwrap();
    assert!(!on_picture(raw, PX));
    assert!(pick(EYE, look_at(off), &quad(), PX, raw, &found, None, tuning.cone_deg_default, &tuning).target.is_none());

    // 50 px right of Play (0.72 degrees): a 1-degree cone takes it, a half-degree one does not.
    let near = egui::pos2(530.0, 360.0);
    let (raw, _) = plane_px(EYE, look_at(near), &quad(), PX).unwrap();
    assert!(pick(EYE, look_at(near), &quad(), PX, raw, &found, None, 1.0, &tuning).target.is_some());
    assert!(pick(EYE, look_at(near), &quad(), PX, raw, &found, None, 0.5, &tuning).target.is_none());
    // A miss just off the picture's edge still reaches a widget near it: the picture is within the cone.
    assert!(picture_angle_deg(EYE, look_at(egui::pos2(1290.0, 360.0)), &quad(), PX, egui::pos2(1290.0, 360.0)) < 0.2);
}

/// No flicker at the boundary: the crosshair wobbling 4 px either side of the exact middle between
/// Play and Stop keeps whichever was chosen, while the same wobble WITHOUT the hysteresis
/// (`switch_ratio` 1, `exit_scale` 1, the positive control) swaps every frame. Sweeping from Play
/// to Stop and back switches once each way, each switch past the middle, so the choice lags the
/// crosshair by a dead band instead of swapping on a line.
///
/// Seen red 2026-11-06 (the same run, the hysteresis ignored): "the wobble keeps its choice"
/// failed with the choice swapping on every one of the 12 frames.
#[test]
fn the_choice_does_not_flicker_at_the_boundary() {
    let (core, _state, _) = scene();
    let tuning = AimTuning::default();
    let found = targets(core.ctx());
    let (play, stop) = (id_of(&found, PLAY), id_of(&found, STOP));
    let middle = (PLAY.max.x + STOP.min.x) * 0.5;
    let run = |xs: &[f32], start: Option<Id>, t: &AimTuning| -> Vec<Option<Id>> {
        let mut current = start;
        xs.iter()
            .map(|&x| {
                let aim = egui::pos2(x, 360.0);
                let (raw, _) = plane_px(EYE, look_at(aim), &quad(), PX).unwrap();
                current = pick(EYE, look_at(aim), &quad(), PX, raw, &found, current, t.cone_deg_default, t).target.map(|t| t.id);
                current
            })
            .collect()
    };
    let wobble: Vec<f32> = (0..12).map(|i| if i % 2 == 0 { middle - 4.0 } else { middle + 4.0 }).collect();
    let held = run(&wobble, Some(play), &tuning);
    assert!(held.iter().all(|c| *c == Some(play)), "the wobble keeps its choice: {held:?}");
    let bare = AimTuning { switch_ratio: 1.0, exit_scale: 1.0, ..tuning.clone() };
    let flicker = run(&wobble, Some(play), &bare);
    let swaps = flicker.windows(2).filter(|w| w[0] != w[1]).count();
    assert!(swaps >= 10, "without the hysteresis the same wobble swaps every frame ({swaps} swaps): the control");

    // Sweep Play -> Stop -> Play in 2 px steps between the two buttons' facing edges.
    let there: Vec<f32> = (0..=90).map(|i| PLAY.max.x + 2.0 * i as f32).collect();
    let back: Vec<f32> = there.iter().rev().copied().collect();
    let out = run(&there, None, &tuning);
    let first_stop = out.iter().position(|c| *c == Some(stop)).expect("it reaches Stop");
    assert!(there[first_stop] > middle + 5.0, "the switch to Stop comes past the middle: at {}", there[first_stop]);
    assert_eq!(out.windows(2).filter(|w| w[0] != w[1]).count(), 1, "one switch on the way there");
    let out = run(&back, Some(stop), &tuning);
    let first_play = out.iter().position(|c| *c == Some(play)).expect("it comes back to Play");
    assert!(back[first_play] < middle - 5.0, "the switch back comes past the middle the other way: at {}", back[first_play]);
    assert_eq!(out.windows(2).filter(|w| w[0] != w[1]).count(), 1, "one switch on the way back");
}

/// A held press freezes the target: the pointer moves only by the look ray's movement since the
/// press, so a few pixels of jitter keep a click a click, and the pointer stays on the surface.
///
/// Seen red 2026-11-06 with `pointer_for` ignoring the ray's movement: the pointer stayed at
/// (476, 360) where (478, 361) was expected, so a real drag (a slider) would not move.
#[test]
fn a_held_press_moves_by_the_rays_movement_only() {
    let h = PressHold { surface: 0, pointer: egui::pos2(476.0, 360.0), raw: egui::pos2(500.0, 360.0) };
    assert_eq!(h.pointer_for(Some(egui::pos2(502.0, 361.0)), PX), egui::pos2(478.0, 361.0));
    assert_eq!(h.pointer_for(None, PX), h.pointer, "a ray off the plane leaves it where the press landed");
    assert_eq!(h.pointer_for(Some(egui::pos2(-5000.0, 360.0)), PX).x, 0.0, "kept on the surface");
}

/// The reach grows with the screen: the old 3.5 m for the shipped 79-inch wall screen (its
/// 1.74 x 0.99 m picture), 1.75 m per metre of diagonal for a bigger one, never past the cap. The
/// defaults are the shipped data file's (`the_shipped_screens_file_...` reads that file).
///
/// Seen red 2026-11-06 with the diagonal term zeroed: "a 200-inch 32:9 TV" reached only 3.5 m.
#[test]
fn reach_grows_with_the_screens_diagonal() {
    let t = AimTuning::default();
    let q = |w: f32, h: f32| QuadGeom { origin: Vec3::ZERO, u_axis: Vec3::new(w, 0.0, 0.0), v_axis: Vec3::new(0.0, -h, 0.0), normal: Vec3::Z };
    assert!((reach_m(&q(1.74, 0.99), &t) - 3.5).abs() < 0.01, "the shipped wall screen reaches as it always did");
    assert_eq!(reach_m(&q(1.16, 0.66), &t), 3.5, "a smaller screen still reaches the floor");
    let two_hundred = reach_m(&q(4.887, 1.374), &t);
    assert!((two_hundred - 8.88).abs() < 0.05, "a 200-inch 32:9 TV: {two_hundred}");
    assert_eq!(reach_m(&q(30.0, 17.0), &t), 25.0, "never past the cap");
}
