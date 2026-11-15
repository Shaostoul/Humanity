// Child of engine/org_mail.rs (#[path]): organisations rung 2's delivery between two desktop apps,
// each with its own state and a socket that records what is sent, no screen and no server: what one
// app sends is opened with the other's seed and handed to its arrival rule, as the relay would hand
// it over. Each test was seen failing once on purpose, recorded at the test.

use super::*;
use crate::net::dm_pq::DmInner;
use humanity_protocol::org::{Act, Directory, MEMBER};
use humanity_protocol::pq_crypto::{derive_dilithium_seed, DilithiumKeypair};
use serde_json::Value;
use std::sync::mpsc::Receiver;

fn identity(n: u8) -> (Vec<u8>, String) {
    let seed = vec![n; 32];
    (seed.clone(), hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&seed)).public_key()))
}

fn kyber(seed: &[u8]) -> String {
    crate::net::dm_pq::DmPqKeypair::from_bip39_seed(seed).unwrap().public_base64()
}

/// A signed-in app whose socket records what is sent, its mailbox read, holding the checked key
/// cards of `others` (seed, key).
fn app(seed: &[u8], me: &str, others: &[(&[u8], &str)]) -> (GuiState, Receiver<String>) {
    let mut gs = GuiState::default();
    gs.profile_public_key = me.to_string();
    gs.private_key_bytes = Some(seed.to_vec());
    for (s, k) in others {
        gs.peer_kyber_keys.insert(k.to_string(), kyber(s));
    }
    let (client, sent) = crate::net::ws_client::WsClient::recording();
    gs.ws_client = Some(client);
    gs.dm_fetch = crate::net::mailbox_fetch::MailboxFetch::already_read();
    assert!(ensure(&mut gs));
    (gs, sent)
}

/// Pump `gs`'s waiting deliveries out (as many frames as it takes), and hand every `dm_put` sealed
/// to `to` (key `to_key`, seed `to_seed`) to that app's arrival rule. How many notes arrived.
fn deliver(gs: &mut GuiState, sent: &Receiver<String>, to: &mut GuiState, to_key: &str, to_seed: &[u8]) -> usize {
    let mut n = 0;
    for _ in 0..20 {
        gs.pass_pacer = Default::default();
        pump(gs, std::time::Instant::now());
        let frames: Vec<Value> = sent.try_iter().map(|f| serde_json::from_str(&f).unwrap()).collect();
        for put in frames.iter().filter(|v| v["type"] == "dm_put" && v["to"] == to_key) {
            let kp = crate::net::dm_pq::DmPqKeypair::from_bip39_seed(to_seed).unwrap();
            let inner: DmInner = crate::net::dm_pq::parse_verify_inner(&crate::net::dm_pq::open_v2(&kp, put["content"].as_str().unwrap()).unwrap()).unwrap();
            assert!(screens_dm(to, &inner), "an organisation note is never shown as a message");
            n += 1;
        }
        if gs.orgs_page.outbox_len() == 0 {
            break;
        }
    }
    n
}

fn state(gs: &mut GuiState, org: &str) -> humanity_protocol::org::OrgState {
    gs.orgs_page.state_of(org).cloned().expect("the organisation folds here")
}

/// The whole of rung 2's delivery between two apps, with nobody copying text: A founds and invites
/// B; the invitation reaches B's app, which shows it; B accepts; the acceptance reaches A, whose app
/// sends B the whole log; A's later change reaches B and applies there; both apps fold the same
/// bytes to the same log. Then A removes B: the removal reaches B, and nothing after it does.
///
/// Red first, 2026-11-15: with `forward_joins` not called in `take`, "B holds the whole log" failed
/// (B's app held only the invitation's chain and its own acceptance, not A's change made while the
/// invitation was open); with `recipients` also listing everyone ever invited, "nothing after the
/// removal reaches B" failed.
#[test]
fn a_change_delivered_by_sealed_dm_is_applied_by_the_other_members_app() {
    let ((sa, ka), (sb, kb)) = (identity(41), identity(42));
    let (mut a, sent_a) = app(&sa, &ka, &[(&sb, &kb)]);
    let (mut b, sent_b) = app(&sb, &kb, &[(&sa, &ka)]);
    let org = a.orgs_page.found(&sa, "Cart crew", "Saturday cans", Directory::Members).expect("founds");
    let b_key = hex::decode(&kb).unwrap();
    let (inv, refused) = a.orgs_page.sign(&sa, &org, Act::Invite { who: b_key.clone(), role: MEMBER.into(), until: now_ms() + 86_400_000 }).unwrap();
    assert_eq!(refused, None);
    let inv = inv.object_id().unwrap().to_hex();

    assert!(deliver(&mut a, &sent_a, &mut b, &kb, &sb) >= 1, "the invitation reaches B");
    assert!(b.orgs_page.objects(&org).len() >= 2, "B's app keeps the invitation");
    // A change while the invitation is open: not in the invitation's chain, so B gets it only with the
    // whole log the inviter sends after B joins.
    a.orgs_page.sign(&sa, &org, Act::Settings { name: None, about: Some("Saturday cans, Kitsap".into()), directory: None }).unwrap();
    deliver(&mut a, &sent_a, &mut b, &kb, &sb);
    let (_, refused) = b.orgs_page.sign(&sb, &org, Act::Accept { invite: inv }).unwrap();
    assert_eq!(refused, None);

    assert!(deliver(&mut b, &sent_b, &mut a, &ka, &sa) >= 1, "the acceptance reaches A");
    assert!(state(&mut a, &org).member(&b_key).is_some(), "A's app applied B's acceptance");
    deliver(&mut a, &sent_a, &mut b, &kb, &sb);
    assert_eq!(b.orgs_page.objects(&org).len(), a.orgs_page.objects(&org).len(), "B holds the whole log");

    a.orgs_page.sign(&sa, &org, Act::Settings { name: Some("Cart crew of Kitsap".into()), about: None, directory: None }).unwrap();
    assert!(deliver(&mut a, &sent_a, &mut b, &kb, &sb) >= 1);
    let (sa_state, sb_state) = (state(&mut a, &org), state(&mut b, &org));
    assert_eq!(sb_state.name, "Cart crew of Kitsap", "A's change applied in B's app");
    assert_eq!(sb_state.held, 0, "nothing waits in B's app");
    assert_eq!(sa_state.log, sb_state.log, "both apps fold the same log");

    a.orgs_page.sign(&sa, &org, Act::Remove { who: b_key.clone() }).unwrap();
    assert!(deliver(&mut a, &sent_a, &mut b, &kb, &sb) >= 1, "the removal reaches B");
    assert!(state(&mut b, &org).member(&b_key).is_none(), "B's app knows B was removed");
    a.orgs_page.sign(&sa, &org, Act::Settings { name: Some("Cans".into()), about: None, directory: None }).unwrap();
    assert_eq!(deliver(&mut a, &sent_a, &mut b, &kb, &sb), 0, "nothing after the removal reaches B");
}

/// A forged change is refused: a note whose entry's signature was broken on the way is merged as
/// bytes but never applied (the fold drops it), and a note sealed by someone outside the
/// organisation is not merged at all.
///
/// Red first, 2026-11-15: with `judge` answering Merge for everyone, "a stranger's note is not
/// merged" failed (the stranger's entry sat in B's log).
#[test]
fn a_forged_or_strangers_change_is_never_applied() {
    let ((sa, ka), (sb, kb), (ss, ks)) = (identity(51), identity(52), identity(59));
    let (mut a, sent_a) = app(&sa, &ka, &[(&sb, &kb)]);
    let (mut b, sent_b) = app(&sb, &kb, &[(&sa, &ka)]);
    let org = a.orgs_page.found(&sa, "Club", "", Directory::Members).unwrap();
    let (inv, _) = a.orgs_page.sign(&sa, &org, Act::Invite { who: hex::decode(&kb).unwrap(), role: MEMBER.into(), until: now_ms() + 86_400_000 }).unwrap();
    deliver(&mut a, &sent_a, &mut b, &kb, &sb);
    b.orgs_page.sign(&sb, &org, Act::Accept { invite: inv.object_id().unwrap().to_hex() }).unwrap();
    deliver(&mut b, &sent_b, &mut a, &ka, &sa);
    deliver(&mut a, &sent_a, &mut b, &kb, &sb);
    let before = state(&mut b, &org);

    // A real change by A with its signature broken, delivered to B in A's own note.
    let heads = state(&mut a, &org).heads;
    let kp_a = DilithiumKeypair::from_seed(&derive_dilithium_seed(&sa));
    let mut forged = humanity_protocol::org::build_entry(&kp_a, &org, &Act::Remove { who: hex::decode(&kb).unwrap() }, &heads, now_ms()).unwrap();
    forged.signature[7] ^= 0xff;
    let text = crate::gui::org_mail::note_texts(&org, &[forged]).remove(0);
    let inner = DmInner { from: ka.clone(), to: kb.clone(), ts: now_ms(), text, sig_b64: String::new(), cert: None };
    assert!(screens_dm(&mut b, &inner));
    let after = state(&mut b, &org);
    assert!(after.member(&hex::decode(&kb).unwrap()).is_some(), "the forged removal is not applied");
    assert_eq!(after.log, before.log);

    // A stranger's own signed change, sealed to B: not merged.
    let kp_s = DilithiumKeypair::from_seed(&derive_dilithium_seed(&ss));
    let theirs = humanity_protocol::org::build_entry(&kp_s, &org, &Act::Leave, &heads, now_ms()).unwrap();
    let n = b.orgs_page.objects(&org).len();
    let text = crate::gui::org_mail::note_texts(&org, &[theirs]).remove(0);
    let inner = DmInner { from: ks.clone(), to: kb.clone(), ts: now_ms(), text, sig_b64: String::new(), cert: None };
    assert!(screens_dm(&mut b, &inner));
    assert_eq!(b.orgs_page.objects(&org).len(), n, "a stranger's note is not merged");
}
