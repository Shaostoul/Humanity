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
const CLOUD_COV_LO: f32 = 0.854;
const CLOUD_COV_HI: f32 = 0.347;
const CLOUD_TOP_RISE: f32 = 0.45;
const CLOUD_BASE_DROP: f32 = 0.35;
const CLOUD_CELL_SPLIT: f32 = 0.15;
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
        let body = s[0]; // single construction (10b): bake owns Perlin-Worley
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
            thr += CLOUD_CELL_SPLIT * cell_amt * reg.fine * (0.481 - c[1]); // centered (increment 11)
        }
        let carve = clampf((body - thr) / (0.79 - thr).max(1.0e-3), 0.0, 1.0) * env; // CLOUD_BODY_TOP
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
            let detail_amt = 1.0; // fades deleted (increment 11)
            let puff_amt = 1.0;
            let cell_amt = 1.0;
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
            let crown_floor = mixf(0.88, 0.62, reg.opacity);
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
            let body = s[0]; // single construction (10b): bake owns Perlin-Worley
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

#[cfg(test)]
mod gpu_vs_reference {
    use super::*;
    use crate::renderer::cloud_noise;

    fn srgb_encode(x: f32) -> f32 {
        if x <= 0.0031308 {
            12.92 * x
        } else {
            1.055 * x.powf(1.0 / 2.4) - 0.055
        }
    }

    /// THE INCREMENT-10 JUDGE: re-march the EXACT captured scene on the CPU
    /// and compare per-ray radiances against the capture's pixels.
    ///
    ///   CLOUD_REF_DUMP=path/to/cloud_ref_dump.json \
    ///   cargo test --release --features native --lib gpu_vs_reference_from_dump -- --ignored --nocapture
    ///
    /// The dump is written beside every screenshot (execute_screenshot_capture);
    /// its "capture" field names the PNG. Gate: mean per-ray luminance error
    /// < 5% on rays where the reference is near-opaque (alpha >= 0.93 - the
    /// sky bleed through the remaining transmittance is ~2-3% and is
    /// reported separately, composited from the capture's own local sky).
    #[test]
    #[ignore = "needs a capture + dump pair; run by hand with CLOUD_REF_DUMP set"]
    fn gpu_vs_reference_from_dump() {
        let dump_path = std::env::var("CLOUD_REF_DUMP")
            .unwrap_or_else(|_| ".probe-rig/debug/cloud_ref_dump.json".to_string());
        let dump: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&dump_path).expect("read dump"))
                .expect("parse dump");
        let shell = &dump["shell"];
        let f = |v: &serde_json::Value| v.as_f64().unwrap() as f32;
        let arr3 = |v: &serde_json::Value| {
            [f(&v[0]), f(&v[1]), f(&v[2])]
        };
        // Pins: the reference only handles the pinned weather path.
        let mut pin = f(&shell["pin"]);
        if shell["temporal"].as_bool().unwrap_or(false) {
            pin -= 4.0;
        }
        assert!(
            pin >= 1.5,
            "capture was not type-pinned (pin {pin}) - the reference needs cloud_cover+cloud_type pins"
        );
        let type_pin = (pin - 2.0).clamp(0.0, 1.0);

        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let shape = cloud_noise::generate_shape(threads);
        let detail = cloud_noise::generate_detail(threads);

        // World -> planet-frame transform.
        let center = arr3(&shell["center"]);
        let rq = &shell["rot"];
        let rot = glam::Quat::from_xyzw(f(&rq[0]), f(&rq[1]), f(&rq[2]), f(&rq[3]));
        let inv_rot = rot.conjugate();
        let visual_scale = f(&shell["visual_scale"]);
        let to_planet = |w: [f32; 3]| -> [f32; 3] {
            let v = glam::Vec3::new(w[0] - center[0], w[1] - center[1], w[2] - center[2]);
            let p = inv_rot * v / visual_scale;
            [p.x, p.y, p.z]
        };
        let dir_to_planet = |w: [f32; 3]| -> [f32; 3] {
            let p = (inv_rot * glam::Vec3::new(w[0], w[1], w[2])).normalize();
            [p.x, p.y, p.z]
        };

        let sun_col = arr3(&dump["sun_color"]);
        let sun_int = f(&dump["sun_intensity"]);
        let ctx = CloudRefCtx {
            shape: RefVolume { data: &shape, size: cloud_noise::SHAPE_SIZE },
            detail: RefVolume { data: &detail, size: cloud_noise::DETAIL_SIZE },
            t: f(&dump["clock"]),
            seed: f(&shell["seed"]),
            coverage: f(&shell["coverage"]),
            type_pin,
            rb: f(&shell["slab_rb"]),
            rt: f(&shell["slab_rt"]),
            upkm: 1.0 / f(&shell["radius_km"]),
            sun_local: dir_to_planet(arr3(&dump["sun_dir"])),
            sun_energy: [
                sun_col[0] * sun_int,
                sun_col[1] * sun_int,
                sun_col[2] * sun_int,
            ],
            sky_aer: arr3(&dump["aerial_sky"]),
            tint: arr3(&shell["tint"]),
            step_km: 0.004,
            sun_step_km: 0.008,
        };

        // Camera rays.
        let cam_pos = to_planet(arr3(&dump["cam_pos"]));
        let fwd_w = arr3(&dump["cam_fwd"]);
        let right_w = arr3(&dump["cam_right"]);
        let up_w = {
            let r = glam::Vec3::from(right_w);
            let fw = glam::Vec3::from(fwd_w);
            let u = r.cross(fw).normalize();
            [u.x, u.y, u.z]
        };
        let fov_y = f(&dump["fov_deg"]).to_radians();
        let aspect = f(&dump["aspect"]);
        let (vw, vh) = (f(&dump["viewport"][0]), f(&dump["viewport"][1]));
        let cap_path = dump["capture"].as_str().unwrap().to_string();
        // The capture path is relative to the APP's working dir (the rig
        // sandbox) - resolve against the dump file's parent-of-parent
        // FIRST. Checking the raw path first once read a STALE capture
        // from the repo's own debug/ dir (same relative name, different
        // session) and reported 2600% error against the wrong image.
        let dump_dir = std::path::Path::new(&dump_path).parent().unwrap();
        let rig_local = dump_dir.parent().unwrap().join(&cap_path);
        let cap_file = if rig_local.exists() {
            rig_local
        } else {
            std::path::PathBuf::from(&cap_path)
        };
        let img = image::open(&cap_file).expect("open capture").to_rgb8();
        assert_eq!(img.width() as f32, vw, "capture width != dump viewport");

        let ray_for = |px: f32, py: f32| -> [f32; 3] {
            let xn = (2.0 * px / vw - 1.0) * (fov_y * 0.5).tan() * aspect;
            let yn = (1.0 - 2.0 * py / vh) * (fov_y * 0.5).tan();
            let fw = glam::Vec3::from(fwd_w);
            let r = glam::Vec3::from(right_w);
            let u = glam::Vec3::from(up_w);
            let d = (fw + r * xn + u * yn).normalize();
            let dp = dir_to_planet([d.x, d.y, d.z]);
            dp
        };

        // Sample a grid across the cloud band (the joint-gate ROI region and
        // above): x in [200, vw-200], y in [420, 900] - sky rows.
        let mut rows: Vec<(f32, f32, [f32; 3], f32, [u8; 3])> = Vec::new();
        for iy in 0..6 {
            for ix in 0..8 {
                let px = 250.0 + ix as f32 * (vw - 500.0) / 7.0;
                let py = 430.0 + iy as f32 * 470.0 / 5.0;
                let rd = ray_for(px, py);
                let (rgb, a) = ctx.reference_radiance(cam_pos, rd);
                let p = img.get_pixel(px as u32, py as u32);
                rows.push((px, py, rgb, a, [p[0], p[1], p[2]]));
            }
        }
        let opaque: Vec<_> = rows.iter().filter(|r| r.3 >= 0.93).collect();
        println!("{} of {} rays near-opaque (alpha >= 0.93)", opaque.len(), rows.len());
        assert!(
            opaque.len() >= 6,
            "only {} near-opaque rays - re-aim the grid or the scene is too clear",
            opaque.len()
        );
        let mut errs: Vec<f32> = Vec::new();
        for (px, py, rgb, a, cap) in &opaque {
            let ref_lum_lin = rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
            let ref_srgb = srgb_encode(ref_lum_lin) * 255.0;
            let cap_lum =
                0.2126 * cap[0] as f32 + 0.7152 * cap[1] as f32 + 0.0722 * cap[2] as f32;
            let rel = (ref_srgb - cap_lum) / cap_lum.max(1.0);
            errs.push(rel);
            println!(
                "px ({px:.0},{py:.0}) alpha {a:.2}: ref {ref_srgb:.0} vs cap {cap_lum:.0} ({:+.1}%)",
                rel * 100.0
            );
        }
        let mean_abs = errs.iter().map(|e| e.abs()).sum::<f32>() / errs.len() as f32;
        let mean_signed = errs.iter().sum::<f32>() / errs.len() as f32;
        println!(
            "mean |err| {:.1}%  signed {:+.1}%  (gate < 5%)",
            mean_abs * 100.0,
            mean_signed * 100.0
        );
        assert!(
            mean_abs < 0.05,
            "GPU-vs-reference mean |err| {:.1}% >= 5%",
            mean_abs * 100.0
        );
    }
}

// ── The GPU-INTEGRATOR TWIN (increment 10 tuning harness) ──
// A CPU emulation of cloud_march_core's INTEGRATOR - the Wave B step law,
// the coarse-entry backtrack, the MFP interior refinement, and the 8-tap
// geometric sun ladder with its first-2-taps-eroded/rest-body-only split -
// over the same field mirrors the converged reference uses. Twin-vs-
// reference isolates integrator bias in SECONDS on the CPU instead of a
// 6-minute build+sweep per hypothesis. The twin's knobs mirror the WGSL
// constants; keep them in lockstep when the shader changes.
impl<'a> CloudRefCtx<'a> {
    pub fn twin_radiance(&self, ro: [f32; 3], rd: [f32; 3], jitter: f32) -> ([f32; 3], f32) {
        let rd = v3_norm(rd);
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
        if d2 < 1.0 {
            let t_surf = tca - (1.0 - d2).sqrt();
            if t_surf > 0.0 && t_surf < m0 {
                return ([0.0; 3], 0.0);
            }
        }
        let seg = m1 - m0;
        let reg = cloud_regime(self.type_pin);
        let wind_ang = self.t * self.wind_omega(reg.wind_lo);
        let wa_at = |me: &Self, p: [f32; 3]| -> f32 {
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
        // WGSL twin constants (keep in lockstep with 40-clouds.wgsl).
        let slab_h = self.rt - self.rb;
        let step_near = slab_h * 0.045; // CLOUD_STEP_BAND_FRAC
        let pix_ang = 0.00144f32; // direct path, 90 deg over 1387 rows
        let cone_k = 24.0; // CLOUD_STEP_CONE_K
        let vert_frac = 0.08;
        let seg_frac = 0.020833;
        let tau_max = 0.75;
        let gate = 0.02;
        let iter_cap = 224;

        let mut t_cur = m0;
        let mut dens_prev = 0.0f32;
        let mut trans: f64 = 1.0;
        let mut acc = [0.0f64; 3];
        let mut acc_w: f64 = 0.0;
        for _ in 0..iter_cap {
            if t_cur >= m1 {
                break;
            }
            let p_cur = v3_add_scaled(ro, rd, t_cur);
            let r_rate = v3_dot(v3_norm(p_cur), rd).abs();
            let dt_vert = (slab_h * vert_frac / r_rate.max(0.05)).max(step_near);
            let dt_seg = (seg * seg_frac).max(step_near);
            let mut dt = (t_cur * pix_ang * cone_k)
                .max(step_near)
                .min(dt_vert.min(dt_seg))
                .min(m1 - t_cur);
            if dens_prev > gate {
                let dt_mfp = tau_max / (sigma_v * dens_prev);
                dt = dt.min(dt_mfp.max(slab_h * 0.002));
            }
            let tm = t_cur + dt * jitter;
            t_cur += dt;
            let p = v3_add_scaled(ro, rd, tm);
            let dirp = v3_norm(p);
            let weather_a = wa_at(self, p);
            let detail_amt = 1.0; // fades deleted (increment 11)
            let puff_amt = 1.0;
            let cell_amt = 1.0;
            let dc = self.density_hi(p, weather_a, &reg, detail_amt, puff_amt, cell_amt);
            let dens = dc[0];
            // Coarse-entry backtrack (mirrors the WGSL).
            if dens > gate && dens_prev <= gate && sigma_v * dens * dt > tau_max {
                t_cur -= dt;
                dens_prev = dens;
                continue;
            }
            dens_prev = dens;
            if dens <= 0.001 {
                continue;
            }
            let a_i = 1.0 - (-sigma_v * dens * dt).exp();
            let ndl = v3_dot(dirp, self.sun_local);
            let day = smoothstep(-0.05, 0.3, ndl);
            let tau = self.twin_sun_tau(p, &wa_at, &reg, detail_amt, puff_amt, cell_amt);
            let powder = 1.0 - CLOUD_POWDER_STRENGTH * (-2.0 * tau).exp();
            let pw = mixf(powder, 1.0, powder_gate);
            let direct = cloud_scatter_energy(tau, cos_vs) * pw;
            let h = clampf((v3_len(p) - self.rb) / (self.rt - self.rb), 0.0, 1.0);
            let amb_h =
                mixf(CLOUD_AMB_BASE, CLOUD_AMB_TOP, h) * (0.35 + 0.65 * (-tau * 0.12).exp());
            let amb_col = [
                (self.sky_aer[0] / sky_peak) * amb_h + 1.0 * (CLOUD_AMB_BOUNCE * (1.0 - h)),
                (self.sky_aer[1] / sky_peak) * amb_h + 0.93 * (CLOUD_AMB_BOUNCE * (1.0 - h)),
                (self.sky_aer[2] / sky_peak) * amb_h + 0.82 * (CLOUD_AMB_BOUNCE * (1.0 - h)),
            ];
            let crown_floor = mixf(0.88, 0.62, reg.opacity);
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
            if trans <= 0.005 {
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

    /// The GPU 8-tap geometric sun ladder (0.03 km / ratio 2.4), first two
    /// taps eroded density, remaining taps body-only.
    fn twin_sun_tau(
        &self,
        p: [f32; 3],
        wa_at: impl Fn(&Self, [f32; 3]) -> f32,
        reg: &CloudRegime,
        detail_amt: f32,
        puff_amt: f32,
        cell_amt: f32,
    ) -> f32 {
        let sigma = reg.ext_km / self.upkm;
        let mut tau = 0.0f32;
        let mut dist = 0.0f32;
        let mut step_d = 0.03 * self.upkm; // CLOUD_LIGHT_NEAR_KM
        for _i in 0..12 {
            dist += step_d;
            let seg = step_d;
            step_d *= 1.9; // CLOUD_LIGHT_RATIO
            let lp = v3_add_scaled(p, self.sun_local, dist);
            let r = v3_len(lp);
            if r < 1.0 {
                break;
            }
            let wa = wa_at(self, lp);
            // ALL taps on the REAL eroded density (increment 10): the old
            // body-only far taps returned ~1 across the whole carved
            // envelope - a MASK, not a density - which at physical
            // extinction (45/km) reported tau in the HUNDREDS where the
            // converged fine march reads 1-10. That bimodal tau (0 in
            // gaps, absurd in bodies) was the dots' 18.9x energy coin
            // flip. Twin-measured: -90% -> see the gap test.
            let dens = self.density_hi(lp, wa, reg, detail_amt, puff_amt, cell_amt)[0];
            tau += sigma * dens * seg;
            if tau > 40.0 {
                break;
            }
        }
        tau
    }
}

#[cfg(test)]
mod twin_vs_reference {
    use super::*;
    use crate::renderer::cloud_noise;

    /// CPU-only integrator-bias isolation (increment 10): march the SAME
    /// rays with the GPU-twin integrator and the converged reference and
    /// report the systematic gap. Run in release:
    ///   cargo test --release --features native --lib twin_vs_reference_gap -- --ignored --nocapture
    #[test]
    #[ignore = "heavy diagnostic; run by hand in release"]
    fn twin_vs_reference_gap() {
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let shape = cloud_noise::generate_shape(threads);
        let detail = cloud_noise::generate_detail(threads);
        let r_km = 6371.0f32;
        let ctx = CloudRefCtx {
            shape: RefVolume { data: &shape, size: cloud_noise::SHAPE_SIZE },
            detail: RefVolume { data: &detail, size: cloud_noise::DETAIL_SIZE },
            t: 3600.0,
            seed: 42.0,
            coverage: 0.95,
            type_pin: 0.34,
            rb: 1.0 + 0.4 / r_km,
            rt: 1.0 + 12.0 / r_km,
            upkm: 1.0 / r_km,
            sun_local: {
                let v = [0.4f32, 0.819, 0.4];
                let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                [v[0] / l, v[1] / l, v[2] / l]
            },
            sun_energy: [2.29, 2.0, 1.47],
            sky_aer: [0.58, 0.61, 0.55],
            tint: [1.0, 1.0, 1.0],
            step_km: 0.004,
            sun_step_km: 0.008,
        };
        let ro = [0.0, 1.0 + 0.3 / r_km, 0.0];
        let mut gaps: Vec<f32> = Vec::new();
        let mut n_cloud = 0;
        for iaz in 0..6 {
            let az = iaz as f32 * 1.0471976;
            for iel in 0..8 {
                let el = 0.05 + iel as f32 * 0.05;
                let rd = [el.cos() * az.cos(), el.sin(), el.cos() * az.sin()];
                let (rref, aref) = ctx.reference_radiance(ro, rd);
                if aref < 0.9 {
                    continue;
                }
                n_cloud += 1;
                // Average the twin over several jitters (the EMA map does).
                let mut tl = 0.0f32;
                for j in 0..5 {
                    let (rt_, _at) = ctx.twin_radiance(ro, rd, 0.1 + 0.2 * j as f32);
                    tl += rt_[0] * 0.2126 + rt_[1] * 0.7152 + rt_[2] * 0.0722;
                }
                tl /= 5.0;
                let rl = rref[0] * 0.2126 + rref[1] * 0.7152 + rref[2] * 0.0722;
                let gap = (tl - rl) / rl.max(1e-3);
                gaps.push(gap);
                println!(
                    "el {:5.1} az {} ref {:.3} twin {:.3} gap {:+.1}%",
                    el.to_degrees(),
                    iaz,
                    rl,
                    tl,
                    gap * 100.0
                );
            }
        }
        let mean = gaps.iter().sum::<f32>() / gaps.len().max(1) as f32;
        println!("cloud rays {n_cloud}; mean signed gap {:+.1}%", mean * 100.0);
    }
}

#[cfg(test)]
mod tau_probe {
    use super::*;
    use crate::renderer::cloud_noise;

    #[test]
    #[ignore = "diagnostic"]
    fn ladder_vs_fine_tau() {
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let shape = cloud_noise::generate_shape(threads);
        let detail = cloud_noise::generate_detail(threads);
        let r_km = 6371.0f32;
        let ctx = CloudRefCtx {
            shape: RefVolume { data: &shape, size: cloud_noise::SHAPE_SIZE },
            detail: RefVolume { data: &detail, size: cloud_noise::DETAIL_SIZE },
            t: 3600.0, seed: 42.0, coverage: 0.95, type_pin: 0.34,
            rb: 1.0 + 0.4 / r_km, rt: 1.0 + 12.0 / r_km, upkm: 1.0 / r_km,
            sun_local: { let v=[0.4f32,0.819,0.4]; let l=(v[0]*v[0]+v[1]*v[1]+v[2]*v[2]).sqrt(); [v[0]/l,v[1]/l,v[2]/l] },
            sun_energy: [2.29, 2.0, 1.47],
            sky_aer: [0.58, 0.61, 0.55],
            tint: [1.0, 1.0, 1.0],
            step_km: 0.004, sun_step_km: 0.008,
        };
        let reg = cloud_regime(ctx.type_pin);
        let wind_ang = ctx.t * ctx.wind_omega(reg.wind_lo);
        let wa_at = |me: &CloudRefCtx, p: [f32; 3]| -> f32 {
            clampf(
                cloud_alpha_from_field(me.weather_pinned(v3_norm(p), wind_ang), me.coverage)
                    + reg.cover_bias, 0.0, 1.0)
        };
        let ro = [0.0, 1.0 + 0.3 / r_km, 0.0];
        let az = 1.0471976f32;
        let el = 0.05f32;
        let rd = [el.cos() * az.cos(), el.sin(), el.cos() * az.sin()];
        // Walk the ray finely; at in-cloud points, compare taus.
        let dt = 0.004 * ctx.upkm * 4.0;
        let mut printed = 0;
        let mut tm = 0.0f32;
        while printed < 12 && tm < 0.02 {
            tm += dt * 8.0;
            let p = v3_add_scaled(ro, rd, tm);
            let wa = wa_at(&ctx, p);
            let dens = ctx.density_hi(p, wa, &reg, 1.0, 1.0, 1.0)[0];
            if dens < 0.3 { continue; }
            printed += 1;
            let sigma = reg.ext_km / ctx.upkm;
            // fine tau
            let mut tf = 0.0f32;
            let step = ctx.sun_step_km * ctx.upkm;
            let mut lp = p;
            for _ in 0..((400.0/ctx.sun_step_km) as usize) {
                lp = v3_add_scaled(lp, ctx.sun_local, step);
                let r = v3_len(lp);
                if r > ctx.rt || r < 1.0 { break; }
                if r >= ctx.rb {
                    let w = wa_at(&ctx, lp);
                    tf += sigma * ctx.density_hi(lp, w, &reg, 1.0, 1.0, 1.0)[0] * step;
                    if tf > 40.0 { break; }
                }
            }
            let tl = ctx.twin_sun_tau(p, &wa_at, &reg, 1.0, 1.0, 1.0);
            println!("tm {:.5} dens {:.2} tau_fine {:.2} tau_ladder {:.2}", tm, dens, tf, tl);
        }
    }
}

#[cfg(test)]
mod cov_recenter {
    use super::*;
    use crate::renderer::cloud_noise;

    /// 10b COVERAGE RE-CENTER: measure the flipped body distribution at
    /// slab heights and print the quantiles matching the OLD thresholds'
    /// percentile cuts, so CLOUD_COV_LO/HI can be re-derived instead of
    /// guessed. (The old window 0.92/0.52 cut the OLD distribution at
    /// specific percentiles; the flipped distribution shifts.)
    #[test]
    #[ignore = "diagnostic, run by hand"]
    fn body_quantiles_after_flip() {
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let shape = cloud_noise::generate_shape(threads);
        let r_km = 6371.0f32;
        let upkm = 1.0 / r_km;
        let shape_freq = 1.0 / (CLOUD_SHAPE_TILE_KM * upkm);
        let vol = RefVolume { data: &shape, size: cloud_noise::SHAPE_SIZE };
        let mut bodies = vec![];
        for i in 0..4000 {
            let a = i as f32 * 0.618034 * std::f32::consts::TAU;
            let z = -1.0 + 2.0 * ((i as f32 + 0.5) / 4000.0);
            let xy = (1.0f32 - z * z).max(0.0).sqrt();
            let dir = [xy * a.cos(), z, xy * a.sin()];
            let r = 1.0 + 3.0 / r_km;
            let p = [dir[0] * r, dir[1] * r, dir[2] * r];
            let s = vol.sample([p[0] * shape_freq, p[1] * shape_freq, p[2] * shape_freq]);
            let lofi = s[1] * 0.625 + s[2] * 0.25 + s[3] * 0.125;
            let body = s[0]; // single construction (10b): bake owns Perlin-Worley
            bodies.push(body);
        }
        bodies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let q = |p: f32| bodies[((bodies.len() - 1) as f32 * p) as usize];
        println!(
            "flipped body quantiles: p01 {:.3} p05 {:.3} p10 {:.3} p25 {:.3} p50 {:.3} p75 {:.3} p90 {:.3} p95 {:.3} p99 {:.3}",
            q(0.01), q(0.05), q(0.10), q(0.25), q(0.50), q(0.75), q(0.90), q(0.95), q(0.99)
        );
        // The OLD distribution's percentile cuts (measured pre-flip,
        // 2026-08-21 probe2): body p05 0.687 p50 0.784 p95 0.854.
        // OLD COV_LO 0.92 sat ABOVE p99 (nearly nothing at wa=0);
        // OLD COV_HI 0.52 sat below p01 (everything at wa=1, minus the
        // rise/cell terms). Print where those percentile anchors land now.
        println!(
            "suggested COV_LO (above p99): {:.3}   COV_HI (near p01 - margin): {:.3}",
            q(0.99) + 0.066, q(0.01) - 0.167
        );
    }
}

#[cfg(test)]
mod field_map {
    use super::*;
    use crate::renderer::cloud_noise;

    /// Renders the carve field at slab height 0.22 over a lat-lon grid to a
    /// PGM (scratch diagnostics for the 10b polarity work): the fastest way
    /// to SEE broken-vs-sheet structure without a GPU boot.
    #[test]
    #[ignore = "diagnostic, run by hand"]
    fn write_carve_map() {
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let shape = cloud_noise::generate_shape(threads);
        let detail = cloud_noise::generate_detail(threads);
        let r_km = 6371.0f32;
        let ctx = CloudRefCtx {
            shape: RefVolume { data: &shape, size: cloud_noise::SHAPE_SIZE },
            detail: RefVolume { data: &detail, size: cloud_noise::DETAIL_SIZE },
            t: 3600.0, seed: 42.0, coverage: 0.95, type_pin: 0.34,
            rb: 1.0 + 0.4 / r_km, rt: 1.0 + 12.0 / r_km, upkm: 1.0 / r_km,
            sun_local: [0.24, 0.94, 0.24],
            sun_energy: [2.29, 2.0, 1.47],
            sky_aer: [0.58, 0.61, 0.55],
            tint: [1.0, 1.0, 1.0],
            step_km: 0.004, sun_step_km: 0.008,
        };
        let reg = cloud_regime(ctx.type_pin);
        let wind_ang = ctx.t * ctx.wind_omega(reg.wind_lo);
        let (w, h) = (768usize, 384usize);
        let mut img = vec![0u8; w * h];
        // A ~1500 km patch around lat 47.6 lon -122.7 (2 km/px).
        for iy in 0..h {
            for ix in 0..w {
                let lat = (47.645 + (iy as f32 - h as f32 / 2.0) * (2.0 / 111.0)).to_radians();
                let lon = (-122.6925 + (ix as f32 - w as f32 / 2.0) * (2.0 / 78.0)).to_radians();
                let dir = [lat.cos() * lon.cos(), lat.sin(), -lat.cos() * lon.sin()];
                let r = ctx.rb + (ctx.rt - ctx.rb) * 0.22;
                let p = [dir[0] * r, dir[1] * r, dir[2] * r];
                let wa = clampf(
                    cloud_alpha_from_field(ctx.weather_pinned(v3_norm(p), wind_ang), ctx.coverage)
                        + reg.cover_bias, 0.0, 1.0);
                let d = ctx.density_hi(p, wa, &reg, 1.0, 1.0, 1.0);
                img[iy * w + ix] = (clampf(d[0] * 2.0, 0.0, 1.0) * 255.0) as u8;
            }
        }
        let mut out = format!("P5\n{w} {h}\n255\n").into_bytes();
        out.extend_from_slice(&img);
        std::fs::write("carve_map.pgm", out).unwrap();
        let cov = img.iter().filter(|v| **v > 32).count() as f32 / img.len() as f32;
        println!("carve map written; fraction carve>0.06: {:.3}", cov);
    }
}

#[cfg(test)]
mod overhead_profile {
    use super::*;
    use crate::renderer::cloud_noise;

    #[test]
    #[ignore = "diagnostic"]
    fn density_along_overhead_ray() {
        let dump: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(".probe-rig/debug/cloud_ref_dump.json").unwrap(),
        )
        .unwrap();
        let shell = &dump["shell"];
        let f = |v: &serde_json::Value| v.as_f64().unwrap() as f32;
        let arr3 = |v: &serde_json::Value| [f(&v[0]), f(&v[1]), f(&v[2])];
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let shape = cloud_noise::generate_shape(threads);
        let detail = cloud_noise::generate_detail(threads);
        let center = arr3(&shell["center"]);
        let rq = &shell["rot"];
        let rot = glam::Quat::from_xyzw(f(&rq[0]), f(&rq[1]), f(&rq[2]), f(&rq[3]));
        let inv_rot = rot.conjugate();
        let vs = f(&shell["visual_scale"]);
        let to_p = |w: [f32; 3]| {
            let v = glam::Vec3::new(w[0] - center[0], w[1] - center[1], w[2] - center[2]);
            let p = inv_rot * v / vs;
            [p.x, p.y, p.z]
        };
        let dir_p = |w: [f32; 3]| {
            let p = (inv_rot * glam::Vec3::new(w[0], w[1], w[2])).normalize();
            [p.x, p.y, p.z]
        };
        let sun_col = arr3(&dump["sun_color"]);
        let si = f(&dump["sun_intensity"]);
        let ctx = CloudRefCtx {
            shape: RefVolume { data: &shape, size: cloud_noise::SHAPE_SIZE },
            detail: RefVolume { data: &detail, size: cloud_noise::DETAIL_SIZE },
            t: f(&dump["clock"]),
            seed: f(&shell["seed"]),
            coverage: f(&shell["coverage"]),
            type_pin: { let mut p=f(&shell["pin"]); if shell["temporal"].as_bool().unwrap_or(false){p-=4.0;} (p-2.0).clamp(0.0,1.0) },
            rb: f(&shell["slab_rb"]),
            rt: f(&shell["slab_rt"]),
            upkm: 1.0 / f(&shell["radius_km"]),
            sun_local: dir_p(arr3(&dump["sun_dir"])),
            sun_energy: [sun_col[0]*si, sun_col[1]*si, sun_col[2]*si],
            sky_aer: arr3(&dump["aerial_sky"]),
            tint: arr3(&shell["tint"]),
            step_km: 0.004,
            sun_step_km: 0.008,
        };
        let ro = to_p(arr3(&dump["cam_pos"]));
        // Straight-up ray in planet frame from the camera.
        let up = v3_norm(ro);
        let reg = cloud_regime(ctx.type_pin);
        let wind_ang = ctx.t * ctx.wind_omega(reg.wind_lo);
        println!("cam r {:.7} rb {:.7} rt {:.7} type_pin {:.2} cov {:.2} clock {:.1}",
            v3_len(ro), ctx.rb, ctx.rt, ctx.type_pin, ctx.coverage, ctx.t);
        println!("reg: h {:.3}..{:.3} fine {:.2} fray {:.2}", reg.h_lo, reg.h_hi, reg.fine, reg.fray);
        for i in 0..24 {
            let alt_km = 0.6 + i as f32 * 0.45;
            let r = 1.0 + alt_km / 6371.0;
            let p = [up[0]*r, up[1]*r, up[2]*r];
            let wa = clampf(
                cloud_alpha_from_field(ctx.weather_pinned(v3_norm(p), wind_ang), ctx.coverage)
                    + reg.cover_bias, 0.0, 1.0);
            let cs = ctx.carve(p, wa, &reg, 1.0);
            let d = ctx.density_hi(p, wa, &reg, 1.0, 1.0, 1.0);
            let h = clampf((r - ctx.rb)/(ctx.rt - ctx.rb), 0.0, 1.0);
            let fray_freq = 1.0 / (CLOUD_FRAY_TILE_KM * ctx.upkm);
            let fr = ctx.detail.sample([cs.ps[0]*fray_freq, cs.ps[1]*fray_freq, cs.ps[2]*fray_freq]);
            let frfbm = fr[0]*0.625 + fr[1]*0.25 + fr[2]*0.125;
            println!("alt {:5.2} km h {:.3} wa {:.3} carve {:.3} dens {:.3} frfbm {:.3}", alt_km, h, wa, cs.carve, d[0], frfbm);
        }
    }
}

#[cfg(test)]
mod bake_stats {
    use crate::renderer::cloud_noise;

    #[test]
    #[ignore = "diagnostic"]
    fn shape_channel_means() {
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let shape = cloud_noise::generate_shape(threads);
        let mut m = [0f64; 4];
        for px in shape.chunks_exact(4) {
            for c in 0..4 {
                m[c] += px[c] as f64;
            }
        }
        let n = (shape.len() / 4) as f64;
        println!(
            "shape channel means: r {:.4} g {:.4} b {:.4} a {:.4}",
            m[0] / n / 255.0,
            m[1] / n / 255.0,
            m[2] / n / 255.0,
            m[3] / n / 255.0
        );
    }
}

#[cfg(test)]
mod g2_calibration {
    use super::*;
    use crate::renderer::cloud_noise;

    /// G2 calibration (increment 11b): (a) the meso pattern's quantile
    /// function q(cl) - the threshold at which P(meso > q) = cl - fitted
    /// as a cubic for the WGSL; (b) F1 = the end-to-end rendered areal
    /// fraction at wa ~= 1 (erosion + lanes eat some), which the live path
    /// divides out so a MODIS texel saying 40% RENDERS 40%.
    #[test]
    #[ignore = "calibration, run by hand in release"]
    fn meso_quantiles_and_f1() {
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let shape = cloud_noise::generate_shape(threads);
        let detail = cloud_noise::generate_detail(threads);
        let r_km = 6371.0f32;
        let ctx = CloudRefCtx {
            shape: RefVolume { data: &shape, size: cloud_noise::SHAPE_SIZE },
            detail: RefVolume { data: &detail, size: cloud_noise::DETAIL_SIZE },
            t: 3600.0, seed: 42.0, coverage: 0.95, type_pin: 0.34,
            rb: 1.0 + 0.4 / r_km, rt: 1.0 + 12.0 / r_km, upkm: 1.0 / r_km,
            sun_local: [0.24, 0.94, 0.24],
            sun_energy: [2.29, 2.0, 1.47],
            sky_aer: [0.58, 0.61, 0.55],
            tint: [1.0, 1.0, 1.0],
            step_km: 0.004, sun_step_km: 0.008,
        };
        // (a) meso distribution: the WGSL live-pattern octaves.
        let mut meso: Vec<f32> = Vec::with_capacity(20000);
        for i in 0..20000u32 {
            let a = i as f32 * 0.618034 * std::f32::consts::TAU;
            let z = -1.0 + 2.0 * ((i as f32 + 0.5) / 20000.0);
            let xy = (1.0f32 - z * z).max(0.0).sqrt();
            let dir = [xy * a.cos(), z, xy * a.sin()];
            let da0 = cloud_rot_y(dir, 0.0);
            let da = v3_norm([da0[0], da0[1] * CLOUD_BAND_STRETCH, da0[2]]);
            let db = dir;
            let m = 0.20 * cloud_noise(db, 7.0, ctx.seed + 101.0)
                + 0.12 * cloud_noise(da, 31.0, ctx.seed + 233.0)
                + 0.08 * cloud_noise(db, 67.0, ctx.seed + 409.0);
            meso.push(m);
        }
        meso.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let q = |cl: f32| meso[(((1.0 - cl) * (meso.len() - 1) as f32) as usize).min(meso.len() - 1)];
        print!("meso quantile knots: ");
        for k in [0.05f32, 0.2, 0.4, 0.6, 0.8, 0.95] {
            print!("q({k:.2})={:.4} ", q(k));
        }
        println!();
        // Fit cubic q(cl) = a + b*cl + c*cl^2 + d*cl^3 on 21 knots by
        // normal equations (tiny system - direct solve via naive Gauss).
        let n = 21usize;
        let mut xtx = [[0f64; 4]; 4];
        let mut xty = [0f64; 4];
        for i in 0..n {
            let cl = i as f32 / (n - 1) as f32 * 0.9 + 0.05;
            let y = q(cl) as f64;
            let x = [1.0, cl as f64, (cl * cl) as f64, (cl * cl * cl) as f64];
            for r in 0..4 {
                for c in 0..4 {
                    xtx[r][c] += x[r] * x[c];
                }
                xty[r] += x[r] * y;
            }
        }
        // Gauss elimination.
        let mut m4 = xtx;
        let mut v4 = xty;
        for col in 0..4 {
            let piv = (col..4).max_by(|&a, &b| m4[a][col].abs().partial_cmp(&m4[b][col].abs()).unwrap()).unwrap();
            m4.swap(col, piv);
            v4.swap(col, piv);
            for row in 0..4 {
                if row == col { continue; }
                let f = m4[row][col] / m4[col][col];
                for c2 in 0..4 {
                    m4[row][c2] -= f * m4[col][c2];
                }
                v4[row] -= f * v4[col];
            }
        }
        let coef: Vec<f64> = (0..4).map(|i| v4[i] / m4[i][i]).collect();
        println!("q(cl) cubic coeffs: {:.5} {:.5} {:.5} {:.5}", coef[0], coef[1], coef[2], coef[3]);
        let mut maxe = 0f64;
        for i in 0..n {
            let cl = i as f32 / (n - 1) as f32 * 0.9 + 0.05;
            let fit = coef[0] + coef[1]*cl as f64 + coef[2]*(cl*cl) as f64 + coef[3]*(cl*cl*cl) as f64;
            maxe = maxe.max((fit - q(cl) as f64).abs());
        }
        println!("fit max err: {maxe:.4}");
        // (b) F1: end-to-end areal density fraction at wa ~= 1 over a wide
        // patch at slab height 0.22, full erosion.
        let reg = cloud_regime(ctx.type_pin);
        let mut hit = 0u32;
        let mut tot = 0u32;
        for i in 0..8000u32 {
            let a = i as f32 * 0.618034 * std::f32::consts::TAU;
            let z = -1.0 + 2.0 * ((i as f32 + 0.5) / 8000.0);
            let xy = (1.0f32 - z * z).max(0.0).sqrt();
            let dir = [xy * a.cos(), z, xy * a.sin()];
            let r = ctx.rb + (ctx.rt - ctx.rb) * 0.22;
            let p = [dir[0] * r, dir[1] * r, dir[2] * r];
            let d = ctx.density_hi(p, 1.0, &reg, 1.0, 1.0, 1.0);
            tot += 1;
            if d[0] > 0.06 {
                hit += 1;
            }
        }
        println!("F1 (areal density fraction at wa=1): {:.3}", hit as f32 / tot as f32);
    }
}
