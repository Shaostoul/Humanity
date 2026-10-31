//! Home: your offline homestead (v0.379).
//!
//! Surfaces the existing Fibonacci homestead blueprint (data/blueprints/
//! fibonacci_homestead.ron) as a browsable Design. Pick a build SCALE (Solo /
//! Family / Community / Colony) and see, for that scale:
//!   - the total power + water DEMAND,
//!   - the aggregated BILL OF MATERIALS (the parts list, the real-world-bridge
//!     north star: what you would 3D-print / buy / trade to build it),
//!   - a SELF-SUFFICIENCY summary (the generation / collection / recycling systems
//!     that close the loops),
//!   - the rooms grouped by construction tier.
//!
//! THE LAYOUT (operator, 2026-10-30: "everything is left aligned. There's a ton of unused space
//! ... everything from the top where it says 'Household' down to 'just you: rooms F1 to F8' is
//! anchored to the screen, then everything below is a scrollable area. We should join the two
//! areas and make everything more visually friendly"): the whole page is ONE scroll, and its parts
//! are cards in rows that fill the width (`widgets::cards::Grid`): the Household card beside
//! "Where you live", the animals (cards or a table, herd.rs), the home's readouts side by side, and
//! the loops as cards with their long explanations folded (home_loops.rs).
//!
//! Offline-first (operator 2026-06-07: "keep developing offline; multiplayer
//! after offline singleplayer works"). Read-only for now (no editing / building
//! yet). Server + Real homes, and exact closure SCORING (output vs demand per
//! loop), are the next data layers. See docs/design/homes-as-profiles.md.

use egui::{Frame, RichText, ScrollArea};
use crate::gui::{GuiState, HomesteadDesign, DesignRoom, TowerConfig, TowerCompat};
use crate::i18n::{t, tf};
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, cards};
use std::cell::RefCell;

/// Build scales, from the blueprint's scaling_notes. Each includes every room up
/// to a Fibonacci-index ceiling (Solo 1-8, Family 1-13, Community 1-55, Colony all).
#[derive(Clone, Copy, PartialEq)]
enum Scale {
    Solo,
    Family,
    Community,
    Colony,
}

impl Scale {
    const ALL: [Scale; 4] = [Scale::Solo, Scale::Family, Scale::Community, Scale::Colony];

    fn ceiling(self) -> u32 {
        match self {
            Scale::Solo => 8,
            Scale::Family => 13,
            Scale::Community => 55,
            Scale::Colony => u32::MAX,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Scale::Solo => t("home.scale_solo"),
            Scale::Family => t("home.scale_family"),
            Scale::Community => t("home.scale_community"),
            Scale::Colony => t("home.scale_colony"),
        }
    }
    fn blurb(self) -> &'static str {
        match self {
            Scale::Solo => t("home.scale_solo_blurb"),
            Scale::Family => t("home.scale_family_blurb"),
            Scale::Community => t("home.scale_community_blurb"),
            Scale::Colony => t("home.scale_colony_blurb"),
        }
    }
    fn hint_key(self) -> &'static str {
        match self {
            Scale::Solo => "home.scale_solo_hint",
            Scale::Family => "home.scale_family_hint",
            Scale::Community => "home.scale_community_hint",
            Scale::Colony => "home.scale_colony_hint",
        }
    }
}

fn with_scale<R>(f: impl FnOnce(&mut Scale) -> R) -> R {
    thread_local! {
        static SCALE: RefCell<Scale> = const { RefCell::new(Scale::Solo) };
    }
    SCALE.with(|s| f(&mut s.borrow_mut()))
}

/// "steel_ingot_0" -> "Steel Ingot" for display (the data ids are game item ids).
fn humanize(id: &str) -> String {
    let base = id.strip_suffix("_0").unwrap_or(id);
    base.split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// One civilizational gap the homestead cannot close alone (electronics, ore-scale
/// metal, medicine synthesis, equipment renewal, raw chemistry inputs). Data-driven
/// from data/self_sufficiency/cannot_close.ron (infinite-of-X: the list IS the data),
/// distilled from docs/design/homestead-solo-design.md section 8.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CannotCloseEntry {
    pub id: String,
    pub title: String,
    /// Plain-language story: what the game recipe abstracts away and why one
    /// person cannot close this loop.
    pub body: String,
    /// Where it really comes from ("Traded from ...").
    pub provided_by: String,
}

/// The whole cannot-close data file: intro line + the gap categories + a closing
/// framing line (the non-defeatist "this gap IS civilization" lesson).
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct CannotCloseData {
    #[serde(default)]
    pub intro: String,
    #[serde(default)]
    pub entries: Vec<CannotCloseEntry>,
    #[serde(default)]
    pub footer: String,
}

/// Pure loader (unit-tested below): parse data/self_sufficiency/cannot_close.ron.
/// Missing or malformed file yields empty data (the panel hides; the page still works).
pub fn load_cannot_close(data_dir: &std::path::Path) -> CannotCloseData {
    let path = data_dir.join("self_sufficiency").join("cannot_close.ron");
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return CannotCloseData::default(),
    };
    match ron::from_str::<CannotCloseData>(&text) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("load_cannot_close: failed to parse {}: {e}", path.display());
            CannotCloseData::default()
        }
    }
}

/// Load-once cache. Same pattern as laws/glossary: `crate::data_dir()` is the
/// CWD-independent resolved data dir in the installed app, and falls back to
/// "data" under tests/snapshots (which run from the repo root).
fn cannot_close() -> &'static CannotCloseData {
    static CACHE: std::sync::OnceLock<CannotCloseData> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| load_cannot_close(&crate::data_dir()))
}

/// One required component of the ideal closed-loop homestead (e.g. 4x
/// solar_panel). `game_id` is a REAL machine/structure/item id in the game
/// data (enforced by `outline_game_ids_are_real` below) so the outline stays
/// an honest requirements list, never wishful copy.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct OutlineRequirement {
    pub item: String,
    pub qty: String,
    /// Empty string = the game has no id for this yet; renders as a
    /// "not in game yet" flag, making the luxury tier double as the
    /// game-content gap list (the operator's stated purpose for the page).
    pub game_id: String,
    pub why: String,
    /// "baseline" (bare minimum) or "luxury" (life of luxury tier,
    /// operator 2026-07-05: assume comfort in space, not austerity).
    /// Defaults to baseline for rows that omit it.
    #[serde(default)]
    pub tier: String,
}

/// One survival loop of the ideal homestead (power/water/food/air/nutrients/
/// shelter): the sized demand, whether it closes, why, and its parts list.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct OutlineLoop {
    pub id: String,
    pub name: String,
    pub demand: String,
    pub closes: bool,
    pub closure_note: String,
    /// The arithmetic behind the loop's claims, one line a step (in_game_next's first item:
    /// "the arithmetic that backs every claim on this page is in the data file and invisible in
    /// the app"). Shown under "How this is worked out" since 2026-10-30.
    #[serde(default)]
    pub balance: Vec<String>,
    /// What the loop's figures assume (the site, the sources), one line each; shown beside the
    /// arithmetic.
    #[serde(default)]
    pub assumptions: Vec<String>,
    #[serde(default)]
    pub requirements: Vec<OutlineRequirement>,
}

/// The ideal closed-loop homestead outline (operator, 2026-07-05: "use Home as
/// a page for outlining what we need in the perfect ideal 100% closed loop
/// self-sustaining homestead"). Doubles as the game's requirements list for
/// the Home feature. Data: data/home_outline.json (top-level so the web deploy
/// publishes it; web/pages/home.html renders the SAME file -- web mirrors
/// native). Numbers distilled from docs/design/homestead-solo-design.md.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct HomeOutline {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub subtitle: String,
    #[serde(default)]
    pub intro: String,
    #[serde(default)]
    pub loops: Vec<OutlineLoop>,
    #[serde(default)]
    pub in_game_next: Vec<String>,
    #[serde(default)]
    pub footer: String,
}

/// Pure loader (unit-tested below): parse data/home_outline.json, the built-in
/// copy when the data folder's is missing or this version cannot read it
/// (BUG-163). When neither loads the data is empty (the panel hides; the page
/// still works).
pub fn load_home_outline(data_dir: &std::path::Path) -> HomeOutline {
    crate::embedded_data::load_data_or_embedded(data_dir, "home_outline.json", crate::data_parse::parse_json::<HomeOutline>)
        .unwrap_or_else(|e| {
            log::warn!("{e}; the home outline panel is hidden");
            HomeOutline::default()
        })
}

fn home_outline() -> &'static HomeOutline {
    static CACHE: std::sync::OnceLock<HomeOutline> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| load_home_outline(&crate::data_dir()))
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(16.0))
        .show(ctx, |ui| {
            // ONE scroll for the whole page (2026-10-30): the Household card and the animals used to
            // stand still above a scroll of their own, and the Household card had a third, capped
            // one inside it.
            ScrollArea::vertical().id_salt("homes_page").auto_shrink([false, false]).show(ui, |ui| {
                // Clear of the scroll bar, which floats over the right edge.
                let clear = theme.spacing_md.round().clamp(0.0, 127.0) as i8;
                Frame::none().inner_margin(egui::Margin { right: clear, ..Default::default() }).show(ui, |ui| {
                    draw_page(ui, theme, state);
                });
            });
        });
}

/// The page inside its scroll: who lives here, the animals, then the homestead's own section.
fn draw_page(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // Home > Household (ship homes 5b, household.rs) beside Where you live (ship homes increment 6,
    // where_you_live.rs: "In this world you live in a Homestead (a family of three).", read only;
    // Settings > Gameplay changes the household, Characters chooses the start), side by side when
    // both fit, one above the other on a narrow window.
    cards::Grid::new("homes_top").plain().min_width(460.0).max_columns(2).show(ui, theme, 2, |ui, i| {
        if i == 0 {
            super::household::draw_card(ui, theme, state);
        } else {
            super::where_you_live::draw_home_line(ui, theme);
        }
    });
    // Home > Animals (2026-10-12, herd.rs): the farm animals, fed and watered or not.
    super::herd::draw_card(ui, theme, state);
    // The homestead's own section below (its design, its loops, what one home cannot
    // close) only for a home that has loops: a Cabin's household file has none, and a
    // cabin is not an acre to close loops on (increment 6, plan 3.9).
    if !home_has_loops(state) {
        return;
    }
    // Clone so the immutable design can be browsed without holding a borrow
    // of state (cheap; the blueprint is small).
    let Some(design) = state.homestead_design.clone() else {
        ui.add_space(theme.spacing_md);
        ui.label(RichText::new(t("home.your_home")).size(theme.font_size_title).color(theme.text_primary()));
        ui.add_space(theme.spacing_sm);
        ui.label(RichText::new(t("home.no_blueprint")).color(theme.text_muted()));
        return;
    };
    // The home's food, computed from the crops in its placed grow machines: the
    // same figure the machine cards and the garden overview sum.
    let grown_kcal = state.home_machines.as_ref().map_or(0.0, |h| h.grown_kcal_per_day());
    // Each loop's live daily energy (2026-10-22): the machines as they ran over their last
    // game day, by the meter the Construction page shows.
    let basis = crate::machines::MeterBasis {
        life_support_on_grid: state.garden_pests.life_support_realistic,
        recorded: Some(&state.machine_day_energy),
    };
    let metered: Vec<Option<String>> = state
        .homestead_loops
        .iter()
        .map(|l| l.meter.zip(state.home_machines.as_ref()).and_then(|(m, h)| h.loop_meter_line(m, crate::machines::METER_SUN_HOURS, basis)))
        .collect();
    // The power meter's make and use, kWh a day: the Energy loop's gauge (its words say they are
    // this meter's figures).
    let power_meter = state.home_machines.as_ref().and_then(|h| {
        h.utility_meters(crate::machines::METER_SUN_HOURS, basis).into_iter().find(|m| m.utility == "power").map(|m| (m.generation, m.demand))
    });
    let now = HomeNow {
        power: LivePower {
            gen: state.power_generation,
            usage: state.power_consumption,
            balance: state.power_balance,
            battery_wh: state.power_battery_wh,
            capacity_wh: state.power_battery_capacity_wh,
            autonomy: state.power_autonomy_hours,
            ship: state.power_ship,
        },
        water: LiveWater {
            production: state.water_production_lpm,
            demand: state.water_demand_lpm,
            stored: state.water_stored_l,
            capacity: state.water_capacity_l,
            days_autonomy: state.water_days_autonomy,
        },
        air: LiveAir {
            o2: state.air_o2_pct,
            co2: state.air_co2_pct,
            pressure: state.air_pressure_atm,
            temp_c: state.air_temp_c,
            breathable: state.air_breathable,
        },
        loops: &state.homestead_loops,
        metered: &metered,
        grown_kcal,
        power_meter,
    };
    draw_design(ui, theme, &design, &state.tower_configs, &state.tower_compat, &now);
}

/// Whether the home has the homestead's coupled loops: the live household file's (a Cabin's
/// cabin.ron has none). With no household file loaded nothing says otherwise, so the page draws
/// as it always did (its "No homestead blueprint loaded" line included).
pub(crate) fn home_has_loops(state: &GuiState) -> bool {
    state.home_machines.as_ref().is_none_or(|home| !home.loops.is_empty())
}

/// Live power readout from the running sim (SolarSystem + ElectricalSystem -> PowerStatus
/// -> GuiState), passed into `draw_design` (which has no `&GuiState`). Zero in a build
/// with no home.ron -> the Live power card hides itself.
#[derive(Clone, Copy, Default)]
struct LivePower {
    gen: f32,
    usage: f32,
    balance: f32,
    battery_wh: f32,
    capacity_wh: f32,
    autonomy: f32,
    /// The ship's reactor feed and its metered totals (systems::ship_power).
    ship: crate::systems::electrical::ShipFeedReading,
}

/// Live WATER readout from the running sim (PlumbingSystem -> WaterStatus -> GuiState), passed into
/// `draw_design`. Zero with no water machines -> the Live water card hides itself. (v0.608)
#[derive(Clone, Copy, Default)]
struct LiveWater {
    production: f32,
    demand: f32,
    stored: f32,
    capacity: f32,
    days_autonomy: f32,
}

/// Live AIR readout from the running sim (AtmosphereSystem -> AirStatus -> GuiState). Zero pressure ->
/// no home air space spawned yet -> the Live air card hides itself. (v0.617)
#[derive(Clone, Copy, Default)]
struct LiveAir {
    o2: f32,
    co2: f32,
    pressure: f32,
    temp_c: f32,
    breathable: bool,
}

/// The home as it runs, for `draw_design` (which has no `&GuiState`): the live readouts, the
/// loops with their live lines, the food grown and the power meter's (make, use).
struct HomeNow<'a> {
    power: LivePower,
    water: LiveWater,
    air: LiveAir,
    loops: &'a [crate::machines::HomeLoop],
    metered: &'a [Option<String>],
    grown_kcal: f32,
    power_meter: Option<(f32, f32)>,
}

/// The cards in the row of readouts under the scale, in order; a live one only while the sim
/// reports it.
#[derive(Clone, Copy, PartialEq)]
enum Readout {
    Glance,
    Power,
    Water,
    Air,
    Kit,
}

/// What the At a glance card counts at the chosen scale.
struct Glance {
    rooms: usize,
    power_w: u32,
    water_l: u32,
    kinds: usize,
    parts: u32,
}

/// A readout card's title and the muted line under it.
fn card_title(ui: &mut egui::Ui, theme: &Theme, title: &str, note: Option<&str>) {
    ui.label(RichText::new(title).size(theme.font_size_body).strong().color(theme.text_primary()));
    if let Some(note) = note {
        ui.label(RichText::new(note).size(theme.font_size_small).color(theme.text_muted()));
    }
    ui.add_space(theme.spacing_xs);
}

/// "Balance  +1350 W surplus" in the good or bad colour.
fn coloured_row(ui: &mut egui::Ui, theme: &Theme, label: &str, value: &str, colour: egui::Color32) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(label).size(theme.font_size_small).color(theme.text_secondary()));
        ui.label(RichText::new(value).size(theme.font_size_small).strong().color(colour));
    });
}

/// A store's fill colour: green while comfortable, amber when low, red when nearly empty.
fn fill_colour(theme: &Theme, fraction: f32) -> egui::Color32 {
    if fraction >= 0.5 {
        theme.success()
    } else if fraction >= 0.2 {
        theme.warning()
    } else {
        theme.danger()
    }
}

fn draw_design(ui: &mut egui::Ui, theme: &Theme, design: &HomesteadDesign, towers: &[TowerConfig], compat: &[TowerCompat], now: &HomeNow) {
    // ── Your Home: the design and the scale it is shown at ──
    ui.add_space(theme.spacing_md);
    ui.label(RichText::new(t("home.your_home")).size(theme.font_size_title).color(theme.text_primary()));
    ui.label(RichText::new(&design.name).size(theme.font_size_heading).color(theme.accent()));
    cards::prose(ui, theme, &design.description, theme.text_muted());
    // (The "Offline home ... design: fibonacci ... saving is the next step" card that stood here
    // was retired in ship homes increment 6: saves have long kept progress, a save names its start
    // rather than a design, and the "Where you live" card above says which home this is.)
    ui.add_space(theme.spacing_sm);

    // Scale selector, its line beside it.
    let scale = with_scale(|s| {
        ui.horizontal_wrapped(|ui| {
            for sc in Scale::ALL {
                if widgets::Button::secondary(sc.label()).active(*s == sc).hint(sc.hint_key()).show(ui, theme) {
                    *s = sc;
                }
            }
            ui.add_space(theme.spacing_sm);
            ui.label(RichText::new(s.blurb()).size(theme.font_size_small).color(theme.text_muted()));
        });
        *s
    });
    ui.add_space(theme.spacing_md);

    // Rooms included at this scale.
    let rooms: Vec<&DesignRoom> = design
        .rooms
        .iter()
        .filter(|r| r.fibonacci_index <= scale.ceiling())
        .collect();

    let total_power: u32 = rooms.iter().map(|r| r.requirements.power_watts).sum();
    let total_water: u32 = rooms.iter().map(|r| r.requirements.water_liters_per_day).sum();

    // Aggregate the bill of materials across the included rooms.
    let mut bom: Vec<(String, u32)> = Vec::new();
    for r in &rooms {
        for (id, qty) in &r.requirements.materials {
            if let Some(slot) = bom.iter_mut().find(|s| s.0 == *id) {
                slot.1 += *qty;
            } else {
                bom.push((id.clone(), *qty));
            }
        }
    }
    bom.sort_by(|a, b| b.1.cmp(&a.1));
    let total_parts: u32 = bom.iter().map(|(_, q)| *q).sum();
    let glance = Glance { rooms: rooms.len(), power_w: total_power, water_l: total_water, kinds: bom.len(), parts: total_parts };
    let kit = self_sufficiency_kit(&bom);

    // ── The home now: at a glance, the live readouts and the systems, side by side ──
    let (power, water, air) = (now.power, now.water, now.air);
    let mut readouts = vec![Readout::Glance];
    // Generation swings with day/night; the battery charges on surplus + discharges on deficit.
    // This is the home as a LIVE sim, not the authored demand (v0.518).
    if power.gen > 0.0 || power.usage > 0.0 || power.capacity_wh > 0.0 || power.ship.fed {
        readouts.push(Readout::Power);
    }
    // Production needs power (cut the power and the cistern stops filling, but for the cistern's
    // own trickle of rain; the purifier makes no water, 2026-10-05); the cistern buffers the
    // difference, so "days of water" is a draining number coupled to the power sim (v0.608).
    if water.production > 0.0 || water.demand > 0.0 || water.capacity > 0.0 {
        readouts.push(Readout::Water);
    }
    // The home is a sealed habitat: this is its life-support air (v0.617).
    if air.pressure > 0.0 {
        readouts.push(Readout::Air);
    }
    readouts.push(Readout::Kit);
    cards::Grid::new("homes_now").min_width(290.0).show(ui, theme, readouts.len(), |ui, i| match readouts[i] {
        Readout::Glance => glance_card(ui, theme, &glance),
        Readout::Power => power_card(ui, theme, power),
        Readout::Water => water_card(ui, theme, water),
        Readout::Air => air_card(ui, theme, air),
        Readout::Kit => kit_card(ui, theme, &kit),
    });

    // ── Loop closure (the self-sufficiency demonstration, v0.432) ──
    // The coupled loops with whether each closes, from data/machines/home.ron.
    super::home_loops::draw_home_loops(ui, theme, now.loops, now.metered, now.grown_kcal, now.power_meter);

    // ── The ideal closed loop (the requirements outline) ──
    // Operator 2026-07-05: Home outlines the perfect ideal closed-loop homestead; the outline IS
    // the game's requirements list for Home. Data-driven from data/home_outline.json; web
    // home.html renders the same file (web mirrors native). Its long words fold since 2026-10-30
    // (the operator's "brutal walls of text"), which supersedes the 2026-07-05 "all the info
    // immediately exposed": every fact is still there, a click away.
    super::home_loops::draw_outline(ui, theme, home_outline());

    // ── What one home cannot close (the pedagogical payoff) ──
    // The survival loops above close; these five gaps do NOT, by the design of reality: the game's
    // own recipes (manufacture_cpu, smelt_steel, ...) abstract away industrial infrastructure no
    // single homestead can carry, and for medicine there is no recipe at all (the trading post
    // sells antibiotics since 2026-10-05). Edged in the warning colour so they read apart from the
    // loops that close. Data: data/self_sufficiency/cannot_close.ron (homestead-solo-design.md
    // section 8).
    super::home_loops::draw_cannot_close(ui, theme, cannot_close());

    // ── Bill of materials ──
    egui::CollapsingHeader::new(
        RichText::new(tf("home.bom_title", &[("kinds", &bom.len()), ("total", &total_parts)]))
            .size(theme.font_size_body)
            .strong()
            .color(theme.text_primary()),
    )
    .id_salt("homes_bom")
    .default_open(true)
    .show(ui, |ui| {
        cards::prose(ui, theme, t("home.bom_note"), theme.text_muted());
        ui.add_space(theme.spacing_xs);
        cards::Grid::new("homes_bom_grid").plain().min_width(230.0).gap(theme.spacing_xs).show(ui, theme, bom.len(), |ui, i| {
            let (id, qty) = &bom[i];
            widgets::detail_row(ui, theme, &humanize(id), &format!("x{qty}"));
        });
    });
    ui.add_space(theme.spacing_sm);

    // ── Rooms by tier, each room a card ──
    cards::group_heading(ui, theme, t("home.rooms_title"), None);
    for tier in &design.tiers {
        let tier_rooms: Vec<&DesignRoom> = rooms
            .iter()
            .filter(|r| r.tier.as_str() == tier.id.as_str())
            .copied()
            .collect();
        if tier_rooms.is_empty() {
            continue;
        }
        egui::CollapsingHeader::new(
            RichText::new(format!("{} ({})", tier.name, tier_rooms.len()))
                .size(theme.font_size_body)
                .strong()
                .color(theme.accent()),
        )
        .id_salt(("homes_tier", tier.id.as_str()))
        .default_open(true)
        .show(ui, |ui| {
            if !tier.description.is_empty() {
                cards::prose(ui, theme, &tier.description, theme.text_muted());
                ui.add_space(theme.spacing_xs);
            }
            cards::Grid::new(("homes_rooms", tier.id.as_str())).min_width(260.0).show_with_details(
                ui,
                theme,
                tier_rooms.len(),
                |ui, i| room_card(ui, theme, tier_rooms[i]),
                |ui, i| room_words(ui, theme, tier_rooms[i]),
            );
        });
    }

    // ── Aeroponic towers (the food loop; v0.382) ──
    if !towers.is_empty() {
        cards::group_heading(ui, theme, t("home.towers_title"), Some(t("home.towers_note")));
        cards::Grid::new("homes_towers").min_width(460.0).top_aligned().show(ui, theme, towers.len(), |ui, i| draw_tower(ui, theme, &towers[i], compat.get(i)));
    }
}

/// At a glance: the rooms, power, water and parts at the chosen scale.
fn glance_card(ui: &mut egui::Ui, theme: &Theme, g: &Glance) {
    card_title(ui, theme, t("home.glance"), None);
    widgets::detail_row(ui, theme, t("home.rooms"), &g.rooms.to_string());
    widgets::detail_row(ui, theme, t("home.power_demand"), &format!("{} W", g.power_w));
    widgets::detail_row(ui, theme, t("home.water_demand"), &format!("{} L/day", g.water_l));
    widgets::detail_row(ui, theme, t("home.distinct_parts"), &g.kinds.to_string());
    widgets::detail_row(ui, theme, t("home.total_parts"), &g.parts.to_string());
}

/// Live power: what the home makes against what it uses (the gauge), the balance, the battery.
fn power_card(ui: &mut egui::Ui, theme: &Theme, p: LivePower) {
    card_title(ui, theme, t("home.live_power"), Some(t("home.live_power_note")));
    if p.usage > 0.0 {
        cards::gauge(ui, theme, p.gen, p.usage, if p.gen >= p.usage { theme.success() } else { theme.warning() });
        ui.add_space(theme.spacing_xs);
    }
    widgets::detail_row(ui, theme, t("home.generation"), &format!("{:.0} W", p.gen));
    widgets::detail_row(ui, theme, t("home.consumption"), &format!("{:.0} W", p.usage));
    if p.balance >= 0.0 {
        coloured_row(ui, theme, t("home.balance"), &tf("home.surplus", &[("w", &format!("{:.0}", p.balance))]), theme.success());
    } else {
        coloured_row(ui, theme, t("home.balance"), &tf("home.deficit", &[("w", &format!("{:.0}", p.balance))]), theme.danger());
    }
    if p.capacity_wh > 0.0 {
        let f = (p.battery_wh / p.capacity_wh).clamp(0.0, 1.0);
        let value = tf(
            "home.battery_value",
            &[("pct", &format!("{:.0}", f * 100.0)), ("kwh", &format!("{:.1}", p.battery_wh / 1000.0)), ("hours", &format!("{:.1}", p.autonomy))],
        );
        widgets::detail_row(ui, theme, t("home.battery"), &value);
        cards::gauge(ui, theme, p.battery_wh, p.capacity_wh, fill_colour(theme, f));
        ui.add_space(theme.spacing_xs);
    }
    ship_power_rows(ui, theme, p.ship);
}

/// Live water: production against demand, the balance, the cistern.
fn water_card(ui: &mut egui::Ui, theme: &Theme, w: LiveWater) {
    card_title(ui, theme, t("home.live_water"), Some(t("home.live_water_note")));
    if w.demand > 0.0 {
        cards::gauge(ui, theme, w.production, w.demand, if w.production >= w.demand { theme.success() } else { theme.warning() });
        ui.add_space(theme.spacing_xs);
    }
    widgets::detail_row(ui, theme, t("home.production"), &format!("{:.1} L/min", w.production));
    widgets::detail_row(ui, theme, t("home.demand"), &format!("{:.1} L/min", w.demand));
    let net = w.production - w.demand;
    if net >= 0.0 {
        coloured_row(ui, theme, t("home.balance"), &tf("home.filling", &[("lpm", &format!("{net:.1}"))]), theme.success());
    } else {
        coloured_row(ui, theme, t("home.balance"), &tf("home.draining", &[("lpm", &format!("{net:.1}"))]), theme.danger());
    }
    if w.capacity > 0.0 {
        let f = (w.stored / w.capacity).clamp(0.0, 1.0);
        let value = tf(
            "home.cistern_value",
            &[
                ("pct", &format!("{:.0}", f * 100.0)),
                ("stored", &format!("{:.0}", w.stored)),
                ("capacity", &format!("{:.0}", w.capacity)),
                ("days", &format!("{:.1}", w.days_autonomy)),
            ],
        );
        widgets::detail_row(ui, theme, t("home.cistern"), &value);
        cards::gauge(ui, theme, w.stored, w.capacity, fill_colour(theme, f));
    }
}

/// Live air: the sealed home's mix and whether it is breathable.
fn air_card(ui: &mut egui::Ui, theme: &Theme, a: LiveAir) {
    card_title(ui, theme, t("home.live_air"), Some(t("home.live_air_note")));
    widgets::detail_row(ui, theme, t("home.oxygen"), &format!("{:.1}%", a.o2));
    widgets::detail_row(ui, theme, t("home.co2"), &format!("{:.2}%", a.co2));
    widgets::detail_row(ui, theme, t("home.pressure"), &format!("{:.2} atm", a.pressure));
    widgets::detail_row(ui, theme, t("home.temp"), &format!("{:.0} C", a.temp_c));
    ui.add_space(theme.spacing_xs);
    if a.breathable {
        cards::status_badge(ui, theme, t("home.breathable"), theme.success());
    } else {
        cards::status_badge(ui, theme, t("home.not_breathable"), theme.danger());
    }
}

/// The self-sufficiency systems in the bill of materials at this scale.
fn kit_card(ui: &mut egui::Ui, theme: &Theme, kit: &[(&'static str, u32)]) {
    card_title(ui, theme, t("home.kit"), Some(t("home.kit_note")));
    if kit.is_empty() {
        ui.label(RichText::new(t("home.kit_none")).size(theme.font_size_small).color(theme.text_muted()));
    }
    for (label, n) in kit {
        widgets::detail_row(ui, theme, label, &n.to_string());
    }
}

/// One aeroponic tower, collapsible: purpose + what it covers / its gaps +
/// disclaimer + the 50-slot planting list (count, plant, role, note).
fn draw_tower(ui: &mut egui::Ui, theme: &Theme, tower: &TowerConfig, compat: Option<&TowerCompat>) {
    let planted: u32 = tower.plantings.iter().map(|p| p.slots).sum();
    egui::CollapsingHeader::new(
        RichText::new(tf("home.tower_title", &[("name", &tower.name), ("planted", &planted), ("slots", &tower.slots)]))
            .size(theme.font_size_body)
            .strong()
            .color(theme.accent()),
    )
    .id_salt(("tower", tower.id.as_str()))
    .show(ui, |ui| {
        if !tower.purpose.is_empty() {
            ui.label(RichText::new(&tower.purpose).size(theme.font_size_small).color(theme.text_secondary()));
        }
        if !tower.description.is_empty() {
            ui.label(RichText::new(&tower.description).size(theme.font_size_small).color(theme.text_muted()));
        }
        ui.add_space(theme.spacing_xs);
        if !tower.covers.is_empty() {
            ui.label(
                RichText::new(tf("home.tower_covers", &[("list", &tower.covers.join(", "))]))
                    .size(theme.font_size_small)
                    .color(theme.success()),
            );
        }
        if !tower.gaps.is_empty() {
            ui.label(
                RichText::new(tf("home.tower_gaps", &[("list", &tower.gaps.join(", "))]))
                    .size(theme.font_size_small)
                    .color(theme.warning()),
            );
            if !tower.gaps_note.is_empty() {
                ui.label(RichText::new(&tower.gaps_note).size(theme.font_size_small).color(theme.text_muted()));
            }
        }
        if !tower.disclaimer.is_empty() {
            ui.add_space(theme.spacing_xs);
            ui.label(RichText::new(&tower.disclaimer).size(theme.font_size_small).color(theme.text_muted()).italics());
        }
        // ── Grow-together check: can these plants share one reservoir + air? ──
        // (operator: "make sure they'd all grow together too"). Aeroponics shares a
        // reservoir + air, not soil, so the constraint is a common temp/humidity
        // window; the solution's pH is held by the grower and shown as its
        // setpoint (loaders::TowerCompat). Green = one shared window; warnings
        // name the plants that conflict.
        if let Some(c) = compat {
            ui.add_space(theme.spacing_xs);
            let shared: Vec<String> = [
                c.held_ph.map(|p| tf("home.tower_ph", &[("ph", &format!("{p:.1}"))])),
                c.temp.map(|(a, b)| format!("{:.0}-{:.0}°C", a, b)),
                c.humidity.map(|(a, b)| tf("home.tower_humidity", &[("low", &format!("{:.0}", a * 100.0)), ("high", &format!("{:.0}", b * 100.0))])),
            ]
            .into_iter()
            .flatten()
            .collect();
            if c.conflicts.is_empty() {
                ui.label(
                    RichText::new(format!("✓ {}", tf("home.tower_share", &[("n", &c.species), ("list", &shared.join(", "))])))
                        .size(theme.font_size_small)
                        .color(theme.success()),
                );
            } else {
                if !shared.is_empty() {
                    ui.label(
                        RichText::new(tf("home.tower_shared_where", &[("list", &shared.join(", "))]))
                            .size(theme.font_size_small)
                            .color(theme.text_secondary()),
                    );
                }
                for note in &c.conflicts {
                    ui.label(
                        RichText::new(format!("⚠ {}", note))
                            .size(theme.font_size_small)
                            .color(theme.warning()),
                    );
                }
                ui.label(RichText::new(t("home.tower_split")).size(theme.font_size_small).color(theme.text_muted()));
            }
            // Self-sufficiency numbers: total daily water draw + the harvest window
            // (ties the tower into the homestead's water + food loops).
            let mut stats: Vec<String> = Vec::new();
            if c.water_per_day_total > 0.0 {
                stats.push(tf("home.tower_water", &[("l", &format!("{:.1}", c.water_per_day_total))]));
            }
            if c.full_harvest_days > 0.0 {
                stats.push(tf("home.tower_harvest", &[("first", &format!("{:.0}", c.first_harvest_days)), ("full", &format!("{:.0}", c.full_harvest_days))]));
            }
            if !stats.is_empty() {
                ui.label(
                    RichText::new(stats.join("  ·  "))
                        .size(theme.font_size_small)
                        .color(theme.text_secondary()),
                );
            }
        }
        ui.add_space(theme.spacing_sm);
        for p in &tower.plantings {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("x{}", p.slots)).size(theme.font_size_small).strong().color(theme.accent()));
                ui.label(RichText::new(humanize(&p.plant)).size(theme.font_size_small).color(theme.text_primary()));
                if !p.role.is_empty() {
                    ui.label(RichText::new(format!("· {}", p.role)).size(theme.font_size_small).color(theme.text_muted()));
                }
            });
            if !p.note.is_empty() {
                ui.label(RichText::new(&p.note).size(theme.font_size_small).color(theme.text_secondary()));
            }
            ui.add_space(2.0);
        }
        // ── Parts list: what to actually BUILD the tower from (the game->real
        //    bridge / north star). Data-driven (the RON's `parts`); a starting
        //    bill of materials the operator + community refine. ──
        if !tower.parts.is_empty() {
            ui.add_space(theme.spacing_sm);
            ui.label(RichText::new(t("home.tower_parts")).size(theme.font_size_body).strong().color(theme.text_primary()));
            ui.label(RichText::new(t("home.tower_parts_note")).size(theme.font_size_small).color(theme.text_muted()));
            ui.add_space(2.0);
            for part in &tower.parts {
                ui.horizontal(|ui| {
                    if !part.qty.is_empty() {
                        ui.label(RichText::new(&part.qty).size(theme.font_size_small).strong().color(theme.accent()));
                    }
                    ui.label(RichText::new(&part.name).size(theme.font_size_small).color(theme.text_primary()));
                    if !part.source.is_empty() {
                        ui.label(
                            RichText::new(format!("· {}", part_source_label(&part.source)))
                                .size(theme.font_size_small)
                                .color(theme.text_muted()),
                        );
                    }
                });
                if !part.note.is_empty() {
                    ui.label(RichText::new(&part.note).size(theme.font_size_small).color(theme.text_secondary()));
                }
                ui.add_space(2.0);
            }
        }
    });
}

/// Humanize a part `source` tag for display ("3d_print" -> "3D-print").
fn part_source_label(source: &str) -> &str {
    match source {
        "3d_print" => "3D-print",
        "diy" => "DIY",
        other => other,
    }
}

/// One room's card: its Fibonacci number and name, its purpose cut to two lines, its footprint,
/// power and water; its environment notes and parts behind a fold ([`room_words`] draws them).
/// Returns whether that fold is open.
fn room_card(ui: &mut egui::Ui, theme: &Theme, r: &DesignRoom) -> bool {
    ui.horizontal(|ui| {
        cards::plain_badge(ui, theme, &format!("F{}", r.fibonacci_index));
        ui.label(RichText::new(&r.name).size(theme.font_size_body).strong().color(theme.text_primary()));
    });
    if !r.purpose.is_empty() {
        ui.add_space(theme.spacing_xs);
        cards::clamped(ui, theme, &r.purpose, theme.font_size_small, theme.text_secondary(), 2);
    }
    ui.add_space(theme.spacing_xs);
    widgets::detail_row(ui, theme, t("home.footprint"), &format!("{:.0} x {:.0} m", r.size.x, r.size.z));
    widgets::detail_row(ui, theme, t("home.power"), &format!("{} W", r.requirements.power_watts));
    widgets::detail_row(ui, theme, t("home.water"), &format!("{} L/day", r.requirements.water_liters_per_day));
    (!r.environment_notes.is_empty() || !r.requirements.materials.is_empty())
        && cards::fold(ui, theme, ("homes_room_more", r.id.as_str()), t("home.room_more"), "home.room_more_hint")
}

/// A room's opened words, under its row: its environment notes beside the parts it is built from.
fn room_words(ui: &mut egui::Ui, theme: &Theme, r: &DesignRoom) {
    cards::detail_heading(ui, theme, &format!("F{}  {}", r.fibonacci_index, r.name), t("home.room_more"));
    cards::Grid::new(("homes_room_detail", r.id.as_str())).plain().min_width(320.0).max_columns(2).gap(theme.spacing_lg).show(ui, theme, 2, |ui, part| {
        if part == 0 {
            if !r.environment_notes.is_empty() {
                cards::prose(ui, theme, &r.environment_notes, theme.text_secondary());
            }
        } else if !r.requirements.materials.is_empty() {
            ui.label(RichText::new(t("home.parts_title")).size(theme.font_size_small).strong().color(theme.text_secondary()));
            cards::Grid::new(("homes_room_parts", r.id.as_str())).plain().min_width(200.0).gap(theme.spacing_xs).show(ui, theme, r.requirements.materials.len(), |ui, k| {
                if let Some((id, qty)) = r.requirements.materials.get(k) {
                    widgets::detail_row(ui, theme, &humanize(id), &format!("x{qty}"));
                }
            });
        }
    });
}

/// Sum the generation / collection / recycling components in the BOM by group, so
/// the page can show, at a glance, the systems that close the homestead's loops. Each group is
/// its words' key and the ids it gathers (the page's own presentation of the BOM).
fn self_sufficiency_kit(bom: &[(String, u32)]) -> Vec<(&'static str, u32)> {
    let groups: [(&str, &[&str]); 5] = [
        ("home.kit_solar", &["solar_panel"]),
        ("home.kit_wind", &["wind_turbine"]),
        ("home.kit_generators", &["generator", "battery"]),
        ("home.kit_water", &["water_tank", "water_pump", "water_purifier", "irrigation"]),
        ("home.kit_food", &["greenhouse", "composter"]),
    ];
    let mut out = Vec::new();
    for (key, subs) in groups {
        let n: u32 = bom
            .iter()
            .filter(|(id, _)| subs.iter().any(|s| id.contains(*s)))
            .map(|(_, q)| *q)
            .sum();
        if n > 0 {
            out.push((crate::i18n::t_dyn(key), n));
        }
    }
    out
}

/// The ship's reactor on the Live power card (2026-09-27, systems::ship_power):
/// in the Station-supplied mode what it supplies now and what the home has drawn
/// from it in all, metered; in the Realistic mode a line saying the home runs on
/// its own. Shown whatever the balance: using ship power is never a fault.
fn ship_power_rows(ui: &mut egui::Ui, theme: &Theme, ship: crate::systems::electrical::ShipFeedReading) {
    if !ship.fed {
        ui.label(RichText::new(t("home.off_reactor")).size(theme.font_size_small).color(theme.text_muted()));
        return;
    }
    widgets::detail_row(
        ui,
        theme,
        t("home.ship_reactor"),
        &tf("home.ship_reactor_value", &[("now", &format!("{:.0}", ship.drawn_w)), ("back", &format!("{:.0}", ship.returned_w))]),
    );
    widgets::detail_row(
        ui,
        theme,
        t("home.ship_metered"),
        &tf("home.ship_metered_value", &[("drawn", &format!("{:.2}", ship.drawn_wh / 1000.0)), ("returned", &format!("{:.2}", ship.returned_wh / 1000.0))]),
    );
}

#[cfg(test)]
mod tests {
    /// The shipped cannot-close data parses and is complete: every entry carries a
    /// non-empty title/body/provided_by, ids are unique, all five design-doc
    /// categories (homestead-solo-design.md section 8) are present, and the copy is
    /// em-dash free (operator rule: no em dashes in user-facing copy).
    #[test]
    fn cannot_close_data_parses_and_is_complete() {
        let data = super::load_cannot_close(std::path::Path::new("data"));
        assert!(
            data.entries.len() >= 5,
            "expected the 5 cannot-close categories, got {}",
            data.entries.len()
        );
        assert!(!data.intro.is_empty(), "cannot_close.ron should carry an intro line");
        assert!(!data.footer.is_empty(), "cannot_close.ron should carry the framing footer");
        let mut ids: Vec<&str> = data.entries.iter().map(|e| e.id.as_str()).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n, "cannot_close.ron has duplicate entry ids");
        for e in &data.entries {
            assert!(!e.id.is_empty(), "entry with empty id");
            assert!(!e.title.is_empty(), "entry '{}' has empty title", e.id);
            assert!(!e.body.is_empty(), "entry '{}' has empty body", e.id);
            assert!(!e.provided_by.is_empty(), "entry '{}' has empty provided_by", e.id);
            for s in [&e.title, &e.body, &e.provided_by, &data.intro, &data.footer] {
                assert!(
                    !s.contains('\u{2014}'),
                    "em dash in cannot-close copy for '{}' (operator rule: none in user-facing text)",
                    e.id
                );
            }
        }
        // The five categories from the design doc, by id.
        for id in ["electronics", "metal_at_ore_scale", "medicine", "equipment_renewal", "raw_chemistry"] {
            assert!(
                data.entries.iter().any(|e| e.id == id),
                "missing cannot-close category '{}'",
                id
            );
        }
    }

    /// A missing file degrades to empty data (panel hides), never a panic.
    #[test]
    fn cannot_close_missing_file_is_empty() {
        let data = super::load_cannot_close(std::path::Path::new("data/definitely_not_a_dir"));
        assert!(data.entries.is_empty());
        assert!(data.intro.is_empty());
    }

    #[test]
    fn home_outline_parses_and_is_complete() {
        let o = super::load_home_outline(std::path::Path::new("data"));
        assert!(!o.title.is_empty(), "home_outline.json should carry a title");
        assert!(!o.intro.is_empty(), "home_outline.json should carry an intro");
        assert!(!o.footer.is_empty(), "home_outline.json should carry a footer");
        assert!(
            o.loops.len() >= 7,
            "expected the 7 loops (power/water/food/air/climate/nutrients/shelter), got {}",
            o.loops.len()
        );
        let mut luxury_rows = 0usize;
        let mut flagged_missing = 0usize;
        for l in &o.loops {
            assert!(!l.demand.is_empty(), "loop '{}' has empty demand", l.id);
            assert!(!l.closure_note.is_empty(), "loop '{}' has empty closure_note", l.id);
            assert!(!l.requirements.is_empty(), "loop '{}' has no requirements", l.id);
            let baseline = l
                .requirements
                .iter()
                .filter(|r| r.tier == "baseline" || r.tier.is_empty())
                .count();
            assert!(baseline > 0, "loop '{}' has no bare-minimum tier rows", l.id);
            for r in &l.requirements {
                assert!(
                    r.tier.is_empty() || r.tier == "baseline" || r.tier == "luxury",
                    "loop '{}' row '{}' has unknown tier '{}'",
                    l.id,
                    r.item,
                    r.tier
                );
                if r.tier == "luxury" {
                    luxury_rows += 1;
                }
                if r.game_id.is_empty() {
                    flagged_missing += 1;
                }
            }
            for s in [&l.demand, &l.closure_note] {
                assert!(
                    !s.contains('\u{2014}'),
                    "em dash in outline copy for loop '{}' (operator rule: none in user-facing text)",
                    l.id
                );
            }
        }
        // The life-of-luxury tier is a first-class part of the outline
        // (operator 2026-07-05), and its not-in-game-yet flags are the
        // page's game-gap list; both must survive edits.
        assert!(luxury_rows >= 5, "expected a real luxury tier, got {luxury_rows} rows");
        assert!(
            flagged_missing >= 1,
            "expected at least one 'not in game yet' flag (empty game_id); if the gaps were all authored, update in_game_next too"
        );
    }

    /// Every game_id in the outline must be a REAL id somewhere in the game
    /// data, so the outline stays an honest requirements list. Haystack:
    /// the data files the design doc cross-checked against, plus blueprint
    /// file stems (fibonacci_homestead names a blueprint file).
    #[test]
    fn home_outline_game_ids_are_real() {
        let o = super::load_home_outline(std::path::Path::new("data"));
        let mut hay = String::new();
        for f in [
            "data/machines/home.ron",
            "data/waste_management.ron",
            "data/structures.csv",
            "data/items.csv",
            "data/self_sufficiency/component_outputs.ron",
            "data/towers/aeroponic_configs.ron",
            "data/hvac.ron",
            "data/electrical.ron",
            "data/rooms.ron",
            "data/blueprints/fibonacci_homestead.ron",
            "data/blueprints/materials.ron",
        ] {
            hay.push_str(&std::fs::read_to_string(f).unwrap_or_default());
        }
        if let Ok(entries) = std::fs::read_dir("data/blueprints") {
            for e in entries.flatten() {
                hay.push_str(&e.file_name().to_string_lossy());
                hay.push('\n');
            }
        }
        for l in &o.loops {
            for r in &l.requirements {
                // Empty game_id is the deliberate "not in game yet" flag;
                // only non-empty ids must be real.
                if r.game_id.is_empty() {
                    continue;
                }
                assert!(
                    hay.contains(&r.game_id),
                    "outline loop '{}' requirement '{}' cites game_id '{}' which exists nowhere in the game data",
                    l.id,
                    r.item,
                    r.game_id
                );
            }
        }
    }

    /// A missing DISK outline falls back to the EMBEDDED copy (v0.744
    /// distributed-build completeness) — a zero-file install still shows the
    /// full Home outline instead of a hidden panel. Never a panic.
    #[test]
    fn home_outline_missing_file_falls_back_to_embedded() {
        let o = super::load_home_outline(std::path::Path::new("data/definitely_not_a_dir"));
        assert!(
            !o.loops.is_empty(),
            "embedded home_outline.json should provide the loops when the disk file is absent"
        );
        assert!(!o.title.is_empty());
    }

    #[test]
    fn tower_configs_parse_max_variety() {
        // Loads the real data/towers/aeroponic_configs.ron from the crate root.
        let towers = crate::gui::load_tower_configs(std::path::Path::new("data"));
        assert_eq!(towers.len(), 2, "expected 2 towers, got {}", towers.len());
        for t in &towers {
            let total: u32 = t.plantings.iter().map(|p| p.slots).sum();
            assert!(
                total <= t.slots,
                "tower '{}' plantings {} exceed capacity {}",
                t.id, total, t.slots
            );
            // Max variety: one of each type, no duplicate plants.
            let mut ids: Vec<&str> = t.plantings.iter().map(|p| p.plant.as_str()).collect();
            let n = ids.len();
            ids.sort();
            ids.dedup();
            assert_eq!(
                ids.len(), n,
                "tower '{}' has duplicate plants (max-variety expects distinct)",
                t.id
            );
            assert!(n >= 20, "tower '{}' should showcase variety (>=20 distinct), has {}", t.id, n);
            // Each tower carries a real-world parts list (the game->real bridge),
            // and every part names something with a source.
            assert!(
                t.parts.len() >= 5,
                "tower '{}' should have a starter parts list (>=5), has {}",
                t.id, t.parts.len()
            );
            for part in &t.parts {
                assert!(!part.name.is_empty(), "tower '{}' has a part with no name", t.id);
                assert!(!part.source.is_empty(), "part '{}' in '{}' has no source", part.name, t.id);
            }
        }
    }
}
