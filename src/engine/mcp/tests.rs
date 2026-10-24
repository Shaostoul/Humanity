//! The dev MCP endpoint's tests (`just test-lib engine::mcp`).
//!
//! RED FIRST (2026-10-23, docs/ai/mcp.md "Tests"). Run against an endpoint with eight faults put
//! in on purpose, every test failed, each at its own assertion (19 of 19):
//!   - `handle` answering `{}` to everything: the handshake, discovery, listing, header,
//!     unknown-tool and every round-trip test;
//!   - `decide` starting anywhere: "the switch off ... is refused" (assertion left == right);
//!   - `bind_loopback` skipping its check (still binding 127.0.0.1 only, so no firewall prompt):
//!     "0.0.0.0:0: TcpListener { addr: 127.0.0.1:58421, .. }";
//!   - the funnel's door shut: "the game did not take mcp_test_..._request.json within 10 s";
//!   - no verb refused: "accept_request must be refused";
//!   - the server-off-this-computer refusal gone, the token not checked, and
//!     `door_points_request.json` renamed on the tools' side only ("engine/ipc.rs no longer
//!     names debug/door_point_request.json").
//! Before any fault, the screenshot test caught a real defect: a mixed-separator path.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use super::http::{self, Request};
use super::link::{GameLink, LiveLink};
use super::protocol::{self, Headers, ToolHost};
use super::tools::{self, GameTools};
use super::{decide, server_is_local, Start};

// ── A double of the game's dev IPC ──────────────────────────────────────────────────────────────

type Answer = Box<dyn Fn(&FakeGame, &str, &Value) + Send + Sync>;

/// Plays the game's part on a temp folder: records each request handed in and answers it the way
/// the handler would (a done file, a picture, a log line), through `answer`.
struct FakeGame {
    dir: crate::test_temp::TempPath,
    requests: Mutex<Vec<(String, Value)>>,
    server_local: bool,
    answer: Answer,
}

impl FakeGame {
    fn new(answer: Answer) -> Arc<FakeGame> {
        let dir = crate::test_temp::dir("mcp_game");
        std::fs::create_dir_all(dir.join("debug")).unwrap();
        std::fs::create_dir_all(dir.join("logs")).unwrap();
        std::fs::write(dir.join("logs").join("run.log"), "[t INFO  boot] ===== HumanityOS starting =====\n").unwrap();
        Arc::new(FakeGame { dir, requests: Mutex::new(Vec::new()), server_local: true, answer })
    }
    fn debug(&self, name: &str) -> PathBuf {
        self.dir.join("debug").join(name)
    }
    fn log(&self, line: &str) {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().append(true).open(self.dir.join("logs").join("run.log")).unwrap();
        writeln!(f, "{line}").unwrap();
    }
    fn sent(&self) -> Vec<(String, Value)> {
        self.requests.lock().unwrap().clone()
    }
}

impl GameLink for FakeGame {
    fn submit(&self, request: &str, body: &str, _timeout: Duration) -> Result<(), String> {
        let v: Value = serde_json::from_str(body).map_err(|e| format!("the tool sent a body that is not JSON: {e}"))?;
        self.requests.lock().unwrap().push((request.to_string(), v.clone()));
        (self.answer)(self, request, &v);
        Ok(())
    }
    fn game_dir(&self) -> PathBuf {
        self.dir.to_path_buf()
    }
    fn run_log(&self) -> Option<PathBuf> {
        Some(self.dir.join("logs").join("run.log"))
    }
    fn crash_log(&self) -> Option<PathBuf> {
        Some(self.dir.join("logs").join("crash.log"))
    }
    fn frame_costs(&self) -> Value {
        json!({ "frame_ms": 16.6, "gpu_timing": "timestamps", "gpu_ms": { "gpu.celestial": 3.5, "gpu.scene": 7.25 } })
    }
    fn frames(&self) -> u64 {
        1
    }
    fn server_local(&self) -> bool {
        self.server_local
    }
}

fn host(game: &Arc<FakeGame>) -> GameTools {
    GameTools::new(game.clone() as Arc<dyn GameLink>)
}

/// What a ToolHost answers a tools/call with, as the protocol carries it.
fn call(h: &dyn ToolHost, name: &str, args: Value) -> Value {
    let r = protocol::handle(&json!({ "jsonrpc": "2.0", "id": 9, "method": "tools/call", "params": { "name": name, "arguments": args } }), None, h);
    assert_eq!(r.status, 200, "{r:?}");
    r.body.unwrap()["result"].clone()
}

fn text_of(result: &Value) -> String {
    result["content"].as_array().unwrap().iter().filter_map(|c| c["text"].as_str()).collect::<Vec<_>>().join("\n")
}

fn no_answer() -> Answer {
    Box::new(|_, _, _| {})
}

// ── The protocol: handshake, discovery, listing ─────────────────────────────────────────────────

/// A legacy client (2025-11-25 and earlier) opens with initialize and gets its own version back,
/// the tools capability and the server's name; a version this does not speak is answered with
/// the newest it does; the initialized notification is taken with no body (HTTP 202).
#[test]
fn legacy_handshake_answers_initialize_and_the_initialized_notification() {
    let game = FakeGame::new(no_answer());
    let h = host(&game);
    let init = |v: &str| {
        protocol::handle(
            &json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": v, "capabilities": {}, "clientInfo": { "name": "t", "version": "0" } } }),
            None,
            &h,
        )
    };
    let r = init("2025-06-18").body.unwrap();
    assert_eq!(r["id"], 1);
    assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
    assert!(r["result"]["capabilities"]["tools"].is_object(), "{r}");
    assert_eq!(r["result"]["serverInfo"]["name"], "humanityos-game");
    assert_eq!(init("1999-01-01").body.unwrap()["result"]["protocolVersion"], "2025-11-25");
    let n = protocol::handle(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }), None, &h);
    assert_eq!((n.status, n.body), (202, None));
    let ping = protocol::handle(&json!({ "jsonrpc": "2.0", "id": "p", "method": "ping" }), None, &h).body.unwrap();
    assert_eq!(ping["result"], json!({}));
}

/// A modern client (2026-07-28) asks server/discover and names its version on every request; a
/// version this does not speak is refused with -32022 listing the ones it does, HTTP 400.
#[test]
fn modern_discovery_and_an_unsupported_version_is_refused_with_the_supported_list() {
    let game = FakeGame::new(no_answer());
    let h = host(&game);
    let meta = |v: &str| json!({ "_meta": { protocol::META_VERSION: v, "io.modelcontextprotocol/clientCapabilities": {} } });
    let d = protocol::handle(&json!({ "jsonrpc": "2.0", "id": 1, "method": "server/discover", "params": meta("2026-07-28") }), None, &h).body.unwrap();
    let versions: Vec<&str> = d["result"]["supportedVersions"].as_array().unwrap().iter().filter_map(|v| v.as_str()).collect();
    assert!(versions.contains(&"2026-07-28") && versions.contains(&"2025-11-25"), "{versions:?}");
    assert_eq!(d["result"]["resultType"], "complete");
    assert!(d["result"]["capabilities"]["tools"].is_object());

    let l = protocol::handle(&json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": meta("2026-07-28") }), None, &h).body.unwrap();
    assert_eq!(l["result"]["resultType"], "complete");
    assert_eq!(l["result"]["ttlMs"], 0);
    assert_eq!(l["result"]["cacheScope"], "private");
    assert_eq!(l["result"]["_meta"][protocol::META_SERVER_INFO]["name"], "humanityos-game");

    let bad = protocol::handle(&json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/list", "params": meta("2099-01-01") }), None, &h);
    assert_eq!(bad.status, 400);
    let body = bad.body.unwrap();
    let e = &body["error"];
    assert_eq!(e["code"], protocol::UNSUPPORTED_PROTOCOL_VERSION);
    assert_eq!(e["data"]["requested"], "2099-01-01");
    assert!(e["data"]["supported"].as_array().unwrap().iter().any(|v| v == "2026-07-28"));
}

/// The tool list is tools.json, every entry a tool with a handler and every handler listed:
/// a tool added to one side only would be listed and fail, or work and never be offered.
#[test]
fn every_listed_tool_has_a_handler_and_back() {
    let game = FakeGame::new(no_answer());
    let h = host(&game);
    let r = protocol::handle(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }), None, &h).body.unwrap();
    let listed: Vec<String> = r["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
    let mut handled: Vec<String> = tools::handled().iter().map(|s| s.to_string()).collect();
    let mut sorted = listed.clone();
    sorted.sort();
    handled.sort();
    assert_eq!(sorted, handled, "tools.json and the registry (tools.rs HANDLERS) must name the same tools");
    sorted.dedup();
    assert_eq!(sorted.len(), listed.len(), "a tool is listed twice");
    for t in r["result"]["tools"].as_array().unwrap() {
        assert!(t["description"].as_str().is_some_and(|d| d.len() > 40), "{} needs a real description", t["name"]);
        assert_eq!(t["inputSchema"]["type"], "object", "{} needs an object input schema", t["name"]);
    }
}

/// On HTTP the method, the version and a tool call's name ride in headers too, and must agree
/// with the body (2026-07-28 Server Validation): -32020, HTTP 400. A name sent base64-encoded is
/// decoded before the comparison.
#[test]
fn http_headers_must_agree_with_the_body() {
    let game = FakeGame::new(Box::new(|g: &FakeGame, req: &str, _: &Value| {
        if req == "showcase_request.json" {
            g.log("[t INFO  ipc] Showcase: page -> Homes");
        }
    }));
    let h = host(&game);
    let body = json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call",
        "params": { "name": "game_open_page", "arguments": { "page": "homes" }, "_meta": { protocol::META_VERSION: "2026-07-28" } } });
    let hdr = |m: &str, n: Option<&str>| Headers { protocol_version: Some("2026-07-28".into()), method: Some(m.into()), name: n.map(String::from) };
    let wrong_method = protocol::handle(&body, Some(&hdr("tools/list", Some("game_open_page"))), &h);
    assert_eq!(wrong_method.status, 400);
    assert_eq!(wrong_method.body.unwrap()["error"]["code"], protocol::HEADER_MISMATCH);
    let wrong_name = protocol::handle(&body, Some(&hdr("tools/call", Some("game_logs"))), &h);
    assert_eq!(wrong_name.body.unwrap()["error"]["code"], protocol::HEADER_MISMATCH);
    let no_name = protocol::handle(&body, Some(&hdr("tools/call", None)), &h);
    assert_eq!(no_name.body.unwrap()["error"]["code"], protocol::HEADER_MISMATCH);
    let encoded = protocol::decode_header_value("=?base64?Z2FtZV9vcGVuX3BhZ2U=?=");
    assert_eq!(encoded.as_deref(), Some("game_open_page"));
    let ok = protocol::handle(&body, Some(&hdr("tools/call", encoded.as_deref())), &h);
    assert_eq!(ok.status, 200, "{ok:?}");
    assert_eq!(ok.body.unwrap()["result"]["resultType"], "complete");
    assert!(game.sent().len() == 1, "only the agreeing request reached the game: {:?}", game.sent());
}

/// An unknown tool is a protocol error (-32602) and an unknown method -32601, never a silent
/// empty answer.
#[test]
fn unknown_tools_and_methods_are_errors() {
    let game = FakeGame::new(no_answer());
    let h = host(&game);
    let t = protocol::handle(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "game_fly_to_mars" } }), None, &h);
    assert_eq!(t.body.unwrap()["error"]["code"], protocol::INVALID_PARAMS);
    let m = protocol::handle(&json!({ "jsonrpc": "2.0", "id": 2, "method": "resources/list" }), None, &h);
    assert_eq!(m.body.unwrap()["error"]["code"], protocol::METHOD_NOT_FOUND);
    assert!(game.sent().is_empty());
}

// ── Tool round trips against the double ─────────────────────────────────────────────────────────

/// game_screenshot hands the game a screenshot request, reads the PNG its done file names, and
/// returns it inline as a JPEG no longer than max_inline_px on its long edge, with the absolute
/// path of the lossless PNG.
#[test]
fn screenshot_round_trip_returns_the_picture_and_its_path() {
    let game = FakeGame::new(Box::new(|g: &FakeGame, req: &str, _: &Value| {
        if req == "screenshot_request.json" {
            let img = image::RgbImage::from_fn(3000, 1000, |x, y| image::Rgb([(x % 251) as u8, (y % 241) as u8, 90]));
            img.save(g.debug("screenshot_7.png")).unwrap();
            std::fs::write(g.debug("screenshot_done.json"), r#"{"ok":true,"path":"debug/screenshot_7.png","fps":59.0}"#).unwrap();
        }
    }));
    let r = call(&host(&game), "game_screenshot", json!({}));
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(game.sent(), vec![("screenshot_request.json".to_string(), json!({}))]);
    let png = game.debug("screenshot_7.png");
    assert!(text_of(&r).contains(&png.display().to_string()), "the text names the PNG: {}", text_of(&r));
    let image = r["content"].as_array().unwrap().iter().find(|c| c["type"] == "image").expect("an image block");
    assert_eq!(image["mimeType"], "image/jpeg");
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD.decode(image["data"].as_str().unwrap()).unwrap();
    let decoded = image::load_from_memory(&bytes).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (1568, 523), "scaled to a 1568 long edge, aspect kept");
    assert_eq!(r["structuredContent"]["width"], 3000);
    // A hi-res capture asks for both sizes; one alone is refused before anything is sent.
    let half = call(&host(&game), "game_screenshot", json!({ "width": 800 }));
    assert_eq!(half["isError"], true);
    assert_eq!(game.sent().len(), 1);
}

/// A capture the game could not make is the tool's error, in the game's words.
#[test]
fn a_failed_capture_is_reported_as_the_games_own_error() {
    let game = FakeGame::new(Box::new(|g: &FakeGame, _: &str, _: &Value| {
        std::fs::write(g.debug("screenshot_done.json"), r#"{"ok":false,"error":"the swapchain does not allow copies"}"#).unwrap();
    }));
    let r = call(&host(&game), "game_screenshot", json!({}));
    assert_eq!(r["isError"], true);
    assert!(text_of(&r).contains("the swapchain does not allow copies"), "{}", text_of(&r));
}

/// game_player_state takes one zero-second reading of the recorder and says where the player is,
/// which room the HUD names, their health and what they carry.
#[test]
fn player_state_round_trip_summarises_the_recorders_reading() {
    let game = FakeGame::new(Box::new(|g: &FakeGame, req: &str, body: &Value| {
        assert_eq!((req, body), ("remote_players_request.json", &json!({ "seconds": 0 })));
        let done = json!({
            "ok": true, "world_loaded": true, "aboard_ship": true, "game_joined": false, "notices": [],
            "frames": [{ "cam": [1.5, 2.0, -3.25, 0.5, -0.1], "health": 97.0, "planet_r": null,
                "storey": { "room": { "name": "Dining room" }, "floor": 0.0, "ceiling": 2.8, "aboard": true, "on_ladder": false, "on_lift": false, "hud_shown": true } }],
            "home_session": { "slot": "offline", "path": "saves/offline_home.json", "credits": 120,
                "backpack": { "wood_plank": 40, "apple": 3, "water_bottle": 2 }, "vitals": { "satiation": 80.0, "hydration": 70.0 } },
        });
        std::fs::write(g.debug("remote_players_done.json"), done.to_string()).unwrap();
    }));
    let r = call(&host(&game), "game_player_state", json!({}));
    assert_eq!(r["isError"], false, "{r}");
    let s = &r["structuredContent"];
    assert_eq!(s["camera"]["pos"], json!([1.5, 2.0, -3.25]));
    assert_eq!(s["camera"]["yaw"], 0.5);
    assert_eq!(s["storey"]["room"]["name"], "Dining room");
    assert_eq!(s["health"], 97.0);
    assert_eq!(s["carrying"]["items"], 45);
    assert_eq!(s["carrying"]["kinds"], 3);
    assert_eq!(s["vitals"]["hydration"], 70.0);
    assert_eq!(s["world_loaded"], true);
}

/// game_console runs one console line through the showcase `console` verb and returns what the
/// console said, from the game's own log line.
#[test]
fn console_returns_what_the_console_said() {
    let game = FakeGame::new(Box::new(|g: &FakeGame, req: &str, body: &Value| {
        assert_eq!(req, "showcase_request.json");
        assert_eq!(body, &json!({ "console": "help" }));
        g.log("[t INFO  ipc] Showcase: console help -> add_structure <kind> x y z turns | rm_structure <id>");
    }));
    let r = call(&host(&game), "game_console", json!({ "line": "help" }));
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(text_of(&r), "add_structure <kind> x y z turns\nrm_structure <id>");
}

/// The showcase verbs that reach a server or act for the identity are refused by name and
/// NOTHING is sent: not the refused verb, and not the harmless ones beside it.
#[test]
fn showcase_refuses_server_and_identity_verbs_and_sends_nothing() {
    let game = FakeGame::new(no_answer());
    for verb in tools::REFUSED_VERBS {
        let r = call(&host(&game), "game_showcase", json!({ "verbs": { "time": "12", *verb: "1" } }));
        assert_eq!(r["isError"], true, "{verb} must be refused");
        assert!(text_of(&r).contains(verb), "the refusal names {verb}: {}", text_of(&r));
    }
    assert!(game.sent().is_empty(), "nothing reached the game: {:?}", game.sent());
    // An allowed request goes through with its values as the showcase reads them.
    let ok = call(&host(&game), "game_showcase", json!({ "verbs": { "time": 12, "hud": false, "weather": "clear" } }));
    assert_eq!(ok["isError"], false);
    assert_eq!(game.sent(), vec![("showcase_request.json".to_string(), json!({ "time": "12", "hud": "0", "weather": "clear" }))]);
}

/// If the game's server ever leaves this computer, every call is refused before it reaches the
/// game.
#[test]
fn every_call_is_refused_once_the_server_is_off_this_computer() {
    let mut g = Arc::try_unwrap(FakeGame::new(no_answer())).ok().unwrap();
    g.server_local = false;
    let game = Arc::new(g);
    for name in tools::handled() {
        let r = call(&host(&game), name, json!({}));
        assert_eq!(r["isError"], true, "{name}");
        assert!(text_of(&r).contains("off this computer"), "{name}: {}", text_of(&r));
    }
    assert!(game.sent().is_empty());
}

/// game_logs returns the tail, a filtered tail, and the PANIC blocks the panic hook wrote.
#[test]
fn logs_return_the_tail_and_the_panics() {
    let game = FakeGame::new(no_answer());
    for i in 0..10 {
        game.log(&format!("[t INFO  x] line {i}"));
    }
    game.log("\n========== PANIC (HumanityOS v0.1) ==========\npanicked at src/lib.rs:1:1:\nboom\n--- backtrace ---\n============================================");
    let tail = call(&host(&game), "game_logs", json!({ "lines": 3, "contains": "line" }));
    assert_eq!(text_of(&tail), "[t INFO  x] line 7\n[t INFO  x] line 8\n[t INFO  x] line 9");
    let panics = call(&host(&game), "game_logs", json!({ "panics": true }));
    assert_eq!(panics["structuredContent"]["panics"], 1);
    assert!(text_of(&panics).contains("boom"), "{}", text_of(&panics));
}

/// game_frame_costs puts the heaviest GPU pass first.
#[test]
fn frame_costs_lead_with_the_heaviest_pass() {
    let game = FakeGame::new(no_answer());
    let r = call(&host(&game), "game_frame_costs", json!({}));
    let t = text_of(&r);
    assert!(t.find("gpu.scene").unwrap() < t.find("gpu.celestial").unwrap(), "{t}");
}

// ── The switch, the sandbox and the bind ───────────────────────────────────────────────────────

/// Off unless HUMANITY_DEV_MCP is set; refused (and said why) outside a portable sandbox or with
/// a server off this computer.
#[test]
fn the_switch_off_starts_nothing_and_a_real_install_or_a_public_server_is_refused() {
    assert_eq!(decide(None, true, ""), Start::Off);
    assert_eq!(decide(Some(""), true, ""), Start::Off);
    assert_eq!(decide(Some("0"), true, ""), Start::Off);
    assert!(matches!(decide(Some("1"), false, ""), Start::Refused(w) if w.contains("portable")));
    assert!(matches!(decide(Some("1"), true, "wss://united-humanity.us/ws"), Start::Refused(w) if w.contains("off this computer")));
    assert!(matches!(decide(Some("1"), true, "ws://192.168.1.20:8080"), Start::Refused(_)));
    assert_eq!(decide(Some("1"), true, ""), Start::Go);
    assert_eq!(decide(Some("1"), true, "ws://127.0.0.1:9/ws"), Start::Go);
    for local in ["", "ws://localhost:3210", "http://127.0.0.1", "ws://[::1]:9", "ws://127.5.0.1:9", "ws://user@127.0.0.1:9/x"] {
        assert!(server_is_local(local), "{local:?} is on this computer");
    }
    for far in ["wss://united-humanity.us", "ws://10.0.0.2:9", "ws://127.0.0.1.evil.com:9", "ws://localhost.example:9", "ws://0.0.0.0:9"] {
        assert!(!server_is_local(far), "{far:?} is not on this computer");
    }
}

/// The endpoint never listens off this computer: a wildcard or network address is refused before
/// any socket exists (so no Windows Firewall prompt either), loopback binds.
#[test]
fn a_non_loopback_bind_is_refused() {
    for addr in ["0.0.0.0:0", "192.168.1.20:0", "10.0.0.5:0", "[::]:0"] {
        let e = http::bind_loopback(addr.parse().unwrap()).expect_err(addr);
        assert!(e.contains("this computer only"), "{addr}: {e}");
    }
    let l = http::bind_loopback("127.0.0.1:0".parse().unwrap()).expect("loopback binds");
    assert!(l.local_addr().unwrap().ip().is_loopback());
}

/// The HTTP gate: the endpoint's path, a Host and an Origin on this computer, POST, the token,
/// JSON. Each refusal is the answer the transport names.
#[test]
fn the_http_gate_refuses_what_it_should() {
    let port = 4242;
    let ok = || Request {
        method: "POST".into(),
        path: "/mcp".into(),
        headers: vec![
            ("Host".into(), format!("127.0.0.1:{port}")),
            ("Authorization".into(), "Bearer t0k3n".into()),
            ("Content-Type".into(), "application/json".into()),
        ],
        body: Vec::new(),
    };
    assert!(http::admit(&ok(), port, "t0k3n").is_ok());
    let status = |r: Request| http::admit(&r, port, "t0k3n").err().map(|e| e.status);
    let with = |k: &str, v: &str| {
        let mut r = ok();
        r.headers.retain(|(h, _)| !h.eq_ignore_ascii_case(k));
        r.headers.push((k.into(), v.into()));
        r
    };
    assert_eq!(status(with("Authorization", "Bearer wrong")), Some(401));
    assert_eq!(status(with("Host", &format!("evil.example:{port}"))), Some(403));
    assert_eq!(status(with("Host", "127.0.0.1:9999")), Some(403));
    assert_eq!(status(with("Origin", "http://evil.example")), Some(403));
    assert!(http::admit(&with("Origin", &format!("http://localhost:{port}")), port, "t0k3n").is_ok());
    assert_eq!(status(with("Content-Type", "text/plain")), Some(415));
    let mut get = ok();
    get.method = "GET".into();
    let refused = http::admit(&get, port, "t0k3n").unwrap_err();
    assert_eq!((refused.status, refused.allow_post_only), (405, true));
    let mut elsewhere = ok();
    elsewhere.path = "/".into();
    assert_eq!(status(elsewhere), Some(404));
    let mut no_token = ok();
    no_token.headers.retain(|(h, _)| h != "Authorization");
    assert_eq!(status(no_token), Some(401));
}

/// End to end over a real loopback socket: a POST with the token lists the tools; without it the
/// answer is 401.
#[test]
fn a_real_loopback_round_trip_lists_the_tools() {
    use std::io::{Read, Write};
    let game = FakeGame::new(no_answer());
    let listener = http::bind_loopback("127.0.0.1:0".parse().unwrap()).unwrap();
    let port = listener.local_addr().unwrap().port();
    let h: Arc<dyn ToolHost + Send + Sync> = Arc::new(host(&game));
    http::serve(listener, "s3cret".into(), h).unwrap();
    let post = |token: &str| -> String {
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }).to_string();
        let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(
            s,
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        out
    };
    let good = post("s3cret");
    assert!(good.starts_with("HTTP/1.1 200 OK"), "{good}");
    let json_body: Value = serde_json::from_str(good.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert!(json_body["result"]["tools"].as_array().unwrap().iter().any(|t| t["name"] == "game_screenshot"));
    assert!(post("guess").starts_with("HTTP/1.1 401"), "a wrong token is refused");
}

// ── The real link against the real funnel ───────────────────────────────────────────────────────

/// LiveLink hands a request to the dev IPC's own funnel (engine/ipc.rs `take_request_text`), the
/// one every poller reads its file through, and returns once the poller took it and a frame ran
/// after. Played here by a thread that does what the main loop does each frame: count the frame,
/// then poll the request.
#[test]
fn the_live_link_goes_through_the_ipc_funnel_and_waits_a_frame() {
    super::ACTIVE.store(true, std::sync::atomic::Ordering::SeqCst);
    let name = format!("mcp_test_{}_request.json", std::process::id());
    let path = PathBuf::from(format!("debug/{name}"));
    let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let main_loop = {
        let (seen, stop, path) = (seen.clone(), stop.clone(), path.clone());
        std::thread::spawn(move || {
            while !stop.load(std::sync::atomic::Ordering::SeqCst) {
                super::FRAME.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if let Some(text) = crate::engine::ipc::take_request_text(&path) {
                    seen.lock().unwrap().push(text);
                }
                std::thread::sleep(Duration::from_millis(2));
            }
        })
    };
    let link = LiveLink::new(std::env::temp_dir(), None);
    link.submit(&name, r#"{"seconds":0}"#, Duration::from_secs(10)).expect("taken and a frame ran");
    assert_eq!(seen.lock().unwrap().as_slice(), [r#"{"seconds":0}"#.to_string()]);
    stop.store(true, std::sync::atomic::Ordering::SeqCst);
    main_loop.join().unwrap();
    // Nobody polls now: a request is not taken, the call says so, and it is withdrawn rather than
    // left to fire later.
    let e = link.submit(&name, "{}", Duration::from_millis(100)).expect_err("nobody took it");
    assert!(e.contains("did not take"), "{e}");
    assert!(crate::engine::ipc::take_request_text(&path).is_none(), "the withdrawn request is gone");
}

/// The requests and done files the tools name are the ones the game's pollers use: a renamed
/// request would otherwise leave a tool waiting on a file nobody reads.
#[test]
fn the_tools_name_the_requests_the_game_polls() {
    let ipc = include_str!("../ipc.rs");
    let map_rig = include_str!("../map_rig.rs");
    let mut names = vec![
        "screenshot_request.json",
        "screenshot_done.json",
        "remote_players_request.json",
        "remote_players_done.json",
        "camera_request.json",
        "camera_done.json",
        "showcase_request.json",
    ];
    for (_, req, done) in tools::DUMP_REQUESTS {
        names.push(req);
        names.push(done);
    }
    for n in names {
        assert!(ipc.contains(&format!("\"debug/{n}\"")), "engine/ipc.rs no longer names debug/{n}");
    }
    assert!(map_rig.contains("\"debug/map_done.json\""), "engine/map_rig.rs no longer names debug/map_done.json");
    assert!(ipc.contains("Showcase: console {line} -> "), "game_console reads this log line; engine/ipc.rs no longer writes it");
    assert!(ipc.contains("\"home_systems\": crate::engine::home_probe::json"), "the recorder carries home_systems");
}
