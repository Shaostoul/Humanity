//! Calls through the server (step E of docs/design/blocking-and-safe-mode.md, section 10f,
//! 2026-10-09): the desktop app's half of asking for call credentials and reading them.
//!
//! A voice room or an accepted 1:1 call connects only through the server's own forwarder
//! ("relay only"), so the other people in it see the server's address and never this device's.
//! Before it connects, the app asks the server over the signed-in chat socket:
//!
//! ```text
//! {"type":"call_credentials","room":"<voice room id>"}
//! {"type":"call_credentials","call":"<the other person's key>"}
//! ```
//!
//! and the server answers only someone in that voice room or in an open call with that person:
//!
//! ```text
//! {"type":"call_credentials","room"|"call":"...",
//!  "urls":["turn:<host>:<port>?transport=udp","stun:<host>:<port>"],
//!  "username":"<expiry>:<room tag>","credential":"<base64 HMAC>","ttl":3600}
//! ```
//!
//! Anyone else is refused at once, with the same type and scope and no credentials:
//!
//! ```text
//! {"type":"call_credentials","room"|"call":"...","refused":true}
//! ```
//!
//! A reply without a usable `turn:` address, a username and a credential counts as a refusal
//! whatever else it says. When credentials are refused, or none come within [`CREDENTIALS_WAIT`]
//! (a server older than this step answers nothing), or they come and the forwarder never answers
//! (its UDP port is not open yet), the call says [`NOT_SET_UP_LINE`] and nothing falls back to a
//! direct connection.
//!
//! Credentials are asked for once per call or room. The allocation made with them is refreshed
//! with the same credentials for as long as it lives: the relay accepts that past their `ttl`
//! (up to a cap of its own), so nothing is asked again in the middle of a call.
//!
//! This file is the protocol and a small state machine, with no socket and no GUI, so each rule
//! is unit tested here. The glue to the app's state is src/engine/call_relay.rs; the forwarder
//! client (TURN, RFC 5766) is `mod turn` in src/net/webrtc.rs.

use std::time::{Duration, Instant};

use serde_json::Value;

/// What the call UI says when a call or a voice room cannot go through the server: no
/// credentials came, or the forwarder did not answer. Worded in the spec (10f).
pub const NOT_SET_UP_LINE: &str =
    "Calls go through the server to keep your address private; this server is not set up for that yet.";

/// What the call UI says when the server's forwarder stops answering in the middle of a call.
pub const LOST_LINE: &str =
    "The call's connection through the server was lost. Leave and join again to reconnect.";

/// How long the app waits for its `call_credentials` answer before it says the server is not
/// set up for calls. A server that knows the request answers in one round trip, a refusal
/// included, so this is only the backstop for one that never answers. The web waits as long.
pub const CREDENTIALS_WAIT: Duration = Duration::from_secs(8);

/// The port a `turn:` address means when it names none (RFC 7065 section 3).
const DEFAULT_TURN_PORT: u16 = 3478;

/// What one set of credentials is for: one voice room, or one accepted 1:1 call. The server
/// ties an allocation made with them to that room or call, and its forwarder only ever passes
/// packets between allocations of the same one.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CallScope {
    /// A voice room, by its id (the text channel's id).
    Room(String),
    /// A 1:1 call, by the other person's key.
    Call(String),
}

impl CallScope {
    /// The scope of a voice connection to `peer`, from the room id the WebRTC manager tags it
    /// with: the reserved call id means the 1:1 call with that person.
    pub fn for_voice_peer(room_id: &str, peer: &str) -> CallScope {
        if room_id == crate::net::webrtc::CALL_ROOM_ID {
            CallScope::Call(peer.to_string())
        } else {
            CallScope::Room(room_id.to_string())
        }
    }

    /// The request sent on the chat socket.
    pub fn request_frame(&self) -> String {
        match self {
            CallScope::Room(id) => serde_json::json!({ "type": "call_credentials", "room": id }),
            CallScope::Call(key) => serde_json::json!({ "type": "call_credentials", "call": key }),
        }
        .to_string()
    }

    /// A short name for logs (a key is thousands of characters).
    pub fn label(&self) -> String {
        match self {
            CallScope::Room(id) => format!("room {id}"),
            CallScope::Call(key) => format!("call with {}", &key[..key.len().min(12)]),
        }
    }
}

/// The server's answer, read: where its forwarder listens and the credentials for one scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallCredentials {
    pub scope: CallScope,
    /// The forwarder's `host:port`, from the reply's `turn:` address.
    pub turn_server: String,
    pub username: String,
    pub credential: String,
    /// How long the server says they are good for, in seconds (0 when it did not say).
    pub ttl_secs: u64,
}

/// A `call_credentials` reply, read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reply {
    /// Credentials the client can use.
    Granted(CallCredentials),
    /// No usable credentials for this scope: refused (`"refused":true`), or missing a UDP
    /// `turn:` address, a username or a credential.
    Refused(CallScope),
}

/// Read a `call_credentials` reply. `None` for anything that is not one or names no single
/// scope. Only plain UDP TURN is taken: that is all the forwarder speaks and all the client in
/// webrtc.rs does, so a reply offering only TCP or TLS gives the client nothing and counts as
/// a refusal.
pub fn parse_reply(frame: &Value) -> Option<Reply> {
    if frame.get("type").and_then(Value::as_str) != Some("call_credentials") {
        return None;
    }
    let text = |k: &str| frame.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string);
    let scope = match (text("room"), text("call")) {
        (Some(room), None) => CallScope::Room(room),
        (None, Some(key)) => CallScope::Call(key),
        _ => return None,
    };
    let refused = frame.get("refused").and_then(Value::as_bool) == Some(true);
    let turn_server = frame
        .get("urls")
        .and_then(Value::as_array)
        .and_then(|urls| urls.iter().filter_map(Value::as_str).find_map(udp_turn_server));
    match (refused, turn_server, text("username"), text("credential")) {
        (false, Some(turn_server), Some(username), Some(credential)) => Some(Reply::Granted(CallCredentials {
            scope,
            turn_server,
            username,
            credential,
            ttl_secs: frame.get("ttl").and_then(Value::as_u64).unwrap_or(0),
        })),
        _ => Some(Reply::Refused(scope)),
    }
}

/// `turn:host:port?transport=udp` (or with no transport, which means UDP) to `host:port`.
/// A `turns:` address, or one for TCP, is not one the client can use.
fn udp_turn_server(url: &str) -> Option<String> {
    let rest = url.strip_prefix("turn:")?;
    let (addr, query) = match rest.split_once('?') {
        Some((a, q)) => (a, Some(q)),
        None => (rest, None),
    };
    if let Some(q) = query {
        let transport = q.split('&').find_map(|kv| kv.strip_prefix("transport="));
        if transport.is_some_and(|t| !t.eq_ignore_ascii_case("udp")) {
            return None;
        }
    }
    with_port(addr, DEFAULT_TURN_PORT)
}

/// `host` or `host:port` (an IPv6 literal in brackets) to `host:port`, with `default` when no
/// port was given. `None` for an empty host.
pub(crate) fn with_port(addr: &str, default: u16) -> Option<String> {
    if addr.is_empty() {
        return None;
    }
    let has_port = if let Some(end) = addr.rfind(']') {
        addr[end..].contains(':')
    } else {
        addr.contains(':')
    };
    if has_port {
        Some(addr.to_string())
    } else {
        Some(format!("{addr}:{default}"))
    }
}

/// Where one scope's connection through the server stands, as the call UI shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayStatus {
    /// The request went out; no answer yet.
    Asking,
    /// Credentials came and went to the WebRTC manager, which is reaching the forwarder.
    Connecting,
    /// The forwarder gave this device its relayed address: calls can connect.
    Ready,
    /// No credentials came, or the forwarder never answered: [`NOT_SET_UP_LINE`].
    Unavailable,
    /// The forwarder stopped answering in the middle of the call: [`LOST_LINE`].
    Lost,
}

/// One thing the app must do, from [`CallRelayUi::update`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Send this request frame on the chat socket.
    Ask(String),
    /// The call or room ended: the WebRTC manager lets its allocation go.
    End(CallScope),
    /// The awaited credentials came: hand them to the WebRTC manager.
    Use(CallCredentials),
    /// The credentials were refused, or none came within [`CREDENTIALS_WAIT`]: tell the WebRTC
    /// manager (it drops the offers it was holding) and say [`NOT_SET_UP_LINE`].
    GaveUp(CallScope),
}

/// The app's side of asking for credentials: which scope it is in, and how far that got.
/// One scope at a time: a call and a voice room cannot both be open on the desktop app (a ring
/// is turned away inside a voice room, and Call is off there).
#[derive(Debug, Default)]
pub struct CallRelayUi {
    scope: Option<CallScope>,
    status: Option<RelayStatus>,
    asked_at: Option<Instant>,
    /// The person closed the room's notice; it stays closed for this scope.
    dismissed: bool,
}

impl CallRelayUi {
    /// The scope being asked for or connected.
    pub fn scope(&self) -> Option<&CallScope> {
        self.scope.as_ref()
    }

    /// How far `scope` got, if it is the one in hand.
    pub fn status_of(&self, scope: &CallScope) -> Option<RelayStatus> {
        if self.scope.as_ref() == Some(scope) {
            self.status
        } else {
            None
        }
    }

    /// The line the call UI shows for `scope`, when there is one to show.
    pub fn line_for(&self, scope: &CallScope) -> Option<&'static str> {
        match self.status_of(scope)? {
            RelayStatus::Unavailable => Some(NOT_SET_UP_LINE),
            RelayStatus::Lost => Some(LOST_LINE),
            _ => None,
        }
    }

    /// Whether the person closed the notice for the scope in hand.
    pub fn dismissed(&self) -> bool {
        self.dismissed
    }

    /// Close the notice for the scope in hand.
    pub fn dismiss(&mut self) {
        self.dismissed = true;
    }

    /// Called every frame with the scope the app is in now (`None` when in no call or room).
    /// A new scope is asked for at once; the old one is ended; an unanswered request gives up
    /// after [`CREDENTIALS_WAIT`].
    pub fn update(&mut self, wanted: Option<CallScope>, now: Instant) -> Vec<Step> {
        let mut steps = Vec::new();
        if wanted != self.scope {
            if let Some(old) = self.scope.take() {
                steps.push(Step::End(old));
            }
            self.status = None;
            self.asked_at = None;
            self.dismissed = false;
            if let Some(new) = wanted {
                steps.push(Step::Ask(new.request_frame()));
                self.scope = Some(new);
                self.status = Some(RelayStatus::Asking);
                self.asked_at = Some(now);
            }
        } else if self.status == Some(RelayStatus::Asking)
            && self.asked_at.is_some_and(|t| now.duration_since(t) >= CREDENTIALS_WAIT)
        {
            self.status = Some(RelayStatus::Unavailable);
            if let Some(scope) = &self.scope {
                steps.push(Step::GaveUp(scope.clone()));
            }
        }
        steps
    }

    /// A reply arrived. For the scope asked for and still awaited: credentials become
    /// [`Step::Use`], a refusal [`Step::GaveUp`] at once. A reply for another scope, or one that
    /// came after the app gave up, is dropped: by then the person was told the call cannot
    /// connect, and joining again asks again.
    pub fn on_reply(&mut self, reply: Reply) -> Option<Step> {
        let scope = match &reply {
            Reply::Granted(creds) => &creds.scope,
            Reply::Refused(scope) => scope,
        };
        if self.scope.as_ref() != Some(scope) || self.status != Some(RelayStatus::Asking) {
            return None;
        }
        match reply {
            Reply::Granted(creds) => {
                self.status = Some(RelayStatus::Connecting);
                Some(Step::Use(creds))
            }
            Reply::Refused(scope) => {
                self.status = Some(RelayStatus::Unavailable);
                Some(Step::GaveUp(scope))
            }
        }
    }

    /// The WebRTC manager reached the forwarder for `scope`.
    pub fn on_ready(&mut self, scope: &CallScope) {
        if self.scope.as_ref() == Some(scope) {
            self.status = Some(RelayStatus::Ready);
        }
    }

    /// The WebRTC manager could not reach the forwarder for `scope` (`lost`: it stopped
    /// answering after it had). Returns true when this is news, so the line is said once.
    pub fn on_unavailable(&mut self, scope: &CallScope, lost: bool) -> bool {
        if self.scope.as_ref() != Some(scope) {
            return false;
        }
        let next = if lost { RelayStatus::Lost } else { RelayStatus::Unavailable };
        if matches!(self.status, Some(RelayStatus::Unavailable) | Some(RelayStatus::Lost)) {
            return false;
        }
        self.status = Some(next);
        true
    }

    /// Forget everything (the chat socket and the WebRTC manager riding it went away).
    pub fn reset(&mut self) {
        *self = CallRelayUi::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn reply(scope: (&str, &str)) -> Value {
        json!({
            "type": "call_credentials",
            scope.0: scope.1,
            "urls": ["turn:relay.example:3478?transport=udp", "stun:relay.example:3478"],
            "username": "1760000000:abcd",
            "credential": "c2VjcmV0",
            "ttl": 3600,
        })
    }

    /// The request is exactly the spec's (10f) for each kind of scope. Seen red 2026-10-09
    /// with the call request built under the key "peer": the second assertion.
    #[test]
    fn the_request_names_the_room_or_the_other_person() {
        let room: Value = serde_json::from_str(&CallScope::Room("12".into()).request_frame()).unwrap();
        assert_eq!(room, json!({ "type": "call_credentials", "room": "12" }));
        let call: Value = serde_json::from_str(&CallScope::Call("ab".into()).request_frame()).unwrap();
        assert_eq!(call, json!({ "type": "call_credentials", "call": "ab" }));
        assert_eq!(CallScope::for_voice_peer(crate::net::webrtc::CALL_ROOM_ID, "ab"), CallScope::Call("ab".into()));
        assert_eq!(CallScope::for_voice_peer("12", "ab"), CallScope::Room("12".into()));
    }

    fn granted(frame: &Value) -> CallCredentials {
        match parse_reply(frame) {
            Some(Reply::Granted(c)) => c,
            other => panic!("expected credentials, got {other:?}"),
        }
    }

    /// The reply is read into the forwarder's address and the credentials; a refusal, and a
    /// reply that gives the client nothing it can use, are refusals for their scope; anything
    /// else is not a reply. Seen red 2026-10-09 with `udp_turn_server` taking any transport:
    /// "a TCP-only reply gives the client nothing it can use".
    #[test]
    fn a_reply_is_read_and_an_unusable_one_is_a_refusal() {
        let creds = granted(&reply(("room", "12")));
        assert_eq!(creds.scope, CallScope::Room("12".into()));
        assert_eq!(creds.turn_server, "relay.example:3478");
        assert_eq!((creds.username.as_str(), creds.credential.as_str(), creds.ttl_secs), ("1760000000:abcd", "c2VjcmV0", 3600));
        assert_eq!(granted(&reply(("call", "ab"))).scope, CallScope::Call("ab".into()));

        let mut no_port = reply(("room", "12"));
        no_port["urls"] = json!(["turn:10.0.0.5"]);
        assert_eq!(granted(&no_port).turn_server, "10.0.0.5:3478", "the default TURN port");

        let refused_12 = Some(Reply::Refused(CallScope::Room("12".into())));
        // The relay's refusal, exactly as it sends it.
        assert_eq!(parse_reply(&json!({ "type": "call_credentials", "room": "12", "refused": true })), refused_12);
        assert_eq!(
            parse_reply(&json!({ "type": "call_credentials", "call": "ab", "refused": true })),
            Some(Reply::Refused(CallScope::Call("ab".into())))
        );
        let mut marked = reply(("room", "12"));
        marked["refused"] = json!(true);
        assert_eq!(parse_reply(&marked), refused_12, "refused wins over whatever else came");
        let mut tcp = reply(("room", "12"));
        tcp["urls"] = json!(["turn:relay.example:3478?transport=tcp", "turns:relay.example:5349"]);
        assert_eq!(parse_reply(&tcp), refused_12, "a TCP-only reply gives the client nothing it can use");
        let mut stun_only = reply(("room", "12"));
        stun_only["urls"] = json!(["stun:relay.example:3478"]);
        assert_eq!(parse_reply(&stun_only), refused_12, "no forwarder, no credentials");
        let mut no_user = reply(("room", "12"));
        no_user["username"] = json!("");
        assert_eq!(parse_reply(&no_user), refused_12);

        let mut both = reply(("room", "12"));
        both["call"] = json!("ab");
        assert_eq!(parse_reply(&both), None, "a room and a call at once names no scope");
        assert_eq!(parse_reply(&json!({ "type": "reports" })), None);
    }

    /// One scope is asked for once, gives up after the wait, and the right replies go through.
    /// Seen red 2026-10-09 with the give-up dropped from `update`: "no answer in time".
    #[test]
    fn asks_once_gives_up_after_the_wait_and_takes_only_the_awaited_reply() {
        let t0 = Instant::now();
        let room = CallScope::Room("12".into());
        let mut ui = CallRelayUi::default();
        assert_eq!(ui.update(Some(room.clone()), t0), vec![Step::Ask(room.request_frame())]);
        assert!(ui.update(Some(room.clone()), t0 + Duration::from_secs(1)).is_empty(), "asked once");
        assert_eq!(ui.status_of(&room), Some(RelayStatus::Asking));

        // A reply for another room is not ours.
        assert_eq!(ui.on_reply(parse_reply(&reply(("room", "99"))).unwrap()), None);
        // Ours is.
        let ours = granted(&reply(("room", "12")));
        assert_eq!(ui.on_reply(Reply::Granted(ours.clone())), Some(Step::Use(ours)));
        assert_eq!(ui.status_of(&room), Some(RelayStatus::Connecting));
        assert!(ui.update(Some(room.clone()), t0 + CREDENTIALS_WAIT * 2).is_empty(), "no give-up once it came");
        ui.on_ready(&room);
        assert_eq!(ui.status_of(&room), Some(RelayStatus::Ready));
        assert_eq!(ui.line_for(&room), None);

        // Leaving ends it; a call starts fresh and gives up when nothing comes.
        let call = CallScope::Call("ab".into());
        let t1 = t0 + Duration::from_secs(60);
        assert_eq!(ui.update(Some(call.clone()), t1), vec![Step::End(room.clone()), Step::Ask(call.request_frame())]);
        assert!(ui.update(Some(call.clone()), t1 + CREDENTIALS_WAIT - Duration::from_millis(1)).is_empty());
        assert_eq!(
            ui.update(Some(call.clone()), t1 + CREDENTIALS_WAIT),
            vec![Step::GaveUp(call.clone())],
            "no answer in time"
        );
        assert_eq!(ui.line_for(&call), Some(NOT_SET_UP_LINE));
        assert!(ui.update(Some(call.clone()), t1 + CREDENTIALS_WAIT * 3).is_empty(), "said once");
        let late = parse_reply(&reply(("call", "ab"))).unwrap();
        assert_eq!(ui.on_reply(late), None, "a reply after giving up is dropped");
        assert!(!ui.on_unavailable(&call, false), "already said");
        assert_eq!(ui.update(None, t1 + CREDENTIALS_WAIT * 4), vec![Step::End(call)]);
    }

    /// A refusal is said at once, without waiting out the backstop. Seen red 2026-10-09 with
    /// `on_reply` ignoring refusals (the old reading, which kept only usable replies): left
    /// None, right Some(GaveUp(..)).
    #[test]
    fn a_refusal_gives_up_at_once() {
        let t0 = Instant::now();
        let call = CallScope::Call("ab".into());
        let mut ui = CallRelayUi::default();
        ui.update(Some(call.clone()), t0);
        let refusal = parse_reply(&json!({ "type": "call_credentials", "call": "ab", "refused": true })).unwrap();
        assert_eq!(ui.on_reply(refusal), Some(Step::GaveUp(call.clone())));
        assert_eq!(ui.line_for(&call), Some(NOT_SET_UP_LINE));
        assert!(ui.update(Some(call.clone()), t0 + CREDENTIALS_WAIT).is_empty(), "and the backstop says nothing more");
    }

    /// The forwarder failing is said once, and a failure mid-call has its own line.
    #[test]
    fn the_forwarder_failing_is_said_once() {
        let t0 = Instant::now();
        let call = CallScope::Call("ab".into());
        let mut ui = CallRelayUi::default();
        ui.update(Some(call.clone()), t0);
        ui.on_reply(parse_reply(&reply(("call", "ab"))).unwrap());
        assert!(!ui.on_unavailable(&CallScope::Room("12".into()), false), "not the scope in hand");
        assert!(ui.on_unavailable(&call, false));
        assert!(!ui.on_unavailable(&call, false), "once");
        assert_eq!(ui.line_for(&call), Some(NOT_SET_UP_LINE));

        let mut mid_call = CallRelayUi::default();
        mid_call.update(Some(call.clone()), t0);
        mid_call.on_reply(parse_reply(&reply(("call", "ab"))).unwrap());
        mid_call.on_ready(&call);
        assert!(mid_call.on_unavailable(&call, true));
        assert_eq!(mid_call.line_for(&call), Some(LOST_LINE));
    }

    /// No Google STUN host anywhere in the desktop app's source, the relay's or the web
    /// client's (step E, 10f: "Every Google STUN entry is removed"). A STUN request shows the
    /// company that answers it this device's address, so the app asks only its own server.
    ///
    /// `PENDING_OTHER_HALVES` names files the parallel halves of step E change, which this
    /// branch may not edit (the relay's `/api/turn-credentials` list, the web client's
    /// `rtcConfig`). Each entry must still contain a Google host: once its half lands the test
    /// fails with "remove it from PENDING_OTHER_HALVES", so the exception cannot outlive its
    /// reason and hide a later regression in that file.
    ///
    /// Seen red 2026-10-09 with `STUN_SERVERS` and its two Google hosts put back in
    /// src/net/webrtc.rs: "src/net/webrtc.rs:...: a Google STUN host".
    #[test]
    fn no_google_stun_host_remains_in_either_client() {
        // Built from pieces so this file does not contain what it looks for.
        let needles = [["l.goo", "gle.com"].concat(), [":193", "02"].concat()];
        // Emptied 2026-10-10 when the relay and web halves of step E merged and
        // app/web was regenerated: no file may name a Google STUN host any more.
        const PENDING_OTHER_HALVES: &[&str] = &[];
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut files = Vec::new();
        for dir in ["src", "web", "app/web"] {
            collect(&root.join(dir), &mut files);
        }
        assert!(files.len() > 100, "the scan found the source tree ({} files)", files.len());

        let mut hits = Vec::new();
        let mut pending_seen = Vec::new();
        for path in &files {
            let rel = path.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
            let Ok(text) = std::fs::read_to_string(path) else { continue };
            for (n, line) in text.lines().enumerate() {
                if needles.iter().any(|needle| line.contains(needle.as_str())) {
                    if PENDING_OTHER_HALVES.contains(&rel.as_str()) {
                        pending_seen.push(rel.clone());
                    } else {
                        hits.push(format!("{rel}:{}: a Google STUN host: {}", n + 1, line.trim()));
                    }
                }
            }
        }
        assert!(hits.is_empty(), "\n{}\n", hits.join("\n"));
        for pending in PENDING_OTHER_HALVES {
            let exists = root.join(pending).exists();
            assert!(
                !exists || pending_seen.iter().any(|p| p == pending),
                "{pending} no longer names a Google STUN host: remove it from PENDING_OTHER_HALVES"
            );
        }
    }

    /// Every `.rs`, `.js` and `.html` file under `dir`.
    fn collect(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect(&path, out);
            } else if matches!(path.extension().and_then(|e| e.to_str()), Some("rs" | "js" | "html")) {
                out.push(path);
            }
        }
    }
}
