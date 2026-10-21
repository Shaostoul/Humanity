//! Sky events for the Maps page: the conjunctions and solar eclipses in the
//! sky from Earth right now, the forward scan for the next ones (cached in
//! `GuiState::maps`, never run every frame), and the point on Earth an
//! eclipse is overhead.
//! Moved out of `gui/pages/cosmos.rs` in Maps increment 0 (2026-10-20).

use crate::cosmos::{body_world_position_3d_au, find_body, KM_PER_AU};

// ─────────────────────── Sky events: conjunctions + eclipses (Phase 4d-tri, v0.210.0) ──

/// How tight a conjunction is. Tighter conjunctions get more visually
/// prominent treatment (brighter ring + status badge).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum ConjunctionTightness {
    Close,       // < 5°  — visible-naked-eye proximity
    Conjunction, // < 1°  — newspaper-headline conjunction
    Tight,       // < 0.5° — telescope-sharing-eyepiece tight
    Occultation, // < 0.1° — one body passes in front of the other
}

impl ConjunctionTightness {
    pub(super) fn label(self) -> &'static str {
        match self {
            ConjunctionTightness::Close => "close",
            ConjunctionTightness::Conjunction => "conjunction",
            ConjunctionTightness::Tight => "tight",
            ConjunctionTightness::Occultation => "occultation",
        }
    }
    /// Visual emphasis multiplier — drives ring brightness + thickness.
    pub(super) fn weight(self) -> f32 {
        match self {
            ConjunctionTightness::Close => 0.3,
            ConjunctionTightness::Conjunction => 0.55,
            ConjunctionTightness::Tight => 0.8,
            ConjunctionTightness::Occultation => 1.0,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct ConjunctionEvent {
    pub(super) body_a_id: String,
    pub(super) body_a_name: String,
    pub(super) body_b_id: String,
    pub(super) body_b_name: String,
    pub(super) angular_sep_deg: f64,
    pub(super) tightness: ConjunctionTightness,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum EclipseKind {
    SolarPartial, // Moon partially covers Sun from Earth
    SolarAnnular, // Moon center on Sun, Moon angular size smaller (ring of fire)
    SolarTotal,   // Moon fully covers Sun
}

impl EclipseKind {
    pub(super) fn label(self) -> &'static str {
        match self {
            EclipseKind::SolarPartial => "partial solar eclipse",
            EclipseKind::SolarAnnular => "annular solar eclipse",
            EclipseKind::SolarTotal => "total solar eclipse",
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct EclipseEvent {
    pub(super) kind: EclipseKind,
    /// Fraction of Sun's disk covered where on Earth it is deepest (0.0 to 1.0).
    pub(super) coverage: f64,
    /// Sun-Moon angular separation in degrees from Earth's centre.
    pub(super) separation_deg: f64,
}

/// Compute current sky events (conjunctions between named bodies + any solar
/// eclipse) as seen from Earth at the given sim_time.
///
/// All angles are computed from Earth's centre, which is close enough for the
/// planets (Earth's radius is a few arcseconds at their distances) but not for
/// the Moon, which Earth's radius shifts by up to 0.95 degree. So the eclipse
/// check takes that parallax into account and answers "is there an eclipse
/// anywhere on Earth?" (2026-10-21), not "is the umbra over this lat/lon"
/// (which would require Earth's rotation phase; see Phase 4d-quad).
pub(super) fn detect_sky_events(sim_time_seconds: f64) -> (Vec<ConjunctionEvent>, Option<EclipseEvent>) {
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

    // Eclipse: the discs overlap as seen from SOMEWHERE on Earth. From the centre
    // they touch at the sum of their radii; the ground sees the Moon up to the
    // difference of the two parallaxes (~0.95 degree) away from there. Until
    // 2026-10-21 only the centre counted, which missed NASA's total eclipse of
    // 2026-08-12 (gamma 0.90) and annular of 2026-02-17 and called central ones
    // partial (`tests::the_maps_page_finds_nasas_solar_eclipses_of_2026_and_2027`).
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
        let earth_radius_au = earth.radius_km / KM_PER_AU;
        let sun_apparent_rad = (sun_radius_au / sun_dist_au).atan();
        let moon_apparent_rad = (moon_radius_au / moon_dist_au).atan();
        // How far the Moon can stand from where Earth's centre sees it.
        let reach = (earth_radius_au / moon_dist_au).asin() - (earth_radius_au / sun_dist_au).asin();
        if sep_rad >= sun_apparent_rad + moon_apparent_rad + reach { return None; }

        // gamma (the catalogs' measure, near enough): the shadow axis's distance
        // from Earth's centre in Earth radii. Under 1 it meets the ground, where
        // the Moon is nearer by sqrt(1 - gamma^2) radii: total if it covers the Sun.
        let gamma = sep_rad / reach;
        let nearest_moon_au = moon_dist_au - earth_radius_au * (1.0 - gamma.min(1.0).powi(2)).sqrt();
        let moon_near_rad = (moon_radius_au / nearest_moon_au).atan();
        let ratio_sq = (moon_near_rad / sun_apparent_rad).powi(2).min(1.0);
        let kind = match (gamma < 1.0, moon_near_rad >= sun_apparent_rad) {
            (true, true) => EclipseKind::SolarTotal,
            (true, false) => EclipseKind::SolarAnnular,
            (false, _) => EclipseKind::SolarPartial,
        };

        // Coverage where the eclipse is deepest: linear in the separation seen
        // there (real overlap needs lens formulas), capped by the Moon's disc.
        let max_sep = sun_apparent_rad + moon_near_rad;
        let deepest = ((max_sep - (sep_rad - reach).max(0.0)) / max_sep).clamp(0.0, 1.0);
        let coverage = match kind {
            EclipseKind::SolarTotal => 1.0,
            EclipseKind::SolarAnnular => ratio_sq,
            EclipseKind::SolarPartial => deepest * ratio_sq,
        };

        Some(EclipseEvent {
            kind,
            coverage: coverage as f64,
            separation_deg: sep_rad.to_degrees() as f64,
        })
    })();

    (events, eclipse)
}

// ─────────── Forward Sky-Events scan (Phase 4d-quad, v0.248.0) ─────────────

/// One predicted upcoming sky event. `when_sim_seconds` is the peak
/// time (seconds since J2000). `severity`: 0 = info, 1 = notable,
/// 2 = major (eclipse / occultation) — drives HUD color.
#[derive(Clone, Debug)]
pub struct UpcomingSkyEvent {
    pub when_sim_seconds: f64,
    pub label: String,
    pub severity: u8,
}

/// Refine an eclipse peak: sample finely around `coarse_t` (± one
/// coarse step) and return the time, coverage and kind of greatest eclipse:
/// the least separation from Earth's centre (NASA's definition), not the most
/// coverage, which stays 100% for the hours a total eclipse is central.
fn refine_eclipse_peak(coarse_t: f64, coarse_step: f64) -> (f64, f64, &'static str) {
    let mut best_t = coarse_t;
    let mut best_sep = f64::MAX;
    let mut best_cov = 0.0_f64;
    let mut best_kind = "solar eclipse";
    let fine = (coarse_step / 30.0).max(60.0); // ≥1-min resolution
    let mut t = coarse_t - coarse_step;
    let end = coarse_t + coarse_step;
    while t <= end {
        if let (_, Some(e)) = detect_sky_events(t) {
            if e.separation_deg < best_sep {
                best_sep = e.separation_deg;
                best_cov = e.coverage;
                best_t = t;
                best_kind = e.kind.label();
            }
        }
        t += fine;
    }
    (best_t, best_cov, best_kind)
}

/// Scan forward from `start_sim` for the next ~400 days and return the
/// soonest handful of notable events: solar eclipses (any), and tight
/// (<1°) conjunctions / occultations between the interesting bodies.
///
/// Cost: ~400 days / 3 h ≈ 3200 coarse samples, each an O(bodies²)
/// `detect_sky_events`. That's milliseconds — but FAR too much to run
/// every frame, so callers cache the result (see GuiState
/// `maps.upcoming_*`) and only re-scan when sim_time drifts.
pub(super) fn scan_upcoming_sky_events(start_sim: f64) -> Vec<UpcomingSkyEvent> {
    const HORIZON_DAYS: f64 = 400.0;
    const STEP: f64 = 3.0 * 3600.0; // 3-hour coarse step
    let end = start_sim + HORIZON_DAYS * 86_400.0;

    let mut out: Vec<UpcomingSkyEvent> = Vec::new();

    // ── Eclipses: enter/exit window tracking (windows never overlap) ──
    {
        let mut in_window = false;
        let mut peak_t = 0.0_f64;
        // Least Sun-Moon separation so far: greatest eclipse (see refine_eclipse_peak).
        let mut peak_sep = f64::MAX;
        let mut t = start_sim;
        while t <= end {
            let (_, ecl) = detect_sky_events(t);
            match (ecl, in_window) {
                (Some(e), false) => {
                    in_window = true;
                    peak_sep = e.separation_deg;
                    peak_t = t;
                }
                (Some(e), true) => {
                    if e.separation_deg < peak_sep {
                        peak_sep = e.separation_deg;
                        peak_t = t;
                    }
                }
                (None, true) => {
                    // Window closed — refine + record.
                    let (rt, rc, rk) = refine_eclipse_peak(peak_t, STEP);
                    out.push(UpcomingSkyEvent {
                        when_sim_seconds: rt,
                        label: format!("{} ({:.0}% max coverage)", rk, rc * 100.0),
                        severity: 2,
                    });
                    in_window = false;
                    peak_sep = f64::MAX;
                    if out.len() >= 16 {
                        break;
                    }
                }
                (None, false) => {}
            }
            t += STEP;
        }
        if in_window {
            let (rt, rc, rk) = refine_eclipse_peak(peak_t, STEP);
            out.push(UpcomingSkyEvent {
                when_sim_seconds: rt,
                label: format!("{} ({:.0}% max coverage)", rk, rc * 100.0),
                severity: 2,
            });
        }
    }

    // ── Tight conjunctions: per-pair local-minimum-separation detector ──
    // Track the last two separation samples per pair; a local min below
    // 1° that is also a true turning point is a distinct close approach.
    {
        use std::collections::HashMap;
        // (sep_2-ago, sep_1-ago, t_1-ago) keyed by "idA|idB".
        let mut hist: HashMap<String, (f64, f64, f64)> = HashMap::new();
        let mut t = start_sim;
        while t <= end {
            let (conj, ecl) = detect_sky_events(t);
            for ev in &conj {
                // The Sun–Moon pair is the eclipse; don't double-report.
                let is_sun_moon = (ev.body_a_id == "sun" || ev.body_b_id == "sun")
                    && (ev.body_a_id == "moon" || ev.body_b_id == "moon");
                if is_sun_moon && ecl.is_some() {
                    continue;
                }
                // Only care about approaches that get genuinely tight.
                if ev.angular_sep_deg > 2.0 {
                    continue;
                }
                let key = if ev.body_a_id < ev.body_b_id {
                    format!("{}|{}", ev.body_a_id, ev.body_b_id)
                } else {
                    format!("{}|{}", ev.body_b_id, ev.body_a_id)
                };
                let cur = ev.angular_sep_deg;
                let prev = hist.get(&key).copied();
                if let Some((s2, s1, t1)) = prev {
                    // Local minimum at the MIDDLE sample (t1, s1)? It must
                    // be a true turning point (≤ both neighbours) and tight.
                    if s1 <= s2 && s1 <= cur && s1 < 1.0 {
                        // Refine around t1 (±STEP) at fine resolution.
                        let fine = STEP / 20.0;
                        let mut bt = t1;
                        let mut bs = s1;
                        let mut k = t1 - STEP;
                        let lim = t1 + STEP;
                        while k <= lim {
                            for e2 in detect_sky_events(k).0 {
                                let kk = if e2.body_a_id < e2.body_b_id {
                                    format!("{}|{}", e2.body_a_id, e2.body_b_id)
                                } else {
                                    format!("{}|{}", e2.body_b_id, e2.body_a_id)
                                };
                                if kk == key && e2.angular_sep_deg < bs {
                                    bs = e2.angular_sep_deg;
                                    bt = k;
                                }
                            }
                            k += fine;
                        }
                        let (tight_label, sev) = if bs < 0.1 {
                            ("occultation", 2u8)
                        } else if bs < 0.5 {
                            ("tight", 1u8)
                        } else {
                            ("conjunction", 1u8)
                        };
                        out.push(UpcomingSkyEvent {
                            when_sim_seconds: bt,
                            label: format!(
                                "{} + {} {:.2}° ({})",
                                ev.body_a_name, ev.body_b_name, bs, tight_label
                            ),
                            severity: sev,
                        });
                    }
                }
                // Slide the 2-sample window: (old_s1, cur, t). First
                // sighting seeds both slots with `cur`.
                let new_s2 = prev.map(|(_, s1, _)| s1).unwrap_or(cur);
                hist.insert(key, (new_s2, cur, t));
            }
            t += STEP;
        }
    }

    out.sort_by(|a, b| {
        a.when_sim_seconds
            .partial_cmp(&b.when_sim_seconds)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out.truncate(6);
    out
}

/// Earth's mean obliquity (IAU 2006): 84381.406" at J2000, -46.836769" per
/// Julian century, in degrees. Physical constants, not infinite-of-X data.
const OBLIQUITY_J2000_DEG: f64 = 23.439_279_444;
const OBLIQUITY_PER_CENTURY_DEG: f64 = -0.013_010_214;
/// General precession in longitude (IAU 2006), arcseconds per century and
/// per century squared: how far the equinox of date has slid from J2000's.
const PRECESSION_ARCSEC: (f64, f64) = (5028.796_195, 1.105_434_8);

/// Geographic latitude & longitude (degrees, +N / +E) of the point on
/// Earth's surface directly beneath `target_pos` — i.e. where `target`
/// is at the observer's zenith. For a solar eclipse, feeding the Moon's
/// position gives the sub-lunar point, which is (to ~1°) where the
/// shadow axis pierces Earth and the eclipse is overhead.
///
/// An educational readout, not a Besselian-element ephemeris: it ignores
/// lunar parallax, Earth's oblateness, refraction, nutation, aberration and
/// UT1-UTC. Against JPL Horizons it is within 0.025 degree for the Sun and
/// Mars (`sky_events_tests.rs`); the Moon's own place is the larger error.
///
/// Frame (BUG-140, 2026-10-21): the world vector goes back into the J2000
/// ecliptic through `cosmos::ecliptic_from_world`, the inverse of the orbit
/// model's own mapping, so the two cannot disagree again. (This used to
/// restate the mapping as "+Z = ecliptic longitude 90" and build the RA axis
/// as P x X in that mirrored frame, which turned right ascension backwards.)
/// The longitude is then precessed to the equinox of date, because sidereal
/// time is measured from that equinox (0.37 degree by 2026), and turned to
/// right ascension and declination with the obliquity of date.
pub(super) fn subpoint_lat_lon_deg(
    target_pos: glam::DVec3,
    earth_pos: glam::DVec3,
    sim_time_seconds: f64,
) -> (f64, f64) {
    let d = target_pos - earth_pos;
    let len = d.length();
    if len <= 0.0 {
        return (0.0, 0.0);
    }
    let e = crate::cosmos::ecliptic_from_world(d / len); // Earth -> target
    let t_cy = sim_time_seconds / 86_400.0 / 36_525.0;
    let precession = (PRECESSION_ARCSEC.0 * t_cy + PRECESSION_ARCSEC.1 * t_cy * t_cy) / 3600.0;
    let lambda = e.y.atan2(e.x) + precession.to_radians();
    let beta = e.z.clamp(-1.0, 1.0).asin();
    let (se, ce) = (OBLIQUITY_J2000_DEG + OBLIQUITY_PER_CENTURY_DEG * t_cy).to_radians().sin_cos();
    // Ecliptic to equatorial of date: a turn about the equinox by epsilon.
    let (x, y, z) = (beta.cos() * lambda.cos(), beta.cos() * lambda.sin(), beta.sin());
    let lat_deg = (y * se + z * ce).clamp(-1.0, 1.0).asin().to_degrees();
    let ra_deg = (y * ce - z * se).atan2(x).to_degrees().rem_euclid(360.0);

    // Greenwich Mean Sidereal Time (IAU 1982), degrees. D = days since
    // J2000.0 (= sim seconds / 86400 — the model's exact time base).
    let d_days = sim_time_seconds / 86_400.0;
    let gmst_deg = (280.460_618_37 + 360.985_647_366_29 * d_days).rem_euclid(360.0);

    // East longitude = RA − GMST, wrapped to (−180, 180].
    let mut lon = (ra_deg - gmst_deg).rem_euclid(360.0);
    if lon > 180.0 {
        lon -= 360.0;
    }
    (lat_deg, lon)
}

/// Format a (lat, lon) pair as e.g. "12.3°N 45.6°W".
pub(super) fn format_geo(lat_deg: f64, lon_deg: f64) -> String {
    let (ns, lat) = if lat_deg >= 0.0 { ('N', lat_deg) } else { ('S', -lat_deg) };
    let (ew, lon) = if lon_deg >= 0.0 { ('E', lon_deg) } else { ('W', -lon_deg) };
    format!("{lat:.1}°{ns} {lon:.1}°{ew}")
}

#[cfg(test)]
#[path = "sky_events_tests.rs"]
mod tests;
