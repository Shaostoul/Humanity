//! THE MESS HALL'S COUNTER ON THIS COMPUTER (ship homes increment 6, lane 1D,
//! docs/design/ship-homes-increment-6-plan.md 3.8): a free crew meal, one a meal time, and
//! drinking water, when no server is counting them.
//!
//! In a server's shared world the server keeps the ship's stores and counts each meal on the
//! player's fleet ledger (src/relay/handlers/ship_stores.rs, `take_meal`); the game only asks
//! (engine/mess_hall.rs). Away from a server, in your own home on the same ship, there is no one
//! to ask, so the ship's stores are counted here, on the save (`persistence::ShipMeals`, the
//! DataStore's `save_load::SHIP_MEALS_KEY`): how many meals the stores have given, when the next
//! one is ready on the world's GAME clock, and how many litres of drinking water. The operator's
//! rule (decision 4 of that plan, 2026-10-12): meals any time, one a meal time per person, so a
//! short session never goes hungry; hot meals at set times come with cooking.
//!
//! Both numbers come from the files the server reads, so a meal here is the same meal as there:
//!   - the meal time: `meal_interval_hours` of data/food/ship_stores.ron (8: three a day);
//!   - what a meal is: the "meal" kind's price in data/ship/fleet_ledger.ron, `OfGood(<item>)`,
//!     a Basic Ration (`ration_basic_0`), which the food system then eats.
//!
//! What fills a bottle: every empty water vessel of data/containers/fluids.ron (`fillables`)
//! the backpack holds becomes the same vessel full, from the ship's water. The ship's water is
//! not metered against anyone's tanks yet: the litres are only counted (`water_l`), and the
//! fleet ledger has no water line (a logged follow-up of the plan).
//!
//! Compiled into the relay too (no native-only crates): the rules and the swap are plain data.

use std::path::Path;

use serde::Deserialize;

use crate::persistence::ShipMeals;
use crate::systems::fluids::FluidTable;
use crate::systems::inventory::{Inventory, ItemRegistry};

/// The ship's stores' file, under data/ (the relay reads the same one).
pub const STORES_FILE: &str = "food/ship_stores.ron";
/// The fleet ledger's file, under data/, whose "meal" kind names the good a meal is.
pub const LEDGER_FILE: &str = "ship/fleet_ledger.ron";

/// What the counter serves and how often: read from the server's own files.
#[derive(Debug, Clone, PartialEq)]
pub struct MealRules {
    /// Game seconds between one person's meals.
    pub interval_s: f64,
    /// The item one meal is (a Basic Ration).
    pub item: String,
}

/// The parts of ship_stores.ron this reads (serde passes over the rest).
#[derive(Deserialize)]
struct StoresPart {
    meal_interval_hours: f64,
}

/// The parts of fleet_ledger.ron this reads.
#[derive(Deserialize)]
struct LedgerPart {
    kinds: Vec<KindPart>,
}

#[derive(Deserialize)]
struct KindPart {
    id: String,
    price: PricePart,
}

/// The ledger's `Price`, as written there (src/relay/handlers/fleet_ledger.rs `Price`).
#[derive(Deserialize)]
#[allow(dead_code)] // the other two prices are read only to be passed over
enum PricePart {
    OfGood(String),
    OfItem,
    Fixed(f64),
}

impl MealRules {
    /// Read both rules from their texts. The meal time must be a number above 0, and the
    /// "meal" kind must be priced as one good (that good is what is eaten).
    pub fn parse(stores_text: &str, ledger_text: &str) -> Result<MealRules, String> {
        let stores: StoresPart = ron::from_str(stores_text).map_err(|e| format!("{STORES_FILE}: {e}"))?;
        if !(stores.meal_interval_hours.is_finite() && stores.meal_interval_hours > 0.0) {
            return Err(format!("{STORES_FILE}: meal_interval_hours must be a number above 0; it is {}", stores.meal_interval_hours));
        }
        let ledger: LedgerPart = ron::from_str(ledger_text).map_err(|e| format!("{LEDGER_FILE}: {e}"))?;
        let item = match ledger.kinds.into_iter().find(|k| k.id == "meal").map(|k| k.price) {
            Some(PricePart::OfGood(id)) if !id.is_empty() => id,
            Some(_) => return Err(format!("{LEDGER_FILE}: the meal kind is not priced as one good, so no good is a meal")),
            None => return Err(format!("{LEDGER_FILE}: no meal kind")),
        };
        Ok(MealRules { interval_s: stores.meal_interval_hours * crate::systems::time::SECONDS_PER_HOUR, item })
    }

    /// Both rules from the data folder, disk first; a file that is missing or does not read is
    /// replaced by the copy built into the exe, and the log says so (the relay's rule for these
    /// same files). Both built-in copies are tested to read.
    pub fn load(data_dir: &Path) -> MealRules {
        // Disk first; a missing file is the built-in copy, which read_data_or_embedded logs.
        let read = |rel: &str| crate::embedded_data::read_data_or_embedded(data_dir, rel).unwrap_or_default();
        let (stores, ledger) = (read(STORES_FILE), read(LEDGER_FILE));
        Self::parse(&stores, &ledger).unwrap_or_else(|e| {
            // This version cannot read the folder's copies: both built-in ones, said in the log.
            crate::embedded_data::note_builtin_copy("food/ship_stores.ron", format_args!("{e}"));
            crate::embedded_data::note_builtin_copy("ship/fleet_ledger.ron", format_args!("{e}"));
            Self::parse(crate::embedded_data::SHIP_STORES_RON, crate::embedded_data::FLEET_LEDGER_RON)
                .expect("the built-in ship_stores.ron and fleet_ledger.ron read (tested)")
        })
    }

    /// The rules as this exe was built with them (tests: the built-in copies read).
    #[cfg(test)]
    pub fn built_in() -> MealRules {
        Self::parse(crate::embedded_data::SHIP_STORES_RON, crate::embedded_data::FLEET_LEDGER_RON)
            .expect("the built-in ship_stores.ron and fleet_ledger.ron read (tested)")
    }
}



/// What the counter does when asked for a meal.
#[derive(Debug, Clone, PartialEq)]
pub enum Served {
    /// One meal, this item, counted on the meter; the caller has it eaten.
    Meal { item: String },
    /// Not yet: the next meal is ready in this many game seconds.
    NotYet { ready_in_s: f64 },
}

/// One meal from the ship's stores at game time `now_game_s`, when one is due: counted on
/// `meter` (`taken` up by one, the next ready a meal time from now). A meter whose next meal is
/// further off than one meal time (the world's clock went back: an older save's clock, or a
/// fresh world each launch) serves at once, so a wait is never longer than one meal time.
pub fn serve(meter: &mut ShipMeals, now_game_s: f64, rules: &MealRules) -> Served {
    let wait = meter.next_at_game_s - now_game_s;
    if wait > 0.0 && wait <= rules.interval_s {
        return Served::NotYet { ready_in_s: wait };
    }
    meter.taken += 1;
    meter.next_at_game_s = now_game_s + rules.interval_s;
    Served::Meal { item: rules.item.clone() }
}

/// "Your next crew meal is ready in 1 h 59 min." (game time; minutes rounded up).
pub fn not_yet_sentence(ready_in_s: f64) -> String {
    let minutes = (ready_in_s.max(0.0) / 60.0).ceil() as u64;
    let (h, m) = (minutes / 60, minutes % 60);
    if h == 0 {
        format!("Your next crew meal is ready in {m} min.")
    } else {
        format!("Your next crew meal is ready in {h} h {m} min.")
    }
}

/// What filling the backpack's vessels did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Filled {
    /// Each vessel filled: its plain name ("Water Bottle") and how many.
    pub vessels: Vec<(String, u32)>,
    /// Litres of drinking water drawn.
    pub litres: f64,
    /// Some empty vessel was carried but the backpack had no room for it full.
    pub no_room: bool,
}

impl Filled {
    /// How many vessels were filled.
    pub fn count(&self) -> u32 {
        self.vessels.iter().map(|(_, n)| n).sum()
    }

    /// The sentence the counter's card shows.
    pub fn sentence(&self) -> String {
        if self.vessels.is_empty() {
            return if self.no_room {
                "No room in your backpack for them full.".to_string()
            } else {
                "You carry no empty bottle to fill.".to_string()
            };
        }
        let parts: Vec<String> = self.vessels.iter().map(|(name, n)| format!("{n} x {name}")).collect();
        format!("Filled {} ({}) from the ship's water.", parts.join(", "), litres_text(self.litres))
    }
}

/// "1.5 L", "20 L".
pub fn litres_text(l: f64) -> String {
    if (l - l.round()).abs() < 0.05 {
        format!("{} L", l.round())
    } else {
        format!("{l:.1} L")
    }
}

/// Fill every empty WATER vessel the player's backpack holds (fluids.ron's fillables) from the
/// ship's water. A swap, as at a tank (fluids::apply_action): the empties go out before the full
/// ones are counted, and any that do not fit full go back empty, so nothing is dropped.
pub fn fill_vessels(table: &FluidTable, world: &mut hecs::World, reg: Option<&ItemRegistry>) -> Filled {
    let mut out = Filled::default();
    let Some((_, (inv, _))) = world.query_mut::<(&mut Inventory, &crate::ecs::components::Controllable)>().into_iter().next() else {
        return out;
    };
    for f in table.fillables.iter().filter(|f| f.fluid == "water") {
        let n = inv.count_item(&f.empty);
        if n == 0 {
            continue;
        }
        let take_vol = reg.map_or(0.0, |r| r.volume_for(&f.empty));
        let give_vol = reg.map_or(0.0, |r| r.volume_for(&f.full));
        inv.remove_item(&f.empty, n);
        inv.volume_current_l = (inv.volume_current_l - n as f32 * take_vol).max(0.0);
        let lost = inv.add_item_volume_gated(&f.full, n, reg.map_or(99, |r| r.max_stack_for(&f.full)), give_vol);
        if lost > 0 {
            let occupied = inv.slots.iter().filter(|s| s.is_some()).count();
            inv.ensure_slots(occupied + lost as usize);
            inv.add_item(&f.empty, lost, reg.map_or(99, |r| r.max_stack_for(&f.empty)));
            inv.volume_current_l += lost as f32 * take_vol;
            out.no_room = true;
        }
        let done = n - lost;
        if done > 0 {
            out.vessels.push((vessel_name(reg, &f.empty), done));
            out.litres += f64::from(done) * f64::from(f.litres);
        }
    }
    out
}

/// A vessel's plain name: its empty item's name without " (empty)".
fn vessel_name(reg: Option<&ItemRegistry>, empty: &str) -> String {
    let n = reg.and_then(|r| r.items.get(empty).map(|d| d.name.clone())).unwrap_or_else(|| empty.to_string());
    n.strip_suffix(" (empty)").map(str::to_string).unwrap_or(n)
}

#[cfg(test)]
#[path = "ship_meals_tests.rs"]
mod tests;
