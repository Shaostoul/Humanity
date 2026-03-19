// pbr_simple.wgsl — Simple PBR-lite shader for solid-color meshes.
// Uses separate bind groups for camera, object transform, and material.
//
// Bind groups:
//   Group 0: Camera uniforms (view_proj matrix)
//   Group 1: Object uniforms (model matrix, normal matrix)
//   Group 2: Material uniforms (base_color, metallic, roughness)

struct CameraUniforms {
    view_proj: mat4x4<f32>,
    view_pos: vec4<f32>,
};

struct ObjectUniforms {
    model: mat4x4<f32>,
    normal_matrix: mat4x4<f32>,
};

struct MaterialUniforms {
    base_color: vec4<f32>,
    // x = metallic, y = roughness, z = unused, w = unused
    params: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(1) @binding(0) var<uniform> object: ObjectUniforms;
@group(2) @binding(0) var<uniform> material: MaterialUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = object.model * vec4<f32>(vertex.position, 1.0);
    out.world_position = world_pos.xyz;
    out.clip_position = camera.view_proj * world_pos;
    // Transform normal by the normal matrix (inverse transpose of model)
    out.world_normal = normalize((object.normal_matrix * vec4<f32>(vertex.normal, 0.0)).xyz);
    out.uv = vertex.uv;
    return out;
}

// Simple directional light + ambient for PBR-lite
const LIGHT_DIR: vec3<f32> = vec3<f32>(0.3, 1.0, 0.5);
const LIGHT_COLOR: vec3<f32> = vec3<f32>(1.0, 0.98, 0.95);
const AMBIENT: vec3<f32> = vec3<f32>(0.15, 0.15, 0.2);

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(in.world_normal);
    let light_dir = normalize(LIGHT_DIR);
    let view_dir = normalize(camera.view_pos.xyz - in.world_position);
    let half_dir = normalize(light_dir + view_dir);

    let base_color = material.base_color.rgb;
    let metallic = material.params.x;
    let roughness = material.params.y;

    // Diffuse (Lambert)
    let n_dot_l = max(dot(normal, light_dir), 0.0);
    let diffuse = base_color * LIGHT_COLOR * n_dot_l * (1.0 - metallic);

    // Specular (Blinn-Phong approximation)
    let shininess = max((1.0 - roughness) * 128.0, 1.0);
    let n_dot_h = max(dot(normal, half_dir), 0.0);
    let specular = LIGHT_COLOR * pow(n_dot_h, shininess) * mix(vec3<f32>(0.04), base_color, metallic);

    // Ambient
    let ambient = base_color * AMBIENT;

    let color = ambient + diffuse + specular;

    // Simple tone mapping
    let mapped = color / (color + vec3<f32>(1.0));

    return vec4<f32>(mapped, material.base_color.a);
}
