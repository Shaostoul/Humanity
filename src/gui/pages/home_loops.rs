//! The Home page's loops as cards (operator, 2026-10-30: "the Closed-loop self-sufficiency areas
//! are brutal walls of text. We need to find some way to make the more visually friendly").
//!
//! Three groups, drawn by homes.rs `draw_design`, each a grid of cards that fills the width
//! (`widgets::cards::Grid`):
//!
//! - **Closed-loop self-sufficiency**: the home's own loops (`data/machines/home.ron`'s `loops`):
//!   its name, a pill for closed or short (and "weakest loop"), a gauge where the page knows the
//!   numbers (the food the placed crops grow against the household's need; the power meter's make
//!   against use), the need and the supply cut to two lines each, and the rest folded behind "How
//!   this is worked out".
//! - **The ideal closed loop** (`data/home_outline.json`, the same file the website's Home page
//!   draws): a card per loop with its pill, its parts count and its demand, the closure note with
//!   the arithmetic and assumptions behind it folded, and the parts list folded; or a table, one
//!   row per loop (`widgets::view_toggle`).
//! - **What one home cannot close** (`data/self_sufficiency/cannot_close.ron`): a card per traded
//!   loop, edged in the warning colour, where it really comes from, and why folded.
//!
//! Nothing is cut from the words: what a card does not show at first is a click away, and a line
//! cut to fit shows whole on hover. Every word a person reads that is not data is a key in
//! `data/i18n/en.json` (`home.*`).

use egui::{Align, Color32, Layout, RichText, Ui};

use super::homes::{CannotCloseData, HomeOutline, OutlineLoop, OutlineRequirement};
use crate::gui::i18n::{t, tf, tn};
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, cards, data_table, view_toggle};

/// Where the ideal loop's list keeps its view (a snapshot sets it to show the table).
pub const OUTLINE_VIEW_ID: &str = "homes_outline_view";

/// What a home loop has against what it needs, when the page knows both as numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gauge {
    pub have: f32,
    pub need: f32,
    /// Which words say it: the food grown, or the power made.
    pub kind: GaugeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GaugeKind {
    /// kcal a day the placed crops grow against the household's need.
    Food,
    /// kWh a day the home makes against what it uses, by the power meter.
    Power,
}

/// A loop's gauge: the food loop's grown calories against its need, or, for the loop the power
/// meter reads (`meter: Some(Power)`), what the meter says the home makes against what it uses.
/// None for a loop whose numbers are only in its words (the page never invents one).
pub fn gauge_for(l: &crate::machines::HomeLoop, grown_kcal: f32, power: Option<(f32, f32)>) -> Option<Gauge> {
    if let Some(need) = l.food_demand_kcal.filter(|d| *d > 0.0) {
        return Some(Gauge { have: grown_kcal.max(0.0), need, kind: GaugeKind::Food });
    }
    match (l.meter, power) {
        (Some(crate::machines::LoopMeter::Power), Some((make, uses))) if uses > 0.0 => Some(Gauge { have: make.max(0.0), need: uses, kind: GaugeKind::Power }),
        _ => None,
    }
}

/// The gauge's line: "Makes 11.0 of the 24.0 kWh/day it uses (46%)".
fn gauge_words(g: Gauge) -> String {
    let pct = (g.have / g.need * 100.0).round();
    match g.kind {
        GaugeKind::Food => tf("home.gauge_food", &[("have", &format!("{:.0}", g.have)), ("need", &format!("{:.0}", g.need)), ("pct", &pct)]),
        GaugeKind::Power => tf("home.gauge_power", &[("have", &format!("{:.1}", g.have)), ("need", &format!("{:.1}", g.need)), ("pct", &pct)]),
    }
}

/// A label of a fixed width beside words cut to `rows` lines: "Needs   ~24.0 kWh/day ...".
fn labelled(ui: &mut Ui, theme: &Theme, label: &str, text: &str, rows: usize) {
    let label_w = theme.font_size_small * 4.4;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let r = ui.label(RichText::new(label).size(theme.font_size_small).color(theme.text_muted()));
        ui.add_space((label_w - r.rect.width()).max(theme.spacing_xs));
        cards::clamped(ui, theme, text, theme.font_size_small, theme.text_primary(), rows);
    });
}

/// A card's top line: on the right its pills, and its name in the room left of them, wrapping
/// there rather than running under them ("Electronics and semiconductors" did, 2026-10-30).
fn title_row(ui: &mut Ui, theme: &Theme, name: &str, pills: &[(&str, Color32)]) {
    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing_xs;
        for (word, colour) in pills.iter().rev() {
            cards::status_badge(ui, theme, word, *colour);
        }
        ui.with_layout(Layout::top_down(Align::Min), |ui| {
            ui.add(egui::Label::new(RichText::new(name).size(theme.font_size_body).strong().color(theme.text_primary())).wrap());
        });
    });
}

// ── The home's own loops ────────────────────────────────────────────────────────────────────

/// The Closed-loop self-sufficiency group: its heading with how many close, then a card a loop.
/// `metered[i]` is loop i's live line (homes.rs works it out), `power` the power meter's (make,
/// use) in kWh a day when the home has one.
pub fn draw_home_loops(ui: &mut Ui, theme: &Theme, loops: &[crate::machines::HomeLoop], metered: &[Option<String>], grown_kcal: f32, power: Option<(f32, f32)>) {
    if loops.is_empty() {
        return;
    }
    let closed = loops.iter().filter(|l| l.closes_given(grown_kcal)).count();
    let summary = tf("home.loops_summary", &[("closed", &closed), ("total", &loops.len())]);
    cards::group_heading(ui, theme, t("home.loops_title"), Some(&summary));
    let face = |ui: &mut Ui, i: usize| {
        let l = &loops[i];
        let closes = l.closes_given(grown_kcal);
        let mut pills: Vec<(&str, Color32)> = vec![if closes { (t("home.loop_closed"), theme.success()) } else { (t("home.loop_short"), theme.danger()) }];
        if l.weakest {
            pills.push((t("home.loop_weakest"), theme.warning()));
        }
        title_row(ui, theme, &l.name, &pills);
        ui.add_space(theme.spacing_xs);
        if let Some(g) = gauge_for(l, grown_kcal, power) {
            let colour = if g.have >= g.need || closes { theme.success() } else { theme.warning() };
            cards::gauge(ui, theme, g.have, g.need, colour);
            ui.label(RichText::new(gauge_words(g)).size(theme.font_size_small).strong().color(theme.text_primary()));
            ui.add_space(theme.spacing_xs);
        }
        labelled(ui, theme, t("home.loop_needs"), &l.demand, 2);
        labelled(ui, theme, t("home.loop_has"), &l.supply, 2);
        ui.add_space(theme.spacing_xs);
        cards::fold(ui, theme, ("homes_loop_how", i), t("home.how_worked_out"), "home.how_worked_out_hint")
    };
    // Opened: the whole need and supply, the food grown, the running line, and the note, under the
    // loop's row at a width that reads.
    let words = |ui: &mut Ui, i: usize| {
        let l = &loops[i];
        cards::detail_heading(ui, theme, &l.name, t("home.how_worked_out"));
        cards::Grid::new(("homes_loop_detail", i)).plain().min_width(380.0).max_columns(2).gap(theme.spacing_lg).show(ui, theme, 2, |ui, part| {
            if part == 0 {
                widgets::detail_row(ui, theme, t("home.loop_needs"), &l.demand);
                widgets::detail_row(ui, theme, t("home.loop_has"), &l.supply);
                if let Some(d) = l.food_demand_kcal.filter(|d| *d > 0.0) {
                    let grown = tf(
                        "home.loop_grown_value",
                        &[("kcal", &format!("{grown_kcal:.0}")), ("pct", &format!("{:.0}", grown_kcal / d * 100.0)), ("need", &format!("{d:.0}"))],
                    );
                    widgets::detail_row(ui, theme, t("home.loop_grown"), &grown);
                }
                // As the home runs (2026-10-22): measured over the last game day, or estimated.
                if let Some(Some(line)) = metered.get(i) {
                    widgets::detail_row(ui, theme, t("home.loop_running"), line);
                }
            } else {
                cards::prose(ui, theme, &l.note, theme.text_secondary());
            }
        });
    };
    cards::Grid::new("homes_loops").min_width(330.0).show_with_details(ui, theme, loops.len(), face, words);
}

// ── The ideal closed loop ───────────────────────────────────────────────────────────────────

/// A loop's requirement rows of one tier ("baseline" also takes rows that name none).
fn tier_rows<'a>(l: &'a OutlineLoop, tier: &str) -> Vec<&'a OutlineRequirement> {
    l.requirements.iter().filter(|r| r.tier == tier || (tier == "baseline" && r.tier.is_empty())).collect()
}

/// How many of a loop's parts the game has no id for yet.
fn missing_in_game(l: &OutlineLoop) -> usize {
    l.requirements.iter().filter(|r| r.game_id.is_empty()).count()
}

/// The ideal closed loop: its title and subtitle, "About this outline" folded, the loops as cards
/// or a table, what the game still needs (folded), and the footer.
pub fn draw_outline(ui: &mut Ui, theme: &Theme, outline: &HomeOutline) {
    if outline.loops.is_empty() {
        return;
    }
    let closing = outline.loops.iter().filter(|l| l.closes).count();
    let note = tf("home.outline_summary", &[("closed", &closing), ("total", &outline.loops.len())]);
    cards::group_heading(ui, theme, &outline.title, Some(&note));
    if !outline.subtitle.is_empty() {
        ui.scope(|ui| {
            ui.set_max_width(cards::prose_width(ui, theme));
            ui.label(RichText::new(&outline.subtitle).size(theme.font_size_small).color(theme.text_muted()));
        });
    }
    ui.add_space(theme.spacing_xs);
    let mut view = view_toggle::ListView::Cards;
    let mut about = false;
    ui.horizontal(|ui| {
        if !outline.intro.is_empty() {
            about = cards::fold(ui, theme, "homes_outline_about", t("home.outline_about"), "home.outline_about_hint");
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            view = view_toggle::remembered(ui, theme, OUTLINE_VIEW_ID, view_toggle::ListView::Cards);
        });
    });
    if about {
        ui.scope(|ui| {
            ui.set_max_width(cards::prose_width(ui, theme));
            ui.label(RichText::new(&outline.intro).size(theme.font_size_small).color(theme.text_secondary()));
        });
    }
    ui.add_space(theme.spacing_sm);
    match view {
        view_toggle::ListView::Cards => {
            cards::Grid::new("homes_outline").min_width(330.0).show_with_details(
                ui,
                theme,
                outline.loops.len(),
                |ui, i| outline_card(ui, theme, &outline.loops[i]),
                |ui, i| outline_words(ui, theme, &outline.loops[i]),
            );
        }
        view_toggle::ListView::Table => {
            outline_table(ui, theme, &outline.loops);
            ui.add_space(theme.spacing_md);
        }
    }
    if !outline.in_game_next.is_empty() && cards::fold(ui, theme, "homes_outline_next", t("home.game_needs"), "home.game_needs_hint") {
        ui.scope(|ui| {
            ui.set_max_width(cards::prose_width(ui, theme));
            for step in &outline.in_game_next {
                ui.horizontal_top(|ui| {
                    ui.label(RichText::new("-").size(theme.font_size_small).color(theme.text_muted()));
                    ui.add(egui::Label::new(RichText::new(step).size(theme.font_size_small).color(theme.text_secondary())).wrap());
                });
            }
        });
    }
    if !outline.footer.is_empty() {
        ui.add_space(theme.spacing_xs);
        ui.scope(|ui| {
            ui.set_max_width(cards::prose_width(ui, theme));
            ui.label(RichText::new(&outline.footer).size(theme.font_size_small).italics().color(theme.text_muted()));
        });
    }
    ui.add_space(theme.spacing_md);
}

/// The colour and word of an ideal loop's pill.
fn outline_pill(theme: &Theme, l: &OutlineLoop) -> (&'static str, Color32) {
    if l.closes {
        (t("home.outline_closes"), theme.success())
    } else {
        (t("home.outline_open"), theme.warning())
    }
}

/// One ideal loop's card: its pill, its parts and the parts the game has no id for, its demand
/// cut to three lines, and the two folds. Returns whether either is open (its words are then drawn
/// under the row by [`outline_words`]).
fn outline_card(ui: &mut Ui, theme: &Theme, l: &OutlineLoop) -> bool {
    title_row(ui, theme, &l.name, &[outline_pill(theme, l)]);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing_xs;
        ui.label(RichText::new(tn("home.parts", l.requirements.len() as i64, &[])).size(theme.font_size_small).color(theme.text_muted()));
        let missing = missing_in_game(l);
        if missing > 0 {
            ui.label(RichText::new(tf("home.not_in_game_count", &[("n", &missing)])).size(theme.font_size_small).color(theme.warning()));
        }
    });
    ui.add_space(theme.spacing_xs);
    labelled(ui, theme, t("home.demand"), &l.demand, 3);
    ui.add_space(theme.spacing_xs);
    let how = cards::fold(ui, theme, ("homes_outline_how", l.id.as_str()), t("home.how_worked_out"), "home.how_worked_out_hint");
    let parts = !l.requirements.is_empty() && cards::fold(ui, theme, ("homes_outline_parts", l.id.as_str()), t("home.what_it_takes"), "home.what_it_takes_hint");
    how || parts
}

/// A heading and its lines, each after a dash.
fn listed(ui: &mut Ui, theme: &Theme, title: &str, lines: &[String]) {
    if lines.is_empty() {
        return;
    }
    ui.add_space(theme.spacing_xs);
    ui.label(RichText::new(title).size(theme.font_size_small).strong().color(theme.text_primary()));
    for line in lines {
        ui.horizontal_top(|ui| {
            ui.label(RichText::new("-").size(theme.font_size_small).color(theme.text_muted()));
            // Wrapped: a label in a row extends past its cell unless told to wrap.
            ui.add(egui::Label::new(RichText::new(line).size(theme.font_size_small).color(theme.text_secondary())).wrap());
        });
    }
}

/// An ideal loop's opened words, under its row: "How this is worked out" (the whole demand and the
/// closure note, then the arithmetic and what it assumes, side by side where they fit) and "What it
/// takes" (the bare minimum beside the life of luxury).
fn outline_words(ui: &mut Ui, theme: &Theme, l: &OutlineLoop) {
    let ctx = ui.ctx().clone();
    if cards::fold_is_open(&ctx, ("homes_outline_how", l.id.as_str())) {
        cards::detail_heading(ui, theme, &l.name, t("home.how_worked_out"));
        cards::Grid::new(("homes_outline_how_detail", l.id.as_str())).plain().min_width(380.0).max_columns(3).gap(theme.spacing_lg).show(ui, theme, 3, |ui, part| {
            cards::readable(ui, theme, |ui| match part {
                0 => {
                    ui.label(RichText::new(t("home.demand")).size(theme.font_size_small).strong().color(theme.text_primary()));
                    ui.label(RichText::new(&l.demand).size(theme.font_size_small).color(theme.text_secondary()));
                    ui.add_space(theme.spacing_xs);
                    ui.label(RichText::new(&l.closure_note).size(theme.font_size_small).color(theme.text_secondary()));
                }
                1 => listed(ui, theme, t("home.the_arithmetic"), &l.balance),
                _ => listed(ui, theme, t("home.assumptions"), &l.assumptions),
            });
        });
    }
    if !l.requirements.is_empty() && cards::fold_is_open(&ctx, ("homes_outline_parts", l.id.as_str())) {
        cards::detail_heading(ui, theme, &l.name, t("home.what_it_takes"));
        // Two tiers per loop (operator 2026-07-05): the bare minimum first, then the life-of-luxury
        // additions. An empty game_id is the honest "not in game yet" flag.
        let tiers = [("baseline", t("home.bare_minimum"), theme.text_primary()), ("luxury", t("home.luxury"), theme.info())];
        cards::Grid::new(("homes_outline_parts_detail", l.id.as_str())).plain().min_width(380.0).max_columns(2).gap(theme.spacing_lg).show(ui, theme, tiers.len(), |ui, k| {
            let (tier, title, colour) = tiers[k];
            let rows = tier_rows(l, tier);
            if rows.is_empty() {
                return;
            }
            cards::readable(ui, theme, |ui| {
                ui.label(RichText::new(title).size(theme.font_size_small).strong().color(colour));
                for r in rows {
                    ui.add_space(theme.spacing_xs);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = theme.spacing_xs;
                        ui.label(RichText::new(format!("{} {}", r.qty, r.item)).size(theme.font_size_small).strong().color(theme.text_primary()));
                        if r.game_id.is_empty() {
                            cards::status_badge(ui, theme, t("home.not_in_game"), theme.warning());
                        }
                    });
                    ui.label(RichText::new(&r.why).size(theme.font_size_small).color(theme.text_muted()));
                }
            });
        });
    }
}

/// The ideal loops as a table: one row a loop.
fn outline_table(ui: &mut Ui, theme: &Theme, loops: &[OutlineLoop]) {
    let columns = [
        data_table::Column::new(t("home.col_loop"), 1.0, 120.0),
        data_table::Column::new(t("home.col_status"), 0.5, 80.0),
        data_table::Column::new(t("home.demand"), 5.0, 200.0),
        data_table::Column::new(t("home.bare_minimum"), 0.6, 90.0),
        data_table::Column::new(t("home.luxury"), 0.6, 90.0),
        data_table::Column::new(t("home.col_missing"), 0.7, 100.0),
    ];
    data_table::show(ui, theme, "homes_outline_table", &columns, loops.len(), |ui, r, c| {
        let l = &loops[r];
        match c {
            0 => data_table::text_cell(ui, theme, &l.name, theme.text_primary(), true),
            1 => {
                let (word, colour) = outline_pill(theme, l);
                data_table::dot(ui, theme, colour);
                data_table::text_cell(ui, theme, word, colour, false);
            }
            2 => data_table::text_cell(ui, theme, &l.demand, theme.text_secondary(), false),
            3 => data_table::text_cell(ui, theme, &tn("home.parts", tier_rows(l, "baseline").len() as i64, &[]), theme.text_primary(), false),
            4 => data_table::text_cell(ui, theme, &tn("home.parts", tier_rows(l, "luxury").len() as i64, &[]), theme.info(), false),
            _ => {
                let missing = missing_in_game(l);
                let colour = if missing > 0 { theme.warning() } else { theme.text_muted() };
                data_table::text_cell(ui, theme, &missing.to_string(), colour, missing > 0);
            }
        }
    });
}

// ── What one home cannot close ──────────────────────────────────────────────────────────────

/// What one home cannot close: its heading and count, the intro, a card a traded loop edged in
/// the warning colour (where it comes from; why folded), and the footer. Muted and outlined so it
/// reads apart from the loops that close.
pub fn draw_cannot_close(ui: &mut Ui, theme: &Theme, cc: &CannotCloseData) {
    if cc.entries.is_empty() {
        return;
    }
    let count = tn("home.cannot_count", cc.entries.len() as i64, &[]);
    ui.add_space(theme.spacing_sm);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(t("home.cannot_title")).size(theme.font_size_heading).color(theme.warning()));
        ui.label(RichText::new(count).size(theme.font_size_small).color(theme.text_muted()));
    });
    if !cc.intro.is_empty() {
        ui.scope(|ui| {
            ui.set_max_width(cards::prose_width(ui, theme));
            ui.label(RichText::new(&cc.intro).size(theme.font_size_small).color(theme.text_muted()));
        });
    }
    ui.add_space(theme.spacing_sm);
    cards::Grid::new("homes_cannot").min_width(300.0).edge(theme.warning()).show_with_details(
        ui,
        theme,
        cc.entries.len(),
        |ui, i| {
            let e = &cc.entries[i];
            title_row(ui, theme, &e.title, &[(t("home.traded"), theme.warning())]);
            ui.add_space(theme.spacing_xs);
            ui.label(RichText::new(&e.provided_by).size(theme.font_size_small).italics().color(theme.text_secondary()));
            ui.add_space(theme.spacing_xs);
            cards::fold(ui, theme, ("homes_cannot_why", e.id.as_str()), t("home.why_open"), "home.why_open_hint")
        },
        |ui, i| {
            let e = &cc.entries[i];
            cards::detail_heading(ui, theme, &e.title, t("home.why_open"));
            cards::prose(ui, theme, &e.body, theme.text_secondary());
        },
    );
    if !cc.footer.is_empty() {
        ui.scope(|ui| {
            ui.set_max_width(cards::prose_width(ui, theme));
            ui.label(RichText::new(&cc.footer).size(theme.font_size_small).italics().color(theme.text_secondary()));
        });
    }
    ui.add_space(theme.spacing_md);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home_loops() -> Vec<crate::machines::HomeLoop> {
        crate::machines::MachineHome::load(&std::path::Path::new("data").join("machines").join("home.ron")).map(|h| h.loops).unwrap_or_default()
    }

    /// THE GAUGES SAY ONLY WHAT IS KNOWN: the food loop's is the grown calories against its stated
    /// need, the power meter's loop's is the meter's make against use, and a loop whose numbers are
    /// only in its words (Water, Air, Nutrients) has none, so the page never draws a bar it made up.
    #[test]
    fn the_gauges_say_only_what_is_known() {
        let loops = home_loops();
        assert!(loops.len() >= 4, "the shipped home's loops: {}", loops.len());
        let food = loops.iter().find(|l| l.food_demand_kcal.is_some()).expect("a food loop");
        let g = gauge_for(food, 6020.0, Some((11.0, 24.0))).expect("food has a gauge");
        assert_eq!((g.have, g.need, g.kind), (6020.0, 6600.0, GaugeKind::Food));
        let energy = loops.iter().find(|l| l.meter == Some(crate::machines::LoopMeter::Power)).expect("a loop the power meter reads");
        assert_eq!(gauge_for(energy, 0.0, Some((11.0, 24.0))), Some(Gauge { have: 11.0, need: 24.0, kind: GaugeKind::Power }));
        assert_eq!(gauge_for(energy, 0.0, None), None, "no meter, no gauge");
        for l in loops.iter().filter(|l| l.food_demand_kcal.is_none() && l.meter != Some(crate::machines::LoopMeter::Power)) {
            assert_eq!(gauge_for(l, 6020.0, Some((11.0, 24.0))), None, "{} has only words", l.name);
        }
        assert_eq!(gauge_words(Gauge { have: 11.0, need: 24.0, kind: GaugeKind::Power }), "Makes 11.0 of the 24.0 kWh/day it uses (46%)");
    }

    /// EVERY FACT IS STILL ON THE PAGE: each ideal loop's balance and assumptions (the arithmetic
    /// behind every claim, in_game_next's first item) are read, so the fold can show them.
    #[test]
    fn the_outline_carries_its_arithmetic() {
        let o = super::super::homes::load_home_outline(std::path::Path::new("data"));
        assert!(o.loops.iter().all(|l| !l.balance.is_empty() && !l.assumptions.is_empty()), "every loop's balance and assumptions read");
    }
}
