//! Cosmos page (v0.203.0, Phase 3 of the cosmos architecture).
//!
//! Three view modes, one canvas:
//! - **System** — Sol's planets in their orbits (top-down 2D, AU-scale)
//! - **Galactic** — Sol-centered top-down map of nearby stars (light-year
//!   scale), labeled with proper names + spectral colors
//! - **Night Sky** — Earth-centered celestial sphere (RA/Dec
//!   equirectangular projection) with bright stars + constellation lines
//!   + the constellation myth/season/key-star info on hover
//!
//! Per `docs/design/cosmos-architecture.md` this is the user-facing
//! surface for the position model. Eventually it'll auto-pick the view
//! based on the player's `PositionInUniverse.container`; for now it
//! has a manual toggle since the position-driven render bridge ships
//! in a later phase.
//!
//! Operator 2026-05-10: "We at least want our galaxy and solar system
//! ... see the real stars and real constellations." This page reads
//! from the existing data files (constellations.json, stars-nearby.json,
//! stars-catalog.json, sol.json) — all of which already ship embedded.
//! The 119k full HYG catalog (data/stars.csv) is used by the 3D skybox
//! renderer; for 2D map purposes the embedded ~300 brightest + nearby
//! is plenty without the overhead.

use egui::{Align2, Color32, Frame, Pos2, Rect, RichText, Rounding, ScrollArea, Sense, Stroke, Vec2};
use std::sync::OnceLock;

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

// ─────────────────────── Data layer (lazy-loaded caches) ────────────────────

/// One nearby star in 3D galactic coordinates (light-years from Sol).
#[derive(Debug, Clone)]
struct NearbyStar {
    name: String,
    /// 3D position in light-years from Sol.
    pos_ly: glam::DVec3,
    spectral: String,
    apparent_magnitude: f64,
    distance_ly: f64,
    /// Alternate / common name, may equal `name`.
    alt_name: String,
}

/// One bright catalog star in equatorial (RA / Dec) coordinates.
#[derive(Debug, Clone)]
struct BrightStar {
    name: String,
    /// Right Ascension in hours (0-24).
    ra_hours: f64,
    /// Declination in degrees (-90 to +90).
    dec_deg: f64,
    /// Apparent magnitude (lower = brighter).
    magnitude: f64,
    spectral: String,
}

/// One constellation with its line geometry.
#[derive(Debug, Clone)]
struct Constellation {
    name: String,
    abbr: String,
    /// Pairs of (star_name_a, star_name_b) — line endpoints.
    lines: Vec<(String, String)>,
    myth: String,
    season: String,
    key_stars: Vec<String>,
    objects: Vec<String>,
}

/// One Sol-system body for the system view + body browser sidebar +
/// details panel. Phase 3 (v0.203.2): expanded with the fields needed
/// for the right-side details panel (radius, mass, gravity, atmosphere
/// composition, orbital period, mean temperature, discovery info,
/// description).
#[derive(Debug, Clone)]
struct SolBody {
    id: String,
    name: String,
    body_type: String,
    /// Parent body id (e.g. "sun" for planets, "earth" for "moon").
    parent: Option<String>,
    /// Semi-major axis in AU (only set for direct sun-orbiters).
    semi_major_axis_au: f64,
    /// Semi-major axis in km (only set for moons orbiting their planet).
    semi_major_axis_km: f64,
    /// Orbital eccentricity. 0 = circle, 0..1 = ellipse, 1 = parabolic
    /// escape, >1 = hyperbolic flyby. v0.207.0.
    eccentricity: f64,
    /// Orbital inclination in degrees (tilt of the orbit plane relative
    /// to the reference plane — ecliptic for Sol-orbiters). v0.207.0.
    inclination_deg: f64,
    /// Longitude of the ascending node in degrees — where the orbit
    /// crosses the reference plane going north. v0.207.0.
    longitude_ascending_node_deg: f64,
    /// Argument of periapsis in degrees — angle from ascending node to
    /// the periapsis point. v0.207.0.
    argument_perihelion_deg: f64,
    /// Mean anomaly at epoch (J2000) in degrees. Combined with
    /// `orbital_period_days` + sim_time, gives the body's snapshot
    /// position. v0.207.0.
    mean_anomaly_deg: f64,
    /// Body radius in km — for visual sizing.
    radius_km: f64,
    /// Mass in kg.
    mass_kg: f64,
    /// Surface gravity in m/s².
    surface_gravity_ms2: f64,
    /// Mean surface / cloud-top temperature in Kelvin.
    mean_temperature_k: f64,
    /// Orbital period in days.
    orbital_period_days: f64,
    /// Atmosphere composition summary (top 3 components, formatted).
    /// Empty string if no atmosphere.
    atmosphere_summary: String,
    /// Free-form description (1-2 sentences).
    description: String,
    /// Discovery year, if known. 0 = ancient / no record.
    discovery_year: i32,
    /// Discoverer name, if known.
    discoverer: String,
    /// IDs of bodies orbiting this one (e.g. moons of a planet).
    children: Vec<String>,
}

static NEARBY_STARS: OnceLock<Vec<NearbyStar>> = OnceLock::new();
static BRIGHT_STARS: OnceLock<Vec<BrightStar>> = OnceLock::new();
static CONSTELLATIONS: OnceLock<Vec<Constellation>> = OnceLock::new();
static SOL_BODIES: OnceLock<Vec<SolBody>> = OnceLock::new();

fn nearby_stars() -> &'static [NearbyStar] {
    NEARBY_STARS.get_or_init(|| {
        let json = crate::embedded_data::STARS_NEARBY_JSON;
        // Schema per data/stars-nearby.json:
        //   [name, x_ly, y_ly, z_ly, spectral, apparent_mag, distance_ly, alt_name]
        let parsed: serde_json::Value = serde_json::from_str(json).unwrap_or(serde_json::Value::Null);
        let mut out = Vec::new();
        if let Some(arr) = parsed.as_array() {
            for row in arr {
                let r = match row.as_array() { Some(r) => r, None => continue };
                if r.len() < 7 { continue; }
                let name = r[0].as_str().unwrap_or("").to_string();
                let x = r[1].as_f64().unwrap_or(0.0);
                let y = r[2].as_f64().unwrap_or(0.0);
                let z = r[3].as_f64().unwrap_or(0.0);
                let spec = r[4].as_str().unwrap_or("").to_string();
                let am = r[5].as_f64().unwrap_or(99.0);
                let dist = r[6].as_f64().unwrap_or(0.0);
                let alt = r.get(7).and_then(|v| v.as_str()).unwrap_or(&name).to_string();
                out.push(NearbyStar {
                    name,
                    pos_ly: glam::DVec3::new(x, y, z),
                    spectral: spec,
                    apparent_magnitude: am,
                    distance_ly: dist,
                    alt_name: alt,
                });
            }
        }
        log::info!("Cosmos: loaded {} nearby stars", out.len());
        out
    })
}

fn bright_stars() -> &'static [BrightStar] {
    BRIGHT_STARS.get_or_init(|| {
        let json = crate::embedded_data::STARS_CATALOG_JSON;
        // Schema per data/stars-catalog.json:
        //   [name, ra_hours, dec_deg, magnitude, spectral]
        let parsed: serde_json::Value = serde_json::from_str(json).unwrap_or(serde_json::Value::Null);
        let mut out = Vec::new();
        if let Some(arr) = parsed.as_array() {
            for row in arr {
                let r = match row.as_array() { Some(r) => r, None => continue };
                if r.len() < 5 { continue; }
                out.push(BrightStar {
                    name: r[0].as_str().unwrap_or("").to_string(),
                    ra_hours: r[1].as_f64().unwrap_or(0.0),
                    dec_deg: r[2].as_f64().unwrap_or(0.0),
                    magnitude: r[3].as_f64().unwrap_or(99.0),
                    spectral: r[4].as_str().unwrap_or("").to_string(),
                });
            }
        }
        log::info!("Cosmos: loaded {} bright catalog stars", out.len());
        out
    })
}

fn constellations() -> &'static [Constellation] {
    CONSTELLATIONS.get_or_init(|| {
        let json = crate::embedded_data::CONSTELLATIONS_JSON;
        let parsed: serde_json::Value = serde_json::from_str(json).unwrap_or(serde_json::Value::Null);
        let mut out = Vec::new();
        if let Some(arr) = parsed.as_array() {
            for c in arr {
                let name = c.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let abbr = c.get("abbr").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let lines: Vec<(String, String)> = c.get("lines")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|pair| {
                        let p = pair.as_array()?;
                        Some((p.get(0)?.as_str()?.to_string(), p.get(1)?.as_str()?.to_string()))
                    }).collect())
                    .unwrap_or_default();
                let myth = c.get("myth").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let season = c.get("season").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let key_stars: Vec<String> = c.get("keyStars")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|s| s.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                let objects: Vec<String> = c.get("objects")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|s| s.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                out.push(Constellation { name, abbr, lines, myth, season, key_stars, objects });
            }
        }
        log::info!("Cosmos: loaded {} constellations", out.len());
        out
    })
}

fn sol_bodies() -> &'static [SolBody] {
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
/// it's fine to call this from per-frame UI code.
fn find_body(id: &str) -> Option<&'static SolBody> {
    sol_bodies().iter().find(|b| b.id == id)
}

// ─────────────────────── Spectral-class → color ─────────────────────────────

/// Map a spectral class letter to an approximate display color.
/// Hot blue (O/B) → white (A/F) → yellow (G) → orange (K) → red (M).
/// All RGB values are real astrophysical color approximations
/// (Mitchell Charity / NASA-published spectral type → RGB conversions),
/// not themable styling. Marked theme-exempt per-line.
fn spectral_color(class: &str) -> Color32 {
    let c = class.chars().next().unwrap_or('?');
    match c {
        'O' => Color32::from_rgb(155, 176, 255), // theme-exempt: spectral class O (hot blue) — physics, not theme
        'B' => Color32::from_rgb(170, 191, 255), // theme-exempt: spectral class B — physics, not theme
        'A' => Color32::from_rgb(202, 215, 255), // theme-exempt: spectral class A (white) — physics, not theme
        'F' => Color32::from_rgb(248, 247, 255), // theme-exempt: spectral class F — physics, not theme
        'G' => Color32::from_rgb(255, 244, 234), // theme-exempt: spectral class G (sun-like) — physics, not theme
        'K' => Color32::from_rgb(255, 210, 161), // theme-exempt: spectral class K (orange) — physics, not theme
        'M' => Color32::from_rgb(255, 180, 130), // theme-exempt: spectral class M (red dwarf) — physics, not theme
        _   => Color32::from_rgb(200, 200, 200), // theme-exempt: unknown spectral class fallback — physics, not theme
    }
}

/// Map a Sol body name to its display color. Approximations of real
/// imagery (Mars red, Jupiter banded, Earth blue, etc.). Domain data,
/// not theme tokens — the Sun stays yellow regardless of UI theme.
fn body_color(name: &str) -> Color32 {
    match name.to_lowercase().as_str() {
        "sun" => Color32::from_rgb(255, 220, 100),     // theme-exempt: real Sun color — astrophysics, not theme
        "mercury" => Color32::from_rgb(160, 130, 100), // theme-exempt: real Mercury color — astrophysics, not theme
        "venus" => Color32::from_rgb(225, 200, 140),   // theme-exempt: real Venus color — astrophysics, not theme
        "earth" => Color32::from_rgb(80, 140, 220),    // theme-exempt: real Earth color — astrophysics, not theme
        "mars" => Color32::from_rgb(200, 90, 60),      // theme-exempt: real Mars color — astrophysics, not theme
        "jupiter" => Color32::from_rgb(220, 180, 140), // theme-exempt: real Jupiter color — astrophysics, not theme
        "saturn" => Color32::from_rgb(220, 200, 150),  // theme-exempt: real Saturn color — astrophysics, not theme
        "uranus" => Color32::from_rgb(170, 220, 230),  // theme-exempt: real Uranus color — astrophysics, not theme
        "neptune" => Color32::from_rgb(80, 130, 220),  // theme-exempt: real Neptune color — astrophysics, not theme
        "pluto" => Color32::from_rgb(180, 160, 140),   // theme-exempt: real Pluto color — astrophysics, not theme
        _ => Color32::from_rgb(180, 180, 200),         // theme-exempt: unknown body color fallback — astrophysics, not theme
    }
}

// ─────────────────────── 3D camera (Phase 4, v0.206.0) ─────────────────────

/// 3D orbital camera for the System view. Looks at `target` from
/// `distance_au` units away, rotated by `yaw_rad` (azimuth around Y)
/// and `pitch_rad` (elevation). Standard turntable camera convention
/// (Blender / KSP / Star Citizen).
#[derive(Debug, Clone, Copy)]
pub struct Cosmos3DCamera {
    /// Look-at point in AU, in the system frame (Sun at origin).
    pub target_au: glam::DVec3,
    /// Azimuthal angle around the Y (vertical) axis. Radians.
    pub yaw_rad: f64,
    /// Elevation angle. Radians. Clamped to (-π/2, π/2) excl. poles.
    pub pitch_rad: f64,
    /// Distance from target in AU. Effective zoom — smaller = closer.
    pub distance_au: f64,
    /// Vertical field of view in radians.
    pub fov_rad: f64,
}

impl Default for Cosmos3DCamera {
    fn default() -> Self {
        // Default: looking down at the ecliptic from a slight tilt,
        // 60 AU away — frames the whole solar system out to Pluto.
        Self {
            target_au: glam::DVec3::ZERO,
            yaw_rad: 0.0,
            pitch_rad: -1.2,                       // ~ -69° (looking down at the ecliptic)
            distance_au: 60.0,
            fov_rad: 60.0_f64.to_radians(),
        }
    }
}

impl Cosmos3DCamera {
    /// Camera world position derived from target + spherical offset.
    pub fn position(&self) -> glam::DVec3 {
        let cp = self.pitch_rad.cos();
        let offset = glam::DVec3::new(
            self.distance_au * cp * self.yaw_rad.cos(),
            self.distance_au * self.pitch_rad.sin(),
            self.distance_au * cp * self.yaw_rad.sin(),
        );
        self.target_au + offset
    }
}

/// Perspective-project a 3D world position (AU) to a 2D screen pixel.
/// Returns Some((screen_pos, depth)) where depth is camera-space Z
/// (used for back-to-front sorting) or None if the point is behind
/// the camera (clipped).
fn project_to_screen(pos_au: glam::DVec3, cam: &Cosmos3DCamera, rect: Rect) -> Option<(Pos2, f64)> {
    let cam_pos = cam.position();
    // View matrix — look from cam_pos to target, with +Y as up. f64 throughout.
    let view = glam::DMat4::look_at_rh(cam_pos, cam.target_au, glam::DVec3::Y);
    let aspect = (rect.width() / rect.height().max(1.0)) as f64;
    let proj = glam::DMat4::perspective_rh(cam.fov_rad, aspect, 0.01_f64, 10_000.0_f64);

    let view_pos = view.transform_point3(pos_au);
    if view_pos.z >= 0.0 {
        // Behind camera (RH look_at puts forward as -Z).
        return None;
    }
    let clip = proj * glam::DVec4::new(view_pos.x, view_pos.y, view_pos.z, 1.0);
    if clip.w.abs() < 1e-6 { return None; }
    let ndc = glam::DVec3::new(clip.x / clip.w, clip.y / clip.w, clip.z / clip.w);
    // NDC y is up; screen y is down — flip.
    let sx = rect.left() + ((ndc.x as f32 + 1.0) * 0.5) * rect.width();
    let sy = rect.top() + ((1.0 - (ndc.y as f32 + 1.0) * 0.5)) * rect.height();
    Some((Pos2::new(sx, sy), -view_pos.z))
}

/// Conversion helper.
const KM_PER_AU: f64 = 149_597_870.7;

/// Solve Kepler's equation `M = E - e*sin(E)` for eccentric anomaly E
/// given mean anomaly M (radians) and eccentricity e (0..1).
/// Newton-Raphson iteration; converges in ~5 iterations for e < 0.9.
/// v0.207.0 — real orbital mechanics.
fn kepler_solve(mean_anom: f64, ecc: f64) -> f64 {
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
/// degrees per day. v0.208.0 (orbital evolution shipped).
fn body_position_relative_au(body: &SolBody, sim_time_seconds: f64) -> glam::DVec3 {
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
fn body_world_position_3d_au(body: &SolBody, sim_time_seconds: f64) -> glam::DVec3 {
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
/// Used by orbit-line rendering. v0.207.0 — real ellipses + inclination.
fn sample_orbit_points(body: &SolBody, n: usize) -> Vec<glam::DVec3> {
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

// ─────────────────────── Page entry point ───────────────────────────────────

/// View mode — operator selectable via tab bar at the top of the page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CosmosView {
    /// 3D System view of Sol — Sun at origin, planets in their orbits,
    /// moons in real positions relative to their planets. Rotatable
    /// camera (drag), zoomable (scroll), pannable target (shift+drag).
    /// v0.206.0: replaced the v0.203 2D top-down with proper 3D.
    System,
    /// Top-down 2D map of stars within ~50 ly of Sol (light-year scale).
    Galactic,
    /// Earth-centered celestial sphere (RA / Dec) with constellation lines.
    NightSky,
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // v0.203.2: System view gets left + right side panels (browser + details);
    // Galactic and Night Sky stay full-width since they're a single canvas
    // and don't have a discrete object hierarchy worth browsing.
    let in_system_view = state.cosmos_view == CosmosView::System;

    if in_system_view {
        // Left: collapsible body browser tree.
        egui::SidePanel::left("cosmos_body_browser")
            .resizable(true)
            .min_width(220.0)
            .max_width(360.0)
            .default_width(260.0)
            .frame(Frame::NONE.fill(theme.bg_panel()).inner_margin(theme.spacing_sm))
            .show(ctx, |ui| {
                draw_body_browser(ui, theme, state);
            });

        // Right: details for the selected body.
        egui::SidePanel::right("cosmos_body_details")
            .resizable(true)
            .min_width(260.0)
            .max_width(420.0)
            .default_width(300.0)
            .frame(Frame::NONE.fill(theme.bg_panel()).inner_margin(theme.spacing_md))
            .show(ctx, |ui| {
                draw_body_details(ui, theme, state);
            });
    }

    egui::CentralPanel::default()
        .frame(Frame::NONE.fill(Color32::from_rgb(8, 8, 14)).inner_margin(0.0))  // theme-exempt: deep-space backdrop — domain aesthetic, not theme
        .show(ctx, |ui| {
            // Header bar with view tabs + scale info.
            ui.horizontal(|ui| {
                ui.add_space(theme.spacing_md);
                ui.label(
                    RichText::new("Cosmos")
                        .size(theme.font_size_heading)
                        .color(theme.text_primary())
                        .strong(),
                );
                ui.add_space(theme.spacing_lg);
                view_tab(ui, theme, state, CosmosView::System,    "System",          "Sol — Sun + planets + moons in 3D. Drag to rotate, scroll to zoom, shift+drag to pan.");
                view_tab(ui, theme, state, CosmosView::Galactic,  "Galactic",        "Sol-centered map of nearby stars, light-year scale (2D top-down).");
                view_tab(ui, theme, state, CosmosView::NightSky,  "Night Sky",       "Earth-centered celestial sphere with constellation lines (2D RA/Dec projection).");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(theme.spacing_md);
                    let hint = match state.cosmos_view {
                        CosmosView::System    => "3D camera · drag to rotate · scroll to zoom · shift+drag to pan",
                        CosmosView::Galactic  => "2D top-down · scroll to zoom · click-drag to pan",
                        CosmosView::NightSky  => "2D celestial sphere · scroll to zoom · click-drag to pan",
                    };
                    ui.label(
                        RichText::new(hint)
                            .size(theme.font_size_small)
                            .color(theme.text_muted())
                            .italics(),
                    );
                });
            });
            ui.separator();

            match state.cosmos_view {
                CosmosView::System    => draw_system_view(ui, theme, state),
                CosmosView::Galactic  => draw_galactic_view(ui, theme, state),
                CosmosView::NightSky  => draw_night_sky_view(ui, theme, state),
            }
        });
}

// ─────────────────────── Body browser (left sidebar) ────────────────────────

/// Region groups for the body browser. Each variant holds the body ids
/// that fall in that region; population happens lazily.
///
/// v0.207.0: Asteroids further subdivided by semi-major axis into the
/// real-astronomy regions so users can see WHICH region a body lives in
/// rather than all asteroids being lumped together (which made it
/// confusing why Ryugu showed up between Earth and Mars vs. Vesta out
/// past the main belt). The cutoffs follow standard convention:
///   - **Near-Earth Asteroids (NEA)**: semi-major axis < 1.3 AU
///   - **Main Belt**: 1.3 ≤ a < 4.0 AU (covers Hildas + Trojans too;
///     we'll split those once we have sample bodies in those regions)
///   - **Trans-Neptunian / Outer Belt**: a ≥ 4.0 AU (Kuiper Belt + scattered disk)
/// Same buckets a planetary scientist would use.
fn body_regions() -> Vec<(&'static str, Vec<&'static SolBody>)> {
    let bodies = sol_bodies();
    let mut star = Vec::new();
    let mut inner = Vec::new();
    let mut outer = Vec::new();
    let mut dwarfs = Vec::new();
    let mut nea = Vec::new();         // Near-Earth asteroids (< 1.3 AU)
    let mut main_belt = Vec::new();   // Main belt (1.3 ≤ a < 4.0 AU)
    let mut trans_nep = Vec::new();   // Trans-Neptunian / outer (≥ 4.0 AU)

    for b in bodies {
        match (b.body_type.as_str(), b.parent.as_deref()) {
            ("star", _)                  => star.push(b),
            ("terrestrial", Some("sun")) => inner.push(b),
            ("gas_giant",   Some("sun")) |
            ("ice_giant",   Some("sun")) => outer.push(b),
            ("dwarf_planet", _)          => dwarfs.push(b),
            ("asteroid", Some("sun")) => {
                let a = b.semi_major_axis_au;
                if a < 1.3 { nea.push(b); }
                else if a < 4.0 { main_belt.push(b); }
                else { trans_nep.push(b); }
            }
            _ => {} // moons handled per-parent in the sidebar tree
        }
    }
    let by_au = |a: &&SolBody, b: &&SolBody|
        a.semi_major_axis_au.partial_cmp(&b.semi_major_axis_au)
            .unwrap_or(std::cmp::Ordering::Equal);
    inner.sort_by(by_au);
    outer.sort_by(by_au);
    dwarfs.sort_by(by_au);
    nea.sort_by(by_au);
    main_belt.sort_by(by_au);
    trans_nep.sort_by(by_au);

    let mut regions: Vec<(&'static str, Vec<&'static SolBody>)> = vec![
        ("Star",          star),
        ("Inner Planets", inner),
        ("Outer Planets", outer),
        ("Dwarf Planets", dwarfs),
    ];
    // Only show non-empty asteroid sub-regions so users don't see "Main
    // Belt (0)" cluttering the sidebar.
    if !nea.is_empty()       { regions.push(("Near-Earth Asteroids", nea)); }
    if !main_belt.is_empty() { regions.push(("Main Belt", main_belt)); }
    if !trans_nep.is_empty() { regions.push(("Trans-Neptunian", trans_nep)); }
    regions
}

fn draw_body_browser(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(
        RichText::new("Celestial Bodies")
            .size(theme.font_size_heading)
            .color(theme.text_primary())
            .strong(),
    );
    ui.label(
        RichText::new("Click a body to focus. Click a planet's ▸ to expand its moons.")
            .size(theme.font_size_small)
            .color(theme.text_muted()),
    );
    ui.add_space(theme.spacing_sm);

    ScrollArea::vertical().show(ui, |ui| {
        for (region_label, members) in body_regions() {
            if members.is_empty() { continue; }
            ui.label(
                RichText::new(region_label)
                    .size(theme.font_size_small)
                    .color(theme.accent())
                    .strong(),
            );
            for body in &members {
                draw_browser_row(ui, theme, state, body, /* depth */ 0);
                // For planets / dwarf planets, expand to show moons if requested.
                if !body.children.is_empty() && state.cosmos_expanded_planets.contains(&body.id) {
                    for moon_id in &body.children {
                        if let Some(moon) = find_body(moon_id) {
                            draw_browser_row(ui, theme, state, moon, /* depth */ 1);
                        }
                    }
                }
            }
            ui.add_space(theme.spacing_xs);
        }
    });
}

fn draw_browser_row(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, body: &SolBody, depth: usize) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.add_space(depth as f32 * 14.0);
        // Expand chevron only if the body has children (planets / dwarf planets with moons).
        if !body.children.is_empty() {
            let expanded = state.cosmos_expanded_planets.contains(&body.id);
            let chevron = if expanded { "▾" } else { "▸" };
            let resp = ui.add(egui::Label::new(
                RichText::new(chevron)
                    .size(theme.font_size_small)
                    .color(theme.text_muted())
                    .monospace(),
            ).sense(Sense::click()));
            if resp.clicked() {
                if expanded {
                    state.cosmos_expanded_planets.remove(&body.id);
                } else {
                    state.cosmos_expanded_planets.insert(body.id.clone());
                }
            }
        } else {
            ui.add_space(10.0);
        }
        // Color dot matching the body's display color.
        let (rect, _r) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
        ui.painter().circle_filled(rect.center(), 4.0, body_color(&body.name));
        // Name — clickable to select.
        let selected = state.cosmos_selected_body.as_deref() == Some(body.id.as_str());
        let label_color = if selected { theme.accent() } else { theme.text_primary() };
        let resp = ui.add(egui::Label::new(
            RichText::new(&body.name).size(theme.font_size_small).color(label_color),
        ).sense(Sense::click()));
        if resp.clicked() {
            state.cosmos_selected_body = Some(body.id.clone());
            // v0.205.0: sidebar clicks also focus the map. The user
            // explicitly said "show me X" by clicking it in the
            // browser — natural to follow up by centering on it.
            state.cosmos_focus_request = Some(body.id.clone());
        }
    });
}

// ─────────────────────── Body details (right sidebar) ───────────────────────

fn draw_body_details(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let body = match state.cosmos_selected_body.as_deref().and_then(find_body) {
        Some(b) => b,
        None => {
            ui.label(
                RichText::new("Select a body")
                    .size(theme.font_size_heading)
                    .color(theme.text_secondary()),
            );
            ui.label(
                RichText::new("Click any planet, moon, or dwarf planet in the left-side browser — its details appear here.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            return;
        }
    };

    ui.horizontal(|ui| {
        ui.label(
            RichText::new(&body.name)
                .size(theme.font_size_title)
                .color(theme.text_primary())
                .strong(),
        );
        // v0.205.0: "Focus" button moves the map view to center on this
        // body. Especially useful when zooming with the cursor parked
        // elsewhere — one click re-centers without manual panning.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::Button::secondary("Focus")
                .tooltip("Center the map view on this body. Combine with scroll-wheel zoom to look closer.")
                .show(ui, theme)
            {
                state.cosmos_focus_request = Some(body.id.clone());
                ui.ctx().request_repaint();
            }
        });
    });
    ui.label(
        RichText::new(format!("{} · {}", titlecase(&body.body_type),
            body.parent.as_deref().map(|p| format!("orbits {}", titlecase(p))).unwrap_or_else(|| "—".to_string())))
            .size(theme.font_size_small)
            .color(theme.accent()),
    );
    ui.add_space(theme.spacing_sm);

    ScrollArea::vertical().show(ui, |ui| {
        // Physical properties.
        section_heading(ui, theme, "Physical");
        if body.radius_km > 0.0 {
            kv(ui, theme, "Radius", &format_km(body.radius_km));
        }
        if body.mass_kg > 0.0 {
            kv(ui, theme, "Mass", &format_mass(body.mass_kg));
        }
        if body.surface_gravity_ms2 > 0.0 {
            kv(ui, theme, "Surface gravity", &format!("{:.2} m/s² ({:.2} g)",
                body.surface_gravity_ms2, body.surface_gravity_ms2 / 9.81));
        }
        if body.mean_temperature_k > 0.0 {
            kv(ui, theme, "Mean temperature", &format_temperature(body.mean_temperature_k));
        }

        // Orbital properties.
        if body.semi_major_axis_au > 0.0 || body.semi_major_axis_km > 0.0 || body.orbital_period_days > 0.0 {
            ui.add_space(theme.spacing_sm);
            section_heading(ui, theme, "Orbit");
            if body.semi_major_axis_au > 0.0 {
                kv(ui, theme, "Semi-major axis", &format!("{:.3} AU", body.semi_major_axis_au));
            } else if body.semi_major_axis_km > 0.0 {
                kv(ui, theme, "Semi-major axis", &format_km(body.semi_major_axis_km));
            }
            if body.orbital_period_days > 0.0 {
                kv(ui, theme, "Orbital period", &format_period(body.orbital_period_days));
            }
        }

        // Atmosphere.
        if !body.atmosphere_summary.is_empty() {
            ui.add_space(theme.spacing_sm);
            section_heading(ui, theme, "Atmosphere");
            ui.label(
                RichText::new(&body.atmosphere_summary)
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
            );
        } else {
            ui.add_space(theme.spacing_sm);
            section_heading(ui, theme, "Atmosphere");
            ui.label(
                RichText::new("None").size(theme.font_size_small).color(theme.text_muted()).italics(),
            );
        }

        // Discovery.
        if body.discovery_year > 0 || !body.discoverer.is_empty() {
            ui.add_space(theme.spacing_sm);
            section_heading(ui, theme, "Discovery");
            if body.discovery_year > 0 {
                kv(ui, theme, "Year", &body.discovery_year.to_string());
            }
            if !body.discoverer.is_empty() {
                kv(ui, theme, "Discoverer", &body.discoverer);
            }
        }

        // Description / flavor.
        if !body.description.is_empty() {
            ui.add_space(theme.spacing_md);
            ui.separator();
            ui.add_space(theme.spacing_xs);
            ui.label(
                RichText::new(&body.description)
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
            );
        }

        // Children list (moons of a planet).
        if !body.children.is_empty() {
            ui.add_space(theme.spacing_md);
            section_heading(ui, theme, &format!("Moons ({})", body.children.len()));
            for moon_id in &body.children {
                if let Some(moon) = find_body(moon_id) {
                    let resp = ui.add(egui::Label::new(
                        RichText::new(format!("• {}", &moon.name))
                            .size(theme.font_size_small)
                            .color(theme.text_secondary()),
                    ).sense(Sense::click()));
                    if resp.clicked() {
                        state.cosmos_selected_body = Some(moon.id.clone());
                        // v0.205.0: also focus on the moon (which falls
                        // back to its parent planet via the focus_request
                        // handler since moons live in the cosmetic ring).
                        state.cosmos_focus_request = Some(moon.id.clone());
                    }
                }
            }
        }
    });
}

fn section_heading(ui: &mut egui::Ui, theme: &Theme, text: &str) {
    ui.label(
        RichText::new(text)
            .size(theme.font_size_small)
            .color(theme.accent())
            .strong(),
    );
    ui.add_space(2.0);
}

fn kv(ui: &mut egui::Ui, theme: &Theme, key: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{}:", key)).size(theme.font_size_small).color(theme.text_muted()));
        ui.label(RichText::new(value).size(theme.font_size_small).color(theme.text_primary()));
    });
}

fn format_km(km: f64) -> String {
    if km >= 1.0e6 { format!("{:.2} million km", km / 1.0e6) }
    else if km >= 1.0e3 { format!("{} km", format_with_commas(km as i64)) }
    else { format!("{:.1} km", km) }
}

fn format_mass(kg: f64) -> String {
    if kg >= 1.0e24 { format!("{:.3e} kg ({:.2} Earth masses)", kg, kg / 5.972e24) }
    else if kg >= 1.0e20 { format!("{:.3e} kg", kg) }
    else { format!("{:.3e} kg", kg) }
}

fn format_temperature(k: f64) -> String {
    let celsius = k - 273.15;
    let fahrenheit = celsius * 9.0 / 5.0 + 32.0;
    format!("{:.0} K  ({:.0} °C / {:.0} °F)", k, celsius, fahrenheit)
}

fn format_period(days: f64) -> String {
    if days.abs() >= 365.0 { format!("{:.2} years ({:.0} days)", days / 365.25, days) }
    else if days.abs() >= 1.0 { format!("{:.2} days", days) }
    else { format!("{:.2} hours", days * 24.0) }
}

fn view_tab(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &mut GuiState,
    mode: CosmosView,
    label: &str,
    tooltip: &str,
) {
    let active = state.cosmos_view == mode;
    if widgets::Button::secondary(label).active(active).tooltip(tooltip).show(ui, theme) {
        state.cosmos_view = mode;
        // Reset zoom + pan on view change so the user lands at a sensible default.
        state.cosmos_zoom = 1.0;
        state.cosmos_pan = Vec2::ZERO;
    }
}

// ─────────────────────── Pan + zoom helper ──────────────────────────────────

/// Common pan/zoom interaction — returns `(rect, response, center, zoom)`
/// for the canvas region.
///
/// v0.205.0 fixes (operator pushback 2026-05-10):
///   1. **Multiplicative zoom** instead of additive. At zoom=50, additive
///      `+= 0.005 * scroll` was a 0.01% step per scroll tick — basically
///      invisible. Multiplicative `*= 1.05^ticks` gives the same percent
///      change at every zoom level, so scroll feels consistent from 0.1×
///      to 50×.
///   2. **Zoom toward cursor.** The point under the cursor stays anchored
///      during zoom. Without this, every zoom-in re-centers on whatever
///      is at canvas center (usually the Sun). Standard map-UI convention
///      (Google Maps, Blender, Photoshop, etc.) is to anchor on cursor.
fn allocate_canvas(
    ui: &mut egui::Ui,
    state: &mut GuiState,
) -> (Rect, egui::Response, Pos2, f32) {
    let available = ui.available_size();
    let (rect, response) = ui.allocate_exact_size(available, Sense::click_and_drag());

    // Pan — click-drag (apply BEFORE zoom so cursor-anchored zoom uses
    // the latest pan).
    if response.dragged() {
        state.cosmos_pan += response.drag_delta();
    }

    // Zoom — multiplicative + cursor-anchored.
    let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
    if scroll_delta != 0.0 && ui.rect_contains_pointer(rect) {
        let cursor = ui.input(|i| i.pointer.hover_pos()).unwrap_or(rect.center());

        // 1.0015 per scroll-pixel = ~1.08× per typical 50-px scroll tick.
        // Picked to feel responsive without being jumpy.
        let zoom_before = state.cosmos_zoom;
        let zoom_after = (zoom_before * (1.0015_f32).powf(scroll_delta)).clamp(0.05, 200.0);

        // Cursor-anchored: shift pan so the world point under the cursor
        // ends up at the same screen position after the zoom change. Math:
        //   cursor_offset = cursor - rect.center()  (cursor relative to canvas center)
        //   ratio = zoom_after / zoom_before
        //   new_pan = cursor_offset * (1 - ratio) + old_pan * ratio
        // Derivation in the cosmos doc / commit message.
        let ratio = zoom_after / zoom_before;
        let cursor_offset = cursor - rect.center();
        state.cosmos_pan = cursor_offset * (1.0 - ratio) + state.cosmos_pan * ratio;
        state.cosmos_zoom = zoom_after;
    }

    let center = rect.center() + state.cosmos_pan;
    (rect, response, center, state.cosmos_zoom)
}

// ─────────────────────── System view ────────────────────────────────────────

fn draw_system_view(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // ── Initialize sim_time to current real-world time on first open
    //    so the user immediately sees today's planetary configuration
    //    (operator 2026-05-10: "if we could have all the planets line
    //    up in real life as they are today on May 12, 2026 then we
    //    could see what it'd look like on a great conjunction"). v0.208.0. ──
    if !state.cosmos_sim_time_initialized {
        state.cosmos_sim_time_seconds = current_real_time_seconds_since_j2000();
        state.cosmos_sim_time_initialized = true;
    }
    // Advance sim_time by real-time delta × sim_speed.
    let now = std::time::Instant::now();
    if let Some(last) = state.cosmos_last_real_instant {
        let dt_real = now.duration_since(last).as_secs_f64();
        state.cosmos_sim_time_seconds += dt_real * state.cosmos_sim_speed;
    }
    state.cosmos_last_real_instant = Some(now);
    // Capture sim_time for this frame.
    let sim_time = state.cosmos_sim_time_seconds;

    // ── Time controls panel — top of the canvas area ──
    draw_time_controls(ui, theme, state);

    // ── Allocate the canvas + handle 3D camera input ──
    let available = ui.available_size();
    let (rect, response) = ui.allocate_exact_size(available, Sense::click_and_drag());
    let paint = ui.painter_at(rect);
    paint.rect_filled(rect, Rounding::ZERO, Color32::from_rgb(8, 8, 14));  // theme-exempt: deep-space backdrop

    // Mouse-drag controls:
    //   - plain drag: rotate camera (yaw + pitch)
    //   - shift+drag: pan target in the camera's local plane
    //   - middle-drag: same as shift (alternative for trackpads)
    if response.dragged() {
        let modifiers = ui.input(|i| i.modifiers);
        let middle = response.dragged_by(egui::PointerButton::Middle);
        if modifiers.shift || middle {
            // Pan target in screen-relative XZ plane scaled by camera distance.
            // Drag right → camera looks further right → target moves left.
            let drag = response.drag_delta();
            let scale = (state.cosmos_camera_3d.distance_au * 0.002) as f32;
            let yaw = state.cosmos_camera_3d.yaw_rad as f32;
            // Camera right vector = perpendicular to forward in XZ plane.
            let right = glam::DVec3::new(-yaw.sin() as f64, 0.0, yaw.cos() as f64);
            let up = glam::DVec3::new(0.0, 1.0, 0.0);
            state.cosmos_camera_3d.target_au -= right * (drag.x * scale) as f64;
            state.cosmos_camera_3d.target_au += up    * (drag.y * scale) as f64;
        } else {
            // Rotate. Sensitivity: 1 px = 0.005 rad (~0.3°).
            let drag = response.drag_delta();
            state.cosmos_camera_3d.yaw_rad   -= (drag.x * 0.005) as f64;
            state.cosmos_camera_3d.pitch_rad += (drag.y * 0.005) as f64;
            // Clamp pitch to (-π/2 + ε, π/2 - ε) to avoid gimbal flip.
            let lim = std::f64::consts::FRAC_PI_2 - 0.01;
            state.cosmos_camera_3d.pitch_rad = state.cosmos_camera_3d.pitch_rad.clamp(-lim, lim);
        }
    }

    // Scroll = multiplicative zoom (decreases distance).
    let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
    if scroll_delta != 0.0 && ui.rect_contains_pointer(rect) {
        let factor = (1.0015_f64).powf(-scroll_delta as f64);
        state.cosmos_camera_3d.distance_au = (state.cosmos_camera_3d.distance_au * factor)
            .clamp(0.001, 10_000.0);
    }

    let bodies = sol_bodies();

    // ── Handle focus requests BEFORE projection (so the request takes
    //    effect on this frame's render, not next frame). Focus simply
    //    moves the camera target to the body's world position; the user
    //    can adjust distance with scroll. ──
    if let Some(focus_id) = state.cosmos_focus_request.take() {
        if let Some(body) = find_body(&focus_id) {
            state.cosmos_camera_3d.target_au = body_world_position_3d_au(body, sim_time);
            // For moons (small bodies orbiting close to their parent),
            // also auto-zoom in close so the moon is meaningfully visible.
            // Without this, focusing on Phobos would just put Mars in
            // the center — Phobos itself sub-pixel.
            if body.body_type == "moon" && body.semi_major_axis_km > 0.0 {
                let moon_orbit_au = body.semi_major_axis_km / KM_PER_AU;
                // 8x the moon's orbital radius gives a comfortable view.
                state.cosmos_camera_3d.distance_au = (moon_orbit_au * 8.0).max(0.001);
            } else if body.semi_major_axis_au > 0.0 {
                // For planets — auto-distance proportional to AU so Mercury
                // doesn't end up the same view-distance as Pluto.
                let auto_d = (body.semi_major_axis_au * 0.3).clamp(0.5, 80.0);
                // Only shrink, never grow — preserves user's preferred
                // wide view if they were already zoomed out.
                if state.cosmos_camera_3d.distance_au > auto_d * 5.0 {
                    state.cosmos_camera_3d.distance_au = auto_d;
                }
            }
        }
        ui.ctx().request_repaint();
    }

    // ── Project all bodies, sort by depth for back-to-front draw ──
    // Re-read the camera AFTER focus may have moved the target on this frame.
    let cam = state.cosmos_camera_3d;
    let _cam_pos = cam.position(); // computed once for any future overlays
    struct ProjectedBody<'a> {
        body: &'a SolBody,
        screen: Pos2,
        depth: f64,
        world_au: glam::DVec3,
    }
    let mut projected: Vec<ProjectedBody> = Vec::with_capacity(bodies.len());
    for body in bodies {
        let world = body_world_position_3d_au(body, sim_time);
        if let Some((screen, depth)) = project_to_screen(world, &cam, rect) {
            projected.push(ProjectedBody { body, screen, depth, world_au: world });
        }
    }
    // Back-to-front: largest depth first (farther from camera).
    projected.sort_by(|a, b| b.depth.partial_cmp(&a.depth).unwrap_or(std::cmp::Ordering::Equal));

    // ── Draw orbit ellipses (v0.207.0: real ellipses with eccentricity
    //    + inclination + argument of perihelion + longitude of ascending
    //    node). Uses sample_orbit_points for the math; same code path
    //    handles direct sun-orbiters AND moons because both are just
    //    "body in parent's frame" — the parent's world position is
    //    folded in by adding it after projection.
    for body in bodies {
        let parent_world = if let Some(parent_id) = &body.parent {
            if parent_id == "sun" {
                glam::DVec3::ZERO
            } else if let Some(parent) = find_body(parent_id) {
                body_world_position_3d_au(parent, sim_time)
            } else {
                continue;
            }
        } else {
            continue; // Sun has no orbit
        };
        let n = if body.body_type == "moon" { 48 } else { 96 };
        let points = sample_orbit_points(body, n);
        if points.is_empty() { continue; }
        let stroke = if body.body_type == "moon" {
            Stroke::new(0.4, Color32::from_rgb(35, 35, 55))  // theme-exempt: moon orbit — faintest
        } else {
            Stroke::new(0.6, Color32::from_rgb(45, 45, 70))  // theme-exempt: planet orbit — faint
        };
        let mut prev: Option<Pos2> = None;
        for p in &points {
            let world_pos = parent_world + *p;
            if let Some((pp, _)) = project_to_screen(world_pos, &cam, rect) {
                if let Some(prev_p) = prev {
                    paint.line_segment([prev_p, pp], stroke);
                }
                prev = Some(pp);
            } else {
                prev = None;
            }
        }
    }

    // Hover position is consumed by both Lagrange hover (just below) and
    // the body draw pass below — declare once here.
    let hover_pos = ui.input(|i| i.pointer.hover_pos());

    // ── Lagrange-point overlay (Phase 4d-soi, v0.211.0) ──
    // Render L1-L5 markers for each interesting parent-child pair when the
    // overlay is toggled on. Hovering a marker shows its name and any
    // notable things parked there (JWST, Trojans, etc.).
    let mut lagrange_markers: Vec<(String, Pos2, &'static str)> = Vec::new();
    if state.cosmos_show_lagrange {
        for pair in LAGRANGE_PAIRS {
            let Some(parent) = find_body(pair.parent_id) else { continue; };
            let Some(child) = find_body(pair.child_id) else { continue; };
            let Some(points) = compute_lagrange_points(parent, child, sim_time) else {
                continue;
            };
            let names = ["L1", "L2", "L3", "L4", "L5"];
            for (i, pt) in points.iter().enumerate() {
                if let Some((screen, _depth)) = project_to_screen(*pt, &cam, rect) {
                    // Render the marker — small "×" via two crossed segments.
                    let s = 5.0_f32;
                    let color = theme.info();
                    paint.line_segment(
                        [screen + Vec2::new(-s, -s), screen + Vec2::new(s, s)],
                        Stroke::new(1.0, color),
                    );
                    paint.line_segment(
                        [screen + Vec2::new(-s, s), screen + Vec2::new(s, -s)],
                        Stroke::new(1.0, color),
                    );
                    // Tiny dot at the center for emphasis.
                    paint.circle_filled(screen, 1.5, color);
                    let label = format!("{} {}", pair.pair_label, names[i]);
                    lagrange_markers.push((label.clone(), screen, names[i]));
                    // Light text label below the × — only render if zoomed
                    // in enough that markers aren't tightly clustered.
                    // (Heuristic: skip labels when parent-child screen
                    // distance is < 40 px.)
                    let Some((parent_screen, _)) = project_to_screen(
                        body_world_position_3d_au(parent, sim_time), &cam, rect)
                    else { continue; };
                    let Some((child_screen, _)) = project_to_screen(
                        body_world_position_3d_au(child, sim_time), &cam, rect)
                    else { continue; };
                    let pair_screen_dist = (child_screen - parent_screen).length();
                    if pair_screen_dist > 50.0 {
                        paint.text(
                            screen + Vec2::new(0.0, 8.0),
                            Align2::CENTER_TOP,
                            names[i],
                            egui::FontId::proportional(9.0),
                            theme.info(),
                        );
                    }
                }
            }
        }

        // Lagrange marker hover tooltip — show name + notable parking
        // when the pointer is near a marker.
        if let Some(hp) = hover_pos {
            for pair in LAGRANGE_PAIRS {
                let Some(parent) = find_body(pair.parent_id) else { continue; };
                let Some(child) = find_body(pair.child_id) else { continue; };
                let Some(points) = compute_lagrange_points(parent, child, sim_time) else {
                    continue;
                };
                let names = ["L1", "L2", "L3", "L4", "L5"];
                for (i, pt) in points.iter().enumerate() {
                    if let Some((screen, _depth)) = project_to_screen(*pt, &cam, rect) {
                        if (hp - screen).length() < 10.0 {
                            let lname = names[i];
                            let notable_text = pair.notable.iter()
                                .find(|(n, _)| *n == lname)
                                .map(|(_, what)| *what)
                                .unwrap_or("");
                            response.clone().on_hover_ui_at_pointer(|ui| {
                                ui.set_max_width(220.0);
                                ui.label(
                                    RichText::new(format!("{} {}", pair.pair_label, lname))
                                        .strong()
                                        .color(theme.text_primary()),
                                );
                                if !notable_text.is_empty() {
                                    ui.label(
                                        RichText::new(notable_text)
                                            .size(theme.font_size_small)
                                            .color(theme.text_secondary()),
                                    );
                                }
                                let blurb = match lname {
                                    "L1" => "Between the two bodies — unstable, needs station-keeping. Good for solar wind observatories looking 'between' the parent and child.",
                                    "L2" => "Past the child along the parent-child line — unstable but a low-station-keeping shadow zone. Telescope heaven.",
                                    "L3" => "On the opposite side of the parent from the child — too far + occluded to be very useful.",
                                    "L4" => "60° ahead of the child in its orbit — stable for μ < 0.0385. Trojan asteroids accumulate here.",
                                    "L5" => "60° behind the child in its orbit — stable for μ < 0.0385. Mirror of L4.",
                                    _ => "",
                                };
                                ui.label(
                                    RichText::new(blurb)
                                        .size(theme.font_size_small)
                                        .color(theme.text_muted())
                                        .italics(),
                                );
                            });
                        }
                    }
                }
            }
        }
    }
    let _ = lagrange_markers; // marker list saved for future click-to-track

    // ── Overlay toggles (top-right of canvas) ──
    // Lagrange + Reference orbits each get a small floating button so the
    // user can flip overlays on/off without leaving the system view.
    let toggles_rect = Rect::from_min_size(
        Pos2::new(rect.right() - 260.0, rect.top() + 8.0),
        egui::vec2(252.0, 22.0),
    );
    let mut toggle_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(toggles_rect)
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
    );
    // Lagrange toggle (rightmost).
    let lag_label = if state.cosmos_show_lagrange { "Lagrange: ON" } else { "Lagrange: OFF" };
    let lag_btn = egui::Button::new(
        RichText::new(lag_label)
            .size(theme.font_size_small)
            .color(if state.cosmos_show_lagrange { theme.info() } else { theme.text_secondary() }),
    )
    .fill(theme.bg_card())
    .stroke(Stroke::new(
        0.8,
        if state.cosmos_show_lagrange { theme.info() } else { theme.border() },
    ));
    if toggle_ui.add(lag_btn).on_hover_text("Toggle Lagrange-point overlay (L1-L5 for Sun-Earth, Earth-Moon, Sun-Mars, Sun-Jupiter, Sun-Saturn pairs)").clicked() {
        state.cosmos_show_lagrange = !state.cosmos_show_lagrange;
    }
    // Reference orbits toggle (next-to-left, only meaningful for Earth/Mars/Jupiter/Moon).
    let ref_label = if state.cosmos_show_reference_orbits { "Ref orbits: ON" } else { "Ref orbits: OFF" };
    let ref_btn = egui::Button::new(
        RichText::new(ref_label)
            .size(theme.font_size_small)
            .color(if state.cosmos_show_reference_orbits { theme.info() } else { theme.text_secondary() }),
    )
    .fill(theme.bg_card())
    .stroke(Stroke::new(
        0.8,
        if state.cosmos_show_reference_orbits { theme.info() } else { theme.border() },
    ));
    if toggle_ui.add(ref_btn).on_hover_text("Toggle reference-orbit rings (LEO/MEO/GEO/HEO around Earth; equivalent rings for Mars, Jupiter, Moon). Visible when zoomed in close enough.").clicked() {
        state.cosmos_show_reference_orbits = !state.cosmos_show_reference_orbits;
    }

    // ── Sky events: detect conjunctions + eclipses for the current sim_time ──
    // (Phase 4d-tri, v0.210.0.) Cheap O(n²) over ~11 named bodies — runs every
    // frame; updates live as the user scrubs sim_time.
    let (conjunctions, eclipse) = detect_sky_events(sim_time);
    // Build a per-body conjunction-tightness map for visual highlighting.
    // Each body in any conjunction gets the TIGHTEST tightness it's involved in.
    let mut body_tightness: std::collections::HashMap<&str, ConjunctionTightness> =
        std::collections::HashMap::new();
    for ev in &conjunctions {
        // Skip events involving the Sun if there's also an eclipse — the
        // eclipse rendering handles Sun+Moon already.
        if eclipse.is_some() && (ev.body_a_id == "sun" || ev.body_b_id == "sun")
            && (ev.body_a_id == "moon" || ev.body_b_id == "moon") {
            continue;
        }
        for id in [ev.body_a_id.as_str(), ev.body_b_id.as_str()] {
            let entry = body_tightness.entry(id).or_insert(ev.tightness);
            if ev.tightness > *entry { *entry = ev.tightness; }
        }
    }

    // ── Draw bodies (back-to-front, with depth-scaled radii) ──
    //
    // Render order (v0.214.0 — operator feedback: "it should be a single
    // thing, not three separate widgets, so the planet icon remains on
    // the original first one"):
    //
    //   1. Compute body radii up front (used for pill-cap sizing).
    //   2. PHASE 1 — paint pill backgrounds (filled rounded-rects). The
    //      body's screen-position is the pill's left-cap center, so the
    //      body circle (drawn next) becomes the visible pill dot.
    //   3. Draw body circles + selected ring + conjunction rings +
    //      eclipse highlights — these all render ON TOP of pill backgrounds.
    //      The body IS the pill's left cap.
    //   4. PHASE 2 — paint pill borders + name text + click interaction.
    //   5. PHASE 3 — for any expanded pill, paint the card extension
    //      below it (flush with pill, sharing the left edge).
    //
    // (`hover_pos` declared earlier — shared with the Lagrange overlay pass.)
    let mut hovered_body: Option<&SolBody> = None;
    let mut clicked_body: Option<&SolBody> = None;

    // PASS A — compute body radii up front (no paint yet). We need radii
    // before we can lay out pills (the pill height matches the body's
    // diameter so the body becomes the pill's left cap).
    let mut body_radii_px: Vec<f32> = Vec::with_capacity(projected.len());
    for pb in &projected {
        let body_radius_au = pb.body.radius_km / KM_PER_AU;
        let apparent_rad = (body_radius_au / pb.depth).abs();
        let pixels_per_rad = (rect.height() as f64) / (cam.fov_rad);
        let mut r_px = (apparent_rad * pixels_per_rad) as f32;
        let min_r = if pb.body.id == "sun" { 8.0 }
                    else if pb.body.body_type == "moon" { 1.5 }
                    else if pb.body.body_type == "asteroid" { 1.0 }
                    else { 2.5 };
        let max_r = if pb.body.id == "sun" { 80.0 } else { 40.0 };
        r_px = r_px.max(min_r).min(max_r);
        body_radii_px.push(r_px);
    }

    // PASS B — hit-test hover/click on body screen positions (still no
    // paint). The pill widget owns the click handler for any pilled body
    // (because the pill rect encompasses the body's left cap area), so
    // body hits here only matter for non-pilled bodies that the user
    // clicks on the canvas to select.
    for (i, pb) in projected.iter().enumerate() {
        let r_px = body_radii_px[i];
        if let Some(hp) = hover_pos {
            if (hp - pb.screen).length() < r_px + 4.0 {
                hovered_body = Some(pb.body);
                if response.clicked() {
                    clicked_body = Some(pb.body);
                }
            }
        }
    }
    let _suppress_unused: Option<glam::DVec3> = projected.first().map(|p| p.world_au);

    // PASS C — build pill input list. Decide visibility (priority tier
    // or forced via hover/select/expand) before any rendering happens.
    //
    // Priority key (lower = drawn first / always-visible):
    //   0 = star (Sun)
    //   1 = terrestrial / gas_giant / ice_giant (true planets)
    //   2 = dwarf_planet (Pluto, Ceres, etc.)
    //   3 = moon (shown only when hovered/selected/expanded)
    //   4 = asteroid (shown only when hovered/selected/expanded)
    use crate::gui::widgets::body_pill::{
        BodyPill, paint_pill_backgrounds, paint_pill_overlays,
    };
    let mut pills: Vec<BodyPill> = Vec::with_capacity(projected.len());
    for (i, pb) in projected.iter().enumerate() {
        let r_px = body_radii_px[i];
        let priority: u8 = match pb.body.body_type.as_str() {
            "star" => 0,
            "terrestrial" | "gas_giant" | "ice_giant" => 1,
            "dwarf_planet" => 2,
            "moon" => 3,
            _ => 4,
        };
        let is_hovered = hovered_body.map(|b| b.id == pb.body.id).unwrap_or(false);
        let is_selected = state.cosmos_selected_body.as_deref() == Some(pb.body.id.as_str());
        let is_expanded = state.cosmos_expanded_body.as_deref() == Some(pb.body.id.as_str());
        let is_forced = is_hovered || is_selected || is_expanded;
        let should_show = priority <= 2 || is_forced;
        if !should_show { continue; }
        pills.push(BodyPill {
            id: pb.body.id.as_str(),
            name: pb.body.name.as_str(),
            color: body_color(&pb.body.name),
            body_screen: pb.screen,
            body_radius_px: r_px,
            priority,
            forced: is_forced,
            expanded: is_expanded,
        });
    }

    // PHASE 1 — paint pill BACKGROUNDS only (filled rounded-rects).
    // Body circles render on top of these and become the pills' left caps.
    let pill_layout = paint_pill_backgrounds(&paint, theme, &pills);

    // PASS D — draw body circles + selected ring + conjunction rings +
    // eclipse highlights. These overlay the pill backgrounds, completing
    // the "body IS the pill's left dot" visual trick. Order matters:
    // pill background first, body circle on top of the cap area, then
    // body decorations on top of body.
    for (i, pb) in projected.iter().enumerate() {
        let r_px = body_radii_px[i];

        paint.circle_filled(pb.screen, r_px, body_color(&pb.body.name));
        if state.cosmos_selected_body.as_deref() == Some(pb.body.id.as_str()) {
            paint.circle_stroke(pb.screen, r_px + 3.0, Stroke::new(1.5, theme.accent()));
        }

        // Conjunction highlight ring — bodies currently in a conjunction
        // pulse with a warm-tone ring. Tightness drives ring thickness
        // and brightness. v0.210.0.
        if let Some(t) = body_tightness.get(pb.body.id.as_str()) {
            let weight = t.weight();
            let outer_r = r_px + 6.0 + weight * 4.0;
            let inner_r = r_px + 3.0;
            paint.circle_stroke(
                pb.screen,
                outer_r,
                Stroke::new(1.0 + weight * 2.0, theme.warning()),
            );
            paint.circle_stroke(
                pb.screen,
                inner_r,
                Stroke::new(0.5, theme.warning()),
            );
        }
        // Eclipse highlight on Earth — Sun-Moon overlap. Render a strong
        // accent ring around Earth and a darker disc overlaid on the Sun.
        if let Some(ref ev) = eclipse {
            if pb.body.id == "earth" {
                paint.circle_stroke(
                    pb.screen,
                    r_px + 8.0,
                    Stroke::new(2.0, theme.danger()),
                );
                paint.text(
                    pb.screen + Vec2::new(0.0, r_px + 22.0),
                    Align2::CENTER_TOP,
                    "ECLIPSE",
                    egui::FontId::proportional(11.0),
                    theme.danger(),
                );
                let _ = ev;
            } else if pb.body.id == "sun" {
                let dark_r = (r_px as f64 * ev.coverage.sqrt()).clamp(0.0, r_px as f64) as f32;
                paint.circle_filled(pb.screen, dark_r, theme.bg_primary());
                paint.circle_stroke(pb.screen, r_px + 2.0, Stroke::new(1.0, theme.danger()));
            } else if pb.body.id == "moon" {
                paint.circle_stroke(
                    pb.screen,
                    r_px + 4.0,
                    Stroke::new(1.5, theme.danger()),
                );
            }
        }
    }

    // ── Reference-orbit rings (Phase 4d-ref, v0.212.0) ──
    // Render LEO / MEO / GEO / etc rings around any supported planet
    // when the toggle is on AND the body is large enough on screen to
    // make rings legible. Currently supports Earth, Mars, Jupiter, Moon.
    let mut ref_orbit_hover: Option<(&'static str, &'static str)> = None;
    if state.cosmos_show_reference_orbits {
        for (i, pb) in projected.iter().enumerate() {
            if reference_orbits_for(pb.body.id.as_str()).is_empty() { continue; }
            let r_px = body_radii_px[i];
            if let Some(hit) = draw_reference_orbits(
                &paint, theme, pb.body, pb.world_au, pb.screen, r_px,
                &cam, rect, hover_pos,
            ) {
                ref_orbit_hover = Some(hit);
            }
        }
    }
    if let Some((name, blurb)) = ref_orbit_hover {
        response.clone().on_hover_ui_at_pointer(|ui| {
            ui.set_max_width(240.0);
            ui.label(RichText::new(name).strong().color(theme.text_primary()));
            ui.label(
                RichText::new(blurb)
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
            );
        });
    }

    // ── PHASE 3 (called BEFORE Phase 2 — see widget docstring):
    //    paint the details PANEL behind the expanded pill so the pill
    //    border layers on top of the panel border. The panel uses a SOFT
    //    gray border; the pill uses a STRONG accent border. Per operator
    //    feedback 2026-05-12 ("notice that this section has a off color
    //    border compared to the top — black is strong, gray is soft").
    if let Some(expanded_id) = state.cosmos_expanded_body.clone() {
        let placed = pill_layout.placed.iter().find(|pp| pp.id == expanded_id).cloned();
        let body = projected.iter().find(|pb| pb.body.id == expanded_id).map(|pb| pb.body);
        if let (Some(pp), Some(body)) = (placed, body) {
            draw_body_info_card_v2(ui, &paint, theme, body, &pp, rect, state);
        }
    }

    // ── PHASE 2: paint pill OVERLAYS (borders + name text + click handling).
    // Pill borders render on top of body circles + their decorations AND
    // on top of any panel border drawn by Phase 3 above. The user sees:
    //   • The body, with the pill's strong accent border wrapping it.
    //   • The pill name to the right of the body.
    //   • If expanded, a soft-gray panel extending right + down from the
    //     pill's bottom-right corner.
    let clicked_pill = paint_pill_overlays(ui, &paint, theme, &pill_layout, "cosmos_pill");

    // Apply click selection. Pill clicks take precedence over body clicks.
    if let Some(pill_id) = clicked_pill {
        // Toggle expanded state on pill click.
        if state.cosmos_expanded_body.as_deref() == Some(pill_id.as_str()) {
            state.cosmos_expanded_body = None;
        } else {
            state.cosmos_expanded_body = Some(pill_id.clone());
            state.cosmos_selected_body = Some(pill_id);
        }
    } else if let Some(b) = clicked_body {
        // Body click (no pill on this body) — select it. Next frame the
        // pill will appear because the body is now selected (forced=true).
        state.cosmos_selected_body = Some(b.id.clone());
    }

    // ── Hover tooltip ──
    if let (Some(body), Some(_)) = (hovered_body, hover_pos) {
        response.on_hover_ui_at_pointer(|ui| {
            ui.set_max_width(280.0);
            ui.label(RichText::new(&body.name).size(theme.font_size_body).color(theme.text_primary()).strong());
            let dist_str = if body.semi_major_axis_au > 0.0 {
                format!("{:.2} AU from Sun", body.semi_major_axis_au)
            } else if body.semi_major_axis_km > 0.0 {
                format!("{} km from {}", format_with_commas(body.semi_major_axis_km as i64),
                    body.parent.as_deref().map(titlecase).unwrap_or_default())
            } else {
                String::new()
            };
            ui.label(RichText::new(format!("{} · {}", titlecase(&body.body_type), dist_str))
                .size(theme.font_size_small).color(theme.text_secondary()));
            if body.radius_km > 0.0 {
                ui.label(RichText::new(format!("Radius: {}", format_km(body.radius_km)))
                    .size(theme.font_size_small).color(theme.text_muted()));
            }
            ui.label(
                RichText::new("Click to open details · drag to rotate · scroll to zoom · shift+drag to pan")
                    .size(theme.font_size_small)
                    .color(theme.accent())
                    .italics(),
            );
        });
    }

    // ── HUD overlay: camera state for the operator ──
    paint.text(
        Pos2::new(rect.left() + 8.0, rect.bottom() - 24.0),
        Align2::LEFT_BOTTOM,
        format!("Cam: {:.1} AU from target · yaw {:+.0}° · pitch {:+.0}°",
            cam.distance_au, cam.yaw_rad.to_degrees(), cam.pitch_rad.to_degrees()),
        egui::FontId::proportional(10.0),
        theme.text_muted(),
    );
    paint.text(
        Pos2::new(rect.left() + 8.0, rect.bottom() - 8.0),
        Align2::LEFT_BOTTOM,
        "Drag to rotate · scroll to zoom · shift+drag to pan target · click body to select · Focus button in details panel",
        egui::FontId::proportional(10.0),
        theme.text_muted(),
    );

    // ── Sky-events HUD (Phase 4d-tri, v0.210.0) ──
    // Bottom-right corner readout. Shows up to 3 tightest conjunctions
    // currently in progress + any solar eclipse. Empty when sky is quiet.
    let sky_y_base = rect.bottom() - 8.0;
    let mut sky_lines: Vec<(String, Color32)> = Vec::new();
    if let Some(ref ev) = eclipse {
        let pct = (ev.coverage * 100.0).round();
        sky_lines.push((
            format!(
                "ECLIPSE: {} · {:.0}% coverage · Sun-Moon sep {:.3}°",
                ev.kind.label(), pct, ev.separation_deg
            ),
            theme.danger(),
        ));
    }
    for ev in conjunctions.iter().take(3) {
        // Skip the Sun-Moon pair if there's an eclipse — already reported.
        if eclipse.is_some()
            && (ev.body_a_id == "sun" || ev.body_b_id == "sun")
            && (ev.body_a_id == "moon" || ev.body_b_id == "moon")
        {
            continue;
        }
        let color = match ev.tightness {
            ConjunctionTightness::Occultation => theme.danger(),
            ConjunctionTightness::Tight => theme.warning(),
            ConjunctionTightness::Conjunction => theme.warning(),
            ConjunctionTightness::Close => theme.text_secondary(),
        };
        sky_lines.push((
            format!(
                "{} + {}  {:.2}°  ({})",
                ev.body_a_name, ev.body_b_name, ev.angular_sep_deg, ev.tightness.label()
            ),
            color,
        ));
    }
    if sky_lines.is_empty() {
        sky_lines.push((
            "Sky is quiet (no conjunctions within 5°)".to_string(),
            theme.text_muted(),
        ));
    }
    // Section header.
    paint.text(
        Pos2::new(rect.right() - 8.0, sky_y_base - sky_lines.len() as f32 * 14.0 - 18.0),
        Align2::RIGHT_BOTTOM,
        "Sky events (from Earth's POV)",
        egui::FontId::proportional(10.0),
        theme.text_primary(),
    );
    for (i, (line, color)) in sky_lines.iter().enumerate() {
        let y = sky_y_base - (sky_lines.len() - 1 - i) as f32 * 14.0;
        paint.text(
            Pos2::new(rect.right() - 8.0, y),
            Align2::RIGHT_BOTTOM,
            line,
            egui::FontId::proportional(10.0),
            *color,
        );
    }
}

// ─────────────────────── Galactic view (Sol-centered, ly) ───────────────────

fn draw_galactic_view(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let (rect, response, center, zoom) = allocate_canvas(ui, state);
    let paint = ui.painter_at(rect);
    paint.rect_filled(rect, Rounding::ZERO, Color32::from_rgb(5, 5, 12));  // theme-exempt: deep-space backdrop
    let stars = nearby_stars();
    // Auto-fit: the most distant star in the dataset sets the base scale.
    let max_dist = stars.iter().map(|s| s.distance_ly).fold(15.0, f64::max).max(15.0);
    let scale = ((rect.width().min(rect.height()) as f64 / 2.0 - 30.0) / max_dist) * zoom as f64;

    // Faint grid rings at 5 / 10 / 25 / 50 ly for visual reference.
    for &ring_ly in &[5.0_f64, 10.0, 25.0, 50.0] {
        let r = (ring_ly * scale) as f32;
        if r > 5.0 && r < rect.width().max(rect.height()) {
            paint.circle_stroke(center, r, Stroke::new(0.5, Color32::from_rgb(25, 25, 40)));  // theme-exempt: distance-ring — faint backdrop
            paint.text(
                center + Vec2::new(r * 0.7, -r * 0.7),
                Align2::CENTER_CENTER,
                format!("{} ly", ring_ly as i64),
                egui::FontId::proportional(9.0),
                Color32::from_rgb(80, 80, 110),  // theme-exempt: distance-ring label — faint backdrop
            );
        }
    }

    // Sol at center — the universal anchor.
    paint.circle_filled(center, 4.0_f32.max(2.0 * zoom), body_color("sun"));
    paint.text(
        center + Vec2::new(8.0, 0.0),
        Align2::LEFT_CENTER,
        "Sol",
        egui::FontId::proportional(11.0),
        theme.text_primary(),
    );

    // Nearby stars projected to the X-Y galactic plane (Z dropped).
    let hover_pos = ui.input(|i| i.pointer.hover_pos());
    let mut hovered_star: Option<&NearbyStar> = None;
    for star in stars {
        let px = center.x + (star.pos_ly.x * scale) as f32;
        let py = center.y - (star.pos_ly.y * scale) as f32; // y inverted (screen y grows down)
        let pos = Pos2::new(px, py);
        // Brighter (lower mag) → larger dot.
        let r = ((6.0 - star.apparent_magnitude.min(6.0)) as f32 * 0.8 + 1.5).clamp(1.5, 6.0);
        paint.circle_filled(pos, r, spectral_color(&star.spectral));
        if zoom > 1.5 {
            paint.text(
                pos + Vec2::new(r + 2.0, 0.0),
                Align2::LEFT_CENTER,
                &star.name,
                egui::FontId::proportional(9.0),
                theme.text_secondary(),
            );
        }
        if let Some(hp) = hover_pos {
            if (hp - pos).length() < r + 4.0 {
                hovered_star = Some(star);
            }
        }
    }

    if let Some(star) = hovered_star {
        response.on_hover_ui_at_pointer(|ui| {
            ui.set_max_width(300.0);
            ui.label(RichText::new(&star.name).size(theme.font_size_body).color(theme.text_primary()).strong());
            if star.alt_name != star.name {
                ui.label(RichText::new(format!("aka {}", &star.alt_name))
                    .size(theme.font_size_small).color(theme.text_secondary()).italics());
            }
            ui.label(RichText::new(format!("{:.2} ly from Sol  ·  spectral type {}", star.distance_ly, star.spectral))
                .size(theme.font_size_small).color(theme.text_secondary()));
            ui.label(RichText::new(format!("Apparent magnitude {:.2}", star.apparent_magnitude))
                .size(theme.font_size_small).color(theme.text_muted()));
            ui.label(RichText::new(format!("Galactic position: ({:.3}, {:.3}, {:.3}) ly", star.pos_ly.x, star.pos_ly.y, star.pos_ly.z))
                .size(theme.font_size_small).color(theme.text_muted()).monospace());
        });
    }

    // Footer hint.
    paint.text(
        Pos2::new(rect.left() + 8.0, rect.bottom() - 8.0),
        Align2::LEFT_BOTTOM,
        format!("Top-down galactic plane · {} stars within ~{:.0} ly · X/Y plane, Z dropped", stars.len(), max_dist),
        egui::FontId::proportional(10.0),
        theme.text_muted(),
    );
}

// ─────────────────────── Night Sky view (RA/Dec, Earth-centered) ────────────

fn draw_night_sky_view(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let (rect, response, _center, zoom) = allocate_canvas(ui, state);
    let paint = ui.painter_at(rect);
    paint.rect_filled(rect, Rounding::ZERO, Color32::from_rgb(5, 5, 12));  // theme-exempt: deep-space backdrop
    // Equirectangular projection of the celestial sphere:
    //   RA  in [0, 24) hours → x in [left, right]
    //   Dec in [-90, +90]    → y in [bottom, top]
    let inner = rect.shrink(20.0);
    let project = |ra: f64, dec: f64| -> Pos2 {
        let xnorm = (ra / 24.0).rem_euclid(1.0);
        let ynorm = ((90.0 - dec) / 180.0).clamp(0.0, 1.0);
        // Apply zoom + pan (zoom is centered on the visible center).
        let x = inner.left() + (xnorm as f32) * inner.width();
        let y = inner.top() + (ynorm as f32) * inner.height();
        let from_center = Pos2::new(x, y) - inner.center();
        inner.center() + from_center * zoom + state.cosmos_pan
    };

    // RA / Dec grid every 2 hours / 30 deg.
    for ra in (0..12).map(|i| i as f64 * 2.0) {
        let p1 = project(ra, -90.0);
        let p2 = project(ra, 90.0);
        paint.line_segment([p1, p2], Stroke::new(0.5, Color32::from_rgb(20, 20, 35)));  // theme-exempt: celestial grid line — faint backdrop
    }
    for dec in (-3..=3).map(|i| i as f64 * 30.0) {
        let p1 = project(0.0, dec);
        let p2 = project(24.0, dec);
        paint.line_segment([p1, p2], Stroke::new(0.5, Color32::from_rgb(20, 20, 35)));  // theme-exempt: celestial grid line — faint backdrop
        paint.text(
            project(0.0, dec) + Vec2::new(4.0, 0.0),
            Align2::LEFT_CENTER,
            format!("{:+}°", dec as i32),
            egui::FontId::proportional(8.0),
            Color32::from_rgb(70, 70, 95),  // theme-exempt: declination-axis label — faint backdrop
        );
    }

    // Index bright stars by name for constellation lookup.
    let stars = bright_stars();
    let star_by_name: std::collections::HashMap<&str, &BrightStar> =
        stars.iter().map(|s| (s.name.as_str(), s)).collect();

    // Constellation lines first (drawn under the stars).
    let const_color = Color32::from_rgb(60, 80, 130);  // theme-exempt: constellation line — faint backdrop element
    let cons = constellations();
    let hover_pos = ui.input(|i| i.pointer.hover_pos());
    let mut hovered_constellation: Option<&Constellation> = None;

    for c in cons {
        for (a, b) in &c.lines {
            if let (Some(sa), Some(sb)) = (star_by_name.get(a.as_str()), star_by_name.get(b.as_str())) {
                let pa = project(sa.ra_hours, sa.dec_deg);
                let pb = project(sb.ra_hours, sb.dec_deg);
                paint.line_segment([pa, pb], Stroke::new(1.0, const_color));
                // Label hover detection: did the cursor pass near this line?
                if let Some(hp) = hover_pos {
                    if dist_point_to_segment(hp, pa, pb) < 6.0 {
                        hovered_constellation = Some(c);
                    }
                }
            }
        }
        // Constellation name label at the centroid of its stars.
        let pts: Vec<Pos2> = c.lines.iter()
            .filter_map(|(a, b)| Some([star_by_name.get(a.as_str())?, star_by_name.get(b.as_str())?]))
            .flat_map(|pair| pair.iter().map(|s| project(s.ra_hours, s.dec_deg)).collect::<Vec<_>>())
            .collect();
        if !pts.is_empty() {
            let avg = pts.iter().fold(Vec2::ZERO, |acc, p| acc + p.to_vec2()) / pts.len() as f32;
            paint.text(
                avg.to_pos2(),
                Align2::CENTER_CENTER,
                &c.abbr,
                egui::FontId::proportional(10.0),
                Color32::from_rgb(110, 130, 180),  // theme-exempt: constellation abbreviation label
            );
        }
    }

    // Stars on top.
    let mut hovered_star: Option<&BrightStar> = None;
    for s in stars {
        let p = project(s.ra_hours, s.dec_deg);
        let r = ((4.5 - s.magnitude.min(4.5)) as f32 * 0.7 + 1.0).clamp(1.0, 4.5);
        paint.circle_filled(p, r, spectral_color(&s.spectral));
        if let Some(hp) = hover_pos {
            if (hp - p).length() < r + 4.0 {
                hovered_star = Some(s);
            }
        }
    }

    // Hover priority: a star tooltip beats a constellation tooltip if both fire.
    if let Some(s) = hovered_star {
        response.on_hover_ui_at_pointer(|ui| {
            ui.set_max_width(280.0);
            ui.label(RichText::new(&s.name).size(theme.font_size_body).color(theme.text_primary()).strong());
            ui.label(RichText::new(format!("RA {:.3}h  ·  Dec {:+.2}°", s.ra_hours, s.dec_deg))
                .size(theme.font_size_small).color(theme.text_secondary()).monospace());
            ui.label(RichText::new(format!("Magnitude {:.2}  ·  Spectral {}", s.magnitude, s.spectral))
                .size(theme.font_size_small).color(theme.text_muted()));
        });
    } else if let Some(c) = hovered_constellation {
        response.on_hover_ui_at_pointer(|ui| {
            ui.set_max_width(360.0);
            ui.label(RichText::new(&c.name).size(theme.font_size_heading).color(theme.text_primary()).strong());
            ui.label(RichText::new(format!("{}  ·  {}", &c.abbr, &c.season))
                .size(theme.font_size_small).color(theme.text_secondary()));
            if !c.myth.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new(&c.myth).size(theme.font_size_small).color(theme.text_secondary()));
            }
            if !c.key_stars.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new("Key stars:").size(theme.font_size_small).color(theme.text_primary()).strong());
                for ks in &c.key_stars {
                    ui.label(RichText::new(format!("  • {}", ks)).size(theme.font_size_small).color(theme.text_muted()));
                }
            }
            if !c.objects.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new("Notable objects:").size(theme.font_size_small).color(theme.text_primary()).strong());
                for o in &c.objects {
                    ui.label(RichText::new(format!("  • {}", o)).size(theme.font_size_small).color(theme.text_muted()));
                }
            }
        });
    }

    // Footer hint.
    paint.text(
        Pos2::new(rect.left() + 8.0, rect.bottom() - 8.0),
        Align2::LEFT_BOTTOM,
        format!("Equirectangular celestial sphere · {} bright stars · {} constellations", stars.len(), cons.len()),
        egui::FontId::proportional(10.0),
        theme.text_muted(),
    );
}

// ─────────────────────── Reference orbits (Phase 4d-ref, v0.212.0) ─────────

/// A named reference orbit around a body — LEO / GEO / etc. Stored as
/// altitude above the body's surface in km. Period is computed from the
/// body's mass via Kepler's third law (a³ = GMP²/4π²).
struct ReferenceOrbit {
    name: &'static str,
    altitude_km: f64,
    /// One-line role/description shown on hover.
    blurb: &'static str,
}

/// Hardcoded reference orbits per supported body. Only bodies for which
/// the "common reference orbit" naming applies — Earth has the canonical
/// LEO/MEO/GEO/HEO; Mars/Moon have analogues but they're not as
/// universally-named. Could move to data/ later.
fn reference_orbits_for(body_id: &str) -> &'static [ReferenceOrbit] {
    match body_id {
        "earth" => &[
            ReferenceOrbit { name: "LEO (low)",  altitude_km: 400.0,    blurb: "ISS altitude — 90 min period" },
            ReferenceOrbit { name: "LEO (high)", altitude_km: 2_000.0,  blurb: "Top of LEO band — 130 min period" },
            ReferenceOrbit { name: "MEO",        altitude_km: 20_200.0, blurb: "GPS / GNSS satellite belt — 12 h period" },
            ReferenceOrbit { name: "GEO",        altitude_km: 35_786.0, blurb: "Geostationary — 24 h period, fixed over equator" },
            ReferenceOrbit { name: "HEO",        altitude_km: 50_000.0, blurb: "High Earth orbit — >24 h period" },
            ReferenceOrbit { name: "Lunar TLI",  altitude_km: 384_400.0, blurb: "Trans-lunar injection apoapsis (Moon's orbit)" },
        ],
        "mars" => &[
            ReferenceOrbit { name: "LMO (low)", altitude_km: 200.0,    blurb: "Low Mars orbit (Mars Reconnaissance Orbiter, etc.)" },
            ReferenceOrbit { name: "areostationary", altitude_km: 17_032.0, blurb: "Mars-stationary — 24.6 h period (sol)" },
        ],
        "jupiter" => &[
            ReferenceOrbit { name: "Low Jovian orbit", altitude_km: 5_000.0, blurb: "Inside the radiation belts — punishing" },
            ReferenceOrbit { name: "Jovo-stationary",  altitude_km: 88_500.0, blurb: "Jupiter-stationary — 9.93 h period" },
        ],
        "moon" => &[
            ReferenceOrbit { name: "Low lunar orbit", altitude_km: 100.0, blurb: "100 km — Apollo command module altitude" },
            ReferenceOrbit { name: "NRHO apoapsis",   altitude_km: 70_000.0, blurb: "Near-rectilinear halo orbit — Gateway target" },
        ],
        _ => &[],
    }
}

/// Render reference-orbit rings around the focused body. The rings live
/// in the body's equatorial plane — for our (ecliptic ≈ equatorial)
/// approximation we use the orbital normal of the body's own orbit.
/// At MVP we use the ecliptic z-axis (good enough; Earth's axial tilt
/// of 23.4° is the worst case and the rings still read clearly).
#[allow(clippy::too_many_arguments)]
fn draw_reference_orbits(
    paint: &egui::Painter,
    theme: &Theme,
    body: &SolBody,
    body_world_au: glam::DVec3,
    body_screen: Pos2,
    body_radius_px: f32,
    cam: &Cosmos3DCamera,
    rect: Rect,
    hover_pos: Option<Pos2>,
) -> Option<(&'static str, &'static str)> {
    let rings = reference_orbits_for(body.id.as_str());
    if rings.is_empty() { return None; }

    // Body radius in AU for orbit-radius scaling.
    let body_radius_au = body.radius_km / KM_PER_AU;

    // Threshold — only render rings when the body's apparent radius is
    // > ~12 px (otherwise rings would overlap the body itself or be
    // sub-pixel). Below threshold render nothing.
    if body_radius_px < 12.0 { return None; }

    let mut hovered_info: Option<(&'static str, &'static str)> = None;

    // Sample each ring at N points (uniform in true anomaly, here circular
    // so just uniform angle). Project each through the camera.
    for ring in rings {
        let r_au = body_radius_au + ring.altitude_km / KM_PER_AU;
        // Use the body's orbital inclination as a rough equatorial tilt.
        // Real obliquity (Earth = 23.4°) differs but ecliptic-aligned is
        // good enough for a navigation overlay.
        let n = 96;
        let mut prev: Option<Pos2> = None;
        let mut first: Option<Pos2> = None;
        let mut closest_to_hover: Option<(Pos2, f32)> = None;
        for i in 0..n {
            let theta = (i as f64) * std::f64::consts::TAU / (n as f64);
            // Ring in body's local XY plane (approximately equatorial).
            let local = glam::DVec3::new(
                r_au * theta.cos(),
                r_au * theta.sin(),
                0.0,
            );
            let world = body_world_au + local;
            if let Some((p, _)) = project_to_screen(world, cam, rect) {
                if let Some(prev_p) = prev {
                    paint.line_segment(
                        [prev_p, p],
                        Stroke::new(0.6, theme.info()),
                    );
                }
                if first.is_none() { first = Some(p); }
                prev = Some(p);
                if let Some(hp) = hover_pos {
                    let d = (hp - p).length();
                    if closest_to_hover.map_or(true, |(_, dd)| d < dd) {
                        closest_to_hover = Some((p, d));
                    }
                }
            } else {
                prev = None;
            }
        }
        // Close the ring (segment from last drawn → first drawn).
        if let (Some(prev_p), Some(first_p)) = (prev, first) {
            paint.line_segment([prev_p, first_p], Stroke::new(0.6, theme.info()));
        }
        // Ring label — placed at the screen position of theta=0 (right
        // side of body). Only if visible.
        let label_world = body_world_au + glam::DVec3::new(r_au, 0.0, 0.0);
        if let Some((label_p, _)) = project_to_screen(label_world, cam, rect) {
            paint.text(
                label_p + Vec2::new(6.0, 0.0),
                Align2::LEFT_CENTER,
                ring.name,
                egui::FontId::proportional(9.0),
                theme.info(),
            );
        }
        // Hover detection — if pointer is close to any sampled point on
        // this ring, surface the ring's blurb.
        if let Some((_, d)) = closest_to_hover {
            if d < 6.0 {
                hovered_info = Some((ring.name, ring.blurb));
            }
        }
    }

    let _ = body_screen; // silence unused warning — kept for future use
    hovered_info
}

// ─────────────────────── Lagrange points (Phase 4d-soi, v0.211.0) ──────────

/// A Lagrange-point pair worth rendering. The "parent" is the heavier body
/// (Sun, Earth, etc.), the "child" is the orbiting smaller body.
struct LagrangePair {
    parent_id: &'static str,
    child_id: &'static str,
    pair_label: &'static str, // short name shown on hover
    notable: &'static [(&'static str, &'static str)], // (lpoint, what's there)
}

/// Hardcoded list of interesting Lagrange pairs. Could move to data/ later
/// once we have a notion of "system bodies + L-point notes" in the data
/// format — but for v0.211 a static list of 5 pairs is plenty.
const LAGRANGE_PAIRS: &[LagrangePair] = &[
    LagrangePair {
        parent_id: "sun",
        child_id: "earth",
        pair_label: "Sun-Earth",
        notable: &[
            ("L1", "SOHO solar observatory"),
            ("L2", "JWST + Gaia + Euclid + Planck"),
            ("L4", "Trojan asteroid 2010 TK7"),
        ],
    },
    LagrangePair {
        parent_id: "earth",
        child_id: "moon",
        pair_label: "Earth-Moon",
        notable: &[
            ("L1", "Future lunar gateway candidate"),
            ("L2", "Lunar far-side relay (Queqiao)"),
            ("L4", "Future lunar gateway candidate"),
            ("L5", "Future lunar gateway candidate"),
        ],
    },
    LagrangePair {
        parent_id: "sun",
        child_id: "mars",
        pair_label: "Sun-Mars",
        notable: &[("L4", "Eureka Trojan asteroid")],
    },
    LagrangePair {
        parent_id: "sun",
        child_id: "jupiter",
        pair_label: "Sun-Jupiter",
        notable: &[
            ("L4", "Greek Trojan asteroids (~10,000 known)"),
            ("L5", "Trojan asteroids (Patroclus, Hektor)"),
        ],
    },
    LagrangePair {
        parent_id: "sun",
        child_id: "saturn",
        pair_label: "Sun-Saturn",
        notable: &[],
    },
];

/// Compute the five Lagrange points for a (parent, child) two-body system
/// at the given sim_time. Returns positions in heliocentric AU.
///
/// L1, L2, L3 use the cube-root-of-mass-ratio approximation
/// (good to ~1% for μ < 0.01, which covers all our pairs since even
/// Jupiter/Sun is ~0.001). L4 and L5 are the exact 60°-ahead/behind
/// equilateral triangles in the child's orbit plane.
fn compute_lagrange_points(
    parent: &SolBody,
    child: &SolBody,
    sim_time_seconds: f64,
) -> Option<[glam::DVec3; 5]> {
    if parent.mass_kg <= 0.0 || child.mass_kg <= 0.0 { return None; }
    let parent_pos = body_world_position_3d_au(parent, sim_time_seconds);
    let child_pos = body_world_position_3d_au(child, sim_time_seconds);
    let separation = child_pos - parent_pos;
    let dist = separation.length();
    if dist <= 0.0 { return None; }
    let to_child = separation / dist;

    // μ = m_child / (m_parent + m_child). For Earth-Moon μ ≈ 0.0123;
    // for Sun-Earth μ ≈ 3e-6; for Sun-Jupiter μ ≈ 9.5e-4.
    let mu = child.mass_kg / (parent.mass_kg + child.mass_kg);
    let r_hill_factor = (mu / 3.0).cbrt();
    let l1_dist = dist * (1.0 - r_hill_factor);
    let l2_dist = dist * (1.0 + r_hill_factor);
    let l3_dist = dist * (1.0 + 5.0 * mu / 12.0);

    let l1 = parent_pos + to_child * l1_dist;
    let l2 = parent_pos + to_child * l2_dist;
    let l3 = parent_pos - to_child * l3_dist;

    // L4 / L5 = 60° rotation in the child's orbit plane.
    // Approximate orbit-plane normal as: cross of to_child with reference up.
    // For most of our pairs the orbit normal is ~+Z (ecliptic-aligned), so
    // we can use to_child × Z as a starting tangent and reconstruct.
    let ecliptic_up = glam::DVec3::new(0.0, 0.0, 1.0);
    let normal = to_child.cross(ecliptic_up).cross(to_child).normalize();
    // Actually we want the orbit normal (perpendicular to orbit plane):
    // for an orbit roughly in the ecliptic, that IS +Z. But Earth-Moon's
    // orbit is tilted ~5° — the eclipsing math elsewhere handles this; for
    // a Lagrange overlay this approximation is fine (sub-arcminute error).
    let orbit_normal = to_child.cross(normal).normalize();

    // Rotate `separation` by +60° and -60° about orbit_normal.
    let angle_rad = 60.0_f64.to_radians();
    let rot = |v: glam::DVec3, axis: glam::DVec3, theta: f64| -> glam::DVec3 {
        // Rodrigues' rotation formula.
        let (s, c) = theta.sin_cos();
        v * c + axis.cross(v) * s + axis * (axis.dot(v)) * (1.0 - c)
    };
    let l4 = parent_pos + rot(separation, orbit_normal, angle_rad);
    let l5 = parent_pos + rot(separation, orbit_normal, -angle_rad);

    Some([l1, l2, l3, l4, l5])
}

// ─────────────────────── Sky events: conjunctions + eclipses (Phase 4d-tri, v0.210.0) ──

/// How tight a conjunction is. Tighter conjunctions get more visually
/// prominent treatment (brighter ring + status badge).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ConjunctionTightness {
    Close,       // < 5°  — visible-naked-eye proximity
    Conjunction, // < 1°  — newspaper-headline conjunction
    Tight,       // < 0.5° — telescope-sharing-eyepiece tight
    Occultation, // < 0.1° — one body passes in front of the other
}

impl ConjunctionTightness {
    fn label(self) -> &'static str {
        match self {
            ConjunctionTightness::Close => "close",
            ConjunctionTightness::Conjunction => "conjunction",
            ConjunctionTightness::Tight => "tight",
            ConjunctionTightness::Occultation => "occultation",
        }
    }
    /// Visual emphasis multiplier — drives ring brightness + thickness.
    fn weight(self) -> f32 {
        match self {
            ConjunctionTightness::Close => 0.3,
            ConjunctionTightness::Conjunction => 0.55,
            ConjunctionTightness::Tight => 0.8,
            ConjunctionTightness::Occultation => 1.0,
        }
    }
}

#[derive(Clone, Debug)]
struct ConjunctionEvent {
    body_a_id: String,
    body_a_name: String,
    body_b_id: String,
    body_b_name: String,
    angular_sep_deg: f64,
    tightness: ConjunctionTightness,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum EclipseKind {
    SolarPartial, // Moon partially covers Sun from Earth
    SolarAnnular, // Moon center on Sun, Moon angular size smaller (ring of fire)
    SolarTotal,   // Moon fully covers Sun
}

impl EclipseKind {
    fn label(self) -> &'static str {
        match self {
            EclipseKind::SolarPartial => "partial solar eclipse",
            EclipseKind::SolarAnnular => "annular solar eclipse",
            EclipseKind::SolarTotal => "total solar eclipse",
        }
    }
}

#[derive(Clone, Debug)]
struct EclipseEvent {
    kind: EclipseKind,
    /// Fraction of Sun's disk covered by Moon (0.0 = none, 1.0 = full).
    coverage: f64,
    /// Sun-Moon angular separation in degrees from Earth's POV.
    separation_deg: f64,
}

/// Compute current sky events (conjunctions between named bodies + any solar
/// eclipse) as seen from Earth at the given sim_time.
///
/// All angles are computed from Earth's barycentric position — close enough
/// to surface-observer accuracy at planetary scales (Earth's radius is ~4
/// arcsec at lunar distance; we report to 0.01°). For eclipses we DO care
/// about Earth's surface — but Phase 4d-tri ships a "is there an eclipse
/// anywhere on Earth?" detector, not "is the umbra over this lat/lon"
/// (which would require Earth's rotation phase + parallax; see Phase 4d-quad).
fn detect_sky_events(sim_time_seconds: f64) -> (Vec<ConjunctionEvent>, Option<EclipseEvent>) {
    let Some(earth) = find_body("earth") else {
        return (Vec::new(), None);
    };
    let earth_pos = body_world_position_3d_au(earth, sim_time_seconds);

    // Major bodies visible to the naked eye + dwarf planets. We exclude
    // Earth itself (can't be in conjunction with yourself). The Moon is
    // included because Sun-Moon conjunctions ARE eclipses, and Moon-
    // planet conjunctions are exciting.
    const INTERESTING: &[&str] = &[
        "sun", "mercury", "venus", "mars", "jupiter",
        "saturn", "uranus", "neptune", "moon", "pluto", "ceres",
    ];

    let positions: Vec<(String, String, glam::DVec3)> = INTERESTING
        .iter()
        .filter_map(|id| {
            find_body(id).map(|b| {
                (b.id.clone(), b.name.clone(), body_world_position_3d_au(b, sim_time_seconds))
            })
        })
        .collect();

    let mut events: Vec<ConjunctionEvent> = Vec::new();
    for i in 0..positions.len() {
        for j in (i + 1)..positions.len() {
            let to_a = (positions[i].2 - earth_pos).normalize();
            let to_b = (positions[j].2 - earth_pos).normalize();
            let cos_angle = to_a.dot(to_b).clamp(-1.0, 1.0);
            let angle_deg = cos_angle.acos().to_degrees();
            let tightness = if angle_deg < 0.1 {
                Some(ConjunctionTightness::Occultation)
            } else if angle_deg < 0.5 {
                Some(ConjunctionTightness::Tight)
            } else if angle_deg < 1.0 {
                Some(ConjunctionTightness::Conjunction)
            } else if angle_deg < 5.0 {
                Some(ConjunctionTightness::Close)
            } else {
                None
            };
            if let Some(t) = tightness {
                events.push(ConjunctionEvent {
                    body_a_id: positions[i].0.clone(),
                    body_a_name: positions[i].1.clone(),
                    body_b_id: positions[j].0.clone(),
                    body_b_name: positions[j].1.clone(),
                    angular_sep_deg: angle_deg,
                    tightness: t,
                });
            }
        }
    }
    // Sort tightest-first so the HUD shows the most exciting events.
    events.sort_by(|a, b| a.angular_sep_deg.partial_cmp(&b.angular_sep_deg)
        .unwrap_or(std::cmp::Ordering::Equal));

    // Eclipse: Sun-Moon angular separation must be less than the sum of
    // their apparent radii (so disks overlap from Earth's POV).
    let eclipse = (|| {
        let sun = find_body("sun")?;
        let moon = find_body("moon")?;
        let sun_pos = body_world_position_3d_au(sun, sim_time_seconds);
        let moon_pos = body_world_position_3d_au(moon, sim_time_seconds);
        let sun_dist_au = (sun_pos - earth_pos).length();
        let moon_dist_au = (moon_pos - earth_pos).length();
        if sun_dist_au <= 0.0 || moon_dist_au <= 0.0 { return None; }
        let to_sun = (sun_pos - earth_pos) / sun_dist_au;
        let to_moon = (moon_pos - earth_pos) / moon_dist_au;
        let sep_rad = to_sun.dot(to_moon).clamp(-1.0, 1.0).acos();

        let sun_radius_au = sun.radius_km / KM_PER_AU;
        let moon_radius_au = moon.radius_km / KM_PER_AU;
        let sun_apparent_rad = (sun_radius_au / sun_dist_au).atan();
        let moon_apparent_rad = (moon_radius_au / moon_dist_au).atan();

        if sep_rad >= sun_apparent_rad + moon_apparent_rad {
            return None; // No disk overlap → no eclipse.
        }

        // Moon disk entirely inside Sun disk?
        let kind = if sep_rad + moon_apparent_rad < sun_apparent_rad {
            // Moon fully overlapped by Sun (Moon center within Sun radius)
            if moon_apparent_rad >= sun_apparent_rad {
                EclipseKind::SolarTotal
            } else {
                EclipseKind::SolarAnnular
            }
        } else {
            EclipseKind::SolarPartial
        };

        // Coverage: rough fraction of Sun area obscured by Moon. Use a
        // linear approximation (real area overlap requires lens formulas).
        // Good enough for HUD readout.
        let max_sep = sun_apparent_rad + moon_apparent_rad;
        let coverage = ((max_sep - sep_rad) / max_sep).clamp(0.0, 1.0);

        Some(EclipseEvent {
            kind,
            coverage: coverage as f64,
            separation_deg: sep_rad.to_degrees() as f64,
        })
    })();

    (events, eclipse)
}

// ─────────────────────── Body info card (Phase 4d-bis, v0.209.0) ───────────

/// Render the expanded info card for a SolBody. v0.214.0 takes a
/// `PlacedPill` (from the widget's phase-1 layout) so the card can attach
/// FLUSH to the pill's bottom edge — no duplicate body dot in the card
/// heading, no gap. The pill is the card's visual header; the card is
/// just the pill's downward extension.
///
/// Translates the SolBody into a generic `BodyCardData` and delegates to
/// `widgets::body_pill::paint_card_extension`. The widget owns the
/// visual layout (anchor flush with pill, flip-up if clipping bottom,
/// canvas clamp); this function owns the domain translation (formatting
/// "5.2 AU from Sun" etc.) plus the Cosmos-specific actions.
fn draw_body_info_card_v2(
    ui: &mut egui::Ui,
    paint: &egui::Painter,
    theme: &Theme,
    body: &SolBody,
    pp: &crate::gui::widgets::body_pill::PlacedPill,
    canvas_rect: Rect,
    state: &mut GuiState,
) {
    use crate::gui::widgets::body_pill::{BodyCardData, paint_card_extension};

    let dist_str = if body.semi_major_axis_au > 0.0 {
        format!("{:.2} AU from Sun", body.semi_major_axis_au)
    } else if body.semi_major_axis_km > 0.0 {
        format!(
            "{} km from {}",
            format_with_commas(body.semi_major_axis_km as i64),
            body.parent.as_deref().map(titlecase).unwrap_or_default(),
        )
    } else {
        String::new()
    };
    let period_str = if body.orbital_period_days > 365.25 {
        format!("{:.2} y period", body.orbital_period_days / 365.25)
    } else if body.orbital_period_days > 1.0 {
        format!("{:.1} d period", body.orbital_period_days)
    } else if body.orbital_period_days > 0.0 {
        format!("{:.1} h period", body.orbital_period_days * 24.0)
    } else {
        String::new()
    };
    let mut subtitle_parts: Vec<String> = vec![titlecase(&body.body_type)];
    if !dist_str.is_empty() { subtitle_parts.push(dist_str); }
    if !period_str.is_empty() { subtitle_parts.push(period_str); }
    let subtitle = Some(subtitle_parts.join(" · "));

    let mut stats: Vec<(String, String)> = Vec::new();
    if body.radius_km > 0.0 {
        stats.push(("Radius".to_string(), format_km(body.radius_km)));
    }
    if body.mean_temperature_k > 0.0 {
        let temp_c = body.mean_temperature_k - 273.15;
        stats.push((
            "Mean temp".to_string(),
            format!("{:.0} °C ({:.0} K)", temp_c, body.mean_temperature_k),
        ));
    }
    if body.surface_gravity_ms2 > 0.0 {
        stats.push((
            "Surface gravity".to_string(),
            format!("{:.2} m/s²", body.surface_gravity_ms2),
        ));
    }

    let description = if body.description.is_empty() {
        None
    } else {
        Some(body.description.as_str())
    };

    let actions = vec![
        ("Focus".to_string(), true),
        ("Track".to_string(), false),
    ];

    let data = BodyCardData {
        subtitle,
        stats,
        description,
        actions,
    };

    let resp = paint_card_extension(ui, paint, theme, pp, &data, canvas_rect);
    if resp.closed {
        state.cosmos_expanded_body = None;
    }
    if let Some(idx) = resp.action_clicked {
        match idx {
            0 => {
                state.cosmos_focus_request = Some(body.id.clone());
                state.cosmos_selected_body = Some(body.id.clone());
            }
            1 => { /* Track stub */ }
            _ => {}
        }
    }
}

// ─────────────────────── Helpers ────────────────────────────────────────────

fn titlecase(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().chain(chars).collect(),
    }
}

fn format_with_commas(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 { out.push(','); }
        out.push(ch);
    }
    if n < 0 { out.push('-'); }
    out.chars().rev().collect()
}

/// Perpendicular distance from `p` to the line segment `a → b`.
fn dist_point_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_sq();
    if len_sq < 1e-6 { return (p - a).length(); }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    let closest = a + ab * t;
    (p - closest).length()
}

// ─────────────────────── Time controls (v0.208.0) ───────────────────────────

/// J2000.0 epoch in seconds since the UNIX epoch (1970-01-01 00:00 UTC).
/// J2000 = 2000-01-01 12:00:00 UTC = UNIX time 946,728,000.
const J2000_UNIX_SECONDS: f64 = 946_728_000.0;

/// Seconds since J2000.0 corresponding to the current real-world clock.
/// Used to initialize sim_time so the user sees today's planetary
/// configuration without scrubbing.
fn current_real_time_seconds_since_j2000() -> f64 {
    let unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    unix - J2000_UNIX_SECONDS
}

/// Format a sim_time (seconds since J2000) as "YYYY-MM-DD HH:MM UTC".
/// Uses Howard Hinnant's days-from-civil algorithm — works for any
/// proleptic-Gregorian year, no external date library required.
fn format_sim_time(sim_time_seconds: f64) -> String {
    let unix = sim_time_seconds + J2000_UNIX_SECONDS;
    let unix_i = unix.floor() as i64;
    let days = unix_i.div_euclid(86_400);
    let secs_in_day = unix_i.rem_euclid(86_400);
    let hour = (secs_in_day / 3600) as u32;
    let minute = ((secs_in_day % 3600) / 60) as u32;
    let (year, month, day) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02}  {:02}:{:02} UTC", year, month, day, hour, minute)
}

/// Convert "days since UNIX epoch" to (year, month, day) using
/// Howard Hinnant's algorithm — accurate for all proleptic-Gregorian
/// dates. https://howardhinnant.github.io/date_algorithms.html
fn days_to_ymd(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468; // shift so era 0 starts at year 0
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year as i32, m as u32, d as u32)
}

/// Format a sim_speed value (multiplier) into a human-readable label
/// like "1 day/sec", "1 year/sec", "paused", "REWIND 1 month/sec".
fn format_sim_speed(speed: f64) -> String {
    if speed == 0.0 { return "paused".to_string(); }
    let abs = speed.abs();
    let prefix = if speed < 0.0 { "REW " } else { "" };
    let unit = if abs >= 365.25 * 86_400.0 {
        let years = abs / (365.25 * 86_400.0);
        format!("{:.1} years/s", years)
    } else if abs >= 30.0 * 86_400.0 {
        let mo = abs / (30.0 * 86_400.0);
        format!("{:.1} mo/s", mo)
    } else if abs >= 86_400.0 {
        let d = abs / 86_400.0;
        format!("{:.1} d/s", d)
    } else if abs >= 3_600.0 {
        let h = abs / 3_600.0;
        format!("{:.1} h/s", h)
    } else if abs >= 60.0 {
        format!("{:.1} min/s", abs / 60.0)
    } else if abs >= 1.0 {
        format!("{:.0}× real-time", abs)
    } else {
        format!("{:.2}× real-time", abs)
    };
    format!("{}{}", prefix, unit)
}

/// Render the time controls bar at the top of the cosmos canvas.
/// Operator can pause / play / fast-forward / rewind / jump to "now".
fn draw_time_controls(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let frame = egui::Frame::none()
        .fill(egui::Color32::from_rgb(15, 15, 22))  // theme-exempt: cosmos UI bg
        .inner_margin(egui::Margin::symmetric(8, 4));
    frame.show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            // ── Date display ──
            ui.label(
                RichText::new(format_sim_time(state.cosmos_sim_time_seconds))
                    .size(theme.font_size_body)
                    .color(theme.text_primary())
                    .monospace()
                    .strong(),
            );
            ui.separator();

            // ── Now button — jumps sim_time to current real-world time ──
            if widgets::Button::secondary("Now")
                .tooltip("Set the date to right now (your system clock). Useful for \
                          seeing today's planetary configuration vs real-life observations.")
                .show(ui, theme)
            {
                state.cosmos_sim_time_seconds = current_real_time_seconds_since_j2000();
            }
            ui.separator();

            // ── Transport controls ──
            // « » jump by 1 month at a time; ⏪ ⏩ jump by 1 day. Operator
            // can combine with play/pause to scrub the orbits visually.
            if widgets::Button::secondary("«")
                .tooltip("Skip back 1 month")
                .show(ui, theme)
            {
                state.cosmos_sim_time_seconds -= 30.0 * 86_400.0;
            }
            if widgets::Button::secondary("⏪")
                .tooltip("Skip back 1 day")
                .show(ui, theme)
            {
                state.cosmos_sim_time_seconds -= 86_400.0;
            }
            // Play / Pause: toggle between sim_speed = 0 and the last used speed.
            let playing = state.cosmos_sim_speed != 0.0;
            let label = if playing { "Pause" } else { "Play" };
            if widgets::Button::primary(label)
                .tooltip(if playing { "Pause the simulation." }
                         else { "Resume / start the simulation at the current speed (default 1 day/sec)." })
                .show(ui, theme)
            {
                if playing {
                    state.cosmos_sim_speed = 0.0;
                } else {
                    // Default to 1 day per real second when starting from paused.
                    state.cosmos_sim_speed = 86_400.0;
                }
            }
            if widgets::Button::secondary("⏩")
                .tooltip("Skip forward 1 day")
                .show(ui, theme)
            {
                state.cosmos_sim_time_seconds += 86_400.0;
            }
            if widgets::Button::secondary("»")
                .tooltip("Skip forward 1 month")
                .show(ui, theme)
            {
                state.cosmos_sim_time_seconds += 30.0 * 86_400.0;
            }
            ui.separator();

            // ── Speed preset buttons ──
            ui.label(RichText::new("Speed:").color(theme.text_secondary()).size(theme.font_size_small));
            for &(label, speed) in &[
                ("1 h/s",   3_600.0_f64),
                ("1 d/s",   86_400.0),
                ("1 mo/s",  30.0 * 86_400.0),
                ("1 y/s",   365.25 * 86_400.0),
                ("10 y/s",  10.0 * 365.25 * 86_400.0),
            ] {
                let is_active = (state.cosmos_sim_speed - speed).abs() < 1e-3;
                let is_active_neg = (state.cosmos_sim_speed + speed).abs() < 1e-3;
                if widgets::Button::secondary(label)
                    .active(is_active)
                    .tooltip("Forward at this speed")
                    .show(ui, theme)
                {
                    state.cosmos_sim_speed = speed;
                }
                // Rewind variant — right-click NOT available so use a small "-" prefix button.
                let _ = is_active_neg;
            }
            // Rewind toggle: flip the sign of sim_speed.
            if widgets::Button::secondary("Reverse")
                .tooltip("Flip the simulation direction (forward ↔ backward).")
                .show(ui, theme)
            {
                state.cosmos_sim_speed = -state.cosmos_sim_speed;
            }

            // ── Current speed label ──
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("· {}", format_sim_speed(state.cosmos_sim_speed)))
                        .size(theme.font_size_small)
                        .color(theme.text_muted())
                        .italics(),
                );
            });
        });

        // ── Scrubber — slide to jump days/months/years quickly ──
        // Range is ±10 years from today's date, scaled in days.
        let now = current_real_time_seconds_since_j2000();
        let mut offset_days = (state.cosmos_sim_time_seconds - now) / 86_400.0;
        let scrubber = egui::Slider::new(&mut offset_days, -(365.25 * 10.0)..=(365.25 * 10.0))
            .text("Days from today")
            .show_value(true);
        if ui.add(scrubber).changed() {
            state.cosmos_sim_time_seconds = now + offset_days * 86_400.0;
        }
    });
}
