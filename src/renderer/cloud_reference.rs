//! The converged CPU reference march - the LIGHTING ARBITER (environment
//! program increment 8, G4).
//!
//! Phase 9 died because nobody knew what the converged answer WAS: an
//! integrator change improved the speckle metric while the look regressed,
//! and there was no ground truth to say which build was closer to correct.
//! This module is that ground truth: a brute-force march of the SAME cloud
//! field the GPU renders - the same baked noise volumes (`cloud_noise`
//! generates them for both), the same carve/erosion/weather math, the same
//! Wrenninge octave ladder and two-tone ambient - integrated at metre-scale
//! steps with no jitter, no early-outs, no band-limiting, and a fine sun
//! march instead of the 8-tap ladder. Every integrator/lighting increment
//! from 10 on is judged against per-ray radiances from this march.
//!
//! Deliberate divergences from `cloud_march_core` (each is the removal of an
//! APPROXIMATION, or an input this offline path cannot reach):
//! - View march: fixed `step_km` steps (default 1 m), midpoint sampling, no
//!   exponential spacing, no jitter, transmittance floor 1e-4 (not 0.02).
//! - Sun march: fixed fine steps to slab exit (tau > 40 early-out kept -
//!   that is exp(-40), numerically exact), using the FULLY ERODED density
//!   everywhere. The GPU's 8-tap geometric ladder and its body-only far
//!   taps are approximations of this.
//! - All texture taps at mip 0: the mip ladder band-limits toward what a
//!   converged march of the full-frequency field looks like under a pixel
//!   footprint; the reference IS that target, evaluated on rays.
//! - No aerial perspective and no limb fade (they depend on the atmosphere
//!   LUT and fragment geometry): reference radiances are PRE-aerial,
//!   ACES-mapped. Compare at near vantages (silverdale-flight, clouds
//!   2-10 km away) where aerial transmittance is ~1.
//! - Weather: the PINNED path only (showcase cloud_cover/cloud_type pins,
//!   bypass = 1, pure five-octave procedural field) - the canonical
//!   verification vantages all pin, and the offline path has no MODIS
//!   texture. NOTE: the older `clouds::cloud_weather` Rust mirror predates
//!   the five-octave split and is NOT this field.
//!
//! Constant values transcribed here are locked to the WGSL source by
//! `wgsl_reference_constants_stay_in_sync` below, same discipline as
//! `clouds::wgsl_cloud_constants_stay_in_sync`.

use super::clouds::{
    cloud_alpha_from_field, cloud_hg, cloud_noise, cloud_regime, cloud_rot_x, cloud_rot_y,
    cloud_scatter_energy, CloudRegime, CLOUD_AMB_BASE, CLOUD_AMB_BOUNCE, CLOUD_AMB_TOP,
    CLOUD_BAND_STRETCH, CLOUD_DRIFT_CROSS, CLOUD_FIELD_HI, CLOUD_FIELD_LO, CLOUD_NIGHT_FLOOR,
    CLOUD_POWDER_STRENGTH,
};

// ── Constants transcribed from assets/shaders/pbr/40-clouds.wgsl ──
// (locked by the sync test at the bottom of this file)
const CLOUD_COV_LO: f32 = 0.92;
const CLOUD_COV_HI: f32 = 0.52;
const CLOUD_TOP_RISE: f32 = 0.45;
const CLOUD_BASE_DROP: f32 = 0.35;
const CLOUD_CELL_SPLIT: f32 = 0.5;
const CLOUD_FRAY_ERODE: f32 = 0.5;
const CLOUD_FIL_LO: f32 = 0.30;
const CLOUD_FIL_HI: f32 = 0.74;
const CLOUD_DETAIL_ERODE: f32 = 0.38;
const CLOUD_PUFF_ERODE: f32 = 0.38;
const CLOUD_PUFF_AO: f32 = 0.60;
const CLOUD_DENSITY_POW: f32 = 1.7;
const CLOUD_HI_MAX_ALPHA: f32 = 0.96;
const CLOUD_SHAPE_TILE_KM: f32 = 267.6;
const CLOUD_DETAIL_TILE_KM: f32 = 107.0;
const CLOUD_PUFF_TILE_KM: f32 = 45.9;
const CLOUD_FRAY_TILE_KM: f32 = 713.6;
const CLOUD_CELL_TILE_KM: f32 = 8.0;
const CLOUD_DETAIL_FADE_NEAR_KM: f32 = 192.7;
const CLOUD_DETAIL_FADE_FAR_KM: f32 = 4495.0;
const CLOUD_PUFF_FADE_NEAR_KM: f32 = 51.4;
const CLOUD_PUFF_FADE_FAR_KM: f32 = 289.0;
const CLOUD_CELL_FADE_NEAR_KM: f32 = 30.0;
const CLOUD_CELL_FADE_FAR_KM: f32 = 60.0;

// ── Small math mirrors (WGSL semantics) ──

fn clampf(x: f32, lo: f32, hi: f32) -> f32 {
    x.max(lo).min(hi)
}

fn mixf(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = clampf((x - e0) / (e1 - e0), 0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn remap(v: f32, l0: f32, h0: f32, l1: f32, h1: f32) -> f32 {
    l1 + (v - l0) * (h1 - l1) / (h0 - l0)
}

fn v3_len(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn v3_norm(v: [f32; 3]) -> [f32; 3] {
    let l = v3_len(v).max(1e-12);
    [v[0] / l, v[1] / l, v[2] / l]
}

fn v3_dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn v3_add_scaled(a: [f32; 3], b: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] + b[0] * s, a[1] + b[1] * s, a[2] + b[2] * s]
}

/// Mirrors `cloud_height_band` (private in the WGSL; the clouds.rs mirror
/// exists too, re-declared here so this module stays freestanding).
fn height_band(h: f32, h_lo: f32, h_hi: f32) -> f32 {
    let a = mixf(h_lo, h_hi, 0.03);
    let b = mixf(h_lo, h_hi, 0.62);
    smoothstep(h_lo, a, h) * (1.0 - smoothstep(b, h_hi, h))
}

/// Mirrors `cloud_stretch_domain`. cross((0,1,0), dir) = (dir.z, 0, -dir.x).
fn stretch_domain(p: [f32; 3], dir: [f32; 3], stretch: f32) -> [f32; 3] {
    let tang = [dir[2], 0.0, -dir[0]];
    let tl = v3_len(tang);
    if tl < 1.0e-4 {
        return p;
    }
    let tang = [tang[0] / tl, tang[1] / tl, tang[2] / tl];
    let k = v3_dot(p, tang) * (1.0 - 1.0 / stretch);
    [p[0] - tang[0] * k, p[1] - tang[1] * k, p[2] - tang[2] * k]
}

// ── The baked noise volumes, sampled like the GPU sampler ──
// (repeat wrap, trilinear filter, mip 0, u8-normalized RGBA)

pub struct RefVolume<'a> {
    pub data: &'a [u8],
    pub size: u32,
}

impl<'a> RefVolume<'a> {
    /// Trilinear repeat-wrapped sample at texture coordinate `uvw` (one full
    /// tile per unit), returning normalized RGBA. Matches wgpu's linear
    /// sampler convention: texel centres at (i + 0.5) / size.
    pub fn sample(&self, uvw: [f32; 3]) -> [f32; 4] {
        let n = self.size as i64;
        let nf = self.size as f32;
        let mut i0 = [0i64; 3];
        let mut f = [0.0f32; 3];
        for a in 0..3 {
            // Texel-space coordinate with the half-texel centre offset.
            let x = uvw[a].rem_euclid(1.0) * nf - 0.5;
            let fl = x.floor();
            i0[a] = fl as i64;
            f[a] = x - fl;
        }
        let mut out = [0.0f32; 4];
        for dz in 0..2i64 {
            for dy in 0..2i64 {
                for dx in 0..2i64 {
                    let ix = (i0[0] + dx).rem_euclid(n) as usize;
                    let iy = (i0[1] + dy).rem_euclid(n) as usize;
                    let iz = (i0[2] + dz).rem_euclid(n) as usize;
                    let w = (if dx == 1 { f[0] } else { 1.0 - f[0] })
                        * (if dy == 1 { f[1] } else { 1.0 - f[1] })
                        * (if dz == 1 { f[2] } else { 1.0 - f[2] });
                    let o = ((iz * self.size as usize + iy) * self.size as usize + ix) * 4;
                    for ch in 0..4 {
                        out[ch] += w * (self.data[o + ch] as f32 / 255.0);
                    }
                }
            }
        }
        out
    }
}

/// Everything the reference march needs about one scene. Positions are in
/// PLANET-RADIUS units (surface at r = 1), which sidesteps the drawn-shell
/// scaling entirely - only ratios enter the math.
pub struct CloudRefCtx<'a> {
    pub shape: RefVolume<'a>,
    pub detail: RefVolume<'a>,
    /// Cloud clock (the app-start-relative seconds the shader gets in
    /// sun_color.w).
    pub t: f32,
    /// Planet cloud seed (material.params.x).
    pub seed: f32,
    /// Coverage (material.base_color.a) - the pinned dev value.
    pub coverage: f32,
    /// The pinned cloud-type coordinate (showcase cloud_type). The canonical
    /// verification vantages always pin; the natural type field would deal
    /// the capture site an unknown family.
    pub type_pin: f32,
    /// Slab bounds as planet-radius multiples (planet.cloud_slab_scales()).
    pub rb: f32,
    pub rt: f32,
    /// Planet units per km (1 / radius_km).
    pub upkm: f32,
    /// Sun direction in planet-local frame, unit.
    pub sun_local: [f32; 3],
    /// Sun colour * intensity (camera.sun_color.rgb * sun_direction.w).
    pub sun_energy: [f32; 3],
    /// The aerial sky hue the two-tone ambient reads
    /// (camera.light2_cone_inner.yzw).
    pub sky_aer: [f32; 3],
    /// Cloud material tint (material.base_color.rgb; 1,1,1 in practice).
    pub tint: [f32; 3],
    /// View-march step in km (0.001 = 1 m; the converged default).
    pub step_km: f32,
    /// Sun-march step in km.
    pub sun_step_km: f32,
}

struct CarveOut {
    carve: f32,
    ps: [f32; 3],
    h: f32,
    crown: f32,
}

impl<'a> CloudRefCtx<'a> {
    fn wind_omega(&self, mps: f32) -> f32 {
        // material.params2.z = planet radius km = 1 / upkm (always > 0.5 for
        // any real planet, so the legacy CLOUD_DRIFT_ZONAL branch never
        // applies here).
        mps * self.upkm / 1000.0
    }

    /// The five-octave PINNED weather field (cloud_weather_adv with
    /// bypass = 1: pure procedural, no MODIS term).
    fn weather_pinned(&self, dir: [f32; 3], drift_ang: f32) -> f32 {
        let da0 = cloud_rot_y(dir, drift_ang);
        let da = v3_norm([da0[0], da0[1] * CLOUD_BAND_STRETCH, da0[2]]);
        let db = cloud_rot_x(dir, self.t * CLOUD_DRIFT_CROSS);
        let macro_f = 0.40 * cloud_noise(da, 5.0, self.seed)
            + 0.24 * cloud_noise(da, 13.0, self.seed + 19.0);
        let meso_f = 0.20 * cloud_noise(db, 7.0, self.seed + 101.0)
            + 0.12 * cloud_noise(da, 31.0, self.seed + 233.0)
            + 0.08 * cloud_noise(db, 67.0, self.seed + 409.0);
        smoothstep(CLOUD_FIELD_LO, CLOUD_FIELD_HI, (macro_f + meso_f) / 1.04)
    }

    /// Mirrors `cloud_carve` (High tier - no clouds-v2 branch).
    fn carve(&self, p: [f32; 3], wa: f32, reg: &CloudRegime, cell_amt: f32) -> CarveOut {
        let r = v3_len(p);
        let h = clampf((r - self.rb) / (self.rt - self.rb), 0.0, 1.0);
        let h_hi_max = (reg.h_hi + 0.8 * (reg.h_hi - reg.h_lo)).min(1.0);
        if height_band(h, reg.h_lo, h_hi_max) <= 0.002 || wa <= 0.003 {
            return CarveOut { carve: 0.0, ps: p, h, crown: 0.0 };
        }
        let omega_c = self.wind_omega(mixf(reg.wind_lo, reg.wind_hi, h));
        let ps0 = cloud_rot_y(p, self.t * omega_c);
        let ps = stretch_domain(ps0, v3_norm(p), reg.stretch);
        let shape_freq = 1.0 / (CLOUD_SHAPE_TILE_KM * self.upkm);
        let s = self
            .shape
            .sample([ps[0] * shape_freq, ps[1] * shape_freq, ps[2] * shape_freq]);
        let lofi = s[1] * 0.625 + s[2] * 0.25 + s[3] * 0.125;
        let body = clampf(remap(s[0], lofi - 1.0, 1.0, 0.0, 1.0), 0.0, 1.0);
        let tower = smoothstep(0.62, 0.92, lofi);
        let h_hi_eff = (reg.h_hi + tower * 0.8 * (reg.h_hi - reg.h_lo)).min(1.0);
        let env = height_band(h, reg.h_lo, h_hi_eff);
        if env <= 0.002 {
            return CarveOut { carve: 0.0, ps, h, crown: 0.0 };
        }
        let u_band = clampf((h - reg.h_lo) / (h_hi_eff - reg.h_lo).max(1.0e-4), 0.0, 1.0);
        let thr_base = mixf(CLOUD_COV_LO, CLOUD_COV_HI, wa);
        let v_band = 1.0 - u_band;
        let mut thr = thr_base
            + CLOUD_TOP_RISE * u_band * u_band
            + CLOUD_BASE_DROP * reg.base_drop * v_band * v_band * (1.0 - lofi);
        if cell_amt > 0.01 {
            let cell_freq = 1.0 / (CLOUD_CELL_TILE_KM * self.upkm);
            let c = self
                .shape
                .sample([ps[0] * cell_freq, ps[1] * cell_freq, ps[2] * cell_freq]);
            thr += CLOUD_CELL_SPLIT * cell_amt * reg.fine * (1.0 - c[1]);
        }
        let carve = clampf((body - thr) / (1.0 - thr).max(1.0e-3), 0.0, 1.0) * env;
        let u_crown = ((body - thr_base).max(0.0) / CLOUD_TOP_RISE).sqrt();
        let crown = clampf(u_band / clampf(u_crown, 1.0e-3, 1.0), 0.0, 1.0);
        CarveOut { carve, ps, h, crown }
    }

    /// Mirrors `cloud_density_hi`: (density, puff cavity, crown proximity).
    fn density_hi(
        &self,
        p: [f32; 3],
        wa: f32,
        reg: &CloudRegime,
        detail_amt: f32,
        puff_amt: f32,
        cell_amt: f32,
    ) -> [f32; 3] {
        let cs = self.carve(p, wa, reg, cell_amt);
        let mut base = cs.carve;
        if base <= 0.003 {
            return [0.0, 0.0, 0.0];
        }
        let fray_freq = 1.0 / (CLOUD_FRAY_TILE_KM * self.upkm);
        let fr = self
            .detail
            .sample([cs.ps[0] * fray_freq, cs.ps[1] * fray_freq, cs.ps[2] * fray_freq]);
        let frfbm = fr[0] * 0.625 + fr[1] * 0.25 + fr[2] * 0.125;
        let erode_c = frfbm * reg.fray * CLOUD_FRAY_ERODE * (0.35 + 0.65 * (1.0 - base));
        base = clampf(remap(base, erode_c, 1.0, 0.0, 1.0), 0.0, 1.0);
        let fmask = smoothstep(CLOUD_FIL_LO, CLOUD_FIL_HI, fr[3]);
        base *= mixf(1.0, fmask, reg.filament);
        if base <= 0.003 {
            return [0.0, 0.0, 0.0];
        }
        // Both near bands sample the drifted-but-UNSTRETCHED domain.
        let pu0 = cloud_rot_y(
            p,
            self.t * self.wind_omega(mixf(reg.wind_lo, reg.wind_hi, cs.h)),
        );
        if detail_amt > 0.01 {
            let detail_freq = 1.0 / (CLOUD_DETAIL_TILE_KM * self.upkm);
            let d = self.detail.sample([
                pu0[0] * detail_freq,
                pu0[1] * detail_freq,
                pu0[2] * detail_freq,
            ]);
            let dfbm = d[0] * 0.625 + d[1] * 0.25 + d[2] * 0.125;
            let dmod = mixf(dfbm, 1.0 - dfbm, clampf(cs.h * 3.0, 0.0, 1.0))
                * CLOUD_DETAIL_ERODE
                * reg.fine
                * detail_amt
                * (0.60 + 0.90 * cs.crown)
                * (0.35 + 0.65 * (1.0 - base));
            base = clampf(remap(base, dmod, 1.0, 0.0, 1.0), 0.0, 1.0);
        }
        let mut cavity = 0.0;
        if puff_amt > 0.01 && base > 0.003 {
            let puff_freq = 1.0 / (CLOUD_PUFF_TILE_KM * self.upkm);
            let pu = self
                .detail
                .sample([pu0[0] * puff_freq, pu0[1] * puff_freq, pu0[2] * puff_freq]);
            let pufbm = pu[0] * 0.625 + pu[1] * 0.25 + pu[2] * 0.125;
            let phased = mixf(pufbm, 1.0 - pufbm, clampf(cs.h * 3.0, 0.0, 1.0));
            let pmod = phased * CLOUD_PUFF_ERODE * reg.fine * puff_amt * (0.30 + 0.70 * (1.0 - base));
            base = clampf(remap(base, pmod, 1.0, 0.0, 1.0), 0.0, 1.0);
            cavity = clampf(phased * reg.fine, 0.0, 1.0) * puff_amt;
        }
        let dens_n = clampf(base / cs.carve.max(1.0e-3), 0.0, 1.0);
        let skirt = smoothstep(0.0, 0.12, cs.carve);
        [dens_n.powf(CLOUD_DENSITY_POW) * skirt, cavity, cs.crown]
    }

    /// The CONVERGED optical depth toward the sun: fine fixed steps on the
    /// fully eroded density, to slab exit (or tau 40 - exp(-40) is exact
    /// zero at f32). The GPU's 8-tap geometric ladder approximates this.
    fn sun_tau(
        &self,
        p: [f32; 3],
        wa_at: impl Fn(&Self, [f32; 3]) -> f32,
        reg: &CloudRegime,
        detail_amt: f32,
        puff_amt: f32,
        cell_amt: f32,
    ) -> f32 {
        let sigma = reg.ext_km / self.upkm;
        let step = self.sun_step_km * self.upkm;
        let mut tau = 0.0;
        let mut lp = p;
        // Hard cap: slab crossings are < 2 * chord of the shell; 400 km of
        // marching covers any slant exit at 1-10 m steps without a runaway.
        let max_steps = ((400.0 / self.sun_step_km) as usize).max(16);
        for _ in 0..max_steps {
            lp = v3_add_scaled(lp, self.sun_local, step);
            let r = v3_len(lp);
            if r > self.rt || r < 1.0 {
                break; // left the slab top, or the ground blocks the path
            }
            if r >= self.rb {
                let wa = wa_at(self, lp);
                let dens = self.density_hi(lp, wa, reg, detail_amt, puff_amt, cell_amt)[0];
                tau += sigma * dens * step;
                if tau > 40.0 {
                    break;
                }
            }
        }
        tau
    }

    /// The converged march: `cloud_march_core` with every integrator
    /// approximation removed. Returns (ACES-mapped rgb, alpha), PRE-aerial.
    pub fn reference_radiance(&self, ro: [f32; 3], rd: [f32; 3]) -> ([f32; 3], f32) {
        let rd = v3_norm(rd);
        // Slab interval (identical geometry to the GPU march).
        let tca = -v3_dot(ro, rd);
        let perp = v3_add_scaled(ro, rd, tca);
        let d2 = v3_dot(perp, perp);
        if d2 >= self.rt * self.rt {
            return ([0.0; 3], 0.0);
        }
        let thc_t = (self.rt * self.rt - d2).sqrt();
        let mut m0 = (tca - thc_t).max(0.0);
        let mut m1 = tca + thc_t;
        if m1 <= 0.0 {
            return ([0.0; 3], 0.0);
        }
        if d2 < self.rb * self.rb {
            let thc_b = (self.rb * self.rb - d2).sqrt();
            let b0 = tca - thc_b;
            let b1 = tca + thc_b;
            if b0 > m0 {
                m1 = m1.min(b0);
            } else if b1 > m0 {
                m0 = b1;
            }
        }
        if m1 <= m0 {
            return ([0.0; 3], 0.0);
        }
        // Ground occlusion (r_surf = 1 in planet units).
        if d2 < 1.0 {
            let t_surf = tca - (1.0 - d2).sqrt();
            if t_surf > 0.0 && t_surf < m0 {
                return ([0.0; 3], 0.0);
            }
        }

        let seg = m1 - m0;
        // The GPU samples the type coordinate at the segment midpoint; the
        // canonical vantages PIN the type, so the regime is direct here.
        let reg = cloud_regime(self.type_pin);
        let wind_ang = self.t * self.wind_omega(reg.wind_lo);
        let wa_at = move |me: &Self, p: [f32; 3]| -> f32 {
            clampf(
                cloud_alpha_from_field(me.weather_pinned(v3_norm(p), wind_ang), me.coverage)
                    + reg.cover_bias,
                0.0,
                1.0,
            )
        };

        let cos_vs = v3_dot(rd, self.sun_local);
        let powder_gate = smoothstep(0.3, 0.9, cos_vs);
        let sigma_v = reg.ext_km / self.upkm;
        let sun_lum = self.sun_energy[0] * 0.2126
            + self.sun_energy[1] * 0.7152
            + self.sun_energy[2] * 0.0722;
        let sky_peak = self.sky_aer[0]
            .max(self.sky_aer[1])
            .max(self.sky_aer[2])
            .max(1.0e-4);

        let dt = self.step_km * self.upkm;
        let n = ((seg / dt).ceil() as usize).max(1);
        let mut trans: f64 = 1.0;
        let mut acc = [0.0f64; 3];
        let mut acc_w: f64 = 0.0;
        for i in 0..n {
            let tm = m0 + (i as f32 + 0.5) * dt;
            if tm >= m1 {
                break;
            }
            let p = v3_add_scaled(ro, rd, tm);
            let dirp = v3_norm(p);
            let weather_a = wa_at(self, p);
            let detail_amt = 1.0
                - smoothstep(
                    CLOUD_DETAIL_FADE_NEAR_KM * self.upkm,
                    CLOUD_DETAIL_FADE_FAR_KM * self.upkm,
                    tm,
                );
            let puff_amt = 1.0
                - smoothstep(
                    CLOUD_PUFF_FADE_NEAR_KM * self.upkm,
                    CLOUD_PUFF_FADE_FAR_KM * self.upkm,
                    tm,
                );
            let cell_amt = 1.0
                - smoothstep(
                    CLOUD_CELL_FADE_NEAR_KM * self.upkm,
                    CLOUD_CELL_FADE_FAR_KM * self.upkm,
                    tm,
                );
            let dc = self.density_hi(p, weather_a, &reg, detail_amt, puff_amt, cell_amt);
            let dens = dc[0];
            if dens <= 0.001 {
                continue;
            }
            let a_i = 1.0 - (-sigma_v * dens * dt).exp();

            let ndl = v3_dot(dirp, self.sun_local);
            let day = smoothstep(-0.05, 0.3, ndl);

            let tau = self.sun_tau(p, &wa_at, &reg, detail_amt, puff_amt, cell_amt);
            let powder = 1.0 - CLOUD_POWDER_STRENGTH * (-2.0 * tau).exp();
            let pw = mixf(powder, 1.0, powder_gate);
            let direct = cloud_scatter_energy(tau, cos_vs) * pw;

            let h = clampf((v3_len(p) - self.rb) / (self.rt - self.rb), 0.0, 1.0);
            let amb_h = mixf(CLOUD_AMB_BASE, CLOUD_AMB_TOP, h) * (0.35 + 0.65 * (-tau * 0.12).exp());
            let amb_col = [
                (self.sky_aer[0] / sky_peak) * amb_h + 1.0 * (CLOUD_AMB_BOUNCE * (1.0 - h)),
                (self.sky_aer[1] / sky_peak) * amb_h + 0.93 * (CLOUD_AMB_BOUNCE * (1.0 - h)),
                (self.sky_aer[2] / sky_peak) * amb_h + 0.82 * (CLOUD_AMB_BOUNCE * (1.0 - h)),
            ];
            let crown_floor = mixf(0.88, 0.70, reg.opacity);
            let crown_shade = mixf(crown_floor, 1.12, dc[2]);
            let ao = (1.0 - CLOUD_PUFF_AO * dc[1]) * crown_shade;
            let direct_lit = direct * mixf(1.0, clampf(ao, 0.0, 1.0), 0.5);

            let w = trans * a_i as f64;
            for ch in 0..3 {
                let c_i = self.tint[ch]
                    * (self.sun_energy[ch] * (direct_lit * day)
                        + amb_col[ch] * (sun_lum * ao * day)
                        + CLOUD_NIGHT_FLOOR);
                acc[ch] += c_i as f64 * w;
            }
            acc_w += w;
            trans *= 1.0 - a_i as f64;
            if trans <= 1.0e-4 {
                break;
            }
        }
        let body_total = (1.0 - trans) as f32;
        if body_total <= 0.003 {
            return ([0.0; 3], 0.0);
        }
        let mut radiance = [
            (acc[0] / acc_w.max(1.0e-9)) as f32,
            (acc[1] / acc_w.max(1.0e-9)) as f32,
            (acc[2] / acc_w.max(1.0e-9)) as f32,
        ];
        for ch in 0..3 {
            radiance[ch] *= reg.tint;
            radiance[ch] *= 1.0 - 0.32 * smoothstep(0.72, 0.98, body_total);
        }
        // Same ACES curve as the pipeline (aerial deliberately skipped -
        // see the module docs).
        let mut mapped = [0.0f32; 3];
        for ch in 0..3 {
            let x = radiance[ch];
            mapped[ch] = clampf(
                (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14),
                0.0,
                1.0,
            );
        }
        (mapped, body_total * CLOUD_HI_MAX_ALPHA)
    }
}

/// A dual-lobe HG sanity anchor used by the bimodality test: forward-scatter
/// rays (toward the sun) must carry more single-scatter energy than
/// back-scatter rays at the same tau. Kept here (not in the test) so the
/// reference and its acceptance share one definition.
pub fn phase_anchor(cos_vs: f32) -> f32 {
    0.3 * cloud_hg(cos_vs, -0.15) + 0.7 * cloud_hg(cos_vs, 0.80)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::cloud_noise;

    fn shader_source() -> String {
        let root = env!("CARGO_MANIFEST_DIR");
        std::fs::read_to_string(format!("{root}/assets/shaders/pbr/40-clouds.wgsl"))
            .expect("read 40-clouds.wgsl")
    }

    fn wgsl_const(src: &str, name: &str) -> f32 {
        for line in src.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix(&format!("const {name}: f32 = ")) {
                let v = rest.trim_end_matches(';').trim();
                return v.parse::<f32>().unwrap_or_else(|_| panic!("parse {name} = {v}"));
            }
        }
        panic!("const {name} not found in 40-clouds.wgsl");
    }

    /// Every constant transcribed into this module must match the shader
    /// source byte-for-value - same discipline as clouds.rs's sync test.
    #[test]
    fn wgsl_reference_constants_stay_in_sync() {
        let src = shader_source();
        let pairs: [(&str, f32); 24] = [
            ("CLOUD_COV_LO", CLOUD_COV_LO),
            ("CLOUD_COV_HI", CLOUD_COV_HI),
            ("CLOUD_TOP_RISE", CLOUD_TOP_RISE),
            ("CLOUD_BASE_DROP", CLOUD_BASE_DROP),
            ("CLOUD_CELL_SPLIT", CLOUD_CELL_SPLIT),
            ("CLOUD_FRAY_ERODE", CLOUD_FRAY_ERODE),
            ("CLOUD_FIL_LO", CLOUD_FIL_LO),
            ("CLOUD_FIL_HI", CLOUD_FIL_HI),
            ("CLOUD_DETAIL_ERODE", CLOUD_DETAIL_ERODE),
            ("CLOUD_PUFF_ERODE", CLOUD_PUFF_ERODE),
            ("CLOUD_PUFF_AO", CLOUD_PUFF_AO),
            ("CLOUD_DENSITY_POW", CLOUD_DENSITY_POW),
            ("CLOUD_HI_MAX_ALPHA", CLOUD_HI_MAX_ALPHA),
            ("CLOUD_SHAPE_TILE_KM", CLOUD_SHAPE_TILE_KM),
            ("CLOUD_DETAIL_TILE_KM", CLOUD_DETAIL_TILE_KM),
            ("CLOUD_PUFF_TILE_KM", CLOUD_PUFF_TILE_KM),
            ("CLOUD_FRAY_TILE_KM", CLOUD_FRAY_TILE_KM),
            ("CLOUD_CELL_TILE_KM", CLOUD_CELL_TILE_KM),
            ("CLOUD_DETAIL_FADE_NEAR_KM", CLOUD_DETAIL_FADE_NEAR_KM),
            ("CLOUD_DETAIL_FADE_FAR_KM", CLOUD_DETAIL_FADE_FAR_KM),
            ("CLOUD_PUFF_FADE_NEAR_KM", CLOUD_PUFF_FADE_NEAR_KM),
            ("CLOUD_PUFF_FADE_FAR_KM", CLOUD_PUFF_FADE_FAR_KM),
            ("CLOUD_CELL_FADE_NEAR_KM", CLOUD_CELL_FADE_NEAR_KM),
            ("CLOUD_CELL_FADE_FAR_KM", CLOUD_CELL_FADE_FAR_KM),
        ];
        for (name, rust_v) in pairs {
            let wgsl_v = wgsl_const(&src, name);
            assert!(
                (wgsl_v - rust_v).abs() < 1.0e-6,
                "{name}: WGSL {wgsl_v} != Rust {rust_v} - the reference no longer marches the shipped field"
            );
        }
        // The five-octave weather transcription: assert the WGSL still
        // carries the exact octave lines this module mirrors.
        for needle in [
            "0.40 * cloud_noise(da, 5.0, seed)",
            "0.24 * cloud_noise(da, 13.0, seed + 19.0)",
            "0.20 * cloud_noise(db, 7.0, seed + 101.0)",
            "0.12 * cloud_noise(da, 31.0, seed + 233.0)",
            "0.08 * cloud_noise(db, 67.0, seed + 409.0)",
            "(macro_f + meso_f) / 1.04",
        ] {
            assert!(
                src.contains(needle),
                "weather octave drifted: `{needle}` not in 40-clouds.wgsl - update weather_pinned"
            );
        }
    }

    #[test]
    fn trilinear_sampler_reproduces_texel_centres() {
        // A 4^3 volume with distinct voxel values: sampling at each texel
        // centre must return that voxel exactly.
        let n = 4u32;
        let mut data = vec![0u8; (n * n * n * 4) as usize];
        for i in 0..(n * n * n) as usize {
            data[i * 4] = (i * 3 % 251) as u8;
            data[i * 4 + 1] = (i * 7 % 251) as u8;
        }
        let vol = RefVolume { data: &data, size: n };
        for z in 0..n {
            for y in 0..n {
                for x in 0..n {
                    let uvw = [
                        (x as f32 + 0.5) / n as f32,
                        (y as f32 + 0.5) / n as f32,
                        (z as f32 + 0.5) / n as f32,
                    ];
                    let s = vol.sample(uvw);
                    let o = (((z * n + y) * n + x) * 4) as usize;
                    assert!((s[0] - data[o] as f32 / 255.0).abs() < 1e-4);
                    assert!((s[1] - data[o + 1] as f32 / 255.0).abs() < 1e-4);
                }
            }
        }
        // Repeat wrap: sampling one tile over must match.
        let a = vol.sample([0.125, 0.375, 0.625]);
        let b = vol.sample([1.125, 0.375, -0.375]);
        for ch in 0..4 {
            assert!((a[ch] - b[ch]).abs() < 1e-4, "repeat wrap broken");
        }
    }

    /// THE SANITY GATE (heavy: generates the real 192^3 + 128^3 volumes and
    /// marches 40 rays at 4 m view / 8 m sun steps - converged enough for a
    /// bimodality verdict; increment 10's per-ray TARGETS use 1-2 m in
    /// release on chosen rays). Run explicitly, IN RELEASE (debug is ~10x):
    ///   cargo test --release --features native --lib reference_radiance_is -- --ignored --nocapture
    /// Asserts the converged reference produces a lit/shaded BIMODAL sky at
    /// the canonical broken-cumulus setup - a monomodal answer would mean
    /// the reference lost either its shadows or its silver lining, and
    /// judging increment 10 against it would tune the wrong thing.
    #[test]
    #[ignore = "heavy (tens of seconds): generates the real noise volumes and marches 40 rays"]
    fn reference_radiance_is_deterministic_and_bimodal() {
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let shape = cloud_noise::generate_shape(threads);
        let detail = cloud_noise::generate_detail(threads);
        let r_km = 6371.0f32;
        let ctx = CloudRefCtx {
            shape: RefVolume { data: &shape, size: cloud_noise::SHAPE_SIZE },
            detail: RefVolume { data: &detail, size: cloud_noise::DETAIL_SIZE },
            t: 3600.0,
            seed: 0.37,
            coverage: 0.95,
            type_pin: 0.34,
            rb: 1.0 + 0.4 / r_km,
            rt: 1.0 + 12.0 / r_km,
            upkm: 1.0 / r_km,
            // Sun at ~55 degrees elevation, tilted east: the silverdale-noon
            // regime (lit tops + shaded bases both in view).
            sun_local: {
                let v = [0.4f32, 0.819, 0.4];
                let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                [v[0] / l, v[1] / l, v[2] / l]
            },
            sun_energy: [2.5, 2.45, 2.4],
            sky_aer: [0.35, 0.55, 0.95],
            tint: [1.0, 1.0, 1.0],
            step_km: 0.004,
            sun_step_km: 0.008,
        };
        // Camera 2 km up. Rays sweep 1.7-20 degrees above the horizon - the
        // elevations where the broken-cumulus field actually lives (probed
        // 2026-08-21: at type 0.34 the cumulus band tops out at ~7 km, so
        // steep rays exit it within the cell-split zone and see only the
        // sparse discrete cells; the GPU capture shows the same structure -
        // horizon-dense deck, scattered cells overhead. The joint gate's
        // ROI sits in the horizon-dense band for the same reason).
        let ro = [0.0, 1.0 + 2.0 / r_km, 0.0];
        let mut lums: Vec<f32> = Vec::new();
        let mut alphas = 0usize;
        for iaz in 0..6 {
            let az = iaz as f32 * 1.0471976; // 60 degrees apart
            for iel in 0..10 {
                let el = 0.025 + iel as f32 * 0.018; // 1.4 .. 10.7 degrees
                let rd = [el.cos() * az.cos(), el.sin(), el.cos() * az.sin()];
                let (rgb, a) = ctx.reference_radiance(ro, rd);
                assert!(rgb.iter().all(|v| v.is_finite()) && a.is_finite());
                assert!((0.0..=1.0).contains(&a), "alpha out of range: {a}");
                if a > 0.15 {
                    alphas += 1;
                    lums.push(rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722);
                }
                // Determinism: the same ray twice is bit-identical.
                let (rgb2, a2) = ctx.reference_radiance(ro, rd);
                assert_eq!(rgb, rgb2);
                assert_eq!(a, a2);
            }
        }
        assert!(
            alphas >= 8,
            "only {alphas} of 60 deck-band rays hit cloud at coverage 0.95 - the field mirror is broken"
        );
        lums.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p10 = lums[lums.len() / 10];
        let p90 = lums[lums.len() * 9 / 10];
        println!(
            "reference: {} cloud rays, lum p10 {:.3} p90 {:.3} ratio {:.2}",
            lums.len(),
            p10,
            p90,
            p90 / p10.max(1e-4)
        );
        assert!(
            p90 / p10.max(1e-4) >= 1.25,
            "lit/shaded spread p90/p10 = {:.2} - the reference reads monomodal (lost its shadows or its silver lining)",
            p90 / p10.max(1e-4)
        );
        assert!(p90 > 0.35, "brightest cloud rays are dark ({p90:.3}) - lighting energy lost");
    }
}

#[cfg(test)]
mod debug_probe {
    use super::*;
    use crate::renderer::cloud_noise;

    /// TEMPORARY diagnostic for the increment-8 bring-up: prints the field
    /// distributions layer by layer so a broken mirror localizes itself.
    #[test]
    #[ignore = "diagnostic probe, run by hand"]
    fn probe_field_layers() {
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let shape = cloud_noise::generate_shape(threads);
        let detail = cloud_noise::generate_detail(threads);
        let r_km = 6371.0f32;
        let ctx = CloudRefCtx {
            shape: RefVolume { data: &shape, size: cloud_noise::SHAPE_SIZE },
            detail: RefVolume { data: &detail, size: cloud_noise::DETAIL_SIZE },
            t: 3600.0,
            seed: 0.37,
            coverage: 0.95,
            type_pin: 0.34,
            rb: 1.0 + 0.4 / r_km,
            rt: 1.0 + 12.0 / r_km,
            upkm: 1.0 / r_km,
            sun_local: [0.4, 0.819, 0.4],
            sun_energy: [2.5, 2.45, 2.4],
            sky_aer: [0.35, 0.55, 0.95],
            tint: [1.0, 1.0, 1.0],
            step_km: 0.004,
            sun_step_km: 0.008,
        };
        let reg = cloud_regime(ctx.type_pin);
        println!(
            "regime: h_lo {} h_hi {} cover_bias {} fray {} fine {} ext {}",
            reg.h_lo, reg.h_hi, reg.cover_bias, reg.fray, reg.fine, reg.ext_km
        );
        let wind_ang = ctx.t * ctx.wind_omega(reg.wind_lo);
        // Weather across 20 directions near the camera column.
        let mut was = vec![];
        for i in 0..20 {
            let a = i as f32 * 0.31;
            let d = v3_norm([0.02 * a.cos(), 1.0, 0.02 * a.sin()]);
            let w = ctx.weather_pinned(d, wind_ang);
            let wa = clampf(
                cloud_alpha_from_field(w, ctx.coverage) + reg.cover_bias,
                0.0,
                1.0,
            );
            was.push((w, wa));
        }
        println!("weather (proc, wa): {:?}", &was[..8]);
        // Carve along a vertical column at h = 0.05..0.5 of the slab.
        let dir = v3_norm([0.013, 1.0, 0.007]);
        for i in 0..10 {
            let h = 0.02 + i as f32 * 0.05;
            let r = ctx.rb + (ctx.rt - ctx.rb) * h;
            let p = [dir[0] * r, dir[1] * r, dir[2] * r];
            let wa = clampf(
                cloud_alpha_from_field(ctx.weather_pinned(v3_norm(p), wind_ang), ctx.coverage)
                    + reg.cover_bias,
                0.0,
                1.0,
            );
            let cs = ctx.carve(p, wa, &reg, 1.0);
            let dh = ctx.density_hi(p, wa, &reg, 1.0, 1.0, 1.0);
            println!(
                "h {:.2} wa {:.3} carve {:.4} dens {:.4} crown {:.3}",
                h, wa, cs.carve, dh[0], cs.crown
            );
        }
    }
}

#[cfg(test)]
mod debug_probe2 {
    use super::*;
    use crate::renderer::cloud_noise;

    #[test]
    #[ignore = "diagnostic probe, run by hand"]
    fn probe_body_vs_threshold() {
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let shape = cloud_noise::generate_shape(threads);
        let r_km = 6371.0f32;
        let upkm = 1.0 / r_km;
        let shape_freq = 1.0 / (CLOUD_SHAPE_TILE_KM * upkm);
        let vol = RefVolume { data: &shape, size: cloud_noise::SHAPE_SIZE };
        // Sample the shape texture across a wide sweep at slab heights and
        // report the body-vs-threshold distribution the carve sees.
        let wa = 0.979f32;
        let thr_base = mixf(CLOUD_COV_LO, CLOUD_COV_HI, wa);
        let mut bodies = vec![];
        for i in 0..2000 {
            let a = i as f32 * 0.618034 * std::f32::consts::TAU;
            let z = -1.0 + 2.0 * ((i as f32 + 0.5) / 2000.0);
            let xy = (1.0f32 - z * z).max(0.0).sqrt();
            let dir = [xy * a.cos(), z, xy * a.sin()];
            let r = 1.0 + 3.0 / r_km; // 3 km up, mid low-band
            let p = [dir[0] * r, dir[1] * r, dir[2] * r];
            let s = vol.sample([p[0] * shape_freq, p[1] * shape_freq, p[2] * shape_freq]);
            let lofi = s[1] * 0.625 + s[2] * 0.25 + s[3] * 0.125;
            let body = clampf(remap(s[0], lofi - 1.0, 1.0, 0.0, 1.0), 0.0, 1.0);
            bodies.push((s[0], lofi, body));
        }
        let mut bs: Vec<f32> = bodies.iter().map(|b| b.2).collect();
        bs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let q = |p: f32| bs[((bs.len() - 1) as f32 * p) as usize];
        println!(
            "thr_base {:.3}; body p05 {:.3} p25 {:.3} p50 {:.3} p75 {:.3} p95 {:.3} max {:.3}",
            thr_base, q(0.05), q(0.25), q(0.5), q(0.75), q(0.95), bs[bs.len()-1]
        );
        let above = bs.iter().filter(|v| **v > thr_base).count();
        println!("fraction above thr_base: {:.3}", above as f32 / bs.len() as f32);
        println!("sample rows (s.r, lofi, body): {:?}", &bodies[..6]);
    }
}

#[cfg(test)]
mod debug_probe3 {
    use super::*;
    use crate::renderer::cloud_noise;

    #[test]
    #[ignore = "diagnostic probe, run by hand"]
    fn probe_ray_alphas() {
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let shape = cloud_noise::generate_shape(threads);
        let detail = cloud_noise::generate_detail(threads);
        let r_km = 6371.0f32;
        let ctx = CloudRefCtx {
            shape: RefVolume { data: &shape, size: cloud_noise::SHAPE_SIZE },
            detail: RefVolume { data: &detail, size: cloud_noise::DETAIL_SIZE },
            t: 3600.0,
            seed: 0.37,
            coverage: 0.95,
            type_pin: 0.34,
            rb: 1.0 + 0.4 / r_km,
            rt: 1.0 + 12.0 / r_km,
            upkm: 1.0 / r_km,
            sun_local: [0.4, 0.819, 0.4],
            sun_energy: [2.5, 2.45, 2.4],
            sky_aer: [0.35, 0.55, 0.95],
            tint: [1.0, 1.0, 1.0],
            step_km: 0.008,
            sun_step_km: 0.016,
        };
        let ro = [0.0, 1.0 + 2.0 / r_km, 0.0];
        for iaz in 0..3 {
            let az = iaz as f32 * 2.094;
            let mut line = String::new();
            for iel in 0..8 {
                let el = 0.14 + iel as f32 * 0.14;
                let rd = [el.cos() * az.cos(), el.sin(), el.cos() * az.sin()];
                let (rgb, a) = ctx.reference_radiance(ro, rd);
                let lum = rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
                line.push_str(&format!("({:.0},a{:.2},L{:.2}) ", el.to_degrees(), a, lum));
            }
            println!("az{iaz}: {line}");
        }
    }
}

#[cfg(test)]
mod debug_probe4 {
    use super::*;
    use crate::renderer::cloud_noise;

    #[test]
    #[ignore = "diagnostic probe, run by hand"]
    fn probe_erosion_chain() {
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let shape = cloud_noise::generate_shape(threads);
        let detail = cloud_noise::generate_detail(threads);
        let r_km = 6371.0f32;
        let ctx = CloudRefCtx {
            shape: RefVolume { data: &shape, size: cloud_noise::SHAPE_SIZE },
            detail: RefVolume { data: &detail, size: cloud_noise::DETAIL_SIZE },
            t: 3600.0,
            seed: 0.37,
            coverage: 0.95,
            type_pin: 0.34,
            rb: 1.0 + 0.4 / r_km,
            rt: 1.0 + 12.0 / r_km,
            upkm: 1.0 / r_km,
            sun_local: [0.4, 0.819, 0.4],
            sun_energy: [2.5, 2.45, 2.4],
            sky_aer: [0.35, 0.55, 0.95],
            tint: [1.0, 1.0, 1.0],
            step_km: 0.008,
            sun_step_km: 0.016,
        };
        let reg = cloud_regime(ctx.type_pin);
        println!("reg: fray {:.3} fine {:.3} filament {:.3} stretch {:.3} base_drop {:.3} h {:.3}..{:.3}",
            reg.fray, reg.fine, reg.filament, reg.stretch, reg.base_drop, reg.h_lo, reg.h_hi);
        let wa = 0.979f32;
        let mut printed = 0;
        for i in 0..400 {
            if printed >= 8 { break; }
            let a = i as f32 * 0.618034 * std::f32::consts::TAU;
            let z = 0.3 * ((i as f32 * 0.317).sin());
            let xy = (1.0f32 - z * z).sqrt();
            let dir = [xy * a.cos(), z, xy * a.sin()];
            let h = 0.15 + 0.02 * (i as f32 % 7.0);
            let r = ctx.rb + (ctx.rt - ctx.rb) * h;
            let p = [dir[0] * r, dir[1] * r, dir[2] * r];
            let cs = ctx.carve(p, wa, &reg, 0.0);
            if cs.carve <= 0.01 { continue; }
            printed += 1;
            let mut base = cs.carve;
            let fray_freq = 1.0 / (CLOUD_FRAY_TILE_KM * ctx.upkm);
            let fr = ctx.detail.sample([cs.ps[0]*fray_freq, cs.ps[1]*fray_freq, cs.ps[2]*fray_freq]);
            let frfbm = fr[0]*0.625 + fr[1]*0.25 + fr[2]*0.125;
            let erode_c = frfbm * reg.fray * CLOUD_FRAY_ERODE * (0.35 + 0.65*(1.0-base));
            let after_fray = clampf(remap(base, erode_c, 1.0, 0.0, 1.0), 0.0, 1.0);
            let fmask = smoothstep(CLOUD_FIL_LO, CLOUD_FIL_HI, fr[3]);
            let after_fil = after_fray * mixf(1.0, fmask, reg.filament);
            let dh = ctx.density_hi(p, wa, &reg, 1.0, 1.0, 0.0);
            println!("carve {:.3} frfbm {:.3} erode_c {:.3} after_fray {:.3} fr.a {:.3} fmask {:.3} after_fil {:.3} FINAL {:.3}",
                cs.carve, frfbm, erode_c, after_fray, fr[3], fmask, after_fil, dh[0]);
        }
        println!("printed {printed} of 400 candidate points (carve > 0.01)");
    }
}

#[cfg(test)]
mod debug_probe5 {
    use super::*;
    use crate::renderer::cloud_noise;

    #[test]
    #[ignore = "diagnostic probe, run by hand"]
    fn probe_carve_internals() {
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let shape = cloud_noise::generate_shape(threads);
        let r_km = 6371.0f32;
        let upkm = 1.0 / r_km;
        let rb = 1.0 + 0.4 / r_km;
        let rt = 1.0 + 12.0 / r_km;
        let vol = RefVolume { data: &shape, size: cloud_noise::SHAPE_SIZE };
        let reg = cloud_regime(0.34);
        let wa = 0.979f32;
        let t = 3600.0f32;
        let shape_freq = 1.0 / (CLOUD_SHAPE_TILE_KM * upkm);
        for i in 0..12 {
            let a = i as f32 * 0.618034 * std::f32::consts::TAU;
            let z = 0.3 * ((i as f32 * 0.317).sin());
            let xy = (1.0f32 - z * z).sqrt();
            let dir = [xy * a.cos(), z, xy * a.sin()];
            let h = 0.15 + 0.02 * (i as f32 % 7.0);
            let r = rb + (rt - rb) * h;
            let p = [dir[0] * r, dir[1] * r, dir[2] * r];
            // replicate carve internals
            let omega = reg.wind_lo * upkm / 1000.0 * 0.0 + (reg.wind_lo + (reg.wind_hi - reg.wind_lo) * h) * upkm / 1000.0;
            let ps0 = cloud_rot_y(p, t * omega);
            let ps = stretch_domain(ps0, v3_norm(p), reg.stretch);
            let s = vol.sample([ps[0]*shape_freq, ps[1]*shape_freq, ps[2]*shape_freq]);
            let lofi = s[1]*0.625 + s[2]*0.25 + s[3]*0.125;
            let body = clampf(remap(s[0], lofi-1.0, 1.0, 0.0, 1.0), 0.0, 1.0);
            let tower = smoothstep(0.62, 0.92, lofi);
            let h_hi_eff = (reg.h_hi + tower*0.8*(reg.h_hi-reg.h_lo)).min(1.0);
            let env = height_band(h, reg.h_lo, h_hi_eff);
            let u_band = clampf((h-reg.h_lo)/(h_hi_eff-reg.h_lo).max(1e-4), 0.0, 1.0);
            let thr_base = mixf(CLOUD_COV_LO, CLOUD_COV_HI, wa);
            let v_band = 1.0-u_band;
            let thr = thr_base + CLOUD_TOP_RISE*u_band*u_band
                + CLOUD_BASE_DROP*reg.base_drop*v_band*v_band*(1.0-lofi);
            let carve = clampf((body-thr)/(1.0-thr).max(1e-3), 0.0, 1.0)*env;
            println!("h {:.2} |ps| {:.4} s.r {:.3} lofi {:.3} body {:.3} thr {:.3} env {:.3} carve {:.3}",
                h, v3_len(ps), s[0], lofi, body, thr, env, carve);
        }
    }
}

#[cfg(test)]
mod debug_probe6 {
    use super::*;
    use crate::renderer::cloud_noise;

    #[test]
    #[ignore = "diagnostic probe, run by hand"]
    fn probe_low_rays() {
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let shape = cloud_noise::generate_shape(threads);
        let detail = cloud_noise::generate_detail(threads);
        let r_km = 6371.0f32;
        let ctx = CloudRefCtx {
            shape: RefVolume { data: &shape, size: cloud_noise::SHAPE_SIZE },
            detail: RefVolume { data: &detail, size: cloud_noise::DETAIL_SIZE },
            t: 3600.0, seed: 0.37, coverage: 0.95, type_pin: 0.34,
            rb: 1.0 + 0.4 / r_km, rt: 1.0 + 12.0 / r_km, upkm: 1.0 / r_km,
            sun_local: [0.4, 0.819, 0.4],
            sun_energy: [2.5, 2.45, 2.4],
            sky_aer: [0.35, 0.55, 0.95],
            tint: [1.0, 1.0, 1.0],
            step_km: 0.008, sun_step_km: 0.016,
        };
        let ro = [0.0, 1.0 + 2.0 / r_km, 0.0];
        for iaz in 0..4 {
            let az = iaz as f32 * 1.5708;
            let mut line = String::new();
            for iel in 0..6 {
                let el = 0.03 + iel as f32 * 0.035; // 1.7 .. 11.7 degrees
                let rd = [el.cos() * az.cos(), el.sin(), el.cos() * az.sin()];
                let (rgb, a) = ctx.reference_radiance(ro, rd);
                let lum = rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
                line.push_str(&format!("({:.1},a{:.2},L{:.2}) ", el.to_degrees(), a, lum));
            }
            println!("az{iaz}: {line}");
        }
    }
}
