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

fn atmosphere_scattering(world_position: vec3<f32>, front_facing: bool) -> vec4<f32> {
    // Shell center + radius recovered from the object transform: the shell
    // mesh is a UNIT icosphere placed via Vec3::splat(scale), so column 0's
    // length IS the shell radius and column 3 is the planet center. Nothing
    // extra to plumb through the material uniforms.
    let center = obj_model()[3].xyz;
    let shell_r = length(obj_model()[0].xyz);
    let rp = clamp(material.params.x, 0.01, 0.9999); // planet radius (shell units)
    let h = max(material.params.y, 1.0e-6);          // scale height (shell units)

    // Camera + ray in shell units, planet center at the origin.
    let ro = (camera.view_pos.xyz - center) / shell_r;
    let rd = normalize(world_position - camera.view_pos.xyz);
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
    var alpha_occ = alpha;
    if (!hits_surface) {
        alpha_occ = max(alpha, max(clamp(sky_lum * 4.5, 0.0, 1.0), day * 0.985));
    }
    alpha_occ = mix(alpha_occ, alpha, toward_sun);
    // ALPHA_BLENDING computes src.rgb * src.a + dst * (1 - src.a); divide
    // the radiance back out of the alpha so exactly `mapped` lands on
    // screen. Both terms go to zero together for thin air, so the ratio
    // stays finite; the clamp guards the pathological alpha -> 0 corner.
    let rgb = clamp(mapped / max(alpha_occ, 1.0e-3), vec3<f32>(0.0), vec3<f32>(1.0));
    // rgb keeps the ORIGINAL alpha (its colour + brightness); scaling only the
    // returned alpha by haze_scale dims the additive in-scatter and clears the
    // surface together, and is a no-op wherever haze_scale == 1.
    return vec4<f32>(rgb, alpha_occ * haze_scale);
}


