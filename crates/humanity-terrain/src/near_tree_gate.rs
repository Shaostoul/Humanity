//! WHEN the near-field tree harvest re-runs (the `[NearTree] recompute` gate
//! in the frame loop), as one pure function so the rule can be read and
//! tested without a planet.
//!
//! The harvest (`planet_chunks::near_tree_instances_at_density`) walks the
//! vegetation stream around the camera and re-anchors every photoscanned
//! tree to the DRAWN ground mesh. It is cheap but not free (it is the
//! `cpu.near_tree_harvest` stage on the Performance page), and it also
//! restarts the fade-in clock for newcomers, so running it when nothing has
//! changed is pure waste. Three things can make it necessary:
//!
//! * the camera MOVED far enough that the set is no longer centred on the
//!   player (12 m: the v0.995 hysteresis, chosen so trees ahead of a walking
//!   player are already there);
//! * the drawn terrain LOD under the camera CHANGED, so the trees' bases sit
//!   on the previous mesh (the v0.1097 floating-trees fix);
//! * the forest DENSITY slider moved, so the card bake and the models would
//!   disagree until the next 12 m of walking (v0.1111).
//!
//! The 2026-09-18 measurement at the operator's settings found the log line
//! firing EVERY FRAME while the camera was parked. Movement cannot be the
//! trigger when parked, so one of the two change terms was oscillating from
//! frame to frame: the drawn depth is the max over `last_drawn`, which is
//! reassigned every frame from the selection, and the same measurement saw
//! the patch build budget saturated for the whole park, so patches kept
//! landing and leaving the drawn set. Whichever term it is, the fix is the
//! same shape: movement stays EAGER (a walking player must not wait), while
//! the two change-driven terms are RATE LIMITED to one re-harvest per
//! [`NEAR_TREE_CHANGE_MIN_INTERVAL_S`]. A genuine LOD change still
//! re-anchors the trees within a quarter second; an input that flips every
//! frame costs four harvests a second instead of sixty. The reason is
//! returned so the (debug-level) log names it and the next boot answers
//! which term it was.

/// Camera movement, in metres of planet-local displacement since the last
/// harvest, past which the set is re-harvested at once.
pub const NEAR_TREE_REHARVEST_M: f64 = 12.0;

/// The least time between two harvests triggered by a CHANGE (drawn depth or
/// density) rather than by movement. Long enough that an input flipping
/// every frame cannot re-run the harvest every frame; short enough that a
/// real LOD change under a standing player is corrected before it is
/// noticed.
pub const NEAR_TREE_CHANGE_MIN_INTERVAL_S: f32 = 0.25;

/// Why the harvest re-ran, for the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NearTreeRecompute {
    /// The camera moved more than [`NEAR_TREE_REHARVEST_M`].
    Moved,
    /// The drawn terrain LOD under the camera changed.
    DepthChanged,
    /// The harvest density (the slider, or a drawn patch's own bake density)
    /// changed.
    DensityChanged,
}

impl NearTreeRecompute {
    /// The word that goes in the log line.
    pub fn label(self) -> &'static str {
        match self {
            NearTreeRecompute::Moved => "moved",
            NearTreeRecompute::DepthChanged => "depth changed",
            NearTreeRecompute::DensityChanged => "density changed",
        }
    }
}

/// Decide whether the harvest re-runs this frame, and why.
///
/// * `moved_m`: planet-local camera displacement since the last harvest
///   (`f64::MAX`-ish before the first one, which reads as "moved").
/// * `drawn_depth` / `last_depth`: the max drawn patch depth now and at the
///   last harvest.
/// * `density` / `last_density`: the harvest density now and at the last
///   harvest. Compared with a tolerance rather than `!=` so a NaN can never
///   compare unequal to itself every frame.
/// * `since_last_s`: seconds since the last harvest.
pub fn near_tree_recompute_reason(
    moved_m: f64,
    drawn_depth: u8,
    last_depth: u8,
    density: f32,
    last_density: f32,
    since_last_s: f32,
) -> Option<NearTreeRecompute> {
    if moved_m > NEAR_TREE_REHARVEST_M {
        return Some(NearTreeRecompute::Moved);
    }
    // A change-driven re-harvest waits out the interval; a NaN interval
    // (should never happen) counts as "not yet", never as "due".
    let change_due = since_last_s >= NEAR_TREE_CHANGE_MIN_INTERVAL_S;
    if !change_due {
        return None;
    }
    if drawn_depth != last_depth {
        return Some(NearTreeRecompute::DepthChanged);
    }
    if (density - last_density).abs() > 1.0e-6 {
        return Some(NearTreeRecompute::DensityChanged);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A parked camera with nothing changed never re-harvests, however long
    /// it stands there: this is the per-frame log flood the gate exists to
    /// stop.
    #[test]
    fn parked_and_unchanged_never_recomputes() {
        for since in [0.0_f32, 0.1, 1.0, 60.0] {
            assert_eq!(near_tree_recompute_reason(0.0, 9, 9, 0.5, 0.5, since), None);
        }
    }

    /// Movement past the hysteresis is eager: it does not wait for the
    /// change interval (a walking player must not see trees lag).
    #[test]
    fn movement_recomputes_at_once() {
        assert_eq!(
            near_tree_recompute_reason(NEAR_TREE_REHARVEST_M + 0.01, 9, 9, 0.5, 0.5, 0.0),
            Some(NearTreeRecompute::Moved)
        );
        // Exactly at the threshold is "not yet" (strict), the same as before.
        assert_eq!(near_tree_recompute_reason(NEAR_TREE_REHARVEST_M, 9, 9, 0.5, 0.5, 0.0), None);
        // The first harvest ever: the centre is at f64::MAX, so "moved".
        assert_eq!(
            near_tree_recompute_reason(f64::MAX, 0, 0, 0.5, 0.5, 0.0),
            Some(NearTreeRecompute::Moved)
        );
    }

    /// A drawn-depth change re-harvests, but only once the interval has
    /// passed: an input that flips every frame is bounded to 1/interval
    /// harvests a second instead of one per frame.
    #[test]
    fn depth_change_is_rate_limited() {
        let just_after = NEAR_TREE_CHANGE_MIN_INTERVAL_S;
        let too_soon = NEAR_TREE_CHANGE_MIN_INTERVAL_S - 0.01;
        assert_eq!(near_tree_recompute_reason(0.0, 10, 9, 0.5, 0.5, too_soon), None);
        assert_eq!(
            near_tree_recompute_reason(0.0, 10, 9, 0.5, 0.5, just_after),
            Some(NearTreeRecompute::DepthChanged)
        );
    }

    /// Same rule for the density term, and a NaN density can never loop:
    /// NaN != NaN is true, which is exactly the every-frame trap a plain
    /// `!=` would have.
    #[test]
    fn density_change_is_rate_limited_and_nan_safe() {
        assert_eq!(near_tree_recompute_reason(0.0, 9, 9, 0.7, 0.5, 0.1), None);
        assert_eq!(
            near_tree_recompute_reason(0.0, 9, 9, 0.7, 0.5, 0.3),
            Some(NearTreeRecompute::DensityChanged)
        );
        assert_eq!(near_tree_recompute_reason(0.0, 9, 9, f32::NAN, f32::NAN, 5.0), None);
    }

    /// Depth wins over density when both changed (either is a full
    /// re-harvest; the label just says which was noticed first).
    #[test]
    fn labels_name_the_reason() {
        assert_eq!(NearTreeRecompute::Moved.label(), "moved");
        assert_eq!(NearTreeRecompute::DepthChanged.label(), "depth changed");
        assert_eq!(NearTreeRecompute::DensityChanged.label(), "density changed");
        assert_eq!(
            near_tree_recompute_reason(0.0, 10, 9, 0.7, 0.5, 1.0),
            Some(NearTreeRecompute::DepthChanged)
        );
    }
}
