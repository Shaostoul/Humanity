//! The pulley model's tests (docs/design/pulleys.md, rung 1). Each gate was seen RED first
//! against a wrong model before the right one went in; the evidence is in each test's comment.

use super::data::{material_from_csv, PulleyData, PULLEYS_RON};
use super::geometry::{BlockDims, ShapeStore, StageShape};
use super::tackle::{evaluate, recommend, stage_real_ma, Inputs, Mode, Refusal, Verdict};
use super::Material;

fn shipped() -> PulleyData {
    PulleyData::parse(PULLEYS_RON).expect("the shipped data/pulleys/pulleys.ron parses")
}

fn steel() -> Material {
    material_from_csv(humanity_core::embedded_data::MATERIALS_CSV, "steel").expect("materials.csv has steel")
}

fn inputs<'a>(d: &'a PulleyData, load_kg: f64, puller: &str, rope: &str, bearing: &str, mode: Mode) -> Inputs<'a> {
    Inputs {
        load_kg,
        lift_m: 3.0,
        gravity: 9.81,
        puller: d.puller(puller).unwrap(),
        rope: d.rope(rope).unwrap(),
        bearing: d.bearing(bearing).unwrap(),
        mode,
        rope_d_mm: None,
    }
}

/// The shipped rows parse, every row the configurator names exists, and the block material is
/// a materials.csv row (validate-data runs every test named "shipped").
#[test]
fn shipped_pulley_data_parses() {
    let d = shipped();
    for id in ["person", "crew_of_four", "truck_winch", "ship_winch", "spaceship_hoist"] {
        assert!(d.puller(id).is_some(), "puller {id}");
    }
    for id in ["manila", "wire_6x19_iwrc"] {
        assert!(d.rope(id).is_some(), "rope {id}");
    }
    for id in ["plain_bore", "bronze_bushed", "anti_friction"] {
        assert!(d.bearing(id).is_some(), "bearing {id}");
    }
    assert!(material_from_csv(humanity_core::embedded_data::MATERIALS_CSV, &d.block.material).is_some());
    assert!(!d.hoists.is_empty());
}

/// Real advantage is below ideal by the friction model, and it is the model the sources print.
/// Crosby's own line-pull ratios (finding F6, catalog page 443) for 1 to 8 parts of line: the sum
/// of e^k reproduces both columns within 2 percent.
///
/// RED first (2026-11-13): with `stage_real_ma` written as the Army's linear rule taken alone,
/// n / (1 + 0.1 n), the test failed at "bronze, 1 parts: model 0.909, Crosby 0.96".
#[test]
fn real_advantage_is_below_ideal_by_the_friction_model() {
    let d = shipped();
    let crosby_bronze = [0.96, 1.87, 2.75, 3.59, 4.39, 5.16, 5.90, 6.60];
    let crosby_roller = [0.98, 1.94, 2.88, 3.81, 4.71, 5.60, 6.47, 7.32];
    for (bearing, table) in [("bronze_bushed", crosby_bronze), ("anti_friction", crosby_roller)] {
        let e = d.bearing(bearing).unwrap().efficiency;
        for (i, &crosby) in table.iter().enumerate() {
            let n = i as u32 + 1;
            let model = stage_real_ma(n, e);
            assert!(model < n as f64, "{bearing}, {n} parts: real {model} must be below ideal {n}");
            assert!(((model - crosby) / crosby).abs() < 0.02, "{bearing}, {n} parts: model {model:.3}, Crosby {crosby}");
        }
        // However many parts, never more than e / (1 - e).
        assert!(stage_real_ma(200, e) < e / (1.0 - e) + 1e-9);
    }
    // A whole system, Realistic: the load over the pull stays below the ideal advantage.
    let inp = inputs(&d, 1000.0, "person", "manila", "bronze_bushed", Mode::Realistic);
    let out = evaluate(&inp, &d, steel(), &[4, 4]);
    assert_eq!(out.ideal_ma, 16.0);
    assert!(out.real_ma < 3.59 * 3.59 + 0.2 && out.real_ma > 10.0, "real {}", out.real_ma);
    // Simplified is the ideal machine.
    let ideal = evaluate(&Inputs { mode: Mode::Simplified, ..inp }, &d, steel(), &[4, 4]);
    assert!((ideal.pull_n - 1000.0 * 9.81 / 16.0).abs() < 1e-6, "simplified pull {}", ideal.pull_n);
}

/// Rope taken in = ideal advantage x lift, whatever the friction or the mode.
///
/// RED first (2026-11-13): with `rope_pulled_m` worked out from the real advantage, the test
/// failed at "[1] plain_bore Realistic: rope 3.3 m, expected 3 m".
#[test]
fn rope_pulled_is_advantage_times_lift() {
    let d = shipped();
    for plan in [vec![1], vec![2], vec![6], vec![3, 3], vec![4, 4], vec![2, 3, 4]] {
        for bearing in ["plain_bore", "bronze_bushed", "anti_friction"] {
            for mode in [Mode::Realistic, Mode::Simplified] {
                let inp = inputs(&d, 500.0, "person", "manila", bearing, mode);
                let out = evaluate(&inp, &d, steel(), &plan);
                let ideal: f64 = plan.iter().map(|&n| n as f64).product();
                assert!(
                    (out.rope_pulled_m - ideal * 3.0).abs() < 1e-9,
                    "{plan:?} {bearing} {mode:?}: rope {:.1} m, expected {} m",
                    out.rope_pulled_m,
                    ideal * 3.0
                );
            }
        }
    }
}

/// The recommendation follows the puller: a person lifting 1,000 kg needs a compound tackle, a
/// truck's winch and a ship's winch lift it nearly straight; at 100 t the truck needs a big
/// tackle and the ship's winch a small one. Every recommended pull is within the puller's force.
///
/// RED first (2026-11-13): with `recommend` ignoring the puller (always the first plan that
/// carries the rope), the test failed at "person: pull 10801.31393668189 over its force".
#[test]
fn the_recommendation_follows_the_puller() {
    let d = shipped();
    let rec = |kg: f64, who: &str| {
        let inp = inputs(&d, kg, who, "wire_6x19_iwrc", "bronze_bushed", Mode::Realistic);
        let out = recommend(&inp, &d, steel());
        assert!(out.refusal.is_none(), "{who}, {kg} kg refused: {:?}", out.refusal);
        assert!(out.pull_n <= d.puller(who).unwrap().force_n, "{who}: pull {} over its force", out.pull_n);
        out.stages.iter().map(|s| s.parts).collect::<Vec<_>>()
    };
    let (person, truck, ship) = (rec(1000.0, "person"), rec(1000.0, "truck_winch"), rec(1000.0, "ship_winch"));
    assert_ne!(person, truck, "person and truck both got {person:?} for 1000 kg");
    assert_ne!(person, ship, "person and ship both got {person:?} for 1000 kg");
    assert!(person.len() >= 2, "a person lifting 1,000 kg needs a compound tackle, got {person:?}");
    let (truck_100t, ship_100t) = (rec(100_000.0, "truck_winch"), rec(100_000.0, "ship_winch"));
    assert_ne!(truck_100t, ship_100t, "100 t: truck and ship both got {truck_100t:?}");
    // The verdicts read the same way.
    let direct = recommend(&inputs(&d, 1000.0, "ship_winch", "wire_6x19_iwrc", "bronze_bushed", Mode::Realistic), &d, steel());
    assert_eq!(direct.verdict, Verdict::Direct);
}

/// A load over the rope's working limit is refused in the Realistic mode, with the numbers that
/// make the reason; the Simplified mode lifts it (no failures).
///
/// RED first (2026-11-13): with the refusal check comparing against the breaking strength (WLL x
/// design factor) instead of the working limit, the test failed at "300 kg on 10 mm manila,
/// [2]: not refused (None)".
#[test]
fn a_rope_over_its_working_limit_is_refused_in_realistic_mode() {
    let d = shipped();
    let mut inp = inputs(&d, 300.0, "truck_winch", "manila", "bronze_bushed", Mode::Realistic);
    inp.rope_d_mm = Some(10.0);
    let out = evaluate(&inp, &d, steel(), &[2]);
    match out.refusal {
        Some(Refusal::RopeOverLimit { stage, rope_d_mm, wll_n, tension_n }) => {
            assert_eq!(stage, 0);
            assert_eq!(rope_d_mm, 10.0);
            assert!((wll_n - 1379.0).abs() < 1.0, "10 mm manila WLL {wll_n}");
            assert!(tension_n > wll_n);
        }
        other => panic!("300 kg on 10 mm manila, [2]: not refused ({other:?})"),
    }
    let simple = evaluate(&Inputs { mode: Mode::Simplified, ..inp }, &d, steel(), &[2]);
    assert!(simple.refusal.is_none(), "Simplified never refuses");
    // The configurator, asked with that rope fixed, finds a tackle with enough parts to keep the
    // thin rope within its limit, or refuses with a reason; it never hands back an overloaded rope.
    let rec = recommend(&inp, &d, steel());
    if rec.refusal.is_none() {
        assert!(rec.stages.iter().all(|s| s.hauling_n <= s.rope_wll_n));
    }
}

/// Block sizes follow the parameters: a 100 t block is bigger in every dimension than a 1 t
/// block, and its rope is thicker.
///
/// RED first (2026-11-13): with `BlockDims::design` ignoring `wll_n` (pin and plates sized from
/// the rope alone), the test failed at "plate: 1 t 0.0020, 100 t 0.0020".
#[test]
fn block_sizes_follow_the_load() {
    let d = shipped();
    let rope = d.rope("wire_6x19_iwrc").unwrap();
    let block = |kg: f64| {
        let w = kg * 9.81;
        let dia = rope.size_for(w / 4.0).unwrap();
        BlockDims::design(2, rope, dia, w, steel(), &d.block)
    };
    let (small, big) = (block(1000.0), block(100_000.0));
    for (name, a, b) in [
        ("rope", small.rope_d, big.rope_d),
        ("tread", small.tread_d, big.tread_d),
        ("pin", small.pin_d, big.pin_d),
        ("plate", small.cheek_t, big.cheek_t),
        ("width", small.width, big.width),
        ("length", small.cheek_h, big.cheek_h),
        ("shackle", small.shackle_d, big.shackle_d),
        ("mass", small.mass_kg, big.mass_kg),
    ] {
        assert!(b > a, "{name}: 1 t {a:.4}, 100 t {b:.4}");
    }
    // The same rope with a heavier load: the pin and plates still grow.
    let dia = rope.size_for(100_000.0 * 9.81 / 4.0).unwrap();
    let light = BlockDims::design(2, rope, dia, 1000.0 * 9.81, steel(), &d.block);
    let heavy = BlockDims::design(2, rope, dia, 100_000.0 * 9.81, steel(), &d.block);
    assert!(heavy.pin_d > light.pin_d, "pin: 1 t {:.4} m, 100 t {:.4} m", light.pin_d, heavy.pin_d);
    // The tread follows the rope's rule: 20 d for wire rope.
    assert!((big.tread_d - 20.0 * big.rope_d).abs() < 1e-9);
}

/// The collision boxes enclose the geometry: every vertex of every block lies inside one of its
/// boxes, and every vertex of the rope inside one of the stage's capsules or boxes.
///
/// RED first (2026-11-13): with the body box built from the sheave's flange instead of the side
/// plates' full length, the test failed at "manila 1 sheaves head up: vertex (-0.012, -0.068,
/// -0.068) outside every box".
#[test]
fn collision_shapes_enclose_the_geometry() {
    let d = shipped();
    let eps = 1e-4;
    for rope_id in ["manila", "wire_6x19_iwrc"] {
        let rope = d.rope(rope_id).unwrap();
        for (kg, parts) in [(200.0, 1u32), (1000.0, 2), (5000.0, 5), (100_000.0, 8)] {
            let w = kg * 9.81;
            let dia = rope.size_for(w / parts as f64).unwrap_or(*rope.diameters_mm.last().unwrap());
            let upper = BlockDims::design(parts.div_ceil(2), rope, dia, w * 1.2, steel(), &d.block);
            let lower = BlockDims::design(parts / 2, rope, dia, w, steel(), &d.block);
            for (b, up) in [(&upper, true), (&lower, false)] {
                let boxes = b.colliders(up);
                for v in b.mesh(up).vertices() {
                    assert!(
                        boxes.iter().any(|bx| bx.contains(v, eps)),
                        "{rope_id} {} sheaves head {}: vertex ({:.3}, {:.3}, {:.3}) outside every box",
                        b.sheaves,
                        if up { "up" } else { "down" },
                        v.x,
                        v.y,
                        v.z
                    );
                }
            }
            let span = 2.0 + 0.5 * (upper.cheek_h + lower.cheek_h);
            let stage = StageShape::build(parts, &upper, &lower, span, 1.5);
            assert!(!stage.rope.positions.is_empty() && !stage.capsules.is_empty());
            for v in stage.rope.vertices() {
                let inside = stage.capsules.iter().any(|c| c.contains(v, eps)) || stage.boxes.iter().any(|b| b.contains(v, eps));
                assert!(inside, "{rope_id} {parts} parts: rope vertex ({:.3}, {:.3}, {:.3}) outside every shape", v.x, v.y, v.z);
            }
            for v in stage.lower.vertices() {
                assert!(stage.boxes.iter().any(|b| b.contains(v, eps)), "{rope_id} {parts} parts: lower block vertex outside");
            }
        }
    }
}

/// A shape is built once per configuration and again when a number changes; with the reuse
/// switch off it is built every time (CLAUDE.md, the reuse rule).
///
/// RED first (2026-11-13): with `get_with` keeping the shape whatever the switch said, the test
/// failed at "reuse off: 1 builds after two asks" (the assert_eq! on `off.builds`).
#[test]
fn shapes_are_built_once_per_configuration_unless_reuse_is_off() {
    let d = shipped();
    let inp = inputs(&d, 1000.0, "person", "manila", "bronze_bushed", Mode::Realistic);
    let out = evaluate(&inp, &d, steel(), &[3, 3]);
    let key = super::shape_key(&out, d.block.lead_m);
    let build = || super::stage_shapes(&out, d.block.lead_m);
    let mut store = ShapeStore::default();
    store.get_with(key, true, build);
    store.get_with(key, true, build);
    assert_eq!(store.builds, 1, "reuse on: one build for one configuration");
    let other = evaluate(&inp, &d, steel(), &[4, 4]);
    store.get_with(super::shape_key(&other, d.block.lead_m), true, || super::stage_shapes(&other, d.block.lead_m));
    assert_eq!(store.builds, 2, "a changed configuration builds anew");
    let mut off = ShapeStore::default();
    off.get_with(key, false, build);
    off.get_with(key, false, build);
    assert_eq!(off.builds, 2, "reuse off: {} builds after two asks", off.builds);
}
