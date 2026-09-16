//! Browser: the websites database as site cards, and the readable web view.
//!
//! Two modes, decided by the `readable_web` opt-in (Settings > Privacy, off
//! by default):
//!
//! - OFF: every card opens its URL in the OS default browser. The app never
//!   fetches a web page. A hint on the page says where the switch is.
//! - ON: a card (or a typed address) opens INSIDE HumanityOS in the readable
//!   web view (`widgets/web_view.rs`): the page is fetched over HTTPS on a
//!   background thread, parsed with html5ever, and drawn as text, links,
//!   images, lists, tables and code. No browser engine, no JavaScript. Links
//!   navigate inside the view; "Open in browser" is the escape hatch for
//!   pages that need scripts.
//!
//! The cards come from `data/web/sites.json` (schema `schemas/web_sites.toml`),
//! one record per site with its embedding-legality and affiliate fields. The
//! web mirror (`web/pages/web.html`) reads the same file. A card's hover text
//! shows the site's embed status so the review backlog is visible where the
//! sites are. Design: docs/design/readable-web.md.

use egui::{Frame, RichText, ScrollArea, Stroke};

use crate::gui::theme::Theme;
use crate::gui::widgets::{self, ButtonVariant};
use crate::gui::{GuiPage, GuiState, SettingsCategory, WebSite, WebSiteCategory};

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_primary()).inner_margin(0.0))
        .show(ctx, |ui| {
            if state.settings.readable_web && state.web_view.is_open() {
                draw_reader(ui, theme, state);
            } else {
                draw_sites(ctx, ui, theme, state);
            }
        });
}

/// The in-app web view, full page. The toolbar's "Sites" button returns to
/// the card list; history survives so Back still works after reopening.
fn draw_reader(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // The affiliate transparency line (from the database) for the site the
    // current page belongs to, if it carries a tag. None today.
    let disclosure: Option<String> = state
        .web_view
        .current_url()
        .and_then(|u| state.web_sites.disclosure_for(u))
        .map(str::to_string);
    let inner = theme.spacing_md;
    Frame::none().inner_margin(egui::Margin::same(inner as i8)).show(ui, |ui| {
        let resp = state.web_view.show(ui, theme, &mut state.image_cache, disclosure.as_deref());
        if resp.wants_close {
            state.web_view.close();
        }
    });
}

/// The card list: header, opt-in hint or address row, category filter, cards.
fn draw_sites(ctx: &egui::Context, ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.set_max_width(1024.0);
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                draw_header(ui, theme, state);
                ui.add_space(theme.spacing_md);
                draw_filter_bar(ui, theme, state);
                ui.add_space(theme.spacing_md);
                draw_categories(ctx, ui, theme, state);
                ui.add_space(theme.spacing_xl);
            });
        });
    });
}

fn draw_header(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.add_space(theme.spacing_xl);
    ui.label(RichText::new("BROWSER").size(theme.font_size_small).color(theme.accent()).strong());
    ui.add_space(theme.spacing_sm);
    let on = state.settings.readable_web;
    ui.label(
        RichText::new(if on { "The readable web" } else { "Curated sites, opens in your default browser" })
            .size(theme.font_size_title)
            .color(theme.text_primary())
            .strong(),
    );
    ui.add_space(theme.spacing_sm);
    if on {
        ui.label(
            RichText::new(
                "Sites open inside HumanityOS as readable text, links, images and tables: no \
                 scripts, no cookies, no trackers. Only the address you open (and its images) \
                 leaves your machine, from your own connection. Pages that are only a \
                 JavaScript app will look empty here; the view's \"Open in browser\" button \
                 covers those.",
            )
            .size(theme.font_size_body)
            .color(theme.text_secondary()),
        );
        ui.add_space(theme.spacing_sm);
        // A typed address, for pages that are not on a card.
        ui.horizontal(|ui| {
            let field_w = (ui.available_width() - 80.0).max(160.0);
            let field = ui.add_sized(
                egui::vec2(field_w, theme.input_height),
                egui::TextEdit::singleline(&mut state.web_view.url_input)
                    .hint_text("Open an address: https://")
                    .font(egui::FontId::monospace(theme.font_size_small)),
            );
            let entered = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if widgets::compact_button(ui, theme, "Go", ButtonVariant::Primary) || entered {
                let u = state.web_view.url_input.clone();
                if !u.trim().is_empty() {
                    state.web_view.navigate(&u);
                }
            }
        });
    } else {
        ui.label(
            RichText::new(
                "These sites open in your system browser. HumanityOS can also read them \
                 inside the app (text, links, images and tables, with no scripts or \
                 trackers); that is off until you turn it on.",
            )
            .size(theme.font_size_body)
            .color(theme.text_secondary()),
        );
        ui.add_space(theme.spacing_xs);
        // GUI-first: the switch is one click away, not a doc reference.
        if widgets::Button::secondary("Turn on in-app reading (Settings > Privacy)")
            .size(widgets::ButtonSize::Small)
            .show(ui, theme)
        {
            state.settings.category = SettingsCategory::Privacy;
            state.settings.scroll_to_section = Some(SettingsCategory::Privacy);
            state.push_nav_to(GuiPage::Settings);
        }
    }
}

fn draw_filter_bar(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.horizontal_wrapped(|ui| {
        if widgets::Button::secondary("All").active(state.browser_filter == "all").show(ui, theme) {
            state.browser_filter = "all".to_string();
        }
        let cats: Vec<(String, String)> =
            state.web_sites.categories.iter().map(|c| (c.id.clone(), c.name.clone())).collect();
        for (id, name) in cats {
            let is_active = state.browser_filter == id;
            if widgets::Button::secondary(&name).active(is_active).show(ui, theme) {
                state.browser_filter = id;
            }
        }
    });
}

fn draw_categories(ctx: &egui::Context, ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    if state.web_sites.sites.is_empty() {
        ui.label(
            RichText::new("No sites loaded. Check data/web/sites.json.")
                .size(theme.font_size_small)
                .color(theme.text_muted())
                .italics(),
        );
        return;
    }

    let filter = state.browser_filter.clone();
    let readable = state.settings.readable_web;
    // Collect the click during the immutable draw, act on it after.
    let mut clicked: Option<String> = None;
    for cat in &state.web_sites.categories {
        if filter != "all" && cat.id != filter {
            continue;
        }
        let sites: Vec<&WebSite> = state.web_sites.sites.iter().filter(|s| s.category == cat.id).collect();
        if sites.is_empty() {
            continue;
        }
        draw_category(ctx, ui, theme, cat, &sites, readable, &mut clicked);
        ui.add_space(theme.spacing_md);
    }
    if let Some(url) = clicked {
        if readable {
            state.web_view.navigate(&url);
        } else {
            ctx.open_url(egui::OpenUrl::new_tab(&url));
        }
    }
}

fn draw_category(
    ctx: &egui::Context,
    ui: &mut egui::Ui,
    theme: &Theme,
    cat: &WebSiteCategory,
    sites: &[&WebSite],
    readable: bool,
    clicked: &mut Option<String>,
) {
    // Category color names map to theme tokens; "info" has its own token.
    let header_color = match cat.color.as_str() {
        "info" => theme.info(),
        "success" => theme.success(),
        "warning" => theme.warning(),
        "danger" => theme.danger(),
        _ => theme.accent(),
    };

    ui.label(RichText::new(&cat.name).size(theme.font_size_heading).color(header_color).strong());
    ui.add_space(theme.spacing_xs);

    // Card grid: let egui flow horizontally and wrap naturally.
    ui.horizontal_wrapped(|ui| {
        for site in sites {
            draw_site_card(ctx, ui, theme, header_color, site, readable, clicked);
        }
    });
}

fn draw_site_card(
    ctx: &egui::Context,
    ui: &mut egui::Ui,
    theme: &Theme,
    accent: egui::Color32,
    site: &WebSite,
    readable: bool,
    clicked: &mut Option<String>,
) {
    let card_w = 240.0;
    let card_h = 130.0;

    let (rect, resp) = ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());
    if !ui.is_rect_visible(rect) {
        return;
    }

    let painter = ui.painter_at(rect);
    // Hover effect: the accent at low alpha over the card background.
    let bg = if resp.hovered() {
        let a = theme.accent();
        egui::Color32::from_rgba_unmultiplied(a.r(), a.g(), a.b(), 30)
    } else {
        theme.bg_card()
    };
    let stroke_color = if resp.hovered() { accent } else { theme.border() };

    painter.rect_filled(rect, egui::Rounding::same(theme.border_radius as u8), bg);
    painter.rect_stroke(
        rect,
        egui::Rounding::same(theme.border_radius as u8),
        Stroke::new(1.5, stroke_color),
        egui::StrokeKind::Inside,
    );

    // Icon (top-left)
    let icon_pos = rect.left_top() + egui::vec2(theme.card_padding, theme.card_padding);
    painter.text(
        icon_pos,
        egui::Align2::LEFT_TOP,
        &site.icon,
        egui::FontId::proportional(22.0),
        theme.text_primary(),
    );

    // Name (right of icon)
    let title_pos = icon_pos + egui::vec2(34.0, 2.0);
    painter.text(
        title_pos,
        egui::Align2::LEFT_TOP,
        &site.name,
        egui::FontId::proportional(theme.font_size_body),
        theme.text_primary(),
    );

    // Description (below)
    let desc_pos = rect.left_top() + egui::vec2(theme.card_padding, theme.card_padding + 36.0);
    let desc_max = card_w - theme.card_padding * 2.0;
    let galley = ctx.fonts(|f| {
        f.layout(
            site.description.clone(),
            egui::FontId::proportional(theme.font_size_small),
            theme.text_secondary(),
            desc_max,
        )
    });
    painter.galley(desc_pos, galley, theme.text_secondary());

    // Host at the bottom
    let host = crate::gui::host_of(&site.url).unwrap_or_default();
    let url_pos = rect.left_bottom() + egui::vec2(theme.card_padding, -theme.card_padding);
    painter.text(
        url_pos,
        egui::Align2::LEFT_BOTTOM,
        &host,
        egui::FontId::monospace(theme.font_size_small),
        theme.text_muted(),
    );

    if resp.clicked() {
        *clicked = Some(site.url.clone());
    }
    // The hover text carries the embed-review state so the backlog is visible
    // where the sites are, not only in the data file.
    let opens = if readable { "Opens inside HumanityOS" } else { "Opens in your system browser" };
    let review = match site.embed.status.as_str() {
        "allowed" => format!("Embedding: allowed ({})", site.embed.basis),
        "forbidden" => format!("Embedding: forbidden ({})", site.embed.basis),
        "unknown" => "Embedding: terms could not be found".to_string(),
        _ => "Embedding: not yet reviewed".to_string(),
    };
    resp.on_hover_text(format!("{}\n{}\n{opens}\n{review}", site.name, site.url));
}
