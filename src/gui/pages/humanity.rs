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

    // Why it's built this way: each system's design mapped to the poverty-ending
    // job it does. Operator 2026-06-05: lean into HOW HumanityOS actually ends
    // poverty and WHY each system, by its design and intended use, serves that,
    // the way the Onboarding page is concrete about it. This is the mechanism,
    // the heart of the pitch, not just the aspiration.
    widgets::card_with_header(ui, theme, "Why it's built this way", |ui| {
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
    widgets::card_with_header(ui, theme, "What it protects", |ui| {
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
    });
    ui.add_space(theme.spacing_md);

    // Built for every situation: the resilience / universality angle the operator
    // asked for (2026-06-05: "make sure what we have or are planning actually helps
    // them in ALL scenarios"). Mined from the docs (offline-first, federation +
    // no-home-server identity, social recovery, no-accounts + VCs/reputation,
    // accessibility + i18n, off-site backups; radio mesh flagged in-progress so the
    // pitch stays honest, shipped vs planned).
    widgets::card_with_header(ui, theme, "Built for every situation", |ui| {
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
