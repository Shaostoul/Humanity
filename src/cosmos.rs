//! Canonical Sol-system model (v0.262.8 — extracted from
//! `gui/pages/cosmos.rs` so it is the SINGLE source of truth).
//!
//! Before this module the solar system existed as **four drifted
//! copies**: this Keplerian model (Maps page, accurate), a log-scaled
//! `data/world/solar_system.ron` (the in-home hologram), a circular
//! approximation in `terrain/planet_registry.rs` (never instantiated;
//! deleted 2026-08-12, artificial-planet increment 5), and a hardcoded
//! JS array in `web/pages/maps.html`. The operator asked us to "sync the
//! maps". Per the project **infinite-of-x** rule the solar system is
//! data, not code-duplicated-per-view: there is ONE `SolBody` set
//! loaded from `data/star_systems/sol.json`, ONE Kepler propagator, and
//! every view (Maps page, FPS world, in-home orrery) consumes it at its
//! own scale. This file is engine-wide (NOT `#[cfg(native)]`) so the
//! renderer / terrain / world spawn can call it the same way the GUI
//! does.
//!
//! Nothing here touches egui — it is pure data + orbital mechanics.
//! The Maps page (`gui/pages/cosmos.rs`) now `use`s these symbols
//! instead of defining its own; behavior is byte-for-byte identical
//! (same struct, same fn signatures, same `&'static` caching).

use std::sync::OnceLock;

/// Kilometres per astronomical unit (IAU 2012 definition, rounded as the
/// dataset uses it). Moons store `semi_major_axis_km`; dividing by this
/// converts to the AU the propagator works in.
pub const KM_PER_AU: f64 = 149_597_870.7;

/// Exact metres per astronomical unit (IAU). Used by world-space
/// consumers (FPS spawn, floating origin) to turn the AU positions this
/// module returns into engine metres. `KM_PER_AU * 1000` rounds the
/// last digits, so keep the canonical metre value separate.
pub const M_PER_AU: f64 = 149_597_870_700.0;

/// One Sol-system body: orbital elements + physical params + the browser
/// metadata the Maps page details panel shows. Loaded from
/// `data/star_systems/sol.json`. Fields are `pub` so every consumer
/// (GUI page, renderer, terrain, world spawn) reads them directly.
#[derive(Debug, Clone)]
pub struct SolBody {
    pub id: String,
    pub name: String,
    pub body_type: String,
    /// Parent body id (e.g. "sun" for planets, "earth" for "moon").
    pub parent: Option<String>,
    /// Semi-major axis in AU (only set for direct sun-orbiters).
    pub semi_major_axis_au: f64,
    /// Semi-major axis in km (only set for moons orbiting their planet).
    pub semi_major_axis_km: f64,
    /// Orbital eccentricity. 0 = circle, 0..1 = ellipse, 1 = parabolic
    /// escape, >1 = hyperbolic flyby.
    pub eccentricity: f64,
    /// Orbital inclination in degrees (tilt of the orbit plane relative
    /// to the reference plane — ecliptic for Sol-orbiters).
    pub inclination_deg: f64,
    /// Longitude of the ascending node in degrees — where the orbit
    /// crosses the reference plane going north.
    pub longitude_ascending_node_deg: f64,
    /// Argument of periapsis in degrees — angle from ascending node to
    /// the periapsis point.
    pub argument_perihelion_deg: f64,
    /// Mean anomaly at epoch (J2000) in degrees. Combined with
    /// `orbital_period_days` + sim_time, gives the body's snapshot
    /// position.
    pub mean_anomaly_deg: f64,
    /// Body radius in km — for visual sizing.
    pub radius_km: f64,
    /// Mass in kg.
    pub mass_kg: f64,
    /// Surface gravity in m/s².
    pub surface_gravity_ms2: f64,
    /// Mean surface / cloud-top temperature in Kelvin.
    pub mean_temperature_k: f64,
    /// Global magnetic field strength at the surface (equatorial) in tesla.
    /// 0.0 when the body has no significant global field (Mars, Venus, the
    /// Moon) or when the dataset does not list one. First consumers are the
    /// body info readout and a compass; radiation shielding comes later
    /// (artificial-planet gap 4).
    pub magnetic_field_t: f64,
    /// Surface atmospheric pressure in pascals. 0.0 means effectively vacuum
    /// OR undefined: gas giants have no surface to measure at, so they stay
    /// at 0.0 by convention (their `surface_pressure_atm` in the JSON is
    /// null for the same reason).
    pub surface_pressure_pa: f64,
    /// Orbital period in days.
    pub orbital_period_days: f64,
    /// Atmosphere composition summary (top 3 components, formatted).
    /// Empty string if no atmosphere.
    pub atmosphere_summary: String,
    /// Free-form description (1-2 sentences).
    pub description: String,
    /// Discovery year, if known. 0 = ancient / no record.
    pub discovery_year: i32,
    /// Discoverer name, if known.
    pub discoverer: String,
    /// IDs of bodies orbiting this one (e.g. moons of a planet).
    pub children: Vec<String>,
}

static SOL_BODIES: OnceLock<Vec<SolBody>> = OnceLock::new();

/// Raw JSON text for the Sol system: DISK FIRST, embedded fallback.
///
/// Disk-first (`<data_dir>/star_systems/sol.json`) means adding a body or
/// editing a mass is a pure data drop, no rebuild (artificial-planet gap 7).
/// The embedded copy keeps a bare portable exe (no data/ folder next to it)
/// fully working. Decision order (review fixes 2026-08-12):
///
/// 1. No disk file: embedded, silently. The normal portable-exe case.
/// 2. Disk file unusable as a catalog (JSON parse error, or no non-empty
///    `bodies` array): embedded, with a warning. Before this check a
///    malformed disk file silently produced a ZERO-body catalog, because
///    `parse_sol_bodies` swallows parse errors by design.
/// 3. Disk `catalog_version` OLDER than the embedded one: embedded, with an
///    info log. Installed builds extract data/ exactly once and never
///    refresh it (`extract_data_if_needed`), so without this gate a stale
///    install's July-era 64-body copy would shadow the shipped 69-body
///    catalog forever. A missing `catalog_version` counts as 0, which is
///    what every pre-versioning disk copy looks like. Operator hand-tuning
///    keeps working: editing the extracted file preserves its shipped
///    version number, and same-or-newer versions win.
/// 4. Otherwise the disk copy wins.
///
/// Future work: live hot-reload of orbital data via the file watcher plus a
/// cache bust; today the OnceLock in `sol_bodies()` means the file is read
/// once per run, so an edit needs a restart (not a rebuild).
fn load_sol_json(data_dir: &std::path::Path) -> String {
    let embedded = crate::embedded_data::SOLAR_SYSTEM_JSON;
    let disk_path = data_dir.join("star_systems").join("sol.json");
    let disk = match std::fs::read_to_string(&disk_path) {
        Ok(s) => s,
        // Absent or unreadable file: the bare-portable-exe path, no log
        // noise. (A permissions error lands here too; acceptable, since the
        // embedded copy is always complete.)
        Err(_) => return embedded.to_string(),
    };
    match sol_json_catalog_version(&disk) {
        Err(why) => {
            log::warn!(
                "Cosmos: on-disk {} is unusable ({why}); using the embedded catalog instead",
                disk_path.display()
            );
            embedded.to_string()
        }
        Ok(disk_version) => {
            // unwrap_or(0): the embedded copy is compiled from this same
            // repo file and always validates today; if a future edit ever
            // broke it we would rather compare against 0 (letting any valid
            // disk copy win) than panic at startup.
            let embedded_version = sol_json_catalog_version(embedded).unwrap_or(0);
            if disk_version < embedded_version {
                log::info!(
                    "Cosmos: on-disk catalog at {} is version {disk_version}, older than the shipped version {embedded_version}; using the embedded catalog (delete the file to stop this message, or re-copy the shipped one to hand-tune it)",
                    disk_path.display()
                );
                embedded.to_string()
            } else {
                disk
            }
        }
    }
}

/// Validate candidate sol.json text and extract its `catalog_version`.
///
/// `Ok(version)` when the text parses as JSON AND carries a non-empty
/// `bodies` array. A missing `catalog_version` key maps to version 0 (every
/// disk copy extracted before the versioning gate existed looks like that).
/// `Err(reason)` when the text cannot serve as a catalog at all, so the
/// caller must fall back to the embedded copy.
fn sol_json_catalog_version(json: &str) -> Result<i64, String> {
    let parsed: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("JSON parse error: {e}"))?;
    match parsed.get("bodies").and_then(|b| b.as_array()) {
        Some(arr) if !arr.is_empty() => {
            Ok(parsed.get("catalog_version").and_then(|v| v.as_i64()).unwrap_or(0))
        }
        Some(_) => Err("catalog has an empty bodies array".to_string()),
        None => Err("catalog has no bodies array".to_string()),
    }
}

/// Parse + cache the Sol system into `SolBody` rows (with parent→children
/// links). Loads via `load_sol_json` (disk first, embedded fallback).
/// `region` rows (asteroid belts) are skipped: they are not positionable
/// point bodies. Cached `&'static` so per-frame UI / render code can call
/// it freely.
pub fn sol_bodies() -> &'static [SolBody] {
    SOL_BODIES.get_or_init(|| {
        let json = load_sol_json(&crate::data_dir());
        let out = parse_sol_bodies(&json);
        log::info!("Cosmos: loaded {} Sol bodies (with parent-child links)", out.len());
        out
    })
}

/// The pure parse step, split from `sol_bodies()` so tests can feed it an
/// edited JSON copy without fighting the process-wide OnceLock cache.
fn parse_sol_bodies(json: &str) -> Vec<SolBody> {
    // `load_sol_json` has already validated any disk text before it reaches
    // here, so a parse failure can only come from a test feeding garbage on
    // purpose; Null then yields an empty catalog rather than a panic.
    let parsed: serde_json::Value = serde_json::from_str(json).unwrap_or(serde_json::Value::Null);
    let mut out: Vec<SolBody> = Vec::new();
    if let Some(arr) = parsed.get("bodies").and_then(|b| b.as_array()) {
        for body in arr {
            let id = body.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let name = body.get("name").and_then(|v| v.as_str()).unwrap_or(&id).to_string();
            let body_type = body.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if body_type == "region" { continue; } // skip belts as positionable bodies
            let parent = body.get("parent").and_then(|v| v.as_str()).map(String::from);
            let orbit = body.get("orbit");
            let semi_major_axis_au = orbit.and_then(|o| o.get("semi_major_axis_au")).and_then(|v| v.as_f64()).unwrap_or(0.0);
            let semi_major_axis_km = orbit.and_then(|o| o.get("semi_major_axis_km")).and_then(|v| v.as_f64()).unwrap_or(0.0);
            let orbital_period_days = orbit.and_then(|o| o.get("orbital_period_days")).and_then(|v| v.as_f64()).unwrap_or(0.0);
            let eccentricity = orbit.and_then(|o| o.get("eccentricity")).and_then(|v| v.as_f64()).unwrap_or(0.0);
            let inclination_deg = orbit.and_then(|o| o.get("inclination_deg")).and_then(|v| v.as_f64()).unwrap_or(0.0);
            let longitude_ascending_node_deg = orbit
                .and_then(|o| o.get("longitude_ascending_node_deg"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let argument_perihelion_deg = orbit
                .and_then(|o| o.get("argument_perihelion_deg"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let mean_anomaly_deg = orbit
                .and_then(|o| o.get("mean_anomaly_deg"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let physical = body.get("physical");
            let radius_km = physical.and_then(|p| p.get("radius_km")).and_then(|v| v.as_f64()).unwrap_or(1000.0);
            let mass_kg = physical.and_then(|p| p.get("mass_kg")).and_then(|v| v.as_f64()).unwrap_or(0.0);
            let surface_gravity_ms2 = physical.and_then(|p| p.get("surface_gravity_ms2")).and_then(|v| v.as_f64()).unwrap_or(0.0);
            let mean_temperature_k = physical.and_then(|p| p.get("mean_temperature_k")).and_then(|v| v.as_f64()).unwrap_or(0.0);
            // Optional physical-profile fields (artificial-planet gap 4):
            // absent for most minor bodies, so they default to 0.0 the
            // same way the other physical params do.
            let magnetic_field_t = physical.and_then(|p| p.get("magnetic_field_t")).and_then(|v| v.as_f64()).unwrap_or(0.0);
            // Surface pressure lives in the catalog TWICE: the new sparse
            // physical.surface_pressure_pa (pascals, only the majors carry
            // it so far) and the older atmosphere.surface_pressure_atm
            // (standard atmospheres, ~21 bodies, e.g. Titan 1.47 atm).
            // Primary first, then the atm fallback converted at the standard
            // atmosphere (101325 Pa). Without the fallback Titan parsed as
            // 0.0 Pa, i.e. hard vacuum on the one moon famous for its thick
            // atmosphere. Gas giants store null atm (no surface to measure
            // at), and as_f64() on null is None, so they stay at 0.0.
            let surface_pressure_pa = physical.and_then(|p| p.get("surface_pressure_pa")).and_then(|v| v.as_f64())
                .or_else(|| body.get("atmosphere")
                    .and_then(|a| a.get("surface_pressure_atm"))
                    .and_then(|v| v.as_f64())
                    .map(|atm| atm * 101_325.0))
                .unwrap_or(0.0);
            // Build a compact atmosphere summary from the composition map
            // ("78% N₂ · 21% O₂ · …"). Empty string if no atmosphere.
            let atmosphere_summary = body.get("atmosphere")
                .and_then(|a| a.get("composition"))
                .and_then(|c| c.as_object())
                .map(|comp| {
                    let mut pairs: Vec<(String, f64)> = comp.iter()
                        .filter_map(|(k, v)| Some((k.clone(), v.as_f64()?)))
                        .collect();
                    // Highest concentration first.
                    pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                    pairs.iter().take(3)
                        .map(|(k, v)| format!("{:.1}% {}", v, k))
                        .collect::<Vec<_>>()
                        .join(" · ")
                })
                .unwrap_or_default();
            let description = body.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let (discovery_year, discoverer) = body.get("discovery")
                .and_then(|d| {
                    let y = d.get("year").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    let who = d.get("discoverer").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    Some((y, who))
                })
                .unwrap_or((0, String::new()));
            out.push(SolBody {
                id, name, body_type, parent,
                semi_major_axis_au, semi_major_axis_km,
                eccentricity, inclination_deg,
                longitude_ascending_node_deg, argument_perihelion_deg, mean_anomaly_deg,
                orbital_period_days,
                radius_km, mass_kg, surface_gravity_ms2, mean_temperature_k,
                magnetic_field_t, surface_pressure_pa,
                atmosphere_summary, description, discovery_year, discoverer,
                children: Vec::new(), // populated in second pass below
            });
        }
    }
    // Second pass: populate `children` lists by inverting the parent
    // relationship. This is what lets the body browser sidebar nest
    // moons under their planet without re-scanning every frame.
    let mut child_lists: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for b in &out {
        if let Some(p) = &b.parent {
            child_lists.entry(p.clone()).or_default().push(b.id.clone());
        }
    }
    for b in &mut out {
        if let Some(kids) = child_lists.get(&b.id) {
            b.children = kids.clone();
        }
    }
    out
}

/// Look up a body by id. O(N) scan, but the dataset is ~64 entries so
/// it's fine to call this from per-frame UI / render code.
pub fn find_body(id: &str) -> Option<&'static SolBody> {
    sol_bodies().iter().find(|b| b.id == id)
}

/// Solve Kepler's equation `M = E - e*sin(E)` for eccentric anomaly E
/// given mean anomaly M (radians) and eccentricity e (0..1).
/// Newton-Raphson iteration; converges in ~5 iterations for e < 0.9.
pub fn kepler_solve(mean_anom: f64, ecc: f64) -> f64 {
    let mut e_anom = mean_anom;
    for _ in 0..12 {
        let delta = (e_anom - ecc * e_anom.sin() - mean_anom) / (1.0 - ecc * e_anom.cos());
        e_anom -= delta;
        if delta.abs() < 1e-12 { break; }
    }
    e_anom
}

/// Compute a body's position relative to its parent, in AU. Applies real
/// Kepler orbital mechanics — eccentricity, inclination, argument of
/// perihelion, longitude of ascending node, mean anomaly at epoch +
/// mean motion × sim_time.
///
/// `sim_time_seconds` is seconds since the J2000.0 epoch
/// (2000-01-01 12:00:00 UTC). Pass 0 for the snapshot configuration
/// (used by orbit-line sampling so the line itself doesn't slither as
/// the user scrubs time). For LIVE body positions, pass the cosmos
/// sim_time so mean anomaly advances at `360 / orbital_period_days`
/// degrees per day.
pub fn body_position_relative_au(body: &SolBody, sim_time_seconds: f64) -> glam::DVec3 {
    let a_au = if body.semi_major_axis_au > 0.0 {
        body.semi_major_axis_au
    } else if body.semi_major_axis_km > 0.0 {
        body.semi_major_axis_km / KM_PER_AU
    } else {
        return glam::DVec3::ZERO;
    };
    let e = body.eccentricity.clamp(0.0, 0.99);
    // Mean anomaly at epoch J2000 — from data if present, else hashed
    // from name so untagged minor bodies don't all start at periapsis.
    let m0_deg = if body.mean_anomaly_deg != 0.0 {
        body.mean_anomaly_deg
    } else {
        (body.name.bytes().fold(0u64, |a, b| a.wrapping_add(b as u64)) % 360) as f64
    };
    // Advance by sim_time. Mean motion = 360 deg / orbital_period.
    // Bodies without an orbital_period_days value stay at their epoch
    // anomaly (Phase 4d may estimate it from Kepler's third law later).
    let n_deg_per_sec = if body.orbital_period_days > 0.0 {
        360.0 / (body.orbital_period_days * 86_400.0)
    } else {
        0.0
    };
    let m_deg = (m0_deg + n_deg_per_sec * sim_time_seconds).rem_euclid(360.0);
    let m_rad = m_deg.to_radians();
    let ea = kepler_solve(m_rad, e);
    // Perifocal coordinates: periapsis along +X of the orbital plane.
    //   x = a * (cos E - e)
    //   y = a * sqrt(1 - e²) * sin E
    let x_peri = a_au * (ea.cos() - e);
    let y_peri = a_au * (1.0 - e * e).sqrt() * ea.sin();
    // 3-1-3 Euler rotation: Rz(Ω) · Rx(i) · Rz(ω) applied to the
    // perifocal (x, y, 0) vector. Combined rotation matrix entries:
    let big_omega = body.longitude_ascending_node_deg.to_radians();
    let inc = body.inclination_deg.to_radians();
    let small_omega = body.argument_perihelion_deg.to_radians();
    let (s_om, c_om) = big_omega.sin_cos();
    let (s_inc, c_inc) = inc.sin_cos();
    let (s_w, c_w) = small_omega.sin_cos();
    let r11 = c_om * c_w - s_om * s_w * c_inc;
    let r12 = -c_om * s_w - s_om * c_w * c_inc;
    let r21 = s_om * c_w + c_om * s_w * c_inc;
    let r22 = -s_om * s_w + c_om * c_w * c_inc;
    let r31 = s_w * s_inc;
    let r32 = c_w * s_inc;
    // World convention: Y is up, ecliptic plane is XZ. Map perifocal X→X,
    // perifocal Y→Z, perifocal Z (always 0 here) drops out. Out-of-plane
    // component ends up in world Y via r31/r32.
    let world_x = r11 * x_peri + r12 * y_peri;
    let world_z = r21 * x_peri + r22 * y_peri;
    let world_y = r31 * x_peri + r32 * y_peri;
    glam::DVec3::new(world_x, world_y, world_z)
}

/// Compute world position in AU including parent recursion. Moons are
/// positioned relative to their parent planet, and the parent's own
/// position folds in. Recursion bottoms out at Sun (position = origin).
/// `sim_time_seconds` is passed through to every level so parent +
/// child positions are synchronized in time.
pub fn body_world_position_3d_au(body: &SolBody, sim_time_seconds: f64) -> glam::DVec3 {
    let local = body_position_relative_au(body, sim_time_seconds);
    if let Some(parent_id) = &body.parent {
        if parent_id == "sun" {
            local
        } else if let Some(parent) = find_body(parent_id) {
            body_world_position_3d_au(parent, sim_time_seconds) + local
        } else {
            local
        }
    } else {
        local // Sun itself
    }
}

/// Sample a body's orbit at N points around the orbital ellipse, in the
/// PARENT's frame (parent at origin). Returns positions in AU.
/// Used by orbit-line rendering.
pub fn sample_orbit_points(body: &SolBody, n: usize) -> Vec<glam::DVec3> {
    let a_au = if body.semi_major_axis_au > 0.0 {
        body.semi_major_axis_au
    } else if body.semi_major_axis_km > 0.0 {
        body.semi_major_axis_km / KM_PER_AU
    } else {
        return Vec::new();
    };
    let e = body.eccentricity.clamp(0.0, 0.99);
    let big_omega = body.longitude_ascending_node_deg.to_radians();
    let inc = body.inclination_deg.to_radians();
    let small_omega = body.argument_perihelion_deg.to_radians();
    let (s_om, c_om) = big_omega.sin_cos();
    let (s_inc, c_inc) = inc.sin_cos();
    let (s_w, c_w) = small_omega.sin_cos();
    let r11 = c_om * c_w - s_om * s_w * c_inc;
    let r12 = -c_om * s_w - s_om * c_w * c_inc;
    let r21 = s_om * c_w + c_om * s_w * c_inc;
    let r22 = -s_om * s_w + c_om * c_w * c_inc;
    let r31 = s_w * s_inc;
    let r32 = c_w * s_inc;
    let mut out = Vec::with_capacity(n + 1);
    for i in 0..=n {
        // Sample uniformly in eccentric anomaly so high-e ellipses still
        // produce well-spaced points around the curve.
        let ea = (i as f64 / n as f64) * std::f64::consts::TAU;
        let x_peri = a_au * (ea.cos() - e);
        let y_peri = a_au * (1.0 - e * e).sqrt() * ea.sin();
        let wx = r11 * x_peri + r12 * y_peri;
        let wz = r21 * x_peri + r22 * y_peri;
        let wy = r31 * x_peri + r32 * y_peri;
        out.push(glam::DVec3::new(wx, wy, wz));
    }
    out
}

#[cfg(test)]
mod tests {
    //! Path note: `sol_bodies()` goes through `crate::data_dir()`, which in
    //! a test process finds no exe-adjacent data/ folder and falls back to
    //! plain "data" relative to the CURRENT WORKING DIRECTORY. So the tests
    //! below that call `sol_bodies()` / `find_body()` depend on the test
    //! runner being launched from a checkout root (cargo does this). The
    //! loader tests build their own temp dirs instead, with names embedding
    //! `std::process::id()` so parallel test runs from concurrent sessions
    //! (routine in this repo) cannot interleave in a shared fixed path.
    use super::*;

    /// Unique-per-process temp dir for loader tests. Different tests pass
    /// different `tag`s so they also cannot collide with each other inside
    /// one process (cargo runs test fns on parallel threads).
    fn loader_test_dir(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("hos_cosmos_{tag}_{}", std::process::id()))
    }

    #[test]
    fn sol_json_loads_core_bodies() {
        let b = sol_bodies();
        assert!(b.len() > 8, "expected the full Sol catalog, got {}", b.len());
        // The four bodies all three views must agree on.
        for id in ["sun", "earth", "mars", "moon"] {
            assert!(find_body(id).is_some(), "canonical model missing '{id}'");
        }
    }

    #[test]
    fn earth_is_about_one_au_from_sun() {
        let earth = find_body("earth").expect("earth in model");
        // At any sim time Earth's heliocentric distance is ~1 AU
        // (a=1.0, e≈0.0167 → 0.983..1.017 AU).
        for t in [0.0, 1.0e7, 7.5e6, 3.15e7] {
            let r = body_world_position_3d_au(earth, t).length();
            assert!(
                (0.95..1.05).contains(&r),
                "Earth heliocentric r={r} AU at t={t} — orbital math drifted"
            );
        }
    }

    /// Loader test half 1 (artificial-planet gap 7): when the data dir has
    /// NO star_systems/sol.json, the loader must hand back the embedded
    /// copy byte-for-byte. This is the portable-exe-in-a-bare-folder path.
    #[test]
    fn disk_load_falls_back_to_embedded_when_file_absent() {
        let dir = loader_test_dir("fallback_test");
        // Make sure the dir exists but is empty of star_systems/.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let json = load_sol_json(&dir);
        assert_eq!(
            json,
            crate::embedded_data::SOLAR_SYSTEM_JSON,
            "missing disk file must fall back to the embedded sol.json"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Loader test half 2: an edited on-disk sol.json OVERRIDES the embedded
    /// copy. Editing a mass or adding a body is a pure data drop, no rebuild.
    /// This is ALSO the same-catalog-version-wins half of the staleness gate:
    /// the disk copy below carries the embedded copy's own catalog_version,
    /// so version parity plus a disk file means the disk file wins.
    #[test]
    fn edited_disk_copy_overrides_embedded() {
        let dir = loader_test_dir("override_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("star_systems")).expect("temp dir");
        // Read the embedded catalog_version dynamically so bumping the
        // shipped catalog later does not silently strand this test on an
        // old number (a stale hardcoded version would LOSE the gate and
        // turn this into a confusing failure).
        let embedded_meta: serde_json::Value =
            serde_json::from_str(crate::embedded_data::SOLAR_SYSTEM_JSON).expect("embedded parses");
        let embedded_version = embedded_meta.get("catalog_version").and_then(|v| v.as_i64()).unwrap_or(0);
        // A minimal system: Earth with a deliberately WRONG mass (so we can
        // prove the disk value won) plus a body that does not exist in the
        // embedded catalog at all.
        let edited = r#"{
            "id": "sol-test",
            "catalog_version": CATVER,
            "bodies": [
                {
                    "id": "earth",
                    "name": "Earth",
                    "type": "terrestrial",
                    "parent": "sun",
                    "physical": { "mass_kg": 1.23e+25, "radius_km": 6371 }
                },
                {
                    "id": "testworld",
                    "name": "Testworld",
                    "type": "terrestrial",
                    "parent": "sun",
                    "physical": { "magnetic_field_t": 5e-5, "surface_pressure_pa": 200000 }
                }
            ]
        }"#.replace("CATVER", &embedded_version.to_string());
        std::fs::write(dir.join("star_systems").join("sol.json"), edited).expect("write");
        let bodies = parse_sol_bodies(&load_sol_json(&dir));
        assert_eq!(bodies.len(), 2, "disk copy (2 bodies) must win over embedded (~69)");
        let earth = bodies.iter().find(|b| b.id == "earth").expect("earth from disk");
        // Relative tolerance, not ==: serde_json's decimal-to-f64 parse can
        // land one ulp away from the equivalent Rust float literal.
        assert!(
            (earth.mass_kg - 1.23e25).abs() / 1.23e25 < 1e-12,
            "edited on-disk mass must override embedded (got {})", earth.mass_kg
        );
        let test = bodies.iter().find(|b| b.id == "testworld").expect("new body from disk");
        assert_eq!(test.magnetic_field_t, 5e-5);
        assert_eq!(test.surface_pressure_pa, 200_000.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Review fix 2026-08-12: a MALFORMED on-disk sol.json must not empty
    /// the catalog. Before the loader validated disk text, garbage JSON
    /// sailed through to `parse_sol_bodies`, whose unwrap_or(Null) turned it
    /// into ZERO bodies and failed 7 downstream tests (Maps, orrery, world
    /// spawn all read this catalog). Now the loader falls back to embedded
    /// with a warning.
    #[test]
    fn malformed_disk_copy_falls_back_to_embedded() {
        let dir = loader_test_dir("garbage_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("star_systems")).expect("temp dir");
        let embedded_count = parse_sol_bodies(crate::embedded_data::SOLAR_SYSTEM_JSON).len();
        // Case 1: not JSON at all (a half-written or corrupted install file).
        std::fs::write(dir.join("star_systems").join("sol.json"), "{ this is not json !!!").expect("write");
        let bodies = parse_sol_bodies(&load_sol_json(&dir));
        assert_eq!(
            bodies.len(), embedded_count,
            "garbage disk file must yield the full embedded catalog, not an empty one"
        );
        // Case 2: valid JSON that is not a catalog (no bodies array).
        std::fs::write(dir.join("star_systems").join("sol.json"), r#"{"id": "not-a-catalog"}"#).expect("write");
        let bodies = parse_sol_bodies(&load_sol_json(&dir));
        assert_eq!(
            bodies.len(), embedded_count,
            "a bodies-less disk file must yield the full embedded catalog"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Review fix 2026-08-12: a STALE installed data dir must not shadow a
    /// newer embedded catalog. Installed builds extract data/ exactly once
    /// (`extract_data_if_needed` never refreshes), so a July-era 64-body
    /// disk copy would otherwise hide the shipped 69-body catalog forever.
    /// Every pre-versioning disk copy has no catalog_version key, which the
    /// gate counts as version 0: older than shipped, so embedded wins. The
    /// version-parity-wins half lives in `edited_disk_copy_overrides_embedded`.
    #[test]
    fn stale_disk_copy_without_version_loses_to_embedded() {
        let dir = loader_test_dir("stale_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("star_systems")).expect("temp dir");
        // Structurally VALID catalog (parses, has bodies) but no
        // catalog_version key: exactly what an old install's extracted
        // copy looks like.
        let stale = r#"{
            "id": "sol-stale",
            "bodies": [
                { "id": "earth", "name": "Earth", "type": "terrestrial", "parent": "sun" }
            ]
        }"#;
        std::fs::write(dir.join("star_systems").join("sol.json"), stale).expect("write");
        let json = load_sol_json(&dir);
        assert_eq!(
            json,
            crate::embedded_data::SOLAR_SYSTEM_JSON,
            "a versionless (pre-gate) disk copy must lose to the shipped embedded catalog"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The new physical-profile fields parse from the shipped catalog, and
    /// bodies without them default to 0.0 instead of failing. Values are the
    /// NASA planetary fact sheet numbers written into sol.json this
    /// increment; if you retune the data, retune these assertions with it.
    #[test]
    fn physical_profile_fields_parse() {
        let bodies = parse_sol_bodies(crate::embedded_data::SOLAR_SYSTEM_JSON);
        let get = |id: &str| bodies.iter().find(|b| b.id == id).unwrap_or_else(|| panic!("{id} in catalog"));

        let earth = get("earth");
        assert!((earth.magnetic_field_t - 3.05e-5).abs() < 1e-9, "Earth equatorial surface field ~0.305 gauss");
        assert_eq!(earth.surface_pressure_pa, 101_325.0, "Earth 1 atm via physical.surface_pressure_pa");

        let mars = get("mars");
        assert_eq!(mars.magnetic_field_t, 0.0, "Mars has no global field");
        // The EXACT equality below doubles as a primary-beats-fallback
        // proof: mars stores physical.surface_pressure_pa = 610 AND
        // atmosphere.surface_pressure_atm = 0.006 (which would convert to
        // 607.95). Only the primary path produces exactly 610.0. Venus and
        // the Moon below work the same way (9.2e6 vs 9.3219e6; 3e-10 vs
        // 3.03975e-10).
        assert_eq!(mars.surface_pressure_pa, 610.0, "Mars ~6.1 mbar");

        let venus = get("venus");
        assert_eq!(venus.magnetic_field_t, 0.0, "Venus has no global field");
        assert_eq!(venus.surface_pressure_pa, 9.2e6, "Venus ~92 bar");

        // Titan carries NO physical.surface_pressure_pa; its pressure must
        // come from the older atmosphere.surface_pressure_atm = 1.47 atm via
        // the fallback conversion (1.47 * 101325 = 148947.75 Pa). Before the
        // fallback existed Titan parsed as 0.0 Pa: hard vacuum on the one
        // moon famous for its thick atmosphere.
        let titan = get("titan");
        assert!(
            (titan.surface_pressure_pa - 148_947.75).abs() < 0.5,
            "Titan ~1.47 atm via the atm fallback (got {})", titan.surface_pressure_pa
        );

        let jupiter = get("jupiter");
        assert!((jupiter.magnetic_field_t - 4.28e-4).abs() < 1e-9, "Jupiter cloud-top equatorial field");
        assert_eq!(jupiter.surface_pressure_pa, 0.0, "gas giants: surface pressure undefined, kept 0.0");

        let moon = get("moon");
        assert_eq!(moon.magnetic_field_t, 0.0);
        assert_eq!(moon.surface_pressure_pa, 3e-10, "lunar exosphere, effectively vacuum");

        let mercury = get("mercury");
        assert!((mercury.magnetic_field_t - 1.9e-7).abs() < 1e-12, "Mercury weak global field");

        // Unfilled bodies stay at the 0.0 default rather than erroring.
        let phobos = get("phobos");
        assert_eq!(phobos.magnetic_field_t, 0.0);
        assert_eq!(phobos.surface_pressure_pa, 0.0);

        // Presence guards (review fix 2026-08-12): every 0.0 assertion above
        // would ALSO pass if the key lookup itself broke (a renamed JSON key
        // makes every lookup miss and default to 0.0, and the test would
        // stay green). Pin the raw JSON shape: mars carries an EXPLICIT
        // magnetic_field_t and jupiter an EXPLICIT surface_pressure_pa (both
        // deliberately 0-valued in the data), while phobos carries NEITHER,
        // so its zeros above exercise the true absent-key default path.
        let raw: serde_json::Value =
            serde_json::from_str(crate::embedded_data::SOLAR_SYSTEM_JSON).expect("embedded sol.json parses");
        let phys = |id: &str| -> serde_json::Value {
            raw["bodies"].as_array().expect("bodies array").iter()
                .find(|b| b["id"] == id)
                .unwrap_or_else(|| panic!("{id} in raw catalog"))
                .get("physical").cloned()
                .unwrap_or(serde_json::Value::Null)
        };
        assert!(
            phys("mars").get("magnetic_field_t").is_some(),
            "sol.json: mars.physical must carry an explicit magnetic_field_t key (renamed?)"
        );
        assert!(
            phys("jupiter").get("surface_pressure_pa").is_some(),
            "sol.json: jupiter.physical must carry an explicit surface_pressure_pa key (renamed?)"
        );
        assert!(
            phys("phobos").get("magnetic_field_t").is_none()
                && phys("phobos").get("surface_pressure_pa").is_none(),
            "phobos should carry neither profile key so its 0.0 asserts test the absent-key default"
        );
    }

    #[test]
    fn moon_tracks_earth() {
        // The Moon's world position must stay within ~0.004 AU of Earth
        // (lunar orbit ≈ 384,400 km ≈ 0.00257 AU). This is the parent
        // recursion the FPS world relies on to place "home in high Earth
        // orbit".
        let earth = find_body("earth").expect("earth");
        let moon = find_body("moon").expect("moon");
        for t in [0.0, 5.0e6, 2.0e7] {
            let de = body_world_position_3d_au(earth, t);
            let dm = body_world_position_3d_au(moon, t);
            let sep = (dm - de).length();
            assert!(sep < 0.004, "Moon-Earth separation {sep} AU too large at t={t}");
        }
    }
}
