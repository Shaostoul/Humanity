//! Organisations (rung 1, 2026-11-14, docs/design/organisations.md): a group with members, roles
//! and permissions, invitations the invitee accepts, a directory with privacy levels and an audit
//! log every member can read.
//!
//! An organisation is NOT a server row. It is a founding object (`org_v1`) and an append-only log
//! of signed entries (`org_entry_v1`), each a canonical-CBOR [`Object`] signed by its author's
//! Dilithium3 / ML-DSA-65 key (docs/reference/cryptography.md, "Organisations"; no new
//! algorithm). Every app computes the current state the same way, [`fold`], so membership and
//! roles are proven by entries the members hold and any app checks for itself (the certificate
//! pattern, docs/ai/storage.md), never by a table a server keeps. The relay refuses `org_` objects
//! outright (`put_signed_object`, src/relay/storage/signed_objects.rs), so a log is never stored in the clear by mistake.
//!
//! The website's twin is `web/shared/org-log.js`; both fold the frozen log
//! scripts/tests/fixtures/org-log-v1.json (built by the website's code) to the state recorded beside it.

use std::collections::{BTreeMap, BTreeSet};

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ciborium::Value;

use crate::did::did_for_pubkey;
use crate::encoding::{cbor_bytes, cbor_int, cbor_map, cbor_text, from_canonical_bytes};
use crate::object::{Object, ObjectBuilder};
use crate::pq_crypto::DilithiumKeypair;

/// The founding object's type.
pub const FOUNDING_TYPE: &str = "org_v1";
/// One change's type.
pub const ENTRY_TYPE: &str = "org_entry_v1";
/// The carried text's version (`{"hum_org": 1, ...}`).
pub const BUNDLE_VERSION: u64 = 1;
/// Most entries one log may hold in rung 1 (bounds the fold; rung 7's checkpoints lift it).
pub const MAX_ENTRIES: usize = 10_000;
/// An entry dated further ahead of the reader's clock than this waits until it is not.
pub const FUTURE_SLACK_MS: u64 = 5 * 60 * 1000;
/// The owner's role id. Exactly one member holds it (rung 2 adds handing it on).
pub const OWNER: &str = "owner";
/// The role every organisation has for people with no other.
pub const MEMBER: &str = "member";
/// The admin role every organisation starts with.
pub const ADMIN: &str = "admin";
/// The ranks run 0 to this; the owner holds it.
pub const TOP_RANK: u64 = 100;

/// Text limits, in characters.
pub const NAME_MAX: usize = 80;
pub const ABOUT_MAX: usize = 500;
pub const ROLE_NAME_MAX: usize = 40;
pub const ROLE_ID_MAX: usize = 32;
pub const LISTING_NAME_MAX: usize = 60;
pub const PERMS_MAX: usize = 64;
pub const ROLES_MAX: usize = 64;

/// The permissions the fold enforces since rung 1. Others (calendar.edit, ...) are carried in
/// roles and enforced when their rung lands (data/orgs/roles.json lists them all).
pub mod perm {
    pub const INVITE: &str = "members.invite";
    pub const REMOVE: &str = "members.remove";
    pub const ASSIGN: &str = "roles.assign";
    pub const DEFINE: &str = "roles.define";
    pub const SETTINGS: &str = "org.settings";
    pub const SEE_ALL: &str = "directory.see_all";
    /// Every permission the rung 1 fold enforces.
    pub const ENFORCED: [&str; 6] = [INVITE, REMOVE, ASSIGN, DEFINE, SETTINGS, SEE_ALL];
}

/// One role: who may do what, and how high it ranks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Role {
    /// Lowercase letters, digits and `_`, 1 to 32.
    pub id: String,
    /// The organisation's own word for it; empty for owner, admin and member (the apps show their
    /// own words for those, by key).
    pub name: String,
    /// 0 to 100. You act only on people and roles ranked below you.
    pub rank: u64,
    /// The permissions it grants, exactly these.
    pub perms: Vec<String>,
}

impl Role {
    /// Whether this role grants `p`.
    pub fn grants(&self, p: &str) -> bool {
        self.perms.iter().any(|x| x == p)
    }
    /// Whether this is one of the three every organisation has.
    pub fn is_built_in(&self) -> bool {
        is_built_in(&self.id)
    }
}

/// Whether `id` names owner, admin or member.
pub fn is_built_in(id: &str) -> bool {
    id == OWNER || id == ADMIN || id == MEMBER
}

/// Who may read the member directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Directory {
    /// Members only (the default).
    Members,
    /// Only those with `directory.see_all`.
    Organisers,
    /// Anyone the log is shared with.
    Public,
}

impl Directory {
    pub fn as_str(self) -> &'static str {
        match self {
            Directory::Members => "members",
            Directory::Organisers => "organisers",
            Directory::Public => "public",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "members" => Some(Directory::Members),
            "organisers" => Some(Directory::Organisers),
            "public" => Some(Directory::Public),
            _ => None,
        }
    }
}

/// How one member appears in the directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Listing {
    /// Named to everyone who can read the directory (the default).
    Members,
    /// Named only to organisers; counted, not named, to everyone else.
    Organisers,
}

impl Listing {
    pub fn as_str(self) -> &'static str {
        match self {
            Listing::Members => "members",
            Listing::Organisers => "organisers",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "members" => Some(Listing::Members),
            "organisers" => Some(Listing::Organisers),
            _ => None,
        }
    }
}

/// What the founding object says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Founding {
    pub name: String,
    pub about: String,
    pub directory: Directory,
    pub roles: Vec<Role>,
}

/// One change, as an entry's payload says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    Invite { who: Vec<u8>, role: String, until: u64 },
    Withdraw { invite: String },
    Accept { invite: String },
    Remove { who: Vec<u8> },
    Leave,
    Assign { who: Vec<u8>, role: String },
    RoleSet { role: Role },
    RoleDrop { role: String },
    Listing { name: String, level: Listing },
    Settings { name: Option<String>, about: Option<String>, directory: Option<Directory> },
}

impl Act {
    /// The `act` word on the wire, which is also the end of its i18n key (`orgs.act.<word>`).
    pub fn word(&self) -> &'static str {
        match self {
            Act::Invite { .. } => "invite",
            Act::Withdraw { .. } => "withdraw",
            Act::Accept { .. } => "accept",
            Act::Remove { .. } => "remove",
            Act::Leave => "leave",
            Act::Assign { .. } => "assign",
            Act::RoleSet { .. } => "role_set",
            Act::RoleDrop { .. } => "role_drop",
            Act::Listing { .. } => "listing",
            Act::Settings { .. } => "settings",
        }
    }
}

// ── Checking text ──

/// Whether `s` holds no control character (a line break included) and at most `max` characters.
fn plain(s: &str, max: usize) -> bool {
    s.chars().count() <= max && !s.chars().any(|c| c.is_control())
}

fn role_id_ok(id: &str) -> bool {
    !id.is_empty() && id.len() <= ROLE_ID_MAX && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

fn perm_id_ok(p: &str) -> bool {
    !p.is_empty() && p.len() <= 48 && p.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'.')
}

/// Why a role map is not acceptable, or `None`.
pub fn role_problem(r: &Role) -> Option<&'static str> {
    if !role_id_ok(&r.id) {
        return Some("role_id");
    }
    if !plain(&r.name, ROLE_NAME_MAX) {
        return Some("role_name");
    }
    if r.rank > TOP_RANK {
        return Some("role_rank");
    }
    if r.perms.len() > PERMS_MAX || r.perms.iter().any(|p| !perm_id_ok(p)) {
        return Some("role_perms");
    }
    let unique: BTreeSet<&String> = r.perms.iter().collect();
    if unique.len() != r.perms.len() {
        return Some("role_perms");
    }
    None
}

/// Why a founding is not acceptable, or `None`: a name, text within limits, unique valid roles,
/// owner at the top rank alone, member present.
pub fn founding_problem(f: &Founding) -> Option<&'static str> {
    if f.name.trim().is_empty() || !plain(&f.name, NAME_MAX) {
        return Some("name");
    }
    if !plain(&f.about, ABOUT_MAX) {
        return Some("about");
    }
    if f.roles.is_empty() || f.roles.len() > ROLES_MAX {
        return Some("roles");
    }
    let mut ids = BTreeSet::new();
    for r in &f.roles {
        if let Some(p) = role_problem(r) {
            return Some(p);
        }
        if !ids.insert(r.id.as_str()) {
            return Some("roles");
        }
        if r.id != OWNER && r.rank >= TOP_RANK {
            return Some("role_rank");
        }
    }
    match f.roles.iter().find(|r| r.id == OWNER) {
        Some(o) if o.rank == TOP_RANK => {}
        _ => return Some("owner"),
    }
    if !ids.contains(MEMBER) || !ids.contains(ADMIN) {
        return Some("roles");
    }
    None
}

// ── Roles as data (data/orgs/roles.json) ──

/// The roles a new organisation starts with and every permission's id, read from the text of
/// `data/orgs/roles.json` (the app reads the file; the tests read the shipped one).
#[derive(Debug, Clone)]
pub struct RoleBook {
    /// The starting roles, built-in names left empty.
    pub roles: Vec<Role>,
    /// Every permission id the file lists, in its order.
    pub permissions: Vec<String>,
}

/// Read a role book from the JSON text of `data/orgs/roles.json`.
pub fn role_book_from_json(text: &str) -> Result<RoleBook, String> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("roles.json: {e}"))?;
    let permissions: Vec<String> = v["permissions"]
        .as_array()
        .ok_or("roles.json: no permissions")?
        .iter()
        .filter_map(|p| p["id"].as_str().map(str::to_string))
        .collect();
    let mut roles = Vec::new();
    for r in v["roles"].as_array().ok_or("roles.json: no roles")? {
        let role = Role {
            id: r["id"].as_str().ok_or("roles.json: a role without an id")?.to_string(),
            name: String::new(),
            rank: r["rank"].as_u64().ok_or("roles.json: a role without a rank")?,
            perms: r["perms"]
                .as_array()
                .ok_or("roles.json: a role without perms")?
                .iter()
                .filter_map(|p| p.as_str().map(str::to_string))
                .collect(),
        };
        if let Some(bad) = role.perms.iter().find(|p| !permissions.contains(p)) {
            return Err(format!("roles.json: role {} names {bad}, which is not in permissions", role.id));
        }
        roles.push(role);
    }
    Ok(RoleBook { roles, permissions })
}

// ── Encoding ──

fn role_value(r: &Role) -> Value {
    cbor_map(vec![
        ("id", cbor_text(&r.id)),
        ("name", cbor_text(&r.name)),
        ("perms", Value::Array(r.perms.iter().map(|p| cbor_text(p)).collect())),
        ("rank", cbor_int(r.rank)),
    ])
}

fn founding_value(f: &Founding) -> Value {
    cbor_map(vec![
        ("about", cbor_text(&f.about)),
        ("directory", cbor_text(f.directory.as_str())),
        ("name", cbor_text(&f.name)),
        ("roles", Value::Array(f.roles.iter().map(role_value).collect())),
    ])
}

fn act_value(a: &Act) -> Value {
    let mut e: Vec<(&str, Value)> = vec![("act", cbor_text(a.word()))];
    match a {
        Act::Invite { who, role, until } => {
            e.push(("who", cbor_bytes(who)));
            e.push(("role", cbor_text(role)));
            e.push(("until", cbor_int(*until)));
        }
        Act::Withdraw { invite } | Act::Accept { invite } => e.push(("invite", cbor_text(invite))),
        Act::Remove { who } => e.push(("who", cbor_bytes(who))),
        Act::Leave => {}
        Act::Assign { who, role } => {
            e.push(("who", cbor_bytes(who)));
            e.push(("role", cbor_text(role)));
        }
        Act::RoleSet { role } => e.push(("role", role_value(role))),
        Act::RoleDrop { role } => e.push(("role", cbor_text(role))),
        Act::Listing { name, level } => {
            e.push(("name", cbor_text(name)));
            e.push(("level", cbor_text(level.as_str())));
        }
        Act::Settings { name, about, directory } => {
            if let Some(n) = name {
                e.push(("name", cbor_text(n)));
            }
            if let Some(a) = about {
                e.push(("about", cbor_text(a)));
            }
            if let Some(d) = directory {
                e.push(("directory", cbor_text(d.as_str())));
            }
        }
    }
    cbor_map(e)
}

/// Sign the founding of a new organisation. Its `object_id` is the organisation's id.
pub fn build_founding(kp: &DilithiumKeypair, f: &Founding, now_ms: u64) -> Result<Object, String> {
    if let Some(p) = founding_problem(f) {
        return Err(format!("founding: {p}"));
    }
    ObjectBuilder::new(FOUNDING_TYPE)
        .created_at(now_ms)
        .payload_cbor(&founding_value(f))
        .and_then(|b| b.sign(kp))
        .map_err(|e| e.to_string())
}

/// Sign one change to organisation `org_id`.
pub fn build_entry(kp: &DilithiumKeypair, org_id: &str, act: &Act, now_ms: u64) -> Result<Object, String> {
    ObjectBuilder::new(ENTRY_TYPE)
        .created_at(now_ms)
        .reference(org_id)
        .payload_cbor(&act_value(act))
        .and_then(|b| b.sign(kp))
        .map_err(|e| e.to_string())
}

// ── Decoding ──

fn map_get<'a>(m: &'a [(Value, Value)], k: &str) -> Option<&'a Value> {
    m.iter().find(|(key, _)| key.as_text() == Some(k)).map(|(_, v)| v)
}
fn text_of(m: &[(Value, Value)], k: &str) -> Option<String> {
    map_get(m, k).and_then(|v| v.as_text()).map(str::to_string)
}
fn bytes_of(m: &[(Value, Value)], k: &str) -> Option<Vec<u8>> {
    map_get(m, k).and_then(|v| v.as_bytes()).cloned()
}
fn uint_of(m: &[(Value, Value)], k: &str) -> Option<u64> {
    map_get(m, k).and_then(|v| v.as_integer()).and_then(|i| u64::try_from(i).ok())
}

fn role_from(v: &Value) -> Option<Role> {
    let m = v.as_map()?;
    let perms = map_get(m, "perms")?.as_array()?.iter().map(|p| p.as_text().map(str::to_string)).collect::<Option<Vec<_>>>()?;
    let role = Role { id: text_of(m, "id")?, name: text_of(m, "name")?, rank: uint_of(m, "rank")?, perms };
    if role_problem(&role).is_some() {
        return None;
    }
    Some(role)
}

fn payload_map(obj: &Object) -> Option<Vec<(Value, Value)>> {
    match from_canonical_bytes(&obj.payload).ok()? {
        Value::Map(m) => Some(m),
        _ => None,
    }
}

/// What a founding object says, or `None` when it is not a valid founding.
pub fn parse_founding(obj: &Object) -> Option<Founding> {
    if obj.object_type != FOUNDING_TYPE {
        return None;
    }
    let m = payload_map(obj)?;
    let roles = map_get(&m, "roles")?.as_array()?.iter().map(role_from).collect::<Option<Vec<_>>>()?;
    let f = Founding {
        name: text_of(&m, "name")?,
        about: text_of(&m, "about")?,
        directory: Directory::parse(&text_of(&m, "directory")?)?,
        roles,
    };
    if founding_problem(&f).is_some() {
        return None;
    }
    Some(f)
}

/// What an entry's payload says, or `None` when it is malformed.
pub fn parse_act(obj: &Object) -> Option<Act> {
    let m = payload_map(obj)?;
    let act = match text_of(&m, "act")?.as_str() {
        "invite" => Act::Invite { who: bytes_of(&m, "who")?, role: text_of(&m, "role")?, until: uint_of(&m, "until")? },
        "withdraw" => Act::Withdraw { invite: text_of(&m, "invite")? },
        "accept" => Act::Accept { invite: text_of(&m, "invite")? },
        "remove" => Act::Remove { who: bytes_of(&m, "who")? },
        "leave" => Act::Leave,
        "assign" => Act::Assign { who: bytes_of(&m, "who")?, role: text_of(&m, "role")? },
        "role_set" => Act::RoleSet { role: role_from(map_get(&m, "role")?)? },
        "role_drop" => Act::RoleDrop { role: text_of(&m, "role")? },
        "listing" => {
            let name = text_of(&m, "name")?;
            if !plain(&name, LISTING_NAME_MAX) {
                return None;
            }
            Act::Listing { name, level: Listing::parse(&text_of(&m, "level")?)? }
        }
        "settings" => {
            let name = text_of(&m, "name");
            let about = text_of(&m, "about");
            let directory = match text_of(&m, "directory") {
                Some(d) => Some(Directory::parse(&d)?),
                None => None,
            };
            if name.as_deref().is_some_and(|n| n.trim().is_empty() || !plain(n, NAME_MAX)) || about.as_deref().is_some_and(|a| !plain(a, ABOUT_MAX)) {
                return None;
            }
            if name.is_none() && about.is_none() && directory.is_none() {
                return None;
            }
            Act::Settings { name, about, directory }
        }
        _ => return None,
    };
    Some(act)
}

// ── The carried text and the file (one JSON shape) ──

/// One signed object as the JSON `POST /api/v2/objects` takes (`SignedObjectSubmission`).
pub fn object_to_json(o: &Object) -> serde_json::Value {
    let mut v = serde_json::json!({
        "protocol_version": o.protocol_version,
        "object_type": o.object_type,
        "author_public_key_b64": B64.encode(&o.author_public_key),
        "references": o.references,
        "payload_schema_version": o.payload_schema_version,
        "payload_encoding": o.payload_encoding,
        "payload_b64": B64.encode(&o.payload),
        "signature_b64": B64.encode(&o.signature),
    });
    if let Some(t) = o.created_at {
        v["created_at"] = serde_json::json!(t);
    }
    if let Some(s) = &o.space_id {
        v["space_id"] = serde_json::json!(s);
    }
    if let Some(c) = &o.channel_id {
        v["channel_id"] = serde_json::json!(c);
    }
    v
}

/// The object a submission-shaped JSON value holds.
pub fn object_from_json(v: &serde_json::Value) -> Result<Object, String> {
    let s = |k: &str| v[k].as_str().map(str::to_string).ok_or_else(|| format!("object: no {k}"));
    let b = |k: &str| -> Result<Vec<u8>, String> { B64.decode(s(k)?).map_err(|e| format!("object: {k} is not base64: {e}")) };
    Ok(Object {
        protocol_version: v["protocol_version"].as_u64().ok_or("object: no protocol_version")?,
        object_type: s("object_type")?,
        space_id: v["space_id"].as_str().map(str::to_string),
        channel_id: v["channel_id"].as_str().map(str::to_string),
        author_public_key: b("author_public_key_b64")?,
        created_at: v["created_at"].as_u64(),
        references: v["references"]
            .as_array()
            .ok_or("object: no references")?
            .iter()
            .map(|r| r.as_str().map(str::to_string).ok_or_else(|| "object: a reference is not text".to_string()))
            .collect::<Result<_, _>>()?,
        payload_schema_version: v["payload_schema_version"].as_u64().ok_or("object: no payload_schema_version")?,
        payload_encoding: s("payload_encoding")?,
        payload: b("payload_b64")?,
        signature: b("signature_b64")?,
    })
}

/// The text a person carries (and the file a log is kept in):
/// `{"hum_org": 1, "org": "<id>", "objects": [...]}`.
pub fn bundle_text(org_id: &str, objects: &[Object]) -> String {
    let objs: Vec<serde_json::Value> = objects.iter().map(object_to_json).collect();
    serde_json::json!({ "hum_org": BUNDLE_VERSION, "org": org_id, "objects": objs }).to_string()
}

/// The organisation id and objects a carried text holds. Surrounding words (a message it was
/// pasted from) are ignored: the text from the first `{` to the last `}` is read.
pub fn parse_bundle(text: &str) -> Result<(String, Vec<Object>), String> {
    let (Some(a), Some(z)) = (text.find('{'), text.rfind('}')) else {
        return Err("no_bundle".into());
    };
    if z < a {
        return Err("no_bundle".into());
    }
    let v: serde_json::Value = serde_json::from_str(&text[a..=z]).map_err(|_| "no_bundle".to_string())?;
    if v["hum_org"].as_u64() != Some(BUNDLE_VERSION) {
        return Err("bundle_version".into());
    }
    let org = v["org"].as_str().ok_or("no_bundle")?.to_string();
    if org.len() != 64 || !org.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("no_bundle".into());
    }
    let arr = v["objects"].as_array().ok_or("no_bundle")?;
    if arr.len() > MAX_ENTRIES + 1 {
        return Err("too_many".into());
    }
    let objects = arr.iter().map(object_from_json).collect::<Result<Vec<_>, _>>()?;
    Ok((org, objects))
}

/// Merge `incoming` into `log` by `object_id` (opening the same text twice changes nothing; texts
/// may arrive in any order). Keeps only this organisation's founding and entries. Returns how many
/// were new.
pub fn merge(org_id: &str, log: &mut Vec<Object>, incoming: Vec<Object>) -> usize {
    let mut have: BTreeSet<String> = log.iter().filter_map(|o| o.object_id().ok().map(|h| h.to_hex())).collect();
    let mut added = 0;
    for o in incoming {
        let belongs = match o.object_type.as_str() {
            FOUNDING_TYPE => true,
            ENTRY_TYPE => o.references.first().map(String::as_str) == Some(org_id),
            _ => false,
        };
        if !belongs || log.len() > MAX_ENTRIES {
            continue;
        }
        let Ok(id) = o.object_id().map(|h| h.to_hex()) else { continue };
        if o.object_type == FOUNDING_TYPE && id != org_id {
            continue;
        }
        if have.insert(id) {
            log.push(o);
            added += 1;
        }
    }
    added
}

// ── The fold ──

/// Why an entry was not applied. Each word is the end of an i18n key (`orgs.refused.<word>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    Malformed,
    BeforeFounding,
    NotAMember,
    LacksPermission,
    RankTooLow,
    NoSuchRole,
    AlreadyMember,
    NoSuchInvite,
    NotTheInvitee,
    InviteUsed,
    InviteWithdrawn,
    InviteExpired,
    InviterCannot,
    RemovedSince,
    OwnerStays,
    BuiltInRole,
}

impl Refusal {
    pub fn word(self) -> &'static str {
        match self {
            Refusal::Malformed => "malformed",
            Refusal::BeforeFounding => "before_founding",
            Refusal::NotAMember => "not_a_member",
            Refusal::LacksPermission => "lacks_permission",
            Refusal::RankTooLow => "rank_too_low",
            Refusal::NoSuchRole => "no_such_role",
            Refusal::AlreadyMember => "already_member",
            Refusal::NoSuchInvite => "no_such_invite",
            Refusal::NotTheInvitee => "not_the_invitee",
            Refusal::InviteUsed => "invite_used",
            Refusal::InviteWithdrawn => "invite_withdrawn",
            Refusal::InviteExpired => "invite_expired",
            Refusal::InviterCannot => "inviter_cannot",
            Refusal::RemovedSince => "removed_since",
            Refusal::OwnerStays => "owner_stays",
            Refusal::BuiltInRole => "built_in_role",
        }
    }
}

/// One member, as the fold leaves them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub key: Vec<u8>,
    /// `did:hum:...` of the key: the member's id everywhere.
    pub did: String,
    pub role: String,
    /// The name they gave this organisation (`listing`), if any.
    pub name: Option<String>,
    pub listing: Listing,
    pub joined_at: u64,
    /// The entry ids that gave this member their standing (invite, acceptance, role changes, the
    /// definition of their role), and theirs in turn back to the founder: what proves it.
    pub standing: BTreeSet<String>,
}

/// An invitation not yet accepted, withdrawn or void.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invite {
    pub id: String,
    pub who: Vec<u8>,
    pub who_did: String,
    pub role: String,
    pub until: u64,
    pub by: Vec<u8>,
    pub at: u64,
    /// What proves the inviter could invite (their standing when they did).
    pub support: BTreeSet<String>,
    pub used: bool,
    pub withdrawn: bool,
}

/// One line of the audit log: an entry whose signature verified, applied or not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    pub entry_id: String,
    pub at: u64,
    /// The signer: the entry's own author key, whose signature verified.
    pub actor: Vec<u8>,
    /// The act, when the payload parsed.
    pub act: Option<Act>,
    /// The person the act was about (invite, remove, assign: `who`; accept and leave: the actor).
    pub subject: Option<Vec<u8>>,
    pub refused: Option<Refusal>,
}

/// The current state of an organisation: the fold of its log.
#[derive(Debug, Clone)]
pub struct OrgState {
    pub org_id: String,
    pub founded_at: u64,
    pub founder: Vec<u8>,
    pub name: String,
    pub about: String,
    pub directory: Directory,
    pub roles: Vec<Role>,
    /// Keyed by `did:hum:`.
    pub members: BTreeMap<String, Member>,
    /// Every invitation, by its entry id.
    pub invites: BTreeMap<String, Invite>,
    /// Every verified entry in fold order, the founding first.
    pub log: Vec<LogLine>,
    /// Entries dated too far ahead of the reader's clock, waiting.
    pub waiting: usize,
    /// Objects dropped before the fold: bad signatures, another organisation's, no date.
    pub dropped: usize,
}

/// Why a log does not fold at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FoldError {
    /// No founding object whose id is the organisation's and whose signature verifies.
    NoFounding,
}

impl OrgState {
    pub fn role(&self, id: &str) -> Option<&Role> {
        self.roles.iter().find(|r| r.id == id)
    }
    pub fn member(&self, key: &[u8]) -> Option<&Member> {
        self.members.get(&did_for_pubkey(key))
    }
    /// The rank of `key`'s role, `None` for a non-member.
    pub fn rank_of(&self, key: &[u8]) -> Option<u64> {
        self.member(key).and_then(|m| self.role(&m.role)).map(|r| r.rank)
    }
    /// Whether `key` is a member whose role grants `p`.
    pub fn can(&self, key: &[u8], p: &str) -> bool {
        self.member(key).and_then(|m| self.role(&m.role)).is_some_and(|r| r.grants(p))
    }
    /// Invitations still open at `now_ms`.
    pub fn open_invites(&self, now_ms: u64) -> Vec<&Invite> {
        self.invites.values().filter(|i| !i.used && !i.withdrawn && i.until >= now_ms).collect()
    }
}

/// Fold `objects` (organisation `org_id`'s founding and entries, in any order, duplicates
/// allowed) into the current state, as every app does. `now_ms` is the reader's clock.
pub fn fold(org_id: &str, objects: &[Object], now_ms: u64) -> Result<OrgState, FoldError> {
    let mut founding: Option<(&Object, Founding)> = None;
    let mut entries: BTreeMap<(u64, String), &Object> = BTreeMap::new();
    let mut dropped = 0usize;
    let mut waiting = 0usize;
    for o in objects {
        let Ok(id) = o.object_id().map(|h| h.to_hex()) else {
            dropped += 1;
            continue;
        };
        match o.object_type.as_str() {
            FOUNDING_TYPE if id == org_id && founding.is_none() => {
                if o.verify_signature().is_err() {
                    dropped += 1;
                    continue;
                }
                match parse_founding(o) {
                    Some(f) if o.created_at.is_some() => founding = Some((o, f)),
                    _ => dropped += 1,
                }
            }
            ENTRY_TYPE if o.references.first().map(String::as_str) == Some(org_id) => {
                let Some(at) = o.created_at else {
                    dropped += 1;
                    continue;
                };
                if entries.contains_key(&(at, id.clone())) {
                    continue;
                }
                if o.verify_signature().is_err() {
                    dropped += 1;
                    continue;
                }
                if at > now_ms.saturating_add(FUTURE_SLACK_MS) {
                    waiting += 1;
                    continue;
                }
                entries.insert((at, id), o);
            }
            FOUNDING_TYPE => {}
            _ => dropped += 1,
        }
        if entries.len() > MAX_ENTRIES {
            break;
        }
    }
    let (fobj, f) = founding.ok_or(FoldError::NoFounding)?;
    let founded_at = fobj.created_at.unwrap_or(0);
    let founder = fobj.author_public_key.clone();
    let mut st = OrgState {
        org_id: org_id.to_string(),
        founded_at,
        founder: founder.clone(),
        name: f.name,
        about: f.about,
        directory: f.directory,
        roles: f.roles,
        members: BTreeMap::new(),
        invites: BTreeMap::new(),
        log: Vec::new(),
        waiting,
        dropped,
    };
    st.members.insert(
        did_for_pubkey(&founder),
        Member { key: founder.clone(), did: did_for_pubkey(&founder), role: OWNER.into(), name: None, listing: Listing::Members, joined_at: founded_at, standing: BTreeSet::new() },
    );
    st.log.push(LogLine { entry_id: org_id.to_string(), at: founded_at, actor: founder, act: None, subject: None, refused: None });
    // When each removed or departed member last left: an invitation made before then is void.
    let mut left_at: BTreeMap<String, u64> = BTreeMap::new();
    // Each custom role's defining entries (with their support), for a holder's standing.
    let mut role_support: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for ((at, id), o) in entries {
        let actor = o.author_public_key.clone();
        let act = parse_act(o);
        let subject = match &act {
            Some(Act::Invite { who, .. } | Act::Remove { who } | Act::Assign { who, .. }) => Some(who.clone()),
            Some(Act::Accept { .. } | Act::Leave | Act::Listing { .. }) => Some(actor.clone()),
            _ => None,
        };
        let refused = match &act {
            None => Some(Refusal::Malformed),
            Some(_) if at < founded_at => Some(Refusal::BeforeFounding),
            Some(a) => apply(&mut st, &mut left_at, &mut role_support, &id, at, &actor, a).err(),
        };
        st.log.push(LogLine { entry_id: id, at, actor, act, subject, refused });
    }
    Ok(st)
}

/// The actor's standing plus their own membership-making entries: what an entry they sign leans on.
fn support_of(st: &OrgState, actor: &[u8]) -> BTreeSet<String> {
    st.member(actor).map(|m| m.standing.clone()).unwrap_or_default()
}

fn apply(
    st: &mut OrgState,
    left_at: &mut BTreeMap<String, u64>,
    role_support: &mut BTreeMap<String, BTreeSet<String>>,
    id: &str,
    at: u64,
    actor: &[u8],
    act: &Act,
) -> Result<(), Refusal> {
    let actor_did = did_for_pubkey(actor);
    let actor_rank = st.rank_of(actor);
    // Everything but an acceptance is signed by a member.
    if !matches!(act, Act::Accept { .. }) && actor_rank.is_none() {
        return Err(Refusal::NotAMember);
    }
    let need = |st: &OrgState, p: &str| if st.can(actor, p) { Ok(()) } else { Err(Refusal::LacksPermission) };
    let ar = actor_rank.unwrap_or(0);
    match act {
        Act::Invite { who, role, until } => {
            need(st, perm::INVITE)?;
            let r = st.role(role).ok_or(Refusal::NoSuchRole)?;
            if r.rank >= ar {
                return Err(Refusal::RankTooLow);
            }
            if st.member(who).is_some() {
                return Err(Refusal::AlreadyMember);
            }
            let support = support_of(st, actor);
            st.invites.insert(
                id.to_string(),
                Invite { id: id.to_string(), who: who.clone(), who_did: did_for_pubkey(who), role: role.clone(), until: *until, by: actor.to_vec(), at, support, used: false, withdrawn: false },
            );
        }
        Act::Withdraw { invite } => {
            let inv = st.invites.get(invite).ok_or(Refusal::NoSuchInvite)?;
            if inv.by != actor {
                need(st, perm::INVITE)?;
            }
            if inv.used {
                return Err(Refusal::InviteUsed);
            }
            st.invites.get_mut(invite).expect("checked").withdrawn = true;
        }
        Act::Accept { invite } => {
            let inv = st.invites.get(invite).ok_or(Refusal::NoSuchInvite)?.clone();
            if inv.who != actor {
                return Err(Refusal::NotTheInvitee);
            }
            if inv.used {
                return Err(Refusal::InviteUsed);
            }
            if inv.withdrawn {
                return Err(Refusal::InviteWithdrawn);
            }
            if at > inv.until {
                return Err(Refusal::InviteExpired);
            }
            if st.member(actor).is_some() {
                return Err(Refusal::AlreadyMember);
            }
            if left_at.get(&actor_did).is_some_and(|&t| t >= inv.at) {
                return Err(Refusal::RemovedSince);
            }
            // The inviter must still be able to invite to that role when it is accepted.
            let role = if st.role(&inv.role).is_some() { inv.role.clone() } else { MEMBER.to_string() };
            let role_rank = st.role(&role).map(|r| r.rank).unwrap_or(0);
            match st.rank_of(&inv.by) {
                Some(r) if st.can(&inv.by, perm::INVITE) && role_rank < r => {}
                _ => return Err(Refusal::InviterCannot),
            }
            let mut standing = inv.support.clone();
            standing.insert(inv.id.clone());
            standing.insert(id.to_string());
            if let Some(s) = role_support.get(&role) {
                standing.extend(s.iter().cloned());
            }
            st.invites.get_mut(invite).expect("checked").used = true;
            st.members.insert(actor_did.clone(), Member { key: actor.to_vec(), did: actor_did, role, name: None, listing: Listing::Members, joined_at: at, standing });
        }
        Act::Remove { who } => {
            need(st, perm::REMOVE)?;
            let target = st.member(who).ok_or(Refusal::NotAMember)?;
            if target.role == OWNER {
                return Err(Refusal::OwnerStays);
            }
            if st.rank_of(who).unwrap_or(0) >= ar {
                return Err(Refusal::RankTooLow);
            }
            let did = target.did.clone();
            st.members.remove(&did);
            left_at.insert(did, at);
        }
        Act::Leave => {
            let m = st.member(actor).ok_or(Refusal::NotAMember)?;
            if m.role == OWNER {
                return Err(Refusal::OwnerStays);
            }
            st.members.remove(&actor_did);
            left_at.insert(actor_did, at);
        }
        Act::Assign { who, role } => {
            need(st, perm::ASSIGN)?;
            let target = st.member(who).ok_or(Refusal::NotAMember)?.clone();
            let r = st.role(role).ok_or(Refusal::NoSuchRole)?;
            if target.role == OWNER {
                return Err(Refusal::OwnerStays);
            }
            if r.rank >= ar || st.rank_of(who).unwrap_or(0) >= ar {
                return Err(Refusal::RankTooLow);
            }
            let mut standing = target.standing.clone();
            standing.extend(support_of(st, actor));
            standing.insert(id.to_string());
            if let Some(s) = role_support.get(role) {
                standing.extend(s.iter().cloned());
            }
            let m = st.members.get_mut(&target.did).expect("checked");
            m.role = role.clone();
            m.standing = standing;
        }
        Act::RoleSet { role } => {
            need(st, perm::DEFINE)?;
            if role.is_built_in() {
                return Err(Refusal::BuiltInRole);
            }
            if role.rank >= ar {
                return Err(Refusal::RankTooLow);
            }
            if let Some(old) = st.role(&role.id) {
                if old.rank >= ar {
                    return Err(Refusal::RankTooLow);
                }
            } else if st.roles.len() >= ROLES_MAX {
                return Err(Refusal::Malformed);
            }
            if role.perms.iter().any(|p| !st.can(actor, p)) {
                return Err(Refusal::LacksPermission);
            }
            let mut support = support_of(st, actor);
            support.insert(id.to_string());
            role_support.entry(role.id.clone()).or_default().extend(support);
            match st.roles.iter_mut().find(|r| r.id == role.id) {
                Some(r) => *r = role.clone(),
                None => st.roles.push(role.clone()),
            }
        }
        Act::RoleDrop { role } => {
            need(st, perm::DEFINE)?;
            if is_built_in(role) {
                return Err(Refusal::BuiltInRole);
            }
            let r = st.role(role).ok_or(Refusal::NoSuchRole)?;
            if r.rank >= ar {
                return Err(Refusal::RankTooLow);
            }
            st.roles.retain(|r| &r.id != role);
            role_support.remove(role);
            for m in st.members.values_mut() {
                if &m.role == role {
                    m.role = MEMBER.into();
                }
            }
        }
        Act::Listing { name, level } => {
            let m = st.members.get_mut(&actor_did).ok_or(Refusal::NotAMember)?;
            m.name = if name.trim().is_empty() { None } else { Some(name.trim().to_string()) };
            m.listing = *level;
        }
        Act::Settings { name, about, directory } => {
            need(st, perm::SETTINGS)?;
            if let Some(n) = name {
                st.name = n.trim().to_string();
            }
            if let Some(a) = about {
                st.about = a.clone();
            }
            if let Some(d) = directory {
                st.directory = *d;
            }
        }
    }
    Ok(())
}

// ── What a reader may see ──

/// Why a reader may not read something.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadRefusal {
    /// The reader is not a member and the directory is not public (or it is the audit log).
    NotAMember,
    /// The directory is for organisers and the reader is not one.
    OrganisersOnly,
}

/// One person as a reader is shown them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shown {
    /// Named: their id, the name they gave (if any) and their role id.
    Named { did: String, name: Option<String>, role: Option<String> },
    /// A member who keeps their listing to organisers, to a reader who is not one.
    Private,
}

/// The directory as one reader may see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryView {
    /// The members the reader may see named, in id order.
    pub named: Vec<Shown>,
    /// How many members keep their listing to organisers (counted, not named, for this reader).
    pub private: usize,
}

impl OrgState {
    /// Whether `viewer` sees `m` named.
    fn names(&self, viewer: &[u8], m: &Member) -> bool {
        m.listing == Listing::Members || m.key == viewer || self.can(viewer, perm::SEE_ALL)
    }

    fn shown(&self, viewer: &[u8], key: &[u8]) -> Shown {
        match self.member(key) {
            Some(m) if !self.names(viewer, m) => Shown::Private,
            Some(m) => Shown::Named { did: m.did.clone(), name: m.name.clone(), role: Some(m.role.clone()) },
            None => Shown::Named { did: did_for_pubkey(key), name: None, role: None },
        }
    }

    /// The member directory as `viewer` may read it. A non-member reads a private organisation's
    /// directory never; an `organisers` directory only those with `directory.see_all` read.
    pub fn directory_for(&self, viewer: &[u8]) -> Result<DirectoryView, ReadRefusal> {
        let member = self.member(viewer).is_some();
        match self.directory {
            Directory::Public => {}
            Directory::Members if !member => return Err(ReadRefusal::NotAMember),
            Directory::Organisers if !member => return Err(ReadRefusal::NotAMember),
            Directory::Organisers if !self.can(viewer, perm::SEE_ALL) => return Err(ReadRefusal::OrganisersOnly),
            _ => {}
        }
        let mut view = DirectoryView { named: Vec::new(), private: 0 };
        for m in self.members.values() {
            if self.names(viewer, m) {
                view.named.push(Shown::Named { did: m.did.clone(), name: m.name.clone(), role: Some(m.role.clone()) });
            } else {
                view.private += 1;
            }
        }
        Ok(view)
    }

    /// The audit log as `viewer` may read it: members only, newest first, every line's actor and
    /// subject named unless they keep their listing to organisers and the reader is not one.
    pub fn audit_log_for(&self, viewer: &[u8]) -> Result<Vec<(LogLine, Shown, Option<Shown>)>, ReadRefusal> {
        if self.member(viewer).is_none() {
            return Err(ReadRefusal::NotAMember);
        }
        Ok(self
            .log
            .iter()
            .rev()
            .map(|l| {
                let actor = self.shown(viewer, &l.actor);
                let subject = l.subject.as_ref().map(|s| self.shown(viewer, s));
                (l.clone(), actor, subject)
            })
            .collect())
    }

    /// What an invitation carries: the founding, the invite, and the entries that gave the inviter
    /// their standing, never the rest of the log (a private organisation's member list stays with
    /// its members until the invitee joins).
    pub fn invite_bundle(&self, objects: &[Object], invite_id: &str) -> Option<String> {
        let inv = self.invites.get(invite_id)?;
        let mut want: BTreeSet<String> = inv.support.clone();
        want.insert(invite_id.to_string());
        want.insert(self.org_id.clone());
        let picked: Vec<Object> = objects.iter().filter(|o| o.object_id().is_ok_and(|h| want.contains(&h.to_hex()))).cloned().collect();
        Some(bundle_text(&self.org_id, &picked))
    }
}

#[cfg(test)]
#[path = "org_tests.rs"]
mod tests;
