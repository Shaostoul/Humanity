// Child of engine/household.rs (#[path]): ship homes increment 5b, lane 1A (permits travel, desktop
// DMs), with the app state and a recording socket, no screen. Each test was seen failing once on
// purpose, recorded at the test.

use super::*;
use crate::net::dm_pq::DmInner;
use crate::net::dm_store::DmStore;
use crate::net::household::{Judged, PermitKey};
use crate::relay::core::pq_crypto::{build_friend_cert, build_plot_permit, derive_dilithium_seed, DilithiumKeypair, FRIEND_PASS_DEFAULT_MAY};
use serde_json::{json, Value};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

/// The server every app here is on.
const SERVER: &str = "did:hum:4dQe1bVHyiHm1Vh8rWbx2F";
const SERIAL: &str = "00112233445566778899aabbccddeeff";

/// The seed and key (Dilithium3, hex) of the person whose seed is `[n; 32]`.
fn identity(n: u8) -> (Vec<u8>, String) {
    let seed = vec![n; 32];
    (seed.clone(), hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed)).public_key()))
}

fn kyber(seed: &[u8]) -> String {
    crate::net::dm_pq::DmPqKeypair::from_bip39_seed(seed).unwrap().public_base64()
}

/// A signed-in app on its own store of [`SERVER`] (`tag` names it), with a socket that records what
/// is sent. Each of `friends` (seed, key, name) is a mutual follow whose DM key is known, whose
/// name is on the member list, and whose friendship pass to me is held.
fn app(seed: &[u8], me: &str, tag: &str, friends: &[(&[u8], &str, &str)]) -> (GuiState, Receiver<String>) {
    let mut gs = GuiState::default();
    gs.profile_public_key = me.to_string();
    gs.private_key_bytes = Some(seed.to_vec());
    gs.user_name = "Me".into();
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    gs.server_url = format!("https://household-{tag}-{nanos}.example");
    let mut store = DmStore::load(seed, me, &crate::gui::pages::chat::norm_server_url(&gs.server_url));
    store.set_pass_server(SERVER);
    for (s, k, name) in friends {
        befriend(&mut gs, &mut store, s, k, name, me);
    }
    gs.dm_store = Some(store);
    gs.block_list = Some(crate::net::block_list::BlockList::in_temp(seed, me, tag));
    let (client, sent) = crate::net::ws_client::WsClient::recording();
    gs.ws_client = Some(client);
    gs.dm_fetch = crate::net::mailbox_fetch::MailboxFetch::already_read();
    (gs, sent)
}

/// `key` (seed `seed`, called `name`) becomes a friend of `me` in `store`: a mutual follow, their DM
/// key known, their pass to me held, their name on the member list.
fn befriend(gs: &mut GuiState, store: &mut DmStore, seed: &[u8], key: &str, name: &str, me: &str) {
    gs.peer_kyber_keys.insert(key.to_string(), kyber(seed));
    store.set_following(key, true);
    store.set_follower(key, true);
    store.store_cert_from(key, &their_pass(seed, key, me));
    gs.chat_users.push(crate::gui::ChatUser { name: name.into(), public_key: key.into(), role: String::new(), status: String::new() });
}

/// The friendship pass `key` (seed `seed`) gives `me` on [`SERVER`].
fn their_pass(seed: &[u8], key: &str, me: &str) -> String {
    build_friend_cert(seed, SERVER, key, me, SERIAL, &FRIEND_PASS_DEFAULT_MAY).unwrap()
}

fn tidy(gs: &GuiState) {
    gs.dm_store.as_ref().unwrap().remove_file_for_test();
    gs.block_list.as_ref().unwrap().remove_file_for_test();
}

/// Everything sent since the last look.
fn frames(sent: &Receiver<String>) -> Vec<Value> {
    sent.try_iter().map(|f| serde_json::from_str(&f).expect("a JSON frame")).collect()
}

/// The `dm_put`s among `frames` addressed to `to`.
fn puts_to(frames: &[Value], to: &str) -> Vec<Value> {
    frames.iter().filter(|v| v["type"] == "dm_put" && v["to"] == to).cloned().collect()
}

/// The serials `frames` asked the relay to withdraw.
fn revoked(frames: &[Value]) -> Vec<String> {
    frames.iter().filter(|v| v["type"] == "cert_revoke").map(|v| v["serial"].as_str().unwrap().to_string()).collect()
}

/// Open a `dm_put` with its recipient's seed: the verified inner payload.
fn open(put: &Value, seed: &[u8]) -> DmInner {
    let kp = crate::net::dm_pq::DmPqKeypair::from_bip39_seed(seed).unwrap();
    crate::net::dm_pq::parse_verify_inner(&crate::net::dm_pq::open_v2(&kp, put["content"].as_str().unwrap()).unwrap()).unwrap()
}

/// The server's answer to `put` (engine/put_answer.rs `on_frame`, as the message pump calls it).
fn answer(gs: &mut GuiState, put: &Value, ok: bool, reason: &str) {
    let r = put["ref"].as_str().expect("the put carries a ref");
    let frame = if ok { json!({ "type": "dm_put_ok", "ref": r }) } else { json!({ "type": "dm_put_refused", "ref": r, "reason": reason }) };
    assert!(crate::engine::put_answer::on_frame(gs, &frame), "the answer is handled");
}

/// The relay's `cert_revoked {serial}`.
fn confirmed(gs: &mut GuiState, serial: &str) {
    crate::engine::dm::withdrawal_confirmed(gs, &json!({ "type": "cert_revoked", "serial": serial }));
}

fn now() -> u64 {
    now_s()
}

/// The permit `giver` (seed) mints on `server` for `grantee`'s key, for `plot`, named `serial`, running
/// out at `expiry`, minted at `minted`.
fn permit_by(seed: &[u8], issuer: &str, server: &str, plot: &str, grantee: &str, serial: &str, expiry: u64, minted: u64) -> Permit {
    let grantee_did = plot_owner_id(grantee);
    let sig = build_plot_permit(seed, server, plot, &grantee_did, serial, expiry, minted).expect("minted");
    Permit { issuer: issuer.into(), server: server.into(), plot: plot.into(), grantee: grantee_did, serial: serial.into(), expiry, sig }
}

/// A DM `from` to `to` saying `text`, as `ingest_dm` gets it once opened and checked.
fn dm(from: &str, to: &str, text: &str) -> DmInner {
    DmInner { from: from.into(), to: to.into(), ts: now_ms(), text: text.into(), sig_b64: format!("sig-{}", text.len()), cert: None }
}

/// A row I gave `to`, for `plot`, named `serial`, given `ago` seconds ago for `days`, standing as `state`.
fn given_row(to: &str, plot: &str, serial: &str, ago: u64, days: u64, state: GivenState) -> GivenPermit {
    given_at(now(), to, plot, serial, ago, days, state)
}

/// [`given_row`] counted back from `t` (Unix seconds), for a test that compares end dates.
fn given_at(t: u64, to: &str, plot: &str, serial: &str, ago: u64, days: u64, state: GivenState) -> GivenPermit {
    let at_s = t - ago;
    GivenPermit { to: to.into(), plot: plot.into(), serial: serial.into(), expiry: at_s + days * DAY, at: at_s * 1000, state }
}

fn given_to(gs: &GuiState, to: &str) -> Vec<GivenPermit> {
    gs.dm_store.as_ref().unwrap().permits_given().iter().filter(|g| g.to == to).cloned().collect()
}

fn state_of(gs: &GuiState, serial: &str) -> Option<GivenState> {
    gs.dm_store.as_ref().unwrap().permits_given().iter().find(|g| g.serial == serial).map(|g| g.state.clone())
}

fn row_for(gs: &GuiState, key: &str) -> FriendRow {
    friends(gs).into_iter().find(|r| r.key == key).unwrap_or_else(|| panic!("a row for {}", &key[..8]))
}

/// GIVING SEALS ONE NOTE TO THE FRIEND, AND IT COUNTS ONLY ONCE THE SERVER TOOK IT (plan 3.3, the
/// 10l rule). Ada gives Bo 30 days on her plot p2: exactly one `dm_put` goes, to Bo, carrying the
/// pass Bo gave her (so it rides past his gate) and a `ref`, and holding the permit note for p2,
/// made out to Bo's did:hum, running 30 days. No self-copy goes and the row is on its way, not
/// taken, and nothing waits among the passes' sends. On `dm_put_ok` the row is taken and exactly
/// one self-copy goes to Ada's own mailbox, the same note to Bo. Bo's app keeps the permit and
/// says so in a notice.
/// Seen red 2026-10-12 with `Taken` recorded at send (`state: GivenState::Taken` in `give_until`):
/// "nothing counts as given until the server took it" failed, left `Taken`, right `OnItsWay {
/// reference: "..." }`.
#[test]
fn giving_seals_one_note_to_the_friend_and_counts_only_once_the_server_took_it() {
    crate::config::keep_saves_off_disk();
    let (ada_seed, ada) = identity(240);
    let (bo_seed, bo) = identity(241);
    let (mut gs, sent) = app(&ada_seed, &ada, "give", &[(&bo_seed, &bo, "Bo")]);
    gs.household.my_plot = Some("p2".into());

    give(&mut gs, &bo, 30).expect("given");
    let out = frames(&sent);
    let to_bo = puts_to(&out, &bo);
    assert_eq!((out.len(), to_bo.len()), (1, 1), "one note, to Bo, and nothing else: {out:?}");
    assert_eq!(to_bo[0]["friend_cert"], their_pass(&bo_seed, &bo, &ada), "it carries the pass Bo gave Ada");
    let reference = to_bo[0]["ref"].as_str().expect("and a ref").to_string();
    let inner = open(&to_bo[0], &bo_seed);
    assert_eq!((inner.from.as_str(), inner.to.as_str()), (ada.as_str(), bo.as_str()));
    let p = crate::net::household::parse_note(&inner.text).expect("a permit note");
    assert_eq!((p.issuer.as_str(), p.server.as_str(), p.plot.as_str(), p.grantee.clone()), (ada.as_str(), SERVER, "p2", plot_owner_id(&bo)));
    assert!((now() + 30 * DAY).abs_diff(p.expiry) <= 5, "30 days from now: {}", p.expiry);
    let rows = given_to(&gs, &bo);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].state, GivenState::OnItsWay { reference }, "nothing counts as given until the server took it");
    assert!(!gs.pending_puts.has_peer(&bo), "held apart from the passes' sends");
    assert_eq!(row_for(&gs, &bo).permit, Standing::Sending { expiry: p.expiry });

    answer(&mut gs, &to_bo[0], true, "");
    let out = frames(&sent);
    let ours = puts_to(&out, &ada);
    assert_eq!((out.len(), ours.len()), (1, 1), "taken: exactly one self-copy: {out:?}");
    let echo = open(&ours[0], &ada_seed);
    assert_eq!((echo.to.as_str(), echo.text.as_str()), (bo.as_str(), inner.text.as_str()), "the same note to Bo");
    assert_eq!(given_to(&gs, &bo)[0].state, GivenState::Taken);
    assert_eq!(row_for(&gs, &bo).permit, Standing::Stands { expiry: p.expiry, days: 30 });
    assert!(gs.household.line.starts_with("Bo has a permit for your plot p2 until "), "{}", gs.household.line);

    let (mut bo_gs, _bo_sent) = app(&bo_seed, &bo, "give-bo", &[(&ada_seed, &ada, "Ada")]);
    assert!(ingest(&mut bo_gs, &inner), "the note is the household's");
    assert_eq!(bo_gs.dm_store.as_ref().unwrap().permits_held().iter().map(|h| h.permit.clone()).collect::<Vec<_>>(), [p.clone()], "Bo keeps it");
    let notice = bo_gs.pending_notices.last().cloned().unwrap_or_default();
    assert!(notice.starts_with("Ada gave you a household permit for plot p2 until ") && notice.ends_with("Home > Household lists it."), "{notice}");
    tidy(&gs);
    tidy(&bo_gs);
}

/// A RECEIVED PERMIT IS CHECKED, AND KEPT PER SERVER (plan 3.1 and 3.2). I am Bo. Ada's good permit
/// for her p1 is kept. Not kept: one signed by Cy in Ada's name (a forgery), one Ada gave on
/// another server, one she gave Cy, and one that ran out. My store for another server holds none
/// of it.
/// Seen red 2026-10-12 with `judge_arrival` bypassed in `take_note` (the check replaced by `if
/// false`): "a forged permit (signed by Cy in Ada's name) is not kept" failed, left 2, right 1.
#[test]
fn a_received_permit_is_verified_and_kept_per_server() {
    crate::config::keep_saves_off_disk();
    let (bo_seed, bo) = identity(242);
    let (ada_seed, ada) = identity(243);
    let (cy_seed, cy) = identity(244);
    let (mut gs, _sent) = app(&bo_seed, &bo, "kept", &[(&ada_seed, &ada, "Ada"), (&cy_seed, &cy, "Cy")]);
    let t = now();
    let good = permit_by(&ada_seed, &ada, SERVER, "p1", &bo, SERIAL, t + 30 * DAY, t);
    assert!(ingest(&mut gs, &dm(&ada, &bo, &crate::net::household::note_text(&good))));
    let held = |gs: &GuiState| gs.dm_store.as_ref().unwrap().permits_held().len();
    assert_eq!(held(&gs), 1, "Ada's permit is kept");

    let forged = Permit { issuer: ada.clone(), ..permit_by(&cy_seed, &cy, SERVER, "p3", &bo, SERIAL, t + DAY, t) };
    let rows: Vec<(&str, Permit)> = vec![
        ("a forged permit (signed by Cy in Ada's name) is not kept", forged),
        ("a permit Ada gave on another server is not kept", permit_by(&ada_seed, &ada, "did:hum:AnotherServer1111111", "p4", &bo, SERIAL, t + DAY, t)),
        ("a permit Ada gave Cy is not kept", permit_by(&ada_seed, &ada, SERVER, "p5", &cy, SERIAL, t + DAY, t)),
        ("a permit that ran out is not kept", permit_by(&ada_seed, &ada, SERVER, "p6", &bo, SERIAL, t - DAY, t - 10 * DAY)),
    ];
    for (what, p) in rows {
        assert!(ingest(&mut gs, &dm(&ada, &bo, &crate::net::household::note_text(&p))), "{what}: still consumed");
        assert_eq!(held(&gs), 1, "{what}");
    }
    assert_eq!(gs.dm_store.as_ref().unwrap().permits_held()[0].permit, good);

    let (elsewhere, _) = app(&bo_seed, &bo, "kept-elsewhere", &[(&ada_seed, &ada, "Ada")]);
    assert!(elsewhere.dm_store.as_ref().unwrap().permits_held().is_empty(), "my store for another server holds none");
    tidy(&gs);
    tidy(&elsewhere);
}

/// A PERMIT FROM SOMEONE NOT A FRIEND IS DROPPED AND NEVER SHOWN (plan 3.1). Cy follows me but I do
/// not follow her back, so she is not a friend. Her permit, though genuinely hers and made out to
/// me on this server, is consumed by `ingest_dm`: not kept, not stored as a message, no
/// conversation listed, no notice, and nothing put under Requests.
/// Seen red 2026-10-12 with the friend check removed from `take_note`: "Cy's permit is not kept"
/// failed, left 1, right 0.
#[test]
fn a_permit_from_someone_not_a_friend_is_dropped_and_never_shown() {
    crate::config::keep_saves_off_disk();
    let (bo_seed, bo) = identity(245);
    let (cy_seed, cy) = identity(246);
    let (mut gs, _sent) = app(&bo_seed, &bo, "stranger", &[]);
    gs.dm_store.as_mut().unwrap().set_follower(&cy, true);
    let t = now();
    let p = permit_by(&cy_seed, &cy, SERVER, "p1", &bo, SERIAL, t + 30 * DAY, t);
    assert!(!crate::engine::dm::ingest_dm(&mut gs, &dm(&cy, &bo, &crate::net::household::note_text(&p))), "nothing to show");
    let store = gs.dm_store.as_ref().unwrap();
    assert_eq!(store.permits_held().len(), 0, "Cy's permit is not kept");
    assert!(store.conversation(&cy).is_empty() && gs.chat_dms.is_empty(), "never stored or listed as a message");
    assert!(gs.pending_notices.is_empty(), "no notice");
    assert!(store.requests().is_empty(), "and not put under Requests");
    tidy(&gs);
}

/// MY PERMIT, ECHOED FROM MY OTHER DEVICE, IS LISTED AS GIVEN (plan 3.1, "From me"). Ada's first
/// device gives Bo a permit and the server takes it; its self-copy reaches her second device,
/// which lists it as given to Bo, taken, with the same serial, plot and end date, and shows it on
/// Bo's row. A permit "from me" that does not check out as mine (signed by Cy) is not listed.
/// Seen red 2026-10-12 with the from-me arm missing from `take_note` (an echo fell through to the
/// "not to me" drop): "the echo is listed as given on my other device" failed, left 0, right 1.
#[test]
fn my_permit_echoed_from_my_other_device_is_listed_as_given() {
    crate::config::keep_saves_off_disk();
    let (ada_seed, ada) = identity(247);
    let (bo_seed, bo) = identity(248);
    let (cy_seed, cy) = identity(249);
    let (mut one, sent) = app(&ada_seed, &ada, "echo-one", &[(&bo_seed, &bo, "Bo")]);
    one.household.my_plot = Some("p2".into());
    give(&mut one, &bo, 7).unwrap();
    let put = puts_to(&frames(&sent), &bo).remove(0);
    answer(&mut one, &put, true, "");
    let self_copy = puts_to(&frames(&sent), &ada).remove(0);
    let first = given_to(&one, &bo).remove(0);

    let (mut two, _two_sent) = app(&ada_seed, &ada, "echo-two", &[(&bo_seed, &bo, "Bo")]);
    two.household.my_plot = Some("p2".into());
    let echo = open(&self_copy, &ada_seed);
    assert!(ingest(&mut two, &echo), "the echo is the household's");
    let listed = given_to(&two, &bo);
    assert_eq!(listed.len(), 1, "the echo is listed as given on my other device");
    assert_eq!(
        (listed[0].plot.as_str(), listed[0].serial.as_str(), listed[0].expiry, &listed[0].state),
        (first.plot.as_str(), first.serial.as_str(), first.expiry, &GivenState::Taken)
    );
    assert_eq!(row_for(&two, &bo).permit, Standing::Stands { expiry: first.expiry, days: 7 });

    let t = now();
    let not_mine = Permit { issuer: ada.clone(), ..permit_by(&cy_seed, &cy, SERVER, "p2", &bo, "ffeeddccbbaa99887766554433221100", t + DAY, t) };
    assert!(ingest(&mut two, &dm(&ada, &bo, &crate::net::household::note_text(&not_mine))), "consumed");
    assert_eq!(given_to(&two, &bo).len(), 1, "a permit that is not mine is not listed");
    tidy(&one);
    tidy(&two);
}

/// GIVING NEEDS THE PLOT THIS SESSION AND A FRIEND IT CAN BE SEALED FOR (plan 3.3). Each refusal says
/// its own sentence, sends nothing and lists nothing: locked, not connected, no plot from this
/// session's welcome, someone blocked, someone not a friend, a friend whose DM key is not known
/// this run, a friend with a permit still on its way, and 0 or 91 days.
/// Seen red 2026-10-12 with the plot check replaced by `.unwrap_or_default()` in `give_until`: "no
/// plot this session: refused before anything is minted" failed, left `Err("The permit could not
/// be made; try again.")`, right `Err("Step into this server's shared world first: ...")`.
#[test]
fn giving_needs_the_plot_this_session_and_a_sealable_friend() {
    crate::config::keep_saves_off_disk();
    let (ada_seed, ada) = identity(250);
    let (bo_seed, bo) = identity(251);
    let (dee_seed, dee) = identity(252);
    let (_, eve) = identity(253);
    let (mut gs, sent) = app(&ada_seed, &ada, "refusals", &[(&bo_seed, &bo, "Bo"), (&dee_seed, &dee, "Dee")]);
    gs.household.my_plot = Some("p2".into());
    let check = |gs: &mut GuiState, what: &str, peer: &str, days: u64, want: &str| {
        let before = gs.dm_store.as_ref().unwrap().permits_given().len();
        match give(gs, peer, days) {
            Err(e) => assert!(e.contains(want), "{what}: {e:?} should say {want:?}"),
            Ok(()) => panic!("{what}: given"),
        }
        assert!(frames(&sent).is_empty(), "{what}: nothing sent");
        assert_eq!(gs.dm_store.as_ref().unwrap().permits_given().len(), before, "{what}: nothing listed");
    };

    let seed = gs.private_key_bytes.take();
    check(&mut gs, "locked", &bo, 30, LOCKED);
    gs.private_key_bytes = seed;
    let client = gs.ws_client.take();
    check(&mut gs, "not connected", &bo, 30, NOT_CONNECTED);
    gs.ws_client = client;
    gs.household.my_plot = None;
    assert_eq!(give(&mut gs, &bo, 30), Err(NO_PLOT.to_string()), "no plot this session: refused before anything is minted");
    check(&mut gs, "no plot", &bo, 30, NO_PLOT);
    gs.household.my_plot = Some("p2".into());
    gs.peer_kyber_keys.remove(&dee);
    check(&mut gs, "no DM key", &dee, 30, "has not been online on this server since you started the app");
    check(&mut gs, "not a friend", &eve, 30, "is not a friend here");
    for days in [0, 91] {
        check(&mut gs, &format!("{days} days"), &bo, days, BAD_LENGTH);
    }
    gs.block_list.as_mut().unwrap().block(&bo, now_ms());
    check(&mut gs, "blocked", &bo, 30, "You blocked Bo.");
    gs.block_list.as_mut().unwrap().unblock(&bo);

    give(&mut gs, &bo, 30).expect("Bo, 30 days");
    assert_eq!(puts_to(&frames(&sent), &bo).len(), 1);
    check(&mut gs, "one on its way", &bo, 30, "A permit for Bo is still on its way");
    tidy(&gs);
}

/// RENEW GIVES THE SAME LENGTH, COUNTED FROM TODAY (plan 3.3). Bo's permit was given 10 days ago for
/// 30 days (20 left): Renew sends a new one, under a new serial, running 30 days from now. One
/// given for 7 days renews for 7. A permit for a plot that is no longer mine is not renewed, and
/// someone given nothing cannot be renewed.
/// Seen red 2026-10-12 with the old end date reused in `renew` (`let expiry = row.expiry;`): "30 days
/// counted from today: 20 days from now" failed.
#[test]
fn renew_gives_the_same_length_counted_from_today() {
    crate::config::keep_saves_off_disk();
    let (ada_seed, ada) = identity(254);
    let (bo_seed, bo) = identity(255);
    let (cy_seed, cy) = identity(200);
    let (dee_seed, dee) = identity(201);
    let (mut gs, sent) = app(&ada_seed, &ada, "renew", &[(&bo_seed, &bo, "Bo"), (&cy_seed, &cy, "Cy"), (&dee_seed, &dee, "Dee")]);
    gs.household.my_plot = Some("p2".into());
    let store = gs.dm_store.as_mut().unwrap();
    store.record_given(given_row(&bo, "p2", "a1".repeat(16).as_str(), 10 * DAY, 30, GivenState::Taken), now());
    store.record_given(given_row(&cy, "p2", "a2".repeat(16).as_str(), DAY, 7, GivenState::Taken), now());
    store.record_given(given_row(&dee, "p3", "a3".repeat(16).as_str(), DAY, 30, GivenState::Taken), now());

    renew(&mut gs, &bo).expect("renewed");
    let p = crate::net::household::parse_note(&open(&puts_to(&frames(&sent), &bo)[0], &bo_seed).text).unwrap();
    assert!((now() + 30 * DAY).abs_diff(p.expiry) <= 5, "30 days counted from today: {} days from now", (p.expiry as i64 - now() as i64) / DAY as i64);
    assert_ne!(p.serial, "a1".repeat(16), "under a new serial");
    renew(&mut gs, &cy).expect("renewed");
    let p = crate::net::household::parse_note(&open(&puts_to(&frames(&sent), &cy)[0], &cy_seed).text).unwrap();
    assert!((now() + 7 * DAY).abs_diff(p.expiry) <= 5, "a 7-day permit renews for 7");
    let e = renew(&mut gs, &dee).expect_err("another plot");
    assert!(e.contains("plot p3, which is no longer yours here"), "{e}");
    let (_, eve) = identity(202);
    assert!(renew(&mut gs, &eve).is_err(), "nothing given, nothing to renew");
    assert!(frames(&sent).is_empty());
    tidy(&gs);
}

/// A REFUSED OR UNANSWERED PERMIT SAYS SO AND SENDS NO SELF-COPY (plan 3.3). Bo's settings refuse
/// Ada's permit (`reach`): no self-copy goes, the row says refused, and the card's line says why.
/// Cy's gets no answer in 30 seconds: no self-copy, the row and line say unconfirmed. A late
/// `dm_put_ok` for Cy's changes nothing and sends nothing.
/// Seen red 2026-10-12 with the self-copy sent on refusal (`client.send(&put.self_copy...)` in the
/// Refused arm of `on_answer`): "refused: no self-copy" failed, left 1, right 0.
#[test]
fn a_refused_or_unanswered_permit_says_so_and_sends_no_self_copy() {
    crate::config::keep_saves_off_disk();
    let (ada_seed, ada) = identity(203);
    let (bo_seed, bo) = identity(204);
    let (cy_seed, cy) = identity(205);
    let (mut gs, sent) = app(&ada_seed, &ada, "refused", &[(&bo_seed, &bo, "Bo"), (&cy_seed, &cy, "Cy")]);
    gs.household.my_plot = Some("p2".into());

    give(&mut gs, &bo, 30).unwrap();
    let put = puts_to(&frames(&sent), &bo).remove(0);
    answer(&mut gs, &put, false, "reach");
    assert_eq!(puts_to(&frames(&sent), &ada).len(), 0, "refused: no self-copy");
    assert_eq!(given_to(&gs, &bo)[0].state, GivenState::Refused { reason: "reach".into() });
    assert_eq!(gs.household.line, "Bo's settings do not take messages from you right now.");
    assert_eq!(row_for(&gs, &bo).permit, Standing::Refused { reason: "reach".into() });

    give(&mut gs, &cy, 30).unwrap();
    let put = puts_to(&frames(&sent), &cy).remove(0);
    crate::engine::put_answer::expire(&mut gs, Instant::now() + Duration::from_secs(31));
    assert!(gs.household.on_their_way.is_empty(), "no longer waiting");
    assert_eq!(given_to(&gs, &cy)[0].state, GivenState::Unconfirmed);
    assert_eq!(gs.household.line, "The server did not say whether Cy got the permit. Give it again to be sure.");
    assert!(matches!(row_for(&gs, &cy).permit, Standing::Unconfirmed { .. }));
    answer(&mut gs, &put, true, "");
    assert!(frames(&sent).is_empty(), "a late answer sends nothing");
    assert_eq!(given_to(&gs, &cy)[0].state, GivenState::Unconfirmed, "and changes nothing");
    tidy(&gs);
}

/// `ingest_dm` HANDS A PERMIT NOTE TO THE HOUSEHOLD BEFORE ANYTHING IS STORED (plan 3.1). Ada is a
/// friend whose messages my settings let in (she holds my pass), so any other text of hers would
/// be stored. Her permit note, through the whole arrival path, is kept as a permit and never
/// stored as a message, listed as a conversation or put under Requests; so is a malformed note
/// (consumed, kept nowhere).
/// Seen red 2026-10-12 without the call in engine/dm.rs `ingest_dm`: "the note is not stored as a
/// message" failed (`ingest_dm` returned true: stored).
#[test]
fn ingest_dm_hands_a_permit_note_to_the_household_before_anything_is_stored() {
    crate::config::keep_saves_off_disk();
    let (bo_seed, bo) = identity(206);
    let (ada_seed, ada) = identity(207);
    let (mut gs, _sent) = app(&bo_seed, &bo, "ingest-dm", &[(&ada_seed, &ada, "Ada")]);
    let mine = crate::net::dm_store::SentPass { serial: "ab".repeat(16), may: crate::net::reach::intended_may_wire(Default::default()) };
    gs.dm_store.as_mut().unwrap().record_pass_sent(&ada, mine);
    assert!(gs.dm_store.as_ref().unwrap().admits_dm_from(&ada, false), "Ada's messages are let in");
    let t = now();
    let p = permit_by(&ada_seed, &ada, SERVER, "p1", &bo, SERIAL, t + 30 * DAY, t);
    assert!(!crate::engine::dm::ingest_dm(&mut gs, &dm(&ada, &bo, &crate::net::household::note_text(&p))), "the note is not stored as a message");
    assert!(!crate::engine::dm::ingest_dm(&mut gs, &dm(&ada, &bo, &format!("{CTL_PERMIT}{{not json"))), "nor is a malformed one");
    let store = gs.dm_store.as_ref().unwrap();
    assert!(store.conversation(&ada).is_empty() && gs.chat_dms.is_empty(), "no conversation");
    assert!(store.requests().is_empty(), "nothing under Requests");
    assert_eq!(store.permits_held().iter().map(|h| h.permit.clone()).collect::<Vec<_>>(), [p], "it is kept as a permit");
    tidy(&gs);
}

/// The fixture the scripted walker's DM library reads (lane 1E).
const FIXTURE: &str = "scripts/tests/fixtures/permit-note-v1.json";
/// Its fixed inputs: the giver's, the grantee's and the server's seed bytes, the plot, the serial,
/// when it was minted and the DM's signed time.
const FX_GIVER: u8 = 11;
const FX_GRANTEE: u8 = 12;
const FX_SERVER: u8 = 5;
const FX_PLOT: &str = "p2";
const FX_SERIAL: &str = "0f1e2d3c4b5a69788796a5b4c3d2e1f0";
const FX_MINTED: u64 = 1_760_000_000;
const FX_DAYS: u64 = 30;
const FX_DM_TS: u64 = 1_760_000_000_123;

fn fx_server() -> String {
    crate::relay::core::did::did_for_pubkey(&DilithiumKeypair::from_seed(&derive_dilithium_seed(&[FX_SERVER; 32])).public_key())
}

/// The fixture's permit, rebuilt from its fixed inputs (ML-DSA signing here is deterministic).
fn fx_permit() -> Permit {
    let (seed, giver) = identity(FX_GIVER);
    let (_, grantee) = identity(FX_GRANTEE);
    permit_by(&seed, &giver, &fx_server(), FX_PLOT, &grantee, FX_SERIAL, FX_MINTED + FX_DAYS * DAY, FX_MINTED)
}

/// WRITES scripts/tests/fixtures/permit-note-v1.json, the shared fixture lane 1E's DM library
/// (scripts/lib/sealed-dm.js) and the walker's tests read: one permit from fixed seeds, its signed
/// words, its note text, the inner DM the game signs around it, one envelope sealed to the grantee
/// by this code (sealing is random, so the envelope is written once and checked by opening it), and
/// a take-back note naming it. Run it again only when the shape changes on purpose:
/// `node scripts/lib-tests.js write_the_permit_note_fixture --ignored`.
#[test]
#[ignore = "writes scripts/tests/fixtures/permit-note-v1.json; run: node scripts/lib-tests.js write_the_permit_note_fixture --ignored"]
fn write_the_permit_note_fixture() {
    let (giver_seed, giver) = identity(FX_GIVER);
    let (grantee_seed, grantee) = identity(FX_GRANTEE);
    let p = fx_permit();
    let note = crate::net::household::note_text(&p);
    let inner = crate::net::dm_pq::build_signed_inner_ext(&giver_seed, &giver, &grantee, FX_DM_TS, &note, None).unwrap();
    let envelope = crate::net::dm_pq::seal_v2(&kyber(&grantee_seed), &inner).unwrap();
    let row = GivenPermit { to: grantee.clone(), plot: p.plot.clone(), serial: p.serial.clone(), expiry: p.expiry, at: FX_DM_TS, state: GivenState::TakenBack };
    let fx = json!({
        "about": "One household permit as a sealed DM note (ship homes increment 5b, docs/design/ship-homes-increment-5b-plan.md 3.1 and amendment A1, 2026-10-12). Minted by the game's own builder (src/relay/core/pq_crypto.rs build_plot_permit; ML-DSA signing there is deterministic) from the BIP39 seeds [giver_master_byte; 32] (the plot's holder) and [grantee_master_byte; 32] (the friend), on the server whose did:hum: comes from the seed [server_master_byte; 32], at minted_at (Unix seconds), running out days later. `preimage` is the exact text signed; `permit` the permit object in the order its words are signed; `note_text` the DM text exactly as the game writes it (the marker, then the permit's compact JSON). `inner` is the signed inner DM the game builds around it (Dilithium3 over hum/dm/v2, padded as the game pads), signed at dm_ts (ms); `envelope` is that inner sealed by the game to the grantee's DM key (ML-KEM-768, BLAKE3 derive_key hum/dm-aes/v1, AES-256-GCM): open it with the keys from recipient_seed_hex. `take_back_note_text` is the note to oneself the game sends when it takes this permit back: it also starts with the permit marker, so a client that swallows every text starting with the marker swallows it too. Written by src/engine/household_tests.rs write_the_permit_note_fixture; checked by the_rigs_permit_note_fixture_is_the_games.",
        "giver_master_byte": FX_GIVER,
        "grantee_master_byte": FX_GRANTEE,
        "server_master_byte": FX_SERVER,
        "giver_seed_hex": hex::encode(&giver_seed),
        "recipient_seed_hex": hex::encode(&grantee_seed),
        "giver": giver,
        "grantee": grantee,
        "grantee_did": plot_owner_id(&grantee),
        "server": fx_server(),
        "plot": FX_PLOT,
        "serial": FX_SERIAL,
        "minted_at": FX_MINTED,
        "days": FX_DAYS,
        "expiry": p.expiry,
        "preimage": crate::relay::core::pq_crypto::plot_permit_preimage(&p.server, &p.plot, &p.grantee, &p.serial, p.expiry),
        "permit": p,
        "note_text": note,
        "dm_ts": FX_DM_TS,
        "inner": inner,
        "envelope": envelope,
        "take_back_note_text": take_back_note_text(&[row]),
    });
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    std::fs::write(&path, format!("{}\n", serde_json::to_string_pretty(&fx).unwrap())).expect("the fixture is written");
}

/// THE RIG'S PERMIT NOTE FIXTURE IS THE GAME'S (lane 1A for lane 1E). The fixture's keys, server and
/// grantee are the ones its seeds give; its permit and signed words are what the builder makes from
/// them, byte for byte; its note text is the game's `note_text` and reads back as the permit; its
/// envelope opens with the recipient's seed to an inner DM from the giver to the grantee whose
/// signature checks and whose text is the note, and the grantee would keep that permit on the day
/// it was minted; its take-back note is the game's.
/// Seen red 2026-10-12 with the marker in src/net/dm_pq.rs changed to `[[hum:permit:v2]]`: "the note
/// text is the game's" failed (left `[[hum:permit:v1]]{...`, right `[[hum:permit:v2]]{...`).
#[test]
fn the_rigs_permit_note_fixture_is_the_games() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let fx: Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("the fixture is there")).expect("JSON");
    let s = |k: &str| fx[k].as_str().unwrap_or_else(|| panic!("fixture field {k}")).to_string();
    let (giver_seed, giver) = identity(FX_GIVER);
    let (grantee_seed, grantee) = identity(FX_GRANTEE);
    assert_eq!(
        (s("giver"), s("grantee"), s("grantee_did"), s("server")),
        (giver.clone(), grantee.clone(), plot_owner_id(&grantee), fx_server()),
        "the keys and ids the seeds give"
    );
    assert_eq!((s("giver_seed_hex"), s("recipient_seed_hex")), (hex::encode(&giver_seed), hex::encode(&grantee_seed)));
    let p = fx_permit();
    assert_eq!(serde_json::from_value::<Permit>(fx["permit"].clone()).expect("a permit"), p, "the builder makes the fixture's permit");
    assert_eq!(s("preimage"), crate::relay::core::pq_crypto::plot_permit_preimage(&p.server, &p.plot, &p.grantee, &p.serial, p.expiry));
    assert_eq!(s("note_text"), crate::net::household::note_text(&p), "the note text is the game's");
    assert_eq!(crate::net::household::parse_note(&s("note_text")), Some(p.clone()), "and reads back as the permit");

    let kp = crate::net::dm_pq::DmPqKeypair::from_bip39_seed(&grantee_seed).unwrap();
    let opened = crate::net::dm_pq::open_v2(&kp, &s("envelope")).expect("the envelope opens with the recipient's seed");
    assert_eq!(opened, s("inner"), "to the fixture's inner");
    let inner = crate::net::dm_pq::parse_verify_inner(&opened).expect("whose signature checks");
    assert_eq!((inner.from.as_str(), inner.to.as_str(), inner.ts, inner.text.as_str()), (giver.as_str(), grantee.as_str(), FX_DM_TS, s("note_text").as_str()));
    assert_eq!(crate::net::household::judge_arrival(&p, &giver, Some(&fx_server()), &plot_owner_id(&grantee), FX_MINTED), Ok(()), "the grantee keeps it");
    let row = GivenPermit { to: grantee.clone(), plot: p.plot.clone(), serial: p.serial.clone(), expiry: p.expiry, at: FX_DM_TS, state: GivenState::TakenBack };
    assert_eq!(s("take_back_note_text"), take_back_note_text(&[row]), "the take-back note is the game's");
    assert!(crate::net::household::is_note(&s("take_back_note_text")), "and starts with the permit marker");
}

/// TAKE BACK SENDS `cert_revoke` AND RESENDS IT UNTIL THE RELAY CONFIRMS (amendment A1). Ada took
/// back the permit Bo holds: the row says taken back at once, its serial goes to the relay
/// (`cert_revoke`) and waits among the withdrawals, and a note to Ada's own mailbox names it; the
/// row reads "Taking it back...". The next member list sends it again; once the relay confirms
/// (`cert_revoked`) it is sent no more and the row reads taken back. Taking back again finds
/// nothing. Done offline, the row is marked and the serial waits all the same.
/// Seen red 2026-10-12 with `take_back` not calling `send_pending_withdrawals`: "the withdrawal goes
/// out at once" failed, left `[]`, right `["<serial>"]`.
#[test]
fn take_back_sends_cert_revoke_and_resends_until_confirmed() {
    crate::config::keep_saves_off_disk();
    let (ada_seed, ada) = identity(208);
    let (bo_seed, bo) = identity(209);
    let (cy_seed, cy) = identity(210);
    let (mut gs, sent) = app(&ada_seed, &ada, "take-back", &[(&bo_seed, &bo, "Bo"), (&cy_seed, &cy, "Cy")]);
    gs.household.my_plot = Some("p2".into());
    give(&mut gs, &bo, 30).unwrap();
    let put = puts_to(&frames(&sent), &bo).remove(0);
    answer(&mut gs, &put, true, "");
    frames(&sent);
    let serial = given_to(&gs, &bo)[0].serial.clone();

    assert_eq!(take_back(&mut gs, &bo), Ok(1));
    let out = frames(&sent);
    assert_eq!(revoked(&out), [serial.clone()], "the withdrawal goes out at once");
    let notes = puts_to(&out, &ada);
    assert_eq!(notes.len(), 1, "with one note to my own mailbox");
    let note = open(&notes[0], &ada_seed);
    assert_eq!((note.to.as_str(), note.text.as_str()), (ada.as_str(), take_back_note_text(&given_to(&gs, &bo)).as_str()));
    assert_eq!(state_of(&gs, &serial), Some(GivenState::TakenBack), "the row says so at once");
    assert_eq!(gs.dm_store.as_ref().unwrap().pending_withdrawals(), [serial.clone()], "and its serial waits for the relay");
    assert!(matches!(row_for(&gs, &bo).permit, Standing::TakingBack { .. }), "Taking it back...");

    crate::engine::dm::send_pending_withdrawals(&mut gs);
    assert_eq!(revoked(&frames(&sent)), [serial.clone()], "the next member list sends it again");
    confirmed(&mut gs, &serial);
    crate::engine::dm::send_pending_withdrawals(&mut gs);
    assert!(revoked(&frames(&sent)).is_empty(), "once confirmed it is sent no more");
    assert!(matches!(row_for(&gs, &bo).permit, Standing::TakenBack { .. }));
    assert!(take_back(&mut gs, &bo).unwrap_err().contains("Bo holds no permit from you to take back"));

    // Offline: marked and waiting all the same, sent with the next connection's withdrawals.
    give(&mut gs, &cy, 30).unwrap();
    let put = puts_to(&frames(&sent), &cy).remove(0);
    answer(&mut gs, &put, true, "");
    frames(&sent);
    let cy_serial = given_to(&gs, &cy)[0].serial.clone();
    let client = gs.ws_client.take();
    assert_eq!(take_back(&mut gs, &cy), Ok(1));
    assert_eq!(state_of(&gs, &cy_serial), Some(GivenState::TakenBack));
    assert_eq!(gs.dm_store.as_ref().unwrap().pending_withdrawals(), [cy_serial.clone()]);
    gs.ws_client = client;
    crate::engine::dm::send_pending_withdrawals(&mut gs);
    assert_eq!(revoked(&frames(&sent)), [cy_serial], "sent on the next connection");
    tidy(&gs);
}

/// MY OTHER DEVICES LEARN WHAT I TOOK BACK (amendment A1, the self-copy pattern). Ada's second device
/// lists the permit to Bo as given; the note her first device sent when it took it back marks it
/// taken back there too. A device that never saw the permit lists it as taken back from the note
/// alone, and the permit's own echo read after that does not bring it back. A take-back note from
/// anyone but me, to anyone but me, changes nothing.
/// Seen red 2026-10-12 with the to-myself arm of `take_note` dropping the note: "my other device
/// lists it as taken back" failed, left `Some(Taken)`, right `Some(TakenBack)`.
#[test]
fn my_other_devices_learn_what_i_took_back() {
    crate::config::keep_saves_off_disk();
    let (ada_seed, ada) = identity(211);
    let (bo_seed, bo) = identity(212);
    let (mut one, sent) = app(&ada_seed, &ada, "learn-one", &[(&bo_seed, &bo, "Bo")]);
    one.household.my_plot = Some("p2".into());
    give(&mut one, &bo, 30).unwrap();
    let put = puts_to(&frames(&sent), &bo).remove(0);
    answer(&mut one, &put, true, "");
    let echo = open(&puts_to(&frames(&sent), &ada).remove(0), &ada_seed);
    let serial = given_to(&one, &bo)[0].serial.clone();
    take_back(&mut one, &bo).unwrap();
    let note = open(&puts_to(&frames(&sent), &ada).remove(0), &ada_seed);

    let (mut two, _) = app(&ada_seed, &ada, "learn-two", &[(&bo_seed, &bo, "Bo")]);
    assert!(ingest(&mut two, &echo));
    assert_eq!(state_of(&two, &serial), Some(GivenState::Taken));
    let (_, cy) = identity(213);
    assert!(ingest(&mut two, &DmInner { from: cy.clone(), ..note.clone() }), "consumed");
    assert!(ingest(&mut two, &DmInner { to: bo.clone(), ..note.clone() }), "consumed");
    assert_eq!(state_of(&two, &serial), Some(GivenState::Taken), "a note not from me to me changes nothing");
    assert!(ingest(&mut two, &note));
    assert_eq!(state_of(&two, &serial), Some(GivenState::TakenBack), "my other device lists it as taken back");

    let (mut three, _) = app(&ada_seed, &ada, "learn-three", &[(&bo_seed, &bo, "Bo")]);
    assert!(ingest(&mut three, &note));
    assert_eq!(state_of(&three, &serial), Some(GivenState::TakenBack), "from the note alone");
    assert!(ingest(&mut three, &echo));
    assert_eq!(state_of(&three, &serial), Some(GivenState::TakenBack), "and the echo read after it does not bring it back");
    tidy(&one);
    tidy(&two);
    tidy(&three);
}

/// BLOCK TAKES BACK EVERY PERMIT GIVEN TO THAT PERSON (amendment A1), in the step that withdraws
/// their passes. Ada blocks Bo: his permit for p2 and the unanswered one for her old plot p3 are
/// taken back, their serials go to the relay with his pass's, and the one the server refused (it
/// never reached him) and Cy's are untouched. Blocked while offline, the same is marked and waits.
/// A permit to someone blocked that my other device gave before it heard of the block is taken
/// back as soon as its echo arrives.
/// Seen red 2026-10-12 with `send_pending_withdrawals` not calling `take_back_from_blocked`: "Bo's
/// permits are taken back" failed, left `[Some(Taken), Some(Unconfirmed)]`, right
/// `[Some(TakenBack), Some(TakenBack)]`.
#[test]
fn block_takes_back_every_permit_given_to_that_person() {
    crate::config::keep_saves_off_disk();
    let (ada_seed, ada) = identity(214);
    let (bo_seed, bo) = identity(215);
    let (cy_seed, cy) = identity(216);
    let (mut gs, sent) = app(&ada_seed, &ada, "block", &[(&bo_seed, &bo, "Bo"), (&cy_seed, &cy, "Cy")]);
    let (b1, b2, b3, c1) = ("b1".repeat(16), "b2".repeat(16), "b3".repeat(16), "c1".repeat(16));
    let store = gs.dm_store.as_mut().unwrap();
    store.record_given(given_row(&bo, "p2", &b1, DAY, 30, GivenState::Taken), now());
    store.record_given(given_row(&bo, "p3", &b2, 2 * DAY, 30, GivenState::Unconfirmed), now());
    store.record_given(given_row(&bo, "p2", &b3, 3 * DAY, 30, GivenState::Refused { reason: "rate".into() }), now());
    store.record_given(given_row(&cy, "p2", &c1, DAY, 30, GivenState::Taken), now());
    let pass = crate::net::dm_store::SentPass { serial: "d1".repeat(16), may: "invite".into() };
    store.record_pass_sent(&bo, pass.clone());

    crate::engine::block::block(&mut gs, &bo);
    assert_eq!([state_of(&gs, &b1), state_of(&gs, &b2)], [Some(GivenState::TakenBack), Some(GivenState::TakenBack)], "Bo's permits are taken back");
    assert_eq!(state_of(&gs, &b3), Some(GivenState::Refused { reason: "rate".into() }), "the refused one never reached him");
    assert_eq!(state_of(&gs, &c1), Some(GivenState::Taken), "Cy's is untouched");
    let mut out = revoked(&frames(&sent));
    out.sort();
    assert_eq!(out, [b1.clone(), b2.clone(), pass.serial.clone()], "with his pass, to the relay");

    // Offline.
    let (mut off, _off_sent) = app(&ada_seed, &ada, "block-off", &[(&bo_seed, &bo, "Bo")]);
    off.dm_store.as_mut().unwrap().record_given(given_row(&bo, "p2", &b1, DAY, 30, GivenState::Taken), now());
    off.ws_client = None;
    crate::engine::block::block(&mut off, &bo);
    assert_eq!(state_of(&off, &b1), Some(GivenState::TakenBack), "offline too");
    assert!(off.dm_store.as_ref().unwrap().pending_withdrawals().contains(&b1), "and it waits for the next connection");

    // My other device's permit to Bo, given before it heard of the block, echoed here after it.
    for s in [&b1, &b2, &pass.serial] {
        confirmed(&mut gs, s);
    }
    frames(&sent);
    let t = now();
    let late = permit_by(&ada_seed, &ada, SERVER, "p2", &bo, &"e1".repeat(16), t + 30 * DAY, t);
    assert!(!crate::engine::dm::ingest_dm(&mut gs, &dm(&ada, &bo, &crate::net::household::note_text(&late))), "not shown");
    assert_eq!(state_of(&gs, &"e1".repeat(16)), Some(GivenState::TakenBack), "an echo for someone blocked is taken back at once");
    assert_eq!(revoked(&frames(&sent)), ["e1".repeat(16)]);
    tidy(&gs);
    tidy(&off);
}

/// RENEW TAKES BACK THE OLD PERMIT ONCE THE NEW ONE IS TAKEN (amendment A1: one permit per friend
/// and plot is ever live). While the renewal is on its way the old permit stands and nothing is
/// withdrawn; on `dm_put_ok` the new one's self-copy goes, the old one is taken back (its serial
/// to the relay, a note to my own mailbox), and Bo's row shows the new end date. A renewal the
/// server refused takes nothing back.
/// Seen red 2026-10-12 with the take-back of older permits removed from `on_answer`: "the old one is
/// taken back now" failed, left `Some(Taken)`, right `Some(TakenBack)`.
#[test]
fn renew_takes_back_the_old_permit_once_the_new_one_is_taken() {
    crate::config::keep_saves_off_disk();
    let (ada_seed, ada) = identity(217);
    let (bo_seed, bo) = identity(218);
    let (mut gs, sent) = app(&ada_seed, &ada, "renew-old", &[(&bo_seed, &bo, "Bo")]);
    gs.household.my_plot = Some("p2".into());
    let old = "f1".repeat(16);
    gs.dm_store.as_mut().unwrap().record_given(given_row(&bo, "p2", &old, 10 * DAY, 30, GivenState::Taken), now());

    renew(&mut gs, &bo).unwrap();
    let out = frames(&sent);
    assert!(revoked(&out).is_empty() && puts_to(&out, &ada).is_empty(), "nothing withdrawn and no self-copy yet");
    assert_eq!(state_of(&gs, &old), Some(GivenState::Taken), "the old one stands until the server took the new one");
    let put = puts_to(&out, &bo).remove(0);
    let new = crate::net::household::parse_note(&open(&put, &bo_seed).text).unwrap();
    answer(&mut gs, &put, true, "");
    let out = frames(&sent);
    assert_eq!(state_of(&gs, &old), Some(GivenState::TakenBack), "the old one is taken back now");
    assert_eq!(revoked(&out), [old.clone()], "its serial to the relay");
    let mine: Vec<String> = puts_to(&out, &ada).iter().map(|p| open(p, &ada_seed).text).collect();
    assert_eq!(mine.len(), 2, "the new one's self-copy and the take-back note");
    assert!(mine.contains(&crate::net::household::note_text(&new)) && mine.iter().any(|t| t.contains(&old) && t.contains("taken_back")));
    assert_eq!(row_for(&gs, &bo).permit, Standing::Stands { expiry: new.expiry, days: 30 });

    renew(&mut gs, &bo).unwrap();
    let put = puts_to(&frames(&sent), &bo).remove(0);
    answer(&mut gs, &put, false, "rate");
    assert!(frames(&sent).is_empty(), "a refused renewal takes nothing back");
    assert_eq!(state_of(&gs, &new.serial), Some(GivenState::Taken));
    assert_eq!(gs.household.line, "The server is busy; try again in a moment.");
    tidy(&gs);
}

/// THE CARD'S "YOUR PLOT" ROWS SAY WHERE EACH PERMIT STANDS (plan 3.9, amendment A1). Every friend
/// is listed (Ann, given nothing), and so is someone no longer a friend who still holds one (Ivy);
/// each row says one thing: standing with its length, on its way, unconfirmed (sent before a
/// restart too), refused, ran out, being taken back, taken back. A permit for a plot no longer
/// mine is listed apart. Rows are by name.
/// Seen red 2026-10-12 with `friends` listing only mutual follows (the given people not added):
/// "everyone listed, by name" failed (Ivy missing).
#[test]
fn the_cards_rows_say_where_each_permit_stands() {
    crate::config::keep_saves_off_disk();
    let people: Vec<(Vec<u8>, String, &str)> =
        ["Ann", "Bo", "Cy", "Dee", "Eve", "Fay", "Gus", "Hal"].iter().enumerate().map(|(i, n)| {
            let (s, k) = identity(220 + i as u8);
            (s, k, *n)
        }).collect();
    let (my_seed, me) = identity(219);
    let friends_in: Vec<(&[u8], &str, &str)> = people.iter().map(|(s, k, n)| (s.as_slice(), k.as_str(), *n)).collect();
    let (mut gs, _sent) = app(&my_seed, &me, "rows", &friends_in);
    let (_, ivy) = identity(230);
    gs.chat_users.push(crate::gui::ChatUser { name: "Ivy".into(), public_key: ivy.clone(), role: String::new(), status: String::new() });
    gs.household.my_plot = Some("p2".into());
    let k = |n: &str| people.iter().find(|p| p.2 == n).unwrap().1.clone();
    let serial = |i: u8| format!("{i:02x}").repeat(16);
    let t = now();
    let given_row = |to: &str, plot: &str, serial: &str, ago: u64, days: u64, state: GivenState| given_at(t, to, plot, serial, ago, days, state);
    let store = gs.dm_store.as_mut().unwrap();
    store.record_given(given_row(&k("Bo"), "p2", &serial(1), DAY, 30, GivenState::Taken), now());
    store.record_given(given_row(&k("Bo"), "p3", &serial(2), DAY, 30, GivenState::Taken), now());
    store.record_given(given_row(&k("Cy"), "p2", &serial(3), 0, 7, GivenState::OnItsWay { reference: "live".into() }), now());
    store.record_given(given_row(&k("Dee"), "p2", &serial(4), DAY, 30, GivenState::OnItsWay { reference: "before-restart".into() }), now());
    store.record_given(given_row(&k("Eve"), "p2", &serial(5), DAY, 30, GivenState::Refused { reason: "rate".into() }), now());
    store.record_given(given_row(&k("Fay"), "p2", &serial(6), 40 * DAY, 30, GivenState::Taken), now());
    store.record_given(given_row(&k("Gus"), "p2", &serial(7), DAY, 30, GivenState::Taken), now());
    store.record_given(given_row(&k("Hal"), "p2", &serial(8), DAY, 30, GivenState::Taken), now());
    store.record_given(given_row(&ivy, "p2", &serial(9), DAY, 90, GivenState::Taken), now());
    store.take_back_permits(&k("Gus"), |_| false);
    store.take_back_permits(&k("Hal"), |_| false);
    gs.household.on_their_way.push(PermitPut {
        reference: "live".into(),
        peer: k("Cy"),
        plot: "p2".into(),
        serial: serial(3),
        expiry: t + 7 * DAY,
        days: 7,
        self_copy: Value::Null,
        at: Instant::now(),
    });
    confirmed(&mut gs, &serial(8));

    let rows = friends(&gs);
    let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["Ann", "Bo", "Cy", "Dee", "Eve", "Fay", "Gus", "Hal", "Ivy"], "everyone listed, by name");
    let by = |n: &str| rows.iter().find(|r| r.name == n).unwrap().clone();
    let expiry = |ago: u64, days: u64| t - ago + days * DAY;
    assert_eq!(by("Ann").permit, Standing::None);
    assert_eq!(by("Bo").permit, Standing::Stands { expiry: expiry(DAY, 30), days: 30 });
    assert_eq!(by("Bo").other_plots, [("p3".to_string(), expiry(DAY, 30))], "the old plot's permit, apart");
    assert_eq!(by("Cy").permit, Standing::Sending { expiry: expiry(0, 7) });
    assert_eq!(by("Dee").permit, Standing::Unconfirmed { expiry: expiry(DAY, 30) }, "sent before a restart");
    assert_eq!(by("Eve").permit, Standing::Refused { reason: "rate".into() });
    assert_eq!(by("Fay").permit, Standing::RanOut { expiry: expiry(40 * DAY, 30) });
    assert_eq!(by("Gus").permit, Standing::TakingBack { expiry: expiry(DAY, 30) });
    assert_eq!(by("Hal").permit, Standing::TakenBack { expiry: expiry(DAY, 30) });
    let ivy_row = by("Ivy");
    assert_eq!((ivy_row.friend, ivy_row.sealable, ivy_row.permit), (false, false, Standing::Stands { expiry: expiry(DAY, 90), days: 90 }));
    assert!(by("Bo").friend && by("Bo").sealable && !by("Bo").blocked);
    assert_eq!(days_left(now() + 29 * DAY + 5, now()), 30, "a part day counts");
    tidy(&gs);
}

/// A PARKED SERVER'S PERMIT NOTE IS KEPT IN THAT SERVER'S STORE (engine/bg_connections.rs
/// `bg_screen`, by its wiring line). I am on server A; Ada's permit for her plot on server B
/// arrives by way of my parked connection to B: it is kept in B's store, the store of the server
/// it came by, and not in A's; a message that is not a permit note is not taken.
/// Seen red 2026-10-12 with `screens_dm_parked` judging against the active store instead of the
/// parked one: "kept in the parked server's store" failed, left 0, right 1.
#[test]
fn a_parked_servers_permit_note_is_kept_in_that_servers_store() {
    crate::config::keep_saves_off_disk();
    let (bo_seed, bo) = identity(231);
    let (ada_seed, ada) = identity(232);
    let (mut gs, _sent) = app(&bo_seed, &bo, "parked-a", &[(&ada_seed, &ada, "Ada")]);
    let server_b = "did:hum:ServerB1111111111111";
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let mut parked = DmStore::load(&bo_seed, &bo, &format!("https://household-parked-b-{nanos}.example"));
    parked.set_pass_server(server_b);
    parked.set_following(&ada, true);
    parked.set_follower(&ada, true);
    let t = now();
    let p = permit_by(&ada_seed, &ada, server_b, "p4", &bo, SERIAL, t + 30 * DAY, t);
    assert!(screens_dm_parked(&mut gs, &mut parked, &dm(&ada, &bo, &crate::net::household::note_text(&p))), "taken");
    assert_eq!(parked.permits_held().len(), 1, "kept in the parked server's store");
    assert!(gs.dm_store.as_ref().unwrap().permits_held().is_empty(), "not in the active one");
    assert!(gs.pending_notices.is_empty(), "and no notice while that server is parked");
    assert!(!screens_dm_parked(&mut gs, &mut parked, &dm(&ada, &bo, "hello")), "a message is not taken");
    parked.remove_file_for_test();
    tidy(&gs);
}

/// GIVING IS REFUSED UNDER THE PROTECTED SETUP (lane 1A's review, 2026-10-12): a protected account
/// does not hand out a way into its home. With the setup on, Give and Renew each say so in a plain
/// sentence (the PIN does not let them through: no PIN prompt opens), and nothing is minted, sent or
/// listed. Take back, which only shuts a way in, still works. With the setup off again, Give goes.
/// Seen red 2026-10-12 with the check removed from `give` and from `renew`: "with the setup on,
/// Give is refused" failed, left `Ok(())`.
#[test]
fn giving_is_refused_under_the_protected_setup() {
    crate::config::keep_saves_off_disk();
    let (ada_seed, ada) = identity(236);
    let (bo_seed, bo) = identity(237);
    let (mut gs, sent) = app(&ada_seed, &ada, "protected", &[(&bo_seed, &bo, "Bo")]);
    gs.household.my_plot = Some("p2".into());
    let old = "c3".repeat(16);
    gs.dm_store.as_mut().unwrap().record_given(given_row(&bo, "p2", &old, DAY, 30, GivenState::Taken), now());
    gs.protected.setup.on = true;

    let given = gs.dm_store.as_ref().unwrap().permits_given().len();
    let refused = give(&mut gs, &bo, 30);
    assert_eq!(refused, Err(PROTECTED.to_string()), "with the setup on, Give is refused");
    assert_eq!(renew(&mut gs, &bo), Err(PROTECTED.to_string()), "and so is Renew");
    assert!(gs.protected.prompt.is_none(), "no PIN prompt: the PIN does not let it through");
    assert!(frames(&sent).is_empty(), "nothing sent");
    assert_eq!(gs.dm_store.as_ref().unwrap().permits_given().len(), given, "nothing listed");
    assert!(gs.household.on_their_way.is_empty());
    for word in ["certificate", "signature", "key", "token", "revoke", "verify"] {
        assert!(!PROTECTED.to_lowercase().contains(word), "{word:?} in {PROTECTED:?}");
    }

    assert_eq!(take_back(&mut gs, &bo), Ok(1), "Take back still works");
    assert_eq!(revoked(&frames(&sent)), [old], "its serial goes to the relay");

    gs.protected.setup.on = false;
    give(&mut gs, &bo, 30).expect("with the setup off, Give goes");
    assert_eq!(puts_to(&frames(&sent), &bo).len(), 1);
    tidy(&gs);
}

/// THE PERMIT A BUILD ON A PLOT SHOWS (lane 2A's `send_next`): of the permits held for that plot, one
/// the relay said works this session before any other, else the latest-ending; never one that
/// ran out; none for a plot I hold no permit for.
/// Seen red 2026-10-12 with the "works first" preference removed from `permit_for_at` (latest end
/// date only): "the one that works" failed (Cy's, ending later, was picked).
#[test]
fn permit_for_picks_the_working_permit_for_that_plot() {
    crate::config::keep_saves_off_disk();
    let (bo_seed, bo) = identity(233);
    let (ada_seed, ada) = identity(234);
    let (cy_seed, cy) = identity(235);
    let (mut gs, _sent) = app(&bo_seed, &bo, "permit-for", &[(&ada_seed, &ada, "Ada"), (&cy_seed, &cy, "Cy")]);
    let t = now();
    let ada_p1 = permit_by(&ada_seed, &ada, SERVER, "p1", &bo, SERIAL, t + 10 * DAY, t);
    let cy_p1 = permit_by(&cy_seed, &cy, SERVER, "p1", &bo, SERIAL, t + 60 * DAY, t);
    let cy_p5 = permit_by(&cy_seed, &cy, SERVER, "p5", &bo, SERIAL, t + 60 * DAY, t);
    for (from, p) in [(&ada, &ada_p1), (&cy, &cy_p1), (&cy, &cy_p5)] {
        assert!(ingest(&mut gs, &dm(from, &bo, &crate::net::household::note_text(p))));
    }
    assert_eq!(permit_for(&gs, "p1"), Some(cy_p1.clone()), "with no word from the relay, the latest-ending");
    let held = gs.dm_store.as_ref().unwrap().permits_held().to_vec();
    let ada_held = held.iter().find(|h| h.from == ada).unwrap();
    let cy_held = held.iter().find(|h| h.from == cy && h.permit.plot == "p1").unwrap();
    gs.household.verdicts.insert(PermitKey::of(ada_held), Judged { verdict: Verdict::Works, at: 0.0 });
    gs.household.verdicts.insert(PermitKey::of(cy_held), Judged { verdict: Verdict::Refused(crate::systems::construction::shared::Why::PermitNotFromHolder), at: 0.0 });
    assert_eq!(permit_for(&gs, "p1"), Some(ada_p1.clone()), "the one that works");
    assert_eq!(permit_for_at(&gs, "p1", t + 30 * DAY), Some(cy_p1), "never one that ran out");
    assert_eq!(permit_for(&gs, "p9"), None);
    tidy(&gs);
}
