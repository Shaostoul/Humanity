// drywall.wgsl — Procedural painted drywall with subtle paper texture and brush strokes.
// Designed for interior walls: fine paper grain, paint brush marks, and configurable
// base color through material uniform. PBR lit with point lights.
//
// Binding conventions:
//   Group 0: Camera, Group 1: Mesh transform, Group 2: Lights, Group 3: Material
//   Vertex inputs: position, normal, tangent, bitangent, uv (standard layout)

struct Camera { view_proj: mat4x4<f32>, position: vec3<f32>, _pad: f32, };
@group(0) @binding(0) var<uniform> camera: Camera;
struct MeshTransform { matrix: mat4x4<f32>, };
@group(1) @binding(0) var<uniform> mesh_transform: MeshTransform;
const MAX_LIGHTS: u32 = 16u;
struct Light { position: vec3<f32>, intensity: f32, color: vec3<f32>, range: f32, };
@group(2) @binding(0) var<uniform> lights: array<Light, MAX_LIGHTS>;
struct Material { base_color: vec4<f32>, metallic: f32, roughness: f32, ao: f32, ambient_strength: f32, };
@group(3) @binding(0) var<uniform> material: Material;

struct VertexOutput { @builtin(position) clip_position: vec4<f32>, @location(0) world_pos: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) tex_coords: vec2<f32>, };

@vertex
fn vs_main(@location(0) p: vec3<f32>, @location(1) n: vec3<f32>, @location(2) t: vec3<f32>, @location(3) b: vec3<f32>, @location(4) uv: vec2<f32>) -> VertexOutput {
    var out: VertexOutput;
    let wp = (mesh_transform.matrix * vec4<f32>(p, 1.0)).xyz;
    out.clip_position = camera.view_proj * vec4<f32>(wp, 1.0);
    out.world_pos = wp; out.normal = normalize((mesh_transform.matrix * vec4<f32>(n, 0.0)).xyz); out.tex_coords = uv;
    return out;
}

const PI: f32 = 3.14159265359;

fn drywall_texture(uv: vec2<f32>) -> vec3<f32> {
    let paper_grain = sin(uv.x * 200.0) * sin(uv.y * 180.0) * 0.02;
    let paper_var = sin(uv.x * 50.0 + uv.y * 30.0) * 0.015;
    let brush_x = sin(uv.y * 8.0) * 0.005;
    let brush_y = cos(uv.x * 12.0) * 0.003;
    return vec3<f32>(1.0) + vec3<f32>(paper_grain + paper_var + brush_x + brush_y);
}

fn distribution_ggx(n: vec3<f32>, h: vec3<f32>, r: f32) -> f32 { let a=r*r; let a2=a*a; let ndh=max(dot(n,h),0.0); var d=(ndh*ndh*(a2-1.0)+1.0); d=PI*d*d; return a2/d; }
fn geometry_schlick_ggx(ndv: f32, r: f32) -> f32 { let k=((r+1.0)*(r+1.0))/8.0; return ndv/(ndv*(1.0-k)+k); }
fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, r: f32) -> f32 { return geometry_schlick_ggx(max(dot(n,v),0.0),r)*geometry_schlick_ggx(max(dot(n,l),0.0),r); }

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.normal); let v = normalize(camera.position - in.world_pos);
    let albedo = drywall_texture(in.tex_coords) * material.base_color.rgb;
    let f0 = mix(vec3<f32>(0.04), albedo, material.metallic);
    var radiance = vec3<f32>(0.0);
    for (var i: u32 = 0u; i < MAX_LIGHTS; i = i + 1u) {
        let light = lights[i]; if (light.intensity <= 0.0) { continue; }
        let ltf = light.position - in.world_pos; let dist = length(ltf);
        if (dist > light.range) { continue; }
        let att = max(0.0, 1.0 - pow(clamp(dist / light.range, 0.0, 1.0), 2.0));
        let l = normalize(ltf); let h = normalize(v + l);
        let ndf = distribution_ggx(n, h, material.roughness); let g = geometry_smith(n, v, l, material.roughness);
        let f = f0 + (1.0 - f0) * pow(clamp(1.0 - max(dot(h, v), 0.0), 0.0, 1.0), 5.0);
        let ks = f; let kd = (vec3<f32>(1.0) - ks) * (1.0 - material.metallic);
        let spec = (ndf * g * f) / (4.0 * max(dot(n, v), 0.0) * max(dot(n, l), 0.0) + 0.0001);
        radiance += (kd * albedo / PI * max(dot(n, l), 0.0) + spec) * light.color * light.intensity * att;
    }
    var color = albedo * material.ambient_strength + radiance;
    color = color / (color + vec3<f32>(1.0));
    color = pow(color, vec3<f32>(1.0 / 2.2));
    return vec4<f32>(color, 1.0);
}
