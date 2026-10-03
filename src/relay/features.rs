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
    // The member directory rides the message plane: friend codes, threads,
    // member lists, profiles and link previews are all chat surfaces, and
    // several persist to the owner's disk. (follow/unfollow types died
    // 2026-08-24 with the follows table — the social graph is client-side.)
    if msg_type.starts_with("friend_code_")
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
        | "pin_request" | "dm_put" | "dm_fetch" | "dm_purge"
        // Server→client DM frames (classified so the shared enum's rename
        // strings don't fail open; a client echoing one at the relay is
        // simply gated like any chat message).
        | "dm_new" | "dm_batch" | "dm_purged" => {
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
    // A user controlling their OWN privacy (presence hiding, 2026-08-23).
    // Like account security, privacy controls must never be switch-off-able
    // by a feature toggle - hiding yourself has to work on every server.
    "privacy_update",
    // A user's data sovereignty (2026-08-23): exporting and erasing your
    // own data must work regardless of which features the owner offers.
    // (account_export/account_export_data moved to POST /api/account/export on
    // 2026-09-06; that route is deliberately absent from ROUTE_FEATURES, so it
    // cannot be switched off either.)
    "account_delete",
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
        assert_eq!(ws_message_feature("dm_put"), Some(Feature::Chat));
        assert_eq!(ws_message_feature("dm_fetch"), Some(Feature::Chat));
        assert_eq!(ws_message_feature("dm_purge"), Some(Feature::Chat));
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

    /// Connect to `/ws` and complete the Dilithium identify handshake; returns
    /// the open socket and the identity's key. Waits until the relay lists the
    /// key as signed in, so the caller starts from a bound socket.
    async fn bind_socket(
        state: &std::sync::Arc<crate::relay::relay::RelayState>,
        port: u16,
        seed: [u8; 32],
        name: Option<&str>,
        expect_live: usize,
    ) -> (
        tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
        String,
    ) {
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
            serde_json::json!({ "type": "identify", "public_key": pubkey, "display_name": name }).to_string().into(),
        ))
        .await
        .unwrap();
        let challenge: Value = serde_json::from_str(&sock.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
        let nonce = challenge["nonce"].as_str().expect("an identify challenge").to_string();
        let sig = B64.encode(dil.sign(format!("hum/identify/v1\n{nonce}\n{pubkey}").as_bytes()));
        sock.send(WsMsg::Text(serde_json::json!({ "type": "identify_response", "sig_b64": sig }).to_string().into()))
            .await
            .unwrap();
        let ok = wait_until(|| async { live_count(state, &pubkey).await == expect_live }).await;
        assert!(ok, "the socket never signed in ({expect_live} live expected)");
        (sock, pubkey)
    }

    async fn live_count(state: &std::sync::Arc<crate::relay::relay::RelayState>, key: &str) -> usize {
        state.live_conns.read().await.sockets.get(key).map_or(0, |s| s.len())
    }

    /// Poll for up to 5 s (the relay tears a socket down on its own task).
    async fn wait_until<F, Fut>(mut f: F) -> bool
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        for _ in 0..100 {
            if f().await {
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        false
    }

    /// ONE PERSON, TWO SOCKETS (2026-10-02). Chat and the Tasks board, or the
    /// desktop app and a web tab, sign in with the same identity. Closing the
    /// NEWER one (which owns the registration) must leave the person signed in,
    /// with the registration handed to the socket still open; only closing the
    /// last socket takes them off. Red check, run: without the live set the
    /// newer socket's close removed the registration while the older one stayed
    /// open.
    #[tokio::test]
    async fn closing_one_of_two_sockets_keeps_the_person_signed_in() {
        let (state, port, path) = spawn_relay("two_sockets", Features::all_enabled()).await;
        let seed = [42u8; 32];
        let (mut first, key) = bind_socket(&state, port, seed, Some("TwoTabs"), 1).await;
        let (mut second, _) = bind_socket(&state, port, seed, Some("TwoTabs"), 2).await;

        use futures::SinkExt;
        second.close(None).await.ok();
        assert!(wait_until(|| async { live_count(&state, &key).await == 1 }).await, "the closed socket left the live set");
        {
            let peers = state.peers.read().await;
            let live = state.live_conns.read().await;
            let p = peers.get(&key).expect("still signed in while one socket is open");
            assert!(live.sockets[&key].contains(&p.conn_id), "the registration belongs to the socket still open");
        }

        first.close(None).await.ok();
        assert!(
            wait_until(|| async { !state.peers.read().await.contains_key(&key) }).await,
            "closing the last socket signs the person out"
        );
        assert_eq!(live_count(&state, &key).await, 0);
        let _ = std::fs::remove_file(&path);
    }

    type TestSocket =
        tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

    /// The id of the identity's one live socket (call it while only one is open).
    async fn only_conn(state: &std::sync::Arc<crate::relay::relay::RelayState>, key: &str) -> u64 {
        let live = state.live_conns.read().await;
        let set = &live.sockets[key];
        assert_eq!(set.len(), 1, "expected exactly one live socket");
        *set.iter().next().unwrap()
    }

    async fn send_json(sock: &mut TestSocket, v: Value) {
        use futures::SinkExt;
        sock.send(tokio_tungstenite::tungstenite::Message::Text(v.to_string().into())).await.unwrap();
    }

    /// Send `game_join` the way the desktop client does (`src/lib.rs`) and wait
    /// until the player is in the world with the seat on `conn`.
    async fn join_game(state: &std::sync::Arc<crate::relay::relay::RelayState>, sock: &mut TestSocket, key: &str, conn: u64) {
        send_json(sock, serde_json::json!({ "type": "game_join", "player_name": "Seated", "character_mode": "local" })).await;
        let seated = wait_until(|| async {
            state.game_world.read().await.find_player_entity(key).is_some()
                && state.live_conns.read().await.game_seat.get(key) == Some(&conn)
        })
        .await;
        assert!(seated, "the join never put the player in the world on that socket");
    }

    /// The game departure has started: link-dead under the reconnect grace,
    /// or already despawned when this machine's server config turns it off.
    async fn game_departed(state: &std::sync::Arc<crate::relay::relay::RelayState>, key: &str) -> bool {
        state.link_dead.read().await.contains_key(key)
            || state.game_world.read().await.find_player_entity(key).is_none()
    }

    /// QUITTING THE GAME WITH A WEB TAB OPEN (2026-10-02). Socket A is the
    /// desktop game (it joined the shared world), socket B a web tab for the
    /// same person. Closing A must start the game departure even though B
    /// stays open; before the seat existed the departure waited for the LAST
    /// socket, so the avatar stood in the world for as long as the tab lived.
    /// Seen red 2026-10-02 by deleting the `depart_owned_seats` call from the
    /// teardown in relay.rs: "closing the game socket starts the game
    /// departure" failed after the 5 s wait.
    #[tokio::test]
    async fn closing_the_game_socket_departs_the_game_while_a_tab_stays_open() {
        let (state, port, path) = spawn_relay("game_seat_close", Features::all_enabled()).await;
        let seed = [43u8; 32];
        let (mut game, key) = bind_socket(&state, port, seed, Some("GameAndTab"), 1).await;
        let game_conn = only_conn(&state, &key).await;
        let (mut tab, _) = bind_socket(&state, port, seed, Some("GameAndTab"), 2).await;
        join_game(&state, &mut game, &key, game_conn).await;

        use futures::SinkExt;
        game.close(None).await.ok();
        assert!(
            wait_until(|| async { game_departed(&state, &key).await }).await,
            "closing the game socket starts the game departure"
        );
        assert_eq!(live_count(&state, &key).await, 1, "the tab is still signed in");
        assert!(state.peers.read().await.contains_key(&key), "and so is the person");
        assert!(
            state.live_conns.read().await.game_seat.get(&key).is_none(),
            "the seat went with its socket"
        );

        tab.close(None).await.ok();
        let _ = std::fs::remove_file(&path);
    }

    /// The other half: closing a tab that never joined the game leaves the
    /// game socket's place in the world alone. Seen red 2026-10-02 by making
    /// `depart_owned_seats` depart the game for ANY closing socket (the
    /// `ours` test forced true): the player went link-dead while the game
    /// socket was still open.
    #[tokio::test]
    async fn closing_a_tab_that_never_joined_leaves_the_game_seat_alone() {
        let (state, port, path) = spawn_relay("game_seat_keep", Features::all_enabled()).await;
        let seed = [44u8; 32];
        let (mut game, key) = bind_socket(&state, port, seed, Some("GameKeeps"), 1).await;
        let game_conn = only_conn(&state, &key).await;
        let (mut tab, _) = bind_socket(&state, port, seed, Some("GameKeeps"), 2).await;
        join_game(&state, &mut game, &key, game_conn).await;

        use futures::SinkExt;
        tab.close(None).await.ok();
        // The teardown runs the seat departures before it leaves the live set,
        // so once the count drops, whatever it was going to do is done.
        assert!(wait_until(|| async { live_count(&state, &key).await == 1 }).await, "the tab left");
        assert!(
            state.game_world.read().await.find_player_entity(&key).is_some(),
            "the player is still in the world"
        );
        assert!(!state.link_dead.read().await.contains_key(&key), "and not link-dead");
        assert_eq!(state.live_conns.read().await.game_seat.get(&key), Some(&game_conn), "the seat stays with the game");

        game.close(None).await.ok();
        let _ = std::fs::remove_file(&path);
    }

    /// LEAVING ON PURPOSE IS NOT A DROPPED LINE (2026-10-02). The reconnect
    /// grace holds a player's figure in the world for 90 s after their socket
    /// DROPS, so someone whose internet blinked comes back to their own place.
    /// A player who sends `game_leave` chose to go: everyone should see them
    /// leave now. Until this date `handle_game_leave` ran the dropped-socket
    /// path, so a deliberate leave stood frozen in the world for the full
    /// grace (found by the scripted second player, scripts/second-player.js).
    ///
    /// The grace is pinned to 90 s here, because `RelayState::new` reads it
    /// from this machine's data/server-config.json, which may turn it off.
    ///
    /// Seen red 2026-10-02:
    ///  - `handle_game_leave` put back to calling `handle_game_disconnect` (the
    ///    old code): FAILED at "a deliberate game_leave takes the player out of
    ///    the world at once, not after the reconnect grace".
    ///  - `handle_game_disconnect` made to despawn at once whatever the grace
    ///    (the wrong fix): FAILED at "a dropped socket keeps its place in the
    ///    world for the grace".
    #[tokio::test]
    async fn a_deliberate_leave_despawns_at_once_while_a_dropped_socket_keeps_its_place() {
        use crate::relay::relay::RelayState;
        use std::sync::Arc;

        // spawn_relay, with the grace set before the state is shared.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir()
            .join(format!("hum_featws_leave_vs_drop_{}_{nanos}.db", std::process::id()));
        let db = crate::relay::storage::Storage::open(&path).expect("open test db");
        let mut st = RelayState::new(db);
        st.features = Features::all_enabled();
        st.reconnect_grace = std::time::Duration::from_secs(90);
        let state = Arc::new(st);
        let app = crate::relay::build_router(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        // 1. The leaver joins the world, then steps out on purpose. Its
        //    socket stays open (chat goes on), only the world membership ends.
        let (mut leaver, leaver_key) = bind_socket(&state, port, [45u8; 32], Some("Leaver"), 1).await;
        let leaver_conn = only_conn(&state, &leaver_key).await;
        join_game(&state, &mut leaver, &leaver_key, leaver_conn).await;
        send_json(&mut leaver, serde_json::json!({ "type": "game_leave" })).await;
        assert!(
            wait_until(|| async { state.game_world.read().await.find_player_entity(&leaver_key).is_none() }).await,
            "a deliberate game_leave takes the player out of the world at once, not after the reconnect grace"
        );
        assert!(!state.link_dead.read().await.contains_key(&leaver_key), "and does not hold them as link-dead");
        assert!(state.live_conns.read().await.game_seat.get(&leaver_key).is_none(), "the game seat is given up");
        assert_eq!(live_count(&state, &leaver_key).await, 1, "the leaver's socket is still open");

        // 2. The dropper joins the world, then its socket simply closes, the
        //    way a lost connection looks to the relay. No game_leave.
        let (mut dropper, dropper_key) = bind_socket(&state, port, [46u8; 32], Some("Dropper"), 1).await;
        let dropper_conn = only_conn(&state, &dropper_key).await;
        join_game(&state, &mut dropper, &dropper_key, dropper_conn).await;
        use futures::SinkExt;
        dropper.close(None).await.ok();
        // The teardown runs the seat departures before it leaves the live set,
        // so once the count drops, whatever it was going to do is done.
        assert!(
            wait_until(|| async { live_count(&state, &dropper_key).await == 0 }).await,
            "the relay noticed the dropped socket"
        );
        assert!(
            state.game_world.read().await.find_player_entity(&dropper_key).is_some(),
            "a dropped socket keeps its place in the world for the grace"
        );
        assert!(
            state.link_dead.read().await.contains_key(&dropper_key),
            "held as link-dead, so the sweep collects it if it never comes back"
        );

        leaver.close(None).await.ok();
        let _ = std::fs::remove_file(&path);
    }

    /// A GAME BAN TAKES THE PLAYER OUT OF THE WORLD AT ONCE (2026-10-03,
    /// found by the review of the deliberate-leave fix above). The ban
    /// handler used the dropped-socket path, so with the reconnect grace on
    /// (90 s by default) a banned player was only marked link-dead and kept
    /// their seat: their figure stayed in the world, and their movement kept
    /// reaching everyone, for the whole grace. Their chat socket stays open.
    ///
    /// Seen red 2026-10-03: `handle_game_ban` put back to calling
    /// `handle_game_disconnect` FAILED at "a game ban takes the player out of the world at once, not after
    /// the reconnect grace".
    #[tokio::test]
    async fn a_game_ban_takes_the_player_out_of_the_world_at_once() {
        use crate::relay::relay::RelayState;
        use std::sync::Arc;

        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("hum_featws_ban_{}_{nanos}.db", std::process::id()));
        let db = crate::relay::storage::Storage::open(&path).expect("open test db");
        let mut st = RelayState::new(db);
        st.features = Features::all_enabled();
        st.reconnect_grace = std::time::Duration::from_secs(90);
        let state = Arc::new(st);
        let app = crate::relay::build_router(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let (mut admin, admin_key) = bind_socket(&state, port, [47u8; 32], Some("BanAdmin"), 1).await;
        state.db.set_role(&admin_key, "admin").expect("make admin");
        let (mut target, target_key) = bind_socket(&state, port, [48u8; 32], Some("Banned"), 1).await;
        let target_conn = only_conn(&state, &target_key).await;
        join_game(&state, &mut target, &target_key, target_conn).await;

        send_json(&mut admin, serde_json::json!({ "type": "game_ban", "target": target_key, "reason": "test" })).await;
        assert!(
            wait_until(|| async { state.game_world.read().await.find_player_entity(&target_key).is_none() }).await,
            "a game ban takes the player out of the world at once, not after the reconnect grace"
        );
        assert!(!state.link_dead.read().await.contains_key(&target_key), "not held as link-dead");
        assert!(state.live_conns.read().await.game_seat.get(&target_key).is_none(), "the game seat is given up");
        assert_eq!(live_count(&state, &target_key).await, 1, "their chat socket stays open");

        use futures::SinkExt;
        admin.close(None).await.ok();
        target.close(None).await.ok();
        let _ = std::fs::remove_file(&path);
    }

    /// Voice follows the same rule: the socket that joined a room holds the
    /// voice seat. A tab closing changes nothing; the voice socket closing
    /// takes the person out of the room though another socket stays open.
    /// Seen red 2026-10-02 by deleting the `depart_owned_seats` call from the
    /// teardown: "closing the socket in voice takes the person out of voice"
    /// failed, the roster kept them.
    #[tokio::test]
    async fn closing_the_socket_in_voice_leaves_voice_while_another_stays_open() {
        let (state, port, path) = spawn_relay("voice_seat", Features::all_enabled()).await;
        state.db.create_channel("lounge", "Lounge", None, "test", false).expect("a voice-enabled channel");
        let in_voice = |key: String| {
            let state = state.clone();
            async move {
                state.voice_rooms.read().await.values().any(|r| r.participants.iter().any(|(k, _)| *k == key))
            }
        };
        let seed = [46u8; 32];
        let (mut caller, key) = bind_socket(&state, port, seed, Some("OnACall"), 1).await;
        let caller_conn = only_conn(&state, &key).await;
        let (mut tab, _) = bind_socket(&state, port, seed, Some("OnACall"), 2).await;
        send_json(&mut caller, serde_json::json!({ "type": "voice_room", "action": "join", "room_id": "lounge" })).await;
        assert!(
            wait_until(|| async {
                in_voice(key.clone()).await
                    && state.live_conns.read().await.voice_seat.get(&key) == Some(&caller_conn)
            })
            .await,
            "the join put the person in voice on the caller's socket"
        );

        use futures::SinkExt;
        tab.close(None).await.ok();
        assert!(wait_until(|| async { live_count(&state, &key).await == 1 }).await, "the tab left");
        assert!(in_voice(key.clone()).await, "a tab closing does not end the call");

        let (mut other, _) = bind_socket(&state, port, seed, Some("OnACall"), 2).await;
        caller.close(None).await.ok();
        assert!(
            wait_until(|| async { !in_voice(key.clone()).await }).await,
            "closing the socket in voice takes the person out of voice"
        );
        assert_eq!(live_count(&state, &key).await, 1, "the other socket is still signed in");

        other.close(None).await.ok();
        let _ = std::fs::remove_file(&path);
    }

    /// A NAMELESS SECOND SOCKET (2026-10-02). The web Tasks board signs in
    /// without a name; its socket took over the registration with none, and
    /// the hand-over on the named socket's close kept it, so other clients
    /// showed the person as "Anonymous". The registration must keep the name.
    /// Seen red 2026-10-02 by making `settle_name` return what was offered:
    /// the first assertion failed with `None`.
    #[tokio::test]
    async fn a_nameless_second_socket_keeps_the_persons_name() {
        let (state, port, path) = spawn_relay("nameless_tab", Features::all_enabled()).await;
        let seed = [45u8; 32];
        let name_of = |key: String| {
            let state = state.clone();
            async move { state.peers.read().await.get(&key).and_then(|p| p.display_name.clone()) }
        };
        let (mut chat, key) = bind_socket(&state, port, seed, Some("Named"), 1).await;
        let (mut tasks, _) = bind_socket(&state, port, seed, None, 2).await;
        assert_eq!(
            name_of(key.clone()).await.as_deref(),
            Some("Named"),
            "the nameless socket now owns the registration, under the person's name"
        );

        use futures::SinkExt;
        chat.close(None).await.ok();
        assert!(wait_until(|| async { live_count(&state, &key).await == 1 }).await, "the named socket left");
        assert_eq!(
            name_of(key.clone()).await.as_deref(),
            Some("Named"),
            "the registration handed to the Tasks socket keeps the name"
        );

        tasks.close(None).await.ok();
        let _ = std::fs::remove_file(&path);
    }

    /// A NAMELESS SIGN-IN AFTER A RENAME (review, 2026-10-02). The person
    /// signed in as Aold, later as Znew; then a nameless socket (the Tasks
    /// board) signs in. It must be registered as Znew, and the member row must
    /// stay Znew: the fallback used to take the key's OLDEST registered name
    /// and write it over the member row, undoing the rename. Also: closing the
    /// Znew socket clears the status text saved under Znew.
    /// Seen red 2026-10-02 twice: with `name_for_key` back in `settle_name`
    /// the registration came back "Aold"; with it back in the teardown's
    /// status clear, "the status text was cleared under the name in use"
    /// failed (it cleared Aold's).
    #[tokio::test]
    async fn a_nameless_sign_in_after_a_rename_keeps_the_new_name() {
        let (state, port, path) = spawn_relay("renamed", Features::all_enabled()).await;
        let seed = [47u8; 32];
        use futures::SinkExt;
        let (mut old, key) = bind_socket(&state, port, seed, Some("Aold"), 1).await;
        old.close(None).await.ok();
        assert!(wait_until(|| async { live_count(&state, &key).await == 0 }).await, "Aold left");

        let (mut renamed, _) = bind_socket(&state, port, seed, Some("Znew"), 1).await;
        send_json(&mut renamed, serde_json::json!({ "type": "set_status", "status": "away", "text": "brb" })).await;
        let status = |name: &'static str| {
            let state = state.clone();
            async move { state.db.load_user_status(name).ok().flatten() }
        };
        assert!(
            wait_until(|| async { status("Znew").await == Some(("away".into(), "brb".into())) }).await,
            "the status was saved under Znew"
        );
        renamed.close(None).await.ok();
        assert!(wait_until(|| async { live_count(&state, &key).await == 0 }).await, "Znew left");
        assert!(
            wait_until(|| async { status("Znew").await == Some(("away".into(), String::new())) }).await,
            "the status text was cleared under the name in use"
        );

        let (mut tasks, _) = bind_socket(&state, port, seed, None, 1).await;
        let settled = wait_until(|| async {
            let registered = state.peers.read().await.get(&key).and_then(|p| p.display_name.clone());
            let member = state.db.get_member(&key).ok().flatten().and_then(|m| m.name);
            registered.as_deref() == Some("Znew") && member.as_deref() == Some("Znew")
        })
        .await;
        assert!(
            settled,
            "registration {:?}, member {:?}",
            state.peers.read().await.get(&key).and_then(|p| p.display_name.clone()),
            state.db.get_member(&key).ok().flatten().and_then(|m| m.name)
        );

        tasks.close(None).await.ok();
        let _ = std::fs::remove_file(&path);
    }

    /// Every JSON frame that arrives until the socket has been quiet for `idle_ms`.
    async fn frames_until_quiet(sock: &mut TestSocket, idle_ms: u64) -> Vec<Value> {
        use futures::StreamExt;
        let idle = std::time::Duration::from_millis(idle_ms);
        let mut out = Vec::new();
        while let Ok(Some(Ok(msg))) = tokio::time::timeout(idle, sock.next()).await {
            if let Some(v) = msg.into_text().ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
                out.push(v);
            }
        }
        out
    }

    /// The real relay with a voice-enabled channel, "lounge".
    async fn voice_relay(
        tag: &str,
    ) -> (std::sync::Arc<crate::relay::relay::RelayState>, u16, std::path::PathBuf) {
        let (state, port, path) = spawn_relay(tag, Features::all_enabled()).await;
        state.db.create_channel("lounge", "Lounge", None, "test", false).expect("a voice-enabled channel");
        (state, port, path)
    }

    /// How many times `key` is listed in the lounge (more than once is a bug).
    async fn times_listed(state: &std::sync::Arc<crate::relay::relay::RelayState>, key: &str) -> usize {
        state
            .voice_rooms
            .read()
            .await
            .get("lounge")
            .map_or(0, |r| r.participants.iter().filter(|(k, _)| k == key).count())
    }

    async fn voice_join(sock: &mut TestSocket) {
        send_json(sock, serde_json::json!({ "type": "voice_room", "action": "join", "room_id": "lounge" })).await;
    }

    /// The socket that is not `not`, while exactly two are open.
    async fn other_conn(state: &std::sync::Arc<crate::relay::relay::RelayState>, key: &str, not: u64) -> u64 {
        let live = state.live_conns.read().await;
        *live.sockets[key].iter().find(|c| **c != not).expect("a second socket")
    }

    /// VOICE THROUGH A NETWORK BLIP, part (a) (2026-10-02). Socket A is in
    /// voice; its network blips and the client's new socket B signs in before
    /// the relay notices A closed, then re-sends its voice join (both clients
    /// do this once identify is accepted). The join must move the seat to B
    /// without listing the person twice, and A's close must then leave them in
    /// the room. Before the seat moved, A's close took them off every roster
    /// while their call audio kept playing. Seen red 2026-10-02 by moving the
    /// `take_voice_seat` call in the voice join (msg_handlers.rs) inside the
    /// "not yet listed" branch, so a re-join kept the seat where it was: "the
    /// re-sent join moves the seat to the new socket" failed.
    #[tokio::test]
    async fn a_voice_rejoin_on_a_new_socket_keeps_the_person_in_the_room() {
        let (state, port, path) = voice_relay("voice_rejoin").await;
        let seed = [48u8; 32];
        let (mut a, key) = bind_socket(&state, port, seed, Some("Blip"), 1).await;
        let a_conn = only_conn(&state, &key).await;
        voice_join(&mut a).await;
        assert!(
            wait_until(|| async {
                times_listed(&state, &key).await == 1 && state.live_conns.read().await.voice_seat.get(&key) == Some(&a_conn)
            })
            .await,
            "A joined voice"
        );

        let (mut b, _) = bind_socket(&state, port, seed, Some("Blip"), 2).await;
        let b_conn = other_conn(&state, &key, a_conn).await;
        voice_join(&mut b).await;
        assert!(
            wait_until(|| async { state.live_conns.read().await.voice_seat.get(&key) == Some(&b_conn) }).await,
            "the re-sent join moves the seat to the new socket"
        );
        assert_eq!(times_listed(&state, &key).await, 1, "listed once, not twice");

        use futures::SinkExt;
        a.close(None).await.ok();
        assert!(wait_until(|| async { live_count(&state, &key).await == 1 }).await, "A's close was handled");
        assert_eq!(times_listed(&state, &key).await, 1, "still in the room after A's close");
        assert_eq!(state.live_conns.read().await.voice_seat.get(&key), Some(&b_conn), "on B's seat");

        b.close(None).await.ok();
        let _ = std::fs::remove_file(&path);
    }

    /// VOICE THROUGH A NETWORK BLIP, part (b): what the others in the room
    /// see. A's teardown arrives after B's re-join, so it must leave B's seat
    /// alone, and nobody else may be shown a roster without the person or be
    /// told they are a new participant (that would make them dial again).
    /// Seen red 2026-10-02 by making `give_up_seat` in live_conns.rs report
    /// every seat as the closing socket's: the watcher got a roster without
    /// the person.
    #[tokio::test]
    async fn the_old_sockets_close_after_a_voice_rejoin_changes_nothing_others_see() {
        let (state, port, path) = voice_relay("voice_rejoin_seen").await;
        let (mut watcher, watcher_key) = bind_socket(&state, port, [49u8; 32], Some("Watcher"), 1).await;
        voice_join(&mut watcher).await;
        assert!(wait_until(|| async { times_listed(&state, &watcher_key).await == 1 }).await, "the watcher is in the room");

        let seed = [50u8; 32];
        let (mut a, key) = bind_socket(&state, port, seed, Some("Blinker"), 1).await;
        let a_conn = only_conn(&state, &key).await;
        voice_join(&mut a).await;
        assert!(wait_until(|| async { times_listed(&state, &key).await == 1 }).await, "A joined voice");
        let (mut b, _) = bind_socket(&state, port, seed, Some("Blinker"), 2).await;
        let b_conn = other_conn(&state, &key, a_conn).await;
        // Everything the watcher was sent up to here is before the re-join.
        frames_until_quiet(&mut watcher, 300).await;

        voice_join(&mut b).await;
        assert!(
            wait_until(|| async { state.live_conns.read().await.voice_seat.get(&key) == Some(&b_conn) }).await,
            "B holds the seat"
        );
        use futures::SinkExt;
        a.close(None).await.ok();
        assert!(wait_until(|| async { live_count(&state, &key).await == 1 }).await, "A's close was handled");

        for frame in frames_until_quiet(&mut watcher, 300).await {
            match frame["type"].as_str() {
                Some("voice_channel_list") => {
                    let lounge = frame["channels"].as_array().and_then(|c| c.iter().find(|c| c["id"] == "lounge"));
                    let people = lounge.and_then(|l| l["participants"].as_array()).cloned().unwrap_or_default();
                    let listed = people.iter().any(|p| p["public_key"].as_str() == Some(key.as_str()));
                    let names: Vec<&str> = people.iter().filter_map(|p| p["display_name"].as_str()).collect();
                    assert!(listed, "a roster without the person was broadcast: {names:?}");
                }
                Some("voice_room_signal") => {
                    assert_ne!(frame["signal_type"], "new_participant", "the re-join was announced as new: {frame}");
                }
                _ => {}
            }
        }
        assert_eq!(state.live_conns.read().await.voice_seat.get(&key), Some(&b_conn), "A left B's seat alone");
        assert_eq!(times_listed(&state, &key).await, 1);

        b.close(None).await.ok();
        watcher.close(None).await.ok();
        let _ = std::fs::remove_file(&path);
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
