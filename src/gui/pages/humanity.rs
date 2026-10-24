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

use egui::{Align2, Frame, Margin, RichText, Rounding, ScrollArea, Stroke};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::i18n::{t, t_marked};
use crate::gui::widgets::{self, with_hint, Hint, SectionNavItem};
use super::{civilization, governance, identity, donate};

/// The Humanity Accord, embedded at compile time from the canonical repo copy so
/// it is readable in-app with no network and no separate data file to ship or let
/// drift. A stable CC0 document; editing docs/accord/humanity_accord.md and
/// rebuilding updates the in-app copy. (Operator 2026-06-06: "a button that takes
/// users directly to the Humanity Accord inside the app.")
const ACCORD_TEXT: &str = include_str!("../../../docs/accord/humanity_accord.md");

/// In-app document viewer state (a simple modal). Lives in a thread_local like
/// the other page-local UI state, so it needs no GuiState plumbing. The planned
/// Library will reuse this same viewer to open any doc.
struct DocViewer {
    open: bool,
    title: String,
    body: String,
}

fn doc_viewer<R>(f: impl FnOnce(&mut DocViewer) -> R) -> R {
    use std::cell::RefCell;
    thread_local! {
        static DV: RefCell<DocViewer> = RefCell::new(DocViewer {
            open: false,
            title: String::new(),
            body: String::new(),
        });
    }
    DV.with(|d| f(&mut d.borrow_mut()))
}

/// Open the in-app doc viewer with the given title and raw markdown body.
fn open_doc(title: &str, body: &str) {
    doc_viewer(|d| {
        d.open = true;
        d.title = title.to_string();
        d.body = body.to_string();
    });
}

/// Render the doc-viewer modal if it is open. Drawn from `draw` so it floats over
/// whichever section is showing. The window's close button drives `open` back to
/// false.
fn render_doc_modal(ctx: &egui::Context, theme: &Theme) {
    if !doc_viewer(|d| d.open) {
        return;
    }
    let mut still_open = true;
    doc_viewer(|d| {
        egui::Window::new(
            RichText::new(d.title.as_str())
                .size(theme.font_size_heading)
                .strong()
                .color(theme.text_primary()),
        )
        .open(&mut still_open)
        .collapsible(false)
        .resizable(true)
        .default_width(720.0)
        .default_height(560.0)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .frame(
            Frame::none()
                .fill(theme.bg_panel())
                .inner_margin(theme.card_padding)
                .stroke(Stroke::new(1.0, theme.border()))
                .rounding(Rounding::same(8)),
        )
        .show(ctx, |ui| {
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                widgets::markdown::render_markdown(ui, theme, d.body.as_str());
            });
        });
    });
    if !still_open {
        doc_viewer(|d| d.open = false);
    }
}

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
                SectionNavItem::new("civilization", t("humanity.nav_dashboard"), c).hint("humanity.nav_dashboard_hint"), // i18n-exempt: the section's id, not words
                SectionNavItem::new("governance", t("governance.title"), c).hint("humanity.nav_governance_hint"), // i18n-exempt: the section's id, not words
                SectionNavItem::new("laws", t("humanity.nav_laws"), c).hint("humanity.nav_laws_hint"), // i18n-exempt: the section's id, not words
                SectionNavItem::new("identity", t("humanity.nav_directory"), c).hint("humanity.nav_directory_hint"), // i18n-exempt: the section's id, not words
                SectionNavItem::new("donate", t("nav.donate"), c).hint("humanity.nav_donate_hint"), // i18n-exempt: the section's id, not words
            ];
            if let Some(clicked) = widgets::section_nav(
                ui,
                theme,
                Some(t("humanity.title")),
                &items,
                &state.active_humanity_section,
            ) {
                state.active_humanity_section = clicked;
            }
        });

    let section = state.active_humanity_section.clone();
    match section.as_str() {
        "governance" => governance::draw(ctx, theme, state),
        "laws" => super::laws::draw(ctx, theme, state),
        "identity" => identity::draw(ctx, theme, state),
        "donate" => donate::draw(ctx, theme, state),
        // v0.415.0: "onboarding" + "resources" arms removed with their retired
        // pages (neither was in the sidebar; Library + the dashboard cover them).
        // Default = the Mission Dashboard (the real Humanity landing).
        _ => {
            // The Mission Dashboard embeds the live community dashboard, so its
            // relay fetch has to be pumped from here (poll_stats needs the
            // Context; draw_mission_dashboard only gets a Ui).
            civilization::poll_stats(ctx, state);
            egui::CentralPanel::default()
                .frame(Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
                .show(ctx, |ui| {
                    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        draw_mission_dashboard(ui, theme, state);
                    });
                });
        }
    }

    // In-app document viewer overlay (e.g. the Humanity Accord). Drawn last so it
    // floats over whichever section is showing.
    render_doc_modal(ctx, theme);
}

/// The mission of our civilization. The landing the H button deserves.
fn draw_mission_dashboard(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // Hero: an accent top rule + a heart icon badge next to the title (visual
    // pass, v0.662 -- the page read as "slapped together" with 9 identical grey
    // cards and a bare text hero).
    ui.add_space(theme.spacing_sm);
    let rule_w = ui.available_width();
    let (rule, _) = ui.allocate_exact_size(egui::Vec2::new(rule_w, 3.0), egui::Sense::hover());
    ui.painter().rect_filled(rule, Rounding::same(2), theme.accent());
    ui.add_space(theme.spacing_md);
    ui.horizontal(|ui| {
        let (badge, _) = ui.allocate_exact_size(egui::Vec2::splat(36.0), egui::Sense::hover());
        ui.painter().rect_filled(badge, Rounding::same(8), theme.accent());
        widgets::icons::paint_heart(ui.painter(), badge.shrink(8.0), theme.text_on_accent());
        ui.add_space(theme.spacing_sm);
        // Heading = the nav label ("Humanity", the tab), not the product
        // name (naming rule 2026-08-16). The mission line below stays.
        ui.label(
            RichText::new(t("humanity.title"))
                .size(theme.font_size_title * 1.3)
                .strong()
                .color(theme.text_primary()),
        );
    });
    ui.add_space(theme.spacing_xs);
    ui.label(
        RichText::new(t("humanity.mission"))
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
        RichText::new(t("humanity.for_you"))
            .size(theme.font_size_body)
            .color(theme.text_secondary()),
    );
    ui.add_space(theme.spacing_xs);
    ui.label(
        RichText::new(t("humanity.for_all"))
            .size(theme.font_size_body)
            .color(theme.text_secondary()),
    );
    ui.add_space(theme.spacing_lg);

    // Start here: the first action a newcomer can take, right under the hero, so
    // the page leads with "what do I do" before the deeper manifesto below.
    accent_card_with_header(ui, theme, t("humanity.start_here"), |ui| {
        ui.label(
            RichText::new(t("humanity.start_here_desc"))
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
        ui.add_space(theme.spacing_sm);
        ui.horizontal_wrapped(|ui| {
            if widgets::Button::primary(t("humanity.get_oriented")).hint("humanity.get_oriented_hint").show(ui, theme) {
                // Quests is its own top-level page now (not a Real-tab section);
                // the old `active_real_section = "quests"` was an unknown id that
                // silently fell through to Profile > Body & Measurements (v0.699 fix).
                state.active_page = crate::gui::GuiPage::Quests;
            }
            if widgets::Button::secondary(t("humanity.see_laws")).hint("humanity.see_laws_hint").show(ui, theme) {
                state.active_humanity_section = "laws".to_string();
            }
            if widgets::Button::secondary(t("humanity.fund_work")).hint("humanity.fund_work_hint").show(ui, theme) {
                state.active_humanity_section = "donate".to_string();
            }
            if widgets::Button::secondary(t("humanity.shape_rules")).hint("humanity.shape_rules_hint").show(ui, theme) {
                state.active_humanity_section = "governance".to_string();
            }
        });
    });
    ui.add_space(theme.spacing_md);

    // Why this exists (the personal "why" that grounds the grand mission)
    quiet_card_with_header(ui, theme, t("humanity.why_exists"), |ui| {
        ui.label(
            RichText::new(t("humanity.why_exists_1"))
                .size(theme.font_size_body)
                .color(theme.text_secondary()),
        );
        ui.add_space(theme.spacing_sm);
        widgets::glossary_words(ui, theme, t_marked("humanity.why_exists_2"), theme.font_size_body, theme.text_secondary());
    });
    ui.add_space(theme.spacing_md);

    // Early access, building in public (operator 2026-06-06: state plainly that
    // the app is early, not everything works, and that building in the open with
    // real users is how it gets fixed; no small team can test everything). Sits
    // high, right after the personal why, so it frames the capability claims that
    // follow (which themselves flag anything not ready as "in progress").
    quiet_card_with_header(ui, theme, t("humanity.early_days"), |ui| {
        ui.label(
            RichText::new(t("humanity.early_days_1"))
                .size(theme.font_size_body)
                .color(theme.text_secondary()),
        );
        ui.add_space(theme.spacing_sm);
        ui.label(
            RichText::new(t("humanity.early_days_2"))
                .size(theme.font_size_body)
                .color(theme.text_secondary()),
        );
    });
    ui.add_space(theme.spacing_md);

    // The mission
    quiet_card_with_header(ui, theme, t("humanity.our_mission"), |ui| {
        ui.label(
            RichText::new(t("humanity.mission_body"))
                .size(theme.font_size_body)
                .color(theme.text_secondary()),
        );
        ui.add_space(theme.spacing_sm);
        ui.horizontal_wrapped(|ui| {
            for goal in [
                t("humanity.end_corruption"),
                t("humanity.end_fraud"),
                t("humanity.end_tyranny"),
                t("humanity.end_poverty"),
                t("humanity.end_pollution"),
            ] {
                goal_chip(ui, theme, goal);
            }
        });
    });
    ui.add_space(theme.spacing_md);

    // Why it's built this way: each system's design mapped to the poverty-ending
    // job it does. Operator 2026-06-05: lean into HOW HumanityOS actually ends
    // poverty and WHY each system, by its design and intended use, serves that,
    // the way the Onboarding page is concrete about it. This is the mechanism,
    // the heart of the pitch, not just the aspiration.
    with_hint(ui, "humanity.why_built_hint", |ui| widgets::collapsible_section(ui, t("humanity.why_built"), false, |ui| {
        ui.label(
            RichText::new(t("humanity.why_built_body"))
                .size(theme.font_size_body)
                .color(theme.text_secondary()),
        );
        ui.add_space(theme.spacing_sm);
        scope_block(ui, theme, t("humanity.why_quests"),
            t("humanity.why_quests_body"));
        scope_block(ui, theme, t("humanity.why_chat"),
            t("humanity.why_chat_body"));
        scope_block(ui, theme, t("humanity.why_tasks"),
            t("humanity.why_tasks_body"));
        scope_block(ui, theme, t("humanity.why_maps"),
            t("humanity.why_maps_body"));
        scope_block(ui, theme, t("humanity.why_market"),
            t("humanity.why_market_body"));
        scope_block(ui, theme, t("humanity.why_identity"),
            t_marked("humanity.why_identity_body"));
        scope_block(ui, theme, t("humanity.why_remade"),
            t("humanity.why_remade_body"));
        scope_block(ui, theme, t("humanity.why_public"),
            t("humanity.why_public_body"));
    }));
    ui.add_space(theme.spacing_md);

    // What it protects: the freedoms a free people needs, and how the design
    // defends each one. Operator 2026-06-05: lean into the per-feature detail,
    // especially free speech ("key to a healthy civilization; loss of free
    // speech is a death sentence for a free and independent people"). Ending
    // poverty is not enough if the result is a cage, so this sits beside the
    // poverty mechanism above, the liberty half of the same promise.
    with_hint(ui, "humanity.protects_hint", |ui| widgets::collapsible_section(ui, t("humanity.protects"), false, |ui| {
        ui.label(
            RichText::new(t("humanity.protects_body"))
                .size(theme.font_size_body)
                .color(theme.text_secondary()),
        );
        ui.add_space(theme.spacing_sm);
        scope_block(ui, theme, t("humanity.protects_speech"),
            t("humanity.protects_speech_body"));
        scope_block(ui, theme, t("humanity.protects_privacy"),
            t_marked("humanity.protects_privacy_body"));
        scope_block(ui, theme, t("humanity.protects_yours"),
            t_marked("humanity.protects_yours_body"));
        scope_block(ui, theme, t("humanity.protects_locked_out"),
            t_marked("humanity.protects_locked_out_body"));
        scope_block(ui, theme, t("humanity.protects_rules"),
            t_marked("humanity.protects_rules_body"));
        scope_block(ui, theme, t("humanity.protects_constitution"),
            t_marked("humanity.protects_constitution_body"));
        ui.add_space(theme.spacing_xs);
        if widgets::Button::secondary(t("humanity.read_accord")).hint("humanity.read_accord_hint").show(ui, theme) {
            open_doc(t("humanity.accord_title"), ACCORD_TEXT);
        }
    }));
    ui.add_space(theme.spacing_md);

    // Built for every situation: the resilience / universality angle the operator
    // asked for (2026-06-05: "make sure what we have or are planning actually helps
    // them in ALL scenarios"). Mined from the docs (offline-first, federation +
    // no-home-server identity, social recovery, no-accounts + VCs/reputation,
    // accessibility + i18n, off-site backups; radio mesh flagged in-progress so the
    // pitch stays honest, shipped vs planned).
    with_hint(ui, "humanity.every_situation_hint", |ui| widgets::collapsible_section(ui, t("humanity.every_situation"), false, |ui| {
        ui.label(
            RichText::new(t("humanity.every_situation_body"))
                .size(theme.font_size_body)
                .color(theme.text_secondary()),
        );
        ui.add_space(theme.spacing_sm);
        scope_block(ui, theme, t("humanity.situation_offline"),
            t("humanity.situation_offline_body"));
        scope_block(ui, theme, t("humanity.situation_server_down"),
            t("humanity.situation_server_down_body"));
        scope_block(ui, theme, t("humanity.situation_lost_device"),
            t_marked("humanity.situation_lost_device_body"));
        scope_block(ui, theme, t("humanity.situation_no_money"),
            t("humanity.situation_no_money_body"));
        scope_block(ui, theme, t("humanity.situation_language"),
            t("humanity.situation_language_body"));
        scope_block(ui, theme, t("humanity.situation_disaster"),
            t_marked("humanity.situation_disaster_body"));
    }));
    ui.add_space(theme.spacing_md);

    // How we get there (three scopes)
    quiet_card_with_header(ui, theme, t("humanity.get_there"), |ui| {
        scope_block(
            ui,
            theme,
            t("humanity.get_there_civ"),
            t("humanity.get_there_civ_body"),
        );
        scope_block(
            ui,
            theme,
            t("humanity.get_there_earth"),
            t("humanity.get_there_earth_body"),
        );
        scope_block(
            ui,
            theme,
            t("humanity.get_there_community"),
            t("humanity.get_there_community_body"),
        );
    });
    ui.add_space(theme.spacing_md);

    // Where we stand: the REAL community dashboard, live from the relay.
    //
    // Until now this section only borrowed civilization.rs's `draw_stat_card`
    // (v0.662) for three local tiles, one of which counted `chat_users` while
    // the other two were placeholder words. The full page it borrowed from was
    // already built and already fetching GET /api/civilization, but nothing in
    // the app ever routed to it, so members / messages / market / tasks /
    // follows were invisible to users. It renders HERE now (same fetch, same
    // GuiState cache, one visual language). The standalone Civilization page
    // still exists and still works; this is a rescue, not a replacement.
    widgets::card_with_header(ui, theme, t("humanity.where_we_stand"), |ui| {
        ui.label(
            RichText::new(t("humanity.where_we_stand_body"))
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
        ui.add_space(theme.spacing_sm);
        ui.horizontal(|ui| {
            if widgets::Button::secondary(t("common.refresh")).hint("civilization.refresh_hint").show(ui, theme) {
                // poll_stats (pumped from `draw`) re-fetches on the next frame.
                state.civ_stats_loaded = false;
            }
        });
        ui.add_space(theme.spacing_xs);
        civilization::draw_live_body(ui, theme, state);
        ui.add_space(theme.spacing_md);
        // The two things the mission is measured by that no relay counter can
        // report. Qualitative on purpose: never dressed up as a fake number.
        // A Grid (not horizontal_wrapped) because wrapped rows baseline-drift as
        // tile heights differ, which stair-stepped the old scoreboard (operator
        // screenshot 2026-07-04); a Grid top-aligns every cell.
        egui::Grid::new("humanity_scoreboard_grid")
            .num_columns(2)
            .spacing(egui::Vec2::new(theme.spacing_sm, theme.spacing_sm))
            .show(ui, |ui| {
                civilization::draw_stat_card(ui, theme, t("humanity.stat_ai"), t("humanity.stat_ai_value"), "", 0.0);
                civilization::draw_stat_card(ui, theme, t("humanity.stat_federated"), t("humanity.stat_federated_value"), "", 0.0);
                ui.end_row();
            });
    });
    ui.add_space(theme.spacing_md);

    // Be part of it (calls to action)
    widgets::card_with_header(ui, theme, t("humanity.be_part"), |ui| {
        ui.horizontal_wrapped(|ui| {
            if widgets::Button::primary(t("humanity.get_oriented")).hint("humanity.get_oriented_hint").show(ui, theme) {
                // Onboarding was retired into the unified Quests (Real tab) on
                // 2026-06-06; "Get oriented" now jumps straight to the quests.
                // Quests is its own top-level page now (not a Real-tab section);
                // the old `active_real_section = "quests"` was an unknown id that
                // silently fell through to Profile > Body & Measurements (v0.699 fix).
                state.active_page = crate::gui::GuiPage::Quests;
            }
            if widgets::Button::secondary(t("humanity.fund_work")).hint("humanity.fund_work_hint").show(ui, theme) {
                state.active_humanity_section = "donate".to_string();
            }
            if widgets::Button::secondary(t("humanity.shape_rules")).hint("humanity.shape_rules_hint").show(ui, theme) {
                state.active_humanity_section = "governance".to_string();
            }
        });
    });
    ui.add_space(theme.spacing_lg);

    // AI as Humanity
    ui.label(
        RichText::new(t("humanity.together"))
            .size(theme.font_size_body)
            .italics()
            .color(theme.text_secondary()),
    );
    ui.add_space(theme.spacing_lg);
}

/// Emphasized card: accent-stroked, for the page's primary call to action
/// ("Start here"). Part of the v0.662 visual-weight pass: the CTA pops, the
/// long-form essays recede, instead of nine identical grey cards.
fn accent_card_with_header(
    ui: &mut egui::Ui,
    theme: &Theme,
    title: &str,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    Frame::none()
        .fill(theme.bg_card())
        .rounding(Rounding::same(theme.border_radius as u8))
        .inner_margin(theme.card_padding)
        .stroke(Stroke::new(1.5, theme.accent()))
        .show(ui, |ui| {
            ui.label(
                RichText::new(title)
                    .size(theme.font_size_heading)
                    .color(theme.accent()),
            );
            ui.add_space(theme.spacing_sm);
            add_contents(ui);
        });
}

/// Quiet card: borderless, for long-form essay/manifesto sections that should
/// read as prose blocks rather than compete with the calls to action.
fn quiet_card_with_header(
    ui: &mut egui::Ui,
    theme: &Theme,
    title: &str,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    Frame::none()
        .fill(theme.bg_card())
        .rounding(Rounding::same(theme.border_radius as u8))
        .inner_margin(theme.card_padding)
        .show(ui, |ui| {
            ui.label(
                RichText::new(title)
                    .size(theme.font_size_heading)
                    .color(theme.text_primary()),
            );
            ui.add_space(theme.spacing_sm);
            add_contents(ui);
        });
}

/// A chip naming one of the five things the mission ends ("End Poverty").
fn goal_chip(ui: &mut egui::Ui, theme: &Theme, goal: &str) {
    Frame::none()
        .fill(theme.bg_card())
        .rounding(Rounding::same(6))
        .inner_margin(Margin::symmetric(10, 4))
        .stroke(Stroke::new(1.0, theme.border()))
        .show(ui, |ui| {
            ui.label(
                RichText::new(goal)
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
    // Its words may mark glossary terms ([[term]]): underlined, explained on hover.
    widgets::glossary_words(ui, theme, body, theme.font_size_small, theme.text_secondary());
    ui.add_space(theme.spacing_xs);
}

// The old local `metric` tile was replaced by civilization.rs's shared
// `draw_stat_card` (v0.662) so the Mission Dashboard and Civilization pages
// share one stat-tile visual language.
