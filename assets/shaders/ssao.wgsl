// Screen-space ambient occlusion (v0.901; estimator rebuilt v0.1100) —
// depth-only INPUT, normal-aware occlusion, celestial-slot.
//
// Runs right after the god-ray pass, while the depth buffer still holds the
// terrain + vegetation silhouettes, and MULTIPLIES the color target.
//
// v0.1100 rebuild (BUG-062, the "tree aura"): the v0.901 estimator counted
// any tap up to 1.6 m nearer as a full occluder inside a 48 px search disc,
// so every trunk darkened ground metres behind it — a measured 6.6-9.5%
// ring hugging every silhouette at the operator's settings — and a flat
// ground plane self-occluded at grazing view angles. Now: view-space
// positions are reconstructed from depth, a surface normal comes from
// neighbor depths (smaller-delta side per axis so silhouette edges don't
// smear the normal across two surfaces), and each tap contributes
// cosine-weighted occlusion above the tangent plane with a hard range
// falloff at 2x the world radius. A separate foreground object (trunk in
// front of distant ground) fails the range falloff, so it cannot shade what
// it does not touch; taps ON a flat plane have ~zero cosine, so grazing
// ground no longer self-occludes.
//
// Reverse-Z + the celestial projection's huge far plane are handled by
// linearizing with the REAL matrix elements (m22/m32 passed in the
// uniform): dist = m32 / (d + m22). Sky fragments (d ~ 0) pass through
// untouched.

struct SsaoUniforms {
    // x = m22, y = m32 of the celestial projection (column-major [2][2],
    // [3][2]), z = focal length in PIXELS ((h/2)/tan(fov/2)), w = enable.
    proj: vec4<f32>,
    // x = world radius (m) of the occlusion neighborhood, y = strength
    // (0..1), z/w = unused.
    params: vec4<f32>,
};

@group(0) @binding(0) var ssao_depth: texture_depth_2d;
@group(0) @binding(1) var<uniform> u: SsaoUniforms;

fn lin_dist(d: f32) -> f32 {
    // Metres from the camera along the view axis; huge for the sky.
    return u.proj.y / max(d + u.proj.x, 1.0e-12);
}

// View-space position of a pixel center at linear distance `dist`.
// +x right, +y up, camera looking down -z. Signs only need to be
// self-consistent for the occlusion geometry below.
fn view_pos(px: vec2<f32>, dims: vec2<f32>, dist: f32) -> vec3<f32> {
    let off = px - 0.5 * dims;
    return vec3<f32>(off.x / u.proj.z * dist, -off.y / u.proj.z * dist, -dist);
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(3.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );
    return vec4<f32>(pos[vi], 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) fc: vec4<f32>) -> @location(0) vec4<f32> {
    if (u.proj.w < 0.5) {
        return vec4<f32>(1.0, 1.0, 1.0, 1.0);
    }
    let dims = vec2<f32>(textureDimensions(ssao_depth));
    let px = vec2<i32>(fc.xy);
    let d0 = textureLoad(ssao_depth, px, 0);
    if (d0 <= 1.0e-7) {
        return vec4<f32>(1.0, 1.0, 1.0, 1.0); // sky
    }
    let dist0 = lin_dist(d0);
    let p0 = view_pos(fc.xy, dims, dist0);

    // ---- Surface normal from neighbor depths -------------------------------
    // Per axis, use whichever side steps LESS in depth: at a silhouette edge
    // one side belongs to a different surface (or the sky, whose linearized
    // distance is astronomically large), and the smaller-delta side is the
    // one on OUR surface.
    let last = vec2<i32>(dims) - vec2<i32>(1, 1);
    let zero = vec2<i32>(0, 0);
    let dist_r = lin_dist(max(textureLoad(ssao_depth, clamp(px + vec2<i32>(1, 0), zero, last), 0), 1.0e-7));
    let dist_l = lin_dist(max(textureLoad(ssao_depth, clamp(px - vec2<i32>(1, 0), zero, last), 0), 1.0e-7));
    let dist_d = lin_dist(max(textureLoad(ssao_depth, clamp(px + vec2<i32>(0, 1), zero, last), 0), 1.0e-7));
    let dist_u = lin_dist(max(textureLoad(ssao_depth, clamp(px - vec2<i32>(0, 1), zero, last), 0), 1.0e-7));
    var dpdx: vec3<f32>;
    if (abs(dist_r - dist0) <= abs(dist0 - dist_l)) {
        dpdx = view_pos(fc.xy + vec2<f32>(1.0, 0.0), dims, dist_r) - p0;
    } else {
        dpdx = p0 - view_pos(fc.xy - vec2<f32>(1.0, 0.0), dims, dist_l);
    }
    var dpdy: vec3<f32>;
    if (abs(dist_d - dist0) <= abs(dist0 - dist_u)) {
        dpdy = view_pos(fc.xy + vec2<f32>(0.0, 1.0), dims, dist_d) - p0;
    } else {
        dpdy = p0 - view_pos(fc.xy - vec2<f32>(0.0, 1.0), dims, dist_u);
    }
    var n = normalize(cross(dpdy, dpdx));
    // Orient toward the camera (p0 points away from it); sidesteps the
    // cross-product handedness question entirely.
    if (dot(n, p0) > 0.0) {
        n = -n;
    }

    // ---- Occlusion taps ----------------------------------------------------
    // Contact AO is a decimetre-scale effect: params.x is ~0.4 m and the
    // screen disc is capped at 16 px (the old 48 px cap bound for everything
    // nearer than ~29 m and WAS the aura).
    let radius_px = clamp(u.params.x / max(dist0, 1.0) * u.proj.z, 2.0, 16.0);
    var occl = 0.0;
    // 10-tap golden-angle spiral. Constants precomputed (cos/sin of
    // n * 2.399963); radius grows sqrt(n/N) for even area coverage.
    var dirs = array<vec2<f32>, 10>(
        vec2<f32>(1.0, 0.0),
        vec2<f32>(-0.7374, 0.6755),
        vec2<f32>(0.0874, -0.9962),
        vec2<f32>(0.6083, 0.7937),
        vec2<f32>(-0.9847, -0.1744),
        vec2<f32>(0.8437, -0.5368),
        vec2<f32>(-0.2596, 0.9657),
        vec2<f32>(-0.4607, -0.8876),
        vec2<f32>(0.9392, 0.3434),
        vec2<f32>(-0.9257, 0.3782),
    );
    for (var i = 0; i < 10; i = i + 1) {
        let r = radius_px * sqrt((f32(i) + 0.5) / 10.0);
        let sp = fc.xy + dirs[i] * r;
        if (sp.x < 0.0 || sp.y < 0.0 || sp.x >= dims.x || sp.y >= dims.y) {
            continue;
        }
        let dt = textureLoad(ssao_depth, vec2<i32>(sp), 0);
        if (dt <= 1.0e-7) {
            continue; // sky tap
        }
        let pt = view_pos(sp, dims, lin_dist(dt));
        let v = pt - p0;
        let vlen = length(v);
        if (vlen < 1.0e-4) {
            continue;
        }
        // Cosine-weighted occlusion above the tangent plane. The 0.05 bias
        // rejects the surface itself (depth precision + gentle curvature),
        // which is what stops grazing ground planes self-shading.
        let cos_occ = max(0.0, dot(n, v / vlen) - 0.05);
        // Hard range falloff: beyond 2x the neighborhood radius a tap shades
        // nothing, so a trunk cannot darken ground metres behind it.
        let fall = 1.0 - smoothstep(u.params.x, 2.0 * u.params.x, vlen);
        occl = occl + cos_occ * fall;
    }
    // 1.6 gain recalibrates the cosine estimator so a deep crease at full
    // strength reaches roughly the darkening the old estimator applied to
    // true contacts (without its false positives).
    let ao = 1.0 - u.params.y * min(occl / 10.0 * 1.6, 1.0);
    return vec4<f32>(ao, ao, ao, 1.0);
}
