// pbr_simple.wgsl — Cook-Torrance GGX PBR shader with procedural materials.
//
// Bind groups:
//   Group 0: Camera (view_proj, view_pos)
//   Group 1: Object (model, normal_matrix) — dynamic offset
//   Group 2: Material (base_color, params: metallic/roughness/material_type)

struct CameraUniforms {
    view_proj: mat4x4<f32>,
    view_pos: vec4<f32>,
    // Point lights: xyz = position, w = intensity. Up to 8 lights.
    light0: vec4<f32>,
    light1: vec4<f32>,
    light2: vec4<f32>,
    light3: vec4<f32>,
    light4: vec4<f32>,
    light5: vec4<f32>,
    light6: vec4<f32>,
    light7: vec4<f32>,
    // xyz = color for each light, w = radius
    light0_color: vec4<f32>,
    light1_color: vec4<f32>,
    light2_color: vec4<f32>,
    light3_color: vec4<f32>,
    light4_color: vec4<f32>,
    light5_color: vec4<f32>,
    light6_color: vec4<f32>,
    light7_color: vec4<f32>,
    // Spot cone aim (v0.639): xyz = aim direction (light-to-fragment sense), w = cos(outer
    // cone half-angle). w == -1.0 is the Point/Bar sentinel -- no cone, skipped entirely.
    light0_spot: vec4<f32>,
    light1_spot: vec4<f32>,
    light2_spot: vec4<f32>,
    light3_spot: vec4<f32>,
    light4_spot: vec4<f32>,
    light5_spot: vec4<f32>,
    light6_spot: vec4<f32>,
    light7_spot: vec4<f32>,
    // Spot cone inner angle: x = cos(inner cone half-angle), yzw unused.
    light0_cone_inner: vec4<f32>,
    light1_cone_inner: vec4<f32>,
    light2_cone_inner: vec4<f32>,
    light3_cone_inner: vec4<f32>,
    light4_cone_inner: vec4<f32>,
    light5_cone_inner: vec4<f32>,
    light6_cone_inner: vec4<f32>,
    light7_cone_inner: vec4<f32>,
    // x = number of active point lights
    light_count: vec4<f32>,
    // Directional sun light: xyz = direction (toward light), w = intensity
    sun_direction: vec4<f32>,
    // Sun color: rgb, w = unused
    sun_color: vec4<f32>,
    // Fill light: xyz = direction, w = intensity
    fill_direction: vec4<f32>,
    // Fill color: rgb, w = unused
    fill_color: vec4<f32>,
};

struct ObjectUniforms {
    model: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,
};

struct MaterialUniforms {
    base_color: vec4<f32>,
    // x = metallic, y = roughness, z = material_type, w = emissive_strength
    params: vec4<f32>,
};

// One scene light in the UNCAPPED storage-buffer list (v0.782). Packing
// matches Renderer::set_point_lights: pos_intensity = [pos.xyz, intensity],
// color_range = [rgb, range], spot = [aim.xyz, cos_outer (-1 = no cone)],
// cone_inner = [cos_inner, 0, 0, 0]. The light0..7 fields above are legacy
// (unused, kept so no uniform byte offset shifts); camera.light_count.x
// bounds the loop over this buffer.
struct GpuLight {
    pos_intensity: vec4<f32>,
    color_range: vec4<f32>,
    spot: vec4<f32>,
    cone_inner: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(0) @binding(1) var<storage, read> scene_lights: array<GpuLight>;
@group(1) @binding(0) var<uniform> object: ObjectUniforms;
@group(2) @binding(0) var<uniform> material: MaterialUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = object.model * vec4<f32>(vertex.position, 1.0);
    out.world_position = world_pos.xyz;
    out.clip_position = camera.view_proj * world_pos;
    out.world_normal = normalize((object.normal_matrix * vec4<f32>(vertex.normal, 0.0)).xyz);
    out.uv = vertex.uv;
    return out;
}

// ── Lighting ──

const PI: f32 = 3.14159265359;

// Directional lights are now driven from Rust via CameraUniforms.
// camera.sun_direction.xyz = direction, .w = intensity
// camera.sun_color.rgb = color
// camera.fill_direction.xyz = direction, .w = intensity
// camera.fill_color.rgb = color

// Ambient
const AMBIENT_COLOR: vec3<f32> = vec3<f32>(0.03, 0.03, 0.05);

// ── GGX PBR Functions ──

// Normal Distribution Function (Trowbridge-Reitz GGX)
fn distribution_ggx(n_dot_h: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let denom = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (PI * denom * denom);
}

// Geometry function (Schlick-GGX)
fn geometry_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return n_dot_v / (n_dot_v * (1.0 - k) + k);
}

// Smith's method: combined geometry for both view and light directions
fn geometry_smith(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
    return geometry_schlick_ggx(n_dot_v, roughness) * geometry_schlick_ggx(n_dot_l, roughness);
}

// Fresnel-Schlick approximation
fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    let t = clamp(1.0 - cos_theta, 0.0, 1.0);
    let t2 = t * t;
    return f0 + (vec3<f32>(1.0) - f0) * (t2 * t2 * t);
}

// ── Procedural Patterns ──

// Hash function for procedural noise
fn hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + vec3<f32>(dot(p3, vec3<f32>(p3.y + 33.33, p3.z + 33.33, p3.x + 33.33)));
    return fract((p3.x + p3.y) * p3.z);
}

// Value noise
fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f); // smoothstep

    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));

    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// FBM (fractal Brownian motion) — 4 octaves
fn fbm(p: vec2<f32>) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var pp = p;
    for (var i = 0; i < 4; i = i + 1) {
        value = value + amplitude * value_noise(pp * frequency);
        frequency = frequency * 2.0;
        amplitude = amplitude * 0.5;
    }
    return value;
}

// Panel seam grid (1m panels with 3cm seam lines)
fn grid_pattern(world_pos: vec3<f32>, normal: vec3<f32>) -> f32 {
    var u: f32;
    var v: f32;
    let an = abs(normal);
    if an.y > an.x && an.y > an.z {
        u = world_pos.x;
        v = world_pos.z;
    } else if an.x > an.z {
        u = world_pos.y;
        v = world_pos.z;
    } else {
        u = world_pos.x;
        v = world_pos.y;
    }
    let seam_width = 0.03;
    let fu = fract(u);
    let fv = fract(v);
    let su = smoothstep(0.0, seam_width, fu) * smoothstep(0.0, seam_width, 1.0 - fu);
    let sv = smoothstep(0.0, seam_width, fv) * smoothstep(0.0, seam_width, 1.0 - fv);
    return su * sv;
}

// Brushed metal pattern (directional micro-scratches).
// v0.696 fix: the old vec2(u * 200.0, 0.0) sampled noise along ONE axis with
// the other pinned to zero -- mathematically that is unbroken full-length
// stripes, which is exactly the "horizontal or vertical lines of varied
// colors" the operator screenshotted. A cross-axis coordinate keeps the
// brushed DIRECTION while ending each scratch, and a low-frequency 2D breakup
// varies the field so it reads as metal, not wallpaper.
fn brushed_metal(world_pos: vec3<f32>, normal: vec3<f32>) -> f32 {
    var u: f32;
    var v: f32;
    let an = abs(normal);
    if an.y > an.x && an.y > an.z {
        u = world_pos.x;
        v = world_pos.z;
    } else if an.x > an.z {
        u = world_pos.y;
        v = world_pos.z;
    } else {
        u = world_pos.x;
        v = world_pos.y;
    }
    let scratch = value_noise(vec2<f32>(u * 200.0, v * 7.0));
    let breakup = value_noise(vec2<f32>(u * 3.0, v * 3.0));
    return mix(0.85, 1.0, scratch * 0.7 + breakup * 0.3);
}

// Concrete texture (rough, speckled surface)
fn concrete_pattern(world_pos: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    let an = abs(normal);
    var uv: vec2<f32>;
    if an.y > an.x && an.y > an.z {
        uv = world_pos.xz;
    } else if an.x > an.z {
        uv = world_pos.yz;
    } else {
        uv = world_pos.xy;
    }
    let noise = fbm(uv * 3.0);
    let speckle = value_noise(uv * 40.0) * 0.08;
    // Slightly varied grey
    let base = 0.55 + noise * 0.15 + speckle;
    return vec3<f32>(base, base * 0.98, base * 0.96);
}

// Tri-planar UV projection (reusable helper)
fn triplanar_uv(world_pos: vec3<f32>, normal: vec3<f32>) -> vec2<f32> {
    let an = abs(normal);
    if an.y > an.x && an.y > an.z {
        return world_pos.xz;
    } else if an.x > an.z {
        return world_pos.yz;
    }
    return world_pos.xy;
}

// Voronoi cell noise (returns distance to nearest cell center)
fn voronoi(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    var min_dist = 1.0;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let neighbor = vec2<f32>(f32(x), f32(y));
            let cell_center = vec2<f32>(hash21(i + neighbor), hash21(i + neighbor + vec2<f32>(57.0, 113.0)));
            let diff = neighbor + cell_center - f;
            min_dist = min(min_dist, dot(diff, diff));
        }
    }
    return sqrt(min_dist);
}

// Wood grain pattern
fn wood_pattern(world_pos: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    let an = abs(normal);
    var uv: vec2<f32>;
    if an.y > an.x && an.y > an.z {
        uv = world_pos.xz;
    } else if an.x > an.z {
        uv = world_pos.yz;
    } else {
        uv = world_pos.xy;
    }
    // Ring pattern along one axis
    let ring = sin(uv.x * 25.0 + fbm(uv * 2.0) * 6.0) * 0.5 + 0.5;
    let grain = value_noise(vec2<f32>(uv.x * 2.0, uv.y * 80.0)) * 0.1;
    // Warm wood tones
    let dark = vec3<f32>(0.35, 0.2, 0.1);
    let light = vec3<f32>(0.6, 0.4, 0.2);
    return mix(dark, light, ring) + vec3<f32>(grain);
}

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
    let center = object.model[3].xyz;
    let shell_r = length(object.model[0].xyz);
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
    let sun_radiance = camera.sun_color.rgb * camera.sun_direction.w * ATMO_EXPOSURE;
    let radiance = sun_radiance
        * (beta_ray * atmo_rayleigh_phase(cos_theta)
            + vec3<f32>(beta_mie) * atmo_mie_phase(cos_theta))
        * inscatter;

    // Per-channel transmittance of whatever sits behind this pixel,
    // collapsed to the single gray alpha fixed-function blending can
    // express. The surface stays readable at every angle because this path
    // only ever alpha-blends over it.
    let trans = exp(-beta_ext * od_view);
    let alpha = clamp(1.0 - (trans.r + trans.g + trans.b) / 3.0, 0.0, 1.0);

    // Tone-map the in-scattered light with the SAME ACES curve as the rest
    // of the pipeline; all math above is linear. The render target is an
    // sRGB view, so writing linear values is the honest handoff -- the
    // hardware applies the sRGB transfer on store, and blending against an
    // sRGB target happens in LINEAR space per the WebGPU spec (the
    // v0.802/v0.803 glow-layer lesson: know the target's gamma, encode once,
    // never twice).
    let aces_a = 2.51;
    let aces_b = 0.03;
    let aces_c = 2.43;
    let aces_d = 0.59;
    let aces_e = 0.14;
    let mapped = clamp(
        (radiance * (aces_a * radiance + vec3<f32>(aces_b)))
            / (radiance * (aces_c * radiance + vec3<f32>(aces_d)) + vec3<f32>(aces_e)),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );

    // ALPHA_BLENDING computes src.rgb * src.a + dst * (1 - src.a); divide
    // the radiance back out of the alpha so exactly `mapped` lands on
    // screen. Both terms go to zero together for thin air, so the ratio
    // stays finite; the clamp guards the pathological alpha -> 0 corner.
    let rgb = clamp(mapped / max(alpha, 1.0e-3), vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(rgb, alpha);
}

// ── Cook-Torrance BRDF Evaluation ──

fn evaluate_light(
    light_dir: vec3<f32>,
    light_color: vec3<f32>,
    light_intensity: f32,
    normal: vec3<f32>,
    view_dir: vec3<f32>,
    albedo: vec3<f32>,
    metallic: f32,
    roughness: f32,
    f0: vec3<f32>,
) -> vec3<f32> {
    let l = normalize(light_dir);
    let h = normalize(view_dir + l);

    let n_dot_l = max(dot(normal, l), 0.0);
    let n_dot_v = max(dot(normal, view_dir), 0.001);
    let n_dot_h = max(dot(normal, h), 0.0);
    let h_dot_v = max(dot(h, view_dir), 0.0);

    // Cook-Torrance specular BRDF
    let ndf = distribution_ggx(n_dot_h, roughness);
    let geo = geometry_smith(n_dot_v, n_dot_l, roughness);
    let fresnel = fresnel_schlick(h_dot_v, f0);

    let numerator = ndf * geo * fresnel;
    let denominator = 4.0 * n_dot_v * n_dot_l + 0.0001;
    let specular = numerator / denominator;

    // Energy conservation: diffuse is reduced by specular reflection
    let ks = fresnel;
    var kd = vec3<f32>(1.0) - ks;
    kd = kd * (1.0 - metallic); // Metals have no diffuse

    let diffuse = kd * albedo / PI;

    return (diffuse + specular) * light_color * light_intensity * n_dot_l;
}

// ── Fragment Shader ──

@fragment
fn fs_main(in: VertexOutput, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    let normal = normalize(in.world_normal);
    let view_dir = normalize(camera.view_pos.xyz - in.world_position);

    var albedo = material.base_color.rgb;
    var metallic = material.params.x;
    var roughness = material.params.y;
    let material_type = material.params.z;
    var proc_emissive = vec3<f32>(0.0); // extra emissive from procedural materials (e.g. lava cracks)
    var out_alpha = material.base_color.a; // types below may modulate (atmosphere fresnel)

    // Type 14 short-circuits the whole PBR surface path: an atmosphere is a
    // participating MEDIUM, not a lit surface -- its color comes entirely
    // from the scattering integral, never from a BRDF. Types >= 14.5 would
    // fall through to the default panel-grid look (none exist yet).
    if (material_type >= 13.5 && material_type < 14.5) {
        return atmosphere_scattering(in.world_position, front_facing);
    }

    // Apply procedural material based on type:
    //   0 = Panel grid (walls, floors)    4 = Glass            8 = Crystal
    //   1 = Brushed metal                 5 = Ice              9 = Rust/Corroded
    //   2 = Concrete                      6 = Water surface   10 = Moss/Growth
    //   3 = Wood                          7 = Leather         11 = Lava
    //  12 = Planet surface (vertex color packed in UV)
    //  13 = Atmosphere shell (fresnel limb tint -- the pre-v0.807 fallback)
    //  14 = Atmosphere shell (analytic single scattering -- handled above)
    if material_type < 0.5 {
        // Type 0: Default panel grid (walls, floors)
        if metallic < 0.1 && roughness > 0.3 {
            let panel = grid_pattern(in.world_position, normal);
            albedo = albedo * mix(0.65, 1.0, panel);
            roughness = mix(roughness + 0.1, roughness, panel);
        }
    } else if material_type < 1.5 {
        // Type 1: Brushed metal (metallic surfaces)
        let scratch = brushed_metal(in.world_position, normal);
        albedo = albedo * scratch;
        roughness = roughness + (1.0 - scratch) * 0.15;
    } else if material_type < 2.5 {
        // Type 2: Concrete
        albedo = concrete_pattern(in.world_position, normal) * albedo * 2.0;
        roughness = roughness + fbm(in.world_position.xz * 5.0) * 0.1;
    } else if material_type < 3.5 {
        // Type 3: Wood
        albedo = wood_pattern(in.world_position, normal);
        roughness = 0.5 + value_noise(in.world_position.xz * 10.0) * 0.2;
        metallic = 0.0;
    } else if material_type < 4.5 {
        // Type 4: Glass -- high reflectivity via Fresnel boost, subtle color shift
        let fresnel = pow(1.0 - max(dot(normal, view_dir), 0.0), 3.0);
        albedo = mix(albedo * 0.15, vec3<f32>(0.8, 0.9, 1.0), fresnel * 0.6);
        metallic = 0.1;
        roughness = 0.05 + value_noise(triplanar_uv(in.world_position, normal) * 20.0) * 0.03;
    } else if material_type < 5.5 {
        // Type 5: Ice -- blue-white tint, wrap lighting approx, crystalline noise
        let uv = triplanar_uv(in.world_position, normal);
        let crystal = voronoi(uv * 8.0);
        let wrap = dot(normal, normalize(camera.sun_direction.xyz)) * 0.5 + 0.5; // wrap lighting for SSS
        albedo = mix(vec3<f32>(0.6, 0.8, 1.0), vec3<f32>(0.95, 0.98, 1.0), crystal) * (0.7 + wrap * 0.3);
        roughness = 0.1 + crystal * 0.2;
        metallic = 0.05;
    } else if material_type < 6.5 {
        // Type 6: Water surface -- animated wave normals, blue-green, foam at shallow angles
        let uv = in.world_position.xz;
        let t = in.world_position.x * 0.01; // pseudo-time from position for static shader
        let wave = fbm(uv * 2.0 + vec2<f32>(t * 3.0, t * 1.7)) * 0.5;
        let foam = smoothstep(0.35, 0.5, wave);
        albedo = mix(vec3<f32>(0.02, 0.15, 0.2), vec3<f32>(0.05, 0.3, 0.35), wave);
        albedo = mix(albedo, vec3<f32>(0.8, 0.9, 0.95), foam * 0.6);
        roughness = mix(0.05, 0.6, foam);
        metallic = 0.02;
    } else if material_type < 7.5 {
        // Type 7: Leather -- Voronoi pore pattern, warm brown tones
        let uv = triplanar_uv(in.world_position, normal);
        let pores = voronoi(uv * 15.0);
        let coarse = fbm(uv * 4.0) * 0.15;
        albedo = mix(vec3<f32>(0.25, 0.13, 0.06), vec3<f32>(0.45, 0.28, 0.14), pores + coarse);
        roughness = 0.5 + (1.0 - pores) * 0.25;
        metallic = 0.0;
    } else if material_type < 8.5 {
        // Type 8: Crystal -- faceted sharp noise, prismatic color from view angle, high metallic
        let uv = triplanar_uv(in.world_position, normal);
        let facets = voronoi(uv * 12.0);
        let angle = dot(normal, view_dir);
        let prism = vec3<f32>(
            smoothstep(0.3, 0.7, sin(angle * 12.0) * 0.5 + 0.5),
            smoothstep(0.3, 0.7, sin(angle * 12.0 + 2.094) * 0.5 + 0.5),
            smoothstep(0.3, 0.7, sin(angle * 12.0 + 4.189) * 0.5 + 0.5)
        );
        albedo = mix(albedo * 0.3, prism, 0.7) * (0.6 + facets * 0.4);
        roughness = 0.02 + (1.0 - facets) * 0.08;
        metallic = 0.9;
    } else if material_type < 9.5 {
        // Type 9: Rust/Corroded -- FBM-driven orange-brown patches over base metal
        let uv = triplanar_uv(in.world_position, normal);
        let rust_mask = smoothstep(0.35, 0.65, fbm(uv * 3.0));
        let rust_color = vec3<f32>(0.5, 0.2, 0.05) + value_noise(uv * 20.0) * 0.1;
        albedo = mix(albedo, rust_color, rust_mask);
        roughness = mix(roughness, 0.85, rust_mask);
        metallic = mix(metallic, 0.1, rust_mask);
    } else if material_type < 10.5 {
        // Type 10: Moss/Growth -- green patches on upward/north-facing surfaces (world-space)
        let uv = in.world_position.xz;
        let up_factor = smoothstep(0.3, 0.8, normal.y); // grows on tops
        let coverage = smoothstep(0.3, 0.6, fbm(uv * 2.5)) * up_factor;
        let moss_color = vec3<f32>(0.15, 0.35, 0.08) + value_noise(uv * 12.0) * 0.08;
        albedo = mix(albedo, moss_color, coverage);
        roughness = mix(roughness, 0.9, coverage);
        metallic = mix(metallic, 0.0, coverage);
    } else if material_type < 11.5 {
        // Type 11: Lava -- black rock with glowing orange cracks, emissive in veins
        let uv = triplanar_uv(in.world_position, normal);
        let cracks = 1.0 - smoothstep(0.0, 0.12, voronoi(uv * 5.0));
        let heat = cracks * (0.7 + value_noise(uv * 8.0) * 0.3);
        albedo = mix(vec3<f32>(0.05, 0.04, 0.03), vec3<f32>(1.0, 0.35, 0.0), heat);
        proc_emissive = vec3<f32>(1.0, 0.3, 0.0) * heat * 3.0; // glowing cracks
        roughness = mix(0.9, 0.3, heat);
        metallic = 0.0;
    } else if material_type < 12.5 {
        // Type 12: Planet surface (v0.763) -- per-face color packed into the UV
        // channel by Mesh::from_planet_surface / terrain::planet_surface::
        // pack_color_to_uv. uv.x holds two 8-bit channels as one exact integer
        // (round(r*255)*256 + round(g*255)); uv.y holds blue as a plain float.
        // All three corners of a flat-shaded face carry the SAME uv, so linear
        // interpolation leaves the packed integer intact. Keep the decode in
        // sync with terrain::planet_surface::unpack_uv_to_color (unit-tested).
        let packed = u32(round(max(in.uv.x, 0.0)));
        let pr = f32((packed >> 8u) & 255u) / 255.0;
        let pg = f32(packed & 255u) / 255.0;
        albedo = vec3<f32>(pr, pg, in.uv.y) * material.base_color.rgb;
    } else if material_type < 13.5 {
        // Type 13: Atmosphere shell (v0.763) -- fresnel limb tint on a slightly
        // oversized transparent sphere. Nearly invisible looking straight
        // through the center, densest at the grazing-angle limb, so it reads as
        // a thin halo of air hugging the planet. Airless bodies simply never
        // spawn the shell. KEPT as the fallback behind Settings > Graphics >
        // Planets > "Scattering atmosphere" (off = this path): forever-dev
        // A/B reference + a safety hatch if a GPU dislikes the type-14 math.
        let limb = pow(1.0 - abs(dot(normal, view_dir)), 2.0);
        out_alpha = material.base_color.a * limb;
        proc_emissive = albedo * limb * 0.6; // limb stays visible on the night side edge
        roughness = 1.0;
        metallic = 0.0;
    }

    // Fresnel reflectance at normal incidence
    // Dielectrics: 0.04, metals: tinted by albedo
    let f0 = mix(vec3<f32>(0.04), albedo, metallic);

    // Evaluate main directional light (from camera uniforms)
    var lo = evaluate_light(
        camera.sun_direction.xyz, camera.sun_color.rgb, camera.sun_direction.w,
        normal, view_dir, albedo, metallic, roughness, f0);

    // Evaluate fill light (from camera uniforms)
    lo = lo + evaluate_light(
        camera.fill_direction.xyz, camera.fill_color.rgb, camera.fill_direction.w,
        normal, view_dir, albedo, metallic, roughness, f0);

    // Point + spot lights — UNCAPPED (v0.782): the storage buffer holds every
    // scene light; light_count bounds the loop. The early range/attenuation
    // rejection keeps far lights nearly free, so the practical ceiling is GPU
    // fill cost, not a software cap.
    let num_lights = i32(camera.light_count.x);
    for (var i = 0; i < num_lights; i = i + 1) {
        var light_pos = scene_lights[i].pos_intensity.xyz;
        let intensity = scene_lights[i].pos_intensity.w;
        let light_color = scene_lights[i].color_range.xyz;
        let radius = scene_lights[i].color_range.w;
        let sent = scene_lights[i].spot.w;

        // LINE light (v0.786, sentinel cos_outer == -2.0): the whole segment
        // [pos, spot.xyz] emits -- light each fragment from the CLOSEST point
        // on the segment (capsule-light representative point), so a strip
        // washes the full wall instead of pooling at one point. Rust mirror +
        // tests: light::line_light_closest_point.
        if (sent < -1.5) {
            let a = light_pos;
            let b = scene_lights[i].spot.xyz;
            let ab = b - a;
            let t = clamp(dot(in.world_position - a, ab) / max(dot(ab, ab), 1e-6), 0.0, 1.0);
            light_pos = a + ab * t;
        }

        let to_light = light_pos - in.world_position;
        let dist = length(to_light);

        // Cheap reject: outside the light's range, contribution is exactly 0
        // (the linear range window below hits zero at dist == radius).
        if (dist >= radius) { continue; }

        let light_dir = to_light / max(dist, 0.001);

        // Attenuation: inverse square with radius falloff
        var attenuation = intensity / (1.0 + dist * dist) * max(1.0 - dist / max(radius, 0.001), 0.0);

        // Spot cone (v0.639): cos_outer == -1.0 is the Point/Bar sentinel, so this only narrows
        // an actual spot light -- zero extra cost/behavior change for every other light.
        let spot = scene_lights[i].spot;
        let cos_outer = spot.w;
        if (cos_outer > -1.0) {
            let cos_inner = scene_lights[i].cone_inner.x;
            // spot.xyz is the aim direction in the light-to-fragment sense; -light_dir (which
            // points fragment-to-light) flips to the same sense for the dot product.
            let cos_angle = dot(normalize(spot.xyz), -light_dir);
            attenuation = attenuation * smoothstep(cos_outer, cos_inner, cos_angle);
        }

        if (attenuation > 0.001) {
            lo = lo + evaluate_light(light_dir, light_color, attenuation, normal, view_dir, albedo, metallic, roughness, f0);
        }
    }

    // Ambient (near-zero so space is truly black and the sun is the only
    // light source). A thin floor prevents absolute black so unlit faces
    // still have a subtle silhouette against the starfield instead of
    // vanishing into artefacts from tone mapping.
    let ambient = albedo * vec3<f32>(0.005, 0.005, 0.006);

    var color = ambient + lo;

    // Emissive: params.w controls emissive strength (0 = none, 1+ = glow)
    // Emissive objects use base_color as their glow color, bypassing lighting.
    let emissive_strength = material.params.w;
    if (emissive_strength > 0.0) {
        color = color + albedo * emissive_strength;
    }

    // Procedural emissive (e.g. lava cracks) -- additive, independent of params.w
    color = color + proc_emissive;

    // ACES-like tone mapping (more filmic than Reinhard)
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    color = clamp((color * (a * color + vec3<f32>(b))) / (color * (c * color + vec3<f32>(d)) + vec3<f32>(e)), vec3<f32>(0.0), vec3<f32>(1.0));

    return vec4<f32>(color, out_alpha);
}
