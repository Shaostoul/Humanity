//! Grow lights light the plots near them (2026-09-26, gardening depth).
//!
//! The first light rung made ONE powered grow light light every indoor grow
//! area in the home, which overstated a 100 W fixture by a room's worth of
//! beds. Here a light covers the area its photons can actually serve, nearest
//! plots first. The numbers and their sources live in data/garden/lighting.ron.
//!
//! THE LOOP:
//!
//! 1. The engine publishes the home's grow machines as `GrowPlot`s (DataStore
//!    key "grow_plots", a `Vec<GrowPlot>`) whenever the layout is rebuilt:
//!    each machine's instance id, where it stands, its floor footprint, and a
//!    tower's planted cups.
//! 2. Each tick FarmingSystem gathers the powered grow lights
//!    (`powered_lights`: a `GrowLight` whose `PowerConsumer` is enabled, at its
//!    `Transform`) and shares their light out (`light_cover`): a light covers
//!    `LightingData::area_lit_m2` of canopy, giving each indoor plot within
//!    `reach_m` what it still lacks, nearest first, until it runs out.
//! 3. A crop's cover (0 to 1, looked up by its `tower_id`) is the share of a
//!    night's growth it gets after dark (`farming::light_growth_rate`).
//!
//! Crops with no place (hand-planted, `tower_id` None) are lit by the sun
//! only: nothing says where they stand.

use std::collections::HashMap;

use serde::Deserialize;

/// The hour the sun rises in `solar::sun_factor` (its arc runs 6 to 18);
/// pinned by the test `the_lamp_timer_keeps_a_dark_period`.
const SUNRISE_H: f64 = 6.0;

/// The shipped copy, so a data folder without the file still runs.
pub const LIGHTING_RON: &str = include_str!("../../../data/garden/lighting.ron");

/// The grow-light numbers (data/garden/lighting.ron, every one cited there).
#[derive(Debug, Clone, Deserialize)]
pub struct LightingData {
    /// Photosynthetic photon efficacy, micromoles of photons per joule.
    pub ppe_umol_per_j: f64,
    /// The daily light integral a lit plot is planned for, mol per m2 a day.
    pub target_dli_mol_m2_d: f64,
    /// Canopy area one planted tower cup needs, m2.
    pub cup_canopy_m2: f64,
    /// How far across the floor a light reaches, m (a game estimate).
    pub reach_m: f64,
    /// Hours of light a day the grow lights' timer gives, sun included: they
    /// switch on at sunset and off once the day reaches this, leaving the
    /// rest of the night dark (tomato needs 4 to 6 h of dark, Runkle).
    pub lamp_photoperiod_h: f64,
}

impl LightingData {
    pub fn parse(text: &str) -> Result<Self, String> {
        let d: Self = ron::from_str(text).map_err(|e| e.to_string())?;
        if !(d.ppe_umol_per_j > 0.0
            && d.target_dli_mol_m2_d > 0.0
            && d.cup_canopy_m2 > 0.0
            && d.reach_m > 0.0
            && d.lamp_photoperiod_h > 0.0)
        {
            return Err("every lighting number must be positive".to_string());
        }
        Ok(d)
    }

    pub fn load() -> Self {
        let path = crate::data_dir().join("garden").join("lighting.ron");
        if let Ok(text) = std::fs::read_to_string(&path) {
            match Self::parse(&text) {
                Ok(d) => return d,
                Err(e) => log::warn!("[Farming] {} does not parse ({e}); using the shipped copy", path.display()),
            }
        }
        Self::parse(LIGHTING_RON).expect("the shipped data/garden/lighting.ron parses")
    }

    /// Photon flux density, umol per m2 a second, a plot needs through the
    /// night to receive the target daily light integral in the dark hours
    /// alone (the light is what stands in for a second day of sun).
    pub fn night_ppfd(&self) -> f64 {
        let night_s = 86_400.0 * (1.0 - super::DAYLIGHT_FRACTION);
        self.target_dli_mol_m2_d * 1e6 / night_s
    }

    /// Canopy area, m2, a light drawing `watts` covers at `night_ppfd`.
    pub fn area_lit_m2(&self, watts: f64) -> f64 {
        (watts.max(0.0) * self.ppe_umol_per_j) / self.night_ppfd()
    }

    /// Whether the timer has the grow lights on at `hour`: from sunset until
    /// the day's light reaches `lamp_photoperiod_h`. Never by day (the sun
    /// is up), and all night only when the photoperiod is 24 h.
    pub fn lamps_on_at(&self, hour: f32) -> bool {
        let day_h = 24.0 * super::DAYLIGHT_FRACTION;
        let sunset = SUNRISE_H + day_h;
        let lamp_h = (self.lamp_photoperiod_h - day_h).clamp(0.0, 24.0 - day_h);
        let since_sunset = (f64::from(hour) - sunset).rem_euclid(24.0);
        since_sunset < lamp_h && since_sunset < 24.0 - day_h
    }
}

/// One grow machine as the engine publishes it (DataStore "grow_plots").
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GrowPlot {
    /// The machine instance id; crops in it carry it as `tower_id`.
    pub id: String,
    /// Other ids crops here may carry: a tower's design id ("nutrition"),
    /// which the Garden panel's tower Plant button tags crops with. The first
    /// plot listing an alias owns it.
    pub aliases: Vec<String>,
    /// Where it stands (x, y, z), in the same frame as machine `Transform`s.
    pub pos: [f32; 3],
    /// Floor footprint, m2 (a bed, tray, rack or field).
    pub footprint_m2: f32,
    /// Planted cups for a tower (its canopy is cups x `cup_canopy_m2`); 0 for
    /// anything else.
    pub cups: u32,
    /// An outdoor field: grow lights never serve it.
    pub outdoors: bool,
}

impl GrowPlot {
    /// The canopy a light must cover to light this whole machine, m2.
    pub fn canopy_m2(&self, d: &LightingData) -> f64 {
        if self.cups > 0 {
            f64::from(self.cups) * d.cup_canopy_m2
        } else {
            f64::from(self.footprint_m2.max(0.0))
        }
    }
}

/// The powered grow lights: (position, watts) for every `GrowLight` whose
/// `PowerConsumer` is enabled, at its `Transform`. A shed or switched-off
/// light, or one with no power role or no place, gives no light.
pub fn powered_lights(world: &hecs::World) -> Vec<([f32; 3], f64)> {
    let mut lights: Vec<([f32; 3], f64)> = world
        .query::<(
            &crate::ecs::components::GrowLight,
            &crate::ecs::components::PowerConsumer,
            &crate::ecs::components::Transform,
        )>()
        .iter()
        .filter(|(_, (_, pc, _))| pc.enabled && pc.draw_watts > 0.0)
        .map(|(_, (_, pc, t))| (t.position.to_array(), f64::from(pc.draw_watts)))
        .collect();
    // A fixed order, so two lights sharing a plot always share it the same way.
    lights.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    lights
}

/// How much of each plot the lights cover, 0 to 1, by instance id and alias.
/// Each light covers `area_lit_m2` of canopy at the reference light need
/// (`target_dli_mol_m2_d`); a plot whose crops need more (`need`, mol/m2/day
/// by area id, from `area_needs`) takes proportionally more of it, so a
/// tomato at 25 gets 17/25 of what a lettuce gets from the same light. It
/// gives each indoor plot within `reach_m` (across the floor) what it still
/// lacks, nearest first.
pub fn light_cover(
    lights: &[([f32; 3], f64)],
    plots: &[GrowPlot],
    d: &LightingData,
    need: &HashMap<String, f64>,
) -> HashMap<String, f64> {
    let need_of = |p: &GrowPlot| -> f64 {
        std::iter::once(&p.id)
            .chain(p.aliases.iter())
            .find_map(|k| need.get(k).copied())
            .filter(|t| t.is_finite() && *t > 0.0)
            .unwrap_or(d.target_dli_mol_m2_d)
    };
    let mut cover = vec![0.0_f64; plots.len()];
    for (lp, watts) in lights {
        let mut budget = d.area_lit_m2(*watts);
        let mut near: Vec<(f64, usize)> = plots
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.outdoors)
            .map(|(i, p)| {
                let (dx, dz) = (f64::from(p.pos[0] - lp[0]), f64::from(p.pos[2] - lp[2]));
                ((dx * dx + dz * dz).sqrt(), i)
            })
            .filter(|(dist, _)| *dist <= d.reach_m)
            .collect();
        near.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal).then(plots[a.1].id.cmp(&plots[b.1].id)));
        for (_, i) in near {
            if budget <= 0.0 {
                break;
            }
            let canopy = plots[i].canopy_m2(d) * need_of(&plots[i]) / d.target_dli_mol_m2_d;
            if canopy <= 0.0 {
                continue;
            }
            let give = (canopy * (1.0 - cover[i])).min(budget);
            cover[i] = (cover[i] + give / canopy).min(1.0);
            budget -= give;
        }
    }
    let mut out = HashMap::new();
    for (p, c) in plots.iter().zip(cover) {
        out.insert(p.id.clone(), c);
        for a in &p.aliases {
            out.entry(a.clone()).or_insert(c);
        }
    }
    out
}

/// The light a garden has at one moment.
pub struct LightNow {
    pub sun_up: bool,
    /// The grow lights' timer has them on (never while the sun is up).
    pub lamps_on: bool,
    /// Each grow area's cover by the powered lights, 0 to 1 (empty while
    /// the lamps are off).
    pub cover: HashMap<String, f64>,
}

/// The light at `hour`: the sun, the grow lights' timer, and each area's
/// cover for its crops' light need. What the farming tick and the Garden
/// panel both read.
pub fn light_at(world: &hecs::World, data: &crate::hot_reload::data_store::DataStore, hour: f32) -> LightNow {
    let sun_up = crate::systems::solar::sun_factor(hour) > 0.0;
    let ld = data.get::<LightingData>("garden_lighting");
    let lamps_on = !sun_up && ld.map_or(false, |d| d.lamps_on_at(hour));
    let cover = match (data.get::<Vec<GrowPlot>>("grow_plots"), ld) {
        (Some(plots), Some(d)) if lamps_on => {
            let need = area_needs(world, data.get::<super::PlantRegistry>("plant_registry"), d);
            light_cover(&powered_lights(world), plots, d, &need)
        }
        _ => HashMap::new(),
    };
    LightNow { sun_up, lamps_on, cover }
}

/// `light_at` for the clock's hour now (no clock = noon, the tick's default).
pub fn light_now(world: &hecs::World, data: &crate::hot_reload::data_store::DataStore) -> LightNow {
    let hour = data
        .get::<std::sync::Mutex<crate::systems::time::GameTime>>("game_time")
        .and_then(|m| m.lock().ok().map(|g| g.hour))
        .unwrap_or(12.0);
    light_at(world, data, hour)
}

/// The light need of each grow area, mol/m2/day: the highest `dli_target` of
/// the living crops in it that need light, a crop with no sourced target
/// counting at the reference (`target_dli_mol_m2_d`). A light is shared out
/// for the neediest crop in a plot, the way a grower hangs it.
pub fn area_needs(
    world: &hecs::World,
    plants: Option<&super::PlantRegistry>,
    d: &LightingData,
) -> HashMap<String, f64> {
    let mut need: HashMap<String, f64> = HashMap::new();
    for (_, c) in world.query::<&crate::ecs::components::CropInstance>().iter() {
        if c.growth_stage == crate::ecs::components::STAGE_DEAD {
            continue;
        }
        let Some(area) = c.tower_id.as_deref() else { continue };
        let def = plants.and_then(|r| r.get(&c.crop_def_id));
        if !def.map_or(true, |d| d.needs_light) {
            continue;
        }
        let t = def.and_then(|p| p.dli_target).map_or(d.target_dli_mol_m2_d, f64::from);
        let e = need.entry(area.to_string()).or_insert(0.0);
        *e = e.max(t);
    }
    need
}

/// A crop's light need in words for its card: its sourced target (and range),
/// or the reference it is planned at when its own is not sourced.
pub fn light_need_word(def: Option<&super::PlantDef>, d: &LightingData) -> String {
    let Some(def) = def else { return String::new() };
    if !def.needs_light {
        return "none (grows in the dark)".to_string();
    }
    match (def.dli_target, def.dli_min, def.dli_saturation) {
        (Some(t), Some(lo), Some(hi)) => format!("{t:.0} mol/m2 a day ({lo:.0} to {hi:.0})"),
        (Some(t), _, _) => format!("{t:.0} mol/m2 a day"),
        _ => format!("not sourced for this crop; planned at {:.0} mol/m2 a day", d.target_dli_mol_m2_d),
    }
}

/// What is lighting a crop right now, in words for its card: the same cases
/// `farming::light_growth_rate` decides between.
pub fn light_word(needs_light: bool, outdoors: bool, sun_up: bool, cover: f64) -> String {
    if !needs_light {
        "not needed".to_string()
    } else if sun_up {
        if outdoors { "the sun" } else { "the sun, through the skylight" }.to_string()
    } else if outdoors || cover <= 0.0 {
        "dark: growth waits for sunrise".to_string()
    } else if cover >= 0.999 {
        "grow light".to_string()
    } else {
        format!("grow light on {:.0}% of it: grows at that share", cover * 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> LightingData {
        LightingData::parse(LIGHTING_RON).expect("the shipped lighting.ron parses")
    }

    fn bed(id: &str, x: f32, m2: f32) -> GrowPlot {
        GrowPlot { id: id.to_string(), pos: [x, 0.0, 0.0], footprint_m2: m2, ..Default::default() }
    }

    /// The cited numbers give a 100 W light 0.58 m2: 230 umol/s (DLC Hort
    /// V3.0's 2.30 umol/J) over the 394 umol/m2/s that puts Cornell's
    /// 17 mol/m2/d into a 12-hour night. Which is also why it agrees with the
    /// 2.2 kWh per m2 a day self-sufficiency.md gives greens: 1.2 kWh a night
    /// over 0.58 m2 is 2.1.
    #[test]
    fn a_100_w_light_covers_about_half_a_square_metre() {
        let d = shipped();
        assert!((d.night_ppfd() - 393.5).abs() < 0.5, "night PPFD {}", d.night_ppfd());
        let a = d.area_lit_m2(100.0);
        assert!((a - 0.5845).abs() < 0.001, "area {a}");
        let kwh_per_m2 = 0.1 * 24.0 * (1.0 - super::super::DAYLIGHT_FRACTION) / a;
        assert!((2.0..2.2).contains(&kwh_per_m2), "{kwh_per_m2} kWh per m2 a day");
    }

    /// One light lights the plot under it, not the room. A 2 m2 tray under a
    /// 100 W light is 29% covered; the tray beside it (1.5 m away) gets
    /// nothing, because the first took the whole budget; a tray 5 m away is
    /// out of reach. Seen red by giving every plot within reach the light's
    /// whole area (the home-wide rule this replaces): the second tray was
    /// then lit too.
    #[test]
    fn a_light_covers_the_nearest_plot_first_and_only_what_it_can() {
        let d = shipped();
        let plots = vec![bed("near", 0.2, 2.0), bed("beside", 1.5, 2.0), bed("far", 5.0, 2.0)];
        let cover = light_cover(&[([0.0, 2.0, 0.0], 100.0)], &plots, &d, &HashMap::new());
        let near = cover["near"];
        assert!((near - d.area_lit_m2(100.0) / 2.0).abs() < 1e-9, "near {near}");
        assert_eq!(cover["beside"], 0.0, "the budget went to the nearer tray");
        assert_eq!(cover["far"], 0.0, "out of reach");
    }

    /// A small plot takes only what it needs and the rest goes on: a light
    /// over a 0.3 m2 tray and a 2 m2 tray covers the small one fully and the
    /// big one with what is left. Two lights over one bed add up, and a bed
    /// is never more than fully covered.
    #[test]
    fn light_left_over_goes_to_the_next_plot_and_lights_add_up() {
        let d = shipped();
        let plots = vec![bed("small", 0.0, 0.3), bed("big", 1.0, 2.0)];
        let one = light_cover(&[([0.0, 2.0, 0.0], 100.0)], &plots, &d, &HashMap::new());
        assert!((one["small"] - 1.0).abs() < 1e-9, "small {}", one["small"]);
        let rest = d.area_lit_m2(100.0) - 0.3;
        assert!((one["big"] - rest / 2.0).abs() < 1e-6, "big {}", one["big"]);

        let solo = vec![bed("bed", 0.0, 0.5)];
        let two = light_cover(&[([0.0, 2.0, 0.0], 30.0), ([0.1, 2.0, 0.0], 30.0)], &solo, &d, &HashMap::new());
        let expect = (2.0 * d.area_lit_m2(30.0) / 0.5).min(1.0);
        assert!((two["bed"] - expect).abs() < 1e-9, "two lights add: {} vs {expect}", two["bed"]);
        let many = light_cover(&[([0.0, 2.0, 0.0], 500.0), ([0.0, 2.0, 0.0], 500.0)], &solo, &d, &HashMap::new());
        assert_eq!(many["bed"], 1.0, "never more than fully lit");
    }

    /// The card's words follow the same cases as the growth rate: fungi need
    /// no light, the sun by day, a partly lit plot says how much, a dark one
    /// says it waits.
    #[test]
    fn the_light_word_matches_what_the_crop_gets() {
        assert_eq!(light_word(false, false, false, 0.0), "not needed");
        assert_eq!(light_word(true, true, true, 0.0), "the sun");
        assert_eq!(light_word(true, false, false, 1.0), "grow light");
        assert!(light_word(true, false, false, 0.29).starts_with("grow light on 29%"));
        assert!(light_word(true, true, false, 1.0).starts_with("dark"), "no lamp reaches a field");
        assert!(light_word(true, false, false, 0.0).starts_with("dark"));
    }

    /// A crop that needs more light takes more of the same lamp (plants.csv
    /// `dli_target`): on a 0.5 m2 plot a 100 W light (0.58 m2 at the
    /// lettuce reference, 17) covers lettuce fully and a tomato (25) only
    /// 0.58 x 17/25 / 0.5 = 79%. An area's need is its neediest crop. Seen red
    /// by ignoring the need map (the tomato plot was then fully covered).
    #[test]
    fn a_crop_that_needs_more_light_gets_less_of_the_same_lamp() {
        let d = shipped();
        let plots = vec![bed("bed_a", 0.0, 0.5)];
        let light = [([0.0, 2.0, 0.0], 100.0)];
        let lettuce = light_cover(&light, &plots, &d, &HashMap::from([("bed_a".to_string(), 17.0)]));
        let tomato = light_cover(&light, &plots, &d, &HashMap::from([("bed_a".to_string(), 25.0)]));
        assert_eq!(lettuce["bed_a"], 1.0);
        let want = d.area_lit_m2(100.0) * 17.0 / 25.0 / 0.5;
        assert!((tomato["bed_a"] - want).abs() < 1e-6, "tomato {} vs {want}", tomato["bed_a"]);
        assert!(tomato["bed_a"] < 0.8 && tomato["bed_a"] > 0.78);
    }

    /// The timer: with the shipped 18 h photoperiod the lights are on from
    /// sunset (18:00) to midnight and off until sunrise, so a plot gets six
    /// hours of dark (tomato needs four to six, Runkle), and never by day. A
    /// 24 h photoperiod keeps them on all night. The sun the timer counts
    /// from is `solar::sun_factor`'s: up just before 18:00, down just after.
    /// Seen red by returning true for any night hour (02:00 was then lit).
    #[test]
    fn the_lamp_timer_keeps_a_dark_period() {
        let d = shipped();
        assert_eq!(d.lamp_photoperiod_h, 18.0);
        for (h, on) in [(18.5, true), (23.9, true), (0.1, false), (3.0, false), (5.9, false), (12.0, false)] {
            assert_eq!(d.lamps_on_at(h), on, "{h}:00");
        }
        let mut all_night = d.clone();
        all_night.lamp_photoperiod_h = 24.0;
        assert!(all_night.lamps_on_at(3.0) && all_night.lamps_on_at(20.0) && !all_night.lamps_on_at(12.0));
        let sunset = (SUNRISE_H + 24.0 * super::super::DAYLIGHT_FRACTION) as f32;
        assert!(crate::systems::solar::sun_factor(sunset - 0.05) > 0.0);
        assert_eq!(crate::systems::solar::sun_factor(sunset + 0.05), 0.0);
    }

    /// The card's light need: a sourced crop gives its target and range, an
    /// unsourced one says it is planned at the reference, a fungus needs none.
    #[test]
    fn the_light_need_word_says_where_the_number_comes_from() {
        let d = shipped();
        let csv = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/plants.csv"));
        let plants = super::super::PlantRegistry::from_csv(csv.as_bytes()).expect("plants.csv");
        assert_eq!(light_need_word(plants.get("tomato"), &d), "25 mol/m2 a day (15 to 30)");
        assert!(light_need_word(plants.get("carrot"), &d).starts_with("not sourced"));
        assert!(light_need_word(plants.get("oyster_mushroom"), &d).starts_with("none"));
    }

    /// The shipped dli columns, as data: every filled cell is one the loader
    /// read (a typo would quietly fall back to the reference), each crop's
    /// numbers run min <= target <= saturation where present, and the 27
    /// sourced targets of the findings document are all there. Seen red by
    /// setting tomato's dli_min to 35 (above its target).
    #[test]
    fn shipped_dli_columns_are_read_and_ordered() {
        let csv = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/plants.csv"));
        let plants = super::super::PlantRegistry::from_csv(csv.as_bytes()).expect("plants.csv");
        let mut header: Vec<&str> = Vec::new();
        let mut targets = 0;
        for line in csv.lines().filter(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty()) {
            let cells: Vec<&str> = line.split(',').collect();
            if header.is_empty() {
                header = cells;
                continue;
            }
            let col = |name: &str| cells[header.iter().position(|h| *h == name).expect(name)].trim();
            let def = plants.get(cells[0]).unwrap_or_else(|| panic!("{} loads", cells[0]));
            for (name, v) in [("dli_min", def.dli_min), ("dli_target", def.dli_target), ("dli_saturation", def.dli_saturation)] {
                assert_eq!(col(name).is_empty(), v.is_none(), "{} {name} {:?} was not read", cells[0], col(name));
                if let Some(v) = v {
                    assert!((4.0..=160.0).contains(&v), "{} {name} {v} out of range", cells[0]);
                }
            }
            let (lo, t, hi) = (def.dli_min, def.dli_target, def.dli_saturation);
            if let (Some(lo), Some(t)) = (lo, t) {
                assert!(lo <= t, "{}: min {lo} above target {t}", cells[0]);
            }
            if let (Some(t), Some(hi)) = (t, hi) {
                assert!(t <= hi, "{}: target {t} above saturation {hi}", cells[0]);
            }
            targets += usize::from(t.is_some());
        }
        assert_eq!(targets, 27, "the findings document sources 27 targets");
    }

    /// A tower's canopy is its planted cups at Cornell's finishing spacing
    /// (38 plants a m2), and a crop tagged with the tower's design id finds
    /// the first tower of that design. An outdoor field is never lit.
    #[test]
    fn towers_count_their_cups_aliases_resolve_and_fields_stay_dark() {
        let d = shipped();
        let tower = GrowPlot {
            id: "ntower_0".into(),
            aliases: vec!["nutrition".into()],
            pos: [0.0, 0.0, 0.0],
            footprint_m2: 0.09,
            cups: 12,
            outdoors: false,
        };
        assert!((tower.canopy_m2(&d) - 12.0 * 0.026).abs() < 1e-9);
        let field = GrowPlot { id: "grain_field_1".into(), pos: [0.5, 0.0, 0.0], footprint_m2: 5.76, outdoors: true, ..Default::default() };
        let cover = light_cover(&[([0.0, 2.0, 0.0], 100.0)], &[tower, field], &d, &HashMap::new());
        assert_eq!(cover["ntower_0"], 1.0, "0.31 m2 of cups under a 0.58 m2 light");
        assert_eq!(cover["nutrition"], 1.0, "the design id resolves to the tower");
        assert_eq!(cover["grain_field_1"], 0.0, "a field has only the sun");
    }
}
