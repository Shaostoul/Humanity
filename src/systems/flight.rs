//! Fleet flight state and felt gravity.
//!
//! The mothership is under way. This module owns the one number every other
//! system asks about: **how hard is gravity pulling right now, and which way**.
//!
//! ## The model
//!
//! Gravity aboard is a vector sum of independent contributors (see
//! `docs/design/gravity-and-movement.md`):
//!
//! - **thrust** -- uniform through the whole hull, pointing aft, from the drive.
//!   This is what gives the static spine its gravity, and it is why a
//!   perpetually-burning fleet is habitable outside the rotating sections.
//! - **spin** -- `omega^2 * r` radially outward inside a rotating section.
//! - (bodies -- the existing planet/moon term, handled elsewhere.)
//!
//! Thrust and spin are PERPENDICULAR when the drum axis lies along the thrust
//! axis, which is the only orientation that gives uniform gravity around the
//! rim. So inside a spinning drum under thrust, "down" is tilted by
//! `atan(thrust / spin)` away from the floor, and the felt magnitude is the
//! hypotenuse. At a realistic cruise thrust that is a few degrees of permanent
//! lean; during a hard evasion burn it is most of a right angle, which is what
//! makes such a burn catastrophic without anything being scripted.
//!
//! ## Inertial dampeners
//!
//! Dampeners cancel part of the THRUST term, drawing power to do it. Their job
//! is to keep a spinning section's floor FLAT, not to reduce gravity: an array
//! sized for cruise erases the permanent lean outright and is simply outmatched
//! by an evasion burn, so the drama is bounded by arithmetic rather than by a
//! clamp. They are a bounded, powered, failable machine, so shedding their power
//! tips the floor back over -- which couples the consequence to the live
//! electrical simulation rather than to a difficulty setting.
//!
//! ## Why the maths is here and pure
//!
//! Same discipline as `surface_move::radial_step`: a pure function of its
//! inputs, no ECS, no globals, headless-testable. Everything below can be
//! checked without booting the engine.

use serde::{Deserialize, Serialize};

/// Standard gravity, m/s^2. The unit everything here is expressed in.
pub const G: f32 = 9.80665;

/// A drive the fleet can run. Data, not code: new drives are rows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveDef {
    pub id: String,
    pub label: String,
    /// What this drive burns at during normal interstellar cruise, in g.
    /// Deliberately LOW: a high cruise thrust tilts a spinning drum's floor
    /// far enough to make it unusable, which would defeat the whole point of
    /// building a rotating habitat.
    pub cruise_thrust_g: f32,
    /// The most it can produce, in g. Reaching this is an emergency, not a
    /// travel mode.
    pub max_thrust_g: f32,
    #[serde(default)]
    pub description: String,
}

/// The inertial dampener array.
///
/// Its job is NOT "reduce gravity", which would be a difficulty slider wearing
/// a machine's name. Its job is to **keep the drum floor flat under burn** by
/// cancelling the THRUST component of the gravity vector, which is the
/// component that tilts it.
///
/// That framing is what makes the 5 g drama bounded by construction instead of
/// by a clamp. The budget is an absolute capacity in g, so an array sized for
/// cruise (a tenth of a g or so) erases the permanent lean completely and is
/// simply overwhelmed by an evasion burn. No special case, no cap, no "but not
/// during combat" rule: a device that can cancel 0.15 g cannot do anything
/// meaningful about 5 g, and the arithmetic says so on its own.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DampenerDef {
    /// Thrust-axis acceleration it can cancel, in g, at full power. Size this
    /// to cruise thrust, not to emergencies.
    pub thrust_cancel_g: f32,
    /// Watts drawn per g actually cancelled. Cancelling more costs more, so a
    /// hard burn is a power crisis as well as a gravity one.
    pub watts_per_g: f32,
}

impl Default for DampenerDef {
    fn default() -> Self {
        // Sized for cruise: enough to flatten a 0.05-0.1 g lean entirely, and
        // nowhere near enough to matter during a burn.
        Self { thrust_cancel_g: 0.15, watts_per_g: 40_000.0 }
    }
}

/// What a given kind of occupant or object can take before it starts suffering.
/// One row per tolerance class, so tuning is data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GToleranceDef {
    /// Tolerance class id: "crew", "plant", "livestock", ...
    pub id: String,
    /// Felt g at or below which nothing happens at all.
    pub safe_g: f32,
    /// Felt g at which harm accrues at `harm_per_sec`; scales linearly beyond
    /// `safe_g`, so the curve is defined by two points rather than a table.
    pub harm_g: f32,
    /// Harm per second once at `harm_g`. Units are per-consumer (health points
    /// for crew, crop-health points for plants).
    pub harm_per_sec: f32,
    #[serde(default)]
    pub description: String,
}

/// Everything in `data/ship/flight.ron`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlightData {
    pub drives: Vec<DriveDef>,
    #[serde(default)]
    pub dampener: DampenerDef,
    #[serde(default)]
    pub tolerances: Vec<GToleranceDef>,
}

impl Default for FlightData {
    fn default() -> Self {
        Self { drives: Vec::new(), dampener: DampenerDef::default(), tolerances: Vec::new() }
    }
}

impl FlightData {
    /// Load `data/ship/flight.ron`. A missing or malformed file is NOT fatal:
    /// it degrades to defaults, which means no drives and no tolerance rows,
    /// which in turn means `harm_per_sec` is never consulted and nobody is
    /// harmed. Failing safe matters here because this file's whole job is to
    /// decide when to damage the player.
    pub fn load(data_dir: &std::path::Path) -> Self {
        let path = data_dir.join("ship").join("flight.ron");
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                log::warn!("flight: could not read {}: {e}; g-load effects are OFF", path.display());
                return Self::default();
            }
        };
        match ron::from_str::<FlightData>(&text) {
            Ok(d) => {
                log::info!(
                    "flight: {} drives, {} tolerance classes, dampener cancels {} g",
                    d.drives.len(),
                    d.tolerances.len(),
                    d.dampener.thrust_cancel_g
                );
                d
            }
            Err(e) => {
                log::warn!("flight: could not parse {}: {e}; g-load effects are OFF", path.display());
                Self::default()
            }
        }
    }

    /// Look up a tolerance class, e.g. "crew" or "plant".
    pub fn tolerance(&self, id: &str) -> Option<&GToleranceDef> {
        self.tolerances.iter().find(|t| t.id == id)
    }

    /// Look up a drive by id.
    pub fn drive(&self, id: &str) -> Option<&DriveDef> {
        self.drives.iter().find(|d| d.id == id)
    }
}

/// The fleet's live flight state: what the drive is doing right now.
#[derive(Debug, Clone, PartialEq)]
pub struct FlightState {
    /// Current drive thrust, in g, along the ship's aft axis.
    pub thrust_g: f32,
    /// Spin gravity at the occupant's radius, in g. Zero outside a rotating
    /// section and zero on the axis.
    pub spin_g: f32,
    /// Whether the dampener array is powered and running.
    pub dampeners_online: bool,
}

impl Default for FlightState {
    fn default() -> Self {
        // Parked: no burn, no spin, nothing to cancel.
        Self { thrust_g: 0.0, spin_g: 0.0, dampeners_online: true }
    }
}

/// The result of combining every gravity contributor with the dampeners.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeltGravity {
    /// Raw magnitude before dampening, in g. This is the physical truth, and
    /// it is what structures and unsecured objects experience: dampeners
    /// protect occupants, not the hull.
    pub raw_g: f32,
    /// Magnitude actually felt by occupants after dampening, in g.
    pub felt_g: f32,
    /// How far "down" leans away from a spinning section's floor, in degrees,
    /// AFTER dampening. This is the slope you walk on and the one water runs
    /// down. Zero when not spinning (no floor to lean away from) and zero when
    /// the dampeners have cancelled the thrust entirely.
    pub tilt_deg: f32,
    /// The lean there would be with the dampeners off. Compare against
    /// `tilt_deg` to show what the array is buying, and to make its failure
    /// legible the instant power is shed.
    pub raw_tilt_deg: f32,
    /// Power the dampeners are drawing to achieve this, in watts.
    pub dampener_watts: f32,
}

/// Combine thrust, spin and the dampeners into what is actually felt.
///
/// Thrust is axial and spin is radial, so they are perpendicular and the
/// magnitude is the hypotenuse. The tilt is meaningful only inside a spinning
/// section, where there is a floor for "down" to lean away from.
///
/// The dampeners cancel part of the THRUST term, which shrinks both the tilt
/// and (a little) the magnitude. Sized for cruise they erase the lean outright;
/// during an evasion burn they are simply outmatched, and the drama survives
/// without a clamp anywhere in the code.
pub fn felt_gravity(state: &FlightState, damp: &DampenerDef) -> FeltGravity {
    let hypot = |t: f32, s: f32| (t * t + s * s).sqrt();
    let tilt = |t: f32, s: f32| if s > 0.0 { t.atan2(s).to_degrees() } else { 0.0 };

    let raw_g = hypot(state.thrust_g, state.spin_g);
    let cancel = if state.dampeners_online {
        // Never cancel more thrust than there is: a cruise-sized array running
        // during a coast must not bill for work it did not do.
        damp.thrust_cancel_g.max(0.0).min(state.thrust_g.abs())
    } else {
        0.0
    };
    let thrust_eff = state.thrust_g - cancel * state.thrust_g.signum();

    FeltGravity {
        raw_g,
        felt_g: hypot(thrust_eff, state.spin_g),
        raw_tilt_deg: tilt(state.thrust_g, state.spin_g),
        tilt_deg: tilt(thrust_eff, state.spin_g),
        dampener_watts: cancel * damp.watts_per_g.max(0.0),
    }
}

/// Harm per second for one tolerance class at a given felt gravity.
///
/// Zero at or below `safe_g`, then linear, reaching `harm_per_sec` exactly at
/// `harm_g` and continuing to climb beyond it. Two points define the curve so
/// the data stays readable; the linear continuation past `harm_g` is what makes
/// an extreme burn lethal quickly rather than merely unpleasant.
pub fn harm_per_sec(tol: &GToleranceDef, felt_g: f32) -> f32 {
    if felt_g <= tol.safe_g {
        return 0.0;
    }
    let span = tol.harm_g - tol.safe_g;
    if span <= 0.0 {
        // Degenerate row (harm_g not above safe_g): treat any exceedance as
        // full harm rather than dividing by zero.
        return tol.harm_per_sec.max(0.0);
    }
    (felt_g - tol.safe_g) / span * tol.harm_per_sec.max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn damp(thrust_cancel_g: f32) -> DampenerDef {
        DampenerDef { thrust_cancel_g, watts_per_g: 1000.0 }
    }

    fn off() -> DampenerDef {
        DampenerDef { thrust_cancel_g: 0.0, watts_per_g: 1000.0 }
    }

    /// Parked with no spin and no burn is genuinely weightless, and nothing
    /// tilts because there is no floor to lean away from.
    #[test]
    fn parked_and_unspun_is_weightless() {
        let f = felt_gravity(&FlightState::default(), &off());
        assert_eq!(f.raw_g, 0.0);
        assert_eq!(f.felt_g, 0.0);
        assert_eq!(f.tilt_deg, 0.0);
    }

    /// The spine under cruise thrust: thrust alone IS the gravity, which is the
    /// whole reason a perpetually-burning fleet has a habitable static section.
    #[test]
    fn thrust_alone_gives_the_spine_its_gravity() {
        let s = FlightState { thrust_g: 0.1, spin_g: 0.0, dampeners_online: false };
        let f = felt_gravity(&s, &off());
        assert!((f.raw_g - 0.1).abs() < 1e-6);
        assert_eq!(f.tilt_deg, 0.0, "no spin means no floor to tilt away from");
    }

    /// The drum while coasting: spin alone, floor normal is down, no tilt.
    #[test]
    fn spin_alone_is_untilted() {
        let s = FlightState { thrust_g: 0.0, spin_g: 1.0, dampeners_online: false };
        let f = felt_gravity(&s, &off());
        assert!((f.raw_g - 1.0).abs() < 1e-6);
        assert!(f.tilt_deg.abs() < 1e-4);
    }

    /// The headline case, dampeners off. At a realistic cruise thrust the drum
    /// floor leans a few degrees: characterful rather than a problem.
    #[test]
    fn cruise_thrust_leans_the_drum_floor_only_slightly() {
        let s = FlightState { thrust_g: 0.1, spin_g: 1.0, dampeners_online: false };
        let f = felt_gravity(&s, &off());
        assert!((f.raw_g - 1.00499).abs() < 1e-3, "raw was {}", f.raw_g);
        assert!((f.tilt_deg - 5.71).abs() < 0.05, "tilt was {}", f.tilt_deg);
    }

    /// The other end of the same expression: an evasion burn puts "down" most
    /// of a right angle off the floor, so the drum floor is effectively a wall
    /// and everything in it falls aft. Nothing scripted this.
    #[test]
    fn an_evasion_burn_turns_the_drum_floor_into_a_wall() {
        let s = FlightState { thrust_g: 5.0, spin_g: 1.0, dampeners_online: false };
        let f = felt_gravity(&s, &off());
        assert!((f.raw_g - 5.09902).abs() < 1e-3, "raw was {}", f.raw_g);
        assert!((f.tilt_deg - 78.69).abs() < 0.05, "tilt was {}", f.tilt_deg);
    }

    /// The dampeners' actual job: a cruise-sized array erases the permanent
    /// lean entirely, and bills the power system for exactly what it cancelled.
    #[test]
    fn a_cruise_sized_array_flattens_the_floor_completely() {
        let s = FlightState { thrust_g: 0.1, spin_g: 1.0, dampeners_online: true };
        let f = felt_gravity(&s, &damp(0.15));
        assert!(f.tilt_deg.abs() < 1e-4, "floor should be flat, leaned {}", f.tilt_deg);
        assert!((f.raw_tilt_deg - 5.71).abs() < 0.05, "and it should report what it saved");
        assert!((f.felt_g - 1.0).abs() < 1e-4, "only the spin term should remain");
        // Cancelled 0.1 g, not its full 0.15 g rating: it cannot bill for
        // thrust that was never there.
        assert!((f.dampener_watts - 100.0).abs() < 1e-3, "billed {}", f.dampener_watts);
    }

    /// THE POINT OF THE WHOLE DESIGN. The same array is simply outmatched by an
    /// evasion burn, so the drama is bounded by construction rather than by a
    /// clamp. If this ever passes with a small tilt, dampeners have become a
    /// difficulty slider and the 5 g burn has stopped meaning anything.
    #[test]
    fn the_same_array_cannot_save_you_from_an_evasion_burn() {
        let s = FlightState { thrust_g: 5.0, spin_g: 1.0, dampeners_online: true };
        let f = felt_gravity(&s, &damp(0.15));
        assert!(f.tilt_deg > 78.0, "floor must still be a wall, was {} deg", f.tilt_deg);
        assert!(f.felt_g > 4.9, "felt g must stay lethal, was {}", f.felt_g);
    }

    /// Cancelling must never invert thrust or bill for work not done.
    #[test]
    fn dampeners_never_overshoot_into_negative_thrust() {
        let s = FlightState { thrust_g: 0.05, spin_g: 0.0, dampeners_online: true };
        let f = felt_gravity(&s, &damp(0.15));
        assert_eq!(f.felt_g, 0.0, "cancelling more than exists is still zero, not negative");
        assert!((f.dampener_watts - 50.0).abs() < 1e-3, "billed {}", f.dampener_watts);
    }

    /// The coupling that makes them a machine rather than a setting: shed the
    /// power mid-cruise and the lean comes straight back.
    #[test]
    fn losing_dampener_power_brings_the_lean_back() {
        let mut s = FlightState { thrust_g: 0.1, spin_g: 1.0, dampeners_online: true };
        let d = damp(0.15);
        assert!(felt_gravity(&s, &d).tilt_deg.abs() < 1e-4);
        s.dampeners_online = false;
        assert!((felt_gravity(&s, &d).tilt_deg - 5.71).abs() < 0.05);
    }

    /// Dampeners protect occupants, not the hull. Raw magnitude is untouched,
    /// because a structural load is not something you can talk out of.
    #[test]
    fn dampeners_do_not_protect_the_structure() {
        let s = FlightState { thrust_g: 5.0, spin_g: 1.0, dampeners_online: true };
        assert_eq!(felt_gravity(&s, &damp(0.15)).raw_g, felt_gravity(&s, &off()).raw_g);
    }

    fn crew() -> GToleranceDef {
        GToleranceDef {
            id: "crew".into(),
            safe_g: 1.5,
            harm_g: 4.0,
            harm_per_sec: 2.0,
            description: String::new(),
        }
    }

    #[test]
    fn nothing_is_harmed_inside_the_safe_band() {
        assert_eq!(harm_per_sec(&crew(), 1.0), 0.0);
        assert_eq!(harm_per_sec(&crew(), 1.5), 0.0, "the boundary itself is safe");
    }

    #[test]
    fn harm_ramps_linearly_and_hits_the_named_rate_at_harm_g() {
        let c = crew();
        assert!((harm_per_sec(&c, 4.0) - 2.0).abs() < 1e-5, "harm_g must give harm_per_sec");
        // Halfway between 1.5 and 4.0 is 2.75, so half the rate.
        assert!((harm_per_sec(&c, 2.75) - 1.0).abs() < 1e-5);
    }

    /// Past the named point the curve keeps climbing, which is what makes an
    /// extreme burn lethal quickly rather than merely uncomfortable.
    #[test]
    fn harm_keeps_climbing_past_harm_g() {
        let c = crew();
        let at = harm_per_sec(&c, 4.0);
        let beyond = harm_per_sec(&c, 6.5);
        assert!(beyond > at * 1.9, "{beyond} should roughly double {at}");
    }

    /// A badly authored row must not divide by zero or emit NaN into health.
    #[test]
    fn a_degenerate_tolerance_row_is_survivable() {
        let bad = GToleranceDef {
            id: "bad".into(),
            safe_g: 2.0,
            harm_g: 2.0, // no span
            harm_per_sec: 3.0,
            description: String::new(),
        };
        assert_eq!(harm_per_sec(&bad, 1.0), 0.0);
        let h = harm_per_sec(&bad, 5.0);
        assert!(h.is_finite(), "must not be NaN or infinite");
        assert_eq!(h, 3.0);
    }
}

#[cfg(test)]
mod data_tests {
    use super::*;

    /// The shipped file must actually parse, and its numbers must agree with
    /// the design. A typo here silently disables every g-load consequence,
    /// because `load` fails SAFE by design.
    #[test]
    fn the_shipped_flight_data_parses_and_is_sane() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let d = FlightData::load(&dir);
        assert!(!d.drives.is_empty(), "data/ship/flight.ron did not parse (drives empty)");

        let torch = d.drive("fusion_torch").expect("fusion_torch drive missing");
        assert!(
            torch.cruise_thrust_g > 0.0 && torch.cruise_thrust_g <= 0.15,
            "cruise thrust {} g would tilt a spinning drum past usable; see the file header",
            torch.cruise_thrust_g
        );
        assert!(torch.max_thrust_g >= torch.cruise_thrust_g);

        for id in ["crew", "plant", "livestock"] {
            let t = d.tolerance(id).unwrap_or_else(|| panic!("tolerance '{id}' missing"));
            assert!(t.harm_g > t.safe_g, "'{id}' has no harm span, so harm would be a step");
            assert!(t.harm_per_sec > 0.0, "'{id}' harms by zero, so the row does nothing");
        }
    }

    /// The operator's own example, checked against the shipped numbers rather
    /// than against a comment: a sustained 5 g evasion burn must actually kill
    /// a crop, and in a time that reads as "during the burn".
    #[test]
    fn a_five_g_burn_really_does_kill_a_crop() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let d = FlightData::load(&dir);
        let plant = d.tolerance("plant").expect("plant tolerance missing");

        let burn = FlightState { thrust_g: 5.0, spin_g: 1.0, dampeners_online: false };
        let felt = felt_gravity(&burn, &d.dampener).felt_g;
        let rate = harm_per_sec(plant, felt);
        assert!(rate > 0.0, "a 5 g burn must harm crops at all");

        // Crop health is a 0-100 scale.
        let seconds_to_death = 100.0 / rate;
        assert!(
            (10.0..=120.0).contains(&seconds_to_death),
            "a crop dies in {seconds_to_death:.0}s; outside the 10-120s band that reads as \
             either instant or irrelevant"
        );
    }

    /// Cruise must be boring. If ordinary interstellar travel harmed the crew
    /// or the crops, the whole setting would be unplayable.
    #[test]
    fn ordinary_cruise_harms_nobody() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let d = FlightData::load(&dir);
        let torch = d.drive("fusion_torch").unwrap();
        let cruise = FlightState {
            thrust_g: torch.cruise_thrust_g,
            spin_g: 1.0,
            dampeners_online: false,
        };
        let felt = felt_gravity(&cruise, &d.dampener).felt_g;
        for id in ["crew", "plant", "livestock"] {
            let t = d.tolerance(id).unwrap();
            assert_eq!(
                harm_per_sec(t, felt),
                0.0,
                "'{id}' is harmed at cruise ({felt} g), which would make travel unplayable"
            );
        }
    }
}
