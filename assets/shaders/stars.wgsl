// Star skybox shader — renders the real star catalog as colored points
// (120k standard / 2.5M extended / 25M ultra tiers, all the same pipeline).
// Stars are at infinity: only camera rotation affects them, not translation.
// The vertex shader multiplies direction by a large radius and uses a
// rotation-only view-projection matrix passed in the camera uniform.
//
// PACKED VERTEX (2026-07-11, 12 B/star; keeps the 25M-star ultra tier at
// ~300 MB of GPU vertex memory instead of ~700 MB):
//   location(0) Snorm16x4: quantized unit direction; w is zero padding
//     (wgpu has no Snorm16x3). Renormalized below; max angular error
//     ~0.0015 degrees, far below a pixel (error math: StarVertex in
//     src/renderer/stars.rs).
//   location(1) Unorm8x4: rgb = linear star color; a = sqrt(brightness),
//     decoded a*a here (gamma-2: linear u8 would band the faint stars that
//     dominate the big catalogs through the sRGB framebuffer curve).
// Keep in sync with FALLBACK_STAR_SHADER in src/renderer/stars.rs.

// Must match the Rust-side CameraUniforms struct exactly (672 bytes, v0.639).
struct CameraUniforms {
    view_proj: mat4x4<f32>,
    view_pos: vec4<f32>,
    // Point lights (unused by stars, but must match buffer layout)
    light0: vec4<f32>, light1: vec4<f32>, light2: vec4<f32>, light3: vec4<f32>,
    light4: vec4<f32>, light5: vec4<f32>, light6: vec4<f32>, light7: vec4<f32>,
    light0_color: vec4<f32>, light1_color: vec4<f32>, light2_color: vec4<f32>, light3_color: vec4<f32>,
    light4_color: vec4<f32>, light5_color: vec4<f32>, light6_color: vec4<f32>, light7_color: vec4<f32>,
    // Spot cone data (unused by stars, but must match buffer layout)
    light0_spot: vec4<f32>, light1_spot: vec4<f32>, light2_spot: vec4<f32>, light3_spot: vec4<f32>,
    light4_spot: vec4<f32>, light5_spot: vec4<f32>, light6_spot: vec4<f32>, light7_spot: vec4<f32>,
    light0_cone_inner: vec4<f32>, light1_cone_inner: vec4<f32>, light2_cone_inner: vec4<f32>, light3_cone_inner: vec4<f32>,
    light4_cone_inner: vec4<f32>, light5_cone_inner: vec4<f32>, light6_cone_inner: vec4<f32>, light7_cone_inner: vec4<f32>,
    light_count: vec4<f32>,
    // Directional lights (unused by stars, but must match buffer layout)
    sun_direction: vec4<f32>,
    sun_color: vec4<f32>,
    fill_direction: vec4<f32>,
    fill_color: vec4<f32>,
};
@group(0) @binding(0)
var<uniform> camera: CameraUniforms;

struct StarInput {
    // Packed direction (Snorm16x4; w = pad). GPU hands us floats in [-1,1].
    @location(0) direction: vec4<f32>,
    // Packed color (Unorm8x4): rgb linear, a = sqrt(brightness).
    @location(1) color_brightness: vec4<f32>,
};

struct StarOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) brightness: f32,
};

@vertex
fn vs_main(input: StarInput) -> StarOutput {
    var out: StarOutput;

    // Renormalize the quantized direction (unit to within 2.6e-5 before this;
    // exact after), then push the star far away (within the star far plane).
    // The camera uniform for stars has translation stripped,
    // so this only rotates with the camera.
    let dir = normalize(input.direction.xyz);
    let world_pos = dir * 5000.0;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);

    out.color = input.color_brightness.rgb;
    // Gamma-2 decode of the sqrt-encoded brightness.
    let s = input.color_brightness.a;
    out.brightness = s * s;

    return out;
}

@fragment
fn fs_main(input: StarOutput) -> @location(0) vec4<f32> {
    // Brightness modulates alpha and intensity.
    let intensity = input.brightness;
    let color = input.color * intensity;
    return vec4<f32>(color, intensity);
}
