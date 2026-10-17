// The 3D model viewer's finished picture, copied into the page's rectangle inside egui's own
// render pass (docs/design/model-viewer.md). A shader of its own: it binds group 0 as the
// picture, which the model shader binds as its camera.

@group(0) @binding(0) var shown: texture_2d<f32>;
@group(0) @binding(1) var shown_samp: sampler;

struct BlitOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_blit(@builtin(vertex_index) i: u32) -> BlitOut {
    var o: BlitOut;
    let x = f32((i << 1u) & 2u);
    let y = f32(i & 2u);
    o.clip = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    o.uv = vec2<f32>(x, y);
    return o;
}

// For a window whose pixels are sRGB: the hardware encodes, so linear goes out.
@fragment
fn fs_blit_linear(in: BlitOut) -> @location(0) vec4<f32> {
    return textureSample(shown, shown_samp, in.uv);
}

fn encode_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

// For a window whose pixels are not sRGB: encode here, as egui itself does there.
@fragment
fn fs_blit_encode(in: BlitOut) -> @location(0) vec4<f32> {
    let c = textureSample(shown, shown_samp, in.uv);
    return vec4<f32>(encode_srgb(c.rgb), c.a);
}
