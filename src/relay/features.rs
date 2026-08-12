//! The server capability manifest: which features this relay actually offers.
//!
//! One binary serves every role (chat, the shared game world, the Market, backup
//! storage for other people's vaults, ...). A server OWNER does not necessarily
//! want to serve all of them: "some people only need the chat", "not all need the
//! market features enabled", "not all want to allow people to use their relay as
//! a backup" (operator, 2026-08-12).
//!
//! The switches live in `data/server-config.json` under a `features` block. Every
//! feature DEFAULTS TO ON, so an existing deployment that upgrades and never edits
//! its config behaves exactly as it did before.
//!
//! **This module is the decision layer, not the hiding layer.** A disabled feature
//! must REFUSE at the API, not merely disappear from a client's menu — a hidden tab
//! whose endpoint still answers is a lie, and for `vault_backup` it would be an
//! open invitation to store strangers' blobs on a disk the owner said no to. Two
//! enforcement points consume the maps below, and they are the only two doors in:
//!
//!   * HTTP  — [`route_feature`] + the `feature_gate` middleware in
//!             `src/relay/mod.rs`, one layer over the WHOLE router.
//!   * WS    — [`ws_message_feature`] + the single guard at the top of the raw
//!             message dispatch in `src/relay/relay.rs`.
//!
//! Both are ONE place on purpose. A per-handler `if enabled` check is how one
//! endpoint gets missed.
//!
//! ## Why an enum and not pure data
//!
//! `docs/design/infinite-of-x.md` says list-shaped domain objects belong in data
//! files. Feature *values* are data (the JSON block). Feature *names* are not: a
//! name only means something because code enforces it, so you cannot add a real
//! feature by editing JSON. Making it an enum means the route table and the guards
//! are checked by the compiler instead of by string luck.

use serde_json::Value;

/// Everything a server owner can switch off. Add a variant only together with its
/// enforcement point (a route-table row and/or a WS message-type row below) —
/// a name with no enforcement is exactly the lie this module exists to prevent.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Feature {
    /// Public channels and direct messages: the message plane.
    Chat,
    /// The shared, server-authoritative game world (avatars, NPC simulation,
    /// in-world player-to-player trade).
    Game,
    /// The Market: offerings/listings, reviews, seller ratings, order book.
    Market,
    /// Relay-as-backup: storing other people's encrypted vault blobs.
    VaultBackup,
    /// File uploads, the shared-file library, and the asset library — everything
    /// that consumes the owner's disk on a stranger's behalf.
    Uploads,
    /// The task board and projects.
    Tasks,
    /// Voice channels (the TURN credential issuer and voice signalling).
    Voice,
    /// Live video fanout.
    LiveVideo,
    /// Talking to peer servers: outbound federation connections and the peer
    /// directory.
    Federation,
    /// Web push notifications.
    Push,
}

impl Feature {
    /// How many features exist — the width of [`Features`].
    pub const COUNT: usize = 10;

    /// Every feature, in the order they appear in `server-config.json` and in
    /// `/api/server-info`.
    pub const ALL: [Feature; Feature::COUNT] = [
        Feature::Chat,
        Feature::Game,
        Feature::Market,
        Feature::VaultBackup,
        Feature::Uploads,
        Feature::Tasks,
        Feature::Voice,
        Feature::LiveVideo,
        Feature::Federation,
        Feature::Push,
    ];

    /// The JSON key in the `features` block (and in `/api/server-info`).
    pub fn key(self) -> &'static str {
        match self {
            Feature::Chat => "chat",
            Feature::Game => "game",
            Feature::Market => "market",
            Feature::VaultBackup => "vault_backup",
            Feature::Uploads => "uploads",
            Feature::Tasks => "tasks",
            Feature::Voice => "voice",
            Feature::LiveVideo => "live_video",
            Feature::Federation => "federation",
            Feature::Push => "push",
        }
    }

    /// Plain-language description, shown to whoever hit a disabled endpoint.
    /// This is the only explanation a stranger gets, so write it for a human.
    pub fn summary(self) -> &'static str {
        match self {
            Feature::Chat => "public channels and direct messages",
            Feature::Game => "the shared game world",
            Feature::Market => "the market: offerings, reviews and the order book",
            Feature::VaultBackup => "storing your encrypted backup on this server",
            Feature::Uploads => "file uploads and the shared-file library",
            Feature::Tasks => "the task board and projects",
            Feature::Voice => "voice channels",
            Feature::LiveVideo => "live video",
            Feature::Federation => "connecting to peer servers",
            Feature::Push => "web push notifications",
        }
    }

    /// Position in [`Feature::ALL`] — the index into [`Features::on`].
    fn index(self) -> usize {
        // Small and exhaustive; a match keeps it compiler-checked rather than
        // relying on a runtime search that could silently return the wrong slot.
        match self {
            Feature::Chat => 0,
            Feature::Game => 1,
            Feature::Market => 2,
            Feature::VaultBackup => 3,
            Feature::Uploads => 4,
            Feature::Tasks => 5,
            Feature::Voice => 6,
            Feature::LiveVideo => 7,
            Feature::Federation => 8,
            Feature::Push => 9,
        }
    }
}

/// The resolved manifest for this running server. Built once at startup from
/// `data/server-config.json` and then read-only.
#[derive(Clone, Debug)]
pub struct Features {
    on: [bool; Feature::COUNT],
}

impl Default for Features {
    fn default() -> Self {
        Self::all_enabled()
    }
}

impl Features {
    /// Today's behaviour: everything on. This is what a server with no `features`
    /// block gets, which is why upgrading changes nothing.
    pub fn all_enabled() -> Self {
        Features { on: [true; Feature::COUNT] }
    }

    /// Read the `features` block out of a parsed `server-config.json`.
    ///
    /// Missing block, missing key, or a non-boolean value all mean ON. A typo
    /// (`"marklet": false`) therefore leaves the Market ENABLED rather than
    /// silently disabling something else — the safe direction for a
    /// misconfiguration is "behaves like it always did".
    pub fn from_config(config: &Value) -> Self {
        let mut f = Features::all_enabled();
        let block = match config.get("features") {
            Some(Value::Object(map)) => map,
            _ => return f,
        };
        for feature in Feature::ALL {
            if let Some(Value::Bool(v)) = block.get(feature.key()) {
                f.on[feature.index()] = *v;
            }
        }
        f
    }

    pub fn enabled(&self, feature: Feature) -> bool {
        self.on[feature.index()]
    }

    /// Test/tooling helper: flip one switch on an already-built manifest.
    pub fn set(&mut self, feature: Feature, on: bool) {
        self.on[feature.index()] = on;
    }

    /// The features this owner has switched OFF, for the startup log.
    pub fn disabled(&self) -> Vec<Feature> {
        Feature::ALL.into_iter().filter(|f| !self.enabled(*f)).collect()
    }

    /// The manifest as it is advertised on `/api/server-info`, so a client can
    /// hide UI it cannot use and a federation peer knows what this node offers.
    pub fn as_json(&self) -> Value {
        let mut map = serde_json::Map::new();
        for feature in Feature::ALL {
            map.insert(feature.key().to_string(), Value::Bool(self.enabled(feature)));
        }
        Value::Object(map)
    }
}

/// Path prefixes owned by a feature. A row matches a request path exactly, or as
/// a path-segment prefix (`/api/tasks` matches `/api/tasks/7/comments` but never
/// `/api/tasksomething`). Longest match wins, so a more specific row can carve an
/// exception out of a broader one later without reordering the table.
///
/// Routes NOT listed here are always served: `/health`, `/api/stats`,
/// `/api/peers`, `/api/members*`, `/api/server-info`, `/api/profile/*`, identity,
/// moderation and the static web client. Those are the floor every node provides
/// — an owner who could switch off `/api/server-info` would just be invisible,
/// not lighter.
const ROUTE_FEATURES: &[(&str, Feature)] = &[
    // ── Chat: the message plane. Reading history is as much "hosting a chat
    //    server" as accepting a post, so both directions are gated.
    ("/api/send", Feature::Chat),
    ("/api/messages", Feature::Chat),
    ("/api/search", Feature::Chat),
    ("/api/reactions", Feature::Chat),
    ("/api/pins", Feature::Chat),
    // ── Market. `/api/trade/*` is the public ORDER BOOK (server-hosted commerce);
    //    the in-world avatar-to-avatar trade session is WS `trade_*` and belongs
    //    to Game instead — see `ws_message_feature`.
    ("/api/listings", Feature::Market),
    ("/api/sellers", Feature::Market),
    ("/api/trade", Feature::Market),
    // ── Relay-as-backup. THE storage-abuse switch: with this off, a stranger
    //    cannot park a blob on this owner's disk at all, not even by talking
    //    straight to the API with a client that never drew the UI.
    ("/api/vault", Feature::VaultBackup),
    // ── Uploads: anything that spends the owner's disk. `/uploads` is the static
    //    ServeDir, so turning uploads off also stops SERVING what was uploaded
    //    before — the intent is "this server is not a file host", not "no new
    //    files please".
    ("/api/upload", Feature::Uploads),
    ("/api/uploads", Feature::Uploads),
    ("/api/assets", Feature::Uploads),
    ("/uploads", Feature::Uploads),
    // ── Tasks + projects.
    ("/api/tasks", Feature::Tasks),
    ("/api/projects", Feature::Tasks),
    // ── Voice.
    ("/api/turn-credentials", Feature::Voice),
    // ── Live video (both the status endpoint and the binary WS fanout).
    ("/api/live", Feature::LiveVideo),
    ("/ws/live", Feature::LiveVideo),
    // ── Federation.
    ("/api/federation", Feature::Federation),
    // ── Web push.
    ("/api/push", Feature::Push),
    ("/api/vapid-public-key", Feature::Push),
];

/// True when `path` is `prefix` itself or something below it, on a path-segment
/// boundary. Avoids the classic `starts_with` bleed where `/api/live` would also
/// claim `/api/livestock`.
fn path_matches(path: &str, prefix: &str) -> bool {
    if path == prefix {
        return true;
    }
    path.starts_with(prefix) && path.as_bytes().get(prefix.len()) == Some(&b'/')
}

/// Which feature owns this HTTP path, if any. `None` means "always served".
pub fn route_feature(path: &str) -> Option<Feature> {
    let mut best: Option<(usize, Feature)> = None;
    for (prefix, feature) in ROUTE_FEATURES {
        if path_matches(path, prefix) {
            let len = prefix.len();
            if best.map(|(best_len, _)| len > best_len).unwrap_or(true) {
                best = Some((len, *feature));
            }
        }
    }
    best.map(|(_, feature)| feature)
}

/// Which feature owns this inbound WebSocket message type, if any.
///
/// The relay's `/ws` socket is shared by every feature, so the socket itself is
/// never refused — an admin must still be able to identify, moderate and read the
/// server settings on a node with everything else switched off. The refusal lands
/// on the individual message type instead.
pub fn ws_message_feature(msg_type: &str) -> Option<Feature> {
    // Game world, including the in-world trade session (`trade_request`,
    // `trade_confirm`, ...). Those are two avatars swapping items in the
    // simulation, not the Market's order book.
    if msg_type.starts_with("game_") || msg_type.starts_with("trade_") {
        return Some(Feature::Game);
    }
    if msg_type.starts_with("task_") {
        return Some(Feature::Tasks);
    }
    // The OTHER way a stranger parks data on this owner's disk: `sync_save`
    // accepts up to 512 KB of arbitrary user data per key over the socket. An
    // owner who switched off relay-backup and only got `/api/vault/sync` gated
    // would still be hosting everybody's blobs through this door.
    //
    // NOT to be confused with `backup_run` / `backup_list_request`, which are the
    // ADMIN's own database backups — those stay available (they are the owner
    // acting on their own box, not a stranger spending their disk).
    if msg_type == "sync_save" || msg_type == "sync_load" {
        return Some(Feature::VaultBackup);
    }
    // Voice signalling. `webrtc_signal` carries the peer connection offers for
    // voice calls and has no other user.
    if msg_type.starts_with("voice_") || msg_type == "webrtc_signal" {
        return Some(Feature::Voice);
    }
    // The message plane. Listed explicitly rather than by prefix so a future
    // message type has to be classified deliberately.
    // The MARKET's order book over the socket. Missing this family meant
    // `market: false` refused the REST routes while listings were still written
    // through /ws - proven by a reviewer probe on 2026-08-12 that created a
    // listing on a server with the Market switched off.
    if msg_type.starts_with("listing_") {
        return Some(Feature::Market);
    }
    // Projects are the Project Board, same feature as tasks (/api/projects* was
    // already gated under Tasks - the WS side simply had no row).
    if msg_type.starts_with("project_") {
        return Some(Feature::Tasks);
    }
    // Live video / screen-share signalling. `live_video: false` refused
    // /api/live and /ws/live/* while this entire plane stayed open.
    if msg_type.starts_with("stream_") {
        return Some(Feature::LiveVideo);
    }
    // Group messaging is part of the message plane and PERSISTS to the owner's
    // disk (handle_group_msg -> store_group_message), so `chat: false` was
    // still accepting and storing group traffic.
    if msg_type.starts_with("group_") {
        return Some(Feature::Chat);
    }
    // Listing REVIEWS are part of the Market's order book (a review is attached
    // to a listing). The completeness gate found this one; the human review had
    // already found listing_* and still missed it, which is the argument for
    // having the gate at all.
    if msg_type.starts_with("review_") {
        return Some(Feature::Market);
    }
    // The server-to-server plane. `federation: false` skipped outbound dialling
    // and refused /api/federation/*, but every INBOUND federation frame was
    // still processed - so an owner who declined to federate still accepted
    // peers' chat, profile gossip and signed objects onto their disk.
    if msg_type.starts_with("federation_")
        || msg_type == "federated_chat"
        || msg_type == "profile_gossip"
        || msg_type == "signed_object_gossip"
    {
        return Some(Feature::Federation);
    }
    // Web-push preferences belong to push notifications.
    if msg_type.ends_with("notification_prefs") || msg_type == "notification_prefs_data" {
        return Some(Feature::Push);
    }
    // The social graph and the member directory ride the message plane: follows,
    // friend codes, threads, member lists, profiles and link previews are all
    // chat surfaces, and several persist to the owner's disk.
    if msg_type.starts_with("follow")
        || msg_type == "unfollow"
        || msg_type.starts_with("friend_code_")
        || msg_type.starts_with("thread_")
        || msg_type.starts_with("member_")
        || msg_type.starts_with("profile_")
        || msg_type == "link_previews"
        || msg_type == "search_results"
    {
        return Some(Feature::Chat);
    }
    match msg_type {
        "chat" | "edit" | "delete" | "delete_by_id" | "reaction" | "typing" | "search"
        | "pin_request" | "dm" | "dm_open" | "dm_history" | "dm_list" | "dm_read" => {
            Some(Feature::Chat)
        }
        _ => None,
    }
}

/// Inbound WS message types that are deliberately NEVER gated, with the reason.
///
/// THIS IS THE OTHER HALF OF THE GATE. `ws_message_feature` returning `None`
/// means "always on", so an unclassified type FAILS OPEN - which is exactly how
/// market, tasks, chat and live_video shipped half-enforced and were caught by a
/// reviewer probe rather than by a test (2026-08-12). Every inbound type must
/// now be either classified above or named here, and the completeness test
/// fails the build if a new one appears in neither.
///
/// What belongs here:
///   - the handshake, which must work before anything can be refused;
///   - the OWNER administering their own box (switching off a feature does not
///     switch off your own controls);
///   - server -> client notifications, which are outbound and cannot be
///     meaningfully refused at ingest.
pub const WS_ALWAYS_ON: &[&str] = &[
    // Handshake and transport floor.
    "identify", "identify_challenge", "identify_response", "system", "private",
    "name_taken", "peer_list", "peer_joined", "peer_left", "full_user_list",
    "set_status", "online", "unreachable",
    // The owner administering their own server.
    "backup_run", "backup_list", "backup_list_request",
    "server_settings_request", "server_settings_state", "server_settings_update",
    "role_list", "role_upsert", "role_delete", "set_user_role",
    "banned_list", "banned_list_request", "muted_list", "muted_list_request",
    "unban", "unmute", "admin", "mod", "muted", "verified", "donor",
    "service_control", "service_state",
    // A user managing their OWN devices and keys. Account security must keep
    // working no matter which features the owner offers - locking someone out
    // of revoking a lost device would be a worse failure than any feature.
    "device_list", "device_list_request", "device_label", "device_revoke",
    "mod_action",
    // Outbound notifications (server -> client echoes of state changes).
    "message_deleted", "pin_added", "pin_removed", "pins_sync", "reactions_sync",
    "channel_list", "channel_update", "profile_data", "announcements",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_preserve_todays_behaviour() {
        // No config file at all.
        let none = Features::from_config(&serde_json::json!({}));
        for f in Feature::ALL {
            assert!(none.enabled(f), "{} must default ON with no features block", f.key());
        }
        // A config that predates this block entirely (the shape shipped before).
        let legacy = serde_json::json!({
            "server_name": "United Humanity",
            "max_connections": 500,
            "funding": { "enabled": true }
        });
        let legacy = Features::from_config(&legacy);
        for f in Feature::ALL {
            assert!(legacy.enabled(f), "{} must default ON for a pre-manifest config", f.key());
        }
        // A partial block: unmentioned features stay ON.
        let partial = Features::from_config(&serde_json::json!({
            "features": { "game": false }
        }));
        assert!(!partial.enabled(Feature::Game));
        assert!(partial.enabled(Feature::Chat));
        assert!(partial.enabled(Feature::Market));
        assert!(partial.enabled(Feature::VaultBackup));
    }

    #[test]
    fn a_typo_or_junk_value_leaves_the_feature_on() {
        let f = Features::from_config(&serde_json::json!({
            "features": { "marklet": false, "vault_backup": "no", "chat": 0 }
        }));
        assert!(f.enabled(Feature::Market), "an unknown key must not disable anything");
        assert!(f.enabled(Feature::VaultBackup), "a non-boolean must not disable");
        assert!(f.enabled(Feature::Chat), "a non-boolean must not disable");
    }

    #[test]
    fn the_shipped_server_config_parses_and_covers_every_feature() {
        // Guards the manifest file itself: every switch this code enforces has a
        // row in data/server-config.json, so an owner reading the file sees the
        // complete set rather than having to know the enum.
        let raw = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/server-config.json"
        ))
        .expect("data/server-config.json must exist");
        let cfg: Value = serde_json::from_str(&raw).expect("server-config.json must be valid JSON");
        let block = cfg
            .get("features")
            .and_then(|v| v.as_object())
            .expect("server-config.json must carry a features block");
        for f in Feature::ALL {
            assert!(
                block.contains_key(f.key()),
                "data/server-config.json is missing the '{}' switch",
                f.key()
            );
        }
        let parsed = Features::from_config(&cfg);
        for f in Feature::ALL {
            assert!(parsed.enabled(f), "the shipped config must ship every feature ON: {}", f.key());
        }
    }

    #[test]
    fn routes_map_to_the_feature_that_owns_them() {
        assert_eq!(route_feature("/api/vault/sync"), Some(Feature::VaultBackup));
        assert_eq!(route_feature("/api/listings"), Some(Feature::Market));
        assert_eq!(route_feature("/api/listings/42/reviews"), Some(Feature::Market));
        assert_eq!(route_feature("/api/sellers/abc/rating"), Some(Feature::Market));
        assert_eq!(route_feature("/api/trade/orders"), Some(Feature::Market));
        assert_eq!(route_feature("/api/messages"), Some(Feature::Chat));
        assert_eq!(route_feature("/api/tasks/7/comments"), Some(Feature::Tasks));
        assert_eq!(route_feature("/api/projects"), Some(Feature::Tasks));
        assert_eq!(route_feature("/api/upload"), Some(Feature::Uploads));
        assert_eq!(route_feature("/api/uploads/delete"), Some(Feature::Uploads));
        assert_eq!(route_feature("/api/assets/xyz"), Some(Feature::Uploads));
        assert_eq!(route_feature("/uploads/pic.png"), Some(Feature::Uploads));
        assert_eq!(route_feature("/api/turn-credentials"), Some(Feature::Voice));
        assert_eq!(route_feature("/ws/live/pub"), Some(Feature::LiveVideo));
        assert_eq!(route_feature("/api/live"), Some(Feature::LiveVideo));
        assert_eq!(route_feature("/api/federation/servers"), Some(Feature::Federation));
        assert_eq!(route_feature("/api/push/subscribe"), Some(Feature::Push));
        assert_eq!(route_feature("/api/vapid-public-key"), Some(Feature::Push));
    }

    #[test]
    fn the_always_on_floor_is_never_gated() {
        for path in [
            "/health",
            "/ws",
            "/api/stats",
            "/api/peers",
            "/api/members",
            "/api/members/count",
            "/api/server-info",
            "/api/profile/deadbeef",
            "/api/civilization",
            "/api/admin/stats",
            "/",
            "/index.html",
        ] {
            assert_eq!(route_feature(path), None, "{path} must always be served");
        }
    }

    #[test]
    fn prefixes_do_not_bleed_into_neighbouring_paths() {
        // The bug this guards: `/api/live` claiming `/api/listings`, or
        // `/api/upload` claiming `/api/uploadsomething`.
        assert_eq!(route_feature("/api/listings"), Some(Feature::Market));
        assert_eq!(route_feature("/api/livestock"), None);
        assert_eq!(route_feature("/api/uploader"), None);
        assert_eq!(route_feature("/api/searching"), None);
        assert_eq!(route_feature("/uploadsomething"), None);
    }

    #[test]
    fn ws_message_types_map_to_the_feature_that_owns_them() {
        assert_eq!(ws_message_feature("game_join"), Some(Feature::Game));
        assert_eq!(ws_message_feature("game_position_update"), Some(Feature::Game));
        assert_eq!(ws_message_feature("trade_request"), Some(Feature::Game));
        assert_eq!(ws_message_feature("task_create"), Some(Feature::Tasks));
        assert_eq!(ws_message_feature("voice_call"), Some(Feature::Voice));
        assert_eq!(ws_message_feature("webrtc_signal"), Some(Feature::Voice));
        assert_eq!(ws_message_feature("chat"), Some(Feature::Chat));
        assert_eq!(ws_message_feature("dm"), Some(Feature::Chat));
        assert_eq!(ws_message_feature("dm_history"), Some(Feature::Chat));
        // The second door onto the owner's disk, closed by the same switch.
        assert_eq!(ws_message_feature("sync_save"), Some(Feature::VaultBackup));
        assert_eq!(ws_message_feature("sync_load"), Some(Feature::VaultBackup));
        // ...but the admin's own DB backups are not a stranger's storage claim.
        assert_eq!(ws_message_feature("backup_run"), None);
        assert_eq!(ws_message_feature("backup_list_request"), None);
        // Identity + moderation must survive on a server with everything off,
        // or an owner could lock themselves out of their own box.
        assert_eq!(ws_message_feature("identify"), None);
        assert_eq!(ws_message_feature("identify_response"), None);
        assert_eq!(ws_message_feature("set_user_role"), None);
        assert_eq!(ws_message_feature("server_settings_request"), None);
        assert_eq!(ws_message_feature("unban"), None);
    }

    #[test]
    fn advertised_json_lists_every_feature_with_its_state() {
        let mut f = Features::all_enabled();
        f.set(Feature::VaultBackup, false);
        f.set(Feature::Game, false);
        let json = f.as_json();
        let obj = json.as_object().unwrap();
        assert_eq!(obj.len(), Feature::ALL.len());
        assert_eq!(obj["vault_backup"], serde_json::json!(false));
        assert_eq!(obj["game"], serde_json::json!(false));
        assert_eq!(obj["chat"], serde_json::json!(true));
        let disabled: Vec<&str> = f.disabled().into_iter().map(|f| f.key()).collect();
        assert_eq!(disabled, vec!["game", "vault_backup"]);
    }

    /// END TO END over a real socket: build the REAL router the relay serves,
    /// switch off `vault_backup`, and prove the endpoint itself refuses.
    ///
    /// This is deliberately not a unit test of `route_feature` — that function
    /// can be perfect while nobody ever calls it. Deleting the `feature_gate`
    /// layer in `src/relay/mod.rs` leaves every unit test above green and turns
    /// THIS one red, which is the only reason it is worth its runtime.
    #[tokio::test]
    async fn a_disabled_feature_refuses_at_the_api_and_an_enabled_one_still_serves() {
        use crate::relay::relay::RelayState;
        use std::sync::Arc;

        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir()
            .join(format!("hum_featgate_{}_{nanos}.db", std::process::id()));
        let db = crate::relay::storage::Storage::open(&path).expect("open test db");

        let mut state = RelayState::new(db);
        // The owner said: chat yes, backups no.
        state.features = Features::all_enabled();
        state.features.set(Feature::VaultBackup, false);
        let state = Arc::new(state);

        let app = crate::relay::build_router(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let client = reqwest::Client::new();
        let base = format!("http://127.0.0.1:{port}");

        // 1. The disabled feature REFUSES, with a body that says why.
        let res = client
            .get(format!("{base}/api/vault/sync?key=deadbeef"))
            .send()
            .await
            .expect("request reaches the relay");
        assert_eq!(
            res.status().as_u16(),
            403,
            "a disabled feature must refuse at the API, not merely be hidden by a client"
        );
        let body: Value = res.json().await.expect("refusal carries a JSON body");
        assert_eq!(body["error"], "feature_disabled");
        assert_eq!(body["feature"], "vault_backup");
        assert!(
            body["message"].as_str().unwrap_or("").contains("backup"),
            "the refusal must explain itself: {body}"
        );

        // 2. A WRITE to the disabled feature is refused too — this is the one
        //    that actually protects the owner's disk. It must never reach the
        //    handler's own auth path (which would answer 400/401 instead).
        let res = client
            .put(format!("{base}/api/vault/sync"))
            .json(&serde_json::json!({ "public_key": "deadbeef", "blob": "x" }))
            .send()
            .await
            .expect("request reaches the relay");
        assert_eq!(res.status().as_u16(), 403, "a disabled feature must refuse writes");

        // 3. An ENABLED feature is untouched: /api/messages answers normally.
        let res = client
            .get(format!("{base}/api/messages?channel=general&limit=1"))
            .send()
            .await
            .expect("request reaches the relay");
        assert_eq!(
            res.status().as_u16(),
            200,
            "an enabled feature must keep working while another is off"
        );

        // 4. And the always-on floor still answers.
        let res = client.get(format!("{base}/health")).send().await.unwrap();
        assert_eq!(res.status().as_u16(), 200, "/health is never gated");

        // 5. The manifest is advertised so clients can hide what they cannot use.
        let info: Value = client
            .get(format!("{base}/api/server-info"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .expect("server-info is JSON");
        assert_eq!(info["features"]["vault_backup"], serde_json::json!(false));
        assert_eq!(info["features"]["chat"], serde_json::json!(true));
        // The pre-existing fields must survive — clients already read these.
        assert!(info.get("name").is_some(), "server-info kept its existing fields");
        assert!(info.get("version").is_some());

        let _ = std::fs::remove_file(&path);
    }

    /// Spin the REAL relay (real router, real `/ws` handler) on an ephemeral
    /// port with a given manifest. Returns the state (so a test can inspect the
    /// database afterwards) and the port.
    async fn spawn_relay(
        tag: &str,
        features: Features,
    ) -> (std::sync::Arc<crate::relay::relay::RelayState>, u16, std::path::PathBuf) {
        use crate::relay::relay::RelayState;
        use std::sync::Arc;

        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir()
            .join(format!("hum_featws_{tag}_{}_{nanos}.db", std::process::id()));
        let db = crate::relay::storage::Storage::open(&path).expect("open test db");
        let mut state = RelayState::new(db);
        state.features = features;
        let state = Arc::new(state);

        let app = crate::relay::build_router(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        (state, port, path)
    }

    /// Connect to `/ws`, complete the two-phase Dilithium identify handshake,
    /// then send one raw message and return the first reply that mentions
    /// `needle`. The relay floods a freshly-bound socket with peer/channel/
    /// history frames, so we read past those rather than assuming a position.
    async fn identify_then_send(
        port: u16,
        seed: [u8; 32],
        name: &str,
        payload: serde_json::Value,
        needles: &[&str],
    ) -> Option<String> {
        use base64::{engine::general_purpose::STANDARD as B64, Engine};
        use futures::{SinkExt, StreamExt};
        use tokio_tungstenite::tungstenite::Message as WsMsg;

        let dil_seed = crate::relay::core::pq_crypto::derive_dilithium_seed(&seed);
        let dil = crate::relay::core::pq_crypto::DilithiumKeypair::from_seed(&dil_seed);
        let pubkey = hex::encode(dil.public_key());

        let (mut sock, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws"))
            .await
            .expect("client connects to /ws");

        sock.send(WsMsg::Text(
            serde_json::json!({ "type": "identify", "public_key": pubkey, "display_name": name })
                .to_string()
                .into(),
        ))
        .await
        .unwrap();

        // Phase 2: sign the server's nonce challenge.
        let challenge = sock.next().await.unwrap().unwrap().into_text().unwrap();
        let challenge: Value = serde_json::from_str(&challenge).unwrap();
        assert_eq!(challenge["type"], "identify_challenge", "expected a challenge: {challenge}");
        let nonce = challenge["nonce"].as_str().unwrap();
        let sig = B64.encode(dil.sign(format!("hum/identify/v1\n{nonce}\n{pubkey}").as_bytes()));
        sock.send(WsMsg::Text(
            serde_json::json!({ "type": "identify_response", "sig_b64": sig })
                .to_string()
                .into(),
        ))
        .await
        .unwrap();

        sock.send(WsMsg::Text(payload.to_string().into())).await.unwrap();

        // Read past the post-bind flood looking for the reply we care about.
        // Bounded by a timeout so a missing reply fails the test instead of
        // hanging it.
        let deadline = tokio::time::Duration::from_secs(10);
        tokio::time::timeout(deadline, async {
            while let Some(Ok(msg)) = sock.next().await {
                if let Ok(text) = msg.into_text() {
                    if needles.iter().any(|n| text.contains(n)) {
                        return Some(text.to_string());
                    }
                }
            }
            None
        })
        .await
        .unwrap_or(None)
    }

    /// The OTHER door onto the server owner's disk, over the WebSocket.
    ///
    /// `/api/vault/sync` is not the only way to park data on someone else's
    /// relay: `sync_save` accepts up to 512 KB of user data per key over `/ws`.
    /// An owner who said "do not use my relay as a backup" and only got the REST
    /// route gated would still be hosting everybody's blobs, and no HTTP test
    /// would notice. This proves the switch closes BOTH doors — and that the
    /// write genuinely does not land in the database.
    #[tokio::test]
    async fn disabling_relay_backup_also_stops_the_websocket_save_path() {
        // ── Backup OFF: the save is refused and nothing is written.
        let mut off = Features::all_enabled();
        off.set(Feature::VaultBackup, false);
        let (state, port, db_path) = spawn_relay("off", off).await;

        let seed = [23u8; 32];
        let dil_seed = crate::relay::core::pq_crypto::derive_dilithium_seed(&seed);
        let pubkey =
            hex::encode(crate::relay::core::pq_crypto::DilithiumKeypair::from_seed(&dil_seed).public_key());

        let reply = identify_then_send(
            port,
            seed,
            "backupuser",
            serde_json::json!({ "type": "sync_save", "data": "{\"secret\":\"mine\"}" }),
            &["vault_backup", "sync_ack"],
        )
        .await
        .expect("the relay answers a sync_save attempt");
        assert!(
            reply.contains("vault_backup"),
            "a disabled backup must refuse the WS save path, got: {reply}"
        );
        assert!(!reply.contains("sync_ack"), "the save must NOT be acknowledged: {reply}");
        assert!(
            state.db.load_user_data(&pubkey).unwrap_or(None).is_none(),
            "refusing must actually stop the write — nothing may be stored for this key"
        );

        // ── Backup ON: the very same request succeeds. Without this half, a
        //    gate that refused everything would look correct.
        let (on_state, on_port, on_db_path) = spawn_relay("on", Features::all_enabled()).await;
        let reply = identify_then_send(
            on_port,
            seed,
            "backupuser",
            serde_json::json!({ "type": "sync_save", "data": "{\"secret\":\"mine\"}" }),
            &["vault_backup", "sync_ack"],
        )
        .await
        .expect("the relay answers a sync_save attempt");
        assert!(
            reply.contains("sync_ack"),
            "with backup enabled the save must still work, got: {reply}"
        );
        assert!(
            on_state.db.load_user_data(&pubkey).unwrap_or(None).is_some(),
            "with backup enabled the blob must actually be stored"
        );

        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(&on_db_path);
    }

    /// THE COMPLETENESS GATE. Every inbound WS message type must be either
    /// classified by `ws_message_feature` or named in `WS_ALWAYS_ON`.
    ///
    /// WHY THIS EXISTS: `ws_message_feature` returns Option, so an
    /// unclassified type FAILS OPEN. On 2026-08-12 that shipped `market`,
    /// `tasks`, `chat` and `live_video` half-enforced - the REST routes
    /// refused while the same features stayed fully usable over /ws, and a
    /// reviewer proved it by creating a listing on a server with the Market
    /// switched off. No test could have caught it, because the map was
    /// self-consistent; only its INCOMPLETENESS was wrong.
    ///
    /// The authoritative list is the `#[serde(rename = "...")]` values on
    /// RelayMessage, read from the source. A new variant therefore fails this
    /// test until somebody decides which feature owns it - which is the whole
    /// point: the decision becomes mandatory rather than forgotten.
    #[test]
    fn every_inbound_ws_type_is_classified_or_explicitly_always_on() {
        let src = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src").join("relay").join("relay.rs"),
        )
        .expect("relay.rs reads");

        let mut types: Vec<String> = Vec::new();
        for line in src.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("#[serde(rename = \"") {
                if let Some(end) = rest.find('"') {
                    types.push(rest[..end].to_string());
                }
            }
        }
        assert!(
            types.len() > 50,
            "only found {} RelayMessage type tags - the extraction broke, and a              broken extraction would make this gate silently vacuous",
            types.len()
        );

        let mut unclassified: Vec<&str> = types
            .iter()
            .map(|s| s.as_str())
            .filter(|t| ws_message_feature(t).is_none() && !WS_ALWAYS_ON.contains(t))
            .collect();
        unclassified.sort_unstable();
        unclassified.dedup();

        assert!(
            unclassified.is_empty(),
            "these inbound WS message types belong to no feature and are not in              WS_ALWAYS_ON, so they FAIL OPEN - they stay fully usable even when              the owner switches their feature off:
  {}

Classify each in              ws_message_feature, or add it to WS_ALWAYS_ON with a reason.",
            unclassified.join("
  ")
        );
    }

}
