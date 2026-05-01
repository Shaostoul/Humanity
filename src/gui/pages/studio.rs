//! Broadcasting Studio page — simplified OBS-like streaming control UI.
//!
//! Layout: left panel (scenes + sources), center (preview + controls), right panel (properties + settings).
//! UI only; no actual WebRTC/streaming implementation yet.

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Stroke, Vec2};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;

const LEFT_PANEL_WIDTH: f32 = 220.0;
const RIGHT_PANEL_WIDTH: f32 = 220.0;
// Live/REC indicator colors come from theme.success() / theme.danger() so they
// stay in sync with the rest of the UI's semantic palette.
// Panel + preview backgrounds use theme.bg_card() / theme.bg_panel() / theme.bg_sidebar_dark().

// Platform/resolution/fps/position picker options live in
// data/studio/streaming_config.json — load them via state.studio_streaming_config.

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // Left panel: scenes + sources
    egui::SidePanel::left("studio_left_panel")
        .exact_width(LEFT_PANEL_WIDTH)
        .frame(Frame::none().fill(theme.bg_card()).inner_margin(theme.panel_margin))
        .show(ctx, |ui| {
            draw_left_panel(ui, theme, state);
        });

    // Right panel: properties + stream settings
    egui::SidePanel::right("studio_right_panel")
        .exact_width(RIGHT_PANEL_WIDTH)
        .frame(Frame::none().fill(theme.bg_card()).inner_margin(theme.panel_margin))
        .show(ctx, |ui| {
            draw_right_panel(ui, theme, state);
        });

    // Center panel: preview + controls
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_primary()).inner_margin(theme.panel_margin))
        .show(ctx, |ui| {
            draw_center_panel(ui, theme, state);
        });
}

// ── Left Panel ──────────────────────────────────────────────────────────────

fn draw_left_panel(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ScrollArea::vertical().id_salt("studio_left_scroll").show(ui, |ui| {
        // ── Scenes ──
        ui.label(
            RichText::new("Scenes")
                .size(theme.font_size_heading)
                .color(theme.text_primary()),
        );
        ui.add_space(theme.section_gap);

        let active_scene = state.studio.active_scene_index;
        let mut clicked_scene: Option<usize> = None;
        let mut delete_scene: Option<usize> = None;

        for (i, scene) in state.studio.scenes.iter().enumerate() {
            let is_active = i == active_scene;
            let bg = if is_active {
                let a = theme.accent();
                Color32::from_rgba_unmultiplied(a.r(), a.g(), a.b(), 40)
            } else {
                Color32::TRANSPARENT
            };
            let border = if is_active {
                Stroke::new(1.0, theme.accent())
            } else {
                Stroke::NONE
            };
            let text_color = if is_active {
                theme.accent()
            } else {
                theme.text_secondary()
            };

            Frame::none()
                .fill(bg)
                .stroke(border)
                .rounding(Rounding::same(4))
                .inner_margin(Vec2::new(6.0, 3.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let resp = ui.selectable_label(false,
                            RichText::new(&scene.name).size(theme.font_size_body).color(text_color),
                        );
                        if resp.clicked() {
                            clicked_scene = Some(i);
                        }
                        // Delete button for custom scenes
                        if !scene.is_default {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button(RichText::new("x").color(theme.text_muted())).clicked() {
                                    delete_scene = Some(i);
                                }
                            });
                        }
                    });
                });
        }

        if let Some(idx) = clicked_scene {
            state.studio.active_scene_index = idx;
            // Apply scene source visibility
            let scene_vis = state.studio.scenes[idx].source_visibility.clone();
            for (j, src) in state.studio.sources.iter_mut().enumerate() {
                if let Some(&vis) = scene_vis.get(j) {
                    src.visible = vis;
                }
            }
        }

        if let Some(idx) = delete_scene {
            if !state.studio.scenes[idx].is_default {
                state.studio.scenes.remove(idx);
                if state.studio.active_scene_index >= state.studio.scenes.len() {
                    state.studio.active_scene_index = 0;
                }
            }
        }

        ui.add_space(theme.section_gap);
        if widgets::secondary_button(ui, theme, "+ New Scene") {
            let idx = state.studio.scenes.len();
            let vis = state.studio.sources.iter().map(|s| s.visible).collect();
            state.studio.scenes.push(crate::gui::StudioScene {
                name: format!("Custom {}", idx + 1),
                is_default: false,
                source_visibility: vis,
            });
        }

        ui.add_space(theme.card_padding);
        ui.separator();
        ui.add_space(theme.panel_margin);

        // ── Sources ──
        ui.label(
            RichText::new("Sources")
                .size(theme.font_size_heading)
                .color(theme.text_primary()),
        );
        ui.add_space(theme.section_gap);

        let mut selected = state.studio.selected_source_index;
        let source_count = state.studio.sources.len();

        for i in 0..source_count {
            let src = &state.studio.sources[i];
            let is_selected = selected == Some(i);
            let bg = if is_selected {
                let a = theme.accent();
                Color32::from_rgba_unmultiplied(a.r(), a.g(), a.b(), 25)
            } else {
                Color32::TRANSPARENT
            };

            let name = src.name.clone();
            let visible = src.visible;
            let type_icon = match &src.source_type {
                crate::gui::StudioSourceType::Camera(_) => "[CAM]",
                crate::gui::StudioSourceType::Screen(_) => "[SCR]",
                crate::gui::StudioSourceType::Microphone(_) => "[MIC]",
                crate::gui::StudioSourceType::ChatOverlay => "[CHAT]",
                crate::gui::StudioSourceType::Image(_) => "[IMG]",
                crate::gui::StudioSourceType::Text(_) => "[TXT]",
                crate::gui::StudioSourceType::Timer => "[TMR]",
            };

            Frame::none()
                .fill(bg)
                .rounding(Rounding::same(3))
                .inner_margin(Vec2::new(4.0, 2.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Visibility checkbox
                        let mut vis = visible;
                        if ui.checkbox(&mut vis, "").changed() {
                            state.studio.sources[i].visible = vis;
                        }
                        // Type icon + name (clickable to select)
                        let label = format!("{} {}", type_icon, name);
                        let text_color = if visible {
                            theme.text_primary()
                        } else {
                            theme.text_muted()
                        };
                        let resp = ui.selectable_label(
                            false,
                            RichText::new(label).size(theme.font_size_small).color(text_color),
                        );
                        if resp.clicked() {
                            selected = Some(i);
                        }
                    });
                });
        }
        state.studio.selected_source_index = selected;

        // Z-order up/down buttons for selected source
        if let Some(sel) = selected {
            ui.add_space(theme.section_gap);
            ui.horizontal(|ui| {
                if sel > 0 {
                    if ui.small_button(RichText::new("Up").size(theme.font_size_small)).clicked() {
                        state.studio.sources.swap(sel, sel - 1);
                        state.studio.selected_source_index = Some(sel - 1);
                    }
                }
                if sel + 1 < state.studio.sources.len() {
                    if ui.small_button(RichText::new("Down").size(theme.font_size_small)).clicked() {
                        state.studio.sources.swap(sel, sel + 1);
                        state.studio.selected_source_index = Some(sel + 1);
                    }
                }
            });
        }

        ui.add_space(theme.section_gap);
        if widgets::secondary_button(ui, theme, "+ Add Source") {
            let idx = state.studio.sources.len() as u32;
            state.studio.sources.push(crate::gui::StudioSource {
                name: format!("Source {}", idx + 1),
                source_type: crate::gui::StudioSourceType::Text("New Source".to_string()),
                visible: true,
                position: (0.1, 0.1),
                size: (0.3, 0.3),
                opacity: 1.0,
                z_order: idx,
            });
        }
    });
}

// ── Center Panel ────────────────────────────────────────────────────────────

fn draw_center_panel(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let avail = ui.available_size();

    // ── Preview/Live header ──
    ui.horizontal(|ui| {
        let preview_text = if state.studio.is_live {
            RichText::new("LIVE").size(theme.font_size_body).color(theme.success()).strong()
        } else {
            RichText::new("PREVIEW").size(theme.font_size_body).color(theme.text_secondary())
        };
        ui.label(preview_text);

        if state.studio.is_live {
            // Elapsed time indicator
            let elapsed = ui.ctx().input(|i| i.time) - state.studio.live_start_time;
            let secs = elapsed as u64;
            let h = secs / 3600;
            let m = (secs % 3600) / 60;
            let s = secs % 60;
            ui.label(
                RichText::new(format!("{}:{:02}:{:02}", h, m, s))
                    .size(theme.font_size_small)
                    .color(theme.success()),
            );
        }
    });
    ui.add_space(theme.section_gap);

    // ── Preview area ──
    let controls_height = 80.0;
    let preview_height = (avail.y - controls_height - 20.0).max(100.0);
    let preview_width = avail.x;

    Frame::none()
        .fill(theme.bg_sidebar_dark())
        .rounding(Rounding::same(4))
        .stroke(Stroke::new(1.0, theme.border()))
        .show(ui, |ui| {
            let (rect, _) = ui.allocate_exact_size(
                Vec2::new(preview_width, preview_height),
                egui::Sense::hover(),
            );

            let painter = ui.painter_at(rect);

            // Draw visible sources as labeled rectangles
            for src in &state.studio.sources {
                if !src.visible {
                    continue;
                }
                let x = rect.min.x + src.position.0 * rect.width();
                let y = rect.min.y + src.position.1 * rect.height();
                let w = src.size.0 * rect.width();
                let h = src.size.1 * rect.height();
                let src_rect = egui::Rect::from_min_size(
                    egui::pos2(x, y),
                    Vec2::new(w, h),
                );

                let alpha = (src.opacity * 255.0) as u8;
                let fill_color = match &src.source_type {
                    crate::gui::StudioSourceType::Camera(_) => Color32::from_rgba_unmultiplied(46, 134, 193, alpha.min(60)),
                    crate::gui::StudioSourceType::Screen(_) => Color32::from_rgba_unmultiplied(142, 68, 173, alpha.min(60)),
                    crate::gui::StudioSourceType::Microphone(_) => Color32::from_rgba_unmultiplied(231, 76, 60, alpha.min(40)),
                    crate::gui::StudioSourceType::ChatOverlay => Color32::from_rgba_unmultiplied(46, 204, 113, alpha.min(50)),
                    crate::gui::StudioSourceType::Image(_) => Color32::from_rgba_unmultiplied(241, 196, 15, alpha.min(50)),
                    crate::gui::StudioSourceType::Text(_) => Color32::from_rgba_unmultiplied(236, 240, 241, alpha.min(40)),
                    crate::gui::StudioSourceType::Timer => Color32::from_rgba_unmultiplied(230, 126, 34, alpha.min(50)),
                };
                let border_color = Color32::from_rgba_unmultiplied(255, 255, 255, alpha.min(80));

                painter.rect_filled(src_rect, 2.0, fill_color);
                painter.rect_stroke(src_rect, 2.0, Stroke::new(1.0, border_color), egui::StrokeKind::Outside);

                // Source label
                painter.text(
                    src_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &src.name,
                    egui::FontId::proportional(11.0),
                    Color32::from_rgba_unmultiplied(255, 255, 255, alpha),
                );
            }

            // "No sources visible" message if all hidden
            if state.studio.sources.iter().all(|s| !s.visible) {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "No visible sources",
                    egui::FontId::proportional(14.0),
                    theme.text_muted(),
                );
            }

            // Live border glow
            if state.studio.is_live {
                painter.rect_stroke(
                    rect, 4.0,
                    Stroke::new(2.0, theme.success()),
                    egui::StrokeKind::Outside,
                );
            }
        });

    ui.add_space(theme.panel_margin);

    // ── Controls bar ──
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;

        // Go Live / LIVE button — Success variant when live (green), Primary when not.
        if state.studio.is_live {
            // Indicator only — clicking does nothing (use Stop to end stream).
            widgets::Button::success("LIVE").show(ui, theme);
        } else if widgets::Button::primary("Go Live").show(ui, theme) {
            state.studio.is_live = true;
            state.studio.is_paused = false;
            state.studio.live_start_time = ui.ctx().input(|i| i.time);
        }

        // Pause / Resume — Secondary that flips to accent fill via .active() when paused.
        let pause_label = if state.studio.is_paused { "Resume" } else { "Pause" };
        if widgets::Button::secondary(pause_label)
            .active(state.studio.is_paused)
            .show(ui, theme)
        {
            state.studio.is_paused = !state.studio.is_paused;
        }

        // Stop — Danger variant.
        if widgets::Button::danger("Stop").show(ui, theme) {
            state.studio.is_live = false;
            state.studio.is_paused = false;
            state.studio.is_afk = false;
        }

        ui.add_space(theme.panel_margin);
        ui.separator();
        ui.add_space(theme.panel_margin);

        // AFK toggle — Secondary that flips to accent when active.
        if widgets::Button::secondary("AFK")
            .active(state.studio.is_afk)
            .show(ui, theme)
        {
            state.studio.is_afk = !state.studio.is_afk;
            if state.studio.is_afk {
                state.studio.afk_start_time = ui.ctx().input(|i| i.time);
                state.studio.is_paused = true;
                if let Some(brb_idx) = state.studio.scenes.iter().position(|s| s.name == "BRB") {
                    state.studio.active_scene_index = brb_idx;
                }
            } else {
                state.studio.is_paused = false;
            }
        }

        // BRB — same toggle state as AFK, different label.
        if widgets::Button::secondary("BRB")
            .active(state.studio.is_afk)
            .show(ui, theme)
        {
            state.studio.is_afk = !state.studio.is_afk;
            if state.studio.is_afk {
                state.studio.afk_start_time = ui.ctx().input(|i| i.time);
                state.studio.is_paused = true;
            } else {
                state.studio.is_paused = false;
            }
        }

        // AFK timer display
        if state.studio.is_afk {
            let elapsed = ui.ctx().input(|i| i.time) - state.studio.afk_start_time;
            let secs = elapsed as u64;
            let h = secs / 3600;
            let m = (secs % 3600) / 60;
            let s = secs % 60;
            ui.label(
                RichText::new(format!("Away: {}:{:02}:{:02}", h, m, s))
                    .size(theme.font_size_small)
                    .color(Color32::from_rgb(155, 89, 182)),
            );
            ui.ctx().request_repaint(); // keep timer updating
        }

        ui.add_space(theme.panel_margin);
        ui.separator();
        ui.add_space(theme.panel_margin);

        // Audio level meter (placeholder bar)
        ui.label(RichText::new("Audio:").size(theme.font_size_small).color(theme.text_muted()));
        let (meter_rect, _) = ui.allocate_exact_size(Vec2::new(80.0, 12.0), egui::Sense::hover());
        let painter = ui.painter_at(meter_rect);
        painter.rect_filled(meter_rect, 2.0, Color32::from_rgb(30, 30, 40));
        // Simulated level (static placeholder)
        let level = 0.4_f32;
        let fill_rect = egui::Rect::from_min_size(
            meter_rect.min,
            Vec2::new(meter_rect.width() * level, meter_rect.height()),
        );
        let meter_color = if level < 0.6 {
            theme.success()
        } else if level < 0.85 {
            theme.warning()
        } else {
            theme.danger()
        };
        painter.rect_filled(fill_rect, 2.0, meter_color);

        // Connection status placeholder
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if state.studio.is_live {
                ui.label(
                    RichText::new("0 dropped | 3500 kbps")
                        .size(theme.font_size_small)
                        .color(theme.success()),
                );
            } else {
                ui.label(
                    RichText::new("Offline")
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
            }
        });
    });
}

// ── Right Panel ─────────────────────────────────────────────────────────────

fn draw_right_panel(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ScrollArea::vertical().id_salt("studio_right_scroll").show(ui, |ui| {
        // ── Source Properties ──
        ui.label(
            RichText::new("Source Properties")
                .size(theme.font_size_heading)
                .color(theme.text_primary()),
        );
        ui.add_space(theme.section_gap);

        if let Some(sel) = state.studio.selected_source_index {
            if sel < state.studio.sources.len() {
                let src = &mut state.studio.sources[sel];

                ui.label(
                    RichText::new(&src.name)
                        .size(theme.font_size_body)
                        .color(theme.accent()),
                );
                ui.add_space(theme.section_gap);

                // Position
                ui.label(RichText::new("Position").size(theme.font_size_small).color(theme.text_secondary()));
                ui.horizontal(|ui| {
                    ui.label(RichText::new("X:").size(theme.font_size_small).color(theme.text_muted()));
                    ui.add(egui::Slider::new(&mut src.position.0, 0.0..=1.0).step_by(0.01).show_value(true));
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Y:").size(theme.font_size_small).color(theme.text_muted()));
                    ui.add(egui::Slider::new(&mut src.position.1, 0.0..=1.0).step_by(0.01).show_value(true));
                });

                ui.add_space(theme.section_gap);

                // Size
                ui.label(RichText::new("Size").size(theme.font_size_small).color(theme.text_secondary()));
                ui.horizontal(|ui| {
                    ui.label(RichText::new("W:").size(theme.font_size_small).color(theme.text_muted()));
                    ui.add(egui::Slider::new(&mut src.size.0, 0.05..=1.0).step_by(0.01).show_value(true));
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("H:").size(theme.font_size_small).color(theme.text_muted()));
                    ui.add(egui::Slider::new(&mut src.size.1, 0.05..=1.0).step_by(0.01).show_value(true));
                });

                ui.add_space(theme.section_gap);

                // Opacity
                ui.label(RichText::new("Opacity").size(theme.font_size_small).color(theme.text_secondary()));
                ui.add(egui::Slider::new(&mut src.opacity, 0.0..=1.0).step_by(0.01).show_value(true));

                ui.add_space(theme.section_gap);

                // Visibility toggle
                ui.checkbox(&mut src.visible, RichText::new("Visible").size(theme.font_size_small).color(theme.text_secondary()));

                ui.add_space(theme.panel_margin);

                // Remove source button
                if widgets::danger_button(ui, theme, "Remove Source") {
                    state.studio.sources.remove(sel);
                    state.studio.selected_source_index = None;
                }
            } else {
                state.studio.selected_source_index = None;
                ui.label(
                    RichText::new("No source selected")
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
            }
        } else {
            ui.label(
                RichText::new("No source selected")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
        }

        ui.add_space(theme.card_padding);
        ui.separator();
        ui.add_space(theme.panel_margin);

        // ── Stream Settings ──
        ui.label(
            RichText::new("Stream Settings")
                .size(theme.font_size_heading)
                .color(theme.text_primary()),
        );
        ui.add_space(theme.section_gap);

        // Platform selector
        ui.label(RichText::new("Platform").size(theme.font_size_small).color(theme.text_secondary()));
        egui::ComboBox::from_id_salt("studio_platform")
            .selected_text(&state.studio.stream_platform)
            .width(190.0)
            .show_ui(ui, |ui| {
                for p in &state.studio_streaming_config.platforms {
                    ui.selectable_value(&mut state.studio.stream_platform, p.clone(), p);
                }
            });

        // Stream key (hidden for HumanityOS)
        if state.studio.stream_platform != "HumanityOS Server" {
            ui.add_space(theme.section_gap);
            ui.label(RichText::new("Stream Key").size(theme.font_size_small).color(theme.text_secondary()));
            ui.add(
                egui::TextEdit::singleline(&mut state.studio.stream_key)
                    .password(true)
                    .desired_width(190.0)
                    .hint_text("Enter stream key..."),
            );
        }

        // Server URL (for HumanityOS)
        if state.studio.stream_platform == "HumanityOS Server" {
            ui.add_space(theme.section_gap);
            ui.label(RichText::new("Server URL").size(theme.font_size_small).color(theme.text_secondary()));
            ui.add(
                egui::TextEdit::singleline(&mut state.studio.stream_server_url)
                    .desired_width(190.0)
                    .hint_text("wss://..."),
            );
        }

        ui.add_space(theme.section_gap);

        // Resolution
        ui.label(RichText::new("Resolution").size(theme.font_size_small).color(theme.text_secondary()));
        egui::ComboBox::from_id_salt("studio_resolution")
            .selected_text(&state.studio.stream_resolution)
            .width(190.0)
            .show_ui(ui, |ui| {
                for r in &state.studio_streaming_config.resolutions {
                    ui.selectable_value(&mut state.studio.stream_resolution, r.clone(), r);
                }
            });

        ui.add_space(theme.section_gap);

        // Bitrate
        ui.label(RichText::new("Bitrate (kbps)").size(theme.font_size_small).color(theme.text_secondary()));
        ui.add(egui::Slider::new(&mut state.studio.stream_bitrate, 1000..=10000).step_by(100.0).show_value(true));

        ui.add_space(theme.section_gap);

        // FPS
        ui.label(RichText::new("FPS").size(theme.font_size_small).color(theme.text_secondary()));
        egui::ComboBox::from_id_salt("studio_fps")
            .selected_text(format!("{}", state.studio.stream_fps))
            .width(190.0)
            .show_ui(ui, |ui| {
                for &f in &state.studio_streaming_config.fps {
                    ui.selectable_value(&mut state.studio.stream_fps, f, format!("{}", f));
                }
            });

        ui.add_space(theme.card_padding);
        ui.separator();
        ui.add_space(theme.panel_margin);

        // ── Chat Overlay Settings ──
        ui.label(
            RichText::new("Chat Overlay")
                .size(theme.font_size_heading)
                .color(theme.text_primary()),
        );
        ui.add_space(theme.section_gap);

        ui.label(RichText::new("Channel").size(theme.font_size_small).color(theme.text_secondary()));
        ui.add(
            egui::TextEdit::singleline(&mut state.studio.chat_overlay_channel)
                .desired_width(190.0)
                .hint_text("general"),
        );

        ui.add_space(theme.section_gap);

        ui.label(RichText::new("Font Size").size(theme.font_size_small).color(theme.text_secondary()));
        ui.add(egui::Slider::new(&mut state.studio.chat_overlay_font_size, 8.0..=32.0).step_by(1.0).show_value(true));

        ui.add_space(theme.section_gap);

        ui.label(RichText::new("Position").size(theme.font_size_small).color(theme.text_secondary()));
        egui::ComboBox::from_id_salt("studio_chat_pos")
            .selected_text(&state.studio.chat_overlay_position)
            .width(190.0)
            .show_ui(ui, |ui| {
                for pos in &state.studio_streaming_config.chat_positions {
                    ui.selectable_value(&mut state.studio.chat_overlay_position, pos.clone(), pos);
                }
            });

        ui.add_space(theme.section_gap);

        ui.label(RichText::new("Opacity").size(theme.font_size_small).color(theme.text_secondary()));
        ui.add(egui::Slider::new(&mut state.studio.chat_overlay_opacity, 0.0..=1.0).step_by(0.05).show_value(true));

        ui.add_space(theme.section_gap);

        ui.label(RichText::new("Max Messages").size(theme.font_size_small).color(theme.text_secondary()));
        ui.add(egui::Slider::new(&mut state.studio.chat_overlay_max_messages, 1..=50).show_value(true));

        ui.add_space(theme.section_gap);

        ui.label(RichText::new("Background Opacity").size(theme.font_size_small).color(theme.text_secondary()));
        ui.add(egui::Slider::new(&mut state.studio.chat_overlay_bg_opacity, 0.0..=1.0).step_by(0.05).show_value(true));
    });
}
