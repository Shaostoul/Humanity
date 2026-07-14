//! Bug Reporter page - file a bug against the connected relay and read what
//! everyone else has already filed.
//!
//! v0.851: this page LOOKED finished but persisted nothing. "Submit Report"
//! pushed a `BugReport` into an in-session `Vec` that died with the process, and
//! the list underneath was seeded with two invented example bugs, so a user's
//! report went nowhere and no maintainer ever saw it. The relay has had the full
//! path all along and nothing was calling it:
//!
//!   POST /api/bugs  -> `relay::api::create_bug`  -> `Storage::create_bug`  -> table `bug_reports`
//!   GET  /api/bugs  -> `relay::api::get_bugs`    -> `Storage::get_bugs`    -> table `bug_reports`
//!
//! Both are wired now: submit POSTs and only clears the form once the server
//! hands back an id, and the list is whatever the server actually holds (which
//! is why it survives a restart). Fetch/submit run on worker threads with the
//! mpsc + `try_recv` pattern the Files page uses, so the UI never blocks.

use egui::{Color32, Frame, RichText, ScrollArea};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;

// Severity and category lists are loaded from `data/bugs/taxonomy.json` into
// `GuiState.bug_severities` / `GuiState.bug_categories` at startup.

/// A bug report exactly as the relay stores it (the shape GET /api/bugs
/// returns). Never a local echo of what we just typed: after a successful
/// submit we re-fetch, so what is on screen is what is on the server.
#[derive(Debug, Clone)]
pub struct BugReport {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub severity: String,
    pub category: String,
    pub version: String,
    pub status: String,
    pub votes: i64,
    pub reporter_name: String,
}

type ListResult = Result<Vec<BugReport>, String>;
type SubmitResult = Result<i64, String>;

/// Local state for the bug reporter page.
pub struct BugReporterState {
    pub title: String,
    pub description: String,
    pub severity_idx: usize,
    pub category_idx: usize,
    /// The server's reports (empty until the first fetch lands).
    pub reports: Vec<BugReport>,
    pub status_message: String,
    /// Pull the list once on first view.
    pub loaded: bool,
    /// In-flight GET /api/bugs.
    pub list_rx: Option<std::sync::mpsc::Receiver<ListResult>>,
    /// In-flight POST /api/bugs; on success we re-fetch the list.
    pub submit_rx: Option<std::sync::mpsc::Receiver<SubmitResult>>,
}

impl Default for BugReporterState {
    fn default() -> Self {
        Self {
            title: String::new(),
            description: String::new(),
            severity_idx: 0,
            category_idx: 0,
            reports: Vec::new(),
            status_message: String::new(),
            loaded: false,
            list_rx: None,
            submit_rx: None,
        }
    }
}

fn with_state<R>(f: impl FnOnce(&mut BugReporterState) -> R) -> R {
    use std::cell::RefCell;
    thread_local! {
        static STATE: RefCell<BugReporterState> = RefCell::new(BugReporterState::default());
    }
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// Blocking GET /api/bugs. Runs on a worker thread at the call site.
fn fetch_bugs_blocking(server_url: &str) -> ListResult {
    let base = server_url.trim_end_matches('/');
    let body = ureq::get(&format!("{base}/api/bugs?limit=50"))
        .call()
        .map_err(|e| format!("Could not load reports: {e}"))?
        .into_string()
        .map_err(|e| format!("read: {e}"))?;
    let val: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("parse: {e}"))?;
    let arr = val
        .get("bugs")
        .and_then(|b| b.as_array())
        .cloned()
        .unwrap_or_default();
    let rows = arr
        .into_iter()
        .map(|b| BugReport {
            id: b.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
            title: b.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            description: b.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            severity: b.get("severity").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            category: b.get("category").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            version: b.get("version").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            status: b.get("status").and_then(|v| v.as_str()).unwrap_or("open").to_string(),
            votes: b.get("votes").and_then(|v| v.as_i64()).unwrap_or(0),
            reporter_name: b.get("reporter_name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        })
        .collect();
    Ok(rows)
}

/// The severity string the relay accepts. Its whitelist is lowercase
/// (critical/high/medium/low/cosmetic) and `data/bugs/taxonomy.json` is
/// title-case, so every native label maps cleanly once lowercased.
fn relay_severity(label: &str) -> String {
    label.trim().to_lowercase()
}

/// The category string the relay accepts.
///
/// KNOWN MISMATCH (not fixable from this page): the relay whitelists
/// chat/tasks/market/wallet/maps/game/settings/other (`relay::api::create_bug`),
/// while `data/bugs/taxonomy.json` offers UI/Gameplay/Network/Performance/Crash/
/// Other. Anything the relay does not recognise it silently stores as "other",
/// so only Gameplay (-> game) and Other survive the trip. The report is still
/// persisted with its real title, description, severity and version; only the
/// category coarsens. Aligning the two lists is a one-line edit to either the
/// taxonomy data file or the relay whitelist, and needs whoever owns those.
fn relay_category(label: &str) -> String {
    match label.trim().to_lowercase().as_str() {
        "gameplay" | "game" => "game".to_string(),
        other => other.to_string(),
    }
}

/// Blocking POST /api/bugs. Returns the new report's server-assigned id.
///
/// The route is unauthenticated by design on the relay side (no signature is
/// verified; `reporter_key` is whatever the client claims), so unlike the Files
/// page this sends no `pq_sign_chat` signature: adding one would be theatre,
/// since `create_bug` never checks it. We do send our real public key and name
/// so a maintainer can follow up.
#[allow(clippy::too_many_arguments)]
fn submit_bug_blocking(
    server_url: &str,
    title: &str,
    description: &str,
    severity: &str,
    category: &str,
    reporter_key: &str,
    reporter_name: &str,
) -> SubmitResult {
    let base = server_url.trim_end_matches('/');
    let body = serde_json::json!({
        "title": title,
        "description": description,
        "severity": relay_severity(severity),
        "category": relay_category(category),
        "reporter_key": reporter_key,
        "reporter_name": reporter_name,
        // Provenance: which build and which client the report came from. The
        // relay's `browser_info` / `page_url` columns are free-form strings.
        "browser_info": format!(
            "HumanityOS native v{} ({})",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS
        ),
        "page_url": "native/bugs",
        "version": env!("CARGO_PKG_VERSION"),
    });
    let resp = ureq::post(&format!("{base}/api/bugs"))
        .set("Content-Type", "application/json")
        .send_string(&body.to_string());
    match resp {
        Ok(r) => {
            let text = r.into_string().unwrap_or_default();
            let val: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
            Ok(val.get("id").and_then(|v| v.as_i64()).unwrap_or(0))
        }
        Err(ureq::Error::Status(code, r)) => {
            let msg = r.into_string().unwrap_or_default();
            Err(format!("Submit failed ({code}): {msg}"))
        }
        Err(e) => Err(format!("Submit failed: {e}")),
    }
}

/// Kick off a list fetch on a worker thread.
fn spawn_list_fetch(bs: &mut BugReporterState, server_url: &str) {
    let (tx, rx) = std::sync::mpsc::channel();
    let server = server_url.to_string();
    std::thread::spawn(move || {
        let _ = tx.send(fetch_bugs_blocking(&server));
    });
    bs.list_rx = Some(rx);
    bs.loaded = true;
}

/// The relay's statuses are snake_case (open, in_progress, fixed, wont_fix,
/// duplicate); show them the way a human writes them.
fn status_label(status: &str) -> String {
    match status {
        "open" => "Open".to_string(),
        "in_progress" => "In Progress".to_string(),
        "fixed" => "Fixed".to_string(),
        "wont_fix" => "Won't Fix".to_string(),
        "duplicate" => "Duplicate".to_string(),
        other if other.is_empty() => "Open".to_string(),
        other => other.to_string(),
    }
}

fn status_color(status: &str, theme: &Theme) -> Color32 {
    match status {
        "open" => theme.warning(),
        "in_progress" => Theme::c32(&theme.info),
        "fixed" => theme.success(),
        "wont_fix" | "duplicate" => theme.text_muted(),
        _ => theme.text_secondary(),
    }
}

fn severity_label(severity: &str) -> String {
    let mut chars = severity.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn severity_color(severity: &str, theme: &Theme) -> Color32 {
    match severity {
        "critical" => theme.danger(),
        "high" => Theme::c32(&theme.badge_live),
        "medium" => theme.warning(),
        "low" | "cosmetic" => theme.text_secondary(),
        _ => theme.text_muted(),
    }
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let server = state.server_url.clone();
    let my_key = state.profile_public_key.clone();
    let my_name = if !state.profile_name.trim().is_empty() {
        state.profile_name.clone()
    } else {
        state.user_name.clone()
    };

    // Drain any finished network work before drawing, so the frame shows the
    // freshest truth we have.
    with_state(|bs| {
        if let Some(rx) = &bs.list_rx {
            match rx.try_recv() {
                Ok(Ok(rows)) => {
                    bs.reports = rows;
                    bs.list_rx = None;
                }
                Ok(Err(e)) => {
                    bs.status_message = e;
                    bs.list_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(300));
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    bs.list_rx = None;
                }
            }
        }
        if let Some(rx) = &bs.submit_rx {
            match rx.try_recv() {
                Ok(Ok(id)) => {
                    bs.submit_rx = None;
                    // Only NOW is the form safe to clear: the server has the
                    // report. A failed submit keeps everything the user typed.
                    bs.title.clear();
                    bs.description.clear();
                    bs.severity_idx = 0;
                    bs.category_idx = 0;
                    bs.status_message = format!("Report saved on the server (#{id}). Thank you.");
                    spawn_list_fetch(bs, &server);
                }
                Ok(Err(e)) => {
                    bs.status_message = e;
                    bs.submit_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(300));
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    bs.submit_rx = None;
                }
            }
        }
        if !bs.loaded && bs.list_rx.is_none() {
            spawn_list_fetch(bs, &server);
        }
    });

    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Report a Bug")
                        .size(theme.font_size_title)
                        .color(theme.text_primary()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::secondary_button(ui, theme, "Refresh") {
                        with_state(|bs| spawn_list_fetch(bs, &server));
                    }
                });
            });
            ui.label(
                RichText::new(format!("Version: v{}", env!("CARGO_PKG_VERSION")))
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_sm);

            ScrollArea::vertical().show(ui, |ui| {
                with_state(|bs| {
                    let submitting = bs.submit_rx.is_some();

                    // Form
                    widgets::card(ui, theme, |ui| {
                        // Title
                        ui.label(RichText::new("Title").color(theme.text_secondary()));
                        ui.add(
                            egui::TextEdit::singleline(&mut bs.title)
                                .desired_width(f32::INFINITY)
                                .hint_text("Brief summary of the bug"),
                        );
                        ui.add_space(theme.spacing_xs);

                        // Description
                        ui.label(RichText::new("Description").color(theme.text_secondary()));
                        ui.add(
                            egui::TextEdit::multiline(&mut bs.description)
                                .desired_width(f32::INFINITY)
                                .desired_rows(4)
                                .hint_text("Steps to reproduce, expected vs actual behavior..."),
                        );
                        ui.add_space(theme.spacing_xs);

                        // Severity + Category dropdowns (lists from data/bugs/taxonomy.json)
                        let severities = &state.bug_severities;
                        let categories = &state.bug_categories;
                        let sev_label = severities.get(bs.severity_idx).map(String::as_str).unwrap_or("");
                        let cat_label = categories.get(bs.category_idx).map(String::as_str).unwrap_or("");
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Severity:").color(theme.text_secondary()));
                            egui::ComboBox::from_id_salt("severity")
                                .selected_text(sev_label)
                                .show_ui(ui, |ui| {
                                    for (i, sev) in severities.iter().enumerate() {
                                        ui.selectable_value(&mut bs.severity_idx, i, sev.as_str());
                                    }
                                });

                            ui.add_space(theme.spacing_md);

                            ui.label(RichText::new("Category:").color(theme.text_secondary()));
                            egui::ComboBox::from_id_salt("category")
                                .selected_text(cat_label)
                                .show_ui(ui, |ui| {
                                    for (i, cat) in categories.iter().enumerate() {
                                        ui.selectable_value(&mut bs.category_idx, i, cat.as_str());
                                    }
                                });
                        });
                        ui.add_space(theme.spacing_sm);

                        // Submit: goes to the server, not to a local list.
                        ui.horizontal(|ui| {
                            let submit_label = if submitting { "Sending..." } else { "Submit Report" };
                            let clicked = widgets::primary_button(ui, theme, submit_label);
                            if clicked && !submitting {
                                let title = bs.title.trim().to_string();
                                let description = bs.description.trim().to_string();
                                // The relay requires BOTH (create_bug returns 400
                                // otherwise), so say so here rather than let a
                                // report bounce off the server.
                                if title.is_empty() {
                                    bs.status_message = "Title is required.".into();
                                } else if description.is_empty() {
                                    bs.status_message = "Description is required.".into();
                                } else {
                                    let severity = severities
                                        .get(bs.severity_idx)
                                        .cloned()
                                        .unwrap_or_default();
                                    let category = categories
                                        .get(bs.category_idx)
                                        .cloned()
                                        .unwrap_or_default();
                                    let (tx, rx) = std::sync::mpsc::channel();
                                    let server = server.clone();
                                    let key = my_key.clone();
                                    let name = my_name.clone();
                                    std::thread::spawn(move || {
                                        let _ = tx.send(submit_bug_blocking(
                                            &server,
                                            &title,
                                            &description,
                                            &severity,
                                            &category,
                                            &key,
                                            &name,
                                        ));
                                    });
                                    bs.submit_rx = Some(rx);
                                    bs.status_message = "Sending to the server...".into();
                                }
                            }
                            if !bs.status_message.is_empty() {
                                ui.label(
                                    RichText::new(&bs.status_message)
                                        .color(theme.text_muted())
                                        .size(theme.font_size_small),
                                );
                            }
                        });
                    });

                    ui.add_space(theme.spacing_md);

                    // Reports the SERVER holds (survives a restart, visible to
                    // every other member and to maintainers).
                    ui.label(
                        RichText::new("Reports on this server")
                            .size(theme.font_size_body)
                            .color(theme.text_secondary()),
                    );
                    if bs.reports.is_empty() {
                        let empty = if bs.list_rx.is_some() {
                            "Loading reports from the server..."
                        } else {
                            "No reports on this server yet."
                        };
                        ui.label(RichText::new(empty).color(theme.text_muted()));
                    }
                    for report in &bs.reports {
                        widgets::card(ui, theme, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(&report.title)
                                        .color(theme.text_primary())
                                        .strong(),
                                );
                                widgets::badge(
                                    ui,
                                    theme,
                                    &severity_label(&report.severity),
                                    severity_color(&report.severity, theme),
                                );
                                widgets::badge(
                                    ui,
                                    theme,
                                    &status_label(&report.status),
                                    status_color(&report.status, theme),
                                );
                            });
                            let mut meta = format!("{} | v{}", report.category, report.version);
                            if !report.reporter_name.is_empty() {
                                meta.push_str(&format!(" | {}", report.reporter_name));
                            }
                            if report.votes > 0 {
                                meta.push_str(&format!(" | {} votes", report.votes));
                            }
                            ui.label(
                                RichText::new(meta)
                                    .size(theme.font_size_small)
                                    .color(theme.text_muted()),
                            );
                            if !report.description.is_empty() {
                                ui.label(
                                    RichText::new(&report.description)
                                        .size(theme.font_size_small)
                                        .color(theme.text_secondary()),
                                );
                            }
                        });
                        ui.add_space(theme.row_gap);
                    }
                });
            });
        });
}
