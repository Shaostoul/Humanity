//! The how-to pane, in the order that is its safety design (docs/design/how-to.md, "Layout"):
//! the title, its line and who it is for, then time, difficulty, hazard, review state and who
//! wrote it; the Short or Full switch; the Stop and call someone if box; What you need; Start; the
//! steps; It worked if and the outcome; If something goes wrong; Learn more; Your history; and the
//! Sources with the date each was read.

use egui::{RichText, Ui};

use super::{difficulty_word, hazard_badge, item_name, review_words, steps, time_words, w, wf};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;
use crate::how_to::library::{Library, Loaded};
use crate::how_to::model::{Need, Reality, WrittenBy};
use crate::how_to::runs::Outcome;
use crate::how_to::view::{self as rules, View};

fn heading(ui: &mut Ui, theme: &Theme, text: &str) {
    ui.add_space(theme.spacing_md);
    ui.label(RichText::new(text).strong().size(theme.font_size_body + 2.0).color(theme.text_primary()));
    ui.add_space(theme.spacing_xs);
}

fn small(ui: &mut Ui, theme: &Theme, text: &str, colour: egui::Color32) {
    ui.add(egui::Label::new(RichText::new(text).color(colour).size(theme.font_size_small)).wrap());
}

pub fn draw(ui: &mut Ui, theme: &Theme, state: &mut GuiState, lib: &Library) {
    let Some(id) = state.how_to.open.clone() else {
        ui.label(RichText::new(w("choose_one")).color(theme.text_muted()));
        return;
    };
    if let Some(p) = lib.find_planned(&id) {
        ui.label(RichText::new(&p.title).strong().size(theme.font_size_heading).color(theme.text_muted()));
        small(ui, theme, w("not_written"), theme.text_muted());
        ui.add_space(theme.spacing_sm);
        small(ui, theme, w("planned_line"), theme.text_secondary());
        ui.label(RichText::new(&p.summary).color(theme.text_secondary()));
        return;
    }
    let Some(l) = lib.find(&id) else {
        ui.label(RichText::new(w("choose_one")).color(theme.text_muted()));
        return;
    };
    let h = &l.how_to;

    // ── What it is, and how dangerous ──
    ui.label(RichText::new(&h.title).strong().size(theme.font_size_heading).color(theme.text_primary()));
    ui.add(egui::Label::new(RichText::new(&h.summary).color(theme.text_secondary())).wrap());
    small(ui, theme, &format!("{}: {}", w("for"), h.for_whom), theme.text_muted());
    ui.add_space(theme.spacing_xs);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(format!("{} · {}", time_words(&h.time), difficulty_word(h.difficulty))).color(theme.text_secondary()).size(theme.font_size_small));
        hazard_badge(ui, theme, h.hazard);
    });
    ui.horizontal_wrapped(|ui| {
        let review = review_words(l);
        let colour = if l.review_current { theme.text_muted() } else { theme.warning() };
        ui.label(RichText::new(review).color(colour).size(theme.font_size_small));
        let who = match h.written_by {
            WrittenBy::Ai => w("written_ai"),
            WrittenBy::Person => w("written_person"),
        };
        ui.label(RichText::new(format!("· {who}")).color(theme.text_muted()).size(theme.font_size_small));
        if h.realities.contains(&Reality::Sim) {
            ui.label(RichText::new(format!("· {}", w("also_in_game"))).color(theme.text_muted()).size(theme.font_size_small));
        }
    });

    // ── Short or Full ──
    ui.add_space(theme.spacing_sm);
    ui.horizontal(|ui| {
        for (v, key) in [(View::Short, "view_short"), (View::Full, "view_full")] {
            if widgets::Button::tab(w(key), state.settings.how_to_view == v).tooltip(w("view_tip")).show(ui, theme) && state.settings.how_to_view != v {
                state.settings.how_to_view = v;
                state.settings_dirty = true;
            }
        }
    });
    let v = state.settings.how_to_view;

    // ── Stop and call someone if: first, before the steps ──
    if !h.stop_and_call.is_empty() {
        ui.add_space(theme.spacing_sm);
        egui::Frame::NONE
            .stroke(egui::Stroke::new(2.0, theme.danger()))
            .fill(theme.bg_secondary())
            .corner_radius(egui::CornerRadius::same(theme.border_radius as u8))
            .inner_margin(egui::Margin::same(theme.spacing_sm as i8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let size = theme.font_size_body + 2.0;
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
                    widgets::icons::paint_warning_filled(ui.painter(), rect, theme.danger(), theme.bg_secondary());
                    ui.label(RichText::new(w("stop_title")).strong().color(theme.danger()));
                });
                for c in &h.stop_and_call {
                    ui.add_space(theme.spacing_xs);
                    ui.add(egui::Label::new(RichText::new(&c.when).strong().color(theme.text_primary())).wrap());
                    ui.add(egui::Label::new(RichText::new(&c.then).color(theme.text_primary())).wrap());
                    if let Some(who) = &c.who {
                        match who.strip_prefix("role:") {
                            Some(role) => {
                                let general = lib.role_names.get(role).cloned().unwrap_or_else(|| role.to_string());
                                small(ui, theme, &general, theme.text_secondary());
                                if let Some((place, (name, url, note))) = lib.locale.as_ref().and_then(|loc| loc.authorities.get(role).map(|a| (loc.name.clone(), a.clone()))) {
                                    small(ui, theme, &wf("where_you_are", &[("place", &place), ("who", &name)]), theme.text_secondary());
                                    if !note.is_empty() {
                                        small(ui, theme, &note, theme.text_muted());
                                    }
                                    if !url.is_empty() && ui.link(RichText::new(&url).size(theme.font_size_small)).clicked() {
                                        ui.ctx().open_url(egui::OpenUrl::new_tab(&url));
                                    }
                                }
                            }
                            None => small(ui, theme, who, theme.text_secondary()),
                        }
                    }
                }
            });
    }

    // ── What you need ──
    if !h.tools.is_empty() || !h.materials.is_empty() {
        heading(ui, theme, w("need_title"));
        for (key, list) in [("tools", &h.tools), ("materials", &h.materials)] {
            if list.is_empty() {
                continue;
            }
            small(ui, theme, w(key), theme.text_secondary());
            for n in list {
                need_row(ui, theme, lib, n);
            }
        }
    }

    // ── Start, and where the run is ──
    let mut run = state.how_to.open_run(&id).cloned();
    ui.add_space(theme.spacing_md);
    if let Some(r) = run.as_ref() {
        let line = match rules::position(h, r) {
            Some((n, total)) => wf("step_of", &[("n", &n.to_string()), ("total", &total.to_string())]),
            None => wf("all_done", &[("total", &rules::visible_steps(h, Some(r)).len().to_string())]),
        };
        ui.label(RichText::new(line).strong().color(theme.accent()));
        if r.hash != l.hash {
            small(ui, theme, w("changed_since_start"), theme.warning());
        }
    } else {
        let ever = state.how_to.runs.iter().any(|r| r.how_to == id);
        let label = if ever { w("start_again") } else { w("start") };
        if widgets::Button::primary(label).tooltip(w("start_tip")).show(ui, theme) {
            let started = crate::how_to::runs::Run::start(h, &l.hash, crate::persistence::now_ms());
            state.how_to.save_run(started.clone());
            run = Some(started);
        }
    }

    // ── The steps ──
    heading(ui, theme, w("steps_title"));
    let after = steps::draw(ui, theme, state, lib, l, &mut run, v);
    if after.changed {
        if let Some(r) = run.clone() {
            state.how_to.save_run(r);
        }
    }

    // ── It worked if, and the end ──
    if !h.worked_if.is_empty() {
        heading(ui, theme, w("worked_if"));
        for line in &h.worked_if {
            small(ui, theme, &format!("· {line}"), theme.text_secondary());
        }
    }
    if let Some(r) = run.as_ref().filter(|r| r.is_open()) {
        ui.add_space(theme.spacing_sm);
        let mut end = None;
        ui.horizontal_wrapped(|ui| {
            if widgets::Button::success(w("worked")).show(ui, theme) {
                end = Some(Outcome::Worked);
            }
            if widgets::Button::secondary(w("did_not_work")).show(ui, theme) {
                end = Some(Outcome::DidNotWork);
            }
            if widgets::Button::ghost(w("stop_run")).show(ui, theme) {
                end = Some(Outcome::Stopped);
            }
        });
        if let Some(o) = end {
            let mut r = r.clone();
            r.finish(o, crate::persistence::now_ms());
            state.how_to.save_run(r);
        }
    }

    // ── If something goes wrong ──
    if !h.problems.is_empty() {
        heading(ui, theme, w("problems_title"));
        let pointed: Vec<String> = run.as_ref().map(|r| r.steps.values().filter_map(|s| s.went_to.clone()).collect()).unwrap_or_default();
        for p in &h.problems {
            let lit = pointed.contains(&p.id);
            egui::Frame::NONE
                .stroke(egui::Stroke::new(if lit { 2.0 } else { 1.0 }, if lit { theme.warning() } else { theme.border() }))
                .corner_radius(egui::CornerRadius::same(theme.border_radius as u8))
                .inner_margin(egui::Margin::same(theme.spacing_sm as i8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.add(egui::Label::new(RichText::new(&p.sign).strong().color(theme.text_primary())).wrap());
                    if let Some(why) = &p.why {
                        small(ui, theme, &format!("{} {why}", w("problem_why")), theme.text_secondary());
                    }
                    small(ui, theme, &format!("{} {}", w("problem_fix"), p.fix), theme.text_primary());
                    if p.stop {
                        small(ui, theme, w("get_help"), theme.danger());
                    }
                });
            ui.add_space(theme.spacing_xs);
        }
    }

    // ── Learn more ──
    if !h.reading.is_empty() || !h.topics.is_empty() || !h.skills.is_empty() {
        heading(ui, theme, w("learn_more"));
        let mut open_doc = None;
        for slug in &h.reading {
            let title = lib.reading_titles.get(slug).cloned().unwrap_or_else(|| slug.clone());
            ui.horizontal(|ui| {
                ui.label(RichText::new(&title).color(theme.text_secondary()));
                if widgets::compact_button(ui, theme, w("read_library"), widgets::ButtonVariant::Secondary) {
                    open_doc = Some(slug.clone());
                }
            });
        }
        for t in &h.topics {
            let title = lib.topic_titles.get(t).cloned().unwrap_or_else(|| t.clone());
            small(ui, theme, &wf("curriculum_topic", &[("title", &title)]), theme.text_secondary());
        }
        for s in &h.skills {
            let name = lib.skill_names.get(s).cloned().unwrap_or_else(|| s.clone());
            small(ui, theme, &wf("skill", &[("name", &name)]), theme.text_secondary());
        }
        if let Some(slug) = open_doc {
            if crate::gui::pages::library::show_doc(state, &slug) {
                state.push_nav_to(crate::gui::GuiPage::Library);
            }
        }
    }

    // ── Your history ──
    heading(ui, theme, w("history_title"));
    let past: Vec<&crate::how_to::runs::Run> = state.how_to.runs.iter().filter(|r| r.how_to == id).collect();
    if past.is_empty() {
        small(ui, theme, w("history_none"), theme.text_muted());
    }
    for r in past {
        let day = r.started.get(..10).unwrap_or(&r.started).to_string();
        let line = match r.outcome {
            None => wf("history_open", &[("date", &day)]),
            Some(o) => {
                let key = match o {
                    Outcome::Worked => "outcome_worked",
                    Outcome::DidNotWork => "outcome_did_not_work",
                    Outcome::Stopped => "outcome_stopped",
                };
                wf("ended", &[("date", &day), ("outcome", w(key))])
            }
        };
        let values: Vec<String> = h.steps.iter().filter_map(|s| r.steps.get(&s.id).and_then(|st| st.value.as_ref()).map(|v| format!("{}: {v}", s.record.as_ref().map(|rc| rc.label.as_str()).unwrap_or(&s.id)))).collect();
        let line = if values.is_empty() { line } else { format!("{line} · {}", values.join(" · ")) };
        small(ui, theme, &line, theme.text_secondary());
    }

    // ── Sources ──
    heading(ui, theme, w("sources_title"));
    for s in &h.sources {
        ui.add(egui::Label::new(RichText::new(&s.title).color(theme.text_primary())).wrap());
        let dated = if s.dated == "undated" { w("undated").to_string() } else { s.dated.clone() };
        let by = s.source.as_ref().map(|id| format!("{id} · ")).unwrap_or_default();
        small(ui, theme, &format!("{by}{}", wf("source_dates", &[("dated", &dated), ("read", &s.read)])), theme.text_muted());
        if let Some(url) = &s.at {
            if ui.link(RichText::new(url).size(theme.font_size_small)).clicked() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(url));
            }
        }
        if let Some(path) = &s.in_project {
            small(ui, theme, path, theme.text_muted());
        }
        if let Some(q) = &s.quote {
            ui.add(egui::Label::new(RichText::new(format!("\"{q}\"")).italics().color(theme.text_secondary()).size(theme.font_size_small)).wrap());
        }
        if v == View::Full {
            if let Some(n) = &s.note {
                small(ui, theme, n, theme.text_muted());
            }
        }
        ui.add_space(theme.spacing_xs);
    }
    small(ui, theme, &review_words(l), theme.text_muted());

    if let Some((page, section)) = after.go {
        super::go(state, &page, section.as_deref());
    }
    if let Some(target) = after.open {
        state.how_to.open(&target);
    }
}

fn need_row(ui: &mut Ui, theme: &Theme, lib: &Library, n: &Need) {
    let name = n.item.as_deref().map(|i| item_name(lib, i)).or_else(|| n.proposed_item.clone()).unwrap_or_default();
    let mut line = name;
    if let Some(q) = &n.qty {
        let amount = match (q.value, q.min, q.max) {
            (Some(v), _, _) => v.to_string(),
            (None, Some(a), Some(b)) => wf("qty_range", &[("a", &a.to_string()), ("b", &b.to_string())]),
            (None, Some(a), None) => a.to_string(),
            (None, None, Some(b)) => b.to_string(),
            _ => String::new(),
        };
        if !amount.is_empty() {
            let unit = if q.unit == crate::how_to::model::Unit::Each { String::new() } else { format!(" {}", q.unit.id()) };
            line = format!("{amount}{unit} {line}");
        }
    }
    if n.optional {
        line = format!("{line} ({})", w("only_sometimes"));
    }
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(format!("· {line}")).color(theme.text_primary()));
    });
    if let Some(note) = &n.note {
        ui.indent(("need_note", line.as_str()), |ui| small(ui, theme, note, theme.text_muted()));
    }
}
