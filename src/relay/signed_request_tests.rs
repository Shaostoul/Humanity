// Every family of signed REQUESTS on the relay, through its real route (2026-11-20).
//
// THE FAULT: requests were signed over `purpose\ntimestamp`, the very bytes a chat message is signed
// over (`text\ntimestamp`, the same key), so the chat line `account_export` was a credential for
// POST /api/account/export (and `bug_vote`, `vault_sync`, `system_profile`, a channel read...); and
// account export, the bug routes, the system profile, the admin stats and federation, the market,
// the group reads and the agents override took one signature any number of times inside five
// minutes. Now every route checks `signed_request::check`: signed over
// `hum/request/v1\n{purpose}\n{timestamp}`, fresh, used once.
//
// Each test drives one route family over HTTP on a relay bound to 127.0.0.1 and asserts, for every
// family: a chat-message signature of the bare purpose is refused; the same request signed for
// ANOTHER server is refused (round 2, 2026-11-21: requests name their server); the request a real
// client sends (signed by the very preimage builder the desktop and, through signed-request-v2.json,
// the website use, for this relay's own address) is accepted; that same request sent again is refused.
//
// ROUND 2 RED EVIDENCE (2026-11-21): with the `names.accepts` test taken out of `check_for` (the
// signature checked against whatever name the request's Host gave), every family failed at "a
// request signed for another server is refused even naming it (got 200)": account export, admin
// stats, the admin action run, the backups panel, the channel read, the Files page, push subscribe,
// the trade order, and the live frame.
//
// RED EVIDENCE (2026-11-21, `cargo test --features relay --no-default-features --lib -- signed_request`).
// (A) With `signed_request::verify` checking the old `{purpose}\n{timestamp}` (the chat shape): all
// nine tests here failed at "a chat line's signature over the bare purpose is refused" (got 200 from
// account export, admin stats, the admin action run, the backups panel, the channel read, the Files
// page, push subscribe and the trade order; the live frame was accepted, left: Some("lux")).
// (B) With the `auth_nonce_fresh` check taken out of `signed_request::check`: all nine failed at
// "the same request again is refused" (got 200 from the admin run, backups, admin stats, the
// channel read, the Files page, push and the trade order; 429 from account export, its per-minute
// limit, not a replay refusal; the live frame was accepted again). Before this change the account
// export, bug, system profile, admin stats and federation, market, group read and agents routes
// kept no replay memory at all.

use std::sync::Arc;

use humanity_protocol::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};
use serde_json::{json, Value};

use crate::relay::relay::RelayState;
use crate::relay::signed_request::request_preimage;
use crate::relay::storage::Storage;

/// A person: their keypair and public key hex.
struct Person {
    dil: DilithiumKeypair,
    key: String,
}

fn person(seed: u8) -> Person {
    let dil = DilithiumKeypair::from_seed(&derive_dilithium_seed(&[seed; 32]));
    let key = hex::encode(dil.public_key());
    Person { dil, key }
}

impl Person {
    /// The signature a chat message holding exactly `purpose` carries: `text\ntimestamp`.
    fn chat_sig(&self, purpose: &str, ts: u64) -> String {
        hex::encode(self.dil.sign(format!("{purpose}\n{ts}").as_bytes()))
    }
    /// The signature a real client sends for the request to the server at `aud` (`request_preimage`,
    /// which the desktop's `net::identity::pq_sign_request` calls and the website's pq.js is held to
    /// by the fixture).
    fn request_sig(&self, aud: &str, purpose: &str, ts: u64) -> String {
        hex::encode(self.dil.sign(request_preimage(aud, purpose, ts).as_bytes()))
    }
}

/// A fresh millisecond each call, never the same twice in this test run.
fn tick() -> u64 {
    crate::relay::signed_request::next_test_timestamp()
}

struct Relay {
    state: Arc<RelayState>,
    base: String,
    /// This relay's own address, the audience its requests are signed for.
    aud: String,
    http: reqwest::Client,
}

async fn relay(tag: &str) -> Relay {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let state = crate::relay::serve_relay(Storage::open_temp(tag), listener).await;
    crate::relay::default_channels::make_missing(&state.db, true);
    Relay { state, base: format!("http://127.0.0.1:{port}"), aud: format!("127.0.0.1:{port}"), http: reqwest::Client::new() }
}

impl Relay {
    /// A name here (a member already: chosen long enough ago), and a role when given.
    fn member(&self, p: &Person, name: &str, role: Option<&str>) {
        self.state.db.register_name(name, &p.key).unwrap();
        self.state.db.with_conn(|c| c.execute("UPDATE registered_names SET registered_at = 0 WHERE public_key = ?1", [&p.key])).unwrap();
        self.state.db.join_server(&p.key, name).unwrap();
        if let Some(r) = role {
            self.state.db.set_role(&p.key, r).unwrap();
        }
    }

    async fn send(&self, method: &str, path: &str, body: &Value) -> u16 {
        self.send_as(None, method, path, body).await
    }

    /// [`Self::send`] with the `Host` header naming `host` when given: what a server replaying a
    /// request it received sends, so this relay checks the signature against that name.
    async fn send_as(&self, host: Option<&str>, method: &str, path: &str, body: &Value) -> u16 {
        let url = format!("{}{path}", self.base);
        let mut req = match method {
            "PUT" => self.http.put(url),
            "DELETE" => self.http.delete(url),
            _ => self.http.post(url),
        };
        if let Some(h) = host {
            req = req.header(reqwest::header::HOST, h);
        }
        req.json(body).send().await.expect("the relay answers").status().as_u16()
    }
}

/// One signed route: how to send it, what it signs, and what an accepted request answers.
struct Route<'a> {
    family: &'a str,
    method: &'a str,
    path: String,
    purpose: String,
    accepted: u16,
    body: Box<dyn Fn(&str, u64, &str) -> Value + 'a>,
}

/// The four checks: a chat line's signature over the bare purpose refused, the request signed for
/// another server refused, the real client's request accepted, the same request again refused.
async fn three_checks(r: &Relay, p: &Person, route: Route<'_>) {
    let ts = tick();
    let chat = r.send(route.method, &route.path, &(route.body)(&p.key, ts, &p.chat_sig(&route.purpose, ts))).await;
    assert!(
        matches!(chat, 400 | 401),
        "{}: a chat line's signature over the bare purpose is refused (got {chat})",
        route.family
    );

    // Round 2 (D1): the same request, signed for another server, is refused here.
    let ts = tick();
    let body = (route.body)(&p.key, ts, &p.request_sig("other.example", &route.purpose, ts));
    let elsewhere = r.send(route.method, &route.path, &body).await;
    assert_eq!(elsewhere, 401, "{}: a request signed for another server is refused (got {elsewhere})", route.family);
    // ...and sent the way a replaying server would, naming that server as its Host.
    let named = r.send_as(Some("other.example"), route.method, &route.path, &body).await;
    assert_eq!(named, 401, "{}: a request signed for another server is refused even naming it (got {named})", route.family);

    let ts = tick();
    let body = (route.body)(&p.key, ts, &p.request_sig(&r.aud, &route.purpose, ts));
    let real = r.send(route.method, &route.path, &body).await;
    assert_eq!(real, route.accepted, "{}: the real client's request is accepted", route.family);

    let again = r.send(route.method, &route.path, &body).await;
    assert!(matches!(again, 401 | 409), "{}: the same request again is refused (got {again})", route.family);
}

fn signed(key: &str, ts: u64, sig: &str) -> Value {
    json!({ "key": key, "timestamp": ts, "sig": sig })
}

fn with(mut base: Value, extra: Value) -> Value {
    for (k, v) in extra.as_object().unwrap() {
        base[k] = v.clone();
    }
    base
}

#[test]
fn identity_routes_in_api_rs() {
    on_a_big_stack(|| async {
        let r = relay("sigreq_identity").await;
        let p = person(11);
        r.member(&p, "Ivy", None);
        let bug = r
            .state
            .db
            .create_bug("A", "B", "", "", "", "low", "other", "reporter", "Rowan", "", "", "0")
            .unwrap();
        for route in [
            Route { family: "account export", method: "POST", path: "/api/account/export".into(), purpose: "account_export".into(), accepted: 200, body: Box::new(signed) },
            Route {
                family: "bug vote",
                method: "POST",
                path: format!("/api/bugs/{bug}/vote"),
                purpose: crate::relay::api::bug_vote_purpose(bug),
                accepted: 200,
                body: Box::new(|k, ts, s| json!({ "voter_key": k, "timestamp": ts, "sig": s })),
            },
            Route { family: "vault sync", method: "PUT", path: "/api/vault/sync".into(), purpose: crate::relay::api::method_purpose("vault_sync", "PUT", Some(b"sealed")), accepted: 200, body: Box::new(|k, ts, s| with(signed(k, ts, s), json!({ "blob": "sealed" }))) },
            Route { family: "system profile", method: "PUT", path: "/api/me/system".into(), purpose: crate::relay::api::method_purpose("system_profile", "PUT", Some(b"{}")), accepted: 200, body: Box::new(|k, ts, s| with(signed(k, ts, s), json!({ "profile": "{}" }))) },
        ] {
            three_checks(&r, &p, route).await;
        }
    });
}

#[test]
fn admin_routes_in_api_rs() {
    on_a_big_stack(|| async {
        let r = relay("sigreq_admin_api").await;
        let ada = person(12);
        r.member(&ada, "Ada", Some("admin"));
        for route in [
            Route { family: "admin stats", method: "POST", path: "/api/admin/stats".into(), purpose: "admin_stats".into(), accepted: 200, body: Box::new(signed) },
            Route {
                family: "data files",
                method: "POST",
                path: "/api/data-files".into(),
                purpose: crate::relay::api::list_purpose("data"),
                accepted: 200,
                body: Box::new(|k, ts, s| with(signed(k, ts, s), json!({ "path": "data" }))),
            },
        ] {
            three_checks(&r, &ada, route).await;
        }
    });
}

#[test]
fn the_admin_action_registry() {
    on_a_big_stack(|| async {
        let r = relay("sigreq_admin_run").await;
        let ada = person(13);
        r.member(&ada, "Ada", Some("admin"));
        let route = Route {
            family: "admin action run",
            method: "POST",
            path: "/api/admin/actions/run".into(),
            purpose: crate::relay::admin::run_purpose("log.recent", "{}"),
            accepted: 200,
            body: Box::new(|k, ts, s| with(signed(k, ts, s), json!({ "action": "log.recent", "inputs": "{}" }))),
        };
        three_checks(&r, &ada, route).await;
    });
}

#[test]
fn the_files_page_routes() {
    on_a_big_stack(|| async {
        let r = relay("sigreq_files").await;
        let p = person(14);
        r.member(&p, "Fen", None);
        let route = Route { family: "files (mine)", method: "POST", path: "/api/files/mine".into(), purpose: "media_mine".into(), accepted: 200, body: Box::new(signed) };
        three_checks(&r, &p, route).await;
    });
}

#[test]
fn signed_channel_and_group_reads() {
    on_a_big_stack(|| async {
        let r = relay("sigreq_reads").await;
        let p = person(15);
        r.member(&p, "Rae", None);
        let channel = humanity_protocol::chat_access::MAIN_CHANNEL;
        for route in [
            Route {
                family: "channel read",
                method: "POST",
                path: "/api/messages".into(),
                purpose: humanity_protocol::chat_access::read_purpose("messages", channel),
                accepted: 200,
                body: Box::new(move |k, ts, s| with(signed(k, ts, s), json!({ "channel": channel }))),
            },
            Route { family: "group read", method: "POST", path: "/api/v2/groups".into(), purpose: crate::relay::api_v2_objects::group_read_purpose("mine"), accepted: 200, body: Box::new(signed) },
        ] {
            three_checks(&r, &p, route).await;
        }
    });
}

#[test]
fn the_push_subscription() {
    on_a_big_stack(|| async {
        let r = relay("sigreq_push").await;
        let p = person(16);
        r.member(&p, "Pia", None);
        let path = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("scripts/tests/fixtures/push-signing-v1.json");
        let f: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let (endpoint, p256dh, auth) = (f["endpoint"].as_str().unwrap().to_string(), f["p256dh"].as_str().unwrap().to_string(), f["auth"].as_str().unwrap().to_string());
        let route = Route {
            family: "push subscribe",
            method: "POST",
            path: "/api/push/subscribe".into(),
            purpose: format!("push_subscribe\n{endpoint}\n{p256dh}\n{auth}"),
            accepted: 200,
            body: Box::new(move |k, ts, s| with(signed(k, ts, s), json!({ "endpoint": endpoint, "p256dh": p256dh, "auth": auth }))),
        };
        three_checks(&r, &p, route).await;
    });
}

#[test]
fn the_backups_panel() {
    on_a_big_stack(|| async {
        let r = relay("sigreq_backups").await;
        let ada = person(17);
        r.member(&ada, "Ada", Some("admin"));
        let route = Route {
            family: "admin backups",
            method: "POST",
            path: "/api/admin/backups".into(),
            purpose: crate::relay::backup_stream::api::purpose("status", ""),
            accepted: 200,
            body: Box::new(|k, ts, s| with(signed(k, ts, s), json!({ "action": "status", "params": "" }))),
        };
        three_checks(&r, &ada, route).await;
    });
}

#[test]
fn the_market_and_the_agents_override() {
    on_a_big_stack(|| async {
        let r = relay("sigreq_market").await;
        let p = person(18);
        r.member(&p, "Max", None);
        for route in [
            Route {
                family: "trade order",
                method: "POST",
                path: "/api/trade/orders".into(),
                purpose: "trade_order\nwood\n\n5\n2.5\ncredits".into(),
                accepted: 200,
                body: Box::new(|k, ts, s| json!({ "public_key": k, "timestamp": ts, "signature": s, "item_type": "wood", "quantity": 5, "price_per_unit": 2.5, "currency": "credits" })),
            },
            // Not an admin: an accepted signature is answered 403 for the role, never 401. (An admin's
            // override writes a coordination file, which a test must not.)
            Route {
                family: "agents override",
                method: "POST",
                path: "/api/v2/agents/override".into(),
                purpose: crate::relay::api_v2_agents::override_purpose("x", "ok"),
                accepted: 403,
                body: Box::new(|k, ts, s| with(signed(k, ts, s), json!({ "scope_id": "x", "status": "ok" }))),
            },
        ] {
            three_checks(&r, &p, route).await;
        }
    });
}

/// C7 (2026-11-21, round 3): a category suggestion and an agents override name their targets, so a
/// signature made for one suggestion or one scope's status is refused for another. Red evidence
/// (2026-11-21, each route's purpose fixed to one target whatever the body names): "a suggestion's
/// signature carries another suggestion" failed, left 200; and the override's route test
/// (`the_market_and_the_agents_override`) failed at "the real client's request is accepted", left
/// 401, the signature for scope x status ok no longer good for the fixed target.
#[test]
fn suggestions_and_overrides_name_their_targets() {
    on_a_big_stack(|| async {
        let r = relay("sigreq_targets").await;
        let p = person(20);
        r.member(&p, "Tam", None);
        let ts = tick();
        let sig = p.request_sig(&r.aud, &crate::relay::files::suggest_purpose("Builds", "Ships"), ts);
        let other = with(signed(&p.key, ts, &sig), json!({ "name": "Spam", "note": "Ships" }));
        assert_eq!(r.send("POST", "/api/files/suggest", &other).await, 401, "a suggestion's signature carries another suggestion");
        let real = with(signed(&p.key, ts, &sig), json!({ "name": "Builds", "note": "Ships" }));
        assert_eq!(r.send("POST", "/api/files/suggest", &real).await, 200, "the suggestion it was made for is taken");

        let ts = tick();
        let sig = p.request_sig(&r.aud, &crate::relay::api_v2_agents::override_purpose("x", "active"), ts);
        let other = with(signed(&p.key, ts, &sig), json!({ "scope_id": "x", "status": "off" }));
        assert_eq!(r.send("POST", "/api/v2/agents/override", &other).await, 401, "an override's signature sets another status");
        let other = with(signed(&p.key, ts, &sig), json!({ "scope_id": "y", "status": "active" }));
        assert_eq!(r.send("POST", "/api/v2/agents/override", &other).await, 401, "or another scope");
    });
}

/// The live publisher's auth frame (a WebSocket's first message, so checked through its own
/// function rather than a route).
#[test]
fn the_live_publisher() {
    on_a_big_stack(|| async {
        let r = relay("sigreq_live").await;
        let p = person(19);
        r.member(&p, "Lux", None);
        let frame = |ts: u64, sig: String| -> crate::relay::live::AuthFrame { serde_json::from_value(json!({ "key": p.key, "timestamp": ts, "sig": sig })).unwrap() };
        let at = |host: &str| crate::relay::audience::Arrival { host: Some(host.to_string()), peer: Some(std::net::Ipv4Addr::LOCALHOST.into()), ..Default::default() };
        let here = &at(&r.aud);
        let ts = tick();
        assert_eq!(crate::relay::live::authenticate(&r.state, &frame(ts, p.chat_sig("live_publish", ts)), here).await, None, "live: a chat line's signature is refused");
        let ts = tick();
        let elsewhere = frame(ts, p.request_sig("other.example", "live_publish", ts));
        assert_eq!(crate::relay::live::authenticate(&r.state, &elsewhere, here).await, None, "live: a frame signed for another server is refused");
        assert_eq!(crate::relay::live::authenticate(&r.state, &elsewhere, &at("other.example")).await, None, "live: and naming that server here changes nothing");
        let ts = tick();
        let real = frame(ts, p.request_sig(&r.aud, "live_publish", ts));
        assert_eq!(crate::relay::live::authenticate(&r.state, &real, here).await.as_deref(), Some("lux"), "live: the real client's frame is accepted");
        assert_eq!(crate::relay::live::authenticate(&r.state, &real, here).await, None, "live: the same frame again is refused");
    });
}

/// Run `test` on a thread with a 64 MB stack and a runtime of its own: Dilithium steps and `json!`
/// in a debug build overflow a test thread's 2 MB (as chat_access_tests.rs does).
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
