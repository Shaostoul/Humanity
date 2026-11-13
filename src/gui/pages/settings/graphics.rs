//! Settings > Graphics (moved out of settings.rs on 2026-11-10; docs/design/graphics-settings.md).
//! Child of settings.rs.
//!
//! EVERY CONTROL SAYS WHAT IT CHANGES AND WHAT IT COSTS. The operator, 2026-11-10: "I want to make
//! sure when people change graphics settings they know what they're actually changing." So each
//! control here is followed by [`graphics_hint`]: its description by key (what you will see
//! differently) and its cost on this computer, worked out the way its row in
//! data/gui/graphics_settings.ron says: MEASURED live from the engine's own timers and memory
//! counters (the Performance page's numbers: the graphics card's time each frame in the passes the
//! control drives, the processor's, the video memory), WORKED OUT from the choice (shown beside it),
//! a DOWNLOAD whose size is on its button, or nothing measurable, with why. The test
//! `every_graphics_control_states_its_cost` fails when a control has no row or a row no cost.
//!
//! The website has no twin of this page: it draws no 3D world.

use super::*;

use humanity_core::frame_costs::{self, GpuTiming, Snapshot};
use humanity_core::i18n::{t, t_dyn, t_marked_dyn, tf};

/// How one control's cost is known (data/gui/graphics_settings.ron's `cost`).
#[derive(Debug, Clone, serde::Deserialize, PartialEq)]
pub(crate) enum Cost {
    /// It turns these passes on or off, or scales them: the graphics card's time, measured live.
    Gpu(Vec<String>),
    /// It changes work inside these passes, which do other work too: their time, measured live.
    Shares(Vec<String>),
    /// It changes these processor stages: measured live.
    Cpu(Vec<String>),
    /// It changes these video memory allocations: measured live.
    Memory(Vec<String>),
    /// It caps or paces frames: the frame time and rate, measured live.
    Frame,
    /// Its own lines work the cost out from the choice and show it beside each choice.
    Computed,
    /// It downloads; the sizes are on its buttons.
    Download,
    /// Nothing measurable; the words (by key) say why.
    Free(String),
}

/// One control's row.
#[derive(Debug, Clone, serde::Deserialize, PartialEq)]
pub(crate) struct Row {
    pub id: String,
    /// Its name's and hint's keys when they are not `graphics.<id>` and `graphics.<id>_hint` (the
    /// furniture and garment sections keep their own words).
    #[serde(default)]
    pub words: Option<String>,
    #[serde(default)]
    pub hint: Option<String>,
    pub cost: Vec<Cost>,
}

impl Row {
    pub fn hint_key(&self) -> String {
        self.hint.clone().unwrap_or_else(|| format!("graphics.{}_hint", self.id))
    }
}

/// data/gui/graphics_settings.ron.
#[derive(Debug, Clone, serde::Deserialize, PartialEq, Default)]
pub(crate) struct Registry {
    /// Each measured id's plain name, by key.
    pub passes: std::collections::BTreeMap<String, String>,
    pub settings: Vec<Row>,
}

impl Registry {
    pub fn row(&self, id: &str) -> Option<&Row> {
        self.settings.iter().find(|r| r.id == id)
    }
}

/// The compiled-in registry (embedded like the Performance page's, so a stripped install has it).
pub(crate) const REGISTRY_SRC: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/gui/graphics_settings.ron"));

pub(crate) fn registry() -> &'static Registry {
    static REG: std::sync::OnceLock<Registry> = std::sync::OnceLock::new();
    REG.get_or_init(|| {
        ron::from_str(REGISTRY_SRC).unwrap_or_else(|e| {
            log::error!("data/gui/graphics_settings.ron did not read ({e}); the Graphics page shows no costs");
            Registry::default()
        })
    })
}

/// The plain names of `ids`, each once, joined ("the cloud passes").
fn pass_names(reg: &Registry, ids: &[String]) -> String {
    let mut names: Vec<&str> = Vec::new();
    for id in ids {
        let name = reg.passes.get(id).map_or(id.as_str(), |k| t_dyn(k));
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names.join(", ")
}

/// The cost line for `row` from `snap` (the engine's measurements), in words.
pub(crate) fn cost_line(reg: &Registry, row: &Row, snap: &Snapshot) -> String {
    let ms = |ids: &[String]| ids.iter().map(|i| snap.value(i)).sum::<f64>();
    let mut parts: Vec<String> = Vec::new();
    // Before the world has drawn a frame: one line naming everything it will be measured from.
    if !snap.has_frames {
        let mut ids: Vec<String> = Vec::new();
        let mut frame = false;
        for c in &row.cost {
            match c {
                Cost::Gpu(i) | Cost::Shares(i) | Cost::Cpu(i) | Cost::Memory(i) => ids.extend(i.iter().cloned()),
                Cost::Frame => frame = true,
                _ => {}
            }
        }
        if !ids.is_empty() || frame {
            let mut what = pass_names(reg, &ids);
            if frame {
                what = if what.is_empty() { t("graphics.cost.frames").to_string() } else { format!("{what}, {}", t("graphics.cost.frames")) };
            }
            parts.push(tf("graphics.cost.unmeasured", &[("what", &what)]));
        }
    }
    for c in &row.cost {
        let part = match c {
            Cost::Gpu(_) | Cost::Shares(_) | Cost::Cpu(_) | Cost::Memory(_) | Cost::Frame if !snap.has_frames => continue,
            Cost::Gpu(ids) | Cost::Shares(ids) if snap.gpu_timing == GpuTiming::CpuFallback && !ids.iter().any(|i| snap.gpu_measurable(i)) => {
                tf("graphics.cost.unmeasurable", &[("what", &pass_names(reg, ids))])
            }
            Cost::Gpu(ids) => {
                let v = ms(ids);
                if v <= 0.0 {
                    tf("graphics.cost.gpu_idle", &[("what", &pass_names(reg, ids))])
                } else {
                    tf("graphics.cost.gpu", &[("ms", &format!("{v:.2}")), ("what", &pass_names(reg, ids))])
                }
            }
            Cost::Shares(ids) => tf("graphics.cost.shares", &[("what", &pass_names(reg, ids)), ("ms", &format!("{:.2}", ms(ids)))]),
            Cost::Cpu(ids) => tf("graphics.cost.cpu", &[("ms", &format!("{:.2}", ms(ids))), ("what", &pass_names(reg, ids))]),
            Cost::Memory(ids) => tf("graphics.cost.memory", &[("memory", &crate::renderer::bounced_light_detail::mb_words(ms(ids) as u64)), ("what", &pass_names(reg, ids))]),
            Cost::Frame => {
                let f = snap.frame_ms.max(0.001);
                tf("graphics.cost.frame", &[("ms", &format!("{f:.1}")), ("fps", &format!("{:.0}", 1000.0 / f))])
            }
            Cost::Computed => t("graphics.cost.computed").to_string(),
            Cost::Download => t("graphics.cost.download").to_string(),
            Cost::Free(key) => t_dyn(key).to_string(),
        };
        parts.push(part);
    }
    format!("{} {}", t("graphics.cost.label"), parts.join(" "))
}

thread_local! {
    /// The engine's measurements as this frame's page draws them: taken once when the page starts
    /// drawing and let go when it ends (one snapshot a frame, not one a control), so nothing in it
    /// outlives the frame it was taken for.
    static PAGE_SNAPSHOT: std::cell::RefCell<Option<Snapshot>> = const { std::cell::RefCell::new(None) };
}

/// The description under a graphics control: its hint (what you will see differently, by key, with
/// its glossary marks) and its cost on this computer ([`cost_line`]), shown the way the person's
/// hint mode asks (inline, behind a (?), or not at all). Every control on this page is followed by
/// one; `every_graphics_control_states_its_cost` holds it.
pub(crate) fn graphics_hint(ui: &mut egui::Ui, theme: &Theme, mode: crate::gui::HintDisplay, id: &str) {
    use crate::gui::HintDisplay;
    let reg = registry();
    let Some(row) = reg.row(id) else {
        log::warn!("[Graphics] no row for {id} in data/gui/graphics_settings.ron");
        widgets::setting_hint(ui, theme, mode, "");
        return;
    };
    // The page takes one snapshot a frame (`draw_graphics_content`); a description drawn outside it
    // takes its own.
    let cost = PAGE_SNAPSHOT.with(|s| match s.borrow().as_ref() {
        Some(snap) => cost_line(reg, row, snap),
        None => cost_line(reg, row, &frame_costs::snapshot()),
    });
    let hint_key = row.hint_key();
    match mode {
        HintDisplay::Full => {
            let saved = ui.spacing().item_spacing.y;
            ui.spacing_mut().item_spacing.y = theme.spacing_xs;
            widgets::glossary_words(ui, theme, t_marked_dyn(&hint_key), theme.font_size_small, theme.text_muted());
            ui.label(RichText::new(cost).size(theme.font_size_small).color(theme.text_muted()).italics());
            ui.spacing_mut().item_spacing.y = saved;
            ui.add_space(theme.spacing_md);
        }
        _ => widgets::setting_hint(ui, theme, mode, &format!("{}\n\n{}", t_dyn(&hint_key), cost)),
    }
}

/// The key of a window mode's name (`graphics.window_mode_choice.<key>`).
fn window_mode_key(mode: crate::config::WindowMode) -> &'static str {
    use crate::config::WindowMode as W;
    match mode {
        W::Windowed => "windowed",
        W::WindowedFullscreen => "windowed_fullscreen",
        W::BorderlessWindowed => "borderless_windowed",
        W::BorderlessFullscreen => "borderless_fullscreen",
        W::ExclusiveFullscreen => "exclusive_fullscreen",
    }
}

/// The key of a downloadable star catalogue's words (`graphics.star_tier_name.<key>`).
fn star_tier_key(tier: crate::renderer::stars::StarCatalogTier) -> &'static str {
    use crate::renderer::stars::StarCatalogTier as S;
    match tier {
        S::Extended => "extended",
        S::Ultra => "ultra",
    }
}

fn muted(ui: &mut egui::Ui, theme: &Theme, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).color(theme.text_muted()).size(theme.font_size_small));
}

fn warn(ui: &mut egui::Ui, theme: &Theme, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).color(theme.warning()).size(theme.font_size_small));
}

fn group_label(ui: &mut egui::Ui, theme: &Theme, key: &str) {
    ui.label(RichText::new(t_dyn(key)).color(theme.text_secondary()).strong());
}

/// Settings > Graphics > Light and sky > Bounced light detail (2026-10-20):
/// how finely the room light probes are laid out (`renderer::room_probes_gpu`,
/// the levels in data/gui/bounced_light_detail.json). One row per level with
/// its cap and its video memory, both worked out from the level's numbers;
/// then the graphics card's own memory and a warning when the chosen level
/// would take more than a twentieth of it. Applies live through
/// `engine::room_gi` the next frame.
fn draw_bounced_light_detail(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, hint: crate::gui::HintDisplay) {
    use crate::renderer::bounced_light_detail as gi;
    group_label(ui, theme, "graphics.bounced_light_detail");
    // A grid, so the four memory figures line up for comparing.
    egui::Grid::new("bounced_light_detail_levels").num_columns(2).show(ui, |ui| {
        for level in gi::detail_levels() {
            let sel = state.settings.bounced_light_detail == level.id;
            if ui.selectable_label(sel, RichText::new(level.name.as_str()).size(theme.font_size_small)).clicked() && !sel {
                state.settings.bounced_light_detail = level.id.clone();
                state.settings_dirty = true;
            }
            muted(ui, theme, level.words());
            ui.end_row();
        }
    });
    graphics_hint(ui, theme, hint, "bounced_light_detail");
    let level = gi::detail_level(&state.settings.bounced_light_detail);
    let (card, warning) = gi::memory_check(level, gi::adapter_memory());
    muted(ui, theme, card);
    if let Some(w) = warning {
        warn(ui, theme, w);
    }
}

/// Room light shadows (2026-11-10, renderer/local_shadows.rs, docs/design/local-light-shadows.md): the
/// levels from data/gui/shadow_levels.json, each with what it shadows and its video memory worked out
/// from its own numbers, the card's memory against the chosen level, and what the shadows are doing
/// this frame.
fn draw_local_shadows(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, hint: crate::gui::HintDisplay) {
    use crate::gui::widgets::Hint;
    use crate::renderer::bounced_light_detail::{adapter_memory, mb_words};
    use crate::renderer::local_shadows as ls;
    group_label(ui, theme, "graphics.local_shadows");
    egui::Grid::new("local_shadow_levels").num_columns(2).show(ui, |ui| {
        for level in ls::levels() {
            let sel = state.settings.local_shadows == level.id;
            let name = t_dyn(&format!("graphics.local_shadows_choice.{}", level.id));
            if ui.selectable_label(sel, RichText::new(name).size(theme.font_size_small)).hint("graphics.local_shadows_level_hint").clicked() && !sel {
                state.settings.local_shadows = level.id.clone();
                state.settings_dirty = true;
            }
            let words = if level.atlas == 0 {
                t("graphics.local_shadows_level_off").to_string()
            } else {
                tf(
                    "graphics.local_shadows_level",
                    &[
                        ("lights", &level.lights.to_string()),
                        ("face", &level.face.to_string()),
                        ("sun", &format!("{:.1}", level.sun_texel_cm())),
                        ("taps", &level.taps.to_string()),
                        ("memory", &mb_words(level.atlas_bytes())),
                    ],
                )
            };
            muted(ui, theme, words);
            ui.end_row();
        }
    });
    graphics_hint(ui, theme, hint, "local_shadows");
    // The card's own memory against the chosen level (the hardware-notice rule): an eighth of it is
    // where the line warns, as the garment textures' does.
    let level = ls::level(&state.settings.local_shadows);
    if level.atlas > 0 {
        let used = level.atlas_bytes();
        match adapter_memory().and_then(|a| a.dedicated_bytes.filter(|b| *b > 0).map(|b| (a.name.clone(), b))) {
            Some((card, own)) => {
                let share = used as f64 / own as f64 * 100.0;
                muted(
                    ui,
                    theme,
                    tf(
                        "graphics.local_shadows_card",
                        &[("card", &card), ("own", &mb_words(own)), ("used", &mb_words(used)), ("share", &format!("{share:.1}"))],
                    ),
                );
                if used * 8 > own {
                    warn(ui, theme, tf("graphics.local_shadows_card_warn", &[("used", &mb_words(used)), ("own", &mb_words(own))]));
                }
            }
            None => muted(ui, theme, t("graphics.local_shadows_card_unknown")),
        }
    }
    // What the shadows did this frame (the renderer publishes it with the frame costs).
    if let Some(s) = crate::renderer::local_shadows::published_stats() {
        // What the shading carries (shadows rung 1b): at Off the lookups are compiled out, so Off
        // costs the scene pass nothing; for the second or two the other variant takes to build after
        // a change, the line says the shading still holds the lookups.
        if s.lookups_building {
            muted(ui, theme, t("graphics.local_shadows_code_building"));
        } else if s.atlas == 0 && !s.lookups_compiled {
            muted(ui, theme, t("graphics.local_shadows_code_out"));
        }
        if s.atlas > 0 {
            muted(
                ui,
                theme,
                tf(
                    "graphics.local_shadows_now",
                    &[
                        ("shadowed", &s.shadowed.to_string()),
                        ("lights", &s.lights.to_string()),
                        ("over", &s.over_budget.to_string()),
                        ("views", &s.views.to_string()),
                    ],
                ),
            );
        }
    }
}

/// Anti-aliasing (2026-11-13, renderer/taa.rs, docs/design/renderer-roadmap.md section 3.6): the
/// modes with what each changes, the history's video memory at this window's size against the card's
/// own (the hardware-notice rule: compute shaders and float textures every card runs, so no card is
/// refused), the sharpening under Temporal, and why the vendor upscalers are not offered.
fn draw_anti_aliasing(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, hint: crate::gui::HintDisplay) {
    use crate::gui::widgets::Hint;
    use crate::renderer::bounced_light_detail::{adapter_memory, mb_words};
    use crate::renderer::taa;
    group_label(ui, theme, "graphics.anti_aliasing");
    egui::Grid::new("anti_aliasing_modes").num_columns(2).show(ui, |ui| {
        for id in taa::MODES {
            let sel = state.settings.anti_aliasing == *id;
            let name = t_dyn(&format!("graphics.anti_aliasing_choice.{id}"));
            if ui.selectable_label(sel, RichText::new(name).size(theme.font_size_small)).hint("graphics.anti_aliasing_choice_hint").clicked() && !sel {
                state.settings.anti_aliasing = id.to_string();
                state.settings_dirty = true;
            }
            muted(ui, theme, t_dyn(&format!("graphics.anti_aliasing_says.{id}")));
            ui.end_row();
        }
    });
    graphics_hint(ui, theme, hint, "anti_aliasing");
    if state.settings.anti_aliasing == "temporal" {
        // The history at this window's size, in physical pixels.
        let px = ui.ctx().screen_rect().size() * ui.ctx().pixels_per_point();
        let (w, h) = (px.x.round().max(1.0) as u32, px.y.round().max(1.0) as u32);
        let used = taa::Taa::history_bytes_at(w, h);
        let size = format!("{w} x {h}");
        match adapter_memory().and_then(|a| a.dedicated_bytes.filter(|b| *b > 0).map(|b| (a.name.clone(), b))) {
            Some((card, own)) => {
                let share = used as f64 / own as f64 * 100.0;
                muted(
                    ui,
                    theme,
                    tf("graphics.anti_aliasing_card", &[("card", &card), ("own", &mb_words(own)), ("used", &mb_words(used)), ("share", &format!("{share:.1}")), ("size", &size)]),
                );
                if used * 8 > own {
                    warn(ui, theme, tf("graphics.anti_aliasing_card_warn", &[("used", &mb_words(used)), ("own", &mb_words(own))]));
                }
            }
            None => muted(ui, theme, tf("graphics.anti_aliasing_memory", &[("used", &mb_words(used)), ("size", &size)])),
        }
        if widgets::labeled_slider(ui, theme, t("graphics.sharpening"), &mut state.settings.sharpening, 0.0..=1.0) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "sharpening");
    }
    muted(ui, theme, t("graphics.anti_aliasing_vendor"));
}

pub(crate) fn draw_graphics_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let hint = state.settings.hint_display;
    // The section accent drives every subsection header below, so the whole
    // ladder (section title > subsection > group label) wears one colour.
    let accent = section_accent(SettingsCategory::Graphics, theme);
    // The costs each control's line reads, measured this frame: the expensive inventory (the
    // driver's memory report) is sampled while the page is watched.
    frame_costs::watch();
    PAGE_SNAPSHOT.with(|s| *s.borrow_mut() = Some(frame_costs::snapshot()));
    draw_page(ui, theme, state, hint, accent);
    PAGE_SNAPSHOT.with(|s| *s.borrow_mut() = None);
}

fn draw_page(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, hint: crate::gui::HintDisplay, accent: Color32) {
    widgets::card(ui, theme, |ui| {
        // ── General: universal traits (window, frame pacing, camera) ──
        widgets::subsection_header(ui, theme, accent, t("graphics.section_general"), "");
        // Window presentation mode (v0.454). Default = Windowed fullscreen (maximized, title
        // bar + taskbar still visible). Selecting a mode applies it immediately.
        group_label(ui, theme, "graphics.window_mode");
        ui.horizontal_wrapped(|ui| {
            for mode in crate::config::WindowMode::ALL {
                let selected = state.settings.window_mode == mode;
                if ui.selectable_label(selected, t_dyn(&format!("graphics.window_mode_choice.{}", window_mode_key(mode)))).clicked() && !selected {
                    state.settings.window_mode = mode;
                    state.settings_dirty = true;
                }
            }
        });
        graphics_hint(ui, theme, hint, "window_mode");
        // Graphics presets (v0.1289, data/gui/graphics_presets.json). "Ultra
        // (reference)" is what the probe rig measures and what every
        // performance number in PRIORITIES means by Ultra, so an operator on
        // it sees what the rig sees; "Extreme" is every slider at its limit.
        // The presets' names and notes are the data file's own words.
        group_label(ui, theme, "graphics.presets");
        ui.horizontal_wrapped(|ui| {
            for p in graphics_presets() {
                if ui.button(p.name.as_str()).on_hover_text(p.note.as_str()).clicked() {
                    apply_graphics_preset(state, &p);
                }
            }
        });
        graphics_hint(ui, theme, hint, "presets");
        if widgets::toggle(ui, theme, t("graphics.vsync"), &mut state.settings.vsync) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "vsync");
        // Frame-rate caps (v0.1016, operator request: "set background and
        // foreground FPS"). Foreground = window focused; background =
        // alt-tabbed away (e.g. chatting on the website while the game
        // runs). The pacing itself lives in the redraw loop.
        if widgets::toggle(ui, theme, t("graphics.fps_foreground_unlimited"), &mut state.settings.fps_foreground_unlimited) {
            state.settings_dirty = true;
        }
        if !state.settings.fps_foreground_unlimited {
            let mut v = state.settings.fps_foreground as f32;
            if widgets::labeled_slider(ui, theme, t("graphics.fps_foreground"), &mut v, 10.0..=240.0) {
                state.settings.fps_foreground = v.round() as u32;
                state.settings_dirty = true;
            }
        }
        graphics_hint(ui, theme, hint, "fps_foreground");
        if widgets::toggle(ui, theme, t("graphics.fps_background_sync"), &mut state.settings.fps_background_sync) {
            state.settings_dirty = true;
        }
        if !state.settings.fps_background_sync {
            let mut v = state.settings.fps_background as f32;
            if widgets::labeled_slider(ui, theme, t("graphics.fps_background"), &mut v, 5.0..=240.0) {
                state.settings.fps_background = v.round() as u32;
                state.settings_dirty = true;
            }
        }
        graphics_hint(ui, theme, hint, "fps_background");
        if widgets::labeled_slider(ui, theme, t("graphics.fov"), &mut state.settings.fov, 60.0..=120.0) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "fov");
        if widgets::labeled_slider(ui, theme, t("graphics.render_distance"), &mut state.settings.render_distance, 50.0..=2000.0) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "render_distance");

        widgets::subsection_header(ui, theme, accent, t("graphics.section_planets"), t("graphics.section_planets_note"));
        widgets::setting_hint(ui, theme, hint, t("graphics.section_planets_hint"));
        // Procedural fractal surfaces (oceans, continents, polar caps) vs the
        // old flat single-color spheres. Data lives in data/planets/<id>.ron.
        if widgets::toggle(ui, theme, t("graphics.planet_detail"), &mut state.settings.planet_detail) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "planet_detail");
        if widgets::labeled_slider(ui, theme, t("graphics.planet_lod_px"), &mut state.settings.planet_lod_px, 4.0..=64.0) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "planet_lod_px");
        // Ceiling raised 7 -> 9 (2026-07-11) for FTL close approaches; the
        // top levels only trigger when one planet fills the screen (see
        // terrain::planet::MAX_SKY_SUBDIVISION for the face/memory table).
        if widgets::labeled_slider(ui, theme, t("graphics.planet_max_subdiv"), &mut state.settings.planet_max_subdiv, 0.0..=9.0) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "planet_max_subdiv");
        widgets::subsection_header(ui, theme, accent, t("graphics.section_ground"), t("graphics.section_ground_note"));
        // Chunked planetary LOD (2026-07-11): quadtree surface patches that
        // follow the camera once a planet fills the screen, replacing the
        // heavy uniform level 8-9 spheres near heightmap planets (Earth).
        if widgets::toggle(ui, theme, t("graphics.planet_chunked"), &mut state.settings.planet_chunked) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "planet_chunked");
        // Geomorph crossfades (v0.920): LOD swaps dissolve instead of pop.
        if widgets::toggle(ui, theme, t("graphics.terrain_lod_fade"), &mut state.settings.terrain_lod_fade) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "terrain_lod_fade");
        // Planet LOD knobs (v0.873, operator: "I want to see more real
        // terrain further away from me... add settings for all these
        // variables"). All four apply live next frame.
        if widgets::labeled_slider(ui, theme, t("graphics.terrain_split_px"), &mut state.settings.terrain_split_px, 2.0..=24.0) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "terrain_split_px");
        if widgets::labeled_slider(ui, theme, t("graphics.terrain_patch_budget"), &mut state.settings.terrain_patch_budget, 256.0..=12288.0) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "terrain_patch_budget");
        if widgets::labeled_slider(ui, theme, t("graphics.terrain_detail_distance"), &mut state.settings.terrain_detail_distance, 0.5..=3.0) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "terrain_detail_distance");
        if widgets::labeled_slider(ui, theme, t("graphics.terrain_builds_per_frame"), &mut state.settings.terrain_builds_per_frame, 6.0..=64.0) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "terrain_builds_per_frame");
        // ── Detail distances by item type (v0.965, operator: "a settings
        // section dedicated to the LODs of everything organized by type of
        // item") ── every row a live control; categories whose ladder
        // stages have not shipped yet are listed muted rather than given
        // lying sliders (honest-UI rule). Rows and labels come from the
        // LOD category registry (data/vegetation/lod_categories.ron).
        widgets::subsection_header(ui, theme, accent, t("graphics.section_distances"), "");
        widgets::setting_hint(ui, theme, hint, t("graphics.section_distances_hint"));
        // v0.1109 (operator: "sliders or maybe just number selectors so we can
        // go to any value ... then I wouldn't have to ask you to increase the
        // ceiling"): every distance here is a slider PLUS a number box. The
        // slider covers the band worth dragging; the box goes all the way to
        // the engine ceiling. Ceilings live in one place (config::TREE_MODEL_MAX_M
        // and friends) so the box, the saved config and the engine's clamp agree.
        widgets::setting_hint(ui, theme, hint, t("graphics.section_distances_entry_hint"));
        if !VEG_AWAITING_WIRING.is_empty() {
            // Better a blunt notice than a control that quietly does nothing.
            // This block deletes itself the moment the engine reads them; a
            // test fails if it is left standing after that.
            let names: Vec<&str> = VEG_AWAITING_WIRING.iter().map(|(_, label)| *label).collect();
            warn(ui, theme, tf("graphics.not_connected", &[("names", &names.join(", "))]));
        }
        let tree_label = humanity_core::lod_registry::category("tree").map_or(t("graphics.trees"), |c| c.label.as_str());
        if widgets::labeled_slider_entry(ui, theme, &tf("graphics.tree_model_distance", &[("tree", &tree_label)]), &mut state.settings.tree_model_distance, 0.0..=400.0, crate::config::TREE_MODEL_MAX_M, 2.0) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "tree_model_distance");
        if widgets::labeled_slider_entry(ui, theme, &tf("graphics.near_tree_budget", &[("tree", &tree_label)]), &mut state.settings.near_tree_budget, 32.0..=1024.0, crate::config::NEAR_TREE_BUDGET_MAX, 4.0) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "near_tree_budget");
        {
            // The honest bit: the budget, not the distance, is what usually
            // ends the model stage, and the engine handles running out
            // GRACEFULLY (it tracks how far models actually reached and hands
            // over to cards exactly there). The triangles are worked out.
            let budget = state.settings.near_tree_budget.max(1.0);
            let tris_m = budget * 155_000.0 / 1.0e6;
            muted(ui, theme, tf("graphics.near_tree_budget_cost", &[("tris", &format!("{tris_m:.1}"))]));
        }
        if widgets::labeled_slider_entry(ui, theme, &tf("graphics.veg_tree_card_m", &[("tree", &tree_label)]), &mut state.settings.veg_tree_card_m, 100.0..=3000.0, crate::config::TREE_CARD_MAX_M, 10.0) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "veg_tree_card_m");
        {
            // THE REQUESTED NUMBER NEXT TO THE REAL ONE (v0.1111). Cards are
            // baked into terrain patches at the 215 m detail level, so this
            // slider can only reveal trees as far as the terrain LOD actually
            // built that level - which depends on 'Terrain sharpness' and
            // 'Terrain patch budget', not on this control. The engine measures
            // the reach every selection; print it.
            let requested = state.settings.veg_tree_card_m;
            let reach = humanity_terrain::far_trees::measured_card_reach_m();
            if reach.is_finite() {
                let effective = humanity_terrain::far_trees::effective_card_far_m(requested, reach);
                let n = |v: f32| format!("{v:.0}");
                muted(ui, theme, tf("graphics.veg_tree_card_effective", &[("effective", &n(effective)), ("requested", &n(requested)), ("reach", &n(reach))]));
                if effective < requested - 1.0 {
                    warn(ui, theme, tf("graphics.veg_tree_card_too_far", &[("effective", &n(effective)), ("requested", &n(requested))]));
                }
            } else {
                muted(ui, theme, t("graphics.veg_tree_card_unmeasured"));
            }
        }
        // TREES, GRASS COVER and GRASS DETAIL are three separate controls
        // (v0.1106). ZERO MEANS OFF on all three (2026-09-18); the number box
        // beside each slider is there so exactly 0 can be TYPED, not just
        // dragged to; the range constants are the same ones the terrain code
        // counts with.
        if widgets::labeled_slider_entry(ui, theme, t("graphics.tree_density"), &mut state.settings.tree_density, humanity_terrain::planet_chunks::TREE_DENSITY_MIN..=humanity_terrain::planet_chunks::TREE_DENSITY_MAX, humanity_terrain::planet_chunks::TREE_DENSITY_MAX, 0.01) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "tree_density");
        // GRASS DRAW DISTANCE (v0.1109). The operator's headline ask: "I would
        // like to see how the game performs when I extend the grass to render
        // further away." Everything under it is a live estimate rather than a
        // static sentence, because the two costs that matter (instance count
        // and harvest walk) both depend on the COVERAGE slider as well.
        if widgets::labeled_slider_entry(ui, theme, t("graphics.grass_far_m"), &mut state.settings.grass_far_m, 12.0..=80.0, crate::config::GRASS_FAR_MAX_M, 0.5) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "grass_far_m");
        {
            let far = state.settings.grass_far_m;
            let cover = state.settings.grass_density;
            let drawn = grass_drawn_estimate(far, cover);
            let harvest = grass_harvest_estimate(far, cover);
            let walk = grass_harvest_walk_multiple(far);
            let cap = state.settings.grass_harvest_cap;
            muted(
                ui,
                theme,
                tf(
                    "graphics.grass_far_cost",
                    &[("far", &format!("{far:.0}")), ("drawn", &thousands(drawn)), ("tris", &thousands(drawn * 90.0)), ("harvest", &thousands(harvest)), ("walk", &format!("{walk:.1}"))],
                ),
            );
            if harvest >= cap {
                // The defect class this whole increment is guarding against: a
                // control that looks like it worked. The harvest walks
                // nearest-first and BREAKS at the cap, so the field ends in a
                // hard circle instead of fading.
                warn(ui, theme, tf("graphics.grass_far_over_cap", &[("harvest", &thousands(harvest)), ("cap", &thousands(cap)), ("fits", &format!("{:.0}", grass_far_within_cap(cap, cover)))]));
            }
        }
        if widgets::labeled_slider_entry(ui, theme, t("graphics.grass_harvest_cap"), &mut state.settings.grass_harvest_cap, 50_000.0..=500_000.0, crate::config::GRASS_HARVEST_CAP_MAX, 500.0) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "grass_harvest_cap");
        if widgets::labeled_slider_entry(ui, theme, t("graphics.grass_density"), &mut state.settings.grass_density, humanity_terrain::grass::GRASS_COVER_MIN..=humanity_terrain::grass::GRASS_COVER_MAX, humanity_terrain::grass::GRASS_COVER_MAX, 0.02) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "grass_density");
        if widgets::labeled_slider_entry(ui, theme, t("graphics.grass_detail"), &mut state.settings.grass_detail, 0.0..=1.0, 1.0, 0.01) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "grass_detail");
        if widgets::labeled_slider(ui, theme, t("graphics.water_detail_depth"), &mut state.settings.water_detail_depth, 14.0..=20.0) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "water_detail_depth");
        for cat in humanity_core::lod_registry::categories() {
            if cat.id == "tree" || cat.id == "water" {
                continue; // live sliders above own these
            }
            // v0.971: types whose live controls sit elsewhere say WHERE;
            // types with no shipped stages say so. Nothing is omitted and
            // nothing gets a dead slider (honest-UI rule).
            let line = if cat.controls_note.is_empty() {
                tf("graphics.lod_unshipped", &[("label", &cat.label)])
            } else {
                tf("graphics.lod_note", &[("label", &cat.label), ("note", &cat.controls_note)])
            };
            ui.label(RichText::new(line).color(theme.text_muted()).size(theme.font_size_small).italics());
        }
        // Image (2026-11-13, docs/design/renderer-roadmap.md section 3.6): how the picture is finished.
        widgets::subsection_header(ui, theme, accent, t("graphics.section_image"), "");
        draw_anti_aliasing(ui, theme, state, hint);
        widgets::subsection_header(ui, theme, accent, t("graphics.section_light"), "");
        // Lighting passes (v0.907): the three surface-lighting features
        // gained user controls. All apply live next frame.
        if widgets::toggle(ui, theme, t("graphics.sun_shadows"), &mut state.settings.sun_shadows) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "sun_shadows");
        draw_local_shadows(ui, theme, state, hint);
        // "Many lights at once" (renamed from "Tiled light lists" on 2026-08-25, renderer
        // jargon nobody would search for; the term is kept in the hint for anyone who knows it).
        if widgets::toggle(ui, theme, t("graphics.lights_tiled"), &mut state.settings.lights_tiled) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "lights_tiled");
        draw_bounced_light_detail(ui, theme, state, hint);
        if widgets::labeled_slider(ui, theme, t("graphics.shadow_strength"), &mut state.settings.shadow_strength, 0.0..=1.0) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "shadow_strength");
        if widgets::labeled_slider(ui, theme, t("graphics.aerial_strength"), &mut state.settings.aerial_strength, 0.0..=2.0) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "aerial_strength");
        if widgets::labeled_slider(ui, theme, t("graphics.godray_intensity"), &mut state.settings.godray_intensity, 0.0..=1.5) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "godray_intensity");
        // The Sun's glare (2026-10-21, renderer/sun_disc.rs) is the diffraction of the camera's
        // opening, and how wide it spreads depends on how wide that opening is. It was a dev key
        // only (showcase `sun_aperture_mm`) until the GUI-first rule caught it (2026-10-22).
        if widgets::labeled_slider(ui, theme, t("graphics.sun_aperture_mm"), &mut state.settings.sun_aperture_mm, 1.0..=100.0) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "sun_aperture_mm");
        if widgets::labeled_slider(ui, theme, t("graphics.ssao_strength"), &mut state.settings.ssao_strength, 0.0..=1.5) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "ssao_strength");
        // Analytic scattering atmosphere (v0.807): per-pixel single
        // scattering on the planet air shells. Off = the pre-v0.807 fresnel
        // tint, kept forever-dev style as the A/B reference + a safety hatch.
        if widgets::toggle(ui, theme, t("graphics.planet_atmo_scatter"), &mut state.settings.planet_atmo_scatter) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "planet_atmo_scatter");
        // Animated procedural cloud deck (clouds increment 1): a second
        // translucent shell under the atmosphere, on planets whose RON
        // declares cloud_coverage. Applies live.
        if widgets::toggle(ui, theme, t("graphics.planet_clouds"), &mut state.settings.planet_clouds) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "planet_clouds");
        // Live weather (v0.874): NASA GIBS MODIS cloud fraction placed on
        // the game sky. Fetcher spawns once per session; the toggle gates
        // both the spawn and per-frame uploads.
        if state.settings.planet_clouds {
            if widgets::toggle(ui, theme, t("graphics.live_weather"), &mut state.settings.live_weather) {
                state.settings_dirty = true;
            }
            graphics_hint(ui, theme, hint, "live_weather");
        }
        // Cloud quality ladder (clouds increment 3). Applies live: the cloud
        // material is made per (body, quality), so flipping tiers rebuilds
        // it the next frame the deck draws.
        if state.settings.planet_clouds {
            ui.horizontal(|ui| {
                ui.label(RichText::new(t("graphics.cloud_quality")).color(theme.text_secondary()));
                for val in ["low", "medium", "high", "ultra"] {
                    let sel = state.settings.cloud_quality == val;
                    if ui.selectable_label(sel, RichText::new(t_dyn(&format!("graphics.cloud_quality_choice.{val}"))).size(theme.font_size_small)).clicked() && !sel {
                        state.settings.cloud_quality = val.to_string();
                        state.settings_dirty = true;
                    }
                }
            });
            graphics_hint(ui, theme, hint, "cloud_quality");
        }
        // Close-range surface detail (v0.816): animated ocean waves + land
        // micro-texture on planets with real imagery. Applies live.
        if widgets::toggle(ui, theme, t("graphics.planet_surface_detail"), &mut state.settings.planet_surface_detail) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "planet_surface_detail");
        // Underwater clarity (v0.1054). Deliberately a slider rather than a
        // toggle: the operator wants the physical fade AND the ability to keep
        // seeing far underwater for exploration.
        if widgets::labeled_slider(ui, theme, t("graphics.water_clarity"), &mut state.settings.water_clarity, 0.0..=1.0) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "water_clarity");

        // ── Sky / map lines (v0.786, operator sky settings) ──
        ui.add_space(theme.spacing_md);
        group_label(ui, theme, "graphics.sky_lines");
        widgets::setting_hint(ui, theme, hint, t("graphics.sky_lines_hint"));
        ui.horizontal(|ui| {
            ui.label(RichText::new(t("graphics.sky_orbit_mode")).color(theme.text_secondary()));
            for val in ["off", "planets", "planets_moons"] {
                let sel = state.settings.sky_orbit_mode == val;
                if ui.selectable_label(sel, RichText::new(t_dyn(&format!("graphics.sky_orbit_mode_choice.{val}"))).size(theme.font_size_small)).clicked() && !sel {
                    state.settings.sky_orbit_mode = val.to_string();
                    state.settings_dirty = true;
                }
            }
        });
        graphics_hint(ui, theme, hint, "sky_orbit_mode");
        if widgets::toggle(ui, theme, t("graphics.sky_constellations"), &mut state.settings.sky_constellations) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "sky_constellations");
        // Milky Way glow (2026-07-10): the baked all-sky texture of real
        // integrated catalog starlight (data/galaxy_glow.png), drawn behind
        // the star points. Both controls apply live.
        if widgets::toggle(ui, theme, t("graphics.sky_milkyway_glow"), &mut state.settings.sky_milkyway_glow) {
            state.settings_dirty = true;
        }
        if state.settings.sky_milkyway_glow {
            if widgets::labeled_slider(ui, theme, t("graphics.sky_milkyway_intensity"), &mut state.settings.sky_milkyway_intensity, 0.0..=2.0) {
                state.settings_dirty = true;
            }
        }
        graphics_hint(ui, theme, hint, "sky_milkyway_glow");
        if state.settings.sky_milkyway_glow {
            // Its texture's memory, worked out: RGBA8 with no mip levels (renderer/stars.rs).
            let ultra = state.settings.sky_glow_tier == "ultra" && state.galaxy_glow_installed.is_some();
            let (w, h): (u64, u64) = if ultra { (16384, 8192) } else { (8192, 4096) };
            muted(ui, theme, tf("graphics.glow_memory", &[("memory", &crate::renderer::bounced_light_detail::mb_words(w * h * 4)), ("width", &w.to_string()), ("height", &h.to_string())]));
        }
        // Glow texture tier (2026-07-11): Standard ships with the app; Ultra
        // is a one-time download fetched exactly like the star catalog tiers
        // below (background thread, progress bar, retry on FAILED). The
        // chooser only offers Ultra once the file is installed; while absent
        // the Download button stands in for it.
        if state.settings.sky_milkyway_glow {
            ui.add_space(theme.spacing_xs);
            ui.horizontal(|ui| {
                ui.label(RichText::new(t("graphics.sky_glow_tier")).color(theme.text_secondary()).size(theme.font_size_small));
                let std_sel = state.settings.sky_glow_tier != "ultra";
                if ui.selectable_label(std_sel, RichText::new(t("graphics.sky_glow_standard")).size(theme.font_size_small)).clicked() && !std_sel {
                    state.settings.sky_glow_tier = "standard".to_string();
                    state.settings_dirty = true;
                }
                match state.galaxy_glow_installed {
                    Some(bytes) => {
                        let ultra_sel = state.settings.sky_glow_tier == "ultra";
                        let words = tf("graphics.sky_glow_ultra_installed", &[("mb", &(bytes / 1_048_576).to_string())]);
                        if ui.selectable_label(ultra_sel, RichText::new(words).size(theme.font_size_small)).clicked() && !ultra_sel {
                            state.settings.sky_glow_tier = "ultra".to_string();
                            state.settings_dirty = true;
                        }
                    }
                    None => {
                        // Same one-download-at-a-time rule as the catalog
                        // buttons: disabled while ACTIVELY transferring, a
                        // FAILED attempt re-enables (retry replaces the dead
                        // handle in lib.rs).
                        let downloading = state.galaxy_glow_dl.as_ref().and_then(|p| p.lock().ok().map(|g| !g.2.starts_with("FAILED"))).unwrap_or(false);
                        let words = tf("graphics.sky_glow_download", &[("size", &crate::renderer::bounced_light_detail::mb_words(crate::renderer::stars::GALAXY_GLOW_ULTRA_BYTES))]);
                        if ui.add_enabled(!downloading, egui::Button::new(words)).clicked() {
                            state.galaxy_glow_download = true;
                        }
                    }
                }
            });
            if let Some(dl) = &state.galaxy_glow_dl {
                if let Ok(g) = dl.lock() {
                    let (done, total, ref status) = *g;
                    if status.starts_with("FAILED") {
                        // Render a failure as a plain danger-colored line, NOT white text
                        // on the filled orange progress bar (operator feedback 2026-07-15).
                        ui.label(RichText::new(tf("graphics.sky_glow_failed", &[("status", &status.to_lowercase())])).color(theme.danger()).size(theme.font_size_small));
                    } else {
                        let frac = if total > 0 { done as f32 / total as f32 } else { 0.0 };
                        let words = tf("graphics.sky_glow_progress", &[("status", status), ("done", &(done / 1_048_576).to_string()), ("total", &(total.max(1) / 1_048_576).to_string())]);
                        ui.add(egui::ProgressBar::new(frac).text(words));
                    }
                }
            }
            if state.galaxy_glow_installed.is_some() && state.galaxy_glow_dl.is_none() && ui.button(t("graphics.sky_glow_remove")).clicked() {
                state.galaxy_glow_remove = true;
            }
            graphics_hint(ui, theme, hint, "sky_glow_tier");
        }
        // Star halos (2026-07-11): soft photographic glow + a faint 4-point
        // diffraction cross on the ~50 brightest stars (mag <= 2), drawn
        // additively over the star points. Applies live.
        if widgets::toggle(ui, theme, t("graphics.sky_star_halos"), &mut state.settings.sky_star_halos) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "sky_star_halos");

        // ── Star catalog (v0.800 rung 2; 2026-07-11 rung 4: 3-tier chooser) ──
        // Standard ships with the app; Extended (ATHYG) and Ultra (Gaia G<14)
        // are one-time downloads from GitHub release assets, dropped beside
        // stars.bin. The loader prefers the biggest installed catalog on the
        // next world entry.
        {
            use crate::renderer::stars::StarCatalogTier;
            ui.add_space(theme.spacing_md);
            group_label(ui, theme, "graphics.star_catalog");
            // Which tier actually renders: mirror of StarCatalog::load's
            // prefer order (biggest installed wins).
            let active = StarCatalogTier::ALL
                .iter()
                .rev()
                .find(|t| state.star_catalog_installed[t.index()].is_some())
                .map_or("standard", |t| star_tier_key(*t));
            let active = t_dyn(&format!("graphics.star_tier_name.{active}"));
            ui.label(RichText::new(tf("graphics.star_catalog_active", &[("tier", &active)])).color(theme.text_secondary()).size(theme.font_size_small));
            ui.add_space(theme.spacing_xs);

            // Render-tier CEILING (2026-07-12 dev tooling): caps which catalog
            // actually LOADS, independent of what is downloaded. Applies next
            // world entry. HUMANITY_STAR_TIER overrides this for scripted boots.
            ui.label(RichText::new(t("graphics.star_catalog_tier")).size(theme.font_size_small).color(theme.text_secondary()));
            ui.horizontal_wrapped(|ui| {
                for val in ["auto", "standard", "extended", "ultra"] {
                    let selected = state.settings.star_catalog_tier == val;
                    if ui.selectable_label(selected, t_dyn(&format!("graphics.star_catalog_tier_choice.{val}"))).clicked() && !selected {
                        state.settings.star_catalog_tier = val.to_string();
                        state.settings_dirty = true;
                    }
                }
            });
            graphics_hint(ui, theme, hint, "star_catalog_tier");

            // Standard tier: always installed, nothing to download or remove.
            muted(ui, theme, t("graphics.star_catalog_standard"));
            ui.add_space(theme.spacing_xs);

            // One download slot: every Download button disables while a
            // transfer is ACTIVELY running; a FAILED attempt re-enables them
            // (the retry click replaces the dead handle in lib.rs).
            let downloading = state.star_catalog_dl.as_ref().and_then(|(_, p)| p.lock().ok().map(|g| !g.2.starts_with("FAILED"))).unwrap_or(false);
            for tier in StarCatalogTier::ALL {
                let key = star_tier_key(tier);
                let name = t_dyn(&format!("graphics.star_tier_name.{key}"));
                let blurb = t_dyn(&format!("graphics.star_tier_blurb.{key}"));
                match state.star_catalog_installed[tier.index()] {
                    Some(bytes) => {
                        muted(ui, theme, tf("graphics.star_catalog_installed", &[("tier", &name), ("blurb", &blurb), ("mb", &(bytes / 1_048_576).to_string())]));
                        if ui.button(tf("graphics.star_catalog_remove", &[("tier", &name)])).clicked() {
                            state.star_catalog_remove = Some(tier);
                        }
                    }
                    None => {
                        muted(ui, theme, tf("graphics.star_catalog_offered", &[("tier", &name), ("blurb", &blurb)]));
                        let words = tf("graphics.star_catalog_download", &[("tier", &name), ("size", &crate::renderer::bounced_light_detail::mb_words(tier.download_bytes()))]);
                        if ui.add_enabled(!downloading, egui::Button::new(words)).clicked() {
                            state.star_catalog_download = Some(tier);
                        }
                    }
                }
                ui.add_space(theme.spacing_xs);
            }
            if let Some((tier, dl)) = &state.star_catalog_dl {
                if let Ok(g) = dl.lock() {
                    let (done, total, ref status) = *g;
                    let frac = if total > 0 { done as f32 / total as f32 } else { 0.0 };
                    let name = t_dyn(&format!("graphics.star_tier_name.{}", star_tier_key(*tier)));
                    let words = tf("graphics.star_catalog_progress", &[("tier", &name), ("status", status), ("done", &(done / 1_048_576).to_string()), ("total", &(total.max(1) / 1_048_576).to_string())]);
                    ui.add(egui::ProgressBar::new(frac).text(words));
                }
            }
            graphics_hint(ui, theme, hint, "star_catalog_download");
            muted(ui, theme, t("graphics.star_catalog_footer"));
        }

        // ── Experimental: default-off features kept for A/B and early access ──
        widgets::subsection_header(ui, theme, accent, t("graphics.section_experimental"), "");
        // FFT ocean (v0.1029, water-fft.md increment 1). Applies live: the
        // spectrum builds on first use; the mode flag rides the per-frame
        // uniform so geometry and buoyancy flip together.
        if widgets::toggle(ui, theme, t("graphics.water_fft"), &mut state.settings.water_fft) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "water_fft");
        // GPU particle simulation (v0.1068). Same shape as the FFT-ocean
        // toggle: experimental, default off, CPU path stays as the fallback.
        if widgets::toggle(ui, theme, t("graphics.gpu_particles"), &mut state.settings.gpu_particles) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "gpu_particles");
        // Far-tree card sheet (v0.1022, default off since v0.1029): kept
        // for A/B against the upcoming impostor system.
        if widgets::toggle(ui, theme, t("graphics.far_tree_sheet"), &mut state.settings.far_tree_sheet) {
            state.settings_dirty = true;
        }
        graphics_hint(ui, theme, hint, "far_tree_sheet");

        // ── Furniture: detail level and model packs (settings/furniture.rs) ──
        furniture::draw_furniture(ui, theme, state, hint, accent);

        // ── Garment textures: the people's clothes and hair (settings/garments.rs) ──
        garments::draw_garment_textures(ui, theme, state, hint, accent);

        // ── Machine labels and Home view ──
        widgets::subsection_header(ui, theme, accent, t("graphics.section_labels"), "");
        group_label(ui, theme, "graphics.machine_labels");
        // These live on GuiState (session-tunable); the defaults (21 / 13 / 8) are the
        // saved-feel values. Not persisted to settings yet.
        widgets::labeled_slider(ui, theme, t("graphics.machine_label_dot"), &mut state.machine_label_dot_dist, 2.0..=60.0);
        widgets::labeled_slider(ui, theme, t("graphics.machine_label_name"), &mut state.machine_label_name_dist, 1.0..=40.0);
        widgets::labeled_slider(ui, theme, t("graphics.machine_label_card"), &mut state.machine_label_card_dist, 1.0..=30.0);
        graphics_hint(ui, theme, hint, "machine_labels");

        ui.add_space(theme.spacing_md);
        group_label(ui, theme, "graphics.home");
        // Construction mode (v0.453): the home roof. Off by default so the sky shows through
        // the open top; on seals it. Also toggled with the R key in-world.
        widgets::toggle(ui, theme, t("graphics.show_roof"), &mut state.show_roof);
        graphics_hint(ui, theme, hint, "show_roof");
        // Hull wrap (ship-superstructure increment D): the generated exterior shell around the
        // zone cluster (data/blueprints/hull_profile.ron). Default ON; also toggled with H.
        widgets::toggle(ui, theme, t("graphics.show_hull"), &mut state.show_hull);
        graphics_hint(ui, theme, hint, "show_hull");
    });
}
