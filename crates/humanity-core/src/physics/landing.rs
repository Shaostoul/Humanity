//! How hard a body meets the floor, and the watch that decides whether a landing is a fall that
//! can hurt (falls, 2026-10-20).
//!
//! IMPACT SPEED, NOT HEIGHT: what hurts a body is how fast it is going when the ground stops it,
//! v^2 = v0^2 + 2 g h, the same rule under a ship's spin gravity, on the Moon and on Earth. The
//! game walks a body two ways and both report the speed their floor stopped: aboard, the
//! renderer's `CameraController::update_first_person`; on a planet's surface, the simulation's
//! `surface_move::radial_step`. Both feed one `FallWatch` on the controller. What a landing then
//! does to the body (the harm curve, the wounds, the two modes) is the simulation's:
//! `systems::falls`.
//!
//! WHY IT LIVES IN THE CORE (the crate split's rung 0, 2026-10-31,
//! docs/design/crate-split.md): it was the physics half of `systems::falls`, and the renderer's
//! camera controller keeps the watch and measures the aboard landing, so the renderer reached into
//! the simulation for it. It is plain physics and two flags: here the camera, the surface walk and
//! the falls rules all read the one definition.

/// The height of a drop that lands at `v` m/s under gravity `g`: h = v^2 / (2 g).
pub fn drop_height_m(v: f32, g: f32) -> f32 {
    if g <= 0.0 {
        return 0.0;
    }
    v * v / (2.0 * g)
}

/// The speed a body meets the floor at, from what it had at the start of the frame: `v_up` its
/// vertical speed then (+ up), `above_m` how far above the floor it was, `g` the gravity pulling
/// it down. Energy, not the integrated velocity: v^2 = v0^2 + 2 g h holds whatever the frame's
/// length, so a long frame (a stall while loading) neither invents a fall for a body standing
/// still (0 and 0 give 0) nor overstates a real one, as the frame's last velocity step would.
pub fn landing_speed(v_up: f64, above_m: f64, g: f64) -> f64 {
    (v_up * v_up + 2.0 * g.max(0.0) * above_m.max(0.0)).sqrt()
}

/// Below this a landing is a step, not a fall, and is not reported: standing still lands at 0,
/// and the aboard walk steps down a stair's riser without leaving the floor at all
/// (`renderer::camera::STEP_DOWN_M`, BUG-208). Well under every harm in falls.ron.
pub const LANDING_M_S: f64 = 1.0;

/// One body's falls, between leaving the ground and landing again (the controller keeps one for
/// the player). Decides whether a landing is a fall that can hurt.
#[derive(Debug, Clone, Default)]
pub struct FallWatch {
    /// This time off the ground was flight or a placement: its landing hurts nobody.
    exempt: bool,
    /// The hardest landing not yet taken, m/s.
    pending: Option<f32>,
}

impl FallWatch {
    /// Dev flight is on (fly mode, the F9 hover, the planet's flight band), or something other
    /// than walking moved the body (a teleport, driving, the rig's camera hold): the landing
    /// that ends this time off the ground hurts nobody.
    pub fn exempt(&mut self) {
        self.exempt = true;
    }

    /// Held up by something other than the floor (a ladder): this time off the ground is over,
    /// and a fall from here starts fresh.
    pub fn supported(&mut self) {
        self.exempt = false;
    }

    /// The floor stopped the body at `impact_m_s` (0 for a frame standing on it). A fall that can
    /// hurt is kept for `take`; one onto water, an exempt one or a mere step is not. Either way
    /// the time off the ground is over.
    pub fn landed(&mut self, impact_m_s: f64, on_water: bool) {
        if !self.exempt && !on_water && impact_m_s >= LANDING_M_S {
            let v = impact_m_s as f32;
            self.pending = Some(self.pending.map_or(v, |p| p.max(v)));
        }
        self.exempt = false;
    }

    /// The landing waiting to be given to the body, taken.
    pub fn take(&mut self) -> Option<f32> {
        self.pending.take()
    }

    /// Whether the time off the ground so far is exempt (for tests).
    pub fn is_exempt(&self) -> bool {
        self.exempt
    }
}
