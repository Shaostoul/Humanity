//! Donations page — funding progress, donation methods, and FAQ.

use egui::{Color32, RichText, Rounding, Vec2};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;

/// A donation source/method.
struct DonationSource {
    name: &'static str,
    description: &'static str,
    address_or_url: &'static str,
    is_url: bool,
}

const DONATION_SOURCES: &[DonationSource] = &[
    DonationSource {
        name: "GitHub Sponsors",
        description: "Recurring or one-time sponsorship via GitHub.",
        address_or_url: "https://github.com/sponsors/Shaostoul",
        is_url: true,
    },
    DonationSource {
        name: "Solana (SOL)",
        description: "Send SOL or SPL tokens to the project wallet.",
        address_or_url: "Shaostoul.sol",
        is_url: false,
    },
    DonationSource {
        name: "Bitcoin (BTC)",
        description: "Send BTC to the project address.",
        address_or_url: "bc1qhumanityos",
        is_url: false,
    },
];

struct FaqEntry {
    question: &'static str,
    answer: &'static str,
}

const FAQ: &[FaqEntry] = &[
    FaqEntry {
        question: "Where do donations go?",
        answer: "100% goes to server hosting, development tools, and contributor stipends. All spending is transparent.",
    },
    FaqEntry {
        question: "Is HumanityOS a nonprofit?",
        answer: "HumanityOS is an open-source cooperative project. Formal nonprofit status is planned.",
    },
    FaqEntry {
        question: "Can I contribute without money?",
        answer: "Absolutely! Code contributions, bug reports, translations, and community building are all valuable.",
    },
    FaqEntry {
        question: "How is funding tracked?",
        answer: "All transactions are public on the blockchain. GitHub Sponsors provides monthly transparency reports.",
    },
];

/// Local state for copied-address feedback.
pub struct DonatePageState {
    pub copied_message: String,
    pub copied_timer: f32,
}

impl Default for DonatePageState {
    fn default() -> Self {
        Self {
            copied_message: String::new(),
            copied_timer: 0.0,
        }
    }
}

fn with_state<R>(f: impl FnOnce(&mut DonatePageState) -> R) -> R {
    use std::cell::RefCell;
    thread_local! {
        static STATE: RefCell<DonatePageState> = RefCell::new(DonatePageState::default());
    }
    STATE.with(|s| f(&mut s.borrow_mut()))
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::Window::new("Donate")
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(Vec2::new(520.0, 520.0))
        .show(ctx, |ui| {
            ui.label(
                RichText::new("Support HumanityOS")
                    .size(theme.font_size_heading)
                    .color(theme.text_primary()),
            );
            ui.label(
                RichText::new("Help us end poverty and unite humanity.")
                    .size(theme.font_size_body)
                    .color(theme.text_secondary()),
            );
            ui.add_space(theme.spacing_md);

            // Funding goal progress bar
            widgets::card(ui, theme, |ui| {
                ui.label(
                    RichText::new("Monthly Funding Goal")
                        .color(theme.text_secondary()),
                );
                let progress = 0.35; // placeholder
                widgets::progress_bar(ui, theme, progress, Some(&format!("${} / ${} ({}%)", 350, 1000, (progress * 100.0) as u32)));
                ui.label(
                    RichText::new("Covers server hosting, domain, and dev tools.")
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
            });
            ui.add_space(theme.spacing_md);

            // Donation source cards
            egui::ScrollArea::vertical()
                .id_salt("donate_sources")
                .max_height(200.0)
                .show(ui, |ui| {
                    for source in DONATION_SOURCES {
                        widgets::card(ui, theme, |ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new(source.name)
                                            .size(theme.font_size_body)
                                            .color(theme.accent())
                                            .strong(),
                                    );
                                    ui.label(
                                        RichText::new(source.description)
                                            .size(theme.font_size_small)
                                            .color(theme.text_secondary()),
                                    );
                                    ui.horizontal(|ui| {
                                        let addr_color = if source.is_url {
                                            Theme::c32(&theme.info)
                                        } else {
                                            theme.text_primary()
                                        };
                                        ui.label(
                                            RichText::new(source.address_or_url)
                                                .size(theme.font_size_small)
                                                .color(addr_color),
                                        );
                                        if !source.is_url {
                                            if ui.small_button("Copy").clicked() {
                                                ui.output_mut(|o| {
                                                    o.copied_text = source.address_or_url.to_string();
                                                });
                                                with_state(|ds| {
                                                    ds.copied_message = format!("Copied {}", source.name);
                                                    ds.copied_timer = 2.0;
                                                });
                                            }
                                        } else if ui.small_button("Open").clicked() {
                                            ui.ctx().open_url(egui::OpenUrl::new_tab(source.address_or_url));
                                        }
                                    });
                                });
                            });
                        });
                        ui.add_space(4.0);
                    }
                });

            // Copied feedback
            with_state(|ds| {
                if ds.copied_timer > 0.0 {
                    ui.label(
                        RichText::new(&ds.copied_message)
                            .color(theme.success())
                            .size(theme.font_size_small),
                    );
                    ds.copied_timer -= ctx.input(|i| i.predicted_dt);
                }
            });

            ui.add_space(theme.spacing_md);

            // FAQ section
            widgets::collapsible_section(ui, "FAQ", false, |ui| {
                for entry in FAQ {
                    ui.add_space(theme.spacing_xs);
                    ui.label(
                        RichText::new(entry.question)
                            .color(theme.text_primary())
                            .strong(),
                    );
                    ui.label(
                        RichText::new(entry.answer)
                            .size(theme.font_size_small)
                            .color(theme.text_secondary()),
                    );
                }
            });

            ui.add_space(theme.spacing_sm);
            if widgets::secondary_button(ui, theme, "Close") {
                state.active_page = crate::gui::GuiPage::EscapeMenu;
            }
        });
}
