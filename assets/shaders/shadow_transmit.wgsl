// THE PARTIAL CASTERS (2026-11-13, renderer/shadow_transmit.rs; docs/design/local-light-shadows.md
// section 10, "Built: partial casters").
//
// Glass and a tent's frosted film block only part of a lamp's light. The depth atlas holds what
// blocks it all; beside it, a TRANSMITTANCE ATLAS (the same faces and tiles at half the texels,
// Rgba16Float) holds, per texel, how much of the light gets through the sheets in front of it
// (rgb, multiplied together by the blend: two sheets pass the product) and how far from the light
// the nearest of those sheets is (a, kept by a Min blend), so a surface in front of the sheet (the
// sheet itself, a person between the lamp and the tent) is not dimmed. Cleared to white and
// "nothing here" each frame a sheet is drawn. Read by shadow_atlas_view in
// 00-bindings-vertex.wgsl (`shadow_transmit_at`), which multiplies the light by it.
//
// WHAT A SHEET PASSES (`partial_transmittance`, CPU twin shadow_transmit::transmittance, lockstep
// test transmit_formula_matches_the_shader):
// - A frosted film (part.w > 0, its face-on share): a closed cover, so the light is counted where
//   it ENTERS (a face turned toward the light) and not again where it leaves (the far side's
//   shadow falls outside the cover, on what is behind it, which then reads one sheet, not two);
//   more opaque at a slant, T0^(1/cos), the Beer-Lambert slab the film's own look uses.
// - Glass (part.w < 0): a pane is crossed once whichever side the light is on, so both faces;
//   two surfaces each reflecting Schlick's Fresnel with f0 0.04, (1 - F)^2: 0.92 face-on.
// Both tinted by part.rgb (the sheet's colour, worked out on the processor).
//
// WHERE (the metric in a): a perspective view (a lamp's cube face or spot) the distance from the
// light in metres; the orthographic near sun tile the distance along the sunlight, -dot(p, toward
// the sun), in render space (both written and read in the same frame). f16 keeps a few millimetres
// at lamp distances and about a centimetre across the sun tile.
//
// Drawn per caster per view (the per-draw loop: few sheets, the tents and the windows), into its
// view's half-size tile by the viewport, no depth test: a sheet behind an opaque caster dims only
// what the opaque caster already shadows.

struct TransmitView {
    view_proj: mat4x4<f32>,
    // xyz: the light's position (perspective) or the direction toward it (orthographic); w: 1 for
    // an orthographic view.
    origin: vec4<f32>,
};

@group(0) @binding(0) var<storage, read> views: array<TransmitView>;

const GLASS_F0: f32 = 0.04;

struct SheetIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) model0: vec4<f32>,
    @location(3) model1: vec4<f32>,
    @location(4) model2: vec4<f32>,
    @location(5) model3: vec4<f32>,
    // rgb: the sheet's tint; w: a film's face-on share (> 0) or -1 for glass.
    @location(6) part: vec4<f32>,
};

struct SheetOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) @interpolate(flat) part: vec4<f32>,
    @location(3) @interpolate(flat) view: u32,
};

@vertex
fn vs_transmit(v: SheetIn) -> SheetOut {
    // The view's index rides in the first column's w, as on the fast caster path.
    let model = mat4x4<f32>(vec4<f32>(v.model0.xyz, 0.0), v.model1, v.model2, v.model3);
    let k = u32(v.model0.w + 0.5);
    let world = model * vec4<f32>(v.position, 1.0);
    var o: SheetOut;
    o.clip = views[k].view_proj * vec4<f32>(world.xyz, 1.0);
    o.world = world.xyz;
    o.normal = (model * vec4<f32>(v.normal, 0.0)).xyz;
    o.part = v.part;
    o.view = k;
    return o;
}

// What a sheet passes of light arriving at cosine c to its normal: rgb, and w 0 where the sheet
// is not counted (a film's face turned away from the light).
fn partial_transmittance(part: vec4<f32>, c: f32) -> vec4<f32> {
    let mu = max(abs(c), 0.05);
    if (part.w > 0.0) {
        if (c <= 0.0) {
            return vec4<f32>(1.0, 1.0, 1.0, 0.0);
        }
        return vec4<f32>(part.rgb * pow(part.w, 1.0 / mu), 1.0);
    }
    let f = GLASS_F0 + (1.0 - GLASS_F0) * pow(1.0 - mu, 5.0);
    return vec4<f32>(part.rgb * ((1.0 - f) * (1.0 - f)), 1.0);
}

@fragment
fn fs_transmit(i: SheetOut) -> @location(0) vec4<f32> {
    let v = views[i.view];
    var to_light = v.origin.xyz;
    var metric = -dot(i.world, v.origin.xyz);
    if (v.origin.w < 0.5) {
        let d = v.origin.xyz - i.world;
        metric = length(d);
        to_light = d / max(metric, 1.0e-6);
    }
    let n = normalize(i.normal);
    let t = partial_transmittance(i.part, dot(n, to_light));
    if (t.w < 0.5) {
        discard;
    }
    return vec4<f32>(t.rgb, metric);
}
