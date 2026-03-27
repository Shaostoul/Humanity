// Procedural Woven Fabric Shader
//
// Algorithm: Models interlocking warp (vertical) and weft (horizontal) threads
// on a regular grid. Three weave patterns are supported:
//   0 = Plain weave: alternating over/under every thread
//   1 = Twill weave: diagonal pattern shifting one thread per row
//   2 = Satin weave: long floats with widely spaced interlacings
//
// Each thread crossing is evaluated to determine which thread (warp or weft)
// is on top. Thread shape is modeled as a rounded rectangle within each cell.
// The slight curvature where threads bend over each other creates the normal
// map. Thread color comes from the warp_color or weft_color parameter.
//
// Outputs PBR channels: base_color, roughness, metallic, normal.

// ── Parameters ──
struct FabricParams {
    weave_type: i32,         // 0 = plain, 1 = twill, 2 = satin
    thread_size: f32,        // thread width in UV units (e.g. 0.01)
    warp_color: vec3<f32>,   // vertical thread color
    weft_color: vec3<f32>,   // horizontal thread color
}

// ── Utility functions ──

fn hash(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash(i), hash(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash(i + vec2<f32>(0.0, 1.0)), hash(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y
    );
}

fn fbm(p: vec2<f32>, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var pos = p;
    for (var i = 0; i < octaves; i = i + 1) {
        value = value + amplitude * noise(pos);
        pos = pos * 2.0;
        amplitude = amplitude * 0.5;
    }
    return value;
}

// ── Main material function ──

struct MaterialOutput {
    base_color: vec3<f32>,
    roughness: f32,
    metallic: f32,
    normal: vec3<f32>,
}

// Determine if warp is on top at grid position (col, row) based on weave pattern
fn warp_on_top(col: i32, row: i32, weave_type: i32) -> bool {
    if weave_type == 0 {
        // Plain weave: checkerboard
        return ((col + row) & 1) == 0;
    } else if weave_type == 1 {
        // Twill weave: diagonal (2/1 twill)
        return ((col - row) % 3 + 3) % 3 != 0;
    } else {
        // Satin weave: 5-harness satin (long floats)
        let shift = (row * 2) % 5;
        return (col % 5) != shift;
    }
}

fn fabric_material(uv: vec2<f32>, params: FabricParams) -> MaterialOutput {
    var out: MaterialOutput;

    let cell_size = params.thread_size * 2.0; // each cell has one warp + one weft
    let grid_uv = uv / cell_size;
    let cell = floor(grid_uv);
    let local = fract(grid_uv);

    let col = i32(cell.x);
    let row = i32(cell.y);
    let is_warp_top = warp_on_top(col, row, params.weave_type);

    // Thread shape: rounded cross-section within cell
    // Warp thread runs vertically (centered on x = 0.5 of cell)
    // Weft thread runs horizontally (centered on y = 0.5 of cell)
    let warp_dist = abs(local.x - 0.5); // distance from warp thread center
    let weft_dist = abs(local.y - 0.5); // distance from weft thread center

    let thread_half = 0.4; // thread fills 80% of half-cell
    let warp_mask = smoothstep(thread_half, thread_half - 0.08, warp_dist);
    let weft_mask = smoothstep(thread_half, thread_half - 0.08, weft_dist);

    // Per-thread subtle color variation
    let warp_var = hash(vec2<f32>(f32(col), 0.0)) * 0.06 - 0.03;
    let weft_var = hash(vec2<f32>(0.0, f32(row))) * 0.06 - 0.03;
    let warp_col = params.warp_color + vec3<f32>(warp_var);
    let weft_col = params.weft_color + vec3<f32>(weft_var);

    // Determine visible color based on which thread is on top
    var color: vec3<f32>;
    var height = 0.0; // for normal calculation

    if is_warp_top {
        // Warp on top: warp visible where present, weft visible in gaps
        let overlap = warp_mask * weft_mask;
        if warp_mask > weft_mask {
            color = warp_col;
            height = warp_mask * 0.5;
        } else {
            color = weft_col;
            height = weft_mask * 0.3; // underneath, slightly lower
        }
    } else {
        // Weft on top
        if weft_mask > warp_mask {
            color = weft_col;
            height = weft_mask * 0.5;
        } else {
            color = warp_col;
            height = warp_mask * 0.3;
        }
    }

    // Add fine fiber noise
    let fiber_noise = fbm(uv * 300.0, 2) * 0.03;
    out.base_color = clamp(color + vec3<f32>(fiber_noise), vec3<f32>(0.0), vec3<f32>(1.0));

    // Fabric is rough and not metallic
    out.roughness = 0.85;
    out.metallic = 0.0;

    // Normal from thread curvature
    // Warp threads curve in X, weft threads curve in Y
    let warp_curve = -cos(local.x * 3.14159 * 2.0) * warp_mask;
    let weft_curve = -cos(local.y * 3.14159 * 2.0) * weft_mask;

    var nx: f32;
    var ny: f32;
    if is_warp_top {
        nx = sin(local.x * 3.14159 * 2.0) * warp_mask * 0.3;
        ny = sin(local.y * 3.14159 * 2.0) * weft_mask * 0.15;
    } else {
        nx = sin(local.x * 3.14159 * 2.0) * warp_mask * 0.15;
        ny = sin(local.y * 3.14159 * 2.0) * weft_mask * 0.3;
    }

    out.normal = normalize(vec3<f32>(nx, ny, 1.0));

    return out;
}
