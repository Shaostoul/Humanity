//! Tests for `systems::body_heat`. The literature figures each test holds the
//! model to are in its doc comment, beside what the model gives.
//!
//! "Seen red" for these: the whole file is new (the model it tests did not
//! exist), so the red run is the FoodSystem one in `systems::food`
//! (`outside_at_15c_a_walking_person_lasts_hours`), which failed on the old
//! drift model with "hypothermia at 15 C after 4.5 s". Each test below says
//! what change to the model turns it red.

use super::*;

/// A still, bare person in calm air (about 1 km/h), the CESM's reference case.
fn calm(air_c: f32) -> Exposure {
    // 0.42 m/s at 10 m is 0.28 m/s (1 km/h) at the body.
    Exposure::outdoors(air_c, 0.7, 0.42, 0.0)
}

/// Run a body from `start_c` for `hours`; returns (hours to 35 C, hours to
/// 28 C, the core every hour). Steps of a minute (cut to ten seconds inside).
fn run(ex: &Exposure, clo: f32, met: f32, hours: f32, start_c: f64) -> (Option<f32>, Option<f32>, Vec<f64>, BodyHeat) {
    let mut b = BodyHeat::new(start_c);
    let (mut t35, mut t28, mut hourly) = (None, None, vec![start_c]);
    let steps = (hours * 60.0) as usize;
    for i in 1..=steps {
        b.step(ex, clo, met, 60.0);
        let h = i as f32 / 60.0;
        if t35.is_none() && b.core_c < 35.0 {
            t35 = Some(h);
        }
        if t28.is_none() && b.core_c < 28.0 {
            t28 = Some(h);
        }
        if i % 60 == 0 {
            hourly.push(b.core_c);
        }
    }
    (t35, t28, hourly, b)
}

/// THE DEFECT, in the model: 15 C still air, the everyday outfit (0.61 clo),
/// walking. A real person is comfortable like this all day; the old drift
/// model put them below 35 C in 4.5 s. The model holds the core at 36.96 C
/// for 8 h, and the Forgiving mode shows 36.88 C. Standing still is fine too
/// (36.78 C at 8 h, the skin cooling to about 28 C as the vessels narrow).
/// Red check: remove the metabolic heat (`metabolic = 0`) and the core falls
/// below 35 C within the 8 h.
#[test]
fn fifteen_c_still_air_walking_is_comfortable_for_hours_in_both_modes() {
    let ex = Exposure::outdoors(15.0, 0.6, 0.0, 0.0);
    for met in [MET_WALKING, MET_STANDING] {
        let (t35, _, hourly, _) = run(&ex, BASE_OUTFIT_CLO, met, 8.0, 37.0);
        assert!(t35.is_none(), "{met} met: hypothermic at {t35:?} h");
        for (h, c) in hourly.iter().enumerate() {
            for mode in [Mode::Realistic, Mode::Forgiving] {
                let shown = mode.shown(*c);
                assert!((36.5..37.5).contains(&shown), "{met} met, {mode:?}, hour {h}: core {shown:.2}");
            }
        }
    }
}

/// 0 C, a 5 m/s wind, the everyday outfit, standing still (someone waiting
/// outside). Literature: Helland et al. 2025 measured a 0.25 C fall over 3 h
/// for people in wet light clothes at 5 C in a wind; Thompson and Hayward 1996
/// found most walkers hold their core for at least 4 h in 5 C rain and warned
/// that past 4 h shivering exhaustion brings a rapid decline; the Cold Exposure
/// Survival Model (Tikuisis 1995) gives 9.0 h to a 28 C core for a bare person
/// in calm 0 C air, and 0.61 clo in this wind is about the same insulation.
/// The model: the core holds near 36.6 C for 6 h (the skin cooling to take
/// the loss), then falls as the shivering tires: 35 C at about 9.8 h, 28 C at
/// about 14.3 h. Red check: without shivering fatigue (`shiver_capacity`
/// never falls) the core never reaches 35 C in 16 h.
#[test]
fn light_clothes_at_0c_in_wind_reach_mild_hypothermia_in_hours() {
    let ex = Exposure::outdoors(0.0, 0.8, 5.0, 0.0);
    let (t35, t28, hourly, _) = run(&ex, BASE_OUTFIT_CLO, MET_STANDING, 16.0, 37.0);
    assert!(hourly[3] > 36.2, "defended for the first 3 h like Helland's subjects: {:.2}", hourly[3]);
    let t35 = t35.expect("cools to mild hypothermia");
    assert!((6.0..13.0).contains(&t35), "35 C after {t35:.1} h");
    let t28 = t28.expect("and on to severe");
    assert!(t28 > t35 + 2.0, "28 C after {t28:.1} h");
    // Walking in the same weather keeps the core up (Thompson and Hayward's walkers).
    let (walk35, _, _, _) = run(&ex, BASE_OUTFIT_CLO, MET_WALKING, 16.0, 37.0);
    assert!(walk35.is_none(), "walking held off hypothermia, got {walk35:?}");
}

/// The calibration, pinned. CESM (Tikuisis 1995): a bare still person in calm
/// air survives (core above 28 C) 9.0 h at 0 C and more than 24 h at 10 C. The
/// model gives 9.6 h and 24.7 h. Red check: raise `SHIVER_ENDURANCE` to 1.0
/// and 0 C takes more than 12 h.
#[test]
fn a_bare_person_in_calm_air_matches_the_cold_exposure_survival_model() {
    let (_, t28, _, _) = run(&calm(0.0), 0.0, 1.0, 14.0, 37.0);
    let t28 = t28.expect("0 C calm air, bare, reaches 28 C");
    assert!((7.5..11.5).contains(&t28), "0 C: 28 C after {t28:.1} h (CESM 9.0 h)");
    let (_, t28, _, _) = run(&calm(10.0), 0.0, 1.0, 24.0, 37.0);
    assert!(t28.is_none(), "10 C: survives past 24 h (CESM: more than 24 h), got {t28:?}");
}

/// Helland et al. 2025: wet light clothes, 5 C, a wind, still, 3 h: the core
/// fell 0.25 C (0.17 C an hour). The model, from a neutral 36.8 C: 36.56 C at
/// 3 h, a fall of 0.24 C. Red check: set `SHIVER_SUSTAINED_SHARE` to 0.3 and
/// the fall is about 1 C.
#[test]
fn wet_light_clothes_at_5c_for_3_hours_match_helland() {
    let ex = Exposure::outdoors(5.0, 0.9, 3.0, 1.0);
    let (_, _, hourly, b) = run(&ex, BASE_OUTFIT_CLO, 1.0, 3.0, CORE_NEUTRAL_C);
    let fall = CORE_NEUTRAL_C - hourly[3];
    assert!((0.05..0.5).contains(&fall), "fell {fall:.2} C in 3 h (Helland: 0.25)");
    assert!(b.clothing_wetness > 0.9, "the rain soaked the clothes: {}", b.clothing_wetness);
    assert!(b.is_shivering(), "and the body is shivering to hold the core");
}

/// Hard work in heat raises the core. 35 C, 50 percent humidity, a light
/// breeze, light clothes (0.5 clo), heavy work (4 met): the core reaches about
/// 39.0 C in an hour and 40.3 C in two, into heatstroke. The same work at
/// 20 C settles at about 37.4 C. Red check: without the metabolic heat
/// (`met = 1`) the 35 C core stays under 38 C.
#[test]
fn hard_work_in_heat_raises_the_core() {
    let hot = Exposure::outdoors(35.0, 0.5, 1.0, 0.0);
    let (_, _, hourly, _) = run(&hot, 0.5, 4.0, 2.0, 37.0);
    assert!(hourly[1] > 38.5, "an hour of heavy work at 35 C: {:.2}", hourly[1]);
    assert!(hourly[2] > f64::from(HEAT_STROKE_C), "two hours: {:.2}", hourly[2]);
    let mild = Exposure::outdoors(20.0, 0.5, 1.0, 0.0);
    let (_, _, hourly, _) = run(&mild, 0.5, 4.0, 2.0, 37.0);
    assert!(hourly[2] < 37.8, "the same work at 20 C: {:.2}", hourly[2]);
}

/// Wet clothing cools faster than dry. 5 C, a 3.3 m/s wind, the everyday
/// outfit, standing: soaked by rain, the core reaches 35 C at about 7.3 h;
/// dry, it holds at 36.6 C for 12 h. The skin is colder wet from the first
/// hour. Red check: set `WET_INSULATION_LOSS` and `WET_CLOTHES_COOLING_SHARE`
/// to 0 and the two runs are the same.
#[test]
fn wet_clothing_cools_faster_than_dry() {
    let wet = Exposure::outdoors(5.0, 0.9, 3.3, 1.0);
    let dry = Exposure { precipitation: 0.0, ..wet };
    let (wet35, _, wet_h, wet_b) = run(&wet, BASE_OUTFIT_CLO, MET_STANDING, 12.0, 37.0);
    let (dry35, _, dry_h, dry_b) = run(&dry, BASE_OUTFIT_CLO, MET_STANDING, 12.0, 37.0);
    assert!(wet_b.skin_c < dry_b.skin_c, "wet skin {:.1}, dry {:.1}", wet_b.skin_c, dry_b.skin_c);
    let wet35 = wet35.expect("soaked, the core reaches 35 C");
    assert!(dry35.is_none(), "dry, it holds: {dry35:?}");
    assert!(wet35 < 10.0, "soaked: 35 C after {wet35:.1} h");
    assert!(wet_h[6] < dry_h[6], "at 6 h: wet {:.2}, dry {:.2}", wet_h[6], dry_h[6]);
    // Shelter stops the rain, and the clothes start drying.
    let mut b = wet_b.clone();
    b.step(&Exposure::indoors(20.0, 0.4), BASE_OUTFIT_CLO, MET_STANDING, 3600.0);
    assert!(b.clothing_wetness < wet_b.clothing_wetness, "drying indoors: {}", b.clothing_wetness);
}

/// Clothes help: at -10 C in a 5 m/s wind the winter kit (the everyday
/// outfit plus the parka, knit hat, winter gloves and boots of
/// data/equipment.csv: 0.61 + 0.70 + 0.03 + 0.05 + 0.08 = 1.47 clo) holds the
/// core at 36.6 C after 4 h, where the everyday outfit alone has fallen to
/// 36.1 C and is tiring. And shelter helps: a rainy 0 C day in an 8 m/s wind
/// takes a person in the everyday outfit to 28 C in about 6 h; under a roof
/// they hold 36.6 C. Red check: make `Exposure::air_speed` ignore `sheltered`
/// and let rain fall under the roof, and the sheltered run matches the open one.
#[test]
fn clothes_and_shelter_help() {
    let ex = Exposure::outdoors(-10.0, 0.7, 5.0, 0.0);
    let (_, _, light, _) = run(&ex, BASE_OUTFIT_CLO, MET_STANDING, 4.0, 37.0);
    let (_, _, kit, _) = run(&ex, BASE_OUTFIT_CLO + 0.70 + 0.03 + 0.05 + 0.08, MET_STANDING, 4.0, 37.0);
    assert!(kit[4] > light[4] + 0.2, "kit {:.2}, light {:.2}", kit[4], light[4]);
    let windy = Exposure::outdoors(0.0, 0.8, 8.0, 1.0);
    let sheltered = Exposure { sheltered: true, ..windy };
    let (_, _, open, _) = run(&windy, BASE_OUTFIT_CLO, MET_STANDING, 8.0, 37.0);
    let (_, _, under, _) = run(&sheltered, BASE_OUTFIT_CLO, MET_STANDING, 8.0, 37.0);
    assert!(under[8] > open[8], "sheltered {:.2}, open {:.2}", under[8], open[8]);
}

/// Forgiving never kills sooner than Realistic. Over a grid of weather,
/// clothes and work, health is run down from 100 by each mode's harm for 12 h;
/// the Forgiving death, when there is one, never comes first. The grid is not
/// vacuous: Realistic kills in some of it. Red check: set `FORGIVING_SWING`
/// above 1 and Forgiving dies first in the cold.
#[test]
fn forgiving_never_kills_sooner_than_realistic() {
    let mut realistic_deaths = 0;
    for air in [-30.0_f32, -10.0, 0.0, 10.0, 40.0, 50.0] {
        for wind in [0.0_f32, 8.0] {
            for clo in [0.0_f32, BASE_OUTFIT_CLO, 1.5] {
                for met in [MET_STANDING, 4.0] {
                    for rain in [0.0_f32, 1.0] {
                        let ex = Exposure::outdoors(air, 0.5, wind, rain);
                        let mut b = BodyHeat::new(37.0);
                        let (mut hp_r, mut hp_f) = (100.0_f32, 100.0_f32);
                        let (mut dead_r, mut dead_f) = (None, None);
                        for minute in 1..=(12 * 60) {
                            b.step(&ex, clo, met, 60.0);
                            for (mode, hp, dead) in
                                [(Mode::Realistic, &mut hp_r, &mut dead_r), (Mode::Forgiving, &mut hp_f, &mut dead_f)]
                            {
                                *hp -= harm_per_s(mode.shown(b.core_c) as f32, mode) * 60.0;
                                if *hp <= 0.0 && dead.is_none() {
                                    *dead = Some(minute);
                                }
                            }
                        }
                        if dead_r.is_some() {
                            realistic_deaths += 1;
                        }
                        if let Some(f) = dead_f {
                            let r = dead_r.unwrap_or(usize::MAX);
                            assert!(
                                f >= r,
                                "{air} C, wind {wind}, {clo} clo, {met} met, rain {rain}: Forgiving dead at {f} min, Realistic at {r}"
                            );
                        }
                    }
                }
            }
        }
    }
    assert!(realistic_deaths >= 5, "the grid includes deadly weather: {realistic_deaths} Realistic deaths");
}

/// The harm schedule: nothing between 32 and 40 C, rising a degree at a time
/// beyond, Forgiving at half. And the Forgiving core is the real one's swing
/// halved, both ways.
#[test]
fn harm_starts_at_moderate_hypothermia_and_heatstroke() {
    for c in [32.0_f32, 35.0, 37.0, 39.5, 40.0] {
        assert_eq!(harm_per_s(c, Mode::Realistic), 0.0, "{c} C");
    }
    assert!((harm_per_s(28.0, Mode::Realistic) - 100.0 / 3600.0).abs() < 1e-6, "28 C: 100 points an hour");
    assert!((harm_per_s(42.0, Mode::Realistic) - 200.0 / 3600.0).abs() < 1e-6, "42 C: 100 points in 30 minutes");
    assert!((harm_per_s(28.0, Mode::Forgiving) - 0.5 * harm_per_s(28.0, Mode::Realistic)).abs() < 1e-9);
    assert!((Mode::Forgiving.shown(34.8) - 35.8).abs() < 1e-9);
    assert!((Mode::Forgiving.real(35.8) - 34.8).abs() < 1e-9);
    assert_eq!(Mode::Realistic.shown(33.0), 33.0);
}

/// Rain wets a person at its rate, snow at a third, nothing falling not at
/// all, and a mix by its parts (the phase comes from the air where it falls:
/// `systems::precipitation`).
#[test]
fn precipitation_from_the_weather() {
    use crate::systems::precipitation::Falling;
    assert_eq!(precipitation(Falling { rain: 0.6, snow: 0.0 }), 0.6);
    assert_eq!(precipitation(Falling { rain: 1.4, snow: 0.0 }), 1.0);
    assert!((precipitation(Falling { rain: 0.0, snow: 0.6 }) - 0.2).abs() < 1e-6);
    assert!((precipitation(Falling { rain: 0.3, snow: 0.3 }) - 0.4).abs() < 1e-6);
    assert_eq!(precipitation(Falling::NONE), 0.0);
}

/// The figures docs/design/body-heat.md quotes, printed (run with
/// `cargo test --features native --lib reference_figures -- --ignored --nocapture`).
#[test]
#[ignore]
fn print_reference_figures() {
    let show = |name: &str, ex: Exposure, clo: f32, met: f32, hours: f32| {
        let (t35, t28, hourly, b) = run(&ex, clo, met, hours, 37.0);
        let h = |x: Option<f32>| x.map_or("never".to_string(), |v| format!("{v:.1} h"));
        println!(
            "{name:<44} 35 C: {:>8}  28 C: {:>8}  core at 3 h {:.2}, at end {:.2}, skin {:.1}",
            h(t35),
            h(t28),
            hourly.get(3).copied().unwrap_or(f64::NAN),
            b.core_c,
            b.skin_c
        );
    };
    let everyday = BASE_OUTFIT_CLO;
    show("15 C still, everyday clothes, walking", Exposure::outdoors(15.0, 0.6, 0.0, 0.0), everyday, MET_WALKING, 8.0);
    show("15 C still, everyday clothes, standing", Exposure::outdoors(15.0, 0.6, 0.0, 0.0), everyday, MET_STANDING, 8.0);
    show("0 C, 5 m/s wind, everyday clothes, standing", Exposure::outdoors(0.0, 0.8, 5.0, 0.0), everyday, MET_STANDING, 16.0);
    show("0 C, 5 m/s wind, everyday clothes, walking", Exposure::outdoors(0.0, 0.8, 5.0, 0.0), everyday, MET_WALKING, 16.0);
    show("bare, calm, 0 C (CESM 9.0 h to 28 C)", calm(0.0), 0.0, 1.0, 16.0);
    show("bare, calm, -10 C (CESM 4.1 h)", calm(-10.0), 0.0, 1.0, 16.0);
    show("bare, calm, -20 C (CESM 2.5 h)", calm(-20.0), 0.0, 1.0, 16.0);
    show("bare, calm, 10 C (CESM more than 24 h)", calm(10.0), 0.0, 1.0, 26.0);
    show("5 C rain, 3.3 m/s, everyday, standing", Exposure::outdoors(5.0, 0.9, 3.3, 1.0), everyday, MET_STANDING, 12.0);
    show("5 C dry, 3.3 m/s, everyday, standing", Exposure::outdoors(5.0, 0.9, 3.3, 0.0), everyday, MET_STANDING, 12.0);
    show("5 C rain walking (Thompson and Hayward)", Exposure::outdoors(5.0, 0.95, 3.3, 1.0), everyday, 2.6, 5.0);
    show("35 C 50%, 0.5 clo, 4 met", Exposure::outdoors(35.0, 0.5, 1.0, 0.0), 0.5, 4.0, 2.0);
    show("-20 C, 5 m/s, everyday clothes, standing", Exposure::outdoors(-20.0, 0.7, 5.0, 0.0), everyday, MET_STANDING, 8.0);
    show("-20 C, 5 m/s, + winter coat, standing", Exposure::outdoors(-20.0, 0.7, 5.0, 0.0), everyday + 0.70, MET_STANDING, 8.0);
    let (_, _, hourly, _) = run(&Exposure::outdoors(5.0, 0.9, 3.0, 1.0), everyday, 1.0, 3.0, CORE_NEUTRAL_C);
    println!("Helland (5 C, wet, wind, still, 3 h from 36.8): fell {:.2} C (measured 0.25)", CORE_NEUTRAL_C - hourly[3]);
}

/// A sealed or sheltered context is still, dry air whatever the weather says.
#[test]
fn a_sealed_context_is_still_dry_air() {
    let env = EnvironmentContext {
        sealed: true,
        ambient_temp_c: 20.0,
        wind_m_s: 15.0,
        precipitation: 1.0,
        ..Default::default()
    };
    let ex = Exposure::from_context(&env);
    assert!(ex.sheltered && ex.wind_10m_m_s == 0.0 && ex.precipitation == 0.0);
    let out = Exposure::from_context(&EnvironmentContext { sealed: false, ..env });
    assert!(!out.sheltered && out.wind_10m_m_s == 15.0 && out.precipitation == 1.0);
    let roofed = Exposure::from_context(&EnvironmentContext { sealed: false, sheltered: true, ..env });
    assert!(roofed.sheltered && roofed.precipitation == 0.0, "the shelter input blocks wind and rain");
}

/// THE CLEAR NIGHT SKY (2026-09-28). On a clear 10 C night the open-air mean
/// radiant temperature is about 0.5 C (Swinbank's sky at 263 K, half the view
/// sky and half ground at the air's 283 K, mixed as T^4). Overcast, the cloud
/// base radiates at the air's temperature, and by day (the sun's warmth not
/// modelled) the surroundings read the air, as before. Red check, run:
/// ignoring the cloud share (a clear sky always) fails the overcast
/// assertion.
#[test]
fn a_clear_night_sky_is_colder_than_the_air() {
    let clear_night = open_sky_radiant_c(10.0, 0.0, 1.0);
    assert!((clear_night - 0.45).abs() < 0.3, "clear 10 C night: {clear_night}");
    assert!((open_sky_radiant_c(10.0, 1.0, 1.0) - 10.0).abs() < 1e-3, "overcast night");
    assert!((open_sky_radiant_c(10.0, 0.0, 0.0) - 10.0).abs() < 1e-3, "by day");
    // The weight: all night, none at noon, fading in the first hour.
    assert_eq!(night_sky_weight(0.0), 1.0);
    assert_eq!(night_sky_weight(6.0), 1.0, "sunrise");
    assert_eq!(night_sky_weight(12.0), 0.0, "noon");
    assert_eq!(night_sky_weight(18.0), 1.0, "sunset");
    let early = night_sky_weight(6.5);
    assert!(early > 0.0 && early < 1.0, "half an hour after sunrise: {early}");
}
