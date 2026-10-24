//! The admin console's state (`GuiState::admin_console`, 2026-10-22): Server Settings > ADMIN's
//! Alert channels, Federation and Every admin action panels, and the admin log, all drawn from
//! what the connected server answers through its action registry (src/relay/admin/,
//! docs/design/in-app-ops.md). The views are kept as the JSON the server sent, so a field the
//! server adds reaches the panel without a type change here.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};

/// An answer from a worker thread.
pub enum AdminReply {
    /// GET /api/admin/actions, parsed.
    Registry { server: String, result: Result<crate::relay::admin::Registry, String> },
    /// POST /api/admin/actions/run: the server's whole answer, or why there was none.
    Ran { server: String, action: String, result: Result<serde_json::Value, String> },
}

/// A new alert channel, or a change to one, as typed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AlertDraft {
    pub kind: String,
    pub label: String,
    pub address: String,
    pub chat_id: String,
}

/// What the console holds between frames.
pub struct AdminConsoleState {
    pub tx: Sender<AdminReply>,
    pub rx: Receiver<AdminReply>,
    /// Requests sent and not yet answered.
    pub in_flight: usize,
    /// The server the views below came from; when the app talks to another, they are dropped.
    pub server: String,
    pub registry: Option<crate::relay::admin::Registry>,
    pub registry_status: String,
    /// `alerts.*`'s view: `{channels, file, kinds}`.
    pub alerts: Option<serde_json::Value>,
    /// The last alert action's sentence, and whether it worked.
    pub alerts_status: (bool, String),
    pub alert_draft: AlertDraft,
    /// The alert channel being changed, and what has been typed for it.
    pub alert_edit: Option<(usize, AlertDraft)>,
    /// `federation.*`'s view: `{peers, channels, own_key, local_room_guaranteed}`.
    pub federation: Option<serde_json::Value>,
    pub federation_status: (bool, String),
    pub fed_add_url: String,
    pub fed_add_name: String,
    pub fed_add_key: String,
    pub fed_add_key_name: String,
    /// `log.recent`'s entries, newest first.
    pub log: Option<Vec<serde_json::Value>>,
    pub log_status: String,
    /// What is waiting to be confirmed (`"<action>:<what>"`), and what has been typed for a
    /// destructive one.
    pub confirm: Option<String>,
    pub confirm_typed: String,
    /// Every admin action: the action whose form is open, its inputs as typed, and the last
    /// sentence each action answered with (and whether it worked).
    pub open_action: Option<String>,
    pub forms: HashMap<String, HashMap<String, String>>,
    pub results: HashMap<String, (bool, String)>,
    /// The search box over every admin action.
    pub filter: String,
}

impl Default for AdminConsoleState {
    fn default() -> Self {
        let (tx, rx) = channel();
        AdminConsoleState {
            tx,
            rx,
            in_flight: 0,
            server: String::new(),
            registry: None,
            registry_status: String::new(),
            alerts: None,
            alerts_status: (true, String::new()),
            alert_draft: AlertDraft { kind: "ntfy".into(), ..Default::default() },
            alert_edit: None,
            federation: None,
            federation_status: (true, String::new()),
            fed_add_url: String::new(),
            fed_add_name: String::new(),
            fed_add_key: String::new(),
            fed_add_key_name: String::new(),
            log: None,
            log_status: String::new(),
            confirm: None,
            confirm_typed: String::new(),
            open_action: None,
            forms: HashMap::new(),
            results: HashMap::new(),
            filter: String::new(),
        }
    }
}

impl AdminConsoleState {
    /// Forget every view (the app is talking to another server now).
    pub fn forget_views(&mut self) {
        let keep = std::mem::take(&mut self.filter);
        let (tx, rx) = channel();
        *self = AdminConsoleState { tx, rx, filter: keep, ..Default::default() };
    }
}
