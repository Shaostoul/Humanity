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

/// Sentinel id for the VIRTUAL "server you are connected to right now" row in
/// the launcher's Servers list (v0.775). The app auto-connects to `server_url`
/// (united-humanity.us by default) for chat, but that live connection was never
/// shown here -- only explicitly-saved `chat_servers` were -- so the operator
/// saw "No servers yet" while actually connected. This id is not a real saved
/// server; draw_server_details special-cases it to read `server_url` directly.
const CONNECTED_SERVER_ID: &str = "__connected__";

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
    // Servers list: the LIVE connection you are on right now (virtual, v0.775)
    // first, then your explicitly-saved bookmarks. The virtual row is what the
    // operator was missing -- the app auto-connects to server_url for chat, but
    // only saved chat_servers were listed here, so a connected user saw "No
    // servers yet". Deduped against saved servers by url so it never doubles.
    let ws_connected = state.ws_client.as_ref().map_or(false, |c| c.is_connected());
    let primary_url = state.server_url.trim_end_matches('/').to_string();
    let already_saved = state
        .chat_servers
        .iter()
        .any(|s| s.url.trim_end_matches('/') == primary_url);
    let mut servers: Vec<(String, String, bool)> = Vec::new();
    if ws_connected && !primary_url.is_empty() && !already_saved {
        servers.push((
            CONNECTED_SERVER_ID.to_string(),
            crate::gui::pages::chat::server_display_name(&state.server_url),
            true,
        ));
    }
    servers.extend(state.chat_servers.iter().map(|s| {
        // A saved bookmark of the server we're LIVE-connected to counts as
        // connected (v0.779): ChatServer.connected is never maintained by the
        // connection code, so without this URL match, bookmarking your own
        // server made it show "Not connected" and permanently disabled Enter
        // World (the working virtual row is deduped away above).
        let live = ws_connected && s.url.trim_end_matches('/') == primary_url;
        (s.id.clone(), s.name.clone(), s.connected || live)
    }));
    let selected_server = state.launcher_selected_server.clone();

    // Deferred mutations (applied after the closure so we never alias state).
    let mut pick_home: Option<String> = None;
    let mut pick_server: Option<String> = None;
    let mut toggle_default = false;

    ScrollArea::vertical().show(ui, |ui| {
        // ── Your Homes: RED (offline, fully self-owned) ──
        section_card(ui, theme, "Your Homes", theme.danger(), |ui| {
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
        });

        // ── Open-Net: GREEN (discovery; bring your offline character) ──
        ui.add_space(theme.spacing_sm);
        section_card(ui, theme, "Open Net", theme.success(), |ui| {
            hint(
                ui,
                theme,
                "Visit a server with your OFFLINE character to see what it's about -- \
                 no new character needed. Self-custody, like Open Battle.net.",
            );
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

        // ── Closed-Net: BLUE (the committed, server-held story world) ──
        ui.add_space(theme.spacing_sm);
        section_card(ui, theme, "Closed Net", theme.info(), |ui| {
            hint(
                ui,
                theme,
                "Commit to a server's main story arc: characters the server holds so \
                 progress cannot be forged, like Closed Battle.net. Your base body \
                 carries over; augments are earned in-world. Arrives with multiplayer.",
            );
        });
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
    // Resolve the selection to (name, url, connected). The virtual
    // CONNECTED_SERVER_ID row reads server_url directly -- it is the LIVE
    // connection, not a saved bookmark; everything else looks up chat_servers.
    let (svr_name, svr_url, mut svr_connected) = if id == CONNECTED_SERVER_ID {
        (
            crate::gui::pages::chat::server_display_name(&state.server_url),
            state.server_url.clone(),
            state.ws_client.as_ref().map_or(false, |c| c.is_connected()),
        )
    } else {
        match state.chat_servers.iter().find(|s| s.id == id) {
            Some(s) => (s.name.clone(), s.url.clone(), s.connected),
            None => {
                ui.label(RichText::new("That server is no longer in your list.").color(theme.text_muted()));
                return;
            }
        }
    };
    // A saved bookmark of the live connection counts as connected (v0.779):
    // ChatServer.connected is never maintained, so the URL match is the truth.
    if !svr_connected
        && state.ws_client.as_ref().map_or(false, |c| c.is_connected())
        && svr_url.trim_end_matches('/') == state.server_url.trim_end_matches('/')
    {
        svr_connected = true;
    }

    // Kick off a one-time fetch of this server's info if we don't have it.
    // The VIRTUAL row's cache entry is keyed by its URL (v0.779): the sentinel
    // id maps to "whatever server_url is NOW", so a plain sentinel key served
    // the PREVIOUS server's cached info after switching connections.
    let cache_id = if id == CONNECTED_SERVER_ID {
        format!("{CONNECTED_SERVER_ID}:{svr_url}")
    } else {
        id.clone()
    };
    if !state.server_info_cache.contains_key(&cache_id) {
        fetch_server_info(state, &cache_id, &svr_url);
    }
    let info = state.server_info_cache.get(&cache_id).cloned();

    // Name: the fetched name if we have it, else the locally-known one.
    let title = info.as_ref().map(|i| i.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| svr_name.clone());
    ui.label(RichText::new(title).size(theme.font_size_heading).strong().color(theme.text_primary()));
    ui.add_space(theme.spacing_xs);

    detail_row(ui, theme, "Address", &svr_url);
    detail_row(ui, theme, "Status", if svr_connected { "Connected" } else { "Not connected" });

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
            // In-world co-presence count (v0.776): avatars actually in the
            // shared world right now, distinct from chat "Online now".
            detail_row(ui, theme, "In world", &i.game_players.to_string());
            detail_row(ui, theme, "Channels", &i.channels.len().to_string());
            detail_row(ui, theme, "Accord", if i.accord_compliant { "Compliant" } else { "Not declared" });
        }
    }

    // The description is EDITED in Server Settings (the admin's home for their
    // server), not here. For the server you are connected to, point the way.
    let connected_here = !state.server_url.is_empty()
        && svr_url.trim_end_matches('/') == state.server_url.trim_end_matches('/');
    if connected_here {
        ui.add_space(theme.spacing_sm);
        hint(ui, theme, "Admins: edit this server's description in Server Settings (server cog in Chat).");
    }

    ui.add_space(theme.spacing_md);
    // Enter the shared world ON this server (v0.775). The game auto-joins the
    // shared world over the live connection whenever you are in-world (v0.472+),
    // so entering the server you are connected to drops you straight in with
    // co-presence active -- you will see others who are also present, tracked in
    // the top-left roster. Only enabled for the server you are actually
    // connected to; switching the live connection to a DIFFERENT saved server
    // from here is the multiplayer-future step.
    if svr_connected && connected_here {
        if ui
            .button(RichText::new("Enter World").size(theme.font_size_body).strong())
            .on_hover_text("Drop into the shared world. You'll see others who are here too.")
            .clicked()
        {
            state.showroom_confirm = true;
        }
        ui.add_space(theme.spacing_xs);
        hint(ui, theme, "Connected. Enter to join the shared world; the top-left roster shows who else is here.");
    } else {
        ui.add_enabled(
            false,
            egui::Button::new(RichText::new("Enter World").size(theme.font_size_body).strong()),
        )
        .on_disabled_hover_text("Connect to this server in the Chat sidebar first, then enter.");
    }
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

/// A COLOR-CODED launcher section card (v0.784, operator's RGB scheme):
/// offline homes = RED, open-net = GREEN, closed-net = BLUE. A tinted frame +
/// colored title so the three trust models read at a glance; the color carries
/// meaning (self-custody vs discovery vs server-held), matching the same
/// red/green/blue language planned across surfaces.
fn section_card(
    ui: &mut egui::Ui,
    theme: &Theme,
    title: &str,
    color: egui::Color32,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    egui::Frame::none()
        .fill(color.linear_multiply(0.06))
        .stroke(egui::Stroke::new(1.0, color.linear_multiply(0.55)))
        .rounding(egui::Rounding::same(theme.border_radius as u8))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new(title).size(theme.font_size_body).strong().color(color));
            add_contents(ui);
        });
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
