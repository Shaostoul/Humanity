//! The Garden data by area, kept until it changes (2026-10-31, docs/design/sim-frame-cost.md, "The
//! Garden views by area"; the module notes of garden_mirror.rs).
//!
//! What is proved here, against the real farm (the step_tests garden: two grow rooms with their
//! air, a tower, a field, every plant in each, every Garden dataset loaded, a bumblebee hive):
//! - the build that keeps rows gives, after every kind of change, exactly what a build from
//!   scratch gives the same frame (`every_kind_of_change_reaches_the_rows_the_same_build`): an
//!   area's own turn, a command, a reader's settle, a load, the data files, a machine edit, each
//!   setting, the tanks, the clock, the air and the light;
//! - and after a long run of turns and commands, frame by frame
//!   (`every_area_equals_a_build_from_scratch_after_a_long_run`);
//! - nothing is built again when nothing changed, and an area's own turn builds that area alone;
//! - the wall screen's budget (`pick`).
//!
//! Seen red 2026-10-31 with the area's own version left out of its key (`build` reading 0 for it):
//! "an area's own turn: the kept rows differ from a build from scratch", and "one turn built its
//! own areas, not every area: 0 of 4"; and with the farm's own bump left out of `FarmingSystem::run`
//! (`v.touch_area(name)` removed): "interval 1, frame 5: the kept rows differ from a build from
//! scratch".

use std::collections::HashMap;
use std::sync::Mutex;

use super::{build, pick, Rows};
use humanity_core::ecs::components::{CropInstance, MachineInstanceId, PollinatorHive, Transform};
use humanity_core::ecs::systems::System;
use crate::gui::{GardenPests, GuiCrop};
use humanity_core::hot_reload::data_store::DataStore;
use humanity_systems::farming::{self, fixtures::step, FarmingSystem};

/// The page's data as one text: every crop's row (its tile key included) and every per-area list,
/// the outlook sorted by crop.
fn snapshot(crops: &[GuiCrop], p: &GardenPests) -> String {
    let mut outlook: Vec<String> = p.outlook.iter().map(|(k, v)| format!("{k}: {v:?}")).collect();
    outlook.sort();
    let mut s = String::new();
    for c in crops {
        s.push_str(&format!("{c:?}\n"));
    }
    s.push_str(&format!(
        "pests {:?}\ncovers {:?}\nweeds {:?}\npollinate {:?}\nair {:?}\nhome {:?}\nph {:?}\n",
        p.areas, p.covers, p.weeds, p.pollinate_areas, p.air, p.home_air, p.ph_amendments
    ));
    for o in outlook {
        s.push_str(&o);
        s.push('\n');
    }
    s
}

/// The first line two snapshots differ on, for the failure message.
fn first_difference(a: &str, b: &str) -> String {
    match a.lines().zip(b.lines()).find(|(x, y)| x != y) {
        Some((x, y)) => format!("\n  kept:    {x}\n  scratch: {y}"),
        None => format!("\n  lengths {} vs {}", a.lines().count(), b.lines().count()),
    }
}

/// The garden: the step_tests one with its air, every Garden dataset and setting registered as at
/// boot, the farm's change numbers, a bumblebee hive beside the greenhouse bed, at `interval_s`.
fn garden(interval_s: f64) -> (DataStore, hecs::World, FarmingSystem) {
    let (mut data, mut world) = step::garden(true);
    farming::soil_ph::register(&mut data);
    farming::pollination::register(&mut data);
    farming::picking::register(&mut data);
    farming::weeds::register(&mut data);
    farming::versions::register(&mut data);
    data.insert("garden_pests", farming::pests::PestData::load());
    data.insert("garden_lighting", farming::lighting::LightingData::load());
    data.insert("soil_ph_request", Mutex::new(Option::<(String, String)>::None));
    data.insert(farming::clock::INTERVAL_KEY, Mutex::new(interval_s));
    world.spawn((PollinatorHive, MachineInstanceId("bumblebee_hive_1".into()), Transform { position: glam::Vec3::new(6.0, 0.0, 6.0), ..Default::default() }));
    let mut sys = FarmingSystem::new();
    sys.reuse_override = Some(true);
    (data, world, sys)
}

fn clock_now(data: &DataStore) -> f64 {
    data.get::<Mutex<humanity_systems::time::GameTime>>("game_time").unwrap().lock().unwrap().elapsed_seconds
}

fn set_clock(data: &DataStore, t: f64) {
    data.get::<Mutex<humanity_systems::time::GameTime>>("game_time").unwrap().lock().unwrap().set_elapsed(t);
}

fn frame(data: &DataStore, world: &mut hecs::World, sys: &mut FarmingSystem, dt: f32) {
    set_clock(data, clock_now(data) + f64::from(dt));
    sys.tick(world, dt, data);
}

/// What the page holds, and its kept rows.
#[derive(Default)]
struct Page {
    crops: Vec<GuiCrop>,
    pests: GardenPests,
    rows: Rows,
}

impl Page {
    /// A build that keeps rows; returns the snapshot.
    fn kept(&mut self, world: &hecs::World, data: &DataStore) -> String {
        build(world, data, &mut self.crops, &mut self.pests, &mut self.rows, true, false);
        snapshot(&self.crops, &self.pests)
    }

    /// The areas the last build built.
    fn built(&self) -> usize {
        self.rows.areas.iter().filter(|a| a.built_at == self.rows.builds).count()
    }
}

/// A build from scratch: nothing kept, the Dev page's switch off.
fn scratch(world: &hecs::World, data: &DataStore) -> String {
    let mut p = Page::default();
    build(world, data, &mut p.crops, &mut p.pests, &mut p.rows, false, false);
    snapshot(&p.crops, &p.pests)
}

/// The kept build after `what` equals a build from scratch, and (with `visible`) differs from the
/// page before it: the change reached the rows the same build.
fn check(page: &mut Page, world: &hecs::World, data: &DataStore, before: &str, what: &str, visible: bool) -> String {
    let kept = page.kept(world, data);
    let fresh = scratch(world, data);
    assert!(kept == fresh, "{what}: the kept rows differ from a build from scratch{}", first_difference(&kept, &fresh));
    if visible {
        assert!(kept != before, "{what}: the page did not change at all, so this case shows nothing");
    }
    kept
}

/// A crop of `area` (the first in the world's order).
fn crop_in(world: &hecs::World, area: &str) -> hecs::Entity {
    world.query::<&CropInstance>().iter().find(|(_, c)| c.tower_id.as_deref() == Some(area)).map(|(e, _)| e).expect("a crop there")
}

/// THE RULE OF A WALL SCREEN'S BUDGET. The main screen's page builds every area that needs it; a
/// wall screen's build builds every area that must be built (no rows to keep) and the others
/// oldest-built first while the budget lasts, and always at least one.
#[test]
fn a_wall_screen_builds_the_oldest_areas_within_its_budget() {
    let dirty = [(0, 30, 5, false), (1, 30, 2, false), (2, 30, 9, false), (3, 10, 1, true)];
    assert_eq!(pick(&dirty, None), vec![0, 1, 2, 3], "the main screen's page: every one");
    assert_eq!(pick(&dirty, Some(40)), vec![1, 3], "the must, then the oldest while the budget lasts");
    assert_eq!(pick(&dirty, Some(5)), vec![3], "a must spends the budget");
    assert_eq!(pick(&dirty[..3], Some(1)), vec![1], "at least one, the oldest");
    assert!(pick(&[], Some(10)).is_empty());
}

/// NOTHING IS BUILT AGAIN WHEN NOTHING CHANGED, AND A TURN BUILDS ITS OWN AREAS. With the farm at
/// an hour's interval and the clock held, a second build builds no area; a frame that brings up one
/// turn builds that turn's areas and no other (the air's words may move; no area is built for it).
#[test]
fn a_build_with_nothing_changed_builds_nothing() {
    let (data, mut world, mut sys) = garden(3600.0);
    frame(&data, &mut world, &mut sys, 1.0);
    let mut page = Page::default();
    page.kept(&world, &data);
    let all = page.rows.areas.len();
    assert!(page.built() == all && all >= 4, "the first build builds every area: {} of {all}", page.built());
    page.kept(&world, &data);
    let elapsed = clock_now(&data);
    let which: Vec<String> = page
        .rows
        .areas
        .iter()
        .filter(|a| a.built_at == page.rows.builds)
        .map(|a| format!("{} (valid {:?} at {elapsed}, {} crops)", a.name, a.valid, a.crops.len()))
        .collect();
    assert!(which.is_empty(), "nothing changed: nothing built, but {which:?}");
    // Frames to just short of the first area's turn of the hour (slot 1 starts at 225 s of the
    // farm's clock, which started at the first frame), the page built there; then 0.4 s that
    // crosses it. (Over minutes every area holds some crop whose count of minutes
    // moved, so every area is built again then, rightly.)
    for _ in 0..22 {
        frame(&data, &mut world, &mut sys, 10.0);
    }
    frame(&data, &mut world, &mut sys, 3.8);
    page.kept(&world, &data);
    frame(&data, &mut world, &mut sys, 0.4);
    let before = page.kept(&world, &data);
    let built = page.built();
    assert!(built >= 1 && built < all, "one turn built its own areas, not every area: {built} of {all}");
    let fresh = scratch(&world, &data);
    assert!(before == fresh, "and the page is the page from scratch{}", first_difference(&before, &fresh));
}

/// EVERY KIND OF CHANGE REACHES THE ROWS THE SAME BUILD (2026-10-31; the operator distrusts caches
/// because a version that misses one write keeps a stale row). Each case changes one input, then a
/// build that keeps rows is compared with a build from scratch, and the page must have changed:
/// every case changes something the rows show (the critic's F5, 2026-10-31: seven cases changed
/// nothing a row showed, where the comparison proves nothing). The one that cannot: the air's own
/// turns (a second of air moves a room's humidity by far less than the whole percent a row shows;
/// a grow room's air turns over in minutes to hours), so they follow a room made far damper,
/// whose words the rows do show. Creative mode, so a mulch and a colony need no items.
#[test]
fn every_kind_of_change_reaches_the_rows_the_same_build() {
    let (mut data, mut world, mut sys) = garden(3600.0);
    data.insert("creative_mode", Mutex::new(true));
    frame(&data, &mut world, &mut sys, 1.0);
    let mut page = Page::default();
    let mut before = page.kept(&world, &data);

    // (a) An area's own turn of the hour: the clock run on to just short of it and the page built
    // there, then 0.4 s that crosses it, so no crop's words move by the clock and only the turn's
    // own version can tell the page its areas changed.
    for _ in 0..22 {
        frame(&data, &mut world, &mut sys, 10.0);
    }
    frame(&data, &mut world, &mut sys, 3.8);
    before = check(&mut page, &world, &data, &before, "the clock run on", true);
    frame(&data, &mut world, &mut sys, 0.4);
    before = check(&mut page, &world, &data, &before, "an area's own turn", true);

    // (b) A command: a hand watering of a crop in a bed between its turns.
    let crop = crop_in(&world, "bed_b");
    *data.get::<Mutex<Option<u64>>>("water_request").unwrap().lock().unwrap() = Some(crop.to_bits().get());
    frame(&data, &mut world, &mut sys, 1.0);
    before = check(&mut page, &world, &data, &before, "a command (a hand watering)", true);

    // (b) A command that names an area: a mulch laid (the weeds' line names it), then a pest
    // control (every area and the air).
    let mulch = farming::weeds::WeedData::load().controls[1].id.clone();
    *data.get::<Mutex<Option<(String, String)>>>(farming::weeds::REQUEST_KEY).unwrap().lock().unwrap() = Some(("bed_a".into(), mulch));
    frame(&data, &mut world, &mut sys, 1.0);
    before = check(&mut page, &world, &data, &before, "a command (a mulch laid)", true);
    let cover = farming::pests::PestData::load().controls.iter().find(|c| c.is_cover()).map(|c| c.id.clone()).expect("a row cover in pests.ron");
    *data.get::<Mutex<Option<(String, String)>>>("pest_control_request").unwrap().lock().unwrap() = Some(("veg_field_1".into(), cover));
    frame(&data, &mut world, &mut sys, 1.0);
    before = check(&mut page, &world, &data, &before, "a command (a row cover laid)", true);

    // (b) A colony introduced in the hive: its line under the areas it reaches, and a working hive
    // for the crop cards of the greenhouse bed.
    *data.get::<Mutex<Option<farming::pollination::Request>>>(farming::pollination::REQUEST_KEY).unwrap().lock().unwrap() =
        Some(farming::pollination::Request::Colony("bumblebee_hive_1".into()));
    frame(&data, &mut world, &mut sys, 1.0);
    before = check(&mut page, &world, &data, &before, "a colony introduced", true);

    // (c) A reader's settle: ten minutes on with no turn of the hour's slots for most areas, then
    // the save's kind of ask, every area to now.
    for _ in 0..3 {
        frame(&data, &mut world, &mut sys, 200.0);
    }
    before = check(&mut page, &world, &data, &before, "the clock run on", true);
    sys.settle(&mut world, &data, 0.0);
    before = check(&mut page, &world, &data, &before, "a reader's settle", true);

    // (d) A load: the garden saved and applied again (new crops, a new soil memory), handed over.
    let save = crate::save_load::extract_world_save(&world);
    crate::save_load::apply_save_to_world(&mut world, &save);
    farming::clock::hand_over_resume(&data, farming::clock::Owed::default());
    before = check(&mut page, &world, &data, &before, "a save applied (before the farm's next tick)", true);
    // The farm's next tick, 200 s on (a second moves nothing a row shows for a watered crop at full
    // health, nor a minute's count).
    frame(&data, &mut world, &mut sys, 200.0);
    before = check(&mut page, &world, &data, &before, "the load handed over", true);

    // (e) The data files: plants.csv reloaded with a plant renamed.
    {
        let mut reg = data.get::<farming::PlantRegistry>("plant_registry").unwrap().clone();
        let id = world.query::<&CropInstance>().iter().next().map(|(_, c)| c.crop_def_id.clone()).unwrap();
        reg.plants.get_mut(&id).unwrap().name = "Renamed plant".into();
        data.insert("plant_registry", reg);
    }
    before = check(&mut page, &world, &data, &before, "plants.csv reloaded", true);

    // (e) A machine edit: the grow plots published again with the greenhouse bed moved into the
    // other grow room, out of the hive's reach and into another room's air.
    {
        let mut plots = data.get::<Vec<farming::lighting::GrowPlot>>("grow_plots").unwrap().clone();
        plots.iter_mut().find(|p| p.id == "bed_a").unwrap().pos = [22.0, 0.0, 3.0];
        data.insert("grow_plots", plots);
    }
    before = check(&mut page, &world, &data, &before, "a machine edit (the grow plots)", true);

    // (e) Each setting the views read.
    // The levels since 2026-11-12 (Off, Gentle, Realistic: the share of the loss kept), each a
    // change of the page and of the key.
    *data.get::<Mutex<f32>>(farming::pollination::MODE_KEY).unwrap().lock().unwrap() = 0.0;
    before = check(&mut page, &world, &data, &before, "pollination off", true);
    *data.get::<Mutex<f32>>(farming::pollination::MODE_KEY).unwrap().lock().unwrap() = 1.0;
    before = check(&mut page, &world, &data, &before, "pollination realistic", true);
    *data.get::<Mutex<f32>>(farming::pollination::MODE_KEY).unwrap().lock().unwrap() = 0.5;
    before = check(&mut page, &world, &data, &before, "pollination gentle", true);
    *data.get::<Mutex<f32>>(farming::soil_ph::MODE_KEY).unwrap().lock().unwrap() = 0.0;
    before = check(&mut page, &world, &data, &before, "soil pH off", true);
    *data.get::<Mutex<f32>>(farming::soil_ph::MODE_KEY).unwrap().lock().unwrap() = 1.0;
    before = check(&mut page, &world, &data, &before, "soil pH realistic", true);
    *data.get::<Mutex<f32>>("garden_pest_severity").unwrap().lock().unwrap() = 0.0;
    before = check(&mut page, &world, &data, &before, "pests off", true);
    *data.get::<Mutex<f32>>("garden_pest_severity").unwrap().lock().unwrap() = 1.0;
    before = check(&mut page, &world, &data, &before, "pests realistic", true);
    // Picking reads the ripe crops picked over a season: the dev grow ripens every crop first (a
    // command), then the mode.
    *data.get::<Mutex<bool>>("dev_grow_crops").unwrap().lock().unwrap() = true;
    frame(&data, &mut world, &mut sys, 1.0);
    before = check(&mut page, &world, &data, &before, "a command (the dev grow)", true);
    // The modes differ once a picked crop has been ripe past its first share (Forgiving keeps the
    // shares not taken, Realistic only the one ripe now): two game days on, an hour a frame.
    for _ in 0..48 {
        frame(&data, &mut world, &mut sys, 3600.0);
    }
    before = check(&mut page, &world, &data, &before, "two game days on", true);
    *data.get::<Mutex<farming::picking::Mode>>(farming::picking::MODE_KEY).unwrap().lock().unwrap() = farming::picking::Mode::Realistic;
    before = check(&mut page, &world, &data, &before, "picking realistic", true);
    *data.get::<Mutex<farming::picking::Mode>>(farming::picking::MODE_KEY).unwrap().lock().unwrap() = farming::picking::Mode::Off;
    before = check(&mut page, &world, &data, &before, "picking off", true);
    *data.get::<Mutex<farming::picking::Mode>>(farming::picking::MODE_KEY).unwrap().lock().unwrap() = farming::picking::Mode::Realistic;
    before = check(&mut page, &world, &data, &before, "picking realistic again", true);
    data.get::<Mutex<HashMap<String, f32>>>("garden_irrigation").unwrap().lock().unwrap().insert("bed_a".into(), 0.1);
    before = check(&mut page, &world, &data, &before, "an area's irrigation target", true);

    // (e) The tanks run dry: nothing waters automatically any more.
    {
        let ws = data.get::<Mutex<humanity_systems::plumbing::WaterStatus>>("water_status").unwrap();
        let mut w = ws.lock().unwrap();
        w.capacity_l = 1000.0;
        w.stored_l = 0.0;
    }
    before = check(&mut page, &world, &data, &before, "the tanks empty", true);

    // The clock alone, no farm tick: a day on, every "ripens in" a day less.
    set_clock(&data, clock_now(&data) + 86_400.0);
    before = check(&mut page, &world, &data, &before, "the clock a day on (no farm tick)", true);
    // And back (a clock set back).
    set_clock(&data, clock_now(&data) - 86_400.0);
    before = check(&mut page, &world, &data, &before, "the clock set back", true);

    // The air: a grow room's air made far damper (as a humidifier stuck on would leave it), written
    // into the soil memory with no farm run, so the rows' air words can only come from the room's
    // own reading; then frames through the evening, the air on its own turn every second (moving
    // by less than a shown percent: see above), and the sun going down.
    {
        let mut q = world.query::<&mut humanity_core::ecs::components::SoilMemory>();
        let (_, m) = q.iter().next().expect("the soil memory");
        let room = m.rooms.get_mut("room-a").expect("the greenhouse's air is tracked");
        room.vapour_g_m3 *= 1.6;
    }
    before = check(&mut page, &world, &data, &before, "a room's air far damper", true);
    for _ in 0..20 {
        frame(&data, &mut world, &mut sys, 0.5);
        before = check(&mut page, &world, &data, &before, "the air's own turn", false);
    }
    set_clock(&data, 19.0 * 3600.0);
    frame(&data, &mut world, &mut sys, 1.0);
    check(&mut page, &world, &data, &before, "sunset", true);
}

/// EVERY STAMPED ENTRY REACHES THE ROWS WHEN IT IS PUBLISHED AGAIN (2026-10-31, the critic's F5:
/// of the entries the rows key on by their DataStore stamp, only plants.csv was proven). Each is
/// inserted again with a value the kept rows show differently, the way a hot reload or a machine
/// edit inserts it; each time the page must change and equal a build from scratch, and every
/// entry of `STAMPED` must have its case. Seen red 2026-10-31 with "garden_lighting" left out of
/// `STAMPED`: "the lighting data published again: the kept rows differ from a build from scratch".
#[test]
fn every_stamped_entry_reaches_the_rows_when_published_again() {
    use farming::{humidity, lighting, picking, pollination, soil, soil_ph, weeds};
    let (mut data, mut world, mut sys) = garden(3600.0);
    data.insert("creative_mode", Mutex::new(true));
    // Ripe crops, so the picking data shows: the dev grow (a command).
    frame(&data, &mut world, &mut sys, 1.0);
    *data.get::<Mutex<bool>>("dev_grow_crops").unwrap().lock().unwrap() = true;
    frame(&data, &mut world, &mut sys, 1.0);
    let mut page = Page::default();
    let mut before = page.kept(&world, &data);
    let mut proven: Vec<&str> = Vec::new();
    let mut case = |data: &mut DataStore, key: &'static str, what: &str, change: &dyn Fn(&mut DataStore)| {
        change(data);
        before = check(&mut page, &world, data, &before, what, true);
        proven.push(key);
    };
    case(&mut data, "plant_registry", "plants.csv published again", &|d| {
        let mut reg = d.get::<farming::PlantRegistry>("plant_registry").unwrap().clone();
        for p in reg.plants.values_mut() {
            p.name = format!("{} (renamed)", p.name);
        }
        d.insert("plant_registry", reg);
    });
    case(&mut data, "garden_nutrients", "the nutrient data published again (the anchor's removal doubled; before it, no crop has a season need)", &|d| {
        let mut nd = soil::NutrientData::parse(soil::NUTRIENTS_RON).unwrap();
        nd.demand_anchor.n_g_per_kg *= 2.0;
        nd.demand_anchor.p2o5_g_per_kg *= 2.0;
        nd.demand_anchor.k2o_g_per_kg *= 2.0;
        d.insert("garden_nutrients", nd);
    });
    case(&mut data, "item_registry", "items.csv published again (every mass doubled: a harvest twice the weight takes twice the nutrients)", &|d| {
        let mut items = d.get::<humanity_systems::inventory::ItemRegistry>("item_registry").unwrap().clone();
        for item in items.items.values_mut() {
            item.mass_kg *= 2.0;
        }
        d.insert("item_registry", items);
    });
    case(&mut data, farming::units::PLOT_AREA_KEY, "the plots' floor areas published again (a bed a quarter the size)", &|d| {
        let mut areas = d.get::<HashMap<String, f32>>(farming::units::PLOT_AREA_KEY).unwrap().clone();
        *areas.get_mut("bed_a").unwrap() *= 0.25;
        d.insert(farming::units::PLOT_AREA_KEY, areas);
    });
    case(&mut data, "garden_lighting", "the lighting data published again", &|d| {
        let mut ld = lighting::LightingData::parse(lighting::LIGHTING_RON).unwrap();
        ld.target_dli_mol_m2_d *= 2.0;
        d.insert("garden_lighting", ld);
    });
    case(&mut data, "garden_soil_ph", "the soil pH data published again (a harder cap outside a window)", &|d| {
        let mut ph = soil_ph::SoilPhData::parse(soil_ph::SOIL_PH_RON).unwrap();
        ph.loss_per_ph_unit = (ph.loss_per_ph_unit * 3.0).min(0.9);
        d.insert("garden_soil_ph", ph);
    });
    case(&mut data, pollination::DATA_KEY, "the pollination data published again (tomatoes left out)", &|d| {
        let text = pollination::POLLINATION_RON.replacen("id: \"tomato\"", "id: \"tomato_not_listed\"", 1);
        d.insert(pollination::DATA_KEY, pollination::PollinationData::parse(&text).unwrap());
    });
    case(&mut data, picking::DATA_KEY, "the harvest windows published again (tomatoes left out)", &|d| {
        let text = picking::HARVEST_WINDOWS_RON.replacen("plant: \"tomato\"", "plant: \"tomato_not_listed\"", 1);
        d.insert(picking::DATA_KEY, picking::HarvestWindows::parse(&text).unwrap());
    });
    case(&mut data, humidity::DATA_KEY, "the humidity data published again (the rooms warmer)", &|d| {
        let mut hd = humidity::HumidityData::parse(humidity::HUMIDITY_RON).unwrap();
        hd.room_temp_c += 8.0;
        d.insert(humidity::DATA_KEY, hd);
    });
    case(&mut data, weeds::DATA_KEY, "the weeds data published again (weeds noticed at once)", &|d| {
        let mut wd = weeds::WeedData::parse(weeds::WEEDS_RON).unwrap();
        wd.detect_at = 0.0;
        d.insert(weeds::DATA_KEY, wd);
    });
    case(&mut data, "garden_pests", "the pests data published again (the row cover renamed)", &|d| {
        let mut pd = farming::pests::PestData::parse(farming::pests::PESTS_RON).unwrap();
        let cover = pd.controls.iter_mut().find(|c| c.is_cover()).unwrap();
        cover.name = format!("{} (renamed)", cover.name);
        d.insert("garden_pests", pd);
    });
    case(&mut data, "grow_plots", "the grow plots published again (a bed moved to the other room)", &|d| {
        let mut plots = d.get::<Vec<lighting::GrowPlot>>("grow_plots").unwrap().clone();
        plots.iter_mut().find(|p| p.id == "bed_b").unwrap().pos = [5.0, 0.0, 6.0];
        d.insert("grow_plots", plots);
    });
    let mut stamped: Vec<&str> = super::STAMPED.to_vec();
    stamped.sort_unstable();
    proven.sort_unstable();
    assert_eq!(proven, stamped, "every stamped entry has its case here");
}

/// EVERY AREA EQUALS A BUILD FROM SCRATCH AFTER A LONG RUN of turns and commands, at a second's
/// interval and an hour's: frames of half a second for five game minutes, a command every few
/// frames (waterings, harvests, hoeing, a pH amendment, a colony), the page built after every
/// frame and compared with a build from scratch every fifth frame and each of the last twenty (a
/// test build is unoptimised, and a build from scratch is the slow one).
#[test]
fn every_area_equals_a_build_from_scratch_after_a_long_run() {
    for interval in [1.0, 3600.0] {
        let (data, mut world, mut sys) = garden(interval);
        frame(&data, &mut world, &mut sys, 1.0);
        let mut page = Page::default();
        page.kept(&world, &data);
        let amendment = farming::soil_ph::SoilPhData::load().amendments[0].id.clone();
        let hoe = farming::weeds::WeedData::load().controls[0].id.clone();
        let areas = ["bed_a", "bed_b", "aeroponic_tower_1", "veg_field_1"];
        for k in 0..600usize {
            match k % 37 {
                3 => {
                    let crop = crop_in(&world, areas[k % 4]);
                    *data.get::<Mutex<Option<u64>>>("water_request").unwrap().lock().unwrap() = Some(crop.to_bits().get());
                }
                11 => {
                    let ripe: Option<hecs::Entity> = {
                        let reg = data.get::<farming::PlantRegistry>("plant_registry").unwrap();
                        world.query::<&CropInstance>().iter().find(|(_, c)| reg.get(&c.crop_def_id).is_some_and(|d| c.growth_stage == d.last_stage())).map(|(e, _)| e)
                    };
                    if let Some(e) = ripe {
                        *data.get::<Mutex<Option<u64>>>("harvest_request").unwrap().lock().unwrap() = Some(e.to_bits().get());
                    }
                }
                19 => {
                    *data.get::<Mutex<Option<(String, String)>>>(farming::weeds::REQUEST_KEY).unwrap().lock().unwrap() = Some((areas[k % 2].into(), hoe.clone()));
                }
                27 => {
                    *data.get::<Mutex<Option<(String, String)>>>("soil_ph_request").unwrap().lock().unwrap() = Some(("bed_b".into(), amendment.clone()));
                }
                31 if k < 40 => {
                    *data.get::<Mutex<Option<farming::pollination::Request>>>(farming::pollination::REQUEST_KEY).unwrap().lock().unwrap() =
                        Some(farming::pollination::Request::Colony("bumblebee_hive_1".into()));
                }
                _ => {}
            }
            frame(&data, &mut world, &mut sys, 0.5);
            let kept = page.kept(&world, &data);
            if k % 5 == 0 || k >= 580 {
                let fresh = scratch(&world, &data);
                assert!(kept == fresh, "interval {interval}, frame {k}: the kept rows differ from a build from scratch{}", first_difference(&kept, &fresh));
            }
        }
    }
}

/// A CROP THAT ONLY MOVES IN THE WORLD'S ORDER BUILDS NOTHING (2026-10-31, the critic's F1). hecs
/// keeps an archetype's entities in a dense array and takes one out by swap-remove, so adding or
/// removing any component on a crop (a ripe crop's picking record, a pollination record, a first
/// soil record, the sowing marker coming off, a harvest) moves it to the end of another archetype
/// and moves the last crop of its old one into its place: the world's order of crops changes,
/// across areas, while no crop and no row did. The page kept an area only while its crops stood in
/// exactly the world's order, so each such change rebuilt two areas as never built, past a wall
/// screen's budget: with the showcase's field of about 2,000 crops, one ripening rebuilt it whole.
/// Here a component no row reads is added to one crop: no area is built, and the page is still the
/// page from scratch. Seen red 2026-10-31 on the code before the fix (areas kept in the world's
/// order).
#[test]
fn a_reorder_of_the_crops_alone_builds_nothing() {
    struct Moved;
    let (data, mut world, mut sys) = garden(3600.0);
    frame(&data, &mut world, &mut sys, 1.0);
    let mut page = Page::default();
    page.kept(&world, &data);
    page.kept(&world, &data);
    assert_eq!(page.built(), 0, "nothing changed: nothing built");
    let e = crop_in(&world, "bed_a");
    world.insert_one(e, Moved).expect("the crop is there");
    let kept = page.kept(&world, &data);
    let which: Vec<String> = page.rows.areas.iter().filter(|a| a.built_at == page.rows.builds).map(|a| a.name.clone()).collect();
    assert!(which.is_empty(), "a crop moved in the world's order, nothing else changed, but these areas were built again: {which:?}");
    let fresh = scratch(&world, &data);
    assert!(kept == fresh, "and the page is the page from scratch{}", first_difference(&kept, &fresh));
}
