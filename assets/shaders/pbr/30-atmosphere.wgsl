// ── Analytic atmosphere scattering (material type 14, v0.807) ──
//
// Single-scattering approximation evaluated per fragment on the oversized
// atmosphere shell sphere (O'Neil-class: a short numeric march along the
// view ray with an ANALYTIC Chapman-function optical depth toward the sun,
// so there is no nested sampling loop and no precomputed LUT). All positions
// are normalized to SHELL RADII (shell boundary = 1.0) before any math: at
// planetary magnitudes (1e7..1e11 m) the raw world-space ray-sphere terms
// would shred f32 precision, while in shell units everything stays O(1e3).
//
// Look targets (verify by flying at Earth):
//  (a) from space: a thin bright blue limb hugging the horizon;
//  (b) the day side brightens toward the sun and the terminator fades warm
//      (Mie forward lobe + Rayleigh-reddened sun transmittance);
//  (c) the night-side atmosphere is nearly invisible (sun transmittance
//      kills in-scatter; the remaining alpha only darkens, never glows);
//  (d) from INSIDE the atmosphere: deep blue zenith, pale bright horizon.
//      The same math handles it -- the ray segment start is clamped to the
//      camera position whenever the camera is within the shell.
//
// Material packing (producer: lib.rs planet_atmo_materials; Rust mirror +
// unit tests: src/renderer/atmosphere.rs -- keep the constants in sync):
//   base_color.rgb  relative per-channel scattering strengths (LINEAR, the
//                   planet RON's atmosphere_color.rgb verbatim). The mapping
//                   is: per-channel vertical optical depth = rgb * alpha *
//                   ATMO_TAU_RAYLEIGH, and beta = depth / scale height. So a
//                   blue-dominant color scatters blue hardest = blue sky +
//                   warm sunsets (Earth), while a red-dominant color gives a
//                   butterscotch sky (Mars). Any modded planet just works.
//   base_color.a    overall density multiplier (atmosphere_color alpha)
//   params.x        planet radius / shell radius
//   params.y        density scale height / shell radius
//   params.z        14.0 (this shader type)

const ATMO_SAMPLES: i32 = 12;
// Vertical optical depth contributed by a 1.0-strength color channel at
// density (alpha) 1.0. Earth's real blue-channel Rayleigh depth is ~0.28;
// earth.ron ships color.b = 1.0, alpha = 0.5, so 1.0 * 0.5 * 0.6 = 0.30.
const ATMO_TAU_RAYLEIGH: f32 = 0.6;
// Mie (aerosol haze) vertical depth at density 1.0: small, gray, strongly
// forward-scattering; supplies the warm glow around the sun near the limb.
const ATMO_TAU_MIE: f32 = 0.02;
const ATMO_MIE_G: f32 = 0.76;
// Radiance-to-display multiplier: THE artistic brightness knob. Raising it
// brightens limb + sky; the surface stays readable regardless because this
// path only ever alpha-blends (never additive white-out).
const ATMO_EXPOSURE: f32 = 4.0;
// Close-range tune (v0.815): ATMO_EXPOSURE was calibrated against BLACK SPACE
// -- the from-orbit limb and far-disc tint, which the operator approved. But
// the same 4x in-scatter boost applied to rays that TERMINATE ON THE LIT
// SURFACE floods the view once the planet fills the screen (verified capture
// at 400 km: the whole disc washed pale). The in-scatter is boosted 4x while
// the surface behind it is not, so haze contrast is exaggerated 4x exactly
// where the eye wants ground detail. Fix: per fragment, blend the exposure
// between a calm surface value and the full limb value using two weights,
// taking the MAX of:
//  (a) limb weight -- rays that miss the planet (or graze within half a
//      shell thickness of the limb) keep the FULL exposure, so the blue limb
//      glow and the ground-level sky/horizon gradient never change;
//  (b) distance weight -- cameras beyond ATMO_FAR_R shell radii keep the
//      full exposure on the WHOLE disc, so the approved 12,000 km blue-marble
//      look is bit-identical; the disc clears smoothly on approach between
//      FAR_R and NEAR_R (reads as detail resolving, no popping).
// Mirror + unit tests: renderer::atmosphere::atmo_exposure.
const ATMO_EXPOSURE_NEAR: f32 = 1.4;
const ATMO_NEAR_R: f32 = 1.25;
const ATMO_FAR_R: f32 = 2.5;
// Low-altitude aerial-perspective trim (v0.826): from a near camera the long,
// near-horizontal path to a surface point piles up in-scatter and opacity,
// veiling the coast + ocean under a milky wash (the operator's "washed out"
// complaint at 0.4-3 km over Oahu). Scaling the returned ALPHA by this factor
// on those rays dims the additive haze AND lets the surface show through in one
// stroke (blended: out = mapped*k + surface*(1 - alpha*k)). Applied via
// near_surf = 1 - max(w_limb, w_far) -- EXACTLY the rays the exposure blend
// already calls "near surface", so it is 1.0 (no change) for limb rays,
// ground-level sky (upward, w_limb=1), and any far camera (w_far=1). The
// approved from-orbit limb + 12,000 km disc stay bit-identical. Mirror + tests:
// renderer::atmosphere::near_haze_scale.
const ATMO_NEAR_HAZE: f32 = 0.45;
// Ground-level sky-dome tier (v0.918, exposure calibration / research item 4):
// miss-the-planet rays from a camera INSIDE the shell used to ride the full
// space-calibrated ATMO_EXPOSURE and ACES-clipped a broad band of the dome to
// white (the operator's washed sky). The dome tier ramps back to ATMO_EXPOSURE
// as the camera climbs out of the shell (w_alt in atmosphere_scattering), so
// the 400 km limb glow and every from-orbit look stay bit-identical.
// Mirror + tests: renderer::atmosphere::EXPOSURE_DOME.
const ATMO_EXPOSURE_DOME: f32 = 1.7;
// Sky-view LUT radiance -> scene-radiance scale (stage 3c). The LUT is in
// sun-irradiance-=-1 units (CPU-twin tests: noon zenith green ~ 0.02); this
// lifts it into the ACES range the dome path lives in. Tuned on the rig.
const SKY_LUT_EXPOSURE: f32 = 15.0;
// Isotropic multiple-scatter bounce (v0.918): single scattering alone leaves
// the dimmer dome starved where the phase functions de-weight it (zenith away
// from the sun). One extra-bounce term with a flat phase rides the SAME
// per-channel path integral, restoring that energy without re-brightening the
// forward lobe. Gated by the same weight that lowers the dome, so it is
// exactly zero wherever the exposure is unchanged.
const ATMO_MS_ISO: f32 = 0.07;

// NOTE (v0.988): v0.986 briefly added an ATMO_MID_VEIL gain here that
// RAISED surface-ray alpha from mid-disc outward, chasing the classic
// blue-marble photo veil. Reverted same-day: the PRIORITIES want it
// implemented actually asked to THIN the mid-disc opacity (the operator's
// v0.956 correction - "the blue of the atmosphere completely hides the
// terrain on the edges" - and v0.956's surface-ray fix already shipped
// that direction). If a photo-style veil is ever wanted, re-propose it to
// the operator as a question first; the v0.986 release notes hold the
// implementation sketch.

// Scaled complementary error function erfcx(z) = exp(z^2) * erfc(z) for
// z >= 0, the kernel of the Chapman function below. Two branches, both
// sub-percent (verified in renderer::atmosphere against brute force):
//  - z <= 2.5: Abramowitz-Stegun 7.1.26. Its erfc polynomial carries an
//    exp(-z^2) factor that cancels our exp(z^2) EXACTLY, leaving a pure
//    polynomial. (Its ABSOLUTE erfc error of 1.5e-7 becomes a huge RELATIVE
//    error once multiplied by exp(z^2), which is why large z must switch.)
//  - z > 2.5: the 3-term asymptotic series 1/(sqrt(pi) z) (1 - 1/(2z^2)
//    + 3/(4z^4)), which is where erfc's absolute smallness lives.
fn atmo_erfcx(z: f32) -> f32 {
    if (z <= 2.5) {
        let t = 1.0 / (1.0 + 0.3275911 * z);
        return t
            * (0.254829592
                + t * (-0.284496736 + t * (1.421413741 + t * (-1.453152027 + t * 1.061405429))));
    }
    let inv_z2 = 1.0 / (z * z);
    return 0.5641896 / z * (1.0 + inv_z2 * (-0.5 + 0.75 * inv_z2));
}

// Closed-form Chapman function: relative slant-path air mass at radius x
// (in SCALE HEIGHTS) for zenith cosine mu >= 0, via the large-x asymptotic
// Ch(x, mu) = sqrt(pi*x/2) * erfcx(mu * sqrt(x/2)). ~1 at the zenith,
// sqrt(pi*x/2) at the horizon; ~0.1% error for planetary x (hundreds+),
// tested in Rust against brute-force integration (renderer::atmosphere).
// A simpler rational interpolation was tried first and missed by ~10% at
// mid angles -- a visibly wrong mid-sky -- hence the erfcx machinery.
fn atmo_chapman(x: f32, mu: f32) -> f32 {
    return sqrt(1.5707964 * x) * atmo_erfcx(mu * sqrt(0.5 * x));
}

// Density-integrated path length (units: shell radii at surface density)
// from radius r along zenith cosine mu out to space, for an exponential
// atmosphere over planet radius rp with scale height h. Rays dipping below
// the planet surface return a huge depth (sun geometrically occluded); the
// terminator still fades smoothly because the near-grazing depths are
// already enormous before the hard cutoff engages. Accuracy vs brute-force
// numeric integration: a few percent (unit-tested in renderer::atmosphere).
fn atmo_od_to_space(r: f32, mu: f32, rp: f32, h: f32) -> f32 {
    let x = r / h;
    let alt = max(r - rp, 0.0) / h;
    if (mu >= 0.0) {
        return h * exp(-alt) * atmo_chapman(x, mu);
    }
    // Downward ray: mirror the path at the tangent point (lowest radius on
    // the ray) -- down-leg = 2x the horizontal integral there minus the
    // up-leg we did not traverse.
    let sin_chi = sqrt(max(1.0 - mu * mu, 0.0));
    let rt = r * sin_chi;
    if (rt < rp) {
        return 1.0e9;
    }
    let alt_t = (rt - rp) / h;
    let horiz_t = h * exp(-alt_t) * atmo_chapman(rt / h, 0.0);
    return max(2.0 * horiz_t - h * exp(-alt) * atmo_chapman(x, -mu), 0.0);
}

// Rayleigh phase 3/(16pi)(1 + cos^2 theta); integrates to 1 over the sphere.
fn atmo_rayleigh_phase(c: f32) -> f32 {
    return 0.0596831 * (1.0 + c * c);
}

// Henyey-Greenstein phase for the Mie forward lobe; integrates to 1.
fn atmo_mie_phase(c: f32) -> f32 {
    let g = ATMO_MIE_G;
    let denom = 1.0 + g * g - 2.0 * g * c;
    return (1.0 - g * g) / (12.566371 * denom * sqrt(denom));
}

// ── THE AURORA (v0.1331) ──────────────────────────────────────────────────
//
// The second consumer of the environment region buffer, and a fair test of
// whether that mechanism generalises: an aurora shares nothing with a storm
// except having a place and a size, and it needed no new uniform channel.
//
// It is emission inside the air, so it belongs here rather than in a shell of
// its own: the view ray already has its atmosphere chord, and integrating
// along it gives limb brightening for free. That is why a real aurora looks
// like a bright arc on the edge of the disc from orbit and a faint glow
// straight down: the same emissivity, a longer path through it.
//
// A region of band 2 carries its geometry in the payload:
//   params.x  inner edge of the oval, radians from the pole
//   params.y  outer edge, radians (an oval is a RING, not a cap)
//   params.z  bottom of the emitting layer, fraction of shell thickness
//   params.w  top of it
//
// The direction is the spin axis, which is the one direction that reads the
// same in the body frame and in world space (bodies spin about +Y). Any
// NON-polar region compared here would have to undo the spin first.

/// Samples along the atmosphere chord. Pure ALU, no texture fetches, and the
/// whole loop is skipped for rays that cannot reach the layer at all.
const AURORA_STEPS: i32 = 7;
/// How many arcs the oval is made of. One reads as a drawn circle; several
/// that meander, cross and break is what a photograph actually shows.
const AURORA_STRANDS: i32 = 4;
// Half-thickness of one curtain, in radians of arc (about 2 km). A real
// discrete arc is a SHEET roughly 1 to 10 km thick standing on edge and
// 100 km or more tall; the strands used to be 100 to 150 km wide and only
// 90 km tall, a band lying on its side rather than a wall standing up.
// Nothing here draws it thinner than a pixel or a march step: see
// aurora_sheet below, which widens it to the footprint and dims it by the
// same factor, so its integrated light is conserved at every range.
const AURORA_SHEET_HALF: f32 = 3.0e-4;
// Emission density of a curtain relative to the diffuse glow. A discrete
// arc is far brighter per unit volume than diffuse aurora (tens of
// kilorayleighs against about one), and with the sheet now a few km thick
// rather than 150 km, its FACE only reads at all if it carries that.
const AURORA_SHEET_GAIN: f32 = 20.0;
/// How far a strand wanders in latitude, as a multiple of the oval half-width.
// 1.6, down from 3.0 when the strands became thin sheets. At 3.0 every strand
// wandered across the whole oval about the same centre line, so four thin
// lines braided like rope. Spread apart (AURORA_STRAND_SPACING) and meandering
// less, they run as separate parallel arcs that still cross now and then,
// which keeps the web the operator asked for on 2026-09-22.
const AURORA_MEANDER: f32 = 1.6;
// Spacing between neighbouring arcs, in half-widths of the oval.
const AURORA_STRAND_SPACING: f32 = 1.1;
/// Speed of the slow structural drift of the oval itself, per second of clock.
/// Deliberately small: a feature crossing the real oval at about 1 km/s takes
/// hours to travel round it, so the shape must barely move over a minute.
const AURORA_DRIFT: f32 = 0.02;
/// Speed of the broad curtain folds.
const AURORA_FOLD_SPEED: f32 = 0.25;
/// Speed of the fine ray shimmer. An order of magnitude faster than the drift,
/// because that is the split in the real thing: the arc creeps, the rays
/// flicker, and the flicker is the motion the eye reads as alive.
const AURORA_FLICKER: f32 = 1.6;
/// Speed of pulsating patches (real pulsating aurora cycles over 1 to 20 s).
const AURORA_PULSE: f32 = 0.55;
/// How many vertical rays fit around the whole oval.
///
/// Was 47, and that is the number that kept the curtain looking painted. 47
/// rays around a 40,000 km circle is one ray every 850 km, which is not a ray,
/// it is a lobe of the oval. Real auroral rays are field-aligned filaments
/// tens of km across. 1200 puts them at about 34 km, which reads as striation
/// from a few hundred km away and still resolves at several thousand.
const AURORA_RAY_LOBES: f32 = 8000.0;
/// Phase lean of a ray across the layer height. Rays follow the magnetic
/// field, which is close to vertical at these latitudes, so this is a slight
/// tilt and not a shear.
const AURORA_RAY_LEAN: f32 = 8.0;
/// What the ray term averages to over a cycle, which is what the detail fades
/// TO rather than fading to zero, so losing it at range costs no brightness.
/// pow(0.5 + 0.5 sin, 2) averages 3/8, times the mean of ray_amp (0.725) is
/// 0.272.
const AURORA_RAY_MEAN: f32 = 0.272;
/// Ray detail fades out below this many SCREEN PIXELS per stripe and is full
/// above the second. Distance was the wrong variable: a stripe is drawable or
/// not according to its width in pixels, which the field of view and the
/// resolution decide as much as the range does.
const AURORA_RAY_PX_LO: f32 = 2.0;
const AURORA_RAY_PX_HI: f32 = 5.0;
/// How far the DIFFUSE aurora spreads beyond the discrete arcs, as a multiple
/// of the oval's own half-width. Photographs from orbit show narrow bright
/// ribbons with much dimmer sheets fanning away from them, so the oval is
/// really two populations rather than one smooth band.
const AURORA_DIFFUSE_SPREAD: f32 = 5.0;
/// Brightness of those sheets relative to the arc.
// Lowered from 0.22 with the move to thin sheets. The diffuse glow is real
// (a broad faint emission equatorward of the discrete arcs) but at the old
// level it was a wide bright band of its own, which is half of what read as
// "a rubber band on its fat side".
const AURORA_DIFFUSE_LEVEL: f32 = 0.06;
/// The red 630 nm cap is real but far dimmer than the green ribbon.
///
/// Was 0.55, which with the old height profile gave red SEVENTY PERCENT of
/// the curtain and made it the dominant colour. Every reference photograph is
/// green-dominated: 557.7 nm is the brightest auroral line by a wide margin,
/// and the red cap only takes over in strong events and at heights where the
/// green has already stopped.
const AURORA_RED_LEVEL: f32 = 0.30;
/// Scales the integrated emission to screen radiance.
const AURORA_STRENGTH: f32 = 24.0;
/// 557.7 nm atomic oxygen: the green every photograph is dominated by.
const AURORA_GREEN: vec3<f32> = vec3<f32>(0.16, 1.0, 0.42);
/// 630 nm oxygen, which sits ABOVE the green and is thinner and redder.
const AURORA_RED: vec3<f32> = vec3<f32>(1.0, 0.26, 0.20);

/// Three incommensurate harmonics around the oval, in roughly -1..1.
///
/// Used for BOTH a strand path and its presence mask, with a different `seed`
/// per strand so no two follow the same course. Incommensurate on purpose: at
/// 3, 7 and 13 lobes the sum never repeats over a circuit, so the oval does not
/// read as a sine wave bent into a ring, which is what a single harmonic gives.
fn aurora_wave(phi: f32, t: f32, seed: f32) -> f32 {
    return sin(phi * 3.0 + t + seed) * 0.55
        + sin(phi * 7.0 - t * 0.61 + seed * 2.3) * 0.30
        + sin(phi * 13.0 + t * 0.37 + seed * 4.1) * 0.15;
}

// d/dphi of aurora_wave, analytically. A strand's centre line meanders with
// phi, so how fast a ray crosses the sheet depends on the slope of that
// meander as well as on the ray's own direction.
fn aurora_wave_d(phi: f32, t: f32, seed: f32) -> f32 {
    return cos(phi * 3.0 + t + seed) * 1.65
        + cos(phi * 7.0 - t * 0.61 + seed * 2.3) * 2.10
        + cos(phi * 13.0 + t * 0.37 + seed * 4.1) * 1.95;
}

// ── BOX-FILTERED SMOOTHSTEP (operator, 2026-09-24) ──
//
// "The close up one has a lot of banding to the coloring ... Or is that more
// of a layers stacking on top of layers?" It was exactly that. The march
// takes AURORA_STEPS point samples through the layer, and the green-to-red
// height ramp was evaluated at each sample's single height, so an oblique
// ray saw the ramp as a few flat colour slabs with hard steps between them.
//
// A step does not stand for a point; it stands for the whole segment it
// covers. So each step takes the ramp's AVERAGE over the height span of its
// own segment, which is a smooth function of the ray and has no steps in it.
// Exact rather than approximate, because smoothstep integrates in closed
// form: the integral of 3u^2 - 2u^3 is u^3 - u^4 / 2.
fn aurora_ss_int(u: f32) -> f32 {
    let x = clamp(u, 0.0, 1.0);
    return x * x * x - 0.5 * x * x * x * x + max(u - 1.0, 0.0);
}
fn aurora_ss_avg(e0: f32, e1: f32, a: f32, b: f32) -> f32 {
    let w = e1 - e0;
    if (abs(b - a) < 1.0e-5) {
        return smoothstep(e0, e1, 0.5 * (a + b));
    }
    return w * (aurora_ss_int((b - e0) / w) - aurora_ss_int((a - e0) / w)) / (b - a);
}

// ── ONE CURTAIN, AS A SHEET (operator, 2026-09-24) ──
//
// "Ours seem more like a rubberband on its fat side instead of thin side."
// Real curtains are thin sheets standing on edge, and that is what gives
// them their look: a ray that runs along the sheet (looking down its length,
// or straight down through it) collects light over the whole height and
// reads as a bright line or fold, while a ray crossing it face-on collects a
// few kilometres and reads faint. The strands used to be 100-150 km wide,
// so every ray collected plenty and the curtain structure could never show.
//
// A 4 km sheet is far thinner than a pixel from most of orbit, and thinner
// than a march step on an oblique ray, and point-sampling it would alias
// into exactly the shimmer the operator has been reporting. So the sheet
// is WIDENED to whichever is larger of its own half-thickness, half the
// distance the ray moves across it in one step, and one pixel, and DIMMED
// by the same factor. The bell's integral is proportional to its width, so
// the light it delivers is the same at every range; only how finely it is
// drawn changes.
//
// `g` is the signed angular distance from the sheet, `rate` how fast the
// ray changes it per unit of march distance, `pix` one pixel in radians.
fn aurora_sheet(g: f32, rate: f32, dt: f32, pix: f32) -> f32 {
    // The FULL step crossing, not half of it. Neighbouring samples along the
    // ray sit rate * dt apart in g, and a bell of half-width e sums to exactly
    // one across samples spaced e apart (smoothstep is symmetric: S(u) +
    // S(1 - u) = 1). So with e = rate * dt the samples tile the sheet with no
    // gaps and no overlap, and the delivered light is AURORA_SHEET_HALF / rate,
    // which is the true line integral of the sheet. With half the step, as
    // first written, the samples left gaps and every curtain drew as several
    // thin parallel copies of itself, one per march step.
    let e = max(AURORA_SHEET_HALF, max(rate * dt, pix));
    return (AURORA_SHEET_HALF / e) * (1.0 - smoothstep(0.0, e, abs(g)));
}

fn aurora_emission(ro: vec3<f32>, rd: vec3<f32>, t0: f32, t1: f32, rp: f32, pix_ang: f32) -> vec3<f32> {
    var total = vec3<f32>(0.0);
    let seg = t1 - t0;
    if (seg <= 0.0) {
        return total;
    }
    let sun = normalize(camera.sun_direction.xyz);
    let time = camera.sun_color.w;

    let n = arrayLength(&env_regions);
    for (var i = 0u; i < n; i = i + 1u) {
        let reg = env_regions[i];
        // Band 2 only. A storm is not an aurora, and a fog bank is certainly
        // not: an empty row fails this too, because its band is 0.
        if (abs(reg.kind_shape.w - 2.0) >= 0.5 || reg.kind_shape.y <= 0.0) {
            continue;
        }
        let pole = reg.dir_radius.xyz;
        let inner = reg.params.x;
        let outer = reg.params.y;
        let r_lo = rp + reg.params.z * (1.0 - rp);
        let r_hi = rp + reg.params.w * (1.0 - rp);
        if (outer <= inner || r_hi <= r_lo) {
            continue;
        }
        // A stable tangent basis about the pole, so the curtain folds are
        // anchored to the world and do not swim when the camera moves.
        var ref_v = vec3<f32>(1.0, 0.0, 0.0);
        if (abs(pole.y) < 0.9) {
            ref_v = vec3<f32>(0.0, 1.0, 0.0);
        }
        let ex = normalize(cross(pole, ref_v));
        let ey = cross(pole, ex);
        // Solve the ray against the layer instead of sampling the whole
        // atmosphere chord and hoping. The emitting layer is a thin shell, so
        // spreading a handful of samples over the full chord puts most of them
        // outside it: grazing rays, which are exactly the ones that should be
        // BRIGHTEST because they travel furthest through the layer, got the
        // fewest samples and often none at all. Clipping to the layer first
        // makes every sample count and makes limb brightening come out right.
        let tca_a = -dot(ro, rd);
        let perp_a = ro + rd * tca_a;
        let d2_a = dot(perp_a, perp_a);
        if (d2_a >= r_hi * r_hi) {
            continue; // never reaches the layer
        }
        let th_hi = sqrt(r_hi * r_hi - d2_a);
        var a_t = tca_a - th_hi;
        var b_t = tca_a + th_hi;
        if (d2_a < r_lo * r_lo) {
            // The ray dips inside the layer's floor, so take the NEAR crossing
            // only; the far one is behind the planet and occluded anyway.
            b_t = tca_a - sqrt(r_lo * r_lo - d2_a);
        }
        a_t = max(a_t, t0);
        b_t = min(b_t, t1);
        if (b_t <= a_t) {
            continue;
        }
        let dt_a = (b_t - a_t) / f32(AURORA_STEPS);
        for (var k = 0; k < AURORA_STEPS; k = k + 1) {
            let t = a_t + dt_a * (f32(k) + 0.5);
            let pnt = ro + rd * t;
            let r = length(pnt);
            let up = pnt / r;
            // Where in the ring, how far around it, and how far up the layer.
            let cos_a = clamp(dot(up, pole), -1.0, 1.0);
            let ang = acos(cos_a);
            let mid = (inner + outer) * 0.5;
            let halfw = max((outer - inner) * 0.5, 1.0e-5);
            let half_d = halfw * AURORA_DIFFUSE_SPREAD;
            // Cheap rejections BEFORE the strand loop. Nothing reaches past
            // the diffuse half-width (the strands meander within it), and
            // nothing is drawn over daylit ground, so both used to pay for
            // every strand before being thrown away.
            if (abs(ang - mid) >= half_d) {
                continue;
            }
            // Only over ground that is in darkness. The soft edge keeps the
            // oval from ending in a hard line along the terminator.
            let night = smoothstep(0.12, -0.10, dot(up, sun));
            if (night <= 0.0) {
                continue;
            }
            let tang = normalize(up - pole * cos_a);
            let phi = atan2(dot(tang, ey), dot(tang, ex));
            let hgt = clamp((r - r_lo) / (r_hi - r_lo), 0.0, 1.0);
            // The height span this step's segment covers, for the box filter.
            let h_a = clamp((length(pnt - rd * (0.5 * dt_a)) - r_lo) / (r_hi - r_lo), 0.0, 1.0);
            let h_b = clamp((length(pnt + rd * (0.5 * dt_a)) - r_lo) / (r_hi - r_lo), 0.0, 1.0);
            // How fast the ray moves in ang (away from the pole) and in phi
            // (around it), per unit of march distance: the tangents at this
            // point, and the radius of the circle phi is measured on.
            let sin_a = max(sqrt(max(1.0 - cos_a * cos_a, 0.0)), 1.0e-4);
            let dang_dt = dot(rd, (up * cos_a - pole) / sin_a) / r;
            let dphi_dt = dot(rd, cross(pole, up) / sin_a) / (r * sin_a);
            let pix_foot = pix_ang * t / r;

            // ── THE OVAL IS NOT A CIRCLE (operator, 2026-09-22) ──
            //
            // "Can we add more shape to the aurora so it is not just a ring
            // but, like kinda spider webbing out?"
            //
            // That is what the reference photographs show. A real oval is
            // several arcs that meander in latitude, run parallel for a
            // stretch, cross, break into segments and fade out. Drawing one
            // smooth band is what made the first version read as a painted
            // ring rather than a structure.
            //
            // Each strand wanders on its own path and exists only along part
            // of the oval. Where two cross they merge; where a presence mask
            // closes, the web has a hole. Nothing here is noise: the same
            // harmonics at the same phi always give the same shape, so the
            // web is a STRUCTURE that drifts rather than a flicker.
            var arc = 0.0;
            for (var sI = 0; sI < AURORA_STRANDS; sI = sI + 1) {
                let sd = f32(sI);
                let center = mid
                    + (sd - 0.5 * f32(AURORA_STRANDS - 1)) * AURORA_STRAND_SPACING * halfw
                    + aurora_wave(phi, time * AURORA_DRIFT, sd * 1.7)
                        * halfw * AURORA_MEANDER;
                let center_d = aurora_wave_d(phi, time * AURORA_DRIFT, sd * 1.7)
                    * halfw * AURORA_MEANDER;
                // Gentler than -0.30/0.40: a presence mask that shuts quickly
                // ends a strand in a visible cap, which is the seam again in a
                // different place.
                let pres = smoothstep(-0.75, 0.55,
                    aurora_wave(phi * 0.55, time * AURORA_DRIFT * 0.6, sd * 3.9 + 11.0));
                // The rate the ray crosses THIS sheet: its own motion away
                // from the pole, less the sheet's motion as it meanders.
                let rate = abs(dang_dt - center_d * dphi_dt);
                // Not every arc is equal: a display has a dominant arc and fainter
                // companions. A golden-ratio walk gives an irregular order with no
                // table to maintain as AURORA_STRANDS changes.
                let bright = 0.45 + 0.55 * fract(sd * 0.618034 + 0.3);
                let v = aurora_sheet(ang - center, rate, dt_a, pix_foot) * pres * bright;
                // Screen combine, for the same reason as arc and diffuse below:
                // max() creases where two strands cross.
                arc = arc + v * (1.0 - arc);
            }
            // And the DIFFUSE glow fanning away from the strands, several
            // times wider and a fraction as bright. Without this the oval
            // reads as a bare stripe; without the strands it reads as a
            // smooth wash, which is what the first version did.
            let diffuse = (1.0 - smoothstep(0.0, half_d, abs(ang - mid)))
                * AURORA_DIFFUSE_LEVEL;
            // ── NO max() HERE (operator, 2026-09-23) ──
            //
            // "there is a weird seam where two aurora lines seem to meet. One
            // is much wider than the other."
            //
            // Those two are the narrow ARC and the wide DIFFUSE sheet, and
            // max() joins them with a crease: wherever one overtakes the other
            // the slope changes discontinuously, and a discontinuity in slope
            // reads as a drawn line even though neither term has an edge there.
            // A screen combine is smooth everywhere and keeps both at full
            // strength where they do not overlap.
            // A SUM now rather than the screen combine. Both terms are light
            // emitted by the same air, so they add; the screen combine was
            // only needed while both lived in 0..1, and with the sheet gain
            // above the arc no longer does. A sum has no crease either.
            let ring = arc * AURORA_SHEET_GAIN + diffuse;
            if (ring <= 0.001) {
                continue;
            }
            // ── WHAT MOVES, AND HOW FAST (operator, 2026-09-22) ──
            //
            // "It also seems rather static, are not auroras supposed to shift
            // around a bit or is the time that happens over not so short?"
            //
            // Both halves of that are right, and the first version only had
            // the slow half. The OVAL drifts over hours. The FINE STRUCTURE
            // does not: rays shimmer and pulsating patches brighten and fade
            // over 1 to 20 seconds, and that is the motion the eye reads as
            // alive. So the drift stays slow and honest, and the flicker and
            // the pulse run an order of magnitude faster.
            let folds = 0.5 + 0.5 * sin(phi * 9.0 + time * AURORA_FOLD_SPEED);
            // RAYS, sharpened. A curtain is made of near-vertical rays, so the
            // modulation wants contrast rather than a gentle ripple, and the
            // pattern must stay COHERENT with height or it reads as noise
            // instead of structure. The small height term leans them rather
            // than scrambling them.
            // ── RAYS ARE NOT A COMB (operator, 2026-09-22) ──
            //
            // He photographed the band covered in regular fine stripes. A pure
            // sine at a fixed 1200 cycles around the oval lands those cycles a
            // few pixels apart at his range, and a REGULAR few-pixel stripe
            // pattern reads as corduroy, not as an aurora. It also loses half
            // its brightness into the dark half of every cycle, which is part
            // of the "weird darkness" in the same report.
            //
            // Real rays are irregularly spaced and individually bright or
            // faint. Warping the phase makes the spacing wander, and
            // modulating the amplitude means some rays carry the display and
            // others barely show, which is what breaks the fabric read.
            let ray_warp = aurora_wave(phi * 11.0, time * AURORA_DRIFT, 5.0) * 2.5;
            let ray_amp = 0.45 + 0.55
                * smoothstep(-0.6, 0.6, aurora_wave(phi * 23.0, time * AURORA_DRIFT * 1.7, 19.0));
            let ray_s = 0.5 + 0.5 * sin(phi * AURORA_RAY_LOBES + ray_warp
                - time * AURORA_FLICKER + hgt * AURORA_RAY_LEAN);
            // RAY DETAIL IS A SAMPLING QUESTION, NOT A FIELD QUESTION, and the
            // difference matters because BUG-080 was caused by confusing the
            // two. This does not change WHERE the aurora is or HOW BRIGHT it
            // is; it fades a high-frequency detail to its own exact mean once
            // the detail is too small to resolve, which is a mip fade and is
            // energy preserving by construction. `t` is distance from the
            // camera because that is what sets the sampling rate. A weight on
            // the EFFECT may never read the camera; an antialiasing term has
            // nothing else it could read.
            // NYQUIST FADE. The old fade keyed on raw distance, which is only a
            // proxy: what decides whether a stripe can be drawn is its width in
            // PIXELS, and that depends on the field of view and the resolution
            // as well as the range. Below a couple of pixels per cycle the comb
            // is beating against the pixel grid rather than describing anything,
            // so it fades to its own exact mean and costs no brightness.
            //
            // Stripe spacing at this sample is the circumference at its distance
            // from the spin axis divided by the lobe count; dividing by t gives
            // radians, and by pix_ang gives pixels.
            let r_perp = max(length(pnt - pole * dot(pnt, pole)), 1.0e-4);
            let stripe_px = (6.2831853 * r_perp / AURORA_RAY_LOBES)
                / max(t * pix_ang, 1.0e-9);
            // And along the RAY: on an oblique or limb view one march step can
            // cross many stripes, and a point sample of a comb that fine is a
            // coin flip. Fade to the mean once a step spans half a cycle.
            let cyc_step = abs(dphi_dt) * dt_a * AURORA_RAY_LOBES / 6.2831853;
            let ray_lod = smoothstep(AURORA_RAY_PX_LO, AURORA_RAY_PX_HI, stripe_px)
                * (1.0 - smoothstep(0.25, 0.5, cyc_step));
            // pow 2 rather than 3: the sharper power is what made each cycle
            // mostly dark, and the amplitude modulation now supplies the
            // contrast that the exponent used to.
            let rays = mix(AURORA_RAY_MEAN, pow(ray_s, 2.0) * ray_amp, ray_lod);
            // Pulsating patches: two slow beating cells, so brightness moves
            // around the oval independently of the rays.
            let pulse = 0.65 + 0.35 * sin(phi * 4.0 + time * AURORA_PULSE)
                * sin(phi * 2.3 - time * AURORA_PULSE * 0.7 + 1.7);
            let curtain = (0.30 + 0.70 * folds * (0.25 + 0.75 * rays)) * pulse;
            // Height profiles, separately per emission line. The 557.7 nm green
            // really does sit at the BOTTOM of the curtain and stop, while the
            // 630 nm red caps it and reaches much higher. Giving them one shared
            // profile is what made the first version a uniform slab.
            // Green fills the lower two thirds and stops; red caps the top
            // third only. The previous split had them overlapping across most
            // of the layer, so the curtain was red with a green hem instead of
            // green with a red cap.
            // Averaged over the step's height span, not sampled at its middle:
            // see aurora_ss_avg. This is what removes the colour banding.
            let green_v = 1.0 - aurora_ss_avg(0.05, 0.62, h_a, h_b);
            let red_v = aurora_ss_avg(0.55, 0.95, h_a, h_b)
                * (1.0 - 0.5 * aurora_ss_avg(0.90, 1.0, h_a, h_b));
            let col = AURORA_GREEN * green_v
                + AURORA_RED * (red_v * AURORA_RED_LEVEL);
            total = total + col * (ring * night * curtain
                * reg.kind_shape.y * AURORA_STRENGTH * dt_a);
        }
    }
    return total;
}

// ── DITHER BEFORE AN 8-BIT WRITE (operator, 2026-09-24) ──
//
// The scene renders straight into the 8-bit sRGB surface format with no
// dither anywhere in the path. A slow, dark gradient therefore quantises
// into flat rings one display level apart, and the eye reads each ring as a
// hard edge: the aurora's diffuse glow spans only three or four levels, so
// it drew as nested ellipses with crisp outlines (proven by rendering it
// alone at full strength, where the rings multiply into a contour map).
//
// Triangular-distribution noise of plus or minus one step, the standard
// choice: it removes the banding without leaving the noise level visibly
// tied to the signal. One value for all three channels so it adds no colour
// speckle. The step is one 8-bit sRGB code converted to LINEAR at this
// value, because the blend happens in linear and the encode is what
// quantises. The encode 1.055 * v^(1/2.4) has slope 0.4396 * v^-0.5833, so
// one code (1/255) is v^0.5833 / 112.1 in linear, floored at the linear toe.
//
// The real fix is an HDR scene target with one tonemap and one dither at
// the end; this covers the surface the operator reported until then.
fn srgb_dither(v: vec3<f32>, pix: vec2<f32>) -> vec3<f32> {
    let p = vec2<u32>(max(pix, vec2<f32>(0.0)));
    let n = pcg2d_hash(p + vec2<u32>(0x2C1Bu, 0x7F4Du))
        + pcg2d_hash(p + vec2<u32>(0x91E3u, 0x0A57u)) - 1.0;
    let step = pow(max(v, vec3<f32>(0.0031308)), vec3<f32>(0.5833)) / 112.1;
    return max(v + n * step, vec3<f32>(0.0));
}

fn atmosphere_scattering(world_position: vec3<f32>, front_facing: bool, pix: vec2<f32>) -> vec4<f32> {
    // Shell center + radius recovered from the object transform: the shell
    // mesh is a UNIT icosphere placed via Vec3::splat(scale), so column 0's
    // length IS the shell radius and column 3 is the planet center. Nothing
    // extra to plumb through the material uniforms.
    let center = obj_model()[3].xyz;
    let shell_r = length(obj_model()[0].xyz);
    // ── THE SHELL IS DRAWN TWICE (operator, 2026-09-23) ──
    //
    // "The aurora is still darker over land masses." Measured: it is not the
    // land, it is the CLOUD DECK. The deck sits at 12 km and the aurora emits
    // between 99 and 190 km, so from above the clouds are BEHIND it, but the
    // fullscreen cloud composite runs after this pass and paints over the
    // emission. Cloud cover is regional, so the dimming wears a coastline.
    // With the deck off, land and water measured 14.811 and 14.826, identical;
    // with it on, 4.288 and 2.198.
    //
    // The composite cannot simply move: it must run after the dome, because the
    // cloud march already applies aerial perspective at the cloud first-hit
    // distance and letting the dome blend over the deck applied that same air
    // twice and opaquely (the erased-clouds regression, 1.2 percent of the disc
    // written with the old order against 99.9 with this one).
    //
    // So the shell splits. The AIR draws before the composite exactly as it
    // always did, and a second draw of the same shell carries the EMISSION
    // afterwards. params.w is the material emissive lane, unused here, and the
    // CPU can read it off Material::emissive to route the two draws.
    let aurora_only = material.params.w > 0.5;
    let rp = clamp(material.params.x, 0.01, 0.9999); // planet radius (shell units)
    let h = max(material.params.y, 1.0e-6);          // scale height (shell units)

    // Camera + ray in shell units, planet center at the origin.
    let ro = (camera.view_pos.xyz - center) / shell_r;
    let rd = normalize(world_position - camera.view_pos.xyz);
    // Angular size of one screen pixel, in radians, taken here and ONLY here.
    // dpdx/dpdy are defined only under uniform control flow and this function
    // discards and returns early a few lines below, so the derivative has to be
    // read before any of that. rd is a unit vector, so the length of its screen
    // derivative IS the per-pixel angle.
    let pix_ang = max(max(length(dpdx(rd)), length(dpdy(rd))), 1.0e-9);
    let cam_inside = dot(ro, ro) < 1.0;

    // The transparent pipeline draws BOTH faces of the shell (cull_mode:
    // None, shared with glass). A camera outside the shell would therefore
    // blend the same ray twice (front face + back face). Keep exactly one
    // layer: front faces when outside, back faces when inside (from inside a
    // sphere only back faces are visible, so this is also what makes the
    // sky appear at low altitude instead of vanishing on shell entry).
    if (front_facing == cam_inside) {
        discard;
    }

    // Ray vs shell sphere (radius 1) via the geometric formulation: the
    // naive b^2 - c quadratic catastrophically cancels when the camera is
    // thousands of radii out; the explicit perpendicular foot does not.
    let tca = -dot(ro, rd);
    let perp = ro + rd * tca;
    let d2 = dot(perp, perp);
    if (d2 >= 1.0) {
        return vec4<f32>(0.0); // grazing numeric miss: fully transparent
    }
    let thc = sqrt(1.0 - d2);
    var t0 = tca - thc;
    var t1 = tca + thc;
    if (t1 <= 0.0) {
        return vec4<f32>(0.0); // shell entirely behind the camera
    }
    t0 = max(t0, 0.0); // camera inside the shell: integrate from the eye

    // Clip the segment at the planet surface: air BEHIND the planet
    // contributes nothing to this pixel (the opaque surface occludes it).
    // (The pure-black horizon hairline was A/B-tested against a
    // near-tangent cancellation guard here on 2026-08-22: the census did
    // NOT move - this clip is exonerated. The hairline's current best
    // theory is a raster coverage gap between the sea's silhouette edge
    // and the sky; see the journal.)
    if (d2 < rp * rp && tca > 0.0) {
        let t_planet = tca - sqrt(rp * rp - d2);
        if (t_planet > t0) {
            t1 = min(t1, t_planet);
        }
    }
    if (t1 <= t0) {
        return vec4<f32>(0.0);
    }

    // ── THE EMISSION DRAW RETURNS HERE, BEFORE ANY AIR WORK (2026-09-24) ──
    //
    // It needs only the ray and its clipped segment, which exist from this
    // line. It used to sit after the whole scattering integral, the sky-view
    // LUT, the tonemap and the haze, and throw all of that away.
    //
    // Measured when it moved, and worth recording because the obvious guess was
    // wrong: it saved NOTHING measurable (celestial_t 25.64 to 25.63 ms looking
    // straight down, where the shell fills the screen; the image unchanged to
    // 0.025 percent of pixels). The discarded scattering was never the cost of
    // that pass, so do not look here for it. It stays here because computing
    // work only to discard it is wrong in principle, not because it was slow.
    if (aurora_only) {
        let au_raw = aurora_emission(ro, rd, t0, t1, rp, pix_ang);
        // A SHOULDER, NOT A CLIP (operator, 2026-09-24: "the harsh edges
        // for the different shades of green/orange look weird"). This draw
        // lands on an already tonemapped image, and the old path clamped at
        // 1.0 per channel: a bright fold clipped its green while red and
        // blue kept rising, so the hue jumped at the clip boundary and drew
        // an edge. 1 - exp(-x) is linear for faint light (a dim aurora is
        // unchanged) and rolls a bright core off smoothly toward white-green,
        // which is also how an over-bright real curtain photographs.
        let au = srgb_dither(vec3<f32>(1.0) - exp(-au_raw), pix);
        let al = clamp(max(au.r, max(au.g, au.b)), 0.0, 1.0);
        if (al <= 0.0005) {
            discard; // no aurora on this ray: leave the clouds untouched
        }
        return vec4<f32>(
            clamp(au / max(al, 1.0e-3), vec3<f32>(0.0), vec3<f32>(1.0)),
            al,
        );
    }

    // Scattering coefficients per shell radius. The vertical optical depth
    // of an exponential profile is beta * H, so beta = target_depth / H --
    // this keeps the LOOK invariant across planet sizes AND across the
    // far-body disc-size floor (which inflates the drawn radius).
    let density_mul = material.base_color.a;
    let beta_ray = material.base_color.rgb * (density_mul * ATMO_TAU_RAYLEIGH / h);
    let beta_mie = density_mul * ATMO_TAU_MIE / h;
    // Extinction carries a touch of Mie absorption (the classic /0.9).
    let beta_ext = beta_ray + vec3<f32>(beta_mie * 1.11);

    let sun = normalize(camera.sun_direction.xyz);

    // Midpoint march along the view segment. od_view accumulates the density
    // integral camera->sample numerically (needed for in-scatter anyway);
    // the per-sample sun leg is ANALYTIC -- that is the O'Neil-class trick
    // that removes the nested loop.
    let dt = (t1 - t0) / f32(ATMO_SAMPLES);
    var od_view = 0.0;
    var inscatter = vec3<f32>(0.0);
    for (var i = 0; i < ATMO_SAMPLES; i = i + 1) {
        let t = t0 + (f32(i) + 0.5) * dt;
        let p = ro + rd * t;
        let r = length(p);
        let dens = exp(-max(r - rp, 0.0) / h);
        // Half-sample lag: transmittance to the CENTER of this slice.
        let od_here = od_view + dens * dt * 0.5;
        od_view = od_view + dens * dt;
        let mu_s = dot(p, sun) / max(r, 1.0e-6);
        let od_sun = atmo_od_to_space(r, mu_s, rp, h);
        let tau = beta_ext * (od_here + od_sun);
        inscatter = inscatter + dens * exp(-tau) * dt;
    }

    // Phase evaluation: cos of the angle between view ray and sun direction;
    // +1 = looking straight at the sun (forward scattering).
    let cos_theta = dot(rd, sun);
    // A ray hits the planet iff it runs forward (tca > 0) with impact
    // parameter below rp -- for a camera above the surface, b rises through
    // rp BEFORE tca changes sign as the ray tilts from down to up, so the
    // hit gate never introduces a visible seam.
    let b_impact = sqrt(d2);
    let hits_surface = tca > 0.0 && b_impact < rp;
    let cam_r = length(ro);
    let w_far = smoothstep(ATMO_NEAR_R, ATMO_FAR_R, cam_r);
    // v0.918 three-tier rework (see ATMO_EXPOSURE_DOME): the SKY tier is the
    // dome exposure at ground level, ramping back to full as the camera
    // climbs out of the shell (w_alt) or recedes (w_far). Surface-hitting
    // rays keep the calm v0.815 near exposure in the disc interior and blend
    // toward the SKY tier across the limb band, so the horizon seam stays
    // continuous. Grazing rays (b_impact ~ rp: horizon water/coast from
    // ground level) used to land in that band at FULL space-calibrated
    // exposure -- the white veil the operator saw on grazing-angle water.
    let w_alt = smoothstep(rp, 1.0, cam_r);
    let sky_base = mix(ATMO_EXPOSURE_DOME, ATMO_EXPOSURE, max(w_alt, w_far));
    var base = sky_base;
    var edge_surf = 0.0;
    if (hits_surface) {
        let w_edge = smoothstep(rp - (1.0 - rp) * 0.5, rp, b_impact);
        base = mix(ATMO_EXPOSURE_NEAR, sky_base, w_edge);
        edge_surf = clamp(1.0 - max(w_edge, w_far), 0.0, 1.0);
    }
    let exposure = mix(base, ATMO_EXPOSURE, w_far);
    // Low-altitude aerial-perspective trim: the same near-surface weight the
    // exposure blend uses drives the haze-alpha scale (1.0 for limb/sky/far).
    let near_surf = edge_surf;
    let haze_scale = mix(1.0, ATMO_NEAR_HAZE, near_surf);
    let sun_radiance = camera.sun_color.rgb * camera.sun_direction.w * exposure;
    // Isotropic multiple-scatter bounce (see ATMO_MS_ISO): gated by the same
    // weight that lowers the dome, so every unchanged-exposure view (400 km
    // limb, 12,000 km blue marble) stays bit-identical.
    let ms_gate = ATMO_MS_ISO * (1.0 - max(w_alt, w_far));
    let radiance = sun_radiance
        * (beta_ray * atmo_rayleigh_phase(cos_theta)
            + vec3<f32>(beta_mie) * atmo_mie_phase(cos_theta)
            + (beta_ray + vec3<f32>(beta_mie)) * ms_gate)
        * inscatter;

    // Per-channel transmittance of whatever sits behind this pixel,
    // collapsed to the single gray alpha fixed-function blending can
    // express. The surface stays readable at every angle because this path
    // only ever alpha-blends over it.
    let trans = exp(-beta_ext * od_view);
    let alpha = clamp(1.0 - (trans.r + trans.g + trans.b) / 3.0, 0.0, 1.0);

    // ── Sky-view LUT hybrid (stage 3c, v0.948) ── near the surface the sky
    // radiance comes from the per-frame Hillaire table (sky_view_lut.wgsl,
    // transcribed from the TESTED CPU twin) instead of this function's coarse
    // dome march. Blended PRE-tonemap so sky_lum (star occlusion) and the
    // alpha logic key off the real sky automatically - daytime star hiding
    // becomes physics. Gate: zero from orbit (w_alt / w_far, the approved
    // space look is untouched) and zero when the table is stale
    // (shadow_u.params2.y = rendered-this-frame flag).
    let w_lut = (1.0 - max(w_alt, w_far)) * shadow_u.params2.y;
    var radiance_sky = radiance;
    if (w_lut > 0.001) {
        let sun_lut = normalize(camera.sun_direction.xyz);
        let up_c = normalize(ro);
        let l_elev = asin(clamp(dot(rd, up_c), -1.0, 1.0));
        // Hillaire's non-linear latitude (texels packed at the horizon).
        let v_lut = clamp(0.5 + 0.5 * sign(l_elev) * sqrt(abs(l_elev) / (PI * 0.5)), 0.0, 1.0);
        // Azimuth from the sun, symmetric half-circle (seam-free with the
        // clamping sampler): u = acos(cos_phi) / 2pi covers cos fully.
        let sun_h = sun_lut - up_c * dot(sun_lut, up_c);
        let view_h = rd - up_c * dot(rd, up_c);
        let sh_len = length(sun_h);
        let vh_len = length(view_h);
        var u_lut = 0.25;
        if (sh_len > 1e-4 && vh_len > 1e-4) {
            let cphi = clamp(dot(sun_h / sh_len, view_h / vh_len), -1.0, 1.0);
            u_lut = acos(cphi) / (2.0 * PI);
        }
        let lut_rgb = textureSampleLevel(sky_view_tex, albedo_sampler, vec2<f32>(u_lut, v_lut), 0.0).rgb;
        radiance_sky = mix(radiance, lut_rgb * SKY_LUT_EXPOSURE, w_lut);
    }

    // Tone-map the in-scattered light with the SAME ACES curve as the rest
    // of the pipeline; all math above is linear. The render target is an
    // sRGB view, so writing linear values is the honest handoff -- the
    // hardware applies the sRGB transfer on store, and blending against an
    // sRGB target happens in LINEAR space per the WebGPU spec (the
    // v0.802/v0.803 glow-layer lesson: know the target's gamma, encode once,
    // never twice).
    // ── WEATHER FOG REACHES THE SKY (v0.1060) ──
    // Operator: "I would think that while a sandstorm is happening I couldn't
    // see the sky", with reference photos of a dust wall swallowing everything.
    // v0.1059 gave the weather control of the aerial-haze sigma, which fogs
    // every surface that reaches the shared fragment tail - terrain, vegetation,
    // objects, water - but the SKY is this shell, a different material type that
    // returns long before that tail. So a sandstorm hazed the ground
    // convincingly and left a clear blue sky above it, which is exactly
    // backwards: in a real dust storm the sky is the FIRST thing to go.
    //
    // The dome sits at the top of an optically thick layer, so the fog's own
    // colour simply replaces it as density rises. Weight by how much sky is
    // left after the ground-layer extinction over one slant cap - the same
    // numbers the surface path uses, so sky and ground agree by construction.
    var radiance_fog = radiance_sky;
    let fog_sigma = camera.light1_cone_inner.y;
    if (fog_sigma > 1.0e-4) {
        let fog_rgb = vec3<f32>(
            camera.light2_cone_inner.y,
            camera.light2_cone_inner.z,
            camera.light2_cone_inner.w,
        );
        // SLANT optical depth, the SAME integral the surface path uses
        // (v0.1108). This used to be the layer thickness straight, with no
        // view dependence at all - so in heavy weather the sea integrated
        // `cap / sin(elevation)` and saturated (2286 m of fog for a horizon
        // ray at an 80 m cap) while the sky integrated a flat 80 m and stayed
        // crisp. Measured at the operator's fog_density: sea 99.7% fog, sky
        // 18.8%, clouds 0%. The frame then reads as "the water is broken",
        // when in fact the water was the only surface telling the truth about
        // the weather. One fog, one integral, or they cannot agree.
        let layer = max(camera.light1_cone_inner.z, 1.0);
        let sky_up = normalize(vec3<f32>(
            camera.light3_cone_inner.y,
            camera.light3_cone_inner.z,
            camera.light3_cone_inner.w,
        ));
        let sky_updot = abs(dot(rd, sky_up));
        let w_fog = clamp(1.0 - exp(-fog_sigma * (layer / max(sky_updot, 0.035))), 0.0, 1.0);
        radiance_fog = mix(radiance_sky, fog_rgb, w_fog);
    }
    let aces_a = 2.51;
    let aces_b = 0.03;
    let aces_c = 2.43;
    let aces_d = 0.59;
    let aces_e = 0.14;
    let mapped = clamp(
        (radiance_fog * (aces_a * radiance_fog + vec3<f32>(aces_b)))
            / (radiance_fog * (aces_c * radiance_fog + vec3<f32>(aces_d)) + vec3<f32>(aces_e)),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );

    // Daylight star occlusion (v0.912, operator: "I'm able to see the
    // galaxy in the background but in real life the sky is just blue"):
    // the transmittance alpha is only ~0.1-0.3 looking straight up, so the
    // star skybox bled through the daytime sky. Physically, stars vanish
    // because the scattered radiance OUT-SHINES them, not because air
    // absorbs them - in fixed-function blending that means the alpha must
    // rise with the sky's own brightness. Night sky: mapped ~ 0, alpha
    // unchanged, full starfield. Day: bright dome occludes. Twilight
    // blends smoothly in between.
    let sky_lum = dot(mapped, vec3<f32>(0.2126, 0.7152, 0.0722));
    // v0.913 (operator: "looking away from the sun... I can still see the
    // stars" + "our changes have hidden OUR sun"): occlusion is now driven
    // by the DAY itself (sun elevation at the camera), so the whole dome
    // hides stars at noon, not just the bright half; and a narrow window
    // toward the sun disc keeps the sky from occluding the sun - the sun
    // outshines its own sky, and the disc stays sharp.
    let sun_l = normalize(camera.sun_direction.xyz);
    // v0.925 (operator: "a hard edged black shell around the Earth that
    // should be the atmosphere fading to stars"): the geometric day term
    // is a GROUND-VIEW rule - at noon the whole dome outshines the stars.
    // From ORBIT the same rule forced ~98.5% opacity onto every shell
    // fragment, including the thin outer limb where the in-scatter is
    // nearly zero: an opaque near-black ring swallowing the starfield.
    // Gate it by "camera inside the atmosphere" (the v0.918 w_alt weight):
    // ground keeps the full noon occlusion, and from space the limb
    // occludes stars only by its own BRIGHTNESS (sky_lum below), so the
    // faint outer shell fades smoothly into stars.
    let day = smoothstep(-0.08, 0.12, dot(normalize(ro), sun_l))
        * (1.0 - max(w_alt, w_far));
    let toward_sun = smoothstep(0.9986, 0.9997, dot(rd, sun_l));
    // 4.5 (was 3.2 pre-v0.918): the calibrated dome is dimmer, so the
    // luminance-driven twilight occlusion needs a stronger gain to keep
    // stars hidden through civil dusk. Daytime is owned by the `day` term
    // (0.985 dominates) and night sky_lum ~ 0, so only twilight shifts.
    // v0.956 (operator: "the blue of the atmosphere completely hides the
    // terrain on the edges" - southern Australia read as open water from
    // 12,000 km): stars only ever sit behind rays that MISS the planet, so
    // the occlusion boost must not touch surface-hitting rays. Near the
    // disc edge the limb in-scatter is bright enough that sky_lum * 4.5
    // saturated alpha to 1.0 and painted flat sky over the continent; the
    // pure transmittance alpha (~0.5 at those angles) keeps the land
    // readable through physically blue haze, exactly like real limb photos.
    // The aurora rides ON the in-scatter: adding it to `mapped` puts it
    // straight on screen additively, because the blend resolves to
    // mapped + dst * (1 - alpha). It must NOT raise the occlusion on its own
    // terms, since emission adds light without hiding what is behind it, but
    // alpha does have to be large enough that the rgb = mapped / alpha divide
    // below does not clamp the glow away over thin polar air.
    // The AIR draw carries no emission at all now, so it does not pay for the
    // region walk either.
    let aurora = vec3<f32>(0.0);
    let mapped_a = mapped + aurora;
    let aurora_lum = 0.0;
    var alpha_occ = alpha;
    if (!hits_surface) {
        alpha_occ = max(alpha, max(clamp(sky_lum * 4.5, 0.0, 1.0), day * 0.985));
    }
    alpha_occ = mix(alpha_occ, alpha, toward_sun);
    // ALPHA_BLENDING computes src.rgb * src.a + dst * (1 - src.a); divide
    // the radiance back out of the alpha so exactly `mapped` lands on
    // screen. Both terms go to zero together for thin air, so the ratio
    // stays finite; the clamp guards the pathological alpha -> 0 corner.
    // ── THE AURORA MUST NOT BE TRIMMED BY THE HAZE SCALE (operator, 2026-09-22) ──
    //
    // "the aurora seems kind of dark ... over the land it is almost
    // imperceptibly dark. Are we properly applying emissiveness to the aurora?"
    //
    // No, we were not. haze_scale is the low-altitude AERIAL PERSPECTIVE trim:
    // it exists so scattered haze does not wash out terrain, and it is keyed on
    // edge_surf, which is about 1 for a ray looking STEEPLY DOWN at the ground
    // and 0 for a grazing limb ray. The blend delivers rgb * alpha_out, and with
    // the aurora folded into mapped before the divide that came out as
    // (mapped + aurora) * haze_scale, so the aurora was cut to ATMO_NEAR_HAZE
    // (0.45) wherever the view looked down at the surface and was full strength
    // only near the limb. That reads as an aurora that works on the edge of the
    // disc and vanishes under you.
    //
    // Aerial perspective is a statement about SCATTERED light between the eye
    // and a surface. Emission is not scattered light and has no business being
    // trimmed by it, so the two terms are now scaled separately: haze_scale
    // applies to `mapped` alone, and the aurora is delivered whole.
    //
    // Reduces to the old expression exactly when aurora == 0.
    let haze_occ = alpha_occ * haze_scale;
    let alpha_a = max(haze_occ, aurora_lum);
    let delivered = mapped * haze_scale + aurora;
    let rgb = clamp(delivered / max(alpha_a, 1.0e-3), vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(rgb, alpha_a);
}


