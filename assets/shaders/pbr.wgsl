// pbr.wgsl — Full physically-based rendering shader with Cook-Torrance BRDF.
// Supports: albedo, metallic, roughness, normal mapping, ambient occlusion, and emission.
// Uses GGX normal distribution, Smith geometry, and Schlick Fresnel approximation.
// Up to 16 point lights with distance attenuation. HDR tonemapping + gamma correction.
//
// Binding conventions:
//   Group 0: Camera/view uniforms
//   Group 1: Material/object uniforms (mesh transform)
//   Group 2: Textures/samplers (albedo, normal, metallic, roughness, AO, emission)
//   Group 3: Lights + material parameters
//   Vertex inputs: position, normal, tangent, bitangent, uv (standard layout)

struct Camera {
    view_proj: mat4x4<f32>,
    position: vec3<f32>,
    _padding: f32,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var<uniform> mesh_transform: mat4x4<f32>;

// --- Textures (Group 2) ---
@group(2) @binding(0)  var base_color_texture: texture_2d<f32>;
@group(2) @binding(1)  var base_color_sampler: sampler;
@group(2) @binding(2)  var normal_texture: texture_2d<f32>;
@group(2) @binding(3)  var normal_sampler: sampler;
@group(2) @binding(4)  var metallic_texture: texture_2d<f32>;
@group(2) @binding(5)  var metallic_sampler: sampler;
@group(2) @binding(6)  var roughness_texture: texture_2d<f32>;
@group(2) @binding(7)  var roughness_sampler: sampler;
@group(2) @binding(8)  var ao_texture: texture_2d<f32>;
@group(2) @binding(9)  var ao_sampler: sampler;
@group(2) @binding(10) var emission_texture: texture_2d<f32>;
@group(2) @binding(11) var emission_sampler: sampler;

// --- Material + Lights (Group 3) ---
struct MaterialUniform {
    base_color: vec4<f32>,
    metallic: f32,
    roughness: f32,
    ao: f32,
    emission_strength: f32,
    emission_color: vec3<f32>,
    _padding: f32,
};

const MAX_LIGHTS: u32 = 16u;
struct Light {
    position: vec3<f32>,
    intensity: f32,
    color: vec3<f32>,
    range: f32,
};

@group(3) @binding(0)
var<uniform> material: MaterialUniform;

@group(3) @binding(1)
var<uniform> lights: array<Light, MAX_LIGHTS>;

// --- Vertex ---

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec3<f32>,
    @location(3) bitangent: vec3<f32>,
    @location(4) tex_coords: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec3<f32>,
    @location(3) bitangent: vec3<f32>,
    @location(4) tex_coords: vec2<f32>,
};

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = mesh_transform * vec4<f32>(model.position, 1.0);
    out.world_position = world_pos.xyz;
    out.normal = normalize((mesh_transform * vec4<f32>(model.normal, 0.0)).xyz);
    out.tangent = normalize((mesh_transform * vec4<f32>(model.tangent, 0.0)).xyz);
    out.bitangent = normalize((mesh_transform * vec4<f32>(model.bitangent, 0.0)).xyz);
    out.tex_coords = model.tex_coords;
    out.clip_position = camera.view_proj * world_pos;
    return out;
}

// --- PBR functions ---

const PI: f32 = 3.14159265359;

fn distribution_ggx(n: vec3<f32>, h: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let n_dot_h = max(dot(n, h), 0.0);
    let n_dot_h2 = n_dot_h * n_dot_h;
    let num = a2;
    var denom = (n_dot_h2 * (a2 - 1.0) + 1.0);
    denom = PI * denom * denom;
    return num / denom;
}

fn geometry_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32 {
    let r = (roughness + 1.0);
    let k = (r * r) / 8.0;
    return n_dot_v / (n_dot_v * (1.0 - k) + k);
}

fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    let n_dot_v = max(dot(n, v), 0.0);
    let n_dot_l = max(dot(n, l), 0.0);
    return geometry_schlick_ggx(n_dot_v, roughness) * geometry_schlick_ggx(n_dot_l, roughness);
}

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (1.0 - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

fn get_normal_from_map(normal_map: vec3<f32>, normal: vec3<f32>, tangent: vec3<f32>, bitangent: vec3<f32>) -> vec3<f32> {
    let tbn = mat3x3<f32>(tangent, bitangent, normal);
    let nm = normal_map * 2.0 - 1.0;
    return normalize(tbn * nm);
}

// --- Fragment ---

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample all PBR textures
    let base_color_sample = textureSample(base_color_texture, base_color_sampler, in.tex_coords);
    let albedo = base_color_sample.rgb * material.base_color.rgb;

    let metallic_sample = textureSample(metallic_texture, metallic_sampler, in.tex_coords);
    let metallic = metallic_sample.r * material.metallic;

    let roughness_sample = textureSample(roughness_texture, roughness_sampler, in.tex_coords);
    let roughness = roughness_sample.r * material.roughness;

    let ao_sample = textureSample(ao_texture, ao_sampler, in.tex_coords);
    let ao = ao_sample.r * material.ao;

    let emission_sample = textureSample(emission_texture, emission_sampler, in.tex_coords);
    let emission = emission_sample.rgb * material.emission_color * material.emission_strength;

    // Normal mapping
    let normal_sample = textureSample(normal_texture, normal_sampler, in.tex_coords);
    let n = get_normal_from_map(normal_sample.rgb, normalize(in.normal), normalize(in.tangent), normalize(in.bitangent));
    let v = normalize(camera.position - in.world_position);

    // Reflectance at normal incidence
    let f0 = mix(vec3<f32>(0.04), albedo, metallic);

    // Accumulate light contributions
    var radiance = vec3<f32>(0.0);
    for (var i: u32 = 0u; i < MAX_LIGHTS; i = i + 1u) {
        let light = lights[i];
        if (light.intensity <= 0.0) { continue; }
        let light_to_frag = light.position - in.world_position;
        let distance = length(light_to_frag);
        if (distance > light.range) { continue; }

        let normalized_distance = clamp(distance / light.range, 0.0, 1.0);
        let attenuation = max(0.0, 1.0 - (normalized_distance * normalized_distance));
        let l = normalize(light_to_frag);
        let h = normalize(v + l);

        let ndf = distribution_ggx(n, h, roughness);
        let g = geometry_smith(n, v, l, roughness);
        let f = fresnel_schlick(max(dot(h, v), 0.0), f0);

        let k_s = f;
        let k_d = (vec3<f32>(1.0) - k_s) * (1.0 - metallic);
        let numerator = ndf * g * f;
        let denominator = 4.0 * max(dot(n, v), 0.0) * max(dot(n, l), 0.0) + 0.0001;
        let specular = numerator / denominator;
        let n_dot_l = max(dot(n, l), 0.0);
        let diffuse = k_d * albedo / PI;
        radiance += (diffuse * n_dot_l + specular) * light.color * light.intensity * attenuation;
    }

    // Ambient + emission
    let ambient = albedo * 0.15 * ao;
    var color = ambient + radiance + emission;

    // HDR tonemapping (Reinhard)
    color = color / (color + vec3<f32>(1.0));
    // Gamma correction
    color = pow(color, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(color, base_color_sample.a * material.base_color.a);
}
