//! Server Settings > ADMIN > Every admin action, and the admin log (2026-10-22;
//! docs/design/in-app-ops.md, the north star). Drawn entirely from the connected server's own
//! registry (`GET /api/admin/actions`, data/admin/actions.json): every action an admin or
//! moderator can take, grouped, with the role it needs, how dangerous it is and a form for its
//! inputs. An action that runs through the registry runs from here; one that is still a chat
//! command is sent as one; one with a panel of its own says where. Nothing here names an action:
//! a new entry in the server's file appears without a change to this page. The website's admin
//! page draws the same list (web/pages/admin-console.js).

use std::collections::HashMap;

use egui::RichText;
use serde_json::{json, Value};

use super::admin_api::{fetch_registry, run_action, server_base, time_ago};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;
use crate::relay::admin::ActionDef;

/// The inputs as typed, turned into what the action takes: numbers as numbers, yes-or-no as true
/// or false, empty boxes left out. The reason, when a number is not one.
pub(crate) fn inputs_from_form(def: &ActionDef, form: &HashMap<String, String>) -> Result<Value, String> {
    let mut out = serde_json::Map::new();
    for i in &def.inputs {
        let text = form.get(&i.name).map(|s| s.trim().to_string()).unwrap_or_default();
        if text.is_empty() {
            if i.kind == "bool" {
                out.insert(i.name.clone(), Value::Bool(false));
            }
            continue;
        }
        let v = match i.kind.as_str() {
            "number" => Value::from(text.parse::<i64>().map_err(|_| format!("{} must be a whole number.", i.label))?),
            "bool" => Value::Bool(text == "true"),
            _ => Value::String(text),
        };
        out.insert(i.name.clone(), v);
    }
    Ok(Value::Object(out))
}

/// The role an action needs, in words.
fn role_words(role: &str) -> &'static str {
    match role {
        "admin" => "admins",
        "mod" => "moderators",
        _ => "anyone",
    }
}

pub(crate) fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::subsection_label(ui, theme, "Every admin action");
    widgets::body_hint(
        ui,
        theme,
        "Everything an admin or moderator can do on this server, from the server's own list. The panels \
         above are built on the same actions, and an AI reads the same list (GET /api/admin/actions) and \
         runs the same actions, signed with its own key. Run sends it now; Send as chat command types it \
         in the chat you have open.",
    );
    ui.add_space(theme.spacing_xs);

    if server_base(state).is_some() && state.admin_console.registry.is_none() && state.admin_console.registry_status.is_empty() {
        fetch_registry(state);
    }
    if !state.admin_console.registry_status.is_empty() {
        ui.label(RichText::new(&state.admin_console.registry_status).size(theme.font_size_small).color(theme.text_muted()));
    }
    let Some(registry) = state.admin_console.registry.clone() else {
        if widgets::Button::secondary("Load the list").show(ui, theme) {
            fetch_registry(state);
        }
        return;
    };

    ui.horizontal(|ui| {
        ui.label(RichText::new("Find:").size(theme.font_size_small).color(theme.text_secondary()));
        ui.add(egui::TextEdit::singleline(&mut state.admin_console.filter).desired_width(220.0).hint_text("ban, channel, alert..."));
    });
    let filter = state.admin_console.filter.trim().to_lowercase();

    let mut to_run: Option<(String, Value, Option<String>)> = None;
    let mut to_chat: Option<String> = None;
    for cat in &registry.categories {
        let actions: Vec<&ActionDef> = registry
            .actions
            .iter()
            .filter(|a| a.category == cat.id)
            .filter(|a| filter.is_empty() || a.label.to_lowercase().contains(&filter) || a.description.to_lowercase().contains(&filter) || a.id.contains(&filter))
            .collect();
        if actions.is_empty() {
            continue;
        }
        egui::CollapsingHeader::new(RichText::new(format!("{} ({})", cat.label, actions.len())).size(theme.font_size_body).color(theme.text_primary()))
            .id_salt(format!("admin_cat_{}", cat.id))
            .default_open(!filter.is_empty())
            .show(ui, |ui| {
                widgets::body_hint(ui, theme, &cat.description);
                for a in actions {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(&a.label).size(theme.font_size_body).color(theme.text_primary()).strong());
                        ui.label(RichText::new(format!("for {}", role_words(&a.role))).size(theme.font_size_small).color(theme.text_muted()));
                        let (danger_words, danger_colour) = match a.danger.as_str() {
                            "destructive" => ("cannot be undone", theme.danger()),
                            "caution" => ("asks first", theme.warning()),
                            "read" => ("changes nothing", theme.text_muted()),
                            _ => ("", theme.text_muted()),
                        };
                        if !danger_words.is_empty() {
                            ui.label(RichText::new(danger_words).size(theme.font_size_small).color(danger_colour));
                        }
                        let open = state.admin_console.open_action.as_deref() == Some(a.id.as_str());
                        if widgets::Button::ghost(if open { "Close" } else { "Open" }).show(ui, theme) {
                            state.admin_console.open_action = if open { None } else { Some(a.id.clone()) };
                            state.admin_console.confirm_typed.clear();
                        }
                    });
                    ui.label(RichText::new(&a.description).size(theme.font_size_small).color(theme.text_secondary()));
                    if state.admin_console.open_action.as_deref() == Some(a.id.as_str()) {
                        draw_form(ui, theme, state, a, &mut to_run, &mut to_chat);
                    }
                    if let Some((ok, words)) = state.admin_console.results.get(&a.id) {
                        ui.label(RichText::new(words).size(theme.font_size_small).color(if *ok { theme.success() } else { theme.danger() }));
                    }
                    ui.add_space(theme.spacing_xs);
                }
            });
    }
    if let Some((action, inputs, confirm)) = to_run {
        run_action(state, &action, inputs, confirm);
    }
    if let Some(text) = to_chat {
        super::send_slash(state, &text);
    }

    ui.add_space(theme.spacing_md);
    draw_log(ui, theme, state);
}

/// One action's form: its inputs, its confirm word when it cannot be undone, and its button.
fn draw_form(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &mut GuiState,
    a: &ActionDef,
    to_run: &mut Option<(String, Value, Option<String>)>,
    to_chat: &mut Option<String>,
) {
    let form = state.admin_console.forms.entry(a.id.clone()).or_default();
    egui::Grid::new(format!("admin_form_{}", a.id)).num_columns(2).spacing([theme.spacing_md, theme.spacing_xs]).show(ui, |ui| {
        for i in &a.inputs {
            ui.label(RichText::new(format!("{}{}", i.label, if i.required { "" } else { " (optional)" })).size(theme.font_size_small).color(theme.text_secondary()));
            let value = form.entry(i.name.clone()).or_default();
            match i.kind.as_str() {
                "bool" => {
                    let mut on = value == "true";
                    if ui.checkbox(&mut on, "").changed() {
                        *value = on.to_string();
                    }
                }
                "choice" => {
                    egui::ComboBox::from_id_salt(format!("admin_choice_{}_{}", a.id, i.name))
                        .selected_text(if value.is_empty() { "choose".to_string() } else { value.clone() })
                        .show_ui(ui, |ui| {
                            for c in &i.choices {
                                ui.selectable_value(value, c.clone(), c);
                            }
                        });
                }
                kind => {
                    let mut edit = egui::TextEdit::singleline(value).desired_width(260.0).password(kind == "secret");
                    if let Some(h) = &i.hint {
                        edit = edit.hint_text(h.as_str());
                    }
                    ui.add(edit);
                }
            }
            ui.end_row();
        }
    });
    let word = a.confirm_word.clone().unwrap_or_default();
    if a.is_destructive() {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("This cannot be undone. Type {word} to go ahead:")).size(theme.font_size_small).color(theme.danger()));
            ui.add(egui::TextEdit::singleline(&mut state.admin_console.confirm_typed).desired_width(120.0));
        });
    }
    let confirmed = !a.is_destructive() || state.admin_console.confirm_typed.trim() == word;
    let form = state.admin_console.forms.get(&a.id).cloned().unwrap_or_default();
    ui.horizontal(|ui| {
        if a.runs_here() {
            if widgets::Button::primary("Run").disabled(!confirmed).tooltip("Send it to the server now, signed with your key.").show(ui, theme) {
                match inputs_from_form(a, &form) {
                    Ok(inputs) => *to_run = Some((a.id.clone(), inputs, a.is_destructive().then(|| word.clone()))),
                    Err(why) => {
                        state.admin_console.results.insert(a.id.clone(), (false, why));
                    }
                }
            }
        } else if a.chat.is_some() {
            if widgets::Button::primary("Send as chat command").disabled(!confirmed).tooltip("Type it in the chat channel you have open; the answer appears there.").show(ui, theme) {
                match inputs_from_form(a, &form) {
                    Ok(Value::Object(m)) => {
                        if let Some(text) = a.chat_text(&m) {
                            state.admin_console.results.insert(a.id.clone(), (true, format!("Sent {text} in #{}: the answer appears in Chat.", state.chat_active_channel)));
                            *to_chat = Some(text);
                        }
                    }
                    Ok(_) => {}
                    Err(why) => {
                        state.admin_console.results.insert(a.id.clone(), (false, why));
                    }
                }
            }
        } else if let Some(route) = &a.route {
            ui.label(RichText::new(format!("Runs at {route}: the panels for it are elsewhere on this page.")).size(theme.font_size_small).color(theme.text_muted()));
        }
    });
}

/// The admin log: what admins changed here, when, and who.
fn draw_log(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::subsection_label(ui, theme, "Admin log");
    widgets::body_hint(
        ui,
        theme,
        "Every change made through these actions and the panels above: when, which admin, what, and whether \
         it worked. It never holds a password, token or address, and never the member an action was about.",
    );
    if widgets::Button::secondary(if state.admin_console.log.is_some() { "Refresh" } else { "Show the log" }).show(ui, theme) {
        run_action(state, "log.recent", json!({}), None);
    }
    if !state.admin_console.log_status.is_empty() && state.admin_console.log.is_none() {
        ui.label(RichText::new(&state.admin_console.log_status).size(theme.font_size_small).color(theme.text_muted()));
    }
    if let Some(entries) = &state.admin_console.log {
        if entries.is_empty() {
            widgets::body_hint(ui, theme, "Nothing has been changed through the admin actions yet.");
        }
        egui::Grid::new("admin_log_grid").num_columns(4).spacing([theme.spacing_md, theme.spacing_xs]).striped(true).show(ui, |ui| {
            for e in entries.iter().take(50) {
                ui.label(RichText::new(time_ago(e["at"].as_i64().unwrap_or(0))).size(theme.font_size_small).color(theme.text_muted()));
                ui.label(RichText::new(e["who"].as_str().unwrap_or("")).size(theme.font_size_small).color(theme.text_secondary()));
                ui.label(RichText::new(e["label"].as_str().unwrap_or("")).size(theme.font_size_small).color(theme.text_primary()));
                let ok = e["ok"] != false;
                ui.label(RichText::new(e["summary"].as_str().unwrap_or("")).size(theme.font_size_small).color(if ok { theme.text_secondary() } else { theme.danger() }));
                ui.end_row();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A form becomes the inputs the action takes: numbers as numbers, an unticked box as false,
    /// empty boxes left out, and a word in a number box refused before anything is sent. Seen red
    /// 2026-10-22 with every value sent as text: "a number is sent as a number".
    #[test]
    fn a_form_becomes_the_inputs_the_action_takes() {
        let reorder = crate::relay::admin::find("channels.reorder").unwrap();
        let mut f = HashMap::new();
        f.insert("name".to_string(), " general ".to_string());
        f.insert("position".to_string(), "3".to_string());
        assert_eq!(inputs_from_form(reorder, &f).unwrap(), json!({ "name": "general", "position": 3 }), "a number is sent as a number");
        f.insert("position".to_string(), "three".to_string());
        assert!(inputs_from_form(reorder, &f).is_err());
        let chan = crate::relay::admin::find("federation.channel").unwrap();
        let mut g = HashMap::new();
        g.insert("channel".to_string(), "general".to_string());
        assert_eq!(inputs_from_form(chan, &g).unwrap(), json!({ "channel": "general", "federated": false }), "an unticked box is false");
        let checked = crate::relay::admin::check_inputs(chan, &inputs_from_form(chan, &g).unwrap());
        assert!(checked.is_ok(), "and the relay takes what the form makes: {:?}", checked.err());
    }
}
