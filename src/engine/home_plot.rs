//! Your home on the shared ship (increment 1b of docs/design/ship-homes-and-logistics.md).
//!
//! WHY: a relay now hands every player a plot of the mothership and spawns them on it
//! (src/relay/handlers/game_state.rs `assign_home`). The game first builds the home on the
//! ship's default plot (p1, increment 1a), because it does not know its plot until it joins.
//! If the relay then says p2 and the game kept drawing its home at p1, the player would stand
//! in p1 while the relay holds them at p2's front door, 99 to 139 m away: every update they
//! sent would be refused by the relay's 100 m rule, and everyone else would see them frozen.
//! So the two halves land together, and this file is the game's half.
//!
//! On joining, the game tells the relay which ship it draws and where its own home's door is
//! (`add_join_fields`): only a join naming the relay's ship holds a plot (a join naming another
//! ship is refused before anything is spawned), and the player arrives at THEIR door, which
//! they may have moved with the build-mode avatar, not at the default design's. A home with no
//! authored door names none, and both sides take the middle of the plot handed out. The game
//! joins only while aboard (`aboard`: never from a planet or Dev travel, where its camera is
//! not in ship metres), and only once the world has loaded with its ship (`join_step`: on the
//! frame Enter World is pressed the join gate runs before the world loads, and a join from
//! there named no ship). A join is never built without a ship (`add_join_fields`).
//!
//! A `game_welcome` is applied only while the game is still in the shared world and aboard
//! (`accept_welcome`; one that lands after the game stepped out is dropped). Then the game:
//!   1. checks the relay's ship is its own ship (the `ship.hash`, `ShipStructure::ship_hash`):
//!      positions only agree when everyone has the same ship, so on a mismatch it refuses to
//!      join, with one plain sentence that says what to do, kept under the HUD while it holds;
//!   2. moves its `home` zone to the plot the relay named, through 1a's assembly (the ship
//!      file plus the home design on that plot plus the plot's door corridor), rebuilds the
//!      home through the editor's rebuild path, which carries along what the home holds
//!      (`follow_home_box`: the pieces the player built aboard, their parked vehicles, the
//!      hologram and the showroom stage), carries the farm animals and decoration plants with
//!      their machines, starts the editor's undo history again from the moved home (an undo
//!      must never put the home back on a plot that is someone else's), and moves the Respawn
//!      point. A home that cannot stand on that plot refuses to join and gives the plot back;
//!   3. as a guest (the ship is full, or the join named no ship), makes the Commons its
//!      Respawn point and puts its home AWAY, off the ship with all it holds (increment 2,
//!      `ShipStructure::put_home_away`): a guest has no plot, so it draws no home of its own and
//!      every plot is a neighbour's; the home comes back when the game leaves the shared world
//!      (`bring_home_back`) or a later welcome gives it a plot. On its own plot, its own door;
//!   4. stands the player where the relay holds them (their own entry in the welcome's
//!      `world_snapshot`) when `stand_where_held` says so: on an ARRIVAL (the first welcome
//!      since the world loaded, or from another server), on any welcome that spawned them
//!      afresh (`rejoin` false: after stepping out to solo play or Dev travel and back, after
//!      the relay's 90 s grace ran out, after a relay restart, after Respawn), and whenever the
//!      game stands more than `FAR_FROM_HELD_M` from that point. Otherwise (a reconnect inside
//!      the grace, near where the relay kept them) the player keeps walking where they are:
//!      snapping them back to their last update would undo the walk.
//!
//! The Respawn button, in the shared world, goes through the relay (`respawn_through_relay`):
//! the relay still holds the player where they died, which can be over 100 m from their door.
//! Switching to another server leaves the shared world on the one the game joined
//! (`follow_server`), and a server that refused us is tried again after a fresh connection.
//!
//! `plan_welcome` decides, and is pure (tested below); `apply_welcome_home` does it.
//!
//! THE REMEMBERED PLOT (increment 2). Every applied welcome tells the game its plot on that
//! server (`plot_memory_after`), which it keeps per identity and server in its config
//! (`remember_plot`, `plot_memory_key`, AppConfig `home_plots`, with the ship's hash). The next
//! world load builds the home on that plot before it joins (`assemble_for_boot`, `boot_plot`), so
//! a returning player's welcome only confirms it (a Stay). Offline play builds on it too: the
//! plot remembered for the configured server, else the default plot. A server that forgot or
//! released the plot corrects it (a Move, or a guest's put-away), a changed ship is not trusted
//! (the hash), an erased account forgets it (gui/connections.rs), and "Start every session from
//! the default home" does not touch it: that setting is about what the home holds, not where it
//! stands.
//!
//! THE SAVE'S FRAME. Pieces built aboard and parked vehicles are saved where they stand, beside
//! the box of the plot the home stood on (save_load.rs `HomeFrame`, `WorldSave::home_plot_box`).
//! Loading a save carries the ones that stood in the home to wherever the home stands then
//! (`carry_saved_pieces`), whichever plot that is: the default plot at a boot, the player's own
//! plot in a running world. The frame follows the live ship through every rebuild
//! (`follow_home_box`), so a Dev edit of the plots cannot leave it stale.

use crate::ecs::components::Transform;
use crate::engine::state::{ConstructionHistory, EditorSnapshot, EngineState};
use crate::ship::ship_structure::ShipStructure;
use glam::Vec3;

/// The sentence a player reads when the server's ship is not theirs (one copy, shared with
/// the relay, which refuses such a join with it).
pub(crate) const SHIP_MISMATCH: &str = crate::ship::ship_structure::OTHER_SHIP_SENTENCE;

/// The sentence a player reads when the server has no ship at all (relay reason "no_ship").
pub(crate) const NO_SHIP: &str = crate::ship::ship_structure::NO_SHIP_SENTENCE;

/// The sentence a player reads when their OWN ship did not load (the ship file and the copy
/// built into the app both failed, and the legacy layout is showing): there is nowhere aboard
/// to stand and no ship to name in a join. One copy, shared with the relay, which refuses a
/// join naming an empty ship with it (reason "no_ship_named"). Round 4 of the 1b review: this
/// case used to read "this server has a different ship from yours".
pub(crate) const OWN_SHIP: &str = crate::ship::ship_structure::OWN_SHIP_SENTENCE;

/// The sentence a player reads when their account on the server was erased while their figure
/// stood in its shared world (relay reason "account_erased"). One copy, shared with the relay.
pub(crate) const ERASED: &str = crate::ship::ship_structure::ERASED_SENTENCE;

/// What a `game_join_denied` from the relay means for the shared world (net_route.rs), by its
/// `reason`: the sentence to refuse it with, kept under the HUD, with no retry on that server
/// until a fresh connection to it, a switch away and back, or a fresh world load
/// (`refuse_shared_world`). None for a reason that is not ours to act on (a game ban, shown
/// under the admin controls). The ship reasons come before anything was spawned, so there is
/// nothing to leave; "account_erased" comes after the relay took the figure out (round 5 of
/// the 1b review: the erasing game went on showing the shared world, its updates dropped, and
/// Respawn joined it again, claiming a new plot for the account just erased). Pure.
pub(crate) fn join_denied_sentence(reason: &str) -> Option<&'static str> {
    match reason {
        "other_ship" => Some(SHIP_MISMATCH),
        "no_ship" => Some(NO_SHIP),
        "no_ship_named" => Some(OWN_SHIP),
        "account_erased" => Some(ERASED),
        _ => None,
    }
}

/// The sentence for a server whose ship is not ours (reason "other_ship", or a welcome naming
/// another ship). `theirs`: the server's ship hash; `built_in`: the hash of the ship built into
/// this app; `ours_from`: the ship file this game read its own ship from (None: the copy built
/// into the app). Pure.
///
/// The review of increment 4, P6: an installed game writes the ship file into its data folder on
/// its first run and never refreshes it (src/storage.rs `extract_data_if_needed`), the updater
/// swaps only the exe, and the ship is read from the data folder first. So after an update that
/// changes the ship, the game names the OLD ship, and every server running the new one refused
/// it with "update whichever of the app and the server is older", which cannot help: both are
/// new. When the server runs the ship built into this app and ours came from the data folder,
/// that file is the old one: the sentence says so and names it. Otherwise the usual sentence.
pub(crate) fn other_ship_sentence(theirs: Option<&str>, built_in: Option<&str>, ours_from: Option<&std::path::Path>) -> String {
    match (theirs, built_in, ours_from) {
        (Some(t), Some(b), Some(file)) if t == b => format!(
            "Not joining the shared world: this server runs the ship built into this app, but this game read an older ship file from its data folder ({}), which updating the app does not replace, so move that file aside (the app then uses its own) and reconnect.",
            file.display()
        ),
        _ => SHIP_MISMATCH.to_string(),
    }
}

/// `other_ship_sentence` for this game: the ship file it read from `data_dir` (when the data
/// folder has one: `ShipStructure::load_ship_file` reads it first) and, only then and only now,
/// the hash of the ship built into this app, to compare with the server's. That read goes
/// through `built_in_ship_file`, so the log says the built-in copy was read and why (BUG-133:
/// a rig refuses a run that logs it; no rig run is ever refused a ship).
pub(crate) fn other_ship_sentence_here(data_dir: &std::path::Path, theirs: Option<&str>) -> String {
    let file = data_dir.join(crate::ship::ship_structure::SHIP_FILE);
    let ours_from = file.exists().then_some(file.as_path());
    let built_in = ours_from
        .and(theirs)
        .and_then(|_| ShipStructure::built_in_ship_file("read only to compare with the ship of a server that refused ours; this game runs its data folder's copy").ok())
        .map(|s| s.ship_hash());
    other_ship_sentence(theirs, built_in.as_deref(), ours_from)
}

/// The sentence for a welcome that gives a plot with no id, which only a server of another
/// version sends. Round 4 of the 1b review: it used to read as another ship.
pub(crate) const WELCOME_WITHOUT_PLOT_ID: &str = "Not joining the shared world: this server's welcome gave a plot with no id, as a server of another version can, so update whichever of the app and the server is older and reconnect.";

/// How far the game may stand from where the relay holds the player before a welcome stands
/// them back there, metres, whatever else it says: one place for the game and the relay since
/// increment 4 (src/ship/moves.rs), whose speed check grants a reconnect at most this.
pub(crate) use crate::ship::moves::FAR_FROM_HELD_M;

/// A plot box, (min, max) in ship metres.
pub(crate) type PlotBox = (Vec3, Vec3);

/// What a welcome asks of the home and of where the player stands.
#[derive(Debug)]
pub(crate) enum WelcomeHome {
    /// Do not join: the sentence to show. `give_up_plot`: the relay gave us a plot our home
    /// cannot stand on, so we give it back as we leave (`game_leave` with `give_up_plot`).
    Refuse { sentence: String, give_up_plot: bool },
    /// The home already stands on the plot the relay named: nothing moves. `door` is where its
    /// holder arrives (the Respawn point), `stand_at` where the player goes (None: they stay
    /// where they are).
    Stay { door: Option<Vec3>, stand_at: Option<Vec3> },
    /// Move the home: the ship re-assembled on `plot`. `door` is where the holder of that plot
    /// arrives with this home (the Respawn button's point from now on), `stand_at` where the
    /// player goes now.
    Move { ship: ShipStructure, plot: String, door: Vec3, stand_at: Vec3 },
    /// A guest (the ship is full, or the join named no ship). `door` is the Commons arrival (the
    /// Respawn point), None for a ship with no Commons; `stand_at` where the player goes (None:
    /// they stay). `away`: the ship with the home PUT AWAY (`ShipStructure::put_home_away`), when
    /// it stood on a plot; None when it is away already. A guest has no plot of this ship, so it
    /// draws no home of its own: every plot is a neighbour's (increment 2; 1b put the home back
    /// on the default plot, which is someone else's, and drew it there with all it holds).
    /// `join_afresh`: the relay holds the guest on a plot, farther from the Commons than it lets
    /// anyone jump (`guest_stand`), so after standing them in the Commons the game steps out and
    /// joins again, and the relay spawns them there afresh (what Respawn does).
    Guest { door: Option<Vec3>, stand_at: Option<Vec3>, away: Option<ShipStructure>, join_afresh: bool },
}

/// The notice when a guest presses B (engine/editor.rs `toggle_build_editor`): its home is put
/// away, so there is nothing aboard to edit.
pub(crate) const GUEST_NO_EDITOR: &str = "You are a guest on this ship, with no plot of your own, so your home is not aboard to build on; it comes back when you step out of the shared world or the server gives you a plot.";

/// The sentence a player reads when a welcome makes them a guest (first-hour audit 2026-10-04,
/// Blocker 1: becoming a guest showed no message, so a third player on a ship of two plots found
/// their home, its smelter and their Barn simply gone). What it means, and the way back. It starts
/// unlike `GUEST_NO_EDITOR` on purpose: the copresence rig knows that one by its first words.
pub(crate) const GUEST_ARRIVAL: &str = "This server has no home plot free for you, so you are a guest on its ship: your home, with its stations and storage, is put away and you cannot build, but the ship's shared machines are open to you. Your home comes back when you leave the shared world (Characters > Your Homes) or a plot frees up.";

/// Whether this welcome tells the player they are a guest (`GUEST_ARRIVAL`): when it makes them
/// one, that is on an ARRIVAL (`is_arrival`) or after a welcome that was not a guest's
/// (`previous`, `EngineState::last_welcome`: a plot holder whose plot was released). Not again on
/// a reconnect or a Respawn while still a guest, and not on a welcome that only steps out to join
/// afresh (`join_afresh`): the welcome after that join is an arrival and tells them. Pure.
pub(crate) fn guest_notice(plan: &WelcomeHome, arriving: bool, previous: Option<&str>) -> Option<&'static str> {
    match plan {
        WelcomeHome::Guest { join_afresh: false, .. } if arriving || previous != Some("guest") => Some(GUEST_ARRIVAL),
        _ => None,
    }
}

/// What the game knows when a welcome arrives, besides the welcome: the server whose welcome
/// last stood us where it holds us (`EngineState::home_arrived_on`), the server this welcome
/// comes from, and where the camera stands (ship metres, the frame the game sends).
pub(crate) struct WelcomeContext<'a> {
    pub arrived_on: Option<&'a str>,
    pub server: &'a str,
    pub camera: Vec3,
    /// The game's data folder, for the sentence of a welcome naming another ship
    /// (`other_ship_sentence_here`); None: the usual sentence.
    pub data_dir: Option<&'a std::path::Path>,
}

/// The server the game is talking to, as the connection list keys it (normalized, the URL the
/// socket was dialed for, never the server field being edited): the key the co-presence state
/// is kept under (which server we joined, arrived on, or were refused by).
pub(crate) fn active_server_key(gui: &crate::gui::GuiState) -> String {
    let dialed = if gui.connected_server_url.trim().is_empty() { &gui.server_url } else { &gui.connected_server_url };
    crate::gui::pages::chat::norm_server_url(dialed)
}

/// True while the player stands aboard the ship, where the camera is in ship metres: not on
/// a Dev trip (a planet, a moon, a bookmark) and not held in a planet's frame. Only then does
/// the game join the shared world, or stand where a welcome says (the third review of 1b: a
/// welcome that reached a player on a planet moved them by ship coordinates in the planet's
/// frame).
pub(crate) fn aboard(state: &EngineState) -> bool {
    !state.gui_state.dev_travel_away && state.frame_lock_body.is_none()
}

/// Whether a `game_welcome` is applied: only while the game is in the shared world (it joined
/// and has not stepped out to solo play) and aboard. A welcome that lands after the game
/// stepped out (one round trip after its join), or reaches it away from the ship, is dropped:
/// applying it yanked the player to their door and could move the home while they were out of
/// the world (the third review of 1b). Pure.
pub(crate) fn accept_welcome(joined: bool, solo: bool, aboard: bool) -> bool {
    joined && !solo && aboard
}

/// What the join gate sees on a frame (lib.rs, the co-presence block).
#[derive(Debug, Clone, Copy)]
pub(crate) struct JoinGate {
    /// The world view: no page, showroom or editor open.
    pub in_world: bool,
    /// Already in the shared world on our side.
    pub joined: bool,
    /// The connection finished its identify handshake (the relay drops anything before it).
    pub identified: bool,
    /// Stepped out to solo play.
    pub solo: bool,
    /// Aboard the ship (`aboard`).
    pub aboard: bool,
    /// This server refused us, and the refusal still holds.
    pub refused_here: bool,
    /// The 3D world has loaded (`EngineState::world_loaded`).
    pub world_loaded: bool,
    /// The ship assembled (`GuiState::ship_structure`).
    pub has_ship: bool,
}

/// What the join gate does this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JoinStep {
    Wait,
    Join,
    /// The world loaded without a ship of its own (the legacy layout): say so under the HUD,
    /// once per connection, instead of joining.
    RefuseOwnShip,
}

/// The join gate. Pure. Round 4 of the 1b review: a returning player's game identifies on the
/// main menu, so on the frame Enter World (or Play) is pressed the gate ran BEFORE the world
/// loaded (lib.rs runs the co-presence block ahead of `load_world` in the same frame) and sent
/// a join naming no ship, which the relay refused as another ship; the refusal landed after the
/// world load had cleared refusals, and held until a reconnect. Now it waits for the world and
/// its ship, and a world with no ship of its own says why instead of joining.
pub(crate) fn join_step(g: &JoinGate) -> JoinStep {
    if !g.in_world || g.joined || !g.identified || g.solo || !g.aboard || g.refused_here {
        return JoinStep::Wait;
    }
    if !g.world_loaded {
        return JoinStep::Wait;
    }
    if !g.has_ship {
        return JoinStep::RefuseOwnShip;
    }
    JoinStep::Join
}

impl<'a> WelcomeContext<'a> {
    /// Read from the running game.
    fn of(state: &'a EngineState, server: &'a str) -> Self {
        WelcomeContext { arrived_on: state.home_arrived_on.as_deref(), server, camera: state.camera.position, data_dir: Some(&state.data_dir) }
    }
}

/// True when a welcome from `server` is an ARRIVAL: the first since the world loaded
/// (`arrived_on` None, world_load clears it) or from another server than the one whose welcome
/// last stood us where it holds us. Pure.
pub(crate) fn is_arrival(arrived_on: Option<&str>, server: &str) -> bool {
    arrived_on != Some(server)
}

/// Whether a welcome stands the player where the relay holds them. `arriving`: `is_arrival`.
/// `rejoin`: the welcome's own word that the relay found them still in the world (false: it
/// spawned them afresh, at their door or in the Commons, wherever the game stands). `gap_m`:
/// how far the camera stands from where the relay holds them (None: the welcome says nothing
/// about us). Pure; the second review of 1b found the first rule ("only on an arrival") froze
/// every player who stepped out to solo play and back.
pub(crate) fn stand_where_held(arriving: bool, rejoin: bool, gap_m: Option<f32>) -> bool {
    arriving || !rejoin || gap_m.is_some_and(|d| d > FAR_FROM_HELD_M)
}

/// Decide what a `game_welcome` asks of the home, from the ship the game runs now (`None` when
/// no ship assembled and the legacy layout is showing) and the game's `ctx`. Pure.
pub(crate) fn plan_welcome(ship: Option<&ShipStructure>, welcome: &serde_json::Value, ctx: &WelcomeContext) -> WelcomeHome {
    let refuse = |sentence: &str, give_up_plot: bool| WelcomeHome::Refuse { sentence: sentence.to_string(), give_up_plot };
    let theirs = welcome.get("ship").and_then(|s| s.get("hash")).and_then(|h| h.as_str());
    let Some(ship) = ship else { return refuse(OWN_SHIP, false) };
    if theirs != Some(ship.ship_hash().as_str()) {
        let sentence = ctx.data_dir.map_or_else(|| SHIP_MISMATCH.to_string(), |d| other_ship_sentence_here(d, theirs));
        return refuse(&sentence, false);
    }
    // Where the relay holds us right now (our own entry in the snapshot), and whether to stand
    // there.
    let held = relay_holds_us(welcome);
    let rejoin = welcome.get("rejoin").and_then(|r| r.as_bool()).unwrap_or(false);
    let stand = stand_where_held(is_arrival(ctx.arrived_on, ctx.server), rejoin, held.map(|h| h.distance(ctx.camera)));
    let stand_at = |door: Option<Vec3>| if stand { held.or(door) } else { None };
    let Some(hp) = welcome.get("home_plot").filter(|v| !v.is_null()) else {
        let door = ship.guest_spawn();
        let (stand_at, join_afresh) = guest_stand(ship, held, door, ctx.camera, stand);
        return WelcomeHome::Guest { door, stand_at, away: ship.put_home_away(), join_afresh };
    };
    let Some(id) = hp.get("id").and_then(|i| i.as_str()) else {
        return refuse(WELCOME_WITHOUT_PLOT_ID, false);
    };
    if ship.home.as_ref().is_some_and(|a| a.plot == id) {
        let door = own_door(ship);
        return WelcomeHome::Stay { door, stand_at: stand_at(door) };
    }
    let (Some(design), Some(plot)) = (ship.home_design(), ship.plots.iter().find(|p| p.id == id)) else {
        log::warn!("Co-presence: the plot {id:?} this server gave us is not one our ship has, or our ship has no home design");
        return refuse(&cannot_place_sentence(id), true);
    };
    let door = ShipStructure::plot_spawn(plot, &design);
    match ship.ship_file().assemble(design, id) {
        // The old home is gone from under the player, so they always go somewhere: where the
        // relay holds them, or this home's door on the new plot.
        Ok(moved) => WelcomeHome::Move { ship: moved, plot: id.to_string(), door, stand_at: held.unwrap_or(door) },
        // The plot this server gave us cannot hold our home: give it back (`give_up_plot`) so
        // it is not held for good by someone who never lives there. What did not fit goes to
        // the log: it can be a sentence of its own, and the refusal is one.
        Err(e) => {
            log::warn!("Co-presence: our home does not fit plot {id:?}: {e}");
            refuse(&does_not_fit_sentence(id), true)
        }
    }
}

/// Where a guest's welcome stands them (`plan_welcome`), and whether the game then steps out and
/// joins again (`WelcomeHome::Guest::join_afresh`). `held`: where the relay holds them; `door`: the
/// Commons arrival (None for a ship with no Commons); `camera`: where the game stands; `stand`:
/// `stand_where_held`. Pure.
///
/// A guest has no plot, so it never stands on one: every plot is a neighbour's, drawn with no
/// collision (src/ship/neighbours.rs), and a shared zone's wall is solid across a neighbour's
/// corridor, so a guest left on a plot cannot walk back out (and has no Respawn button while
/// alive). Increment 2 review, findings 1 and 9: a guest whose connection dropped got its home
/// back on the default plot and walked into it, and its welcome inside the grace (`stand` false:
/// not an arrival, a rejoin, under the backstop) left it there when the home was put away again.
/// So wherever the welcome would leave a guest (where the relay holds it, or where it stands when
/// nobody moves it) is on a plot (`on_plot_ground`), it goes where the relay holds it when that is
/// off every plot, else to the Commons. When the relay holds it on a plot farther than
/// `FAR_FROM_HELD_M` from the Commons, every update from there would be refused, so the game also
/// steps out and joins again: the relay then spawns it in the Commons afresh.
pub(crate) fn guest_stand(ship: &ShipStructure, held: Option<Vec3>, door: Option<Vec3>, camera: Vec3, stand: bool) -> (Option<Vec3>, bool) {
    let wanted = if stand { held.or(door) } else { None };
    if !on_plot_ground(ship, wanted.unwrap_or(camera)) {
        return (wanted, false);
    }
    match held.filter(|h| !on_plot_ground(ship, *h)).or(door) {
        Some(at) => (Some(at), held.is_some_and(|h| h.distance(at) > FAR_FROM_HELD_M)),
        // No Commons, and the relay holds us on a plot: nowhere better to stand.
        None => (wanted, false),
    }
}

/// True when `p` stands on a plot's ground: over a plot's box across the floor (`over_plot`), or
/// inside the door corridor a plot's home makes to its street (from the plot's own box,
/// `ShipStructure::plot_door_tube`). Pure.
pub(crate) fn on_plot_ground(ship: &ShipStructure, p: Vec3) -> bool {
    ship.plots.iter().any(|plot| {
        over_plot(p, plot.aabb()) || ship.plot_door_tube(plot, None).is_ok_and(|g| in_corridor(&g, p))
    })
}

/// True when `p` stands inside corridor tube `g` across the floor (x and z), with a centimetre of
/// slack, the way `over_plot` tests a plot. Pure.
fn in_corridor(g: &crate::ship::ship_structure::CorridorGeom, p: Vec3) -> bool {
    use crate::ship::ship_structure::CorridorAxis;
    let (along, across) = match g.axis {
        CorridorAxis::X => (p.x, p.z),
        CorridorAxis::Z => (p.z, p.x),
    };
    let (lo, hi) = (g.start.min(g.end), g.start.max(g.end));
    along >= lo - 0.01 && along <= hi + 0.01 && (across - g.lat).abs() <= g.width * 0.5 + 0.01
}

/// A plot id as a refusal shows it: the id when it is a short plain name (the ship file's
/// `p1`, `p2`), else "unnamed", so an id a server sends can never add a sentence of its own.
fn plain_plot_id(id: &str) -> &str {
    let plain = !id.is_empty() && id.len() <= 32 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if plain {
        id
    } else {
        "unnamed"
    }
}

/// The refusal for a plot our home cannot be placed on: the plot is not one our ship has, or
/// our ship has no home design. The plot goes back (`give_up_plot`).
fn cannot_place_sentence(id: &str) -> String {
    format!(
        "Not joining the shared world: your home could not be placed on the plot this server gave you ({}), so the plot went back to the server; restart the app, and if it happens again the reason is in logs/run.log.",
        plain_plot_id(id)
    )
}

/// The refusal for a plot our home does not fit (the assembly refused it). The plot goes back.
fn does_not_fit_sentence(id: &str) -> String {
    format!(
        "Not joining the shared world: your home does not fit the plot this server gave you ({}), so the plot went back to the server; change your home's design to fit this ship's plots and reconnect, and logs/run.log says which part did not fit.",
        plain_plot_id(id)
    )
}

/// Where the holder of the plot this home stands on arrives with it: its door there, else the
/// middle of the plot (`ShipStructure::plot_spawn`, what the relay spawns at too). None for a
/// ship that was not assembled with a home.
fn own_door(ship: &ShipStructure) -> Option<Vec3> {
    Some(ShipStructure::plot_spawn(ship.home_plot()?, &ship.home_design()?))
}

/// Our own position in a welcome's `world_snapshot` (the entry whose `entity_id` is the
/// welcome's `player_id`): where the relay holds us. None when the welcome carries no such
/// entry. Ship metres at eye height, the frame the game sends its camera in.
fn relay_holds_us(welcome: &serde_json::Value) -> Option<Vec3> {
    let me = welcome.get("player_id")?.as_u64()?;
    let entry = welcome
        .get("world_snapshot")?
        .as_array()?
        .iter()
        .find(|e| e.get("entity_id").and_then(|i| i.as_u64()) == Some(me))?;
    let p = entry.get("position")?.as_array()?;
    let n = |i: usize| p.get(i).and_then(|v| v.as_f64()).map(|v| v as f32);
    let at = Vec3::new(n(0)?, n(1)?, n(2)?);
    at.is_finite().then_some(at)
}

/// The two fields our `game_join` carries for the plot (lib.rs, the join): `ship_hash`, the
/// ship we draw, and `home_spawn`, our own home's door in plot-local metres
/// (`ShipStructure::home_arrival_local`), left out when the home has no authored door.
/// False, and nothing added, when there is no ship: a join must never go out naming none
/// (round 4 of the 1b review: an empty hash went out on the frame Enter World was pressed,
/// before the world loaded, and the relay refused it as another ship).
#[must_use]
pub(crate) fn add_join_fields(join: &mut serde_json::Value, ship: Option<&ShipStructure>) -> bool {
    let (Some(obj), Some(ship)) = (join.as_object_mut(), ship) else { return false };
    obj.insert("ship_hash".into(), serde_json::json!(ship.ship_hash()));
    if let Some((x, z)) = ship.home_arrival_local() {
        obj.insert("home_spawn".into(), serde_json::json!([x, z]));
    }
    true
}

/// Send `game_leave` on the active connection, with `give_up_plot` when asked.
fn send_game_leave(state: &EngineState, give_up_plot: bool) {
    if let Some(ws) = state.gui_state.ws_client.as_ref() {
        let msg = if give_up_plot {
            serde_json::json!({ "type": "game_leave", "give_up_plot": true })
        } else {
            serde_json::json!({ "type": "game_leave" })
        };
        ws.send(&msg.to_string());
    }
}

/// Out of the shared world on our side: not joined, not welcomed, no host clock, and the other
/// players and the relay's crew gone from our world (they are stale without the relay's feed;
/// a join brings them back). The caller sends `game_leave` where there is a socket to send it on.
/// A guest's home put away comes back onto the ship (`bring_home_back`): out of the shared
/// world there is nobody whose plot it would stand on.
pub(crate) fn forget_shared_world(state: &mut EngineState) {
    forget_shared_world_keeping_home(state);
    bring_home_back(state);
}

/// `forget_shared_world` with a home put away left away: Respawn steps out and joins again at
/// once (`respawn_through_relay`), and the guest's next welcome would only put it away again.
fn forget_shared_world_keeping_home(state: &mut EngineState) {
    state.game_joined = false;
    state.game_welcomed = false;
    state.gui_state.copresence_active = false;
    // The speed check's books of that session (engine/move_check.rs, increment 4 review M2, M6).
    state.moves.forget_session();
    crate::systems::time::release_host_clock(&state.data_store);
    state.gui_state.copresence_names.clear();
    let gone: Vec<hecs::Entity> = state
        .game_world
        .world
        .query::<hecs::Or<&crate::net::sync::RemotePlayer, &crate::net::sync::RemoteNpc>>()
        .iter()
        .map(|(e, _)| e)
        .collect();
    for e in gone {
        let _ = state.game_world.world.despawn(e);
    }
}

/// Stop joining the shared world on this server and show `sentence`. `leave`: Some(give_up)
/// when the relay put us in the world (we send `game_leave`, with `give_up_plot` when the plot
/// it gave us cannot hold our home), None when it never did (it refused our join, net_route.rs
/// `game_join_denied`). The join gate (lib.rs) skips this server while `copresence_refused`
/// names it, and the HUD keeps the sentence showing (`copresence_refused_note`); it is tried
/// again after a fresh connection to it, a switch to another server and back, or a fresh world
/// load (`follow_server`, world_load.rs), not before (the third review of 1b: "until the world
/// loads afresh" meant until the app restarted, and nothing on screen said so after 12 s).
pub(crate) fn refuse_shared_world(state: &mut EngineState, sentence: String, leave: Option<bool>) {
    log::warn!("Co-presence: {sentence} (ours {:?})", state.gui_state.ship_structure.as_ref().map(|s| s.ship_hash()));
    if let Some(give_up) = leave {
        send_game_leave(state, give_up);
    }
    forget_shared_world(state);
    state.copresence_refused = Some(active_server_key(&state.gui_state));
    state.gui_state.copresence_refused_note = Some(sentence.clone());
    state.gui_state.pending_notices.push(sentence);
}

/// Try the shared world on every server again (a fresh connection, a switch, a world load).
pub(crate) fn clear_refusal(state: &mut EngineState) {
    state.copresence_refused = None;
    state.gui_state.copresence_refused_note = None;
}

/// What a frame's server and connection ask of the shared world (`follow_server`). Pure.
#[derive(Debug, PartialEq)]
pub(crate) struct ServerFollow {
    /// We joined on `prev`, and the game now talks to another server: leave there (on its
    /// parked connection) and forget the shared world here, so the join gate joins this one.
    pub leave_on: Option<String>,
    /// Try this server again even if it refused us: we switched to it, or its connection is
    /// being made afresh (the identify handshake has not finished).
    pub clear_refusal: bool,
}

/// `prev`: the server the last frame talked to (empty on the first frame); `now`: this
/// frame's; `joined`: we are in the shared world (on `prev`); `identified`: this frame's
/// connection has finished its handshake; `erased_here`: this identity's account on `now` was
/// erased (gui/connections.rs `account_erased_here`), so there is no connection being made
/// afresh, only one that will not be made until the person presses Connect. Review of BUG-135
/// option 2, finding 13: a device that was offline during the erase is told so when it next
/// connects, and its socket is never signed in, so `identified` stays false; taking that for a
/// fresh connection cleared the in-world sentence that says how to come back one frame after it
/// was set. Pure; the third review of 1b found a click on a
/// saved server swapped in its live background connection in one frame, so the game never
/// left the first server and never joined the second, while its updates went to a relay that
/// held nothing for it.
pub(crate) fn server_follow(prev: &str, now: &str, joined: bool, identified: bool, erased_here: bool) -> ServerFollow {
    let switched = !prev.is_empty() && prev != now;
    ServerFollow { leave_on: (switched && joined).then(|| prev.to_string()), clear_refusal: switched || (!identified && !erased_here) }
}

/// Whether this identity's account was erased on the server the game talks to: the CONNECTED
/// address (`active_server_key`, as `follow_server`'s `now` is), never the Chat page's server
/// field, which the person may be editing (review of BUG-135 option 2, second round,
/// finding 9: the follow read the field while its `now` came from the connection).
pub(crate) fn erased_on_followed_server(gui: &crate::gui::GuiState) -> bool {
    gui.account_erased_here(&active_server_key(gui))
}

/// Every frame, before the co-presence block (lib.rs): follow the server the game talks to.
/// On a switch while joined, `game_leave` goes on the parked connection to the server we
/// joined (it stays open in the background, so its relay would otherwise hold our figure
/// frozen for good), and the shared world is forgotten here, so the join gate joins the new
/// server and its welcome is an arrival.
pub(crate) fn follow_server(state: &mut EngineState) {
    let now = active_server_key(&state.gui_state);
    let erased_here = erased_on_followed_server(&state.gui_state);
    let f = server_follow(&state.copresence_server, &now, state.game_joined, state.gui_state.ws_identified, erased_here);
    if let Some(old) = &f.leave_on {
        let parked = state.gui_state.connections.iter().find(|c| &c.url == old).and_then(|c| c.ws.as_ref());
        if let Some(ws) = parked.filter(|w| w.is_connected()) {
            ws.send(&serde_json::json!({ "type": "game_leave" }).to_string());
        }
        forget_shared_world(state);
        log::info!("Co-presence: switched from {old} to {now}; left the shared world there");
    }
    if f.clear_refusal && state.copresence_refused.is_some() {
        clear_refusal(state);
    }
    state.copresence_server = now;
}

/// The Respawn button while in the shared world (lib.rs, after it stands the player at
/// `fps_spawn`): the relay still holds them where they died, up to the far end of the ship
/// from their door, and refuses every update more than 100 m from that point, so they would
/// stand frozen at the death spot for everyone else (the third review of 1b). So the game
/// steps out and joins again: the relay spawns them afresh at their door (or the Commons, for
/// a guest), and the welcome stands them there (`rejoin` false). The others see the figure
/// leave and arrive at the door, which is what a respawn is.
pub(crate) fn respawn_through_relay(state: &mut EngineState) {
    if !state.game_joined {
        return;
    }
    send_game_leave(state, false);
    forget_shared_world_keeping_home(state);
    log::info!("Co-presence: respawned; stepping out and joining again so the relay stands us at our door");
}

/// A guest's home comes back onto the ship (increment 2): onto the plot this player remembers
/// for the server the game now talks to (`boot_plot`), else the ship's default plot, through
/// the same move a welcome makes (`move_home`: what it holds goes with it), and Respawn is its
/// door again. Called when the game leaves the shared world (`forget_shared_world`). Nothing
/// when the home is not away, or cannot be placed (logged; it then stays away until the next
/// world load or a welcome with a plot).
fn bring_home_back(state: &mut EngineState) {
    let Some(ship) = state.gui_state.ship_structure.as_ref() else { return };
    let server = active_server_key(&state.gui_state);
    match home_back_from_away(ship, state.gui_state.home_plots.get(&plot_memory_key(&state.gui_state, &server))) {
        None => {}
        Some(Ok(back)) => {
            log::info!("Co-presence: out of the shared world; our home comes back from where it was put away, onto plot {:?}", back.home_plot().map(|p| &p.id));
            let door = own_door(&back);
            move_home(state, back);
            if let Some(d) = door {
                state.fps_spawn = d;
            }
        }
        Some(Err(e)) => log::warn!("Co-presence: our home could not come back from where it was put away ({e}); it stays put away"),
    }
}

/// The ship with a home put away brought back (`bring_home_back`): on the plot `remembered` for
/// the server the game talks to (`boot_plot`), else on the ship's default plot. None when the
/// home is not away; Err when it cannot be placed there. Pure.
pub(crate) fn home_back_from_away(ship: &ShipStructure, remembered: Option<&crate::config::RememberedPlot>) -> Option<Result<ShipStructure, String>> {
    if !ship.home_is_away() {
        return None;
    }
    let file = ship.ship_file();
    let design = ship.home_design()?;
    let plot = boot_plot(&file, remembered).or_else(|| file.default_plot_id())?;
    Some(file.assemble(design, &plot))
}

/// The plot a world load builds the home on (increment 2): the plot this player remembers for
/// the server they are about to join (`remembered`, `GuiState::home_plots`), when it is a plot of
/// `ship_file` and was remembered on this same ship (the same hash: a plot id names a place only
/// on the same ship; after a ship change it is not trusted). None otherwise: the ship's default
/// plot, where 1b built every home. The welcome then confirms the plot (a Stay) or corrects it
/// (a Move, a guest's put-away). Pure.
pub(crate) fn boot_plot(ship_file: &ShipStructure, remembered: Option<&crate::config::RememberedPlot>) -> Option<String> {
    let r = remembered?;
    (r.ship_hash == ship_file.ship_hash() && ship_file.plots.iter().any(|p| p.id == r.plot)).then(|| r.plot.clone())
}

/// The ship a world load runs (engine/world_load.rs): the ship file with this player's home on
/// the plot `boot_plot` picks for the server the game talks to, else on the default plot. A
/// remembered plot the home cannot be put on (a design changed since) falls back to the default
/// plot, with the reason in the log. Err only when there is no ship (the legacy layout shows).
pub(crate) fn assemble_for_boot(data_dir: &std::path::Path, gui: &crate::gui::GuiState) -> Result<ShipStructure, String> {
    let file = ShipStructure::load_ship_file(data_dir)?;
    let server = active_server_key(gui);
    if let Some(plot) = boot_plot(&file, gui.home_plots.get(&plot_memory_key(gui, &server))) {
        match ShipStructure::assemble_from(file.clone(), data_dir, Some(&plot)) {
            Ok(ship) => {
                log::info!("load_world: building the home on plot {plot}, the plot remembered for {server}");
                return Ok(ship);
            }
            Err(e) => log::warn!("load_world: the home does not go on plot {plot}, remembered for {server} ({e}); the default plot instead"),
        }
    }
    ShipStructure::assemble_from(file, data_dir, None)
}

/// The key a plot is remembered under (`GuiState::home_plots`): this identity on `server`, the
/// key the erased-server memory uses (`gui::erased_entry`), so a second identity on the same app
/// does not build its home on the first one's plot, and an erase forgets exactly this entry
/// (gui/connections.rs `account_erased_on_active`). Increment 2 review, finding 3. Pure.
pub(crate) fn plot_memory_key(gui: &crate::gui::GuiState, server: &str) -> String {
    crate::gui::erased_entry(&gui.profile_public_key, server)
}

/// What an applied welcome's plan teaches about this server's plot (`GuiState::home_plots`):
/// Some(Some(plot)) to remember it (a Move to it, or a Stay on the plot the home stands on),
/// Some(None) to forget it (a guest has none; a plot our home does not fit went back), None to
/// leave the memory as it is (a refusal over the ship: the hash check at the next boot already
/// distrusts it). Pure.
pub(crate) fn plot_memory_after(plan: &WelcomeHome, ship: Option<&ShipStructure>) -> Option<Option<String>> {
    match plan {
        WelcomeHome::Move { plot, .. } => Some(Some(plot.clone())),
        WelcomeHome::Stay { .. } => Some(ship.and_then(|s| s.home_plot()).map(|p| p.id.clone())),
        WelcomeHome::Guest { .. } => Some(None),
        WelcomeHome::Refuse { give_up_plot: true, .. } => Some(None),
        WelcomeHome::Refuse { .. } => None,
    }
}

/// Remember (`Some(plot)`) or forget (`None`) this player's plot on `server`, of the ship the
/// game runs, and save the config when that changed it (AppConfig `home_plots`), so the next
/// world load builds the home there.
pub(crate) fn remember_plot(state: &mut EngineState, server: &str, plot: Option<String>) {
    let hash = state.gui_state.ship_structure.as_ref().map(|s| s.ship_hash()).unwrap_or_default();
    let key = plot_memory_key(&state.gui_state, server);
    let plots = &mut state.gui_state.home_plots;
    let changed = match (plot, hash.is_empty()) {
        (Some(plot), false) => {
            let r = crate::config::RememberedPlot { ship_hash: hash, plot };
            plots.insert(key.clone(), r.clone()).as_ref() != Some(&r)
        }
        // No ship, no plot to remember (a welcome is never applied without one).
        (Some(_), true) => false,
        (None, _) => plots.remove(&key).is_some(),
    };
    if changed {
        log::info!("Co-presence: this server's plot is now {:?} (remembered for the next world load)", plots.get(&key).map(|r| &r.plot));
        crate::config::AppConfig::from_gui_state(&state.gui_state).save();
    }
}

/// Act on a `game_welcome` (engine/net_route.rs, before the welcome reaches net_sync). Returns
/// false when the game does not take it (the caller then drops the welcome): refused, or
/// arrived when the game is no longer in the shared world or not aboard (`accept_welcome`).
pub(crate) fn apply_welcome_home(state: &mut EngineState, welcome: &serde_json::Value) -> bool {
    let (joined, solo, on_ship) = (state.game_joined, state.gui_state.copresence_solo, aboard(state));
    if !accept_welcome(joined, solo, on_ship) {
        log::info!("Co-presence: a welcome after leaving the shared world (joined {joined}, solo {solo}, aboard {on_ship}); not applied");
        if joined && !solo {
            // In the world by the relay's count but away from the ship: leave, and join again
            // once aboard (the join gate waits for it).
            send_game_leave(state, false);
            forget_shared_world(state);
        }
        return false;
    }
    let server = active_server_key(&state.gui_state);
    let plan = plan_welcome(state.gui_state.ship_structure.as_ref(), welcome, &WelcomeContext::of(state, &server));
    // What this welcome teaches about our plot on this server, remembered so the next world load
    // builds the home there (increment 2), and what it does with the home, for the rig's probe.
    let memory = plot_memory_after(&plan, state.gui_state.ship_structure.as_ref());
    // Whether it makes us a guest the player has not been told about (first-hour audit
    // 2026-10-04, Blocker 1), read before this welcome's own word replaces the last one.
    let told = guest_notice(&plan, is_arrival(state.home_arrived_on.as_deref(), &server), state.last_welcome);
    state.last_welcome = Some(match &plan {
        WelcomeHome::Refuse { .. } => "refused",
        WelcomeHome::Stay { .. } => "stay",
        WelcomeHome::Move { .. } => "move",
        WelcomeHome::Guest { .. } => "guest",
    });
    state.last_welcome_rejoin = welcome.get("rejoin").and_then(|r| r.as_bool());
    if let Some(plot) = memory {
        remember_plot(state, &server, plot);
    }
    if let Some(sentence) = told {
        state.gui_state.pending_notices.push(sentence.to_string());
    }
    match plan {
        WelcomeHome::Refuse { sentence, give_up_plot } => {
            log::warn!("Co-presence: refusing the welcome (server ship {})", welcome["ship"]);
            refuse_shared_world(state, sentence, Some(give_up_plot));
            return false;
        }
        WelcomeHome::Stay { door, stand_at } => {
            // Respawn brings the player back to their own door on this plot, whatever another
            // server's welcome (a guest's Commons) set it to before.
            if let Some(d) = door {
                state.fps_spawn = d;
            }
            if let Some(at) = stand_at {
                put_player_at(state, at);
            }
        }
        WelcomeHome::Move { ship, plot, door, stand_at } => {
            log::info!("Co-presence: the server gave us plot {plot}; moving the home there");
            move_home(state, ship);
            // Respawn brings the player back to their own door, on this plot.
            state.fps_spawn = door;
            put_player_at(state, stand_at);
        }
        WelcomeHome::Guest { door, stand_at, away, join_afresh } => {
            log::info!("Co-presence: no plot of our own here; joining as a guest in the Commons");
            // A guest has no plot of this ship, so it draws no home of its own: the home is put
            // away, with all it holds, off the ship (increment 2; 1b drew it on the default
            // plot, which is someone else's). Every plot is then drawn as a neighbour's.
            if let Some(ship) = away {
                log::info!("Co-presence: our home is put away while we are a guest");
                move_home(state, ship);
                // A guest cannot build aboard (`GUEST_NO_EDITOR`): an editor opened while the home
                // was still on a plot (pressed between a reconnect and its welcome) shuts, and its
                // close stands the player where this welcome does (lib.rs, `editor_close_spot`).
                if state.gui_state.construction_active {
                    state.gui_state.construction_active = false;
                    state.gui_state.pending_notices.push(GUEST_NO_EDITOR.to_string());
                }
            }
            // Respawn brings them back to the Commons, where the relay put them.
            if let Some(d) = door {
                state.fps_spawn = d;
            }
            if let Some(at) = stand_at {
                put_player_at(state, at);
            }
            if join_afresh {
                // The relay holds us on a plot, too far from the Commons to walk on from (every
                // update would be refused): step out and join again, so it spawns us there.
                log::info!("Co-presence: the relay holds us on a plot as a guest, too far from the Commons; joining again so it spawns us there");
                welcome_settles_the_home(&mut state.game_world.world);
                respawn_through_relay(state);
                return false;
            }
        }
    }
    // Where the home belongs is settled: what stands in it now is the home's.
    welcome_settles_the_home(&mut state.game_world.world);
    state.game_welcomed = true;
    state.home_arrived_on = Some(server);
    true
}

/// Put the home on the plot `ship` was assembled with: the editor's own rebuild, called
/// directly rather than through the dirty flag (a plot assignment is not an edit, so it must
/// not arm the autosave), which carries what the home holds and republishes the save's frame
/// (`follow_home_box`); then the animals and plants go with their machines, and the editor's
/// undo history starts again from the moved home (`history_after_move`).
fn move_home(state: &mut EngineState, ship: ShipStructure) {
    // Where the machines stood before the move, to carry what was placed around them.
    let before = crate::engine::home_meshes::current_placements(state).unwrap_or_default();
    state.gui_state.construction_zone = ship.home_zone_index();
    state.gui_state.ship_structure = Some(ship);
    crate::engine::home_meshes::rebuild_homestead(state);
    let after = crate::engine::home_meshes::current_placements(state).unwrap_or_default();
    carry_anchored_things(state, &machine_shifts(&before, &after));
    let moved = crate::engine::editor::editor_snapshot(state);
    history_after_move(&mut state.construction_history, moved);
}

/// The editor's undo history once the home moved to another plot underneath it: it starts
/// again from the moved home. The third review of 1b: with the editor open when the welcome
/// moved the home, its baseline was the home on the old plot, so one edit and one Ctrl+Z put
/// the home back there (someone else's plot), 99 m from everything the move had carried.
pub(crate) fn history_after_move(h: &mut ConstructionHistory, moved: EditorSnapshot) {
    crate::engine::editor::history_start_from(h, moved);
}

/// The box of the plot the home stands on, (min, max) in ship metres; while the home is put
/// away (a guest, increment 2), the box of the home where it is kept, so the move that puts it
/// away carries what it holds there, a save records that frame, and the move back carries it all
/// onto the plot again (`follow_home_box`, `carry_saved_pieces`).
pub(crate) fn home_box(ship: &ShipStructure) -> Option<PlotBox> {
    if ship.home_is_away() {
        let z = ship.zones.get(ship.home_zone_index())?;
        let o = z.origin_vec();
        return Some((o, o + Vec3::new(z.body.width, z.body.height, z.body.depth)));
    }
    ship.home_plot().map(|p| p.aabb())
}

/// Where the home stands, for the save's frame (save_load.rs `HomeFrame`). None for a ship
/// with no home (the legacy layout).
pub(crate) fn home_frame(ship: &ShipStructure) -> Option<crate::save_load::HomeFrame> {
    Some(crate::save_load::HomeFrame { home: home_box(ship)? })
}

/// Put the home's frame in the DataStore for the save (`save_load::HOME_FRAME_KEY`), from the
/// live ship. Taken away when there is no frame (the legacy layout), so a save then records
/// none.
pub(crate) fn publish_home_frame(state: &mut EngineState) {
    match state.gui_state.ship_structure.as_ref().and_then(home_frame) {
        Some(f) => state.data_store.insert(crate::save_load::HOME_FRAME_KEY, f),
        None => state.data_store.remove(crate::save_load::HOME_FRAME_KEY),
    }
}

/// What a rebuild of the home asks of the save's frame, from the box the frame holds
/// (`published`) and the box of the plot the home stands on now (`live`). Pure.
#[derive(Debug, PartialEq)]
pub(crate) enum HomeBoxChange {
    /// Nothing changed.
    Same,
    /// The plot was resized, made, or taken away, but not moved: republish the frame only.
    Republish,
    /// The home now stands `delta` from where it stood in `from`: carry what it holds along,
    /// then republish.
    Carry { from: PlotBox, delta: Vec3 },
}

pub(crate) fn home_box_change(published: Option<PlotBox>, live: Option<PlotBox>) -> HomeBoxChange {
    match (published, live) {
        (p, l) if p == l => HomeBoxChange::Same,
        (Some(old), Some(new)) if new.0 != old.0 => HomeBoxChange::Carry { from: old, delta: new.0 - old.0 },
        _ => HomeBoxChange::Republish,
    }
}

/// After every rebuild of the home (home_meshes.rs `rebuild_homestead`): when the plot it
/// stands on moved (a relay's welcome, the Dev Plots panel's move of the home's plot, an undo
/// of one), carry what the home holds with it (`carry_with_home`), and keep the save's frame
/// with the live ship either way. The third review of 1b: the frame was published only when
/// the home was placed, so a Dev edit of the plots left every save writing the home's pieces
/// in the wrong frame, and a Dev move of the home's plot moved the home but nothing in it.
pub(crate) fn follow_home_box(state: &mut EngineState) {
    let live = state.gui_state.ship_structure.as_ref().and_then(home_box);
    let published = state.data_store.get::<crate::save_load::HomeFrame>(crate::save_load::HOME_FRAME_KEY).map(|f| f.home);
    match home_box_change(published, live) {
        HomeBoxChange::Same => return,
        HomeBoxChange::Carry { from, delta } => carry_with_home(state, from, delta),
        HomeBoxChange::Republish => {}
    }
    publish_home_frame(state);
}

/// A piece or vehicle that a loaded save left standing where the home now stands, though it
/// was not in the home when the save was written (a truck left on the plot the boot builds the
/// home on, while the home stood on another): the next move of the home leaves it where it is,
/// while it still stands at `at` (`carry_built_pieces`). Every move of the home clears them.
/// The mark is saved with what it marks (`VehicleSave::outside_home`, `ConstructionSave::
/// outside_home`, while it still holds, `still_not_the_homes`) and put back when the save
/// loads: round 4 of the 1b review found a save written before the welcome (on quit, or the
/// periodic save) recorded the truck inside the box of the home it stood in, and the next
/// launch's welcome then took it into the home.
pub(crate) struct NotTheHomes {
    pub at: Vec3,
}

/// True while `mark` still holds for a piece or vehicle standing at `at`: it has not moved
/// since it was marked. Pure.
pub(crate) fn still_not_the_homes(mark: Option<&NotTheHomes>, at: Vec3) -> bool {
    mark.is_some_and(|m| m.at.distance(at) < 0.01)
}

/// After a save was loaded into the world: carry the pieces and vehicles that stood in the
/// home when it was saved (inside `saved`, `WorldSave::home_plot_box`) to the plot the home
/// stands on now (`home`), and mark the ones that now stand over the home but were not in it
/// (`NotTheHomes`), so a move of the home does not take them along. Returns (pieces, vehicles)
/// carried and how many were marked. Nothing happens without both boxes. Pure on the world.
pub(crate) fn carry_saved_pieces(world: &mut hecs::World, saved: Option<[[f32; 3]; 2]>, home: Option<PlotBox>) -> (usize, usize, usize) {
    use crate::systems::construction::{Construction, PlanetSite, Structure};
    let (Some(s), Some(home)) = (saved, home) else { return (0, 0, 0) };
    let saved: PlotBox = (Vec3::from_array(s[0]), Vec3::from_array(s[1]));
    // Not the home's: over the home now, and not in the home's box when saved (planet pieces
    // are never the ship's).
    let mut foreign: Vec<(hecs::Entity, Vec3)> = Vec::new();
    for (e, (t, _built, site)) in world.query::<(&Transform, hecs::Or<&Structure, &Construction>, Option<&PlanetSite>)>().iter() {
        if site.is_none() && over_plot(t.position, home) && !over_plot(t.position, saved) {
            foreign.push((e, t.position));
        }
    }
    for (e, (_v, t)) in world.query::<(&crate::ecs::components::Vehicle, &Transform)>().iter() {
        if over_plot(t.position, home) && !over_plot(t.position, saved) {
            foreign.push((e, t.position));
        }
    }
    let marked = foreign.len();
    for (e, at) in foreign {
        let _ = world.insert_one(e, NotTheHomes { at });
    }
    let (pieces, vehicles) = carry_built_pieces(world, saved, home.0 - saved.0);
    (pieces, vehicles, marked)
}

/// Clear every `NotTheHomes` mark: the home has moved, and they have done their work.
pub(crate) fn clear_not_the_homes(world: &mut hecs::World) {
    let marked: Vec<hecs::Entity> = world.query::<&NotTheHomes>().iter().map(|(e, _)| e).collect();
    for e in marked {
        let _ = world.remove_one::<NotTheHomes>(e);
    }
}

/// A welcome was applied (`apply_welcome_home`): it settled where the home belongs, whichever
/// it was (a move, a stay on the plot the home already stands on, a guest's), so whatever
/// stands in the home now is the home's, and every `NotTheHomes` mark goes. Round 5 of the 1b
/// review: only a move cleared them, so a truck marked at boot and then standing in the home
/// a server let it keep stayed marked, and since round 4 the mark is saved, so it outlived
/// every launch and the truck was left behind by the next move of the home.
pub(crate) fn welcome_settles_the_home(world: &mut hecs::World) {
    clear_not_the_homes(world);
}

/// A save was loaded into the running world (lib.rs, the launcher's character pick and a
/// restored snapshot): carry its home's pieces and vehicles to the plot the home stands on. A
/// save that records no box is read as standing on the live ship's default plot
/// (`saved_or_default_box`).
pub(crate) fn carry_saved_pieces_home(state: &mut EngineState, saved: Option<[[f32; 3]; 2]>) {
    let ship = state.gui_state.ship_structure.as_ref();
    let (pieces, vehicles, marked) = carry_loaded_save(&mut state.game_world.world, &mut state.data_store, saved, ship);
    if pieces + vehicles + marked > 0 {
        log::info!("Loaded save: carried {pieces} built pieces and {vehicles} vehicles to the home's plot ({marked} left where they stood)");
    }
}

/// `carry_saved_pieces_home` on its own parts: carry a save just loaded into the running world
/// (`saved`, its `WorldSave::home_plot_box`) into the home of `ship`. With no home to carry to
/// (the legacy layout, the ship did not assemble) the pieces stay where the save put them, so
/// the box a save of this session records becomes this save's
/// (`save_load::LOADED_HOME_BOX_KEY`, read by `save_load::frame_for_save`), or none for a save
/// that records none. Round 5 of the 1b review: the box the startup save left waiting stayed,
/// so after a snapshot restored on the legacy layout every save recorded the startup save's
/// box while the restored pieces stood in their own. Returns (pieces, vehicles, marked). Pure
/// on the world and the DataStore.
pub(crate) fn carry_loaded_save(
    world: &mut hecs::World,
    data: &mut crate::hot_reload::data_store::DataStore,
    saved: Option<[[f32; 3]; 2]>,
    ship: Option<&ShipStructure>,
) -> (usize, usize, usize) {
    let Some(home) = ship.and_then(home_box) else {
        match saved {
            Some(b) => data.insert(crate::save_load::LOADED_HOME_BOX_KEY, crate::save_load::LoadedHomeBox(b)),
            None => data.remove(crate::save_load::LOADED_HOME_BOX_KEY),
        }
        return (0, 0, 0);
    };
    carry_saved_pieces(world, saved_or_default_box(saved, ship), Some(home))
}

/// The box a loaded save's home stood on: the one it records, else, for a save that records
/// none (one written before increment 1b, or a snapshot from then), the live ship's default
/// plot, where every home stood until 1b (its pieces were saved in that frame). Round 4 of the
/// 1b review: such a save loaded while the home stood on p2 was not carried at all, and later
/// saves then pinned its pieces outside the home. The one line that reads old saves correctly
/// (the pre-launch rule allows it). Pure.
pub(crate) fn saved_or_default_box(saved: Option<[[f32; 3]; 2]>, ship: Option<&ShipStructure>) -> Option<[[f32; 3]; 2]> {
    saved.or_else(|| {
        let ship = ship?;
        let id = ship.default_plot_id()?;
        let (lo, hi) = ship.plots.iter().find(|p| p.id == id)?.aabb();
        Some([lo.to_array(), hi.to_array()])
    })
}

/// The world load, once the home is built (engine/world_load.rs): the save applied at startup
/// (before there was a ship) left the box its home stood on in the DataStore
/// (`save_load::LOADED_HOME_BOX_KEY`); carry its pieces to the plot the home is built on.
pub(crate) fn carry_loaded_save_home(state: &mut EngineState) {
    let home = state.gui_state.ship_structure.as_ref().and_then(home_box);
    if let Some((pieces, vehicles, marked)) = carry_loaded_box(&mut state.game_world.world, &mut state.data_store, home) {
        if pieces + vehicles + marked > 0 {
            log::info!("Loaded save: carried {pieces} built pieces and {vehicles} vehicles to the home's plot ({marked} left where they stood)");
        }
    }
}

/// `carry_loaded_save_home` on its own parts: carry the waiting box's pieces into `home` and
/// let the box go. With no home to carry to (the legacy layout showing because the ship did
/// not assemble) the box STAYS, so every save of that session still records the frame its
/// pieces stand in (`save_load::frame_for_save`; round 4 of the 1b review: it was dropped,
/// and that session's saves recorded none). None when nothing was carried. Pure on the world.
pub(crate) fn carry_loaded_box(
    world: &mut hecs::World,
    data: &mut crate::hot_reload::data_store::DataStore,
    home: Option<PlotBox>,
) -> Option<(usize, usize, usize)> {
    let b = data.get::<crate::save_load::LoadedHomeBox>(crate::save_load::LOADED_HOME_BOX_KEY).copied()?;
    let home = home?;
    data.remove(crate::save_load::LOADED_HOME_BOX_KEY);
    Some(carry_saved_pieces(world, Some(b.0), Some(home)))
}

/// One machine the move carried: where it stood (x, z) and how far it went.
pub(crate) type MachineShift = ((f32, f32), Vec3);

/// Every machine whose position changed between two placements of the same machines (by id),
/// with where it stood and how far it went. The home's machines go with the home; the ship's
/// (the Commons rows) stay, and so are not listed. Pure.
pub(crate) fn machine_shifts(before: &[crate::machines::PlacedMachine], after: &[crate::machines::PlacedMachine]) -> Vec<MachineShift> {
    let after: std::collections::HashMap<&str, &crate::machines::PlacedMachine> = after.iter().map(|m| (m.id.as_str(), m)).collect();
    before
        .iter()
        .filter_map(|b| {
            let a = after.get(b.id.as_str())?;
            let d = Vec3::new(a.pos.0 - b.pos.0, a.pos.1 - b.pos.1, a.pos.2 - b.pos.2);
            (d != Vec3::ZERO).then_some(((b.pos.0, b.pos.2), d))
        })
        .collect()
}

/// How far something anchored at machine position `at` went: that machine's shift, matched on
/// x and z (a sphere's placement is lifted by its radius, the world load's anchor is not, and
/// two machines never share a footprint point across zones, which may not overlap). Zero for
/// an anchor no moved machine stands at. Pure.
pub(crate) fn shift_of(shifts: &[MachineShift], at: Vec3) -> Vec3 {
    shifts
        .iter()
        .find(|((x, z), _)| (x - at.x).abs() < 1e-3 && (z - at.z).abs() < 1e-3)
        .map_or(Vec3::ZERO, |(_, d)| *d)
}

/// True when `p` stands over a plot's floor: inside its box across x and z (height is not
/// bounded, the rule building aboard uses, engine/build_place.rs `outside_own_plot`), with a
/// centimetre of slack for rounding. A built piece may overhang its plot by up to 0.15 m, but
/// its centre, which is what is tested here, is always inside.
pub(crate) fn over_plot(p: Vec3, plot: PlotBox) -> bool {
    let (lo, hi) = plot;
    p.x >= lo.x - 0.01 && p.x <= hi.x + 0.01 && p.z >= lo.z - 0.01 && p.z <= hi.z + 0.01
}

/// Carry the pieces the player built aboard and their parked vehicles standing in the plot
/// `from` by `delta` (the second review of 1b: a chest built at home on p1 stayed in p1, by then
/// someone else's home, with its contents, when the home moved to p2). A piece built on a
/// planet (it carries a `PlanetSite`, its pose is in that site's frame) never moves; neither does
/// anything outside `from` (the ship's own spaces), nor anything a loaded save marked as not
/// the home's that still stands where it was marked (`NotTheHomes`). A vehicle driving to a
/// point in the home drives to the same point of the moved home. Health, contents (filed under
/// the piece's uid) and open doors are kept: the pieces are moved, not built again. Returns
/// (pieces, vehicles) moved. Pure on the world.
pub(crate) fn carry_built_pieces(world: &mut hecs::World, from: PlotBox, delta: Vec3) -> (usize, usize) {
    use crate::systems::construction::{Construction, PlanetSite, Structure};
    if delta == Vec3::ZERO {
        return (0, 0);
    }
    let stays = |t: &Transform, mark: Option<&NotTheHomes>| still_not_the_homes(mark, t.position);
    let mut pieces = 0;
    for (_e, (t, _built, site, mark)) in
        world.query_mut::<(&mut Transform, hecs::Or<&Structure, &Construction>, Option<&PlanetSite>, Option<&NotTheHomes>)>()
    {
        if site.is_none() && over_plot(t.position, from) && !stays(t, mark) {
            t.position += delta;
            pieces += 1;
        }
    }
    let mut vehicles = 0;
    for (_e, (_v, t, route, mark)) in world.query_mut::<(
        &crate::ecs::components::Vehicle,
        &mut Transform,
        Option<&mut crate::ecs::components::VehicleRoute>,
        Option<&NotTheHomes>,
    )>() {
        if over_plot(t.position, from) && !stays(t, mark) {
            t.position += delta;
            vehicles += 1;
            if let Some(r) = route.filter(|r| over_plot(r.dest, from)) {
                r.dest += delta;
            }
        }
    }
    // A pack left where the player fell in the home goes with the home, as a chest built in it
    // does (2026-10-04, systems::death_pack).
    crate::systems::death_pack::carry_aboard(world, |p| over_plot(p, from), delta);
    (pieces, vehicles)
}

/// Where the pieces built aboard and the vehicles stand (ship metres), for the rig's report
/// (`home_things_json`). Planet pieces are left out (their poses are in their site's frame).
/// Pure on the world.
pub(crate) fn built_positions(world: &hecs::World) -> (Vec<Vec3>, Vec<Vec3>) {
    use crate::systems::construction::{Construction, PlanetSite, Structure};
    let pieces = world
        .query::<(&Transform, hecs::Or<&Structure, &Construction>, Option<&PlanetSite>)>()
        .iter()
        .filter(|(_, (_, _, site))| site.is_none())
        .map(|(_, (t, _, _))| t.position)
        .collect();
    let vehicles = world
        .query::<(&crate::ecs::components::Vehicle, &Transform)>()
        .iter()
        .map(|(_, (_, t))| t.position)
        .collect();
    (pieces, vehicles)
}

/// Carry what the world load placed for the home around its machines and the rebuild does
/// not redo, by each machine's own shift (`machine_shifts`), so an animal kept near a ship
/// machine stays put: the farm animals (each grazes around a machine, `Creature.anchor`) and
/// the decoration plants (each scattered around one). Health, yield timers and which animals
/// are still alive are kept: they are moved, not spawned again.
fn carry_anchored_things(state: &mut EngineState, shifts: &[MachineShift]) {
    let mut animals = 0;
    for (_e, (c, t)) in state
        .game_world
        .world
        .query_mut::<(&mut crate::ecs::components::Creature, &mut crate::ecs::components::Transform)>()
    {
        let d = shift_of(shifts, c.anchor);
        if d != Vec3::ZERO {
            c.anchor += d;
            t.position += d;
            animals += 1;
        }
    }
    let mut plants = 0;
    for deco in state.decoration_objects.iter_mut() {
        let d = shift_of(shifts, deco.5);
        if d != Vec3::ZERO {
            deco.2 += d;
            deco.5 += d;
            plants += 1;
        }
    }
    log::info!("Co-presence: carried {animals} animals and {plants} plants with the home's machines");
}

/// Carry what the home holds that is not anchored to a machine, after the plot it stands on
/// moved by `delta` from the box `old_home` (`follow_home_box`):
///   - the pieces the player built aboard and their parked vehicles (`carry_built_pieces`), and
///     then every `NotTheHomes` mark is cleared;
///   - the solar-system hologram: on an assembled ship its centre is the world load's fallback
///     point at the home's corner (no ship room is a hologram room), and it is part of the home
///     (lib.rs draws it "in the home"), so it moves with the home's origin;
///   - the showroom stage and the avatar standing on it (`avatar_base`, the respawner or
///     wardrobe room), when that room is in the home.
fn carry_with_home(state: &mut EngineState, old_home: PlotBox, delta: Vec3) {
    let (pieces, vehicles) = carry_built_pieces(&mut state.game_world.world, old_home, delta);
    clear_not_the_homes(&mut state.game_world.world);
    state.hologram_room_center += delta;
    let b = state.avatar_base;
    let stage_in_home = b.cmpge(old_home.0).all() && b.cmple(old_home.1).all();
    if stage_in_home {
        state.avatar_base += delta;
        let start = state.avatar_obj_start.min(state.placeholder_objects.len());
        for o in state.placeholder_objects[start..].iter_mut() {
            o.2 += delta;
        }
    }
    log::info!(
        "The home's plot moved by {delta:?}: carried {pieces} built pieces, {vehicles} vehicles, the hologram{} with it",
        if stage_in_home { " and the showroom stage" } else { "" }
    );
}

/// Stand the player at `at` (ship metres, eye height): the walking body and the camera
/// (`stand_player_at`). Also what a relay's correction does (engine/move_check.rs).
pub(crate) fn put_player_at(state: &mut EngineState, at: Vec3) {
    let standing = Standing {
        camera: &mut state.camera.position,
        showroom_open: state.gui_state.showroom_active,
        showroom_return: &mut state.showroom_return_pos,
        // The editor's camera, not its panel flag: the close that puts the player back
        // (lib.rs) runs while this is still set, also on the frame the panel was shut.
        editor_open: state.construction_cam_active,
        editor_return: &mut state.construction_return_pos,
        driving: &mut state.driving_vehicle,
        following: &mut state.follow_vehicle,
    };
    stand_player_at(&mut state.game_world.world, standing, at);
}

/// The notice when closing the build editor leaves the player where they stood
/// (`editor_close_spot`), so the build-mode avatar's spot not being used is explained.
pub(crate) const EDITOR_HELD_BACK: &str = "You are back where you stood before building: in the shared world, shutting the editor only puts you at your build spot when it is on your own plot and within 90 m, so walk there.";

/// Where closing the build editor stands the player (lib.rs, the editor's close), and whether
/// it held them back from the editor's own pick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct EditorClose {
    pub at: Vec3,
    /// True when the editor's pick was too far from where the relay holds the player, so they
    /// stay where they stood instead (the caller says so on screen).
    pub held_back: bool,
}

/// Where closing the build editor stands the player. `chosen`: the editor's own pick, the
/// build-mode avatar's spot, else the home's spawn (v0.557: "where I'm at" when I leave build
/// mode); None when it has neither. `back`: where they stood when the editor opened, moved to
/// wherever a welcome stood them while it was open (`stand_player_at`). `joined`: in the shared
/// world, where the relay holds them at `back` (no update goes out while the editor is open,
/// net_route.rs `drive_position_send`) and refuses any update more than 100 m from there.
///
/// Round 5 of the 1b review: B is allowed anywhere aboard, so a player at the far end of First
/// Street who pressed B twice was put at their build spot in the home, about 150 m away, and
/// every update after that was refused: everyone else saw them frozen on the street. So in
/// the shared world a pick more than `FAR_FROM_HELD_M` from `back` leaves them at `back`.
/// Stepping out and joining again (what Respawn does) would not land them on the pick either:
/// a fresh join puts them at their door, which is neither where they stood nor where the
/// avatar stands, and the others would see the figure leave and arrive.
///
/// `home_away`: the home is put away (a guest, increment 2), so a pick in it (the build-mode
/// avatar's spot is in the home) is no place to stand, and they stay at `back` with nothing to
/// explain: the welcome that put the home away shut the editor and said why (`GUEST_NO_EDITOR`).
/// Increment 2 review, finding 1: an editor opened between a reconnect and its guest welcome.
///
/// `own_plot`: the box of the plot the home stands on (None for none). Increment 4: the relay
/// passes this jump, declared as the editor's (`MoveDecl::Editor`, engine/move_check.rs), only
/// onto the player's own plot (src/relay/handlers/move_check.rs), so in the shared world a pick
/// anywhere else (a Dev's build-mode avatar left in a shared zone it was editing) leaves them at
/// `back` too, instead of a jump the relay would correct. Pure.
pub(crate) fn editor_close_spot(chosen: Option<Vec3>, back: Vec3, joined: bool, home_away: bool, own_plot: Option<PlotBox>) -> EditorClose {
    let chosen = chosen.filter(|_| !home_away);
    let on_own_plot = |c: Vec3| own_plot.is_some_and(|(lo, hi)| c.x >= lo.x && c.x <= hi.x && c.z >= lo.z && c.z <= hi.z);
    match chosen {
        Some(c) if !joined || (on_own_plot(c) && c.distance(back) <= FAR_FROM_HELD_M) => EditorClose { at: c, held_back: false },
        Some(_) => EditorClose { at: back, held_back: true },
        None => EditorClose { at: back, held_back: false },
    }
}

/// What standing the player somewhere changes besides the body (`stand_player_at`): the
/// camera, the points the showroom and the build editor close onto, and the vehicle they
/// drive or the follow cam chases.
pub(crate) struct Standing<'a> {
    pub camera: &'a mut Vec3,
    pub showroom_open: bool,
    pub showroom_return: &'a mut Vec3,
    /// The build editor's orbit camera is up (`EngineState::construction_cam_active`).
    pub editor_open: bool,
    /// Where it opened from (`EngineState::construction_return_pos`).
    pub editor_return: &'a mut Vec3,
    pub driving: &'a mut Option<hecs::Entity>,
    pub following: &'a mut Option<hecs::Entity>,
}

/// Out of the vehicle the player drives (its seat freed) and off the one the follow cam
/// chases: each puts the camera back on its vehicle every frame (lib.rs, the drive and the
/// follow cam), which undoes any move of the player. Respawn and `stand_player_at`. Pure on
/// the world.
pub(crate) fn step_out_of_vehicles(world: &mut hecs::World, driving: &mut Option<hecs::Entity>, following: &mut Option<hecs::Entity>) {
    if let Some(veh) = driving.take() {
        if let Ok(mut seat) = world.get::<&mut crate::ecs::components::VehicleSeat>(veh) {
            seat.occupant_key = None;
        }
    }
    *following = None;
}

/// Stand the player at `at` (ship metres, eye height), the way the showcase request's `cam`
/// verb does it (engine/ipc.rs): the walking body, and the camera, since in first person the
/// camera follows the body each frame. Out of any vehicle first (`step_out_of_vehicles`):
/// the drive and the follow cam put the camera back on their vehicle the next frame, and the
/// relay refuses every update from there (round 4 of the 1b review, finding 2). With the
/// character showroom open, its camera is left alone and the point it closes onto moves
/// instead, or closing it would undo the welcome (finding 8). The build editor the same way
/// (round 5, finding 1): its orbit camera is left alone, and the point it closes onto, which
/// `editor_close_spot` judges the build spot against, moves. Pure on the world.
pub(crate) fn stand_player_at(world: &mut hecs::World, s: Standing, at: Vec3) {
    step_out_of_vehicles(world, s.driving, s.following);
    for (_e, (t, _c)) in world.query_mut::<(&mut Transform, &crate::ecs::components::Controllable)>() {
        t.position = at;
    }
    if s.showroom_open {
        *s.showroom_return = at;
    }
    if s.editor_open {
        *s.editor_return = at;
    }
    if !s.showroom_open && !s.editor_open {
        *s.camera = at;
    }
}

/// For the recorder's probe (engine/ipc.rs): the plot the home stands on, every plot of the
/// ship, and where the home's own things are (`home_things_json`), so a rig can check where the
/// game put its camera, its home and the other players.
pub(crate) fn probe_json(ship: Option<&ShipStructure>) -> (serde_json::Value, serde_json::Value) {
    let plot_json = |p: &crate::ship::ship_structure::Plot| {
        serde_json::json!({
            "id": p.id,
            "kind": p.kind,
            "origin": [p.origin.0, p.origin.1, p.origin.2],
            "size": [p.size.0, p.size.1, p.size.2],
        })
    };
    let Some(ship) = ship else { return (serde_json::Value::Null, serde_json::json!([])) };
    let home = ship.home_plot().map(plot_json).unwrap_or(serde_json::Value::Null);
    (home, serde_json::Value::Array(ship.plots.iter().map(plot_json).collect()))
}

/// Where the things that belong to the home stand now, for the rig (verify-copresence --plots
/// judges each one against the plot the game should hold): the Respawn point, the hologram,
/// the showroom stage, every farm animal's grazing point, every decoration plant, every piece
/// built aboard and every parked vehicle.
pub(crate) fn home_things_json(state: &EngineState) -> serde_json::Value {
    let v = |p: Vec3| serde_json::json!([p.x, p.y, p.z]);
    let animals: Vec<serde_json::Value> = state
        .game_world
        .world
        .query::<(&crate::ecs::components::Creature, &crate::systems::livestock::HerdSlot)>()
        .iter()
        .map(|(_, (c, _))| v(c.anchor))
        .collect();
    let plants: Vec<serde_json::Value> = state.decoration_objects.iter().map(|d| v(d.2)).collect();
    let (pieces, vehicles) = built_positions(&state.game_world.world);
    serde_json::json!({
        "respawn": v(state.fps_spawn),
        "hologram": v(state.hologram_room_center),
        "showroom": v(state.avatar_base),
        "animals": animals,
        "plants": plants,
        "structures": pieces.into_iter().map(v).collect::<Vec<_>>(),
        "vehicles": vehicles.into_iter().map(v).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
    }

    /// The game as it boots: the home on the default plot, p1.
    fn booted() -> ShipStructure {
        ShipStructure::load_and_assemble_shipped(&data_dir(), None).expect("the shipped ship assembles")
    }

    /// A welcome as the relay sends it (relay/handlers/msg_handlers.rs handle_game_join), with
    /// the relay holding us (player 7) at `held`, and `rejoin` as the relay says.
    fn welcome_full(plot: Option<&str>, hash: &str, held: Option<[f32; 3]>, rejoin: bool) -> serde_json::Value {
        let ship = booted();
        let home_plot = match plot {
            Some(id) => {
                let p = ship.plots.iter().find(|p| p.id == id).unwrap();
                serde_json::json!({ "id": id, "kind": p.kind, "origin": [p.origin.0, p.origin.1, p.origin.2], "size": [p.size.0, p.size.1, p.size.2] })
            }
            None => serde_json::Value::Null,
        };
        let mut snapshot = vec![serde_json::json!({ "entity_id": 3, "entity_type": "npc", "position": [1.0, 1.0, 1.0] })];
        if let Some(h) = held {
            snapshot.push(serde_json::json!({ "entity_id": 7, "entity_type": "player", "position": h }));
        }
        serde_json::json!({
            "type": "game_welcome", "player_id": 7, "world_snapshot": snapshot, "home_plot": home_plot,
            "ship": { "id": "mothership-1", "hash": hash }, "rejoin": rejoin,
        })
    }

    /// A fresh spawn (rejoin false), as on every first join.
    fn welcome_at(plot: Option<&str>, hash: &str, held: Option<[f32; 3]>) -> serde_json::Value {
        welcome_full(plot, hash, held, false)
    }

    fn welcome(plot: Option<&str>, hash: &str) -> serde_json::Value {
        welcome_at(plot, hash, None)
    }

    const SERVER: &str = "http://127.0.0.1:3210";

    /// The first welcome since the world loaded, the camera at `camera`.
    fn arriving(camera: Vec3) -> WelcomeContext<'static> {
        WelcomeContext { arrived_on: None, server: SERVER, camera, data_dir: None }
    }

    /// A later welcome from the same server (we arrived on it before), the camera at `camera`.
    fn again(camera: Vec3) -> WelcomeContext<'static> {
        WelcomeContext { arrived_on: Some(SERVER), server: SERVER, camera, data_dir: None }
    }

    const P1_DOOR: Vec3 = Vec3::new(53.5, 1.7, 40.5);
    const P2_DOOR: Vec3 = Vec3::new(53.5, 1.7, 139.5);
    // Where a guest arrives: the Commons' spawn (data/blueprints/ship_structure.ron, increment 4).
    const COMMONS: Vec3 = Vec3::new(87.5, 1.7, 67.5);

    /// THE CASE 1b EXISTS FOR: the relay says p2. The home moves to p2 (every room and the
    /// spawn by exactly p2's offset, through 1a's assembly), Respawn becomes p2's door, and the
    /// player stands where the relay holds them, which on a fresh join is that door. Seen red
    /// 2026-10-03 with the Move arm of `plan_welcome` replaced by `Stay` (the 1a client, which
    /// never moved its home): "the relay said p2 and the home did not move: Stay".
    /// Review of BUG-135 option 2, second round, finding 9: the follow reads the erase on the
    /// server the game talks to (the connected address, as `now` is), never the Chat page's
    /// server field, which the person may be editing.
    ///
    /// Seen red 2026-10-04 with the follow reading `server_url` (as on 8695b08d4): "an erase on
    /// the connected server was missed while another address was typed".
    #[test]
    fn the_follow_reads_the_erase_on_the_connected_server_not_the_typed_one() {
        let mut gui = crate::gui::GuiState::default();
        gui.profile_public_key = "ab12cd34".into();
        gui.server_url = "https://erased.example".into();
        gui.connected_server_url = "https://erased.example".into();
        gui.account_erased_on_active(crate::gui::EraseOutcome::Erased);
        gui.server_url = "https://being-typed.example".into();
        assert!(erased_on_followed_server(&gui), "an erase on the connected server was missed while another address was typed");
        let mut gui = crate::gui::GuiState::default();
        gui.profile_public_key = "ab12cd34".into();
        gui.server_url = "https://erased.example".into();
        gui.connected_server_url = "https://erased.example".into();
        gui.account_erased_on_active(crate::gui::EraseOutcome::Erased);
        gui.connected_server_url = "https://fine.example".into();
        assert!(!erased_on_followed_server(&gui), "a server the game does not talk to was taken for the one it does");
    }

    #[test]
    fn a_welcome_naming_another_plot_moves_the_home_there() {
        let ship = booted();
        let hash = ship.ship_hash();
        match plan_welcome(Some(&ship), &welcome_at(Some("p2"), &hash, Some(P2_DOOR.into())), &arriving(P1_DOOR)) {
            WelcomeHome::Move { ship: moved, plot, door, stand_at } => {
                assert_eq!(plot, "p2");
                assert_eq!(moved.home.as_ref().map(|a| a.plot.as_str()), Some("p2"));
                let dz = 99.0; // p2 sits 99 m south of p1 (design section 2.4)
                let home = |s: &ShipStructure| s.zones[s.home_zone_index()].origin;
                assert_eq!(home(&moved), (0.0, 0.0, dz));
                assert_eq!(moved.zones.len(), ship.zones.len());
                // The door: what the relay spawns at, and what the game's own path gives.
                assert!((door - P2_DOOR).length() < 1e-4, "door {door:?}");
                assert!((moved.home_spawn_world().unwrap() - door).length() < 1e-4);
                assert!((stand_at - P2_DOOR).length() < 1e-4, "stand_at {stand_at:?}");
                // The door lies inside p2's box, not p1's.
                let (lo, hi) = moved.home_plot().unwrap().aabb();
                assert!(door.cmpge(lo).all() && door.cmple(hi).all());
                // Moving is not a new ship: the fingerprint holds.
                assert_eq!(moved.ship_hash(), hash);
            }
            other => panic!("the relay said p2 and the home did not move: {other:?}"),
        }
        // A welcome with no entry for us (never sent by the relay, but possible): the door.
        match plan_welcome(Some(&ship), &welcome(Some("p2"), &hash), &arriving(P1_DOOR)) {
            WelcomeHome::Move { stand_at, .. } => assert!((stand_at - P2_DOOR).length() < 1e-4),
            other => panic!("{other:?}"),
        }
    }

    /// The game restarted inside the relay's 90 s grace: its world loaded afresh with the home
    /// on p1, and the relay kept the player where they were, deep in p2 at z 170 (`rejoin`
    /// true). The player stands THERE, not at the door 30 m away, or the figure the others see
    /// would jump. Seen red 2026-10-03 with `stand_at` set to the door (the first 1b client):
    /// "after a restart in the grace the player stands at Vec3(53.5, 1.7, 139.5), the relay
    /// holds them at Vec3(30.0, 1.7, 170.0)".
    #[test]
    fn a_restart_inside_the_grace_stands_where_the_relay_kept_the_player() {
        let ship = booted();
        let deep = Vec3::new(30.0, 1.7, 170.0);
        let w = welcome_full(Some("p2"), &ship.ship_hash(), Some(deep.into()), true);
        match plan_welcome(Some(&ship), &w, &arriving(P1_DOOR)) {
            WelcomeHome::Move { door, stand_at, .. } => {
                assert!((stand_at - deep).length() < 1e-4, "after a restart in the grace the player stands at {stand_at:?}, the relay holds them at {deep:?}");
                assert!((door - P2_DOOR).length() < 1e-4, "Respawn is still the door: {door:?}");
            }
            other => panic!("{other:?}"),
        }
    }

    /// The home already on the plot the relay named: nothing moves, and Respawn is our own door
    /// there. On ARRIVAL the player stands where the relay holds them (their own door, which may
    /// not be where the world load put the camera: the player may have walked before the
    /// connection came up). On a reconnect near where the relay kept them they keep walking.
    /// Seen red 2026-10-03 with the Stay arm never moving anyone (the first 1b client): "an
    /// arrival on our own plot stands at our door: Stay { stand_at: None }"; and (the second
    /// review's Respawn finding) with the Stay arm naming no door: "a welcome on our own plot
    /// leaves Respawn wherever it was: None", so a guest on another server who then came back
    /// as the p1 holder respawned in the Commons.
    #[test]
    fn a_welcome_naming_our_own_plot_moves_nothing() {
        let ship = booted();
        let w = welcome_at(Some("p1"), &ship.ship_hash(), Some(P1_DOOR.into()));
        match plan_welcome(Some(&ship), &w, &arriving(Vec3::new(20.0, 1.7, 30.0))) {
            WelcomeHome::Stay { door, stand_at: Some(at) } => {
                assert!((at - P1_DOOR).length() < 1e-4, "{at:?}");
                assert!(door.is_some_and(|d| (d - P1_DOOR).length() < 1e-4), "a welcome on our own plot leaves Respawn wherever it was: {door:?}");
            }
            other => panic!("an arrival on our own plot stands at our door: {other:?}"),
        }
        let near = welcome_full(Some("p1"), &ship.ship_hash(), Some(P1_DOOR.into()), true);
        match plan_welcome(Some(&ship), &near, &again(P1_DOOR + Vec3::new(3.0, 0.0, 0.0))) {
            WelcomeHome::Stay { door: Some(d), stand_at: None } => assert!((d - P1_DOOR).length() < 1e-4),
            other => panic!("a reconnect near where the relay kept us keeps walking: {other:?}"),
        }
    }

    /// A full ship: home_plot null, and the player arrives in the Commons, at the point the
    /// relay put them (game_state.rs assign_home uses the same `guest_spawn`); the Commons is
    /// their Respawn point, and their home is put away (increment 2).
    #[test]
    fn a_full_ship_makes_a_guest_in_the_commons() {
        let ship = booted();
        match plan_welcome(Some(&ship), &welcome_at(None, &ship.ship_hash(), Some(COMMONS.into())), &arriving(P1_DOOR)) {
            WelcomeHome::Guest { door: Some(d), stand_at: Some(at), away: Some(away), join_afresh: false } => {
                assert!((d - COMMONS).length() < 1e-4, "{d:?}");
                assert!((at - COMMONS).length() < 1e-4, "{at:?}");
                assert!(away.home_is_away(), "the guest's home is put away");
            }
            other => panic!("a full ship should make a guest: {other:?}"),
        }
    }

    /// A guest who reconnects inside the grace (the socket dropped for a moment, `rejoin` true)
    /// keeps walking where they are: the relay still holds them about there, at the end of
    /// street-1 here, 148 m from the Commons. Sending them back to the Commons would put them
    /// 148 m from where the relay holds them: every update refused, frozen for everyone else.
    /// Seen red 2026-10-03 on the first 1b client (the guest arm always went to the Commons):
    /// "a reconnecting guest is moved to Some(Vec3(82.0, 1.7, 47.5))".
    #[test]
    fn a_reconnecting_guest_keeps_walking_where_they_are() {
        let ship = booted();
        let street_end = Vec3::new(70.0, 1.7, 195.0);
        let w = welcome_full(None, &ship.ship_hash(), Some(street_end.into()), true);
        match plan_welcome(Some(&ship), &w, &again(street_end + Vec3::new(0.5, 0.0, 0.0))) {
            WelcomeHome::Guest { door, stand_at, .. } => {
                assert_eq!(stand_at, None, "a reconnecting guest is moved to {stand_at:?}");
                assert!(door.is_some_and(|d| (d - COMMONS).length() < 1e-4), "Respawn is the Commons: {door:?}");
            }
            other => panic!("{other:?}"),
        }
        // The same guest arriving afresh (their game restarted inside the grace) stands where
        // the relay kept them, not at the Commons.
        match plan_welcome(Some(&ship), &w, &arriving(P1_DOOR)) {
            WelcomeHome::Guest { stand_at: Some(at), .. } => assert!((at - street_end).length() < 1e-4, "{at:?}"),
            other => panic!("{other:?}"),
        }
    }

    /// FINDING 1 of the increment 2 review. A guest's connection drops: the game leaves the shared
    /// world, so its home comes back onto the ship (`bring_home_back`), on the default plot p1,
    /// which is someone else's. The guest walks into it, to p1's door, 29 m from where the relay
    /// still holds them in the Commons, and reconnects inside the relay's 90 s grace (`rejoin`
    /// true, not an arrival, under the 90 m backstop). The welcome puts the home away again; it
    /// must also stand the guest back where the relay holds them, as any welcome that takes the
    /// ground from under the player does (a Move always stands). Left where they were, they stand
    /// in a neighbour's home drawn with no collision, the Commons wall at its corridor is solid
    /// again, and everyone else sees them in someone's home. The same in the home's door corridor,
    /// which goes away with the home. Seen red 2026-10-04 on c98c5465b (the guest arm stood the
    /// player only when `stand_where_held` said so): "a guest back inside the grace is left on the
    /// plot its home leaves: None".
    #[test]
    fn a_guest_back_inside_the_grace_is_stood_off_the_plot_its_home_leaves() {
        let ship = booted();
        let w = welcome_full(None, &ship.ship_hash(), Some(COMMONS.into()), true);
        for (camera, where_) in [(P1_DOOR, "at p1's door"), (Vec3::new(60.0, 1.7, 40.0), "in p1's door corridor"), (Vec3::new(10.0, 1.7, 80.0), "deep in p1")] {
            match plan_welcome(Some(&ship), &w, &again(camera)) {
                WelcomeHome::Guest { stand_at, away, join_afresh, .. } => {
                    assert!(away.is_some(), "the home is put away");
                    assert!(!join_afresh, "29 m from where the relay holds them: a walk, not a fresh join");
                    assert!(
                        stand_at.is_some_and(|a| (a - COMMONS).length() < 1e-4),
                        "a guest back inside the grace is left on the plot its home leaves ({where_}): {stand_at:?}"
                    );
                }
                other => panic!("{other:?}"),
            }
        }
    }

    /// FINDING 9 of the increment 2 review: the relay holds the player inside the home they stood
    /// in (p1), and their welcome inside the grace comes back as a guest's (their plot was lost:
    /// a storage error makes a joiner a guest). The home is put away and p1 becomes a neighbour's,
    /// drawn with no collision: left there, they walk through its walls into the gap between the
    /// plots. They go to the Commons. Seen red 2026-10-04 on c98c5465b: "a guest held in the home
    /// it loses keeps standing in it: None".
    #[test]
    fn a_guest_held_inside_the_home_it_loses_goes_to_the_commons() {
        let ship = booted();
        let w = welcome_full(None, &ship.ship_hash(), Some(P1_DOOR.into()), true);
        match plan_welcome(Some(&ship), &w, &again(P1_DOOR)) {
            WelcomeHome::Guest { stand_at, .. } => assert!(
                stand_at.is_some_and(|a| (a - COMMONS).length() < 1e-4),
                "a guest held in the home it loses keeps standing in it: {stand_at:?}"
            ),
            other => panic!("{other:?}"),
        }
    }

    /// The same as a guest held deep in p2, 133 m from the Commons: standing them in the Commons
    /// would put them farther from where the relay holds them than it lets anyone jump, so every
    /// update would be refused (the 1b freeze). They stand in the Commons and the game joins
    /// again (`join_afresh`), so the relay spawns them there afresh, as Respawn does. Seen red
    /// 2026-10-04 with `guest_stand` never asking for a fresh join: "a guest held 133 m from the
    /// Commons walks on from there: false". Held off every plot (the Commons, the street), a
    /// guest never joins afresh.
    #[test]
    fn a_guest_held_far_inside_a_plot_joins_again_to_reach_the_commons() {
        let ship = booted();
        let deep = Vec3::new(30.0, 1.7, 170.0);
        let w = welcome_full(None, &ship.ship_hash(), Some(deep.into()), true);
        match plan_welcome(Some(&ship), &w, &again(deep)) {
            WelcomeHome::Guest { stand_at, join_afresh, .. } => {
                assert!(stand_at.is_some_and(|a| (a - COMMONS).length() < 1e-4), "{stand_at:?}");
                assert!(join_afresh, "a guest held {:.0} m from the Commons walks on from there: {join_afresh}", deep.distance(COMMONS));
            }
            other => panic!("{other:?}"),
        }
        let street_end = Vec3::new(70.0, 1.7, 195.0);
        for held in [COMMONS, street_end] {
            let w = welcome_full(None, &ship.ship_hash(), Some(held.into()), true);
            let plan = plan_welcome(Some(&ship), &w, &again(held));
            assert!(matches!(plan, WelcomeHome::Guest { stand_at: None, join_afresh: false, .. }), "held at {held:?}: {plan:?}");
        }
    }

    /// A player who arrives as a guest (every plot of the ship taken: it has two, so the third
    /// player ever is one) is told so in plain words, what it means and how their home comes
    /// back (first-hour audit 2026-10-04, Blocker 1: "Becoming a guest shows no message"). Once:
    /// not again on a reconnect or a Respawn while still a guest, nor on the welcome that only
    /// steps out to join afresh (the welcome after that join tells them); and a plot holder who
    /// becomes a guest is told.
    ///
    /// Seen red 2026-10-04 on 8e400d7ed (`guest_notice` answering None, as the game said
    /// nothing): "a new guest is told nothing: None"; and broken on purpose the same day with
    /// every guest welcome telling: "a guest is told again on every reconnect".
    #[test]
    fn a_new_guest_is_told_in_plain_words() {
        let ship = booted();
        let hash = ship.ship_hash();
        let arrival = plan_welcome(Some(&ship), &welcome_at(None, &hash, Some(COMMONS.into())), &arriving(P1_DOOR));
        assert!(matches!(arrival, WelcomeHome::Guest { join_afresh: false, .. }), "{arrival:?}");
        let told = guest_notice(&arrival, true, None);
        assert_eq!(told, Some(GUEST_ARRIVAL), "a new guest is told nothing: {told:?}");
        for words in ["guest", "no home plot free", "cannot build", "put away", "Characters > Your Homes"] {
            assert!(GUEST_ARRIVAL.contains(words), "the guest sentence does not say {words:?}: {GUEST_ARRIVAL}");
        }
        // The rig tells the build editor's refusal apart from this one by its first words.
        assert!(!GUEST_ARRIVAL.starts_with(&GUEST_NO_EDITOR[..40]), "the two guest sentences start alike");
        // Once: a reconnect (or a Respawn) while still a guest says nothing more.
        let reconnect = plan_welcome(Some(&ship), &welcome_full(None, &hash, Some(COMMONS.into()), true), &again(COMMONS));
        assert!(matches!(reconnect, WelcomeHome::Guest { join_afresh: false, .. }), "{reconnect:?}");
        assert_eq!(guest_notice(&reconnect, false, Some("guest")), None, "a guest is told again on every reconnect");
        // A plot holder whose plot was released becomes a guest on a reconnect: told.
        assert_eq!(guest_notice(&reconnect, false, Some("stay")), Some(GUEST_ARRIVAL), "a player who just lost their plot is told nothing");
        // Another server's welcome is an arrival, guest before or not.
        assert_eq!(guest_notice(&reconnect, true, Some("guest")), Some(GUEST_ARRIVAL));
        // The welcome that steps out to join afresh says nothing; the next one does.
        let deep = Vec3::new(30.0, 1.7, 170.0);
        let afresh = plan_welcome(Some(&ship), &welcome_full(None, &hash, Some(deep.into()), true), &again(deep));
        assert!(matches!(afresh, WelcomeHome::Guest { join_afresh: true, .. }), "{afresh:?}");
        assert_eq!(guest_notice(&afresh, true, None), None, "told twice across one fresh join");
        // A plot of our own is not a guest's.
        let own = plan_welcome(Some(&ship), &welcome_at(Some("p1"), &hash, Some(P1_DOOR.into())), &arriving(P1_DOOR));
        assert_eq!(guest_notice(&own, true, None), None, "{own:?}");
    }

    /// The rig's guest leg (verify-copresence --plots --order guest) knows the build editor's
    /// refusal by the start of this sentence (scripts/lib/copresence-judge.js
    /// GUEST_NO_EDITOR_START): a rewording must change both, or the leg fails for the wrong
    /// reason. Seen red 2026-10-04 with the judge's start changed to "You are a visitor": "the
    /// rig looks for \"You are a visitor on this ship, with no plot of your own\"".
    #[test]
    fn the_rig_knows_the_guest_editor_sentence() {
        let judge = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/lib/copresence-judge.js")).unwrap();
        let start = judge
            .split("const GUEST_NO_EDITOR_START = \"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .expect("the judge names the sentence's start");
        assert!(start.len() > 20 && GUEST_NO_EDITOR.starts_with(start), "the rig looks for {start:?}");
    }

    /// The build editor opened between a reconnect and its guest welcome (finding 1 of the
    /// increment 2 review, its third route): the welcome puts the home away and shuts the editor,
    /// and the close must not stand the player at their build spot, which is in the home put away
    /// (or, before the home went, on p1, someone else's plot). They stay where the welcome stood
    /// them, with no "walk to your build spot" notice. Seen red 2026-10-04 with `home_away`
    /// ignored: "a build spot in a home put away stood the player at Vec3(53.5, 1.7, 40.5)".
    #[test]
    fn closing_the_editor_with_the_home_put_away_stands_where_the_welcome_did() {
        let c = editor_close_spot(Some(P1_DOOR), COMMONS, true, true, None);
        assert_eq!(c.at, COMMONS, "a build spot in a home put away stood the player at {:?}", c.at);
        assert!(!c.held_back, "nothing to explain: the welcome said why the editor shut");
        let out = editor_close_spot(Some(P1_DOOR), COMMONS, false, true, None);
        assert_eq!(out.at, COMMONS, "out of the shared world too");
    }

    /// THE SECOND REVIEW'S FREEZE: a guest at the end of street-1 steps out of the shared world
    /// (the launcher's offline home, Dev travel or fly mode) and back. The relay took them out
    /// at the `game_leave` and spawns them AFRESH in the Commons (`rejoin` false), 148 m from
    /// where the game stands, while the game is not arriving (same server, same world session).
    /// They must stand where the relay now holds them, or every update is refused by the 100 m
    /// rule. Seen red 2026-10-03 on 65b3e2c0c (stand only on an arrival): "a fresh spawn after
    /// stepping out leaves the player 148 m from where the relay holds them: None".
    #[test]
    fn a_fresh_spawn_after_stepping_out_stands_where_the_relay_holds_us() {
        let ship = booted();
        let street_end = Vec3::new(70.0, 1.7, 195.0);
        let w = welcome_full(None, &ship.ship_hash(), Some(COMMONS.into()), false);
        match plan_welcome(Some(&ship), &w, &again(street_end)) {
            WelcomeHome::Guest { stand_at, .. } => assert!(
                stand_at.is_some_and(|at| (at - COMMONS).length() < 1e-4),
                "a fresh spawn after stepping out leaves the player {:.0} m from where the relay holds them: {stand_at:?}",
                street_end.distance(COMMONS)
            ),
            other => panic!("{other:?}"),
        }
        // The same on the player's own plot: the relay spawned them afresh at their door.
        let w = welcome_full(Some("p1"), &ship.ship_hash(), Some(P1_DOOR.into()), false);
        match plan_welcome(Some(&ship), &w, &again(Vec3::new(70.0, 1.7, 190.0))) {
            WelcomeHome::Stay { stand_at: Some(at), .. } => assert!((at - P1_DOOR).length() < 1e-4, "{at:?}"),
            other => panic!("a fresh spawn on our own plot stands at our door: {other:?}"),
        }
    }

    /// A reconnect inside the grace (`rejoin` true, not arriving) where the game nonetheless
    /// stands far from where the relay kept them (its own updates were refused, say): the
    /// backstop stands them back there. Seen red 2026-10-03 on 65b3e2c0c (no backstop): "a
    /// reconnect 120 m from where the relay holds us is left there: None".
    #[test]
    fn a_reconnect_far_from_where_the_relay_holds_us_brings_us_back() {
        let ship = booted();
        let w = welcome_full(Some("p1"), &ship.ship_hash(), Some(P1_DOOR.into()), true);
        let far = P1_DOOR + Vec3::new(0.0, 0.0, 120.0);
        match plan_welcome(Some(&ship), &w, &again(far)) {
            WelcomeHome::Stay { stand_at, .. } => assert!(
                stand_at.is_some_and(|at| (at - P1_DOOR).length() < 1e-4),
                "a reconnect 120 m from where the relay holds us is left there: {stand_at:?}"
            ),
            other => panic!("{other:?}"),
        }
    }

    /// The decision on its own, every case. Seen red 2026-10-03:
    ///  - with the 65b3e2c0c rule (`arriving` alone): "not arriving, spawned afresh, 148 m away:
    ///    expected true" (the second review's freeze);
    ///  - with "always stand where held" (the wrong fix): "a reconnect 3 m away keeps walking:
    ///    expected false".
    #[test]
    fn stand_where_held_decides_from_arrival_rejoin_and_distance() {
        let cases: [(bool, bool, Option<f32>, bool, &str); 9] = [
            (true, false, Some(0.0), true, "arriving, spawned afresh at the camera"),
            (true, true, Some(3.0), true, "arriving (a game restart inside the grace)"),
            (false, false, Some(148.0), true, "not arriving, spawned afresh, 148 m away"),
            (false, false, Some(2.0), true, "not arriving, spawned afresh, 2 m away"),
            (false, true, Some(3.0), false, "a reconnect 3 m away keeps walking"),
            (false, true, Some(89.9), false, "a reconnect inside the backstop keeps walking"),
            (false, true, Some(90.1), true, "a reconnect beyond the backstop comes back"),
            (false, true, None, false, "a reconnect the welcome says nothing about"),
            (false, false, None, true, "spawned afresh, no entry (stands at the door)"),
        ];
        for (arriving, rejoin, gap, want, what) in cases {
            assert_eq!(stand_where_held(arriving, rejoin, gap), want, "{what}: expected {want}");
        }
    }

    /// Which welcome is an arrival. The second review: hard-coding the arrival decision to false
    /// passed every test. Seen red 2026-10-03 with `is_arrival` returning false: "the first
    /// welcome since the world loaded is an arrival".
    #[test]
    fn the_first_welcome_and_another_servers_are_arrivals() {
        assert!(is_arrival(None, SERVER), "the first welcome since the world loaded is an arrival");
        assert!(is_arrival(Some("http://elsewhere:3210"), SERVER), "a welcome from another server is an arrival");
        assert!(!is_arrival(Some(SERVER), SERVER), "a later welcome from the same server is not");
    }

    /// Another ship (a different hash), no ship named at all, or no ship of our own: refuse,
    /// with the one sentence, never move the home, and never give a plot back (a plot that
    /// is not ours to give).
    #[test]
    fn another_ship_is_refused_in_one_plain_sentence() {
        let ship = booted();
        for w in [
            welcome(Some("p2"), "0123456789abcdef"),
            serde_json::json!({ "type": "game_welcome", "player_id": 7, "home_plot": null }),
        ] {
            match plan_welcome(Some(&ship), &w, &arriving(P1_DOOR)) {
                WelcomeHome::Refuse { sentence, give_up_plot } => {
                    assert_eq!(sentence, SHIP_MISMATCH);
                    assert!(!give_up_plot);
                }
                other => panic!("a different ship must be refused: {other:?}"),
            }
        }
        assert!(matches!(plan_welcome(None, &welcome(Some("p1"), &ship.ship_hash()), &arriving(P1_DOOR)), WelcomeHome::Refuse { .. }));
        // One sentence: one full stop, at the end.
        assert_eq!(SHIP_MISMATCH.matches(". ").count(), 0);
        assert!(SHIP_MISMATCH.ends_with('.'));
    }

    /// AN UPDATED APP WITH AN OLD SHIP FILE IN ITS DATA FOLDER IS TOLD WHICH FILE (the review of
    /// increment 4, P6). An installed game writes the ship file to its data folder on the first
    /// run and never refreshes it (src/storage.rs `extract_data_if_needed`); the updater swaps
    /// only the exe, and the ship is read from the data folder first. So after an update that
    /// changes the ship, the game names the OLD ship and every server running the new one
    /// refuses it with "update whichever of the app and the server is older", which cannot help:
    /// both are new. When the server's ship is the one built into this app, the sentence says
    /// the data folder's file is the old one and names it. Otherwise the usual sentence.
    ///
    /// Seen red 2026-10-04 on the code before the fix: "the sentence does not name the old ship
    /// file: Not joining the shared world: this server has a different ship from yours, and
    /// positions only agree when everyone has the same ship, so update whichever of the app and
    /// the server is older and reconnect."
    #[test]
    fn an_updated_app_with_an_old_ship_file_is_told_which_file() {
        let file = std::path::Path::new("C:/Users/someone/AppData/Roaming/HumanityOS/data/blueprints/ship_structure.ron");
        let said = other_ship_sentence(Some("1234abcd"), Some("1234abcd"), Some(file));
        assert!(said.contains(&file.display().to_string()), "the sentence does not name the old ship file: {said}");
        assert_ne!(said, SHIP_MISMATCH);
        assert!(said.ends_with('.') && said.matches(". ").count() == 0, "one sentence: {said}");
        assert_eq!(other_ship_sentence(Some("1234abcd"), Some("99990000"), Some(file)), SHIP_MISMATCH, "the server's ship is not the app's either");
        assert_eq!(other_ship_sentence(Some("1234abcd"), Some("1234abcd"), None), SHIP_MISMATCH, "our ship is the app's own copy");
        assert_eq!(other_ship_sentence(None, Some("1234abcd"), Some(file)), SHIP_MISMATCH, "the server named no ship");
    }

    /// A plot our home does not fit (here p2 made narrower than the home, on a ship whose hash
    /// the relay shares): refuse, and GIVE THE PLOT BACK as we leave, or it is held for good by
    /// someone who never lives there (the second review). Seen red 2026-10-03 with
    /// `give_up_plot` false on that path: "a home that does not fit its plot gives it back:
    /// false".
    #[test]
    fn a_home_that_does_not_fit_its_plot_gives_it_back() {
        let mut file = ShipStructure::load_ship_file(&data_dir()).unwrap();
        let p2 = file.plots.iter().position(|p| p.id == "p2").unwrap();
        file.plots[p2].size.0 = 30.0;
        let design = booted().home_design().unwrap();
        let ship = file.clone().assemble(design, "p1").expect("the home stands on p1");
        let w = welcome(Some("p2"), &ship.ship_hash());
        match plan_welcome(Some(&ship), &w, &arriving(P1_DOOR)) {
            WelcomeHome::Refuse { sentence, give_up_plot } => {
                assert!(give_up_plot, "a home that does not fit its plot gives it back: {give_up_plot}");
                assert!(sentence.contains("does not fit"), "{sentence}");
            }
            other => panic!("{other:?}"),
        }
    }

    /// What the game puts in its `game_join`: the ship it draws and its own home's door,
    /// plot-local. No ship: no join at all (`add_join_fields` refuses to build one; round 4). A home with no authored door: no door (each side takes the plot's middle). Seen red
    /// 2026-10-03 with `add_join_fields` adding nothing (the first 1b join): "the join names no
    /// ship: null"; and with `home_arrival_local` sending the middle of the plot the home was
    /// built on (65b3e2c0c, the second review's finding 6): "a home with no door names none:
    /// [27.5,44.5]".
    #[test]
    fn the_join_names_our_ship_and_our_door() {
        let mut ship = booted();
        let home = ship.home_zone_index();
        ship.zones[home].body.spawn = Some((12.5, 30.0));
        let mut join = serde_json::json!({ "type": "game_join" });
        assert!(add_join_fields(&mut join, Some(&ship)), "a join with our ship is built");
        assert_eq!(join["ship_hash"], ship.ship_hash().as_str(), "the join names no ship: {}", join["ship_hash"]);
        assert_eq!(join["home_spawn"], serde_json::json!([12.5, 30.0]));
        // No ship: no join (round 4 of the 1b review, finding 1). Seen red 2026-10-03 on
        // db551f530, which sent "" (the relay refused it as another ship): "a join with no ship
        // is built anyway, naming the ship Some(String(\"\"))".
        let mut bare = serde_json::json!({ "type": "game_join" });
        let built = add_join_fields(&mut bare, None);
        assert!(!built && bare.get("ship_hash").is_none(), "a join with no ship is built anyway, naming the ship {:?}", bare.get("ship_hash"));
        assert!(bare.get("home_spawn").is_none());
        ship.zones[home].body.spawn = None;
        let mut doorless = serde_json::json!({ "type": "game_join" });
        assert!(add_join_fields(&mut doorless, Some(&ship)));
        assert!(doorless.get("home_spawn").is_none(), "a home with no door names none: {}", doorless["home_spawn"]);
    }

    /// What the world load anchored to a machine goes with that machine. Placing the shipped
    /// machines with the home on p1 and again on p2: an anchor at a home machine (the grain
    /// field the chickens graze around) moves by exactly p2's 99 m, a sphere's lifted
    /// placement still matches its unlifted anchor, and an anchor at a ship machine (a Commons
    /// row) or at no machine at all does not move. Seen red 2026-10-03 with `shift_of`
    /// returning zero (the first 1b client, which carried nothing): "the chickens' grain
    /// field moved by Vec3(0.0, 0.0, 0.0)".
    #[test]
    fn what_was_anchored_to_a_home_machine_goes_with_it() {
        let home = crate::machines::MachineHome::load(&crate::machines::home_ron_path(&data_dir())).expect("the machines load");
        let place = |plot: &str| {
            let ship = ShipStructure::load_and_assemble_shipped(&data_dir(), Some(plot)).unwrap();
            home.placements(&std::collections::HashMap::new(), Some(&ship.zone_rects()))
        };
        let (on_p1, on_p2) = (place("p1"), place("p2"));
        let shifts = machine_shifts(&on_p1, &on_p2);
        let g = on_p1.iter().find(|m| m.id == "grain_field_1").expect("the grain field is placed");
        let d = shift_of(&shifts, Vec3::new(g.pos.0, g.pos.1, g.pos.2));
        assert!((d - Vec3::new(0.0, 0.0, 99.0)).length() < 1e-4, "the chickens' grain field moved by {d:?}");
        // A sphere's placement is lifted by its radius; the world load anchors at the floor.
        if let Some(s) = on_p1.iter().find(|m| m.shape == "sphere") {
            let floor = Vec3::new(s.pos.0, s.pos.1 - s.size.0, s.pos.2);
            assert!((shift_of(&shifts, floor) - Vec3::new(0.0, 0.0, 99.0)).length() < 1e-4, "{} (a sphere)", s.id);
        }
        // A machine outside the home (a Commons row, the ship's) stays, and so does what
        // grazes around it; so does an anchor at no machine at all.
        let (lo, hi) = booted().home_plot().unwrap().aabb();
        let c = on_p1
            .iter()
            .find(|m| {
                let p = Vec3::new(m.pos.0, m.pos.1, m.pos.2);
                !(p.cmpge(lo).all() && p.cmple(hi).all())
            })
            .expect("the ship has machines outside the home");
        assert_eq!(shift_of(&shifts, Vec3::new(c.pos.0, 0.0, c.pos.2)), Vec3::ZERO, "{} is not the home's", c.id);
        assert_eq!(shift_of(&shifts, Vec3::new(-500.0, 0.0, -500.0)), Vec3::ZERO);
    }

    /// A world with what the player may have built: on p1 (the home), a finished chest with
    /// its uid, a wall still going up, a parked vehicle driving to a point in the home; in the
    /// Commons, a Dev-built piece and a vehicle; and a piece built on a planet whose site-local
    /// pose happens to read as a point inside p1.
    fn built_world() -> hecs::World {
        use crate::systems::construction::{Construction, PlanetSite, Structure};
        let tf = |x: f32, z: f32| Transform { position: Vec3::new(x, 0.0, z), rotation: glam::Quat::IDENTITY, scale: Vec3::ONE };
        let built = |id: &str, uid: u32| Structure { blueprint_id: id.into(), health: 42.0, max_health: 100.0, provides: None, uid };
        let vehicle = || crate::ecs::components::Vehicle { item_id: "truck_pickup_0".into() };
        let mut w = hecs::World::new();
        w.spawn((tf(20.0, 30.0), built("chest", 7)));
        w.spawn((tf(10.0, 12.0), Construction { blueprint_id: "wood_wall".into(), progress: 3.0, build_time: 10.0, builder_key: None }));
        w.spawn((
            tf(40.0, 70.0),
            vehicle(),
            crate::ecs::components::VehicleRoute { dest: Vec3::new(45.0, 0.0, 80.0), speed_mps: 3.0, arrive_radius: 2.0 },
        ));
        w.spawn((tf(80.0, 40.0), built("wood_wall", 8))); // the Commons, x 65..99
        w.spawn((tf(90.0, 50.0), vehicle()));
        w.spawn((tf(25.0, 35.0), built("wood_wall", 9), PlanetSite { body: "earth".into(), origin: glam::DVec3::new(6.371e6, 0.0, 0.0) }));
        w
    }

    /// FINDING 3 of the second review: the pieces built aboard and the parked vehicles in the
    /// home go with it (99 m to p2); the ship's spaces and a planet site keep theirs; a chest
    /// keeps its uid (its contents are filed under it) and health; a scaffold its progress; a
    /// vehicle's destination in the home moves with it. Seen red 2026-10-03 with
    /// `carry_built_pieces` moving nothing (65b3e2c0c carried neither): "two pieces and one
    /// vehicle were in the home: left: (0, 0), right: (2, 1)".
    #[test]
    fn what_the_player_built_and_parked_in_the_home_goes_with_it() {
        use crate::systems::construction::{Construction, PlanetSite, Structure};
        let p1 = booted().home_plot().unwrap().aabb();
        let mut w = built_world();
        let delta = Vec3::new(0.0, 0.0, 99.0);
        assert_eq!(carry_built_pieces(&mut w, p1, delta), (2, 1), "two pieces and one vehicle were in the home");
        let at = |w: &hecs::World, id: &str| -> Vec3 {
            w.query::<(&Transform, &Structure)>().iter().find(|(_, (_, s))| s.blueprint_id == id).map(|(_, (t, _))| t.position).unwrap()
        };
        let chest = at(&w, "chest");
        assert_eq!(chest, Vec3::new(20.0, 0.0, 129.0), "the chest stayed at {chest:?}, in p1");
        let (uid, health) = w.query::<&Structure>().iter().find(|(_, s)| s.blueprint_id == "chest").map(|(_, s)| (s.uid, s.health)).unwrap();
        assert_eq!((uid, health), (7, 42.0), "moved, not built again");
        let scaffold = w.query::<(&Transform, &Construction)>().iter().map(|(_, (t, c))| (t.position, c.progress)).next().unwrap();
        assert_eq!(scaffold, (Vec3::new(10.0, 0.0, 111.0), 3.0));
        // The ship's own spaces and the planet site keep their pieces.
        let commons_wall = w.query::<(&Transform, &Structure)>().iter().find(|(_, (_, s))| s.uid == 8).map(|(_, (t, _))| t.position).unwrap();
        assert_eq!(commons_wall, Vec3::new(80.0, 0.0, 40.0));
        let planet = w.query::<(&Transform, &PlanetSite)>().iter().map(|(_, (t, _))| t.position).next().unwrap();
        assert_eq!(planet, Vec3::new(25.0, 0.0, 35.0), "a planet piece never moves with the home");
        // Vehicles: the home's goes, with its destination; the Commons one stays.
        let vehicles: Vec<(Vec3, Option<Vec3>)> = w
            .query::<(&Transform, &crate::ecs::components::Vehicle, Option<&crate::ecs::components::VehicleRoute>)>()
            .iter()
            .map(|(_, (t, _, r))| (t.position, r.map(|r| r.dest)))
            .collect();
        assert!(vehicles.contains(&(Vec3::new(40.0, 0.0, 169.0), Some(Vec3::new(45.0, 0.0, 179.0)))), "{vehicles:?}");
        assert!(vehicles.contains(&(Vec3::new(90.0, 0.0, 50.0), None)), "{vehicles:?}");
        // The rig's report lists the aboard pieces (not the planet's) and the vehicles.
        let (pieces, cars) = built_positions(&w);
        assert_eq!((pieces.len(), cars.len()), (3, 2));
        assert!(pieces.contains(&chest));
        // Moving by nothing moves nothing.
        assert_eq!(carry_built_pieces(&mut built_world(), p1, Vec3::ZERO), (0, 0));
    }


    /// THE SAVE'S FRAME, the third review's version (findings 2 and 3): the save records the
    /// box of the plot its home stood on, and its pieces and vehicles where they stood; a load
    /// carries the ones that were in the home to wherever the home stands then. The second
    /// review's frame ("as if the home stood on the default plot") held only while every boot
    /// built the home on the default plot, which increment 2's remembered plot ends, and it
    /// could not tell a vehicle in the home from one left on the default plot.
    ///
    /// The live world here has the home on p2: a chest in it, a scaffold, a truck parked in it,
    /// a truck left standing on p1 (driven out through a wall: driving has no collision), a
    /// piece in the Commons and one on a planet.
    ///   (a) loaded into a world whose home is built on p2 directly (increment 2): nothing moves;
    ///   (b) loaded at a boot that builds the home on the default plot, p1, and then a welcome
    ///       moves it back to p2: the chest goes to p1 and back to p2, the truck left on p1 is
    ///       marked not the home's and stays on p1 through the move.
    ///
    /// Seen red 2026-10-03 with `carry_saved_pieces` carrying and marking nothing (the
    /// a504c5cd9 load, which applied the save where it stood and carried nothing at a boot):
    /// "loaded at a boot on p1, the chest stands at Vec3(20.0, 0.0, 129.0), outside the home".
    #[test]
    fn the_save_records_where_its_home_stood_and_loads_into_the_home_on_any_plot() {
        use crate::systems::construction::Structure;
        let on_p2 = ShipStructure::load_and_assemble_shipped(&data_dir(), Some("p2")).unwrap();
        let (p1, p2) = (home_box(&booted()).unwrap(), home_box(&on_p2).unwrap());
        let tf = |x: f32, z: f32| Transform { position: Vec3::new(x, 0.0, z), rotation: glam::Quat::IDENTITY, scale: Vec3::ONE };
        let truck = || crate::ecs::components::Vehicle { item_id: "truck_pickup_0".into() };
        let mut live = built_world(); // built with the home on p1 ...
        carry_built_pieces(&mut live, p1, p2.0 - p1.0); // ... then a welcome moved it to p2
        live.spawn((tf(14.0, 85.0), truck())); // left standing on p1, someone else's plot now
        let mut save = crate::save_load::extract_world_save(&live);
        crate::save_load::record_home_frame(&mut save, home_frame(&on_p2).as_ref());
        assert_eq!(save.home_plot_box, Some([p2.0.to_array(), p2.1.to_array()]), "the save says where its home stood");
        let chest_at = |w: &hecs::World| {
            w.query::<(&Transform, &Structure)>().iter().find(|(_, (_, s))| s.blueprint_id == "chest").map(|(_, (t, _))| t.position).unwrap()
        };
        let trucks = |w: &hecs::World| -> Vec<Vec3> {
            w.query::<(&Transform, &crate::ecs::components::Vehicle)>().iter().map(|(_, (t, _))| t.position).collect()
        };
        assert_eq!(chest_at(&live), Vec3::new(20.0, 0.0, 129.0), "the chest is saved where it stands");

        // (a) Straight into a home built on p2.
        let mut onto_p2 = hecs::World::new();
        crate::save_load::apply_save_to_world(&mut onto_p2, &save);
        assert_eq!(carry_saved_pieces(&mut onto_p2, save.home_plot_box, Some(p2)), (0, 0, 0), "nothing to carry or mark");
        assert_eq!(chest_at(&onto_p2), Vec3::new(20.0, 0.0, 129.0));
        assert!(trucks(&onto_p2).contains(&Vec3::new(14.0, 0.0, 85.0)), "the truck left on p1 stays on p1: {:?}", trucks(&onto_p2));

        // (b) A boot that builds the home on p1 first.
        let mut boot = hecs::World::new();
        crate::save_load::apply_save_to_world(&mut boot, &save);
        let (pieces, vehicles, marked) = carry_saved_pieces(&mut boot, save.home_plot_box, Some(p1));
        let chest = chest_at(&boot);
        assert!(over_plot(chest, p1), "loaded at a boot on p1, the chest stands at {chest:?}, outside the home");
        assert_eq!((pieces, vehicles, marked), (2, 1, 1), "the chest and the scaffold and the parked truck go to p1; the truck left there is marked");
        // The welcome moves the home to p2 (what `follow_home_box` does on the move).
        carry_built_pieces(&mut boot, p1, p2.0 - p1.0);
        clear_not_the_homes(&mut boot);
        assert_eq!(chest_at(&boot), Vec3::new(20.0, 0.0, 129.0), "the chest is back in the home on p2");
        let t = trucks(&boot);
        assert!(t.contains(&Vec3::new(14.0, 0.0, 85.0)), "a truck saved standing on p1 was adopted into the home: {t:?}");
        assert!(t.contains(&Vec3::new(40.0, 0.0, 169.0)), "the truck parked in the home is back in it: {t:?}");
        assert_eq!(boot.query::<&NotTheHomes>().iter().count(), 0, "the marks are cleared by the move");
        // The ship's own spaces and the planet keep theirs throughout.
        let (pieces, _) = built_positions(&boot);
        assert!(pieces.contains(&Vec3::new(80.0, 0.0, 40.0)), "the Commons piece did not move: {pieces:?}");
        // No box in the save (the legacy layout): nothing is carried.
        let mut legacy = hecs::World::new();
        crate::save_load::apply_save_to_world(&mut legacy, &save);
        assert_eq!(carry_saved_pieces(&mut legacy, None, Some(p1)), (0, 0, 0));
    }

    /// FINDING 2 of the third review: a Dev makes p2 the plot offline play uses (the Plots
    /// panel's "Offline play uses", `default_plot`) while the home stands on p1. The home
    /// does not move, so the frame does not change; the next launch builds the home on p2, and
    /// the save's pieces must be carried there. The second review's frame wrote them as if the
    /// home stood on the default plot, from a copy published at the world load (default p1),
    /// so the chest came back 99 m outside the home.
    ///
    /// Seen red 2026-10-03 with `carry_saved_pieces` carrying nothing: "after the default plot
    /// became p2, the chest stands at Some(Vec3(20.0, 0.0, 30.0)), outside the home on p2".
    #[test]
    fn the_save_frame_follows_the_live_ship_after_a_dev_plot_edit() {
        let mut ship = booted();
        let frame = home_frame(&ship).expect("a frame");
        let mut w = hecs::World::new();
        w.spawn((
            Transform { position: Vec3::new(20.0, 0.0, 30.0), rotation: glam::Quat::IDENTITY, scale: Vec3::ONE },
            crate::systems::construction::Structure { blueprint_id: "chest".into(), health: 1.0, max_health: 1.0, provides: None, uid: 3 },
        ));
        ship.default_plot = Some("p2".into());
        assert_eq!(home_box_change(Some(frame.home), home_box(&ship)), HomeBoxChange::Same, "the home did not move");
        let mut save = crate::save_load::extract_world_save(&w);
        crate::save_load::record_home_frame(&mut save, Some(&frame));
        // The next launch: the home on the new default plot.
        let next = ship.ship_file().assemble(ship.home_design().unwrap(), &ship.default_plot_id().unwrap()).expect("p2 takes the home");
        let p2 = home_box(&next).unwrap();
        let mut fresh = hecs::World::new();
        crate::save_load::apply_save_to_world(&mut fresh, &save);
        carry_saved_pieces(&mut fresh, save.home_plot_box, Some(p2));
        let (pieces, _) = built_positions(&fresh);
        assert!(
            pieces.iter().any(|p| over_plot(*p, p2)),
            "after the default plot became p2, the chest stands at {:?}, outside the home on p2",
            pieces.first()
        );
    }

    /// FINDING 2, the other half: a Dev moves the plot the home stands on (the Plots panel's
    /// `move_plot`). The rebuild follows the home's box (`home_box_change`, run by
    /// `follow_home_box` after every rebuild): it carries what the home holds by the move; a
    /// resize only republishes the frame; nothing changed is nothing to do.
    ///
    /// Seen red 2026-10-03 with `home_box_change` always `Same` (the a504c5cd9 rebuild, which
    /// never looked at the frame): "a Dev move of the home's plot carries nothing: Same".
    #[test]
    fn a_dev_move_of_the_homes_plot_carries_what_it_holds() {
        let mut ship = booted();
        let before = home_box(&ship).unwrap();
        assert!(ship.move_plot("p1", (-60.0, 0.0, 0.0)), "the plot moves");
        let after = home_box(&ship).unwrap();
        match home_box_change(Some(before), Some(after)) {
            HomeBoxChange::Carry { from, delta } => {
                assert_eq!((from, delta), (before, Vec3::new(-60.0, 0.0, 0.0)));
                let mut w = built_world();
                assert_eq!(carry_built_pieces(&mut w, from, delta), (2, 1), "the chest, the scaffold and the truck go with the home");
            }
            other => panic!("a Dev move of the home's plot carries nothing: {other:?}"),
        }
        let grown = (before.0, before.1 + Vec3::new(5.0, 0.0, 0.0));
        assert_eq!(home_box_change(Some(before), Some(grown)), HomeBoxChange::Republish, "a resize keeps everything where it is");
        assert_eq!(home_box_change(Some(before), Some(before)), HomeBoxChange::Same);
        assert_eq!(home_box_change(None, Some(before)), HomeBoxChange::Republish, "a first frame carries nothing");
    }

    /// FINDING 1 of the third review: the editor is open when a welcome moves the home from p1
    /// to p2. The undo history starts again from the moved home (`history_after_move`), so an
    /// edit and Ctrl+Z restore the home on p2, never the p1 baseline taken when the editor
    /// opened (that put the home back on someone else's plot, 99 m from everything the move had
    /// carried). The restored home's box is p2's, so the rebuild's follow carries nothing back.
    ///
    /// Seen red 2026-10-03 with `history_after_move` doing nothing (the a504c5cd9 move): "an
    /// edit and an undo after the move put the home back on Some(\"p1\")".
    #[test]
    fn a_move_while_the_editor_is_open_leaves_no_undo_back_to_the_old_plot() {
        use crate::engine::editor::{history_checkpoint, history_start_from, history_undo};
        let snap = |s: &ShipStructure| EditorSnapshot { structure: Some(s.clone()), machines: None };
        let on_p1 = booted();
        let on_p2 = ShipStructure::load_and_assemble_shipped(&data_dir(), Some("p2")).unwrap();
        let mut h = ConstructionHistory::default();
        history_start_from(&mut h, snap(&on_p1)); // the editor opens, the home on p1
        history_after_move(&mut h, snap(&on_p2)); // the welcome moves it to p2
        let mut edited = on_p2.clone(); // one edit of the moved home
        let home = edited.home_zone_index();
        edited.zones[home].body.spawn = Some((10.0, 10.0));
        history_checkpoint(&mut h, snap(&edited), 64);
        let restored = history_undo(&mut h, snap(&edited)).and_then(|s| s.structure).expect("one step to undo");
        assert_eq!(
            restored.home.as_ref().map(|a| a.plot.as_str()),
            Some("p2"),
            "an edit and an undo after the move put the home back on {:?}",
            restored.home.as_ref().map(|a| a.plot.as_str())
        );
        let p2 = home_box(&on_p2).unwrap();
        assert_eq!(home_box_change(Some(p2), home_box(&restored)), HomeBoxChange::Same, "and nothing it holds is carried back");
        assert!(h.undo.is_empty(), "nothing further back than the move");
    }

    /// THE 1b LEFTOVER (increment 2): a guest draws no home of its own. Whichever plot the home
    /// stood on, the default one (p1, someone else's: the game builds there at boot) or one a
    /// released player's earlier welcome moved it to (finding 8 of 1b's third review), a guest
    /// welcome puts it AWAY: off every plot, at `HOME_AWAY_ORIGIN`, so both plots are drawn as
    /// neighbours'. Respawn is the Commons. A home already away moves nothing.
    ///
    /// Seen red 2026-10-04 with the guest arm putting nothing away (`away: None`, as 1b's left a
    /// home on the default plot where it stood): "a guest whose home stood on p1 still draws it on a
    /// plot: None" (and `a_full_ship_makes_a_guest_in_the_commons` failed with `away: None`).
    #[test]
    fn a_guest_puts_its_home_away_and_draws_none_of_its_own() {
        for own in ["p1", "p2"] {
            let ship = ShipStructure::load_and_assemble_shipped(&data_dir(), Some(own)).unwrap();
            let w = welcome_full(None, &ship.ship_hash(), Some(COMMONS.into()), false);
            match plan_welcome(Some(&ship), &w, &again(P1_DOOR)) {
                WelcomeHome::Guest { door, stand_at, away, join_afresh: false } => {
                    let plot = away.as_ref().map(|s| s.home_plot().map(|p| p.id.clone()));
                    assert_eq!(plot, Some(None), "a guest whose home stood on {own} still draws it on a plot: {:?}", plot.clone().flatten());
                    let away = away.unwrap();
                    assert!(away.home_is_away());
                    let home = &away.zones[away.home_zone_index()];
                    assert_eq!(home.origin, crate::ship::ship_structure::HOME_AWAY_ORIGIN, "kept where homes are put away");
                    assert_eq!(away.neighbour_plots().count(), 2, "both plots are drawn as neighbours'");
                    assert_eq!(away.ship_hash(), ship.ship_hash(), "putting the home away is not a new ship");
                    assert!(door.is_some_and(|d| (d - COMMONS).length() < 1e-4), "Respawn is the Commons: {door:?}");
                    assert!(stand_at.is_some_and(|a| (a - COMMONS).length() < 1e-4), "{stand_at:?}");
                }
                other => panic!("{other:?}"),
            }
        }
        let away = booted().put_home_away().unwrap();
        let w = welcome_full(None, &away.ship_hash(), Some(COMMONS.into()), true);
        match plan_welcome(Some(&away), &w, &again(COMMONS)) {
            WelcomeHome::Guest { away: None, .. } => {}
            other => panic!("a home already away moves nothing: {other:?}"),
        }
    }

    /// A guest whose home is put away, given a plot by a later welcome (one freed up), gets its
    /// home back ON that plot: a Move from where it was kept, whose box is the home's own at
    /// `HOME_AWAY_ORIGIN`, so everything it holds is carried by the same delta (`home_box`,
    /// `home_box_change`). The box where a home is kept shares no footprint with any plot or zone
    /// of the ship, since the carry tests x and z only. Seen red 2026-10-04 with `home_box` reading
    /// only the plot (None while away): "a home put away has a box: where it is kept".
    #[test]
    fn a_home_put_away_comes_back_onto_a_plot_with_everything_it_holds() {
        let ship = booted();
        let away = ship.put_home_away().unwrap();
        let p1 = home_box(&ship).unwrap();
        let kept = home_box(&away).expect("a home put away has a box: where it is kept");
        let delta = Vec3::from(crate::ship::ship_structure::HOME_AWAY_ORIGIN);
        assert_eq!(home_box_change(Some(p1), Some(kept)), HomeBoxChange::Carry { from: p1, delta }, "putting the home away carries nothing: Republish");
        for p in &ship.plots {
            let (lo, hi) = p.aabb();
            assert!(kept.1.x < lo.x || kept.0.x > hi.x || kept.1.z < lo.z || kept.0.z > hi.z, "where a home is kept overlaps {} across the floor", p.id);
        }
        for z in ship.zones.iter().filter(|z| z.id != "home") {
            let o = z.origin_vec();
            assert!(kept.1.x < o.x || kept.0.x > o.x + z.body.width, "where a home is kept overlaps {} across the floor", z.id);
        }
        // A plot freed up: the next welcome moves the home from where it was kept onto p2.
        let w = welcome_at(Some("p2"), &away.ship_hash(), Some(P2_DOOR.into()));
        match plan_welcome(Some(&away), &w, &again(COMMONS)) {
            WelcomeHome::Move { ship: back, plot, door, stand_at } => {
                assert_eq!((plot.as_str(), back.home_is_away()), ("p2", false));
                assert!((door - P2_DOOR).length() < 1e-4 && (stand_at - P2_DOOR).length() < 1e-4, "{door:?} {stand_at:?}");
                assert_eq!(home_box_change(Some(kept), home_box(&back)), HomeBoxChange::Carry { from: kept, delta: home_box(&back).unwrap().0 - kept.0 });
            }
            other => panic!("a guest given a plot gets its home back on it: {other:?}"),
        }
        // A guest names its own door in the join all the same: the relay may give it a plot.
        assert_eq!(away.home_arrival_local(), Some((53.5, 40.5)));
    }

    /// A guest who leaves the shared world (steps out to solo play, is refused, switches server,
    /// loses the connection) gets its home back onto the ship: onto the plot it remembers for the
    /// server it now talks to, else the default plot. A home that is not away stays where it is.
    /// Seen red 2026-10-04 with `home_back_from_away` always None (a guest's home stayed away for
    /// the rest of the session): "out of the shared world, the guest's home comes back: None".
    #[test]
    fn a_guest_out_of_the_shared_world_gets_its_home_back() {
        let away = booted().put_home_away().unwrap();
        let back = home_back_from_away(&away, None).map(|r| r.map(|s| s.home_plot().map(|p| p.id.clone())));
        assert_eq!(back, Some(Ok(Some("p1".to_string()))), "out of the shared world, the guest's home comes back: {back:?}");
        let mem = crate::config::RememberedPlot { ship_hash: away.ship_hash(), plot: "p2".into() };
        let onto = home_back_from_away(&away, Some(&mem)).unwrap().unwrap();
        assert_eq!(onto.home_plot().map(|p| p.id.as_str()), Some("p2"), "onto the plot remembered for the server it now talks to");
        assert!(!onto.home_is_away());
        assert!(home_back_from_away(&booted(), None).is_none(), "a home on a plot stays where it is");
    }

    /// THE REMEMBERED PLOT, at boot: the world load builds the home on the plot this player
    /// remembers for the server, when it is a plot of the ship file and was remembered on that
    /// same ship (its hash); otherwise the default plot. Seen red 2026-10-04 with `boot_plot`
    /// always None (the 1b world load, which always built on the default plot): "the plot
    /// remembered for this server: None".
    #[test]
    fn the_world_load_builds_on_the_plot_remembered_for_the_server() {
        let file = crate::ship::ship_structure::ShipStructure::load_ship_file(&data_dir()).unwrap();
        let hash = file.ship_hash();
        let r = |plot: &str, ship_hash: &str| crate::config::RememberedPlot { ship_hash: ship_hash.into(), plot: plot.into() };
        assert_eq!(boot_plot(&file, Some(&r("p2", &hash))).as_deref(), Some("p2"), "the plot remembered for this server: {:?}", boot_plot(&file, Some(&r("p2", &hash))));
        assert_eq!(boot_plot(&file, None), None, "nothing remembered: the default plot");
        assert_eq!(boot_plot(&file, Some(&r("p2", "0123456789abcdef"))), None, "remembered on another ship: not trusted");
        assert_eq!(boot_plot(&file, Some(&r("p9", &hash))), None, "a plot this ship does not have: the default plot");
        // And the assembled ship puts the home there.
        let ship = crate::ship::ship_structure::ShipStructure::assemble_from(file.clone(), &data_dir(), boot_plot(&file, Some(&r("p2", &hash))).as_deref()).unwrap();
        assert_eq!(ship.home_plot().map(|p| p.id.as_str()), Some("p2"));
        // The returning player's welcome then only confirms it: a Stay, nothing moves.
        let w = welcome_at(Some("p2"), &hash, Some(P2_DOOR.into()));
        assert!(matches!(plan_welcome(Some(&ship), &w, &arriving(P2_DOOR)), WelcomeHome::Stay { .. }), "a returning player's welcome is a Stay");
    }

    /// A plot is remembered for THIS IDENTITY on a server (the key `gui::erased_entry` makes, the
    /// one the erased-server memory uses), so a second identity on the same app, or one restored
    /// from another recovery phrase, does not build its home on the first one's plot at boot.
    /// Increment 2 review, finding 3. Seen red 2026-10-04 on c98c5465b (keyed by the server
    /// alone): "the plot remembered by this identity on this server: Some(\"p1\")".
    #[test]
    fn a_plot_is_remembered_for_one_identity_on_one_server() {
        let hash = ShipStructure::load_ship_file(&data_dir()).unwrap().ship_hash();
        let mut gui = crate::gui::GuiState::default();
        gui.server_url = SERVER.to_string();
        gui.connected_server_url = SERVER.to_string();
        gui.profile_public_key = "aa11".to_string();
        let server = active_server_key(&gui);
        gui.home_plots.insert(crate::gui::erased_entry("aa11", &server), crate::config::RememberedPlot { ship_hash: hash, plot: "p2".into() });
        let built = |gui: &crate::gui::GuiState| assemble_for_boot(&data_dir(), gui).unwrap().home_plot().map(|p| p.id.clone());
        assert_eq!(built(&gui).as_deref(), Some("p2"), "the plot remembered by this identity on this server: {:?}", built(&gui));
        gui.profile_public_key = "bb22".to_string();
        assert_eq!(built(&gui).as_deref(), Some("p1"), "another identity on the same app builds on the default plot");
    }

    /// What each welcome teaches the memory: a Move or a Stay remembers the plot, a guest and a
    /// plot that went back forget it, and a refusal over the ship leaves it (the hash at the next
    /// boot distrusts it). Seen red 2026-10-04 with `plot_memory_after` always None (the 1b game,
    /// which remembered nothing): "a Move to p2 remembers p2: None".
    #[test]
    fn each_welcome_teaches_the_memory_its_plot() {
        let ship = booted();
        let hash = ship.ship_hash();
        let mv = plan_welcome(Some(&ship), &welcome_at(Some("p2"), &hash, Some(P2_DOOR.into())), &arriving(P1_DOOR));
        assert_eq!(plot_memory_after(&mv, Some(&ship)), Some(Some("p2".to_string())), "a Move to p2 remembers p2: {:?}", plot_memory_after(&mv, Some(&ship)));
        let stay = plan_welcome(Some(&ship), &welcome_at(Some("p1"), &hash, Some(P1_DOOR.into())), &arriving(P1_DOOR));
        assert_eq!(plot_memory_after(&stay, Some(&ship)), Some(Some("p1".to_string())), "a Stay remembers the plot the home stands on");
        let guest = plan_welcome(Some(&ship), &welcome_at(None, &hash, Some(COMMONS.into())), &arriving(P1_DOOR));
        assert_eq!(plot_memory_after(&guest, Some(&ship)), Some(None), "a guest forgets its plot here");
        let gone = WelcomeHome::Refuse { sentence: String::new(), give_up_plot: true };
        assert_eq!(plot_memory_after(&gone, Some(&ship)), Some(None), "a plot that went back is forgotten");
        let other = plan_welcome(Some(&ship), &welcome(Some("p2"), "0123456789abcdef"), &arriving(P1_DOOR));
        assert_eq!(plot_memory_after(&other, Some(&ship)), None, "another ship's refusal leaves the memory");
    }

    /// An old config.json, written before increment 2, has no `home_plots`: it loads, with none
    /// remembered (so the first world entry builds on the default plot, as 1b did), and a config
    /// with one round-trips it. Seen red 2026-10-04 with the field's `#[serde(default)]` removed:
    /// "a config from before increment 2 does not load: missing field `home_plots` at line 1 column
    /// 39" (and every AppConfig::default() panicked: "every field must carry a serde default").
    #[test]
    fn an_old_config_loads_with_no_plot_remembered() {
        let old: crate::config::AppConfig = serde_json::from_str("{\"server_url\": \"http://127.0.0.1:3210\"}")
            .unwrap_or_else(|e| panic!("a config from before increment 2 does not load: {e}"));
        assert!(old.home_plots.is_empty());
        let mut cfg = old.clone();
        cfg.home_plots.insert("http://127.0.0.1:3210".into(), crate::config::RememberedPlot { ship_hash: "abc".into(), plot: "p2".into() });
        let back: crate::config::AppConfig = serde_json::from_str(&serde_json::to_string(&cfg).unwrap()).unwrap();
        assert_eq!(back.home_plots, cfg.home_plots);
    }

    /// FINDINGS 12 and 13 of the third review: a welcome is applied only while the game is in
    /// the shared world and aboard. One that lands after the game stepped out (solo, within a
    /// round trip of its join) yanked the player to their door, and one that reached a player
    /// on a planet (joined from there after an outage) moved them by ship coordinates in the
    /// planet's frame.
    ///
    /// Seen red 2026-10-03 with `accept_welcome` always true (the a504c5cd9 welcome, applied
    /// whatever the game was doing): "a welcome after stepping out (solo) is applied".
    #[test]
    fn a_welcome_after_stepping_out_or_away_from_the_ship_is_not_applied() {
        assert!(accept_welcome(true, false, true), "joined, in the world, aboard");
        assert!(!accept_welcome(true, true, true), "a welcome after stepping out (solo) is applied");
        assert!(!accept_welcome(false, false, true), "a welcome after leaving (not joined) is applied");
        assert!(!accept_welcome(true, false, false), "a welcome on a planet or a Dev trip is applied");
    }

    /// FINDINGS 11 and 14 of the third review. A click on a saved server swapped in its live
    /// background connection in one frame: the game never left the first server (its relay
    /// kept the figure frozen for good) and never joined the second. Now a switch while joined
    /// leaves the first server, and a switch, or a connection made afresh, tries a server that
    /// refused us again (a refusal used to hold until the app restarted).
    ///
    /// Seen red 2026-10-03 with `server_follow` doing nothing (the a504c5cd9 game, which had no
    /// such step): "a switch while joined leaves the server we joined on: ServerFollow {
    /// leave_on: None, clear_refusal: false }".
    #[test]
    fn switching_servers_leaves_the_one_we_joined_and_a_fresh_connection_tries_again() {
        let (a, b) = ("https://a.example", "https://b.example");
        let f = server_follow(a, b, true, true, false);
        assert_eq!(f, ServerFollow { leave_on: Some(a.into()), clear_refusal: true }, "a switch while joined leaves the server we joined on: {f:?}");
        assert_eq!(server_follow(a, b, false, true, false), ServerFollow { leave_on: None, clear_refusal: true }, "a switch tries the new server again");
        assert_eq!(server_follow(a, a, true, true, false), ServerFollow { leave_on: None, clear_refusal: false }, "the same server, connected: nothing");
        assert_eq!(server_follow(a, a, false, false, false), ServerFollow { leave_on: None, clear_refusal: true }, "a connection made afresh tries again");
        assert_eq!(server_follow("", a, false, true, false), ServerFollow { leave_on: None, clear_refusal: false }, "the first frame is no switch");
    }

    /// Review of BUG-135 option 2, finding 13: a device that was offline during the erase is
    /// told so when it next connects (the relay's `account_erased` with `earlier`), its socket
    /// is never signed in, and the game shows the sentence saying how to come back. That
    /// server is erased here, not being connected afresh, so the sentence stays; a switch
    /// away from it still clears it, and once the person pressed Connect (the erase is
    /// forgotten here) a fresh connection tries again.
    ///
    /// Seen red 2026-10-04 with `erased_here` ignored (the old rule): "the erased-earlier
    /// sentence was cleared one frame after it was set".
    #[test]
    fn an_erased_server_keeps_its_sentence_until_the_person_comes_back() {
        let (a, b) = ("https://a.example", "https://b.example");
        assert!(!server_follow(a, a, false, false, true).clear_refusal, "the erased-earlier sentence was cleared one frame after it was set");
        assert!(server_follow(a, b, false, false, true).clear_refusal, "a switch away still clears it");
        assert!(server_follow(a, a, false, false, false).clear_refusal, "after Connect, a fresh connection tries again");
    }

    /// FINDINGS 7 and 14 of the third review: each refusal is one plain sentence (the design's
    /// rule) that says what to do, because it now stays under the HUD while it holds, and a
    /// server with no ship says so instead of "a different ship from yours".
    ///
    /// Seen red 2026-10-03 on the a504c5cd9 sentence: "the other-ship sentence says what to
    /// do: Not joining the shared world: this server has a different ship from yours, and
    /// positions only agree when everyone has the same ship.".
    ///
    /// Round 4 (finding 9): every refusal the game can show, each from the path that gives it,
    /// is one sentence with a next step, and no two causes share a sentence: our OWN ship not
    /// loading read "this server has a different ship from yours", so did a welcome with no
    /// plot id, and the two plot refusals named no next step (and put the assembly error, which
    /// can be a sentence of its own, inside theirs). Seen red 2026-10-03 on db551f530: "a plot
    /// our ship does not have: says what to do: Not joining the shared world: your home could
    /// not be placed on the plot this server gave you (p9)."
    ///
    /// BUG-135: the erased sentence named the way back "reconnect", which no control in the
    /// app is called. Seen red 2026-10-04 on 1c41de3b9: "the erased sentence names the real
    /// way back: Out of the shared world: your account on this server was erased, so your
    /// figure and your plot there are gone; reconnect to come back, with a free plot, or a
    /// guest place when the ship is full."
    #[test]
    fn each_refusal_is_one_sentence_that_says_what_to_do() {
        assert!(SHIP_MISMATCH.contains("update"), "the other-ship sentence says what to do: {SHIP_MISMATCH}");
        assert!(NO_SHIP.contains("did not load") && !NO_SHIP.contains("different ship"), "{NO_SHIP}");
        let ship = booted();
        let hash = ship.ship_hash();
        let refusal = |plan: WelcomeHome| match plan {
            WelcomeHome::Refuse { sentence, .. } => sentence,
            other => panic!("expected a refusal: {other:?}"),
        };
        let other_ship = refusal(plan_welcome(Some(&ship), &welcome(Some("p2"), "0123456789abcdef"), &arriving(P1_DOOR)));
        let own_ship = refusal(plan_welcome(None, &welcome(Some("p1"), &hash), &arriving(P1_DOOR)));
        let mut no_id = welcome(Some("p2"), &hash);
        no_id["home_plot"].as_object_mut().unwrap().remove("id");
        let no_id = refusal(plan_welcome(Some(&ship), &no_id, &arriving(P1_DOOR)));
        let mut unknown = welcome(Some("p2"), &hash);
        unknown["home_plot"]["id"] = serde_json::json!("p9");
        let unknown = refusal(plan_welcome(Some(&ship), &unknown, &arriving(P1_DOOR)));
        let mut file = ShipStructure::load_ship_file(&data_dir()).unwrap();
        let p2 = file.plots.iter().position(|p| p.id == "p2").unwrap();
        file.plots[p2].size.0 = 30.0;
        let narrow = file.assemble(ship.home_design().unwrap(), "p1").expect("the home stands on p1");
        let no_fit = refusal(plan_welcome(Some(&narrow), &welcome(Some("p2"), &narrow.ship_hash()), &arriving(P1_DOOR)));
        let causes = [
            ("another ship", other_ship),
            ("a server with no ship", NO_SHIP.to_string()),
            ("our own ship did not load", own_ship),
            ("a welcome with no plot id", no_id),
            ("a plot our ship does not have", unknown),
            ("a plot our home does not fit", no_fit),
            ("our account erased while we stood in the world", ERASED.to_string()),
        ];
        for (cause, s) in &causes {
            assert_eq!(s.matches(". ").count(), 0, "{cause}: one sentence: {s}");
            assert!(s.ends_with('.'), "{cause}: {s}");
            assert!(["update", "restart", "reconnect", "Connect"].iter().any(|w| s.contains(w)), "{cause}: says what to do: {s}");
        }
        // The erased account's way back is a control the app really has (BUG-135): the
        // Chat page's Connect, which signs up again. No control is called "reconnect".
        assert!(
            ERASED.contains("Chat") && ERASED.contains("Connect") && ERASED.contains("signs you up again") && !ERASED.contains("reconnect"),
            "the erased sentence names the real way back: {ERASED}"
        );
        for (i, (a, sa)) in causes.iter().enumerate() {
            for (b, sb) in &causes[i + 1..] {
                assert_ne!(sa, sb, "{a} and {b} read the same");
            }
        }
    }

    /// ROUND 4, finding 1: the join waits for the world and its ship. A returning player's game
    /// identifies on the main menu; on the frame Enter World is pressed the co-presence block
    /// runs before `load_world` (lib.rs), so the first build joined there with no ship, the
    /// relay refused it as another ship, and the refusal held until a reconnect (the rig's menu
    /// entry: "refused=true (\"Not joining the shared world: this server has a different ship
    /// from yours, ...\")"). A world that loaded with no ship of its own says so instead.
    ///
    /// Seen red 2026-10-03 with `join_step` carrying db551f530's rule: "a join on the frame
    /// Enter World was pressed, before the world loaded: left: Join, right: Wait".
    #[test]
    fn the_join_waits_for_the_world_and_its_ship() {
        let ready = JoinGate {
            in_world: true,
            joined: false,
            identified: true,
            solo: false,
            aboard: true,
            refused_here: false,
            world_loaded: true,
            has_ship: true,
        };
        assert_eq!(join_step(&ready), JoinStep::Join);
        let pressed = JoinGate { world_loaded: false, has_ship: false, ..ready };
        assert_eq!(join_step(&pressed), JoinStep::Wait, "a join on the frame Enter World was pressed, before the world loaded");
        assert_eq!(join_step(&JoinGate { has_ship: false, ..ready }), JoinStep::RefuseOwnShip, "a world with no ship of its own says so");
        for (what, g) in [
            ("not in the world view", JoinGate { in_world: false, ..ready }),
            ("already joined", JoinGate { joined: true, ..ready }),
            ("not identified yet", JoinGate { identified: false, ..ready }),
            ("stepped out", JoinGate { solo: true, ..ready }),
            ("away from the ship", JoinGate { aboard: false, ..ready }),
            ("refused here", JoinGate { refused_here: true, ..ready }),
            ("refused here, no ship", JoinGate { refused_here: true, has_ship: false, ..ready }),
        ] {
            assert_eq!(join_step(&g), JoinStep::Wait, "{what}");
        }
    }

    /// A player on their own home, a parked truck they are driving, and a bus the follow cam
    /// chases.
    fn rider_world() -> (hecs::World, hecs::Entity, hecs::Entity) {
        let tf = |p: Vec3| Transform { position: p, rotation: glam::Quat::IDENTITY, scale: Vec3::ONE };
        let mut w = hecs::World::new();
        w.spawn((tf(Vec3::new(30.0, 0.0, 30.0)), crate::ecs::components::Controllable));
        let truck = w.spawn((
            tf(Vec3::new(30.0, 0.0, 30.0)),
            crate::ecs::components::Vehicle { item_id: "truck_pickup_0".into() },
            crate::ecs::components::VehicleSeat { occupant_key: Some("player".into()), seat_type: "pilot".into() },
        ));
        let bus = w.spawn((tf(Vec3::new(80.0, 0.0, 40.0)), crate::ecs::components::Vehicle { item_id: "truck_pickup_0".into() }));
        (w, truck, bus)
    }

    fn body_at(w: &hecs::World) -> Vec3 {
        w.query::<(&Transform, &crate::ecs::components::Controllable)>().iter().map(|(_, (t, _))| t.position).next().unwrap()
    }

    /// ROUND 4, finding 2: a welcome stands the player where the relay holds them while they
    /// drive a truck or watch one on the follow cam. Each puts the camera back on its vehicle
    /// every frame (lib.rs), so the player stayed there, over 100 m from where the relay holds
    /// them, every update refused. Standing them ends both, the way Respawn does, and frees the
    /// seat.
    ///
    /// Seen red 2026-10-03 with `stand_player_at` moving only the body and the camera (the
    /// db551f530 `put_player_at`): "the player still drives Some(1v1): the next frame puts the
    /// camera back in the cab".
    #[test]
    fn standing_the_player_where_the_relay_holds_them_ends_a_drive_and_a_follow() {
        let (mut w, truck, bus) = rider_world();
        let (mut camera, mut back) = (Vec3::new(30.0, 3.0, 30.0), Vec3::ZERO);
        let (mut driving, mut following) = (Some(truck), Some(bus));
        let mut ed = Vec3::ZERO;
        let standing = Standing { camera: &mut camera, showroom_open: false, showroom_return: &mut back, editor_open: false, editor_return: &mut ed, driving: &mut driving, following: &mut following };
        stand_player_at(&mut w, standing, P2_DOOR);
        assert_eq!(driving, None, "the player still drives {driving:?}: the next frame puts the camera back in the cab");
        assert_eq!(following, None, "the follow cam still chases {following:?}: the next frame puts the camera back behind it");
        let seat = w.get::<&crate::ecs::components::VehicleSeat>(truck).unwrap().occupant_key.clone();
        assert_eq!(seat, None, "the truck's seat is free again");
        assert_eq!(body_at(&w), P2_DOOR);
        assert_eq!(camera, P2_DOOR);
        assert_eq!(back, Vec3::ZERO, "the showroom is shut: its return point is not touched");
    }

    /// ROUND 4, finding 8: a welcome lands while the character showroom is open (the join went
    /// out, then the player opened ESC > Play before it came back). Closing the showroom puts
    /// the camera at the point it opened from, which undid the welcome: the player went back
    /// to their old spot, up to 99 m from where the relay holds them. The welcome now moves
    /// that point, and leaves the showroom's own camera alone.
    ///
    /// Seen red 2026-10-03 with `stand_player_at` moving only the body and the camera (the
    /// db551f530 `put_player_at`): "closing the showroom puts the player back at Vec3(53.5,
    /// 1.7, 40.5), where they stood before the welcome".
    #[test]
    fn a_welcome_while_the_showroom_is_open_lands_where_it_closes() {
        let (mut w, _, _) = rider_world();
        let showroom_camera = Vec3::new(5.0, 2.0, 5.0);
        let (mut camera, mut back) = (showroom_camera, P1_DOOR);
        let mut ed = Vec3::ZERO;
        let standing = Standing { camera: &mut camera, showroom_open: true, showroom_return: &mut back, editor_open: false, editor_return: &mut ed, driving: &mut None, following: &mut None };
        stand_player_at(&mut w, standing, P2_DOOR);
        assert_eq!(back, P2_DOOR, "closing the showroom puts the player back at {back:?}, where they stood before the welcome");
        assert_eq!(camera, showroom_camera, "the showroom's own camera is left alone");
        assert_eq!(body_at(&w), P2_DOOR);
    }

    /// A world with one chest standing at `at`.
    fn chest_world(at: Vec3) -> hecs::World {
        let mut w = hecs::World::new();
        w.spawn((
            Transform { position: at, rotation: glam::Quat::IDENTITY, scale: Vec3::ONE },
            crate::systems::construction::Structure { blueprint_id: "chest".into(), health: 1.0, max_health: 1.0, provides: None, uid: 5 },
        ));
        w
    }

    fn box_of(b: PlotBox) -> [[f32; 3]; 2] {
        [b.0.to_array(), b.1.to_array()]
    }

    /// ROUND 4, finding 4, the carry: the home stood on p2; the next launch applied the save
    /// and was closed before the world loaded, so the save on quit was written with no frame
    /// published, its chest still at p2's (20, 129). It records the box the startup save left
    /// waiting (`save_load::frame_for_save`), and the launch after carries the chest into the
    /// home on p1. (save_load.rs `a_save_before_the_world_loads_keeps_the_box_its_pieces_stand_in`
    /// checks the save path itself.)
    ///
    /// Seen red 2026-10-03 with `frame_for_save` reading only the published frame (the
    /// db551f530 save): "the chest saved before the world loaded stands at Vec3(20.0, 0.0,
    /// 129.0), outside the home on p1".
    #[test]
    fn a_save_before_the_world_loads_is_carried_into_the_home_at_the_next_launch() {
        let on_p2 = ShipStructure::load_and_assemble_shipped(&data_dir(), Some("p2")).unwrap();
        let (p1, p2) = (home_box(&booted()).unwrap(), home_box(&on_p2).unwrap());
        let mut data = crate::hot_reload::data_store::DataStore::new();
        data.insert(crate::save_load::LOADED_HOME_BOX_KEY, crate::save_load::LoadedHomeBox(box_of(p2)));
        let w = chest_world(Vec3::new(20.0, 0.0, 129.0));
        let mut save = crate::save_load::extract_world_save(&w);
        crate::save_load::record_home_frame(&mut save, crate::save_load::frame_for_save(&data).as_ref());
        let mut next = hecs::World::new();
        crate::save_load::apply_save_to_world(&mut next, &save);
        carry_saved_pieces(&mut next, save.home_plot_box, Some(p1));
        let (pieces, _) = built_positions(&next);
        let chest = pieces[0];
        assert!(over_plot(chest, p1), "the chest saved before the world loaded stands at {chest:?}, outside the home on p1");
    }

    /// ROUND 4, finding 5: the ship did not assemble, so the legacy layout shows and there is
    /// no home to carry the startup save's pieces to. They stay where they were saved, and the
    /// box they stand in must stay too, or every save of that session records none.
    ///
    /// Seen red 2026-10-03 with `carry_loaded_box` letting the box go first (the db551f530
    /// `carry_loaded_save_home`): "a session on the legacy layout forgot the box its pieces
    /// stand in".
    #[test]
    fn a_session_on_the_legacy_layout_keeps_the_saved_box() {
        let on_p2 = ShipStructure::load_and_assemble_shipped(&data_dir(), Some("p2")).unwrap();
        let (p1, p2) = (home_box(&booted()).unwrap(), home_box(&on_p2).unwrap());
        let mut data = crate::hot_reload::data_store::DataStore::new();
        data.insert(crate::save_load::LOADED_HOME_BOX_KEY, crate::save_load::LoadedHomeBox(box_of(p2)));
        let mut w = chest_world(Vec3::new(20.0, 0.0, 129.0));
        assert_eq!(carry_loaded_box(&mut w, &mut data, None), None, "nothing to carry to");
        assert!(
            data.get::<crate::save_load::LoadedHomeBox>(crate::save_load::LOADED_HOME_BOX_KEY).is_some(),
            "a session on the legacy layout forgot the box its pieces stand in"
        );
        let recorded = crate::save_load::frame_for_save(&data).map(|f| box_of(f.home));
        assert_eq!(recorded, Some(box_of(p2)), "a save of that session records where its pieces stand");
        // With a home to carry to, the box does its work and goes.
        assert_eq!(carry_loaded_box(&mut w, &mut data, Some(p1)).map(|(n, _, _)| n), Some(1));
        assert!(data.get::<crate::save_load::LoadedHomeBox>(crate::save_load::LOADED_HOME_BOX_KEY).is_none());
        assert!(over_plot(built_positions(&w).0[0], p1));
    }

    /// ROUND 4, finding 6: a save that records no box (written before 1b, or a snapshot from
    /// then) has its pieces in the default plot's frame, where every home stood. Loaded into a
    /// running world whose home stands on p2 (the launcher's pick, a restored snapshot), they
    /// go into the home on p2.
    ///
    /// Seen red 2026-10-03 with `saved_or_default_box` passing the missing box on (the
    /// db551f530 load, which carried nothing): "a pre-1b save loaded while the home stands on
    /// p2 leaves the chest at Vec3(20.0, 0.0, 30.0), outside the home".
    #[test]
    fn a_pre_1b_save_loaded_while_the_home_stands_on_p2_lands_in_the_home() {
        let on_p2 = ShipStructure::load_and_assemble_shipped(&data_dir(), Some("p2")).unwrap();
        let p2 = home_box(&on_p2).unwrap();
        let save = crate::save_load::extract_world_save(&chest_world(Vec3::new(20.0, 0.0, 30.0)));
        assert_eq!(save.home_plot_box, None, "a save of that time records no box");
        let mut w = hecs::World::new();
        crate::save_load::apply_save_to_world(&mut w, &save);
        carry_saved_pieces(&mut w, saved_or_default_box(save.home_plot_box, Some(&on_p2)), Some(p2));
        let chest = built_positions(&w).0[0];
        assert!(over_plot(chest, p2), "a pre-1b save loaded while the home stands on p2 leaves the chest at {chest:?}, outside the home");
        // A save that records its box keeps it, and no ship reads nothing in.
        let b = [[1.0, 0.0, 2.0], [3.0, 4.0, 5.0]];
        assert_eq!(saved_or_default_box(Some(b), Some(&on_p2)), Some(b));
        assert_eq!(saved_or_default_box(None, None), None);
    }

    /// ROUND 4, finding 7: the `NotTheHomes` mark is saved with what it marks. The home stood
    /// on p2 with a truck left standing on p1; the next launch built the home on p1, carried
    /// the save there and marked the truck; a save was written before the welcome (on quit, or
    /// the periodic one), in the frame of the home on p1, where the truck stands. The launch
    /// after that, the welcome moves the home to p2 again: the truck stays on p1.
    ///
    /// Seen red 2026-10-03 with the mark not saved (`outside_home` written false, the
    /// db551f530 save, which had no such field): "a truck left on p1 was adopted into the home
    /// on the launch after a save before the welcome: [Vec3(40.0, 0.0, 169.0), Vec3(90.0, 0.0,
    /// 50.0), Vec3(14.0, 0.0, 184.0)]".
    #[test]
    fn a_truck_left_behind_stays_behind_through_a_save_before_the_welcome() {
        let on_p2 = ShipStructure::load_and_assemble_shipped(&data_dir(), Some("p2")).unwrap();
        let (p1, p2) = (home_box(&booted()).unwrap(), home_box(&on_p2).unwrap());
        let tf = |x: f32, z: f32| Transform { position: Vec3::new(x, 0.0, z), rotation: glam::Quat::IDENTITY, scale: Vec3::ONE };
        let mut live = built_world();
        carry_built_pieces(&mut live, p1, p2.0 - p1.0); // the home on p2 ...
        live.spawn((tf(14.0, 85.0), crate::ecs::components::Vehicle { item_id: "truck_pickup_0".into() })); // ... a truck left on p1
        let mut first = crate::save_load::extract_world_save(&live);
        crate::save_load::record_home_frame(&mut first, home_frame(&on_p2).as_ref());
        // The next launch: the home on p1, the save carried there, the truck marked.
        let mut boot = hecs::World::new();
        crate::save_load::apply_save_to_world(&mut boot, &first);
        assert_eq!(carry_saved_pieces(&mut boot, first.home_plot_box, Some(p1)).2, 1, "the truck left on p1 is marked");
        // Saved before the welcome, in the frame of the home on p1.
        let mut second = crate::save_load::extract_world_save(&boot);
        crate::save_load::record_home_frame(&mut second, home_frame(&booted()).as_ref());
        // The launch after: the home on p1 again, then the welcome moves it to p2.
        let mut again = hecs::World::new();
        crate::save_load::apply_save_to_world(&mut again, &second);
        carry_saved_pieces(&mut again, second.home_plot_box, Some(p1));
        carry_built_pieces(&mut again, p1, p2.0 - p1.0);
        clear_not_the_homes(&mut again);
        let trucks: Vec<Vec3> =
            again.query::<(&Transform, &crate::ecs::components::Vehicle)>().iter().map(|(_, (t, _))| t.position).collect();
        assert!(trucks.contains(&Vec3::new(14.0, 0.0, 85.0)), "a truck left on p1 was adopted into the home on the launch after a save before the welcome: {trucks:?}");
        assert!(trucks.contains(&Vec3::new(40.0, 0.0, 169.0)), "the truck parked in the home went with it: {trucks:?}");
        // Once the home has moved, the mark has done its work: the next save carries none.
        let third = crate::save_load::extract_world_save(&again);
        assert!(third.deployed_vehicles.iter().all(|v| !v.outside_home), "a mark outlived the move");
    }
    /// ROUND 5, finding 1: closing the build editor far from where the relay holds the player.
    /// B works anywhere aboard; a joined player at the far end of First Street, (70, 1.7, 190),
    /// pressed B twice, and the close put them at their build spot in the home, the home spawn
    /// (53.5, 1.7, 40.5), about 150 m from where the relay last held them: every update after
    /// that was refused, and everyone else saw them frozen on the street. In the shared world
    /// the close now leaves them where they stood; out of it, or near enough, the pick stands.
    ///
    /// Seen red 2026-10-04 with `editor_close_spot` carrying the c8b3a8d54 rule (the pick,
    /// wherever it is): "closing the editor at the far end of First Street put the player
    /// at Vec3(53.5, 1.7, 40.5), 150 m from where the relay holds them".
    #[test]
    fn closing_the_build_editor_far_from_where_the_relay_holds_us_leaves_us_there() {
        let street_end = Vec3::new(70.0, 1.7, 190.0);
        let p1 = Some(p1_box());
        let c = editor_close_spot(Some(P1_DOOR), street_end, true, false, p1);
        assert_eq!(
            c.at,
            street_end,
            "closing the editor at the far end of First Street put the player at {:?}, {:.0} m from where the relay holds them",
            c.at,
            c.at.distance(street_end)
        );
        assert!(c.held_back, "the player is told why the build spot was not used");
        // Near where the relay holds them, the build spot is used, as before.
        let near = Vec3::new(60.0, 1.7, 70.0);
        assert_eq!(editor_close_spot(Some(P1_DOOR), near, true, false, p1), EditorClose { at: P1_DOOR, held_back: false });
        // Out of the shared world nothing holds them: the build spot, wherever it is.
        assert_eq!(editor_close_spot(Some(P1_DOOR), street_end, false, false, p1), EditorClose { at: P1_DOOR, held_back: false });
        // No build spot and no home spawn: where they stood.
        assert_eq!(editor_close_spot(None, street_end, true, false, p1), EditorClose { at: street_end, held_back: false });
        // The margin is the welcome's own (`FAR_FROM_HELD_M`): at it, the pick; past it, held. (A
        // plot as big as the street, so only the distance decides here.)
        let street_plot = Some((Vec3::new(0.0, 0.0, 0.0), Vec3::new(200.0, 3.0, 200.0)));
        let at_edge = street_end + Vec3::new(0.0, 0.0, -FAR_FROM_HELD_M);
        assert_eq!(editor_close_spot(Some(at_edge), street_end, true, false, street_plot).at, at_edge);
        let past = street_end + Vec3::new(0.0, 0.0, -FAR_FROM_HELD_M - 0.5);
        assert_eq!(editor_close_spot(Some(past), street_end, true, false, street_plot).at, street_end);
    }

    /// The plot p1's box, ship metres (data/blueprints/ship_structure.ron).
    fn p1_box() -> PlotBox {
        (Vec3::ZERO, Vec3::new(55.0, 3.0, 89.0))
    }

    /// INCREMENT 4: in the shared world, shutting the build editor stands the player at their
    /// build spot only on their OWN plot, the one place the relay lets that jump land
    /// (src/relay/handlers/move_check.rs, `MoveDecl::Editor`). A Dev's build-mode avatar left in
    /// the Commons it was editing, 20 m away, holds them back with the notice; the same spot out
    /// of the shared world is used as before; a spot on their own plot is used.
    /// Seen red 2026-10-04 with the own-plot condition left out: "a build spot in the Commons
    /// put the player at Vec3(80.0, 1.7, 60.0)", which the relay corrects.
    #[test]
    fn shutting_the_editor_stands_us_only_on_our_own_plot() {
        let held = Vec3::new(60.0, 1.7, 45.0); // in the corridor from p1 to the Commons
        let in_commons = Vec3::new(80.0, 1.7, 60.0);
        let c = editor_close_spot(Some(in_commons), held, true, false, Some(p1_box()));
        assert_eq!(c.at, held, "a build spot in the Commons put the player at {:?}", c.at);
        assert!(c.held_back);
        assert_eq!(editor_close_spot(Some(in_commons), held, false, false, Some(p1_box())).at, in_commons, "out of the shared world");
        assert_eq!(editor_close_spot(Some(P1_DOOR), held, true, false, Some(p1_box())).at, P1_DOOR, "on their own plot");
        assert_eq!(editor_close_spot(Some(P1_DOOR), held, true, false, None).at, held, "with no plot, nowhere is theirs");
    }

    /// ROUND 5, finding 1, the other half: a welcome lands while the build editor is open (a
    /// reconnect, or the first welcome after the editor was opened on joining). It wrote the
    /// camera, which is the editor's orbit camera then, and the close then put the player at
    /// their build spot or the point the editor opened from, undoing the welcome without the
    /// relay knowing. Now, as for the showroom, the welcome moves the point the editor closes
    /// onto and leaves the editor's camera alone, and the close judges the build spot against
    /// that point.
    ///
    /// Seen red 2026-10-04 with `stand_player_at` handling only the showroom (the c8b3a8d54
    /// rule): "closing the editor puts the player back at Vec3(53.5,
    /// 1.7, 40.5), where they stood before the welcome".
    #[test]
    fn a_welcome_while_the_build_editor_is_open_lands_where_it_closes() {
        let (mut w, _, _) = rider_world();
        let orbit = Vec3::new(40.0, 14.0, 52.0);
        let (mut camera, mut back, mut ed) = (orbit, Vec3::ZERO, P1_DOOR);
        let standing = Standing {
            camera: &mut camera,
            showroom_open: false,
            showroom_return: &mut back,
            editor_open: true,
            editor_return: &mut ed,
            driving: &mut None,
            following: &mut None,
        };
        stand_player_at(&mut w, standing, P2_DOOR);
        assert_eq!(ed, P2_DOOR, "closing the editor puts the player back at {ed:?}, where they stood before the welcome");
        assert_eq!(camera, orbit, "the editor's orbit camera is left alone");
        assert_eq!(back, Vec3::ZERO, "the showroom is shut: its return point is not touched");
        assert_eq!(body_at(&w), P2_DOOR);
        // A build spot left in the home on p1, 99 m from p2's door, where the welcome stood
        // them: the close leaves them at the door.
        assert_eq!(editor_close_spot(Some(P1_DOOR), ed, true, false, Some(p1_box())).at, P2_DOOR);
    }

    /// The game's side of an erase in the shared world (ROUND 5, findings 2 and 4): the relay
    /// takes the figure out and tells the erasing game (`game_join_denied`, reason
    /// "account_erased"), and the game leaves the shared world on its side with one sentence
    /// that says how to come back, and does not join again on that server by itself (which
    /// would claim a new plot for the account just erased). The relay's half is the relay test
    /// `erasing_in_the_world_tells_the_erasing_game_it_left`.
    ///
    /// Seen red 2026-10-04 with `join_denied_sentence` carrying only the ship reasons (the
    /// c8b3a8d54 net_route.rs): "an erase that took our figure out is not acted on" (left: None).
    #[test]
    fn an_erased_account_leaves_the_shared_world_with_one_sentence() {
        assert_eq!(
            join_denied_sentence("account_erased"),
            Some(ERASED),
            "an erase that took our figure out is not acted on"
        );
        for reason in ["other_ship", "no_ship", "no_ship_named"] {
            assert!(join_denied_sentence(reason).is_some(), "{reason}");
        }
        assert_eq!(join_denied_sentence("spamming the commons"), None, "a game ban's reason is shown under the admin controls");
        assert_eq!(join_denied_sentence(""), None);
    }

    /// ROUND 5, finding 5: a welcome that keeps the home where it is settles what stands in it.
    /// On server X the player held p2 and left a truck on p1. The next launch built the home on
    /// p1, carried the save there and marked the truck (it stands in the home but was not the
    /// home's). Then they joined server Y, which gives them p1: the home stays, and the truck
    /// now stands in the home they hold there. Only a move cleared the mark, and since round 4
    /// it is saved, so it lasted every launch: two launches later a server that gives p2 moved
    /// the home and left the truck behind on p1, someone else's plot there.
    ///
    /// Seen red 2026-10-04 with `welcome_settles_the_home` doing nothing (the c8b3a8d54 rule,
    /// only a move cleared marks): "a truck standing in the home a server let it keep was
    /// left behind by the next move: [Vec3(14.0, 0.0, 85.0)]".
    #[test]
    fn a_welcome_that_keeps_the_home_where_it_is_settles_what_stands_in_it() {
        let on_p2 = ShipStructure::load_and_assemble_shipped(&data_dir(), Some("p2")).unwrap();
        let (p1, p2) = (home_box(&booted()).unwrap(), home_box(&on_p2).unwrap());
        let tf = |x: f32, z: f32| Transform { position: Vec3::new(x, 0.0, z), rotation: glam::Quat::IDENTITY, scale: Vec3::ONE };
        // Server X: the home on p2, a truck parked on p1, outside it.
        let mut live = hecs::World::new();
        live.spawn((tf(14.0, 85.0), crate::ecs::components::Vehicle { item_id: "truck_pickup_0".into() }));
        let mut first = crate::save_load::extract_world_save(&live);
        crate::save_load::record_home_frame(&mut first, home_frame(&on_p2).as_ref());
        // The next launch: the home on p1, the save carried there, the truck marked.
        let mut boot = hecs::World::new();
        crate::save_load::apply_save_to_world(&mut boot, &first);
        assert_eq!(carry_saved_pieces(&mut boot, first.home_plot_box, Some(p1)).2, 1, "the truck on p1 is marked");
        // Server Y gives p1: the welcome keeps the home where it is.
        welcome_settles_the_home(&mut boot);
        // A save, and the launch after.
        let mut second = crate::save_load::extract_world_save(&boot);
        crate::save_load::record_home_frame(&mut second, home_frame(&booted()).as_ref());
        let mut again = hecs::World::new();
        crate::save_load::apply_save_to_world(&mut again, &second);
        carry_saved_pieces(&mut again, second.home_plot_box, Some(p1));
        // A server that gives p2: the home moves, and what stands in it goes along.
        carry_built_pieces(&mut again, p1, p2.0 - p1.0);
        let trucks: Vec<Vec3> =
            again.query::<(&Transform, &crate::ecs::components::Vehicle)>().iter().map(|(_, (t, _))| t.position).collect();
        assert_eq!(
            trucks,
            vec![Vec3::new(14.0, 0.0, 184.0)],
            "a truck standing in the home a server let it keep was left behind by the next move: {trucks:?}"
        );
    }

    /// ROUND 5, finding 6: a snapshot restored on the legacy layout. The ship did not assemble,
    /// so the box the startup save left waiting (p2's here) stays for the session (round 4,
    /// finding 5). Then a snapshot from when the home stood on p1 is restored: its pieces stand
    /// in p1's box, and nothing carried them (there is no home), but every later save of that
    /// session still recorded p2's box, so the next launch with a working ship did not carry
    /// them into the home. The box a save records is now the loaded save's.
    ///
    /// Seen red 2026-10-04 with `carry_loaded_save` leaving the waiting box alone (the
    /// c8b3a8d54 rule): "a save after restoring a snapshot on the legacy
    /// layout records Some([[0.0, 0.0, 99.0], [55.0, 3.0, 188.0]]), the startup save's box,
    /// while the restored pieces stand in p1's".
    #[test]
    fn a_snapshot_restored_on_the_legacy_layout_is_the_box_later_saves_record() {
        let on_p2 = ShipStructure::load_and_assemble_shipped(&data_dir(), Some("p2")).unwrap();
        let (p1, p2) = (home_box(&booted()).unwrap(), home_box(&on_p2).unwrap());
        let mut data = crate::hot_reload::data_store::DataStore::new();
        data.insert(crate::save_load::LOADED_HOME_BOX_KEY, crate::save_load::LoadedHomeBox(box_of(p2)));
        let mut w = chest_world(Vec3::new(20.0, 0.0, 30.0));
        assert_eq!(carry_loaded_save(&mut w, &mut data, Some(box_of(p1)), None), (0, 0, 0), "no home to carry to");
        let recorded = crate::save_load::frame_for_save(&data).map(|f| box_of(f.home));
        assert_eq!(
            recorded,
            Some(box_of(p1)),
            "a save after restoring a snapshot on the legacy layout records {recorded:?}, the startup save's box, while the restored pieces stand in p1's"
        );
        // The next launch, with a working ship and the home on p2, carries them into it.
        let mut save = crate::save_load::extract_world_save(&w);
        crate::save_load::record_home_frame(&mut save, crate::save_load::frame_for_save(&data).as_ref());
        let mut next = hecs::World::new();
        crate::save_load::apply_save_to_world(&mut next, &save);
        carry_saved_pieces(&mut next, save.home_plot_box, Some(p2));
        let chest = built_positions(&next).0[0];
        assert!(over_plot(chest, p2), "the restored chest stands at {chest:?}, outside the home on p2");
        // A restored save that records no box: the saves after it record none either.
        carry_loaded_save(&mut w, &mut data, None, None);
        assert_eq!(crate::save_load::frame_for_save(&data), None);
    }
}
