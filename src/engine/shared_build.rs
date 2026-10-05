//! The pieces the server keeps, the game's side (ship homes increment 5, "building only on your
//! own plot", 2026-10-05; docs/design/ship-homes-increment-5-plan.md sections 3.4 and 3.5). The
//! contract both sides compile is src/systems/construction/shared.rs; the relay's side is
//! src/relay/handlers/shared_build.rs.
//!
//! WHAT IT IS. In a server's shared world, the shell pieces a player builds aboard (foundations,
//! walls, the window wall, the roof: the blueprints marked `shared` in data/blueprints/basic.ron)
//! are kept by the server, and everyone near sees them. Everything the game does about that is
//! here:
//! - THE GATE ([`gate`]): before anything is spent, whether the piece in hand is the player's own
//!   (kept in their home and their save, as every build was before), goes to the server, or is
//!   refused, and in which words. The relay decides for real; the game asks the same questions
//!   first only so the player is told at the crosshair, before anything is paid.
//! - SENDING ([`tick`]): the ConstructionSystem checks the spot and takes the materials as for any
//!   build, then hands the paid-for build over in `shared::OUT_CHANNEL` instead of putting up a
//!   scaffold. Each frame the tick sends what waits there as a `game_build`, at most one request
//!   every `shared::SEND_INTERVAL_MS`, and keeps each one until the relay answers it.
//! - APPLYING what the relay says ([`on_game_message`]): a frame's whole list of pieces
//!   (`game_pieces`, in parts) replaces what this game holds for THAT frame; a piece built or taken
//!   down (`game_built`, `game_unbuilt`) moves the frame's `seq` on by exactly one; a frame that
//!   left the player's view is forgotten; a refusal gives back exactly what was spent and says why.
//! - TAKING DOWN ([`take_down_gate`]): F on a piece the server keeps asks the relay, and the piece
//!   stays until the relay says it came down; then its materials come back to whoever took it
//!   down (the operator's decision of 2026-10-05), once, the way F's take-down of the player's own
//!   piece gives them back: into the backpack, and what it has no room for into home storage.
//! - PUTTING RIGHT WHAT WAS LOST (the review of increment 5, finding 1). The relay skips what a
//!   socket that fell behind missed and keeps the connection open (relay.rs `recv_skipping_lag`),
//!   so any message can be lost without a word. A frame's whole list is asked for again
//!   (`game_pieces_request`, one a second, the relay's limit) whenever this game may have lost one
//!   of its messages: a gap in its `seq`; news about a frame it holds no list of; a list whose
//!   other parts never came (`shared::PARTS_WAIT_S`); a take-down the relay says is of a piece
//!   already gone (which comes down here at once); and the relay's check (`game_pieces_check`,
//!   every `shared::CHECK_INTERVAL_S`), which names each frame in view with its `seq`, so a lost
//!   change with nothing after it is noticed too. A frame the check does not name has left the
//!   view. The check also carries the player's ranks as they stand (finding 5).
//! - EXACTLY ONCE (finding 2). Every build and take-down sent settles exactly once: by its answer
//!   (`req_id`), whenever it comes, in a session or out of one; or, when that answer is lost, by
//!   its frame's next list known to come after the relay handled it (asked for after
//!   `shared::PENDING_TIMEOUT_S`, or in a later session). A build the list holds as ours (its
//!   blueprint and its box) was kept; one it does not was not, and its materials come back. A
//!   take-down whose piece the list lacks was done, and the materials come back; one whose piece
//!   still stands was not. Nothing settles on a timer, and leaving the shared world keeps what
//!   waits: a Respawn steps out and in on the same connection, and the answer still comes. Only
//!   the server a request went to can settle it.
//!
//! A PIECE IS DRAWN ONLY WHEN THE RELAY SAYS IT IS KEPT. The player's own build never puts up a
//! scaffold here: the relay's `game_built` does, for the builder exactly as for everyone near, so
//! nobody ever sees a piece the server did not keep. A piece younger than its build time grows
//! from the moment the relay took it (`server_time - placed_at`); an older one stands finished.
//!
//! NOTHING HERE IS SAVED. Shared pieces carry `shared::SharedPiece`, which the save, the home's
//! moves and the uid counter leave alone (Wave 1B). Every welcome starts them afresh (the relay
//! sends each frame in view whole right after it), and every departure takes them all down
//! (engine/home_plot.rs `forget_shared_entities`, and [`tick`] here once it sees the session end).
//!
//! WHOSE WORD ON MATERIALS. The relay holds no inventories yet (increment 8), so what a build cost
//! and what a take-down gives back are this game's own count, as the fleet ledger's gives are.
//! A refused build gives back exactly what it took: the backpack's part through the "Take to
//! backpack" channel (`inventory_transfer_ops`, so what no longer fits goes to storage, and the
//! player is told), the home storage's part straight back into placed storage, which is where
//! `stock_piles::take_consumed_home_stock` took it from.
//!
//! Everything below the engine entry points takes the parts it needs ([`Ctx`]), never the
//! EngineState, so the whole of it is tested without a window (shared_build_tests.rs).

use crate::ecs::components::Transform;
use crate::engine::build_place;
use crate::engine::state::EngineState;
use crate::gui::GuiState;
use crate::hot_reload::data_store::DataStore;
use crate::ship::build_frames::{parse_frame_id, BuildFrame, BuildFrames, FrameKind};
use crate::ship::ship_structure::ShipStructure;
use crate::systems::construction::shared::{
    self, msg, Action, FromRelay, OutQueue, Piece, Ranks, Reason, SharedBuildIntent, SharedPiece, Spent, ToRelay, Why,
};
use crate::systems::construction::{Blueprint, BlueprintRegistry, BuildRequest, Construction, PlanetSite, Structure};
use glam::{Mat4, Vec2, Vec3};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

// ── The gate ──────────────────────────────────────────────────────────────

/// What E does with the piece in hand, decided before anything is spent ([`gate`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Gate {
    /// The player's own piece, kept in their home and their save, as every build was before.
    Private,
    /// Kept by the server in this frame (`plot:p1`, `zone:commons`): sent, and drawn only when the
    /// relay says it is kept.
    Shared(String),
    /// Not built, and nothing spent: the words that follow "Placing Wood Wall: " under the
    /// crosshair, or "Wood Wall not built: " in a notice.
    Refused(String),
}

/// What the gate looks at. `pose` is in ship metres (the home frame's), as the ghost has it.
pub(crate) struct GateInput<'a> {
    /// In a server's shared world (`GuiState::copresence_active`).
    pub joined: bool,
    /// The ship's structure is this session's to edit: the Dev mode, and only offline
    /// (`config::ship_editing_for`). Read only offline; in a shared world nobody is exempt.
    pub ship_scope: bool,
    pub ship: Option<&'a ShipStructure>,
    /// For the words naming the kinds of piece that can be built outside a home.
    pub registry: Option<&'a BlueprintRegistry>,
    /// The piece in hand (None for a blueprint this game does not know: never shared).
    pub bp: Option<&'a Blueprint>,
    pub pose: &'a Transform,
    /// The build site on a planet's ground, or None aboard.
    pub site: Option<&'a PlanetSite>,
    /// What the last welcome said this player may do beyond their own plot.
    pub ranks: Ranks,
}

/// Why nothing is built in a shared space reaching past its edge: the words after "Placing Wood
/// Wall: ". Only someone with the ship-editing rank ever hears it (the rank comes first).
pub(crate) const ZONE_EDGE: &str = "a piece in the ship's shared spaces must stand wholly inside one";

/// Why nothing is built where no frame is (a corridor, the gap between two plots), for someone
/// with the ship-editing rank; anyone else hears the plot rule ([`build_place::off_plot_reason`]).
pub(crate) const NOT_IN_A_CORRIDOR: &str = "nothing is built in the ship's corridors, only on plots and in its shared spaces";

/// THE GATE: what E does with the piece in hand (ship homes increment 5, plan section 3.4). It
/// mirrors the relay's `may_build` and the pose rules (`shared::box_inside_frame`) so the player
/// is told at the crosshair, before anything is spent, what the relay would refuse. In order:
/// - on a planet's ground: the player's own, joined or not (the shared world is the ship);
/// - OFFLINE: exactly as before: the player's own, inside their own plot; the Dev mode anywhere
///   (`ship_scope`);
/// - in a shared world, nobody exempt (the ship is the server's, the Dev mode included):
///   - where no frame is (a corridor): refused;
///   - on the player's own plot, all of it inside the plot: a shareable piece goes to the server
///     (`plot:<own>`), any other stays in the home;
///   - anywhere else only shareable pieces can go (the words name the kinds);
///   - in a shared space: with the `can_edit_ship` rank, to the server (`zone:<id>`); without it,
///     refused with the rank's words;
///   - on another plot: refused, as someone else's plot (a guest: with no plot of their own).
///     Household permits (increment 5b) will open that row.
/// Pure, so every row is tested.
pub(crate) fn gate(g: &GateInput) -> Gate {
    if g.site.is_some() {
        return Gate::Private;
    }
    let guest = g.ship.is_some_and(|s| s.home_is_away());
    if !g.joined {
        if !g.ship_scope && build_place::outside_own_plot(g.ship, g.pose) {
            return Gate::Refused(build_place::off_plot_reason(guest).to_string());
        }
        return Gate::Private;
    }
    // No ship, no plot and no frames (the legacy layout): nothing to bound, as offline. A join
    // never goes out without a ship, so this is only a frame on the way in.
    let Some(ship) = g.ship else { return Gate::Private };
    let frames = BuildFrames::of_ship(ship);
    let Some(frame) = frames.frame_at(g.pose.position) else {
        return Gate::Refused(if g.ranks.can_edit_ship { NOT_IN_A_CORRIDOR.to_string() } else { build_place::off_plot_reason(guest).to_string() });
    };
    let local = frame.to_local(g.pose);
    let shareable = g.bp.is_some_and(|b| b.shared);
    let own = !guest && ship.home_plot().is_some_and(|p| frame.plot_id() == Some(p.id.as_str()));
    if own {
        // All of it on the plot, the rule of increment 1a, now the relay's own (build_frames.rs).
        if shared::box_inside_frame(frame, &local).is_err() {
            return Gate::Refused(build_place::off_plot_reason(false).to_string());
        }
        return if shareable { Gate::Shared(frame.id.clone()) } else { Gate::Private };
    }
    if !shareable {
        let empty = BlueprintRegistry::new();
        return Gate::Refused(shared::only_shell_pieces_words(g.registry.unwrap_or(&empty)));
    }
    let why = match frame.kind {
        FrameKind::Zone if g.ranks.can_edit_ship => {
            return match shared::box_inside_frame(frame, &local) {
                Ok(()) => Gate::Shared(frame.id.clone()),
                Err(_) => Gate::Refused(ZONE_EDGE.to_string()),
            };
        }
        FrameKind::Zone => Why::ShipRank,
        FrameKind::Plot if guest => Why::Guest,
        FrameKind::Plot => Why::NotYourPlot,
    };
    Gate::Refused(shared::why_words(Action::Build, Reason::NotAllowed, Some(why)))
}

/// [`gate`] for this session as it stands: whether it is in a shared world, its ship, its play
/// mode, and the ranks its last welcome gave.
pub(crate) fn gate_here(
    gui: &GuiState,
    registry: Option<&BlueprintRegistry>,
    ranks: Ranks,
    bp: Option<&Blueprint>,
    pose: &Transform,
    site: Option<&PlanetSite>,
) -> Gate {
    gate(&GateInput {
        joined: gui.copresence_active,
        ship_scope: crate::config::ship_editing_for(gui),
        ship: gui.ship_structure.as_ref(),
        registry,
        bp,
        pose,
        site,
        ranks,
    })
}

/// Whether the piece `bp` at `pose` (ship metres, aboard) goes to the server: what the ghost needs
/// to know before it is posed, because a piece the server keeps rests only on pieces the server
/// keeps (`placement::placement_pose_where` with `placement::shared_pieces`: everyone else sees
/// those, and never this player's private foundation under it).
pub(crate) fn will_be_shared(gui: &GuiState, data: &DataStore, ranks: Ranks, bp: &Blueprint, pose: &Transform) -> bool {
    let registry = data.get::<BlueprintRegistry>("blueprint_registry");
    matches!(gate_here(gui, registry, ranks, Some(bp), pose, None), Gate::Shared(_))
}

/// THE TAKE-DOWN GATE (F on a piece the server keeps): whether the relay would let this player take
/// `kept` down, the relay's `may_remove` mirrored: the server's admins and its owner anything
/// anywhere (`take_down_any`); a plot's holder anything on their own plot; the ship's shared spaces
/// only with the `can_edit_ship` rank; anyone else nothing, as someone else's plot (household
/// permits, increment 5b, will let a permit holder take down their own pieces there). Err is the
/// `why` its words come from. Pure.
pub(crate) fn take_down_gate(kept: &SharedPiece, ship: Option<&ShipStructure>, ranks: Ranks) -> Result<(), Why> {
    if ranks.take_down_any {
        return Ok(());
    }
    let own = ship.filter(|s| !s.home_is_away()).and_then(|s| s.home_plot()).map(|p| p.id.as_str());
    match parse_frame_id(&kept.frame) {
        Some((FrameKind::Zone, _)) if ranks.can_edit_ship => Ok(()),
        Some((FrameKind::Zone, _)) => Err(Why::ShipRank),
        Some((FrameKind::Plot, plot)) if own == Some(plot) => Ok(()),
        _ => Err(Why::NotYourPlot),
    }
}

// ── What the game holds ───────────────────────────────────────────────────

/// One frame's list arriving in parts (`game_pieces`).
#[derive(Debug, Default)]
struct Staging {
    seq: u64,
    server_time: f64,
    parts: u32,
    got: BTreeMap<u32, Vec<Piece>>,
    /// When its first part came ([`SharedBuild`]'s clock). Every part of one list leaves the relay
    /// together, so past `shared::PARTS_WAIT_S` the rest was lost ([`parts_overdue`]).
    since: f64,
}

/// A take-down this player asked for: the piece, its name, and what comes back when the relay says
/// it came down.
#[derive(Debug, Clone)]
pub(crate) struct Unbuild {
    pub piece_id: u64,
    pub name: String,
    pub materials: Vec<(String, u32)>,
}

/// A build sent and not answered yet.
#[derive(Debug, Clone)]
struct PendingBuild {
    req_id: u32,
    intent: SharedBuildIntent,
    /// The message as sent, to send again when the relay asks for it later (`rate_limited`).
    message: String,
    /// When it was sent ([`SharedBuild`]'s clock); a welcome starts it again.
    sent_at: f64,
    /// The session it was sent in: a list from a later one settles it (its answer is lost).
    session: u32,
    /// The server it went to ([`SharedBuild::server`]): only that server's lists settle it.
    server: String,
    /// When its frame's list was last asked for, each `shared::PENDING_TIMEOUT_S` with no answer.
    asked: Option<f64>,
    /// The relay said too much at once: send it again then.
    resend_at: Option<f64>,
}

/// A take-down sent and not answered yet: settled as a build is ([`PendingBuild`]), by its answer
/// or by its frame's list.
#[derive(Debug, Clone)]
struct PendingUnbuild {
    req_id: u32,
    unbuild: Unbuild,
    /// The frame the piece stood in when it was sent: that frame's list settles it.
    frame: String,
    message: String,
    sent_at: f64,
    session: u32,
    server: String,
    asked: Option<f64>,
    resend_at: Option<f64>,
}

/// The game's books on the pieces the server keeps: one field on EngineState. Nothing in it is
/// saved; a welcome and a departure start it again (see the top of this file).
#[derive(Debug, Default)]
pub(crate) struct SharedBuild {
    /// What this player may do beyond their own plot: the last welcome's `ranks`.
    pub(crate) ranks: Ranks,
    /// A welcome was applied and this session's pieces are in the world.
    active: bool,
    /// Counts welcomes, so a build sent in an earlier session is settled by this one's lists.
    session: u32,
    /// The server this session is with (`home_plot::active_server_key`, set as its welcome comes,
    /// [`on_welcome`]): builds and take-downs sent are filed under it, and only its lists settle
    /// them. A request sent to one server and never answered waits for that server.
    pub(crate) server: String,
    /// Real seconds since the game started (every timer here runs on it).
    now: f64,
    /// Each frame whose list this game holds, with its `seq`. A frame not here has no list:
    /// news about it waits for one.
    seqs: BTreeMap<String, u64>,
    staging: HashMap<String, Staging>,
    /// Each piece's entity, by the relay's id.
    index: HashMap<u64, hecs::Entity>,
    /// Paid-for builds waiting to be sent, in order.
    queue: VecDeque<SharedBuildIntent>,
    /// Take-downs waiting to be sent.
    unbuilds: VecDeque<Unbuild>,
    pending: Vec<PendingBuild>,
    pending_unbuilds: Vec<PendingUnbuild>,
    /// The last `req_id` used (each request gets the next).
    last_req: u32,
    /// The next build or take-down may go then (`shared::SEND_INTERVAL_MS` apart).
    next_send_at: f64,
    /// Frames whose whole list this game will ask for again (whenever it may have lost one of
    /// their messages, or to settle a request with no answer; see the top of this file), and the
    /// last one asked for and when: one player asks at most once a
    /// `shared::PIECES_REQUEST_INTERVAL_MS`, whichever frame.
    lists_wanted: BTreeSet<String>,
    last_list: Option<(String, f64)>,
    /// Messages for the relay. The engine sends them on the active connection ([`flush`]); the
    /// tests read them.
    pub(crate) outbox: Vec<String>,
}

impl SharedBuild {
    /// Builds and take-downs not answered yet, and paid-for builds not sent yet (the probe).
    pub(crate) fn counts(&self) -> (usize, usize, usize) {
        (self.pending.len(), self.pending_unbuilds.len() + self.unbuilds.len(), self.queue.len())
    }

    /// Every frame whose list this game holds, with its `seq` (the probe).
    pub(crate) fn frame_seqs(&self) -> impl Iterator<Item = (&String, &u64)> {
        self.seqs.iter()
    }

    /// Is a build with the box `pose` (ship metres) already on its way to the server: paid for
    /// and waiting here or in the channel, asked for this frame, or sent and not answered? E on it
    /// would spend the materials twice for what the relay would refuse as `occupied` (Wave 1B's
    /// note: `placement::occupied` sees only what stands in the world).
    pub(crate) fn waiting_at(&self, data: &DataStore, pose: &Transform) -> bool {
        let same = |q: &Transform| shared::same_box(q, pose);
        self.queue.iter().any(|i| same(&i.pose))
            // A build waiting for another server is of a spot there, not here.
            || self.pending.iter().any(|p| p.server == self.server && same(&p.intent.pose))
            || data
                .get::<OutQueue>(shared::OUT_CHANNEL)
                .is_some_and(|q| q.lock().is_ok_and(|q| q.iter().any(|i| same(&i.pose))))
            || data
                .get::<std::sync::Mutex<Vec<BuildRequest>>>("build_request")
                .is_some_and(|c| c.lock().is_ok_and(|c| c.iter().any(|r| r.shared_frame.is_some() && same(&r.pose))))
    }

    /// Ask the relay to take a piece down (F, or the dev verb): sent by the tick, never twice for
    /// one piece.
    pub(crate) fn ask_take_down(&mut self, u: Unbuild) {
        // (A take-down waiting for another server is of that server's piece, whatever its number.)
        let asked = self.unbuilds.iter().any(|q| q.piece_id == u.piece_id)
            || self.pending_unbuilds.iter().any(|q| q.unbuild.piece_id == u.piece_id && q.server == self.server);
        if !asked {
            self.unbuilds.push_back(u);
        }
    }

    fn next_req_id(&mut self) -> u32 {
        self.last_req = self.last_req.wrapping_add(1).max(1);
        self.last_req
    }

    /// Whether a `game_built` or `game_unbuilt` with `seq` in `frame` is applied, moving the frame's
    /// seq on: never outside a session, never for a frame with no list (the list will hold it,
    /// and it is asked for), never at or below the frame's seq (a duplicate, or news the list
    /// already holds); after a gap (more than one ahead) it is applied, and the frame's whole list
    /// asked for again.
    fn step(&mut self, frame: &str, seq: u64) -> bool {
        if !self.active {
            return false;
        }
        let Some(have) = self.seqs.get_mut(frame) else {
            // News about a frame this game holds no whole list of: the list, or a part of it,
            // was lost (the relay sends a frame's list before any news of it), or this is our own
            // answer from a frame out of view. The list holds this news too, so it is asked for
            // (review of increment 5, finding 1, case B: before, every piece built there was
            // ignored for the rest of the session, the player's own included).
            self.want_list(frame);
            return false;
        };
        if seq <= *have {
            return false;
        }
        if seq > *have + 1 {
            // (`want_list`, by its field: `have` holds `seqs`.)
            self.lists_wanted.insert(frame.to_string());
        }
        *have = seq;
        true
    }

    /// Ask for `frame`'s whole list, the next time one may be asked for ([`ask_for_lists`]).
    fn want_list(&mut self, frame: &str) {
        self.lists_wanted.insert(frame.to_string());
    }

    /// Keep one part of a frame's list; the whole list (and the relay's clock with it) once every
    /// part is in. A part of another `seq` starts the frame's list again.
    fn stage(&mut self, frame: &str, seq: u64, server_time: f64, part: u32, parts: u32, pieces: Vec<Piece>) -> Option<(Vec<Piece>, f64)> {
        if parts == 0 || part == 0 || part > parts {
            log::warn!("Shared building: part {part} of {parts} of {frame}'s list makes no sense; ignored");
            return None;
        }
        let now = self.now;
        let s = self.staging.entry(frame.to_string()).or_default();
        if s.got.is_empty() || s.seq != seq || s.parts != parts {
            *s = Staging { seq, server_time, parts, got: BTreeMap::new(), since: now };
        }
        s.got.insert(part, pieces);
        if s.got.len() < parts as usize {
            return None;
        }
        let s = self.staging.remove(frame)?;
        Some((s.got.into_values().flatten().collect(), s.server_time))
    }
}

/// The parts of the engine the books work on. Built from the EngineState by the engine entry
/// points, from plain values by the tests.
pub(crate) struct Ctx<'a> {
    pub sb: &'a mut SharedBuild,
    pub world: &'a mut hecs::World,
    pub data: &'a DataStore,
    pub gui: &'a mut GuiState,
}

fn ctx(state: &mut EngineState) -> Ctx<'_> {
    Ctx { sb: &mut state.shared_build, world: &mut state.game_world.world, data: &state.data_store, gui: &mut state.gui_state }
}

// ── The engine's entry points ─────────────────────────────────────────────

/// A relay message about the pieces the server keeps; true when it was one (engine/net_route.rs
/// hands every `game_*` message here before its own match).
pub(crate) fn on_game_message(state: &mut EngineState, v: &serde_json::Value) -> bool {
    let claimed = on_message(&mut ctx(state), v);
    flush(state);
    claimed
}

/// A welcome to a shared world (engine/net_route.rs, once the home has taken it). The server it
/// comes from is this session's ([`SharedBuild::server`]).
pub(crate) fn on_welcome(state: &mut EngineState, v: &serde_json::Value) {
    state.shared_build.server = crate::engine::home_plot::active_server_key(&state.gui_state);
    welcome(&mut ctx(state), v);
    flush(state);
}

/// Once a frame, at the end of the co-presence block (lib.rs), joined or not: paid-for builds are
/// sent (or, out of the shared world, given back), unanswered ones looked after, and a session
/// that ended is cleared away.
pub(crate) fn tick(state: &mut EngineState, real_dt: f32) {
    let (joined, welcomed) = (state.game_joined, state.game_welcomed);
    tick_core(&mut ctx(state), real_dt, joined, welcomed);
    flush(state);
}

/// Send what waits in the outbox on the active connection.
fn flush(state: &mut EngineState) {
    if state.shared_build.outbox.is_empty() {
        return;
    }
    let out = std::mem::take(&mut state.shared_build.outbox);
    match state.gui_state.ws_client.as_ref() {
        Some(ws) => {
            for m in &out {
                ws.send(m);
            }
        }
        None => log::info!("Shared building: no connection; {} message(s) for the relay not sent", out.len()),
    }
}

// ── Messages ──────────────────────────────────────────────────────────────

/// Is `ty` one of the relay's messages about the pieces it keeps?
pub(crate) fn is_ours(ty: &str) -> bool {
    [msg::BUILT, msg::UNBUILT, msg::PIECES, msg::FRAME_OUT_OF_VIEW, msg::BUILD_REFUSED, msg::PIECES_CHECK].contains(&ty)
}

/// A relay message: true when it is about the pieces the server keeps (then it is applied here,
/// or logged when it cannot be read).
pub(crate) fn on_message(cx: &mut Ctx, v: &serde_json::Value) -> bool {
    let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
    if !is_ours(ty) {
        return false;
    }
    match serde_json::from_value::<FromRelay>(v.clone()) {
        Ok(m) => receive(cx, m),
        Err(e) => log::warn!("Shared building: a {ty} this game cannot read ({e}); ignored"),
    }
    true
}

fn receive(cx: &mut Ctx, m: FromRelay) {
    match m {
        FromRelay::Built { frame, seq, server_time, piece, req_id } => {
            // Our own build, kept: its materials stay spent. Settled even with no list for the
            // frame (the list will hold the piece) and after leaving (a late answer).
            if let Some(id) = req_id {
                cx.sb.pending.retain(|p| p.req_id != id);
            }
            if cx.sb.step(&frame, seq) {
                if let Some(f) = frame_of(cx.gui, &frame) {
                    spawn_listed(cx, &f, &piece, server_time);
                }
            }
        }
        FromRelay::Unbuilt { frame, seq, piece_id, req_id } => {
            match req_id {
                // Our own take-down, done: its materials come back. Settled even after leaving
                // (a late answer on this connection).
                Some(id) => took_down(cx, id),
                // Someone else's: a take-down of ours of the same piece is beaten to it.
                None => beaten_to_it(cx, piece_id),
            }
            if cx.sb.step(&frame, seq) {
                despawn_piece(cx.world, &mut cx.sb.index, piece_id);
            }
        }
        FromRelay::Pieces { frame, seq, server_time, part, parts, pieces } => {
            if !cx.sb.active {
                return;
            }
            if let Some((all, server_time)) = cx.sb.stage(&frame, seq, server_time, part, parts, pieces) {
                // A list older than the news already applied cannot come over the same connection
                // (the relay sends in order); kept out all the same, so it can never undo news.
                if cx.sb.seqs.get(&frame).is_some_and(|have| seq < *have) {
                    log::warn!("Shared building: {frame}'s list at seq {seq} is older than what this game holds; not applied");
                    return;
                }
                replace_frame(cx, &frame, seq, server_time, &all);
            }
        }
        FromRelay::FrameOutOfView { frame } => {
            if cx.sb.active {
                forget_frame(cx, &frame);
            }
        }
        FromRelay::Refused { req_id, action, reason, why, message } => refused(cx, req_id, action, reason, why, &message),
        FromRelay::Check { frames, ranks } => {
            // A check from a session this game has left says nothing about the next one.
            if cx.sb.active {
                cx.sb.ranks = ranks;
                check(cx, &frames);
            }
        }
    }
}

/// The relay's check (`game_pieces_check`, every `shared::CHECK_INTERVAL_S`; the review of
/// increment 5, finding 1): what this game should hold. It goes out in order with the frames'
/// news, so a frame's `seq` in it is exactly that of the last news about the frame sent before it:
/// a frame this game holds at another `seq`, or holds no whole list of, lost a message, and its
/// list is asked for. A frame whose list is arriving in parts at that very `seq` is left to its
/// parts (a list the player asked for can go out beside a check; [`parts_overdue`] looks after
/// them). A frame this game holds and the check does not name has left the player's view, and the
/// word of it was lost: it is forgotten, as `game_frame_out_of_view` would have.
fn check(cx: &mut Ctx, frames: &BTreeMap<String, u64>) {
    for (frame, seq) in frames {
        let held = cx.sb.seqs.get(frame) == Some(seq);
        let arriving = cx.sb.staging.get(frame).is_some_and(|s| s.seq == *seq);
        if !held && !arriving {
            cx.sb.want_list(frame);
        }
    }
    let left: Vec<String> = cx.sb.seqs.keys().chain(cx.sb.staging.keys()).filter(|f| !frames.contains_key(*f)).cloned().collect();
    for frame in left {
        forget_frame(cx, &frame);
    }
}

/// The frame `id` of the ship this game runs, or None (a frame this ship does not have: another
/// version of the ship file, which the relay's join check should have refused).
fn frame_of(gui: &GuiState, id: &str) -> Option<BuildFrame> {
    gui.ship_structure.as_ref().and_then(|s| BuildFrames::of_ship(s).get(id).cloned())
}

/// The entity drawing `piece_id`, from the index, when it really is that piece.
fn live_entity(world: &hecs::World, index: &HashMap<u64, hecs::Entity>, piece_id: u64) -> Option<hecs::Entity> {
    index.get(&piece_id).copied().filter(|e| world.get::<&SharedPiece>(*e).is_ok_and(|k| k.piece_id == piece_id))
}

/// The entity drawing `piece_id`, looked for in the world (the dev verb `take_down`).
pub(crate) fn entity_of(world: &hecs::World, piece_id: u64) -> Option<hecs::Entity> {
    let mut q = world.query::<&SharedPiece>();
    let found = q.iter().find(|(_e, k)| k.piece_id == piece_id).map(|(e, _)| e);
    found
}

/// Put `piece` (in `frame`, measured from its corner) into the world as the relay keeps it, at
/// ship metres: a scaffold that has grown for as long as the relay has had it
/// (`server_time - placed_at`), or a finished piece when that is past its build time (Wave 1B's
/// note: so a piece seen again is never grown again from nothing, and the player's own never
/// earns its reward twice). Marked `SharedPiece`. Pure on the world.
pub(crate) fn spawn_piece(world: &mut hecs::World, registry: Option<&BlueprintRegistry>, frame: &BuildFrame, piece: &Piece, server_time: f64) -> hecs::Entity {
    let pose = frame.to_ship(&piece.local_pose());
    let marker = piece.marker(&frame.id);
    let bp = registry.and_then(|r| r.get(&piece.blueprint_id));
    let build_time = bp.map_or(0.0, |b| b.build_time);
    let age = (server_time - piece.placed_at).max(0.0) as f32;
    if age >= build_time {
        let (health, provides) = bp.map_or((100.0, None), |b| (b.health, b.provides.clone()));
        world.spawn((pose, Structure { blueprint_id: piece.blueprint_id.clone(), health, max_health: health, provides, uid: 0 }, marker))
    } else {
        world.spawn((pose, Construction { blueprint_id: piece.blueprint_id.clone(), progress: age, build_time, builder_key: None }, marker))
    }
}

fn spawn_listed(cx: &mut Ctx, frame: &BuildFrame, piece: &Piece, server_time: f64) {
    if live_entity(cx.world, &cx.sb.index, piece.piece_id).is_some() {
        return;
    }
    let e = spawn_piece(cx.world, cx.data.get::<BlueprintRegistry>("blueprint_registry"), frame, piece, server_time);
    cx.sb.index.insert(piece.piece_id, e);
}

/// Take `piece_id` out of the world. The index can be stale (a departure takes the pieces down
/// without it, home_plot.rs `forget_shared_entities`), so an entity is taken down only when it
/// still IS that piece; otherwise the piece is looked for in the world. Nothing else is ever
/// despawned. True when a piece came down.
fn despawn_piece(world: &mut hecs::World, index: &mut HashMap<u64, hecs::Entity>, piece_id: u64) -> bool {
    let indexed = index.remove(&piece_id);
    let e = match indexed.filter(|e| world.get::<&SharedPiece>(*e).is_ok_and(|k| k.piece_id == piece_id)) {
        Some(e) => e,
        None => match entity_of(world, piece_id) {
            Some(e) => e,
            None => return false,
        },
    };
    world.despawn(e).is_ok()
}

/// A frame's whole list (`game_pieces`, every part in): what this game holds for THAT frame
/// becomes exactly the list. A piece no longer listed comes down; a listed piece already drawn
/// stays as it is (same entity, its scaffold still growing); a new one goes up. Every other frame
/// is left alone. Then the frame's `seq` is the list's, and the builds the list settles are
/// settled ([`settle_by_list`]).
fn replace_frame(cx: &mut Ctx, frame_id: &str, seq: u64, server_time: f64, pieces: &[Piece]) {
    let Some(frame) = frame_of(cx.gui, frame_id) else {
        log::warn!("Shared building: a list for {frame_id}, which this ship does not have; not drawn");
        return;
    };
    let listed: HashSet<u64> = pieces.iter().map(|p| p.piece_id).collect();
    let gone: Vec<(hecs::Entity, u64)> = cx
        .world
        .query::<&SharedPiece>()
        .iter()
        .filter(|(_e, k)| k.frame == frame_id && !listed.contains(&k.piece_id))
        .map(|(e, k)| (e, k.piece_id))
        .collect();
    for (e, id) in gone {
        let _ = cx.world.despawn(e);
        cx.sb.index.remove(&id);
    }
    for p in pieces {
        spawn_listed(cx, &frame, p, server_time);
    }
    cx.sb.seqs.insert(frame_id.to_string(), seq);
    cx.sb.lists_wanted.remove(frame_id);
    settle_by_list(cx, &frame, pieces);
}

/// The builds and take-downs a frame's list settles, each exactly once (the review of increment 5,
/// finding 2), and only those sent to this session's server.
/// - A build whose blueprint and box a piece of ours in the list has was kept (its answer was
///   lost): its materials stay spent.
/// - Anything else is settled only when the list is known to come after the relay handled it: its
///   frame's list was asked for after it went unanswered, or it was sent in an earlier session
///   (its answer went with that one). Then a build the list does not hold was never kept, and
///   what it took comes back; a take-down whose piece the list lacks was done, and the piece's
///   materials come back; one whose piece the list still holds was not done, and the player is
///   told. A list the relay sent before handling a request says nothing about it, so any other
///   request waits on.
fn settle_by_list(cx: &mut Ctx, frame: &BuildFrame, pieces: &[Piece]) {
    let (session, server) = (cx.sb.session, cx.sb.server.clone());
    let after_it = |asked: Option<f64>, sent_in: u32| asked.is_some() || sent_in != session;
    let mut lost = Vec::new();
    cx.sb.pending.retain(|p| {
        if p.intent.frame != frame.id || p.server != server {
            return true;
        }
        let local = frame.to_local(&p.intent.pose);
        if pieces.iter().any(|q| q.mine && q.blueprint_id == p.intent.blueprint_id && shared::same_box(&q.local_pose(), &local)) {
            return false;
        }
        if after_it(p.asked, p.session) {
            lost.push(p.intent.clone());
            return false;
        }
        true
    });
    for intent in lost {
        let line = never_answered(cx.data, &intent.blueprint_id);
        refund_build(cx, &intent, line);
    }
    let (mut done, mut standing) = (Vec::new(), Vec::new());
    cx.sb.pending_unbuilds.retain(|u| {
        if u.frame != frame.id || u.server != server || !after_it(u.asked, u.session) {
            return true;
        }
        if pieces.iter().any(|q| q.piece_id == u.unbuild.piece_id) {
            standing.push(u.unbuild.clone());
        } else {
            done.push(u.unbuild.clone());
        }
        false
    });
    for u in done {
        give_back_taken_down(cx, &u);
    }
    for u in standing {
        cx.gui.pending_notices.push(format!("{} not taken down: the server never said it came down.", u.name));
    }
}

/// A frame left the player's view: its pieces come down, and news about it waits for its next list.
/// A build or take-down waiting on it keeps waiting ([`timeouts`] asks for the list, which comes
/// out of view too).
fn forget_frame(cx: &mut Ctx, frame_id: &str) {
    let gone: Vec<(hecs::Entity, u64)> = cx.world.query::<&SharedPiece>().iter().filter(|(_e, k)| k.frame == frame_id).map(|(e, k)| (e, k.piece_id)).collect();
    for (e, id) in gone {
        let _ = cx.world.despawn(e);
        cx.sb.index.remove(&id);
    }
    cx.sb.seqs.remove(frame_id);
    cx.sb.staging.remove(frame_id);
    cx.sb.lists_wanted.remove(frame_id);
}

/// Our take-down `req_id` came down: its materials come back to us, and we are told.
fn took_down(cx: &mut Ctx, req_id: u32) {
    let Some(i) = cx.sb.pending_unbuilds.iter().position(|u| u.req_id == req_id) else { return };
    let u = cx.sb.pending_unbuilds.remove(i).unbuild;
    give_back_taken_down(cx, &u);
}

/// A take-down of ours the relay did: the piece's materials come back to us, once (decision 1 of
/// the increment 5 plan: whoever takes a piece down gets them), and we are told.
fn give_back_taken_down(cx: &mut Ctx, u: &Unbuild) {
    let back = Spent { pack: u.materials.clone(), storage: Vec::new() };
    let got = give_back(cx, &back);
    cx.gui.pending_notices.push(if got.is_empty() { format!("Took down the {}", u.name) } else { format!("Took down the {}: {got} back", u.name) });
}

/// Someone else took piece `piece_id` down (a `game_unbuilt` carrying no req_id of ours): a
/// take-down of ours of the same piece, waiting to go or waiting for its answer, was beaten to it.
/// Nothing comes back (whoever took it down got its materials), the player is told once, in the
/// words of the relay's own answer to ours (`no_such_piece`), and that answer, when it comes, finds
/// nothing left to settle. Only a take-down sent to this session's server: another server's piece
/// numbers are its own.
fn beaten_to_it(cx: &mut Ctx, piece_id: u64) {
    let mut names: Vec<String> = cx.sb.unbuilds.iter().filter(|u| u.piece_id == piece_id).map(|u| u.name.clone()).collect();
    cx.sb.unbuilds.retain(|u| u.piece_id != piece_id);
    let server = cx.sb.server.clone();
    let ours = |u: &PendingUnbuild| u.unbuild.piece_id == piece_id && u.server == server;
    names.extend(cx.sb.pending_unbuilds.iter().filter(|u| ours(u)).map(|u| u.unbuild.name.clone()));
    cx.sb.pending_unbuilds.retain(|u| !ours(u));
    for name in names {
        cx.gui.pending_notices.push(shared::refusal_message(Action::Unbuild, &name, Reason::NoSuchPiece, None));
    }
}

/// A refusal (`game_build_refused`). A build gives back exactly what it took and says why; a
/// take-down says why, and one of a piece the server no longer keeps (`no_such_piece`) also takes
/// that piece down here and asks for its frame's list; "too much at once" (`rate_limited`) sends
/// the request again a moment later. The sentence is the contract's (`shared::refusal_message`),
/// or the relay's own for a code this game does not know.
fn refused(cx: &mut Ctx, req_id: Option<u32>, action: Action, reason: Reason, why: Option<Why>, message: &str) {
    let retry = cx.sb.now + shared::RATE_LIMITED_RETRY_MS as f64 / 1000.0;
    let said = |name: &str| if reason == Reason::Unknown { message.to_string() } else { shared::refusal_message(action, name, reason, why) };
    match (action, req_id) {
        (Action::Build, Some(id)) => {
            let Some(i) = cx.sb.pending.iter().position(|p| p.req_id == id) else { return };
            if reason == Reason::RateLimited {
                cx.sb.pending[i].resend_at = Some(retry);
                return;
            }
            let p = cx.sb.pending.remove(i);
            let line = said(&blueprint_name(cx.data, &p.intent.blueprint_id));
            refund_build(cx, &p.intent, line);
        }
        (Action::Unbuild, Some(id)) => {
            let Some(i) = cx.sb.pending_unbuilds.iter().position(|u| u.req_id == id) else { return };
            if reason == Reason::RateLimited {
                cx.sb.pending_unbuilds[i].resend_at = Some(retry);
                return;
            }
            let u = cx.sb.pending_unbuilds.remove(i);
            if reason == Reason::NoSuchPiece && cx.sb.active && u.server == cx.sb.server {
                // The server no longer keeps it, and this game still drew it, solid: the news of
                // its take-down was lost (review of increment 5, finding 1, case A). It comes down
                // here too, and its frame's whole list is asked for, which may have missed more.
                // (Out of a session every piece is down already.)
                despawn_piece(cx.world, &mut cx.sb.index, u.unbuild.piece_id);
                cx.sb.want_list(&u.frame);
            }
            cx.gui.pending_notices.push(said(&u.unbuild.name));
        }
        // A list we asked for (it carries no req_id): too soon, and the last frame asked for is
        // asked for again; anything else is only logged, and a request with no answer is asked
        // about again on its own clock ([`timeouts`]).
        (Action::Pieces, _) => {
            if reason == Reason::RateLimited {
                if let Some((frame, _)) = cx.sb.last_list.clone() {
                    cx.sb.lists_wanted.insert(frame);
                }
            }
            log::info!("Shared building: a list was not sent ({}): {message}", reason.as_str());
        }
        (_, None) => log::warn!("Shared building: a refusal naming no request ({}): {message}", reason.as_str()),
    }
}

/// The name of blueprint `id` (the id itself for one this game does not know).
fn blueprint_name(data: &DataStore, id: &str) -> String {
    data.get::<BlueprintRegistry>("blueprint_registry").and_then(|r| r.get(id)).map_or_else(|| id.to_string(), |b| b.name.clone())
}

/// The line for a build the relay never answered.
fn never_answered(data: &DataStore, blueprint_id: &str) -> String {
    format!("{} not built: the server never said it kept it.", blueprint_name(data, blueprint_id))
}

/// Give back what `intent` took, and say `line` with what came back: "Wood Wall not built: one
/// already stands there. 6 Wood Plank back."
fn refund_build(cx: &mut Ctx, intent: &SharedBuildIntent, line: String) {
    let got = give_back(cx, &intent.spent);
    cx.gui.pending_notices.push(if got.is_empty() { line } else { format!("{line} {got} back.") });
}

/// Give back `spent`: the backpack's part through the "Take to backpack" channel (so what no
/// longer fits goes to storage, and the player is told), the home storage's part straight back
/// into placed storage, onto a stack of the item in the home (never a chest on a planet), else
/// loose in the home. Returns what came back by the items' names, "6 Wood Plank" (empty for
/// nothing).
fn give_back(cx: &mut Ctx, spent: &Spent) -> String {
    use crate::systems::inventory::{placed::PlacedItem, ItemRegistry, TransferOp};
    if let Some(chan) = cx.data.get::<std::sync::Mutex<Vec<TransferOp>>>("inventory_transfer_ops") {
        let mut c = chan.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        for (id, qty) in spent.pack.iter().filter(|(_, q)| *q > 0) {
            c.push(TransferOp { item_id: id.clone(), qty: *qty, add: true, ..Default::default() });
        }
    }
    let items = cx.data.get::<ItemRegistry>("item_registry");
    let name_of = |id: &str| items.and_then(|r| r.items.get(id).map(|d| d.name.clone())).unwrap_or_else(|| id.to_string());
    let away = crate::systems::construction::uses::planet_store_paths(cx.world);
    let elsewhere = |c: &str| away.iter().any(|p| c == p || c.strip_prefix(p.as_str()).is_some_and(|rest| rest.starts_with('/')));
    for (id, qty) in spent.storage.iter().filter(|(_, q)| *q > 0) {
        let pool = &mut cx.gui.placed_items;
        match pool.iter_mut().find(|p| p.key == *id && p.wear == 0 && p.quality == 0 && !elsewhere(&p.container)) {
            Some(p) => {
                p.age_s = crate::systems::inventory::blend_age(p.age_s, p.qty, 0.0, *qty);
                p.qty += qty;
            }
            None => pool.push(PlacedItem { key: id.clone(), name: name_of(id), qty: *qty, container: "Home".to_string(), ..Default::default() }),
        }
    }
    // One line, each item once, in the order it was first spent.
    let mut total: Vec<(String, u32)> = Vec::new();
    for (id, qty) in spent.pack.iter().chain(&spent.storage).filter(|(_, q)| *q > 0) {
        match total.iter_mut().find(|(t, _)| t == id) {
            Some((_, n)) => *n += qty,
            None => total.push((id.clone(), *qty)),
        }
    }
    total.iter().map(|(id, n)| format!("{n} {}", name_of(id))).collect::<Vec<_>>().join(", ")
}

// ── Welcome, leaving, the tick ────────────────────────────────────────────

/// A welcome to a shared world: what this player may do (`ranks`, absent fields false; the relay's
/// check keeps them current, [`check`]), and the shared pieces start afresh, because the relay
/// sends every frame in view whole right after it. Builds and take-downs sent before it stay
/// waiting: the lists that follow settle them ([`settle_by_list`]), on a fresh clock, and one
/// whose frame sends no list is asked about after `shared::PENDING_TIMEOUT_S` ([`timeouts`]).
pub(crate) fn welcome(cx: &mut Ctx, v: &serde_json::Value) {
    leave(cx);
    cx.sb.ranks = v.get("ranks").and_then(|r| serde_json::from_value::<Ranks>(r.clone()).ok()).unwrap_or_default();
    cx.sb.session = cx.sb.session.wrapping_add(1);
    cx.sb.active = true;
    let now = cx.sb.now;
    for p in &mut cx.sb.pending {
        p.asked = None;
        p.sent_at = now;
    }
    for u in &mut cx.sb.pending_unbuilds {
        u.asked = None;
        u.sent_at = now;
    }
}

/// The session ended (left, dropped, or a new join waiting for its welcome): every shared piece
/// comes down (home_plot.rs `forget_shared_entities` usually took them already), what this game
/// knew of the frames goes with them, and so do the take-downs not sent yet. Builds and take-downs
/// sent and not answered STAY (the review of increment 5, finding 2: the take-downs were dropped
/// here, so one the relay did gave nothing back): their answer can still come on this connection
/// (a Respawn steps out and in on the same one), and the next session's lists settle the rest.
fn leave(cx: &mut Ctx) {
    let pieces: Vec<hecs::Entity> = cx.world.query::<&SharedPiece>().iter().map(|(e, _)| e).collect();
    let taken = pieces.len();
    for e in pieces {
        let _ = cx.world.despawn(e);
    }
    let sb = &mut *cx.sb;
    sb.index.clear();
    sb.seqs.clear();
    sb.staging.clear();
    sb.lists_wanted.clear();
    sb.unbuilds.clear();
    sb.ranks = Ranks::default();
    if sb.active {
        log::info!(
            "Shared building: out of the session; took down {taken} piece(s) the server keeps; {} build(s) and {} take-down(s) wait for their answer",
            sb.pending.len(),
            sb.pending_unbuilds.len()
        );
    }
    sb.active = false;
}

/// [`tick`] on the parts: `joined` (the game sent its join) and `welcomed` (the welcome was applied).
pub(crate) fn tick_core(cx: &mut Ctx, real_dt: f32, joined: bool, welcomed: bool) {
    cx.sb.now += f64::from(real_dt.max(0.0));
    // What the ConstructionSystem paid for this frame.
    if let Some(q) = cx.data.get::<OutQueue>(shared::OUT_CHANNEL) {
        let mut q = q.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        cx.sb.queue.extend(q.drain(..));
    }
    if cx.sb.active && !(joined && welcomed) {
        leave(cx);
    }
    if !joined {
        // Out of the shared world nothing can be sent: a paid-for build that never went comes
        // back now, in the words of the relay's own refusal, and a take-down that never went is
        // dropped. What WAS sent and not answered waits: its answer can still come on this
        // connection, and the next session's lists settle it either way (the review of increment
        // 5, finding 2: a timer here gave back builds the relay had kept).
        while let Some(intent) = cx.sb.queue.pop_front() {
            let line = shared::refusal_message(Action::Build, &blueprint_name(cx.data, &intent.blueprint_id), Reason::NotInGame, None);
            refund_build(cx, &intent, line);
        }
        cx.sb.unbuilds.clear();
        return;
    }
    if !welcomed {
        // Joined, the welcome on its way: builds wait for it.
        return;
    }
    timeouts(cx);
    parts_overdue(cx);
    ask_for_lists(cx);
    send_next(cx);
}

/// A build or take-down sent to this session's server with no answer after
/// `shared::PENDING_TIMEOUT_S` asks for its frame's list, which settles it ([`settle_by_list`]);
/// with no list either after as long again (the ask, or the list, lost), it asks again. Never a
/// refund on a timer: the relay may have kept the build, and the list is the only word on it (the
/// review of increment 5, finding 2). A frame out of the player's view is sent all the same, so
/// this settles wherever they stand.
fn timeouts(cx: &mut Ctx) {
    let now = cx.sb.now;
    let timeout = f64::from(shared::PENDING_TIMEOUT_S);
    let server = cx.sb.server.clone();
    let due = |sent_at: f64, asked: Option<f64>| now - asked.unwrap_or(sent_at) >= timeout;
    let mut want = Vec::new();
    for p in cx.sb.pending.iter_mut().filter(|p| p.server == server && due(p.sent_at, p.asked)) {
        p.asked = Some(now);
        want.push(p.intent.frame.clone());
    }
    for u in cx.sb.pending_unbuilds.iter_mut().filter(|u| u.server == server && due(u.sent_at, u.asked)) {
        u.asked = Some(now);
        want.push(u.frame.clone());
    }
    for frame in want {
        cx.sb.want_list(&frame);
    }
}

/// A frame's list whose first part came more than `shared::PARTS_WAIT_S` ago and whose other
/// parts have not: they were lost (every part leaves the relay together), so what came is let go
/// and the whole list asked for again (the review of increment 5, finding 1, case B: the part
/// used to wait for good, and the frame never had a list again that session).
fn parts_overdue(cx: &mut Ctx) {
    let now = cx.sb.now;
    let wait = f64::from(shared::PARTS_WAIT_S);
    let overdue: Vec<String> = cx.sb.staging.iter().filter(|(_, s)| now - s.since > wait).map(|(f, _)| f.clone()).collect();
    for frame in overdue {
        cx.sb.staging.remove(&frame);
        cx.sb.want_list(&frame);
    }
}

/// How much later than the relay's own spacing this game asks, seconds: the two clocks measure the
/// same second a little apart, and one asked a hair too soon is turned away.
const PACE_MARGIN_S: f64 = 0.05;

/// Ask for the next list wanted: one at a time, at most once a `shared::PIECES_REQUEST_INTERVAL_MS`
/// (the relay's limit is per player, whichever frame).
fn ask_for_lists(cx: &mut Ctx) {
    let now = cx.sb.now;
    let gap = shared::PIECES_REQUEST_INTERVAL_MS as f64 / 1000.0 + PACE_MARGIN_S;
    if cx.sb.last_list.as_ref().is_some_and(|(_, t)| now - t < gap) {
        return;
    }
    let Some(frame) = cx.sb.lists_wanted.pop_first() else { return };
    cx.sb.last_list = Some((frame.clone(), now));
    cx.sb.outbox.push(wire(&ToRelay::PiecesRequest { frame }));
}

/// The message as it goes on the wire.
fn wire(m: &ToRelay) -> String {
    serde_json::to_string(m).unwrap_or_default()
}

/// Send the next request, at most one each `shared::SEND_INTERVAL_MS`: first one the relay asked
/// to have again (`rate_limited`), then a take-down, then a build. A build goes measured from its
/// frame's corner (`BuildFrame::to_local`); one for a frame this ship does not have comes back.
fn send_next(cx: &mut Ctx) {
    let now = cx.sb.now;
    if now < cx.sb.next_send_at {
        return;
    }
    let message = if let Some(i) = cx.sb.pending.iter().position(|p| p.resend_at.is_some_and(|t| t <= now)) {
        let p = &mut cx.sb.pending[i];
        p.resend_at = None;
        Some(p.message.clone())
    } else if let Some(i) = cx.sb.pending_unbuilds.iter().position(|u| u.resend_at.is_some_and(|t| t <= now)) {
        let u = &mut cx.sb.pending_unbuilds[i];
        u.resend_at = None;
        Some(u.message.clone())
    } else if let Some(unbuild) = cx.sb.unbuilds.pop_front() {
        // The frame the piece stands in, whose list settles the take-down if its answer is lost.
        // A piece that left this game's world meanwhile (its frame out of view, or someone else
        // took it down) is not asked about.
        let frame = entity_of(cx.world, unbuild.piece_id).and_then(|e| cx.world.get::<&SharedPiece>(e).ok().map(|k| k.frame.clone()));
        match frame {
            None => {
                log::info!("Shared building: piece {} left this world before its take-down went; not sent", unbuild.piece_id);
                None
            }
            Some(frame) => {
                let req_id = cx.sb.next_req_id();
                let message = wire(&ToRelay::Unbuild { req_id, piece_id: unbuild.piece_id, permit: None });
                let (session, server) = (cx.sb.session, cx.sb.server.clone());
                cx.sb.pending_unbuilds.push(PendingUnbuild { req_id, unbuild, frame, message: message.clone(), sent_at: now, session, server, asked: None, resend_at: None });
                Some(message)
            }
        }
    } else if let Some(intent) = cx.sb.queue.pop_front() {
        match frame_of(cx.gui, &intent.frame) {
            None => {
                let line = shared::refusal_message(Action::Build, &blueprint_name(cx.data, &intent.blueprint_id), Reason::BadFrame, None);
                refund_build(cx, &intent, line);
                None
            }
            Some(frame) => {
                let req_id = cx.sb.next_req_id();
                let local = frame.to_local(&intent.pose);
                let message = wire(&ToRelay::Build {
                    req_id,
                    frame: intent.frame.clone(),
                    blueprint_id: intent.blueprint_id.clone(),
                    position: local.position.to_array(),
                    rotation: local.rotation.to_array(),
                    scale: local.scale.to_array(),
                    permit: None,
                });
                let (session, server) = (cx.sb.session, cx.sb.server.clone());
                cx.sb.pending.push(PendingBuild { req_id, intent, message: message.clone(), sent_at: now, session, server, asked: None, resend_at: None });
                Some(message)
            }
        }
    } else {
        None
    };
    if let Some(m) = message {
        cx.sb.outbox.push(m);
        cx.sb.next_send_at = now + shared::SEND_INTERVAL_MS as f64 / 1000.0;
    }
}

// ── What the dev IPC reports (engine/ipc.rs) ──────────────────────────────

/// Where a piece's box falls on the screen: this frame's camera, the window's size in physical
/// pixels (the screenshot's space), and the offset the scene pass draws home-frame content at
/// (`EngineState::station_off`).
pub(crate) struct ScreenView {
    pub view_proj: Mat4,
    pub size: [f32; 2],
    pub offset: Vec3,
}

/// The box of `pose` (ship metres, the bottom centre) on the screen, [x0, y0, x1, y1] in the
/// window's pixels, or None when any corner is behind the camera.
pub(crate) fn screen_rect(pose: &Transform, view: &ScreenView) -> Option<[f32; 4]> {
    let half = pose.scale * 0.5;
    let centre = pose.position + view.offset + pose.rotation * Vec3::new(0.0, half.y, 0.0);
    let (mut lo, mut hi) = (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY));
    for i in 0..8u8 {
        let sign = |bit: u8| if i & bit == 0 { -1.0 } else { 1.0 };
        let corner = centre + pose.rotation * (half * Vec3::new(sign(1), sign(2), sign(4)));
        let clip = view.view_proj * corner.extend(1.0);
        if clip.w <= 1.0e-4 {
            return None;
        }
        let ndc = clip.truncate() / clip.w;
        let px = Vec2::new((ndc.x * 0.5 + 0.5) * view.size[0], (0.5 - ndc.y * 0.5) * view.size[1]);
        lo = lo.min(px);
        hi = hi.max(px);
    }
    Some([lo.x, lo.y, hi.x, hi.y])
}

/// One row per piece the server keeps that this game has in its world, in piece order: its id and
/// frame, its blueprint, where it is drawn from (ship metres, the bottom centre), its turn and
/// size, whether it is this player's, whether it is finished (false while a scaffold) and how far
/// it has grown (seconds; its build time once finished); with `view`, its box on the screen
/// (null when behind the camera). What scripts/lib/shared-build-judge.js reads.
pub(crate) fn piece_rows(world: &hecs::World, registry: Option<&BlueprintRegistry>, view: Option<&ScreenView>) -> Vec<serde_json::Value> {
    let mut q = world.query::<(&SharedPiece, &Transform, Option<&Construction>, Option<&Structure>)>();
    let mut rows: Vec<(u64, serde_json::Value)> = q
        .iter()
        .map(|(_e, (k, tf, scaffold, done))| {
            let blueprint_id = scaffold.map(|c| c.blueprint_id.as_str()).or(done.map(|s| s.blueprint_id.as_str())).unwrap_or("");
            let progress = match scaffold {
                Some(c) => Some(c.progress),
                None => registry.and_then(|r| r.get(blueprint_id)).map(|b| b.build_time),
            };
            let mut row = serde_json::json!({
                "piece_id": k.piece_id,
                "frame": k.frame,
                "blueprint_id": blueprint_id,
                "pos": tf.position.to_array(),
                "rot": tf.rotation.to_array(),
                "scale": tf.scale.to_array(),
                "mine": k.mine,
                "built": scaffold.is_none() && done.is_some(),
                "progress": progress,
            });
            if let Some(v) = view {
                row["rect"] = serde_json::json!(screen_rect(tf, v));
            }
            (k.piece_id, row)
        })
        .collect();
    rows.sort_by_key(|(id, _)| *id);
    rows.into_iter().map(|(_, r)| r).collect()
}

/// The backpack, item by item (the probe's `pack`).
pub(crate) fn pack_counts(world: &hecs::World) -> BTreeMap<String, u32> {
    use crate::systems::inventory::Inventory;
    let mut out = BTreeMap::new();
    let mut q = world.query::<(&Inventory, &crate::ecs::components::Controllable)>();
    if let Some((_e, (inv, _))) = q.iter().next() {
        for s in inv.slots.iter().flatten() {
            *out.entry(s.item_id.clone()).or_insert(0) += s.quantity;
        }
    }
    out
}

/// Home storage, item by item (the probe's `storage`): what the automated machines, the build
/// menu and hand crafts count as the home's (`placed::stock_counts`; a store built on a planet is
/// not the home's). A build pays from it once the backpack runs short, and what a take-down gives
/// back lands in it when the backpack has no room (the "Take to backpack" channel sends what does
/// not fit back to storage, lib.rs), so what came back is the backpack's and this together.
pub(crate) fn storage_counts(world: &hecs::World, placed: &[crate::systems::inventory::placed::PlacedItem]) -> BTreeMap<String, u32> {
    let away = crate::systems::construction::uses::planet_store_paths(world);
    crate::systems::inventory::placed::stock_counts(placed, &away).into_iter().filter(|(_, n)| *n > 0).collect()
}

/// This frame's pieces with their screen boxes, for each recorded frame (engine/ipc.rs).
pub(crate) fn recorder_rows(state: &EngineState) -> Vec<serde_json::Value> {
    let (w, h) = state.renderer.viewport_size();
    let view = ScreenView { view_proj: state.camera.view_projection_matrix(), size: [w as f32, h as f32], offset: state.station_off };
    piece_rows(&state.game_world.world, state.data_store.get::<BlueprintRegistry>("blueprint_registry"), Some(&view))
}

/// The probe's `shared_build` (engine/ipc.rs's recorder, done JSON): what the last welcome said
/// this player may do, whether the ship is theirs to edit (`config::ship_editing_for`), every piece
/// the server keeps in this world, how many constructions the save would hold now (shared pieces
/// never among them), the backpack and home storage, what waits for the relay, the placing line
/// under the crosshair, the zone the build editor edits, and every frame's `seq`.
pub(crate) fn probe_json(state: &EngineState) -> serde_json::Value {
    let gui = &state.gui_state;
    let world = &state.game_world.world;
    let (builds, unbuilds, queued) = state.shared_build.counts();
    let in_channel = state.data_store.get::<OutQueue>(shared::OUT_CHANNEL).map_or(0, |q| q.lock().map_or(0, |q| q.len()));
    serde_json::json!({
        "ranks": state.shared_build.ranks,
        "ship_editing": crate::config::ship_editing_for(gui),
        "pieces": piece_rows(world, state.data_store.get::<BlueprintRegistry>("blueprint_registry"), None),
        "save_constructions": crate::save_load::extract_world_save(world).constructions.len(),
        "pack": pack_counts(world),
        "storage": storage_counts(world, &gui.placed_items),
        "pending": { "builds": builds, "unbuilds": unbuilds, "intents": queued + in_channel },
        "hint": gui.build_placing.as_ref().map(|p| p.hint.clone()),
        "editor_zone": gui.ship_structure.as_ref().and_then(|s| s.zones.get(gui.construction_zone)).map(|z| z.id.clone()),
        "frames": state.shared_build.frame_seqs().map(|(f, s)| serde_json::json!({ "frame": f, "seq": s })).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
#[path = "shared_build_tests.rs"]
mod tests;
