//! The How-to page's list: the how-tos in the chosen branch, or the search's matches across all of
//! them, each with its title, its line, its time, its difficulty in words, its hazard as a word
//! and an icon, and its review state. Planned ones are listed dimmed, "Not written yet". Sorted
//! Suggested (the tree's order), A to Z, or Quickest; chips narrow to Easy and Under 30 minutes.

use egui::{RichText, Ui};

use super::{difficulty_word, hazard_badge, review_words, time_words, w, wf};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;
use crate::how_to::library::{Library, Loaded};
use crate::how_to::model::Planned;

/// One row: a written how-to or a planned one.
pub enum Entry<'a> {
    Written(&'a Loaded),
    Planned(&'a Planned),
}

/// What the list shows for the page's current branch, search, chips and sort.
pub fn entries<'a>(state: &GuiState, lib: &'a Library, dev: bool) -> Vec<Entry<'a>> {
    let h = &state.how_to;
    let q = h.search.trim();
    let branch = h.branch.as_ref().map(|b| lib.branch_ids(b));
    let in_branch = |cat: &str| branch.as_ref().map(|ids| ids.contains(cat)).unwrap_or(true);
    let shows = |a: crate::how_to::model::Audience, cat: &str| {
        (a != crate::how_to::model::Audience::Developer || dev) && lib.path_to(cat).iter().all(|c| lib.category(c).map(|n| n.audience != crate::how_to::model::Audience::Developer || dev).unwrap_or(true))
    };
    let mut written: Vec<&Loaded> = lib
        .how_tos
        .iter()
        .filter(|l| in_branch(&l.how_to.category) && shows(l.how_to.audience, &l.how_to.category))
        .filter(|l| crate::how_to::view::matches(&l.how_to, q))
        .filter(|l| !h.easy_only || l.how_to.difficulty <= 1)
        .filter(|l| !h.quick_only || l.how_to.time.active_min + l.how_to.time.waiting_min < 30)
        .collect();
    let order = |cat: &str| -> Vec<u32> { lib.path_to(cat).iter().filter_map(|c| lib.category(c).map(|n| n.order)).collect() };
    match h.sort {
        super::Sort::Suggested => written.sort_by(|a, b| order(&a.how_to.category).cmp(&order(&b.how_to.category)).then(a.how_to.title.cmp(&b.how_to.title))),
        super::Sort::AToZ => written.sort_by(|a, b| a.how_to.title.to_lowercase().cmp(&b.how_to.title.to_lowercase())),
        super::Sort::Quickest => written.sort_by_key(|l| (l.how_to.time.active_min + l.how_to.time.waiting_min, l.how_to.title.clone())),
    }
    let mut out: Vec<Entry> = written.into_iter().map(Entry::Written).collect();
    // Planned ones after the written, never narrowed by the chips (they have no time yet).
    if !h.easy_only && !h.quick_only {
        let ql = q.to_lowercase();
        let mut planned: Vec<&Planned> = lib
            .planned
            .iter()
            .filter(|p| in_branch(&p.category) && shows(p.audience, &p.category))
            .filter(|p| ql.split_whitespace().all(|word| format!("{} {}", p.title, p.summary).to_lowercase().contains(word)))
            .collect();
        planned.sort_by(|a, b| order(&a.category).cmp(&order(&b.category)).then(a.title.cmp(&b.title)));
        out.extend(planned.into_iter().map(Entry::Planned));
    }
    out
}

pub fn draw(ui: &mut Ui, theme: &Theme, state: &mut GuiState, lib: &Library) {
    let h = &state.how_to;
    let q = h.search.trim().to_string();
    let (title, line) = if !q.is_empty() {
        (wf("results_for", &[("q", &q)]), String::new())
    } else if let Some(c) = h.branch.as_ref().and_then(|b| lib.category(b)) {
        (c.title.clone(), c.line.clone())
    } else {
        (w("all").to_string(), String::new())
    };
    ui.label(RichText::new(title).strong().size(theme.font_size_heading).color(theme.text_primary()));
    if !line.is_empty() {
        ui.label(RichText::new(line).color(theme.text_muted()).size(theme.font_size_small));
    }
    ui.add_space(theme.spacing_xs);
    ui.horizontal_wrapped(|ui| {
        for (sort, key) in [(super::Sort::Suggested, "sort_suggested"), (super::Sort::AToZ, "sort_az"), (super::Sort::Quickest, "sort_quickest")] {
            if widgets::Button::tab(w(key), state.how_to.sort == sort).show(ui, theme) {
                state.how_to.sort = sort;
            }
        }
        ui.add_space(theme.spacing_sm);
        if widgets::Button::tab(w("chip_easy"), state.how_to.easy_only).show(ui, theme) {
            state.how_to.easy_only = !state.how_to.easy_only;
        }
        if widgets::Button::tab(w("chip_quick"), state.how_to.quick_only).show(ui, theme) {
            state.how_to.quick_only = !state.how_to.quick_only;
        }
    });
    ui.add_space(theme.spacing_sm);

    // Developer mode lives on the theme (Settings > Account > Developer mode).
    let list = entries(state, lib, theme.nav_dev_visible);
    if list.is_empty() {
        ui.label(RichText::new(w("list_empty")).color(theme.text_muted()));
        return;
    }
    let mut open = None;
    for e in &list {
        let (id, selected) = match e {
            Entry::Written(l) => (l.how_to.id.as_str(), state.how_to.open.as_deref() == Some(l.how_to.id.as_str())),
            Entry::Planned(p) => (p.id.as_str(), state.how_to.open.as_deref() == Some(p.id.as_str())),
        };
        let frame = egui::Frame::NONE
            .fill(if selected { theme.bg_secondary() } else { theme.bg_card() })
            .stroke(egui::Stroke::new(1.0, if selected { theme.accent() } else { theme.border() }))
            .corner_radius(egui::CornerRadius::same(theme.border_radius as u8))
            .inner_margin(egui::Margin::same(theme.spacing_sm as i8));
        let resp = frame
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                match e {
                    Entry::Written(l) => {
                        ui.label(RichText::new(&l.how_to.title).strong().color(theme.text_primary()));
                        ui.label(RichText::new(&l.how_to.summary).color(theme.text_secondary()).size(theme.font_size_small));
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new(format!("{} · {}", time_words(&l.how_to.time), difficulty_word(l.how_to.difficulty))).color(theme.text_muted()).size(theme.font_size_small));
                            hazard_badge(ui, theme, l.how_to.hazard);
                            ui.label(RichText::new(review_words(l)).color(theme.text_muted()).size(theme.font_size_small));
                        });
                    }
                    Entry::Planned(p) => {
                        ui.label(RichText::new(&p.title).color(theme.text_muted()));
                        ui.label(RichText::new(w("not_written")).color(theme.text_muted()).size(theme.font_size_small).italics());
                    }
                }
            })
            .response
            .interact(egui::Sense::click());
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if resp.clicked() {
            open = Some(id.to_string());
        }
        ui.add_space(theme.spacing_xs);
    }
    if let Some(id) = open {
        state.how_to.open(&id);
    }
}
