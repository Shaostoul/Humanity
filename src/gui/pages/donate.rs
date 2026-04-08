//! Donations page -- hero section, funding goal progress bar, donation method cards,
//! and collapsible FAQ sections.
//!
//! Supports dynamic donation addresses from server config (funding.addresses array)
//! with fallback to local config for offline mode.

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Stroke, Vec2};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;
use std::cell::RefCell;

/// A donation source/method -- built dynamically from config.
struct DonationSource {
    network: String,
    label: String,
    value: String,
    is_url: bool,
    icon_abbrev: String,
    icon_color: Color32,
}

/// Map network name to an icon color.
fn network_color(name: &str) -> Color32 {
    let lower = name.to_lowercase();
    if lower.contains("github") { return Color32::from_rgb(110, 84, 148); }
    if lower.contains("solana") { return Color32::from_rgb(153, 69, 255); }
    if lower.contains("bitcoin") || lower.contains("btc") { return Color32::from_rgb(247, 147, 26); }
    if lower.contains("ethereum") || lower.contains("eth") { return Color32::from_rgb(98, 126, 234); }
    if lower.contains("monero") || lower.contains("xmr") { return Color32::from_rgb(255, 102, 0); }
    if lower.contains("litecoin") || lower.contains("ltc") { return Color32::from_rgb(191, 187, 187); }
    if lower.contains("polygon") || lower.contains("matic") { return Color32::from_rgb(130, 71, 229); }
    if lower.contains("cardano") || lower.contains("ada") { return Color32::from_rgb(0, 51, 173); }
    if lower.contains("dogecoin") || lower.contains("doge") { return Color32::from_rgb(194, 166, 51); }
    Color32::from_rgb(74, 153, 153) // default teal
}

/// Extract abbreviation from network name, e.g. "Solana (SOL)" -> "SOL"
fn network_abbrev(name: &str) -> String {
    if let Some(start) = name.find('(') {
        if let Some(end) = name.find(')') {
            if end > start + 1 {
                return name[start + 1..end].to_string();
            }
        }
    }
    // Fallback: first 3 alpha chars uppercase
    name.chars()
        .filter(|c| c.is_alphabetic())
        .take(3)
        .collect::<String>()
        .to_uppercase()
}

/// Build donation sources from the dynamic addresses in GuiState.
fn build_donation_sources(state: &GuiState) -> Vec<DonationSource> {
    let mut sources = Vec::new();

    // Use dynamic addresses if available
    if !state.donate_addresses.is_empty() {
        for addr in &state.donate_addresses {
            let abbrev = network_abbrev(&addr.network);
            let color = network_color(&addr.network);
            sources.push(DonationSource {
                network: addr.network.clone(),
                label: addr.label.clone(),
                value: addr.value.clone(),
                is_url: addr.addr_type == "url",
                icon_abbrev: abbrev,
                icon_color: color,
            });
        }
        return sources;
    }

    // Fallback: build from legacy fields
    sources.push(DonationSource {
        network: "GitHub Sponsors".into(),
        label: "Recurring or one-time sponsorship via GitHub.".into(),
        value: "https://github.com/sponsors/Shaostoul".into(),
        is_url: true,
        icon_abbrev: "GH".into(),
        icon_color: Color32::from_rgb(110, 84, 148),
    });

    let sol_address = if !state.donate_solana_address.is_empty() {
        state.donate_solana_address.clone()
    } else if !state.profile_public_key.is_empty() {
        crate::config::pubkey_hex_to_solana_address(&state.profile_public_key)
            .unwrap_or_default()
    } else {
        String::new()
    };

    sources.push(DonationSource {
        network: "Solana (SOL)".into(),
        label: "Send SOL or SPL tokens".into(),
        value: sol_address,
        is_url: false,
        icon_abbrev: "SOL".into(),
        icon_color: Color32::from_rgb(153, 69, 255),
    });

    let btc_address = state.donate_btc_address.clone();
    sources.push(DonationSource {
        network: "Bitcoin (BTC)".into(),
        label: "Send BTC".into(),
        value: btc_address,
        is_url: false,
        icon_abbrev: "BTC".into(),
        icon_color: Color32::from_rgb(247, 147, 26),
    });

    sources
}

struct FaqEntry {
    question: &'static str,
    answer: &'static str,
}

const FAQ: &[FaqEntry] = &[
    FaqEntry {
        question: "Where does my money go?",
        answer: "100% goes to server hosting, development tools, and contributor stipends. All spending is transparent and publicly tracked.",
    },
    FaqEntry {
        question: "Is it tax deductible?",
        answer: "HumanityOS is an open-source cooperative project. Formal nonprofit status is planned, which would enable tax-deductible donations in the future.",
    },
    FaqEntry {
        question: "Can I donate anonymously?",
        answer: "Yes! Cryptocurrency donations are pseudonymous by default. GitHub Sponsors also supports anonymous tiers.",
    },
    FaqEntry {
        question: "Can I contribute without money?",
        answer: "Absolutely! Code contributions, bug reports, translations, documentation, and community building are all incredibly valuable.",
    },
    FaqEntry {
        question: "How is funding tracked?",
        answer: "All crypto transactions are public on the blockchain. GitHub Sponsors provides monthly transparency reports. We publish quarterly spending breakdowns.",
    },
];

/// Local state for copied-address feedback and FAQ open state.
struct DonatePageState {
    copied_message: String,
    copied_timer: f32,
    faq_open: Vec<bool>,
}

impl Default for DonatePageState {
    fn default() -> Self {
        Self {
            copied_message: String::new(),
            copied_timer: 0.0,
            faq_open: vec![false; FAQ.len()],
        }
    }
}

thread_local! {
    static LOCAL: RefCell<DonatePageState> = RefCell::new(DonatePageState::default());
}

fn with_local<R>(f: impl FnOnce(&mut DonatePageState) -> R) -> R {
    LOCAL.with(|s| f(&mut s.borrow_mut()))
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let sources = build_donation_sources(state);

    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                // Hero section
                ui.add_space(theme.spacing_lg);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("Support HumanityOS")
                            .size(theme.title_size + 8.0)
                            .color(theme.text_primary()),
                    );
                    ui.add_space(theme.spacing_sm);
                    ui.label(
                        RichText::new("Help us end poverty and unite humanity through open-source technology.")
                            .size(theme.font_size_body)
                            .color(theme.text_secondary()),
                    );
                    ui.label(
                        RichText::new("Every contribution, no matter how small, makes a difference.")
                            .size(theme.font_size_body)
                            .color(theme.text_secondary()),
                    );
                });
                ui.add_space(theme.spacing_lg);

                // Funding goal progress bar
                widgets::card(ui, theme, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Monthly Funding Goal")
                                .size(theme.font_size_heading)
                                .color(theme.text_primary()),
                        );
                    });
                    ui.add_space(theme.spacing_sm);

                    let current = 350.0_f32;
                    let target = 1000.0_f32;
                    let progress = current / target;

                    let bar = egui::ProgressBar::new(progress.clamp(0.0, 1.0))
                        .fill(theme.accent())
                        .text(format!("${:.0} / ${:.0}", current, target));
                    ui.add(bar);

                    ui.add_space(theme.spacing_xs);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("{:.0}% funded", progress * 100.0))
                                .size(theme.font_size_body)
                                .color(theme.accent()),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new("Covers server hosting, domain, and dev tools")
                                    .size(theme.font_size_small)
                                    .color(theme.text_muted()),
                            );
                        });
                    });
                });

                ui.add_space(theme.spacing_lg);

                // Donation method cards
                ui.label(
                    RichText::new("Ways to Donate")
                        .size(theme.font_size_heading)
                        .color(theme.text_primary()),
                );
                ui.add_space(theme.spacing_sm);

                for source in &sources {
                    let has_value = !source.value.is_empty();

                    let frame = egui::Frame::none()
                        .fill(theme.bg_card())
                        .rounding(Rounding::same(theme.border_radius as u8))
                        .stroke(Stroke::new(1.0, theme.border()))
                        .inner_margin(theme.card_padding);

                    frame.show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // Icon: colored circle with abbreviation
                            let (icon_rect, _) = ui.allocate_exact_size(Vec2::new(44.0, 44.0), egui::Sense::hover());
                            ui.painter().rect_filled(icon_rect, Rounding::same(22), source.icon_color);
                            ui.painter().text(
                                icon_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                &source.icon_abbrev,
                                egui::FontId::proportional(12.0),
                                Color32::WHITE,
                            );

                            ui.add_space(theme.spacing_sm);

                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(&source.network)
                                        .size(theme.font_size_heading)
                                        .color(theme.text_primary()),
                                );
                                ui.label(
                                    RichText::new(&source.label)
                                        .size(theme.font_size_small)
                                        .color(theme.text_secondary()),
                                );
                                ui.add_space(theme.spacing_xs);

                                ui.horizontal(|ui| {
                                    if !has_value {
                                        ui.label(
                                            RichText::new("Not configured")
                                                .size(theme.font_size_body)
                                                .color(theme.text_muted()),
                                        );
                                    } else if source.is_url {
                                        ui.label(
                                            RichText::new(&source.value)
                                                .size(theme.font_size_body)
                                                .color(Theme::c32(&theme.info))
                                                .monospace(),
                                        );
                                        let open_btn = egui::Button::new(
                                            RichText::new("Open")
                                                .size(theme.font_size_small)
                                                .color(theme.text_on_accent()),
                                        )
                                        .fill(theme.accent())
                                        .min_size(Vec2::new(60.0, 24.0));
                                        if ui.add(open_btn).clicked() {
                                            ui.ctx().open_url(egui::OpenUrl::new_tab(&source.value));
                                        }
                                    } else {
                                        ui.label(
                                            RichText::new(&source.value)
                                                .size(theme.font_size_body)
                                                .color(theme.text_primary())
                                                .monospace(),
                                        );
                                        let copy_btn = egui::Button::new(
                                            RichText::new("Copy Address")
                                                .size(theme.font_size_small)
                                                .color(theme.text_primary()),
                                        )
                                        .fill(Color32::TRANSPARENT)
                                        .stroke(Stroke::new(1.0, theme.accent()))
                                        .min_size(Vec2::new(100.0, 24.0));
                                        if ui.add(copy_btn).clicked() {
                                            ui.output_mut(|o| {
                                                o.copied_text = source.value.clone();
                                            });
                                            with_local(|ds| {
                                                ds.copied_message = format!("Copied {} address!", source.network);
                                                ds.copied_timer = 3.0;
                                            });
                                        }
                                    }
                                });
                            });
                        });
                    });
                    ui.add_space(theme.spacing_sm);
                }

                // Copied feedback
                with_local(|ds| {
                    if ds.copied_timer > 0.0 {
                        ui.label(
                            RichText::new(&ds.copied_message)
                                .color(theme.success())
                                .size(theme.font_size_body),
                        );
                        ds.copied_timer -= ctx.input(|i| i.predicted_dt);
                        ctx.request_repaint();
                    }
                });

                ui.add_space(theme.spacing_lg);

                // FAQ section
                ui.label(
                    RichText::new("Frequently Asked Questions")
                        .size(theme.font_size_heading)
                        .color(theme.text_primary()),
                );
                ui.add_space(theme.spacing_sm);

                for (i, entry) in FAQ.iter().enumerate() {
                    let is_open = with_local(|ds| {
                        if i >= ds.faq_open.len() {
                            ds.faq_open.resize(FAQ.len(), false);
                        }
                        ds.faq_open[i]
                    });

                    let frame = egui::Frame::none()
                        .fill(theme.bg_card())
                        .rounding(Rounding::same(4))
                        .stroke(Stroke::new(1.0, theme.border()))
                        .inner_margin(12.0);

                    frame.show(ui, |ui| {
                        let arrow = if is_open { "v" } else { ">" };
                        let question_resp = ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(arrow)
                                    .size(theme.font_size_body)
                                    .color(theme.accent()),
                            );
                            ui.label(
                                RichText::new(entry.question)
                                    .size(theme.font_size_body)
                                    .color(theme.text_primary()),
                            );
                        }).response;

                        if question_resp.interact(egui::Sense::click()).clicked() {
                            with_local(|ds| {
                                if i < ds.faq_open.len() {
                                    ds.faq_open[i] = !ds.faq_open[i];
                                }
                            });
                        }

                        if is_open {
                            ui.add_space(theme.spacing_xs);
                            ui.label(
                                RichText::new(entry.answer)
                                    .size(theme.font_size_small)
                                    .color(theme.text_secondary()),
                            );
                        }
                    });
                    ui.add_space(theme.section_gap);
                }

                ui.add_space(theme.spacing_xl);
            });
        });
}
