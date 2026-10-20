// Child of pages/games.rs (#[path]): the toolbar (search, sort, view, the stores' pills) and the
// library itself, a Grid of tiles or a List of rows. Drawn from the page's state as it is; what a
// click asks for is returned as an `Act` and done by games.rs after the frame's drawing. The
// toolbar's line, the grid, the tile's card and the List's headings and rows are the shelf the
// Tools page shares (widgets/shelf.rs, 2026-10-19).

use egui::{RichText, Sense, Vec2};

use super::{Act, GamesState, Layout, SortBy};
use crate::games::{Game, Store};
use crate::gui::theme::Theme;
use crate::gui::widgets::shelf::{self, Heading};
use crate::gui::widgets::{self, cover_tile, thumbnails};
use crate::gui::GuiState;

/// A tile's width at the least (tiles stretch to fill a row).
pub(super) const TILE_W: f32 = 220.0;
/// The cover slot's shape: Steam's wide library picture, 460 by 215, so its artwork fills it
/// exactly the day the app may show it (games/art.rs).
pub(super) const COVER_ASPECT: f32 = 460.0 / 215.0;

/// Play's height on a tile and in a row (a button is as tall as its words and their padding), and
/// the height of what stands in its place, so tiles and rows with and without it line up.
fn play_height(theme: &Theme) -> f32 {
    theme.font_size_body + 2.0 * theme.button_pad_y + theme.spacing_xs
}

/// The toolbar: search, sort, view, and one pill per store found with its count.
pub(super) fn toolbar(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let Some(found) = state.games.found.as_ref() else { return };
    let mut stores = Vec::new();
    for (store, list) in [(Store::Steam, &found.steam), (Store::Epic, &found.epic), (Store::Gog, &found.gog)] {
        if list.is_some() {
            stores.push(store);
        }
    }
    stores.push(Store::Added);

    let tips = ["A to Z.", "The game you played last first.", "The game you have played longest first.", "The largest on the disk first."];
    let sorts: Vec<(SortBy, &str, &str)> = SortBy::ALL.iter().zip(tips).map(|(s, tip)| (*s, s.words(), tip)).collect();
    let picked = shelf::toolbar(ui, theme, &mut state.games.search, super::SEARCH_HINT, &sorts, state.settings.games_sort, state.settings.games_layout, ["A tile for each game.", "A row for each game, its numbers in columns."]);
    if let Some(sort) = picked.sort {
        state.settings.games_sort = sort;
        state.settings_dirty = true;
    }
    if let Some(layout) = picked.layout {
        state.settings.games_layout = layout;
        state.settings_dirty = true;
    }
    ui.add_space(theme.spacing_xs);
    ui.horizontal_wrapped(|ui| {
        let count = |store: Option<Store>| state.games.all.iter().filter(|g| store.is_none_or(|s| g.store == s)).count();
        let mut options = vec![(None, format!("{} {}", super::ALL, count(None)))];
        options.extend(stores.iter().map(|s| (Some(*s), format!("{} {}", s.name(), count(Some(*s))))));
        let labels: Vec<String> = options.iter().map(|(_, l)| l.clone()).collect();
        let chosen = options.iter().position(|(s, _)| *s == state.games.filter).unwrap_or(0);
        if let Some(i) = cover_tile::choice_pills(ui, theme, &labels, chosen, &[]) {
            state.games.filter = options[i].0;
        }
    });
    ui.add_space(theme.spacing_sm);
}

/// The library: what was read, in the order worked out, as tiles or rows.
pub(super) fn library(ui: &mut egui::Ui, theme: &Theme, state: &GuiState) -> Vec<Act> {
    let page = &state.games;
    let mut acts = Vec::new();
    let Some(found) = &page.found else {
        ui.label(RichText::new(super::READING).size(theme.font_size_body).color(theme.text_muted()));
        return acts;
    };
    if found.steam.is_none() && found.epic.is_none() && found.gog.is_none() {
        ui.label(RichText::new(super::NO_STORES).size(theme.font_size_small).color(theme.text_muted()));
        ui.add_space(theme.spacing_sm);
    }
    let order: &[usize] = page.order.as_ref().map(|(_, o)| o.as_slice()).unwrap_or(&[]);
    if order.is_empty() {
        if let Some(words) = empty_words(page) {
            ui.label(RichText::new(words).size(theme.font_size_small).color(theme.text_muted()));
        }
    } else {
        match state.settings.games_layout {
            Layout::Grid => grid(ui, theme, page, order, state.settings.games_sort, &mut acts),
            Layout::List => list(ui, theme, page, order, state.settings.games_sort, &mut acts),
        }
    }
    if page.added.is_empty() && page.filter.is_none_or(|s| s == Store::Added) {
        ui.add_space(theme.spacing_sm);
        ui.label(RichText::new(super::ADDED_NONE).size(theme.font_size_small).color(theme.text_muted()));
    }
    acts
}

/// What the library says when nothing is shown.
fn empty_words(page: &GamesState) -> Option<String> {
    if !page.search.trim().is_empty() {
        return Some(format!("No game's name holds \"{}\".", page.search.trim()));
    }
    match page.filter {
        Some(Store::Steam) => Some("No Steam games are installed on this computer.".to_string()),
        Some(Store::Epic) => Some("No games from the Epic Games Store are installed on this computer.".to_string()),
        Some(Store::Gog) => Some("No GOG games are installed on this computer.".to_string()),
        // Added by you, empty: ADDED_NONE says how.
        _ => None,
    }
}

fn is_selected(page: &GamesState, game: &Game) -> bool {
    page.selected.as_ref().is_some_and(|(s, id)| *s == game.store && *id == game.id)
}

/// The one number a tile shows beside its store: the one the library is sorted by, or (sorted by
/// name) the time played, else the size.
fn tile_number(game: &Game, sort: SortBy) -> Option<String> {
    let d = &game.details;
    match sort {
        SortBy::RecentlyPlayed => d.last_played.map(|t| format!("Played {}", super::day_words(t, true))),
        SortBy::MostPlayed => d.played_minutes.filter(|m| *m > 0).map(super::played_words),
        SortBy::Size => d.size.map(super::size_words),
        SortBy::Name => d.played_minutes.filter(|m| *m > 0).map(super::played_words).or_else(|| d.size.map(super::size_words)),
    }
}

/// The grid: as many tiles to a row as fit, each stretched so the rows fill the width.
fn grid(ui: &mut egui::Ui, theme: &Theme, page: &GamesState, order: &[usize], sort: SortBy, acts: &mut Vec<Act>) {
    shelf::grid(ui, theme, order, TILE_W, |ui, i, tile_w| {
        let game = &page.all[i];
        tile(ui, theme, game, page.icon_of(game), tile_w, is_selected(page, game), sort, acts);
    });
}

/// One tile: its cover slot (Steam's cached picture, else a plain panel with its program's icon or
/// its first letter), its name, its store and one number, its achievements, and Play (or why it
/// cannot start, in Play's place). A click on its cover or its name opens its details.
#[allow(clippy::too_many_arguments)]
fn tile(ui: &mut egui::Ui, theme: &Theme, game: &Game, icon: Option<(egui::TextureId, Vec2)>, width: f32, selected: bool, sort: SortBy, acts: &mut Vec<Act>) {
    shelf::tile_frame(ui, theme, width, selected, |ui, w| {
        let (rect, cover) = ui.allocate_exact_size(Vec2::new(w, (w / COVER_ASPECT).round()), Sense::click());
        let picture = game.details.art.cover.as_deref().and_then(|p| thumbnails::get(ui.ctx(), p, w));
        cover_tile::paint_cover(ui.painter(), theme, rect, picture, icon, &game.name, &game.id, theme.font_size_heading, theme.border_radius);
        let name = ui.add(egui::Label::new(RichText::new(&game.name).size(theme.font_size_body).strong().color(theme.text_primary())).truncate().sense(Sense::click()));
        if cover.clicked() || name.clicked() {
            acts.push(Act::Select(game.store, game.id.clone()));
        }
        if cover.hovered() || name.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let meta = match tile_number(game, sort) {
            Some(n) => format!("{} \u{b7} {n}", game.store.name()),
            None => game.store.name().to_string(),
        };
        ui.add(egui::Label::new(RichText::new(meta).size(theme.font_size_small).color(theme.text_muted())).truncate());
        // Its achievements, or an empty line of the same height, so the tiles of a row line up.
        let line_h = theme.font_size_small + theme.spacing_xs + 4.0;
        match &game.details.achievements {
            Some(a) => {
                ui.horizontal(|ui| {
                    ui.set_min_height(line_h);
                    let words = super::achievements_words(a);
                    if let Some(n) = a.unlocked() {
                        let bar_w = (w * 0.35).max(40.0);
                        cover_tile::thin_bar(ui, theme, n as f32 / a.total().max(1) as f32, bar_w);
                    }
                    ui.add(egui::Label::new(RichText::new(words).size(theme.font_size_small).color(theme.text_secondary())).truncate());
                });
            }
            None => {
                ui.allocate_exact_size(Vec2::new(w, line_h), Sense::hover());
            }
        }
        ui.add_space(theme.spacing_xs);
        if game.play.is_some() {
            if widgets::Button::primary(super::PLAY).full_width().min_height(play_height(theme)).tooltip(super::play_tip(game.store)).show(ui, theme) {
                acts.push(Act::Play(game.clone()));
            }
        } else {
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, play_height(theme)), Sense::hover());
            let words = super::why_not_short(game).unwrap_or_default();
            let galley = ui.painter().layout_no_wrap(words, egui::FontId::proportional(theme.font_size_small), theme.warning());
            let at = egui::pos2(rect.min.x, rect.center().y - galley.size().y / 2.0);
            ui.painter().with_clip_rect(rect).galley(at, galley, theme.warning());
            if let Some(full) = super::why_not_words(game) {
                resp.on_hover_text(full);
            }
        }
    });
}

/// The list's columns, each shown only when the room allows: (its heading, its width, the sort
/// its heading sets).
#[derive(Clone, Copy, PartialEq)]
enum Column {
    Store,
    Played,
    LastPlayed,
    Size,
    Achievements,
}

impl Column {
    fn heading(self) -> &'static str {
        match self {
            Column::Store => "Store",
            Column::Played => "Played",
            Column::LastPlayed => "Last played",
            Column::Size => "Size",
            Column::Achievements => "Achievements",
        }
    }

    fn width(self) -> f32 {
        match self {
            Column::Store => 130.0,
            Column::Played => 96.0,
            Column::LastPlayed => 110.0,
            Column::Size => 84.0,
            Column::Achievements => 120.0,
        }
    }

    fn sort(self) -> Option<SortBy> {
        match self {
            Column::Played => Some(SortBy::MostPlayed),
            Column::LastPlayed => Some(SortBy::RecentlyPlayed),
            Column::Size => Some(SortBy::Size),
            Column::Store | Column::Achievements => None,
        }
    }

    fn value(self, game: &Game) -> String {
        let d = &game.details;
        match self {
            Column::Store => game.store.name().to_string(),
            Column::Played => d.played_minutes.filter(|m| *m > 0).map(super::played_words).unwrap_or_default(),
            Column::LastPlayed => d.last_played.map(|t| super::day_words(t, true)).unwrap_or_default(),
            Column::Size => d.size.map(super::size_words).unwrap_or_default(),
            Column::Achievements => d.achievements.as_ref().map(|a| match a.unlocked() {
                Some(n) => format!("{n} of {}", a.total()),
                None => format!("{}", a.total()),
            }).unwrap_or_default(),
        }
    }
}

/// The Play cell's width, the name's least, and the room kept for the store's own button.
const PLAY_W: f32 = 96.0;
const NAME_MIN: f32 = 200.0;
const STORE_BUTTON_W: f32 = 210.0;

/// The columns that fit in `width`, most wanted first, in the order drawn.
fn columns_for(width: f32) -> Vec<Column> {
    let wanted: Vec<(Column, f32)> = [Column::Store, Column::Played, Column::Size, Column::Achievements, Column::LastPlayed].iter().map(|c| (*c, c.width())).collect();
    let drawn = [Column::Store, Column::Played, Column::LastPlayed, Column::Size, Column::Achievements];
    shelf::columns_that_fit(width - PLAY_W - NAME_MIN - STORE_BUTTON_W, &wanted, &drawn)
}

/// The list: a heading row (a click on a heading with a number sorts by it), then a row a game.
fn list(ui: &mut egui::Ui, theme: &Theme, page: &GamesState, order: &[usize], sort: SortBy, acts: &mut Vec<Act>) {
    let width = ui.available_width();
    let columns = columns_for(width);
    let gap = ui.spacing().item_spacing.x;
    let fixed: f32 = PLAY_W + columns.iter().map(|c| c.width()).sum::<f32>() + STORE_BUTTON_W + gap * (columns.len() as f32 + 3.0);
    let name_w = (width - fixed).max(NAME_MIN * 0.6);
    let row_h = play_height(theme) + theme.spacing_sm;

    // The headings: none over Play, then Name and each column (a click on one with a number sorts).
    let mut cells = vec![Heading { text: "", width: PLAY_W, sorts: false, on: false }, Heading { text: "Name", width: name_w, sorts: true, on: sort == SortBy::Name }];
    cells.extend(columns.iter().map(|c| Heading { text: c.heading(), width: c.width(), sorts: c.sort().is_some(), on: c.sort() == Some(sort) }));
    if let Some(clicked) = shelf::list_headings(ui, theme, &cells) {
        let chosen = if clicked == 1 { Some(SortBy::Name) } else { clicked.checked_sub(2).and_then(|c| columns.get(c)).and_then(|c| c.sort()) };
        if let Some(s) = chosen {
            acts.push(Act::Sort(s));
        }
    }

    for (n, &i) in order.iter().enumerate() {
        let game = &page.all[i];
        let selected = is_selected(page, game);
        shelf::list_row(ui, theme, n, width, row_h, selected, |ui| {
            cover_tile::fixed_cell(ui, PLAY_W, row_h, |ui| {
                if game.play.is_some() {
                    if widgets::Button::primary(super::PLAY).min_height(play_height(theme)).tooltip(super::play_tip(game.store)).show(ui, theme) {
                        acts.push(Act::Play(game.clone()));
                    }
                } else if let Some(tag) = super::why_not_tag(game) {
                    let resp = ui.add(egui::Label::new(RichText::new(tag).size(theme.font_size_small).color(theme.warning())).truncate());
                    if let Some(full) = super::why_not_words(game) {
                        resp.on_hover_text(full);
                    }
                }
            });
            cover_tile::fixed_cell(ui, name_w, row_h, |ui| {
                let name = ui.add(egui::Label::new(RichText::new(&game.name).size(theme.font_size_body).strong().color(theme.text_primary())).truncate().sense(Sense::click()));
                if name.clicked() {
                    acts.push(Act::Select(game.store, game.id.clone()));
                }
                if name.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            });
            for c in &columns {
                cover_tile::fixed_cell(ui, c.width(), row_h, |ui| {
                    let color = if *c == Column::Store { theme.text_muted() } else { theme.text_secondary() };
                    ui.add(egui::Label::new(RichText::new(c.value(game)).size(theme.font_size_small).color(color)).truncate());
                });
            }
            if let Some(b) = game.store_button() {
                if widgets::Button::secondary(b.label).size(widgets::ButtonSize::Small).tooltip(b.tip).show(ui, theme) {
                    acts.push(Act::Open(b.open));
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE LIST'S COLUMNS GIVE WAY WHEN THE ROOM DOES: all five on a wide page, in their order; on
    /// a narrow one the least wanted go first (Last played, then Achievements), Store last of all.
    /// The narrow room here (336) would hold Store, Played and Last played if they were taken in
    /// the order drawn, so it tells the two apart.
    ///
    /// A first version (room 320) stayed GREEN with the columns taken in the order drawn: that room
    /// gave the same three either way. At 336 it went red, 2026-10-19: "the least wanted go first"
    /// failed, Last played kept and Size dropped.
    #[test]
    fn the_lists_columns_give_way() {
        assert!(columns_for(1400.0) == vec![Column::Store, Column::Played, Column::LastPlayed, Column::Size, Column::Achievements], "all five, in their order");
        let narrow = columns_for(PLAY_W + NAME_MIN + STORE_BUTTON_W + 130.0 + 96.0 + 110.0);
        assert!(narrow == vec![Column::Store, Column::Played, Column::Size], "the least wanted go first");
        assert!(columns_for(300.0).is_empty());
    }
}
