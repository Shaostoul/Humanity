//! Donations page -- hero section, funding goal progress bar, donation method cards,
//! and collapsible FAQ sections.
//!
//! Solana address is derived from the owner's Ed25519 public key (base58).
//! Bitcoin address is read from config. Both can be set in Settings > Account.

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Stroke, Vec2};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;
use std::cell::RefCell;

/// A donation source/method.
struct DonationSource {
    name: String,
    description: String,
    address_or_url: String,
    is_url: bool,
    icon_letter: &'static str,
    icon_color: Color32,
}

/// Build the list of donation sources dynamically from state.
fn build_donation_sources(state: &GuiState) -> Vec<DonationSource> {
    let mut sources = vec![
        DonationSource {
            name: "GitHub Sponsors".into(),
            description: "Recurring or one-time sponsorship via GitHub. Supports monthly tiers with perks.".into(),
            address_or_url: "https://github.com/sponsors/Shaostoul".into(),
            is_url: true,
            icon_letter: "GH",
            icon_color: Color32::from_rgb(110, 84, 148),
        },
    ];

    // Solana: use configured address, or derive from public key, or show "not configured"
    let sol_address = if !state.donate_solana_address.is_empty() {
        state.donate_solana_address.clone()
    } else if !state.profile_public_key.is_empty() {
        // Derive Solana address (base58) from Ed25519 public key
        crate::config::pubkey_hex_to_solana_address(&state.profile_public_key)
            .unwrap_or_else(|_| "Not configured".into())
    } else {
        "Not configured".into()
    };

    sources.push(DonationSource {
        name: "Solana (SOL)".into(),
        description: "Send SOL or SPL tokens to the project wallet. Fast, low fees.".into(),
        address_or_url: sol_address,
        is_url: false,
        icon_letter: "SOL",
        icon_color: Color32::from_rgb(153, 69, 255),
    });

    // Bitcoin: use configured address or show "not configured"
    let btc_address = if !state.donate_btc_address.is_empty() {
        state.donate_btc_address.clone()
    } else {
        "Not configured - set in Settings > Account > Donation Addresses".into()
    };

    sources.push(DonationSource {
        name: "Bitcoin (BTC)".into(),
        description: "Send BTC to the project address. The original cryptocurrency.".into(),
        address_or_url: btc_address,
        is_url: false,
        icon_letter: "BTC",
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
        answer: "Yes! Cryptocurrency donations (SOL, BTC) are pseudonymous by default. GitHub Sponsors also supports anonymous tiers.",
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
        .frame(Frame::none().fill(Color32::from_rgb(20, 20, 25)).inner_margin(16.0))
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                // Hero section
                ui.add_space(theme.spacing_lg);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("Support HumanityOS")
                            .size(36.0)
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

                    // Progress bar
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
                    let is_not_configured = source.address_or_url.starts_with("Not configured");

                    let frame = egui::Frame::none()
                        .fill(theme.bg_card())
                        .rounding(Rounding::same(theme.border_radius as u8))
                        .stroke(Stroke::new(1.0, theme.border()))
                        .inner_margin(theme.card_padding);

                    frame.show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // Icon
                            let (icon_rect, _) = ui.allocate_exact_size(Vec2::new(44.0, 44.0), egui::Sense::hover());
                            ui.painter().rect_filled(icon_rect, Rounding::same(8), source.icon_color);
                            ui.painter().text(
                                icon_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                source.icon_letter,
                                egui::FontId::proportional(14.0),
                                Color32::WHITE,
                            );

                            ui.add_space(theme.spacing_sm);

                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(&source.name)
                                        .size(theme.font_size_heading)
                                        .color(theme.text_primary()),
                                );
                                ui.label(
                                    RichText::new(&source.description)
                                        .size(theme.font_size_small)
                                        .color(theme.text_secondary()),
                                );
                                ui.add_space(theme.spacing_xs);

                                ui.horizontal(|ui| {
                                    let addr_color = if source.is_url {
                                        Theme::c32(&theme.info)
                                    } else if is_not_configured {
                                        theme.text_muted()
                                    } else {
                                        theme.text_primary()
                                    };
                                    ui.label(
                                        RichText::new(&source.address_or_url)
                                            .size(theme.font_size_body)
                                            .color(addr_color)
                                            .monospace(),
                                    );

                                    if source.is_url {
                                        let open_btn = egui::Button::new(
                                            RichText::new("Open")
                                                .size(theme.font_size_small)
                                                .color(theme.text_on_accent()),
                                        )
                                        .fill(theme.accent())
                                        .min_size(Vec2::new(60.0, 24.0));
                                        if ui.add(open_btn).clicked() {
                                            ui.ctx().open_url(egui::OpenUrl::new_tab(&source.address_or_url));
                                        }
                                    } else if !is_not_configured {
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
                                                o.copied_text = source.address_or_url.clone();
                                            });
                                            with_local(|ds| {
                                                ds.copied_message = format!("Copied {} address!", source.name);
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
                    ui.add_space(4.0);
                }

                ui.add_space(theme.spacing_xl);
            });
        });
}
