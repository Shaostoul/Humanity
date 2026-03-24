//! Marketplace — browse, search, and create listings.
//!
//! Search bar at top, category filter sidebar on left, listing cards grid,
//! detail modal on click, and "Create Listing" form.

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Vec2};
use crate::gui::{GuiState, GuiListing};
use crate::gui::theme::Theme;
use crate::gui::widgets;

const CATEGORIES: &[&str] = &["All", "Tools", "Materials", "Food", "Equipment", "Services", "Other"];

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // Detail modal (drawn first so it overlays everything)
    let mut close_detail = false;
    if let Some(sel_idx) = state.listing_selected {
        if let Some(listing) = state.listings.get(sel_idx) {
            let title = listing.title.clone();
            let desc = listing.description.clone();
            let price = listing.price;
            let seller = listing.seller.clone();
            let category = listing.category.clone();

            egui::Window::new("Listing Detail")
                .resizable(false)
                .collapsible(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .fixed_size(Vec2::new(theme.modal_width, 350.0))
                .show(ctx, |ui| {
                    ui.label(RichText::new(&title).size(theme.font_size_title).color(theme.accent()));
                    ui.add_space(theme.spacing_sm);

                    // Category badge
                    egui::Frame::none()
                        .fill(Theme::c32(&theme.info))
                        .rounding(Rounding::same(3))
                        .inner_margin(Vec2::new(6.0, 2.0))
                        .show(ui, |ui| {
                            ui.label(RichText::new(&category).size(theme.font_size_small).color(Color32::WHITE));
                        });

                    ui.add_space(theme.spacing_sm);

                    // Image placeholder
                    let (img_rect, _) = ui.allocate_exact_size(Vec2::new(200.0, 120.0), egui::Sense::hover());
                    ui.painter().rect_filled(img_rect, Rounding::same(4), theme.bg_secondary());
                    ui.painter().text(
                        img_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "No Image",
                        egui::FontId::proportional(14.0),
                        theme.text_muted(),
                    );

                    ui.add_space(theme.spacing_sm);

                    ui.label(RichText::new(&desc).color(theme.text_secondary()));
                    ui.add_space(theme.spacing_sm);

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Seller:").color(theme.text_muted()));
                        ui.label(RichText::new(&seller).color(theme.text_primary()));
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Price:").color(theme.text_muted()));
                        ui.label(RichText::new(format!("{:.2} SOL", price)).size(theme.font_size_heading).color(theme.accent()));
                    });

                    ui.add_space(theme.spacing_md);
                    ui.horizontal(|ui| {
                        if widgets::primary_button(ui, theme, "Buy") {
                            // Placeholder: buying not implemented
                        }
                        if widgets::secondary_button(ui, theme, "Close") {
                            close_detail = true;
                        }
                    });
                });
        }
    }
    if close_detail {
        state.listing_selected = None;
    }

    egui::CentralPanel::default()
        .frame(Frame::none().fill(Color32::from_rgb(20, 20, 25)).inner_margin(16.0))
        .show(ctx, |ui| {
            // Header
            ui.horizontal(|ui| {
                ui.label(RichText::new("Marketplace").size(theme.font_size_title).color(theme.text_primary()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::primary_button(ui, theme, "+ Create Listing") {
                        state.listing_show_new_form = !state.listing_show_new_form;
                    }
                });
            });

            ui.add_space(theme.spacing_sm);

            // Search bar
            ui.horizontal(|ui| {
                ui.label(RichText::new("Search:").color(theme.text_secondary()));
                ui.add(egui::TextEdit::singleline(&mut state.listing_search)
                    .desired_width(300.0)
                    .hint_text("Search listings..."));
            });

            ui.add_space(theme.spacing_sm);

            // New listing form
            if state.listing_show_new_form {
                widgets::card(ui, theme, |ui| {
                    ui.label(RichText::new("Create Listing").size(theme.font_size_heading).color(theme.accent()));
                    ui.add_space(theme.spacing_xs);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Title:").color(theme.text_secondary()));
                        ui.add(egui::TextEdit::singleline(&mut state.listing_new_title).desired_width(300.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Description:").color(theme.text_secondary()));
                        ui.add(egui::TextEdit::multiline(&mut state.listing_new_description)
                            .desired_width(300.0)
                            .desired_rows(2));
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Price (SOL):").color(theme.text_secondary()));
                        ui.add(egui::TextEdit::singleline(&mut state.listing_new_price).desired_width(100.0));
                        ui.add_space(theme.spacing_md);
                        ui.label(RichText::new("Category:").color(theme.text_secondary()));
                        egui::ComboBox::from_id_salt("new_listing_category")
                            .selected_text(if state.listing_new_category.is_empty() { "Select..." } else { &state.listing_new_category })
                            .show_ui(ui, |ui| {
                                for &cat in &CATEGORIES[1..] { // Skip "All"
                                    if ui.selectable_label(state.listing_new_category == cat, cat).clicked() {
                                        state.listing_new_category = cat.to_string();
                                    }
                                }
                            });
                    });
                    ui.add_space(theme.spacing_xs);
                    ui.horizontal(|ui| {
                        if widgets::primary_button(ui, theme, "Create") && !state.listing_new_title.is_empty() {
                            let price = state.listing_new_price.parse::<f64>().unwrap_or(0.0);
                            let listing = GuiListing {
                                id: state.listing_next_id,
                                title: state.listing_new_title.clone(),
                                description: state.listing_new_description.clone(),
                                price,
                                seller: if state.user_name.is_empty() { "You".to_string() } else { state.user_name.clone() },
                                category: if state.listing_new_category.is_empty() { "Other".to_string() } else { state.listing_new_category.clone() },
                            };
                            state.listing_next_id += 1;
                            state.listings.push(listing);
                            state.listing_new_title.clear();
                            state.listing_new_description.clear();
                            state.listing_new_price.clear();
                            state.listing_new_category.clear();
                            state.listing_show_new_form = false;
                        }
                        if widgets::secondary_button(ui, theme, "Cancel") {
                            state.listing_show_new_form = false;
                        }
                    });
                });
                ui.add_space(theme.spacing_sm);
            }

            // Main area: category sidebar + listing grid
            let search_lower = state.listing_search.to_lowercase();
            let filter_cat = state.listing_filter_category.clone();

            ui.columns(2, |cols| {
                // Left: category filter
                cols[0].set_max_width(140.0);
                cols[0].vertical(|ui| {
                    ui.label(RichText::new("Categories").size(theme.font_size_body).color(theme.text_primary()));
                    ui.add_space(theme.spacing_xs);
                    for &cat in CATEGORIES {
                        let is_active = (cat == "All" && filter_cat.is_empty()) || filter_cat == cat;
                        let text_color = if is_active { theme.accent() } else { theme.text_secondary() };
                        if ui.selectable_label(is_active, RichText::new(cat).color(text_color)).clicked() {
                            state.listing_filter_category = if cat == "All" { String::new() } else { cat.to_string() };
                        }
                    }
                });

                // Right: listing cards grid
                cols[1].vertical(|ui| {
                    let filtered: Vec<usize> = state.listings.iter().enumerate()
                        .filter(|(_, l)| {
                            if !search_lower.is_empty()
                                && !l.title.to_lowercase().contains(&search_lower)
                                && !l.description.to_lowercase().contains(&search_lower)
                            {
                                return false;
                            }
                            if !state.listing_filter_category.is_empty() && l.category != state.listing_filter_category {
                                return false;
                            }
                            true
                        })
                        .map(|(i, _)| i)
                        .collect();

                    if filtered.is_empty() {
                        ui.add_space(theme.spacing_xl);
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new("No listings").size(theme.font_size_heading).color(theme.text_muted()));
                            ui.label(RichText::new("Create one to get started.").color(theme.text_secondary()));
                        });
                    } else {
                        ScrollArea::vertical().show(ui, |ui| {
                            // 2-column card grid
                            let mut iter = filtered.iter();
                            loop {
                                let a = iter.next();
                                if a.is_none() { break; }
                                let b = iter.next();
                                ui.horizontal(|ui| {
                                    for idx_opt in [a, b] {
                                        if let Some(&idx) = idx_opt {
                                            let listing = &state.listings[idx];
                                            let card_width = 300.0;
                                            ui.allocate_ui(Vec2::new(card_width, 100.0), |ui| {
                                                widgets::card(ui, theme, |ui| {
                                                    ui.horizontal(|ui| {
                                                        ui.label(RichText::new(&listing.title).color(theme.text_primary()));
                                                        egui::Frame::none()
                                                            .fill(Theme::c32(&theme.info))
                                                            .rounding(Rounding::same(3))
                                                            .inner_margin(Vec2::new(4.0, 1.0))
                                                            .show(ui, |ui| {
                                                                ui.label(RichText::new(&listing.category).size(theme.font_size_small).color(Color32::WHITE));
                                                            });
                                                    });
                                                    ui.label(RichText::new(format!("{:.2} SOL", listing.price)).color(theme.accent()));
                                                    ui.label(RichText::new(format!("Seller: {}", listing.seller)).color(theme.text_muted()).size(theme.font_size_small));
                                                    if widgets::secondary_button(ui, theme, "View") {
                                                        state.listing_selected = Some(idx);
                                                    }
                                                });
                                            });
                                        }
                                    }
                                });
                                ui.add_space(theme.spacing_xs);
                            }
                        });
                    }
                });
            });
        });
}
