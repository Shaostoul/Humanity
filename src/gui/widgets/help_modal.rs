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
    /// Which topics belong to which page, keyed by WEB ROUTE ("/chat" -> ["chat-context-menu", ...]).
    /// This map already existed for the web client; native reads the same one so a
    /// page's help is written once and shows up in both UIs.
    #[serde(default)]
    pages: HashMap<String, Vec<String>>,
    /// Alias map for native screens whose name does not derive to a web route.
    /// GuiPage variant name -> a key in `pages` (e.g. "Quests" -> "/onboarding").
    /// Data, not a match arm, so pairing a new page with existing help is a
    /// one-line edit to the JSON. The web reader ignores unknown top-level keys.
    #[serde(default)]
    native_pages: HashMap<String, String>,
}

fn default_version() -> u32 { 1 }

/// Runtime help-topic registry. Populate from JSON at startup.
#[derive(Default, Debug, Clone)]
pub struct HelpRegistry {
    topics: HashMap<String, HelpTopic>,
    /// Route -> topic ids, straight from the JSON's shared `pages` map.
    pages: HashMap<String, Vec<String>>,
    /// GuiPage name -> route alias, for screens whose name is not the route.
    native_pages: HashMap<String, String>,
}

impl HelpRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn get(&self, id: &str) -> Option<&HelpTopic> {
        self.topics.get(id)
    }

    /// The prose topics that belong to a native page, in the order the content
    /// author listed them. Empty when nobody has written help for this screen,
    /// so the caller renders keys only instead of an empty prose section.
    ///
    /// `page_name` is the `GuiPage` variant name (e.g. "Wallet"). Resolution:
    ///   1. `native_pages["Wallet"]` if present, for screens whose name is not
    ///      the web route (GuiPage::Quests is the web's "/onboarding" page);
    ///   2. otherwise the derived route "/" + lowercased name, which already
    ///      hits for most pages ("Chat" -> "/chat", "Studio" -> "/studio").
    /// Both land in the SAME `pages` map the web client uses, so a topic added
    /// for a web route immediately shows up on the matching native screen.
    pub fn topics_for_page(&self, page_name: &str) -> Vec<&HelpTopic> {
        let derived = format!("/{}", page_name.to_lowercase());
        let route = self.native_pages.get(page_name).map(|s| s.as_str()).unwrap_or(&derived);
        self.pages
            .get(route)
            .map(|ids| ids.iter().filter_map(|id| self.topics.get(id)).collect())
            .unwrap_or_default()
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
    HelpRegistry {
        topics: parsed.topics,
        pages: parsed.pages,
        native_pages: parsed.native_pages,
    }
}

/// Strip simple inline HTML-like tags from a string so native can render plain text.
/// Recognises `<strong>`, `</strong>`, `<em>`, `</em>`, etc. Drops anything in `<...>`.
/// `pub(crate)` because the top-right help panel (pages/keymap.rs) renders the same
/// topic bodies and must strip them the same way.
pub(crate) fn strip_html(s: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> HelpRegistry {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        load_help_registry(&dir)
    }

    /// `load_help_registry` swallows every error and returns an EMPTY registry, so a
    /// stray comma in topics.json would not crash or warn - the help panel would just
    /// quietly show no prose anywhere, on both clients. This is the gate against that.
    #[test]
    fn shipped_help_topics_parse() {
        let reg = shipped();
        assert!(!reg.is_empty(), "data/help/topics.json produced no topics (parse error?)");
    }

    /// The native panel finds a page's prose by deriving a web route from the page
    /// name. If that derivation ever stops matching the shared `pages` map, every
    /// page silently loses its prose half while still looking fine (keys still show).
    #[test]
    fn a_derived_route_page_resolves_its_topics() {
        let reg = shipped();
        assert!(
            !reg.topics_for_page("Chat").is_empty(),
            "GuiPage::Chat should derive to the \"/chat\" entry of the shared pages map"
        );
    }

    /// Every alias in `native_pages` must point at a route that exists AND carries at
    /// least one real topic. A typo here is invisible at runtime (you just get no
    /// prose), which is exactly the kind of silent miss this repo keeps getting bitten
    /// by, so it is asserted instead of trusted.
    #[test]
    fn every_native_page_alias_points_at_real_topics() {
        let reg = shipped();
        for (page, route) in &reg.native_pages {
            let ids = reg
                .pages
                .get(route)
                .unwrap_or_else(|| panic!("native_pages[{page}] -> {route}, which is not a key in `pages`"));
            assert!(
                ids.iter().any(|id| reg.topics.contains_key(id)),
                "native_pages[{page}] -> {route} lists no topic that actually exists"
            );
        }
    }
}
