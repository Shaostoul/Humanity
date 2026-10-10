//! The Report dialog (step D of docs/design/blocking-and-safe-mode.md, section 10e, 2026-10-09)
//! and the buttons that open it: Report in a message's menu (chat.rs, beside Block), in the DM
//! conversation header, on a person's profile (the member list opens it), and in a member row's
//! right-click menu. Opening, sending and the relay's answers are src/engine/report.rs; the
//! signed report and the evidence picker's choice are src/net/report.rs.
//!
//! The dialog: the reasons from `data/safety/report_reasons.json` with the chosen one's help text,
//! the evidence (a DM report's list of that person's messages to tick; a post or group message
//! as it is; none for a person's profile), an optional note, and "Also block them", ticked by
//! default for a DM report. Nothing in it is a setting: it starts fresh each time it opens, except
//! the country of the help outside this server.
//!
//! Help outside this server (10e-ii, 2026-10-10): for the two danger reasons, a block under the
//! reason's help with the emergency number and the official place to report a child being
//! exploited online, for a country picked in the block (rules and words: src/net/outside_help.rs).
//! The pick is kept on this device for the next dialog and never sent.
//!
//! Takes `use super::*` like the page's other children.

use super::*;
use crate::net::outside_help;
use crate::net::report::{ReportContext, FILES_NOT_INCLUDED, MAX_EVIDENCE_ITEMS, MAX_NOTE_CHARS, REASONS_FILE};

/// What a Report button says when hovered.
pub(crate) const REPORT_TIP: &str =
    "Tell this server's admins and moderators about them, with the messages you choose. They are not told who reported them.";

/// The DM conversation header's Report button.
pub(super) fn draw_dm_header_report(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, peer: &str) {
    if peer.is_empty() || peer == state.profile_public_key {
        return;
    }
    if widgets::Button::ghost("Report").tooltip(REPORT_TIP).show(ui, theme) {
        crate::engine::report::open_for_dm(state, peer, None);
    }
}

/// A person's profile (opened from the member list, a message or a mention): Report. The
/// profile closes, so only one dialog is on screen.
pub(super) fn draw_profile_report(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, key: &str) {
    if key.is_empty() || key == state.profile_public_key {
        return;
    }
    ui.add_space(theme.spacing_sm);
    if widgets::Button::secondary("Report").full_width().tooltip(REPORT_TIP).show(ui, theme) {
        state.chat_user_modal_open = false;
        crate::engine::report::open_for_person(state, key);
    }
}

/// The Report entry of a member row's right-click menu.
pub(super) fn member_menu(ui: &mut egui::Ui, state: &mut GuiState, key: &str) {
    if key.is_empty() || key == state.profile_public_key {
        return;
    }
    if ui.button("Report").on_hover_text(REPORT_TIP).clicked() {
        crate::engine::report::open_for_person(state, key);
        ui.close_menu();
    }
}

/// At most `max` characters of `text` on one line, with "..." when cut.
fn one_line(text: &str, max: usize) -> String {
    let flat = text.replace(['\n', '\r'], " ");
    if flat.chars().count() <= max {
        return flat;
    }
    format!("{}...", flat.chars().take(max).collect::<String>())
}

/// The Report dialog, while one is open (the chat page draws it every frame).
pub(crate) fn draw_report_dialog(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let Some(mut d) = state.reports.dialog.take() else { return };
    let reasons = state.reports.reasons.clone();
    let outside = state.reports.outside_help.as_ref();
    let mut open = true;
    let mut send = false;
    let mut cancel = false;
    let mut picked = false;
    let name = if d.target_name.is_empty() { d.target.chars().take(8).collect() } else { d.target_name.clone() };

    // The two danger reasons add the help outside this server (10e-ii), which can make the dialog
    // taller than a small window, so the whole dialog scrolls (`dialog_scrolling`) and Send is
    // reached by scrolling. An inner ScrollArea was tried first and hid everything after it,
    // Send included, because a window does not grow to fit one (seen in the snapshot, 2026-10-10).
    widgets::dialog_scrolling(ctx, theme, "report_dialog", "Report", &mut open, |ui| {
        ui.set_min_width(440.0);
        ui.set_max_width(520.0);
        ui.vertical(|ui| {
            ui.label(RichText::new(format!("Report {name}")).size(theme.font_size_heading).color(theme.text_primary()).strong());
            widgets::body_hint(
                ui,
                theme,
                &format!("This goes to this server's admins and moderators, signed with your key. {name} is not told who reported them."),
            );
            ui.add_space(theme.spacing_sm);

            // ── Reason ──
            widgets::subsection_label(ui, theme, "What is happening?");
            if reasons.is_empty() {
                ui.label(
                    RichText::new(format!(
                        "The list of reasons could not be loaded (data/{REASONS_FILE}), so a report cannot be sent from this app right now."
                    ))
                    .size(theme.font_size_small)
                    .color(theme.warning()),
                );
            }
            for r in &reasons {
                ui.radio_value(&mut d.reason, r.id.clone(), RichText::new(&r.label).color(theme.text_primary()));
            }
            if let Some(help) = reasons.iter().find(|r| r.id == d.reason).map(|r| r.help.as_str()).filter(|h| !h.is_empty()) {
                ui.add_space(theme.spacing_xs);
                widgets::alert(ui, theme, widgets::AlertKind::Info, help);
            }
            // Under that help, for the two danger reasons only (10e-ii).
            if let Some(view) = outside.and_then(|help| outside_help::view(help, &d.reason, &d.country)) {
                ui.add_space(theme.spacing_xs);
                picked |= draw_outside_help(ui, theme, &view, &mut d.country);
            }
            ui.add_space(theme.spacing_sm);

            // ── Evidence ──
            match d.context {
                Some(ReportContext::Dm) => draw_dm_picker(ui, theme, &mut d.candidates),
                Some(ReportContext::Post) => {
                    widgets::subsection_label(ui, theme, "The post");
                    widgets::card(ui, theme, |ui| {
                        ui.label(RichText::new(one_line(&d.fixed_text, 300)).color(theme.text_secondary()));
                    });
                    widgets::body_hint(ui, theme, "The server already has this post, signed by its author, and finds it from its author and time.");
                }
                Some(ReportContext::Group) if d.fixed.is_none() => {
                    widgets::subsection_label(ui, theme, "The message");
                    widgets::body_hint(ui, theme, &format!("{FILES_NOT_INCLUDED} The report goes without it."));
                }
                Some(ReportContext::Group) => {
                    widgets::subsection_label(ui, theme, "The message");
                    widgets::card(ui, theme, |ui| {
                        ui.label(RichText::new(one_line(&d.fixed_text, 300)).color(theme.text_secondary()));
                    });
                    widgets::body_hint(
                        ui,
                        theme,
                        "Group messages are encrypted for the group, so the server's admins cannot check who wrote this. \
                         It is sent as text and marked Not proven.",
                    );
                }
                _ => widgets::body_hint(
                    ui,
                    theme,
                    "No messages are attached. To include messages, report them from your conversation with them.",
                ),
            }
            ui.add_space(theme.spacing_sm);

            // ── Note ──
            widgets::subsection_label(ui, theme, "Anything else they should know (optional)");
            ui.add(
                egui::TextEdit::multiline(&mut d.note)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY)
                    .char_limit(MAX_NOTE_CHARS)
                    .hint_text("For example, when it started."),
            );
            ui.add_space(theme.spacing_sm);

            // ── Also block them ──
            ui.checkbox(&mut d.also_block, RichText::new("Also block them").color(theme.text_primary()))
                .on_hover_text(super::blocking::BLOCK_TIP);
            widgets::body_hint(ui, theme, "You will not see anything from them on any server. They are not told.");
        });

        if !d.problem.is_empty() {
            ui.add_space(theme.spacing_xs);
            ui.label(RichText::new(&d.problem).size(theme.font_size_small).color(theme.warning()));
        }
        ui.add_space(theme.spacing_md);
        ui.horizontal(|ui| {
            let ready = !reasons.is_empty() && !d.reason.is_empty();
            if widgets::Button::danger("Send report")
                .disabled(!ready)
                .tooltip(if ready { REPORT_TIP } else { "Choose what is happening first." })
                .show(ui, theme)
            {
                send = true;
            }
            if widgets::Button::secondary("Cancel").show(ui, theme) {
                cancel = true;
            }
        });
    });

    if picked {
        remember_country(state, &d.country);
    }
    if send {
        state.reports.dialog = Some(d);
        crate::engine::report::send(state); // closes the dialog when it went, says why when not
    } else if open && !cancel {
        state.reports.dialog = Some(d);
    }
}

/// Keep the country picked in the outside help block for the next dialog (10e-ii): in AppConfig,
/// written through `settings_dirty` like every other local setting. It stays on this device and
/// is never part of a report.
fn remember_country(state: &mut GuiState, code: &str) {
    state.settings.outside_help_country = code.to_string();
    state.settings_dirty = true;
}

/// Help outside this server (10e-ii), under a danger reason's help: the country picker, then for
/// the chosen entry the emergency number (large), its other numbers, the child report line
/// (smaller for someone in danger), its note, and the date the numbers were checked, in the web
/// dialog's order so both apps read the same. True when another country was picked.
fn draw_outside_help(ui: &mut egui::Ui, theme: &Theme, v: &outside_help::HelpView, country: &mut String) -> bool {
    let before = country.clone();
    widgets::card(ui, theme, |ui| {
        ui.label(RichText::new(outside_help::TITLE).size(theme.font_size_body).color(theme.text_primary()).strong());
        ui.horizontal(|ui| {
            ui.label(RichText::new("Country:").color(theme.text_secondary()));
            egui::ComboBox::from_id_salt("report_outside_help_country").selected_text(v.name.as_str()).show_ui(ui, |ui| {
                for (code, name) in &v.choices {
                    ui.selectable_value(country, code.clone(), name.as_str());
                }
            });
        });
        ui.add_space(theme.spacing_xs);
        // The number is what someone in danger needs first, so it is the largest line; Another
        // country's sentence stands in its place at body size.
        let size = if v.emergency.is_some() { theme.font_size_heading } else { theme.font_size_body };
        ui.label(RichText::new(v.emergency_line()).size(size).color(theme.text_primary()).strong());
        for also in &v.also {
            ui.label(RichText::new(also.line()).color(theme.text_primary()));
        }
        if let Some(child) = &v.child {
            ui.add_space(theme.spacing_xs);
            let size = if v.child_small { theme.font_size_small } else { theme.font_size_body };
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(outside_help::CHILD_LEAD).size(size).color(theme.text_secondary()));
                let text = RichText::new(child.name.as_str()).size(size).color(theme.accent());
                // Opens in the browser. On hover: the INHOPE directory's own name when it stands
                // in for a country's body, otherwise the address, so the person sees where it goes.
                ui.add(egui::Hyperlink::from_label_and_url(text, child.url.as_str()).open_in_new_tab(true))
                    .on_hover_text(child.hover.as_deref().unwrap_or(child.url.as_str()));
            });
        }
        if !v.note.is_empty() {
            widgets::body_hint(ui, theme, &v.note);
        }
        if !v.date_line.is_empty() {
            widgets::body_hint(ui, theme, &v.date_line);
        }
    });
    *country != before
}

/// A DM report's evidence: that person's messages to us, newest first, to tick.
fn draw_dm_picker(ui: &mut egui::Ui, theme: &Theme, candidates: &mut [crate::net::report::DmCandidate]) {
    widgets::subsection_label(ui, theme, "Messages to include");
    widgets::body_hint(
        ui,
        theme,
        &format!(
            "Tick the messages that show what happened (up to {MAX_EVIDENCE_ITEMS}). Each carries their signature, so the \
             admins can check that they wrote it and sent it to you. The time on each is their device's clock."
        ),
    );
    widgets::body_hint(ui, theme, FILES_NOT_INCLUDED);
    if candidates.is_empty() {
        widgets::body_hint(
            ui,
            theme,
            "There are no messages from them here that can be included: only messages kept on this device since \
             reports began carry their signature.",
        );
        return;
    }
    let ticked = candidates.iter().filter(|c| c.ticked).count();
    egui::ScrollArea::vertical().id_salt("report_dm_picker").max_height(200.0).auto_shrink([false, true]).show(ui, |ui| {
        for c in candidates.iter_mut() {
            let text = one_line(&crate::engine::dm::dm_preview_text(&c.text), 120);
            let when = crate::gui::pages::game_admin::format_ban_date(c.ts as i64);
            let full = !c.ticked && ticked >= MAX_EVIDENCE_ITEMS;
            ui.horizontal(|ui| {
                ui.add_enabled(!full, egui::Checkbox::new(&mut c.ticked, RichText::new(text).color(theme.text_primary())));
                ui.label(RichText::new(when).size(theme.font_size_small).color(theme.text_muted()));
            });
        }
    });
    ui.label(RichText::new(format!("{ticked} ticked")).size(theme.font_size_small).color(theme.text_muted()));
}
