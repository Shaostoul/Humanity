//! Library: the single in-app home for reference docs AND the directory of free
//! tools/websites we point people to (the Resources page was retired into here
//! 2026-06-06). A top-level tab. Loaded data-driven into collapsible sections:
//! "HumanityOS" (the Accord + companions, from `data/library/`) and "Tools and
//! Websites" (the curated external links, from `data/resources/catalog.json`).
//!
//! Built to scale: the "Tools and Websites" set could grow to thousands of
//! entries, so the top-level sections AND their categories collapse, and a search
//! box filters every entry by title / url / description. Left: the nested tree.
//! Right: the selected document (links open in the browser).

use egui::{Align, Frame, Layout, RichText, ScrollArea, TextEdit, Vec2};
use crate::gui::{GuiState, LibraryEntry, LibraryEntryKind};
use crate::gui::theme::Theme;
use crate::gui::widgets::markdown;

/// Page-local state: the open document + the search query.
struct LibState {
    selected: Option<(usize, usize, usize)>,
    query: String,
}

fn lib_state<R>(f: impl FnOnce(&mut LibState) -> R) -> R {
    use std::cell::RefCell;
    thread_local! {
        static S: RefCell<LibState> = RefCell::new(LibState { selected: None, query: String::new() });
    }
    S.with(|s| f(&mut s.borrow_mut()))
}

/// True if an entry matches the (already-lowercased) query. Searches titles, plus
/// the url + description for website links. Doc bodies are not searched (they can
/// be large); titles are enough to find them.
fn entry_matches(e: &LibraryEntry, q: &str) -> bool {
    if e.title.to_lowercase().contains(q) {
        return true;
    }
    match &e.kind {
        LibraryEntryKind::Link { url, desc } => {
            url.to_lowercase().contains(q) || desc.to_lowercase().contains(q)
        }
        LibraryEntryKind::Doc(_) => false,
    }
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
            ui.add_space(theme.spacing_sm);

            // Search box: the key to scaling the websites list to thousands.
            lib_state(|s| {
                ui.horizontal(|ui| {
                    ui.add(
                        TextEdit::singleline(&mut s.query)
                            .hint_text("Filter documents and websites")
                            .desired_width(360.0),
                    );
                    if !s.query.is_empty() && ui.button("Clear").clicked() {
                        s.query.clear();
                    }
                });
            });
            ui.separator();

            if state.library.is_empty() {
                ui.label(
                    RichText::new("Nothing loaded. Run scripts/build-library.js to populate data/library/.")
                        .color(theme.text_muted()),
                );
                return;
            }

            let query = lib_state(|s| s.query.trim().to_lowercase());
            let filtering = !query.is_empty();

            // Auto-select the first document so the right pane is never blank.
            lib_state(|s| {
                if s.selected.is_none() {
                    'find: for (si, sec) in state.library.iter().enumerate() {
                        for (ci, c) in sec.categories.iter().enumerate() {
                            for (ei, e) in c.entries.iter().enumerate() {
                                if matches!(e.kind, LibraryEntryKind::Doc(_)) {
                                    s.selected = Some((si, ci, ei));
                                    break 'find;
                                }
                            }
                        }
                    }
                }
            });

            let rail_w = 280.0;
            let content_w = (ui.available_width() - rail_w - 24.0).max(280.0);
            let body_h = ui.available_height();
            let link_color = Theme::c32(&theme.info);

            ui.horizontal_top(|ui| {
                // Left rail: collapsible section -> collapsible category -> entries.
                ui.allocate_ui_with_layout(Vec2::new(rail_w, body_h), Layout::top_down(Align::Min), |ui| {
                    ScrollArea::vertical().id_salt("library_rail").auto_shrink([false, false]).show(ui, |ui| {
                        lib_state(|s| {
                            let mut any_shown = false;
                            for (si, section) in state.library.iter().enumerate() {
                                // Skip sections with no matching entry while filtering.
                                let section_has_match = !filtering
                                    || section.categories.iter().any(|c| c.entries.iter().any(|e| entry_matches(e, &query)));
                                if !section_has_match {
                                    continue;
                                }
                                any_shown = true;
                                egui::CollapsingHeader::new(
                                    RichText::new(section.name.as_str())
                                        .size(theme.font_size_body)
                                        .strong()
                                        .color(theme.text_primary()),
                                )
                                .id_salt(("libsec", si))
                                .default_open(si == 0 || filtering)
                                .show(ui, |ui| {
                                    for (ci, cat) in section.categories.iter().enumerate() {
                                        let visible: Vec<(usize, &LibraryEntry)> = cat
                                            .entries
                                            .iter()
                                            .enumerate()
                                            .filter(|(_, e)| !filtering || entry_matches(e, &query))
                                            .collect();
                                        if visible.is_empty() {
                                            continue;
                                        }
                                        egui::CollapsingHeader::new(
                                            RichText::new(cat.name.as_str()).color(theme.accent()),
                                        )
                                        .id_salt(("libcat", si, ci))
                                        .default_open(filtering || si == 0)
                                        .show(ui, |ui| {
                                            for (ei, entry) in visible {
                                                match &entry.kind {
                                                    LibraryEntryKind::Doc(_) => {
                                                        let is_sel = s.selected == Some((si, ci, ei));
                                                        let color = if is_sel { theme.accent() } else { theme.text_primary() };
                                                        if ui
                                                            .selectable_label(is_sel, RichText::new(entry.title.as_str()).color(color))
                                                            .clicked()
                                                        {
                                                            s.selected = Some((si, ci, ei));
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
                                });
                            }
                            if !any_shown {
                                ui.label(
                                    RichText::new("No matches.")
                                        .size(theme.font_size_small)
                                        .color(theme.text_muted()),
                                );
                            }
                        });
                    });
                });

                ui.separator();

                // Right: the selected document (links open in the browser instead).
                ui.allocate_ui_with_layout(Vec2::new(content_w, body_h), Layout::top_down(Align::Min), |ui| {
                    let sel = lib_state(|s| s.selected);
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
