//! Onboarding page — first-run orientation plus permanent reference.
//!
//! Mirrors the web `/onboarding` page. Quest chains are loaded from
//! `data/onboarding/quests.json` so adding new quests never requires a
//! recompile. Progress is tracked per-step in `gui_state.onboarding_quest_progress`.

use egui::{Align, Frame, Layout, RichText, Rounding, ScrollArea, Stroke, Vec2};
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

/// A core concept card shown above the quest chains.
struct Concept {
    title: &'static str,
    body: &'static str,
}

const CORE_CONCEPTS: &[Concept] = &[
    Concept {
        title: "Your DID is your identity",
        body: "A post-quantum cryptographic key on your device. No username, no password. Backup is a 24-word phrase in Settings. Lose your phone? Recover through guardians you pick \u{2014} friends and family who hold encrypted shares.",
    },
    Concept {
        title: "Credentials prove what's true about you",
        body: "Schools, employers, communities, even individual humans can issue Verifiable Credentials. You hold them, you choose when to share. No central registry, no single point of revocation.",
    },
    Concept {
        title: "Trust grows from what you do",
        body: "A multi-layer trust score combines your vouches, credentials, activity, and age. Anti-Sybil math makes farms count for less. Inputs are always visible \u{2014} no black-box reputation.",
    },
    Concept {
        title: "Anyone can host a server",
        body: "The network is federated. No central owner. If one server goes down, the rest keep working. Each server runs local governance; civilization-scope changes need federation-wide quorum.",
    },
];

/// Core page shortcuts rendered in a grid.
struct CorePage {
    label: &'static str,
    description: &'static str,
    page: GuiPage,
}

const CORE_PAGES: &[CorePage] = &[
    CorePage { label: "Chat", description: "Text, voice, video. Encrypted DMs, channels, servers.", page: GuiPage::Chat },
    CorePage { label: "Profile", description: "Your name, avatar, bio, skills. Seen by others across the network.", page: GuiPage::Profile },
    CorePage { label: "Wallet", description: "Optional Solana wallet derived from your seed. Send, receive, stake, tip. Decoupled from identity \u{2014} not required to use HumanityOS.", page: GuiPage::Wallet },
    CorePage { label: "Tasks", description: "Kanban boards for projects, teams, personal life.", page: GuiPage::Tasks },
    CorePage { label: "Market", description: "Peer-to-peer listings, reviews, buyer-seller messaging.", page: GuiPage::Market },
    CorePage { label: "Maps", description: "Local to galactic scale. Real navigation or sim exploration.", page: GuiPage::Maps },
    CorePage { label: "Settings", description: "Identity backup, notifications, theme, accessibility.", page: GuiPage::Settings },
    CorePage { label: "Notes", description: "Encrypted notes for journals, drafts, anything private.", page: GuiPage::Notes },
];

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_primary()).inner_margin(0.0))
        .show(ctx, |ui| {
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    draw_hero(ui, theme);
                    ui.add_space(theme.spacing_xl);
                    draw_concepts(ui, theme);
                    ui.add_space(theme.spacing_xl);
                    draw_core_pages(ui, theme, state);
                    ui.add_space(theme.spacing_xl);
                    draw_quests(ui, theme, state);
                    ui.add_space(theme.spacing_xl);
                    draw_cta(ui, theme, state);
                    ui.add_space(theme.spacing_xl * 2.0);
                });
        });
}

fn draw_hero(ui: &mut egui::Ui, theme: &Theme) {
    ui.add_space(theme.spacing_xl);
    ui.with_layout(Layout::top_down(Align::Center), |ui| {
        ui.label(
            RichText::new("GETTING STARTED")
                .size(theme.font_size_small)
                .color(theme.accent())
                .strong(),
        );
        ui.add_space(theme.spacing_sm);
        ui.label(
            RichText::new("Welcome. Let's get you oriented.")
                .size(theme.font_size_title)
                .color(theme.text_primary())
                .strong(),
        );
        ui.add_space(theme.spacing_md);
        ui.label(
            RichText::new(
                "HumanityOS is a free, public-domain platform for communication, \n\
                 coordination, and cooperation. Your identity lives on your device.\n\
                 No signup required. Your identity is created the moment you open the chat.",
            )
            .size(theme.font_size_body)
            .color(theme.text_secondary()),
        );
    });
}

fn draw_concepts(ui: &mut egui::Ui, theme: &Theme) {
    let max_w = ui.available_width().min(960.0);
    let col_w = ((max_w - theme.spacing_md as f32 * 3.0) / 4.0).max(180.0);

    ui.vertical_centered(|ui| {
        ui.set_max_width(max_w);
        ui.with_layout(Layout::left_to_right(Align::TOP), |ui| {
            ui.label(
                RichText::new("FOUR THINGS TO KNOW FIRST")
                    .size(theme.font_size_small)
                    .color(theme.accent())
                    .strong(),
            );
        });
        ui.add_space(theme.spacing_sm);
        ui.with_layout(Layout::left_to_right(Align::TOP), |ui| {
            ui.label(
                RichText::new("The four core concepts")
                    .size(theme.font_size_heading)
                    .color(theme.text_primary())
                    .strong(),
            );
        });
        ui.add_space(theme.spacing_md);

        ui.horizontal_wrapped(|ui| {
            for concept in CORE_CONCEPTS {
                Frame::none()
                    .fill(theme.bg_card())
                    .stroke(Stroke::new(1.0, theme.border()))
                    .rounding(Rounding::same(theme.border_radius as u8))
                    .inner_margin(theme.card_padding * 1.5)
                    .show(ui, |ui| {
                        ui.set_width(col_w);
                        ui.label(
                            RichText::new(concept.title)
                                .size(theme.font_size_body)
                                .color(theme.text_primary())
                                .strong(),
                        );
                        ui.add_space(theme.spacing_sm);
                        ui.label(
                            RichText::new(concept.body)
                                .size(theme.font_size_small)
                                .color(theme.text_secondary()),
                        );
                    });
            }
        });
    });
}

fn draw_core_pages(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let max_w = ui.available_width().min(960.0);
    let col_w = ((max_w - theme.spacing_md as f32 * 3.0) / 4.0).max(180.0);

    ui.vertical_centered(|ui| {
        ui.set_max_width(max_w);
        ui.with_layout(Layout::left_to_right(Align::TOP), |ui| {
            ui.label(
                RichText::new("THE PLATFORM")
                    .size(theme.font_size_small)
                    .color(theme.accent())
                    .strong(),
            );
        });
        ui.add_space(theme.spacing_sm);
        ui.with_layout(Layout::left_to_right(Align::TOP), |ui| {
            ui.label(
                RichText::new("Where to go next")
                    .size(theme.font_size_heading)
                    .color(theme.text_primary())
                    .strong(),
            );
        });
        ui.add_space(theme.spacing_md);

        let mut clicked: Option<GuiPage> = None;
        ui.horizontal_wrapped(|ui| {
            for page in CORE_PAGES {
                let response = Frame::none()
                    .fill(theme.bg_card())
                    .stroke(Stroke::new(1.0, theme.border()))
                    .rounding(Rounding::same(theme.border_radius as u8))
                    .inner_margin(theme.card_padding)
                    .show(ui, |ui| {
                        ui.set_width(col_w);
                        ui.set_height(70.0);
                        ui.label(
                            RichText::new(page.label)
                                .size(theme.font_size_body)
                                .color(theme.text_primary())
                                .strong(),
                        );
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new(page.description)
                                .size(theme.font_size_small)
                                .color(theme.text_secondary()),
                        );
                    })
                    .response
                    .interact(egui::Sense::click());

                if response.clicked() {
                    clicked = Some(page.page.clone());
                }
                if response.hovered() {
                    let painter = ui.painter();
                    painter.rect_stroke(
                        response.rect,
                        Rounding::same(theme.border_radius as u8),
                        Stroke::new(1.5, theme.accent()),
                        egui::StrokeKind::Outside,
                    );
                }
            }
        });

        if let Some(page) = clicked {
            state.active_page = page;
        }
    });
}

fn draw_quests(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let max_w = ui.available_width().min(960.0);

    ui.vertical_centered(|ui| {
        ui.set_max_width(max_w);
        ui.with_layout(Layout::left_to_right(Align::TOP), |ui| {
            ui.label(
                RichText::new("QUESTS")
                    .size(theme.font_size_small)
                    .color(theme.accent())
                    .strong(),
            );
        });
        ui.add_space(theme.spacing_sm);
        ui.with_layout(Layout::left_to_right(Align::TOP), |ui| {
            ui.label(
                RichText::new("Learn by doing")
                    .size(theme.font_size_heading)
                    .color(theme.text_primary())
                    .strong(),
            );
        });
        ui.add_space(theme.spacing_sm);
        ui.with_layout(Layout::left_to_right(Align::TOP), |ui| {
            ui.label(
                RichText::new(
                    "Small tasks that teach the platform. Click a step to mark it done. \
                     Progress saved locally.",
                )
                .size(theme.font_size_small)
                .color(theme.text_secondary()),
            );
        });
        ui.add_space(theme.spacing_md);

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
                    ui.set_width(max_w - 4.0);
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
    });
}

fn draw_cta(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.vertical_centered(|ui| {
        ui.set_max_width(600.0);
        ui.label(
            RichText::new("READY?")
                .size(theme.font_size_small)
                .color(theme.accent())
                .strong(),
        );
        ui.add_space(theme.spacing_sm);
        ui.label(
            RichText::new("Start using it")
                .size(theme.font_size_heading)
                .color(theme.text_primary())
                .strong(),
        );
        ui.add_space(theme.spacing_sm);
        ui.label(
            RichText::new(
                "The fastest way to understand HumanityOS is to open the chat and say hi.",
            )
            .size(theme.font_size_small)
            .color(theme.text_secondary()),
        );
        ui.add_space(theme.spacing_md);

        ui.horizontal(|ui| {
            let primary = egui::Button::new(
                RichText::new("Open the Chat")
                    .color(theme.text_on_accent())
                    .size(theme.font_size_body),
            )
            .fill(theme.accent())
            .rounding(Rounding::same(theme.border_radius as u8))
            .min_size(Vec2::new(140.0, theme.button_height));
            if ui.add(primary).clicked() {
                state.active_page = GuiPage::Chat;
            }

            let settings_btn = egui::Button::new(
                RichText::new("Settings")
                    .color(theme.text_primary())
                    .size(theme.font_size_body),
            )
            .fill(egui::Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, theme.border()))
            .rounding(Rounding::same(theme.border_radius as u8))
            .min_size(Vec2::new(100.0, theme.button_height));
            if ui.add(settings_btn).clicked() {
                state.active_page = GuiPage::Settings;
            }
        });
    });
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
        "civilization" => Some(GuiPage::Civilization),
        "guilds" => Some(GuiPage::Guilds),
        "tools" => Some(GuiPage::Tools),
        "studio" => Some(GuiPage::Studio),
        "inventory" => Some(GuiPage::Inventory),
        // External URLs or unknown paths: no navigation.
        _ => None,
    }
}
