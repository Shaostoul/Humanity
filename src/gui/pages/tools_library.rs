// Child of pages/tools.rs (#[path]): the toolbar (search, sort, view, the kind and category pills)
// and the shelf itself, a Grid of tiles or a List of rows, drawn on the shelf the Games page uses
// (widgets/shelf.rs). Drawn from the page's state as it is; what a click asks for is returned as
// an `Act` and done by tools.rs after the frame's drawing. The view, the sort and the filters are
// kept in the settings (`tools_view`), here, so they survive a restart.

use egui::{Rect, RichText, Sense, Stroke, Vec2};

use super::{Act, SortBy, ToolsState, View};
use crate::gui::theme::Theme;
use crate::gui::widgets::shelf::{self, Heading, Layout};
use crate::gui::widgets::{self, cover_tile, icons};
use crate::gui::GuiState;
use crate::software::ToolEntry;

/// A tile's width at the least (tiles stretch to fill a row).
pub(super) const TILE_W: f32 = 220.0;
/// The cover slot's shape: the Games page's, so the two shelves look alike.
pub(super) const COVER_ASPECT: f32 = 460.0 / 215.0;

/// The main button's height on a tile and in a row, and the height of what stands in its place.
fn button_height(theme: &Theme) -> f32 {
    theme.font_size_body + 2.0 * theme.button_pad_y + theme.spacing_xs
}

/// Keep a new view in the settings (marked for saving), and close details the new filter hides.
pub(super) fn keep_view(state: &mut GuiState, view: View) {
    if state.settings.tools_view != view {
        state.settings.tools_view = view;
        state.settings_dirty = true;
    }
}

/// The toolbar: search, sort, view; then the kind pills with their counts, and the category pills
/// (only the categories of the kind chosen, so a pill always matches something).
pub(super) fn toolbar(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let mut view = state.settings.tools_view.clone();
    let sorts: Vec<(SortBy, &str, &str)> = SortBy::ALL.iter().map(|s| (*s, s.words(), s.tip())).collect();
    let picked = shelf::toolbar(ui, theme, &mut state.tools.search, super::SEARCH_HINT, &sorts, view.sort, view.layout, ["A tile for each.", "A row for each, its facts in columns."]);
    if let Some(sort) = picked.sort {
        view.sort = sort;
    }
    if let Some(layout) = picked.layout {
        view.layout = layout;
    }
    ui.add_space(theme.spacing_xs);

    // The kinds, in the catalog's order, each with how many it holds.
    let catalog = &state.tools_catalog;
    let mut kinds: Vec<String> = Vec::new();
    for e in catalog {
        if !e.kind.is_empty() && !kinds.contains(&e.kind) {
            kinds.push(e.kind.clone());
        }
    }
    ui.horizontal_wrapped(|ui| {
        let mut options: Vec<(Option<String>, String)> = vec![(None, format!("{} {}", super::EVERYTHING, catalog.len()))];
        for k in &kinds {
            options.push((Some(k.clone()), format!("{} {}", super::kind_label(k), catalog.iter().filter(|e| &e.kind == k).count())));
        }
        let labels: Vec<String> = options.iter().map(|(_, l)| l.clone()).collect();
        let chosen = options.iter().position(|(k, _)| *k == view.kind).unwrap_or(0);
        if let Some(i) = cover_tile::choice_pills(ui, theme, &labels, chosen, &[]) {
            view.kind = options[i].0.clone();
            // Categories belong to a kind, so one chosen under another kind would match nothing.
            view.category = None;
        }
    });
    ui.add_space(theme.spacing_xs);

    // The categories of the kind chosen, in the catalog's order.
    let mut categories: Vec<(String, String)> = Vec::new();
    for e in catalog.iter().filter(|e| view.kind.as_deref().is_none_or(|k| e.kind == k)) {
        if !categories.iter().any(|(id, _)| *id == e.category_id) {
            categories.push((e.category_id.clone(), e.category.clone()));
        }
    }
    ui.horizontal_wrapped(|ui| {
        let mut labels = vec![super::ALL.to_string()];
        labels.extend(categories.iter().map(|(_, name)| name.clone()));
        let chosen = view.category.as_ref().and_then(|c| categories.iter().position(|(id, _)| id == c)).map_or(0, |p| p + 1);
        if let Some(i) = cover_tile::choice_pills(ui, theme, &labels, chosen, &[]) {
            view.category = if i == 0 { None } else { Some(categories[i - 1].0.clone()) };
        }
    });
    ui.add_space(theme.spacing_sm);
    keep_view(state, view);
}

/// The shelf: the catalog, in the order worked out, as tiles or rows.
pub(super) fn shelf(ui: &mut egui::Ui, theme: &Theme, state: &GuiState) -> Vec<Act> {
    let page = &state.tools;
    let mut acts = Vec::new();
    let order: &[usize] = page.order.as_ref().map(|(_, o)| o.as_slice()).unwrap_or(&[]);
    if order.is_empty() {
        let words = if page.search.trim().is_empty() { "Nothing here matches the filters.".to_string() } else { format!("Nothing here matches \"{}\".", page.search.trim()) };
        ui.label(RichText::new(words).size(theme.font_size_small).color(theme.text_muted()));
        return acts;
    }
    let view = &state.settings.tools_view;
    match view.layout {
        Layout::Grid if view.sort == SortBy::Category => {
            // Each category under its name.
            let mut start = 0;
            while start < order.len() {
                let id = &state.tools_catalog[order[start]].category_id;
                let end = order[start..].iter().position(|&i| &state.tools_catalog[i].category_id != id).map_or(order.len(), |n| start + n);
                group_heading(ui, theme, &state.tools_catalog[order[start]], end - start);
                grid(ui, theme, state, &order[start..end], &mut acts);
                start = end;
            }
        }
        Layout::Grid => grid(ui, theme, state, order, &mut acts),
        Layout::List => list(ui, theme, state, order, &mut acts),
    }
    acts
}

/// A category's name over its tiles (the Grid sorted by category), with how many.
fn group_heading(ui: &mut egui::Ui, theme: &Theme, first: &ToolEntry, n: usize) {
    ui.add_space(theme.spacing_xs);
    ui.horizontal(|ui| {
        let (dot, _) = ui.allocate_exact_size(Vec2::splat(theme.font_size_small), Sense::hover());
        ui.painter().circle_filled(dot.center(), dot.width() * 0.35, category_color(theme, first));
        ui.label(RichText::new(&first.category).size(theme.font_size_body).strong().color(theme.text_primary()));
        ui.label(RichText::new(n.to_string()).size(theme.font_size_small).color(theme.text_muted()));
    });
    ui.add_space(theme.spacing_xs);
}

/// An entry's category colour: its category's own hue, as the plain covers are tinted.
pub(super) fn category_color(theme: &Theme, e: &ToolEntry) -> egui::Color32 {
    theme.text_secondary().lerp_to_gamma(widgets::swatch_color(&e.category_id), 0.7)
}

fn grid(ui: &mut egui::Ui, theme: &Theme, state: &GuiState, items: &[usize], acts: &mut Vec<Act>) {
    shelf::grid(ui, theme, items, TILE_W, |ui, i, w| tile(ui, theme, state, i, w, acts));
}

/// The line under a tile's name and category: installed (and its version), else where it runs and
/// its size, or for a help service its website.
fn third_line(state: &GuiState, i: usize) -> (String, bool) {
    let e = &state.tools_catalog[i];
    if let Some(found) = state.tools.installed(i) {
        let words = match (&found.version, found.via) {
            (Some(v), _) => format!("{} \u{b7} {v}", super::INSTALLED),
            (None, crate::software::Via::Steam) => format!("{} through Steam", super::INSTALLED),
            (None, _) => super::INSTALLED.to_string(),
        };
        return (words, true);
    }
    if !e.is_software() {
        return (e.host(), false);
    }
    if e.web_only() {
        return ("Runs in your web browser".to_string(), false);
    }
    let mut words = super::platforms_words(e);
    if !e.size.is_empty() {
        words.push_str(" \u{b7} ");
        words.push_str(&e.size);
    }
    (words, false)
}

/// One tile: its cover (a panel in its category's colour with its name, its program's icon when
/// installed, an "Installed" mark), its name, its category (and licence), one more line, and its
/// main button. A click on its cover or its name opens its details.
fn tile(ui: &mut egui::Ui, theme: &Theme, state: &GuiState, i: usize, width: f32, acts: &mut Vec<Act>) {
    let e = &state.tools_catalog[i];
    let page = &state.tools;
    let installed = page.installed(i);
    shelf::tile_frame(ui, theme, width, page.selected == Some(i), |ui, w| {
        let (rect, cover) = ui.allocate_exact_size(Vec2::new(w, (w / COVER_ASPECT).round()), Sense::click());
        cover_tile::paint_cover(ui.painter(), theme, rect, picture_of(state, i), page.icon_of(i), &e.name, &e.category_id, theme.font_size_heading, theme.border_radius);
        if installed.is_some() {
            installed_mark(ui, theme, rect);
        }
        let name = ui.add(egui::Label::new(RichText::new(&e.name).size(theme.font_size_body).strong().color(theme.text_primary())).truncate().sense(Sense::click()));
        if cover.clicked() || name.clicked() {
            acts.push(Act::Select(i));
        }
        if cover.hovered() || name.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let meta = if e.license.is_empty() { e.category.clone() } else { format!("{} \u{b7} {}", e.category, e.license) };
        ui.add(egui::Label::new(RichText::new(meta).size(theme.font_size_small).color(theme.text_muted())).truncate());
        // One more line, always the same height, so the tiles of a row line up.
        let line_h = theme.font_size_small + theme.spacing_xs + 4.0;
        let (words, is_installed) = third_line(state, i);
        ui.horizontal(|ui| {
            ui.set_min_height(line_h);
            if is_installed {
                let (mark, _) = ui.allocate_exact_size(Vec2::splat(theme.font_size_small + 2.0), Sense::hover());
                icons::paint_check(ui.painter(), mark, theme.success());
            }
            let color = if is_installed { theme.success() } else { theme.text_secondary() };
            ui.add(egui::Label::new(RichText::new(words).size(theme.font_size_small).color(color)).truncate());
        });
        ui.add_space(theme.spacing_xs);
        main_button(ui, theme, e, i, installed, installing(page, i), true, acts);
    });
}

/// The picture a tile's cover shows: none. No logo is shipped with HumanityOS or fetched from any
/// website: each maker sets its own rules for its logo (the 2026-10-19 finding, judgement call 6:
/// docs/reference/findings/2026-10-19-tools-page-install-launch-and-icons.md). An installed
/// program's own icon, read in place from its own file and shown to this person only, is drawn on
/// the plain cover as its mark. The one place a tile's picture comes from, so a change to that (a
/// maker who allows its logo, say) is made here.
pub(super) fn picture_of(_state: &GuiState, _i: usize) -> Option<(egui::TextureId, Vec2)> {
    None
}

/// "Installed", small, at a cover's top left.
fn installed_mark(ui: &egui::Ui, theme: &Theme, cover: Rect) {
    let pad = theme.spacing_sm;
    let galley = ui.painter().layout_no_wrap(super::INSTALLED.to_string(), egui::FontId::proportional(theme.font_size_small), theme.text_on_accent());
    let size = galley.size() + Vec2::new(2.0 * theme.spacing_sm, theme.spacing_xs * 2.0);
    let rect = Rect::from_min_size(cover.min + Vec2::splat(pad), size);
    let corners = egui::CornerRadius::same(theme.pill_radius.round().clamp(0.0, 255.0) as u8);
    ui.painter().rect_filled(rect, corners, theme.success().gamma_multiply(0.9));
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, theme.text_on_accent());
    ui.painter().rect_stroke(rect, corners, Stroke::new(1.0, theme.success()), egui::StrokeKind::Inside);
}

/// The main button: Launch when installed (or, installed with no program to start, the few words
/// that say so), otherwise Download or Open website. `full` stretches it across a tile.
/// Whether winget is installing entry `i` now.
fn installing(page: &ToolsState, i: usize) -> bool {
    matches!(&page.install, Some(super::Installing::Running { entry, .. }) if *entry == i)
}

#[allow(clippy::too_many_arguments)]
fn main_button(ui: &mut egui::Ui, theme: &Theme, e: &ToolEntry, i: usize, installed: Option<&crate::software::Installed>, busy: bool, full: bool, acts: &mut Vec<Act>) {
    if busy {
        ui.horizontal(|ui| {
            ui.set_min_height(button_height(theme));
            ui.add(egui::Spinner::new().size(theme.font_size_body));
            ui.label(RichText::new("Installing...").size(theme.font_size_small).color(theme.text_secondary()));
        });
        return;
    }
    match installed {
        Some(found) if found.launch.is_some() => {
            let tip = if found.via == crate::software::Via::Steam { "Asks Steam to start it." } else { "Starts its program." };
            let mut b = widgets::Button::primary(super::LAUNCH).min_height(button_height(theme)).tooltip(tip);
            if full {
                b = b.full_width();
            }
            if b.show(ui, theme) {
                acts.push(Act::Launch(i));
            }
        }
        Some(_) => {
            let w = if full { ui.available_width() } else { ui.available_width().min(160.0) };
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, button_height(theme)), Sense::hover());
            let galley = ui.painter().layout_no_wrap(super::NO_PROGRAM_SHORT.to_string(), egui::FontId::proportional(theme.font_size_small), theme.text_muted());
            let at = egui::pos2(rect.min.x, rect.center().y - galley.size().y / 2.0);
            ui.painter().with_clip_rect(rect).galley(at, galley, theme.text_muted());
            resp.on_hover_text(super::NO_PROGRAM);
        }
        None => {
            let label = super::web_button(e);
            let tip = if label == super::DOWNLOAD { "Opens the project's own download page in your browser." } else { "Opens its website in your browser." };
            let mut b = widgets::Button::secondary(label).min_height(button_height(theme)).tooltip(tip);
            if full {
                b = b.full_width();
            }
            if b.show(ui, theme) {
                let url = if label == super::DOWNLOAD { e.download_page() } else { e.url.as_str() };
                acts.push(Act::Web(url.to_string()));
            }
        }
    }
}

/// The list's columns, each shown only when the room allows.
#[derive(Clone, Copy, PartialEq)]
enum Column {
    Category,
    Installed,
    License,
    Platforms,
    Size,
}

impl Column {
    fn heading(self) -> &'static str {
        match self {
            Column::Category => "Category",
            Column::Installed => "Installed",
            Column::License => "Licence",
            Column::Platforms => "Runs on",
            Column::Size => "Size",
        }
    }

    fn width(self) -> f32 {
        match self {
            Column::Category => 200.0,
            Column::Installed => 120.0,
            Column::License => 110.0,
            Column::Platforms => 170.0,
            Column::Size => 80.0,
        }
    }

    fn sort(self) -> Option<SortBy> {
        match self {
            Column::Category => Some(SortBy::Category),
            Column::Installed => Some(SortBy::InstalledFirst),
            Column::License | Column::Platforms | Column::Size => None,
        }
    }

    fn value(self, state: &GuiState, i: usize) -> String {
        let e = &state.tools_catalog[i];
        match self {
            Column::Category => e.category.clone(),
            Column::Installed => state
                .tools
                .installed(i)
                .map(|f| match (&f.version, f.via) {
                    (Some(v), _) => v.clone(),
                    (None, crate::software::Via::Steam) => "Through Steam".to_string(),
                    (None, _) => "Yes".to_string(),
                })
                .unwrap_or_default(),
            Column::License => e.license.clone(),
            Column::Platforms => super::platforms_words(e),
            Column::Size => if e.web_only() { String::new() } else { e.size.clone() },
        }
    }
}

/// The action cell's width, and the name's least.
const ACTION_W: f32 = 132.0;
const NAME_MIN: f32 = 200.0;

/// The columns that fit in `width`, most wanted first, in the order drawn.
fn columns_for(width: f32) -> Vec<Column> {
    let wanted: Vec<(Column, f32)> = [Column::Category, Column::Installed, Column::License, Column::Platforms, Column::Size].iter().map(|c| (*c, c.width())).collect();
    shelf::columns_that_fit(width - ACTION_W - NAME_MIN, &wanted, &[Column::Category, Column::Installed, Column::License, Column::Platforms, Column::Size])
}

/// The list: a heading row (a click on Name, Category or Installed sorts by it), then a row each.
fn list(ui: &mut egui::Ui, theme: &Theme, state: &GuiState, order: &[usize], acts: &mut Vec<Act>) {
    let page: &ToolsState = &state.tools;
    let view = &state.settings.tools_view;
    let width = ui.available_width();
    let columns = columns_for(width);
    let gap = ui.spacing().item_spacing.x;
    let fixed: f32 = ACTION_W + columns.iter().map(|c| c.width()).sum::<f32>() + gap * (columns.len() as f32 + 2.0);
    let name_w = (width - fixed).max(NAME_MIN * 0.6);
    let row_h = button_height(theme) + theme.spacing_sm;

    let mut cells = vec![Heading { text: "", width: ACTION_W, sorts: false, on: false }, Heading { text: "Name", width: name_w, sorts: true, on: view.sort == SortBy::Name }];
    cells.extend(columns.iter().map(|c| Heading { text: c.heading(), width: c.width(), sorts: c.sort().is_some(), on: c.sort() == Some(view.sort) }));
    if let Some(clicked) = shelf::list_headings(ui, theme, &cells) {
        let sort = if clicked == 1 { Some(SortBy::Name) } else { clicked.checked_sub(2).and_then(|c| columns.get(c)).and_then(|c| c.sort()) };
        if let Some(sort) = sort {
            acts.push(Act::View(View { sort, ..view.clone() }));
        }
    }

    for (n, &i) in order.iter().enumerate() {
        let e = &state.tools_catalog[i];
        shelf::list_row(ui, theme, n, width, row_h, page.selected == Some(i), |ui| {
            cover_tile::fixed_cell(ui, ACTION_W, row_h, |ui| main_button(ui, theme, e, i, page.installed(i), installing(page, i), false, acts));
            cover_tile::fixed_cell(ui, name_w, row_h, |ui| {
                if let Some((tex, size)) = page.icon_of(i) {
                    let side = theme.font_size_body + theme.spacing_xs;
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
                    let k = side / size.x.max(size.y).max(1.0);
                    ui.painter().image(tex, Rect::from_center_size(rect.center(), size * k), Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                }
                let name = ui.add(egui::Label::new(RichText::new(&e.name).size(theme.font_size_body).strong().color(theme.text_primary())).truncate().sense(Sense::click()));
                if name.clicked() {
                    acts.push(Act::Select(i));
                }
                if name.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            });
            for c in &columns {
                cover_tile::fixed_cell(ui, c.width(), row_h, |ui| {
                    let installed = *c == Column::Installed;
                    let color = if installed { theme.success() } else if *c == Column::Category { theme.text_muted() } else { theme.text_secondary() };
                    ui.add(egui::Label::new(RichText::new(c.value(state, i)).size(theme.font_size_small).color(color)).truncate());
                });
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE LIST'S COLUMNS GIVE WAY WHEN THE ROOM DOES: all five on a wide page, in their order; on
    /// a narrow one the least wanted go first (Size, then Runs on), Category last of all.
    ///
    /// Seen red 2026-10-19 with the wanted order reversed (Size most wanted): "the least wanted go
    /// first" failed.
    #[test]
    fn the_lists_columns_give_way() {
        assert!(columns_for(1400.0) == vec![Column::Category, Column::Installed, Column::License, Column::Platforms, Column::Size], "all five, in their order");
        let narrow = columns_for(ACTION_W + NAME_MIN + 200.0 + 120.0 + 110.0 + 20.0);
        assert!(narrow == vec![Column::Category, Column::Installed, Column::License], "the least wanted go first");
        assert!(columns_for(300.0).is_empty());
    }
}
