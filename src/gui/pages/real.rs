//! Profile — your character / identity tab (renamed from "Real" v0.378; page carve v0.358).
//!
//! ONE page with a `section_nav` sidebar that folds in Profile's sections
//! (Body / Identity / Notes / Network / Interests / Skills / Social /
//! Streaming) PLUS Possessions (Inventory), Wallet, Tasks, Map, Market — per the
//! operator's call to merge Profile's sidebar in rather than special-case it.
//!
//! The content area to the right delegates: Profile sections render Profile's
//! section content directly; the other sections delegate to the existing page's
//! `draw` (its CentralPanel renders beside this sidebar, so no page rewrite was
//! needed). Replaces six separate top-nav buttons with a single "Real" tab —
//! the sidebar IS the operator's section_nav table-of-contents.

use egui::{Frame, RichText, ScrollArea, Stroke};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, SectionNavItem};
use super::profile::{self, PRIVATE_DOT, PERSONAL_DOT, PUBLIC_DOT};
use super::{wallet, market, trade, guilds};

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // ── Unified left section-nav sidebar (the TOC) ──
    egui::SidePanel::left("real_section_nav")
        .default_width(210.0)
        .min_width(170.0)
        .max_width(280.0)
        .frame(
            Frame::none()
                .fill(theme.bg_sidebar())
                .inner_margin(egui::Margin::symmetric(8, 12))
                .stroke(Stroke::new(1.0, theme.border())),
        )
        .show(ctx, |ui| {
            let belongings = theme.warning(); // your stuff
            let life = theme.info(); // your activities
            let items = [
                // Profile sections (flattened in; the old standalone sidebar is gone).
                SectionNavItem::new("body", "Body & Measurements", PRIVATE_DOT).group("PRIVATE"),
                SectionNavItem::new("identity", "Identity", PRIVATE_DOT),
                SectionNavItem::new("notes", "Private Notes", PRIVATE_DOT),
                SectionNavItem::new("network", "Network Profile", PERSONAL_DOT).group("PERSONAL"),
                SectionNavItem::new("interests", "Interests", PERSONAL_DOT),
                SectionNavItem::new("skills", "Skills", PERSONAL_DOT),
                SectionNavItem::new("social", "Social Links", PUBLIC_DOT).group("PUBLIC"),
                // Wallet + Market stay here; Possessions/Tasks/Map became their own
                // top-level tabs (operator 2026-06-07) and Streaming moved into Studio.
                SectionNavItem::new("wallet", "Wallet", belongings).group("BELONGINGS"),
                SectionNavItem::new("market", "Market", life).group("LIFE"),
                // Trade + Guilds rejoined the nav here (v0.699): both are P2P
                // social/economy surfaces that fit alongside Market, and both
                // had been stranded (only reachable via the removed category
                // overview pages) since the v0.196 nav rewrite.
                SectionNavItem::new("trade", "Trade", life),
                SectionNavItem::new("guilds", "Guilds", life),
            ];
            // Profile selector at the top (operator 2026-06-07: "add a profile
            // selector. I only want one profile"). One base character today; the
            // per-server augmented versions slot in here later (homes-as-profiles.md).
            draw_profile_selector(ui, theme);
            ui.add_space(theme.spacing_sm);

            if let Some(clicked) =
                widgets::section_nav(ui, theme, Some("Profile"), &items, &state.active_real_section)
            {
                state.active_real_section = clicked;
            }
        });

    // ── Content: delegate to the selected section ──
    // Clone the id so the match doesn't hold a borrow of `state` while the
    // delegate mutates it.
    let section = state.active_real_section.clone();
    match section.as_str() {
        "wallet" => wallet::draw(ctx, theme, state),
        "market" => market::draw(ctx, theme, state),
        "trade" => trade::draw(ctx, theme, state),
        "guilds" => guilds::draw(ctx, theme, state),
        // Anything else is a Profile section — point Profile at it + render.
        other => {
            state.profile_section = profile::section_from_id(other);
            egui::CentralPanel::default()
                .frame(Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
                .show(ctx, |ui| {
                    ScrollArea::vertical().show(ui, |ui| {
                        profile::draw_section_content(ui, theme, state);
                    });
                });
        }
    }
}

/// The profile/character selector at the top of the Profile sidebar. One base
/// character today (the operator wants exactly one identity, typed once); when the
/// home/character model lands, per-server AUGMENTED versions appear here as extra
/// options while the base stays shared, so your look/biography carry across
/// servers. See docs/design/homes-as-profiles.md (Characters section).
fn draw_profile_selector(ui: &mut egui::Ui, theme: &Theme) {
    use std::cell::RefCell;
    thread_local! {
        static SELECTED: RefCell<usize> = RefCell::new(0);
    }
    // One option for now; this slice is the seam the per-server versions slot into.
    let options = ["Base (your real self)"];
    Frame::none()
        .fill(theme.bg_card())
        .rounding(egui::Rounding::same(theme.border_radius as u8))
        .stroke(Stroke::new(1.0, theme.border()))
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("CHARACTER").size(theme.font_size_small).color(theme.text_muted()));
            SELECTED.with(|sel| {
                let mut idx = *sel.borrow();
                egui::ComboBox::from_id_salt("profile_character_selector")
                    .selected_text(options[idx])
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        for (i, opt) in options.iter().enumerate() {
                            ui.selectable_value(&mut idx, i, *opt);
                        }
                    });
                *sel.borrow_mut() = idx;
            });
            ui.label(
                RichText::new("One base character, typed once. Servers save augmented versions of you.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
        });
}
