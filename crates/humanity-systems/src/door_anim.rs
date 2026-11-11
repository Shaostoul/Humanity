//! Door + window OPENING ANIMATION (v0.535). An opening cut into a wall (see
//! `ship::home_structure::Opening`) stores a data-driven `style` string; this module turns that
//! string + an open-fraction into how the opening's PANEL moves. New door/window kinds plug in as
//! DATA -- a new `style` arm here -- without touching the renderer or the editor. The operator's
//! brief: sliding, rotating, iris, energy walls, nanowalls (the Doom dissolve), organic -- one model
//! that covers them all.
//!
//! Pure math, no GPU: the renderer fills the closed opening with a panel and multiplies it by the
//! returned `PanelMotion`, expressed in the WALL's local frame -- u runs along the wall (corner a to
//! b), v is up, n is the wall normal (thickness). Keeping it GPU-free means it compiles in the
//! headless relay too and is unit-testable without a window.

use std::f32::consts::PI;

/// How a door/window panel is displaced from its CLOSED state at open-fraction `open` (0 = closed,
/// 1 = fully open), in the wall's local frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelMotion {
    /// Local translation (along-wall u, up v, normal n), metres.
    pub offset: (f32, f32, f32),
    /// Rotation about the hinge (the panel's `a`-side vertical edge), radians. Swing + rotate doors.
    pub hinge: f32,
    /// Multiplicative scale (u, v, n). Iris/fold collapse the panel toward nothing.
    pub scale: (f32, f32, f32),
    /// Opacity 0..1. Energy + nanowall fields fade as they open.
    pub alpha: f32,
    /// True once the panel is fully open with nothing left to draw -- the renderer can cull it.
    pub hidden: bool,
}

impl PanelMotion {
    /// The closed panel: no displacement, fully opaque, drawn.
    pub const CLOSED: PanelMotion = PanelMotion {
        offset: (0.0, 0.0, 0.0),
        hinge: 0.0,
        scale: (1.0, 1.0, 1.0),
        alpha: 1.0,
        hidden: false,
    };
}

/// Resolve an opening's `style` + open-fraction into a `PanelMotion`. An unknown style falls back to
/// a swing so a typo still animates plausibly. `width`/`height` are the opening's size in metres,
/// used by styles that translate by the panel's own size (slide, organic).
pub fn panel_motion(style: &str, open: f32, width: f32, height: f32) -> PanelMotion {
    let t = open.clamp(0.0, 1.0);
    let done = t >= 0.999;
    match style {
        // A fixed pane (most windows): never moves.
        "fixed" => PanelMotion::CLOSED,
        // Slides along the wall into a wall pocket.
        "slide" => PanelMotion { offset: (t * width, 0.0, 0.0), hidden: done, ..PanelMotion::CLOSED },
        // Hinged swing, up to a quarter turn.
        "swing" => PanelMotion { hinge: t * (PI * 0.5), ..PanelMotion::CLOSED },
        // Spins about the hinge a half turn (a revolving panel).
        "rotate" => PanelMotion { hinge: t * PI, ..PanelMotion::CLOSED },
        // Iris: the panel scales toward nothing from every side.
        "iris" => {
            let s = 1.0 - t;
            PanelMotion { scale: (s, s, 1.0), hidden: done, ..PanelMotion::CLOSED }
        }
        // Fold (accordion, simplified): squishes flat along the wall.
        "fold" => {
            let s = 1.0 - t;
            PanelMotion { scale: (s, 1.0, 1.0), hidden: done, ..PanelMotion::CLOSED }
        }
        // Energy field: fades out as it opens (passable once faint).
        "energy" => PanelMotion { alpha: 1.0 - t, hidden: done, ..PanelMotion::CLOSED },
        // Nanowall (the Doom dissolve): a thin field that fades + thins toward the wall.
        "nanowall" => PanelMotion {
            alpha: 1.0 - t * 0.9,
            scale: (1.0, 1.0, 1.0 - t),
            hidden: done,
            ..PanelMotion::CLOSED
        },
        // Organic: irises open with a soft upward creep.
        "organic" => {
            let s = 1.0 - t;
            PanelMotion { scale: (s, s, 1.0), offset: (0.0, t * height * 0.15, 0.0), hidden: done, ..PanelMotion::CLOSED }
        }
        // Unknown -> swing, so a typo still moves plausibly.
        _ => PanelMotion { hinge: t * (PI * 0.5), ..PanelMotion::CLOSED },
    }
}

/// One opening style as `data/blueprints/opening_styles.ron` lists it (v0.931, rows since character
/// animation rung 2b, 2026-11-10): its id (what an opening stores and `panel_motion` reads) and the
/// interaction anchors a door of this style offers (ids of `data/characters/interactions.ron`: a
/// swing door's handle a hand closes round while it opens). Infinite-of-x: a style is a row.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct OpeningStyle {
    pub id: String,
    #[serde(default)]
    pub anchors: Vec<String>,
    /// A hand opens a door of this style (None: it opens by itself at its `open_dist`).
    #[serde(default)]
    pub hand: Option<HandOpen>,
}

/// A door a HAND opens (character animation rung 2b's follow-up; the operator, 2026-11-10, asked
/// whether hinged home doors should start swinging when a hand reaches the handle instead of by
/// themselves at 2.6 m, sliding and corridor doors and any door while sprinting keeping 2.6 m:
/// "Sure."). A row of `data/blueprints/opening_styles.ron` carries it.
#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize)]
pub struct HandOpen {
    /// How near a walking person's feet come to the handle, across the floor, when the door starts
    /// to swing (metres): their hand is on the handle then.
    pub reach_m: f32,
    /// How near the door's face a person's feet come, anywhere across its width, when it opens
    /// whether or not the hand found the handle (metres): just short of the body touching it (the
    /// walk's 0.3 m radius and the slab's half thickness), so nobody stands stuck at a shut door.
    pub touch_m: f32,
    /// The push's pace: while a hand opens it the door eases open at this rate (`ease_open_at`, per
    /// second), slower than a door that opens itself (9), so the hand stays on it as they walk on.
    pub swing_rate: f32,
    /// Faster than this across the floor (metres a second) is a sprint: the door opens at its
    /// `open_dist` as one that opens itself, so nobody runs into it shut.
    pub sprint_mps: f32,
}

/// Which way a door's panel is swinging, and why (one per door panel, beside the open fraction):
/// `side` multiplies `panel_motion`'s hinge turn (+1 the way it turns, -1 the other), and
/// `by_hand` says a hand opened it this time (it swings at the push's pace and no longer blocks
/// the walk once it moves, `HandOpen`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Swing {
    pub side: f32,
    pub by_hand: bool,
}

impl Default for Swing {
    fn default() -> Self {
        Swing { side: 1.0, by_hand: false }
    }
}

/// How a door of `style` is opened by hand, if it is (`OpeningStyle::hand`).
pub fn style_hand(style: &str) -> Option<HandOpen> {
    opening_styles().iter().find(|s| s.id == style).and_then(|s| s.hand)
}

/// Where a HAND-opened door is heading this frame (0 shut, 1 open). `near` is the nearest actor
/// of any kind to its middle and `fast` the nearest that cannot open it by hand (a sprinting person,
/// an animal), both across the floor; `hand` the nearest walking person's feet to its handle and
/// `touch` the nearest walking person's feet to its face. Shut, it opens when a hand reaches the
/// handle (`reach_m`), a body is about to touch it (`touch_m`), or something fast comes within
/// `open_dist`; once it moves it stays open while anyone is within `open_dist` and the hysteresis,
/// as a door that opens itself does, so it never swings shut on someone in the doorway.
pub fn hand_door_target(near: f32, hand: f32, touch: f32, fast: f32, open_dist: f32, h: &HandOpen, open_now: f32) -> f32 {
    if open_now > 0.02 {
        if near < open_dist + AUTO_CLOSE_HYSTERESIS { 1.0 } else { 0.0 }
    } else if hand <= h.reach_m || touch <= h.touch_m || fast < open_dist {
        1.0
    } else {
        0.0
    }
}

/// HOW FAR A BODY PUSHES A HINGED PANEL: the least open fraction (0 to 1, a quarter turn) at which the
/// panel, hinged at `hinge` with its closed middle at `center` and swinging `side` (`Swing::side`),
/// keeps every point of it at least `clear` across the floor from `body` (1 when no swing does).
/// A person walking through a door a hand opened pushes it ahead of them, so the panel never
/// passes through them: the door eases open at the push's pace, and never less than this.
pub fn push_open(hinge: glam::Vec3, center: glam::Vec3, side: f32, body: glam::Vec3, clear: f32) -> f32 {
    let flat = |v: glam::Vec3| glam::Vec3::new(v.x, 0.0, v.z);
    let reach = flat(center - hinge) * 2.0;
    let b = flat(body - hinge);
    let off = |open: f32| {
        let edge = glam::Quat::from_rotation_y(open * PI * 0.5 * side) * reach;
        let t = (b.dot(edge) / edge.length_squared().max(1e-8)).clamp(0.0, 1.0);
        (b - edge * t).length()
    };
    if off(0.0) >= clear {
        return 0.0;
    }
    if off(1.0) < clear {
        return 1.0;
    }
    let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
    for _ in 0..14 {
        let mid = 0.5 * (lo + hi);
        if off(mid) >= clear {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    hi
}

/// The way a hinged panel swings AWAY from `opener` (+1: the way `panel_motion`'s hinge angle turns
/// it, -1: the other), from its hinge, its closed middle and the opener, across the floor. A person
/// opening a door by hand stands inside the arc a panel swinging toward them would sweep through
/// them, and the body cannot yet step back to pull it, so the panel goes away from them.
pub fn swing_away_from(hinge: glam::Vec3, center: glam::Vec3, opener: glam::Vec3) -> f32 {
    let e = center - hinge;
    // A small positive turn about +Y moves the panel's middle this way.
    let moves = glam::Vec3::new(e.z, 0.0, -e.x);
    if (opener - center).dot(moves) > 0.0 { -1.0 } else { 1.0 }
}

/// Every opening style, compiled in (the construction editor lists them; the engine reads a door's
/// anchors from its style). A broken file falls back to swing and fixed, logged.
pub fn opening_styles() -> &'static [OpeningStyle] {
    static REG: std::sync::OnceLock<Vec<OpeningStyle>> = std::sync::OnceLock::new();
    REG.get_or_init(|| {
        const SRC: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/blueprints/opening_styles.ron"));
        match ron::from_str::<Vec<OpeningStyle>>(SRC) {
            Ok(v) if !v.is_empty() => v,
            Ok(_) | Err(_) => {
                log::error!("opening_styles.ron missing/invalid; using fallback");
                ["swing", "fixed"].iter().map(|s| OpeningStyle { id: s.to_string(), anchors: Vec::new(), hand: None }).collect()
            }
        }
    })
}

/// The anchors a door of `style` offers (none for a style the file does not list).
pub fn style_anchors(style: &str) -> &'static [String] {
    opening_styles().iter().find(|s| s.id == style).map_or(&[], |s| s.anchors.as_slice())
}

/// Whether a style is an operable door (vs a fixed pane like a window). The renderer/interaction
/// layer uses this to decide whether an opening animates on approach at all.
pub fn is_operable(style: &str) -> bool {
    style != "fixed"
}

/// Metres past `open_dist` an OPEN auto-door stays open before closing (v0.540 hysteresis,
/// extracted here v0.795): without the band, standing exactly at the threshold flickers the door
/// open/shut every frame as float noise crosses the line.
pub const AUTO_CLOSE_HYSTERESIS: f32 = 0.8;


// Whether a body's feet are on a door's storey (`feet_reach_door`) is `ship::door_panels`', the
// ship's door code that also asks it, since the crate split's rung 0.

/// Target open fraction for an AUTO door: `dist` is the nearest actor's HORIZONTAL distance to
/// the door (eye height must not count -- a tall camera would otherwise never trigger a short
/// door; an actor on another storey is not counted at all, `feet_reach_door`), `open_now` the
/// door's current fraction. Closed doors open inside `open_dist`; open
/// doors stay open until the nearest actor backs past `open_dist + AUTO_CLOSE_HYSTERESIS`.
/// Pure (v0.795, extracted from the render loop) so proximity logic is testable without a GPU.
pub fn auto_open_target(dist: f32, open_dist: f32, open_now: f32) -> f32 {
    if open_now > 0.5 {
        if dist < open_dist + AUTO_CLOSE_HYSTERESIS { 1.0 } else { 0.0 }
    } else if dist < open_dist {
        1.0
    } else {
        0.0
    }
}

/// One animation step: ease `open` toward `target` by a frame-rate-independent EXPONENTIAL lerp
/// (v0.540, extracted v0.795). No linear stepping, no snapping: the fraction covers ~87% of the
/// remaining gap every 1/9 s, so a door visually settles in roughly 0.4 s regardless of frame
/// rate. dt is clamped non-negative (a scheduler hiccup must not run the door backwards) and the
/// result to 0..1 (what the slide/iris styles and the collision gate expect).
pub fn ease_open(open: f32, target: f32, dt: f32) -> f32 {
    ease_open_at(open, target, dt, 9.0)
}

/// `ease_open` at another `rate` (per second): a door a hand pushes swings at the push's pace
/// (`HandOpen::swing_rate`).
pub fn ease_open_at(open: f32, target: f32, dt: f32, rate: f32) -> f32 {
    let ease = 1.0 - (-dt.max(0.0) * rate.max(0.0)).exp();
    (open + (target - open) * ease).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    // The storey band is the ship door code's (the crate split's rung 0); its tests stay here.
    use humanity_ship::door_panels::feet_reach_door;

    #[test]
    fn closed_at_zero_for_every_style() {
        for s in ["swing", "slide", "iris", "rotate", "fold", "energy", "nanowall", "organic", "fixed"] {
            let m = panel_motion(s, 0.0, 1.0, 2.1);
            assert_eq!(m.offset, (0.0, 0.0, 0.0), "{s} should not translate when closed");
            assert_eq!(m.hinge, 0.0, "{s} should not rotate when closed");
            assert_eq!(m.scale, (1.0, 1.0, 1.0), "{s} should be full scale when closed");
            assert_eq!(m.alpha, 1.0, "{s} should be opaque when closed");
            assert!(!m.hidden, "{s} should be drawn when closed");
        }
    }

    #[test]
    fn fixed_never_moves() {
        assert_eq!(panel_motion("fixed", 1.0, 1.0, 1.2), PanelMotion::CLOSED);
        assert!(!is_operable("fixed"));
        assert!(is_operable("swing"));
    }

    #[test]
    fn slide_translates_by_width_and_hides() {
        let m = panel_motion("slide", 1.0, 0.9, 2.1);
        assert!((m.offset.0 - 0.9).abs() < 1e-5, "slides one panel width");
        assert!(m.hidden, "fully open slide is culled");
    }

    #[test]
    fn swing_and_rotate_hinge() {
        assert!((panel_motion("swing", 1.0, 1.0, 2.1).hinge - PI * 0.5).abs() < 1e-5);
        assert!((panel_motion("rotate", 1.0, 1.0, 2.1).hinge - PI).abs() < 1e-5);
    }

    #[test]
    fn iris_and_energy_vanish_when_open() {
        let iris = panel_motion("iris", 1.0, 1.0, 2.1);
        assert!(iris.scale.0 < 1e-3 && iris.scale.1 < 1e-3 && iris.hidden);
        let energy = panel_motion("energy", 1.0, 1.0, 2.1);
        assert!(energy.alpha < 1e-3 && energy.hidden);
    }

    #[test]
    fn open_fraction_is_clamped() {
        // open > 1 must not over-rotate / over-translate.
        let over = panel_motion("swing", 5.0, 1.0, 2.1);
        assert!((over.hinge - PI * 0.5).abs() < 1e-5, "clamped to fully open");
        let under = panel_motion("slide", -2.0, 0.9, 2.1);
        assert_eq!(under.offset, (0.0, 0.0, 0.0), "clamped to closed");
    }

    #[test]
    fn unknown_style_defaults_to_swing() {
        let typo = panel_motion("sliiide", 1.0, 1.0, 2.1);
        assert!((typo.hinge - PI * 0.5).abs() < 1e-5, "unknown falls back to a swing");
    }

    #[test]
    fn auto_target_opens_near_closes_far_with_hysteresis() {
        // Closed door: opens strictly inside open_dist, stays shut outside it.
        assert_eq!(auto_open_target(2.0, 3.0, 0.0), 1.0, "near actor opens a closed door");
        assert_eq!(auto_open_target(3.5, 3.0, 0.0), 0.0, "far actor leaves it closed");
        // Open door: the hysteresis band (open_dist..open_dist + 0.8) holds it open, so standing
        // at the threshold does not flicker it; past the band it closes.
        assert_eq!(auto_open_target(3.5, 3.0, 1.0), 1.0, "just past the line, still held open");
        assert_eq!(auto_open_target(3.0 + AUTO_CLOSE_HYSTERESIS + 0.1, 3.0, 1.0), 0.0, "beyond the band, closes");
    }

    #[test]
    fn ease_open_converges_in_about_point_four_seconds_without_snapping() {
        // ~0.4 s of 60 fps steps: monotonic rise (no snap, no overshoot) landing near fully open.
        let mut open = 0.0f32;
        for _ in 0..24 {
            let prev = open;
            open = ease_open(open, 1.0, 1.0 / 60.0);
            assert!(open > prev && open <= 1.0, "each step advances smoothly, got {prev} -> {open}");
        }
        assert!(open > 0.9, "roughly settled after ~0.4 s, got {open}");
        // Closing converges the same way.
        let closed = (0..24).fold(1.0f32, |o, _| ease_open(o, 0.0, 1.0 / 60.0));
        assert!(closed < 0.1, "closes in ~0.4 s too, got {closed}");
    }

    /// FEET REACH A DOOR ON THEIR OWN STOREY ONLY (ship homes increment 7). A door from 3.0 to 5.1:
    /// feet on its floor, on a raised step 0.5 m above it, or 0.4 m below its sill on a stair's last
    /// risers reach it; feet on the floor under it (0) or the floor over it (6) do not.
    /// Seen red 2026-10-20 with the band ignored (`feet_reach_door` returning true): "feet on the
    /// floor under the door reach it".
    #[test]
    fn feet_reach_a_door_on_their_own_storey_only() {
        for (feet, why) in [(3.0, "on its floor"), (3.5, "on a step"), (2.6, "on the stair below its landing")] {
            assert!(feet_reach_door(feet, 3.0, 5.1), "feet {why} do not reach the door");
        }
        assert!(!feet_reach_door(0.0, 3.0, 5.1), "feet on the floor under the door reach it");
        assert!(!feet_reach_door(6.0, 3.0, 5.1), "feet on the floor over the door reach it");
    }

    /// A DOOR OPENS ONLY FOR A BODY ON ITS OWN STOREY, HOWEVER TALL THE DOOR (ship homes increment
    /// 7 review). The homestead's tallest ground doors are 2.4 m, under a slab whose top is 3.0:
    /// feet on that floor (3.0, or 2.9999998 as the walk works it out) do not reach a 2.4 m door at
    /// sill 0, so it stays shut (and silent) for someone walking upstairs; feet on the ground floor
    /// do not reach a door upstairs (sill 3.0), tall or not. A body a stair step under or over a
    /// door's sill does reach it, and so does one in the middle of a jump in its doorway (the feet
    /// 1.27 m up at the top of a jump at 1 g), so the door does not shut on it mid-jump.
    ///
    /// Seen red first (2026-10-21) with the band reaching `DOOR_STOREY_TOLERANCE_M` over the
    /// door's head: "feet on the floor over a 2.4 m ground door (3) reach it".
    #[test]
    fn a_door_opens_only_for_a_body_on_its_own_storey_however_tall() {
        for feet in [3.0, 2.999_999_8, 3.5] {
            assert!(!feet_reach_door(feet, 0.0, 2.4), "feet on the floor over a 2.4 m ground door ({feet}) reach it");
        }
        for (sill, top) in [(3.0, 5.1), (3.0, 5.4), (3.0, 5.75)] {
            assert!(!feet_reach_door(0.0, sill, top), "feet on the ground floor reach the door upstairs from {sill} to {top}");
        }
        for (feet, sill, top, why) in [
            (-0.5, 0.0, 2.4, "a step under a ground door's sill"),
            (0.5, 0.0, 2.4, "a step over a ground door's sill"),
            (2.5, 3.0, 5.1, "on the stair a step under an upstairs door's sill"),
            (3.5, 3.0, 5.1, "a step over an upstairs door's sill"),
            (1.27, 0.0, 2.1, "at the top of a jump in a 2.1 m doorway"),
        ] {
            assert!(feet_reach_door(feet, sill, top), "feet {why} ({feet}, the door {sill} to {top}) do not reach it");
        }
    }

    /// A HAND OPENS A HINGED DOOR (the operator, 2026-11-10: "Sure."): shut, it stays shut for a
    /// walking person until their feet are within reach of the handle (0.40 m) or about to touch its
    /// face (0.42 m), whatever their distance to its middle; something fast (a sprint, an animal)
    /// opens it at its 2.6 m as before; once moving it stays open while anyone is within 3.4 m.
    /// Seen red 2026-11-10 with the walker's distance to the middle opening it as before (`near <
    /// open_dist` in place of the hand's reach): "a walker 1.5 m off opened the door by hand" (left: 1.0).
    #[test]
    fn a_hand_opens_a_hinged_door_and_a_sprint_still_opens_it_at_its_distance() {
        let h = style_hand("swing").expect("the swing row says a hand opens it");
        assert_eq!((h.reach_m, h.touch_m), (0.40, 0.42));
        assert!(style_hand("slide").is_none(), "a sliding door opens itself");
        let far = f32::MAX;
        assert_eq!(hand_door_target(1.5, 1.6, 1.5, far, 2.6, &h, 0.0), 0.0, "a walker 1.5 m off opened the door by hand");
        assert_eq!(hand_door_target(0.6, 0.39, 0.5, far, 2.6, &h, 0.0), 1.0, "a hand on the handle did not open it");
        assert_eq!(hand_door_target(0.6, 0.9, 0.4, far, 2.6, &h, 0.0), 1.0, "a body against the face did not open it");
        assert_eq!(hand_door_target(2.0, far, far, 2.0, 2.6, &h, 0.0), 1.0, "a sprint at 2 m did not open it");
        assert_eq!(hand_door_target(3.0, far, far, 3.0, 2.6, &h, 0.0), 0.0);
        assert_eq!(hand_door_target(3.0, 2.9, 2.9, far, 2.6, &h, 0.1), 1.0, "a door swinging open shut on someone 3 m off");
        assert_eq!(hand_door_target(3.5, far, far, far, 2.6, &h, 0.8), 0.0, "an open door nobody is near stayed open");
        assert!(ease_open_at(0.0, 1.0, 0.1, h.swing_rate) < ease_open(0.0, 1.0, 0.1), "the push swung as fast as a door that opens itself");
    }

    /// THE PANEL SWINGS AWAY FROM WHOEVER OPENS IT: from either side, the swing's first turn moves
    /// the panel's middle further from the opener.
    /// Seen red 2026-11-10 with the side's sign flipped: "opened from Vec3(0.45, 0.0, 1.0) the panel
    /// swung toward them".
    #[test]
    fn the_panel_swings_away_from_whoever_opens_it() {
        let (hinge, center) = (glam::Vec3::ZERO, glam::Vec3::new(0.45, 0.0, 0.0));
        for opener in [glam::Vec3::new(0.45, 0.0, 1.0), glam::Vec3::new(0.45, 0.0, -1.0), glam::Vec3::new(-0.3, 0.0, 0.6)] {
            let side = swing_away_from(hinge, center, opener);
            let turned = hinge + glam::Quat::from_rotation_y(side * 0.3) * (center - hinge);
            assert!((turned - opener).length() > (center - opener).length(), "opened from {opener:?} the panel swung toward them");
        }
    }

    /// A BODY WALKING THROUGH PUSHES THE PANEL AHEAD OF IT: a person walking straight through a
    /// 0.9 m doorway 0.15 m off its middle toward the handle, from 0.6 m before it to 0.6 m past,
    /// with the panel at `push_open` never has any of the panel within 0.28 m of them; a body
    /// clear of the door asks for nothing.
    /// Seen red 2026-11-10 with the swing's side left out (`* side` dropped, the panel turned the
    /// other way): "at 0.05 m past the doorway the panel came 0.188 m from the body".
    #[test]
    fn a_body_walking_through_pushes_the_panel_ahead_of_it() {
        let (hinge, center) = (glam::Vec3::ZERO, glam::Vec3::new(0.45, 0.0, 0.0));
        // The opener comes from -z, so the panel swings toward +z.
        let side = swing_away_from(hinge, center, glam::Vec3::new(0.6, 0.0, -1.0));
        let mut z = -0.6_f32;
        while z <= 0.6 {
            let body = glam::Vec3::new(0.6, 0.0, z);
            let open = push_open(hinge, center, side, body, 0.28);
            let edge = glam::Quat::from_rotation_y(open * PI * 0.5 * side) * (center * 2.0);
            let t = (body.dot(edge) / edge.length_squared()).clamp(0.0, 1.0);
            let gap = (body - edge * t).length();
            assert!(gap >= 0.28 - 1e-3, "at {z:.2} m past the doorway the panel came {gap:.3} m from the body");
            z += 0.05;
        }
        assert_eq!(push_open(hinge, center, side, glam::Vec3::new(0.6, 0.0, -1.0), 0.28), 0.0, "a body a metre off pushed the door");
    }

    #[test]
    fn ease_open_is_safe_at_the_edges() {
        assert_eq!(ease_open(0.3, 1.0, 0.0), 0.3, "dt = 0 is a no-op");
        assert_eq!(ease_open(0.3, 1.0, -1.0), 0.3, "negative dt never runs the door backwards");
        // A huge frame hitch jumps essentially to the target but stays clamped in 0..1.
        let hitch = ease_open(0.5, 1.0, 10.0);
        assert!(hitch <= 1.0 && hitch > 0.999, "got {hitch}");
    }
}
