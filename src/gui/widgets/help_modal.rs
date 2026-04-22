//! Universal help modal. Mirrors the web `window.hosHelp` system.
//!
//! Both UIs read from `data/help/topics.json` so editing that file updates
//! both the desktop and web help content in one shot.
//!
//! Typical use:
//!   - At startup: `gui_state.help_registry = load_help_registry(&data_dir);`
//!   - Anywhere you want a help affordance:
//!       `widgets::help_modal::help_button(ui, theme, "real-sim", &mut gui_state.active_help_topic);`
//!   - In the top-level render loop, after drawing pages:
//!       `widgets::help_modal::draw(ctx, theme, &gui_state.help_registry,
//!                                  &mut gui_state.active_help_topic);`

use egui::{Align2, Context, Frame, RichText, Rounding, Sense, Stroke, Ui, Vec2};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

use crate::gui::theme::Theme;

/// A single help topic loaded from `data/help/topics.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct HelpTopic {
    pub title: String,
    #[serde(default)]
    pub body: Vec<String>,
}

/// Shape of `data/help/topics.json`.
#[derive(Debug, Clone, Deserialize)]
struct HelpTopicsFile {
    #[serde(default = "default_version")]
    #[allow(dead_code)]
    version: u32,
    topics: HashMap<String, HelpTopic>,
}

fn default_version() -> u32 { 1 }

/// Runtime help-topic registry. Populate from JSON at startup.
#[derive(Default, Debug, Clone)]
pub struct HelpRegistry {
    topics: HashMap<String, HelpTopic>,
}

impl HelpRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn get(&self, id: &str) -> Option<&HelpTopic> {
        self.topics.get(id)
    }

    pub fn len(&self) -> usize { self.topics.len() }
    pub fn is_empty(&self) -> bool { self.topics.is_empty() }
}

/// Load help topics from `data/help/topics.json`.
/// Returns an empty registry on any error so startup does not fail.
pub fn load_help_registry(data_dir: &Path) -> HelpRegistry {
    let path = data_dir.join("help").join("topics.json");
    let bytes = match std::fs::read_to_string(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[help] Could not read {}: {}", path.display(), e);
            return HelpRegistry::new();
        }
    };
    let parsed: HelpTopicsFile = match serde_json::from_str(&bytes) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[help] Could not parse topics.json: {}", e);
            return HelpRegistry::new();
        }
    };
    log::info!("Loaded {} help topics from {}", parsed.topics.len(), path.display());
    HelpRegistry { topics: parsed.topics }
}

/// Strip simple inline HTML-like tags from a string so native can render plain text.
/// Recognises `<strong>`, `</strong>`, `<em>`, `</em>`, etc. Drops anything in `<...>`.
fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

/// Render a small "?" help button. Clicking it sets `active_topic` to `topic_id`.
/// Returns true if clicked this frame.
pub fn help_button(
    ui: &mut Ui,
    theme: &Theme,
    topic_id: &str,
    active_topic: &mut Option<String>,
) -> bool {
    let size = Vec2::new(18.0, 18.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let (stroke_color, text_color) = if response.hovered() {
            (theme.accent(), theme.accent())
        } else {
            (theme.border(), theme.text_muted())
        };
        painter.circle_stroke(rect.center(), 8.0, Stroke::new(1.0, stroke_color));
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "?",
            egui::FontId::proportional(10.0),
            text_color,
        );
    }

    if response.clicked() {
        *active_topic = Some(topic_id.to_string());
        true
    } else {
        false
    }
}

/// Draw the help modal if an active topic is set. Call this once per frame after
/// drawing the main page content, so the modal overlays everything.
pub fn draw(
    ctx: &Context,
    theme: &Theme,
    registry: &HelpRegistry,
    active_topic: &mut Option<String>,
) {
    let topic_id = match active_topic.clone() {
        Some(id) => id,
        None => return,
    };

    let topic = match registry.get(&topic_id) {
        Some(t) => t.clone(),
        None => {
            // Unknown topic — clear and bail.
            *active_topic = None;
            return;
        }
    };

    let mut should_close = false;

    // Backdrop
    let screen = ctx.screen_rect();
    let bg_modal = Theme::c32(&theme.bg_modal);
    egui::Area::new(egui::Id::new("hos_help_backdrop"))
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (_, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(screen, Rounding::ZERO, bg_modal);
            if resp.clicked() {
                should_close = true;
            }
        });

    // Modal window
    let modal_w = theme.modal_width;
    egui::Window::new(&topic.title)
        .id(egui::Id::new("hos_help_modal"))
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .fixed_size(Vec2::new(modal_w, 0.0))
        .title_bar(true)
        .frame(
            Frame::none()
                .fill(theme.bg_card())
                .rounding(Rounding::same(theme.border_radius_lg as u8))
                .inner_margin(theme.card_padding)
                .stroke(Stroke::new(1.0, theme.border())),
        )
        .show(ctx, |ui| {
            // Body paragraphs
            for paragraph in &topic.body {
                let plain = strip_html(paragraph);
                ui.label(
                    RichText::new(plain)
                        .size(theme.font_size_body)
                        .color(theme.text_secondary()),
                );
                ui.add_space(theme.spacing_sm);
            }

            ui.add_space(theme.spacing_md);
            ui.separator();
            ui.add_space(theme.spacing_sm);

            ui.horizontal(|ui| {
                ui.with_layout(
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        let btn = egui::Button::new(
                            RichText::new("Got it")
                                .color(theme.text_on_accent())
                                .size(theme.font_size_body),
                        )
                        .fill(theme.accent())
                        .rounding(Rounding::same(theme.border_radius as u8));
                        if ui.add(btn).clicked() {
                            should_close = true;
                        }
                    },
                );
            });
        });

    // Close via Escape
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        should_close = true;
    }

    if should_close {
        *active_topic = None;
    }
}
