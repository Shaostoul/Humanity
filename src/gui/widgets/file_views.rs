//! The file picker's ways to show a folder (operator, 2026-10-17: "different view modes kinda like
//! windows file explorer but, better. List, detail, icons, then like a slider for preview size ...
//! Maybe someone wants to look at 1 image at a time to scroll through at full preview size. Then 2
//! could split it evenly so there's like 2 columns. Then 3 for 3 columns, etc."):
//!
//! - List: one row each, its type's badge first (widgets/file_badge.rs), then its name and size.
//! - Details: a table of type, name, size and date changed, sorted by a click on a heading.
//! - Pictures: a grid 1 to 8 across ("Across"): a picture's own preview (widgets/thumbnails.rs,
//!   read on workers, only what is on screen), any other file its type's drawing, large, in its
//!   type's colour, and every cell its badge and its name. One across shows one picture at a time
//!   at the window's full width, to scroll through.
//!
//! Which view and how many across are remembered on this computer (file_picker_memory.rs). The
//! views only draw and say what was pressed ([`Act`]); the picker does it, so all three choose,
//! tick and open folders the same way.

use super::file_browser::{human_size, FsEntry};
use super::file_picker_memory::PickerView;
use crate::gui::theme::Theme;
use crate::net::file_risk::{self, FileType};
use egui::{Pos2, Rect, RichText, Sense, Stroke, Vec2};
use std::path::{Path, PathBuf};

/// What was pressed in a view.
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    /// A folder: open it.
    Open(PathBuf),
    /// A file, in a picker for one: it is the one chosen (Choose takes it).
    Select(FsEntry),
    /// A file, in a picker for several: ticked or unticked.
    Toggle(FsEntry),
    /// A file, double-clicked: chosen at once.
    Pick(PathBuf),
    /// One of the links beside a file ([`Look::notes`]): the file, and which of its links.
    Note(PathBuf, usize),
}

/// The links a view draws beside files (the Media page's Your folders: "On united-humanity.us as
/// Dusk"), by the file's path: each its words and its hover words.
pub type Notes = std::collections::HashMap<PathBuf, Vec<(String, String)>>;

/// What the Details view is sorted by.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SortBy {
    #[default]
    Name,
    Kind,
    Size,
    Changed,
}

/// What the views need to know about the picker.
pub struct Look<'a> {
    pub selected: Option<&'a Path>,
    pub chosen: &'a [FsEntry],
    pub multiple: bool,
    pub dir_mode: bool,
    pub max_size: u64,
    /// Links drawn beside files, each a click away ([`Act::Note`]), with the heading of their
    /// column in the Details view: None in the file picker.
    pub notes: Option<(&'a str, &'a Notes)>,
}

impl Look<'_> {
    /// The links beside `e`, as link-styled buttons in a row; which was clicked, if one was.
    fn note_links(&self, ui: &mut egui::Ui, theme: &Theme, e: &FsEntry) -> Option<Act> {
        let notes = self.notes?.1.get(&e.path)?;
        let mut act = None;
        for (i, (words, tip)) in notes.iter().enumerate() {
            if ui.link(RichText::new(words).size(theme.font_size_small).color(theme.accent())).on_hover_text(tip).clicked() {
                act = Some(Act::Note(e.path.clone(), i));
            }
        }
        act
    }

    fn too_big(&self, e: &FsEntry) -> bool {
        !e.is_dir && self.max_size > 0 && e.size > self.max_size
    }
    fn is_chosen(&self, e: &FsEntry) -> bool {
        self.chosen.iter().any(|c| c.path == e.path)
    }
    fn is_selected(&self, e: &FsEntry) -> bool {
        self.selected == Some(e.path.as_path())
    }
    /// What a click on file `e` does: tick or choose it (nothing for a file over the size limit).
    fn click(&self, e: &FsEntry) -> Option<Act> {
        if e.is_dir {
            Some(Act::Open(e.path.clone()))
        } else if self.too_big(e) || self.dir_mode {
            None
        } else if self.multiple {
            Some(Act::Toggle(e.clone()))
        } else {
            Some(Act::Select(e.clone()))
        }
    }
    /// What a double-click on file `e` does: choose it at once (a picker for one).
    fn double_click(&self, e: &FsEntry) -> Option<Act> {
        (!e.is_dir && !self.too_big(e) && !self.dir_mode && !self.multiple).then(|| Act::Pick(e.path.clone()))
    }
}

/// The views' names, as their buttons say them.
pub const VIEW_NAMES: [(&str, PickerView); 3] = [("List", PickerView::List), ("Details", PickerView::Details), ("Pictures", PickerView::Pictures)];
/// The slider's word.
pub const ACROSS: &str = "Across";
/// How many pictures to a row at most.
pub const MAX_ACROSS: u8 = 8;

/// An entry's type, for its badge: a folder's, or a file's by its name.
pub fn type_of(e: &FsEntry) -> FileType {
    if e.is_dir {
        super::file_badge::folder()
    } else {
        file_risk::file_type(&e.name, None)
    }
}

/// The buttons that choose the view and, in Pictures, the slider for how many across. True when
/// either changed.
pub fn toolbar(ui: &mut egui::Ui, theme: &Theme, view: &mut PickerView, across: &mut u8) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Show").size(theme.font_size_small).color(theme.text_secondary()));
        for (name, v) in VIEW_NAMES {
            if ui.selectable_label(*view == v, RichText::new(name).size(theme.font_size_small)).clicked() && *view != v {
                *view = v;
                changed = true;
            }
        }
        if *view == PickerView::Pictures {
            ui.add_space(theme.spacing_sm);
            let moved = crate::gui::widgets::slider(ui, theme, egui::Slider::new(across, 1..=MAX_ACROSS).text(ACROSS).integer());
            if moved.on_hover_text("How many pictures to a row: 1 shows one at a time, as large as the window.").changed() {
                changed = true;
            }
        }
    });
    changed
}

/// The List view: a row each, its badge first.
pub fn list(ui: &mut egui::Ui, theme: &Theme, entries: &[FsEntry], look: &Look) -> Option<Act> {
    let mut act = None;
    for e in entries {
        let too_big = look.too_big(e);
        let color = if too_big { theme.text_muted() } else if e.is_dir { theme.accent() } else { theme.text_primary() };
        let label = if e.is_dir { e.name.clone() } else { format!("{}  ({})", e.name, human_size(e.size)) };
        ui.horizontal(|ui| {
            super::file_badge::show(ui, theme, &type_of(e));
            if look.multiple && !look.dir_mode && !e.is_dir {
                let mut on = look.is_chosen(e);
                let tick = egui::Checkbox::new(&mut on, RichText::new(&label).size(theme.font_size_small).color(color));
                if ui.add_enabled(!too_big, tick).changed() {
                    act = look.click(e);
                }
            } else {
                let resp = ui.selectable_label(look.is_selected(e), RichText::new(&label).size(theme.font_size_small).color(color));
                if resp.double_clicked() {
                    act = look.double_click(e).or(act.take());
                } else if resp.clicked() {
                    act = look.click(e);
                }
            }
            if let Some(a) = look.note_links(ui, theme, e) {
                act = Some(a);
            }
        });
    }
    act
}

/// The entries sorted for the Details view: folders first, then files, each by `by` (`desc`
/// reverses it).
pub fn sorted(entries: &[FsEntry], by: SortBy, desc: bool) -> Vec<FsEntry> {
    let mut v = entries.to_vec();
    v.sort_by(|a, b| {
        let order = b.is_dir.cmp(&a.is_dir).then_with(|| {
            let o = match by {
                SortBy::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortBy::Kind => type_of(a).label.cmp(&type_of(b).label).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
                SortBy::Size => a.size.cmp(&b.size),
                SortBy::Changed => a.modified.cmp(&b.modified),
            };
            if desc {
                o.reverse()
            } else {
                o
            }
        });
        order
    });
    v
}

/// A date changed as words: "2026-10-17 21:03 UTC", or empty when not known.
pub fn changed_words(t: Option<std::time::SystemTime>) -> String {
    // In the zone Settings > Time zone says, as the chat's times are (chat/timestamps.rs).
    let Some(ms) = t.and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as u64) else { return String::new() };
    crate::gui::pages::chat::format_date_minute(ms)
}

/// The Details view: a heading row (a click sorts by it, a second click reverses it), then a row
/// each: its badge, its name (a tick first in a picker for several), its size, its date changed.
pub fn details(ui: &mut egui::Ui, theme: &Theme, entries: &[FsEntry], look: &Look, by: &mut SortBy, desc: &mut bool) -> Option<Act> {
    let mut act = None;
    let small = |t: &str| RichText::new(t).size(theme.font_size_small);
    let columns = if look.notes.is_some() { 5 } else { 4 };
    egui::Grid::new("file_picker_details").num_columns(columns).striped(true).spacing([theme.spacing_md, theme.spacing_xs]).show(ui, |ui| {
        for (name, key) in [("Type", SortBy::Kind), ("Name", SortBy::Name), ("Size", SortBy::Size), ("Changed", SortBy::Changed)] {
            let on = *by == key;
            let text = if on { small(name).strong().color(theme.accent()) } else { small(name).color(theme.text_secondary()) };
            let tip = if on {
                if *desc { "Sorted by this, the other way round. Click to reverse." } else { "Sorted by this. Click to reverse." }
            } else {
                "Click to sort by this."
            };
            if ui.add(egui::Label::new(text).sense(Sense::click())).on_hover_text(tip).clicked() {
                if on {
                    *desc = !*desc;
                } else {
                    *by = key;
                    *desc = false;
                }
            }
        }
        // The links' own column, where a view has them (not sortable).
        if let Some((heading, _)) = look.notes {
            ui.label(small(heading).color(theme.text_secondary()));
        }
        ui.end_row();
        for e in sorted(entries, *by, *desc) {
            let too_big = look.too_big(&e);
            let color = if too_big { theme.text_muted() } else if e.is_dir { theme.accent() } else { theme.text_primary() };
            super::file_badge::show(ui, theme, &type_of(&e));
            if look.multiple && !look.dir_mode && !e.is_dir {
                let mut on = look.is_chosen(&e);
                if ui.add_enabled(!too_big, egui::Checkbox::new(&mut on, small(&e.name).color(color))).changed() {
                    act = look.click(&e);
                }
            } else {
                let resp = ui.selectable_label(look.is_selected(&e), small(&e.name).color(color));
                if resp.double_clicked() {
                    act = look.double_click(&e).or(act.take());
                } else if resp.clicked() {
                    act = look.click(&e);
                }
            }
            ui.label(small(&if e.is_dir { String::new() } else { human_size(e.size) }).color(theme.text_secondary()));
            ui.label(small(&changed_words(e.modified)).color(theme.text_secondary()));
            if look.notes.is_some() {
                let clicked = ui.horizontal_wrapped(|ui| look.note_links(ui, theme, &e)).inner;
                if clicked.is_some() {
                    act = clicked;
                }
            }
            ui.end_row();
        }
    });
    act
}

/// The Pictures view: `across` to a row (1 to 8), each cell a preview (a picture's own, any other
/// file's type drawn large in its colour), its badge over the preview's corner, its name under it,
/// and in a picker for several a tick in the other corner.
pub fn pictures(ui: &mut egui::Ui, theme: &Theme, entries: &[FsEntry], look: &Look, across: u8) -> Option<Act> {
    let mut act = None;
    let across = across.clamp(1, MAX_ACROSS) as usize;
    let gap = theme.spacing_sm;
    let width = ((ui.available_width() - gap * (across as f32 - 1.0)) / across as f32).floor().max(48.0);
    let name_h = theme.font_size_small + 6.0;
    for row in entries.chunks(across) {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for e in row {
                // A file with links has them under its cell, the cell's width wide.
                let pressed = if look.notes.is_some_and(|(_, n)| n.contains_key(&e.path)) {
                    ui.vertical(|ui| {
                        ui.set_width(width);
                        let a = cell(ui, theme, e, look, width, name_h, across == 1);
                        ui.horizontal_wrapped(|ui| look.note_links(ui, theme, e)).inner.or(a)
                    })
                    .inner
                } else {
                    cell(ui, theme, e, look, width, name_h, across == 1)
                };
                if let Some(a) = pressed {
                    act = Some(a);
                }
            }
        });
        ui.add_space(gap);
    }
    act
}

/// One cell of the Pictures view.
fn cell(ui: &mut egui::Ui, theme: &Theme, e: &FsEntry, look: &Look, width: f32, name_h: f32, alone: bool) -> Option<Act> {
    let ctx = ui.ctx().clone();
    let px = width * ctx.pixels_per_point();
    let picture = !e.is_dir && super::thumbnails::is_previewable(&e.name);
    // The preview's height: square, or, one across, the picture's own shape (a little shorter while
    // it is read).
    let mut preview_h = if alone { width * 0.6 } else { width };
    let mut ready = None;
    // Only a cell on screen is read (the space above it decides; a guess, refined next frame).
    let guess = Rect::from_min_size(ui.cursor().min, Vec2::new(width, preview_h + name_h));
    if picture && ui.is_rect_visible(guess) {
        ready = super::thumbnails::get(&ctx, &e.path, px);
        if let (true, Some((_, size))) = (alone, ready) {
            preview_h = (width * size.y / size.x.max(1.0)).min(width * 1.5);
        }
    }
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, preview_h + name_h), Sense::click());
    if !ui.is_rect_visible(rect) {
        return None;
    }
    let painter = ui.painter_at(rect.expand(2.0));
    let t = type_of(e);
    let level = super::file_badge::level_color(theme, t.level);
    let too_big = look.too_big(e);
    let chosen = look.is_chosen(e) || look.is_selected(e);
    let preview = Rect::from_min_size(rect.min, Vec2::new(width, preview_h));
    painter.rect_filled(preview, theme.border_radius, theme.bg_card());
    match ready {
        Some((tex, size)) => {
            let scale = (preview.width() / size.x).min(preview.height() / size.y);
            let shown = Rect::from_center_size(preview.center(), size * scale);
            painter.image(tex, shown, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), if too_big { theme.text_muted() } else { egui::Color32::WHITE }); // theme-exempt: an image's own colours, untinted
        }
        None => {
            let side = (preview.width().min(preview.height()) * 0.5).max(16.0);
            let icon = Rect::from_center_size(preview.center(), Vec2::splat(side));
            let color = if e.is_dir { theme.accent() } else { level };
            super::file_badge::paint_icon(&painter, icon, &t.icon, if picture { color.gamma_multiply(0.5) } else { color });
        }
    }
    // Its type, always: the badge over the preview's top left.
    let badge_at = preview.min + Vec2::splat(4.0);
    let badge = super::file_badge::measure(&painter, theme, &t);
    if badge.x <= preview.width() - 8.0 {
        super::file_badge::paint(&painter, theme, &t, badge_at);
    }
    // In a picker for several, the tick in the top right.
    if look.multiple && !look.dir_mode && !e.is_dir {
        let tick = Rect::from_min_size(Pos2::new(preview.right() - 22.0, preview.top() + 4.0), Vec2::splat(18.0));
        painter.rect(tick, 3.0, theme.bg_card(), Stroke::new(1.5, theme.text_primary()), egui::StrokeKind::Inside);
        if look.is_chosen(e) {
            super::icons::paint_check(&painter, tick.shrink(3.0), theme.accent());
        }
    }
    // Its name under the preview, cut to one line.
    let mut job = egui::text::LayoutJob::simple_singleline(e.name.clone(), egui::FontId::proportional(theme.font_size_small), if too_big { theme.text_muted() } else if e.is_dir { theme.accent() } else { theme.text_primary() });
    job.wrap = egui::text::TextWrapping { max_width: width - 4.0, max_rows: 1, break_anywhere: true, overflow_character: Some('\u{2026}') };
    let galley = painter.layout_job(job);
    painter.galley(Pos2::new(rect.left() + 2.0, preview.bottom() + 3.0), galley, theme.text_primary());
    if chosen {
        painter.rect_stroke(preview, theme.border_radius, Stroke::new(2.0, theme.accent()), egui::StrokeKind::Inside);
    } else if resp.hovered() {
        painter.rect_stroke(preview, theme.border_radius, Stroke::new(1.0, theme.border()), egui::StrokeKind::Inside);
    }
    let resp = resp.on_hover_text(if e.is_dir { e.name.clone() } else { format!("{} ({})", e.name, human_size(e.size)) });
    if resp.double_clicked() {
        look.double_click(e).or_else(|| e.is_dir.then(|| Act::Open(e.path.clone())))
    } else if resp.clicked() {
        look.click(e)
    } else {
        None
    }
}

#[cfg(test)]
#[path = "file_views_tests.rs"]
mod tests;
