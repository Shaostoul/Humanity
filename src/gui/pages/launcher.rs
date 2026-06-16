//! Character launcher (v0.474) -- what the Play button opens.
//!
//! Before this, Play dropped you straight into the 3D world (active_page = None).
//! Now Play opens this launcher: a character/home picker modeled on the
//! self-custodial vs server-authoritative split the operator asked for (the
//! "open vs closed Battle.net" model -- see docs/design/characters-and-servers.md):
//!
//!   - Your Homes (offline, self-custodial): local save profiles you fully own.
//!     Each save bundles a character (name + appearance + outfit) with a home +
//!     inventory + skills. This is the only section wired today (offline-first).
//!   - Open-Net Characters (self-custodial, server-listed): your local character
//!     used on a server that ALLOWS self-custody. Arrives with multiplayer.
//!   - Closed-Net Characters (server-held, anti-cheat): the server owns the
//!     character data so progress can't be forged. Arrives with multiplayer.
//!
//! Set a character as DEFAULT to skip this picker entirely: Play then enters the
//! world with that character directly (the checkbox the operator wanted so you
//! don't go through select every time). Persisted to AppConfig.default_character.
//!
//! "Customize Look" opens the appearance/character editor (the showroom) so you
//! can change how you look offline anytime -- no server changing-station needed
//! in offline mode. (On a closed-net server a fresh look can optionally require
//! visiting a changing station so outfits can't swap mid-expedition; that gate is
//! server-side and comes with multiplayer.)

use egui::{RichText, ScrollArea, Frame, Align, Layout};
use crate::gui::{GuiState, GuiPage};
use crate::gui::theme::Theme;
use crate::gui::widgets;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // Refresh the local save list once per opening (lib.rs clears
    // launcher_saves_loaded on the transition INTO this page).
    if !state.launcher_saves_loaded {
        state.launcher_saves = crate::persistence::list_saves(&crate::persistence::saves_dir());
        state.launcher_saves_loaded = true;
        // Pre-select the default character if one is set + still exists,
        // else the most-recent save, else the implicit new homestead.
        if !state.launcher_default_character.is_empty()
            && state.launcher_saves.iter().any(|(n, _)| n == &state.launcher_default_character)
        {
            state.launcher_selected = state.launcher_default_character.clone();
        } else if let Some((name, _)) = state.launcher_saves.first() {
            state.launcher_selected = name.clone();
        } else {
            state.launcher_selected = NEW_HOMESTEAD.to_string();
        }
    }

    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(16.0))
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                draw_header(ui, theme, state);
                ui.add_space(theme.spacing_md);
                draw_homes_section(ui, theme, state);
                ui.add_space(theme.spacing_md);
                draw_open_net_section(ui, theme);
                ui.add_space(theme.spacing_md);
                draw_closed_net_section(ui, theme);
                ui.add_space(theme.spacing_lg);
                draw_actions(ui, theme, state);
            });
        });
}

/// Display name for the implicit "no saves yet, enter a fresh homestead" entry.
const NEW_HOMESTEAD: &str = "My Homestead";

fn draw_header(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.add_space(theme.spacing_sm);
    // Back, centered + predictable (same convention as Server Settings).
    ui.vertical_centered(|ui| {
        if widgets::Button::secondary("< Back")
            .tooltip("Return without entering the world. Same as pressing Esc.")
            .show(ui, theme)
        {
            // Next open reloads the save list fresh.
            state.launcher_saves_loaded = false;
            if !state.pop_nav_back() {
                state.active_page = GuiPage::Humanity;
            }
        }
    });
    ui.add_space(theme.spacing_sm);
    ui.with_layout(Layout::top_down(Align::Center), |ui| {
        ui.label(RichText::new("PLAY").size(theme.font_size_small).color(theme.accent()).strong());
        ui.add_space(theme.spacing_xs);
        ui.label(
            RichText::new("Choose a character")
                .size(theme.font_size_title)
                .color(theme.text_primary())
                .strong(),
        );
        ui.label(
            RichText::new("Pick who you are, then Enter World. Set a default to skip this next time.")
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
    });
}

/// Section 1: local, self-custodial homes/characters. The only wired section.
fn draw_homes_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::section_header(ui, theme, "Your Homes");
    widgets::body_hint(
        ui, theme,
        "Offline, self-custodial. These saves live on this device and you fully own \
         them. Each is a character (name, look, outfit) with a home, inventory, and \
         skills. Select one, then Enter World.",
    );
    ui.add_space(theme.spacing_xs);

    // Build the row list: the real saves, or a single implicit new-homestead row
    // on a fresh install so you can always start playing.
    let mut rows: Vec<(String, u64)> = state.launcher_saves.clone();
    if rows.is_empty() {
        rows.push((NEW_HOMESTEAD.to_string(), 0));
    }

    let selected = state.launcher_selected.clone();
    let default_name = state.launcher_default_character.clone();
    let mut new_selected: Option<String> = None;
    let mut toggled_default: Option<String> = None;

    for (name, ts) in &rows {
        let is_selected = &selected == name;
        let is_default = !default_name.is_empty() && &default_name == name;
        widgets::card(ui, theme, |ui| {
            ui.horizontal(|ui| {
                // Select button doubles as the radio indicator.
                let sel_label = if is_selected { "[ Selected ]" } else { "Select" };
                if widgets::Button::secondary(sel_label).active(is_selected).show(ui, theme) {
                    new_selected = Some(name.clone());
                }
                ui.add_space(theme.spacing_sm);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(name)
                            .size(theme.font_size_heading)
                            .color(theme.text_primary())
                            .strong(),
                    );
                    let sub = if *ts == 0 {
                        "New homestead -- enter to begin".to_string()
                    } else {
                        format!("Last played {}", format_save_date(*ts))
                    };
                    ui.label(RichText::new(sub).size(theme.font_size_small).color(theme.text_muted()));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let dlabel = if is_default { "Default" } else { "Set default" };
                    if widgets::Button::secondary(dlabel).active(is_default).show(ui, theme) {
                        toggled_default = Some(name.clone());
                    }
                });
            });
        });
        ui.add_space(theme.spacing_xs);
    }

    if let Some(sel) = new_selected {
        state.launcher_selected = sel;
    }
    if let Some(name) = toggled_default {
        // Toggle: clicking the current default clears it (back to always-show).
        if state.launcher_default_character == name {
            state.launcher_default_character.clear();
        } else {
            state.launcher_default_character = name;
        }
        crate::config::AppConfig::from_gui_state(state).save();
    }

    if !state.launcher_default_character.is_empty() {
        ui.add_space(theme.spacing_xs);
        ui.label(
            RichText::new(format!(
                "Play will skip this screen and enter as \"{}\". Clear the default to bring the picker back.",
                state.launcher_default_character
            ))
            .size(theme.font_size_small)
            .color(theme.accent()),
        );
    }
}

/// Section 2: open-net (self-custodial on a server). Placeholder until multiplayer.
fn draw_open_net_section(ui: &mut egui::Ui, theme: &Theme) {
    widgets::section_header(ui, theme, "Open-Net Characters");
    widgets::card(ui, theme, |ui| {
        ui.label(
            RichText::new("Self-custodial, server-listed")
                .size(theme.font_size_small)
                .strong()
                .color(theme.text_secondary()),
        );
        ui.label(
            RichText::new(
                "Your own local character, played on a server that allows self-custody (like \
                 Open Battle.net). The server trusts your client. Arrives with multiplayer.",
            )
            .size(theme.font_size_small)
            .color(theme.text_muted()),
        );
    });
}

/// Section 3: closed-net (server-held, anti-cheat). Placeholder until multiplayer.
fn draw_closed_net_section(ui: &mut egui::Ui, theme: &Theme) {
    widgets::section_header(ui, theme, "Closed-Net Characters");
    widgets::card(ui, theme, |ui| {
        ui.label(
            RichText::new("Server-held, anti-cheat")
                .size(theme.font_size_small)
                .strong()
                .color(theme.text_secondary()),
        );
        ui.label(
            RichText::new(
                "The server owns the character data so progress and items can't be forged (like \
                 Closed Battle.net). You select from characters that server holds for you. \
                 Arrives with multiplayer.",
            )
            .size(theme.font_size_small)
            .color(theme.text_muted()),
        );
    });
}

/// Bottom action row: Customize Look + Enter World.
fn draw_actions(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.separator();
    ui.add_space(theme.spacing_sm);
    ui.with_layout(Layout::top_down(Align::Center), |ui| {
        ui.horizontal(|ui| {
            if widgets::Button::secondary("Customize Look")
                .tooltip("Open the character editor (appearance + outfit). Works offline anytime.")
                .show(ui, theme)
            {
                state.launcher_open_showroom = true;
            }
            ui.add_space(theme.spacing_sm);
            if widgets::Button::primary("Enter World")
                .tooltip("Load the selected character and drop into the 3D world.")
                .show(ui, theme)
            {
                // Ask lib.rs to apply the selected save after the world loads, then
                // enter first-person. An empty/implicit selection just enters the
                // active offline home (the existing Play path, unchanged).
                if !state.launcher_selected.is_empty() && !state.launcher_saves.is_empty() {
                    state.launcher_pending_load = Some(state.launcher_selected.clone());
                }
                state.launcher_enter = true;
            }
        });
    });
}

/// Format a Unix-seconds timestamp as `YYYY-MM-DD` (UTC), chrono-free
/// (Howard Hinnant civil-date math, same as server_settings::format_ban_date).
fn format_save_date(secs: u64) -> String {
    if secs == 0 {
        return "never".to_string();
    }
    let days = (secs / 86_400) as i64;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    format!("{year:04}-{m:02}-{d:02}")
}
