//! The How-to page's rail: the search box (it reads the signs of problems too, because people
//! arrive with a symptom), Doing now (this person's unfinished runs, each with its next step or
//! its timer), and the category tree with counts, planned how-tos counted beside the written ones.
//! A refused file is listed at the foot with its reason, so an edit that broke one is visible.

use egui::{RichText, Sense, Ui};

use super::{w, wf, words, HowToState};
use crate::gui::theme::Theme;
use crate::gui::widgets::icons;
use crate::gui::GuiState;
use crate::how_to::library::Library;
use crate::how_to::model::Category;

/// The painter for a category's `icon` name (data/how_to/categories.json).
pub fn icon_painter(name: &str) -> Option<fn(&egui::Painter, egui::Rect, egui::Color32)> {
    Some(match name {
        "house" => icons::paint_house,
        "leaf" => icons::paint_leaf,
        "droplet" => icons::paint_droplet,
        "heart" => icons::paint_heart,
        "hammer" => icons::paint_hammer,
        "bolt" => icons::paint_bolt,
        "lifebuoy" => icons::paint_lifebuoy,
        "wallet" => icons::paint_wallet,
        "person" => icons::paint_person,
        "server" => icons::paint_server,
        "globe" => icons::paint_globe,
        _ => return None,
    })
}

pub fn draw(ui: &mut Ui, theme: &Theme, state: &mut GuiState, lib: &Library) {
    let resp = ui.add(egui::TextEdit::singleline(&mut state.how_to.search).hint_text(w("search_hint")).desired_width(f32::INFINITY));
    if resp.changed() && !state.how_to.search.trim().is_empty() {
        state.how_to.branch = None;
    }
    ui.add_space(theme.spacing_sm);

    draw_doing_now(ui, theme, state, lib);

    ui.label(RichText::new(w("categories")).color(theme.text_secondary()).size(theme.font_size_small).strong());
    ui.add_space(theme.spacing_xs);
    let (written, planned) = (lib.how_tos.len(), lib.planned.len());
    let all_on = state.how_to.branch.is_none() && state.how_to.search.trim().is_empty();
    if row(ui, theme, 0, None, w("all"), &wf("counts", &[("written", &written.to_string()), ("planned", &planned.to_string())]), all_on, None).clicked {
        state.how_to.branch = None;
        state.how_to.search.clear();
    }
    for c in &lib.categories {
        draw_node(ui, theme, &mut state.how_to, lib, c, 0, theme.nav_dev_visible);
    }

    if !lib.refused.is_empty() || !lib.problems.is_empty() {
        ui.add_space(theme.spacing_md);
        ui.label(RichText::new(w("could_not_show")).color(theme.danger()).size(theme.font_size_small).strong());
        ui.label(RichText::new(w("could_not_show_line")).color(theme.text_muted()).size(theme.font_size_small));
        for p in &lib.problems {
            ui.label(RichText::new(p).color(theme.text_secondary()).size(theme.font_size_small));
        }
        for r in &lib.refused {
            let first = crate::how_to::check::refusals(&r.findings).next().map(|f| format!("{}: {}", f.code.id(), f.message)).unwrap_or_default();
            ui.label(RichText::new(format!("{}: {first}", r.file)).color(theme.text_secondary()).size(theme.font_size_small));
        }
    }
}

fn draw_doing_now(ui: &mut Ui, theme: &Theme, state: &mut GuiState, lib: &Library) {
    let now = crate::persistence::now_ms();
    let open: Vec<(String, String, String)> = state
        .how_to
        .runs
        .iter()
        .filter(|r| r.is_open())
        .filter_map(|r| {
            let l = lib.find(&r.how_to)?;
            // A timer running or due says so; otherwise the next step.
            let timer = r.waits.keys().filter(|s| !r.is_done(s)).filter_map(|s| r.wait_left_ms(s, now)).min();
            let line = match timer {
                Some(0) => w("wait_up").to_string(),
                Some(ms) => wf("wait_left", &[("time", &words::clock(ms))]),
                None => {
                    match crate::how_to::view::position(&l.how_to, r) {
                        None => wf("all_done", &[("total", &crate::how_to::view::visible_steps(&l.how_to, Some(r)).len().to_string())]),
                        Some((n, total)) => {
                            let next = crate::how_to::view::current_step(&l.how_to, r).map(|s| s.text.clone()).unwrap_or_default();
                            format!("{}: {next}", wf("step_of", &[("n", &n.to_string()), ("total", &total.to_string())]))
                        }
                    }
                }
            };
            Some((l.how_to.id.clone(), l.how_to.title.clone(), line))
        })
        .collect();
    if open.is_empty() {
        return;
    }
    ui.label(RichText::new(w("doing_now")).color(theme.text_secondary()).size(theme.font_size_small).strong());
    ui.add_space(theme.spacing_xs);
    for (id, title, line) in open {
        let on = state.how_to.open.as_deref() == Some(id.as_str());
        let resp = ui.selectable_label(on, RichText::new(&title).strong().color(theme.text_primary()));
        ui.label(RichText::new(line).color(theme.text_muted()).size(theme.font_size_small));
        if resp.clicked() {
            state.how_to.open(&id);
        }
        ui.add_space(theme.spacing_xs);
    }
    ui.add_space(theme.spacing_sm);
}

/// What a row reported.
struct RowResp {
    clicked: bool,
    toggled: bool,
}

/// One row of the tree: its fold mark (painted, never a glyph), its icon, its title and count.
#[allow(clippy::too_many_arguments)]
fn row(ui: &mut Ui, theme: &Theme, depth: usize, open: Option<bool>, title: &str, count: &str, selected: bool, icon: Option<&str>) -> RowResp {
    let h = theme.row_height.max(theme.font_size_body + theme.spacing_sm * 2.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), h), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, egui::CornerRadius::same(theme.border_radius as u8), theme.bg_secondary());
    } else if resp.hovered() {
        p.rect_filled(rect, egui::CornerRadius::same(theme.border_radius as u8), theme.bg_tertiary());
    }
    let mark = theme.font_size_small;
    let mut x = rect.left() + theme.spacing_xs + depth as f32 * theme.spacing_md;
    let cy = rect.center().y;
    let mut toggled = false;
    if let Some(o) = open {
        let mr = egui::Rect::from_center_size(egui::pos2(x + mark / 2.0, cy), egui::vec2(mark, mark));
        if o {
            icons::paint_triangle_down(p, mr, theme.text_secondary());
        } else {
            icons::paint_triangle_right(p, mr, theme.text_secondary());
        }
        // A click on the fold mark folds; a click anywhere else opens the branch (and unfolds it).
        if resp.clicked() && resp.interact_pointer_pos().map(|pos| pos.x < x + mark + theme.spacing_xs).unwrap_or(false) {
            toggled = true;
        }
    }
    x += mark + theme.spacing_xs;
    let colour = if selected { theme.text_primary() } else { theme.text_secondary() };
    if let Some(paint) = icon.and_then(icon_painter) {
        let ir = egui::Rect::from_center_size(egui::pos2(x + theme.icon_small / 2.0, cy), egui::vec2(theme.icon_small, theme.icon_small));
        paint(p, ir, colour);
        x += theme.icon_small + theme.spacing_xs;
    }
    let font = egui::FontId::proportional(theme.font_size_body);
    let small = egui::FontId::proportional(theme.font_size_small);
    let count_g = p.layout_no_wrap(count.to_string(), small, theme.text_muted());
    let room = (rect.right() - theme.spacing_xs - count_g.size().x - theme.spacing_sm - x).max(0.0);
    let title_g = p.layout(title.to_string(), font, colour, f32::INFINITY);
    let title_g = if title_g.size().x > room { p.layout_no_wrap(truncate(title, room, theme), egui::FontId::proportional(theme.font_size_body), colour) } else { title_g };
    p.galley(egui::pos2(x, cy - title_g.size().y / 2.0), title_g, colour);
    p.galley(egui::pos2(rect.right() - theme.spacing_xs - count_g.size().x, cy - count_g.size().y / 2.0), count_g, theme.text_muted());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    RowResp { clicked: resp.clicked() && !toggled, toggled }
}

/// A title cut to fit `room` points, ending in an ellipsis of three dots.
fn truncate(title: &str, room: f32, theme: &Theme) -> String {
    let per = theme.font_size_body * 0.55;
    let keep = ((room / per) as usize).saturating_sub(3).max(1);
    if title.chars().count() <= keep {
        return title.to_string();
    }
    format!("{}...", title.chars().take(keep).collect::<String>())
}

fn draw_node(ui: &mut Ui, theme: &Theme, h: &mut HowToState, lib: &Library, c: &Category, depth: usize, dev: bool) {
    if c.audience == crate::how_to::model::Audience::Developer && !dev {
        return;
    }
    let (written, planned) = lib.counts(&c.id);
    let has_children = !c.children.is_empty();
    let open = h.expanded.contains(&c.id);
    let count = if planned > 0 { format!("{written} / {}", written + planned) } else { written.to_string() };
    let selected = h.branch.as_deref() == Some(c.id.as_str()) && h.search.trim().is_empty();
    let r = row(ui, theme, depth, has_children.then_some(open), &c.title, &count, selected, c.icon.as_deref());
    if r.toggled {
        if open {
            h.expanded.remove(&c.id);
        } else {
            h.expanded.insert(c.id.clone());
        }
    }
    if r.clicked {
        h.branch = Some(c.id.clone());
        h.search.clear();
        h.expanded.insert(c.id.clone());
    }
    if open || (r.clicked && has_children) {
        for child in &c.children {
            draw_node(ui, theme, h, lib, child, depth + 1, dev);
        }
    }
}
