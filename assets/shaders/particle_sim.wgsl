// particle_sim.wgsl - GPU particle simulation (v0.1068).
//
// The CPU path (renderer::particles) was measured in v0.1066/v0.1067 and its
// ceiling is memory bandwidth: ~17-20 ns per particle per frame to advance the
// pool and build vertex data, streaming the whole pool through cache twice.
// Shrinking the struct and adding rayon got 1.5x; past that the only way up is
// to stop moving particles across the bus at all.
//
// So here the pool LIVES on the GPU. The CPU submits one dispatch and never
// touches a particle. Two buffers:
//   state  - storage, read-write: position, velocity, age, lifetime (32 B)
//   verts  - storage AND vertex: exactly the layout the existing particle
//            vertex shader already reads
//
// That second point is the whole trick. The compute pass writes the SAME
// ParticleVertexData the CPU used to upload, into a buffer flagged
// VERTEX | STORAGE, so the draw path is completely unchanged - same pipeline,
// same instanced quad, same fragment shader with its procedural snowflake. It
// also dodges the failure that took three releases in v0.782: a vertex shader
// reading a storage buffer needs vertex-stage storage support, and this design
// never asks for it.
//
// RECYCLING instead of spawn/free. A dead slot immediately respawns inside the
// emitter volume rather than being compacted away. For weather that is exactly
// right - steady rain is a pool in equilibrium, not a queue - and it means no
// free list, no compaction, no atomic counter, and a dispatch whose size is
// constant from frame to frame.

struct SimParams {
    // xyz = emitter origin (world), w = spawn radius
    origin_radius: vec4<f32>,
    // xyz = base fall direction (unit), w = spread in radians
    dir_spread: vec4<f32>,
    // xyz = gravity, w = dt
    gravity_dt: vec4<f32>,
    // x = speed_min, y = speed_max, z = life_min, w = life_max
    speed_life: vec4<f32>,
    // x = size_start, y = size_end, z = emissive, w = shape
    size_shape: vec4<f32>,
    color_start: vec4<f32>,
    color_end: vec4<f32>,
    // x = live particle count, y = frame counter (RNG salt),
    // z = streak seconds, w = unused
    count_frame_streak: vec4<f32>,
};

struct ParticleState {
    position: vec3<f32>,
    age: f32,
    velocity: vec3<f32>,
    lifetime: f32,
};

// The draw side reads renderer::particles::ParticleVertexData: TIGHTLY packed
// 52-byte records (pos 0, color 12, size_emissive 28, stretch 40). A WGSL
// struct CANNOT mirror that: vec3 in a storage buffer aligns to 16, so the
// struct version of this silently wrote 64-byte records and every instance
// after the first read shifted garbage (BUG-050: giant spheres, cyan snow).
// So the verts buffer is a raw float array and we pack all 13 floats by hand.
const VERT_FLOATS: u32 = 13u;

@group(0) @binding(0) var<uniform> params: SimParams;
@group(0) @binding(1) var<storage, read_write> state: array<ParticleState>;
@group(0) @binding(2) var<storage, read_write> verts: array<f32>;

// Integer hash -> uniform [0,1). Same shape as the CPU xorshift so the two
// paths produce statistically identical seas of particles, not identical
// sequences (which would be pointless to guarantee across architectures).
fn hash11(x: u32) -> f32 {
    var h = x;
    h = h ^ (h >> 16u);
    h = h * 0x7feb352du;
    h = h ^ (h >> 15u);
    h = h * 0x846ca68bu;
    h = h ^ (h >> 16u);
    return f32(h >> 8u) / 16777216.0;
}

fn rand3(seed: u32) -> vec3<f32> {
    return vec3<f32>(
        hash11(seed * 3u + 0u),
        hash11(seed * 3u + 1u),
        hash11(seed * 3u + 2u),
    );
}

/// Place a fresh particle inside the emitter volume with a fresh velocity.
/// Called both for genuinely new slots and for recycled dead ones.
fn respawn(idx: u32, salt: u32) -> ParticleState {
    let seed = idx * 9781u + salt * 6151u;
    let r = rand3(seed);
    let jitter = (r * 2.0 - vec3<f32>(1.0)) * params.origin_radius.w;
    let dir = normalize(params.dir_spread.xyz);
    // Cone spread about the fall direction: build any two perpendicular axes,
    // then tilt by a random angle within the spread.
    var up = vec3<f32>(0.0, 1.0, 0.0);
    if (abs(dir.y) > 0.95) {
        up = vec3<f32>(1.0, 0.0, 0.0);
    }
    let tx = normalize(cross(up, dir));
    let ty = cross(dir, tx);
    let a = hash11(seed + 77u) * 6.2831853;
    let t = hash11(seed + 131u) * params.dir_spread.w;
    let spread_dir = normalize(dir + (tx * cos(a) + ty * sin(a)) * tan(t));
    let speed = mix(params.speed_life.x, params.speed_life.y, hash11(seed + 211u));
    let life = mix(params.speed_life.z, params.speed_life.w, hash11(seed + 307u));
    var s: ParticleState;
    s.position = params.origin_radius.xyz + jitter;
    s.velocity = spread_dir * speed;
    // Stagger the initial age across the pool so the first frame does not
    // create one cohort that then dies together - the v0.1064 pulsing lesson,
    // which applies with far more force here because the whole pool is seeded
    // in a single dispatch.
    s.age = hash11(seed + 419u) * life;
    s.lifetime = life;
    return s;
}

@compute @workgroup_size(256)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    let count = u32(params.count_frame_streak.x);
    if (idx >= count) {
        return;
    }
    let salt = u32(params.count_frame_streak.y);
    let dt = params.gravity_dt.w;

    var s = state[idx];
    // A zero lifetime marks a slot that has never been initialised, which is
    // how the pool bootstraps without a separate seeding pass.
    if (s.lifetime <= 0.0) {
        s = respawn(idx, salt);
    } else {
        s.age = s.age + dt;
        if (s.age >= s.lifetime) {
            s = respawn(idx, salt);
        } else {
            s.velocity = s.velocity + params.gravity_dt.xyz * dt;
            s.position = s.position + s.velocity * dt;
        }
    }
    state[idx] = s;

    // Build the draw vertex in the same pass - the data is already in
    // registers, so this is nearly free and saves a second dispatch.
    let t = clamp(s.age / max(s.lifetime, 1.0e-4), 0.0, 1.0);
    let col = mix(params.color_start, params.color_end, t);
    let stretch = s.velocity * params.count_frame_streak.z;
    let base = idx * VERT_FLOATS;
    verts[base + 0u] = s.position.x;
    verts[base + 1u] = s.position.y;
    verts[base + 2u] = s.position.z;
    verts[base + 3u] = col.x;
    verts[base + 4u] = col.y;
    verts[base + 5u] = col.z;
    verts[base + 6u] = col.w;
    verts[base + 7u] = mix(params.size_shape.x, params.size_shape.y, t);
    verts[base + 8u] = params.size_shape.z;
    verts[base + 9u] = params.size_shape.w;
    verts[base + 10u] = stretch.x;
    verts[base + 11u] = stretch.y;
    verts[base + 12u] = stretch.z;
}
