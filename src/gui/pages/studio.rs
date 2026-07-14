//! Broadcasting Studio page — simplified OBS-like streaming control UI.
//!
//! Layout: left panel (scenes + sources), center (Program/Preview canvases + controls),
//! right panel (properties + settings). UI only; no actual WebRTC/streaming
//! implementation yet.
//!
//! PROGRAM/PREVIEW SPLIT (v0.664, OBS-style): clicking a scene stages it into
//! PREVIEW only; the PROGRAM side (what would be broadcast) does not change until
//! the "Cut to Program" button deliberately pushes preview live. Source editing
//! (position/size/visibility/add/remove) always operates on the preview working
//! set, so a streamer can rearrange safely mid-broadcast. When the center panel is
//! wide enough the two canvases render side by side (Program left, Preview right);
//! narrow windows fall back to one canvas with a Program/Preview toggle. State
//! model + transition logic live in `GuiState.studio` (`StudioState` methods
//! `select_preview_scene` / `cut_to_program`, unit-tested in gui/mod.rs).
//!
//! PERSISTENCE (operator 2026-06-07: "chat and studio should always persist load
//! ... even if I switch away from studio my stream doesn't die so I can keep playing
//! the game"): all stream state lives in `GuiState.studio` (is_live, sources,
//! settings), NOT in this draw fn, so navigating to another page or into the FPS
//! world does NOT reset or stop it — the page simply stops being drawn and resumes
//! from the same state when reopened. When real streaming/capture lands, its pump
//! MUST run in the engine loop every frame (like the chat WS client thread), never
//! gated on this page being the active one, so the broadcast keeps running while you
//! play. Chat already works this way (the WS client is a background thread that fills
//! GuiState regardless of the active page).

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Stroke, Vec2};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;

const LEFT_PANEL_WIDTH: f32 = 220.0;
const RIGHT_PANEL_WIDTH: f32 = 220.0;
// Live/REC indicator colors come from theme.success() / theme.danger() so they
// stay in sync with the rest of the UI's semantic palette.
// Panel + preview backgrounds use theme.bg_card() / theme.bg_panel() / theme.bg_sidebar_dark().
// Source-type fills + outline/label/AFK/meter colors come from the theme's
// studio_* tokens (v0.670 migration off hardcoded literals) so the Settings
// color editor can restyle the whole page.

/// Apply a runtime alpha to a theme token color. The token stores the RGB;
/// the alpha comes from runtime state (a source's opacity slider, selection
/// tint strength). Computed from a token, not hardcoded, so theme_token_lint
/// sees no literal here.
fn with_alpha(c: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha) // theme-exempt: alpha-composited from a theme token, no literal RGB
}

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
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Scenes")
                    .size(theme.font_size_heading)
                    .color(theme.text_primary()),
            );
            widgets::help_modal::help_button(ui, theme, "studio-scenes-sources", &mut state.active_help_topic);
        });
        ui.add_space(theme.section_gap);

        // Program = live output (success-colored), Preview = staged/editing (accent).
        // Clicking a scene only STAGES it into preview; "Cut to Program" in the
        // center panel is what makes it live. A scene can be both at once.
        let program_idx = state.studio.program_scene_index;
        let preview_idx = state.studio.preview_scene_index;
        let is_live = state.studio.is_live;
        let mut clicked_scene: Option<usize> = None;
        let mut delete_scene: Option<usize> = None;

        for (i, scene) in state.studio.scenes.iter().enumerate() {
            let in_program = i == program_idx;
            let in_preview = i == preview_idx;
            let bg = if in_preview {
                with_alpha(theme.accent(), 40)
            } else if in_program {
                with_alpha(theme.success(), 25)
            } else {
                Color32::TRANSPARENT
            };
            // Program's live border outranks preview's accent when a scene is both.
            let border = if in_program {
                Stroke::new(1.0, theme.success())
            } else if in_preview {
                Stroke::new(1.0, theme.accent())
            } else {
                Stroke::NONE
            };
            let text_color = if in_preview {
                theme.accent()
            } else if in_program {
                theme.text_primary()
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
                        let resp = ui
                            .selectable_label(false,
                                RichText::new(&scene.name).size(theme.font_size_body).color(text_color),
                            )
                            .on_hover_text(
                                "Click to stage this scene in Preview. What is live does not \
                                 change until you press Cut to Program.",
                            );
                        if resp.clicked() {
                            clicked_scene = Some(i);
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Delete button for custom scenes
                            if !scene.is_default {
                                if ui.small_button(RichText::new("x").color(theme.text_muted())).clicked() {
                                    delete_scene = Some(i);
                                }
                            }
                            if in_program {
                                let tag_color = if is_live { theme.success() } else { theme.text_muted() };
                                ui.label(
                                    RichText::new("PGM").size(theme.font_size_small).color(tag_color).strong(),
                                )
                                .on_hover_text("In Program: the live output side (what would be broadcast)");
                            }
                            if in_preview {
                                ui.label(
                                    RichText::new("PRE").size(theme.font_size_small).color(theme.accent()),
                                )
                                .on_hover_text("In Preview: staged for editing, viewers would not see it");
                            }
                        });
                    });
                });
        }

        if let Some(idx) = clicked_scene {
            // Stage into PREVIEW (applies the scene's source visibility to the
            // preview working set); program is untouched by design.
            state.studio.select_preview_scene(idx);
        }

        if let Some(idx) = delete_scene {
            state.studio.delete_scene(idx);
        }

        ui.add_space(theme.section_gap);
        if widgets::secondary_button(ui, theme, "+ New Scene") {
            state.studio.add_custom_scene();
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
        // The whole point of the split: source edits stage in preview, never live.
        ui.label(
            RichText::new("Edits apply to the Preview scene")
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
        ui.add_space(theme.section_gap);

        let mut selected = state.studio.selected_source_index;
        let source_count = state.studio.sources.len();

        for i in 0..source_count {
            let src = &state.studio.sources[i];
            let is_selected = selected == Some(i);
            let bg = if is_selected {
                with_alpha(theme.accent(), 25)
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

/// Render one scene canvas (each visible source as a labeled rectangle) into a
/// `size`-sized dark frame. Pure rendering over a source SLICE so the same fn
/// draws both the frozen program snapshot and the live preview working set.
/// `live_glow` adds the green live border (program pane while "live").
fn draw_scene_canvas(
    ui: &mut egui::Ui,
    theme: &Theme,
    sources: &[crate::gui::StudioSource],
    size: Vec2,
    live_glow: bool,
    empty_msg: &str,
) {
    Frame::none()
        .fill(theme.bg_sidebar_dark())
        .rounding(Rounding::same(4))
        .stroke(Stroke::new(1.0, theme.border()))
        .show(ui, |ui| {
            let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            let painter = ui.painter_at(rect);

            // Draw visible sources as labeled rectangles
            for src in sources {
                if !src.visible {
                    continue;
                }
                // Audio-only / zero-area sources (the Microphone ships at size
                // 0x0) have no visual footprint -- skip them, or their centered
                // label paints a stray "...phone" sliver at the canvas origin
                // (caught in the 2026-07-04 snapshot QA sweep).
                if src.size.0 <= 0.0 || src.size.1 <= 0.0 {
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
                // Semantic per-source-type fills from the theme's studio_*
                // tokens; the source's opacity slider caps the alpha so
                // overlapping rectangles stay readable.
                let fill_color = match &src.source_type {
                    crate::gui::StudioSourceType::Camera(_) => with_alpha(theme.studio_source_camera(), alpha.min(60)),
                    crate::gui::StudioSourceType::Screen(_) => with_alpha(theme.studio_source_screen(), alpha.min(60)),
                    crate::gui::StudioSourceType::Microphone(_) => with_alpha(theme.studio_source_microphone(), alpha.min(40)),
                    crate::gui::StudioSourceType::ChatOverlay => with_alpha(theme.studio_source_chat(), alpha.min(50)),
                    crate::gui::StudioSourceType::Image(_) => with_alpha(theme.studio_source_image(), alpha.min(50)),
                    crate::gui::StudioSourceType::Text(_) => with_alpha(theme.studio_source_text(), alpha.min(40)),
                    crate::gui::StudioSourceType::Timer => with_alpha(theme.studio_source_timer(), alpha.min(50)),
                };
                let border_color = with_alpha(theme.studio_source_border(), alpha.min(80));

                painter.rect_filled(src_rect, 2.0, fill_color);
                painter.rect_stroke(src_rect, 2.0, Stroke::new(1.0, border_color), egui::StrokeKind::Outside);

                // Source label
                painter.text(
                    src_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &src.name,
                    egui::FontId::proportional(11.0),
                    with_alpha(theme.studio_source_label(), alpha),
                );
            }

            // Placeholder message if nothing is visible in this pane
            if sources.iter().all(|s| !s.visible) {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    empty_msg,
                    egui::FontId::proportional(14.0),
                    theme.text_muted(),
                );
            }

            // Live border glow
            if live_glow {
                painter.rect_stroke(
                    rect, 4.0,
                    Stroke::new(2.0, theme.success()),
                    egui::StrokeKind::Outside,
                );
            }
        });
}

/// Label row above a canvas pane: PROGRAM (or LIVE while live) / PREVIEW plus the
/// scene name currently on that side.
fn pane_label(ui: &mut egui::Ui, theme: &Theme, state: &GuiState, program_side: bool) {
    ui.horizontal(|ui| {
        if program_side {
            if state.studio.is_live {
                ui.label(RichText::new("LIVE").size(theme.font_size_body).color(theme.success()).strong());
            } else {
                ui.label(
                    RichText::new("PROGRAM").size(theme.font_size_body).color(theme.text_secondary()).strong(),
                );
            }
            let scene_name = state
                .studio
                .scenes
                .get(state.studio.program_scene_index)
                .map(|s| s.name.as_str())
                .unwrap_or("");
            ui.label(RichText::new(scene_name).size(theme.font_size_small).color(theme.text_muted()));
        } else {
            ui.label(RichText::new("PREVIEW").size(theme.font_size_body).color(theme.accent()).strong());
            let scene_name = state
                .studio
                .scenes
                .get(state.studio.preview_scene_index)
                .map(|s| s.name.as_str())
                .unwrap_or("");
            ui.label(RichText::new(scene_name).size(theme.font_size_small).color(theme.text_muted()));
        }
    });
}

/// Minimum center-panel width for the side-by-side Program/Preview canvases;
/// below this the panel falls back to one canvas with a pane toggle.
const SPLIT_MIN_WIDTH: f32 = 660.0;

fn draw_center_panel(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // ── Stream status header ──
    ui.horizontal(|ui| {
        if state.studio.is_live {
            ui.label(RichText::new("LIVE").size(theme.font_size_body).color(theme.success()).strong());
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
        } else {
            ui.label(
                RichText::new("Offline").size(theme.font_size_body).color(theme.text_secondary()),
            );
        }
    });
    ui.add_space(theme.section_gap);

    // ── Program/Preview canvases ──
    let avail = ui.available_size();
    let controls_height = 80.0;
    let label_height = 22.0;
    let canvas_height = (avail.y - controls_height - label_height - 20.0).max(100.0);
    let program_empty_msg = "Nothing live yet: stage a scene in Preview, then Cut to Program";
    let preview_empty_msg = "No visible sources";

    // A stream canvas IS 16:9 (the 1920x1080 output): LETTERBOX it into the
    // pane instead of stretching to whatever height is left -- the panes were
    // rendering portrait, so every scene mock read as a phone stream (caught in
    // the 2026-07-04 snapshot QA sweep). Width-driven, capped by the available
    // height (shrink width to keep 16:9 when height-limited).
    let letterbox_16_9 = |max_w: f32, max_h: f32| -> Vec2 {
        let ideal_h = max_w * 9.0 / 16.0;
        if ideal_h <= max_h {
            Vec2::new(max_w, ideal_h)
        } else {
            Vec2::new(max_h * 16.0 / 9.0, max_h)
        }
    };

    if avail.x >= SPLIT_MIN_WIDTH {
        // Wide: Program (left, the live output) and Preview (right, staged) side by side.
        let gap = 8.0;
        let pane_w = ((avail.x - gap) / 2.0).max(100.0);
        let canvas_size = letterbox_16_9(pane_w, canvas_height);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            ui.vertical(|ui| {
                ui.set_width(pane_w);
                pane_label(ui, theme, state, true);
                draw_scene_canvas(
                    ui,
                    theme,
                    &state.studio.program_sources,
                    canvas_size,
                    state.studio.is_live,
                    program_empty_msg,
                );
            });
            ui.vertical(|ui| {
                ui.set_width(pane_w);
                pane_label(ui, theme, state, false);
                draw_scene_canvas(
                    ui,
                    theme,
                    &state.studio.sources,
                    canvas_size,
                    false,
                    preview_empty_msg,
                );
            });
        });
    } else {
        // Narrow: one canvas with a Program/Preview toggle in the label row.
        ui.horizontal(|ui| {
            let on_program = state.studio.focused_pane == crate::gui::StudioPane::Program;
            if ui
                .selectable_label(
                    on_program,
                    RichText::new("Program").size(theme.font_size_small),
                )
                .on_hover_text("Show the live output side")
                .clicked()
            {
                state.studio.focused_pane = crate::gui::StudioPane::Program;
            }
            if ui
                .selectable_label(
                    !on_program,
                    RichText::new("Preview").size(theme.font_size_small),
                )
                .on_hover_text("Show the staged side you are editing")
                .clicked()
            {
                state.studio.focused_pane = crate::gui::StudioPane::Preview;
            }
            pane_label(ui, theme, state, state.studio.focused_pane == crate::gui::StudioPane::Program);
        });
        let on_program = state.studio.focused_pane == crate::gui::StudioPane::Program;
        if on_program {
            draw_scene_canvas(
                ui,
                theme,
                &state.studio.program_sources,
                letterbox_16_9(avail.x, canvas_height),
                state.studio.is_live,
                program_empty_msg,
            );
        } else {
            draw_scene_canvas(
                ui,
                theme,
                &state.studio.sources,
                letterbox_16_9(avail.x, canvas_height),
                false,
                preview_empty_msg,
            );
        }
    }

    ui.add_space(theme.panel_margin);

    // ── Controls bar ──
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;

        // Cut to Program — THE deliberate transition. Scene clicks only stage into
        // preview; nothing changes on the live side until this button pushes the
        // staged preview to program (a hard cut; fades are a later add).
        if widgets::Button::primary("Cut to Program")
            .tooltip(
                "Make the Preview scene the live Program output. Rehearsal mode: \
                 no video/audio is actually broadcast yet.",
            )
            .show(ui, theme)
        {
            state.studio.cut_to_program();
        }
        widgets::help_modal::help_button(ui, theme, "studio-program-preview", &mut state.active_help_topic);

        ui.add_space(theme.panel_margin);
        ui.separator();
        ui.add_space(theme.panel_margin);

        // Go Live / LIVE button — Success variant when live (green), Primary when not.
        // Real capture/encoding/transport isn't built yet (STATUS.md TIER 2 gap) --
        // this drives scene/source rehearsal state only, so both labels carry an
        // honest tooltip rather than implying a real broadcast is happening.
        if state.studio.is_live {
            // Indicator only — clicking does nothing (use Stop to end stream).
            widgets::Button::success("LIVE")
                .tooltip("Rehearsal mode only -- no video/audio is actually being sent anywhere yet.")
                .show(ui, theme);
        } else if widgets::Button::primary("Go Live")
            .tooltip("Rehearsal mode: lets you practice scenes/sources. Streaming isn't connected to a real broadcast yet.")
            .show(ui, theme)
        {
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
                    // AFK is itself the deliberate action (OBS-hotkey style): stage
                    // BRB and cut it straight to program so the audience-facing side
                    // flips to BRB in the same click.
                    state.studio.select_preview_scene(brb_idx);
                    state.studio.cut_to_program();
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
                    .color(theme.studio_afk()),
            );
            ui.ctx().request_repaint(); // keep timer updating
        }

        ui.add_space(theme.panel_margin);
        ui.separator();
        ui.add_space(theme.panel_margin);

        // Audio level meter -- the REAL mic peak (crate::net::voice::mic_level()), the
        // same reader the voice-chat mic test uses. It reads 0 unless a mic capture
        // stream is actually running (a mic test or a live voice session), which is
        // honest: Studio itself doesn't open the mic, so silence here means nothing is
        // capturing yet, not that the meter is broken.
        ui.label(RichText::new("Audio:").size(theme.font_size_small).color(theme.text_muted()));
        let (meter_rect, _) = ui.allocate_exact_size(Vec2::new(80.0, 12.0), egui::Sense::hover());
        let painter = ui.painter_at(meter_rect);
        painter.rect_filled(meter_rect, 2.0, theme.studio_meter_bg());
        let level = crate::net::voice::mic_level();
        if crate::net::voice::mic_test_running() || crate::net::voice::voice_session_running() {
            ui.ctx().request_repaint();
        }
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

        // Connection status. No real transport exists yet (see the Go Live tooltip
        // above), so this must not claim a live bitrate/drop count that was never
        // measured -- it previously showed a hardcoded "0 dropped | 3500 kbps" that
        // looked like a real stat regardless of whether anything was connected.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if state.studio.is_live {
                ui.label(
                    RichText::new("Rehearsing (not broadcasting)")
                        .size(theme.font_size_small)
                        .color(theme.warning()),
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

    // Live chat panel (v0.850): read the stream's chat right inside Studio, with a
    // channel selector, so you can watch #general while broadcasting.
    draw_studio_chat(ui, theme, state);
}

/// Read-only live chat inside Studio + a channel selector. Reuses the same
/// message stream as the Chat page (`state.chat_messages` = the active channel's
/// messages); picking a channel here switches the active channel and refetches.
fn draw_studio_chat(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.add_space(theme.spacing_sm);
    ui.separator();
    ui.add_space(theme.spacing_sm);

    ui.horizontal(|ui| {
        ui.label(RichText::new("Live Chat").size(theme.font_size_heading).color(theme.text_primary()));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let current = state.chat_active_channel.clone();
            let mut picked: Option<String> = None;
            egui::ComboBox::from_id_salt("studio_chat_channel")
                .selected_text(format!("#{}", current.trim_start_matches('#')))
                .show_ui(ui, |ui| {
                    for ch in &state.chat_channels {
                        let sel = ch.id == current;
                        if ui.selectable_label(sel, format!("#{}", ch.name.trim_start_matches('#'))).clicked() {
                            picked = Some(ch.id.clone());
                        }
                    }
                });
            ui.label(RichText::new("Channel").size(theme.font_size_small).color(theme.text_muted()));
            if let Some(id) = picked {
                if id != state.chat_active_channel {
                    state.chat_active_channel = id;
                    state.chat_messages.clear();
                    state.history_fetched = false;
                }
            }
        });
    });
    ui.add_space(theme.spacing_xs);

    let connected = state.ws_client.as_ref().map_or(false, |c| c.is_connected());
    if !connected {
        ui.label(
            RichText::new("Connect to a relay on the Chat page to see live messages here.")
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
        return;
    }

    egui::ScrollArea::vertical()
        .id_salt("studio_chat_scroll")
        .stick_to_bottom(true)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if state.chat_messages.is_empty() {
                ui.add_space(theme.spacing_sm);
                ui.label(
                    RichText::new("No messages yet in this channel.")
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
            }
            let start = state.chat_messages.len().saturating_sub(200);
            for msg in &state.chat_messages[start..] {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 5.0;
                    ui.label(
                        RichText::new(&msg.sender_name)
                            .size(theme.font_size_small)
                            .color(crate::gui::pages::chat::name_color(&msg.sender_name))
                            .strong(),
                    );
                    ui.label(
                        RichText::new(&msg.content)
                            .size(theme.font_size_small)
                            .color(theme.text_secondary()),
                    );
                });
            }
        });
}

// ── Right Panel ─────────────────────────────────────────────────────────────

fn draw_right_panel(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ScrollArea::vertical().id_salt("studio_right_scroll").show(ui, |ui| {
        // ── Your public channel ──
        // Moved from the Profile page (operator 2026-06-07: "streaming should be
        // part of studio"). These are PUBLIC profile fields: the channel link people
        // follow + a manual LIVE flag shown on your public profile. Distinct from the
        // broadcast controls in the center panel (which drive is_live).
        ui.label(
            RichText::new("Your Public Channel")
                .size(theme.font_size_heading)
                .color(theme.text_primary()),
        );
        ui.add_space(theme.section_gap);
        ui.label(RichText::new("Channel URL").size(theme.font_size_small).color(theme.text_secondary()));
        ui.add(
            egui::TextEdit::singleline(&mut state.profile_streaming_url)
                .desired_width(190.0)
                .hint_text("https://..."),
        );
        ui.add_space(theme.section_gap);
        widgets::toggle(ui, theme, "Show LIVE on profile", &mut state.profile_streaming_live);
        ui.add_space(theme.card_padding);
        ui.separator();
        ui.add_space(theme.panel_margin);

        // ── Source Properties ──
        ui.label(
            RichText::new("Source Properties")
                .size(theme.font_size_heading)
                .color(theme.text_primary()),
        );
        // These sliders edit the PREVIEW working set; the live program layout only
        // changes when Cut to Program pushes the staged preview live.
        ui.label(
            RichText::new("Edits apply to the Preview scene")
                .size(theme.font_size_small)
                .color(theme.text_muted()),
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
        ui.horizontal(|ui| {
            ui.label(RichText::new("Resolution").size(theme.font_size_small).color(theme.text_secondary()));
            widgets::help_modal::help_button(ui, theme, "studio-stream-settings", &mut state.active_help_topic);
        });
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
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Chat Overlay")
                    .size(theme.font_size_heading)
                    .color(theme.text_primary()),
            );
            widgets::help_modal::help_button(ui, theme, "studio-chat-overlay", &mut state.active_help_topic);
        });
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
