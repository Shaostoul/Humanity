//! Marketplace — browse, search, and create listings.
//!
//! Left sidebar: category filter. Center: search bar, sort dropdown, listing card grid.
//! Detail view on click. "Create Listing" form.

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Vec2};
use crate::gui::{GuiState, GuiListing};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use std::cell::RefCell;

// Marketplace categories are loaded from `data/market/categories.json` into
// `GuiState.market_categories` at startup.

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

/// Leading numeric value of a free-text price ("12.5 SOL" -> 12.5) for
/// sorting; non-numeric or empty prices sort last.
fn price_num(p: &str) -> f64 {
    let s: String = p
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    s.parse().unwrap_or(f64::MAX)
}

/// Client-generated listing id, unique per publish (same contract as web:
/// the relay stores whatever id the creating client minted).
fn new_listing_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNT: AtomicU32 = AtomicU32::new(0);
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("n{:x}-{:03x}", millis, COUNT.fetch_add(1, Ordering::Relaxed))
}

/// Seller display: profile name when the relay knows it, key prefix otherwise.
fn seller_label(l: &GuiListing) -> String {
    if !l.seller_name.is_empty() {
        l.seller_name.clone()
    } else if l.seller_key.len() > 8 {
        format!("{}...", &l.seller_key[..8])
    } else {
        l.seller_key.clone()
    }
}

/// Background REST fetch of a listing's reviews (public read endpoint; same
/// worker-thread + mpsc pattern as Server Settings' federation panel). The
/// review LIST rides REST like web does; creates/deletes ride the WS.
fn spawn_reviews_fetch(state: &mut GuiState, listing_id: &str) {
    let base = state.server_url.trim_end_matches('/').to_string();
    let id = listing_id.to_string();
    let (tx, rx) = std::sync::mpsc::channel();
    state.listing_reviews_rx = Some(rx);
    state.listing_reviews_for = listing_id.to_string();
    state.listing_reviews.clear();
    state.listing_reviews_avg = 0.0;
    state.listing_reviews_count = 0;
    std::thread::spawn(move || {
        let fetch = || -> Result<(Vec<crate::gui::GuiReview>, f32, i64), String> {
            let body = ureq::get(&format!("{base}/api/listings/{id}/reviews"))
                .call()
                .map_err(|e| format!("reviews: {e}"))?
                .into_string()
                .map_err(|e| format!("read: {e}"))?;
            let val: serde_json::Value =
                serde_json::from_str(&body).map_err(|e| format!("parse: {e}"))?;
            let rows = val
                .get("reviews")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().map(crate::gui::GuiReview::from_relay_json).collect())
                .unwrap_or_default();
            let avg = val.get("avg_rating").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let count = val.get("review_count").and_then(|v| v.as_i64()).unwrap_or(0);
            Ok((rows, avg, count))
        };
        let _ = tx.send(fetch());
    });
}

fn category_color(category: &str) -> Color32 {
    match category {
        "Tools" => Color32::from_rgb(70, 130, 180),
        "Materials" => Color32::from_rgb(139, 119, 101), // theme-exempt: categorical badge palette (brown), no semantic token exists for it
        "Food" => Color32::from_rgb(60, 150, 60), // theme-exempt: categorical badge palette (green), category identity not a success state
        "Equipment" => Color32::from_rgb(180, 140, 50), // theme-exempt: categorical badge palette (amber), category identity not a warning state
        "Vehicles" => Color32::from_rgb(140, 80, 160), // theme-exempt: categorical badge palette (purple), no semantic token exists for it
        "Electronics" => Color32::from_rgb(50, 150, 200), // theme-exempt: categorical badge palette (cyan blue), must stay distinct from the Tools blue
        "Services" => Color32::from_rgb(200, 100, 80), // theme-exempt: categorical badge palette (terracotta), category identity not a danger state
        _ => Color32::from_rgb(120, 120, 130), // theme-exempt: categorical badge palette fallback (neutral grey) for unknown/custom categories from data/market/categories.json
    }
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // Live sync with the connected relay (v0.752, ladder rung 5): first view
    // after connect pulls the live list via listing_browse; the lib.rs WS
    // dispatch keeps it current from listing_new/updated/deleted broadcasts.
    // Dropping the connection clears the flag so a reconnect re-syncs.
    let connected = state.ws_client.as_ref().map_or(false, |c| c.is_connected());
    if connected && !state.listings_synced {
        if let Some(ws) = &state.ws_client {
            ws.send(&serde_json::json!({"type": "listing_browse"}).to_string());
        }
        state.listings_synced = true;
    }
    if !connected {
        state.listings_synced = false;
    }

    // Detail view (replaces center panel when viewing a listing)
    let showing_detail = with_state(|ps| ps.detail_view) && state.listing_selected.is_some();
    let mut close_detail = false;
    // Deferred action out of the listing borrow below.
    let mut delete_listing_id: Option<String> = None;

    // Reviews fetch lifecycle (v0.755): opening a detail view pulls that
    // listing's reviews over REST; drain the worker when it lands.
    if let Some(rx) = &state.listing_reviews_rx {
        match rx.try_recv() {
            Ok(Ok((rows, avg, count))) => {
                state.listing_reviews = rows;
                state.listing_reviews_avg = avg;
                state.listing_reviews_count = count;
                state.listing_reviews_rx = None;
            }
            Ok(Err(_)) => {
                // Offline / server missing the route: reviews just stay empty.
                state.listing_reviews_rx = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(300));
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                state.listing_reviews_rx = None;
            }
        }
    }
    if showing_detail {
        let sel_id = state.listing_selected.clone().unwrap_or_default();
        if state.listing_reviews_for != sel_id && state.listing_reviews_rx.is_none() {
            spawn_reviews_fetch(state, &sel_id);
            // A fresh detail view also resets the message thread.
            state.listing_thread.clear();
            state.listing_thread_open = false;
            state.listing_thread_for.clear();
            state.listing_msg_draft.clear();
            state.review_rating_draft = 5;
            state.review_comment_draft.clear();
        }
    }

    // Left sidebar: category filter
    egui::SidePanel::left("market_categories")
        .min_width(140.0)
        .max_width(170.0)
        .frame(Frame::none().fill(theme.bg_sidebar()).inner_margin(10.0))
        .show(ctx, |ui| {
            ui.label(RichText::new("Categories").size(theme.font_size_heading).color(theme.text_primary()));
            ui.add_space(theme.spacing_xs);
            ui.separator();
            ui.add_space(theme.spacing_xs);

            let cat_strs: Vec<&str> = state.market_categories.iter().map(String::as_str).collect();
            let active_idx = cat_strs.iter().position(|c| {
                (*c == "All" && state.listing_filter_category.is_empty())
                    || state.listing_filter_category == *c
            }).unwrap_or(0);
            if let Some(new_idx) = widgets::sidebar_nav(ui, theme, &cat_strs, active_idx) {
                state.listing_filter_category = if new_idx == 0 {
                    String::new()
                } else {
                    cat_strs.get(new_idx).map(|s| s.to_string()).unwrap_or_default()
                };
            }
        });

    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
        .show(ctx, |ui| {
            if showing_detail {
                // ── Detail view ──
                let sel_id = state.listing_selected.clone().unwrap_or_default();
                let my_key = state.profile_public_key.clone();
                // Cloned (small struct) so the reviews/thread UI below can
                // mutate GuiState form fields without fighting the borrow.
                if let Some(listing) = state.listings.iter().find(|l| l.id == sel_id).cloned() {
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
                        widgets::badge(ui, theme, &listing.category, category_color(&listing.category));

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

                        // Price (free text, matching web)
                        let price = if listing.price.is_empty() { "Price on request" } else { &listing.price };
                        ui.label(RichText::new(price).size(theme.font_size_title).color(theme.accent()));

                        ui.add_space(theme.spacing_sm);

                        // Description
                        widgets::card(ui, theme, |ui| {
                            ui.label(RichText::new("Description").size(theme.font_size_body).color(theme.text_secondary()));
                            ui.add_space(theme.spacing_xs);
                            let desc = if listing.description.is_empty() { "No description." } else { &listing.description };
                            ui.label(RichText::new(desc).color(theme.text_primary()));
                        });

                        ui.add_space(theme.spacing_sm);

                        // Seller + trade details
                        widgets::card(ui, theme, |ui| {
                            ui.label(RichText::new("Seller Information").size(theme.font_size_body).color(theme.text_secondary()));
                            ui.add_space(theme.spacing_xs);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Seller:").color(theme.text_muted()));
                                ui.label(RichText::new(seller_label(&listing)).color(theme.text_primary()));
                            });
                            for (label, value) in [
                                ("Condition:", &listing.condition),
                                ("Payment:", &listing.payment_methods),
                                ("Location:", &listing.location),
                                ("Listed:", &listing.created_at),
                            ] {
                                if !value.is_empty() {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(label).color(theme.text_muted()));
                                        ui.label(RichText::new(value).color(theme.text_primary()));
                                    });
                                }
                            }
                        });

                        ui.add_space(theme.spacing_md);

                        // ── Reviews (v0.755) ── list rides REST; creates ride
                        // the WS; review_created broadcasts keep it live.
                        ui.add_space(theme.spacing_sm);
                        let mut delete_review_id: Option<i64> = None;
                        widgets::card(ui, theme, |ui| {
                            let header = if state.listing_reviews_count > 0 {
                                format!(
                                    "Reviews ({}) - {:.1} average",
                                    state.listing_reviews_count, state.listing_reviews_avg
                                )
                            } else {
                                "Reviews".to_string()
                            };
                            ui.label(RichText::new(header).size(theme.font_size_body).color(theme.text_secondary()));
                            ui.add_space(theme.spacing_xs);
                            if state.listing_reviews.is_empty() {
                                ui.label(
                                    RichText::new("No reviews yet.")
                                        .color(theme.text_muted())
                                        .size(theme.font_size_small),
                                );
                            }
                            for r in &state.listing_reviews {
                                ui.horizontal(|ui| {
                                    let stars = "⭐".repeat(r.rating.clamp(0, 5) as usize);
                                    ui.label(RichText::new(stars).size(theme.font_size_small));
                                    let who = if r.reviewer_name.is_empty() {
                                        if r.reviewer_key.len() > 8 {
                                            format!("{}...", &r.reviewer_key[..8])
                                        } else {
                                            r.reviewer_key.clone()
                                        }
                                    } else {
                                        r.reviewer_name.clone()
                                    };
                                    ui.label(
                                        RichText::new(who)
                                            .color(theme.text_primary())
                                            .size(theme.font_size_small),
                                    );
                                    if r.reviewer_key == my_key
                                        && widgets::secondary_button(ui, theme, "Delete")
                                    {
                                        delete_review_id = Some(r.id);
                                    }
                                });
                                if !r.comment.is_empty() {
                                    ui.label(
                                        RichText::new(&r.comment)
                                            .color(theme.text_secondary())
                                            .size(theme.font_size_small),
                                    );
                                }
                            }
                            // Leave a review on someone else's listing.
                            if listing.seller_key != my_key {
                                ui.add_space(theme.spacing_xs);
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new("Rate:")
                                            .color(theme.text_muted())
                                            .size(theme.font_size_small),
                                    );
                                    for i in 1..=5 {
                                        let mark = if state.review_rating_draft >= i { "⭐" } else { "·" };
                                        if ui.selectable_label(false, mark).clicked() {
                                            state.review_rating_draft = i;
                                        }
                                    }
                                });
                                ui.horizontal(|ui| {
                                    ui.add(
                                        egui::TextEdit::singleline(&mut state.review_comment_draft)
                                            .hint_text("Optional comment")
                                            .desired_width(220.0),
                                    );
                                    if widgets::secondary_button(ui, theme, "Submit review") {
                                        if connected {
                                            if let Some(ws) = &state.ws_client {
                                                ws.send(
                                                    &serde_json::json!({
                                                        "type": "review_create",
                                                        "listing_id": listing.id,
                                                        "rating": state.review_rating_draft,
                                                        "comment": state.review_comment_draft.trim(),
                                                    })
                                                    .to_string(),
                                                );
                                            }
                                            state.review_comment_draft.clear();
                                            state.listing_status = "Review submitted.".to_string();
                                        } else {
                                            state.listing_status =
                                                "Connect to a server to review listings.".to_string();
                                        }
                                    }
                                });
                            }
                        });
                        if let Some(rid) = delete_review_id {
                            if let Some(ws) = &state.ws_client {
                                ws.send(
                                    &serde_json::json!({
                                        "type": "review_delete",
                                        "listing_id": listing.id,
                                        "review_id": rid,
                                    })
                                    .to_string(),
                                );
                            }
                        }

                        // ── Messages (v0.755) ── the buyer-seller thread the
                        // relay already stores; history is pulled on open and
                        // listing_message_new broadcasts keep it live.
                        ui.add_space(theme.spacing_sm);
                        widgets::card(ui, theme, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Messages")
                                        .size(theme.font_size_body)
                                        .color(theme.text_secondary()),
                                );
                                if !state.listing_thread_open {
                                    let label = if listing.seller_key == my_key {
                                        "View messages"
                                    } else {
                                        "Contact Seller"
                                    };
                                    if connected && widgets::secondary_button(ui, theme, label) {
                                        state.listing_thread_open = true;
                                        state.listing_thread_for = listing.id.clone();
                                        if let Some(ws) = &state.ws_client {
                                            ws.send(
                                                &serde_json::json!({
                                                    "type": "listing_message_history",
                                                    "listing_id": listing.id,
                                                })
                                                .to_string(),
                                            );
                                        }
                                    }
                                }
                            });
                            if !connected {
                                ui.label(
                                    RichText::new("Connect to a server to message the seller.")
                                        .color(theme.text_muted())
                                        .size(theme.font_size_small),
                                );
                            }
                            if state.listing_thread_open {
                                if state.listing_thread.is_empty() {
                                    ui.label(
                                        RichText::new("No messages yet - ask the seller anything.")
                                            .color(theme.text_muted())
                                            .size(theme.font_size_small),
                                    );
                                }
                                for m in &state.listing_thread {
                                    ui.horizontal_wrapped(|ui| {
                                        let who = if m.sender_name.is_empty() {
                                            if m.sender_key.len() > 8 {
                                                format!("{}...", &m.sender_key[..8])
                                            } else {
                                                m.sender_key.clone()
                                            }
                                        } else {
                                            m.sender_name.clone()
                                        };
                                        ui.label(
                                            RichText::new(format!("{who}:"))
                                                .color(theme.text_primary())
                                                .size(theme.font_size_small),
                                        );
                                        ui.label(
                                            RichText::new(&m.content)
                                                .color(theme.text_secondary())
                                                .size(theme.font_size_small),
                                        );
                                    });
                                }
                                ui.add_space(theme.spacing_xs);
                                ui.horizontal(|ui| {
                                    ui.add(
                                        egui::TextEdit::singleline(&mut state.listing_msg_draft)
                                            .hint_text("Write a message...")
                                            .desired_width(220.0),
                                    );
                                    if widgets::secondary_button(ui, theme, "Send")
                                        && !state.listing_msg_draft.trim().is_empty()
                                    {
                                        if connected {
                                            if let Some(ws) = &state.ws_client {
                                                ws.send(
                                                    &serde_json::json!({
                                                        "type": "listing_message_send",
                                                        "listing_id": listing.id,
                                                        "content": state.listing_msg_draft.trim(),
                                                    })
                                                    .to_string(),
                                                );
                                            }
                                            state.listing_msg_draft.clear();
                                        } else {
                                            state.listing_status =
                                                "Connect to a server to send messages.".to_string();
                                        }
                                    }
                                });
                            }
                        });

                        ui.add_space(theme.spacing_md);

                        // Your own listing can be removed; escrow buying still
                        // waits for the trade-flow follow-up (no dead buttons).
                        if listing.seller_key == my_key {
                            if widgets::secondary_button(ui, theme, "Delete Listing") {
                                delete_listing_id = Some(listing.id.clone());
                            }
                        }
                    });
                } else {
                    // The listing vanished (deleted remotely) - fall back.
                    close_detail = true;
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
                        if connected && widgets::secondary_button(ui, theme, "Refresh") {
                            if let Some(ws) = &state.ws_client {
                                ws.send(&serde_json::json!({"type": "listing_browse"}).to_string());
                            }
                        }
                    });
                });
                if !connected {
                    ui.label(
                        RichText::new("Offline - connect to a server (Chat page) to browse the live market.")
                            .color(theme.warning())
                            .size(theme.font_size_small),
                    );
                }
                if !state.listing_status.is_empty() {
                    ui.label(
                        RichText::new(&state.listing_status)
                            .color(theme.text_secondary())
                            .size(theme.font_size_small),
                    );
                }

                ui.add_space(theme.spacing_sm);

                // Search bar and sort
                widgets::search_bar(ui, theme, &mut state.listing_search, "Search listings...");
                ui.horizontal(|ui| {
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
                    widgets::card_with_header(ui, theme, "Create Listing", |ui| {
                        widgets::form_row(ui, theme, "Title", |ui| {
                            ui.add(egui::TextEdit::singleline(&mut state.listing_new_title).desired_width(280.0));
                        });
                        widgets::form_row(ui, theme, "Description", |ui| {
                            ui.add(egui::TextEdit::multiline(&mut state.listing_new_description)
                                .desired_width(280.0)
                                .desired_rows(2));
                        });
                        widgets::form_row(ui, theme, "Price", |ui| {
                            ui.add(egui::TextEdit::singleline(&mut state.listing_new_price)
                                .hint_text("e.g. 5 SOL, 20 CR, free")
                                .desired_width(160.0));
                        });
                        widgets::form_row(ui, theme, "Category", |ui| {
                            let create_cats: Vec<String> = state.market_categories.iter().skip(1).cloned().collect();
                            egui::ComboBox::from_id_salt("new_listing_category")
                                .selected_text(if state.listing_new_category.is_empty() { "Select..." } else { &state.listing_new_category })
                                .show_ui(ui, |ui| {
                                    for cat in &create_cats {
                                        if ui.selectable_label(state.listing_new_category == *cat, cat.as_str()).clicked() {
                                            state.listing_new_category = cat.clone();
                                        }
                                    }
                                });
                        });
                        ui.add_space(theme.spacing_sm);
                        ui.horizontal(|ui| {
                            if widgets::Button::primary("Publish").show(ui, theme) && !state.listing_new_title.trim().is_empty() {
                                if connected {
                                    // Same contract as web: client mints the id,
                                    // the relay stores + broadcasts listing_new
                                    // (which is what adds it to our list - no
                                    // local echo, the round-trip IS the confirm).
                                    let msg = serde_json::json!({
                                        "type": "listing_create",
                                        "id": new_listing_id(),
                                        "title": state.listing_new_title.trim(),
                                        "description": state.listing_new_description.trim(),
                                        "category": if state.listing_new_category.is_empty() { "Other" } else { state.listing_new_category.as_str() },
                                        "price": state.listing_new_price.trim(),
                                    });
                                    if let Some(ws) = &state.ws_client {
                                        ws.send(&msg.to_string());
                                    }
                                    state.listing_status = "Publishing...".to_string();
                                    state.listing_new_title.clear();
                                    state.listing_new_description.clear();
                                    state.listing_new_price.clear();
                                    state.listing_new_category.clear();
                                    state.listing_show_new_form = false;
                                } else {
                                    state.listing_status =
                                        "Connect to a server (Chat page) to publish listings.".to_string();
                                }
                            }
                            ui.add_space(theme.spacing_sm);
                            if widgets::Button::secondary("Cancel").show(ui, theme) {
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

                // Apply sort (prices are free text - sort by leading number)
                match sort_order {
                    SortOrder::PriceLowHigh => {
                        filtered.sort_by(|&a, &b| {
                            price_num(&state.listings[a].price)
                                .partial_cmp(&price_num(&state.listings[b].price))
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });
                    }
                    SortOrder::PriceHighLow => {
                        filtered.sort_by(|&a, &b| {
                            price_num(&state.listings[b].price)
                                .partial_cmp(&price_num(&state.listings[a].price))
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });
                    }
                    SortOrder::Newest => {
                        // The relay returns newest-first; broadcasts insert at
                        // the front - natural order IS newest.
                    }
                    SortOrder::Relevance => {
                        // Keep natural order
                    }
                }

                if filtered.is_empty() {
                    ui.add_space(theme.spacing_xl);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("No listings").size(theme.font_size_heading).color(theme.text_muted()));
                        let hint = if connected {
                            "Create one to get started."
                        } else {
                            "Connect to a server to see the live market."
                        };
                        ui.label(RichText::new(hint).color(theme.text_secondary()));
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
                                                widgets::badge_sm(ui, theme, &listing.category, category_color(&listing.category));
                                            });
                                            let price = if listing.price.is_empty() { "Price on request".to_string() } else { listing.price.clone() };
                                            ui.label(RichText::new(price).size(theme.font_size_heading).color(theme.accent()));
                                            // Short description
                                            if !listing.description.is_empty() {
                                                let preview: String = listing.description.chars().take(60).collect();
                                                let suffix = if listing.description.chars().count() > 60 { "..." } else { "" };
                                                ui.label(RichText::new(format!("{}{}", preview, suffix)).color(theme.text_muted()).size(theme.font_size_small));
                                            }
                                            ui.label(RichText::new(format!("Seller: {}", seller_label(listing))).color(theme.text_muted()).size(theme.font_size_small));
                                            let id = listing.id.clone();
                                            if widgets::secondary_button(ui, theme, "View") {
                                                state.listing_selected = Some(id);
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

    // Deferred delete (collected inside the listing borrow above): the relay
    // enforces ownership and broadcasts listing_deleted, which removes it
    // from our list - no local removal here, the round-trip is the truth.
    if let Some(id) = delete_listing_id {
        if let Some(ws) = &state.ws_client {
            ws.send(&serde_json::json!({"type": "listing_delete", "id": id}).to_string());
        }
        state.listing_status = "Deleting listing...".to_string();
        close_detail = true;
    }

    if close_detail {
        with_state(|ps| ps.detail_view = false);
        state.listing_selected = None;
    }
}
