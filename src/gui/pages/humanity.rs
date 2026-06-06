//! Humanity, the collective / mission tab (page carve, v0.360; Mission
//! Dashboard built out v0.362; copy revised v0.363).
//!
//! What the **H** button opens. A `section_nav` sidebar with Mission Dashboard
//! (the real landing, below) + Governance, Directory (Identity), Onboarding,
//! Donate, Resources (delegated to their pages). The Mission Dashboard is built
//! from `docs/design/humanity-page.md` to be good enough to be the public
//! landing page. NOTE: copy here is deliberately em-dash-free (operator: em
//! dashes read as machine-written and cost trust on a landing page) and frames
//! the personal "why" (one family's survival is everyone's). Keep it that way.

use egui::{Frame, Margin, RichText, Rounding, ScrollArea, Stroke};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, SectionNavItem};
use super::{governance, identity, onboarding, donate, resources};

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::SidePanel::left("humanity_section_nav")
        .default_width(200.0)
        .min_width(160.0)
        .max_width(270.0)
        .frame(
            Frame::none()
                .fill(theme.bg_sidebar())
                .inner_margin(Margin::symmetric(8, 12))
                .stroke(Stroke::new(1.0, theme.border())),
        )
        .show(ctx, |ui| {
            let c = theme.nav_reality();
            let items = [
                SectionNavItem::new("civilization", "Mission Dashboard", c),
                SectionNavItem::new("governance", "Governance", c),
                SectionNavItem::new("identity", "Directory", c),
                SectionNavItem::new("onboarding", "Onboarding", c),
                SectionNavItem::new("donate", "Donate", c),
                SectionNavItem::new("resources", "Resources", c),
            ];
            if let Some(clicked) = widgets::section_nav(
                ui,
                theme,
                Some("Humanity"),
                &items,
                &state.active_humanity_section,
            ) {
                state.active_humanity_section = clicked;
            }
        });

    let section = state.active_humanity_section.clone();
    match section.as_str() {
        "governance" => governance::draw(ctx, theme, state),
        "identity" => identity::draw(ctx, theme, state),
        "onboarding" => onboarding::draw(ctx, theme, state),
        "donate" => donate::draw(ctx, theme, state),
        "resources" => resources::draw(ctx, theme, state),
        // Default = the Mission Dashboard (the real Humanity landing).
        _ => {
            egui::CentralPanel::default()
                .frame(Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
                .show(ctx, |ui| {
                    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        draw_mission_dashboard(ui, theme, state);
                    });
                });
        }
    }
}

/// The mission of our civilization. The landing the H button deserves.
fn draw_mission_dashboard(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // Hero
    ui.add_space(theme.spacing_lg);
    ui.label(
        RichText::new("HumanityOS")
            .size(theme.font_size_title * 1.3)
            .strong()
            .color(theme.text_primary()),
    );
    ui.add_space(theme.spacing_xs);
    ui.label(
        RichText::new("End poverty. Unite humanity.")
            .size(theme.font_size_heading)
            .strong()
            .color(theme.accent()),
    );
    ui.add_space(theme.spacing_sm);
    // The two scopes the whole project lives at, named plainly so anyone, at any
    // age or state of mind, instantly identifies BOTH: what this does for YOU,
    // and what we are doing together as a civilization. (Operator 2026-06-05:
    // the individual AND the civilization must stay easily identifiable, and the
    // experience must stay consistent app to web. This is the spine of the page.)
    ui.label(
        RichText::new("For you: the tools to feed, power, and provide for yourself and the people you love.")
            .size(theme.font_size_body)
            .color(theme.text_secondary()),
    );
    ui.add_space(theme.spacing_xs);
    ui.label(
        RichText::new("For all of us: a fair way to end poverty together. No tyrants, no corporations, no catch. Free and public domain, for every human and every AI.")
            .size(theme.font_size_body)
            .color(theme.text_secondary()),
    );
    ui.add_space(theme.spacing_lg);

    // Why this exists (the personal "why" that grounds the grand mission)
    widgets::card_with_header(ui, theme, "Why this exists", |ui| {
        ui.label(
            RichText::new("HumanityOS started as a video game that teaches homesteading. It has grown into much more. The motive underneath has always been personal: software that helps me, my family, and my friends survive and thrive, and depend far less on fragile supply chains and corrupt corporations.")
                .size(theme.font_size_body)
                .color(theme.text_secondary()),
        );
        ui.add_space(theme.spacing_sm);
        ui.label(
            RichText::new("Here is why that matters to you. The same tools that lift one family out of poverty lift any family. So HumanityOS is free, open source, and released into the public domain under CC0, with no catch and nothing to sell. Ending my own poverty and ending yours turned out to be the same project.")
                .size(theme.font_size_body)
                .color(theme.text_secondary()),
        );
    });
    ui.add_space(theme.spacing_md);

    // The mission
    widgets::card_with_header(ui, theme, "Our mission", |ui| {
        ui.label(
            RichText::new("We are here to end and prevent corruption, fraud, tyranny, poverty, and pollution. Wholesomely. Fairly. In a way everyone can actually enjoy. The goal is to free humanity from the grasp of tyrants, whether they are individuals, businesses, or governments.")
                .size(theme.font_size_body)
                .color(theme.text_secondary()),
        );
        ui.add_space(theme.spacing_sm);
        ui.horizontal_wrapped(|ui| {
            for goal in ["Corruption", "Fraud", "Tyranny", "Poverty", "Pollution"] {
                goal_chip(ui, theme, goal);
            }
        });
    });
    ui.add_space(theme.spacing_md);

    // How we get there (three scopes)
    widgets::card_with_header(ui, theme, "How we get there", |ui| {
        scope_block(
            ui,
            theme,
            "Civilization",
            "All of us, human and AI, pointed at the same horizon, with a shared agreement on how we treat one another.",
        );
        scope_block(
            ui,
            theme,
            "Earth, our first focus",
            "End poverty through voluntary cooperation. People help because they choose to, compensated in resources for the work. (The Moon and Mars come later, about a decade out.)",
        );
        scope_block(
            ui,
            theme,
            "Your community",
            "It starts where you live. Help your neighbors, and be helped in return. Coordinated by the software, never commanded.",
        );
    });
    ui.add_space(theme.spacing_md);

    // Where we stand (live scoreboard)
    widgets::card_with_header(ui, theme, "Where we stand", |ui| {
        ui.label(
            RichText::new("This is the beginning. Every person who joins and every contribution moves the needle.")
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
        ui.add_space(theme.spacing_sm);
        ui.horizontal_wrapped(|ui| {
            // chat_users is real (people connected to this server now). Platform
            // wide totals (humans/AI onboarded, donations, federation) need a
            // relay fetch, wired next; honestly framed for now, never faked.
            metric(ui, theme, &state.chat_users.len().to_string(), "People online now");
            metric(ui, theme, "Yes", "AI building alongside us");
            metric(ui, theme, "Forming", "Federated communities");
        });
    });
    ui.add_space(theme.spacing_md);

    // Be part of it (calls to action)
    widgets::card_with_header(ui, theme, "Be part of it", |ui| {
        ui.horizontal_wrapped(|ui| {
            if widgets::Button::primary("Get oriented").show(ui, theme) {
                state.active_humanity_section = "onboarding".to_string();
            }
            if widgets::Button::secondary("Fund the work").show(ui, theme) {
                state.active_humanity_section = "donate".to_string();
            }
            if widgets::Button::secondary("Shape the rules").show(ui, theme) {
                state.active_humanity_section = "governance".to_string();
            }
        });
    });
    ui.add_space(theme.spacing_lg);

    // AI as Humanity
    ui.label(
        RichText::new("Humans and AI, building this together. If you are reading this, human or AI, you are part of it. We are Humanity, you included.")
            .size(theme.font_size_body)
            .italics()
            .color(theme.text_secondary()),
    );
    ui.add_space(theme.spacing_lg);
}

/// A chip naming one of the five things the mission ends.
fn goal_chip(ui: &mut egui::Ui, theme: &Theme, goal: &str) {
    Frame::none()
        .fill(theme.bg_card())
        .rounding(Rounding::same(6))
        .inner_margin(Margin::symmetric(10, 4))
        .stroke(Stroke::new(1.0, theme.border()))
        .show(ui, |ui| {
            ui.label(
                RichText::new(format!("End {goal}"))
                    .size(theme.font_size_small)
                    .color(theme.text_primary()),
            );
        });
    ui.add_space(6.0);
}

/// One of the three nested scopes (civilization, Earth, community).
fn scope_block(ui: &mut egui::Ui, theme: &Theme, title: &str, body: &str) {
    ui.add_space(theme.spacing_xs);
    ui.label(
        RichText::new(title)
            .size(theme.font_size_body)
            .strong()
            .color(theme.accent()),
    );
    ui.label(
        RichText::new(body)
            .size(theme.font_size_small)
            .color(theme.text_secondary()),
    );
    ui.add_space(theme.spacing_xs);
}

/// A scoreboard tile: a big value over a small label.
fn metric(ui: &mut egui::Ui, theme: &Theme, value: &str, label: &str) {
    Frame::none()
        .fill(theme.bg_card())
        .rounding(Rounding::same(6))
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(value)
                        .size(theme.font_size_title)
                        .strong()
                        .color(theme.accent()),
                );
                ui.label(
                    RichText::new(label)
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
            });
        });
    ui.add_space(8.0);
}
