//! Library: the single in-app home for reference docs AND a directory of the free
//! tools/websites we point people to. A top-level tab.
//!
//! Two faces (operator 2026-06-06):
//! - DOCUMENTS: the Humanity Accord + companions (data/library/), a collapsible
//!   nested tree on the left, rendered in the right pane via widgets::markdown.
//! - EXTERNAL RESOURCES: every website as a full-width card in a single scrolling
//!   column, with a search box + tag filter (built to scale to thousands). The
//!   tags are the catalog's categories. Clicking a card opens an in-app DETAIL
//!   page (title / tags / description / url) with a "Load website" button, so a
//!   click never launches the browser immediately, the person chooses to.

use egui::{Align, CursorIcon, Frame, Label, Layout, RichText, ScrollArea, Sense, Stroke, TextEdit, Vec2};
use crate::gui::{GuiState, LibraryEntryKind};
use crate::gui::theme::Theme;
use crate::gui::widgets::markdown;

/// Which view the right pane shows.
#[derive(Clone, PartialEq)]
enum Sel {
    /// A document (section, category, entry) rendered as markdown.
    Doc(usize, usize, usize),
    /// The External Resources card list.
    Resources,
    /// One website's in-app detail page (index into the flattened website list).
    Detail(usize),
}

struct LibState {
    sel: Sel,
    initialized: bool,
    query: String,
    tag: Option<String>,
}

fn lib_state<R>(f: impl FnOnce(&mut LibState) -> R) -> R {
    use std::cell::RefCell;
    thread_local! {
        static S: RefCell<LibState> = RefCell::new(LibState {
            sel: Sel::Resources,
            initialized: false,
            query: String::new(),
            tag: None,
        });
    }
    S.with(|s| f(&mut s.borrow_mut()))
}

/// A flattened website (borrowing the loaded library). `tag` is its catalog category.
struct Website<'a> {
    title: &'a str,
    url: &'a str,
    desc: &'a str,
    tag: &'a str,
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(16.0))
        .show(ctx, |ui| {
            ui.label(RichText::new("Library").size(theme.font_size_title).color(theme.text_primary()));
            ui.label(
                RichText::new("The Humanity Accord and the reference it rests on, plus the free tools and websites we point you to.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.separator();

            if state.library.is_empty() {
                ui.label(
                    RichText::new("Nothing loaded. Run scripts/build-library.js to populate data/library/.")
                        .color(theme.text_muted()),
                );
                return;
            }

            // Flatten every website link (across all sections) into one list, tagged
            // by its catalog category. Borrows the library, no clone.
            let mut websites: Vec<Website> = Vec::new();
            for section in &state.library {
                for cat in &section.categories {
                    for entry in &cat.entries {
                        if let LibraryEntryKind::Link { url, desc } = &entry.kind {
                            websites.push(Website {
                                title: entry.title.as_str(),
                                url: url.as_str(),
                                desc: desc.as_str(),
                                tag: cat.name.as_str(),
                            });
                        }
                    }
                }
            }
            // Unique tags, in first-seen order.
            let mut tags: Vec<&str> = Vec::new();
            for w in &websites {
                if !tags.contains(&w.tag) {
                    tags.push(w.tag);
                }
            }

            // One-time selection default: the first document.
            lib_state(|s| {
                if !s.initialized {
                    s.initialized = true;
                    'find: for (si, sec) in state.library.iter().enumerate() {
                        for (ci, c) in sec.categories.iter().enumerate() {
                            for (ei, e) in c.entries.iter().enumerate() {
                                if matches!(e.kind, LibraryEntryKind::Doc(_)) {
                                    s.sel = Sel::Doc(si, ci, ei);
                                    break 'find;
                                }
                            }
                        }
                    }
                }
            });

            let rail_w = 250.0;
            let content_w = (ui.available_width() - rail_w - 24.0).max(320.0);
            let body_h = ui.available_height();
            let link_color = Theme::c32(&theme.info);

            ui.horizontal_top(|ui| {
                // ── Left rail: document tree + the External Resources entry ──
                ui.allocate_ui_with_layout(Vec2::new(rail_w, body_h), Layout::top_down(Align::Min), |ui| {
                    ScrollArea::vertical().id_salt("library_rail").auto_shrink([false, false]).show(ui, |ui| {
                        lib_state(|s| {
                            for (si, section) in state.library.iter().enumerate() {
                                let has_docs = section
                                    .categories
                                    .iter()
                                    .any(|c| c.entries.iter().any(|e| matches!(e.kind, LibraryEntryKind::Doc(_))));
                                if !has_docs {
                                    continue; // website-only sections live in External Resources
                                }
                                egui::CollapsingHeader::new(
                                    RichText::new(section.name.as_str()).size(theme.font_size_body).strong().color(theme.text_primary()),
                                )
                                .id_salt(("libsec", si))
                                .default_open(true)
                                .show(ui, |ui| {
                                    for (ci, cat) in section.categories.iter().enumerate() {
                                        let docs: Vec<(usize, &str)> = cat
                                            .entries
                                            .iter()
                                            .enumerate()
                                            .filter_map(|(ei, e)| match &e.kind {
                                                LibraryEntryKind::Doc(_) => Some((ei, e.title.as_str())),
                                                _ => None,
                                            })
                                            .collect();
                                        if docs.is_empty() {
                                            continue;
                                        }
                                        egui::CollapsingHeader::new(RichText::new(cat.name.as_str()).color(theme.accent()))
                                            .id_salt(("libcat", si, ci))
                                            .default_open(true)
                                            .show(ui, |ui| {
                                                for (ei, title) in docs {
                                                    let is_sel = s.sel == Sel::Doc(si, ci, ei);
                                                    let color = if is_sel { theme.accent() } else { theme.text_primary() };
                                                    if ui.selectable_label(is_sel, RichText::new(title).color(color)).clicked() {
                                                        s.sel = Sel::Doc(si, ci, ei);
                                                    }
                                                }
                                            });
                                    }
                                });
                            }

                            ui.add_space(theme.spacing_sm);
                            let res_active = matches!(s.sel, Sel::Resources | Sel::Detail(_));
                            let color = if res_active { theme.accent() } else { theme.text_primary() };
                            if ui
                                .selectable_label(res_active, RichText::new("External Resources").strong().color(color))
                                .clicked()
                            {
                                s.sel = Sel::Resources;
                            }
                        });
                    });
                });

                ui.separator();

                // ── Right pane ──
                ui.allocate_ui_with_layout(Vec2::new(content_w, body_h), Layout::top_down(Align::Min), |ui| {
                    lib_state(|s| match s.sel.clone() {
                        Sel::Doc(si, ci, ei) => {
                            let body = state
                                .library
                                .get(si)
                                .and_then(|sec| sec.categories.get(ci))
                                .and_then(|c| c.entries.get(ei))
                                .and_then(|e| match &e.kind {
                                    LibraryEntryKind::Doc(b) => Some(b.as_str()),
                                    _ => None,
                                });
                            ScrollArea::vertical().id_salt("library_doc").auto_shrink([false, false]).show(ui, |ui| {
                                if let Some(body) = body {
                                    markdown::render_markdown(ui, theme, body);
                                } else {
                                    ui.label(RichText::new("Select a document on the left.").size(theme.font_size_small).color(theme.text_muted()));
                                }
                            });
                        }
                        Sel::Resources => {
                            // Search + tag filter, then the full-width card column.
                            ui.horizontal(|ui| {
                                ui.add(
                                    TextEdit::singleline(&mut s.query)
                                        .hint_text("Search tools and websites")
                                        .desired_width(320.0),
                                );
                                if !s.query.is_empty() && ui.button("Clear").clicked() {
                                    s.query.clear();
                                }
                            });
                            ui.add_space(theme.spacing_xs);
                            ui.horizontal_wrapped(|ui| {
                                if tag_chip(ui, theme, "All", s.tag.is_none()) {
                                    s.tag = None;
                                }
                                for t in &tags {
                                    let active = s.tag.as_deref() == Some(*t);
                                    if tag_chip(ui, theme, t, active) {
                                        s.tag = if active { None } else { Some((*t).to_string()) };
                                    }
                                }
                            });
                            ui.separator();

                            let q = s.query.trim().to_lowercase();
                            ScrollArea::vertical().id_salt("library_cards").auto_shrink([false, false]).show(ui, |ui| {
                                let mut shown = 0usize;
                                for (idx, w) in websites.iter().enumerate() {
                                    if let Some(t) = &s.tag {
                                        if w.tag != t {
                                            continue;
                                        }
                                    }
                                    if !q.is_empty()
                                        && !w.title.to_lowercase().contains(&q)
                                        && !w.desc.to_lowercase().contains(&q)
                                        && !w.url.to_lowercase().contains(&q)
                                    {
                                        continue;
                                    }
                                    shown += 1;
                                    let card = Frame::none()
                                        .fill(theme.bg_card())
                                        .rounding(egui::Rounding::same(theme.border_radius as u8))
                                        .stroke(Stroke::new(1.0, theme.border()))
                                        .inner_margin(egui::Margin::symmetric(14, 10))
                                        .show(ui, |ui| {
                                            ui.set_width(ui.available_width());
                                            ui.label(RichText::new(w.title).size(theme.font_size_body).strong().color(theme.text_primary()));
                                            ui.label(RichText::new(w.tag).size(theme.font_size_small).color(theme.accent()));
                                            ui.label(RichText::new(w.desc).size(theme.font_size_small).color(theme.text_secondary()));
                                        });
                                    if card.response.interact(Sense::click()).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                        s.sel = Sel::Detail(idx);
                                    }
                                    ui.add_space(8.0);
                                }
                                if shown == 0 {
                                    ui.label(RichText::new("No matches.").size(theme.font_size_small).color(theme.text_muted()));
                                }
                            });
                        }
                        Sel::Detail(idx) => {
                            let Some(w) = websites.get(idx) else {
                                s.sel = Sel::Resources;
                                return;
                            };
                            if ui.button("Back to resources").clicked() {
                                s.sel = Sel::Resources;
                            }
                            ui.add_space(theme.spacing_sm);
                            ScrollArea::vertical().id_salt("library_detail").auto_shrink([false, false]).show(ui, |ui| {
                                ui.label(RichText::new(w.title).size(theme.font_size_heading).strong().color(theme.text_primary()));
                                ui.label(RichText::new(w.tag).size(theme.font_size_small).color(theme.accent()));
                                ui.add_space(theme.spacing_sm);
                                ui.label(RichText::new(w.desc).size(theme.font_size_body).color(theme.text_secondary()));
                                ui.add_space(theme.spacing_sm);
                                ui.label(RichText::new(w.url).size(theme.font_size_small).color(link_color));
                                ui.add_space(theme.spacing_md);
                                if widgets_button_load(ui, theme) {
                                    ui.ctx().open_url(egui::OpenUrl::new_tab(w.url.to_string()));
                                }
                                ui.add_space(theme.spacing_xs);
                                ui.label(
                                    RichText::new("Opens in your browser. (An in-app browser is planned.)")
                                        .size(theme.font_size_small)
                                        .color(theme.text_muted()),
                                );
                            });
                        }
                    });
                });
            });
        });
}

/// A clickable tag-filter chip. Returns true when clicked this frame.
fn tag_chip(ui: &mut egui::Ui, theme: &Theme, label: &str, active: bool) -> bool {
    let (fill, text) = if active {
        (theme.accent(), theme.bg_primary())
    } else {
        (theme.bg_card(), theme.text_secondary())
    };
    let mut clicked = false;
    Frame::none()
        .fill(fill)
        .rounding(egui::Rounding::same(10))
        .inner_margin(egui::Margin::symmetric(10, 3))
        .stroke(Stroke::new(1.0, theme.border()))
        .show(ui, |ui| {
            let resp = ui.add(Label::new(RichText::new(label).size(theme.font_size_small).color(text)).sense(Sense::click()));
            if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                clicked = true;
            }
        });
    ui.add_space(4.0);
    clicked
}

/// The "Load website" button (accent, prominent).
fn widgets_button_load(ui: &mut egui::Ui, theme: &Theme) -> bool {
    crate::gui::widgets::Button::primary("Load website").show(ui, theme)
}
