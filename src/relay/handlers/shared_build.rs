//! Building in the shared world, the relay's side (ship homes increment 5, "building only on your
//! own plot", 2026-10-05; docs/design/ship-homes-increment-5-plan.md sections 3.2 and 3.3). The
//! wire and the rules for a proper piece are the contract the relay and the game both compile,
//! src/systems/construction/shared.rs; this file is what the relay does with them.
//!
//! WHAT THE RELAY KEEPS. Every shell piece built in the shared world (a foundation, a wall, a
//! window wall, a roof: the blueprints data/blueprints/basic.ron marks `shared: true`) is one row
//! of `world_pieces` (storage/world_pieces.rs) and one entry in the [`PieceBook`] on the game
//! world, filed under its FRAME (src/ship/build_frames.rs: a plot, `plot:p3`, or one of the ship's
//! shared spaces, `zone:commons`) with its pose measured from the frame's corner. The row is
//! written BEFORE anyone is told, so nobody is ever shown a piece the relay could not bring back
//! after a restart. Each frame counts its changes (`seq`: up by one for each piece built or taken
//! down there, from 0 when the relay starts), so a game can tell a message it already has from
//! one it missed and ask for the frame again.
//!
//! WHO MAY BUILD AND TAKE DOWN WHAT (the operator, 2026-10-03 and 2026-10-05; [`may_build`],
//! [`may_remove`]):
//! - on your own plot you build, and you take down anything that stands on it;
//! - on someone else's plot, only with a household permit its holder signed for you on THIS
//!   server, checked without storing it (`relay::core::pq_crypto::verify_plot_permit`, given the
//!   relay's own facts: its server DID, the frame's plot and the sender's own id, never what the
//!   permit says it is for), and there you take down only what you put up;
//! - in the ship's shared spaces, only with the server's `can_edit_ship` rank
//!   (`Storage::has_ship_rank`), and with it you take down any piece there;
//! - the server's admins and its owner take down anything, anywhere (`take_down_any`);
//! - a guest (no plot on this ship) builds nowhere without a permit;
//! - whoever takes a piece down gets its materials back (the game does that: the relay holds no
//!   inventories yet);
//! - a plot given back (an admin's release, a home that does not fit, an erased account) takes
//!   down every piece on it, whoever built them, and an erased account's pieces come down
//!   wherever they stand.
//!
//! NOBODY IS TOLD WHO BUILT WHAT. The relay keeps each piece's builder (`owner_did`) to answer who
//! may take it down, and never sends it: each player's copy of a piece says only whether it is
//! theirs (`mine`). Increment 4's rule that nobody is told who lives where, carried over to what
//! they build.
//!
//! WHO HEARS OF WHAT. A player is sent the pieces of the frames in their view (increment 4's
//! distances, data/ship/shared_world.ron `delivery`: in view once within 250 m of a frame's floor,
//! out again only past 300 m, the gap keeping someone at the edge from flickering): the whole
//! list when the frame comes into view or they join (`game_pieces`, in parts of 128), then each
//! change as it happens (`game_built`, `game_unbuilt`), and `game_frame_out_of_view` when it
//! leaves their view. Whoever asked for a change is always answered, frame in view or not. A
//! change to many pieces at once (a plot given back, an erased account) is told as the frame's
//! new list, one message per player, rather than a message per piece, which could overrun the
//! relay's 256-message send queue (relay.rs `DEFAULT_BROADCAST_CAPACITY`).
//!
//! WHAT THE RELAY LOGS (the proof rig reads these lines, scripts/lib/shared-build-judge.js; none
//! names a key): `Game: built piece {id} {bp} on {frame}`, `Game: build refused ({reason}/{why})
//! on {frame}` (`({reason})` with no why), `Game: took down piece {id} on {frame}`, `Game:
//! take-down refused ({reason}/{why})`, exactly; a refusal's own detail (which number was off,
//! and by how much) goes to a debug line of its own.
//!
//! Its own file because relay.rs and msg_handlers.rs are held to line budgets
//! (tests/file_size_ratchet.rs): each makes one-line calls into it.

use super::game_interest::send_to;
use super::game_state::{plot_owner_id, GameEntity, GameWorld};
use crate::ecs::components::Transform;
use crate::relay::core::pq_crypto::{verify_plot_permit, PlotPermitError};
use crate::relay::relay::RelayState;
use crate::relay::storage::{NewPiece, Storage, StoredPiece};
use crate::ship::build_frames::{plot_frame_id, BuildFrame, BuildFrames, FrameKind};
use crate::ship::ship_structure::ShipStructure;
use crate::systems::construction::shared::{self, Action, FromRelay, Permit, Ranks, Reason, Refusal, ToRelay, Why};
use crate::systems::construction::BlueprintRegistry;
use glam::{Quat, Vec3};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::Arc;

/// The longest a permit's issuer may be, characters: a Dilithium3 public key is 1,952 bytes, 3,904
/// in hex. Anything longer is no key, and is refused as `bad_shape` before any work is done on it.
const PERMIT_ISSUER_MAX_LEN: usize = 4096;
/// The longest a permit's signature may be, characters: a Dilithium3 signature is 3,293 bytes,
/// about 4,400 in base64.
const PERMIT_SIG_MAX_LEN: usize = 8192;

// ── What the relay holds ──────────────────────────────────────────────────

/// Every piece the relay keeps in its shared world, and which frames each player has in view
/// (`GameWorld::pieces`). The frames and the blueprints come with the ship (`set_ship`, from
/// `GameWorld::load_ship`); the pieces from the database when the relay starts ([`load_pieces`]).
/// Never saved as a whole: each piece is its own row, written as it changes.
pub struct PieceBook {
    /// This ship's frames: its shared spaces and its plots. Empty when the ship did not load, and
    /// then every build is refused as `bad_frame`.
    frames: BuildFrames,
    /// What a piece may be: data/blueprints/basic.ron (the file on disk, else the copy built in).
    blueprints: BlueprintRegistry,
    /// The pieces, by frame and then by number, with each frame's count of changes.
    by_frame: BTreeMap<String, FrameBook>,
    /// Each player in the shared world, by entity id: who they are and which frames they have in
    /// view. Made on their join ([`on_join`]), forgotten when they leave
    /// (`GameWorld::despawn_player`).
    views: HashMap<u64, Viewer>,
    /// This relay's own `did:hum:` (`Storage::server_did`, its /api/server-info `server_did`): the
    /// server a household permit must name to be good here.
    server_did: String,
}

/// One frame's pieces and its count of changes.
#[derive(Default)]
struct FrameBook {
    /// Up by one for every piece built or taken down in this frame since the relay started.
    seq: u64,
    pieces: BTreeMap<u64, StoredPiece>,
}

/// One player, as building news is sent to them.
struct Viewer {
    key: String,
    /// Their id as plots are held under it (`plot_owner_id`): what tells their own pieces apart.
    did: String,
    /// The frames in their view.
    frames: BTreeSet<String>,
}

impl Viewer {
    fn new(key: &str) -> Viewer {
        Viewer { key: key.to_string(), did: plot_owner_id(key), frames: BTreeSet::new() }
    }
}

impl Default for PieceBook {
    fn default() -> Self {
        PieceBook {
            frames: BuildFrames::default(),
            blueprints: BlueprintRegistry::new(),
            by_frame: BTreeMap::new(),
            views: HashMap::new(),
            server_did: String::new(),
        }
    }
}

impl PieceBook {
    /// The frames and blueprints of the ship the relay just loaded (`GameWorld::load_ship`).
    pub fn set_ship(&mut self, ship: &ShipStructure) {
        self.frames = BuildFrames::of_ship(ship);
        self.blueprints = load_blueprints(std::path::Path::new("data"));
    }

    /// Forget a player who left the shared world (`GameWorld::despawn_player`).
    pub fn forget(&mut self, entity: u64) {
        self.views.remove(&entity);
    }

    /// Piece `piece_id`, wherever it stands.
    fn piece(&self, piece_id: u64) -> Option<&StoredPiece> {
        self.by_frame.values().find_map(|f| f.pieces.get(&piece_id))
    }

    /// The pieces kept in `frame`, by number.
    pub fn pieces_in(&self, frame: &str) -> Vec<&StoredPiece> {
        self.by_frame.get(frame).map(|f| f.pieces.values().collect()).unwrap_or_default()
    }

    /// `frame`'s count of changes since the relay started.
    pub fn seq_of(&self, frame: &str) -> u64 {
        self.by_frame.get(frame).map_or(0, |f| f.seq)
    }

    /// How many pieces `owner_did` keeps on this ship.
    fn count_of(&self, owner_did: &str) -> usize {
        self.by_frame.values().map(|f| f.pieces.values().filter(|p| p.owner_did == owner_did).count()).sum()
    }

    /// How many pieces this ship holds.
    fn count_all(&self) -> usize {
        self.by_frame.values().map(|f| f.pieces.len()).sum()
    }

    /// The name a refusal gives a piece of blueprint `id`: its own ("Wood Wall"), or "piece".
    fn name_of(&self, id: &str) -> String {
        self.blueprints.get(id).map_or_else(|| "piece".to_string(), |b| b.name.clone())
    }

    /// The frames `player` has in view (empty for one the relay does not know).
    pub fn frames_in_view(&self, player: u64) -> Vec<String> {
        self.views.get(&player).map(|v| v.frames.iter().cloned().collect()).unwrap_or_default()
    }

    /// The players with `frame` in view.
    fn viewers(&self, frame: &str) -> impl Iterator<Item = &Viewer> {
        let frame = frame.to_string();
        self.views.values().filter(move |v| v.frames.contains(&frame))
    }

    /// Judge which frames `player`, standing at `at`, has in view: a frame comes into view within
    /// `in_m` of its floor and leaves it only past `out_m`; between the two nothing changes.
    /// Returns the frames that came into view and those that left it, in the ship's order.
    pub fn rejudge(&mut self, player: u64, at: Vec3, in_m: f32, out_m: f32) -> (Vec<String>, Vec<String>) {
        let Some(v) = self.views.get_mut(&player) else { return (Vec::new(), Vec::new()) };
        let (mut came, mut went) = (Vec::new(), Vec::new());
        for f in &self.frames.frames {
            let d = f.floor_distance(at);
            let seen = v.frames.contains(&f.id);
            if !seen && d <= in_m {
                v.frames.insert(f.id.clone());
                came.push(f.id.clone());
            } else if seen && d > out_m {
                v.frames.remove(&f.id);
                went.push(f.id.clone());
            }
        }
        (came, went)
    }
}

/// The blueprints a piece may be: data/blueprints/basic.ron from `data` (disk first, as the game
/// reads it), else the copy built into the exe, said in the log (BUG-133: the rigs refuse a run
/// that served a built-in copy). Empty, with an error, if neither parses: every build is then
/// refused as `unknown_blueprint`.
fn load_blueprints(data: &std::path::Path) -> BlueprintRegistry {
    let read = crate::embedded_data::read_data_or_embedded(data, "blueprints/basic.ron")
        .ok_or_else(|| "it is neither on disk nor built in".to_string())
        .and_then(|text| BlueprintRegistry::from_ron(text.as_bytes()));
    match read {
        Ok(reg) => reg,
        Err(e) => {
            crate::embedded_data::note_builtin_copy("blueprints/basic.ron", format_args!("data/blueprints/basic.ron does not load ({e})"));
            BlueprintRegistry::from_ron(crate::embedded_data::BLUEPRINT_BASIC_RON.as_bytes()).unwrap_or_else(|e| {
                tracing::error!("Game: the built-in blueprints do not parse either ({e}); nothing can be built in the shared world");
                BlueprintRegistry::new()
            })
        }
    }
}

/// Fill `world`'s piece book from the database when the relay starts (relay.rs `RelayState::new`):
/// this relay's own `did:hum:` (what a permit must name), and every piece kept on this ship. A
/// piece whose frame has left the ship file, or whose blueprint has left basic.ron, is left out
/// with a log line and its row kept, so putting the frame or the blueprint back brings it back.
pub fn load_pieces(world: &mut GameWorld, db: &Storage) {
    match db.server_did() {
        Ok(did) => world.pieces.server_did = did,
        Err(e) => tracing::error!("Game: this relay's did:hum could not be read ({e}); no household permit will be accepted"),
    }
    let ship = world.ship_plots.ship_id.clone();
    if ship.is_empty() {
        return;
    }
    let rows = match db.load_world_pieces(&ship) {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!("Game: the building pieces of ship {ship} could not be read ({e}); the shared world starts with none");
            return;
        }
    };
    let book = &mut world.pieces;
    let (mut kept, mut left_out) = (0usize, 0usize);
    for p in rows {
        if book.frames.get(&p.frame).is_none() || book.blueprints.get(&p.blueprint_id).is_none() {
            tracing::warn!(
                "Game: piece {} ({}) on {} is not of this ship's frames and blueprints any more: left out, its row kept",
                p.piece_id,
                clean(&p.blueprint_id),
                clean(&p.frame)
            );
            left_out += 1;
            continue;
        }
        book.by_frame.entry(p.frame.clone()).or_default().pieces.insert(p.piece_id, p);
        kept += 1;
    }
    tracing::info!("Game: ship {ship} keeps {kept} building piece(s) in its shared world ({left_out} left out)");
}

// ── Who is asking ─────────────────────────────────────────────────────────

/// The player a request comes from, as the rules need them.
#[derive(Debug, Clone)]
pub struct Asker {
    pub key: String,
    /// Their id as plots are held under it (`plot_owner_id`): what their pieces are kept under,
    /// and the grantee a permit for them must name.
    pub did: String,
    /// The plot this relay holds for them this session (their entity's `home_plot`, set on every
    /// join, ship_world.rs `set_home_plot`), by its ship-file id: None for a guest.
    pub home_plot: Option<String>,
    /// What they may do beyond their own plot ([`ranks_of`]).
    pub ranks: Ranks,
}

impl Asker {
    /// The player with `key`, None when they are not in the shared world.
    pub fn in_world(world: &GameWorld, key: &str, ranks: Ranks) -> Option<Asker> {
        let e = world.entities.get(&world.find_player_entity(key)?)?;
        Some(Asker { key: key.to_string(), did: plot_owner_id(key), home_plot: home_plot_id(e), ranks })
    }
}

/// The ship-file id of the plot a player's entity carries (`home_plot.id`), None for a guest.
fn home_plot_id(e: &GameEntity) -> Option<String> {
    e.components.get("home_plot")?.get("id")?.as_str().map(str::to_string)
}

/// What the player with `key` may do beyond their own plot: build in the ship's shared spaces
/// (`Storage::has_ship_rank`: a role with `can_edit_ship`, or the server's owner), and take down
/// anything anywhere (the game-admin rule, `is_game_admin`: an admin or the owner). The welcome
/// says it (`ranks`), and every build and take-down is judged by it afresh, so a rank given or
/// taken away counts from the next request.
pub fn ranks_of(state: &Arc<RelayState>, key: &str) -> Ranks {
    Ranks { can_edit_ship: state.db.has_ship_rank(key), take_down_any: super::msg_handlers::is_game_admin(state, key) }
}

// ── The rules ─────────────────────────────────────────────────────────────

/// May `who` build in `frame`?
/// - a plot: its holder, without a word (their entity's plot, no database read); anyone else
///   only with a permit for it ([`permit_lets_in`]); else `not_your_plot`, or `guest` for someone
///   with no plot of their own;
/// - a shared space: only with the `can_edit_ship` rank, else `ship_rank`.
pub fn may_build(
    book: &PieceBook,
    db: &Storage,
    world_id: &str,
    frame: &BuildFrame,
    who: &Asker,
    permit: Option<&Permit>,
    now_s: u64,
) -> Result<(), Refusal> {
    match frame.kind {
        FrameKind::Zone if who.ranks.can_edit_ship => Ok(()),
        FrameKind::Zone => Err(Refusal::not_allowed(Why::ShipRank)),
        FrameKind::Plot => {
            let plot = frame.place_id();
            if who.home_plot.as_deref() == Some(plot) {
                return Ok(());
            }
            match permit {
                Some(p) => permit_lets_in(book, db, world_id, plot, who, p, now_s),
                None if who.home_plot.is_none() => Err(Refusal::not_allowed(Why::Guest)),
                None => Err(Refusal::not_allowed(Why::NotYourPlot)),
            }
        }
    }
}

/// May `who` take `piece` (standing in `frame`) down?
/// - the server's admins and its owner: anything (`take_down_any`);
/// - a plot's holder: anything on their plot, whoever built it;
/// - someone with a permit for the plot: only what they put up there, else `not_your_piece`;
/// - anyone else on a plot: `not_your_plot`;
/// - a shared space: anyone with the `can_edit_ship` rank, else `ship_rank`.
#[allow(clippy::too_many_arguments)]
pub fn may_remove(
    book: &PieceBook,
    db: &Storage,
    world_id: &str,
    frame: &BuildFrame,
    piece: &StoredPiece,
    who: &Asker,
    permit: Option<&Permit>,
    now_s: u64,
) -> Result<(), Refusal> {
    if who.ranks.take_down_any {
        return Ok(());
    }
    match frame.kind {
        FrameKind::Zone if who.ranks.can_edit_ship => Ok(()),
        FrameKind::Zone => Err(Refusal::not_allowed(Why::ShipRank)),
        FrameKind::Plot => {
            let plot = frame.place_id();
            if who.home_plot.as_deref() == Some(plot) {
                return Ok(());
            }
            let Some(p) = permit else { return Err(Refusal::not_allowed(Why::NotYourPlot)) };
            permit_lets_in(book, db, world_id, plot, who, p, now_s)?;
            if piece.owner_did == who.did {
                Ok(())
            } else {
                Err(Refusal::not_allowed(Why::NotYourPiece))
            }
        }
    }
}

/// Does `p` let `who` onto `plot`? Its signature must be its issuer's over the words this relay
/// rebuilds from its OWN facts (its own server DID, this plot, the sender's own id), so a permit
/// given on another server, for another plot or to someone else does not check out
/// (`permit_bad`); it must not have run out (`permit_expired`) nor run more than 90 days ahead
/// (`permit_too_long`); and its issuer must hold the plot now, so a permit stops working when the
/// plot changes hands (`permit_not_from_holder`).
fn permit_lets_in(book: &PieceBook, db: &Storage, world_id: &str, plot: &str, who: &Asker, p: &Permit, now_s: u64) -> Result<(), Refusal> {
    verify_plot_permit(&p.issuer, &book.server_did, plot, &who.did, p.expiry, &p.sig, now_s).map_err(|e| {
        Refusal::not_allowed(match e {
            PlotPermitError::Expired => Why::PermitExpired,
            PlotPermitError::TooLong => Why::PermitTooLong,
            PlotPermitError::Malformed | PlotPermitError::BadSignature => Why::PermitBad,
        })
    })?;
    match db.plot_holder(world_id, plot) {
        Ok(Some(holder)) if holder == plot_owner_id(&p.issuer) => Ok(()),
        Ok(_) => Err(Refusal::not_allowed(Why::PermitNotFromHolder)),
        Err(e) => Err(Refusal::new(Reason::StorageError, format!("who holds {plot} could not be read: {e}"))),
    }
}

/// A piece's pose as kept, measured from its frame's corner.
fn local_pose(p: &StoredPiece) -> Transform {
    Transform { position: Vec3::from_array(p.position), rotation: Quat::from_array(p.rotation), scale: Vec3::from_array(p.scale) }
}

/// What a `game_build` asks for, once it reads.
#[derive(Debug, Clone)]
pub struct BuildAsk {
    pub frame: String,
    pub blueprint_id: String,
    /// The pose as sent, measured from the frame's corner.
    pub local: Transform,
    pub permit: Option<Permit>,
}

/// A build the rules accepted, ready to keep ([`keep`]).
#[derive(Debug, Clone)]
pub struct Accepted {
    /// The piece as it will be kept: the exact pose, its builder, the relay's time.
    pub piece: NewPiece,
    /// Its blueprint's name ("Wood Wall").
    pub name: String,
}

/// A request the rules refused: why, and the name its sentence gives the piece (the blueprint's,
/// or "piece" when it is not known, shared.rs `refusal_message`).
#[derive(Debug, Clone)]
pub struct Refused {
    pub refusal: Refusal,
    pub piece: String,
}

impl Refused {
    fn of(piece: &str, refusal: Refusal) -> Refused {
        Refused { refusal, piece: piece.to_string() }
    }
}

/// Judge a build by `who`, cheapest check first, each failure its own reason (the sender's place
/// in the world and the message's shape are checked before, by [`handle`]):
/// 1. the frame is one of this ship's (`bad_frame`);
/// 2. the blueprint is known (`unknown_blueprint`) and shareable (`not_shared`);
/// 3. the pose is one the game's placement makes, made exact, inside the frame
///    (`shared::pose_in_frame`: `bad_shape`, `bad_turn`, `off_grid`, `bad_scale`,
///    `outside_frame`, `out_of_bounds`);
/// 4. `who` may build there ([`may_build`]: `not_allowed`);
/// 5. no piece with the same box stands there already (`occupied`);
/// 6. there is room: in the frame (`frame_full`), for the builder (`owner_full`), on the ship
///    (`world_full`).
///
/// Ok holds the piece as it will be kept, its pose the EXACT one (on the grid, a whole quarter
/// turn, the blueprint's own footprint), so every game holds the same numbers.
pub fn judge_build(world: &GameWorld, db: &Storage, who: &Asker, ask: &BuildAsk, now_ms: i64, now_s: u64) -> Result<Accepted, Refused> {
    let book = &world.pieces;
    let Some(frame) = book.frames.get(&ask.frame) else {
        return Err(Refused::of("piece", Refusal::new(Reason::BadFrame, format!("{} is not a frame of this ship", clean(&ask.frame)))));
    };
    let Some(bp) = book.blueprints.get(&ask.blueprint_id) else {
        let detail = format!("no blueprint {} here", clean(&ask.blueprint_id));
        return Err(Refused::of("piece", Refusal::new(Reason::UnknownBlueprint, detail)));
    };
    let refused = |r: Refusal| Refused::of(&bp.name, r);
    if !bp.shared {
        return Err(refused(Refusal::new(Reason::NotShared, format!("{} is not marked shared in basic.ron", bp.id))));
    }
    let exact = shared::pose_in_frame(bp, frame, &ask.local).map_err(refused)?;
    may_build(book, db, &world.ship_plots.ship_id, frame, who, ask.permit.as_ref(), now_s).map_err(refused)?;
    let here = book.by_frame.get(&frame.id);
    if here.is_some_and(|f| f.pieces.values().any(|p| shared::same_box(&local_pose(p), &exact))) {
        return Err(refused(Refusal::new(Reason::Occupied, format!("a piece with that box stands on {} already", frame.id))));
    }
    if here.map_or(0, |f| f.pieces.len()) >= shared::MAX_PIECES_PER_FRAME {
        return Err(refused(Refusal::new(Reason::FrameFull, format!("{} holds {} pieces", frame.id, shared::MAX_PIECES_PER_FRAME))));
    }
    if book.count_of(&who.did) >= shared::MAX_PIECES_PER_OWNER {
        return Err(refused(Refusal::new(Reason::OwnerFull, format!("the builder keeps {} pieces", shared::MAX_PIECES_PER_OWNER))));
    }
    if book.count_all() >= shared::MAX_PIECES_PER_WORLD {
        return Err(refused(Refusal::new(Reason::WorldFull, format!("the ship holds {} pieces", shared::MAX_PIECES_PER_WORLD))));
    }
    Ok(Accepted {
        piece: NewPiece {
            world_id: world.ship_plots.ship_id.clone(),
            frame: frame.id.clone(),
            blueprint_id: bp.id.clone(),
            position: exact.position.to_array(),
            rotation: exact.rotation.to_array(),
            scale: exact.scale.to_array(),
            owner_did: who.did.clone(),
            placed_at_ms: now_ms,
        },
        name: bp.name.clone(),
    })
}

/// Keep an accepted piece: its row first (a database that will not take it refuses the build as
/// `storage_error`, and nothing changes), then the relay's own copy, and the frame's count of
/// changes moves on by one. The piece as kept, with its number, and the frame's new `seq`.
pub fn keep(world: &mut GameWorld, db: &Storage, a: Accepted) -> Result<(StoredPiece, u64), Refused> {
    let id = db
        .insert_world_piece(&a.piece)
        .map_err(|e| Refused::of(&a.name, Refusal::new(Reason::StorageError, format!("the piece was not written: {e}"))))?;
    let kept = a.piece.kept_as(id);
    let frame = world.pieces.by_frame.entry(kept.frame.clone()).or_default();
    frame.seq += 1;
    frame.pieces.insert(id, kept.clone());
    Ok((kept, frame.seq))
}

/// Judge a take-down by `who`: the piece is on this ship (`no_such_piece`), and `who` may take it
/// down ([`may_remove`]). Ok holds the piece as kept.
pub fn judge_unbuild(world: &GameWorld, db: &Storage, who: &Asker, piece_id: u64, permit: Option<&Permit>, now_s: u64) -> Result<StoredPiece, Refused> {
    let book = &world.pieces;
    let Some(piece) = book.piece(piece_id) else {
        return Err(Refused::of("piece", Refusal::new(Reason::NoSuchPiece, format!("no piece {piece_id} on this ship"))));
    };
    let name = book.name_of(&piece.blueprint_id);
    let Some(frame) = book.frames.get(&piece.frame) else {
        return Err(Refused::of(&name, Refusal::new(Reason::BadFrame, format!("piece {piece_id} stands in no frame of this ship"))));
    };
    may_remove(book, db, &world.ship_plots.ship_id, frame, piece, who, permit, now_s).map_err(|r| Refused::of(&name, r))?;
    Ok(piece.clone())
}

/// Take `piece` down: its row first (a database that will not let it go refuses the take-down as
/// `storage_error`, and nothing changes), then the relay's copy, and its frame's count of changes
/// moves on by one. The frame's new `seq`.
pub fn take_down(world: &mut GameWorld, db: &Storage, piece: &StoredPiece) -> Result<u64, Refused> {
    let name = world.pieces.name_of(&piece.blueprint_id);
    match db.delete_world_piece(&piece.world_id, piece.piece_id) {
        Err(e) => Err(Refused::of(&name, Refusal::new(Reason::StorageError, format!("the piece was not deleted: {e}")))),
        Ok(gone) => {
            if !gone {
                // Its row was gone already (nothing but this relay writes the table): it comes
                // down here too, so every game agrees with the database.
                tracing::warn!("Game: piece {} on {} had no row left; taken down all the same", piece.piece_id, piece.frame);
            }
            let frame = world.pieces.by_frame.entry(piece.frame.clone()).or_default();
            frame.pieces.remove(&piece.piece_id);
            frame.seq += 1;
            Ok(frame.seq)
        }
    }
}

// ── What is said, and to whom ─────────────────────────────────────────────

/// A message for the wire. Made through text, so each f32 goes out as the short number it is
/// (`0.2`), not as the f64 it widens to (`0.20000000298023224`); every game reads back the same
/// f32 either way. Serialising the contract's own types cannot fail; if it ever did, the error is
/// logged and `null` goes, which every game ignores.
fn wire(m: &FromRelay) -> serde_json::Value {
    serde_json::to_string(m).and_then(|text| serde_json::from_str(&text)).unwrap_or_else(|e| {
        tracing::error!("Game: a building message did not serialise ({e})");
        serde_json::Value::Null
    })
}

/// The time, as the relay keeps it (Unix milliseconds) and as the wire carries it (seconds), both
/// from one reading of the clock so a piece built now is 0 s old when its news arrives.
fn now() -> (i64, f64, u64) {
    let d = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    (d.as_millis() as i64, d.as_secs_f64(), d.as_secs())
}

/// What a piece going up sends, as (to, message): the builder's own copy, with their `req_id`
/// and `mine`, to them, whether or not the frame is in their view (it answers their request);
/// and to every other player with the frame in view a copy that says nothing of who built it,
/// `mine` only in the copies of whoever did.
pub fn built_messages(book: &PieceBook, builder_key: &str, req_id: Option<u32>, piece: &StoredPiece, seq: u64, server_time: f64) -> Vec<(HashSet<String>, serde_json::Value)> {
    let copy = |viewer_did: &str, req_id: Option<u32>| {
        wire(&FromRelay::Built { frame: piece.frame.clone(), seq, server_time, piece: piece.to_piece_for(viewer_did), req_id })
    };
    let (mut theirs, mut others) = (HashSet::new(), HashSet::new());
    for v in book.viewers(&piece.frame).filter(|v| v.key != builder_key) {
        let to = if v.did == piece.owner_did { &mut theirs } else { &mut others };
        to.insert(v.key.clone());
    }
    let mut out = vec![(HashSet::from([builder_key.to_string()]), copy(&piece.owner_did, req_id))];
    if !theirs.is_empty() {
        out.push((theirs, copy(&piece.owner_did, None)));
    }
    if !others.is_empty() {
        out.push((others, copy("", None)));
    }
    out
}

/// What a piece coming down sends: the remover's copy, with their `req_id`, to them, in view or
/// not; the same without it to every other player with the frame in view.
pub fn unbuilt_messages(book: &PieceBook, remover_key: &str, req_id: Option<u32>, piece: &StoredPiece, seq: u64) -> Vec<(HashSet<String>, serde_json::Value)> {
    let copy = |req_id: Option<u32>| wire(&FromRelay::Unbuilt { frame: piece.frame.clone(), seq, piece_id: piece.piece_id, req_id });
    let others: HashSet<String> = book.viewers(&piece.frame).filter(|v| v.key != remover_key).map(|v| v.key.clone()).collect();
    let mut out = vec![(HashSet::from([remover_key.to_string()]), copy(req_id))];
    if !others.is_empty() {
        out.push((others, copy(None)));
    }
    out
}

/// Every piece of `frame` as the player whose id is `viewer_did` is shown them (`mine` on their
/// own), in parts of `shared::PIECES_PER_PART` numbered from 1, all carrying the frame's `seq`;
/// an empty frame is one part with no pieces.
pub fn snapshot_messages(book: &PieceBook, frame: &str, viewer_did: &str, server_time: f64) -> Vec<serde_json::Value> {
    let seq = book.seq_of(frame);
    let pieces: Vec<shared::Piece> = book.pieces_in(frame).into_iter().map(|p| p.to_piece_for(viewer_did)).collect();
    let parts: Vec<Vec<shared::Piece>> =
        if pieces.is_empty() { vec![Vec::new()] } else { pieces.chunks(shared::PIECES_PER_PART).map(|c| c.to_vec()).collect() };
    let count = parts.len() as u32;
    parts
        .into_iter()
        .enumerate()
        .map(|(i, pieces)| wire(&FromRelay::Pieces { frame: frame.to_string(), seq, server_time, part: i as u32 + 1, parts: count, pieces }))
        .collect()
}

/// Send `player` the frames that came into their view (each one's whole list) and tell them of
/// those that left it.
fn send_view_changes(state: &RelayState, book: &PieceBook, player: u64, came: &[String], went: &[String]) {
    let Some(v) = book.views.get(&player) else { return };
    let (_, server_time, _) = now();
    for frame in came {
        for m in snapshot_messages(book, frame, &v.did, server_time) {
            send_to(state, HashSet::from([v.key.clone()]), &m);
        }
    }
    for frame in went {
        send_to(state, HashSet::from([v.key.clone()]), &wire(&FromRelay::FrameOutOfView { frame: frame.clone() }));
    }
}

/// Send every player with `frame` in view the frame's whole list again: how a change to many of
/// its pieces at once is told (see the top of this file).
fn resend_frame(state: &RelayState, book: &PieceBook, frame: &str) {
    let (_, server_time, _) = now();
    for v in book.viewers(frame) {
        for m in snapshot_messages(book, frame, &v.did, server_time) {
            send_to(state, HashSet::from([v.key.clone()]), &m);
        }
    }
}

// ── The world's comings and goings ────────────────────────────────────────

/// A player joined or rejoined the shared world (msg_handlers.rs `handle_game_join`, right after
/// their welcome, which starts their game's shared pieces afresh): their view is judged anew from
/// where they stand and each frame in it is sent whole. Called with the world's lock held, so
/// nothing built after the lists can reach them before.
pub fn on_join(state: &RelayState, world: &mut GameWorld, player: u64, key: &str) {
    let Some(at) = world.entities.get(&player).map(|e| Vec3::from_array(e.position)) else { return };
    let (in_m, out_m) = (world.rules.delivery.in_view_m, world.rules.delivery.out_of_view_m);
    world.pieces.views.insert(player, Viewer::new(key));
    let (came, went) = world.pieces.rejudge(player, at, in_m, out_m);
    send_view_changes(state, &world.pieces, player, &came, &went);
}

/// A player's move was accepted (msg_handlers.rs `handle_game_position_update`): a frame that
/// came into their view is sent whole, one that left it is said to have gone. Called with the
/// world's lock held, before it is let go (increment 4's P4 rule).
pub fn on_move(state: &RelayState, world: &mut GameWorld, player: u64) {
    let Some(e) = world.entities.get(&player) else { return };
    let at = Vec3::from_array(e.position);
    if !world.pieces.views.contains_key(&player) {
        // In the world without a join this relay saw (none today): their view starts now.
        let Some(key) = e.owner.clone() else { return };
        world.pieces.views.insert(player, Viewer::new(&key));
    }
    let (in_m, out_m) = (world.rules.delivery.in_view_m, world.rules.delivery.out_of_view_m);
    let (came, went) = world.pieces.rejudge(player, at, in_m, out_m);
    send_view_changes(state, &world.pieces, player, &came, &went);
}

/// Take down every piece in `frame`, whoever built it: a plot given back (decision 2 of the
/// increment 5 plan: otherwise the next household moves in among a stranger's walls). The rows
/// first; then the relay's copies, the frame's count of changes moving on by one for each piece,
/// and every player with the frame in view is sent its new, empty list. How many rows went (0,
/// with an error in the log, when the database would not let them go: they then stay, and the
/// plot's next holder can take them down).
pub fn take_down_frame(state: &RelayState, world: &mut GameWorld, db: &Storage, frame: &str) -> usize {
    let ship = world.ship_plots.ship_id.clone();
    let rows = match db.delete_world_pieces_in_frame(&ship, frame) {
        Ok(n) => n,
        Err(e) => {
            tracing::error!("Game: the pieces on {} could not be taken down ({e}); they stay", clean(frame));
            return 0;
        }
    };
    if let Some(f) = world.pieces.by_frame.get_mut(frame) {
        let had = f.pieces.len() as u64;
        f.pieces.clear();
        f.seq += had;
        if had > 0 {
            resend_frame(state, &world.pieces, frame);
        }
    }
    if rows > 0 {
        tracing::info!("Game: {rows} piece(s) on {} came down with the plot", clean(frame));
    }
    rows
}

/// [`take_down_frame`] for the plot whose ship-file id is `plot` (`p3`): a plot given back.
pub fn take_down_plot(state: &RelayState, world: &mut GameWorld, db: &Storage, plot: &str) -> usize {
    take_down_frame(state, world, db, &plot_frame_id(plot))
}

/// An account is being erased (home_plots.rs `leave_world_for_erase`, under the world's lock,
/// before `Storage::delete_account`): the plot it held (`freed_plot`, given back in the same
/// step) takes its pieces down with it, rows and all, and every piece the account built anywhere
/// else leaves the relay's copy now (its rows go with the rest of the account). Every player with
/// a frame that changed in view gets the frame's new list. How many rows the plot's pieces were,
/// for the erase's receipt (the account's own rows elsewhere are counted by the erase).
pub fn erase_account(state: &RelayState, world: &mut GameWorld, db: &Storage, key: &str, freed_plot: Option<&str>) -> usize {
    let on_plot = freed_plot.map_or(0, |plot| take_down_plot(state, world, db, plot));
    let did = plot_owner_id(key);
    let mut changed = Vec::new();
    for (frame, f) in world.pieces.by_frame.iter_mut() {
        let before = f.pieces.len();
        f.pieces.retain(|_, p| p.owner_did != did);
        let gone = (before - f.pieces.len()) as u64;
        if gone > 0 {
            f.seq += gone;
            changed.push(frame.clone());
        }
    }
    for frame in &changed {
        resend_frame(state, &world.pieces, frame);
    }
    on_plot
}

/// The note on an admin's release of a plot saying what came down with it ("" when nothing did).
pub fn came_down_note(pieces: usize) -> String {
    match pieces {
        0 => String::new(),
        1 => " The one piece built on it came down with it.".to_string(),
        n => format!(" The {n} pieces built on it came down with it."),
    }
}

// ── The messages ──────────────────────────────────────────────────────────

/// The building messages (relay.rs routes `game_build`, `game_unbuild` and `game_pieces_request`
/// here). Every refusal goes to the sender alone, with the one sentence shared.rs keeps for it.
pub async fn handle(state: &Arc<RelayState>, key: &str, kind: &str, raw: &serde_json::Value) {
    match kind {
        shared::msg::BUILD => build(state, key, raw).await,
        shared::msg::UNBUILD => unbuild(state, key, raw).await,
        shared::msg::PIECES_REQUEST => pieces_request(state, key, raw).await,
        _ => {}
    }
}

/// The `req_id` a request carries, when it can be read (a refusal echoes it).
fn req_id_of(raw: &serde_json::Value) -> Option<u32> {
    raw.get("req_id").and_then(|v| v.as_u64()).and_then(|n| u32::try_from(n).ok())
}

/// A string from a message, made fit for one log line: at most 64 characters, anything but a
/// letter, a digit or `:-_.` shown as `?`, and `-` when empty. Nothing a sender writes can start
/// a second line or pass for another entry.
fn clean(s: &str) -> String {
    let c: String = s.chars().take(64).map(|c| if c.is_ascii_alphanumeric() || ":-_.".contains(c) { c } else { '?' }).collect();
    if c.is_empty() {
        "-".to_string()
    } else {
        c
    }
}

/// Read a building request, refusing as `bad_shape` one that does not read as its type, has an id
/// over its cap, a number that is not finite (a JSON number past f32's range reads as infinity),
/// or a permit whose fields could be nothing a permit holds.
fn read_request(raw: &serde_json::Value) -> Result<ToRelay, Refusal> {
    let shape = |what: &str| Refusal::new(Reason::BadShape, what.to_string());
    let msg: ToRelay = serde_json::from_value(raw.clone()).map_err(|_| shape("the message does not read as its type says"))?;
    let permit_ok = |p: &Option<Permit>| {
        p.as_ref().is_none_or(|p| {
            p.issuer.len() <= PERMIT_ISSUER_MAX_LEN
                && p.sig.len() <= PERMIT_SIG_MAX_LEN
                && [&p.server, &p.plot, &p.grantee].iter().all(|s| s.len() <= crate::relay::core::pq_crypto::PLOT_PERMIT_FIELD_MAX_LEN)
        })
    };
    match &msg {
        ToRelay::Build { frame, blueprint_id, position, rotation, scale, permit, .. } => {
            if frame.len() > shared::MAX_FRAME_LEN || blueprint_id.len() > shared::MAX_BLUEPRINT_ID_LEN {
                return Err(shape("a frame or blueprint id longer than the wire allows"));
            }
            if position.iter().chain(rotation).chain(scale).any(|v| !v.is_finite()) {
                return Err(shape("a position, turn or size that is not a finite number"));
            }
            if !permit_ok(permit) {
                return Err(shape("a permit field longer than any permit holds"));
            }
        }
        ToRelay::Unbuild { permit, .. } => {
            if !permit_ok(permit) {
                return Err(shape("a permit field longer than any permit holds"));
            }
        }
        ToRelay::PiecesRequest { frame } => {
            if frame.len() > shared::MAX_FRAME_LEN {
                return Err(shape("a frame id longer than the wire allows"));
            }
        }
    }
    Ok(msg)
}

/// Tell the sender `key` their request was refused, with the one sentence for it, and log it
/// in exactly the form the proof rig reads (no key); the refusal's own detail goes to a debug
/// line of its own.
fn refuse(state: &RelayState, key: &str, action: Action, req_id: Option<u32>, frame: &str, refused: &Refused) {
    let r = &refused.refusal;
    let message = shared::refusal_message(action, &refused.piece, r.reason, r.why);
    let m = wire(&FromRelay::Refused { req_id, action, reason: r.reason, why: r.why, message });
    send_to(state, HashSet::from([key.to_string()]), &m);
    let codes = match r.why {
        Some(w) => format!("{}/{}", r.reason.as_str(), w.as_str()),
        None => r.reason.as_str().to_string(),
    };
    match action {
        Action::Build => tracing::info!("Game: build refused ({codes}) on {}", clean(frame)),
        Action::Unbuild => tracing::info!("Game: take-down refused ({codes})"),
        Action::Pieces => tracing::info!("Game: pieces request refused ({codes}) on {}", clean(frame)),
    }
    tracing::debug!("Game: why that {} was refused: {}", action_word(action), r.detail);
}

/// The request a refusal's debug line names.
fn action_word(action: Action) -> &'static str {
    match action {
        Action::Build => "build",
        Action::Unbuild => "take-down",
        Action::Pieces => "pieces request",
    }
}

/// `game_build`: judged ([`judge_build`]), kept ([`keep`]) and told ([`built_messages`]) under the
/// world's lock, so the order the news goes out in is the order of the frame's `seq`.
async fn build(state: &Arc<RelayState>, key: &str, raw: &serde_json::Value) {
    let req_id = req_id_of(raw);
    let frame_said = raw.get("frame").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let no = |r: Refusal| refuse(state, key, Action::Build, req_id, &frame_said, &Refused::of("piece", r));
    if !super::msg_handlers::perception_rate_allows(state, key, "build") {
        return no(Refusal::new(Reason::RateLimited, "a second build within 200 ms"));
    }
    let ranks = ranks_of(state, key);
    let mut world = state.game_world.write().await;
    let Some(who) = Asker::in_world(&world, key, ranks) else {
        return no(Refusal::new(Reason::NotInGame, "the sender is not in the shared world"));
    };
    let ask = match read_request(raw) {
        Ok(ToRelay::Build { frame, blueprint_id, position, rotation, scale, permit, .. }) => BuildAsk {
            frame,
            blueprint_id,
            local: Transform { position: Vec3::from_array(position), rotation: Quat::from_array(rotation), scale: Vec3::from_array(scale) },
            permit,
        },
        Ok(_) => return no(Refusal::new(Reason::BadShape, "not a game_build")),
        Err(r) => return no(r),
    };
    let (now_ms, server_time, now_s) = now();
    let kept = judge_build(&world, &state.db, &who, &ask, now_ms, now_s).and_then(|a| keep(&mut world, &state.db, a));
    match kept {
        Ok((piece, seq)) => {
            for (to, m) in built_messages(&world.pieces, key, req_id, &piece, seq, server_time) {
                send_to(state, to, &m);
            }
            tracing::info!("Game: built piece {} {} on {}", piece.piece_id, piece.blueprint_id, piece.frame);
        }
        Err(refused) => refuse(state, key, Action::Build, req_id, &frame_said, &refused),
    }
}

/// `game_unbuild`: judged ([`judge_unbuild`]), taken down ([`take_down`]) and told
/// ([`unbuilt_messages`]) under the world's lock.
async fn unbuild(state: &Arc<RelayState>, key: &str, raw: &serde_json::Value) {
    let req_id = req_id_of(raw);
    let no = |r: Refusal| refuse(state, key, Action::Unbuild, req_id, "", &Refused::of("piece", r));
    if !super::msg_handlers::perception_rate_allows(state, key, "unbuild") {
        return no(Refusal::new(Reason::RateLimited, "a second take-down within 200 ms"));
    }
    let ranks = ranks_of(state, key);
    let mut world = state.game_world.write().await;
    let Some(who) = Asker::in_world(&world, key, ranks) else {
        return no(Refusal::new(Reason::NotInGame, "the sender is not in the shared world"));
    };
    let (piece_id, permit) = match read_request(raw) {
        Ok(ToRelay::Unbuild { piece_id, permit, .. }) => (piece_id, permit),
        Ok(_) => return no(Refusal::new(Reason::BadShape, "not a game_unbuild")),
        Err(r) => return no(r),
    };
    let (_, _, now_s) = now();
    let done = judge_unbuild(&world, &state.db, &who, piece_id, permit.as_ref(), now_s)
        .and_then(|piece| take_down(&mut world, &state.db, &piece).map(|seq| (piece, seq)));
    match done {
        Ok((piece, seq)) => {
            for (to, m) in unbuilt_messages(&world.pieces, key, req_id, &piece, seq) {
                send_to(state, to, &m);
            }
            tracing::info!("Game: took down piece {} on {}", piece.piece_id, piece.frame);
        }
        Err(refused) => refuse(state, key, Action::Unbuild, req_id, "", &refused),
    }
}

/// `game_pieces_request`: a game that missed a change in a frame (a gap in its `seq`) asks for the
/// frame's whole list again. Once a second per player, whichever frame (`PIECES_REQUEST_INTERVAL_MS`;
/// the game paces its asks a little slower and asks again after a `rate_limited`); only for a
/// frame in their view: one that is not is answered `game_frame_out_of_view`, so the game forgets
/// it rather than asking again.
async fn pieces_request(state: &Arc<RelayState>, key: &str, raw: &serde_json::Value) {
    let frame_said = raw.get("frame").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let no = |r: Refusal| refuse(state, key, Action::Pieces, None, &frame_said, &Refused::of("piece", r));
    let world = state.game_world.read().await;
    let Some(player) = world.find_player_entity(key) else {
        return no(Refusal::new(Reason::NotInGame, "the sender is not in the shared world"));
    };
    let frame = match read_request(raw) {
        Ok(ToRelay::PiecesRequest { frame }) => frame,
        Ok(_) => return no(Refusal::new(Reason::BadShape, "not a game_pieces_request")),
        Err(r) => return no(r),
    };
    if world.pieces.frames.get(&frame).is_none() {
        return no(Refusal::new(Reason::BadFrame, format!("{} is not a frame of this ship", clean(&frame))));
    }
    if !pieces_request_allowed(state, key) {
        return no(Refusal::new(Reason::RateLimited, "a list was asked for within the last second"));
    }
    let book = &world.pieces;
    let Some(v) = book.views.get(&player) else { return };
    if !v.frames.contains(&frame) {
        send_to(state, HashSet::from([key.to_string()]), &wire(&FromRelay::FrameOutOfView { frame }));
        return;
    }
    let (_, server_time, _) = now();
    for m in snapshot_messages(book, &frame, &v.did, server_time) {
        send_to(state, HashSet::from([key.to_string()]), &m);
    }
}

/// One player may ask for a list once a `shared::PIECES_REQUEST_INTERVAL_MS`, whichever frame: the
/// perception limit's map and clock (msg_handlers.rs `perception_rate_allows`, whose 200 ms is
/// too short for a whole list), in a bucket of its own.
fn pieces_request_allowed(state: &RelayState, key: &str) -> bool {
    let now = state.perception_now();
    let bucket = format!("{key}|pieces");
    let mut map = match state.last_perception_times.lock() {
        Ok(m) => m,
        Err(p) => p.into_inner(),
    };
    let allowed = map.get(&bucket).is_none_or(|last| now.duration_since(*last).as_millis() as u64 >= shared::PIECES_REQUEST_INTERVAL_MS);
    if allowed {
        map.insert(bucket, now);
    }
    allowed
}

#[cfg(test)]
#[path = "shared_build_tests.rs"]
mod tests;
