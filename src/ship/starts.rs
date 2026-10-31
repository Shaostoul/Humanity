//! The STARTS: the kinds of home a new player can start in, as data (ship homes increment 6,
//! docs/design/ship-homes-increment-6-plan.md section 3.1).
//!
//! data/homes/starts.ron holds one row per start (Cabin, Apartment, Homestead): the home design it
//! builds, the plot kind it lives on, its households (the old Settings choice of Family or Solo,
//! now per start), what each of food, water, energy and fertilizer asks of the player by default,
//! the ship's shared places it relies on, what a new character carries, the home's storage at the
//! start, the opening quest, whether the home has its own air, and which of the homestead's world
//! content it carries. The relay reads it to give a player a plot of the kind they asked for, the
//! game to build that home and load its household, and both agree because both read this file
//! (disk first, else the copy built into the exe, `embedded_data::STARTS_RON`).
//!
//! The `game_home` wire shapes live here too, so the relay, the game and the web test share them:
//! `game_home_request` asks where you live on this server and what is free, `game_choose_home`
//! chooses a start while you hold no home there, and both are answered with `game_home`
//! (`HomeAnswer`), to the asker alone.
//!
//! Compiled into the relay as well as the game: nothing here may use a native-only crate.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// The starts file, under data/.
pub const STARTS_FILE: &str = "homes/starts.ron";

/// How a domain (food, water, energy, fertilizer) is looked after by default
/// (docs/design/engagement-modes.md): by your own hands, by trade, by machines you set up, with
/// others, or in the background by the ship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Engagement {
    Direct,
    Trade,
    Automation,
    Cooperative,
    Background,
}

/// One start's default engagement per domain. Shown in words; nothing else reads it yet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngagementDefaults {
    pub food: Engagement,
    pub water: Engagement,
    pub energy: Engagement,
    pub fertilizer: Engagement,
}

/// Who lives in a start's home: an id a save records ("family", "solo"), the words the choice
/// shows, and the machine file under data/machines/ it loads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Household {
    pub id: String,
    pub name: String,
    pub machines: String,
}

/// The homestead's world content a start's home carries (src/engine/world_load.rs spawns each
/// only for a home that lists it): the herd, the wild spawns, the decoration plants, the
/// showcase garden's first planting, the hologram.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HomeContent {
    Herd,
    WildSpawns,
    Decorations,
    ShowcaseGarden,
    Hologram,
}

/// One start: a row of data/homes/starts.ron (its header says what each field means).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Start {
    pub id: String,
    pub name: String,
    pub sentence: String,
    pub offered: bool,
    /// Why a start is not offered yet, shown on its greyed row. Empty for an offered start.
    #[serde(default)]
    pub not_offered_why: String,
    /// data/homes/shipped/<design>.ron; its `kind` must equal `plot_kind`.
    pub design: String,
    pub plot_kind: String,
    /// The first is the default.
    pub households: Vec<Household>,
    pub engagement: EngagementDefaults,
    /// zone_types.ron ids this start relies on aboard.
    pub shared: Vec<String>,
    /// What a new character carries: (items.csv id, how many).
    pub kit: Vec<(String, u32)>,
    /// The home's storage at the start, a places file under data/ ("" for none).
    pub storage: String,
    /// The quest a new character is given, or None.
    pub opening: Option<String>,
    /// False: the home is on the ship's ventilation and breathes the ship's air.
    pub own_air: bool,
    pub home_content: Vec<HomeContent>,
}

/// The whole starts file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Starts {
    /// The start a player who asks for nothing gets; offered.
    pub default: String,
    pub starts: Vec<Start>,
}

/// True for a start id a relay accepts in `game_join`: 1 to 32 characters of a-z, 0-9, - and _.
pub fn is_start_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 32 && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

/// Every start's opening quest, as (start id, quest id), read once per run. The Quests page's
/// Available list asks every frame which quests are some start's opening, so a player is never
/// offered another start's (2026-10-12: a homestead player was offered the Cabin's "Settling
/// In", and a Cabin player the homestead's "First Steps").
pub fn opening_quests(data_dir: &Path) -> &'static [(String, String)] {
    static OPENINGS: std::sync::OnceLock<Vec<(String, String)>> = std::sync::OnceLock::new();
    OPENINGS.get_or_init(|| {
        Starts::load(data_dir).starts.iter().filter_map(|s| s.opening.clone().map(|q| (s.id.clone(), q))).collect()
    })
}

impl Starts {
    /// Parse a starts file and check it (`validate`): what `load` builds the disk copy and the
    /// built-in copy with, so a file that parses but breaks a rule is set aside like one that
    /// does not parse.
    pub fn parse(bytes: &[u8]) -> Result<Starts, String> {
        let starts: Starts = crate::data_parse::parse_ron(bytes)?;
        starts.validate()?;
        Ok(starts)
    }

    /// data/homes/starts.ron from `data_dir`: the disk copy when this version can read it and it
    /// keeps the rules, else the copy built into the exe, which `load_data_or_embedded` logs
    /// (BUG-133). Both failing is a broken build (a test holds the built-in copy to the rules),
    /// and nothing in the game or the relay can place a player without starts, so that stops here
    /// with the reason rather than going on with none.
    pub fn load(data_dir: &Path) -> Starts {
        crate::embedded_data::load_data_or_embedded(data_dir, STARTS_FILE, Self::parse)
            .unwrap_or_else(|e| panic!("{e}: this build has no starts to give anyone a home"))
    }

    /// The start with this id, offered or not.
    pub fn get(&self, id: &str) -> Option<&Start> {
        self.starts.iter().find(|s| s.id == id)
    }

    /// The default start (`default`, offered once validated).
    pub fn default_start(&self) -> &Start {
        self.get(&self.default)
            .or_else(|| self.starts.iter().find(|s| s.offered))
            .or_else(|| self.starts.first())
            .expect("a starts file lists at least one start (validate)")
    }

    /// The start a player who asked for `asked` gets: that start when it exists and is offered,
    /// else the default (an unknown id, a start not offered yet, or no ask at all).
    pub fn resolve(&self, asked: Option<&str>) -> &Start {
        asked.and_then(|a| self.get(a)).filter(|s| s.offered).unwrap_or_else(|| self.default_start())
    }

    /// The first OFFERED start whose plots are of `kind`: what a plot of that kind holds.
    pub fn for_plot_kind(&self, kind: &str) -> Option<&Start> {
        self.starts.iter().find(|s| s.offered && s.plot_kind == kind)
    }

    /// The household `household` of the start `start`: the start's first household when it has
    /// no such household, and the default start's when there is no such start.
    pub fn household(&self, start: &str, household: &str) -> &Household {
        let s = self.get(start).unwrap_or_else(|| self.default_start());
        s.households
            .iter()
            .find(|h| h.id == household)
            .or_else(|| s.households.first())
            .or_else(|| self.default_start().households.first())
            .expect("every start has a household (validate)")
    }

    /// The starts a player may choose, in file order.
    pub fn offered(&self) -> impl Iterator<Item = &Start> {
        self.starts.iter().filter(|s| s.offered)
    }

    /// The rules every starts file keeps, whatever it holds: at least one start; every id a
    /// start id and unique; the default names an offered start; every start has households, each
    /// with a unique id and a plain machine file name; an offered start names its design and plot
    /// kind; a start not offered says why; no kit line carries nothing. (Whether the design is
    /// built in, the machine files load, the kit's items exist and the opening is a quest are the
    /// tests' to check against the data, below.)
    pub fn validate(&self) -> Result<(), String> {
        if self.starts.is_empty() {
            return Err("the starts file lists no start".into());
        }
        let mut seen = std::collections::HashSet::new();
        for s in &self.starts {
            if !is_start_id(&s.id) {
                return Err(format!("'{}' is not a start id (1 to 32 of a-z, 0-9, - and _)", s.id));
            }
            if !seen.insert(s.id.as_str()) {
                return Err(format!("the start '{}' is listed twice", s.id));
            }
            if s.households.is_empty() {
                return Err(format!("the start '{}' has no household", s.id));
            }
            let mut households = std::collections::HashSet::new();
            for h in &s.households {
                if h.id.is_empty() || !households.insert(h.id.as_str()) {
                    return Err(format!("the start '{}' has an empty or repeated household id '{}'", s.id, h.id));
                }
                let plain = !h.machines.is_empty() && h.machines.ends_with(".ron") && !h.machines.contains(['/', '\\']) && !h.machines.contains("..");
                if !plain {
                    return Err(format!("the start '{}' household '{}' names '{}', not a file in data/machines/", s.id, h.id, h.machines));
                }
            }
            if s.offered && (s.design.is_empty() || s.plot_kind.is_empty()) {
                return Err(format!("the start '{}' is offered but names no design or plot kind", s.id));
            }
            if !s.offered && s.not_offered_why.trim().is_empty() {
                return Err(format!("the start '{}' is not offered and does not say why", s.id));
            }
            if let Some((item, _)) = s.kit.iter().find(|(_, n)| *n == 0) {
                return Err(format!("the start '{}' carries none of {item}", s.id));
            }
        }
        match self.get(&self.default) {
            None => Err(format!("the default start '{}' is not a start", self.default)),
            Some(d) if !d.offered => Err(format!("the default start '{}' is not offered", self.default)),
            Some(_) => Ok(()),
        }
    }
}

// ── The game_home wire (3.1 "Wire shapes") ──────────────────────────────────────────────────

/// `{type}`: where do I live on this server, and what is free?
pub const HOME_REQUEST: &str = "game_home_request";
/// `{type, start}`: give me a home of this start (only while I hold none here).
pub const CHOOSE_HOME: &str = "game_choose_home";
/// The answer to either, to the asker alone: `HomeAnswer` plus `type`.
pub const HOME_ANSWER: &str = "game_home";

/// The home the asker holds on this server: the plot id, its kind, and the start that kind is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeldHome {
    pub id: String,
    pub kind: String,
    pub start: String,
}

/// One start as the answer offers it: its words (from the server's own starts file, so a client
/// never shows a copy of its own), whether it may be chosen, why not, and how many plots of its
/// kind nobody holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StartOffer {
    pub id: String,
    pub name: String,
    pub sentence: String,
    pub offered: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub why_not: String,
    pub free: u32,
}

impl StartOffer {
    /// A start as offered, with `free` plots of its kind free.
    pub fn of(start: &Start, free: u32) -> StartOffer {
        StartOffer {
            id: start.id.clone(),
            name: start.name.clone(),
            sentence: start.sentence.clone(),
            offered: start.offered,
            why_not: if start.offered { String::new() } else { start.not_offered_why.clone() },
            free,
        }
    }
}

/// Why a choice was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HomeRefusal {
    /// You already hold a home on this ship (moving house comes later).
    HoldsOne,
    /// No plot of that kind is free.
    Full,
    /// That start is unknown or not offered.
    NotOffered,
    /// The server's ship did not load.
    NoShip,
    /// Asked again too soon.
    Again,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// The `game_home` answer: the asker's own home here (None: none), the starts with their free
/// counts, and for a choice, whether it was made, or why not in a sentence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HomeAnswer {
    pub plot: Option<HeldHome>,
    pub starts: Vec<StartOffer>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub chose: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refused: Option<HomeRefusal>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub message: String,
}

impl HomeAnswer {
    /// The message as sent: this answer's fields with `"type": "game_home"`.
    pub fn to_message(&self) -> serde_json::Value {
        let mut v = serde_json::to_value(self).unwrap_or_else(|_| serde_json::json!({}));
        if let Some(o) = v.as_object_mut() {
            o.insert("type".into(), serde_json::Value::String(HOME_ANSWER.into()));
        }
        v
    }

    /// Read a received `game_home` message (None for any other message or a malformed one).
    pub fn from_message(v: &serde_json::Value) -> Option<HomeAnswer> {
        if v.get("type").and_then(|t| t.as_str()) != Some(HOME_ANSWER) {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }
}

/// "a cabin", "an apartment": a start or kind name as a person says it mid-sentence.
fn a_or_an(name: &str) -> String {
    let word = name.replace(['_', '-'], " ").to_lowercase();
    let article = if word.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" };
    format!("{article} {word}")
}

/// "a homestead", "a homestead or a farm", "a homestead, a farm or a house".
fn either(names: &[&str]) -> String {
    let named: Vec<String> = names.iter().map(|n| a_or_an(n)).collect();
    match named.split_last() {
        None => String::new(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
    }
}

/// The sentence a refusal shows (`HomeAnswer::message`). `asked` is the start asked for, `held`
/// the asker's home here when they hold one, `free_names` the names of the offered starts that
/// still have a plot free.
pub fn refusal_words(r: HomeRefusal, asked: &Start, held: Option<&HeldHome>, free_names: &[&str]) -> String {
    const LATER: &str = "Moving to another kind of home comes in a later update.";
    match r {
        HomeRefusal::HoldsOne => match held {
            Some(h) => format!("You already have a home on this ship: {}, {}. {LATER}", a_or_an(&h.kind), h.id),
            None => format!("You already have a home on this ship. {LATER}"),
        },
        HomeRefusal::Full => {
            let none = format!("No {} is free on this ship right now.", asked.name.to_lowercase());
            if free_names.is_empty() {
                none
            } else {
                format!("{none} You can have {} instead.", either(free_names))
            }
        }
        HomeRefusal::NotOffered if !asked.not_offered_why.trim().is_empty() => asked.not_offered_why.clone(),
        HomeRefusal::NotOffered => format!("{} homes are not offered on this ship.", asked.name),
        HomeRefusal::NoShip => "This server's ship did not load, so there is no home to choose.".into(),
        HomeRefusal::Again => "The server is busy; try again in a moment.".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// EACH START NAMES ITS OWN OPENING, READ ONCE (the Quests page leaves out every other
    /// start's, lib.rs's Available list). Seen red 2026-10-12 with `opening_quests` returning an
    /// empty list: "the homestead's opening is listed" failed.
    #[test]
    fn each_start_names_its_own_opening() {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let openings = opening_quests(&data);
        let of = |start: &str| openings.iter().find(|(s, _)| s == start).map(|(_, q)| q.as_str());
        assert_eq!(of("homestead"), Some("gs_first_steps"), "the homestead's opening is listed");
        assert_eq!(of("cabin"), Some("gs_first_steps_cabin"), "and the cabin's");
        assert!(std::ptr::eq(openings, opening_quests(&data)), "read once");
    }
    use std::path::PathBuf;

    fn data_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
    }

    /// The repo's starts file as the game reads it from disk, held to the rules.
    fn the_file() -> Starts {
        Starts::parse(&std::fs::read(data_dir().join(STARTS_FILE)).expect("data/homes/starts.ron is in the repo"))
            .unwrap_or_else(|e| panic!("data/homes/starts.ron: {e}"))
    }

    /// The starts are data, every offered one can be built and lived in, and none is called a
    /// tier: three rows with unique ids, the default offered, every offered row's design built
    /// into the exe with the kind of its plots, every household file loading with every machine
    /// it places defined and every one of its own standing inside the design, every kit item a
    /// real item, every opening a real quest, every storage seed a places file with the backpack
    /// in it, and no word a player reads saying "tier", "rank" or "level" (a start is how much
    /// space you hold, never a step on a ladder: engagement-modes.md).
    ///
    /// Seen red 2026-10-12 with the apartment flipped to `offered: true`: "the apartment start is
    /// offered but no apartment design is built in" failed.
    #[test]
    fn the_starts_are_data_and_none_is_called_a_tier() {
        let starts = the_file();
        assert_eq!(starts.starts.len(), 3, "Cabin, Apartment and Homestead");
        let ids: std::collections::BTreeSet<&str> = starts.starts.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids.len(), 3, "the ids are unique: {ids:?}");
        assert!(starts.default_start().offered, "the default start is offered");
        assert_eq!(starts.default_start().id, starts.default);

        let items = crate::systems::inventory::ItemRegistry::from_csv(&std::fs::read(data_dir().join("items.csv")).unwrap())
            .expect("items.csv parses");
        let quests = crate::systems::quests::QuestRegistry::from_ron_dir(&data_dir().join("quests"));
        assert!(quests.get("gs_first_steps").is_some(), "the quests parsed");
        for s in starts.offered() {
            let design = crate::ship::ship_structure::HomeDesign::built_in(&s.design)
                .unwrap_or_else(|| panic!("the {} start is offered but no {} design is built in", s.id, s.design));
            assert_eq!(design.kind, s.plot_kind, "the {} start's design is for {} plots, not {}", s.id, design.kind, s.plot_kind);
            let (w, d) = (design.body.width, design.body.depth);
            for h in &s.households {
                let path = data_dir().join("machines").join(&h.machines);
                let home = crate::machines::MachineHome::load(&path)
                    .unwrap_or_else(|| panic!("the {} start's {} household file {} does not load", s.id, h.id, path.display()));
                for i in &home.instances {
                    assert!(home.catalog.contains_key(&i.machine), "{}: {} is a {} the catalog does not define", h.machines, i.id, i.machine);
                    if i.zone == "home" {
                        let (x, z) = (i.offset.0, i.offset.2);
                        assert!(
                            (0.0..=w).contains(&x) && (0.0..=d).contains(&z),
                            "{}: {} at ({x}, {z}) stands outside the {} design's {w} x {d} m",
                            h.machines,
                            i.id,
                            s.design
                        );
                    }
                }
                for a in &home.arrays {
                    assert!(home.catalog.contains_key(&a.machine), "{}: an array of {} the catalog does not define", h.machines, a.machine);
                }
            }
            for (item, n) in &s.kit {
                assert!(items.items.contains_key(item), "the {} start's kit carries {item}, which is not in items.csv", s.id);
                assert!(*n > 0);
            }
            if let Some(q) = &s.opening {
                assert!(quests.get(q).is_some(), "the {} start opens with {q}, which is not a quest", s.id);
            }
            if !s.storage.is_empty() {
                let text = std::fs::read_to_string(data_dir().join(&s.storage))
                    .unwrap_or_else(|e| panic!("the {} start's storage {}: {e}", s.id, s.storage));
                let seed: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", s.storage));
                let entities = seed["entities"].as_array().unwrap_or_else(|| panic!("{} lists no entities", s.storage));
                let backpack = entities.iter().flat_map(|e| e["children"].as_array().into_iter().flatten()).any(|c| c["kind"] == "backpack");
                assert!(backpack, "{} has no backpack node for the live backpack", s.storage);
            }
        }
        for s in &starts.starts {
            let mut words = vec![s.name.clone(), s.sentence.clone(), s.not_offered_why.clone()];
            words.extend(s.households.iter().map(|h| h.name.clone()));
            for text in &words {
                for word in text.to_lowercase().split(|c: char| !c.is_ascii_alphabetic()) {
                    assert!(!["tier", "tiers", "rank", "ranks", "level", "levels"].contains(&word), "the {} start says {word:?}: {text:?}", s.id);
                }
            }
        }
    }

    /// EACH OFFERED START HAS PLOTS AND ITS PLACES ABOARD (ship homes increment 6, lane 1A): on the
    /// shipped ship file, every start a player may choose has plots of its kind, and its design
    /// stands on each of them (`assemble`: the kind, the size, the door); and every shared place
    /// it relies on (`shared`, zone_types.ron ids) is a zone type and stands aboard, as a labelled
    /// volume of that type in one of the ship's zones (the mess hall, the cabin block's washroom,
    /// the recreation room). The Cabin's "the washroom is down the hall" is true only while
    /// these hold.
    ///
    /// Seen red 2026-10-12 with the sixteen cabin plots' kind spelt "cabins": "the cabin start
    /// lives on cabin plots, and the shipped ship has none".
    #[test]
    fn each_offered_start_has_plots_and_its_places_aboard() {
        use crate::ship::ship_structure::{HomeDesign, ShipStructure};
        let starts = the_file();
        let ship = ShipStructure::load_ship_file(&data_dir()).expect("the ship file loads");
        let types = crate::ship::structure::zone_types();
        for s in starts.offered() {
            let plots: Vec<&str> = ship.plots.iter().filter(|p| p.kind == s.plot_kind).map(|p| p.id.as_str()).collect();
            assert!(!plots.is_empty(), "the {} start lives on {} plots, and the shipped ship has none", s.id, s.plot_kind);
            let design = HomeDesign::built_in(&s.design).expect("the design is built in");
            for id in &plots {
                ship.clone().assemble(design.clone(), id).unwrap_or_else(|e| panic!("the {} start's home does not stand on {id}: {e}", s.id));
            }
            for place in &s.shared {
                assert!(types.iter().any(|t| &t.id == place), "the {} start relies on '{place}', which is not a zone type", s.id);
                let aboard = ship.zones.iter().flat_map(|z| z.body.zones.iter()).filter(|v| &v.type_id == place && !v.label.trim().is_empty()).count();
                assert!(aboard > 0, "the {} start relies on a {place} aboard, and the ship has none", s.id);
            }
        }
        // The Cabin's places, by name: the mess hall, a bathroom down the hall, the recreation room.
        let cabin = starts.get("cabin").expect("the cabin row");
        assert_eq!(cabin.shared, ["mess_hall", "room_bathroom", "recreation"]);
    }

    /// The copy built into the exe is the file in the repo, so a fresh install, a throwaway relay
    /// and a rig all agree on the starts with the data folder this exe was built from.
    ///
    /// Seen red 2026-10-12 with one sentence changed in the disk copy after the build: "the
    /// starts built into the exe are not data/homes/starts.ron" failed.
    #[test]
    fn the_built_in_starts_are_the_file() {
        let built_in = Starts::parse(crate::embedded_data::STARTS_RON.as_bytes()).expect("the built-in starts parse and keep the rules");
        assert_eq!(built_in, the_file(), "the starts built into the exe are not data/homes/starts.ron");
        // And a data folder without the file gets the built-in copy (logged).
        let empty = crate::test_temp::dir("starts_empty");
        assert_eq!(Starts::load(&empty), built_in);
    }

    /// Asking for a start nobody may choose gets the default, never that start: an unknown id, a
    /// start not offered yet, or no ask at all. A household the start lacks is its first, and a
    /// start that does not exist is the default's.
    ///
    /// Seen red 2026-10-12 with `resolve` returning any start it found: "an unoffered start
    /// resolves to the default" failed (it gave the apartment).
    #[test]
    fn an_unknown_or_unoffered_start_is_the_default() {
        let starts = the_file();
        let default = starts.default_start().id.clone();
        assert_eq!(starts.resolve(None).id, default);
        assert_eq!(starts.resolve(Some("palace")).id, default, "an unknown start resolves to the default");
        assert!(!starts.get("apartment").expect("the apartment row").offered);
        assert_eq!(starts.resolve(Some("apartment")).id, default, "an unoffered start resolves to the default");
        assert_eq!(starts.resolve(Some("cabin")).id, "cabin");
        assert_eq!(starts.for_plot_kind("cabin").map(|s| s.id.as_str()), Some("cabin"));
        assert_eq!(starts.for_plot_kind("apartment"), None, "no offered start lives on apartment plots yet");
        assert_eq!(starts.household("homestead", "solo").machines, "home_solo.ron");
        assert_eq!(starts.household("homestead", "nobody").machines, "home.ron", "an unknown household is the first");
        assert_eq!(starts.household("cabin", "family").machines, "cabin.ron", "a cabin has one household");
        assert_eq!(starts.household("palace", "solo").machines, starts.household(&default, "solo").machines);
        // The rules a file must keep.
        let mut broken = starts.clone();
        broken.default = "apartment".into();
        assert!(broken.validate().unwrap_err().contains("not offered"));
        let mut broken = starts.clone();
        broken.starts[0].households.clear();
        assert!(broken.validate().unwrap_err().contains("no household"));
        let mut broken = starts.clone();
        broken.starts.push(broken.starts[0].clone());
        assert!(broken.validate().unwrap_err().contains("twice"));
        assert!(is_start_id("cabin") && !is_start_id("Cabin") && !is_start_id("") && !is_start_id(&"x".repeat(33)));
    }

    /// The `game_home` answer reads on the wire exactly as section 3.1 writes it: `type`, the
    /// asker's plot, each start's words and free count, and `chose`, `refused` and `message`
    /// only when they say something. It reads back the same.
    ///
    /// Seen red 2026-10-12 with `free` renamed `free_count`: "assert_eq!(wire, json!({..}))" (the
    /// held home's answer against its JSON) failed, the answer carrying "free_count" for "free".
    #[test]
    fn the_wire_reads_as_written() {
        let starts = the_file();
        let cabin = starts.get("cabin").unwrap();
        let apartment = starts.get("apartment").unwrap();
        let held = HomeAnswer {
            plot: Some(HeldHome { id: "c2".into(), kind: "cabin".into(), start: "cabin".into() }),
            starts: vec![StartOffer::of(cabin, 14), StartOffer::of(apartment, 0)],
            chose: false,
            refused: None,
            message: String::new(),
        };
        let wire = held.to_message();
        assert_eq!(
            wire,
            serde_json::json!({
                "type": "game_home",
                "plot": {"id": "c2", "kind": "cabin", "start": "cabin"},
                "starts": [
                    {"id": "cabin", "name": cabin.name, "sentence": cabin.sentence, "offered": true, "free": 14},
                    {"id": "apartment", "name": apartment.name, "sentence": apartment.sentence, "offered": false,
                     "why_not": apartment.not_offered_why, "free": 0},
                ],
            })
        );
        for absent in ["chose", "refused", "message"] {
            assert!(wire.get(absent).is_none(), "{absent} is left out when it says nothing");
        }
        assert_eq!(HomeAnswer::from_message(&wire), Some(held));

        let mine = HeldHome { id: "p3".into(), kind: "homestead".into(), start: "homestead".into() };
        let refused = HomeAnswer {
            plot: Some(mine.clone()),
            starts: Vec::new(),
            chose: false,
            refused: Some(HomeRefusal::HoldsOne),
            message: refusal_words(HomeRefusal::HoldsOne, cabin, Some(&mine), &[]),
        };
        let wire = refused.to_message();
        assert_eq!(wire["refused"], "holds_one");
        assert_eq!(
            wire["message"],
            "You already have a home on this ship: a homestead, p3. Moving to another kind of home comes in a later update."
        );
        assert_eq!(HomeAnswer::from_message(&wire), Some(refused));
        let chose = HomeAnswer { plot: None, starts: Vec::new(), chose: true, refused: None, message: String::new() };
        assert_eq!(chose.to_message(), serde_json::json!({"type": "game_home", "plot": null, "starts": [], "chose": true}));
        assert_eq!(HomeAnswer::from_message(&serde_json::json!({"type": "game_welcome"})), None, "another message is not an answer");
        for (r, wire) in [
            (HomeRefusal::Full, "full"),
            (HomeRefusal::NotOffered, "not_offered"),
            (HomeRefusal::NoShip, "no_ship"),
            (HomeRefusal::Again, "again"),
        ] {
            assert_eq!(serde_json::to_value(r).unwrap(), wire);
        }
    }

    /// Each refusal in plain words: what was asked, what is free instead, and never a key.
    #[test]
    fn a_refusal_says_what_is_free_instead() {
        let starts = the_file();
        let (cabin, apartment) = (starts.get("cabin").unwrap(), starts.get("apartment").unwrap());
        assert_eq!(refusal_words(HomeRefusal::Full, cabin, None, &[]), "No cabin is free on this ship right now.");
        assert_eq!(
            refusal_words(HomeRefusal::Full, cabin, None, &["Homestead"]),
            "No cabin is free on this ship right now. You can have a homestead instead."
        );
        assert_eq!(
            refusal_words(HomeRefusal::Full, cabin, None, &["Homestead", "Apartment", "Farm"]),
            "No cabin is free on this ship right now. You can have a homestead, an apartment or a farm instead."
        );
        assert_eq!(refusal_words(HomeRefusal::NotOffered, apartment, None, &[]), apartment.not_offered_why);
        assert_eq!(refusal_words(HomeRefusal::NoShip, cabin, None, &[]), "This server's ship did not load, so there is no home to choose.");
        assert_eq!(refusal_words(HomeRefusal::Again, cabin, None, &[]), "The server is busy; try again in a moment.");
    }
}
