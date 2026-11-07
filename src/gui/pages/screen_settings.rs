//! A placed in-world screen's settings panel, opened by the cog on its picture in the build editor
//! (rung 2 of "Aim assist, any size, and each screen's own settings",
//! docs/design/in-world-screens.md, 2026-11-06). The operator: "a cog like at the top right of the
//! screen for editing that screens settings. So I could say ... this screen always displays this
//! HumanityOS page."
//!
//! Rung 2 offers the size (presets by diagonal and aspect from data/gui/screens.json, or a width
//! and height) and what the screen shows: always a HumanityOS page, or always a camera post. Both
//! write the placed screen's own fields in the home's data (`MachineInstance::screen_source`,
//! `screen_size_m`) and ask the engine to follow (`construction_machines_dirty`); the editor's save
//! writes the home file. A screen showing anything else (a video, the web, a live stream) says so
//! and keeps it until one of the two is chosen; those choices are rung 3 (the design's table).

use crate::gui::screen_surface::ScreenSource;
use crate::gui::theme::Theme;
use crate::gui::widgets::Hint;
use crate::gui::{GuiPage, GuiState};
use egui::RichText;
use humanity_core::i18n::{t, t_dyn, tf};
use humanity_ship::machines::{display_size_of, MachineHome};

/// What a screen shows, as the panel offers it.
#[derive(Debug, Clone, PartialEq)]
pub enum Show {
    /// Always this HumanityOS page.
    Page(GuiPage),
    /// Always this camera post (its placed instance id).
    Camera(String),
}

impl Show {
    /// The source string a screen keeps (`ScreenDef::source`'s schemes).
    pub fn source(&self) -> String {
        match self {
            Show::Page(p) => format!("page:{}", crate::gui::dispatch::page_id(*p)),
            Show::Camera(id) => format!("camera:{id}"),
        }
    }

    /// The panel's reading of a source string; `None` for the kinds it does not offer yet.
    pub fn of_source(source: &str) -> Option<Show> {
        match ScreenSource::parse(source) {
            ScreenSource::Page(p) => Some(Show::Page(p)),
            ScreenSource::Camera(id) => Some(Show::Camera(id)),
            _ => None,
        }
    }
}

/// The pages a screen can show: every page but the two that need the engine to draw.
pub fn showable_pages() -> Vec<GuiPage> {
    crate::gui::dispatch::ALL_PAGES.iter().copied().filter(|p| !crate::gui::dispatch::ENGINE_BOUND_PAGES.contains(p)).collect()
}

/// A page's name in the person's language (its menu name when it has one, else its id).
pub fn page_name(page: GuiPage) -> String {
    let id = crate::gui::dispatch::page_id(page);
    let key = format!("nav.{id}");
    if humanity_core::i18n::has(&key) { t_dyn(&key).to_string() } else { id.to_string() }
}

/// The camera posts placed in the home, by instance id, in the file's order.
pub fn camera_posts(home: &MachineHome) -> Vec<String> {
    home.all_instances()
        .into_iter()
        .filter(|i| home.catalog.get(&i.machine).is_some_and(|d| d.camera.is_some()))
        .map(|i| i.id)
        .collect()
}

/// What placed screen `id` is now: its name, its source, its picture size in metres, and whether
/// that size is its own (resized) rather than the def's. `None` when it is not a placed screen.
pub fn current(home: &MachineHome, id: &str) -> Option<(String, String, (f32, f32), bool)> {
    let inst = home.all_instances().into_iter().find(|i| i.id == id)?;
    let def = home.catalog.get(&inst.machine)?;
    let sd = def.screen.as_ref()?;
    let source = inst.screen_source.clone().unwrap_or_else(|| sd.source.clone());
    let size = inst.screen_size_m.unwrap_or_else(|| display_size_of(def.size, &sd.face, sd.bezel_m));
    let name = if def.label.is_empty() { inst.machine.clone() } else { def.label.clone() };
    Some((name, source, size, inst.screen_size_m.is_some()))
}

/// The placed instance `id`, detached from its array first when it is an array cell (as a machine
/// drag does), so a change belongs to that one screen.
fn instance_mut<'a>(home: &'a mut MachineHome, id: &str) -> Option<&'a mut humanity_ship::machines::MachineInstance> {
    if !home.instances.iter().any(|m| m.id == id) {
        home.detach_array_member(id);
    }
    home.instances.iter_mut().find(|m| m.id == id)
}

/// Make placed screen `id` show `show`. Returns whether anything changed.
pub fn set_show(home: &mut MachineHome, id: &str, show: &Show) -> bool {
    let source = show.source();
    let Some(inst) = instance_mut(home, id) else { return false };
    if inst.screen_source.as_deref() == Some(source.as_str()) {
        return false;
    }
    inst.screen_source = Some(source);
    true
}

/// Set placed screen `id`'s picture size in metres (`None`: back to the def's). Returns whether
/// anything changed.
pub fn set_size(home: &mut MachineHome, id: &str, size: Option<(f32, f32)>) -> bool {
    let Some(inst) = instance_mut(home, id) else { return false };
    if inst.screen_size_m == size {
        return false;
    }
    inst.screen_size_m = size;
    true
}

/// The panel, while `state.screen_settings_open` names a placed screen.
pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let Some(id) = state.screen_settings_open.clone() else { return };
    let Some((name, source, size, custom)) = state.home_machines.as_ref().and_then(|h| current(h, &id)) else {
        state.screen_settings_open = None;
        return;
    };
    let posts = state.home_machines.as_ref().map(camera_posts).unwrap_or_default();
    let tuning = &crate::gui::screen_tuning::tuning().sizes;
    let shown = Show::of_source(&source);
    let mut open = true;
    let mut new_show: Option<Show> = None;
    let mut new_size: Option<Option<(f32, f32)>> = None;
    crate::gui::layers::window(ctx, egui::Id::new("screen_settings_window"), tf("screens.settings_title", &[("name", &name)]), crate::gui::layers::Layer::PageFloat)
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(380.0)
        // Over the middle of the viewport, clear of the editor's side panels.
        .default_pos(ctx.screen_rect().center() - egui::vec2(190.0, 160.0))
        .show(ctx, |ui| {
            ui.label(RichText::new(t("screens.how_to_edit")).size(theme.font_size_small).color(theme.text_secondary()));
            ui.add_space(theme.spacing_sm);

            // The size: a preset, or a width and a height.
            ui.label(RichText::new(t("screens.size")).strong().color(theme.text_primary()));
            ui.label(RichText::new(crate::gui::screen_tuning::size_label(size.0, size.1)).color(theme.text_secondary()));
            egui::ComboBox::from_id_salt("screen_settings_preset")
                .selected_text(t("screens.preset"))
                .show_ui(ui, |ui| {
                    for p in &tuning.presets {
                        let label = tf("screens.preset_item", &[("inches", &p.diagonal_in), ("a", &p.aspect.0), ("b", &p.aspect.1)]);
                        if ui.selectable_label(false, label).hint("screens.preset_item_hint").clicked() {
                            new_size = Some(Some(p.metres()));
                        }
                    }
                })
                .response
                .hint("screens.preset_hint");
            let (mut w, mut h) = size;
            ui.horizontal(|ui| {
                ui.label(t("screens.width"));
                let rw = ui.add(egui::DragValue::new(&mut w).speed(0.01).range(tuning.min_m.0..=tuning.max_m.0).max_decimals(2)).hint("screens.width_hint");
                ui.label(t("screens.height"));
                let rh = ui.add(egui::DragValue::new(&mut h).speed(0.01).range(tuning.min_m.1..=tuning.max_m.1).max_decimals(2)).hint("screens.height_hint");
                if rw.changed() || rh.changed() {
                    new_size = Some(Some((w, h)));
                }
            });
            if custom && ui.button(t("screens.default_size")).hint("screens.default_size_hint").clicked() {
                new_size = Some(None);
            }
            ui.add_space(theme.spacing_md);

            // What it shows.
            ui.label(RichText::new(t("screens.shows")).strong().color(theme.text_primary()));
            let is_page = matches!(shown, Some(Show::Page(_)));
            let is_camera = matches!(shown, Some(Show::Camera(_)));
            if ui.radio(is_page, t("screens.show_page")).hint("screens.show_page_hint").clicked() && !is_page {
                new_show = Some(Show::Page(GuiPage::Tasks));
            }
            if let Some(Show::Page(p)) = &shown {
                egui::ComboBox::from_id_salt("screen_settings_page")
                    .selected_text(page_name(*p))
                    .show_ui(ui, |ui| {
                        for page in showable_pages() {
                            if ui.selectable_label(page == *p, page_name(page)).hint("screens.page_item_hint").clicked() {
                                new_show = Some(Show::Page(page));
                            }
                        }
                    })
                    .response
                    .hint("screens.page_hint");
            }
            let camera_radio = ui.add_enabled(!posts.is_empty(), egui::RadioButton::new(is_camera, t("screens.show_camera"))).hint("screens.show_camera_hint");
            if camera_radio.clicked() && !is_camera {
                if let Some(first) = posts.first() {
                    new_show = Some(Show::Camera(first.clone()));
                }
            }
            if posts.is_empty() {
                ui.label(RichText::new(t("screens.no_camera")).size(theme.font_size_small).color(theme.text_muted()));
            }
            if let Some(Show::Camera(c)) = &shown {
                egui::ComboBox::from_id_salt("screen_settings_camera")
                    .selected_text(c.clone())
                    .show_ui(ui, |ui| {
                        for post in &posts {
                            if ui.selectable_label(post == c, post.clone()).hint("screens.camera_item_hint").clicked() {
                                new_show = Some(Show::Camera(post.clone()));
                            }
                        }
                    })
                    .response
                    .hint("screens.camera_hint");
            }
            if shown.is_none() {
                ui.label(RichText::new(tf("screens.show_other", &[("source", &source)])).size(theme.font_size_small).color(theme.text_muted()));
            }
            ui.add_space(theme.spacing_sm);
            if ui.button(t("common.close")).hint("screens.close_hint").clicked() {
                state.screen_settings_open = None;
            }
        });
    if !open {
        state.screen_settings_open = None;
    }
    let Some(home) = state.home_machines.as_mut() else { return };
    let mut changed = false;
    if let Some(show) = new_show {
        changed |= set_show(home, &id, &show);
    }
    if let Some(size) = new_size {
        changed |= set_size(home, &id, size);
    }
    if changed {
        state.construction_machines_dirty = true;
    }
}

#[cfg(test)]
#[path = "screen_settings_tests.rs"]
mod tests;
