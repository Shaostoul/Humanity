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
    /// The harvest's REACH changed: the tree model distance slider (how far
    /// out it walks) or the near-tree budget (how many trees it keeps). Until
    /// 2026-11-13 neither was in the gate, so a slider move showed nothing
    /// until the player walked 12 m.
    ReachChanged,
    /// The reuse switch is off (`humanity_core::reuse::enabled()`, the Dev
    /// page): the set is harvested again every frame instead of kept.
    /// reuse: site: with the switch off the near-tree set is harvested every frame; with it on the
    /// gate also watches the tree model distance and the near-tree budget.
    ReuseOff,
}

impl NearTreeRecompute {
    /// The word that goes in the log line.
    pub fn label(self) -> &'static str {
        match self {
            NearTreeRecompute::Moved => "moved",
            NearTreeRecompute::DepthChanged => "depth changed",
            NearTreeRecompute::DensityChanged => "density changed",
            NearTreeRecompute::ReachChanged => "reach changed",
            NearTreeRecompute::ReuseOff => "reuse off",
        }
    }
}

/// What a harvest was made from, kept beside the set it made so the next
/// frame can tell whether any of it changed.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct HarvestInputs {
    /// The max drawn patch depth (the bases sit on that mesh).
    pub depth: u8,
    /// The harvest density. Compared with a tolerance rather than `!=` so a
    /// NaN can never compare unequal to itself every frame.
    pub density: f32,
    /// How far out the harvest walks, metres (the tree model distance slider
    /// plus the fade margin).
    pub reach_m: f64,
    /// The most trees it keeps (from the near-tree budget slider).
    pub cap: usize,
}

/// Decide whether the harvest re-runs this frame, and why.
///
/// * `moved_m`: planet-local camera displacement since the last harvest
///   (`f64::MAX`-ish before the first one, which reads as "moved").
/// * `now` / `last`: what a harvest would be made from now, and what the
///   kept one was made from.
/// * `since_last_s`: seconds since the last harvest.
/// * `reuse`: `humanity_core::reuse::enabled()`; false harvests every frame.
pub fn near_tree_recompute_reason(
    moved_m: f64,
    now: HarvestInputs,
    last: HarvestInputs,
    since_last_s: f32,
    reuse: bool,
) -> Option<NearTreeRecompute> {
    if moved_m > NEAR_TREE_REHARVEST_M {
        return Some(NearTreeRecompute::Moved);
    }
    if !reuse {
        return Some(NearTreeRecompute::ReuseOff);
    }
    // A change-driven re-harvest waits out the interval; a NaN interval
    // (should never happen) counts as "not yet", never as "due".
    let change_due = since_last_s >= NEAR_TREE_CHANGE_MIN_INTERVAL_S;
    if !change_due {
        return None;
    }
    if now.depth != last.depth {
        return Some(NearTreeRecompute::DepthChanged);
    }
    if (now.density - last.density).abs() > 1.0e-6 {
        return Some(NearTreeRecompute::DensityChanged);
    }
    if (now.reach_m - last.reach_m).abs() > 1.0e-6 || now.cap != last.cap {
        return Some(NearTreeRecompute::ReachChanged);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The depth and density cases, with the reach unchanged and reuse on.
    fn reason(moved: f64, depth: u8, last_depth: u8, density: f32, last_density: f32, since: f32) -> Option<NearTreeRecompute> {
        let reach = |depth, density| HarvestInputs { depth, density, reach_m: 260.0, cap: 640 };
        near_tree_recompute_reason(moved, reach(depth, density), reach(last_depth, last_density), since, true)
    }

    /// THE SLIDERS REAL BUG (2026-11-13): the tree model distance and the
    /// near-tree budget size the harvest, but neither was in the gate, so a
    /// slider move changed nothing on screen until the player walked 12 m.
    /// A parked player who moves either slider gets a new harvest once the
    /// change interval has passed (rate limited like the density, since a
    /// drag moves the value every frame). Seen red 2026-11-13 with the two
    /// left out: "the distance slider moved, parked: no harvest".
    #[test]
    fn a_reach_slider_move_reharvests_without_walking() {
        let last = HarvestInputs { depth: 9, density: 0.5, reach_m: 260.0, cap: 640 };
        let farther = HarvestInputs { reach_m: 460.0, ..last };
        let more = HarvestInputs { cap: 1280, ..last };
        let since = NEAR_TREE_CHANGE_MIN_INTERVAL_S;
        assert_eq!(
            near_tree_recompute_reason(0.0, farther, last, since, true),
            Some(NearTreeRecompute::ReachChanged),
            "the distance slider moved, parked: no harvest"
        );
        assert_eq!(
            near_tree_recompute_reason(0.0, more, last, since, true),
            Some(NearTreeRecompute::ReachChanged),
            "the budget slider moved, parked: no harvest"
        );
        assert_eq!(near_tree_recompute_reason(0.0, farther, last, since - 0.01, true), None, "rate limited like the density");
        assert_eq!(near_tree_recompute_reason(0.0, last, last, 60.0, true), None, "nothing moved: kept");
    }

    /// With the reuse switch off the set is harvested again every frame,
    /// parked or not. Seen red 2026-11-13 with the switch ignored: "reuse
    /// off kept the harvest".
    #[test]
    fn with_reuse_off_the_harvest_runs_every_frame() {
        let last = HarvestInputs { depth: 9, density: 0.5, reach_m: 260.0, cap: 640 };
        for since in [0.0_f32, 0.016, 5.0] {
            assert_eq!(
                near_tree_recompute_reason(0.0, last, last, since, false),
                Some(NearTreeRecompute::ReuseOff),
                "reuse off kept the harvest"
            );
        }
        assert_eq!(NearTreeRecompute::ReuseOff.label(), "reuse off");
        assert_eq!(NearTreeRecompute::ReachChanged.label(), "reach changed");
    }

    /// A parked camera with nothing changed never re-harvests, however long
    /// it stands there: this is the per-frame log flood the gate exists to
    /// stop.
    #[test]
    fn parked_and_unchanged_never_recomputes() {
        for since in [0.0_f32, 0.1, 1.0, 60.0] {
            assert_eq!(reason(0.0, 9, 9, 0.5, 0.5, since), None);
        }
    }

    /// Movement past the hysteresis is eager: it does not wait for the
    /// change interval (a walking player must not see trees lag).
    #[test]
    fn movement_recomputes_at_once() {
        assert_eq!(
            reason(NEAR_TREE_REHARVEST_M + 0.01, 9, 9, 0.5, 0.5, 0.0),
            Some(NearTreeRecompute::Moved)
        );
        // Exactly at the threshold is "not yet" (strict), the same as before.
        assert_eq!(reason(NEAR_TREE_REHARVEST_M, 9, 9, 0.5, 0.5, 0.0), None);
        // The first harvest ever: the centre is at f64::MAX, so "moved".
        assert_eq!(
            reason(f64::MAX, 0, 0, 0.5, 0.5, 0.0),
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
        assert_eq!(reason(0.0, 10, 9, 0.5, 0.5, too_soon), None);
        assert_eq!(
            reason(0.0, 10, 9, 0.5, 0.5, just_after),
            Some(NearTreeRecompute::DepthChanged)
        );
    }

    /// Same rule for the density term, and a NaN density can never loop:
    /// NaN != NaN is true, which is exactly the every-frame trap a plain
    /// `!=` would have.
    #[test]
    fn density_change_is_rate_limited_and_nan_safe() {
        assert_eq!(reason(0.0, 9, 9, 0.7, 0.5, 0.1), None);
        assert_eq!(
            reason(0.0, 9, 9, 0.7, 0.5, 0.3),
            Some(NearTreeRecompute::DensityChanged)
        );
        assert_eq!(reason(0.0, 9, 9, f32::NAN, f32::NAN, 5.0), None);
    }

    /// Depth wins over density when both changed (either is a full
    /// re-harvest; the label just says which was noticed first).
    #[test]
    fn labels_name_the_reason() {
        assert_eq!(NearTreeRecompute::Moved.label(), "moved");
        assert_eq!(NearTreeRecompute::DepthChanged.label(), "depth changed");
        assert_eq!(NearTreeRecompute::DensityChanged.label(), "density changed");
        assert_eq!(
            reason(0.0, 10, 9, 0.7, 0.5, 1.0),
            Some(NearTreeRecompute::DepthChanged)
        );
    }
}
