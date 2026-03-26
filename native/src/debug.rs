//! In-app debug console: captures log messages for display in an egui overlay.
//!
//! Uses a global ring buffer so any code can call `push_debug()` to log a
//! timestamped message. The GUI drains this buffer each frame into
//! `GuiState::debug_log` for rendering.

use std::sync::Mutex;

/// Maximum number of entries kept in the global debug ring buffer.
const MAX_DEBUG_ENTRIES: usize = 500;

/// Global debug log buffer, drained each frame by the render loop.
static DEBUG_LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Push a timestamped debug message into the global buffer.
///
/// Safe to call from any thread. Messages exceeding `MAX_DEBUG_ENTRIES`
/// cause the oldest entry to be discarded.
pub fn push_debug(msg: impl Into<String>) {
    if let Ok(mut log) = DEBUG_LOG.lock() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let secs = now.as_secs();
        let hours = (secs / 3600) % 24;
        let mins = (secs / 60) % 60;
        let s = secs % 60;
        let ms = now.subsec_millis();
        let stamped = format!("[{:02}:{:02}:{:02}.{:03}] {}", hours, mins, s, ms, msg.into());
        log.push(stamped);
        if log.len() > MAX_DEBUG_ENTRIES {
            log.remove(0);
        }
    }
}

/// Drain all pending debug messages from the global buffer.
///
/// Called once per frame by the render loop to transfer messages into
/// `GuiState::debug_log` for the overlay to display.
pub fn drain_debug_log() -> Vec<String> {
    if let Ok(mut log) = DEBUG_LOG.lock() {
        std::mem::take(&mut *log)
    } else {
        Vec::new()
    }
}

/// Draw the debug console overlay (F12 toggle).
///
/// Renders a semi-transparent panel at the bottom of the screen showing
/// the most recent log lines. Includes a scrollable area and a Clear button.
#[cfg(feature = "native")]
pub fn draw_debug_console(ctx: &egui::Context, debug_log: &mut Vec<String>, visible: &mut bool) {
    // F12 toggle
    if ctx.input(|i| i.key_pressed(egui::Key::F12)) {
        *visible = !*visible;
    }

    if !*visible {
        return;
    }

    let screen = ctx.screen_rect();
    let panel_height = (screen.height() * 0.35).min(300.0);

    egui::Area::new(egui::Id::new("debug_console_area"))
        .fixed_pos(egui::pos2(0.0, screen.height() - panel_height))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let panel_rect = egui::Rect::from_min_size(
                egui::pos2(0.0, screen.height() - panel_height),
                egui::vec2(screen.width(), panel_height),
            );
            ui.set_clip_rect(panel_rect);

            egui::Frame::NONE
                .fill(egui::Color32::from_rgba_premultiplied(15, 15, 20, 220))
                .inner_margin(egui::Margin::same(6))
                .show(ui, |ui| {
                    ui.set_min_size(egui::vec2(screen.width() - 12.0, panel_height - 12.0));

                    // Header row
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Debug Console (F12)")
                                .size(13.0)
                                .color(egui::Color32::from_rgb(237, 140, 36))
                                .strong()
                                .family(egui::FontFamily::Monospace),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("Clear").clicked() {
                                debug_log.clear();
                            }
                            ui.label(
                                egui::RichText::new(format!("{} entries", debug_log.len()))
                                    .size(11.0)
                                    .color(egui::Color32::from_rgb(140, 140, 160))
                                    .family(egui::FontFamily::Monospace),
                            );
                        });
                    });

                    ui.separator();

                    // Scrollable log area
                    let scroll_height = panel_height - 50.0;
                    egui::ScrollArea::vertical()
                        .max_height(scroll_height)
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for line in debug_log.iter() {
                                ui.label(
                                    egui::RichText::new(line)
                                        .size(11.0)
                                        .color(egui::Color32::from_rgb(200, 200, 210))
                                        .family(egui::FontFamily::Monospace),
                                );
                            }
                        });
                });
        });
}
