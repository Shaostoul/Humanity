// astro_turf.wgsl — Procedural artificial grass with blade texture and seams.
// Green synthetic turf with directional row patterns, wear marks, and
// visible backing material at seam lines. Highly diffuse, non-metallic.
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
struct Material { base_color: vec3<f32>, metallic: f32, roughness: f32, ao: f32, seam_spacing: f32, _padding: f32, };
@group(3) @binding(0) var<uniform> material: Material;

struct VertexOutput { @builtin(position) position: vec4<f32>, @location(0) world_pos: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) uv: vec2<f32>, };

@vertex
fn vs_main(@location(0) p: vec3<f32>, @location(1) n: vec3<f32>, @location(2) t: vec3<f32>, @location(3) b: vec3<f32>, @location(4) uv: vec2<f32>) -> VertexOutput {
    var out: VertexOutput;
    let wp = (mesh_transform.matrix * vec4<f32>(p, 1.0)).xyz;
    out.position = camera.view_proj * vec4<f32>(wp, 1.0);
    out.world_pos = wp; out.normal = normalize((mesh_transform.matrix * vec4<f32>(n, 0.0)).xyz); out.uv = uv;
    return out;
}

const PI: f32 = 3.14159265359;
fn hash(p: vec2<f32>) -> f32 { var p3=fract(vec3<f32>(p.xyx)*0.1031); p3+=dot(p3,p3.yzx+33.33); return fract((p3.x+p3.y)*p3.z); }
fn noise(p: vec2<f32>) -> f32 { let i=floor(p); let f=fract(p); let u=f*f*(3.0-2.0*f); return mix(mix(hash(i),hash(i+vec2<f32>(1.0,0.0)),u.x),mix(hash(i+vec2<f32>(0.0,1.0)),hash(i+vec2<f32>(1.0,1.0)),u.x),u.y); }

fn turf_color(uv: vec2<f32>) -> vec3<f32> {
    let bright = material.base_color;
    let dark = material.base_color * 0.7;
    let yellow = material.base_color * vec3<f32>(2.0, 1.15, 1.0);
    let bn1 = noise(uv * 150.0); let bn2 = noise(uv * 300.0);
    let bp = bn1 * 0.6 + bn2 * 0.4;
    let row = sin(uv.y * 80.0) * 0.5 + 0.5;
    var color = bright;
    if (bp > 0.7) { color = mix(color, yellow, (bp - 0.7) * 3.33); }
    else if (bp < 0.3) { color = mix(color, dark, (0.3 - bp) * 3.33); }
    color *= mix(0.8, 1.2, row);
    let wear = noise(uv * 8.0);
    if (wear > 0.85) { color *= 0.85; }
    let ss = 1.0 / material.seam_spacing;
    let sl = fract(uv * ss);
    let sf = min(smoothstep(0.0, 0.02, sl.x) * smoothstep(0.0, 0.02, 1.0 - sl.x), smoothstep(0.0, 0.02, sl.y) * smoothstep(0.0, 0.02, 1.0 - sl.y));
    return mix(vec3<f32>(0.1, 0.15, 0.1), color, sf);
}

fn distribution_ggx(n: vec3<f32>, h: vec3<f32>, r: f32) -> f32 { let a=r*r; let a2=a*a; let ndh=max(dot(n,h),0.0); var d=(ndh*ndh*(a2-1.0)+1.0); d=PI*d*d; return a2/d; }
fn geometry_schlick_ggx(ndv: f32, r: f32) -> f32 { let k=((r+1.0)*(r+1.0))/8.0; return ndv/(ndv*(1.0-k)+k); }
fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, r: f32) -> f32 { return geometry_schlick_ggx(max(dot(n,v),0.0),r)*geometry_schlick_ggx(max(dot(n,l),0.0),r); }

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.normal); let v = normalize(camera.position - in.world_pos);
    let albedo = turf_color(in.uv);
    let f0 = vec3<f32>(0.04);
    var radiance = vec3<f32>(0.0);
    for (var i: u32 = 0u; i < MAX_LIGHTS; i = i + 1u) {
        let light = lights[i]; if (light.intensity <= 0.0) { continue; }
        let ltf = light.position - in.world_pos; let dist = length(ltf);
        if (dist > light.range) { continue; }
        let att = max(0.0, 1.0 - pow(clamp(dist / light.range, 0.0, 1.0), 2.0));
        let l = normalize(ltf); let h = normalize(v + l);
        let ndf = distribution_ggx(n, h, material.roughness); let g = geometry_smith(n, v, l, material.roughness);
        let ks = f0; let kd = vec3<f32>(1.0) - ks;
        let spec = (ndf * g * ks) / (4.0 * max(dot(n, v), 0.0) * max(dot(n, l), 0.0) + 0.0001);
        radiance += (kd * albedo / PI * max(dot(n, l), 0.0) + spec) * light.color * light.intensity * att;
    }
    var color = radiance;
    color = color / (color + vec3<f32>(1.0));
    color = pow(color, vec3<f32>(1.0 / 2.2));
    return vec4<f32>(color, 1.0);
}
