//! Open Source Tools Catalog — searchable grid of tools with category filters.
//! Reads from the embedded tools catalog (data/tools/catalog.json format).

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Vec2};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;

/// A tool entry (mirrors catalog.json structure).
#[derive(Debug, Clone)]
struct Tool {
    name: &'static str,
    description: &'static str,
    url: &'static str,
    license: &'static str,
    platforms: &'static [&'static str],
    category: &'static str,
}

/// Embedded catalog data (from data/tools/catalog.json).
const TOOLS: &[Tool] = &[
    // 3D Modeling
    Tool { name: "Blender", description: "Industry-standard 3D creation suite.", url: "https://blender.org", license: "GPL", platforms: &["Win", "Mac", "Linux"], category: "3D Modeling" },
    Tool { name: "FreeCAD", description: "Parametric 3D CAD modeler for engineering.", url: "https://freecad.org", license: "LGPL", platforms: &["Win", "Mac", "Linux"], category: "3D Modeling" },
    // Image Editing
    Tool { name: "GIMP", description: "Full-featured image editor and compositor.", url: "https://gimp.org", license: "GPL", platforms: &["Win", "Mac", "Linux"], category: "Image Editing" },
    Tool { name: "Krita", description: "Professional digital painting application.", url: "https://krita.org", license: "GPL", platforms: &["Win", "Mac", "Linux"], category: "Image Editing" },
    // Vector Graphics
    Tool { name: "Inkscape", description: "Professional vector graphics editor.", url: "https://inkscape.org", license: "GPL", platforms: &["Win", "Mac", "Linux"], category: "Vector Graphics" },
    // Documents
    Tool { name: "LibreOffice", description: "Complete office suite.", url: "https://libreoffice.org", license: "MPL", platforms: &["Win", "Mac", "Linux"], category: "Documents" },
    Tool { name: "OnlyOffice", description: "MS Office-compatible suite with collaboration.", url: "https://onlyoffice.com", license: "AGPL", platforms: &["Win", "Mac", "Linux", "Web"], category: "Documents" },
    // Video Editing
    Tool { name: "Kdenlive", description: "Non-linear video editor.", url: "https://kdenlive.org", license: "GPL", platforms: &["Win", "Mac", "Linux"], category: "Video" },
    Tool { name: "DaVinci Resolve", description: "Professional editing and color grading.", url: "https://blackmagicdesign.com/products/davinciresolve", license: "Free tier", platforms: &["Win", "Mac", "Linux"], category: "Video" },
    // Audio
    Tool { name: "Audacity", description: "Multi-track audio editor and recorder.", url: "https://audacityteam.org", license: "GPL", platforms: &["Win", "Mac", "Linux"], category: "Audio" },
    Tool { name: "LMMS", description: "Digital audio workstation with synthesizers.", url: "https://lmms.io", license: "GPL", platforms: &["Win", "Mac", "Linux"], category: "Audio" },
    // Code Editors
    Tool { name: "VS Code", description: "Lightweight code editor with extensions.", url: "https://code.visualstudio.com", license: "MIT", platforms: &["Win", "Mac", "Linux"], category: "Code Editors" },
    Tool { name: "Zed", description: "High-performance editor built in Rust.", url: "https://zed.dev", license: "GPL/AGPL", platforms: &["Mac", "Linux"], category: "Code Editors" },
    Tool { name: "Lapce", description: "Lightning-fast editor with WASI plugins.", url: "https://lapce.dev", license: "Apache-2.0", platforms: &["Win", "Mac", "Linux"], category: "Code Editors" },
    // PDF
    Tool { name: "SumatraPDF", description: "Fast, minimalist PDF/EPUB reader.", url: "https://sumatrapdfreader.org", license: "GPL", platforms: &["Win"], category: "PDF" },
    // Pixel Art
    Tool { name: "LibreSprite", description: "Free pixel art editor with animation.", url: "https://libresprite.github.io", license: "GPL", platforms: &["Win", "Mac", "Linux"], category: "Pixel Art" },
    // Streaming
    Tool { name: "OBS Studio", description: "Streaming and screen recording.", url: "https://obsproject.com", license: "GPL", platforms: &["Win", "Mac", "Linux"], category: "Streaming" },
    Tool { name: "ShareX", description: "Screenshot and recording with annotation.", url: "https://getsharex.com", license: "GPL", platforms: &["Win"], category: "Streaming" },
    // Utilities
    Tool { name: "7-Zip", description: "High compression ratio archiver.", url: "https://7-zip.org", license: "LGPL", platforms: &["Win", "Linux"], category: "Utilities" },
    Tool { name: "KeePassXC", description: "Cross-platform password manager.", url: "https://keepassxc.org", license: "GPL", platforms: &["Win", "Mac", "Linux"], category: "Utilities" },
];

fn all_categories() -> Vec<&'static str> {
    let mut cats: Vec<&str> = TOOLS.iter().map(|t| t.category).collect();
    cats.sort();
    cats.dedup();
    cats
}

/// Local page state.
pub struct ToolsPageState {
    pub search: String,
    pub active_category: Option<&'static str>,
}

impl Default for ToolsPageState {
    fn default() -> Self {
        Self {
            search: String::new(),
            active_category: None,
        }
    }
}

fn with_state<R>(f: impl FnOnce(&mut ToolsPageState) -> R) -> R {
    use std::cell::RefCell;
    thread_local! {
        static STATE: RefCell<ToolsPageState> = RefCell::new(ToolsPageState::default());
    }
    STATE.with(|s| f(&mut s.borrow_mut()))
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(Color32::from_rgb(20, 20, 25)).inner_margin(16.0))
        .show(ctx, |ui| {
            ui.label(
                RichText::new("Open Source Tools")
                    .size(theme.font_size_title)
                    .color(theme.text_primary()),
            );
            ui.add_space(theme.spacing_xs);

            // Search bar
            with_state(|ts| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Search:").color(theme.text_secondary()));
                    ui.add(
                        egui::TextEdit::singleline(&mut ts.search)
                            .desired_width(200.0)
                            .hint_text("Filter tools..."),
                    );
                });
            });
            ui.add_space(theme.spacing_xs);

            // Category filter buttons
            let categories = all_categories();
            with_state(|ts| {
                ui.horizontal_wrapped(|ui| {
                    // "All" button
                    let all_active = ts.active_category.is_none();
                    let all_text = if all_active {
                        RichText::new("All").color(theme.text_on_accent()).size(theme.font_size_small)
                    } else {
                        RichText::new("All").color(theme.text_secondary()).size(theme.font_size_small)
                    };
                    let all_fill = if all_active { theme.accent() } else { Color32::TRANSPARENT };
                    if ui
                        .add(
                            egui::Button::new(all_text)
                                .fill(all_fill)
                                .rounding(Rounding::same(theme.border_radius as u8)),
                        )
                        .clicked()
                    {
                        ts.active_category = None;
                    }

                    for cat in &categories {
                        let is_active = ts.active_category == Some(*cat);
                        let text = if is_active {
                            RichText::new(*cat).color(theme.text_on_accent()).size(theme.font_size_small)
                        } else {
                            RichText::new(*cat).color(theme.text_secondary()).size(theme.font_size_small)
                        };
                        let fill = if is_active { theme.accent() } else { Color32::TRANSPARENT };
                        if ui
                            .add(
                                egui::Button::new(text)
                                    .fill(fill)
                                    .rounding(Rounding::same(theme.border_radius as u8)),
                            )
                            .clicked()
                        {
                            ts.active_category = if is_active { None } else { Some(*cat) };
                        }
                    }
                });
            });

            ui.separator();

            // Tool cards grid
            ScrollArea::vertical()
                .id_salt("tools_grid")
                .show(ui, |ui| {
                    with_state(|ts| {
                        let search_lower = ts.search.to_lowercase();
                        let filtered: Vec<&Tool> = TOOLS
                            .iter()
                            .filter(|t| {
                                let matches_cat = ts
                                    .active_category
                                    .map_or(true, |c| t.category == c);
                                let matches_search = search_lower.is_empty()
                                    || t.name.to_lowercase().contains(&search_lower)
                                    || t.description.to_lowercase().contains(&search_lower)
                                    || t.category.to_lowercase().contains(&search_lower);
                                matches_cat && matches_search
                            })
                            .collect();

                        if filtered.is_empty() {
                            ui.label(
                                RichText::new("No tools match your search.")
                                    .color(theme.text_muted()),
                            );
                        }

                        // Two-column grid layout
                        let cols = 2;
                        egui::Grid::new("tools_card_grid")
                            .num_columns(cols)
                            .spacing(Vec2::new(theme.spacing_sm, theme.spacing_sm))
                            .show(ui, |ui| {
                                for (i, tool) in filtered.iter().enumerate() {
                                    egui::Frame::none()
                                        .fill(theme.bg_card())
                                        .rounding(Rounding::same(theme.border_radius as u8))
                                        .inner_margin(theme.card_padding)
                                        .stroke(egui::Stroke::new(1.0, theme.border()))
                                        .show(ui, |ui| {
                                            ui.set_min_width(260.0);
                                            // Name
                                            ui.label(
                                                RichText::new(tool.name)
                                                    .size(theme.font_size_body)
                                                    .color(theme.text_primary())
                                                    .strong(),
                                            );
                                            // Category badge
                                            ui.horizontal(|ui| {
                                                egui::Frame::none()
                                                    .fill(Theme::c32(&theme.info))
                                                    .rounding(Rounding::same(3))
                                                    .inner_margin(Vec2::new(6.0, 2.0))
                                                    .show(ui, |ui| {
                                                        ui.label(
                                                            RichText::new(tool.category)
                                                                .size(theme.font_size_small)
                                                                .color(Color32::WHITE),
                                                        );
                                                    });
                                                ui.label(
                                                    RichText::new(tool.license)
                                                        .size(theme.font_size_small)
                                                        .color(theme.text_muted()),
                                                );
                                            });
                                            // Description
                                            ui.label(
                                                RichText::new(tool.description)
                                                    .size(theme.font_size_small)
                                                    .color(theme.text_secondary()),
                                            );
                                            // Platforms
                                            ui.label(
                                                RichText::new(tool.platforms.join(", "))
                                                    .size(theme.font_size_small)
                                                    .color(theme.text_muted()),
                                            );
                                            // Download link
                                            if ui.small_button("Download").clicked() {
                                                ui.ctx().open_url(egui::OpenUrl::new_tab(tool.url));
                                            }
                                        });

                                    if (i + 1) % cols == 0 {
                                        ui.end_row();
                                    }
                                }
                            });
                    });
                });
        });
}
