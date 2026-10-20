//! The Media page in three panes (operator, 2026-10-19: "what if for the media page we have a left
//! side card that is the list of all the media and right side is like viewing the in-depth details
//! of one media item. Maybe into thirds? Left side is the list of all of them, middle is the fit to
//! area picture, 3D model, sound/music visualizer, then right side is data/details/info. We could
//! do that top, middle bottom too. Like right now we have 3 large spots on my monitor."):
//!
//! - the LIST (list.rs): every item as a row, the one shown highlighted, with the filters, the
//!   featured shelf and the uploads above them;
//! - the VIEWER (viewer.rs): the item shown to fit;
//! - the DETAILS (item_view.rs `draw_details`): everything about it and its buttons, or, in edit
//!   mode, the edit form in its three coloured parts with the pane's border in the nav bar's moving
//!   colours (edit.rs `draw_in_place`).
//!
//! Side by side when the page is wider than tall and each fits at its narrowest; one above another
//! on a tall or narrow window (widgets/panes.rs `arrange_for`). Both are drawn by the same three
//! parts, so they cannot differ. The handles between the panes move the split, which is kept in
//! the app's settings with the view ([`Layout`]); a double-click on a handle puts its two panes
//! back. The Gallery (every item as a tile across the page, as the page was before) is the other
//! view: a press on a tile shows that item here.

use serde::{Deserialize, Serialize};

use super::item_view::{self, Pressed, DETAILS_LINE};
use super::{list, viewer};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::widgets::panes::Arrange;
use crate::gui::GuiState;

/// The narrowest a pane gets side by side, and the shortest stacked.
pub const MIN_PANE: f32 = 280.0;
pub const MIN_PANE_DOWN: f32 = 170.0;
/// The panes' first split side by side (list, viewer, details) and stacked.
pub const ACROSS: [f32; 3] = [0.28, 0.42, 0.30];
pub const DOWN: [f32; 3] = [0.30, 0.36, 0.34];

/// The view's labels, and what each is.
pub const PANES: &str = "Panes";
pub const PANES_TIP: &str = "The list, the item and its details side by side (one above another on a tall window).";
pub const GALLERY: &str = "Gallery";
pub const GALLERY_TIP: &str = "Every item as a tile across the page. Press one to see it in the panes.";

/// The Media page's view and its split, kept in the settings (config.json `media_layout`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    /// The Gallery instead of the panes.
    pub gallery: bool,
    /// Each pane's share of the page side by side (list, viewer, details), and stacked.
    pub across: Vec<f32>,
    pub down: Vec<f32>,
}

impl Default for Layout {
    fn default() -> Self {
        Layout { gallery: false, across: ACROSS.to_vec(), down: DOWN.to_vec() }
    }
}

impl Layout {
    /// This layout with any split that is not usable (hand-written, or from another number of
    /// panes) put back to the default.
    pub fn checked(&self) -> Layout {
        Layout { gallery: self.gallery, across: widgets::panes::checked(&self.across, &ACROSS), down: widgets::panes::checked(&self.down, &DOWN) }
    }

    /// The shares for panes going `arrange`.
    pub fn shares(&self, arrange: Arrange) -> Vec<f32> {
        match arrange {
            Arrange::Across => widgets::panes::checked(&self.across, &ACROSS),
            Arrange::Down => widgets::panes::checked(&self.down, &DOWN),
        }
    }

    fn set(&mut self, arrange: Arrange, shares: &[f32]) {
        match arrange {
            Arrange::Across => self.across = shares.to_vec(),
            Arrange::Down => self.down = shares.to_vec(),
        }
    }
}

/// The default split for panes going `arrange`.
pub fn defaults(arrange: Arrange) -> [f32; 3] {
    match arrange {
        Arrange::Across => ACROSS,
        Arrange::Down => DOWN,
    }
}

/// The way the panes go in a page area of `area`.
pub fn arrange_of(area: egui::Vec2, theme: &Theme) -> Arrange {
    widgets::panes::arrange_for(area, 3, MIN_PANE, theme.spacing_md)
}

/// The view switch, Panes or Gallery, in the page's heading row.
pub(crate) fn draw_switch(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let gallery = state.settings.media_layout.gallery;
    let small = widgets::ButtonSize::Small;
    let pick = |on: bool, label: &'static str| if on { widgets::Button::primary(label) } else { widgets::Button::secondary(label) };
    if pick(!gallery, PANES).size(small).tooltip(PANES_TIP).show(ui, theme) && gallery {
        set_gallery(state, false);
    }
    if pick(gallery, GALLERY).size(small).tooltip(GALLERY_TIP).show(ui, theme) && !gallery {
        set_gallery(state, true);
    }
}

/// Show the panes or the Gallery, and keep the choice.
pub(crate) fn set_gallery(state: &mut GuiState, on: bool) {
    if state.settings.media_layout.gallery != on {
        let mut layout = state.settings.media_layout.clone();
        layout.gallery = on;
        state.settings.media_layout = layout;
        state.settings_dirty = true;
    }
}

/// A place's panes in the space left on the page, `n` of them (two or three), the places at the
/// top of the first (places.rs); `add` draws each pane's own part (the first's under the places).
/// The split is the three panes' own, kept in the settings: two panes take the first pane's share
/// and the other two's together, and moving their handle moves the first share, the other two
/// keeping their proportion.
pub(crate) fn show_place(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, n: usize, mut add: impl FnMut(&mut egui::Ui, &mut GuiState, usize, egui::Rect)) {
    let arrange = widgets::panes::arrange_for(ui.available_size(), n, MIN_PANE, theme.spacing_md);
    let min = if arrange == Arrange::Across { MIN_PANE } else { MIN_PANE_DOWN };
    let three = state.settings.media_layout.shares(arrange);
    let d3 = defaults(arrange);
    let (mut shares, defs) = if n == 3 { (three.clone(), d3.to_vec()) } else { (vec![three[0], three[1] + three[2]], vec![d3[0], d3[1] + d3[2]]) };
    let moved = widgets::panes::show(ui, theme, egui::Id::new("media_panes"), arrange, &mut shares, &defs, min, |ui, i, card| {
        if i == 0 {
            super::places::draw(ui, theme, state);
        }
        add(ui, state, i, card);
    });
    if moved.changed {
        let new3 = if n == 3 {
            shares
        } else {
            let rest = three[1] + three[2];
            let k = if rest > 0.0 { shares[1] / rest } else { 1.0 };
            vec![shares[0], three[1] * k, three[2] * k]
        };
        let mut layout = state.settings.media_layout.clone();
        layout.set(arrange, &new3);
        state.settings.media_layout = layout;
    }
    if moved.settled {
        state.settings_dirty = true;
    }
}

/// A place in two panes: the places and `left` under them, and `right` beside (the Gallery, the
/// admins' section, Your folders' open folder).
pub(crate) fn draw_two(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &mut GuiState,
    left: impl FnOnce(&mut egui::Ui, &Theme, &mut GuiState),
    right: impl FnOnce(&mut egui::Ui, &Theme, &mut GuiState),
) {
    let (mut left, mut right) = (Some(left), Some(right));
    show_place(ui, theme, state, 2, |ui, state, i, _card| {
        let f = if i == 0 { left.take().map(|f| Box::new(f) as Box<dyn FnOnce(&mut egui::Ui, &Theme, &mut GuiState)>) } else { right.take().map(|f| Box::new(f) as Box<dyn FnOnce(&mut egui::Ui, &Theme, &mut GuiState)>) };
        if let Some(f) = f {
            f(ui, theme, state);
        }
    });
}

/// A place in three panes, as Shared on <server> is: the places and `left` under them, `middle`,
/// and `right` (Shared files, the HumanityOS folder).
pub(crate) fn draw_three(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &mut GuiState,
    left: fn(&mut egui::Ui, &Theme, &mut GuiState),
    middle: fn(&mut egui::Ui, &Theme, &mut GuiState),
    right: fn(&mut egui::Ui, &Theme, &mut GuiState),
) {
    show_place(ui, theme, state, 3, |ui, state, i, _card| match i {
        0 => left(ui, theme, state),
        1 => middle(ui, theme, state),
        _ => right(ui, theme, state),
    });
}

/// What is shared on the server, in three panes: the places and the list, the item to fit, its
/// details.
pub(crate) fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let selected = state.media.item.as_ref().map(|p| p.id);
    // Shown on the page now, whatever opened it.
    if let Some(page) = state.media.item.as_mut() {
        page.window = false;
    }
    let mut picked = None;
    // The item shown, taken out once the list is drawn (the list's rows read the state as it is).
    let mut taken: Option<Option<(item_view::ItemPage, Option<super::open::Opened>)>> = None;
    let mut pressed = Pressed::default();
    show_place(ui, theme, state, 3, |ui, state, i, card| {
        if i == 0 {
            picked = list::draw(ui, theme, state, selected);
            return;
        }
        let shown = taken.get_or_insert_with(|| item_view::take(state));
        match (i, shown.as_mut()) {
            (1, Some((page, opened))) => viewer::draw(ui, theme, state, page, opened.as_mut(), &mut pressed),
            (1, None) => viewer::nothing_chosen(ui, theme, item_view::CHOOSE_LINE),
            (_, Some((page, opened))) => {
                if page.edit.is_some() {
                    edit_border(ui, theme, card);
                }
                item_view::draw_details(ui, theme, state, page, opened.as_ref(), &mut pressed);
            }
            (_, None) => viewer::nothing_chosen(ui, theme, DETAILS_LINE),
        }
    });
    if let Some(Some((page, opened))) = taken {
        item_view::finish(state, page, opened, pressed);
    }
    if let Some(item) = picked {
        item_view::open(state, &item);
    }
}

/// A row of buttons at the foot of a bottom-up layout (Save and Cancel under the edit form, the
/// middle pane's buttons under a file's card), centred across it when `centre`. egui's own
/// `horizontal` row starts one line tall and grows downwards, so in a bottom-up layout a row of
/// taller buttons runs off the pane's foot (it did: Save and Cancel were cut in half on the first
/// snapshot, 2026-10-19). So the row is given the height and width it took last frame, measured
/// again each frame; the first frame guesses, and asks for another.
pub(crate) fn foot_row<R>(ui: &mut egui::Ui, id: egui::Id, centre: bool, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let (width, height): (f32, f32) = ui.ctx().data(|d| d.get_temp(id)).unwrap_or((0.0, ui.spacing().interact_size.y * 1.6));
    let r = ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
        if centre {
            ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
        }
        let start = ui.cursor().left();
        let inner = add(ui);
        (inner, (ui.cursor().left() - start - ui.spacing().item_spacing.x).max(0.0))
    });
    let (inner, took_w) = r.inner;
    let took_h = r.response.rect.height();
    if (took_w - width).abs() > 0.5 || (took_h - height).abs() > 0.5 {
        ui.ctx().data_mut(|d| d.insert_temp(id, (took_w, took_h)));
        ui.ctx().request_repaint();
    }
    inner
}

/// Edit mode's border around the details pane's card (`card`): the nav bar's active border, in its
/// moving colours (widgets/tint_section.rs `rgb_border`), so edit mode stands out at a glance.
pub(crate) fn edit_border(ui: &mut egui::Ui, theme: &Theme, card: egui::Rect) {
    let width = theme.nav_active_border_width.max(1.5);
    let clip = ui.clip_rect();
    ui.set_clip_rect(card.expand(width));
    widgets::tint_section::rgb_border(ui, theme, card.shrink(width + 1.0));
    ui.set_clip_rect(clip);
}
