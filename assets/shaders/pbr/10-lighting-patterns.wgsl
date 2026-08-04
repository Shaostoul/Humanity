// ── Lighting ──

const PI: f32 = 3.14159265359;

// Directional lights are now driven from Rust via CameraUniforms.
// camera.sun_direction.xyz = direction, .w = intensity
// camera.sun_color.rgb = color
// camera.fill_direction.xyz = direction, .w = intensity
// camera.fill_color.rgb = color

// Ambient
const AMBIENT_COLOR: vec3<f32> = vec3<f32>(0.03, 0.03, 0.05);

// ── GGX PBR Functions ──

// Normal Distribution Function (Trowbridge-Reitz GGX)
fn distribution_ggx(n_dot_h: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let denom = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (PI * denom * denom);
}

// Geometry function (Schlick-GGX)
fn geometry_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return n_dot_v / (n_dot_v * (1.0 - k) + k);
}

// Smith's method: combined geometry for both view and light directions
fn geometry_smith(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
    return geometry_schlick_ggx(n_dot_v, roughness) * geometry_schlick_ggx(n_dot_l, roughness);
}

// Fresnel-Schlick approximation
fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    let t = clamp(1.0 - cos_theta, 0.0, 1.0);
    let t2 = t * t;
    return f0 + (vec3<f32>(1.0) - f0) * (t2 * t2 * t);
}

// ── VEGETATION: CROWN DEPTH AND THE CANOPY CARD KERNEL (v0.1110) ─────────
//
// THE DEFECT THIS EXISTS TO CLOSE. Operator, v0.1109.3: "The LOD billboards
// seem like they're getting full light making them much brighter than the high
// poly trees." A top-down capture shows it exactly: a DARK disc of 3D models
// under the camera, a clean circle at the model/card handoff radius, and a
// much BRIGHTER field of cards beyond it.
//
// The three vegetation representations disagreed about light in two ways, and
// both are pure gain, so they multiply:
//
//   1. ALBEDO. A near cluster card (type 21) shows its sprite texel times
//      `crown_depth_shade(ao)` below - the crown's own extinction, 0.48 at the
//      shaded core rising to 1.00 at the sunlit shell. The atlas BAKE decoded
//      the very same AO code out of the very same uv and then THREW IT AWAY
//      (billboard_bake::BAKE_WGSL), so every baked sprite carried the crown's
//      albedo with none of the crown's depth. Measured mean over the shipped
//      species' cards: see billboard_bake::canopy_parity_tests.
//
//   2. DIRECTION. A card's shading normal is the RADIAL UP, so its sun term is
//      max(up . L, 0) - uniform over the whole card, and the single most
//      favourable orientation there is. A real crown is a volume of leaves
//      pointing every which way: the near path's own leaves carry a spread of
//      normals (tree_mesh::emit_card spherifies each corner toward its tuft
//      centre and then toward the crown centre), so its mean response is far
//      lower AND far flatter with sun angle. Lighting a card as a plate facing
//      the sky is what "full light" means.
//
// WHAT REPLACES IT. Not a fudge factor: the near path's own statistic in
// closed form. Take the crown's leaf normals as isotropic (which is what
// spherifying toward a ball centre produces in aggregate) and ask for the mean
// of max(N . L, 0) over the leaves you can actually SEE, i.e. weighted by
// |N . V|. With psi the sun-view angle,
//
//   E[max(N.L,0) |N.V|] / E[|N.V|] = (2 sin psi + (pi - 2 psi) cos psi) / 3 pi
//
// (numerator from the standard lune integral over the sphere; denominator is
// exactly 1/2 for any V, the spherical leaf-angle distribution's G-function -
// Ross, "The Radiation Regime and Architecture of Plant Stands", 1981.)
//
// It lands at 1/3 head-on (psi = 0), 2/(3 pi) = 0.212 across (psi = pi/2), and
// 1/3 again at full backlight, because the near path deliberately does NOT
// flip a card's normal on its back face - a leaf lit from behind glows rather
// than going black. So this kernel reproduces the near path's behaviour, not
// an idealisation of it. `canopy_ndl_mean_matches_monte_carlo` in
// billboard_bake proves the closed form against sampled normals, and
// `canopy_kernel_matches_the_near_crown` proves it against the REAL cards of
// the shipped species.

// The sky share a canopy card keeps, against the fully sky-facing surface its
// radial-up normal would otherwise be. sky_ambient is linear in
// w = 0.5 + 0.5 * dot(n, up), and the isotropic-leaf mean of that w is exactly
// 0.5 for ANY view direction (the odd part integrates away), so the fraction is
// (SKY_GROUND_BOUNCE + (1 - SKY_GROUND_BOUNCE) * 0.5) / 1.0 = 0.5 * (1 + g).
// LOCKSTEP with SKY_GROUND_BOUNCE in 90-fragment-main.wgsl - written as a
// literal only because that constant is declared in a later part;
// `canopy_sky_fraction_matches_the_ground_bounce` fails if either drifts.
const CANOPY_SKY_FRACTION: f32 = 0.61;

// How much darker a far-sheet canopy card is than the ground imagery it grows
// out of. A closed forest canopy reflects less than the open ground beside it
// and does it greener; these are the numbers `terrain::far_trees` used to bake
// into a per-card colour that the shader then ignored (the far sheet draws with
// the planet's TEXTURED material, so the has_tex branch overwrote the packed
// colour with the raw imagery). LOCKSTEP with far_trees::FAR_CARD_CANOPY_TINT.
const CANOPY_GROUND_TINT: vec3<f32> = vec3<f32>(0.68, 0.76, 0.62);

// Per-CELL tone jitter for a far-sheet canopy card. far_trees varies each
// cell's canopy a little so a 150 km sheet does not read as one flat wash; the
// code rides bits 18..21 of the type-12 packed channel, which that type has
// never used (it reads bits 0..17: colour, water at 16, tree card at 17).
// LOCKSTEP with far_trees::{FAR_CARD_SHADE_LO, FAR_CARD_SHADE_HI} - checked by
// `far_card_shade_constants_match_the_shader`.
const CANOPY_CARD_SHADE_LO: f32 = 0.85;
const CANOPY_CARD_SHADE_HI: f32 = 1.15;

fn canopy_card_shade(packed: u32) -> f32 {
    let code = f32((packed >> 18u) & 15u) / 15.0;
    return CANOPY_CARD_SHADE_LO + (CANOPY_CARD_SHADE_HI - CANOPY_CARD_SHADE_LO) * code;
}

// CROWN-DEPTH LOCKSTEP BEGIN
// Multiplier a cluster card at crown depth `ao` applies to its sprite texel:
// an achromatic extinction toward the shaded core, plus the sun-leaf /
// shade-leaf CHROMATIC gradient (outer leaves smaller, thicker, yellower;
// inner ones larger, thinner, bluer - Boardman 1977, Annu. Rev. Plant Physiol.
// 28:355). The two tints are near luminance-neutral against the Rec.709
// weights, so the achromatic part of the gradient lives entirely in the
// (0.35 + 0.65 * ao) term.
//
// This block is DUPLICATED VERBATIM into billboard_bake::BAKE_WGSL and
// `crown_depth_shade_is_identical_in_the_bake_shader` fails if the two copies
// drift by so much as a space. The bake has no way to include a WGSL file, and
// a card that bakes with a different crown depth than it renders with is
// exactly the brightness step this whole block exists to remove.
fn crown_depth_shade(ao: f32) -> vec3<f32> {
    let tint = mix(
        vec3<f32>(0.88, 0.98, 1.10),
        vec3<f32>(1.10, 1.03, 0.84),
        ao);
    return tint * (0.35 + 0.65 * ao);
}
// CROWN-DEPTH LOCKSTEP END

// Visible-area-weighted mean of max(N . L, 0) over an isotropic leaf cloud,
// as a function of cos(psi) = dot(L, V). See the derivation above.
fn canopy_ndl_mean(cos_psi: f32) -> f32 {
    let c = clamp(cos_psi, -1.0, 1.0);
    let psi = acos(c);
    let s = sqrt(max(1.0 - c * c, 0.0));
    return (2.0 * s + (PI - 2.0 * psi) * c) / (3.0 * PI);
}

// What a vegetation CARD hands the shared PBR tail so it shades like the crown
// it stands for. Two scalars, because the tail already has exactly two places
// that want them and nowhere else does:
//
//   sun_gain -> sun_gate, which multiplies ONLY the direct sun term. The gain
//     is the ratio that turns evaluate_light's built-in max(n . l, 0) - taken
//     against the card's radial-up normal - into the canopy mean. Clamped
//     because that ratio diverges as the sun reaches the local horizon; at the
//     clamp the sun is within ~0.3 degrees of setting and the type-12
//     terminator smoothstep has already closed.
//
//   sky_frac -> ao, which multiplies ONLY the indirect term. Not a fudge: the
//     leaves of a crown really are turned away from the sky in proportion, and
//     `ao` is the tail's documented hemisphere-occlusion input (v0.1104).
//
// A struct return, deliberately: an array return passes naga and then fails at
// device init on the DX12 HLSL backend (shader_loader::check_hlsl_expressible,
// the v0.1101 incident).
struct CanopyShading {
    sun_gain: f32,
    sky_frac: f32,
};

fn canopy_card_shading(n: vec3<f32>, sun_dir: vec3<f32>, view_dir: vec3<f32>) -> CanopyShading {
    let l = normalize(sun_dir);
    let v = normalize(view_dir);
    let plate_ndl = max(dot(normalize(n), l), 0.0);
    var out: CanopyShading;
    out.sun_gain = clamp(canopy_ndl_mean(dot(l, v)) / max(plate_ndl, 1.0e-3), 0.0, 64.0);
    out.sky_frac = CANOPY_SKY_FRACTION;
    return out;
}

// ── Procedural Patterns ──

// Hash function for procedural noise
fn hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + vec3<f32>(dot(p3, vec3<f32>(p3.y + 33.33, p3.z + 33.33, p3.x + 33.33)));
    return fract((p3.x + p3.y) * p3.z);
}

// Value noise
fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f); // smoothstep

    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));

    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// FBM (fractal Brownian motion) — 4 octaves
fn fbm(p: vec2<f32>) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var pp = p;
    for (var i = 0; i < 4; i = i + 1) {
        value = value + amplitude * value_noise(pp * frequency);
        frequency = frequency * 2.0;
        amplitude = amplitude * 0.5;
    }
    return value;
}

// Panel seam grid (1m panels with 3cm seam lines)
fn grid_pattern(world_pos: vec3<f32>, normal: vec3<f32>) -> f32 {
    var u: f32;
    var v: f32;
    let an = abs(normal);
    if an.y > an.x && an.y > an.z {
        u = world_pos.x;
        v = world_pos.z;
    } else if an.x > an.z {
        u = world_pos.y;
        v = world_pos.z;
    } else {
        u = world_pos.x;
        v = world_pos.y;
    }
    let seam_width = 0.03;
    let fu = fract(u);
    let fv = fract(v);
    let su = smoothstep(0.0, seam_width, fu) * smoothstep(0.0, seam_width, 1.0 - fu);
    let sv = smoothstep(0.0, seam_width, fv) * smoothstep(0.0, seam_width, 1.0 - fv);
    return su * sv;
}

// Brushed metal pattern (directional micro-scratches).
// v0.696 fix: the old vec2(u * 200.0, 0.0) sampled noise along ONE axis with
// the other pinned to zero -- mathematically that is unbroken full-length
// stripes, which is exactly the "horizontal or vertical lines of varied
// colors" the operator screenshotted. A cross-axis coordinate keeps the
// brushed DIRECTION while ending each scratch, and a low-frequency 2D breakup
// varies the field so it reads as metal, not wallpaper.
fn brushed_metal(world_pos: vec3<f32>, normal: vec3<f32>) -> f32 {
    var u: f32;
    var v: f32;
    let an = abs(normal);
    if an.y > an.x && an.y > an.z {
        u = world_pos.x;
        v = world_pos.z;
    } else if an.x > an.z {
        u = world_pos.y;
        v = world_pos.z;
    } else {
        u = world_pos.x;
        v = world_pos.y;
    }
    let scratch = value_noise(vec2<f32>(u * 200.0, v * 7.0));
    let breakup = value_noise(vec2<f32>(u * 3.0, v * 3.0));
    return mix(0.85, 1.0, scratch * 0.7 + breakup * 0.3);
}

// Concrete texture (rough, speckled surface)
fn concrete_pattern(world_pos: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    let an = abs(normal);
    var uv: vec2<f32>;
    if an.y > an.x && an.y > an.z {
        uv = world_pos.xz;
    } else if an.x > an.z {
        uv = world_pos.yz;
    } else {
        uv = world_pos.xy;
    }
    let noise = fbm(uv * 3.0);
    let speckle = value_noise(uv * 40.0) * 0.08;
    // Slightly varied grey
    let base = 0.55 + noise * 0.15 + speckle;
    return vec3<f32>(base, base * 0.98, base * 0.96);
}

// Tri-planar UV projection (reusable helper)
fn triplanar_uv(world_pos: vec3<f32>, normal: vec3<f32>) -> vec2<f32> {
    let an = abs(normal);
    if an.y > an.x && an.y > an.z {
        return world_pos.xz;
    } else if an.x > an.z {
        return world_pos.yz;
    }
    return world_pos.xy;
}

// Voronoi cell noise (returns distance to nearest cell center)
fn voronoi(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    var min_dist = 1.0;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let neighbor = vec2<f32>(f32(x), f32(y));
            let cell_center = vec2<f32>(hash21(i + neighbor), hash21(i + neighbor + vec2<f32>(57.0, 113.0)));
            let diff = neighbor + cell_center - f;
            min_dist = min(min_dist, dot(diff, diff));
        }
    }
    return sqrt(min_dist);
}

// Voronoi EDGE distance (F2 - F1): near 0 ON a cell border, large at a cell
// centre. `voronoi` above returns F1, which is smallest at CENTRES, so
// thresholding it paints blobs. A leaf vein (or any crack/seam network) is a
// BORDER, so it needs this. Added v0.1063 for the type-20 plant branch.
fn voronoi_edge(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    var d1 = 8.0;
    var d2 = 8.0;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let neighbor = vec2<f32>(f32(x), f32(y));
            let cell_center = vec2<f32>(hash21(i + neighbor), hash21(i + neighbor + vec2<f32>(57.0, 113.0)));
            let diff = neighbor + cell_center - f;
            let d = dot(diff, diff);
            if (d < d1) {
                d2 = d1;
                d1 = d;
            } else if (d < d2) {
                d2 = d;
            }
        }
    }
    return sqrt(d2) - sqrt(d1);
}

// Wood grain pattern
fn wood_pattern(world_pos: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    let an = abs(normal);
    var uv: vec2<f32>;
    if an.y > an.x && an.y > an.z {
        uv = world_pos.xz;
    } else if an.x > an.z {
        uv = world_pos.yz;
    } else {
        uv = world_pos.xy;
    }
    // Ring pattern along one axis
    let ring = sin(uv.x * 25.0 + fbm(uv * 2.0) * 6.0) * 0.5 + 0.5;
    let grain = value_noise(vec2<f32>(uv.x * 2.0, uv.y * 80.0)) * 0.1;
    // Warm wood tones
    let dark = vec3<f32>(0.35, 0.2, 0.1);
    let light = vec3<f32>(0.6, 0.4, 0.2);
    return mix(dark, light, ring) + vec3<f32>(grain);
}

