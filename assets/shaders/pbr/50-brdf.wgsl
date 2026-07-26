// ── Cook-Torrance BRDF Evaluation ──

fn evaluate_light(
    light_dir: vec3<f32>,
    light_color: vec3<f32>,
    light_intensity: f32,
    normal: vec3<f32>,
    view_dir: vec3<f32>,
    albedo: vec3<f32>,
    metallic: f32,
    roughness: f32,
    f0: vec3<f32>,
) -> vec3<f32> {
    let l = normalize(light_dir);
    let h = normalize(view_dir + l);

    let n_dot_l = max(dot(normal, l), 0.0);
    let n_dot_v = max(dot(normal, view_dir), 0.001);
    let n_dot_h = max(dot(normal, h), 0.0);
    let h_dot_v = max(dot(h, view_dir), 0.0);

    // Cook-Torrance specular BRDF
    let ndf = distribution_ggx(n_dot_h, roughness);
    let geo = geometry_smith(n_dot_v, n_dot_l, roughness);
    let fresnel = fresnel_schlick(h_dot_v, f0);

    let numerator = ndf * geo * fresnel;
    let denominator = 4.0 * n_dot_v * n_dot_l + 0.0001;
    let specular = numerator / denominator;

    // Energy conservation: diffuse is reduced by specular reflection
    let ks = fresnel;
    var kd = vec3<f32>(1.0) - ks;
    kd = kd * (1.0 - metallic); // Metals have no diffuse

    let diffuse = kd * albedo / PI;

    return (diffuse + specular) * light_color * light_intensity * n_dot_l;
}

