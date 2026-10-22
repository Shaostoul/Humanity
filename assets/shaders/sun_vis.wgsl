// HOW MUCH OF THE SUN'S DISC CAN THE CAMERA SEE (2026-10-21, renderer/sun_glare.rs).
//
// The glare round the Sun happens in the camera, so it is added over the
// finished frame in the present pass. Its strength is the light of the part of
// the disc that is not hidden, and only the depth buffers know what hides it: a
// planet's limb or a mountain (the celestial pass's depth) and the ship's hull or
// a wall (the scene pass's, which clears and reuses the same buffer). So this
// one-fragment pass samples the depth at 64 points over the disc twice:
//
//   fs_celestial, right after the celestial pass, REPLACES the 1x1 result with
//     the share of the disc nothing nearer than the Sun covers;
//   fs_scene, right before the present, MULTIPLIES it by the share no scene
//     geometry covers (that pass's depth is cleared to 0, reverse-Z).
//
// Each tap is weighted by the disc's limb darkening (green channel), so a hidden
// centre costs more than a hidden edge, as it does in light. A tap off the
// screen cannot be measured: it takes `u.misc.x`, the CPU's estimate (the planet
// limb and clouds, frame_lock::sun_disc_clear) for the celestial pass and 1 for
// the scene pass.

struct SunVisUniform {
    // xy: the disc's centre in pixels (may be off the screen), z: its radius in
    // pixels, w: the depth above which a tap is covered (reverse-Z: larger is nearer).
    disc: vec4<f32>,
    // x: the value of a tap off the screen.
    misc: vec4<f32>,
};

@group(0) @binding(0) var depth_tex: texture_depth_2d;
@group(0) @binding(1) var<uniform> u: SunVisUniform;

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    let x = f32((vi << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(vi & 2u) * 2.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

// Green-channel limb darkening (renderer::sun_disc::LIMB_ALPHA[1]).
const VIS_LIMB_ALPHA: f32 = 0.5132;
const VIS_TAPS: u32 = 64u;

fn visible_share() -> f32 {
    let dims = vec2<f32>(textureDimensions(depth_tex));
    var w_sum = 0.0;
    var v_sum = 0.0;
    for (var i = 0u; i < VIS_TAPS; i = i + 1u) {
        // A Vogel spiral: area-uniform over the unit disc.
        let rr = sqrt((f32(i) + 0.5) / f32(VIS_TAPS));
        let th = f32(i) * 2.39996323;
        let mu = sqrt(max(1.0 - rr * rr, 0.0));
        let w = pow(max(mu, 1.0e-3), VIS_LIMB_ALPHA);
        let p = u.disc.xy + vec2<f32>(cos(th), sin(th)) * rr * max(u.disc.z, 0.5);
        var open = u.misc.x;
        if (p.x >= 0.0 && p.y >= 0.0 && p.x < dims.x && p.y < dims.y) {
            let d = textureLoad(depth_tex, vec2<i32>(p), 0);
            open = select(1.0, 0.0, d > u.disc.w);
        }
        v_sum = v_sum + w * open;
        w_sum = w_sum + w;
    }
    return v_sum / max(w_sum, 1.0e-6);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(visible_share(), 0.0, 0.0, 1.0);
}
