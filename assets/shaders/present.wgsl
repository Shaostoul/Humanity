// THE PRESENT PASS: the last thing between the scene target and the display
// (HDR scene target, increment 1; docs/design/hdr-scene-target.md).
//
// Every scene pass draws into the renderer's scene target; this full-screen
// pass copies it into the display target (the swapchain, a camera screen's
// texture, a hi-res capture target), and egui draws over the result.
//
// TODAY IT IS A PASSTHROUGH, AND THAT IS THE POINT. The scene target is in the
// display format for now, so the copy must be bit-exact: textureLoad (no
// sampler, no filtering, the texel at this pixel's own integer coordinate)
// and write it back, alpha included, with no blend. Every capture taken
// through this pass is byte-identical to one drawn straight into the display.
// The later increments put the real work here: a clamp when the scene target
// becomes Rgba16Float (increment 3), then the one dither (increment 4), then
// the one tonemap (increment 5).
//
// `params.flags.x` > 0.5 clamps rgba to 0..1 before the write. It is off in
// increment 1 (an 8-bit UNORM scene holds nothing outside 0..1 anyway) and is
// the hook increment 3 needs, carried now so the bind group layout never has
// to change between increments.

struct PresentParams {
    // x: clamp to 0..1 (1) or not (0). y, z, w: reserved for the dither and
    // the tonemap increments.
    flags: vec4<f32>,
};

@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var<uniform> params: PresentParams;

// One triangle that covers the whole target: (-1,-1), (3,-1), (-1,3).
@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    let x = f32((vi << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(vi & 2u) * 2.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    // frag.xy is the pixel CENTRE (x + 0.5); truncation gives the pixel's own
    // integer coordinate. The scene target and the display target are always
    // the same size (the renderer resizes them together, and an off-screen
    // view's scratch is made at the view's size).
    var c = textureLoad(scene_tex, vec2<i32>(frag.xy), 0);
    if (params.flags.x > 0.5) {
        c = clamp(c, vec4<f32>(0.0), vec4<f32>(1.0));
    }
    return c;
}
