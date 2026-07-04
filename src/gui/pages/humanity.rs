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
use crate::gui::widgets::{self, SectionNavItem};
use super::{governance, identity, donate};

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
                SectionNavItem::new("civilization", "Mission Dashboard", c),
                SectionNavItem::new("governance", "Governance", c),
                SectionNavItem::new("laws", "Laws", c),
                SectionNavItem::new("identity", "Directory", c),
                SectionNavItem::new("donate", "Donate", c),
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
        "laws" => super::laws::draw(ctx, theme, state),
        "identity" => identity::draw(ctx, theme, state),
        "donate" => donate::draw(ctx, theme, state),
        // v0.415.0: "onboarding" + "resources" arms removed with their retired
        // pages (neither was in the sidebar; Library + the dashboard cover them).
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
        ui.label(
            RichText::new("HumanityOS")
                .size(theme.font_size_title * 1.3)
                .strong()
                .color(theme.text_primary()),
        );
    });
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

    // Start here: the first action a newcomer can take, right under the hero, so
    // the page leads with "what do I do" before the deeper manifesto below.
    accent_card_with_header(ui, theme, "Start here", |ui| {
        ui.label(
            RichText::new("New here? Pick a place to begin, then read as much or as little below as you like.")
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
        ui.add_space(theme.spacing_sm);
        ui.horizontal_wrapped(|ui| {
            if widgets::Button::primary("Get oriented").show(ui, theme) {
                state.active_page = crate::gui::GuiPage::Real;
                state.active_real_section = "quests".to_string();
            }
            if widgets::Button::secondary("See your Laws").show(ui, theme) {
                state.active_humanity_section = "laws".to_string();
            }
            if widgets::Button::secondary("Fund the work").show(ui, theme) {
                state.active_humanity_section = "donate".to_string();
            }
            if widgets::Button::secondary("Shape the rules").show(ui, theme) {
                state.active_humanity_section = "governance".to_string();
            }
        });
    });
    ui.add_space(theme.spacing_md);

    // Why this exists (the personal "why" that grounds the grand mission)
    quiet_card_with_header(ui, theme, "Why this exists", |ui| {
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

    // Early access, building in public (operator 2026-06-06: state plainly that
    // the app is early, not everything works, and that building in the open with
    // real users is how it gets fixed; no small team can test everything). Sits
    // high, right after the personal why, so it frames the capability claims that
    // follow (which themselves flag anything not ready as "in progress").
    quiet_card_with_header(ui, theme, "Early days, built in the open", |ui| {
        ui.label(
            RichText::new("HumanityOS is early, and we build it in public on purpose. A lot already works. Some of it is half-built, some is rough, and some will break. Where a feature is not ready, we say so plainly (you will see 'in progress'), because hiding it would betray the whole reason this exists.")
                .size(theme.font_size_body)
                .color(theme.text_secondary()),
        );
        ui.add_space(theme.spacing_sm);
        ui.label(
            RichText::new("No team, ours included, can test everything. Real people, in every kind of place, on every kind of device and connection, find what we never could. So a bug you hit or a gap you notice is not the project failing. It is the project working: building in the open is how it gets better. Tell us what breaks, and you are already helping build it.")
                .size(theme.font_size_body)
                .color(theme.text_secondary()),
        );
    });
    ui.add_space(theme.spacing_md);

    // The mission
    quiet_card_with_header(ui, theme, "Our mission", |ui| {
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

    // Why it's built this way: each system's design mapped to the poverty-ending
    // job it does. Operator 2026-06-05: lean into HOW HumanityOS actually ends
    // poverty and WHY each system, by its design and intended use, serves that,
    // the way the Onboarding page is concrete about it. This is the mechanism,
    // the heart of the pitch, not just the aspiration.
    widgets::collapsible_section(ui, "Why it's built this way", false, |ui| {
        ui.label(
            RichText::new("Poverty is forced dependence. When you cannot provide your own water, food, or power, someone else sets the price of your survival. Every part of HumanityOS is built to remove that dependence: teach the skills, connect the people, and cut out the middlemen. Here is how each part pulls its weight.")
                .size(theme.font_size_body)
                .color(theme.text_secondary()),
        );
        ui.add_space(theme.spacing_sm);
        scope_block(ui, theme, "Guided quests and a free-to-fail simulation",
            "Learn to collect water, grow food, and generate power by doing it in the simulation first, where a mistake costs nothing. The skills carry straight into real life, so a lack of know-how is never what keeps you poor.");
        scope_block(ui, theme, "Encrypted chat, no account needed",
            "Find people already doing it and learn from them directly. No signup, no gatekeeper, and nothing harvested, so no company can lock you out, sell your attention, or decide who is allowed to take part.");
        scope_block(ui, theme, "Tasks and a private notebook",
            "Turn a vague hope (get off the water bill) into a plan you actually finish, and keep a private record of what works, so your hard-won experience compounds instead of evaporating.");
        scope_block(ui, theme, "Maps of what is near you",
            "See the gardens, tools, workshops, and people around you. Providing for yourself is easier together, and cooperation is easiest when it is local.");
        scope_block(ui, theme, "A marketplace with trust scores",
            "Trade your surplus straight with your neighbors. No middleman takes a cut, so the value you create stays with you and your community instead of leaking away to a distant corporation.");
        scope_block(ui, theme, "An identity you own, on a network no one owns",
            "Your identity is a key on your device, not an account a company can suspend. The network is federated, with no single owner and no single point of failure, so it cannot be bought, censored, or switched off.");
        scope_block(ui, theme, "Built to be remade by anyone",
            "Every plant, recipe, quest, and price is a plain data file, not locked-up code. A teacher in a remote village can add the crops that grow in their soil and the lessons their people need, with no programming, and pass it on as a single file. The tool bends to fit the place, instead of forcing every place to fit the tool.");
        scope_block(ui, theme, "Public domain, forever",
            "The whole thing belongs to everyone and can never be fenced off or sold back to you. The tools that end one family's poverty stay free for every family, on this world and the next.");
    });
    ui.add_space(theme.spacing_md);

    // What it protects: the freedoms a free people needs, and how the design
    // defends each one. Operator 2026-06-05: lean into the per-feature detail,
    // especially free speech ("key to a healthy civilization; loss of free
    // speech is a death sentence for a free and independent people"). Ending
    // poverty is not enough if the result is a cage, so this sits beside the
    // poverty mechanism above, the liberty half of the same promise.
    widgets::collapsible_section(ui, "What it protects", false, |ui| {
        ui.label(
            RichText::new("Ending poverty is not enough if the result is a cage. A free people needs more than full bellies. It needs the freedoms that keep power honest, and those have to be built into the tools, not promised on top of them.")
                .size(theme.font_size_body)
                .color(theme.text_secondary()),
        );
        ui.add_space(theme.spacing_sm);
        scope_block(ui, theme, "Free speech",
            "A people who cannot speak freely cannot defend anything else they have. Losing that voice is a death sentence for a free and independent people, so it comes first. There is no central censor and no off switch: messages are encrypted, your identity lives on your device, and once two people connect they can speak directly with no server in the middle. No company, government, or mob can quietly erase what you said or forbid you from saying it.");
        scope_block(ui, theme, "Privacy, even from us",
            "Surveillance is how control begins. We collect nothing to sell and keep as little as we can. Your private messages are sealed on your device with post-quantum encryption, the kind built to resist even tomorrow's computers, and the server holds nothing but unreadable ciphertext. Even we cannot read them, and a warrant cannot force us to hand over what does not exist.");
        scope_block(ui, theme, "What is yours stays yours",
            "Your identity, your keys, your words, and your tools belong to you, not to a platform that can revoke them. The whole system is public domain, so it can never be bought, locked down, or rented back to you.");
        scope_block(ui, theme, "You can never be locked out",
            "There is no account to suspend and no password to lose. Your identity is a key on your own device, recoverable from your seed phrase or from trusted friends who each hold an encrypted piece of it. No company, and no fee, stands between you and your own name.");
        scope_block(ui, theme, "Rules made by the people they bind",
            "The community sets its own rules through transparent voting, weighted by trust and capped so that no single person, however trusted, can dominate. AI take part openly but do not vote, because consent belongs to the people whose lives the rules govern.");
        scope_block(ui, theme, "Bound by a constitution, not a promise",
            "None of this rests on trusting us. It rests on the Humanity Accord, a public-domain constitution that places dignity, consent, transparency, and freedom from domination above any operator, company, or government. Anyone can read it, adopt it for their own community, or hold us to it.");
        ui.add_space(theme.spacing_xs);
        if widgets::Button::secondary("Read the Humanity Accord").show(ui, theme) {
            open_doc("The Humanity Accord", ACCORD_TEXT);
        }
    });
    ui.add_space(theme.spacing_md);

    // Built for every situation: the resilience / universality angle the operator
    // asked for (2026-06-05: "make sure what we have or are planning actually helps
    // them in ALL scenarios"). Mined from the docs (offline-first, federation +
    // no-home-server identity, social recovery, no-accounts + VCs/reputation,
    // accessibility + i18n, off-site backups; radio mesh flagged in-progress so the
    // pitch stays honest, shipped vs planned).
    widgets::collapsible_section(ui, "Built for every situation", false, |ui| {
        ui.label(
            RichText::new("A tool that only works when everything is going well is not much help. HumanityOS is built to keep working when things go wrong, wherever you are and whatever you have.")
                .size(theme.font_size_body)
                .color(theme.text_secondary()),
        );
        ui.add_space(theme.spacing_sm);
        scope_block(ui, theme, "When there is no internet",
            "The app, your data, your saved work, and your skills all live on your device. You can learn, plan, and build completely offline, and sync only when, and if, a connection comes back. A mountain village, a boat, or a blackout does not stop you.");
        scope_block(ui, theme, "When the server goes down",
            "No one owns the network. Anyone can run their own in minutes, and your identity moves with you to any of them, because it is yours, not an account on someone else's machine. Take one server down and the rest carry on.");
        scope_block(ui, theme, "When you lose your device",
            "Recover everything from your 24-word seed phrase, or from trusted friends who each hold an encrypted piece of it. No email, no phone number, no recovery fee, and no company that can refuse you.");
        scope_block(ui, theme, "When you have no money, papers, or bank",
            "No accounts, no subscriptions, no fees. Reputation you earn and credentials your neighbors sign stand in for credit scores and ID, so a refugee, a young person, or anyone starting over can build a real, verifiable history from zero.");
        scope_block(ui, theme, "Whatever your language or ability",
            "It speaks several languages, with high-contrast, colorblind, and reduced-motion modes, keyboard-only navigation, and a plain-language glossary for every term. It runs on cheap, old, low-power hardware, not just new machines.");
        scope_block(ui, theme, "When disaster strikes",
            "Off-site backups survive a fire, a flood, or a seizure, so a community can rebuild from nothing. Radio links that need no internet (in progress) aim to carry the essentials, calls for help, recovery, coordination, even when the grid and the network are down.");
    });
    ui.add_space(theme.spacing_md);

    // How we get there (three scopes)
    quiet_card_with_header(ui, theme, "How we get there", |ui| {
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
        // chat_users is real (people connected to this server now). Platform
        // wide totals (humans/AI onboarded, donations, federation) need a
        // relay fetch, wired next; honestly framed for now, never faked.
        // Stat cards reuse civilization.rs's draw_stat_card (v0.662) AND its
        // Grid container (v0.684): the earlier horizontal_wrapped layout
        // stair-stepped the three tiles downward (wrapped rows baseline-drift
        // as item heights differ -- operator screenshot 2026-07-04); a Grid
        // top-aligns every cell, matching the Civilization page exactly.
        egui::Grid::new("humanity_scoreboard_grid")
            .num_columns(3)
            .spacing(egui::Vec2::new(theme.spacing_sm, theme.spacing_sm))
            .show(ui, |ui| {
                super::civilization::draw_stat_card(ui, theme, "People online now", &state.chat_users.len().to_string(), "", 0.0);
                super::civilization::draw_stat_card(ui, theme, "AI building alongside us", "Yes", "", 0.0);
                super::civilization::draw_stat_card(ui, theme, "Federated communities", "Forming", "", 0.0);
                ui.end_row();
            });
    });
    ui.add_space(theme.spacing_md);

    // Be part of it (calls to action)
    widgets::card_with_header(ui, theme, "Be part of it", |ui| {
        ui.horizontal_wrapped(|ui| {
            if widgets::Button::primary("Get oriented").show(ui, theme) {
                // Onboarding was retired into the unified Quests (Real tab) on
                // 2026-06-06; "Get oriented" now jumps straight to the quests.
                state.active_page = crate::gui::GuiPage::Real;
                state.active_real_section = "quests".to_string();
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

// The old local `metric` tile was replaced by civilization.rs's shared
// `draw_stat_card` (v0.662) so the Mission Dashboard and Civilization pages
// share one stat-tile visual language.
