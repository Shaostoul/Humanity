//! Character showroom panel (v0.441/442). One orbiting-avatar scene, three modes:
//!   0 = character select (on spawn): edit appearance + backdrop, "Enter your home".
//!   1 = appearance editor (wetroom mirror): edit appearance, "Done".
//!   2 = wardrobe (bedroom): equip cosmetics per slot, "Done".
//! The panel only edits `gui_state` (appearance / outfit / backdrop / confirm); the main
//! loop applies it to the avatar mesh, camera, backdrop, and save (edit-buffer-then-sync).

use egui::{Context, RichText, ScrollArea};

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::{GuiState, LauncherSel};

const SLOTS: [&str; 6] = ["head", "chest", "legs", "feet", "hands", "back"];

/// Display name for the implicit "no saves yet, enter a fresh homestead" row.
const NEW_HOMESTEAD: &str = "My Homestead";

pub fn draw(ctx: &Context, theme: &Theme, state: &mut GuiState) {
    // Land any finished server-info fetch into the cache (v0.478).
    drain_server_info(state);
    // mode 0 (Play) is the UNIFIED launcher: the left pane lists homes/characters
    // AND servers, and the right pane shows the character editor OR server details
    // depending on what's selected. Modes 1/2 (in-world mirror/wardrobe) are the
    // focused editors -- no server browsing. (v0.476)
    let show_server = state.showroom_mode == 0 && state.launcher_selected_kind == LauncherSel::Server;
    let left_title = if state.showroom_mode == 0 { "Play" } else { "Character" };
    let (right_title, confirm_label, right_hint) = match state.showroom_mode {
        1 => ("Appearance", "Done", "Drag the center to orbit. Wheel to zoom."),
        2 => ("Wardrobe", "Done", "Drag the center to orbit. Wheel to zoom."),
        _ if show_server => ("Server", "Connect", "Details about the server you picked on the left."),
        _ => ("Character", "Enter World", "Drag the center to orbit. Wheel to zoom."),
    };

    // ── LEFT column: home/character/server selector ──
    egui::SidePanel::left("showroom_select")
        .resizable(false)
        .exact_width(230.0)
        .show(ctx, |ui| {
            ui.add_space(theme.spacing_md);
            // In the Play picker (mode 0) a visible Back returns to the menu without
            // entering the world -- the nav bar is hidden here, so Esc alone is not
            // discoverable. (v0.476.1)
            if state.showroom_mode == 0
                && widgets::Button::secondary("< Back")
                    .tooltip("Return to the menu without entering the world. Same as Esc.")
                    .show(ui, theme)
            {
                state.showroom_cancel = true;
            }
            ui.label(RichText::new(left_title).size(theme.font_size_body).strong().color(theme.text_primary()));
            ui.add_space(theme.spacing_sm);
            draw_character_select(ui, theme, state);
        });

    // ── RIGHT column: details + customization ──
    egui::SidePanel::right("showroom_details")
        .resizable(false)
        .exact_width(310.0)
        .show(ctx, |ui| {
            ui.add_space(theme.spacing_md);
            ui.label(RichText::new(right_title).size(theme.font_size_body).strong().color(theme.text_primary()));
            ui.label(
                RichText::new(right_hint)
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
            );
            ui.add_space(theme.spacing_sm);

            if show_server {
                draw_server_details(ui, theme, state);
                return;
            }

            // Character name (the GAME character's name, separate from your chat profile).
            if state.showroom_mode != 2 {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Name").color(theme.text_secondary()));
                    ui.text_edit_singleline(&mut state.character_name);
                });
                ui.add_space(theme.spacing_sm);
            }

            if state.showroom_mode == 2 {
                draw_wardrobe(ui, theme, state);
            } else {
                draw_appearance(ui, theme, state);
            }

            ui.add_space(theme.spacing_sm);
            draw_backdrop(ui, theme, state);
            ui.add_space(theme.spacing_md);

            if ui
                .button(RichText::new(confirm_label).size(theme.font_size_body).strong())
                .clicked()
            {
                state.showroom_confirm = true;
            }
        });
    // (No CentralPanel: the 3D avatar renders in the center gap between the two columns.)
}

/// Left column. In the Play character-select (mode 0) this is the UNIFIED
/// LAUNCHER: Your Homes (offline saves), Open-Net + Closed-Net characters
/// (multiplayer placeholders), and Servers. In the in-world mirror/wardrobe
/// (modes 1/2) it is just the character you are editing. (v0.476)
fn draw_character_select(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // Modes 1/2: focused editor, no picker.
    if state.showroom_mode != 0 {
        let name = if state.character_name.trim().is_empty() {
            "Your Character".to_string()
        } else {
            state.character_name.clone()
        };
        let _ = ui.selectable_label(true, RichText::new(name).color(theme.text_primary()));
        ui.add_space(theme.spacing_xs);
        ui.label(
            RichText::new("Editing your look. Close to return to the world.")
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
        return;
    }

    // Mode 0: the unified launcher. Lazy-load the local save list once per open.
    if !state.launcher_saves_loaded {
        state.launcher_saves = crate::persistence::list_saves(&crate::persistence::saves_dir());
        state.launcher_saves_loaded = true;
        if state.launcher_selected.is_empty() {
            state.launcher_selected = state
                .launcher_saves
                .first()
                .map(|(n, _)| n.clone())
                .unwrap_or_else(|| NEW_HOMESTEAD.to_string());
        }
    }

    // Snapshot everything the closure reads, so it never holds a borrow of state.
    let mut home_rows: Vec<String> = state.launcher_saves.iter().map(|(n, _)| n.clone()).collect();
    if home_rows.is_empty() {
        home_rows.push(NEW_HOMESTEAD.to_string());
    }
    let selected = state.launcher_selected.clone();
    let selected_kind = state.launcher_selected_kind;
    let default_name = state.launcher_default_character.clone();
    let servers: Vec<(String, String, bool)> = state
        .chat_servers
        .iter()
        .map(|s| (s.id.clone(), s.name.clone(), s.connected))
        .collect();
    let selected_server = state.launcher_selected_server.clone();

    // Deferred mutations (applied after the closure so we never alias state).
    let mut pick_home: Option<String> = None;
    let mut pick_server: Option<String> = None;
    let mut toggle_default = false;

    ScrollArea::vertical().show(ui, |ui| {
        // ── Your Homes ──
        section_header(ui, theme, "Your Homes");
        hint(ui, theme, "Offline saves you fully own. Each is a character plus a home.");
        for name in &home_rows {
            let is_sel = selected_kind == LauncherSel::Home && &selected == name;
            let mut label = name.clone();
            if !default_name.is_empty() && &default_name == name {
                label.push_str("  (default)");
            }
            if ui.selectable_label(is_sel, RichText::new(label).color(theme.text_primary())).clicked() {
                pick_home = Some(name.clone());
            }
        }
        // Default toggle for the selected home.
        if selected_kind == LauncherSel::Home {
            ui.add_space(theme.spacing_xs);
            let sel_is_default = !default_name.is_empty() && default_name == selected;
            let btn = if sel_is_default { "Clear default" } else { "Set as default" };
            if ui.small_button(btn).clicked() {
                toggle_default = true;
            }
            hint(ui, theme, "A default lets Play skip this screen and drop you straight in.");
        }

        // ── Open-Net Characters (multiplayer placeholder) ──
        ui.add_space(theme.spacing_sm);
        section_header(ui, theme, "Open-Net Characters");
        hint(ui, theme, "Your local character on a server that allows self-custody, like Open Battle.net. Arrives with multiplayer.");

        // ── Closed-Net Characters (multiplayer placeholder) ──
        ui.add_space(theme.spacing_sm);
        section_header(ui, theme, "Closed-Net Characters");
        hint(ui, theme, "Characters the server holds so progress cannot be forged, like Closed Battle.net. Arrives with multiplayer.");

        // ── Servers ──
        ui.add_space(theme.spacing_sm);
        section_header(ui, theme, "Servers");
        hint(ui, theme, "Communities you can join. Click one for details.");
        if servers.is_empty() {
            hint(ui, theme, "No servers yet. Add one from the Chat sidebar.");
        } else {
            for (id, name, connected) in &servers {
                let is_sel = selected_kind == LauncherSel::Server && selected_server.as_deref() == Some(id.as_str());
                let label = if *connected {
                    format!("{name}  (connected)")
                } else {
                    name.clone()
                };
                if ui.selectable_label(is_sel, RichText::new(label).color(theme.text_primary())).clicked() {
                    pick_server = Some(id.clone());
                }
            }
        }
    });

    // Apply deferred mutations.
    if let Some(name) = pick_home {
        state.launcher_selected = name.clone();
        state.launcher_selected_kind = LauncherSel::Home;
        // Swap the previewed avatar to this character (lib.rs applies the save).
        state.launcher_pending_load = Some(name);
    }
    if let Some(id) = pick_server {
        state.launcher_selected_kind = LauncherSel::Server;
        state.launcher_selected_server = Some(id);
    }
    if toggle_default {
        if state.launcher_default_character == state.launcher_selected {
            state.launcher_default_character.clear();
        } else {
            state.launcher_default_character = state.launcher_selected.clone();
        }
        crate::config::AppConfig::from_gui_state(state).save();
    }
}

/// Right pane when a Server is selected in the launcher: live metadata fetched
/// from the server's /api/server-info (name, description, version, members,
/// online, accord, channels), an admin-only description editor for the server
/// you are connected to, and a Connect action (multiplayer-future). (v0.478)
fn draw_server_details(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let Some(id) = state.launcher_selected_server.clone() else {
        ui.label(RichText::new("Pick a server on the left.").color(theme.text_muted()));
        return;
    };
    let Some(server) = state.chat_servers.iter().find(|s| s.id == id).cloned() else {
        ui.label(RichText::new("That server is no longer in your list.").color(theme.text_muted()));
        return;
    };

    // Kick off a one-time fetch of this server's info if we don't have it.
    if !state.server_info_cache.contains_key(&id) {
        fetch_server_info(state, &id, &server.url);
    }
    let info = state.server_info_cache.get(&id).cloned();

    // Name: the fetched name if we have it, else the locally-known one.
    let title = info.as_ref().map(|i| i.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| server.name.clone());
    ui.label(RichText::new(title).size(theme.font_size_heading).strong().color(theme.text_primary()));
    ui.add_space(theme.spacing_xs);

    detail_row(ui, theme, "Address", &server.url);
    detail_row(ui, theme, "Status", if server.connected { "Connected" } else { "Not connected" });

    match &info {
        None => {
            ui.add_space(theme.spacing_xs);
            hint(ui, theme, "Loading server info...");
        }
        Some(i) => {
            ui.add_space(theme.spacing_xs);
            if !i.description.trim().is_empty() {
                ui.label(RichText::new(&i.description).size(theme.font_size_small).color(theme.text_secondary()));
                ui.add_space(theme.spacing_xs);
            }
            if !i.version.is_empty() { detail_row(ui, theme, "Version", &i.version); }
            detail_row(ui, theme, "Members", &i.member_count.to_string());
            detail_row(ui, theme, "Online now", &i.users_online.to_string());
            detail_row(ui, theme, "Channels", &i.channels.len().to_string());
            detail_row(ui, theme, "Accord", if i.accord_compliant { "Compliant" } else { "Not declared" });
        }
    }

    // Admin description editor: only for the server you are CONNECTED to (the one
    // your authenticated socket can update) AND only if you are admin/owner. The
    // relay is the authoritative gate; this is defense-in-depth.
    let connected_here = !state.server_url.is_empty()
        && server.url.trim_end_matches('/') == state.server_url.trim_end_matches('/');
    if connected_here && current_user_is_admin(state) {
        ui.add_space(theme.spacing_md);
        ui.separator();
        widgets::section_header(ui, theme, "Server description");
        hint(ui, theme, "Shown to everyone who views this server. Admins only.");
        // Reload the draft from the fetched description when switching servers.
        // Only mark it synced once the info has actually loaded, so the editor
        // doesn't lock in an empty draft while the fetch is still in flight.
        if state.server_desc_draft_for != id {
            if let Some(i) = &info {
                state.server_desc_draft = i.description.clone();
                state.server_desc_draft_for = id.clone();
                state.server_desc_status.clear();
            } else {
                state.server_desc_draft.clear();
            }
        }
        ui.add(egui::TextEdit::multiline(&mut state.server_desc_draft).desired_rows(3).desired_width(f32::INFINITY));
        ui.add_space(theme.spacing_xs);
        if widgets::Button::primary("Save description").show(ui, theme) {
            let desc = state.server_desc_draft.clone();
            send_server_description(state, &desc);
            // Optimistically reflect it locally so the pane updates immediately.
            if let Some(i) = state.server_info_cache.get_mut(&id) {
                i.description = desc;
            }
            state.server_desc_status = "Saved. The new description is live.".to_string();
        }
        if !state.server_desc_status.is_empty() {
            ui.label(RichText::new(state.server_desc_status.clone()).size(theme.font_size_small).color(theme.accent()));
        }
    }

    ui.add_space(theme.spacing_md);
    ui.add_enabled(
        false,
        egui::Button::new(RichText::new("Connect").size(theme.font_size_body).strong()),
    )
    .on_disabled_hover_text("Joining servers in-game arrives with multiplayer.");
}

/// Is the current user an admin/owner (from the chat user list)? Defense-in-depth
/// only; the relay is authoritative. Mirrors game_admin's gate.
fn current_user_is_admin(state: &GuiState) -> bool {
    state
        .chat_users
        .iter()
        .find(|u| u.public_key == state.profile_public_key)
        .map(|u| matches!(u.role.as_str(), "admin" | "owner"))
        .unwrap_or(false)
}

/// Spawn a background blocking fetch of GET {url}/api/server-info. Stores the
/// result channel in state; drain_server_info lands it into the cache. No-op if
/// already cached or a fetch for this id is already in flight.
fn fetch_server_info(state: &mut GuiState, server_id: &str, url: &str) {
    if state.server_info_cache.contains_key(server_id) {
        return;
    }
    if state.server_info_loader.as_ref().map_or(false, |(id, _)| id == server_id) {
        return;
    }
    let api = format!("{}/api/server-info", url.trim_end_matches('/'));
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = (|| -> Result<crate::gui::ServerInfo, String> {
            let resp = ureq::get(&api).call().map_err(|e| e.to_string())?;
            let body = resp.into_string().map_err(|e| e.to_string())?;
            serde_json::from_str::<crate::gui::ServerInfo>(&body).map_err(|e| e.to_string())
        })();
        let _ = tx.send(result);
    });
    state.server_info_loader = Some((server_id.to_string(), rx));
}

/// Land a finished server-info fetch into the cache. Called once per frame.
fn drain_server_info(state: &mut GuiState) {
    let mut done: Option<(String, Result<crate::gui::ServerInfo, String>)> = None;
    if let Some((id, rx)) = &state.server_info_loader {
        match rx.try_recv() {
            Ok(res) => done = Some((id.clone(), res)),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                done = Some((id.clone(), Err("fetch worker exited".to_string())));
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
    }
    if let Some((id, res)) = done {
        state.server_info_loader = None;
        if let Ok(info) = res {
            state.server_info_cache.insert(id, info);
        }
        // On error we just leave it uncached; a later reselect retries.
    }
}

/// Send a partial server_settings_update carrying ONLY the description. The
/// relay treats every other field as None (unchanged), so this never clobbers
/// other settings. Admin-gated server-side.
fn send_server_description(state: &GuiState, desc: &str) {
    if let Some(ref client) = state.ws_client {
        if client.is_connected() {
            let msg = serde_json::json!({ "type": "server_settings_update", "server_description": desc });
            client.send(&msg.to_string());
        }
    }
}

/// A small section header used by the launcher left pane.
fn section_header(ui: &mut egui::Ui, theme: &Theme, text: &str) {
    ui.label(RichText::new(text).size(theme.font_size_body).strong().color(theme.text_primary()));
}

/// A muted one-line contextual hint (the operator loves in-page help).
fn hint(ui: &mut egui::Ui, theme: &Theme, text: &str) {
    ui.label(RichText::new(text).size(theme.font_size_small).color(theme.text_muted()));
}

/// A "Label: value" detail line for the server pane.
fn detail_row(ui: &mut egui::Ui, theme: &Theme, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{label}:")).size(theme.font_size_small).color(theme.text_secondary()));
        ui.label(RichText::new(value).size(theme.font_size_small).color(theme.text_primary()));
    });
}

fn draw_appearance(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Appearance").strong().color(theme.text_primary()));
    ui.horizontal(|ui| {
        ui.label(RichText::new("Skin").color(theme.text_secondary()));
        if ui.color_edit_button_rgb(&mut state.appearance.skin_tone).changed() {
            state.appearance_dirty = true;
        }
    });
    ui.horizontal(|ui| {
        ui.label(RichText::new("Hair").color(theme.text_secondary()));
        if ui.color_edit_button_rgb(&mut state.appearance.hair_color).changed() {
            state.appearance_dirty = true;
        }
    });
    ui.horizontal(|ui| {
        ui.label(RichText::new("Eyes").color(theme.text_secondary()));
        if ui.color_edit_button_rgb(&mut state.appearance.eye_color).changed() {
            state.appearance_dirty = true;
        }
    });
    if widgets::labeled_slider(ui, theme, "Height", &mut state.appearance.height_scale, 0.8..=1.2) {
        state.appearance_dirty = true;
    }
    if state.showroom_mode == 0 {
        ui.label(
            RichText::new("Outfits: change them at the bedroom wardrobe.")
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
    }
}

fn draw_wardrobe(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Wardrobe").strong().color(theme.text_primary()));
    for slot in SLOTS {
        let current = state.outfit.equipped.get(slot).cloned();
        // Cosmetics available for this slot (id, name).
        let items: Vec<(String, String)> = state
            .cosmetics_list
            .iter()
            .filter(|(_, _, s)| s == slot)
            .map(|(id, name, _)| (id.clone(), name.clone()))
            .collect();
        if items.is_empty() {
            continue;
        }
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new(cap(slot)).color(theme.text_secondary()));
        ui.horizontal_wrapped(|ui| {
            if ui.selectable_label(current.is_none(), "None").clicked() {
                state.outfit.equipped.remove(slot);
                state.outfit_dirty = true;
            }
            for (id, name) in &items {
                let selected = current.as_deref() == Some(id.as_str());
                if ui.selectable_label(selected, name).clicked() {
                    state.outfit.equipped.insert(slot.to_string(), id.clone());
                    state.outfit_dirty = true;
                }
            }
        });
    }
}

fn draw_backdrop(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Backdrop").strong().color(theme.text_primary()));
    let n = state.showroom_backdrop_names.len().max(1);
    ui.horizontal(|ui| {
        if ui.button(RichText::new("  <  ")).clicked() {
            state.showroom_backdrop = (state.showroom_backdrop + n - 1) % n;
        }
        let name = state
            .showroom_backdrop_names
            .get(state.showroom_backdrop)
            .cloned()
            .unwrap_or_default();
        ui.label(RichText::new(name).color(theme.text_secondary()));
        if ui.button(RichText::new("  >  ")).clicked() {
            state.showroom_backdrop = (state.showroom_backdrop + 1) % n;
        }
    });
}

/// Capitalize a slot id for display ("chest" -> "Chest").
fn cap(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}
