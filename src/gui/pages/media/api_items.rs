//! Media round 5's requests and answers (2026-10-12; docs/design/media-round-5-plan.md 3.2 to 3.5,
//! with section 10's answers): an item's files and cover, the licences and the words beside them,
//! the list's order, and the requests the item's page and its Edit window send. The relay half is
//! src/relay/media_edit.rs, media_counts.rs and media_wire.rs; the shapes are held to the relay's
//! own by the shared fixture, scripts/tests/fixtures/media-round5-v1.json
//! (`the_desktop_reads_and_signs_the_shared_fixture` below).
//!
//! Every write is signed the way the rest of the page's are ([`Signer`], a purpose naming its
//! target): `media_edit:<id>:<BLAKE3 hex of the edit member's text exactly as sent>`,
//! `media_file_add:<id>:<upload>:<file|cover>` and `media_file_remove:<id>:<file number>`. An
//! edit's text is written once and both hashed and sent as it is ([`edit_request`]), the categories
//! request's pattern, so the relay hashes exactly the same characters.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::api::{bare_name, post, Item, ItemRef, Method, Refusal, Request, Signer, DEFAULT_LICENCE};

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

fn i(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(Value::as_i64).unwrap_or(0)
}

fn b(v: &Value, k: &str) -> bool {
    v.get(k).and_then(Value::as_bool).unwrap_or(false)
}

// ── What the server sends back ──────────────────────────────────────────────────────────────

/// One of an item's files (file 0 is the item's own) or its cover, as an item carries it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ItemFile {
    /// Its number on the item: 0 for the item's own file.
    pub file: i64,
    pub filename: String,
    /// The uploader's own name for it ('' when the server has none).
    pub original_name: String,
    /// `picture`, `video`, `model` or `file`.
    pub kind: String,
    pub size_bytes: i64,
    /// `/uploads/<name>` while the item is public; `/api/media/file/<id>?file=<n>` for a
    /// members-only one, fetched with this session's upload token in a header or a ticket.
    pub url: String,
}

impl ItemFile {
    pub fn from_json(v: &Value) -> Option<ItemFile> {
        let file = v.get("file").and_then(Value::as_i64)?;
        Some(ItemFile { file, filename: s(v, "filename"), original_name: s(v, "original_name"), kind: s(v, "kind"), size_bytes: i(v, "size_bytes"), url: s(v, "url") })
    }
}

/// One licence the server offers (data/media/licences.json on the relay).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Licence {
    pub id: String,
    /// The short name ("CC BY 4.0").
    pub name: String,
    /// Its official name ('' for "All rights reserved").
    pub official_name: String,
    /// Its deed ('' for none).
    pub url: String,
    /// A Creative Commons licence: the editor shows the warnings before it is saved.
    pub cc: bool,
    /// An item under it may be offered for the game.
    pub game: bool,
    /// One plain line of what it allows.
    pub line: String,
}

/// The licences the server offers and the words beside them: the list's and the categories'
/// `licences`. The words are the server's, never this app's.
#[derive(Debug, Clone, PartialEq)]
pub struct Licences {
    /// Preselected.
    pub default: String,
    pub offered: Vec<Licence>,
    /// The "Offer this for the game" tick's label and its line.
    pub game_label: String,
    pub game_line: String,
    /// Shown before a Creative Commons licence is saved, and the line added on a members-only item.
    pub cc_warning: String,
    pub cc_members_warning: String,
    /// The paragraph beside Add to Media: what putting a file on this server lets its people do.
    pub hosting_heading: String,
    pub hosting_text: String,
}

impl Default for Licences {
    /// "All rights reserved" alone, preselected: what an answer with no licences means (an older
    /// server, or a members-only page seen from outside).
    fn default() -> Self {
        Licences {
            default: DEFAULT_LICENCE.to_string(),
            offered: vec![Licence { id: DEFAULT_LICENCE.to_string(), name: "All rights reserved".to_string(), ..Default::default() }],
            game_label: String::new(),
            game_line: String::new(),
            cc_warning: String::new(),
            cc_members_warning: String::new(),
            hosting_heading: String::new(),
            hosting_text: String::new(),
        }
    }
}

impl Licences {
    /// Read an answer's `licences` (None or null: [`Licences::default`]).
    pub fn from_json(v: Option<&Value>) -> Licences {
        let Some(v) = v.filter(|v| v.is_object()) else { return Licences::default() };
        let offered: Vec<Licence> = v
            .get("offered")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter(|l| !s(l, "id").is_empty())
                    .map(|l| Licence {
                        id: s(l, "id"),
                        name: s(l, "name"),
                        official_name: s(l, "official_name"),
                        url: s(l, "url"),
                        cc: b(l, "cc"),
                        game: b(l, "game"),
                        line: s(l, "line"),
                    })
                    .collect()
            })
            .unwrap_or_default();
        if offered.is_empty() {
            return Licences::default();
        }
        let game = v.get("game_offer").cloned().unwrap_or(Value::Null);
        let hosting = v.get("hosting").cloned().unwrap_or(Value::Null);
        let default = s(v, "default");
        Licences {
            default: if default.is_empty() { DEFAULT_LICENCE.to_string() } else { default },
            offered,
            game_label: s(&game, "label"),
            game_line: s(&game, "line"),
            cc_warning: s(v, "cc_warning"),
            cc_members_warning: s(v, "cc_members_warning"),
            hosting_heading: s(&hosting, "heading"),
            hosting_text: s(&hosting, "text"),
        }
    }

    /// Licence `id`, when the server offers it.
    pub fn get(&self, id: &str) -> Option<&Licence> {
        self.offered.iter().find(|l| l.id == id)
    }
}

/// The list's order (round 5), each then newest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sort {
    #[default]
    New,
    Picks,
    Downloads,
}

impl Sort {
    /// The word the relay takes: `new`, `picks` or `downloads`.
    pub fn word(self) -> &'static str {
        match self {
            Sort::New => "new",
            Sort::Picks => "picks",
            Sort::Downloads => "downloads",
        }
    }
}

// ── Requests ───────────────────────────────────────────────────────────────────────────────

/// What an edit sets: every field, as the Edit window shows it (the relay takes nothing less).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EditFields {
    pub title: String,
    pub description: String,
    pub category_id: i64,
    pub labels: Vec<String>,
    pub licence: String,
    pub project_use: bool,
}

/// The `edit` member's text as it is sent and hashed: compact JSON, its keys in name order (what
/// serde_json writes), the title and description trimmed.
pub fn edit_text(e: &EditFields) -> String {
    json!({
        "title": e.title.trim(),
        "description": e.description.trim(),
        "category_id": e.category_id,
        "labels": e.labels,
        "licence": e.licence,
        "project_use": e.project_use,
    })
    .to_string()
}

/// The purpose an edit of item `id` whose `edit` member is `text` is signed over.
pub fn edit_purpose(id: i64, text: &str) -> String {
    format!("media_edit:{id}:{}", blake3::hash(text.as_bytes()).to_hex())
}

/// `POST /api/media/edit`: the edit's text is written once and both hashed into the purpose and
/// put in the body as it is.
pub fn edit_request(signer: &Signer, item: &ItemRef, e: &EditFields) -> Request {
    let text = edit_text(e);
    let purpose = edit_purpose(item.id, &text);
    let auth = signer.signed(&purpose, json!({}));
    let body = format!(
        "{{\"key\":{},\"timestamp\":{},\"sig\":{},\"id\":{},\"filename\":{},\"edit\":{text}}}",
        auth["key"],
        auth["timestamp"],
        auth["sig"],
        item.id,
        Value::from(bare_name(&item.filename))
    );
    Request { method: Method::Post, path: "/api/media/edit".to_string(), body: Some(body), purpose: Some(purpose) }
}

/// What an upload is added to an item as. Kept in the record of an upload a restart cut off
/// ([`AttachTo`]), as the relay's own word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileRole {
    #[default]
    File,
    Cover,
}

impl FileRole {
    pub fn word(self) -> &'static str {
        match self {
            FileRole::File => "file",
            FileRole::Cover => "cover",
        }
    }
}

/// Where an upload goes once it is up, instead of being published as an item of its own (lane
/// 1D): added to item `id`, whose own stored file is `filename` (every write names it, W5), as a
/// file or as its cover. The upload's record carries it (resumable.rs `Unfinished::attach`), so an
/// upload a restart cut off and carried on later still goes to the same item.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AttachTo {
    pub id: i64,
    pub filename: String,
    pub role: FileRole,
}

impl AttachTo {
    /// The item it goes to, as every write names it.
    pub fn item(&self) -> ItemRef {
        ItemRef { id: self.id, filename: self.filename.clone() }
    }
}

/// The number the relay gave the upload stored as `stored` on `item` (an added file or the cover):
/// what a file's Remove names. None when the item does not hold it.
pub fn file_number_of(item: &Item, stored: &str) -> Option<i64> {
    let stored = bare_name(stored);
    item.files.iter().chain(item.cover.iter()).find(|f| f.filename == stored).map(|f| f.file)
}

/// The purpose making item `id` public or members only is signed over (the relay's existing
/// `/api/media/visibility`).
pub fn visibility_purpose(id: i64, members: bool) -> String {
    format!("media_visibility:{id}:{}", if members { "members" } else { "public" })
}

/// `POST /api/media/visibility`: who can see item `item`, sent by the Edit window apart from the
/// edit and only when it changed. The relay moves every one of the item's files with it.
pub fn visibility_request(signer: &Signer, item: &ItemRef, members: bool) -> Request {
    let purpose = visibility_purpose(item.id, members);
    let body = json!({ "id": item.id, "filename": bare_name(&item.filename), "visibility": if members { "members" } else { "public" } });
    post("/api/media/visibility", purpose.clone(), signer.signed(&purpose, body))
}

/// The most characters a card shows of a description's first line, "..." included.
pub const CARD_LINE_MAX: usize = 120;

/// What a card shows of `description`: its first line, at most [`CARD_LINE_MAX`] characters, cut
/// at a word with "..." when it is longer. The item's page shows the whole description.
pub fn first_line(description: &str) -> String {
    let line = description.trim().lines().next().unwrap_or("").trim();
    if line.chars().count() <= CARD_LINE_MAX {
        return line.to_string();
    }
    let cut: String = line.chars().take(CARD_LINE_MAX - 3).collect();
    // Back to the last whole word, unless the line is one word longer than the card.
    let at_word = match cut.rfind(char::is_whitespace) {
        Some(i) if i > 0 => &cut[..i],
        _ => cut.as_str(),
    };
    format!("{}...", at_word.trim_end())
}

/// The item's own page on the server at `base`: `https://<host>/media/<id>`, what Copy link copies
/// (the relay serves it, with its preview for social sites; never `/media?id=`).
pub fn item_link(base: &str, id: i64) -> String {
    format!("{}/media/{id}", base.trim_end_matches('/'))
}

/// Send `req` to the server at `base` like `api::send`, with `header` on it when one is given (the
/// session's upload token, which only counts a members-only item's view). Blocking: call it on a
/// worker thread.
pub fn send_with_header(base: &str, req: &Request, header: Option<(&str, &str)>) -> Result<Value, Refusal> {
    let url = format!("{}{}", base.trim_end_matches('/'), req.path);
    let mut call = match req.method {
        Method::Get => ureq::get(&url),
        Method::Post => ureq::post(&url),
    }
    .timeout(std::time::Duration::from_secs(30));
    if let Some((name, value)) = header {
        call = call.set(name, value);
    }
    let result = match (req.method, req.body.as_deref()) {
        (Method::Get, _) => call.call(),
        (Method::Post, body) => call.set("Content-Type", "application/json").send_string(body.unwrap_or("{}")),
    };
    let resp = result.map_err(|e| match e {
        ureq::Error::Status(code, r) => {
            let text = r.into_string().unwrap_or_default();
            let text = text.trim();
            Refusal::new(code, if text.is_empty() || text.starts_with('<') { format!("The server refused it ({code}).") } else { text.to_string() })
        }
        other => Refusal::new(0, format!("The server could not be reached ({other}).")),
    })?;
    let text = resp.into_string().map_err(|e| Refusal::new(0, format!("The answer could not be read ({e}).")))?;
    serde_json::from_str(&text).map_err(|e| Refusal::new(0, format!("The answer was not understood ({e}).")))
}

/// The purpose adding `add` to item `id` as `role` is signed over.
pub fn file_add_purpose(id: i64, add: &str, role: FileRole) -> String {
    format!("media_file_add:{id}:{}:{}", bare_name(add), role.word())
}

/// `POST /api/media/files/add`: one of this person's uploads waiting on the Media shelf, added to
/// their item as a file or as its cover.
pub fn file_add_request(signer: &Signer, item: &ItemRef, add: &str, role: FileRole) -> Request {
    let purpose = file_add_purpose(item.id, add, role);
    let body = json!({ "id": item.id, "filename": bare_name(&item.filename), "add": bare_name(add), "as": role.word() });
    post("/api/media/files/add", purpose.clone(), signer.signed(&purpose, body))
}

/// The purpose taking file `file` off item `id` is signed over.
pub fn file_remove_purpose(id: i64, file: i64) -> String {
    format!("media_file_remove:{id}:{file}")
}

/// `POST /api/media/files/remove`: an added file or the cover (never file 0), named by its number
/// and its own stored name (W5 for the file).
pub fn file_remove_request(signer: &Signer, item: &ItemRef, file: i64, file_name: &str) -> Request {
    let purpose = file_remove_purpose(item.id, file);
    let body = json!({ "id": item.id, "filename": bare_name(&item.filename), "file": file, "file_name": bare_name(file_name) });
    post("/api/media/files/remove", purpose.clone(), signer.signed(&purpose, body))
}

/// The purpose renaming file `file` of item `id` to `name` is signed over (relay/media_edit.rs
/// `file_rename_purpose`): the name as its BLAKE3 hex, the name exactly as sent.
pub fn file_rename_purpose(id: i64, file: i64, name: &str) -> String {
    format!("media_file_rename:{id}:{file}:{}", blake3::hash(name.as_bytes()).to_hex())
}

/// `POST /api/media/files/rename` (2026-10-17): file `file` (0: the item's own) named `name`, named
/// by its number and its own stored name (W5 for the file). The relay keeps the file's ending.
pub fn file_rename_request(signer: &Signer, item: &ItemRef, file: i64, file_name: &str, name: &str) -> Request {
    let purpose = file_rename_purpose(item.id, file, name);
    let body = json!({ "id": item.id, "filename": bare_name(&item.filename), "file": file, "file_name": bare_name(file_name), "name": name });
    post("/api/media/files/rename", purpose.clone(), signer.signed(&purpose, body))
}

/// The purpose making file `file` item `id`'s cover is signed over.
pub fn file_cover_purpose(id: i64, file: i64) -> String {
    format!("media_file_cover:{id}:{file}")
}

/// `POST /api/media/files/cover` (2026-10-17): picture file `file`, already on the item, made its
/// cover; the old cover becomes one of its files.
pub fn file_cover_request(signer: &Signer, item: &ItemRef, file: i64, file_name: &str) -> Request {
    let purpose = file_cover_purpose(item.id, file);
    let body = json!({ "id": item.id, "filename": bare_name(&item.filename), "file": file, "file_name": bare_name(file_name) });
    post("/api/media/files/cover", purpose.clone(), signer.signed(&purpose, body))
}

/// `POST /api/media/viewed`: unsigned, the item's number alone (the session's upload token, needed
/// only to count a members-only item, goes in the `X-Upload-Token` header where it is sent). The
/// answer is always `{ok: true}`.
pub fn viewed_request(id: i64) -> Request {
    Request { method: Method::Post, path: "/api/media/viewed".to_string(), body: Some(json!({ "id": id }).to_string()), purpose: None }
}

/// Where file `file` of a public item is downloaded, counted: `/api/media/download/<id>?file=<n>`
/// on the server at `base`. A members-only item's file goes through a ticket asked for with
/// `download` ([`ticket_request`]), never with the token in an address.
pub fn download_url(base: &str, item: &Item, file: i64) -> String {
    format!("{}/api/media/download/{}?file={file}", base.trim_end_matches('/'), item.id)
}

/// The body of `POST /api/media/ticket` (round 5): the item, its own file name (W5), which of its
/// files, and whether the address is the counted download route.
pub fn ticket_request(item: &ItemRef, file: i64, download: bool) -> Value {
    json!({ "id": item.id, "filename": bare_name(&item.filename), "file": file, "download": download })
}

#[cfg(test)]
mod tests {
    use super::super::api::{feature_purpose, list_request, parse_list, publish_purpose, Featured, Filters};
    use super::*;

    fn fixture() -> Value {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/tests/fixtures/media-round5-v1.json");
        serde_json::from_str(&std::fs::read_to_string(path).expect("the shared fixture is there")).expect("JSON")
    }

    /// THE DESKTOP READS AND SIGNS THE SHARED FIXTURE (scripts/tests/fixtures/media-round5-v1.json,
    /// which the relay's own code makes): an item parses with its files (file 0 first), cover,
    /// licence, game tick, `featured` as a word, counts and link, and its members-only copy with
    /// every file through the members' route; the list answer's `next_cursor` and licences (the
    /// offered five, the game tick on CC0 and CC BY, the warnings and the hosting paragraph); the
    /// list asks with `sort` and `cursor`; and every purpose this app signs is the relay's, the
    /// edit's over its text exactly as sent, which the relay's own parse of the body agrees with.
    ///
    /// Seen red 2026-10-12 with `edit_request` hashing the edit re-serialised with spacing
    /// (`serde_json::to_string_pretty`) instead of the text sent: "the edit's purpose is the
    /// relay's / left: Some(\"media_edit:1:f795dde7...\") / right:
    /// Some(\"media_edit:1:2cbdf899...\")".
    #[test]
    fn the_desktop_reads_and_signs_the_shared_fixture() {
        let fx = fixture();
        let item = Item::from_json(&fx["item_members_view"]).expect("an item");
        assert_eq!((item.featured, item.licence.as_str(), item.project_use), (Featured::Category, "cc_by_4", true));
        assert_eq!((item.downloads, item.views, item.total_bytes, item.file_count, item.link.as_str()), (3, 5, 9876, 2, "/media/1"));
        assert_eq!(item.files.iter().map(|f| (f.file, f.filename.as_str())).collect::<Vec<_>>(), vec![(0, "1760_x.png"), (1, "1761_y.zip")]);
        assert_eq!(item.files[1].original_name, "textures.zip");
        assert_eq!(item.cover.as_ref().map(|c| (c.filename.as_str(), c.kind.as_str())), Some(("1762_c.jpg", "picture")));
        assert!(item.picked_by_me && item.featured.is_on());
        let members = Item::from_json(&fx["item_members_only"]).expect("the members-only copy");
        assert!(members.members_only());
        assert!(members.files.iter().all(|f| f.url.starts_with("/api/media/file/1?file=")), "{:?}", members.files);

        let list = parse_list(&fx["list_by_downloads"]);
        assert_eq!(list.next_cursor.as_deref(), Some("3.1"));
        let l = &list.licences;
        assert_eq!(l.default, DEFAULT_LICENCE);
        assert_eq!(l.offered.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), vec!["all_rights_reserved", "cc_by_4", "cc_by_sa_4", "cc_by_nc_4", "cc0_1"]);
        assert_eq!(l.offered.iter().filter(|x| x.game).map(|x| x.id.as_str()).collect::<Vec<_>>(), vec!["cc_by_4", "cc0_1"]);
        assert_eq!((l.game_label.as_str(), l.hosting_heading.as_str()), ("Offer this for the game", "Your uploads stay yours."));
        assert!(!l.cc_warning.is_empty() && !l.cc_members_warning.is_empty() && l.hosting_text.starts_with("When you put a file"));
        assert_eq!(Licences::from_json(None), Licences::default(), "no licences: All rights reserved alone");

        let downloads = Filters { sort: Sort::Downloads, ..Default::default() };
        let get = list_request(&downloads, Some("3.1"), 1, "me", None);
        assert!(get.path.contains("sort=downloads") && get.path.contains("cursor=3.1"), "{}", get.path);
        assert!(!list_request(&Filters::default(), None, 1, "me", None).path.contains("sort="), "newest first is not sent");

        let p = &fx["purposes"];
        assert_eq!(publish_purpose("1760_x.png", "cc_by_4", true), p["publish"]);
        assert_eq!(publish_purpose("/uploads/1763_z.glb", "", false), p["publish_default"], "the default licence");
        assert_eq!(feature_purpose(1, Featured::Main), p["feature_main"]);
        assert_eq!(feature_purpose(1, Featured::Category), p["feature_category"]);
        assert_eq!(feature_purpose(1, Featured::None), p["feature_none"]);
        assert_eq!(file_add_purpose(1, "1761_y.zip", FileRole::File), p["file_add"]);
        assert_eq!(file_add_purpose(1, "1762_c.jpg", FileRole::Cover), p["cover_add"]);
        assert_eq!(file_remove_purpose(1, 1), p["file_remove"]);

        let edit = EditFields {
            title: "Dusk".into(),
            description: "First line\nThe rest".into(),
            category_id: 2,
            labels: vec!["flashing".into()],
            licence: "cc_by_4".into(),
            project_use: true,
        };
        assert_eq!(edit_text(&edit), fx["edit"]["text"].as_str().unwrap(), "the edit's text, byte for byte");
        let signer = Signer::new("a1".repeat(32).as_str(), &[7u8; 32]);
        let req = edit_request(&signer, &ItemRef { id: 1, filename: "1760_x.png".into() }, &edit);
        assert_eq!(req.purpose.as_deref(), fx["edit"]["purpose"].as_str(), "the edit's purpose is the relay's");
        let body = req.body.as_deref().unwrap();
        let sent = crate::relay::handlers::reports::raw_member(body, "edit").expect("an edit member");
        assert_eq!(sent, fx["edit"]["text"].as_str().unwrap(), "the text sent is the text hashed");
        let (_, _, relays) = crate::relay::media_edit::edit_request(body).expect("the relay reads the body");
        assert_eq!(Some(relays.as_str()), req.purpose.as_deref(), "the relay checks the same purpose for it");
        assert_eq!(ticket_request(&ItemRef { id: 1, filename: "1760_x.png".into() }, 1, true), json!({ "id": 1, "filename": "1760_x.png", "file": 1, "download": true }));
    }

    /// A RENAME'S AND A COVER'S PURPOSES ARE THE RELAY'S (2026-10-17): the relay reads each body
    /// and rebuilds the same purpose from it, the new name's hash included, so a signature made
    /// here is one it accepts. Seen red 2026-10-17 with the name left out of the desktop's purpose:
    /// "the relay checks the same purpose for a rename" (left: "media_file_rename:7:1:<hash>").
    #[test]
    fn a_renames_and_a_covers_purposes_are_the_relays() {
        let signer = Signer::new("a1".repeat(32).as_str(), &[7u8; 32]);
        let item = ItemRef { id: 7, filename: "/uploads/7_dusk.glb".into() };
        let rename = file_rename_request(&signer, &item, 1, "/uploads/8_textures.zip", "skin.zip");
        let body: crate::relay::media_edit::FileRenameBody = serde_json::from_str(rename.body.as_deref().unwrap()).unwrap();
        assert_eq!((body.id, body.filename.as_str(), body.file, body.file_name.as_str(), body.name.as_str()), (7, "7_dusk.glb", 1, "8_textures.zip", "skin.zip"), "the body names the item, the file and the name");
        let relays = crate::relay::media_edit::file_rename_purpose(body.id, body.file, &body.name);
        assert_eq!(Some(relays.as_str()), rename.purpose.as_deref(), "the relay checks the same purpose for a rename");
        let cover = file_cover_request(&signer, &item, 2, "9_shot.png");
        let body: crate::relay::media_edit::FileCoverBody = serde_json::from_str(cover.body.as_deref().unwrap()).unwrap();
        assert_eq!((body.id, body.file, body.file_name.as_str()), (7, 2, "9_shot.png"), "the body names the picture");
        let relays = crate::relay::media_edit::file_cover_purpose(body.id, body.file);
        assert_eq!(Some(relays.as_str()), cover.purpose.as_deref(), "the relay checks the same purpose for a cover");
    }
}
