//! Character showroom panel (v0.441/442). One orbiting-avatar scene, three modes:
//!   0 = the Play picker: WHO (left column) + WHERE (right column) + ONE Enter
//!       bar along the bottom that states the pairing it commits.
//!   1 = appearance editor: the in-world wetroom mirror, and the picker's
//!       "Edit look" (whose Done returns to the picker, not the world).
//!   2 = wardrobe (bedroom): equip cosmetics per slot, "Done".
//! The panel only edits `gui_state` (appearance / outfit / backdrop / selection /
//! confirm); the main loop applies it to the avatar mesh, camera, backdrop, and
//! save (edit-buffer-then-sync). Structure spec: docs/design/play-characters.md.

use egui::{Context, RichText, ScrollArea};

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::{GuiState, LauncherWhere};

// Wardrobe slots come from data/inventory/equipment_slots.json (v0.942
// infinite-of-x: was a hardcoded array duplicating that registry). The
// inventory page and this wardrobe now share the one slot list via
// gui_state.equipment_slots (id, label), loaded at startup.

/// Display label for the WHERE row that means "the world you are already in,
/// made yours". It has no save on disk yet, so its id is the empty string.
const NEW_HOMESTEAD: &str = "My Homestead (new)";

/// Sentinel id for the VIRTUAL "server you are connected to right now" row in
/// the picker's Open Net card (v0.775). The app auto-connects to `server_url`
/// (united-humanity.us by default) for chat, but that live connection was never
/// shown here -- only explicitly-saved `chat_servers` were -- so the operator
/// saw "No servers yet" while actually connected. This id is not a real saved
/// server; `resolve_server` special-cases it to read `server_url` directly.
const CONNECTED_SERVER_ID: &str = "__connected__";

pub fn draw(ctx: &Context, theme: &Theme, state: &mut GuiState) {
    // Land any finished server-info fetch into the cache (v0.478).
    drain_server_info(state);
    if state.showroom_mode == 0 {
        draw_picker(ctx, theme, state);
    } else {
        draw_editor(ctx, theme, state);
    }
    // (No CentralPanel: the 3D avatar renders in the center gap between the columns.)
}

// ─────────────────────────────────────────────────────────────────────────
// Mode 0: the Play picker (WHO / WHERE / one Enter)
// ─────────────────────────────────────────────────────────────────────────

/// The pairing the bottom bar states and the Enter button commits.
struct Pairing {
    /// Display name of the WHO half.
    who: String,
    /// Display name of the WHERE half.
    world: String,
    /// True for a server world (shared), false for a local home (solo).
    shared: bool,
    /// Why this pairing cannot be entered yet, stated in the open. None = go.
    blocked: Option<String>,
}

fn draw_picker(ctx: &Context, theme: &Theme, state: &mut GuiState) {
    // Rescan the saves once per opening; the openers clear the flag.
    if !state.launcher_homes_loaded {
        load_homes(state);
    }

    let pair = pairing(state);
    let sentence = format!(
        "Enter as {} -> {} ({})",
        pair.who,
        pair.world,
        if pair.shared { "shared" } else { "solo" }
    );

    // The Enter bar is shown FIRST on purpose: egui panels claim space in the
    // order they are added, so a bottom panel added after the two side columns
    // would only span the gap between them instead of the window.
    egui::TopBottomPanel::bottom("showroom_enter_bar").show(ctx, |ui| {
        ui.add_space(theme.spacing_sm);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(&sentence)
                    .size(theme.font_size_body)
                    .strong()
                    .color(theme.text_primary()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::Button::primary("Enter")
                    .disabled(pair.blocked.is_some())
                    .tooltip("Play as this character in this world.")
                    .show(ui, theme)
                {
                    state.showroom_confirm = true;
                }
            });
        });
        // A disabled Enter always says WHY, on screen, not in a tooltip nobody
        // hovers (play-characters.md: honest disabled states, never hidden).
        if let Some(reason) = &pair.blocked {
            ui.label(
                RichText::new(reason)
                    .size(theme.font_size_small)
                    .color(theme.warning()),
            );
        }
        ui.add_space(theme.spacing_sm);
    });

    // ── WHO: the characters you can be ──
    egui::SidePanel::left("showroom_select")
        .resizable(false)
        .exact_width(230.0)
        .show(ctx, |ui| {
            ui.add_space(theme.spacing_md);
            // A visible Back returns to the menu without entering the world --
            // the nav bar is hidden here, so Esc alone is not discoverable.
            // (v0.476.1)
            if widgets::Button::secondary("< Back")
                .tooltip("Return to the menu without entering the world. Same as Esc.")
                .show(ui, theme)
            {
                state.showroom_cancel = true;
            }
            ui.label(
                RichText::new("Who")
                    .size(theme.font_size_body)
                    .strong()
                    .color(theme.text_primary()),
            );
            ui.add_space(theme.spacing_sm);
            draw_who_column(ui, theme, state);
        });

    // ── WHERE: the worlds that character can enter ──
    egui::SidePanel::right("showroom_details")
        .resizable(false)
        .exact_width(310.0)
        .show(ctx, |ui| {
            ui.add_space(theme.spacing_md);
            ui.label(
                RichText::new("Where")
                    .size(theme.font_size_body)
                    .strong()
                    .color(theme.text_primary()),
            );
            ui.label(
                RichText::new("Pick the world this character enters.")
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
            );
            ui.add_space(theme.spacing_sm);
            draw_where_column(ui, theme, state);
        });
}

/// Rescan the saves directory into the picker's (character, home) rows.
///
/// This reads the files itself instead of calling `persistence::list_saves`,
/// which returns only (world name, timestamp): the WHO column needs the
/// character name and the home summary needs the design, and both already sit
/// in the very file list_saves parses and throws away. Rows are deduped by
/// world name (the saves dir holds `offline_home.json` AND `auto_save.json`,
/// which carry the SAME `WorldSave.name`), newest kept, so a row name is a
/// usable id.
fn load_homes(state: &mut GuiState) {
    let dir = crate::persistence::saves_dir();
    let mut homes: Vec<crate::gui::LauncherHome> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            let Ok(save) = crate::persistence::load_world(&path) else {
                continue;
            };
            if save.name.trim().is_empty() {
                continue;
            }
            match homes.iter_mut().find(|h| h.world == save.name) {
                Some(existing) => {
                    if save.timestamp > existing.timestamp {
                        existing.character = save.character_name.clone();
                        existing.design = save.design.clone();
                        existing.timestamp = save.timestamp;
                    }
                }
                None => homes.push(crate::gui::LauncherHome {
                    world: save.name.clone(),
                    character: save.character_name.clone(),
                    design: save.design.clone(),
                    timestamp: save.timestamp,
                }),
            }
        }
    }
    // Most recently played first, so the fallback preselect lands on the home
    // you were last in.
    homes.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    state.launcher_homes = homes;
    state.launcher_homes_loaded = true;
    preselect(state);
}

/// Preselect the picker: the last pairing when its rows still exist, else the
/// most recent home paired with itself, else the first-run "new character into
/// a new home" pair. A row that vanished from disk never survives the fallback.
fn preselect(state: &mut GuiState) {
    let newest = state.launcher_homes.first().map(|h| h.world.clone());
    let last_char = state.launcher_last_character.clone();
    let last_where = state.launcher_last_world.clone();
    // "" is the new-character row, which always exists; any other WHO must
    // still have its save on disk.
    let char_ok = last_char.is_empty() || state.launcher_homes.iter().any(|h| h.world == last_char);
    if char_ok {
        if let Some(id) = last_where.strip_prefix("server:") {
            if !id.is_empty() {
                state.launcher_who = last_char;
                state.launcher_where_kind = LauncherWhere::Server;
                state.launcher_selected_server = Some(id.to_string());
                state.launcher_selected_world = String::new();
                return;
            }
        }
        if let Some(world) = last_where.strip_prefix("home:") {
            // The fresh-homestead row only survives while you still have no
            // home: entering it creates one, and that save is the real WHERE.
            let row_ok = if world.is_empty() {
                newest.is_none()
            } else {
                state.launcher_homes.iter().any(|h| h.world == world)
            };
            if row_ok {
                state.launcher_who = last_char;
                state.launcher_where_kind = LauncherWhere::Home;
                state.launcher_selected_world = world.to_string();
                return;
            }
        }
    }
    let world = newest.unwrap_or_default();
    state.launcher_who = world.clone();
    state.launcher_where_kind = LauncherWhere::Home;
    state.launcher_selected_world = world;
}

/// WHO column: one flat row per local character, plus "+ New character", then
/// the inline Name field and the door to the appearance editor. A character IS
/// its save file until the character/world split, so a row is keyed by the name
/// of the home it lives in (docs/design/character-and-save-custody.md).
fn draw_who_column(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let homes = state.launcher_homes.clone();
    let who = state.launcher_who.clone();
    let live_name = display_name(&state.character_name);

    // Deferred so the row closures never mutate state while it is snapshotted.
    let mut pick: Option<String> = None;
    let mut new_character = false;
    let mut edit_look = false;

    ScrollArea::vertical().show(ui, |ui| {
        hint(
            ui,
            theme,
            "Your name and look travel with you. Gear and skills stay in the \
             world you earn them in.",
        );
        ui.add_space(theme.spacing_xs);
        for home in &homes {
            let is_sel = who == home.world;
            // The selected row reads the LIVE name buffer, so an inline rename
            // shows up in the list as you type it.
            let label = if is_sel {
                live_name.clone()
            } else {
                display_name(&home.character)
            };
            if ui
                .selectable_label(is_sel, RichText::new(label).color(theme.text_primary()))
                .on_hover_text(format!("Lives in {}", home.world))
                .clicked()
            {
                pick = Some(home.world.clone());
            }
        }
        ui.add_space(theme.spacing_xs);
        if ui
            .selectable_label(
                who.is_empty(),
                RichText::new("+ New character").color(theme.text_primary()),
            )
            .on_hover_text("Start fresh: a new name and look, in a new home.")
            .clicked()
        {
            new_character = true;
        }

        ui.add_space(theme.spacing_sm);
        ui.separator();
        // Renaming is identity housekeeping, so it lives here; colours and
        // height are a session of their own, behind Edit look.
        ui.label(
            RichText::new("Name")
                .size(theme.font_size_small)
                .color(theme.text_secondary()),
        );
        ui.add(egui::TextEdit::singleline(&mut state.character_name).desired_width(f32::INFINITY));
        ui.add_space(theme.spacing_xs);
        if widgets::Button::secondary("Edit look")
            .tooltip("Skin, hair, eyes, height and the backdrop. Done returns here.")
            .show(ui, theme)
        {
            edit_look = true;
        }
    });

    if let Some(world) = pick {
        state.launcher_who = world.clone();
        // Pairing validity, first increment: a character stays bound to its own
        // home until the file split, so picking WHO carries the home half of
        // WHERE with it. A selected server is left alone (any character may
        // visit an Open Net server).
        state.launcher_selected_world = world.clone();
        // Preview this character on the 3D stage (lib.rs applies the save).
        state.launcher_pending_load = Some(world);
    }
    if new_character {
        state.launcher_who.clear();
        state.launcher_selected_world.clear();
        state.character_name = "Wanderer".to_string();
        // A new character starts from the default body, not the last one you
        // previewed, so the stage shows what you are actually about to be.
        state.appearance = Default::default();
        state.outfit = Default::default();
        state.appearance_dirty = true;
        state.outfit_dirty = true;
    }
    if edit_look {
        state.showroom_mode = 1;
        state.showroom_return_to_picker = true;
    }
}

/// WHERE column: the three trust models as color cards (v0.784, operator's RGB
/// scheme). Selecting a row expands its details inline in its own card.
fn draw_where_column(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let homes = state.launcher_homes.clone();
    let who_is_new = state.launcher_who.is_empty();
    let kind = state.launcher_where_kind;
    let sel_world = state.launcher_selected_world.clone();
    let sel_server = state.launcher_selected_server.clone();
    let servers = server_rows(state);

    let mut pick_world: Option<String> = None;
    let mut pick_server: Option<String> = None;

    ScrollArea::vertical().show(ui, |ui| {
        // ── Your Homes: RED (offline, fully self-owned) ──
        section_card(ui, theme, "Your Homes", theme.danger(), |ui| {
            hint(
                ui,
                theme,
                "Worlds you fully own, saved on this machine. Entering one plays \
                 SOLO: you won't appear in the shared world (pick a server below \
                 for that), and the Dev travel tools work freely.",
            );
            for home in &homes {
                let is_sel = kind == LauncherWhere::Home && sel_world == home.world;
                if ui
                    .selectable_label(is_sel, RichText::new(&home.world).color(theme.text_primary()))
                    .clicked()
                {
                    pick_world = Some(home.world.clone());
                }
                if is_sel {
                    detail_row(ui, theme, "Design", &home.design);
                    detail_row(ui, theme, "Last played", &last_played(home.timestamp));
                }
            }
            // The fresh-home row is offered when you own none, and whenever the
            // new-character row is picked: a new character needs a new home
            // until characters and worlds are separate files.
            if homes.is_empty() || who_is_new {
                let is_sel = kind == LauncherWhere::Home && sel_world.is_empty();
                if ui
                    .selectable_label(is_sel, RichText::new(NEW_HOMESTEAD).color(theme.text_primary()))
                    .clicked()
                {
                    pick_world = Some(String::new());
                }
            }
        });

        // ── Open Net: GREEN (discovery; bring your own character) ──
        ui.add_space(theme.spacing_sm);
        section_card(ui, theme, "Open Net", theme.success(), |ui| {
            hint(
                ui,
                theme,
                "Visit a server with your own character to see what it's about, \
                 no new character needed. Self-custody, like Open Battle.net.",
            );
            if servers.is_empty() {
                hint(ui, theme, "No servers yet. Add one from the Chat sidebar.");
            } else {
                for (id, name, connected) in &servers {
                    let is_sel =
                        kind == LauncherWhere::Server && sel_server.as_deref() == Some(id.as_str());
                    let label = if *connected {
                        format!("{name}  (connected)")
                    } else {
                        name.clone()
                    };
                    if ui
                        .selectable_label(is_sel, RichText::new(label).color(theme.text_primary()))
                        .clicked()
                    {
                        pick_server = Some(id.clone());
                    }
                    if is_sel {
                        // The server's own details, in the card it belongs to.
                        draw_server_info(ui, theme, state, id);
                    }
                }
            }
        });

        // ── Closed Net: BLUE (the committed, server-held story world) ──
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

    if let Some(world) = pick_world {
        state.launcher_where_kind = LauncherWhere::Home;
        state.launcher_selected_world = world;
    }
    if let Some(id) = pick_server {
        state.launcher_where_kind = LauncherWhere::Server;
        state.launcher_selected_server = Some(id);
    }
}

/// The Open Net rows: the LIVE connection you are on right now (virtual,
/// v0.775) first, then your explicitly-saved bookmarks. The virtual row is what
/// the operator was missing -- the app auto-connects to server_url for chat, but
/// only saved chat_servers were listed, so a connected user saw "No servers
/// yet". Deduped against saved servers by url so it never doubles.
/// Returns (row id, display name, connected).
fn server_rows(state: &GuiState) -> Vec<(String, String, bool)> {
    let ws_connected = state.ws_client.as_ref().map_or(false, |c| c.is_connected());
    let primary_url = state.server_url.trim_end_matches('/').to_string();
    let already_saved = state
        .chat_servers
        .iter()
        .any(|s| s.url.trim_end_matches('/') == primary_url);
    let mut rows: Vec<(String, String, bool)> = Vec::new();
    if ws_connected && !primary_url.is_empty() && !already_saved {
        rows.push((
            CONNECTED_SERVER_ID.to_string(),
            crate::gui::pages::chat::server_display_name(&state.server_url),
            true,
        ));
    }
    rows.extend(state.chat_servers.iter().map(|s| {
        // A saved bookmark of the server we're LIVE-connected to counts as
        // connected (v0.779): ChatServer.connected is never maintained by the
        // connection code, so without this URL match, bookmarking your own
        // server made it show "Not connected" and permanently disabled Enter
        // (the working virtual row is deduped away above).
        let live = ws_connected && s.url.trim_end_matches('/') == primary_url;
        (s.id.clone(), s.name.clone(), s.connected || live)
    }));
    rows
}

/// Resolve a WHERE server row to (display name, url, connected). The virtual
/// CONNECTED_SERVER_ID row reads `server_url` directly: it is the LIVE
/// connection, not a saved bookmark. None = the row is gone from your list.
fn resolve_server(state: &GuiState, id: &str) -> Option<(String, String, bool)> {
    let live = state.ws_client.as_ref().map_or(false, |c| c.is_connected());
    if id == CONNECTED_SERVER_ID {
        return Some((
            crate::gui::pages::chat::server_display_name(&state.server_url),
            state.server_url.clone(),
            live,
        ));
    }
    let s = state.chat_servers.iter().find(|s| s.id == id)?;
    let same = is_live_url(state, &s.url);
    Some((s.name.clone(), s.url.clone(), s.connected || (live && same)))
}

/// True when `url` is the connection this client is actually live on. Entering
/// a server world means joining the shared world over THAT socket; switching
/// the live connection to a different saved server from here is the
/// multiplayer-future step.
fn is_live_url(state: &GuiState, url: &str) -> bool {
    !state.server_url.is_empty()
        && url.trim_end_matches('/') == state.server_url.trim_end_matches('/')
}

/// Compose the current selection into the pairing the Enter bar states. The
/// blocked reasons are the first-increment pairing rules from
/// docs/design/play-characters.md, section 3.
fn pairing(state: &GuiState) -> Pairing {
    let who = display_name(&state.character_name);
    match state.launcher_where_kind {
        LauncherWhere::Home => {
            let world = if state.launcher_selected_world.is_empty() {
                NEW_HOMESTEAD.to_string()
            } else {
                state.launcher_selected_world.clone()
            };
            // A character and its home are ONE file today, so the only home a
            // character can enter is its own.
            let blocked = if state.launcher_selected_world == state.launcher_who {
                None
            } else if state.launcher_who.is_empty() {
                Some(
                    "A new character starts in a new home until characters and worlds \
                     are separate files."
                        .to_string(),
                )
            } else {
                Some("Characters cannot move between homes yet.".to_string())
            };
            Pairing { who, world, shared: false, blocked }
        }
        LauncherWhere::Server => {
            let Some(id) = state.launcher_selected_server.as_deref() else {
                return Pairing {
                    who,
                    world: "a server".to_string(),
                    shared: true,
                    blocked: Some("Pick a server in Open Net.".to_string()),
                };
            };
            let Some((name, url, connected)) = resolve_server(state, id) else {
                return Pairing {
                    who,
                    world: "a server".to_string(),
                    shared: true,
                    blocked: Some("That server is no longer in your list.".to_string()),
                };
            };
            let blocked = if connected && is_live_url(state, &url) {
                None
            } else {
                Some("Connect to this server in Chat first.".to_string())
            };
            Pairing { who, world: name, shared: true, blocked }
        }
        LauncherWhere::ClosedNet => Pairing {
            who,
            world: "a closed server".to_string(),
            shared: true,
            blocked: Some("This server holds its own characters; create one here.".to_string()),
        },
    }
}

/// Live metadata for the selected server, fetched once from its
/// /api/server-info (name, description, version, members, online, accord,
/// channels) and shown inline in the Open Net card (v0.478). No Enter button:
/// the bottom bar is the single place a pairing is committed.
fn draw_server_info(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, id: &str) {
    let Some((svr_name, svr_url, svr_connected)) = resolve_server(state, id) else {
        ui.label(RichText::new("That server is no longer in your list.").color(theme.text_muted()));
        return;
    };

    // Kick off a one-time fetch of this server's info if we don't have it.
    // The VIRTUAL row's cache entry is keyed by its URL (v0.779): the sentinel
    // id maps to "whatever server_url is NOW", so a plain sentinel key served
    // the PREVIOUS server's cached info after switching connections.
    let cache_id = if id == CONNECTED_SERVER_ID {
        format!("{CONNECTED_SERVER_ID}:{svr_url}")
    } else {
        id.to_string()
    };
    if !state.server_info_cache.contains_key(&cache_id) {
        fetch_server_info(state, &cache_id, &svr_url);
    }
    let info = state.server_info_cache.get(&cache_id).cloned();

    // Name: the fetched name if we have it, else the locally-known one.
    let title = info
        .as_ref()
        .map(|i| i.name.clone())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| svr_name.clone());
    ui.add_space(theme.spacing_xs);
    ui.label(RichText::new(title).size(theme.font_size_body).strong().color(theme.text_primary()));

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
                ui.label(
                    RichText::new(&i.description)
                        .size(theme.font_size_small)
                        .color(theme.text_secondary()),
                );
                ui.add_space(theme.spacing_xs);
            }
            if !i.version.is_empty() {
                detail_row(ui, theme, "Version", &i.version);
            }
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
    if is_live_url(state, &svr_url) {
        ui.add_space(theme.spacing_xs);
        hint(ui, theme, "Admins: edit this server's description in Server Settings (server cog in Chat).");
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Modes 1/2: the focused editors (wetroom mirror, bedroom wardrobe)
// ─────────────────────────────────────────────────────────────────────────

fn draw_editor(ctx: &Context, theme: &Theme, state: &mut GuiState) {
    let wardrobe = state.showroom_mode == 2;
    let right_title = if wardrobe { "Wardrobe" } else { "Appearance" };
    // "Edit look" from the picker borrows this same editor, so Done has to go
    // back where it came from instead of dropping you into the world.
    let from_picker = state.showroom_return_to_picker;

    egui::SidePanel::left("showroom_select")
        .resizable(false)
        .exact_width(230.0)
        .show(ctx, |ui| {
            ui.add_space(theme.spacing_md);
            ui.label(
                RichText::new("Character")
                    .size(theme.font_size_body)
                    .strong()
                    .color(theme.text_primary()),
            );
            ui.add_space(theme.spacing_sm);
            let name = display_name(&state.character_name);
            let _ = ui.selectable_label(true, RichText::new(name).color(theme.text_primary()));
            ui.add_space(theme.spacing_xs);
            hint(
                ui,
                theme,
                if from_picker {
                    "Editing your look. Done returns to the picker."
                } else {
                    "Editing your look. Close to return to the world."
                },
            );
        });

    egui::SidePanel::right("showroom_details")
        .resizable(false)
        .exact_width(310.0)
        .show(ctx, |ui| {
            ui.add_space(theme.spacing_md);
            ui.label(
                RichText::new(right_title)
                    .size(theme.font_size_body)
                    .strong()
                    .color(theme.text_primary()),
            );
            ui.label(
                RichText::new("Drag the center to orbit. Wheel to zoom.")
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
            );
            ui.add_space(theme.spacing_sm);

            if wardrobe {
                draw_wardrobe(ui, theme, state);
            } else {
                // The GAME character's name (separate from your chat profile).
                // The picker has its own inline Name field; this one serves the
                // in-world mirror, where there is no WHO column.
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Name").color(theme.text_secondary()));
                    ui.text_edit_singleline(&mut state.character_name);
                });
                ui.add_space(theme.spacing_sm);
                draw_appearance(ui, theme, state);
            }

            ui.add_space(theme.spacing_sm);
            // Backdrop lives here, not in the picker: it is stage dressing for
            // editing, not part of the launch flow.
            draw_backdrop(ui, theme, state);
            ui.add_space(theme.spacing_md);

            if ui
                .button(RichText::new("Done").size(theme.font_size_body).strong())
                .clicked()
            {
                if from_picker {
                    state.showroom_return_to_picker = false;
                    state.showroom_mode = 0;
                } else {
                    state.showroom_confirm = true;
                }
            }
        });
}

// ─────────────────────────────────────────────────────────────────────────
// Shared pieces
// ─────────────────────────────────────────────────────────────────────────

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

/// A COLOR-CODED picker section card (v0.784, operator's RGB scheme):
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

/// A "Label: value" detail line for the inline row summaries.
fn detail_row(ui: &mut egui::Ui, theme: &Theme, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{label}:")).size(theme.font_size_small).color(theme.text_secondary()));
        ui.label(RichText::new(value).size(theme.font_size_small).color(theme.text_primary()));
    });
}

/// A character's display name, with the same fallback the confirm handler uses
/// so the Enter sentence never disagrees with who you end up as.
fn display_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        "Wanderer".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Coarse "last played" for a save timestamp (unix seconds). Coarse on purpose:
/// the row is a reminder of which home this is, not a log.
fn last_played(timestamp: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if timestamp == 0 || now <= timestamp {
        return "just now".to_string();
    }
    let secs = now - timestamp;
    if secs < 3600 {
        format!("{} min ago", (secs / 60).max(1))
    } else if secs < 86_400 {
        format!("{} h ago", secs / 3600)
    } else {
        format!("{} days ago", secs / 86_400)
    }
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
    // Reached from the picker there is no bedroom to walk to, so say where
    // outfits actually live.
    if state.showroom_return_to_picker {
        ui.label(
            RichText::new("Outfits: change them at the bedroom wardrobe.")
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
    }
}

fn draw_wardrobe(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Wardrobe").strong().color(theme.text_primary()));
    let slot_ids: Vec<String> = state.equipment_slots.iter().map(|(id, _)| id.clone()).collect();
    for slot in slot_ids.iter().map(|s| s.as_str()) {
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
