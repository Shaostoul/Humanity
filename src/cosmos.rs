//! Canonical Sol-system model (v0.262.8 — extracted from
//! the Maps page, then `gui/pages/cosmos.rs`, now `gui/pages/maps/`, so it is
//! the SINGLE source of truth).
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
//! The Maps page (`gui/pages/maps/`) now `use`s these symbols
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
    /// How the ascending node turns, as a SIGNED precession period in Julian
    /// years (the time it takes to go once round, the column JPL lists as
    /// "P node"): positive when the node advances, negative when it regresses.
    /// 0.0, or absent from the data, holds the node at its epoch value. The
    /// Moon's node regresses once in 18.6 years, so its value is -18.6.
    pub longitude_ascending_node_precession_years: f64,
    /// The same for the argument of periapsis (JPL's "P apsis"). The Moon's
    /// advances once in 6.0 years; with the node going the other way, its
    /// perigee (node plus argument) goes round once in 8.85 years.
    pub argument_perihelion_precession_years: f64,
    /// Mean anomaly at epoch (J2000) in degrees. Combined with
    /// `orbital_period_days` + sim_time, gives the body's snapshot
    /// position (`mean_anomaly_deg_at`).
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
    /// Sidereal orbital period in days: the time the mean longitude (node plus
    /// argument of periapsis plus mean anomaly) takes to go once round against
    /// the stars. When the node and the periapsis turn, the mean anomaly runs
    /// at the anomalistic rate, 360 / period less the two precession rates
    /// (`mean_anomaly_deg_at`), so the mean longitude still keeps this period.
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
    // Every return of the built-in catalog says so in the log
    // (embedded_data::note_builtin_copy): a rig refuses a run that served one,
    // since the tree's own sol.json is what a rebuild would carry (BUG-133).
    let disk_path = data_dir.join("star_systems").join("sol.json");
    let disk = match std::fs::read_to_string(&disk_path) {
        Ok(s) => s,
        // Absent or unreadable file: the bare-portable-exe path. (A
        // permissions error lands here too; acceptable, since the embedded
        // copy is always complete.)
        Err(e) => {
            crate::embedded_data::note_builtin_copy(
                "star_systems/sol.json",
                format_args!("{} could not be read ({e})", disk_path.display()),
            );
            return crate::embedded_data::SOLAR_SYSTEM_JSON.to_string();
        }
    };
    let embedded = crate::embedded_data::SOLAR_SYSTEM_JSON;
    match sol_json_catalog_version(&disk) {
        Err(why) => {
            crate::embedded_data::note_builtin_copy(
                "star_systems/sol.json",
                format_args!("{} is unusable ({why})", disk_path.display()),
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
                crate::embedded_data::note_builtin_copy(
                    "star_systems/sol.json",
                    format_args!(
                        "the catalog at {} is version {disk_version}, older than the shipped version {embedded_version} (delete the file to stop this message, or re-copy the shipped one to hand-tune it)",
                        disk_path.display()
                    ),
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
            // Optional secular rates: absent means the angle is held at epoch.
            let longitude_ascending_node_precession_years = orbit
                .and_then(|o| o.get("longitude_ascending_node_precession_years"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let argument_perihelion_precession_years = orbit
                .and_then(|o| o.get("argument_perihelion_precession_years"))
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
                longitude_ascending_node_deg, argument_perihelion_deg,
                longitude_ascending_node_precession_years, argument_perihelion_precession_years,
                mean_anomaly_deg,
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

/// Seconds in a Julian year (365.25 days of 86,400 s), the year the
/// precession periods in sol.json are given in.
const SECONDS_PER_JULIAN_YEAR: f64 = 365.25 * 86_400.0;

/// Degrees per second from a signed precession period in Julian years (see
/// `SolBody::longitude_ascending_node_precession_years`). 0 holds the angle.
fn precession_deg_per_sec(period_years: f64) -> f64 {
    if period_years != 0.0 {
        360.0 / (period_years * SECONDS_PER_JULIAN_YEAR)
    } else {
        0.0
    }
}

/// The orbit's longitude of the ascending node and argument of periapsis at
/// `sim_time_seconds`, in degrees (not wrapped): the J2000 values carried on by
/// their precession rates. A body with no rates in the data keeps its J2000
/// orientation, exactly as before the rates existed.
///
/// The Moon is why this exists (2026-10-21, after BUG-140): held at J2000, its
/// node sat about 200 degrees from the real one by 2026, which put the drawn
/// Moon up to 10 degrees off its real ecliptic latitude and moved every eclipse
/// season (`tests::the_moon_stands_at_its_jpl_horizons_latitude_and_longitude`).
pub fn orbit_angles_deg_at(body: &SolBody, sim_time_seconds: f64) -> (f64, f64) {
    (
        body.longitude_ascending_node_deg
            + precession_deg_per_sec(body.longitude_ascending_node_precession_years) * sim_time_seconds,
        body.argument_perihelion_deg
            + precession_deg_per_sec(body.argument_perihelion_precession_years) * sim_time_seconds,
    )
}

/// The body's mean anomaly at `sim_time_seconds`, degrees in 0..360.
///
/// `orbital_period_days` is the SIDEREAL period, the period of the mean
/// longitude (node + argument of periapsis + mean anomaly), so when the node
/// and periapsis turn the mean anomaly runs at the anomalistic rate: 360 /
/// period less both precession rates. For the Moon that is 27.5546 days from
/// the sidereal 27.3217, the anomalistic month. A body with no rates runs at
/// 360 / period, as it always did.
pub fn mean_anomaly_deg_at(body: &SolBody, sim_time_seconds: f64) -> f64 {
    // Mean anomaly at epoch J2000 — from data if present, else hashed
    // from name so untagged minor bodies don't all start at periapsis.
    let m0_deg = if body.mean_anomaly_deg != 0.0 {
        body.mean_anomaly_deg
    } else {
        (body.name.bytes().fold(0u64, |a, b| a.wrapping_add(b as u64)) % 360) as f64
    };
    // Bodies without an orbital_period_days value stay at their epoch
    // anomaly (Phase 4d may estimate it from Kepler's third law later).
    let m_deg_per_sec = if body.orbital_period_days > 0.0 {
        360.0 / (body.orbital_period_days * 86_400.0)
            - precession_deg_per_sec(body.longitude_ascending_node_precession_years)
            - precession_deg_per_sec(body.argument_perihelion_precession_years)
    } else {
        0.0
    };
    (m0_deg + m_deg_per_sec * sim_time_seconds).rem_euclid(360.0)
}

/// The orbit's in-plane axes in the ecliptic frame of J2000 at a moment: P
/// toward periapsis and Q a quarter turn on in the direction of motion (the
/// standard perifocal P and Q vectors), so a point (x, y) of the orbital plane
/// lies at `x * P + y * Q`. They are the first two columns of the 3-1-3 Euler
/// rotation Rz(node) * Rx(inclination) * Rz(argument of periapsis).
fn perifocal_axes_ecliptic(body: &SolBody, sim_time_seconds: f64) -> (glam::DVec3, glam::DVec3) {
    let (node_deg, arg_deg) = orbit_angles_deg_at(body, sim_time_seconds);
    let (s_om, c_om) = node_deg.to_radians().sin_cos();
    let (s_inc, c_inc) = body.inclination_deg.to_radians().sin_cos();
    let (s_w, c_w) = arg_deg.to_radians().sin_cos();
    (
        glam::DVec3::new(c_om * c_w - s_om * s_w * c_inc, s_om * c_w + c_om * s_w * c_inc, s_w * s_inc),
        glam::DVec3::new(-c_om * s_w - s_om * c_w * c_inc, -s_om * s_w + c_om * c_w * c_inc, c_w * s_inc),
    )
}

/// Compute a body's position relative to its parent, in AU. Applies real
/// Kepler orbital mechanics — eccentricity, inclination, argument of
/// perihelion, longitude of ascending node, mean anomaly at epoch +
/// mean motion × sim_time — on an ellipse that turns at the node and
/// periapsis rates the data gives (`orbit_angles_deg_at`, the Moon's).
///
/// `sim_time_seconds` is seconds since the J2000.0 epoch
/// (2000-01-01 12:00:00 UTC). For LIVE body positions, pass the cosmos
/// sim_time.
pub fn body_position_relative_au(body: &SolBody, sim_time_seconds: f64) -> glam::DVec3 {
    let a_au = if body.semi_major_axis_au > 0.0 {
        body.semi_major_axis_au
    } else if body.semi_major_axis_km > 0.0 {
        body.semi_major_axis_km / KM_PER_AU
    } else {
        return glam::DVec3::ZERO;
    };
    let e = body.eccentricity.clamp(0.0, 0.99);
    let ea = kepler_solve(mean_anomaly_deg_at(body, sim_time_seconds).to_radians(), e);
    // Perifocal coordinates: periapsis along +X of the orbital plane.
    //   x = a * (cos E - e)
    //   y = a * sqrt(1 - e²) * sin E
    let x_peri = a_au * (ea.cos() - e);
    let y_peri = a_au * (1.0 - e * e).sqrt() * ea.sin();
    // The perifocal (x, y, 0) vector in the ecliptic frame, then into the
    // world by the one mapping every reader shares (BUG-140).
    let (p, q) = perifocal_axes_ecliptic(body, sim_time_seconds);
    world_from_ecliptic(p * x_peri + q * y_peri)
}

/// How the ecliptic sits in the world, the one place it is decided.
///
/// The orbit model works in the ecliptic frame of J2000 (the frame JPL
/// Horizons quotes with REF_PLANE=ECLIPTIC): x toward the March equinox, y
/// toward ecliptic longitude 90 degrees, z toward ecliptic north. The world is
/// Y-up, and the planet's spin fixes which way east is: longitude is
/// `atan2(-z, x)` and the planet turns by `Ry(+spin)`
/// (`dev_travel::planet_spin_from_time`), so east is -Z. The proper rotation
/// that lines the two up takes ecliptic x to world X, ecliptic north to world
/// +Y, and therefore ecliptic y to world -Z (X cross -Z is +Y).
///
/// BUG-140 (fixed 2026-10-21): until then ecliptic y went to world +Z.
/// Swapping two axes is a mirror, not a rotation, so every orbit ran clockwise
/// seen from +Y against the planet's counter-clockwise spin, and the Moon and
/// the planets stood at minus their real longitudes (Mars 170 degrees out on
/// 2026-10-21). The Sun on the ground never moved, because the spin is defined
/// from the sun's azimuth (`frame_lock::sun_azimuth`); the stars never moved
/// either, because the star sky is turned to the sun the same way
/// (`renderer::sky_frame::equatorial_to_world`).
pub fn world_from_ecliptic(e: glam::DVec3) -> glam::DVec3 {
    glam::DVec3::new(e.x, e.z, -e.y)
}

/// Inverse of [`world_from_ecliptic`]: a world vector in the ecliptic frame of
/// J2000 (x March equinox, z ecliptic north). The Maps page's sub-point
/// readout uses it to reach right ascension without restating the mapping.
pub fn ecliptic_from_world(w: glam::DVec3) -> glam::DVec3 {
    glam::DVec3::new(w.x, -w.z, w.y)
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
///
/// `sim_time_seconds` picks the moment whose ellipse is drawn: an orbit whose
/// node and periapsis turn (the Moon's, a degree every 19 days) is drawn
/// where it lies then, so the body sits on its own line. The ellipse's SHAPE
/// never depends on it, and for a body without rates the line is the same at
/// every moment.
pub fn sample_orbit_points(body: &SolBody, n: usize, sim_time_seconds: f64) -> Vec<glam::DVec3> {
    let a_au = if body.semi_major_axis_au > 0.0 {
        body.semi_major_axis_au
    } else if body.semi_major_axis_km > 0.0 {
        body.semi_major_axis_km / KM_PER_AU
    } else {
        return Vec::new();
    };
    let e = body.eccentricity.clamp(0.0, 0.99);
    let (p, q) = perifocal_axes_ecliptic(body, sim_time_seconds);
    let mut out = Vec::with_capacity(n + 1);
    for i in 0..=n {
        // Sample uniformly in eccentric anomaly so high-e ellipses still
        // produce well-spaced points around the curve.
        let ea = (i as f64 / n as f64) * std::f64::consts::TAU;
        let x_peri = a_au * (ea.cos() - e);
        let y_peri = a_au * (1.0 - e * e).sqrt() * ea.sin();
        out.push(world_from_ecliptic(p * x_peri + q * y_peri));
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

    // ── BUG-140: the ecliptic in the world, checked against real ephemerides ──

    /// A world direction's longitude, measured the way the ENGINE measures
    /// east: `atan2(-z, x)`, the convention the planet's spin is built on
    /// (`dev_travel::planet_spin_from_time`, `engine::frame_lock::sun_azimuth`):
    /// +Y is north, east is toward -Z, and the planet turns by `Ry(+spin)`.
    /// Deliberately NOT read back through this module's own mapping
    /// (`ecliptic_from_world`), which would make every check below circular:
    /// the question is whether the world agrees with the ground's east.
    fn engine_longitude_deg(v: glam::DVec3) -> f64 {
        (-v.z).atan2(v.x).to_degrees().rem_euclid(360.0)
    }

    fn wrap_deg(d: f64) -> f64 {
        (d + 180.0).rem_euclid(360.0) - 180.0
    }

    /// BUG-140: the ecliptic enters the world by a rotation, never a mirror.
    /// A rotation keeps handedness (x cross y lands on z), puts ecliptic north
    /// on world up, and puts ecliptic longitude 90 where the ground reads 90
    /// degrees east; its inverse undoes it. The old mapping (ecliptic y to
    /// world +Z) fails the first and third.
    #[test]
    fn the_ecliptic_enters_the_world_by_a_rotation_never_a_mirror() {
        use glam::DVec3;
        let (x, y, z) = (
            world_from_ecliptic(DVec3::X),
            world_from_ecliptic(DVec3::Y),
            world_from_ecliptic(DVec3::Z),
        );
        assert_eq!(x.cross(y), z, "handedness kept: x cross y must land on z");
        assert_eq!(z, DVec3::Y, "ecliptic north is world up");
        assert!((engine_longitude_deg(y) - 90.0).abs() < 1e-12, "longitude 90 is the ground's 90 E");
        assert!(engine_longitude_deg(x).abs() < 1e-12, "the March equinox is longitude 0");
        for v in [DVec3::new(0.3, -1.2, 2.5), DVec3::new(-4.0, 0.5, -0.25)] {
            assert_eq!(ecliptic_from_world(world_from_ecliptic(v)), v);
            assert_eq!(world_from_ecliptic(ecliptic_from_world(v)), v);
            assert!((world_from_ecliptic(v).length() - v.length()).abs() < 1e-12);
        }
    }

    /// JPL Horizons heliocentric positions, AU, in the ecliptic and mean
    /// equinox of J2000 (DE441; request: EPHEM_TYPE=VECTORS, CENTER=500@10,
    /// REF_PLANE=ECLIPTIC, REF_SYSTEM=ICRF, TIME_TYPE=UT), at 00:00 UT on each
    /// date, fetched 2026-10-21 from https://ssd.jpl.nasa.gov/api/horizons.api.
    /// Columns: body, date, the engine's sim time for that moment (seconds since
    /// 2000-01-01 12:00 UTC), then X, Y, Z exactly as Horizons printed them.
    const HORIZONS_HELIO_AU: &[(&str, &str, f64, f64, f64, f64)] = &[
        ("mercury", "2010-06-15", 329832000.0, 0.3248049780986699, 0.09112965024587234, -0.02235795453857692),
        ("mercury", "2026-10-21", 845812800.0, 0.3417005017930915, -0.19061532425813, -0.04691738700694836),
        ("mercury", "2040-03-01", 1267444800.0, -0.3941437275951654, -0.05684102413058151, 0.03148917521066164),
        ("venus", "2010-06-15", 329832000.0, -0.718568767764062, 0.01623735820446922, 0.04169218897903367),
        ("venus", "2026-10-21", 845812800.0, 0.6542463742023901, 0.3098357945618266, -0.03349153047294431),
        ("venus", "2040-03-01", 1267444800.0, 0.1851296778769237, -0.7034291206170185, -0.02037061791364632),
        ("earth", "2010-06-15", 329832000.0, -0.1119955760518236, -1.009553893743397, 0.0000248952606914923),
        ("earth", "2026-10-21", 845812800.0, 0.8852152560721928, 0.4557374555437795, -0.00003298955131867717),
        ("earth", "2040-03-01", 1267444800.0, -0.9337195419577451, 0.3314107731105732, -0.00002317641524215849),
        ("mars", "2010-06-15", 329832000.0, -1.61416360194892, -0.2753795282358758, 0.03386580337462371),
        ("mars", "2026-10-21", 845812800.0, -0.141675840057934, 1.575321493269372, 0.03648686033988838),
        ("mars", "2040-03-01", 1267444800.0, -1.015633580245849, 1.294407897428868, 0.05201027030361947),
        ("jupiter", "2010-06-15", 329832000.0, 4.882188426306629, -0.9268496029155743, -0.1054059405051032),
        ("jupiter", "2026-10-21", 845812800.0, -3.604584803293383, 3.903210584337429, 0.06443383010825503),
        ("jupiter", "2040-03-01", 1267444800.0, -5.416112062714304, 0.4591265248132358, 0.1192771112808239),
        ("saturn", "2010-06-15", 329832000.0, -9.493850485847451, -0.6622927719029593, 0.3894313809275238),
        ("saturn", "2026-10-21", 845812800.0, 9.231402316708277, 1.875894476981356, -0.4001354105074595),
        ("saturn", "2040-03-01", 1267444800.0, -9.478777672582895, -1.195655963991231, 0.3980966850220831),
    ];

    /// BUG-140: every planet stands at its real heliocentric ecliptic longitude
    /// (and latitude), read in the world by the ground's own east. The model is
    /// two-body Kepler on J2000 mean elements with no secular rates, measured
    /// 2026-10-21 at 0.32 degree worst (Venus, whose J2000 mean anomaly in
    /// sol.json sits 0.26 degree off) and 0.02 for Earth and Mars, so 0.5 is
    /// the gate. Red check, run: the mirror (ecliptic y to world +Z) put each
    /// planet at minus its longitude, up to 170 degrees out (Mars, 2026-10-21).
    #[test]
    fn planets_stand_at_their_jpl_horizons_longitudes() {
        let mut table = String::new();
        let mut worst = 0.0_f64;
        for &(id, date, t, x, y, z) in HORIZONS_HELIO_AU {
            let want_lon = y.atan2(x).to_degrees().rem_euclid(360.0);
            let want_lat = (z / (x * x + y * y + z * z).sqrt()).asin().to_degrees();
            let w = body_world_position_3d_au(find_body(id).expect(id), t);
            let got_lon = engine_longitude_deg(w);
            let got_lat = (w.y / w.length()).asin().to_degrees();
            let dlon = wrap_deg(got_lon - want_lon);
            let dlat = got_lat - want_lat;
            worst = worst.max(dlon.abs()).max(dlat.abs() * 5.0);
            table.push_str(&format!(
                "\n  {id:<8} {date}: Horizons lon {want_lon:7.3} lat {want_lat:6.3} | world lon {got_lon:7.3} lat {got_lat:6.3} | off {dlon:8.3} {dlat:6.3}"
            ));
        }
        println!("heliocentric ecliptic longitude, Horizons vs the world:{table}");
        assert!(
            worst < 0.5,
            "a planet is off its real place (longitude 0.5, latitude 0.1 degree allowed):{table}"
        );
    }

    /// BUG-140: every orbit goes round its parent the way the planet spins,
    /// counter-clockwise seen from world +Y (ecliptic north), as real orbits
    /// and Earth's real spin both do. The spin's sense is MEASURED here, not
    /// assumed: an hour later `planet_spin_from_time` is larger, and `Ry` of it
    /// carries a point round +Y the way the cross product reports. A body
    /// with an inclination past 90 degrees is retrograde and must go the other
    /// way. Red check, run: the mirror ran every prograde orbit clockwise.
    #[test]
    fn every_orbit_turns_the_way_the_planet_spins() {
        let a = crate::dev_travel::planet_spin_from_time(10.0, 1.0);
        let b = crate::dev_travel::planet_spin_from_time(11.0, 1.0);
        let spin_sense = (glam::DQuat::from_rotation_y(a) * glam::DVec3::X)
            .cross(glam::DQuat::from_rotation_y(b) * glam::DVec3::X)
            .y
            .signum();
        assert_eq!(spin_sense, 1.0, "the planet's spin is counter-clockwise about +Y");
        let mut wrong = Vec::new();
        let mut checked = 0;
        for body in sol_bodies() {
            let cos_i = body.inclination_deg.to_radians().cos();
            if body.parent.is_none() || body.orbital_period_days <= 0.0 || cos_i.abs() < 0.1 {
                continue;
            }
            // A thousandth of an orbit, at a few moments, so a near-periapsis
            // sample cannot hide anything.
            let dt = body.orbital_period_days * 86_400.0 / 1000.0;
            for t in [0.0, 3.1e8, 8.458e8] {
                let p0 = body_position_relative_au(body, t);
                let p1 = body_position_relative_au(body, t + dt);
                let h = p0.cross(p1).y;
                checked += 1;
                if h.signum() != spin_sense * cos_i.signum() {
                    wrong.push(format!("{} at t {t}: angular momentum y {h:.3e}", body.id));
                }
            }
        }
        assert!(checked > 100, "only {checked} orbit samples checked");
        assert!(
            wrong.is_empty(),
            "{} of {checked} orbit samples turn against the spin: {}",
            wrong.len(),
            wrong.iter().take(8).cloned().collect::<Vec<_>>().join("; ")
        );
    }

    /// The Moon's phases from the US Naval Observatory's table
    /// (https://aa.usno.navy.mil/api/moon/phases/date?date=2026-09-01&nump=12,
    /// fetched 2026-10-21; times UT), as the Moon's elongation east of the Sun
    /// at that instant (the phases' definition: 0 new, 90 first quarter, 180
    /// full, 270 last quarter), plus one JPL Horizons reading on the game's own
    /// date (2026-10-21 00:00 UT, observer 500@399: S-O-T 115.8412 "/T", the
    /// Moon trailing the Sun, so east of it, waxing).
    const MOON_ELONGATION_CHECKS: &[(&str, &str, f64, f64)] = &[
        ("2026-09-04 07:51", "Last Quarter", 841780260.0, 270.0),
        ("2026-09-11 03:27", "New Moon", 842369220.0, 0.0),
        ("2026-09-18 20:44", "First Quarter", 843036240.0, 90.0),
        ("2026-09-26 16:49", "Full Moon", 843713340.0, 180.0),
        ("2026-10-03 13:25", "Last Quarter", 844305900.0, 270.0),
        ("2026-10-10 15:50", "New Moon", 844919400.0, 0.0),
        ("2026-10-18 16:12", "First Quarter", 845611920.0, 90.0),
        ("2026-10-21 00:00", "Horizons waxing gibbous", 845812800.0, 115.8412),
        ("2026-10-26 04:12", "Full Moon", 846259920.0, 180.0),
        ("2026-11-01 20:28", "Last Quarter", 846836880.0, 270.0),
        ("2026-11-09 07:02", "New Moon", 847479720.0, 0.0),
        ("2026-11-17 11:48", "First Quarter", 848188080.0, 90.0),
        ("2026-11-24 14:53", "Full Moon", 848803980.0, 180.0),
    ];

    /// The Moon's elongation from the Sun as the GROUND reads it: the engine
    /// longitude of the Moon minus the Sun's, both seen from Earth, eastward
    /// positive, 0..360.
    fn moon_elongation_deg(t: f64) -> f64 {
        let earth = body_world_position_3d_au(find_body("earth").expect("earth"), t);
        let moon = body_world_position_3d_au(find_body("moon").expect("moon"), t);
        let sun = body_world_position_3d_au(find_body("sun").expect("sun"), t);
        (engine_longitude_deg(moon - earth) - engine_longitude_deg(sun - earth)).rem_euclid(360.0)
    }

    /// BUG-140 and the Moon's missing elements: the Moon is waxing when it is
    /// waxing. Measured 2026-10-21 within 1.54 degrees at all thirteen instants
    /// (1.44 before the node and perigee turned; the model has none of the
    /// Moon's large perturbations, which is a few degrees in a bad month), so
    /// the gate is 2.5. Red check, run, two ways: today's sol.json had no
    /// mean anomaly, node or perigee for the Moon (its phase came from a hash
    /// of its name, 165 to 179 degrees out), and with those filled in but the
    /// mirror kept, every waxing Moon reads as waning (First Quarter at 270).
    #[test]
    fn the_moon_keeps_the_naval_observatory_phases() {
        let mut table = String::new();
        let mut worst = 0.0_f64;
        for &(when, what, t, want) in MOON_ELONGATION_CHECKS {
            let got = moon_elongation_deg(t);
            let off = wrap_deg(got - want);
            worst = worst.max(off.abs());
            table.push_str(&format!("\n  {when} {what:<24} want {want:8.3} world {got:8.3} off {off:7.2}"));
        }
        println!("the Moon's elongation east of the Sun:{table}");
        assert!(worst < 2.5, "the Moon is off its phase (2.5 degrees allowed):{table}");
    }

    // ── The Moon's turning orbit (2026-10-21, the follow-up to BUG-140) ──

    /// The Moon's geocentric place from JPL Horizons (DE441), km, in the
    /// ecliptic and mean equinox of J2000, geometric (no light time). Request:
    /// COMMAND=301, EPHEM_TYPE=VECTORS, CENTER=500@399, REF_PLANE=ECLIPTIC,
    /// REF_SYSTEM=ICRF, TIME_TYPE=UT, VEC_TABLE=1, OUT_UNITS=KM-S, TLIST of
    /// these instants as Julian days; fetched 2026-10-21 from
    /// https://ssd.jpl.nasa.gov/api/horizons.api. Columns: UT, the engine's sim
    /// seconds, then X, Y, Z exactly as Horizons printed them. The dates span
    /// two turns of the node; 2001-03-06 and 2027-12-16 are the worst days in
    /// longitude and latitude of a daily sweep (see the test), and 2027-08-08
    /// and 2039-12-22 the worst for the elements held at J2000.
    const HORIZONS_MOON_GEO_KM: &[(&str, f64, f64, f64, f64)] = &[
        ("2000-01-01 12:00", 0.0, -2.915670760779644E+05, -2.750266556035658E+05, 3.627045763156361E+04),
        ("2001-03-06 12:00", 37152000.0, -1.862151051737822E+05, 3.110136986257118E+05, 9.623889631305196E+03),
        ("2004-07-15 00:00", 143121600.0, 3.065378240420515E+04, 4.041801760965426E+05, 2.619267778092049E+04),
        ("2008-11-20 00:00", 280411200.0, -3.253246972717696E+05, 1.939615200617575E+05, -9.644420532948578E+03),
        ("2013-04-10 00:00", 418824000.0, 3.763332355565282E+05, 1.036983650043483E+05, 1.857819913302323E+04),
        ("2017-08-21 18:00", 556610400.0, -3.166771021663696E+05, 1.952454742972497E+05, 2.609061109067989E+03),
        ("2022-01-05 00:00", 694612800.0, 2.590957196923129E+05, -2.571625789964821E+05, -3.172899037377932E+04),
        ("2026-10-21 00:00", 845812800.0, 3.154012378105788E+05, -2.370060813648762E+05, -2.812831707305464E+03),
        ("2027-08-08 12:00", 870998400.0, -3.062095733542076E+05, -2.296017422678114E+05, -3.535927460396917E+04),
        ("2027-12-16 12:00", 882230400.0, -1.942268818351540E+05, 3.080066786248458E+05, 9.559772928238090E+01),
        ("2031-03-15 00:00", 984571200.0, -1.444374844083635E+05, -3.423628636763556E+05, 4.161863716041145E+03),
        ("2035-08-02 00:00", 1122897600.0, -9.388244383557852E+04, 3.473973537701076E+05, -2.524743389550396E+04),
        ("2039-12-22 12:00", 1261440000.0, 3.847426607384254E+05, 6.125661945332113E+03, -3.369625478179914E+04),
        ("2040-03-01 00:00", 1267444800.0, -3.842213253735183E+05, -2.544236015532333E+04, 3.017009109964336E+04),
    ];

    /// The Moon stands at its real ecliptic latitude and longitude, seen from
    /// Earth, from 2000 to 2040: only possible while its node regresses and its
    /// perigee advances at their real rates. Read in the world by the ground's
    /// own east, through the same chain the game places the Moon by (Moon world
    /// position less Earth's).
    ///
    /// The model is a precessing Kepler ellipse with none of the Moon's periodic
    /// perturbations (the largest, evection, is over a degree of longitude).
    /// Measured 2026-10-21: 0.343 degree
    /// worst in latitude and 2.54 in longitude on these dates. A daily sweep of
    /// Horizons from 2000-01-01 to 2041-01-01 (14,976 days, same request with
    /// STEP_SIZE 1 d), run through a JavaScript copy of this propagator, found
    /// the same two worst days and rms errors of 0.135 (latitude) and 1.03
    /// (longitude). So the gates are 0.4 and 3.0.
    ///
    /// Red check, run 2026-10-21 against the node and perigee held at their J2000
    /// values (sol.json catalog_version 4, before the rates): "worst lat 10.423,
    /// lon 14.245", thirteen of the fourteen dates out (all but the epoch), the
    /// Moon on the wrong side of the ecliptic on four. From that run's table
    /// (Horizons | world):
    ///   2004-07-15 00:00: lat  3.697 | -3.287
    ///   2013-04-10 00:00: lon  15.406 lat  2.725 | lon   4.593 lat -4.437
    ///   2026-10-21 00:00: lat -0.408 | -1.559
    ///   2027-08-08 12:00: lat -5.278 |  5.144
    ///   2039-12-22 12:00: lon   0.912 | 346.667 (14.2 degrees behind)
    /// Fixed: "worst lat 0.343, lon 2.540", and 2026-10-21 at lat -0.268.
    #[test]
    fn the_moon_stands_at_its_jpl_horizons_latitude_and_longitude() {
        let earth = find_body("earth").expect("earth");
        let moon = find_body("moon").expect("moon");
        let mut table = String::new();
        let (mut worst_lat, mut worst_lon) = (0.0_f64, 0.0_f64);
        for &(when, t, x, y, z) in HORIZONS_MOON_GEO_KM {
            let want_lon = y.atan2(x).to_degrees().rem_euclid(360.0);
            let want_lat = (z / (x * x + y * y + z * z).sqrt()).asin().to_degrees();
            let w = body_world_position_3d_au(moon, t) - body_world_position_3d_au(earth, t);
            let got_lon = engine_longitude_deg(w);
            let got_lat = (w.y / w.length()).asin().to_degrees();
            let (dlon, dlat) = (wrap_deg(got_lon - want_lon), got_lat - want_lat);
            worst_lat = worst_lat.max(dlat.abs());
            worst_lon = worst_lon.max(dlon.abs());
            table.push_str(&format!(
                "\n  {when}: Horizons lon {want_lon:8.3} lat {want_lat:7.3} | world lon {got_lon:8.3} lat {got_lat:7.3} | off {dlon:8.3} {dlat:7.3}"
            ));
        }
        println!("the Moon's geocentric ecliptic place, Horizons vs the world (worst lat {worst_lat:.3}, lon {worst_lon:.3}):{table}");
        assert!(
            worst_lat < 0.4 && worst_lon < 3.0,
            "the Moon is off its real place (latitude 0.4, longitude 3.0 degrees allowed; worst {worst_lat:.3}, {worst_lon:.3}):{table}"
        );
    }

    /// The rates in sol.json are the ones the Moon's orbit actually has, read
    /// from the model's own orbit: its ascending node, measured as the
    /// longitude where the Moon crosses the ecliptic going north, regresses
    /// 19.355 degrees a year (once in 18.600 years), and its perigee advances
    /// once in 8.85 years. A sign typed wrong in the data (JPL lists the
    /// periods unsigned) would advance the node instead, which this catches
    /// even where a latitude check on a few dates might not. Measured the same
    /// way from Horizons' daily vectors 2000 to 2041 (550 crossings, 2026-10-21):
    /// node -19.355 degrees a year, perigee +40.671 (8.852 years).
    #[test]
    fn the_moons_node_regresses_and_its_perigee_advances() {
        let moon = find_body("moon").expect("moon");
        let year = SECONDS_PER_JULIAN_YEAR;
        let (n0, w0) = orbit_angles_deg_at(moon, 0.0);
        let (n1, w1) = orbit_angles_deg_at(moon, 10.0 * year);
        let node_per_year = (n1 - n0) / 10.0;
        let perigee_per_year = ((n1 + w1) - (n0 + w0)) / 10.0;
        println!("node {node_per_year:.3} deg/yr, perigee {perigee_per_year:.3} deg/yr");
        assert!((node_per_year + 19.355).abs() < 0.01, "node {node_per_year} deg/yr, want -19.355");
        assert!((perigee_per_year - 40.67).abs() < 0.02, "perigee {perigee_per_year} deg/yr, want +40.67");
        // And the ecliptic crossing itself, from positions: the Moon going north
        // through the ecliptic in October 2026 does so near the real node.
        // Horizons' daily vectors, interpolated, cross going north at about
        // 329.2 (2026-09-24 03 UT), 327.7 (2026-10-21 09 UT) and 324.7
        // (2026-11-17 12 UT): the mean node, 326.4, with the real node's swing
        // of 1.5 degrees either way that the Sun drives on top, which the model
        // does not have.
        let earth = find_body("earth").expect("earth");
        let rel = |t: f64| body_world_position_3d_au(moon, t) - body_world_position_3d_au(earth, t);
        let start = 844_128_000.0; // 2026-10-01 12:00 UTC
        let mut crossing = None;
        let mut t = start;
        while t < start + 30.0 * 86_400.0 {
            let (a, b) = (rel(t), rel(t + 3600.0));
            if a.y < 0.0 && b.y >= 0.0 {
                let f = -a.y / (b.y - a.y);
                crossing = Some(engine_longitude_deg(a + (b - a) * f));
                break;
            }
            t += 3600.0;
        }
        let lon = crossing.expect("the Moon crosses the ecliptic going north once a month");
        println!("ascending crossing, October 2026: {lon:.2}");
        assert!(wrap_deg(lon - 326.4).abs() < 3.0, "the Moon crosses north at {lon:.2}, the real node is near 326.4");
    }

    /// NASA's lunar eclipses of 2026 (F. Espenak, "Decade Tables of Lunar
    /// Eclipses: 2021 to 2030", https://eclipse.gsfc.nasa.gov/LEdecade/LEdecade2021.html,
    /// page dated 2013 Dec 09, fetched 2026-10-21): "2026 Mar 03 11:34:52 Total"
    /// (umbral magnitude 1.151) and "2026 Aug 28 04:14:04 Partial" (0.930),
    /// times in TD, here less the 75 s of Delta T the solar catalog lists for
    /// 2026. NASA has no other lunar eclipse in 2026, penumbral or umbral.
    const NASA_LUNAR_ECLIPSES_2026: &[(&str, f64, &str)] = &[
        ("2026-03-03 11:33:37", 825_809_617.0, "total"),
        ("2026-08-28 04:12:49", 841_162_369.0, "partial"),
    ];

    /// The Moon's eclipse seasons: of the thirteen full Moons of 2026 in the
    /// model, the Moon meets Earth's umbra at exactly NASA's two, as total and
    /// partial, and misses it at the other eleven. The umbra is the one NASA's
    /// canon uses (Danjon's rule, quoted from
    /// https://eclipse.gsfc.nasa.gov/LEcat5/shadow.html, fetched 2026-10-21:
    /// "umbral radius: Ru = 1.01 * Pm - Ss + Ps", Pm and Ps the Moon's and the
    /// Sun's equatorial horizontal parallax, Ss the Sun's semi-diameter; "Meeus
    /// and Mucke (1979) and Espenak (2006) both use Danjon's method"). Each
    /// eclipse's middle (least distance from the shadow's centre) must fall
    /// within 5 hours of NASA's, since the model's Moon runs up to 2.5 degrees
    /// off in longitude, 4.6 hours of its motion. Measured 2026-10-21: total
    /// 2.9 hours early at 0.343 degree from the shadow's centre (total needs
    /// under about 0.43), partial 2.6 hours late at 0.489.
    ///
    /// Red check, run 2026-10-21 against the node and perigee held at J2000:
    /// the umbral eclipses fell on the full Moons of 2026-02-01 ("partial ...
    /// 0.682 degree from the centre") and 2026-07-29 ("total ... 0.105"), a
    /// month early each, and none on NASA's dates.
    #[test]
    fn the_moon_meets_earths_shadow_at_nasas_2026_lunar_eclipses() {
        const EARTH_EQUATORIAL_KM: f64 = 6378.137;
        let (earth, moon, sun) = (
            find_body("earth").expect("earth"),
            find_body("moon").expect("moon"),
            find_body("sun").expect("sun"),
        );
        // (distance of the Moon from the shadow's centre, umbra radius, Moon radius), degrees.
        let geometry = |t: f64| -> (f64, f64, f64) {
            let e = body_world_position_3d_au(earth, t);
            let to_moon = body_world_position_3d_au(moon, t) - e;
            let to_sun = body_world_position_3d_au(sun, t) - e;
            let (dm, ds) = (to_moon.length() * KM_PER_AU, to_sun.length() * KM_PER_AU);
            let from_antisun = to_moon.angle_between(-to_sun).to_degrees();
            let pm = (EARTH_EQUATORIAL_KM / dm).asin().to_degrees();
            let ps = (EARTH_EQUATORIAL_KM / ds).asin().to_degrees();
            let ss = (sun.radius_km / ds).asin().to_degrees();
            let sm = (moon.radius_km / dm).asin().to_degrees();
            (from_antisun, 1.01 * pm - ss + ps, sm)
        };
        let start = 820_497_600.0; // 2026-01-01 00:00 UTC
        let end = 852_033_600.0; // 2027-01-01 00:00 UTC
        let step = 600.0;
        // (sim seconds, distance from the shadow's centre, kind, total under, umbral under)
        let mut found: Vec<(f64, f64, &str, f64, f64)> = Vec::new();
        let mut full_moons = 0;
        let (mut s2, mut s1) = (f64::MAX, f64::MAX);
        let mut t = start;
        while t <= end {
            let (s, _, _) = geometry(t);
            // A full Moon: the middle sample is the least distance from the anti-Sun point.
            if s1 < s2 && s1 <= s && s1 < 30.0 {
                full_moons += 1;
                let tm = t - step;
                let (d, umbra, rm) = geometry(tm);
                if d < umbra + rm {
                    found.push((tm, d, if d < umbra - rm { "total" } else { "partial" }, umbra - rm, umbra + rm));
                }
            }
            (s2, s1) = (s1, s);
            t += step;
        }
        let shown: Vec<String> = found
            .iter()
            .map(|(tm, d, k, tot, umb)| {
                format!(
                    "{k} at sim {tm:.0} ({:+.2} days from 2026-01-01), {d:.3} degree from the centre (total under {tot:.3}, umbral under {umb:.3})",
                    (tm - start) / 86_400.0
                )
            })
            .collect();
        println!("2026 in the model: {full_moons} full Moons; umbral eclipses: {shown:?}");
        assert_eq!(full_moons, 13, "2026 has thirteen full Moons");
        assert_eq!(found.len(), NASA_LUNAR_ECLIPSES_2026.len(), "umbral eclipses in 2026: {shown:?}");
        for (&(when, nasa_t, nasa_kind), &(tm, _, kind, _, _)) in NASA_LUNAR_ECLIPSES_2026.iter().zip(&found) {
            let hours = (tm - nasa_t) / 3600.0;
            assert!(hours.abs() < 5.0 && kind == nasa_kind, "NASA {nasa_kind} {when}: the model's is {kind}, {hours:+.1} h ({shown:?})");
        }
    }
}
