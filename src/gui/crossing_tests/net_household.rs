//! From net/household.rs's tests: the household permits, their owners named the way the relay names
//! a plot's owner (`relay::storage::plot_owner_id`).

use crate::net::household::*;
use humanity_core::build_permission::Permit;
use crate::net::dm_pq::CTL_PERMIT;
use crate::net::household::tests::DAY;
use crate::net::household::tests::NOW;
use crate::net::household::tests::SERIAL;
use crate::net::household::tests::SERVER;
use crate::net::household::tests::key;
use humanity_protocol::pq_crypto::DilithiumKeypair;
use humanity_protocol::pq_crypto::PLOT_PERMIT_FIELD_MAX_LEN;
use humanity_protocol::pq_crypto::build_plot_permit;
use humanity_protocol::pq_crypto::derive_dilithium_seed;
use humanity_protocol::pq_crypto::plot_permit_preimage;
use crate::relay::storage::plot_owner_id;
use humanity_core::build_permission::Why;
use crate::net::household::tests::given;
use std::collections::BTreeSet;

/// Their `did:hum:`, as the relay holds plots under it.
fn did(n: u8) -> String {
    plot_owner_id(&key(n))
}

/// The permit `giver` mints on `server` for `grantee` on `plot`, running out at `expiry`,
/// minted at `minted`, named `serial`.
fn permit_on(server: &str, giver: u8, plot: &str, grantee: u8, serial: &str, expiry: u64, minted: u64) -> Permit {
    let sig = build_plot_permit(&[giver; 32], server, plot, &did(grantee), serial, expiry, minted).expect("minted");
    Permit { issuer: key(giver), server: server.into(), plot: plot.into(), grantee: did(grantee), serial: serial.into(), expiry, sig }
}

/// [`permit_on`] this server, minted now, named [`SERIAL`].
fn permit(giver: u8, plot: &str, grantee: u8, expiry: u64) -> Permit {
    permit_on(SERVER, giver, plot, grantee, SERIAL, expiry, NOW)
}

/// A permit for the keeping rules, which never look at its signature.
fn unsigned(from: &str, plot: &str, serial: &str, expiry: u64, at: u64) -> HeldPermit {
    let permit = Permit {
        issuer: from.into(),
        server: SERVER.into(),
        plot: plot.into(),
        grantee: "did:hum:me".into(),
        serial: serial.into(),
        expiry,
        sig: "c2ln".into(),
    };
    HeldPermit { from: from.into(), permit, at }
}

/// THE PERMIT NOTE (plan section 3.1). Its text is exactly the marker `[[hum:permit:v1]]`
/// followed by the permit's JSON in the order its words are signed; it reads back as the same
/// permit, about 8.6 KB of it (padded to the 16,384 bucket). Each malformed one (not JSON,
/// not a permit, an issuer not hex or over the cap, a signature over the cap, an empty plot, a
/// line break in an id, a grantee over the cap, a serial in capitals, a body over the cap) is
/// read as no permit, yet is still a note, so it is never stored or shown as a message. A
/// text that does not START with the marker is not a note.
/// Seen red 2026-10-12 with the marker spelt `[[hum:permit]]` in dm_pq.rs: "the marker, then
/// the permit's JSON" failed on `"[[hum:permit]]{\"issuer\":\"da0280f47df7e04"`.
#[test]
fn the_permit_note_round_trips_and_a_bad_one_is_still_a_note() {
    let p = permit(1, "p1", 2, NOW + 30 * DAY);
    let text = note_text(&p);
    assert!(text.starts_with("[[hum:permit:v1]]{\"issuer\":\""), "the marker, then the permit's JSON: {:?}", &text[..40]);
    let order: Vec<usize> = ["\"issuer\":", "\"server\":", "\"plot\":", "\"grantee\":", "\"serial\":", "\"expiry\":", "\"sig\":"]
        .iter()
        .map(|k| text.find(k).unwrap_or_else(|| panic!("{k} in the note")))
        .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "the fields in the order their words are signed: {order:?}");
    assert!((8_000..NOTE_BODY_MAX_LEN).contains(&text.len()), "a real note is about 8.4 KB: {}", text.len());
    assert!(is_note(&text));
    assert_eq!(parse_note(&text), Some(p.clone()), "it reads back as the permit");

    let with = |f: &dyn Fn(&mut Permit)| {
        let mut q = p.clone();
        f(&mut q);
        note_text(&q)
    };
    let bad: Vec<(&str, String)> = vec![
        ("not JSON", format!("{CTL_PERMIT}{{not json")),
        ("JSON that is not a permit", format!("{CTL_PERMIT}{{\"issuer\":\"aa\"}}")),
        ("an issuer that is not hex", with(&|q| q.issuer = format!("zz{}", &q.issuer[2..]))),
        ("an issuer over the cap", with(&|q| q.issuer = "a".repeat(ISSUER_MAX_LEN + 1))),
        ("a signature over the cap", with(&|q| q.sig = "A".repeat(SIG_MAX_LEN + 1))),
        ("an empty plot", with(&|q| q.plot = String::new())),
        ("a line break in the plot", with(&|q| q.plot = "p1\np2".into())),
        ("a grantee over the cap", with(&|q| q.grantee = "d".repeat(PLOT_PERMIT_FIELD_MAX_LEN + 1))),
        ("a serial in capitals", with(&|q| q.serial = SERIAL.to_uppercase())),
        ("a body over the cap", with(&|q| {
            q.issuer = "a".repeat(ISSUER_MAX_LEN);
            q.sig = "A".repeat(SIG_MAX_LEN);
        })),
    ];
    for (what, text) in &bad {
        assert!(is_note(text), "{what}: still a note");
        assert_eq!(parse_note(text), None, "{what}: no permit");
    }
    for not in ["hello", " [[hum:permit:v1]]{}", "[[hum:choice:v1]]abc/invite", "[[hum:permit:v2]]{}"] {
        assert!(!is_note(not) && parse_note(not).is_none(), "{not:?} is not a permit note");
    }
}

/// WHO A PERMIT IS KEPT FROM (plan section 3.1). I am Bo; Ada gives me a 30-day permit for
/// her p1 on this server, and it is kept. Not kept, each for its own reason: the same permit
/// forwarded by Cy (`NotFromItsIssuer`), one Ada gave on another server (`OtherServer`), one
/// she gave Cy (`NotForMe`), one that ran out (`RanOut`), one signed for 92 days (`TooLong`),
/// one whose signature is not one and one whose serial is not the one signed
/// (`DoesNotCheckOut`), and any permit before this device knows the server's identity
/// (`NoServerYet`).
/// Seen red 2026-10-12 with the issuer-equals-sender line removed from `judge_arrival`:
/// "a permit Cy forwarded from Ada: Err(DoesNotCheckOut), expected Err(NotFromItsIssuer)" (the
/// signature, checked against the sender's key, still refused it).
#[test]
fn a_permit_is_kept_only_from_its_giver_for_me_on_this_server() {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    let (ada_key, me, cy_key) = (key(1), did(2), key(3));
    let (ada, cy) = (ada_key.as_str(), cy_key.as_str());
    let good = permit(1, "p1", 2, NOW + 30 * DAY);
    let too_long = {
        // Signed by hand, as a modified app could: the minting never makes one past 90 days.
        let mut p = good.clone();
        p.expiry = NOW + 92 * DAY;
        let signer = DilithiumKeypair::from_seed(&derive_dilithium_seed(&[1; 32]));
        p.sig = B64.encode(signer.sign(plot_permit_preimage(SERVER, "p1", &me, SERIAL, p.expiry).as_bytes()));
        p
    };
    let rows: Vec<(&str, Permit, &str, Option<&str>, Result<(), Arrival>)> = vec![
        ("Ada's permit, from Ada", good.clone(), ada, Some(SERVER), Ok(())),
        ("a permit Cy forwarded from Ada", good.clone(), cy, Some(SERVER), Err(Arrival::NotFromItsIssuer)),
        (
            "a permit Ada gave on another server",
            permit_on("did:hum:AnotherServer1111111", 1, "p1", 2, SERIAL, NOW + DAY, NOW),
            ada,
            Some(SERVER),
            Err(Arrival::OtherServer),
        ),
        ("a permit Ada gave Cy", permit(1, "p1", 3, NOW + DAY), ada, Some(SERVER), Err(Arrival::NotForMe)),
        ("a permit that ran out", permit_on(SERVER, 1, "p1", 2, SERIAL, NOW - DAY, NOW - 10 * DAY), ada, Some(SERVER), Err(Arrival::RanOut)),
        ("a permit signed for 92 days", too_long, ada, Some(SERVER), Err(Arrival::TooLong)),
        ("a permit whose signature is not one", Permit { sig: "bm90LWEtc2ln".into(), ..good.clone() }, ada, Some(SERVER), Err(Arrival::DoesNotCheckOut)),
        (
            "a permit whose serial is not the one signed",
            Permit { serial: "ffeeddccbbaa99887766554433221100".into(), ..good.clone() },
            ada,
            Some(SERVER),
            Err(Arrival::DoesNotCheckOut),
        ),
        ("Ada's permit before the server is known", good, ada, None, Err(Arrival::NoServerYet)),
    ];
    let mut wrong = Vec::new();
    for (what, p, sender, server, want) in &rows {
        let got = judge_arrival(p, sender, *server, &me, NOW);
        if got != *want {
            wrong.push(format!("{what}: {got:?}, expected {want:?}"));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// ONE PERMIT PER GIVER AND PLOT, THE ONE GIVEN LATER WINNING (plan section 3.2). Ada's
/// first permit for p1 is new; one she gave EARLIER, read late from the mailbox, changes
/// nothing even though it runs longer; her renewal replaces the first; the renewal again
/// changes nothing; her p2 and Cy's p1 each get a row of their own. Past 64 rows the
/// earliest-ended go.
/// Seen red 2026-10-12 with the comparison in `keep_held` reversed (`later(k, &h)`): "an
/// earlier permit read late" left `Replaced`, right `Unchanged`.
#[test]
fn a_later_permit_replaces_and_an_earlier_one_changes_nothing() {
    let mut held = Vec::new();
    let first = unsigned("ada", "p1", "11111111111111111111111111111111", NOW + 30 * DAY, 2_000);
    assert_eq!(keep_held(&mut held, first.clone()), Kept::New, "the first");
    let earlier = unsigned("ada", "p1", "22222222222222222222222222222222", NOW + 90 * DAY, 1_000);
    assert_eq!(keep_held(&mut held, earlier), Kept::Unchanged, "an earlier permit read late");
    assert_eq!(held, vec![first.clone()], "the first still stands");
    let renewal = unsigned("ada", "p1", "33333333333333333333333333333333", NOW + 60 * DAY, 3_000);
    assert_eq!(keep_held(&mut held, renewal.clone()), Kept::Replaced, "the renewal");
    assert_eq!(keep_held(&mut held, renewal.clone()), Kept::Unchanged, "the renewal again");
    assert_eq!(held, vec![renewal.clone()]);
    assert_eq!(keep_held(&mut held, unsigned("ada", "p2", SERIAL, NOW + DAY, 4_000)), Kept::New, "her other plot");
    assert_eq!(keep_held(&mut held, unsigned("cy", "p1", SERIAL, NOW + DAY, 4_000)), Kept::New, "Cy's p1");
    assert_eq!(held.len(), 3);

    let mut many = Vec::new();
    for i in 0..(HELD_MAX as u64 + 6) {
        keep_held(&mut many, unsigned(&format!("giver{i}"), "p1", SERIAL, NOW + (i + 1) * DAY, 1));
    }
    assert_eq!(many.len(), HELD_MAX);
    assert!(many.iter().all(|h| h.permit.expiry > NOW + 6 * DAY), "the earliest-ended went");
}

/// AN ENDED PERMIT STAYS LISTED FOR THE KEPT DAYS, THEN GOES (plan section 3.2), from both
/// lists: with 30 days kept, one that ended 31 days ago goes, one that ended 29 days ago
/// stays ("ran out on <date>"), and a standing one stays.
/// Seen red 2026-10-12 with `prune` skipping the given list: "given" left `["31", "29",
/// "live"]`, right `["29", "live"]`.
#[test]
fn ended_permits_leave_both_lists_after_the_kept_days() {
    let ends = [("31", NOW - 31 * DAY), ("29", NOW - 29 * DAY), ("live", NOW + DAY)];
    let mut held: Vec<HeldPermit> = ends.iter().map(|(from, e)| unsigned(from, "p1", SERIAL, *e, 1)).collect();
    let mut gave: Vec<GivenPermit> = ends.iter().map(|(to, e)| given(to, "p2", &format!("{:0>32}", to.len()), *e, GivenState::Taken)).collect();
    prune(&mut held, &mut gave, NOW, 30);
    assert_eq!(held.iter().map(|h| h.from.as_str()).collect::<Vec<_>>(), ["29", "live"], "held");
    assert_eq!(gave.iter().map(|g| g.to.as_str()).collect::<Vec<_>>(), ["29", "live"], "given");
}

/// WHAT THIS SESSION MAY USE (plan sections 3.2 and 3.7). Of the permits held, only one the
/// relay said works, for a plot of this ship that is not my own, and not run out, is usable:
/// here p1. Not p3 (still being checked), p4 (refused), p5 (ran out since the check), p2 (my
/// own plot) or p99 (not on this ship). And a given permit sent before a restart, whose send
/// is no longer in memory, reads as unconfirmed; one still on its way reads as on its way.
/// Seen red 2026-10-12 with the verdict filter removed from `usable_plots`: left `{"p1",
/// "p3", "p4", "p6"}`, right `{"p1"}`.
#[test]
fn only_a_working_permit_for_a_plot_of_this_ship_is_usable() {
    let mut hh = Household { my_plot: Some("p2".into()), ..Default::default() };
    let rows = [
        ("p1", NOW + DAY, Some(Verdict::Works)),
        ("p3", NOW + DAY, Some(Verdict::Checking)),
        ("p4", NOW + DAY, Some(Verdict::Refused(Why::PermitNotFromHolder))),
        ("p5", NOW - 1, Some(Verdict::Works)),
        ("p2", NOW + DAY, Some(Verdict::Works)),
        ("p99", NOW + DAY, Some(Verdict::Works)),
        ("p6", NOW + DAY, None),
    ];
    let held: Vec<HeldPermit> = rows.iter().map(|(plot, expiry, _)| unsigned("ada", plot, SERIAL, *expiry, 1)).collect();
    for (h, (_, _, v)) in held.iter().zip(rows.iter()) {
        if let Some(v) = v {
            hh.verdicts.insert(PermitKey::of(h), Judged { verdict: *v, at: 0.0 });
        }
    }
    let on_ship = |p: &str| p != "p99";
    let usable = hh.usable_plots(&held, on_ship, NOW);
    assert_eq!(usable, BTreeSet::from(["p1".to_string()]));

    let gone = given("bo", "p2", SERIAL, NOW + DAY, GivenState::OnItsWay { reference: "before-restart".into() });
    assert_eq!(hh.given_state(&gone), GivenState::Unconfirmed, "its send is not in memory any more");
    hh.on_their_way.push(PermitPut {
        reference: "now".into(),
        peer: "bo".into(),
        plot: "p2".into(),
        serial: SERIAL.into(),
        expiry: NOW + DAY,
        days: 1,
        self_copy: serde_json::Value::Null,
        at: std::time::Instant::now(),
    });
    let sent = given("bo", "p2", SERIAL, NOW + DAY, GivenState::OnItsWay { reference: "now".into() });
    assert_eq!(hh.given_state(&sent), sent.state, "still on its way");
}
