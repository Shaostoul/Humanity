//! Environment LAYER 1: each world's analytic climate, the continuous base
//! field under all weather (docs/design/environment-fields.md, "The layering"
//! and "Layer 1 as built").
//!
//! Air temperature, air pressure and the prevailing wind at ANY point on a
//! world, as a pure function of where (the unit direction from the body centre,
//! the altitude, how much of the neighbourhood is land) and when (the fraction
//! of the game year). No storage, no state, no camera: the rule the whole
//! environment-fields design rests on is that an effect's weight at a point is
//! a function of that point.
//!
//! The weather system's condition is the DEVIATION on top of this
//! (`systems::weather`), and the environment regions (Layer 2) place that
//! deviation on the world.
//!
//! TWO TWINS, ONE CONTRACT. This file is the simulation's copy, in f64 from a
//! `DVec3` direction (the f32-at-planet-scale rule in CLAUDE.md: never pass a
//! planet direction through f32 on its way to a sampler). The renderer's copy
//! is `EnvClimate` and the `env_l1_*` functions in
//! `assets/shaders/pbr/00-bindings-vertex.wgsl`, in f32 with small magnitudes
//! only (a unit direction, metres of altitude, a year fraction). The lockstep
//! test at the bottom of this file RUNS those shader functions through a
//! naga-IR interpreter (`renderer::shader_loader::wgsl_eval`) over a grid of
//! points and compares them with this file, the discipline the ocean wave twin
//! started (`terrain::ocean_waves`), taken one step further: that test compares
//! the shader's constants, this one compares its answers.
//!
//! Per-world parameters are DATA, one row per world, in
//! `data/environment/climate.ron`; the sources for every number are written
//! beside it there, and Earth's are reproducible with `scripts/climate-fit.js`.
//!
//! Pure math and serde only, so it compiles in every feature set (the relay
//! build includes systems/).

use glam::DVec3;
use serde::Deserialize;

/// Where the table lives under data/, and its DataStore key.
pub const DATA_FILE: &str = "environment/climate.ron";
pub const STORE_KEY: &str = "climate_table";

/// Terms in the wind series. The GPU twin unrolls exactly this many, as four
/// vec4 per coefficient set, so a row may carry FEWER (the rest are zero) but
/// never more; the loader refuses a longer list.
pub const WIND_TERMS: usize = 16;

/// The molar gas constant, J/(mol K): exact since the 2019 SI redefinition
/// (BIPM, The International System of Units, 9th edition, 2019). A physical
/// constant, not a per-world parameter.
pub const GAS_CONSTANT: f64 = 8.314_462_618_153_24;

/// Floor on a column temperature, K. Only keeps the barometric power law
/// finite where a lapse rate would otherwise carry the air below absolute zero,
/// which is far above any air worth breathing (Mars reaches it near 100 km).
pub const MIN_COLUMN_TEMP_K: f64 = 1.0;

/// Below this a lapse rate (K per metre) is treated as isothermal, where the
/// barometric formula is an exponential instead of a power law.
const ISOTHERMAL_LAPSE: f64 = 1e-9;

/// Kelvin at 0 C.
const KELVIN: f64 = 273.15;

/// Floats in the packed GPU record: 5 vec4 of scalars plus 6 x 4 vec4 of wind
/// series. MUST match `struct EnvClimate` in 00-bindings-vertex.wgsl; the
/// lockstep test builds the shader's struct from this layout using the
/// shader's OWN member offsets, so a mismatch fails there rather than reading
/// the wrong floats.
pub const GPU_FLOATS: usize = 5 * 4 + 6 * WIND_TERMS;

/// One world's climate. RON field docs live in the data file's header.
#[derive(Debug, Clone, Deserialize)]
pub struct ClimateRow {
    pub body: String,
    pub sea_level_mean_c: f64,
    pub p2_c: f64,
    pub north_winter_solstice_year_fraction: f64,
    pub season_north_sea: (f64, f64),
    pub season_north_land: (f64, f64),
    pub season_south_sea: (f64, f64),
    pub season_south_land: (f64, f64),
    pub surface_pressure_kpa: f64,
    pub gravity_ms2: f64,
    pub molar_mass_kg_per_mol: f64,
    pub geopotential_radius_km: f64,
    pub lapse_k_per_km: f64,
    pub lapse_break_km: f64,
    pub upper_lapse_k_per_km: f64,
    #[serde(default)]
    pub wind_u_mean: Vec<f64>,
    #[serde(default)]
    pub wind_u_cos: Vec<f64>,
    #[serde(default)]
    pub wind_u_sin: Vec<f64>,
    #[serde(default)]
    pub wind_v_mean: Vec<f64>,
    #[serde(default)]
    pub wind_v_cos: Vec<f64>,
    #[serde(default)]
    pub wind_v_sin: Vec<f64>,
}

/// Every world's row.
#[derive(Debug, Clone, Deserialize)]
pub struct ClimateTable {
    pub worlds: Vec<ClimateRow>,
}

/// The air at one place and time, as Layer 1 gives it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AirAt {
    /// Air temperature at the altitude, C.
    pub temp_c: f64,
    /// Air pressure at the altitude, kPa.
    pub pressure_kpa: f64,
    /// Prevailing wind toward the east, m/s.
    pub wind_east: f64,
    /// Prevailing wind toward the north, m/s.
    pub wind_north: f64,
}

impl AirAt {
    /// Prevailing wind speed, m/s.
    pub fn wind_speed(&self) -> f64 {
        self.wind_east.hypot(self.wind_north)
    }
}

impl ClimateTable {
    /// Parse and validate the RON text.
    pub fn from_ron(src: &str) -> Result<Self, String> {
        let t: ClimateTable = ron::from_str(src).map_err(|e| format!("climate.ron: {e}"))?;
        t.validate()?;
        Ok(t)
    }

    /// Byte-oriented parse, the shape `engine::registries` loads every RON
    /// table through (disk first for modding, embedded fallback).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let src = std::str::from_utf8(bytes).map_err(|e| format!("climate.ron is not utf-8: {e}"))?;
        Self::from_ron(src)
    }

    /// The table compiled into the binary (`embedded_data::CLIMATE_RON`), for a
    /// reader that finds none in the DataStore (unit tests, a headless build).
    /// None only if the embedded copy fails to parse, which the shipped-data
    /// test forbids.
    pub fn shipped() -> Option<&'static ClimateTable> {
        static TABLE: std::sync::OnceLock<Option<ClimateTable>> = std::sync::OnceLock::new();
        TABLE
            .get_or_init(|| match ClimateTable::from_ron(crate::embedded_data::CLIMATE_RON) {
                Ok(t) => Some(t),
                Err(e) => {
                    log::error!("embedded {DATA_FILE} failed to load: {e}");
                    None
                }
            })
            .as_ref()
    }

    /// The row for a body id, if the world has one.
    pub fn row(&self, body: &str) -> Option<&ClimateRow> {
        self.worlds.iter().find(|w| w.body == body)
    }

    fn validate(&self) -> Result<(), String> {
        let mut seen = std::collections::HashSet::new();
        for w in &self.worlds {
            if !seen.insert(w.body.as_str()) {
                return Err(format!("climate.ron: body '{}' has two rows", w.body));
            }
            w.validate()?;
        }
        Ok(())
    }
}

impl ClimateRow {
    fn validate(&self) -> Result<(), String> {
        let b = &self.body;
        if b.is_empty() {
            return Err("climate.ron: a row has no body id".into());
        }
        for (name, list) in self.wind_lists() {
            if list.len() > WIND_TERMS {
                return Err(format!(
                    "climate.ron '{b}': {name} has {} terms; the shader twin evaluates {WIND_TERMS}",
                    list.len()
                ));
            }
        }
        if self.surface_pressure_kpa < 0.0 {
            return Err(format!("climate.ron '{b}': negative surface pressure"));
        }
        if self.surface_pressure_kpa > 0.0
            && (self.gravity_ms2 <= 0.0 || self.molar_mass_kg_per_mol <= 0.0)
        {
            return Err(format!("climate.ron '{b}': air needs a positive gravity and molar mass"));
        }
        if self.geopotential_radius_km <= 0.0 || self.lapse_break_km < 0.0 {
            return Err(format!("climate.ron '{b}': radius must be positive, the lapse break not negative"));
        }
        let all = [
            self.sea_level_mean_c,
            self.p2_c,
            self.north_winter_solstice_year_fraction,
            self.season_north_sea.0,
            self.season_north_sea.1,
            self.season_north_land.0,
            self.season_north_land.1,
            self.season_south_sea.0,
            self.season_south_sea.1,
            self.season_south_land.0,
            self.season_south_land.1,
            self.lapse_k_per_km,
            self.upper_lapse_k_per_km,
        ];
        if all.iter().any(|v| !v.is_finite())
            || self.wind_lists().iter().any(|(_, l)| l.iter().any(|v| !v.is_finite()))
        {
            return Err(format!("climate.ron '{b}': a number is not finite"));
        }
        Ok(())
    }

    fn wind_lists(&self) -> [(&'static str, &Vec<f64>); 6] {
        [
            ("wind_u_mean", &self.wind_u_mean),
            ("wind_u_cos", &self.wind_u_cos),
            ("wind_u_sin", &self.wind_u_sin),
            ("wind_v_mean", &self.wind_v_mean),
            ("wind_v_cos", &self.wind_v_cos),
            ("wind_v_sin", &self.wind_v_sin),
        ]
    }

    /// The seasonal phase, radians: 2 pi times the years since the northern
    /// winter solstice. Twin of `env_l1_season_angle`.
    pub fn season_angle(&self, year_fraction: f64) -> f64 {
        let tau = year_fraction - self.north_winter_solstice_year_fraction;
        std::f64::consts::TAU * (tau - tau.floor())
    }

    /// Sea-level air temperature, C, at `sin_lat` (the direction's y, since
    /// bodies spin about +Y), with `land` the share of land around the place
    /// (0 open sea, 1 deep inland). Twin of `env_l1_sea_level_temp_c`.
    pub fn sea_level_temp_c(&self, sin_lat: f64, land: f64, year_fraction: f64) -> f64 {
        let x = sin_lat.clamp(-1.0, 1.0);
        let land = land.clamp(0.0, 1.0);
        let (sea, lnd) = if x >= 0.0 {
            (self.season_north_sea, self.season_north_land)
        } else {
            (self.season_south_sea, self.season_south_land)
        };
        let a = sea.0 + (lnd.0 - sea.0) * land;
        let b = sea.1 + (lnd.1 - sea.1) * land;
        let ang = self.season_angle(year_fraction);
        let p2 = 1.5 * x * x - 0.5;
        self.sea_level_mean_c + self.p2_c * p2 + x * (a * ang.cos() + b * ang.sin())
    }

    /// The air column: (temperature K, pressure kPa) at geometric altitude
    /// `alt_m` above the datum, for a column whose sea-level temperature is
    /// `t_sea_level_k`. The 1976 US Standard Atmosphere's two lowest layers,
    /// generalised: geopotential height H = h / (1 + h / r0); below the break,
    /// T = Ts - L1 H and p = p0 (T / Ts)^(gM / (R L1)); above it the same with
    /// the upper lapse from the break's values, or an exponential where a layer
    /// is isothermal. Twin of `env_l1_column`.
    pub fn column(&self, t_sea_level_k: f64, alt_m: f64) -> (f64, f64) {
        let r0 = self.geopotential_radius_km * 1000.0;
        let h = alt_m / (1.0 + alt_m / r0);
        let k = self.gravity_ms2 * self.molar_mass_kg_per_mol / GAS_CONSTANT;
        let hb = self.lapse_break_km * 1000.0;
        let l1 = self.lapse_k_per_km / 1000.0;
        let l2 = self.upper_lapse_k_per_km / 1000.0;
        let ts = t_sea_level_k.max(MIN_COLUMN_TEMP_K);
        let hl = h.min(hb);
        let t1 = (ts - l1 * hl).max(MIN_COLUMN_TEMP_K);
        let p1 = if l1.abs() > ISOTHERMAL_LAPSE {
            self.surface_pressure_kpa * (t1 / ts).powf(k / l1)
        } else {
            self.surface_pressure_kpa * (-k * hl / ts).exp()
        };
        if h <= hb {
            return (t1, p1);
        }
        let hu = h - hb;
        let t2 = (t1 - l2 * hu).max(MIN_COLUMN_TEMP_K);
        let p2 = if l2.abs() > ISOTHERMAL_LAPSE {
            p1 * (t2 / t1).powf(k / l2)
        } else {
            p1 * (-k * hu / t1).exp()
        };
        (t2, p2)
    }

    /// Prevailing wind (toward east, toward north), m/s, at unit direction
    /// `dir`. Twin of `env_l1_wind_en`.
    pub fn wind_east_north(&self, dir: DVec3, year_fraction: f64) -> (f64, f64) {
        let s = sine_series_basis(dir.y);
        let ang = self.season_angle(year_fraction);
        let (c, sn) = (ang.cos(), ang.sin());
        let mut u = 0.0;
        let mut v = 0.0;
        for (n, basis) in s.iter().enumerate() {
            let term = |list: &Vec<f64>| list.get(n).copied().unwrap_or(0.0);
            u += basis * (term(&self.wind_u_mean) + term(&self.wind_u_cos) * c + term(&self.wind_u_sin) * sn);
            v += basis * (term(&self.wind_v_mean) + term(&self.wind_v_cos) * c + term(&self.wind_v_sin) * sn);
        }
        (u, v)
    }

    /// Everything Layer 1 says about the air at a place and time. `dir` must be
    /// unit length; `land` is the share of land around the place (0..1).
    pub fn air_at(&self, dir: DVec3, alt_m: f64, land: f64, year_fraction: f64) -> AirAt {
        let ts = self.sea_level_temp_c(dir.y, land, year_fraction) + KELVIN;
        let (t, p) = self.column(ts, alt_m);
        let (e, n) = self.wind_east_north(dir, year_fraction);
        AirAt { temp_c: t - KELVIN, pressure_kpa: p, wind_east: e, wind_north: n }
    }

    /// The row packed as the shader's `EnvClimate` reads it, for the first GPU
    /// consumer's upload and for the lockstep test. Order is the WGSL struct's.
    pub fn pack_gpu(&self) -> [f32; GPU_FLOATS] {
        let mut out = [0.0f32; GPU_FLOATS];
        let head: [f64; 20] = [
            self.sea_level_mean_c,
            self.p2_c,
            self.north_winter_solstice_year_fraction,
            0.0,
            self.season_north_sea.0,
            self.season_north_sea.1,
            self.season_north_land.0,
            self.season_north_land.1,
            self.season_south_sea.0,
            self.season_south_sea.1,
            self.season_south_land.0,
            self.season_south_land.1,
            self.surface_pressure_kpa,
            self.gravity_ms2 * self.molar_mass_kg_per_mol / GAS_CONSTANT,
            self.geopotential_radius_km * 1000.0,
            self.lapse_break_km * 1000.0,
            self.lapse_k_per_km / 1000.0,
            self.upper_lapse_k_per_km / 1000.0,
            0.0,
            0.0,
        ];
        for (o, v) in out.iter_mut().zip(head.iter()) {
            *o = *v as f32;
        }
        for (set, (_, list)) in self.wind_lists().iter().enumerate() {
            for (n, v) in list.iter().enumerate() {
                out[20 + set * WIND_TERMS + n] = *v as f32;
            }
        }
        out
    }
}

/// sin(n theta) for n = 1..16, theta the colatitude, from sin(latitude) alone:
/// cos(theta) = sin_lat and sin(theta) = cos(latitude), then the Chebyshev
/// recurrence sin((n+1) t) = 2 cos(t) sin(n t) - sin((n-1) t). The shader twin
/// runs the SAME recurrence (no trigonometry at all), so the two agree to f32
/// rounding rather than to the accuracy of two different sine routines.
pub fn sine_series_basis(sin_lat: f64) -> [f64; WIND_TERMS] {
    let y = sin_lat.clamp(-1.0, 1.0);
    let c2 = 2.0 * y;
    let mut s = [0.0; WIND_TERMS];
    s[0] = (1.0 - y * y).max(0.0).sqrt();
    s[1] = c2 * s[0];
    for n in 2..WIND_TERMS {
        s[n] = c2 * s[n - 1] - s[n - 2];
    }
    s
}

/// Radius of the land-share sample ring, degrees of arc (about 111 km on Earth).
pub const LAND_RING_DEG: f64 = 1.0;

/// The share of land around a place, 0 (open sea) to 1 (deep inland), from any
/// land/sea lookup (`is_ocean(lat_deg, lon_deg)`). Why a share and not a yes
/// or no: the seasonal land and sea coefficients were fitted on reanalysis
/// cells 1.9 degrees across, each counted as land when it was mostly land, so
/// what they describe is a NEIGHBOURHOOD. Nine samples, the place and eight on
/// a ring `LAND_RING_DEG` around it, approximate one; stepping onto a beach
/// moves the answer by a ninth rather than from sea to continent at once.
pub fn land_fraction_around(lat_deg: f64, lon_deg: f64, is_ocean: impl Fn(f64, f64) -> bool) -> f32 {
    let mut land = if is_ocean(lat_deg, lon_deg) { 0 } else { 1 };
    // Longitude degrees per degree of arc grow toward the poles; capped so the
    // ring stays finite right at a pole (where every longitude is one place).
    let lon_scale = 1.0 / lat_deg.to_radians().cos().max(0.05);
    for k in 0..8 {
        let bearing = (k as f64 * 45.0).to_radians();
        let lat = (lat_deg + LAND_RING_DEG * bearing.cos()).clamp(-90.0, 90.0);
        let lon = lon_deg + LAND_RING_DEG * bearing.sin() * lon_scale;
        if !is_ocean(lat, lon) {
            land += 1;
        }
    }
    land as f32 / 9.0
}

/// Unit east and north at a unit direction in the body frame, for a body
/// spinning about +Y with the house latitude/longitude handedness
/// (`terrain::planet_heightmap::latlon_to_dir`: east is the direction of
/// increasing longitude). None at a pole, where east does not exist.
pub fn east_north(dir: DVec3) -> Option<(DVec3, DVec3)> {
    let e = DVec3::new(dir.z, 0.0, -dir.x);
    let len = e.length();
    if len < 1e-12 {
        return None;
    }
    let east = e / len;
    Some((east, dir.cross(east)))
}

/// The prevailing wind as a vector in the body frame, m/s, tangent to the
/// surface at `dir`. Twin of `env_l1_wind_body`: the form a GPU consumer (cloud
/// advection) wants.
pub fn wind_body(row: &ClimateRow, dir: DVec3, year_fraction: f64) -> DVec3 {
    let (u, v) = row.wind_east_north(dir, year_fraction);
    match east_north(dir) {
        Some((east, north)) => east * u + north * v,
        None => DVec3::ZERO,
    }
}

#[cfg(test)]
#[path = "env_layer1_tests.rs"]
mod tests;
