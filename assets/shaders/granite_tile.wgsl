// granite_tile.wgsl — Procedural granite tile flooring with PBR lighting.
// Generates speckled granite patterns with grout lines between tiles.
// Gray base with dark and light mineral specks, per-tile color variation.
//
// Binding conventions:
//   Group 0: Camera/view uniforms
//   Group 1: Material/object uniforms (mesh transform)
//   Group 2: Lights
//   Group 3: Material parameters (base_color tints the granite)
//   Vertex inputs: position, normal, tangent, bitangent, uv (standard layout)

struct Camera { view_proj: mat4x4<f32>, position: vec3<f32>, _pad: f32, };
@group(0) @binding(0) var<uniform> camera: Camera;

struct MeshTransform { matrix: mat4x4<f32>, };
@group(1) @binding(0) var<uniform> mesh_transform: MeshTransform;

const MAX_LIGHTS: u32 = 16u;
struct Light { position: vec3<f32>, intensity: f32, color: vec3<f32>, range: f32, };
@group(2) @binding(0) var<uniform> lights: array<Light, MAX_LIGHTS>;

struct Material {
    base_color: vec3<f32>, metallic: f32,
    roughness: f32, ao: f32,
    tile_scale: f32,       // Tiles per meter (default ~2.0)
    grout_width: f32,      // Grout line width (default ~0.02)
};
@group(3) @binding(0) var<uniform> material: Material;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

@vertex
fn vs_main(
    @location(0) in_pos: vec3<f32>, @location(1) in_normal: vec3<f32>,
    @location(2) in_tangent: vec3<f32>, @location(3) in_bitangent: vec3<f32>,
    @location(4) in_uv: vec2<f32>
) -> VertexOutput {
    var out: VertexOutput;
    let wp = (mesh_transform.matrix * vec4<f32>(in_pos, 1.0)).xyz;
    out.position = camera.view_proj * vec4<f32>(wp, 1.0);
    out.world_pos = wp;
    out.normal = normalize((mesh_transform.matrix * vec4<f32>(in_normal, 0.0)).xyz);
    out.uv = in_uv;
    return out;
}

const PI: f32 = 3.14159265359;

fn hash(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p); let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2<f32>(1.0, 0.0)), u.x),
               mix(hash(i + vec2<f32>(0.0, 1.0)), hash(i + vec2<f32>(1.0, 1.0)), u.x), u.y);
}

fn granite_color(uv: vec2<f32>) -> vec3<f32> {
    let tile_uv = uv * material.tile_scale;
    let tile_id = floor(tile_uv);
    let local_uv = fract(tile_uv);
    let base = material.base_color;
    let dark_speck = base * 0.35;
    let light_speck = base * 1.35;
    let gn = noise(tile_uv * 8.0 + tile_id * 17.3);
    let sn = noise(tile_uv * 32.0 + tile_id * 73.1);
    var color = base;
    if (gn > 0.7) { color = mix(color, light_speck, 0.6); }
    else if (gn < 0.3) { color = mix(color, dark_speck, 0.4); }
    if (sn > 0.85) { color = mix(color, light_speck, 0.8); }
    else if (sn < 0.15) { color = mix(color, dark_speck, 0.6); }
    color += hash(tile_id) * 0.1 - 0.05;
    let gf = min(
        smoothstep(0.0, material.grout_width, local_uv.x) * smoothstep(0.0, material.grout_width, 1.0 - local_uv.x),
        smoothstep(0.0, material.grout_width, local_uv.y) * smoothstep(0.0, material.grout_width, 1.0 - local_uv.y));
    let grout = vec3<f32>(0.3, 0.3, 0.35);
    return mix(grout, color, gf);
}

fn distribution_ggx(n: vec3<f32>, h: vec3<f32>, r: f32) -> f32 { let a=r*r; let a2=a*a; let ndh=max(dot(n,h),0.0); var d=(ndh*ndh*(a2-1.0)+1.0); d=PI*d*d; return a2/d; }
fn geometry_schlick_ggx(ndv: f32, r: f32) -> f32 { let k=((r+1.0)*(r+1.0))/8.0; return ndv/(ndv*(1.0-k)+k); }
fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, r: f32) -> f32 { return geometry_schlick_ggx(max(dot(n,v),0.0),r)*geometry_schlick_ggx(max(dot(n,l),0.0),r); }

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let v = normalize(camera.position - in.world_pos);
    let albedo = granite_color(in.uv);
    let f0 = mix(vec3<f32>(0.04), albedo, material.metallic);
    var radiance = vec3<f32>(0.0);
    for (var i: u32 = 0u; i < MAX_LIGHTS; i = i + 1u) {
        let light = lights[i];
        if (light.intensity <= 0.0) { continue; }
        let ltf = light.position - in.world_pos;
        let dist = length(ltf);
        if (dist > light.range) { continue; }
        let nd = clamp(dist / light.range, 0.0, 1.0);
        let att = max(0.0, 1.0 - nd * nd);
        let l = normalize(ltf); let h = normalize(v + l);
        let ndf = distribution_ggx(n, h, material.roughness);
        let g = geometry_smith(n, v, l, material.roughness);
        let ks = f0; let kd = (vec3<f32>(1.0) - ks) * (1.0 - material.metallic);
        let spec = (ndf * g * ks) / (4.0 * max(dot(n, v), 0.0) * max(dot(n, l), 0.0) + 0.0001);
        let ndl = max(dot(n, l), 0.0);
        radiance += (kd * albedo / PI * ndl + spec) * light.color * light.intensity * att;
    }
    let ambient = albedo * 0.15;
    var color = ambient + radiance;
    color = color / (color + vec3<f32>(1.0));
    color = pow(color, vec3<f32>(1.0 / 2.2));
    return vec4<f32>(color, 1.0);
}
