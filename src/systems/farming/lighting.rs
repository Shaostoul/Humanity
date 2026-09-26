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
}

impl LightingData {
    pub fn parse(text: &str) -> Result<Self, String> {
        let d: Self = ron::from_str(text).map_err(|e| e.to_string())?;
        if !(d.ppe_umol_per_j > 0.0 && d.target_dli_mol_m2_d > 0.0 && d.cup_canopy_m2 > 0.0 && d.reach_m > 0.0) {
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
/// Each light covers `area_lit_m2` of canopy, giving each indoor plot within
/// `reach_m` (measured across the floor) what it still lacks, nearest first.
pub fn light_cover(lights: &[([f32; 3], f64)], plots: &[GrowPlot], d: &LightingData) -> HashMap<String, f64> {
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
            let canopy = plots[i].canopy_m2(d);
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
        let cover = light_cover(&[([0.0, 2.0, 0.0], 100.0)], &plots, &d);
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
        let one = light_cover(&[([0.0, 2.0, 0.0], 100.0)], &plots, &d);
        assert!((one["small"] - 1.0).abs() < 1e-9, "small {}", one["small"]);
        let rest = d.area_lit_m2(100.0) - 0.3;
        assert!((one["big"] - rest / 2.0).abs() < 1e-6, "big {}", one["big"]);

        let solo = vec![bed("bed", 0.0, 0.5)];
        let two = light_cover(&[([0.0, 2.0, 0.0], 30.0), ([0.1, 2.0, 0.0], 30.0)], &solo, &d);
        let expect = (2.0 * d.area_lit_m2(30.0) / 0.5).min(1.0);
        assert!((two["bed"] - expect).abs() < 1e-9, "two lights add: {} vs {expect}", two["bed"]);
        let many = light_cover(&[([0.0, 2.0, 0.0], 500.0), ([0.0, 2.0, 0.0], 500.0)], &solo, &d);
        assert_eq!(many["bed"], 1.0, "never more than fully lit");
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
        let cover = light_cover(&[([0.0, 2.0, 0.0], 100.0)], &[tower, field], &d);
        assert_eq!(cover["ntower_0"], 1.0, "0.31 m2 of cups under a 0.58 m2 light");
        assert_eq!(cover["nutrition"], 1.0, "the design id resolves to the tower");
        assert_eq!(cover["grain_field_1"], 0.0, "a field has only the sun");
    }
}
