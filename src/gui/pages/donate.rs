//! Donations page -- hero section, funding goal progress bar, donation method cards,
//! and collapsible FAQ sections.
//!
//! Supports dynamic donation addresses from server config (funding.addresses array)
//! with fallback to local config for offline mode.

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Vec2};
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
/// Preference order: the CONNECTED server's funding list (fetched from
/// /api/server-info on connect, v0.659) > the locally-configured Settings list
/// (a self-hosting operator's own) > the legacy hardcoded fallback.
/// Parse a "#rrggbb" hex color, falling back to the network-name color.
fn parse_hex_color(hex: &str, fallback: Color32) -> Color32 {
    let h = hex.trim().trim_start_matches('#');
    if h.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&h[0..2], 16),
            u8::from_str_radix(&h[2..4], 16),
            u8::from_str_radix(&h[4..6], 16),
        ) {
            return Color32::from_rgb(r, g, b); // theme-exempt: color parsed from data/donate/methods.json, not a hardcoded literal
        }
    }
    fallback
}

fn build_donation_sources(state: &GuiState) -> Vec<DonationSource> {
    let mut sources = Vec::new();

    // Direct-support link methods from data/donate/methods.json (GitHub, Patreon,
    // PayPal, Cash App). Data-driven + shared with the web donate page. These go
    // to the maintainer, not the Sponsor-A-Can 501c3.
    for m in &state.donate_methods {
        let color = parse_hex_color(&m.color, network_color(&m.network));
        let abbrev = if m.abbrev.is_empty() { network_abbrev(&m.network) } else { m.abbrev.clone() };
        sources.push(DonationSource {
            network: m.network.clone(),
            label: m.label.clone(),
            value: m.value.clone(),
            is_url: m.kind == "url",
            icon_abbrev: abbrev,
            icon_color: color,
        });
    }

    // Networks already added from methods.json, so a server-config duplicate
    // (e.g. GitHub Sponsors also in the relay's funding config) isn't listed
    // twice. Mirrors the web donate page's de-dupe.
    let mut seen: std::collections::HashSet<String> =
        sources.iter().map(|s| s.network.to_lowercase()).collect();

    let dynamic = if !state.donate_addresses_server.is_empty() {
        &state.donate_addresses_server
    } else {
        &state.donate_addresses
    };
    if !dynamic.is_empty() {
        for addr in dynamic {
            if !seen.insert(addr.network.to_lowercase()) { continue; }
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

    // Crypto fallback (GitHub Sponsors now comes from data/donate/methods.json).
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

// FAQ entries are loaded at startup from data/donate/faq.json into
// state.donate_faq (see crate::gui::load_donate_faq).

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
            // Resized to match state.donate_faq.len() at draw time.
            faq_open: Vec::new(),
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

                // Primary route: Sponsor-A-Can (the operator's 501(c)(3)).
                // Mirrors the web donate page (v0.845.1): the headline donation
                // channel, crypto demoted below. No tax-deductibility claim is
                // made pending the operator confirming the earmarking + the exact
                // donation URL; links the org's site for now.
                widgets::card(ui, theme, |ui| {
                    ui.label(
                        RichText::new("501(c)(3) nonprofit")
                            .size(theme.font_size_small)
                            .color(theme.accent())
                            .strong(),
                    );
                    ui.add_space(theme.spacing_xs);
                    ui.label(
                        RichText::new("Donate through Sponsor-A-Can")
                            .size(theme.font_size_heading)
                            .color(theme.text_primary()),
                    );
                    ui.add_space(theme.spacing_xs);
                    ui.label(
                        RichText::new("A registered 501(c)(3) nonprofit fighting poverty through sanitation and recycling programs, and the nonprofit behind HumanityOS's maintainer. Supporting it sustains both its mission and continued work on HumanityOS. Gifts are not earmarked to HumanityOS; tax-deductibility depends on your situation (see sponsor-a-can.org).")
                            .size(theme.font_size_body)
                            .color(theme.text_secondary()),
                    );
                    ui.add_space(theme.spacing_sm);
                    if widgets::Button::primary("Donate via Sponsor-A-Can").show(ui, theme) {
                        ui.ctx().open_url(egui::OpenUrl::new_tab("https://www.sponsor-a-can.org/donate/"));
                    }
                });
                ui.add_space(theme.spacing_lg);

                // Funding goal -- the CONNECTED server's real goal from
                // /api/server-info `funding.goal_usd`/`goal_label` (v0.659). Only
                // renders when a real goal exists; the old card showed a hardcoded
                // fake "$350 / $1000 -- 35% funded" progress bar regardless of
                // reality (same honesty bug class as Studio's fake bitrate). No
                // progress fraction is drawn because nothing tracks "raised so
                // far" yet -- a bar would just be a fabricated number again.
                if let Some((goal_usd, goal_label)) = &state.donate_funding_goal {
                    widgets::card(ui, theme, |ui| {
                        ui.label(
                            RichText::new("Funding Goal")
                                .size(theme.font_size_heading)
                                .color(theme.text_primary()),
                        );
                        ui.add_space(theme.spacing_sm);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("${:.0}", goal_usd))
                                    .size(theme.title_size)
                                    .color(theme.accent()),
                            );
                            if !goal_label.is_empty() {
                                ui.label(
                                    RichText::new(goal_label.as_str())
                                        .size(theme.font_size_body)
                                        .color(theme.text_secondary()),
                                );
                            }
                        });
                    });
                    ui.add_space(theme.spacing_lg);
                }

                // Donation method cards (secondary: direct crypto)
                ui.label(
                    RichText::new("Or donate crypto directly")
                        .size(theme.font_size_heading)
                        .color(theme.text_primary()),
                );
                ui.add_space(theme.spacing_sm);

                for source in &sources {
                    let has_value = !source.value.is_empty();

                    widgets::card(ui, theme, |ui| {
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
                                        if widgets::primary_button(ui, theme, "Open") {
                                            ui.ctx().open_url(egui::OpenUrl::new_tab(&source.value));
                                        }
                                    } else {
                                        ui.label(
                                            RichText::new(&source.value)
                                                .size(theme.font_size_body)
                                                .color(theme.text_primary())
                                                .monospace(),
                                        );
                                        if widgets::secondary_button(ui, theme, "Copy Address") {
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

                for (i, entry) in state.donate_faq.iter().enumerate() {
                    let len = state.donate_faq.len();
                    let is_open = with_local(|ds| {
                        if i >= ds.faq_open.len() {
                            ds.faq_open.resize(len, false);
                        }
                        ds.faq_open[i]
                    });

                    widgets::card(ui, theme, |ui| {
                        let arrow = if is_open { "v" } else { ">" };
                        let question_resp = ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(arrow)
                                    .size(theme.font_size_body)
                                    .color(theme.accent()),
                            );
                            ui.label(
                                RichText::new(&entry.question)
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
                                RichText::new(&entry.answer)
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
