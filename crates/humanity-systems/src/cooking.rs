//! COOKING AS A PROCEDURE (cooking rung 1, 2026-11-21; docs/design/cooking.md).
//!
//! A hand-cooked dish has a method (boiled, fried, baked, roasted, grilled, steamed), and the cook
//! chooses a heat and a time. What comes out follows from a model of the food's inside heating
//! toward the temperature around it: undercooked, done, overcooked or burnt, and, separately,
//! safe or not (a food class's safety line, from CDC and FDA, in `data/food_system.ron`
//! `food_classes`). The station picks a sensible heat and time in the Simplified level (it always
//! comes out done); Guided and Realistic let the cook choose and get it wrong, Realistic showing
//! the temperature inside only with a food thermometer in the backpack.
//!
//! The data is food_system.ron's (`cooking_methods`, `food_classes`, `dishes`, `not_dishes`,
//! `meal_quality_levels`), read once into a [`Cooking`] under [`DATA_KEY`]. The crafting system
//! asks [`start`] when a hand craft of a dish begins and carries the [`CookResult`] to its end; the
//! food system reads a dish's [`DishState`] off the eaten stack's grade byte. Nothing here runs per
//! frame: one closed-form evaluation per craft, and the cooking card's preview while it is drawn.

use std::path::Path;

use humanity_core::hot_reload::data_store::DataStore;
use serde::{Deserialize, Serialize};

/// DataStore: the [`Cooking`] tables (the engine's registries, at boot).
pub const DATA_KEY: &str = "cooking";
/// DataStore: the chosen level of Settings > Gameplay > Cooking ([`Mode`]), published each frame.
/// Absent: the default level, Simplified.
pub const MODE_KEY: &str = "cooking_mode";
/// DataStore: `Mutex<Option<Choice>>`, the heat and time the cooking card chose for the next
/// craft of its recipe (the GUI's request bridge puts it beside the craft request).
pub const CHOICE_KEY: &str = "cook_choice";
/// The game's boiling point of water, at sea-level pressure. A pot cannot get hotter than this,
/// and dry beans are safe only after time at it (FDA). Rung 3 lowers it with the air's pressure.
pub const BOILING_C: f32 = 100.0;

/// Settings > Gameplay > Cooking (data/gameplay/levels.json, 2026-11-21).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// The station picks the heat and the time; it always comes out done and safe.
    #[default]
    Simplified,
    /// The cook chooses, and sees the temperature inside as a thermometer would show it.
    Guided,
    /// The cook chooses, and sees the temperature inside only with a food thermometer.
    Realistic,
}

impl Mode {
    /// The mode a level of data/gameplay/levels.json names, None for one this system does not know.
    pub fn from_level(level: &str) -> Option<Self> {
        match level {
            "simplified" => Some(Self::Simplified),
            "guided" => Some(Self::Guided),
            "realistic" => Some(Self::Realistic),
            _ => None,
        }
    }

    /// The level published this frame, the default when none is.
    pub fn from_store(data: &DataStore) -> Self {
        data.get::<Mode>(MODE_KEY).copied().unwrap_or_default()
    }

    /// Whether the cook chooses the heat and the time.
    pub fn chooses(self) -> bool {
        self != Self::Simplified
    }
}

/// The item that shows the temperature inside food in the Realistic level.
pub const THERMOMETER: &str = "food_thermometer_0";

/// A station's heat setting. The data names them "low", "medium" and "high".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Heat {
    Low,
    Medium,
    High,
}

impl Heat {
    pub const ALL: [Heat; 3] = [Heat::Low, Heat::Medium, Heat::High];

    pub fn id(self) -> &'static str {
        match self {
            Heat::Low => "low",
            Heat::Medium => "medium",
            Heat::High => "high",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|h| h.id() == id)
    }
}

/// One row of food_system.ron `cooking_methods`: the heat around the food.
#[derive(Debug, Clone, Deserialize)]
pub struct CookingMethod {
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// The temperature around the food at low, medium and high heat; None: no heat.
    #[serde(default)]
    pub medium_c: Option<(f32, f32, f32)>,
    /// How fast the medium heats food, boiling water 1.0.
    #[serde(default)]
    pub heat_transfer: f32,
    /// The skill a dish's meal grade reads when its recipe names none.
    #[serde(default)]
    pub skill_required: Option<String>,
}

impl CookingMethod {
    /// The temperature around the food at `heat`, None for a method with no heat.
    pub fn medium(&self, heat: Heat) -> Option<f32> {
        self.medium_c.map(|(lo, me, hi)| match heat {
            Heat::Low => lo,
            Heat::Medium => me,
            Heat::High => hi,
        })
    }
}

/// One row of food_system.ron `meal_quality_levels`. Fields nothing acts on yet (morale, health,
/// reputation, ingredient quality) are left to serde to skip.
#[derive(Debug, Clone, Deserialize)]
pub struct MealQuality {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub min_cook_skill: u32,
    #[serde(default)]
    pub status_effects: Vec<String>,
    #[serde(default)]
    pub status_effect_chance: f32,
}

/// A food class's safety line (food_system.ron `food_classes`, each with its source there).
#[derive(Debug, Clone, Deserialize)]
pub struct Safe {
    /// The inside must reach this.
    #[serde(default)]
    pub core_c: Option<f32>,
    /// The medium must be at a full boil ([`BOILING_C`]) at least this many minutes.
    #[serde(default)]
    pub boil_min: Option<f32>,
    /// Rest after cooking (said on the card; resting is rung 2).
    #[serde(default)]
    pub rest_min: f32,
    /// Safe only once the dish is done: raw dough, batter or egg in it (CDC's riskier foods),
    /// with no temperature of its own.
    #[serde(default)]
    pub done: bool,
    /// The card's words for this line (an en.json key) when the general ones would say more
    /// than the source does (whole eggs: firm, not a temperature).
    #[serde(default)]
    pub say: Option<String>,
}

/// What an overcooked dish keeps again of its vitamin C and B group (USDA retention factors).
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Keeps {
    pub c: f32,
    pub b: f32,
}

/// One row of food_system.ron `food_classes`.
#[derive(Debug, Clone, Deserialize)]
pub struct FoodClass {
    pub id: String,
    #[serde(default)]
    pub raw_profile: Option<String>,
    pub start_c: f32,
    pub heat_up_min: f32,
    pub done_c: f32,
    #[serde(default)]
    pub tender_min: f32,
    #[serde(default)]
    pub safe: Option<Safe>,
    pub overcook_after_min: f32,
    pub burn_after_min: f32,
    pub burns_above_c: f32,
    #[serde(default)]
    pub keeps: Option<Keeps>,
}

/// One row of food_system.ron `dishes`: a hand-cooked recipe and how it cooks.
#[derive(Debug, Clone, Deserialize)]
pub struct Dish {
    pub recipe: String,
    /// The item it makes, which an eaten dish's state is read against.
    pub makes: String,
    pub method: String,
    pub class: String,
    /// The heat the station picks in Simplified ("low", "medium", "high").
    pub heat: String,
    /// This dish's own heat-up time constant, when it differs from its class's.
    #[serde(default)]
    pub heat_up_min: Option<f32>,
    /// The raw food whose risk this dish keeps undercooked, when it is not its class's (a cake
    /// carries raw egg, a loaf only raw flour).
    #[serde(default)]
    pub raw_profile: Option<String>,
    /// What this dish keeps again overcooked, when USDA has a row for it of its own (rice, pasta).
    #[serde(default)]
    pub keeps: Option<Keeps>,
}

/// How a dish came out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Doneness {
    Undercooked,
    Done,
    Overcooked,
    Burnt,
}

impl Doneness {
    /// Its words' key in data/i18n/en.json.
    pub fn key(self) -> &'static str {
        match self {
            Doneness::Undercooked => "cooking.doneness_undercooked",
            Doneness::Done => "cooking.doneness_done",
            Doneness::Overcooked => "cooking.doneness_overcooked",
            Doneness::Burnt => "cooking.doneness_burnt",
        }
    }
}

/// The model's answer for one heat and time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Outcome {
    pub doneness: Doneness,
    /// Safe to eat as far as cooking goes (always, for a class with no safety line).
    pub safe: bool,
    /// The temperature inside at the end, what a thermometer would read.
    pub core_c: f32,
    /// The temperature around the food.
    pub medium_c: f32,
    /// Minutes until it is done at this heat, None when this heat never gets it done.
    pub ready_min: Option<f32>,
}

/// Minutes until the inside reaches `target` on the first-order curve, None when the medium
/// never gets it there (a medium at or below the target).
fn minutes_to(start: f32, medium: f32, tau: f32, target: f32) -> Option<f32> {
    if target <= start {
        return Some(0.0);
    }
    if medium <= target {
        return None;
    }
    Some(tau * ((medium - start) / (medium - target)).ln())
}

/// The temperature inside after `minutes` in a medium at `medium`.
pub fn core_after(start: f32, medium: f32, tau: f32, minutes: f32) -> f32 {
    medium - (medium - start) * (-minutes.max(0.0) / tau.max(1e-3)).exp()
}

/// How `class` cooked by `method` at `heat` for `minutes` comes out (docs/design/cooking.md
/// section 1). `heat_up_min` is the dish's own time constant when it has one.
pub fn outcome(method: &CookingMethod, class: &FoodClass, heat_up_min: Option<f32>, heat: Heat, minutes: f32) -> Outcome {
    let Some(medium) = method.medium(heat) else {
        // No heat: it stays as it was, raw.
        return Outcome { doneness: Doneness::Undercooked, safe: class.safe.is_none(), core_c: class.start_c, medium_c: class.start_c, ready_min: None };
    };
    let tau = heat_up_min.unwrap_or(class.heat_up_min) / method.heat_transfer.max(0.01);
    let core = core_after(class.start_c, medium, tau, minutes);
    let ready = minutes_to(class.start_c, medium, tau, class.done_c).map(|t| t + class.tender_min);
    let doneness = match ready {
        Some(r) if minutes >= r => {
            let past = minutes - r;
            if medium >= class.burns_above_c && past >= class.burn_after_min {
                Doneness::Burnt
            } else if past >= class.overcook_after_min {
                Doneness::Overcooked
            } else {
                Doneness::Done
            }
        }
        _ => Doneness::Undercooked,
    };
    let safe = match &class.safe {
        None => true,
        Some(s) => {
            // By time, not by comparing temperatures, so a dish exactly at its line is safe.
            let core_ok = s.core_c.map_or(true, |c| minutes_to(class.start_c, medium, tau, c).is_some_and(|t| minutes >= t));
            let boil_ok = s.boil_min.map_or(true, |m| medium >= BOILING_C && minutes >= m);
            let done_ok = !s.done || ready.is_some_and(|r| minutes >= r);
            core_ok && boil_ok && done_ok
        }
    };
    Outcome { doneness, safe, core_c: core, medium_c: medium, ready_min: ready }
}

/// The heat and time the station picks in Simplified: the dish's own heat (high when that heat
/// never gets it done), and whole minutes one past what makes it done and safe. None when no
/// heat does (a data error the tests refuse).
pub fn sensible(method: &CookingMethod, class: &FoodClass, dish: &Dish) -> Option<(Heat, f32)> {
    let first = Heat::from_id(&dish.heat).unwrap_or(Heat::Medium);
    for heat in [first, Heat::High] {
        let Some(medium) = method.medium(heat) else { continue };
        let tau = dish.heat_up_min.unwrap_or(class.heat_up_min) / method.heat_transfer.max(0.01);
        let Some(ready) = minutes_to(class.start_c, medium, tau, class.done_c).map(|t| t + class.tender_min) else { continue };
        let mut need = ready;
        if let Some(s) = &class.safe {
            if let Some(c) = s.core_c {
                match minutes_to(class.start_c, medium, tau, c) {
                    Some(t) => need = need.max(t),
                    None => continue,
                }
            }
            if let Some(m) = s.boil_min {
                if medium < BOILING_C {
                    continue;
                }
                need = need.max(m);
            }
        }
        return Some((heat, need.ceil() + 1.0));
    }
    None
}

/// A hand-cooked dish's state, carried on its stack's grade byte (`ItemStack::quality`, which no
/// food used: only durable goods are graded and no food is durable). 0 is "not cooked by hand"
/// (bought or made by a machine), which eats as its profile says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DishState {
    /// Undercooked, Done or Overcooked (a burnt dish is lost, so never carried).
    pub doneness: Doneness,
    pub safe: bool,
    /// Index into `meal_quality_levels`.
    pub meal: u8,
}

impl DishState {
    /// The byte: 1 + meal (0..7) + 8 x doneness (0..2) + 32 if safe. Burnt has none.
    pub fn to_byte(self) -> Option<u8> {
        let d = match self.doneness {
            Doneness::Undercooked => 0,
            Doneness::Done => 1,
            Doneness::Overcooked => 2,
            Doneness::Burnt => return None,
        };
        Some(1 + self.meal.min(7) + 8 * d + if self.safe { 32 } else { 0 })
    }

    /// The state a food stack's byte carries, None for 0 (not cooked by hand) or a byte no dish
    /// writes.
    pub fn from_byte(b: u8) -> Option<Self> {
        if b == 0 || b > 56 {
            return None;
        }
        let v = b - 1;
        let doneness = match (v / 8) % 4 {
            0 => Doneness::Undercooked,
            1 => Doneness::Done,
            2 => Doneness::Overcooked,
            _ => return None,
        };
        Some(Self { doneness, safe: v & 32 != 0, meal: v % 8 })
    }
}

/// What a hand craft of a dish came to, decided when it starts (the heat and the time are chosen
/// then) and carried with the craft, through a save, to its end.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CookResult {
    pub heat: Heat,
    pub minutes: f32,
    pub doneness: Doneness,
    pub safe: bool,
    /// The temperature inside at the end.
    pub core_c: f32,
    /// Index into `meal_quality_levels`.
    pub meal: u8,
}

impl CookResult {
    /// The state its food goes out with, None when it burnt.
    pub fn state(&self) -> Option<DishState> {
        (self.doneness != Doneness::Burnt).then_some(DishState { doneness: self.doneness, safe: self.safe, meal: self.meal })
    }

    /// Whether it counts as cooked through for a `Cook` quest step: done or overcooked, and safe.
    pub fn cooked_through(&self) -> bool {
        self.safe && matches!(self.doneness, Doneness::Done | Doneness::Overcooked)
    }
}

/// The heat and time the cooking card chose for the next craft of `recipe`.
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    pub recipe: String,
    pub heat: Heat,
    pub minutes: f32,
}

/// The cooking tables, read from food_system.ron.
#[derive(Debug, Clone, Default)]
pub struct Cooking {
    pub methods: Vec<CookingMethod>,
    pub classes: Vec<FoodClass>,
    pub dishes: Vec<Dish>,
    pub not_dishes: Vec<(String, String)>,
    pub meals: Vec<MealQuality>,
}

impl Cooking {
    /// Its tables out of the food data.
    pub fn from_food_data(data: &crate::food::FoodData) -> Self {
        Self {
            methods: data.cooking_methods.clone(),
            classes: data.food_classes.clone(),
            dishes: data.dishes.clone(),
            not_dishes: data.not_dishes.clone(),
            meals: data.meal_quality_levels.clone(),
        }
    }

    /// Disk first, the copy built into the exe otherwise, as the food system reads the file. With
    /// neither, no recipe is a dish and every craft is the plain recipe it was.
    pub fn load(data_dir: &Path) -> Self {
        match humanity_core::embedded_data::load_data_or_embedded(data_dir, "food_system.ron", humanity_core::data_parse::parse_ron::<crate::food::FoodData>) {
            Ok(d) => Self::from_food_data(&d),
            Err(e) => {
                log::warn!("{e}; no recipe cooks as a dish this session");
                Self::default()
            }
        }
    }

    pub fn dish(&self, recipe: &str) -> Option<&Dish> {
        self.dishes.iter().find(|d| d.recipe == recipe)
    }

    /// The dish that makes `item` (the first, should two).
    pub fn dish_for_item(&self, item: &str) -> Option<&Dish> {
        self.dishes.iter().find(|d| d.makes == item)
    }

    pub fn method(&self, id: &str) -> Option<&CookingMethod> {
        self.methods.iter().find(|m| m.id == id)
    }

    pub fn class(&self, id: &str) -> Option<&FoodClass> {
        self.classes.iter().find(|c| c.id == id)
    }

    /// A dish's method and class, None when either is missing.
    pub fn plan(&self, dish: &Dish) -> Option<(&CookingMethod, &FoodClass)> {
        Some((self.method(&dish.method)?, self.class(&dish.class)?))
    }

    /// How `dish` comes out at `heat` for `minutes`.
    pub fn outcome(&self, dish: &Dish, heat: Heat, minutes: f32) -> Option<Outcome> {
        let (m, c) = self.plan(dish)?;
        Some(outcome(m, c, dish.heat_up_min, heat, minutes))
    }

    /// What the station picks for `dish` in Simplified.
    pub fn sensible(&self, dish: &Dish) -> Option<(Heat, f32)> {
        let (m, c) = self.plan(dish)?;
        sensible(m, c, dish)
    }

    /// The meal grade a dish comes out at: the highest level whose `min_cook_skill` the cook's
    /// `skill_level` reaches when done; at most "poor" (the second level) overcooked; the lowest
    /// undercooked. 0 with no levels loaded.
    pub fn meal_level(&self, skill_level: u32, doneness: Doneness) -> u8 {
        let by_skill = self.meals.iter().rposition(|m| m.min_cook_skill <= skill_level).unwrap_or(0) as u8;
        match doneness {
            Doneness::Done => by_skill,
            Doneness::Overcooked => by_skill.min(1),
            Doneness::Undercooked | Doneness::Burnt => 0,
        }
    }

    /// The raw nutrition profile whose risk an unsafe `dish` keeps: its own, else its class's.
    pub fn raw_profile_of<'a>(&'a self, dish: &'a Dish) -> Option<&'a str> {
        dish.raw_profile.as_deref().or_else(|| self.class(&dish.class).and_then(|c| c.raw_profile.as_deref()))
    }

    /// What an overcooked `dish` keeps again of its vitamins: its own factors, else its class's.
    pub fn keeps_of(&self, dish: &Dish) -> Option<Keeps> {
        dish.keeps.or_else(|| self.class(&dish.class).and_then(|c| c.keeps))
    }

    /// The skill a dish's meal grade reads: its recipe's, else its method's.
    pub fn skill_for<'a>(&'a self, dish: &Dish, recipe_skill: Option<&'a str>) -> Option<&'a str> {
        recipe_skill.or_else(|| self.method(&dish.method).and_then(|m| m.skill_required.as_deref()))
    }
}

/// The heat and time for a hand craft of `recipe_id` that starts now: the card's choice when the
/// level lets the cook choose and the card chose for this recipe, else the station's pick. The
/// choice is taken either way, so a stale one never carries to a later craft.
fn settings_for(data: &DataStore, cooking: &Cooking, dish: &Dish) -> Option<(Heat, f32)> {
    let chosen = data
        .get::<std::sync::Mutex<Option<Choice>>>(CHOICE_KEY)
        .and_then(|m| m.lock().ok().and_then(|mut c| c.take()))
        .filter(|c| c.recipe == dish.recipe && c.minutes.is_finite() && c.minutes >= 0.0);
    match chosen {
        Some(c) if Mode::from_store(data).chooses() => Some((c.heat, c.minutes)),
        _ => cooking.sensible(dish),
    }
}

/// The line a finished craft of `recipe_id` says, read against the cooking tables in `data`.
pub fn finished_line_for(data: &DataStore, recipe_id: &str, dish_name: &str, r: &CookResult) -> String {
    let safe = data.get::<Cooking>(DATA_KEY).and_then(|c| c.dish(recipe_id).and_then(|d| c.class(&d.class))).and_then(|k| k.safe.as_ref());
    finished_line(dish_name, r, safe)
}

/// When a hand craft of `recipe_id` by `crafter` starts: None when the recipe is not a dish (it
/// crafts as it always did), else how it will come out.
pub fn start(world: &hecs::World, data: &DataStore, recipe_id: &str, recipe_skill: Option<&str>, crafter: hecs::Entity) -> Option<CookResult> {
    let cooking = data.get::<Cooking>(DATA_KEY)?;
    let dish = cooking.dish(recipe_id)?;
    let (heat, minutes) = settings_for(data, cooking, dish)?;
    let out = cooking.outcome(dish, heat, minutes)?;
    let level = cooking
        .skill_for(dish, recipe_skill)
        .and_then(|s| world.get::<&crate::skills::PlayerSkills>(crafter).ok().map(|k| k.level(s)))
        .unwrap_or(0);
    Some(CookResult { heat, minutes, doneness: out.doneness, safe: out.safe, core_c: out.core_c, meal: cooking.meal_level(level, out.doneness) })
}

/// The line a finished dish says (words by key, data/i18n/en.json). `safe` is its class's safety
/// line, which says why an unsafe dish is unsafe (a temperature not reached, a boil not kept long
/// enough, raw dough or egg not cooked through).
pub fn finished_line(dish_name: &str, r: &CookResult, safe: Option<&Safe>) -> String {
    // The word source the crafting system reads in both builds (the server has no i18n module).
    let t = crate::crafting::home_store::words;
    let unsafe_key = match safe {
        Some(s) if s.core_c.is_none() && s.boil_min.is_some() => "cooking.finished_unsafe_boil",
        Some(s) if s.core_c.is_none() && s.done => "cooking.finished_unsafe_raw",
        _ => "cooking.finished_unsafe",
    };
    let boil = safe.and_then(|s| s.boil_min).map(|m| format!("{m:.0}")).unwrap_or_default();
    let key = match (r.doneness, r.safe) {
        (Doneness::Burnt, _) => "cooking.finished_burnt",
        (Doneness::Undercooked, false) | (Doneness::Done, false) | (Doneness::Overcooked, false) => unsafe_key,
        (Doneness::Undercooked, true) => "cooking.finished_undercooked",
        (Doneness::Overcooked, true) => "cooking.finished_overcooked",
        (Doneness::Done, true) => "cooking.finished_done",
    };
    let core = format!("{:.0}", r.core_c);
    t(key).replace("{{dish}}", dish_name).replace("{{core}}", &core).replace("{{n}}", &boil)
}

/// The quest event one unit of `item_id` cooked through reports, read by `Cook` objectives.
pub fn cooked_event_key(item_id: &str) -> String {
    format!("{COOKED_EVENT}{item_id}")
}

/// The prefix of the cooked events: a `Cook` step naming no item counts every one.
pub const COOKED_EVENT: &str = "cooked_";

/// A dish that came out cooked through (done or overcooked, and safe) reports one `cooked_`
/// event per unit of food it made.
pub fn credit(data: &DataStore, r: &CookResult, outputs: &[(String, u32)]) {
    if !r.cooked_through() {
        return;
    }
    for (item, qty) in outputs {
        if crate::food::consume_kinds().contains_key(item) {
            for _ in 0..*qty {
                crate::quests::push_quest_event(data, cooked_event_key(item));
            }
        }
    }
}

/// The craft grade of a stack of `item_id` whose grade byte is `quality`: food's byte is a dish's
/// state, never a craft grade, so it prices and names as ungraded (0).
pub fn craft_grade(item_id: &str, quality: u8) -> u8 {
    if crate::food::consume_kinds().contains_key(item_id) {
        0
    } else {
        quality
    }
}

#[cfg(test)]
#[path = "cooking_tests.rs"]
mod tests;
