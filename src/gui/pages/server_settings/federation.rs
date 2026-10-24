//! Server Settings > ADMIN > Federation (rebuilt 2026-10-22 on the server's action registry,
//! `federation.*`, src/relay/admin/federation.rs; first built v0.722 on chat commands): the peer
//! servers this one links with, which channels travel between them, how much each is trusted,
//! whether a link is open now and when each was last heard from; add, trust, pause, resume,
//! re-check and remove a peer, and federate a channel. The website's admin page has the same
//! panel (web/pages/admin-console.js).
//!
//! Every action answers with the whole view, so the panel always draws what the server holds
//! after the change, never a guess.

use egui::RichText;
use serde_json::{json, Value};

use super::admin_api::{run_action, server_base, span_words, time_ago};
use super::send_server_settings_update;
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

/// A trust tier in words.
fn tier_words(t: i64) -> &'static str {
    match t {
        3 => "3, verified and Accord",
        2 => "2, verified",
        1 => "1, Accord but unverified",
        _ => "0, untrusted",
    }
}

/// How a peer is doing, in words: paused, linked for how long, or its last status and when it
/// was last heard from.
pub(crate) fn peer_state_words(p: &Value) -> String {
    // An admin's pause (the peer left out of federation), never the link's automatic hold-back.
    if p["paused"] == true {
        return "paused by an admin".into();
    }
    // `last_seen` moves on every status change and, while a link is up, on every message from the
    // peer: "last heard" while linked, "since" for the status it has otherwise.
    let ago = p["last_seen"].as_i64().map(time_ago);
    if p["linked"] == true {
        let since = p["linked_secs"].as_u64().map(|s| format!(" for {}", span_words(s))).unwrap_or_default();
        let heard = ago.map(|a| format!(", last heard {a}")).unwrap_or_default();
        return format!("linked{since}{heard}");
    }
    let status = p["status"].as_str().unwrap_or("unknown");
    let since = ago.map(|a| format!(" since {a}")).unwrap_or_default();
    let why = if p["trust_tier"].as_i64().unwrap_or(0) < 2 { " (trust below 2: not federating)" } else { "" };
    format!("not linked, {status}{since}{why}")
}

/// The link's own state in words (the `link` object, `federation_log::peer_view`): how its
/// messages flow (normally, slowed, or held back automatically for sending too fast, which is not
/// an admin's pause), how often it was held back, what was missed or set aside, and how far this
/// server has read the peer's log. None when the peer has no link state (no key yet).
pub(crate) fn link_words(p: &Value) -> Option<String> {
    let flow = p.get("link")?.get("flow").filter(|f| f.is_object())?;
    let mut parts = vec![match flow["state"].as_str() {
        Some("paused") => "held back automatically for sending too fast".to_string(),
        Some("slowed") => "slowed automatically for sending fast".to_string(),
        _ => "flowing normally".to_string(),
    }];
    let n = |k: &str| flow[k].as_u64().unwrap_or(0);
    if n("pauses") > 0 {
        parts.push(format!("held back {} times", n("pauses")));
    }
    if n("missed") > 0 {
        parts.push(format!("{} messages missed", n("missed")));
    }
    if n("set_aside") > 0 {
        parts.push(format!("{} set aside", n("set_aside")));
    }
    if let Some(seq) = p["link"]["cursor"]["seq"].as_u64() {
        parts.push(format!("read their log to message {seq}"));
    }
    Some(format!("Link: {}.", parts.join("; ")))
}

pub(crate) fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::subsection_label(ui, theme, "Federation");
    widgets::body_hint(
        ui,
        theme,
        "The other servers this one links with. A peer does nothing until its trust is 2 or more: below \
         that, no link is made in either direction. Pause stops federating with a peer for now and keeps \
         it and its trust; Remove forgets it. Only the channels ticked below travel to peers.",
    );
    ui.add_space(theme.spacing_xs);

    let connected = server_base(state).is_some();
    if connected && state.admin_console.federation.is_none() && state.admin_console.federation_status.1.is_empty() {
        state.admin_console.federation_status = (true, "Loading...".into());
        run_action(state, "federation.list", json!({}), None);
    }

    // The guaranteed local-only room (v0.1132): beside federation, because it is its counterweight.
    // Only on a working copy made from the server's real settings: the box sends the whole copy.
    if let Some(draft) = crate::gui::connections::page_draft(state.server_settings_draft.as_ref(), state.server_settings.as_ref()).as_mut() {
        let mut on = draft.local_channel_enabled;
        if ui
            .checkbox(&mut on, "Keep a guaranteed local-only room (#local) that can never be bridged")
            .on_hover_text(
                "While on, this server always has a #local channel and REFUSES to federate it, so members \
                 have a room that provably stays here. Turning it off makes #local an ordinary channel.",
            )
            .changed()
        {
            draft.local_channel_enabled = on;
            if let Some(d) = state.server_settings_draft.as_mut() {
                d.local_channel_enabled = on;
            }
            let d = draft.clone();
            send_server_settings_update(state, &d);
        }
    }

    let mut to_run: Vec<(&'static str, Value)> = Vec::new();
    let view = state.admin_console.federation.clone();
    if let Some(v) = &view {
        // This server's key, for another server's admin to add it by.
        if let Some(key) = v["own_key"].as_str().filter(|k| !k.is_empty()) {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("This server's federation key:").size(theme.font_size_small).color(theme.text_secondary()));
                ui.label(RichText::new(format!("{}...{}", &key[..12.min(key.len())], &key[key.len().saturating_sub(6)..])).size(theme.font_size_small).color(theme.text_muted()).monospace());
                if widgets::Button::ghost("Copy").tooltip("Copy the whole key, for another server's admin to add this one by key.").show(ui, theme) {
                    ui.ctx().copy_text(key.to_string());
                }
            });
        }

        let peers = v["peers"].as_array().cloned().unwrap_or_default();
        if peers.is_empty() {
            widgets::body_hint(ui, theme, "No peers yet. This server works fully on its own until you add one.");
        }
        for p in &peers {
            let id = p["server_id"].as_str().unwrap_or("").to_string();
            let paused = p["paused"] == true;
            ui.horizontal_wrapped(|ui| {
                let (dot, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                let colour = if p["linked"] == true { theme.success() } else if paused { theme.warning() } else { theme.text_muted() };
                ui.painter().circle_filled(dot.center(), 4.0, colour);
                ui.label(RichText::new(p["name"].as_str().unwrap_or(&id)).size(theme.font_size_body).color(theme.text_primary()).strong());
                let where_words = if p["by_key"] == true { "added by key, dials in".to_string() } else { p["url"].as_str().unwrap_or("").to_string() };
                ui.label(RichText::new(where_words).size(theme.font_size_small).color(theme.text_secondary()));
                ui.label(RichText::new(peer_state_words(p)).size(theme.font_size_small).color(if paused { theme.warning() } else { theme.text_muted() }));
                if p["accord_compliant"] == true {
                    ui.label(RichText::new("accord").size(theme.font_size_small).color(theme.success()));
                }
                let tier = p["trust_tier"].as_i64().unwrap_or(0);
                let mut picked = tier;
                egui::ComboBox::from_id_salt(format!("fed_trust_{id}")).selected_text(format!("trust {tier}")).width(80.0).show_ui(ui, |ui| {
                    for t in 0..=3 {
                        ui.selectable_value(&mut picked, t, tier_words(t));
                    }
                });
                if picked != tier {
                    to_run.push(("federation.trust", json!({ "server_id": id, "tier": picked.to_string() })));
                }
                if paused {
                    if widgets::Button::ghost("Resume").tooltip("Federate with it again, at the trust it had.").show(ui, theme) {
                        to_run.push(("federation.resume", json!({ "server_id": id })));
                    }
                } else {
                    let key = format!("federation.pause:{id}");
                    if state.admin_console.confirm.as_deref() == Some(key.as_str()) {
                        if widgets::Button::danger("Really pause?").show(ui, theme) {
                            to_run.push(("federation.pause", json!({ "server_id": id })));
                            state.admin_console.confirm = None;
                        }
                        if widgets::Button::secondary("Cancel").show(ui, theme) {
                            state.admin_console.confirm = None;
                        }
                    } else if widgets::Button::ghost("Pause").tooltip("Stop federating with it for now. An open link closes within 15 seconds. Asks once.").show(ui, theme) {
                        state.admin_console.confirm = Some(key);
                    }
                    if p["by_key"] != true
                        && widgets::Button::ghost("Re-check key").tooltip("Fetch its key again from its own address and pin it, for when its key changed and the link keeps dropping.").show(ui, theme)
                    {
                        to_run.push(("federation.recheck", json!({ "server_id": id })));
                    }
                }
                let key = format!("federation.remove:{id}");
                if state.admin_console.confirm.as_deref() == Some(key.as_str()) {
                    if widgets::Button::danger("Really remove?").show(ui, theme) {
                        to_run.push(("federation.remove", json!({ "server_id": id })));
                        state.admin_console.confirm = None;
                    }
                    if widgets::Button::secondary("Cancel").show(ui, theme) {
                        state.admin_console.confirm = None;
                    }
                } else if widgets::Button::ghost("Remove").tooltip("Stop federating and forget it. Asks once.").show(ui, theme) {
                    state.admin_console.confirm = Some(key);
                }
            });
            if let Some(words) = link_words(p) {
                ui.label(RichText::new(words).size(theme.font_size_small).color(theme.text_muted()));
            }
        }

        // Which channels travel.
        let channels = v["channels"].as_array().cloned().unwrap_or_default();
        if !channels.is_empty() {
            ui.add_space(theme.spacing_xs);
            ui.label(RichText::new("Channels that travel to peers:").size(theme.font_size_small).color(theme.text_secondary()));
            ui.horizontal_wrapped(|ui| {
                for c in &channels {
                    let id = c["id"].as_str().unwrap_or("").to_string();
                    let mut on = c["federated"] == true;
                    let can = c["can_federate"] != false;
                    let hover = if can { "Ticked: its new messages travel to every linked peer that federates it too." } else { "The guaranteed local-only room never federates while that guarantee is on." };
                    let changed = ui.add_enabled(can || on, egui::Checkbox::new(&mut on, format!("#{}", c["name"].as_str().unwrap_or(&id)))).on_hover_text(hover).changed();
                    if changed {
                        to_run.push(("federation.channel", json!({ "channel": id, "federated": on })));
                    }
                }
            });
        }
    }

    // Add a peer.
    ui.add_space(theme.spacing_xs);
    let mut add_url = false;
    let mut add_key = false;
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Add by address:").size(theme.font_size_small).color(theme.text_secondary()));
        let c = &mut state.admin_console;
        ui.add(egui::TextEdit::singleline(&mut c.fed_add_url).desired_width(220.0).hint_text("https://server.example.com"));
        ui.add(egui::TextEdit::singleline(&mut c.fed_add_name).desired_width(110.0).hint_text("name (optional)"));
        let url = c.fed_add_url.trim();
        let ok = url.starts_with("http://") || url.starts_with("https://");
        add_url = widgets::Button::primary("Add").disabled(!ok || !connected).tooltip("Register a server reachable on the internet. It starts at trust 0.").show(ui, theme);
    });
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Add by key:").size(theme.font_size_small).color(theme.text_secondary()));
        let c = &mut state.admin_console;
        ui.add(egui::TextEdit::singleline(&mut c.fed_add_key).desired_width(220.0).hint_text("the key from its Host Node page"));
        ui.add(egui::TextEdit::singleline(&mut c.fed_add_key_name).desired_width(110.0).hint_text("name (optional)"));
        let key = c.fed_add_key.trim().to_lowercase();
        let ok = crate::relay::handlers::federation::is_server_key(&key);
        if !key.is_empty() && !ok {
            ui.label(
                RichText::new(format!("a server key is {} hex characters", crate::relay::handlers::federation::SERVER_KEY_HEX_LEN))
                    .size(theme.font_size_small)
                    .color(theme.warning()),
            );
        }
        add_key = widgets::Button::primary("Pair").disabled(!ok || !connected).tooltip("Register a server behind a home router by its key. It dials this one; no port forwarding needed on its side.").show(ui, theme);
    });
    if add_url {
        let c = &mut state.admin_console;
        let mut inputs = json!({ "url": c.fed_add_url.trim() });
        if !c.fed_add_name.trim().is_empty() {
            inputs["name"] = Value::String(c.fed_add_name.trim().to_string());
        }
        c.fed_add_url.clear();
        c.fed_add_name.clear();
        to_run.push(("federation.add", inputs));
    }
    if add_key {
        let c = &mut state.admin_console;
        let mut inputs = json!({ "key": c.fed_add_key.trim().to_lowercase() });
        if !c.fed_add_key_name.trim().is_empty() {
            inputs["name"] = Value::String(c.fed_add_key_name.trim().to_string());
        }
        c.fed_add_key.clear();
        c.fed_add_key_name.clear();
        to_run.push(("federation.add_key", inputs));
    }

    for (action, inputs) in to_run {
        run_action(state, action, inputs, None);
    }

    let (ok, words) = state.admin_console.federation_status.clone();
    if !words.is_empty() {
        ui.label(RichText::new(words).size(theme.font_size_small).color(if ok { theme.text_muted() } else { theme.danger() }));
    }
    ui.horizontal(|ui| {
        if widgets::Button::secondary("Refresh").tooltip("Read the peers and channels from the server again.").show(ui, theme) {
            run_action(state, "federation.list", json!({}), None);
        }
        if widgets::Button::secondary("Connect to trusted peers").tooltip("Open links now to every peer at trust 2 or more that is not paused and has an address.").show(ui, theme) {
            run_action(state, "federation.connect", json!({}), None);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::screen_surface::find_text_in_shapes;

    fn frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, events: Vec<egui::Event>) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1600.0, 900.0))),
            events,
            ..Default::default()
        };
        ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| draw(ui, theme, state));
        })
    }

    fn click(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, label: &str) {
        let out = frame(ctx, theme, state, Vec::new());
        let pos = find_text_in_shapes(&out.shapes, label).unwrap_or_else(|| panic!("{label} is drawn")).rect.center();
        let m = egui::Modifiers::default();
        frame(ctx, theme, state, vec![egui::Event::PointerMoved(pos)]);
        frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
        frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
    }

    /// THE PANEL CAN BE USED, not only seen: a peer's row draws its state and the channels draw
    /// as boxes; Pause asks first, and "Really pause?" sends federation.pause (offline here, so
    /// the panel says to connect first, which is the run reaching `run_action`). Seen red
    /// 2026-10-22 with the confirm's button pushing nothing: "Really pause? reaches run_action".
    #[test]
    fn pause_asks_first_then_runs() {
        let ctx = egui::Context::default();
        crate::gui::fonts::install_font_fallbacks(&ctx);
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        let mut state = GuiState::default();
        state.admin_console.federation = Some(json!({
            "peers": [{ "server_id": "https://a.example", "name": "Alpha", "url": "https://a.example", "trust_tier": 2,
                        "paused": false, "linked": true, "linked_secs": 7200, "status": "online" }],
            "channels": [{ "id": "general", "name": "general", "federated": true, "local_only": false, "can_federate": true }],
            "own_key": "ab".repeat(1952),
        }));
        state.admin_console.federation_status = (true, "1 peer, 1 linked now.".into());
        frame(&ctx, &theme, &mut state, Vec::new());
        let out = frame(&ctx, &theme, &mut state, Vec::new());
        assert!(find_text_in_shapes(&out.shapes, "linked for 2 h").is_some(), "the peer's state is drawn");
        assert!(find_text_in_shapes(&out.shapes, "#general").is_some(), "the channel is a box");
        click(&ctx, &theme, &mut state, "Pause");
        assert_eq!(state.admin_console.confirm.as_deref(), Some("federation.pause:https://a.example"), "Pause asks first");
        click(&ctx, &theme, &mut state, "Really pause?");
        assert_eq!(state.admin_console.confirm, None);
        assert_eq!(state.admin_console.federation_status, (false, "Connect to the server first.".to_string()), "Really pause? reaches run_action");
    }

    /// A peer's state reads as words: paused first, then linked and for how long, then why not.
    /// Seen red 2026-10-22 with the paused check after the linked one: "a paused peer says so".
    #[test]
    fn a_peers_state_reads_as_words() {
        assert_eq!(peer_state_words(&json!({ "paused": true, "linked": true })), "paused by an admin", "a paused peer says so");
        // The link's automatic hold-back reads as that, never as the admin's pause.
        let link = json!({ "link": { "flow": { "state": "paused", "pauses": 2, "missed": 3, "set_aside": 0 }, "cursor": { "log": "x", "seq": 154 } } });
        assert_eq!(
            link_words(&link).as_deref(),
            Some("Link: held back automatically for sending too fast; held back 2 times; 3 messages missed; read their log to message 154.")
        );
        assert_eq!(link_words(&json!({ "link": null })), None, "no link state, no line");
        assert_eq!(peer_state_words(&json!({ "paused": false, "linked": true, "linked_secs": 7200 })), "linked for 2 h");
        assert_eq!(
            peer_state_words(&json!({ "paused": false, "linked": false, "status": "unreachable", "trust_tier": 0 })),
            "not linked, unreachable (trust below 2: not federating)"
        );
        // last_seen is the time of the last status change unless a link is up (seen on the real
        // relay 2026-10-23: "unreachable, last heard just now"): "since", not "last heard".
        let two_min_ago = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64 - 125_000;
        assert_eq!(
            peer_state_words(&json!({ "paused": false, "linked": false, "status": "unreachable", "trust_tier": 2, "last_seen": two_min_ago })),
            "not linked, unreachable since 2 min ago"
        );
    }
}
