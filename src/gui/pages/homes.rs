//! Home — your offline homestead (v0.379).
//!
//! Surfaces the existing Fibonacci homestead blueprint (data/blueprints/
//! fibonacci_homestead.ron) as a browsable Design. Pick a build SCALE (Solo /
//! Family / Community / Colony) and see, for that scale:
//!   - the total power + water DEMAND,
//!   - the aggregated BILL OF MATERIALS (the parts list — the real-world-bridge
//!     north star: what you would 3D-print / buy / trade to build it),
//!   - a SELF-SUFFICIENCY summary (the generation / collection / recycling systems
//!     that close the loops),
//!   - the rooms grouped by construction tier.
//!
//! Offline-first (operator 2026-06-07: "keep developing offline; multiplayer
//! after offline singleplayer works"). Read-only for now (no editing / building
//! yet). Server + Real homes, and exact closure SCORING (output vs demand per
//! loop), are the next data layers. See docs/design/homes-as-profiles.md.

use egui::{RichText, ScrollArea, Frame};
use crate::gui::{GuiState, HomesteadDesign, DesignRoom, TowerConfig, TowerCompat};
use crate::gui::theme::Theme;
use crate::gui::widgets;
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
            Scale::Solo => "Solo",
            Scale::Family => "Family",
            Scale::Community => "Community",
            Scale::Colony => "Colony",
        }
    }
    fn blurb(self) -> &'static str {
        match self {
            Scale::Solo => "Just you: rooms F1 to F8.",
            Scale::Family => "A household: rooms F1 to F13.",
            Scale::Community => "A village: rooms F1 to F55.",
            Scale::Colony => "Everything: all rooms to F233+.",
        }
    }
}

fn with_scale<R>(f: impl FnOnce(&mut Scale) -> R) -> R {
    thread_local! {
        static SCALE: RefCell<Scale> = RefCell::new(Scale::Solo);
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

/// Pure loader (unit-tested below): parse data/home_outline.json. Missing or
/// malformed file yields empty data (the panel hides; the page still works).
pub fn load_home_outline(data_dir: &std::path::Path) -> HomeOutline {
    let path = data_dir.join("home_outline.json");
    let text = match crate::embedded_data::read_data_or_embedded(data_dir, "home_outline.json") {
        Some(t) => t,
        None => return HomeOutline::default(),
    };
    match serde_json::from_str::<HomeOutline>(&text) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("load_home_outline: failed to parse {}: {e}", path.display());
            HomeOutline::default()
        }
    }
}

fn home_outline() -> &'static HomeOutline {
    static CACHE: std::sync::OnceLock<HomeOutline> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| load_home_outline(&crate::data_dir()))
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(16.0))
        .show(ctx, |ui| {
            // Clone so the immutable design can be browsed without holding a borrow
            // of state (cheap; the blueprint is small).
            let Some(design) = state.homestead_design.clone() else {
                ui.label(RichText::new("Home").size(theme.font_size_title).color(theme.text_primary()));
                ui.add_space(theme.spacing_sm);
                ui.label(
                    RichText::new("No homestead blueprint loaded (expected data/blueprints/fibonacci_homestead.ron).")
                        .color(theme.text_muted()),
                );
                return;
            };
            let power = LivePower {
                gen: state.power_generation,
                usage: state.power_consumption,
                balance: state.power_balance,
                battery_wh: state.power_battery_wh,
                capacity_wh: state.power_battery_capacity_wh,
                autonomy: state.power_autonomy_hours,
            };
            let water = LiveWater {
                production: state.water_production_lpm,
                demand: state.water_demand_lpm,
                stored: state.water_stored_l,
                capacity: state.water_capacity_l,
                days_autonomy: state.water_days_autonomy,
            };
            let air = LiveAir {
                o2: state.air_o2_pct,
                co2: state.air_co2_pct,
                pressure: state.air_pressure_atm,
                temp_c: state.air_temp_c,
                breathable: state.air_breathable,
            };
            draw_design(ui, theme, &design, &state.tower_configs, &state.tower_compat, &state.homestead_loops, power, water, air);
        });
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

fn draw_design(
    ui: &mut egui::Ui,
    theme: &Theme,
    design: &HomesteadDesign,
    towers: &[TowerConfig],
    compat: &[TowerCompat],
    loops: &[crate::machines::HomeLoop],
    power: LivePower,
    water: LiveWater,
    air: LiveAir,
) {
    ui.label(RichText::new("Your Home").size(theme.font_size_title).color(theme.text_primary()));
    ui.label(RichText::new(&design.name).size(theme.font_size_heading).color(theme.accent()));
    ui.label(RichText::new(&design.description).size(theme.font_size_small).color(theme.text_muted()));
    ui.add_space(theme.spacing_xs);

    // "Your Home" identity (the save-wrapper model, v0.380): this design IS your
    // offline home. Progressive disclosure -- one home, no manager, until there is
    // a reason for more. Play-loads-this-home + progress persistence is next.
    widgets::card(ui, theme, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Offline home").size(theme.font_size_small).strong().color(theme.accent()));
            ui.label(RichText::new("you own the save").size(theme.font_size_small).color(theme.text_muted()));
        });
        ui.label(
            RichText::new("This homestead is now a save profile (kind: offline, design: fibonacci). Saving your progress here and entering it from Play is the next step.")
                .size(theme.font_size_small)
                .color(theme.text_secondary()),
        );
    });
    ui.add_space(theme.spacing_sm);

    // Scale selector.
    let scale = with_scale(|s| {
        ui.horizontal(|ui| {
            for sc in [Scale::Solo, Scale::Family, Scale::Community, Scale::Colony] {
                if widgets::Button::secondary(sc.label()).active(*s == sc).show(ui, theme) {
                    *s = sc;
                }
            }
        });
        *s
    });
    ui.label(RichText::new(scale.blurb()).size(theme.font_size_small).color(theme.text_muted()));
    ui.add_space(theme.spacing_sm);

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

    // Live power from the running sim (passed in; draw_design has no &GuiState).
    let live_gen = power.gen;
    let live_use = power.usage;
    let live_balance = power.balance;
    let live_battery_wh = power.battery_wh;
    let live_capacity = power.capacity_wh;
    let live_autonomy = power.autonomy;

    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        // ── At a glance ──
        widgets::card(ui, theme, |ui| {
            ui.label(RichText::new("At a glance").size(theme.font_size_body).strong().color(theme.text_primary()));
            ui.add_space(theme.spacing_xs);
            widgets::detail_row(ui, theme, "Rooms", &rooms.len().to_string());
            widgets::detail_row(ui, theme, "Power demand", &format!("{} W", total_power));
            widgets::detail_row(ui, theme, "Water demand", &format!("{} L/day", total_water));
            widgets::detail_row(ui, theme, "Distinct parts", &bom.len().to_string());
            widgets::detail_row(ui, theme, "Total parts", &total_parts.to_string());
        });
        ui.add_space(theme.spacing_sm);

        // ── Live power (the running sim, v0.518) ──
        // Generation swings with day/night; the battery charges on surplus + discharges
        // on deficit. This is the home as a LIVE sim, not the authored demand above.
        if live_gen > 0.0 || live_use > 0.0 || live_capacity > 0.0 {
            widgets::card(ui, theme, |ui| {
                ui.label(RichText::new("Live power").size(theme.font_size_body).strong().color(theme.text_primary()));
                ui.label(
                    RichText::new("The running sim: solar tracks the time of day, the battery buffers the swing.")
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
                ui.add_space(theme.spacing_xs);
                widgets::detail_row(ui, theme, "Generation", &format!("{:.0} W", live_gen));
                widgets::detail_row(ui, theme, "Consumption", &format!("{:.0} W", live_use));
                let (bal_text, bal_color) = if live_balance >= 0.0 {
                    (format!("+{:.0} W surplus", live_balance), theme.success())
                } else {
                    (format!("{:.0} W deficit", live_balance), theme.danger())
                };
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Balance").size(theme.font_size_small).color(theme.text_secondary()));
                    ui.label(RichText::new(bal_text).size(theme.font_size_small).strong().color(bal_color));
                });
                if live_capacity > 0.0 {
                    let pct = (live_battery_wh / live_capacity * 100.0).clamp(0.0, 100.0);
                    widgets::detail_row(
                        ui,
                        theme,
                        "Battery",
                        &format!("{:.0}%  ({:.1} kWh)  ~{:.1} h autonomy", pct, live_battery_wh / 1000.0, live_autonomy),
                    );
                }
            });
            ui.add_space(theme.spacing_sm);
        }

        // ── Live water (the running PlumbingSystem, v0.608) ──
        // Production needs power (cut the power and the cistern stops filling); the cistern buffers
        // the difference, so "days of water" is a draining number coupled to the power sim.
        if water.production > 0.0 || water.demand > 0.0 || water.capacity > 0.0 {
            widgets::card(ui, theme, |ui| {
                ui.label(RichText::new("Live water").size(theme.font_size_body).strong().color(theme.text_primary()));
                ui.label(
                    RichText::new("The running sim: powered pumps + purifiers fill the cistern; cut the power and it drains.")
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
                ui.add_space(theme.spacing_xs);
                widgets::detail_row(ui, theme, "Production", &format!("{:.1} L/min", water.production));
                widgets::detail_row(ui, theme, "Demand", &format!("{:.1} L/min", water.demand));
                let (bal_text, bal_color) = if water.production - water.demand >= 0.0 {
                    (format!("+{:.1} L/min filling", water.production - water.demand), theme.success())
                } else {
                    (format!("{:.1} L/min draining", water.production - water.demand), theme.danger())
                };
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Balance").size(theme.font_size_small).color(theme.text_secondary()));
                    ui.label(RichText::new(bal_text).size(theme.font_size_small).strong().color(bal_color));
                });
                if water.capacity > 0.0 {
                    let pct = (water.stored / water.capacity * 100.0).clamp(0.0, 100.0);
                    widgets::detail_row(
                        ui,
                        theme,
                        "Cistern",
                        &format!("{:.0}%  ({:.0} / {:.0} L)  ~{:.1} days", pct, water.stored, water.capacity, water.days_autonomy),
                    );
                }
            });
            ui.add_space(theme.spacing_sm);
        }

        // ── Live air (the running AtmosphereSystem, v0.617) ──
        // The home is a sealed habitat: this is its life-support air. Stage 1 holds an Earth-like mix;
        // occupancy + powered scrubbers (the power -> air consequence) land next.
        if air.pressure > 0.0 {
            widgets::card(ui, theme, |ui| {
                ui.label(RichText::new("Live air").size(theme.font_size_body).strong().color(theme.text_primary()));
                ui.label(
                    RichText::new("The sealed home's life-support mix. Powered scrubbers will keep it breathable.")
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
                ui.add_space(theme.spacing_xs);
                widgets::detail_row(ui, theme, "Oxygen", &format!("{:.1}%", air.o2));
                widgets::detail_row(ui, theme, "CO2", &format!("{:.2}%", air.co2));
                widgets::detail_row(ui, theme, "Pressure", &format!("{:.2} atm", air.pressure));
                widgets::detail_row(ui, theme, "Temp", &format!("{:.0} C", air.temp_c));
                let (txt, col) = if air.breathable {
                    ("breathable".to_string(), theme.success())
                } else {
                    ("NOT breathable".to_string(), theme.danger())
                };
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Status").size(theme.font_size_small).color(theme.text_secondary()));
                    ui.label(RichText::new(txt).size(theme.font_size_small).strong().color(col));
                });
            });
            ui.add_space(theme.spacing_sm);
        }

        // ── Self-sufficiency systems ──
        let kit = self_sufficiency_kit(&bom);
        widgets::card(ui, theme, |ui| {
            ui.label(RichText::new("Self-sufficiency systems").size(theme.font_size_body).strong().color(theme.text_primary()));
            ui.label(
                RichText::new("What closes your loops: generate power, collect and recycle water, grow food.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_xs);
            if kit.is_empty() {
                ui.label(RichText::new("None at this scale.").size(theme.font_size_small).color(theme.text_muted()));
            } else {
                for (label, n) in &kit {
                    widgets::detail_row(ui, theme, label, &n.to_string());
                }
            }
        });
        ui.add_space(theme.spacing_sm);

        // ── Loop closure (the self-sufficiency demonstration, v0.432) ──
        // The four coupled loops with whether each closes, from data/machines/home.ron.
        if !loops.is_empty() {
            let closed = loops.iter().filter(|l| l.closes).count();
            widgets::card(ui, theme, |ui| {
                ui.label(RichText::new("Closed-loop self-sufficiency").size(theme.font_size_body).strong().color(theme.text_primary()));
                ui.label(
                    RichText::new(format!(
                        "{}/{} loops close. You are only as self-sufficient as your weakest loop.",
                        closed, loops.len()
                    ))
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
                );
                ui.add_space(theme.spacing_xs);
                for l in loops {
                    let (mark, mark_color) = if l.closes {
                        ("closed", theme.success())
                    } else {
                        ("short", theme.danger())
                    };
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&l.name).size(theme.font_size_small).strong().color(theme.text_primary()));
                        ui.label(RichText::new(mark).size(theme.font_size_small).strong().color(mark_color));
                        if l.weakest {
                            ui.label(RichText::new("weakest loop").size(theme.font_size_small).color(theme.warning()));
                        }
                    });
                    widgets::detail_row(ui, theme, "  demand", &l.demand);
                    widgets::detail_row(ui, theme, "  supply", &l.supply);
                    ui.label(RichText::new(&l.note).size(theme.font_size_small).color(theme.text_secondary()));
                    ui.add_space(theme.spacing_xs);
                }
            });
            ui.add_space(theme.spacing_sm);
        }

        // ── The ideal closed loop (the requirements outline) ──
        // Operator 2026-07-05: Home outlines the perfect ideal closed-loop
        // homestead; the outline IS the game's requirements list for Home.
        // Data-driven from data/home_outline.json; web home.html renders the
        // same file (web mirrors native).
        let outline = home_outline();
        if !outline.loops.is_empty() {
            widgets::card(ui, theme, |ui| {
                ui.label(
                    RichText::new(&outline.title)
                        .size(theme.font_size_body)
                        .strong()
                        .color(theme.text_primary()),
                );
                if !outline.subtitle.is_empty() {
                    ui.label(RichText::new(&outline.subtitle).size(theme.font_size_small).color(theme.text_muted()));
                }
                if !outline.intro.is_empty() {
                    ui.add_space(theme.spacing_xs);
                    ui.label(RichText::new(&outline.intro).size(theme.font_size_small).color(theme.text_secondary()));
                }
                ui.add_space(theme.spacing_xs);
                // Fully exposed, no expand/collapse (operator 2026-07-05:
                // "I'd rather have all the info immediately exposed since
                // there's not terribly much").
                for l in &outline.loops {
                    let closes_mark = if l.closes { "closes" } else { "open" };
                    let mark_color = if l.closes { theme.success() } else { theme.warning() };
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(&l.name)
                                .size(theme.font_size_body)
                                .strong()
                                .color(theme.text_primary()),
                        );
                        ui.label(
                            RichText::new(closes_mark)
                                .size(theme.font_size_small)
                                .strong()
                                .color(mark_color),
                        );
                    });
                    widgets::detail_row(ui, theme, "demand", &l.demand);
                    ui.label(
                        RichText::new(&l.closure_note)
                            .size(theme.font_size_small)
                            .color(theme.text_secondary()),
                    );
                    ui.add_space(theme.spacing_xs);
                    // Two tiers per loop (operator 2026-07-05): the bare
                    // minimum first, then the life-of-luxury additions.
                    // Empty game_id = "not in game yet" (the honest gap flag).
                    for (tier_key, tier_label) in
                        [("baseline", "Bare minimum"), ("luxury", "Life of luxury")]
                    {
                        let rows: Vec<&OutlineRequirement> = l
                            .requirements
                            .iter()
                            .filter(|r| {
                                r.tier == tier_key
                                    || (tier_key == "baseline" && r.tier.is_empty())
                            })
                            .collect();
                        if rows.is_empty() {
                            continue;
                        }
                        ui.label(
                            RichText::new(tier_label)
                                .size(theme.font_size_small)
                                .strong()
                                .color(if tier_key == "luxury" {
                                    theme.info()
                                } else {
                                    theme.text_primary()
                                }),
                        );
                        for r in rows {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(
                                    RichText::new(format!("{} {}", r.qty, r.item))
                                        .size(theme.font_size_small)
                                        .strong()
                                        .color(theme.text_primary()),
                                );
                                if r.game_id.is_empty() {
                                    ui.label(
                                        RichText::new("not in game yet")
                                            .size(theme.font_size_small)
                                            .strong()
                                            .color(theme.warning()),
                                    );
                                }
                                ui.label(
                                    RichText::new(&r.why)
                                        .size(theme.font_size_small)
                                        .color(theme.text_muted()),
                                );
                            });
                        }
                        ui.add_space(theme.spacing_xs);
                    }
                    ui.separator();
                    ui.add_space(theme.spacing_xs);
                }
                if !outline.in_game_next.is_empty() {
                    ui.add_space(theme.spacing_xs);
                    ui.label(
                        RichText::new("What the game still needs for Home")
                            .size(theme.font_size_small)
                            .strong()
                            .color(theme.text_primary()),
                    );
                    for step in &outline.in_game_next {
                        ui.label(
                            RichText::new(format!("- {step}"))
                                .size(theme.font_size_small)
                                .color(theme.text_secondary()),
                        );
                    }
                }
                if !outline.footer.is_empty() {
                    ui.add_space(theme.spacing_xs);
                    ui.label(RichText::new(&outline.footer).size(theme.font_size_small).italics().color(theme.text_muted()));
                }
            });
            ui.add_space(theme.spacing_sm);
        }

        // ── What one home cannot close (the pedagogical payoff) ──
        // The survival loops above close; these five gaps do NOT, by the design of
        // reality: the game's own recipes (manufacture_cpu, smelt_steel,
        // craft_antibiotics, ...) abstract away industrial infrastructure no single
        // homestead can carry. Marked externally-sourced/traded, in a deliberately
        // muted OUTLINED treatment (warning stroke on the panel background) so it
        // reads clearly apart from the green closed-loop rows above. Data:
        // data/self_sufficiency/cannot_close.ron (homestead-solo-design.md section 8).
        let cc = cannot_close();
        if !cc.entries.is_empty() {
            egui::Frame::none()
                .fill(theme.bg_panel())
                .rounding(egui::Rounding::same(theme.border_radius as u8))
                .inner_margin(theme.card_padding)
                .stroke(egui::Stroke::new(1.0, theme.warning()))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("What one home cannot close")
                                .size(theme.font_size_body)
                                .strong()
                                .color(theme.warning()),
                        );
                        ui.label(
                            RichText::new(format!("{} traded loops", cc.entries.len()))
                                .size(theme.font_size_small)
                                .color(theme.text_muted()),
                        );
                    });
                    if !cc.intro.is_empty() {
                        ui.label(RichText::new(&cc.intro).size(theme.font_size_small).color(theme.text_muted()));
                    }
                    ui.add_space(theme.spacing_xs);
                    // Fully exposed, no expand/collapse (operator 2026-07-05:
                    // all Home info immediately visible).
                    for entry in &cc.entries {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(&entry.title)
                                    .size(theme.font_size_small)
                                    .strong()
                                    .color(theme.text_primary()),
                            );
                            ui.label(
                                RichText::new("traded")
                                    .size(theme.font_size_small)
                                    .strong()
                                    .color(theme.warning()),
                            );
                        });
                        ui.label(
                            RichText::new(&entry.body)
                                .size(theme.font_size_small)
                                .color(theme.text_secondary()),
                        );
                        ui.label(
                            RichText::new(&entry.provided_by)
                                .size(theme.font_size_small)
                                .italics()
                                .color(theme.text_muted()),
                        );
                        ui.add_space(theme.spacing_xs);
                    }
                    if !cc.footer.is_empty() {
                        ui.add_space(theme.spacing_xs);
                        ui.label(
                            RichText::new(&cc.footer)
                                .size(theme.font_size_small)
                                .italics()
                                .color(theme.text_secondary()),
                        );
                    }
                });
            ui.add_space(theme.spacing_sm);
        }

        // ── Bill of materials ──
        egui::CollapsingHeader::new(
            RichText::new(format!("Bill of materials ({} kinds, {} total)", bom.len(), total_parts))
                .size(theme.font_size_body)
                .strong()
                .color(theme.text_primary()),
        )
        .id_salt("homes_bom")
        .default_open(true)
        .show(ui, |ui| {
            ui.label(
                RichText::new("Everything to build it. Real-world part numbers and where to 3D-print / buy / trade each one are the planned next layer.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_xs);
            for (id, qty) in &bom {
                widgets::detail_row(ui, theme, &humanize(id), &format!("x{}", qty));
            }
        });
        ui.add_space(theme.spacing_sm);

        // ── Rooms by tier ──
        ui.label(RichText::new("Rooms").size(theme.font_size_heading).color(theme.text_primary()));
        ui.add_space(theme.spacing_xs);
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
                    ui.label(RichText::new(&tier.description).size(theme.font_size_small).color(theme.text_muted()));
                    ui.add_space(theme.spacing_xs);
                }
                for r in tier_rooms {
                    draw_room(ui, theme, r);
                }
            });
        }

        // ── Aeroponic towers (the food loop; v0.382) ──
        if !towers.is_empty() {
            ui.add_space(theme.spacing_md);
            ui.separator();
            ui.add_space(theme.spacing_sm);
            ui.label(RichText::new("Aeroponic towers").size(theme.font_size_heading).color(theme.text_primary()));
            ui.label(
                RichText::new("Your homestead's food loop: two curated 50-slot vertical towers.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_xs);
            for (i, tower) in towers.iter().enumerate() {
                draw_tower(ui, theme, tower, compat.get(i));
            }
        }
    });
}

/// One aeroponic tower, collapsible: purpose + what it covers / its gaps +
/// disclaimer + the 50-slot planting list (count, plant, role, note).
fn draw_tower(ui: &mut egui::Ui, theme: &Theme, tower: &TowerConfig, compat: Option<&TowerCompat>) {
    let planted: u32 = tower.plantings.iter().map(|p| p.slots).sum();
    egui::CollapsingHeader::new(
        RichText::new(format!("{}  ({}/{} slots)", tower.name, planted, tower.slots))
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
                RichText::new(format!("Covers: {}", tower.covers.join(", ")))
                    .size(theme.font_size_small)
                    .color(theme.success()),
            );
        }
        if !tower.gaps.is_empty() {
            ui.label(
                RichText::new(format!("Gaps: {}", tower.gaps.join(", ")))
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
        // reservoir + air, not soil, so the constraint is a common pH/temp/humidity
        // window. Green = one shared window; warnings name the plants that conflict.
        if let Some(c) = compat {
            ui.add_space(theme.spacing_xs);
            let shared: Vec<String> = [
                c.ph.map(|(a, b)| format!("pH {:.1}-{:.1}", a, b)),
                c.temp.map(|(a, b)| format!("{:.0}-{:.0}°C", a, b)),
                c.humidity.map(|(a, b)| format!("humidity {:.0}-{:.0}%", a * 100.0, b * 100.0)),
            ]
            .into_iter()
            .flatten()
            .collect();
            if c.conflicts.is_empty() {
                ui.label(
                    RichText::new(format!(
                        "✓ These {} plants share one reservoir: {}",
                        c.species,
                        shared.join(", ")
                    ))
                    .size(theme.font_size_small)
                    .color(theme.success()),
                );
            } else {
                if !shared.is_empty() {
                    ui.label(
                        RichText::new(format!("Shared where they can: {}", shared.join(", ")))
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
                ui.label(
                    RichText::new(
                        "Split the flagged plants into a separate tower or climate zone.",
                    )
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
                );
            }
            // Self-sufficiency numbers: total daily water draw + the harvest window
            // (ties the tower into the homestead's water + food loops).
            let mut stats: Vec<String> = Vec::new();
            if c.water_per_day_total > 0.0 {
                stats.push(format!("Water draw ~{:.1} L/day", c.water_per_day_total));
            }
            if c.full_harvest_days > 0.0 {
                stats.push(format!(
                    "Harvest ~{:.0}-{:.0} days",
                    c.first_harvest_days, c.full_harvest_days
                ));
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
            ui.label(
                RichText::new("Parts list (real-world build)")
                    .size(theme.font_size_body)
                    .strong()
                    .color(theme.text_primary()),
            );
            ui.label(
                RichText::new(
                    "What to 3D-print, buy, trade, or scavenge to build it. A starting list, refine for your setup.",
                )
                .size(theme.font_size_small)
                .color(theme.text_muted()),
            );
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
    ui.add_space(theme.spacing_xs);
}

/// Humanize a part `source` tag for display ("3d_print" -> "3D-print").
fn part_source_label(source: &str) -> &str {
    match source {
        "3d_print" => "3D-print",
        "diy" => "DIY",
        other => other,
    }
}

fn draw_room(ui: &mut egui::Ui, theme: &Theme, r: &DesignRoom) {
    egui::CollapsingHeader::new(
        RichText::new(format!("F{}  {}", r.fibonacci_index, r.name)).color(theme.text_primary()),
    )
    .id_salt(("homes_room", r.id.as_str()))
    .show(ui, |ui| {
        if !r.purpose.is_empty() {
            ui.label(RichText::new(&r.purpose).size(theme.font_size_small).color(theme.text_secondary()));
        }
        widgets::detail_row(ui, theme, "Footprint", &format!("{:.0} x {:.0} m", r.size.x, r.size.z));
        widgets::detail_row(ui, theme, "Power", &format!("{} W", r.requirements.power_watts));
        widgets::detail_row(ui, theme, "Water", &format!("{} L/day", r.requirements.water_liters_per_day));
        if !r.environment_notes.is_empty() {
            ui.add_space(theme.spacing_xs);
            ui.label(RichText::new(&r.environment_notes).size(theme.font_size_small).color(theme.text_muted()));
        }
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new("Parts").size(theme.font_size_small).color(theme.text_secondary()));
        for (id, qty) in &r.requirements.materials {
            widgets::detail_row(ui, theme, &humanize(id), &format!("x{}", qty));
        }
    });
}

/// Sum the generation / collection / recycling components in the BOM by group, so
/// the page can show, at a glance, the systems that close the homestead's loops.
fn self_sufficiency_kit(bom: &[(String, u32)]) -> Vec<(&'static str, u32)> {
    let groups: [(&str, &[&str]); 5] = [
        ("Solar panels", &["solar_panel"]),
        ("Wind turbines", &["wind_turbine"]),
        ("Generators + batteries", &["generator", "battery"]),
        ("Water (tank / pump / purifier / irrigation)", &["water_tank", "water_pump", "water_purifier", "irrigation"]),
        ("Food + recycling (greenhouse / composter)", &["greenhouse", "composter"]),
    ];
    let mut out = Vec::new();
    for (label, subs) in groups {
        let n: u32 = bom
            .iter()
            .filter(|(id, _)| subs.iter().any(|s| id.contains(*s)))
            .map(|(_, q)| *q)
            .sum();
        if n > 0 {
            out.push((label, n));
        }
    }
    out
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
