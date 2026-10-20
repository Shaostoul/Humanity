//! The Files page's left pane (2026-10-19, the three panes: panes.rs): every item as a compact row,
//! the one shown in the middle and on the right highlighted. Above the rows, as the page had them:
//! what was last done, the uploads in progress and waiting, the featured shelf (as rows), and the
//! filters; under them, More and the page's foot (Report a copyright problem, the takedown log).
//! It scrolls on its own.
//!
//! A ROW is the item's small picture, its title, "by Ann in Fire flow arts", and a line with its
//! kind, picks and downloads, size and badges (Featured, Kept by the server, Members only, its
//! content labels). The small picture keeps the tiles' rules (tiles.rs `show_of`): only an item
//! whose picture may show (not hidden by the protected setup, not labelled, not a stranger's
//! waiting for a press, not too large) shows it; any other shows its kind's mark, and the middle
//! pane asks for the press. A press on a row shows that item; the row has no buttons of its own
//! (they are on the right). The tiles, with their buttons, are the Gallery view (media.rs).

use egui::{Color32, Rect, Sense, Vec2};

use super::api::{Filters, Item};
use super::{filters, kinds, tiles};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

/// A row's small picture.
const THUMB: Vec2 = Vec2::new(88.0, 62.0);

/// The pane: everything above the rows, the rows, and what is under them. Returns the item whose
/// row was pressed, if one was.
pub(crate) fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, selected: Option<i64>) -> Option<Item> {
    let mut picked = None;
    egui::ScrollArea::vertical().id_salt("media_list").auto_shrink([false, false]).show(ui, |ui| {
        if !state.files.status.is_empty() {
            ui.label(egui::RichText::new(&state.files.status).size(theme.font_size_small).color(theme.text_secondary()));
        }
        if state.files.answered && !state.files.list.members_view {
            ui.label(egui::RichText::new(super::SIGN_IN_LINE).size(theme.font_size_small).color(theme.warning()));
        }
        super::uploading::draw_list(ui, theme, state);
        super::waiting::draw(ui, theme, state);
        let featured = state.files.featured.clone();
        if !featured.is_empty() {
            widgets::subsection_label(ui, theme, &filters::shelf_heading(state));
            for item in &featured {
                if row(ui, theme, state, item, selected, "featured") {
                    picked = Some(item.clone());
                }
            }
            ui.add_space(theme.spacing_md);
        }
        filters::draw_filters(ui, theme, state);
        ui.add_space(theme.spacing_sm);
        let items = state.files.list.items.clone();
        if items.is_empty() {
            let line = if !state.files.answered {
                "Loading..."
            } else if state.files.filters != Filters::default() {
                "Nothing matches these filters."
            } else {
                super::EMPTY_LINE
            };
            ui.label(egui::RichText::new(line).size(theme.font_size_body).color(theme.text_muted()));
        } else {
            for item in &items {
                if row(ui, theme, state, item, selected, "list") {
                    picked = Some(item.clone());
                }
            }
        }
        super::draw_more(ui, theme, state);
        super::takedown::draw_foot(ui, theme, state);
    });
    picked
}

/// One row. True when it was pressed. `salt` keeps the shelf's row and the list's row of the same
/// item apart (each its own steady id, ui-system.md's rule for a clickable region).
pub(crate) fn row(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, item: &Item, selected: Option<i64>, salt: &str) -> bool {
    let pad = theme.spacing_xs;
    let height = THUMB.y + 2.0 * pad;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width().max(THUMB.x * 2.0), height), Sense::hover());
    let resp = ui.interact(rect, egui::Id::new(("media_row", salt, item.id)), Sense::click());
    ui.add_space(theme.spacing_xs);
    // Nothing is asked for or painted for a row out of sight (a long list stays cheap).
    if !ui.is_rect_visible(rect) {
        return resp.clicked();
    }
    let show = tiles::show_of(state, item);
    if show == tiles::Show::Load {
        if item.cover.is_some() {
            tiles::request_cover(state, item);
        } else if item.kind == "picture" {
            tiles::request_picture(state, item);
        }
    }
    let chosen = selected == Some(item.id);
    let radius = egui::CornerRadius::same(theme.border_radius as u8);
    let painter = ui.painter_at(rect);
    if chosen {
        painter.rect(rect, radius, theme.accent().gamma_multiply(0.18), egui::Stroke::new(1.5, theme.accent()), egui::StrokeKind::Inside);
    } else if resp.hovered() {
        painter.rect_filled(rect, radius, theme.bg_tertiary());
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    // The small picture, or the kind's mark.
    let thumb = Rect::from_min_size(rect.min + egui::vec2(pad, pad), THUMB);
    painter.rect_filled(thumb, radius, theme.bg_tertiary());
    let picture = (show == tiles::Show::Load).then(|| if item.cover.is_some() { Some(tiles::cover_key(state, item)) } else { (item.kind == "picture").then(|| tiles::cache_key(state, item)) }).flatten();
    let mut drawn = false;
    if let Some(tex) = picture.as_deref().and_then(|key| state.image_cache.get_texture(key)) {
        let [w, h] = tex.size();
        let fitted = fit(thumb, w as f32 / h.max(1) as f32);
        let mut mesh = egui::Mesh::with_texture(tex.id());
        mesh.add_rect_with_uv(fitted, Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
        painter.add(egui::Shape::mesh(mesh));
        drawn = true;
    }
    if !drawn {
        let mark = Rect::from_center_size(thumb.center(), Vec2::splat(THUMB.y * 0.55));
        if item.kind == "file" {
            // A file's own type's drawing (a song's note, an archive's box), as its badge has it.
            let own = &super::item_view::files_of(item)[0];
            let t = crate::net::file_risk::file_type(&super::open::shown_file_name(item, own), None);
            widgets::file_badge::paint_icon(&painter, mark, &t.icon, widgets::file_badge::level_color(theme, t.level));
        } else {
            tiles::paint_mark(&painter, mark, kinds::mark(&item.kind), theme.text_secondary());
        }
    }

    // Three lines of words beside it, each cut short with "..." when the pane is narrow.
    let left = thumb.right() + theme.spacing_sm;
    let width = (rect.right() - pad - left).max(10.0);
    let mut y = rect.top() + pad;
    let mut line = |text: &str, size: f32, color: Color32| {
        let font = egui::FontId::proportional(size);
        let mut job = egui::text::LayoutJob::single_section(text.to_string(), egui::TextFormat { font_id: font, color, ..Default::default() });
        job.wrap = egui::text::TextWrapping::truncate_at_width(width);
        let galley = ui.fonts(|f| f.layout_job(job));
        let h = galley.size().y;
        painter.galley(egui::pos2(left, y), galley, color);
        y += h + 1.0;
    };
    line(&item.display_title(), theme.font_size_body, theme.text_strong());
    line(&super::item_view::by_line(item), theme.font_size_small, theme.text_secondary());
    let mut facts = vec![super::viewer::kind_words(item), tiles::card_counts(item), super::api::size_words(item.total_bytes.max(item.size_bytes))];
    if item.featured.is_on() {
        facts.push(filters::feature_words(item.featured).to_string());
    }
    if item.kept {
        facts.push(tiles::KEPT_BADGE.to_string());
    }
    if item.members_only() {
        facts.push("Members only".to_string());
    }
    let labels = tiles::label_names(state, item);
    let color = if labels.is_empty() { theme.text_muted() } else { theme.warning() };
    facts.extend(labels);
    line(&facts.join(", "), theme.font_size_small, color);
    resp.clicked()
}

/// The largest rect of `aspect` (width over height) inside `area`, centred.
pub(crate) fn fit(area: Rect, aspect: f32) -> Rect {
    let aspect = if aspect.is_finite() && aspect > 0.0 { aspect } else { 1.0 };
    let (mut w, mut h) = (area.width(), area.width() / aspect);
    if h > area.height() {
        h = area.height();
        w = h * aspect;
    }
    Rect::from_center_size(area.center(), egui::vec2(w, h))
}
