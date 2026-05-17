//! Canonical Sol-system model (v0.262.8 — extracted from
//! `gui/pages/cosmos.rs` so it is the SINGLE source of truth).
//!
//! Before this module the solar system existed as **four drifted
//! copies**: this Keplerian model (Maps page, accurate), a log-scaled
//! `data/world/solar_system.ron` (the in-home hologram), a circular
//! approximation in `terrain/planet_registry.rs`, and a hardcoded JS
//! array in `web/pages/maps.html`. The operator asked us to "sync the
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

/// Parse + cache `data/star_systems/sol.json` into `SolBody` rows
/// (with parent→children links). `region` rows (asteroid belts) are
/// skipped — they are not positionable point bodies. Cached `&'static`
/// so per-frame UI / render code can call it freely.
pub fn sol_bodies() -> &'static [SolBody] {
    SOL_BODIES.get_or_init(|| {
        let json = crate::embedded_data::SOLAR_SYSTEM_JSON;
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
        log::info!("Cosmos: loaded {} Sol bodies (with parent-child links)", out.len());
        out
    })
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
    use super::*;

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
