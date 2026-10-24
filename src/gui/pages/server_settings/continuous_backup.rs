//! Continuous backup in Server Settings > ADMIN > Backups (2026-10-22). The relay side is
//! `src/relay/backup_stream/`; this panel and the web admin page's Backups panel
//! (`web/pages/backups-admin.js`) both talk to its one endpoint, `POST /api/admin/backups`, so
//! the two apps show and do the same things:
//!
//! - where the copies go, how far behind they are, what the destination can restore (from when
//!   to when), its size, the history kept, the backup key's fingerprint;
//! - Restore drill: the server restores from the destination into a scratch file and compares
//!   it with the live database (which it only reads), and says whether it passed;
//! - the destination editor (a folder, an S3-compatible bucket, or another HumanityOS server),
//!   tested before it is saved; a secret left blank keeps the saved one, and none is ever sent
//!   back to this app;
//! - which other servers may keep their backups on this one, and how much space each gets.
//!
//! Every request is signed by the admin's own identity over the action and a hash of its
//! settings, on a worker thread, so the page never waits on the network.

use std::cell::RefCell;
use std::sync::mpsc;

use egui::RichText;

use crate::gui::theme::Theme;
use crate::gui::{widgets, GuiState};
use crate::relay::backup_stream::utc_text;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Kind {
    #[default]
    Off,
    Folder,
    S3,
    Server,
}

type Reply = Result<(bool, String, serde_json::Value), String>;

/// The panel's own state, kept with the page rather than in `GuiState`.
struct Panel {
    status: Option<serde_json::Value>,
    rx: Option<mpsc::Receiver<Reply>>,
    busy: String,
    message: String,
    message_ok: bool,
    last_fetch: Option<std::time::Instant>,
    editor_loaded: bool,
    kind: Kind,
    folder: String,
    endpoint: String,
    region: String,
    bucket: String,
    prefix: String,
    key_id: String,
    secret: String,
    path_style: bool,
    server_url: String,
    retention_hours: String,
    snapshot_hours: String,
    alert_minutes: String,
    host_label: String,
    host_key: String,
    host_gb: String,
    confirm_remove: Option<String>,
}

impl Default for Panel {
    fn default() -> Self {
        Self {
            status: None,
            rx: None,
            busy: String::new(),
            message: String::new(),
            message_ok: true,
            last_fetch: None,
            editor_loaded: false,
            kind: Kind::Off,
            folder: String::new(),
            endpoint: String::new(),
            region: "us-east-1".into(),
            bucket: String::new(),
            prefix: "humanityos".into(),
            key_id: String::new(),
            secret: String::new(),
            path_style: true,
            server_url: String::new(),
            retention_hours: "72".into(),
            snapshot_hours: "24".into(),
            alert_minutes: "5".into(),
            host_label: String::new(),
            host_key: String::new(),
            host_gb: "20".into(),
            confirm_remove: None,
        }
    }
}

thread_local! {
    static PANEL: RefCell<Panel> = RefCell::new(Panel::default());
}

/// Send one action on a worker thread.
fn send(p: &mut Panel, state: &GuiState, action: &str, params: String, busy: &str) {
    let base = state.server_url.trim_end_matches('/').to_string();
    let key = state.profile_public_key.clone();
    let seed = state.private_key_bytes.clone();
    let action = action.to_string();
    let (tx, rx) = mpsc::channel();
    p.rx = Some(rx);
    p.busy = busy.to_string();
    std::thread::spawn(move || {
        let run = || -> Reply {
            let seed = seed.ok_or("No identity loaded: sign in first.")?;
            if key.is_empty() {
                return Err("No identity loaded: sign in first.".into());
            }
            let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_millis() as u64;
            // The relay's own function for the words, so the two can never differ.
            let sig = crate::net::identity::pq_sign_chat(&seed, &crate::relay::backup_stream::api::purpose(&action, &params), ts);
            let body = serde_json::json!({ "key": key, "timestamp": ts, "sig": sig, "action": action, "params": params }).to_string();
            let text = match ureq::post(&format!("{base}/api/admin/backups")).set("Content-Type", "application/json").send_string(&body) {
                Ok(r) => r.into_string().map_err(|e| format!("read: {e}"))?,
                Err(ureq::Error::Status(code, r)) => {
                    let t = r.into_string().unwrap_or_default();
                    if serde_json::from_str::<serde_json::Value>(&t).is_err() {
                        return Err(match code {
                            404 => "This server does not have continuous backup yet (it needs an update).".into(),
                            _ => format!("The server answered HTTP {code}."),
                        });
                    }
                    t
                }
                Err(e) => return Err(format!("Could not reach the server: {e}")),
            };
            let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("parse: {e}"))?;
            Ok((v["ok"].as_bool().unwrap_or(false), v["message"].as_str().unwrap_or("").to_string(), v["status"].clone()))
        };
        let _ = tx.send(run());
    });
}

fn load_editor(p: &mut Panel, s: &serde_json::Value) {
    let c = &s["config"];
    let d = &c["destination"];
    p.kind = match (c["enabled"].as_bool().unwrap_or(false), d["kind"].as_str()) {
        (_, Some("folder")) => Kind::Folder,
        (_, Some("s3")) => Kind::S3,
        (_, Some("server")) => Kind::Server,
        _ => Kind::Off,
    };
    if !c["enabled"].as_bool().unwrap_or(false) {
        p.kind = Kind::Off;
    }
    let st = |v: &serde_json::Value| v.as_str().unwrap_or("").to_string();
    if let Some(k) = d["kind"].as_str() {
        match k {
            "folder" => p.folder = st(&d["path"]),
            "s3" => {
                p.endpoint = st(&d["endpoint"]);
                p.region = st(&d["region"]);
                p.bucket = st(&d["bucket"]);
                p.prefix = st(&d["prefix"]);
                p.path_style = d["path_style"].as_bool().unwrap_or(true);
            }
            "server" => p.server_url = st(&d["url"]),
            _ => {}
        }
    }
    for (field, key) in [(&mut p.retention_hours, "retention_hours"), (&mut p.snapshot_hours, "snapshot_every_hours"), (&mut p.alert_minutes, "alert_after_minutes")] {
        if let Some(n) = c[key].as_u64() {
            *field = n.to_string();
        }
    }
    p.editor_loaded = true;
}

/// The destination as the editor holds it, in the relay's shape.
fn destination_json(p: &Panel) -> serde_json::Value {
    match p.kind {
        Kind::Off => serde_json::Value::Null,
        Kind::Folder => serde_json::json!({ "kind": "folder", "path": p.folder.trim() }),
        Kind::S3 => serde_json::json!({
            "kind": "s3",
            "endpoint": p.endpoint.trim(),
            "region": p.region.trim(),
            "bucket": p.bucket.trim(),
            "prefix": p.prefix.trim(),
            "access_key_id": p.key_id.trim(),
            "secret_access_key": p.secret,
            "path_style": p.path_style,
        }),
        Kind::Server => serde_json::json!({ "kind": "server", "url": p.server_url.trim() }),
    }
}

fn status_dot(ui: &mut egui::Ui, color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(10.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 5.0, color);
}

fn ago(now_ms: u64, then_ms: u64) -> String {
    if then_ms == 0 {
        return "never".into();
    }
    let s = now_ms.saturating_sub(then_ms) / 1000;
    if s < 90 {
        format!("{s} s ago")
    } else if s < 90 * 60 {
        format!("{} min ago", s / 60)
    } else if s < 48 * 3600 {
        format!("{} h ago", s / 3600)
    } else {
        format!("{} days ago", s / 86_400)
    }
}

pub(crate) fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    PANEL.with(|cell| {
        let mut p = cell.borrow_mut();
        draw_panel(ui, theme, state, &mut p);
    });
}

fn draw_panel(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, p: &mut Panel) {
    // A finished request.
    if let Some(rx) = &p.rx {
        match rx.try_recv() {
            Ok(Ok((ok, msg, status))) => {
                if status.is_object() {
                    if !p.editor_loaded {
                        load_editor(p, &status);
                    }
                    p.status = Some(status);
                }
                if !msg.is_empty() {
                    p.message = msg;
                    p.message_ok = ok;
                }
                p.rx = None;
                p.busy.clear();
                p.last_fetch = Some(std::time::Instant::now());
            }
            Ok(Err(e)) => {
                p.message = e;
                p.message_ok = false;
                p.rx = None;
                p.busy.clear();
                p.last_fetch = Some(std::time::Instant::now());
            }
            Err(mpsc::TryRecvError::Empty) => ui.ctx().request_repaint_after(std::time::Duration::from_millis(300)),
            Err(mpsc::TryRecvError::Disconnected) => {
                p.rx = None;
                p.busy.clear();
            }
        }
    }
    // Keep the status fresh while the panel is open: every 10 s, every 2 s during a drill.
    let connected = state.ws_client.as_ref().is_some_and(|c| c.is_connected());
    let drilling = p.status.as_ref().is_some_and(|s| s["stream"]["drill"]["running"].as_bool().unwrap_or(false));
    let every = if drilling { 2 } else { 10 };
    if connected && p.rx.is_none() && p.last_fetch.map_or(true, |t| t.elapsed().as_secs() >= every) {
        send(p, state, "status", String::new(), "");
    }
    ui.ctx().request_repaint_after(std::time::Duration::from_secs(every));

    widgets::subsection_label(ui, theme, "Continuous backup");
    widgets::body_hint(
        ui,
        theme,
        "Every change to this server's database is copied, within seconds, to a place you choose: \
         another disk, an S3-compatible bucket, or another HumanityOS server you trust. Each copy is \
         sealed with this server's backup key before it leaves, so the place keeping it cannot read \
         it. If the server's machine is lost, about a minute of changes is the most that is lost, and any \
         moment inside the history you keep can be restored.",
    );
    ui.add_space(theme.spacing_xs);

    let now = crate::relay::storage::now_millis();
    if let Some(s) = p.status.clone() {
        let st = &s["stream"];
        let state_word = st["state"].as_str().unwrap_or("off");
        let (color, label) = match state_word {
            "up_to_date" => (theme.success(), "Up to date"),
            "catching_up" => (theme.info(), "Sending"),
            "starting" => (theme.info(), "Starting"),
            "failing" => (theme.warning(), "Retrying"),
            "behind" => (theme.danger(), "Behind"),
            "refused" => (theme.danger(), "Refused"),
            _ => (theme.text_muted(), "Off"),
        };
        ui.horizontal_wrapped(|ui| {
            status_dot(ui, color);
            ui.label(RichText::new(label).strong().color(color));
            ui.label(RichText::new(st["words"].as_str().unwrap_or("")).size(theme.font_size_small).color(theme.text_secondary()));
        });
        ui.add_space(theme.spacing_xs);
        if state_word != "off" {
            let g = |k: &str| st[k].as_u64().unwrap_or(0);
            egui::Grid::new("continuous_backup_grid").num_columns(2).spacing([theme.spacing_xl, theme.spacing_xs]).show(ui, |ui| {
                let row = |ui: &mut egui::Ui, k: &str, v: String| {
                    ui.label(RichText::new(k).size(theme.font_size_small).color(theme.text_secondary()));
                    ui.label(RichText::new(v).size(theme.font_size_small).color(theme.text_primary()));
                    ui.end_row();
                };
                row(ui, "Where the copies go", s["config"]["destination"]["summary"].as_str().unwrap_or("nowhere yet").to_string());
                row(ui, "Newest copy there", if g("restorable_to_ms") > 0 { format!("{} ({})", utc_text(g("restorable_to_ms")), ago(now, g("restorable_to_ms"))) } else { "none yet".into() });
                let lag = g("lag_secs");
                row(
                    ui,
                    "Behind by",
                    match lag {
                        0 => "nothing".into(),
                        1..=119 => format!("{lag} s"),
                        120..=7199 => format!("{} min", lag / 60),
                        _ => format!("{} h", lag / 3600),
                    },
                );
                row(ui, "Can restore", if g("restorable_from_ms") > 0 { format!("any moment from {} to {}", utc_text(g("restorable_from_ms")), utc_text(g("restorable_to_ms"))) } else { "not yet".into() });
                row(ui, "At the destination", format!("{} in {} objects, {} full copies", human(g("dest_bytes")), g("dest_objects"), g("snapshots")));
                row(ui, "Waiting to send", format!("{} parts ({})", g("pending_objects"), human(g("pending_bytes"))));
                row(
                    ui,
                    "History kept",
                    format!("{} hours; a full copy every {} hours; alert after {} min behind", g("retention_hours"), g("snapshot_every_hours"), g("alert_after_minutes")),
                );
                row(ui, "Last copy sent", ago(now, g("last_copy_ms")));
                row(ui, "Backup key", st["key_fingerprint"].as_str().unwrap_or("").to_string());
                if !st["last_alert"].as_str().unwrap_or("").is_empty() {
                    row(ui, "Last alert", format!("{} ({})", st["last_alert"].as_str().unwrap_or(""), ago(now, g("last_alert_ms"))));
                }
            });
            widgets::body_hint(
                ui,
                theme,
                "Keep a copy of the server's data/backup.key and data/backup-destination.secrets.json somewhere \
                 off its machine: a restore needs both, and the fingerprint above tells you the key you kept is \
                 this one.",
            );
        }
        ui.add_space(theme.spacing_xs);
        ui.horizontal(|ui| {
            let idle = p.rx.is_none();
            if widgets::Button::secondary("Refresh").disabled(!idle).show(ui, theme) {
                send(p, state, "status", String::new(), "Refreshing...");
            }
            let on = state_word != "off";
            if widgets::Button::primary("Restore drill")
                .disabled(!idle || !on || drilling)
                .tooltip("Restore from the destination into a scratch file, check it, and compare it with the live database at the same moment. The live database is only read.")
                .show(ui, theme)
            {
                send(p, state, "restore_drill", String::new(), "Starting the drill...");
            }
            if !p.busy.is_empty() {
                ui.label(RichText::new(&p.busy).size(theme.font_size_small).color(theme.text_muted()));
            }
        });
        let d = &st["drill"];
        let words = d["words"].as_str().unwrap_or("");
        if !words.is_empty() {
            let c = match d["ok"].as_bool() {
                Some(true) => theme.success(),
                Some(false) => theme.danger(),
                None => theme.text_secondary(),
            };
            ui.label(RichText::new(format!("Restore drill: {words}")).size(theme.font_size_small).color(c));
        }
    } else if !connected {
        widgets::body_hint(ui, theme, "Connect to the server to see its backups.");
    }
    if !p.message.is_empty() {
        let c = if p.message_ok { theme.success() } else { theme.danger() };
        ui.label(RichText::new(&p.message).size(theme.font_size_small).color(c));
    }

    ui.add_space(theme.spacing_sm);
    egui::CollapsingHeader::new("Change where the copies go").id_salt("cb_editor").show(ui, |ui| draw_editor(ui, theme, state, p));
    egui::CollapsingHeader::new("Keep other servers' backups here").id_salt("cb_host").show(ui, |ui| draw_host(ui, theme, state, p));
}

fn human(n: u64) -> String {
    crate::relay::backup_stream::human_bytes(n)
}

fn field(ui: &mut egui::Ui, theme: &Theme, label: &str, value: &mut String, hint: &str, password: bool) {
    ui.horizontal(|ui| {
        ui.add_sized([150.0, 20.0], egui::Label::new(RichText::new(label).size(theme.font_size_small).color(theme.text_secondary())));
        ui.add(egui::TextEdit::singleline(value).desired_width(300.0).hint_text(hint).password(password));
    });
}

fn draw_editor(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, p: &mut Panel) {
    ui.horizontal_wrapped(|ui| {
        ui.radio_value(&mut p.kind, Kind::Off, "Off");
        ui.radio_value(&mut p.kind, Kind::Folder, "A folder (another disk)");
        ui.radio_value(&mut p.kind, Kind::S3, "An S3-compatible bucket");
        ui.radio_value(&mut p.kind, Kind::Server, "Another HumanityOS server");
    });
    ui.add_space(theme.spacing_xs);
    match p.kind {
        Kind::Off => widgets::body_hint(ui, theme, "Off: nothing is copied. What was already sent stays where it is."),
        Kind::Folder => {
            widgets::body_hint(
                ui,
                theme,
                "A folder on the server's own machine (not this computer, unless the server runs here): \
                 best on another disk, or a share mounted from another machine. A folder on the same disk \
                 as the database does not survive that disk failing.",
            );
            field(ui, theme, "Folder (full path)", &mut p.folder, "/mnt/backup/relay or D:\\Backups\\relay", false);
        }
        Kind::S3 => {
            widgets::body_hint(
                ui,
                theme,
                "Any S3-compatible storage: a provider's bucket, or your own (Garage and MinIO are free \
                 software). Leave the key fields empty to keep the saved ones.",
            );
            field(ui, theme, "Endpoint", &mut p.endpoint, "https://s3.eu-central-1.amazonaws.com", false);
            field(ui, theme, "Region", &mut p.region, "us-east-1", false);
            field(ui, theme, "Bucket", &mut p.bucket, "my-relay-backups", false);
            field(ui, theme, "Folder in the bucket", &mut p.prefix, "humanityos", false);
            let id_hint = p.status.as_ref().and_then(|s| s["config"]["destination"]["access_key_hint"].as_str().map(|h| format!("saved: {h}"))).unwrap_or_else(|| "access key ID".into());
            field(ui, theme, "Access key ID", &mut p.key_id, &id_hint, false);
            let secret_set = p.status.as_ref().is_some_and(|s| s["config"]["destination"]["secret_set"].as_bool().unwrap_or(false));
            field(ui, theme, "Secret access key", &mut p.secret, if secret_set { "saved (leave empty to keep it)" } else { "secret access key" }, true);
            ui.checkbox(&mut p.path_style, "Bucket name in the path (works with every S3-compatible server)");
        }
        Kind::Server => {
            widgets::body_hint(
                ui,
                theme,
                "Another HumanityOS server keeps the copies. First give its admin this server's backup sending \
                 key (below, under Keep other servers' backups here): they add it on their server, with the \
                 space they give you. Then enter that server's address here and save.",
            );
            field(ui, theme, "Server address", &mut p.server_url, "https://public.guide", false);
        }
    }
    ui.add_space(theme.spacing_xs);
    field(ui, theme, "History kept (hours)", &mut p.retention_hours, "72", false);
    field(ui, theme, "Full copy every (hours)", &mut p.snapshot_hours, "24", false);
    field(ui, theme, "Alert after (minutes behind)", &mut p.alert_minutes, "5", false);
    widgets::body_hint(
        ui,
        theme,
        "Alerts go to the alert channels in data/alert-channels.secrets.json (ntfy, Discord, Telegram, a \
         webhook). The panel here always shows the state.",
    );
    ui.horizontal(|ui| {
        let idle = p.rx.is_none();
        let num = |s: &str| s.trim().parse::<u32>().ok();
        let numbers = (num(&p.retention_hours), num(&p.snapshot_hours), num(&p.alert_minutes));
        if p.kind != Kind::Off && widgets::Button::secondary("Test").disabled(!idle).tooltip("Write a sealed test copy there, read it back and delete it. Nothing is saved.").show(ui, theme) {
            let params = serde_json::json!({ "destination": destination_json(p) }).to_string();
            send(p, state, "test_destination", params, "Testing...");
        }
        if widgets::Button::primary("Save").disabled(!idle).tooltip("Test the destination, then save. A new destination gets a fresh full copy.").show(ui, theme) {
            match numbers {
                (Some(r), Some(s), Some(a)) => {
                    let dest = destination_json(p);
                    let params = serde_json::json!({
                        "enabled": p.kind != Kind::Off,
                        // Off sends no destination: the relay keeps the saved one for next time.
                        "destination": dest,
                        "retention_hours": r,
                        "snapshot_every_hours": s,
                        "alert_after_minutes": a,
                    })
                    .to_string();
                    p.secret.clear();
                    p.key_id.clear();
                    p.editor_loaded = false;
                    send(p, state, "set_destination", params, "Testing and saving...");
                }
                _ => {
                    p.message = "Hours and minutes are whole numbers.".into();
                    p.message_ok = false;
                }
            }
        }
    });
}

fn draw_host(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, p: &mut Panel) {
    widgets::body_hint(
        ui,
        theme,
        "Other HumanityOS servers can keep their continuous backup here, sealed with their own keys: this \
         server stores ciphertext it cannot read. Each one you add gets the space you give it, and this \
         server also refuses copies once its own disk is 90% full.",
    );
    let Some(s) = p.status.clone() else { return };
    let sending = s["sending_key"].as_str().unwrap_or("").to_string();
    if !sending.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("This server's backup sending key:").size(theme.font_size_small).color(theme.text_secondary()));
            ui.label(RichText::new(format!("{}...{} ({} characters)", &sending[..16.min(sending.len())], &sending[sending.len().saturating_sub(16)..], sending.len())).size(theme.font_size_small).monospace());
            if widgets::Button::secondary("Copy").tooltip("Give this to the admin of the server that will keep this server's backups.").show(ui, theme) {
                ui.ctx().copy_text(sending.clone());
                p.message = "Copied the backup sending key.".into();
                p.message_ok = true;
            }
        });
    }
    ui.add_space(theme.spacing_xs);
    let senders = s["host"]["senders"].as_array().cloned().unwrap_or_default();
    if senders.is_empty() {
        widgets::body_hint(ui, theme, "No other server keeps its backups here.");
    }
    for snd in senders {
        let fp = snd["fingerprint"].as_str().unwrap_or("").to_string();
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(snd["label"].as_str().unwrap_or("")).strong().size(theme.font_size_small));
            ui.label(
                RichText::new(format!(
                    "{} of {} used, {} objects",
                    human(snd["used_bytes"].as_u64().unwrap_or(0)),
                    human(snd["quota_bytes"].as_u64().unwrap_or(0)),
                    snd["objects"].as_u64().unwrap_or(0)
                ))
                .size(theme.font_size_small)
                .color(theme.text_secondary()),
            );
            if p.confirm_remove.as_deref() == Some(fp.as_str()) {
                if widgets::Button::secondary("Stop, keep their copies").show(ui, theme) {
                    let params = serde_json::json!({ "fingerprint": fp, "delete_copies": false }).to_string();
                    p.confirm_remove = None;
                    send(p, state, "host_remove", params, "Removing...");
                }
                if widgets::Button::danger("Stop and delete their copies").show(ui, theme) {
                    let params = serde_json::json!({ "fingerprint": fp, "delete_copies": true }).to_string();
                    p.confirm_remove = None;
                    send(p, state, "host_remove", params, "Removing...");
                }
                if widgets::Button::ghost("Cancel").show(ui, theme) {
                    p.confirm_remove = None;
                }
            } else if widgets::Button::secondary("Stop keeping").show(ui, theme) {
                p.confirm_remove = Some(fp.clone());
            }
        });
    }
    ui.add_space(theme.spacing_xs);
    field(ui, theme, "Name", &mut p.host_label, "the server's name, for you", false);
    ui.horizontal(|ui| {
        ui.add_sized([150.0, 20.0], egui::Label::new(RichText::new("Its backup sending key").size(theme.font_size_small).color(theme.text_secondary())));
        ui.add(egui::TextEdit::multiline(&mut p.host_key).desired_rows(2).desired_width(300.0).hint_text("paste the key from that server's Backups panel"));
    });
    field(ui, theme, "Space to give (GB)", &mut p.host_gb, "20", false);
    if widgets::Button::primary("Keep its backups here").disabled(p.rx.is_some()).show(ui, theme) {
        match p.host_gb.trim().parse::<f64>() {
            Ok(gb) if gb > 0.0 => {
                let params = serde_json::json!({ "label": p.host_label.trim(), "public_key": p.host_key.trim(), "quota_gb": gb }).to_string();
                p.host_key.clear();
                send(p, state, "host_allow", params, "Adding...");
            }
            _ => {
                p.message = "Give the space in GB, more than 0.".into();
                p.message_ok = false;
            }
        }
    }
}
