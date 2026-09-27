//! Tests for environment Layer 1 (`env_layer1.rs`). Each says how it was seen
//! RED before it was trusted green: a check that has never failed is a check
//! whose evidence may be its own setup (memory: feedback_checks_that_cannot_fail).

use super::*;
use crate::renderer::shader_loader::wgsl_eval::{Evaluator, Value};

fn shipped() -> ClimateTable {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/data/environment/climate.ron"))
        .expect("the climate table ships with the repo");
    ClimateTable::from_ron(&src).expect("the shipped climate table must parse and validate")
}

fn earth() -> ClimateRow {
    shipped().row("earth").expect("Earth has a climate row").clone()
}

/// Unit direction at a latitude and longitude, degrees, with the house
/// handedness (terrain::planet_heightmap::latlon_to_dir), in f64.
fn dir_at(lat: f64, lon: f64) -> DVec3 {
    let (la, lo) = (lat.to_radians(), lon.to_radians());
    DVec3::new(la.cos() * lo.cos(), la.sin(), -la.cos() * lo.sin())
}

/// The game year's fraction at which a hemisphere's summer peaks for a
/// coefficient pair: the maximum of x (A cos + B sin) for x > 0.
fn peak_year_fraction(row: &ClimateRow, pair: (f64, f64)) -> f64 {
    let ang = pair.1.atan2(pair.0);
    let tau = (ang / std::f64::consts::TAU).rem_euclid(1.0);
    (row.north_winter_solstice_year_fraction + tau).rem_euclid(1.0)
}

/// The shipped table parses, each world is one row, and the numbers mean what
/// the data file says. Named for the `from_ron` filter `just validate-data`
/// runs. Its red arm runs every time: the test edits a copy of the file to
/// carry a 17th wind term and asserts the loader refuses it, and checks the
/// edit actually landed so that refusal cannot pass vacuously.
#[test]
fn climate_table_parses_from_ron_and_holds_its_invariants() {
    let t = shipped();
    let e = t.row("earth").expect("earth row");
    let m = t.row("mars").expect("mars row");
    assert!(t.row("moon").is_none(), "the Moon has no air and no row on purpose");
    assert_eq!(e.wind_u_mean.len(), WIND_TERMS);

    // Mars: the column reproduces the fact sheet's own scale height, R T / (g M)
    // = 11.0 km at its 214 K average temperature. A units slip in the molar
    // mass (43.49 instead of 0.04349) or gravity would miss by orders.
    let h = GAS_CONSTANT * 214.0 / (m.gravity_ms2 * m.molar_mass_kg_per_mol);
    assert!((h - 11_000.0).abs() < 200.0, "Mars scale height {h} m, fact sheet 11.0 km");
    let (_, p0) = m.column(214.0, 0.0);
    assert!((p0 - 0.636).abs() < 1e-9, "Mars datum pressure is the fact sheet's 6.36 mb");

    // A longer wind series than the shader unrolls must be refused, not
    // silently truncated on the GPU side only.
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/data/environment/climate.ron")).unwrap();
    let broken = src.replacen("wind_u_mean: [-0.358,", "wind_u_mean: [9.0, -0.358,", 1);
    assert!(broken != src, "the test's edit must land");
    assert!(ClimateTable::from_ron(&broken).is_err(), "17 wind terms must be refused");
}

/// The 1976 US Standard Atmosphere, with a 15 C sea level, at 0, 1,500 and
/// 5,000 m GEOMETRIC altitude. Expected values are the standard's tabulated
/// ones: 288.150 K and 101.325 kPa; 278.402 K and 84.560 kPa; 255.676 K and
/// 54.048 kPa. Seen red by removing the geopotential conversion (h used as H):
/// 5,000 m then read 255.650 K, outside the 0.01 K tolerance (its pressure,
/// 54.020 kPa, is 0.05 percent low, outside 2e-4 as well), which is why the
/// tolerances are this tight.
#[test]
fn the_standard_atmosphere_at_0_1500_and_5000_m() {
    let e = earth();
    for (alt, t_k, p_kpa) in [(0.0, 288.150, 101.325), (1_500.0, 278.402, 84.560), (5_000.0, 255.676, 54.048)] {
        let (t, p) = e.column(288.15, alt);
        assert!((t - t_k).abs() < 0.01, "{alt} m: {t} K, standard {t_k} K");
        assert!(((p - p_kpa) / p_kpa).abs() < 2e-4, "{alt} m: {p} kPa, standard {p_kpa} kPa");
    }
    // Above the tropopause the standard's second layer is isothermal.
    let (t11, _) = e.column(288.15, 11_100.0);
    let (t15, p15) = e.column(288.15, 15_000.0);
    assert!((t11 - t15).abs() < 1e-9, "isothermal from 11 km");
    assert!((p15 - 12.11).abs() < 0.02, "15 km: {p15} kPa, standard 12.11");
}

/// The equator is warmer than the poles, and the seasons FLIP between the
/// hemispheres: when the north is in summer the south is in winter. Seen red
/// by taking |sin latitude| in the seasonal term (both hemispheres warmed and
/// cooled together): "45 S must be warmer in the northern winter" failed.
#[test]
fn the_equator_is_warmer_and_the_seasons_flip_between_hemispheres() {
    let e = earth();
    for yf in [0.0, 0.25, 0.5, 0.75] {
        for land in [0.0, 1.0] {
            let eq = e.sea_level_temp_c(0.0, land, yf);
            for pole in [1.0, -1.0] {
                let p = e.sea_level_temp_c(pole, land, yf);
                assert!(eq > p + 15.0, "year {yf} land {land}: equator {eq} vs pole {p}");
            }
        }
    }
    // Northern summer peak on land: mid July in the real year, which in the
    // game calendar lands inside Summer (days 30 to 60).
    let peak = peak_year_fraction(&e, e.season_north_land);
    assert!((0.25..0.5).contains(&peak), "northern land peaks at year fraction {peak}");
    let winter = (peak + 0.5).rem_euclid(1.0);
    for land in [0.0, 0.5, 1.0] {
        let n_summer = e.sea_level_temp_c(45f64.to_radians().sin(), land, peak);
        let n_winter = e.sea_level_temp_c(45f64.to_radians().sin(), land, winter);
        let s_then = e.sea_level_temp_c(-(45f64.to_radians().sin()), land, peak);
        let s_later = e.sea_level_temp_c(-(45f64.to_radians().sin()), land, winter);
        assert!(n_summer > n_winter + 3.0, "land {land}: 45 N summer {n_summer} vs winter {n_winter}");
        assert!(s_later > s_then, "land {land}: 45 S must be warmer in the northern winter");
    }
    // Land swings more than sea: continentality, fitted from the reanalysis.
    let x = 60f64.to_radians().sin();
    let land_range = e.sea_level_temp_c(x, 1.0, peak) - e.sea_level_temp_c(x, 1.0, winter);
    let sea_range = e.sea_level_temp_c(x, 0.0, peak) - e.sea_level_temp_c(x, 0.0, winter);
    assert!(land_range > 2.0 * sea_range, "60 N: land range {land_range} vs sea {sea_range}");
}

/// The circulation bands: trades from the east with an equatorward drift in
/// both tropics, westerlies with a poleward drift in both mid-latitudes, the
/// Southern Ocean's the strongest, and easterlies off Antarctica. Values from
/// the 1991-2020 reanalysis the series was fitted to (the data file's
/// comment), within the fit's own error. Seen red by building the basis from
/// latitude instead of colatitude (sin(n phi) for sin(n theta)): the 16 N
/// trades came out calm, u +0.03 m/s.
#[test]
fn the_circulation_bands_blow_the_right_way_by_latitude() {
    let e = earth();
    // Annual mean: average the first harmonic out over a year.
    let annual = |lat: f64| {
        let d = dir_at(lat, 30.0);
        let n = 24;
        let (mut u, mut v) = (0.0, 0.0);
        for i in 0..n {
            let (a, b) = e.wind_east_north(d, i as f64 / n as f64);
            u += a / n as f64;
            v += b / n as f64;
        }
        (u, v)
    };
    let (u, v) = annual(16.0);
    assert!(u < -2.5 && v < -0.8, "16 N trades from the east-northeast: u {u} v {v}");
    let (u, v) = annual(-16.0);
    assert!(u < -3.0 && v > 1.2, "16 S trades from the east-southeast: u {u} v {v}");
    let (u, v) = annual(47.0);
    assert!(u > 1.0 && v > 0.0, "47 N westerlies drifting poleward: u {u} v {v}");
    let (u, v) = annual(-52.0);
    assert!(u > 6.0 && v < -1.0, "52 S westerlies, the strongest: u {u} v {v}");
    let (u, _) = annual(-71.0);
    assert!(u < -1.0, "71 S easterlies off Antarctica: u {u}");
    // The band edges: calm-ish zonal wind at the horse latitudes.
    for lat in [31.0, -32.0] {
        assert!(annual(lat).0.abs() < 0.6, "{lat}: near the trade/westerly boundary");
    }
    // Poles: every term of the series vanishes.
    for pole in [DVec3::Y, -DVec3::Y] {
        let (u, v) = e.wind_east_north(pole, 0.3);
        assert!(u.abs() < 1e-12 && v.abs() < 1e-12, "no zonal wind AT a pole");
        assert_eq!(wind_body(&e, pole, 0.3), DVec3::ZERO);
    }
    // The body-frame vector is tangent and carries the same speed.
    let d = dir_at(16.0, 70.0);
    let w = wind_body(&e, d, 0.4);
    let (u, v) = e.wind_east_north(d, 0.4);
    assert!(w.dot(d).abs() < 1e-9, "tangent to the surface");
    assert!((w.length() - u.hypot(v)).abs() < 1e-9);
    // East really is increasing longitude.
    let (east, north) = east_north(d).unwrap();
    let step = dir_at(16.0, 70.001) - d;
    assert!(east.dot(step) > 0.0 && north.dot(dir_at(16.001, 70.0) - d) > 0.0);
}

/// The land share walks across a coast in ninths instead of jumping, and
/// reads 0 and 1 away from it. A synthetic coast: ocean west of 0 degrees.
#[test]
fn the_land_share_crosses_a_coast_smoothly() {
    let ocean_west = |_lat: f64, lon: f64| lon < 0.0;
    assert_eq!(land_fraction_around(40.0, -5.0, ocean_west), 0.0);
    assert_eq!(land_fraction_around(40.0, 5.0, ocean_west), 1.0);
    let shore = land_fraction_around(40.0, 0.0, ocean_west);
    assert!(shore > 0.3 && shore < 0.9, "on the coast: {shore}");
    // Near a pole the ring stays finite and the answer stays a share.
    let polar = land_fraction_around(89.9, 10.0, ocean_west);
    assert!((0.0..=1.0).contains(&polar));
}

/// The Chebyshev recurrence the twins share IS sin(n theta).
#[test]
fn the_sine_basis_is_sin_n_colatitude() {
    for lat in [-89.0f64, -60.0, -12.5, 0.0, 7.0, 45.0, 88.0] {
        let theta = (90.0 - lat).to_radians();
        let s = sine_series_basis(lat.to_radians().sin());
        for (i, v) in s.iter().enumerate() {
            let want = ((i + 1) as f64 * theta).sin();
            assert!((v - want).abs() < 1e-9, "lat {lat} n {}: {v} vs {want}", i + 1);
        }
    }
}

/// The CPU and WGSL twins agree over a grid of places, altitudes, land shares
/// and dates, for every world with a row. The WGSL side is the SHIPPED
/// megashader, parsed by naga and run by `wgsl_eval`; its `EnvClimate` is built
/// from `pack_gpu` using the shader's own member offsets, so this is also the
/// layout check. Seen red three ways, each alone: swapping the cos and sin
/// coefficients in the WGSL seasonal term (88 S at year 0 read -4.81 C on the
/// GPU against -4.37 on the CPU); swapping the northern sea and land lanes in
/// `pack_gpu` (5 N over sea, 30.36 against 30.51); and removing the
/// geopotential conversion from the CPU column only.
#[test]
fn wgsl_twin_matches_the_cpu_model() {
    let module = Evaluator::parse(crate::renderer::shader_loader::assembled_pbr_source()).expect("megashader parses");
    let ev = Evaluator::new(&module);
    let ty = ev.type_named("EnvClimate").expect("EnvClimate declared in the shader");
    assert_eq!(
        ev.struct_span(ty),
        Some((GPU_FLOATS * 4) as u32),
        "the WGSL struct and ClimateRow::pack_gpu disagree about the record's size"
    );
    let table = shipped();
    let mut checked = 0;
    for row in &table.worlds {
        let climate = ev.value_from_floats(ty, &row.pack_gpu()).expect("pack fits the struct");
        let mut i = 0usize;
        for lat in [-88.0f64, -71.0, -52.0, -33.0, -16.0, -3.0, 0.0, 5.0, 16.0, 31.0, 47.0, 66.0, 87.0] {
            // Longitude varies along the grid: the fields are zonal, the
            // directions and the east/north frame are not.
            let lon = -170.0 + 47.0 * i as f64;
            i += 1;
            let d = dir_at(lat, lon);
            let df = Value::vec(&[d.x as f32, d.y as f32, d.z as f32]);
            for yf in [0.0f64, 0.13, 0.41, 0.77] {
                for alt in [-400.0f64, 0.0, 1_500.0, 5_000.0, 9_000.0, 18_000.0] {
                    for land in [0.0f64, 0.4, 1.0] {
                        let cpu = row.air_at(d, alt, land, yf);
                        let gpu = ev
                            .call(
                                "env_l1_air",
                                vec![
                                    climate.clone(),
                                    df.clone(),
                                    Value::F32(alt as f32),
                                    Value::F32(land as f32),
                                    Value::F32(yf as f32),
                                ],
                            )
                            .expect("env_l1_air runs")
                            .floats()
                            .unwrap();
                        let ctx = format!("{} lat {lat} alt {alt} land {land} year {yf}", row.body);
                        assert!((gpu[0] as f64 - cpu.temp_c).abs() < 0.01, "{ctx}: T gpu {} cpu {}", gpu[0], cpu.temp_c);
                        let rel = (gpu[1] as f64 - cpu.pressure_kpa).abs() / cpu.pressure_kpa.max(1e-9);
                        assert!(rel < 1e-4, "{ctx}: p gpu {} cpu {}", gpu[1], cpu.pressure_kpa);
                        checked += 1;
                    }
                }
                let wind = ev
                    .call("env_l1_wind_en", vec![climate.clone(), df.clone(), Value::F32(yf as f32)])
                    .expect("env_l1_wind_en runs")
                    .floats()
                    .unwrap();
                let (u, v) = row.wind_east_north(d, yf);
                assert!(
                    (wind[0] as f64 - u).abs() < 1e-3 && (wind[1] as f64 - v).abs() < 1e-3,
                    "{} lat {lat} year {yf}: wind gpu {wind:?} cpu ({u}, {v})",
                    row.body
                );
                let body = ev
                    .call("env_l1_wind_body", vec![climate.clone(), df.clone(), Value::F32(yf as f32)])
                    .expect("env_l1_wind_body runs")
                    .floats()
                    .unwrap();
                let want = wind_body(row, d, yf);
                for k in 0..3 {
                    assert!((body[k] as f64 - want[k]).abs() < 1e-3, "{} lat {lat}: body wind {body:?} vs {want}", row.body);
                }
            }
        }
    }
    assert!(checked > 1000, "the grid must actually run: {checked} comparisons");
}
