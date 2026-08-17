//! Learn-by-doing quest chains (the surviving core of the old onboarding page).
//!
//! The standalone onboarding PAGE was retired in v0.373 (its hero/concepts
//! overlap the Mission Dashboard, which is the first-boot landing as of
//! v0.415.0); what remains here is the quest-chain machinery the top-level
//! Quests page renders. Chains are loaded from `data/onboarding/quests.json`
//! so adding new quests never requires a recompile. Progress is tracked
//! per-step in `gui_state.onboarding_quest_progress`.

use egui::{Frame, RichText, Rounding, Stroke, Vec2};
use serde::Deserialize;
use std::path::Path;

use crate::gui::theme::Theme;
use crate::gui::{GuiPage, GuiState};

#[derive(Debug, Clone, Deserialize)]
pub struct QuestStep {
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub link: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QuestChain {
    pub id: String,
    pub title: String,
    pub description: String,
    pub steps: Vec<QuestStep>,
}

#[derive(Debug, Clone, Deserialize)]
struct QuestFile {
    #[allow(dead_code)]
    #[serde(default = "default_version")]
    version: u32,
    chains: Vec<QuestChain>,
}

fn default_version() -> u32 { 1 }

/// Load quest chains from `data/onboarding/quests.json`.
/// Returns an empty vec on error so startup does not fail.
pub fn load_quest_chains(data_dir: &Path) -> Vec<QuestChain> {
    let path = data_dir.join("onboarding").join("quests.json");
    let bytes = match std::fs::read_to_string(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[onboarding] Could not read {}: {}", path.display(), e);
            return Vec::new();
        }
    };
    let parsed: QuestFile = match serde_json::from_str(&bytes) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[onboarding] Could not parse quests.json: {}", e);
            return Vec::new();
        }
    };
    log::info!("Loaded {} quest chains from {}", parsed.chains.len(), path.display());
    parsed.chains
}

// v0.415.0: the page renderer (draw + hero/concepts/core-pages/CTA sections)
// was deleted with the retired page; draw_quests below is the live surface.

/// Render the learn-by-doing quest chains. Since the v0.1145 Quests/Tasks
/// split this draws inside the Tasks page's guide panel, which supplies the
/// header and hint itself, so this fn renders chains only (its old
/// "QUEST CHAINS" section_header + hint line were deleted with their sole
/// caller). The chains come from `state.onboarding_quest_chains`
/// (data/onboarding/quests.json); First Steps sits at top.
pub fn draw_quests(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let avail = ui.available_width();

    if state.onboarding_quest_chains.is_empty() {
        ui.label(
            RichText::new(
                "Quest chains not loaded. Make sure data/onboarding/quests.json exists.",
            )
            .size(theme.font_size_small)
            .color(theme.text_muted())
            .italics(),
        );
        return;
    }

    for chain in state.onboarding_quest_chains.clone().iter() {
        Frame::none()
            .fill(theme.bg_card())
            .stroke(Stroke::new(1.0, theme.border()))
            .rounding(Rounding::same(theme.border_radius as u8))
            .inner_margin(theme.card_padding * 1.5)
            .show(ui, |ui| {
                ui.set_width(avail - 4.0);
                    ui.label(
                        RichText::new(&chain.title)
                            .size(theme.font_size_body)
                            .color(theme.text_primary())
                            .strong(),
                    );
                    ui.label(
                        RichText::new(&chain.description)
                            .size(theme.font_size_small)
                            .color(theme.text_secondary()),
                    );
                    ui.add_space(theme.spacing_sm);

                    for step in &chain.steps {
                        let key = format!("{}:{}", chain.id, step.id);
                        let done = state.onboarding_quest_progress.get(&key).copied().unwrap_or(false);

                        let row_bg = if done { theme.bg_secondary() } else { theme.bg_primary() };
                        let response = Frame::none()
                            .fill(row_bg)
                            .stroke(Stroke::new(1.0, theme.border()))
                            .rounding(Rounding::same(theme.border_radius as u8))
                            .inner_margin(theme.card_padding)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    // Checkbox
                                    let (cb_rect, _) = ui.allocate_exact_size(
                                        Vec2::splat(14.0),
                                        egui::Sense::hover(),
                                    );
                                    if ui.is_rect_visible(cb_rect) {
                                        let painter = ui.painter();
                                        let rounding = Rounding::same(2);
                                        if done {
                                            painter.rect_filled(cb_rect, rounding, theme.accent());
                                            painter.text(
                                                cb_rect.center(),
                                                egui::Align2::CENTER_CENTER,
                                                "✓",
                                                egui::FontId::proportional(9.0),
                                                theme.text_on_accent(),
                                            );
                                        } else {
                                            painter.rect_stroke(
                                                cb_rect,
                                                rounding,
                                                Stroke::new(1.0, theme.border()),
                                                egui::StrokeKind::Outside,
                                            );
                                        }
                                    }
                                    ui.add_space(6.0);
                                    ui.vertical(|ui| {
                                        ui.label(
                                            RichText::new(&step.title)
                                                .size(theme.font_size_small)
                                                .color(if done {
                                                    theme.text_muted()
                                                } else {
                                                    theme.text_primary()
                                                })
                                                .strong(),
                                        );
                                        ui.label(
                                            RichText::new(&step.description)
                                                .size(theme.font_size_small)
                                                .color(theme.text_secondary()),
                                        );
                                    });
                                });
                            })
                            .response
                            .interact(egui::Sense::click());

                        if response.clicked() {
                            let current = state.onboarding_quest_progress.get(&key).copied().unwrap_or(false);
                            state.onboarding_quest_progress.insert(key.clone(), !current);
                            // Write it out now (v0.1066). Before this, ticking a
                            // step only changed an in-memory map, so a person
                            // could work through a whole chain and find it blank
                            // again on the next launch.
                            crate::config::AppConfig::from_gui_state(state).save();

                            // If the step has a link to another page, also navigate.
                            if let Some(link) = step.link.as_ref() {
                                if let Some(page) = page_from_link(link) {
                                    state.active_page = page;
                                }
                            }
                        }
                        ui.add_space(4.0);
                    }
                });
        ui.add_space(theme.spacing_md);
    }
}

/// Map the `link` field on a quest step to a GuiPage where possible.
/// Returns None for external URLs or paths that do not correspond to a native page.
fn page_from_link(link: &str) -> Option<GuiPage> {
    match link.trim_start_matches('/') {
        "chat" => Some(GuiPage::Chat),
        "profile" => Some(GuiPage::Profile),
        "wallet" => Some(GuiPage::Wallet),
        "tasks" => Some(GuiPage::Tasks),
        "market" => Some(GuiPage::Market),
        "maps" => Some(GuiPage::Maps),
        "settings" => Some(GuiPage::Settings),
        "notes" => Some(GuiPage::Notes),
        "calendar" => Some(GuiPage::Calendar),
        "crafting" => Some(GuiPage::Crafting),
        // v0.1147: the Civilization page variant merged into the Humanity
        // tab's Mission Dashboard; the link lands on the hub that holds it.
        "civilization" => Some(GuiPage::Humanity),
        "guilds" => Some(GuiPage::Guilds),
        "tools" => Some(GuiPage::Tools),
        // Library and Platform became top-level tabs in v0.1063; without these
        // a quest step pointing at either would silently do nothing.
        "library" => Some(GuiPage::Library),
        "platform" => Some(GuiPage::Platform),
        "quests" => Some(GuiPage::Quests),
        "studio" => Some(GuiPage::Studio),
        "inventory" => Some(GuiPage::Inventory),
        // External URLs or unknown paths: no navigation.
        _ => None,
    }
}
