//! Home > Animals (2026-10-12, first-hour audit S6): every farm animal the home keeps, how it
//! is (fed, watered, hungry, sick) and what it needs a day, in plain words. The rows come from
//! `systems::animal_care::herd_rows`, which lib.rs puts in `GuiState::herd` each frame; the
//! card is hidden when the home keeps no animals that need care.
//!
//! Two ways to see them (operator, 2026-10-30: "a multiview list ... Like an excel spreadsheet
//! for compact detail and tiles/cards for each animal in a more visually friendly style"): a card
//! each, with its kind's tile, a pill for how it is and what it makes, or a table, one row each,
//! its facts in columns. `widgets::view_toggle` switches them and keeps the choice while the app
//! runs; the cards are `widgets::cards::Grid`, the table `widgets::data_table`.

use egui::{Align, Layout, Pos2, RichText, Sense, Ui, Vec2};

use crate::gui::i18n::{t, tf, tn};
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, cards, data_table, view_toggle};
use crate::gui::GuiState;
use crate::systems::animal_care::HerdRow;

/// Where the Animals card keeps its view (a snapshot sets it to show the table).
pub const VIEW_ID: &str = "homes_herd_view";

/// The Animals card, on the Home page under the Household card (homes.rs `draw`).
pub fn draw_card(ui: &mut Ui, theme: &Theme, state: &mut GuiState) {
    if state.herd.is_empty() {
        return;
    }
    let rows = &state.herd;
    widgets::card(ui, theme, |ui| {
        ui.set_min_width(ui.available_width());
        let mut open = false;
        let mut view = view_toggle::ListView::Cards;
        ui.horizontal(|ui| {
            open = widgets::section_disclosure(ui, theme, ("homes_sec", "animals"), t("home.animals"), None);
            if open {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    view = view_toggle::remembered(ui, theme, VIEW_ID, view_toggle::ListView::Cards);
                });
            }
        });
        if !open {
            return;
        }
        ui.scope(|ui| {
            ui.set_max_width(cards::prose_width(ui, theme));
            widgets::body_hint(ui, theme, t("home.animals_intro"));
        });
        ui.add_space(theme.spacing_xs);
        summary(ui, theme, rows);
        ui.add_space(theme.spacing_sm);
        match view {
            view_toggle::ListView::Cards => {
                cards::Grid::new("homes_herd_cards").min_width(250.0).show(ui, theme, rows.len(), |ui, i| animal_card(ui, theme, &rows[i]));
            }
            view_toggle::ListView::Table => table(ui, theme, rows),
        }
    });
    ui.add_space(theme.spacing_sm);
}

/// How it is, as a word and a colour: doing well, needs care, or dead.
fn state_of(theme: &Theme, row: &HerdRow) -> (&'static str, egui::Color32) {
    if row.dead {
        (t("home.animal_dead"), theme.danger())
    } else if row.alert {
        (t("home.animal_needs_care"), theme.warning())
    } else {
        (t("home.animal_well"), theme.success())
    }
}

/// The counts at a glance: how many animals, how many need care, how many have something ready.
fn summary(ui: &mut Ui, theme: &Theme, rows: &[HerdRow]) {
    let alive = rows.iter().filter(|r| !r.dead).count();
    let care = rows.iter().filter(|r| r.alert && !r.dead).count();
    let ready = rows.iter().filter(|r| r.ready).count();
    let lost = rows.len() - alive;
    ui.horizontal_wrapped(|ui| {
        cards::plain_badge(ui, theme, &tn("home.animals_count", alive as i64, &[]));
        if care > 0 {
            cards::status_badge(ui, theme, &tn("home.animals_need_care", care as i64, &[]), theme.warning());
        }
        if ready > 0 {
            cards::status_badge(ui, theme, &tn("home.animals_ready", ready as i64, &[]), theme.success());
        }
        if lost > 0 {
            cards::status_badge(ui, theme, &tn("home.animals_lost", lost as i64, &[]), theme.danger());
        }
    });
}

/// A tile in the kind's own colour with its first letter: chickens look alike, goats alike.
fn kind_tile(ui: &mut Ui, theme: &Theme, kind: &str, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let fill = theme.bg_tertiary().lerp_to_gamma(widgets::swatch_color(kind), 0.45);
    let corners = egui::CornerRadius::same(theme.border_radius_lg.round().clamp(0.0, 255.0) as u8);
    ui.painter().rect_filled(rect, corners, fill);
    let letter: String = kind.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
    let galley = ui.painter().layout_no_wrap(letter, egui::FontId::proportional(size * 0.5), theme.text_strong());
    let at = Pos2::new(rect.center().x - galley.size().x / 2.0, rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(at, galley, theme.text_strong());
}

/// One animal's card: its tile, name and kind, a pill for how it is, then what it makes and what
/// it eats and drinks a day.
fn animal_card(ui: &mut Ui, theme: &Theme, row: &HerdRow) {
    let tile = theme.font_size_title + theme.spacing_md;
    ui.horizontal(|ui| {
        kind_tile(ui, theme, &row.kind, tile);
        ui.vertical(|ui| {
            ui.label(RichText::new(&row.title).size(theme.font_size_body).strong().color(theme.text_primary()));
            ui.label(RichText::new(&row.kind).size(theme.font_size_small).color(theme.text_muted()));
        });
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            let (word, colour) = state_of(theme, row);
            cards::status_badge(ui, theme, word, colour);
        });
    });
    ui.add_space(theme.spacing_xs);
    let condition_colour = if row.alert { theme.warning() } else { theme.text_secondary() };
    ui.label(RichText::new(&row.condition).size(theme.font_size_small).color(condition_colour));
    if !row.product.is_empty() {
        let colour = if row.ready { theme.success() } else { theme.text_primary() };
        ui.label(RichText::new(&row.product).size(theme.font_size_small).strong().color(colour));
    }
    if !row.feed.is_empty() || !row.water.is_empty() {
        ui.add_space(theme.spacing_xs);
        let y = ui.cursor().min.y;
        ui.painter().hline(ui.max_rect().x_range(), y, egui::Stroke::new(1.0, theme.border()));
        ui.add_space(theme.spacing_xs);
    }
    if !row.feed.is_empty() {
        ui.label(RichText::new(tf("home.animal_eats", &[("feed", &row.feed)])).size(theme.font_size_small).color(theme.text_secondary()));
    }
    if !row.water.is_empty() {
        ui.label(RichText::new(tf("home.animal_drinks", &[("water", &row.water)])).size(theme.font_size_small).color(theme.text_secondary()));
    }
}

/// The table: one row per animal, its facts in columns.
fn table(ui: &mut Ui, theme: &Theme, rows: &[HerdRow]) {
    let columns = [
        data_table::Column::new(t("home.col_name"), 1.0, 90.0),
        data_table::Column::new(t("home.col_kind"), 0.6, 70.0),
        data_table::Column::new(t("home.col_state"), 2.4, 150.0),
        data_table::Column::new(t("home.col_product"), 1.8, 130.0),
        data_table::Column::new(t("home.col_feed"), 1.4, 120.0),
        data_table::Column::new(t("home.col_water"), 0.6, 70.0),
    ];
    data_table::show(ui, theme, "homes_herd_table", &columns, rows.len(), |ui, r, c| {
        let row = &rows[r];
        match c {
            0 => data_table::text_cell(ui, theme, &row.title, theme.text_primary(), true),
            1 => data_table::text_cell(ui, theme, &row.kind, theme.text_secondary(), false),
            2 => {
                data_table::dot(ui, theme, state_of(theme, row).1);
                let colour = if row.alert { theme.warning() } else { theme.text_primary() };
                data_table::text_cell(ui, theme, &row.condition, colour, false);
            }
            3 => {
                let colour = if row.ready { theme.success() } else { theme.text_primary() };
                data_table::text_cell(ui, theme, &row.product, colour, row.ready);
            }
            4 => data_table::text_cell(ui, theme, &row.feed, theme.text_secondary(), false),
            _ => data_table::text_cell(ui, theme, &row.water, theme.text_secondary(), false),
        }
    });
}
