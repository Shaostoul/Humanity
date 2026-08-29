//! Analytic ocean disaster fields: tsunami soliton, rogue wave group,
//! Rankine vortex (maelstrom / waterspout base), hurricane eyewall.
//!
//! Rung 1 of the ABYSSAL adoption arc (docs/PRIORITIES.md; full menu in
//! docs/reference/abyssal-ocean-weather.md). The field math is adopted from
//! the MIT-licensed ABYSSAL simulator (github.com/Token-Gremlin/
//! natural-disasters, src/ocean/OceanSampleGLSL.js + src/weather/Director.js)
//! and follows the same contract this repo already enforces for waves:
//! whatever the water DRAWS is exactly what physics SAMPLES. Every event is a
//! closed-form height field of (position, a few parameters), so the CPU float
//! clamp, the camera, and the future WGSL twin (rung 2) all evaluate the same
//! numbers with no GPU readback. A rung-2 lockstep test in the
//! ocean_waves.rs L192-252 pattern will parse the WGSL for these constants.
//!
//! FRAME CONTRACT (the f32-at-planet-scale rule applies): `p` is a 2D
//! position in a LOCAL east-north tangent frame anchored near the active
//! events, in f64 meters. Callers project planet positions into that frame in
//! f64 FIRST; the f32 downcast happens only in `to_uniform_vec4s`, at the
//! local-frame boundary where magnitudes are tens of km at most. Heights are
//! radial (up) displacements in meters. The chord-vs-arc error of a tangent
//! frame is ~0.01% at 50 km, far below anything these shapes resolve.
//!
//! NO GAMEPLAY DEFAULTS LIVE HERE (infinite-of-x): per-event tuning (a
//! tsunami's amplitude, a maelstrom's radius) arrives from
//! data/weather/events.ron at rung 3. ABYSSAL's demo values appear only in
//! the tests below, as pinned reference vectors.

use glam::{DVec2, DVec3};

// ---------------------------------------------------------------- constants
//
// Adopted verbatim from ABYSSAL and pinned by the tests at the bottom. These
// are part of the field SHAPES, not tuning: change one and the rung-2 WGSL
// twin, the buoyancy ride, and the foam placement all change together.

/// sech() argument clamp. Part of the profile, not a safety detail: the CPU
/// twin and the WGSL twin must saturate at the same argument or their tails
/// disagree bit-visibly under a riding camera.
pub const SECH_ARG_CLAMP: f64 = 12.0;

/// Tsunami: the leading (unbroken-water) half of the coordinate is
/// compressed by (1 + steep * this), which stands the front face up while
/// the back stays a deep-water mound. One smooth function, so normals, foam
/// and the ride height all inherit the shoaled asymmetry for free.
pub const SOLITON_SHOAL_COMPRESS: f64 = 1.35;
/// Tsunami drawdown trough: centered this many widths AHEAD of the crest
/// (the receding-sea precursor), with width this fraction of the crest width
/// and depth (per unit steep) this fraction of the peak.
pub const SOLITON_DRAWDOWN_AHEAD_WIDTHS: f64 = 1.6;
pub const SOLITON_DRAWDOWN_WIDTH_FRAC: f64 = 1.1;
pub const SOLITON_DRAWDOWN_DEPTH_PER_STEEP: f64 = 0.16;
/// Tsunami overhang push: crest water thrown forward, concentrated as
/// profile^2 so only the lip translates (a whole-neighbourhood shift tears a
/// hole under the camera). Capped at this fraction of the width so a tall
/// narrow wall does not throw its lip a full wavelength forward.
pub const SOLITON_OVERHANG_WIDTH_CAP: f64 = 0.4;
pub const SOLITON_OVERHANG_GAIN: f64 = 0.28;
/// Tsunami breaking-lip foam: crest band of the normalized profile, biased
/// onto the forward face (foam on the lip and the wash, not the shoulder).
pub const SOLITON_CREST_LO: f64 = 0.80;
pub const SOLITON_CREST_HI: f64 = 0.985;
pub const SOLITON_FACE_BIAS_MIN: f64 = 0.25;

/// Rogue wave carrier: a 3-mode Gerstner-like group. Mode frequency ratios /
/// phase offsets / amplitudes, the amplitude normalizer (sum of mode amps),
/// and the crest-sharpening exponent (peaky crests, flat troughs).
pub const ROGUE_MODE2_FREQ: f64 = 1.87;
pub const ROGUE_MODE2_PHASE: f64 = 1.1;
pub const ROGUE_MODE2_AMP: f64 = 0.42;
pub const ROGUE_MODE3_FREQ: f64 = 0.61;
pub const ROGUE_MODE3_PHASE: f64 = -0.7;
pub const ROGUE_MODE3_AMP: f64 = 0.55;
pub const ROGUE_NORM: f64 = 1.97;
pub const ROGUE_SHARPEN_POW: f64 = 0.72;
/// The ride rule: the camera/buoyancy twin tracks the group ENVELOPE at this
/// fraction of the amplitude, never the oscillating carrier. Chasing the
/// carrier's exact phase makes a riding camera judder (ABYSSAL's words);
/// the GPU still draws the full peaky group.
pub const ROGUE_RIDE_ENVELOPE_FRAC: f64 = 0.8;
/// Horizontal orbital push amplitude (fraction of amp along the carrier dir).
pub const ROGUE_PUSH_GAIN: f64 = 0.45;

/// Rankine vortex (maelstrom, and the ocean base of a waterspout): solid-body
/// rotation inside the core radius, ~1/r^2 tail outside via
/// 1/(TAIL_A*x^2 + TAIL_B), an exp(-ENV_K*x^2) reach envelope, a funnel
/// depression 1/(1 + DEPRESSION_K*x^2), and shear-driven foam that SATURATES:
/// past full whiteness the sea cannot get whiter, and without the cap a 40 m
/// maelstrom's spiral arms merge into one white disc.
pub const VORTEX_TAIL_A: f64 = 0.65;
pub const VORTEX_TAIL_B: f64 = 0.35;
pub const VORTEX_ENV_K: f64 = 0.55;
pub const VORTEX_DEPRESSION_K: f64 = 2.2;
pub const VORTEX_SHEAR_PER_STRENGTH: f64 = 0.022;
pub const VORTEX_SHEAR_CAP: f64 = 0.62;
pub const VORTEX_HEIGHT_GAIN: f64 = 0.09;
/// The swirl-COORD variant (slightly different tail/envelope, per ABYSSAL):
/// instead of simulating rotating water, the wave-texture lookup coordinate
/// is rotated about the vortex center by a time-growing Rankine angle, so the
/// existing sea detail spirals into the drain for free.
pub const VORTEX_SWIRL_TAIL_A: f64 = 0.6;
pub const VORTEX_SWIRL_TAIL_B: f64 = 0.4;
pub const VORTEX_SWIRL_ENV_K: f64 = 0.5;
pub const VORTEX_SWIRL_RATE: f64 = 0.05;

/// Hurricane: a Gaussian eyewall swell RING peaking at RING_POS eye radii,
/// a glassy calm exp(-EYE_CALM_K*x^2) inside the eye (also exported to
/// suppress chop and foam there), a mild depression under the eye, and a
/// slow-decay tangential swirl (1/x^TANG_OUT_POW outside the eyewall).
pub const HURRICANE_EYE_CALM_K: f64 = 1.6;
pub const HURRICANE_RING_POS: f64 = 1.25;
pub const HURRICANE_RING_WIDTH_K: f64 = 1.4;
pub const HURRICANE_RING_GAIN: f64 = 3.0;
pub const HURRICANE_EYE_DEPRESSION: f64 = 0.4;
pub const HURRICANE_TANG_INNER: f64 = 1.05;
pub const HURRICANE_TANG_OUT_POW: f64 = 0.55;
pub const HURRICANE_SWIRL_RATE: f64 = 0.0015;
/// Eye radius floor: the field divides by it.
pub const HURRICANE_EYE_MIN_M: f64 = 50.0;

/// Activity thresholds, matching the shader-side w-component convention so
/// the uniform packing needs no separate flags: an event whose strength /
/// amplitude / intensity sits at or below its epsilon is OFF.
pub const VORTEX_ACTIVE_EPS: f64 = 1e-4;
pub const EVENT_ACTIVE_EPS: f64 = 1e-3;

/// Slot budget, mirrored by the rung-2 uniform layout.
pub const MAX_VORTICES: usize = 4;
pub const MAX_SOLITONS: usize = 2;

const G: f64 = 9.80665;

// ------------------------------------------------------------------ helpers

/// sech with the shared argument clamp. WGSL has cosh; the rung-2 twin must
/// clamp identically (see SECH_ARG_CLAMP).
#[inline]
fn sech(x: f64) -> f64 {
    1.0 / x.clamp(-SECH_ARG_CLAMP, SECH_ARG_CLAMP).cosh()
}

#[inline]
fn smoothstep(lo: f64, hi: f64, x: f64) -> f64 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Deep-water phase speed c = sqrt(g * lambda / 2pi). A rogue wave's
/// wavelength and travel speed must stay a consistent pair or the group
/// slides through its own carrier (ABYSSAL pairs 420 m with 26 m/s).
pub fn deep_water_phase_speed_mps(wavelength_m: f64) -> f64 {
    (G * wavelength_m / std::f64::consts::TAU).sqrt()
}

// ------------------------------------------------------------------- events

/// Maelstrom / whirlpool / the ocean-surface base of a waterspout.
#[derive(Clone, Copy, Debug, Default)]
pub struct Vortex {
    pub center: DVec2,
    pub radius_m: f64,
    /// Depression depth scale in meters; <= VORTEX_ACTIVE_EPS means OFF.
    pub strength_m: f64,
}

/// Travelling solitary wave (tsunami). The crest is the line
/// dot(p, dir) == crest_dist_m; positive along-track coordinate is water the
/// wave has not reached yet (the flank that breaks).
#[derive(Clone, Copy, Debug, Default)]
pub struct Soliton {
    /// Unit travel direction in the event frame.
    pub dir: DVec2,
    /// Along-track crest position in meters (rung 3 advances this by
    /// speed * dt; the field itself is time-free).
    pub crest_dist_m: f64,
    /// Peak height in meters; <= EVENT_ACTIVE_EPS means OFF.
    pub amp_m: f64,
    /// sech^2 half-width in meters.
    pub width_m: f64,
    /// Shoaling steepness (0 = symmetric deep-water mound; ~1.2 = the
    /// standing-up nearshore wall everybody pictures).
    pub steep: f64,
    /// Lateral Gaussian half-extent in meters, making the wave a finite
    /// front rather than an infinite line.
    pub lateral_m: f64,
}

/// Rogue wave: a Gaussian group envelope times a peaky 3-mode carrier.
#[derive(Clone, Copy, Debug, Default)]
pub struct RogueWave {
    pub center: DVec2,
    /// Envelope radius in meters.
    pub radius_m: f64,
    /// Carrier amplitude in meters; <= EVENT_ACTIVE_EPS means OFF.
    pub amp_m: f64,
    /// Unit carrier direction.
    pub dir: DVec2,
    /// Carrier wavelength in meters. Keep paired with the event's travel
    /// speed via deep_water_phase_speed_mps.
    pub wavelength_m: f64,
    /// Carrier phase in radians (rung 3 advances it).
    pub phase: f64,
}

/// Hurricane at sea: eyewall swell ring + glassy eye.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hurricane {
    pub center: DVec2,
    /// Eye radius in meters (floored at HURRICANE_EYE_MIN_M).
    pub eye_m: f64,
    /// Ring height scale in meters; <= EVENT_ACTIVE_EPS means OFF.
    pub intensity_m: f64,
}

/// The full analytic modifier stack at one point: what the GPU adds to the
/// FFT/train sea. `disp` is (east, up, north) displacement in meters;
/// `crest` in [0, ~1] seeds breaking foam; `calm` in [0, 1] suppresses
/// chop and foam inside a hurricane eye.
#[derive(Clone, Copy, Debug, Default)]
pub struct EventModifiers {
    pub disp: DVec3,
    pub crest: f64,
    pub calm: f64,
}

/// Every active disaster on the local sea, in one struct whose layout
/// mirrors the rung-2 uniform block (fixed slots, epsilon-off convention).
#[derive(Clone, Copy, Debug, Default)]
pub struct OceanEvents {
    pub vortices: [Vortex; MAX_VORTICES],
    pub solitons: [Soliton; MAX_SOLITONS],
    pub rogue: RogueWave,
    pub hurricane: Hurricane,
}

impl Soliton {
    /// Crest profile normalized to a peak of ~1, as a function of the
    /// along-track distance from the crest. Shoaling asymmetry comes from
    /// compressing the LEADING half of the coordinate, so one smooth
    /// function serves height, normals, foam and the ride.
    pub fn profile(x_m: f64, width_m: f64, steep: f64) -> f64 {
        let w = width_m.max(1.0);
        let xf = if x_m > 0.0 { x_m * (1.0 + steep * SOLITON_SHOAL_COMPRESS) } else { x_m };
        let s = sech(xf / w);
        // Drawdown ahead of the face: the volume standing up in the crest
        // comes from the water immediately in front, which is the sea
        // running out before the wave lands.
        let d = sech((x_m - w * SOLITON_DRAWDOWN_AHEAD_WIDTHS) / (w * SOLITON_DRAWDOWN_WIDTH_FRAC));
        s * s - d * d * SOLITON_DRAWDOWN_DEPTH_PER_STEEP * steep
    }

    fn lateral_envelope(&self, p: DVec2) -> f64 {
        let lateral = p.dot(DVec2::new(-self.dir.y, self.dir.x));
        (-(lateral * lateral) / (self.lateral_m * self.lateral_m + 1.0)).exp()
    }

    /// Vertical height only (the ride / cheap-iteration form).
    pub fn height_m(&self, p: DVec2) -> f64 {
        if self.amp_m <= EVENT_ACTIVE_EPS {
            return 0.0;
        }
        let x = p.dot(self.dir) - self.crest_dist_m;
        self.amp_m * Self::profile(x, self.width_m, self.steep) * self.lateral_envelope(p)
    }

    /// Full modifier: height, forward overhang push, breaking-lip crest foam.
    pub fn modifiers(&self, p: DVec2) -> (f64, DVec2, f64) {
        if self.amp_m <= EVENT_ACTIVE_EPS {
            return (0.0, DVec2::ZERO, 0.0);
        }
        let w = self.width_m.max(1.0);
        let x = p.dot(self.dir) - self.crest_dist_m;
        let lat_env = self.lateral_envelope(p);
        let prof = Self::profile(x, self.width_m, self.steep);
        let h = self.amp_m * prof * lat_env;

        // Overhang: crest water thrown forward over the face. profile^2
        // concentrates it in the lip so the neighbourhood is not shifted
        // bodily (which tears a hole under a riding camera).
        let top = prof.clamp(0.0, 1.0);
        let push = self.dir
            * (top * top
                * self.amp_m.min(w * SOLITON_OVERHANG_WIDTH_CAP)
                * self.steep
                * SOLITON_OVERHANG_GAIN
                * lat_env);

        // Foam on the breaking lip and the wash running down the face, not
        // the whole shoulder (a foamed shoulder reads as a painted slab).
        let face_bias = SOLITON_FACE_BIAS_MIN
            + (1.0 - SOLITON_FACE_BIAS_MIN) * smoothstep(-w * 0.35, w * 0.2, x);
        let crest = smoothstep(SOLITON_CREST_LO, SOLITON_CREST_HI, prof) * lat_env * face_bias;
        (h, push, crest)
    }
}

impl Vortex {
    #[inline]
    fn normalized_radius(&self, p: DVec2) -> f64 {
        ((p - self.center).length() + 1e-3) / self.radius_m.max(1.0)
    }

    /// Rankine tangential speed profile at normalized radius x: solid body
    /// inside the core, ~1/r^2 outside. Continuous at x = 1 because
    /// TAIL_A + TAIL_B = 1.
    #[inline]
    pub fn rankine(x: f64) -> f64 {
        if x < 1.0 { x } else { 1.0 / (x * x * VORTEX_TAIL_A + VORTEX_TAIL_B) }
    }

    /// Funnel depression (negative height contribution) in meters.
    pub fn depression_m(&self, p: DVec2) -> f64 {
        if self.strength_m <= VORTEX_ACTIVE_EPS {
            return 0.0;
        }
        let x = self.normalized_radius(p);
        -self.strength_m / (1.0 + x * x * VORTEX_DEPRESSION_K)
    }

    /// Shear-driven foam seed, SATURATING at VORTEX_SHEAR_CAP.
    pub fn shear(&self, p: DVec2) -> f64 {
        if self.strength_m <= VORTEX_ACTIVE_EPS {
            return 0.0;
        }
        let x = self.normalized_radius(p);
        Vortex::rankine(x)
            * (-x * x * VORTEX_ENV_K).exp()
            * (self.strength_m * VORTEX_SHEAR_PER_STRENGTH).min(VORTEX_SHEAR_CAP)
    }

    /// Small positive swirl-height ripple riding the depression rim.
    pub fn swirl_height_m(&self, p: DVec2) -> f64 {
        if self.strength_m <= VORTEX_ACTIVE_EPS {
            return 0.0;
        }
        let x = self.normalized_radius(p);
        Vortex::rankine(x) * (-x * x * VORTEX_ENV_K).exp() * self.strength_m * VORTEX_HEIGHT_GAIN
    }

    /// Rotation angle (radians) applied to the wave-texture lookup at p and
    /// time t: the drain spiral for free. Uses the swirl-variant tail.
    pub fn swirl_angle(&self, p: DVec2, t_s: f64) -> f64 {
        if self.strength_m <= VORTEX_ACTIVE_EPS {
            return 0.0;
        }
        let x = self.normalized_radius(p);
        let vt = if x < 1.0 {
            x
        } else {
            1.0 / (x * x * VORTEX_SWIRL_TAIL_A + VORTEX_SWIRL_TAIL_B)
        };
        vt * (-x * x * VORTEX_SWIRL_ENV_K).exp() * self.strength_m * VORTEX_SWIRL_RATE * t_s
    }
}

impl RogueWave {
    #[inline]
    fn envelope(&self, p: DVec2) -> f64 {
        let d = p - self.center;
        let r = self.radius_m.max(1.0);
        (-d.dot(d) / (r * r)).exp()
    }

    #[inline]
    fn carrier_phase(&self, p: DVec2) -> f64 {
        let d = p - self.center;
        let k = std::f64::consts::TAU / self.wavelength_m.max(4.0);
        d.dot(self.dir) * k + self.phase
    }

    /// Normalized peaky carrier in [-1, 1]: three incommensurate modes,
    /// crest-sharpened so crests spike and troughs flatten.
    pub fn carrier(phase: f64) -> f64 {
        let g = (phase.cos()
            + (phase * ROGUE_MODE2_FREQ + ROGUE_MODE2_PHASE).cos() * ROGUE_MODE2_AMP
            + (phase * ROGUE_MODE3_FREQ + ROGUE_MODE3_PHASE).cos() * ROGUE_MODE3_AMP)
            / ROGUE_NORM;
        g.signum() * g.abs().powf(ROGUE_SHARPEN_POW)
    }

    /// Drawn height: full peaky group.
    pub fn height_m(&self, p: DVec2) -> f64 {
        if self.amp_m <= EVENT_ACTIVE_EPS {
            return 0.0;
        }
        Self::carrier(self.carrier_phase(p)) * self.envelope(p) * self.amp_m
    }

    /// Ride height for buoyancy / the camera: ENVELOPE ONLY, at
    /// ROGUE_RIDE_ENVELOPE_FRAC of the amplitude. Deliberately not the
    /// carrier: chasing its exact phase judders whatever rides it.
    pub fn ride_height_m(&self, p: DVec2) -> f64 {
        if self.amp_m <= EVENT_ACTIVE_EPS {
            return 0.0;
        }
        self.envelope(p) * self.amp_m * ROGUE_RIDE_ENVELOPE_FRAC
    }

    /// Full modifier: height, horizontal orbital push, crest foam band.
    pub fn modifiers(&self, p: DVec2) -> (f64, DVec2, f64) {
        if self.amp_m <= EVENT_ACTIVE_EPS {
            return (0.0, DVec2::ZERO, 0.0);
        }
        let env = self.envelope(p);
        let phase = self.carrier_phase(p);
        let peaky = Self::carrier(phase);
        let push = self.dir * (-phase.sin() * env * self.amp_m * ROGUE_PUSH_GAIN);
        let crest = smoothstep(0.55, 1.0, peaky) * env;
        (peaky * env * self.amp_m, push, crest)
    }
}

impl Hurricane {
    #[inline]
    fn normalized_radius(&self, p: DVec2) -> f64 {
        ((p - self.center).length() + 1e-3) / self.eye_m.max(HURRICANE_EYE_MIN_M)
    }

    /// Glassy-eye factor in [0, 1]: 1 at the eye center. Consumers use it to
    /// suppress chop, foam and spray inside the eye.
    pub fn calm(&self, p: DVec2) -> f64 {
        if self.intensity_m <= EVENT_ACTIVE_EPS {
            return 0.0;
        }
        let x = self.normalized_radius(p);
        (-x * x * HURRICANE_EYE_CALM_K).exp()
    }

    /// Eyewall swell ring minus the mild eye depression, in meters.
    pub fn height_m(&self, p: DVec2) -> f64 {
        if self.intensity_m <= EVENT_ACTIVE_EPS {
            return 0.0;
        }
        let x = self.normalized_radius(p);
        let ring = (-((x - HURRICANE_RING_POS) * HURRICANE_RING_WIDTH_K).powi(2)).exp();
        ring * self.intensity_m * HURRICANE_RING_GAIN
            - (-x * x * HURRICANE_EYE_CALM_K).exp() * self.intensity_m * HURRICANE_EYE_DEPRESSION
    }

    /// Rotation angle for the wave-texture lookup (the cyclonic drift of the
    /// whole sea state around the eye).
    pub fn swirl_angle(&self, p: DVec2, t_s: f64) -> f64 {
        if self.intensity_m <= EVENT_ACTIVE_EPS {
            return 0.0;
        }
        let x = self.normalized_radius(p);
        let vt = if x < 1.0 { x * HURRICANE_TANG_INNER } else { 1.0 / x.max(1e-3).powf(HURRICANE_TANG_OUT_POW) };
        vt * self.intensity_m * HURRICANE_SWIRL_RATE * t_s
    }
}

impl OceanEvents {
    pub fn any_active(&self) -> bool {
        self.vortices.iter().any(|v| v.strength_m > VORTEX_ACTIVE_EPS)
            || self.solitons.iter().any(|s| s.amp_m > EVENT_ACTIVE_EPS)
            || self.rogue.amp_m > EVENT_ACTIVE_EPS
            || self.hurricane.intensity_m > EVENT_ACTIVE_EPS
    }

    /// Vertical-only event height in meters: the DRAWN sum (rogue carrier
    /// included). This is the cheap form the rung-2 vertex shader can also
    /// iterate against to keep a 30 m wall properly tessellated.
    pub fn event_height_m(&self, p: DVec2) -> f64 {
        let mut h = 0.0;
        for v in &self.vortices {
            h += v.depression_m(p);
        }
        for s in &self.solitons {
            h += s.height_m(p);
        }
        h += self.rogue.height_m(p);
        h += self.hurricane.height_m(p);
        h
    }

    /// Ride height in meters for buoyancy, the float clamp and the camera:
    /// identical to event_height_m EXCEPT the rogue wave, which contributes
    /// its envelope only (see ROGUE_RIDE_ENVELOPE_FRAC).
    pub fn ride_height_m(&self, p: DVec2) -> f64 {
        let mut h = 0.0;
        for v in &self.vortices {
            h += v.depression_m(p);
        }
        for s in &self.solitons {
            h += s.height_m(p);
        }
        h += self.rogue.ride_height_m(p);
        h += self.hurricane.height_m(p);
        h
    }

    /// The full analytic modifier stack the GPU adds on top of the wave sea.
    /// The rung-2 WGSL twin reproduces exactly this.
    pub fn modifiers(&self, p: DVec2) -> EventModifiers {
        let mut m = EventModifiers::default();
        for v in &self.vortices {
            m.disp.y += v.depression_m(p);
            m.crest += v.shear(p);
        }
        for s in &self.solitons {
            let (h, push, crest) = s.modifiers(p);
            m.disp.y += h;
            m.disp.x += push.x;
            m.disp.z += push.y;
            m.crest = m.crest.max(crest);
        }
        let (h, push, crest) = self.rogue.modifiers(p);
        m.disp.y += h;
        m.disp.x += push.x;
        m.disp.z += push.y;
        m.crest = m.crest.max(crest);

        m.disp.y += self.hurricane.height_m(p);
        m.calm = self.hurricane.calm(p);
        m
    }

    /// Rotate a wave-texture lookup coordinate about every active vortex and
    /// the hurricane by their time-growing swirl angles. Pure isometry per
    /// event (radius to each center is preserved), so the FFT sea appears to
    /// spiral in without any extra simulation.
    pub fn swirl_coords(&self, p: DVec2, t_s: f64) -> DVec2 {
        let mut q = p;
        for v in &self.vortices {
            if v.strength_m <= VORTEX_ACTIVE_EPS {
                continue;
            }
            let ang = v.swirl_angle(q, t_s);
            let d = q - v.center;
            let (s, c) = ang.sin_cos();
            q = v.center + DVec2::new(c * d.x - s * d.y, s * d.x + c * d.y);
        }
        if self.hurricane.intensity_m > EVENT_ACTIVE_EPS {
            let ang = self.hurricane.swirl_angle(q, t_s);
            let d = q - self.hurricane.center;
            let (s, c) = ang.sin_cos();
            q = self.hurricane.center + DVec2::new(c * d.x - s * d.y, s * d.x + c * d.y);
        }
        q
    }

    /// Pack every event into the rung-2 uniform layout, downcasting to f32
    /// at this local-frame boundary (the f32-at-planet-scale rule: positions
    /// here are event-frame offsets, tens of km at most). Layout, one vec4
    /// per row:
    ///   0..4  vortices        (center.x, center.y, radius_m, strength_m)
    ///   4,6   soliton A/B a   (dir.x, dir.y, crest_dist_m, amp_m)
    ///   5,7   soliton A/B b   (width_m, steep, lateral_m, 0 [speed lives
    ///                          CPU-side at rung 3; slot reserved])
    ///   8     rogue a         (center.x, center.y, radius_m, amp_m)
    ///   9     rogue b         (dir.x, dir.y, wavelength_m, phase)
    ///   10    hurricane       (center.x, center.y, eye_m, intensity_m)
    pub fn to_uniform_vec4s(&self) -> [[f32; 4]; 11] {
        let mut u = [[0.0f32; 4]; 11];
        for (i, v) in self.vortices.iter().enumerate() {
            u[i] = [v.center.x as f32, v.center.y as f32, v.radius_m as f32, v.strength_m as f32];
        }
        for (i, s) in self.solitons.iter().enumerate() {
            u[4 + i * 2] = [s.dir.x as f32, s.dir.y as f32, s.crest_dist_m as f32, s.amp_m as f32];
            u[5 + i * 2] = [s.width_m as f32, s.steep as f32, s.lateral_m as f32, 0.0];
        }
        u[8] = [
            self.rogue.center.x as f32,
            self.rogue.center.y as f32,
            self.rogue.radius_m as f32,
            self.rogue.amp_m as f32,
        ];
        u[9] = [
            self.rogue.dir.x as f32,
            self.rogue.dir.y as f32,
            self.rogue.wavelength_m as f32,
            self.rogue.phase as f32,
        ];
        u[10] = [
            self.hurricane.center.x as f32,
            self.hurricane.center.y as f32,
            self.hurricane.eye_m as f32,
            self.hurricane.intensity_m as f32,
        ];
        u
    }
}

// ------------------------------------------------------- frame + GPU bridge

/// The event tangent frame: a local east-north basis on the planet sphere,
/// anchored at the event site in PLANET-MODEL coordinates (the same frame the
/// water vertex shader's `p_m = dir * r` lives in, so the anchor stays glued
/// to the sea as the planet spins). All event math runs in this frame's 2D
/// coordinates; the f32 downcast happens at the uniform boundary where
/// magnitudes are anchor-relative.
#[derive(Clone, Copy, Debug)]
pub struct OceanEventFrame {
    /// Anchor point in planet-model meters (on or near the sea surface).
    pub anchor: DVec3,
    /// Unit east basis (tangent, perpendicular to up).
    pub east: DVec3,
    /// Unit north basis (tangent, completes the right-handed set).
    pub north: DVec3,
}

impl OceanEventFrame {
    /// Build the tangent frame at a planet-model surface point. Pole-safe:
    /// the reference axis flips when the anchor is nearly polar.
    pub fn at(anchor: DVec3) -> Self {
        let up = anchor.normalize_or_zero();
        let reference =
            if up.y.abs() > 0.99 { DVec3::new(1.0, 0.0, 0.0) } else { DVec3::new(0.0, 1.0, 0.0) };
        let east = reference.cross(up).normalize_or_zero();
        let north = up.cross(east);
        Self { anchor, east, north }
    }

    /// Project a planet-model position into the frame's 2D event
    /// coordinates. Do this in f64; the chord-vs-arc error is negligible at
    /// event ranges (tens of km at most).
    pub fn project(&self, p_planet: DVec3) -> DVec2 {
        let rel = p_planet - self.anchor;
        DVec2::new(rel.dot(self.east), rel.dot(self.north))
    }
}

/// An event set bound to its tangent frame: what the engine holds while
/// events are live (today the showcase dev pin; the rung-3 lifecycle will
/// own the same type).
#[derive(Clone, Copy, Debug)]
pub struct PinnedOceanEvents {
    pub events: OceanEvents,
    pub frame: OceanEventFrame,
}

impl PinnedOceanEvents {
    /// The camera-uniform block: rows 0..10 are `to_uniform_vec4s`, row 11 is
    /// the frame anchor (planet-model meters) with w = active flag, rows
    /// 12/13 are the east/north basis (w reserved: 12.w will carry the swirl
    /// clock when the shading rung lands). The WGSL twin reads exactly this
    /// layout from `camera.ocean_event`.
    pub const UNIFORM_ROWS: usize = 14;

    pub fn to_camera_rows(&self) -> [[f32; 4]; Self::UNIFORM_ROWS] {
        let mut rows = [[0.0f32; 4]; Self::UNIFORM_ROWS];
        rows[..11].copy_from_slice(&self.events.to_uniform_vec4s());
        let a = self.frame.anchor;
        rows[11] = [a.x as f32, a.y as f32, a.z as f32, 1.0];
        let e = self.frame.east;
        rows[12] = [e.x as f32, e.y as f32, e.z as f32, 0.0];
        let n = self.frame.north;
        rows[13] = [n.x as f32, n.y as f32, n.z as f32, 0.0];
        rows
    }

    /// Worst-case |event height| for this set, for crest/bounds publishing.
    pub fn max_abs_height_m(&self) -> f64 {
        let mut m: f64 = 0.0;
        for v in &self.events.vortices {
            m = m.max(v.strength_m);
        }
        for s in &self.events.solitons {
            m = m.max(s.amp_m);
        }
        m = m.max(self.events.rogue.amp_m);
        // Eyewall ring peaks at RING_GAIN * intensity.
        m = m.max(self.events.hurricane.intensity_m * HURRICANE_RING_GAIN);
        m
    }
}

/// Dev-pin event sets for the showcase/probe rig ({"ocean_event":"tsunami"}
/// etc.) - permanent dev tooling, not game content (the rung-3 lifecycle
/// reads real parameters from data/weather/events.ron). Values are ABYSSAL's
/// demo vectors with amplitudes clamped to the water patches' +-12 m radial
/// culling band (ocean_waves::MAX_SEA_HEIGHT_M); full-scale 34 m walls need
/// the dynamic bounds that ship with the lifecycle rung. The event sits at
/// the frame origin; `toward` should point from the event toward the viewer
/// (in frame coordinates) so a soliton wall faces the camera.
pub fn dev_pin_events(kind: &str, toward: DVec2) -> Option<OceanEvents> {
    let dir = if toward.length_squared() > 1e-12 { toward.normalize() } else { DVec2::new(1.0, 0.0) };
    let mut ev = OceanEvents::default();
    match kind {
        "tsunami" => {
            ev.solitons[0] = Soliton {
                dir,
                crest_dist_m: 0.0,
                amp_m: 10.0,
                width_m: 150.0,
                steep: 1.2,
                lateral_m: 9000.0,
            };
        }
        "rogue" => {
            ev.rogue = RogueWave {
                center: DVec2::ZERO,
                radius_m: 240.0,
                amp_m: 10.0,
                dir,
                wavelength_m: 420.0,
                phase: 0.0,
            };
        }
        "maelstrom" => {
            ev.vortices[0] = Vortex { center: DVec2::ZERO, radius_m: 70.0, strength_m: 10.0 };
        }
        "hurricane" => {
            // Ring gain 3x: intensity 3.5 keeps the eyewall ring near 10 m.
            ev.hurricane = Hurricane { center: DVec2::ZERO, eye_m: 900.0, intensity_m: 3.5 };
        }
        _ => return None,
    }
    Some(ev)
}

// -------------------------------------------------------------------- tests
//
// The numeric vectors below are ABYSSAL's demo parameters, used purely as
// pinned references so a retune of any adopted constant fails loudly here
// (and, at rung 2, in the WGSL lockstep test).

#[cfg(test)]
mod tests {
    use super::*;

    fn demo_soliton() -> Soliton {
        // ABYSSAL tsunami defaults: amp 34 m, width 150 m, steep 1.2,
        // lateral 9000 m.
        Soliton {
            dir: DVec2::new(1.0, 0.0),
            crest_dist_m: 0.0,
            amp_m: 34.0,
            width_m: 150.0,
            steep: 1.2,
            lateral_m: 9000.0,
        }
    }

    #[test]
    fn soliton_peak_is_normalized() {
        // The drawdown term reaches back to the crest a little:
        // sech(1.6/1.1)^2 * 0.16 * 1.2 = 0.038, so the peak is ~0.962.
        let p = Soliton::profile(0.0, 150.0, 1.2);
        assert!(p > 0.95 && p <= 1.0, "profile(0) = {p}");
    }

    #[test]
    fn soliton_front_face_is_steeper_than_back() {
        // Shoaling compression: at half a width AHEAD the profile must have
        // dropped far below the same distance BEHIND.
        let front = Soliton::profile(75.0, 150.0, 1.2);
        let back = Soliton::profile(-75.0, 150.0, 1.2);
        assert!(front < back * 0.5, "front {front} back {back}");
    }

    #[test]
    fn soliton_drawdown_precedes_the_wall() {
        // The receding-sea precursor: negative height ahead of the face at
        // the drawdown center, 1.6 widths out.
        let d = Soliton::profile(150.0 * SOLITON_DRAWDOWN_AHEAD_WIDTHS, 150.0, 1.2);
        assert!(d < -0.1, "drawdown {d}");
    }

    #[test]
    fn soliton_sech_clamp_keeps_far_field_finite_and_dead() {
        let far = Soliton::profile(1.0e12, 150.0, 1.2);
        assert!(far.is_finite() && far.abs() < 1e-8, "far field {far}");
    }

    #[test]
    fn soliton_crest_foam_sits_on_the_lip_biased_forward() {
        let s = demo_soliton();
        let (_, _, at_crest) = s.modifiers(DVec2::new(0.0, 0.0));
        let (_, _, behind) = s.modifiers(DVec2::new(-120.0, 0.0));
        assert!(at_crest > 0.0, "crest foam {at_crest}");
        assert!(behind < at_crest, "behind {behind} at {at_crest}");
    }

    #[test]
    fn rogue_wavelength_speed_pairing_is_deep_water_consistent() {
        // ABYSSAL pairs wavelength 420 m with 26 m/s; the dispersion
        // relation says 25.6.
        let c = deep_water_phase_speed_mps(420.0);
        assert!((c - 25.6).abs() < 0.2, "c = {c}");
    }

    #[test]
    fn rogue_ride_is_the_envelope_not_the_carrier() {
        let r = RogueWave {
            center: DVec2::ZERO,
            radius_m: 240.0,
            amp_m: 27.0,
            dir: DVec2::new(1.0, 0.0),
            wavelength_m: 420.0,
            phase: 0.0,
        };
        // Ride at the center is exactly the envelope fraction, and it never
        // oscillates along the carrier.
        let ride0 = r.ride_height_m(DVec2::ZERO);
        assert!((ride0 - 27.0 * ROGUE_RIDE_ENVELOPE_FRAC).abs() < 1e-9);
        let mut min_ride = f64::INFINITY;
        let mut max_ride = f64::NEG_INFINITY;
        let mut max_drawn: f64 = 0.0;
        for i in 0..400 {
            let p = DVec2::new(i as f64 * 0.5, 0.0);
            let env = (-(p.x * p.x) / (240.0 * 240.0)).exp();
            let ride = r.ride_height_m(p) / env;
            min_ride = min_ride.min(ride);
            max_ride = max_ride.max(ride);
            max_drawn = max_drawn.max(r.height_m(p).abs());
        }
        assert!(max_ride - min_ride < 1e-9, "ride oscillates: {min_ride}..{max_ride}");
        assert!(max_drawn > ride0, "drawn peak {max_drawn} should exceed ride {ride0}");
    }

    #[test]
    fn rankine_is_continuous_at_the_core_edge() {
        // TAIL_A + TAIL_B = 1 makes the solid-body and tail branches meet
        // exactly at x = 1.
        let inner = Vortex::rankine(1.0 - 1e-9);
        let outer = Vortex::rankine(1.0 + 1e-9);
        assert!((inner - outer).abs() < 1e-6, "inner {inner} outer {outer}");
        assert!((inner - 1.0).abs() < 1e-6);
    }

    #[test]
    fn vortex_shear_saturates_so_maelstrom_arms_stay_arms() {
        // ABYSSAL whirlpool: radius 70 m, strength 34. A 10x stronger one
        // must not ask for 10x the foam.
        let base = Vortex { center: DVec2::ZERO, radius_m: 70.0, strength_m: 34.0 };
        let monster = Vortex { center: DVec2::ZERO, radius_m: 70.0, strength_m: 340.0 };
        let p = DVec2::new(70.0, 0.0);
        let ratio = monster.shear(p) / base.shear(p);
        assert!(ratio < 1.01, "shear did not saturate: ratio {ratio}");
        // The demo whirlpool (34 * 0.022 = 0.75) already sits AT the cap,
        // which is the point of the cap; a weak eddy sits below it.
        assert!((10.0f64 * VORTEX_SHEAR_PER_STRENGTH).min(VORTEX_SHEAR_CAP) < VORTEX_SHEAR_CAP);
        assert!((34.0f64 * VORTEX_SHEAR_PER_STRENGTH).min(VORTEX_SHEAR_CAP) == VORTEX_SHEAR_CAP);
    }

    #[test]
    fn hurricane_eye_is_glassy_and_ringed() {
        // ABYSSAL hurricane: eye 900 m, intensity 26.
        let h = Hurricane { center: DVec2::ZERO, eye_m: 900.0, intensity_m: 26.0 };
        assert!(h.calm(DVec2::ZERO) > 0.999, "eye calm {}", h.calm(DVec2::ZERO));
        assert!(h.height_m(DVec2::ZERO) < 0.0, "eye should sit in a depression");
        // The eyewall ring peaks near RING_POS eye radii.
        let mut best_r = 0.0;
        let mut best_h = f64::NEG_INFINITY;
        for i in 1..400 {
            let r = i as f64 * 10.0;
            let hh = h.height_m(DVec2::new(r, 0.0));
            if hh > best_h {
                best_h = hh;
                best_r = r;
            }
        }
        let x = best_r / 900.0;
        assert!(x > 1.1 && x < 1.4, "ring peak at {x} eye radii");
        assert!(best_h > 26.0, "ring height {best_h} should exceed intensity");
    }

    #[test]
    fn swirl_is_an_isometry_about_each_center() {
        let mut ev = OceanEvents::default();
        ev.vortices[0] = Vortex { center: DVec2::new(50.0, -20.0), radius_m: 70.0, strength_m: 34.0 };
        let p = DVec2::new(120.0, 35.0);
        let q = ev.swirl_coords(p, 8.0);
        let before = (p - ev.vortices[0].center).length();
        let after = (q - ev.vortices[0].center).length();
        assert!((before - after).abs() < 1e-9, "radius changed {before} -> {after}");
        assert!((p - q).length() > 1.0, "swirl at t=8 s should visibly rotate");
    }

    #[test]
    fn drawn_and_ride_heights_agree_except_for_the_rogue() {
        let mut ev = OceanEvents::default();
        ev.solitons[0] = demo_soliton();
        ev.vortices[0] = Vortex { center: DVec2::new(300.0, 0.0), radius_m: 70.0, strength_m: 34.0 };
        ev.hurricane = Hurricane { center: DVec2::new(-2000.0, 500.0), eye_m: 900.0, intensity_m: 26.0 };
        let p = DVec2::new(80.0, 40.0);
        assert!((ev.event_height_m(p) - ev.ride_height_m(p)).abs() < 1e-12);
        // Vertical-only must equal the full stack's vertical component.
        assert!((ev.event_height_m(p) - ev.modifiers(p).disp.y).abs() < 1e-12);

        ev.rogue = RogueWave {
            center: DVec2::new(60.0, 30.0),
            radius_m: 240.0,
            amp_m: 27.0,
            dir: DVec2::new(1.0, 0.0),
            wavelength_m: 420.0,
            phase: 0.3,
        };
        // With a rogue active the two heights legitimately differ.
        assert!((ev.event_height_m(p) - ev.ride_height_m(p)).abs() > 0.1);
    }

    #[test]
    fn default_is_dead_calm() {
        let ev = OceanEvents::default();
        assert!(!ev.any_active());
        let p = DVec2::new(123.0, -456.0);
        assert_eq!(ev.event_height_m(p), 0.0);
        assert_eq!(ev.ride_height_m(p), 0.0);
        let m = ev.modifiers(p);
        assert_eq!(m.disp, DVec3::ZERO);
        assert_eq!(m.crest, 0.0);
        assert_eq!(m.calm, 0.0);
        assert_eq!(ev.swirl_coords(p, 100.0), p);
    }

    #[test]
    fn tangent_frame_projects_its_own_axes() {
        // Frame at a mid-latitude point on an Earth-radius sphere.
        let anchor = DVec3::new(3.2e6, 4.1e6, 3.4e6);
        let f = OceanEventFrame::at(anchor);
        // Basis sanity: unit, orthogonal, tangent.
        assert!((f.east.length() - 1.0).abs() < 1e-12);
        assert!((f.north.length() - 1.0).abs() < 1e-12);
        assert!(f.east.dot(f.north).abs() < 1e-12);
        assert!(f.east.dot(anchor.normalize()).abs() < 1e-12);
        // A step along east projects to (step, 0); along north to (0, step).
        let p = f.project(anchor + f.east * 137.0 + f.north * -41.0);
        assert!((p.x - 137.0).abs() < 1e-9 && (p.y + 41.0).abs() < 1e-9, "{p:?}");
        // Pole-safe: a polar anchor still yields a real basis.
        let fp = OceanEventFrame::at(DVec3::new(0.0, 6.37e6, 0.0));
        assert!((fp.east.length() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn dev_pins_fit_the_patch_band_and_face_the_viewer() {
        for kind in ["tsunami", "rogue", "maelstrom", "hurricane"] {
            let ev = dev_pin_events(kind, DVec2::new(0.0, 1.0)).expect(kind);
            assert!(ev.any_active(), "{kind} inactive");
            let pin = PinnedOceanEvents { events: ev, frame: OceanEventFrame::at(DVec3::new(0.0, 6.37e6, 0.0)) };
            // Every dev pin stays inside the water patches' +-12 m radial
            // culling band (ocean_waves::MAX_SEA_HEIGHT_M).
            assert!(
                pin.max_abs_height_m() <= crate::terrain::ocean_waves::MAX_SEA_HEIGHT_M as f64,
                "{kind} exceeds the patch band: {}",
                pin.max_abs_height_m()
            );
        }
        assert!(dev_pin_events("nonsense", DVec2::ZERO).is_none());
    }

    #[test]
    fn camera_rows_carry_frame_and_flag() {
        let ev = dev_pin_events("tsunami", DVec2::new(1.0, 0.0)).unwrap();
        let frame = OceanEventFrame::at(DVec3::new(0.0, 6.37e6, 0.0));
        let pin = PinnedOceanEvents { events: ev, frame };
        let rows = pin.to_camera_rows();
        assert_eq!(rows[11][3], 1.0, "active flag");
        assert!((rows[11][1] - 6.37e6).abs() < 1.0, "anchor y");
        // Basis rows are unit vectors after the f32 downcast.
        for r in [12, 13] {
            let l = (rows[r][0] * rows[r][0] + rows[r][1] * rows[r][1] + rows[r][2] * rows[r][2])
                .sqrt();
            assert!((l - 1.0).abs() < 1e-5, "row {r} length {l}");
        }
        // Soliton A rides rows 4/5 with the demo vector.
        assert_eq!(rows[4][3], 10.0);
        assert_eq!(rows[5][0], 150.0);
    }

    /// The rung-2 WGSL twin lockstep guard, in the ocean_waves.rs pattern:
    /// parse the vertex shader source and assert every adopted constant
    /// appears in the ocean-event block, so a shader-side retune that
    /// forgets this file fails the build immediately.
    #[test]
    fn wgsl_twin_pins_the_adopted_constants() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/shaders/pbr/00-bindings-vertex.wgsl");
        let src = std::fs::read_to_string(path).expect("read 00-bindings-vertex.wgsl");
        let start = src.find("fn ocean_event_p2").expect("ocean_event block missing from WGSL");
        let end = src.find("fn water_disp_height").expect("water_disp_height anchor");
        let block = &src[start..end];
        for needle in [
            // sech clamp (SECH_ARG_CLAMP)
            "-12.0, 12.0",
            // soliton: shoal compression, drawdown center/width/depth
            "1.35", "* 1.6)", "* 1.1)", "0.16",
            // vortex depression falloff + activity epsilons
            "2.2", "0.0001", "0.001",
            // rogue carrier modes, normalizer, sharpening
            "1.87", "0.42", "0.61", "0.55", "1.97", "0.72",
            // hurricane ring position/width/gain, eye calm/depression, floor
            "1.25", "1.4", "3.0", "1.6)", "0.4", "50.0",
        ] {
            assert!(block.contains(needle), "WGSL ocean-event block lost constant {needle}");
        }
        // And the uniform bridge rows exist.
        assert!(src.contains("ocean_event: array<vec4<f32>, 14>"), "uniform array missing");
    }

    #[test]
    fn uniform_packing_matches_the_documented_layout() {
        let mut ev = OceanEvents::default();
        ev.solitons[1] = demo_soliton();
        ev.hurricane = Hurricane { center: DVec2::new(-2000.0, 500.0), eye_m: 900.0, intensity_m: 26.0 };
        let u = ev.to_uniform_vec4s();
        assert_eq!(u[6], [1.0, 0.0, 0.0, 34.0]); // soliton B row a
        assert_eq!(u[7], [150.0, 1.2, 9000.0, 0.0]); // soliton B row b
        assert_eq!(u[10], [-2000.0, 500.0, 900.0, 26.0]); // hurricane
        assert_eq!(u[0], [0.0; 4]); // inactive vortex slot stays zeroed
    }
}
