//! Category overview / landing page (v0.181.0).
//!
//! Each top-tier nav button (Reality / Sim / Tools / Settings / Dev) lands
//! here when clicked. The page renders a header with the category color +
//! description, then a card grid of every sub-page with its label and
//! one-line description. Click a card to navigate into the page.
//!
//! Single shared draw function — the dispatcher in lib.rs picks which
//! category id to render based on the active GuiPage variant.

use egui::{Frame, RichText, ScrollArea, Stroke};

use crate::gui::theme::Theme;
use crate::gui::{GuiPage, GuiState};

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, category_id: &str) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_primary()).inner_margin(0.0))
        .show(ctx, |ui| {
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.set_max_width(1024.0);
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        let meta = crate::gui::pages::escape_menu::category_meta(category_id, theme);
                        if let Some((label, color, summary)) = meta {
                            draw_header(ui, theme, label, color, summary);
                        } else {
                            ui.label(RichText::new("Unknown category").color(theme.danger()));
                            return;
                        }

                        ui.add_space(theme.spacing_lg);

                        let pages = crate::gui::pages::escape_menu::category_pages(category_id);
                        let cat_color = meta.unwrap().1;

                        if pages.is_empty() {
                            ui.label(
                                RichText::new("No pages in this category yet.")
                                    .size(theme.font_size_small)
                                    .color(theme.text_muted())
                                    .italics(),
                            );
                            return;
                        }

                        draw_card_grid(ctx, ui, theme, state, &pages, cat_color);
                        ui.add_space(theme.spacing_xl);
                    });
                });
            });
        });
}

fn draw_header(ui: &mut egui::Ui, theme: &Theme, label: &str, color: egui::Color32, summary: &str) {
    ui.add_space(theme.spacing_xl);
    ui.label(
        RichText::new(label.to_uppercase())
            .size(theme.font_size_small)
            .color(color)
            .strong(),
    );
    ui.add_space(theme.spacing_sm);
    ui.label(
        RichText::new(label)
            .size(theme.font_size_title)
            .color(theme.text_primary())
            .strong(),
    );
    ui.add_space(theme.spacing_sm);
    ui.label(
        RichText::new(summary)
            .size(theme.font_size_body)
            .color(theme.text_secondary()),
    );
}

fn draw_card_grid(
    ctx: &egui::Context,
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &mut GuiState,
    pages: &[(&'static str, GuiPage, &'static str)],
    cat_color: egui::Color32,
) {
    ui.horizontal_wrapped(|ui| {
        for (label, page, description) in pages {
            draw_page_card(ctx, ui, theme, state, label, page.clone(), description, cat_color);
        }
    });
}

fn draw_page_card(
    ctx: &egui::Context,
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &mut GuiState,
    label: &str,
    page: GuiPage,
    description: &str,
    cat_color: egui::Color32,
) {
    let card_w = 240.0;
    let card_h = 110.0;

    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(card_w, card_h),
        egui::Sense::click(),
    );
    if !ui.is_rect_visible(rect) { return; }

    let painter = ui.painter_at(rect);
    let bg = if resp.hovered() {
        let a = theme.accent();
        egui::Color32::from_rgba_unmultiplied(a.r(), a.g(), a.b(), 30)
    } else {
        theme.bg_card()
    };
    let stroke_color = if resp.hovered() { cat_color } else { theme.border() };

    painter.rect_filled(rect, egui::Rounding::same(theme.border_radius as u8), bg);
    painter.rect_stroke(
        rect,
        egui::Rounding::same(theme.border_radius as u8),
        Stroke::new(1.5, stroke_color),
        egui::StrokeKind::Inside,
    );

    // Title row
    let title_pos = rect.left_top() + egui::vec2(theme.card_padding, theme.card_padding);
    painter.text(
        title_pos,
        egui::Align2::LEFT_TOP,
        label,
        egui::FontId::proportional(theme.font_size_heading),
        theme.text_primary(),
    );

    // Description (wrapped)
    let desc_pos = rect.left_top() + egui::vec2(theme.card_padding, theme.card_padding + 28.0);
    let desc_max = card_w - theme.card_padding * 2.0;
    let galley = ctx.fonts(|f| {
        f.layout(
            description.to_string(),
            egui::FontId::proportional(theme.font_size_small),
            theme.text_secondary(),
            desc_max,
        )
    });
    painter.galley(desc_pos, galley, theme.text_secondary());

    if resp.clicked() {
        state.active_page = page;
    }
    resp.on_hover_text(format!("Open {}", label));
}
