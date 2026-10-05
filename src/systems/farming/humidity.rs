//! Greenhouse humidity (2026-09-26, gardening depth: the air the crops grow
//! in).
//!
//! Until this rung the grow rooms had no air of their own: the one home air
//! space (systems::atmosphere) holds 40% forever, and nothing the crops did
//! reached it. Every number and its source live in data/garden/humidity.ron;
//! this file is the model.
//!
//! THE BALANCE, per grow room (a room some grow machine stands in):
//!
//! ```text
//! dv/dt = S - n (v - v_home)          v: g of water vapour per m3 of the room
//! S     = breathed (L/day) x vapour_share x 1000 / 24 / volume   (g/m3 an hour)
//! n     = base_air_changes_per_hour + fan speed x fans' m3/h / volume
//! ```
//!
//! solved exactly over each tick (`step_vapour`), so a frame, a clock jump and
//! a catch-up agree, and capped at saturation (the rest condenses). The room
//! settles at `v_home + S / n`: the crops' water over the air that carries it
//! away. Relative humidity is `v` over what saturated air at the room's
//! temperature holds, by the FAO-56 formula (`HumidityData::saturation_kpa`).
//!
//! WHO BREATHES: the growing, watered crops of each grow area in the room, at
//! plants.csv water_liters_per_day times the plants in the unit (the same
//! litres the irrigation bills; a thirsty crop has closed its stomata and a
//! ripe one no longer draws water). The farming tick tallies them. The day's
//! water is breathed evenly, day and night: a real crop breathes most of it
//! in daylight, and the rooms have no night cooling yet to make that matter.
//!
//! WHERE THE ROOMS ARE: the engine publishes the home's room boxes (lib.rs,
//! `publish_rooms`, from the room bounds) and the grow machines' positions
//! ("grow_plots", lighting::GrowPlot); a grow area belongs to the smallest
//! room box its machine stands in. Outdoor fields breathe into the weather,
//! and use the weather's humidity. A crop in no known room (hand planted, or a
//! machine outside every room) grows in the home's air.
//!
//! ENCLOSURES (a mushroom rack's fruiting tent): a grow machine whose medium
//! has an `enclosure` grows in a small room of its own, "tent:<machine>",
//! centred on it. The tent exchanges air with the room it stands in (the
//! home's, if none), at the fresh air its substrate's CO2 needs
//! (`HumidityData::tent_fresh_air_m3_h`) over its volume, and what it vents is
//! a source of vapour for that room, stepped after it (`step_rooms`).
//!
//! THE FANS: an exhaust fan (`Ventilator`) standing in a room exchanges its air
//! with the home's while its PowerConsumer is enabled. Its controller runs it
//! flat out above `fan_setpoint_rh`, at the speed that holds the setpoint once
//! there, and idle below (`fan_speed`); it draws its watts times the cube of
//! its speed (the fan laws), which this step writes into its PowerConsumer.
//!
//! THE HUMIDIFIERS (the mushroom room): a `Humidifier` standing in a room
//! adds vapour while its PowerConsumer is enabled and the home has water for
//! the garden (the tanks not dry and the irrigation running, the same gate the
//! crops' water has). Its controller runs it flat out below
//! `humidifier_setpoint_rh`, at the output that holds the setpoint once there
//! (`humidifier_share`), and off above it; its draw is its watts times its
//! output share, and the litres it puts in are billed to the home's tanks with
//! the irrigation (`step_rooms` returns them). In a room a humidifier holds,
//! the fans work `humidified_fan_margin_rh` above its setpoint so the two
//! never fight, and the damp-disease notice is not given: the damp is meant.
//!
//! SHIP LIFE SUPPORT (2026-09-26, systems::life_support, data/life_support.ron):
//! with that data registered, the same step also runs each room's carbon
//! dioxide (its crops' uptake, its tents' substrate, its scrubbers), its air
//! handlers (a cold coil that condenses the room's water and hands it back to
//! the tanks), and after the rooms the home's own air, whose vapour and carbon
//! dioxide the rooms exchange with. Every exchange is solved exactly
//! (`life_support::relax`), so the air's water ledger (`HomeAirState::ledger`)
//! closes to the gram.
//!
//! WHAT IT DRIVES: the diseases' Humidity condition (pests.rs) and a health
//! cap on a crop outside its plants.csv humidity window (`health_ceiling`),
//! never below the house floor: gentle for a green crop, steep for a fungus
//! (plants.csv `needs_light` false) below its fruiting range.
//!
//! THE HEAT (2026-10-05, BUG-155, farming::heat, humidity.ron THE HEAT): a
//! space heater standing in a room warms its air, which is then at
//! `room_temp_c` plus that warmth (`room_temp_at`), and everything above that
//! reads a room's temperature reads it there: what saturated air holds, and so
//! its relative humidity and every humidity setpoint, its carbon dioxide's ppm
//! and its coils. The step below steps each air's warmth with its water and
//! gases, and passes a room's heat on to the air around it as it does its
//! vapour.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Deserialize;

use crate::ecs::components::{
    CropInstance, HomeAirState, Humidifier, PowerConsumer, RoomAir, SoilMemory, Transform, Ventilator, STAGE_DEAD,
};
use crate::hot_reload::data_store::DataStore;
use crate::systems::life_support::{self, LifeSupportData};

use super::lighting::GrowPlot;
use super::pests::{Condition, PestData};
use super::PlantDef;

/// The shipped copy, so a bare exe with no data folder still has the model.
pub const HUMIDITY_RON: &str = include_str!("../../../data/garden/humidity.ron");

/// DataStore key of the loaded `HumidityData` (lib.rs registers it).
pub const DATA_KEY: &str = "garden_humidity";

/// DataStore key of the home's room boxes, `Mutex<Vec<GrowRoom>>`, kept
/// current by lib.rs through `publish_rooms`.
pub const ROOMS_KEY: &str = "garden_room_boxes";

/// A fan's controller counts the room as at its setpoint down to this share of
/// it, and holds its speed there; below it the fan idles. A small band keeps a
/// fan from flicking on and off every frame.
const HOLD_BAND: f64 = 0.98;

// -- Data: data/garden/humidity.ron -----------------------------------------------

/// What data/garden/humidity.ron holds. Its comments carry every source.
#[derive(Debug, Clone, Deserialize)]
pub struct HumidityData {
    pub svp_a_kpa: f64,
    pub svp_b: f64,
    pub svp_c_c: f64,
    pub dry_air_gas_constant: f64,
    pub vapour_weight_ratio: f64,
    pub room_temp_c: f64,
    pub vapour_share: f64,
    pub base_air_changes_per_hour: f64,
    pub fan_setpoint_rh: f64,
    pub fan_power_exponent: f64,
    pub humidifier_setpoint_rh: f64,
    pub humidified_fan_margin_rh: f64,
    pub notice_above_rh: f64,
    pub stress_loss_max: f64,
    pub stress_full_at_rh: f64,
    pub fungi_loss_full_at_rh: f64,
    pub health_floor: f32,
    pub substrate_co2_g_kg_h: f64,
    pub fruiting_co2_limit_ppm: f64,
    pub intake_co2_ppm: f64,
    pub co2_molar_mass: f64,
    pub air_pressure_kpa: f64,
    pub molar_gas_constant: f64,
    /// The share of a fruiting fungus's crop lost for every 1,000 ppm of
    /// carbon dioxide over its limit (2026-09-27, humidity.ron STALE AIR).
    pub fungi_co2_loss_per_1000_ppm: f64,
    /// What each fungus breathes out per "plant" (a block, a square foot of
    /// bed), and its own CO2 limit where sourced (humidity.ron, THE FUNGI'S
    /// BREATH).
    pub fungi: Vec<FungusBreath>,
    /// THE HEAT (2026-10-05, BUG-155, farming::heat): air's specific heat,
    /// J/(kg K); the walls' and ceiling's heat loss, W/(m2 K); and the storey
    /// the home's own air is taken to stand in, m.
    pub air_specific_heat_j_kg_k: f64,
    pub envelope_u_w_m2_k: f64,
    pub home_storey_m: f64,
}

/// One fungus's breath (2026-09-27): grams of carbon dioxide an hour from each
/// of its "plants" (plants.csv; a 5 lb block, a square foot of bed) while it
/// grows, and the CO2 above which its fruiting suffers, where a source gives
/// it its own (else `HumidityData::fruiting_co2_limit_ppm`).
#[derive(Debug, Clone, Deserialize)]
pub struct FungusBreath {
    pub plants: Vec<String>,
    pub co2_g_h_per_plant: f64,
    #[serde(default)]
    pub co2_limit_ppm: Option<f64>,
}

impl HumidityData {
    pub fn parse(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    /// The data folder's copy first (so it can be modded), the shipped copy
    /// if that is missing or does not parse. The shipped copy always parses:
    /// a test pins it.
    pub fn load() -> Self {
        let path = crate::data_dir().join("garden").join("humidity.ron");
        // Disk first; the shipped copy only when the file is missing or does not
        // parse, and then the log says so (embedded_data::note_builtin_copy: a rig
        // refuses a run that served a built-in copy, BUG-133).
        let why = match std::fs::read_to_string(&path) {
            Ok(text) => match Self::parse(&text) {
                Ok(d) => return d,
                Err(e) => format!("{} does not parse ({e})", path.display()),
            },
            Err(e) => format!("{} could not be read ({e})", path.display()),
        };
        crate::embedded_data::note_builtin_copy("garden/humidity.ron", why);
        Self::parse(HUMIDITY_RON).expect("the shipped data/garden/humidity.ron parses")
    }

    /// Saturation vapour pressure at `t_c`, kPa: FAO-56 equation 11,
    /// e°(T) = 0.6108 exp[17.27 T / (T + 237.3)].
    pub fn saturation_kpa(&self, t_c: f64) -> f64 {
        self.svp_a_kpa * (self.svp_b * t_c / (t_c + self.svp_c_c)).exp()
    }

    /// Water vapour's gas constant, J/(kg K): dry air's over the
    /// molecular-weight ratio (FAO-56 Annex 3: 287 / 0.622 = 461.4).
    pub fn vapour_gas_constant(&self) -> f64 {
        self.dry_air_gas_constant / self.vapour_weight_ratio
    }

    /// Grams of water vapour per m3 in air at relative humidity `rh` (0..1)
    /// and `t_c`: the ideal gas law, rho = e / (R_v T).
    pub fn vapour_at(&self, rh: f64, t_c: f64) -> f64 {
        let e_pa = rh.max(0.0) * self.saturation_kpa(t_c) * 1000.0;
        e_pa / (self.vapour_gas_constant() * (t_c + 273.15)) * 1000.0
    }

    /// Relative humidity (0..1, not capped) of `vapour` g/m3 at `t_c`.
    pub fn rh_of(&self, vapour: f64, t_c: f64) -> f64 {
        let sat = self.vapour_at(1.0, t_c);
        if sat > 0.0 {
            (vapour / sat).max(0.0)
        } else {
            0.0
        }
    }

    /// What saturated air holds at the grow rooms' temperature, g/m3.
    pub fn room_saturation(&self) -> f64 {
        self.vapour_at(1.0, self.room_temp_c)
    }

    /// Grams of CO2 in a cubic metre of pure CO2 at the rooms' temperature:
    /// the ideal gas law, P M / (R T).
    pub fn co2_density(&self) -> f64 {
        self.air_pressure_kpa * 1000.0 * self.co2_molar_mass / (self.molar_gas_constant * (self.room_temp_c + 273.15))
    }

    /// The fresh air, m3 an hour, a fruiting tent holding `substrate_kg` of
    /// fruiting substrate must be given to keep its CO2 at the fruiting limit:
    /// what the substrate breathes out over what each cubic metre of intake
    /// air can take up before it reaches the limit (humidity.ron, THE
    /// FRUITING TENT).
    pub fn tent_fresh_air_m3_h(&self, substrate_kg: f64) -> f64 {
        let room = (self.fruiting_co2_limit_ppm - self.intake_co2_ppm).max(1.0) * 1e-6 * self.co2_density();
        substrate_kg.max(0.0) * self.substrate_co2_g_kg_h.max(0.0) / room
    }

    /// The row of `fungi` naming plants.csv crop `plant`, if any.
    fn fungus_row(&self, plant: &str) -> Option<&FungusBreath> {
        self.fungi.iter().find(|f| f.plants.iter().any(|p| p == plant))
    }

    /// Grams of carbon dioxide an hour one "plant" of fungus `plant` breathes
    /// out while it grows (2026-09-27): its row of `fungi`, or the median of the
    /// rows for a fungus not listed (humidity.ron, THE FUNGI'S BREATH).
    pub fn fungus_co2_g_h(&self, plant: &str) -> f64 {
        if let Some(row) = self.fungus_row(plant) {
            return row.co2_g_h_per_plant.max(0.0);
        }
        let mut v: Vec<f64> = self.fungi.iter().map(|f| f.co2_g_h_per_plant.max(0.0)).collect();
        v.sort_by(f64::total_cmp);
        match v.len() {
            0 => 0.0,
            n if n % 2 == 1 => v[n / 2],
            n => (v[n / 2 - 1] + v[n / 2]) / 2.0,
        }
    }

    /// The CO2, ppm, above which fungus `plant` fruits poorly: its own limit
    /// where a source gives one, else `fruiting_co2_limit_ppm`.
    pub fn fungus_co2_limit_ppm(&self, plant: &str) -> f64 {
        self.fungus_row(plant).and_then(|f| f.co2_limit_ppm).unwrap_or(self.fruiting_co2_limit_ppm)
    }
}

// -- The balance ----------------------------------------------------------------------

/// A room's vapour after `hours`: dv/dt = `source` - `n` (v - `outside`),
/// solved exactly (v settles at outside + source / n), or growing by `source`
/// an hour when nothing exchanges its air; never above `sat` (the rest
/// condenses on the leaves and the glazing) nor below zero.
pub fn step_vapour(v0: f64, source: f64, n: f64, outside: f64, hours: f64, sat: f64) -> f64 {
    let v0 = if v0.is_finite() { v0 } else { outside };
    if !(hours > 0.0) {
        return v0.clamp(0.0, sat.max(0.0));
    }
    let v = if n > 1e-12 {
        let eq = outside + source / n;
        eq + (v0 - eq) * (-n * hours).exp()
    } else {
        v0 + source * hours
    };
    v.clamp(0.0, sat.max(0.0))
}

/// The speed, 0..1, a room's fan controller runs at, from the room's vapour
/// `v`, what the crops add (`source`, g/m3 an hour), the room's own leakage
/// `base` and its fans' full-speed air changes `fan_max` (per hour), the home
/// air's vapour and the setpoint's. Flat out above the setpoint; at it (down
/// to `HOLD_BAND` of it), the speed whose balance settles exactly on it;
/// idle below. No fans, no speed.
pub fn fan_speed(v: f64, source: f64, base: f64, fan_max: f64, outside: f64, set: f64) -> f64 {
    if !(fan_max > 0.0) {
        return 0.0;
    }
    if v > set {
        return 1.0;
    }
    if v < set * HOLD_BAND {
        return 0.0;
    }
    let room = set - outside;
    if !(room > 0.0) {
        // The home air is itself at the setpoint: a fan cannot dry below it.
        return 0.0;
    }
    ((source / room - base) / fan_max).clamp(0.0, 1.0)
}

/// The share of a slice `hours` long, 0..1, a room's CO2 fans are switched on
/// (2026-09-27, humidity.ron THE CO2 FANS): from its carbon dioxide `c0` (g/m3),
/// what its sources add (`source`, g/m3 an hour), its other air changes an hour
/// (`base`), its CO2 fans' full-speed air changes (`fan_max`), the air around
/// it and the setpoint (g/m3). An on/off controller with a small switching
/// band holds its air AT the setpoint, cycling the fan; over a slice that is
/// the one steady share that ends the slice on the setpoint (solved exactly
/// through `life_support::relax`), off when the air would end it below without
/// the fan, flat out when even that cannot bring it down. Judged once a slice
/// instead, a 384 m3/h fan (over 200 changes an hour of a 1.73 m3 tent) flushed
/// the tent to its room's air, humidity and all, and then left it to climb to
/// about 1,100 ppm the next slice.
pub fn co2_fan_duty(c0: f64, source: f64, base: f64, fan_max: f64, outside: f64, set: f64, hours: f64) -> f64 {
    if !(fan_max > 0.0) {
        return 0.0;
    }
    if !(hours > 0.0) {
        return if c0 > set { 1.0 } else { 0.0 };
    }
    let end = |duty: f64| life_support::relax(c0, source, &[(base.max(0.0) + duty * fan_max, outside)], hours, f64::INFINITY).x;
    if end(0.0) <= set {
        return 0.0;
    }
    if end(1.0) >= set {
        return 1.0;
    }
    // More fan, lower air at the slice's end: bisect for the share that lands on it.
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    for _ in 0..40 {
        let mid = 0.5 * (lo + hi);
        if end(mid) > set {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// The share, 0..1 of their full output, a room's humidifiers run at, from
/// the room's vapour `v`, what its crops add (`source`, g/m3 an hour), its air
/// changes an hour this tick (`n`: leakage and fans), the humidifiers' full
/// output `max` (g/m3 an hour; 0 when none can run), the home air's vapour and
/// the setpoint's. Flat out below the setpoint (`step_rooms` stops it on the
/// setpoint when it gets there within a tick); at it, and up to a
/// `HOLD_BAND` share above it, the output whose balance settles exactly on it
/// (none if the crops alone hold it); off once the room is further above.
pub fn humidifier_share(v: f64, source: f64, n: f64, max: f64, outside: f64, set: f64) -> f64 {
    if !(max > 0.0) {
        return 0.0;
    }
    if v < set {
        return 1.0;
    }
    if v > set * (2.0 - HOLD_BAND) {
        return 0.0;
    }
    ((n * (set - outside) - source) / max).clamp(0.0, 1.0)
}

/// The highest health a crop can hold in air at `rh` (0..1): 100 inside its
/// plants.csv humidity window, down to 100 x (1 - stress_loss_max) as the air
/// goes `stress_full_at_rh` or further outside it, never below the floor.
/// A fungus (`needs_light` false) BELOW its window loses far more: its pins
/// dry out, 100 x (1 - (d / fungi_loss_full_at_rh)^2) for d points under it
/// (humidity.ron, FUNGI). 100 for an unknown plant or one with no window.
pub fn health_ceiling(d: &HumidityData, def: Option<&PlantDef>, rh: f64) -> f32 {
    let Some(def) = def else { return 100.0 };
    let (lo, hi) = (f64::from(def.humidity_min), f64::from(def.humidity_max));
    if !(hi > 0.0) || hi < lo || !rh.is_finite() {
        return 100.0;
    }
    let off = if rh < lo {
        lo - rh
    } else if rh > hi {
        rh - hi
    } else {
        return 100.0;
    };
    let loss = if rh < lo && !def.needs_light {
        (off / d.fungi_loss_full_at_rh.max(1e-9)).powi(2).min(1.0)
    } else {
        d.stress_loss_max.clamp(0.0, 1.0) * (off / d.stress_full_at_rh.max(1e-9)).min(1.0)
    };
    ((100.0 * (1.0 - loss)) as f32).clamp(d.health_floor, 100.0)
}

/// The highest health a FUNGUS can hold in air holding `ppm` of carbon
/// dioxide (2026-09-27, humidity.ron STALE AIR): 100 up to its limit, then
/// 100 x (1 - severity x fungi_co2_loss_per_1000_ppm x (ppm - limit) / 1000),
/// never below the floor. `severity` is the garden's Off / Gentle / Realistic
/// setting (`garden_pest_severity`: 0, 0.5, 1). A green crop (plants.csv
/// `needs_light` true) is not harmed by it here: it takes the carbon dioxide
/// up (life_support.rs). 100 for an unknown plant.
pub fn co2_ceiling(d: &HumidityData, def: Option<&PlantDef>, ppm: f64, severity: f32) -> f32 {
    let Some(def) = def else { return 100.0 };
    if def.needs_light || !ppm.is_finite() {
        return 100.0;
    }
    let over = (ppm - d.fungus_co2_limit_ppm(&def.id)).max(0.0);
    let sev = f64::from(severity).clamp(0.0, 1.0);
    let loss = (sev * d.fungi_co2_loss_per_1000_ppm.max(0.0) * over / 1000.0).clamp(0.0, 1.0);
    ((100.0 * (1.0 - loss)) as f32).clamp(d.health_floor, 100.0)
}

// -- The rooms ------------------------------------------------------------------------

/// One room of the home, as the engine's room bounds give it, or a grow
/// machine's enclosure (a fruiting tent): an axis-aligned box in world metres.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GrowRoom {
    pub id: String,
    pub name: String,
    pub min: [f32; 3],
    pub max: [f32; 3],
    /// Its own air changes an hour with the air around it: a tent's fresh
    /// air over its volume. None = a room, which leaks at
    /// `base_air_changes_per_hour`.
    pub air_changes_per_hour: Option<f64>,
    /// The fruiting substrate a tent holds when full, kg (0 for a room): what
    /// its own fresh air is sized for (`HumidityData::tent_fresh_air_m3_h`).
    /// What breathes is only the fungus actually planted in it, at each
    /// species' rate (`AirStep::fungi`, 2026-09-27).
    pub substrate_kg: f64,
}

impl GrowRoom {
    /// Its air volume, m3.
    pub fn volume_m3(&self) -> f64 {
        (0..3).map(|i| f64::from((self.max[i] - self.min[i]).max(0.0))).product()
    }

    /// Does a machine at `p` stand in it? Its floor plan, and between just
    /// under its floor and just over its ceiling.
    pub fn contains(&self, p: [f32; 3]) -> bool {
        p[0] >= self.min[0]
            && p[0] <= self.max[0]
            && p[2] >= self.min[2]
            && p[2] <= self.max[2]
            && p[1] >= self.min[1] - 0.5
            && p[1] <= self.max[1] + 0.5
    }
}

/// Register the data and the room channel (lib.rs, at startup).
pub fn register(data_store: &mut DataStore) {
    data_store.insert(DATA_KEY, HumidityData::load());
    data_store.insert(ROOMS_KEY, Mutex::new(Vec::<GrowRoom>::new()));
}

/// Publish the home's rooms as (id, display name, min, max), from the
/// engine's room bounds. Called every frame; it only rebuilds the list when a
/// room changed.
pub fn publish_rooms<'a, I>(data: &DataStore, rooms: I)
where
    I: Iterator<Item = (&'a str, &'a str, [f32; 3], [f32; 3])> + Clone,
{
    let Some(Ok(mut v)) = data.get::<Mutex<Vec<GrowRoom>>>(ROOMS_KEY).map(|m| m.lock()) else { return };
    let same = v.len() == rooms.clone().count()
        && v.iter().zip(rooms.clone()).all(|(a, (id, name, min, max))| a.id == id && a.name == room_name(id, name) && a.min == min && a.max == max);
    if !same {
        *v = rooms
            .map(|(id, name, min, max)| GrowRoom { id: id.to_string(), name: room_name(id, name), min, max, air_changes_per_hour: None, substrate_kg: 0.0 })
            .collect();
    }
}

/// What to call a room in a notice. The engine's display name is the room's
/// TYPE ("Garden" for the greenhouse and the mushroom room alike), so a zone
/// id ("room-greenhouse", "room-mushroom") is read instead where there is
/// one: "Greenhouse", "Mushroom room".
fn room_name(id: &str, display: &str) -> String {
    let Some(rest) = id.strip_prefix("room-").filter(|r| !r.is_empty()) else { return display.to_string() };
    let words = rest.replace(['-', '_'], " ");
    let mut name: String = words.chars().take(1).flat_map(char::to_uppercase).chain(words.chars().skip(1)).collect();
    // A word "room", not the letters: a mushroom room is still named one.
    if !name.ends_with("house") && !name.split(' ').any(|w| w.eq_ignore_ascii_case("room")) {
        name.push_str(" room");
    }
    name
}

/// Where each grow area's air is (built once a tick): the grow rooms, which
/// room each grow area stands in, the home's air and the weather's.
#[derive(Debug, Clone, Default)]
pub struct AirMap {
    /// The rooms that hold a grow machine, the machines' enclosures (fruiting
    /// tents), and the rooms those stand in.
    pub rooms: Vec<GrowRoom>,
    /// For each of `rooms`, the room its air exchanges with: an enclosure's
    /// room (index into `rooms`), or None for the home's air.
    parent: Vec<Option<usize>>,
    /// Grow area tag (a machine instance id, or a tower design alias) ->
    /// index into `rooms`.
    area_room: HashMap<String, usize>,
    /// The home air's vapour, g/m3, and its relative humidity at its own
    /// temperature.
    pub home_vapour: f64,
    pub home_rh: f64,
    /// True when the home's air space exists (systems::atmosphere::HomeAir);
    /// without one the home air above is only an Earth-like default.
    pub home_known: bool,
    /// The weather's relative humidity, for outdoor fields (None: no weather).
    pub outdoor_rh: Option<f64>,
    /// The home's own air (2026-09-26, ship life support): its carbon dioxide
    /// (g/m3), its temperature (C), its volume (the sealed home less the grow
    /// rooms that keep their own air, m3), the household's food energy (kcal a
    /// day: what they breathe) and the home's rooms, counted as modules for the
    /// leak. Earth-like defaults and no volume when there is no home air space.
    pub home_co2: f64,
    /// Its oxygen, g/m3 (with the life-support data; 0 without).
    pub home_o2: f64,
    pub home_temp_c: f64,
    /// Its own temperature, C, with no heater warming it (its air space's own,
    /// `HomeAir::own_temp_k`), and how far its heaters have warmed it, K
    /// (`HomeAirState::warmed_k`, 2026-10-05): `home_temp_c` is the two added.
    pub home_own_c: f64,
    pub home_warmed_k: f64,
    pub home_volume_m3: f64,
    pub home_kcal: f64,
    pub modules: f64,
}

impl AirMap {
    pub fn new(world: &hecs::World, data: &DataStore, d: &HumidityData) -> Self {
        let boxes: Vec<GrowRoom> = data
            .get::<Mutex<Vec<GrowRoom>>>(ROOMS_KEY)
            .and_then(|m| m.lock().ok().map(|v| v.clone()))
            .unwrap_or_default();
        let mut map = AirMap::default();
        if let Some(plots) = data.get::<Vec<GrowPlot>>("grow_plots") {
            for p in plots.iter().filter(|p| !p.outdoors) {
                // The smallest of the home's rooms the machine stands in.
                let room = boxes
                    .iter()
                    .filter(|r| r.contains(p.pos))
                    .min_by(|a, b| a.volume_m3().total_cmp(&b.volume_m3()))
                    .map(|r| map.add(r.clone(), None));
                // A machine in an enclosure grows in the enclosure's air,
                // which exchanges with that room's (or the home's).
                let idx = match &p.enclosure {
                    Some(e) => map.add(tent_box(d, p, e), room),
                    None => match room {
                        Some(i) => i,
                        None => continue,
                    },
                };
                map.area_room.insert(p.id.clone(), idx);
                for a in &p.aliases {
                    map.area_room.entry(a.clone()).or_insert(idx);
                }
            }
        }
        // The home's air: THE home space's, else an Earth-like default.
        use crate::systems::atmosphere::{Atmosphere, EnclosedSpace, HomeAir};
        let home = world
            .query::<(&HomeAir, &EnclosedSpace)>()
            .iter()
            .next()
            .map(|(_, (h, s))| (s.atmosphere.clone(), s.volume_m3, h.metabolic_kcal_per_day, h.own_temp_k));
        map.home_known = home.is_some();
        let (atmo, volume, kcal, own_k) = home.unwrap_or_else(|| (Atmosphere::default(), 0.0, 0.0, 0.0));
        map.home_temp_c = f64::from(atmo.temperature_k) - 273.15;
        // Its own temperature, and its heaters' warmth as the air step last
        // left it (2026-10-05, BUG-155): a space the farming tick has not
        // written yet is at its own temperature, and a loaded save's space
        // starts fresh, so the saved warmth is the one to trust.
        map.home_own_c = if own_k > 0.0 { f64::from(own_k) - 273.15 } else { map.home_temp_c };
        if map.home_known {
            map.home_warmed_k = world
                .query::<&SoilMemory>()
                .iter()
                .next()
                .map_or(0.0, |(_, m)| m.home_air.warmed_k)
                .max(0.0);
            map.home_temp_c = map.home_own_c + map.home_warmed_k;
        }
        map.home_rh = f64::from(atmo.humidity).clamp(0.0, 1.0);
        map.home_vapour = d.vapour_at(map.home_rh, map.home_temp_c);
        map.home_co2 = f64::from(atmo.gas_percent("CO2")).max(0.0) / 100.0
            * crate::systems::life_support::pure_g_m3(d, d.co2_molar_mass, map.home_temp_c);
        map.home_kcal = f64::from(kcal).max(0.0);
        if let Some(ld) = data.get::<LifeSupportData>(life_support::DATA_KEY) {
            map.home_o2 = f64::from(atmo.gas_percent("O2")).max(0.0) / 100.0
                * life_support::pure_g_m3(d, ld.o2_molar_mass, map.home_temp_c);
        }
        // The home's own air as the life-support model last left it (it is
        // written back to the home space each tick, but a loaded save's space
        // starts fresh, so the saved state is the one to trust).
        if map.home_known {
            if let Some(h) = world.query::<&SoilMemory>().iter().next().map(|(_, m)| m.home_air.clone()) {
                if h.vapour_g_m3 > 0.0 {
                    map.home_vapour = h.vapour_g_m3;
                    map.home_rh = d.rh_of(h.vapour_g_m3, map.home_temp_c).min(1.0);
                }
                if h.co2_g_m3 > 0.0 {
                    map.home_co2 = h.co2_g_m3;
                }
                if h.o2_g_m3 > 0.0 {
                    map.home_o2 = h.o2_g_m3;
                }
            }
        }
        // Its own volume: the sealed home less the grow rooms that keep their
        // own air (tents stand inside rooms and are not subtracted), never
        // below a tenth of it. Its rooms count as modules for the leak.
        let grow: f64 = (0..map.rooms.len()).filter(|i| map.parent[*i].is_none()).map(|i| map.rooms[i].volume_m3()).sum();
        let whole = f64::from(volume).max(0.0);
        map.home_volume_m3 = (whole - grow).max(whole * 0.1);
        map.modules = (boxes.len() as f64).max(1.0);
        map.outdoor_rh = data
            .get::<Mutex<crate::systems::weather::Weather>>("weather")
            .and_then(|m| m.lock().ok().map(|w| f64::from(w.humidity).clamp(0.0, 1.0)));
        map
    }

    /// `room` as an index into `rooms`, added (exchanging with `parent`) if
    /// it is not there yet.
    fn add(&mut self, room: GrowRoom, parent: Option<usize>) -> usize {
        if let Some(i) = self.rooms.iter().position(|x| x.id == room.id) {
            return i;
        }
        self.rooms.push(room);
        self.parent.push(parent);
        self.rooms.len() - 1
    }

    /// The vapour, g/m3, of the air room `i` exchanges with: its parent
    /// room's, or the home's.
    fn outside(&self, i: usize, state: &HashMap<String, RoomAir>) -> f64 {
        self.parent
            .get(i)
            .copied()
            .flatten()
            .and_then(|p| state.get(&self.rooms[p].id))
            .map_or(self.home_vapour, |a| a.vapour_g_m3)
    }

    /// The carbon dioxide, g/m3, of the air room `i` exchanges with: its
    /// parent room's (once known), or the home's.
    fn outside_co2(&self, i: usize, state: &HashMap<String, RoomAir>) -> f64 {
        self.parent
            .get(i)
            .copied()
            .flatten()
            .and_then(|p| state.get(&self.rooms[p].id))
            .map(|a| a.co2_g_m3)
            .filter(|c| *c > 0.0)
            .unwrap_or(self.home_co2)
    }

    /// Is room `i` a room of the home (not a tent inside one)?
    pub fn is_root(&self, i: usize) -> bool {
        self.parent.get(i).map_or(false, |p| p.is_none())
    }

    /// The room index grow area `area` belongs to, if any.
    pub fn room_index(&self, area: &str) -> Option<usize> {
        self.area_room.get(area).copied()
    }

    /// The grow room `area`'s machine stands in, if any.
    pub fn room_of(&self, area: &str) -> Option<&GrowRoom> {
        self.area_room.get(area).and_then(|i| self.rooms.get(*i))
    }

    /// The relative humidity (0..1) the crops of `area` grow in: the
    /// weather's in an outdoor field, their room's, or the home's.
    pub fn rh_for(&self, d: &HumidityData, area: &str, state: &HashMap<String, RoomAir>) -> f64 {
        if super::is_field_area(area) {
            return self.outdoor_rh.unwrap_or(self.home_rh);
        }
        match self.room_of(area) {
            Some(r) => {
                let a = state.get(&r.id);
                let v = a.map_or(self.home_vapour, |a| a.vapour_g_m3);
                d.rh_of(v, room_temp_at(d, a.map_or(0.0, |a| a.warmed_k))).min(1.0)
            }
            None => self.home_rh,
        }
    }

    /// How far heaters have warmed the air the crops of `area` grow in, K
    /// (2026-10-05, BUG-155, farming::heat): their grow room's or tent's own
    /// warmth, or the home's own air's for an indoor crop in no grow room. 0 for
    /// an outdoor field, whose air is the weather's.
    pub fn warmed_k_for(&self, area: &str, state: &HashMap<String, RoomAir>) -> f64 {
        if super::is_field_area(area) {
            return 0.0;
        }
        match self.room_of(area) {
            Some(r) => state.get(&r.id).map_or(0.0, |a| a.warmed_k.max(0.0)),
            None => self.home_warmed_k,
        }
    }

    /// The carbon dioxide, ppm, the crops of `area` grow in (2026-09-27), where
    /// the game tracks it: their grow room's or tent's own (with the life
    /// support data), or the home's own air for a crop in no grow room when the
    /// home's air space exists. None for an outdoor field (the weather's air)
    /// and wherever it is not known, where the fungi are not judged by it.
    pub fn known_co2_ppm(&self, d: &HumidityData, area: &str, state: &HashMap<String, RoomAir>) -> Option<f64> {
        if super::is_field_area(area) {
            return None;
        }
        match self.room_of(area) {
            Some(r) => state
                .get(&r.id)
                .filter(|a| a.co2_g_m3 > 0.0)
                .map(|a| crate::systems::life_support::co2_ppm(d, a.co2_g_m3, room_temp_at(d, a.warmed_k))),
            None => (self.home_known && self.home_co2 > 0.0)
                .then(|| crate::systems::life_support::co2_ppm(d, self.home_co2, self.home_temp_c)),
        }
    }

    /// `rh_for`, but only where the game knows the air: a grow room's own
    /// balance, an outdoor field's weather, or the home's air space. None
    /// otherwise (a headless world with none of them), where a crop's
    /// humidity window is not applied rather than judged against a default.
    pub fn known_rh(&self, d: &HumidityData, area: &str, state: &HashMap<String, RoomAir>) -> Option<f64> {
        let known = if super::is_field_area(area) {
            self.outdoor_rh.is_some()
        } else {
            self.room_of(area).is_some() || self.home_known
        };
        known.then(|| self.rh_for(d, area, state))
    }

    /// The room a fan (or any air machine) at `pos` stands in, as an index
    /// into `rooms`: the smallest that holds it.
    pub fn room_at(&self, pos: [f32; 3]) -> Option<usize> {
        self.rooms
            .iter()
            .enumerate()
            .filter(|(_, r)| r.contains(pos))
            .min_by(|a, b| a.1.volume_m3().total_cmp(&b.1.volume_m3()))
            .map(|(i, _)| i)
    }
}

/// A grow machine's enclosure as a room: its box centred on the machine (on
/// its floor), named for the player, and exchanging the fresh air its
/// substrate's CO2 needs (`HumidityData::tent_fresh_air_m3_h`) over its
/// volume, an hour.
fn tent_box(d: &HumidityData, p: &GrowPlot, e: &crate::systems::grow_machines::Enclosure) -> GrowRoom {
    let (w, h, dz) = (e.size.0.max(0.01), e.size.1.max(0.01), e.size.2.max(0.01));
    let mut room = GrowRoom {
        id: format!("tent:{}", p.id),
        name: "Fruiting tent".to_string(),
        min: [p.pos[0] - w / 2.0, p.pos[1], p.pos[2] - dz / 2.0],
        max: [p.pos[0] + w / 2.0, p.pos[1] + h, p.pos[2] + dz / 2.0],
        air_changes_per_hour: None,
        substrate_kg: f64::from(e.substrate_kg.max(0.0)),
    };
    room.air_changes_per_hour = Some(d.tent_fresh_air_m3_h(f64::from(e.substrate_kg)) / room.volume_m3().max(1e-6));
    room
}

/// A placed exhaust fan, by the room (or tent) it stands in.
#[derive(Debug, Clone, Copy)]
struct Fan {
    entity: hecs::Entity,
    /// Index into `AirMap::rooms`.
    room: usize,
    /// Full airflow, m3 an hour, and full draw, W.
    airflow: f64,
    watts: f64,
    /// Its PowerConsumer is enabled.
    powered: bool,
    /// 0 for a humidity-controlled fan; else the CO2, ppm, its CO2 controller
    /// switches it on above (2026-09-27, `Ventilator::co2_setpoint_ppm`).
    co2_set: f64,
}

impl Fan {
    fn on_co2(&self) -> bool {
        self.co2_set > 0.0
    }
}

/// The exhaust fans, by the room they stand in.
fn room_fans(world: &hecs::World, map: &AirMap) -> Vec<Fan> {
    world
        .query::<(&Ventilator, &Transform, Option<&PowerConsumer>)>()
        .iter()
        .filter_map(|(e, (v, t, pc))| {
            let room = map.room_at(t.position.to_array())?;
            Some(Fan {
                entity: e,
                room,
                airflow: f64::from(v.airflow_m3_h),
                watts: f64::from(v.watts),
                powered: pc.map_or(false, |p| p.enabled),
                co2_set: f64::from(v.co2_setpoint_ppm).max(0.0),
            })
        })
        .collect()
}

/// The humidifiers, by the room they stand in: (entity, room index, full
/// L/h, full-output watts, powered).
fn room_humidifiers(world: &hecs::World, map: &AirMap) -> Vec<(hecs::Entity, usize, f64, f64, bool)> {
    world
        .query::<(&Humidifier, &Transform, Option<&PowerConsumer>)>()
        .iter()
        .filter_map(|(e, (h, t, pc))| {
            let room = map.room_at(t.position.to_array())?;
            Some((e, room, f64::from(h.output_l_h), f64::from(h.watts), pc.map_or(false, |p| p.enabled)))
        })
        .collect()
}

/// The air handlers and the CO2 scrubbers (2026-09-26, ship life support), by
/// the air they stand in: a grow room's (the smallest box they stand in), or
/// None for the home's own air. Each unit's capacity is its full airflow (m3
/// an hour) or its rated carbon dioxide (kg a day).
fn life_units(world: &hecs::World, map: &AirMap) -> (Vec<life_support::Unit>, Vec<life_support::Unit>) {
    use crate::ecs::components::{AirHandler, Co2Scrubber};
    let handlers = world
        .query::<(&AirHandler, &Transform, Option<&PowerConsumer>)>()
        .iter()
        .map(|(e, (a, t, pc))| life_support::Unit {
            entity: e,
            room: map.room_at(t.position.to_array()),
            capacity: f64::from(a.airflow_m3_h),
            watts: f64::from(a.watts),
            powered: pc.map_or(false, |p| p.enabled),
        })
        .collect();
    let scrubbers = world
        .query::<(&Co2Scrubber, &Transform, Option<&PowerConsumer>)>()
        .iter()
        .map(|(e, (s, t, pc))| life_support::Unit {
            entity: e,
            room: map.room_at(t.position.to_array()),
            capacity: f64::from(s.rated_kg_day),
            watts: f64::from(s.watts),
            powered: pc.map_or(false, |p| p.enabled),
        })
        .collect();
    (handlers, scrubbers)
}

/// The vapour the fans of a room hold it under, g/m3: `fan_setpoint_rh`, or,
/// in a room a humidifier holds, `humidified_fan_margin_rh` above the
/// humidifier's setpoint, so the two never work against each other. `sat` is
/// what saturated air holds at the room's temperature (a warmed room holds
/// more, 2026-10-05).
fn fan_set(d: &HumidityData, humidified: bool, sat: f64) -> f64 {
    let rh = if humidified {
        d.fan_setpoint_rh.max(d.humidifier_setpoint_rh + d.humidified_fan_margin_rh)
    } else {
        d.fan_setpoint_rh
    };
    rh.clamp(0.0, 1.0) * sat
}

/// A grow room's or tent's temperature, C, when its heaters have warmed it
/// `warmed_k` above the rooms' own (2026-10-05, BUG-155, farming::heat).
pub fn room_temp_at(d: &HumidityData, warmed_k: f64) -> f64 {
    d.room_temp_c + if warmed_k.is_finite() { warmed_k.max(0.0) } else { 0.0 }
}

/// The air a person standing at `pos` breathes when it is a grow room's own
/// (2026-10-05, BUG-155): its temperature (C, its heaters' warmth included)
/// and its relative humidity (0..1), from the smallest published room around
/// `pos` that the farming tick keeps air for. None anywhere else (the home's
/// own air is then the one), and before the rooms or their air are known.
pub fn grow_room_air_at(world: &hecs::World, data: &DataStore, pos: [f32; 3]) -> Option<(f64, f64)> {
    let d = data.get::<HumidityData>(DATA_KEY)?;
    let boxes = data.get::<Mutex<Vec<GrowRoom>>>(ROOMS_KEY)?.lock().ok()?;
    let mut q = world.query::<&SoilMemory>();
    let (_, mem) = q.iter().next()?;
    let room = boxes
        .iter()
        .filter(|r| r.contains(pos) && mem.rooms.contains_key(&r.id))
        .min_by(|a, b| a.volume_m3().total_cmp(&b.volume_m3()))?;
    let st = mem.rooms.get(&room.id)?;
    let t = room_temp_at(d, st.warmed_k);
    Some((t, d.rh_of(st.vapour_g_m3, t).min(1.0)))
}

/// The diseases whose Humidity window holds `rh`, by name (for the notice).
fn humid_diseases(pests: &PestData, rh: f64) -> Vec<String> {
    pests
        .pests
        .iter()
        .filter(|p| p.disease)
        .filter(|p| {
            p.favoured_by
                .iter()
                .any(|c| matches!(c, Condition::Humidity { min, max } if rh >= *min && rh <= *max))
        })
        .map(|p| p.name.to_lowercase())
        .collect()
}

/// What one air step needs besides the rooms' own state (2026-09-26): each
/// grow area's crops' breath and gas exchange, which areas hold growing
/// mushrooms, the garden's water gate and the game hours to step. The farming
/// tick tallies the crops into it.
#[derive(Debug, Clone, Default)]
pub struct AirStep {
    /// Litres a day each area's growing, watered crops breathe out.
    pub breathed: HashMap<String, f64>,
    /// Grams a day of carbon dioxide each area's crops would take up, and of
    /// oxygen they would give out, at the reference CO2 and their light
    /// (life_support.rs, `Exchange`).
    pub photo: HashMap<String, (f64, f64)>,
    /// Grams of carbon dioxide an hour each area's growing fungus breathes out
    /// (2026-09-27): the blocks or square feet actually planted there, each at
    /// its species' rate (`HumidityData::fungus_co2_g_h`). It used to be a set
    /// of areas, and a tent breathed for its whole ten-block load as soon as
    /// one shelf was planted.
    pub fungi: HashMap<String, f64>,
    /// The home has water for the garden (the humidifiers may run).
    pub water_ok: bool,
    pub hours: f64,
}

/// What one air step did, for the caller to bill and to say.
#[derive(Debug, Clone, Default)]
pub struct AirOut {
    /// Notices to give (a room just became humid enough for the damp diseases).
    pub notices: Vec<String>,
    /// Litres a day the humidifiers are turning into vapour: drawn from the
    /// tanks with the irrigation.
    pub humidifier_l_day: f64,
    /// Litres a day each air handler's coil is condensing: handed back to the
    /// tanks through its plumbing island.
    pub condensate_by_entity: HashMap<hecs::Entity, f64>,
}

/// Step every air of the home `step.hours` game hours: each grow room's and
/// tent's, then the home's own (2026-09-26, ship life support, when
/// `life` is given and the home has an air space). In each room: the water its
/// crops breathe out and its humidifiers put in, exchanged with the air around
/// it by its leakage and fans and condensed by its air handlers; and, with
/// `life`, its carbon dioxide (its crops' uptake at their light and the room's
/// CO2, its mushrooms' breath, its scrubbers). Everything a room passes to the
/// air around it is that air's source, so nothing is created or lost between
/// them (`life_support::relax` is exact). The step sets every fan's,
/// humidifier's, air handler's and scrubber's share and draw, and records the
/// air's water in `home.ledger`. A long step (a catch-up) is cut into slices of
/// `life_support::MAX_SLICE_H`. Rooms the map no longer knows are forgotten.
#[allow(clippy::too_many_arguments)]
pub fn step_rooms(
    world: &mut hecs::World,
    d: &HumidityData,
    life: Option<&LifeSupportData>,
    pests: &PestData,
    map: &AirMap,
    state: &mut HashMap<String, RoomAir>,
    home: &mut HomeAirState,
    step: &AirStep,
) -> AirOut {
    use life_support::{relax, Relaxed};
    let mut out = AirOut::default();
    // A room the home no longer has is forgotten, but only once the engine
    // has published the rooms at all: until then (early boot, before the
    // world is entered) a loaded save's rooms are kept as they were.
    if !map.rooms.is_empty() {
        state.retain(|id, _| map.rooms.iter().any(|r| r.id == *id));
    }
    let n_rooms = map.rooms.len();
    // Each room's crops' breath and gas exchange; crops standing in no grow
    // room breathe into the home's own air (outdoor fields into the weather).
    let mut litres = vec![0.0f64; n_rooms];
    let mut photo = vec![(0.0f64, 0.0f64); n_rooms];
    let (mut home_litres, mut home_photo) = (0.0f64, (0.0f64, 0.0f64));
    for (area, l) in &step.breathed {
        match map.area_room.get(area) {
            Some(i) => litres[*i] += l.max(0.0),
            None if !super::is_field_area(area) => home_litres += l.max(0.0),
            None => {}
        }
    }
    for (area, (c, o)) in &step.photo {
        match map.area_room.get(area) {
            Some(i) => {
                photo[*i].0 += c.max(0.0);
                photo[*i].1 += o.max(0.0);
            }
            None if !super::is_field_area(area) => {
                home_photo.0 += c.max(0.0);
                home_photo.1 += o.max(0.0);
            }
            None => {}
        }
    }
    // What each room's or tent's growing fungus breathes out, g of CO2 an
    // hour: the blocks or square feet planted there (2026-09-27; a tent used
    // to breathe for its whole load as soon as one shelf was planted).
    let mut fungi_rate = vec![0.0f64; n_rooms];
    for (area, g_h) in &step.fungi {
        if let Some(i) = map.area_room.get(area) {
            fungi_rate[*i] += g_h.max(0.0);
        }
    }
    let fans = room_fans(world, map);
    let hums = room_humidifiers(world, map);
    let (handlers, scrubbers) = life_units(world, map);
    // The space heaters (2026-10-05, BUG-155, farming::heat), by the air they
    // stand in. Each room's temperature is its own (`room_temp_c`) plus what
    // they have warmed it by, read at each slice's start below: its humidity,
    // its carbon dioxide and its coils all follow it.
    let heaters = super::heat::heaters(world, map);
    // The home's own air: its live state when the model runs it, else the
    // map's (an Earth-like default without a home air space).
    let runs_home = life.is_some() && map.home_known;
    if runs_home {
        if home.vapour_g_m3 <= 0.0 {
            home.vapour_g_m3 = map.home_vapour;
        }
        if home.co2_g_m3 <= 0.0 {
            home.co2_g_m3 = map.home_co2;
        }
        if home.o2_g_m3 <= 0.0 {
            home.o2_g_m3 = map.home_o2;
        }
    }
    let (home_handlers, home_scrubbers): (Vec<_>, Vec<_>) = (
        handlers.iter().filter(|u| u.room.is_none()).copied().collect(),
        scrubbers.iter().filter(|u| u.room.is_none()).copied().collect(),
    );
    // Tents first: what a tent vents is a source for the room it stands in.
    let order: Vec<usize> = (0..n_rooms)
        .filter(|i| map.parent[*i].is_some())
        .chain((0..n_rooms).filter(|i| map.parent[*i].is_none()))
        .collect();
    let slices = if step.hours > 0.0 { (step.hours / life_support::MAX_SLICE_H).ceil().clamp(1.0, 10_000.0) as usize } else { 1 };
    let sh = step.hours.max(0.0) / slices as f64;
    // Each room's machines as their controllers set them this step: the
    // powered ones' share (the physics), and the share a unit the electrical
    // sim has shed would ask for if it had power (its request, 2026-09-27: a
    // shed unit that asked for nothing, or for what it last drew, never came
    // back, or kept a phantom load on the island).
    let mut speed = vec![0.0f64; n_rooms];
    let mut speed_req = vec![0.0f64; n_rooms];
    let mut duty = vec![0.0f64; n_rooms];
    let mut duty_req = vec![0.0f64; n_rooms];
    let mut share = vec![0.0f64; n_rooms];
    let mut share_req = vec![0.0f64; n_rooms];
    let mut handler_share = vec![0.0f64; n_rooms];
    let mut handler_req = vec![0.0f64; n_rooms];
    let mut scrubber_share = vec![0.0f64; n_rooms];
    let mut scrubber_req = vec![0.0f64; n_rooms];
    // Each air's heaters' share of the time on, and what a shed one asks for
    // (the home's own air's apart).
    let mut heat_duty = vec![0.0f64; n_rooms];
    let mut heat_req = vec![0.0f64; n_rooms];
    let (mut home_heat_duty, mut home_heat_req) = (0.0f64, 0.0f64);
    for slice in 0..slices {
        let last = slice + 1 == slices;
        let (home_v, home_c) = if runs_home { (home.vapour_g_m3, home.co2_g_m3) } else { (map.home_vapour, map.home_co2) };
        let mut vented_v = vec![0.0f64; n_rooms];
        let mut vented_c = vec![0.0f64; n_rooms];
        let mut home_in = life_support::HomeInputs::default();
        // The heat each air passed the air around it this slice, J: a tent's to
        // its room, a room's to the home's own air (farming::heat).
        let mut heat_in_j = vec![0.0f64; n_rooms];
        let mut home_heat_in_j = 0.0f64;
        // The home's own air's warmth at the slice's start, what a room's warmth
        // relaxes toward (none without a home air space).
        let home_k = if map.home_known { home.warmed_k.max(0.0) } else { 0.0 };
        for &i in &order {
            let room = &map.rooms[i];
            let volume = room.volume_m3().max(0.01);
            let parent_air = map.parent[i].and_then(|p| state.get(&map.rooms[p].id).copied());
            let outside = parent_air.map_or(home_v, |a| a.vapour_g_m3);
            let outside_c = parent_air.map(|a| a.co2_g_m3).filter(|c| *c > 0.0).unwrap_or(home_c);
            // The warmth of the air around it: its tent's room's, or the home's.
            let around_k = parent_air.map_or(home_k, |a| a.warmed_k);
            let breath_g_h = litres[i] * d.vapour_share.max(0.0) * 1000.0 / 24.0;
            let source = (breath_g_h + if sh > 0.0 { vented_v[i] / sh } else { 0.0 }) / volume;
            // Its fans, each air changes an hour at full speed: the humidity
            // ones, and the ones a CO2 controller switches (2026-09-27), the
            // powered ones and all of them (for a shed fan's request).
            let air_changes = |co2: bool, powered_only: bool| -> f64 {
                fans.iter()
                    .filter(|f| f.room == i && f.on_co2() == co2 && (f.powered || !powered_only))
                    .map(|f| f.airflow)
                    .sum::<f64>()
                    / volume
            };
            let (fan_max, fan_max_all) = (air_changes(false, true), air_changes(false, false));
            let (cfan_max, cfan_max_all) = (air_changes(true, true), air_changes(true, false));
            // The CO2 fans' controller setpoint: the lowest among them.
            let co2_set_ppm = fans.iter().filter(|f| f.room == i && f.on_co2()).map(|f| f.co2_set).fold(f64::INFINITY, f64::min);
            let humidified = hums.iter().any(|h| h.1 == i);
            let powered_l_h: f64 = hums.iter().filter(|h| h.1 == i && h.4).map(|h| h.2).sum();
            let all_l_h: f64 = hums.iter().filter(|h| h.1 == i).map(|h| h.2).sum();
            // What the humidifiers can put in, g/m3 an hour: none without water.
            // (Tested with `>`: an empty float sum is -0.0, which would print.)
            let hum_max = if step.water_ok && powered_l_h > 0.0 { powered_l_h * 1000.0 / volume } else { 0.0 };
            let hum_max_all = if step.water_ok && all_l_h > 0.0 { all_l_h * 1000.0 / volume } else { 0.0 };
            let st = state
                .entry(room.id.clone())
                .or_insert(RoomAir { vapour_g_m3: outside, co2_g_m3: outside_c, ..Default::default() });
            if st.co2_g_m3 <= 0.0 {
                st.co2_g_m3 = outside_c;
            }
            // Its temperature at the slice's start: the rooms' own, plus what its
            // heaters have warmed it by (2026-10-05, BUG-155). What saturated air
            // holds there, and so every humidity setpoint, follows it.
            let t_room = room_temp_at(d, st.warmed_k);
            let sat = d.vapour_at(1.0, t_room);
            let hum_set = d.humidifier_setpoint_rh.clamp(0.0, 1.0) * sat;
            let base = room.air_changes_per_hour.unwrap_or(d.base_air_changes_per_hour).max(0.0);
            // Its carbon dioxide's sources at the slice's start (with the
            // life-support data): what its tents vented into it, its fungi's
            // breath, less its crops' uptake at their light and its CO2.
            let c0 = st.co2_g_m3;
            let (co2_f, uptake_g_h, fungi_g_h) = match life {
                Some(ld) => {
                    let f = ld.co2_factor(life_support::co2_ppm(d, c0, t_room));
                    (f, photo[i].0 / 24.0 * f, fungi_rate[i])
                }
                None => (0.0, 0.0, 0.0),
            };
            let s_c = ((if sh > 0.0 { vented_c[i] / sh } else { 0.0 }) + fungi_g_h - uptake_g_h) / volume;
            // The CO2 fans (2026-09-27): switched on above their setpoint, on
            // for the share of the time that holds it, off below
            // (`co2_fan_duty`); the humidity fans are counted as they last ran.
            // Only with the life-support data, which tracks the carbon dioxide.
            let (dt_c, dt_c_req) = match (life.is_some(), co2_set_ppm.is_finite()) {
                (true, true) => {
                    let set_c = life_support::co2_g_m3(d, co2_set_ppm, t_room);
                    let around = base + st.fan_speed * fan_max;
                    (
                        co2_fan_duty(c0, s_c, around, cfan_max, outside_c, set_c, sh),
                        co2_fan_duty(c0, s_c, around, cfan_max_all, outside_c, set_c, sh),
                    )
                }
                _ => (0.0, 0.0),
            };
            let base_c = base + dt_c * cfan_max;
            let s = fan_speed(st.vapour_g_m3, source, base_c, fan_max, outside, fan_set(d, humidified, sat));
            let s_req = fan_speed(st.vapour_g_m3, source, base_c, fan_max_all, outside, fan_set(d, humidified, sat));
            let n = base_c + s * fan_max;
            // Its air handlers: their coil, the setpoint they hold (above a
            // humidifier's, like the fans, where one holds the room), their pull.
            let (v_coil, set_a, a_max, a_max_all) = match life {
                Some(ld) => {
                    let hq: f64 = handlers.iter().filter(|u| u.room == Some(i) && u.powered).map(|u| u.capacity).sum();
                    let hq_all: f64 = handlers.iter().filter(|u| u.room == Some(i)).map(|u| u.capacity).sum();
                    let rh = if humidified {
                        ld.grow_room_setpoint_rh.max(d.humidifier_setpoint_rh + d.humidified_fan_margin_rh)
                    } else {
                        ld.grow_room_setpoint_rh
                    };
                    (life_support::coil_vapour(d, ld, t_room), rh.clamp(0.0, 1.0) * sat, hq / volume, hq_all / volume)
                }
                None => (0.0, sat, 0.0, 0.0),
            };
            let v0 = st.vapour_g_m3;
            let mut h = humidifier_share(v0, source, n, hum_max, outside, hum_set);
            let h_req = humidifier_share(v0, source, n, hum_max_all, outside, hum_set);
            let ash = life_support::pull_share(v0, set_a, a_max, v_coil, source + h * hum_max + n * (outside - set_a));
            let ash_req = life_support::pull_share(v0, set_a, a_max_all, v_coil, source + h * hum_max + n * (outside - set_a));
            let a = ash * a_max;
            let sinks = [(n, outside), (a, v_coil)];
            // The humidifiers' output held at its setpoint, the coil counted.
            let hold = || humidifier_share(hum_set, source - a * (hum_set - v_coil), n, hum_max, outside, hum_set);
            // Flat out, a humidifier that reaches its setpoint within the
            // slice holds it from then on: the slice is solved in two parts,
            // so the water it did not put in is not counted.
            let (rv, hum_g_m3): (Relaxed, f64) = if h >= 1.0 && v0 < hum_set && hum_max > 0.0 {
                match life_support::time_to(v0, source + hum_max, &sinks, hum_set).filter(|t| *t < sh) {
                    Some(t1) => {
                        let q1 = relax(v0, source + hum_max, &sinks, t1, sat);
                        h = hold();
                        let q2 = relax(q1.x, source + h * hum_max, &sinks, sh - t1, sat);
                        let both = Relaxed { x: q2.x, integral: q1.integral + q2.integral, over: q1.over + q2.over, short: q1.short + q2.short };
                        (both, hum_max * (t1 + h * (sh - t1)))
                    }
                    None => (relax(v0, source + hum_max, &sinks, sh, sat), hum_max * sh),
                }
            } else {
                (relax(v0, source + h * hum_max, &sinks, sh, sat), h * hum_max * sh)
            };
            st.vapour_g_m3 = rv.x;
            // What it passed to the air around it, and what its coils took.
            let vent_v = rv.to_sink(n, outside, sh) * volume;
            match map.parent[i] {
                Some(p) => vented_v[p] += vent_v,
                None => home_in.vapour_in_g += vent_v,
            }
            let condensed_g = rv.to_sink(a, v_coil, sh) * volume;
            home.ledger.breathed_l += breath_g_h * sh / 1000.0 + rv.short * volume / 1000.0;
            home.ledger.humidified_l += hum_g_m3 * volume / 1000.0;
            home.ledger.condensed_l += condensed_g / 1000.0;
            home.ledger.surface_l += rv.over * volume / 1000.0;
            st.fan_speed = s;
            st.breathed_l_day = litres[i];
            st.humidifier = h;
            st.humidifier_l_day = h * hum_max * volume / 1000.0 * 24.0;
            // Powered, and stopped for want of water: dry tanks or the irrigation off.
            st.humidifier_dry = !step.water_ok && powered_l_h > 0.0;
            st.air_handler = ash;
            st.condensate_l_day = a * (rv.x - v_coil).max(0.0) * volume * 24.0 / 1000.0;
            st.co2_fan = dt_c;
            speed[i] = s;
            speed_req[i] = s_req;
            duty[i] = dt_c;
            duty_req[i] = dt_c_req;
            share[i] = h;
            share_req[i] = h_req;
            handler_share[i] = ash;
            handler_req[i] = ash_req;
            // Its carbon dioxide (with the life-support data).
            if let Some(ld) = life {
                let f = co2_f;
                let rated = life_support::co2_g_m3(d, ld.scrubber_rated_ppm, t_room).max(1e-9);
                let scrub = |powered_only: bool| -> f64 {
                    scrubbers
                        .iter()
                        .filter(|u| u.room == Some(i) && (u.powered || !powered_only))
                        .map(|u| u.capacity * 1000.0 / 24.0)
                        .sum::<f64>()
                        / rated
                        / volume
                };
                let (b_max, b_max_all) = (scrub(true), scrub(false));
                let set_c = life_support::co2_g_m3(d, ld.scrubber_setpoint_ppm, t_room);
                let csh = life_support::pull_share(c0, set_c, b_max, 0.0, s_c + n * (outside_c - set_c));
                scrubber_req[i] = life_support::pull_share(c0, set_c, b_max_all, 0.0, s_c + n * (outside_c - set_c));
                let b = csh * b_max;
                let rc = relax(c0, s_c, &[(n, outside_c), (b, 0.0)], sh, f64::INFINITY);
                st.co2_g_m3 = rc.x;
                let vent_c = rc.to_sink(n, outside_c, sh) * volume;
                match map.parent[i] {
                    Some(p) => vented_c[p] += vent_c,
                    None => home_in.co2_in_g += vent_c,
                }
                // The uptake the air could give (a room drawn to nothing gives
                // no more), and the oxygen for it; the mushrooms' breath.
                let want = uptake_g_h * sh;
                let taken = (want - rc.short * volume).clamp(0.0, want.max(0.0));
                let frac = if want > 0.0 { taken / want } else { 0.0 };
                home_in.o2_made_g += photo[i].1 / 24.0 * f * sh * frac;
                home_in.o2_used_g += fungi_g_h * sh * ld.o2_molar_mass / d.co2_molar_mass / ld.fungi_respiratory_quotient.max(1e-9);
                home_in.co2_out_rooms_g += fungi_g_h * sh;
                home_in.co2_uptake_rooms_g += taken;
                st.co2_uptake_g_day = if sh > 0.0 { taken / sh * 24.0 } else { uptake_g_h * 24.0 };
                st.co2_out_g_day = fungi_g_h * 24.0;
                st.scrubber = csh;
                scrubber_share[i] = csh;
            }
            // Its warmth (2026-10-05, BUG-155, farming::heat): its heaters on
            // their thermostat and the heat its tents passed it, given to the air
            // around it through its walls and ceiling and with the air it
            // exchanges (`n`, the changes its water just used), and taken by its
            // air handlers' coils (`a`, the air they moved).
            let cap = super::heat::heat_capacity_j_m3_k(d, d.room_temp_c);
            let (q, q_all, set_c) = super::heat::heat_in(&heaters, Some(i));
            let w = super::heat::step(
                &super::heat::AirHeat {
                    warmed_k: st.warmed_k,
                    capacity_j_k: cap * volume,
                    around_w_k: super::heat::conductance_w_k(d, super::heat::envelope_m2(room), n, volume, cap),
                    around_k,
                    coil_w_k: cap * a * volume / 3600.0,
                    in_w: if sh > 0.0 { heat_in_j[i] / (sh * 3600.0) } else { 0.0 },
                    heat_w: q,
                    heat_all_w: q_all,
                    set_k: set_c.map(|c| c - d.room_temp_c),
                },
                sh,
            );
            st.warmed_k = w.warmed_k;
            st.heater = w.duty;
            heat_duty[i] = w.duty;
            heat_req[i] = w.duty_req;
            match map.parent[i] {
                Some(p) => heat_in_j[p] += w.passed_j,
                None => home_heat_in_j += w.passed_j,
            }
            if last {
                out.humidifier_l_day += st.humidifier_l_day;
                let hq: f64 = handlers.iter().filter(|u| u.room == Some(i) && u.powered).map(|u| u.capacity).sum();
                for u in handlers.iter().filter(|u| u.room == Some(i) && u.powered) {
                    out.condensate_by_entity.insert(u.entity, if hq > 0.0 { st.condensate_l_day * u.capacity / hq } else { 0.0 });
                }
                let rh = d.rh_of(st.vapour_g_m3, room_temp_at(d, st.warmed_k));
                if humidified {
                    // The damp is meant here (a mushroom room): no disease notice.
                } else if rh >= d.notice_above_rh && !st.told {
                    st.told = true;
                    let names = humid_diseases(pests, rh.min(1.0));
                    let spread = if names.is_empty() { String::new() } else { format!(", where {} spread", names.join(", ")) };
                    out.notices.push(format!(
                        "The {} air is at {:.0}% humidity{spread}: its crops breathe out {:.0} L of water a day. \
                         Ventilate it from the Garden panel, or run an exhaust fan or an air handler there.",
                        room.name,
                        (rh * 100.0).min(100.0),
                        litres[i]
                    ));
                } else if rh < d.notice_above_rh - 0.05 {
                    st.told = false;
                }
            }
        }
        // The home's own air takes in what the rooms passed it and steps, at
        // its temperature this slice (its own, plus its heaters' warmth).
        if let (Some(ld), true) = (life, runs_home) {
            home_in.volume_m3 = map.home_volume_m3;
            home_in.temp_c = map.home_own_c + home_k;
            home_in.kcal_per_day = map.home_kcal;
            home_in.modules = map.modules;
            home_in.breathed_l_day = home_litres;
            home_in.photo_co2_g_day = home_photo.0;
            home_in.photo_o2_g_day = home_photo.1;
            let ho = life_support::step_home(world, d, ld, home, &home_in, &home_handlers, &home_scrubbers, sh);
            if last {
                out.condensate_by_entity.extend(ho.condensate_by_entity);
            }
        }
        // The home's own air's warmth (2026-10-05, BUG-155, farming::heat): its
        // heaters on their thermostat and the heat the grow rooms passed it,
        // given to the station around the home through its walls and roof, and
        // taken by its own air handlers' coils (the air they moved this slice).
        if map.home_known {
            let volume = map.home_volume_m3.max(0.0);
            let cap = super::heat::heat_capacity_j_m3_k(d, map.home_own_c);
            let coil_m3_h = if runs_home { home.air_handler * home_handlers.iter().filter(|u| u.powered).map(|u| u.capacity).sum::<f64>() } else { 0.0 };
            let (q, q_all, set_c) = super::heat::heat_in(&heaters, None);
            let w = super::heat::step(
                &super::heat::AirHeat {
                    warmed_k: home.warmed_k,
                    capacity_j_k: cap * volume,
                    around_w_k: super::heat::conductance_w_k(d, super::heat::home_envelope_m2(volume, d.home_storey_m), 0.0, volume, cap),
                    around_k: 0.0,
                    coil_w_k: cap * coil_m3_h / 3600.0,
                    in_w: if sh > 0.0 { home_heat_in_j / (sh * 3600.0) } else { 0.0 },
                    heat_w: q,
                    heat_all_w: q_all,
                    set_k: set_c.map(|c| c - map.home_own_c),
                },
                sh,
            );
            home.warmed_k = w.warmed_k;
            home.heater = w.duty;
            home_heat_duty = w.duty;
            home_heat_req = w.duty_req;
        }
    }
    // Each heater draws its watts for the share of the time its thermostat
    // runs it (humidity.ron, THE HEAT); one in an air the game does not keep
    // (no home air space) does not run.
    for h in &heaters {
        if let Ok(mut pc) = world.get::<&mut PowerConsumer>(h.entity) {
            let (on, req) = match h.room {
                Some(i) => (heat_duty[i], heat_req[i]),
                None => (home_heat_duty, home_heat_req),
            };
            pc.draw_watts = (h.watts * if h.powered { on } else { req }) as f32;
        }
    }
    // Every machine's draw is written every step, powered or not
    // (2026-09-27): a powered one draws what its controller runs it at, and
    // one the electrical sim has shed asks for what it would draw with power,
    // so the island sees its real request and gives it back its power when
    // there is room. Writing only the powered ones left a shed unit asking
    // for whatever it drew last (its spawn nameplate, 325 W for an air handler
    // whose station supplies it), or for nothing, and it never came back.
    //
    // A humidity fan draws its watts at the cube of its speed (the fan laws);
    // a CO2 fan is switched on and off by its controller, so it draws its
    // watts for the share of the time it is on (humidity.ron, THE CO2 FANS).
    for f in &fans {
        if let Ok(mut pc) = world.get::<&mut PowerConsumer>(f.entity) {
            pc.draw_watts = if f.on_co2() {
                (f.watts * if f.powered { duty[f.room] } else { duty_req[f.room] }) as f32
            } else {
                let s = if f.powered { speed[f.room] } else { speed_req[f.room] };
                (f.watts * s.powf(d.fan_power_exponent.max(1.0))) as f32
            };
        }
    }
    // Each humidifier draws its watts in proportion to its output share
    // (humidity.ron, THE HUMIDIFIER).
    for (e, room, _, watts, powered) in hums {
        if let Ok(mut pc) = world.get::<&mut PowerConsumer>(e) {
            pc.draw_watts = (watts * if powered { share[room] } else { share_req[room] }) as f32;
        }
    }
    // Each room's air handlers along their fan's curve, its scrubbers for
    // the share of time they run (life_support.ron), in either Ship life
    // support mode (the Station-supplied one feeds them from the ship's
    // reactor, systems::ship_power); the home's own are set by its step.
    if let Some(ld) = life {
        for u in &handlers {
            if let (Some(i), Ok(mut pc)) = (u.room, world.get::<&mut PowerConsumer>(u.entity)) {
                let s = if u.powered { handler_share[i] } else { handler_req[i] };
                pc.draw_watts = (u.watts * ld.fan_power_share(s)) as f32;
            }
        }
        for u in &scrubbers {
            if let (Some(i), Ok(mut pc)) = (u.room, world.get::<&mut PowerConsumer>(u.entity)) {
                let s = if u.powered { scrubber_share[i] } else { scrubber_req[i] };
                pc.draw_watts = (u.watts * s) as f32;
            }
        }
    }
    out
}

/// The Ventilate control on `area` (pests.ron `ventilate`): a heat-and-vent
/// of the grow room it stands in, `air_changes` changes of its air with the
/// home's at once. Well-mixed air keeps e^-N of its excess vapour over the
/// home air's after N changes. Returns what to tell the player.
pub fn ventilate(world: &mut hecs::World, data: &DataStore, d: &HumidityData, area: &str, air_changes: f64) -> String {
    let map = AirMap::new(world, data, d);
    let Some(room) = map.room_of(area).cloned() else {
        return if super::is_field_area(area) {
            "An outdoor field has all the air there is: there is nothing to ventilate.".to_string()
        } else {
            format!("{} is not in a grow room with its own air: there is nothing to ventilate.", super::pests::place(area))
        };
    };
    let ri = map.rooms.iter().position(|r| r.id == room.id);
    let fans = room_fans(world, &map);
    let volume = room.volume_m3().max(0.01);
    let memory = super::soil::soil_memory_entity(world);
    let Ok(mut mem) = world.get::<&mut SoilMemory>(memory) else { return String::new() };
    // The air it is changed with: a tent's room's, or the home's.
    let outside = ri.map_or(map.home_vapour, |i| map.outside(i, &mem.rooms));
    let st = mem.rooms.entry(room.id.clone()).or_insert(RoomAir { vapour_g_m3: outside, ..Default::default() });
    let before = st.vapour_g_m3;
    st.vapour_g_m3 = outside + (before - outside) * (-air_changes.max(0.0)).exp();
    // At the room's own temperature, its heaters' warmth included (2026-10-05).
    let t_room = room_temp_at(d, st.warmed_k);
    let (rh0, rh1) = (d.rh_of(before, t_room).min(1.0), d.rh_of(st.vapour_g_m3, t_room).min(1.0));
    // How long until the crops breathe it back: to the disease line, or to
    // where it was if that was lower, at the air change it has now.
    let source = st.breathed_l_day * d.vapour_share.max(0.0) * 1000.0 / 24.0 / volume;
    // Its air changes now: its leakage, its humidity fans at their speed and
    // its CO2 fans for the share of the time they run.
    let fan_max = |co2: bool| -> f64 {
        fans.iter().filter(|f| Some(f.room) == ri && f.powered && f.on_co2() == co2).map(|f| f.airflow).sum::<f64>() / volume
    };
    let n = room.air_changes_per_hour.unwrap_or(d.base_air_changes_per_hour).max(0.0)
        + st.fan_speed * fan_max(false)
        + st.co2_fan * fan_max(true);
    let target = before.min(d.notice_above_rh * d.vapour_at(1.0, t_room));
    let back = if n > 1e-12 {
        let eq = outside + source / n;
        (eq > target && st.vapour_g_m3 < target).then(|| ((st.vapour_g_m3 - eq) / (target - eq)).ln() / n)
    } else {
        (source > 0.0 && st.vapour_g_m3 < target).then(|| (target - st.vapour_g_m3) / source)
    };
    let tail = match back {
        Some(h) => format!(
            " Its crops breathe out {:.0} L of water a day, so it is back to {:.0}% in about {}. An exhaust fan does this all the time.",
            st.breathed_l_day,
            d.rh_of(target, t_room).min(1.0) * 100.0,
            hours_word(h)
        ),
        None => " At this air change it stays drier than that.".to_string(),
    };
    format!(
        "Heat and vent in the {}: its air went from {:.0}% to {:.0}% humidity.{tail}",
        room.name,
        rh0 * 100.0,
        rh1 * 100.0
    )
}

/// "40 minutes", "2 hours" (game time).
fn hours_word(h: f64) -> String {
    if h < 1.0 {
        format!("{:.0} minutes of game time", (h * 60.0).max(1.0))
    } else {
        format!("{:.1} hours of game time", h)
    }
}

// -- What the Garden panel shows ------------------------------------------------------

/// A whole number with thousands separated: "1,240".
fn thousands(x: f64) -> String {
    let digits = (x.max(0.0).round() as u64).to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The home's own air and its loops, for the top of the Garden panel
/// (2026-09-26, ship life support): the air and its machines, then the day's
/// carbon and oxygen, then the day's water between the garden and the tanks.
/// Empty until the air step has run once.
fn home_lines(
    world: &hecs::World,
    data: &DataStore,
    d: &HumidityData,
    ld: &LifeSupportData,
    map: &AirMap,
    handlers: &[life_support::Unit],
    scrubbers: &[life_support::Unit],
) -> Vec<(String, u8)> {
    let watts_word = |w: f64| format!("{w:.0} W");
    let Some((h, rooms)) = world.query::<&SoilMemory>().iter().next().map(|(_, m)| (m.home_air.clone(), m.rooms.clone())) else {
        return Vec::new();
    };
    if !(h.vapour_g_m3 > 0.0) {
        return Vec::new();
    }
    let t = map.home_temp_c;
    let rh = d.rh_of(h.vapour_g_m3, t).min(1.0);
    let ppm = life_support::co2_ppm(d, h.co2_g_m3, t);
    let o2 = h.o2_g_m3 / life_support::pure_g_m3(d, ld.o2_molar_mass, t).max(1e-9) * 100.0;
    let own_h: Vec<_> = handlers.iter().filter(|u| u.room.is_none()).collect();
    let own_s: Vec<_> = scrubbers.iter().filter(|u| u.room.is_none()).collect();
    let mut parts = Vec::new();
    let mut level = 0u8;
    if own_h.is_empty() {
        parts.push("no air handler in it".to_string());
    } else if !own_h.iter().any(|u| u.powered) {
        parts.push("air handler off (no power)".to_string());
        level = 1;
    } else if h.air_handler <= 0.0 {
        parts.push("air handler idle".to_string());
    } else {
        let w: f64 = own_h.iter().filter(|u| u.powered).map(|u| u.watts * ld.fan_power_share(h.air_handler)).sum();
        parts.push(format!(
            "air handler{} at {:.0}%, {}, {:.0} L of water a day back to the tanks",
            if own_h.len() > 1 { "s" } else { "" },
            h.air_handler * 100.0,
            watts_word(w),
            h.condensate_l_day
        ));
    }
    if own_s.is_empty() {
        parts.push("no CO2 scrubber".to_string());
    } else if !own_s.iter().any(|u| u.powered) {
        parts.push("CO2 scrubber off (no power)".to_string());
        level = 1;
    } else if h.scrubber <= 0.0 {
        parts.push(format!("CO2 scrubber idle (it runs above {} ppm)", thousands(ld.scrubber_setpoint_ppm)));
    } else {
        let w: f64 = own_s.iter().filter(|u| u.powered).map(|u| u.watts * h.scrubber).sum();
        parts.push(format!("CO2 scrubber at {:.0}%, {}, {:.1} kg of CO2 a day overboard", h.scrubber * 100.0, watts_word(w), h.scrubbed_kg_day));
    }
    if rh > ld.home_setpoint_rh + 0.05 || ppm > ld.scrubber_setpoint_ppm * 1.02 {
        level = 1;
    }
    let mut lines = vec![(
        format!(
            "Home air {:.0}% humidity, {} ppm CO2, {:.2}% oxygen: {}",
            (rh * 100.0).clamp(0.0, 100.0),
            thousands(ppm),
            o2,
            parts.join("; ")
        ),
        level,
    )];
    let net_o2 = h.o2_made_kg_day - h.o2_used_kg_day;
    lines.push((
        format!(
            "A day: the household and the mushrooms breathe out {:.1} kg of CO2 and the crops take up {:.1} kg; the crops give out {:.1} kg of oxygen and {:.1} kg is breathed in, so the home's oxygen {} {:.2} kg a day",
            h.co2_out_kg_day,
            h.co2_uptake_kg_day,
            h.o2_made_kg_day,
            h.o2_used_kg_day,
            if net_o2 >= 0.0 { "gains" } else { "loses" },
            net_o2.abs()
        ),
        0,
    ));
    let drawn = data
        .get::<Mutex<f32>>("irrigation_demand_lpm")
        .and_then(|m| m.lock().ok().map(|v| f64::from(*v) * 1440.0))
        .unwrap_or(0.0);
    let returned = h.condensate_l_day + rooms.values().map(|a| a.condensate_l_day).sum::<f64>();
    lines.push((
        format!(
            "Water a day: the garden draws {:.0} L from the tanks (its crops' breath and tissue, the humidifiers) and the air handlers give {:.0} L back; the air leaks {:.1} kg overboard",
            drawn, returned, h.leak_kg_day
        ),
        0,
    ));
    lines
}

/// The Garden panel's words for the space heaters of one air (2026-10-05,
/// BUG-155, farming::heat), `room` an index into the map's rooms or None for
/// the home's own air: whether they have power, how much of the time their
/// thermostat runs them, and the air's temperature (`temp_c`, warmed
/// `warmed_k` above its own). None where no heater stands and nothing has
/// warmed the air: a room at its own temperature has nothing new to say.
fn heater_part(heaters: &[super::heat::Heater], room: Option<usize>, duty: f64, temp_c: f64, warmed_k: f64) -> Option<String> {
    let here: Vec<&super::heat::Heater> = heaters.iter().filter(|h| h.room == room).collect();
    if here.is_empty() {
        return (warmed_k > 0.05).then(|| format!("the air warmed to {temp_c:.1} °C"));
    }
    let name = if here.len() > 1 { "heaters" } else { "heater" };
    let set = here.iter().map(|h| h.setpoint_c).fold(f64::NEG_INFINITY, f64::max);
    if !here.iter().any(|h| h.powered) {
        return Some(format!("{name} off (no power), the air at {temp_c:.1} °C"));
    }
    if duty <= 0.0 {
        return Some(format!("{name} idle, the air at {temp_c:.1} °C (it heats below {set:.0} °C)"));
    }
    let w: f64 = here.iter().filter(|h| h.powered).map(|h| h.watts * duty).sum();
    Some(if duty >= 0.999 && temp_c < set - 0.05 {
        format!("{name} flat out, {} W, the air at {temp_c:.1} °C, short of the {set:.0} °C it is set to", thousands(w))
    } else {
        format!("{name} on {:.0}% of the time, {} W, holding the air at {set:.0} °C", duty * 100.0, thousands(w))
    })
}

/// The Garden panel's humidity (built once a frame in lib.rs): a line per
/// grow area, and the crop card's "Humidity" row.
#[derive(Debug, Clone, Default)]
pub struct GuiView {
    data: Option<HumidityData>,
    map: AirMap,
    state: HashMap<String, RoomAir>,
    /// (area tag, the line, how humid: 0 fine, 1 above the fans' setpoint, 2
    /// at the damp diseases' line), for each grow area with a living crop. In
    /// a humidified room, 1 means its humidifier cannot run (no power or no
    /// water) or the air is past its fans' raised setpoint, and there is no 2.
    pub areas: Vec<(String, String, u8)>,
    /// The home's own air and its loops (2026-09-26, ship life support): a
    /// few lines for the top of the Garden panel, each with how it stands (0
    /// fine, 1 a machine cannot keep up or has no power, 2 the air is past a
    /// limit). Empty without a home air space or the life-support data.
    pub home: Vec<(String, u8)>,
    /// The garden's Off / Gentle / Realistic setting (`garden_pest_severity`),
    /// which scales what stale air costs a fungus (2026-09-27).
    severity: f32,
}

impl GuiView {
    pub fn new(world: &hecs::World, data: &DataStore) -> Self {
        let Some(d) = data.get::<HumidityData>(DATA_KEY) else { return Self::default() };
        let map = AirMap::new(world, data, d);
        let state: HashMap<String, RoomAir> = world
            .query::<&SoilMemory>()
            .iter()
            .next()
            .map(|(_, m)| m.rooms.clone())
            .unwrap_or_default();
        let fans = room_fans(world, &map);
        let hums = room_humidifiers(world, &map);
        let life = data.get::<LifeSupportData>(life_support::DATA_KEY);
        let (handlers, scrubbers) = life_units(world, &map);
        let heaters = super::heat::heaters(world, &map);
        // Who powers the air machines (Settings: Ship life support).
        let watts_word = |w: f64| format!("{w:.0} W");
        let mut tags: Vec<String> = world
            .query::<&CropInstance>()
            .iter()
            .filter(|(_, c)| c.growth_stage != STAGE_DEAD)
            .map(|(_, c)| c.tower_id.clone().unwrap_or_default())
            .collect();
        tags.sort();
        tags.dedup();
        let pct = |rh: f64| format!("{:.0}%", (rh * 100.0).clamp(0.0, 100.0));
        let areas = tags
            .into_iter()
            .map(|area| {
                let rh = map.rh_for(d, &area, &state);
                let mut humidified = None;
                // A mushroom tent over its CO2 limit, or its CO2 fan without
                // power (2026-09-27).
                let mut co2_warn = false;
                let line = if super::is_field_area(&area) {
                    format!("Outdoor air {} humidity", pct(rh))
                } else if let Some((i, room)) = map.area_room.get(&area).and_then(|i| map.rooms.get(*i).map(|r| (*i, r))) {
                    let st = state.get(&room.id).copied().unwrap_or_default();
                    let here: Vec<_> = fans.iter().filter(|f| f.room == i && !f.on_co2()).collect();
                    let co2_fans: Vec<_> = fans.iter().filter(|f| f.room == i && f.on_co2()).collect();
                    let hum: Vec<_> = hums.iter().filter(|h| h.1 == i).collect();
                    let mut parts = Vec::new();
                    if let Some(ach) = room.air_changes_per_hour {
                        // A fruiting tent: its own fresh air is set by its CO2.
                        parts.push(format!("fresh air {:.0} m3 an hour for its CO2", ach * room.volume_m3()));
                    }
                    if here.is_empty() {
                        // A humidified room, a tent or a room with a CO2 fan
                        // says nothing of a fan it need not have.
                        if hum.is_empty() && room.air_changes_per_hour.is_none() && co2_fans.is_empty() {
                            parts.push("no exhaust fan, its own leakage only".to_string());
                        }
                    } else if !here.iter().any(|f| f.powered) {
                        parts.push("exhaust fan off (no power)".to_string());
                    } else if st.fan_speed <= 0.0 {
                        parts.push("exhaust fan idle".to_string());
                    } else {
                        let w: f64 = here
                            .iter()
                            .filter(|f| f.powered)
                            .map(|f| f.watts * st.fan_speed.powf(d.fan_power_exponent.max(1.0)))
                            .sum();
                        parts.push(format!("exhaust fan at {:.0}%, {w:.0} W", st.fan_speed * 100.0));
                    }
                    // Its CO2 fans (2026-09-27): switched on by their controller.
                    if !co2_fans.is_empty() {
                        let set = co2_fans.iter().map(|f| f.co2_set).fold(f64::INFINITY, f64::min);
                        parts.push(if !co2_fans.iter().any(|f| f.powered) {
                            co2_warn = true;
                            "CO2 fan off (no power)".to_string()
                        } else if st.co2_fan <= 0.0 {
                            format!("CO2 fan idle (it runs above {} ppm)", thousands(set))
                        } else {
                            let w: f64 = co2_fans.iter().filter(|f| f.powered).map(|f| f.watts * st.co2_fan).sum();
                            format!("CO2 fan on {:.0}% of the time, {w:.0} W", st.co2_fan * 100.0)
                        });
                    }
                    if !hum.is_empty() {
                        let running = hum.iter().any(|h| h.4) && !st.humidifier_dry;
                        parts.push(if !hum.iter().any(|h| h.4) {
                            "humidifier off (no power)".to_string()
                        } else if st.humidifier_dry {
                            "humidifier stopped: no water from the tanks".to_string()
                        } else if st.humidifier <= 0.0 {
                            "humidifier idle".to_string()
                        } else {
                            let w: f64 = hum.iter().filter(|h| h.4).map(|h| h.3 * st.humidifier).sum();
                            format!(
                                "humidifier at {:.0}%, {w:.0} W, {:.0} L of water a day",
                                st.humidifier * 100.0,
                                st.humidifier_l_day
                            )
                        });
                        humidified = Some(running);
                    }
                    // Ship life support (2026-09-26): its air handlers and its
                    // carbon dioxide.
                    if let Some(ld) = life {
                        let here: Vec<_> = handlers.iter().filter(|u| u.room == Some(i)).collect();
                        if !here.is_empty() {
                            parts.push(if !here.iter().any(|u| u.powered) {
                                "air handler off (no power)".to_string()
                            } else if st.air_handler <= 0.0 {
                                "air handler idle".to_string()
                            } else {
                                let w: f64 = here.iter().filter(|u| u.powered).map(|u| u.watts * ld.fan_power_share(st.air_handler)).sum();
                                format!(
                                    "air handler{} at {:.0}%, {}, {:.0} L of water a day back to the tanks",
                                    if here.len() > 1 { "s" } else { "" },
                                    st.air_handler * 100.0,
                                    watts_word(w),
                                    st.condensate_l_day
                                )
                            });
                        }
                        if st.co2_g_m3 > 0.0 {
                            let ppm = life_support::co2_ppm(d, st.co2_g_m3, room_temp_at(d, st.warmed_k));
                            let mut co2 = format!("CO2 {} ppm", thousands(ppm));
                            if room.substrate_kg > 0.0 && ppm > d.fruiting_co2_limit_ppm {
                                co2_warn = true;
                                co2.push_str(&format!(
                                    ", over the {} the mushrooms fruit under: long stems, small caps and a smaller crop",
                                    thousands(d.fruiting_co2_limit_ppm)
                                ));
                            } else if st.co2_uptake_g_day > 0.0 {
                                co2.push_str(&format!(", its crops take up {:.1} kg a day", st.co2_uptake_g_day / 1000.0));
                            }
                            parts.push(co2);
                        }
                    }
                    // Its space heaters and its temperature (2026-10-05, BUG-155,
                    // farming::heat), whenever a heater stands in it or has warmed it.
                    if let Some(h) = heater_part(&heaters, Some(i), st.heater, room_temp_at(d, st.warmed_k), st.warmed_k) {
                        parts.push(h);
                    }
                    format!(
                        "Air {} humidity in the {} ({:.0} L a day breathed out): {}",
                        pct(rh),
                        room.name,
                        st.breathed_l_day,
                        parts.join("; ")
                    )
                } else {
                    format!("Home air {} humidity", pct(rh))
                };
                // A humidified room is damp on purpose: warn only when its
                // humidifier cannot run, or the air is past its fans' line
                // (both sides at the same saturation, so the room's warmth cancels).
                let level = match humidified {
                    Some(running) => u8::from(!running || rh * d.room_saturation() > fan_set(d, true, d.room_saturation())),
                    None if rh >= d.notice_above_rh => 2,
                    None if rh > d.fan_setpoint_rh => 1,
                    None => 0,
                };
                (area, line, level.max(u8::from(co2_warn)))
            })
            .collect();
        let mut home = match (life, map.home_known) {
            (Some(ld), true) => home_lines(world, data, d, ld, &map, &handlers, &scrubbers),
            _ => Vec::new(),
        };
        // The heaters in the home's own air (2026-10-05, BUG-155): a line of
        // their own, so the home's air line keeps its shape.
        if map.home_known {
            let h = world.query::<&SoilMemory>().iter().next().map(|(_, m)| (m.home_air.heater, m.home_air.warmed_k)).unwrap_or_default();
            if let Some(part) = heater_part(&heaters, None, h.0, map.home_temp_c, h.1) {
                home.push((format!("Home air: {part}"), 0));
            }
        }
        let severity = data
            .get::<Mutex<f32>>("garden_pest_severity")
            .and_then(|m| m.lock().ok().map(|v| *v))
            .filter(|v| v.is_finite())
            .unwrap_or(super::pests::DEFAULT_PEST_SEVERITY)
            .clamp(0.0, 1.0);
        Self { data: Some(d.clone()), map, state, areas, home, severity }
    }

    /// The crop card's "Humidity" row: the air it grows in against its
    /// window, and the cap when it is outside. "" when there is no data or
    /// the air there is not known (`AirMap::known_rh`).
    pub fn crop_row(&self, crop: &CropInstance, def: Option<&PlantDef>) -> String {
        let Some(d) = &self.data else { return String::new() };
        let Some(rh) = self.map.known_rh(d, crop.tower_id.as_deref().unwrap_or(""), &self.state) else {
            return String::new();
        };
        let mut s = format!("{:.0}%", (rh * 100.0).clamp(0.0, 100.0));
        if let Some(def) = def.filter(|p| p.humidity_max > 0.0) {
            s.push_str(&format!(
                " (grows best at {:.0}% to {:.0}%)",
                def.humidity_min * 100.0,
                def.humidity_max * 100.0
            ));
            let cap = health_ceiling(d, Some(def), rh);
            if cap < 99.5 {
                s.push_str(&format!(", holds health to {cap:.0}%"));
            }
        }
        // A fungus's carbon dioxide (2026-09-27): stale air costs it its crop.
        if let Some(def) = def.filter(|p| !p.needs_light) {
            if let Some(ppm) = self.map.known_co2_ppm(d, crop.tower_id.as_deref().unwrap_or(""), &self.state) {
                let limit = d.fungus_co2_limit_ppm(&def.id);
                s.push_str(&format!("; CO2 {} ppm", thousands(ppm)));
                if ppm > limit {
                    s.push_str(&format!(", over the {} it fruits under", thousands(limit)));
                    let cap = co2_ceiling(d, Some(def), ppm, self.severity);
                    if cap < 99.5 {
                        s.push_str(&format!(", holds health to {cap:.0}%"));
                    }
                }
            }
        }
        s
    }
}
