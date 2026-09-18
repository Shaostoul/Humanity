// ── SHARED FRAGMENT PROLOGUE AND TAIL (increment P3 of the frame-cost arc,
//    docs/design/frame-cost-arc.md section 1(c), 2026-09-18) ──
//
// Until P3 the megashader had ONE colour fragment entry, fs_main, that
// carried every material's code in one function: the wall panels, the
// terrain block, the vegetation family, the atmosphere integrator, the
// cloud march and the ocean shell, dispatched on material.params.z. P1 and
// P2 switched the three heavyweight shell branches off per pipeline with
// override constants; P3 finishes the job structurally. Each material
// CLASS has its own entry point in 90-fragment-main.wgsl (fs_surface,
// fs_terrain, fs_vegetation, fs_water, fs_shell, fs_cloud), and a pipeline
// compiles ONLY the entry of the class it draws, so the backend never
// budgets registers or private storage for a path that pipeline cannot
// take. Which class a material type belongs to is `shader_class` in
// src/renderer/pipeline.rs, pinned to the type tests in the entries by
// its tests.
//
// What the six entries share lives here, as two functions:
//   frag_prologue  the per-fragment setup every entry runs FIRST and
//                  unconditionally: the instance-data hand-off, the
//                  screen-space derivatives (taken in uniform control flow,
//                  before any discard or type branch, so textureSampleGrad
//                  is legal inside the branches), the LOD-crossfade Bayer
//                  discard, the normal and view direction, and the material
//                  reads with their defaults.
//   frag_tail      the shared PBR lighting every surface class ends with:
//                  the shadowed and gated sun, the fill, the point-light
//                  loop (tiled or not), the sky-irradiance indirect term
//                  with AO, the emissives and the screen-emitter override,
//                  aerial perspective, underwater extinction and the ACES
//                  compose.
// A class entry is therefore: prologue, then only its own material blocks,
// then the tail (or an early return for the shells, which are participating
// media and never take the BRDF path).
//
// The text of both functions, and of every block in the class entries, is
// the fs_main text of v0.1315 moved VERBATIM: P3 is a code motion, gated
// by the probe rig on every capture matching the pre-split build. Keep it
// that way when editing: a lighting change goes in ONE place here and every
// class picks it up; a material change goes in its class entry.
//
// WGSL notes for anyone touching this file: `discard` and `dpdx`/`dpdy` are
// legal in a helper as long as every caller is a fragment entry (naga
// checks the call graph), and the derivatives must be taken in UNIFORM
// control flow, which is why frag_prologue is the first statement of every
// entry and is never called from inside a branch.

// Everything frag_prologue hands a class entry, and everything the entry
// hands back to frag_tail. One struct rather than a dozen parameters so a
// new per-fragment input (an AO channel, a second emissive) is one field
// added here and read in the tail, not a signature change at six sites.
// The class entries copy these into locals of the SAME names before their
// blocks and copy them back after, so the moved block text reads exactly
// as it did inside fs_main.
struct FragSetup {
    // Screen-space derivatives of the world position and the uv, valid
    // everywhere because they were taken before any branch.
    wp_dx: vec3<f32>,
    wp_dy: vec3<f32>,
    uv_dx: vec2<f32>,
    uv_dy: vec2<f32>,
    // Shading normal (a block may perturb it) and the unit fragment-to-eye
    // direction.
    normal: vec3<f32>,
    view_dir: vec3<f32>,
    // The material as read from the uniform, before the class block
    // rewrites what it needs to.
    albedo: vec3<f32>,
    metallic: f32,
    roughness: f32,
    material_type: f32,
    // Sun visibility for THIS fragment (the terrain terminator gate, the
    // canopy-card sun gain); 1 for everything else.
    sun_gate: f32,
    // Hemisphere occlusion, consumed by the indirect term only.
    ao: f32,
    // Local up for the sky-irradiance term (camera radial by default, the
    // fragment's own radial on a planet surface).
    frag_up: vec3<f32>,
    // Additive emissive from the material blocks (lava, leaf transmission,
    // the water term), independent of params.w.
    proc_emissive: vec3<f32>,
    // Type 24: the fragment is a display; its colour replaces the lit result.
    screen_emitter: bool,
    // Output alpha (the Fresnel atmosphere fallback modulates it).
    out_alpha: f32,
    // params.w as emissive strength, zeroed by the types that repurpose
    // params.w as a data channel.
    emissive_strength: f32,
}

// The setup every class entry runs first, in uniform control flow. Returns
// the defaults a fragment starts from; the entry's blocks then rewrite the
// fields their material needs. May `discard` (the LOD crossfade dither).
fn frag_prologue(in: VertexOutput) -> FragSetup {
    // Route the per-instance data to the obj_* accessors (flat varying;
    // zero for classic draws, the batched patch's translation + fade for
    // terrain-batch draws).
    g_inst_data = in.inst_data;
    // Screen-space derivatives of the world position, taken FIRST - before
    // the Bayer discard below or any non-uniform branch - so they are valid
    // wherever they are later consumed (v0.977: the ground textures rotate
    // these into the pinned domain for textureSampleGrad anisotropy).
    let wp_dx = dpdx(in.world_position);
    let wp_dy = dpdy(in.world_position);
    // Screen-space derivatives of the TEXTURE coordinate, taken here for the
    // same reason and under the same rule (v0.1089, baked bark): they are what
    // `textureSampleGrad` needs to pick a mip level, and textureSampleGrad is
    // the LOD-selecting sample that is legal inside non-uniform control flow -
    // which every material-type branch below is. Meaningless for the material
    // types whose uv carries a packed integer; those never read it.
    let uv_dx = dpdx(in.uv);
    let uv_dy = dpdy(in.uv);
    // LOD crossfade (v0.920): model[0].w carries the per-object fade (see
    // RenderObject::fade). 0 = normal. Positive f = fading IN: keep pixels
    // whose 4x4 Bayer threshold is below f. Negative -f = fading OUT: keep
    // pixels at/above f. A rising patch at t and its falling partner at -t
    // partition the screen per-pixel, so terrain LOD swaps dissolve instead
    // of popping - with opaque depth intact and zero overdraw holes.
    let lod_fade = obj_lod_fade();
    if (lod_fade != 0.0) {
        let px = vec2<u32>(u32(in.clip_position.x), u32(in.clip_position.y));
        // 4x4 Bayer matrix via bit interleaving: thresholds (0.5..15.5)/16.
        let bx = px.x % 4u;
        let by = px.y % 4u;
        let bayer_i = (bx % 2u) * 8u + (by % 2u) * 4u + ((bx / 2u) % 2u) * 2u + (by / 2u) % 2u;
        let b = (f32(bayer_i) + 0.5) / 16.0;
        if (lod_fade > 0.0) {
            if (b >= lod_fade) { discard; }
        } else {
            if (b < -lod_fade) { discard; }
        }
    }
    // var (not let) since v0.907: the ground PBR pass perturbs the terrain
    // normal with the material's normal map before the lighting below.
    var normal = normalize(in.world_normal);
    let view_dir = normalize(camera.view_pos.xyz - in.world_position);

    var albedo = material.base_color.rgb;
    var metallic = material.params.x;
    var roughness = material.params.y;
    let material_type = material.params.z;
    // Sun visibility for THIS fragment, 1 unless a branch knows better. Only
    // the planet-surface branch sets it (v0.1052 terminator gate); everything
    // else - ship interiors, props, the other bodies - is unaffected.
    var sun_gate = 1.0;
    // ── AMBIENT OCCLUSION, OFF ALBEDO (v0.1104) ──
    // Two branches used to fold their cavity occlusion straight into albedo
    // (ground detail, baked bark), both with a comment admitting why: "the
    // shared PBR tail has no AO input on this path". It has one now. Darkening
    // albedo attenuates the SUN exactly as much as it attenuates the sky,
    // which no occlusion term should ever do - a crevice is shielded from the
    // hemisphere, not from a collimated beam it happens to face. This is
    // consumed by the indirect term at the bottom of fs_main and by nothing
    // else. (SSAO into the same slot is a separate job: it needs the SSAO
    // texture bound into group 3, i.e. three create_bind_group sites.)
    var ao = 1.0;
    // Local up for this fragment, for the sky-irradiance term. Default is the
    // camera's radial up (light3_cone_inner.yzw, published every frame by
    // lib.rs) which is right for anything within a few km of the camera -
    // trees, props, grass, machines. The planet-surface branch overwrites it
    // per fragment, because that one pass draws the whole disc from orbit and
    // a single camera-relative up would be wrong across most of it.
    var frag_up = vec3<f32>(
        camera.light3_cone_inner.y,
        camera.light3_cone_inner.z,
        camera.light3_cone_inner.w,
    );
    var proc_emissive = vec3<f32>(0.0); // extra emissive from procedural materials (e.g. lava cracks)
    // Type 24 (in-world screen) sets this: the fragment is a DISPLAY and its
    // colour is the sampled page times brightness, replacing the lit result
    // at the compose step below (a display emits, it does not reflect).
    var screen_emitter = false;
    var out_alpha = material.base_color.a; // types below may modulate (atmosphere fresnel)
    // Emissive strength normally rides in params.w -- but material type 12
    // REPURPOSES params.w as the "albedo texture present" flag (v0.811), so
    // the type-12 branch zeroes this to keep planets from self-glowing.
    var emissive_strength = material.params.w;

    // Hand everything above to the class entry as one value.
    var s: FragSetup;
    s.wp_dx = wp_dx;
    s.wp_dy = wp_dy;
    s.uv_dx = uv_dx;
    s.uv_dy = uv_dy;
    s.normal = normal;
    s.view_dir = view_dir;
    s.albedo = albedo;
    s.metallic = metallic;
    s.roughness = roughness;
    s.material_type = material_type;
    s.sun_gate = sun_gate;
    s.ao = ao;
    s.frag_up = frag_up;
    s.proc_emissive = proc_emissive;
    s.screen_emitter = screen_emitter;
    s.out_alpha = out_alpha;
    s.emissive_strength = emissive_strength;
    return s;
}

// The shared PBR lighting tail every surface class ends with. Reads the
// setup as the class entry left it and returns the final tone-mapped colour
// with the output alpha. The locals below carry the fs_main names so the
// moved text is verbatim.
fn frag_tail(in: VertexOutput, s: FragSetup) -> vec4<f32> {
    let normal = s.normal;
    let view_dir = s.view_dir;
    let albedo = s.albedo;
    let metallic = s.metallic;
    let roughness = s.roughness;
    let material_type = s.material_type;
    let sun_gate = s.sun_gate;
    let ao = s.ao;
    let frag_up = s.frag_up;
    let proc_emissive = s.proc_emissive;
    let screen_emitter = s.screen_emitter;
    let out_alpha = s.out_alpha;
    let emissive_strength = s.emissive_strength;

    // Fresnel reflectance at normal incidence
    // Dielectrics: 0.04, metals: tinted by albedo
    let f0 = mix(vec3<f32>(0.04), albedo, metallic);

    // Evaluate main directional light (from camera uniforms), attenuated
    // by the sun shadow map (v0.899). Only the SUN term is shadowed; fill
    // and ambient stay, so shadows read as shade, not holes.
    // ── TERRAIN TERMINATOR GATE (v0.1052) ──
    // Operator: "some weird lighting at night in the desert... we've had this
    // lighting bug before." They are right that it recurred, and this is why.
    //
    // The celestial pass (which draws planet terrain) stamps a HARDCODED white
    // sun at intensity 2.5 over the camera uniform - unchanged since v0.451 -
    // so the atmosphere-corrected night sun colour that lib.rs computes never
    // reaches the ground. On top of that, the terrain sun term is
    // dot(MESH normal, sun_dir) with no local-horizon test, and the sand normal
    // map tilts that normal by tens of degrees. So after sunset the flat desert
    // correctly falls to ambient, while the band toward the sunset azimuth -
    // where grazing geometry and normal-map facets present the most surfaces
    // tilted at a sun that is BELOW THE HORIZON - still catches ~25x more light
    // than anything else in frame. That is the bright streak.
    //
    // Every other surface in that pass already has this gate: water and foam
    // test dot(RADIAL normal, sun), and the cloud march tests each sample's own
    // sphere normal. Terrain never got one. The window keeps a small negative
    // tail so genuine alpenglow and mountain-top light survive - a fragment
    // above the local sphere really does see the sun a little past geometric
    // sunset - while ruling out light from a sun a degree or more under.
    let sun_ndl = dot(normal, normalize(camera.sun_direction.xyz));
    var lo = evaluate_light(
        camera.sun_direction.xyz, camera.sun_color.rgb, camera.sun_direction.w,
        normal, view_dir, albedo, metallic, roughness, f0)
        * sun_shadow_offset(in.world_position, sun_ndl, normal)
        * sun_gate;

    // Evaluate fill light (from camera uniforms). NIGHT GATE for planet
    // surfaces (operator, v0.1186: "the oceans are glowing" on the dark
    // side from orbit): the fill is a fixed cool light with no relation
    // to the sun, so on a planet's night side it fabricated illumination
    // - invisible on dark land albedo, glowing cyan on the bright
    // bathymetry ocean. Type 12 (textured planet) scales the fill by the
    // sun's local elevation with a small twilight tail; every other
    // material keeps the unconditional fill (it exists for near-field
    // readability, not planetary lighting).
    var fill_gate = 1.0;
    if (material_type >= 11.5 && material_type < 12.5) {
        fill_gate = smoothstep(-0.08, 0.12, sun_ndl);
    }
    lo = lo + evaluate_light(
        camera.fill_direction.xyz, camera.fill_color.rgb, camera.fill_direction.w,
        normal, view_dir, albedo, metallic, roughness, f0) * fill_gate;

    // Point + spot lights, UNCAPPED (v0.782): the storage buffer holds every
    // scene light; light_count bounds the loop. The early range/attenuation
    // rejection keeps far lights nearly free, so the practical ceiling is GPU
    // fill cost, not a software cap.
    let num_lights = i32(camera.light_count.x);
    // Clustering L1b: when tiling is on, loop ONLY this fragment's tile
    // list (bounded by local overlap, not the global count - what lifts
    // the 256 cap to 2048). The light body below is untouched: only the
    // index it evaluates comes from the tile list.
    let tile_w_px = shadow_u.params2.z;
    let use_tiles = tile_w_px > 0.5;
    var tile_base = 0u;
    var loop_n = num_lights;
    if (use_tiles) {
        let tx = min(u32(in.clip_position.x / tile_w_px), TILE_COLS - 1u);
        let ty = min(u32(in.clip_position.y / shadow_u.params2.w), TILE_ROWS - 1u);
        let tile = ty * TILE_COLS + tx;
        tile_base = tile * TILE_CAP;
        loop_n = i32(min(tile_counts[tile], TILE_CAP));
    }
    for (var j = 0; j < loop_n; j = j + 1) {
        var i = j;
        if (use_tiles) {
            i = i32(tile_indices[tile_base + u32(j)]);
            // Respect THIS pass's declared light count (v0.1155, the
            // tiled-only night glow): the tile lists are built once per
            // frame from the lit interior pass, but the celestial/terrain
            // pass writes its camera uniform without lit_uniform, so its
            // light_count is 0 - the classic loop gives terrain NO point
            // lights, and the tiled path must not smuggle them in through
            // the tile lists. Without this guard, interior lights lit the
            // whole night terrain whenever tiling was on.
            if (i >= num_lights) {
                continue;
            }
        }
        var light_pos = scene_lights[i].pos_intensity.xyz;
        let intensity = scene_lights[i].pos_intensity.w;
        let light_color = scene_lights[i].color_range.xyz;
        let radius = scene_lights[i].color_range.w;
        let sent = scene_lights[i].spot.w;

        // LINE light (v0.786, sentinel cos_outer == -2.0): the whole segment
        // [pos, spot.xyz] emits -- light each fragment from the CLOSEST point
        // on the segment (capsule-light representative point), so a strip
        // washes the full wall instead of pooling at one point. Rust mirror +
        // tests: light::line_light_closest_point.
        if (sent < -1.5) {
            let a = light_pos;
            let b = scene_lights[i].spot.xyz;
            let ab = b - a;
            let t = clamp(dot(in.world_position - a, ab) / max(dot(ab, ab), 1e-6), 0.0, 1.0);
            light_pos = a + ab * t;
        }

        let to_light = light_pos - in.world_position;
        let dist = length(to_light);

        // Cheap reject: outside the light's range, contribution is exactly 0
        // (the linear range window below hits zero at dist == radius).
        if (dist >= radius) { continue; }

        let light_dir = to_light / max(dist, 0.001);

        // Attenuation: inverse square with radius falloff
        var attenuation = intensity / (1.0 + dist * dist) * max(1.0 - dist / max(radius, 0.001), 0.0);

        // Spot cone (v0.639): cos_outer == -1.0 is the Point/Bar sentinel, so this only narrows
        // an actual spot light -- zero extra cost/behavior change for every other light.
        let spot = scene_lights[i].spot;
        let cos_outer = spot.w;
        if (cos_outer > -1.0) {
            let cos_inner = scene_lights[i].cone_inner.x;
            // spot.xyz is the aim direction in the light-to-fragment sense; -light_dir (which
            // points fragment-to-light) flips to the same sense for the dot product.
            let cos_angle = dot(normalize(spot.xyz), -light_dir);
            attenuation = attenuation * smoothstep(cos_outer, cos_inner, cos_angle);
        }

        if (attenuation > 0.001) {
            lo = lo + evaluate_light(light_dir, light_color, attenuation, normal, view_dir, albedo, metallic, roughness, f0);
        }
    }

    // ── INDIRECT LIGHT (v0.1104) ──
    // Real sky irradiance from the per-frame sky-view table (see sky_ambient
    // at the top of this file), floored at the old constant so ship interiors
    // and deep space - where there is no sky and the pads that carry it are
    // zeroed - keep exactly the silhouette floor they had. With sky = 0 this
    // expression is bit-identical to the pre-v0.1104 line it replaces; the
    // Rust twin renderer::sky_ambient asserts that.
    //
    // AO multiplies ONLY this term. It is occlusion of the HEMISPHERE.
    let ambient = albedo * max(sky_ambient(normal, frag_up), AMBIENT_FLOOR) * ao;

    var color = ambient + lo;

    // Emissive: params.w controls emissive strength (0 = none, 1+ = glow)
    // Emissive objects use base_color as their glow color, bypassing lighting.
    // (Declared as a var at the top; type 12 zeroes it -- see there.)
    if (emissive_strength > 0.0) {
        color = color + albedo * emissive_strength;
    }

    // Procedural emissive (e.g. lava cracks) -- additive, independent of params.w
    color = color + proc_emissive;

    // Type 24 (in-world screen): the display's own light REPLACES the lit
    // result. Sun, shadow map, sky ambient and room lights above are all
    // discarded for this fragment; a screen showing a dark page reads dark
    // and a bright page reads bright regardless of where the sun is. Aerial
    // haze and underwater extinction below still apply, so a far screen
    // fades into the atmosphere like everything else in the scene.
    if (screen_emitter) {
        color = albedo * max(emissive_strength, 0.0);
    }

    // ── Aerial perspective (v0.916, research roadmap item 2) ──
    // Distant surfaces fade toward the sky's in-scatter color - the single
    // strongest landscape realism cue. Exponential height haze: the CPU
    // pokes sigma (already folded with the camera-altitude density falloff
    // and the Settings strength) into light1_cone_inner.y, the slant cap
    // scale into light1_cone_inner.z, the day/sunset-tinted sky color into [2].yzw, and
    // the camera's radial up into [3].yzw. The SLANT path bound keeps a
    // noon sun and orbit views clear: looking up exits the haze layer in a
    // few km, so only long, flat sightlines accumulate fog. sigma = 0 (off
    // in space, at night the color also darkens) makes this a no-op.
    color = aerial_apply(color, in.world_position);
    // Underwater extinction AFTER aerial haze: above water the aerial term is
    // the atmosphere, below it the water column is what attenuates, and the two
    // are mutually exclusive in practice (aerial sigma is a surface-air value).
    // `false`: everything that reaches the shared tail - terrain, seabed,
    // props, vegetation - is geometry seen THROUGH the water column when it
    // sits below sea level, not the interface itself.
    color = underwater_apply(color, in.world_position, false);

    // ACES-like tone mapping (more filmic than Reinhard)
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    color = clamp((color * (a * color + vec3<f32>(b))) / (color * (c * color + vec3<f32>(d)) + vec3<f32>(e)), vec3<f32>(0.0), vec3<f32>(1.0));

    return vec4<f32>(color, out_alpha);
}
