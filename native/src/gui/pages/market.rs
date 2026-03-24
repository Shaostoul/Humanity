//! Marketplace — browse, search, and create listings.
//!
//! Left sidebar: category filter. Center: search bar, sort dropdown, listing card grid.
//! Detail view on click. "Create Listing" form.

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Vec2};
use crate::gui::{GuiState, GuiListing};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use std::cell::RefCell;

const CATEGORIES: &[&str] = &[
    "All", "Tools", "Materials", "Food", "Equipment",
    "Vehicles", "Electronics", "Services", "Other",
];

/// Sort options for listings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortOrder {
    Newest,
    PriceLowHigh,
    PriceHighLow,
    Relevance,
}

impl SortOrder {
    fn label(self) -> &'static str {
        match self {
            Self::Newest => "Newest",
            Self::PriceLowHigh => "Price: Low to High",
            Self::PriceHighLow => "Price: High to Low",
            Self::Relevance => "Relevance",
        }
    }
}

/// Page-local state for the marketplace.
struct MarketPageState {
    sort_order: SortOrder,
    detail_view: bool,
}

impl Default for MarketPageState {
    fn default() -> Self {
        Self {
            sort_order: SortOrder::Newest,
            detail_view: false,
        }
    }
}

fn with_state<R>(f: impl FnOnce(&mut MarketPageState) -> R) -> R {
    thread_local! {
        static STATE: RefCell<MarketPageState> = RefCell::new(MarketPageState::default());
    }
    STATE.with(|s| f(&mut s.borrow_mut()))
}

fn category_color(category: &str) -> Color32 {
    match category {
        "Tools" => Color32::from_rgb(70, 130, 180),
        "Materials" => Color32::from_rgb(139, 119, 101),
        "Food" => Color32::from_rgb(60, 150, 60),
        "Equipment" => Color32::from_rgb(180, 140, 50),
        "Vehicles" => Color32::from_rgb(140, 80, 160),
        "Electronics" => Color32::from_rgb(50, 150, 200),
        "Services" => Color32::from_rgb(200, 100, 80),
        _ => Color32::from_rgb(120, 120, 130),
    }
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // Detail view (replaces center panel when viewing a listing)
    let showing_detail = with_state(|ps| ps.detail_view) && state.listing_selected.is_some();
    let mut close_detail = false;

    // Left sidebar: category filter
    egui::SidePanel::left("market_categories")
        .min_width(140.0)
        .max_width(170.0)
        .frame(Frame::none().fill(Color32::from_rgb(22, 22, 28)).inner_margin(10.0))
        .show(ctx, |ui| {
            ui.label(RichText::new("Categories").size(theme.font_size_heading).color(theme.text_primary()));
            ui.add_space(theme.spacing_xs);
            ui.separator();
            ui.add_space(theme.spacing_xs);

            for &cat in CATEGORIES {
                let is_active = (cat == "All" && state.listing_filter_category.is_empty())
                    || state.listing_filter_category == cat;
                let text_color = if is_active { theme.accent() } else { theme.text_secondary() };

                ui.horizontal(|ui| {
                    if cat != "All" {
                        let (dot_rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), egui::Sense::hover());
                        ui.painter().circle_filled(dot_rect.center(), 4.0, category_color(cat));
                    }
                    if ui.selectable_label(is_active, RichText::new(cat).color(text_color)).clicked() {
                        state.listing_filter_category = if cat == "All" { String::new() } else { cat.to_string() };
                    }
                });
            }
        });

    egui::CentralPanel::default()
        .frame(Frame::none().fill(Color32::from_rgb(20, 20, 25)).inner_margin(16.0))
        .show(ctx, |ui| {
            if showing_detail {
                // ── Detail view ──
                if let Some(sel_idx) = state.listing_selected {
                    if let Some(listing) = state.listings.get(sel_idx) {
                        ScrollArea::vertical().show(ui, |ui| {
                            // Back button
                            ui.horizontal(|ui| {
                                if widgets::secondary_button(ui, theme, "< Back to Listings") {
                                    close_detail = true;
                                }
                            });
                            ui.add_space(theme.spacing_sm);

                            ui.label(RichText::new(&listing.title).size(theme.font_size_title).color(theme.accent()));
                            ui.add_space(theme.spacing_xs);

                            // Category badge
                            let cat_col = category_color(&listing.category);
                            egui::Frame::none()
                                .fill(cat_col)
                                .rounding(Rounding::same(3))
                                .inner_margin(Vec2::new(6.0, 2.0))
                                .show(ui, |ui| {
                                    ui.label(RichText::new(&listing.category).size(theme.font_size_small).color(Color32::WHITE));
                                });

                            ui.add_space(theme.spacing_md);

                            // Image placeholder
                            let (img_rect, _) = ui.allocate_exact_size(Vec2::new(300.0, 180.0), egui::Sense::hover());
                            ui.painter().rect_filled(img_rect, Rounding::same(6), theme.bg_secondary());
                            ui.painter().text(
                                img_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "No Image",
                                egui::FontId::proportional(16.0),
                                theme.text_muted(),
                            );

                            ui.add_space(theme.spacing_md);

                            // Price
                            ui.label(RichText::new(format!("{:.2} SOL", listing.price)).size(theme.font_size_title).color(theme.accent()));

                            ui.add_space(theme.spacing_sm);

                            // Description
                            widgets::card(ui, theme, |ui| {
                                ui.label(RichText::new("Description").size(theme.font_size_body).color(theme.text_secondary()));
                                ui.add_space(theme.spacing_xs);
                                ui.label(RichText::new(&listing.description).color(theme.text_primary()));
                            });

                            ui.add_space(theme.spacing_sm);

                            // Seller info
                            widgets::card(ui, theme, |ui| {
                                ui.label(RichText::new("Seller Information").size(theme.font_size_body).color(theme.text_secondary()));
                                ui.add_space(theme.spacing_xs);
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Seller:").color(theme.text_muted()));
                                    ui.label(RichText::new(&listing.seller).color(theme.text_primary()));
                                });
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Reputation:").color(theme.text_muted()));
                                    // Placeholder reputation
                                    ui.label(RichText::new("No ratings yet").color(theme.text_muted()));
                                });
                            });

                            ui.add_space(theme.spacing_md);

                            // Action buttons
                            ui.horizontal(|ui| {
                                if widgets::primary_button(ui, theme, "Buy") {
                                    // Placeholder
                                }
                                if widgets::secondary_button(ui, theme, "Contact Seller") {
                                    // Placeholder
                                }
                            });
                        });
                    }
                }
            } else {
                // ── Listing grid view ──

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

                // Search bar and sort
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Search:").color(theme.text_secondary()));
                    ui.add(egui::TextEdit::singleline(&mut state.listing_search)
                        .desired_width(250.0)
                        .hint_text("Search listings..."));
                    ui.add_space(theme.spacing_md);
                    ui.label(RichText::new("Sort:").color(theme.text_secondary()));
                    with_state(|ps| {
                        egui::ComboBox::from_id_salt("listing_sort")
                            .selected_text(ps.sort_order.label())
                            .width(140.0)
                            .show_ui(ui, |ui| {
                                for order in [SortOrder::Newest, SortOrder::PriceLowHigh, SortOrder::PriceHighLow, SortOrder::Relevance] {
                                    if ui.selectable_label(ps.sort_order == order, order.label()).clicked() {
                                        ps.sort_order = order;
                                    }
                                }
                            });
                    });
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
                                    for &cat in &CATEGORIES[1..] {
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

                // Filter and sort listings
                let search_lower = state.listing_search.to_lowercase();
                let sort_order = with_state(|ps| ps.sort_order);
                let mut filtered: Vec<usize> = state.listings.iter().enumerate()
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

                // Apply sort
                match sort_order {
                    SortOrder::PriceLowHigh => {
                        filtered.sort_by(|&a, &b| {
                            state.listings[a].price.partial_cmp(&state.listings[b].price).unwrap_or(std::cmp::Ordering::Equal)
                        });
                    }
                    SortOrder::PriceHighLow => {
                        filtered.sort_by(|&a, &b| {
                            state.listings[b].price.partial_cmp(&state.listings[a].price).unwrap_or(std::cmp::Ordering::Equal)
                        });
                    }
                    SortOrder::Newest => {
                        filtered.sort_by(|&a, &b| b.cmp(&a));
                    }
                    SortOrder::Relevance => {
                        // Keep natural order
                    }
                }

                if filtered.is_empty() {
                    ui.add_space(theme.spacing_xl);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("No listings").size(theme.font_size_heading).color(theme.text_muted()));
                        ui.label(RichText::new("Create one to get started.").color(theme.text_secondary()));
                    });
                } else {
                    // Result count
                    ui.label(RichText::new(format!("{} listing(s)", filtered.len())).color(theme.text_muted()).size(theme.font_size_small));
                    ui.add_space(theme.spacing_xs);

                    ScrollArea::vertical().show(ui, |ui| {
                        // 3-column card grid
                        let chunks: Vec<&[usize]> = filtered.chunks(3).collect();
                        for chunk in chunks {
                            ui.horizontal(|ui| {
                                for &idx in chunk {
                                    let listing = &state.listings[idx];
                                    let card_width = 260.0;
                                    ui.allocate_ui(Vec2::new(card_width, 120.0), |ui| {
                                        widgets::card(ui, theme, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(RichText::new(&listing.title).color(theme.text_primary()));
                                                let cat_col = category_color(&listing.category);
                                                egui::Frame::none()
                                                    .fill(cat_col)
                                                    .rounding(Rounding::same(3))
                                                    .inner_margin(Vec2::new(4.0, 1.0))
                                                    .show(ui, |ui| {
                                                        ui.label(RichText::new(&listing.category).size(theme.font_size_small).color(Color32::WHITE));
                                                    });
                                            });
                                            ui.label(RichText::new(format!("{:.2} SOL", listing.price)).size(theme.font_size_heading).color(theme.accent()));
                                            // Short description
                                            if !listing.description.is_empty() {
                                                let preview: String = listing.description.chars().take(60).collect();
                                                let suffix = if listing.description.chars().count() > 60 { "..." } else { "" };
                                                ui.label(RichText::new(format!("{}{}", preview, suffix)).color(theme.text_muted()).size(theme.font_size_small));
                                            }
                                            ui.label(RichText::new(format!("Seller: {}", listing.seller)).color(theme.text_muted()).size(theme.font_size_small));
                                            if widgets::secondary_button(ui, theme, "View") {
                                                state.listing_selected = Some(idx);
                                                with_state(|ps| ps.detail_view = true);
                                            }
                                        });
                                    });
                                }
                            });
                            ui.add_space(theme.spacing_xs);
                        }
                    });
                }
            }
        });

    if close_detail {
        with_state(|ps| ps.detail_view = false);
        state.listing_selected = None;
    }
}
