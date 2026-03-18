// procedural_material.wgsl — Base procedural material: zero-texture PBR foundation.
//
// Generates all material properties from noise functions alone:
//   - Noise-based albedo (configurable base color + variation color + blend amount)
//   - Noise-based roughness variation (base roughness + noise perturbation)
//   - Noise-based normal perturbation (tangent-space bump from noise gradients)
//   - Noise-based metallic variation (for mixed metal/dielectric surfaces)
//   - Noise-based AO approximation (cavity darkening from noise)
//
// This is the foundation for the "procedural-first, zero texture files" philosophy.
// Any surface can be approximated by tuning the uniform parameters.
// Use as-is for generic surfaces, or as a starting point for specialized materials.
//
// Binding conventions:
//   Group 0: Camera/view uniforms
//   Group 1: Material/object uniforms (mesh transform)
//   Group 2: Lights
//   Group 3: Procedural material parameters
//   Vertex inputs: position, normal, tangent, bitangent, uv (standard layout)

struct Camera {
    view_proj: mat4x4<f32>,
    position: vec3<f32>,
    _padding: f32,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct MeshTransform {
    matrix: mat4x4<f32>,
};

@group(1) @binding(0)
var<uniform> mesh_transform: MeshTransform;

const MAX_LIGHTS: u32 = 16u;
struct Light {
    position: vec3<f32>,
    intensity: f32,
    color: vec3<f32>,
    range: f32,
};

@group(2) @binding(0)
var<uniform> lights: array<Light, MAX_LIGHTS>;

struct ProceduralMaterial {
    // Base color + variation
    base_color: vec3<f32>,
    color_variation_amount: f32,  // 0.0 = solid color, 1.0 = full noise blend

    variation_color: vec3<f32>,
    color_noise_scale: f32,       // Noise frequency for color variation (default ~5.0)

    // Roughness
    base_roughness: f32,
    roughness_variation: f32,     // How much noise affects roughness (default ~0.2)
    roughness_noise_scale: f32,   // Noise frequency for roughness (default ~8.0)

    // Metallic
    base_metallic: f32,
    metallic_variation: f32,      // How much noise affects metallic (default ~0.0)
    metallic_noise_scale: f32,    // Noise frequency for metallic (default ~4.0)

    // Normal perturbation
    bump_strength: f32,           // How strongly noise perturbs normals (default ~0.3)
    bump_noise_scale: f32,        // Noise frequency for bump (default ~12.0)

    // AO approximation
    ao_strength: f32,             // How much noise-based AO darkens cavities (default ~0.5)
    ao_noise_scale: f32,          // Noise frequency for AO (default ~3.0)

    // Ambient
    ambient_strength: f32,        // Ambient light multiplier (default ~0.15)
};

@group(3) @binding(0)
var<uniform> mat: ProceduralMaterial;

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
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec3<f32>,
    @location(3) bitangent: vec3<f32>,
    @location(4) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = (mesh_transform.matrix * vec4<f32>(in.position, 1.0)).xyz;
    out.world_pos = world_pos;
    out.normal = normalize((mesh_transform.matrix * vec4<f32>(in.normal, 0.0)).xyz);
    out.tangent = normalize((mesh_transform.matrix * vec4<f32>(in.tangent, 0.0)).xyz);
    out.bitangent = normalize((mesh_transform.matrix * vec4<f32>(in.bitangent, 0.0)).xyz);
    out.uv = in.tex_coords;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    return out;
}

// --- Noise utilities ---

fn hash2(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash2(i + vec2<f32>(0.0, 0.0)), hash2(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash2(i + vec2<f32>(0.0, 1.0)), hash2(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y
    );
}

fn hash3(p: vec3<f32>) -> f32 {
    let p3 = fract(p * 0.1031);
    let p4 = p3 + dot(p3, p3.yzx + 19.19);
    return fract((p4.x + p4.y) * p4.z);
}

fn noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(
            mix(hash3(i + vec3<f32>(0,0,0)), hash3(i + vec3<f32>(1,0,0)), u.x),
            mix(hash3(i + vec3<f32>(0,1,0)), hash3(i + vec3<f32>(1,1,0)), u.x), u.y),
        mix(
            mix(hash3(i + vec3<f32>(0,0,1)), hash3(i + vec3<f32>(1,0,1)), u.x),
            mix(hash3(i + vec3<f32>(0,1,1)), hash3(i + vec3<f32>(1,1,1)), u.x), u.y), u.z);
}

// Fractal Brownian Motion for richer noise patterns
fn fbm2(p: vec2<f32>) -> f32 {
    var f = 0.0;
    var amp = 0.5;
    var freq = 1.0;
    for (var i = 0; i < 4; i++) {
        f += noise2(p * freq) * amp;
        freq *= 2.0;
        amp *= 0.5;
    }
    return f;
}

fn fbm3(p: vec3<f32>) -> f32 {
    var f = 0.0;
    var amp = 0.5;
    var freq = 1.0;
    for (var i = 0; i < 4; i++) {
        f += noise3(p * freq) * amp;
        freq *= 2.0;
        amp *= 0.5;
    }
    return f;
}

// Compute noise-based normal perturbation via finite differences
fn compute_bump_normal(
    world_pos: vec3<f32>,
    normal: vec3<f32>,
    tangent: vec3<f32>,
    bitangent: vec3<f32>,
    scale: f32,
    strength: f32
) -> vec3<f32> {
    let eps = 0.01;
    let p = world_pos * scale;

    // Sample noise at offset positions to get gradient
    let h_center = fbm3(p);
    let h_dx = fbm3(p + vec3<f32>(eps, 0.0, 0.0));
    let h_dy = fbm3(p + vec3<f32>(0.0, eps, 0.0));
    let h_dz = fbm3(p + vec3<f32>(0.0, 0.0, eps));

    // Gradient in world space
    let grad_x = (h_dx - h_center) / eps;
    let grad_y = (h_dy - h_center) / eps;
    let grad_z = (h_dz - h_center) / eps;
    let grad = vec3<f32>(grad_x, grad_y, grad_z);

    // Project gradient onto tangent plane and perturb normal
    let perturbed = normalize(normal - grad * strength);
    return perturbed;
}

// --- PBR functions ---

const PI: f32 = 3.14159265359;

fn distribution_ggx(n: vec3<f32>, h: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let n_dot_h = max(dot(n, h), 0.0);
    let n_dot_h2 = n_dot_h * n_dot_h;
    var denom = (n_dot_h2 * (a2 - 1.0) + 1.0);
    denom = PI * denom * denom;
    return a2 / denom;
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

// --- Fragment ---

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let world_pos = in.world_pos;

    // --- Procedural albedo ---
    // Use 3D world-space noise so the pattern is consistent from all angles
    let color_noise = fbm3(world_pos * mat.color_noise_scale);
    let albedo = mix(
        mat.base_color,
        mat.variation_color,
        color_noise * mat.color_variation_amount
    );

    // --- Procedural roughness ---
    let roughness_noise = fbm3(world_pos * mat.roughness_noise_scale + 7.31);
    let roughness = clamp(
        mat.base_roughness + (roughness_noise - 0.5) * mat.roughness_variation,
        0.05, 1.0
    );

    // --- Procedural metallic ---
    let metallic_noise = fbm3(world_pos * mat.metallic_noise_scale + 13.7);
    let metallic = clamp(
        mat.base_metallic + (metallic_noise - 0.5) * mat.metallic_variation,
        0.0, 1.0
    );

    // --- Procedural normal perturbation ---
    let n = compute_bump_normal(
        world_pos,
        normalize(in.normal),
        normalize(in.tangent),
        normalize(in.bitangent),
        mat.bump_noise_scale,
        mat.bump_strength
    );

    // --- Procedural AO ---
    let ao_noise = fbm3(world_pos * mat.ao_noise_scale + 23.1);
    let ao = 1.0 - (1.0 - ao_noise) * mat.ao_strength;

    // --- PBR lighting ---
    let v = normalize(camera.position - world_pos);
    let f0 = mix(vec3<f32>(0.04), albedo, metallic);

    var radiance = vec3<f32>(0.0);
    for (var i: u32 = 0u; i < MAX_LIGHTS; i = i + 1u) {
        let light = lights[i];
        if (light.intensity <= 0.0) { continue; }
        let light_to_frag = light.position - world_pos;
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

    // Ambient with AO
    let ambient = albedo * mat.ambient_strength * ao;
    var color = ambient + radiance;

    // HDR tonemapping (Reinhard)
    color = color / (color + vec3<f32>(1.0));
    // Gamma correction
    color = pow(color, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(color, 1.0);
}
