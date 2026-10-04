//! Donations page: the hero, then the ways to give (data/donate/routes.json: the
//! maintainer on Patreon, not tax-deductible, then the nonprofit Sponsor-a-Can,
//! tax-deductible), then more direct links with the server's funding goal,
//! endorsed charities, and the collapsible FAQ. The web page
//! (web/pages/donate-app.js) reads the same four data files;
//! tests/page_parity_lint.rs checks that both sides call the reads.
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
    if lower.contains("github") { return Color32::from_rgb(110, 84, 148); } // theme-exempt: GitHub brand color, third-party identity, not a design token
    if lower.contains("solana") { return Color32::from_rgb(153, 69, 255); } // theme-exempt: Solana brand color, third-party identity, not a design token
    if lower.contains("bitcoin") || lower.contains("btc") { return Color32::from_rgb(247, 147, 26); } // theme-exempt: Bitcoin brand color, third-party identity, not a design token
    if lower.contains("ethereum") || lower.contains("eth") { return Color32::from_rgb(98, 126, 234); } // theme-exempt: Ethereum brand color, third-party identity, not a design token
    if lower.contains("monero") || lower.contains("xmr") { return Color32::from_rgb(255, 102, 0); } // theme-exempt: Monero brand color, third-party identity, not a design token
    if lower.contains("litecoin") || lower.contains("ltc") { return Color32::from_rgb(191, 187, 187); } // theme-exempt: Litecoin brand color, third-party identity, not a design token
    if lower.contains("polygon") || lower.contains("matic") { return Color32::from_rgb(130, 71, 229); } // theme-exempt: Polygon brand color, third-party identity, not a design token
    if lower.contains("cardano") || lower.contains("ada") { return Color32::from_rgb(0, 51, 173); } // theme-exempt: Cardano brand color, third-party identity, not a design token
    if lower.contains("dogecoin") || lower.contains("doge") { return Color32::from_rgb(194, 166, 51); } // theme-exempt: Dogecoin brand color, third-party identity, not a design token
    Color32::from_rgb(74, 153, 153) // theme-exempt: neutral fallback in the brand-color lookup for an unrecognized network (default teal)
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

/// Compare links loosely: case, a trailing slash and http/https do not make two
/// different links. Mirrors `sameLink` in web/pages/donate-app.js.
fn same_link(a: &str, b: &str) -> bool {
    fn norm(u: &str) -> String {
        let l = u.trim().to_lowercase();
        let l = l.strip_prefix("https://").or_else(|| l.strip_prefix("http://")).unwrap_or(l.as_str());
        l.trim_end_matches('/').to_string()
    }
    let na = norm(a);
    !na.is_empty() && na == norm(b)
}

/// The three rules for listing an entry under "More ways to give to the
/// maintainer directly", the same as `add` in web/pages/donate-app.js: it has a
/// link or address, its network is not listed already, and its link is neither a
/// route card above nor a card already in this list. Records the network when it
/// admits one.
fn admit(
    state: &GuiState,
    listed: &[DonationSource],
    seen: &mut std::collections::HashSet<String>,
    network: &str,
    value: &str,
) -> bool {
    if network.is_empty() || value.trim().is_empty() || seen.contains(&network.to_lowercase()) {
        return false;
    }
    if state.donate_routes.iter().any(|r| same_link(&r.url, value))
        || listed.iter().any(|s| same_link(&s.value, value))
    {
        return false;
    }
    seen.insert(network.to_lowercase());
    true
}

/// Build the "More ways to give to the maintainer directly" list: the links in
/// data/donate/methods.json first, then the CONNECTED server's funding list
/// (fetched from /api/server-info on connect, v0.659), or failing that the
/// locally-configured Settings list (a self-hosting operator's own), or failing
/// that the two single-address Settings fields. Every entry passes `admit`.
fn build_donation_sources(state: &GuiState) -> Vec<DonationSource> {
    let mut sources = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // More direct links from data/donate/methods.json (GitHub Sponsors, PayPal,
    // Cash App), shared with the web donate page. Like the Patreon route above
    // them, these go to the maintainer, not to the nonprofit Sponsor-a-Can.
    for m in &state.donate_methods {
        if !admit(state, &sources, &mut seen, &m.network, &m.value) { continue; }
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

    let dynamic = if !state.donate_addresses_server.is_empty() {
        &state.donate_addresses_server
    } else {
        &state.donate_addresses
    };
    for addr in dynamic {
        if !admit(state, &sources, &mut seen, &addr.network, &addr.value) { continue; }
        sources.push(DonationSource {
            network: addr.network.clone(),
            label: addr.label.clone(),
            value: addr.value.clone(),
            is_url: addr.addr_type == "url",
            icon_abbrev: network_abbrev(&addr.network),
            icon_color: network_color(&addr.network),
        });
    }

    // The two single-address Settings fields, used only when neither the server
    // nor Settings has a list. Shown only when filled in: until 2026-10-04 an
    // empty Solana field fell back to an address made from the VIEWER'S OWN key,
    // offering people their own wallet as the place to send money.
    if dynamic.is_empty() {
        let legacy = [
            ("Solana (SOL)", "Send SOL or SPL tokens", &state.donate_solana_address),
            ("Bitcoin (BTC)", "Send BTC", &state.donate_btc_address),
        ];
        for (network, label, value) in legacy {
            if !admit(state, &sources, &mut seen, network, value) { continue; }
            sources.push(DonationSource {
                network: network.into(),
                label: label.into(),
                value: value.clone(),
                is_url: false,
                icon_abbrev: network_abbrev(network),
                icon_color: network_color(network),
            });
        }
    }

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

/// Colored circle with a short abbreviation: the icon every card on this page uses.
fn paint_icon(ui: &mut egui::Ui, abbrev: &str, color: Color32) {
    let (icon_rect, _) = ui.allocate_exact_size(Vec2::new(44.0, 44.0), egui::Sense::hover());
    ui.painter().rect_filled(icon_rect, Rounding::same(22), color);
    ui.painter().text(
        icon_rect.center(),
        egui::Align2::CENTER_CENTER,
        abbrev,
        egui::FontId::proportional(12.0),
        Color32::WHITE,
    );
}

/// A route is a card only when it has a name and a link to give through; one
/// without is skipped, as `renderRoutes` in web/pages/donate-app.js skips it.
fn route_is_shown(route: &crate::gui::DonateRoute) -> bool {
    !route.name.trim().is_empty() && !route.url.trim().is_empty()
}

/// One way to give (data/donate/routes.json). Laid out top to bottom so the
/// sentence wraps to the card's width: name, kind, the tax badge, who they are,
/// THE sentence (where the money goes, and whether it is tax-deductible), the
/// disclosure note, then the button. The web card (`renderRoutes` in
/// web/pages/donate-app.js) shows the same fields in the same order.
fn draw_route_card(ui: &mut egui::Ui, theme: &Theme, route: &crate::gui::DonateRoute) {
    widgets::card(ui, theme, |ui| {
        ui.horizontal(|ui| {
            let abbrev = if route.abbrev.is_empty() { network_abbrev(&route.name) } else { route.abbrev.clone() };
            paint_icon(ui, &abbrev, parse_hex_color(&route.color, network_color(&route.name)));
            ui.add_space(theme.spacing_sm);
            ui.vertical(|ui| {
                ui.label(RichText::new(&route.name).size(theme.font_size_heading).color(theme.text_primary()));
                if !route.kind.is_empty() {
                    ui.label(RichText::new(&route.kind).size(theme.font_size_small).color(theme.text_muted()));
                }
            });
        });
        ui.add_space(theme.spacing_xs);
        if route.tax_deductible {
            widgets::badge_sm(ui, theme, "Tax-deductible", theme.success());
        } else {
            widgets::badge_sm(ui, theme, "Not tax-deductible", theme.text_muted());
        }
        if !route.about.is_empty() {
            ui.add_space(theme.spacing_xs);
            ui.label(RichText::new(&route.about).size(theme.font_size_small).color(theme.text_secondary()));
        }
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new(&route.goes_to).size(theme.font_size_body).color(theme.text_primary()));
        if !route.note.is_empty() {
            ui.label(RichText::new(&route.note).size(theme.font_size_small).color(theme.text_muted()));
        }
        ui.add_space(theme.spacing_sm);
        ui.horizontal_wrapped(|ui| {
            let button = if route.button.is_empty() { "Open" } else { route.button.as_str() };
            if widgets::primary_button(ui, theme, button) {
                ui.ctx().open_url(egui::OpenUrl::new_tab(&route.url));
            }
            ui.label(RichText::new(&route.url).size(theme.font_size_small).color(Theme::c32(&theme.info)).monospace());
        });
    });
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
                    // Heading = the nav label ("Donate"); the warm phrase
                    // moves to the subtitle line below (naming rule).
                    ui.label(
                        RichText::new("Donate")
                            .size(theme.title_size + 8.0)
                            .color(theme.text_primary()),
                    );
                    ui.label(
                        RichText::new("Support HumanityOS")
                            .size(theme.font_size_heading)
                            .color(theme.text_secondary()),
                    );
                    ui.add_space(theme.spacing_sm);
                    ui.label(
                        RichText::new("HumanityOS is free and public-domain (CC0), with no company or nonprofit behind it.")
                            .size(theme.font_size_body)
                            .color(theme.text_secondary()),
                    );
                    ui.label(
                        RichText::new("It is built in the open by one person, so supporting the maintainer directly is what keeps the work going.")
                            .size(theme.font_size_body)
                            .color(theme.text_secondary()),
                    );
                });
                ui.add_space(theme.spacing_lg);

                // The ways to give (data/donate/routes.json, in file order), as
                // the operator set them on 2026-10-04: the maintainer on Patreon
                // (he receives it, not tax-deductible) and the nonprofit
                // Sponsor-a-Can (tax-deductible, the money goes to Sponsor-a-Can).
                // Patreon is first because this page is "Support HumanityOS" and
                // the Humanity page's "Fund the work" buttons open it. Each card
                // says where the money goes in one sentence, with a badge.
                // HumanityOS itself still has no company or nonprofit behind it
                // (the hero says so); Sponsor-a-Can is its own organization,
                // which the FAQ spells out.
                if state.donate_routes.iter().any(route_is_shown) {
                    ui.label(
                        RichText::new("Ways to give")
                            .size(theme.font_size_heading)
                            .color(theme.text_primary()),
                    );
                    ui.add_space(theme.spacing_sm);
                    for route in state.donate_routes.iter().filter(|r| route_is_shown(r)) {
                        draw_route_card(ui, theme, route);
                        ui.add_space(theme.spacing_sm);
                    }
                    ui.add_space(theme.spacing_lg);
                }

                // More direct links, under one sentence that says they are not
                // tax-deductible either. Hidden when there is nothing to list.
                let show_direct = !sources.is_empty() || state.donate_funding_goal.is_some();
                if show_direct {
                    ui.label(
                        RichText::new("More ways to give to the maintainer directly")
                            .size(theme.font_size_heading)
                            .color(theme.text_primary()),
                    );
                    ui.label(
                        RichText::new("These also go to the maintainer personally, and are not tax-deductible.")
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                    ui.add_space(theme.spacing_sm);
                }

                // Funding goal -- the CONNECTED server's real goal from
                // /api/server-info `funding.goal_usd`/`goal_label` (v0.659). Only
                // renders when a real goal exists; the old card showed a hardcoded
                // fake "$350 / $1000 -- 35% funded" progress bar regardless of
                // reality (same honesty bug class as Studio's fake bitrate). No
                // progress fraction is drawn because nothing tracks "raised so
                // far" yet -- a bar would just be a fabricated number again. The
                // web page drew one until 2026-10-04 and now matches this.
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

                for source in &sources {
                    widgets::card(ui, theme, |ui| {
                        ui.horizontal(|ui| {
                            paint_icon(ui, &source.icon_abbrev, source.icon_color);
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

                                // Every listed source has a value (`admit` skips
                                // empty ones, as the web page does).
                                ui.horizontal(|ui| {
                                    if source.is_url {
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

                // Charities the maintainer personally endorses (independent
                // nonprofits from data/donate/charities.json). NOT HumanityOS
                // funding: a gift goes to that organization directly. Sponsor-a-Can
                // was listed here from v0.846.3 until 2026-10-04, when it became
                // one of the ways to give above; the list is empty for now, so
                // the section is hidden.
                if !state.donate_charities.is_empty() {
                    ui.label(
                        RichText::new("Charities I support")
                            .size(theme.font_size_heading)
                            .color(theme.text_primary()),
                    );
                    ui.label(
                        RichText::new("Independent nonprofits I personally stand behind, unaffiliated with HumanityOS. Donate to them directly; deductibility depends on the charity and your situation.")
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                    ui.add_space(theme.spacing_sm);
                    for c in &state.donate_charities {
                        widgets::card(ui, theme, |ui| {
                            ui.label(
                                RichText::new(&c.name)
                                    .size(theme.font_size_heading)
                                    .color(theme.text_primary()),
                            );
                            if !c.mission.is_empty() {
                                ui.add_space(theme.spacing_xs);
                                ui.label(
                                    RichText::new(&c.mission)
                                        .size(theme.font_size_body)
                                        .color(theme.text_secondary()),
                                );
                            }
                            if !c.note.is_empty() {
                                ui.add_space(theme.spacing_xs);
                                ui.label(
                                    RichText::new(&c.note)
                                        .size(theme.font_size_small)
                                        .color(theme.text_muted()),
                                );
                            }
                            if !c.url.is_empty() {
                                ui.add_space(theme.spacing_sm);
                                if widgets::Button::primary("Donate").show(ui, theme) {
                                    ui.ctx().open_url(egui::OpenUrl::new_tab(&c.url));
                                }
                            }
                        });
                        ui.add_space(theme.spacing_sm);
                    }
                    ui.add_space(theme.spacing_lg);
                }

                // FAQ section (data/donate/faq.json). Like the web page, no
                // heading over an empty FAQ, and every answer starts closed.
                if !state.donate_faq.is_empty() {
                    ui.label(
                        RichText::new("Frequently Asked Questions")
                            .size(theme.font_size_heading)
                            .color(theme.text_primary()),
                    );
                    ui.add_space(theme.spacing_sm);
                }

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

#[cfg(test)]
mod tests {
    use super::build_donation_sources;
    use crate::gui::{DonateFaqEntry, DonateMethod, DonateRoute, GuiState};
    use crate::gui::screen_surface::find_text_in_shapes;

    fn route(url: &str, tax_deductible: bool) -> DonateRoute {
        DonateRoute {
            name: "R".into(), kind: String::new(), about: String::new(), url: url.into(),
            button: String::new(), goes_to: String::new(), tax_deductible,
            note: String::new(), abbrev: String::new(), color: String::new(),
        }
    }

    fn method(network: &str, value: &str) -> DonateMethod {
        DonateMethod {
            network: network.into(), label: String::new(), value: value.into(),
            kind: "url".into(), abbrev: String::new(), color: String::new(),
        }
    }

    /// The shipped routes file is what tells people where their money goes, so
    /// each card's one sentence must say the same thing its flag says. Seen red
    /// 2026-10-04 by dropping the "not" from the Patreon sentence on disk (then
    /// restoring it): "Shaostoul on Patreon: tax_deductible is false but its
    /// sentence says `Your gift goes to Shaostoul, the maintainer, who receives
    /// it personally, so it is tax-deductible.`" And by flipping that route's
    /// flag to true: the "no direct route (the maintainer)" assertion fired.
    #[test]
    fn every_shipped_route_says_what_its_tax_flag_says() {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let routes = crate::gui::load_donate_routes(&data);
        assert!(routes.iter().any(|r| r.tax_deductible), "no tax-deductible route (the nonprofit) in data/donate/routes.json");
        assert!(routes.iter().any(|r| !r.tax_deductible), "no direct route (the maintainer) in data/donate/routes.json");
        for r in &routes {
            let says_not = r.goes_to.contains("not tax-deductible");
            assert!(r.goes_to.contains("tax-deductible"), "{}: its sentence never says whether it is tax-deductible", r.name);
            assert!(says_not != r.tax_deductible,
                "{}: tax_deductible is {} but its sentence says `{}`", r.name, r.tax_deductible, r.goes_to);
            assert!(r.url.starts_with("https://"), "{}: not an https link: {}", r.name, r.url);
        }
    }

    /// A link that is already a route card is not listed again further down.
    /// Seen red 2026-10-04 before the de-dupe: the Patreon link appeared twice
    /// ("assertion `left == right` failed, left: 2, right: 1").
    #[test]
    fn a_route_link_is_not_listed_twice() {
        let mut state = GuiState::default();
        state.donate_routes = vec![route("https://www.patreon.com/Shaostoul", false)];
        state.donate_methods = vec![
            method("Patreon", "https://www.patreon.com/Shaostoul/"),
            method("PayPal", "https://paypal.me/Shaostoul"),
        ];
        let listed = build_donation_sources(&state);
        let patreon = state.donate_routes.len() + listed.iter().filter(|s| s.value.contains("patreon.com")).count();
        assert_eq!(patreon, 1);
        assert_eq!(listed.len(), 1, "PayPal stays");
    }

    /// Draw the whole page headlessly (no GPU) and say whether `text` appears.
    /// The screen is tall so nothing the page draws is scrolled out of view.
    fn page_shows(state: &mut GuiState, text: &str) -> bool {
        let ctx = egui::Context::default();
        crate::gui::fonts::install_font_fallbacks(&ctx);
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        let mut out = None;
        for _ in 0..2 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 4000.0))),
                ..Default::default()
            };
            out = Some(ctx.run(input, |ctx| super::draw(ctx, &theme, state)));
        }
        find_text_in_shapes(&out.expect("two frames ran").shapes, text).is_some()
    }

    /// Native draws what the web page draws (web/pages/donate-app.js): a route
    /// with no link is not a card, and the FAQ heading is not drawn over an
    /// empty FAQ. The filled state is checked too, so a page that drew nothing
    /// at all could not pass. Seen red 2026-10-04 before either check was in
    /// draw(): "the FAQ heading is drawn with no FAQ entries"; then, with only
    /// the FAQ check in: "a route with no link is drawn as a card".
    #[test]
    fn empty_faq_and_a_route_with_no_link_are_not_drawn() {
        let mut state = GuiState::default();
        let mut nowhere = route("  ", false);
        nowhere.name = "Nowhere Fund".into();
        state.donate_routes = vec![nowhere];
        assert!(!page_shows(&mut state, "Frequently Asked Questions"), "the FAQ heading is drawn with no FAQ entries");
        assert!(!page_shows(&mut state, "Nowhere Fund"), "a route with no link is drawn as a card");
        assert!(!page_shows(&mut state, "Ways to give"), "the Ways to give heading is drawn over no cards");

        let mut somewhere = route("https://example.org/give", false);
        somewhere.name = "Somewhere Fund".into();
        state.donate_routes.push(somewhere);
        state.donate_faq = vec![DonateFaqEntry { question: "Where does it go?".into(), answer: "There.".into() }];
        assert!(page_shows(&mut state, "Somewhere Fund"), "a route with a link is drawn");
        assert!(page_shows(&mut state, "Ways to give"), "the Ways to give heading is drawn over a card");
        assert!(page_shows(&mut state, "Frequently Asked Questions"), "the FAQ heading is drawn over an entry");
        assert!(!page_shows(&mut state, "Nowhere Fund"), "the route with no link is still not drawn");
    }

    /// Nothing configured means nothing listed: no "Not configured" cards, and
    /// never an address made from the VIEWER'S OWN key, which the old fallback
    /// did (a 32-byte key became a Solana address offered as the place to send
    /// money). Seen red 2026-10-04: "listed: [(\"Solana (SOL)\",
    /// \"29d2S7vB453rNYFdR5Ycwt7y9haRT5fwVwL9zTmBhfV2\"), (\"Bitcoin (BTC)\", \"\")]
    /// left: 2, right: 0", the viewer's own address and an empty card.
    #[test]
    fn nothing_configured_lists_nothing_and_never_the_viewers_own_wallet() {
        let mut state = GuiState::default();
        state.profile_public_key = "11".repeat(32);
        let listed = build_donation_sources(&state);
        assert_eq!(listed.len(), 0, "listed: {:?}", listed.iter().map(|s| (&s.network, &s.value)).collect::<Vec<_>>());
    }
}
