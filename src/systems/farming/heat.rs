//! Space heaters and the air they warm (2026-10-05, BUG-155).
//!
//! Until this, a heater the player built (the Build Heater recipe, the step
//! the Greenhouse Construction quest asks for) warmed nothing: the grow rooms
//! sat at `room_temp_c` and the home's own air at the temperature its air
//! space was made at, whatever stood in them. Every number and its source live
//! in data/garden/humidity.ron (THE HEAT), and each heater's watts and
//! thermostat in its machine catalog (data/machines: `heats_w`,
//! `heat_setpoint_c`); this file is the model, and `humidity::step_rooms`
//! steps it with the water and the gases of the same airs.
//!
//! THE AIRS are the ones the home already keeps (humidity.rs and
//! life_support.rs): each grow room's and fruiting tent's own air, and the
//! home's own air for everything else. A heater warms the air it stands in,
//! found the way an air handler's is (`AirMap::room_at`): the smallest grow
//! room or tent around it, else the home's own air. A heater in a room with no
//! grow machine therefore warms the home's own air, all of it: that is the
//! limit of keeping one air for the whole home.
//!
//! THE BALANCE, per air, in theta, how far the heaters have warmed it above
//! its own temperature (K), with C its heat capacity (J/K):
//!
//! ```text
//! C dtheta/dt = Q + Q_in - G (theta - theta_around) - G_coil theta
//! ```
//!
//! `Q` is its heaters' watts for the share of the time their thermostat runs
//! them; `Q_in` what the tents in a room, or the grow rooms of the home, passed
//! it; `G` its loss to the air around it (a tent's room, a room's home air, and
//! for the home's own air the station around the home) through its walls and
//! ceiling (`envelope_u_w_m2_k` times their area) and with the air it exchanges
//! (its air changes an hour times C / 3600); `G_coil` the heat its air
//! handlers' cold coils take out of the air they move. It is linear with
//! constant coefficients over a slice, so `life_support::relax` solves it
//! exactly, as it does the water and the gases, and an air settles at
//! `theta = Q / G` over the air around it.
//!
//! Linear also means a heater's heat comes on top of whatever holds each air at
//! its own temperature now (the station's climate, the sun, the crops' and
//! humidifiers' evaporation, the lights, the people): none of that is modelled,
//! and the heater changes none of it. With no heater anywhere every theta stays
//! exactly 0 and every air is where it was before this file existed.
//!
//! THE THERMOSTAT. A heater runs flat out while the air is under its setpoint,
//! for the share of the time that holds the air at it once there, and not at
//! all above it. Over a slice that is the one steady share that ends the slice
//! on the setpoint (`step`), the way the CO2 fans' controller is solved
//! (`humidity::co2_fan_duty`); because the balance is linear in the heat, the
//! share comes out in closed form. Several heaters in one air share one
//! thermostat, set at the highest of theirs.
//!
//! WHAT READS IT: a grow room's air is at `room_temp_c` plus its theta
//! (`humidity::room_temp_at`), the temperature its humidity, its carbon
//! dioxide, its air handlers' coils and its pests read; the home's own air is
//! written back into its air space at its own temperature plus its theta
//! (the farming tick), which the body heat model reads inside the home
//! (engine::survival_env, which reads a grow room's own air where the player
//! stands in one) and the Air readout shows.
//!
//! THREE KINDS OF HEATER (2026-10-20). An electric resistance heater turns
//! what it draws into the same watts of heat. A HEAT PUMP (`HeatPump`) moves
//! heat in from the station's air around the home, which the station holds at
//! the home's own temperature: its heat is its draw times its coefficient of
//! performance at that temperature, read off its spec sheet's table
//! (`machines::cop_at`), and only its draw is charged as electricity. A STOVE
//! (`FuelStove`) draws no power and runs while its box holds fuel: it burns
//! its kilograms an hour at its full heat, for the share of the time its
//! thermostat runs it (`stoke`, `burn`), the box losing each piece once it has
//! burned through (`burn_stoves`), all of it exact in f64 so the same game
//! hours burn the same wood at any frame rate. Its fire takes the oxygen it
//! burns out of the home's air (the one air that keeps oxygen), and its smoke,
//! carbon dioxide, water and carbon monoxide go up its flue, out of the home.
//! Not modelled: the excess air a real stove pulls through its fire and sends
//! up the flue with the smoke, and the air that has to come in to replace it
//! (the home's air keeps no pressure, so it cannot lose or take in air as a
//! whole), a flue that blocks or spills back, and carbon monoxide in the
//! home's air (nothing tracks it there). The thermostat stands in for the air
//! control a person turns down on a real stove.
//!
//! WHAT IT LEAVES OUT: the heat stored in walls, benches, soil and crops (only
//! the air's own heat capacity is counted, so a room warms in minutes where a
//! real one takes longer; where it settles is the same); a room's real walls
//! (every grow room is taken as single glass, humidity.ron); a heater's radiant
//! warmth on a body standing next to it; and every other room of the home
//! keeping its own air.
//!
//! COST: per step, a few exp() for each air the home keeps, and nothing
//! drawn or networked.

use std::collections::HashMap;

use crate::ecs::components::{BurnsFuels, FuelStove, HeatPump, PowerConsumer, SpaceHeater, Transform};
use crate::systems::inventory::containers::Container;
use crate::systems::inventory::ItemRegistry;
use crate::systems::life_support;

use super::humidity::{AirMap, GrowRoom, HumidityData};

/// What a heater runs on (2026-10-20), for its heat and the Garden panel's
/// words.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HeaterKind {
    /// An electric resistance heater: power in, the same watts of heat out.
    #[default]
    Electric,
    /// A heat pump (`HeatPump`): power in, that times its coefficient of
    /// performance out, the rest moved in from the air outside the home.
    HeatPump,
    /// A stove (`FuelStove`): no power; it burns the fuel in its box.
    Stove,
}

impl HeaterKind {
    /// What the Garden panel calls one.
    pub fn word(self) -> &'static str {
        match self {
            Self::Electric => "heater",
            Self::HeatPump => "heat pump",
            Self::Stove => "stove",
        }
    }
}

/// A placed space heater the air step drives: its entity, the air it stands in
/// (a grow room or tent, an index into `AirMap::rooms`, or None for the home's
/// own air), its heat at full output and its draw (W), its thermostat (C), and
/// whether it can run (an electric heater or a heat pump: it has power; a
/// stove: it has fuel in its box).
///
/// A STOVE (2026-10-20) also carries its heat while it burns at full
/// (`full_heat_w`), the kilograms of fuel an hour that takes (`kg_per_h`), the
/// oxygen each kilogram burns (`o2_kg_per_kg`), the fuel left in its box
/// (`fuel_kg`, spent slice by slice through the step) and the share of the
/// current slice that fuel lasts at full (`avail`, `stoke`): its `heat_w` for
/// the slice is `full_heat_w` times that, so a box that runs out part-way
/// through a slice gives exactly the heat and burns exactly the fuel it had,
/// whatever the slice's length.
#[derive(Debug, Clone, Copy)]
pub struct Heater {
    pub entity: hecs::Entity,
    pub room: Option<usize>,
    pub heat_w: f64,
    pub watts: f64,
    pub setpoint_c: f64,
    pub powered: bool,
    pub kind: HeaterKind,
    pub full_heat_w: f64,
    pub kg_per_h: f64,
    pub o2_kg_per_kg: f64,
    pub fuel_kg: f64,
    pub avail: f64,
}

/// The placed heaters, by the air they stand in. A heat pump's heat is its
/// draw times its coefficient of performance at the temperature of the air it
/// draws from: the station's air around the home, which the station holds at
/// the home's own temperature (`AirMap::home_own_c`; it is also the air the
/// home's heat is lost to, `step`). A stove's fuel is `fuel_kg` (by entity,
/// `stove_fuel_kg`; none listed is an empty box).
pub fn heaters(world: &hecs::World, map: &AirMap, fuel_kg: &HashMap<hecs::Entity, f64>) -> Vec<Heater> {
    world
        .query::<(&SpaceHeater, &Transform, Option<&PowerConsumer>, Option<&HeatPump>, Option<&FuelStove>)>()
        .iter()
        .map(|(e, (h, t, pc, pump, stove))| {
            let watts = f64::from(h.watts).max(0.0);
            let powered = pc.map_or(false, |p| p.enabled);
            let mut out = Heater {
                entity: e,
                room: map.room_at(t.position.to_array()),
                heat_w: f64::from(h.heat_w).max(0.0),
                watts,
                setpoint_c: f64::from(h.setpoint_c),
                powered,
                kind: HeaterKind::Electric,
                full_heat_w: f64::from(h.heat_w).max(0.0),
                kg_per_h: 0.0,
                o2_kg_per_kg: 0.0,
                fuel_kg: 0.0,
                avail: 1.0,
            };
            if let Some(s) = stove {
                out.kind = HeaterKind::Stove;
                out.watts = 0.0;
                out.kg_per_h = f64::from(s.kg_per_h).max(0.0);
                out.o2_kg_per_kg = f64::from(s.o2_kg_per_kg).max(0.0);
                out.fuel_kg = fuel_kg.get(&e).copied().filter(|f| f.is_finite()).unwrap_or(0.0).max(0.0);
                out.powered = out.fuel_kg > 0.0;
            } else if let Some(p) = pump {
                out.kind = HeaterKind::HeatPump;
                let cop = crate::machines::cop_at(&p.cop_by_source_c, map.home_own_c).unwrap_or(0.0).max(0.0);
                out.heat_w = watts * cop;
                out.full_heat_w = out.heat_w;
            }
            out
        })
        .collect()
}

/// Ready each stove for a slice of `hours` (2026-10-20): the share of the
/// slice its fuel lasts at full (`Heater::avail`, at most 1), its heat for the
/// slice (`full_heat_w` times that share), and whether it can run at all (it
/// has fuel). Anything that is not a stove is left as it is.
pub fn stoke(heaters: &mut [Heater], hours: f64) {
    for h in heaters.iter_mut().filter(|h| h.kind == HeaterKind::Stove) {
        h.avail = if !(h.fuel_kg > 0.0) {
            0.0
        } else if h.kg_per_h > 0.0 && hours > 0.0 {
            (h.fuel_kg / (h.kg_per_h * hours)).min(1.0)
        } else {
            1.0
        };
        h.heat_w = h.full_heat_w * h.avail;
        h.powered = h.avail > 0.0;
    }
}

/// What the stoves of one air (`room`) burned over a slice of `hours` while
/// their thermostat ran them `duty` of it (2026-10-20): each burns its
/// `kg_per_h` for the share of the slice its fuel lasted at full and its
/// thermostat ran it, never more than its box held; its box's fuel goes down
/// by that and `burned` (kg, by entity) up. Returns the grams of oxygen they
/// burned, all of it out of the air they stand in. What they burn into goes up
/// the flue: none of it enters the home's air.
pub fn burn(heaters: &mut [Heater], room: Option<usize>, duty: f64, hours: f64, burned: &mut HashMap<hecs::Entity, f64>) -> f64 {
    let mut o2_g = 0.0;
    for h in heaters.iter_mut().filter(|h| h.kind == HeaterKind::Stove && h.room == room && h.powered) {
        let kg = (h.kg_per_h * h.avail * duty.clamp(0.0, 1.0) * hours.max(0.0)).min(h.fuel_kg).max(0.0);
        h.fuel_kg -= kg;
        *burned.entry(h.entity).or_insert(0.0) += kg;
        o2_g += kg * h.o2_kg_per_kg * 1000.0;
    }
    o2_g
}

/// The fuel in each stove's box now, kg, by entity (2026-10-20): its pieces
/// times each piece's mass (`data/items.csv`), less what has burned of the one
/// at the front (`FuelStove::burned_kg`). A box holding nothing the stove
/// burns (its `BurnsFuels`) holds no fuel. With no item registry nothing has a
/// mass, so nothing burns.
pub fn stove_fuel_kg(world: &hecs::World, items: Option<&ItemRegistry>) -> HashMap<hecs::Entity, f64> {
    let Some(items) = items else { return HashMap::new() };
    world
        .query::<(&FuelStove, &Container, &BurnsFuels)>()
        .iter()
        .filter_map(|(e, (s, c, burns))| {
            let item = c.current_content_item.as_deref().filter(|i| burns.burns(i))?;
            let mass = f64::from(items.mass_for(item)).max(0.0);
            let kg = (f64::from(c.current_qty) * mass - s.burned_kg.max(0.0)).max(0.0);
            (kg > 0.0).then_some((e, kg))
        })
        .collect()
}

/// Take what the stoves burned (kg, by entity: `burn`) out of their boxes
/// (2026-10-20): it goes onto the piece at the front (`FuelStove::burned_kg`),
/// and each piece burned through leaves the box, its litres with it. A box
/// burned empty forgets the partial piece and keeps its residue, as a drum run
/// dry does. Exact in f64, so no frame's share is ever lost.
pub fn burn_stoves(world: &mut hecs::World, items: Option<&ItemRegistry>, burned: &HashMap<hecs::Entity, f64>) {
    let Some(items) = items else { return };
    for (e, kg) in burned {
        if !(*kg > 0.0) {
            continue;
        }
        let Ok((s, c)) = world.query_one_mut::<(&mut FuelStove, &mut Container)>(*e) else { continue };
        let Some(item) = c.current_content_item.clone() else { continue };
        let mass = f64::from(items.mass_for(&item));
        if !(mass > 0.0) {
            continue;
        }
        s.burned_kg += kg;
        // A piece is gone once all but a rounding's worth of it has burned.
        while c.current_qty > 0 && s.burned_kg >= mass - 1e-9 {
            let per_unit = c.used_liters / c.current_qty as f32;
            c.current_qty -= 1;
            c.used_liters = (c.used_liters - per_unit).max(0.0);
            s.burned_kg = (s.burned_kg - mass).max(0.0);
        }
        if c.current_qty == 0 {
            c.current_content_item = None;
            c.used_liters = 0.0;
            s.burned_kg = 0.0;
        }
    }
}

/// The heaters of one air (`room`, as in `Heater::room`): the full heat of the
/// ones that can run and of all of them (W, for a shed one's request; a stove
/// with no fuel adds nothing to either, since no power would light it), and
/// their thermostat (the highest setpoint among them, C), None when there are
/// none.
pub fn heat_in(heaters: &[Heater], room: Option<usize>) -> (f64, f64, Option<f64>) {
    let here = heaters.iter().filter(|h| h.room == room);
    let powered: f64 = here.clone().filter(|h| h.powered).map(|h| h.heat_w).sum();
    let all: f64 = here.clone().map(|h| h.heat_w).sum();
    let set = here.map(|h| h.setpoint_c).filter(|s| s.is_finite()).fold(None, |m: Option<f64>, s| Some(m.map_or(s, |m| m.max(s))));
    (powered, all, set)
}

/// What warming a cubic metre of air by a degree takes, J: its density at
/// `t_c` (P / (R T), `life_support::air_kg_m3`) times air's specific heat
/// (humidity.ron, FAO-56's cp).
pub fn heat_capacity_j_m3_k(d: &HumidityData, t_c: f64) -> f64 {
    life_support::air_kg_m3(d, t_c) * d.air_specific_heat_j_kg_k.max(0.0)
}

/// A room's or tent's walls and ceiling, m2: the surface it loses its heat
/// through (the floor is not counted, as greenhouse sizing does not count it;
/// humidity.ron, THE HEAT).
pub fn envelope_m2(r: &GrowRoom) -> f64 {
    let [w, h, dz] = [0, 1, 2].map(|i| f64::from((r.max[i] - r.min[i]).max(0.0)));
    2.0 * (w + dz) * h + w * dz
}

/// The home's own air's walls and roof, m2: one storey `storey_m` high holding
/// `volume_m3` on a square floor (it has no box of its own: it is the home
/// less its grow rooms).
pub fn home_envelope_m2(volume_m3: f64, storey_m: f64) -> f64 {
    if !(volume_m3 > 0.0) {
        return 0.0;
    }
    let h = storey_m.max(0.1);
    let floor = volume_m3 / h;
    floor + 4.0 * floor.sqrt() * h
}

/// How fast an air gives heat to the air around it, W/K: through `envelope_m2`
/// of walls and ceiling at `envelope_u_w_m2_k`, and with the air it exchanges,
/// `air_changes_h` of its `volume_m3` an hour, each cubic metre carrying
/// `cap_j_m3_k` a degree.
pub fn conductance_w_k(d: &HumidityData, envelope_m2: f64, air_changes_h: f64, volume_m3: f64, cap_j_m3_k: f64) -> f64 {
    d.envelope_u_w_m2_k.max(0.0) * envelope_m2.max(0.0) + cap_j_m3_k.max(0.0) * air_changes_h.max(0.0) * volume_m3.max(0.0) / 3600.0
}

/// One air's warmth over one slice: its state and what bears on it.
#[derive(Debug, Clone, Copy, Default)]
pub struct AirHeat {
    /// How far it is warmed above its own temperature at the slice's start, K.
    pub warmed_k: f64,
    /// Its heat capacity, J/K (0: no air, nothing to warm).
    pub capacity_j_k: f64,
    /// How fast it gives heat to the air around it (`conductance_w_k`), W/K,
    /// and how far that air is warmed, K (0 for the station around the home).
    pub around_w_k: f64,
    pub around_k: f64,
    /// How fast its air handlers' coils take its warmth, W/K.
    pub coil_w_k: f64,
    /// The heat the airs inside it passed it this slice, W.
    pub in_w: f64,
    /// Its heaters' full heat, W: the powered ones', and all of them.
    pub heat_w: f64,
    pub heat_all_w: f64,
    /// Their thermostat as a warming, K (its setpoint less the air's own
    /// temperature); None when it has no heater.
    pub set_k: Option<f64>,
}

/// What one slice did to one air's warmth.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Warmth {
    /// How far it is warmed at the slice's end, K.
    pub warmed_k: f64,
    /// Its heaters' share of the slice on, and the share they would ask for if
    /// all of them had power.
    pub duty: f64,
    pub duty_req: f64,
    /// The heat it gave the air around it, and its coils took, over the
    /// slice, J (negative when the air around it warmed it).
    pub passed_j: f64,
    pub coil_j: f64,
}

/// Step one air's warmth `hours` game hours (humidity.ron, THE HEAT): its
/// heaters run for the share of the slice that ends it on their thermostat
/// (flat out when even that does not reach it, off when it ends the slice
/// there without them), and the balance is solved exactly
/// (`life_support::relax`).
pub fn step(a: &AirHeat, hours: f64) -> Warmth {
    let c = a.capacity_j_k;
    let x0 = if a.warmed_k.is_finite() { a.warmed_k.max(0.0) } else { 0.0 };
    if !(c > 0.0) {
        return Warmth { warmed_k: x0, ..Default::default() };
    }
    // Per hour: the warmth the heat adds, and how fast each sink takes it.
    let k_around = a.around_w_k.max(0.0) * 3600.0 / c;
    let k_coil = a.coil_w_k.max(0.0) * 3600.0 / c;
    let sinks = [(k_around, a.around_k.max(0.0)), (k_coil, 0.0)];
    let source = |q_w: f64| (a.in_w + q_w) * 3600.0 / c;
    let end = |q_w: f64| life_support::relax(x0, source(q_w), &sinks, hours, f64::INFINITY).x;
    // The share of the slice that ends it on the setpoint. The end is linear in
    // the heat, so it is a straight line between none and all of it.
    let duty_for = |q_max: f64| -> f64 {
        let Some(set) = a.set_k.filter(|s| s.is_finite()) else { return 0.0 };
        if !(q_max > 0.0) {
            return 0.0;
        }
        let (none, full) = (end(0.0), end(q_max));
        if none >= set {
            0.0
        } else if full <= set {
            1.0
        } else {
            ((set - none) / (full - none)).clamp(0.0, 1.0)
        }
    };
    let duty = duty_for(a.heat_w);
    let duty_req = duty_for(a.heat_all_w);
    let r = life_support::relax(x0, source(duty * a.heat_w), &sinks, hours, f64::INFINITY);
    Warmth {
        warmed_k: r.x,
        duty,
        duty_req,
        passed_j: c * r.to_sink(sinks[0].0, sinks[0].1, hours.max(0.0)),
        coil_j: c * r.to_sink(k_coil, 0.0, hours.max(0.0)),
    }
}
