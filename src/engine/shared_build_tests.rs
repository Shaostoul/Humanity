//! Tests of the pieces the server keeps, the game's side (shared_build.rs). Each was seen to fail
//! before it passed; the failure is in its comment.

use super::*;
use crate::systems::construction::placement;
use crate::systems::inventory::{placed::PlacedItem, Inventory, ItemRegistry, TransferOp};
use glam::Quat;
use std::sync::Mutex;

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

fn registry() -> BlueprintRegistry {
    BlueprintRegistry::from_ron(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/blueprints/basic.ron"))).unwrap()
}

/// The shipped ship with the home on `plot`.
fn ship(plot: &str) -> ShipStructure {
    ShipStructure::load_and_assemble_shipped(&data_dir(), Some(plot)).expect("the shipped ship assembles")
}

/// The relay's clock, Unix seconds.
const T: f64 = 1_759_500_000.0;

/// A piece as the relay sends it: `local` metres from its frame's corner, unturned, the blueprint's
/// size, kept at `placed_at`.
fn piece(id: u64, bp: &str, local: [f32; 3], placed_at: f64, mine: bool) -> Piece {
    let size = registry().get(bp).unwrap_or_else(|| panic!("{bp} in basic.ron")).size;
    Piece { piece_id: id, blueprint_id: bp.into(), position: local, rotation: [0.0, 0.0, 0.0, 1.0], scale: size, placed_at, mine }
}

/// A frame's whole list in one part.
fn list(frame: &str, seq: u64, pieces: Vec<Piece>) -> FromRelay {
    part(frame, seq, 1, 1, pieces)
}

fn part(frame: &str, seq: u64, part: u32, parts: u32, pieces: Vec<Piece>) -> FromRelay {
    FromRelay::Pieces { frame: frame.into(), seq, server_time: T, part, parts, pieces }
}

fn built(frame: &str, seq: u64, piece: Piece, req_id: Option<u32>) -> FromRelay {
    FromRelay::Built { frame: frame.into(), seq, server_time: T, piece, req_id }
}

fn unbuilt(frame: &str, seq: u64, piece_id: u64, req_id: Option<u32>) -> FromRelay {
    FromRelay::Unbuilt { frame: frame.into(), seq, piece_id, req_id }
}

fn refused(req_id: Option<u32>, action: Action, reason: Reason, why: Option<Why>) -> FromRelay {
    FromRelay::Refused { req_id, action, reason, why, message: "(the relay's words)".into() }
}

/// What the game holds for one test: the books, a world with the player's pack, the data the code
/// reads (blueprints, items, the channels), and the GUI state with the home on `plot`, in a shared
/// world.
struct Game {
    sb: SharedBuild,
    world: hecs::World,
    data: DataStore,
    gui: GuiState,
}

impl Game {
    fn at(plot: &str) -> Game {
        let mut data = DataStore::new();
        data.insert("blueprint_registry", registry());
        data.insert("item_registry", ItemRegistry::from_csv(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/items.csv"))).unwrap());
        data.insert("inventory_transfer_ops", Mutex::new(Vec::<TransferOp>::new()));
        data.insert(shared::OUT_CHANNEL, OutQueue::default());
        data.insert("build_request", Mutex::new(Vec::<BuildRequest>::new()));
        let mut world = hecs::World::new();
        world.spawn((Inventory::new(16), crate::ecs::components::Controllable));
        let mut gui = GuiState::default();
        gui.ship_structure = Some(ship(plot));
        gui.copresence_active = true;
        Game { sb: SharedBuild::default(), world, data, gui }
    }

    fn cx(&mut self) -> Ctx<'_> {
        Ctx { sb: &mut self.sb, world: &mut self.world, data: &self.data, gui: &mut self.gui }
    }

    /// The relay says `m`, written as it goes on the wire.
    fn hear(&mut self, m: &FromRelay) -> bool {
        let v = serde_json::to_value(m).unwrap();
        on_message(&mut self.cx(), &v)
    }

    /// A welcome, as the relay writes it (`ranks` may be missing).
    fn welcome(&mut self, v: serde_json::Value) {
        welcome(&mut self.cx(), &v);
    }

    /// One frame of `dt` real seconds, joined and welcomed.
    fn frame(&mut self, dt: f32) {
        tick_core(&mut self.cx(), dt, true, true);
    }

    /// What went to the relay since the last look.
    fn sent(&mut self) -> Vec<serde_json::Value> {
        self.sb.outbox.drain(..).map(|m| serde_json::from_str(&m).unwrap()).collect()
    }

    /// Every piece the server keeps in the world: (piece id, frame, finished), in id order.
    fn pieces(&self) -> Vec<(u64, String, bool)> {
        let mut out: Vec<(u64, String, bool)> = self
            .world
            .query::<(&SharedPiece, Option<&Structure>)>()
            .iter()
            .map(|(_e, (k, done))| (k.piece_id, k.frame.clone(), done.is_some()))
            .collect();
        out.sort();
        out
    }

    fn entity(&self, id: u64) -> hecs::Entity {
        entity_of(&self.world, id).unwrap_or_else(|| panic!("piece {id} is in the world"))
    }

    /// What was queued back into the backpack.
    fn ops(&self) -> Vec<TransferOp> {
        self.data.get::<Mutex<Vec<TransferOp>>>("inventory_transfer_ops").unwrap().lock().unwrap().clone()
    }

    /// A build the ConstructionSystem paid for, waiting in the channel: `bp` at the floor point
    /// (x, z) of ship metres, posed as the ghost poses it, having spent `spent`.
    fn paid(&mut self, frame: &str, bp: &str, x: f32, z: f32, spent: Spent) -> Transform {
        let reg = registry();
        let pose = placement::placement_pose(reg.get(bp).unwrap(), Vec3::new(x, 0.0, z), 0, &hecs::World::new(), &reg, None);
        let intent = SharedBuildIntent { frame: frame.into(), blueprint_id: bp.into(), pose: pose.clone(), spent };
        self.data.get::<OutQueue>(shared::OUT_CHANNEL).unwrap().lock().unwrap().push(intent);
        pose
    }
}

fn planks(n: u32) -> Spent {
    Spent { pack: vec![("wood_plank_0".into(), n)], storage: Vec::new() }
}

fn plank_op(n: u32) -> TransferOp {
    TransferOp { item_id: "wood_plank_0".into(), qty: n, add: true, ..Default::default() }
}

/// THE GATE, EVERY ROW (plan section 3.4). On the shipped ship with the home on p1: OFFLINE, as
/// before increment 5 (your own yard yes, the Commons and someone else's plot no, the Dev mode
/// anywhere, a guest nowhere); a planet's ground is always the player's own; IN A SHARED WORLD, a
/// shell piece in your own yard goes to the server and a chest stays in your home; a foundation
/// over your plot's line is refused for the plot rule; someone else's plot refuses a wall as theirs
/// and a chest for its kind; the Commons refuses a wall without the ship-editing rank and takes it
/// with the rank, but never a chest; a corridor takes nothing (with the rank its own words); the
/// Dev mode is exempt from nothing; a guest builds on no plot but in the Commons with the rank; and
/// a wall reaching past the Commons' edge is refused for that.
///
/// Seen red 2026-10-05 with the zone and rank arm removed from `gate` (every shared space refused
/// for the rank): `["joined, the rank, a wall in the Commons: want Shared(\"zone:commons\"), got
/// Refused(\"the ship's shared spaces are built by people this server has given that rank\")",
/// "joined guest, the rank, a wall in the Commons: want Shared(\"zone:commons\"), got Refused(\"the
/// ship's shared spaces are built by people this server has given that rank\")", "joined, the rank,
/// a wall past the Commons' edge: want Refused(\"a piece in the ship's shared spaces must stand
/// wholly inside one\"), got Refused(\"the ship's shared spaces are built by people this server has
/// given that rank\")"]`.
#[test]
fn the_gate_in_and_out_of_the_shared_world() {
    let reg = registry();
    let p1 = ship("p1");
    let guest = p1.put_home_away().expect("the home can be put away");
    let none = Ranks::default();
    let rank = Ranks { can_edit_ship: true, take_down_any: false };
    let theirs = |w: Why| Gate::Refused(shared::why_words(Action::Build, Reason::NotAllowed, Some(w)));
    let plot_rule = |g: bool| Gate::Refused(build_place::off_plot_reason(g).to_string());
    let shells = Gate::Refused(shared::only_shell_pieces_words(&reg));
    struct Row<'a> {
        what: &'a str,
        joined: bool,
        dev: bool,
        ship: &'a ShipStructure,
        bp: &'a str,
        at: (f32, f32),
        planet: bool,
        ranks: Ranks,
        want: Gate,
    }
    #[allow(clippy::too_many_arguments)]
    fn row<'a>(what: &'a str, joined: bool, dev: bool, ship: &'a ShipStructure, bp: &'a str, at: (f32, f32), ranks: Ranks, want: Gate) -> Row<'a> {
        Row { what, joined, dev, ship, bp, at, planet: false, ranks, want }
    }
    let (yard, commons, their_plot, corridor) = ((30.0, 20.0), (80.0, 40.0), (30.0, 140.0), (60.0, 40.0));
    let mut rows = vec![
        row("offline, your yard", false, false, &p1, "wood_wall", yard, none, Gate::Private),
        row("offline, the Commons", false, false, &p1, "wood_wall", commons, none, plot_rule(false)),
        row("offline, someone else's plot", false, false, &p1, "wood_wall", their_plot, none, plot_rule(false)),
        row("offline Dev, the Commons", false, true, &p1, "wood_wall", commons, none, Gate::Private),
        row("offline guest, the default plot's yard", false, false, &guest, "wood_wall", yard, none, plot_rule(true)),
        row("joined, your yard, a wall", true, false, &p1, "wood_wall", yard, none, Gate::Shared("plot:p1".into())),
        row("joined, your yard, a chest", true, false, &p1, "storage_chest", yard, none, Gate::Private),
        row("joined, a foundation over your plot's line", true, false, &p1, "wood_foundation", (54.0, 40.0), none, plot_rule(false)),
        row("joined, someone else's plot, a wall", true, false, &p1, "wood_wall", their_plot, none, theirs(Why::NotYourPlot)),
        row("joined, someone else's plot, a chest", true, false, &p1, "storage_chest", their_plot, none, shells.clone()),
        row("joined, the Commons, no rank", true, false, &p1, "wood_wall", commons, none, theirs(Why::ShipRank)),
        row("joined, the rank, a wall in the Commons", true, false, &p1, "wood_wall", commons, rank, Gate::Shared("zone:commons".into())),
        row("joined, the rank, a chest in the Commons", true, false, &p1, "storage_chest", commons, rank, shells.clone()),
        row("joined, a corridor, no rank", true, false, &p1, "wood_wall", corridor, none, plot_rule(false)),
        row("joined, the rank, a corridor", true, false, &p1, "wood_wall", corridor, rank, Gate::Refused(NOT_IN_A_CORRIDOR.into())),
        row("joined Dev, the Commons, no rank", true, true, &p1, "wood_wall", commons, none, theirs(Why::ShipRank)),
        row("joined Dev, someone else's plot", true, true, &p1, "wood_wall", their_plot, none, theirs(Why::NotYourPlot)),
        row("joined guest, the default plot's yard", true, false, &guest, "wood_wall", yard, none, theirs(Why::Guest)),
        row("joined guest, the rank, a wall in the Commons", true, false, &guest, "wood_wall", commons, rank, Gate::Shared("zone:commons".into())),
        // The Commons is x 65 to 99: a wall centred on x 99 running east-west reaches x 101.
        row("joined, the rank, a wall past the Commons' edge", true, false, &p1, "wood_wall", (99.0, 40.0), rank, Gate::Refused(ZONE_EDGE.into())),
    ];
    rows.push(Row { planet: true, ..row("joined, on a planet's ground", true, false, &p1, "wood_wall", commons, none, Gate::Private) });
    let site = PlanetSite { body: "earth".into(), origin: glam::DVec3::new(6.371e6, 0.0, 0.0) };
    let world = hecs::World::new();
    let mut wrong = Vec::new();
    for r in &rows {
        let bp = reg.get(r.bp).unwrap_or_else(|| panic!("{} in basic.ron", r.bp));
        let pose = placement::placement_pose(bp, Vec3::new(r.at.0, 0.0, r.at.1), 0, &world, &reg, None);
        let got = gate(&GateInput {
            joined: r.joined,
            ship_scope: r.dev,
            ship: Some(r.ship),
            registry: Some(&reg),
            bp: Some(bp),
            pose: &pose,
            site: r.planet.then_some(&site),
            ranks: r.ranks,
        });
        if got != r.want {
            wrong.push(format!("{}: want {:?}, got {:?}", r.what, r.want, got));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// THE TAKE-DOWN GATE MIRRORS THE RELAY'S `may_remove`. The home on p1: the holder takes down
/// anything on their own plot, their own piece or a household member's; a visitor takes down
/// nothing on someone else's plot, not even a piece of their own there (no permit until 5b); the
/// Commons needs the ship-editing rank; the server's admins take down anything anywhere; a guest
/// takes down nothing on the plot their home stood on; a frame the ship does not have is nobody's.
/// And F's refusal reads as the take-down sentence of the contract.
///
/// Seen red 2026-10-05 with the visitor row allowed (`take_down_gate`'s last arm `Ok(())`):
/// `["a visitor, the holder's piece on p2: want Err(NotYourPlot), got Ok(())", "a visitor, their own
/// piece on p2 (no permit): want Err(NotYourPlot), got Ok(())", "a guest, the default plot: want
/// Err(NotYourPlot), got Ok(())", "a planet site: want Err(NotYourPlot), got Ok(())"]`.
#[test]
fn take_down_gate_mirrors_may_remove() {
    let p1 = ship("p1");
    let guest = p1.put_home_away().unwrap();
    let kept = |frame: &str, mine: bool| SharedPiece { piece_id: 7, frame: frame.into(), mine };
    let none = Ranks::default();
    let rank = Ranks { can_edit_ship: true, take_down_any: false };
    let admin = Ranks { can_edit_ship: true, take_down_any: true };
    let rows: Vec<(&str, &ShipStructure, SharedPiece, Ranks, Result<(), Why>)> = vec![
        ("the holder, their own piece on their plot", &p1, kept("plot:p1", true), none, Ok(())),
        ("the holder, a household member's piece on their plot", &p1, kept("plot:p1", false), none, Ok(())),
        ("a visitor, the holder's piece on p2", &p1, kept("plot:p2", false), none, Err(Why::NotYourPlot)),
        ("a visitor, their own piece on p2 (no permit)", &p1, kept("plot:p2", true), none, Err(Why::NotYourPlot)),
        ("the Commons without the rank", &p1, kept("zone:commons", true), none, Err(Why::ShipRank)),
        ("the Commons with the rank", &p1, kept("zone:commons", false), rank, Ok(())),
        ("an admin, someone else's plot", &p1, kept("plot:p2", false), admin, Ok(())),
        ("an admin, the Commons", &p1, kept("zone:commons", false), admin, Ok(())),
        ("a guest, the default plot", &guest, kept("plot:p1", true), none, Err(Why::NotYourPlot)),
        ("a planet site", &p1, kept("site:moon-1", true), none, Err(Why::NotYourPlot)),
    ];
    let mut wrong = Vec::new();
    for (what, ship, k, ranks, want) in &rows {
        let got = take_down_gate(k, Some(ship), *ranks);
        if got != *want {
            wrong.push(format!("{what}: want {want:?}, got {got:?}"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
    assert_eq!(
        shared::refusal_message(Action::Unbuild, "Wood Wall", Reason::NotAllowed, Some(Why::NotYourPlot)),
        "Wood Wall not taken down: this is someone else's plot, and you cannot take down what stands on it."
    );
}

/// A FRAME'S LIST REPLACES THAT FRAME AND LEAVES THE OTHERS. Plot p1 holds pieces 1 and 2 and the
/// Commons piece 3; then p1's new list arrives in two parts: nothing changes until the last part,
/// then piece 1 is gone, piece 2 stays the same entity (a listed piece already drawn is kept as it
/// is), piece 5 is new, and the Commons keeps piece 3, the same entity. Each frame's seq is its own
/// list's.
///
/// Seen red 2026-10-05 with a global replace (`replace_frame` taking down every piece the list
/// does not name, whatever its frame): the Commons' list took p1's pieces down, "left: [(3,
/// \"zone:commons\", true)], right: [(1, \"plot:p1\", true), (2, \"plot:p1\", true), (3,
/// \"zone:commons\", true)]".
#[test]
fn a_snapshot_replaces_one_frame_and_leaves_the_others() {
    let mut g = Game::at("p1");
    g.welcome(serde_json::json!({ "type": "game_welcome", "player_id": 3 }));
    let old = T - 100.0;
    g.hear(&list("plot:p1", 4, vec![piece(1, "wood_foundation", [20.0, 0.0, 20.0], old, false), piece(2, "wood_wall", [24.0, 0.0, 20.0], old, false)]));
    g.hear(&list("zone:commons", 7, vec![piece(3, "wood_wall", [11.0, 0.0, 50.0], old, false)]));
    let s = |id: u64, f: &str| (id, f.to_string(), true);
    assert_eq!(g.pieces(), vec![s(1, "plot:p1"), s(2, "plot:p1"), s(3, "zone:commons")]);
    let (two, three) = (g.entity(2), g.entity(3));

    g.hear(&part("plot:p1", 6, 1, 2, vec![piece(2, "wood_wall", [24.0, 0.0, 20.0], old, false)]));
    assert_eq!(g.pieces(), vec![s(1, "plot:p1"), s(2, "plot:p1"), s(3, "zone:commons")], "nothing changes until the last part");
    g.hear(&part("plot:p1", 6, 2, 2, vec![piece(5, "wood_foundation", [30.0, 0.0, 30.0], old, false)]));
    let now = g.pieces();
    assert_eq!(now, vec![s(2, "plot:p1"), s(3, "zone:commons"), s(5, "plot:p1")], "left: {now:?}: the Commons lost its piece to p1's list");
    assert_eq!(g.entity(2), two, "a listed piece already drawn is kept as it is");
    assert_eq!(g.entity(3), three, "another frame's piece is left alone");
    let seqs: Vec<(String, u64)> = g.sb.frame_seqs().map(|(f, s)| (f.clone(), *s)).collect();
    assert_eq!(seqs, vec![("plot:p1".to_string(), 6), ("zone:commons".to_string(), 7)]);
}

/// A DUPLICATE IS IGNORED AND A GAP ASKS FOR THAT FRAME AGAIN. After p2's (empty) list at seq 3:
/// piece 10 built at seq 4 goes up once, though the message comes twice; piece 11 "built" at seq 3
/// (news the list already holds) does not go up; 10 taken down at seq 5 comes down; piece 12 built
/// at seq 8 (three ahead: two messages lost) goes up AND the next frame asks for p2's whole list,
/// once; and news about p7, whose list this game does not hold, waits for it.
///
/// Seen red 2026-10-05 with the seq compare removed from `SharedBuild::step`: "a game_built at a seq
/// the list already holds drew piece 11: [(10, \"plot:p2\", true), (11, \"plot:p2\", true)]".
#[test]
fn a_duplicate_is_ignored_and_a_gap_asks_for_that_frame_again() {
    let mut g = Game::at("p1");
    g.welcome(serde_json::json!({ "type": "game_welcome", "player_id": 3 }));
    let old = T - 100.0;
    let p = |id: u64| piece(id, "wood_wall", [10.0 + id as f32, 0.0, 10.0], old, false);
    g.hear(&list("plot:p2", 3, Vec::new()));
    assert!(g.pieces().is_empty(), "an empty plot");
    g.hear(&built("plot:p2", 4, p(10), None));
    g.hear(&built("plot:p2", 4, p(10), None));
    assert_eq!(g.pieces().len(), 1, "piece 10 once: {:?}", g.pieces());
    g.hear(&built("plot:p2", 3, p(11), None));
    let now = g.pieces();
    assert_eq!(now.len(), 1, "a game_built at a seq the list already holds drew piece 11: {now:?}");
    g.hear(&unbuilt("plot:p2", 5, 10, None));
    g.hear(&unbuilt("plot:p2", 5, 10, None));
    assert!(g.pieces().is_empty(), "10 came down");
    g.frame(0.016);
    assert!(g.sent().is_empty(), "no gap yet, nothing asked");
    g.hear(&built("plot:p2", 8, p(12), None));
    assert_eq!(g.pieces(), vec![(12, "plot:p2".to_string(), true)], "after a gap the news is applied");
    g.frame(0.016);
    g.frame(0.016);
    assert_eq!(g.sent(), vec![serde_json::json!({ "type": "game_pieces_request", "frame": "plot:p2" })], "and p2's whole list asked for, once");
    g.hear(&built("plot:p7", 1, p(13), None));
    assert_eq!(g.pieces().len(), 1, "news about a frame with no list waits for its list");
    let seqs: Vec<(String, u64)> = g.sb.frame_seqs().map(|(f, s)| (f.clone(), *s)).collect();
    assert_eq!(seqs, vec![("plot:p2".to_string(), 8)]);
}

/// OUT OF VIEW TAKES ONLY THAT FRAME'S PIECES DOWN. Plot p1 holds pieces 1 and 2, the Commons
/// piece 3; p1 goes out of view: 1 and 2 come down, 3 stays, and p1 has no list any more, so a
/// piece built there meanwhile is not drawn; p1's next list (when it comes back into view) draws
/// all three of its pieces again.
///
/// Seen red 2026-10-05 with the frame filter removed from `forget_frame` (every shared piece taken
/// down): "the Commons' piece went with p1: left: []", left: `[]`, right: `[(3, "zone:commons",
/// true)]`.
#[test]
fn out_of_view_takes_only_that_frames_pieces_down() {
    let mut g = Game::at("p2");
    g.welcome(serde_json::json!({ "type": "game_welcome", "player_id": 3 }));
    let old = T - 100.0;
    let p = |id: u64| piece(id, "wood_wall", [10.0 + id as f32, 0.0, 10.0], old, false);
    g.hear(&list("plot:p1", 4, vec![p(1), p(2)]));
    g.hear(&list("zone:commons", 2, vec![p(3)]));
    assert_eq!(g.pieces().len(), 3);
    g.hear(&FromRelay::FrameOutOfView { frame: "plot:p1".into() });
    let now = g.pieces();
    assert_eq!(now, vec![(3, "zone:commons".to_string(), true)], "the Commons' piece went with p1: left: {now:?}");
    g.hear(&built("plot:p1", 5, p(4), None));
    assert_eq!(g.pieces().len(), 1, "p1 has no list: its news waits");
    g.hear(&list("plot:p1", 5, vec![p(1), p(2), p(4)]));
    let ids: Vec<u64> = g.pieces().iter().map(|r| r.0).collect();
    assert_eq!(ids, vec![1, 2, 3, 4], "back in view, p1 is drawn again");
}

/// A REFUSAL GIVES BACK EXACTLY WHAT WAS SPENT AND SAYS WHY (plan section 3.5). A Wood Wall paid
/// for with 4 planks from the pack and 2 from home storage goes to the relay. "Too much at once"
/// gives nothing back and sends the same request again half a second later; "one already stands
/// there" gives back the 4 to the pack (through the Take to backpack channel) and the 2 into placed
/// storage (onto the planks in the Barn), and says so in one line; the same refusal again gives
/// nothing more.
///
/// Seen red 2026-10-05 with the refund removed from `refund_build`: "the 4 planks from the pack go
/// back", left: `[]`, right: `[TransferOp { item_id: "wood_plank_0", qty: 4, add: true, wear: 0,
/// quality: 0, age_s: 0.0 }]`.
#[test]
fn a_refusal_refunds_exactly_what_was_spent_and_says_why() {
    let mut g = Game::at("p1");
    g.welcome(serde_json::json!({ "type": "game_welcome", "player_id": 3 }));
    g.gui.placed_items = vec![PlacedItem { key: "wood_plank_0".into(), name: "Wood Plank".into(), qty: 3, container: "0/1".into(), ..Default::default() }];
    let spent = Spent { pack: vec![("wood_plank_0".into(), 4)], storage: vec![("wood_plank_0".into(), 2)] };
    let pose = g.paid("plot:p1", "wood_wall", 30.0, 20.0, spent);
    g.frame(0.3);
    let sent = g.sent();
    assert_eq!(sent.len(), 1, "{sent:?}");
    let (b, req) = (&sent[0], sent[0]["req_id"].as_u64().unwrap() as u32);
    assert_eq!((b["type"].as_str(), b["frame"].as_str(), b["blueprint_id"].as_str()), (Some("game_build"), Some("plot:p1"), Some("wood_wall")));
    assert_eq!(b["position"], serde_json::json!(pose.position.to_array()), "p1's corner is the ship's origin: the same numbers");
    assert!(b.get("permit").is_none(), "no permit on your own plot");

    g.hear(&refused(Some(req), Action::Build, Reason::RateLimited, None));
    g.frame(0.3);
    assert!(g.sent().is_empty() && g.ops().is_empty() && g.gui.pending_notices.is_empty(), "too soon: nothing back, nothing said yet");
    g.frame(0.3);
    assert_eq!(g.sent(), sent, "the same request again, half a second later");

    g.hear(&refused(Some(req), Action::Build, Reason::Occupied, None));
    assert_eq!(g.ops(), vec![plank_op(4)], "the 4 planks from the pack go back");
    assert_eq!(g.gui.placed_items.len(), 1, "{:?}", g.gui.placed_items);
    assert_eq!((g.gui.placed_items[0].qty, g.gui.placed_items[0].container.as_str()), (5, "0/1"), "the 2 from storage go back onto the stored planks");
    assert_eq!(g.gui.pending_notices, vec!["Wood Wall not built: one already stands there. 6 Wood Plank back.".to_string()]);
    assert_eq!(g.sb.counts(), (0, 0, 0), "nothing waits");
    g.hear(&refused(Some(req), Action::Build, Reason::Occupied, None));
    assert_eq!(g.ops().len(), 1, "a second refusal of the same request gives nothing more");
}

/// MY PIECE IS CONFIRMED BY ITS REQ_ID AND BY A LIST. Three Wood Walls paid for in p1's yard go to
/// the relay. The first is answered (`game_built` with its req_id, `mine`): it is settled at once,
/// nothing comes back, and the relay's piece goes up as a scaffold. The other two go unanswered: ten
/// seconds on, p1's list is asked for, once; the list holds the second as ours (its answer was
/// lost: kept, nothing back) and not the third (never kept: its 6 planks come back, and the player
/// is told).
///
/// Seen red 2026-10-05 with the confirmation by req_id removed from `receive`: "the answered build
/// is settled: left: 3, right: 2".
#[test]
fn my_piece_is_confirmed_by_its_req_id_and_by_a_snapshot() {
    let mut g = Game::at("p1");
    g.welcome(serde_json::json!({ "type": "game_welcome", "player_id": 3 }));
    g.hear(&list("plot:p1", 0, Vec::new()));
    for x in [20.0, 24.0, 28.0] {
        g.paid("plot:p1", "wood_wall", x, 20.0, planks(6));
    }
    for _ in 0..3 {
        g.frame(0.3);
    }
    let reqs: Vec<u32> = g.sent().iter().map(|m| m["req_id"].as_u64().unwrap() as u32).collect();
    assert_eq!(reqs.len(), 3, "one build each 250 ms");
    g.hear(&built("plot:p1", 1, piece(21, "wood_wall", [20.0, 0.0, 20.0], T, true), Some(reqs[0])));
    assert_eq!(g.sb.counts().0, 2, "the answered build is settled: left: {}, right: 2", g.sb.counts().0);
    assert_eq!(g.pieces(), vec![(21, "plot:p1".to_string(), false)], "the relay's piece goes up, a scaffold");
    assert!(g.ops().is_empty() && g.gui.pending_notices.is_empty());

    g.frame(11.0);
    g.frame(0.016);
    assert_eq!(g.sent(), vec![serde_json::json!({ "type": "game_pieces_request", "frame": "plot:p1" })], "ten seconds unanswered: p1's list, asked for once");
    g.hear(&list("plot:p1", 2, vec![piece(21, "wood_wall", [20.0, 0.0, 20.0], T, true), piece(22, "wood_wall", [24.0, 0.0, 20.0], T, true)]));
    assert_eq!(g.sb.counts(), (0, 0, 0), "both settled by the list");
    assert_eq!(g.ops(), vec![plank_op(6)], "only the third, never kept, gives its planks back");
    assert_eq!(g.gui.pending_notices, vec!["Wood Wall not built: the server never said it kept it. 6 Wood Plank back.".to_string()]);
}

/// A STALE INDEX ENTRY NEVER DESPAWNS ANOTHER ENTITY. The index can point at an entity that is no
/// longer the piece (a departure takes pieces down without it). Here piece 7's entry points at the
/// player's own wall in the home: when the relay takes 7 down, piece 7 comes down (found by its
/// marker) and the player's wall stays.
///
/// Seen red 2026-10-05 with the SharedPiece id check removed from `despawn_piece` (the index
/// trusted as it stands): "the player's own wall was taken down by a stale index entry".
#[test]
fn a_stale_index_entry_never_despawns_another_entity() {
    let mut g = Game::at("p1");
    g.welcome(serde_json::json!({ "type": "game_welcome", "player_id": 3 }));
    let old = T - 100.0;
    g.hear(&list("plot:p1", 1, vec![piece(7, "wood_wall", [20.0, 0.0, 20.0], old, true), piece(8, "wood_wall", [24.0, 0.0, 20.0], old, true)]));
    let seven = g.entity(7);
    let reg = registry();
    let bp = reg.get("wood_wall").unwrap();
    let pose = placement::placement_pose(bp, Vec3::new(10.0, 0.0, 10.0), 0, &hecs::World::new(), &reg, None);
    let wall = g.world.spawn((pose, Structure { blueprint_id: "wood_wall".into(), health: bp.health, max_health: bp.health, provides: bp.provides.clone(), uid: 1 }));
    g.sb.index.insert(7, wall);
    g.hear(&unbuilt("plot:p1", 2, 7, None));
    assert!(g.world.contains(wall), "the player's own wall was taken down by a stale index entry");
    assert!(!g.world.contains(seven), "piece 7 itself came down");
    assert_eq!(g.pieces(), vec![(8, "plot:p1".to_string(), true)]);
}

/// A WELCOME STARTS THE SHARED PIECES AFRESH AND READS THE RANKS. The first welcome gives the
/// ship-editing rank and the take-down-anything rank. A build goes to the relay and the connection
/// drops before its answer; the next welcome (the reconnect) has no `ranks` (both read false) and
/// takes down every piece of the last session and its lists: news about p1 waits for p1's list,
/// which draws p1's pieces again. That list does not hold the build sent before the welcome, so it
/// was never kept, and its planks come back.
///
/// Seen red 2026-10-05 with `welcome` doing nothing but starting the session (the code before
/// `on_welcome`): "the welcome's ranks: left: Ranks { can_edit_ship: false, take_down_any: false },
/// right: Ranks { can_edit_ship: true, take_down_any: true }".
#[test]
fn a_welcome_starts_the_shared_pieces_afresh_and_reads_the_ranks() {
    let mut g = Game::at("p1");
    g.welcome(serde_json::json!({ "type": "game_welcome", "player_id": 3, "ranks": { "can_edit_ship": true, "take_down_any": true } }));
    assert_eq!(g.sb.ranks, Ranks { can_edit_ship: true, take_down_any: true }, "the welcome's ranks");
    let old = T - 100.0;
    g.hear(&list("plot:p1", 4, vec![piece(1, "wood_wall", [20.0, 0.0, 20.0], old, true)]));
    g.hear(&list("zone:commons", 2, vec![piece(3, "wood_wall", [11.0, 0.0, 50.0], old, false)]));
    g.paid("plot:p1", "wood_foundation", 40.0, 40.0, planks(8));
    g.frame(0.3);
    assert_eq!(g.sent().len(), 1, "the build went out");

    g.welcome(serde_json::json!({ "type": "game_welcome", "player_id": 4 }));
    assert!(g.pieces().is_empty(), "the last session's pieces are still drawn: {:?}", g.pieces());
    assert_eq!(g.sb.ranks, Ranks::default(), "no ranks in the welcome: none");
    assert!(g.sb.frame_seqs().next().is_none(), "no list is held");
    g.hear(&built("plot:p1", 5, piece(2, "wood_wall", [24.0, 0.0, 20.0], old, false), None));
    assert!(g.pieces().is_empty(), "news about p1 waits for p1's list");
    assert!(g.ops().is_empty(), "the build sent before is still waiting");
    g.hear(&list("plot:p1", 5, vec![piece(1, "wood_wall", [20.0, 0.0, 20.0], old, true), piece(2, "wood_wall", [24.0, 0.0, 20.0], old, false)]));
    let ids: Vec<u64> = g.pieces().iter().map(|r| r.0).collect();
    assert_eq!(ids, vec![1, 2]);
    assert_eq!(g.ops(), vec![plank_op(8)], "the build from before, not in the list, gives its planks back");
    assert_eq!(g.gui.pending_notices, vec!["Wood Foundation not built: the server never said it kept it. 8 Wood Plank back.".to_string()]);
}

/// A piece seen young grows from when the relay took it; one older than its build time stands
/// finished (Wave 1B's note), and each carries the relay's id and whether it is ours; a frame's
/// corner is added to its pose. Seen red 2026-10-05 with the age test inverted in `spawn_piece` (a
/// young piece spawned finished): "2 s into a 4 s wall is a scaffold 2 s grown:
/// MissingComponent(MissingComponent(\"humanity_engine::systems::construction::Construction\"))".
#[test]
fn a_piece_grows_from_when_the_relay_took_it() {
    let reg = registry();
    let frames = BuildFrames::of_ship(&ship("p1"));
    let commons = frames.get("zone:commons").unwrap();
    let mut world = hecs::World::new();
    let young = spawn_piece(&mut world, Some(&reg), commons, &piece(5, "wood_wall", [11.0, 0.0, 50.0], T - 2.0, true), T);
    let old = spawn_piece(&mut world, Some(&reg), commons, &piece(6, "wood_wall", [15.0, 0.0, 50.0], T - 4.5, false), T);
    let c = world.get::<&Construction>(young).expect("2 s into a 4 s wall is a scaffold 2 s grown");
    assert!((c.progress - 2.0).abs() < 1e-3 && c.build_time == 4.0, "{} of {}", c.progress, c.build_time);
    drop(c);
    assert!(world.get::<&Structure>(old).is_ok(), "past its build time it stands finished");
    assert_eq!(*world.get::<&SharedPiece>(young).unwrap(), SharedPiece { piece_id: 5, frame: "zone:commons".into(), mine: true });
    assert_eq!(world.get::<&Transform>(young).unwrap().position, Vec3::new(76.0, 0.0, 70.0), "the Commons' corner (65, 0, 20) added");
    assert_eq!(world.get::<&Transform>(old).unwrap().rotation, Quat::IDENTITY);
}
