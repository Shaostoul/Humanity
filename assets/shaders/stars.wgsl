// Star skybox shader — renders 119,627 real stars as colored points.
// Stars are at infinity: only camera rotation affects them, not translation.
// The vertex shader multiplies direction by a large radius and uses a
// rotation-only view-projection matrix passed in the camera uniform.

// Must match the Rust-side CameraUniforms struct exactly (416 bytes).
struct CameraUniforms {
    view_proj: mat4x4<f32>,
    view_pos: vec4<f32>,
    // Point lights (unused by stars, but must match buffer layout)
    light0: vec4<f32>, light1: vec4<f32>, light2: vec4<f32>, light3: vec4<f32>,
    light4: vec4<f32>, light5: vec4<f32>, light6: vec4<f32>, light7: vec4<f32>,
    light0_color: vec4<f32>, light1_color: vec4<f32>, light2_color: vec4<f32>, light3_color: vec4<f32>,
    light4_color: vec4<f32>, light5_color: vec4<f32>, light6_color: vec4<f32>, light7_color: vec4<f32>,
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
    @location(0) direction: vec3<f32>,
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

    // Push the star far away (within far plane).
    // The camera uniform for stars has translation stripped,
    // so this only rotates with the camera.
    let world_pos = input.direction * 5000.0;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);

    out.color = input.color_brightness.rgb;
    out.brightness = input.color_brightness.a;

    return out;
}

@fragment
fn fs_main(input: StarOutput) -> @location(0) vec4<f32> {
    // Brightness modulates alpha and intensity.
    let intensity = input.brightness;
    let color = input.color * intensity;
    return vec4<f32>(color, intensity);
}
