// THE BLUR BEHIND A FROSTED FILM (2026-11-13, renderer/film_blur.rs; docs/design/
// local-light-shadows.md section 10, "Built: the blur behind the film").
//
// A frosted sheet scatters the light coming through it, so what is behind it reads as soft
// shapes, softer the farther behind it is: a point of the sheet sees what lies behind it through a
// cone of scattered light, which meets a surface a gap g behind the sheet in a disc of radius
// g * tan(the cone's half-angle). This builds what the film's shading samples for that: a COLOUR
// PYRAMID of the scene behind the films (the opaque scene and the films' far sides, drawn before
// it), half the render's size at its base and halving with each level, each level a 13-tap
// downsample of the one below (Jimenez, "Next Generation Post Processing in Call of Duty: Advanced
// Warfare", SIGGRAPH 2014; the colour pyramid HDRP's rough refraction reads). The base's alpha holds
// the view depth of the nearest surface under each of its texels, the gap's far end.
//
// The film's shading (90-fragment-main.wgsl, `film_frost`) works out the disc's radius on screen,
// picks the level whose texels are that size, and blends four bilinear taps round it (a tent filter,
// so no level's square texels show).

struct BlurParams {
    // xy: the source's size in texels; zw: one over it.
    src: vec4<f32>,
    // x, y: the projection's [2][2] and [3][2] (view depth z = y / (depth + x), so z = near / depth
    // for the infinite reverse-Z projection); z: the depth for "nothing drawn" (cleared, the sky).
    proj: vec4<f32>,
};

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_samp: sampler;
@group(0) @binding(2) var<uniform> params: BlurParams;
@group(0) @binding(3) var depth_tex: texture_depth_2d;

struct FullOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_full(@builtin(vertex_index) i: u32) -> FullOut {
    let x = f32((i << 1u) & 2u);
    let y = f32(i & 2u);
    var o: FullOut;
    o.pos = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    o.uv = vec2<f32>(x, y);
    return o;
}

// The 13-tap downsample, offsets in SOURCE texels: four overlapping 2x2 boxes round the centre and
// the centre box, weighted 0.5 for the inner box and 0.125 for each outer one.
fn down13(uv: vec2<f32>) -> vec4<f32> {
    let t = params.src.zw;
    let s = src_samp;
    let a = textureSampleLevel(src_tex, s, uv + t * vec2<f32>(-2.0, -2.0), 0.0);
    let b = textureSampleLevel(src_tex, s, uv + t * vec2<f32>(0.0, -2.0), 0.0);
    let c = textureSampleLevel(src_tex, s, uv + t * vec2<f32>(2.0, -2.0), 0.0);
    let d = textureSampleLevel(src_tex, s, uv + t * vec2<f32>(-2.0, 0.0), 0.0);
    let e = textureSampleLevel(src_tex, s, uv, 0.0);
    let f = textureSampleLevel(src_tex, s, uv + t * vec2<f32>(2.0, 0.0), 0.0);
    let g = textureSampleLevel(src_tex, s, uv + t * vec2<f32>(-2.0, 2.0), 0.0);
    let h = textureSampleLevel(src_tex, s, uv + t * vec2<f32>(0.0, 2.0), 0.0);
    let i = textureSampleLevel(src_tex, s, uv + t * vec2<f32>(2.0, 2.0), 0.0);
    let j = textureSampleLevel(src_tex, s, uv + t * vec2<f32>(-1.0, -1.0), 0.0);
    let k = textureSampleLevel(src_tex, s, uv + t * vec2<f32>(1.0, -1.0), 0.0);
    let l = textureSampleLevel(src_tex, s, uv + t * vec2<f32>(-1.0, 1.0), 0.0);
    let m = textureSampleLevel(src_tex, s, uv + t * vec2<f32>(1.0, 1.0), 0.0);
    return e * 0.125 + (a + c + g + i) * 0.03125 + (b + d + f + h) * 0.0625 + (j + k + l + m) * 0.125;
}

// The pyramid's base from the scene: the colour downsampled, and in alpha the view depth of the
// nearest of the four depth texels under this one (metres; the gap is measured to the nearest so a
// shelf's edge never reads as the wall behind it).
@fragment
fn fs_base(i: FullOut) -> @location(0) vec4<f32> {
    let c = down13(i.uv);
    let base = vec2<i32>(i.pos.xy) * 2;
    let lim = vec2<i32>(params.src.xy) - vec2<i32>(1);
    var d = 0.0;
    for (var k = 0u; k < 4u; k = k + 1u) {
        let o = vec2<i32>(i32(k & 1u), i32(k >> 1u));
        d = max(d, textureLoad(depth_tex, min(base + o, lim), 0));
    }
    var z = 10000.0;
    if (d > params.proj.z) {
        z = min(params.proj.y / (d + params.proj.x), 10000.0);
    }
    return vec4<f32>(c.rgb, z);
}

// Each level above the base: the one below downsampled.
@fragment
fn fs_down(i: FullOut) -> @location(0) vec4<f32> {
    return down13(i.uv);
}
