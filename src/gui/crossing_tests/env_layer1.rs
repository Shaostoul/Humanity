//! From systems/env_layer1_tests.rs: the shader's twin of the environment model matches the CPU
//! model, through the renderer's shader loader.

use crate::renderer::shader_loader::wgsl_eval::Evaluator;
use crate::renderer::shader_loader::wgsl_eval::Value;
use humanity_systems::env_layer1::GPU_FLOATS;
use humanity_systems::env_layer1::fixtures::dir_at;
use humanity_systems::env_layer1::fixtures::shipped;
use humanity_systems::env_layer1::wind_body;

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
