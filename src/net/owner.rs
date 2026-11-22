//! The server's owner in the desktop app (2026-11-20, docs/design/hide-and-keep.md, "How the owner
//! role is set"): Claim ownership, Hand over ownership and the named Successor, the client's rules
//! with no socket and no GUI so each one is unit tested. The screen is
//! src/gui/pages/server_settings/owner.rs; the frames and the background job are
//! src/engine/owner.rs. The relay's half is src/relay/handlers/owner.rs; the website's twin is
//! built beside this one with the same words.
//!
//! WHY. Hide and keep seals every held copy to the owner's key card (and to the successor's, when
//! the owner named one). The operator approved on 2026-11-20 a one-time claim code the relay
//! prints to its log, a handover by the living owner, and a named successor ("Might as well, just
//! in case."). His concern: "We just want to make sure the system itself can't be abused." So the
//! relay's word is never enough for whom a copy is sealed to (the owner branch's second review,
//! 2026-11-20, W1: the relay chooses which members it lists and under what names):
//! - the person picked is told apart by their KEY: EVERY handover and naming of a successor goes
//!   ahead only once the owner typed the FOUR groups of the person's fingerprint at positions drawn
//!   at random for each ask, as the person read them out FROM THEIR OWN SCREEN (Settings > Account,
//!   and nowhere else: the sixth review's H1), and they match the listed key ([`judge`], core
//!   `key_card::judge_at`).
//!   The owner's picker names only the positions: the listed key's fingerprint is not on the
//!   owner's screen until the typed groups matched (the fifth review's H1). A typed set that does
//!   not match is a warning in words, logged, and that key is refused on that server until the app
//!   restarts ([`OwnerUi::refused`], per server and key: the sixth review's L1). A name an honest relay could not list is shown as a refused name (core
//!   `key_card::honest_name`, the third review's M2). No key is exempt (the fourth review's finding
//!   1): not a friend given a pass (a mutual follow is one click on a name the relay listed), not a
//!   pinned card, not a key confirmed before;
//! - every recipient's Kyber768 key comes only from a key card this device checked (signed by that
//!   very person's key and naming them, dated as the relay said) ([`recipient_keys`]);
//! - the relay's `owner_change_ready` must name the very action this device signed (its object id
//!   is the grant), the person it named, and exactly the people that change seals to, in slot order
//!   ([`check_ready`]): a relay that slipped its own key in among the recipients is refused;
//! - a recipient's device checks the owner's signed action that `owner_change_check` carries
//!   ([`check_action`]), and its own change is known by that action being signed by its own key,
//!   not by anything it remembers ([`is_own_change`]).
//!
//! THE FRAMES. `owner_state {has_owner}` (to everyone), `owner_reply {kind, ok, message}` (to the
//! one who acted), `owner_change_ready {grant, action, target, recipients, items}` (to the owner);
//! and from here `owner_claim {code}`, `owner_change_begin {action}`, `owner_change_finish {grant}`,
//! `owner_change_cancel`. A change RE-WRAPS every held copy (format v4, core `held_seal`): between
//! ready and finish the device fetches each part's HEADER alone (`GET /api/held/{id}/{part}/header
//! ?grant=`), opens its own slot, wraps the same part key for the recipients ([`rewrap_all`], core
//! `held_seal::rewrap_package`) and sends the new header (`PUT` the same route). No body is fetched,
//! opened or sent, nothing is drawn, and the finish carries no key.

use humanity_core::i18n::{t, tf};
use humanity_protocol::held_seal;
use humanity_protocol::key_card;
use humanity_protocol::pq_crypto::KyberKeypair;
use serde_json::Value;

use super::held::{Caller, Signed};

/// The owner's three signed changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// Hand the owner role to a member.
    Handover,
    /// Name a member the successor every held copy is also sealed to.
    Successor,
    /// Remove the successor named.
    SuccessorRemove,
}

impl Change {
    /// The signed `mod_action_v1` action.
    pub fn action(self) -> &'static str {
        match self {
            Change::Handover => "owner_handover",
            Change::Successor => "owner_successor",
            Change::SuccessorRemove => "owner_successor_remove",
        }
    }

    pub fn from_action(action: &str) -> Option<Self> {
        match action {
            "owner_handover" => Some(Change::Handover),
            "owner_successor" => Some(Change::Successor),
            "owner_successor_remove" => Some(Change::SuccessorRemove),
            _ => None,
        }
    }
}

/// The successor the owner named, as `held_items` lists it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Successor {
    pub key: String,
    pub name: String,
}

/// One line of the owner log: a claim, a handover or a change of successor.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OwnerLogLine {
    /// "claim", "owner_handover", "owner_successor" or "owner_successor_remove".
    pub kind: String,
    /// The owner who signed it (empty for a claim).
    pub old_name: String,
    /// The claimer, the new owner, or the successor named or removed.
    pub new_name: String,
    pub reason: String,
    pub at: i64,
}

/// An `owner_reply`: how a claim, a handover or a change of successor went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    /// "claim", "handover" or "successor".
    pub kind: String,
    pub ok: bool,
    pub message: String,
}

/// One person a change seals every held copy to, with the time of the key card the relay holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipient {
    pub key: String,
    pub card_at: u64,
}

/// One held item a change seals again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadyItem {
    pub id: i64,
    pub package: String,
    pub parts: i64,
}

/// The relay's `owner_change_ready`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ready {
    pub grant: String,
    pub action: String,
    pub target: String,
    pub recipients: Vec<Recipient>,
    pub items: Vec<ReadyItem>,
}

fn text(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

/// Read `owner_state`: whether a key holds the owner role. None for any other frame.
pub fn parse_state(frame: &Value) -> Option<bool> {
    (frame.get("type").and_then(Value::as_str) == Some("owner_state")).then(|| frame.get("has_owner").and_then(Value::as_bool).unwrap_or(true))
}

/// Read `owner_reply`. None for any other frame.
pub fn parse_reply(frame: &Value) -> Option<Reply> {
    (frame.get("type").and_then(Value::as_str) == Some("owner_reply")).then(|| Reply {
        kind: text(frame, "kind"),
        ok: frame.get("ok").and_then(Value::as_bool).unwrap_or(false),
        message: text(frame, "message"),
    })
}

/// Read `owner_change_ready`. None for any other frame.
pub fn parse_ready(frame: &Value) -> Option<Ready> {
    if frame.get("type").and_then(Value::as_str) != Some("owner_change_ready") {
        return None;
    }
    let list = |k: &str| frame.get(k).and_then(Value::as_array).cloned().unwrap_or_default();
    Some(Ready {
        grant: text(frame, "grant"),
        action: text(frame, "action"),
        target: text(frame, "target"),
        recipients: list("recipients").iter().map(|r| Recipient { key: text(r, "key"), card_at: r.get("card_at").and_then(Value::as_u64).unwrap_or(0) }).collect(),
        items: list("items")
            .iter()
            .map(|i| ReadyItem { id: i.get("id").and_then(Value::as_i64).unwrap_or(0), package: text(i, "package"), parts: i.get("parts").and_then(Value::as_i64).unwrap_or(0) })
            .collect(),
    })
}

/// The successor in a `held_items` frame (None when none is named).
pub fn parse_successor(frame: &Value) -> Option<Successor> {
    let s = frame.get("successor").filter(|s| s.is_object())?;
    let key = text(s, "key");
    (!key.is_empty()).then(|| Successor { key, name: text(s, "name") })
}

/// The owner log in a `held_items` frame, newest first as the relay sends it.
pub fn parse_log(frame: &Value) -> Vec<OwnerLogLine> {
    frame
        .get("owner_log")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|l| OwnerLogLine {
            kind: text(l, "kind"),
            old_name: text(l, "old_name"),
            new_name: text(l, "new_name"),
            reason: text(l, "reason"),
            at: l.get("at").and_then(Value::as_i64).unwrap_or(0),
        })
        .collect()
}

/// Sign `change` naming the member `target` (their key) as the owner whose seed this is, for the
/// server `server_did`, at `now_ms`, with the reason the owner typed.
pub fn sign(seed: &[u8], server_did: &str, change: Change, target: &str, reason: &str, now_ms: u64) -> Result<Signed, String> {
    use humanity_protocol::moderation::{build_mod_action_v1, ModAction};
    if server_did.is_empty() {
        return Err("no server identity".into());
    }
    let a = ModAction { action: change.action(), target, target_kind: "identity", reason: reason.trim(), rule: "", expires_at: 0, role: "" };
    let o = build_mod_action_v1(seed, server_did, &a, now_ms, &[]).map_err(|e| e.to_string())?;
    super::held::signed_of(&o)
}

/// The `owner_claim` frame carrying the pasted code (spaces and line breaks around it dropped).
pub fn claim_frame(code: &str) -> String {
    serde_json::json!({ "type": "owner_claim", "code": code.trim() }).to_string()
}

/// A character no one sees: a zero-width or other format character (Unicode's Cf), which a pasted
/// line can carry in front of `/claim` without changing how it looks.
pub fn is_invisible(c: char) -> bool {
    matches!(c, '\u{00AD}' | '\u{061C}' | '\u{180E}' | '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{206F}' | '\u{FEFF}' | '\u{FFF9}'..='\u{FFFB}')
}

/// What a composer line starting with `/claim` is ([`typed_claim`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedClaim {
    /// A well-formed claim: the code (empty for the bare command), sent to the server alone.
    Code(String),
    /// Anything else that starts with `/claim`: never sent anywhere; the claim help is shown.
    Help,
}

/// A composer line (or an edit) whose first word starts with `/claim` (the owner review's finding 6
/// and the second review's W5, 2026-11-20): zero-width and format characters removed and the line
/// trimmed, `/claim` in any letter case followed by a space, a colon, the code directly, or nothing.
/// Such a line is NEVER a message to a person or a group, whatever room, direct message or group is
/// open: a code of letters, digits and dashes alone is sent to the server alone ([`typed_claim_frame`]),
/// anything else shows the claim help. None for every other line.
pub fn typed_claim(line: &str) -> Option<TypedClaim> {
    let clean: String = line.chars().filter(|c| !is_invisible(*c)).collect();
    let s = clean.trim();
    let head = s.get(..6)?;
    if !head.eq_ignore_ascii_case("/claim") {
        return None;
    }
    // `get`, never a byte slice (BUG-340's lint): the six bytes before it are ASCII "/claim", so it
    // always succeeds, and it could never panic if that check moved.
    let code = s.get(6..)?.trim_start_matches(|c: char| c == ':' || c.is_whitespace()).trim();
    if code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        Some(TypedClaim::Code(code.to_string()))
    } else {
        Some(TypedClaim::Help)
    }
}

/// The `owner_claim` frame of a `/claim` typed in the chat: the relay answers in the chat too.
pub fn typed_claim_frame(code: &str) -> String {
    serde_json::json!({ "type": "owner_claim", "code": code.trim(), "typed": true }).to_string()
}

/// The `owner_change_begin` frame carrying the owner's signed change.
pub fn begin_frame(signed: &Signed) -> String {
    serde_json::json!({ "type": "owner_change_begin", "action": signed.submission }).to_string()
}

/// The `owner_change_finish` frame: every new header is in. No key travels (part 0's body never
/// changes in a re-wrap).
pub fn finish_frame(grant: &str) -> String {
    serde_json::json!({ "type": "owner_change_finish", "grant": grant }).to_string()
}

/// The `owner_change_cancel` frame.
pub fn cancel_frame() -> String {
    serde_json::json!({ "type": "owner_change_cancel" }).to_string()
}

// ── A recipient's check (the owner review's finding 5; re-wrap, the second review) ──

/// The relay's `owner_change_check`: an owner change waits for this device to check its own slot of
/// every new header and accept, by `until` (Unix ms). `object` is the owner's signed action
/// ([`check_action`]); `round` is bound into the acceptance.
#[derive(Debug, Clone, PartialEq)]
pub struct Check {
    pub grant: String,
    pub round: String,
    pub action: String,
    pub object: Value,
    pub owner: String,
    pub owner_name: String,
    pub target: String,
    pub target_name: String,
    pub items: Vec<ReadyItem>,
    pub until: i64,
}

/// Read `owner_change_check`. None for any other frame.
pub fn parse_check(frame: &Value) -> Option<Check> {
    if frame.get("type").and_then(Value::as_str) != Some("owner_change_check") {
        return None;
    }
    let items = frame.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
    Some(Check {
        grant: text(frame, "grant"),
        round: text(frame, "round"),
        action: text(frame, "action"),
        object: frame.get("object").cloned().unwrap_or(Value::Null),
        owner: text(frame, "owner"),
        owner_name: text(frame, "owner_name"),
        target: text(frame, "target"),
        target_name: text(frame, "target_name"),
        items: items.iter().map(|i| ReadyItem { id: i.get("id").and_then(Value::as_i64).unwrap_or(0), package: text(i, "package"), parts: i.get("parts").and_then(Value::as_i64).unwrap_or(0) }).collect(),
        until: frame.get("until").and_then(Value::as_i64).unwrap_or(0),
    })
}

/// A signed object as a client submits it (base64 fields), read back. None when it is not one.
fn object_of(v: &Value) -> Option<humanity_protocol::object::Object> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    let s = |k: &str| v.get(k).and_then(Value::as_str);
    let n = |k: &str| v.get(k).and_then(Value::as_u64);
    Some(humanity_protocol::object::Object {
        protocol_version: n("protocol_version")?,
        object_type: s("object_type")?.to_string(),
        space_id: s("space_id").map(str::to_string),
        channel_id: s("channel_id").map(str::to_string),
        author_public_key: B64.decode(s("author_public_key_b64")?).ok()?,
        created_at: n("created_at"),
        references: v.get("references").and_then(Value::as_array).map(|a| a.iter().filter_map(|r| r.as_str().map(str::to_string)).collect()).unwrap_or_default(),
        payload_schema_version: n("payload_schema_version")?,
        payload_encoding: s("payload_encoding")?.to_string(),
        payload: B64.decode(s("payload_b64")?).ok()?,
        signature: B64.decode(s("signature_b64")?).ok()?,
    })
}

/// The text field `key` of a signed action's payload.
fn payload_text(o: &humanity_protocol::object::Object, key: &str) -> Option<String> {
    let ciborium::Value::Map(entries) = humanity_protocol::encoding::from_canonical_bytes(&o.payload).ok()? else { return None };
    entries.into_iter().find_map(|(k, v)| match (k, v) {
        (ciborium::Value::Text(k), ciborium::Value::Text(v)) if k == key => Some(v),
        _ => None,
    })
}

/// Check, on this device, the owner's signed action `check` carries (the second review, W1 and W6):
/// its signature, signed by the owner the frame names, its object id the grant, and naming the action
/// and the person the frame says. The words why not.
pub fn check_action(check: &Check) -> Result<Change, String> {
    let bad = || t("owner.check_action_bad").to_string();
    let o = object_of(&check.object).ok_or_else(bad)?;
    o.verify_signature().map_err(|_| bad())?;
    let author = hex::encode(&o.author_public_key);
    let id = o.object_id().map_err(|_| bad())?.to_hex();
    let action = payload_text(&o, "action").unwrap_or_default();
    let change = Change::from_action(&action).ok_or_else(bad)?;
    if author != check.owner || id != check.grant || action != check.action || payload_text(&o, "target").as_deref() != Some(check.target.as_str()) {
        return Err(bad());
    }
    Ok(change)
}

/// Whether `check` is THIS device's own change (the second review, W6): its signed action checks out
/// and was signed by `me`. Known from the action itself, so it survives a reload or a second window;
/// the owner's own check then runs without asking.
pub fn is_own_change(check: &Check, me: &str) -> bool {
    !me.is_empty() && check.owner == me && check_action(check).is_ok()
}

/// What this device does with a check frame (the sixth review's H1 (c), 2026-11-21).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckGate {
    /// Its signed action verifies and its signer is this server's owner as this device knows it:
    /// the prompt may be drawn.
    Draw,
    /// Its signed action verifies, but this device does not know yet who the owner is (the member
    /// list has not come): held, nothing drawn, until it does.
    Wait,
    /// Anything else: nothing is drawn; the words are for the log.
    Refuse(&'static str),
}

/// Whether the prompt for `check` may be drawn, given `owners`, every key the member list shows
/// with the owner role (the sixth review's H1 (c): a relay sent an UNSIGNED check whose owner name
/// read "Your fingerprint is <its own key's>... read them from here", and the prompt was drawn as
/// the app's own words, the signature checked only on accept). Drawn only once its signed action
/// verifies here ([`check_action`]) AND its signer is the one owner listed; a list naming more than
/// one owner refuses (an honest server never lists two).
pub fn gate_check(check: &Check, owners: &[String]) -> CheckGate {
    if check_action(check).is_err() {
        return CheckGate::Refuse("its signed action does not verify on this device");
    }
    match owners {
        [] => CheckGate::Wait,
        [one] if *one == check.owner => CheckGate::Draw,
        [_] => CheckGate::Refuse("it is not signed by this server's owner as this device knows it"),
        _ => CheckGate::Refuse("the member list names more than one owner"),
    }
}

/// Check every held item of a change with this device's key `kyber` (its identity `me`): the signed
/// action first ([`check_action`]), then `me`'s own slot of every new header, at its place in the
/// order the action implies (core `held_seal::check_rewrapped_package`), each header read by
/// `fetch(id, part)`, `progress` hearing after every item. No body is read. The digest to sign.
pub fn check_all(kyber: &KyberKeypair, me: &str, check: &Check, mut fetch: impl FnMut(i64, usize) -> Result<Vec<u8>, String>, mut progress: impl FnMut(usize)) -> Result<String, String> {
    let change = check_action(check)?;
    let order = expected_recipients(change, &check.owner, &check.target);
    let slot = order.iter().position(|k| k == me).ok_or_else(|| t("owner.check_not_yours").to_string())?;
    let mut items = Vec::new();
    for (n, item) in check.items.iter().enumerate() {
        let hashes = held_seal::check_rewrapped_package(kyber, &item.package, item.parts.max(0) as usize, order.len(), slot, |part| fetch(item.id, part)).map_err(|why| format!("held item #{}: {why}", item.id))?;
        items.push((item.package.clone(), hashes));
        progress(n + 1);
    }
    Ok(held_seal::acceptance_digest(&check.round, &items))
}

/// Sign the acceptance of the change `grant` with `digest` (what this device checked), for the
/// server `server_did`, at `now_ms`.
pub fn sign_accept(seed: &[u8], server_did: &str, grant: &str, digest: &str, now_ms: u64) -> Result<Signed, String> {
    use humanity_protocol::moderation::{build_mod_action_v1, ModAction, OWNER_ACCEPT};
    if server_did.is_empty() {
        return Err("no server identity".into());
    }
    let reason = format!("checked {digest}");
    let a = ModAction { action: OWNER_ACCEPT, target: grant, target_kind: "object", reason: &reason, rule: "", expires_at: 0, role: "" };
    let o = build_mod_action_v1(seed, server_did, &a, now_ms, &[]).map_err(|e| e.to_string())?;
    super::held::signed_of(&o)
}

/// The `owner_change_accept` frame: the signed acceptance alone (no restore key travels).
pub fn accept_frame(signed: &Signed) -> String {
    serde_json::json!({ "type": "owner_change_accept", "action": signed.submission }).to_string()
}

/// The `owner_change_decline` frame, with why (a check that failed, or the person said no).
pub fn decline_frame(grant: &str, why: &str) -> String {
    serde_json::json!({ "type": "owner_change_decline", "grant": grant, "why": why }).to_string()
}

/// Whom `change`, signed by `owner` naming `target`, seals every held copy to, in slot order: the
/// new owner alone for a handover, the owner then the successor for a successor, the owner alone
/// when the successor is removed (the relay's `check_owner_change`).
pub fn expected_recipients(change: Change, owner: &str, target: &str) -> Vec<String> {
    match change {
        Change::Handover => vec![target.to_string()],
        Change::Successor => vec![owner.to_string(), target.to_string()],
        Change::SuccessorRemove => vec![owner.to_string()],
    }
}

/// Is the relay's `ready` the answer to what this device signed: its grant is the signed action's
/// object id, it names the same change and the same person, and it seals to exactly the people
/// that change seals to, in order. The words to show when not.
pub fn check_ready(ready: &Ready, change: Change, owner: &str, target: &str, grant: &str) -> Result<(), String> {
    if ready.grant != grant || Change::from_action(&ready.action) != Some(change) || ready.target != target {
        return Err(t("owner.ready_mismatch").to_string());
    }
    let named: Vec<&str> = ready.recipients.iter().map(|r| r.key.as_str()).collect();
    if named != expected_recipients(change, owner, target).iter().map(String::as_str).collect::<Vec<_>>() {
        return Err(t("owner.recipients_mismatch").to_string());
    }
    Ok(())
}

/// What checking one person's key card on this device found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardCheck {
    /// Signed by that very person's key and naming them: its time and its Kyber768 key.
    Good { at: u64, kyber: Vec<u8> },
    /// The server holds no card for them.
    None,
    /// A card that is not theirs, or not signed by their key, or of the wrong shape.
    Bad,
    /// A good card whose key is not the one this device pinned for them (net/key_pins.rs): their
    /// key changed, and nothing is sealed to it until the person accepts it in Chat.
    Changed,
    /// The card could not be fetched: the words why.
    Failed(String),
}

/// Check the card `card` (as the server sent it; None: it holds none) for the person `key`.
pub fn check_card(card: Option<&Value>, key: &str) -> CardCheck {
    let Some(card) = card else { return CardCheck::None };
    match key_card::parse_card(card).filter(|c| key_card::card_for(c, key)) {
        Some(c) => match key_card::kyber_bytes(&c) {
            Some(kyber) => CardCheck::Good { at: c.at, kyber },
            None => CardCheck::Bad,
        },
        None => CardCheck::Bad,
    }
}

/// A good card checked against the key this device pinned for the person (`pinned`, base64 as
/// the card carries it; None when nothing is pinned): another key is [`CardCheck::Changed`].
pub fn against_pin(check: CardCheck, pinned: Option<&str>) -> CardCheck {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    match (&check, pinned) {
        (CardCheck::Good { kyber, .. }, Some(pin)) if B64.encode(kyber) != pin => CardCheck::Changed,
        _ => check,
    }
}

/// The words for a card check that did not come out good, naming the person as `name`.
pub fn card_words(check: &CardCheck, name: &str) -> Option<String> {
    match check {
        CardCheck::Good { .. } => None,
        CardCheck::None => Some(tf("owner.card_none", &[("name", &name)])),
        CardCheck::Bad => Some(tf("owner.card_bad", &[("name", &name)])),
        CardCheck::Changed => Some(tf("owner.card_changed", &[("name", &name)])),
        CardCheck::Failed(why) => Some(tf("owner.card_fetch_failed", &[("name", &name), ("why", why)])),
    }
}

/// Every recipient's Kyber768 key, in slot order, each from a card this device checked: fetched
/// with `fetch_card(key)` (None: the server holds none), signed by that very key and naming it,
/// and dated `card_at` (the card the relay checked, so the parts the relay takes are the ones it
/// will hand on), and the key this device pinned for them when it pinned one (`pinned(key)`,
/// [`against_pin`]). The owner's own card must carry the owner's own key, `my_kyber`. `name(key)`
/// names a person in the words.
pub fn recipient_keys(
    me: &str,
    my_kyber: &[u8],
    recipients: &[Recipient],
    mut fetch_card: impl FnMut(&str) -> Result<Option<Value>, String>,
    pinned: impl Fn(&str) -> Option<String>,
    name: impl Fn(&str) -> String,
) -> Result<Vec<Vec<u8>>, String> {
    let mut keys = Vec::with_capacity(recipients.len());
    for r in recipients {
        let who = name(&r.key);
        let card = fetch_card(&r.key).map_err(|why| tf("owner.card_fetch_failed", &[("name", &who), ("why", &why)]))?;
        let (at, kyber) = match against_pin(check_card(card.as_ref(), &r.key), pinned(&r.key).as_deref()) {
            CardCheck::Good { at, kyber } => (at, kyber),
            other => return Err(card_words(&other, &who).unwrap_or_default()),
        };
        if at != r.card_at {
            return Err(tf("owner.card_at_mismatch", &[("name", &who)]));
        }
        if r.key == me && kyber != my_kyber {
            return Err(t("owner.own_card_not_mine").to_string());
        }
        keys.push(kyber);
    }
    Ok(keys)
}

// ── Telling the person picked apart (the second review, W1) ──

/// What the owner typed for the person `key`, judged (core `key_card::judge_at`): only
/// `Typed::Matched` lets the change go ahead, and only when `typed` is the four groups of the
/// listed key's fingerprint at the positions this device `asked` for (drawn at random per ask),
/// as the person read them from their own screen. Nothing asked is nothing typed. Every time, for
/// every key: a friend given a pass, a pinned card or a key confirmed before is no exemption (the
/// fourth review's finding 1). Two members with one name are told apart the same way.
pub fn judge(key: &str, asked: Option<key_card::Asked>, typed: &str) -> key_card::Typed {
    match asked {
        Some(at) => key_card::judge_at(key, at, typed),
        None => key_card::Typed::Empty,
    }
}

/// The name the picker shows for a member the relay listed as `name`: the name itself when an
/// honest relay could have listed it (core `key_card::honest_name`), else a refused name in words,
/// never the relay's text (it could carry a fingerprint: the third review's M2).
pub fn shown_name(name: &str) -> String {
    if key_card::honest_name(name) {
        name.to_string()
    } else {
        t("owner.name_refused").to_string()
    }
}

/// How far a re-wrap has come.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Progress {
    pub items_done: usize,
    pub items: usize,
    pub parts_done: usize,
    pub parts: usize,
}

/// Re-wrap every held item in `items` for `recipients` (Kyber768 keys in slot order, each from a
/// checked card), opening each part's header with the owner's key `me` (core
/// `held_seal::rewrap_package`): `fetch(id, part)` reads a part's current header, `upload(id, part,
/// header)` sends the new one, `progress` hears after every part. No body is fetched or sent.
pub fn rewrap_all(
    me: &KyberKeypair,
    recipients: &[Vec<u8>],
    items: &[ReadyItem],
    mut fetch: impl FnMut(i64, usize) -> Result<Vec<u8>, String>,
    mut upload: impl FnMut(i64, usize, &[u8]) -> Result<(), String>,
    mut progress: impl FnMut(Progress),
) -> Result<(), String> {
    let to: Vec<&[u8]> = recipients.iter().map(Vec::as_slice).collect();
    let mut at = Progress { items: items.len(), parts: items.iter().map(|i| i.parts.max(0) as usize).sum(), ..Progress::default() };
    progress(at);
    for item in items {
        held_seal::rewrap_package(me, &to, &item.package, item.parts.max(0) as usize, |part| fetch(item.id, part), |part, header| {
            upload(item.id, part, header)?;
            at.parts_done += 1;
            progress(at);
            Ok(())
        })?;
        at.items_done += 1;
        progress(at);
    }
    Ok(())
}

// ── Over the network (the job's thread, src/engine/owner.rs) ──

/// Where `key`'s key card is fetched.
pub fn card_url(base: &str, key: &str) -> String {
    format!("{}/api/key-card/{key}", base.trim_end_matches('/'))
}

/// Where a part's current header is fetched, and its new header sent, by the owner in a change.
pub fn header_url(base: &str, id: i64, part: usize, grant: &str) -> String {
    format!("{}/api/held/{id}/{part}/header?grant={grant}", base.trim_end_matches('/'))
}

/// Where a recipient fetches a new header to check it.
pub fn rewrapped_url(base: &str, id: i64, part: usize, grant: &str) -> String {
    format!("{}/api/held/{id}/{part}/rewrapped?grant={grant}", base.trim_end_matches('/'))
}

/// A header's bytes from the relay's answer: at most a header for the most recipients, so a relay
/// that sends a body gets nothing past it read.
fn header_of(r: ureq::Response) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut out = Vec::new();
    r.into_reader().take(held_seal::header_len(held_seal::MAX_RECIPIENTS) as u64 + 1).read_to_end(&mut out).map_err(|e| format!("the server could not be read ({e})"))?;
    Ok(out)
}

fn error_of(r: ureq::Response) -> String {
    r.into_string().ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()).and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_string)).unwrap_or_default()
}

/// The owner's device: part `part`'s current header under `grant`.
pub fn fetch_header(base: &str, id: i64, part: usize, grant: &str, caller: &Caller) -> Result<Vec<u8>, String> {
    let purpose = held_seal::caller_purpose("header", grant, id, part as i64, 0);
    match caller.sign(ureq::get(&header_url(base, id, part, grant)), &purpose).call() {
        Ok(r) => header_of(r),
        Err(ureq::Error::Status(_, _)) => Err("the server did not give this device the copy's header (the change may have been cancelled or run out of time)".into()),
        Err(e) => Err(format!("the server could not be reached ({e})")),
    }
}

/// The owner's device: send part `part`'s new header under `grant`.
pub fn upload_header(base: &str, id: i64, part: usize, grant: &str, caller: &Caller, header: &[u8]) -> Result<(), String> {
    let purpose = held_seal::caller_purpose("rewrap", grant, id, part as i64, 0);
    match caller.sign(ureq::put(&header_url(base, id, part, grant)), &purpose).send_bytes(header) {
        Ok(_) => Ok(()),
        Err(ureq::Error::Status(_, r)) => Err(error_of(r)),
        Err(e) => Err(format!("the server could not be reached ({e})")),
    }
}

/// A recipient's device: part `part`'s new header under `grant`, to check.
pub fn fetch_rewrapped(base: &str, id: i64, part: usize, grant: &str, caller: &Caller) -> Result<Vec<u8>, String> {
    let purpose = held_seal::caller_purpose("rewrapped", grant, id, part as i64, 0);
    match caller.sign(ureq::get(&rewrapped_url(base, id, part, grant)), &purpose).call() {
        Ok(r) => header_of(r),
        Err(ureq::Error::Status(_, _)) => Err("the server did not give this device the new header (the change may have been cancelled or run out of time)".into()),
        Err(e) => Err(format!("the server could not be reached ({e})")),
    }
}

/// `key`'s key card as the server holds it (None: it holds none). Checked by the caller.
pub fn fetch_card(base: &str, key: &str) -> Result<Option<Value>, String> {
    match ureq::get(&card_url(base, key)).call() {
        Ok(r) => r.into_string().map_err(|e| e.to_string()).and_then(|s| serde_json::from_str(&s).map_err(|e| e.to_string())).map(Some),
        Err(ureq::Error::Status(404, _)) => Ok(None),
        Err(ureq::Error::Status(code, r)) => Err(format!("{code}: {}", error_of(r))),
        Err(e) => Err(e.to_string()),
    }
}

/// What the owner's UI keeps (`GuiState::reports.owner`): the claim, the chosen member and their
/// card check, the change in flight and its job.
#[derive(Debug, Default)]
pub struct OwnerUi {
    /// From `owner_state`: None until this server says. "Claim ownership" shows only on Some(false).
    pub has_owner: Option<bool>,
    /// The claim code pasted.
    pub code: String,
    /// The relay's answer to the last claim, or why nothing was sent; and whether it worked.
    pub claim_words: String,
    pub claim_ok: bool,
    /// The search box over the members.
    pub search: String,
    /// The member chosen: their key and name.
    pub picked: Option<(String, String)>,
    /// The chosen member's card as this device checked it (None while it is being fetched).
    pub card: Option<CardCheck>,
    /// The card check running, for which key.
    pub card_job: Option<(String, std::sync::mpsc::Receiver<CardCheck>)>,
    /// The four groups of the chosen member's fingerprint this device asked for, typed by the owner
    /// as the person read them from their own screen.
    pub fp_confirm: String,
    /// Which four groups (0-based) of the chosen member's fingerprint were asked for, drawn
    /// uniformly when they were chosen and again after every change signed (core
    /// `key_card::confirm_positions`; the fifth review's H1). None for a refused key.
    pub asked: Option<key_card::Asked>,
    /// The key whose typed groups matched (Check these groups, or a change judged), so the picker
    /// shows its whole fingerprint now and not before; cleared when the groups typed, the member
    /// or the positions change.
    pub fp_ok: Option<String>,
    /// The keys whose typed groups did NOT match, per server and key: each is refused on that
    /// server until the app restarts, with the warning in words, so a key made to match a few groups
    /// gets one ask and never a fresh draw (the fifth review's H1 (c)). Kept across a switch of
    /// server (the sixth review's L1: `ReportUi::forget_server` reset it, so switching away and
    /// back gave a ground key a fresh draw).
    /// reuse: a record of what happened, kept for a security rule, not a result kept to save work;
    /// the switch must never change it.
    pub refused: RefusedKeys,
    /// The name typed to confirm, and the reason the change is signed with.
    pub confirm: String,
    pub reason: String,
    /// One line: the relay's answer to the last change, the job's words, or why nothing was sent.
    pub status: String,
    /// The change signed and sent, waiting for `owner_change_ready`, then running.
    pub pending: Option<PendingChange>,
    /// The running re-wrap's messages.
    pub job: Option<std::sync::mpsc::Receiver<JobMsg>>,
    /// How far the running re-wrap has come.
    pub progress: Option<Progress>,
    /// A change waiting for this device's check and acceptance (the prompt), its running check, and
    /// the words about it.
    pub check: Option<Check>,
    /// A check whose signed action verified but which came before the member list said who the
    /// owner is ([`CheckGate::Wait`]): taken again when the list comes, drawn nothing until then.
    pub waiting_check: Option<Check>,
    pub check_job: Option<std::sync::mpsc::Receiver<CheckMsg>>,
    pub check_words: String,
    /// The answer this device last sent to a check, until the relay acknowledges it (the third
    /// review's W5): a re-ask of the same change and round with no acknowledgement is answered
    /// again (the same decline, or the check run and signed afresh); once acknowledged, not.
    pub sent_answer: Option<SentAnswer>,
}

/// The keys refused per server ([`OwnerUi::refused`]): (server address, key).
#[derive(Debug, Default)]
pub struct RefusedKeys(std::collections::HashSet<(String, String)>);

impl RefusedKeys {
    /// Whether `key` is refused on `server`.
    pub fn contains(&self, server: &str, key: &str) -> bool {
        self.0.contains(&(server.to_string(), key.to_string()))
    }
    /// Refuse `key` on `server` until the app restarts.
    pub fn insert(&mut self, server: &str, key: &str) {
        self.0.insert((server.to_string(), key.to_string()));
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// An answer sent to a check ([`OwnerUi::sent_answer`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentAnswer {
    pub grant: String,
    pub round: String,
    /// An acceptance (else a decline).
    pub accepted: bool,
    /// The decline's words, given again as they were.
    pub why: String,
    /// The owner's own change (its outcome acknowledges it too).
    pub mine: bool,
    /// The relay answered it.
    pub acked: bool,
}

/// What the check thread tells the page.
#[derive(Debug)]
pub enum CheckMsg {
    /// Items checked so far.
    Progress(usize),
    /// The digest to sign, or why the copies did not check out.
    Done(Result<String, String>),
}

/// A change signed and sent.
#[derive(Debug, Clone)]
pub struct PendingChange {
    pub change: Change,
    /// The member it names (their key) and their name.
    pub target: String,
    pub target_name: String,
    /// The signed action's object id: the grant the headers are fetched and sent under.
    pub grant: String,
    pub sent_at_ms: i64,
    /// The relay answered ready and the job is running (or done, the finish sent).
    pub started: bool,
}

/// What the re-wrap thread tells the page.
#[derive(Debug)]
pub enum JobMsg {
    Progress(Progress),
    /// Every new header sent, ready for the finish; or the words why it stopped.
    Done(Result<(), String>),
}

#[cfg(test)]
#[path = "owner_tests.rs"]
pub(crate) mod tests;
