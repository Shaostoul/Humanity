// Star halo pass (2026-07-11) - soft photographic halos on the brightest
// catalog stars, the final layer of the sky stack (glow texture ->
// constellation lines -> packed star points -> THIS). Reference look: a
// long-exposure Milky Way photograph, where the famous stars bloom into a
// gentle gaussian glow a few pixels wide with a very faint 4-point
// diffraction cross, instead of staying bare 1-2 px points.
//
// Geometry: one camera-facing quad per selected star (two triangles, six
// vertices, no index buffer). Each vertex carries the star's unit direction
// plus a corner coordinate in {-1,+1}^2; the VERTEX shader expands the
// corner in the tangent plane of the celestial sphere at that direction.
// Because the star camera is rotation-only and sits at the sphere's center,
// the tangent plane at the star IS the plane perpendicular to the view ray
// toward it, so a tangent-plane quad is exactly camera-facing with no
// per-frame CPU billboarding. The tangent frame is derived from the star
// direction alone, which also FIXES the diffraction cross to the celestial
// sphere: the spikes do not spin when the player rolls the camera (a real
// camera's spider vanes would; stability reads better in free look - taste,
// not physics).
//
// ── THE TANGENT-FRAME + FALLOFF CONTRACT ──
// The tangent-frame construction in vs_main and the gaussian+cross falloff
// in fs_main are each mirrored in TWO other places and must stay identical:
//   1. THIS file,
//   2. the embedded FALLBACK_HALO_SHADER copy in src/renderer/stars.rs,
//   3. the pure Rust ports halo_tangent_frame / halo_falloff in
//      src/renderer/stars.rs (what the unit tests lock).
//
// Intensity discipline: the pass blends ADDITIVELY (ONE, ONE) over the sRGB
// swapchain, exactly like the Milky Way glow pass. The per-star `amplitude`
// vertex attribute is computed CPU-side in LINEAR light (peak ~0.3 at
// Sirius, falling with magnitude - see halo_amplitude in stars.rs); linear
// values are what an additive blend into an sRGB-encoded target wants (the
// hardware re-applies the display transform on write), so unlike the glow
// texture's display-referred texels there is NO pow(2.2) here - the v0.802
// lesson applied at the constant-definition site instead of the shader.
// The star's own point (drawn just before this pass) stays visibly on top
// because the halo peak never reaches the point's full brightness.

// Must match the Rust-side CameraUniforms struct exactly (672 bytes, v0.639).
struct CameraUniforms {
    view_proj: mat4x4<f32>,
    view_pos: vec4<f32>,
    // Point lights (unused by halos, but must match the buffer layout)
    light0: vec4<f32>, light1: vec4<f32>, light2: vec4<f32>, light3: vec4<f32>,
    light4: vec4<f32>, light5: vec4<f32>, light6: vec4<f32>, light7: vec4<f32>,
    light0_color: vec4<f32>, light1_color: vec4<f32>, light2_color: vec4<f32>, light3_color: vec4<f32>,
    light4_color: vec4<f32>, light5_color: vec4<f32>, light6_color: vec4<f32>, light7_color: vec4<f32>,
    light0_spot: vec4<f32>, light1_spot: vec4<f32>, light2_spot: vec4<f32>, light3_spot: vec4<f32>,
    light4_spot: vec4<f32>, light5_spot: vec4<f32>, light6_spot: vec4<f32>, light7_spot: vec4<f32>,
    light0_cone_inner: vec4<f32>, light1_cone_inner: vec4<f32>, light2_cone_inner: vec4<f32>, light3_cone_inner: vec4<f32>,
    light4_cone_inner: vec4<f32>, light5_cone_inner: vec4<f32>, light6_cone_inner: vec4<f32>, light7_cone_inner: vec4<f32>,
    light_count: vec4<f32>,
    sun_direction: vec4<f32>,
    sun_color: vec4<f32>,
    fill_direction: vec4<f32>,
    fill_color: vec4<f32>,
};
@group(0) @binding(0)
var<uniform> camera: CameraUniforms;

struct HaloInput {
    // Unit star direction on the celestial sphere (f32x3 - only ~128 quads,
    // so unlike the 25M-star point pipeline there is nothing to pack).
    @location(0) direction: vec3<f32>,
    // Quad corner in {-1,+1}^2; doubles as the falloff coordinate.
    @location(1) corner: vec2<f32>,
    // Linear star RGB from ci_to_rgb - the SAME raw values the star point
    // uses, so the halo tint always matches its star.
    @location(2) color: vec3<f32>,
    // Peak linear additive contribution at the quad center (<= ~0.3).
    @location(3) amplitude: f32,
    // tan(half-angle) of the quad's angular half-extent: tangent-plane
    // units per corner unit.
    @location(4) half_extent: f32,
};

struct HaloOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) corner: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) amplitude: f32,
};

@vertex
fn vs_main(input: HaloInput) -> HaloOutput {
    var out: HaloOutput;
    let dir = normalize(input.direction);
    // Tangent frame on the celestial sphere (THE CONTRACT - keep identical
    // to halo_tangent_frame in stars.rs). The pole fallback keeps the cross
    // product well-conditioned for stars within ~8 degrees of the celestial
    // poles (Polaris!).
    var up = vec3<f32>(0.0, 0.0, 1.0);
    if (abs(dir.z) > 0.99) {
        up = vec3<f32>(1.0, 0.0, 0.0);
    }
    let t1 = normalize(cross(up, dir));
    let t2 = cross(dir, t1); // unit by construction: dir and t1 are unit + orthogonal
    // Flat tangent-plane quad, then out to the star radius (same 5000 as
    // the points; the pass is depthless so only the direction matters).
    let offset = (input.corner.x * t1 + input.corner.y * t2) * input.half_extent;
    let world_pos = (dir + offset) * 5000.0;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.corner = input.corner;
    out.color = input.color;
    out.amplitude = input.amplitude;
    return out;
}

@fragment
fn fs_main(input: HaloOutput) -> @location(0) vec4<f32> {
    // THE FALLOFF CONTRACT - keep identical to halo_falloff in stars.rs.
    // Shape = normalized gaussian disk + a faint 4-point diffraction cross,
    // both forced to EXACTLY zero at the quad's inscribed circle (r = 1) so
    // no square edge can ever show against black space:
    //   g     edge-subtracted gaussian: exp(-k r^2) shifted down by its own
    //         r = 1 value and renormalized, so g(0) = 1 and g(1) = 0.
    //   sx/sy the two perpendicular streaks: narrow across the axis
    //         (sigma^2 = 1/120), long along it (sigma^2 = 1/5).
    //   w     hard window (1 - r^2): zeroes the streaks at the edge; the
    //         corners (r^2 up to 2) clamp to 0 through both terms.
    // The sum is divided by its own center value (1 + 2 * cross amplitude)
    // so the vertex amplitude IS the true peak - the intensity-discipline
    // numbers stay honest.
    let c = input.corner;
    let r2 = dot(c, c);
    let g = max(exp(-5.5 * r2) - exp(-5.5), 0.0) / (1.0 - exp(-5.5));
    let sx = exp(-c.y * c.y * 60.0) * exp(-c.x * c.x * 2.5);
    let sy = exp(-c.x * c.x * 60.0) * exp(-c.y * c.y * 2.5);
    let w = clamp(1.0 - r2, 0.0, 1.0);
    let shape = (g + 0.15 * (sx + sy) * w) / 1.3;
    let intensity = input.amplitude * shape;
    // Additive blend (ONE, ONE): output = halo + whatever is behind.
    return vec4<f32>(input.color * intensity, 1.0);
}
