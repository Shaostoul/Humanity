//! Settings > Safety > Protected setup (step G of docs/design/blocking-and-safe-mode.md, section
//! 10h, 2026-10-10), and the small PIN prompt every locked action opens.
//!
//! OFF: the button "Turn on the protected setup" and, under it, the finding's sentence 1. Turning
//! it on is three steps drawn in place: Read (the preset's sentences, then Continue), Choose a PIN
//! (twice), Review (everyone who can already reach this device, each with Remove; "Keep the rest"
//! applies it). ON: the routes line, public rooms (Show needs the PIN, Hide never does), pictures
//! from people who are not friends (Shown after a click needs the PIN, Not shown never does),
//! Change the PIN, Turn off, and "Forgot the PIN?". The always-visible line heads the page
//! (`draw_status_line`) and the chat's rail (chat/protected.rs).
//!
//! THE PIN PROMPT asks, in turn, for the PIN, for this identity's recovery phrase after "Forgot
//! the PIN?", and for a new PIN twice; after a forgotten PIN it asks for the new one before the
//! waiting action runs (as the web chat does).
//!
//! Every word comes from the preset (data/gui/safety_presets.json: its lines, sentences and
//! `labels`), through engine/protected.rs `ensure_preset`; this file types none of its own. The
//! words test (net/protected_tests.rs) also reads this file's string literals and holds them to
//! the preset's `avoid_words`.
//!
//! Persistence: the setup's state is `GuiState::protected.setup`, saved in config.json through
//! `settings_dirty` like the other safety settings (config.rs `protected_setup`); the PINs and
//! phrases typed here are cleared as soon as they are checked or made into a verifier.

use egui::RichText;

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;
use crate::net::protected::{Preset, PromptMode, ProtectedAction, ReviewItem, SetupStep};

/// THE ALWAYS-VISIBLE LINE at the top of Settings > Safety while the setup is on (10h): the
/// preset's `status_line`, the same words the chat's rail shows. Nothing while it is off.
pub(crate) fn draw_status_line(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    if !crate::engine::protected::is_on(state) || !crate::engine::protected::ensure_preset(state) {
        return;
    }
    let Some(line) = state.protected.preset.as_ref().map(|p| p.status_line.clone()) else { return };
    widgets::card(ui, theme, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new(line).size(theme.font_size_body).color(theme.text_primary()).strong());
    });
}

/// The Requests list's Accept: plain "Accept" (the list's own word, safety.rs), or the preset's
/// "Accept (needs the PIN)" while the setup is on (10h).
pub(crate) fn accept_label(state: &mut GuiState) -> String {
    if crate::engine::protected::is_on(state) && crate::engine::protected::ensure_preset(state) {
        if let Some(p) = state.protected.preset.as_ref() {
            return p.accept_needs_pin.clone();
        }
    }
    "Accept".to_string()
}

/// Two masked PIN fields, hinted with the preset's "PIN" and "The same PIN again".
fn pin_fields(ui: &mut egui::Ui, state: &mut GuiState, preset: &Preset) {
    ui.add(egui::TextEdit::singleline(&mut state.protected.pin_first).password(true).hint_text(&preset.labels.pin).desired_width(200.0));
    ui.add(egui::TextEdit::singleline(&mut state.protected.pin_second).password(true).hint_text(&preset.labels.pin_again).desired_width(200.0));
}

/// The section, drawn last on Settings > Safety.
pub(crate) fn draw_protected_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, accent: egui::Color32) {
    if !crate::engine::protected::ensure_preset(state) {
        // Without the preset there are no words to show it with; the reason is logged.
        if let Some(e) = state.protected.preset_error.clone() {
            widgets::body_hint(ui, theme, &e);
        }
        return;
    }
    let Some(preset) = state.protected.preset.clone() else { return };
    widgets::subsection_header(ui, theme, accent, &preset.name, "");
    widgets::card(ui, theme, |ui| {
        ui.set_min_width(ui.available_width());
        if state.protected.setup.on {
            draw_on(ui, theme, state, &preset);
        } else {
            match state.protected.step {
                None => {
                    if widgets::Button::primary(&preset.button).show(ui, theme) {
                        crate::engine::protected::begin(state);
                    }
                    ui.add_space(theme.spacing_xs);
                    widgets::body_hint(ui, theme, &preset.summary);
                }
                Some(SetupStep::Read) => draw_read(ui, theme, state, &preset),
                Some(SetupStep::ChoosePin) => draw_choose_pin(ui, theme, state, &preset),
                Some(SetupStep::Review) => draw_review(ui, theme, state, &preset),
            }
        }
        if !state.protected.line.is_empty() {
            ui.add_space(theme.spacing_xs);
            ui.label(RichText::new(&state.protected.line).size(theme.font_size_small).color(theme.warning()));
        }
    });
}

/// Step 1, Read: the preset's sentences in order, then Continue (10h).
fn draw_read(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, preset: &Preset) {
    for sentence in &preset.sentences {
        ui.label(RichText::new(sentence).size(theme.font_size_body).color(theme.text_primary()));
        ui.add_space(theme.spacing_xs);
    }
    ui.horizontal_wrapped(|ui| {
        if widgets::Button::primary(&preset.labels.continue_).show(ui, theme) {
            crate::engine::protected::read_done(state);
        }
        if widgets::Button::secondary(&preset.labels.cancel).show(ui, theme) {
            crate::engine::protected::cancel(state);
        }
    });
}

/// Step 2, Choose a PIN: the preset's rule, and the PIN twice.
fn draw_choose_pin(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, preset: &Preset) {
    ui.label(RichText::new(preset.pin_rule()).size(theme.font_size_body).color(theme.text_primary()));
    pin_fields(ui, state, preset);
    ui.horizontal_wrapped(|ui| {
        if widgets::Button::primary(&preset.labels.continue_).show(ui, theme) {
            crate::engine::protected::pin_chosen(state);
        }
        if widgets::Button::secondary(&preset.labels.cancel).show(ui, theme) {
            crate::engine::protected::cancel(state);
        }
    });
}

/// Step 3, Review: the preset's intro, then friends, groups and voice rooms, each with Remove
/// (Unfollow or Leave, never a PIN), and "Keep the rest", which applies the setup (step 4).
fn draw_review(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, preset: &Preset) {
    crate::gui::pages::chat::drain_p2p_loaders(state); // the group list asked for in step 2
    widgets::body_hint(ui, theme, &preset.review_intro);
    let items = crate::engine::protected::review_items(state);
    let mut remove: Option<ReviewItem> = None;
    let kinds: [(&str, fn(&ReviewItem) -> bool); 3] = [
        (&preset.labels.friends, |i| matches!(i, ReviewItem::Friend { .. })),
        (&preset.labels.groups, |i| matches!(i, ReviewItem::Group { .. })),
        (&preset.labels.rooms, |i| matches!(i, ReviewItem::Room { .. })),
    ];
    for (title, is_kind) in kinds {
        ui.add_space(theme.spacing_sm);
        widgets::subsection_label(ui, theme, title);
        let rows: Vec<&ReviewItem> = items.iter().filter(|i| is_kind(i)).collect();
        if rows.is_empty() {
            widgets::body_hint(ui, theme, &preset.labels.none);
        }
        for item in rows {
            ui.horizontal(|ui| {
                ui.label(RichText::new(item.name()).size(theme.font_size_body).color(theme.text_primary()));
                if widgets::Button::secondary(&preset.labels.remove).show(ui, theme) {
                    remove = Some(item.clone());
                }
            });
        }
    }
    if let Some(item) = remove {
        crate::engine::protected::review_remove(state, &item);
    }
    ui.add_space(theme.spacing_sm);
    ui.horizontal_wrapped(|ui| {
        if widgets::Button::primary(&preset.review_keep).show(ui, theme) {
            crate::engine::protected::apply(state);
        }
        if widgets::Button::secondary(&preset.labels.cancel).show(ui, theme) {
            crate::engine::protected::cancel(state);
        }
    });
}

/// While it is on: the routes line; public rooms and pictures, where the way up needs the PIN
/// and the way back never does; Change the PIN, Turn off (both need the PIN) and "Forgot the PIN?"
/// (which asks for the recovery phrase in the prompt).
fn draw_on(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, preset: &Preset) {
    ui.label(RichText::new(&preset.routes_line).size(theme.font_size_body).color(theme.text_primary()));
    ui.add_space(theme.spacing_sm);

    let rooms_hidden = state.protected.setup.public_rooms_hidden;
    if rooms_hidden {
        widgets::body_hint(ui, theme, &preset.public_rooms_hidden_line);
    }
    widgets::body_hint(ui, theme, &preset.public_rooms_explain);
    let rooms_button = if rooms_hidden { &preset.labels.show_rooms } else { &preset.labels.hide_rooms };
    if widgets::Button::secondary(rooms_button).show(ui, theme) {
        if rooms_hidden {
            crate::engine::protected::perform(state, ProtectedAction::ShowPublicRooms);
        } else {
            state.protected.setup.public_rooms_hidden = true;
            state.settings_dirty = true;
        }
    }
    ui.add_space(theme.spacing_sm);

    widgets::subsection_label(ui, theme, &preset.labels.pictures);
    let pictures_hidden = state.protected.setup.pictures_hidden;
    ui.horizontal_wrapped(|ui| {
        if widgets::Button::secondary(&preset.labels.pictures_never).active(pictures_hidden).show(ui, theme) && !pictures_hidden {
            state.protected.setup.pictures_hidden = true;
            state.settings_dirty = true;
        }
        if widgets::Button::secondary(&preset.labels.pictures_click).active(!pictures_hidden).show(ui, theme) && pictures_hidden {
            crate::engine::protected::perform(state, ProtectedAction::ShowPictures);
        }
    });
    if pictures_hidden {
        widgets::body_hint(ui, theme, &preset.picture_hidden_line);
    }
    ui.add_space(theme.spacing_sm);

    ui.horizontal_wrapped(|ui| {
        if widgets::Button::secondary(&preset.labels.change_pin).show(ui, theme) {
            crate::engine::protected::perform(state, ProtectedAction::ChangePin);
        }
        if widgets::Button::danger(&preset.labels.turn_off).show(ui, theme) {
            crate::engine::protected::perform(state, ProtectedAction::TurnOff);
        }
        if widgets::Button::ghost(&preset.labels.forgot).show(ui, theme) {
            crate::engine::protected::open_forgot(state);
        }
    });
    widgets::body_hint(ui, theme, &preset.forgot_pin_explain);
}

/// THE PIN PROMPT, drawn over every page while it is open (lib.rs, beside the passphrase prompt):
/// in turn the PIN for the action waiting (with "Forgot the PIN?"), this identity's recovery
/// phrase, and a new PIN twice. The preset's name heads it; its labels are its words.
pub(crate) fn draw_pin_prompt(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let Some(mode) = state.protected.prompt.as_ref().map(|p| p.mode) else { return };
    if !crate::engine::protected::ensure_preset(state) {
        crate::engine::protected::cancel_prompt(state);
        return;
    }
    let Some(preset) = state.protected.preset.clone() else { return };
    let (mut go, mut cancel, mut forgot) = (false, false, false);
    egui::Window::new(&preset.name)
        .id(egui::Id::new("protected_pin_prompt"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            match mode {
                PromptMode::Pin => {
                    let field = ui.add(egui::TextEdit::singleline(&mut state.protected.prompt_input).password(true).hint_text(&preset.labels.pin).desired_width(220.0));
                    if !field.has_focus() && state.protected.prompt_line.is_empty() {
                        field.request_focus();
                    }
                    go = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                }
                PromptMode::Forgot => {
                    widgets::body_hint(ui, theme, &preset.forgot_pin_explain);
                    ui.add(
                        egui::TextEdit::multiline(&mut state.protected.forgot_phrase)
                            .password(true)
                            .hint_text(&preset.labels.phrase)
                            .desired_rows(2)
                            .desired_width(360.0),
                    );
                }
                PromptMode::NewPin => {
                    ui.label(RichText::new(preset.pin_rule()).size(theme.font_size_body).color(theme.text_primary()));
                    pin_fields(ui, state, &preset);
                }
            }
            if !state.protected.prompt_line.is_empty() {
                ui.label(RichText::new(&state.protected.prompt_line).size(theme.font_size_small).color(theme.warning()));
            }
            ui.add_space(theme.spacing_xs);
            ui.horizontal_wrapped(|ui| {
                go |= widgets::Button::primary(&preset.labels.continue_).show(ui, theme);
                cancel = widgets::Button::secondary(&preset.labels.cancel).show(ui, theme);
                if mode == PromptMode::Pin {
                    forgot = widgets::Button::ghost(&preset.labels.forgot).show(ui, theme);
                }
            });
        });
    if cancel {
        crate::engine::protected::cancel_prompt(state);
    } else if forgot {
        crate::engine::protected::open_forgot(state);
    } else if go {
        match mode {
            PromptMode::Pin => crate::engine::protected::submit_prompt(state, crate::engine::protected::now_secs()),
            PromptMode::Forgot => {
                crate::engine::protected::submit_phrase(state);
            }
            PromptMode::NewPin => {
                crate::engine::protected::submit_new_pin(state);
            }
        }
    }
}
