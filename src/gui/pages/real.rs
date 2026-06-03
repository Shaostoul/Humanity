//! Real — the merged "your actual life" tab (page carve, v0.358).
//!
//! ONE page with a `section_nav` sidebar that folds in Profile's sections
//! (Body / Identity / Notes / Network / Interests / Skills / Quests / Social /
//! Streaming) PLUS Possessions (Inventory), Wallet, Tasks, Map, Market — per the
//! operator's call to merge Profile's sidebar in rather than special-case it.
//!
//! The content area to the right delegates: Profile sections render Profile's
//! section content directly; the other sections delegate to the existing page's
//! `draw` (its CentralPanel renders beside this sidebar, so no page rewrite was
//! needed). Replaces six separate top-nav buttons with a single "Real" tab —
//! the sidebar IS the operator's section_nav table-of-contents.

use egui::{Frame, ScrollArea, Stroke};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, SectionNavItem};
use super::profile::{self, PRIVATE_DOT, PERSONAL_DOT, PUBLIC_DOT};
use super::{inventory, wallet, tasks, market, cosmos};

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
                // Profile — flattened in (its old standalone sidebar is gone).
                SectionNavItem::new("body", "Body & Measurements", PRIVATE_DOT).group("PRIVATE"),
                SectionNavItem::new("identity", "Identity", PRIVATE_DOT),
                SectionNavItem::new("notes", "Private Notes", PRIVATE_DOT),
                SectionNavItem::new("network", "Network Profile", PERSONAL_DOT).group("PERSONAL"),
                SectionNavItem::new("interests", "Interests", PERSONAL_DOT),
                SectionNavItem::new("skills", "Skills", PERSONAL_DOT),
                SectionNavItem::new("quests", "Quests", PERSONAL_DOT),
                SectionNavItem::new("social", "Social Links", PUBLIC_DOT).group("PUBLIC"),
                SectionNavItem::new("streaming", "Streaming", PUBLIC_DOT),
                // The former standalone pages, folded in.
                SectionNavItem::new("inventory", "Possessions", belongings).group("BELONGINGS"),
                SectionNavItem::new("wallet", "Wallet", belongings),
                SectionNavItem::new("tasks", "Tasks", life).group("LIFE"),
                SectionNavItem::new("maps", "Map", life),
                SectionNavItem::new("market", "Market", life),
            ];
            if let Some(clicked) =
                widgets::section_nav(ui, theme, Some("Real"), &items, &state.active_real_section)
            {
                state.active_real_section = clicked;
            }
        });

    // ── Content: delegate to the selected section ──
    // Clone the id so the match doesn't hold a borrow of `state` while the
    // delegate mutates it.
    let section = state.active_real_section.clone();
    match section.as_str() {
        "inventory" => inventory::draw(ctx, theme, state),
        "wallet" => wallet::draw(ctx, theme, state),
        "tasks" => tasks::draw(ctx, theme, state),
        "maps" => cosmos::draw(ctx, theme, state), // Maps routes to the universal Cosmos map
        "market" => market::draw(ctx, theme, state),
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
