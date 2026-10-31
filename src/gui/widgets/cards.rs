//! Cards that use the width (operator, 2026-10-30, of the Home page: "everything is left
//! aligned. There's a ton of unused space ... We should join the two areas and make everything
//! more visually friendly"), and the parts a card is made of.
//!
//! - [`Grid`]: cards in rows, as many to a row as fit at a minimum width, stretched so each row
//!   fills the width; one column on a narrow window, five on a wide one. The cards of a row share
//!   the row's height, so their edges line up. A plain grid places things that draw their own
//!   frame (the Home page's Household card beside "Where you live"). [`Grid::show_with_details`]
//!   draws a card's opened long words in a panel the grid's width under its row, so they read at a
//!   comfortable width and the cards beside it keep their size (an explanation of 3,000
//!   characters opened inside its card stretched every card of its row into a tall empty one).
//! - [`detail_heading`]: such a panel's heading.
//! - [`status_badge`]: a short word in a filled pill ("closed", "short", "traded").
//! - [`gauge`]: what there is against what is needed, as a bar in the state's colour.
//! - [`fold`]: a small "How this is worked out" line that opens the long words under it, so a
//!   card shows its facts first and its explanation a click away.
//! - [`clamped`]: words cut to a few lines with "..." and the whole of them on hover.
//!
//! Each cell's place is fixed by its index (row = index / columns), never by the heights drawn,
//! so opening a fold in one card never moves another card to a different column.
//!
//! Every colour and size comes from the theme. The website's twin is `web/shared/list-views.css`
//! (`.card-grid`, `.status-badge`, `.gauge`, `.fold`).

use egui::{Align, Color32, Id, Layout, Pos2, Rect, RichText, Sense, Stroke, Ui, UiBuilder, Vec2};

use super::hint::Hint;
use crate::gui::theme::Theme;

/// How many columns at least `min_w` wide fit in `width` with `gap` between them: at least one,
/// at most `max`.
pub fn columns_for(width: f32, min_w: f32, gap: f32, max: usize) -> usize {
    if !width.is_finite() || width <= 0.0 || min_w <= 0.0 {
        return 1;
    }
    let n = ((width + gap) / (min_w + gap)).floor().max(1.0) as usize;
    n.clamp(1, max.max(1))
}

/// Cards in rows that fill the width. Build it, then [`Grid::show`] it with a count and a closure
/// that draws cell `i` into the Ui it is given (already the cell's width, top to bottom).
#[derive(Clone, Copy)]
pub struct Grid {
    id: Id,
    min_w: f32,
    max_cols: usize,
    cards: bool,
    stretch: bool,
    edge: Option<Color32>,
    gap: Option<f32>,
}

impl Grid {
    /// A grid of cards, each at least 320 points wide, as many to a row as fit.
    pub fn new(id_salt: impl std::hash::Hash) -> Self {
        Self { id: Id::new(id_salt), min_w: 320.0, max_cols: 8, cards: true, stretch: true, edge: None, gap: None }
    }

    /// Each card only as tall as its own contents, tops in line (for cards that open to very
    /// different heights, the Home page's towers).
    pub fn top_aligned(mut self) -> Self {
        self.stretch = false;
        self
    }

    /// The room between cells, across and down (the theme's spacing_md unless set).
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = Some(gap);
        self
    }

    /// The narrowest a cell may be before the row loses a column.
    pub fn min_width(mut self, w: f32) -> Self {
        self.min_w = w;
        self
    }

    /// The most cells to a row, however wide the window.
    pub fn max_columns(mut self, n: usize) -> Self {
        self.max_cols = n.max(1);
        self
    }

    /// The cells draw their own frames: the grid only places them, each at its own height.
    pub fn plain(mut self) -> Self {
        self.cards = false;
        self.stretch = false;
        self
    }

    /// The cards' edge in this colour instead of the border's (the Home page's traded loops).
    pub fn edge(mut self, color: Color32) -> Self {
        self.edge = Some(color);
        self
    }

    /// Draw `count` cells. Returns how many columns the rows had.
    pub fn show(self, ui: &mut Ui, theme: &Theme, count: usize, mut cell: impl FnMut(&mut Ui, usize)) -> usize {
        self.draw(
            ui,
            theme,
            count,
            &mut |ui, i| {
                cell(ui, i);
                false
            },
            None,
        )
    }

    /// Draw `count` cells, each of which may have its long words open: `cell` draws cell `i` and
    /// says whether they are (its [`fold`] is open), and `detail` draws them, in a panel the full
    /// width of the grid right under that cell's row, edged in the accent and headed `title(i)`.
    /// So an open explanation reads at a comfortable width and never stretches the cards beside
    /// its own into tall empty ones (the accordion a picture search uses). Returns the columns.
    pub fn show_with_details(
        self,
        ui: &mut Ui,
        theme: &Theme,
        count: usize,
        mut cell: impl FnMut(&mut Ui, usize) -> bool,
        mut detail: impl FnMut(&mut Ui, usize),
    ) -> usize {
        self.draw(ui, theme, count, &mut cell, Some(&mut detail as &mut dyn FnMut(&mut Ui, usize)))
    }

    fn draw(self, ui: &mut Ui, theme: &Theme, count: usize, cell: &mut dyn FnMut(&mut Ui, usize) -> bool, mut detail: Option<&mut dyn FnMut(&mut Ui, usize)>) -> usize {
        if count == 0 {
            return 0;
        }
        let gap = self.gap.unwrap_or(theme.spacing_md);
        let width = ui.available_width().max(1.0);
        let cols = columns_for(width, self.min_w, gap, self.max_cols).min(count);
        let cell_w = ((width - gap * (cols as f32 - 1.0)) / cols as f32).floor().max(1.0);
        let pad = if self.cards { theme.card_padding } else { 0.0 };
        let corners = egui::CornerRadius::same(theme.border_radius.round().clamp(0.0, 255.0) as u8);
        let edge = self.edge.unwrap_or_else(|| theme.border());
        let mut start = 0;
        while start < count {
            let top_left = ui.cursor().min;
            let mut drawn: Vec<(egui::layers::ShapeIdx, f32, f32)> = Vec::new();
            let mut opened: Vec<usize> = Vec::new();
            for j in 0..cols {
                let i = start + j;
                if i >= count {
                    break;
                }
                let x = top_left.x + j as f32 * (cell_w + gap);
                // The card's background goes in first, so it sits under the contents; its size is
                // set once the row's tallest card is known.
                let bg = ui.painter().add(egui::Shape::Noop);
                let inner = Rect::from_min_max(Pos2::new(x + pad, top_left.y + pad), Pos2::new(x + cell_w - pad, f32::INFINITY));
                let mut child = ui.new_child(UiBuilder::new().id_salt((self.id, i)).max_rect(inner).layout(Layout::top_down(Align::Min)));
                child.set_min_width(inner.width());
                if cell(&mut child, i) {
                    opened.push(i);
                }
                drawn.push((bg, x, child.min_rect().height() + 2.0 * pad));
            }
            let row_h = drawn.iter().map(|d| d.2).fold(0.0_f32, f32::max);
            if self.cards {
                for (bg, x, h) in &drawn {
                    let rect = Rect::from_min_size(Pos2::new(*x, top_left.y), Vec2::new(cell_w, if self.stretch { row_h } else { *h }));
                    ui.painter().set(*bg, egui::epaint::RectShape::new(rect, corners, theme.bg_card(), Stroke::new(1.0, edge), egui::StrokeKind::Inside));
                }
            }
            ui.advance_cursor_after_rect(Rect::from_min_size(top_left, Vec2::new(width, row_h)));
            ui.add_space((gap - ui.spacing().item_spacing.y).max(0.0));
            // The open cells' long words, each in a panel the grid's width, under their row.
            if let Some(detail) = detail.as_mut() {
                for i in opened {
                    egui::Frame::none()
                        .fill(theme.bg_card())
                        .stroke(Stroke::new(1.0, theme.accent()))
                        .corner_radius(corners)
                        .inner_margin(theme.card_padding)
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            detail(ui, i);
                        });
                    ui.add_space((gap - ui.spacing().item_spacing.y).max(0.0));
                }
            }
            start += cols;
        }
        cols
    }
}

/// A detail panel's heading: the card's name in the accent, and what the panel holds beside it.
pub fn detail_heading(ui: &mut Ui, theme: &Theme, name: &str, what: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(name).size(theme.font_size_body).strong().color(theme.accent()));
        ui.label(RichText::new(what).size(theme.font_size_small).color(theme.text_muted()));
    });
    ui.add_space(theme.spacing_xs);
}

/// A short word in a pill tinted with `color`: the words in the colour, on a faint wash of it, in
/// a thin edge of it ("closed" in green, "short" in red). Reads on the dark and the light theme.
pub fn status_badge(ui: &mut Ui, theme: &Theme, text: &str, color: Color32) {
    pill(ui, theme, text, color.gamma_multiply(0.18), color.gamma_multiply(0.55), color);
}

/// A neutral pill: a count or a label that is no state ("6 animals", "F3").
pub fn plain_badge(ui: &mut Ui, theme: &Theme, text: &str) {
    pill(ui, theme, text, theme.bg_tertiary(), theme.border(), theme.text_secondary());
}

fn pill(ui: &mut Ui, theme: &Theme, text: &str, fill: Color32, edge: Color32, ink: Color32) {
    egui::Frame::none()
        .fill(fill)
        .stroke(Stroke::new(1.0, edge))
        .corner_radius(egui::CornerRadius::same(theme.pill_radius.round().clamp(0.0, 255.0) as u8))
        .inner_margin(egui::Margin::symmetric(theme.badge_padding_h.round() as i8, theme.badge_padding_v.round() as i8))
        .show(ui, |ui| {
            // A pill is a word or two and never breaks (badge_sm's lesson, 2026-09-27).
            ui.add(egui::Label::new(RichText::new(text).size(theme.font_size_small).strong().color(ink)).extend());
        });
}

/// What there is against what is needed: a bar the width there is, filled to `have / need` (full
/// at or past it) in `color`, on the slider track. Nothing is drawn for a need of zero or less.
pub fn gauge(ui: &mut Ui, theme: &Theme, have: f32, need: f32, color: Color32) {
    if need <= 0.0 || !need.is_finite() || !have.is_finite() {
        return;
    }
    let height = (theme.spacing_sm).max(6.0);
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width.max(height), height), Sense::hover());
    let corners = egui::CornerRadius::same((height / 2.0).round().clamp(0.0, 255.0) as u8);
    ui.painter().rect_filled(rect, corners, theme.slider_track());
    let f = (have / need).clamp(0.0, 1.0);
    if f > 0.0 {
        let filled = Rect::from_min_size(rect.min, Vec2::new((rect.width() * f).max(height), height));
        ui.painter().rect_filled(filled, corners, color);
    }
}

/// A small line that opens the words under it: a painted triangle and `title` in the accent,
/// closed until clicked, its state kept by `id_salt` (unique on the page). Returns whether it is
/// open; the caller draws the words when it is. `hint_key` is its hover hint.
pub fn fold(ui: &mut Ui, theme: &Theme, id_salt: impl std::hash::Hash, title: &str, hint_key: &str) -> bool {
    let id = Id::new(("hos_fold", id_salt));
    let mut open = ui.data_mut(|d| *d.get_temp_mut_or(id, false));
    let size = theme.font_size_small;
    let row = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing_xs;
        let (tri, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
        let c = tri.center();
        let r = size * 0.3;
        let pts = if open {
            vec![c + Vec2::new(-r, -r * 0.6), c + Vec2::new(r, -r * 0.6), c + Vec2::new(0.0, r * 0.8)]
        } else {
            vec![c + Vec2::new(-r * 0.6, -r), c + Vec2::new(-r * 0.6, r), c + Vec2::new(r * 0.8, 0.0)]
        };
        ui.painter().add(egui::Shape::convex_polygon(pts, theme.accent(), Stroke::NONE));
        ui.add(egui::Label::new(RichText::new(title).size(size).color(theme.accent())).selectable(false));
    });
    let resp = ui.interact(row.response.rect, id.with("row"), Sense::click());
    let resp = resp.hint_dyn(hint_key);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if resp.clicked() {
        open = !open;
        ui.data_mut(|d| d.insert_temp(id, open));
    }
    open
}

/// Whether the fold `id_salt` is open, and setting it (a snapshot opens one to show its words).
pub fn fold_is_open(ctx: &egui::Context, id_salt: impl std::hash::Hash) -> bool {
    ctx.data(|d| d.get_temp::<bool>(Id::new(("hos_fold", id_salt)))).unwrap_or(false)
}

/// See [`fold_is_open`].
pub fn set_fold_open(ctx: &egui::Context, id_salt: impl std::hash::Hash, open: bool) {
    ctx.data_mut(|d| d.insert_temp(Id::new(("hos_fold", id_salt)), open));
}

/// `text` cut to `rows` lines at the width there is, ending "..." when cut, with the whole of it
/// shown when the pointer rests on it.
pub fn clamped(ui: &mut Ui, theme: &Theme, text: &str, size: f32, color: Color32, rows: usize) {
    let mut job = egui::text::LayoutJob::single_section(
        text.to_owned(),
        egui::TextFormat { font_id: egui::FontId::proportional(size), color, ..Default::default() },
    );
    job.wrap = egui::text::TextWrapping { max_width: ui.available_width(), max_rows: rows.max(1), break_anywhere: false, overflow_character: Some('\u{2026}') };
    let galley = ui.fonts(|f| f.layout_job(job));
    let cut = galley.elided;
    let resp = ui.add(egui::Label::new(galley));
    if cut {
        // The whole of the words: what was cut is still a fact, only folded away.
        resp.on_hover_ui(|ui| {
            ui.set_max_width(theme.modal_width.max(320.0));
            ui.label(RichText::new(text).size(size).color(theme.text_primary()));
        });
    }
}

/// A heading for a group of cards: the words at heading size, an optional note beside them, and
/// room under it.
pub fn group_heading(ui: &mut Ui, theme: &Theme, title: &str, note: Option<&str>) {
    ui.add_space(theme.spacing_sm);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(title).size(theme.font_size_heading).color(theme.text_primary()));
        if let Some(note) = note.filter(|n| !n.is_empty()) {
            ui.label(RichText::new(note).size(theme.font_size_small).color(theme.text_muted()));
        }
    });
    ui.add_space(theme.spacing_xs);
}

/// The widest a paragraph should run before it is hard to read (about 110 characters), or the
/// room there is when that is less.
pub fn prose_width(ui: &Ui, theme: &Theme) -> f32 {
    (theme.font_size_small * 60.0).min(ui.available_width())
}

/// A paragraph at that width, small and in `colour`. (In a scope of its own: a Ui already as wide
/// as its cell cannot be narrowed, egui keeps its minimum.)
pub fn prose(ui: &mut Ui, theme: &Theme, text: &str, colour: Color32) {
    ui.scope(|ui| {
        ui.set_max_width(prose_width(ui, theme));
        ui.label(RichText::new(text).size(theme.font_size_small).color(colour));
    });
}

/// `add` at the readable width (see [`prose`]).
pub fn readable(ui: &mut Ui, theme: &Theme, add: impl FnOnce(&mut Ui)) {
    ui.scope(|ui| {
        ui.set_max_width(prose_width(ui, theme));
        add(ui);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// HOW MANY COLUMNS FIT: one on a narrow window, as many as fit on a wide one, never more than
    /// asked, and one for nothing sensible. Three 320-point columns need 992 points with two
    /// 16-point gaps between them, so 976 is two.
    #[test]
    fn how_many_columns_fit() {
        assert_eq!(columns_for(300.0, 320.0, 16.0, 8), 1, "narrower than one column is still one");
        assert_eq!(columns_for(992.0, 320.0, 16.0, 8), 3);
        assert_eq!(columns_for(976.0, 320.0, 16.0, 8), 2, "three need 992");
        assert_eq!(columns_for(2560.0, 320.0, 16.0, 8), 7);
        assert_eq!(columns_for(2560.0, 320.0, 16.0, 4), 4, "never more than asked");
        assert_eq!(columns_for(f32::INFINITY, 320.0, 16.0, 8), 1);
        assert_eq!(columns_for(800.0, 0.0, 16.0, 8), 1);
    }

    /// THE CARDS OF A ROW SHARE ITS HEIGHT, and the rows fill the width: three cards in 1000
    /// points at a 300 minimum make one row of three, each a third of the width less the gaps,
    /// and the short card's background is as tall as the tall one's.
    #[test]
    fn a_rows_cards_line_up() {
        let ctx = egui::Context::default();
        let theme = crate::gui::theme::load_theme();
        let mut rects: Vec<Rect> = Vec::new();
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 800.0))), ..Default::default() };
        let out = ctx.run(input, |ctx| {
            egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| {
                Grid::new("t").min_width(300.0).show(ui, &theme, 3, |ui, i| {
                    for _ in 0..(1 + i * 3) {
                        ui.label("x");
                    }
                    rects.push(ui.min_rect());
                });
            });
        });
        assert_eq!(rects.len(), 3);
        let backgrounds: Vec<Rect> = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Rect(r) if r.fill == theme.bg_card() => Some(r.rect),
                _ => None,
            })
            .collect();
        assert_eq!(backgrounds.len(), 3, "a card behind each cell: {backgrounds:?}");
        let h = backgrounds[0].height();
        assert!(backgrounds.iter().all(|b| (b.height() - h).abs() < 0.5), "one height for the row: {backgrounds:?}");
        assert!(backgrounds[2].right() > 990.0, "the row fills the width: {backgrounds:?}");
        assert!(rects[0].top() == rects[2].top(), "one row: {rects:?}");
    }

    /// AN OPENED CARD'S WORDS GO UNDER ITS ROW, the grid's width, and the cards beside it keep their
    /// size: four cards in two rows of two, the first one open, its 40 lines drawn between the
    /// rows, the second card no taller than its own one line, and the second row below the words.
    ///
    /// Seen red 2026-10-30 with the words drawn inside the open card (the first version): "four
    /// cards and one panel", no panel, and the second card's background 706 points tall beside the
    /// open one's 40 lines.
    #[test]
    fn opened_words_go_under_the_row() {
        let ctx = egui::Context::default();
        let theme = crate::gui::theme::load_theme();
        let mut cells: Vec<Rect> = vec![Rect::NOTHING; 4];
        let mut words = Rect::NOTHING;
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(700.0, 3000.0))), ..Default::default() };
        let out = ctx.run(input, |ctx| {
            egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| {
                Grid::new("d").min_width(300.0).show_with_details(
                    ui,
                    &theme,
                    4,
                    |ui, i| {
                        ui.label("card");
                        cells[i] = ui.min_rect();
                        i == 0
                    },
                    |ui, _| {
                        for _ in 0..40 {
                            ui.label("words");
                        }
                        words = ui.min_rect();
                    },
                );
            });
        });
        let backgrounds: Vec<Rect> = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Rect(r) if r.fill == theme.bg_card() => Some(r.rect),
                _ => None,
            })
            .collect();
        assert_eq!(backgrounds.len(), 5, "four cards and one panel: {backgrounds:?}");
        assert!(cells[0].top() == cells[1].top() && cells[2].top() == cells[3].top() && cells[2].top() > cells[0].top(), "two rows of two: {cells:?}");
        assert!(words.top() > cells[1].bottom() && words.bottom() < cells[2].top(), "the words between the rows: {words:?} {cells:?}");
        assert!(words.width() > 600.0, "the grid's width: {words:?}");
        let second = backgrounds.iter().find(|b| (b.left() - (cells[1].left() - theme.card_padding)).abs() < 1.0 && b.top() < words.top()).expect("the second card's background");
        assert!(second.height() < 100.0, "the card beside the open one keeps its size: {second:?}");
    }
}
