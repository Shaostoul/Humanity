//! Library: an in-app reader for the project's reference docs and a directory of
//! the free tools and websites we point people to. A top-level tab (operator
//! 2026-06-06). Loaded data-driven into sections: "HumanityOS" (the Humanity
//! Accord + companions, from `data/library/`) and "Tools and Websites" (the
//! curated external links shared with the Resources page, from
//! `data/resources/catalog.json`). Left: a nested section -> category -> entry
//! tree. Right: the selected document, rendered by the shared `widgets::markdown`
//! reader. Website entries open in your browser.

use egui::{Align, Frame, Layout, RichText, ScrollArea, Vec2};
use crate::gui::{GuiState, LibraryEntryKind};
use crate::gui::theme::Theme;
use crate::gui::widgets::markdown;

/// Page-local selection of the open document: (section, category, entry) indices.
fn selected<R>(f: impl FnOnce(&mut Option<(usize, usize, usize)>) -> R) -> R {
    use std::cell::RefCell;
    thread_local! {
        static SEL: RefCell<Option<(usize, usize, usize)>> = RefCell::new(None);
    }
    SEL.with(|s| f(&mut s.borrow_mut()))
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(16.0))
        .show(ctx, |ui| {
            ui.label(RichText::new("Library").size(theme.font_size_title).color(theme.text_primary()));
            ui.label(
                RichText::new("The Humanity Accord and the reference it rests on, plus the free tools and websites we point you to. Read a document, or click a website to open it in your browser.")
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

            // Auto-select the first document so the right pane is never blank.
            selected(|sel| {
                if sel.is_none() {
                    'find: for (si, s) in state.library.iter().enumerate() {
                        for (ci, c) in s.categories.iter().enumerate() {
                            for (ei, e) in c.entries.iter().enumerate() {
                                if matches!(e.kind, LibraryEntryKind::Doc(_)) {
                                    *sel = Some((si, ci, ei));
                                    break 'find;
                                }
                            }
                        }
                    }
                }
            });

            let rail_w = 260.0;
            let content_w = (ui.available_width() - rail_w - 24.0).max(280.0);
            let body_h = ui.available_height();
            let link_color = Theme::c32(&theme.info);

            ui.horizontal_top(|ui| {
                // Left rail: section -> category -> entry tree.
                ui.allocate_ui_with_layout(Vec2::new(rail_w, body_h), Layout::top_down(Align::Min), |ui| {
                    ScrollArea::vertical().id_salt("library_rail").auto_shrink([false, false]).show(ui, |ui| {
                        selected(|sel| {
                            for (si, section) in state.library.iter().enumerate() {
                                ui.add_space(theme.spacing_xs);
                                ui.label(
                                    RichText::new(section.name.as_str())
                                        .size(theme.font_size_body)
                                        .strong()
                                        .color(theme.text_primary()),
                                );
                                for (ci, cat) in section.categories.iter().enumerate() {
                                    egui::CollapsingHeader::new(
                                        RichText::new(cat.name.as_str()).color(theme.accent()),
                                    )
                                    .id_salt(("lib", si, ci))
                                    .default_open(si == 0)
                                    .show(ui, |ui| {
                                        for (ei, entry) in cat.entries.iter().enumerate() {
                                            match &entry.kind {
                                                LibraryEntryKind::Doc(_) => {
                                                    let is_sel = *sel == Some((si, ci, ei));
                                                    let color = if is_sel { theme.accent() } else { theme.text_primary() };
                                                    if ui
                                                        .selectable_label(is_sel, RichText::new(entry.title.as_str()).color(color))
                                                        .clicked()
                                                    {
                                                        *sel = Some((si, ci, ei));
                                                    }
                                                }
                                                LibraryEntryKind::Link { url, desc } => {
                                                    let resp = ui.hyperlink_to(
                                                        RichText::new(entry.title.as_str()).color(link_color),
                                                        url.as_str(),
                                                    );
                                                    if !desc.is_empty() {
                                                        resp.on_hover_text(desc.as_str());
                                                    }
                                                }
                                            }
                                        }
                                    });
                                }
                                ui.add_space(theme.spacing_sm);
                            }
                        });
                    });
                });

                ui.separator();

                // Right: the selected document (links open in the browser instead).
                ui.allocate_ui_with_layout(Vec2::new(content_w, body_h), Layout::top_down(Align::Min), |ui| {
                    let sel = selected(|s| *s);
                    let body = sel.and_then(|(si, ci, ei)| {
                        state
                            .library
                            .get(si)
                            .and_then(|s| s.categories.get(ci))
                            .and_then(|c| c.entries.get(ei))
                            .and_then(|e| match &e.kind {
                                LibraryEntryKind::Doc(b) => Some(b.as_str()),
                                LibraryEntryKind::Link { .. } => None,
                            })
                    });
                    if let Some(body) = body {
                        ScrollArea::vertical()
                            .id_salt("library_doc")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                markdown::render_markdown(ui, theme, body);
                            });
                    } else {
                        ui.label(
                            RichText::new("Select a document on the left, or click a website to open it.")
                                .size(theme.font_size_small)
                                .color(theme.text_muted()),
                        );
                    }
                });
            });
        });
}
