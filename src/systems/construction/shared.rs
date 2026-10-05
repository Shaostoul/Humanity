//! Shared building: the contract (ship homes increment 5, "building only on your own plot",
//! 2026-10-05; docs/design/ship-homes-and-logistics.md section 4 and increment 5,
//! docs/design/shared-building.md).
//!
//! WHAT THIS IS. In a server's shared world, the shell pieces a player builds aboard (the
//! blueprints marked `shared: true` in data/blueprints/basic.ron: foundations, walls, the window
//! wall, the roof) are kept by the server instead of in the player's own save, and everyone near
//! sees them. The server (the relay) keeps one record per piece in a FRAME (src/ship/
//! build_frames.rs: a plot, `plot:p3`, or one of the ship's shared spaces, `zone:commons`), its
//! pose measured from the frame's corner. Each game puts the pieces it is told about into its
//! world marked with a [`SharedPiece`], so they draw, block and shelter like any built piece and
//! never enter the save.
//!
//! WHO MAY BUILD WHERE (the operator, 2026-10-03 and 2026-10-05). The relay decides; the game
//! asks the same questions first only so a player is told before anything is spent.
//! - On your own plot you build, and you may take down anything that stands on it.
//! - On someone else's plot, only with a household permit its holder signed for you
//!   (`relay::core::pq_crypto::verify_plot_permit`, at most [`PERMIT_MAX_DAYS`] days), and you
//!   take down only what you put up there.
//! - In the ship's shared spaces, only people the server gave the `can_edit_ship` rank.
//! - The server's admins and owners take down anything.
//! - A guest (no plot on this ship) builds nowhere without a permit.
//! - Whoever takes a piece down gets its materials back.
//!
//! NOBODY IS TOLD WHO BUILT WHAT. Increment 4's rule (nobody is told who lives where) holds for
//! pieces: no message carries a piece's builder. The relay keeps the builder, to answer who may
//! take a piece down, and tells each player only whether a piece is THEIR OWN ([`Piece::mine`]).
//!
//! WHY ONE FILE BOTH SIDES COMPILE. The relay and the game must agree to the millimetre on what
//! counts as a proper piece (on the grid, turned a whole quarter, its own size, inside its
//! frame). With a copy of the rules on each side they would drift, and a build the game showed as
//! fine would be refused for no reason the player could see. So the rules live here once, and
//! this file compiles into both builds (`--features native` and `--features relay`). The test
//! [`tests::every_shareable_pose_the_placer_makes_passes_validate_pose`] runs the game's own
//! placement against the server's check. One copy of every sentence a refusal shows lives here
//! too ([`why_words`], [`refusal_message`]), so the server's words and the game's are the same.
//! Nothing here refers to the relay module, so the game's world code does not depend on it.
//!
//! WHAT IS HERE:
//! - the wire: [`ToRelay`] and [`FromRelay`] (every message, `__game__:` JSON with its `type`),
//!   [`Piece`], [`Permit`], [`Ranks`] (the welcome's `ranks`), the type strings in [`msg`];
//! - why a request was refused: [`Reason`], [`Why`], [`Action`], [`Refusal`], and their words;
//! - the rules: [`validate_pose`], [`canonicalise`], [`same_box`], [`box_inside_frame`], and
//!   [`pose_in_frame`], all of them in the relay's order;
//! - the game's side: [`SharedPiece`] (the ECS marker), [`SharedBuildIntent`] and [`Spent`] (a
//!   paid-for build waiting to be sent), [`OUT_CHANNEL`] and [`OutQueue`] (where it waits);
//! - the limits: caps, pacing, tolerances.

use super::placement::{quarter_turn, world_aabb, GRID_M, LEVEL_TOLERANCE_M};
use super::{Blueprint, BlueprintRegistry};
use crate::ecs::components::Transform;
use crate::ship::build_frames::{BuildFrame, EDGE_SLACK_M, MAX_LOCAL_Y_M, MIN_LOCAL_Y_M};
use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};

// ── Where a paid-for build waits ──────────────────────────────────────────

/// The DataStore key a shared build waits under, between the ConstructionSystem (which has
/// checked the spot and taken the materials) and the engine (which sends it to the relay). The
/// value is an [`OutQueue`]: the system pushes, the engine drains, the same shape as the
/// `build_request` channel.
pub const OUT_CHANNEL: &str = "shared_build_out";

/// The type stored under [`OUT_CHANNEL`].
pub type OutQueue = std::sync::Mutex<Vec<SharedBuildIntent>>;

// ── How much ──────────────────────────────────────────────────────────────

/// The most pieces one frame (a plot, a shared space) holds: `frame_full` past it.
pub const MAX_PIECES_PER_FRAME: usize = 512;
/// The most pieces one player keeps in one world, wherever they stand: `owner_full` past it.
pub const MAX_PIECES_PER_OWNER: usize = 512;
/// The most pieces one world (one ship) holds: `world_full` past it. Compile-time numbers for
/// now; making them server settings needs Server Settings rows (logged in
/// docs/design/in-app-ops.md).
pub const MAX_PIECES_PER_WORLD: usize = 4096;
/// The most pieces one `game_pieces` message carries; a frame with more arrives in numbered parts.
pub const PIECES_PER_PART: usize = 128;
/// The longest frame id a message may carry, bytes (`plot:p3` is 7).
pub const MAX_FRAME_LEN: usize = 64;
/// The longest blueprint id a message may carry, bytes.
pub const MAX_BLUEPRINT_ID_LEN: usize = 64;

/// The longest a household permit may run, days (the operator, 2026-10-05: every permit has an
/// end date, renewable; no endless permits). The relay enforces it in
/// `relay::core::pq_crypto::verify_plot_permit` (its `PLOT_PERMIT_MAX_DAYS`, which must equal
/// this: the pq_crypto test pins them together), and [`Why::PermitTooLong`]'s words name it.
pub const PERMIT_MAX_DAYS: u64 = 90;

// ── Pace ──────────────────────────────────────────────────────────────────

/// The game sends at most one build or take-down this often, milliseconds: slower than the
/// relay's own limit (one per 200 ms per player and kind of request, `PERCEPTION_MIN_INTERVAL_MS`
/// in relay/handlers/msg_handlers.rs), so a steady builder is never refused for pace.
pub const SEND_INTERVAL_MS: u64 = 250;
/// A request the relay refused as `rate_limited` goes again after this, milliseconds.
pub const RATE_LIMITED_RETRY_MS: u64 = 500;
/// One player may ask for a frame's pieces again (`game_pieces_request`) this often at most,
/// milliseconds.
pub const PIECES_REQUEST_INTERVAL_MS: u64 = 1000;
/// A build or take-down left this long without an answer asks once for its frame's pieces, and
/// that list settles it, seconds.
pub const PENDING_TIMEOUT_S: f32 = 10.0;

// ── How exact a pose must be ──────────────────────────────────────────────

/// How far off the metre grid x or z may be, metres (1 mm). The game's placement puts them
/// exactly on it; this only absorbs rounding on the way.
pub const GRID_TOLERANCE_M: f32 = 0.001;
/// How far a piece's footprint may differ from its blueprint's size, and its height from what
/// levelling allows, metres (1 mm).
pub const SIZE_TOLERANCE_M: f32 = 0.001;
/// How far a rotation's length may be from 1. A rotation is a unit quaternion; anything else
/// also stretches the piece.
pub const UNIT_TOLERANCE: f32 = 1.0e-3;
/// How far a piece may be turned from a whole quarter turn about the vertical, tilt included,
/// degrees.
pub const ANGLE_TOLERANCE_DEG: f32 = 0.5;
/// The least height levelling ever makes a piece, metres: the floor of `placement::level_top`'s
/// `.max(0.1)`.
pub const MIN_HEIGHT_M: f32 = 0.1;
/// Two boxes closer than this at every face are the same footprint, metres. MUST equal
/// placement.rs's private `SAME_BOX_M`, the rule `placement::occupied` uses in the player's own
/// home; [`tests::same_box_agrees_with_occupied`] fails if the two drift apart.
pub const SAME_BOX_M: f32 = 0.02;

// ── Why a request was refused ─────────────────────────────────────────────

/// What a refused request asked for (`game_build_refused`'s `action`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    /// A `game_build`.
    Build,
    /// A `game_unbuild` (taking a piece down).
    Unbuild,
    /// A `game_pieces_request`.
    Pieces,
}

/// Why the relay refused a request: the `reason` code a `game_build_refused` carries, exactly as
/// it goes on the wire (snake case: `NotInGame` is `"not_in_game"`). Listed in the order the
/// relay checks a build, cheapest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// The sender is not in the shared world (identified, but not joined).
    NotInGame,
    /// Too many requests too fast (see [`SEND_INTERVAL_MS`]).
    RateLimited,
    /// The message is malformed: a missing or wrong-sized field, a string over its cap, a number
    /// that is not a finite number.
    BadShape,
    /// A frame that is not a frame of this ship (a planet site, a corridor, a typo).
    BadFrame,
    /// The relay has no blueprint with that id.
    UnknownBlueprint,
    /// The blueprint exists but is not marked `shared: true`.
    NotShared,
    /// x or z is not on the metre grid.
    OffGrid,
    /// The rotation is not a whole quarter turn about the vertical (or not a rotation at all).
    BadTurn,
    /// The size is not the blueprint's, or a height levelling cannot make.
    BadScale,
    /// The piece reaches below or above the frame's sanity range of heights.
    OutOfBounds,
    /// The piece's footprint reaches past its frame's edge.
    OutsideFrame,
    /// The sender may not build or take down here; [`Why`] says why.
    NotAllowed,
    /// A piece with the same box already stands there.
    Occupied,
    /// The frame holds [`MAX_PIECES_PER_FRAME`] pieces already.
    FrameFull,
    /// The builder keeps [`MAX_PIECES_PER_OWNER`] pieces already.
    OwnerFull,
    /// The world holds [`MAX_PIECES_PER_WORLD`] pieces already.
    WorldFull,
    /// A take-down named a piece that is not there (already gone).
    NoSuchPiece,
    /// The relay could not write the change to its database, so nothing changed.
    StorageError,
    /// A code this game does not know (a newer server). Never sent.
    #[serde(other)]
    Unknown,
}

impl Reason {
    /// Every code the relay sends, in its checking order.
    pub const ALL: [Reason; 18] = [
        Reason::NotInGame,
        Reason::RateLimited,
        Reason::BadShape,
        Reason::BadFrame,
        Reason::UnknownBlueprint,
        Reason::NotShared,
        Reason::OffGrid,
        Reason::BadTurn,
        Reason::BadScale,
        Reason::OutOfBounds,
        Reason::OutsideFrame,
        Reason::NotAllowed,
        Reason::Occupied,
        Reason::FrameFull,
        Reason::OwnerFull,
        Reason::WorldFull,
        Reason::NoSuchPiece,
        Reason::StorageError,
    ];

    /// The code as it goes on the wire (and in the relay's log lines).
    pub fn as_str(self) -> &'static str {
        match self {
            Reason::NotInGame => "not_in_game",
            Reason::RateLimited => "rate_limited",
            Reason::BadShape => "bad_shape",
            Reason::BadFrame => "bad_frame",
            Reason::UnknownBlueprint => "unknown_blueprint",
            Reason::NotShared => "not_shared",
            Reason::OffGrid => "off_grid",
            Reason::BadTurn => "bad_turn",
            Reason::BadScale => "bad_scale",
            Reason::OutOfBounds => "out_of_bounds",
            Reason::OutsideFrame => "outside_frame",
            Reason::NotAllowed => "not_allowed",
            Reason::Occupied => "occupied",
            Reason::FrameFull => "frame_full",
            Reason::OwnerFull => "owner_full",
            Reason::WorldFull => "world_full",
            Reason::NoSuchPiece => "no_such_piece",
            Reason::StorageError => "storage_error",
            Reason::Unknown => "unknown",
        }
    }

    /// Why, in words that follow "Wood Wall not built: ". Every code that means the game sent
    /// what its own rules would not have sent (a pose off the grid, a frame the ship lacks) says
    /// the same thing, because to the player it is the same thing: two versions disagree.
    fn words(self) -> String {
        match self {
            Reason::NotInGame => "you are not in the shared world right now".into(),
            Reason::RateLimited => "too much was asked at once; it goes again in a moment".into(),
            Reason::BadShape
            | Reason::BadFrame
            | Reason::UnknownBlueprint
            | Reason::NotShared
            | Reason::OffGrid
            | Reason::BadTurn
            | Reason::BadScale
            | Reason::OutOfBounds
            | Reason::OutsideFrame
            | Reason::Unknown => DISAGREE.into(),
            Reason::NotAllowed => "this is not yours to change".into(),
            Reason::Occupied => "one already stands there".into(),
            Reason::FrameFull => {
                format!("this place already holds as many pieces as the server allows ({})", thousands(MAX_PIECES_PER_FRAME))
            }
            Reason::OwnerFull => format!(
                "you already keep as many pieces in the shared world as the server allows ({})",
                thousands(MAX_PIECES_PER_OWNER)
            ),
            Reason::WorldFull => {
                format!("the server already keeps as many pieces as it allows ({})", thousands(MAX_PIECES_PER_WORLD))
            }
            Reason::NoSuchPiece => "it is already gone".into(),
            Reason::StorageError => "the server could not save the change; try again".into(),
        }
    }
}

/// The words for every code that means this game and the server disagree.
const DISAGREE: &str = "this game and the server disagree about the piece; update whichever is older";

/// Why a request was `not_allowed`: the `why` code that comes with it, exactly as it goes on the
/// wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Why {
    /// Someone else's plot, and no household permit for it.
    NotYourPlot,
    /// A take-down by a permit holder of a piece someone else put up.
    NotYourPiece,
    /// The sender holds no plot on this ship (a guest) and brought no permit.
    Guest,
    /// The household permit has run out.
    PermitExpired,
    /// The permit's signer does not hold the plot (it may have changed hands).
    PermitNotFromHolder,
    /// The permit is malformed or its signature does not check out.
    PermitBad,
    /// The permit runs out more than [`PERMIT_MAX_DAYS`] days ahead.
    PermitTooLong,
    /// A shared space (`zone:*`), and the sender lacks the `can_edit_ship` rank.
    ShipRank,
    /// A code this game does not know (a newer server). Never sent.
    #[serde(other)]
    Unknown,
}

impl Why {
    /// Every code the relay sends.
    pub const ALL: [Why; 8] = [
        Why::NotYourPlot,
        Why::NotYourPiece,
        Why::Guest,
        Why::PermitExpired,
        Why::PermitNotFromHolder,
        Why::PermitBad,
        Why::PermitTooLong,
        Why::ShipRank,
    ];

    /// The code as it goes on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            Why::NotYourPlot => "not_your_plot",
            Why::NotYourPiece => "not_your_piece",
            Why::Guest => "guest",
            Why::PermitExpired => "permit_expired",
            Why::PermitNotFromHolder => "permit_not_from_holder",
            Why::PermitBad => "permit_bad",
            Why::PermitTooLong => "permit_too_long",
            Why::ShipRank => "ship_rank",
            Why::Unknown => "unknown",
        }
    }

    /// Why, in words that follow "Wood Wall not built: " or "Placing Wood Wall: ".
    fn words(self) -> String {
        match self {
            Why::NotYourPlot => {
                "this is someone else's plot; you build only on your own plot, or where its holder has given you a household permit"
                    .into()
            }
            Why::NotYourPiece => "it was put up by someone else, and only this plot's holder can take it down".into(),
            Why::Guest => "you are a guest on this ship, with no plot of your own to build on".into(),
            Why::PermitExpired => "your household permit for this plot has run out; ask its holder for a new one".into(),
            Why::PermitNotFromHolder => "this permit is not from the plot's holder (it may have changed hands)".into(),
            Why::PermitBad => "this household permit does not check out; ask the plot's holder for a new one".into(),
            Why::PermitTooLong => format!(
                "a household permit lasts at most {PERMIT_MAX_DAYS} days, and this one runs longer; ask the plot's holder for a new one"
            ),
            Why::ShipRank => "the ship's shared spaces are built by people this server has given that rank".into(),
            Why::Unknown => DISAGREE.into(),
        }
    }
}

/// Why a request was refused, in plain words: the clause that follows "Wood Wall not built: "
/// (a refusal, [`refusal_message`]) or "Placing Wood Wall: " (the crosshair, before anything is
/// sent). A `why` says more than its `reason`, so it wins. A take-down on someone else's plot
/// says so as a take-down.
pub fn why_words(action: Action, reason: Reason, why: Option<Why>) -> String {
    match (action, why) {
        (Action::Unbuild, Some(Why::NotYourPlot | Why::Guest)) => {
            "this is someone else's plot, and you cannot take down what stands on it".into()
        }
        (_, Some(w)) => w.words(),
        (_, None) => reason.words(),
    }
}

/// The whole sentence a refusal carries: the relay sends it as `game_build_refused`'s `message`,
/// and the game shows it (adding what came back, "6 Wood Plank back."). `piece` is the
/// blueprint's name ("Wood Wall"), or "piece" when it is not known.
pub fn refusal_message(action: Action, piece: &str, reason: Reason, why: Option<Why>) -> String {
    let words = why_words(action, reason, why);
    match action {
        Action::Build => format!("{piece} not built: {words}."),
        Action::Unbuild => format!("{piece} not taken down: {words}."),
        Action::Pieces => format!("The pieces here were not sent: {words}."),
    }
}

/// Why a piece the data does not mark shared cannot be built where the player aims, in a shared
/// world, outside their own home: "only foundations, roofs and walls can be built outside your
/// own home". The kinds are the categories of the blueprints `registry` marks shared, so a kind
/// added to basic.ron is named with no code change.
pub fn only_shell_pieces_words(registry: &BlueprintRegistry) -> String {
    let mut kinds: Vec<&str> = registry.blueprints.values().filter(|b| b.shared).map(|b| b.category.as_str()).collect();
    kinds.sort_unstable();
    kinds.dedup();
    let kinds: Vec<String> = kinds.iter().map(|k| format!("{k}s")).collect();
    match kinds.as_slice() {
        [] => "nothing can be built outside your own home".into(),
        [one] => format!("only {one} can be built outside your own home"),
        [rest @ .., last] => format!("only {} and {last} can be built outside your own home", rest.join(", ")),
    }
}

/// The placing hint's note on a piece that will be kept by the server: "[E] build here (kept by
/// the server: anyone near sees it)".
pub const KEPT_BY_THE_SERVER: &str = "kept by the server: anyone near sees it";
/// The placing hint's note, in a shared world, on a piece that stays in the player's own home.
pub const ONLY_IN_YOUR_HOME: &str = "only in your own home";

/// `n` with a comma between each three digits: 4096 is "4,096".
fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Why one of the rules below refused a pose: the codes for the wire, and a detail for the
/// relay's log and for tests (the player reads [`refusal_message`]'s words, never this).
#[derive(Debug, Clone, PartialEq)]
pub struct Refusal {
    pub reason: Reason,
    pub why: Option<Why>,
    pub detail: String,
}

impl Refusal {
    pub fn new(reason: Reason, detail: impl Into<String>) -> Self {
        Self { reason, why: None, detail: detail.into() }
    }

    /// A `not_allowed` refusal saying `why`.
    pub fn not_allowed(why: Why) -> Self {
        Self { reason: Reason::NotAllowed, why: Some(why), detail: why.as_str().to_string() }
    }
}

// ── The wire ──────────────────────────────────────────────────────────────

/// The `type` of every message, as the relay's dispatch (relay.rs) and the game's router
/// (engine/net_route.rs) match on it. [`tests::the_wire_messages_have_exactly_the_contract_fields`]
/// holds each to its [`ToRelay`] or [`FromRelay`] variant.
pub mod msg {
    /// Game to relay: build a piece.
    pub const BUILD: &str = "game_build";
    /// Game to relay: take a piece down.
    pub const UNBUILD: &str = "game_unbuild";
    /// Game to relay: send a frame's pieces again.
    pub const PIECES_REQUEST: &str = "game_pieces_request";
    /// Relay to the games with the frame in view: a piece went up.
    pub const BUILT: &str = "game_built";
    /// Relay to the games with the frame in view: a piece came down.
    pub const UNBUILT: &str = "game_unbuilt";
    /// Relay to one game: every piece in a frame (in parts).
    pub const PIECES: &str = "game_pieces";
    /// Relay to one game: a frame left your view; forget its pieces.
    pub const FRAME_OUT_OF_VIEW: &str = "game_frame_out_of_view";
    /// Relay to one game: your request was refused, and why.
    pub const BUILD_REFUSED: &str = "game_build_refused";
}

/// A household permit (ship homes section 4): a plot's holder lets one person build on that
/// plot until a date. Held by the grantee and sent with each build or take-down they make there;
/// the relay checks it without storing it (`relay::core::pq_crypto::verify_plot_permit`), the
/// friendship certificate's pattern. The relay rebuilds the signed words from what it knows (the
/// frame's plot, the sender's own id), so `plot` and `grantee` here only say what the permit
/// claims to be for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Permit {
    /// The plot holder's Dilithium3 public key, hex: whose signature this is.
    pub issuer: String,
    /// The plot it is for, by its ship-file id (`p3`, not `plot:p3`).
    pub plot: String,
    /// Who it lets build: the grantee's id as a relay holds plots under it (`did:hum:...`,
    /// `relay::storage::plots::plot_owner_id`).
    pub grantee: String,
    /// When it runs out, Unix seconds; never more than [`PERMIT_MAX_DAYS`] days ahead.
    pub expiry: u64,
    /// The issuer's Dilithium3 signature over `hum/permit/v1\n{plot}\n{grantee}\n{expiry}`,
    /// base64.
    pub sig: String,
}

/// What this player may do beyond their own plot, as the welcome's `ranks` field tells it
/// (absent fields read false).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ranks {
    /// Builds in the ship's shared spaces through the server: the `can_edit_ship` rank (the
    /// built-in Admin role has it; the server's owner always does).
    #[serde(default)]
    pub can_edit_ship: bool,
    /// Takes down any piece anywhere: the server's admins and its owner.
    #[serde(default)]
    pub take_down_any: bool,
}

/// One piece as it travels (`game_built`, `game_pieces`). The relay fills every field; the game
/// never chooses the id or the time. The frame is the message's, and the pose is measured from
/// its corner. There is no builder: see "nobody is told who built what" at the top.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Piece {
    /// The relay's number for this piece. Never reused, even after the piece is taken down or
    /// the relay restarts (SQLite AUTOINCREMENT).
    pub piece_id: u64,
    /// Which blueprint (data/blueprints/basic.ron).
    pub blueprint_id: String,
    /// Where it stands, metres from the frame's corner, y up: the BOTTOM centre of its box (the
    /// renderer's convention, placement.rs). On the metre grid in ship metres ([`canonicalise`]).
    pub position: [f32; 3],
    /// Its turn as a quaternion (x, y, z, w): exactly a whole quarter turn about the vertical.
    pub rotation: [f32; 4],
    /// Its size, metres: x and z the blueprint's; y may differ by up to
    /// `placement::LEVEL_TOLERANCE_M`, because walls on uneven ground are made taller or shorter
    /// so their tops meet (`placement::level_top`).
    pub scale: [f32; 3],
    /// When the relay accepted it, Unix seconds on the relay's clock (the clock of
    /// `game_time_sync`'s `server_time`). A game that sees the piece later grows its scaffold
    /// from how long ago this was.
    pub placed_at: f64,
    /// The player this copy is sent to built it. Only ever true in that player's own copies;
    /// absent (false) in everyone else's.
    #[serde(default, skip_serializing_if = "is_false")]
    pub mine: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl Piece {
    /// The pose, measured from the frame's corner, as the ECS transform.
    pub fn local_pose(&self) -> Transform {
        Transform {
            position: Vec3::from_array(self.position),
            rotation: Quat::from_array(self.rotation),
            scale: Vec3::from_array(self.scale),
        }
    }

    /// Store `pose` (measured from the frame's corner) in the wire fields.
    pub fn set_local_pose(&mut self, pose: &Transform) {
        self.position = pose.position.to_array();
        self.rotation = pose.rotation.to_array();
        self.scale = pose.scale.to_array();
    }

    /// The ECS marker for this piece in `frame`.
    pub fn marker(&self, frame: &str) -> SharedPiece {
        SharedPiece { piece_id: self.piece_id, frame: frame.to_string(), mine: self.mine }
    }
}

/// A message from a game to the relay. JSON with its `type` ([`msg`]); every pose is measured
/// from the frame's corner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToRelay {
    /// Build a piece. `req_id` is the game's own number for the request, echoed in the answer.
    #[serde(rename = "game_build")]
    Build {
        req_id: u32,
        frame: String,
        blueprint_id: String,
        position: [f32; 3],
        rotation: [f32; 4],
        scale: [f32; 3],
        /// Only when building on someone else's plot.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        permit: Option<Permit>,
    },
    /// Take a piece down.
    #[serde(rename = "game_unbuild")]
    Unbuild {
        req_id: u32,
        piece_id: u64,
        /// Only when taking down your own piece on someone else's plot.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        permit: Option<Permit>,
    },
    /// Send this frame's pieces again (after a gap in its `seq`).
    #[serde(rename = "game_pieces_request")]
    PiecesRequest { frame: String },
}

/// A message from the relay to a game. JSON with its `type` ([`msg`]). `seq` counts each frame's
/// changes: up by exactly one per piece built or taken down there, from 0 when the relay starts
/// (every rejoin brings fresh lists, so the restart is harmless).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FromRelay {
    /// A piece went up in `frame`. To every player with the frame in view; the builder's copy
    /// carries their `req_id` and `piece.mine`.
    #[serde(rename = "game_built")]
    Built {
        frame: String,
        seq: u64,
        /// The relay's clock now, Unix seconds, to age `piece.placed_at` by.
        server_time: f64,
        piece: Piece,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        req_id: Option<u32>,
    },
    /// A piece came down in `frame`. To every player with the frame in view; the remover's copy
    /// carries their `req_id`.
    #[serde(rename = "game_unbuilt")]
    Unbuilt {
        frame: String,
        seq: u64,
        piece_id: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        req_id: Option<u32>,
    },
    /// Every piece in `frame` as of `seq`, to one player: when the frame comes into view, on a
    /// join, or asked for. At most [`PIECES_PER_PART`] per message, parts numbered from 1; an
    /// empty frame is one part with no pieces.
    #[serde(rename = "game_pieces")]
    Pieces { frame: String, seq: u64, server_time: f64, part: u32, parts: u32, pieces: Vec<Piece> },
    /// `frame` left this player's view: forget its pieces until it comes back.
    #[serde(rename = "game_frame_out_of_view")]
    FrameOutOfView { frame: String },
    /// A request was refused, and nothing changed.
    #[serde(rename = "game_build_refused")]
    Refused {
        /// The request's `req_id`, when it had one the relay could read.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        req_id: Option<u32>,
        action: Action,
        reason: Reason,
        /// Only with `not_allowed`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        why: Option<Why>,
        /// [`refusal_message`]'s sentence.
        message: String,
    },
}

// ── The game's side ───────────────────────────────────────────────────────

/// The ECS marker on a piece the server keeps. A built piece WITHOUT it is the player's own, kept
/// in their save as always. One WITH it:
/// - stays out of the player's save, and a save being loaded leaves it alone;
/// - never gets a `Structure.uid` (so `built:{uid}` stays the player's own namespace);
/// - earns the quest event, XP and sound when it finishes only when `mine`;
/// - is taken down by asking the relay, never by despawning it locally;
/// - is never carried along when the player's home moves to another plot.
#[derive(Debug, Clone, PartialEq)]
pub struct SharedPiece {
    /// The relay's [`Piece::piece_id`].
    pub piece_id: u64,
    /// The frame it is kept in (`plot:p3`).
    pub frame: String,
    /// This player built it ([`Piece::mine`]).
    pub mine: bool,
}

/// What a shared build took from the builder, so a refusal gives back exactly that: (item id,
/// count) taken from the pack, and from the home's storage.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Spent {
    pub pack: Vec<(String, u32)>,
    pub storage: Vec<(String, u32)>,
}

/// A shared build the player has asked for and paid for, waiting in [`OUT_CHANNEL`] for the
/// engine to send it as a `game_build`.
#[derive(Debug, Clone)]
pub struct SharedBuildIntent {
    /// The frame it goes to (`plot:p3`), from the build request's `shared_frame`.
    pub frame: String,
    pub blueprint_id: String,
    /// The ghost's pose, in SHIP metres (`BuildFrame::to_local` makes the wire's pose).
    pub pose: Transform,
    /// Exactly what was taken for it.
    pub spent: Spent,
}

// ── The rules ─────────────────────────────────────────────────────────────

/// The whole quarter turn about the vertical nearest to `q` (0 to 3), and how far `q` is from it
/// as an angle in radians, tilt included. Either sign of the quaternion works (q and -q are the
/// same turn).
fn nearest_quarter_turn(q: Quat) -> (u8, f32) {
    let len = q.length();
    let q = if len.is_finite() && len > 0.0 { q / len } else { Quat::IDENTITY };
    let mut best = (0u8, f32::INFINITY);
    for t in 0..4u8 {
        // The rotation that takes the quarter turn to q; its angle is the gap between them.
        let d = quarter_turn(t).conjugate() * q;
        let gap = 2.0 * Vec3::new(d.x, d.y, d.z).length().atan2(d.w.abs());
        if gap < best.1 {
            best = (t, gap);
        }
    }
    best
}

/// Is `v` more than [`GRID_TOLERANCE_M`] off the metre grid?
fn off_the_grid(v: f32) -> bool {
    (v - (v / GRID_M).round() * GRID_M).abs() > GRID_TOLERANCE_M
}

/// Could the game's own placement (`placement::placement_pose`, what the ghost shows and E
/// builds) have made `pose` for `bp`? `pose` is in SHIP metres, where the build grid is. The
/// relay asks this of every shared build, and the game's tests ask it of every pose the
/// placement makes, so the two cannot disagree. Checked plainest first:
/// 1. every number is a real number (no NaN, no infinity): `bad_shape`;
/// 2. the rotation is a rotation (unit length within [`UNIT_TOLERANCE`]), and a whole quarter
///    turn about the vertical within [`ANGLE_TOLERANCE_DEG`], tilt included: `bad_turn`;
/// 3. x and z on the metre grid within [`GRID_TOLERANCE_M`]: `off_grid`;
/// 4. the footprint the blueprint's (within [`SIZE_TOLERANCE_M`]), and the height one levelling
///    can make: the blueprint's, give or take `LEVEL_TOLERANCE_M`, never under
///    [`MIN_HEIGHT_M`]: `bad_scale`.
///
/// It looks at no other piece and no frame: whether the spot is free is [`same_box`], and
/// whether it lies in its frame is [`box_inside_frame`].
pub fn validate_pose(bp: &Blueprint, pose: &Transform) -> Result<(), Refusal> {
    let (p, r, s) = (pose.position, pose.rotation, pose.scale);
    // 1. A NaN slips through every comparison below (NaN > limit is false), so it is caught
    // first, by itself.
    if [p.x, p.y, p.z, r.x, r.y, r.z, r.w, s.x, s.y, s.z].iter().any(|v| !v.is_finite()) {
        return Err(Refusal::new(Reason::BadShape, "a position, turn or size that is not a finite number"));
    }
    // 2.
    if (r.length() - 1.0).abs() > UNIT_TOLERANCE {
        return Err(Refusal::new(Reason::BadTurn, format!("a rotation of length {}, not 1", r.length())));
    }
    let (_, gap) = nearest_quarter_turn(r);
    if gap > ANGLE_TOLERANCE_DEG.to_radians() {
        return Err(Refusal::new(
            Reason::BadTurn,
            format!("{:.2} degrees from a whole quarter turn about the vertical", gap.to_degrees()),
        ));
    }
    // 3.
    if off_the_grid(p.x) || off_the_grid(p.z) {
        return Err(Refusal::new(Reason::OffGrid, format!("x {} or z {} is off the {GRID_M} m grid", p.x, p.z)));
    }
    // 4. Footprint, then height. Levelling (placement::level_top) makes a piece at most
    // LEVEL_TOLERANCE_M taller or shorter than its blueprint, and never shorter than MIN_HEIGHT_M.
    let [sx, sy, sz] = bp.size;
    if (s.x - sx).abs() > SIZE_TOLERANCE_M || (s.z - sz).abs() > SIZE_TOLERANCE_M {
        return Err(Refusal::new(
            Reason::BadScale,
            format!("{} by {} m across where the {} is {sx} by {sz} m", s.x, s.z, bp.name),
        ));
    }
    let lowest = (sy - LEVEL_TOLERANCE_M).max(MIN_HEIGHT_M);
    let highest = sy + LEVEL_TOLERANCE_M;
    if s.y < lowest - SIZE_TOLERANCE_M || s.y > highest + SIZE_TOLERANCE_M {
        return Err(Refusal::new(
            Reason::BadScale,
            format!("{} m tall where levelling makes the {} {lowest} to {highest} m", s.y, bp.name),
        ));
    }
    Ok(())
}

/// The exact pose the relay keeps and sends for a pose that passed [`validate_pose`], in SHIP
/// metres like its input: x and z exactly on the grid, the rotation exactly the quarter turn the
/// placement makes (`placement::quarter_turn`), the footprint exactly the blueprint's. The
/// height (y, and a levelled scale.y) is kept as sent: it comes from the floor and from
/// levelling, not from a grid. So every game holds the same numbers, and a pose the game's
/// placement made comes back as it was, which is how a game recognises its own build in the
/// relay's list. A zero is always a plain zero, never a negative one.
pub fn canonicalise(bp: &Blueprint, pose: &Transform) -> Transform {
    // `+ 0.0` turns a negative zero (-0.3 rounds to -0.0) into a plain zero.
    let snap = |v: f32| (v / GRID_M).round() * GRID_M + 0.0;
    let (turns, _) = nearest_quarter_turn(pose.rotation);
    Transform {
        position: Vec3::new(snap(pose.position.x), pose.position.y + 0.0, snap(pose.position.z)),
        rotation: quarter_turn(turns),
        scale: Vec3::new(bp.size[0], pose.scale.y + 0.0, bp.size[2]),
    }
}

/// Do `a` and `b` cover the same box, every face within [`SAME_BOX_M`]? A second piece with the
/// same box would be the same piece built twice (materials spent twice for what looks like one
/// wall), so the relay refuses it as `occupied`: the rule `placement::occupied` applies in the
/// player's own home. Both poses must be in the same frame's metres (the relay compares only
/// pieces of one frame).
pub fn same_box(a: &Transform, b: &Transform) -> bool {
    let (a0, a1) = world_aabb(a);
    let (b0, b1) = world_aabb(b);
    (b0 - a0).abs().max_element() < SAME_BOX_M && (b1 - a1).abs().max_element() < SAME_BOX_M
}

/// Does `local` (a pose measured from `frame`'s corner) lie in `frame`: all its turned footprint
/// on the frame's floor, give or take `build_frames::EDGE_SLACK_M` (`outside_frame`), and its
/// box within the frame's sanity range of heights (`out_of_bounds`)? The one rule the relay and
/// the game's placing gate both apply (src/ship/build_frames.rs).
pub fn box_inside_frame(frame: &BuildFrame, local: &Transform) -> Result<(), Refusal> {
    if !frame.footprint_inside(local) {
        return Err(Refusal::new(
            Reason::OutsideFrame,
            format!("the footprint reaches more than {EDGE_SLACK_M} m past the edge of {}", frame.id),
        ));
    }
    if !frame.height_inside(local) {
        return Err(Refusal::new(
            Reason::OutOfBounds,
            format!("the box reaches outside {MIN_LOCAL_Y_M} to {MAX_LOCAL_Y_M} m over the floor of {}", frame.id),
        ));
    }
    Ok(())
}

/// The pose checks the relay runs on a build, in its order, and the pose it then keeps: `local`
/// (as a `game_build` carries it, measured from `frame`'s corner) must be a pose the placement
/// can make, checked in ship metres where the grid is ([`validate_pose`]); it is made exact
/// ([`canonicalise`]); and the exact pose must lie in the frame ([`box_inside_frame`]). Ok holds
/// the exact pose, measured from the frame's corner, ready to store and send.
pub fn pose_in_frame(bp: &Blueprint, frame: &BuildFrame, local: &Transform) -> Result<Transform, Refusal> {
    let ship = frame.to_ship(local);
    validate_pose(bp, &ship)?;
    let exact = frame.to_local(&canonicalise(bp, &ship));
    box_inside_frame(frame, &exact)?;
    Ok(exact)
}

#[cfg(test)]
mod tests {
    use super::super::placement::{occupied, placement_pose};
    use super::super::{Construction, PlanetSite, Structure};
    use super::*;
    use crate::ship::build_frames::BuildFrames;
    use crate::ship::ship_structure::ShipStructure;

    fn shipped() -> BlueprintRegistry {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("blueprints").join("basic.ron");
        BlueprintRegistry::from_ron(&std::fs::read(path).unwrap()).unwrap()
    }

    /// The relay's frames of the shipped ship.
    fn frames() -> BuildFrames {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        BuildFrames::of_ship(&ShipStructure::ship_for_relay(&data).expect("the ship file loads"))
    }

    /// The blueprints marked shared, in a fixed order (the registry is a HashMap).
    fn shared_blueprints(reg: &BlueprintRegistry) -> Vec<&Blueprint> {
        let mut out: Vec<&Blueprint> = reg.blueprints.values().filter(|b| b.shared).collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    /// Place `id` aimed at `at` with `turns` the way the ghost does, and finish it in `world` the
    /// way the ConstructionSystem does, ship metres.
    fn build(world: &mut hecs::World, reg: &BlueprintRegistry, id: &str, at: Vec3, turns: u8) -> Transform {
        let bp = reg.get(id).unwrap_or_else(|| panic!("{id} in basic.ron"));
        let tf = placement_pose(bp, at, turns, world, reg, None);
        world.spawn((
            tf.clone(),
            Structure { blueprint_id: id.into(), health: bp.health, max_health: bp.health, provides: bp.provides.clone(), uid: 0 },
        ));
        tf
    }

    /// The validator accepts `pose`, and canonicalising it changes nothing.
    fn assert_placer_pose_passes(bp: &Blueprint, pose: &Transform, what: &str) {
        if let Err(r) = validate_pose(bp, pose) {
            panic!("{} {what}: the placer made {pose:?} and the validator refused it: {r:?}", bp.id);
        }
        let c = canonicalise(bp, pose);
        assert!(
            c.position == pose.position && c.rotation == pose.rotation && c.scale == pose.scale,
            "{} {what}: canonicalising the placer's pose moved it: {pose:?} -> {c:?}",
            bp.id
        );
    }

    /// THE PLACER AND THE VALIDATOR CANNOT DISAGREE. Every pose the game's own placement
    /// (`placement_pose`: the ghost, and what E builds) makes for every shareable blueprint
    /// passes the relay's `validate_pose`, and `canonicalise` leaves it as it was, so a game
    /// recognises its own build in the relay's list by its box. And the whole check the relay
    /// runs on a `game_build` (`pose_in_frame`, from the pose measured from plot p2's corner)
    /// gives back exactly that pose. Covered: all four turns; grid points, off-grid aims and
    /// aims near zero; three floor heights; an empty world; a stacked room on p2 (walls on a
    /// foundation at 0.2 m, a roof on them at 3.2 m); and uneven ground, where levelling makes
    /// walls taller and shorter and a foundation as thin as levelling allows (0.1 m).
    /// Seen red 2026-10-05 with the quarter-turn check inverted (refusing a pose within the
    /// tolerance instead of past it): "metal_wall empty world, aim (0, 0, 0), 0 turns: the placer
    /// made Transform { position: Vec3(0.0, 0.0, 0.0), rotation: Quat(0.0, 0.0, 0.0, 1.0), scale:
    /// Vec3(4.0, 3.0, 0.15) } and the validator refused it: Refusal { reason: BadTurn, why: None,
    /// detail: "0.00 degrees from a whole quarter turn about the vertical" }" (and
    /// `validate_pose_refuses_and_canonicalises` went red with it).
    #[test]
    fn every_shareable_pose_the_placer_makes_passes_validate_pose() {
        let reg = shipped();
        let shared = shared_blueprints(&reg);
        let ids: Vec<&str> = shared.iter().map(|b| b.id.as_str()).collect();
        assert!(ids.contains(&"wood_foundation") && ids.contains(&"wood_wall") && ids.contains(&"roof"), "{ids:?}");
        let frames = frames();
        let p2 = frames.get("plot:p2").expect("plot p2");

        // An empty world, anywhere: validate_pose looks at no frame.
        let empty = hecs::World::new();
        let aims = [(0.0, 0.0), (3.4, -7.6), (-12.49, 5.51), (-0.3, 0.2), (731.4, -999.3), (20.6, 140.4)];
        let mut checked = 0;
        for bp in &shared {
            for turns in 0..4u8 {
                for (x, z) in aims {
                    for floor in [0.0_f32, -3.5, 12.25] {
                        let pose = placement_pose(bp, Vec3::new(x, floor, z), turns, &empty, &reg, None);
                        assert_placer_pose_passes(bp, &pose, &format!("empty world, aim ({x}, {floor}, {z}), {turns} turns"));
                        checked += 1;
                    }
                }
                // Inside plot p2, the relay's whole check gives back the placer's pose.
                for (x, z) in [(20.6, 140.4), (8.0, 110.0), (44.5, 170.49)] {
                    let pose = placement_pose(bp, Vec3::new(x, 0.0, z), turns, &empty, &reg, None);
                    let local = p2.to_local(&pose);
                    match pose_in_frame(bp, p2, &local) {
                        Ok(exact) => assert!(
                            exact.position == local.position && exact.rotation == local.rotation && exact.scale == local.scale,
                            "{} on p2 at ({x}, {z}), {turns} turns: {local:?} came back as {exact:?}",
                            bp.id
                        ),
                        Err(r) => panic!("{} on p2 at ({x}, {z}), {turns} turns: refused {r:?}", bp.id),
                    }
                }
            }
        }
        assert_eq!(checked, shared.len() * 4 * aims.len() * 3);

        // Stacked, on p2: a foundation, four walls on it, a roof on them.
        let mut room = hecs::World::new();
        let c = Vec3::new(20.0, 0.0, 140.0);
        build(&mut room, &reg, "wood_foundation", c, 0);
        build(&mut room, &reg, "wood_wall", c + Vec3::new(0.0, 0.0, -2.0), 0);
        build(&mut room, &reg, "wood_wall", c + Vec3::new(0.0, 0.0, 2.0), 0);
        build(&mut room, &reg, "wood_wall", c + Vec3::new(-2.0, 0.0, 0.0), 1);
        let wall = build(&mut room, &reg, "wood_wall", c + Vec3::new(2.0, 0.0, 0.0), 1);
        let roof = build(&mut room, &reg, "roof", c, 0);
        assert!((wall.position.y - 0.2).abs() < 1e-5 && (roof.position.y - 3.2).abs() < 1e-5, "the room really is stacked");
        let mut stacked_seen = 0;
        for bp in &shared {
            for turns in 0..4u8 {
                for (dx, floor, dz) in [(0.0, 0.0, -2.0), (2.0, 0.0, 0.0), (0.4, 0.0, 0.3), (0.0, 3.2, 0.0)] {
                    let pose = placement_pose(bp, c + Vec3::new(dx, floor, dz), turns, &room, &reg, None);
                    if pose.position.y != floor {
                        stacked_seen += 1;
                    }
                    let what = format!("stacked room, aim ({dx}, {floor}, {dz}) from its middle, {turns} turns");
                    assert_placer_pose_passes(bp, &pose, &what);
                    let local = p2.to_local(&pose);
                    let exact = pose_in_frame(bp, p2, &local).unwrap_or_else(|r| panic!("{} {what}: refused {r:?}", bp.id));
                    assert!(exact.position == local.position && exact.scale == local.scale, "{} {what}: {exact:?}", bp.id);
                }
            }
        }
        assert!(stacked_seen > 0, "some poses really rested on another piece");

        // Uneven ground: levelling stretches and shrinks pieces.
        let mut uneven = hecs::World::new();
        build(&mut uneven, &reg, "wood_wall", Vec3::new(10.0, 0.0, -2.0), 0);
        let taller = build(&mut uneven, &reg, "wood_wall", Vec3::new(8.0, -0.25, 0.0), 1);
        let shorter = build(&mut uneven, &reg, "wood_wall", Vec3::new(12.0, 0.25, 0.0), 1);
        build(&mut uneven, &reg, "wood_foundation", Vec3::new(20.0, 0.0, 0.0), 0);
        let thin = build(&mut uneven, &reg, "wood_foundation", Vec3::new(23.0, 0.25, 0.0), 0);
        let thick = build(&mut uneven, &reg, "wood_foundation", Vec3::new(20.0, -0.25, 3.0), 0);
        let (wall_bp, found_bp) = (reg.get("wood_wall").unwrap(), reg.get("wood_foundation").unwrap());
        assert!((taller.scale.y - 3.25).abs() < 1e-4, "levelled taller: {}", taller.scale.y);
        assert!((shorter.scale.y - 2.75).abs() < 1e-4, "levelled shorter: {}", shorter.scale.y);
        assert_eq!(thin.scale.y, MIN_HEIGHT_M, "levelled down to the least height");
        assert!((thick.scale.y - 0.45).abs() < 1e-4, "levelled taller: {}", thick.scale.y);
        assert_placer_pose_passes(wall_bp, &taller, "levelled taller");
        assert_placer_pose_passes(wall_bp, &shorter, "levelled shorter");
        assert_placer_pose_passes(found_bp, &thin, "levelled to the least height");
        assert_placer_pose_passes(found_bp, &thick, "levelled taller");
    }

    /// WHAT THE RULES REFUSE, AND THE POSE THEY KEEP. From a proper pose (a wood wall on the
    /// grid, turned a quarter), each change is refused with its own code and each change inside
    /// the tolerances accepted: off the grid by 30 cm and 2 mm (half a millimetre passes);
    /// tilted 2 degrees (0.3 passes); turned 45 or 91 degrees (90.3 passes); the other sign of
    /// the same turn (passes); a rotation 1% too long; NaN or infinity in the position, the turn
    /// or the size; the wrong footprint; a height levelling cannot make (a foundation 1 cm under
    /// its least, a wall 3.5 m or 2 m tall); levelled heights that it can make. A pose a hair off
    /// is kept as exactly the placer's, every turn, with plain zeros. In a frame
    /// (`pose_in_frame`, on p2): the exact pose comes back measured from p2's corner; a
    /// foundation over p2's line is `outside_frame`; one sunk under the deck or lifted past 40 m
    /// is `out_of_bounds`; a bad pose is refused for its pose before its frame is looked at.
    /// Seen red 2026-10-05 with the grid check removed from `validate_pose`: `["30 cm off the
    /// grid: want Some(OffGrid), got None", "2 mm off the grid on z: want Some(OffGrid), got
    /// None"]`.
    #[test]
    fn validate_pose_refuses_and_canonicalises() {
        let reg = shipped();
        let bp = reg.get("wood_wall").unwrap();
        let base = placement_pose(bp, Vec3::new(3.0, 0.0, -2.0), 1, &hecs::World::new(), &reg, None);
        assert_eq!(validate_pose(bp, &base), Ok(()), "the starting pose is a proper one");
        let with = |f: &dyn Fn(&mut Transform)| {
            let mut t = base.clone();
            f(&mut t);
            t
        };
        let turned = |deg: f32| Quat::from_rotation_y(deg.to_radians());
        let cases: Vec<(&str, Transform, Option<Reason>)> = vec![
            ("30 cm off the grid", with(&|t| t.position.x += 0.3), Some(Reason::OffGrid)),
            ("2 mm off the grid on z", with(&|t| t.position.z += 0.002), Some(Reason::OffGrid)),
            ("half a millimetre off the grid", with(&|t| t.position.x += 0.0005), None),
            ("tilted 2 degrees", with(&|t| t.rotation = t.rotation * Quat::from_rotation_x(2f32.to_radians())), Some(Reason::BadTurn)),
            ("tilted 0.3 degrees", with(&|t| t.rotation = t.rotation * Quat::from_rotation_z(0.3f32.to_radians())), None),
            ("turned 45 degrees", with(&|t| t.rotation = turned(45.0)), Some(Reason::BadTurn)),
            ("turned 91 degrees", with(&|t| t.rotation = turned(91.0)), Some(Reason::BadTurn)),
            ("turned 90.3 degrees", with(&|t| t.rotation = turned(90.3)), None),
            ("the same turn, the other sign", with(&|t| t.rotation = -t.rotation), None),
            ("a rotation 1% too long", with(&|t| t.rotation = t.rotation * 1.01), Some(Reason::BadTurn)),
            ("NaN in the position", with(&|t| t.position.y = f32::NAN), Some(Reason::BadShape)),
            ("NaN in the rotation", with(&|t| t.rotation = Quat::from_xyzw(0.0, f32::NAN, 0.0, 1.0)), Some(Reason::BadShape)),
            ("NaN in the size", with(&|t| t.scale.x = f32::NAN), Some(Reason::BadShape)),
            ("infinity in the position", with(&|t| t.position.x = f32::INFINITY), Some(Reason::BadShape)),
            ("a 3 m long wall", with(&|t| t.scale.x = 3.0), Some(Reason::BadScale)),
            ("a 25 cm thick wall", with(&|t| t.scale.z = 0.25), Some(Reason::BadScale)),
            ("a wall 3.5 m tall", with(&|t| t.scale.y = 3.5), Some(Reason::BadScale)),
            ("a wall 2 m tall", with(&|t| t.scale.y = 2.0), Some(Reason::BadScale)),
            ("a wall 5 cm tall", with(&|t| t.scale.y = 0.05), Some(Reason::BadScale)),
            ("a wall levelled to 3.29 m", with(&|t| t.scale.y = 3.29), None),
            ("a wall levelled to 2.71 m", with(&|t| t.scale.y = 2.71), None),
        ];
        let mut wrong = Vec::new();
        for (what, pose, want) in &cases {
            let got = validate_pose(bp, pose).err().map(|r| r.reason);
            if got != *want {
                wrong.push(format!("{what}: want {want:?}, got {got:?}"));
            }
        }
        // A foundation (0.2 m): levelling makes it 0.1 to 0.5 m, never less than 0.1.
        let found = reg.get("wood_foundation").unwrap();
        let slab = |h: f32| Transform { scale: Vec3::new(4.0, h, 4.0), ..placement_pose(found, Vec3::ZERO, 0, &hecs::World::new(), &reg, None) };
        for (h, want) in [(0.1, None), (0.09, Some(Reason::BadScale)), (0.5, None), (0.52, Some(Reason::BadScale))] {
            let got = validate_pose(found, &slab(h)).err().map(|r| r.reason);
            if got != want {
                wrong.push(format!("a foundation {h} m thick: want {want:?}, got {got:?}"));
            }
        }
        assert!(wrong.is_empty(), "{wrong:#?}");

        // A pose a hair off is kept as exactly the placer's, every turn.
        for turns in 0..4u8 {
            let exact = placement_pose(bp, Vec3::new(3.0, 0.2, -2.0), turns, &hecs::World::new(), &reg, None);
            let mut sent = exact.clone();
            sent.position.x += 0.0004;
            sent.position.z -= 0.0007;
            sent.rotation = -(exact.rotation * Quat::from_rotation_y(0.3f32.to_radians()));
            sent.scale.x += 0.0006;
            assert_eq!(validate_pose(bp, &sent), Ok(()), "{turns} turns: a hair off still passes");
            let c = canonicalise(bp, &sent);
            assert!(
                c.position == exact.position && c.rotation == exact.rotation && c.scale == exact.scale,
                "{turns} turns: {c:?} is not the placer's {exact:?}"
            );
        }
        let mut near_zero = placement_pose(bp, Vec3::ZERO, 0, &hecs::World::new(), &reg, None);
        near_zero.position.x = -0.0004;
        near_zero.position.z = -0.0;
        near_zero.position.y = -0.0;
        let c = canonicalise(bp, &near_zero);
        assert_eq!(c.position.to_array().map(f32::to_bits), [0, 0, 0], "plain zeros: {c:?}");

        // In a frame: plot p2 (corner (0, 0, 99), 55 x 89 m).
        let frames = frames();
        let p2 = frames.get("plot:p2").unwrap();
        let ship = placement_pose(bp, Vec3::new(30.0, 0.0, 140.0), 1, &hecs::World::new(), &reg, None);
        let mut sent = p2.to_local(&ship);
        sent.position.x += 0.0004;
        let kept = pose_in_frame(bp, p2, &sent).expect("a proper wall in p2's yard");
        assert_eq!(kept.position, Vec3::new(30.0, 0.0, 41.0), "kept exact, measured from p2's corner");
        assert_eq!(kept.rotation, quarter_turn(1));
        let over_the_line = p2.to_local(&placement_pose(found, Vec3::new(20.0, 0.0, 98.0), 0, &hecs::World::new(), &reg, None));
        // (Transform has no PartialEq, so the refusals are compared by their codes.)
        let refused = |b: &Blueprint, local: &Transform| pose_in_frame(b, p2, local).err().map(|r| r.reason);
        assert_eq!(refused(found, &over_the_line), Some(Reason::OutsideFrame));
        let sunk = Transform { position: Vec3::new(30.0, -1.5, 41.0), ..kept.clone() };
        let lifted = Transform { position: Vec3::new(30.0, 38.0, 41.0), ..kept.clone() };
        assert_eq!(refused(bp, &sunk), Some(Reason::OutOfBounds));
        assert_eq!(refused(bp, &lifted), Some(Reason::OutOfBounds), "a 3 m wall standing at 38 m reaches 41 m");
        let tilted_and_outside = Transform { rotation: turned(45.0), ..over_the_line };
        assert_eq!(refused(found, &tilted_and_outside), Some(Reason::BadTurn), "the pose first");
    }

    /// `same_box` is the rule `placement::occupied` uses: for a wall in the home and a set of
    /// candidate poses (the same box; moved 1.5 cm and 2.5 cm sideways and up; turned; a thicker
    /// wall in the same place; a wall levelled 1 cm and 3 cm taller) the two give the same
    /// answer, and the answer is the one written down. `occupied` looks at one frame only, as
    /// the relay compares only one frame's pieces.
    /// Seen red 2026-10-05 with this file's `SAME_BOX_M` changed to 0.03 m: `["2.5 cm sideways:
    /// same_box true, occupied false, want false", "2.5 cm up: same_box true, occupied false,
    /// want false", "3 cm taller: same_box true, occupied false, want false"]`.
    #[test]
    fn same_box_agrees_with_occupied() {
        let reg = shipped();
        let mut world = hecs::World::new();
        let wall = build(&mut world, &reg, "wood_wall", Vec3::new(0.0, 0.0, 2.0), 0);
        let moved = |dx: f32, dy: f32| Transform { position: wall.position + Vec3::new(dx, dy, 0.0), ..wall.clone() };
        let taller = |h: f32| Transform { scale: Vec3::new(wall.scale.x, h, wall.scale.z), ..wall.clone() };
        let empty = hecs::World::new();
        let cases: Vec<(&str, Transform, bool)> = vec![
            ("the same box", wall.clone(), true),
            ("1.5 cm sideways", moved(0.015, 0.0), true),
            ("2.5 cm sideways", moved(0.025, 0.0), false),
            ("1.5 cm up", moved(0.0, 0.015), true),
            ("2.5 cm up", moved(0.0, 0.025), false),
            ("turned", placement_pose(reg.get("wood_wall").unwrap(), Vec3::new(0.0, 0.0, 2.0), 1, &empty, &reg, None), false),
            ("a stone wall there", placement_pose(reg.get("stone_wall").unwrap(), Vec3::new(0.0, 0.0, 2.0), 0, &empty, &reg, None), false),
            ("1 cm taller", taller(3.01), true),
            ("3 cm taller", taller(3.03), false),
        ];
        let mut wrong = Vec::new();
        for (what, cand, want) in &cases {
            let (sb, occ) = (same_box(&wall, cand), occupied(&world, cand, None));
            if sb != occ || sb != *want {
                wrong.push(format!("{what}: same_box {sb}, occupied {occ}, want {want}"));
            }
        }
        assert!(wrong.is_empty(), "{wrong:#?}");
        // A scaffold still going up counts for occupied, and same_box does not care which it is.
        let mut scaffolds = hecs::World::new();
        scaffolds.spawn((wall.clone(), Construction { blueprint_id: "wood_wall".into(), progress: 0.0, build_time: 4.0, builder_key: None }));
        assert!(occupied(&scaffolds, &moved(0.015, 0.0), None) && same_box(&wall, &moved(0.015, 0.0)));
        let site = PlanetSite { body: "earth".into(), origin: glam::DVec3::new(0.0, 6.371e6, 0.0) };
        assert!(!occupied(&world, &wall, Some(&site)), "another frame");
    }

    /// Why `bp` must NOT be marked shared, or None when its pose is its whole state. Each is
    /// state the relay does not keep: the recipes it serves and their power, the power it makes,
    /// a door open or shut, contents filed under `built:{uid}`, a bed's rest, a fire's fuel. And
    /// a piece built only outdoors can never stand aboard, where shared pieces are. What a piece
    /// provides is allowed by name (a floor to stand on, shelter), never by elimination, so a
    /// new kind of state is refused until someone says the server keeps it.
    fn why_not_shareable(bp: &Blueprint) -> Option<String> {
        if !bp.stations.is_empty() {
            return Some(format!("it is a crafting station ({:?})", bp.stations));
        }
        if bp.generates.is_some() {
            return Some("it makes power".into());
        }
        if bp.power_watts > 0.0 || bp.idle_watts > 0.0 {
            return Some("it draws power".into());
        }
        if bp.doorway.is_some() {
            return Some("it has a door that opens and shuts".into());
        }
        if bp.burns.is_some() {
            return Some("it is a fire, burning down its fuel".into());
        }
        if bp.outdoors_only {
            return Some("it is built only outdoors, and shared pieces stand aboard".into());
        }
        match bp.provides.as_deref() {
            None | Some("foundation" | "shelter") => None,
            Some(other) => Some(format!("it provides {other}, which the server does not keep")),
        }
    }

    /// THE DATA TEST. `shared: true` is only on blueprints whose pose is their whole state,
    /// because the relay keeps the pose and nothing else. The seven the design names are marked
    /// (both foundations, the three plain walls, the window wall, the roof). And the rule bites:
    /// it names a reason for the door wall, the chest, the bed, the furnace, the crafting table,
    /// the solar panel, the stove and the campfire (added 2026-10-05: it burns its logs), none of
    /// which is marked.
    /// Seen red 2026-10-05 before basic.ron gained the seven flags: `["wood_foundation is not
    /// shareable", "stone_foundation is not shareable", "wood_wall is not shareable", "stone_wall
    /// is not shareable", "metal_wall is not shareable", "wood_wall_window is not shareable",
    /// "roof is not shareable"]`.
    #[test]
    fn shared_only_on_stateless_blueprints() {
        let reg = shipped();
        let mut bad: Vec<String> = reg
            .blueprints
            .values()
            .filter(|b| b.shared)
            .filter_map(|b| why_not_shareable(b).map(|why| format!("{} is marked shared, but {why}", b.id)))
            .collect();
        bad.sort();
        assert!(bad.is_empty(), "{bad:#?}");
        let unmarked: Vec<String> = ["wood_foundation", "stone_foundation", "wood_wall", "stone_wall", "metal_wall", "wood_wall_window", "roof"]
            .iter()
            .filter(|id| !reg.get(id).unwrap_or_else(|| panic!("{id} in basic.ron")).shared)
            .map(|id| format!("{id} is not shareable"))
            .collect();
        assert!(unmarked.is_empty(), "{unmarked:?}");
        for id in ["wood_wall_door", "storage_chest", "bed", "furnace", "crafting_table", "solar_panel", "stove", "campfire"] {
            let bp = reg.get(id).unwrap_or_else(|| panic!("{id} in basic.ron"));
            assert!(!bp.shared, "{id} is not shared (no flag means false)");
            assert!(why_not_shareable(bp).is_some(), "the rule must name why {id} cannot be shared");
        }
    }

    /// THE WIRE. Every message, written exactly as the plan spells it, parses as its variant and
    /// writes back with exactly the plan's fields: the `type` strings are the `msg` constants; a
    /// piece carries no builder, and `mine` only in its builder's own copy; a refusal's codes are
    /// snake case; the welcome's `ranks` reads false where a field is missing; a code from a
    /// newer server reads as `unknown` instead of losing the whole message. The relay, the game
    /// and the rig's scripts all read these names, so a rename here would break the others
    /// silently.
    /// Seen red 2026-10-05 with `Piece::mine` always written (its `skip_serializing_if`
    /// removed): "someone else's copy: no builder, no mine", left: `["blueprint_id", "mine",
    /// "piece_id", "placed_at", "position", "rotation", "scale"]`.
    #[test]
    fn the_wire_messages_have_exactly_the_contract_fields() {
        let keys = |v: &serde_json::Value| -> Vec<String> {
            let mut k: Vec<String> = v.as_object().expect("an object").keys().cloned().collect();
            k.sort();
            k
        };
        let sorted = |want: &[&str]| -> Vec<String> {
            let mut w: Vec<String> = want.iter().map(|s| s.to_string()).collect();
            w.sort();
            w
        };
        let piece_json = r#"{"piece_id":42,"blueprint_id":"wood_wall","position":[30.0,0.2,41.0],"rotation":[0.0,0.70710677,0.0,0.70710677],"scale":[4.0,3.0,0.2],"placed_at":1759500000.25}"#;
        let piece: Piece = serde_json::from_str(piece_json).expect("the plan's piece parses");
        assert!(!piece.mine, "absent: not mine");
        let pose = piece.local_pose();
        assert_eq!((pose.position, pose.scale), (Vec3::new(30.0, 0.2, 41.0), Vec3::new(4.0, 3.0, 0.2)));
        let mut again = piece.clone();
        again.set_local_pose(&pose);
        assert_eq!(again, piece, "local_pose and set_local_pose are inverses");
        let piece_keys = ["piece_id", "blueprint_id", "position", "rotation", "scale", "placed_at"];
        assert_eq!(keys(&serde_json::to_value(&piece).unwrap()), sorted(&piece_keys), "someone else's copy: no builder, no mine");
        let mine = Piece { mine: true, ..piece.clone() };
        let mut with_mine = piece_keys.to_vec();
        with_mine.push("mine");
        assert_eq!(keys(&serde_json::to_value(&mine).unwrap()), sorted(&with_mine), "the builder's own copy says mine");
        assert_eq!(mine.marker("plot:p2"), SharedPiece { piece_id: 42, frame: "plot:p2".into(), mine: true });

        let permit = r#"{"issuer":"aabb","plot":"p3","grantee":"did:hum:abc","expiry":1767225600,"sig":"c2ln"}"#;
        // (json text, the variant's type constant, the fields it writes back with)
        let to_relay: Vec<(String, &str, Vec<&str>)> = vec![
            (
                format!(r#"{{"type":"game_build","req_id":7,"frame":"plot:p3","blueprint_id":"wood_wall","position":[30.0,0.0,42.0],"rotation":[0.0,0.0,0.0,1.0],"scale":[4.0,3.0,0.2],"permit":{permit}}}"#),
                msg::BUILD,
                vec!["type", "req_id", "frame", "blueprint_id", "position", "rotation", "scale", "permit"],
            ),
            (
                r#"{"type":"game_build","req_id":8,"frame":"plot:p1","blueprint_id":"roof","position":[30.0,3.2,42.0],"rotation":[0.0,0.0,0.0,1.0],"scale":[4.0,0.2,4.0]}"#.into(),
                msg::BUILD,
                vec!["type", "req_id", "frame", "blueprint_id", "position", "rotation", "scale"],
            ),
            (r#"{"type":"game_unbuild","req_id":9,"piece_id":42}"#.into(), msg::UNBUILD, vec!["type", "req_id", "piece_id"]),
            (format!(r#"{{"type":"game_unbuild","req_id":9,"piece_id":42,"permit":{permit}}}"#), msg::UNBUILD, vec!["type", "req_id", "piece_id", "permit"]),
            (r#"{"type":"game_pieces_request","frame":"zone:commons"}"#.into(), msg::PIECES_REQUEST, vec!["type", "frame"]),
        ];
        for (json, ty, fields) in &to_relay {
            let m: ToRelay = serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"));
            let out = serde_json::to_value(&m).unwrap();
            assert_eq!(out["type"], *ty, "{json}");
            assert_eq!(keys(&out), sorted(fields), "{json}");
            assert_eq!(serde_json::from_value::<ToRelay>(out).unwrap(), m, "{json}: round trip");
        }
        let from_relay: Vec<(String, &str, Vec<&str>)> = vec![
            (
                format!(r#"{{"type":"game_built","frame":"plot:p2","seq":5,"server_time":1759500001.5,"piece":{piece_json}}}"#),
                msg::BUILT,
                vec!["type", "frame", "seq", "server_time", "piece"],
            ),
            (
                format!(r#"{{"type":"game_built","frame":"plot:p2","seq":5,"server_time":1759500001.5,"piece":{piece_json},"req_id":7}}"#),
                msg::BUILT,
                vec!["type", "frame", "seq", "server_time", "piece", "req_id"],
            ),
            (r#"{"type":"game_unbuilt","frame":"plot:p2","seq":6,"piece_id":42}"#.into(), msg::UNBUILT, vec!["type", "frame", "seq", "piece_id"]),
            (
                r#"{"type":"game_unbuilt","frame":"plot:p2","seq":6,"piece_id":42,"req_id":9}"#.into(),
                msg::UNBUILT,
                vec!["type", "frame", "seq", "piece_id", "req_id"],
            ),
            (
                format!(r#"{{"type":"game_pieces","frame":"plot:p2","seq":6,"server_time":1759500002.0,"part":1,"parts":1,"pieces":[{piece_json}]}}"#),
                msg::PIECES,
                vec!["type", "frame", "seq", "server_time", "part", "parts", "pieces"],
            ),
            (r#"{"type":"game_frame_out_of_view","frame":"plot:p4"}"#.into(), msg::FRAME_OUT_OF_VIEW, vec!["type", "frame"]),
            (
                r#"{"type":"game_build_refused","req_id":7,"action":"build","reason":"not_allowed","why":"not_your_plot","message":"Wood Wall not built: ..."}"#.into(),
                msg::BUILD_REFUSED,
                vec!["type", "req_id", "action", "reason", "why", "message"],
            ),
            (
                r#"{"type":"game_build_refused","action":"pieces","reason":"rate_limited","message":"..."}"#.into(),
                msg::BUILD_REFUSED,
                vec!["type", "action", "reason", "message"],
            ),
        ];
        for (json, ty, fields) in &from_relay {
            let m: FromRelay = serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"));
            let out = serde_json::to_value(&m).unwrap();
            assert_eq!(out["type"], *ty, "{json}");
            assert_eq!(keys(&out), sorted(fields), "{json}");
            assert_eq!(serde_json::from_value::<FromRelay>(out).unwrap(), m, "{json}: round trip");
            let text = serde_json::to_string(&m).unwrap();
            for builder in ["owner", "did:hum", "builder", "name"] {
                assert!(!text.contains(builder), "{json}: no builder on the wire, found {builder:?} in {text}");
            }
        }
        let refused: FromRelay = serde_json::from_str(&from_relay[6].0).unwrap();
        assert!(matches!(
            refused,
            FromRelay::Refused { req_id: Some(7), action: Action::Build, reason: Reason::NotAllowed, why: Some(Why::NotYourPlot), .. }
        ));

        // The welcome's ranks: absent fields read false.
        let ranks: Ranks = serde_json::from_str(r#"{"can_edit_ship":true}"#).unwrap();
        assert_eq!(ranks, Ranks { can_edit_ship: true, take_down_any: false });
        assert_eq!(serde_json::from_str::<Ranks>("{}").unwrap(), Ranks::default());

        // A code from a newer server reads as unknown, and the message still parses.
        let newer: FromRelay = serde_json::from_str(
            r#"{"type":"game_build_refused","req_id":3,"action":"build","reason":"melted","why":"too_hot","message":"Wood Wall not built: it melted."}"#,
        )
        .expect("a newer server's refusal still parses");
        assert!(matches!(newer, FromRelay::Refused { req_id: Some(3), reason: Reason::Unknown, why: Some(Why::Unknown), .. }));
    }

    /// EVERY REFUSAL SAYS WHY IN PLAIN WORDS, ONE SENTENCE PER CASE. Each code the relay sends is
    /// written on the wire as its `as_str`, each has its words, and no words are empty, carry an
    /// em dash or end in a full stop (the sentence adds it). The sentences read as the plan
    /// wrote them: a build on someone else's plot, a take-down of someone else's piece, a
    /// take-down on someone else's plot, the shared spaces' rank, an expired permit, the three
    /// caps with their numbers (512, 512, 4,096) and the permit ceiling (90 days); a `why` wins
    /// over its `reason`; and the game's line for a piece the data does not share names the
    /// shipped kinds.
    /// Seen red 2026-10-05 with `why_words` reading the `reason` before the `why`: left "Wood
    /// Wall not built: this is not yours to change.", right "Wood Wall not built: this is someone
    /// else's plot; you build only on your own plot, or where its holder has given you a
    /// household permit."
    #[test]
    fn every_refusal_says_why_in_plain_words() {
        for r in Reason::ALL {
            assert_eq!(serde_json::to_value(r).unwrap(), r.as_str(), "{r:?}");
        }
        for w in Why::ALL {
            assert_eq!(serde_json::to_value(w).unwrap(), w.as_str(), "{w:?}");
        }
        let codes: std::collections::HashSet<&str> = Reason::ALL.iter().map(|r| r.as_str()).chain(Why::ALL.iter().map(|w| w.as_str())).collect();
        assert_eq!(codes.len(), Reason::ALL.len() + Why::ALL.len(), "every code different");
        for action in [Action::Build, Action::Unbuild, Action::Pieces] {
            let all = Reason::ALL.iter().map(|r| (*r, None)).chain(Why::ALL.iter().map(|w| (Reason::NotAllowed, Some(*w))));
            for (reason, why) in all {
                let words = why_words(action, reason, why);
                let what = format!("{action:?} {reason:?} {why:?}: {words:?}");
                assert!(!words.is_empty() && !words.contains('\u{2014}') && !words.ends_with('.'), "{what}");
                assert!(words.chars().next().is_some_and(|c| c.is_lowercase()), "{what}: words follow a colon");
            }
        }
        let wall = "Wood Wall";
        assert_eq!(
            refusal_message(Action::Build, wall, Reason::NotAllowed, Some(Why::NotYourPlot)),
            "Wood Wall not built: this is someone else's plot; you build only on your own plot, or where its holder has given you a household permit."
        );
        assert_eq!(
            refusal_message(Action::Unbuild, wall, Reason::NotAllowed, Some(Why::NotYourPiece)),
            "Wood Wall not taken down: it was put up by someone else, and only this plot's holder can take it down."
        );
        assert_eq!(
            refusal_message(Action::Unbuild, wall, Reason::NotAllowed, Some(Why::NotYourPlot)),
            "Wood Wall not taken down: this is someone else's plot, and you cannot take down what stands on it."
        );
        assert_eq!(
            why_words(Action::Build, Reason::NotAllowed, Some(Why::ShipRank)),
            "the ship's shared spaces are built by people this server has given that rank"
        );
        assert_eq!(
            why_words(Action::Build, Reason::NotAllowed, Some(Why::PermitExpired)),
            "your household permit for this plot has run out; ask its holder for a new one"
        );
        assert!(why_words(Action::Build, Reason::NotAllowed, Some(Why::PermitTooLong)).contains("at most 90 days"));
        assert!(why_words(Action::Build, Reason::FrameFull, None).ends_with("(512)"));
        assert!(why_words(Action::Build, Reason::OwnerFull, None).ends_with("(512)"));
        assert!(why_words(Action::Build, Reason::WorldFull, None).ends_with("(4,096)"));
        assert_eq!(why_words(Action::Build, Reason::Occupied, None), "one already stands there");
        assert_eq!(why_words(Action::Build, Reason::OffGrid, None), why_words(Action::Build, Reason::NotShared, None), "the same to a player");
        assert_eq!(refusal_message(Action::Unbuild, "piece", Reason::NoSuchPiece, None), "piece not taken down: it is already gone.");
        assert_eq!(thousands(1_000_000), "1,000,000");
        assert_eq!(only_shell_pieces_words(&shipped()), "only foundations, roofs and walls can be built outside your own home");
        assert_eq!(only_shell_pieces_words(&BlueprintRegistry::new()), "nothing can be built outside your own home");
        assert_eq!(Refusal::not_allowed(Why::Guest).why, Some(Why::Guest));
    }
}
