//! Library: the in-app home for everything you READ. A top-level tab.
//!
//! Three faces:
//! - DOCUMENTS: the Humanity Accord + companions (data/library/), a collapsible
//!   nested tree on the left, rendered in the right pane via widgets::markdown.
//! - DICTIONARY: every glossary term, searchable and category-filtered.
//! - CURRICULUM: the syllabus (data/curriculum/syllabus.json), which is the
//!   Library's map of ITSELF: every subject a person needs, and honestly how far
//!   each one has got. Showing the gaps is the point, because a library that
//!   only displays what it has cannot tell you what it is missing.
//!
//! The external tools/websites directory that used to live here as a third face
//! moved to the Tools page in v0.1063, so the two pages split by what you DO
//! with them: Library is what you read, Tools is what you go use. The web mirror
//! is `web/pages/library.html`, reading the same `data/library/` manifest.

use egui::{Align, CursorIcon, Frame, Label, Layout, RichText, ScrollArea, Sense, Stroke, TextEdit, Vec2};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets::markdown;

/// Which view the right pane shows.
#[derive(Clone, PartialEq)]
enum Sel {
    /// A document (section, category, entry) rendered as markdown.
    Doc(usize, usize, usize),
    /// The Dictionary: every glossary term, searchable + category-filtered
    /// (v0.989, operator: "assume people aren't going to know all the
    /// words so we should have a way of quickly learning words").
    Dictionary,
    /// The syllabus: every subject a person needs, and how far each one has
    /// got. The Library's own map of itself.
    Curriculum,
}

struct LibState {
    sel: Sel,
    initialized: bool,
    /// Dictionary search text.
    dict_query: String,
    /// Dictionary category filter (glossary category id).
    dict_cat: Option<String>,
    /// "Define words" toggle for the doc pane: on = every word in the open
    /// document is clickable for a definition.
    define_mode: bool,
    /// A word the reader clicked in define mode - drives the popup.
    define_popup: Option<String>,
    /// Document search text. Native searches the in-memory bodies directly:
    /// every document is already loaded, so unlike web there is no index to
    /// fetch and no cost to searching.
    search: String,
    /// Where the reader came from, so following a cross-reference is
    /// reversible. Pushed on every link jump, popped by Back. Without it a
    /// document that sends you somewhere else is a one-way trip, which is the
    /// thing that makes people avoid following links at all.
    back: Vec<Sel>,
    /// A heading slug to scroll to on the next frame, then cleared. Set by the
    /// Contents outline and by opening a search hit. One frame only: holding it
    /// would fight the reader every time they scrolled away.
    scroll_to: Option<String>,
    /// Whether the Contents outline is expanded. Remembered across documents,
    /// because a reader who wants an outline wants it for the next one too.
    toc_open: bool,
    /// Which slice of the curriculum the Curriculum view shows: empty for
    /// everything, else one of written / absent / lethal / locale.
    cur_filter: String,
    /// Active document tag filter (a tag id from `data/library/tags.json`).
    /// None shows everything. Narrows the rail without changing what is open,
    /// so filtering never yanks the document you are reading out from under you.
    tag_filter: Option<String>,
}

/// Open the document with this slug, if the Library has one.
///
/// The same resolution a `/library#<slug>` cross-reference performs, exposed so
/// the snapshot harness can open a document that has siblings. Without it the
/// page snapshot always opens whatever happens to be first, and a footer that
/// only appears on a document with siblings would never be photographed, which
/// is a guard that cannot fail.
pub fn show_doc(state: &GuiState, slug: &str) -> bool {
    for (si, sec) in state.library.iter().enumerate() {
        for (ci, c) in sec.categories.iter().enumerate() {
            if let Some(di) = c.entries.iter().position(|e| e.slug == slug) {
                lib_state(|s| {
                    s.sel = Sel::Doc(si, ci, di);
                    s.initialized = true;
                });
                return true;
            }
        }
    }
    false
}

/// Open the Curriculum view.
///
/// Exists because two callers need to reach it without a click: the
/// `/library#curriculum` deep link, so any document or page can point a reader
/// at the map, and the snapshot harness, which has no cursor.
pub fn show_curriculum() {
    lib_state(|s| {
        s.sel = Sel::Curriculum;
        s.initialized = true;
    });
}

fn lib_state<R>(f: impl FnOnce(&mut LibState) -> R) -> R {
    use std::cell::RefCell;
    thread_local! {
        static S: RefCell<LibState> = RefCell::new(LibState {
            // Replaced on the first frame by the first document; Dictionary is
            // only the placeholder for a library with no docs at all.
            sel: Sel::Dictionary,
            initialized: false,
            dict_query: String::new(),
            dict_cat: None,
            define_mode: false,
            define_popup: None,
            back: Vec::new(),
            search: String::new(),
            scroll_to: None,
            cur_filter: String::new(),
            toc_open: false,
            tag_filter: None,
        });
    }
    S.with(|s| f(&mut s.borrow_mut()))
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(16.0))
        .show(ctx, |ui| {
            ui.label(RichText::new("Library").size(theme.font_size_title).color(theme.text_primary()));
            ui.label(
                RichText::new("The Humanity Accord, the reference it rests on, and a dictionary for every term. Looking for software or help services? Those live on the Tools page.")
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

            // One-time selection default: the first document.
            lib_state(|s| {
                if !s.initialized {
                    s.initialized = true;
                    'find: for (si, sec) in state.library.iter().enumerate() {
                        for (ci, c) in sec.categories.iter().enumerate() {
                            if !c.entries.is_empty() {
                                s.sel = Sel::Doc(si, ci, 0);
                                break 'find;
                            }
                        }
                    }
                }
            });

            // ── Tag filter: one wrapping row of chips per axis ──
            // Full width, above both panes, because the 250px rail cannot hold
            // four groups of chips. Tags cross-cut the categories: a doc sits in
            // exactly one category but carries as many tags as apply, so this is
            // the only way to ask "show me everything safety-critical".
            if !state.library_tags.is_empty() {
                let active = lib_state(|s| s.tag_filter.clone());
                let mut picked: Option<Option<String>> = None;
                for group in state.library_tags.iter() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            RichText::new(group.label.as_str())
                                .size(theme.font_size_small)
                                .color(theme.text_muted()),
                        );
                        for t in group.tags.iter() {
                            let is_on = active.as_deref() == Some(t.id.as_str());
                            if tag_chip(ui, theme, t.label.as_str(), is_on) {
                                // Clicking the active chip clears the filter.
                                picked = Some(if is_on { None } else { Some(t.id.clone()) });
                            }
                        }
                    });
                }
                if active.is_some() {
                    ui.horizontal(|ui| {
                        if crate::gui::widgets::Button::secondary("Clear filter").show(ui, theme) {
                            picked = Some(None);
                        }
                    });
                }
                if let Some(next) = picked {
                    lib_state(|s| s.tag_filter = next);
                }
                ui.add_space(theme.spacing_xs);
                ui.separator();
            }

            let rail_w = 250.0;
            let content_w = (ui.available_width() - rail_w - 24.0).max(320.0);
            let body_h = ui.available_height();

            // Set by a click on an inline link in the doc pane; resolved after
            // the panes are drawn, because resolving needs the whole library and
            // the pane closure only has the open document.
            let mut nav_request: Option<String> = None;

            ui.horizontal_top(|ui| {
                // ── Left rail: document tree + the Dictionary entry ──
                ui.allocate_ui_with_layout(Vec2::new(rail_w, body_h), Layout::top_down(Align::Min), |ui| {
                    // ── Document search ──
                    // The Library had none: the only search box on this page
                    // searched the Dictionary's glossary terms. Browsing a
                    // 17-category rail works at 83 documents and not at 500.
                    // Keyboard: "/" focuses the box from anywhere on the page,
                    // Escape empties it and gives focus back. The same two keys
                    // web uses (web/pages/library-app.js), because a reader who
                    // learns them in one client should not have to learn them
                    // again in the other. Read BEFORE the widget so the "/" that
                    // triggered the focus is not also typed into it.
                    // Only when nothing else already has the keyboard, so "/"
                    // typed into the Dictionary box stays a "/" there.
                    let typing_elsewhere = ui.memory(|m| m.focused()).is_some();
                    let focus_search = !typing_elsewhere
                        && ui.input(|i| {
                            i.events.iter().any(|e| match e {
                                egui::Event::Text(t) => t.as_str() == "/",
                                _ => false,
                            })
                        });
                    let clear_search = ui.input(|i| i.key_pressed(egui::Key::Escape));
                    lib_state(|s| {
                        if clear_search && !s.search.is_empty() {
                            s.search.clear();
                        }
                        let r = ui.add(
                            TextEdit::singleline(&mut s.search)
                                .hint_text("Search all documents  (press /)")
                                .desired_width(rail_w - 8.0),
                        );
                        if focus_search && !r.has_focus() {
                            r.request_focus();
                        }
                    });
                    ui.add_space(theme.spacing_xs);

                    ScrollArea::vertical().id_salt("library_rail").auto_shrink([false, false]).show(ui, |ui| {
                        // Results REPLACE the tree while searching, so the rail
                        // shows one thing at a time rather than two competing
                        // navigations.
                        let q = lib_state(|s| s.search.trim().to_lowercase());
                        if !q.is_empty() {
                            // (section, category, entry, title, score, heading)
                            // The heading is which SECTION of the document the
                            // match sits in, so opening a hit lands the reader on
                            // the passage instead of at the top of a long file.
                            let mut hits: Vec<(usize, usize, usize, &str, i32, Option<markdown::Heading>)> =
                                Vec::new();
                            for (si, sec) in state.library.iter().enumerate() {
                                for (ci, c) in sec.categories.iter().enumerate() {
                                    for (ei, e) in c.entries.iter().enumerate() {
                                        let t = e.title.to_lowercase();
                                        let b = e.body.to_lowercase();
                                        // Title beats body: somebody searching
                                        // "botulism" wants the document about it,
                                        // not one that mentions it in passing.
                                        let score = if t.contains(&q) { 10 } else { 0 }
                                            + if b.contains(&q) { 1 } else { 0 };
                                        if score > 0 {
                                            let head = heading_for_match(&e.body, &q);
                                            hits.push((si, ci, ei, e.title.as_str(), score, head));
                                        }
                                    }
                                }
                            }
                            hits.sort_by(|a, b| b.4.cmp(&a.4));
                            ui.label(
                                RichText::new(format!(
                                    "{} result{}",
                                    hits.len(),
                                    if hits.len() == 1 { "" } else { "s" }
                                ))
                                .size(theme.font_size_small)
                                .color(theme.text_muted()),
                            );
                            ui.add_space(theme.spacing_xs);
                            let mut picked: Option<(Sel, Option<String>)> = None;
                            for (si, ci, ei, title, _, head) in hits.into_iter().take(40) {
                                let label = match &head {
                                    Some(h) if h.text != title => {
                                        format!("{title}  \u{203a} {}", h.text)
                                    }
                                    _ => title.to_string(),
                                };
                                if ui
                                    .selectable_label(
                                        false,
                                        RichText::new(label)
                                            .size(theme.font_size_small)
                                            .color(theme.text_primary()),
                                    )
                                    .clicked()
                                {
                                    picked = Some((Sel::Doc(si, ci, ei), head.map(|h| h.slug)));
                                }
                            }
                            if let Some((next, anchor)) = picked {
                                lib_state(|s| {
                                    let prev = s.sel.clone();
                                    if prev != next {
                                        s.back.push(prev);
                                        s.sel = next;
                                    }
                                    s.scroll_to = anchor;
                                });
                            }
                            return;
                        }

                        lib_state(|s| {
                            for (si, section) in state.library.iter().enumerate() {
                                // Never render a section header with nothing under
                                // it, which an active tag filter can easily cause.
                                let has_docs = section.categories.iter().any(|c| {
                                    c.entries.iter().any(|e| {
                                        s.tag_filter.as_ref().map_or(true, |t| e.tags.iter().any(|x| x == t))
                                    })
                                });
                                if !has_docs {
                                    continue;
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
                                            .filter(|(_, e)| {
                                                s.tag_filter
                                                    .as_ref()
                                                    .map_or(true, |t| e.tags.iter().any(|x| x == t))
                                            })
                                            .map(|(ei, e)| (ei, e.title.as_str()))
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
                                                    // egui fills a selected label with the
                                                    // accent, so accent TEXT on it is invisible.
                                                    // Flip to the panel colour, same as tag_chip.
                                                    let color = if is_sel { theme.bg_primary() } else { theme.text_primary() };
                                                    if ui.selectable_label(is_sel, RichText::new(title).color(color)).clicked() {
                                                        s.sel = Sel::Doc(si, ci, ei);
                                                    }
                                                }
                                            });
                                    }
                                });
                            }

                            // A filter that matches nothing would otherwise leave
                            // a rail containing only "Dictionary", which reads as
                            // a broken page rather than an empty result.
                            if let Some(tag) = s.tag_filter.clone() {
                                let none = !state.library.iter().any(|sec| {
                                    sec.categories.iter().any(|c| {
                                        c.entries.iter().any(|e| e.tags.iter().any(|x| *x == tag))
                                    })
                                });
                                if none {
                                    ui.label(
                                        RichText::new("No documents carry that tag yet.")
                                            .size(theme.font_size_small)
                                            .color(theme.text_muted()),
                                    );
                                }
                            }

                            ui.add_space(theme.spacing_sm);
                            let dict_active = s.sel == Sel::Dictionary;
                            let dcolor = if dict_active { theme.bg_primary() } else { theme.text_primary() };
                            if ui
                                .selectable_label(dict_active, RichText::new("Dictionary").strong().color(dcolor))
                                .clicked()
                            {
                                s.sel = Sel::Dictionary;
                            }
                            // The syllabus, in the app. It is what makes "how
                            // complete is this Library" answerable, and until
                            // now the only way to read it was a command line,
                            // which the GUI-first rule in CLAUDE.md says is not
                            // good enough for anything a user might want to see.
                            if !state.curriculum.is_empty() {
                                let cur_active = s.sel == Sel::Curriculum;
                                let ccolor =
                                    if cur_active { theme.bg_primary() } else { theme.text_primary() };
                                if ui
                                    .selectable_label(
                                        cur_active,
                                        RichText::new("What there is to learn").strong().color(ccolor),
                                    )
                                    .clicked()
                                {
                                    s.sel = Sel::Curriculum;
                                }
                            }
                        });
                    });
                });

                ui.separator();

                // ── Right pane ──
                ui.allocate_ui_with_layout(Vec2::new(content_w, body_h), Layout::top_down(Align::Min), |ui| {
                    lib_state(|s| match s.sel.clone() {
                        Sel::Doc(si, ci, ei) => {
                            let entry = state
                                .library
                                .get(si)
                                .and_then(|sec| sec.categories.get(ci))
                                .and_then(|c| c.entries.get(ei));
                            let body = entry.map(|e| e.body.as_str());

                            // The open document's own tags, clickable so a reader
                            // who likes this doc can find its siblings in one tap.
                            if let Some(e) = entry {
                                if !e.tags.is_empty() && !state.library_tags.is_empty() {
                                    let mut jump: Option<String> = None;
                                    ui.horizontal_wrapped(|ui| {
                                        for id in e.tags.iter() {
                                            // Show the human label, fall back to the
                                            // raw id so an unregistered tag is visible
                                            // rather than silently dropped.
                                            let label = state
                                                .library_tags
                                                .iter()
                                                .flat_map(|g| g.tags.iter())
                                                .find(|t| t.id == *id)
                                                .map(|t| t.label.as_str())
                                                .unwrap_or(id.as_str());
                                            let is_on = s.tag_filter.as_deref() == Some(id.as_str());
                                            if tag_chip(ui, theme, label, is_on) {
                                                jump = Some(id.clone());
                                            }
                                        }
                                    });
                                    if let Some(id) = jump {
                                        s.tag_filter =
                                            if s.tag_filter.as_deref() == Some(id.as_str()) { None } else { Some(id) };
                                    }
                                    ui.add_space(theme.spacing_xs);
                                }
                            }
                            // What this document is FOR. A guide sitting on a
                            // shelf does not tell you which real-life question
                            // it answers or what you should already know; the
                            // syllabus does, and this is where the two meet.
                            if let Some(e) = entry {
                                let taught = state.curriculum.topics_for_slug(&e.slug);
                                if !taught.is_empty() {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(
                                            RichText::new("Teaches")
                                                .size(theme.font_size_small)
                                                .color(theme.text_muted()),
                                        );
                                        for t in taught.iter() {
                                            let subject = state
                                                .curriculum
                                                .subjects
                                                .iter()
                                                .find(|x| x.id == t.subject)
                                                .map(|x| x.title.as_str())
                                                .unwrap_or(t.subject.as_str());
                                            ui.label(
                                                RichText::new(format!("{} ({subject})", t.title))
                                                    .size(theme.font_size_small)
                                                    .color(theme.text_secondary()),
                                            );
                                            if t.hazard == "lethal" {
                                                ui.label(
                                                    RichText::new("\u{26a0} can kill you if taught wrong")
                                                        .size(theme.font_size_small)
                                                        .color(theme.danger()),
                                                );
                                            }
                                        }
                                    });
                                    ui.add_space(theme.spacing_xs);
                                }
                            }
                            // Define-words toggle (v0.989): on = click any word
                            // in the document for its definition; dictionary
                            // hits show underlined. Plain fast rendering when off.
                            ui.horizontal(|ui| {
                                // Back first, so following a cross-reference is
                                // visibly reversible. Only shown when there is
                                // somewhere to go, rather than a dead control.
                                if !s.back.is_empty() {
                                    let n = s.back.len();
                                    let label = if n == 1 { "Back".to_string() } else { format!("Back ({n})") };
                                    if crate::gui::widgets::Button::secondary(&label).show(ui, theme) {
                                        if let Some(prev) = s.back.pop() {
                                            s.sel = prev;
                                        }
                                    }
                                }
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

                            // ── Contents ──
                            // A document the length of the Constitution or
                            // SELF-HOSTING had exactly one way in: scroll. The
                            // outline is collapsed by default so it costs a
                            // reader who does not want it a single line, and the
                            // open/closed choice carries to the next document.
                            if let Some(body) = body {
                                let heads = markdown::headings(body);
                                // One or two headings is not an outline, it is
                                // the title. Below three this is noise.
                                if heads.len() >= 3 {
                                    let mut jump: Option<String> = None;
                                    let label = format!("Contents ({} sections)", heads.len());
                                    let resp = egui::CollapsingHeader::new(
                                        RichText::new(label)
                                            .size(theme.font_size_small)
                                            .color(theme.text_muted()),
                                    )
                                    .id_salt("library_toc")
                                    .open(Some(s.toc_open))
                                    .show(ui, |ui| {
                                        for h in heads.iter() {
                                            // Indent by level so the shape of the
                                            // document is visible, not just its
                                            // section names.
                                            ui.horizontal(|ui| {
                                                ui.add_space(theme.spacing_sm * (h.level.saturating_sub(1)) as f32);
                                                let color = if h.level == 1 {
                                                    theme.text_primary()
                                                } else {
                                                    theme.text_secondary()
                                                };
                                                if ui
                                                    .add(
                                                        Label::new(
                                                            RichText::new(&h.text)
                                                                .size(theme.font_size_small)
                                                                .color(color),
                                                        )
                                                        .sense(Sense::click()),
                                                    )
                                                    .on_hover_cursor(CursorIcon::PointingHand)
                                                    .clicked()
                                                {
                                                    jump = Some(h.slug.clone());
                                                }
                                            });
                                        }
                                    });
                                    if resp.header_response.clicked() {
                                        s.toc_open = !s.toc_open;
                                    }
                                    if jump.is_some() {
                                        s.scroll_to = jump;
                                    }
                                    ui.add_space(theme.spacing_xs);
                                }
                            }

                            // One frame only. Holding the target would re-scroll
                            // every frame and the reader could never scroll away.
                            let scroll_to = s.scroll_to.take();
                            ScrollArea::vertical().id_salt("library_doc").auto_shrink([false, false]).show(ui, |ui| {
                                if let Some(body) = body {
                                    if s.define_mode {
                                        let mut clicked: Option<String> = None;
                                        markdown::render_markdown_defining(ui, theme, body, &mut clicked);
                                        if clicked.is_some() {
                                            s.define_popup = clicked;
                                        }
                                    } else {
                                        let mut link: Option<String> = None;
                                        markdown::render_markdown_linked_scrolled(
                                            ui,
                                            theme,
                                            body,
                                            &mut link,
                                            scroll_to.as_deref(),
                                        );
                                        if let Some(target) = link {
                                            nav_request = Some(target);
                                        }
                                    }

                                    // ── Where to go next ──
                                    // A document used to end at its last full
                                    // stop and offer nothing. The categories in
                                    // Learn are ORDERED ladders, so the next rung
                                    // is a real answer and not a guess: finish
                                    // Your First Tomato and the thing to read is
                                    // Starting Seeds, not whatever shares a tag.
                                    if let Sel::Doc(si, ci, di) = s.sel.clone() {
                                        let entries = state
                                            .library
                                            .get(si)
                                            .and_then(|sec| sec.categories.get(ci))
                                            .map(|c| &c.entries);
                                        if let Some(entries) = entries {
                                            if entries.len() > 1 {
                                                ui.add_space(theme.spacing_md);
                                                ui.separator();
                                                let mut go: Option<Sel> = None;
                                                ui.horizontal_wrapped(|ui| {
                                                    if di > 0 {
                                                        let t = &entries[di - 1].title;
                                                        if crate::gui::widgets::Button::secondary(
                                                            &format!("\u{2039} {t}"),
                                                        )
                                                        .show(ui, theme)
                                                        {
                                                            go = Some(Sel::Doc(si, ci, di - 1));
                                                        }
                                                    }
                                                    if di + 1 < entries.len() {
                                                        let t = &entries[di + 1].title;
                                                        if crate::gui::widgets::Button::secondary(
                                                            &format!("Next: {t} \u{203a}"),
                                                        )
                                                        .show(ui, theme)
                                                        {
                                                            go = Some(Sel::Doc(si, ci, di + 1));
                                                        }
                                                    } else {
                                                        ui.label(
                                                            RichText::new(
                                                                "That is the last one on this shelf.",
                                                            )
                                                            .size(theme.font_size_small)
                                                            .color(theme.text_muted()),
                                                        );
                                                    }
                                                });
                                                if let Some(next) = go {
                                                    let prev = s.sel.clone();
                                                    s.back.push(prev);
                                                    s.sel = next;
                                                }
                                            }
                                        }
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
                        Sel::Curriculum => {
                            // The syllabus as a reader sees it: what there is to
                            // learn, and honestly how far each subject has got.
                            // Showing the gaps is the point. A Library that only
                            // displays what it HAS cannot tell you what it is
                            // missing, and "how complete is this" was the
                            // question that started this whole arc.
                            let cur = &state.curriculum;
                            let total = cur.topics.len();
                            let written = cur.topics.iter().filter(|t| t.status != "absent").count();
                            let graded = cur.topics.iter().filter(|t| t.status == "verified").count();
                            ui.label(
                                RichText::new("What there is to learn")
                                    .size(theme.font_size_heading)
                                    .strong()
                                    .color(theme.text_primary()),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "{total} topics across {} subjects. {written} have something written, \
                                     {graded} have been checked by a second pass. The rest are named so the \
                                     gap is countable rather than invisible.",
                                    cur.subjects.len()
                                ))
                                .size(theme.font_size_small)
                                .color(theme.text_muted()),
                            );
                            ui.add_space(theme.spacing_xs);

                            let mut jump: Option<String> = None;
                            ui.horizontal_wrapped(|ui| {
                                for (label, key) in [
                                    ("Everything", ""),
                                    ("Written", "written"),
                                    ("Not yet written", "absent"),
                                    ("Can kill you if taught wrong", "lethal"),
                                    ("Depends where you are", "locale"),
                                ] {
                                    let on = s.cur_filter == key;
                                    if tag_chip(ui, theme, label, on) {
                                        s.cur_filter = if on { String::new() } else { key.to_string() };
                                    }
                                }
                            });
                            ui.separator();

                            let keep = |t: &crate::gui::CurriculumTopic| match s.cur_filter.as_str() {
                                "written" => t.status != "absent",
                                "absent" => t.status == "absent",
                                "lethal" => t.hazard == "lethal",
                                "locale" => t.locale_dependent,
                                _ => true,
                            };

                            ScrollArea::vertical()
                                .id_salt("library_curriculum")
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    for subj in cur.subjects.iter() {
                                        let topics: Vec<_> = cur
                                            .topics
                                            .iter()
                                            .filter(|t| t.subject == subj.id && keep(t))
                                            .collect();
                                        if topics.is_empty() {
                                            continue;
                                        }
                                        let done = topics.iter().filter(|t| t.status != "absent").count();
                                        egui::CollapsingHeader::new(
                                            RichText::new(format!(
                                                "{}   {}/{}",
                                                subj.title,
                                                done,
                                                topics.len()
                                            ))
                                            .size(theme.font_size_body)
                                            .strong()
                                            .color(theme.text_primary()),
                                        )
                                        .id_salt(("libsubj", subj.id.as_str()))
                                        .default_open(done > 0)
                                        .show(ui, |ui| {
                                            for t in topics {
                                                ui.horizontal_top(|ui| {
                                                    ui.add_space(theme.spacing_sm);
                                                    // Status first, because it is
                                                    // what the reader is asking.
                                                    let (mark, mc) = match t.status.as_str() {
                                                        "verified" => ("\u{2713}", theme.success()),
                                                        "sourced" => ("\u{00b7}", theme.accent()),
                                                        "stub" => ("\u{00b7}", theme.warning()),
                                                        _ => (" ", theme.text_muted()),
                                                    };
                                                    ui.label(RichText::new(mark).size(theme.font_size_body).color(mc));
                                                    ui.vertical(|ui| {
                                                        ui.set_max_width(ui.available_width());
                                                        let tcolor = if t.status == "absent" {
                                                            theme.text_muted()
                                                        } else {
                                                            theme.text_primary()
                                                        };
                                                        ui.horizontal_wrapped(|ui| {
                                                            ui.label(
                                                                RichText::new(&t.title)
                                                                    .size(theme.font_size_small)
                                                                    .strong()
                                                                    .color(tcolor),
                                                            );
                                                            if t.hazard == "lethal" {
                                                                ui.label(
                                                                    RichText::new("\u{26a0} can kill you if taught wrong")
                                                                        .size(theme.font_size_small)
                                                                        .color(theme.danger()),
                                                                );
                                                            } else if t.hazard == "serious" {
                                                                ui.label(
                                                                    RichText::new("\u{26a0} serious")
                                                                        .size(theme.font_size_small)
                                                                        .color(theme.warning()),
                                                                );
                                                            }
                                                            if t.locale_dependent {
                                                                ui.label(
                                                                    RichText::new("depends where you are")
                                                                        .size(theme.font_size_small)
                                                                        .color(theme.text_muted()),
                                                                );
                                                            }
                                                        });
                                                        if !t.summary.is_empty() {
                                                            ui.label(
                                                                RichText::new(&t.summary)
                                                                    .size(theme.font_size_small)
                                                                    .color(theme.text_secondary()),
                                                            );
                                                        }
                                                        // The link that makes this
                                                        // a map rather than a list.
                                                        for slug in t.reading.iter() {
                                                            if ui
                                                                .add(
                                                                    Label::new(
                                                                        RichText::new(format!("Read: {slug}"))
                                                                            .size(theme.font_size_small)
                                                                            .underline()
                                                                            .color(theme.accent()),
                                                                    )
                                                                    .sense(Sense::click()),
                                                                )
                                                                .on_hover_cursor(CursorIcon::PointingHand)
                                                                .clicked()
                                                            {
                                                                jump = Some(format!("/library#{slug}"));
                                                            }
                                                        }
                                                    });
                                                });
                                                ui.add_space(theme.spacing_xs);
                                            }
                                        });
                                    }
                                });
                            if jump.is_some() {
                                nav_request = jump;
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
                    });
                });
            });

            // ── Follow a cross-reference ──
            // `/library#some-slug` is the Library's own deep-link grammar, the
            // same one the web client uses, so one link form works in both. An
            // external http link is left alone: opening a browser from here is a
            // separate decision and is not what these links are for.
            if let Some(target) = nav_request {
                if let Some(frag) = target.strip_prefix("/library#") {
                    // `/library#<doc>` opens a document; `/library#<doc>/<heading>`
                    // opens it at a section. The second form is what lets one
                    // document link into the middle of another, rather than
                    // dropping the reader at the top and leaving them to hunt.
                    let (slug, anchor) = match frag.split_once('/') {
                        Some((d, h)) => (d, Some(h.to_string())),
                        None => (frag, None),
                    };
                    // Two reserved fragments that are views rather than
                    // documents, so a document can send a reader to the map or
                    // to the words without either becoming a fake .md file.
                    if slug == "curriculum" {
                        lib_state(|s| {
                            let prev = s.sel.clone();
                            if prev != Sel::Curriculum {
                                s.back.push(prev);
                                s.sel = Sel::Curriculum;
                            }
                        });
                        return;
                    }
                    if slug == "dictionary" {
                        lib_state(|s| {
                            let prev = s.sel.clone();
                            if prev != Sel::Dictionary {
                                s.back.push(prev);
                                s.sel = Sel::Dictionary;
                            }
                        });
                        return;
                    }
                    let found = state.library.iter().enumerate().find_map(|(si, sec)| {
                        sec.categories.iter().enumerate().find_map(|(ci, c)| {
                            c.entries.iter().position(|e| e.slug == slug
                                )
                                .map(|ei| Sel::Doc(si, ci, ei))
                        })
                    });
                    if let Some(next) = found {
                        lib_state(|s| {
                            let prev = s.sel.clone();
                            if prev != next {
                                s.back.push(prev);
                                s.sel = next;
                            }
                            if anchor.is_some() {
                                s.scroll_to = anchor;
                            }
                        });
                    }
                }
            }
        });
}

/// A clickable tag-filter chip. Returns true when clicked this frame.
/// Which section of `body` the first match for `q` (already lowercased) falls
/// in: the last heading before the matching line.
///
/// The heading list comes from `markdown::headings`, the same call the Contents
/// outline and the renderer's anchors use, so all three agree about which
/// headings exist and what each one's slug is. Deriving it here a second time
/// would be a slug that drifts the moment a document repeats a heading name.
///
/// Matching walks LINES rather than a byte offset into `body.to_lowercase()`,
/// because lowercasing can change a string's byte length and the offset would
/// then point a few bytes off.
fn heading_for_match(body: &str, q: &str) -> Option<markdown::Heading> {
    let heads = markdown::headings(body);
    if heads.is_empty() {
        return None;
    }
    for (n, line) in body.lines().enumerate() {
        let t = line.trim();
        if t.to_lowercase().contains(q) {
            return heads.iter().rev().find(|h| h.line < n).cloned();
        }
    }
    None
}

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
