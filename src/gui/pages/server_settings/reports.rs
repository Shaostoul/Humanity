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
        "media" => "Item on the Files page",
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
        "Reports members sent from a message, a conversation, a person's profile or an item on the Files page, with the evidence they chose. \
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
    let mut open_media = None;
    for e in &r.evidence {
        if let Some(id) = draw_evidence(ui, theme, state, r, e, &target_name) {
            open_media = Some(id);
        }
    }
    if let Some(id) = open_media {
        crate::gui::pages::media::open_item_by_number(state, id);
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
/// carries (the sender's own clock). A Media item (the relay's A5) shows what the server kept of
/// it (its title, kind, labels and who may see it) and an Open that shows the item on the Media
/// page: returns its number when Open was pressed.
fn draw_evidence(ui: &mut egui::Ui, theme: &Theme, state: &GuiState, r: &ListedReport, e: &ListedEvidence, target_name: &str) -> Option<i64> {
    let sender = if e.from == r.target || e.from.is_empty() { target_name.to_string() } else { name_for(state, &e.from, "") };
    let badge = report::badge(e, &sender);
    let mut open = None;
    widgets::card(ui, theme, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(
            RichText::new(badge)
                .size(theme.font_size_small)
                .color(if e.checked { theme.success() } else { theme.warning() })
                .strong(),
        );
        if let Some(m) = e.media.as_ref() {
            // Label ids by the names the Media page knows them by, when it has them.
            let label_name = |id: &str| state.media.list.labels.iter().find(|l| l.id == id).map(|l| l.label.clone()).unwrap_or_else(|| id.to_string());
            ui.label(RichText::new(media_words(m, label_name)).color(theme.text_primary()));
            if widgets::Button::secondary(OPEN_ITEM).tooltip("Show this item on the Files page, if it is still there.").show(ui, theme) {
                open = Some(m.id);
            }
            return;
        }
        let text = if e.text.is_empty() { "(no text kept)".to_string() } else { crate::engine::dm::dm_preview_text(&e.text) };
        ui.label(RichText::new(text).color(theme.text_primary()));
        if e.ts > 0 {
            let when = crate::gui::pages::game_admin::format_ban_date(report::as_millis(e.ts) as i64);
            let clock = if e.kind == "dm" { " (by the sender's own clock)" } else { "" };
            ui.label(RichText::new(format!("{when}{clock}")).size(theme.font_size_small).color(theme.text_muted()));
        }
    });
    open
}

/// The button on a reported Media item (the list's own Open tab is another thing).
pub(crate) const OPEN_ITEM: &str = "Open the item";

/// A reported item of the Files page in words: "Item 12 on the Files page: "Dusk", a video, labelled Flashing lights,
/// members only". Only its number when the server could not check it.
pub(crate) fn media_words(m: &report::ListedMedia, label_name: impl Fn(&str) -> String) -> String {
    let mut words = format!("Item {} on the Files page", m.id);
    if !m.title.trim().is_empty() {
        words.push_str(&format!(": \"{}\"", m.title.trim()));
    }
    if !m.kind.is_empty() {
        let kind = crate::gui::pages::media::kinds::one_name(&m.kind).to_lowercase();
        let article = if kind.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" };
        words.push_str(&format!(", {article} {kind}"));
    }
    if !m.labels.is_empty() {
        let names: Vec<String> = m.labels.iter().map(|l| label_name(l)).collect();
        words.push_str(&format!(", labelled {}", names.join(", ")));
    }
    match m.visibility.as_str() {
        "members" => words.push_str(", members only"),
        "public" => words.push_str(", public"),
        _ => {}
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::screen_surface::find_text_in_shapes;

    /// A REPORTED MEDIA ITEM SHOWS WHAT THE SERVER KEPT OF IT, WITH AN OPEN THAT SHOWS IT (the
    /// relay's A5): its number, title, kind, labels and who may see it, the checked badge, and Open,
    /// which takes the admin to the Media page with that item's own page open, asked for by its
    /// number (the list is not filtered down to it: media/item_tests.rs
    /// `the_reports_open_opens_the_items_own_page` follows the answer onto the page).
    ///
    /// Seen red 2026-10-11 with media evidence drawn as text like any other: "the item is
    /// described" (the card said "(no text kept)"). Seen red 2026-10-12 against the old Open, which
    /// filtered the list: "Open opens item 12's own page" (left: None, right: Some(12)).
    #[test]
    fn a_reported_media_item_is_described_and_opens() {
        crate::config::keep_saves_off_disk();
        let ctx = egui::Context::default();
        crate::gui::fonts::install_font_fallbacks(&ctx);
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        let mut state = GuiState::default();
        state.server_url = "http://127.0.0.1:9".into();
        state.profile_public_key = "me".into();
        state.reports.reasons_error = Some("not needed here".into());
        state.reports.list = report::parse_reports(&serde_json::json!({ "type": "reports", "state": "open", "items": [{
            "id": 5, "target": "k2", "target_name": "Ann", "context": "media", "reason": "spam", "reason_label": "Spam", "created_at": 1,
            "evidence": [{"kind": "media", "id": 12, "uploader": "k2", "title": "Dusk", "media_kind": "video", "labels": ["flashing"], "visibility": "members", "checked": true}]
        }] }));
        let run = |state: &mut GuiState, events: Vec<egui::Event>| {
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1100.0, 1400.0))), events, ..Default::default() };
            ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| draw(ui, &theme, state, true));
            })
        };
        let out = run(&mut state, Vec::new());
        let out = if find_text_in_shapes(&out.shapes, "Item 12 on the Files page").is_some() { out } else { run(&mut state, Vec::new()) };
        assert!(find_text_in_shapes(&out.shapes, "Item 12 on the Files page: \"Dusk\", a video, labelled flashing, members only").is_some(), "the item is described");
        assert!(find_text_in_shapes(&out.shapes, "Found on this server: put on its Files page by Ann").is_some(), "with the checked badge");
        assert!(find_text_in_shapes(&out.shapes, "Item on the Files page").is_some(), "and the context says Item on the Files page");
        let open = find_text_in_shapes(&out.shapes, OPEN_ITEM).expect("an Open the item").rect.center();
        let m = egui::Modifiers::default();
        run(&mut state, vec![egui::Event::PointerMoved(open)]);
        run(&mut state, vec![egui::Event::PointerButton { pos: open, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
        run(&mut state, vec![egui::Event::PointerButton { pos: open, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
        assert_eq!(state.active_page, crate::gui::GuiPage::Media, "Open goes to the Files page");
        assert_eq!(state.media.item.as_ref().map(|p| p.id), Some(12), "Open opens item 12's own page");
        assert!(
            state.media.jobs.iter().any(|(j, _)| *j == crate::gui::pages::media::Job::Item { id: 12 }),
            "and asks for it by its number, since the list does not hold it"
        );
        assert_eq!(state.media.filters.id, None, "the list is not filtered down to it");
    }
}
