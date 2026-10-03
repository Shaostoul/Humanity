//! P2P Trading page - live escrow trades against the connected relay
//! (v0.756, closure ladder rung 5). The relay stores the whole flow
//! (request, accept/reject, per-side item offers, dual confirm, cancel)
//! and delivers state through targeted `__trade_data__:` / `__trade_list__:`
//! private wrappers that lib.rs routes into GuiState.trades. This page
//! renders that live state and sends the typed WS requests back - the
//! round-trip is the truth, no local echo. (Replaced the hardcoded
//! Alice/Bob/Carol mock that shipped with the first page skeleton.)
//!
//! TRADES MOVE ITEMS (2026-09-29). An offer line names an item you carry (typed
//! by name or id, resolved against your backpack, refused otherwise) and carries
//! its items.csv id, wear and grade. Confirm waits until you carry everything you
//! offered. When the relay completes the trade, each player's own game hands over
//! its side: what you offered leaves your backpack, what they offered arrives
//! (`settle_completed`). The relay holds no inventories yet, so it cannot keep the
//! items in escrow; the confirm check is the guard, and a line typed as text (the
//! web page's offers) is shown but cannot move.
//!
//! WITH THE PAGE CLOSED (2026-10-02). Everything that makes a trade actually
//! happen now runs from the relay pump every frame, not from this page:
//! `route_trade_frame` takes the trade wrappers (which arrive as `system` frames,
//! the relay's conversion of every targeted message; the client used to look for
//! them only under `private` and so never listed or settled a trade), and `tick`
//! fetches the trade list on every connect and withdraws a confirmation whose
//! offered items have since left the backpack. Which trades are settled is saved
//! with the backpack (`TradeSettlements`), so a trade that completed while this
//! game was closed settles at the next connect, once.
//!
//! AFTER A SAVE LOAD (2026-10-02, review blocker). The relay connects at the
//! main menu, so a trade can settle into the home loaded at startup before the
//! player clicks Play; Play then loads a save from disk (`launcher_pending_load`
//! in lib.rs, `save_load::apply_save_to_world`), which puts back the backpack
//! AND the settled set from before the trade. `tick` therefore settles, every
//! frame, any completed trade in hand that the loaded backpack has not settled,
//! so the trade lands once in the world that will actually be saved. Settling at
//! the menu is not held back: a player who never clicks Play keeps that world,
//! and the periodic save writes it.

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::{GuiItemSlot, GuiState, GuiTrade, GuiTradeItem};
use crate::systems::inventory::{Inventory, TradeSettlements, TransferOp};
use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Vec2};

/// Colour for a relay trade status string.
fn status_color(theme: &Theme, status: &str) -> Color32 {
    match status {
        "pending" => theme.warning(),
        "active" => Theme::c32(&theme.info),
        "completed" => theme.success(),
        _ => theme.danger(), // cancelled / rejected
    }
}

/// Short display for the other party's key.
fn key_label(key: &str) -> String {
    if key.len() > 10 {
        format!("{}...", &key[..10])
    } else {
        key.to_string()
    }
}

/// One line of my offer draft (2026-09-29): the carried item it names (its
/// items.csv id, display name, and the stack's wear and grade) and the quantity
/// text. An empty `item_id` is a line typed as text, which cannot move.
#[derive(Debug, Clone, Default, PartialEq)]
struct OfferLine {
    item_id: String,
    name: String,
    qty: String,
    wear: u32,
    quality: u8,
}

impl OfferLine {
    fn from_item(i: &GuiTradeItem) -> Self {
        Self {
            item_id: i.reference_id.clone().unwrap_or_default(),
            name: i.name.clone(),
            qty: i.quantity.to_string(),
            wear: i.wear,
            quality: i.quality,
        }
    }

    /// The relay's TradeItem shape; the id, wear and grade ride along.
    fn to_json(&self) -> serde_json::Value {
        let mut v = serde_json::json!({
            "item_type": "item",
            "name": self.name.trim(),
            "quantity": self.qty.trim().parse::<u32>().unwrap_or(1).max(1),
            "description": "",
        });
        if !self.item_id.is_empty() {
            v["reference_id"] = serde_json::json!(self.item_id);
            v["wear"] = serde_json::json!(self.wear);
            v["quality"] = serde_json::json!(self.quality);
        }
        v
    }
}

/// The carried stack a typed offer means: an item id or a name, ignoring case.
/// None when nothing in the backpack matches.
fn resolve_carried(inv: &[Option<GuiItemSlot>], typed: &str) -> Option<GuiItemSlot> {
    let t = typed.trim();
    if t.is_empty() {
        return None;
    }
    inv.iter().flatten().find(|s| s.item_id.eq_ignore_ascii_case(t) || s.name.eq_ignore_ascii_case(t)).cloned()
}

/// How many of an item the backpack holds, across all its stacks.
fn carried_count(inv: &[Option<GuiItemSlot>], item_id: &str) -> u32 {
    inv.iter().flatten().filter(|s| s.item_id == item_id).map(|s| s.quantity).sum()
}

/// Why this offer cannot be confirmed yet, or None when the backpack holds all
/// of it. Lines of the same item are added up before counting.
fn offer_shortfall(inv: &[Option<GuiItemSlot>], offer: &[GuiTradeItem]) -> Option<String> {
    if let Some(i) = offer.iter().find(|i| i.reference_id.is_none()) {
        return Some(format!("\"{}\" is text, not an item from your backpack: remove it and add the item by name.", i.name));
    }
    carried_shortfall(inv, offer)
}

/// The first carried item an offer names that the backpack no longer covers,
/// as a sentence (2026-10-02, split out of `offer_shortfall` for the confirm
/// guard). Text lines are skipped: no backpack item stands behind them, so a
/// text offer confirmed on the web page is not this game's to withdraw.
fn carried_shortfall(inv: &[Option<GuiItemSlot>], offer: &[GuiTradeItem]) -> Option<String> {
    let mut wanted: Vec<(&str, &str, u32)> = Vec::new();
    for i in offer {
        let Some(id) = i.reference_id.as_deref() else { continue };
        match wanted.iter_mut().find(|w| w.0 == id) {
            Some(w) => w.2 += i.quantity,
            None => wanted.push((id, &i.name, i.quantity)),
        }
    }
    wanted.into_iter().find_map(|(id, name, q)| {
        let have = carried_count(inv, id);
        (have < q).then(|| format!("You offer {q} x {name} but carry {have}."))
    })
}

/// One offer line in the relay's TradeItem shape, every field kept, so a
/// re-sent offer is the same offer (2026-10-02, the confirm guard).
fn item_json(i: &GuiTradeItem) -> serde_json::Value {
    let mut v = serde_json::json!({
        "item_type": i.item_type,
        "name": i.name,
        "quantity": i.quantity,
        "description": i.description,
    });
    if let Some(id) = &i.reference_id {
        v["reference_id"] = serde_json::json!(id);
        v["wear"] = serde_json::json!(i.wear);
        v["quality"] = serde_json::json!(i.quality);
    }
    v
}

/// My side of a trade: (what I offer, whether I confirmed). None when I am
/// not a party to it.
fn my_side<'a>(t: &'a GuiTrade, me: &str) -> Option<(&'a [GuiTradeItem], bool)> {
    if me.is_empty() {
        None
    } else if t.initiator_key == me {
        Some((&t.initiator_items, t.initiator_confirmed))
    } else if t.recipient_key == me {
        Some((&t.recipient_items, t.recipient_confirmed))
    } else {
        None
    }
}

/// Confirmations to withdraw (2026-10-02): every active trade I confirmed whose
/// offered items the backpack no longer covers, as (trade id, the
/// `trade_update_items` message re-sending the same offer, the line to show).
///
/// WHY re-send the offer: the relay clears BOTH confirmations on any item
/// update, so this is the one request that un-confirms. Before it, Confirm was
/// checked once, when clicked; afterwards the player could drop, use or store
/// the items, the trade still completed, and the other player received items
/// this player never lost. This only narrows the gap: two confirmations can
/// still land before this game notices, and true escrow needs the server to
/// hold the inventories (it holds none yet).
fn withdrawals(gs: &GuiState) -> Vec<(String, String, String)> {
    gs.trades
        .iter()
        .filter(|t| t.status == "active")
        .filter_map(|t| {
            let (mine, confirmed) = my_side(t, &gs.profile_public_key)?;
            if !confirmed {
                return None;
            }
            let why = carried_shortfall(&gs.inventory_items, mine)?;
            let items: Vec<serde_json::Value> = mine.iter().map(item_json).collect();
            let msg = serde_json::json!({"type": "trade_update_items", "trade_id": t.id, "items": items});
            let line = format!("{why} Your confirmation was withdrawn; confirm again once you carry it.");
            Some((t.id.clone(), msg.to_string(), line))
        })
        .collect()
}

/// Withdraw my confirmation on every trade whose offered items have left the
/// backpack, once per confirmation: the id is remembered until the relay's
/// answer shows me unconfirmed, so the request is not re-sent every frame.
fn guard_confirmed_offers(gs: &mut GuiState) {
    let wanted = withdrawals(gs);
    let still_confirmed: Vec<String> = gs
        .trades
        .iter()
        .filter(|t| t.status == "active" && my_side(t, &gs.profile_public_key).is_some_and(|(_, c)| c))
        .map(|t| t.id.clone())
        .collect();
    with_state(|ts| ts.withdrawn.retain(|id| still_confirmed.contains(id)));
    for (id, msg, line) in wanted {
        if with_state(|ts| ts.withdrawn.insert(id)) {
            if let Some(ws) = &gs.ws_client {
                ws.send(&msg);
            }
            // As a notice too (2026-10-02): `trade_status` shows only on the
            // Trade page, which is usually closed when the backpack changes.
            gs.pending_notices.push(line.clone());
            gs.trade_status = line;
        }
    }
}

/// Whether to ask for my trade list now (2026-10-02): once per identified
/// connection, whether or not the Trade page is ever opened, so a trade that
/// completed while this game was closed or offline reaches it and settles.
/// `synced` re-arms whenever the link is not identified (a drop, a reconnect).
fn list_request_due(synced: &mut bool, identified: bool) -> bool {
    if !identified {
        *synced = false;
        return false;
    }
    !std::mem::replace(synced, true)
}

/// Whether to send the automatic list request now (`list_request_due`),
/// remembering while it is unanswered (2026-10-02) so that a server without
/// the game feature can have its refusal of it kept out of chat
/// (`refuses_auto_list`). Forgotten whenever the link is not identified.
fn auto_list_due(synced: &mut bool, identified: bool) -> bool {
    let due = list_request_due(synced, identified);
    with_state(|ts| ts.auto_list_pending = due || (identified && ts.auto_list_pending));
    due
}

/// The start of the relay's refusal when its owner switched the `game`
/// feature off (the capability gate in relay.rs: "This server has 'game'
/// disabled (...)"). Trades belong to `game` (relay::features).
fn game_off_prefix() -> String {
    format!("This server has '{}' disabled", crate::relay::features::Feature::Game.key())
}

/// True when `msg` is that refusal answering the automatic list request
/// (2026-10-02, review should-fix). `tick` asks on every connect, so on a
/// server without the game this refusal was printed into chat each time. Only
/// the one answer is taken: the relay answers a socket's requests in order, so
/// a later refusal (of something the player did) still reaches chat.
fn refuses_auto_list(msg: &str) -> bool {
    msg.starts_with(&game_off_prefix()) && with_state(|ts| std::mem::take(&mut ts.auto_list_pending))
}

/// Completed trades in hand that the player's backpack has not settled
/// (2026-10-02). Normally none: route_trade_frame settles each one as it
/// arrives. A save load is what makes one: it puts back the backpack and
/// the settled set of a save written before the trade settled.
fn unsettled(gs: &GuiState, world: &hecs::World) -> Vec<String> {
    let mut done = gs.trades.iter().filter(|t| t.status == "completed").peekable();
    if done.peek().is_none() {
        return Vec::new();
    }
    let Some(player) = player_entity(world) else {
        return Vec::new();
    };
    let ts = world.get::<&TradeSettlements>(player).ok();
    done.filter(|t| ts.as_ref().map_or(true, |ts| !ts.knows(&t.id))).map(|t| t.id.clone()).collect()
}

/// The trade work that must run with the Trade page closed, once a frame
/// from the relay pump (`engine::frame_ws_poll`): fetch the trade list on
/// every connect, withdraw confirmations the backpack no longer covers, and
/// settle into the loaded backpack any completed trade a save load undid.
pub(crate) fn tick(gs: &mut GuiState, world: &mut hecs::World, known: impl Fn(&str) -> bool) {
    // ws_identified, not just connected: the relay answers a trade list only
    // on a socket bound to my key (after the identify challenge).
    let identified = gs.ws_identified && gs.ws_client.as_ref().is_some_and(|c| c.is_connected());
    if auto_list_due(&mut gs.trades_synced, identified) {
        if let Some(ws) = &gs.ws_client {
            ws.send(&serde_json::json!({"type": "trade_list_request"}).to_string());
        }
    }
    if identified {
        guard_confirmed_offers(gs);
    }
    // Not gated on the link: the records in hand are the relay's own, and a
    // save load can happen offline too.
    let ids = unsettled(gs, world);
    if !ids.is_empty() {
        settle_and_report(gs, world, &ids, &known, true);
    }
}

/// The backpack moves a completed trade makes for me: what I offered goes out,
/// what they offered comes in, with the offered wear and grade. A line with no
/// item id, or naming an item this game does not know (`known` says no), cannot
/// move and is counted instead.
fn trade_transfers(t: &GuiTrade, my_key: &str, known: impl Fn(&str) -> bool) -> (Vec<TransferOp>, usize) {
    let (give, get) = if t.initiator_key == my_key {
        (&t.initiator_items, &t.recipient_items)
    } else if t.recipient_key == my_key {
        (&t.recipient_items, &t.initiator_items)
    } else {
        return (Vec::new(), 0);
    };
    let mut ops = Vec::new();
    let mut unmovable = 0;
    for (items, add) in [(give, false), (get, true)] {
        for i in items {
            match i.reference_id.as_deref() {
                Some(id) if i.quantity > 0 && (!add || known(id)) => ops.push(TransferOp {
                    item_id: id.to_string(),
                    qty: i.quantity,
                    add,
                    wear: i.wear,
                    quality: i.quality,
                }),
                _ => unmovable += 1,
            }
        }
    }
    (ops, unmovable)
}

/// The player a trade settles into: the first entity with a backpack and
/// player control, the one the InventorySystem applies transfers to.
fn player_entity(world: &hecs::World) -> Option<hecs::Entity> {
    world
        .query::<(&Inventory, &crate::ecs::components::Controllable)>()
        .iter()
        .next()
        .map(|(e, _)| e)
}

/// What settling one trade moved (2026-10-02): the counts behind the line the
/// player is shown, kept as numbers so several trades can share one line.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Moved {
    /// Items that leave my backpack.
    pub(crate) gave: u32,
    /// Items that arrive in it.
    pub(crate) got: u32,
    /// Items I offered that had already left the backpack (not taken).
    pub(crate) short: u32,
    /// Lines that cannot move: typed as text, or an item this game lacks.
    pub(crate) unmovable: usize,
}

impl Moved {
    /// Nothing moved and no missing item to own up to: an offer of text
    /// lines only, or nothing on either side.
    fn is_empty(&self) -> bool {
        self.gave == 0 && self.got == 0 && self.short == 0
    }

    fn plus(self, o: Moved) -> Moved {
        Moved {
            gave: self.gave + o.gave,
            got: self.got + o.got,
            short: self.short + o.short,
            unmovable: self.unmovable + o.unmovable,
        }
    }

    /// The line for the player, after `head` ("Trade completed").
    fn line(&self, head: &str) -> String {
        let count = |n: u32| if n == 1 { "1 item".to_string() } else { format!("{n} items") };
        let mut msg = format!("{head}: {} left your backpack and {} arrived.", count(self.gave), count(self.got));
        if self.short > 0 {
            msg += &format!(
                " {} you offered had already left your backpack; the other player still receives {}, because the server cannot hold traded items yet.",
                count(self.short),
                if self.short == 1 { "it" } else { "them" }
            );
        }
        if self.unmovable > 0 {
            msg += &format!(
                " {} line(s) could not move (typed as text, or an item this game does not know).",
                self.unmovable
            );
        }
        msg
    }
}

/// Settle a completed trade in this game, once per trade (2026-09-29): queue the
/// backpack moves on the player's `TradeSettlements` (the InventorySystem
/// applies them and marks the trade settled on its next tick; what does not fit
/// goes to Home storage and says so) and return what moves. None when there is
/// nothing to do yet: already settled or queued, the record is not here or not
/// completed, or I am not a party to it (an unset key must not mark a trade
/// settled with nothing moved). Each of those settles later, when the completed
/// record arrives again (every connect asks for the list) or `tick` finds it.
///
/// HONEST REPORT (2026-10-02): what I offered is checked against the backpack
/// as it is NOW, less what is already queued to leave it, and only that is
/// taken. The relay holds no inventories, so it cannot hold offered items in
/// escrow, and the other player receives the full offer in their own game
/// either way; the line says so instead of claiming items left that were not
/// there. The confirm guard (`guard_confirmed_offers`) makes this rare.
///
/// "NOW" is the player's own `Inventory` (2026-10-02, round 2), not the
/// GUI's copy of it: that copy is refreshed once a frame, so right after a
/// save load put back an older backpack it still showed the one before.
pub(crate) fn settle_completed(
    gs: &GuiState,
    world: &mut hecs::World,
    trade_id: &str,
    known: impl Fn(&str) -> bool,
) -> Option<Moved> {
    let t = gs.trades.iter().find(|t| t.id == trade_id && t.status == "completed")?;
    my_side(t, &gs.profile_public_key)?;
    let player = player_entity(world)?;
    if world.get::<&TradeSettlements>(player).is_err() {
        world.insert_one(player, TradeSettlements::default()).ok()?;
    }
    let mut ts = world.get::<&mut TradeSettlements>(player).ok()?;
    if ts.knows(trade_id) {
        return None;
    }
    let inv = world.get::<&Inventory>(player).ok()?;
    let (mut ops, unmovable) = trade_transfers(t, &gs.profile_public_key, known);
    // Removals already on their way out of the backpack (an earlier trade, a
    // move to storage this frame) are not there to give twice.
    let leaving = |id: &str| -> u32 {
        let queued = ts.pending.iter().flat_map(|(_, m)| m.iter());
        queued
            .chain(gs.pending_inventory_transfers.iter())
            .filter(|o| !o.add && o.item_id == id)
            .map(|o| o.qty)
            .sum()
    };
    let mut short = 0;
    for i in 0..ops.len() {
        if ops[i].add {
            continue;
        }
        let claimed: u32 = ops[..i].iter().filter(|o| !o.add && o.item_id == ops[i].item_id).map(|o| o.qty).sum();
        let have = inv.count_item(&ops[i].item_id).saturating_sub(leaving(&ops[i].item_id) + claimed);
        let take = ops[i].qty.min(have);
        short += ops[i].qty - take;
        ops[i].qty = take;
    }
    ops.retain(|o| o.qty > 0);
    let moved = Moved {
        gave: ops.iter().filter(|o| !o.add).map(|o| o.qty).sum(),
        got: ops.iter().filter(|o| o.add).map(|o| o.qty).sum(),
        short,
        unmovable,
    };
    ts.pending.push((trade_id.to_string(), ops));
    Some(moved)
}

/// Settle these trades if they are ready and tell the player, on the Trade
/// page and as a notice, since the page is usually closed when a trade
/// completes.
///
/// ONE NOTICE PER BATCH (2026-10-02, round 2). `catch_up` marks trades that
/// came in bulk (the list fetched on connect, or `tick` after a save load):
/// they share one line, and a trade that moved nothing is left out of it. One
/// notice per trade pushed everything else off the toast stack, which keeps
/// only a few. A trade settled AGAIN after a save load (the load put back a
/// backpack from before it) was reported the first time, so it settles
/// quietly; otherwise Play would announce every trade a second time.
fn settle_and_report(
    gs: &mut GuiState,
    world: &mut hecs::World,
    ids: &[String],
    known: &dyn Fn(&str) -> bool,
    catch_up: bool,
) {
    let (mut sum, mut trades) = (Moved::default(), 0);
    for id in ids {
        let Some(m) = settle_completed(gs, world, id, known) else { continue };
        if !with_state(|ts| ts.reported.insert(id.clone())) {
            log::info!("Trade {id} settled again: a save load put back a backpack from before it");
            continue;
        }
        if catch_up && m.is_empty() {
            continue;
        }
        sum = sum.plus(m);
        trades += 1;
    }
    if trades == 0 {
        return;
    }
    let head = if trades == 1 { "Trade completed".to_string() } else { format!("{trades} trades completed") };
    let msg = sum.line(&head);
    gs.trade_status = msg.clone();
    gs.pending_notices.push(msg);
}

/// Route one relay frame that carries a trade wrapper; true when it was one
/// (handled: the pump must keep it out of chat). Both the `system` and the
/// `private` arm of the pump call this (`engine::frame_ws_poll`).
///
/// WHY `system` (2026-10-02): the relay converts every targeted Private
/// message into a `system` frame before sending it (the broadcast loop in
/// relay.rs), so `{"type":"system","message":"__trade_data__:..."}` is what
/// the client receives. These branches used to sit only under `private`, which
/// never arrives: the desktop Trade page never listed a trade, no completed
/// trade moved an item, and the raw wrappers were printed into chat.
///
/// A completed trade settles whichever wrapper brings its record: the trade's
/// own update, any element of the list (a trade that completed while this game
/// was closed or offline; the completion notice is sent only once), or the
/// completion notice. Settling is idempotent, so all three may arrive.
pub(crate) fn route_trade_frame(
    gs: &mut GuiState,
    world: &mut hecs::World,
    known: impl Fn(&str) -> bool,
    frame: &serde_json::Value,
) -> bool {
    if !matches!(frame.get("type").and_then(|t| t.as_str()), Some("system" | "private")) {
        return false;
    }
    let Some(msg) = frame.get("message").and_then(|m| m.as_str()) else {
        return false;
    };
    if refuses_auto_list(msg) {
        log::info!("Trades: this server does not host the game, so it keeps no trades");
        return true;
    }
    let parse = |p: &str| serde_json::from_str::<serde_json::Value>(p).ok();
    if let Some(payload) = msg.strip_prefix("__trade_data__:") {
        if let Some(t) = parse(payload).as_ref().and_then(|v| v.get("trade")) {
            let gt = GuiTrade::from_relay_json(t);
            let id = gt.id.clone();
            match gs.trades.iter_mut().find(|x| x.id == gt.id) {
                Some(slot) => *slot = gt,
                None => gs.trades.insert(0, gt),
            }
            settle_and_report(gs, world, &[id], &known, false);
        }
        return true;
    }
    if let Some(payload) = msg.strip_prefix("__trade_list__:") {
        with_state(|ts| ts.auto_list_pending = false);
        if let Some(arr) = parse(payload).as_ref().and_then(|v| v.get("trades")).and_then(|x| x.as_array()) {
            gs.trades = arr.iter().map(GuiTrade::from_relay_json).collect();
            let done: Vec<String> = gs.trades.iter().filter(|t| t.status == "completed").map(|t| t.id.clone()).collect();
            settle_and_report(gs, world, &done, &known, true);
        }
        return true;
    }
    if let Some(payload) = msg.strip_prefix("__trade_complete__:") {
        let v = parse(payload);
        if let Some(tid) = v.as_ref().and_then(|v| v.get("trade_id")).and_then(|x| x.as_str()) {
            match gs.trades.iter_mut().find(|x| x.id == tid) {
                Some(t) => {
                    t.status = "completed".to_string();
                    settle_and_report(gs, world, &[tid.to_string()], &known, false);
                }
                // Its record is not here: fetch the list, and the completed
                // record settles when it lands (it used to say "nothing moved"
                // and mark the trade settled for good).
                None => {
                    if let Some(ws) = &gs.ws_client {
                        ws.send(&serde_json::json!({"type": "trade_list_request"}).to_string());
                    }
                }
            }
        }
        return true;
    }
    false
}

/// Page-local UI state: selection + form drafts. The live trade data itself
/// lives in GuiState (bridged from the relay), never here.
struct TradePageState {
    /// Selected trade by ID (broadcasts reorder the vector).
    selected: Option<String>,
    show_new: bool,
    new_recipient: String,
    new_message: String,
    /// My-side offer draft rows, seeded from the selected trade's current
    /// items; `draft_for` says which trade.
    draft_items: Vec<OfferLine>,
    draft_for: String,
    new_item_name: String,
    new_item_qty: String,
    /// Trades whose confirmation `guard_confirmed_offers` has withdrawn and
    /// the relay has not yet answered (2026-10-02). Which trades are SETTLED
    /// no longer lives here: it is saved with the backpack (TradeSettlements).
    withdrawn: std::collections::HashSet<String>,
    /// The automatic list request is out and unanswered (`auto_list_due`).
    auto_list_pending: bool,
    /// Trades already announced this run (`settle_and_report`), so one settled
    /// again after a save load is not announced twice.
    reported: std::collections::HashSet<String>,
}

impl Default for TradePageState {
    fn default() -> Self {
        Self {
            selected: None,
            show_new: false,
            new_recipient: String::new(),
            new_message: String::new(),
            draft_items: Vec::new(),
            draft_for: String::new(),
            new_item_name: String::new(),
            new_item_qty: "1".to_string(),
            withdrawn: std::collections::HashSet::new(),
            auto_list_pending: false,
            reported: std::collections::HashSet::new(),
        }
    }
}

fn with_state<R>(f: impl FnOnce(&mut TradePageState) -> R) -> R {
    use std::cell::RefCell;
    thread_local! {
        static STATE: RefCell<TradePageState> = RefCell::new(TradePageState::default());
    }
    STATE.with(|s| f(&mut s.borrow_mut()))
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // The trade list is fetched on every connect by `tick` (2026-10-02),
    // page open or not; the relay-pump bridge keeps it current.
    let connected = state.ws_client.as_ref().map_or(false, |c| c.is_connected());
    let my_key = state.profile_public_key.clone();

    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(16.0))
        .show(ctx, |ui| {
            // Header
            ui.horizontal(|ui| {
                // Heading matches the Profile sidebar label ("Trade").
                ui.label(
                    RichText::new("Trade")
                        .size(theme.font_size_title)
                        .color(theme.text_primary()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    with_state(|ts| {
                        if widgets::primary_button(ui, theme, "New Trade") {
                            ts.show_new = !ts.show_new;
                        }
                    });
                    if connected && widgets::secondary_button(ui, theme, "Refresh") {
                        if let Some(ws) = &state.ws_client {
                            ws.send(&serde_json::json!({"type": "trade_list_request"}).to_string());
                        }
                    }
                });
            });
            if !connected {
                ui.label(
                    RichText::new("Offline - connect to a server (Chat page) to trade.")
                        .color(theme.warning())
                        .size(theme.font_size_small),
                );
            }
            if !state.trade_status.is_empty() {
                ui.label(
                    RichText::new(&state.trade_status)
                        .color(theme.text_secondary())
                        .size(theme.font_size_small),
                );
            }
            ui.separator();

            // New trade form: ask another player (by public key) to trade.
            with_state(|ts| {
                if ts.show_new {
                    widgets::card(ui, theme, |ui| {
                        ui.label(
                            RichText::new("Start New Trade")
                                .size(theme.font_size_body)
                                .color(theme.accent()),
                        );
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Their key:").color(theme.text_secondary()));
                            ui.add(
                                egui::TextEdit::singleline(&mut ts.new_recipient)
                                    .hint_text("paste the other player's public key")
                                    .desired_width(320.0),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Message:").color(theme.text_secondary()));
                            ui.add(
                                egui::TextEdit::singleline(&mut ts.new_message)
                                    .hint_text("optional greeting / what you are after")
                                    .desired_width(320.0),
                            );
                        });
                        ui.horizontal(|ui| {
                            if widgets::primary_button(ui, theme, "Send request")
                                && !ts.new_recipient.trim().is_empty()
                            {
                                if connected {
                                    if let Some(ws) = &state.ws_client {
                                        ws.send(
                                            &serde_json::json!({
                                                "type": "trade_request",
                                                "target_key": ts.new_recipient.trim(),
                                                "message": ts.new_message.trim(),
                                            })
                                            .to_string(),
                                        );
                                    }
                                    state.trade_status = "Trade request sent.".to_string();
                                    ts.new_recipient.clear();
                                    ts.new_message.clear();
                                    ts.show_new = false;
                                } else {
                                    state.trade_status =
                                        "Connect to a server to send trade requests.".to_string();
                                }
                            }
                            if widgets::secondary_button(ui, theme, "Cancel") {
                                ts.show_new = false;
                            }
                        });
                    });
                    ui.add_space(theme.spacing_sm);
                }
            });

            if state.trades.is_empty() {
                ui.add_space(theme.spacing_xl);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("No trades")
                            .size(theme.font_size_heading)
                            .color(theme.text_muted()),
                    );
                    let hint = if connected {
                        "Start one with New Trade - you need the other player's public key."
                    } else {
                        "Connect to a server to see your trades."
                    };
                    ui.label(RichText::new(hint).color(theme.text_secondary()));
                });
                return;
            }

            // List + detail
            ui.columns(2, |cols| {
                // Left: my trades, newest first (relay order).
                cols[0].label(
                    RichText::new("Trades")
                        .size(theme.font_size_body)
                        .color(theme.text_secondary()),
                );
                let rows: Vec<(String, String, String, usize, usize, bool)> = state
                    .trades
                    .iter()
                    .map(|t| {
                        let partner = if t.initiator_key == my_key {
                            &t.recipient_key
                        } else {
                            &t.initiator_key
                        };
                        (
                            t.id.clone(),
                            key_label(partner),
                            t.status.clone(),
                            t.initiator_items.len(),
                            t.recipient_items.len(),
                            t.initiator_key == my_key,
                        )
                    })
                    .collect();
                ScrollArea::vertical().id_salt("trade_list").show(&mut cols[0], |ui| {
                    with_state(|ts| {
                        for (id, partner, status, init_n, recv_n, i_started) in &rows {
                            let selected = ts.selected.as_deref() == Some(id.as_str());
                            let fill = if selected { theme.bg_card() } else { Color32::TRANSPARENT };
                            egui::Frame::none()
                                .fill(fill)
                                .rounding(Rounding::same(theme.border_radius as u8))
                                .inner_margin(8.0)
                                .show(ui, |ui| {
                                    let resp = ui.horizontal(|ui| {
                                        ui.label(RichText::new(partner).color(theme.text_primary()));
                                        egui::Frame::none()
                                            .fill(status_color(theme, status))
                                            .rounding(Rounding::same(3))
                                            .inner_margin(Vec2::new(6.0, 2.0))
                                            .show(ui, |ui| {
                                                ui.label(
                                                    RichText::new(status.as_str())
                                                        .size(theme.font_size_small)
                                                        .color(Color32::WHITE),
                                                );
                                            });
                                        let (mine, theirs) = if *i_started {
                                            (init_n, recv_n)
                                        } else {
                                            (recv_n, init_n)
                                        };
                                        ui.label(
                                            RichText::new(format!("{mine} for {theirs} items"))
                                                .color(theme.text_muted())
                                                .size(theme.font_size_small),
                                        );
                                    });
                                    if resp.response.interact(egui::Sense::click()).clicked() {
                                        ts.selected = Some(id.clone());
                                    }
                                });
                        }
                    });
                });

                // Right: selected trade detail + actions.
                let sel_id = with_state(|ts| ts.selected.clone()).unwrap_or_default();
                let trade: Option<GuiTrade> =
                    state.trades.iter().find(|t| t.id == sel_id).cloned();
                if let Some(t) = trade {
                    let i_am_initiator = t.initiator_key == my_key;
                    let (my_items, their_items) = if i_am_initiator {
                        (&t.initiator_items, &t.recipient_items)
                    } else {
                        (&t.recipient_items, &t.initiator_items)
                    };
                    let (my_confirmed, their_confirmed) = if i_am_initiator {
                        (t.initiator_confirmed, t.recipient_confirmed)
                    } else {
                        (t.recipient_confirmed, t.initiator_confirmed)
                    };
                    let partner = if i_am_initiator { &t.recipient_key } else { &t.initiator_key };

                    // Seed / reseed the offer draft when the selection changes.
                    with_state(|ts| {
                        if ts.draft_for != t.id {
                            ts.draft_for = t.id.clone();
                            ts.draft_items = my_items.iter().map(OfferLine::from_item).collect();
                            ts.new_item_name.clear();
                            ts.new_item_qty = "1".to_string();
                        }
                    });

                    let ui = &mut cols[1];
                    ui.label(
                        RichText::new(format!("Trade with {}", key_label(partner)))
                            .size(theme.font_size_body)
                            .color(theme.accent()),
                    );
                    if !t.message.is_empty() {
                        ui.label(
                            RichText::new(format!("\"{}\"", t.message))
                                .color(theme.text_muted())
                                .size(theme.font_size_small),
                        );
                    }
                    ui.add_space(theme.spacing_sm);

                    // Their offer (read-only).
                    ui.label(RichText::new("Their offer:").color(theme.text_secondary()));
                    if their_items.is_empty() {
                        ui.label(RichText::new("  (nothing yet)").color(theme.text_muted()));
                    }
                    for item in their_items {
                        ui.label(
                            RichText::new(format!("  {} x{}", item.name, item.quantity))
                                .color(theme.text_primary()),
                        );
                    }
                    ui.add_space(theme.spacing_sm);

                    // My offer: editable while the trade is active.
                    ui.label(RichText::new("Your offer:").color(theme.text_secondary()));
                    if t.status == "active" {
                        with_state(|ts| {
                            let mut remove: Option<usize> = None;
                            for (i, line) in ts.draft_items.iter_mut().enumerate() {
                                ui.horizontal(|ui| {
                                    // A text line (from before offers named items)
                                    // shows in the warning colour: it cannot move.
                                    let color = if line.item_id.is_empty() { theme.warning() } else { theme.text_primary() };
                                    ui.add_sized([160.0, 18.0], egui::Label::new(RichText::new(&line.name).color(color)));
                                    ui.add(egui::TextEdit::singleline(&mut line.qty).desired_width(40.0));
                                    if widgets::secondary_button(ui, theme, "x") {
                                        remove = Some(i);
                                    }
                                });
                            }
                            if let Some(i) = remove {
                                ts.draft_items.remove(i);
                            }
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut ts.new_item_name)
                                        .hint_text("an item you carry")
                                        .desired_width(160.0),
                                );
                                ui.add(
                                    egui::TextEdit::singleline(&mut ts.new_item_qty)
                                        .desired_width(40.0),
                                );
                                if widgets::secondary_button(ui, theme, "Add")
                                    && !ts.new_item_name.trim().is_empty()
                                {
                                    match resolve_carried(&state.inventory_items, &ts.new_item_name) {
                                        Some(slot) => {
                                            ts.draft_items.push(OfferLine {
                                                item_id: slot.item_id,
                                                name: slot.name,
                                                qty: ts.new_item_qty.trim().to_string(),
                                                wear: slot.wear,
                                                quality: slot.quality,
                                            });
                                            ts.new_item_name.clear();
                                            ts.new_item_qty = "1".to_string();
                                        }
                                        None => {
                                            state.trade_status = format!(
                                                "You aren't carrying anything called \"{}\".",
                                                ts.new_item_name.trim()
                                            );
                                        }
                                    }
                                }
                            });
                            if widgets::secondary_button(ui, theme, "Update offer") {
                                let items: Vec<serde_json::Value> = ts
                                    .draft_items
                                    .iter()
                                    .filter(|l| !l.name.trim().is_empty())
                                    .map(OfferLine::to_json)
                                    .collect();
                                if let Some(ws) = &state.ws_client {
                                    ws.send(
                                        &serde_json::json!({
                                            "type": "trade_update_items",
                                            "trade_id": t.id,
                                            "items": items,
                                        })
                                        .to_string(),
                                    );
                                }
                                state.trade_status = "Offer updated.".to_string();
                            }
                        });
                    } else {
                        if my_items.is_empty() {
                            ui.label(RichText::new("  (nothing yet)").color(theme.text_muted()));
                        }
                        for item in my_items {
                            ui.label(
                                RichText::new(format!("  {} x{}", item.name, item.quantity))
                                    .color(theme.text_primary()),
                            );
                        }
                    }
                    ui.add_space(theme.spacing_sm);

                    // Confirmations + actions per status.
                    match t.status.as_str() {
                        "pending" => {
                            if i_am_initiator {
                                ui.label(
                                    RichText::new("Waiting for them to accept.")
                                        .color(theme.text_muted())
                                        .size(theme.font_size_small),
                                );
                                if widgets::secondary_button(ui, theme, "Cancel request") {
                                    if let Some(ws) = &state.ws_client {
                                        ws.send(&serde_json::json!({"type": "trade_cancel", "trade_id": t.id}).to_string());
                                    }
                                }
                            } else {
                                ui.horizontal(|ui| {
                                    if widgets::primary_button(ui, theme, "Accept") {
                                        if let Some(ws) = &state.ws_client {
                                            ws.send(&serde_json::json!({"type": "trade_response", "trade_id": t.id, "accepted": true}).to_string());
                                        }
                                    }
                                    if widgets::secondary_button(ui, theme, "Reject") {
                                        if let Some(ws) = &state.ws_client {
                                            ws.send(&serde_json::json!({"type": "trade_response", "trade_id": t.id, "accepted": false}).to_string());
                                        }
                                    }
                                });
                            }
                        }
                        "active" => {
                            let conf = format!(
                                "You: {}   Them: {}",
                                if my_confirmed { "confirmed ✓" } else { "not confirmed" },
                                if their_confirmed { "confirmed ✓" } else { "not confirmed" },
                            );
                            ui.label(
                                RichText::new(conf)
                                    .color(theme.text_secondary())
                                    .size(theme.font_size_small),
                            );
                            let shortfall = offer_shortfall(&state.inventory_items, my_items);
                            ui.horizontal(|ui| {
                                if !my_confirmed
                                    && shortfall.is_none()
                                    && widgets::primary_button(ui, theme, "Confirm trade")
                                {
                                    if let Some(ws) = &state.ws_client {
                                        ws.send(&serde_json::json!({"type": "trade_confirm", "trade_id": t.id}).to_string());
                                    }
                                }
                                if widgets::secondary_button(ui, theme, "Cancel trade") {
                                    if let Some(ws) = &state.ws_client {
                                        ws.send(&serde_json::json!({"type": "trade_cancel", "trade_id": t.id}).to_string());
                                    }
                                }
                            });
                            if let Some(why) = &shortfall {
                                ui.label(RichText::new(why).color(theme.warning()).size(theme.font_size_small));
                            }
                            if my_confirmed && !their_confirmed {
                                ui.label(
                                    RichText::new("Confirmed - waiting on them. Changing items resets confirmations.")
                                        .color(theme.text_muted())
                                        .size(theme.font_size_small),
                                );
                            }
                        }
                        other => {
                            ui.label(
                                RichText::new(format!("This trade is {other}."))
                                    .color(theme.text_muted())
                                    .size(theme.font_size_small),
                            );
                        }
                    }
                } else {
                    cols[1].label(
                        RichText::new("Select a trade to view it.")
                            .color(theme.text_muted()),
                    );
                }
            });
        });
}

/// TRADES MOVE ITEMS (2026-09-29). Red checks, run: settling twice moves the
/// items twice without the settled guard; an offer the backpack cannot cover
/// confirms without `offer_shortfall`. The 2026-10-02 tests below (wrappers
/// arriving as system frames, settling from the list, the confirm guard, the
/// honest settlement) each say how they were seen red.
#[cfg(all(test, feature = "native"))]
mod trade_moves_tests {
    use super::*;
    use crate::relay::relay::{RelayMessage, TradeDataPayload, TradeItem};

    fn slot(id: &str, name: &str, qty: u32) -> Option<GuiItemSlot> {
        Some(GuiItemSlot { item_id: id.into(), name: name.into(), quantity: qty, ..Default::default() })
    }
    fn offered(id: Option<&str>, name: &str, qty: u32) -> GuiTradeItem {
        GuiTradeItem { name: name.into(), quantity: qty, reference_id: id.map(str::to_string), ..Default::default() }
    }
    fn op(id: &str, qty: u32, add: bool) -> TransferOp {
        TransferOp { item_id: id.into(), qty, add, ..Default::default() }
    }
    /// A world holding the player (a backpack under player control).
    fn world_with_player() -> (hecs::World, hecs::Entity) {
        let mut world = hecs::World::new();
        let p = world.spawn((Inventory::new(16), crate::ecs::components::Controllable));
        (world, p)
    }
    /// Put `qty` of `id` in the player's own backpack, the one settling
    /// reads (2026-10-02, round 2; it used to read the GUI's copy).
    fn carry(world: &mut hecs::World, p: hecs::Entity, id: &str, qty: u32) {
        world.get::<&mut Inventory>(p).unwrap().add_item(id, qty, 99);
    }
    /// What is queued to settle on the player, trade by trade.
    fn queued(world: &hecs::World, p: hecs::Entity) -> Vec<(String, Vec<TransferOp>)> {
        world.get::<&TradeSettlements>(p).map(|t| t.pending.clone()).unwrap_or_default()
    }
    fn relay_item(id: Option<&str>, name: &str, qty: u32) -> TradeItem {
        TradeItem {
            item_type: "item".into(),
            name: name.into(),
            quantity: qty,
            description: String::new(),
            reference_id: id.map(str::to_string),
            wear: id.map(|_| 0),
            quality: id.map(|_| 0),
        }
    }
    fn payload(id: &str, status: &str, init: Vec<TradeItem>, recv: Vec<TradeItem>) -> TradeDataPayload {
        TradeDataPayload {
            id: id.into(),
            initiator_key: "alice".into(),
            recipient_key: "bob".into(),
            status: status.into(),
            initiator_items: init,
            recipient_items: recv,
            initiator_confirmed: status != "pending",
            recipient_confirmed: status != "pending",
            created_at: 1_727_000_000_000,
            completed_at: None,
            message: None,
        }
    }
    /// The frame a client receives for a targeted relay message: the relay's
    /// broadcast loop turns every RelayMessage::Private into this System
    /// frame before sending it (relay.rs).
    fn delivered(message: String) -> serde_json::Value {
        let wire = serde_json::to_string(&RelayMessage::System { message }).unwrap();
        serde_json::from_str(&wire).unwrap()
    }
    /// The relay's wrappers, built the way msg_handlers.rs builds them.
    fn trade_data(p: TradeDataPayload) -> String {
        format!("__trade_data__:{}", serde_json::to_string(&RelayMessage::TradeData { trade: p }).unwrap())
    }
    fn trade_list(ps: Vec<TradeDataPayload>) -> String {
        format!("__trade_list__:{}", serde_json::json!({"type": "trade_list", "trades": ps}))
    }
    fn trade_complete(id: &str) -> String {
        format!("__trade_complete__:{{\"trade_id\":\"{}\"}}", id)
    }

    #[test]
    fn an_offer_names_a_carried_item_and_must_be_covered() {
        let inv = vec![slot("rope_0", "Rope", 2), None, slot("rope_0", "Rope", 1)];
        // By name or id, any case; something not carried is refused.
        assert_eq!(resolve_carried(&inv, " rope ").map(|s| s.item_id), Some("rope_0".into()));
        assert_eq!(resolve_carried(&inv, "ROPE_0").map(|s| s.name), Some("Rope".into()));
        assert!(resolve_carried(&inv, "Hammer").is_none());
        // Three ropes across two stacks cover two lines of 1 and 2, not 4.
        assert_eq!(offer_shortfall(&inv, &[offered(Some("rope_0"), "Rope", 1), offered(Some("rope_0"), "Rope", 2)]), None);
        let short = offer_shortfall(&inv, &[offered(Some("rope_0"), "Rope", 4)]).expect("4 ropes are not carried");
        assert!(short.contains("carry 3"), "{short}");
        // A text line cannot be confirmed.
        assert!(offer_shortfall(&inv, &[offered(None, "Help building", 1)]).is_some());
        // The line the relay stores keeps the id, wear and grade.
        let line = OfferLine { item_id: "hammer_0".into(), name: "Hammer".into(), qty: "1".into(), wear: 150, quality: 2 };
        let back = GuiTrade::from_relay_json(&serde_json::json!({"initiator_items": [line.to_json()]}));
        assert_eq!(OfferLine::from_item(&back.initiator_items[0]), line);
    }

    #[test]
    fn a_completed_trade_moves_each_side_once() {
        let (mut world, p) = world_with_player();
        let mut gs = GuiState::default();
        gs.profile_public_key = "bob".into();
        carry(&mut world, p, "wheat_0", 5);
        gs.trades.push(GuiTrade {
            id: "t-1".into(),
            initiator_key: "alice".into(),
            recipient_key: "bob".into(),
            status: "completed".into(),
            initiator_items: vec![
                GuiTradeItem { wear: 150, ..offered(Some("hammer_0"), "Hammer", 1) },
                offered(None, "A hug", 1),
                offered(Some("modded_thing"), "Modded", 1),
            ],
            recipient_items: vec![offered(Some("wheat_0"), "Wheat", 5)],
            ..Default::default()
        });
        let known = |id: &str| id != "modded_thing";
        let msg = settle_completed(&gs, &mut world, "t-1", known).expect("a completed trade settles").line("Trade completed");
        let q = queued(&world, p);
        let ops = &q[0].1;
        assert!(ops.contains(&op("wheat_0", 5, false)), "Bob gives his wheat: {ops:?}");
        assert!(ops.contains(&TransferOp { wear: 150, ..op("hammer_0", 1, true) }), "the hammer arrives worn: {ops:?}");
        assert_eq!(ops.len(), 2, "the hug and the unknown item do not move: {ops:?}");
        assert!(msg.contains("5 items left") && msg.contains("1 item arrived") && msg.contains("2 line(s)"), "{msg}");
        // A second notice for the same trade moves nothing more.
        assert!(settle_completed(&gs, &mut world, "t-1", known).is_none());
        assert_eq!(queued(&world, p).len(), 1);
        // Someone outside the trade moves nothing.
        assert_eq!(trade_transfers(&gs.trades[0], "carol", known).0.len(), 0);
    }

    /// FINDING 1 (2026-10-02): the relay converts every targeted Private into
    /// a `system` frame before sending it, so the trade wrappers reach the
    /// client as {"type":"system","message":"__trade_data__:..."}. They were
    /// handled only under `private`, which never arrives: the desktop never
    /// listed a trade or moved a traded item. Seen red, 2026-10-02: with
    /// `route_trade_frame` accepting only "private" (the old branches' one
    /// home), the first assert on the system frame failed (not handled).
    #[test]
    fn trade_wrappers_arrive_as_system_frames_and_settle() {
        let (mut world, p) = world_with_player();
        let mut gs = GuiState::default();
        gs.profile_public_key = "bob".into();
        carry(&mut world, p, "wheat_0", 5);
        let known = |_: &str| true;
        let hammer = || vec![relay_item(Some("hammer_0"), "Hammer", 1)];
        let wheat = || vec![relay_item(Some("wheat_0"), "Wheat", 5)];

        let frame = delivered(trade_data(payload("t-7", "active", hammer(), wheat())));
        assert_eq!(frame["type"], "system", "what the client actually receives");
        assert!(route_trade_frame(&mut gs, &mut world, known, &frame), "a trade wrapper is taken, never shown in chat");
        assert_eq!((gs.trades.len(), gs.trades[0].status.as_str()), (1, "active"));
        assert!(queued(&world, p).is_empty(), "an active trade moves nothing yet");

        // The relay's completion notice settles it.
        assert!(route_trade_frame(&mut gs, &mut world, known, &delivered(trade_complete("t-7"))));
        assert_eq!(gs.trades[0].status, "completed");
        let q = queued(&world, p);
        assert_eq!(q.len(), 1, "{q:?}");
        assert_eq!(q[0].0, "t-7");
        assert_eq!(q[0].1, vec![op("wheat_0", 5, false), op("hammer_0", 1, true)]);
        assert!(gs.trade_status.starts_with("Trade completed: 5 items left"), "{}", gs.trade_status);
        assert_eq!(gs.pending_notices.len(), 1, "shown with the Trade page closed");

        // The completed record the relay also sends moves nothing twice.
        let again = delivered(trade_data(payload("t-7", "completed", hammer(), wheat())));
        assert!(route_trade_frame(&mut gs, &mut world, known, &again));
        assert_eq!(queued(&world, p).len(), 1);
        assert_eq!(gs.pending_notices.len(), 1);

        // Other relay text is not a trade and goes on to chat.
        assert!(!route_trade_frame(&mut gs, &mut world, known, &delivered("Trade target is not online.".into())));
    }

    /// FINDING 3 (2026-10-02): the completion notice is sent once, so a player
    /// offline when a trade completed only ever sees it in the trade list.
    /// Seen red: with the settle loop taken out of the `__trade_list__`
    /// branch, nothing was queued for t-8.
    #[test]
    fn a_trade_completed_while_away_settles_from_the_list_once() {
        let (mut world, p) = world_with_player();
        let mut gs = GuiState::default();
        gs.profile_public_key = "bob".into();
        carry(&mut world, p, "wheat_0", 9);
        let known = |_: &str| true;
        let list = || {
            delivered(trade_list(vec![
                payload("t-8", "completed", vec![relay_item(Some("hammer_0"), "Hammer", 1)], vec![relay_item(Some("wheat_0"), "Wheat", 2)]),
                payload("t-9", "active", vec![], vec![relay_item(Some("wheat_0"), "Wheat", 1)]),
            ]))
        };
        assert!(route_trade_frame(&mut gs, &mut world, known, &list()));
        assert_eq!(gs.trades.len(), 2);
        let q = queued(&world, p);
        assert_eq!(q.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), vec!["t-8"], "only the completed one settles");
        // The list again (a reconnect) before the moves applied: still once.
        assert!(route_trade_frame(&mut gs, &mut world, known, &list()));
        assert_eq!(queued(&world, p).len(), 1);
        // And after they applied (the InventorySystem marks it settled).
        {
            let mut ts = world.get::<&mut TradeSettlements>(p).unwrap();
            ts.pending.clear();
            ts.settled.insert("t-8".into());
        }
        assert!(route_trade_frame(&mut gs, &mut world, known, &list()));
        assert!(queued(&world, p).is_empty());
        // A completion for a trade whose record is not here moves nothing
        // and is not marked settled: the list fetched for it settles it.
        assert!(route_trade_frame(&mut gs, &mut world, known, &delivered(trade_complete("t-10"))));
        assert!(!world.get::<&TradeSettlements>(p).unwrap().knows("t-10"));
    }

    /// FINDING 2a (2026-10-02): a confirmation is withdrawn when the offered
    /// items leave the backpack afterwards. Seen red: with `withdrawals`
    /// returning nothing (the old behaviour, the check ran only on the Confirm
    /// click), the 2-rope offer against 1 carried rope stood.
    #[test]
    fn a_confirmation_the_backpack_no_longer_covers_is_withdrawn() {
        let mut gs = GuiState::default();
        gs.profile_public_key = "alice".into();
        let rope = GuiTradeItem { wear: 3, quality: 1, ..offered(Some("rope_0"), "Rope", 2) };
        gs.trades.push(GuiTrade {
            id: "t-1".into(),
            initiator_key: "alice".into(),
            recipient_key: "bob".into(),
            status: "active".into(),
            initiator_items: vec![rope, offered(None, "A hug", 1)],
            initiator_confirmed: true,
            recipient_confirmed: true,
            ..Default::default()
        });
        gs.inventory_items = vec![slot("rope_0", "Rope", 2)];
        assert!(withdrawals(&gs).is_empty(), "both ropes still carried");

        gs.inventory_items = vec![slot("rope_0", "Rope", 1)];
        let w = withdrawals(&gs);
        assert_eq!(w.len(), 1);
        let (id, msg, line) = &w[0];
        assert_eq!(id, "t-1");
        assert!(line.contains("carry 1") && line.contains("withdrawn"), "{line}");
        // The same offer, re-sent: the relay clears both confirmations on it.
        let v: serde_json::Value = serde_json::from_str(msg).unwrap();
        assert_eq!((v["type"].as_str(), v["trade_id"].as_str()), (Some("trade_update_items"), Some("t-1")));
        let back = GuiTrade::from_relay_json(&serde_json::json!({"initiator_items": v["items"]}));
        let lines: Vec<_> =
            back.initiator_items.iter().map(|i| (i.name.as_str(), i.quantity, i.reference_id.as_deref(), i.wear, i.quality)).collect();
        assert_eq!(lines, vec![("Rope", 2, Some("rope_0"), 3, 1), ("A hug", 1, None, 0, 0)]);

        // Sent once per confirmation, not every frame, and said as a notice
        // too, since the Trade page (the only place trade_status shows) is
        // usually closed. Seen red, 2026-10-02 round 2: with the
        // pending_notices push removed, the notice assert failed.
        guard_confirmed_offers(&mut gs);
        assert!(gs.trade_status.contains("withdrawn"));
        assert_eq!(gs.pending_notices.len(), 1);
        assert!(gs.pending_notices[0].contains("withdrawn"), "{:?}", gs.pending_notices);
        gs.trade_status.clear();
        guard_confirmed_offers(&mut gs);
        assert!(gs.trade_status.is_empty(), "still waiting on the relay's answer");
        // Answered (unconfirmed), then confirmed again while still short.
        gs.trades[0].initiator_confirmed = false;
        guard_confirmed_offers(&mut gs);
        gs.trades[0].initiator_confirmed = true;
        guard_confirmed_offers(&mut gs);
        assert!(gs.trade_status.contains("withdrawn"));

        // Not confirmed, or a text-only offer (confirmed on the web page):
        // nothing in this backpack backs it, so nothing to withdraw.
        gs.trades[0].initiator_confirmed = false;
        assert!(withdrawals(&gs).is_empty());
        gs.trades[0].initiator_confirmed = true;
        gs.trades[0].initiator_items = vec![offered(None, "A hug", 1)];
        assert!(withdrawals(&gs).is_empty());
    }

    /// FINDING 2b (2026-10-02): settling takes only what the backpack holds
    /// now, less what is already leaving it, and says what it could not take.
    /// Seen red: without the clamp, the queue took 3 ropes from a backpack with
    /// 1 to spare and the line claimed "3 items left your backpack".
    #[test]
    fn settling_takes_only_what_the_backpack_still_holds() {
        let (mut world, p) = world_with_player();
        let mut gs = GuiState::default();
        gs.profile_public_key = "alice".into();
        carry(&mut world, p, "rope_0", 2);
        // One of the two is on its way to storage this frame.
        gs.pending_inventory_transfers.push(op("rope_0", 1, false));
        gs.trades.push(GuiTrade {
            id: "t-2".into(),
            initiator_key: "alice".into(),
            recipient_key: "bob".into(),
            status: "completed".into(),
            initiator_items: vec![offered(Some("rope_0"), "Rope", 3)],
            recipient_items: vec![offered(Some("wheat_0"), "Wheat", 1)],
            ..Default::default()
        });
        let msg = settle_completed(&gs, &mut world, "t-2", |_: &str| true).unwrap().line("Trade completed");
        assert_eq!(queued(&world, p)[0].1, vec![op("rope_0", 1, false), op("wheat_0", 1, true)]);
        assert!(msg.contains("1 item left your backpack"), "{msg}");
        assert!(msg.contains("2 items you offered had already left") && msg.contains("still receives them"), "{msg}");
    }

    /// FINDING 3 (2026-10-02): the list is asked for on every identified
    /// connection with the Trade page closed, and again after a server switch.
    /// Seen red: with the reset on a link that is not identified removed, a
    /// reconnect never asked again; with the two trade lines in
    /// `reset_per_server_transients` removed, the switch kept the old list.
    #[test]
    fn the_trade_list_is_requested_once_per_connection() {
        let mut synced = false;
        assert!(!list_request_due(&mut synced, false), "not before identify");
        assert!(list_request_due(&mut synced, true));
        assert!(!list_request_due(&mut synced, true), "once per connection");
        assert!(!list_request_due(&mut synced, false), "the link dropped");
        assert!(list_request_due(&mut synced, true), "asked again after the reconnect");

        let mut gs = GuiState::default();
        gs.server_url = "https://a.example".into();
        gs.trades.push(GuiTrade { id: "t-1".into(), ..Default::default() });
        gs.trades_synced = true;
        gs.park_active_connection();
        assert!(gs.trades.is_empty() && !gs.trades_synced, "another server's trades are its own");
    }

    /// The player as the game spawns it: everything a save is read from and
    /// applied to (save_load::extract_world_save / apply_save_to_world).
    fn full_player() -> (hecs::World, hecs::Entity) {
        let mut world = hecs::World::new();
        let p = world.spawn((
            crate::ecs::components::Controllable,
            Inventory::new(16),
            crate::systems::skills::PlayerSkills::new(),
            crate::ecs::components::Name("Bob".to_string()),
            crate::ecs::components::Appearance::default(),
            crate::ecs::components::Outfit::default(),
        ));
        (world, p)
    }
    /// (wheat, hammers) in the player's backpack.
    fn held(world: &hecs::World, p: hecs::Entity) -> (u32, u32) {
        let inv = world.get::<&Inventory>(p).unwrap();
        (inv.count_item("wheat_0"), inv.count_item("hammer_0"))
    }
    /// The relay's list holding one completed trade: Alice's hammer for
    /// Bob's 5 wheat.
    fn hammer_for_wheat() -> serde_json::Value {
        let hammer = vec![relay_item(Some("hammer_0"), "Hammer", 1)];
        delivered(trade_list(vec![payload("t-1", "completed", hammer, vec![relay_item(Some("wheat_0"), "Wheat", 5)])]))
    }
    /// Frames of the main loop, in its order: the systems tick (the
    /// InventorySystem applies queued trade moves), then the relay pump ends
    /// with `tick`.
    fn frames(n: usize, gs: &mut GuiState, world: &mut hecs::World, inv: &mut crate::systems::inventory::InventorySystem) {
        use crate::ecs::systems::System;
        let data = crate::hot_reload::data_store::DataStore::new();
        for _ in 0..n {
            inv.tick(world, 0.016, &data);
            tick(gs, world, |_: &str| true);
        }
    }

    /// BLOCKER (2026-10-02 review): the relay connects at the main menu, so a
    /// trade settles into the home loaded at startup; clicking Play then loads
    /// the save from disk (lib.rs `launcher_pending_load` ->
    /// `save_load::apply_save_to_world`), which puts back the backpack AND the
    /// settled set from before the trade, and nothing settled it again: inside
    /// the 120 s autosave interval the items were lost. This runs that sequence
    /// through the real functions in the main loop's order. Seen red,
    /// 2026-10-02: with the re-settle taken out of `tick` (the old behaviour),
    /// the loaded backpack still held (5 wheat, 0 hammers) after Play.
    #[test]
    fn a_trade_undone_by_a_save_load_settles_once_into_the_loaded_home() {
        let (mut world, p) = full_player();
        carry(&mut world, p, "wheat_0", 5);
        // The save on disk: the last autosave, from before the trade.
        let disk = crate::save_load::extract_world_save(&world);
        let mut inv = crate::systems::inventory::InventorySystem::new();
        let mut gs = GuiState::default();
        gs.profile_public_key = "bob".into();
        let known = |_: &str| true;

        // Main menu: the list fetched on connect settles the trade.
        assert!(route_trade_frame(&mut gs, &mut world, known, &hammer_for_wheat()));
        frames(1, &mut gs, &mut world, &mut inv);
        assert_eq!(held(&world, p), (0, 1));
        // Play: the save from disk is loaded onto the live world.
        crate::save_load::apply_save_to_world(&mut world, &disk);
        assert_eq!(held(&world, p), (5, 0), "the load put back the backpack from before the trade");
        // The frames after Play settle it into the loaded home.
        frames(3, &mut gs, &mut world, &mut inv);
        assert_eq!(held(&world, p), (0, 1), "the trade lands in the home that will be saved");
        {
            let ts = world.get::<&TradeSettlements>(p).unwrap();
            assert!(ts.pending.is_empty() && ts.settled.contains("t-1"), "{:?}", *ts);
        }
        // Exactly once: the list again (a reconnect) and more frames move nothing.
        assert!(route_trade_frame(&mut gs, &mut world, known, &hammer_for_wheat()));
        frames(3, &mut gs, &mut world, &mut inv);
        assert_eq!(held(&world, p), (0, 1));
        // Announced once: settling again after the load is quiet.
        assert_eq!(gs.pending_notices.len(), 1, "{:?}", gs.pending_notices);
        // The next save carries the trade with its items.
        let save = crate::save_load::extract_world_save(&world);
        assert_eq!(save.settled_trades, vec!["t-1".to_string()]);
        assert!(save.inventory.iter().any(|(id, q)| id == "hammer_0" && *q == 1), "{:?}", save.inventory);
    }

    /// A save load DROPS trade moves queued before it (2026-10-02, round 2):
    /// they were sized against the backpack the load replaced, and `tick`
    /// queues them again against the loaded one. Here the completion arrives
    /// and Play loads the save in the same frame, before the systems tick.
    /// Seen red, 2026-10-02: with `ts.pending.clear()` taken out of
    /// `save_load::restore_settled_trades`, the move sized against the 2 wheat
    /// carried before the load took 2 of the loaded 5, so 3 wheat the other
    /// player also receives stayed behind: (3, 1).
    #[test]
    fn moves_queued_before_a_save_load_are_sized_again_against_the_loaded_backpack() {
        let (mut world, p) = full_player();
        carry(&mut world, p, "wheat_0", 5);
        let disk = crate::save_load::extract_world_save(&world);
        // Since that save, 3 of the wheat were eaten.
        world.get::<&mut Inventory>(p).unwrap().remove_item("wheat_0", 3);
        let mut inv = crate::systems::inventory::InventorySystem::new();
        let mut gs = GuiState::default();
        gs.profile_public_key = "bob".into();

        assert!(route_trade_frame(&mut gs, &mut world, |_: &str| true, &hammer_for_wheat()));
        assert_eq!(queued(&world, p)[0].1, vec![op("wheat_0", 2, false), op("hammer_0", 1, true)]);
        crate::save_load::apply_save_to_world(&mut world, &disk);
        frames(3, &mut gs, &mut world, &mut inv);
        assert_eq!(held(&world, p), (0, 1), "all 5 wheat the loaded home holds go, as the other player receives 5");
    }

    /// NIT (2026-10-02 review): the list fetched on connect settled each
    /// completed trade with a notice of its own, and the toast stack keeps
    /// only a few. One notice now covers the batch, and a trade that moved
    /// nothing (text lines only) is settled but left out of it. Seen red,
    /// 2026-10-02: with the list branch settling one trade at a time with a
    /// notice each (the old behaviour), three notices were pushed.
    #[test]
    fn a_list_catch_up_makes_one_notice() {
        let (mut world, p) = world_with_player();
        carry(&mut world, p, "wheat_0", 9);
        carry(&mut world, p, "rope_0", 1);
        let mut gs = GuiState::default();
        gs.profile_public_key = "bob".into();
        let list = delivered(trade_list(vec![
            payload("t-1", "completed", vec![relay_item(Some("hammer_0"), "Hammer", 1)], vec![relay_item(Some("wheat_0"), "Wheat", 2)]),
            payload("t-2", "completed", vec![relay_item(Some("nail_0"), "Nail", 3)], vec![relay_item(Some("rope_0"), "Rope", 1)]),
            payload("t-3", "completed", vec![relay_item(None, "A hug", 1)], vec![]),
            payload("t-4", "active", vec![], vec![relay_item(Some("wheat_0"), "Wheat", 1)]),
        ]));
        assert!(route_trade_frame(&mut gs, &mut world, |_: &str| true, &list));
        assert_eq!(gs.pending_notices, vec!["2 trades completed: 3 items left your backpack and 4 items arrived.".to_string()]);
        assert_eq!(gs.trade_status, gs.pending_notices[0]);
        let ids: Vec<String> = queued(&world, p).into_iter().map(|(id, _)| id).collect();
        assert_eq!(ids, vec!["t-1", "t-2", "t-3"], "the quiet one is settled all the same");
    }

    /// SHOULD-FIX (2026-10-02 review): `tick` asks for the trade list on every
    /// connect, and a server whose owner switched the game off answers with a
    /// refusal that was printed into chat each time. The refusal answering
    /// THAT request is taken; a later one (something the player did) still
    /// reaches chat. Seen red, 2026-10-02: with the `refuses_auto_list` check
    /// taken out of `route_trade_frame`, the first refusal was not taken and
    /// would have gone to chat.
    #[test]
    fn a_server_without_the_game_refuses_the_automatic_list_quietly() {
        use crate::relay::features::Feature;
        // Worded as relay.rs's capability gate words it.
        let refusal = || {
            delivered(format!(
                "This server has '{}' disabled ({}). The server owner chose not to host it.",
                Feature::Game.key(),
                Feature::Game.summary()
            ))
        };
        let (mut world, _) = world_with_player();
        let mut gs = GuiState::default();
        let known = |_: &str| true;
        let mut synced = false;

        // Identified: tick sends the automatic request.
        assert!(auto_list_due(&mut synced, true));
        assert!(route_trade_frame(&mut gs, &mut world, known, &refusal()), "its refusal is taken");
        assert!(!route_trade_frame(&mut gs, &mut world, known, &refusal()), "a second refusal goes to chat");
        // Once per connection: no request, so nothing to take.
        assert!(!auto_list_due(&mut synced, true));
        assert!(!route_trade_frame(&mut gs, &mut world, known, &refusal()));
        // A link that drops before the answer forgets the request.
        auto_list_due(&mut synced, false);
        assert!(auto_list_due(&mut synced, true));
        auto_list_due(&mut synced, false);
        assert!(!route_trade_frame(&mut gs, &mut world, known, &refusal()));
        // So does the list arriving: that request was answered.
        assert!(auto_list_due(&mut synced, true));
        assert!(route_trade_frame(&mut gs, &mut world, known, &delivered(trade_list(vec![]))));
        assert!(!route_trade_frame(&mut gs, &mut world, known, &refusal()));
    }
}
