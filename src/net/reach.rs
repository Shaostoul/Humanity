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

    /// The tick's name beside each friend in "People I choose" (10c-ii): one thing they may do.
    pub fn tick_label(self) -> &'static str {
        match self {
            ReachKind::Message => "Message",
            ReachKind::Call => "Call",
            ReachKind::Trade => "Trade",
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
            // The three "People I choose" lines name the list the ticks are on (10c-ii), word for
            // word as the web client says them (web/shared/reach.js `reachExplain`). A contact
            // request still gets through this one (the relay refuses one only under "Nobody").
            (ReachKind::Message, Audience::Chosen) => {
                "Only the friends you tick for Message on your \"People I choose\" list can message you. Anyone else can send a contact request that shows you only their name."
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
                "Only the friends you tick for Call on your \"People I choose\" list can call you."
            }
            (ReachKind::Call, Audience::Friends) => "Any of your friends can call you.",
            (ReachKind::Call, Audience::Groups) => "Friends and people who share a group with you can call you.",
            (ReachKind::Call, Audience::Anyone) => "Anyone on this server can call you.",
            (ReachKind::Trade, Audience::Nobody) => "No one can send you trade requests.",
            (ReachKind::Trade, Audience::Chosen) => {
                "Only the friends you tick for Trade on your \"People I choose\" list can send you trade requests."
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

// ── "People I choose": a tick per friend (10c-ii, 2026-10-10) ───────────────

/// The words a pass carries when its Message tick is on: an invitation and a voice message are
/// forms of messaging, so they travel together with it (10c-ii). The relay enforces only
/// `message` today; the other two stay in the pass for when it enforces them as well.
pub const MESSAGE_TICK_WORDS: [&str; 3] = ["message", "invite", "voice_message"];

/// The one word a pass carries when all three ticks are off. See [`intended_may`] for why.
pub const NO_TICKS_WORD: &str = "invite";

/// One friend's ticks in Settings > Safety's "People I choose" (10c-ii): which kinds of contact
/// the pass I give them allows. They count only for a row set to "People I choose"; under
/// "Friends" every friend gets through whatever is ticked. Kept per friend in the DM store (per
/// server, encrypted); a friend with no saved choice has the defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FriendTicks {
    pub message: bool,
    pub call: bool,
    pub trade: bool,
}

impl Default for FriendTicks {
    /// Message and Trade ticked, Call not: the same as a new friend's pass (step A), so a friend
    /// nobody has chosen anything for holds exactly the pass they were first given.
    fn default() -> Self {
        Self { message: true, call: false, trade: true }
    }
}

impl FriendTicks {
    pub fn get(&self, kind: ReachKind) -> bool {
        match kind {
            ReachKind::Message => self.message,
            ReachKind::Call => self.call,
            ReachKind::Trade => self.trade,
        }
    }

    pub fn set(&mut self, kind: ReachKind, on: bool) {
        match kind {
            ReachKind::Message => self.message = on,
            ReachKind::Call => self.call = on,
            ReachKind::Trade => self.trade = on,
        }
    }

    /// The ticks a pass's `may` (canonical or not) stands for: how this device learns a choice
    /// made on another of the person's devices, from the echo of the pass that device re-issued.
    pub fn from_may(may: &str) -> Self {
        Self { message: may_names(may, ReachKind::Message), call: may_names(may, ReachKind::Call), trade: may_names(may, ReachKind::Trade) }
    }
}

/// Does a pass's `may` name this kind?
fn may_names(may: &str, kind: ReachKind) -> bool {
    may.split(',').any(|k| k == kind.wire())
}

/// The kinds a pass to a friend should allow, from the person's ticks for them (10c-ii):
/// `message`, `invite` and `voice_message` with Message, `trade` with Trade, `call` with Call.
pub fn intended_may(ticks: FriendTicks) -> Vec<&'static str> {
    let mut words: Vec<&'static str> = Vec::new();
    if ticks.message {
        words.extend(MESSAGE_TICK_WORDS);
    }
    if ticks.trade {
        words.push(ReachKind::Trade.wire());
    }
    if ticks.call {
        words.push(ReachKind::Call.wire());
    }
    if words.is_empty() {
        // All three unticked. The pass format refuses an empty `may` (`FriendMay::from_words` in
        // relay/core/pq_crypto.rs), so the pass carries `invite` alone, which gives nothing the
        // relay enforces today. Unticking everything does not end the friendship: it is still a
        // mutual follow, and a row set to "Friends" lets them through, which needs them to hold
        // a pass. So the pass stays, allowing nothing a "People I choose" row checks.
        words.push(NO_TICKS_WORD);
    }
    words
}

/// The canonical `may` (sorted, comma-joined) of [`intended_may`], as passes carry it.
pub fn intended_may_wire(ticks: FriendTicks) -> String {
    crate::relay::core::pq_crypto::FriendMay::from_words(intended_may(ticks)).map(|m| m.wire()).unwrap_or_default()
}

/// Does a pass allowing `may` let its holder through for a kind the relay checks today that
/// `want` does not? That is, would replacing it with a `want` pass TAKE something away? Only the
/// kinds the relay enforces count: going from the all-unticked pass (`invite` alone) to one with
/// Trade ticked drops `invite` on the wire, but takes nothing away anyone can feel, and must not
/// withdraw the friend's only pass while the new one waits to go out (engine/dm.rs `reissue_pass`).
pub fn grants_beyond(may: &str, want: &str) -> bool {
    ReachKind::ALL.into_iter().any(|k| may_names(may, k) && !may_names(want, k))
}

/// "A", "A and B", "A, B and C".
fn join_and(words: &[&str]) -> String {
    match words {
        [] => String::new(),
        [one] => one.to_string(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// The line above the "People I choose" list saying when its ticks count (10c-ii).
pub const CHOSEN_TICKS_NOTE: &str = "These ticks count for a row set to People I choose.";

/// [`chosen_in_use_line`] when no row is set to "People I choose".
pub const CHOSEN_TICKS_UNUSED: &str = "Not in use now: no row is set to People I choose.";

/// For a row NOT set to "People I choose": who gets through it instead of the ticked friends.
/// Word for word as the web client says it (web/shared/reach.js `REACH_THROUGH`).
fn who_gets_through(audience: Audience) -> &'static str {
    match audience {
        Audience::Nobody => "no one gets through",
        // Not reached: a row on "People I choose" is named in the "In use now" list instead.
        Audience::Chosen => "the friends you tick get through",
        Audience::Friends => "every friend gets through",
        Audience::Groups => "every friend and everyone in your groups gets through",
        Audience::Anyone => "anyone gets through",
    }
}

/// The line directly under [`CHOSEN_TICKS_NOTE`] on the Safety page (10c-ii), both above the
/// list: which rows use the ticks now, and for each other row who gets through instead, built
/// from the person's settings. For example "In use now: Calls. Messages and Trades are set to
/// Friends, so every friend gets through for those." The other rows are grouped by audience in
/// the page's row order (Messages, Calls, Trades), one sentence each. The row names are plural
/// nouns, so "are" and "those" fit even a single row. Word for word as the web client builds it
/// (web/shared/reach.js `reachTicksInUse`), so both apps say the same thing.
pub fn chosen_in_use_line(settings: &ReachSettings) -> String {
    let chosen: Vec<&str> = ReachKind::ALL.into_iter().filter(|k| settings.get(*k) == Audience::Chosen).map(ReachKind::label).collect();
    if chosen.is_empty() {
        return CHOSEN_TICKS_UNUSED.to_string();
    }
    let mut line = format!("In use now: {}.", join_and(&chosen));
    // The other rows' audiences, each once, in the order the rows first show it.
    let mut audiences: Vec<Audience> = Vec::new();
    for kind in ReachKind::ALL {
        let a = settings.get(kind);
        if a != Audience::Chosen && !audiences.contains(&a) {
            audiences.push(a);
        }
    }
    for audience in audiences {
        let rows: Vec<&str> = ReachKind::ALL.into_iter().filter(|k| settings.get(*k) == audience).map(ReachKind::label).collect();
        line.push_str(&format!(" {} are set to {}, so {} for those.", join_and(&rows), audience.label(), who_gets_through(audience)));
    }
    line
}

/// The Safety page's state that lives only in this run: what we asked the relay for and are
/// waiting to hear back about, and which people refused our messages (for the notice in their
/// conversation). Reset with the other per-server transients on a server switch.
/// Where a refused message to someone stands, for the notice in our conversation with them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Their gate refused a message: the notice offers a contact request.
    Refused,
    /// We sent them a contact request.
    RequestSent,
    /// Our contact request was refused too, which only "Nobody" does (the relay marks it
    /// `request: true`, 2026-10-10): the notice says so and offers nothing more.
    NotTakingRequests,
}

/// The notice's words when someone is not taking contact requests (the web chat says the same).
pub const NOT_TAKING_REQUESTS: &str = "They are not taking contact requests right now, so nothing was sent.";

#[derive(Debug, Default)]
pub struct ReachUi {
    /// A `reach_set` sent and not yet answered by a `reach_settings`, and when.
    pub asked: Option<(ReachSettings, std::time::Instant)>,
    /// People whose gate refused a message from us (`reach_refused`), by key, and where that stands.
    pub refused: std::collections::HashMap<String, Refusal>,
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
        assert_eq!(ReachKind::ALL.map(|k| k.tick_label()), ["Message", "Call", "Trade"], "the ticks' words of 10c-ii");
    }

    /// The kinds of a `may`, as a set, to say what a change added and dropped.
    fn kinds(may: &str) -> std::collections::BTreeSet<String> {
        may.split(',').filter(|k| !k.is_empty()).map(str::to_string).collect()
    }

    /// 10c-ii: a friend with no saved choice has Message and Trade ticked and Call not, and the
    /// pass they get is exactly the step A default pass, so nobody's pass changes until someone
    /// chooses something.
    /// Seen red 2026-10-10 with `FriendTicks::default()` giving `call: true`: "Call is not
    /// ticked by default" failed.
    #[test]
    fn a_friend_with_no_choice_gets_the_default_may() {
        let d = FriendTicks::default();
        assert!(d.message && d.trade, "Message and Trade are ticked by default");
        assert!(!d.call, "Call is not ticked by default");
        let step_a = crate::relay::core::pq_crypto::FriendMay::from_words(crate::relay::core::pq_crypto::FRIEND_PASS_DEFAULT_MAY).unwrap().wire();
        assert_eq!(intended_may_wire(d), step_a, "the same pass a new friend gets");
        assert_eq!(intended_may_wire(d), "invite,message,trade,voice_message");
        assert_eq!(FriendTicks::from_may(&step_a), d, "and that pass reads back as the defaults");
    }

    /// 10c-ii: unticking Message drops `message`, `invite` and `voice_message` together, and
    /// nothing else; ticking it again brings back the same three.
    /// Seen red 2026-10-10 with `invite` and `voice_message` pushed whatever the Message tick
    /// said: "unticking Message drops exactly its three words" failed (only `message` dropped).
    #[test]
    fn unticking_message_drops_exactly_its_three_words() {
        let before = intended_may_wire(FriendTicks::default());
        let after = intended_may_wire(FriendTicks { message: false, ..FriendTicks::default() });
        let dropped: Vec<String> = kinds(&before).difference(&kinds(&after)).cloned().collect();
        let added: Vec<String> = kinds(&after).difference(&kinds(&before)).cloned().collect();
        assert_eq!(dropped, ["invite", "message", "voice_message"], "unticking Message drops exactly its three words");
        assert!(added.is_empty(), "and adds nothing");
        assert_eq!(after, "trade", "Trade still ticked");
        // The same three, whatever else is ticked.
        let with_call = intended_may_wire(FriendTicks { message: true, call: true, trade: false });
        let without = intended_may_wire(FriendTicks { message: false, call: true, trade: false });
        assert_eq!(kinds(&with_call).difference(&kinds(&without)).cloned().collect::<Vec<_>>(), ["invite", "message", "voice_message"]);
        assert_eq!(without, "call");
    }

    /// 10c-ii: ticking Call adds `call` and nothing else, from any starting point.
    /// Seen red 2026-10-10 with the Call branch pushing `voice_message` beside `call`: "ticking
    /// Call adds only call" failed (added `call` and `voice_message` with Message unticked).
    #[test]
    fn ticking_call_adds_only_call() {
        for message in [true, false] {
            for trade in [true, false] {
                let off = FriendTicks { message, call: false, trade };
                let on = FriendTicks { call: true, ..off };
                let (before, after) = (intended_may_wire(off), intended_may_wire(on));
                let added: Vec<String> = kinds(&after).difference(&kinds(&before)).cloned().collect();
                let dropped: Vec<String> = kinds(&before).difference(&kinds(&after)).cloned().collect();
                if !message && !trade {
                    // From the all-unticked pass (`invite` alone), the filler word goes once a real
                    // kind is ticked; `call` is still the only thing added.
                    assert_eq!((added.as_slice(), dropped.as_slice()), (&["call".to_string()][..], &["invite".to_string()][..]), "from nothing ticked");
                } else {
                    assert_eq!(added, ["call"], "ticking Call adds only call (message {message}, trade {trade})");
                    assert!(dropped.is_empty(), "and drops nothing (message {message}, trade {trade})");
                }
                assert!(FriendTicks::from_may(&after).call, "and reads back as ticked");
            }
        }
    }

    /// 10c-ii: with all three unticked the pass is `invite` alone, which the format accepts (an
    /// empty `may` it refuses), so the friend still holds a valid pass that a "Friends" row lets
    /// through and a "People I choose" row does not.
    /// Seen red 2026-10-10 with the `invite` fallback taken out of `intended_may`: "a may the
    /// format accepts" failed (the may was empty).
    #[test]
    fn all_three_unticked_still_gives_a_valid_pass() {
        use crate::relay::core::pq_crypto::{build_friend_cert, derive_dilithium_seed, verify_friend_cert, DilithiumKeypair, FriendMay};
        let none = FriendTicks { message: false, call: false, trade: false };
        let may = intended_may_wire(none);
        assert!(FriendMay::parse(&may).is_ok(), "a may the format accepts, got {may:?}");
        assert_eq!(may, "invite", "invite alone");
        assert_eq!(FriendTicks::from_may(&may), none, "which reads back as nothing ticked");

        let seed = [9u8; 32];
        let me = hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed)).public_key());
        let server = "did:hum:4dQe1bVHyiHm1Vh8rWbx2F";
        let cert = build_friend_cert(&seed, server, &me, "ben", &"ab".repeat(16), &intended_may(none)).expect("the pass is minted");
        assert_eq!(verify_friend_cert(server, &me, "ben", &cert).map(|p| p.may.wire()), Ok("invite".to_string()), "and it checks out");

        let held = [pass(&may)];
        let rel = Relation { is_me: false, passes: &held, shares_group: false };
        for kind in ReachKind::ALL {
            assert!(admits(Audience::Friends, kind, &rel), "Friends still lets them through for {kind:?}");
            assert!(!admits(Audience::Chosen, kind, &rel), "People I choose does not, for {kind:?}");
        }
    }

    /// What a re-issue takes away: only the kinds the relay checks count, so going from nothing
    /// ticked to Trade takes nothing away, while unticking Message or Call does.
    /// Seen red 2026-10-10 with `grants_beyond` comparing every word of the pass: "from nothing
    /// ticked to Trade takes nothing away" failed (`invite` counted).
    #[test]
    fn what_a_reissue_takes_away() {
        let w = |message, call, trade| intended_may_wire(FriendTicks { message, call, trade });
        assert!(!grants_beyond(&w(false, false, false), &w(false, false, true)), "from nothing ticked to Trade takes nothing away");
        assert!(!grants_beyond(&w(true, false, true), &w(true, true, true)), "ticking Call takes nothing away");
        assert!(grants_beyond(&w(true, false, true), &w(false, false, true)), "unticking Message does");
        assert!(grants_beyond(&w(true, true, true), &w(true, false, true)), "unticking Call does");
        assert!(grants_beyond(&w(true, false, true), &w(false, false, false)), "unticking everything does");
    }

    /// 10c-ii: the "In use now" line for each mix of row settings: none set to People I choose,
    /// one, two, all three, and the other rows grouped by audience in the page's row order, one
    /// sentence each, in the web client's exact words (web/shared/reach.js `reachTicksInUse`).
    /// Seen red 2026-10-10 with every row not set to People I choose named under the first
    /// one's audience: "one row, the other two on different audiences" failed (Trades named
    /// under Anyone).
    #[test]
    fn in_use_now_line_for_each_mix() {
        use Audience::*;
        let s = |message, call, trade| ReachSettings { message, call, trade };
        assert_eq!(chosen_in_use_line(&s(Friends, Friends, Friends)), "Not in use now: no row is set to People I choose.", "none");
        assert_eq!(chosen_in_use_line(&s(Nobody, Anyone, Groups)), "Not in use now: no row is set to People I choose.", "none, other audiences");
        assert_eq!(
            chosen_in_use_line(&ReachSettings::default()),
            "In use now: Calls. Messages and Trades are set to Friends, so every friend gets through for those.",
            "one row (the safe defaults), the spec's own example"
        );
        assert_eq!(
            chosen_in_use_line(&s(Anyone, Chosen, Friends)),
            "In use now: Calls. Messages are set to Anyone, so anyone gets through for those. Trades are set to Friends, so every friend gets through for those.",
            "one row, the other two on different audiences"
        );
        assert_eq!(
            chosen_in_use_line(&s(Chosen, Chosen, Friends)),
            "In use now: Messages and Calls. Trades are set to Friends, so every friend gets through for those.",
            "two rows"
        );
        assert_eq!(
            chosen_in_use_line(&s(Chosen, Nobody, Chosen)),
            "In use now: Messages and Trades. Calls are set to Nobody, so no one gets through for those.",
            "two rows, the third on Nobody"
        );
        assert_eq!(
            chosen_in_use_line(&s(Groups, Chosen, Groups)),
            "In use now: Calls. Messages and Trades are set to Friends and people in my groups, so every friend and everyone in your groups gets through for those.",
            "one row, the other two on groups"
        );
        assert_eq!(
            chosen_in_use_line(&s(Nobody, Chosen, Anyone)),
            "In use now: Calls. Messages are set to Nobody, so no one gets through for those. Trades are set to Anyone, so anyone gets through for those.",
            "one row, the others on Nobody and Anyone"
        );
        assert_eq!(chosen_in_use_line(&s(Chosen, Chosen, Chosen)), "In use now: Messages, Calls and Trades.", "all three");
        assert_eq!(CHOSEN_TICKS_NOTE, "These ticks count for a row set to People I choose.", "the line above, word for word");
    }

    /// The help line under a row set to "People I choose" names the list for each kind
    /// (10c-ii), in the web client's exact words (web/shared/reach.js `reachExplain`).
    /// Seen red 2026-10-10 with the Calls line still naming "People who may call me": "Calls
    /// names the new list" failed.
    #[test]
    fn people_i_choose_rows_name_the_list() {
        assert_eq!(
            Audience::Chosen.meaning(ReachKind::Message),
            "Only the friends you tick for Message on your \"People I choose\" list can message you. Anyone else can send a contact request that shows you only their name.",
            "Messages names the new list"
        );
        assert_eq!(Audience::Chosen.meaning(ReachKind::Call), "Only the friends you tick for Call on your \"People I choose\" list can call you.", "Calls names the new list");
        assert_eq!(
            Audience::Chosen.meaning(ReachKind::Trade),
            "Only the friends you tick for Trade on your \"People I choose\" list can send you trade requests.",
            "Trades names the new list"
        );
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
