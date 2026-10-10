//! Server Settings > Moderator > Reports (step D of docs/design/blocking-and-safe-mode.md,
//! section 10e, 2026-10-09): the reports members sent this server, for its admins and
//! moderators, in two lists (Open and Decided). Each report shows its reason, where it came
//! from, who was reported, the note, and every piece of evidence with what the server could
//! prove about it: "Signature checked: sent by <name> to the reporter" on a direct message whose
//! signature the relay checked against the reported person's key, and "Not proven" on the rest.
//! An open report has the decision buttons, carried out by the relay through its moderation path
//! (so a moderator still cannot act on an admin, and Ban stays admin-only).
//!
//! The reporter is named only to admins; the relay sends moderators "a member" (10e), and the
//! person reported is never told. This replaced the old "View reports" button, which typed
//! `/reports` into the chat; the slash commands still work.
//!
//! The page asks the server for the list (`reports_list`) the first time it is drawn on a
//! socket, on switching lists, on Refresh and after each decision. Nothing here is a setting.

use egui::RichText;

use crate::gui::theme::Theme;
use crate::gui::{widgets, GuiState};
use crate::net::report::{self, Decision, ListedEvidence, ListedReport};

/// Where a report came from, in words.
fn context_words(context: &str) -> &str {
    match context {
        "dm" => "Direct messages",
        "post" => "Public post",
        "group" => "Group message",
        "profile" => "Profile",
        other => other,
    }
}

/// A key's name as this page can know it: the name the server sent, else the member list's,
/// else the start of the key.
fn name_for(state: &GuiState, key: &str, sent: &str) -> String {
    if !sent.trim().is_empty() {
        return sent.to_string();
    }
    if key.is_empty() {
        return "someone".to_string();
    }
    crate::engine::dm::dm_display_name(state, key)
}

/// The section, inside the Moderator band. `is_admin` decides only which buttons are shown (Ban
/// is admin-only on the relay); the relay decides what each viewer is sent.
pub(crate) fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, is_admin: bool) {
    widgets::subsection_label(ui, theme, "Reports");
    widgets::body_hint(
        ui,
        theme,
        "Reports members sent from a message, a conversation or a person's profile, with the evidence they chose. \
         The person reported is never told who reported them, and moderators see the reporter only as a member.",
    );
    ui.add_space(theme.spacing_xs);
    widgets::alert(ui, theme, widgets::AlertKind::Info, report::CHECKED_DOES_NOT_PROVE);
    ui.add_space(theme.spacing_sm);

    if state.reports.reasons.is_empty() && state.reports.reasons_error.is_none() {
        crate::engine::report::ensure_reasons(state); // for the reasons' labels
    }
    if !state.reports.requested {
        crate::engine::report::request_list(state);
    }

    ui.horizontal(|ui| {
        for (label, decided) in [("Open", false), ("Decided", true)] {
            if widgets::Button::secondary(label).active(state.reports.show_decided == decided).show(ui, theme)
                && state.reports.show_decided != decided
            {
                state.reports.show_decided = decided;
                crate::engine::report::request_list(state);
            }
        }
        ui.add_space(theme.spacing_sm);
        if widgets::Button::secondary("Refresh").tooltip("Ask the server for the list again.").show(ui, theme) {
            crate::engine::report::request_list(state);
        }
        if !state.reports.status.is_empty() {
            ui.label(RichText::new(&state.reports.status).size(theme.font_size_small).color(theme.text_muted()));
        }
    });
    ui.add_space(theme.spacing_sm);

    // The list the server last sent is shown only under the tab it answers.
    let list: Vec<ListedReport> = if state.reports.list_is_decided == state.reports.show_decided {
        state.reports.list.clone()
    } else {
        Vec::new()
    };
    if list.is_empty() {
        widgets::body_hint(ui, theme, if state.reports.show_decided { "No decided reports." } else { "No open reports." });
        return;
    }
    let mut decided: Option<(serde_json::Value, Decision, String)> = None;
    for r in &list {
        widgets::card(ui, theme, |ui| {
            ui.set_min_width(ui.available_width());
            if let Some(d) = draw_report(ui, theme, state, r, is_admin) {
                decided = Some(d);
            }
        });
        ui.add_space(theme.spacing_sm);
    }
    if let Some((id, decision, note)) = decided {
        let key = match &id {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        state.reports.decide_notes.remove(&key);
        crate::engine::report::decide(state, &id, decision, &note);
    }
}

/// One report. Returns a decision when one of its buttons was pressed.
fn draw_report(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &mut GuiState,
    r: &ListedReport,
    is_admin: bool,
) -> Option<(serde_json::Value, Decision, String)> {
    // The relay's label, else this app's copy of the reasons, else the id itself.
    let reason = if !r.reason_label.is_empty() {
        r.reason_label.clone()
    } else {
        state.reports.reasons.iter().find(|x| x.id == r.reason).map(|x| x.label.clone()).unwrap_or_else(|| r.reason.clone())
    };
    let when = crate::gui::pages::game_admin::format_ban_date(report::as_millis(r.created_at) as i64);
    ui.label(RichText::new(&reason).size(theme.font_size_body).color(theme.text_primary()).strong());
    ui.label(
        RichText::new(format!("{} · report {} · {when}", context_words(&r.context), r.id_text()))
            .size(theme.font_size_small)
            .color(theme.text_muted()),
    );
    let target_name = name_for(state, &r.target, &r.target_name);
    let short: String = r.target.chars().take(16).collect();
    ui.label(RichText::new(format!("Reported: {target_name} ({short})")).color(theme.text_secondary()));
    // The relay names the reporter: their name or "an erased account" to an admin, always "a
    // member" to a moderator.
    let reporter = if !r.reporter_name.is_empty() {
        r.reporter_name.clone()
    } else if !r.reporter.is_empty() {
        name_for(state, &r.reporter, "")
    } else {
        "a member".to_string()
    };
    ui.label(RichText::new(format!("Reported by: {reporter}")).color(theme.text_secondary()));
    if !r.note.trim().is_empty() {
        ui.label(RichText::new(format!("Their note: {}", r.note)).color(theme.text_secondary()));
    }

    ui.add_space(theme.spacing_xs);
    if r.evidence.is_empty() {
        widgets::body_hint(ui, theme, "No messages attached.");
    }
    for e in &r.evidence {
        draw_evidence(ui, theme, state, r, e, &target_name);
    }

    if r.is_decided() {
        ui.add_space(theme.spacing_xs);
        let mut line = format!("Decided: {}", Decision::label_of(&r.decision));
        if !r.decided_by.is_empty() || !r.decided_by_name.is_empty() {
            line.push_str(&format!(" by {}", name_for(state, &r.decided_by, &r.decided_by_name)));
        }
        if r.decided_at > 0 {
            line.push_str(&format!(" on {}", crate::gui::pages::game_admin::format_ban_date(report::as_millis(r.decided_at) as i64)));
        }
        ui.label(RichText::new(line).color(theme.success()));
        if !r.decision_note.trim().is_empty() {
            ui.label(RichText::new(format!("Note: {}", r.decision_note)).size(theme.font_size_small).color(theme.text_muted()));
        }
        return None;
    }

    ui.add_space(theme.spacing_sm);
    let id_key = r.id_text();
    let note = state.reports.decide_notes.entry(id_key.clone()).or_default();
    ui.add(
        egui::TextEdit::singleline(note)
            .desired_width(f32::INFINITY)
            .char_limit(report::MAX_NOTE_CHARS)
            .id(egui::Id::new(("report_decide_note", id_key.as_str())))
            .hint_text("A note on your decision (optional, kept with the report)"),
    );
    let note = note.clone();
    let mut chosen = None;
    ui.horizontal_wrapped(|ui| {
        for decision in Decision::ALL {
            if (decision == Decision::Ban && !is_admin) || (decision == Decision::DeletePost && !r.has_post()) {
                continue;
            }
            let button = match decision {
                Decision::Dismiss | Decision::Warn | Decision::Mute => widgets::Button::secondary(decision.label()),
                Decision::Kick | Decision::Ban | Decision::DeletePost => widgets::Button::danger(decision.label()),
            };
            if button.tooltip(decision.tip()).show(ui, theme) {
                chosen = Some((r.id.clone(), decision, note.clone()));
            }
        }
    });
    chosen
}

/// One piece of evidence: its badge, then its text, and for a direct message the time it
/// carries (the sender's own clock).
fn draw_evidence(ui: &mut egui::Ui, theme: &Theme, state: &GuiState, r: &ListedReport, e: &ListedEvidence, target_name: &str) {
    let sender = if e.from == r.target || e.from.is_empty() { target_name.to_string() } else { name_for(state, &e.from, "") };
    let badge = report::badge(e, &sender);
    widgets::card(ui, theme, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(
            RichText::new(badge)
                .size(theme.font_size_small)
                .color(if e.checked { theme.success() } else { theme.warning() })
                .strong(),
        );
        let text = if e.text.is_empty() { "(no text kept)".to_string() } else { crate::engine::dm::dm_preview_text(&e.text) };
        ui.label(RichText::new(text).color(theme.text_primary()));
        if e.ts > 0 {
            let when = crate::gui::pages::game_admin::format_ban_date(report::as_millis(e.ts) as i64);
            let clock = if e.kind == "dm" { " (by the sender's own clock)" } else { "" };
            ui.label(RichText::new(format!("{when}{clock}")).size(theme.font_size_small).color(theme.text_muted()));
        }
    });
}
