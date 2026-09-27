//! Ship life support (2026-09-26): the home's air and the garden's water as
//! closed loops, aboard the station the home is (data/stations/home.ron).
//!
//! The operator's direction (2026-09-27): "We should focus on the space ship
//! first ... I imagine the spaceship gardens are a great way to figure out all
//! the physics, like the gasses, liquids, etc. to properly account for things."
//! Every number and its source live in data/life_support.ron; the design, with
//! every balance worked for both shipped homes, is
//! docs/design/ship-life-support.md. This file is the physics the farming
//! tick's air step (farming::humidity::step_rooms) uses, and the home's own air.
//!
//! THE AIRS. Each grow room keeps its own air (farming::humidity), a fruiting
//! tent keeps its own inside its room, and everything else in the sealed home
//! is the home's own air, `HomeAirState` (saved in SoilMemory). Each air holds
//! three things tracked as grams per cubic metre: water vapour, carbon dioxide
//! and, in the home's air, oxygen. Nitrogen is the rest.
//!
//! THE BALANCE of one gas in one air, per hour:
//!
//! ```text
//! dx/dt = s - SUM_j k_j (x - u_j)
//! ```
//!
//! `s` is what its sources put in (crops, people, mushrooms, humidifiers), per
//! m3 an hour, and each sink j pulls it toward a level u_j at a rate k_j an
//! hour: the air it exchanges with (its leakage and fans, u = that air's
//! level), an air handler's cold coil (u = what air leaving the coil holds), a
//! scrubber (u = 0), the leak overboard (u = 0). Linear, so `relax` solves it
//! exactly over any tick, and returns how much went to each sink: nothing is
//! created or lost inside the air, and a frame, a clock jump and a catch-up
//! agree. A crop's uptake depends on the air's carbon dioxide (Kimball's law,
//! `co2_factor`), so it is taken at the tick's start and the step is cut into
//! slices of at most `MAX_SLICE_H` (a catch-up).
//!
//! THE BOUNDARY WITH THE TANKS. The air runs on the game clock (its hours are
//! game hours, as the rooms' always have). The tanks run on the plumbing sim's
//! own clock, real minutes (plumbing.rs). Which of the two is right is the
//! operator's open question (docs/PRIORITIES.md, blocked 3; BUG-092 item 7),
//! so this does not choose. Every flow crosses the boundary as LITRES A DAY,
//! taken from one physical state: the garden draws its crops' water (their
//! transpiration plus what their tissue keeps) and the humidifiers' mist, and
//! each air handler hands back the litres a day its coil condenses. Each side
//! applies those daily figures on its own clock. Because they are the same
//! day's figures, a day's water balances on the tank side (drawn = returned +
//! kept + lost) exactly as it does on the air side; the clocks only decide how
//! long a day lasts. The air's own store (the vapour it holds) fills and
//! empties on the air's clock; at steady state it does neither.
//! `farming::life_support_tests` proves both halves.
//!
//! COST: per tick, one pass over the airs (tens of rooms and tents, a handful
//! of machines each), a few exp() each. Gameplay state only, nothing drawn or
//! networked.

use std::collections::HashMap;

use serde::Deserialize;

use crate::hot_reload::data_store::DataStore;
use crate::systems::farming::humidity::HumidityData;

/// The shipped copy, so a bare exe with no data folder still has the model.
pub const LIFE_SUPPORT_RON: &str = include_str!("../../data/life_support.ron");

/// DataStore key of the loaded `LifeSupportData` (lib.rs registers it).
pub const DATA_KEY: &str = "life_support";

/// The longest slice of game hours one step solves in one go: a catch-up is
/// cut into slices this long, so a crop's uptake (taken at a slice's start)
/// never runs far past the air it was set from. A frame is 1/50 of a game hour.
pub const MAX_SLICE_H: f64 = 0.25;

/// A controller counts an air as at its setpoint down to this share of it, and
/// holds there; further below its machine idles (the exhaust fans' band).
pub const HOLD_BAND: f64 = 0.98;

// -- Data: data/life_support.ron ------------------------------------------------

/// One crop's gas exchange as NASA measured it (BVAD 2022 Tables 4-90, 4-91),
/// per square metre a day.
#[derive(Debug, Clone, Deserialize)]
pub struct CropGas {
    pub name: String,
    /// The plants.csv ids it stands for.
    pub plants: Vec<String>,
    pub co2_g_m2_d: f64,
    pub o2_g_m2_d: f64,
    pub water_kg_m2_d: f64,
    pub edible_fw_g_m2_d: f64,
    pub edible_water_pct: f64,
    pub inedible_fw_g_m2_d: f64,
    pub inedible_water_pct: f64,
}

/// What a crop exchanges per litre of water it breathes out: grams of carbon
/// dioxide it fixes and of oxygen it gives out (at the reference CO2, while
/// lit), and litres its tissue keeps on top.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Exchange {
    pub co2_g_per_l: f64,
    pub o2_g_per_l: f64,
    pub kept_l_per_l: f64,
}

impl CropGas {
    pub fn exchange(&self) -> Exchange {
        let kg = self.water_kg_m2_d.max(1e-9);
        let kept_g = (self.edible_fw_g_m2_d * self.edible_water_pct + self.inedible_fw_g_m2_d * self.inedible_water_pct) / 100.0;
        Exchange { co2_g_per_l: self.co2_g_m2_d / kg, o2_g_per_l: self.o2_g_m2_d / kg, kept_l_per_l: kept_g / (kg * 1000.0) }
    }
}

/// What data/life_support.ron holds. Its comments carry every source.
#[derive(Debug, Clone, Deserialize)]
pub struct LifeSupportData {
    pub reference_mj_per_day: f64,
    pub reference_o2_kg: f64,
    pub reference_co2_kg: f64,
    pub reference_water_kg: f64,
    pub kj_per_kcal: f64,
    pub crops: Vec<CropGas>,
    pub co2_reference_ppm: f64,
    pub co2_gain_per_doubling: f64,
    pub co2_doubling_base_ppm: f64,
    pub fungi_respiratory_quotient: f64,
    pub o2_molar_mass: f64,
    pub coil_dewpoint_c: f64,
    pub grow_room_setpoint_rh: f64,
    pub home_setpoint_rh: f64,
    /// (share of full airflow, share of full power), ascending.
    pub air_handler_speeds: Vec<(f64, f64)>,
    pub scrubber_setpoint_ppm: f64,
    pub scrubber_rated_ppm: f64,
    pub leak_kg_per_day_per_module: f64,
    /// Every listed plant's exchange and the median for the rest, worked out
    /// once when the data is read (`parse`): the crop loop asks for one per
    /// growing crop, twice a tick, a few thousand times in a full garden.
    #[serde(skip)]
    by_plant: HashMap<String, Exchange>,
    #[serde(skip)]
    median: Exchange,
}

/// A person's (or a household's) daily breath: grams of oxygen in, of carbon
/// dioxide and water vapour out.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Breath {
    pub o2_g_day: f64,
    pub co2_g_day: f64,
    pub water_g_day: f64,
}

impl LifeSupportData {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut d: Self = ron::from_str(text).map_err(|e| e.to_string())?;
        d.median = d.median_exchange();
        // The first row naming a plant is its row.
        for row in &d.crops {
            for p in &row.plants {
                d.by_plant.entry(p.clone()).or_insert_with(|| row.exchange());
            }
        }
        Ok(d)
    }

    /// The data folder's copy first (so it can be modded), the shipped copy if
    /// that is missing or does not parse. The shipped copy always parses: a
    /// test pins it.
    pub fn load() -> Self {
        let path = crate::data_dir().join("life_support.ron");
        if let Ok(text) = std::fs::read_to_string(&path) {
            match Self::parse(&text) {
                Ok(d) => return d,
                Err(e) => log::warn!("[LifeSupport] {} does not parse ({e}); using the shipped copy", path.display()),
            }
        }
        Self::parse(LIFE_SUPPORT_RON).expect("the shipped data/life_support.ron parses")
    }

    /// The exchange a crop that NASA did not measure is given: the median of
    /// the measured rows, each figure on its own.
    pub fn default_exchange(&self) -> Exchange {
        self.median
    }

    /// The median of the measured rows (`parse` keeps it).
    fn median_exchange(&self) -> Exchange {
        let rows: Vec<Exchange> = self.crops.iter().map(CropGas::exchange).collect();
        let median = |f: fn(&Exchange) -> f64| {
            let mut v: Vec<f64> = rows.iter().map(f).collect();
            v.sort_by(f64::total_cmp);
            match v.len() {
                0 => 0.0,
                n if n % 2 == 1 => v[n / 2],
                n => (v[n / 2 - 1] + v[n / 2]) / 2.0,
            }
        };
        Exchange { co2_g_per_l: median(|e| e.co2_g_per_l), o2_g_per_l: median(|e| e.o2_g_per_l), kept_l_per_l: median(|e| e.kept_l_per_l) }
    }

    /// The exchange of plants.csv crop `plant`: its measured row, or the median.
    pub fn exchange_for(&self, plant: &str) -> Exchange {
        self.by_plant.get(plant).copied().unwrap_or(self.median)
    }

    /// The share, 0..1, of its reference uptake a crop manages in air holding
    /// `ppm` of carbon dioxide: Kimball 1983's gain per doubling as a
    /// logarithmic law from `co2_doubling_base_ppm`, normalised to 1 at the
    /// reference, never above it and never below 0 (life_support.ron).
    pub fn co2_factor(&self, ppm: f64) -> f64 {
        let b = self.co2_gain_per_doubling.max(0.0) / std::f64::consts::LN_2;
        let base = self.co2_doubling_base_ppm.max(1e-9);
        let at = |c: f64| 1.0 + b * (c.max(1e-9) / base).ln();
        let r = at(self.co2_reference_ppm);
        if !(r > 0.0) || !ppm.is_finite() {
            return 0.0;
        }
        (at(ppm) / r).clamp(0.0, 1.0)
    }

    /// What a household burning `kcal_per_day` of food breathes: the reference
    /// crew member's figures scaled by the food energy (life_support.ron).
    pub fn breath(&self, kcal_per_day: f64) -> Breath {
        let r = (kcal_per_day.max(0.0) * self.kj_per_kcal / 1000.0) / self.reference_mj_per_day.max(1e-9);
        Breath {
            o2_g_day: self.reference_o2_kg * r * 1000.0,
            co2_g_day: self.reference_co2_kg * r * 1000.0,
            water_g_day: self.reference_water_kg * r * 1000.0,
        }
    }

    /// An air handler's draw, as a share of its full power, at `share` of its
    /// full airflow: along the catalogue's speed points, cycling on the lowest
    /// speed below it, nothing at rest (life_support.ron, ITS FAN).
    pub fn fan_power_share(&self, share: f64) -> f64 {
        let s = share.clamp(0.0, 1.0);
        let pts = &self.air_handler_speeds;
        let Some(&(f0, p0)) = pts.first() else { return s };
        if s <= 0.0 {
            return 0.0;
        }
        if s <= f0 {
            return p0 * s / f0.max(1e-9);
        }
        for w in pts.windows(2) {
            let ((fa, pa), (fb, pb)) = (w[0], w[1]);
            if s <= fb {
                return pa + (pb - pa) * (s - fa) / (fb - fa).max(1e-9);
            }
        }
        pts.last().map_or(s, |p| p.1)
    }
}

/// DataStore key of the Settings mode, `Mutex<bool>`: true is Realistic (the
/// home's grid powers the air handlers and the CO2 scrubber), false is
/// Station-supplied (the station's own plant does). The air, the water and
/// the carbon are modelled the same either way: the simplified mode only takes
/// the machines' draw off the home's grid.
pub const MODE_KEY: &str = "life_support_realistic";

/// Register the data and the mode (lib.rs, at startup). The mode starts
/// Station-supplied, the softened default of the house rule for deep systems
/// (CLAUDE.md, "Dual modes"); Settings > Ship life support switches it.
pub fn register(data_store: &mut DataStore) {
    data_store.insert(DATA_KEY, LifeSupportData::load());
    data_store.insert(MODE_KEY, std::sync::Mutex::new(false));
}

/// Publish the Settings mode (lib.rs, each frame).
pub fn publish(data: &DataStore, realistic: bool) {
    if let Some(Ok(mut v)) = data.get::<std::sync::Mutex<bool>>(MODE_KEY).map(|m| m.lock()) {
        *v = realistic;
    }
}

/// Does the home's grid power its air machines? Absent (headless tests) is
/// Realistic: the physics the tests measure.
pub fn is_realistic(data: &DataStore) -> bool {
    data.get::<std::sync::Mutex<bool>>(MODE_KEY).and_then(|m| m.lock().ok().map(|v| *v)).unwrap_or(true)
}

// -- Gases -------------------------------------------------------------------------

/// Grams per m3 of a pure gas of `molar_mass` at `t_c` and the air pressure
/// humidity.ron gives: the ideal gas law, P M / (R T).
pub fn pure_g_m3(hd: &HumidityData, molar_mass: f64, t_c: f64) -> f64 {
    hd.air_pressure_kpa * 1000.0 * molar_mass / (hd.molar_gas_constant * (t_c + 273.15))
}

/// Parts per million by volume of carbon dioxide held at `g_m3`.
pub fn co2_ppm(hd: &HumidityData, g_m3: f64, t_c: f64) -> f64 {
    g_m3 / pure_g_m3(hd, hd.co2_molar_mass, t_c).max(1e-9) * 1e6
}

/// Grams per m3 of carbon dioxide at `ppm`.
pub fn co2_g_m3(hd: &HumidityData, ppm: f64, t_c: f64) -> f64 {
    ppm.max(0.0) * 1e-6 * pure_g_m3(hd, hd.co2_molar_mass, t_c)
}

/// Grams of water vapour per m3 of air at `t_c` that has passed a coil: what
/// saturated air holds at the coil's dew point, at the room's temperature (the
/// vapour's pressure is kept as the air warms back up).
pub fn coil_vapour(hd: &HumidityData, ld: &LifeSupportData, t_c: f64) -> f64 {
    let e_pa = hd.saturation_kpa(ld.coil_dewpoint_c) * 1000.0;
    e_pa / (hd.vapour_gas_constant() * (t_c + 273.15)) * 1000.0
}

/// Dry air's density, kg/m3, at `t_c`: FAO-56's P / (R T) with dry air's
/// gas constant (humidity.ron).
pub fn air_kg_m3(hd: &HumidityData, t_c: f64) -> f64 {
    hd.air_pressure_kpa * 1000.0 / (hd.dry_air_gas_constant * (t_c + 273.15))
}

// -- The exact balance ---------------------------------------------------------------

/// One gas in one air after a slice, from `relax`: where it ends, its time
/// integral over the slice (per m3, so each sink's share follows), what went
/// over the ceiling (condensed on the walls, per m3) and what a negative
/// source could not take because the air ran out (per m3).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Relaxed {
    pub x: f64,
    pub integral: f64,
    pub over: f64,
    pub short: f64,
}

impl Relaxed {
    /// What sink (k, u) took over the slice, per m3: k times the integral of
    /// (x - u).
    pub fn to_sink(&self, k: f64, u: f64, hours: f64) -> f64 {
        k * (self.integral - u * hours)
    }
}

/// Solve `dx/dt = s - SUM k_j (x - u_j)` exactly over `hours`, from `x0`,
/// holding x inside [0, `ceiling`]. Anything pushed above the ceiling goes
/// into `over`; anything a negative source would have taken below zero is
/// `short` (it did not happen). So, exactly:
/// x - x0 = s hours - SUM k_j (integral - u_j hours) - over + short.
pub fn relax(x0: f64, s: f64, sinks: &[(f64, f64)], hours: f64, ceiling: f64) -> Relaxed {
    let ceiling = if ceiling.is_finite() { ceiling.max(0.0) } else { f64::INFINITY };
    let mut x0 = if x0.is_finite() { x0 } else { 0.0 };
    let mut out = Relaxed::default();
    // A start outside the bounds (a save at another temperature) is brought in
    // and counted, so the ledger still closes.
    if x0 > ceiling {
        out.over += x0 - ceiling;
        x0 = ceiling;
    }
    if x0 < 0.0 {
        out.short -= x0;
        x0 = 0.0;
    }
    if !(hours > 0.0) {
        out.x = x0;
        return out;
    }
    let k: f64 = sinks.iter().map(|(k, _)| k.max(0.0)).sum();
    let m: f64 = s + sinks.iter().map(|(k, u)| k.max(0.0) * u).sum::<f64>();
    // The free motion from x0 for t hours: where it is, and its integral.
    let free = |t: f64| -> (f64, f64) {
        if k > 1e-12 {
            let eq = m / k;
            let e = (-k * t).exp();
            (eq + (x0 - eq) * e, eq * t + (x0 - eq) * (1.0 - e) / k)
        } else {
            (x0 + m * t, x0 * t + 0.5 * m * t * t)
        }
    };
    // When the free motion would reach `bound`, if within the slice.
    let reach = |bound: f64| -> Option<f64> {
        let t = if k > 1e-12 {
            let eq = m / k;
            let a = (eq - x0) / (eq - bound);
            (a > 1.0).then(|| a.ln() / k)?
        } else if m.abs() > 1e-15 {
            let t = (bound - x0) / m;
            (t > 0.0).then_some(t)?
        } else {
            return None;
        };
        (t < hours).then_some(t)
    };
    // The rate x would move at if held at `at` (m - k at): >0 pushes past a
    // ceiling, <0 pulls under the floor.
    let push = |at: f64| m - k * at;
    if x0 >= ceiling && push(ceiling) >= 0.0 {
        out.x = ceiling;
        out.integral = ceiling * hours;
        out.over += push(ceiling) * hours;
        return out;
    }
    if x0 <= 0.0 && push(0.0) <= 0.0 {
        out.x = 0.0;
        out.short += -push(0.0) * hours;
        return out;
    }
    // Which bound the free motion heads past, if any: toward its equilibrium
    // m / k, or along its slope m when nothing exchanges.
    let bound = if k > 1e-12 {
        let eq = m / k;
        if eq > ceiling {
            Some(ceiling)
        } else if eq < 0.0 {
            Some(0.0)
        } else {
            None
        }
    } else if m > 0.0 && ceiling.is_finite() {
        Some(ceiling)
    } else if m < 0.0 {
        Some(0.0)
    } else {
        None
    };
    match bound.and_then(|b| reach(b).map(|t| (b, t))) {
        Some((bound, t)) => {
            let (_, i) = free(t);
            let rest = hours - t;
            out.x = bound;
            out.integral = i + bound * rest;
            if bound > 0.0 {
                out.over += push(bound) * rest;
            } else {
                out.short += -push(0.0) * rest;
            }
        }
        None => {
            let (x, i) = free(hours);
            out.x = x.clamp(0.0, ceiling);
            out.integral = i;
        }
    }
    out
}

/// How long, in hours, the free motion of `relax` from `x0` takes to reach
/// `target`, if it ever does (it heads toward its equilibrium, or along its
/// slope when nothing exchanges).
pub fn time_to(x0: f64, s: f64, sinks: &[(f64, f64)], target: f64) -> Option<f64> {
    let k: f64 = sinks.iter().map(|(k, _)| k.max(0.0)).sum();
    let m: f64 = s + sinks.iter().map(|(k, u)| k.max(0.0) * u).sum::<f64>();
    if (x0 - target).abs() < 1e-15 {
        return Some(0.0);
    }
    if k > 1e-12 {
        let eq = m / k;
        let a = (eq - x0) / (eq - target);
        (a > 1.0 && a.is_finite()).then(|| a.ln() / k)
    } else if m.abs() > 1e-15 {
        let t = (target - x0) / m;
        (t > 0.0).then_some(t)
    } else {
        None
    }
}

/// The share, 0..1, a machine that pulls an air's `x` toward `target` (a
/// coil's vapour, a scrubber's zero) runs at to hold the air at `set`: flat
/// out above it; at it (down to `HOLD_BAND` of it) the share whose balance
/// settles exactly on it; idle below. `gain` is the machine's pull an hour at
/// full share (per m3); `load` is what the rest of the air's balance adds an
/// hour when the air sits at `set` (its sources less its other sinks).
pub fn pull_share(x: f64, set: f64, gain: f64, target: f64, load: f64) -> f64 {
    if !(gain > 0.0) || !(set > target) {
        return 0.0;
    }
    if x > set {
        return 1.0;
    }
    if x < set * HOLD_BAND {
        return 0.0;
    }
    (load / (gain * (set - target))).clamp(0.0, 1.0)
}

// -- The machines -------------------------------------------------------------------

/// A placed air handler or scrubber the air step can drive: its entity, the
/// air it stands in (a room index, or None for the home's own air), its full
/// capacity (m3/h of air, or kg/day of CO2 at the rated inlet), its full draw,
/// and whether it has power.
#[derive(Debug, Clone, Copy)]
pub struct Unit {
    pub entity: hecs::Entity,
    pub room: Option<usize>,
    pub capacity: f64,
    pub watts: f64,
    pub powered: bool,
}

/// What the home's own air hands the caller from one step: each of its air
/// handlers' condensate, litres a day, for the plumbing. (The rest, what the
/// scrubbers vent and the gases the air traded, is kept on `HomeAirState`.)
#[derive(Debug, Clone, Default)]
pub struct HomeOut {
    pub condensate_by_entity: HashMap<hecs::Entity, f64>,
}

/// The home's own air, the inputs one step needs besides its state.
#[derive(Debug, Clone, Default)]
pub struct HomeInputs {
    /// Its volume (the sealed home less the grow rooms that keep their own
    /// air), m3, and its temperature, C.
    pub volume_m3: f64,
    pub temp_c: f64,
    /// The household's food energy, kcal a day (their breath).
    pub kcal_per_day: f64,
    /// The home's modules for the leak (its rooms).
    pub modules: f64,
    /// Grams of vapour and of carbon dioxide the grow rooms passed to it over
    /// the step (their leakage and fans), and litres a day breathed out by
    /// crops standing in its own air, with their uptake at the reference
    /// (grams of CO2 and O2 a day, light included).
    pub vapour_in_g: f64,
    pub co2_in_g: f64,
    pub breathed_l_day: f64,
    pub photo_co2_g_day: f64,
    pub photo_o2_g_day: f64,
    /// Oxygen the grow rooms' crops gave out and their mushrooms took in over
    /// the step, grams.
    pub o2_made_g: f64,
    pub o2_used_g: f64,
    /// Carbon dioxide the grow rooms' mushrooms breathed out and crops took
    /// up over the step, grams (the day's loop line; the gas itself reaches
    /// the home through `co2_in_g`).
    pub co2_out_rooms_g: f64,
    pub co2_uptake_rooms_g: f64,
    /// The home's grid powers its air machines (Settings: Realistic); false
    /// has the station's plant do it, and their draw on the home is 0.
    pub realistic: bool,
}

/// Step the home's own air `hours` game hours: vapour (sources: the rooms'
/// exhaust, its crops, the people; sinks: its air handlers, the leak), carbon
/// dioxide (sources: the rooms, the people; sinks: its crops, its scrubbers,
/// the leak) and oxygen (the crops' out, the people's and mushrooms' in, the
/// leak). Sets its machines' draw and returns their condensate. Its water goes
/// into `state.ledger`.
#[allow(clippy::too_many_arguments)]
pub fn step_home(
    world: &mut hecs::World,
    hd: &HumidityData,
    ld: &LifeSupportData,
    state: &mut crate::ecs::components::HomeAirState,
    inp: &HomeInputs,
    handlers: &[Unit],
    scrubbers: &[Unit],
    hours: f64,
) -> HomeOut {
    let mut out = HomeOut::default();
    let v = inp.volume_m3.max(1.0);
    let t = inp.temp_c;
    let sat = hd.vapour_at(1.0, t);
    let breath = ld.breath(inp.kcal_per_day);
    // The leak overboard, a share of the air an hour.
    let leak_kg_day = ld.leak_kg_per_day_per_module.max(0.0) * inp.modules.max(0.0);
    let f_leak = leak_kg_day / (air_kg_m3(hd, t) * v) / 24.0;
    // What each source adds an hour, per m3.
    let h = hours.max(0.0);
    let per_h = |g_day: f64| g_day / 24.0 / v;
    let from_rooms = |g: f64| if h > 0.0 { g / h / v } else { 0.0 };
    let s_v = from_rooms(inp.vapour_in_g) + per_h(inp.breathed_l_day * 1000.0 + breath.water_g_day);
    let v_coil = coil_vapour(hd, ld, t);
    let hq: f64 = handlers.iter().filter(|u| u.powered).map(|u| u.capacity).sum();
    let a_max = hq / v;
    let set_v = ld.home_setpoint_rh.clamp(0.0, 1.0) * sat;
    let load_v = s_v - f_leak * set_v;
    let share_v = pull_share(state.vapour_g_m3, set_v, a_max, v_coil, load_v);
    // What a handler the electrical sim has shed would run at with power:
    // its request (2026-09-27), the controller's share with every unit.
    let a_all = handlers.iter().map(|u| u.capacity).sum::<f64>() / v;
    let share_v_req = pull_share(state.vapour_g_m3, set_v, a_all, v_coil, load_v);
    let a = share_v * a_max;
    let rv = relax(state.vapour_g_m3, s_v, &[(a, v_coil), (f_leak, 0.0)], h, sat);
    let condensed_g = rv.to_sink(a, v_coil, h) * v;
    let leaked_v_g = rv.to_sink(f_leak, 0.0, h) * v;
    state.ledger.breathed_l += inp.breathed_l_day * h / 24.0;
    state.ledger.people_l += breath.water_g_day * h / 24.0 / 1000.0;
    state.ledger.condensed_l += condensed_g / 1000.0;
    state.ledger.leaked_l += leaked_v_g / 1000.0;
    state.ledger.surface_l += rv.over * v / 1000.0;
    // A negative share of the source cannot occur for vapour; `short` is only
    // a start below zero.
    state.ledger.breathed_l += rv.short * v / 1000.0;
    state.vapour_g_m3 = rv.x;
    state.air_handler = share_v;
    // Litres a day the coil condenses at the end of the step, split by airflow.
    let rate_l_day = a * (rv.x - v_coil).max(0.0) * v * 24.0 / 1000.0;
    state.condensate_l_day = rate_l_day;
    for u in handlers.iter().filter(|u| u.powered) {
        out.condensate_by_entity.insert(u.entity, if hq > 0.0 { rate_l_day * u.capacity / hq } else { 0.0 });
    }
    // Every handler's draw, powered or not (2026-09-27): a powered one draws
    // along its fan's curve, a shed one asks for what it would draw with
    // power, so the island sees its request and powers it again when it can.
    // In the Station-supplied mode the station's plant powers them: 0 W on
    // the home's grid, which the electrical sim never sheds. (Writing only the
    // powered ones left a shed handler asking for its spawn nameplate, a
    // phantom 325 W on the home's grid in the Station-supplied mode.)
    for u in handlers {
        if let Ok(mut pc) = world.get::<&mut crate::ecs::components::PowerConsumer>(u.entity) {
            let share = if u.powered { share_v } else { share_v_req };
            pc.draw_watts = if inp.realistic { (u.watts * ld.fan_power_share(share)) as f32 } else { 0.0 };
        }
    }

    // Carbon dioxide.
    let ppm0 = co2_ppm(hd, state.co2_g_m3, t);
    let uptake_g_h = inp.photo_co2_g_day / 24.0 * ld.co2_factor(ppm0);
    let s_c = from_rooms(inp.co2_in_g) + (breath.co2_g_day / 24.0 - uptake_g_h) / v;
    let rated_g_m3 = co2_g_m3(hd, ld.scrubber_rated_ppm, t).max(1e-9);
    let sc_g_h: f64 = scrubbers.iter().filter(|u| u.powered).map(|u| u.capacity * 1000.0 / 24.0).sum();
    let b_max = sc_g_h / rated_g_m3 / v;
    let set_c = co2_g_m3(hd, ld.scrubber_setpoint_ppm, t);
    let load_c = s_c - f_leak * set_c;
    let share_c = pull_share(state.co2_g_m3, set_c, b_max, 0.0, load_c);
    let b_all = scrubbers.iter().map(|u| u.capacity * 1000.0 / 24.0).sum::<f64>() / rated_g_m3 / v;
    let share_c_req = pull_share(state.co2_g_m3, set_c, b_all, 0.0, load_c);
    let b = share_c * b_max;
    let rc = relax(state.co2_g_m3, s_c, &[(b, 0.0), (f_leak, 0.0)], h, f64::INFINITY);
    let taken_home_g = uptake_g_h * h - rc.short * v;
    state.co2_g_m3 = rc.x;
    state.scrubber = share_c;
    state.scrubbed_kg_day = b * rc.x * v * 24.0 / 1000.0;
    // Every scrubber's draw, powered or not, the same way (2026-09-27): above
    // its setpoint a shed scrubber asks for its full 860 W, and as a
    // priority-1 load it is fed before the optional ones.
    for u in scrubbers {
        if let Ok(mut pc) = world.get::<&mut crate::ecs::components::PowerConsumer>(u.entity) {
            let share = if u.powered { share_c } else { share_c_req };
            pc.draw_watts = if inp.realistic { (u.watts * share) as f32 } else { 0.0 };
        }
    }

    // Oxygen: the crops' own (its share of what they could fix that they did)
    // and the rooms', less the people's and the mushrooms', less the leak.
    let fixed_share = if uptake_g_h * h > 0.0 { (taken_home_g / (uptake_g_h * h)).clamp(0.0, 1.0) } else { 0.0 };
    let o2_home_g = inp.photo_o2_g_day / 24.0 * ld.co2_factor(ppm0) * h * fixed_share;
    let o2_in = o2_home_g + inp.o2_made_g - inp.o2_used_g - breath.o2_g_day / 24.0 * h;
    let o2_leak = state.o2_g_m3 * (1.0 - (-f_leak * h).exp());
    state.o2_g_m3 = (state.o2_g_m3 + o2_in / v - o2_leak).max(0.0);

    // The day's loops, for the panel.
    if h > 0.0 {
        let per_day = 24.0 / h / 1000.0;
        state.co2_out_kg_day = (breath.co2_g_day / 24.0 * h + inp.co2_out_rooms_g) * per_day;
        state.co2_uptake_kg_day = (taken_home_g.max(0.0) + inp.co2_uptake_rooms_g) * per_day;
        state.o2_made_kg_day = (o2_home_g + inp.o2_made_g) * per_day;
        state.o2_used_kg_day = (inp.o2_used_g + breath.o2_g_day / 24.0 * h) * per_day;
    }
    state.leak_kg_day = leak_kg_day;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> LifeSupportData {
        LifeSupportData::parse(LIFE_SUPPORT_RON).expect("the shipped life_support.ron parses")
    }

    fn air() -> HumidityData {
        HumidityData::parse(crate::systems::farming::humidity::HUMIDITY_RON).unwrap()
    }

    /// The balance is exact whatever the slice: one long step and many short
    /// ones land in the same place, and in each the change equals the sources
    /// less what every sink took, less what went over the ceiling, plus what a
    /// negative source could not take. Seen red by not counting what goes over
    /// the ceiling partway through a slice (the third case's budget then missed
    /// what condensed).
    #[test]
    fn the_balance_is_exact_and_slicing_does_not_change_it() {
        let cases: [(f64, f64, [(f64, f64); 2], f64); 6] = [
            (6.0, 2.0, [(0.5, 7.0), (0.3, 5.0)], 18.0),  // settles below the ceiling
            (6.0, 9.0, [(0.5, 7.0), (0.3, 5.0)], 18.0),  // settles just under it
            (6.0, 12.0, [(0.5, 7.0), (0.3, 5.0)], 18.0), // reaches it about 1.9 h in
            (18.0, 9.0, [(0.5, 7.0), (0.0, 0.0)], 18.0), // starts on it, stays
            (5.0, -8.0, [(0.1, 1.0), (0.0, 0.0)], 1e9),  // a sink that empties it
            (5.0, 1.0, [(0.0, 0.0), (0.0, 0.0)], 1e9),   // nothing exchanges
        ];
        for (x0, s, sinks, ceil) in cases {
            let h = 3.0;
            let one = relax(x0, s, &sinks, h, ceil);
            let took: f64 = sinks.iter().map(|(k, u)| one.to_sink(*k, *u, h)).sum();
            let budget = x0 + s * h - took - one.over + one.short;
            assert!((one.x - budget).abs() < 1e-9, "{x0} {s}: {} vs budget {budget} ({one:?})", one.x);
            let mut x = x0;
            for _ in 0..3000 {
                x = relax(x, s, &sinks, h / 3000.0, ceil).x;
            }
            assert!((x - one.x).abs() < 1e-6, "{x0} {s}: sliced {x} vs one {}", one.x);
        }
    }

    /// Kimball's law as the data reads it: the reference is 1, 400 ppm about
    /// 69%, a doubling from the base 33% more, and it is zero near 40 ppm and
    /// never above 1. Seen red by dropping the normalisation to the reference
    /// (400 ppm then read the cap, 1.0).
    #[test]
    fn crops_take_up_less_carbon_dioxide_in_thin_air() {
        let d = data();
        assert!((d.co2_factor(1100.0) - 1.0).abs() < 1e-12);
        assert!((d.co2_factor(400.0) - 0.694).abs() < 0.001, "{}", d.co2_factor(400.0));
        let gain = d.co2_factor(660.0) / d.co2_factor(330.0);
        assert!((gain - 1.33).abs() < 1e-9, "a doubling from 330: {gain}");
        assert_eq!(d.co2_factor(3000.0), 1.0);
        assert_eq!(d.co2_factor(30.0), 0.0);
    }

    /// The crops NASA measured, per litre breathed out: lettuce 5.1 g of CO2,
    /// wheat 6.5, potato 11.3; tissue keeps lettuce 6.3%, wheat 2.3%, tomato
    /// 10.0%; a crop not in the table takes the medians. Every row's plants
    /// are real plants.csv ids.
    #[test]
    fn crop_exchange_follows_the_bvad_tables() {
        let d = data();
        let e = |p: &str| d.exchange_for(p);
        assert!((e("lettuce").co2_g_per_l - 5.095).abs() < 0.01 && (e("wheat").co2_g_per_l - 6.531).abs() < 0.01);
        assert!((e("potato").co2_g_per_l - 11.31).abs() < 0.01);
        assert!((e("lettuce").kept_l_per_l - 0.0625).abs() < 1e-3 && (e("wheat").kept_l_per_l - 0.0231).abs() < 1e-3);
        assert!((e("tomato").kept_l_per_l - 0.1004).abs() < 1e-3);
        assert_eq!(e("basil"), d.default_exchange());
        let m = d.default_exchange();
        assert!(m.co2_g_per_l > 9.0 && m.co2_g_per_l < 14.0 && m.kept_l_per_l > 0.04 && m.kept_l_per_l < 0.08, "{m:?}");
        let csv = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("plants.csv")).unwrap();
        let ids: Vec<&str> = csv.lines().filter(|l| !l.starts_with('#')).skip(1).filter_map(|l| l.split(',').next()).collect();
        for row in &d.crops {
            for p in &row.plants {
                assert!(ids.contains(&p.as_str()), "{}: {p} is not a plants.csv id", row.name);
            }
        }
    }

    /// The household breathes in proportion to its food: 2,600 kcal gives
    /// home_outline.json's 0.77 kg of oxygen and 0.92 of carbon dioxide, and
    /// the reference 11.82 MJ gives Hanford 2004's own figures back.
    #[test]
    fn a_household_breathes_what_it_eats() {
        let d = data();
        let b = d.breath(2600.0);
        assert!((b.o2_g_day - 768.0).abs() < 2.0 && (b.co2_g_day - 918.0).abs() < 2.0, "{b:?}");
        let r = d.breath(11820.0 / 4.184);
        assert!((r.o2_g_day - 835.0).abs() < 0.5 && (r.co2_g_day - 998.0).abs() < 0.5 && (r.water_g_day - 2277.0).abs() < 1.0);
    }

    /// The fan's draw follows the catalogue's three speeds and cycles on Low
    /// below it. Seen red by the fan laws' cube (half speed then drew 12%, the
    /// catalogue's permanent split capacitor motors draw 53% cycling on Low).
    #[test]
    fn the_air_handler_draws_along_its_catalogue_curve() {
        let d = data();
        assert_eq!(d.fan_power_share(0.0), 0.0);
        assert!((d.fan_power_share(1.0) - 1.0).abs() < 1e-12);
        assert!((d.fan_power_share(0.841) - 0.818).abs() < 1e-9 && (d.fan_power_share(0.658) - 0.695).abs() < 1e-9);
        assert!((d.fan_power_share(0.329) - 0.3475).abs() < 1e-9, "half of Low cycles half the time");
    }

    /// The coil's air at the home's 20 C holds about 40% humidity: why the
    /// home's setpoint is 50, not the ISS's 40 (life_support.ron).
    #[test]
    fn the_coil_dries_air_to_its_dew_point() {
        let (hd, d) = (air(), data());
        let rh = coil_vapour(&hd, &d, 20.0) / hd.vapour_at(1.0, 20.0);
        assert!((rh - 0.397).abs() < 0.002, "{rh}");
        // At the coil's own temperature the air is saturated.
        assert!((coil_vapour(&hd, &d, d.coil_dewpoint_c) - hd.vapour_at(1.0, d.coil_dewpoint_c)).abs() < 1e-9);
    }
}
