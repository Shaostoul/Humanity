// The 3D model viewer's shading (docs/design/model-viewer.md, "Lighting and shading"): glTF's
// metallic-roughness model (GGX, Smith-Schlick, Schlick Fresnel), three studio lights that move
// with the camera, a sky-and-ground hemisphere, then exposure and the ACES curve. The website draws
// the same (web/shared/model-view.js).

struct Frame {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    // World-space directions towards each light (xyz) and intensity (w).
    light_dir: array<vec4<f32>, 3>,
    light_color: array<vec4<f32>, 3>,
};
@group(0) @binding(0) var<uniform> frame: Frame;

struct Material {
    base: vec4<f32>,
    // rgb emissive factor.
    emissive: vec4<f32>,
    // metallic, roughness, alpha cutoff, alpha mode (0 opaque, 1 mask, 2 blend).
    params: vec4<f32>,
    // normal scale, occlusion strength, has normal texture, unused.
    extra: vec4<f32>,
};
@group(1) @binding(0) var<uniform> mat: Material;
@group(1) @binding(1) var base_tex: texture_2d<f32>;
@group(1) @binding(2) var mr_tex: texture_2d<f32>;
@group(1) @binding(3) var normal_tex: texture_2d<f32>;
@group(1) @binding(4) var occ_tex: texture_2d<f32>;
@group(1) @binding(5) var emissive_tex: texture_2d<f32>;
// Each texture read as its file says (its own sampler).
@group(1) @binding(6) var base_samp: sampler;
@group(1) @binding(7) var mr_samp: sampler;
@group(1) @binding(8) var normal_samp: sampler;
@group(1) @binding(9) var occ_samp: sampler;
@group(1) @binding(10) var emissive_samp: sampler;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) nor: vec3<f32>,
    @location(2) uv: vec2<f32>,
};
struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) nor: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

@vertex
fn vs_main(v: VsIn) -> VsOut {
    var o: VsOut;
    o.clip = frame.view_proj * vec4<f32>(v.pos, 1.0);
    o.world = v.pos;
    o.nor = v.nor;
    o.uv = v.uv;
    return o;
}

const PI: f32 = 3.14159265;

fn ggx(n_dot_h: f32, a: f32) -> f32 {
    let a2 = a * a;
    let d = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (PI * d * d);
}

fn smith(n_dot_v: f32, n_dot_l: f32, rough: f32) -> f32 {
    let k = (rough + 1.0) * (rough + 1.0) / 8.0;
    let gv = n_dot_v / (n_dot_v * (1.0 - k) + k);
    let gl = n_dot_l / (n_dot_l * (1.0 - k) + k);
    return gv * gl;
}

fn aces(x: vec3<f32>) -> vec3<f32> {
    return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_main(in: VsOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    // Every texture read and derivative first, while all the pixel's neighbours still run.
    let base_sample = textureSample(base_tex, base_samp, in.uv);
    let mr = textureSample(mr_tex, mr_samp, in.uv);
    let tn = textureSample(normal_tex, normal_samp, in.uv).xyz * 2.0 - 1.0;
    let occ_sample = textureSample(occ_tex, occ_samp, in.uv).r;
    let emissive_sample = textureSample(emissive_tex, emissive_samp, in.uv).rgb;
    let dp1 = dpdx(in.world);
    let dp2 = dpdy(in.world);
    let duv1 = dpdx(in.uv);
    let duv2 = dpdy(in.uv);

    let base = mat.base * base_sample;
    var n = normalize(in.nor);
    if (!front) {
        n = -n; // the back of a double-sided surface
    }
    if (mat.extra.z > 0.5) {
        // Tangent frame from screen-space derivatives (no tangent attribute needed).
        let dp2perp = cross(dp2, n);
        let dp1perp = cross(n, dp1);
        let t = dp2perp * duv1.x + dp1perp * duv2.x;
        let b = dp2perp * duv1.y + dp1perp * duv2.y;
        let inv = inverseSqrt(max(max(dot(t, t), dot(b, b)), 1e-20));
        let scaled = vec3<f32>(tn.xy * mat.extra.x, tn.z);
        n = normalize(mat3x3<f32>(t * inv, b * inv, n) * scaled);
    }
    let v = normalize(frame.eye.xyz - in.world);
    let metallic = clamp(mat.params.x * mr.b, 0.0, 1.0);
    let rough = clamp(mat.params.y * mr.g, 0.04, 1.0);
    let a = rough * rough;
    let f0 = mix(vec3<f32>(0.04), base.rgb, metallic);
    let diffuse = base.rgb * (1.0 - metallic);
    let n_dot_v = max(dot(n, v), 1e-4);

    var color = vec3<f32>(0.0);
    for (var i = 0; i < 3; i = i + 1) {
        let l = normalize(frame.light_dir[i].xyz);
        let h = normalize(l + v);
        let n_dot_l = max(dot(n, l), 0.0);
        if (n_dot_l <= 0.0) {
            continue;
        }
        let f = f0 + (1.0 - f0) * pow(1.0 - max(dot(h, v), 0.0), 5.0);
        let spec = ggx(max(dot(n, h), 0.0), a) * smith(n_dot_v, n_dot_l, rough) * f / max(4.0 * n_dot_v * n_dot_l, 1e-4);
        let kd = (1.0 - f) * diffuse / PI;
        color += (kd + spec) * frame.light_color[i].rgb * frame.light_dir[i].w * n_dot_l;
    }
    let occlusion = mix(1.0, occ_sample, mat.extra.y);
    let sky = vec3<f32>(0.55, 0.60, 0.70);
    let ground = vec3<f32>(0.25, 0.22, 0.20);
    let hemi = mix(ground, sky, n.y * 0.5 + 0.5) * 0.6;
    color += hemi * (diffuse + f0 * 0.25) * occlusion;
    color += mat.emissive.rgb * emissive_sample;
    color = aces(color);

    var alpha = base.a;
    if (mat.params.w < 0.5) {
        alpha = 1.0;
    } else if (mat.params.w < 1.5) {
        if (alpha < mat.params.z) {
            discard;
        }
        alpha = 1.0;
    }
    return vec4<f32>(color, alpha);
}
