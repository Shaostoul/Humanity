//! PRESERVING FOOD AS A PROCEDURE (preserving rung 1, 2026-11-21; docs/design/preserving.md).
//!
//! Spoilage already runs (food.rs: every food's clock, sped up or slowed by where it is kept).
//! Preserving answers it with procedures that have right and wrong ways, as the sources teach them:
//! DRYING takes water out on the thin-layer drying curve until the food is dry (fruit about 20%
//! water, vegetables about 10%, NCHFP), in an oven at 60 C or in the sun (fruit only, 29 C or warmer);
//! CANNING heats food sealed in jars against the tested tables of the USDA guide: acid food in a
//! boiling-water canner, low-acid food ONLY in a pressure canner, vented, at the tested pressure for
//! the tested time. A low-acid jar canned any other way carries a botulism risk, said on the jar.
//!
//! The data is food_system.ron's `preserving` (every figure with its source there), read once into a
//! [`Preserving`] under [`DATA_KEY`]. The crafting system asks [`start`] when a hand craft of a
//! procedure begins and carries the [`PreserveResult`] to its end; the food system reads a preserved
//! food's [`PreserveState`] off the eaten stack's grade byte. Nothing here runs per frame: one
//! closed-form evaluation per craft, and the preserving card's preview while it is drawn.

use std::path::Path;

use humanity_core::hot_reload::data_store::DataStore;
use serde::{Deserialize, Serialize};

/// DataStore: the [`Preserving`] tables (the engine's registries, at boot).
pub const DATA_KEY: &str = "preserving";
/// DataStore: the chosen level of Settings > Gameplay > Preserving ([`Mode`]), published each frame.
pub const MODE_KEY: &str = "preserving_mode";
/// DataStore: `Mutex<Option<Choice>>`, the method and time the preserving card chose for the next
/// craft of its recipe (the GUI's request bridge puts it beside the craft request).
pub const CHOICE_KEY: &str = "preserve_choice";
/// The quest event prefix of a food preserved safely: a `Preserve` step naming no item counts any.
pub const PRESERVED_EVENT: &str = "preserved_";

/// Settings > Gameplay > Preserving (data/gameplay/levels.json, 2026-11-21).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// The station picks the tested method and time; it always comes out safe.
    #[default]
    Simplified,
    /// The person chooses, and the card says before the start how it will come out.
    Guided,
    /// The person chooses; the card shows the tested table and judges nothing until it comes out.
    Realistic,
}

impl Mode {
    pub fn from_level(level: &str) -> Option<Self> {
        match level {
            "simplified" => Some(Self::Simplified),
            "guided" => Some(Self::Guided),
            "realistic" => Some(Self::Realistic),
            _ => None,
        }
    }

    pub fn from_store(data: &DataStore) -> Self {
        data.get::<Mode>(MODE_KEY).copied().unwrap_or_default()
    }

    /// Whether the person chooses the method and the time.
    pub fn chooses(self) -> bool {
        self != Self::Simplified
    }
}

/// A drying method (food_system.ron `preserving.drying_methods`): the oven or dehydrator, the sun.
#[derive(Debug, Clone, Deserialize)]
pub struct DryingMethod {
    pub id: String,
    /// The share of a procedure's oven drying rate this method dries at.
    pub rate_share: f32,
    /// The air must be at least this warm (the sun: 29.4 C, 85 F); None: the method makes its heat.
    #[serde(default)]
    pub min_air_c: Option<f32>,
    /// The drying classes it may dry (empty: every class).
    #[serde(default)]
    pub classes: Vec<String>,
}

/// A drying class: how dry its food must be (`water_done`, a share of the food's weight).
#[derive(Debug, Clone, Deserialize)]
pub struct DryingClass {
    pub id: String,
    pub water_done: f32,
}

/// One altitude band of a tested table: the value holds up to `up_to_ft` (None: above every other).
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
pub struct Band {
    #[serde(default)]
    pub up_to_ft: Option<f32>,
    pub value: f32,
}

/// The value of a table's band for `elevation_ft`, None for an empty table.
pub fn band_value(bands: &[Band], elevation_ft: f32) -> Option<f32> {
    bands.iter().find(|b| b.up_to_ft.map_or(true, |top| elevation_ft <= top)).map(|b| b.value)
}

/// One tested process for a food in one canner, as the USDA guide's table prints it.
#[derive(Debug, Clone, Deserialize)]
pub struct Tested {
    /// "boiling_water", "pressure_weighted" or "pressure_dial".
    pub canner: String,
    /// Minutes for pint jars (half-pints take the pint time), by altitude band.
    pub pints: Vec<Band>,
    /// Minutes for quart jars, by altitude band (shown on the card; rung 1 cans pints).
    #[serde(default)]
    pub quarts: Vec<Band>,
    /// A pressure canner's pounds per square inch, by altitude band; empty for boiling water.
    #[serde(default)]
    pub psi: Vec<Band>,
}

/// Drying or canning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum Kind {
    Dry,
    Can,
}

/// One row of food_system.ron `preserving.procedures`: a preserving recipe and how it works.
#[derive(Debug, Clone, Deserialize)]
pub struct Procedure {
    pub recipe: String,
    /// The preserved item it makes, which an eaten food's state is read against.
    pub makes: String,
    pub kind: Kind,
    /// The fresh food it starts from: a result that spoils keeps only as long as this does.
    pub fresh: String,
    /// The method the station picks in Simplified (a drying method, or a canner).
    pub pick: String,
    /// Drying: its class, the food's water share at the start, and the drying rate per hour in an
    /// oven at 60 C (a game choice inside the source's time range).
    #[serde(default)]
    pub class: Option<String>,
    #[serde(default)]
    pub water_start: f32,
    #[serde(default)]
    pub rate_per_h: f32,
    /// Canning: acid food (pH 4.6 or lower), the jar eating it hands back, the tested processes.
    #[serde(default)]
    pub acid: bool,
    #[serde(default)]
    pub jar: Option<String>,
    #[serde(default)]
    pub tested: Vec<Tested>,
}

/// The preserving tables, food_system.ron `preserving`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PreservingData {
    #[serde(default)]
    pub drying_methods: Vec<DryingMethod>,
    #[serde(default)]
    pub drying_classes: Vec<DryingClass>,
    #[serde(default)]
    pub procedures: Vec<Procedure>,
    /// (recipe, why) for the preserving-looking recipes that are not procedures yet.
    #[serde(default)]
    pub not_procedures: Vec<(String, String)>,
    /// The chance of Botulism from eating one jar with the risk (a game choice, said so in the data).
    #[serde(default)]
    pub botulism_risk: f32,
    /// The home's elevation, which picks a tested table's band (0: sea-level air aboard).
    #[serde(default)]
    pub elevation_ft: f32,
}

/// How a preserved food came out, carried on its stack's grade byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreserveState {
    /// Dry enough, or canned by the tested process: it keeps as its own profile says.
    Safe,
    /// Not dry, or an acid food under-processed: it keeps only as long as its fresh food.
    Spoils,
    /// A low-acid food not canned by the tested pressure process: a botulism risk.
    Botulism,
}

impl PreserveState {
    /// 0x41, 0x42, 0x43: beyond every byte a cooked dish writes (1..=56), so the two never read
    /// each other's. 0 is "not preserved by hand" (bought), which eats as its profile says.
    pub fn to_byte(self) -> u8 {
        match self {
            PreserveState::Safe => 0x41,
            PreserveState::Spoils => 0x42,
            PreserveState::Botulism => 0x43,
        }
    }

    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x41 => Some(PreserveState::Safe),
            0x42 => Some(PreserveState::Spoils),
            0x43 => Some(PreserveState::Botulism),
            _ => None,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            PreserveState::Safe => "preserving.state_safe",
            PreserveState::Spoils => "preserving.state_spoils",
            PreserveState::Botulism => "preserving.state_botulism",
        }
    }
}

/// Why it came out as it did: the finished line's words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reason {
    /// Dried to its dryness, or canned by the tested process.
    Done,
    /// Taken off before it was dry.
    NotDry,
    /// The sun on a day under 29 C (85 F).
    SunTooCool,
    /// The sun on food it must not dry (vegetables, meat).
    SunNotForThis,
    /// A canner with no tested process for this food (a low-acid food in boiling water).
    NoTestedProcess,
    /// Fewer minutes than the tested process.
    TooShort,
    /// A pressure canner not vented 10 minutes first.
    NotVented,
    /// A pressure canner below the band's pounds.
    PressureLow,
}

impl Reason {
    pub fn key(self) -> &'static str {
        match self {
            Reason::Done => "preserving.why_done",
            Reason::NotDry => "preserving.why_not_dry",
            Reason::SunTooCool => "preserving.why_sun_too_cool",
            Reason::SunNotForThis => "preserving.why_sun_not_for_this",
            Reason::NoTestedProcess => "preserving.why_no_tested_process",
            Reason::TooShort => "preserving.why_too_short",
            Reason::NotVented => "preserving.why_not_vented",
            Reason::PressureLow => "preserving.why_pressure_low",
        }
    }
}

/// What the person chose for one craft (or the station picked).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Choice {
    pub recipe: String,
    /// A drying method id or a canner id.
    pub method: String,
    /// Hours for drying, minutes for canning.
    pub time: f32,
    /// A pressure canner's weight, pounds per square inch (0 for every other method).
    #[serde(default)]
    pub psi: f32,
    /// A pressure canner vented 10 minutes before the weight went on.
    #[serde(default)]
    pub vented: bool,
}

/// The model's answer for one choice.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Outcome {
    pub state: PreserveState,
    pub reason: Reason,
    /// Drying: the food's water share at the end.
    pub water: Option<f32>,
    /// Drying: hours until it is dry by this method (None: never); canning: the tested minutes.
    pub ready: Option<f32>,
}

/// The water share after `hours` of drying at `rate_per_h` (Lewis's thin-layer model on the dry
/// basis, docs/design/preserving.md section 1).
pub fn water_after(water_start: f32, rate_per_h: f32, hours: f32) -> f32 {
    let w0 = water_start.clamp(0.0, 0.999);
    let x = w0 / (1.0 - w0) * (-rate_per_h.max(0.0) * hours.max(0.0)).exp();
    x / (1.0 + x)
}

/// Hours until the water share falls to `water_done`, None at no rate.
pub fn hours_to_dry(water_start: f32, rate_per_h: f32, water_done: f32) -> Option<f32> {
    if water_start <= water_done {
        return Some(0.0);
    }
    if rate_per_h <= 0.0 || water_done <= 0.0 {
        return None;
    }
    let x0 = water_start / (1.0 - water_start);
    let xd = water_done / (1.0 - water_done);
    Some((x0 / xd).ln() / rate_per_h)
}

/// The weight that comes out of drying `weight_in`: the solids stay, the water falls to `water_done`.
pub fn dried_weight(weight_in: f32, water_start: f32, water_done: f32) -> f32 {
    weight_in * (1.0 - water_start) / (1.0 - water_done)
}

/// The preserving tables, read from food_system.ron.
#[derive(Debug, Clone, Default)]
pub struct Preserving {
    pub data: PreservingData,
}

impl Preserving {
    pub fn from_food_data(data: &crate::food::FoodData) -> Self {
        Self { data: data.preserving.clone() }
    }

    /// Disk first, the copy built into the exe otherwise. With neither, no recipe is a procedure.
    pub fn load(data_dir: &Path) -> Self {
        match humanity_core::embedded_data::load_data_or_embedded(data_dir, "food_system.ron", humanity_core::data_parse::parse_ron::<crate::food::FoodData>) {
            Ok(d) => Self::from_food_data(&d),
            Err(e) => {
                log::warn!("{e}; no recipe preserves as a procedure this session");
                Self::default()
            }
        }
    }

    pub fn procedure(&self, recipe: &str) -> Option<&Procedure> {
        self.data.procedures.iter().find(|p| p.recipe == recipe)
    }

    pub fn procedure_for_item(&self, item: &str) -> Option<&Procedure> {
        self.data.procedures.iter().find(|p| p.makes == item)
    }

    pub fn method(&self, id: &str) -> Option<&DryingMethod> {
        self.data.drying_methods.iter().find(|m| m.id == id)
    }

    pub fn class(&self, id: &str) -> Option<&DryingClass> {
        self.data.drying_classes.iter().find(|c| c.id == id)
    }

    /// The fresh food whose clock a stack of `item` with grade byte `quality` keeps: its procedure's
    /// fresh food when it came out spoiling, None otherwise (it keeps by its own profile).
    pub fn keeps_as(&self, item: &str, quality: u8) -> Option<&str> {
        let p = self.procedure_for_item(item)?;
        (PreserveState::from_byte(quality)? == PreserveState::Spoils).then_some(p.fresh.as_str())
    }

    /// Whether a stack of `item` with grade byte `quality` carries a botulism risk.
    pub fn botulism(&self, item: &str, quality: u8) -> bool {
        self.procedure_for_item(item).is_some() && PreserveState::from_byte(quality) == Some(PreserveState::Botulism)
    }

    /// The empty jar eating a jar of `item` hands back.
    pub fn empty_jar_of(&self, item: &str) -> Option<&str> {
        self.procedure_for_item(item).and_then(|p| p.jar.as_deref())
    }

    /// How drying `p` by `method` for `hours` comes out, the air around it at `air_c`.
    pub fn dry_outcome(&self, p: &Procedure, method: &str, hours: f32, air_c: f32) -> Option<Outcome> {
        let class = self.class(p.class.as_deref()?)?;
        let m = self.method(method)?;
        let allowed = m.classes.is_empty() || m.classes.iter().any(|c| Some(c.as_str()) == p.class.as_deref());
        let warm = m.min_air_c.map_or(true, |lo| air_c >= lo);
        // The sun dries nothing it must not, and nothing on a cool day: no drying credit at all.
        let rate = if allowed && warm { p.rate_per_h * m.rate_share } else { 0.0 };
        let water = water_after(p.water_start, rate, hours);
        let ready = hours_to_dry(p.water_start, rate, class.water_done);
        let (state, reason) = if !allowed {
            (PreserveState::Spoils, Reason::SunNotForThis)
        } else if !warm {
            (PreserveState::Spoils, Reason::SunTooCool)
        } else if ready.is_some_and(|r| hours >= r) {
            (PreserveState::Safe, Reason::Done)
        } else {
            (PreserveState::Spoils, Reason::NotDry)
        };
        Some(Outcome { state, reason, water: Some(water), ready })
    }

    /// How canning `p` in `canner` for `minutes` comes out (pints; `psi` and `vented` for a pressure
    /// canner), at the home's elevation.
    pub fn can_outcome(&self, p: &Procedure, canner: &str, minutes: f32, psi: f32, vented: bool) -> Outcome {
        let elev = self.data.elevation_ft;
        let fail = |reason| Outcome {
            state: if p.acid { PreserveState::Spoils } else { PreserveState::Botulism },
            reason,
            water: None,
            ready: None,
        };
        let Some(t) = p.tested.iter().find(|t| t.canner == canner) else {
            return fail(Reason::NoTestedProcess);
        };
        let Some(need) = band_value(&t.pints, elev) else {
            return fail(Reason::NoTestedProcess);
        };
        let pressure = canner.starts_with("pressure");
        let mut out = if pressure && !vented {
            fail(Reason::NotVented)
        } else if pressure && band_value(&t.psi, elev).is_some_and(|need_psi| psi < need_psi) {
            fail(Reason::PressureLow)
        } else if minutes < need {
            fail(Reason::TooShort)
        } else {
            Outcome { state: PreserveState::Safe, reason: Reason::Done, water: None, ready: None }
        };
        out.ready = Some(need);
        out
    }

    /// How `p` comes out by `choice`, the air around the person at `air_c` (the sun's test).
    pub fn outcome(&self, p: &Procedure, choice: &Choice, air_c: f32) -> Option<Outcome> {
        match p.kind {
            Kind::Dry => self.dry_outcome(p, &choice.method, choice.time, air_c),
            Kind::Can => Some(self.can_outcome(p, &choice.method, choice.time, choice.psi, choice.vented)),
        }
    }

    /// What the station picks in Simplified: the procedure's own method, and for drying whole hours
    /// one past dry, for canning the tested minutes and pounds at the home's elevation, vented. None
    /// when its data cannot come out safe (a data error the tests refuse).
    pub fn sensible(&self, p: &Procedure) -> Option<Choice> {
        let choice = match p.kind {
            Kind::Dry => {
                let class = self.class(p.class.as_deref()?)?;
                let m = self.method(&p.pick)?;
                let hours = hours_to_dry(p.water_start, p.rate_per_h * m.rate_share, class.water_done)?;
                Choice { recipe: p.recipe.clone(), method: p.pick.clone(), time: hours.ceil() + 1.0, psi: 0.0, vented: false }
            }
            Kind::Can => {
                let t = p.tested.iter().find(|t| t.canner == p.pick)?;
                let elev = self.data.elevation_ft;
                Choice {
                    recipe: p.recipe.clone(),
                    method: p.pick.clone(),
                    time: band_value(&t.pints, elev)?,
                    psi: band_value(&t.psi, elev).unwrap_or(0.0),
                    vented: p.pick.starts_with("pressure"),
                }
            }
        };
        Some(choice)
    }
}

/// What a hand craft of a procedure came to, decided when it starts and carried with the craft,
/// through a save, to its end.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreserveResult {
    pub choice: Choice,
    pub state: PreserveState,
    pub reason: Reason,
    /// Drying: the water share at the end.
    #[serde(default)]
    pub water: Option<f32>,
}

impl PreserveResult {
    pub fn byte(&self) -> u8 {
        self.state.to_byte()
    }
}

/// The method and time for a hand craft of `p` that starts now: the card's choice when the level
/// lets the person choose and the card chose for this recipe, else the station's pick. The choice is
/// taken either way, so a stale one never carries to a later craft.
fn choice_for(data: &DataStore, pr: &Preserving, p: &Procedure) -> Option<Choice> {
    let chosen = data
        .get::<std::sync::Mutex<Option<Choice>>>(CHOICE_KEY)
        .and_then(|m| m.lock().ok().and_then(|mut c| c.take()))
        .filter(|c| c.recipe == p.recipe && c.time.is_finite() && c.time >= 0.0);
    match chosen {
        Some(c) if Mode::from_store(data).chooses() => Some(c),
        _ => pr.sensible(p),
    }
}

/// The air temperature around the person (the sun's test), the engine's environment context.
pub fn air_c(data: &DataStore) -> f32 {
    data.get::<humanity_core::ecs::components::EnvironmentContext>("environment_context")
        .map(|e| e.ambient_temp_c)
        .unwrap_or_else(|| humanity_core::ecs::components::EnvironmentContext::default().ambient_temp_c)
}

/// When a hand craft of `recipe_id` starts: None when the recipe is not a procedure (it crafts as it
/// always did), else how it will come out.
pub fn start(data: &DataStore, recipe_id: &str) -> Option<PreserveResult> {
    let pr = data.get::<Preserving>(DATA_KEY)?;
    let p = pr.procedure(recipe_id)?;
    let choice = choice_for(data, pr, p)?;
    let out = pr.outcome(p, &choice, air_c(data))?;
    Some(PreserveResult { choice, state: out.state, reason: out.reason, water: out.water })
}

/// The line a finished procedure says (words by key, data/i18n/en.json).
pub fn finished_line(name: &str, r: &PreserveResult) -> String {
    let t = crate::crafting::home_store::words;
    let key = match r.state {
        PreserveState::Safe => "preserving.finished_safe",
        PreserveState::Spoils => "preserving.finished_spoils",
        PreserveState::Botulism => "preserving.finished_botulism",
    };
    let water = r.water.map(|w| format!("{:.0}", w * 100.0)).unwrap_or_default();
    t(key).replace("{{name}}", name).replace("{{why}}", t(r.reason.key())).replace("{{water}}", &water)
}

/// The quest event one unit of `item_id` preserved safely reports, read by `Preserve` objectives.
pub fn preserved_event_key(item_id: &str) -> String {
    format!("{PRESERVED_EVENT}{item_id}")
}

/// A procedure that came out safe reports one `preserved_` event per unit of food it made.
pub fn credit(data: &DataStore, r: &PreserveResult, outputs: &[(String, u32)]) {
    if r.state != PreserveState::Safe {
        return;
    }
    for (item, qty) in outputs {
        if crate::food::consume_kinds().contains_key(item) {
            for _ in 0..*qty {
                crate::quests::push_quest_event(data, preserved_event_key(item));
            }
        }
    }
}

#[cfg(test)]
#[path = "preserving_tests.rs"]
mod tests;
