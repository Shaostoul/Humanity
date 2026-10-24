//! PHASE 3 of docs/design/federation-activation.md, as a test that runs with the relay battery
//! (`just verify-relay`): two complete relays on loopback (127.0.0.1 only), linked both ways at
//! tier 2 the way /server-add and the key discovery link them, sharing #general. People sign in
//! through the real /ws handshake and post PQ-signed chat; a moderator bans through the real
//! `mod_action`; profiles are signed by the person's own key. Everything is read back from the
//! OTHER relay's database within [`BOUND`].
//!
//! The load-bearing question the design asked ("does moderation propagate?") is answered by the
//! decision written in federation_log.rs and the design doc: an origin's edit and delete reach
//! every copy, and a ban is honoured in the rooms the servers share.

use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message as WsMsg;

use crate::relay::core::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};
use crate::relay::handlers::federation;
use crate::relay::handlers::federation_flow::{FlowLimits, KindLimit};
use crate::relay::handlers::federation_log;
use crate::relay::relay::RelayState;
use crate::relay::storage::Storage;

/// Everything that happens on one server is on the other within this long.
const BOUND: Duration = Duration::from_secs(10);

type Sock = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

struct Relay {
    state: Arc<RelayState>,
    port: u16,
    key: String,
}

async fn relay(tag: &str) -> Relay {
    let dir = std::env::temp_dir().join(format!("hos_fed_p3_{tag}_{}_{}", std::process::id(), rand::random::<u32>()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = Storage::open(&dir.join("relay.db")).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let state = crate::relay::serve_relay(db, listener).await;
    let (key, _) = state.db.get_or_create_server_keypair().unwrap();
    let _ = state.db.create_channel("general", "General", None, "test", false);
    assert!(state.db.set_channel_federated("general", true).unwrap(), "#general is shared");
    let _ = state.db.create_channel("local", "Local", None, "test", false);
    Relay { state, port, key }
}

/// `a` dials `b`, each trusting the other at tier 2 with its key pinned; both links are up when
/// this returns.
async fn link(a: &Relay, b: &Relay) {
    let b_url = format!("http://127.0.0.1:{}", b.port);
    a.state.db.add_federated_server(&b_url, "Server B", &b_url).unwrap();
    a.state.db.set_server_trust_tier(&b_url, 2).unwrap();
    a.state.db.update_federated_server_info(&b_url, "Server B", Some(&b.key), false).unwrap();
    let placeholder = "http://127.0.0.1:1";
    b.state.db.add_federated_server(placeholder, "Server A", placeholder).unwrap();
    b.state.db.set_server_trust_tier(placeholder, 2).unwrap();
    b.state.db.update_federated_server_info(placeholder, "Server A", Some(&a.key), false).unwrap();
    assert_eq!(federation::start_federation_connections(&a.state).await, 1);
    within("both links are up", || {
        a.state.federation_connections.try_read().map(|c| c.contains_key(&b.key)).unwrap_or(false)
            && b.state.federation_connections.try_read().map(|c| c.contains_key(&a.key)).unwrap_or(false)
    })
    .await;
}

/// Wait until `check` passes, at most [`BOUND`]; how long it took.
async fn within(what: &str, check: impl FnMut() -> bool) -> Duration {
    within_for(BOUND, what, check).await
}

/// How long the flood may take to arrive complete. It is a test of completeness, not of speed:
/// against allowances of 5 a second, every pause in a row is longer (1, 1, 2, 3 s ...), and on a
/// machine busy with other builds a debug relay signs the 40 lines slowly, so the live ones keep
/// arriving between pauses. (2026-10-23: one of this test's 10 s waits ran out once, in a run of
/// 200 tests at once on a machine with 50 compilers running; the same run again, and the test on
/// its own, passed. The flood is the step whose length grows with load, so it waits longer.)
const FLOOD_BOUND: Duration = Duration::from_secs(60);

async fn within_for(bound: Duration, what: &str, mut check: impl FnMut() -> bool) -> Duration {
    let start = Instant::now();
    while start.elapsed() < bound {
        if check() {
            return start.elapsed();
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("not within {bound:?}: {what}");
}

fn keypair(seed: u8) -> (DilithiumKeypair, String) {
    let kp = DilithiumKeypair::from_seed(&derive_dilithium_seed(&[seed; 32]));
    let key = hex::encode(kp.public_key());
    (kp, key)
}

/// Sign in to `r` as `name` with the key of `[seed; 32]`, through /ws and the identify handshake.
/// The account is then made old enough to post (a new one waits a minute and posts slowly).
async fn sign_in(r: &Relay, seed: u8, name: &str) -> Sock {
    let (kp, key) = keypair(seed);
    let (mut sock, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{}/ws", r.port)).await.unwrap();
    sock.send(WsMsg::Text(json!({ "type": "identify", "public_key": key, "display_name": name }).to_string().into())).await.unwrap();
    let challenge: Value = serde_json::from_str(&sock.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
    let nonce = challenge["nonce"].as_str().expect("a challenge").to_string();
    let sig = B64.encode(kp.sign(format!("hum/identify/v1\n{nonce}\n{key}").as_bytes()));
    sock.send(WsMsg::Text(json!({ "type": "identify_response", "sig_b64": sig }).to_string().into())).await.unwrap();
    frame_where(&mut sock, "signed in", |f| f["type"] == "peer_list" || f["type"] == "channel_list").await;
    r.state.db.with_conn(|c| c.execute("UPDATE registered_names SET registered_at = 0 WHERE public_key = ?1", [&key])).unwrap();
    sock
}

async fn send(sock: &mut Sock, frame: Value) {
    sock.send(WsMsg::Text(frame.to_string().into())).await.unwrap();
}

async fn frame_where(sock: &mut Sock, what: &str, want: impl Fn(&Value) -> bool) -> Value {
    let found = tokio::time::timeout(BOUND, async {
        while let Some(Ok(msg)) = sock.next().await {
            if let Ok(f) = msg.into_text().map(|t| serde_json::from_str::<Value>(&t)) {
                if let Ok(f) = f {
                    if want(&f) {
                        return Some(f);
                    }
                }
            }
        }
        None
    })
    .await;
    found.ok().flatten().unwrap_or_else(|| panic!("never came: {what}"))
}

/// Post `content` in `channel` as the key of `[seed; 32]`, signed as the apps sign chat; its time.
async fn post(sock: &mut Sock, seed: u8, channel: &str, content: &str) -> u64 {
    let (kp, key) = keypair(seed);
    let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
    let sig = hex::encode(kp.sign(format!("{content}\n{ts}").as_bytes()));
    send(sock, json!({ "type": "chat", "from": key, "content": content, "timestamp": ts, "channel": channel, "pq_signature": sig })).await;
    ts
}

/// The federated copies `r` holds of `origin`'s messages, as (author, time, content).
fn copies(r: &Relay, origin: &str) -> Vec<(String, u64, String)> {
    r.state
        .db
        .with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT json_extract(raw_json, '$.from_key'), timestamp, content FROM messages
                 WHERE msg_type = 'federated_chat' AND from_key = ?1 ORDER BY id",
            )?;
            let rows = stmt.query_map([origin], |r| Ok((r.get::<_, Option<String>>(0)?.unwrap_or_default(), r.get::<_, i64>(1)? as u64, r.get::<_, String>(2)?)))?;
            rows.collect::<Result<Vec<_>, _>>()
        })
        .unwrap()
}

fn holds(r: &Relay, origin: &str, content: &str) -> usize {
    copies(r, origin).iter().filter(|(_, _, c)| c == content).count()
}

/// A MESSAGE, AN EDIT, A DELETE, A PROFILE CHANGE AND A BAN ON ONE SERVER REACH THE OTHER, AND THE
/// BAN IS HONOURED THERE. On two relays linked both ways at tier 2, sharing #general:
/// - Ann (admin on A) posts in #general: B holds it, with Ann's key as its author.
/// - She edits it: B's copy has the new words. She deletes another: B's copy is gone.
/// - She saves her profile and her key signs its copy: B keeps it.
/// - She bans Mallory on A: B keeps A's ban, so Mallory's post in B's shared #general is refused
///   (with words saying B's own rooms are still open) while her post in B's own #local goes; a
///   line by Mallory that reached A anyway (a server not honouring the ban) is not shown on A.
/// - She lifts the ban: B forgets it, and Mallory's next post in #general reaches A.
/// Each within BOUND; the slowest is printed.
///
/// Seen red 2026-10-22, one break at a time: with the `after_ban` call taken out of
/// `mod_action_carry_out`, "not within 10s: B keeps A's ban" (every step before it passed); with
/// `edit_own_message` not sending to the links (its `shared` check made false), "not within 10s:
/// B's copy has the new words"; with `after_delete` not sending (2026-10-23), "not within 10s:
/// B's copy of the deleted message is gone".
#[test]
fn a_message_edit_delete_profile_and_ban_reach_the_other_server_and_the_ban_is_honoured() {
    on_a_big_stack(|| everything_reaches_the_other_server());
}

/// Run `test` on a thread with a 64 MB stack and a runtime of its own. Two whole relays and a long
/// test body in a DEBUG build overflow a test thread's 2 MB (seen 2026-10-22: "has overflowed its
/// stack", even with the body boxed): debug code gives every temporary its own stack slot, and
/// each `json!` and Dilithium step adds its own. A release relay is nowhere near it. A failure is
/// re-raised here with its own message.
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
        .unwrap();
    if let Err(panic) = run.join() {
        std::panic::resume_unwind(panic);
    }
}

async fn everything_reaches_the_other_server() {
    let a = relay("a").await;
    let b = relay("b").await;
    link(&a, &b).await;
    let (_, ann) = keypair(41);
    let (_, mallory) = keypair(43);
    let mut slowest = Duration::ZERO;
    let mut took = |d: Duration| slowest = slowest.max(d);

    let mut sock = sign_in(&a, 41, "Ann").await;
    a.state.db.set_role(&ann, "admin").unwrap();

    // A message.
    let t1 = post(&mut sock, 41, "general", "hello from A").await;
    took(within("B holds Ann's message", || holds(&b, &a.key, "hello from A") == 1).await);
    assert!(copies(&b, &a.key).iter().any(|(author, ts, _)| *author == ann && *ts == t1), "the author and time travel with it");

    // An edit, through the real edit frame.
    send(&mut sock, json!({ "type": "edit", "from": "", "timestamp": t1, "new_content": "hello from A (edited)", "channel": "general" })).await;
    took(within("B's copy has the new words", || holds(&b, &a.key, "hello from A (edited)") == 1 && holds(&b, &a.key, "hello from A") == 0).await);

    // A delete. (Admins skip the chat limiter, so two posts in a row are fine.)
    let t2 = post(&mut sock, 41, "general", "a line to take back").await;
    within("B holds the line to take back", || holds(&b, &a.key, "a line to take back") == 1).await;
    send(&mut sock, json!({ "type": "delete", "from": "", "timestamp": t2 })).await;
    took(within("B's copy of the deleted message is gone", || holds(&b, &a.key, "a line to take back") == 0).await);

    // A line in A's own room never leaves A.
    post(&mut sock, 41, "local", "stays on A").await;

    // A profile change, signed by her own key (a new socket: one save per 30 s per socket).
    let mut psock = sign_in(&a, 41, "Ann").await;
    send(&mut psock, json!({ "type": "profile_update", "socials": "{}", "bio": "Keeps bees.", "privacy": "{}" })).await;
    let own = frame_where(&mut psock, "her own copy, asking to be signed", |f| f["type"] == "profile_data" && f["sign_now"] == true).await;
    let s = |k: &str| own[k].as_str().unwrap_or("").to_string();
    let copy = crate::relay::core::profile_copy::public_copy(&s("name"), &s("bio"), &s("avatar_url"), &s("banner_url"), &s("socials"), &s("pronouns"), &s("location"), &s("website"), &s("privacy"));
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
    let at = crate::relay::core::profile_copy::next_copy_time(now, own["signed_at"].as_u64().unwrap_or(0), 0);
    let signature = crate::relay::core::profile_copy::sign_copy(&[41; 32], &ann, &copy, at);
    send(&mut psock, json!({ "type": "profile_sign", "at": at, "signature": signature })).await;
    took(within("B keeps Ann's profile", || b.state.db.get_signed_profile(&ann).ok().flatten().is_some_and(|p| p.bio == "Keeps bees.")).await);

    // A ban, through the real moderation action.
    let mut msock = sign_in(&b, 43, "Mallory").await;
    send(&mut sock, json!({ "type": "mod_action", "action": "ban", "target": mallory, "target_name": "" })).await;
    took(within("B keeps A's ban", || b.state.db.fed_banned_by(&mallory).ok().flatten().is_some()).await);
    post(&mut msock, 43, "general", "mallory in the shared room").await;
    let refusal = frame_where(&mut msock, "the refusal", |f| f["type"] == "system" && f["message"].as_str().is_some_and(|m| m.contains("shared with"))).await;
    assert!(refusal["message"].as_str().unwrap().contains("own rooms are still open"), "{refusal}");
    tokio::time::sleep(Duration::from_millis(1_100)).await; // the chat limiter's one second
    post(&mut msock, 43, "local", "mallory in B's own room").await;
    within("her post in B's own room goes", || b.state.db.load_recent_messages(50).unwrap().iter().any(|m| matches!(m, crate::relay::relay::RelayMessage::Chat { content, .. } if content == "mallory in B's own room"))).await;
    assert!(!b.state.db.load_recent_messages(50).unwrap().iter().any(|m| matches!(m, crate::relay::relay::RelayMessage::Chat { content, .. } if content == "mallory in the shared room")), "her post in the shared room was stored on B");
    // A server that does not honour the ban sends her words anyway: A, which banned her, refuses.
    federation::forward_to_federation(&b.state, "general", &mallory, "Mallory", "sent by a server that ignores bans", now).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(holds(&a, &b.key, "sent by a server that ignores bans"), 0, "A showed the words of a key it banned");
    assert_eq!(holds(&b, &a.key, "stays on A"), 0, "a line in A's own room reached B");

    // The ban lifted.
    send(&mut sock, json!({ "type": "unban", "target": mallory })).await;
    took(within("B forgets A's ban", || b.state.db.fed_banned_by(&mallory).ok().flatten().is_none()).await);
    tokio::time::sleep(Duration::from_millis(1_100)).await;
    post(&mut msock, 43, "general", "mallory, welcome back").await;
    took(within("her post reaches A again", || holds(&a, &b.key, "mallory, welcome back") == 1).await);
    println!("PHASE 3: the slowest change took {slowest:?} to reach the other server (bound {BOUND:?})");
}

/// A PAUSE LOSES NOTHING AND REPEATS NOTHING, AND A FLOOD IS SLOWED THEN PAUSED THEN CAUGHT UP.
/// B pauses A for three seconds; meanwhile Ann posts five lines on A, edits one and deletes
/// another, and A keeps a key card of hers. Nothing reaches B during the pause. When it ends B
/// asks, A sends what B set aside, and B holds exactly four lines (the deleted one gone, the
/// edited one edited), each once, and Ann's card. Asking
/// again from three numbers back (a restart before the place was stored) changes nothing. Then,
/// with small allowances on B, A sends 40 lines at once: B is slowed, then paused (the admin API
/// shows the pause), and still holds all 40, each once.
///
/// Seen red 2026-10-22 with the end of a pause no longer asking for the catch-up
/// (`take_resume_due` ignored in `ingest` and `tick`): "not within 10s: B caught up after the
/// pause"; with `serve_resume` reading from one number too far: the same; and (2026-10-23) with a
/// repeat let through twice over (the check in `numbered` and `fed_message_held` both made false),
/// "asking again changed what B holds: [... "during the pause 5", "during the pause 5"]"; and
/// (2026-10-24) with `key_card_gossip` taken out of `apply`, so a card in the log was never kept:
/// "not within 10s: B keeps Ann's card after the pause" (the lines had all arrived).
#[test]
fn a_pause_loses_nothing_and_repeats_nothing_and_a_flood_is_slowed_then_paused() {
    on_a_big_stack(|| a_pause_and_a_flood());
}

async fn a_pause_and_a_flood() {
    let a = relay("pa").await;
    let b = relay("pb").await;
    link(&a, &b).await;
    let (_, ann) = keypair(51);
    let mut sock = sign_in(&a, 51, "Ann").await;
    a.state.db.set_role(&ann, "admin").unwrap();
    let t0 = post(&mut sock, 51, "general", "before the pause").await;
    within("B holds the first line", || holds(&b, &a.key, "before the pause") == 1).await;
    let _ = t0;

    federation_log::pause_peer(&b.state, &a.key, Duration::from_secs(3));
    let paused_at = Instant::now();
    let mut stamps = Vec::new();
    for i in 1..=5 {
        stamps.push(post(&mut sock, 51, "general", &format!("during the pause {i}")).await);
    }
    send(&mut sock, json!({ "type": "edit", "from": "", "timestamp": stamps[1], "new_content": "during the pause 2 (edited)", "channel": "general" })).await;
    send(&mut sock, json!({ "type": "delete", "from": "", "timestamp": stamps[3] })).await;
    // And Ann's key card, kept on A as her sign-in keeps it (handlers/key_cards.rs), so it goes
    // into A's numbered log while B has A paused.
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
    let card = crate::relay::core::key_card::sign_card(&[51; 32], now).unwrap();
    let kept = crate::relay::handlers::key_cards::keep_from_sign_in(&a.state, &ann, crate::relay::core::key_card::card_json(&card)).await;
    assert!(kept.is_some_and(|k| k.stored()), "A keeps Ann's card");
    tokio::time::sleep(Duration::from_millis(1_000)).await;
    assert!(paused_at.elapsed() < Duration::from_secs(3), "test setup: still inside the pause");
    assert!(copies(&b, &a.key).iter().all(|(_, _, c)| !c.starts_with("during the pause")), "a line reached B during the pause");
    assert_eq!(b.state.db.get_key_card(&ann).unwrap(), None, "Ann's card reached B during the pause");

    let caught_up = |b: &Relay| {
        let held = copies(b, &a.key);
        let n = |c: &str| held.iter().filter(|(_, _, x)| x == c).count();
        n("during the pause 1") == 1 && n("during the pause 2 (edited)") == 1 && n("during the pause 2") == 0
            && n("during the pause 3") == 1 && n("during the pause 4") == 0 && n("during the pause 5") == 1
    };
    within("B caught up after the pause", || caught_up(&b)).await;
    within("B keeps Ann's card after the pause", || b.state.db.get_key_card(&ann).ok().flatten().as_ref() == Some(&card)).await;
    let took = paused_at.elapsed();
    println!("PHASE 3: B caught up {took:?} after the pause began ({:?} after it ended)", took.saturating_sub(Duration::from_secs(3)));
    let flow = federation_log::peer_view(&b.state, Some(&a.key));
    assert!(flow["flow"]["set_aside"].as_u64().unwrap() >= 7, "the frames during the pause were set aside: {flow}");

    // Asked again from three numbers back: nothing is stored twice.
    let (_, newest) = a.state.db.fed_outbox_bounds().unwrap();
    federation_log::rewind_for_test(&b.state, &a.key, newest - 3).await;
    within("B is back at the newest number", || b.state.db.fed_cursor(&a.key).ok().flatten().is_some_and(|(_, last)| last == newest)).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let words = |b: &Relay| copies(b, &a.key).into_iter().map(|(_, _, c)| c).collect::<Vec<_>>();
    assert!(caught_up(&b), "asking again changed what B holds: {:?}", words(&b));
    for (_, _, c) in copies(&b, &a.key) {
        assert_eq!(holds(&b, &a.key, &c), 1, "a line arrived twice: {c}");
    }

    // A flood: small allowances on B, 40 lines from A at once.
    *b.state.fed.limits.write().unwrap() = FlowLimits {
        kinds: [KindLimit { per_sec: 5.0, burst: 5.0 }; crate::relay::handlers::federation_flow::Kind::COUNT],
        pause_after: Duration::from_secs(1),
        window: Duration::from_secs(60),
        pause_unit: Duration::from_secs(1),
        calm_after: Duration::from_secs(600),
    };
    let base = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
    for i in 0..40u64 {
        federation::forward_to_federation(&a.state, "general", &ann, "Ann", &format!("flood {i}"), base + i).await;
    }
    let took = within_for(FLOOD_BOUND, "B holds all 40, each once", || (0..40).all(|i| holds(&b, &a.key, &format!("flood {i}")) == 1)).await;
    println!("PHASE 3: the 40-line flood arrived complete in {took:?}");
    let view = federation_log::peer_view(&b.state, Some(&a.key));
    assert!(view["flow"]["pauses"].as_u64().unwrap() >= 2, "the flood was paused: {view}");
    assert!(view["flow"]["slowed_ms_total"].as_u64().unwrap() > 0, "and slowed first: {view}");

    // The admins see it through the API (the web admin page's federation panel reads this).
    let (admin_kp, admin) = keypair(52);
    b.state.db.set_role(&admin, "admin").unwrap();
    let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
    let sig = hex::encode(admin_kp.sign(format!("admin_federation\n{ts}").as_bytes()));
    let answer: Value = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{}/api/admin/federation", b.port))
        .json(&json!({ "key": admin, "timestamp": ts, "sig": sig, "action": "list" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let row = answer["servers"].as_array().unwrap().iter().find(|s| s["name"] == "Server A").expect("A's row").clone();
    assert!(row["link"]["flow"]["pauses"].as_u64().unwrap() >= 2, "the admin API shows the pause: {row}");
    assert_eq!(row["link"]["cursor"]["seq"], json!(a.state.db.fed_outbox_bounds().unwrap().1), "and where B is in A's log");
}

/// A REFUSED CHAT LINE IS NAMED TO ITS SENDER, AND ONLY TO THEM (2026-10-24). Before, a line
/// over the length limit (or sent too fast, or by a brand new account) was answered with a plain
/// system notice naming no line, so the website kept its own copy looking sent (the operator's
/// screenshot: one of Brave's lines shown twice, the first refused for length). Now the sender
/// also gets `chat_refused` with that line's timestamp, the reason and the limit; nobody else
/// does, and the frame never carries the `to` it is addressed by.
/// Seen red 2026-10-24 before the role-limit refusal (the first check, "exceeds your role's limit
/// of 280") sent it: "never came: the sender is told which line was refused" (only the plain
/// notice arrived).
#[tokio::test(flavor = "multi_thread")]
async fn a_refused_line_is_named_to_its_sender_and_no_one_else() {
    let r = relay("refused").await;
    let mut ann = sign_in(&r, 61, "Ann").await;
    let mut bob = sign_in(&r, 62, "Bob").await;
    let long = "x".repeat(2_100);
    let ts = post(&mut ann, 61, "general", &long).await;
    let f = frame_where(&mut ann, "the sender is told which line was refused", |f| f["type"] == "chat_refused").await;
    assert_eq!(f["timestamp"].as_u64(), Some(ts), "it names the refused line by its timestamp");
    assert_eq!(f["reason"], "too_long");
    // The first check a new member meets is their role's own limit (280 for an unverified
    // account, the case in the operator's screenshot), before the server-wide 2,000.
    let limit = f["limit"].as_u64().expect("with the limit the line went over");
    assert!(limit < 2_100, "the limit it names is one the line exceeded: {limit}");
    assert!(f.get("to").is_none(), "the frame does not carry who it is addressed to: {f}");
    // Bob, on the same server, hears nothing about it (a quiet window, then a line of his own
    // proves his socket is live and nothing else was queued for him).
    let bob_ts = post(&mut bob, 62, "general", "hello").await;
    let seen = frame_where(&mut bob, "Bob's own line comes back", |f| f["type"] == "chat_refused" || (f["type"] == "chat" && f["timestamp"].as_u64() == Some(bob_ts))).await;
    assert_eq!(seen["type"], "chat", "Bob was told about Ann's refused line: {seen}");
}

/// A SIGNED-IN MEMBER IS TOLD THEIR OWN LINE LIMIT (2026-10-24): `my_limits` with their role's
/// max_chars, so the website counts against the limit the relay applies (280 for an unverified
/// account) instead of a guessed 2,000. Seen red before the frame was sent: "never came: my
/// limits".
#[tokio::test(flavor = "multi_thread")]
async fn a_member_is_told_their_own_line_limit() {
    let r = relay("limits").await;
    let (_, key) = keypair(63);
    let (mut sock, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{}/ws", r.port)).await.unwrap();
    let (kp, _) = keypair(63);
    send(&mut sock, json!({ "type": "identify", "public_key": key, "display_name": "Cy" })).await;
    let challenge: Value = serde_json::from_str(&sock.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
    let nonce = challenge["nonce"].as_str().expect("a challenge").to_string();
    let sig = B64.encode(kp.sign(format!("hum/identify/v1\n{nonce}\n{key}").as_bytes()));
    send(&mut sock, json!({ "type": "identify_response", "sig_b64": sig })).await;
    let f = frame_where(&mut sock, "my limits", |f| f["type"] == "my_limits").await;
    let want = r.state.db.role_def(&r.state.db.get_role(&key).unwrap_or_default()).max_chars as u64;
    assert_eq!(f["max_chars"].as_u64(), Some(want), "the limit is the role's own: {f}");
}
