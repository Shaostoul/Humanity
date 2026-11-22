//! BUG-332's sweep (2026-11-20): the server's owner (role `owner`) counts as an admin in every role
//! check the relay makes, not only in handlers/msg_handlers.rs. Each test below drives the real
//! check with a key holding the owner role (`claim_owner`, as the claim code sets it; `set_role` to
//! "owner" is refused) and, as the control that shows the check is really there, a key with no
//! role, which the same check refuses. Run with the relay battery (`just verify-relay`) and the
//! native lib tests; the socket test binds 127.0.0.1 only.
//!
//! Seen red on 2026-11-20 against the checks before the sweep (each reading "admin", or "admin"
//! or "mod", on its own), every failure collected before the assert so each check shows:
//! - `the_owner_role_resolves_to_the_admin_role_and_ranks_first`: "the owner's role definition
//!   is the Admin row's" (role_def gave the default-deny Unverified row: no uploads, no stream,
//!   280 characters), then "the user list shows Olive as the owner" (got "").
//! - `the_owner_passes_every_rest_route_an_admin_passes`: all ten routes answered 403 to the
//!   owner (admin stats, federation list, bug details, bug status, agent override, delete another
//!   person's review, add and delete an image on another seller's listing, delete another
//!   person's asset, delete another person's shared file).
//! - `the_owner_passes_the_chat_send_and_dm_gates`: "the owner skips the send limiter" (the
//!   second line in a second was refused), "the owner's DM to a stranger is free" (got Knock).
//! - `the_owner_runs_every_admin_command_on_a_real_relay`: "the owner's connection lifts an
//!   automatic lockdown", "/help shows the owner the admin commands", every slash command in
//!   ADMIN_COMMANDS answered "Only admins ..." (or "Only admins and mods ..."), channel_update,
//!   pin_request, delete and delete_by_id of another person's line refused, the 5,000-character
//!   line refused ("exceeds your role's limit of 280"), the 3,000-character edit refused
//!   ("Edit failed: message must be 1-2000 characters").

use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Json, Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message as WsMsg;

use humanity_protocol::chat_access::PostLevel;
use humanity_protocol::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};

use crate::relay::relay::{RelayMessage, RelayState};
use crate::relay::storage::Storage;

const OWNER: u8 = 0x51;
const MEMBER: u8 = 0x52;
const ADMIN: u8 = 0x53;

fn keypair(seed: u8) -> (DilithiumKeypair, String) {
    let kp = DilithiumKeypair::from_seed(&derive_dilithium_seed(&[seed; 32]));
    let key = hex::encode(kp.public_key());
    (kp, key)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64
}

/// `purpose\ntimestamp` signed by the key of `[seed; 32]`, as every signed admin route reads it.
fn signed(seed: u8, purpose: &str) -> (String, u64, String) {
    let (kp, key) = keypair(seed);
    let ts = now_ms();
    let sig = hex::encode(kp.sign(format!("{purpose}\n{ts}").as_bytes()));
    (key, ts, sig)
}

/// A state whose owner is the key of [`OWNER`] and whose other key ([`MEMBER`]) holds no role.
fn owned_state(tag: &str) -> Arc<RelayState> {
    let st = Arc::new(RelayState::new(Storage::open_temp(tag)));
    assert!(st.db.claim_owner(&keypair(OWNER).1, 0).unwrap(), "the owner claims a fresh server");
    st
}

fn code<T>(r: Result<T, (StatusCode, String)>) -> StatusCode {
    match r {
        Ok(_) => StatusCode::OK,
        Err((c, _)) => c,
    }
}

#[test]
fn the_owner_role_resolves_to_the_admin_role_and_ranks_first() {
    let st = owned_state("owner_sweep_storage");
    let db = &st.db;
    let mut failed = Vec::new();
    let (_, owner) = keypair(OWNER);
    let (_, laptop) = keypair(ADMIN);

    // storage/roles.rs role_def: what the owner may upload, stream and post comes from a row.
    let as_value = |r: crate::relay::storage::RoleDef| serde_json::to_value(r).unwrap();
    if as_value(db.role_def("owner")) != as_value(db.role_def("admin")) {
        failed.push(format!("the owner's role definition is the Admin row's: {}", as_value(db.role_def("owner"))));
    }
    if db.limit_tier_for_role("owner") != "admin" {
        failed.push(format!("the owner's limit tier is admin: {}", db.limit_tier_for_role("owner")));
    }

    // storage/channels.rs list_all_users and storage/pins.rs list_all_users_with_keys: the role a
    // name shows is its highest, the owner's above an admin's (the owner's linked laptop is an admin).
    db.register_name("Olive", &owner).unwrap();
    let shown = |db: &Storage| -> (String, String) {
        let a = db.list_all_users().unwrap().into_iter().find(|u| u.0 == "Olive").map(|u| u.1).unwrap_or_default();
        let b = db.list_all_users_with_keys().unwrap().into_iter().find(|u| u.0 == "Olive").map(|u| u.2).unwrap_or_default();
        (a, b)
    };
    let (a, b) = shown(db);
    if a != "owner" || b != "owner" {
        failed.push(format!("the user list shows Olive as the owner: {a:?} / {b:?}"));
    }
    db.register_name("Olive", &laptop).unwrap();
    db.set_role(&laptop, "admin").unwrap();
    let (a, b) = shown(db);
    if a != "owner" || b != "owner" {
        failed.push(format!("beside her admin laptop Olive still shows as the owner: {a:?} / {b:?}"));
    }
    assert!(failed.is_empty(), "the owner was not read as an admin:\n{}", failed.join("\n"));
}

#[test]
fn the_owner_passes_every_rest_route_an_admin_passes() {
    use crate::relay::api;
    let rt = tokio::runtime::Runtime::new().unwrap();
    let st = owned_state("owner_sweep_rest");
    let (_, seller) = keypair(ADMIN);
    let mut failed = Vec::new();
    // Each route as `seed` signs it: the status it answers.
    let mut check = |what: &str, owner_code: StatusCode, member_code: StatusCode, owner_ok: &[StatusCode]| {
        if member_code != StatusCode::FORBIDDEN {
            failed.push(format!("{what}: the control, a member, is refused with 403, got {member_code}"));
        }
        if !owner_ok.contains(&owner_code) {
            failed.push(format!("{what}: the owner passes as an admin does, got {owner_code}"));
        }
    };

    // api.rs post_admin_stats (Admin role required).
    let stats = |seed: u8| {
        let (key, timestamp, sig) = signed(seed, "admin_stats");
        let q: api::AdminStatsQuery = serde_json::from_value(json!({ "key": key, "timestamp": timestamp, "sig": sig })).unwrap();
        code(rt.block_on(api::post_admin_stats(State(st.clone()), Json(q))))
    };
    check("admin stats", stats(OWNER), stats(MEMBER), &[StatusCode::OK]);

    // api.rs post_admin_federation (Admin role required), the read-only "list".
    let federation = |seed: u8| {
        let (key, timestamp, sig) = signed(seed, "admin_federation");
        let q: api::AdminFederationRequest =
            serde_json::from_value(json!({ "key": key, "timestamp": timestamp, "sig": sig, "action": "list" })).unwrap();
        code(rt.block_on(api::post_admin_federation(State(st.clone()), Json(q))))
    };
    check("federation peers", federation(OWNER), federation(MEMBER), &[StatusCode::OK]);

    // api.rs bug_details (admins and moderators).
    let details = |seed: u8| {
        let (key, timestamp, sig) = signed(seed, api::BUG_DETAILS_PURPOSE);
        let q: api::BugDetailsRequest = serde_json::from_value(json!({ "key": key, "timestamp": timestamp, "sig": sig })).unwrap();
        code(rt.block_on(api::bug_details(State(st.clone()), Json(q))))
    };
    check("bug report details", details(OWNER), details(MEMBER), &[StatusCode::OK]);

    // api.rs update_bug_status (admins and moderators): a status the route does not know, so a
    // caller past the role check gets 400 and nothing is written.
    let status = |seed: u8| {
        let (key, timestamp, sig) = signed(seed, "bug_status");
        let q: api::UpdateBugRequest =
            serde_json::from_value(json!({ "status": "not-a-status", "key": key, "timestamp": timestamp, "sig": sig })).unwrap();
        code(rt.block_on(api::update_bug_status(State(st.clone()), Path(1), Json(q))))
    };
    check("bug status", status(OWNER), status(MEMBER), &[StatusCode::BAD_REQUEST]);

    // api_v2_agents.rs set_agent_override (Admin role required): a scope id the route refuses, so
    // a caller past the role check gets 400 and data/coordination/overrides.ron is never written.
    let agent = |seed: u8| {
        let (key, timestamp, sig) = signed(seed, "agent_override");
        let q: crate::relay::api_v2_agents::OverrideRequest = serde_json::from_value(
            json!({ "scope_id": "not a scope", "status": "off", "key": key, "timestamp": timestamp, "sig": sig }),
        )
        .unwrap();
        rt.block_on(crate::relay::api_v2_agents::set_agent_override(State(st.clone()), Json(q))).into_response().status()
    };
    check("agent override", agent(OWNER), agent(MEMBER), &[StatusCode::BAD_REQUEST]);

    // api_market.rs delete_listing_review (admins and moderators): another person's review.
    st.db.create_listing("lamp", &seller, "Sel", "Lamp", "A lamp", "home", "used", "5", "cash", "here").unwrap();
    let review = |seed: u8| {
        let id = st.db.create_review("lamp", &format!("reviewer-{seed}"), "Rev", 4, "fine").unwrap();
        let (kp, key) = keypair(seed);
        let timestamp = now_ms();
        let sig = hex::encode(kp.sign(format!("review_delete\n{id}\n{timestamp}").as_bytes()));
        let q: api::VaultSyncQuery = serde_json::from_value(json!({ "key": key, "timestamp": timestamp, "sig": sig })).unwrap();
        code(rt.block_on(api::delete_listing_review(State(st.clone()), Path(("lamp".to_string(), id)), Query(q))))
    };
    check("delete another person's review", review(OWNER), review(MEMBER), &[StatusCode::OK]);

    // api_market.rs add_listing_image and delete_listing_image (admins and moderators), on another
    // seller's listing, through each caller's upload token. An address the route refuses and an
    // image that is not there: past the role check they answer 400 and 404, before it 403.
    let token = |seed: u8| {
        let t = format!("owner-sweep-token-{seed}");
        rt.block_on(st.upload_tokens.write()).insert(t.clone(), keypair(seed).1);
        serde_json::from_value::<api::UploadQuery>(json!({ "token": t })).unwrap()
    };
    let add_image = |seed: u8| {
        code(rt.block_on(api::add_listing_image(State(st.clone()), Path("lamp".to_string()), Query(token(seed)), Json(json!({ "url": "" })))))
    };
    check("add an image to another seller's listing", add_image(OWNER), add_image(MEMBER), &[StatusCode::OK, StatusCode::BAD_REQUEST]);
    let delete_image = |seed: u8| {
        code(rt.block_on(api::delete_listing_image(State(st.clone()), Path(("lamp".to_string(), 999_999)), Query(token(seed)))))
    };
    check("delete an image from another seller's listing", delete_image(OWNER), delete_image(MEMBER), &[StatusCode::NOT_FOUND]);

    // api.rs delete_asset (admins and moderators): another person's asset.
    let asset = |seed: u8| {
        let id = format!("asset-{seed}");
        st.db.create_asset(&id, &seller, "a.png", "image", "art", "[]", 1, "/uploads/a.png", "").unwrap();
        let q: api::AssetDeleteQuery = serde_json::from_value(json!({ "token": format!("owner-sweep-token-{seed}") })).unwrap();
        let got = code(rt.block_on(api::delete_asset(State(st.clone()), Path(id), Query(q))));
        // A refused delete answers 404 ("not found or not authorized"): read the row instead.
        if got == StatusCode::NOT_FOUND { StatusCode::FORBIDDEN } else { got }
    };
    check("delete another person's asset", asset(OWNER), asset(MEMBER), &[StatusCode::OK]);

    // api.rs delete_shared_upload (Admin role required): another person's shared file. The name is
    // this test's own, so the best-effort unlink beside the row finds nothing to remove.
    let shared = |seed: u8| {
        let name = format!("owner-sweep-{seed}-{}.txt", now_ms());
        st.db
            .with_conn(|c| c.execute("INSERT INTO user_uploads (public_key, filename, uploaded_at, shared) VALUES (?1, ?2, 0, 1)", [&seller, &name]))
            .unwrap();
        let (key, timestamp, sig) = signed(seed, "delete_upload");
        let q: api::DeleteUploadBody = serde_json::from_value(json!({ "filename": name, "key": key, "timestamp": timestamp, "sig": sig })).unwrap();
        code(rt.block_on(api::delete_shared_upload(State(st.clone()), Json(q))))
    };
    check("delete another person's shared file", shared(OWNER), shared(MEMBER), &[StatusCode::OK]);

    assert!(failed.is_empty(), "the owner was refused where an admin passes:\n{}", failed.join("\n"));
}

#[test]
fn the_owner_passes_the_chat_send_and_dm_gates() {
    use crate::relay::handlers::reach::{dm_gate, Cost, DmAsk};
    let rt = tokio::runtime::Runtime::new().unwrap();
    let st = owned_state("owner_sweep_gates");
    let (_, owner) = keypair(OWNER);
    let (_, member) = keypair(MEMBER);
    let (_, stranger) = keypair(ADMIN);
    let mut failed = Vec::new();

    // handlers/chat_rate.rs allow: bots and admins are not limited; a member's second line in the
    // same second waits (the control).
    let twice = |key: &str, role: &str| {
        rt.block_on(crate::relay::handlers::chat_rate::allow(&st, key, role)) && rt.block_on(crate::relay::handlers::chat_rate::allow(&st, key, role))
    };
    if twice(&member, "") {
        failed.push("the control: a member's second line in a second waits".into());
    }
    if !twice(&owner, "owner") {
        failed.push("the owner skips the send limiter".into());
    }

    // handlers/reach.rs dm_gate: an admin's or moderator's DM to someone who takes messages from
    // strangers is free; a member's costs a knock (the control).
    st.db.set_reach_settings(&stranger, &[("message", "anyone")]).unwrap();
    let cost =|sender: &str, role: &str| dm_gate(&st, sender, &stranger, None, DmAsk::Ordinary, role, None);
    if cost(&member, "") != Some(Cost::Knock) {
        failed.push(format!("the control: a member's DM to a stranger costs a knock, got {:?}", cost(&member, "")));
    }
    if cost(&owner, "owner") != Some(Cost::Free) {
        failed.push(format!("the owner's DM to a stranger is free, got {:?}", cost(&owner, "owner")));
    }
    assert!(failed.is_empty(), "the owner was held where an admin is not:\n{}", failed.join("\n"));
}

// ── The socket half: relay.rs's slash commands and message arms ──

type Sock = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// How long a frame may take to come.
const BOUND: Duration = Duration::from_secs(10);

/// Every slash command whose role check the sweep changed, typed bare: the check comes before the
/// arguments are read, so a caller it lets through gets the usage line or the command's own answer,
/// and one it refuses gets "Only admins ..." (or "Only admins and mods ..."). `/gc` is last: it may
/// clear the test's names.
const ADMIN_COMMANDS: &[&str] = &[
    "/channel-create", "/channel-edit", "/channel-delete", "/channel-readonly", "/channel-reorder", "/invite",
    "/name-release", "/verify", "/donor", "/unverify", "/pin", "/unpin", "/server-add", "/server-add-key",
    "/server-remove", "/server-recheck", "/server-trust", "/server-federate", "/server-connect", "/category-create",
    "/category-delete", "/category-rename", "/channel-category", "/wipe", "/wipe-all", "/lockdown", "/gc",
];

async fn frame_where(sock: &mut Sock, what: &str, want: impl Fn(&Value) -> bool) -> Value {
    let found = tokio::time::timeout(BOUND, async {
        while let Some(Ok(msg)) = sock.next().await {
            if let Ok(Ok(f)) = msg.into_text().map(|t| serde_json::from_str::<Value>(&t)) {
                if want(&f) {
                    return Some(f);
                }
            }
        }
        None
    })
    .await;
    found.ok().flatten().unwrap_or_else(|| panic!("never came: {what}"))
}

/// Sign in with the key of `[seed; 32]` through /ws and the identify handshake, as `name`, made old
/// enough to post (a new account waits a minute).
async fn sign_in(st: &Arc<RelayState>, port: u16, seed: u8, name: &str) -> Sock {
    let (kp, key) = keypair(seed);
    let (mut sock, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws")).await.unwrap();
    let hello = json!({ "type": "identify", "public_key": key, "display_name": name });
    sock.send(WsMsg::Text(hello.to_string().into())).await.unwrap();
    let challenge = frame_where(&mut sock, "a challenge", |f| f["type"] == "identify_challenge").await;
    let nonce = challenge["nonce"].as_str().unwrap().to_string();
    let sig = B64.encode(kp.sign(format!("hum/identify/v1\n{nonce}\n{key}").as_bytes()));
    sock.send(WsMsg::Text(json!({ "type": "identify_response", "sig_b64": sig }).to_string().into())).await.unwrap();
    frame_where(&mut sock, "signed in", |f| f["type"] == "peer_list" || f["type"] == "channel_list").await;
    st.db.with_conn(|c| c.execute("UPDATE registered_names SET registered_at = 0 WHERE public_key = ?1", [&key])).unwrap();
    sock
}

/// A chat line `content` in #global as the key of `[seed; 32]`, signed as the apps sign one; its
/// time. The send limiter is cleared first: it is not what this test is about.
async fn post(st: &Arc<RelayState>, sock: &mut Sock, seed: u8, content: &str) -> u64 {
    st.rate_limits.write().await.clear();
    let (kp, key) = keypair(seed);
    let ts = now_ms();
    let sig = hex::encode(kp.sign(format!("{content}\n{ts}").as_bytes()));
    let frame = json!({ "type": "chat", "from": key, "content": content, "timestamp": ts, "channel": "global", "pq_signature": sig });
    sock.send(WsMsg::Text(frame.to_string().into())).await.unwrap();
    ts
}

/// The words a socket is told after sending `frame` (a `Private` reaches the socket it names as a
/// `system` frame), up to the answer to an edit in a room that does not exist, which every caller
/// gets and which comes after everything `frame` said (one socket's frames are handled in order).
async fn told_after(sock: &mut Sock, frame: Value) -> Vec<String> {
    const MARK: &str = "Edit failed: only messages in this server's rooms can be edited.";
    sock.send(WsMsg::Text(frame.to_string().into())).await.unwrap();
    let mark = json!({ "type": "edit", "from": "", "timestamp": 1, "new_content": "x", "channel": "owner-sweep-no-such-room" });
    sock.send(WsMsg::Text(mark.to_string().into())).await.unwrap();
    let mut said = Vec::new();
    let done = tokio::time::timeout(BOUND, async {
        while let Some(Ok(msg)) = sock.next().await {
            let Ok(Ok(f)) = msg.into_text().map(|t| serde_json::from_str::<Value>(&t)) else { continue };
            if f["type"] == "system" {
                let words = f["message"].as_str().unwrap_or_default().to_string();
                if words == MARK {
                    return true;
                }
                said.push(words);
            }
        }
        false
    })
    .await;
    assert_eq!(done, Ok(true), "the mark's answer came");
    said
}

/// [`told_after`] for a chat line.
async fn told_after_line(st: &Arc<RelayState>, sock: &mut Sock, seed: u8, content: &str) -> Vec<String> {
    st.rate_limits.write().await.clear();
    let (kp, key) = keypair(seed);
    let ts = now_ms();
    let sig = hex::encode(kp.sign(format!("{content}\n{ts}").as_bytes()));
    told_after(sock, json!({ "type": "chat", "from": key, "content": content, "timestamp": ts, "channel": "global", "pq_signature": sig })).await
}

fn refused(said: &[String], how: &str) -> bool {
    said.iter().any(|w| w.starts_with(how))
}

/// Store a line in #global as `from` at `timestamp`; its row id.
fn store(st: &Arc<RelayState>, from: &str, timestamp: u64) -> i64 {
    let chat = RelayMessage::Chat {
        from: from.into(),
        from_name: Some("someone".into()),
        content: format!("line {timestamp}"),
        timestamp,
        signature: None,
        channel: "global".into(),
        reply_to: None,
        thread_count: None,
        message_id: None,
    };
    st.db.store_message_in_channel(&chat, "global").unwrap()
}

fn stored_at(st: &Arc<RelayState>, timestamp: u64) -> bool {
    st.db
        .load_channel_messages("global", 500)
        .unwrap()
        .into_iter()
        .any(|m| matches!(m, RelayMessage::Chat { timestamp: t, .. } if t == timestamp))
}

#[test]
fn the_owner_runs_every_admin_command_on_a_real_relay() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let st = crate::relay::serve_relay(Storage::open_temp("owner_sweep_ws"), listener).await;
        crate::relay::default_channels::make_missing(&st.db, true);
        st.db.create_channel("lounge", "lounge", None, "test", PostLevel::Members).unwrap();
        let (_, owner) = keypair(OWNER);
        let (_, member) = keypair(MEMBER);
        assert!(st.db.claim_owner(&owner, 0).unwrap());
        let mut failed: Vec<String> = Vec::new();

        // relay.rs, the identify arm: a moderator's or admin's connection lifts a lockdown that was
        // set automatically. The owner signs in once to take her name, then again into a lockdown.
        drop(sign_in(&st, port, OWNER, "Olive").await);
        *st.lockdown.write().await = true;
        *st.auto_lockdown.write().await = true;
        let mut o = sign_in(&st, port, OWNER, "Olive").await;
        let mut lifted = false;
        for _ in 0..50 {
            if !*st.lockdown.read().await {
                lifted = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        if !lifted {
            failed.push("the owner's connection lifts an automatic lockdown".into());
        }
        *st.lockdown.write().await = false;
        *st.auto_lockdown.write().await = false;
        let mut m = sign_in(&st, port, MEMBER, "Milo").await;

        // relay.rs /help: the moderator and admin commands are listed for those who may run them.
        let help = told_after_line(&st, &mut o, OWNER, "/help").await.join("\n");
        if !help.contains("Admin commands:") || !help.contains("Moderator commands:") {
            failed.push(format!("/help shows the owner the admin commands: {help}"));
        }

        // chat_gate.rs: the role's length limit (the Admin row's, through role_def) and the admins'
        // 10,000 characters; a member's 5,000 is refused (the control).
        for (seed, sock, who) in [(MEMBER, &mut m, "member"), (OWNER, &mut o, "owner")] {
            let ts = post(&st, sock, seed, &"o".repeat(5_000)).await;
            let answer = frame_where(sock, "the answer to the long line", |f| {
                (f["type"] == "chat" || f["type"] == "chat_refused") && f["timestamp"].as_u64() == Some(ts)
            })
            .await;
            let posted = answer["type"] == "chat";
            match who {
                "member" if posted => failed.push("the control: a member's 5,000-character line is refused".into()),
                "owner" if !posted => failed.push(format!("the owner posts a 5,000-character line as an admin does: {answer}")),
                _ => {}
            }
        }

        // relay.rs, the edit arm and /edit: the admins' 10,000 characters.
        let ts = post(&st, &mut o, OWNER, "short").await;
        frame_where(&mut o, "the short line", |f| f["type"] == "chat" && f["timestamp"].as_u64() == Some(ts)).await;
        let said = told_after(&mut o, json!({ "type": "edit", "from": owner, "timestamp": ts, "new_content": "e".repeat(3_000), "channel": "global" })).await;
        if refused(&said, "Edit failed") {
            failed.push(format!("the owner edits a line to 3,000 characters as an admin does: {said:?}"));
        }
        let said = told_after_line(&st, &mut o, OWNER, &format!("/edit {}", "f".repeat(3_000))).await;
        if refused(&said, "Message too long") || refused(&said, "Message rejected") {
            failed.push(format!("/edit takes the owner's 3,000 characters as an admin's: {said:?}"));
        }

        // relay.rs, the channel_update and pin_request arms.
        let update = json!({ "type": "channel_update", "channel_id": "lounge", "description": "the owner's" });
        let pin = json!({ "type": "pin_request", "from_key": member, "from_name": "Milo", "content": "pin me", "timestamp": 7, "channel": "global" });
        for (what, frame) in [("channel_update", update), ("pin_request", pin)] {
            if !refused(&told_after(&mut m, frame.clone()).await, "Only ") {
                failed.push(format!("the control: a member's {what} is refused"));
            }
            let said = told_after(&mut o, frame).await;
            if refused(&said, "Only ") {
                failed.push(format!("the owner's {what} passes as an admin's: {said:?}"));
            }
        }

        // relay.rs, the delete and delete_by_id arms: another person's line.
        let (theirs, mine) = (now_ms() - 10_000, now_ms() - 9_000);
        store(&st, &member, theirs);
        store(&st, &owner, mine);
        let said = told_after(&mut m, json!({ "type": "delete", "from": owner, "timestamp": mine })).await;
        if !refused(&said, "You can only delete your own messages.") || !stored_at(&st, mine) {
            failed.push("the control: a member cannot delete another person's line".into());
        }
        let said = told_after(&mut o, json!({ "type": "delete", "from": member, "timestamp": theirs })).await;
        if refused(&said, "You can only delete") || stored_at(&st, theirs) {
            failed.push(format!("the owner deletes another person's line as an admin does: {said:?}"));
        }
        let (theirs, mine) = (now_ms() - 8_000, now_ms() - 7_000);
        let their_id = store(&st, &member, theirs);
        let my_id = store(&st, &owner, mine);
        let said = told_after(&mut m, json!({ "type": "delete_by_id", "message_id": my_id })).await;
        if !refused(&said, "Cannot delete") || !stored_at(&st, mine) {
            failed.push("the control: a member cannot delete another person's line by its id".into());
        }
        let said = told_after(&mut o, json!({ "type": "delete_by_id", "message_id": their_id })).await;
        if refused(&said, "Cannot delete") || stored_at(&st, theirs) {
            failed.push(format!("the owner deletes another person's line by its id as an admin does: {said:?}"));
        }

        // relay.rs, the slash commands: the member first (the control), then the owner.
        for cmd in ADMIN_COMMANDS {
            if !refused(&told_after_line(&st, &mut m, MEMBER, cmd).await, "Only ") {
                failed.push(format!("the control: a member running {cmd} is refused"));
            }
        }
        for cmd in ADMIN_COMMANDS {
            let said = told_after_line(&st, &mut o, OWNER, cmd).await;
            if refused(&said, "Only ") {
                failed.push(format!("the owner runs {cmd} as an admin does: {said:?}"));
            }
        }
        assert!(failed.is_empty(), "the owner was refused where an admin passes:\n{}", failed.join("\n"));
    });
}
