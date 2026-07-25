// Sky-view LUT pass (Hillaire 2020; sky arc stage 3b-2, v0.947).
//
// Renders the 200x100 distant-sky radiance table for the CURRENT camera
// altitude + sun elevation. Each texel is one view direction; the fragment
// marches the atmosphere with ANALYTIC per-step integration - a direct
// transcription of the TESTED CPU reference `atmo_luts::sky_view_radiance`
// (src/renderer/atmo_luts.rs; its physics tests are this shader's contract:
// blue zenith, horizon brighter than zenith, reddening sunset, black night).
// t_sun comes from the stage-3a transmittance LUT (that is why it exists);
// the multiple-scatter term samples the stage-2 Psi LUT.
//
// Texel mapping:
// - u: view azimuth RELATIVE TO THE SUN, phi = u * 2pi (the sky is symmetric
//   about the sun's vertical plane, so a half range would do; full circle
//   keeps the 3c lookup trivial).
// - v: Hillaire's non-linear latitude packing texels at the horizon:
//   v = 0.5 + 0.5 * sign(l) * sqrt(|l| / (pi/2)),  l = view elevation.
//   The fragment inverts it: s = 2v - 1, l = sign(s) * s^2 * (pi/2).
//
// Constants TAU_RAYLEIGH / TAU_MIE / MIE_G mirror src/renderer/atmosphere.rs
// (the canonical values; update BOTH if they ever change).

struct SkyViewUniform {
    // rgb = Rayleigh tint (planet atmosphere color), a = density multiplier.
    tint_density: vec4<f32>,
    // x = camera radius r0 (shell space), y = sun elevation cos mu_s,
    // z = planet surface radius rp (shell space), w = scale height h (shell).
    geom: vec4<f32>,
}

@group(0) @binding(0) var<uniform> u_sky: SkyViewUniform;
@group(0) @binding(1) var trans_lut: texture_2d<f32>;
@group(0) @binding(2) var ms_lut: texture_2d<f32>;
@group(0) @binding(3) var lut_samp: sampler;

const PI: f32 = 3.14159265;
const TAU_RAYLEIGH: f32 = 0.6;
const TAU_MIE: f32 = 0.02;
const MIE_G: f32 = 0.76;
const STEPS: i32 = 30;
// Mirrors atmo_luts::ALT_SPAN_H (the LUT v-axis span in scale heights).
const ALT_SPAN_H: f32 = 12.0;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    // Fullscreen triangle.
    var out: VsOut;
    let x = f32(i32(vi & 1u) * 4 - 1);
    let y = f32(i32(vi >> 1u) * 4 - 1);
    out.pos = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>(x, -y) * 0.5 + 0.5;
    return out;
}

fn rayleigh_phase(c: f32) -> f32 {
    return 0.0596831 * (1.0 + c * c);
}

fn hg_phase(c: f32, g: f32) -> f32 {
    let g2 = g * g;
    let denom = 1.0 + g2 - 2.0 * g * c;
    return (1.0 - g2) / (4.0 * PI * denom * sqrt(max(denom, 1e-6)));
}

// Sample the stage-3a transmittance LUT: per-channel T from radius r along
// cos-zenith mu out to space. Mapping mirrors atmo_luts::u_to_mu / v_to_r.
fn sample_transmittance(r: f32, mu: f32, rp: f32, h: f32) -> vec3<f32> {
    let uu = clamp((mu + 0.15) / 1.15, 0.0, 1.0);
    let span = min(ALT_SPAN_H * h, 1.0 - rp);
    let vv = clamp((r - rp) / max(span, 1e-6), 0.0, 1.0);
    return textureSampleLevel(trans_lut, lut_samp, vec2<f32>(uu, vv), 0.0).rgb;
}

fn sample_ms(r: f32, mu_s: f32, rp: f32, h: f32) -> vec3<f32> {
    let uu = clamp((mu_s + 0.15) / 1.15, 0.0, 1.0);
    let span = min(ALT_SPAN_H * h, 1.0 - rp);
    let vv = clamp((r - rp) / max(span, 1e-6), 0.0, 1.0);
    return textureSampleLevel(ms_lut, lut_samp, vec2<f32>(uu, vv), 0.0).rgb;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let tint = u_sky.tint_density.rgb;
    let density_mul = u_sky.tint_density.a;
    let r0 = u_sky.geom.x;
    let mu_s = u_sky.geom.y;
    let rp = u_sky.geom.z;
    let h = u_sky.geom.w;

    // Invert the texel mapping to a view direction.
    let phi = in.uv.x * 2.0 * PI; // azimuth from the sun
    let s = 2.0 * in.uv.y - 1.0;
    let l = sign(s) * s * s * (PI / 2.0); // non-linear latitude
    let mu_v = sin(l);
    let cos_az = cos(phi);

    // Phase angle between view and sun.
    let sin_v = sqrt(max(1.0 - mu_v * mu_v, 0.0));
    let sin_s = sqrt(max(1.0 - mu_s * mu_s, 0.0));
    let cos_theta = clamp(mu_v * mu_s + sin_v * sin_s * cos_az, -1.0, 1.0);
    let ph_r = rayleigh_phase(cos_theta);
    let ph_m = hg_phase(cos_theta, MIE_G);
    let iso = 1.0 / (4.0 * PI);

    // Per-channel extinction at surface density (the shader beta = tau / h
    // convention; 1.11 = the Mie absorption factor).
    let beta_e = (tint * TAU_RAYLEIGH + vec3<f32>(TAU_MIE * 1.11)) / h * density_mul;
    // Density-independent scattering/extinction ratios (see the CPU twin).
    let sig_r_ratio = (tint * TAU_RAYLEIGH / h * density_mul) / beta_e;
    let sig_m_ratio = vec3<f32>(TAU_MIE / h * density_mul) / beta_e;

    let span = max(1.0 - r0, h);
    var seg: f32;
    if (abs(mu_v) > 0.05) {
        seg = min(span / abs(mu_v), span * 8.0);
    } else {
        seg = span * 8.0;
    }
    let dt = seg / f32(STEPS);

    var radiance = vec3<f32>(0.0);
    var od_before = 0.0;
    for (var si: i32 = 0; si < STEPS; si++) {
        let t_mid = (f32(si) + 0.5) * dt;
        let r_s = r0 + t_mid * mu_v;
        if (r_s < rp) {
            break; // ground hit
        }
        let r_c = min(r_s, 1.0);
        let dens = exp(-(r_c - rp) / h);
        let od_after = od_before + dens * dt;
        let t_sun = sample_transmittance(r_c, mu_s, rp, h);
        let psi = sample_ms(r_c, mu_s, rp, h);
        // Analytic per-step integration: (sigma/beta_ext)*(T_start - T_end),
        // exact for any step optical thickness (midpoint sampling read the
        // horizon 7x DIMMER than the zenith - see the CPU twin's comment).
        let t_start = exp(-beta_e * od_before);
        let t_end = exp(-beta_e * od_after);
        let step_energy = t_start - t_end;
        let single = (sig_r_ratio * ph_r + sig_m_ratio * ph_m) * step_energy * t_sun;
        let ms = (sig_r_ratio + sig_m_ratio) * step_energy * psi * iso;
        radiance += single + ms;
        od_before = od_after;
    }
    return vec4<f32>(radiance, 1.0);
}
