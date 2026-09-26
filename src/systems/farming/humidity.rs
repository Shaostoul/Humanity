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
//! THE FANS: an exhaust fan (`Ventilator`) standing in a room exchanges its air
//! with the home's while its PowerConsumer is enabled. Its controller runs it
//! flat out above `fan_setpoint_rh`, at the speed that holds the setpoint once
//! there, and idle below (`fan_speed`); it draws its watts times the cube of
//! its speed (the fan laws), which this step writes into its PowerConsumer.
//!
//! WHAT IT DRIVES: the diseases' Humidity condition (pests.rs) and a gentle
//! health cap on a crop outside its plants.csv humidity window
//! (`health_ceiling`), never below the house floor.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Deserialize;

use crate::ecs::components::{CropInstance, PowerConsumer, RoomAir, SoilMemory, Transform, Ventilator, STAGE_DEAD};
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
    pub notice_above_rh: f64,
    pub stress_loss_max: f64,
    pub stress_full_at_rh: f64,
    pub health_floor: f32,
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

/// The highest health a crop can hold in air at `rh` (0..1): 100 inside its
/// plants.csv humidity window, down to 100 x (1 - stress_loss_max) as the air
/// goes `stress_full_at_rh` or further outside it, never below the floor.
/// 100 for an unknown plant or one with no window.
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
    let loss = d.stress_loss_max.clamp(0.0, 1.0) * (off / d.stress_full_at_rh.max(1e-9)).min(1.0);
    ((100.0 * (1.0 - loss)) as f32).clamp(d.health_floor, 100.0)
}

// -- The rooms ------------------------------------------------------------------------

/// One room of the home, as the engine's room bounds give it: an axis-aligned
/// box in world metres.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GrowRoom {
    pub id: String,
    pub name: String,
    pub min: [f32; 3],
    pub max: [f32; 3],
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
            .map(|(id, name, min, max)| GrowRoom { id: id.to_string(), name: room_name(id, name), min, max })
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
    /// The rooms that hold a grow machine.
    pub rooms: Vec<GrowRoom>,
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
                let Some(r) = boxes
                    .iter()
                    .filter(|r| r.contains(p.pos))
                    .min_by(|a, b| a.volume_m3().total_cmp(&b.volume_m3()))
                else {
                    continue;
                };
                let idx = match map.rooms.iter().position(|x| x.id == r.id) {
                    Some(i) => i,
                    None => {
                        map.rooms.push(r.clone());
                        map.rooms.len() - 1
                    }
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
/// grow areas breathe out (`breathed`, L a day by area tag), exchanged with
/// the home's air by its leakage and its fans, which this sets the speed and
/// the draw of. Rooms the map no longer knows are forgotten. Returns the
/// notices to say: a room that has just become humid enough for the damp
/// diseases.
pub fn step_rooms(
    world: &mut hecs::World,
    d: &HumidityData,
    pests: &PestData,
    map: &AirMap,
    state: &mut HashMap<String, RoomAir>,
    breathed: &HashMap<String, f64>,
    hours: f64,
) -> Vec<String> {
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
    let sat = d.room_saturation();
    let set = d.fan_setpoint_rh.clamp(0.0, 1.0) * sat;
    let mut speed = vec![0.0f64; map.rooms.len()];
    for (i, room) in map.rooms.iter().enumerate() {
        let volume = room.volume_m3().max(1.0);
        let source = litres[i] * d.vapour_share.max(0.0) * 1000.0 / 24.0 / volume;
        let fan_max: f64 = fans.iter().filter(|f| f.1 == i && f.4).map(|f| f.2).sum::<f64>() / volume;
        let st = state.entry(room.id.clone()).or_insert(RoomAir { vapour_g_m3: map.home_vapour, ..Default::default() });
        let base = d.base_air_changes_per_hour.max(0.0);
        let s = fan_speed(st.vapour_g_m3, source, base, fan_max, map.home_vapour, set);
        st.vapour_g_m3 = step_vapour(st.vapour_g_m3, source, base + s * fan_max, map.home_vapour, hours, sat);
        st.fan_speed = s;
        st.breathed_l_day = litres[i];
        speed[i] = s;
        let rh = d.rh_of(st.vapour_g_m3, d.room_temp_c);
        if rh >= d.notice_above_rh && !st.told {
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
    notices
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
    let volume = room.volume_m3().max(1.0);
    let memory = super::soil::soil_memory_entity(world);
    let Ok(mut mem) = world.get::<&mut SoilMemory>(memory) else { return String::new() };
    let st = mem.rooms.entry(room.id.clone()).or_insert(RoomAir { vapour_g_m3: map.home_vapour, ..Default::default() });
    let before = st.vapour_g_m3;
    st.vapour_g_m3 = map.home_vapour + (before - map.home_vapour) * (-air_changes.max(0.0)).exp();
    let (rh0, rh1) = (d.rh_of(before, d.room_temp_c).min(1.0), d.rh_of(st.vapour_g_m3, d.room_temp_c).min(1.0));
    // How long until the crops breathe it back: to the disease line, or to
    // where it was if that was lower, at the air change it has now.
    let source = st.breathed_l_day * d.vapour_share.max(0.0) * 1000.0 / 24.0 / volume;
    let fan_max: f64 = fans.iter().filter(|f| Some(f.1) == ri && f.4).map(|f| f.2).sum::<f64>() / volume;
    let n = d.base_air_changes_per_hour.max(0.0) + st.fan_speed * fan_max;
    let target = before.min(d.notice_above_rh * d.room_saturation());
    let back = if n > 1e-12 {
        let eq = map.home_vapour + source / n;
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
    /// at the damp diseases' line), for each grow area with a living crop.
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
                let line = if super::is_field_area(&area) {
                    format!("Outdoor air {} humidity", pct(rh))
                } else if let Some((i, room)) = map.area_room.get(&area).and_then(|i| map.rooms.get(*i).map(|r| (*i, r))) {
                    let st = state.get(&room.id).copied().unwrap_or_default();
                    let here: Vec<_> = fans.iter().filter(|f| f.1 == i).collect();
                    let vent = if here.is_empty() {
                        "no exhaust fan, its own leakage only".to_string()
                    } else if !here.iter().any(|f| f.4) {
                        "exhaust fan off (no power)".to_string()
                    } else if st.fan_speed <= 0.0 {
                        "exhaust fan idle".to_string()
                    } else {
                        let w: f64 = here
                            .iter()
                            .filter(|f| f.4)
                            .map(|f| f.3 * st.fan_speed.powf(d.fan_power_exponent.max(1.0)))
                            .sum();
                        format!("exhaust fan at {:.0}%, {w:.0} W", st.fan_speed * 100.0)
                    };
                    format!(
                        "Air {} humidity in the {} ({:.0} L a day breathed out): {vent}",
                        pct(rh),
                        room.name,
                        st.breathed_l_day
                    )
                } else {
                    format!("Home air {} humidity", pct(rh))
                };
                let level = if rh >= d.notice_above_rh {
                    2
                } else if rh > d.fan_setpoint_rh {
                    1
                } else {
                    0
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
