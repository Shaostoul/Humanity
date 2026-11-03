//! The chat access levels' tests (2026-11-02, docs/design/chat-access.md), run with the relay
//! battery (`just verify-relay`): a real relay on loopback (127.0.0.1 only, `serve_relay`), people
//! signing in through the real /ws handshake and posting PQ-signed lines, and history read through
//! the real routes, signed or not.

use std::sync::Arc;
use std::time::Duration;

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message as WsMsg;

use humanity_protocol::chat_access::{read_purpose, PostLevel, ReadLevel};
use humanity_protocol::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};

use crate::relay::relay::{RelayMessage, RelayState};
use crate::relay::storage::Storage;

type Sock = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// How long a frame may take to come.
const BOUND: Duration = Duration::from_secs(10);

struct Relay {
    state: Arc<RelayState>,
    port: u16,
}

impl Relay {
    fn base(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

/// A relay on loopback with #general (Anyone / Members, as a fresh server seeds it), #lounge
/// (a new channel: Members / Members), #vip (Verified / Verified) and #announcements (Anyone /
/// Moderators and admins).
async fn relay(tag: &str) -> Relay {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let state = crate::relay::serve_relay(Storage::open_temp(tag), listener).await;
    state.db.ensure_default_channel().unwrap();
    state.db.create_channel("lounge", "lounge", None, "test", PostLevel::Members).unwrap();
    state.db.create_channel("vip", "vip", None, "test", PostLevel::Members).unwrap();
    state.db.set_channel_access("vip", ReadLevel::Verified, PostLevel::Verified).unwrap();
    state.db.create_channel("announcements", "announcements", None, "test", PostLevel::Mods).unwrap();
    state.db.set_channel_access("announcements", ReadLevel::Anyone, PostLevel::Mods).unwrap();
    Relay { state, port }
}

fn keypair(seed: u8) -> (DilithiumKeypair, String) {
    let kp = DilithiumKeypair::from_seed(&derive_dilithium_seed(&[seed; 32]));
    let key = hex::encode(kp.public_key());
    (kp, key)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64
}

/// Sign in to `r` with the key of `[seed; 32]` through /ws and the identify handshake, as `name`
/// (None: a socket that chooses no name, as the Tasks board signs in). A named account is made old
/// enough to post (a new one waits a minute).
async fn sign_in(r: &Relay, seed: u8, name: Option<&str>) -> Sock {
    let (kp, key) = keypair(seed);
    let (mut sock, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{}/ws", r.port)).await.unwrap();
    let mut hello = json!({ "type": "identify", "public_key": key });
    if let Some(n) = name {
        hello["display_name"] = json!(n);
    }
    sock.send(WsMsg::Text(hello.to_string().into())).await.unwrap();
    let challenge = frame_where(&mut sock, "a challenge", |f| f["type"] == "identify_challenge").await;
    let nonce = challenge["nonce"].as_str().unwrap().to_string();
    let sig = B64.encode(kp.sign(format!("hum/identify/v1\n{nonce}\n{key}").as_bytes()));
    sock.send(WsMsg::Text(json!({ "type": "identify_response", "sig_b64": sig }).to_string().into())).await.unwrap();
    frame_where(&mut sock, "signed in", |f| f["type"] == "peer_list" || f["type"] == "channel_list").await;
    r.state.db.with_conn(|c| c.execute("UPDATE registered_names SET registered_at = 0 WHERE public_key = ?1", [&key])).unwrap();
    sock
}

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

/// Post `content` in `channel` as the key of `[seed; 32]`, signed as the apps sign chat; its time.
/// The send limiter (handlers/chat_rate.rs, a second between lines and more) is cleared first: it is
/// not what these tests are about, and each refused line counts against it too.
async fn post(r: &Relay, sock: &mut Sock, seed: u8, channel: &str, content: &str) -> u64 {
    r.state.rate_limits.write().await.clear();
    let (kp, key) = keypair(seed);
    let ts = now_ms();
    let sig = hex::encode(kp.sign(format!("{content}\n{ts}").as_bytes()));
    let frame = json!({ "type": "chat", "from": key, "content": content, "timestamp": ts, "channel": channel, "pq_signature": sig });
    sock.send(WsMsg::Text(frame.to_string().into())).await.unwrap();
    ts
}

/// The server's answer to my line at `ts`: my own line back (posted) or `chat_refused` (with why).
async fn answer_to(sock: &mut Sock, ts: u64) -> Value {
    frame_where(sock, "the answer to my line", |f| {
        (f["type"] == "chat" || f["type"] == "chat_refused") && f["timestamp"].as_u64() == Some(ts)
    })
    .await
}

/// The words of every line stored in `channel`.
fn stored(r: &Relay, channel: &str) -> Vec<String> {
    r.state
        .db
        .load_channel_messages(channel, 100)
        .unwrap()
        .into_iter()
        .filter_map(|m| if let RelayMessage::Chat { content, .. } = m { Some(content) } else { None })
        .collect()
}

/// Store a line in `channel` as `from` at `timestamp`, as the chat arm stores one.
fn store(r: &Relay, channel: &str, from: &str, content: &str, timestamp: u64) {
    let chat = RelayMessage::Chat {
        from: from.into(),
        from_name: Some("someone".into()),
        content: content.into(),
        timestamp,
        // A signature as a poster's app sends one: a visitor must not be given it (it names its key).
        signature: Some(format!("sig-{timestamp}")),
        channel: channel.into(),
        reply_to: None,
        thread_count: None,
        message_id: None,
    };
    r.state.db.store_message_in_channel(&chat, channel).unwrap();
}

/// A signed read of `what` in `channel` by the key of `[seed; 32]`: the status and the body.
async fn signed_read(r: &Relay, seed: u8, what: &str, channel: &str) -> (u16, Value) {
    let (kp, key) = keypair(seed);
    let ts = now_ms();
    let sig = hex::encode(kp.sign(format!("{}\n{ts}", read_purpose(what, channel)).as_bytes()));
    let res = reqwest::Client::new()
        .post(format!("{}/api/{what}", r.base()))
        .json(&json!({ "key": key, "timestamp": ts, "sig": sig, "channel": channel }))
        .send()
        .await
        .unwrap();
    let status = res.status().as_u16();
    (status, res.json().await.unwrap_or(Value::Null))
}

async fn visitor_read(r: &Relay, what: &str, channel: &str) -> (u16, Value) {
    let res = reqwest::get(format!("{}/api/{what}?channel={channel}", r.base())).await.unwrap();
    let status = res.status().as_u16();
    (status, res.json().await.unwrap_or(Value::Null))
}

fn contents(body: &Value) -> Vec<String> {
    body["messages"].as_array().map(|m| m.iter().map(|x| x["content"].as_str().unwrap_or("").to_string()).collect()).unwrap_or_default()
}

/// Run `test` on a thread with a 64 MB stack and a runtime of its own: Dilithium steps and `json!`
/// in a debug build overflow a test thread's 2 MB (federation_phase3_tests.rs, 2026-10-22).
fn on_a_big_stack<F, Fut>(test: F)
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()>,
{
    let run = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread().enable_all().thread_stack_size(16 << 20).build().unwrap();
            rt.block_on(test());
        })
        .unwrap()
        .join();
    if let Err(e) = run {
        std::panic::resume_unwind(e);
    }
}

/// A MEMBERS CHANNEL IS REFUSED TO A REQUEST WITH NO KEY AND SERVED TO A MEMBER; A VERIFIED
/// CHANNEL IS REFUSED TO AN UNVERIFIED MEMBER. Through the real routes: GET /api/messages for
/// #lounge (Members) answers 401 naming the level; a member's signed POST gets its posts; a key
/// with no chosen name here, signing, is refused 403; #vip (Verified) refuses an unverified
/// member's signed POST with 403 and serves a verified one; pins and reactions follow the same
/// rule; a signature over another channel's words is refused.
/// 
/// Seen red 2026-11-02, one break at a time: with `open_to_visitors` answering every channel, "no
/// key, no Members channel" (left: 200, right: 401, its post in the body); with `signed_reader`
/// letting any member read a Verified channel, "an unverified member does not read a Verified
/// channel" (left: 200, right: 403).
#[test]
fn a_members_channel_is_read_only_by_members_and_a_verified_one_only_by_the_verified() {
    on_a_big_stack(|| async {
        let r = relay("access_read").await;
        let (_, ann) = keypair(1);
        r.state.db.register_name("ann", &ann).unwrap();
        let (_, vic) = keypair(2);
        r.state.db.register_name("vic", &vic).unwrap();
        r.state.db.set_role(&vic, "verified").unwrap();
        let (_, _nameless) = keypair(3);
        store(&r, "lounge", &ann, "members only words", now_ms());
        store(&r, "vip", &vic, "verified only words", now_ms());

        let (status, body) = visitor_read(&r, "messages", "lounge").await;
        assert_eq!(status, 401, "no key, no Members channel: {body}");
        assert_eq!(body["read_level"], "members");
        assert!(contents(&body).is_empty(), "and none of its words");
        let (status, body) = signed_read(&r, 1, "messages", "lounge").await;
        assert_eq!(status, 200, "a member reads it: {body}");
        assert_eq!(contents(&body), vec!["members only words"]);
        let (status, _) = signed_read(&r, 3, "messages", "lounge").await;
        assert_eq!(status, 403, "a key that chose no name here is no member");

        let (status, body) = signed_read(&r, 1, "messages", "vip").await;
        assert_eq!(status, 403, "an unverified member does not read a Verified channel: {body}");
        assert_eq!(body["read_level"], "verified");
        let (status, body) = signed_read(&r, 2, "messages", "vip").await;
        assert_eq!(status, 200, "a verified member does: {body}");
        assert_eq!(contents(&body), vec!["verified only words"]);

        for what in ["pins", "reactions"] {
            assert_eq!(visitor_read(&r, what, "lounge").await.0, 401, "{what}: no key, no Members channel");
            assert_eq!(signed_read(&r, 1, what, "lounge").await.0, 200, "{what}: a member reads them");
            assert_eq!(signed_read(&r, 1, what, "vip").await.0, 403, "{what}: not a Verified channel's below Verified");
        }

        // A signature is for one channel: ann's words for #lounge do not open #vip for vic's key.
        let (kp, key) = keypair(1);
        let ts = now_ms();
        let sig = hex::encode(kp.sign(format!("{}\n{ts}", read_purpose("messages", "lounge")).as_bytes()));
        let res = reqwest::Client::new()
            .post(format!("{}/api/messages", r.base()))
            .json(&json!({ "key": key, "timestamp": ts, "sig": sig, "channel": "general" }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status().as_u16(), 401, "a signature over another channel's words is refused");
    });
}

/// A VISITOR READS AN ANYONE CHANNEL'S POSTS SINCE IT OPENED, WITHOUT KEYS (design 4.1): of two
/// #general posts, the one made before the channel opened to Anyone is not served without a
/// member's signature, the later one is, its poster's key blanked; a member's signed read has
/// both, keys included; GET /api/channels lists #general and #announcements and not #lounge.
/// 
/// Seen red 2026-11-02 with GET /api/messages reading from 0 instead of `anyone_since`: "only what
/// was posted after it opened" (left: ["before it opened", "after it opened"]).
#[test]
fn a_visitor_reads_an_anyone_channel_since_it_opened_without_keys() {
    on_a_big_stack(|| async {
        let r = relay("access_visitor").await;
        let (_, ann) = keypair(1);
        r.state.db.register_name("ann", &ann).unwrap();
        let (_, _, since) = r.state.db.channel_access("general").unwrap();
        assert!(since > 0, "#general opened to Anyone when it was made");
        store(&r, "general", &ann, "before it opened", (since - 60_000) as u64);
        store(&r, "general", &ann, "after it opened", (since + 1) as u64);

        let (status, body) = visitor_read(&r, "messages", "general").await;
        assert_eq!(status, 200, "{body}");
        assert_eq!(contents(&body), vec!["after it opened"], "only what was posted after it opened");
        assert_eq!(body["messages"][0]["from"], "", "and no poster's key");
        assert!(body["messages"][0].get("signature").is_none(), "and no signature, which a list of keys would match to its poster: {body}");
        let (_, body) = signed_read(&r, 1, "messages", "general").await;
        assert_eq!(contents(&body), vec!["before it opened", "after it opened"], "a member reads it whole");
        assert_eq!(body["messages"][0]["from"].as_str(), Some(ann.as_str()));
        assert!(body["messages"][0]["signature"].is_string(), "a member is given the signature to check");

        let listed = reqwest::get(format!("{}/api/channels", r.base())).await.unwrap().json::<Value>().await.unwrap();
        let ids: Vec<&str> = listed["channels"].as_array().unwrap().iter().map(|c| c["id"].as_str().unwrap()).collect();
        assert_eq!(ids, vec!["general", "announcements"], "only the Anyone channels, in the sidebar's order");
    });
}

/// THE LIVE FEED, THE POST LEVEL, ONE LINK AND NO ATTACHMENT BELOW VERIFIED. Over real sockets:
/// - a socket that chose no name is sent #general's lines (Anyone) but not #lounge's (Members);
/// - a member's line in #announcements (Moderators and admins) is refused, `post_level`, and not
///   stored; a moderator's is posted;
/// - an unverified member's line with two links is refused, `too_many_links`; with one link it is
///   posted and stored as written, and no link preview is sent;
/// - an unverified member's line holding an upload's address is refused, `attachment`; a verified
///   member's line with both posts;
/// - @everyone from a verified member is refused, `mass_mention`; @here from a moderator posts.
/// 
/// Seen red 2026-11-02, one break at a time: with the send loop's `may_see` line taken out, "a
/// socket with no name is not sent a Members channel's line" (left: "lounge"); with the chat arm's
/// `post_refused` call skipped, the member's announcement came back posted (left: (Some("chat"),
/// None)); with the link limit raised and with the attachment check skipped, the line came back
/// posted where `too_many_links` and `attachment` were wanted (left: Null).
/// With the @everyone check moved below Member, "a verified member does not call on everyone"
/// (left: Null, right: "mass_mention").
#[test]
fn the_feed_follows_the_read_level_and_posts_follow_the_post_level_and_the_link_rule() {
    on_a_big_stack(|| async {
        let r = relay("access_post").await;
        let mut nameless = sign_in(&r, 3, None).await;
        let mut ann = sign_in(&r, 1, Some("ann")).await;

        // The feed: #lounge's line first, then #general's; the nameless socket's first chat is #general's.
        let lounge_ts = post(&r, &mut ann, 1, "lounge", "for members").await;
        assert_eq!(answer_to(&mut ann, lounge_ts).await["type"], "chat", "ann posts in #lounge");
        let general_ts = post(&r, &mut ann, 1, "general", "for anyone").await;
        assert_eq!(answer_to(&mut ann, general_ts).await["type"], "chat");
        let first = frame_where(&mut nameless, "a chat line", |f| f["type"] == "chat").await;
        assert_eq!(first["channel"], "general", "a socket with no name is not sent a Members channel's line: {first}");

        // The post level.
        let ts = post(&r, &mut ann, 1, "announcements", "a member's announcement").await;
        let refused = answer_to(&mut ann, ts).await;
        assert_eq!((refused["type"].as_str(), refused["reason"].as_str()), (Some("chat_refused"), Some("post_level")), "{refused}");
        assert!(stored(&r, "announcements").is_empty(), "and it is not stored");
        let (_, ann_key) = keypair(1);
        r.state.db.set_role(&ann_key, "mod").unwrap();
        let ts = post(&r, &mut ann, 1, "announcements", "a moderator's announcement").await;
        assert_eq!(answer_to(&mut ann, ts).await["type"], "chat", "a moderator posts there");
        r.state.db.set_role(&ann_key, "").unwrap();

        // One link a line below Verified, shown with no preview.
        let ts = post(&r, &mut ann, 1, "general", "two https://a.example and https://b.example").await;
        let refused = answer_to(&mut ann, ts).await;
        assert_eq!(refused["reason"], "too_many_links", "{refused}");
        let ts = post(&r, &mut ann, 1, "general", "one https://a.example").await;
        assert_eq!(answer_to(&mut ann, ts).await["type"], "chat", "one link is posted");
        assert!(stored(&r, "general").contains(&"one https://a.example".to_string()), "and stored as written");
        let previews = tokio::time::timeout(Duration::from_millis(500), frame_where(&mut ann, "a link preview", |f| f["type"] == "link_previews")).await;
        assert!(previews.is_err(), "no preview is fetched or sent");

        // No attachment below Verified.
        let ts = post(&r, &mut ann, 1, "general", "look /uploads/1760000000000_cat.png").await;
        let refused = answer_to(&mut ann, ts).await;
        assert_eq!(refused["reason"], "attachment", "{refused}");
        assert!(!stored(&r, "general").iter().any(|c| c.contains("/uploads/")), "and it is not stored");
        r.state.db.set_role(&ann_key, "verified").unwrap();
        let ts = post(&r, &mut ann, 1, "general", "look /uploads/1760000000000_cat.png and https://a.example https://b.example").await;
        assert_eq!(answer_to(&mut ann, ts).await["type"], "chat", "a verified member posts a picture and two links");

        // @everyone and @here are for moderators and admins (the operator, 2026-11-02).
        let ts = post(&r, &mut ann, 1, "general", "@everyone look at this").await;
        let refused = answer_to(&mut ann, ts).await;
        assert_eq!(refused["reason"], "mass_mention", "a verified member does not call on everyone: {refused}");
        r.state.db.set_role(&ann_key, "mod").unwrap();
        let ts = post(&r, &mut ann, 1, "general", "@here the server restarts at noon").await;
        assert_eq!(answer_to(&mut ann, ts).await["type"], "chat", "a moderator does");
    });
}

/// AN OLD DATABASE GAINS THE LEVELS AT STARTUP. A channels table of the shape the live servers
/// have (read_only, no read_level, post_level or anyone_since), holding #general, #announcements
/// (read only) and #lounge, opens without error: #general and #announcements read at Anyone from
/// the upgrade's moment on, #lounge at Members; #announcements posts at Moderators and admins, the
/// others at Members; the read_only column is gone; a second open changes nothing. The columns are
/// added by guarded ALTERs (tests/schema_alter_order_lint.rs holds the order).
/// 
/// Seen red 2026-11-02 with the UPDATE moving read_only into post_level taken out: "its read-only
/// flag became its post level" (left: (Anyone, Members), right: (Anyone, Mods)); and with the
/// read_level ALTER skipped, `channel_access("general")` found no row to read (the unwrap panicked).
#[test]
fn an_old_database_gains_the_levels_at_startup() {
    let path = humanity_core::test_temp::db("access_old_shape");
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE channels (
                id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT, created_by TEXT,
                created_at INTEGER NOT NULL, read_only INTEGER DEFAULT 0, local_only INTEGER DEFAULT 0
             );
             INSERT INTO channels (id, name, created_at, read_only) VALUES ('general', 'general', 1, 0);
             INSERT INTO channels (id, name, created_at, read_only) VALUES ('announcements', 'announcements', 1, 1);
             INSERT INTO channels (id, name, created_at, read_only) VALUES ('lounge', 'lounge', 1, 0);",
        )
        .unwrap();
    }
    let before = now_ms() as i64;
    let db = Storage::open(&path).expect("an old database opens");
    let (read, post, since) = db.channel_access("general").unwrap();
    assert_eq!((read, post), (ReadLevel::Anyone, PostLevel::Members));
    assert!(since >= before, "#general opens to Anyone from the upgrade on: {since} < {before}");
    let (read, post, since) = db.channel_access("announcements").unwrap();
    assert_eq!((read, post), (ReadLevel::Anyone, PostLevel::Mods), "its read-only flag became its post level");
    assert_eq!(since, 0, "#announcements keeps its whole history public: only the Deploy Bot posted there");
    let (read, post, since) = db.channel_access("lounge").unwrap();
    assert_eq!((read, post, since), (ReadLevel::Members, PostLevel::Members, 0), "any other channel reads at Members");
    let has_read_only = db.with_conn(|c| Ok::<bool, rusqlite::Error>(c.prepare("SELECT read_only FROM channels LIMIT 0").is_ok())).unwrap();
    assert!(!has_read_only, "the read_only column is gone");
    drop(db);
    let db = Storage::open(&path).expect("and opens again");
    assert_eq!(db.channel_access("announcements").unwrap().1, PostLevel::Mods, "a second open changes nothing");
}

/// THE LIVE SERVERS' DATABASE, WHOLE. tests/fixtures/relay/relay_v0_1456.sql is every table,
/// index and trigger the code before this wrote (the FTS triggers included, which SQLite re-checks
/// when a column is dropped), given the live servers' three channels and two more: #todo, read
/// only like #announcements, and #odd, whose read_only is NULL. Every read-only channel posts at
/// Moderators and admins, not just #announcements; a NULL flag is not read only; a second open
/// keeps #general's stamp. (The migration review's case, 2026-11-02.)
///
/// Seen red 2026-11-02 with #announcements stamped like #general: (left: (Anyone, Mods,
/// 1793679599067), right: (Anyone, Mods, 0)).
#[test]
fn the_v0_1456_database_moves_read_only_into_post_level() {
    let path = humanity_core::test_temp::db("access_v0_1456");
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/tests/fixtures/relay/relay_v0_1456.sql"))).expect("the previous code's database loads from its dump");
        conn.execute_batch(
            "INSERT INTO channels (id, name, created_at, read_only, federated) VALUES ('general', 'general', 1, 0, 1);
             INSERT INTO channels (id, name, created_at, read_only, federated) VALUES ('announcements', 'announcements', 1, 1, 1);
             INSERT INTO channels (id, name, created_at, read_only, local_only) VALUES ('local', 'local', 1, 0, 1);
             INSERT INTO channels (id, name, created_at, read_only) VALUES ('todo', 'todo', 1, 1);
             INSERT INTO channels (id, name, created_at, read_only) VALUES ('odd', 'odd', 1, NULL);",
        )
        .unwrap();
    }
    let db = Storage::open(&path).expect("the live servers' database opens");
    let levels = |id: &str| db.channel_access(id).unwrap_or_else(|| panic!("#{id} is kept"));
    let (read, post, general_since) = levels("general");
    assert_eq!((read, post), (ReadLevel::Anyone, PostLevel::Members));
    assert!(general_since > 0, "#general is stamped");
    assert_eq!(levels("announcements"), (ReadLevel::Anyone, PostLevel::Mods, 0));
    assert_eq!(levels("local"), (ReadLevel::Members, PostLevel::Members, 0));
    assert_eq!(levels("todo"), (ReadLevel::Members, PostLevel::Mods, 0), "every read-only channel, not only #announcements");
    assert_eq!(levels("odd"), (ReadLevel::Members, PostLevel::Members, 0), "a NULL flag is not read only");
    drop(db);
    let db = Storage::open(&path).expect("and opens again");
    assert_eq!(db.channel_access("general").unwrap().2, general_since, "a second open keeps #general's stamp");
}

/// THE UPGRADE IS ALL OR NOTHING. A database whose read_only column cannot be dropped (a view
/// names it, so SQLite refuses the DROP) does not open, and is left exactly as it was: no
/// read_level, no post_level, read_only still there, so the next start runs the whole upgrade
/// rather than skipping half of it. Run a step at a time, a crash between adding read_level and
/// opening #general would have left #general at Members for good (the migration review,
/// 2026-11-02).
///
/// Seen red 2026-11-02 with BEGIN IMMEDIATE and COMMIT taken out of the batch: "nothing of the
/// upgrade is left behind" (left: (true, true, true, true), right: (false, false, false, true)):
/// all three columns were added before the DROP failed.
#[test]
fn the_upgrade_is_all_or_nothing() {
    let path = humanity_core::test_temp::db("access_all_or_nothing");
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE channels (
                id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT, created_by TEXT,
                created_at INTEGER NOT NULL, read_only INTEGER DEFAULT 0, local_only INTEGER DEFAULT 0
             );
             INSERT INTO channels (id, name, created_at, read_only) VALUES ('general', 'general', 1, 0);
             CREATE VIEW read_only_channels AS SELECT id FROM channels WHERE read_only = 1;",
        )
        .unwrap();
    }
    assert!(Storage::open(&path).is_err(), "the DROP is refused, so the open fails");
    let conn = rusqlite::Connection::open(&path).unwrap();
    let has = |column: &str| conn.prepare(&format!("SELECT {column} FROM channels LIMIT 0")).is_ok();
    assert_eq!(
        (has("read_level"), has("post_level"), has("anyone_since"), has("read_only")),
        (false, false, false, true),
        "nothing of the upgrade is left behind"
    );
}

/// OPENING A CHANNEL TO ANYONE STAMPS WHEN, AND ONLY A CHANNEL ANYONE READS TRAVELS: setting
/// #lounge to Anyone stamps `anyone_since` now and setting it again keeps the stamp; a channel at
/// Members does not travel to linked servers, whatever its Federated tick; a channel reading at
/// Verified never posts below Verified.
#[test]
fn opening_a_channel_stamps_when_and_only_anyone_channels_travel() {
    let db = Storage::open_temp("access_levels");
    db.create_channel("lounge", "lounge", None, "test", PostLevel::Members).unwrap();
    db.set_channel_federated("lounge", true).unwrap();
    assert!(!crate::relay::chat_access::travels(&db, "lounge"), "a Members channel does not travel");
    let before = now_ms() as i64;
    db.set_channel_access("lounge", ReadLevel::Anyone, PostLevel::Members).unwrap();
    let (_, _, since) = db.channel_access("lounge").unwrap();
    assert!(since >= before, "opened now");
    std::thread::sleep(Duration::from_millis(5));
    db.set_channel_access("lounge", ReadLevel::Anyone, PostLevel::Verified).unwrap();
    assert_eq!(db.channel_access("lounge").unwrap().2, since, "staying at Anyone keeps the stamp");
    assert!(crate::relay::chat_access::travels(&db, "lounge"), "an Anyone channel travels");
    db.set_channel_access("lounge", ReadLevel::Verified, PostLevel::Members).unwrap();
    assert_eq!(db.channel_access("lounge").unwrap().1, PostLevel::Verified, "nobody posts where they cannot read");
}
