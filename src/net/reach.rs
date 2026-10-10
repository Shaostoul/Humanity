//! "Who can reach me" (step B, docs/design/blocking-and-safe-mode.md section 10c): the native
//! client's model, with no socket and no GUI, so every rule here has a unit test.
//!
//! The relay enforces who may reach a person; this file is what the client needs to agree with
//! it: the three kinds of contact the relay checks today (message, call, trade), the five
//! audiences each can be set to, the safe defaults, the `reach_set` / `reach_settings` frames,
//! the "show as a request" rule the client applies to any DM that its own settings would have
//! refused (so a modified sender gains nothing by skipping the contact-request flag), and the
//! contact request itself: an ordinary signed sealed DM whose text is a control marker carrying
//! the requester's name and the pass they give the recipient (10c, amended in review).
//!
//! The kinds and audiences are protocol words fixed by the relay (10c, "exact names"), signed
//! into nothing and compared by string, so they live here as a closed set rather than in a data
//! file: adding one is a protocol change on three programs, never a content edit.

use serde::{Deserialize, Serialize};

use super::dm_store::SentPass;

/// A kind of contact the relay checks against the recipient's settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReachKind {
    /// `dm_put`: a sealed direct message (and every control message, which rides one).
    Message,
    /// `voice_call` rings and the call's signals.
    Call,
    /// `trade_request`.
    Trade,
}

impl ReachKind {
    /// In the order the Safety page lists them.
    pub const ALL: [ReachKind; 3] = [ReachKind::Message, ReachKind::Call, ReachKind::Trade];

    /// The word the relay uses (`reach_set`, `reach_settings`, `reach_refused`), which is also
    /// the word a friendship pass's `may` uses for the same kind.
    pub fn wire(self) -> &'static str {
        match self {
            ReachKind::Message => "message",
            ReachKind::Call => "call",
            ReachKind::Trade => "trade",
        }
    }

    pub fn from_wire(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.wire() == word)
    }

    /// The row's name on the Safety page.
    pub fn label(self) -> &'static str {
        match self {
            ReachKind::Message => "Messages",
            ReachKind::Call => "Calls",
            ReachKind::Trade => "Trades",
        }
    }
}

/// Who gets through for one kind of contact, narrowest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Audience {
    /// No one.
    Nobody,
    /// Holders of a valid pass from me whose `may` includes this kind.
    Chosen,
    /// Holders of any valid pass from me.
    Friends,
    /// Friends, plus people who share a P2P group with me.
    Groups,
    /// Everyone (strangers still spend the relay's daily knock budget).
    Anyone,
}

impl Audience {
    /// Narrowest first: the order the Safety page offers them in.
    pub const ALL: [Audience; 5] = [Audience::Nobody, Audience::Chosen, Audience::Friends, Audience::Groups, Audience::Anyone];

    pub fn wire(self) -> &'static str {
        match self {
            Audience::Nobody => "nobody",
            Audience::Chosen => "chosen",
            Audience::Friends => "friends",
            Audience::Groups => "groups",
            Audience::Anyone => "anyone",
        }
    }

    pub fn from_wire(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.wire() == word)
    }

    /// The plain words 10c gives for each audience.
    pub fn label(self) -> &'static str {
        match self {
            Audience::Nobody => "Nobody",
            Audience::Chosen => "People I choose",
            Audience::Friends => "Friends",
            Audience::Groups => "Friends and people in my groups",
            Audience::Anyone => "Anyone",
        }
    }

    /// The one line under a row saying what this choice means for that kind of contact.
    pub fn meaning(self, kind: ReachKind) -> &'static str {
        match (kind, self) {
            (ReachKind::Message, Audience::Nobody) => {
                "No one can message you here, friends included, and contact requests are turned off."
            }
            (ReachKind::Message, Audience::Chosen) => {
                "Only friends whose pass includes messages (every friend's pass does). Anyone else can send a contact request with just their name."
            }
            (ReachKind::Message, Audience::Friends) => {
                "Your friends can message you. Anyone else can send a contact request with just their name."
            }
            (ReachKind::Message, Audience::Groups) => {
                "Friends and people who share a group with you can message you. Anyone else can send a contact request."
            }
            (ReachKind::Message, Audience::Anyone) => {
                "Anyone on this server can message you. People who are not your friends have a small daily limit."
            }
            (ReachKind::Call, Audience::Nobody) => "No one can call you, friends included.",
            (ReachKind::Call, Audience::Chosen) => {
                "Only the people you tick in the list below can call you. Other calls never ring."
            }
            (ReachKind::Call, Audience::Friends) => "Any of your friends can call you.",
            (ReachKind::Call, Audience::Groups) => "Friends and people who share a group with you can call you.",
            (ReachKind::Call, Audience::Anyone) => "Anyone on this server can call you.",
            (ReachKind::Trade, Audience::Nobody) => "No one can send you trade requests.",
            (ReachKind::Trade, Audience::Chosen) => {
                "Only friends whose pass includes trades (every friend's pass does) can send you trade requests."
            }
            (ReachKind::Trade, Audience::Friends) => "Your friends can send you trade requests.",
            (ReachKind::Trade, Audience::Groups) => {
                "Friends and people who share a group with you can send you trade requests."
            }
            (ReachKind::Trade, Audience::Anyone) => {
                "Anyone on this server can send you trade requests. People who are not your friends have a small daily limit."
            }
        }
    }
}

/// One person's audiences on one server: what the relay last said (`reach_settings`), or the
/// safe defaults when it has said nothing yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReachSettings {
    pub message: Audience,
    pub call: Audience,
    pub trade: Audience,
}

impl Default for ReachSettings {
    /// The safe defaults (10c): messages and trades from friends, calls only from people the
    /// person chooses, an empty list until they add someone.
    fn default() -> Self {
        Self { message: Audience::Friends, call: Audience::Chosen, trade: Audience::Friends }
    }
}

impl ReachSettings {
    pub fn get(&self, kind: ReachKind) -> Audience {
        match kind {
            ReachKind::Message => self.message,
            ReachKind::Call => self.call,
            ReachKind::Trade => self.trade,
        }
    }

    pub fn set(&mut self, kind: ReachKind, audience: Audience) {
        match kind {
            ReachKind::Message => self.message = audience,
            ReachKind::Call => self.call = audience,
            ReachKind::Trade => self.trade = audience,
        }
    }

    /// Read the relay's `{"type":"reach_settings","settings":{...}}`. The relay fills in all
    /// three kinds; a frame missing one, or carrying a word this client does not know, is not
    /// read at all (None), so the page never shows a guess as though the server had said it.
    pub fn from_frame(frame: &serde_json::Value) -> Option<Self> {
        let settings = frame.get("settings")?;
        let read = |kind: ReachKind| settings.get(kind.wire()).and_then(|v| v.as_str()).and_then(Audience::from_wire);
        Some(Self { message: read(ReachKind::Message)?, call: read(ReachKind::Call)?, trade: read(ReachKind::Trade)? })
    }

    /// The `reach_set` frame asking the relay to make `changes` (any subset of kinds).
    pub fn reach_set_frame(changes: &[(ReachKind, Audience)]) -> serde_json::Value {
        let mut settings = serde_json::Map::new();
        for (kind, audience) in changes {
            settings.insert(kind.wire().to_string(), serde_json::Value::String(audience.wire().to_string()));
        }
        serde_json::json!({ "type": "reach_set", "settings": settings })
    }
}

/// What this client knows about someone, for deciding whether its own settings let them reach
/// it: the same facts the relay checks (10c's audience table), seen from this side.
#[derive(Debug, Clone, Copy)]
pub struct Relation<'a> {
    /// The person is us (the self-copy of our own message): always let through.
    pub is_me: bool,
    /// The passes we gave them that still stand (none: they are not a friend at the relay).
    pub passes: &'a [SentPass],
    /// They share a P2P group with us.
    pub shares_group: bool,
}

/// Would `audience` let this person reach us with `kind`? The relay's rule, applied by the
/// client to what actually arrives.
pub fn admits(audience: Audience, kind: ReachKind, rel: &Relation) -> bool {
    if rel.is_me {
        return true;
    }
    let holds_pass = !rel.passes.is_empty();
    let pass_allows = rel.passes.iter().any(|p| p.may.split(',').any(|k| k == kind.wire()));
    match audience {
        Audience::Nobody => false,
        Audience::Chosen => pass_allows,
        Audience::Friends => holds_pass,
        Audience::Groups => holds_pass || rel.shares_group,
        Audience::Anyone => true,
    }
}

/// THE "show as a request" rule (10c): a DM from someone our own message setting would refuse
/// is shown as a contact request, the sender's name only and never its text, whatever path it
/// arrived by. A relay that let it through (an older one, a misconfigured one) or a sender that
/// skipped the contact-request flag gains nothing.
pub fn shows_as_request(settings: &ReachSettings, rel: &Relation) -> bool {
    !admits(settings.message, ReachKind::Message, rel)
}

// ── Contact requests ────────────────────────────────────────────────────────

/// The control marker a contact request's DM text starts with (10c, amended in review). The JSON
/// `{"name":...,"pass":...}` follows it directly. Must match the web client.
pub const CONTACT_REQUEST_MARKER: &str = "[[hum:contact-request:v1]]";

/// The text of a contact request DM: the marker, then `{"name":"<my registered name>","pass":
/// "<my v2 pass for the recipient, as its JSON string>"}`. The DM around it is an ordinary signed
/// sealed v2 DM, so the recipient knows who sent it from the signature, not from `name`. The pass
/// is the requester's consent to hear back: the accepter's reply carries it past the requester's
/// own gate, which is what lets two people on the safe defaults become friends at all.
pub fn contact_request_text(name: &str, pass: &str) -> String {
    format!("{CONTACT_REQUEST_MARKER}{}", serde_json::json!({ "name": name, "pass": pass }))
}

/// Read a contact request's text: the claimed name and the pass, or None when the text is not a
/// contact request (or its JSON is not the right shape). The pass is NOT checked here; that needs
/// the server, the signed sender and us (engine/reach.rs `contact_request_in`).
pub fn parse_contact_request_text(text: &str) -> Option<(String, String)> {
    let json = text.strip_prefix(CONTACT_REQUEST_MARKER)?;
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let pass = v.get("pass").and_then(|x| x.as_str())?.to_string();
    Some((name, pass))
}

/// One entry of the Requests list: a contact request (whose pass checked out), or a DM our own
/// settings would have refused (10c). Kept in the encrypted DM store, per server, until Accept or
/// Ignore. Who it is comes from the DM's signature; its name is the one the member list holds for
/// that key, never the name the request claimed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContactRequest {
    /// The sender's identity key (checked by the DM's signature). Also the entry's id.
    pub key: String,
    /// When it was written (the signed DM time, ms).
    #[serde(default)]
    pub ts: u64,
    /// The pass the requester gave us (empty for a refused DM, which carries none). Kept here,
    /// not with the passes we hold, until Accept: Ignore then leaves nothing behind.
    #[serde(default)]
    pub pass: String,
}

/// The kinds a pass to a friend should allow: the defaults two new friends get (step A), plus
/// `call` when the person ticked them in the Safety page's "People who may call me" list.
pub fn intended_may(may_call: bool) -> Vec<&'static str> {
    let mut words: Vec<&'static str> = crate::relay::core::pq_crypto::FRIEND_PASS_DEFAULT_MAY.to_vec();
    if may_call {
        words.push(ReachKind::Call.wire());
    }
    words
}

/// The canonical `may` (sorted, comma-joined) of [`intended_may`], as passes carry it.
pub fn intended_may_wire(may_call: bool) -> String {
    crate::relay::core::pq_crypto::FriendMay::from_words(intended_may(may_call)).map(|m| m.wire()).unwrap_or_default()
}

/// The Safety page's state that lives only in this run: what we asked the relay for and are
/// waiting to hear back about, and which people refused our messages (for the notice in their
/// conversation). Reset with the other per-server transients on a server switch.
#[derive(Debug, Default)]
pub struct ReachUi {
    /// A `reach_set` sent and not yet answered by a `reach_settings`, and when.
    pub asked: Option<(ReachSettings, std::time::Instant)>,
    /// People whose gate refused a message from us (`reach_refused`), by key: false = refused,
    /// true = we have since sent them a contact request.
    pub refused: std::collections::HashMap<String, bool>,
    /// A one-line result to show on the Safety page or in the notice (e.g. why a request could
    /// not be sent).
    pub status: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pass(may: &str) -> SentPass {
        SentPass { serial: "00".repeat(16), may: may.into() }
    }

    /// The settings model: safe defaults with nothing said; the relay's frame read whole or not
    /// at all; `reach_set` carries exactly the words asked for; every audience round-trips its
    /// wire word and has a label and a meaning for every kind.
    /// Seen red 2026-10-09 with `Default` giving calls `Friends`: "safe default for calls is
    /// People I choose", left `Friends`.
    #[test]
    fn settings_model_defaults_frames_and_words() {
        let d = ReachSettings::default();
        assert_eq!(d.get(ReachKind::Message), Audience::Friends, "safe default for messages is Friends");
        assert_eq!(d.get(ReachKind::Call), Audience::Chosen, "safe default for calls is People I choose");
        assert_eq!(d.get(ReachKind::Trade), Audience::Friends, "safe default for trades is Friends");

        let frame = serde_json::json!({ "type": "reach_settings", "settings": { "message": "anyone", "call": "nobody", "trade": "groups" } });
        let s = ReachSettings::from_frame(&frame).expect("a full frame is read");
        assert_eq!((s.message, s.call, s.trade), (Audience::Anyone, Audience::Nobody, Audience::Groups));
        let missing = serde_json::json!({ "type": "reach_settings", "settings": { "message": "anyone", "call": "nobody" } });
        assert_eq!(ReachSettings::from_frame(&missing), None, "a frame missing a kind is not read");
        let unknown = serde_json::json!({ "type": "reach_settings", "settings": { "message": "everyone", "call": "nobody", "trade": "friends" } });
        assert_eq!(ReachSettings::from_frame(&unknown), None, "an unknown audience word is not read");

        let set = ReachSettings::reach_set_frame(&[(ReachKind::Call, Audience::Friends)]);
        assert_eq!(set, serde_json::json!({ "type": "reach_set", "settings": { "call": "friends" } }), "reach_set carries only what was asked");

        let mut s = ReachSettings::default();
        s.set(ReachKind::Trade, Audience::Anyone);
        assert_eq!(s.trade, Audience::Anyone);
        for a in Audience::ALL {
            assert_eq!(Audience::from_wire(a.wire()), Some(a));
            assert!(!a.label().is_empty());
            for k in ReachKind::ALL {
                assert!(!a.meaning(k).is_empty());
                assert_eq!(ReachKind::from_wire(k.wire()), Some(k));
            }
        }
        assert_eq!(Audience::ALL.map(|a| a.label()), ["Nobody", "People I choose", "Friends", "Friends and people in my groups", "Anyone"], "the plain words of 10c");
        assert_eq!(intended_may_wire(false), "invite,message,trade,voice_message");
        assert_eq!(intended_may_wire(true), "call,invite,message,trade,voice_message");
    }

    /// The "show as a request" rule, under each audience: a friend (any standing pass), a chosen
    /// friend (a pass whose may names the kind), a group mate with no pass, a stranger, and us.
    /// Seen red 2026-10-09 with `Audience::Groups` checking only `shares_group` (friends left
    /// out): "a friend gets through Friends and people in my groups", shown as a request.
    #[test]
    fn show_as_request_follows_the_message_audience() {
        let friend_passes = [pass("invite,message,trade,voice_message")];
        let call_only = [pass("call")];
        let friend = Relation { is_me: false, passes: &friend_passes, shares_group: false };
        let pass_without_messages = Relation { is_me: false, passes: &call_only, shares_group: false };
        let group_mate = Relation { is_me: false, passes: &[], shares_group: true };
        let stranger = Relation { is_me: false, passes: &[], shares_group: false };
        let me = Relation { is_me: true, passes: &[], shares_group: false };
        let with = |message: Audience| ReachSettings { message, ..ReachSettings::default() };

        let defaults = ReachSettings::default();
        assert!(!shows_as_request(&defaults, &friend), "a friend's DM is a DM under the defaults");
        assert!(shows_as_request(&defaults, &stranger), "a stranger's DM is a request under the defaults");
        assert!(shows_as_request(&defaults, &group_mate), "a group mate is not a friend");
        assert!(!shows_as_request(&defaults, &me), "our own self-copy is never a request");

        assert!(shows_as_request(&with(Audience::Nobody), &friend), "Nobody refuses friends too");
        assert!(!shows_as_request(&with(Audience::Nobody), &me));
        assert!(!shows_as_request(&with(Audience::Chosen), &friend), "a pass naming messages gets through People I choose");
        assert!(shows_as_request(&with(Audience::Chosen), &pass_without_messages), "a pass that does not name messages does not");
        assert!(!shows_as_request(&with(Audience::Friends), &pass_without_messages), "but any pass gets through Friends");
        assert!(!shows_as_request(&with(Audience::Groups), &friend), "a friend gets through Friends and people in my groups");
        assert!(!shows_as_request(&with(Audience::Groups), &group_mate), "so does a group mate");
        assert!(shows_as_request(&with(Audience::Groups), &stranger));
        assert!(!shows_as_request(&with(Audience::Anyone), &stranger), "Anyone lets a stranger's DM through");

        // The call row is judged by the same rule with its own kind.
        assert!(admits(Audience::Chosen, ReachKind::Call, &pass_without_messages), "a pass naming calls lets them call");
        assert!(!admits(Audience::Chosen, ReachKind::Call, &friend), "a default pass does not");
    }

    /// A contact request's text: the marker, then the name and the pass as JSON, read back the
    /// same, quotes and all; text without the marker, or with no pass, is not a request.
    /// Seen red 2026-10-09 with the parser reading the JSON without stripping the marker first:
    /// "it reads back", None.
    #[test]
    fn contact_request_text_round_trip() {
        let pass = r#"{"v":2,"serial":"00112233445566778899aabbccddeeff","may":"invite,message,trade,voice_message","sig":"QUJD"}"#;
        let text = contact_request_text("Ann", pass);
        assert!(text.starts_with("[[hum:contact-request:v1]]{"), "the marker, then the JSON");
        assert_eq!(parse_contact_request_text(&text), Some(("Ann".to_string(), pass.to_string())), "it reads back");
        assert_eq!(parse_contact_request_text("hello"), None, "an ordinary DM is not a request");
        assert_eq!(parse_contact_request_text("[[hum:contact-request:v1]]{\"name\":\"Ann\"}"), None, "no pass, no request");
        assert_eq!(parse_contact_request_text(&text[1..]), None, "the marker must lead");
    }
}
