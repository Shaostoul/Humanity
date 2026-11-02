//! From systems/farming/life_support_tests.rs (its `shipped` module): the shipped homes' air and
//! water, the home built the way the engine builds it (`engine::home_spawn`: its machines, its air
//! space and the household's food energy).

use std::collections::HashMap;
use humanity_core::ecs::components::{AirHandler, Co2Scrubber, CropInstance, PowerConsumer, SoilMemory};
use humanity_core::ecs::systems::System;
use humanity_core::hot_reload::data_store::DataStore;
use crate::systems::atmosphere::EnclosedSpace;
use crate::systems::farming::humidity;
use crate::systems::farming::life_support_tests::*;
use crate::systems::farming::lighting::GrowPlot;
use crate::systems::farming::*;
use crate::systems::life_support::{self, LifeSupportData};

/// A shipped home's garden at full planting, built the way the engine builds
/// it: the rooms the engine publishes, through the same two calls (the ship's
/// flood-filled rooms made `RoomBounds` by `RoomTypeRegistry::room_bounds`, published
/// by `humidity::publish_room_bounds`, as world entry and lib.rs do), every
/// machine of the home zone spawned by `spawn_home_machine_entity` at its
/// placed offset with its own wiring's islands, the grow plots and plot areas
/// `publish_grow_plots` makes, the household's air space, and every plot sown
/// with the crop its grow medium and the showcase give it. The Commons'
/// machines (another zone, another frame) are left out.
///
/// Until 2026-10-21 the rooms were built here from the home's ZONES, which
/// the engine does not read for air: the greenhouse's zone was a storey tall
/// (2,970 m3) while the room the game published rose to the 6 m glass roof
/// (5,607 m3), so these tests measured a home the game did not run.
fn shipped_home(file: &str) -> (DataStore, hecs::World) {
    let root = std::path::Path::new(env!("HUMANITY_REPO_ROOT"));
    let data_dir = root.join("data");
    let home = crate::systems::grow_machines::load_home(&data_dir.join("machines").join(file)).expect("home parses");
    let ship = crate::ship::ship_structure::ShipStructure::load_and_assemble_shipped(&data_dir, None).expect("the shipped ship assembles");
    // The machines below stand at their zone-local offsets, which are their ship
    // positions (and so in the rooms' frame) because the home stands on p1, at the origin.
    let hz = &ship.zones[ship.home_zone_index()];
    assert_eq!(hz.origin, (0.0, 0.0, 0.0), "the shipped home stands at the ship's origin");
    let room_types = crate::ship::room_types::RoomTypeRegistry::load(&data_dir);
    let rooms = room_types.room_bounds(&ship.generate_meshes().room_info);
    let model = crate::systems::grow_machines::GrowFoodModel::load(&data_dir).expect("grow model");
    let all: Vec<_> = home.all_instances().into_iter().filter(|i| i.zone == "home").collect();
    let mut plots = Vec::new();
    let mut areas = HashMap::new();
    let mut sow: Vec<(String, String, u32)> = Vec::new();
    for inst in &all {
        let def = &home.catalog[&inst.machine];
        let Some(medium) = model.media.iter().find(|m| m.matches(&inst.machine)) else { continue };
        let Some(plan) = model.plan(&inst.machine, def.size) else { continue };
        let footprint = def.size.0 * def.size.2;
        let cups: u32 = plan.plots.iter().filter(|p| p.1.is_none()).map(|p| p.2 as u32).sum();
        plots.push(GrowPlot {
            id: inst.id.clone(),
            aliases: Vec::new(),
            pos: [inst.offset.0, inst.offset.1, inst.offset.2],
            footprint_m2: footprint,
            cups,
            outdoors: is_field_area(&inst.machine) || is_field_area(&inst.id),
            enclosure: medium.enclosure.clone(),
        });
        let mut slot = 0u32;
        for (crop, per_plot, n) in &plan.plots {
            if let Some(a) = per_plot {
                areas.insert(inst.id.clone(), *a);
            }
            for _ in 0..(*n as u32) {
                sow.push((inst.id.clone(), crop.clone(), slot));
                slot += 1;
            }
        }
    }
    let mut data = store(Vec::new(), plots);
    humidity::publish_room_bounds(&data, &rooms);
    data.insert(units::PLOT_AREA_KEY, areas);
    let mut world = hecs::World::new();
    let (power, water) = (home.electrical_islands(&all), home.water_islands(&all));
    for inst in &all {
        crate::engine::home_spawn::spawn_home_machine_entity(&mut world, inst, &home.catalog[&inst.machine], &power, &water, None, None);
    }
    // As big as the shipped home (ship homes increment 4: each home its own air), the shipped
    // design and not this checkout's own home, which a Save can change.
    let shipped = crate::ship::ship_structure::HomeDesign::built_in("homestead").expect("the shipped homestead");
    let home_m3 = crate::ship::ship_space::home_air_volume_m3(&shipped.body);
    crate::engine::home_spawn::spawn_home_air_space(&mut world, crate::engine::home_spawn::home_metabolic_kcal(&home), home_m3);
    for (area, plant, slot) in sow {
        world.spawn((crop(&data, &plant, &area, slot, 0.0),));
    }
    (data, world)
}

/// What a shipped home's air settles to over a day, with day and night.
#[derive(Debug, Default)]
struct HomeDay {
    greenhouse_rh: f64,
    home_rh: f64,
    tents_rh: (f64, f64),
    co2_ppm: (f64, f64),
    tents_co2_ppm: (f64, f64),
    handler_w: f64,
    scrubber_w: f64,
    fan_w: f64,
    humidifier_w: f64,
    breathed_indoor_l: f64,
    tents_breath_l: f64,
    humidifier_l: f64,
    fields_l: f64,
    kept_l: f64,
    people_l: f64,
    surface_l: f64,
    drawn_l: f64,
    returned_l: f64,
    co2_out_kg: f64,
    co2_uptake_kg: f64,
    o2_made_kg: f64,
    o2_used_kg: f64,
    leak_kg: f64,
    /// Each machine type's draw over the day (2026-09-27): the average
    /// watts of all its units together, and how many units there are.
    by_type: std::collections::BTreeMap<String, (f64, usize)>,
    /// The mushroom room's own CO2 and humidity, the day's range.
    mushroom_room_ppm: (f64, f64),
    mushroom_room_rh: (f64, f64),
    /// The day record the machines' draw wrote over the two days (2026-10-22).
    recorded: crate::machines::day_energy::DayEnergy,
}

/// Run a shipped home a game day to settle, then one to measure, each tick
/// 6 s at 1x (a tenth of a game hour: one slice, the clock turning day and
/// night), with `tweak` applied to the life-support data.
fn settle_home(file: &str, tweak: impl Fn(&mut LifeSupportData)) -> HomeDay {
    settle_home_with(file, tweak, |_| {})
}

/// `settle_home`, with `tweak_world` applied to the world after its first
/// tick (when the air has its state): a machine's setpoint, the home's CO2.
fn settle_home_with(file: &str, tweak: impl Fn(&mut LifeSupportData), tweak_world: impl Fn(&mut hecs::World)) -> HomeDay {
    use humanity_core::ecs::components::{Humidifier, MachineType, RoomAir, Ventilator};
    let (mut data, mut world) = shipped_home(file);
    let mut ld = life();
    tweak(&mut ld);
    data.insert(life_support::DATA_KEY, ld);
    // The day record the meters read (2026-10-22), written each tick as the machines' power
    // tick writes it, over the tick's game seconds.
    crate::machines::day_energy::register(&mut data);
    let d = air();
    let mut sys = FarmingSystem::new();
    let dt = 6.0_f32;
    let per_day = (1200.0 / dt) as usize;
    for tick in 0..per_day {
        advance(&data, f64::from(dt));
        sys.tick(&mut world, dt, &data);
        crate::machines::day_energy::record_world(&world, &data, f64::from(dt) * OLD_S);
        if tick == 0 {
            tweak_world(&mut world);
        }
    }
    let mut day = HomeDay {
        co2_ppm: (f64::INFINITY, 0.0),
        tents_rh: (1.0, 0.0),
        tents_co2_ppm: (f64::INFINITY, 0.0),
        mushroom_room_ppm: (f64::INFINITY, 0.0),
        mushroom_room_rh: (1.0, 0.0),
        ..Default::default()
    };
    let map = humidity::AirMap::new(&world, &data, &d);
    println!("  {file} the home's own air {:.0} m3, in {} rooms counted for the leak", map.home_volume_m3, map.modules);
    for r in map.rooms.iter().filter(|r| !r.id.starts_with("tent:")) {
        println!("  {file} {}: {:.1} m3, {:?} to {:?}", r.id, r.volume_m3(), r.min, r.max);
    }
    let surface0 = home(&world).ledger.surface_l;
    let watts = |world: &hecs::World, pick: fn(&hecs::EntityRef) -> bool| -> f64 {
        world.iter().filter(|e| pick(e)).filter_map(|e| e.get::<&PowerConsumer>().map(|p| f64::from(p.draw_watts))).sum()
    };
    for _ in 0..per_day {
        advance(&data, f64::from(dt));
        sys.tick(&mut world, dt, &data);
        crate::machines::day_energy::record_world(&world, &data, f64::from(dt) * OLD_S);
        let m = memory(&world);
        let n = per_day as f64;
        day.greenhouse_rh += m.rooms.get("room-greenhouse").map_or(0.0, |a: &RoomAir| d.rh_of(a.vapour_g_m3, d.room_temp_c)) / n;
        day.home_rh += d.rh_of(m.home_air.vapour_g_m3, map.home_temp_c) / n;
        for (id, a) in &m.rooms {
            if id.starts_with("tent:") {
                let rh = d.rh_of(a.vapour_g_m3, d.room_temp_c);
                day.tents_rh = (day.tents_rh.0.min(rh), day.tents_rh.1.max(rh));
                let ppm = life_support::co2_ppm(&d, a.co2_g_m3, d.room_temp_c);
                day.tents_co2_ppm = (day.tents_co2_ppm.0.min(ppm), day.tents_co2_ppm.1.max(ppm));
            }
        }
        let ppm = life_support::co2_ppm(&d, m.home_air.co2_g_m3, map.home_temp_c);
        day.co2_ppm = (day.co2_ppm.0.min(ppm), day.co2_ppm.1.max(ppm));
        if let Some(a) = m.rooms.get("room-mushroom") {
            let (p, r) = (life_support::co2_ppm(&d, a.co2_g_m3, d.room_temp_c), d.rh_of(a.vapour_g_m3, d.room_temp_c));
            day.mushroom_room_ppm = (day.mushroom_room_ppm.0.min(p), day.mushroom_room_ppm.1.max(p));
            day.mushroom_room_rh = (day.mushroom_room_rh.0.min(r), day.mushroom_room_rh.1.max(r));
        }
        for (_, (mt, pc)) in world.query::<(&MachineType, &PowerConsumer)>().iter() {
            let e = day.by_type.entry(mt.0.clone()).or_insert((0.0, 0));
            e.0 += f64::from(pc.draw_watts) / n;
        }
        day.handler_w += watts(&world, |e| e.has::<AirHandler>()) / n;
        day.scrubber_w += watts(&world, |e| e.has::<Co2Scrubber>()) / n;
        day.fan_w += watts(&world, |e| e.has::<Ventilator>()) / n;
        day.humidifier_w += watts(&world, |e| e.has::<Humidifier>()) / n;
        day.breathed_indoor_l += m.rooms.iter().filter(|(id, _)| !id.starts_with("tent:")).map(|(_, a)| a.breathed_l_day).sum::<f64>() / n;
        day.drawn_l += demand_lpm(&data) * 1440.0 / n;
        day.returned_l += returned_lpm(&world) * 1440.0 / n;
        day.co2_out_kg += m.home_air.co2_out_kg_day / n;
        day.co2_uptake_kg += m.home_air.co2_uptake_kg_day / n;
        day.o2_made_kg += m.home_air.o2_made_kg_day / n;
        day.o2_used_kg += m.home_air.o2_used_kg_day / n;
        day.leak_kg = m.home_air.leak_kg_day;
        day.tents_breath_l += m.rooms.iter().filter(|(id, _)| id.starts_with("tent:")).map(|(_, a)| a.breathed_l_day).sum::<f64>() / n;
        day.humidifier_l += m.rooms.values().map(|a| a.humidifier_l_day).sum::<f64>() / n;
    }
    // What the crops' irrigation carries besides their breath: the fields'
    // (outdoors, into the weather) and the water every crop's tissue keeps.
    let ld = life();
    let reg = data.get::<PlantRegistry>("plant_registry").unwrap();
    let areas = data.get::<HashMap<String, f32>>(units::PLOT_AREA_KEY);
    for (_, c) in world.query::<&CropInstance>().iter() {
        let Some(def) = reg.get(&c.crop_def_id) else { continue };
        let et = f64::from(def.water_per_day) * f64::from(units::crop_plants(c, Some(reg), areas));
        if c.tower_id.as_deref().map_or(false, is_field_area) {
            day.fields_l += et;
        }
        day.kept_l += et * ld.exchange_for(&c.crop_def_id).kept_l_per_l;
    }
    day.people_l = ld.breath(map.home_kcal).water_g_day / 1000.0;
    day.surface_l = home(&world).ledger.surface_l - surface0;
    for (_, (mt, _)) in world.query::<(&humanity_core::ecs::components::MachineType, &PowerConsumer)>().iter() {
        if let Some(e) = day.by_type.get_mut(&mt.0) {
            e.1 += 1;
        }
    }
    let m = memory(&world);
    let mut rooms: Vec<_> = m.rooms.iter().filter(|(id, _)| !id.starts_with("tent:")).collect();
    rooms.sort_by(|a, b| a.0.cmp(b.0));
    for (id, a) in rooms {
        println!("  {file} {id}: {:.1}% humidity, {:.0} ppm, {:.1} L a day breathed, air handler {:.2}, {:.1} L a day back", d.rh_of(a.vapour_g_m3, d.room_temp_c) * 100.0, life_support::co2_ppm(&d, a.co2_g_m3, d.room_temp_c), a.breathed_l_day, a.air_handler, a.condensate_l_day);
    }
    println!("  {file} ledger {:?}", m.home_air.ledger);
    println!("{file}: {day:#?}");
    day.recorded = crate::machines::day_energy::snapshot(&data);
    day
}

/// Both shipped homes hold their air and hand the garden's water back, at
/// full planting over a day and a night: the greenhouse and the court (whose
/// towers breathe out about 106 L a day) at their air handlers' 75%, the
/// home's own air at 50%, no water condensing on any wall, every mushroom
/// tent still inside the oyster's 85 to 95%, the carbon dioxide under the
/// scrubber's 2,636 ppm, and the coils returning, to within 1%, every litre
/// the indoor crops, the tents, the humidifiers and the household put in the
/// air. The printout is the balance the homes' Energy, Water and Air loops
/// quote (docs/design/ship-life-support.md). Seen red before the court had
/// its air handler: the family home's court sat at 100% and about 45 L a day
/// condensed on its walls, lost to the tanks.
///
/// Since 2026-09-27 (the CO2 fans): every tent is also held under the
/// 1,000 ppm its oysters fruit under, the mushroom room under its fan's
/// setpoint, and every catalog `average_watts` (what the static power
/// meter charges a controller-driven machine for the day) is the draw the
/// day measured, to within 5% or 1 W a unit. Seen red two ways: with the
/// tent CO2 fans moved out of home.ron's tents (the family tents then sat
/// at about 1,200 ppm, 600 over the room its own fan still held at 600),
/// and with home.ron's air handler `average_watts` left at its nameplate
/// 325 W (the day measured 224.8 W a unit).
///
/// Since 2026-10-21 the homes are measured on the rooms the running game
/// publishes (`shipped_home`), where the greenhouse rises to the 6 m roof
/// (nearly twice the air of the storey-tall zone the test used to read),
/// and the home's own air leaks for its own rooms only. Seen red first
/// that day on the game's rooms: "home.ron: home 0.5263749356690928" (the
/// greenhouse, leaking half its whole 6 m box an hour into the house, put
/// more water in the house's air than its four handlers held at 50%), with
/// the leak at 0.96 kg a day for the ship's 48 published rooms where the
/// home has 30.
///
/// Since 2026-10-22 the day also writes the record the meters read
/// (`machines::day_energy`), and the test holds it to what the day measured:
/// each type's recorded days add up to its average draw, and the power
/// meter read from the record is within 5% of the meter on the data's
/// estimates. Seen red (2026-10-23) with `MachineHome::day_draw` reading
/// only the data: "home.ron: the meter read the record".
#[test]
fn the_shipped_homes_hold_their_air_and_return_their_water() {
    let ld = life();
    let d = air();
    let root = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data").join("machines");
    let data_dir = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data");
    let ship = crate::ship::ship_structure::ShipStructure::load_and_assemble_shipped(&data_dir, None).expect("the shipped ship assembles");
    let home_rooms = ship.zones[ship.home_zone_index()].body.detect_rooms().len();
    for file in ["home.ron", "home_solo.ron"] {
        let day = settle_home(file, |_| {});
        // The home's own air leaks for the home's own rooms (BVAD's figure a module).
        assert!(
            (day.leak_kg - ld.leak_kg_per_day_per_module * home_rooms as f64).abs() < 1e-9,
            "{file}: the home's air leaks {} kg a day, {} for each of its {home_rooms} rooms",
            day.leak_kg,
            ld.leak_kg_per_day_per_module
        );
        assert!((day.greenhouse_rh - ld.grow_room_setpoint_rh).abs() < 0.02, "{file}: greenhouse {}", day.greenhouse_rh);
        assert!((day.home_rh - ld.home_setpoint_rh).abs() < 0.02, "{file}: home {}", day.home_rh);
        assert!(day.tents_rh.0 >= 0.85 && day.tents_rh.1 <= 0.95, "{file}: tents {:?}", day.tents_rh);
        assert!(day.co2_ppm.1 <= ld.scrubber_setpoint_ppm * 1.03, "{file}: CO2 {:?}", day.co2_ppm);
        assert!(day.surface_l < 0.01, "{file}: {} L condensed on walls, lost to the tanks", day.surface_l);
        let into_air = day.breathed_indoor_l + day.tents_breath_l + day.humidifier_l + day.people_l;
        assert!((day.returned_l - into_air).abs() < 0.01 * into_air, "{file}: {} L returned of {into_air} L put in the air", day.returned_l);
        // The mushrooms' air (2026-09-27).
        assert!(day.tents_co2_ppm.1 < d.fruiting_co2_limit_ppm, "{file}: tents at {:?} ppm, over the {}", day.tents_co2_ppm, d.fruiting_co2_limit_ppm);
        let home = crate::systems::grow_machines::load_home(&root.join(file)).expect("home parses");
        let room_set = f64::from(home.catalog["room_co2_fan"].co2_setpoint_ppm);
        // The room under its fan's setpoint, with 2% for an evening peak its S6
        // cannot hold flat out (it held 600 at the family home's, the home's air
        // near 495 ppm, on 2026-10-21; it reached about 604 on 2026-09-27).
        assert!(day.mushroom_room_ppm.1 <= room_set * 1.02, "{file}: the mushroom room at {:?} ppm, over its fan's {room_set}", day.mushroom_room_ppm);
        // The static meter's average draws are the measured ones.
        for (id, def) in &home.catalog {
            let Some(crate::machines::MachinePower::Consumer { average_watts: Some(avg), .. }) = def.power else { continue };
            let (total, units) = day.by_type.get(id).copied().unwrap_or_default();
            assert!(units > 0, "{file}: `{id}` carries average_watts but none is placed");
            let measured = total / units as f64;
            assert!(
                (measured - f64::from(avg)).abs() <= (0.05 * measured).max(1.0),
                "{file}: `{id}` averages {measured:.1} W a unit over the day, its catalog says {avg}"
            );
        }
        // The day record holds what the day measured (2026-10-22): each type's units'
        // recorded days add up to what the day averaged, and the power meter read with the
        // record charges about what it charges on the data's estimates, which is what a new
        // game shows until its machines have run a day.
        let placed: Vec<_> = home.all_instances().into_iter().filter(|i| i.zone == "home").collect();
        for (id, (total, _)) in &day.by_type {
            let recorded: f64 = placed.iter().filter(|i| &i.machine == id).filter_map(|i| day.recorded.day_of(&i.id, &i.machine)).map(|d| d.average_w()).sum();
            assert!((recorded - total).abs() <= 1e-6 * total.max(1.0), "{file}: `{id}`'s units recorded {recorded} W over the day, the day averaged {total} W");
        }
        let power = |recorded: Option<&crate::machines::day_energy::DayEnergy>| {
            let basis = crate::machines::MeterBasis { life_support_on_grid: true, recorded };
            home.utility_meters(4.5, basis).into_iter().find(|m| m.utility == "power").expect("a power meter")
        };
        let (with_record, estimated) = (power(Some(&day.recorded)), power(None));
        assert!(with_record.measured_and_estimated().0 > 0, "{file}: the meter read the record");
        assert!(
            (with_record.demand - estimated.demand).abs() <= 0.05 * estimated.demand,
            "{file}: the meter charges {:.2} kWh a day as measured, {:.2} on the data's estimates",
            with_record.demand,
            estimated.demand
        );
    }
}

/// The same two homes with the home's own air held at 60% (the EPA's
/// "below 60 percent" line) instead of 50%: what the drier home costs.
/// A measurement for docs/design/ship-life-support.md, not a guard.
#[test]
#[ignore]
fn print_the_shipped_homes_with_the_home_at_60_percent() {
    for file in ["home.ron", "home_solo.ron"] {
        settle_home(file, |l| l.home_setpoint_rh = 0.60);
    }
}

/// Set the CO2 fans' setpoints: the tents' and the mushroom room's.
fn co2_setpoints(tent: f32, room: f32) -> impl Fn(&mut hecs::World) {
    move |w: &mut hecs::World| {
        for (_, (mt, v)) in w.query_mut::<(&humanity_core::ecs::components::MachineType, &mut humanity_core::ecs::components::Ventilator)>() {
            match mt.0.as_str() {
                "tent_co2_fan" => v.co2_setpoint_ppm = tent,
                "room_co2_fan" => v.co2_setpoint_ppm = room,
                _ => {}
            }
        }
    }
}

/// The mushroom CO2 fans at other setpoints (2026-09-27): what a lower or
/// higher setpoint costs in fan and humidifier power, and whether the
/// tents' small humidifiers still hold 90%. The measurements behind the
/// setpoints docs/design/ship-life-support.md quotes; not a guard.
#[test]
#[ignore]
fn print_the_mushroom_co2_fans_at_other_setpoints() {
    for file in ["home.ron", "home_solo.ron"] {
        for (tent, room) in [(800.0, 600.0), (800.0, 500.0), (900.0, 600.0), (900.0, 550.0), (950.0, 650.0)] {
            println!("== {file}: tents held at {tent} ppm, the mushroom room at {room}");
            let day = settle_home_with(file, |_| {}, co2_setpoints(tent, room));
            println!("== {file} {tent}/{room}: tents {:?} ppm {:?} RH, room {:?} ppm, fans {:.1} W, humidifiers {:.1} W", day.tents_co2_ppm, day.tents_rh, day.mushroom_room_ppm, day.fan_w, day.humidifier_w);
        }
    }
}

/// Both homes with the home's own air started where its carbon dioxide
/// settles (2026-09-27): the family home's crops only take up all it makes
/// near 800 ppm (Kimball's law), weeks of play after a new game starts at
/// 400; the solo home's near 470. Whether the mushroom tents can still be
/// held there, and at what cost. A measurement, not a guard. The family
/// home is started at 700 and at 800 (2026-10-21), the two points the Air
/// loop of data/machines/home.ron puts its settling point between.
#[test]
#[ignore]
fn print_the_shipped_homes_with_the_home_air_at_its_settled_co2() {
    for (file, ppm) in [("home.ron", 700.0), ("home.ron", 800.0), ("home_solo.ron", 450.0)] {
        println!("== {file}: the home's air started at {ppm} ppm of CO2");
        settle_home_with(file, |_| {}, move |w: &mut hecs::World| {
            let t_c = w
                .query::<&EnclosedSpace>()
                .iter()
                .next()
                .map_or(20.0, |(_, s)| f64::from(s.atmosphere.temperature_k) - 273.15);
            let g = life_support::co2_g_m3(&air(), ppm, t_c);
            for (_, m) in w.query_mut::<&mut SoilMemory>() {
                m.home_air.co2_g_m3 = g;
            }
        });
    }
}
