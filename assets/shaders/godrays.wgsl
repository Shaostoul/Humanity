// Crepuscular god rays (v0.895) — a screen-space depth march.
//
// Runs as ONE additive full-screen pass right after the celestial pass
// (terrain + bodies), while the depth buffer still holds the planet's
// silhouettes (the interior scene pass clears depth right afterwards).
// Each fragment marches the depth buffer toward the sun's screen position:
// sky taps (reverse-Z depth == 0, nothing drawn) let light through, terrain
// taps block it — so ridgelines carve visible shafts at low sun angles.
// Clouds and the water shell are alpha passes (no depth write), so rays
// pass through them; cloud-shadowed shafts are a future refinement.
//
// The Rust side skips the pass entirely when the sun is behind the camera
// or below the horizon glow window, so the cost is zero at night.

struct GodrayUniforms {
    // Sun position in uv space (0..1, y down) — may sit outside the screen.
    sun_uv: vec2<f32>,
    // Viewport aspect (w/h), so radial distance is measured in round units.
    aspect: f32,
    // Overall strength, pre-gated by daylight on the Rust side.
    intensity: f32,
    // Ray color (sunlight tint), a = unused.
    color: vec4<f32>,
};

@group(0) @binding(0) var depth_tex: texture_depth_2d;
@group(0) @binding(1) var<uniform> u: GodrayUniforms;

// Fullscreen triangle — no vertex buffer.
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
    let dims = vec2<f32>(textureDimensions(depth_tex));
    let uv = fc.xy / dims;
    let to_sun = u.sun_uv - uv;
    // Round radial distance from the sun for the glow falloff.
    let dist = length(to_sun * vec2<f32>(u.aspect, 1.0));

    // Glow gate FIRST (v0.911, perf audit #4): the radial falloff caps the
    // whole result, so when even a fully open sky could not add visible
    // light there is nothing to march - skip the 40 depth taps for the
    // majority of the screen away from the sun.
    let glow_gate = exp(-dist * 2.6);
    if (glow_gate * u.intensity < 0.003) {
        return vec4<f32>(0.0);
    }

    // March from this fragment toward the sun, accumulating sky visibility
    // with a decay so taps near the fragment matter most (the classic
    // radial-blur shaft estimator, but fed by real depth instead of a
    // bright-pass texture).
    let steps = 40u;
    var lit = 0.0;
    var wsum = 0.0;
    var decay = 1.0;
    for (var i = 1u; i <= steps; i = i + 1u) {
        let t = f32(i) / f32(steps);
        let suv = uv + to_sun * t;
        if (suv.x < 0.0 || suv.x > 1.0 || suv.y < 0.0 || suv.y > 1.0) {
            break;
        }
        let px = vec2<i32>(suv * dims);
        let d = textureLoad(depth_tex, px, 0);
        // Reverse-Z: the celestial pass clears depth to 0.0 (farthest), so
        // a still-zero tap is open sky and passes sunlight.
        let sky = select(0.0, 1.0, d <= 1.0e-7);
        lit = lit + sky * decay;
        wsum = wsum + decay;
        decay = decay * 0.965;
    }
    let open = lit / max(wsum, 1.0e-4);

    // Shafts live near the sun; fade with radial distance so the far half
    // of the sky never picks up a wash.
    let a = clamp(open * glow_gate * u.intensity, 0.0, 1.0);
    // Additive blend (ONE, ONE): rgb IS the added light.
    return vec4<f32>(u.color.rgb * a, 0.0);
}
