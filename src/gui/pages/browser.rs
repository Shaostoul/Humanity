//! Browser — curated bookmarks page. First step toward the in-app browser.
//!
//! For now each card opens its URL in the OS default browser via egui's
//! `open_url`. The page is structured so when the in-app browser ships
//! (CEF / wry / webview), the same data file drives it — at that point
//! the only change is that clicks load the page in an isolated tab
//! instead of handing off to the OS browser.
//!
//! Data source: `data/browser/bookmarks.json` (see BrowserBookmarks struct).
//! Categories filter via the chip bar; "all" shows everything.

use egui::{Frame, RichText, ScrollArea, Stroke};

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_primary()).inner_margin(0.0))
        .show(ctx, |ui| {
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.set_max_width(1024.0);
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        draw_header(ui, theme);
                        ui.add_space(theme.spacing_md);
                        draw_filter_bar(ui, theme, state);
                        ui.add_space(theme.spacing_md);
                        draw_categories(ctx, ui, theme, state);
                        ui.add_space(theme.spacing_xl);
                    });
                });
            });
        });
}

fn draw_header(ui: &mut egui::Ui, theme: &Theme) {
    ui.add_space(theme.spacing_xl);
    ui.label(
        RichText::new("BROWSER")
            .size(theme.font_size_small)
            .color(theme.accent())
            .strong(),
    );
    ui.add_space(theme.spacing_sm);
    ui.label(
        RichText::new("Curated links, opens in your default browser")
            .size(theme.font_size_title)
            .color(theme.text_primary())
            .strong(),
    );
    ui.add_space(theme.spacing_sm);
    ui.label(
        RichText::new(
            "These bookmarks are a stepping stone toward the in-app browser. \
             When that ships, the same list will load inside HumanityOS in an \
             isolated tab with credentials stored in your encrypted vault, \
             so each site sees a clean profile and can't track you across \
             others. Until then, clicks hand off to your OS browser.",
        )
        .size(theme.font_size_body)
        .color(theme.text_secondary()),
    );
}

fn draw_filter_bar(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.horizontal_wrapped(|ui| {
        if widgets::Button::secondary("All")
            .active(state.browser_filter == "all")
            .show(ui, theme)
        {
            state.browser_filter = "all".to_string();
        }
        for cat in &state.browser_bookmarks {
            let id = cat.id.clone();
            let is_active = state.browser_filter == id;
            if widgets::Button::secondary(&cat.name).active(is_active).show(ui, theme) {
                state.browser_filter = id;
            }
        }
    });
}

fn draw_categories(ctx: &egui::Context, ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    if state.browser_bookmarks.is_empty() {
        ui.label(
            RichText::new("No bookmarks loaded. Check data/browser/bookmarks.json.")
                .size(theme.font_size_small)
                .color(theme.text_muted())
                .italics(),
        );
        return;
    }

    let filter = state.browser_filter.clone();
    let cats = state.browser_bookmarks.clone();
    for cat in &cats {
        if filter != "all" && cat.id != filter { continue; }
        draw_category(ctx, ui, theme, cat);
        ui.add_space(theme.spacing_md);
    }
}

fn draw_category(
    ctx: &egui::Context,
    ui: &mut egui::Ui,
    theme: &Theme,
    cat: &crate::gui::BrowserCategory,
) {
    // Map category color names → theme tokens. "info" reuses accent_hover
    // since the theme palette doesn't have a dedicated info color.
    let header_color = match cat.color.as_str() {
        "info"    => theme.accent_hover(),
        "success" => theme.success(),
        "warning" => theme.warning(),
        "danger"  => theme.danger(),
        _         => theme.accent(),
    };

    ui.label(
        RichText::new(&cat.name)
            .size(theme.font_size_heading)
            .color(header_color)
            .strong(),
    );
    ui.add_space(theme.spacing_xs);

    // Card grid — let egui flow horizontally and wrap naturally.
    ui.horizontal_wrapped(|ui| {
        for bm in &cat.bookmarks {
            draw_bookmark_card(ctx, ui, theme, header_color, bm);
        }
    });
}

fn draw_bookmark_card(
    ctx: &egui::Context,
    ui: &mut egui::Ui,
    theme: &Theme,
    accent: egui::Color32,
    bm: &crate::gui::BrowserBookmark,
) {
    let card_w = 240.0;
    let card_h = 130.0;

    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(card_w, card_h),
        egui::Sense::click(),
    );
    if !ui.is_rect_visible(rect) { return; }

    let painter = ui.painter_at(rect);
    // Hover effect — overlay accent color at low alpha on the card bg.
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
        &bm.icon,
        egui::FontId::proportional(22.0),
        theme.text_primary(),
    );

    // Title (right of icon)
    let title_pos = icon_pos + egui::vec2(34.0, 2.0);
    painter.text(
        title_pos,
        egui::Align2::LEFT_TOP,
        &bm.title,
        egui::FontId::proportional(theme.font_size_body),
        theme.text_primary(),
    );

    // Description (below)
    let desc_pos = rect.left_top() + egui::vec2(theme.card_padding, theme.card_padding + 36.0);
    let desc_max = card_w - theme.card_padding * 2.0;
    let galley = ctx.fonts(|f| {
        f.layout(
            bm.description.clone(),
            egui::FontId::proportional(theme.font_size_small),
            theme.text_secondary(),
            desc_max,
        )
    });
    painter.galley(desc_pos, galley, theme.text_secondary());

    // Tiny URL hint at bottom (truncated host)
    let host = host_from_url(&bm.url);
    let url_pos = rect.left_bottom() + egui::vec2(theme.card_padding, -theme.card_padding);
    painter.text(
        url_pos,
        egui::Align2::LEFT_BOTTOM,
        &host,
        egui::FontId::monospace(theme.font_size_small),
        theme.text_muted(),
    );

    if resp.clicked() {
        ctx.open_url(egui::OpenUrl::new_tab(&bm.url));
    }
    resp.on_hover_text(format!("{}\n{}", bm.title, bm.url));
}

/// Pull the host out of a URL for display: "https://example.com/foo" → "example.com"
fn host_from_url(url: &str) -> String {
    let s = url.trim_start_matches("https://").trim_start_matches("http://");
    let end = s.find('/').unwrap_or(s.len());
    s[..end].to_string()
}
