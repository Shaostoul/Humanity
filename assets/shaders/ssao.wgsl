// Screen-space ambient occlusion (v0.901) — depth-only, celestial-slot.
//
// Runs right after the god-ray pass, while the depth buffer still holds the
// terrain + vegetation silhouettes, and MULTIPLIES the color target
// (src * dst blend): creases, tree bases, and canyon walls pick up contact
// shading, which is most of what "everything looks uniformly lit" was
// missing up close. Depth-only estimator (no normals): a golden-angle
// spiral of taps, occlusion where neighbors sit closer to the camera than
// the fragment by a small, range-checked margin.
//
// Reverse-Z + the celestial projection's huge far plane are handled by
// linearizing with the REAL matrix elements (m22/m32 passed in the
// uniform): dist = m32 / (d + m22). Sky fragments (d ~ 0) pass through
// untouched.

struct SsaoUniforms {
    // x = m22, y = m32 of the celestial projection (column-major [2][2],
    // [3][2]), z = pixels per radian of view, w = enable.
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
    // Screen radius (pixels) of a params.x-metre neighborhood at this depth.
    let radius_px = clamp(u.params.x / max(dist0, 1.0) * u.proj.z, 2.0, 48.0);
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
        let dd = dist0 - lin_dist(dt); // positive = tap is closer (occluder)
        // Occlusion window: ignore sub-2 cm noise, fade out past ~2.5x the
        // neighborhood radius so distant foreground objects don't halo.
        let w_in = smoothstep(0.02, 0.25 * u.params.x, dd);
        let w_out = 1.0 - smoothstep(u.params.x, 2.5 * u.params.x, dd);
        occl = occl + w_in * w_out;
    }
    let ao = 1.0 - u.params.y * (occl / 10.0);
    return vec4<f32>(ao, ao, ao, 1.0);
}
