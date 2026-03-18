// hexagon_marble.wgsl — Procedural hexagonal marble tile with veining pattern.
// White marble base with gray veining, hexagonal tile layout with grout lines,
// and per-hexagon color variation. Slightly reflective polished surface.
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
struct Material { base_color: vec3<f32>, metallic: f32, roughness: f32, ao: f32, hex_scale: f32, vein_color_r: f32, };
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

fn hex_distance(uv: vec2<f32>) -> vec3<f32> {
    let s = vec2<f32>(1.0, 1.732);
    var p = abs(uv);
    let a = dot(p, normalize(s)) * 0.5;
    p -= vec2<f32>(clamp(a, 0.0, 0.5 * s.x), 0.5 * s.y);
    let dist = length(p);
    let id = vec2<f32>(floor((uv.x + uv.y * 0.57735) / 1.5), floor(uv.y / 0.8660254));
    return vec3<f32>(dist, id);
}

fn marble_color(uv: vec2<f32>) -> vec3<f32> {
    let hex_uv = uv * material.hex_scale;
    let hi = hex_distance(hex_uv);
    let hd = hi.x; let hid = hi.yz;
    let vein_color = vec3<f32>(material.vein_color_r, material.vein_color_r, material.vein_color_r + 0.05);
    let grout_color = vec3<f32>(0.4, 0.4, 0.45);
    let vn1 = noise(hex_uv * 4.0 + hid * 5.7);
    let vn2 = noise(hex_uv * 8.0 + hid * 13.1);
    let vp = abs(sin(vn1 * 6.28 + vn2 * 2.0));
    var color = material.base_color + hash(hid) * 0.1 - 0.05;
    if (vp < 0.3) { color = mix(color, vein_color, (1.0 - vp / 0.3) * 0.6); }
    let hb = 1.0 - smoothstep(0.45, 0.5, hd);
    return mix(color, grout_color, hb);
}

fn distribution_ggx(n: vec3<f32>, h: vec3<f32>, r: f32) -> f32 { let a=r*r; let a2=a*a; let ndh=max(dot(n,h),0.0); var d=(ndh*ndh*(a2-1.0)+1.0); d=PI*d*d; return a2/d; }
fn geometry_schlick_ggx(ndv: f32, r: f32) -> f32 { let k=((r+1.0)*(r+1.0))/8.0; return ndv/(ndv*(1.0-k)+k); }
fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, r: f32) -> f32 { return geometry_schlick_ggx(max(dot(n,v),0.0),r)*geometry_schlick_ggx(max(dot(n,l),0.0),r); }

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.normal); let v = normalize(camera.position - in.world_pos);
    let albedo = marble_color(in.uv);
    let f0 = mix(vec3<f32>(0.04), albedo, material.metallic);
    var radiance = vec3<f32>(0.0);
    for (var i: u32 = 0u; i < MAX_LIGHTS; i = i + 1u) {
        let light = lights[i]; if (light.intensity <= 0.0) { continue; }
        let ltf = light.position - in.world_pos; let dist = length(ltf);
        if (dist > light.range) { continue; }
        let att = max(0.0, 1.0 - pow(clamp(dist / light.range, 0.0, 1.0), 2.0));
        let l = normalize(ltf); let h = normalize(v + l);
        let ndf = distribution_ggx(n, h, material.roughness); let g = geometry_smith(n, v, l, material.roughness);
        let ks = f0; let kd = (vec3<f32>(1.0) - ks) * (1.0 - material.metallic);
        let spec = (ndf * g * ks) / (4.0 * max(dot(n, v), 0.0) * max(dot(n, l), 0.0) + 0.0001);
        radiance += (kd * albedo / PI * max(dot(n, l), 0.0) + spec) * light.color * light.intensity * att;
    }
    var color = albedo * 0.15 + radiance;
    color = color / (color + vec3<f32>(1.0));
    color = pow(color, vec3<f32>(1.0 / 2.2));
    return vec4<f32>(color, 1.0);
}
