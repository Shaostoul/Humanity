//! The protected setup (step G of docs/design/blocking-and-safe-mode.md, section 10h,
//! 2026-10-10): the native client's rules, with no socket and no GUI so each one is unit tested.
//! The app-state glue (the PIN prompt, turning it on, the review step's Remove, what each locked
//! action does once the PIN is right) is src/engine/protected.rs; Settings > Safety's section and
//! the PIN prompt are src/gui/pages/safety_protected.rs; the chat page's lines are
//! src/gui/pages/chat/protected.rs. The web chat builds the same rules in
//! web/shared/protected.js; where 10h left a choice open, both apps made the same one.
//!
//! WHAT IT IS (10h, in its own words): a PIN lock on this device's safety settings, which a
//! parent or carer can apply for someone they look after, or anyone for themselves. It never
//! checks age, never asks the operating system for one (California Civil Code 1798.501(e); see
//! docs/reference/findings/2026-10-10-california-ab-1043-age-signals.md), never tells any server
//! anything, and is never described as making anything safe.
//!
//! THE WORDS: every word the feature puts on screen comes from data/gui/safety_presets.json
//! (`Preset`: its sentences and lines, and its `labels` for the buttons and prompts), never from
//! this code. A test holds all of them, and every string literal in the feature's files, to the
//! preset's `avoid_words`.
//!
//! THE PIN is kept only as a verifier, `PinVerifier`: PBKDF2-SHA-256 (`crate::config::pbkdf2_sha256`,
//! the vault's derivation) under a random 16-byte salt, at `PIN_ITERATIONS` (600,000, the PIN's
//! own constant) when made, and checked at whatever count it records. Neither the PIN nor
//! anything it could be read back from is stored.
//!
//! PER DEVICE: `ProtectedSetup` lives in its own file beside config.json (net/protected_store.rs,
//! written whole or not at all), so damage to config.json, or a config read that fails and falls
//! back to the defaults, can never turn it off. It is never in a frame: not in the self-sync
//! notes Block uses, not in the vault, not in any export. Turning it on sends one ordinary
//! `reach_set`, the same frame any adult choosing the same rows would send, so no server can tell
//! this device is protected. A missing file is off; one that cannot be read counts as ON with no
//! PIN that matches (fail closed): only the recovery phrase can then set a new PIN.

use serde::{Deserialize, Serialize};

use super::reach::{Audience, ReachKind, ReachSettings};

/// The presets file, under `data/`. Read by both clients; built into the desktop app as
/// `embedded_data::SAFETY_PRESETS_JSON` so an install whose data folder lacks it still has it.
pub const PRESETS_FILE: &str = "gui/safety_presets.json";

/// The id of the one preset this step builds (10h).
pub const PROTECTED_ID: &str = "protected";

/// What the preset's `pictures_from_non_friends` may say. `never`: not shown at all.
pub const PICTURES_NEVER: &str = "never";
/// What the preset's `public_rooms` may say. `read_only_only`: only read-only rooms listed.
pub const ROOMS_READ_ONLY_ONLY: &str = "read_only_only";

/// The preset's reach rows, as the file has them (the relay's audience words).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PresetReach {
    pub message: String,
    pub call: String,
    pub trade: String,
}

/// The preset's short labels: the buttons, field hints and prompt lines (the file's `labels`,
/// shared with the web chat). `pin_rule` holds `{min}` and `{max}`, `pin_wait` holds
/// `{seconds}`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Labels {
    #[serde(rename = "continue")]
    pub continue_: String,
    pub cancel: String,
    pub remove: String,
    pub forgot: String,
    pub pin: String,
    pub pin_again: String,
    pub pin_rule: String,
    pub pin_wrong: String,
    pub pin_wait: String,
    pub friends: String,
    pub groups: String,
    pub rooms: String,
    pub none: String,
    pub turn_off: String,
    pub change_pin: String,
    pub show_rooms: String,
    pub hide_rooms: String,
    pub pictures: String,
    pub pictures_never: String,
    pub pictures_click: String,
    pub phrase: String,
    pub phrase_wrong: String,
    pub waiting_store: String,
    pub not_connected: String,
    /// The locked Show button for the recovery phrase and the device-link QR (Settings), whose
    /// right PIN shows both: the QR carries the seed the phrase comes from.
    pub show_phrase: String,
    /// The line beside that locked button, saying why it needs the PIN.
    pub phrase_needs_pin: String,
}

impl Labels {
    /// Every label, for the words test and the empty-word check.
    pub fn all(&self) -> [&str; 26] {
        [
            &self.continue_, &self.cancel, &self.remove, &self.forgot, &self.pin, &self.pin_again, &self.pin_rule,
            &self.pin_wrong, &self.pin_wait, &self.friends, &self.groups, &self.rooms, &self.none, &self.turn_off,
            &self.change_pin, &self.show_rooms, &self.hide_rooms, &self.pictures, &self.pictures_never,
            &self.pictures_click, &self.phrase, &self.phrase_wrong, &self.waiting_store, &self.not_connected,
            &self.show_phrase, &self.phrase_needs_pin,
        ]
    }

    /// `pin_rule` with the preset's PIN length filled in.
    pub fn pin_rule(&self, min: usize, max: usize) -> String {
        self.pin_rule.replace("{min}", &min.to_string()).replace("{max}", &max.to_string())
    }

    /// `pin_wait` with the seconds left filled in.
    pub fn pin_wait(&self, seconds: u64) -> String {
        self.pin_wait.replace("{seconds}", &seconds.max(1).to_string())
    }
}

/// The `protected` preset, as data/gui/safety_presets.json has it: the settings it applies, the
/// PIN rules, and every word the feature shows (10h, "The preset is data").
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub button: String,
    pub summary: String,
    pub reach: PresetReach,
    pub warnings_on_friends: bool,
    pub pictures_from_non_friends: String,
    pub public_rooms: String,
    pub pin_digits_min: usize,
    pub pin_digits_max: usize,
    pub pin_wrong_tries_before_wait: u32,
    pub pin_wait_seconds: u64,
    pub sentences: Vec<String>,
    pub review_intro: String,
    pub review_keep: String,
    pub status_line: String,
    pub routes_line: String,
    pub public_rooms_hidden_line: String,
    pub public_rooms_explain: String,
    pub picture_hidden_line: String,
    pub accept_needs_pin: String,
    pub forgot_pin_explain: String,
    pub labels: Labels,
    pub avoid_words: Vec<String>,
}

impl Preset {
    /// The preset's "Who can reach me" rows. `parse_presets` refuses a file whose words this
    /// version does not know, so a loaded preset always has them.
    pub fn reach_settings(&self) -> Option<ReachSettings> {
        Some(ReachSettings {
            message: Audience::from_wire(&self.reach.message)?,
            call: Audience::from_wire(&self.reach.call)?,
            trade: Audience::from_wire(&self.reach.trade)?,
        })
    }

    /// The `reach_set` frame that applies the preset: all three rows, the ordinary frame
    /// (net/reach.rs `reach_set_frame`), with nothing added.
    pub fn reach_set_frame(&self) -> Option<serde_json::Value> {
        let s = self.reach_settings()?;
        Some(ReachSettings::reach_set_frame(&ReachKind::ALL.map(|k| (k, s.get(k)))))
    }

    /// Is `pin` the shape the preset asks for: digits only, `pin_digits_min` to `pin_digits_max`?
    pub fn pin_shape_ok(&self, pin: &str) -> bool {
        let n = pin.chars().count();
        n >= self.pin_digits_min && n <= self.pin_digits_max && pin.chars().all(|c| c.is_ascii_digit())
    }

    /// `labels.pin_rule` with this preset's PIN length.
    pub fn pin_rule(&self) -> String {
        self.labels.pin_rule(self.pin_digits_min, self.pin_digits_max)
    }

    /// Every string of the preset that goes on screen, for the words test.
    pub fn shown_words(&self) -> Vec<&str> {
        let mut out: Vec<&str> = vec![
            &self.name,
            &self.button,
            &self.summary,
            &self.review_intro,
            &self.review_keep,
            &self.status_line,
            &self.routes_line,
            &self.public_rooms_hidden_line,
            &self.public_rooms_explain,
            &self.picture_hidden_line,
            &self.accept_needs_pin,
            &self.forgot_pin_explain,
        ];
        out.extend(self.sentences.iter().map(String::as_str));
        out.extend(self.labels.all());
        out
    }
}

/// Read the presets file and return the `protected` preset. A file this version cannot use
/// (no such preset, an audience word or a rule it does not know, a missing or empty word) is an
/// error, so the app falls back to the copy built into the exe rather than applying a guess.
pub fn parse_presets(bytes: &[u8]) -> Result<Preset, String> {
    #[derive(Deserialize)]
    struct File {
        presets: Vec<serde_json::Value>,
    }
    let file: File = serde_json::from_slice(bytes).map_err(|e| format!("safety presets: {e}"))?;
    let raw = file
        .presets
        .into_iter()
        .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(PROTECTED_ID))
        .ok_or("safety presets: no \"protected\" preset")?;
    let preset: Preset = serde_json::from_value(raw).map_err(|e| format!("safety presets: protected: {e}"))?;
    if preset.reach_settings().is_none() {
        return Err("safety presets: protected: a reach word this version does not know".into());
    }
    if preset.pictures_from_non_friends != PICTURES_NEVER && preset.pictures_from_non_friends != "click" {
        return Err(format!("safety presets: protected: pictures_from_non_friends {:?} is not known", preset.pictures_from_non_friends));
    }
    if preset.public_rooms != ROOMS_READ_ONLY_ONLY && preset.public_rooms != "all" {
        return Err(format!("safety presets: protected: public_rooms {:?} is not known", preset.public_rooms));
    }
    if preset.pin_digits_min == 0 || preset.pin_digits_max < preset.pin_digits_min || preset.pin_wrong_tries_before_wait == 0 {
        return Err("safety presets: protected: the PIN rules do not make sense".into());
    }
    if preset.sentences.is_empty() || preset.avoid_words.is_empty() || preset.shown_words().iter().any(|w| w.trim().is_empty()) {
        return Err("safety presets: protected: a word is missing".into());
    }
    Ok(preset)
}

/// Load the protected preset from the data folder, or the copy built into the exe when the
/// folder's copy is missing or this version cannot read it.
pub fn load_preset(data_dir: &std::path::Path) -> Result<Preset, String> {
    crate::embedded_data::load_data_or_embedded(data_dir, PRESETS_FILE, parse_presets)
}

// ── The PIN ─────────────────────────────────────────────────────────────────────────────────

/// The iterations a NEW PIN verifier is made with: 600,000, 10h's number. Its own constant, not
/// the vault's (`config::PBKDF2_ITERATIONS_NEW`): a stored verifier carries the count it was made
/// with and is checked at that count, so if this number is raised one day, every PIN already
/// stored still opens. Tying it to the vault's constant, with the exact match `well_formed` once
/// asked for, meant any future change to the vault would have locked every stored PIN for good
/// (only the recovery phrase could then set a new one).
pub const PIN_ITERATIONS: u32 = 600_000;

/// The fewest iterations a STORED verifier may carry. Every verifier this app has ever made has
/// 600,000, so one with fewer did not come from it (a hand-edited file) and counts as no PIN.
pub const PIN_ITERATIONS_MIN: u32 = 600_000;

/// The most iterations a stored verifier may carry. A PIN is checked on the frame, so a damaged
/// count in the billions would freeze the app on every try; far above any count this app will
/// make, far below one that hangs it.
pub const PIN_ITERATIONS_MAX: u32 = 10_000_000;

/// What is stored for the PIN (10h): a random 16-byte salt and PBKDF2-SHA-256 of the PIN under
/// it, both base64, and the iteration count it was made with. Never the PIN.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinVerifier {
    pub salt: String,
    pub hash: String,
    pub iterations: u32,
}

impl PinVerifier {
    /// A verifier for `pin` under a fresh random salt, at `PIN_ITERATIONS`.
    pub fn new(pin: &str) -> Result<Self, String> {
        let mut salt = [0u8; 16];
        getrandom::getrandom(&mut salt).map_err(|e| format!("RNG failed: {e}"))?;
        Ok(Self::with_salt(pin, &salt, PIN_ITERATIONS))
    }

    /// A verifier for `pin` under `salt` at `iterations` (the tests use few iterations for speed,
    /// and one test holds `new` to `PIN_ITERATIONS`).
    pub fn with_salt(pin: &str, salt: &[u8], iterations: u32) -> Self {
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD;
        let hash = crate::config::pbkdf2_sha256(pin.as_bytes(), salt, iterations);
        Self { salt: b64.encode(salt), hash: b64.encode(hash), iterations }
    }

    /// Is this a verifier this app could have made: an iteration count from `PIN_ITERATIONS_MIN`
    /// to `PIN_ITERATIONS_MAX` (whatever `PIN_ITERATIONS` was when it was made), a 16-byte salt,
    /// a 32-byte hash? A stored one that is not counts as no PIN at all
    /// (`ProtectedSetup::from_stored`).
    pub fn well_formed(&self) -> bool {
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD;
        (PIN_ITERATIONS_MIN..=PIN_ITERATIONS_MAX).contains(&self.iterations)
            && b64.decode(&self.salt).is_ok_and(|s| s.len() == 16)
            && b64.decode(&self.hash).is_ok_and(|h| h.len() == 32)
    }

    /// Is `pin` the PIN this verifier was made from, at the iteration count it was made with?
    /// Compared in constant time. A count over `PIN_ITERATIONS_MAX` is refused before any work,
    /// so a damaged one can never freeze the app.
    pub fn matches(&self, pin: &str) -> bool {
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD;
        let (Ok(salt), Ok(want)) = (b64.decode(&self.salt), b64.decode(&self.hash)) else { return false };
        if want.len() != 32 || self.iterations == 0 || self.iterations > PIN_ITERATIONS_MAX || pin.is_empty() {
            return false;
        }
        let got = crate::config::pbkdf2_sha256(pin.as_bytes(), &salt, self.iterations);
        got.iter().zip(want.iter()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
    }
}

/// What one try at the PIN came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinTry {
    /// The PIN is right. The count of wrong tries starts again.
    Right,
    /// Wrong, and the next try may come at once.
    Wrong,
    /// Wrong, and that made `pin_wrong_tries_before_wait` in a row: the next try waits this many
    /// seconds (10h: "three wrong PINs in a row wait 60 seconds before the next try").
    WaitStarted(u64),
    /// Still waiting: this many seconds left. The PIN was not even looked at.
    Waiting(u64),
    /// The setup is off: nothing to try.
    NoPin,
}

/// The setup's state on this device, kept in its own file beside config.json
/// (net/protected_store.rs) and nowhere else. Its default is off, with nothing stored.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtectedSetup {
    /// Whether the protected setup is on.
    #[serde(default)]
    pub on: bool,
    /// The PIN's verifier, while it is on. None while on means no PIN matches (a damaged stored
    /// state): only the recovery phrase can set one.
    #[serde(default)]
    pub pin: Option<PinVerifier>,
    /// The identity key it was turned on under. "Forgot the PIN?" takes only THIS identity's
    /// recovery phrase, so making a new identity on the device (and so knowing its phrase) does
    /// not open the lock. Public, and on this device only.
    #[serde(default)]
    pub identity: String,
    /// The friends the PIN holder let be friends: those kept in the review step, and each made
    /// with the PIN since. While it is on, a pass is given only to them (engine/dm.rs
    /// `mint_pass`), so a follow made before the setup and followed back after it makes no
    /// friend without the PIN. Unfollow and Block take someone off.
    #[serde(default)]
    pub approved: Vec<String>,
    /// Warnings show under friends' messages too (the preset's `warnings_on_friends`).
    #[serde(default = "yes")]
    pub warnings_on_friends: bool,
    /// Pictures and files from non-friends are not shown at all (the preset's `never`). Turning
    /// this off, back to the click-to-load rule, needs the PIN.
    #[serde(default = "yes")]
    pub pictures_hidden: bool,
    /// Only read-only public rooms are listed (the preset's `read_only_only`). Showing the rest
    /// needs the PIN.
    #[serde(default = "yes")]
    pub public_rooms_hidden: bool,
    /// Wrong PINs in a row since the last right one or the last wait.
    #[serde(default)]
    pub wrong_tries: u32,
    /// Unix seconds before which no PIN is looked at (0 = no wait). Kept on disk, so closing the
    /// app does not end a wait.
    #[serde(default)]
    pub wait_until: u64,
}

/// The safe value of a rule a stored state does not say (fail closed).
fn yes() -> bool {
    true
}

impl ProtectedSetup {
    /// The setup as the preset turns it on, with `pin` as its PIN, under identity `identity`,
    /// with `approved` as the friends kept in the review step.
    pub fn applied(preset: &Preset, pin: PinVerifier, identity: &str, approved: Vec<String>) -> Self {
        Self {
            on: true,
            pin: Some(pin),
            identity: identity.to_string(),
            approved,
            warnings_on_friends: preset.warnings_on_friends,
            pictures_hidden: preset.pictures_from_non_friends == PICTURES_NEVER,
            public_rooms_hidden: preset.public_rooms == ROOMS_READ_ONLY_ONLY,
            wrong_tries: 0,
            wait_until: 0,
        }
    }

    /// The setup from what its file holds (net/protected_store.rs). Nothing there, or a setup
    /// that says it is off, is off. ANYTHING ELSE counts as ON (fail closed, as the web chat
    /// does): a damaged file must not be a way to turn it off.
    ///
    /// EACH FIELD IS READ ON ITS OWN (as the web's `protectedStateParse` does), so one damaged
    /// field never costs the others: a bad `approved` list does not throw away the PIN, and a bad
    /// `wait_until` does not throw away the approved friends. A field that does not read takes its
    /// safe value: the rules on, no friends approved, no wait counted, and for the PIN no PIN at
    /// all (a verifier that is not one this app makes, too), which only the recovery phrase can
    /// then replace. (Reading the whole struct at once, as this did before, let a single bad field
    /// reset every field, the PIN included.)
    pub fn from_stored(value: serde_json::Value) -> Self {
        if value.is_null() || value.get("on") == Some(&serde_json::Value::Bool(false)) {
            return Self::default();
        }
        let field = |name: &str| value.get(name).cloned().unwrap_or(serde_json::Value::Null);
        let rule = |name: &str| field(name).as_bool().unwrap_or(true);
        let pin = serde_json::from_value::<PinVerifier>(field("pin")).ok().filter(PinVerifier::well_formed);
        let approved = match field("approved") {
            serde_json::Value::Array(keys) => keys.iter().filter_map(|k| k.as_str()).map(str::trim).filter(|k| !k.is_empty()).map(str::to_string).collect(),
            _ => Vec::new(),
        };
        Self {
            on: true,
            pin,
            identity: field("identity").as_str().unwrap_or_default().to_string(),
            approved,
            warnings_on_friends: rule("warnings_on_friends"),
            pictures_hidden: rule("pictures_hidden"),
            public_rooms_hidden: rule("public_rooms_hidden"),
            wrong_tries: field("wrong_tries").as_u64().and_then(|n| u32::try_from(n).ok()).unwrap_or(0),
            wait_until: field("wait_until").as_u64().unwrap_or(0),
        }
    }

    /// One try at the PIN at `now` (Unix seconds), counting wrong tries in a row; the
    /// `max_wrong`-th wrong one in a row starts a `wait_secs` wait, and the count starts again
    /// after it. While waiting the PIN is not looked at. While on with no PIN (a damaged stored
    /// state) every try is wrong.
    pub fn try_pin(&mut self, pin: &str, now: u64, max_wrong: u32, wait_secs: u64) -> PinTry {
        if !self.on {
            return PinTry::NoPin;
        }
        if now < self.wait_until {
            return PinTry::Waiting(self.wait_until - now);
        }
        if self.pin.as_ref().is_some_and(|v| v.matches(pin)) {
            self.wrong_tries = 0;
            return PinTry::Right;
        }
        self.wrong_tries += 1;
        if self.wrong_tries >= max_wrong.max(1) {
            self.wrong_tries = 0;
            self.wait_until = now + wait_secs;
            return PinTry::WaitStarted(wait_secs);
        }
        PinTry::Wrong
    }

    /// Did the PIN holder let `key` be a friend?
    pub fn is_approved(&self, key: &str) -> bool {
        self.approved.iter().any(|k| k.eq_ignore_ascii_case(key))
    }

    /// May this device give `key` a pass? Always while it is off; while on, only someone the PIN
    /// holder let be a friend.
    pub fn pass_allowed(&self, key: &str) -> bool {
        !self.on || self.is_approved(key)
    }

    /// Is a public room listed? Read-only rooms always are; the rest only while the setup is off
    /// or they were shown with the PIN (10h).
    pub fn lists_room(&self, read_only: bool) -> bool {
        !self.on || !self.public_rooms_hidden || read_only
    }

    /// Are pictures and files from this sender left out altogether? Only while the setup is on
    /// with its pictures rule, and only for someone who is not a friend; a friend's still follow
    /// the click-to-load rule (10h).
    pub fn hides_pictures(&self, from_friend: bool) -> bool {
        self.on && self.pictures_hidden && !from_friend
    }

    /// Do friends' messages get the warnings for strangers as well as their own (10h)?
    pub fn warns_friends_as_strangers(&self) -> bool {
        self.on && self.warnings_on_friends
    }
}

// ── What needs the PIN ──────────────────────────────────────────────────────────────────────

/// Everything the setup has a rule for, locked or not (10h, "While it is on, these need the PIN"
/// and "Never locked"). Unblock is not here: it adds no one, and becoming friends again after it
/// is a Follow, which is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtectedAction {
    /// Changing a "Who can reach me" row.
    ReachRow(ReachKind, Audience),
    /// Changing a "People I choose" tick: the friend, the kind, ticked or not.
    Tick(String, ReachKind, bool),
    /// Follow or Follow back.
    Follow(String),
    /// Accepting a contact request.
    AcceptRequest(String),
    /// Sending a contact request: asking is following, and gives them a pass.
    SendRequest(String),
    /// Redeeming a friend code.
    RedeemFriendCode(String),
    /// Making a friend code to hand out: whoever redeems it starts a friendship with this device
    /// without asking (the web locks both halves under its one `friend_code`).
    MakeFriendCode,
    /// Joining a group by ticket.
    JoinGroup(String),
    /// Starting a group (the Create Group dialog's Create): a group is a way in that no friend
    /// list covers, and its first invite ticket is made with it.
    StartGroup,
    /// Making (and copying) an invite ticket for a group: the group's id and its name, which
    /// the ticket carries. A ticket lets in whoever holds it.
    InviteToGroup(String, String),
    /// Joining a voice room.
    JoinVoice(String),
    /// Turning "Warnings on messages" off.
    WarningsOff,
    /// Turning the pictures rule down to click-to-load.
    ShowPictures,
    /// Showing public rooms.
    ShowPublicRooms,
    /// Changing the PIN.
    ChangePin,
    /// Turning the setup off.
    TurnOff,
    /// Showing the recovery phrase in Settings. Not in 10h's list: added so the phrase that
    /// "Forgot the PIN?" takes cannot be read off this device by the person the setup protects
    /// (10h's own advice, "Keep the recovery phrase away from the person this protects").
    ShowRecoveryPhrase,
    // Never locked: anything that reduces who can reach this device. (Muting is never locked
    // either: the app's mute switches, a call's and a voice room's own microphone, do not pass
    // through the gate at all, so they need no row here.)
    Block(String),
    Report(String),
    Unfollow(String),
    LeaveGroup(String),
    LeaveRoom(String),
}

impl ProtectedAction {
    /// Does this need the PIN while the setup is on?
    pub fn needs_pin(&self) -> bool {
        use ProtectedAction::*;
        match self {
            ReachRow(..) | Tick(..) | Follow(_) | AcceptRequest(_) | SendRequest(_) | RedeemFriendCode(_) | MakeFriendCode
            | JoinGroup(_) | StartGroup | InviteToGroup(..) | JoinVoice(_) | WarningsOff | ShowPictures | ShowPublicRooms
            | ChangePin | TurnOff | ShowRecoveryPhrase => true,
            Block(_) | Report(_) | Unfollow(_) | LeaveGroup(_) | LeaveRoom(_) => false,
        }
    }

    /// The person this makes a friend of, for the actions that do (Follow, Follow back,
    /// accepting or sending a contact request): someone the PIN holder already let be a friend
    /// needs no PIN for it, and the PIN given for it lets them be one.
    pub fn befriends(&self) -> Option<&str> {
        match self {
            ProtectedAction::Follow(k) | ProtectedAction::AcceptRequest(k) | ProtectedAction::SendRequest(k) => Some(k),
            _ => None,
        }
    }
}

/// A typed chat command that is a locked action: `/redeem <code>` makes a friend, and
/// `/friend-code` makes a code that lets someone else start one. None for anything else (an
/// ordinary message, or a command the setup has no rule for).
pub fn typed_command(text: &str) -> Option<ProtectedAction> {
    let mut words = text.trim().split_whitespace();
    let first = words.next()?;
    if first.eq_ignore_ascii_case("/redeem") {
        return Some(ProtectedAction::RedeemFriendCode(words.collect::<Vec<_>>().join(" ")));
    }
    if first.eq_ignore_ascii_case("/friend-code") {
        return Some(ProtectedAction::MakeFriendCode);
    }
    None
}

// ── Files a message links to ────────────────────────────────────────────────────────────────

/// The kinds of file a link names that the web chat turns into a player or a file card
/// (web/chat/app.js `formatBody`: audio, video, then documents and archives), and so hides from
/// non-friends with pictures while the setup is on (web/shared/protected.js
/// `protectedHidePicturesHtml`). Pictures are the image cache's own list.
pub const FILE_LINK_EXTS: &[&str] = &["mp3", "ogg", "wav", "mp4", "webm", "pdf", "txt", "md", "json", "zip", "tar.gz", "gz"];

/// The links in `text` to a file of one of `FILE_LINK_EXTS`: a web address (`http://` or
/// `https://`) or a server path (`/uploads/`) whose path ends in one, with any query after it, as
/// the web's patterns match them. Trailing punctuation is not part of a link.
pub fn file_urls(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for token in text.split_whitespace() {
        let lower = token.to_ascii_lowercase();
        let Some(start) = ["https://", "http://", "/uploads/"].iter().filter_map(|p| lower.find(p)).min() else { continue };
        let link = token[start..].trim_end_matches(|c: char| ".,;:!?)]}'\"".contains(c));
        let path = link.split(['?', '#']).next().unwrap_or("").to_ascii_lowercase();
        let named = FILE_LINK_EXTS.iter().any(|ext| path.ends_with(&format!(".{ext}")));
        if named && !out.iter().any(|u| u == link) {
            out.push(link.to_string());
        }
    }
    out
}

/// `text` without the links `file_urls` finds, the blank space they leave tidied.
pub fn strip_file_urls(text: &str) -> String {
    let mut out = text.to_string();
    for link in file_urls(text) {
        out = out.replace(&link, "");
    }
    let lines: Vec<String> = out.lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|l| !l.is_empty()).collect();
    lines.join("\n")
}

// ── The review step ─────────────────────────────────────────────────────────────────────────

/// One row of the review step (10h, "Review who can already reach this device"): a friend
/// (everyone holding a pass from us, and every mutual follow still owed one), a group we are in,
/// or a voice room we joined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewItem {
    Friend { key: String, name: String },
    Group { id: String, name: String },
    Room { id: String, name: String },
}

impl ReviewItem {
    /// What its Remove does: for a friend, Unfollow, which withdraws their pass; for a group or a
    /// room, Leave. Each is a never-locked action.
    pub fn removal(&self) -> ProtectedAction {
        match self {
            ReviewItem::Friend { key, .. } => ProtectedAction::Unfollow(key.clone()),
            ReviewItem::Group { id, .. } => ProtectedAction::LeaveGroup(id.clone()),
            ReviewItem::Room { id, .. } => ProtectedAction::LeaveRoom(id.clone()),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            ReviewItem::Friend { name, .. } | ReviewItem::Group { name, .. } | ReviewItem::Room { name, .. } => name,
        }
    }
}

// ── "Forgot the PIN?" ───────────────────────────────────────────────────────────────────────

/// The words of a typed recovery phrase, normalised the way the warnings are (lower-cased, every
/// run of non-letters and non-digits one space), with number-only words left out so a phrase
/// pasted as a numbered list ("1. word 2. word") reads the same as one typed plainly.
pub fn phrase_words(text: &str) -> Vec<String> {
    super::warnings::normalize(text)
        .split(' ')
        .filter(|w| !w.is_empty() && !w.chars().all(char::is_numeric))
        .map(str::to_string)
        .collect()
}

/// Does `typed` hold exactly the words of `derived`, in order? (10h: "when it matches the
/// phrase this device derives".) Nothing typed never matches.
pub fn phrase_matches(typed: &str, derived: &str) -> bool {
    let t = phrase_words(typed);
    !t.is_empty() && t == phrase_words(derived)
}

// ── The words test ──────────────────────────────────────────────────────────────────────────

/// The first of `avoid` that `text` holds, compared lower-cased, or None. The preset's
/// `avoid_words` are the finding's "Words and claims to avoid" (10h).
pub fn avoided_word<'a>(text: &str, avoid: &'a [String]) -> Option<&'a str> {
    let lower = text.to_lowercase();
    avoid.iter().map(String::as_str).find(|w| lower.contains(&w.to_lowercase()))
}

// ── The app's state for it ──────────────────────────────────────────────────────────────────

/// Where turning it on has got to (10h, "Turning it on, in order").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupStep {
    /// 1. The sentences, and Continue.
    Read,
    /// 2. Choose a PIN, twice.
    ChoosePin,
    /// 3. Review who can already reach this device; "Keep the rest" applies (4).
    Review,
}

/// What the PIN prompt is asking for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptMode {
    /// The PIN, for the action waiting.
    Pin,
    /// "Forgot the PIN?": this identity's recovery phrase.
    Forgot,
    /// A new PIN, twice (after the phrase, or after "Change the PIN" and the old PIN).
    NewPin,
}

/// The PIN prompt (a small window over every page): the locked action waiting for the PIN, if
/// any, and what it is asking for now. After "Forgot the PIN?" and a new PIN, it goes back to
/// asking for the PIN (the new one) before the waiting action runs, as the web chat does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinPrompt {
    pub action: Option<ProtectedAction>,
    pub mode: PromptMode,
}

/// Everything the app keeps for the setup: `setup` (saved in its own file) and what lives only
/// while the app runs (the steps, the PIN prompt, a one-shot permission). PINs and phrases typed
/// are held only until they are checked or made into a verifier, then cleared.
#[derive(Debug, Default)]
pub struct ProtectedUi {
    pub setup: ProtectedSetup,
    /// The preset, loaded the first time it is needed.
    pub preset: Option<Preset>,
    /// The preset could not be read (neither the data folder's copy nor the one built in). Only
    /// a flag: the reason goes to the log, never to the screen, where a parser's message would be
    /// words the preset did not choose (10h, "Words").
    pub preset_failed: bool,
    /// Turning it on: the step reached, or None.
    pub step: Option<SetupStep>,
    /// The two PIN fields of a "choose a PIN" step (turning on, or a new PIN in the prompt).
    pub pin_first: String,
    pub pin_second: String,
    /// The verifier made in step 2, kept until step 4 applies it.
    pub chosen: Option<PinVerifier>,
    /// The PIN prompt, while open.
    pub prompt: Option<PinPrompt>,
    /// The PIN typed into the prompt, and the recovery phrase typed after "Forgot the PIN?".
    pub prompt_input: String,
    pub forgot_phrase: String,
    /// The prompt's last answer (a wrong PIN, the wait, a wrong phrase, the PIN rule).
    pub prompt_line: String,
    /// One locked action let through by a right PIN, used up by the next check for that same
    /// action (engine/protected.rs `allows`).
    pub granted: Option<ProtectedAction>,
    /// The recovery phrase may be shown in Settings until the app closes (the PIN was entered
    /// for it).
    pub phrase_shown: bool,
    /// A friend code was redeemed with the PIN, and the server has not yet said whose it was. The
    /// PIN given for the code covers the friend it names, so its answer (engine/friend_code.rs)
    /// follows them without asking again; used up by that answer.
    pub code_redeemed: bool,
    /// Text a locked action made after its PIN that belongs on the clipboard (a group's invite
    /// ticket). The PIN's answer runs without the egui context that copies, so it waits here and
    /// the PIN prompt's drawing, which has the context, copies it on the next frame.
    pub copy_out: Option<String>,
    /// A one-line result for Settings > Safety's section.
    pub line: String,
}

impl ProtectedUi {
    pub fn is_on(&self) -> bool {
        self.setup.on
    }

    /// The action the PIN prompt is open for, if any.
    pub fn waiting_action(&self) -> Option<&ProtectedAction> {
        self.prompt.as_ref().and_then(|p| p.action.as_ref())
    }
}

#[cfg(test)]
#[path = "protected_tests.rs"]
mod tests;
