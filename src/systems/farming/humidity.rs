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
//! WHAT IT DRIVES: the diseases' Humidity condition (pests.rs) and a health
//! cap on a crop outside its plants.csv humidity window (`health_ceiling`),
//! never below the house floor: gentle for a green crop, steep for a fungus
//! (plants.csv `needs_light` false) below its fruiting range.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Deserialize;

use crate::ecs::components::{
    CropInstance, Humidifier, PowerConsumer, RoomAir, SoilMemory, Transform, Ventilator, STAGE_DEAD,
};
use crate::hot_reload::data_store::DataStore;

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
        if let Ok(text) = std::fs::read_to_string(&path) {
            match Self::parse(&text) {
                Ok(d) => return d,
                Err(e) => log::warn!("[Farming] {} does not parse ({e}); using the shipped copy", path.display()),
            }
        }
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
            .map(|(id, name, min, max)| GrowRoom { id: id.to_string(), name: room_name(id, name), min, max, air_changes_per_hour: None })
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
            .map(|(_, (_, s))| (s.atmosphere.humidity, s.atmosphere.temperature_k));
        map.home_known = home.is_some();
        let (rh, t_c) = home.unwrap_or_else(|| {
            let a = Atmosphere::default();
            (a.humidity, a.temperature_k)
        });
        map.home_rh = f64::from(rh).clamp(0.0, 1.0);
        map.home_vapour = d.vapour_at(map.home_rh, f64::from(t_c) - 273.15);
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
                let v = state.get(&r.id).map_or(self.home_vapour, |a| a.vapour_g_m3);
                d.rh_of(v, d.room_temp_c).min(1.0)
            }
            None => self.home_rh,
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

    /// The room a fan at `pos` stands in, as an index into `rooms`.
    fn room_at(&self, pos: [f32; 3]) -> Option<usize> {
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
    };
    room.air_changes_per_hour = Some(d.tent_fresh_air_m3_h(f64::from(e.substrate_kg)) / room.volume_m3().max(1e-6));
    room
}

/// The exhaust fans, by the room they stand in: (entity, room index, full
/// m3/h, full-speed watts, powered).
fn room_fans(world: &hecs::World, map: &AirMap) -> Vec<(hecs::Entity, usize, f64, f64, bool)> {
    world
        .query::<(&Ventilator, &Transform, Option<&PowerConsumer>)>()
        .iter()
        .filter_map(|(e, (v, t, pc))| {
            let room = map.room_at(t.position.to_array())?;
            Some((e, room, f64::from(v.airflow_m3_h), f64::from(v.watts), pc.map_or(false, |p| p.enabled)))
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

/// The vapour the fans of a room hold it under, g/m3: `fan_setpoint_rh`, or,
/// in a room a humidifier holds, `humidified_fan_margin_rh` above the
/// humidifier's setpoint, so the two never work against each other.
fn fan_set(d: &HumidityData, humidified: bool) -> f64 {
    let rh = if humidified {
        d.fan_setpoint_rh.max(d.humidifier_setpoint_rh + d.humidified_fan_margin_rh)
    } else {
        d.fan_setpoint_rh
    };
    rh.clamp(0.0, 1.0) * d.room_saturation()
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

/// Step every grow room's air `hours` game hours: the vapour the crops of its
/// grow areas breathe out (`breathed`, L a day by area tag) and its
/// humidifiers put in, exchanged with the home's air by its leakage and its
/// fans; this sets the fans' speed, the humidifiers' output and the draw of
/// both. The humidifiers run only while `water_ok` (the home has water for the
/// garden). Rooms the map no longer knows are forgotten. Returns the notices
/// to say (a room that has just become humid enough for the damp diseases)
/// and the litres a day the humidifiers are turning into vapour, which the
/// caller bills to the home's tanks with the irrigation.
#[allow(clippy::too_many_arguments)]
pub fn step_rooms(
    world: &mut hecs::World,
    d: &HumidityData,
    pests: &PestData,
    map: &AirMap,
    state: &mut HashMap<String, RoomAir>,
    breathed: &HashMap<String, f64>,
    water_ok: bool,
    hours: f64,
) -> (Vec<String>, f64) {
    let mut notices = Vec::new();
    // A room the home no longer has is forgotten, but only once the engine
    // has published the rooms at all: until then (early boot, before the
    // world is entered) a loaded save's rooms are kept as they were.
    if !map.rooms.is_empty() {
        state.retain(|id, _| map.rooms.iter().any(|r| r.id == *id));
    }
    let mut litres = vec![0.0f64; map.rooms.len()];
    for (area, l) in breathed {
        if let Some(i) = map.area_room.get(area) {
            litres[*i] += l.max(0.0);
        }
    }
    let fans = room_fans(world, map);
    let hums = room_humidifiers(world, map);
    let sat = d.room_saturation();
    let hum_set = d.humidifier_setpoint_rh.clamp(0.0, 1.0) * sat;
    let mut speed = vec![0.0f64; map.rooms.len()];
    let mut share = vec![0.0f64; map.rooms.len()];
    let mut hum_l_day = 0.0f64;
    // An enclosure (a fruiting tent) first: what it vents is a source of
    // vapour for the room it stands in, g an hour, stepped after it.
    let mut vented = vec![0.0f64; map.rooms.len()];
    let order: Vec<usize> = (0..map.rooms.len())
        .filter(|i| map.parent[*i].is_some())
        .chain((0..map.rooms.len()).filter(|i| map.parent[*i].is_none()))
        .collect();
    for i in order {
        let room = &map.rooms[i];
        let volume = room.volume_m3().max(0.01);
        let source = (litres[i] * d.vapour_share.max(0.0) * 1000.0 / 24.0 + vented[i]) / volume;
        let outside = map.outside(i, state);
        let fan_max: f64 = fans.iter().filter(|f| f.1 == i && f.4).map(|f| f.2).sum::<f64>() / volume;
        let humidified = hums.iter().any(|h| h.1 == i);
        let powered_l_h: f64 = hums.iter().filter(|h| h.1 == i && h.4).map(|h| h.2).sum();
        // What the humidifiers can put in, g/m3 an hour: none without water.
        // (Tested with `>`: an empty float sum is -0.0, which would print.)
        let hum_max = if water_ok && powered_l_h > 0.0 { powered_l_h * 1000.0 / volume } else { 0.0 };
        let st = state.entry(room.id.clone()).or_insert(RoomAir { vapour_g_m3: outside, ..Default::default() });
        let base = room.air_changes_per_hour.unwrap_or(d.base_air_changes_per_hour).max(0.0);
        let s = fan_speed(st.vapour_g_m3, source, base, fan_max, outside, fan_set(d, humidified));
        let n = base + s * fan_max;
        let v0 = st.vapour_g_m3;
        let mut h = humidifier_share(v0, source, n, hum_max, outside, hum_set);
        st.vapour_g_m3 = step_vapour(v0, source + h * hum_max, n, outside, hours, sat);
        // Flat out, it reached the setpoint within the tick and held it from
        // there: the tick ends on the setpoint, not past it, however long it
        // was, and the output is the one that holds it.
        if h >= 1.0 && v0 < hum_set && st.vapour_g_m3 > hum_set {
            st.vapour_g_m3 = hum_set;
            h = humidifier_share(hum_set, source, n, hum_max, outside, hum_set);
        }
        // What it vents into the room around it: its air changes times its
        // excess over that room's air, at the vapour it ends the tick on.
        if let Some(p) = map.parent[i] {
            vented[p] += n * volume * (st.vapour_g_m3 - outside);
        }
        st.fan_speed = s;
        st.breathed_l_day = litres[i];
        st.humidifier = h;
        st.humidifier_l_day = h * hum_max * volume / 1000.0 * 24.0;
        // Powered, and stopped for want of water: dry tanks or the irrigation off.
        st.humidifier_dry = !water_ok && powered_l_h > 0.0;
        hum_l_day += st.humidifier_l_day;
        speed[i] = s;
        share[i] = h;
        let rh = d.rh_of(st.vapour_g_m3, d.room_temp_c);
        if humidified {
            // The damp is meant here (a mushroom room): no disease notice.
        } else if rh >= d.notice_above_rh && !st.told {
            st.told = true;
            let names = humid_diseases(pests, rh.min(1.0));
            let spread = if names.is_empty() { String::new() } else { format!(", where {} spread", names.join(", ")) };
            notices.push(format!(
                "The {} air is at {:.0}% humidity{spread}: its crops breathe out {:.0} L of water a day. \
                 Ventilate it from the Garden panel, or run an exhaust fan there.",
                room.name,
                (rh * 100.0).min(100.0),
                litres[i]
            ));
        } else if rh < d.notice_above_rh - 0.05 {
            st.told = false;
        }
    }
    // Each fan draws its watts at the cube of its speed (the fan laws); a
    // fan the electrical sim has shed stays where it was, off.
    for (e, room, _, watts, powered) in fans {
        if !powered {
            continue;
        }
        if let Ok(mut pc) = world.get::<&mut PowerConsumer>(e) {
            pc.draw_watts = (watts * speed[room].powf(d.fan_power_exponent.max(1.0))) as f32;
        }
    }
    // Each humidifier draws its watts in proportion to its output share
    // (humidity.ron, THE HUMIDIFIER); one shed stays where it was, off.
    for (e, room, _, watts, powered) in hums {
        if !powered {
            continue;
        }
        if let Ok(mut pc) = world.get::<&mut PowerConsumer>(e) {
            pc.draw_watts = (watts * share[room]) as f32;
        }
    }
    (notices, hum_l_day)
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
    let (rh0, rh1) = (d.rh_of(before, d.room_temp_c).min(1.0), d.rh_of(st.vapour_g_m3, d.room_temp_c).min(1.0));
    // How long until the crops breathe it back: to the disease line, or to
    // where it was if that was lower, at the air change it has now.
    let source = st.breathed_l_day * d.vapour_share.max(0.0) * 1000.0 / 24.0 / volume;
    let fan_max: f64 = fans.iter().filter(|f| Some(f.1) == ri && f.4).map(|f| f.2).sum::<f64>() / volume;
    let n = room.air_changes_per_hour.unwrap_or(d.base_air_changes_per_hour).max(0.0) + st.fan_speed * fan_max;
    let target = before.min(d.notice_above_rh * d.room_saturation());
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
            d.rh_of(target, d.room_temp_c).min(1.0) * 100.0,
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
                let line = if super::is_field_area(&area) {
                    format!("Outdoor air {} humidity", pct(rh))
                } else if let Some((i, room)) = map.area_room.get(&area).and_then(|i| map.rooms.get(*i).map(|r| (*i, r))) {
                    let st = state.get(&room.id).copied().unwrap_or_default();
                    let here: Vec<_> = fans.iter().filter(|f| f.1 == i).collect();
                    let hum: Vec<_> = hums.iter().filter(|h| h.1 == i).collect();
                    let mut parts = Vec::new();
                    if let Some(ach) = room.air_changes_per_hour {
                        // A fruiting tent: its fresh air is set by its CO2.
                        parts.push(format!("fresh air {:.0} m3 an hour for its CO2", ach * room.volume_m3()));
                    }
                    if here.is_empty() {
                        // A humidified room or a tent says nothing of a fan it need not have.
                        if hum.is_empty() && room.air_changes_per_hour.is_none() {
                            parts.push("no exhaust fan, its own leakage only".to_string());
                        }
                    } else if !here.iter().any(|f| f.4) {
                        parts.push("exhaust fan off (no power)".to_string());
                    } else if st.fan_speed <= 0.0 {
                        parts.push("exhaust fan idle".to_string());
                    } else {
                        let w: f64 = here
                            .iter()
                            .filter(|f| f.4)
                            .map(|f| f.3 * st.fan_speed.powf(d.fan_power_exponent.max(1.0)))
                            .sum();
                        parts.push(format!("exhaust fan at {:.0}%, {w:.0} W", st.fan_speed * 100.0));
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
                // humidifier cannot run, or the air is past its fans' line.
                let level = match humidified {
                    Some(running) => u8::from(!running || rh * d.room_saturation() > fan_set(d, true)),
                    None if rh >= d.notice_above_rh => 2,
                    None if rh > d.fan_setpoint_rh => 1,
                    None => 0,
                };
                (area, line, level)
            })
            .collect();
        Self { data: Some(d.clone()), map, state, areas }
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
        s
    }
}
