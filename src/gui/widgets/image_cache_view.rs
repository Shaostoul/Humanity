//! Full-screen image viewer for chat attachments.
//!
//! Pairs with `image_cache`. When `gui_state.image_viewer_url` is `Some`,
//! this module draws a full-screen overlay with the decoded image centred,
//! a close button, and a "Download" button that writes the raw bytes to
//! the `downloads/` folder next to the executable.

use egui::{Align2, Color32, Context, Frame, RichText, Rounding, Sense, Stroke, Vec2};

use crate::gui::theme::Theme;
use crate::gui::GuiState;
use super::image_cache::{default_downloads_dir, filename_from_url, ImageStatus};

pub fn draw(ctx: &Context, theme: &Theme, state: &mut GuiState) {
    let url = match state.image_viewer_url.clone() {
        Some(u) => u,
        None => return,
    };

    let mut should_close = false;

    // Backdrop: darker than the help modal to emphasise the photo.
    let screen = ctx.screen_rect();
    egui::Area::new(egui::Id::new("hos_image_viewer_backdrop"))
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (_, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(screen, Rounding::ZERO, Color32::from_rgba_unmultiplied(0, 0, 0, 230));
            if resp.clicked() {
                should_close = true;
            }
        });

    // Image + controls window
    egui::Window::new("Image")
        .id(egui::Id::new("hos_image_viewer_window"))
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .frame(
            Frame::none()
                .fill(theme.bg_card())
                .rounding(Rounding::same(theme.border_radius_lg as u8))
                .inner_margin(theme.card_padding)
                .stroke(Stroke::new(1.0, theme.border())),
        )
        .show(ctx, |ui| {
            ui.set_max_width(screen.width() * 0.9);
            ui.set_max_height(screen.height() * 0.9);

            // Status-driven body
            let status = state.image_cache.status(&url);
            match status {
                ImageStatus::Ready { width, height } => {
                    let max_w = (screen.width() * 0.85).min(1600.0);
                    let max_h = (screen.height() * 0.8).min(1200.0);
                    let aspect = width.max(1) as f32 / height.max(1) as f32;
                    let (mut w, mut h) = (max_w, max_w / aspect);
                    if h > max_h {
                        h = max_h;
                        w = max_h * aspect;
                    }
                    if let Some(tex) = state.image_cache.get_texture(&url) {
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
                        let mut mesh = egui::Mesh::with_texture(tex.id());
                        mesh.add_rect_with_uv(
                            rect,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );
                        ui.painter().add(egui::Shape::mesh(mesh));
                    }
                    ui.add_space(theme.spacing_sm);
                    ui.label(
                        RichText::new(format!("{} × {} px", width, height))
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                }
                ImageStatus::Fetching | ImageStatus::Idle => {
                    ui.set_min_size(Vec2::new(360.0, 120.0));
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(
                            RichText::new("Loading…")
                                .size(theme.font_size_body)
                                .color(theme.text_secondary()),
                        );
                        ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
                    });
                }
                ImageStatus::Failed(err) => {
                    ui.set_min_size(Vec2::new(420.0, 140.0));
                    ui.vertical_centered(|ui| {
                        ui.add_space(24.0);
                        ui.label(
                            RichText::new("Could not load image")
                                .size(theme.font_size_heading)
                                .color(theme.danger())
                                .strong(),
                        );
                        ui.label(
                            RichText::new(&err)
                                .size(theme.font_size_small)
                                .color(theme.text_muted()),
                        );
                    });
                }
            }

            ui.add_space(theme.spacing_md);
            ui.separator();
            ui.add_space(theme.spacing_sm);

            // Actions: URL, Download, Close
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(&url)
                        .size(theme.font_size_small)
                        .color(theme.text_muted())
                        .monospace(),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let close_btn = egui::Button::new(
                        RichText::new("Close").size(theme.font_size_body).color(theme.text_primary()),
                    )
                    .fill(Color32::TRANSPARENT)
                    .stroke(Stroke::new(1.0, theme.border()))
                    .rounding(Rounding::same(theme.border_radius as u8));
                    if ui.add(close_btn).clicked() {
                        should_close = true;
                    }

                    let dl_btn = egui::Button::new(
                        RichText::new("Download")
                            .size(theme.font_size_body)
                            .color(theme.text_on_accent()),
                    )
                    .fill(theme.accent())
                    .rounding(Rounding::same(theme.border_radius as u8));
                    if ui.add(dl_btn).clicked() {
                        let dest = default_downloads_dir().join(filename_from_url(&url));
                        state.image_cache.download(&url, dest);
                    }

                    // If this URL was already downloaded, show the saved path.
                    if let Some(path) = state.image_cache.downloaded_path(&url) {
                        ui.label(
                            RichText::new(format!("Saved to {}", path.display()))
                                .size(theme.font_size_small)
                                .color(theme.success()),
                        );
                    }

                    let copy_btn = egui::Button::new(
                        RichText::new("Copy URL").size(theme.font_size_body).color(theme.text_primary()),
                    )
                    .fill(Color32::TRANSPARENT)
                    .stroke(Stroke::new(1.0, theme.border()))
                    .rounding(Rounding::same(theme.border_radius as u8));
                    if ui.add(copy_btn).clicked() {
                        ui.ctx().copy_text(url.clone());
                    }
                });
            });
        });

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        should_close = true;
    }

    if should_close {
        state.image_viewer_url = None;
    }
}
