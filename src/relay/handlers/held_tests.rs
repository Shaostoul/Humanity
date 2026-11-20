// Child of held.rs (#[path]): hide and keep, driven through a report's decision against a real
// database, real Dilithium3 identities and a test's own upload folders (docs/design/
// hide-and-keep.md, the Proof column).

use super::*;
use crate::relay::handlers::reports::handle_decide_in;
use crate::relay::relay::Peer;
use crate::relay::storage::files::Folders;
use crate::relay::storage::held::{held_dir_beside, NO_CARD, NO_OWNER};
use crate::relay::storage::Storage;
use humanity_protocol::key_card;
use humanity_protocol::pq_crypto::{build_report_sig, derive_dilithium_seed, DilithiumKeypair};
use serde_json::json;

pub(super) fn block<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Runtime::new().expect("tokio rt").block_on(f)
}

/// A real identity: its seed and its Dilithium3 public key, hex.
pub(super) struct Person {
    pub seed: Vec<u8>,
    pub key: String,
}

pub(super) fn person(seed_byte: u8) -> Person {
    let seed = vec![seed_byte; 32];
    let key = hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed)).public_key());
    Person { seed, key }
}

/// One folder of a test's own, with the relay's four upload folders inside it, so the held folder
/// beside the uploads folder is the test's own too.
pub(super) struct Dirs {
    _root: humanity_core::test_temp::TempPath,
    pub uploads: std::path::PathBuf,
    pub members: std::path::PathBuf,
    pub resume: std::path::PathBuf,
    pub waiting: std::path::PathBuf,
}

pub(super) fn dirs(tag: &str) -> Dirs {
    let root = humanity_core::test_temp::dir(&format!("hidekeep_{tag}"));
    let sub = |w: &str| {
        let p = root.path().join(w);
        std::fs::create_dir_all(&p).unwrap();
        p
    };
    let (uploads, members, resume, waiting) = (sub("uploads"), sub("media-members"), sub("uploads-resume"), sub("media-waiting"));
    Dirs { _root: root, uploads, members, resume, waiting }
}

impl Dirs {
    pub fn folders(&self) -> Folders<'_> {
        Folders { uploads: &self.uploads, members: &self.members, resume: &self.resume, waiting: &self.waiting }
    }
}

pub(super) const TS: u64 = 1_760_000_000_000;
/// A marker in the post's words and each file, searched for on disk after the hide.
pub(super) const MARKER: &str = "PLAINTEXT-MARKER-7f3a";

/// A server with Tom's reported post in #global, shared with linked servers. The post links a
/// picture and a video with its poster picture in the public folder and a members-only file, each
/// with its upload row; the post is pinned. Rae reported it. Ada is an admin. Returns the state,
/// the report's id, and the people (Rae, Tom, Ada).
pub(super) fn reported_post(tag: &str, d: &Dirs) -> (Arc<RelayState>, i64, Person, Person, Person) {
    let st = Arc::new(RelayState::new(Storage::open_temp(&format!("hidekeep_{tag}"))));
    let (rae, tom, ada) = (person(1), person(2), person(3));
    st.db.register_name("Rae", &rae.key).unwrap();
    st.db.register_name("Tom", &tom.key).unwrap();
    st.db.set_role(&ada.key, "admin").unwrap();
    // #global open to anyone and shared, as a server makes it: only such a room travels.
    let _ = st.db.get_or_create_server_keypair().unwrap();
    let _ = st.db.create_channel("global", "global", None, "test", humanity_protocol::chat_access::PostLevel::Members);
    let _ = st.db.set_channel_access("global", humanity_protocol::chat_access::ReadLevel::Anyone, humanity_protocol::chat_access::PostLevel::Members);
    assert!(st.db.set_channel_federated("global", true).unwrap(), "#global is shared");
    block(async {
        st.peers.write().await.insert(
            tom.key.clone(),
            Peer { public_key_hex: tom.key.clone(), display_name: Some("Tom".into()), upload_token: None, conn_id: 1 },
        );
    });
    std::fs::write(d.uploads.join("pic_1.png"), format!("{MARKER} picture")).unwrap();
    std::fs::write(d.uploads.join("vid_2.mp4"), format!("{MARKER} video")).unwrap();
    std::fs::write(d.uploads.join("vid_2.mp4.poster.jpg"), format!("{MARKER} poster")).unwrap();
    std::fs::write(d.members.join("doc_3.pdf"), format!("{MARKER} members")).unwrap();
    for (name, shelf) in [("pic_1.png", 0), ("vid_2.mp4", 0), ("doc_3.pdf", 2)] {
        st.db.record_upload(&tom.key, name, 50, shelf, name, 20).unwrap();
    }
    let content = format!("{MARKER} look /uploads/pic_1.png and /uploads/vid_2.mp4 and /uploads/doc_3.pdf");
    st.db
        .with_conn(|c| {
            c.execute(
                "INSERT INTO messages (msg_type, from_key, from_name, content, timestamp, raw_json, channel_id) VALUES ('chat', ?1, 'Tom', ?2, 42, '{}', 'global')",
                rusqlite::params![tom.key, content],
            )
        })
        .unwrap();
    st.db.pin_message("global", &tom.key, "Tom", &content, 42, &ada.key).unwrap();
    let evidence = json!([{ "kind": "post", "from": tom.key, "timestamp": 42 }]);
    let sig = build_report_sig(&rae.seed, &rae.key, &tom.key, "child_danger", &evidence.to_string(), TS);
    let v = json!({ "type": "report_v2", "target": tom.key, "context": "post", "reason": "child_danger", "note": "", "evidence": evidence, "ts": TS, "sig": sig });
    let mut rx = st.broadcast_tx.subscribe();
    block(crate::relay::handlers::reports::handle(&st, &rae.key, "report_v2", &v, &v.to_string()));
    let id = drain(&mut rx)
        .into_iter()
        .find_map(|m| match m {
            RelayMessage::ReportReceived { to, id } if to == rae.key => Some(id),
            _ => None,
        })
        .expect("the report is received");
    (st, id, rae, tom, ada)
}

/// Make `who` the server's owner, with a key card kept here when `card`.
pub(super) fn make_owner(st: &RelayState, who: &Person, card: bool) {
    st.db.set_role(&who.key, "owner").unwrap();
    if card {
        let c = key_card::sign_card(&who.seed, TS).unwrap();
        assert!(st.db.store_key_card(&c).unwrap().stored());
    }
}

pub(super) fn drain(rx: &mut tokio::sync::broadcast::Receiver<RelayMessage>) -> Vec<RelayMessage> {
    let mut all = Vec::new();
    while let Ok(m) = rx.try_recv() {
        all.push(m);
    }
    all
}

/// `who` decides report `id` with `decision` (a hide and keep with `who`'s own signed action, as
/// the app sends it): everything the bus carried.
pub(super) fn decide(st: &Arc<RelayState>, d: &Dirs, who: &Person, id: i64, decision: &str) -> Vec<RelayMessage> {
    let mut rx = st.broadcast_tx.subscribe();
    let mut raw = json!({ "type": "report_decide", "id": id, "decision": decision, "note": "" });
    if decision == "hide_and_keep" {
        raw["action"] = signed(st, who, "hide_and_keep", "report", id, 0);
    }
    block(handle_decide_in(st, &who.key, &raw, d.folders()));
    drain(&mut rx)
}

/// What `who` was told, as text.
pub(super) fn told(all: &[RelayMessage], who: &str) -> String {
    all.iter()
        .filter_map(|m| match m {
            RelayMessage::Private { to, message } if to == who => Some(message.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

/// What `GET /uploads/<name>` answers: the public folder served as relay/mod.rs serves it.
pub(super) fn public_status(d: &Dirs, name: &str) -> u16 {
    let req = axum::http::Request::builder().uri(format!("/{name}")).body(axum::body::Body::empty()).unwrap();
    block(async { tower_http::services::ServeDir::new(&d.uploads).try_call(req).await.unwrap().status().as_u16() })
}

fn upload_rows(st: &RelayState) -> i64 {
    st.db.with_conn(|c| c.query_row("SELECT COUNT(*) FROM user_uploads", [], |r| r.get(0))).unwrap()
}

fn outbox_kinds(st: &RelayState) -> Vec<String> {
    st.db.fed_outbox_after(0, 100).unwrap().into_iter().map(|r| r.kind).collect()
}

/// Every file of the post is where it was, and the post is still shown.
fn untouched(st: &RelayState, d: &Dirs, tom: &Person) {
    assert!(st.db.post_for_report(&tom.key, 42).unwrap().is_some(), "the post is still there");
    assert_eq!(public_status(d, "pic_1.png"), 200);
    assert!(d.members.join("doc_3.pdf").is_file());
    assert_eq!(upload_rows(st), 3);
}

/// Rung 1: the hide refused with the design's words when the server cannot seal to its owner
/// (no owner; an owner with no key card), and nothing changed; then, with the owner's card here,
/// the post leaves every view: its row and its pinned copy, every client told to drop it, the room's
/// pins sent again, the delete queued for linked servers, each file out of every served folder (a
/// video's poster picture too) and no longer answered at `/uploads/<name>`, and their upload rows.
///
/// Seen red 2026-11-20 with `hide_post_in`'s `folders.delete` taken out: "pic_1.png is in no
/// served folder"; and with the `after_delete` call taken out of `hide_and_keep_in`: "the delete is
/// queued for linked servers" (the outbox held no delete).
#[test]
fn hide_and_keep_takes_the_post_and_its_files_out_of_every_view() {
    let d = dirs("hide_view");
    let (st, id, _rae, tom, ada) = reported_post("hide_view", &d);
    let own = person(9);

    let all = decide(&st, &d, &ada, id, "hide_and_keep");
    assert!(told(&all, &ada.key).contains(NO_OWNER), "no owner: {}", told(&all, &ada.key));
    untouched(&st, &d, &tom);
    make_owner(&st, &own, false);
    let all = decide(&st, &d, &ada, id, "hide_and_keep");
    assert!(told(&all, &ada.key).contains(NO_CARD), "no card: {}", told(&all, &ada.key));
    untouched(&st, &d, &tom);
    assert_eq!(st.db.report_by_id(id).unwrap().unwrap().state, "open", "a refused decision decides nothing");

    make_owner(&st, &own, true);
    let all = decide(&st, &d, &ada, id, "hide_and_keep");
    let words = told(&all, &ada.key);
    assert!(words.contains("decided: hide_and_keep") && words.contains("1 post(s) and 3 file(s)"), "{words}");
    assert!(st.db.post_for_report(&tom.key, 42).unwrap().is_none(), "the post's row is gone");
    assert!(st.db.get_pinned_messages("global").unwrap().is_empty(), "its pinned copy is gone");
    assert!(all.iter().any(|m| matches!(m, RelayMessage::Delete { from, timestamp: 42 } if *from == tom.key)), "every client is told to drop it");
    assert!(all.iter().any(|m| matches!(m, RelayMessage::PinsSync { channel, pins } if channel == "global" && pins.is_empty())), "the room's pins are sent again");
    assert!(outbox_kinds(&st).iter().any(|k| k == "delete"), "the delete is queued for linked servers: {:?}", outbox_kinds(&st));
    for name in ["pic_1.png", "vid_2.mp4", "vid_2.mp4.poster.jpg", "doc_3.pdf"] {
        assert!(d.folders().spot_of(name).is_none() && !d.uploads.join(name).exists(), "{name} is in no served folder");
        assert_eq!(public_status(&d, name), 404, "{name} is no longer served");
    }
    assert_eq!(upload_rows(&st), 0, "their upload rows are gone");
    for served in [&d.uploads, &d.members, &d.waiting] {
        assert!(!held_dir_beside(d.folders()).starts_with(served), "the held folder is inside no served folder");
    }
    assert_eq!(st.db.report_by_id(id).unwrap().unwrap().decision.as_deref(), Some("hide_and_keep"));
}

/// "Delete the post" sends its delete to linked servers too (the design's known limit, fixed with
/// rung 1): before 2026-11-20 it deleted the post here only.
///
/// Seen red 2026-11-20 against the decision as it was (no `after_delete`): the outbox held no
/// delete.
#[test]
fn delete_the_post_also_reaches_linked_servers() {
    let d = dirs("delete_fed");
    let (st, id, _rae, tom, ada) = reported_post("delete_fed", &d);
    decide(&st, &d, &ada, id, "delete_post");
    assert!(st.db.post_for_report(&tom.key, 42).unwrap().is_none());
    assert!(outbox_kinds(&st).iter().any(|k| k == "delete"), "{:?}", outbox_kinds(&st));
}

/// A moderator may hide and keep a member's post, not an admin's (the moderation rules hold), and
/// the owner decides reports like any admin (BUG-330).
///
/// Seen red 2026-11-20 with `mod_action_refusal` reading "admin" alone: the owner was told "You
/// don't have permission to perform moderation actions."
#[test]
fn the_moderation_rules_hold_and_the_owner_decides() {
    let d = dirs("rules");
    let (st, id, _rae, tom, _ada) = reported_post("rules", &d);
    let own = person(9);
    make_owner(&st, &own, true);
    st.db.set_role(&tom.key, "admin").unwrap();
    let mo = person(5);
    st.db.set_role(&mo.key, "mod").unwrap();
    let all = decide(&st, &d, &mo, id, "hide_and_keep");
    assert!(told(&all, &mo.key).contains("Only an admin can act on another admin"), "{}", told(&all, &mo.key));
    assert!(st.db.post_for_report(&tom.key, 42).unwrap().is_some());
    let all = decide(&st, &d, &own, id, "hide_and_keep");
    assert!(told(&all, &own.key).contains("decided: hide_and_keep"), "{}", told(&all, &own.key));
}

// ── Rung 2: the seal ──

/// Every file under `dir`, at any depth.
fn every_file(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            every_file(&p, out);
        } else {
            out.push(p);
        }
    }
}

/// The one held item's package id, file count, parts, held time, delete time and sealed-to key.
fn held_row(st: &RelayState) -> (String, i64, i64, i64, i64, String) {
    st.db
        .with_conn(|c| c.query_row("SELECT package, file_count, parts, held_at, delete_after, sealed_to FROM held_items", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))))
        .expect("one held item")
}

/// Rung 2: the kept copy is sealed to the owner's card. The held item's row names the package, the
/// three files in five parts (the words, three files, a poster picture), a delete time a year on
/// and the owner; no file anywhere under the test's folders holds the plaintext marker (the post's
/// words and every file carry it); the package opens with the owner's recovery phrase, part 0 the
/// post's words and its list of files, each part byte for byte the file it holds; and no part opens
/// with any other key.
///
/// Seen red 2026-11-20 with `seal_file` copying the file instead of sealing it: "no file on disk
/// holds the plaintext" named the first part.
#[test]
fn the_kept_copy_is_sealed_to_the_owner_and_nothing_readable_is_left() {
    use humanity_protocol::held_seal::{open_bytes, part_context, Manifest};
    use humanity_protocol::pq_crypto::{derive_kyber_seed, KyberKeypair};
    let d = dirs("sealed");
    let (st, id, _rae, tom, ada) = reported_post("sealed", &d);
    let own = person(9);
    make_owner(&st, &own, true);
    let all = decide(&st, &d, &ada, id, "hide_and_keep");
    assert!(told(&all, &ada.key).contains("kept sealed to the owner's key card"), "{}", told(&all, &ada.key));
    let (package, files, parts, held_at, delete_after, sealed_to) = held_row(&st);
    assert_eq!((files, parts), (3, 5));
    assert_eq!(delete_after - held_at, 365 * 86_400_000, "deleted a year after it was held");
    assert_eq!(sealed_to, own.key);

    let mut all_files = Vec::new();
    every_file(d.uploads.parent().unwrap(), &mut all_files);
    assert!(all_files.len() >= 5, "the parts are on disk: {all_files:?}");
    for f in &all_files {
        let bytes = std::fs::read(f).unwrap();
        assert!(!bytes.windows(MARKER.len()).any(|w| w == MARKER.as_bytes()), "no file on disk holds the plaintext: {}", f.display());
    }

    let pkg = held_dir_beside(d.folders()).join(&package);
    let part = |n: usize| std::fs::read(pkg.join(format!("{n}.sealed"))).unwrap();
    let kp = KyberKeypair::from_seed(&derive_kyber_seed(&own.seed)).unwrap();
    let m: Manifest = serde_json::from_slice(&open_bytes(&kp, &part_context(&package, 0), &part(0)).unwrap()).unwrap();
    assert!(m.words.starts_with(MARKER) && m.author == tom.key && m.stamp == 42 && m.report_id == id && m.channel == "global");
    assert_eq!(m.held_by, ada.key);
    let names: Vec<(&str, &str)> = m.files.iter().map(|f| (f.name.as_str(), f.folder.as_str())).collect();
    assert_eq!(names, [("pic_1.png", "public"), ("vid_2.mp4", "public"), ("doc_3.pdf", "members")]);
    assert_eq!((m.files[0].uploader.as_str(), m.files[2].shelf), (tom.key.as_str(), 2));
    for (f, want) in m.files.iter().zip(["picture", "video", "members"]) {
        assert_eq!(open_bytes(&kp, &part_context(&package, f.part), &part(f.part)).unwrap(), format!("{MARKER} {want}").into_bytes(), "{}", f.name);
    }
    let poster = m.files[1].poster_part.expect("the video's poster picture is kept");
    assert_eq!(open_bytes(&kp, &part_context(&package, poster), &part(poster)).unwrap(), format!("{MARKER} poster").into_bytes());
    let other = KyberKeypair::from_seed(&derive_kyber_seed(&[8u8; 32])).unwrap();
    for n in 0..5 {
        assert!(open_bytes(&other, &part_context(&package, n), &part(n)).is_err(), "part {n} opens for nobody else");
    }
}

/// A seal that cannot be written (here the held folder cannot be made) hides nothing and keeps
/// nothing: the admin is told, the post and its files stay where they were, the report stays open.
/// And a held file that a crash left in a served folder is taken out by the expiry pass.
///
/// Seen red 2026-11-20 with the originals deleted before the seal (the order a careless edit
/// gives): "the post is still there" held, but `/uploads/pic_1.png` answered 404 (left 404, right
/// 200).
#[test]
fn a_failed_seal_changes_nothing_and_a_file_left_behind_is_swept() {
    let d = dirs("seal_fails");
    let (st, id, _rae, tom, ada) = reported_post("seal_fails", &d);
    let own = person(9);
    make_owner(&st, &own, true);
    std::fs::write(held_dir_beside(d.folders()), "a file where the held folder goes").unwrap();
    let all = decide(&st, &d, &ada, id, "hide_and_keep");
    assert!(told(&all, &ada.key).contains("a sealed copy could not be kept"), "{}", told(&all, &ada.key));
    untouched(&st, &d, &tom);
    assert_eq!(st.db.report_by_id(id).unwrap().unwrap().state, "open");
    let held: i64 = st.db.with_conn(|c| c.query_row("SELECT COUNT(*) FROM held_items", [], |r| r.get(0))).unwrap();
    assert_eq!(held, 0, "nothing kept");

    std::fs::remove_file(held_dir_beside(d.folders())).unwrap();
    decide(&st, &d, &ada, id, "hide_and_keep");
    assert_eq!(public_status(&d, "pic_1.png"), 404);
    std::fs::write(d.uploads.join("pic_1.png"), "left behind by a crash").unwrap();
    st.db.run_expiry_sweeps_in(d.folders());
    assert_eq!(public_status(&d, "pic_1.png"), 404, "the expiry pass takes a held file out of a served folder");
}

// ── Rung 3: a year, a legal hold, destroy now ──

/// A server with one held item (Tom's post): the state, the report's id, Ada, the owner and the
/// item's row.
fn one_held(tag: &str, d: &Dirs) -> (Arc<RelayState>, i64, Person, Person, crate::relay::storage::held::HeldRow) {
    let (st, id, _rae, _tom, ada) = reported_post(tag, d);
    let own = person(9);
    make_owner(&st, &own, true);
    decide(&st, d, &ada, id, "hide_and_keep");
    let row = st.db.held_items().unwrap().pop().expect("one held item");
    (st, id, ada, own, row)
}

/// Rung 3: the expiry pass keeps a held item at day 364 and deletes it at day 366, its sealed parts
/// from the disk and its row; a legal hold past day 366 keeps it until the hold ends; the report
/// that held it is kept while it is (the 90-day cull and `/reports-clear` pass it over), and goes by
/// the ordinary rule once it is gone.
///
/// Seen red 2026-11-20 with the expiry pass's `legal_hold_until` condition taken out: "a legal
/// hold keeps it past its year" (the item was deleted at day 366).
#[test]
fn a_held_item_goes_after_a_year_unless_a_legal_hold_keeps_it() {
    use crate::relay::storage::held::DAY_MS;
    let d = dirs("expiry");
    let (st, id, _ada, _own, row) = one_held("expiry", &d);
    let pkg = held_dir_beside(d.folders()).join(&row.package);
    assert!(pkg.is_dir());
    let day = |n: i64| row.held_at + n * DAY_MS;

    assert_eq!(st.db.held_expire_in(day(364), d.folders()).unwrap(), 0, "kept at day 364");
    assert!(pkg.is_dir());
    // The report outlives the 90 days after its decision, and a clear, while its item is held.
    assert_eq!(st.db.reports_expire(day(200)).unwrap(), 0);
    assert_eq!(st.db.reports_clear_decided().unwrap(), 0);
    assert!(st.db.report_by_id(id).unwrap().is_some(), "the report is kept while its item is");

    assert!(st.db.held_set_legal_hold(row.id, day(400)).unwrap());
    assert_eq!(st.db.held_expire_in(day(366), d.folders()).unwrap(), 0, "a legal hold keeps it past its year");
    assert!(pkg.is_dir());
    assert_eq!(st.db.held_expire_in(day(401), d.folders()).unwrap(), 1, "gone once the hold ends");
    assert!(!pkg.exists(), "its sealed parts are deleted from the disk");
    assert!(st.db.held_items().unwrap().is_empty(), "and its row");
    assert_eq!(st.db.reports_clear_decided().unwrap(), 1, "then the report goes by the ordinary rule");

    // Without a hold: day 366 deletes.
    let d2 = dirs("expiry_plain");
    let (st2, _id2, _a, _o, row2) = one_held("expiry_plain", &d2);
    assert_eq!(st2.db.held_expire_in(row2.held_at + 366 * DAY_MS, d2.folders()).unwrap(), 1, "deleted at day 366");
    assert!(!held_dir_beside(d2.folders()).join(&row2.package).exists());
}

/// Destroy now deletes the item at once, parts and row; a legal hold's end is refused when it is
/// not after today or more than a year ahead, and 0 lifts it.
///
/// Seen red 2026-11-20 with `held_destroy_in` deleting the row and not the folder: "destroy now
/// deletes its sealed parts".
#[test]
fn destroy_now_deletes_at_once_and_a_hold_has_bounds() {
    use crate::relay::storage::held::{legal_hold_refusal, DAY_MS};
    let d = dirs("destroy");
    let (st, _id, _ada, _own, row) = one_held("destroy", &d);
    let now = row.held_at;
    assert!(legal_hold_refusal(now, now).is_some() && legal_hold_refusal(now + 367 * DAY_MS, now).is_some());
    assert!(legal_hold_refusal(now + 90 * DAY_MS, now).is_none() && legal_hold_refusal(0, now).is_none());
    assert!(st.db.held_destroy_in(row.id, d.folders()).unwrap());
    assert!(!held_dir_beside(d.folders()).join(&row.package).exists(), "destroy now deletes its sealed parts");
    assert!(st.db.held_item(row.id).unwrap().is_none(), "and its row");
    assert!(!st.db.held_destroy_in(row.id, d.folders()).unwrap(), "a second destroy finds nothing");
}

// ── Rung 4: every action signed, recorded, checkable ──

/// `who`'s own signed `mod_action_v1` for this server, as the JSON a client sends under `action`.
pub(super) fn signed(st: &RelayState, who: &Person, action: &str, kind: &str, target: i64, until: u64) -> serde_json::Value {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    use humanity_protocol::moderation::{build_mod_action_v1, ModAction};
    let a = ModAction { action, target: &target.to_string(), target_kind: kind, reason: "the content policy", rule: "", expires_at: until, role: "" };
    let now = crate::relay::storage::now_millis();
    let o = build_mod_action_v1(&who.seed, &st.db.server_did().unwrap(), &a, now, &[]).unwrap();
    json!({
        "protocol_version": o.protocol_version, "object_type": o.object_type, "space_id": o.space_id, "channel_id": o.channel_id,
        "author_public_key_b64": B64.encode(&o.author_public_key), "created_at": o.created_at, "references": o.references,
        "payload_schema_version": o.payload_schema_version, "payload_encoding": o.payload_encoding,
        "payload_b64": B64.encode(&o.payload), "signature_b64": B64.encode(&o.signature),
    })
}

/// `who` sends `held_act` carrying `action`: everything the bus carried.
fn act(st: &Arc<RelayState>, d: &Dirs, who: &Person, action: serde_json::Value) -> Vec<RelayMessage> {
    let mut rx = st.broadcast_tx.subscribe();
    block(handle_act_in(st, &who.key, &json!({ "type": "held_act", "action": action }), d.folders()));
    drain(&mut rx)
}

/// Rung 4: hide and keep refused without a signed action, or with one signed by another key than
/// the one deciding; done with the decider's own, and recorded with the signed object as it came,
/// which verifies and names the decider; the same object refused a second time. The owner's legal
/// hold, export and destroy each signed and recorded; an admin who is not the owner refused; the
/// export grants the owner's device the sealed parts for ten minutes and nothing to another grant;
/// the public counts say how many of each, and nothing else.
///
/// Seen red 2026-11-20 with `check_held_action`'s signer check taken out: "an action signed by
/// another key is refused" (another admin's object, sent from Ada's socket, hid the post). (A
/// member's object is refused by the role check as well, so the test uses an admin's.)
#[test]
fn every_held_action_is_signed_recorded_and_checkable() {
    use crate::relay::storage::held::DAY_MS;
    let d = dirs("signed");
    let (st, id, _rae, tom, ada) = reported_post("signed", &d);
    let own = person(9);
    make_owner(&st, &own, true);

    let raw = |action: Option<serde_json::Value>| {
        let mut v = json!({ "type": "report_decide", "id": id, "decision": "hide_and_keep", "note": "" });
        if let Some(a) = action {
            v["action"] = a;
        }
        v
    };
    let decide_raw = |who: &Person, v: serde_json::Value| {
        let mut rx = st.broadcast_tx.subscribe();
        block(handle_decide_in(&st, &who.key, &v, d.folders()));
        drain(&mut rx)
    };
    let all = decide_raw(&ada, raw(None));
    assert!(told(&all, &ada.key).contains(UNSIGNED), "{}", told(&all, &ada.key));
    // Another admin's own signed action, sent from Ada's socket: it would be recorded as theirs.
    let bo = person(4);
    st.db.set_role(&bo.key, "admin").unwrap();
    let all = decide_raw(&ada, raw(Some(signed(&st, &bo, "hide_and_keep", "report", id, 0))));
    assert!(told(&all, &ada.key).contains("signed by another key"), "an action signed by another key is refused: {}", told(&all, &ada.key));
    assert!(st.db.post_for_report(&tom.key, 42).unwrap().is_some(), "nothing done");
    let mine = signed(&st, &ada, "hide_and_keep", "report", id, 0);
    let all = decide_raw(&ada, raw(Some(mine.clone())));
    assert!(told(&all, &ada.key).contains("decided: hide_and_keep"), "{}", told(&all, &ada.key));

    let log = st.db.held_actions(10).unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!((log[0].action.as_str(), log[0].target, log[0].signer_key.as_str()), ("hide_and_keep", id, ada.key.as_str()));
    let sub: SignedObjectSubmission = serde_json::from_str(&log[0].object).unwrap();
    let o = sub.to_object().unwrap();
    assert!(o.verify_signature().is_ok() && hex::encode(&o.author_public_key) == ada.key, "the record verifies and names its signer");
    let replay = st.db.check_held_action(&o, &ada.key, crate::relay::storage::now_millis() as i64);
    assert!(replay.is_err_and(|e| e.contains("already carried out")), "the same action is refused a second time");

    let item = st.db.held_items().unwrap().pop().unwrap();
    let all = act(&st, &d, &ada, signed(&st, &ada, "held_legal_hold", "held", item.id, (item.held_at + 400 * DAY_MS) as u64));
    assert!(told(&all, &ada.key).contains("only this server's owner"), "{}", told(&all, &ada.key));
    let all = act(&st, &d, &own, signed(&st, &own, "held_legal_hold", "held", item.id, (crate::relay::storage::now_millis() as i64 + 90 * DAY_MS) as u64));
    assert!(told(&all, &own.key).contains("under a legal hold"), "{}", told(&all, &own.key));
    assert!(st.db.held_item(item.id).unwrap().unwrap().legal_hold_until > 0);

    let export = signed(&st, &own, "held_export", "held", item.id, 0);
    let all = act(&st, &d, &own, export.clone());
    assert!(told(&all, &own.key).contains("Export of held item"), "{}", told(&all, &own.key));
    let grant = {
        let sub: SignedObjectSubmission = serde_json::from_value(export).unwrap();
        sub.to_object().unwrap().object_id().unwrap().to_hex()
    };
    let now = crate::relay::storage::now_millis() as i64;
    let path = export_part_path(&st, item.id, 0, &grant, d.folders(), now).expect("the owner's grant reaches part 0");
    assert!(path.is_file());
    assert!(export_part_path(&st, item.id, item.parts, &grant, d.folders(), now).is_none(), "no part past the last");
    assert!(export_part_path(&st, item.id, 0, &"0".repeat(64), d.folders(), now).is_none(), "nothing to another grant");
    assert!(export_part_path(&st, item.id, 0, &grant, d.folders(), now + EXPORT_GRANT_MS + 1).is_none(), "nothing after ten minutes");
    assert_eq!(st.db.held_item(item.id).unwrap().unwrap().exported_count, 1);

    let all = act(&st, &d, &own, signed(&st, &own, "held_destroy", "held", item.id, 0));
    assert!(told(&all, &own.key).contains("destroyed"), "{}", told(&all, &own.key));
    assert!(st.db.held_item(item.id).unwrap().is_none() && !path.exists());

    let counts = st.db.held_counts().unwrap();
    assert_eq!((counts.held, counts.legal_holds, counts.exported, counts.destroyed, counts.restored), (1, 1, 1, 1, 0));
    let public = serde_json::to_string(&counts).unwrap();
    assert!(!public.contains(&tom.key) && !public.contains(&ada.key), "the counts carry no details: {public}");
}

/// A held action never enters the public signed-object store (from a submission or a linked
/// server), so it is never served by `/api/v2/objects` nor gossiped; and the held items list is for
/// admins, with no contents.
///
/// Seen red 2026-11-20 with `put_signed_object`'s held-action refusal taken out: "the public store
/// refuses a held action" (it was stored).
#[test]
fn held_actions_stay_out_of_the_public_store_and_the_list_shows_no_contents() {
    let d = dirs("public");
    let (st, id, rae, _tom, ada) = reported_post("public", &d);
    let own = person(9);
    make_owner(&st, &own, true);
    let sub: SignedObjectSubmission = serde_json::from_value(signed(&st, &ada, "hide_and_keep", "report", id, 0)).unwrap();
    assert!(st.db.put_signed_object(&sub.to_object().unwrap(), None).is_err(), "the public store refuses a held action");
    let mut v = json!({ "type": "report_decide", "id": id, "decision": "hide_and_keep", "note": "" });
    v["action"] = signed(&st, &ada, "hide_and_keep", "report", id, 0);
    block(handle_decide_in(&st, &ada.key, &v, d.folders()));

    let mut rx = st.broadcast_tx.subscribe();
    handle_list(&st, &rae.key);
    handle_list(&st, &ada.key);
    let all = drain(&mut rx);
    assert!(told(&all, &rae.key).contains("Only admins"), "a member is refused");
    let list = all
        .iter()
        .find_map(|m| match m {
            RelayMessage::HeldItems { to, .. } if *to == ada.key => Some(serde_json::to_string(m).unwrap()),
            _ => None,
        })
        .expect("the admin is sent the list");
    assert!(list.contains("\"type\":\"held_items\"") && list.contains("\"report_id\":1") && list.contains("\"can_keep\":true"), "{list}");
    assert!(!list.contains(MARKER) && !list.contains("pic_1.png") && !list.contains("package"), "no contents, no names, no package: {list}");
}

/// The held folder's strays: a seal's staging folder a stop left behind is removed once stale, a
/// package folder with no row (a restore from an older backup) is kept and counted, never deleted,
/// and a package with its row is left alone. (The migration-guard read of 2026-11-20 found that
/// nothing ever looked at either.)
///
/// Seen red 2026-11-20 with the staging folder's removal taken out: "an unfinished seal is
/// removed" (left (0, 1), right (1, 1)).
#[test]
fn the_held_folder_keeps_no_unfinished_seal_and_reports_a_package_with_no_row() {
    let d = dirs("strays");
    let (st, _id, _ada, _own, row) = one_held("strays", &d);
    let held = held_dir_beside(d.folders());
    std::fs::create_dir_all(held.join(".staging-0123")).unwrap();
    std::fs::write(held.join(".staging-0123").join("1.sealed"), "half a seal").unwrap();
    std::fs::create_dir_all(held.join("ffffffffffffffffffffffffffffffff")).unwrap();
    // A fresh staging folder may be a seal in progress: left alone.
    let an_hour_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(3_600);
    assert_eq!(st.db.held_sweep_strays_in(d.folders(), an_hour_ago).unwrap(), (0, 1));
    assert!(held.join(".staging-0123").is_dir());
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(7_200);
    assert_eq!(st.db.held_sweep_strays_in(d.folders(), later).unwrap(), (1, 1), "an unfinished seal is removed");
    assert!(!held.join(".staging-0123").exists());
    assert!(held.join("ffffffffffffffffffffffffffffffff").is_dir(), "a package with no row is kept");
    assert!(held.join(&row.package).is_dir(), "a package with its row is left alone");
}
