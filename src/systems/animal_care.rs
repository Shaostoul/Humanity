//! Farm animals need care (2026-10-12, first-hour audit S6).
//!
//! Until this the homestead's chickens, goats and sheep needed nothing and a hen laid an egg
//! every five minutes. Now every kept animal of a species listed in `data/livestock/care.ron`
//! eats and drinks real amounts a day, on the game clock (1x is real time):
//!
//! - FEED comes out of home storage (the "home_stock" map the automated machines draw on,
//!   `engine::stock_piles`), one whole item at a time into the animal's own trough, in the order
//!   of its `eats` list. Which items are feed, and how much dry matter each holds, is data.
//! - WATER comes out of the home's water tanks (`fluids::draw_from_tanks`), a day's worth into
//!   its drinker at a time, and when the tanks are dry, out of stored tap water (the fluid
//!   table's `tap` list), the way a machine draws tap water.
//! - Each animal counts the hours it has been short of feed and of water. Short, it makes less
//!   (egg, milk, wool), then nothing; in Realistic mode it then falls sick and dies. In
//!   Forgiving mode the count stops where production stops, so neglect only stops what it makes.
//! - Its yield timer runs here, in f64, and is written to the `Harvestable` the E-press collect
//!   reads. The timer used to run in the component's f32, which stops counting a 60 fps frame
//!   once it passes about three days (a sheep's year of wool would never grow).
//!
//! TWO MODES (CLAUDE.md "Dual modes"): Settings > Gameplay > Farm animals, published each frame
//! by `engine::survival_env` under [`MODE_KEY`], read the way `systems::illness` reads its own.
//!
//! WHILE THE PLAYER IS AWAY (docs/design/offline-progression.md): the character feeds the
//! animals. The first tick after a save comes back draws the time away's feed from home
//! storage; water is taken as given (the tanks do not refill offline, so drawing from them
//! would empty the home's water for want of a simulation that was not running). If storage ran
//! short, the animals come back hungry, never past the point where they stop producing:
//! nothing sickens or dies because the player was away.
//!
//! A DEAD HERD ANIMAL STAYS DEAD: its herd slot is kept in the save's lost list, and world entry
//! does not spawn it again (before this, every restart brought every dead animal back).
//!
//! Pure data, math and ECS: compiles in every feature set (the relay build includes `systems/`).

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::ecs::components::{AIBehavior, Creature, Dead, Harvestable, Name};
use crate::hot_reload::data_store::DataStore;
use crate::systems::inventory::ItemRegistry;
use crate::systems::livestock::HerdSlot;

/// The data file, under data/.
pub const FILE: &str = "livestock/care.ron";
/// DataStore key of the loaded [`CareData`] (engine::registries).
pub const DATA_KEY: &str = "animal_care_data";
/// DataStore key of the Settings mode (a plain [`Mode`]), published each frame by
/// `engine::survival_env` from Settings > Gameplay > Farm animals.
pub const MODE_KEY: &str = "animal_care_mode";
/// DataStore key of the saved care waiting for the herd (`Mutex<Option<Restore>>`).
pub const RESTORE_KEY: &str = "animal_care_restore";
/// DataStore key of the herd animals that died (`Mutex<Vec<LostAnimal>>`), kept in the save.
pub const LOST_KEY: &str = "animal_care_lost";

/// A day, for the data's amounts a day: the body's day, whatever the calendar's hours.
const DAY_S: f64 = crate::systems::time::EARTH_DAY_S;
const HOUR_S: f64 = crate::systems::time::SECONDS_PER_HOUR;

// ── The two modes ────────────────────────────────────────────────────────────────────────────

/// The two modes of the house rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// The default: neglect stops what an animal makes, and nothing worse.
    #[default]
    Forgiving,
    /// Neglect makes an animal sick, and then it dies.
    Realistic,
}

impl Mode {
    /// The Settings mode. Absent (headless tests, the relay) is Realistic: what the data
    /// gives, the convention illness and body heat use.
    pub fn from_store(data: &DataStore) -> Self {
        data.get::<Mode>(MODE_KEY).copied().unwrap_or(Mode::Realistic)
    }

    /// The mode the saved setting names (`AppConfig::animal_care_realistic`).
    pub fn from_realistic(realistic: bool) -> Self {
        if realistic {
            Mode::Realistic
        } else {
            Mode::Forgiving
        }
    }
}

// ── The data (data/livestock/care.ron) ───────────────────────────────────────────────────────

/// Hours short of feed (or of water) at which each thing happens.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Limits {
    /// It makes less from here on.
    pub slows_h: f64,
    /// It makes nothing from here on. Forgiving mode's count stops here.
    pub stops_h: f64,
    /// Realistic: it falls sick.
    pub sick_h: f64,
    /// Realistic: it dies.
    pub dies_h: f64,
}

impl Limits {
    /// The share of a full yield it makes at `short_h` hours short: all of it up to
    /// `slows_h`, falling in a straight line to none at `stops_h`.
    pub fn production(&self, short_h: f64) -> f64 {
        if short_h <= self.slows_h {
            1.0
        } else if short_h >= self.stops_h || self.stops_h <= self.slows_h {
            0.0
        } else {
            1.0 - (short_h - self.slows_h) / (self.stops_h - self.slows_h)
        }
    }
}

/// One species' needs: a row of the file's `species`.
#[derive(Debug, Clone, Deserialize)]
pub struct SpeciesCare {
    /// The species in the plural, for what the player is told ("chickens", "sheep").
    pub plural: String,
    /// Dry matter it eats a day, kg.
    pub dry_matter_kg_day: f64,
    /// Water it drinks a day, litres.
    pub water_l_day: f64,
    /// The feed items it eats, in the order it is fed them.
    pub eats: Vec<String>,
    pub feed: Limits,
    pub water: Limits,
    /// Hours short made up for every hour it is cared for (Realistic).
    pub recovery: f64,
}

/// data/livestock/care.ron.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CareData {
    /// Feed item id to the share of its mass that is dry matter.
    pub feeds: HashMap<String, f64>,
    /// Forgiving mode's recovery: hours short made up for every hour cared for.
    pub forgiving_recovery: f64,
    /// Species id (creatures.csv) to its needs.
    pub species: HashMap<String, SpeciesCare>,
}

impl CareData {
    pub fn from_ron(bytes: &[u8]) -> Result<Self, String> {
        let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
        ron::from_str(text).map_err(|e| format!("{FILE}: {e}"))
    }

    /// The file as the game loads it: the data folder's copy, else the built-in one.
    pub fn load(data_dir: &std::path::Path) -> Self {
        crate::embedded_data::load_data_or_embedded(data_dir, FILE, Self::from_ron).unwrap_or_else(|e| {
            log::warn!("{e}; farm animals need nothing");
            Self::default()
        })
    }

    /// Dry matter in one unit of `item`, kg: its mass times its dry-matter share. None when
    /// it is not feed or its mass is unknown.
    pub fn unit_dry_matter_kg(&self, item: &str, items: Option<&ItemRegistry>) -> Option<f64> {
        let share = self.feeds.get(item).copied().filter(|s| *s > 0.0)?;
        let mass = items?.items.get(item).map(|d| f64::from(d.mass_kg)).filter(|m| *m > 0.0)?;
        Some(mass * share)
    }
}

// ── The animal's state ──────────────────────────────────────────────────────────────────────

/// What a kept animal has been given and how long it has gone short. A component on each
/// animal of a species with a care row; the first livestock tick puts it on.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AnimalCare {
    /// Dry matter in its trough, kg: taken from storage, not eaten yet.
    pub trough_kg: f64,
    /// Water in its drinker, litres.
    pub drinker_l: f64,
    /// Hours short of feed.
    pub hungry_h: f64,
    /// Hours short of water.
    pub thirsty_h: f64,
    /// It wanted to eat and nothing was there (this tick).
    pub no_feed: bool,
    /// It wanted to drink and nothing was there (this tick).
    pub no_water: bool,
    /// Time away still to feed it through, game seconds.
    pub away_s: f64,
    /// Its yield timer in f64 (game seconds of production), mirrored to its `Harvestable`.
    pub product_s: f64,
    /// What this module last wrote to the `Harvestable`; anything else there means a collect
    /// (or a restore) changed it, and the f64 timer takes the new value.
    pub written: Option<f32>,
    /// What the player has been told, so a state is told once (the `TOLD_*` bits).
    pub told: u8,
}

const TOLD_NO_FEED: u8 = 1;
const TOLD_NO_WATER: u8 = 2;
const TOLD_SICK: u8 = 4;

impl AnimalCare {
    /// The share of a full yield it makes now.
    pub fn production(&self, sp: &SpeciesCare, mode: Mode) -> f64 {
        if self.sick(sp, mode) {
            return 0.0;
        }
        sp.feed.production(self.hungry_h).min(sp.water.production(self.thirsty_h))
    }

    /// Sick from going without (Realistic only).
    pub fn sick(&self, sp: &SpeciesCare, mode: Mode) -> bool {
        mode == Mode::Realistic && (self.hungry_h >= sp.feed.sick_h || self.thirsty_h >= sp.water.sick_h)
    }

    fn record(&self, slot: &str) -> CareSave {
        CareSave {
            slot: slot.to_string(),
            trough_kg: self.trough_kg,
            drinker_l: self.drinker_l,
            hungry_h: self.hungry_h,
            thirsty_h: self.thirsty_h,
        }
    }

    fn from_record(r: &CareSave) -> Self {
        Self { trough_kg: r.trough_kg, drinker_l: r.drinker_l, hungry_h: r.hungry_h, thirsty_h: r.thirsty_h, ..Default::default() }
    }
}

/// One herd animal's care in the save (`WorldSave::herd_care`), by its herd slot ("goat#1").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CareSave {
    pub slot: String,
    pub trough_kg: f64,
    pub drinker_l: f64,
    pub hungry_h: f64,
    pub thirsty_h: f64,
}

/// A herd animal that died (`WorldSave::herd_lost`): its slot is not spawned again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LostAnimal {
    pub slot: String,
    /// What it died of, in plain words ("starved"); "killed" for anything else.
    pub cause: String,
}

/// Saved care waiting for the herd to be spawned, with the time away to feed it through.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Restore {
    pub care: Vec<CareSave>,
    pub away_s: f64,
}

// ── Channels ─────────────────────────────────────────────────────────────────────────────────

/// Put this module's channels in the DataStore (`livestock::register`, at boot).
pub fn register(store: &mut DataStore) {
    store.insert(RESTORE_KEY, Mutex::new(Option::<Restore>::None));
    store.insert(LOST_KEY, Mutex::new(Vec::<LostAnimal>::new()));
}

/// The herd animals that have died, for the save.
pub fn lost(data: &DataStore) -> Vec<LostAnimal> {
    data.get::<Mutex<Vec<LostAnimal>>>(LOST_KEY).and_then(|m| m.lock().ok().map(|v| v.clone())).unwrap_or_default()
}

/// Whether the herd slot `slot` died (world entry does not spawn it).
pub fn is_lost(data: &DataStore, slot: &str) -> bool {
    data.get::<Mutex<Vec<LostAnimal>>>(LOST_KEY)
        .and_then(|m| m.lock().ok().map(|v| v.iter().any(|l| l.slot == slot)))
        .unwrap_or(false)
}

/// The saved care still waiting for the herd, if any (a save written before world entry
/// keeps it rather than forgetting).
pub fn pending_care(data: &DataStore) -> Option<Vec<CareSave>> {
    data.get::<Mutex<Option<Restore>>>(RESTORE_KEY).and_then(|m| m.lock().ok().and_then(|r| r.as_ref().map(|r| r.care.clone())))
}

fn set_restore(data: &DataStore, value: Option<Restore>) {
    if let Some(Ok(mut r)) = data.get::<Mutex<Option<Restore>>>(RESTORE_KEY).map(|m| m.lock()) {
        *r = value;
    }
}

fn take_restore(data: &DataStore) -> Option<Restore> {
    data.get::<Mutex<Option<Restore>>>(RESTORE_KEY).and_then(|m| m.lock().ok().and_then(|mut r| r.take()))
}

/// Each living herd animal's care, for the save, by slot.
pub fn herd_care(world: &hecs::World) -> Vec<CareSave> {
    let mut out: Vec<CareSave> = world
        .query::<(&HerdSlot, &AnimalCare, Option<&Dead>)>()
        .iter()
        .filter(|(_, (_, _, dead))| dead.is_none())
        .map(|(_, (slot, care, _))| care.record(&slot.0))
        .collect();
    out.sort_by(|a, b| a.slot.cmp(&b.slot));
    out
}

/// Put a save's care and dead on the herd (save_load's resume, at startup and on character
/// select): at once when the herd is in the world, else held for world entry. `away_s` is the
/// time away the character fed them through. Replaces whatever was waiting and the lost list.
pub fn restore(world: &mut hecs::World, data: &DataStore, care: Vec<CareSave>, dead: Vec<LostAnimal>, away_s: f64) {
    if let Some(Ok(mut l)) = data.get::<Mutex<Vec<LostAnimal>>>(LOST_KEY).map(|m| m.lock()) {
        *l = dead;
    }
    let herd: Vec<(hecs::Entity, String, String)> = world
        .query::<(&HerdSlot, &Creature)>()
        .iter()
        .map(|(e, (s, c))| (e, s.0.clone(), c.def_id.clone()))
        .collect();
    let cared = data.get::<CareData>(DATA_KEY);
    if herd.is_empty() || cared.is_none() {
        set_restore(data, Some(Restore { care, away_s: away_s.max(0.0) }));
        return;
    }
    let cared = cared.expect("checked above");
    for (e, slot, species) in herd {
        if is_lost(data, &slot) {
            let _ = world.despawn(e);
            continue;
        }
        if !cared.species.contains_key(&species) {
            continue;
        }
        let mut c = care.iter().find(|r| r.slot == slot).map(AnimalCare::from_record).unwrap_or_default();
        c.away_s = away_s.max(0.0);
        let _ = world.insert_one(e, c);
    }
    set_restore(data, None);
}

/// World entry is about to despawn the herd and spawn it anew: keep its care for the new one,
/// unless a save's is already waiting (world_load, before the despawn).
pub fn hold_current(world: &hecs::World, data: &DataStore) {
    if pending_care(data).is_some() {
        return;
    }
    let care = herd_care(world);
    if !care.is_empty() {
        set_restore(data, Some(Restore { care, away_s: 0.0 }));
    }
}

/// The share of a full yield each saved animal was making when the game was saved, by slot:
/// the time away moves its yield on by only that share of the time
/// (`livestock::timers_after_away`). 1 for a slot with no saved care or no care data.
pub fn away_shares(data: &DataStore, care: &[CareSave]) -> HashMap<String, f64> {
    let Some(cd) = data.get::<CareData>(DATA_KEY) else { return HashMap::new() };
    care.iter()
        .filter_map(|r| {
            let species = r.slot.split('#').next().unwrap_or_default();
            let sp = cd.species.get(species)?;
            Some((r.slot.clone(), AnimalCare::from_record(r).production(sp, Mode::Realistic)))
        })
        .collect()
}

// ── The tick ─────────────────────────────────────────────────────────────────────────────────

/// What one tick told the player, by line, so three hens make one line.
#[derive(Default)]
struct Said(Vec<String>);

impl Said {
    fn say(&mut self, line: String) {
        if !self.0.contains(&line) {
            self.0.push(line);
        }
    }
}

/// What the time away's feeding took, for the one notice.
#[derive(Default)]
struct AwayFed {
    /// Item id to units taken from storage.
    taken: Vec<(String, u32)>,
    /// Animals that went short because storage ran out.
    short: u32,
    fed: u32,
}

/// Feed, water and age every cared-for animal by `game_dt` game seconds (`LivestockSystem`,
/// before its own regrowth pass, which leaves these animals to this). Nothing happens without
/// the care data.
pub fn tick(world: &mut hecs::World, game_dt: f32, data: &DataStore) {
    let Some(cd) = data.get::<CareData>(DATA_KEY) else { return };
    let mode = Mode::from_store(data);
    let dt = f64::from(game_dt.max(0.0));
    let mut said = Said::default();

    keep_the_dead(world, data);
    attach(world, data, cd);

    let items = data.get::<ItemRegistry>("item_registry");
    let fluids = data.get::<crate::systems::fluids::FluidTable>("fluid_table");
    let stock_slot = data.get::<Mutex<HashMap<String, u32>>>("home_stock");
    let mut stock_guard = stock_slot.and_then(|m| m.lock().ok());
    let mut away = AwayFed::default();

    let animals: Vec<(hecs::Entity, String, String)> = world
        .query::<(&Creature, &AnimalCare, Option<&Name>, Option<&Dead>)>()
        .iter()
        .filter(|(_, (_, _, _, dead))| dead.is_none())
        .map(|(e, (c, _, n, _))| (e, c.def_id.clone(), n.map_or_else(|| c.def_id.clone(), |n| n.0.clone())))
        .collect();
    for (e, species, name) in animals {
        let Some(sp) = cd.species.get(&species) else { continue };
        let Some(mut care) = world.get::<&AnimalCare>(e).ok().map(|c| (*c).clone()) else { continue };

        // The time away: the character fed it from storage (water taken as given).
        if care.away_s > 0.0 {
            let need = sp.dry_matter_kg_day * care.away_s / DAY_S;
            let (ate, taken) = eat(&mut care, sp, cd, stock_guard.as_deref_mut(), items, need);
            for (id, n) in taken {
                match away.taken.iter_mut().find(|(i, _)| *i == id) {
                    Some(t) => t.1 += n,
                    None => away.taken.push((id, n)),
                }
            }
            let short_h = (need - ate).max(0.0) / sp.dry_matter_kg_day.max(1e-9) * 24.0;
            if short_h > 1e-6 {
                // Hungry, but never past where it stops producing: nothing sickens while away.
                care.hungry_h = (care.hungry_h + short_h).min(care.hungry_h.max(sp.feed.stops_h));
                away.short += 1;
            }
            away.fed += 1;
            care.away_s = 0.0;
        }

        // This tick's feed and water.
        let need_kg = sp.dry_matter_kg_day * dt / DAY_S;
        let (ate, _) = eat(&mut care, sp, cd, stock_guard.as_deref_mut(), items, need_kg);
        let need_l = sp.water_l_day * dt / DAY_S;
        if care.drinker_l < need_l {
            // A day's water at a time: a tick's sip is too small for the tank's f32 litres.
            let want = (sp.water_l_day - care.drinker_l).max(need_l - care.drinker_l);
            care.drinker_l += f64::from(crate::systems::fluids::draw_from_tanks(world, want as f32));
            while care.drinker_l < need_l {
                let Some((id, litres)) = stored_water(fluids, stock_guard.as_deref()) else { break };
                if let Some(n) = stock_guard.as_deref_mut().and_then(|s| s.get_mut(&id)) {
                    *n -= 1;
                }
                care.drinker_l += litres;
            }
        }
        let drank = care.drinker_l.min(need_l);
        care.drinker_l -= drank;

        if dt > 0.0 {
            let hours = dt / HOUR_S;
            let recovery = match mode {
                Mode::Realistic => sp.recovery,
                Mode::Forgiving => cd.forgiving_recovery,
            };
            care.hungry_h = shortfall(care.hungry_h, hours, ate / need_kg.max(1e-12), recovery);
            care.thirsty_h = shortfall(care.thirsty_h, hours, drank / need_l.max(1e-12), recovery);
            if mode == Mode::Forgiving {
                care.hungry_h = care.hungry_h.min(sp.feed.stops_h);
                care.thirsty_h = care.thirsty_h.min(sp.water.stops_h);
            }
            care.no_feed = ate + 1e-12 < need_kg;
            care.no_water = drank + 1e-12 < need_l;
        }

        // What it makes, on its own f64 clock.
        let share = care.production(sp, mode);
        if let Ok(mut h) = world.get::<&mut Harvestable>(e) {
            if care.written != Some(h.time_since_harvest) {
                care.product_s = f64::from(h.time_since_harvest);
            }
            let regrow = f64::from(h.regrow_time);
            if care.product_s < regrow {
                care.product_s = (care.product_s + dt * share).min(regrow);
            }
            h.time_since_harvest = if care.product_s >= regrow { h.regrow_time } else { care.product_s as f32 };
            care.written = Some(h.time_since_harvest);
        }

        // Tell the player, once per state.
        let lower = name.to_lowercase();
        tell(&mut care, &mut said, sp, cd, items, mode, &lower);

        // Realistic: going without long enough kills.
        let died = (mode == Mode::Realistic).then(|| {
            if care.hungry_h >= sp.feed.dies_h {
                Some("starved")
            } else if care.thirsty_h >= sp.water.dies_h {
                Some("died of thirst")
            } else {
                None
            }
        });
        if let Ok(mut c) = world.get::<&mut AnimalCare>(e) {
            *c = care;
        }
        if let Some(Some(cause)) = died {
            said.say(match cause {
                "starved" => format!("A {lower} has starved to death"),
                _ => format!("A {lower} has died of thirst"),
            });
            let _ = world.insert_one(e, Dead { cause: cause.to_string(), ..Default::default() });
        }
    }
    drop(stock_guard);

    if (away.fed > 0 && !away.taken.is_empty()) || away.short > 0 {
        said.say(away_line(&away, items));
    }
    keep_the_dead(world, data);
    if said.0.is_empty() {
        return;
    }
    if let Some(Ok(mut n)) = data.get::<Mutex<Vec<String>>>("player_notices").map(|m| m.lock()) {
        n.extend(said.0);
    }
}

/// Hours short after `hours` more, of which `fed_share` (0 to 1) it was cared for: it goes
/// short by the rest and makes up `recovery` hours for every hour cared for.
fn shortfall(short_h: f64, hours: f64, fed_share: f64, recovery: f64) -> f64 {
    let fed = fed_share.clamp(0.0, 1.0);
    (short_h + hours * (1.0 - fed) - hours * fed * recovery.max(0.0)).max(0.0)
}

/// Eat `need_kg` of dry matter from the trough, topping it up from storage one whole item at
/// a time, in the order of `eats`. Returns what it ate and what it took from storage.
fn eat(
    care: &mut AnimalCare,
    sp: &SpeciesCare,
    cd: &CareData,
    mut stock: Option<&mut HashMap<String, u32>>,
    items: Option<&ItemRegistry>,
    need_kg: f64,
) -> (f64, Vec<(String, u32)>) {
    let mut taken: Vec<(String, u32)> = Vec::new();
    while care.trough_kg < need_kg {
        let next = stock.as_deref().and_then(|s| {
            sp.eats.iter().find_map(|id| {
                let kg = cd.unit_dry_matter_kg(id, items)?;
                (s.get(id).copied().unwrap_or(0) > 0).then(|| (id.clone(), kg))
            })
        });
        let Some((id, kg)) = next else { break };
        if let Some(n) = stock.as_deref_mut().and_then(|s| s.get_mut(&id)) {
            *n -= 1;
        }
        care.trough_kg += kg;
        match taken.iter_mut().find(|(i, _)| *i == id) {
            Some(t) => t.1 += 1,
            None => taken.push((id, 1)),
        }
    }
    let ate = care.trough_kg.min(need_kg).max(0.0);
    care.trough_kg -= ate;
    (ate, taken)
}

/// The first stored tap water (the fluid table's `tap` list), with its litres a unit.
fn stored_water(fluids: Option<&crate::systems::fluids::FluidTable>, stock: Option<&HashMap<String, u32>>) -> Option<(String, f64)> {
    let (fluids, stock) = (fluids?, stock?);
    let mut taps: Vec<(&String, &f32)> = fluids.tap.iter().filter(|(_, l)| **l > 0.0).collect();
    taps.sort_by(|a, b| a.0.cmp(b.0));
    taps.into_iter().find(|(id, _)| stock.get(*id).copied().unwrap_or(0) > 0).map(|(id, l)| (id.clone(), f64::from(*l)))
}

/// Put `AnimalCare` on every living farm animal of a species with a care row that has none:
/// a herd animal takes its saved care (and the time away) when a save is waiting.
fn attach(world: &mut hecs::World, data: &DataStore, cd: &CareData) {
    let new: Vec<(hecs::Entity, Option<String>)> = world
        .query::<(&Creature, Option<&HerdSlot>, Option<&AIBehavior>, Option<&Dead>, Option<&AnimalCare>)>()
        .iter()
        .filter(|(_, (c, _, ai, dead, care))| {
            care.is_none() && dead.is_none() && ai.is_none() && cd.species.contains_key(&c.def_id)
        })
        .map(|(e, (_, slot, _, _, _))| (e, slot.map(|s| s.0.clone())))
        .collect();
    if new.is_empty() {
        return;
    }
    let restore = if new.iter().any(|(_, s)| s.is_some()) { take_restore(data) } else { None };
    for (e, slot) in new {
        let mut care = AnimalCare::default();
        if let (Some(r), Some(slot)) = (&restore, &slot) {
            if let Some(saved) = r.care.iter().find(|c| c.slot == *slot) {
                care = AnimalCare::from_record(saved);
            }
            care.away_s = r.away_s;
        }
        let _ = world.insert_one(e, care);
    }
}

/// A herd animal that died goes on the lost list (its corpse is despawned after a while, and
/// its slot must not come back at the next world entry), and a living animal in a lost slot
/// (a herd that was in the world when a save said it had died) leaves.
fn keep_the_dead(world: &mut hecs::World, data: &DataStore) {
    let Some(Ok(mut lost)) = data.get::<Mutex<Vec<LostAnimal>>>(LOST_KEY).map(|m| m.lock()) else { return };
    let mut gone = Vec::new();
    for (e, (slot, dead)) in world.query::<(&HerdSlot, Option<&Dead>)>().iter() {
        let known = lost.iter().any(|l| l.slot == slot.0);
        match (dead, known) {
            (Some(d), false) => {
                let cause = if d.cause.is_empty() { "killed".to_string() } else { d.cause.clone() };
                lost.push(LostAnimal { slot: slot.0.clone(), cause });
            }
            (None, true) => gone.push(e),
            _ => {}
        }
    }
    drop(lost);
    for e in gone {
        let _ = world.despawn(e);
    }
}

/// Tell the player what changed for this animal, once per state, by species.
fn tell(care: &mut AnimalCare, said: &mut Said, sp: &SpeciesCare, cd: &CareData, items: Option<&ItemRegistry>, mode: Mode, name: &str) {
    if care.no_feed && care.told & TOLD_NO_FEED == 0 {
        care.told |= TOLD_NO_FEED;
        said.say(format!("Your {} have nothing to eat: put {} in home storage", sp.plural, feed_names(sp, cd, items, 3)));
    } else if !care.no_feed {
        care.told &= !TOLD_NO_FEED;
    }
    if care.no_water && care.told & TOLD_NO_WATER == 0 {
        care.told |= TOLD_NO_WATER;
        said.say(format!("Your {} have no water: the home's water tanks are empty", sp.plural));
    } else if !care.no_water {
        care.told &= !TOLD_NO_WATER;
    }
    if care.sick(sp, mode) && care.told & TOLD_SICK == 0 {
        care.told |= TOLD_SICK;
        let what = if care.hungry_h >= sp.feed.sick_h { "feed it" } else { "give it water" };
        said.say(format!("A {name} is sick from going without: {what} soon or it will die"));
    } else if !care.sick(sp, mode) {
        care.told &= !TOLD_SICK;
    }
}

/// The time away's one line.
fn away_line(away: &AwayFed, items: Option<&ItemRegistry>) -> String {
    let name = |id: &str| items.and_then(|r| r.items.get(id)).map_or_else(|| id.to_string(), |d| d.name.clone());
    let list: Vec<String> = away.taken.iter().map(|(id, n)| format!("{n} {}", name(id))).collect();
    let ate = if list.is_empty() {
        "While you were away your animals found no feed in home storage".to_string()
    } else {
        format!("While you were away your animals ate {} from home storage", crate::systems::crafting::away::join_list(&list))
    };
    if away.short > 0 && !list.is_empty() {
        format!("{ate}, and it ran out: {} went hungry", count(away.short, "animal", "animals"))
    } else if away.short > 0 {
        format!("{ate}: {} went hungry", count(away.short, "animal", "animals"))
    } else {
        ate
    }
}

fn count(n: u32, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

// ── What the player reads ───────────────────────────────────────────────────────────────────

/// The first `n` feeds a species eats, by name: "Hay, Whole Oats or Covered Barley".
pub fn feed_names(sp: &SpeciesCare, cd: &CareData, items: Option<&ItemRegistry>, n: usize) -> String {
    let names: Vec<String> = sp
        .eats
        .iter()
        .filter(|id| cd.feeds.contains_key(*id))
        .take(n)
        .map(|id| items.and_then(|r| r.items.get(id)).map_or_else(|| id.clone(), |d| d.name.clone()))
        .collect();
    match names.len() {
        0 => "feed".to_string(),
        1 => names[0].clone(),
        k => format!("{} or {}", names[..k - 1].join(", "), names[k - 1]),
    }
}

/// A mass as people say it: "110 g", "2.4 kg", "22 kg".
fn mass_words(kg: f64) -> String {
    if kg < 1.0 {
        format!("{:.0} g", (kg * 1000.0 / 5.0).round() * 5.0)
    } else if kg < 10.0 {
        format!("{kg:.1} kg")
    } else {
        format!("{kg:.0} kg")
    }
}

/// Litres as people say them: "0.25 L", "8.4 L", "83 L".
fn litre_words(l: f64) -> String {
    if l < 1.0 {
        format!("{l:.2} L")
    } else if l < 10.0 {
        format!("{l:.1} L")
    } else {
        format!("{l:.0} L")
    }
}

/// A wait as people say it: "4 min", "18 h 20 min", "12 days".
pub fn wait_words(seconds: f64) -> String {
    let s = seconds.max(0.0);
    if s < 60.0 {
        "under a minute".to_string()
    } else if s < 36.0 * HOUR_S {
        crate::systems::construction::fires::span(s as f32)
    } else {
        format!("{:.0} days", (s / DAY_S).round())
    }
}

/// What it eats and drinks a day: "Eats about 110 g of Corn Grain and drinks about 0.25 L of
/// water a day." The feed named is the first in its list that home storage has (the one it is
/// eating), else the first in its list.
pub fn needs_line(sp: &SpeciesCare, cd: &CareData, items: Option<&ItemRegistry>, stock: Option<&HashMap<String, u32>>) -> String {
    let (feed, water) = needs_parts(sp, cd, items, stock);
    match feed {
        Some(feed) => format!("Eats about {feed} and drinks about {water} of water a day."),
        None => format!("Drinks about {water} of water a day."),
    }
}

/// The two halves of [`needs_line`] apart, for the Home page's table: the feed a day ("110 g of
/// Corn Grain"; None for an animal that eats nothing the data names) and the water a day ("0.25 L").
pub fn needs_parts(sp: &SpeciesCare, cd: &CareData, items: Option<&ItemRegistry>, stock: Option<&HashMap<String, u32>>) -> (Option<String>, String) {
    let in_stock = |id: &&String| stock.is_some_and(|s| s.get(*id).copied().unwrap_or(0) > 0);
    let pick = sp.eats.iter().filter(|id| cd.feeds.contains_key(*id)).find(in_stock).or_else(|| sp.eats.iter().find(|id| cd.feeds.contains_key(*id)));
    let feed = pick.map(|id| {
        let as_fed = sp.dry_matter_kg_day / cd.feeds[id].max(1e-9);
        let name = items.and_then(|r| r.items.get(id)).map_or_else(|| id.clone(), |d| d.name.clone());
        format!("{} of {name}", mass_words(as_fed))
    });
    (feed, litre_words(sp.water_l_day))
}

/// How it is, in plain words: "Fed and watered", "Hungry and very thirsty", "Sick from hunger".
pub fn condition_words(care: &AnimalCare, sp: &SpeciesCare, mode: Mode) -> String {
    let feed = if care.hungry_h <= sp.feed.slows_h {
        None
    } else if mode == Mode::Realistic && care.hungry_h >= sp.feed.sick_h {
        Some("sick from hunger")
    } else if care.hungry_h >= sp.feed.stops_h {
        Some("very hungry")
    } else {
        Some("hungry")
    };
    let water = if care.thirsty_h <= sp.water.slows_h {
        None
    } else if mode == Mode::Realistic && care.thirsty_h >= sp.water.sick_h {
        Some("sick from thirst")
    } else if care.thirsty_h >= sp.water.stops_h {
        Some("very thirsty")
    } else {
        Some("thirsty")
    };
    let mut line = match (feed, water) {
        (None, None) => "Fed and watered".to_string(),
        (Some(f), None) => capitalise(f),
        (None, Some(w)) => capitalise(w),
        (Some(f), Some(w)) => format!("{} and {w}", capitalise(f)),
    };
    let mut why = Vec::new();
    if care.no_feed {
        why.push("no feed in home storage");
    }
    if care.no_water {
        why.push("the water tanks are empty");
    }
    if !why.is_empty() {
        line.push_str(&format!(" ({})", why.join(", ")));
    }
    line
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// When its next yield is due, in plain words: "Egg in 18 h", "Egg ready to collect",
/// "Egg in about 3 days at half rate", "No Egg until it is fed and watered".
fn yield_words(h: &Harvestable, item: &str, share: f64) -> String {
    if h.time_since_harvest + f32::EPSILON >= h.regrow_time {
        return format!("{item} ready to collect");
    }
    if share <= 0.0 {
        return format!("No {item} until it is fed and watered");
    }
    let left = f64::from(h.regrow_time - h.time_since_harvest) / share;
    if share < 0.999 {
        format!("{item} in about {} at its slower rate", wait_words(left))
    } else {
        format!("{item} in {}", wait_words(left))
    }
}

/// The two lines under the crosshair for the animal (or forage plant) `e` the player faces:
/// the prompt ("[E] collect Egg (Chicken)", "Chicken: Egg in 18 h") and, for a cared-for
/// animal, how it is and what it needs (empty otherwise). lib.rs's walk-up block shows them.
pub fn walk_up_lines(world: &hecs::World, e: hecs::Entity, data: &DataStore) -> (String, String) {
    let name = world.get::<&Name>(e).map(|n| n.0.clone()).unwrap_or_else(|_| "animal".to_string());
    let Ok(h) = world.get::<&Harvestable>(e) else {
        // No renewable product: a wild creature, for abilities or a swing.
        return (format!("{name} - 1-9 casts, F swings"), String::new());
    };
    let items = data.get::<ItemRegistry>("item_registry");
    let item = items.and_then(|r| r.items.get(&h.resource)).map_or_else(|| h.resource.clone(), |d| d.name.clone());
    let mode = Mode::from_store(data);
    let cd = data.get::<CareData>(DATA_KEY);
    let care = world.get::<&AnimalCare>(e).ok().map(|c| (*c).clone());
    let species = world.get::<&Creature>(e).ok().map(|c| c.def_id.clone()).unwrap_or_default();
    let sp = cd.and_then(|cd| cd.species.get(&species));
    let share = match (&care, sp) {
        (Some(c), Some(sp)) => c.production(sp, mode),
        _ => 1.0,
    };
    let prompt = if h.time_since_harvest + f32::EPSILON >= h.regrow_time {
        format!("[E] collect {item} ({name})")
    } else {
        format!("{name}: {}", yield_words(&h, &item, share))
    };
    let line = match (care, sp, cd) {
        (Some(c), Some(sp), Some(cd)) => {
            let stock = data.get::<Mutex<HashMap<String, u32>>>("home_stock").and_then(|m| m.lock().ok().map(|s| s.clone()));
            format!("{}. {}", condition_words(&c, sp, mode), needs_line(sp, cd, items, stock.as_ref()))
        }
        _ => String::new(),
    };
    (prompt, line)
}

/// One row of the Home page's Animals card: the whole lines its cards show, and the same facts
/// apart for its table's columns (2026-10-30, the operator's "multiview list").
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HerdRow {
    /// "Chicken 1".
    pub title: String,
    /// "Fed and watered. Egg in 18 h."
    pub status: String,
    /// "Eats about 110 g of Corn Grain and drinks about 0.25 L of water a day."
    pub needs: String,
    /// Something is wrong (hungry, thirsty, sick, dead): drawn in the warning colour.
    pub alert: bool,
    /// The kind of animal: "Chicken".
    pub kind: String,
    /// How it is, alone: "Fed and watered", "Hungry (no feed in home storage)", "Starved to death".
    pub condition: String,
    /// What it makes and when: "Egg in 18 h", "Egg ready to collect"; empty when it makes nothing.
    pub product: String,
    /// What it makes is ready to collect now.
    pub ready: bool,
    /// Its feed a day, "110 g of Corn Grain"; empty when it eats nothing named, or is dead.
    pub feed: String,
    /// Its water a day, "0.25 L"; empty when dead.
    pub water: String,
    /// It died (starved, of thirst).
    pub dead: bool,
}

/// "Chicken 1" for the herd slot "chicken#0", by the species' name.
fn slot_title(slot: &str, species_name: &str) -> String {
    match slot.rsplit_once('#').and_then(|(_, n)| n.parse::<u32>().ok()) {
        Some(n) => format!("{species_name} {}", n + 1),
        None => species_name.to_string(),
    }
}

/// Every cared-for animal, and every herd animal that died, as the Home page's Animals card
/// shows them (lib.rs puts these in `GuiState::herd` each frame). Herd animals first, in slot
/// order; empty with no care data.
pub fn herd_rows(world: &hecs::World, data: &DataStore) -> Vec<HerdRow> {
    let Some(cd) = data.get::<CareData>(DATA_KEY) else { return Vec::new() };
    let mode = Mode::from_store(data);
    let items = data.get::<ItemRegistry>("item_registry");
    let creatures = data.get::<crate::systems::livestock::CreatureRegistry>("creature_registry");
    let species_name = |id: &str| creatures.and_then(|r| r.get(id)).map_or_else(|| capitalise(id), |d| d.name.clone());
    let stock = data.get::<Mutex<HashMap<String, u32>>>("home_stock").and_then(|m| m.lock().ok().map(|s| s.clone()));
    let mut rows: Vec<(String, HerdRow)> = Vec::new();
    for (_e, (c, care, h, slot, name, dead)) in world
        .query::<(&Creature, &AnimalCare, Option<&Harvestable>, Option<&HerdSlot>, Option<&Name>, Option<&Dead>)>()
        .iter()
    {
        if dead.is_some() {
            continue;
        }
        let Some(sp) = cd.species.get(&c.def_id) else { continue };
        let title = match slot {
            Some(s) => slot_title(&s.0, &species_name(&c.def_id)),
            None => name.map_or_else(|| species_name(&c.def_id), |n| n.0.clone()),
        };
        let share = care.production(sp, mode);
        let product = h
            .map(|h| {
                let item = items.and_then(|r| r.items.get(&h.resource)).map_or_else(|| h.resource.clone(), |d| d.name.clone());
                yield_words(h, &item, share)
            })
            .unwrap_or_default();
        let ready = h.is_some_and(|h| h.time_since_harvest + f32::EPSILON >= h.regrow_time);
        let alert = care.hungry_h > sp.feed.slows_h || care.thirsty_h > sp.water.slows_h || care.no_feed || care.no_water;
        let key = slot.map_or_else(|| format!("~{title}"), |s| s.0.clone());
        let condition = condition_words(care, sp, mode);
        let (feed, water) = needs_parts(sp, cd, items, stock.as_ref());
        rows.push((
            key,
            HerdRow {
                title,
                status: if product.is_empty() { format!("{condition}.") } else { format!("{condition}. {product}.") },
                needs: needs_line(sp, cd, items, stock.as_ref()),
                alert,
                kind: species_name(&c.def_id),
                condition,
                product,
                ready,
                feed: feed.unwrap_or_default(),
                water,
                dead: false,
            },
        ));
    }
    for l in lost(data) {
        let species = l.slot.split('#').next().unwrap_or_default();
        let how = match l.cause.as_str() {
            "starved" => "Starved to death",
            "died of thirst" => "Died of thirst",
            _ => "Dead",
        };
        rows.push((
            l.slot.clone(),
            HerdRow {
                title: slot_title(&l.slot, &species_name(species)),
                status: format!("{how}."),
                alert: true,
                kind: species_name(species),
                condition: how.to_string(),
                dead: true,
                ..HerdRow::default()
            },
        ));
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    rows.into_iter().map(|(_, r)| r).collect()
}

/// Settings > Gameplay > Farm animals's hint: what each mode does, with the chicken's numbers
/// read from the data it describes, so the two cannot drift.
pub fn mode_hint() -> String {
    static SHIPPED: std::sync::OnceLock<CareData> = std::sync::OnceLock::new();
    hint_text(SHIPPED.get_or_init(|| CareData::load(&crate::data_dir())))
}

fn hint_text(cd: &CareData) -> String {
    let days = |h: f64| crate::systems::illness::duration_words((h * HOUR_S) as f32);
    let example = cd.species.get("chicken").map(|c| {
        format!(
            " A chicken without water stops laying after {}, falls sick after {} and dies after {}; without feed it stops laying after {}.",
            days(c.water.stops_h),
            days(c.water.sick_h),
            days(c.water.dies_h),
            days(c.feed.stops_h)
        )
    });
    format!(
        "Your animals eat and drink every day: feed from home storage (the Barn) and water from the home's \
         water tanks. Short of either, they make less, then nothing. Realistic: left longer, they fall sick \
         and die.{} Forgiving: going without only stops what they make; they never fall sick or die of it, \
         and they come back as fast as they fell off.",
        example.unwrap_or_default()
    )
}

#[cfg(test)]
#[path = "animal_care_tests.rs"]
mod tests;
