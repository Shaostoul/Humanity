//! Tests of the relay's building rules (shared_build.rs) on a game world and a database of their
//! own, with no sockets: who may build and take down what, when a frame comes into view and
//! leaves it, the caps, one piece to a box, and the exact pose kept. The same rules on a real
//! relay, over its socket, are relay/features.rs's `shared_build` tests. Each was seen to fail
//! before it passed; the failure text is in its comment.

use super::*;
use crate::relay::core::pq_crypto::{build_plot_permit, derive_dilithium_seed, plot_permit_preimage, DilithiumKeypair};
use crate::systems::construction::placement::quarter_turn;

/// "Now" for these tests, Unix seconds (any time works: permits are judged against it).
const NOW: u64 = 1_760_000_000;
const DAY: u64 = 86_400;

/// The relay key (Dilithium3 public key, hex) of the player whose seed is `[n; 32]`.
fn key(n: u8) -> String {
    hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&[n; 32])).public_key())
}

/// The shipped ship, its pieces read from a fresh database, and in it the players `(seed, plot)`:
/// each with a plot holds it in the database and on their entity, as a join leaves them; one
/// with none is a guest.
fn ship(tag: &str, players: &[(u8, Option<&str>)]) -> (GameWorld, Storage) {
    let db = Storage::open_temp(&format!("sb_{tag}"));
    let mut world = GameWorld::new();
    load_pieces(&mut world, &db);
    assert!(!world.pieces.server_did.is_empty(), "the relay knows its own did:hum");
    for (n, plot) in players {
        let k = key(*n);
        let id = world.spawn_player(&k, [80.0, 1.7, 40.0]);
        let arrival = plot.and_then(|p| world.ship_plots.plot(p).cloned());
        world.set_home_plot(id, arrival.as_ref());
        if let Some(p) = plot {
            let held = db.claim_plot(&world.ship_plots.ship_id, &plot_owner_id(&k), &[p]).expect("a claim");
            assert_eq!(held.as_deref(), Some(*p));
        }
    }
    (world, db)
}

/// The player with seed `n`, as the rules see them, with `ranks`.
fn who(world: &GameWorld, n: u8, ranks: Ranks) -> Asker {
    Asker::in_world(world, &key(n), ranks).expect("in the world")
}

/// The serial these tests' permits are named by, unless a test names another.
const SERIAL: &str = "00112233445566778899aabbccddeeff";

/// A household permit the holder with seed `issuer` signs, on this relay, for the player with
/// seed `grantee` on `plot`, running out at `expiry`, minted at `minted` (the minting refuses an
/// end date already past at that time), named [`SERIAL`].
fn permit(world: &GameWorld, issuer: u8, plot: &str, grantee: u8, expiry: u64, minted: u64) -> Permit {
    permit_named(world, issuer, plot, grantee, SERIAL, expiry, minted)
}

/// [`permit`], named `serial`.
fn permit_named(world: &GameWorld, issuer: u8, plot: &str, grantee: u8, serial: &str, expiry: u64, minted: u64) -> Permit {
    let server = world.pieces.server_did.clone();
    let grantee = plot_owner_id(&key(grantee));
    let sig = build_plot_permit(&[issuer; 32], &server, plot, &grantee, serial, expiry, minted).expect("minted");
    Permit { issuer: key(issuer), server, plot: plot.into(), grantee, serial: serial.into(), expiry, sig }
}

/// A piece of `blueprint` in `frame` at `at` (frame metres), built by `owner`, for the book alone.
fn stored(id: u64, frame: &str, blueprint: &str, at: [f32; 3], owner: &str) -> StoredPiece {
    StoredPiece {
        piece_id: id,
        world_id: "mothership-1".into(),
        frame: frame.into(),
        blueprint_id: blueprint.into(),
        position: at,
        rotation: [0.0, 0.0, 0.0, 1.0],
        scale: [4.0, 0.2, 4.0],
        owner_did: owner.into(),
        placed_at_ms: 0,
    }
}

/// Put `p` straight into the book, as the database would have brought it at a start.
fn put(world: &mut GameWorld, p: StoredPiece) {
    world.pieces.by_frame.entry(p.frame.clone()).or_default().pieces.insert(p.piece_id, p);
}

/// A build of `blueprint` in `frame` at `(x, y, z)` (frame metres) with `turns` quarter turns,
/// sized as basic.ron says, as the game sends it.
fn ask(world: &GameWorld, frame: &str, blueprint: &str, at: [f32; 3], turns: u8) -> BuildAsk {
    let size = world.pieces.blueprints.get(blueprint).expect("a shipped blueprint").size;
    BuildAsk {
        frame: frame.into(),
        blueprint_id: blueprint.into(),
        local: Transform { position: Vec3::from_array(at), rotation: quarter_turn(turns), scale: Vec3::from_array(size) },
        permit: None,
    }
}

/// Judge `a` for `who` and, when it passes, keep it: what a `game_build` does.
fn build(world: &mut GameWorld, db: &Storage, who: &Asker, a: &BuildAsk) -> Result<(StoredPiece, u64), Refusal> {
    judge_build(world, db, who, a, 1_000, NOW).and_then(|acc| keep(world, db, acc)).map_err(|r| r.refusal)
}

/// WHO MAY BUILD WHERE (the operator, 2026-10-03 and 2026-10-05; shared_build.rs `may_build`),
/// row by row. Ann holds p1, Bo p2, Cy p3, Ray (who has the ship-editing rank) p4; Gil is a
/// guest. Ann builds on her own plot; Bo and Gil on hers only with a household permit she
/// signed for them on THIS relay, and the permit is judged on the relay's own facts: Ann's p1
/// permit does nothing on Cy's p3, Bo's permit does nothing for Cy, one signed for another
/// server or one whose signature is not one is a bad permit, one run out is expired, one running
/// past 90 days is too long, and one Cy signed for p1 is not from its holder. The ship's shared
/// spaces take the rank, whatever permit comes with the build, and the rank gives no plot.
///
/// Seen red 2026-10-05 with the holder check removed from `permit_lets_in` (any valid signature
/// let its grantee in): "Bo with a permit Cy signed for Ann's p1: Ok(()), expected
/// Err(PermitNotFromHolder)".
#[test]
fn who_may_build_where() {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    let (world, db) = ship("may_build", &[(1, Some("p1")), (2, Some("p2")), (3, Some("p3")), (5, Some("p4")), (4, None)]);
    let (plain, rank) = (Ranks::default(), Ranks { can_edit_ship: true, take_down_any: false });
    let good = permit(&world, 1, "p1", 2, NOW + 30 * DAY, NOW);
    let other_server = {
        let mut p = permit(&world, 1, "p1", 2, NOW + 30 * DAY, NOW);
        let elsewhere = "did:hum:SomeOtherServer11111";
        p.sig = build_plot_permit(&[1; 32], elsewhere, "p1", &p.grantee, &p.serial, p.expiry, NOW).unwrap();
        p.server = elsewhere.into();
        p
    };
    let too_long = {
        // Signed by hand, as a modified game could: the minting never makes one past 90 days.
        let mut p = good.clone();
        p.expiry = NOW + 92 * DAY;
        let issuer = DilithiumKeypair::from_seed(&derive_dilithium_seed(&[1; 32]));
        p.sig = B64.encode(issuer.sign(plot_permit_preimage(&world.pieces.server_did, "p1", &p.grantee, &p.serial, p.expiry).as_bytes()));
        p
    };
    let not_a_signature = Permit { sig: "bm90LWEtc2ln".into(), ..good.clone() };
    let rows: Vec<(&str, Asker, &str, Option<Permit>, Option<Why>)> = vec![
        ("Ann on her own plot", who(&world, 1, plain), "plot:p1", None, None),
        ("Bo on Ann's plot", who(&world, 2, plain), "plot:p1", None, Some(Why::NotYourPlot)),
        ("Gil the guest on Ann's plot", who(&world, 4, plain), "plot:p1", None, Some(Why::Guest)),
        ("Bo with Ann's permit", who(&world, 2, plain), "plot:p1", Some(good.clone()), None),
        ("Gil with a permit from Ann", who(&world, 4, plain), "plot:p1", Some(permit(&world, 1, "p1", 4, NOW + DAY, NOW)), None),
        ("Bo with Ann's p1 permit on Cy's p3", who(&world, 2, plain), "plot:p3", Some(good.clone()), Some(Why::PermitBad)),
        ("Cy with the permit Ann gave Bo", who(&world, 3, plain), "plot:p1", Some(good.clone()), Some(Why::PermitBad)),
        ("Bo with a permit given on another server", who(&world, 2, plain), "plot:p1", Some(other_server), Some(Why::PermitBad)),
        ("Bo with a permit whose signature is not one", who(&world, 2, plain), "plot:p1", Some(not_a_signature), Some(Why::PermitBad)),
        ("Bo with a permit that has run out", who(&world, 2, plain), "plot:p1", Some(permit(&world, 1, "p1", 2, NOW - DAY, NOW - 10 * DAY)), Some(Why::PermitExpired)),
        ("Bo with a permit running 92 days", who(&world, 2, plain), "plot:p1", Some(too_long), Some(Why::PermitTooLong)),
        ("Bo with a permit Cy signed for Ann's p1", who(&world, 2, plain), "plot:p1", Some(permit(&world, 3, "p1", 2, NOW + DAY, NOW)), Some(Why::PermitNotFromHolder)),
        ("Ann in the Commons", who(&world, 1, plain), "zone:commons", None, Some(Why::ShipRank)),
        ("Bo in the Commons with Ann's permit", who(&world, 2, plain), "zone:commons", Some(good), Some(Why::ShipRank)),
        ("Ray, with the rank, in the Commons", who(&world, 5, rank), "zone:commons", None, None),
        ("Ray, with the rank, on First Street", who(&world, 5, rank), "zone:street-1", None, None),
        ("Ray, with the rank, on Ann's plot", who(&world, 5, rank), "plot:p1", None, Some(Why::NotYourPlot)),
    ];
    let mut wrong = Vec::new();
    for (what, asker, frame, permit, want) in &rows {
        let f = world.pieces.frames.get(frame).expect("a frame of the ship");
        let got = may_build(&world.pieces, &db, &world.ship_plots.ship_id, f, asker, permit.as_ref(), NOW);
        let want_r = match want {
            None => Ok(()),
            Some(w) => Err(*w),
        };
        let got_r = got.map_err(|r| r.why.unwrap_or(Why::Unknown));
        if got_r != want_r {
            wrong.push(format!("{what}: {got_r:?}, expected {want_r:?}"));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// WHO MAY TAKE DOWN WHAT (shared_build.rs `may_remove`), row by row. On Ann's p1 stand her own
/// foundation and a wall Bo put up with her permit; Ray put a wall up in the Commons. Ann takes
/// down anything on her plot, Bo's wall too; Bo, with his permit, only his own wall (Ann's
/// foundation is not his piece), and without it nothing; with a permit run out, nothing; Cy, a
/// visitor, and Gil, a guest, nothing. Ada, an admin, takes down anything anywhere. In the
/// Commons whoever has the rank takes down any piece, and Ann, without it, none.
///
/// Seen red 2026-10-05 with the builder check removed from `may_remove` (a permit let its holder
/// take down anything on the plot): "Bo with Ann's permit, Ann's foundation: Ok(()), expected
/// Err(NotYourPiece)".
#[test]
fn who_may_take_down_what() {
    let (mut world, db) =
        ship("may_remove", &[(1, Some("p1")), (2, Some("p2")), (3, Some("p3")), (5, Some("p4")), (6, Some("p5")), (4, None)]);
    let did = |n: u8| plot_owner_id(&key(n));
    let anns = stored(1, "plot:p1", "wood_foundation", [20.0, 0.0, 20.0], &did(1));
    let bos = stored(2, "plot:p1", "wood_foundation", [30.0, 0.0, 20.0], &did(2));
    let rays = stored(3, "zone:commons", "wood_foundation", [10.0, 0.0, 10.0], &did(5));
    for p in [&anns, &bos, &rays] {
        put(&mut world, p.clone());
    }
    let plain = Ranks::default();
    let rank = Ranks { can_edit_ship: true, take_down_any: false };
    let admin = Ranks { can_edit_ship: true, take_down_any: true };
    let good = permit(&world, 1, "p1", 2, NOW + 30 * DAY, NOW);
    let run_out = permit(&world, 1, "p1", 2, NOW - DAY, NOW - 10 * DAY);
    let rows: Vec<(&str, Asker, &StoredPiece, Option<Permit>, Option<Why>)> = vec![
        ("Ann, her own foundation", who(&world, 1, plain), &anns, None, None),
        ("Ann, Bo's wall on her plot", who(&world, 1, plain), &bos, None, None),
        ("Bo with Ann's permit, his own wall", who(&world, 2, plain), &bos, Some(good.clone()), None),
        ("Bo with Ann's permit, Ann's foundation", who(&world, 2, plain), &anns, Some(good.clone()), Some(Why::NotYourPiece)),
        ("Bo without the permit, his own wall", who(&world, 2, plain), &bos, None, Some(Why::NotYourPlot)),
        ("Bo with a permit that has run out, his own wall", who(&world, 2, plain), &bos, Some(run_out), Some(Why::PermitExpired)),
        ("Cy, a visitor, Ann's foundation", who(&world, 3, plain), &anns, None, Some(Why::NotYourPlot)),
        ("Gil, a guest, Ann's foundation", who(&world, 4, plain), &anns, None, Some(Why::NotYourPlot)),
        ("Ada, an admin, Ann's foundation", who(&world, 6, admin), &anns, None, None),
        ("Ada, an admin, Ray's piece in the Commons", who(&world, 6, admin), &rays, None, None),
        ("Ray, with the rank, his own piece in the Commons", who(&world, 5, rank), &rays, None, None),
        ("Bo, given the rank, Ray's piece in the Commons", who(&world, 2, rank), &rays, None, None),
        ("Ann, without the rank, Ray's piece in the Commons", who(&world, 1, plain), &rays, None, Some(Why::ShipRank)),
    ];
    let mut wrong = Vec::new();
    for (what, asker, piece, permit, want) in &rows {
        let f = world.pieces.frames.get(&piece.frame).expect("a frame of the ship");
        let got = may_remove(&world.pieces, &db, &world.ship_plots.ship_id, f, piece, asker, permit.as_ref(), NOW);
        let want_r = match want {
            None => Ok(()),
            Some(w) => Err(*w),
        };
        let got_r = got.map_err(|r| r.why.unwrap_or(Why::Unknown));
        if got_r != want_r {
            wrong.push(format!("{what}: {got_r:?}, expected {want_r:?}"));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// A PERMIT ITS HOLDER TOOK BACK LETS NOBODY IN (ship homes 5b, amendment A1). Ann holds p1 and
/// gives Bo a permit named `SERIAL`; Bo's foundation stands there. Before anything is taken back
/// Bo builds, and takes down his own foundation, with it. Cy "withdrawing" that serial on her own
/// account changes nothing (the list is keyed by the issuer, as for friendship passes). Once Ann
/// takes it back (`cert_revoke`, the withdrawal list friendship passes use) Bo can neither build
/// nor take down his own foundation with it, each refused `permit_withdrawn`; a second permit Ann
/// gives him under another serial still works (a withdrawal names one permit); Cy showing the
/// withdrawn permit is told only `permit_bad` (the signature over the asker's own id is checked
/// before the list is read, so nothing is said about someone else's permit); and Ann on her own
/// plot needs no permit at all.
///
/// Seen red 2026-10-12 with the withdrawal lookup skipped in `permit_lets_in`: "Bo builds with
/// the permit Ann took back: Ok(()), expected Err(PermitWithdrawn)" and "Bo takes down his own
/// foundation with the permit Ann took back: Ok(()), expected Err(PermitWithdrawn)".
#[test]
fn a_withdrawn_permit_no_longer_lets_its_holder_in() {
    let (mut world, db) = ship("withdrawn", &[(1, Some("p1")), (2, Some("p2")), (3, Some("p3"))]);
    let bos_piece = stored(1, "plot:p1", "wood_foundation", [30.0, 0.0, 20.0], &plot_owner_id(&key(2)));
    put(&mut world, bos_piece.clone());
    let plain = Ranks::default();
    let p1 = world.pieces.frames.get("plot:p1").expect("p1");
    let given = permit(&world, 1, "p1", 2, NOW + 30 * DAY, NOW);
    let second = permit_named(&world, 1, "p1", 2, "ffeeddccbbaa99887766554433221100", NOW + 30 * DAY, NOW);
    let ship_id = world.ship_plots.ship_id.clone();
    let judge = |who_n: u8, permit: Option<&Permit>| {
        let asker = who(&world, who_n, plain);
        let build = may_build(&world.pieces, &db, &ship_id, &p1, &asker, permit, NOW).map_err(|r| r.why.unwrap_or(Why::Unknown));
        let remove = may_remove(&world.pieces, &db, &ship_id, &p1, &bos_piece, &asker, permit, NOW).map_err(|r| r.why.unwrap_or(Why::Unknown));
        (build, remove)
    };
    let today = (NOW / DAY) as i64;
    assert_eq!(judge(2, Some(&given)), (Ok(()), Ok(())), "before it is taken back, Bo builds and takes down his own foundation");

    db.withdraw_friend_cert(&key(3), SERIAL, today).expect("Cy's withdrawal is kept");
    assert_eq!(judge(2, Some(&given)), (Ok(()), Ok(())), "Cy cannot take back a permit Ann gave");

    db.withdraw_friend_cert(&key(1), SERIAL, today).expect("Ann's withdrawal is kept");
    let mut wrong = Vec::new();
    let (build, remove) = judge(2, Some(&given));
    let rows: Vec<(&str, Result<(), Why>, Result<(), Why>)> = vec![
        ("Bo builds with the permit Ann took back", build, Err(Why::PermitWithdrawn)),
        ("Bo takes down his own foundation with the permit Ann took back", remove, Err(Why::PermitWithdrawn)),
        ("Bo builds with the second permit Ann gave him", judge(2, Some(&second)).0, Ok(())),
        ("Cy shows the permit Ann gave Bo and took back", judge(3, Some(&given)).0, Err(Why::PermitBad)),
        ("Ann on her own plot", judge(1, None).0, Ok(())),
    ];
    for (what, got, want) in rows {
        if got != want {
            wrong.push(format!("{what}: {got:?}, expected {want:?}"));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// A PERMIT CHECK ANSWERS AS A BUILD WOULD (ship homes 5b, plan section 3.5: `judge_permit_check`
/// and `permit_status`, the answer to `game_permit_check`). Ann holds p1, Bo p2, Cy p3 and Eve
/// p5; Gil is a guest. Row by row, the plot each permit names is judged for the player showing
/// it, and the same permit is carried by a wall built on that plot (`judge_build`, the whole
/// build path): the check must say what the build is told, and what the row expects.
/// - Ann's 30-day permit for Bo works, and so does hers for Gil the guest.
/// - Run out: `permit_expired`. Signed by hand to run 92 days: `permit_too_long`.
/// - Eve's permit for p5, after Eve gave p5 back: `permit_not_from_holder`.
/// - Ann's permit for Cy, shown by Bo (another grantee); one she signed for another server; her
///   p1 permit with its plot changed to p3 (another plot): each `permit_bad`, as the signature
///   is over the relay's own facts.
/// - Bo's own plot: works whatever the permit says (here Ann's p1 permit renamed p2, whose
///   signature is no good), as a build on his own plot needs none.
/// - A permit Ann took back (`cert_revoke`'s list): `permit_withdrawn`.
/// - A plot this ship does not have (p99): `permit_bad` (a build there is `bad_frame`).
///
/// The answers as sent: a permit that works is `ok` alone; one that does not carries its why
/// and that why's words, the same a build would; one the relay could not judge (too soon, not
/// in the world, the database not answering) is `again` with neither; and no answer names a key
/// or a `did:hum`.
///
/// Seen red 2026-10-12 with `permit_lets_in` returning Ok at its top: "Bo, a permit that ran out:
/// check Ok(()), build Ok(()), expected Err(PermitExpired)" failed, and the same for the too
/// long, given back, another grantee, another server, another plot and taken back rows (the
/// taken-back row is the check's `permit_withdrawn`). And with the permit branch taken out of
/// `may_build` (a build on another's plot refused as if no permit came with it), the check left
/// as it was, so only the comparison with the build could see it: "Bo, Ann's permit: check
/// Ok(()), build Err(NotYourPlot), expected Ok(())" failed.
#[test]
fn a_permit_check_answers_as_a_build_would() {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    let (world, db) = ship("permit_check", &[(1, Some("p1")), (2, Some("p2")), (3, Some("p3")), (6, Some("p5")), (4, None)]);
    let ship_id = world.ship_plots.ship_id.clone();
    let plain = Ranks::default();
    let good = permit(&world, 1, "p1", 2, NOW + 30 * DAY, NOW);
    let too_long = {
        // Signed by hand, as a modified game could: the minting never makes one past 90 days.
        let mut p = good.clone();
        p.expiry = NOW + 92 * DAY;
        let issuer = DilithiumKeypair::from_seed(&derive_dilithium_seed(&[1; 32]));
        p.sig = B64.encode(issuer.sign(plot_permit_preimage(&world.pieces.server_did, "p1", &p.grantee, &p.serial, p.expiry).as_bytes()));
        p
    };
    let other_server = {
        let mut p = good.clone();
        p.server = "did:hum:SomeOtherServer11111".into();
        p.sig = build_plot_permit(&[1; 32], &p.server, "p1", &p.grantee, &p.serial, p.expiry, NOW).unwrap();
        p
    };
    let renamed = |plot: &str| Permit { plot: plot.into(), ..good.clone() };
    let taken_back = permit_named(&world, 1, "p1", 2, "ffeeddccbbaa99887766554433221100", NOW + 30 * DAY, NOW);
    db.withdraw_friend_cert(&key(1), &taken_back.serial, (NOW / DAY) as i64).expect("Ann's withdrawal is kept");
    let eves = permit(&world, 6, "p5", 2, NOW + 30 * DAY, NOW);
    assert!(db.release_plot_by_id(&ship_id, "p5").unwrap(), "Eve gives p5 back");

    let rows: Vec<(&str, u8, Permit, Result<(), Why>)> = vec![
        ("Bo, Ann's permit", 2, good.clone(), Ok(())),
        ("Gil the guest, Ann's permit for him", 4, permit(&world, 1, "p1", 4, NOW + DAY, NOW), Ok(())),
        ("Bo, a permit that ran out", 2, permit(&world, 1, "p1", 2, NOW - DAY, NOW - 10 * DAY), Err(Why::PermitExpired)),
        ("Bo, a permit running 92 days", 2, too_long, Err(Why::PermitTooLong)),
        ("Bo, Eve's permit for the p5 she gave back", 2, eves, Err(Why::PermitNotFromHolder)),
        ("Bo, Ann's permit for Cy", 2, permit(&world, 1, "p1", 3, NOW + DAY, NOW), Err(Why::PermitBad)),
        ("Bo, Ann's permit given on another server", 2, other_server, Err(Why::PermitBad)),
        ("Bo, Ann's p1 permit renamed p3", 2, renamed("p3"), Err(Why::PermitBad)),
        ("Bo, his own plot", 2, renamed("p2"), Ok(())),
        ("Bo, the permit Ann took back", 2, taken_back, Err(Why::PermitWithdrawn)),
    ];
    let mut wrong = Vec::new();
    for (what, n, p, want) in &rows {
        let asker = who(&world, *n, plain);
        let check = judge_permit_check(&world.pieces, &db, &ship_id, &asker, p, NOW);
        let mut wall = ask(&world, &format!("plot:{}", p.plot), "wood_wall", [20.0, 0.2, 18.0], 0);
        wall.permit = Some(p.clone());
        let build = judge_build(&world, &db, &asker, &wall, 1_000, NOW).map(|_| ()).map_err(|r| r.refusal.why.unwrap_or(Why::Unknown));
        let check_r = check.clone().map_err(|r| r.why.unwrap_or(Why::Unknown));
        if check_r != *want || build != *want {
            wrong.push(format!("{what}: check {check_r:?}, build {build:?}, expected {want:?}"));
        }
        // The answer as sent.
        let text = serde_json::to_string(&permit_status(7, &p.plot, &check)).unwrap();
        let sent: serde_json::Value = serde_json::from_str(&text).unwrap();
        let (ok, why) = (sent["ok"].as_bool(), sent["why"].as_str());
        let want_sent = match want {
            Ok(()) => (Some(true), None),
            Err(w) => (Some(false), Some(w.as_str())),
        };
        let words = want.err().map(|w| shared::why_words(Action::Build, Reason::NotAllowed, Some(w)));
        if (ok, why) != want_sent || sent["message"].as_str().map(str::to_string) != words || sent.get("again").is_some() {
            wrong.push(format!("{what}: sent {text}"));
        }
        if sent["req_id"] != 7 || sent["plot"] != p.plot.as_str() || text.contains("did:hum") || text.contains(&key(1)) || text.contains(&key(2)) {
            wrong.push(format!("{what}: the answer says more than its own: {text}"));
        }
    }
    // A plot this ship does not have: a bad permit to the check (a build there is a bad frame).
    let nowhere = judge_permit_check(&world.pieces, &db, &ship_id, &who(&world, 2, plain), &renamed("p99"), NOW);
    if nowhere.as_ref().err().and_then(|r| r.why) != Some(Why::PermitBad) {
        wrong.push(format!("Bo, a permit for p99: {nowhere:?}, expected PermitBad"));
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));

    // Not judged this time: ask again, with no why and no words.
    for (what, r) in [
        ("too soon", Refusal::new(Reason::RateLimited, "")),
        ("not in the world", Refusal::new(Reason::NotInGame, "")),
        ("the database not answering", Refusal::new(Reason::StorageError, "who holds p1 could not be read")),
    ] {
        let text = serde_json::to_string(&permit_status(8, "p1", &Err(r))).unwrap();
        assert_eq!(text, r#"{"type":"game_permit_status","req_id":8,"plot":"p1","ok":false,"again":true}"#, "{what}");
    }
}

/// A FRAME COMES INTO VIEW AT 250 M AND LEAVES ONLY PAST 300 M (increment 4's distances,
/// data/ship/shared_world.ron). At p1's door the frames in view are the Commons, First Street and
/// p1 to p3, and p4 (256.5 m) is not. Walking north along x 53.5, over p4's floor: at 260 m
/// from it p4 is still out, at 249 m it comes in, back at 290 m it stays in (the gap that keeps
/// someone at the edge from flickering), and at 301 m it leaves; nothing else changes on the way.
///
/// Seen red 2026-10-05 with the gap ignored (a frame left past 250 m, `d > in_m`): "290 m keeps
/// p4 in view / left: [\"plot:p4\"] / right: []".
#[test]
fn a_frame_comes_into_view_at_250_m_and_leaves_past_300() {
    let (mut world, _db) = ship("view", &[(1, Some("p1"))]);
    let id = world.find_player_entity(&key(1)).unwrap();
    let (in_m, out_m) = (world.rules.delivery.in_view_m, world.rules.delivery.out_of_view_m);
    assert_eq!((in_m, out_m), (250.0, 300.0), "the shipped view");
    let book = &mut world.pieces;
    book.views.insert(id, Viewer::new(&key(1)));
    let mut at = |z: f32| book.rejudge(id, Vec3::new(53.5, 1.7, z), in_m, out_m);
    let (came, went) = at(40.5);
    assert_eq!(came, ["zone:commons", "zone:street-1", "plot:p1", "plot:p2", "plot:p3"], "the frames in view at p1's door");
    assert!(went.is_empty());
    // p4's floor starts at z 297, so standing over its x the distance is 297 - z.
    assert_eq!(at(37.0), (vec![], vec![]), "260 m: p4 not yet in view");
    assert_eq!(at(48.0), (vec!["plot:p4".to_string()], vec![]), "249 m: p4 comes into view");
    let (came, went) = at(7.0);
    assert!(came.is_empty());
    assert_eq!(went, Vec::<String>::new(), "290 m keeps p4 in view / left: {went:?} / right: []");
    assert_eq!(at(-4.0), (vec![], vec!["plot:p4".to_string()]), "301 m: p4 leaves the view");
    assert_eq!(world.pieces.frames_in_view(id), ["plot:p1", "plot:p2", "plot:p3", "zone:commons", "zone:street-1"]);
}

/// THE CAPS HOLD (shared.rs: 512 pieces to a frame, 512 to a builder, 4,096 to a ship). Ann
/// builds a foundation on her own p1 each time: refused `frame_full` while p1 holds 512 pieces
/// (a permit holder's, say); `owner_full` while she keeps 512 elsewhere (in the Commons and on
/// p2); `world_full` while the ship holds 4,096 spread over its other frames; and built when the
/// ship holds one fewer.
///
/// Seen red 2026-10-05 with the frame cap's check removed from `judge_build`: "p1 holds 512
/// pieces: Ok(()), expected frame_full".
#[test]
fn the_caps_hold() {
    let (mut world, db) = ship("caps", &[(1, Some("p1"))]);
    let ann = who(&world, 1, Ranks::default());
    let spot = ask(&world, "plot:p1", "wood_foundation", [20.0, 0.0, 20.0], 0);
    let fill = |world: &mut GameWorld, frame: &str, n: usize, owner: &str, first: u64| {
        for i in 0..n as u64 {
            // Far from the spot (and from each other), so only the count matters.
            put(world, stored(first + i, frame, "wood_foundation", [1000.0 + i as f32 * 5.0, 0.0, 0.0], owner));
        }
    };
    let reason = |world: &mut GameWorld| build(world, &db, &ann, &spot).map(|_| ()).map_err(|r| r.reason);

    fill(&mut world, "plot:p1", shared::MAX_PIECES_PER_FRAME, "did:hum:SomeoneElse", 1);
    let got = reason(&mut world);
    assert_eq!(got, Err(Reason::FrameFull), "p1 holds 512 pieces: {got:?}, expected frame_full");
    world.pieces.by_frame.clear();

    let anns = plot_owner_id(&key(1));
    fill(&mut world, "zone:commons", 300, &anns, 1);
    fill(&mut world, "plot:p2", shared::MAX_PIECES_PER_OWNER - 300, &anns, 1000);
    assert_eq!(reason(&mut world), Err(Reason::OwnerFull), "Ann keeps 512 pieces elsewhere");
    world.pieces.by_frame.clear();

    let others: Vec<String> = world.pieces.frames.frames.iter().map(|f| f.id.clone()).filter(|f| f != "plot:p1").collect();
    let each = shared::MAX_PIECES_PER_WORLD / others.len() + 1;
    for (k, f) in others.iter().enumerate() {
        fill(&mut world, f, each, "did:hum:SomeoneElse", 10_000 * (k as u64 + 1));
    }
    assert!(world.pieces.count_all() >= shared::MAX_PIECES_PER_WORLD);
    assert_eq!(reason(&mut world), Err(Reason::WorldFull), "the ship holds 4,096 pieces");
    // One fewer than the cap: built.
    while world.pieces.count_all() >= shared::MAX_PIECES_PER_WORLD {
        let f = world.pieces.by_frame.values_mut().find(|f| !f.pieces.is_empty()).unwrap();
        let id = *f.pieces.keys().next().unwrap();
        f.pieces.remove(&id);
    }
    assert_eq!(reason(&mut world), Ok(()), "one under the cap");
}

/// ONE PIECE TO A BOX (shared.rs `same_box`, the rule the player's own home keeps with
/// `placement::occupied`). A foundation on p1, then the same foundation again: `occupied`, and
/// nothing kept. A wall on it is another box, and so is that wall turned a quarter; turned a
/// half it covers the first wall's box again: `occupied`. A foundation a metre over is another
/// box. Four pieces, four rows.
///
/// Seen red 2026-10-05 with the same-box check removed from `judge_build`: "the same foundation
/// again: Ok(()), expected occupied".
#[test]
fn one_piece_to_a_box() {
    let (mut world, db) = ship("occupied", &[(1, Some("p1"))]);
    let ann = who(&world, 1, Ranks::default());
    let foundation = ask(&world, "plot:p1", "wood_foundation", [20.0, 0.0, 20.0], 0);
    assert!(build(&mut world, &db, &ann, &foundation).is_ok());
    let again = build(&mut world, &db, &ann, &foundation).map(|_| ()).map_err(|r| r.reason);
    assert_eq!(again, Err(Reason::Occupied), "the same foundation again: {again:?}, expected occupied");
    let wall = |turns: u8| ask(&world, "plot:p1", "wood_wall", [20.0, 0.2, 18.0], turns);
    let (wall0, wall1, wall2) = (wall(0), wall(1), wall(2));
    let over = ask(&world, "plot:p1", "wood_foundation", [21.0, 0.0, 20.0], 0);
    assert!(build(&mut world, &db, &ann, &wall0).is_ok(), "a wall on it");
    assert!(build(&mut world, &db, &ann, &wall1).is_ok(), "that wall turned a quarter");
    let half = build(&mut world, &db, &ann, &wall2).map(|_| ()).map_err(|r| r.reason);
    assert_eq!(half, Err(Reason::Occupied), "turned a half, the first wall's box again");
    assert!(build(&mut world, &db, &ann, &over).is_ok(), "a metre over");
    assert_eq!(world.pieces.pieces_in("plot:p1").len(), 4);
    assert_eq!(db.load_world_pieces(&world.ship_plots.ship_id).unwrap().len(), 4, "and four rows");
}

/// THE CHECK GOES OUT WITH THE TIME SYNC (the review of increment 5, findings 1 and 5): the loop in
/// relay/mod.rs that sends `game_time_sync` every `shared::CHECK_INTERVAL_S` seconds also sends
/// every player their check (`send_checks`). The end-to-end test calls `send_checks` itself, so
/// without this a relay that never sent a check would pass it, and every game's lost messages
/// would go unnoticed again.
///
/// Seen red 2026-10-05 with the `send_checks` line taken out of the loop: "the time sync loop
/// never sends the checks".
#[test]
fn the_check_goes_out_with_the_time_sync() {
    let src = include_str!("../mod.rs").replace("\r\n", "\n");
    let start = src.find("// Game TimeSync broadcast").expect("the time sync loop in relay/mod.rs");
    let end = src[start..].find("// Scheduled re-votes").map_or(src.len(), |e| start + e);
    let body = &src[start..end];
    let every = format!("Duration::from_secs({})", shared::CHECK_INTERVAL_S);
    assert!(body.contains(&every), "the time sync loop no longer runs every {} s: {body}", shared::CHECK_INTERVAL_S);
    assert!(body.contains("shared_build::send_checks(&game_state).await"), "the time sync loop never sends the checks");
}

/// THE POSE KEPT IS THE EXACT ONE (shared.rs `pose_in_frame`): a wall sent a little off (x 0.4 mm
/// and z 0.3 mm off the grid, turned 0.2 degrees off a quarter about a tilted axis, all inside
/// the tolerances) is kept, in the relay's book and in its row, exactly on the grid, exactly the
/// quarter turn the game's placement makes and exactly its blueprint's footprint; its height is
/// kept as sent; and the copy sent to its builder carries those numbers, `mine`, and no builder.
///
/// Seen red 2026-10-05 with the pose as sent kept instead of the exact one: "kept on the grid /
/// left: [20.0004, 0.2, 30.0003] / right: [20.0, 0.2, 30.0]".
#[test]
fn the_pose_kept_is_the_exact_one() {
    let (mut world, db) = ship("exact", &[(1, Some("p1"))]);
    let ann = who(&world, 1, Ranks::default());
    let mut sent = ask(&world, "plot:p1", "wood_wall", [20.0004, 0.2, 30.0003], 1);
    sent.local.rotation = (quarter_turn(1) * Quat::from_rotation_x(0.2_f32.to_radians())).normalize();
    let (kept, seq) = build(&mut world, &db, &ann, &sent).expect("inside the tolerances");
    assert_eq!(kept.position, [20.0, 0.2, 30.0], "kept on the grid / left: {:?} / right: [20.0, 0.2, 30.0]", kept.position);
    assert_eq!(kept.rotation, quarter_turn(1).to_array(), "exactly the quarter turn");
    assert_eq!(kept.scale, [4.0, 3.0, 0.2], "exactly the wall's footprint and height");
    assert_eq!(seq, 1, "the first change in p1");
    let rows = db.load_world_pieces(&world.ship_plots.ship_id).unwrap();
    assert_eq!(rows, vec![kept.clone()], "the row holds the same numbers");
    let copy = kept.to_piece_for(&ann.did);
    assert!(copy.mine && copy.position == kept.position && copy.rotation == kept.rotation);
    let text = serde_json::to_string(&copy).unwrap();
    assert!(!text.contains("did:hum") && !text.contains("owner"), "no builder in the copy: {text}");
}
