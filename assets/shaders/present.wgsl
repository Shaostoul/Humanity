// THE PRESENT PASS: the last thing between the scene target and the display
// (HDR scene target; docs/design/hdr-scene-target.md).
//
// Every scene pass draws into the renderer's scene target; this full-screen
// pass copies it into the display target (the swapchain, a camera screen's
// texture, a hi-res capture target), and egui draws over the result.
//
// WHAT IT DOES, BY INCREMENT.
// 1 and 2: a passthrough. The scene target was in the display format, so the
//    copy was bit-exact: textureLoad (no sampler, no filtering, the texel at
//    this pixel's own integer coordinate) written back with no blend, alpha
//    included.
// 3: the scene target is Rgba16Float, so the scene passes blend at full
//    precision and only this write quantises. `flags.x` clamps rgba to 0..1
//    first: additive star sums, the aurora's One+One and additive particles
//    can now exceed 1 in the target, where an 8-bit target clamped every
//    write. A NaN from any pass is written as 0 rather than left to the
//    hardware's conversion.
// 4 (this file now): THE ONE DITHER, before the 8-bit write. See `dither`.
// 5 (later): the one tonemap, in the reserved `flags.w` lane.

struct PresentParams {
    // x: clamp rgba to 0..1 (1) or not (0).
    // y: the dither's code count: 255 for an 8-bit output, 1023 for a 10-bit
    //    one, 0 for no dither (switched off with the showcase key
    //    `present_dither`, or an output that does not quantise).
    // z: 1 when the output encodes sRGB on write (an *Srgb format), 0 when it
    //    stores the value as written.
    // w: reserved for the tonemap increment.
    flags: vec4<f32>,
};

@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var<uniform> params: PresentParams;

// ── THE SUN'S GLARE (2026-10-21, renderer/sun_disc.rs and sun_glare.rs) ──
//
// A camera's aperture diffracts part of the Sun's light into a halo round it.
// It happens in the camera, so it is added here, over the finished frame (walls,
// hull and terrain in front of the Sun are veiled by it too, as in a photograph),
// once the scene is complete. Its strength is the light of the part of the disc
// nobody hides: `sun_vis` holds that share (the depth taps of sun_vis.wgsl), the
// rest (the Sun's irradiance through the air and the clouds, times the far-field
// constant lambda / (pi^3 D)) is in `glare.amp`, per channel.
//
// The radiance at angle theta from the Sun's centre, for a limb-darkened disc of
// angular radius r convolved with the ring-averaged Airy pattern, is
//   amp * h(r / theta) / theta^3,   h(s) = (1 + B s^2 + C s^4) / (1 - s^2)^P,
// the closed form sun_disc::disc_glare checks against the brute-force
// convolution. It is LIGHT, so it is added before the tone curve: the scene
// target holds tone-mapped values, so the pixel is mapped back through the
// curve's inverse, the glare added, and the curve applied again. With no glare
// this is the identity, and the branch is skipped outright.
struct GlareParams {
    // xyz: the unit direction to the Sun in VIEW space (x right, y up, the
    // camera looking down -z); w: the disc's drawn angular radius, radians.
    sun: vec4<f32>,
    // x: the focal length in pixels; y, z: the image centre in pixels; w: 1
    // when there is glare to add this frame, 0 when not.
    view: vec4<f32>,
    // rgb: the Sun's irradiance through the air and clouds times the far-field
    // constant, per channel; w: the angle (radians) beyond which the glare is
    // below a thousandth of a display level and is not evaluated.
    amp: vec4<f32>,
};
@group(0) @binding(2) var<uniform> glare: GlareParams;
@group(0) @binding(3) var sun_vis: texture_2d<f32>;

// The closed-form fit per channel (sun_disc::GLARE_FIT_B / _C / _P, GLARE_S_MAX).
const SUN_GLARE_B: vec3<f32> = vec3<f32>(0.2320, 0.2537, 0.2903);
const SUN_GLARE_C: vec3<f32> = vec3<f32>(0.2582, 0.2992, 0.3762);
const SUN_GLARE_P: vec3<f32> = vec3<f32>(0.7827, 0.7434, 0.6791);
const SUN_GLARE_S_MAX: f32 = 0.98;

fn aces_fwd(x: vec3<f32>) -> vec3<f32> {
    return clamp((x * (2.51 * x + vec3<f32>(0.03))) / (x * (2.43 * x + vec3<f32>(0.59)) + vec3<f32>(0.14)), vec3<f32>(0.0), vec3<f32>(1.0));
}

// The inverse of the tone curve the scene shaders apply (the ACES fit, clamped):
// solve y = x (a x + b) / (x (c x + d) + e) for x >= 0. A value at the clip maps
// to the clip's own input (7.24), where adding light changes nothing anyway.
fn aces_inv(y_in: vec3<f32>) -> vec3<f32> {
    let y = clamp(y_in, vec3<f32>(0.0), vec3<f32>(0.9995));
    let qa = 2.43 * y - vec3<f32>(2.51);
    let qb = 0.59 * y - vec3<f32>(0.03);
    let qc = 0.14 * y;
    let disc = max(qb * qb - 4.0 * qa * qc, vec3<f32>(0.0));
    return max((-qb - sqrt(disc)) / (2.0 * qa), vec3<f32>(0.0));
}

// The glare's radiance at pixel `pix`, scene units, before the visible share.
fn sun_glare_at(pix: vec2<f32>) -> vec3<f32> {
    let f = max(glare.view.x, 1.0);
    let ray = normalize(vec3<f32>((pix.x - glare.view.y) / f, -(pix.y - glare.view.z) / f, -1.0));
    let theta = 2.0 * asin(clamp(0.5 * length(ray - glare.sun.xyz), 0.0, 1.0));
    if (theta > glare.amp.w) {
        return vec3<f32>(0.0);
    }
    let r = max(glare.sun.w, 1.0e-7);
    let th = max(theta, r / SUN_GLARE_S_MAX);
    let s = r / th;
    let s2 = s * s;
    let h = (vec3<f32>(1.0) + SUN_GLARE_B * s2 + SUN_GLARE_C * (s2 * s2)) / pow(vec3<f32>(1.0 - s2), SUN_GLARE_P);
    return glare.amp.rgb * h / (th * th * th);
}

// One triangle that covers the whole target: (-1,-1), (3,-1), (-1,3).
@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    let x = f32((vi << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(vi & 2u) * 2.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

// The strong per-pixel hash the cloud passes use (45-cloud-temporal.wgsl
// `pcg2d_hash`, v0.1237), copied because this shader is its own module.
// Uniform in [0, 1).
fn present_hash(v_in: vec2<u32>) -> f32 {
    var v = v_in * vec2<u32>(1664525u, 1013904223u);
    v.x = v.x + v.y * 1664525u;
    v.y = v.y + v.x * 1013904223u;
    v = v ^ (v >> vec2<u32>(16u));
    v.x = v.x + v.y * 1664525u;
    v.y = v.y + v.x * 1013904223u;
    v = v ^ (v >> vec2<u32>(16u));
    return f32(v.x ^ v.y) * (1.0 / 4294967296.0);
}

// The sRGB transfer pair (IEC 61966-2-1), what an *Srgb target applies on
// write and on read.
fn srgb_encode(v: vec3<f32>) -> vec3<f32> {
    let hi = 1.055 * pow(max(v, vec3<f32>(0.0031308)), vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(v * 12.92, hi, v > vec3<f32>(0.0031308));
}

fn srgb_decode(e: vec3<f32>) -> vec3<f32> {
    let hi = pow((max(e, vec3<f32>(0.04045)) + 0.055) / 1.055, vec3<f32>(2.4));
    return select(e / 12.92, hi, e > vec3<f32>(0.04045));
}

// ── THE ONE DITHER (increment 4, 2026-09-27) ──
//
// A slow dark gradient quantised at the write lands in flat rings one display
// level apart, and the eye reads each ring as a hard edge (the operator's
// banding report of 2026-09-24: the aurora's diffuse glow drew as nested
// ellipses). Adding noise of about one level before the write turns each hard
// ring edge into a fine grain whose AVERAGE is the true value.
//
// TRIANGULAR noise (the sum of two uniforms, -1 to +1 level), the standard
// choice: unlike one uniform it leaves no noise level visibly tied to the
// signal. One value for all three channels, so it adds no colour speckle.
// SPATIAL ONLY, keyed on the pixel and never on the frame, so a parked frame
// and every capture of it stay deterministic.
//
// It works in CODE units, where the write will land: encode (sRGB outputs),
// add the noise, decode, and let the hardware encode again. The aurora's own
// `srgb_dither` (deleted in the same commit) linearised the same thing, one
// code at value v being about v^0.5833 / 112.1 in linear; doing it exactly
// costs two pow per channel on one full-screen pass and is right at every
// value, including the ends.
//
// THE ENDS. Triangular noise of one level at black would lift an eighth of
// pure-black pixels to code 1 (the half that goes negative is clamped, the
// half that goes up is not), a mean shift of 0.125 code on every black sky.
// So within 0.6 code of either end the amplitude narrows to `c + 0.4` (or its
// mirror at white). The noise then never goes below -0.4 code, so nothing is
// clamped and nothing biased, and exact black reaches at most 0.4 code, which
// every GPU rounds back to 0: 0.4 and not 0.5, because the float to 8-bit
// conversion may round a value within a tenth of a code of the half either
// way (Direct3D allows 0.6 ULP; this GPU turned a clamped black into code 1
// with the margin at 0.5, the GPU test caught it). Black stays exactly black,
// white exactly white, and the mean error inside that last 0.6 code is 0.08
// code at worst. Everywhere else the mean is exact.
fn dither(v: vec3<f32>, pix: vec2<u32>, levels: f32, srgb: bool) -> vec3<f32> {
    let e = select(v, srgb_encode(v), srgb);
    let c = e * levels;
    let n = present_hash(pix + vec2<u32>(0x2C1Bu, 0x7F4Du))
        + present_hash(pix + vec2<u32>(0x91E3u, 0x0A57u)) - 1.0;
    let room = min(c, vec3<f32>(levels) - c) + vec3<f32>(0.4);
    let amp = clamp(room, vec3<f32>(0.0), vec3<f32>(1.0));
    let d = (c + n * amp) / levels;
    return select(d, srgb_decode(d), srgb);
}

@fragment
fn fs_main(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    // frag.xy is the pixel CENTRE (x + 0.5); truncation gives the pixel's own
    // integer coordinate. The scene target and the display target are always
    // the same size (the renderer resizes them together, and an off-screen
    // view's scratch is made at the view's size).
    let pix = vec2<u32>(frag.xy);
    var c = textureLoad(scene_tex, vec2<i32>(pix), 0);
    if (params.flags.x > 0.5) {
        // NaN fails every comparison, so `c == c` is false only for NaN.
        c = select(vec4<f32>(0.0), c, c == c);
        c = clamp(c, vec4<f32>(0.0), vec4<f32>(1.0));
    }
    if (glare.view.w > 0.5) {
        let g = sun_glare_at(frag.xy) * textureLoad(sun_vis, vec2<i32>(0, 0), 0).r;
        if (max(g.r, max(g.g, g.b)) > 1.0e-6) {
            c = vec4<f32>(aces_fwd(aces_inv(c.rgb) + g), c.a);
        }
    }
    if (params.flags.y > 0.5) {
        c = vec4<f32>(dither(c.rgb, pix, params.flags.y, params.flags.z > 0.5), c.a);
    }
    return c;
}
