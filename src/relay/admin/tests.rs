// Child of admin/mod.rs: the registry's own checks, the coverage of its runners, and the run
// endpoint called the way axum calls it. Each test was seen failing once; what it saw red is at
// the test.

use std::collections::HashSet;

use axum::extract::State;
use axum::response::Response;
use axum::Json;

use super::*;
use crate::relay::core::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};
use crate::relay::storage::Storage;

struct Person {
    dil: DilithiumKeypair,
    key: String,
}

fn person(b: u8) -> Person {
    let dil = DilithiumKeypair::from_seed(&derive_dilithium_seed(&[b; 32]));
    let key = hex::encode(dil.public_key());
    Person { dil, key }
}

fn state(tag: &str) -> Arc<RelayState> {
    Arc::new(RelayState::new(Storage::open_temp(&format!("admin_actions_{tag}"))))
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64
}

/// A run request for `action` with `inputs`, signed by `p` the way both apps sign it.
fn signed_run(p: &Person, action: &str, inputs: &str, ts: u64) -> RunRequest {
    let sig = p.dil.sign(format!("{}\n{ts}", run_purpose(action, inputs)).as_bytes());
    RunRequest { key: p.key.clone(), timestamp: ts, sig: hex::encode(sig), action: action.into(), inputs: inputs.into(), confirm: None }
}

async fn body(res: Response) -> (u16, serde_json::Value) {
    let code = res.status().as_u16();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 22).await.unwrap();
    (code, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

async fn run(st: &Arc<RelayState>, req: RunRequest) -> (u16, serde_json::Value) {
    body(post_run(State(st.clone()), Json(req)).await).await
}

/// Each run needs its own millisecond (the used-once check is per key, purpose and time).
fn tick() -> u64 {
    static LAST: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let now = now_ms();
    let prev = LAST.fetch_max(now, std::sync::atomic::Ordering::SeqCst);
    if prev >= now { LAST.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1 } else { now }
}

// ── The registry itself ───────────────────────────────────────────────────────────────────────

/// The file parses and holds together: ids unique and in a known category, roles and danger
/// levels known, a destructive action carries its confirm word, every action has a way to run,
/// inputs are named once and of a kind the apps draw, and a chat template names only its inputs.
/// Seen red 2026-10-22 with `channels.delete` given no confirm_word: "channels.delete is
/// destructive and has no confirm_word".
#[test]
fn the_registry_holds_together() {
    let r = registry();
    assert!(r.actions.len() >= 20, "the registry has its actions");
    let cats: HashSet<&str> = r.categories.iter().map(|c| c.id.as_str()).collect();
    let mut ids = HashSet::new();
    for a in &r.actions {
        assert!(ids.insert(a.id.as_str()), "{} is listed twice", a.id);
        assert!(a.id.starts_with(&format!("{}.", a.category)), "{} is named for its category {}", a.id, a.category);
        assert!(cats.contains(a.category.as_str()), "{} is in an unknown category {}", a.id, a.category);
        assert!(r.roles.contains_key(&a.role), "{} asks for an unknown role {}", a.id, a.role);
        assert!(DANGER_LEVELS.contains(&a.danger.as_str()), "{} has an unknown danger {}", a.id, a.danger);
        if a.is_destructive() {
            assert!(a.confirm_word.as_deref().is_some_and(|w| !w.trim().is_empty()), "{} is destructive and has no confirm_word", a.id);
        }
        assert!(a.runs_here() || a.chat.is_some() || a.route.is_some(), "{} has no way to run", a.id);
        if let Some(run) = &a.run {
            assert_eq!(run, "here", "{}: run is \"here\" or absent", a.id);
        }
        if a.runs_here() {
            assert!(matches!(a.role.as_str(), "admin" | "mod"), "{} runs here, so it is for admins or moderators", a.id);
        }
        assert!(!a.label.is_empty() && !a.description.is_empty(), "{} says what it does", a.id);
        let mut names = HashSet::new();
        for i in &a.inputs {
            assert!(names.insert(i.name.as_str()), "{} names input {} twice", a.id, i.name);
            assert!(INPUT_KINDS.contains(&i.kind.as_str()), "{}.{} has an unknown kind {}", a.id, i.name, i.kind);
            if i.kind == "choice" {
                assert!(!i.choices.is_empty(), "{}.{} is a choice with nothing to choose", a.id, i.name);
            }
        }
        if let Some(chat) = &a.chat {
            assert!(chat.starts_with('/'), "{}: a chat command starts with /", a.id);
            for w in chat.split_whitespace() {
                if let Some(n) = w.strip_prefix('{').and_then(|r| r.strip_suffix('}')) {
                    assert!(names.contains(n), "{}'s chat command names {{{n}}}, which it does not take", a.id);
                }
            }
        }
        if let Some(route) = &a.route {
            assert!(route.starts_with("GET /api/") || route.starts_with("POST /api/"), "{}: a route is GET or POST /api/...", a.id);
        }
    }
    for res in &r.reserved {
        assert!(res.prefix.ends_with('.') && !res.for_whom.is_empty(), "a reserved prefix ends with a dot and says whom it is for");
    }
}

/// THE COVERAGE: every action that says it runs here has a runner, and every runner has an
/// action that says so. An admin action cannot exist in one place and not the other. Seen red
/// 2026-10-22 twice: with `federation.pause` taken out of RUNNERS ("federation.pause runs here
/// but has no runner"), and with a runner `alerts.rename` added that no entry names ("a runner
/// with no registry entry: alerts.rename").
#[test]
fn every_action_that_runs_here_has_a_runner_and_every_runner_an_action() {
    let here: Vec<&str> = registry().actions.iter().filter(|a| a.runs_here()).map(|a| a.id.as_str()).collect();
    for id in &here {
        assert!(runner_for(id).is_some(), "{id} runs here but has no runner");
    }
    let mut seen = HashSet::new();
    for (id, _) in RUNNERS {
        assert!(seen.insert(*id), "{id} has two runners");
        assert!(here.contains(id), "a runner with no registry entry: {id}");
    }
}

/// Both apps and an AI sign the same words: the file's example is what this relay checks. The
/// web half reads the same example (scripts/tests/admin-actions-web.test.js). Seen red
/// 2026-10-22 with the example written before its hash was computed.
#[test]
fn the_signing_example_is_what_the_relay_checks() {
    let ex = &registry().signing["example"];
    let (action, inputs) = (ex["action"].as_str().unwrap(), ex["inputs"].as_str().unwrap());
    assert_eq!(run_purpose(action, inputs), ex["purpose"].as_str().unwrap());
}

/// The registry is served with each action's `destructive` spelled out and without the file's
/// note. Seen red 2026-10-22 with the insert taken out: "maintenance.wipe_all is served as
/// destructive".
#[test]
fn the_registry_is_served_with_destructive_spelled_out() {
    let v = served_registry();
    let wipe = v["actions"].as_array().unwrap().iter().find(|a| a["id"] == "maintenance.wipe_all").unwrap();
    assert_eq!(wipe["destructive"], true, "maintenance.wipe_all is served as destructive");
    let list = v["actions"].as_array().unwrap().iter().find(|a| a["id"] == "alerts.list").unwrap();
    assert_eq!(list["destructive"], false);
    assert!(v.get("_purpose").is_none());
    assert_eq!(v["endpoints"]["run"], "POST /api/admin/actions/run");
}

/// Inputs are checked against the entry: nothing it does not take, required ones present, kinds
/// and limits kept, no control characters. Seen red 2026-10-22 with the unknown-name loop taken
/// out: "an input the action does not take is refused".
#[test]
fn inputs_are_checked_against_the_entry() {
    let add = find("alerts.add").unwrap();
    let ok = check_inputs(add, &serde_json::json!({ "kind": "ntfy", "address": " https://ntfy.sh/x ", "enabled": true })).unwrap();
    assert_eq!(ok.text("address").as_deref(), Some("https://ntfy.sh/x"), "trimmed");
    assert!(check_inputs(add, &serde_json::json!({ "kind": "ntfy", "address": "a", "colour": "red" })).is_err(), "an input the action does not take is refused");
    assert!(check_inputs(add, &serde_json::json!({ "kind": "ntfy" })).is_err(), "a required input is needed");
    assert!(check_inputs(add, &serde_json::json!({ "kind": "carrier pigeon", "address": "a" })).is_err(), "a choice outside its list");
    assert!(check_inputs(add, &serde_json::json!({ "kind": "ntfy", "address": "a\nb" })).is_err(), "no line breaks");
    assert!(check_inputs(add, &serde_json::json!({ "kind": "ntfy", "address": "a".repeat(501) })).is_err(), "too long");
    let upd = find("alerts.update").unwrap();
    assert!(check_inputs(upd, &serde_json::json!({ "index": 50 })).is_err(), "a number past its max");
    assert!(check_inputs(upd, &serde_json::json!({ "index": "0" })).is_err(), "a number is a number");
    assert!(check_inputs(find("alerts.set_enabled").unwrap(), &serde_json::json!({ "index": 0, "enabled": "yes" })).is_err(), "a bool is a bool");
}

/// A destructive action needs its confirm word, exactly; nothing else asks for one. Seen red
/// 2026-10-22 with `confirm_refusal` returning None for every action: "a destructive action
/// without its word is refused".
#[test]
fn a_destructive_action_needs_its_confirm_word() {
    let wipe = find("maintenance.wipe_all").unwrap();
    assert!(confirm_refusal(wipe, None).is_some(), "a destructive action without its word is refused");
    assert!(confirm_refusal(wipe, Some("wipe all")).is_some(), "the word is exact");
    assert!(confirm_refusal(wipe, Some(" WIPE ALL ")).is_none(), "trimmed");
    assert!(confirm_refusal(find("federation.remove").unwrap(), None).is_none(), "a caution action asks nothing of the endpoint");
}

/// The chat command an entry names is filled from the inputs, leaving no gap for an empty one.
#[test]
fn a_chat_command_is_filled_from_the_inputs() {
    let add = find("federation.add").unwrap();
    let mut m = serde_json::Map::new();
    m.insert("url".into(), "https://a.example".into());
    assert_eq!(add.chat_text(&m).as_deref(), Some("/server-add https://a.example"));
    m.insert("name".into(), "Alpha".into());
    assert_eq!(add.chat_text(&m).as_deref(), Some("/server-add https://a.example Alpha"));
    assert_eq!(add.chat_command(), Some("/server-add"));
}

// ── The run endpoint: signature, role, inputs ─────────────────────────────────────────────────

/// THE SIGNATURE CHECK: unsigned, wrongly signed, signed for other inputs, signed for another
/// action, reused, and stale requests are all refused before anything runs; the right one runs.
/// Seen red 2026-10-22 with the `if !ok` refusal in `check_signed` taken out: "a signature by
/// another key is refused / left: 200 / right: 401".
#[tokio::test]
async fn the_run_endpoint_checks_the_signature() {
    let st = state("signature");
    let ada = person(0x41);
    st.db.set_role(&ada.key, "admin").unwrap();

    let mut unsigned = signed_run(&ada, "log.recent", "{}", tick());
    unsigned.sig.clear();
    assert_eq!(run(&st, unsigned).await.0, 401, "an unsigned request is refused");

    let mut forged = signed_run(&ada, "log.recent", "{}", tick());
    forged.sig = hex::encode(person(0x42).dil.sign(b"something else\n1"));
    assert_eq!(run(&st, forged).await.0, 401, "a signature by another key is refused");

    let mut other_inputs = signed_run(&ada, "alerts.remove", r#"{"index":0}"#, tick());
    other_inputs.inputs = r#"{"index":1}"#.into();
    let (code, _) = run(&st, other_inputs).await;
    assert_eq!(code, 401, "a signature over other inputs is refused: {code}");

    let mut other_action = signed_run(&ada, "log.recent", "{}", tick());
    other_action.action = "alerts.list".into();
    assert_eq!(run(&st, other_action).await.0, 401, "a signature for another action is refused");

    let stale = signed_run(&ada, "log.recent", "{}", now_ms() - 6 * 60 * 1000);
    assert_eq!(run(&st, stale).await.0, 400, "a stale request is refused");

    let good = signed_run(&ada, "log.recent", "{}", tick());
    let again = RunRequest { key: good.key.clone(), timestamp: good.timestamp, sig: good.sig.clone(), action: good.action.clone(), inputs: good.inputs.clone(), confirm: None };
    let (code, v) = run(&st, good).await;
    assert_eq!(code, 200, "the right signature runs: {v}");
    assert_eq!(v["ok"], true);
    assert_eq!(run(&st, again).await.0, 401, "the same request twice is refused the second time");
}

/// THE ROLE CHECK: a member is refused an admin action and a moderator action; a moderator is
/// refused an admin action and allowed a moderator one; an admin runs it. Seen red 2026-10-22 with
/// the role_refusal check taken out of post_run: "a member is refused alerts.list: 200".
#[tokio::test]
async fn the_run_endpoint_refuses_the_wrong_role() {
    let st = state("roles");
    let (ned, mo, ada) = (person(0x51), person(0x52), person(0x53));
    st.db.set_role(&mo.key, "mod").unwrap();
    st.db.set_role(&ada.key, "admin").unwrap();

    let (code, v) = run(&st, signed_run(&ned, "alerts.list", "{}", tick())).await;
    assert_eq!(code, 403, "a member is refused alerts.list: {code}");
    assert!(v["error"].as_str().unwrap().contains("Only this server's admins"), "{v}");
    assert_eq!(run(&st, signed_run(&ned, "members.kick", r#"{"name":"Ben"}"#, tick())).await.0, 403, "a member is refused a moderator action");
    assert_eq!(run(&st, signed_run(&mo, "federation.list", "{}", tick())).await.0, 403, "a moderator is refused an admin action");
    let (code, v) = run(&st, signed_run(&mo, "members.kick", r#"{"name":"Ben"}"#, tick())).await;
    assert_eq!(code, 404, "a moderator may kick (and is told Ben is not here): {v}");
    assert_eq!(run(&st, signed_run(&ada, "federation.list", "{}", tick())).await.0, 200, "an admin runs it");
}

/// An unknown action, an action that runs as a chat command, and bad inputs each say why.
#[tokio::test]
async fn the_run_endpoint_says_why_it_will_not_run() {
    let st = state("why");
    let ada = person(0x61);
    st.db.set_role(&ada.key, "admin").unwrap();
    let (code, v) = run(&st, signed_run(&ada, "alerts.explode", "{}", tick())).await;
    assert_eq!(code, 404, "{v}");
    let (code, v) = run(&st, signed_run(&ada, "registration.lockdown", "{}", tick())).await;
    assert_eq!(code, 400);
    assert!(v["error"].as_str().unwrap().contains("/lockdown"), "names the chat command: {v}");
    let (code, v) = run(&st, signed_run(&ada, "alerts.remove", r#"{"index":"zero"}"#, tick())).await;
    assert_eq!(code, 400, "{v}");
    let (code, _) = run(&st, signed_run(&ada, "alerts.remove", "not json", tick())).await;
    assert_eq!(code, 400);
}

// ── Alert channels, end to end ────────────────────────────────────────────────────────────────

/// A one-shot web server on 127.0.0.1 that answers `status` to one request and hands back what
/// it was sent.
async fn one_answer(status: u16) -> (String, tokio::sync::oneshot::Receiver<String>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut got = Vec::new();
        let mut buf = [0u8; 4096];
        // Read the head, then the body by its length.
        loop {
            let n = sock.read(&mut buf).await.unwrap_or(0);
            if n == 0 { break; }
            got.extend_from_slice(&buf[..n]);
            let text = String::from_utf8_lossy(&got).to_string();
            if let Some(head_end) = text.find("\r\n\r\n") {
                let len = text[..head_end]
                    .lines()
                    .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0)))
                    .unwrap_or(0);
                if got.len() >= head_end + 4 + len { break; }
            }
        }
        let reason = if status == 204 { "No Content" } else { "Not Found" };
        let _ = sock.write_all(format!("HTTP/1.1 {status} {reason}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()).await;
        let _ = tx.send(String::from_utf8_lossy(&got).to_string());
    });
    (format!("http://{addr}/hooks/very-secret-path-1234"), rx)
}

/// THE ALERT CHANNELS PANEL, the whole way: add (written in the shape the alert script reads),
/// list (the address never whole), change, turn off, test against a real answer and a refusal,
/// remove; the admin log names each change and never the address. Seen red 2026-10-22 with
/// `masked` returning the address as it is: "the address is never shown whole".
#[tokio::test]
async fn the_alert_channels_panel_the_whole_way() {
    let dir = std::env::temp_dir().join(format!("hum_alerts_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let file = dir.join("alert-channels.secrets.json");
    // Only this test sets it, and the alert actions are the only readers.
    std::env::set_var("HUMANITY_ALERT_CONFIG", &file);
    let st = state("alerts");
    let ada = person(0x71);
    st.db.register_name("Ada", &ada.key).unwrap();
    st.db.set_role(&ada.key, "admin").unwrap();

    let (code, v) = run(&st, signed_run(&ada, "alerts.list", "{}", tick())).await;
    assert_eq!(code, 200, "{v}");
    assert!(v["message"].as_str().unwrap().starts_with("No alert channels yet"), "a missing file is no channels: {v}");

    let (hook, got) = one_answer(204).await;
    let add = serde_json::json!({ "kind": "webhook", "label": "ops", "address": hook }).to_string();
    let (code, v) = run(&st, signed_run(&ada, "alerts.add", &add, tick())).await;
    assert_eq!(code, 200, "{v}");
    let written: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(written["channels"][0]["type"], "webhook", "the script's shape");
    assert_eq!(written["channels"][0]["url"], hook.as_str(), "the address under the field the script reads");
    assert_eq!(written["channels"][0]["enabled"], true);

    let (_, v) = run(&st, signed_run(&ada, "alerts.list", "{}", tick())).await;
    let shown = v.to_string();
    assert!(!shown.contains("very-secret-path"), "the address is never shown whole: {shown}");
    assert_eq!(v["data"]["channels"][0]["address"]["ends"], "1234");
    assert_eq!(v["data"]["channels"][0]["address"]["host"], "127.0.0.1");

    let (code, v) = run(&st, signed_run(&ada, "alerts.test", r#"{"index":0}"#, tick())).await;
    assert_eq!(code, 200, "a test that arrives: {v}");
    assert!(got.await.unwrap().contains("Test alert from"), "the other end got the test");

    let (bad_hook, _) = one_answer(404).await;
    let upd = serde_json::json!({ "index": 0, "address": bad_hook }).to_string();
    assert_eq!(run(&st, signed_run(&ada, "alerts.update", &upd, tick())).await.0, 200);
    let (code, v) = run(&st, signed_run(&ada, "alerts.test", r#"{"index":0}"#, tick())).await;
    assert_eq!(code, 502, "a test the other end refuses: {v}");
    assert!(v["error"].as_str().unwrap().contains("404") && !v.to_string().contains("very-secret-path"), "says the answer, never the address: {v}");

    let off = serde_json::json!({ "index": 0, "enabled": false }).to_string();
    assert_eq!(run(&st, signed_run(&ada, "alerts.set_enabled", &off, tick())).await.0, 200);
    let written: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(written["channels"][0]["enabled"], false);

    let bad = serde_json::json!({ "kind": "discord", "address": "http://discord.com/api/webhooks/1/x" }).to_string();
    assert_eq!(run(&st, signed_run(&ada, "alerts.add", &bad, tick())).await.0, 400, "Discord is https only");
    let tg = serde_json::json!({ "kind": "telegram", "address": "123456:ABCDEFGHIJKLMNOPQRSTUVWXYZ", "chat_id": "nope" }).to_string();
    assert_eq!(run(&st, signed_run(&ada, "alerts.add", &tg, tick())).await.0, 400, "a Telegram chat is a number or an @name");

    assert_eq!(run(&st, signed_run(&ada, "alerts.remove", r#"{"index":0}"#, tick())).await.0, 200);
    let written: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(written["channels"].as_array().unwrap().len(), 0);

    let (_, log) = run(&st, signed_run(&ada, "log.recent", "{}", tick())).await;
    let entries = log["data"]["entries"].as_array().unwrap();
    let actions: Vec<&str> = entries.iter().map(|e| e["action"].as_str().unwrap()).collect();
    assert!(actions.contains(&"alerts.add") && actions.contains(&"alerts.remove") && actions.contains(&"alerts.test"), "{actions:?}");
    assert!(!actions.contains(&"alerts.list"), "a read is not a change");
    assert!(entries.iter().all(|e| e["who"] == "Ada"), "named by the admin who ran it");
    assert!(!log.to_string().contains("very-secret-path") && !log.to_string().contains("127.0.0.1"), "the log never holds the address: {log}");
    let failed = entries.iter().find(|e| e["action"] == "alerts.test" && e["ok"] == false);
    assert!(failed.is_some(), "a test that did not arrive is logged as not working");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Masking: the host and the last characters of a long secret, fewer of a short one, nothing of
/// a tiny one. Seen red with `shown` fixed at 4: "a short secret shows less".
#[test]
fn a_secret_is_shown_by_its_host_and_its_last_characters() {
    let m = alerts::mask("https://discord.com/api/webhooks/123/abcdefghijkl");
    assert_eq!((m["set"].as_bool(), m["host"].as_str(), m["ends"].as_str()), (Some(true), Some("discord.com"), Some("ijkl")));
    assert_eq!(alerts::mask("12345678")["ends"], "78", "a short secret shows less");
    assert_eq!(alerts::mask("1234")["ends"], "");
    assert_eq!(alerts::mask("  ")["set"], false);
}

// ── Federation, through the registry ──────────────────────────────────────────────────────────

/// THE FEDERATION PANEL: add a peer, trust it, pause it (left out of what federation reads, kept
/// in the panel with its trust), resume, federate a channel and stop, remove; the legacy
/// /api/admin/federation route reaches the same functions. Seen red 2026-10-22 with the paused
/// filter taken out of `list_federated_servers` (misc.rs): "a paused peer is not one federation
/// reads"; and with federation.pause's line taken out of RUNNERS: "federation.pause is registered
/// to run here but this relay has no runner for it", 500.
#[tokio::test]
async fn the_federation_panel_the_whole_way() {
    let st = state("federation");
    let ada = person(0x81);
    st.db.set_role(&ada.key, "admin").unwrap();
    let add = r#"{"url":"http://127.0.0.1:9/","name":"Alpha"}"#;
    let (code, v) = run(&st, signed_run(&ada, "federation.add", add, tick())).await;
    assert_eq!(code, 200, "{v}");
    let id = "http://127.0.0.1:9";
    assert_eq!(v["data"]["peers"][0]["server_id"], id);
    assert_eq!(run(&st, signed_run(&ada, "federation.add", add, tick())).await.0, 409, "added twice");
    let trust = serde_json::json!({ "server_id": id, "tier": "2" }).to_string();
    let (_, v) = run(&st, signed_run(&ada, "federation.trust", &trust, tick())).await;
    assert_eq!(v["data"]["peers"][0]["federates"], true, "{v}");

    let pause = serde_json::json!({ "server_id": id }).to_string();
    let (code, v) = run(&st, signed_run(&ada, "federation.pause", &pause, tick())).await;
    assert_eq!(code, 200, "{v}");
    assert_eq!(v["data"]["peers"][0]["paused"], true);
    assert_eq!(v["data"]["peers"][0]["trust_tier"], 2, "its trust is kept");
    assert_eq!(v["data"]["peers"][0]["federates"], false);
    assert!(st.db.list_federated_servers().unwrap().is_empty(), "a paused peer is not one federation reads");
    assert_eq!(run(&st, signed_run(&ada, "federation.pause", &pause, tick())).await.0, 400, "already paused");
    assert_eq!(run(&st, signed_run(&ada, "federation.recheck", &pause, tick())).await.0, 400, "resume before re-checking");
    let (code, _) = run(&st, signed_run(&ada, "federation.resume", &pause, tick())).await;
    assert_eq!(code, 200);
    assert_eq!(st.db.list_federated_servers().unwrap().len(), 1, "resumed");

    st.db.create_channel("general", "general", None, "test", false).ok();
    let chan = r#"{"channel":"general","federated":true}"#;
    let (code, v) = run(&st, signed_run(&ada, "federation.channel", chan, tick())).await;
    assert_eq!(code, 200, "{v}");
    let general = v["data"]["channels"].as_array().unwrap().iter().find(|c| c["id"] == "general").cloned().unwrap();
    assert_eq!(general["federated"], true);
    assert!(st.db.is_channel_federated("general").unwrap());
    let (code, _) = run(&st, signed_run(&ada, "federation.channel", r#"{"channel":"nowhere","federated":true}"#, tick())).await;
    assert_eq!(code, 404);

    let (code, v) = run(&st, signed_run(&ada, "federation.remove", &pause, tick())).await;
    assert_eq!(code, 200, "{v}");
    assert!(v["data"]["peers"].as_array().unwrap().is_empty());
    assert_eq!(v["data"]["own_key"].as_str().map(|k| k.len()), Some(crate::relay::handlers::federation::SERVER_KEY_HEX_LEN), "this server's own key, to be added by");

    // The legacy route reaches the same functions.
    let ts = tick();
    let sig = hex::encode(ada.dil.sign(format!("admin_federation\n{ts}").as_bytes()));
    let req: crate::relay::api::AdminFederationRequest = serde_json::from_value(serde_json::json!({
        "key": ada.key, "timestamp": ts, "sig": sig, "action": "add", "url": "https://b.example", "name": "Beta"
    }))
    .unwrap();
    let res = crate::relay::api::post_admin_federation(State(st.clone()), Json(req)).await.unwrap();
    assert_eq!(res.0["servers"][0]["name"], "Beta");

    // THE LINK OBJECT (Federation Phases 2 and 3, `federation_log::peer_view`): both admin views
    // carry it for every peer, and for a peer with a key it holds the flow state the panels show.
    // Seen red 2026-10-24 with the "link" line taken out of the registry's view: "the registry's
    // federation view carries each peer's link".
    assert!(res.0["servers"][0].get("link").is_some(), "POST /api/admin/federation carries each peer's link");
    let by_key = serde_json::json!({ "key": "cd".repeat(1952), "name": "Home" }).to_string();
    let (code, v) = run(&st, signed_run(&ada, "federation.add_key", &by_key, tick())).await;
    assert_eq!(code, 200, "{v}");
    let home = v["data"]["peers"].as_array().unwrap().iter().find(|p| p["name"] == "Home").cloned().unwrap();
    assert!(home.get("link").is_some(), "the registry's federation view carries each peer's link");
    assert_eq!(home["link"]["flow"]["state"], "ok", "a peer with a key has a flow state: {home}");
    assert!(home["link"]["flow"]["missed"].is_number() && home["link"].get("cursor").is_some());
    let (_, log) = run(&st, signed_run(&ada, "log.recent", "{}", tick())).await;
    let actions: Vec<&str> = log["data"]["entries"].as_array().unwrap().iter().map(|e| e["action"].as_str().unwrap()).collect();
    assert!(actions.contains(&"federation.pause") && actions.contains(&"federation.channel"), "{actions:?}");
}
