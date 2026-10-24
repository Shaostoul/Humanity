//! Server Settings > ADMIN > Alert channels (2026-10-22; docs/design/in-app-ops.md, "Alert
//! channels"): where this server sends a warning when something goes wrong. The first panel that
//! WRITES: list, add, change, turn on or off, remove, and send a test, each one an action of the
//! server's registry (`alerts.*`, src/relay/admin/alerts.rs). The website's admin page has the
//! same panel (web/pages/admin-console.js).
//!
//! An address or token is typed here once and never shown whole again: the server answers with
//! its host and last characters only.

use egui::RichText;
use serde_json::{json, Value};

use super::admin_api::{run_action, server_base};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::{AlertDraft, GuiState};

/// What each kind asks for in its address box.
fn address_hint(kind: &str) -> &'static str {
    match kind {
        "ntfy" => "https://ntfy.sh/a-long-random-name",
        "discord" => "https://discord.com/api/webhooks/...",
        "telegram" => "bot token, 123456:ABC-DEF...",
        _ => "https://hooks.example.com/...",
    }
}

/// A masked secret as words: "discord.com, ends ab12".
pub(crate) fn masked_words(m: &Value) -> String {
    if m["set"] != true {
        return "not set".into();
    }
    let host = m["host"].as_str().unwrap_or("");
    let ends = m["ends"].as_str().unwrap_or("");
    match (host.is_empty(), ends.is_empty()) {
        (false, false) => format!("{host}, ends {ends}"),
        (false, true) => host.to_string(),
        (true, false) => format!("ends {ends}"),
        (true, true) => "set".into(),
    }
}

/// The inputs `alerts.add` takes from a draft.
pub(crate) fn add_inputs(d: &AlertDraft) -> Value {
    let mut v = json!({ "kind": d.kind, "address": d.address.trim(), "enabled": true });
    if !d.label.trim().is_empty() {
        v["label"] = Value::String(d.label.trim().to_string());
    }
    if d.kind == "telegram" && !d.chat_id.trim().is_empty() {
        v["chat_id"] = Value::String(d.chat_id.trim().to_string());
    }
    v
}

/// The inputs `alerts.update` takes from an edit: only what was typed.
pub(crate) fn update_inputs(index: usize, d: &AlertDraft) -> Value {
    let mut v = json!({ "index": index });
    for (name, text) in [("label", &d.label), ("address", &d.address), ("chat_id", &d.chat_id)] {
        if !text.trim().is_empty() {
            v[name] = Value::String(text.trim().to_string());
        }
    }
    v
}

pub(crate) fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::subsection_label(ui, theme, "Alert channels");
    widgets::body_hint(
        ui,
        theme,
        "Where this server sends a warning when something goes wrong, such as the relay going down or \
         the disk filling up: a phone notification through ntfy, a Discord or Slack channel, a Telegram \
         chat, or any web address that takes a message. An address or token stays on the server; after \
         you save it, only its last characters are shown. There is no email channel: an email service's \
         webhook address works as a Webhook.",
    );
    ui.add_space(theme.spacing_xs);

    let connected = server_base(state).is_some();
    if connected && state.admin_console.alerts.is_none() && state.admin_console.alerts_status.1.is_empty() {
        state.admin_console.alerts_status = (true, "Loading...".into());
        run_action(state, "alerts.list", json!({}), None);
    }

    // What was clicked this frame, run after the drawing (the rows borrow the view).
    let mut to_run: Vec<(&'static str, Value)> = Vec::new();
    let view = state.admin_console.alerts.clone();
    if let Some(v) = &view {
        let channels = v["channels"].as_array().cloned().unwrap_or_default();
        if channels.is_empty() {
            widgets::body_hint(ui, theme, "No alert channels yet: this server's warnings go nowhere until you add one.");
        }
        for ch in &channels {
            let i = ch["index"].as_u64().unwrap_or(0) as usize;
            let kind = ch["kind"].as_str().unwrap_or("").to_string();
            ui.horizontal_wrapped(|ui| {
                let mut on = ch["enabled"].as_bool().unwrap_or(true);
                if ui.checkbox(&mut on, "").on_hover_text("On: alerts go to this channel. Off: kept, sent nothing.").changed() {
                    to_run.push(("alerts.set_enabled", json!({ "index": i, "enabled": on })));
                }
                ui.label(RichText::new(ch["label"].as_str().unwrap_or("")).size(theme.font_size_body).color(theme.text_primary()).strong());
                ui.label(RichText::new(&kind).size(theme.font_size_small).color(theme.text_secondary()));
                let mut where_words = masked_words(&ch["address"]);
                if kind == "telegram" {
                    where_words = format!("{where_words}; chat {}", masked_words(&ch["chat_id"]));
                }
                ui.label(RichText::new(where_words).size(theme.font_size_small).color(theme.text_muted()));
                if widgets::Button::ghost("Send test").tooltip("Send one real message to this channel now, to see that it arrives.").show(ui, theme) {
                    to_run.push(("alerts.test", json!({ "index": i })));
                }
                if widgets::Button::ghost("Change").tooltip("Rename it, or give it a new address or token.").show(ui, theme) {
                    state.admin_console.alert_edit = Some((i, AlertDraft { kind: kind.clone(), ..Default::default() }));
                }
                let key = format!("alerts.remove:{i}");
                if state.admin_console.confirm.as_deref() == Some(key.as_str()) {
                    if widgets::Button::danger("Really remove?").show(ui, theme) {
                        to_run.push(("alerts.remove", json!({ "index": i })));
                        state.admin_console.confirm = None;
                    }
                    if widgets::Button::secondary("Cancel").show(ui, theme) {
                        state.admin_console.confirm = None;
                    }
                } else if widgets::Button::ghost("Remove").tooltip("Forget this channel and its address. Asks once.").show(ui, theme) {
                    state.admin_console.confirm = Some(key);
                }
            });
            // The change form, under its row.
            let editing = state.admin_console.alert_edit.as_ref().is_some_and(|(e, _)| *e == i);
            if editing {
                let mut save = false;
                let mut cancel = false;
                if let Some((_, d)) = state.admin_console.alert_edit.as_mut() {
                    ui.horizontal_wrapped(|ui| {
                        ui.add_space(theme.spacing_lg);
                        ui.add(egui::TextEdit::singleline(&mut d.label).desired_width(120.0).hint_text("new name"));
                        ui.add(egui::TextEdit::singleline(&mut d.address).password(true).desired_width(220.0).hint_text("new address or token"));
                        if d.kind == "telegram" {
                            ui.add(egui::TextEdit::singleline(&mut d.chat_id).desired_width(110.0).hint_text("new chat number"));
                        }
                        save = widgets::Button::primary("Save").tooltip("Empty boxes keep what the channel has.").show(ui, theme);
                        cancel = widgets::Button::secondary("Cancel").show(ui, theme);
                    });
                }
                if save {
                    if let Some((idx, d)) = state.admin_console.alert_edit.clone() {
                        to_run.push(("alerts.update", update_inputs(idx, &d)));
                    }
                }
                if cancel {
                    state.admin_console.alert_edit = None;
                }
            }
        }
    }

    // Add one.
    ui.add_space(theme.spacing_xs);
    let mut add = false;
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Add:").size(theme.font_size_small).color(theme.text_secondary()));
        let d = &mut state.admin_console.alert_draft;
        egui::ComboBox::from_id_salt("alert_add_kind").selected_text(d.kind.clone()).width(90.0).show_ui(ui, |ui| {
            for k in crate::relay::admin::alerts::KINDS {
                ui.selectable_value(&mut d.kind, (*k).to_string(), *k);
            }
        });
        ui.add(egui::TextEdit::singleline(&mut d.label).desired_width(110.0).hint_text("your name for it"));
        ui.add(egui::TextEdit::singleline(&mut d.address).password(true).desired_width(240.0).hint_text(address_hint(&d.kind)));
        if d.kind == "telegram" {
            ui.add(egui::TextEdit::singleline(&mut d.chat_id).desired_width(110.0).hint_text("chat number"));
        }
        let ready = !d.address.trim().is_empty() && (d.kind != "telegram" || !d.chat_id.trim().is_empty());
        add = widgets::Button::primary("Add channel").disabled(!ready || !connected).tooltip("Save it on the server. Send it a test afterwards.").show(ui, theme);
    });
    if add {
        let inputs = add_inputs(&state.admin_console.alert_draft);
        to_run.push(("alerts.add", inputs));
        let kind = state.admin_console.alert_draft.kind.clone();
        state.admin_console.alert_draft = AlertDraft { kind, ..Default::default() };
    }

    for (action, inputs) in to_run {
        run_action(state, action, inputs, None);
    }

    let (ok, words) = state.admin_console.alerts_status.clone();
    if !words.is_empty() {
        ui.label(RichText::new(words).size(theme.font_size_small).color(if ok { theme.text_muted() } else { theme.danger() }));
    }
    if widgets::Button::secondary("Refresh").tooltip("Read the channels from the server again.").show(ui, theme) {
        run_action(state, "alerts.list", json!({}), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The add form sends the kind, the trimmed address, on, and a label or chat number only when
    /// typed; the change form sends only what was typed. Seen red 2026-10-22 with update_inputs
    /// sending every box: "an empty box keeps what the channel has".
    #[test]
    fn the_forms_send_only_what_was_typed() {
        let d = AlertDraft { kind: "telegram".into(), label: " ".into(), address: " 1:abc ".into(), chat_id: "42".into() };
        assert_eq!(add_inputs(&d), json!({ "kind": "telegram", "address": "1:abc", "enabled": true, "chat_id": "42" }));
        let e = AlertDraft { kind: "ntfy".into(), label: "phone".into(), ..Default::default() };
        assert_eq!(update_inputs(2, &e), json!({ "index": 2, "label": "phone" }), "an empty box keeps what the channel has");
    }

    #[test]
    fn a_masked_secret_reads_as_words() {
        assert_eq!(masked_words(&json!({ "set": true, "host": "discord.com", "ends": "ab12" })), "discord.com, ends ab12");
        assert_eq!(masked_words(&json!({ "set": false })), "not set");
        assert_eq!(masked_words(&json!({ "set": true, "host": null, "ends": "9z" })), "ends 9z");
    }
}
