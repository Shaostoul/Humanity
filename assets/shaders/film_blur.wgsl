// THE BLUR BEHIND A FROSTED FILM (2026-11-13, renderer/film_blur.rs; docs/design/
// local-light-shadows.md section 10, "Built: the blur behind the film").
//
// A frosted sheet scatters the light coming through it, so what is behind it reads as soft
// shapes, softer the farther behind it is: a point of the sheet sees what lies behind it through a
// cone of scattered light, which meets a surface a gap g behind the sheet in a disc of radius
// g * tan(the cone's half-angle). This builds what the film's shading samples for that: a COLOUR
// PYRAMID of the scene behind the films (the opaque scene and the films' far sides, drawn before
// it), half the render's size at its base and halving with each level, each level above the base a
// 13-tap downsample of the one below (Jimenez, "Next Generation Post Processing in Call of Duty:
// Advanced Warfare", SIGGRAPH 2014; the colour pyramid HDRP's rough refraction reads). Layer 1 of
// the base holds the view depth of the nearest surface under each of its texels, the gap's far end.
//
// The film's shading (90-fragment-main.wgsl, `film_frost`) works out the disc's radius on screen,
// picks the level whose texels are that size, and blends four bilinear taps round it (a tent filter,
// so no level's square texels show).
//
// THE COVERAGE (2026-11-14, film_blur.rs's header): the blur is screen-space, so near a cover's
// outline its disc reached past the film into what is BESIDE the tent (at night the dark room pulled
// the nearest tent's front 12% darker). So the pyramid is a NORMALIZED CONVOLUTION (Knutsson and
// Westin, "Normalized and Differential Convolution", CVPR 1993): `fs_cover` marks, at the render's
// size, every pixel a film covers (its faces over the scene's own depth), the base weighs each scene
// texel by that mark, every level averages colour-times-coverage and coverage alike (layer 0: rgb
// and alpha), and the film's shading divides one by the other, so a texel no film covers weighs
// nothing however wide the disc.

struct BlurParams {
    // xy: the source's size in texels; zw: one over it.
    src: vec4<f32>,
    // x, y: the projection's [2][2] and [3][2] (view depth z = y / (depth + x), so z = near / depth
    // for the infinite reverse-Z projection); z: the depth for "nothing drawn" (cleared, the sky);
    // w: 1 when the base weighs every texel one, coverage or none (showcase `film_v1` "1", the
    // merged look: the outline borrows what is beside the tent).
    proj: vec4<f32>,
};

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_samp: sampler;
@group(0) @binding(2) var<uniform> params: BlurParams;
@group(0) @binding(3) var depth_tex: texture_depth_2d;
@group(0) @binding(4) var mask_tex: texture_2d<f32>;

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

// ── THE COVERAGE PASS: the films' every face over the scene's own depth (tested, never written),
// 1 into the mask wherever one is seen; the mask is cleared to 0 first. A face seen means the scene
// under that pixel is behind a film (an occluder in front of the tent fails the test, so it is never
// blurred into the film). Its own binding (5), so its pipeline's layout holds only that. ──
struct CoverCam {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(5) var<uniform> cover_cam: CoverCam;

struct CoverIn {
    @location(0) position: vec3<f32>,
    @location(1) m0: vec4<f32>,
    @location(2) m1: vec4<f32>,
    @location(3) m2: vec4<f32>,
    @location(4) m3: vec4<f32>,
};

@vertex
fn vs_cover(v: CoverIn) -> @builtin(position) vec4<f32> {
    let model = mat4x4<f32>(v.m0, v.m1, v.m2, v.m3);
    return cover_cam.view_proj * (model * vec4<f32>(v.position, 1.0));
}

@fragment
fn fs_cover() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0);
}

// The 13-tap downsample, offsets in SOURCE texels: four overlapping 2x2 boxes round the centre and
// the centre box, weighted 0.5 for the inner box and 0.125 for each outer one. Linear in every
// channel, so colour-times-coverage and coverage stay a matched pair.
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

// The pyramid's base: layer 0 the scene behind the films downsampled by a 4 x 4 tent (weights
// 1 3 3 1 each way) over the render texels under this base texel, every texel weighed by its
// coverage, read texel by texel (a bilinear tap would mix a covered texel's coverage with its
// uncovered neighbour's colour); layer 1 in red the view depth of the nearest of the four depth
// texels under this one (metres; the gap is measured to the nearest so a shelf's edge never reads as
// the wall behind it). CPU twin of layer 0: film_blur::base_texel.
struct BaseOut {
    @location(0) colour: vec4<f32>,
    @location(1) depth: vec4<f32>,
};

@fragment
fn fs_base(i: FullOut) -> BaseOut {
    let base = vec2<i32>(i.pos.xy) * 2;
    let lim = vec2<i32>(params.src.xy) - vec2<i32>(1);
    var tent = array<f32, 4>(1.0, 3.0, 3.0, 1.0);
    let all_count = params.proj.w > 0.5;
    var acc = vec4<f32>(0.0);
    for (var y = 0; y < 4; y = y + 1) {
        for (var x = 0; x < 4; x = x + 1) {
            let p = clamp(base + vec2<i32>(x - 1, y - 1), vec2<i32>(0), lim);
            var m = textureLoad(mask_tex, p, 0).r;
            if (all_count) {
                m = 1.0;
            }
            let w = m * tent[x] * tent[y] / 64.0;
            acc = acc + vec4<f32>(textureLoad(src_tex, p, 0).rgb * w, w);
        }
    }
    var d = 0.0;
    for (var k = 0u; k < 4u; k = k + 1u) {
        let o = vec2<i32>(i32(k & 1u), i32(k >> 1u));
        d = max(d, textureLoad(depth_tex, min(base + o, lim), 0));
    }
    var z = 10000.0;
    if (d > params.proj.z) {
        z = min(params.proj.y / (d + params.proj.x), 10000.0);
    }
    return BaseOut(acc, vec4<f32>(z, 0.0, 0.0, 0.0));
}

// Each level above the base: the one below downsampled (layer 0 only).
@fragment
fn fs_down(i: FullOut) -> @location(0) vec4<f32> {
    return down13(i.uv);
}
