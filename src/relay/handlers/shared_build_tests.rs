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

/// A household permit the holder with seed `issuer` signs, on this relay, for the player with
/// seed `grantee` on `plot`, running out at `expiry`, minted at `minted` (the minting refuses an
/// end date already past at that time).
fn permit(world: &GameWorld, issuer: u8, plot: &str, grantee: u8, expiry: u64, minted: u64) -> Permit {
    let server = world.pieces.server_did.clone();
    let grantee = plot_owner_id(&key(grantee));
    let sig = build_plot_permit(&[issuer; 32], &server, plot, &grantee, expiry, minted).expect("minted");
    Permit { issuer: key(issuer), server, plot: plot.into(), grantee, expiry, sig }
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
        p.sig = build_plot_permit(&[1; 32], elsewhere, "p1", &p.grantee, p.expiry, NOW).unwrap();
        p.server = elsewhere.into();
        p
    };
    let too_long = {
        // Signed by hand, as a modified game could: the minting never makes one past 90 days.
        let mut p = good.clone();
        p.expiry = NOW + 92 * DAY;
        let issuer = DilithiumKeypair::from_seed(&derive_dilithium_seed(&[1; 32]));
        p.sig = B64.encode(issuer.sign(plot_permit_preimage(&world.pieces.server_did, "p1", &p.grantee, p.expiry).as_bytes()));
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
