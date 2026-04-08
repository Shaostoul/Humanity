// bloom.wgsl — Two-pass bloom: threshold+downsample, then blur+composite.
//
// Pass 1 (threshold): Extract bright pixels from the scene.
// Pass 2 (blur_h): Horizontal Gaussian blur on the bright pixels.
// Pass 3 (blur_v): Vertical Gaussian blur.
// Pass 4 (composite): Add blurred bloom back to the original scene.
//
// All passes use fullscreen triangle (vertex shader generates coverage).

struct BloomParams {
    // x = threshold, y = intensity, z = texel_w, w = texel_h
    params: vec4<f32>,
};

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;
@group(0) @binding(2) var<uniform> bloom: BloomParams;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// Fullscreen triangle trick: 3 vertices cover the entire screen
@vertex
fn vs_fullscreen(@builtin(vertex_index) vertex_id: u32) -> VertexOutput {
    var out: VertexOutput;
    // Generate oversized triangle
    let x = f32(i32(vertex_id & 1u) * 4 - 1);
    let y = f32(i32(vertex_id >> 1u) * 4 - 1);
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

// Pass 1: Extract bright pixels above threshold
@fragment
fn fs_threshold(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(input_texture, input_sampler, in.uv);
    let brightness = dot(color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let threshold = bloom.params.x;
    if brightness > threshold {
        let excess = brightness - threshold;
        return vec4<f32>(color.rgb * (excess / brightness), 1.0);
    }
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}

// 9-tap Gaussian weights
const WEIGHTS: array<f32, 5> = array<f32, 5>(
    0.227027, 0.1945946, 0.1216216, 0.054054, 0.016216
);

// Pass 2: Horizontal Gaussian blur
@fragment
fn fs_blur_h(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel = vec2<f32>(bloom.params.z, 0.0); // horizontal step
    var result = textureSample(input_texture, input_sampler, in.uv).rgb * WEIGHTS[0];
    for (var i = 1; i < 5; i = i + 1) {
        let offset = texel * f32(i);
        result += textureSample(input_texture, input_sampler, in.uv + offset).rgb * WEIGHTS[i];
        result += textureSample(input_texture, input_sampler, in.uv - offset).rgb * WEIGHTS[i];
    }
    return vec4<f32>(result, 1.0);
}

// Pass 3: Vertical Gaussian blur
@fragment
fn fs_blur_v(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel = vec2<f32>(0.0, bloom.params.w); // vertical step
    var result = textureSample(input_texture, input_sampler, in.uv).rgb * WEIGHTS[0];
    for (var i = 1; i < 5; i = i + 1) {
        let offset = texel * f32(i);
        result += textureSample(input_texture, input_sampler, in.uv + offset).rgb * WEIGHTS[i];
        result += textureSample(input_texture, input_sampler, in.uv - offset).rgb * WEIGHTS[i];
    }
    return vec4<f32>(result, 1.0);
}

// For composite pass, we need both the original scene and the bloom texture
@group(1) @binding(0) var bloom_texture: texture_2d<f32>;
@group(1) @binding(1) var bloom_sampler: sampler;

// Pass 4: Composite bloom onto original scene
@fragment
fn fs_composite(in: VertexOutput) -> @location(0) vec4<f32> {
    let scene = textureSample(input_texture, input_sampler, in.uv);
    let bloom_color = textureSample(bloom_texture, bloom_sampler, in.uv);
    let intensity = bloom.params.y;
    return vec4<f32>(scene.rgb + bloom_color.rgb * intensity, scene.a);
}
