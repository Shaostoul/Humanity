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

/// Whether a style is an operable door (vs a fixed pane like a window). The renderer/interaction
/// layer uses this to decide whether an opening animates on approach at all.
pub fn is_operable(style: &str) -> bool {
    style != "fixed"
}

/// Metres past `open_dist` an OPEN auto-door stays open before closing (v0.540 hysteresis,
/// extracted here v0.795): without the band, standing exactly at the threshold flickers the door
/// open/shut every frame as float noise crosses the line.
pub const AUTO_CLOSE_HYSTERESIS: f32 = 0.8;

/// Target open fraction for an AUTO door: `dist` is the nearest actor's HORIZONTAL distance to
/// the door (eye height must not count -- a tall camera would otherwise never trigger a short
/// door), `open_now` the door's current fraction. Closed doors open inside `open_dist`; open
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
    let ease = 1.0 - (-dt.max(0.0) * 9.0).exp();
    (open + (target - open) * ease).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn ease_open_is_safe_at_the_edges() {
        assert_eq!(ease_open(0.3, 1.0, 0.0), 0.3, "dt = 0 is a no-op");
        assert_eq!(ease_open(0.3, 1.0, -1.0), 0.3, "negative dt never runs the door backwards");
        // A huge frame hitch jumps essentially to the target but stays clamped in 0..1.
        let hitch = ease_open(0.5, 1.0, 10.0);
        assert!(hitch <= 1.0 && hitch > 0.999, "got {hitch}");
    }
}
