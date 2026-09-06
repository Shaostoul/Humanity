// The cloud PROFILE atlas tracer (perf increment 4, the far rung; E1 gate
// instrument, 2026-09-06). Ray-traces a dumped profile atlas through the
// SHADER'S OWN read (the lattice, the arithmetic level walk with the D1
// re-walk, the toroidal 4-tap bilinear window reads, the global map's mip
// pair, the bin bracketing) and the element law cloud_fr_t_pf
// (40-clouds.wgsl), and prints the predicted decoded mean alpha per
// off-nadir band and the mask fraction at the 0.216 LINEAR threshold (= the
// encoded byte 128 the mask gates use), beside the same numbers MEASURED from
// a map_diag 1 capture of the same camera (sRGB-decoded: the diag leaves
// through the sRGB swapchain), and the residual per band (measured minus
// predicted). The residual is REPORTED, never folded into a pass bar
// (orchestrator decision, E1 lever 2): today it reads about 13 percent of
// the crop mean and 26 percent at the nadir, an unnamed mechanism.
//
//   node scripts/cloud-profile-trace.js --dump=<dir> --mask=<map_diag 1 capture.png> --alt=873
//        [--lat=23 --lon=13] [--res=1] [--rows=1387] [--fov=90.05] [--type=0.34]
//        [--law=shipped|fixed] [--knob=1|hard|L0..L5] [--sub=4] [--steps=4]
//        [--orient=auto] [--crop=0.8] [--bands=0,10,20,30,40,50,90]
//        [--gI=<n> --gJ=<n> --flags=<n>] [--sigma=<per km>] [--base=0.4 --top=12] [--radius=6371]
//
//   --dump    the atlas dump folder (<sweep>/<id>-profile/: cloud_profile_L<L>_s<p>.png,
//             cloud_profile_global_0/1.png and dump.json with ground_i0 / ground_j0 / flags;
//             a cell with dump_cloud_profile: true writes it).
//   --mask    the map_diag 1 capture to measure (the same camera; the atlas is planet-fixed,
//             so a dump from a neighbouring rung serves as long as its windows cover the view:
//             the 1000 km dump against the 873 km mask, both on the Sahara camera).
//   --alt     camera altitude, km; --lat / --lon the camera's ground point, degrees; look 0 only.
//   --res     cloud_res (the march texture's row divisor): pix_ang = 2 tan(fov / 2) / floor(rows / res).
//   --type    the pinned cloud type: the regime's extinction per km, its height band and the
//             element sizes of its family come from the shader's own tables (cloud_regime,
//             cv2_arch_index, cv2_elem_table_m), copied here; keep them in lockstep.
//   --law     shipped: cloud_fr_t_pf as called in v0.1293 (l_v = max(L_v, dz), l_h = L_h unscaled);
//             fixed: E1 lever 2, l_h = L_h * (l_v_eff / L_v), so a slant ray counts horizontal
//             element crossings only inside the cloud layer (expected crossings
//             n A_proj s = f s (c_v / dz + c_h L_v / (L_h dz))). Default shipped.
//   --knob    1 (auto: w_pf = smoothstep(-2, 0, log2(slant * pix_ang) + jitter * 0.35), the pixel
//             footprint of E1 lever 1, jitter 0 here), hard (knob 8: w = 1 where lodf >= -1, no
//             blend), or L0..L5 (that level forced, w = 1 on every sample). The tracer predicts
//             the PROFILE share only; where 0 < w < 1 the marched share is not modelled and the
//             tracer says how many samples sat in the band.
//   --sub     pixel stride (4: every 4th pixel each way); --steps sub-steps per slab bin (4).
//   --orient  auto tries the four image orientations (east/west right, north/south up) and
//             reports each one's pixel correlation between predicted and measured alpha;
//             the default (east right, north up) is the one the reader validated at 0.99.
//
// Prints: the element numbers used, the orientation correlation(s) (the validation: 0.99 on
// the 873 km mask against the 1000 km dump), then per band n, MEASURED mean alpha and mask
// fraction, PREDICTED mean alpha and mask fraction, the residual (measured minus predicted,
// absolute and as a percent of predicted), the mean f the tap saw and the mean level, and
// the crop totals. Exit code 0 always: the script measures, the orchestrator decides.
const sharp = require("sharp");
const fs = require("fs");
const path = require("path");

const argv = process.argv.slice(2);
const opt = (n, d) => {
  const a = argv.find(x => x.startsWith("--" + n + "="));
  return a ? a.slice(n.length + 3) : d;
};
const DUMP = opt("dump");
const MASK = opt("mask");
if (!DUMP || !MASK) {
  console.error("usage: node scripts/cloud-profile-trace.js --dump=<dir> --mask=<png> --alt=<km> [options; see the header]");
  process.exit(2);
}
const ALT = +opt("alt", "873");
const LAT0 = (+opt("lat", "23")) * Math.PI / 180;
const LON0 = (+opt("lon", "13")) * Math.PI / 180;
const RES = +opt("res", "1");
const ROWS = +opt("rows", "1387");
const FOV = +opt("fov", "90.05");
const TC = +opt("type", "0.34");
const LAW = opt("law", "shipped");
const KNOB = opt("knob", "1");
const SUB = Math.max(1, +opt("sub", "4"));
const STEPS = Math.max(1, +opt("steps", "4"));
const ORIENT = opt("orient", "fixed");
const CROP = +opt("crop", "0.8");
const BANDS = opt("bands", "0,10,20,30,40,50,90").split(",").map(Number);
const R = +opt("radius", "6371");
const BASE = +opt("base", "0.4");   // Earth's slab, km (planet.rs cloud_slab_scales defaults)
const TOP = +opt("top", "12");
const JITTER = +opt("jitter", "0");

// ── The shader's constants (40-clouds.wgsl, 41-cloud-bodies.wgsl) ──
const CELL0 = 0.25, LEVELS = 6, NX = 512, NZ = 12, PAIRS = 6, SLICES_PER_LEVEL = 9, SLICE_COLS = 12;
const GLOBAL_W = 2048, GLOBAL_H = 1024, GLOBAL_NZ = 4, GLOBAL_MIPS = 7;
const BLEND_FRAC = 0.20, LOD0 = -2.0, LOD_LO = -2.0, LOD_HI = 0.0, F_EPS = 0.02;
const ELEM_THIN_KM = 8.0, ELEM_SQUASH = 0.65;
const pmod = (a, n) => ((a % n) + n) % n;
const clamp = (x, a, b) => Math.min(Math.max(x, a), b);
const mix = (a, b, t) => a + (b - a) * t;
const smoothstep = (a, b, x) => { const t = clamp((x - a) / (b - a), 0, 1); return t * t * (3 - 2 * t); };
const srgbDecode = v => { v /= 255; return v <= 0.04045 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4); };
// The mask gate's byte threshold 128, in linear alpha (the sRGB decode of 128 / 255).
const T_LINEAR = srgbDecode(128);

// cloud_regime(tc): the tent-weighted family tables (only the rows the
// tracer needs: the height band and the extinction per km).
function cloudRegime(tc) {
  const centers = [0.0, 0.17, 0.33, 0.5, 0.67, 0.83, 1.0];
  const t_h_lo = [0.48, 0.14, 0.01, 0.01, 0.00, 0.00, 0.01];
  const t_h_hi = [1.00, 0.57, 0.48, 1.00, 0.14, 0.40, 0.18];
  const t_ext = [1.2, 8.0, 45.0, 60.0, 22.0, 30.0, 20.0];
  const hw = 0.22;
  let s = 0, h_lo = 0, h_hi = 0, ext = 0;
  for (let i = 0; i < 7; i++) {
    let wi = clamp(1 - Math.abs(tc - centers[i]) / hw, 0, 1);
    wi = wi * wi * (3 - 2 * wi);
    s += wi; h_lo += wi * t_h_lo[i]; h_hi += wi * t_h_hi[i]; ext += wi * t_ext[i];
  }
  const inv = 1 / Math.max(s, 1e-4);
  return { h_lo: h_lo * inv, h_hi: h_hi * inv, ext_km: ext * inv };
}
// cv2_arch_index(tc): -1 thin (cirrus / altocu), 0 humilis, 1 congestus, 3 cumulonimbus, 2 stratocumulus.
function archIndex(tc) {
  if (tc < 0.25) return -1;
  if (tc < 0.40) return 0;
  if (tc < 0.50) return 1;
  if (tc < 0.58) return 3;
  return 2;
}
// cv2_elem_table_m(arch_i): (L_h, L_v) in metres, uncapped.
function elemTableM(i) {
  const w_lo = [300, 800, 1500, 3000], w_hi = [1200, 3000, 6000, 8000];
  const a_lo = [0.45, 1.20, 0.12, 1.60], a_hi = [0.75, 2.60, 0.28, 3.20];
  i = clamp(i, 0, 3);
  const l_h = Math.sqrt(w_lo[i] * w_hi[i]);
  return [l_h, l_h * (a_lo[i] + a_hi[i]) * 0.5 * ELEM_SQUASH];
}
// cloud_fr_elem_km(tc, reg, slab_km): (L_h, L_v) in km, L_v capped at the band.
function elemKm(tc, reg, slab_km) {
  const band_km = Math.max((reg.h_hi - reg.h_lo) * slab_km, 1e-3);
  const i = archIndex(tc);
  if (i < 0) return [ELEM_THIN_KM, band_km];
  const e = elemTableM(i).map(v => v * 0.001);
  return [Math.max(e[0], 1e-3), Math.max(Math.min(e[1], band_km), 1e-3)];
}

async function rawPng(f) {
  const { data, info } = await sharp(f).raw().toBuffer({ resolveWithObject: true });
  return { d: data, W: info.width, H: info.height, C: info.channels };
}

(async () => {
  // ── The dump: per level the six pair slices as f[k], G[k] over 512 x 512 ──
  const dumpJson = fs.existsSync(path.join(DUMP, "dump.json")) ? JSON.parse(fs.readFileSync(path.join(DUMP, "dump.json"), "utf8")) : {};
  const gI0 = +opt("gI", dumpJson.ground_i0 != null ? String(dumpJson.ground_i0) : "0");
  const gJ0 = +opt("gJ", dumpJson.ground_j0 != null ? String(dumpJson.ground_j0) : "0");
  const FLAGS = +opt("flags", dumpJson.flags != null ? String(dumpJson.flags) : "0");
  const levelValid = L => ((FLAGS >> (2 + L)) & 1) === 1;
  const globalValid = ((FLAGS >> 1) & 1) === 1;
  const levels = [];
  for (let L = 0; L < LEVELS; L++) {
    const f = Array.from({ length: NZ }, () => new Float32Array(NX * NX));
    const G = Array.from({ length: NZ }, () => new Float32Array(NX * NX));
    if (levelValid(L)) {
      for (let p = 0; p < PAIRS; p++) {
        const s = await rawPng(path.join(DUMP, `cloud_profile_L${L}_s${p}.png`));
        for (let i = 0; i < NX * NX; i++) {
          const o = i * s.C;
          f[2 * p][i] = s.d[o] / 255; G[2 * p][i] = s.d[o + 1] / 255;
          f[2 * p + 1][i] = s.d[o + 2] / 255; G[2 * p + 1][i] = s.d[o + 3] / 255;
        }
      }
    }
    levels.push({ f, G });
  }
  // The global: pair slices 0 and 1 at mip 0, then the box-filtered mip
  // chain the shader reads through textureSampleLevel at integer mips
  // (fs_cloud_profile_mip: plain 2x2 means of the pair channels; the
  // column slice is not needed for alpha and is not loaded).
  const gmips = [];
  if (globalValid) {
    const g0 = await rawPng(path.join(DUMP, "cloud_profile_global_0.png"));
    const g1 = await rawPng(path.join(DUMP, "cloud_profile_global_1.png"));
    const m0 = { w: GLOBAL_W, h: GLOBAL_H, fp: Array.from({ length: 4 }, () => new Float32Array(GLOBAL_W * GLOBAL_H)), Gp: Array.from({ length: 4 }, () => new Float32Array(GLOBAL_W * GLOBAL_H)) };
    for (let i = 0; i < GLOBAL_W * GLOBAL_H; i++) {
      const o0 = i * g0.C, o1 = i * g1.C;
      m0.fp[0][i] = g0.d[o0] / 255; m0.Gp[0][i] = g0.d[o0 + 1] / 255; m0.fp[1][i] = g0.d[o0 + 2] / 255; m0.Gp[1][i] = g0.d[o0 + 3] / 255;
      m0.fp[2][i] = g1.d[o1] / 255; m0.Gp[2][i] = g1.d[o1 + 1] / 255; m0.fp[3][i] = g1.d[o1 + 2] / 255; m0.Gp[3][i] = g1.d[o1 + 3] / 255;
    }
    gmips.push(m0);
    for (let m = 1; m < GLOBAL_MIPS; m++) {
      const p = gmips[m - 1], w = p.w >> 1, h = p.h >> 1;
      const mm = { w, h, fp: Array.from({ length: 4 }, () => new Float32Array(w * h)), Gp: Array.from({ length: 4 }, () => new Float32Array(w * h)) };
      for (let q = 0; q < 4; q++) for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
        const a = (2 * y) * p.w + 2 * x, b = a + 1, c = a + p.w, d = c + 1;
        mm.fp[q][y * w + x] = 0.25 * (p.fp[q][a] + p.fp[q][b] + p.fp[q][c] + p.fp[q][d]);
        mm.Gp[q][y * w + x] = 0.25 * (p.Gp[q][a] + p.Gp[q][b] + p.Gp[q][c] + p.Gp[q][d]);
      }
      gmips.push(mm);
    }
  }

  // ── The read: cloud_profile_window_uv / contains / walk / level / global / tap ──
  const cellKm = L => CELL0 * Math.pow(2, L);
  function windowUv(L, lon, lat) {
    const c_km = cellKm(L), cell_rad = c_km / R;
    const NI = Math.floor(2 * Math.PI * R / c_km);
    const gI = Math.floor(gI0 / Math.pow(2, L)), gJ = Math.floor(gJ0 / Math.pow(2, L));
    const u = (lon + Math.PI) / cell_rad - 0.5, v = (lat + Math.PI / 2) / cell_rad - 0.5;
    let du = u - gI;
    if (du >= 0.5 * NI) du -= NI; else if (du < -0.5 * NI) du += NI;
    return [du, v - gJ];
  }
  const HALF = NX / 2;
  function contains(L, lon, lat) {
    if (!levelValid(L)) return false;
    const [du, dv] = windowUv(L, lon, lat);
    return du >= -HALF && du < HALF - 1 && dv >= -HALF && dv < HALF - 1;
  }
  function walk(L0, lon, lat) {
    for (let L = L0; L < LEVELS; L++) if (contains(L, lon, lat)) return L;
    return LEVELS;
  }
  // cloud_profile_level: the 4-tap bilinear read of f and G at the bin pair bracketing h.
  function readLevel(L, lon, lat, h) {
    const r = { ok: false, f: 0, G: 0, w_edge: 1 };
    const c_km = cellKm(L);
    const NJ = Math.floor(Math.PI * R / c_km);
    const gI = Math.floor(gI0 / Math.pow(2, L)), gJ = Math.floor(gJ0 / Math.pow(2, L));
    const [du, dv] = windowUv(L, lon, lat);
    if (!(levelValid(L) && du >= -HALF && du < HALF - 1 && dv >= -HALF && dv < HALF - 1)) return r;
    const i0 = Math.floor(du), fu = du - i0, j0 = Math.floor(dv), fv = dv - j0;
    const m = Math.max(Math.abs(du), Math.abs(dv)) / (HALF - 1);
    r.w_edge = smoothstep(1 - BLEND_FRAC, 1, m);
    const ia = gI + i0, ja = gJ + j0;
    const xa = pmod(ia, NX), xb = pmod(ia + 1, NX);
    const ya = pmod(clamp(ja, 0, NJ - 1), NX), yb = pmod(clamp(ja + 1, 0, NJ - 1), NX);
    const hz = h * NZ;
    const fk = clamp(hz - 0.5, 0, NZ - 1), k0 = Math.floor(fk), k1 = Math.min(k0 + 1, NZ - 1), wk = fk - k0;
    const bil = arr => mix(mix(arr[ya * NX + xa], arr[ya * NX + xb], fu), mix(arr[yb * NX + xa], arr[yb * NX + xb], fu), fv);
    r.f = mix(bil(levels[L].f[k0]), bil(levels[L].f[k1]), wk);
    r.G = mix(bil(levels[L].G[k0]), bil(levels[L].G[k1]), wk);
    r.ok = true;
    return r;
  }
  // cloud_profile_global_fetch(m): hardware bilinear at integer mip m, clamped to texel centres.
  function globalFetch(m, lon, lat) {
    const g = gmips[m], w_m = g.w, h_m = g.h;
    const u = clamp((lon + Math.PI) / (2 * Math.PI) * w_m, 0.5, w_m - 0.5) - 0.5;
    const v = clamp((0.5 * Math.PI - lat) / Math.PI * h_m, 0.5, h_m - 0.5) - 0.5;
    const x0 = Math.floor(u), y0 = Math.floor(v), fu = u - x0, fv = v - y0;
    const x1 = Math.min(x0 + 1, w_m - 1), y1 = Math.min(y0 + 1, h_m - 1);
    const rd = arr => mix(mix(arr[y0 * w_m + x0], arr[y0 * w_m + x1], fu), mix(arr[y1 * w_m + x0], arr[y1 * w_m + x1], fu), fv);
    return { fp: g.fp.map(rd), Gp: g.Gp.map(rd) };
  }
  function readGlobal(lon, lat, h, lodb) {
    const r = { ok: false, f: 0, G: 0 };
    if (!globalValid) return r;
    const global_km = 2 * Math.PI * R / GLOBAL_W;
    const mf = clamp(lodb - Math.log2(global_km), 0, GLOBAL_MIPS - 1);
    const m0 = Math.floor(mf), m1 = Math.min(m0 + 1, GLOBAL_MIPS - 1), wm = mf - m0;
    const a = globalFetch(m0, lon, lat), b = m1 !== m0 ? globalFetch(m1, lon, lat) : a;
    const fp = a.fp.map((v, i) => mix(v, b.fp[i], wm)), Gp = a.Gp.map((v, i) => mix(v, b.Gp[i], wm));
    const hz = h * GLOBAL_NZ;
    const fk = clamp(hz - 0.5, 0, GLOBAL_NZ - 1), k0 = Math.floor(fk), k1 = Math.min(k0 + 1, GLOBAL_NZ - 1), wk = fk - k0;
    r.f = mix(fp[k0], fp[k1], wk); r.G = mix(Gp[k0], Gp[k1], wk); r.ok = true;
    return r;
  }
  const knobForced = /^L[0-5]$/.test(KNOB) ? +KNOB[1] : -1;
  const knobHard = KNOB === "hard";
  // cloud_profile_tap with the D1 re-walk (v0.1293): at most two window
  // fetches plus the global; the hand-off weights sum to 1.
  function tap(lon, lat, h, lodb) {
    let lv = clamp(lodb - LOD0, 0, LEVELS - 1);
    if (knobForced >= 0) lv = knobForced;
    let La = Math.floor(lv), Lb = Math.min(La + 1, LEVELS - 1), wl = lv - La;
    if (knobHard) wl = 0;
    La = walk(La, lon, lat); Lb = walk(Lb, lon, lat);
    if (La >= Lb) { wl = 0; Lb = La < LEVELS - 1 ? walk(La + 1, lon, lat) : LEVELS; }
    const ra = La < LEVELS ? readLevel(La, lon, lat, h) : { ok: false, f: 0, G: 0, w_edge: 1 };
    let ea = ra.ok ? ra.w_edge : 1;
    if (knobHard) ea = 0;
    const rb = (Lb < LEVELS && Lb !== La && (wl > 0 || ea > 0)) ? readLevel(Lb, lon, lat, h) : { ok: false, f: 0, G: 0, w_edge: 1 };
    let eb = rb.ok ? rb.w_edge : 1;
    if (knobHard) eb = 0;
    let share_a = 1 - wl, share_b = wl, w_g = 0;
    if (La === LEVELS) { share_a = 0; share_b = 0; w_g = 1; }
    let w_ra = share_a * (1 - ea);
    const handed_a = share_a * ea;
    if (rb.ok) share_b += handed_a; else w_g += handed_a;
    let w_rb = share_b * (1 - eb);
    w_g += share_b * eb;
    const g = w_g > 0 ? readGlobal(lon, lat, h, lodb) : { ok: false, f: 0, G: 0 };
    if (w_g > 0 && !g.ok) {
      const wsum = w_ra + w_rb;
      if (wsum <= 0) return { ok: false, f: 0, G: 0, level: 0 };
      w_ra /= wsum; w_rb /= wsum; w_g = 0;
    }
    return { ok: true, f: w_ra * ra.f + w_rb * rb.f + w_g * g.f, G: w_ra * ra.G + w_rb * rb.G + w_g * g.G, level: w_ra * La + w_rb * Lb + w_g * LEVELS };
  }

  // ── The element law (cloud_fr_t_pf) with the E1 lever-2 option ──
  const reg = cloudRegime(TC);
  const slab_km = TOP - BASE, dz = slab_km / NZ;
  const SIGMA = +opt("sigma", String(reg.ext_km));
  const [L_h, L_v] = elemKm(TC, reg, slab_km);
  const l_v_eff = Math.max(L_v, dz);
  const l_h_eff = LAW === "fixed" ? L_h * (l_v_eff / L_v) : L_h;
  function tPf(w, f, D_in, c_v, seg) {
    const c_h = Math.sqrt(Math.max(1 - c_v * c_v, 0));
    const l_elem = 1 / Math.max(c_v / l_v_eff + c_h / l_h_eff, 1e-9);
    const tau_elem = SIGMA * D_in * l_elem;
    const per_elem = Math.max(1 - w * f * (1 - Math.exp(-tau_elem)), 0);
    return Math.pow(per_elem, Math.max(seg / l_elem, 1e-6));
  }

  // ── The camera (look 0: the nadir at the frame centre) ──
  const mask = await rawPng(MASK);
  const W = mask.W, H = mask.H;
  const CX = W / 2, CY = Math.floor(H / 2);
  const FPX = (H / 2) / Math.tan(FOV * 0.5 * Math.PI / 180);           // focal length, px (693 at 1387 rows, fov 90.05)
  const PIX = 2 * Math.tan(FOV * 0.5 * Math.PI / 180) / Math.floor(ROWS / RES);   // pix_ang_march, rad
  const up = [Math.cos(LAT0) * Math.cos(LON0), Math.sin(LAT0), -Math.cos(LAT0) * Math.sin(LON0)];
  const north = [-Math.sin(LAT0) * Math.cos(LON0), Math.cos(LAT0), Math.sin(LAT0) * Math.sin(LON0)];
  const east = [-Math.sin(LON0), 0, -Math.cos(LON0)];
  const P = up.map(c => c * (R + ALT));
  const PP = P[0] * P[0] + P[1] * P[1] + P[2] * P[2];
  const bandOf = a => { for (let i = 0; i + 1 < BANDS.length; i++) if (a < BANDS[i + 1]) return i; return BANDS.length - 2; };
  // Sphere of radius rr along the ray: the NEARER intersection (the camera sits above the slab).
  function hitT(d, rr) {
    const b = P[0] * d[0] + P[1] * d[1] + P[2] * d[2];
    const disc = b * b - (PP - rr * rr);
    if (disc < 0) return -1;
    const t = -b - Math.sqrt(disc);
    return t > 0 ? t : -1;
  }

  console.log(`cloud-profile-trace: dump ${path.basename(path.resolve(DUMP))} (ground cell ${gI0}, ${gJ0}, flags ${FLAGS}: levels ${[0, 1, 2, 3, 4, 5].filter(levelValid).join(",") || "none"} valid, global ${globalValid ? "valid" : "NOT valid"})`);
  console.log(`  mask ${path.basename(MASK)} ${W}x${H}; camera lat ${(LAT0 * 180 / Math.PI).toFixed(3)} lon ${(LON0 * 180 / Math.PI).toFixed(3)} alt ${ALT} km look 0; cloud_res ${RES}: pix_ang ${PIX.toExponential(4)} rad (nadir foot ${(ALT - TOP) * PIX > 0 ? ((ALT - TOP) * PIX).toFixed(3) : "0"} km, lodb ${Math.log2(Math.max((ALT - TOP) * PIX, 1e-4)).toFixed(2)})`);
  console.log(`  type ${TC}: sigma ${SIGMA.toFixed(2)} per km, band h ${reg.h_lo.toFixed(3)}..${reg.h_hi.toFixed(3)} (${((reg.h_hi - reg.h_lo) * slab_km).toFixed(2)} km), family ${archIndex(TC) < 0 ? "thin" : ["humilis", "congestus", "stratocumulus", "cumulonimbus"][archIndex(TC)]}: L_h ${L_h.toFixed(3)} km, L_v ${L_v.toFixed(3)} km, dz ${dz.toFixed(3)} km, l_v_eff ${l_v_eff.toFixed(3)}; law ${LAW}: l_h ${l_h_eff.toFixed(3)} km (ratio ${(l_h_eff / L_h).toFixed(2)}); knob ${KNOB}; ${STEPS} steps per bin, pixel stride ${SUB}`);

  // ── The trace, per orientation ──
  const orients = ORIENT === "auto" ? [[1, -1], [1, 1], [-1, -1], [-1, 1]] : [[1, -1]];
  const results = [];
  const x0 = Math.floor(W * (1 - CROP) / 2), x1 = Math.ceil(W * (1 + CROP) / 2);
  const y0 = Math.floor(H * (1 - CROP) / 2), y1 = Math.ceil(H * (1 + CROP) / 2);
  for (const [sx, sy] of orients) {
    const nb = BANDS.length - 1;
    const acc = Array.from({ length: nb }, () => ({ n: 0, meas: 0, measHit: 0, pred: 0, predHit: 0, f: 0, lvl: 0, hits: 0, band: 0 }));
    let sxy = 0, sxx = 0, syy = 0, sx1 = 0, sy1 = 0, n = 0, inBand = 0, samples = 0;
    for (let y = y0; y < y1; y += SUB) for (let x = x0; x < x1; x += SUB) {
      const ddx = (x - CX) / FPX, ddy = (y - CY) / FPX;
      const d = [0, 1, 2].map(i => sx * ddx * east[i] + sy * ddy * north[i] - up[i]);
      const dl = Math.hypot(d[0], d[1], d[2]);
      for (let i = 0; i < 3; i++) d[i] /= dl;
      const theta = Math.atan(Math.hypot(ddx, ddy)) * 180 / Math.PI;
      // March the slab top-down through STEPS sub-slabs per bin; the
      // segment is the exact chord between the bounding shells.
      let logT = 0, fw = 0, lw = 0, hits = 0;
      let t_above = hitT(d, R + TOP);
      for (let k = NZ - 1; k >= 0 && t_above > 0; k--) for (let s = STEPS - 1; s >= 0 && t_above > 0; s--) {
        const hz_lo = k + s / STEPS, hz_mid = k + (s + 0.5) / STEPS;
        const t_below = hitT(d, R + BASE + hz_lo * dz);
        if (t_below <= 0) { t_above = -1; break; }
        const seg = t_below - t_above;
        const tm = hitT(d, R + BASE + hz_mid * dz);
        t_above = t_below;
        if (tm <= 0 || seg <= 0) continue;
        const Q = [P[0] + tm * d[0], P[1] + tm * d[1], P[2] + tm * d[2]];
        const ql = Math.hypot(Q[0], Q[1], Q[2]);
        const dirp = [Q[0] / ql, Q[1] / ql, Q[2] / ql];
        const lat = Math.asin(clamp(dirp[1], -1, 1)), lon = Math.atan2(-dirp[2], dirp[0]);
        // E1 lever 1: the hand-off reads the PIXEL footprint slant * pix_ang.
        const lodb_pix = Math.log2(Math.max(tm * PIX, 1e-4));
        const lodf = lodb_pix + JITTER * 0.35;
        const r = tap(lon, lat, hz_mid / NZ, lodb_pix);
        let w = 0;
        if (r.ok) w = knobForced >= 0 ? 1 : (knobHard ? (lodf >= -1 ? 1 : 0) : smoothstep(LOD_LO, LOD_HI, lodf));
        samples++;
        if (w > 0 && w < 1 - 1e-4) inBand++;
        if (w <= 0 || r.f <= 0) continue;
        const c_v = Math.abs(dirp[0] * d[0] + dirp[1] * d[1] + dirp[2] * d[2]);
        const D_in = clamp(r.G / Math.max(r.f, F_EPS), 0, 1);
        logT += Math.log(Math.max(tPf(w, r.f, D_in, c_v, seg), 1e-30));
        fw += r.f; lw += r.level; hits++;
      }
      const pred = 1 - Math.exp(logT);
      const o = (y * W + x) * mask.C;
      const lumByte = mask.C >= 3 ? 0.2126 * mask.d[o] + 0.7152 * mask.d[o + 1] + 0.0722 * mask.d[o + 2] : mask.d[o];
      const meas = srgbDecode(lumByte);
      const a = acc[bandOf(theta)];
      a.n++; a.meas += meas; if (lumByte > 128) a.measHit++;
      a.pred += pred; if (pred > T_LINEAR) a.predHit++;
      if (hits) { a.f += fw / hits; a.lvl += lw / hits; a.hits++; }
      sxy += pred * meas; sxx += pred * pred; syy += meas * meas; sx1 += pred; sy1 += meas; n++;
    }
    const corr = (n * sxy - sx1 * sy1) / Math.sqrt(Math.max((n * sxx - sx1 * sx1) * (n * syy - sy1 * sy1), 1e-30));
    results.push({ sx, sy, corr, acc, inBand, samples });
  }
  const label = ([sx, sy]) => `${sx > 0 ? "east" : "west"} right, ${sy < 0 ? "north" : "south"} up`;
  console.log("  orientation correlation (predicted vs measured alpha, per pixel, crop " + CROP + "): " + results.map(r => `${label([r.sx, r.sy])}: ${r.corr.toFixed(3)}`).join("; "));
  results.sort((a, b) => b.corr - a.corr);
  const best = results[0];
  if (best.inBand > 0) {
    console.log(`  NOTE: ${(100 * best.inBand / best.samples).toFixed(1)} percent of the samples sat in the hand-off band (0 < w < 1): the tracer predicts the PROFILE share only, the marched share is not modelled there.`);
  }
  console.log("");
  console.log("  band    " + "n".padStart(7) + "  MEASURED mean / mask>0.216 | PREDICTED mean / mask>0.216 | residual (meas - pred) | f seen | level");
  const total = { n: 0, meas: 0, measHit: 0, pred: 0, predHit: 0, f: 0, lvl: 0, hits: 0 };
  const row = (name, a) => {
    const mm = a.meas / a.n, pm = a.pred / a.n, res = mm - pm;
    return `  ${name.padEnd(7)}${String(a.n).padStart(7)}   ${mm.toFixed(3)} / ${(100 * a.measHit / a.n).toFixed(1).padStart(5)}%           |  ${pm.toFixed(3)} / ${(100 * a.predHit / a.n).toFixed(1).padStart(5)}%          |  ${(res >= 0 ? "+" : "") + res.toFixed(3)} (${pm > 1e-6 ? ((res >= 0 ? "+" : "") + (100 * res / pm).toFixed(0) + "%") : "n/a"})        | ${a.hits ? (a.f / a.hits).toFixed(3) : "  -  "}  | ${a.hits ? (a.lvl / a.hits).toFixed(2) : "-"}`;
  };
  for (let b = 0; b + 1 < BANDS.length; b++) {
    const a = best.acc[b];
    if (!a.n) continue;
    for (const k of Object.keys(total)) total[k] += a[k];
    console.log(row(`${BANDS[b]}-${BANDS[b + 1]}`, a));
  }
  console.log(row("crop", total));
  console.log(`  (mask fraction = pixels above byte 128 = linear ${T_LINEAR.toFixed(3)}; means are LINEAR alpha, the capture sRGB-decoded; residual = measured minus predicted, reported, not a pass bar)`);
})().catch(e => { console.error(e); process.exit(1); });
