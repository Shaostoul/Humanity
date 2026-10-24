//! The game tools the dev MCP endpoint serves, each a thin call into the dev IPC through a
//! `GameLink` (link.rs): the same request a rig drops as a file, answered by the same handler,
//! read back from the same done file. Their names, descriptions and argument schemas are
//! tools.json (also read by scripts/mcp-server.js), and every name there has a handler here.
//!
//! What no tool here does, by design (docs/ai/mcp.md "Safety"): send a message, sign anything,
//! spend, change the identity, or reach a server. The showcase verbs that would (a contact
//! request accepted, a start chosen on a server, a meal from the fleet, a piece built on the
//! server) are refused by name; the screen and UI requests, which click through the app as a
//! person would, are not offered at all; and the endpoint only starts in a portable sandbox whose
//! server is on this computer (mod.rs `decide`), refusing every call if that ever changes.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};

use super::link::GameLink;
use super::protocol::{text_result, tool_error, ToolHost};

/// The tool list, compiled in (and read from the tree by scripts/mcp-server.js).
pub(crate) const TOOLS_JSON: &str = include_str!("tools.json");

/// Every tool's definition, in tools.json's order, without the file's own notes.
pub(crate) fn definitions() -> &'static [Value] {
    static DEFS: OnceLock<Vec<Value>> = OnceLock::new();
    DEFS.get_or_init(|| {
        serde_json::from_str::<Value>(TOOLS_JSON)
            .ok()
            .and_then(|v| v.get("tools").and_then(|t| t.as_array()).cloned())
            .unwrap_or_default()
    })
}

/// A tool's handler: the link to the game and the call's arguments, to a CallToolResult.
pub(crate) type Handler = fn(&dyn GameLink, &Value) -> Value;

/// THE REGISTRY: every game tool's name and handler. Adding a tool is two edits: its entry in
/// tools.json (name, description, input schema) and its row here. The test
/// `every_listed_tool_has_a_handler_and_back` holds the two to each other, and
/// scripts/mcp-server.js lists the new tool from tools.json without a change.
pub(crate) const HANDLERS: &[(&str, Handler)] = &[
    ("game_screenshot", screenshot),
    ("game_player_state", player_state),
    ("game_camera", camera),
    ("game_console", console),
    ("game_showcase", showcase_tool),
    ("game_open_page", open_page),
    ("game_map", map),
    ("game_home_state", home_state),
    ("game_frame_costs", frame_costs),
    ("game_logs", logs),
    ("game_wait", wait),
    ("game_request", dump_request),
];

/// The names the registry has a handler for.
pub(crate) fn handled() -> Vec<&'static str> {
    HANDLERS.iter().map(|(n, _)| *n).collect()
}

/// Showcase verbs no MCP tool sends, each because it reaches a server or acts for the identity:
/// accept_request answers a contact request with a pass; choose_start and pairing pick and join a
/// server's world; meal and water draw on the fleet's ledger; place and take_down build or take
/// down pieces the server keeps; drop_link and solo cut or leave the shared world. The rigs that
/// need them (verify-copresence, verify-starts, verify-household) run against a throwaway relay of
/// their own.
pub(crate) const REFUSED_VERBS: &[&str] = &[
    "accept_request",
    "choose_start",
    "pairing",
    "meal",
    "water",
    "place",
    "take_down",
    "drop_link",
    "solo",
];

/// The other read-only requests `game_request` offers: (kind, request file, done file).
pub(crate) const DUMP_REQUESTS: &[(&str, &str, &str)] = &[
    ("remote_players", "remote_players_request.json", "remote_players_done.json"),
    ("door_points", "door_points_request.json", "door_points_done.json"),
    ("cloudmap", "cloudmap_request.json", "cloudmap_done.json"),
    ("cloud_profile_dump", "cloud_profile_dump_request.json", "cloud_profile_done.json"),
];

/// How long the game has to take a request (a loading frame can take seconds).
const TAKE_TIMEOUT: Duration = Duration::from_secs(30);
/// The longest text a tool returns; past it the answer is cut and points at the file.
const MAX_TEXT: usize = 100_000;

/// The game's tools, served over a link.
pub(crate) struct GameTools {
    link: Arc<dyn GameLink>,
    /// One call at a time: two calls handing the same request in at once would overwrite each
    /// other in the funnel, and both would then read one answer.
    busy: Mutex<()>,
}

impl GameTools {
    pub(crate) fn new(link: Arc<dyn GameLink>) -> Self {
        GameTools { link, busy: Mutex::new(()) }
    }
}

impl ToolHost for GameTools {
    fn tools(&self) -> Vec<Value> {
        definitions().to_vec()
    }

    fn call(&self, name: &str, args: &Value) -> Option<Value> {
        let (_, handler) = HANDLERS.iter().find(|(n, _)| *n == name)?;
        let _one = self.busy.lock().unwrap_or_else(|p| p.into_inner());
        let link = &*self.link;
        if !link.server_local() {
            return Some(tool_error(
                "refused: this game now names a server off this computer, and the dev MCP endpoint only drives a sandbox \
                 whose server is on this computer (docs/ai/mcp.md, Safety). Boot a fresh rig with game_boot.",
            ));
        }
        Some(handler(link, args))
    }

    fn server_info(&self) -> Value {
        json!({ "name": "humanityos-game", "title": "HumanityOS (the running game)", "version": env!("CARGO_PKG_VERSION") })
    }

    fn instructions(&self) -> String {
        "Drives and inspects the running HumanityOS game through its dev IPC. Look with game_screenshot, read the player \
         with game_player_state, move with game_camera (or game_showcase's walk, face and hold), read problems with \
         game_logs. Pins set with game_showcase stay set until changed. Nothing here sends messages, signs, spends or \
         reaches a server. docs/ai/mcp.md."
            .to_string()
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────────────────────

fn debug_path(link: &dyn GameLink, name: &str) -> PathBuf {
    link.game_dir().join("debug").join(name)
}

fn clear_done(link: &dyn GameLink, name: &str) {
    let _ = std::fs::remove_file(debug_path(link, name));
}

/// Wait for a done file to hold whole JSON (the game writes some with a plain write, so a read
/// can catch one half-written: that is read again, not trusted).
fn wait_done(link: &dyn GameLink, name: &str, timeout: Duration) -> Result<Value, String> {
    let path = debug_path(link, name);
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<Value>(&text) {
                return Ok(v);
            }
        }
        if Instant::now() > deadline {
            return Err(format!("the game wrote no {name} within {:.0} s ({})", timeout.as_secs_f32(), path.display()));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Clear the done file, hand the request in, and wait for the answer.
fn request_and_wait(link: &dyn GameLink, request: &str, body: &Value, done: &str, timeout: Duration) -> Result<Value, String> {
    clear_done(link, done);
    link.submit(request, &body.to_string(), TAKE_TIMEOUT)?;
    wait_done(link, done, timeout)
}

fn log_len(link: &dyn GameLink) -> u64 {
    link.run_log().and_then(|p| std::fs::metadata(p).ok()).map_or(0, |m| m.len())
}

/// The log's text from byte `from` on.
fn log_since(link: &dyn GameLink, from: u64) -> String {
    use std::io::{Read, Seek, SeekFrom};
    let Some(path) = link.run_log() else { return String::new() };
    let Ok(mut f) = std::fs::File::open(path) else { return String::new() };
    if f.seek(SeekFrom::Start(from)).is_err() {
        return String::new();
    }
    let mut bytes = Vec::new();
    let _ = f.read_to_end(&mut bytes);
    String::from_utf8_lossy(&bytes).into_owned()
}

/// A showcase value as the request carries it: text as it is, a number in its shortest form,
/// true as "1" and false as "0" (the switches' own spelling).
fn verb_value(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(if *b { "1" } else { "0" }.to_string()),
        _ => None,
    }
}

/// Send one showcase request and return the game's log lines about it (every "Showcase" line,
/// and every warning, written while it was handled).
fn showcase(link: &dyn GameLink, verbs: &Map<String, Value>) -> Result<Vec<String>, String> {
    let mut body = Map::new();
    for (k, v) in verbs {
        let s = verb_value(v).ok_or_else(|| format!("showcase verb {k:?}: the value must be text, a number or true/false"))?;
        body.insert(k.clone(), Value::String(s));
    }
    let from = log_len(link);
    link.submit("showcase_request.json", &Value::Object(body).to_string(), TAKE_TIMEOUT)?;
    let said = log_since(link, from);
    Ok(said
        .lines()
        .filter(|l| l.contains("Showcase") || l.contains(" WARN ") || l.contains(" ERROR "))
        .map(|l| l.trim_end().to_string())
        .collect())
}

/// Text cut to MAX_TEXT, saying so.
fn bounded(text: String, more_at: Option<&Path>) -> String {
    if text.len() <= MAX_TEXT {
        return text;
    }
    let mut cut = MAX_TEXT;
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    let tail = more_at.map(|p| format!(" The whole answer is {}.", p.display())).unwrap_or_default();
    format!("{}\n... (cut at {MAX_TEXT} characters.{tail})", &text[..cut])
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}

fn secs(args: &Value, key: &str, default: f64, max: f64) -> Duration {
    let s = args.get(key).and_then(|v| v.as_f64()).filter(|s| s.is_finite() && *s > 0.0).unwrap_or(default);
    Duration::from_secs_f64(s.min(max))
}

// ── The tools ─────────────────────────────────────────────────────────────────────────────────

fn screenshot(link: &dyn GameLink, args: &Value) -> Value {
    let w = args.get("width").and_then(|v| v.as_u64());
    let h = args.get("height").and_then(|v| v.as_u64());
    let body = match (w, h) {
        (Some(w), Some(h)) => json!({ "width": w, "height": h }),
        (None, None) => json!({}),
        _ => return tool_error("give both width and height for an offscreen capture, or neither for the window"),
    };
    let max_inline = args.get("max_inline_px").and_then(|v| v.as_u64()).unwrap_or(1568).min(4096) as u32;
    let done = match request_and_wait(link, "screenshot_request.json", &body, "screenshot_done.json", Duration::from_secs(120)) {
        Ok(d) => d,
        Err(e) => return tool_error(format!("screenshot: {e}")),
    };
    if done.get("ok").and_then(|o| o.as_bool()) != Some(true) {
        let why = done.get("error").and_then(|e| e.as_str()).unwrap_or("no reason given");
        return tool_error(format!("the game could not capture: {why}"));
    }
    let Some(rel) = done.get("path").and_then(|p| p.as_str()) else {
        return tool_error(format!("screenshot_done.json named no picture: {done}"));
    };
    // The done file names it as the game's poller does, "debug/screenshot_N.png": joined piece by
    // piece, so the path handed back uses this system's separator throughout (a mixed
    // "C:\rig\debug/screenshot_3.png" was what the round-trip test caught).
    let abs = link.game_dir().join(rel.split(['/', '\\']).filter(|p| !p.is_empty()).collect::<PathBuf>());
    let mut content = Vec::new();
    let mut structured = json!({ "path": abs.display().to_string(), "fps": done.get("fps"), "camera_home": done.get("camera_home") });
    let mut line = format!("Screenshot saved: {}", abs.display());
    if max_inline > 0 {
        match inline_jpeg(&abs, max_inline) {
            Ok(img) => {
                line.push_str(&format!(" ({}x{}; shown at {}x{})", img.full_w, img.full_h, img.w, img.h));
                structured["width"] = json!(img.full_w);
                structured["height"] = json!(img.full_h);
                content.push(json!({ "type": "text", "text": line }));
                content.push(json!({ "type": "image", "data": img.base64, "mimeType": "image/jpeg" }));
            }
            Err(e) => {
                line.push_str(&format!(" (no inline copy: {e})"));
                content.push(json!({ "type": "text", "text": line }));
            }
        }
    } else {
        content.push(json!({ "type": "text", "text": line }));
    }
    json!({ "content": content, "structuredContent": structured, "isError": false })
}

/// A picture made small enough to send inline.
pub(crate) struct InlineImage {
    pub base64: String,
    pub w: u32,
    pub h: u32,
    pub full_w: u32,
    pub full_h: u32,
}

/// Read a capture and make its inline copy: JPEG at quality 85, its long edge at most `max_px`
/// (1568 by default, the size a model reads a picture at without scaling it again). The PNG on
/// disk is untouched: the path is the lossless one.
pub(crate) fn inline_jpeg(path: &Path, max_px: u32) -> Result<InlineImage, String> {
    use base64::Engine as _;
    let img = image::open(path).map_err(|e| format!("could not read {}: {e}", path.display()))?;
    let (full_w, full_h) = (img.width(), img.height());
    let long = full_w.max(full_h).max(1);
    let img = if long > max_px {
        let scale = max_px as f64 / long as f64;
        let w = ((full_w as f64 * scale).round() as u32).max(1);
        let h = ((full_h as f64 * scale).round() as u32).max(1);
        img.resize_exact(w, h, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let rgb = img.to_rgb8();
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 85)
        .encode_image(&rgb)
        .map_err(|e| format!("could not encode the inline copy: {e}"))?;
    Ok(InlineImage {
        base64: base64::engine::general_purpose::STANDARD.encode(&bytes),
        w: rgb.width(),
        h: rgb.height(),
        full_w,
        full_h,
    })
}

/// One reading of the recorder (a zero-second remote_players_request).
fn probe(link: &dyn GameLink) -> Result<Value, String> {
    request_and_wait(link, "remote_players_request.json", &json!({ "seconds": 0 }), "remote_players_done.json", Duration::from_secs(30))
}

/// The player as a person would describe them, out of the recorder's answer.
pub(crate) fn player_summary(done: &Value) -> Value {
    let f0 = done.pointer("/frames/0").cloned().unwrap_or(Value::Null);
    let cam = f0.get("cam").and_then(|c| c.as_array()).cloned().unwrap_or_default();
    let n = |i: usize| cam.get(i).cloned().unwrap_or(Value::Null);
    let hs = done.get("home_session").cloned().unwrap_or(Value::Null);
    // What the character carries: the 15 largest stacks and the totals.
    let backpack = hs.get("backpack").and_then(|b| b.as_object()).cloned().unwrap_or_default();
    let mut stacks: Vec<(String, u64)> = backpack.iter().map(|(k, v)| (k.clone(), v.as_u64().unwrap_or(0))).collect();
    stacks.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let total: u64 = stacks.iter().map(|s| s.1).sum();
    let largest: Map<String, Value> = stacks.iter().take(15).map(|(k, q)| (k.clone(), json!(q))).collect();
    let storey = f0.get("storey").cloned().unwrap_or(Value::Null);
    json!({
        "world_loaded": done.get("world_loaded"),
        "aboard_ship": done.get("aboard_ship"),
        "camera": { "pos": [n(0), n(1), n(2)], "yaw": n(3), "pitch": n(4) },
        "storey": {
            "room": storey.get("room"),
            "floor": storey.get("floor"),
            "ceiling": storey.get("ceiling"),
            "aboard": storey.get("aboard"),
            "on_ladder": storey.get("on_ladder"),
            "on_lift": storey.get("on_lift"),
            "hud_shown": storey.get("hud_shown"),
        },
        "health": f0.get("health"),
        "vitals": hs.get("vitals"),
        "planet_r": f0.get("planet_r"),
        "carrying": { "kinds": stacks.len(), "items": total, "largest": largest },
        "credits": hs.get("credits"),
        "save": { "slot": hs.get("slot"), "path": hs.get("path"), "start": hs.get("start") },
        "falls": done.get("falls"),
        "notices": done.get("notices"),
        "game_joined": done.get("game_joined"),
    })
}

fn player_state(link: &dyn GameLink, args: &Value) -> Value {
    let done = match probe(link) {
        Ok(d) => d,
        Err(e) => return tool_error(format!("player state: {e}")),
    };
    if args.get("full").and_then(|f| f.as_bool()) == Some(true) {
        let at = debug_path(link, "remote_players_done.json");
        return text_result(bounded(pretty(&done), Some(at.as_path())), Some(done));
    }
    let s = player_summary(&done);
    text_result(pretty(&s), Some(s))
}

fn camera(link: &dyn GameLink, args: &Value) -> Value {
    let Some(req) = args.get("request").filter(|r| r.is_object()) else {
        return tool_error("camera: `request` must be the camera_request object");
    };
    let timeout = secs(args, "timeout_s", 60.0, 600.0);
    match request_and_wait(link, "camera_request.json", req, "camera_done.json", timeout) {
        Ok(done) => {
            if done.get("ok").and_then(|o| o.as_bool()) == Some(true) {
                text_result(pretty(&done), Some(done))
            } else {
                let why = done.get("error").and_then(|e| e.as_str()).unwrap_or("no reason given").to_string();
                let mut r = tool_error(format!("the camera did not park: {why}"));
                r["structuredContent"] = done;
                r
            }
        }
        Err(e) => tool_error(format!("camera: {e}")),
    }
}

fn console(link: &dyn GameLink, args: &Value) -> Value {
    let Some(line) = args.get("line").and_then(|l| l.as_str()).map(str::trim).filter(|l| !l.is_empty()) else {
        return tool_error("console: `line` is the console line to run");
    };
    let mut verbs = Map::new();
    verbs.insert("console".into(), json!(line));
    match showcase(link, &verbs) {
        Ok(lines) => {
            // engine/ipc.rs logs "Showcase: console <line> -> <what it said>".
            let marker = format!("Showcase: console {line} -> ");
            match lines.iter().find_map(|l| l.find(&marker).map(|i| l[i + marker.len()..].to_string())) {
                Some(said) => text_result(said.replace(" | ", "\n"), Some(json!({ "said": said }))),
                None => tool_error(format!(
                    "the game took the line but logged no answer for it; its showcase lines were:\n{}",
                    lines.join("\n")
                )),
            }
        }
        Err(e) => tool_error(format!("console: {e}")),
    }
}

fn showcase_tool(link: &dyn GameLink, args: &Value) -> Value {
    let Some(verbs) = args.get("verbs").and_then(|v| v.as_object()) else {
        return tool_error("showcase: `verbs` must be an object of verb or pin names to values");
    };
    if verbs.is_empty() {
        return tool_error("showcase: no verbs given");
    }
    let refused: Vec<&str> = verbs.keys().map(String::as_str).filter(|k| REFUSED_VERBS.contains(k)).collect();
    if !refused.is_empty() {
        return tool_error(format!(
            "refused, nothing was sent: {} reach a server or act for the identity, which no MCP tool does (docs/ai/mcp.md, Safety). \
             The rigs that need them run against a throwaway relay of their own (verify-copresence, verify-starts, verify-household).",
            refused.join(", ")
        ));
    }
    match showcase(link, verbs) {
        Ok(lines) => {
            let text = if lines.is_empty() {
                "Taken. (The game logged nothing about it: most pins apply silently; read the result with game_screenshot or game_player_state.)".to_string()
            } else {
                lines.join("\n")
            };
            text_result(text, Some(json!({ "log": lines })))
        }
        Err(e) => tool_error(format!("showcase: {e}")),
    }
}

fn open_page(link: &dyn GameLink, args: &Value) -> Value {
    let Some(page) = args.get("page").and_then(|p| p.as_str()).map(str::trim).filter(|p| !p.is_empty()) else {
        return tool_error("open page: `page` is the page id (homes, inventory, maps, dev, none, ...)");
    };
    let mut verbs = Map::new();
    verbs.insert("page".into(), json!(page));
    match showcase(link, &verbs) {
        Ok(lines) => {
            if let Some(w) = lines.iter().find(|l| l.contains("Showcase: page wants")) {
                return tool_error(w.clone());
            }
            text_result(lines.join("\n"), Some(json!({ "log": lines })))
        }
        Err(e) => tool_error(format!("open page: {e}")),
    }
}

fn map(link: &dyn GameLink, args: &Value) -> Value {
    let Some(verbs) = args.get("verbs").and_then(|v| v.as_object()) else {
        return tool_error("map: `verbs` must be an object of map verbs");
    };
    if let Some(bad) = verbs.keys().find(|k| !k.starts_with("map")) {
        return tool_error(format!("map: {bad:?} is not a map verb (every map verb starts with `map`); use game_showcase for other verbs"));
    }
    let mut verbs = verbs.clone();
    verbs.insert("map_report".into(), json!("1"));
    clear_done(link, "map_done.json");
    let lines = match showcase(link, &verbs) {
        Ok(l) => l,
        Err(e) => return tool_error(format!("map: {e}")),
    };
    if let Some(w) = lines.iter().find(|l| l.contains(" WARN ") && l.contains("map")) {
        return tool_error(format!("the game refused the map verbs: {w}"));
    }
    match wait_done(link, "map_done.json", secs(args, "timeout_s", 30.0, 120.0)) {
        Ok(done) => {
            let at = debug_path(link, "map_done.json");
            text_result(bounded(pretty(&done), Some(at.as_path())), Some(done))
        }
        Err(e) => tool_error(format!("map: {e} (is the Maps page open? game_open_page maps)")),
    }
}

fn home_state(link: &dyn GameLink, _args: &Value) -> Value {
    let done = match probe(link) {
        Ok(d) => d,
        Err(e) => return tool_error(format!("home state: {e}")),
    };
    let room_gi_path = debug_path(link, "room_gi.json");
    let room_gi = match std::fs::read_to_string(&room_gi_path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
        Some(mut v) => {
            let age = std::fs::metadata(&room_gi_path).ok().and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok());
            v["file_age_s"] = json!(age.map(|a| a.as_secs_f64()));
            v
        }
        None => json!({ "absent": "debug/room_gi.json is written once a second only when the game runs with HUMANITY_FRAME_COSTS=1 (game_boot sets it)" }),
    };
    let home_systems = done.get("home_systems").cloned().unwrap_or_else(|| {
        json!({ "absent": "this build's recorder predates home_systems (engine/home_probe.rs): rebuild the release exe and boot again" })
    });
    let out = json!({
        "home_systems": home_systems,
        "storey": done.pointer("/frames/0/storey"),
        "elevators": done.pointer("/storeys/elevators"),
        "aboard_ship": done.get("aboard_ship"),
        "room_gi": room_gi,
    });
    text_result(bounded(pretty(&out), None), Some(out))
}

fn frame_costs(link: &dyn GameLink, _args: &Value) -> Value {
    let v = link.frame_costs();
    let mut gpu: Vec<(String, f64)> = v
        .get("gpu_ms")
        .and_then(|g| g.as_object())
        .map(|g| g.iter().map(|(k, ms)| (k.clone(), ms.as_f64().unwrap_or(0.0))).collect())
        .unwrap_or_default();
    gpu.sort_by(|a, b| b.1.total_cmp(&a.1));
    let top: Vec<String> = gpu.iter().take(8).map(|(k, ms)| format!("  {k}: {ms:.2} ms")).collect();
    let head = format!(
        "frame {:.2} ms ({}), heaviest GPU passes:\n{}",
        v.get("frame_ms").and_then(|f| f.as_f64()).unwrap_or(0.0),
        v.get("gpu_timing").and_then(|t| t.as_str()).unwrap_or("?"),
        if top.is_empty() { "  (none measured yet)".to_string() } else { top.join("\n") }
    );
    text_result(format!("{head}\n\n{}", pretty(&v)), Some(v))
}

/// The last `n` lines of a log (read from its last 4 MB), those containing `contains` only.
pub(crate) fn tail_lines(path: &Path, n: usize, contains: Option<&str>) -> Result<Vec<String>, String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let from = len.saturating_sub(4 * 1024 * 1024);
    f.seek(SeekFrom::Start(from)).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&bytes);
    let needle = contains.map(str::to_lowercase);
    let mut lines: Vec<String> = text
        .lines()
        .skip(usize::from(from > 0)) // the first line of a cut read may be partial
        .filter(|l| needle.as_ref().is_none_or(|n| l.to_lowercase().contains(n.as_str())))
        .map(str::to_string)
        .collect();
    let keep = lines.len().saturating_sub(n);
    Ok(lines.split_off(keep))
}

/// Every PANIC block in a log (the panic hook's "========== PANIC" to its closing rule).
pub(crate) fn panic_blocks(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur: Option<Vec<&str>> = None;
    for l in text.lines() {
        if l.contains("========== PANIC") {
            cur = Some(vec![l]);
            continue;
        }
        if let Some(block) = cur.as_mut() {
            block.push(l);
            if l.starts_with("====") && l.chars().all(|c| c == '=') {
                out.push(block.join("\n"));
                cur = None;
            }
        }
    }
    if let Some(block) = cur {
        out.push(block.join("\n"));
    }
    out
}

fn logs(link: &dyn GameLink, args: &Value) -> Value {
    let Some(run_log) = link.run_log() else {
        return tool_error("this game's log folder is not known");
    };
    if args.get("panics").and_then(|p| p.as_bool()) == Some(true) {
        let text = std::fs::read_to_string(&run_log).unwrap_or_default();
        let blocks = panic_blocks(&text);
        let crash = link
            .crash_log()
            .and_then(|c| tail_lines(&c, 40, None).ok())
            .map(|l| l.join("\n"))
            .unwrap_or_default();
        let head = if blocks.is_empty() {
            format!("No PANIC in this launch's log ({}).", run_log.display())
        } else {
            format!("{} PANIC block(s) in {}:\n\n{}", blocks.len(), run_log.display(), blocks.join("\n\n"))
        };
        let text = format!("{head}\n\nThe end of crash.log (kept across launches):\n{crash}");
        return text_result(bounded(text, Some(run_log.as_path())), Some(json!({ "panics": blocks.len() })));
    }
    let n = args.get("lines").and_then(|v| v.as_u64()).unwrap_or(60).clamp(1, 2000) as usize;
    let contains = args.get("contains").and_then(|c| c.as_str()).filter(|c| !c.is_empty());
    match tail_lines(&run_log, n, contains) {
        Ok(lines) => text_result(bounded(lines.join("\n"), Some(run_log.as_path())), Some(json!({ "path": run_log.display().to_string(), "lines": lines.len() }))),
        Err(e) => tool_error(format!("could not read the log: {e}")),
    }
}

fn wait(link: &dyn GameLink, args: &Value) -> Value {
    let timeout = secs(args, "timeout_s", 60.0, 600.0);
    let deadline = Instant::now() + timeout;
    let value = args.get("value");
    match args.get("until").and_then(|u| u.as_str()).unwrap_or("") {
        "frames" => {
            let n = value.and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))).unwrap_or(1);
            let start = link.frames();
            while link.frames() < start + n {
                if Instant::now() > deadline {
                    return tool_error(format!("only {} of {n} frames ran in {:.0} s", link.frames() - start, timeout.as_secs_f32()));
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            text_result(format!("{n} frame(s) ran"), Some(json!({ "frames": link.frames() })))
        }
        "seconds" => {
            let s = value.and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))).unwrap_or(1.0);
            let d = Duration::from_secs_f64(s.clamp(0.0, timeout.as_secs_f64()));
            std::thread::sleep(d);
            text_result(format!("waited {:.1} s", d.as_secs_f64()), None)
        }
        "world_loaded" => loop {
            match probe(link) {
                Ok(done) if done.get("world_loaded").and_then(|w| w.as_bool()) == Some(true) => {
                    return text_result("the world is loaded", Some(json!({ "world_loaded": true })))
                }
                Ok(_) | Err(_) if Instant::now() > deadline => {
                    return tool_error(format!("the world was not loaded within {:.0} s", timeout.as_secs_f32()))
                }
                _ => std::thread::sleep(Duration::from_millis(500)),
            }
        },
        "log" => {
            let Some(text) = value.and_then(|v| v.as_str()).filter(|t| !t.is_empty()) else {
                return tool_error("wait for log: `value` is the text to wait for");
            };
            let needle = text.to_lowercase();
            let from = log_len(link);
            loop {
                if let Some(line) = log_since(link, from).lines().find(|l| l.to_lowercase().contains(&needle)) {
                    return text_result(line.to_string(), Some(json!({ "line": line })));
                }
                if Instant::now() > deadline {
                    return tool_error(format!("no log line containing {text:?} in {:.0} s", timeout.as_secs_f32()));
                }
                std::thread::sleep(Duration::from_millis(200));
            }
        }
        other => tool_error(format!("wait: `until` is frames, seconds, world_loaded or log, not {other:?}")),
    }
}

fn dump_request(link: &dyn GameLink, args: &Value) -> Value {
    let kind = args.get("kind").and_then(|k| k.as_str()).unwrap_or("");
    let Some(&(_, request, done)) = DUMP_REQUESTS.iter().find(|(k, _, _)| *k == kind) else {
        let kinds: Vec<&str> = DUMP_REQUESTS.iter().map(|d| d.0).collect();
        return tool_error(format!("request: kind is one of {}, not {kind:?}", kinds.join(", ")));
    };
    let body = args.get("body").cloned().filter(|b| b.is_object()).unwrap_or_else(|| json!({}));
    // A recording answers after its seconds: wait at least that long.
    let recording = body.get("seconds").and_then(|s| s.as_f64()).unwrap_or(0.0).clamp(0.0, 600.0);
    let timeout = secs(args, "timeout_s", (60.0f64).max(recording + 30.0), 600.0);
    match request_and_wait(link, request, &body, done, timeout) {
        Ok(v) => {
            let at = debug_path(link, done);
            text_result(bounded(pretty(&v), Some(at.as_path())), Some(v))
        }
        Err(e) => tool_error(format!("{kind}: {e}")),
    }
}
