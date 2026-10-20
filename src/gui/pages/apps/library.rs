//! The Apps page's toolbar and its six parts (2026-10-19): the search, the sort, Grid or List, a
//! chip per part (in its colour, with its name) and "Only what is on this computer"; then each
//! part under its band (its colour and its name: never the colour alone), as a Grid of tiles or a
//! List of rows, with its own lines below. Drawn from the page's state as it is; what a click asks
//! for is returned as an `Act` and done by apps.rs after the frame's drawing. The line, the grid, a
//! tile's card and the List's headings and rows are the shelf (widgets/shelf.rs); a cover is
//! widgets/cover_tile.rs; the colours are widgets/tint_section.rs's.
//!
//! A TILE IS SHORT (the operator, 2026-10-19: "I kinda feel like the app tiles are too big. The
//! titles are stated twice, we don't need that. We could make them short vertically by removing
//! some superfluousness/spacing."): the name is drawn ONCE, on the plain cover when there is no
//! picture, else in one small line under the picture; then one line of where it is from and one
//! fact; then a small button with, at its right, one more fact.

use egui::{Color32, Rect, RichText, Sense, Stroke, Ui, Vec2};

use super::{games, own_tools, programs, Act, AppsState, Picked, Section, SortBy, View};
use crate::games::{Game, Store};
use crate::gui::theme::Theme;
use crate::gui::widgets::shelf::{self, Heading, Layout};
use crate::gui::widgets::tint_section;
use crate::gui::widgets::{self, cover_tile, icons, thumbnails};
use crate::gui::GuiState;
use crate::software::CatalogEntry;

/// A tile's width at the least (tiles stretch to fill a row).
pub(super) const TILE_W: f32 = 168.0;
/// The cover slot's shape: Steam's wide library picture, 460 by 215, so its artwork fills it.
pub(super) const COVER_ASPECT: f32 = 460.0 / 215.0;

/// The parts' orders and what they were worked out from (worked out again only when that changes).
#[derive(Default)]
pub(crate) struct Orders {
    key: Option<OrderKey>,
    /// The games (indexes into `GamesPart::all`).
    pub games: Vec<usize>,
    /// HumanityOS's own tools (into `AppsState::own_tools`).
    pub tools: Vec<usize>,
    /// The other programs, help and services (into the catalog).
    pub programs: Vec<usize>,
    pub help: Vec<usize>,
    pub services: Vec<usize>,
    /// What was added (into `GamesPart::all`).
    pub added: Vec<usize>,
}

impl Orders {
    /// A catalog part's order.
    pub(crate) fn of(&self, section: Section) -> &[usize] {
        match section {
            Section::Games => &self.games,
            Section::Tools => &self.tools,
            Section::Programs => &self.programs,
            Section::Help => &self.help,
            Section::Services => &self.services,
            Section::Added => &self.added,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct OrderKey {
    games: u64,
    programs: u64,
    entries: usize,
    tools: usize,
    search: String,
    view: View,
    store: Option<Store>,
}

/// Work the orders out again when what they depend on changed (not every frame).
pub(super) fn ensure_orders(state: &mut GuiState) {
    let apps = &state.apps;
    let key = OrderKey {
        games: apps.games.generation,
        programs: apps.programs.generation,
        entries: state.external_catalog.len(),
        tools: apps.own_tools.len(),
        search: apps.search.trim().to_lowercase(),
        view: state.settings.apps_view.clone(),
        store: apps.games.store,
    };
    if apps.orders.key.as_ref() == Some(&key) {
        return;
    }
    let v = &key.view;
    let found = apps.programs.found.as_ref();
    let catalog = |section: Section| {
        let (kind, part) = section.catalog().unwrap_or(("", None));
        let installed_only = v.installed_only && section == Section::Programs;
        programs::order_of(&state.external_catalog, found, kind, part, v.category(section), installed_only, v.sort, &key.search)
    };
    let orders = Orders {
        games: games::order_of(&apps.games.all, v.sort, &key.search, key.store.filter(|s| *s != Store::Added)),
        tools: own_tools::order_of(&apps.own_tools, &key.search),
        programs: catalog(Section::Programs),
        help: catalog(Section::Help),
        services: catalog(Section::Services),
        added: games::order_of(&apps.games.all, SortBy::Name, &key.search, Some(Store::Added)),
        key: Some(key),
    };
    state.apps.orders = orders;
}

/// A small button's height, and the height of what stands in its place, so the tiles of a row
/// line up.
fn button_height(theme: &Theme) -> f32 {
    theme.font_size_small + 2.0 * theme.button_pad_y + 2.0
}

/// The sorts that order a part shown (Name always).
fn sorts_for(view: &View) -> Vec<SortBy> {
    SortBy::ALL.iter().copied().filter(|s| Section::ALL.iter().any(|sec| view.shows(*sec) && s.orders(*sec))).collect()
}

/// A part's band colour (the dark fill, as the Edit form's parts) and its edge (the bright one).
fn colours(theme: &Theme, section: Section) -> (Color32, Color32) {
    (tint_section::color(theme, section.tint()), tint_section::edge(theme, section.tint()))
}

/// A chip: `on` filled in its colour (the accent for one with none).
fn chip(ui: &mut Ui, theme: &Theme, label: &str, on: bool, colour: Option<(Color32, Color32)>, tip: &str) -> bool {
    let (fill, edge) = colour.unwrap_or((theme.accent(), theme.accent()));
    let (fill, text, stroke) = if on {
        (fill, if colour.is_some() { theme.text_strong() } else { theme.text_on_accent() }, edge)
    } else {
        (theme.bg_secondary(), theme.text_secondary(), if colour.is_some() { edge.gamma_multiply(0.6) } else { theme.border() })
    };
    let button = egui::Button::new(RichText::new(label).size(theme.font_size_small).color(text))
        .fill(fill)
        .stroke(Stroke::new(1.0, stroke))
        .corner_radius(egui::CornerRadius::same(theme.pill_radius.round().clamp(0.0, 255.0) as u8));
    ui.add(button).on_hover_text(tip).clicked()
}

/// How many a part holds (before the search narrows it).
pub(super) fn count(state: &GuiState, section: Section) -> usize {
    match section {
        Section::Games => state.apps.games.all.iter().filter(|g| g.store != Store::Added).count(),
        Section::Added => state.apps.games.all.iter().filter(|g| g.store == Store::Added).count(),
        Section::Tools => state.apps.own_tools.len(),
        catalog_part => {
            let (kind, part) = catalog_part.catalog().unwrap_or(("", None));
            state.external_catalog.iter().filter(|e| e.kind == kind && part.is_none_or(|p| e.section == p)).count()
        }
    }
}

/// The toolbar: the search, the sort, Grid or List; then the chips: All, a part each (in its
/// colour, with its name and how many), and only what is on this computer.
pub(super) fn toolbar(ui: &mut Ui, theme: &Theme, state: &mut GuiState) -> Vec<Act> {
    let mut acts = Vec::new();
    let mut view = state.settings.apps_view.clone();
    let sorts = sorts_for(&view);
    let shown = if sorts.contains(&view.sort) { view.sort } else { SortBy::Name };
    let pills: Vec<(SortBy, &str, &str)> = sorts.iter().map(|s| (*s, s.words(), s.tip())).collect();
    let picked = shelf::toolbar(ui, theme, &mut state.apps.search, super::SEARCH_HINT, &pills, shown, view.layout, ["A tile for each.", "A row for each, its facts in columns."]);
    if let Some(sort) = picked.sort {
        view.sort = sort;
    }
    if let Some(layout) = picked.layout {
        view.layout = layout;
    }
    ui.add_space(theme.spacing_xs);
    ui.horizontal_wrapped(|ui| {
        let total: usize = Section::ALL.iter().map(|s| count(state, *s)).sum();
        if chip(ui, theme, &format!("{} {total}", super::ALL), view.section.is_none(), None, "Every part.") {
            view.section = None;
        }
        for s in Section::ALL {
            if chip(ui, theme, &format!("{} {}", s.words(), count(state, s)), view.section == Some(s), Some(colours(theme, s)), s.tip()) {
                view.section = if view.section == Some(s) { None } else { Some(s) };
            }
        }
        ui.add_space(theme.spacing_md);
        if chip(ui, theme, super::INSTALLED_ONLY, view.installed_only, None, "Leave out the help and services, and the other programs not found on this computer. Games, HumanityOS's own tools and what you added always are.") {
            view.installed_only = !view.installed_only;
        }
    });
    ui.add_space(theme.spacing_xs);
    if view != state.settings.apps_view {
        acts.push(Act::View(view));
    }
    acts
}

/// The categories of a catalog part's entries, in the catalog's order: (id, name).
fn categories(catalog: &[CatalogEntry], section: Section) -> Vec<(String, String)> {
    let (kind, part) = section.catalog().unwrap_or(("", None));
    let mut out: Vec<(String, String)> = Vec::new();
    for e in catalog.iter().filter(|e| e.kind == kind && part.is_none_or(|p| e.section == p)) {
        if !out.iter().any(|(id, _)| *id == e.category_id) {
            out.push((e.category_id.clone(), e.category.clone()));
        }
    }
    out
}

/// "All" and a pill per category; `Some(choice)` when one is clicked (`Some(None)`: All).
fn category_pills(ui: &mut Ui, theme: &Theme, categories: &[(String, String)], chosen: Option<&str>) -> Option<Option<String>> {
    let mut out = None;
    ui.horizontal_wrapped(|ui| {
        let mut labels = vec![super::ALL.to_string()];
        labels.extend(categories.iter().map(|(_, name)| name.clone()));
        let at = chosen.and_then(|c| categories.iter().position(|(id, _)| id == c)).map_or(0, |p| p + 1);
        if let Some(i) = cover_tile::choice_pills(ui, theme, &labels, at, &[]) {
            out = Some(if i == 0 { None } else { Some(categories[i - 1].0.clone()) });
        }
    });
    out
}

/// A part's band: its name and how many, in its colour (never the colour alone), and `right` at
/// its right end.
fn band(ui: &mut Ui, theme: &Theme, section: Section, count: usize, right: impl FnOnce(&mut Ui)) {
    let (fill, edge) = colours(theme, section);
    let title = section.words();
    ui.add_space(theme.spacing_sm);
    egui::Frame::NONE
        .fill(fill)
        .stroke(Stroke::new(1.0, edge))
        .corner_radius(egui::CornerRadius::same(theme.border_radius as u8))
        .inner_margin(egui::Margin::symmetric(theme.spacing_md as i8, theme.spacing_xs as i8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(title).size(theme.font_size_heading).strong().color(theme.text_strong()));
                ui.label(RichText::new(count.to_string()).size(theme.font_size_body).color(theme.text_secondary()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), right);
            });
        });
    ui.add_space(theme.spacing_sm);
}

/// A line with its Why at the end (a dated finding in the Library).
fn line_with_why(ui: &mut Ui, theme: &Theme, text: &str, tip: &str, why: (&'static str, &'static str), acts: &mut Vec<Act>) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(text).size(theme.font_size_small).color(theme.text_secondary()));
        if widgets::Button::secondary(super::WHY).size(widgets::ButtonSize::Small).tooltip(tip).show(ui, theme) {
            acts.push(Act::Why(why.0, why.1));
        }
    });
}

/// The parts shown, in their order: red, green, blue, cyan, magenta, yellow. A catalog part or
/// HumanityOS's own tools with nothing to list (its data not read) is left out; the games and what
/// you added always show, each saying how to fill it.
pub(super) fn parts(ui: &mut Ui, theme: &Theme, state: &GuiState) -> Vec<Act> {
    let mut acts = Vec::new();
    let view = &state.settings.apps_view;
    for section in Section::ALL {
        if !view.shows(section) {
            continue;
        }
        match section {
            Section::Games => games_part(ui, theme, state, &mut acts),
            Section::Tools if count(state, section) > 0 => tools_part(ui, theme, state, &mut acts),
            Section::Added => added_part(ui, theme, state, &mut acts),
            s if s.catalog().is_some() && count(state, s) > 0 => catalog_part(ui, theme, state, s, &mut acts),
            _ => {}
        }
    }
    acts
}

// ---------------------------------------------------------------------------------------------
// Games (red)
// ---------------------------------------------------------------------------------------------

fn games_part(ui: &mut Ui, theme: &Theme, state: &GuiState, acts: &mut Vec<Act>) {
    let page = &state.apps;
    band(ui, theme, Section::Games, count(state, Section::Games), |_| {});
    let Some(found) = &page.games.found else {
        ui.label(RichText::new(games::READING).size(theme.font_size_small).color(theme.text_muted()));
        return;
    };
    // A pill per store found with its count (a store not installed has none).
    let mut stores = Vec::new();
    for (store, list) in [(Store::Steam, &found.steam), (Store::Epic, &found.epic), (Store::Gog, &found.gog)] {
        if list.is_some() {
            stores.push(store);
        }
    }
    if !stores.is_empty() {
        ui.horizontal_wrapped(|ui| {
            let n = |store: Option<Store>| page.games.all.iter().filter(|g| g.store != Store::Added && store.is_none_or(|s| g.store == s)).count();
            let mut options = vec![(None, format!("{} {}", super::ALL, n(None)))];
            options.extend(stores.iter().map(|s| (Some(*s), format!("{} {}", s.name(), n(Some(*s))))));
            let labels: Vec<String> = options.iter().map(|(_, l)| l.clone()).collect();
            let chosen = options.iter().position(|(s, _)| *s == page.games.store).unwrap_or(0);
            if let Some(i) = cover_tile::choice_pills(ui, theme, &labels, chosen, &[]) {
                acts.push(Act::Store(options[i].0));
            }
        });
        ui.add_space(theme.spacing_xs);
    } else {
        ui.label(RichText::new(games::NO_STORES).size(theme.font_size_small).color(theme.text_muted()));
    }
    let order = &page.orders.games;
    if order.is_empty() {
        if let Some(words) = games_empty_words(page) {
            ui.label(RichText::new(words).size(theme.font_size_small).color(theme.text_muted()));
        }
    } else {
        games_shelf(ui, theme, state, order, acts);
    }
    // Its lines: what may be done and why, each with its Why; what is never done; the covers
    // switch, when there is an Epic game.
    ui.add_space(theme.spacing_xs);
    line_with_why(ui, theme, games::STORES_LINE, "What Steam's, Epic's and GOG's own terms say, as researched on 13 October 2026, in the Library.", (games::FINDING_SLUG, games::FINDING_PATH), acts);
    line_with_why(ui, theme, games::PICTURES_LINE, "What the stores and the law say about game pictures, store names and logos, play records and links, as researched on 19 October 2026, in the Library.", (games::PICTURES_FINDING_SLUG, games::PICTURES_FINDING_PATH), acts);
    widgets::body_hint(ui, theme, games::NOTHING_BY_ITSELF);
    ui.label(RichText::new(games::NOT_AFFILIATED).size(theme.font_size_small).color(theme.text_muted()));
    if page.games.all.iter().any(super::covers::EpicCovers::has_covers) {
        let mut on = state.settings.apps_epic_covers;
        let resp = ui.checkbox(&mut on, RichText::new(super::covers::SWITCH).size(theme.font_size_small).color(theme.text_secondary())).on_hover_text(super::covers::SWITCH_TIP);
        if resp.changed() {
            acts.push(Act::EpicCovers(on));
        }
        if on && page.covers.failed > 0 {
            ui.label(RichText::new(format!("{} of the covers could not be fetched.", page.covers.failed)).size(theme.font_size_small).color(theme.text_muted()));
        }
    }
}

/// Games (or what was added), in `order`, as tiles or rows.
fn games_shelf(ui: &mut Ui, theme: &Theme, state: &GuiState, order: &[usize], acts: &mut Vec<Act>) {
    let page = &state.apps;
    let sort = state.settings.apps_view.sort;
    match state.settings.apps_view.layout {
        Layout::Grid => shelf::grid(ui, theme, order, TILE_W, |ui, i, w| game_tile(ui, theme, page, &page.games.all[i], w, sort, acts)),
        Layout::List => games_list(ui, theme, page, &state.settings.apps_view, order, acts),
    }
}

/// What the games say when none is shown.
fn games_empty_words(page: &AppsState) -> Option<String> {
    if !page.search.trim().is_empty() {
        return Some(format!("No game's name holds \"{}\".", page.search.trim()));
    }
    match page.games.store {
        Some(Store::Steam) => Some("No Steam games are installed on this computer.".to_string()),
        Some(Store::Epic) => Some("No games from the Epic Games Store are installed on this computer.".to_string()),
        Some(Store::Gog) => Some("No GOG games are installed on this computer.".to_string()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Added by you (yellow)
// ---------------------------------------------------------------------------------------------

/// What the person added, games or any other program, with "Add a program" on its band.
fn added_part(ui: &mut Ui, theme: &Theme, state: &GuiState, acts: &mut Vec<Act>) {
    let page = &state.apps;
    band(ui, theme, Section::Added, count(state, Section::Added), |ui| {
        if widgets::Button::primary(games::ADD).size(widgets::ButtonSize::Small).tooltip("Add any game or program on this computer by its program, such as one downloaded from its maker's own website.").show(ui, theme) {
            acts.push(Act::Add);
        }
    });
    let order = &page.orders.added;
    if order.is_empty() {
        let words = if page.games.added.is_empty() { games::ADDED_NONE.to_string() } else { format!("Nothing you added holds \"{}\".", page.search.trim()) };
        ui.label(RichText::new(words).size(theme.font_size_small).color(theme.text_muted()));
    } else {
        games_shelf(ui, theme, state, order, acts);
    }
}

/// The one number a game's tile shows beside its store: the one the games are sorted by, or (sorted
/// otherwise) the time played, else the size.
fn tile_number(game: &Game, sort: SortBy) -> Option<String> {
    let d = &game.details;
    match sort {
        SortBy::RecentlyPlayed => d.last_played.map(|t| format!("Played {}", games::day_words(t, true))),
        SortBy::MostPlayed => d.played_minutes.filter(|m| *m > 0).map(games::played_words),
        SortBy::Size => d.size.map(games::size_words),
        _ => d.played_minutes.filter(|m| *m > 0).map(games::played_words).or_else(|| d.size.map(games::size_words)),
    }
}

/// A tile's cover: `picture` cropped to fill, or the plain panel in `seed`'s hue with `name` on it
/// (and `mark`, a program's icon, at its top right). While a picture it has is still being read
/// (`has_art` without `picture`) the panel is drawn without the name, which is then under it.
#[allow(clippy::too_many_arguments)]
fn cover(ui: &mut Ui, theme: &Theme, w: f32, name: &str, seed: &str, picture: Option<(egui::TextureId, Vec2)>, has_art: bool, mark: Option<(egui::TextureId, Vec2)>) -> (Rect, egui::Response) {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, (w / COVER_ASPECT).round()), Sense::click());
    let title = if has_art { "" } else { name };
    cover_tile::paint_cover(ui.painter(), theme, rect, picture, mark, title, seed, theme.font_size_body, theme.border_radius);
    (rect, resp)
}

/// The small name under a picture (a tile with one only: the plain cover carries the name itself).
fn name_line(ui: &mut Ui, theme: &Theme, name: &str) -> egui::Response {
    ui.add(egui::Label::new(RichText::new(name).size(theme.font_size_small).strong().color(theme.text_primary())).truncate().sense(Sense::click()))
}

/// The line under the cover: where it is from and one fact.
fn meta_line(ui: &mut Ui, theme: &Theme, text: &str, colour: Color32) -> egui::Response {
    ui.add(egui::Label::new(RichText::new(text).size(theme.font_size_small).color(colour)).truncate())
}

/// The tile's last row, `w` wide: the main button (or what stands in its place) at the left, and
/// at the right a fact, in `colour`, with its hover words.
fn button_row(ui: &mut Ui, theme: &Theme, w: f32, left: impl FnOnce(&mut Ui), right: Option<(String, Color32)>) {
    ui.allocate_ui_with_layout(Vec2::new(w, button_height(theme)), egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.set_min_height(button_height(theme));
        left(ui);
        if let Some((words, colour)) = right {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(egui::Label::new(RichText::new(words).size(theme.font_size_small).color(colour)).truncate());
            });
        }
    });
}

/// Words in a button's place, in `colour`, cut to fit, the whole sentence their hover words.
fn in_buttons_place(ui: &mut Ui, theme: &Theme, short: &str, full: Option<String>, colour: Color32) {
    let resp = ui.add(egui::Label::new(RichText::new(short).size(theme.font_size_small).color(colour)).truncate());
    if let Some(full) = full {
        resp.on_hover_text(full);
    }
}

/// A slim bar along the bottom of a cover: `fraction` of it in the accent (a game's achievements).
fn cover_bar(ui: &Ui, theme: &Theme, cover: Rect, fraction: f32) {
    let h = theme.spacing_xs.max(3.0);
    let pad = theme.spacing_xs;
    let track = Rect::from_min_max(egui::pos2(cover.left() + pad, cover.bottom() - pad - h), egui::pos2(cover.right() - pad, cover.bottom() - pad));
    let corners = egui::CornerRadius::same((h / 2.0).round() as u8);
    ui.painter().rect_filled(track, corners, theme.slider_track());
    let f = fraction.clamp(0.0, 1.0);
    if f > 0.0 {
        ui.painter().rect_filled(Rect::from_min_size(track.min, Vec2::new((track.width() * f).max(h), h)), corners, theme.accent());
    }
}

/// One game's tile.
fn game_tile(ui: &mut Ui, theme: &Theme, page: &AppsState, game: &Game, width: f32, sort: SortBy, acts: &mut Vec<Act>) {
    let picked = Picked::Game(game.store, game.id.clone());
    shelf::tile_frame(ui, theme, width, page.selected.as_ref() == Some(&picked), |ui, w| {
        // Epic's cover (only when the person turned that on, apps/covers.rs), else the picture
        // Steam's client cached, read on thumbnails' own workers.
        let remote = page.covers.tile_of(game);
        let cached = game.details.art.cover.as_deref();
        let picture = remote.or_else(|| cached.and_then(|p| thumbnails::get(ui.ctx(), p, w)));
        let has_art = remote.is_some() || cached.is_some();
        let (rect, cover_resp) = cover(ui, theme, w, &game.name, &game.id, picture, has_art, page.games.icon_of(game));
        let mut clicked = cover_resp.clicked();
        let mut hovered = cover_resp.hovered();
        if let Some(n) = game.details.achievements.as_ref().and_then(|a| a.unlocked().map(|n| n as f32 / a.total().max(1) as f32)) {
            cover_bar(ui, theme, rect, n);
        }
        if has_art {
            let name = name_line(ui, theme, &game.name);
            clicked |= name.clicked();
            hovered |= name.hovered();
        }
        // Where it is from and one number; for what was added (its part says it was), its program.
        let meta = match (game.store, tile_number(game, sort)) {
            (Store::Added, _) => game.show.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            (store, Some(n)) => format!("{} \u{b7} {n}", store.name()),
            (store, None) => store.name().to_string(),
        };
        meta_line(ui, theme, &meta, theme.text_muted());
        let achievements = game.details.achievements.as_ref().map(|a| (games::achievements_words(a), theme.text_secondary()));
        button_row(
            ui,
            theme,
            w,
            |ui| {
                if game.play.is_some() {
                    if widgets::Button::primary(games::play_words(game.store)).size(widgets::ButtonSize::Small).tooltip(games::play_tip(game.store)).show(ui, theme) {
                        acts.push(Act::Play(game.clone()));
                    }
                } else {
                    in_buttons_place(ui, theme, &games::why_not_short(game).unwrap_or_default(), games::why_not_words(game), theme.warning());
                }
            },
            if game.play.is_some() { achievements } else { None },
        );
        if clicked {
            acts.push(Act::Select(picked.clone()));
        }
        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    });
}

/// The games' List columns, each shown only when the room allows.
#[derive(Clone, Copy, PartialEq)]
enum GameColumn {
    Store,
    Played,
    LastPlayed,
    Size,
    Achievements,
}

impl GameColumn {
    fn heading(self) -> &'static str {
        match self {
            GameColumn::Store => "Store",
            GameColumn::Played => "Played",
            GameColumn::LastPlayed => "Last played",
            GameColumn::Size => "Size",
            GameColumn::Achievements => "Achievements",
        }
    }

    fn width(self) -> f32 {
        match self {
            GameColumn::Store => 130.0,
            GameColumn::Played => 96.0,
            GameColumn::LastPlayed => 110.0,
            GameColumn::Size => 84.0,
            GameColumn::Achievements => 120.0,
        }
    }

    fn sort(self) -> Option<SortBy> {
        match self {
            GameColumn::Played => Some(SortBy::MostPlayed),
            GameColumn::LastPlayed => Some(SortBy::RecentlyPlayed),
            GameColumn::Size => Some(SortBy::Size),
            GameColumn::Store | GameColumn::Achievements => None,
        }
    }

    fn value(self, game: &Game) -> String {
        let d = &game.details;
        match self {
            GameColumn::Store => game.store.name().to_string(),
            GameColumn::Played => d.played_minutes.filter(|m| *m > 0).map(games::played_words).unwrap_or_default(),
            GameColumn::LastPlayed => d.last_played.map(|t| games::day_words(t, true)).unwrap_or_default(),
            GameColumn::Size => d.size.map(games::size_words).unwrap_or_default(),
            GameColumn::Achievements => d
                .achievements
                .as_ref()
                .map(|a| match a.unlocked() {
                    Some(n) => format!("{n} of {}", a.total()),
                    None => format!("{}", a.total()),
                })
                .unwrap_or_default(),
        }
    }
}

/// The Play cell's width, a name's least, and the room kept for the store's own button.
const PLAY_W: f32 = 96.0;
const NAME_MIN: f32 = 200.0;
const STORE_BUTTON_W: f32 = 210.0;

/// The games' columns that fit in `width`, most wanted first, in the order drawn.
fn game_columns_for(width: f32) -> Vec<GameColumn> {
    use GameColumn as C;
    let wanted: Vec<(C, f32)> = [C::Store, C::Played, C::Size, C::Achievements, C::LastPlayed].iter().map(|c| (*c, c.width())).collect();
    shelf::columns_that_fit(width - PLAY_W - NAME_MIN - STORE_BUTTON_W, &wanted, &[C::Store, C::Played, C::LastPlayed, C::Size, C::Achievements])
}

/// A list row's height.
fn row_height(theme: &Theme) -> f32 {
    theme.font_size_body + 2.0 * theme.button_pad_y + theme.spacing_xs + theme.spacing_sm
}

/// A click on a list heading that sorts: the view with that sort.
fn sort_act(view: &View, sort: SortBy) -> Act {
    Act::View(View { sort, ..view.clone() })
}

/// The games' List: a heading row (a click on a heading with a number sorts by it), then a row a
/// game: Play, its name, its numbers, the store's own button.
fn games_list(ui: &mut Ui, theme: &Theme, page: &AppsState, view: &View, order: &[usize], acts: &mut Vec<Act>) {
    let sort = view.sort;
    let width = ui.available_width();
    let columns = game_columns_for(width);
    let gap = ui.spacing().item_spacing.x;
    let fixed: f32 = PLAY_W + columns.iter().map(|c| c.width()).sum::<f32>() + STORE_BUTTON_W + gap * (columns.len() as f32 + 3.0);
    let name_w = (width - fixed).max(NAME_MIN * 0.6);
    let row_h = row_height(theme);
    let mut cells = vec![Heading { text: "", width: PLAY_W, sorts: false, on: false }, Heading { text: "Name", width: name_w, sorts: true, on: sort == SortBy::Name }];
    cells.extend(columns.iter().map(|c| Heading { text: c.heading(), width: c.width(), sorts: c.sort().is_some(), on: c.sort() == Some(sort) }));
    if let Some(clicked) = shelf::list_headings(ui, theme, &cells) {
        let chosen = if clicked == 1 { Some(SortBy::Name) } else { clicked.checked_sub(2).and_then(|c| columns.get(c)).and_then(|c| c.sort()) };
        if let Some(s) = chosen {
            acts.push(sort_act(view, s));
        }
    }
    for (n, &i) in order.iter().enumerate() {
        let game = &page.games.all[i];
        let selected = page.selected.as_ref() == Some(&Picked::Game(game.store, game.id.clone()));
        shelf::list_row(ui, theme, n, width, row_h, selected, |ui| {
            cover_tile::fixed_cell(ui, PLAY_W, row_h, |ui| {
                if game.play.is_some() {
                    if widgets::Button::primary(games::play_words(game.store)).tooltip(games::play_tip(game.store)).show(ui, theme) {
                        acts.push(Act::Play(game.clone()));
                    }
                } else if let Some(tag) = games::why_not_tag(game) {
                    in_buttons_place(ui, theme, tag, games::why_not_words(game), theme.warning());
                }
            });
            cover_tile::fixed_cell(ui, name_w, row_h, |ui| {
                let name = ui.add(egui::Label::new(RichText::new(&game.name).size(theme.font_size_body).strong().color(theme.text_primary())).truncate().sense(Sense::click()));
                if name.clicked() {
                    acts.push(Act::Select(Picked::Game(game.store, game.id.clone())));
                }
                if name.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            });
            for c in &columns {
                cover_tile::fixed_cell(ui, c.width(), row_h, |ui| {
                    let colour = if *c == GameColumn::Store { theme.text_muted() } else { theme.text_secondary() };
                    ui.add(egui::Label::new(RichText::new(c.value(game)).size(theme.font_size_small).color(colour)).truncate());
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

// ---------------------------------------------------------------------------------------------
// HumanityOS tools (green)
// ---------------------------------------------------------------------------------------------

fn tools_part(ui: &mut Ui, theme: &Theme, state: &GuiState, acts: &mut Vec<Act>) {
    let page = &state.apps;
    band(ui, theme, Section::Tools, page.own_tools.len(), |_| {});
    let order = &page.orders.tools;
    if order.is_empty() {
        ui.label(RichText::new(format!("No HumanityOS tool holds \"{}\".", page.search.trim())).size(theme.font_size_small).color(theme.text_muted()));
        return;
    }
    match state.settings.apps_view.layout {
        Layout::Grid => shelf::grid(ui, theme, order, TILE_W, |ui, i, w| tool_tile(ui, theme, page, i, w, acts)),
        Layout::List => tools_list(ui, theme, page, order, acts),
    }
}

/// The Open button of one of HumanityOS's own tools.
fn open_button(ui: &mut Ui, theme: &Theme, tool: &own_tools::OwnTool, size: widgets::ButtonSize, acts: &mut Vec<Act>) {
    match tool.gui_page() {
        Some(page) => {
            if widgets::Button::primary(own_tools::OPEN).size(size).tooltip("Opens its page here.").show(ui, theme) {
                acts.push(Act::OpenPage(page));
            }
        }
        None => in_buttons_place(ui, theme, "Not in this version", Some(format!("This version of HumanityOS has no page named \"{}\".", tool.page)), theme.text_muted()),
    }
}

/// One of HumanityOS's own tools: its plain cover with its name and its page's own mark, its line,
/// and Open.
fn tool_tile(ui: &mut Ui, theme: &Theme, page: &AppsState, i: usize, width: f32, acts: &mut Vec<Act>) {
    let tool = &page.own_tools[i];
    shelf::tile_frame(ui, theme, width, page.selected == Some(Picked::Tool(i)), |ui, w| {
        let (rect, cover_resp) = cover(ui, theme, w, &tool.name, &tool.page, None, false, None);
        if let Some(gp) = tool.gui_page() {
            let side = (rect.height() * 0.34).round();
            let pad = (rect.height() * 0.09).clamp(theme.spacing_xs, theme.spacing_md);
            let at = Rect::from_min_size(egui::pos2(rect.right() - pad - side, rect.top() + pad), Vec2::splat(side));
            icons::paint_nav_icon(ui.painter(), at, gp, theme.text_strong().gamma_multiply(0.85));
        }
        meta_line(ui, theme, &tool.line, theme.text_muted()).on_hover_text(&tool.line);
        button_row(ui, theme, w, |ui| open_button(ui, theme, tool, widgets::ButtonSize::Small, acts), None);
        if cover_resp.clicked() {
            acts.push(Act::Select(Picked::Tool(i)));
        }
        if cover_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    });
}

/// HumanityOS's own tools as a List: Open, its name, its line.
fn tools_list(ui: &mut Ui, theme: &Theme, page: &AppsState, order: &[usize], acts: &mut Vec<Act>) {
    let width = ui.available_width();
    let row_h = row_height(theme);
    let gap = ui.spacing().item_spacing.x;
    let name_w = 200.0;
    let line_w = (width - ACTION_W - name_w - gap * 3.0).max(120.0);
    let _ = shelf::list_headings(ui, theme, &[Heading { text: "", width: ACTION_W, sorts: false, on: false }, Heading { text: "Name", width: name_w, sorts: false, on: false }, Heading { text: "What it is", width: line_w, sorts: false, on: false }]);
    for (n, &i) in order.iter().enumerate() {
        let tool = &page.own_tools[i];
        shelf::list_row(ui, theme, n, width, row_h, page.selected == Some(Picked::Tool(i)), |ui| {
            cover_tile::fixed_cell(ui, ACTION_W, row_h, |ui| open_button(ui, theme, tool, widgets::ButtonSize::Medium, acts));
            cover_tile::fixed_cell(ui, name_w, row_h, |ui| {
                let name = ui.add(egui::Label::new(RichText::new(&tool.name).size(theme.font_size_body).strong().color(theme.text_primary())).truncate().sense(Sense::click()));
                if name.clicked() {
                    acts.push(Act::Select(Picked::Tool(i)));
                }
            });
            cover_tile::fixed_cell(ui, line_w, row_h, |ui| {
                ui.add(egui::Label::new(RichText::new(&tool.line).size(theme.font_size_small).color(theme.text_secondary())).truncate());
            });
        });
    }
}

// ---------------------------------------------------------------------------------------------
// Other programs (blue), Help (cyan) and Services (magenta): the external catalog
// ---------------------------------------------------------------------------------------------

/// A catalog part: its band, its categories (when it has more than one), its entries, and its
/// lines below.
fn catalog_part(ui: &mut Ui, theme: &Theme, state: &GuiState, section: Section, acts: &mut Vec<Act>) {
    let page = &state.apps;
    let view = &state.settings.apps_view;
    band(ui, theme, section, count(state, section), |_| {});
    let cats = categories(&state.external_catalog, section);
    if cats.len() > 1 {
        if let Some(c) = category_pills(ui, theme, &cats, view.category(section)) {
            acts.push(Act::View(view.with_category(section, c)));
        }
        ui.add_space(theme.spacing_xs);
    }
    if section == Section::Programs && page.programs.found.is_none() {
        ui.label(RichText::new(programs::READING).size(theme.font_size_small).color(theme.text_muted()));
    }
    entries(ui, theme, state, section, page.orders.of(section), acts);
    ui.add_space(theme.spacing_xs);
    match section {
        Section::Programs => {
            line_with_why(ui, theme, programs::WHERE_FROM, "What Microsoft, the makers and the law say about finding, installing and starting programs, their licences and their logos, as researched on 19 October 2026, in the Library.", (programs::FINDING_SLUG, programs::FINDING_PATH), acts);
            ui.label(RichText::new(programs::SAFETY_LINE).size(theme.font_size_small).color(theme.text_primary()));
            widgets::body_hint(ui, theme, if cfg!(windows) { programs::FOUND_LINE } else { programs::WINDOWS_ONLY });
            ui.label(RichText::new(programs::NOT_AFFILIATED).size(theme.font_size_small).color(theme.text_muted()));
        }
        Section::Help => widgets::body_hint(ui, theme, programs::HELP_LINE),
        _ => widgets::body_hint(ui, theme, programs::SERVICES_LINE),
    }
}

/// Catalog entries (other programs, or help services), in `order`, as tiles or rows; sorted by
/// category, the Grid shows each category under its name.
fn entries(ui: &mut Ui, theme: &Theme, state: &GuiState, section: Section, order: &[usize], acts: &mut Vec<Act>) {
    let page = &state.apps;
    if order.is_empty() {
        let words = if page.search.trim().is_empty() { "Nothing here matches the filters.".to_string() } else { format!("Nothing here matches \"{}\".", page.search.trim()) };
        ui.label(RichText::new(words).size(theme.font_size_small).color(theme.text_muted()));
        return;
    }
    let view = &state.settings.apps_view;
    match view.layout {
        Layout::Grid if view.sort == SortBy::Category => {
            let mut start = 0;
            while start < order.len() {
                let id = &state.external_catalog[order[start]].category_id;
                let end = order[start..].iter().position(|&i| &state.external_catalog[i].category_id != id).map_or(order.len(), |n| start + n);
                group_heading(ui, theme, &state.external_catalog[order[start]], end - start);
                shelf::grid(ui, theme, &order[start..end], TILE_W, |ui, i, w| entry_tile(ui, theme, state, i, w, acts));
                start = end;
            }
        }
        Layout::Grid => shelf::grid(ui, theme, order, TILE_W, |ui, i, w| entry_tile(ui, theme, state, i, w, acts)),
        Layout::List => entries_list(ui, theme, state, section, order, acts),
    }
}

/// A category's name over its tiles (the Grid sorted by category), with how many.
fn group_heading(ui: &mut Ui, theme: &Theme, first: &CatalogEntry, n: usize) {
    ui.horizontal(|ui| {
        let (dot, _) = ui.allocate_exact_size(Vec2::splat(theme.font_size_small), Sense::hover());
        ui.painter().circle_filled(dot.center(), dot.width() * 0.35, category_color(theme, first));
        ui.label(RichText::new(&first.category).size(theme.font_size_body).strong().color(theme.text_primary()));
        ui.label(RichText::new(n.to_string()).size(theme.font_size_small).color(theme.text_muted()));
    });
    ui.add_space(theme.spacing_xs);
}

/// An entry's category colour: its category's own hue, as the plain covers are tinted.
pub(super) fn category_color(theme: &Theme, e: &CatalogEntry) -> Color32 {
    theme.text_secondary().lerp_to_gamma(widgets::swatch_color(&e.category_id), 0.7)
}

/// The fact at the right of an entry's button: installed and its version (in green), else its
/// download size, or for a help service its website.
fn entry_fact(state: &GuiState, theme: &Theme, i: usize) -> Option<(String, Color32)> {
    let e = &state.external_catalog[i];
    if let Some(found) = state.apps.programs.installed(i) {
        let words = match (&found.version, found.via) {
            (Some(v), _) => v.clone(),
            (None, crate::software::Via::Steam) => "Through Steam".to_string(),
            (None, _) => programs::INSTALLED.to_string(),
        };
        return Some((words, theme.success()));
    }
    if !e.is_software() {
        return Some((e.host(), theme.text_secondary()));
    }
    if e.web_only() {
        return Some(("In your browser".to_string(), theme.text_secondary()));
    }
    (!e.size.is_empty()).then(|| (e.size.clone(), theme.text_secondary()))
}

/// The picture an entry's cover shows: none. No logo is shipped with HumanityOS or fetched from any
/// website: each maker sets its own rules for its logo (the 2026-10-19 finding, judgement call 6:
/// docs/reference/findings/2026-10-19-tools-page-install-launch-and-icons.md). An installed
/// program's own icon, read in place from its own file and shown to this person only, is drawn on
/// the plain cover as its mark. The one place an entry's picture comes from, so a change to that (a
/// maker who allows its logo, say) is made here.
pub(super) fn picture_of(_state: &GuiState, _i: usize) -> Option<(egui::TextureId, Vec2)> {
    None
}

/// One catalog entry's tile: its plain cover (its name, an installed program's icon, "Installed"),
/// its category (and licence), its button and one fact.
fn entry_tile(ui: &mut Ui, theme: &Theme, state: &GuiState, i: usize, width: f32, acts: &mut Vec<Act>) {
    let e = &state.external_catalog[i];
    let page = &state.apps;
    let installed = page.programs.installed(i);
    shelf::tile_frame(ui, theme, width, page.selected == Some(Picked::Entry(i)), |ui, w| {
        let picture = picture_of(state, i);
        let (rect, cover_resp) = cover(ui, theme, w, &e.name, &e.category_id, picture, picture.is_some(), page.programs.icon_of(i));
        if installed.is_some() {
            installed_mark(ui, theme, rect);
        }
        let meta = if e.license.is_empty() { e.category.clone() } else { format!("{} \u{b7} {}", e.category, e.license) };
        meta_line(ui, theme, &meta, theme.text_muted());
        button_row(ui, theme, w, |ui| main_button(ui, theme, e, i, installed, page.programs.installing(i), widgets::ButtonSize::Small, acts), entry_fact(state, theme, i));
        if cover_resp.clicked() {
            acts.push(Act::Select(Picked::Entry(i)));
        }
        if cover_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    });
}

/// "Installed", small, at a cover's top left.
fn installed_mark(ui: &Ui, theme: &Theme, cover: Rect) {
    let pad = theme.spacing_xs + 1.0;
    let galley = ui.painter().layout_no_wrap(programs::INSTALLED.to_string(), egui::FontId::proportional(theme.font_size_small), theme.text_on_accent());
    let size = galley.size() + Vec2::new(2.0 * theme.spacing_sm, theme.spacing_xs * 2.0);
    let rect = Rect::from_min_size(cover.min + Vec2::splat(pad), size);
    let corners = egui::CornerRadius::same(theme.pill_radius.round().clamp(0.0, 255.0) as u8);
    ui.painter().rect_filled(rect, corners, theme.success().gamma_multiply(0.9));
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, theme.text_on_accent());
    ui.painter().rect_stroke(rect, corners, Stroke::new(1.0, theme.success()), egui::StrokeKind::Inside);
}

/// The main button: Launch when installed (or, installed with nothing to start, the few words that
/// say so), otherwise Download or Open website; "Installing..." while winget installs it.
#[allow(clippy::too_many_arguments)]
fn main_button(ui: &mut Ui, theme: &Theme, e: &CatalogEntry, i: usize, installed: Option<&crate::software::Installed>, busy: bool, size: widgets::ButtonSize, acts: &mut Vec<Act>) {
    if busy {
        ui.add(egui::Spinner::new().size(theme.font_size_small));
        ui.label(RichText::new("Installing...").size(theme.font_size_small).color(theme.text_secondary()));
        return;
    }
    match installed {
        Some(found) if found.launch.is_some() => {
            if widgets::Button::primary(programs::LAUNCH).size(size).tooltip(programs::launch_tip(found)).show(ui, theme) {
                acts.push(Act::Launch(i));
            }
        }
        Some(_) => in_buttons_place(ui, theme, programs::NO_PROGRAM_SHORT, Some(programs::NO_PROGRAM.to_string()), theme.text_muted()),
        None => {
            let label = programs::web_button(e);
            let tip = if label == programs::DOWNLOAD { "Opens the project's own download page in your browser." } else { "Opens its website in your browser." };
            if widgets::Button::secondary(label).size(size).tooltip(tip).show(ui, theme) {
                let url = if label == programs::DOWNLOAD { e.download_page() } else { e.url.as_str() };
                acts.push(Act::Web(url.to_string()));
            }
        }
    }
}

/// The entries' List columns, each shown only when the room allows.
#[derive(Clone, Copy, PartialEq)]
enum EntryColumn {
    Category,
    Installed,
    License,
    Platforms,
    Size,
    Website,
}

impl EntryColumn {
    fn heading(self) -> &'static str {
        match self {
            EntryColumn::Category => "Category",
            EntryColumn::Installed => "Installed",
            EntryColumn::License => "Licence",
            EntryColumn::Platforms => "Runs on",
            EntryColumn::Size => "Size",
            EntryColumn::Website => "Website",
        }
    }

    fn width(self) -> f32 {
        match self {
            EntryColumn::Category => 200.0,
            EntryColumn::Installed => 120.0,
            EntryColumn::License => 110.0,
            EntryColumn::Platforms => 170.0,
            EntryColumn::Size => 80.0,
            EntryColumn::Website => 200.0,
        }
    }

    fn sort(self) -> Option<SortBy> {
        match self {
            EntryColumn::Category => Some(SortBy::Category),
            EntryColumn::Installed => Some(SortBy::InstalledFirst),
            EntryColumn::License | EntryColumn::Platforms | EntryColumn::Size | EntryColumn::Website => None,
        }
    }

    fn value(self, state: &GuiState, i: usize) -> String {
        let e = &state.external_catalog[i];
        match self {
            EntryColumn::Category => e.category.clone(),
            EntryColumn::Installed => state
                .apps
                .programs
                .installed(i)
                .map(|f| match (&f.version, f.via) {
                    (Some(v), _) => v.clone(),
                    (None, crate::software::Via::Steam) => "Through Steam".to_string(),
                    (None, _) => "Yes".to_string(),
                })
                .unwrap_or_default(),
            EntryColumn::License => e.license.clone(),
            EntryColumn::Platforms => programs::platforms_words(e),
            EntryColumn::Size => {
                if e.web_only() {
                    String::new()
                } else {
                    e.size.clone()
                }
            }
            EntryColumn::Website => e.host(),
        }
    }
}

/// The action cell's width.
const ACTION_W: f32 = 132.0;

/// The entries' columns that fit in `width`, most wanted first, in the order drawn: a program's or
/// (`services`) a help service's.
fn entry_columns_for(width: f32, services: bool) -> Vec<EntryColumn> {
    use EntryColumn as C;
    let (wanted, drawn): (Vec<C>, Vec<C>) = if services { (vec![C::Category, C::Website], vec![C::Category, C::Website]) } else { (vec![C::Category, C::Installed, C::License, C::Platforms, C::Size], vec![C::Category, C::Installed, C::License, C::Platforms, C::Size]) };
    let wanted: Vec<(C, f32)> = wanted.iter().map(|c| (*c, c.width())).collect();
    shelf::columns_that_fit(width - ACTION_W - NAME_MIN, &wanted, &drawn)
}

/// Catalog entries as a List: a heading row (a click on Name, Category or Installed sorts by it),
/// then a row each.
fn entries_list(ui: &mut Ui, theme: &Theme, state: &GuiState, section: Section, order: &[usize], acts: &mut Vec<Act>) {
    let page = &state.apps;
    let view = &state.settings.apps_view;
    let services = section != Section::Programs;
    let width = ui.available_width();
    let columns = entry_columns_for(width, services);
    let gap = ui.spacing().item_spacing.x;
    let fixed: f32 = ACTION_W + columns.iter().map(|c| c.width()).sum::<f32>() + gap * (columns.len() as f32 + 2.0);
    let name_w = (width - fixed).max(NAME_MIN * 0.6);
    let row_h = row_height(theme);
    let mut cells = vec![Heading { text: "", width: ACTION_W, sorts: false, on: false }, Heading { text: "Name", width: name_w, sorts: true, on: view.sort == SortBy::Name }];
    cells.extend(columns.iter().map(|c| Heading { text: c.heading(), width: c.width(), sorts: c.sort().is_some(), on: c.sort() == Some(view.sort) }));
    if let Some(clicked) = shelf::list_headings(ui, theme, &cells) {
        let sort = if clicked == 1 { Some(SortBy::Name) } else { clicked.checked_sub(2).and_then(|c| columns.get(c)).and_then(|c| c.sort()) };
        if let Some(sort) = sort {
            acts.push(sort_act(view, sort));
        }
    }
    for (n, &i) in order.iter().enumerate() {
        let e = &state.external_catalog[i];
        shelf::list_row(ui, theme, n, width, row_h, page.selected == Some(Picked::Entry(i)), |ui| {
            cover_tile::fixed_cell(ui, ACTION_W, row_h, |ui| main_button(ui, theme, e, i, page.programs.installed(i), page.programs.installing(i), widgets::ButtonSize::Medium, acts));
            cover_tile::fixed_cell(ui, name_w, row_h, |ui| {
                if let Some((tex, size)) = page.programs.icon_of(i) {
                    let side = theme.font_size_body + theme.spacing_xs;
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
                    let k = side / size.x.max(size.y).max(1.0);
                    ui.painter().image(tex, Rect::from_center_size(rect.center(), size * k), Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
                }
                let name = ui.add(egui::Label::new(RichText::new(&e.name).size(theme.font_size_body).strong().color(theme.text_primary())).truncate().sense(Sense::click()));
                if name.clicked() {
                    acts.push(Act::Select(Picked::Entry(i)));
                }
                if name.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            });
            for c in &columns {
                cover_tile::fixed_cell(ui, c.width(), row_h, |ui| {
                    let colour = match c {
                        EntryColumn::Installed => theme.success(),
                        EntryColumn::Category => theme.text_muted(),
                        _ => theme.text_secondary(),
                    };
                    ui.add(egui::Label::new(RichText::new(c.value(state, i)).size(theme.font_size_small).color(colour)).truncate());
                });
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE LISTS' COLUMNS GIVE WAY WHEN THE ROOM DOES: the games' five on a wide page, in their
    /// order, and on a narrow one the least wanted go first (Last played, then Achievements), Store
    /// last of all; the programs' five, on a narrow one Size then Runs on first; a help service's
    /// two.
    ///
    /// The games' narrow room (336) would hold Store, Played and Last played if they were taken in
    /// the order drawn, so it tells the two apart. Seen red 2026-10-19 (the Games page's test, kept
    /// here) with the columns taken in the order drawn: "the least wanted go first" failed, Last
    /// played kept and Size dropped. The programs' half seen red 2026-10-19 with the wanted order
    /// reversed (Size most wanted).
    #[test]
    fn the_lists_columns_give_way() {
        use EntryColumn as E;
        use GameColumn as G;
        assert!(game_columns_for(1400.0) == vec![G::Store, G::Played, G::LastPlayed, G::Size, G::Achievements], "all five, in their order");
        let narrow = game_columns_for(PLAY_W + NAME_MIN + STORE_BUTTON_W + 130.0 + 96.0 + 110.0);
        assert!(narrow == vec![G::Store, G::Played, G::Size], "the least wanted go first");
        assert!(game_columns_for(300.0).is_empty());
        assert!(entry_columns_for(1400.0, false) == vec![E::Category, E::Installed, E::License, E::Platforms, E::Size]);
        assert!(entry_columns_for(ACTION_W + NAME_MIN + 200.0 + 120.0 + 110.0 + 20.0, false) == vec![E::Category, E::Installed, E::License], "a program's least wanted go first");
        assert!(entry_columns_for(1400.0, true) == vec![E::Category, E::Website], "a help service's");
    }

    /// THE SORTS OFFERED ARE THE ONES THAT ORDER A PART SHOWN: all six with every part; the games'
    /// four with the games alone; the programs' three with the programs alone; Name alone for
    /// HumanityOS's own tools and for what was added; Name and Category for Help, and for Services.
    ///
    /// Seen red 2026-10-19 with `sorts_for` ignoring the part shown (`view.shows(..)` taken out):
    /// "the games' four" got all six.
    #[test]
    fn the_sorts_offered() {
        use SortBy as S;
        let only = |s: Section| sorts_for(&View { section: Some(s), ..View::default() });
        assert_eq!(sorts_for(&View::default()), S::ALL.to_vec());
        assert_eq!(only(Section::Games), vec![S::Name, S::RecentlyPlayed, S::MostPlayed, S::Size], "the games' four");
        assert_eq!(only(Section::Programs), vec![S::Name, S::InstalledFirst, S::Category]);
        assert_eq!((only(Section::Tools), only(Section::Added)), (vec![S::Name], vec![S::Name]));
        assert_eq!((only(Section::Help), only(Section::Services)), (vec![S::Name, S::Category], vec![S::Name, S::Category]));
    }

    /// EVERY PART CARRIES ITS NAME AND ITS OWN COLOUR: six parts, six names, six different band
    /// colours and six different edges, red, green, blue, cyan, magenta and yellow in that order,
    /// each from the theme; the colour is never the only way to tell them apart.
    ///
    /// Seen red 2026-10-19 with Help and Services both given cyan (`Section::Services => Tint::Cyan`):
    /// "six different band colours" found five.
    #[test]
    fn every_part_carries_its_name_and_its_own_colour() {
        let theme = crate::gui::theme::load_theme();
        let names: std::collections::HashSet<&str> = Section::ALL.iter().map(|s| s.words()).collect();
        assert_eq!(names.len(), 6, "six names");
        let fills: std::collections::HashSet<[u8; 4]> = Section::ALL.iter().map(|s| colours(&theme, *s).0.to_array()).collect();
        let edges: std::collections::HashSet<[u8; 4]> = Section::ALL.iter().map(|s| colours(&theme, *s).1.to_array()).collect();
        assert_eq!((fills.len(), edges.len()), (6, 6), "six different band colours and six different edges");
        assert_eq!(colours(&theme, Section::Games).1, theme.nav_legacy_red());
        assert_eq!(colours(&theme, Section::Added), (theme.section_yellow_bg(), theme.section_yellow()));
        assert_eq!(colours(&theme, Section::Help).1, theme.section_cyan());
        assert_eq!(colours(&theme, Section::Services).1, theme.section_magenta());
    }
}
