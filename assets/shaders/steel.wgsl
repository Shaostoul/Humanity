// steel.wgsl — Procedural industrial steel material with PBR lighting.
// Metallic surface with uniform color and subtle variation.
// Uses Cook-Torrance BRDF with up to 16 point lights.
//
// Binding conventions:
//   Group 0: Camera/view uniforms
//   Group 1: Material/object uniforms (mesh transform)
//   Group 2: Lights
//   Group 3: Material parameters
//   Vertex inputs: position, normal, tangent, bitangent, uv (standard layout)

struct Camera {
    view_proj: mat4x4<f32>,
    position: vec3<f32>,
    _pad: f32,
};

@group(0) @binding(0) var<uniform> camera: Camera;

struct MeshTransform { matrix: mat4x4<f32>, };
@group(1) @binding(0) var<uniform> mesh_transform: MeshTransform;

const MAX_LIGHTS: u32 = 16u;
struct Light {
    position: vec3<f32>,
    intensity: f32,
    color: vec3<f32>,
    range: f32,
};
@group(2) @binding(0) var<uniform> lights: array<Light, MAX_LIGHTS>;

struct Material {
    base_color: vec3<f32>,
    metallic: f32,
    roughness: f32,
    ao: f32,
    ambient_strength: f32,
    _padding: f32,
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
    @location(0) in_pos: vec3<f32>,
    @location(1) in_normal: vec3<f32>,
    @location(2) in_tangent: vec3<f32>,
    @location(3) in_bitangent: vec3<f32>,
    @location(4) in_uv: vec2<f32>
) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = (mesh_transform.matrix * vec4<f32>(in_pos, 1.0)).xyz;
    let world_normal = normalize((mesh_transform.matrix * vec4<f32>(in_normal, 0.0)).xyz);
    out.position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.world_pos = world_pos;
    out.normal = world_normal;
    out.uv = in_uv;
    return out;
}

const PI: f32 = 3.14159265359;

fn distribution_ggx(n: vec3<f32>, h: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness; let a2 = a * a;
    let ndh = max(dot(n, h), 0.0); let ndh2 = ndh * ndh;
    var denom = (ndh2 * (a2 - 1.0) + 1.0); denom = PI * denom * denom;
    return a2 / denom;
}

fn geometry_schlick_ggx(ndv: f32, roughness: f32) -> f32 {
    let r = (roughness + 1.0); let k = (r * r) / 8.0;
    return ndv / (ndv * (1.0 - k) + k);
}

fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    return geometry_schlick_ggx(max(dot(n, v), 0.0), roughness) * geometry_schlick_ggx(max(dot(n, l), 0.0), roughness);
}

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (1.0 - f0) * pow(1.0 - cos_theta, 5.0);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let v = normalize(camera.position - in.world_pos);
    let albedo = material.base_color;
    let f0 = mix(vec3<f32>(0.04), albedo, material.metallic);

    var radiance = vec3<f32>(0.0);
    for (var i: u32 = 0u; i < MAX_LIGHTS; i = i + 1u) {
        let light = lights[i];
        if (light.intensity <= 0.0) { continue; }
        let ltf = light.position - in.world_pos;
        let dist = length(ltf);
        if (dist > light.range) { continue; }
        let nd = clamp(dist / light.range, 0.0, 1.0);
        let att = max(0.0, 1.0 - (nd * nd));
        let l = normalize(ltf); let h = normalize(v + l);
        let ndf = distribution_ggx(n, h, material.roughness);
        let g = geometry_smith(n, v, l, material.roughness);
        let ks = f0;
        let kd = (vec3<f32>(1.0) - ks) * (1.0 - material.metallic);
        let spec = (ndf * g * ks) / (4.0 * max(dot(n, v), 0.0) * max(dot(n, l), 0.0) + 0.0001);
        let ndl = max(dot(n, l), 0.0);
        radiance += (kd * albedo / PI * ndl + spec) * light.color * light.intensity * att;
    }

    let ambient = albedo * material.ambient_strength;
    var color = ambient + radiance;
    color = color / (color + vec3<f32>(1.0));
    color = pow(color, vec3<f32>(1.0 / 2.2));
    return vec4<f32>(color, 1.0);
}
