//! Library: the single in-app home for reference docs AND a directory of the free
//! tools/websites we point people to. A top-level tab.
//!
//! Two faces (operator 2026-06-06):
//! - DOCUMENTS: the Humanity Accord + companions (data/library/), a collapsible
//!   nested tree on the left, rendered in the right pane via widgets::markdown.
//! - EXTERNAL RESOURCES: every website as a full-width, self-contained card in a
//!   single scrolling column, with a search box + tag filter (built to scale to
//!   thousands). The tags are the catalog's categories. Each card carries a "Load
//!   website" button at the TOP, then all of its data (title / tag / description /
//!   url), so a click never launches the browser on its own, the person chooses to.

use egui::{Align, CursorIcon, Frame, Label, Layout, RichText, ScrollArea, Sense, Stroke, TextEdit, Vec2};
use crate::gui::{GuiState, LibraryEntryKind};
use crate::gui::theme::Theme;
use crate::gui::widgets::markdown;

/// Which view the right pane shows.
#[derive(Clone, PartialEq)]
enum Sel {
    /// A document (section, category, entry) rendered as markdown.
    Doc(usize, usize, usize),
    /// The External Resources card list (each card is self-contained).
    Resources,
    /// The Dictionary: every glossary term, searchable + category-filtered
    /// (v0.989, operator: "assume people aren't going to know all the
    /// words so we should have a way of quickly learning words").
    Dictionary,
}

struct LibState {
    sel: Sel,
    initialized: bool,
    query: String,
    tag: Option<String>,
    /// Dictionary search text (separate from the resources query so
    /// switching views never clobbers either).
    dict_query: String,
    /// Dictionary category filter (glossary category id).
    dict_cat: Option<String>,
    /// "Define words" toggle for the doc pane: on = every word in the open
    /// document is clickable for a definition.
    define_mode: bool,
    /// A word the reader clicked in define mode - drives the popup.
    define_popup: Option<String>,
}

fn lib_state<R>(f: impl FnOnce(&mut LibState) -> R) -> R {
    use std::cell::RefCell;
    thread_local! {
        static S: RefCell<LibState> = RefCell::new(LibState {
            sel: Sel::Resources,
            initialized: false,
            query: String::new(),
            tag: None,
            dict_query: String::new(),
            dict_cat: None,
            define_mode: false,
            define_popup: None,
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
                            let dict_active = s.sel == Sel::Dictionary;
                            let dcolor = if dict_active { theme.accent() } else { theme.text_primary() };
                            if ui
                                .selectable_label(dict_active, RichText::new("Dictionary").strong().color(dcolor))
                                .clicked()
                            {
                                s.sel = Sel::Dictionary;
                            }
                            let res_active = s.sel == Sel::Resources;
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
                            // Define-words toggle (v0.989): on = click any word
                            // in the document for its definition; dictionary
                            // hits show underlined. Plain fast rendering when off.
                            ui.horizontal(|ui| {
                                let label = if s.define_mode {
                                    "Define words: ON (click any word)"
                                } else {
                                    "Define words"
                                };
                                if crate::gui::widgets::Button::secondary(label).show(ui, theme) {
                                    s.define_mode = !s.define_mode;
                                    if !s.define_mode {
                                        s.define_popup = None;
                                    }
                                }
                                if s.define_mode {
                                    ui.label(
                                        RichText::new("Underlined words are in the dictionary.")
                                            .size(theme.font_size_small)
                                            .color(theme.text_muted()),
                                    );
                                }
                            });
                            ui.add_space(theme.spacing_xs);
                            ScrollArea::vertical().id_salt("library_doc").auto_shrink([false, false]).show(ui, |ui| {
                                if let Some(body) = body {
                                    if s.define_mode {
                                        let mut clicked: Option<String> = None;
                                        markdown::render_markdown_defining(ui, theme, body, &mut clicked);
                                        if clicked.is_some() {
                                            s.define_popup = clicked;
                                        }
                                    } else {
                                        markdown::render_markdown(ui, theme, body);
                                    }
                                } else {
                                    ui.label(RichText::new("Select a document on the left.").size(theme.font_size_small).color(theme.text_muted()));
                                }
                            });
                            // Definition popup: whatever word was last clicked.
                            if let Some(word) = s.define_popup.clone() {
                                let gl = crate::gui::glossary::glossary();
                                let hit = gl.lookup_word(&word).cloned();
                                let mut open = true;
                                egui::Window::new(RichText::new("Definition").strong())
                                    .id(egui::Id::new("library_define_popup"))
                                    .collapsible(false)
                                    .resizable(false)
                                    .open(&mut open)
                                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                                    .show(ui.ctx(), |ui| {
                                        ui.set_max_width(420.0);
                                        match hit {
                                            Some(e) => {
                                                ui.label(RichText::new(&e.term).size(theme.font_size_heading).strong().color(theme.accent()));
                                                if let Some(cat) = gl.category_name(&e.category) {
                                                    ui.label(RichText::new(cat).size(theme.font_size_small).color(theme.text_muted()));
                                                }
                                                ui.add_space(theme.spacing_xs);
                                                ui.label(RichText::new(&e.definition).size(theme.font_size_body).color(theme.text_primary()));
                                            }
                                            None => {
                                                let bare: String = word
                                                    .trim_matches(|c: char| !c.is_alphanumeric())
                                                    .to_string();
                                                ui.label(RichText::new(format!("\"{bare}\" isn't in the dictionary yet."))
                                                    .size(theme.font_size_body)
                                                    .color(theme.text_primary()));
                                                ui.add_space(theme.spacing_xs);
                                                if crate::gui::widgets::Button::secondary("Search the Dictionary").show(ui, theme) {
                                                    s.dict_query = bare;
                                                    s.sel = Sel::Dictionary;
                                                    s.define_popup = None;
                                                }
                                            }
                                        }
                                    });
                                if !open {
                                    s.define_popup = None;
                                }
                            }
                        }
                        Sel::Dictionary => {
                            let gl = crate::gui::glossary::glossary();
                            ui.horizontal(|ui| {
                                ui.add(
                                    TextEdit::singleline(&mut s.dict_query)
                                        .hint_text("Search words and definitions")
                                        .desired_width(320.0),
                                );
                                if !s.dict_query.is_empty() && ui.button("Clear").clicked() {
                                    s.dict_query.clear();
                                }
                                ui.label(
                                    RichText::new(format!("{} terms", gl.len()))
                                        .size(theme.font_size_small)
                                        .color(theme.text_muted()),
                                );
                            });
                            ui.add_space(theme.spacing_xs);
                            ui.horizontal_wrapped(|ui| {
                                if tag_chip(ui, theme, "All", s.dict_cat.is_none()) {
                                    s.dict_cat = None;
                                }
                                for id in gl.category_ids() {
                                    let name = gl.category_name(id).unwrap_or(id);
                                    let active = s.dict_cat.as_deref() == Some(id);
                                    if tag_chip(ui, theme, name, active) {
                                        s.dict_cat = if active { None } else { Some(id.to_string()) };
                                    }
                                }
                            });
                            ui.separator();

                            let q = s.dict_query.trim().to_lowercase();
                            ScrollArea::vertical().id_salt("library_dict").auto_shrink([false, false]).show(ui, |ui| {
                                let mut shown = 0usize;
                                for e in gl.entries_sorted() {
                                    if let Some(cat) = &s.dict_cat {
                                        if &e.category != cat {
                                            continue;
                                        }
                                    }
                                    if !q.is_empty()
                                        && !e.term.to_lowercase().contains(&q)
                                        && !e.definition.to_lowercase().contains(&q)
                                    {
                                        continue;
                                    }
                                    shown += 1;
                                    Frame::none()
                                        .fill(theme.bg_card())
                                        .rounding(egui::Rounding::same(theme.border_radius as u8))
                                        .stroke(Stroke::new(1.0, theme.border()))
                                        .inner_margin(egui::Margin::symmetric(14, 10))
                                        .show(ui, |ui| {
                                            ui.set_width(ui.available_width());
                                            ui.label(RichText::new(&e.term).size(theme.font_size_body).strong().color(theme.text_primary()));
                                            if let Some(cat) = gl.category_name(&e.category) {
                                                ui.label(RichText::new(cat).size(theme.font_size_small).color(theme.accent()));
                                            }
                                            ui.label(RichText::new(&e.definition).size(theme.font_size_small).color(theme.text_secondary()));
                                        });
                                    ui.add_space(8.0);
                                }
                                if shown == 0 {
                                    ui.label(
                                        RichText::new("No matches. Missing a word we should define? Tell us in chat - the dictionary grows from exactly that.")
                                            .size(theme.font_size_small)
                                            .color(theme.text_muted()),
                                    );
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
                                for w in websites.iter() {
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
                                    Frame::none()
                                        .fill(theme.bg_card())
                                        .rounding(egui::Rounding::same(theme.border_radius as u8))
                                        .stroke(Stroke::new(1.0, theme.border()))
                                        .inner_margin(egui::Margin::symmetric(14, 10))
                                        .show(ui, |ui| {
                                            ui.set_width(ui.available_width());
                                            // Load website at the TOP of the card (operator
                                            // 2026-06-07: "move the load website to the top of
                                            // each card"). A click on the card never launches;
                                            // only this explicit button does.
                                            if widgets_button_load(ui, theme) {
                                                ui.ctx().open_url(egui::OpenUrl::new_tab(w.url.to_string()));
                                            }
                                            ui.add_space(6.0);
                                            // All the data, crammed into the card.
                                            ui.label(RichText::new(w.title).size(theme.font_size_body).strong().color(theme.text_primary()));
                                            ui.label(RichText::new(w.tag).size(theme.font_size_small).color(theme.accent()));
                                            ui.label(RichText::new(w.desc).size(theme.font_size_small).color(theme.text_secondary()));
                                            ui.label(RichText::new(w.url).size(theme.font_size_small).color(link_color));
                                        });
                                    ui.add_space(8.0);
                                }
                                if shown == 0 {
                                    ui.label(RichText::new("No matches.").size(theme.font_size_small).color(theme.text_muted()));
                                }
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
