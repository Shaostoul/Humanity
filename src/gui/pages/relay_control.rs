//! Relay Control Center (v0.846) - one main-menu entry to manage every relay
//! the operator owns from one PC. A left rail lists the operator's relays; the
//! detail pane has three tabs:
//!
//! - **Health** - the rich signed `GET /api/admin/stats` snapshot (version,
//!   uptime, users, messages, DB + upload size, disk usage, watchdog state,
//!   backup age). This is the data an operator used to SSH for; native never
//!   fetched it before this page.
//! - **Control** - watchdog state at a glance, plus restart/logs actions. Where
//!   no in-app endpoint exists yet, the button shows the honest CLI fallback
//!   instead of pretending (the GUI-first rule: surface the gap, never hide it).
//! - **Config** - a jump into the existing Server Settings editors (roles,
//!   channels, policy, announcements, federation) for the selected relay.
//!
//! This elevates the Server Settings ops panels into a top-level, multi-relay
//! surface. See docs/design/in-app-ops.md "Relay Control Center".

use egui::{Align, Color32, Frame, Layout, RichText, Rounding, Stroke, Vec2};

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::{GuiPage, GuiState, RelayAdminStats};

/// One relay in the left rail.
struct RelayRow {
    name: String,
    url: String,
    /// Whether the app currently holds an open WS connection to this relay.
    connected: bool,
}

/// Normalize a relay URL for de-duplication + display (drop a trailing slash).
fn norm_url(u: &str) -> String {
    u.trim().trim_end_matches('/').to_string()
}

/// Build the relay rail from the saved servers + the currently-connected relay.
/// The connected relay is guaranteed present even if it isn't in `chat_servers`
/// yet (fresh session, before the server list is populated).
fn collect_relays(state: &GuiState) -> Vec<RelayRow> {
    let ws_connected = state.ws_client.as_ref().map_or(false, |c| c.is_connected());
    let connected_url = norm_url(&state.server_url);

    let mut rows: Vec<RelayRow> = Vec::new();
    for s in &state.chat_servers {
        let url = norm_url(&s.url);
        if url.is_empty() { continue; }
        let is_conn = s.connected || (url == connected_url && ws_connected);
        rows.push(RelayRow { name: s.name.clone(), url, connected: is_conn });
    }
    // Ensure the connected relay is represented.
    if !connected_url.is_empty() && !rows.iter().any(|r| r.url == connected_url) {
        rows.insert(0, RelayRow {
            name: crate::gui::pages::chat::server_display_name(&connected_url),
            url: connected_url.clone(),
            connected: ws_connected,
        });
    }
    // De-dupe by URL, keeping the first (connected-preferred) entry.
    let mut seen = std::collections::HashSet::new();
    rows.retain(|r| seen.insert(r.url.clone()));
    rows
}

/// "3d 4h 12m" / "5m 12s" style humanized duration.
fn fmt_duration(secs: u64) -> String {
    let d = secs / 86_400;
    let h = (secs % 86_400) / 3_600;
    let m = (secs % 3_600) / 60;
    if d > 0 { format!("{d}d {h}h {m}m") }
    else if h > 0 { format!("{h}h {m}m") }
    else { format!("{m}m {}s", secs % 60) }
}

/// Human byte sizes: 1.4 GB / 812 MB / 47 KB.
fn fmt_bytes(n: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let f = n as f64;
    if f >= GB { format!("{:.1} GB", f / GB) }
    else if f >= MB { format!("{:.0} MB", f / MB) }
    else if f >= KB { format!("{:.0} KB", f / KB) }
    else { format!("{n} B") }
}

/// Kick off a background fetch of the SIGNED /api/admin/stats for `base`.
/// The Dilithium3 signature is computed on the worker thread (CPU-bound), so
/// the UI never blocks. Requires the operator's seed + public key (admin role
/// on the target relay); otherwise the relay returns 401/403 and we surface it.
fn spawn_admin_stats_fetch(state: &mut GuiState, base: &str) {
    let base = norm_url(base);
    let key = state.profile_public_key.clone();
    let seed = state.private_key_bytes.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    state.relay_admin_stats_rx = Some(rx);
    state.relay_admin_stats_status = "Loading…".to_string();

    std::thread::spawn(move || {
        let fetch = || -> Result<RelayAdminStats, String> {
            let seed = seed.ok_or_else(|| "No identity loaded - sign in first.".to_string())?;
            if key.is_empty() {
                return Err("No identity loaded - sign in first.".to_string());
            }
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| format!("clock error: {e}"))?
                .as_millis() as u64;
            // Sign the exact preimage the relay verifies: "admin_stats\n{ts}".
            let sig = crate::net::identity::pq_sign_chat(&seed, "admin_stats", ts);

            // POST, not GET: the Dilithium key (3904 hex) + signature (~6600 hex)
            // are ~10 KB, which as query params made the URL exceed nginx's default
            // header buffer and came back as HTTP 414 URI Too Long. The body has no
            // such limit. (v0.851)
            let body = serde_json::json!({
                "key": key,
                "timestamp": ts,
                "sig": sig,
            })
            .to_string();
            let resp = ureq::post(&format!("{base}/api/admin/stats"))
                .set("Content-Type", "application/json")
                .send_string(&body);
            let body = match resp {
                Ok(r) => r.into_string().map_err(|e| format!("read: {e}"))?,
                Err(ureq::Error::Status(403, _)) =>
                    return Err("You are not an admin on this relay.".to_string()),
                Err(ureq::Error::Status(401, _)) =>
                    return Err("Signature rejected - is your identity registered on this relay?".to_string()),
                Err(ureq::Error::Status(code, _)) =>
                    return Err(format!("Relay returned HTTP {code}.")),
                Err(e) => return Err(format!("Unreachable: {e}")),
            };
            let v: serde_json::Value = serde_json::from_str(&body)
                .map_err(|e| format!("parse: {e}"))?;
            let sys = &v["system"];
            Ok(RelayAdminStats {
                user_count: v["user_count"].as_u64().unwrap_or(0),
                online_count: v["online_count"].as_u64().unwrap_or(0),
                total_messages: v["total_messages"].as_u64().unwrap_or(0),
                message_count_24h: v["message_count_24h"].as_u64().unwrap_or(0),
                db_size_bytes: v["db_size_bytes"].as_u64().unwrap_or(0),
                upload_size_bytes: v["upload_size_bytes"].as_u64().unwrap_or(0),
                uptime_seconds: v["uptime_seconds"].as_u64().unwrap_or(0),
                version: sys["version"].as_str().unwrap_or("unknown").to_string(),
                watchdog_state: sys["watchdog_state"].as_str().unwrap_or("unknown").to_string(),
                disk_used_pct: sys["disk"]["used_pct"].as_u64().map(|x| x as u32),
                disk_total_bytes: sys["disk"]["total_bytes"].as_u64(),
                disk_avail_bytes: sys["disk"]["avail_bytes"].as_u64(),
                backup_age_secs: sys["backup"]["newest_age_secs"].as_u64(),
                backup_count: sys["backup"]["count"].as_u64(),
            })
        };
        let _ = tx.send(fetch());
    });
}

/// Drain a finished admin-stats fetch into state (per-frame, non-blocking).
fn drain_admin_stats(ui: &mut egui::Ui, state: &mut GuiState) {
    if let Some(rx) = &state.relay_admin_stats_rx {
        match rx.try_recv() {
            Ok(Ok(stats)) => {
                state.relay_admin_stats = Some(stats);
                state.relay_admin_stats_status.clear();
                state.relay_admin_stats_rx = None;
            }
            Ok(Err(e)) => {
                state.relay_admin_stats = None;
                state.relay_admin_stats_status = e;
                state.relay_admin_stats_rx = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(300));
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                state.relay_admin_stats_status = "Fetch thread died.".to_string();
                state.relay_admin_stats_rx = None;
            }
        }
    }
}

/// Small colored status dot, painted inline.
fn status_dot(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 5.0, color);
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // Default the focused relay to the connected server on first open.
    if state.relay_cc_selected.is_none() {
        let u = norm_url(&state.server_url);
        if !u.is_empty() {
            state.relay_cc_selected = Some(u);
        }
    }

    let relays = collect_relays(state);
    // If the selected relay vanished (server list changed), fall back to the first.
    if let Some(sel) = state.relay_cc_selected.clone() {
        if !relays.iter().any(|r| r.url == sel) {
            state.relay_cc_selected = relays.first().map(|r| r.url.clone());
        }
    } else {
        state.relay_cc_selected = relays.first().map(|r| r.url.clone());
    }

    // ── Left rail: the operator's relays ──
    egui::SidePanel::left("relay_cc_rail")
        .resizable(false)
        .min_width(190.0)
        .max_width(230.0)
        .frame(Frame::none().fill(theme.bg_sidebar()).inner_margin(12.0))
        .show(ctx, |ui| {
            ui.label(RichText::new("MY RELAYS")
                .size(theme.font_size_small)
                .color(theme.text_muted())
                .strong());
            ui.add_space(theme.spacing_sm);

            if relays.is_empty() {
                ui.label(RichText::new("No relays yet. Connect to one from Chat.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()));
            }

            let selected = state.relay_cc_selected.clone();
            for r in &relays {
                let is_sel = selected.as_deref() == Some(r.url.as_str());
                let bg = if is_sel { theme.bg_tertiary() } else { Color32::TRANSPARENT };
                let resp = Frame::none()
                    .fill(bg)
                    .rounding(Rounding::same(theme.border_radius as u8))
                    .inner_margin(theme.spacing_sm)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            status_dot(ui, if r.connected { theme.success() } else { theme.text_muted() });
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&r.name)
                                    .size(theme.font_size_body)
                                    .color(theme.text_primary()));
                                ui.label(RichText::new(r.url.trim_start_matches("https://").trim_start_matches("http://"))
                                    .size(theme.font_size_small)
                                    .color(theme.text_muted()));
                            });
                        });
                    }).response;
                // Make the whole row clickable to select.
                let resp = resp.interact(egui::Sense::click());
                if resp.clicked() {
                    state.relay_cc_selected = Some(r.url.clone());
                    // A new selection invalidates the cached stats.
                    state.relay_admin_stats = None;
                    state.relay_admin_stats_status.clear();
                }
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                ui.add_space(4.0);
            }

            ui.add_space(theme.spacing_md);
            if widgets::Button::secondary("+ Add relay")
                .full_width()
                .tooltip("Connect to another relay from the Chat server sidebar; it appears here.")
                .show(ui, theme)
            {
                // The add-server modal lives on the Chat page; open it there.
                state.show_add_server_modal = true;
                state.active_page = GuiPage::Chat;
            }
        });

    // ── Detail: header + tabs for the selected relay ──
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
        .show(ctx, |ui| {
            let sel_url = state.relay_cc_selected.clone().unwrap_or_default();
            if sel_url.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(60.0);
                    ui.label(RichText::new("No relay selected.")
                        .size(theme.font_size_heading)
                        .color(theme.text_muted()));
                    ui.label(RichText::new("Connect to a relay from Chat to manage it here.")
                        .size(theme.font_size_body)
                        .color(theme.text_muted()));
                });
                return;
            }
            let sel = relays.iter().find(|r| r.url == sel_url);
            let sel_name = sel.map(|r| r.name.clone())
                .unwrap_or_else(|| crate::gui::pages::chat::server_display_name(&sel_url));
            let sel_connected = sel.map(|r| r.connected).unwrap_or(false);

            // Header
            ui.horizontal(|ui| {
                status_dot(ui, if sel_connected { theme.success() } else { theme.text_muted() });
                ui.label(RichText::new(&sel_name)
                    .size(theme.font_size_title)
                    .color(theme.text_primary())
                    .strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(if sel_connected { "connected" } else { "offline" })
                        .size(theme.font_size_small)
                        .color(if sel_connected { theme.success() } else { theme.text_muted() }));
                });
            });
            ui.label(RichText::new(&sel_url)
                .size(theme.font_size_small)
                .color(theme.text_muted()));
            ui.add_space(theme.spacing_md);

            // One continuous scroll (Health / Control / Config stacked) instead of
            // tabs (operator 2026-07-13: "just have infinite scroll like the
            // settings menu"). Each section keeps its own heading.
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                drain_admin_stats(ui, state);

                let heading = |ui: &mut egui::Ui, t: &str| {
                    ui.label(RichText::new(t).size(theme.font_size_heading).color(theme.text_primary()).strong());
                    ui.add_space(theme.spacing_xs);
                };
                let divider = |ui: &mut egui::Ui| {
                    ui.add_space(theme.spacing_xl);
                    ui.separator();
                    ui.add_space(theme.spacing_md);
                };

                heading(ui, "Health");
                draw_health_tab(ui, theme, state, &sel_url, sel_connected);
                divider(ui);
                heading(ui, "Control");
                draw_control_tab(ui, theme, state, &sel_url);
                divider(ui);
                heading(ui, "Console");
                draw_console_section(ui, theme, state);
                divider(ui);
                heading(ui, "Config");
                draw_config_tab(ui, theme, state);
            });
        });
}

fn draw_health_tab(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, sel_url: &str, connected: bool) {
    // Auto-fetch once when this relay is connected and we have nothing yet.
    if connected
        && state.relay_admin_stats.is_none()
        && state.relay_admin_stats_rx.is_none()
        && state.relay_admin_stats_status.is_empty()
    {
        spawn_admin_stats_fetch(state, sel_url);
    }

    widgets::body_hint(ui, theme,
        "Live admin snapshot from this relay's signed /api/admin/stats - the \
         numbers you used to SSH for. Only visible to an admin of the relay.");
    ui.add_space(theme.spacing_sm);

    if let Some(s) = state.relay_admin_stats.clone() {
        egui::Grid::new("relay_health_grid")
            .num_columns(2)
            .spacing([theme.spacing_xl, theme.spacing_xs])
            .show(ui, |ui| {
                let mut row = |ui: &mut egui::Ui, label: &str, value: String, color: Color32| {
                    ui.label(RichText::new(label).size(theme.font_size_small).color(theme.text_secondary()));
                    ui.label(RichText::new(value).size(theme.font_size_body).color(color));
                    ui.end_row();
                };
                row(ui, "Deployed build", s.version.clone(), theme.text_primary());
                row(ui, "Uptime", fmt_duration(s.uptime_seconds), theme.text_primary());
                row(ui, "Users", format!("{} total · {} online", s.user_count, s.online_count), theme.text_primary());
                row(ui, "Messages", format!("{} total · {} in 24h", s.total_messages, s.message_count_24h), theme.text_primary());
                row(ui, "Database", fmt_bytes(s.db_size_bytes), theme.text_primary());
                row(ui, "Uploads", fmt_bytes(s.upload_size_bytes), theme.text_primary());
                if let Some(pct) = s.disk_used_pct {
                    let dc = if pct >= 90 { theme.danger() } else if pct >= 75 { theme.warning() } else { theme.text_primary() };
                    let detail = match (s.disk_avail_bytes, s.disk_total_bytes) {
                        (Some(a), Some(t)) => format!("{pct}% used · {} free of {}", fmt_bytes(a), fmt_bytes(t)),
                        _ => format!("{pct}% used"),
                    };
                    row(ui, "Disk", detail, dc);
                }
                let (wtxt, wcolor) = watchdog_display(theme, &s.watchdog_state);
                row(ui, "Watchdog", wtxt, wcolor);
                if let Some(age) = s.backup_age_secs {
                    let cnt = s.backup_count.unwrap_or(0);
                    row(ui, "Last backup", format!("{} ago · {} kept", fmt_duration(age), cnt), theme.text_primary());
                }
            });
        ui.add_space(theme.spacing_sm);
    }

    if !state.relay_admin_stats_status.is_empty() {
        ui.label(RichText::new(&state.relay_admin_stats_status)
            .size(theme.font_size_small)
            .color(theme.text_muted()));
        ui.add_space(theme.spacing_sm);
    }

    if widgets::Button::secondary("Refresh")
        .tooltip("Re-poll this relay's signed admin stats now.")
        .show(ui, theme)
    {
        spawn_admin_stats_fetch(state, sel_url);
    }
}

/// Map a watchdog state string to a label + color.
fn watchdog_display(theme: &Theme, s: &str) -> (String, Color32) {
    match s {
        "up" => ("Up".to_string(), theme.success()),
        "suspect" => ("Suspect (probing)".to_string(), theme.warning()),
        "healing" => ("Healing (restarting)".to_string(), theme.warning()),
        "down-critical" => ("Down (critical)".to_string(), theme.danger()),
        _ => ("Unknown".to_string(), theme.text_muted()),
    }
}

fn draw_control_tab(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, _sel_url: &str) {
    widgets::body_hint(ui, theme,
        "Lifecycle control for this relay. The watchdog auto-restarts a dead \
         relay; the actions below cover the rest.");
    ui.add_space(theme.spacing_sm);

    // Watchdog state chip (from the last Health fetch).
    if let Some(s) = state.relay_admin_stats.clone() {
        let (wtxt, wcolor) = watchdog_display(theme, &s.watchdog_state);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Watchdog:").size(theme.font_size_body).color(theme.text_secondary()));
            status_dot(ui, wcolor);
            ui.label(RichText::new(wtxt).size(theme.font_size_body).color(wcolor));
        });
        if let Some(age) = s.backup_age_secs {
            ui.label(RichText::new(format!("Newest backup {} ago.", fmt_duration(age)))
                .size(theme.font_size_small).color(theme.text_muted()));
        }
    } else {
        ui.label(RichText::new("Open the Health tab first to load the watchdog state.")
            .size(theme.font_size_small).color(theme.text_muted()));
    }
    ui.add_space(theme.spacing_md);

    // Restart + Logs: these now RUN over SSH (v0.858) via the Console below, so the
    // operator never has to leave the app for a second terminal. The buttons queue
    // the command into the Console section, where the output appears.
    ui.horizontal(|ui| {
        if widgets::Button::secondary("Restart relay")
            .tooltip("Runs `sudo systemctl restart humanity-relay` on the server over SSH. \
                      Output appears in the Console below.")
            .disabled(state.vps_console_running)
            .show(ui, theme)
        {
            run_vps_command(state, "Restart relay", "sudo systemctl restart humanity-relay");
        }
        if widgets::Button::secondary("Tail logs")
            .tooltip("Runs `journalctl -u humanity-relay -n 100 --no-pager` on the server. \
                      Output appears in the Console below.")
            .disabled(state.vps_console_running)
            .show(ui, theme)
        {
            run_vps_command(state, "Tail logs", "journalctl -u humanity-relay -n 100 --no-pager");
        }
    });
    ui.add_space(theme.spacing_md);

    ui.label(RichText::new("Auxiliary services")
        .size(theme.font_size_small).color(theme.text_secondary()).strong());
    ui.add_space(theme.spacing_xs);
    ui.label(RichText::new("Start/stop the TURN (voice) and torrent services under Server Settings, Services.")
        .size(theme.font_size_small).color(theme.text_muted()));
    ui.add_space(theme.spacing_xs);
    if widgets::Button::secondary("Open Server Settings").show(ui, theme) {
        state.active_page = GuiPage::ServerSettings;
    }
}

/// The in-app server console (v0.858). Runs commands on the VPS over SSH straight
/// from the app, so administering the server is not "go open another program" — the
/// all-in-one principle (docs/design/in-app-ops.md). It shells to the system `ssh`
/// with the `humanity-vps` host alias, which uses the operator's existing key + config;
/// on a machine without that key the command simply fails with an SSH error, so the
/// box is naturally operator-gated by key possession.
fn draw_console_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    drain_vps_console(state);

    widgets::body_hint(ui, theme,
        "Run commands on the server over SSH, without leaving the app. Uses your saved \
         SSH key for the 'humanity-vps' host, so this only works on a machine that has it. \
         Commands run as configured (root), so double-check before you run anything destructive.");
    ui.add_space(theme.spacing_sm);

    // One-click common operations. Each is a plain, readable command so there is no
    // hidden behavior, and the operator can see exactly what runs.
    ui.label(RichText::new("Common tasks").size(theme.font_size_small).color(theme.text_secondary()).strong());
    ui.add_space(theme.spacing_xs);
    let quick: [(&str, &str); 5] = [
        ("Relay status", "systemctl status humanity-relay --no-pager -n 5"),
        ("Disk usage", "df -h /"),
        ("Memory", "free -h"),
        ("nginx test", "nginx -t"),
        ("Uptime", "uptime"),
    ];
    ui.horizontal_wrapped(|ui| {
        for (label, cmd) in quick {
            if widgets::Button::ghost(label)
                .tooltip(cmd)
                .disabled(state.vps_console_running)
                .show(ui, theme)
            {
                run_vps_command(state, label, cmd);
            }
        }
    });
    ui.add_space(theme.spacing_md);

    // Free command entry. Enter or Run submits.
    ui.label(RichText::new("Run a command").size(theme.font_size_small).color(theme.text_secondary()).strong());
    ui.add_space(theme.spacing_xs);
    ui.horizontal(|ui| {
        let resp = ui.add(
            egui::TextEdit::singleline(&mut state.vps_console_input)
                .desired_width(ui.available_width() - 90.0)
                .hint_text("e.g. ls /opt/Humanity"),
        );
        let submit = (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
            || widgets::Button::primary("Run").disabled(state.vps_console_running).show(ui, theme);
        if submit && !state.vps_console_running {
            let cmd = state.vps_console_input.trim().to_string();
            if !cmd.is_empty() {
                state.vps_console_input.clear();
                run_vps_command(state, &cmd.clone(), &cmd);
            }
        }
    });
    ui.add_space(theme.spacing_sm);

    if state.vps_console_running {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(RichText::new("Running...").size(theme.font_size_small).color(theme.text_muted()));
        });
        ui.ctx().request_repaint();
        ui.add_space(theme.spacing_xs);
    }

    // Output transcript. Monospace, scrollable, newest at the bottom.
    if !state.vps_console_output.is_empty() {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Output").size(theme.font_size_small).color(theme.text_secondary()).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::Button::ghost("Clear").show(ui, theme) {
                    state.vps_console_output.clear();
                }
                if widgets::Button::ghost("Copy").show(ui, theme) {
                    ui.ctx().copy_text(state.vps_console_output.clone());
                }
            });
        });
        ui.add_space(theme.spacing_xs);
        Frame::none()
            .fill(theme.bg_secondary())
            .rounding(Rounding::same(theme.border_radius as u8))
            .inner_margin(theme.spacing_sm)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(&state.vps_console_output)
                                    .monospace()
                                    .size(theme.font_size_small)
                                    .color(theme.text_secondary()),
                            )
                            .wrap(),
                        );
                    });
            });
    }
}

/// Spawn `ssh humanity-vps <command>` on a worker thread and stream the result back.
/// Never blocks the UI. `BatchMode=yes` means it fails fast instead of hanging on a
/// password prompt; `ConnectTimeout` bounds a dead host.
fn run_vps_command(state: &mut GuiState, label: &str, command: &str) {
    // Echo the command into the transcript immediately so the operator sees what ran.
    state.vps_console_output.push_str(&format!("$ {command}\n"));
    state.vps_console_running = true;

    let (tx, rx) = std::sync::mpsc::channel();
    state.vps_console_rx = Some(rx);

    let label = label.to_string();
    let command = command.to_string();
    std::thread::spawn(move || {
        let result = std::process::Command::new("ssh")
            .arg("-o")
            .arg("BatchMode=yes")
            .arg("-o")
            .arg("ConnectTimeout=10")
            .arg("humanity-vps")
            .arg(&command)
            .output();
        let (out, ok) = match result {
            Ok(o) => {
                let mut s = String::from_utf8_lossy(&o.stdout).into_owned();
                let err = String::from_utf8_lossy(&o.stderr);
                if !err.trim().is_empty() {
                    s.push_str(&err);
                }
                if s.trim().is_empty() {
                    s = "(no output)\n".to_string();
                }
                (s, o.status.success())
            }
            Err(e) => (
                format!(
                    "Could not run ssh: {e}\nThis machine may not have the 'humanity-vps' SSH \
                     key/host configured.\n"
                ),
                false,
            ),
        };
        let _ = tx.send((label, out, ok));
    });
}

/// Drain a finished command into the transcript.
fn drain_vps_console(state: &mut GuiState) {
    if let Some(rx) = &state.vps_console_rx {
        if let Ok((_label, out, ok)) = rx.try_recv() {
            state.vps_console_output.push_str(&out);
            if !out.ends_with('\n') {
                state.vps_console_output.push('\n');
            }
            if !ok {
                state.vps_console_output.push_str("[command exited non-zero]\n");
            }
            state.vps_console_output.push('\n');
            state.vps_console_rx = None;
            state.vps_console_running = false;
        }
    }
}

fn draw_config_tab(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::body_hint(ui, theme,
        "Change how this relay behaves - no terminal required. These open the \
         existing Server Settings editors for the connected relay.");
    ui.add_space(theme.spacing_sm);

    let items = [
        ("Roles & permissions", "Who can stream, upload, moderate; create custom roles."),
        ("Channels", "Create, rename, and remove channels."),
        ("Server policy & limits", "Registration mode, per-role limits, image/file sharing."),
        ("Announcements", "Post to #announcements; pin server-wide notices."),
        ("Federation", "Add, trust, and defederate peer relays."),
    ];
    for (title, desc) in items {
        Frame::none()
            .fill(theme.bg_card())
            .stroke(Stroke::new(1.0, theme.border()))
            .rounding(Rounding::same(theme.border_radius as u8))
            .inner_margin(theme.spacing_md)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(title).size(theme.font_size_body).color(theme.text_primary()));
                        ui.label(RichText::new(desc).size(theme.font_size_small).color(theme.text_muted()));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if widgets::Button::secondary("Open").show(ui, theme) {
                            state.active_page = GuiPage::ServerSettings;
                            state.server_settings_tab = 0;
                        }
                    });
                });
            });
        ui.add_space(theme.spacing_sm);
    }
}
