//! Curated Resources page — context-aware (Real/Sim) resource directory.

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Vec2};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;

/// A single resource entry.
struct Resource {
    title: &'static str,
    description: &'static str,
    url: &'static str,
}

/// A category of resources.
struct ResourceCategory {
    name: &'static str,
    real_resources: &'static [Resource],
    sim_resources: &'static [Resource],
}

const CATEGORIES: &[ResourceCategory] = &[
    ResourceCategory {
        name: "Education",
        real_resources: &[
            Resource { title: "Khan Academy", description: "Free courses in math, science, computing, and more.", url: "https://khanacademy.org" },
            Resource { title: "MIT OpenCourseWare", description: "Free lecture notes, exams, and videos from MIT.", url: "https://ocw.mit.edu" },
            Resource { title: "Coursera", description: "Online courses from top universities worldwide.", url: "https://coursera.org" },
        ],
        sim_resources: &[
            Resource { title: "Farming Guide", description: "Learn crop rotation, soil types, and seasonal planting.", url: "#farming" },
            Resource { title: "Crafting Handbook", description: "Recipes, materials, and crafting station requirements.", url: "#crafting" },
            Resource { title: "Survival Basics", description: "Water, shelter, food — the essentials for new players.", url: "#survival" },
        ],
    },
    ResourceCategory {
        name: "Health",
        real_resources: &[
            Resource { title: "WHO Resources", description: "World Health Organization health topics and data.", url: "https://who.int" },
            Resource { title: "Medline Plus", description: "Health information from the National Library of Medicine.", url: "https://medlineplus.gov" },
            Resource { title: "Crisis Text Line", description: "Text HOME to 741741 for free crisis counseling.", url: "https://crisistextline.org" },
        ],
        sim_resources: &[
            Resource { title: "Health System", description: "How hunger, thirst, and injury affect your character.", url: "#health" },
            Resource { title: "Medicine Crafting", description: "Recipes for bandages, potions, and medical supplies.", url: "#medicine" },
        ],
    },
    ResourceCategory {
        name: "Legal",
        real_resources: &[
            Resource { title: "Legal Aid", description: "Find free legal help in your area.", url: "https://lawhelp.org" },
            Resource { title: "Know Your Rights", description: "ACLU guide to your constitutional rights.", url: "https://aclu.org/know-your-rights" },
        ],
        sim_resources: &[
            Resource { title: "Guild Laws", description: "Rules and governance within player guilds.", url: "#guilds" },
            Resource { title: "Trade Regulations", description: "Fair trading rules and dispute resolution.", url: "#trade-rules" },
        ],
    },
    ResourceCategory {
        name: "Housing",
        real_resources: &[
            Resource { title: "HUD Resources", description: "US Dept of Housing: rental assistance, fair housing.", url: "https://hud.gov" },
            Resource { title: "Habitat for Humanity", description: "Affordable housing and home repair assistance.", url: "https://habitat.org" },
        ],
        sim_resources: &[
            Resource { title: "Building Guide", description: "How to construct shelters, bases, and advanced structures.", url: "#building" },
            Resource { title: "Base Defense", description: "Fortification and perimeter security for your base.", url: "#defense" },
        ],
    },
    ResourceCategory {
        name: "Food",
        real_resources: &[
            Resource { title: "Feeding America", description: "Find local food banks and meal programs.", url: "https://feedingamerica.org" },
            Resource { title: "SNAP Benefits", description: "Apply for food assistance (SNAP/EBT).", url: "https://fns.usda.gov/snap" },
        ],
        sim_resources: &[
            Resource { title: "Farming System", description: "Crop growth, irrigation, and harvest mechanics.", url: "#farming-sys" },
            Resource { title: "Cooking Recipes", description: "Food recipes with buff effects and nutrition values.", url: "#cooking" },
        ],
    },
    ResourceCategory {
        name: "Technology",
        real_resources: &[
            Resource { title: "FreeCodeCamp", description: "Learn to code for free with interactive lessons.", url: "https://freecodecamp.org" },
            Resource { title: "Open Source Guides", description: "How to contribute to and maintain open source projects.", url: "https://opensource.guide" },
        ],
        sim_resources: &[
            Resource { title: "Tech Tree", description: "Research progression and technology unlocks.", url: "#tech-tree" },
            Resource { title: "Automation", description: "Setting up conveyor belts, sorters, and auto-crafters.", url: "#automation" },
        ],
    },
];

/// Local page state.
pub struct ResourcesPageState {
    pub selected_category: usize,
}

impl Default for ResourcesPageState {
    fn default() -> Self {
        Self {
            selected_category: 0,
        }
    }
}

fn with_state<R>(f: impl FnOnce(&mut ResourcesPageState) -> R) -> R {
    use std::cell::RefCell;
    thread_local! {
        static STATE: RefCell<ResourcesPageState> = RefCell::new(ResourcesPageState::default());
    }
    STATE.with(|s| f(&mut s.borrow_mut()))
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let is_real = state.context_real;

    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(16.0))
        .show(ctx, |ui| {
            // Header
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Resources")
                        .size(theme.font_size_title)
                        .color(theme.text_primary()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mode_label = if is_real { "Real" } else { "Sim" };
                    let mode_color = if is_real { theme.success() } else { Theme::c32(&theme.info) };
                    egui::Frame::none()
                        .fill(mode_color)
                        .rounding(Rounding::same(3))
                        .inner_margin(Vec2::new(8.0, 3.0))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(mode_label)
                                    .size(theme.font_size_small)
                                    .color(Color32::WHITE),
                            );
                        });
                });
            });
            ui.separator();

            ui.columns(2, |cols| {
                // Left: category list
                cols[0].label(
                    RichText::new("Categories")
                        .size(theme.font_size_body)
                        .color(theme.text_secondary()),
                );
                cols[0].add_space(theme.spacing_xs);

                with_state(|rs| {
                    for (i, cat) in CATEGORIES.iter().enumerate() {
                        let selected = rs.selected_category == i;
                        let fill = if selected {
                            theme.bg_card()
                        } else {
                            Color32::TRANSPARENT
                        };
                        egui::Frame::none()
                            .fill(fill)
                            .rounding(Rounding::same(theme.border_radius as u8))
                            .inner_margin(Vec2::new(12.0, 6.0))
                            .show(&mut cols[0], |ui| {
                                let text_color = if selected {
                                    theme.accent()
                                } else {
                                    theme.text_primary()
                                };
                                let resp = ui.selectable_label(
                                    false,
                                    RichText::new(cat.name).color(text_color),
                                );
                                if resp.clicked() {
                                    rs.selected_category = i;
                                }
                            });
                    }
                });

                // Right: resource cards
                with_state(|rs| {
                    let cat = &CATEGORIES[rs.selected_category];
                    let resources = if is_real {
                        cat.real_resources
                    } else {
                        cat.sim_resources
                    };

                    cols[1].label(
                        RichText::new(cat.name)
                            .size(theme.font_size_body)
                            .color(theme.accent()),
                    );
                    cols[1].add_space(theme.spacing_xs);

                    ScrollArea::vertical()
                        .id_salt("resource_cards")
                        .show(&mut cols[1], |ui| {
                            for res in resources {
                                widgets::card(ui, theme, |ui| {
                                    ui.label(
                                        RichText::new(res.title)
                                            .size(theme.font_size_body)
                                            .color(theme.text_primary())
                                            .strong(),
                                    );
                                    ui.label(
                                        RichText::new(res.description)
                                            .size(theme.font_size_small)
                                            .color(theme.text_secondary()),
                                    );
                                    ui.label(
                                        RichText::new(res.url)
                                            .size(theme.font_size_small)
                                            .color(Theme::c32(&theme.info)),
                                    );
                                });
                                ui.add_space(4.0);
                            }
                        });
                });
            });
        });
}
