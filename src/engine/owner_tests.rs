// Child of engine/owner.rs (#[path]): the owner's frames and the re-wrap job, run through the app's
// state against a fake server on 127.0.0.1. Each test was seen red once on purpose, recorded at the
// test.

use super::*;
use crate::net::held::tests::{owner_keys, package, submission_object};
use crate::net::owner::tests::{card_of, key_of, payload_text};
use humanity_protocol::held_seal::{header_len, open_stream, part_context};
use std::sync::{Arc, Mutex};

/// What the fake server holds and was sent.
#[derive(Default)]
pub(crate) struct Fake {
    /// Key cards by key.
    pub cards: HashMap<String, serde_json::Value>,
    /// Sealed parts by (item, part); their headers are served under `grant`.
    pub parts: HashMap<(i64, usize), Vec<u8>>,
    pub grant: String,
    /// The new headers sent back, by (item, part).
    pub rewrapped: HashMap<(i64, usize), Vec<u8>>,
    /// Every request's method and path.
    pub asked: Vec<String>,
}

/// A web server on 127.0.0.1 answering `GET /api/key-card/{key}` and `GET`/`PUT
/// /api/held/{id}/{part}/header?grant=` from `fake` (a part's own route, which would serve a body, is
/// a 404 and recorded); its address.
pub(crate) fn serve(fake: Arc<Mutex<Fake>>) -> String {
    use std::io::{BufRead, Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            let _ = reader.read_line(&mut line);
            let mut length = 0usize;
            loop {
                let mut h = String::new();
                if reader.read_line(&mut h).unwrap_or(0) == 0 || h == "\r\n" {
                    break;
                }
                if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = v.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0u8; length];
            let _ = reader.read_exact(&mut body);
            let mut words = line.split_whitespace();
            let (method, target) = (words.next().unwrap_or("").to_string(), words.next().unwrap_or("").to_string());
            let (path, query) = target.split_once('?').unwrap_or((target.as_str(), ""));
            let q = |k: &str| query.split('&').find_map(|kv| kv.strip_prefix(&format!("{k}="))).unwrap_or("").to_string();
            let mut f = fake.lock().unwrap();
            f.asked.push(format!("{method} {path}"));
            let (status, answer): (&str, Vec<u8>) = if let Some(key) = path.strip_prefix("/api/key-card/") {
                match f.cards.get(key) {
                    Some(card) => ("200 OK", card.to_string().into_bytes()),
                    None => ("404 Not Found", b"{\"error\":\"This server holds no key card for that person.\"}".to_vec()),
                }
            } else if let Some(rest) = path.strip_prefix("/api/held/") {
                let bits: Vec<&str> = rest.split('/').collect();
                let id = bits.first().and_then(|b| b.parse::<i64>().ok()).unwrap_or(-1);
                let part = bits.get(1).and_then(|b| b.parse::<usize>().ok()).unwrap_or(usize::MAX);
                if q("grant") != f.grant || bits.get(2) != Some(&"header") {
                    ("404 Not Found", Vec::new())
                } else if method == "GET" {
                    match f.parts.get(&(id, part)) {
                        Some(p) => ("200 OK", p[..header_len(p[8] as usize)].to_vec()),
                        None => ("404 Not Found", Vec::new()),
                    }
                } else if method == "PUT" {
                    f.rewrapped.insert((id, part), body.clone());
                    ("200 OK", b"{\"done\":true}".to_vec())
                } else {
                    ("404 Not Found", Vec::new())
                }
            } else {
                ("404 Not Found", Vec::new())
            };
            drop(f);
            let mut s = stream;
            let _ = write!(s, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", answer.len());
            let _ = s.write_all(&answer);
        }
    });
    format!("http://{addr}")
}

/// An app signed in as the owner (seed 9), on a recording socket, knowing this server's identity,
/// with Bea (seed 7) and Cy (seed 8) in the member list.
pub(crate) fn owner_app(tag: &str) -> (GuiState, std::sync::mpsc::Receiver<String>) {
    let mut gs = GuiState::default();
    gs.profile_public_key = key_of(9);
    gs.private_key_bytes = Some(vec![9u8; 32]);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let mut store = crate::net::dm_store::DmStore::load(&[9u8; 32], &gs.profile_public_key, &format!("wss://{tag}-{nanos}.example"));
    store.set_pass_server("did:hum:owner-test");
    gs.dm_store = Some(store);
    let (client, sent) = crate::net::ws_client::WsClient::recording();
    gs.ws_client = Some(client);
    gs.ws_identified = true;
    let user = |name: &str, key: String, role: &str| crate::gui::ChatUser { name: name.into(), public_key: key, role: role.into(), status: "online".into() };
    gs.chat_users = vec![user("Ada", key_of(9), "owner"), user("Bea", key_of(7), "member"), user("Cy", key_of(8), "member")];
    (gs, sent)
}

/// The groups at the positions asked, as the person whose key is `key` reads them out from their
/// OWN screen: Settings > Account drawn for them (`own_fingerprint::seen_on_own_screen`), never
/// computed from the key the owner's page lists (the fifth review's H1 (d)).
pub(crate) fn read_out(gs: &GuiState, key: &str) -> String {
    let groups = crate::gui::pages::settings::own_fingerprint::seen_on_own_screen(key);
    gs.reports.owner.asked.expect("four groups asked").iter().map(|p| groups[*p].clone()).collect::<Vec<_>>().join(" ")
}
/// The four groups the person `key_of(seed)` reads out from their own screen.
fn asked_groups(gs: &GuiState, seed: u8) -> String {
    read_out(gs, &key_of(seed))
}
/// The words asking for `name`'s four groups, at the positions asked.
fn fp_needed(gs: &GuiState, name: &str) -> String {
    crate::engine::owner::fp_needed(name, Some(gs.reports.owner.asked.expect("four groups asked")))
}
/// The person whose key is `key` read the four groups asked from their own screen and the owner
/// typed them (positions 1, 2, 4 and 5 when the test set the pick by hand, with nothing drawn).
pub(crate) fn type_groups(gs: &mut GuiState, key: &str) {
    gs.reports.owner.asked.get_or_insert([0, 1, 3, 4]);
    gs.reports.owner.fp_confirm = read_out(gs, key);
}

pub(crate) fn sent_frames(rx: &std::sync::mpsc::Receiver<String>) -> Vec<serde_json::Value> {
    rx.try_iter().filter_map(|s| serde_json::from_str(&s).ok()).collect()
}

/// Run `poll` until `done` or 20 seconds.
pub(crate) fn poll_until(gs: &mut GuiState, done: impl Fn(&GuiState) -> bool) {
    let start = std::time::Instant::now();
    while !done(gs) && start.elapsed() < std::time::Duration::from_secs(20) {
        poll(gs);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// A HANDOVER RUNS END TO END ON THIS DEVICE, HEADERS ALONE: choosing Bea fetches her key card and
/// checks it here; the change is refused until the owner types the four groups asked as Bea reads
/// them from her OWN screen (the second review, W1: a relay can list its own key as "Bea"; the
/// fifth review, H1: never from the owner's screen), one group alone asking for all four rather
/// than counting as a mismatch; the signed `owner_handover` goes out; the relay's ready (checked against what was signed)
/// starts the job, which fetches each part's HEADER under the grant, re-wraps it for Bea's key and
/// sends the new header, the page hearing how far it has come; no body is ever asked for; the finish
/// carries no key; every new header heading its part's old body opens for Bea and not for me. The
/// relay's answer then clears the change.
///
/// Seen red 2026-11-21 (run by mutation, scratchpad ownfix4-combo.js) with `refusal` skipping
/// `pick_allowed`: "a stranger's key is refused until its fingerprint is typed" failed (the change
/// went out signed).
#[test]
fn a_handover_rewraps_every_header_for_the_new_owner_and_fetches_no_body() {
    let (mut gs, sent) = owner_app("owner_flow");
    let (me, bea) = (owner_keys(9), owner_keys(7));
    let fake = Arc::new(Mutex::new(Fake::default()));
    fake.lock().unwrap().cards.insert(key_of(7), card_of(7, 200));
    gs.server_url = serve(fake.clone());

    pick(&mut gs, &key_of(7), "Bea");
    poll_until(&mut gs, |gs| gs.reports.owner.card.is_some());
    assert!(matches!(gs.reports.owner.card, Some(CardCheck::Good { at: 200, .. })), "her card checks out here: {:?}", gs.reports.owner.card);
    gs.reports.owner.confirm = "Bea".into();
    gs.reports.owner.reason = "moving on".into();
    begin(&mut gs, Change::Handover);
    assert!(sent_frames(&sent).iter().all(|f| f["type"] != "owner_change_begin"), "a stranger's key is refused until its fingerprint is typed");
    assert_eq!(gs.reports.owner.status, fp_needed(&gs, "Bea"));
    gs.reports.owner.fp_confirm = asked_groups(&gs, 7).split(' ').next_back().unwrap().to_string();
    begin(&mut gs, Change::Handover);
    assert!(sent_frames(&sent).iter().all(|f| f["type"] != "owner_change_begin"), "one of her groups alone is refused");
    assert_eq!(gs.reports.owner.status, tf("owner.fp_short", &[("name", &"Bea")]), "and asks for all four, not a mismatch");
    assert!(gs.reports.owner.refused.is_empty(), "a short set refuses no key");
    gs.reports.owner.fp_confirm = asked_groups(&gs, 7);
    begin(&mut gs, Change::Handover);
    let frames = sent_frames(&sent);
    let b = frames.iter().find(|f| f["type"] == "owner_change_begin").expect("the signed change is sent");
    let o = submission_object(&b["action"]);
    assert_eq!(payload_text(&o)["action"], "owner_handover");
    let grant = o.object_id().unwrap().to_hex();

    {
        let mut f = fake.lock().unwrap();
        f.grant = grant.clone();
        for (id, pkg) in [(4i64, "p4"), (5, "p5")] {
            for (part, bytes) in package(&me, pkg).into_iter().enumerate() {
                f.parts.insert((id, part), bytes);
            }
        }
    }
    let ready = serde_json::json!({ "type": "owner_change_ready", "grant": grant, "action": "owner_handover", "target": key_of(7),
        "recipients": [{ "key": key_of(7), "card_at": 200 }], "items": [{ "id": 4, "package": "p4", "parts": 3 }, { "id": 5, "package": "p5", "parts": 3 }] });
    assert!(on_frame(&mut gs, &ready));
    poll_until(&mut gs, |gs| gs.reports.owner.job.is_none());
    let frames = sent_frames(&sent);
    let finish = frames.iter().find(|f| f["type"] == "owner_change_finish").unwrap_or_else(|| panic!("the finish is sent: {}", gs.reports.owner.status));
    assert_eq!(finish, &serde_json::json!({ "type": "owner_change_finish", "grant": grant }), "the finish carries no key");
    assert_eq!(gs.reports.owner.progress, Some(crate::net::owner::Progress { items_done: 2, items: 2, parts_done: 6, parts: 6 }), "the page heard every part");
    assert_eq!(gs.reports.owner.status, t("owner.finishing"));
    let f = fake.lock().unwrap();
    assert!(f.asked.iter().filter(|a| a.contains("/api/held/")).all(|a| a.ends_with("/header")), "no body was asked for: {:?}", f.asked);
    assert_eq!(f.rewrapped.len(), 6);
    for ((id, part), header) in &f.rewrapped {
        assert_eq!(header.len(), header_len(1), "a header alone");
        let old = &f.parts[&(*id, *part)];
        let rebuilt = [&header[..], &old[header_len(1)..]].concat();
        let context = part_context(if *id == 4 { "p4" } else { "p5" }, *part);
        assert!(open_stream(&bea, &context, rebuilt.as_slice(), &mut Vec::new()).is_ok(), "Bea opens part {part} of item {id}");
        assert!(open_stream(&me, &context, rebuilt.as_slice(), &mut Vec::new()).is_err(), "and I no longer do");
    }
    drop(f);
    assert!(on_frame(&mut gs, &serde_json::json!({ "type": "owner_reply", "kind": "handover", "ok": true, "message": "Bea is now this server's owner" })));
    assert!(gs.reports.owner.pending.is_none() && gs.reports.owner.picked.is_none() && gs.reports.owner.reason.is_empty(), "the change is over");
    assert_eq!(gs.reports.owner.status, "Bea is now this server's owner");
}

/// THE DESKTOP AND THE RELAY AGREE, END TO END, ON NAMING A SUCCESSOR (format v4): the page's own
/// signed `owner_change_begin` is taken by the relay's real handler; the relay's
/// `owner_change_ready`, serialized exactly as it goes on the wire, is read and checked by the
/// device; the device re-wraps each part's header from where the relay serves it and hands the new
/// header to the relay's handler; the device's finish frame is taken by the relay's finish; each
/// recipient's device reads the relay's check frame (the owner's signed action and the round
/// included), checks its own slot of every new header, and signs. Every held part then opens for the
/// owner's and the successor's test seeds. Seam test: both halves built side by side (memory
/// feedback_review_after_parallel_waves), so a shape one side reads differently fails here.
///
/// Seen red 2026-11-20 with the device expecting the successor's slot before the owner's
/// (`expected_recipients` for a successor returning [target, owner]): "the device takes the
/// relay's ready" failed (the device refused the relay's recipients).
#[test]
fn the_desktop_and_the_relay_agree_on_naming_a_successor_end_to_end() {
    use crate::relay::handlers::held::tests::{dirs, one_held, person, TS};
    use crate::relay::handlers::owner::{begin_in, current_header_in, finish_in, new_header_in};
    let d = dirs("ownernat_seam");
    let (st, _report, _admin, own, _row) = one_held("ownernat_seam", &d);
    assert_eq!(own.key, key_of(9), "the relay's owner is this app's identity");
    let bea = person(7);
    st.db.register_name("Bea", &bea.key).unwrap();
    assert!(st.db.store_key_card(&humanity_protocol::key_card::sign_card(&bea.seed, TS).unwrap()).unwrap().stored());
    let now = || crate::relay::storage::now_millis() as i64;

    // The page signs and sends the change, for this relay's own identity.
    let (mut gs, sent) = owner_app("ownernat_seam");
    gs.dm_store.as_mut().unwrap().set_pass_server(&st.db.server_did().unwrap());
    gs.reports.owner.picked = Some((bea.key.clone(), "Bea".into()));
    gs.reports.owner.card = Some(CardCheck::Good { at: TS, kyber: owner_keys(7).public_key() });
    type_groups(&mut gs, &bea.key);
    gs.reports.owner.confirm = "Bea".into();
    gs.reports.owner.reason = "in case I cannot carry on".into();
    begin(&mut gs, Change::Successor);
    let begin_frame = sent_frames(&sent).into_iter().find(|f| f["type"] == "owner_change_begin").expect("the page sent its change");

    let mut rx = st.broadcast_tx.subscribe();
    begin_in(&st, &own.key, &begin_frame, d.folders(), now());
    let wire: Vec<serde_json::Value> = crate::relay::handlers::held::tests::drain(&mut rx).iter().map(|m| serde_json::to_value(m).unwrap()).collect();
    let ready_frame = wire.iter().find(|f| f["type"] == "owner_change_ready").unwrap_or_else(|| panic!("the relay took the page's change: {wire:?}"));
    let ready = crate::net::owner::parse_ready(ready_frame).expect("the device reads the relay's ready");
    let p = gs.reports.owner.pending.clone().expect("pending");
    crate::net::owner::check_ready(&ready, p.change, &gs.profile_public_key, &p.target, &p.grant).expect("the device takes the relay's ready");

    // The device's work: cards checked here, every header re-wrapped and sent.
    let me = owner_keys(9);
    let card = |k: &str| -> Result<Option<serde_json::Value>, String> { Ok(st.db.get_key_card(k).unwrap().map(|c| humanity_protocol::key_card::card_json(&c))) };
    let short = |k: &str| -> String { k.chars().take(8).collect() };
    let keys = crate::net::owner::recipient_keys(&own.key, &me.public_key(), &ready.recipients, card, |_: &str| None, short).expect("both cards check out here");
    let grant = ready.grant.clone();
    crate::net::owner::rewrap_all(
        &me,
        &keys,
        &ready.items,
        |id, part| current_header_in(&st, id, part as i64, &grant, &own.key, d.folders(), now()).ok_or_else(|| "not served under the grant".to_string()),
        |id, part, header| {
            let r = new_header_in(&st, id, part as i64, &grant, &own.key, header, d.folders(), now());
            if r.status().is_success() {
                Ok(())
            } else {
                Err(format!("the relay refused part {part}'s header: {}", r.status()))
            }
        },
        |_| {},
    )
    .expect("the relay takes every new header");
    let finish: serde_json::Value = serde_json::from_str(&crate::net::owner::finish_frame(&grant)).unwrap();
    let mut rx = st.broadcast_tx.subscribe();
    finish_in(&st, &own.key, &finish, d.folders(), now()).expect("the relay builds every new part");
    assert!(st.db.successor().is_none(), "nothing changes before the recipients accept");
    let wire: Vec<serde_json::Value> = crate::relay::handlers::held::tests::drain(&mut rx).iter().map(|m| serde_json::to_value(m).unwrap()).collect();
    let did = st.db.server_did().unwrap();
    let mut last = None;
    for (who, seed, kp) in [(&own.key, vec![9u8; 32], owner_keys(9)), (&bea.key, bea.seed.clone(), owner_keys(7))] {
        let frame = wire.iter().find(|f| f["type"] == "owner_change_check" && f["items"].as_array().is_some_and(|a| !a.is_empty())).expect("a check frame");
        let check = crate::net::owner::parse_check(frame).expect("the device reads the relay's check");
        assert_eq!(crate::net::owner::is_own_change(&check, who), *who == own.key, "the owner's own change is known from its signature");
        let digest = crate::net::owner::check_all(&kp, who, &check, |id, part| {
            crate::relay::handlers::owner::accept::rewrapped_header_in(&st, id, part as i64, &check.grant, who, d.folders(), now()).ok_or_else(|| "no new header for this recipient".to_string())
        }, |_| {})
        .expect("every new header checks out on the recipient's device");
        let signed = crate::net::owner::sign_accept(&seed, &did, &check.grant, &digest, now() as u64).unwrap();
        let accept: serde_json::Value = serde_json::from_str(&crate::net::owner::accept_frame(&signed)).unwrap();
        assert!(accept.get("keys").is_none(), "no restore key travels");
        last = crate::relay::handlers::owner::accept::accept_in(&st, who, &accept, d.folders(), now()).expect("the relay takes the acceptance");
    }
    let change = last.expect("the last acceptance makes the change");
    assert_eq!(change.target, bea.key);
    assert_eq!(st.db.successor().unwrap(), bea.key, "Bea is the successor");
    let succ = owner_keys(7);
    for row in st.db.held_items().unwrap() {
        assert_eq!(row.also_sealed_to, bea.key, "item {} is sealed to her too", row.id);
        for part in 0..row.parts as usize {
            let file = crate::relay::storage::held::held_dir_beside(d.folders()).join(&row.package).join(crate::relay::storage::held::part_file(part));
            let bytes = std::fs::read(file).unwrap();
            let context = part_context(&row.package, part);
            assert!(open_stream(&me, &context, bytes.as_slice(), &mut Vec::new()).is_ok(), "the owner opens part {part}");
            assert!(open_stream(&succ, &context, bytes.as_slice(), &mut Vec::new()).is_ok(), "the successor opens part {part}");
        }
    }
}

/// THE OWNER'S OWN CHECK SURVIVES A RELOAD AND A DROPPED SOCKET (the second review, W6): a fresh app
/// that remembers nothing knows the check is its own change from the signed action (it checks at
/// once, and asks nothing); when the socket is gone as the check ends, the check is NOT marked
/// answered (it stays, and nothing is lost); the relay's re-ask after reconnecting is answered with
/// the acceptance.
///
/// Seen red 2026-11-20 with `poll_check` dropping the check when not connected (as the branch
/// had it): "the check stays when the socket is gone" failed.
#[test]
fn the_owners_own_check_survives_a_reload_and_a_dropped_socket() {
    let (mut gs, _sent) = owner_app("owner_own_check");
    let s = crate::net::owner::sign(&[9u8; 32], "did:hum:owner-test", Change::Successor, &key_of(7), "a successor", now_ms() as u64).unwrap();
    let frame = serde_json::json!({ "type": "owner_change_check", "grant": s.object_id, "round": "r1", "action": "owner_successor", "object": s.submission,
        "owner": key_of(9), "owner_name": "Ada", "target": key_of(7), "target_name": "Bea", "items": [], "until": now_ms() + 3_600_000 });
    gs.ws_client = None;
    assert!(on_frame(&mut gs, &frame));
    assert_eq!(gs.reports.owner.status, t("owner.check_own"), "checked at once, as the owner's own");
    poll_until(&mut gs, |gs| gs.reports.owner.check_job.is_none());
    assert!(gs.reports.owner.check.is_some(), "the check stays when the socket is gone");
    assert_eq!(gs.reports.owner.check_words, t("held.not_connected"));
    let (client, sent) = crate::net::ws_client::WsClient::recording();
    gs.ws_client = Some(client);
    assert!(on_frame(&mut gs, &frame), "the relay asks again on reconnecting");
    poll_until(&mut gs, |gs| gs.reports.owner.check_job.is_none());
    let accept = sent_frames(&sent).into_iter().find(|f| f["type"] == "owner_change_accept").expect("the re-ask is answered");
    let o = submission_object(&accept["action"]);
    assert_eq!(payload_text(&o)["target"], s.object_id.as_str());
    assert!(gs.reports.owner.check.is_none(), "and only then marked answered");
}

/// A READY THAT DOES NOT MATCH WHAT WAS SIGNED STARTS NOTHING: a relay naming its own key beside
/// Bea's gets a cancel, the page says why in words, and no header is fetched and no card asked for.
///
/// Seen red 2026-11-20 with `on_ready` ignoring `check_ready`'s refusal: the status check failed,
/// the page reading "Bea has no key card on this server yet..." because the job had started and
/// asked the fake server for her card.
#[test]
fn a_ready_naming_other_recipients_is_refused_and_cancelled() {
    let (mut gs, sent) = owner_app("owner_bad_ready");
    let fake = Arc::new(Mutex::new(Fake::default()));
    gs.server_url = serve(fake.clone());
    gs.reports.owner.picked = Some((key_of(7), "Bea".into()));
    gs.reports.owner.card = Some(CardCheck::Good { at: 200, kyber: owner_keys(7).public_key() });
    type_groups(&mut gs, &key_of(7));
    gs.reports.owner.confirm = "Bea".into();
    gs.reports.owner.reason = "moving on".into();
    begin(&mut gs, Change::Handover);
    let grant = gs.reports.owner.pending.as_ref().expect("sent").grant.clone();
    let _ = sent_frames(&sent);
    let ready = serde_json::json!({ "type": "owner_change_ready", "grant": grant, "action": "owner_handover", "target": key_of(7),
        "recipients": [{ "key": key_of(7), "card_at": 200 }, { "key": key_of(8), "card_at": 1 }], "items": [{ "id": 4, "package": "p4", "parts": 3 }] });
    on_frame(&mut gs, &ready);
    poll(&mut gs);
    std::thread::sleep(std::time::Duration::from_millis(200));
    poll(&mut gs);
    let frames = sent_frames(&sent);
    assert!(frames.iter().any(|f| f["type"] == "owner_change_cancel"), "the relay is told to keep everything as it was");
    assert!(!frames.iter().any(|f| f["type"] == "owner_change_finish"));
    assert_eq!(gs.reports.owner.status, tf("owner.failed", &[("why", &t("owner.recipients_mismatch"))]));
    assert!(gs.reports.owner.pending.is_none() && gs.reports.owner.job.is_none());
    assert!(fake.lock().unwrap().asked.is_empty(), "no header is fetched and no card asked for: {:?}", fake.lock().unwrap().asked);
}

/// EVERY HANDOVER AND NAMING ASKS FOUR GROUPS, FOR EVERY KEY, EVERY TIME (the third review's W1; the
/// fourth review's findings 1 and 5, 2026-11-21). Bea's card is pinned here (she wrote once, or a
/// relay handed it over) and she is a friend this device gave a pass (a mutual follow, which the
/// relay can make with its own key under her name: one "Follow back"), yet choosing her asks for
/// four groups, and nothing typed is refused. The four are drawn afresh per pick, from all 15 sets
/// (the fifth review's H1 removed "never the set asked just before"). Once the owner typed what she
/// read from her own screen and the change was signed, the next change asks again, drawn afresh,
/// with the field emptied, even for her: no confirmation is kept.
///
/// Seen red 2026-11-21 (scratchpad ownfix7-batch1.log and ownfix7-batch2.log, mutants of
/// ownfix6-rust-mutants.json) with the round-4 friend exemption put back in `refusal` (R-a, a friend
/// given a pass skips `pick_allowed`): "a friend given a pass is refused with nothing typed" failed;
/// with the fresh draw after `begin` removed (R-b): "the next change asks a new set" failed, the same
/// set asked twice (Some([1, 3, 4, 5]), and Some([1, 2, 3, 4]) on the rerun alone).
#[test]
fn every_change_asks_four_groups_whoever_the_person() {
    let (mut gs, sent) = owner_app("owner_pinned");
    let fake = Arc::new(Mutex::new(Fake::default()));
    gs.server_url = serve(fake);
    crate::engine::key_cards::ensure_pins(&mut gs);
    let card = humanity_protocol::key_card::sign_card(&[7; 32], 200).unwrap();
    gs.key_cards.pins.as_mut().expect("pins").take(&card);
    assert!(gs.key_cards.pins.as_ref().unwrap().pinned(&key_of(7)).is_some(), "Bea is pinned here");
    let store = gs.dm_store.as_mut().expect("store");
    store.set_following(&key_of(7), true);
    store.set_follower(&key_of(7), true);
    store.record_pass_sent(&key_of(7), crate::net::dm_store::SentPass { serial: "ab".repeat(16), may: "dm".into() });
    assert!(store.is_friend(&key_of(7)) && !store.passes_sent_to(&key_of(7)).is_empty(), "Bea is a friend given a pass");
    pick(&mut gs, &key_of(7), "Bea");
    gs.reports.owner.asked.expect("four groups asked of a pinned friend");
    let mut drawn = std::collections::HashSet::new();
    for _ in 0..200 {
        pick(&mut gs, &key_of(7), "Bea");
        let now = gs.reports.owner.asked.unwrap();
        assert!(now.windows(2).all(|w| w[0] < w[1]) && now[3] < 6, "four different groups: {now:?}");
        drawn.insert(now);
    }
    assert!(drawn.len() >= 12, "picks draw afresh from all 15 sets: {} seen in 200", drawn.len());
    let last = gs.reports.owner.asked.unwrap();
    gs.reports.owner.card = Some(CardCheck::Good { at: 200, kyber: owner_keys(7).public_key() });
    gs.reports.owner.confirm = "Bea".into();
    gs.reports.owner.reason = "in case".into();
    begin(&mut gs, Change::Successor);
    assert!(sent_frames(&sent).iter().all(|f| f["type"] != "owner_change_begin"), "a friend given a pass is refused with nothing typed");
    assert_eq!(gs.reports.owner.status, fp_needed(&gs, "Bea"));
    gs.reports.owner.fp_confirm = asked_groups(&gs, 7);
    begin(&mut gs, Change::Successor);
    assert!(sent_frames(&sent).iter().any(|f| f["type"] == "owner_change_begin"), "signed once the groups asked were typed");
    let _ = last;
    assert!(gs.reports.owner.asked.is_some() && gs.reports.owner.fp_confirm.is_empty(), "the next change asks again, drawn afresh, with the field emptied");
    assert!(gs.reports.owner.fp_ok.is_none(), "and no match kept");
    gs.reports.owner.pending = None;
    begin(&mut gs, Change::Handover);
    assert!(sent_frames(&sent).iter().all(|f| f["type"] != "owner_change_begin"), "a second change is refused until the new groups are typed");
    assert_eq!(gs.reports.owner.status, fp_needed(&gs, "Bea"));
}

/// A NAME AN HONEST RELAY COULD NOT LIST IS REFUSED (the third review's M2): shown as a refused name
/// in words, and nothing is signed for a member listed under it (there is no honest name to type).
///
/// Seen red 2026-11-21 (run by mutation, scratchpad ownfix5-rust-mutants2.js) with `refusal` skipping
/// the honest-name check: the status was not the refused-name words (the change was refused only
/// later, for the typed name).
#[test]
fn a_name_an_honest_relay_could_not_list_is_refused() {
    let (mut gs, sent) = owner_app("owner_badname");
    let fake = Arc::new(Mutex::new(Fake::default()));
    gs.server_url = serve(fake);
    let decorated = format!("Bea ({})", humanity_protocol::key_card::fingerprint(&key_of(7)));
    assert_eq!(crate::net::owner::shown_name(&decorated), t("owner.name_refused"));
    assert_eq!(crate::net::owner::shown_name("Bea"), "Bea");
    pick(&mut gs, &key_of(8), &decorated);
    gs.reports.owner.card = Some(CardCheck::Good { at: 200, kyber: owner_keys(8).public_key() });
    gs.reports.owner.confirm = decorated.clone();
    gs.reports.owner.reason = "r".into();
    gs.reports.owner.fp_confirm = asked_groups(&gs, 8);
    for change in [Change::Handover, Change::Successor] {
        begin(&mut gs, change);
        assert_eq!(gs.reports.owner.status, t("owner.name_refused_handover"));
    }
    assert!(sent_frames(&sent).iter().all(|f| f["type"] != "owner_change_begin"), "nothing signed");
}

/// THE FIFTH REVIEW'S DEMO, AS A TEST (H1 (c), (d)): the relay lists its own key R as "Bea". The
/// owner asks Bea for the groups at the positions drawn and types what she reads from HER OWN screen
/// (Settings > Account drawn for her key; the owner's page never shows R's groups:
/// server_settings/owner_tests.rs). They are not R's, so nothing is signed, the words are the
/// warning (not the words asking for the groups, as an empty field gets), and R is refused for the
/// session: picking it again draws no positions, Check refuses it, and no change to it is signed
/// whatever is typed. The real Bea, picked then, is asked as usual.
///
/// Seen red 2026-11-21 (scratchpad ownfix8-red-m2.log) with `check_groups` answering a mismatch the
/// way round 5 answered any wrong set (the words asking for the groups, nothing refused): "a
/// mismatch is a warning, not the words for an empty field" failed.
#[test]
fn groups_read_from_beas_own_screen_refuse_the_relays_key() {
    let (mut gs, sent) = owner_app("owner_demo");
    let r = key_of(3);
    gs.chat_users[1].public_key = r.clone(); // "Bea" is the relay's own key
    let fake = Arc::new(Mutex::new(Fake::default()));
    fake.lock().unwrap().cards.insert(r.clone(), card_of(3, 200));
    fake.lock().unwrap().cards.insert(key_of(7), card_of(7, 200));
    gs.server_url = serve(fake);
    pick(&mut gs, &r, "Bea");
    poll_until(&mut gs, |gs| gs.reports.owner.card.is_some());
    assert!(matches!(gs.reports.owner.card, Some(CardCheck::Good { .. })), "R's own card checks out for R: {:?}", gs.reports.owner.card);
    gs.reports.owner.confirm = "Bea".into();
    gs.reports.owner.reason = "moving on".into();
    gs.reports.owner.fp_confirm = asked_groups(&gs, 7); // what Bea reads from her own screen
    begin(&mut gs, Change::Handover);
    assert!(sent_frames(&sent).iter().all(|f| f["type"] != "owner_change_begin"), "nothing is signed for R");
    let warning = crate::engine::owner::mismatch_words("Bea");
    assert_eq!(gs.reports.owner.status, warning, "a mismatch is a warning, not the words for an empty field");
    assert!(crate::engine::owner::is_refused(&gs, &r) && gs.reports.owner.asked.is_none(), "R is refused on this server");
    pick(&mut gs, &r, "Bea");
    assert!(gs.reports.owner.asked.is_none() && gs.reports.owner.status == warning, "picking R again draws no positions");
    gs.reports.owner.fp_confirm = "0000 0000 0000 0000".into();
    check_groups_now(&mut gs);
    assert_eq!(gs.reports.owner.status, warning, "Check refuses it");
    for change in [Change::Handover, Change::Successor] {
        begin(&mut gs, change);
        assert_eq!(gs.reports.owner.status, warning);
    }
    assert!(sent_frames(&sent).iter().all(|f| f["type"] != "owner_change_begin"), "nothing signed for R, whatever is typed");
    // The real Bea is asked as usual.
    pick(&mut gs, &key_of(7), "Bea");
    assert!(gs.reports.owner.asked.is_some(), "another key is asked");
}

/// AN ANSWER THE RELAY NEVER ACKNOWLEDGED IS GIVEN AGAIN ON ITS RE-ASK (the third review's W5): Bea
/// accepts; no acknowledgement comes (the socket died just after sending); the relay's re-ask of the
/// same change and round is answered again with an acceptance signed afresh, not the same one. Once
/// the relay acknowledges, a re-ask is not answered. A decline is given again the same way.
///
/// Seen red 2026-11-21 with `on_check` ignoring the answer it sent (by mutation, the re-ask treated
/// as a new question): "the re-ask is answered again" failed (Bea was asked again instead).
#[test]
fn an_answer_never_acknowledged_is_given_again() {
    let (mut gs, sent) = owner_app("owner_unacked");
    gs.profile_public_key = key_of(7);
    gs.private_key_bytes = Some(vec![7; 32]);
    let s = crate::net::owner::sign(&[9u8; 32], "did:hum:owner-test", Change::Successor, &key_of(7), "a successor", now_ms() as u64).unwrap();
    let frame = serde_json::json!({ "type": "owner_change_check", "grant": s.object_id, "round": "r1", "action": "owner_successor", "object": s.submission,
        "owner": key_of(9), "owner_name": "Ada", "target": key_of(7), "target_name": "Bea", "items": [], "until": now_ms() + 3_600_000 });
    let accepts = |rx: &std::sync::mpsc::Receiver<String>| sent_frames(rx).into_iter().filter(|f| f["type"] == "owner_change_accept").collect::<Vec<_>>();
    assert!(on_frame(&mut gs, &frame));
    let check = gs.reports.owner.check.clone().expect("Bea is asked");
    start_check(&mut gs, check);
    poll_until(&mut gs, |gs| gs.reports.owner.check_job.is_none());
    let first = accepts(&sent);
    assert_eq!(first.len(), 1, "the first answer");
    assert!(on_frame(&mut gs, &frame), "no acknowledgement came: the relay asks again");
    poll_until(&mut gs, |gs| gs.reports.owner.check_job.is_none());
    let again = accepts(&sent);
    assert_eq!(again.len(), 1, "the re-ask is answered again");
    assert_ne!(again[0], first[0], "signed afresh, not the same acceptance");
    assert!(on_frame(&mut gs, &serde_json::json!({ "type": "owner_reply", "kind": "accept", "ok": true, "message": "Your device checked every copy and accepted." })));
    assert!(on_frame(&mut gs, &frame));
    poll_until(&mut gs, |gs| gs.reports.owner.check_job.is_none());
    assert!(accepts(&sent).is_empty() && gs.reports.owner.check.is_none(), "acknowledged: not answered, not asked");

    let (mut gs, sent) = owner_app("owner_unacked_decline");
    gs.profile_public_key = key_of(7);
    let round2 = serde_json::json!({ "type": "owner_change_check", "grant": s.object_id, "round": "r2", "action": "owner_successor", "object": s.submission,
        "owner": key_of(9), "owner_name": "Ada", "target": key_of(7), "target_name": "Bea", "items": [], "until": now_ms() + 3_600_000 });
    assert!(on_frame(&mut gs, &round2));
    decline_check(&mut gs);
    assert!(on_frame(&mut gs, &round2));
    let declines = sent_frames(&sent).into_iter().filter(|f| f["type"] == "owner_change_decline").count();
    assert_eq!(declines, 2, "a decline never acknowledged is given again");
}

/// A REFUSED KEY STAYS REFUSED ON ITS SERVER ACROSS A SWITCH (the sixth review's L1): R's groups did
/// not match on server A, so R is refused there; a switch to server B (the per-server state is
/// forgotten, as connections.rs does on every switch) asks for R as usual on B; and back on A, R is
/// still refused, with the warning and no fresh draw, until the app restarts.
///
/// Seen red 2026-11-21 (scratchpad ownfix9-red-native.log) against `ReportUi::forget_server`
/// resetting the whole owner state: "back on its server, the key is still refused" failed (a fresh
/// draw was made).
#[test]
fn a_refused_key_stays_refused_on_its_server_across_a_switch() {
    let (mut gs, _sent) = owner_app("refused_switch");
    let r = key_of(3);
    gs.chat_users[1].public_key = r.clone(); // "Bea" is the relay's own key
    let fake = Arc::new(Mutex::new(Fake::default()));
    fake.lock().unwrap().cards.insert(r.clone(), card_of(3, 200));
    let (a, b) = (serve(fake.clone()), serve(fake));
    gs.server_url = a.clone();
    pick(&mut gs, &r, "Bea");
    gs.reports.owner.fp_confirm = asked_groups(&gs, 7); // what the real Bea reads from her own screen
    check_groups_now(&mut gs);
    let warning = crate::engine::owner::mismatch_words("Bea");
    assert_eq!(gs.reports.owner.status, warning, "R is refused on server A");
    gs.reports.forget_server();
    gs.server_url = b;
    pick(&mut gs, &r, "Bea");
    assert!(gs.reports.owner.asked.is_some(), "on another server the key is asked as usual");
    gs.reports.forget_server();
    gs.server_url = a;
    pick(&mut gs, &r, "Bea");
    assert!(gs.reports.owner.asked.is_none() && gs.reports.owner.status == warning, "back on its server, the key is still refused");
}
