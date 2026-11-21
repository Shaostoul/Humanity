//! THE COOKING CARD on the Crafting page (cooking rung 1, 2026-11-21; docs/design/cooking.md).
//!
//! For a recipe that is a dish (data/food_system.ron `dishes`): its method, and, by the level of
//! Settings > Gameplay > Cooking, either the heat and time the station picks (Simplified) or the
//! cook's own choice of heat and time with the temperature inside the food (Guided always,
//! Realistic only with a food thermometer in the backpack: the gear-gated rule's plain toggle is
//! the Guided level). The model is the system's own (`humanity_systems::cooking`); this card only
//! asks it, so the preview and the craft cannot disagree. The choice rides beside the craft
//! request (`GuiState::pending_cook_choice`) and the crafting system takes it when the craft starts.
//!
//! Nothing runs while the card is not drawn; drawn, it evaluates one closed form per frame.

use std::cell::RefCell;
use std::collections::HashMap;

use egui::RichText;
use humanity_core::i18n::{fill, t, t_dyn};
use humanity_systems::cooking::{self, Cooking, Doneness, Heat, Mode};

use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Hint};
use crate::gui::{GuiRecipe, GuiState};

/// The cooking tables, read once from food_system.ron the way `food::consume_kinds` reads the
/// food list (disk first, the built-in copy otherwise). Data that cannot change while the game
/// runs a session, as the item list the Eat button reads.
fn tables() -> &'static Cooking {
    static T: std::sync::OnceLock<Cooking> = std::sync::OnceLock::new();
    T.get_or_init(|| Cooking::load(&humanity_core::storage::data_dir()))
}

thread_local! {
    /// The heat and minutes the cook has set on each dish's card this session.
    static CHOSEN: RefCell<HashMap<String, (Heat, f32)>> = RefCell::new(HashMap::new());
}

/// The cooking level the person chose in Settings > Gameplay.
pub(crate) fn mode(state: &GuiState) -> Mode {
    Mode::from_level(state.settings.level("cooking")).unwrap_or_default()
}

/// The heat and time to send with a craft of `recipe_id`: the card's setting when the level lets
/// the cook choose, None otherwise (the station picks).
pub(crate) fn choice_for(state: &GuiState, recipe_id: &str) -> Option<cooking::Choice> {
    if !mode(state).chooses() {
        return None;
    }
    let dish = tables().dish(recipe_id)?;
    let (heat, minutes) = CHOSEN.with(|c| c.borrow().get(recipe_id).copied()).or_else(|| tables().sensible(dish))?;
    Some(cooking::Choice { recipe: recipe_id.to_string(), heat, minutes })
}

/// Set the card's heat and time for `recipe_id`, as the cook's clicks would (a snapshot's state).
#[cfg(test)]
pub(crate) fn set_choice(recipe_id: &str, heat: Heat, minutes: f32) {
    CHOSEN.with(|c| c.borrow_mut().insert(recipe_id.to_string(), (heat, minutes)));
}

/// A cooked dish's state in words for the inventory card ("Done, safe to eat; meal: Good"), None
/// for food not cooked by hand and for everything that is not a dish.
pub(crate) fn dish_words(item_id: &str, quality: u8) -> Option<String> {
    let c = tables();
    c.dish_for_item(item_id)?;
    let s = cooking::DishState::from_byte(quality)?;
    let safety = if s.safe { t("cooking.state_safe") } else { t("cooking.state_unsafe") };
    let meal = c.meals.get(usize::from(s.meal)).map(|m| t_dyn(&format!("cooking.meal_{}", m.id))).unwrap_or("?");
    Some(fill(t("cooking.state_line"), &[("doneness", &t(s.doneness.key())), ("safety", &safety), ("meal", &meal)]))
}

fn heat_key(h: Heat) -> &'static str {
    match h {
        Heat::Low => "cooking.heat_low",
        Heat::Medium => "cooking.heat_medium",
        Heat::High => "cooking.heat_high",
    }
}

/// The card, for a recipe that is a dish; nothing for any other recipe.
pub(crate) fn draw(ui: &mut egui::Ui, theme: &Theme, state: &GuiState, recipe: &GuiRecipe) {
    let c = tables();
    let Some(dish) = c.dish(&recipe.id) else { return };
    let (Some((method, class)), Some((pick_heat, pick_min))) = (c.plan(dish), c.sensible(dish)) else { return };
    let mode = mode(state);
    let small = |s: String, col| RichText::new(s).size(theme.font_size_small).color(col);
    ui.add_space(theme.spacing_sm);
    widgets::card_with_header(ui, theme, t("cooking.card_title"), |ui| {
        ui.label(small(
            fill(t("cooking.method_line"), &[("method", &t_dyn(&format!("cooking.method_{}", method.id)))]),
            theme.text_primary(),
        ));
        // The safety line, worded as the How-to page words a hazard.
        if let Some(safe) = &class.safe {
            let line = match (safe.say.as_deref(), safe.core_c, safe.boil_min) {
                // The class's own words where the general ones would say more than the source.
                (Some(key), _, _) => t_dyn(key).to_string(),
                (None, Some(core), _) => fill(t("cooking.safe_core"), &[("c", &format!("{core:.0}")), ("f", &format!("{:.0}", core * 9.0 / 5.0 + 32.0))]),
                (None, None, Some(min)) => fill(t("cooking.safe_boil"), &[("n", &format!("{min:.0}"))]),
                (None, None, None) => String::new(),
            };
            ui.label(small(fill(t("cooking.hazard_serious"), &[("line", &line)]), theme.warning()));
        }
        let (heat, minutes) = if mode.chooses() {
            CHOSEN.with(|m| m.borrow().get(&recipe.id).copied()).unwrap_or((pick_heat, pick_min))
        } else {
            (pick_heat, pick_min)
        };
        if !mode.chooses() {
            ui.label(small(
                fill(t("cooking.station_picks"), &[("heat", &t(heat_key(heat))), ("n", &format!("{minutes:.0}"))]),
                theme.text_secondary(),
            ));
            return;
        }
        // The cook's own heat and time.
        let mut new_heat = heat;
        ui.horizontal(|ui| {
            ui.label(small(t("cooking.heat_label").to_string(), theme.text_muted()));
            for h in Heat::ALL {
                let hint = match h {
                    Heat::Low => "cooking.heat_low_hint",
                    Heat::Medium => "cooking.heat_medium_hint",
                    Heat::High => "cooking.heat_high_hint",
                };
                if widgets::Button::secondary(t(heat_key(h))).active(h == heat).hint(hint).show(ui, theme) {
                    new_heat = h;
                }
            }
        });
        let mut new_min = minutes;
        let top = (pick_min * 3.0).max(10.0).ceil();
        ui.horizontal(|ui| {
            ui.label(small(t("cooking.time_label").to_string(), theme.text_muted()));
            widgets::slider(ui, theme, egui::Slider::new(&mut new_min, 1.0..=top).step_by(1.0).suffix(t("cooking.minutes_suffix")))
                .hint("cooking.time_hint");
        });
        if new_heat != heat || new_min != minutes {
            CHOSEN.with(|m| m.borrow_mut().insert(recipe.id.clone(), (new_heat, new_min)));
        }
        let Some(out) = c.outcome(dish, new_heat, new_min) else { return };
        let has_thermometer = state.inventory_items.iter().flatten().any(|i| i.item_id == cooking::THERMOMETER);
        if mode == Mode::Guided || has_thermometer {
            let reading = fill(t("cooking.reading"), &[("c", &format!("{:.0}", out.core_c)), ("f", &format!("{:.0}", out.core_c * 9.0 / 5.0 + 32.0))]);
            ui.label(small(reading, theme.text_primary()));
            if mode == Mode::Guided {
                // Guided also says how it will come out, in the colour of the outcome.
                let col = match (out.doneness, out.safe) {
                    (Doneness::Done, true) => theme.success(),
                    (Doneness::Overcooked, true) => theme.warning(),
                    _ => theme.danger(),
                };
                let safety = if out.safe { t("cooking.state_safe") } else { t("cooking.state_unsafe") };
                ui.label(small(fill(t("cooking.will_come_out"), &[("doneness", &t(out.doneness.key())), ("safety", &safety)]), col));
            }
        } else {
            ui.label(small(t("cooking.no_thermometer").to_string(), theme.text_muted()));
        }
        ui.label(small(fill(t("cooking.station_would"), &[("heat", &t(heat_key(pick_heat))), ("n", &format!("{pick_min:.0}"))]), theme.text_muted()));
    });
}
