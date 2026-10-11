//! The Media page's requests to its server (the relay half is src/relay/media.rs; the design is
//! docs/design/media-showcase.md, "Increment 2, exact"), and the server's answers read into
//! plain structs.
//!
//! EVERY WRITE IS SIGNED the way the relay checks it (`check_signed`): Dilithium over
//! `purpose\ntimestamp`, the same signature every other identity-keyed request of the desktop
//! makes (`net::identity::pq_sign_chat`), with a purpose that names its target, so a signature is
//! good for one thing (the relay's F5): `media_publish:<file name>`, `media_remove:<item id or
//! file name>`, `media_pick:<item id>` (pick and unpick), `media_feature:<item id>`,
//! `media_suggestions:<suggestion id>:<accept|decline>` and `media_categories:<BLAKE3 hex of the
//! category list exactly as sent>`. A new suggestion keeps plain `media_suggest`, and the reads
//! keep `media_list`, `media_usage` and `media_suggestions`. The relay also refuses a (key,
//! purpose, timestamp) it has seen, so every signature here takes a fresh timestamp from
//! [`next_timestamp`], which never hands out the same millisecond twice.
//!
//! A signed read goes in a POST body, never a URL: a Dilithium key and signature make an address
//! of about 10 KB, which a proxy refuses (relay/media.rs).
//!
//! Nothing here runs on the frame: [`Request`] is built on the frame (cheap, and what the tests
//! read), and [`send`] runs it on a worker thread.

use serde_json::{json, Value};

// ── What the server sends back ──────────────────────────────────────────────────────────────

/// One content label as the server lists it (data/media/labels.json on the relay).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Label {
    pub id: String,
    pub label: String,
    pub desc: String,
}

/// One category, in the order the page shows them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub description: String,
}

/// One Media item as every route gives it (relay/media.rs `item_json`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Item {
    pub id: i64,
    /// `/uploads/<name>` while public; `/api/media/file/<id>` for a members-only item, fetched
    /// with this session's upload token.
    pub url: String,
    pub title: String,
    pub description: String,
    pub category_id: i64,
    pub category_name: String,
    /// `picture`, `video`, `model` or `file` (data/media/kinds.json names them).
    pub kind: String,
    /// The content label ids.
    pub labels: Vec<String>,
    /// `public` or `members`.
    pub visibility: String,
    pub featured: bool,
    pub size_bytes: i64,
    pub views: i64,
    pub uploaded_at: i64,
    pub published_at: i64,
    pub uploader_key: String,
    pub uploader_name: String,
    pub picks: i64,
    /// Where the upload was before it went on Media (0 a chat file, 1 Shared Files, 2 uploaded
    /// for Media), when the server says. The relay does not send it today, so the Remove
    /// confirmation works it out where it can ([`remove_words`]).
    pub from_shelf: Option<i64>,
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

fn i(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(Value::as_i64).unwrap_or(0)
}

impl Item {
    /// The item in `v`, or None when it has no id.
    pub fn from_json(v: &Value) -> Option<Item> {
        let id = v.get("id").and_then(Value::as_i64)?;
        let category = v.get("category").cloned().unwrap_or(Value::Null);
        Some(Item {
            id,
            url: s(v, "url"),
            title: s(v, "title"),
            description: s(v, "description"),
            category_id: i(&category, "id"),
            category_name: s(&category, "name"),
            kind: s(v, "kind"),
            labels: v
                .get("labels")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
                .unwrap_or_default(),
            visibility: s(v, "visibility"),
            featured: v.get("featured").and_then(Value::as_bool).unwrap_or(false),
            size_bytes: i(v, "size_bytes"),
            views: i(v, "views"),
            uploaded_at: i(v, "uploaded_at"),
            published_at: i(v, "published_at"),
            uploader_key: s(v, "uploader_key"),
            uploader_name: s(v, "uploader_name"),
            picks: i(v, "picks"),
            from_shelf: v.get("from_shelf").and_then(Value::as_i64),
        })
    }

    /// Members only: anything the server does not call public (it fails closed the same way).
    pub fn members_only(&self) -> bool {
        self.visibility != "public"
    }

    /// The stored file name of a public item (the last part of its `/uploads/` address).
    pub fn upload_name(&self) -> Option<&str> {
        self.url.strip_prefix("/uploads/").filter(|n| !n.is_empty() && !n.contains('/'))
    }

    /// What the tile calls it: its title, else its file name, else the name of its kind.
    pub fn display_title(&self) -> String {
        if !self.title.trim().is_empty() {
            self.title.trim().to_string()
        } else if let Some(name) = self.upload_name() {
            name.to_string()
        } else {
            super::kinds::one_name(&self.kind)
        }
    }

    /// Who put it there, by name ("someone" when the server knows no name).
    pub fn by_name(&self) -> &str {
        if self.uploader_name.trim().is_empty() {
            "someone"
        } else {
            self.uploader_name.trim()
        }
    }
}

/// The list's answer (`GET /api/media`, `POST /api/media/list`, `GET /api/media/picks/{key}`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ListAnswer {
    pub items: Vec<Item>,
    /// The `before` that asks for the next page; None at the end.
    pub next_before: Option<i64>,
    /// Whether the answer is the members' view (a signed request from a member).
    pub members_view: bool,
    /// The server's `media_visibility`: `public` or `members`.
    pub media_visibility: String,
    /// The whole page is members only and this viewer is not one.
    pub members_only_for_you: bool,
    pub categories: Vec<Category>,
    pub labels: Vec<Label>,
}

/// Read a category list (the list's `categories`, or a categories answer's).
pub fn parse_categories(v: Option<&Value>) -> Vec<Category> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|c| Some(Category { id: c.get("id")?.as_i64()?, name: s(c, "name"), description: s(c, "description") }))
                .collect()
        })
        .unwrap_or_default()
}

/// Read a label list.
pub fn parse_labels(v: Option<&Value>) -> Vec<Label> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter(|l| !s(l, "id").is_empty())
                .map(|l| Label { id: s(l, "id"), label: s(l, "label"), desc: s(l, "desc") })
                .collect()
        })
        .unwrap_or_default()
}

/// Read the list's answer.
pub fn parse_list(v: &Value) -> ListAnswer {
    ListAnswer {
        items: v.get("items").and_then(Value::as_array).map(|a| a.iter().filter_map(Item::from_json).collect()).unwrap_or_default(),
        next_before: v.get("next_before").and_then(Value::as_i64),
        members_view: v.get("members_view").and_then(Value::as_bool).unwrap_or(false),
        media_visibility: s(v, "media_visibility"),
        members_only_for_you: v.get("members_only_for_you").and_then(Value::as_bool).unwrap_or(false),
        categories: parse_categories(v.get("categories")),
        labels: parse_labels(v.get("labels")),
    }
}

/// A category suggestion (`GET or POST /api/media/suggestions`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Suggestion {
    pub id: i64,
    pub suggester_key: String,
    pub suggester_name: String,
    pub name: String,
    pub note: String,
    pub created_at: i64,
    /// `open`, `accepted` or `declined`.
    pub state: String,
}

/// Read the suggestions answer.
pub fn parse_suggestions(v: &Value) -> Vec<Suggestion> {
    v.get("suggestions")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| {
                    Some(Suggestion {
                        id: x.get("id")?.as_i64()?,
                        suggester_key: s(x, "suggester_key"),
                        suggester_name: s(x, "suggester_name"),
                        name: s(x, "name"),
                        note: s(x, "note"),
                        created_at: i(x, "created_at"),
                        state: s(x, "state"),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A Media upload not yet published (it expires after a day unless it is).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Waiting {
    pub filename: String,
    pub uploader_name: String,
    pub size_bytes: i64,
    pub uploaded_at: i64,
    pub expires_at: i64,
}

/// The admins' numbers (`POST /api/media/usage`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Usage {
    pub used_bytes: i64,
    pub budget_bytes: i64,
    pub per_person_bytes: i64,
    pub items: i64,
    pub largest: Vec<Item>,
    pub oldest: Vec<Item>,
    pub least_viewed: Vec<Item>,
    pub waiting: Vec<Waiting>,
}

/// Read the usage answer.
pub fn parse_usage(v: &Value) -> Usage {
    let items = |k: &str| v.get(k).and_then(Value::as_array).map(|a| a.iter().filter_map(Item::from_json).collect()).unwrap_or_default();
    Usage {
        used_bytes: i(v, "used_bytes"),
        budget_bytes: i(v, "budget_bytes"),
        per_person_bytes: i(v, "per_person_bytes"),
        items: i(v, "items"),
        largest: items("largest"),
        oldest: items("oldest"),
        least_viewed: items("least_viewed"),
        waiting: v
            .get("waiting")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|w| Waiting {
                        filename: s(w, "filename"),
                        uploader_name: s(w, "uploader_name"),
                        size_bytes: i(w, "size_bytes"),
                        uploaded_at: i(w, "uploaded_at"),
                        expires_at: i(w, "expires_at"),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

// ── Words ──────────────────────────────────────────────────────────────────────────────────

/// A size in the words the page uses: megabytes below a gigabyte, else gigabytes, one decimal
/// with a whole number shown whole ("412 MB", "12.4 GB", "50 GB").
pub fn size_words(bytes: i64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    let b = bytes.max(0) as f64;
    let (n, unit) = if b >= 1024.0 * MB { (b / (1024.0 * MB), "GB") } else { (b / MB, "MB") };
    let shown = format!("{n:.1}");
    let shown = shown.strip_suffix(".0").unwrap_or(&shown);
    format!("{shown} {unit}")
}

/// The meter: "12.4 GB of 50 GB used".
pub fn meter_words(used: i64, budget: i64) -> String {
    format!("{} of {} used", size_words(used), size_words(budget))
}

/// The line above the Add to Media form: "You have 412 MB of your 512 MB left".
pub fn share_line(used: i64, share: i64) -> String {
    format!("You have {} of your {} left", size_words((share - used).max(0)), size_words(share))
}

/// Why a file of `len` bytes cannot go: over the person's upload limit on this server
/// (`role_limit`), or over what is left of their Media share (`left`, when known). None when
/// it fits. Checked before a byte is sent, so an upload over the share never starts.
pub fn room_problem(len: u64, role_limit: Option<u64>, left: Option<i64>) -> Option<String> {
    if let Some(limit) = role_limit.filter(|l| len > *l) {
        return Some(format!(
            "This file is {}, over your {} upload limit on this server. An admin can raise it in Server Settings > Roles.",
            size_words(len as i64),
            size_words(limit as i64)
        ));
    }
    match left {
        Some(left) if len as i64 > left => Some(format!(
            "This file is {}, more than the {} left of your share of this server's Media space, so it was not uploaded. Remove one of your items, or ask an admin to raise each person's share.",
            size_words(len as i64),
            size_words(left.max(0))
        )),
        _ => None,
    }
}

/// The confirmation when a Remove takes the file away with the item.
pub const REMOVE_DELETES: &str = "Remove from Media and delete the file.";
/// The confirmation when the file stays where it came from (a chat or Shared Files).
pub const REMOVE_KEEPS: &str = "Remove from Media. The file stays where it came from.";
/// The confirmation when this app cannot tell which (the server does not say where an item's
/// file came from; see `Item::from_shelf`).
pub const REMOVE_EITHER: &str =
    "Remove from Media. A file uploaded for Media is deleted with it; a file from a chat or Shared Files stays where it came from.";

/// What Remove does to `item`'s file, in the words the confirmation shows. Only a file uploaded
/// for Media loses its file (the relay's F2). A members-only item was uploaded for Media (only
/// such a file can be members only, `was_public`); `uploaded_here` and `put_here` are this
/// session's own Add to Media and Put on Media items.
pub fn remove_words(item: &Item, uploaded_here: bool, put_here: bool) -> &'static str {
    match item.from_shelf {
        Some(2) => REMOVE_DELETES,
        Some(_) => REMOVE_KEEPS,
        None if item.members_only() || uploaded_here => REMOVE_DELETES,
        None if put_here => REMOVE_KEEPS,
        None => REMOVE_EITHER,
    }
}

// ── Signing ────────────────────────────────────────────────────────────────────────────────

/// A timestamp in Unix milliseconds that is never the same as one handed out before in this
/// run: the relay refuses a (key, purpose, timestamp) it has seen, and two requests in one
/// millisecond (a Refresh and a More) would otherwise share one.
pub fn next_timestamp() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static LAST: AtomicU64 = AtomicU64::new(0);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
    let mut prev = LAST.load(Ordering::SeqCst);
    loop {
        let next = now.max(prev + 1);
        match LAST.compare_exchange(prev, next, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => return next,
            Err(seen) => prev = seen,
        }
    }
}

/// Who signs: the person's Dilithium public key (hex) and the seed it comes from.
#[derive(Clone)]
pub struct Signer {
    pub key: String,
    seed: Vec<u8>,
}

impl std::fmt::Debug for Signer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Signer").field("key", &self.key).finish_non_exhaustive() // never the seed
    }
}

impl Signer {
    pub fn new(key: &str, seed: &[u8]) -> Self {
        Signer { key: key.to_string(), seed: seed.to_vec() }
    }

    /// `body` with `key`, `timestamp` and `sig` over `purpose` added.
    pub fn signed(&self, purpose: &str, mut body: Value) -> Value {
        let ts = next_timestamp();
        let sig = crate::net::identity::pq_sign_chat(&self.seed, purpose, ts);
        if !body.is_object() {
            body = json!({});
        }
        body["key"] = json!(self.key);
        body["timestamp"] = json!(ts);
        body["sig"] = json!(sig);
        body
    }
}

/// The signer for this app's identity, when it is unlocked.
pub fn signer_of(state: &crate::gui::GuiState) -> Option<Signer> {
    let seed = state.private_key_bytes.as_ref()?;
    if state.profile_public_key.is_empty() {
        return None;
    }
    Some(Signer::new(&state.profile_public_key, seed))
}

/// The purpose of a signed list read.
pub const LIST: &str = "media_list";
/// The purpose of a new category suggestion.
pub const SUGGEST: &str = "media_suggest";
/// The purpose of the suggestions read.
pub const SUGGESTIONS: &str = "media_suggestions";
/// The purpose of the admins' usage read.
pub const USAGE: &str = "media_usage";

/// The stored file name a request names: as given, or the last part of its `/uploads/` address.
pub fn bare_name(given: &str) -> &str {
    given.trim().trim_start_matches("/uploads/")
}

pub fn publish_purpose(filename: &str) -> String {
    format!("media_publish:{}", bare_name(filename))
}

/// What a removal names: a published item, or a Media upload waiting to be published.
#[derive(Debug, Clone, PartialEq)]
pub enum RemoveTarget {
    Item(i64),
    Upload(String),
}

pub fn remove_purpose(target: &RemoveTarget) -> String {
    match target {
        RemoveTarget::Item(id) => format!("media_remove:{id}"),
        RemoveTarget::Upload(name) => format!("media_remove:{}", bare_name(name)),
    }
}

pub fn pick_purpose(id: i64) -> String {
    format!("media_pick:{id}")
}

pub fn feature_purpose(id: i64) -> String {
    format!("media_feature:{id}")
}

pub fn decide_purpose(id: i64, accept: bool) -> String {
    format!("media_suggestions:{id}:{}", if accept { "accept" } else { "decline" })
}

/// The purpose of a category list: BLAKE3 over the list's JSON text exactly as it is sent.
pub fn categories_purpose(categories_text: &str) -> String {
    format!("media_categories:{}", blake3::hash(categories_text.as_bytes()).to_hex())
}

// ── Requests ───────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

/// One request, built on the frame and sent on a worker thread.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub method: Method,
    /// The path and query, from `/api/...`.
    pub path: String,
    /// The JSON body of a POST, as text (the categories request's text is what is hashed).
    pub body: Option<String>,
    /// The purpose the body is signed over, for the tests and the log; None for an unsigned read.
    pub purpose: Option<String>,
}

fn post(path: &str, purpose: String, body: Value) -> Request {
    Request { method: Method::Post, path: path.to_string(), body: Some(body.to_string()), purpose: Some(purpose) }
}

fn enc(text: &str) -> String {
    url::form_urlencoded::byte_serialize(text.as_bytes()).collect()
}

/// The list's filters, as the page holds them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Filters {
    pub category: Option<i64>,
    pub kind: Option<String>,
    /// A person, by their key (the page shows their name).
    pub by: Option<String>,
    pub q: String,
    /// The person's own picks instead of everything.
    pub my_picks: bool,
}

/// How many items a page of the list asks for.
pub const PAGE: i64 = 48;

/// The list request for `filters`, one page older than `before`. With a signer (someone signed
/// in to this server) it is the signed POST, which gets members-only items to a member; without
/// one, the public GET. "My picks" is `GET /api/media/picks/{me}`, which takes a signature only
/// in its address, too long for a proxy, so it is always the public view of them.
pub fn list_request(filters: &Filters, before: Option<i64>, limit: i64, me: &str, signer: Option<&Signer>) -> Request {
    let q = filters.q.trim();
    if filters.my_picks || signer.is_none() {
        let mut query = vec![format!("limit={limit}")];
        if let Some(c) = filters.category {
            query.push(format!("category={c}"));
        }
        if let Some(k) = filters.kind.as_deref() {
            query.push(format!("kind={}", enc(k)));
        }
        if let Some(by) = filters.by.as_deref() {
            query.push(format!("by={}", enc(by)));
        }
        if !q.is_empty() {
            query.push(format!("q={}", enc(q)));
        }
        if let Some(b) = before {
            query.push(format!("before={b}"));
        }
        let path = if filters.my_picks { format!("/api/media/picks/{}", enc(me)) } else { "/api/media".to_string() };
        return Request { method: Method::Get, path: format!("{path}?{}", query.join("&")), body: None, purpose: None };
    }
    let signer = signer.expect("checked above");
    let mut body = json!({ "limit": limit });
    if let Some(c) = filters.category {
        body["category"] = json!(c);
    }
    if let Some(k) = filters.kind.as_deref() {
        body["kind"] = json!(k);
    }
    if let Some(by) = filters.by.as_deref() {
        body["by"] = json!(by);
    }
    if !q.is_empty() {
        body["q"] = json!(q);
    }
    if let Some(b) = before {
        body["before"] = json!(b);
    }
    post("/api/media/list", LIST.to_string(), signer.signed(LIST, body))
}

/// What an item is published as (the Add to Media and Put on Media form).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PublishFields {
    /// The stored file name (or its `/uploads/` address).
    pub filename: String,
    pub title: String,
    pub description: String,
    pub category_id: Option<i64>,
    pub labels: Vec<String>,
    /// `public` or `members`.
    pub visibility: String,
}

pub fn publish_request(signer: &Signer, f: &PublishFields) -> Request {
    let name = bare_name(&f.filename).to_string();
    let purpose = publish_purpose(&name);
    let mut body = json!({
        "filename": name,
        "title": f.title.trim(),
        "description": f.description.trim(),
        "labels": f.labels,
        "visibility": if f.visibility == "members" { "members" } else { "public" },
    });
    if let Some(c) = f.category_id {
        body["category_id"] = json!(c);
    }
    post("/api/media", purpose.clone(), signer.signed(&purpose, body))
}

pub fn remove_request(signer: &Signer, target: &RemoveTarget) -> Request {
    let purpose = remove_purpose(target);
    let body = match target {
        RemoveTarget::Item(id) => json!({ "id": id }),
        RemoveTarget::Upload(name) => json!({ "filename": bare_name(name) }),
    };
    post("/api/media/remove", purpose.clone(), signer.signed(&purpose, body))
}

/// Pick (`on`) or unpick item `id`: one purpose for both, as the relay checks it.
pub fn pick_request(signer: &Signer, id: i64, on: bool) -> Request {
    let purpose = pick_purpose(id);
    let path = if on { "/api/media/pick" } else { "/api/media/unpick" };
    post(path, purpose.clone(), signer.signed(&purpose, json!({ "id": id })))
}

pub fn feature_request(signer: &Signer, id: i64, featured: bool) -> Request {
    let purpose = feature_purpose(id);
    post("/api/media/feature", purpose.clone(), signer.signed(&purpose, json!({ "id": id, "featured": featured })))
}

pub fn suggest_request(signer: &Signer, name: &str, note: &str) -> Request {
    post("/api/media/suggest", SUGGEST.to_string(), signer.signed(SUGGEST, json!({ "name": name.trim(), "note": note.trim() })))
}

pub fn suggestions_request(signer: &Signer) -> Request {
    post("/api/media/suggestions", SUGGESTIONS.to_string(), signer.signed(SUGGESTIONS, json!({})))
}

pub fn decide_request(signer: &Signer, id: i64, accept: bool) -> Request {
    let purpose = decide_purpose(id, accept);
    post("/api/media/suggestions/decide", purpose.clone(), signer.signed(&purpose, json!({ "id": id, "accept": accept })))
}

pub fn usage_request(signer: &Signer) -> Request {
    post("/api/media/usage", USAGE.to_string(), signer.signed(USAGE, json!({})))
}

/// One category in an admin's whole list: an existing one by id, or a new one with none.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CategoryEdit {
    pub id: Option<i64>,
    pub name: String,
    pub description: String,
}

/// The whole category list. The `categories` member's text is written once and both hashed into
/// the purpose and put in the body as it is, so the relay hashes exactly the same characters.
pub fn categories_request(signer: &Signer, list: &[CategoryEdit]) -> Request {
    let cats: Vec<Value> = list
        .iter()
        .map(|c| {
            let mut v = json!({ "name": c.name.trim(), "description": c.description.trim() });
            if let Some(id) = c.id {
                v["id"] = json!(id);
            }
            v
        })
        .collect();
    let cats_text = Value::Array(cats).to_string();
    let purpose = categories_purpose(&cats_text);
    let ts = next_timestamp();
    let sig = crate::net::identity::pq_sign_chat(&signer.seed, &purpose, ts);
    let body = format!(
        "{{\"key\":{},\"timestamp\":{ts},\"sig\":{},\"categories\":{cats_text}}}",
        Value::from(signer.key.as_str()),
        Value::from(sig)
    );
    Request { method: Method::Post, path: "/api/media/categories".to_string(), body: Some(body), purpose: Some(purpose) }
}

/// Where a Media item's file is fetched from: its public address, or, for a members-only item,
/// the members' file route with this session's upload token (`token`).
pub fn file_url(base: &str, item: &Item, token: Option<&str>) -> String {
    let base = base.trim_end_matches('/');
    if item.members_only() {
        format!("{base}/api/media/file/{}?token={}", item.id, enc(token.unwrap_or("")))
    } else if item.url.starts_with("http://") || item.url.starts_with("https://") {
        item.url.clone()
    } else {
        format!("{base}{}", item.url)
    }
}

// ── Sending ────────────────────────────────────────────────────────────────────────────────

/// Send `req` to the server at `base` (an `https://host` address) and read its JSON answer. A
/// refusal comes back as the server's own words. Blocking: call it on a worker thread.
pub fn send(base: &str, req: &Request) -> Result<Value, String> {
    let url = format!("{}{}", base.trim_end_matches('/'), req.path);
    let result = match (req.method, req.body.as_deref()) {
        (Method::Get, _) => ureq::get(&url).timeout(std::time::Duration::from_secs(30)).call(),
        (Method::Post, body) => ureq::post(&url)
            .timeout(std::time::Duration::from_secs(60))
            .set("Content-Type", "application/json")
            .send_string(body.unwrap_or("{}")),
    };
    let resp = result.map_err(|e| match e {
        ureq::Error::Status(code, r) => {
            let text = r.into_string().unwrap_or_default();
            let text = text.trim();
            if text.is_empty() || text.starts_with('<') {
                format!("The server refused it ({code}).")
            } else {
                text.to_string()
            }
        }
        other => format!("The server could not be reached ({other})."),
    })?;
    let text = resp.into_string().map_err(|e| format!("The answer could not be read ({e})."))?;
    serde_json::from_str(&text).map_err(|e| format!("The answer was not understood ({e})."))
}

#[cfg(test)]
#[path = "api_tests.rs"]
mod tests;
