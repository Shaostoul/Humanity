//! Reusable egui widgets styled by the HumanityOS theme.
//!
//! **For buttons, prefer `widgets::button::Button` (builder API).** The free
//! functions below (`primary_button`, `secondary_button`, `danger_button`)
//! are kept as thin wrappers for existing call sites.

pub mod button;
pub mod row;
pub mod icons;
pub mod search_bar;
pub mod help_modal;
pub mod passphrase_modal;
pub mod image_cache;
pub mod image_cache_view;
pub mod form_row;
pub mod alert;
pub mod dialog;
pub mod tree;
pub mod body_pill;
pub mod markdown;
pub mod msg_format;
pub mod file_browser;

use egui::{Color32, Rect, RichText, Rounding, Sense, Stroke, Ui, Vec2};
use super::theme::Theme;

// Re-export the universal button builder + variant enums so call sites only
// need `use crate::gui::widgets;` and then write `widgets::Button::primary(…)`.
pub use button::{Button, ButtonSize, ButtonVariant};

// Legacy free-function aliases now delegate to the universal Button builder.
// New code should prefer `Button::primary(label).show(ui, theme)`.
pub use button::{primary_button, secondary_button, danger_button, compact_button};

// Convenience re-exports for the common widget surface so call sites
// can write `widgets::form_row(...)`, `widgets::alert(...)`, `widgets::dialog(...)`.
pub use form_row::{form_row, form_row_wide, form_row_with_help};
pub use alert::{alert, alert_with_title, AlertKind};
pub use dialog::{dialog, dialog_anchored};
pub use tree::{tree_node, tree_leaf, tree_leaf_colored, TreeState, TreeNodeResponse};

/// Draw + expire the confirmation toasts (v0.861). Stacked at bottom-center, each
/// fading out over its last half-second. Call once per frame from the main render
/// loop, ABOVE the pages, so a "Theme saved" style confirmation floats over whatever
/// is on screen. The universal answer to "the save button doesn't show it worked".
/// Fullscreen underwater tint (v0.903 diving v1): a translucent blue-green
/// wash whenever the camera is below the sea surface, so being in the water
/// LOOKS like being in water. Painted on egui's background layer - over the
/// 3D scene, under every panel and HUD element.
pub fn draw_underwater_tint(ctx: &egui::Context, state: &super::GuiState) {
    if !state.underwater {
        return;
    }
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("underwater_tint"),
    ));
    let r = ctx.screen_rect();
    // v0.907 depth grading: light dies with depth. The wash deepens from
    // the v0.903 surface tint to near-opaque by ~1 km and the hue slides
    // toward abyssal navy by ~4 km, so the Marianas run actually goes
    // DARK instead of staying snorkel-blue at 10,900 m.
    let d = state.underwater_depth_m.max(0.0);
    let k = (d / 1000.0).clamp(0.0, 1.0);
    let k2 = (d / 4000.0).clamp(0.0, 1.0);
    let a = (110.0 + 108.0 * k) as u8;
    let rc = (8.0 * (1.0 - k2) + 1.0 * k2) as u8;
    let gc = (46.0 * (1.0 - k2) + 7.0 * k2) as u8;
    let bc = (64.0 * (1.0 - k2) + 20.0 * k2) as u8;
    // theme-exempt: environmental water tint, not a UI token.
    painter.rect_filled(r, 0.0, egui::Color32::from_rgba_unmultiplied(rc, gc, bc, a)); // theme-exempt: underwater wash
    // Depth readout: divers get one number they actually want. Top-center,
    // clear of both the vitals HUD (left) and compass strip (right).
    let txt = if d >= 1000.0 {
        format!("Depth {:.2} km", d / 1000.0)
    } else {
        format!("Depth {:.0} m", d)
    };
    painter.text(
        egui::pos2(r.center().x, r.top() + 54.0),
        egui::Align2::CENTER_TOP,
        txt,
        egui::FontId::proportional(16.0),
        egui::Color32::from_rgba_unmultiplied(210, 235, 245, 220), // theme-exempt: underwater HUD overlay
    );
}

pub fn draw_toasts(ctx: &egui::Context, theme: &Theme, state: &mut super::GuiState) {
    use super::ToastKind;
    const LIFE: f64 = 2.6; // seconds fully visible + fade
    const FADE: f64 = 0.5; // fade-out window at the end

    let now = ctx.input(|i| i.time);
    // Adopt engine-queued toasts (v0.890): stamp them with the egui clock here,
    // the first place both the queue and the clock are in scope.
    for (text, kind) in state.pending_toasts.drain(..).collect::<Vec<_>>() {
        state.toast(text, kind, now);
    }
    state.toasts.retain(|t| now - t.created < LIFE);
    if state.toasts.is_empty() {
        return;
    }
    ctx.request_repaint(); // keep the fade animating even with no input

    // Newest toast sits lowest; older ones stack upward.
    for (i, t) in state.toasts.iter().enumerate() {
        let age = now - t.created;
        let alpha = if age > LIFE - FADE {
            (((LIFE - age) / FADE) as f32).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let a = |c: Color32| Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (c.a() as f32 * alpha) as u8);
        // Colour carries the kind (green/blue/red). No leading font glyph: the app
        // font renders a check mark / warning sign as tofu, so we paint a small
        // filled dot instead (a shape, never tofu).
        let accent = match t.kind {
            ToastKind::Success => theme.success(),
            ToastKind::Info => theme.accent(),
            ToastKind::Error => theme.danger(),
        };
        let y_off = -(48.0 + i as f32 * 40.0);
        egui::Area::new(egui::Id::new(("hos_toast", i)))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, y_off))
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(a(theme.bg_card()))
                    .stroke(Stroke::new(1.5, a(accent)))
                    .rounding(Rounding::same(theme.border_radius as u8))
                    .inner_margin(egui::Margin::symmetric(14, 9))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // Painted status dot -- font-independent, so it never tofus.
                            let (rect, _) = ui.allocate_exact_size(Vec2::splat(9.0), Sense::hover());
                            ui.painter().circle_filled(rect.center(), 4.0, a(accent));
                            ui.add_space(2.0);
                            ui.label(
                                RichText::new(&t.text)
                                    .size(theme.font_size_body)
                                    .color(a(theme.text_primary())),
                            );
                        });
                    });
            });
    }
}

/// Styled card container with background.
pub fn card(ui: &mut Ui, theme: &Theme, add_contents: impl FnOnce(&mut Ui)) {
    egui::Frame::none()
        .fill(theme.bg_card())
        .rounding(Rounding::same(theme.border_radius as u8))
        .inner_margin(theme.card_padding)
        .stroke(Stroke::new(1.0, theme.border()))
        .show(ui, |ui| {
            add_contents(ui);
        });
}

/// Card with a title header.
pub fn card_with_header(ui: &mut Ui, theme: &Theme, title: &str, add_contents: impl FnOnce(&mut Ui)) {
    card(ui, theme, |ui| {
        ui.label(RichText::new(title).size(theme.font_size_heading).color(theme.text_primary()));
        ui.add_space(theme.spacing_sm);
        add_contents(ui);
    });
}

/// Tinted section card — like `card`, but with a colored stroke + tinted
/// background derived from the supplied accent color. Used in pages where
/// privilege tier / nav category needs to be color-coded (e.g. server
/// settings: red = USER, green = MOD, blue = ADMIN). Adds a small uppercase
/// title above the card in the same accent color so the section reads at
/// a glance.
///
/// `max_width` lets the page choose how wide the card sits — a page with
/// multiple stacked sections should pass the SAME value to every call so
/// the cards line up. Pages that mix narrow text-only sections with a
/// wide spreadsheet should pick a width that fits the widest content
/// AND apply it uniformly (operator pushback 2026-05-08: mismatched
/// section widths read as a layout bug).
///
/// Replaces the page-local `color_section` helpers that used to copy this
/// pattern in each file. Edit here to restyle every tinted section.
pub fn tinted_section(
    ui: &mut Ui,
    theme: &Theme,
    title: &str,
    accent: Color32,
    max_width: f32,
    contents: impl FnOnce(&mut Ui, &Theme),
) {
    // Reliable centered fixed-width column.
    //
    // History: `vertical_centered + set_max_width` (v≤0.253) and
    // `horizontal + add_space + allocate_ui_with_layout` (v0.257) BOTH
    // failed to constrain — inside a `ScrollArea::auto_shrink([false,
    // false])` viewport, `set_max_width` is only a soft wrapping hint
    // and `allocate_ui_with_layout`'s size is ambiguous, so `ui.separator
    // ()` / grids / wrapping hints all expanded to the full ~1900px and
    // the section was stranded full-bleed-left (operator 2026-05-16,
    // twice). The ONLY robust egui primitive is a child UI with an
    // EXPLICIT `max_rect` — `available_width()` then derives from that
    // rect, so every child (separators included) is hard-bounded. Same
    // pattern proven in `widgets/body_pill.rs`. Content stays
    // LEFT-aligned inside (forms + data grids scan down a consistent
    // left edge — centering each row would wreck scannability); only the
    // column as a whole is centered.
    let avail = ui.available_rect_before_wrap();
    let full_w = avail.width();
    let col_w = full_w.min(max_width);
    let x0 = avail.left() + ((full_w - col_w) * 0.5).max(0.0);
    // Generous height — top_down grows as content is added; the real
    // used height comes from child.min_rect() afterwards.
    let panel_rect = egui::Rect::from_min_size(
        egui::pos2(x0, avail.top()),
        egui::vec2(col_w, avail.height().max(1.0)),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(panel_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    {
        let ui = &mut child;
        ui.set_min_width(col_w);
        ui.set_max_width(col_w);
        ui.label(
            RichText::new(title)
                .size(theme.font_size_small)
                .color(accent)
                .strong(),
        );
        ui.add_space(theme.spacing_sm);
        // Tinted background derived from the accent (alpha 18) — same
        // formula color_section used before extraction.
        let tint = Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 18);
        egui::Frame::none()
            .fill(tint)
            .stroke(Stroke::new(1.5, accent))
            .rounding(Rounding::same(theme.border_radius as u8))
            .inner_margin(theme.card_padding * 1.5)
            .show(ui, |ui| {
                ui.set_min_width(col_w - theme.card_padding * 3.0);
                ui.set_max_width(col_w - theme.card_padding * 3.0);
                contents(ui, theme);
            });
    }
    // Reserve the child's used space in the PARENT so the next section
    // flows below it instead of overlapping (the child was placed at an
    // explicit rect and does not auto-advance the parent cursor).
    let used = child.min_rect();
    ui.allocate_rect(used, egui::Sense::hover());
}

/// Subsection title — bold body-sized label used inside sections to group
/// related controls (e.g. "Registration", "Channels", "User management"
/// inside the Admin section of Server Settings). Pulls font + color from
/// theme tokens so restyling propagates everywhere.
pub fn subsection_label(ui: &mut Ui, theme: &Theme, text: &str) {
    ui.label(
        RichText::new(text)
            .size(theme.font_size_body)
            .color(theme.text_primary())
            .strong(),
    );
    ui.add_space(theme.spacing_xs);
}

/// Muted body text — paragraph-style hint / description. Used right
/// under headings to explain what a section does. Single source so the
/// "muted hint" voice is consistent across pages.
pub fn body_hint(ui: &mut Ui, theme: &Theme, text: &str) {
    ui.label(
        RichText::new(text)
            .size(theme.font_size_small)
            .color(theme.text_muted()),
    );
}

/// Render a text label that, when the user holds Alt and hovers over it,
/// shows the HumanityOS dictionary definition of `term` (looked up
/// case-insensitively in `data/glossary.json`). The visible text can
/// differ from the lookup key — pass the term you want defined as the
/// second argument and the display text as the first.
///
/// Example:
///   `definition_text(ui, "Ed25519", "ed25519")` shows "Ed25519" inline
///   and on Alt+hover pops the definition + Wikipedia link from the
///   glossary.
///
/// If the term isn't in the dictionary, falls back to a plain label
/// (no tooltip) so missing-term tooltips don't surprise users.
///
/// Operator 2026-05-08: "If there's a word on the screen in the menus
/// they should be able to mouse over it and hold the alt key or some
/// other configured key and get the definition of that word as it is
/// in HumanityOS' dictionary."
///
/// Returns the underlying egui Response so callers can chain.
pub fn definition_text(ui: &mut Ui, display: &str, term: &str) -> egui::Response {
    let resp = ui.label(display);
    let entry = crate::gui::glossary::glossary().lookup(term);
    if let Some(e) = entry {
        // Tooltip only fires when Alt is held — without Alt, hovering
        // is silent so the label doesn't spam tooltips when the user
        // is just reading. Matches operator's "Alt + hover" mental model.
        let alt_held = ui.ctx().input(|i| i.modifiers.alt);
        if alt_held {
            let resp_for_tooltip = resp.clone();
            let entry_term = e.term.clone();
            let entry_def = e.definition.clone();
            let entry_link = e.link.clone();
            return resp_for_tooltip.on_hover_ui(move |ui| {
                ui.set_max_width(360.0);
                ui.label(RichText::new(&entry_term).strong());
                ui.add_space(4.0);
                ui.label(&entry_def);
                if !entry_link.is_empty() {
                    ui.add_space(4.0);
                    ui.label(RichText::new(format!("More: {}", &entry_link)).italics().small());
                }
            });
        }
    }
    resp
}

/// Collapsible section with header.
pub fn collapsible_section(ui: &mut Ui, title: &str, default_open: bool, add_contents: impl FnOnce(&mut Ui)) {
    egui::CollapsingHeader::new(title)
        .default_open(default_open)
        .show(ui, |ui| {
            add_contents(ui);
        });
}

/// Aligned settings row: fixed-width label on left, control on right.
pub fn settings_row(ui: &mut Ui, theme: &Theme, label: &str, add_control: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(theme.settings_label_width, ui.spacing().interact_size.y),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.label(RichText::new(label).color(theme.text_secondary()));
            },
        );
        add_control(ui);
    });
}

/// Custom slider with visible track bar and accent fill.
/// Returns true if value changed.
pub fn custom_slider(ui: &mut Ui, theme: &Theme, value: &mut f32, range: std::ops::RangeInclusive<f32>) -> bool {
    let desired_width = ui.available_width().min(250.0);
    custom_slider_with_width(ui, theme, value, range, desired_width)
}

/// Internal slider with explicit pixel width. Used by `custom_slider`
/// (full available width) and `custom_slider_capped` (width minus a
/// reserved tail for the value label).
fn custom_slider_with_width(
    ui: &mut Ui,
    theme: &Theme,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    desired_width: f32,
) -> bool {
    let min = *range.start();
    let max = *range.end();
    let track_h = theme.slider_track_height;
    let thumb_r = theme.slider_thumb_radius;
    let widget_height = thumb_r * 2.0 + 4.0;

    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(desired_width, widget_height),
        Sense::click_and_drag(),
    );

    // Handle drag/click interaction
    let old_value = *value;
    if response.dragged() || response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let t = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            *value = min + t * (max - min);
        }
    }

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let center_y = rect.center().y;
        let t = if (max - min).abs() < f32::EPSILON { 0.5 } else { (*value - min) / (max - min) };
        let thumb_x = rect.left() + t * rect.width();
        let rounding = Rounding::same((track_h / 2.0) as u8);

        // Draw dim track (unfilled portion: thumb to right)
        let track_right = Rect::from_min_max(
            egui::pos2(thumb_x, center_y - track_h / 2.0),
            egui::pos2(rect.right(), center_y + track_h / 2.0),
        );
        painter.rect_filled(track_right, rounding, theme.slider_track());

        // Draw gradient filled portion (left to thumb): blue -> green -> red
        // Paint in thin vertical slices for smooth gradient
        let fill_left = rect.left();
        let fill_width = thumb_x - fill_left;
        if fill_width > 0.5 {
            let steps = (fill_width as usize).max(1).min(120);
            let step_w = fill_width / steps as f32;
            for i in 0..steps {
                let local_t = i as f32 / steps as f32;
                // Blue(0%) -> Green(50%) -> Red(100%)
                let (r, g, b) = if local_t < 0.5 {
                    let s = local_t * 2.0; // 0..1 within first half
                    (0.0, s, 1.0 - s) // blue to green
                } else {
                    let s = (local_t - 0.5) * 2.0; // 0..1 within second half
                    (s, 1.0 - s, 0.0) // green to red
                };
                let color = Color32::from_rgb(
                    (r * 220.0 + 35.0) as u8,
                    (g * 220.0 + 35.0) as u8,
                    (b * 200.0 + 30.0) as u8,
                );
                let x0 = fill_left + i as f32 * step_w;
                let x1 = x0 + step_w + 0.5; // slight overlap to avoid gaps
                let slice = Rect::from_min_max(
                    egui::pos2(x0, center_y - track_h / 2.0),
                    egui::pos2(x1.min(thumb_x), center_y + track_h / 2.0),
                );
                // Only round the leftmost and rightmost slices
                let slice_round = if i == 0 { rounding } else { Rounding::ZERO };
                painter.rect_filled(slice, slice_round, color);
            }
        }

        // Draw thumb: filled circle with animated RGB border. The thumb is a
        // light knob that must read against the gradient track: brightest on
        // hover/drag, a step down at rest. Both come from text tokens (primary =
        // the app's brightest ink, secondary = one step down) so a user who
        // restyles the palette gets a thumb that still matches it.
        let thumb_center = egui::pos2(thumb_x, center_y);
        let thumb_fill = if response.hovered() || response.dragged() {
            theme.text_primary()
        } else {
            theme.text_secondary()
        };
        painter.circle_filled(thumb_center, thumb_r, thumb_fill);
        // RGB animated border (1.5px)
        let ctx_time = ui.ctx().input(|i| i.time);
        let rgb_color = crate::gui::widgets::row::rgb_from_time(ctx_time);
        painter.circle_stroke(thumb_center, thumb_r, egui::Stroke::new(1.5, rgb_color));
        // Request repaint for animation
        ui.ctx().request_repaint();
    }

    let changed = (*value - old_value).abs() > f32::EPSILON;
    changed
}

/// Labeled slider with aligned label and custom track. Returns true if value changed.
///
/// Layout: `[settings_label][slider track][value text]`. The slider reserves
/// 56px on its right for the numeric value so they never overlap, even when
/// the user has set spacing tokens to near-zero.
pub fn labeled_slider(ui: &mut Ui, theme: &Theme, label: &str, value: &mut f32, range: std::ops::RangeInclusive<f32>) -> bool {
    let mut changed = false;
    settings_row(ui, theme, label, |ui| {
        // Reserve space for the value label so the slider doesn't overlap it.
        // Without this, custom_slider grabs all remaining width and the
        // numeric value paints on top — the FOV-overlap bug operator hit.
        const VALUE_RESERVE: f32 = 56.0;
        changed = custom_slider_capped(ui, theme, value, range.clone(), VALUE_RESERVE);
        // Show numeric value after slider
        let max = *range.end();
        let value_text = if max <= 1.0 {
            format!("{:.2}", *value)
        } else if max <= 20.0 {
            format!("{:.1}", *value)
        } else {
            format!("{:.0}", *value)
        };
        ui.label(RichText::new(value_text).color(theme.text_muted()).size(theme.font_size_small));
    });
    changed
}

/// Like custom_slider but reserves `reserve_right` pixels of horizontal
/// space (for a trailing value label / icon / etc). Returns true if changed.
pub fn custom_slider_capped(
    ui: &mut Ui,
    theme: &Theme,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    reserve_right: f32,
) -> bool {
    let avail = ui.available_width();
    let target_w = (avail - reserve_right).max(60.0).min(250.0);
    custom_slider_with_width(ui, theme, value, range, target_w)
}

/// Custom checkbox with visible border when unchecked.
/// Returns true if value changed.
pub fn custom_checkbox(ui: &mut Ui, theme: &Theme, value: &mut bool) -> bool {
    let size = theme.checkbox_size;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());

    if response.clicked() {
        *value = !*value;
    }

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let rounding = Rounding::same(3);

        if *value {
            // Checked: filled accent background + checkmark
            painter.rect_filled(rect, rounding, theme.accent());

            // Draw checkmark
            let check_color = theme.text_on_accent();
            let cx = rect.center().x;
            let cy = rect.center().y;
            let s = size * 0.25;
            let points = [
                egui::pos2(cx - s, cy),
                egui::pos2(cx - s * 0.3, cy + s * 0.7),
                egui::pos2(cx + s, cy - s * 0.6),
            ];
            painter.line_segment([points[0], points[1]], Stroke::new(2.0, check_color));
            painter.line_segment([points[1], points[2]], Stroke::new(2.0, check_color));
        } else {
            // Unchecked: visible border outline (always visible)
            painter.rect_stroke(rect, rounding, Stroke::new(1.5, theme.border()), egui::epaint::StrokeKind::Outside);
        }
    }

    response.clicked()
}

/// Toggle switch with label and visible checkbox. Returns true if value changed.
pub fn toggle(ui: &mut Ui, theme: &Theme, label: &str, value: &mut bool) -> bool {
    let mut changed = false;
    settings_row(ui, theme, label, |ui| {
        changed = custom_checkbox(ui, theme, value);
    });
    changed
}

/// Progress bar (0.0 to 1.0).
pub fn progress_bar(ui: &mut Ui, theme: &Theme, progress: f32, label: Option<&str>) {
    let bar = egui::ProgressBar::new(progress.clamp(0.0, 1.0))
        .fill(theme.accent());
    let bar = if let Some(text) = label {
        bar.text(text)
    } else {
        bar
    };
    ui.add(bar);
}

/// One compact stat row: `name · value · thin bar`, all on ONE line (replacing the
/// old two-row "label, then a tall ProgressBar below"). The name + value columns
/// have fixed shared widths (so stacked rows align into columns); the value is
/// right-aligned (numbers line up); the bar is a thin 6px fill taking the rest of
/// the width. Call several in a vertical for an aligned "stat table". Reusable
/// anywhere a 0..1 stat needs a compact row (vitals, later the inventory tree, …).
pub fn stat_row(
    ui: &mut Ui,
    theme: &Theme,
    name: &str,
    value: &str,
    value_color: egui::Color32,
    frac: f32,
    fill: egui::Color32,
) {
    ui.horizontal(|ui| {
        let h = theme.font_size_body + 2.0;
        ui.allocate_ui_with_layout(
            egui::vec2(theme.stat_name_width, h),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.label(
                    egui::RichText::new(name)
                        .color(theme.text_secondary())
                        .size(theme.font_size_small),
                );
            },
        );
        ui.allocate_ui_with_layout(
            egui::vec2(theme.stat_value_width, h),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.label(
                    egui::RichText::new(value)
                        .color(value_color)
                        .size(theme.font_size_small),
                );
            },
        );
        // Capped width (theme.status_bar_width, default 200px) so the bars stay a
        // tidy fixed column instead of spanning the panel (operator 2026-06-08).
        ui.add(
            egui::ProgressBar::new(frac.clamp(0.0, 1.0))
                .fill(fill)
                .desired_height(theme.status_bar_height)
                .desired_width(theme.status_bar_width),
        );
    });
}

/// A square +/- stepper button with state-driven colours: an enabled "+" gets a
/// green border + very-dark-green fill + white glyph; an enabled "-" the same in
/// red; a DISABLED button is flat dark gray (glyph included). Width = height (a
/// square). Returns true on a click while enabled.
pub fn stepper_button(
    ui: &mut Ui,
    theme: &Theme,
    symbol: &str,
    enabled: bool,
    positive: bool,
) -> bool {
    // Slim square sized to the compact-button height so the +/- match the garden's
    // inline buttons (operator 2026-06-08).
    let s = theme.compact_button_height;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(s, s), egui::Sense::click());
    let (border, fill, fg) = if !enabled {
        (theme.text_muted(), theme.bg_secondary(), theme.text_muted())
    } else if positive {
        let g = theme.success();
        let dark = egui::Color32::from_rgb(g.r() / 5, g.g() / 5, g.b() / 5); // theme-exempt: very-dark fill derived from the success token, not a literal
        (g, dark, theme.text_primary())
    } else {
        let r = theme.danger();
        let dark = egui::Color32::from_rgb(r.r() / 5, r.g() / 5, r.b() / 5); // theme-exempt: very-dark fill derived from the danger token, not a literal
        (r, dark, theme.text_primary())
    };
    let painter = ui.painter();
    painter.rect(
        rect,
        egui::Rounding::same(3),
        fill,
        egui::Stroke::new(1.5, border),
        egui::StrokeKind::Inside,
    );
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        symbol,
        egui::FontId::proportional(theme.font_size_body),
        fg,
    );
    enabled && resp.clicked()
}

/// A node in a [`tree_list`] — the uniform recursive container model: a node carries
/// an optional `detail` value and any number of child nodes. A "container" is a node
/// with children; an "item" is a leaf. The same shape serves your inventory, a
/// vehicle's hold, a home's rooms — anything nested, any depth.
#[derive(Debug, Clone, Default)]
pub struct TreeNode {
    /// Selection id — empty means the row isn't clickable. For a backpack item this
    /// is its slot index; clicking it is how the caller drives the item detail panel.
    pub id: String,
    pub label: String,
    /// Right-aligned secondary text (quantity, weight, location, …). Empty = none.
    pub detail: String,
    /// Optional colour swatch painted before the label — caller-resolved (e.g. by
    /// entity/kind) so the tree is scannable at a glance. `None` = no swatch.
    pub color: Option<Color32>,
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    pub fn leaf(label: impl Into<String>, detail: impl Into<String>) -> Self {
        Self { id: String::new(), label: label.into(), detail: detail.into(), color: None, children: Vec::new() }
    }
    /// A clickable leaf — clicking it makes [`tree_list`] return `id`.
    pub fn selectable(id: impl Into<String>, label: impl Into<String>, detail: impl Into<String>) -> Self {
        Self { id: id.into(), label: label.into(), detail: detail.into(), color: None, children: Vec::new() }
    }
    pub fn branch(label: impl Into<String>, detail: impl Into<String>, children: Vec<TreeNode>) -> Self {
        Self { id: String::new(), label: label.into(), detail: detail.into(), color: None, children }
    }
    /// Attach a colour swatch (builder).
    pub fn with_color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }
}

fn tree_detail(ui: &mut Ui, theme: &Theme, detail: &str) {
    if !detail.is_empty() {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(detail)
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
        });
    }
}

/// Render ONE leaf row (swatch + name cell + inline detail), clickable when it
/// carries an id. Shared by the single-column path and the multi-column grid, so a
/// big flat list of leaves (dozens of seeds) can flow into columns instead of one
/// very tall column. The inline expand-card is rendered by the CALLER (under the
/// row in single-column, below the grid in multi-column).
fn leaf_row(ui: &mut Ui, theme: &Theme, node: &TreeNode, selected: &str, clicked: &mut Option<String>) {
    ui.horizontal(|ui| {
        paint_swatch(ui, node.color);
        row_cell(ui, theme.cell_name_width, |ui| {
            if node.id.is_empty() {
                ui.label(egui::RichText::new(&node.label).color(theme.text_primary()));
            } else if ui
                .selectable_label(
                    selected == node.id,
                    egui::RichText::new(&node.label).color(theme.text_primary()),
                )
                .clicked()
            {
                *clicked = Some(node.id.clone());
            }
        });
        if !node.detail.is_empty() {
            ui.label(
                egui::RichText::new(&node.detail)
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
        }
    });
}

fn container_node(
    ui: &mut Ui,
    theme: &Theme,
    node: &TreeNode,
    selected: &str,
    clicked: &mut Option<String>,
    default_open: bool,
    force: Option<bool>,
    inline: &mut dyn FnMut(&mut Ui, &str),
) {
    if node.children.is_empty() {
        leaf_row(ui, theme, node, selected, clicked);
        // Inline EXPAND-IN-PLACE body (operator 2026-06-08: "click an item row to
        // expand in place ... instead of a popup/top detail"). When this leaf is the
        // selected one, render its detail card directly under the row, indented so it
        // visually belongs to the row above. `inline` is a no-op for trees that don't
        // use it (e.g. the plain `tree_list`).
        if !node.id.is_empty() && selected == node.id {
            ui.indent(("inline_body", &node.id), |ui| inline(ui, &node.id));
        }
    } else {
        // A branch IS the universal [`expandable_row`] (v0.414 — same persisted
        // collapse state, same Collapse/Expand-all `force`, one nesting primitive
        // across the app). Header clicks on a selectable container land in a local
        // and merge into `clicked` after the call, because the body closure needs
        // `clicked` itself for the recursion.
        let mut header_clicked: Option<String> = None;
        expandable_row(
            ui,
            "tree_node",
            default_open,
            force,
            |ui| {
                paint_swatch(ui, node.color);
                // A container header is itself SELECTABLE when it carries an id (so a
                // garden plot / tower can be picked, not only expanded); plain label
                // otherwise. The collapse triangle stays an independent control.
                if node.id.is_empty() {
                    ui.label(egui::RichText::new(&node.label).color(theme.text_primary()).strong());
                } else if ui
                    .selectable_label(
                        selected == node.id,
                        egui::RichText::new(&node.label).color(theme.text_primary()).strong(),
                    )
                    .clicked()
                {
                    header_clicked = Some(node.id.clone());
                }
                tree_detail(ui, theme, &node.detail);
            },
            |ui| {
                // If this container holds MANY leaf items (e.g. dozens of seeds),
                // flow them into columns so it does not become one very tall column;
                // the selected item's inline card renders full-width below the grid.
                // Mixed or few children recurse sequentially as before.
                let all_leaves = node.children.iter().all(|c| c.children.is_empty());
                if all_leaves && node.children.len() > 12 {
                    let avail = ui.available_width();
                    let ncols = (avail / 260.0).floor().clamp(2.0, 4.0) as usize;
                    ui.columns(ncols, |cols| {
                        for (j, child) in node.children.iter().enumerate() {
                            leaf_row(&mut cols[j % ncols], theme, child, selected, clicked);
                        }
                    });
                    if !selected.is_empty() {
                        if let Some(sel) = node.children.iter().find(|c| selected == c.id) {
                            ui.indent(("inline_body", &sel.id), |ui| inline(ui, &sel.id));
                        }
                    }
                } else {
                    for (j, child) in node.children.iter().enumerate() {
                        ui.push_id(j, |ui| {
                            container_node(ui, theme, child, selected, clicked, default_open, force, inline)
                        });
                    }
                }
            },
        );
        if header_clicked.is_some() {
            *clicked = header_clicked;
        }
    }
}

/// Paint a small colour swatch before a tree row's label (skipped when `None`).
/// Vertically centred with the row by the horizontal layout's default align.
fn paint_swatch(ui: &mut Ui, color: Option<Color32>) {
    if let Some(c) = color {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
        ui.painter().rect_filled(rect, Rounding::same(2), c);
        ui.add_space(4.0);
    }
}

/// Render a list of [`TreeNode`] roots as an expand/collapse tree — the reusable
/// nested container/inventory view. Branches use egui's persistent collapsing state
/// (open by default, child indentation handled for us); leaves are rows, clickable if
/// they carry an `id`. Returns the id of a leaf clicked this frame. The SAME widget
/// renders a real inventory or any sim entity's — the caller just hands it a different
/// root, which is how Real and Sim stay structurally separate (different pages, same
/// widget). `selected` highlights the current selection.
pub fn tree_list(ui: &mut Ui, theme: &Theme, roots: &[TreeNode], selected: &str) -> Option<String> {
    tree_list_ex(ui, theme, roots, selected, true, None, &mut |_, _| {})
}

/// [`tree_list`] with explicit collapse control: `default_open` sets the initial
/// state of branches that have no stored state yet (the "Start collapsed"
/// preference passes `false`); `force` overrides EVERY branch open/closed for this
/// frame (the Collapse-all / Expand-all buttons), after which the state persists
/// until the next manual toggle.
pub fn tree_list_ex(
    ui: &mut Ui,
    theme: &Theme,
    roots: &[TreeNode],
    selected: &str,
    default_open: bool,
    force: Option<bool>,
    // Renders the inline expand-in-place body for the SELECTED leaf (called with its
    // id). Pass `&mut |_, _| {}` for a plain tree with no inline detail.
    inline: &mut dyn FnMut(&mut Ui, &str),
) -> Option<String> {
    let mut clicked = None;
    // If the ROOTS are a big flat list of leaves (the flat backpack fallback, when the
    // entity spine isn't loaded), flow them into columns too -- the same treatment a
    // leaf-heavy CONTAINER gets in container_node -- so the layout doesn't depend on
    // whether the spine loaded. The selected item's inline card renders below the grid.
    let all_leaf_roots = roots.iter().all(|n| n.children.is_empty());
    if all_leaf_roots && roots.len() > 12 {
        let avail = ui.available_width();
        let ncols = (avail / 260.0).floor().clamp(2.0, 4.0) as usize;
        ui.columns(ncols, |cols| {
            for (i, node) in roots.iter().enumerate() {
                leaf_row(&mut cols[i % ncols], theme, node, selected, &mut clicked);
            }
        });
        if !selected.is_empty() {
            if let Some(sel) = roots.iter().find(|n| selected == n.id) {
                ui.indent(("inline_body", &sel.id), |ui| inline(ui, &sel.id));
            }
        }
        return clicked;
    }
    for (i, node) in roots.iter().enumerate() {
        ui.push_id(i, |ui| {
            container_node(ui, theme, node, selected, &mut clicked, default_open, force, inline)
        });
    }
    clicked
}

/// A single EXPANDABLE ROW — the universal nesting primitive (operator 2026-06-08:
/// "this nesting style ... should be a universal widget with configurable columns
/// per implementation"). The `header` closure renders the row's content (e.g. a set
/// of fixed-width column cells + inline buttons); clicking the row's triangle toggles
/// the `body` (e.g. nested expandable rows, or a multi-row detail card). `default_open`
/// + `force` wire it to the standard Collapse/Expand/Start-collapsed controls (pass
/// the page's tree_default_open / tree_force). COMPOSE them to any depth: a group row
/// whose body is more `expandable_row`s, whose bodies are detail cards. Reusable across
/// the app (garden towers/slots, mining, the inventory tree, future sections).
pub fn expandable_row(
    ui: &mut Ui,
    id_salt: impl std::hash::Hash,
    default_open: bool,
    force: Option<bool>,
    header: impl FnOnce(&mut Ui),
    body: impl FnOnce(&mut Ui),
) {
    let id = ui.make_persistent_id(id_salt);
    let mut state =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, default_open);
    if let Some(open) = force {
        state.set_open(open);
    }
    state.show_header(ui, |ui| header(ui)).body(|ui| body(ui));
}

/// A fixed-width cell for an [`expandable_row`] header (or any aligned column row):
/// allocates exactly `width` and renders `content` left-aligned + clipped, so stacked
/// rows line up into columns without an egui::Grid (which can't host an inline
/// expanding body). Pair several for a configurable-column row.
pub fn row_cell(ui: &mut Ui, width: f32, content: impl FnOnce(&mut Ui)) {
    ui.allocate_ui_with_layout(
        Vec2::new(width, ui.spacing().interact_size.y),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_clip_rect(ui.max_rect());
            content(ui);
        },
    );
}

/// A deterministic, pleasant color derived from a seed string (e.g. a plant id
/// or category). The same seed always yields the same hue, so a species keeps a
/// stable colour everywhere it appears. Pure FNV-1a-hash → HSV→RGB math — this
/// is an INFINITE, data-seeded palette (one colour per arbitrary id), not a
/// fixed brand colour, so no theme token applies (the args are computed, so the
/// theme-token lint doesn't flag the `from_rgb`). Use for placeholder image
/// tiles, category swatches, avatar fallbacks — anywhere you need "a colour for
/// this thing" without a curated palette.
pub fn swatch_color(seed: &str) -> Color32 {
    // FNV-1a over the bytes — cheap, stable, std-only, well-spread across hues.
    let mut h: u32 = 2166136261;
    for b in seed.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    let hue = (h % 360) as f32;
    let s = 0.55_f32; // muted, so light text reads on top
    let l = 0.42_f32;
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = if hue < 60.0 {
        (c, x, 0.0)
    } else if hue < 120.0 {
        (x, c, 0.0)
    } else if hue < 180.0 {
        (0.0, c, x)
    } else if hue < 240.0 {
        (0.0, x, c)
    } else if hue < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    Color32::from_rgb(((r + m) * 255.0) as u8, ((g + m) * 255.0) as u8, ((b + m) * 255.0) as u8) // theme-exempt: FNV-1a hash to HSV-to-RGB, an infinite data-seeded palette (one color per arbitrary id)
}

/// A colored placeholder tile for "image / 3D model goes here" slots — a rounded
/// square filled with `color` (typically [`swatch_color`] of the item's id) with
/// `glyph` (e.g. a plant's initial) centered in white. Stands in until a real
/// image/model loads; reusable for garden slots, market listings, avatars, etc.
pub fn placeholder_tile(ui: &mut Ui, theme: &Theme, color: Color32, size: f32, glyph: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let rounding = Rounding::same(theme.border_radius_lg as u8);
    ui.painter().rect_filled(rect, rounding, color);
    ui.painter().rect_stroke(
        rect,
        rounding,
        Stroke::new(1.0, theme.border()),
        egui::StrokeKind::Inside,
    );
    if !glyph.is_empty() {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            glyph,
            egui::FontId::proportional(size * 0.46),
            Color32::WHITE,
        );
    }
}

/// A collapsible MAIN-SECTION header (operator 2026-06-08: "every section
/// collapsible + more defined, with the standard Collapse/Expand/Start-collapsed
/// on ALL nested lists"). Draws `title` as a heading with a painted triangle
/// (▶ collapsed / ▼ open — a SHAPE, not a font glyph, so it never renders as
/// tofu) and returns whether the section is OPEN. The caller guards its body
/// with `if section_header(..) { ...body... }`, so the body stays in the caller's
/// scope — no body closure, no re-borrowing of the page's `action_*` locals. The
/// open-state persists per `id_salt` in egui memory; `force` wires the global
/// Collapse-all / Expand-all / Start-collapsed controls (pass the page's
/// `tree_force`) so one control set drives every section + nested tree at once.
/// (Distinct from [`section_header`] — a non-collapsible heading — and from
/// [`collapsible_section`], which takes a body CLOSURE; this one is closure-free
/// and returns the open-state so the caller guards its own body with an `if`.)
pub fn section_disclosure(
    ui: &mut Ui,
    theme: &Theme,
    id_salt: impl std::hash::Hash,
    title: &str,
    force: Option<bool>,
) -> bool {
    let id = ui.make_persistent_id(id_salt);
    // Persisted open-state (defaults open). A global Collapse/Expand click
    // (`force`) overrides it this frame AND is written back so it sticks.
    let mut open = ui.data_mut(|d| *d.get_temp_mut_or(id, true));
    if let Some(f) = force {
        open = f;
    }
    let sz = theme.font_size_heading;
    let row = ui.horizontal(|ui| {
        let (tri_rect, _) = ui.allocate_exact_size(Vec2::splat(sz), Sense::hover());
        // A small triangle pointing right (collapsed) or down (open). Painted as
        // a filled polygon so glyph-font coverage is irrelevant.
        let ctr = tri_rect.center();
        let r = sz * 0.28;
        let pts = if open {
            vec![
                ctr + Vec2::new(-r, -r * 0.6),
                ctr + Vec2::new(r, -r * 0.6),
                ctr + Vec2::new(0.0, r * 0.8),
            ]
        } else {
            vec![
                ctr + Vec2::new(-r * 0.6, -r),
                ctr + Vec2::new(-r * 0.6, r),
                ctr + Vec2::new(r * 0.8, 0.0),
            ]
        };
        ui.painter().add(egui::Shape::convex_polygon(pts, theme.text_secondary(), Stroke::NONE));
        ui.label(RichText::new(title).size(sz).color(theme.text_primary()));
    });
    let resp = row.response.interact(Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if resp.clicked() {
        open = !open;
    }
    ui.data_mut(|d| d.insert_temp(id, open));
    open
}

/// State for a [`lockable_gate`] — kept per-section in GuiState, IN MEMORY ONLY
/// (never persisted), so an app restart re-locks everything.
#[derive(Debug, Default, Clone)]
pub struct LockState {
    /// Currently unlocked (body shown). Cleared on Lock / restart.
    pub unlocked: bool,
    /// Passphrase entry buffer (cleared on successful unlock / on Lock).
    pub input: String,
    /// Reveal the passphrase characters — the header's Show/Hide toggle.
    pub show: bool,
    /// The last unlock attempt failed (shows a hint until the next attempt).
    pub error: bool,
}

/// A private-section gate: collapsed + **locked** by default, with the body
/// rendered by the caller ONLY when this returns `true`. The locked header is
/// `[Title]  [Show/Hide]  [Unlock]  [passphrase entry — fills the rest]`; once
/// unlocked it shows the title + a `[Lock]` button (Lock = collapse = re-lock,
/// the "locked when not actively in use" model). `verify` is the caller's
/// passphrase check (e.g. decrypt the vault) — the widget never handles the
/// secret beyond the typed input, and nothing is persisted, so this is safe for
/// crypto keys / identity / recovery data. All labels are plain text (no glyphs
/// that could render as tofu in the bundled font).
pub fn lockable_gate(
    ui: &mut Ui,
    theme: &Theme,
    lock: &mut LockState,
    title: &str,
    verify: impl Fn(&str) -> bool,
) -> bool {
    if lock.unlocked {
        ui.horizontal(|ui| {
            ui.label(RichText::new(title).strong().color(theme.text_primary()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Lock").clicked() {
                    lock.unlocked = false;
                    lock.input.clear();
                    lock.error = false;
                }
            });
        });
        ui.separator();
        return true;
    }

    ui.horizontal(|ui| {
        ui.label(RichText::new(title).strong().color(theme.text_primary()));
        ui.add_space(8.0);
        let show_label = if lock.show { "Hide" } else { "Show" };
        if ui.button(show_label).clicked() {
            lock.show = !lock.show;
        }
        let unlock_clicked = ui.button("Unlock").clicked();
        let resp = ui.add(
            egui::TextEdit::singleline(&mut lock.input)
                .password(!lock.show)
                .hint_text("passphrase")
                .desired_width(ui.available_width()),
        );
        let submit = unlock_clicked
            || (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
        if submit {
            if verify(&lock.input) {
                lock.unlocked = true;
                lock.input.clear();
                lock.error = false;
            } else {
                lock.error = true;
            }
        }
    });
    if lock.error {
        ui.label(
            RichText::new("Wrong passphrase")
                .size(theme.font_size_small)
                .color(theme.danger()),
        );
    }
    false
}

/// One entry in a [`section_nav`] sidebar / table-of-contents. Carries a stable
/// `id` (returned on click), a display `label`, an `accent` colour (used for the
/// active-row tint and the optional group-header dot), and an optional `group`
/// header rendered above this item when it opens a new group.
#[derive(Debug, Clone)]
pub struct SectionNavItem {
    pub id: String,
    pub label: String,
    pub accent: Color32,
    /// `Some(header)` renders an uppercase group header (with an `accent` dot)
    /// above this item — set it on the FIRST item of each group.
    pub group: Option<String>,
}

impl SectionNavItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>, accent: Color32) -> Self {
        Self { id: id.into(), label: label.into(), accent, group: None }
    }
    /// Open a new group, with `header` shown above this item.
    pub fn group(mut self, header: impl Into<String>) -> Self {
        self.group = Some(header.into());
        self
    }
}

/// Universal section-nav sidebar / table-of-contents — the generalisation of the
/// Profile page's grouped section list, usable on ANY page. Renders an optional
/// `title`, then each item (with a group header where one opens), highlighting the
/// row whose id == `active`. Returns `Some(id)` when a row is clicked. The CALLER
/// owns the active-id state and decides what a click does — switch the shown
/// section, or scroll a long page to that section's anchor — so the same widget
/// drives a switcher page and an infinite-scroll TOC alike.
pub fn section_nav(
    ui: &mut Ui,
    theme: &Theme,
    title: Option<&str>,
    items: &[SectionNavItem],
    active: &str,
) -> Option<String> {
    let mut clicked = None;

    if let Some(t) = title {
        ui.label(RichText::new(t).size(theme.font_size_heading).color(theme.text_primary()));
        ui.add_space(theme.spacing_md);
    }

    let mut first_group = true;
    for item in items {
        // Group header — rendered when this item opens a new group.
        if let Some(header) = &item.group {
            if !first_group {
                ui.add_space(theme.spacing_sm);
            }
            first_group = false;
            ui.horizontal(|ui| {
                let (dot_rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                ui.painter().circle_filled(dot_rect.center(), 4.0, item.accent);
                ui.label(
                    RichText::new(header)
                        .size(theme.font_size_small)
                        .color(item.accent)
                        .strong(),
                );
            });
            ui.add_space(theme.row_gap);
        }

        // Section row — the active row gets a tinted fill + stroke from the accent
        // (alpha-derived from the caller's colour, so no hardcoded palette here).
        let is_active = item.id == active;
        let text_color = if is_active { Color32::WHITE } else { theme.text_muted() };
        let a = item.accent;
        let fill = if is_active {
            Color32::from_rgba_unmultiplied(a.r(), a.g(), a.b(), 30)
        } else {
            Color32::TRANSPARENT
        };
        let stroke = if is_active {
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(a.r(), a.g(), a.b(), 100))
        } else {
            Stroke::NONE
        };
        let btn = egui::Button::new(
            RichText::new(&item.label).size(theme.font_size_body).color(text_color),
        )
        .fill(fill)
        .stroke(stroke)
        .rounding(Rounding::same(4))
        .min_size(Vec2::new(ui.available_width(), 28.0));
        if ui.add(btn).clicked() {
            clicked = Some(item.id.clone());
        }
    }

    clicked
}

/// Tab bar. Updates active index, returns true if changed.
///
/// Uses the universal `Button` builder under the hood — every tab/nav button
/// across the app flows through the same widget. Edit `Button::show` once and
/// every tab updates: header menu, settings categories, marketplace filters,
/// chat channel switcher, all of them.
pub fn tab_bar(ui: &mut Ui, theme: &Theme, tabs: &[&str], active: &mut usize) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        for (i, tab) in tabs.iter().enumerate() {
            let is_active = i == *active;
            if Button::tab(tab, is_active).show(ui, theme) && !is_active {
                *active = i;
                changed = true;
            }
        }
    });
    changed
}

/// Role badge pill.
pub fn role_badge(ui: &mut Ui, theme: &Theme, role: &str) {
    let (color, letter) = match role {
        "admin" => (Theme::c32(&theme.badge_admin), "A"),
        "mod" => (Theme::c32(&theme.badge_mod), "M"),
        "verified" => (Theme::c32(&theme.badge_verified), "V"),
        "donor" => (Theme::c32(&theme.badge_donor), "D"),
        _ => return,
    };
    let text = RichText::new(letter).size(theme.font_size_small).color(Color32::WHITE);
    egui::Frame::none()
        .fill(color)
        .rounding(Rounding::same(3))
        .inner_margin(Vec2::new(4.0, 1.0))
        .show(ui, |ui| { ui.label(text); });
}

// ─────────────────────── UNIVERSAL WIDGETS ───────────────────────
// Shared across all pages. Use these instead of building inline.

/// Colored badge pill. Replaces 20+ inline badge patterns across pages.
/// `text` is the display label, `color` is the badge background.
pub fn badge(ui: &mut Ui, theme: &Theme, text: &str, color: Color32) {
    egui::Frame::none()
        .fill(color)
        .rounding(Rounding::same(theme.badge_radius as u8))
        .inner_margin(theme.badge_padding())
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(theme.small_size).color(Color32::WHITE));
        });
}

/// Small badge variant (tighter padding).
pub fn badge_sm(ui: &mut Ui, theme: &Theme, text: &str, color: Color32) {
    egui::Frame::none()
        .fill(color)
        .rounding(Rounding::same(theme.badge_radius as u8))
        .inner_margin(Vec2::new(4.0, 1.0))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(theme.small_size).color(Color32::WHITE));
        });
}

/// Label: Value detail row. Used in maps, inventory, crafting detail panels.
pub fn detail_row(ui: &mut Ui, theme: &Theme, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{}:", label))
                .color(theme.text_secondary())
                .size(theme.small_size),
        );
        ui.label(
            RichText::new(value)
                .color(theme.text_primary())
                .size(theme.small_size),
        );
    });
}

/// Bold label: Value row (for headers/important stats).
pub fn detail_row_bold(ui: &mut Ui, theme: &Theme, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{}:", label))
                .color(theme.text_secondary())
                .size(theme.body_size)
                .strong(),
        );
        ui.label(
            RichText::new(value)
                .color(theme.text_primary())
                .size(theme.body_size)
                .strong(),
        );
    });
}

/// Search bar with label. Returns true if the text changed.
pub fn search_bar(ui: &mut Ui, theme: &Theme, value: &mut String, hint: &str) -> bool {
    let before = value.clone();
    ui.horizontal(|ui| {
        ui.label(RichText::new("Search:").color(theme.text_secondary()).size(theme.body_size));
        ui.add(
            egui::TextEdit::singleline(value)
                .desired_width(200.0)
                .hint_text(hint),
        );
    });
    *value != before
}

/// Sidebar navigation with active-state highlighting.
/// Returns Some(index) if a new item was clicked.
///
/// Uses the universal `Button` builder — same styling rules as tabs and the
/// header menu. Active item flips to filled-accent automatically.
pub fn sidebar_nav(
    ui: &mut Ui,
    theme: &Theme,
    items: &[&str],
    active: usize,
) -> Option<usize> {
    let mut clicked = None;
    for (i, label) in items.iter().enumerate() {
        let is_active = i == active;
        if Button::tab(label, is_active).full_width().show(ui, theme) {
            clicked = Some(i);
        }
    }
    clicked
}

/// Horizontal category filter buttons. Returns Some(new_index) if changed.
///
/// Uses the universal `Button` builder with `Small` size and `active` state —
/// same look as tabs and the header menu but at filter-pill size.
pub fn category_filter(
    ui: &mut Ui,
    theme: &Theme,
    categories: &[&str],
    active: usize,
) -> Option<usize> {
    let mut clicked = None;
    ui.horizontal_wrapped(|ui| {
        for (i, cat) in categories.iter().enumerate() {
            let is_active = i == active;
            if Button::tab(cat, is_active)
                .size(ButtonSize::Small)
                .show(ui, theme)
            {
                clicked = Some(i);
            }
        }
    });
    clicked
}

/// Stat card for dashboards (label, big value, optional trend text, optional progress bar).
pub fn stat_card(
    ui: &mut Ui,
    theme: &Theme,
    label: &str,
    value: &str,
    trend: Option<&str>,
    progress: Option<f32>,
) {
    card(ui, theme, |ui| {
        ui.label(RichText::new(label).size(theme.small_size).color(theme.text_muted()));
        ui.label(RichText::new(value).size(theme.heading_size).color(theme.text_primary()).strong());
        if let Some(trend_text) = trend {
            let color = if trend_text.starts_with('+') { theme.success() } else { theme.danger() };
            ui.label(RichText::new(trend_text).size(theme.small_size).color(color));
        }
        if let Some(pct) = progress {
            progress_bar(ui, theme, pct, None);
        }
    });
}

/// Standard page frame. Use this instead of a hardcoded panel fill -- it reads
/// the `bg_panel` theme token, so the Settings colour editor restyles every page.
pub fn page_frame(theme: &Theme) -> egui::Frame {
    egui::Frame::NONE.fill(theme.bg_panel()).inner_margin(theme.card_padding)
}

/// Standard sidebar frame. Use this instead of a hardcoded sidebar fill -- it
/// reads the `bg_sidebar` theme token.
pub fn sidebar_frame(theme: &Theme) -> egui::Frame {
    egui::Frame::NONE.fill(theme.bg_sidebar()).inner_margin(theme.panel_margin)
}

/// Dark sidebar frame (for chat-style panels). Uses bg_sidebar_dark.
pub fn sidebar_dark_frame(theme: &Theme) -> egui::Frame {
    egui::Frame::NONE.fill(theme.bg_sidebar_dark()).inner_margin(0.0)
}

/// Section header with consistent styling across all pages.
pub fn section_header(ui: &mut Ui, theme: &Theme, text: &str) {
    ui.add_space(theme.section_gap);
    ui.label(
        RichText::new(text)
            .size(theme.heading_size)
            .color(theme.text_primary())
            .strong(),
    );
    ui.add_space(theme.row_gap);
}

/// Separator with theme-consistent spacing.
pub fn themed_separator(ui: &mut Ui, theme: &Theme) {
    ui.add_space(theme.section_gap);
    ui.separator();
    ui.add_space(theme.section_gap);
}

/// A 1px full-width divider that slowly cycles through the RGB hue spectrum —
/// drawn between main sections (operator 2026-06-08). Animated: requests a repaint
/// each frame. Reuses `row::rgb_from_time` (same hue helper as the nav separators);
/// the time is scaled so a full sweep takes ~40s ("slow").
pub fn rgb_section_divider(ui: &mut Ui, theme: &Theme) {
    let time = ui.input(|i| i.time);
    let color = row::rgb_from_time(time * 0.3);
    ui.add_space(theme.spacing_sm);
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, Rounding::ZERO, color);
    ui.add_space(theme.spacing_sm);
    ui.ctx().request_repaint(); // keep the colour animating
}

