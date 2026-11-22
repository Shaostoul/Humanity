//! THE PRESERVING CARD on the Crafting page (preserving rung 1, 2026-11-21; docs/design/preserving.md).
//!
//! For a recipe that is a preserving procedure (data/food_system.ron `preserving.procedures`): what
//! the method is and how dry or how processed the food must be, the tested canning table as the
//! guide prints it (every altitude band and jar size, because the right number depends on both), and,
//! by the level of Settings > Gameplay > Preserving, either the station's pick (Simplified) or the
//! person's own method, time, weight and venting (Guided says how it will come out before the start;
//! Realistic judges nothing until it comes out). The model is the system's own
//! (`humanity_systems::preserving`); this card only asks it, so the preview and the craft cannot
//! disagree. The choice is handed to the crafting system beside the craft request
//! (`take_pending`, read by engine::frame_sim_requests).
//!
//! Nothing runs while the card is not drawn; drawn, it evaluates one closed form per frame.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Mutex;

use egui::RichText;
use humanity_core::i18n::{fill, t, t_dyn, t_marked};
use humanity_systems::preserving::{Band, Choice, Kind, Mode, PreserveState, Preserving, Procedure};

use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Hint};
use crate::gui::{GuiRecipe, GuiState};

/// The preserving tables, read once from food_system.ron (disk first, the built-in copy otherwise),
/// as the cooking card reads its own.
fn tables() -> &'static Preserving {
    static T: std::sync::OnceLock<Preserving> = std::sync::OnceLock::new();
    T.get_or_init(|| Preserving::load(&humanity_core::storage::data_dir()))
}

thread_local! {
    /// The method, time, weight and venting the person has set on each procedure's card this session.
    static CHOSEN: RefCell<HashMap<String, Choice>> = RefCell::new(HashMap::new());
}

/// The choice going with the craft just pressed, until the frame's request bridge takes it.
static PENDING: Mutex<Option<Choice>> = Mutex::new(None);

/// The preserving level the person chose in Settings > Gameplay.
pub(crate) fn mode(state: &GuiState) -> Mode {
    Mode::from_level(state.settings.level("preserving")).unwrap_or_default()
}

/// The card's choice for `recipe_id`, the station's pick until the person changes it.
fn chosen(p: &Procedure) -> Option<Choice> {
    CHOSEN.with(|c| c.borrow().get(&p.recipe).cloned()).or_else(|| tables().sensible(p))
}

/// The Craft button pressed on `recipe_id`: its card's choice goes with it when the level lets the
/// person choose (none otherwise: the station picks).
pub(crate) fn send_choice(state: &GuiState, recipe_id: &str) {
    let choice = if mode(state).chooses() { tables().procedure(recipe_id).and_then(chosen) } else { None };
    if let Ok(mut p) = PENDING.lock() {
        *p = choice;
    }
}

/// The choice for the craft just pressed, taken once (engine::frame_sim_requests).
pub(crate) fn take_pending() -> Option<Choice> {
    PENDING.lock().ok().and_then(|mut p| p.take())
}

/// Set the card's choice, as the person's clicks would (a snapshot's state).
#[cfg(test)]
pub(crate) fn set_choice(choice: Choice) {
    CHOSEN.with(|c| c.borrow_mut().insert(choice.recipe.clone(), choice));
}

/// A preserved food's state in words for the inventory card, and whether it is a botulism jar
/// (which the card says more about). None for food not preserved by hand.
pub(crate) fn state_words(item_id: &str, quality: u8) -> Option<(String, bool)> {
    tables().procedure_for_item(item_id)?;
    let s = PreserveState::from_byte(quality)?;
    Some((t(s.key()).to_string(), s == PreserveState::Botulism))
}

/// A table's bands in words: "15 min up to 1,000 ft, 20 min up to 3,000 ft, 25 min above".
fn bands(list: &[Band], unit: &str) -> String {
    let one = list.len() == 1;
    list.iter()
        .map(|b| {
            let v = fill(t_dyn(unit), &[("n", &format!("{:.0}", b.value))]);
            match b.up_to_ft {
                _ if one && b.up_to_ft.is_none() => fill(t("preserving.band_all"), &[("v", &v)]),
                Some(ft) => fill(t("preserving.band_upto"), &[("v", &v), ("ft", &thousands(ft))]),
                None => fill(t("preserving.band_above"), &[("v", &v)]),
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// 1000 -> "1,000".
fn thousands(ft: f32) -> String {
    let n = ft.round() as i64;
    if n >= 1000 {
        format!("{},{:03}", n / 1000, n % 1000)
    } else {
        n.to_string()
    }
}

/// A method button's hint key.
fn method_hint(id: &str) -> &'static str {
    match id {
        "oven" => "preserving.method_oven_hint",
        "sun" => "preserving.method_sun_hint",
        "boiling_water" => "preserving.canner_boiling_water_hint",
        _ => "preserving.canner_pressure_weighted_hint",
    }
}

fn method_key(id: &str) -> String {
    match id {
        "oven" | "sun" => format!("preserving.method_{id}"),
        other => format!("preserving.canner_{other}"),
    }
}

/// The card, for a recipe that is a preserving procedure; nothing for any other recipe.
pub(crate) fn draw(ui: &mut egui::Ui, theme: &Theme, state: &GuiState, recipe: &GuiRecipe) {
    let pr = tables();
    let Some(p) = pr.procedure(&recipe.id) else { return };
    let Some(pick) = pr.sensible(p) else { return };
    let mode = mode(state);
    let small = |s: String, col| RichText::new(s).size(theme.font_size_small).color(col);
    ui.add_space(theme.spacing_sm);
    widgets::card_with_header(ui, theme, t("preserving.card_title"), |ui| {
        // What the procedure is and what it must reach, as the sources teach it.
        match p.kind {
            Kind::Dry => {
                ui.label(small(t("preserving.method_dry").to_string(), theme.text_primary()));
                let until = if p.class.as_deref() == Some("fruit") { "preserving.dry_until_fruit" } else { "preserving.dry_until_vegetables" };
                ui.label(small(t(until).to_string(), theme.text_secondary()));
                ui.label(small(t("preserving.sun_rule").to_string(), theme.text_muted()));
            }
            Kind::Can => {
                let key = if p.acid { "preserving.method_can_acid" } else { "preserving.method_can_low" };
                ui.label(small(t(key).to_string(), theme.text_primary()));
                if !p.acid {
                    widgets::glossary_words(ui, theme, t_marked("preserving.hazard_lethal"), theme.font_size_small, theme.danger());
                }
                ui.label(small(t("preserving.tested_header").to_string(), theme.text_secondary()));
                for tested in &p.tested {
                    let row = fill(
                        t_dyn(&format!("preserving.row_{}", tested.canner)),
                        &[
                            ("pints", &bands(&tested.pints, "preserving.minutes")),
                            ("quarts", &bands(&tested.quarts, "preserving.minutes")),
                            ("psi", &bands(&tested.psi, "preserving.pounds")),
                        ],
                    );
                    ui.label(small(row, theme.text_secondary()));
                }
                if p.tested.iter().any(|t| t.canner.starts_with("pressure")) {
                    ui.label(small(t("preserving.pressure_rule").to_string(), theme.text_muted()));
                }
                ui.label(small(t("preserving.half_pints").to_string(), theme.text_muted()));
            }
        }
        if !mode.chooses() {
            let line = match p.kind {
                Kind::Dry => fill(t("preserving.station_picks_dry"), &[("method", &t_dyn(&method_key(&pick.method))), ("n", &format!("{:.0}", pick.time))]),
                Kind::Can => {
                    let psi = if pick.psi > 0.0 { fill(t("preserving.psi_part"), &[("n", &format!("{:.0}", pick.psi))]) } else { String::new() };
                    fill(t("preserving.station_picks_can"), &[("method", &t_dyn(&method_key(&pick.method))), ("n", &format!("{:.0}", pick.time)), ("psi", &psi)])
                }
            };
            ui.label(small(line, theme.text_secondary()));
            return;
        }
        // The person's own choice.
        let mut c = chosen(p).unwrap_or(pick.clone());
        let before = c.clone();
        let methods: Vec<String> = match p.kind {
            Kind::Dry => pr.data.drying_methods.iter().map(|m| m.id.clone()).collect(),
            // The canners the game has: a boiling-water canner and a weighted-gauge pressure canner.
            Kind::Can => vec!["boiling_water".to_string(), "pressure_weighted".to_string()],
        };
        ui.horizontal_wrapped(|ui| {
            ui.label(small(t("preserving.method_label").to_string(), theme.text_muted()));
            for m in &methods {
                if widgets::Button::secondary(t_dyn(&method_key(m))).active(*m == c.method).hint(method_hint(m)).show(ui, theme) {
                    c.method = m.clone();
                }
            }
        });
        let (label, suffix, hint, top) = match p.kind {
            Kind::Dry => ("preserving.time_label_dry", "preserving.hours_suffix", "preserving.time_dry_hint", 96.0),
            Kind::Can => ("preserving.time_label_can", "preserving.minutes_suffix", "preserving.time_can_hint", 90.0),
        };
        ui.horizontal(|ui| {
            ui.label(small(t_dyn(label).to_string(), theme.text_muted()));
            widgets::slider(ui, theme, egui::Slider::new(&mut c.time, 1.0..=top).step_by(1.0).suffix(t_dyn(suffix))).hint(hint);
        });
        if c.method.starts_with("pressure") {
            ui.horizontal(|ui| {
                ui.label(small(t("preserving.weight_label").to_string(), theme.text_muted()));
                for w in [5.0_f32, 10.0, 15.0] {
                    let words = fill(t("preserving.pounds"), &[("n", &format!("{w:.0}"))]);
                    if widgets::Button::secondary(&words).active(c.psi == w).hint("preserving.weight_hint").show(ui, theme) {
                        c.psi = w;
                    }
                }
                if widgets::Button::secondary(t("preserving.vent_label")).active(c.vented).hint("preserving.vent_hint").show(ui, theme) {
                    c.vented = !c.vented;
                }
            });
        }
        if c != before {
            CHOSEN.with(|m| m.borrow_mut().insert(p.recipe.clone(), c.clone()));
        }
        if mode == Mode::Guided {
            let Some(out) = pr.outcome(p, &c, state.air_temp_c) else { return };
            let col = match out.state {
                PreserveState::Safe => theme.success(),
                PreserveState::Spoils => theme.warning(),
                PreserveState::Botulism => theme.danger(),
            };
            ui.label(small(fill(t("preserving.will_come_out"), &[("state", &t(out.state.key())), ("why", &t(out.reason.key()))]), col));
            if let Some(w) = out.water {
                ui.label(small(fill(t("preserving.water_now"), &[("water", &format!("{:.0}", w * 100.0))]), theme.text_secondary()));
            }
        }
    });
}
