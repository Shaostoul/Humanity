//! THE FLEET LEDGER in the game (2026-10-04): Inventory > The fleet, and Server Settings >
//! ADMIN > Fleet supply.
//!
//! The operator, verbatim: "For the sake of simplicity during early development we'll say the
//! fleet has unlimited of everything and just track what they player uses and contributes. That
//! way they can be in the red or black so they can gauge what they're actually
//! using/contributing."
//!
//! The ledger is kept by the server whose shared world the player joins
//! (relay/handlers/fleet_ledger.rs): what they used from the fleet (meals from the ship's
//! stores, power from the ship's reactor) and what they gave it (items handed in at a fleet
//! store, surplus power their home sends back), each line worth what data/ship/fleet_ledger.ron
//! says. This page shows a player THEIR OWN ledger: in the red or in the black, in plain words,
//! the totals used and given, each kind's totals and the newest lines; and lets them take a meal
//! or give items when they stand at a fleet store.
//!
//! A GIVE: `start_give` only asks (`FleetView.outbox`); the next frame engine/fleet.rs takes the
//! items out of the backpack and holds them with the give until the server answers (a yes keeps
//! them given, a refusal puts them back). Held gives are listed here, so the player can see
//! where those items are. Only one give at a time waits for the server.
//!
//! THE WEB mirrors the player's ledger read-only (web/chat/chat-fleet.js, the chat's command
//! palette, "Your fleet ledger"): the ledger is the server's record, so a browser signed in as
//! the player can show it with no game world. Taking a meal and giving stay in the game,
//! because they happen at a store in the shared world. The admin's Fleet supply control and the
//! fleet's totals are mirrored in the chat's Game Admin window (web/chat/chat-game-admin.js).
//! (Until the review of 2026-10-04, finding 14, this said the web could show no ledger because
//! it has no game world: viewing never needed one.)

use egui::RichText;

use crate::gui::theme::Theme;
use crate::gui::{widgets, GuiState};
use crate::systems::inventory::FleetHeld;

/// How near a player stands to use a fleet store (the relay's ship_stores.rs `STORE_REACH_M`,
/// the reach of every interaction; the relay checks it, this only says so first).
pub const STORE_REACH_M: f32 = 5.0;

/// One of the fleet's stores in the shared world (from the welcome's snapshot).
#[derive(Debug, Clone, PartialEq)]
pub struct FleetStore {
    pub entity_id: u64,
    pub name: String,
    pub position: [f32; 3],
}

/// One kind's totals in a ledger.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FleetKindLine {
    pub kind: String,
    pub label: String,
    pub unit: String,
    pub direction: String,
    pub quantity: f64,
    pub value: f64,
}

/// One line of a ledger.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FleetLine {
    pub label: String,
    pub unit: String,
    pub direction: String,
    pub item_name: Option<String>,
    pub quantity: f64,
    pub value: f64,
    pub game_time: f64,
    pub real_day: i64,
}

/// A player's ledger as the server last sent it (`game_fleet_ledger`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FleetLedger {
    /// "unlimited" or "stocked".
    pub supply: String,
    pub used: f64,
    pub contributed: f64,
    /// "black", "red" or "even".
    pub standing: String,
    pub kinds: Vec<FleetKindLine>,
    pub recent: Vec<FleetLine>,
}

impl FleetLedger {
    /// Read a `game_fleet_ledger` message; None for one that carries an error.
    pub fn from_json(v: &serde_json::Value) -> Option<FleetLedger> {
        if v.get("error").is_some() {
            return None;
        }
        let s = |o: &serde_json::Value, k: &str| o.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        let f = |o: &serde_json::Value, k: &str| o.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0);
        let arr = |k: &str| v.get(k).and_then(|x| x.as_array()).cloned().unwrap_or_default();
        Some(FleetLedger {
            supply: s(v, "supply"),
            used: f(v, "used_value"),
            contributed: f(v, "contributed_value"),
            standing: s(v, "standing"),
            kinds: arr("kinds")
                .iter()
                .map(|k| FleetKindLine { kind: s(k, "kind"), label: s(k, "label"), unit: s(k, "unit"), direction: s(k, "direction"), quantity: f(k, "quantity"), value: f(k, "value") })
                .collect(),
            recent: arr("recent")
                .iter()
                .map(|l| FleetLine {
                    label: s(l, "label"),
                    unit: s(l, "unit"),
                    direction: s(l, "direction"),
                    item_name: l.get("item_name").and_then(|x| x.as_str()).map(str::to_string),
                    quantity: f(l, "quantity"),
                    value: f(l, "value"),
                    game_time: f(l, "game_time"),
                    real_day: l.get("real_day").and_then(|x| x.as_i64()).unwrap_or(0),
                })
                .collect(),
        })
    }

    /// What the reactor's power came to on each side (used, given), CR: the "power" kinds.
    pub fn power_values(&self) -> (f64, f64) {
        let mut out = (0.0, 0.0);
        for k in self.kinds.iter().filter(|k| k.kind.starts_with("power")) {
            if k.direction == "used" {
                out.0 += k.value;
            } else {
                out.1 += k.value;
            }
        }
        out
    }
}

/// A give the player asked for, not yet out of the backpack (engine/fleet.rs holds it next).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FleetGive {
    pub give_id: String,
    pub store: u64,
    pub item_id: String,
    pub name: String,
    pub qty: u32,
    pub wear: u32,
    pub quality: u8,
}

/// The whole fleet's sums, for an admin (`game_fleet_totals`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FleetTotals {
    pub players: i64,
    pub used: f64,
    pub contributed: f64,
    /// Held back: too few players other than the admin have a ledger to hide any one of them
    /// in the sums (the review of 2026-10-04, finding 9).
    pub withheld: bool,
    /// How many other players the sums need before they are shown.
    pub others_needed: i64,
}

impl FleetTotals {
    pub fn from_json(v: &serde_json::Value) -> FleetTotals {
        let f = |k: &str| v.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0);
        FleetTotals {
            players: v.get("players").and_then(|x| x.as_i64()).unwrap_or(0),
            used: f("used_value"),
            contributed: f("contributed_value"),
            withheld: v.get("withheld").and_then(|x| x.as_bool()).unwrap_or(false),
            others_needed: v.get("others_needed").and_then(|x| x.as_i64()).unwrap_or(0),
        }
    }
}

/// Everything the fleet panel and the admin section hold (GuiState::fleet).
#[derive(Debug, Clone, Default)]
pub struct FleetView {
    /// The player's ledger as last received from the server they are connected to.
    pub ledger: Option<FleetLedger>,
    /// The fleet's stores in the shared world (engine/fleet.rs, from the welcome).
    pub stores: Vec<FleetStore>,
    /// Where the player stands in the shared world (ship metres, as sent to the server),
    /// mirrored each frame while joined.
    pub my_position: Option<[f32; 3]>,
    /// Each item the fleet has a price for, from this game's data/trade_goods.ron:
    /// (item id, base value). The server's own copy is what counts; this picks the list.
    pub prices: std::collections::BTreeMap<String, f64>,
    /// The give form: the item picked, and how many.
    pub give_item: String,
    pub give_qty: u32,
    /// Gives asked for this frame; engine/fleet.rs takes their items out and holds them.
    pub outbox: Vec<FleetGive>,
    /// The gives whose items are held for a server's answer, as the backpack's record has them
    /// (engine/fleet.rs mirrors it): for every server, each tagged with its own.
    pub held: Vec<FleetHeld>,
    /// Give ids sent on this connection and not answered yet.
    pub sent: Vec<String>,
    /// Real seconds until the next give may be sent (spacing, and a retry after "too soon").
    pub next_send_in: f32,
    /// The home whose recorded gives were asked for on this connection.
    pub gives_asked_for: Option<String>,
    /// One plain sentence about the last action.
    pub status: String,
    /// The admin's unapplied choice of supply mode.
    pub admin_draft: Option<String>,
    /// The fleet's totals, when an admin asked.
    pub totals: Option<FleetTotals>,
    /// Power reporting (engine/fleet.rs): the reactor ledger's drawn and returned watt-hours
    /// already reported, and the real seconds since the last report.
    pub power_baseline: Option<(f64, f64)>,
    pub power_timer: f32,
}

impl FleetView {
    /// Forget what belongs to the server just left (gui/connections.rs, on a switch): its
    /// ledger, its stores, its totals and what was sent to it. Gives held for it stay held, tagged
    /// with that server, and are sent there again when the player goes back (finding 16).
    pub fn forget_server(&mut self) {
        self.ledger = None;
        self.stores.clear();
        self.totals = None;
        self.sent.clear();
        self.gives_asked_for = None;
        self.status.clear();
        self.admin_draft = None;
    }
}

/// Credits as people write them: whole numbers bare, else one decimal.
pub fn credits(v: f64) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{} CR", v.round())
    } else {
        format!("{:.1} CR", v)
    }
}

/// The headline: in the red or in the black, in plain words. A difference that prints as 0 CR
/// reads as even whatever the server's standing says (finding 17: "In the black: you have given
/// the fleet 0 CR more").
pub fn standing_sentence(l: &FleetLedger) -> String {
    let diff = (l.contributed - l.used).abs();
    if l.used == 0.0 && l.contributed == 0.0 {
        return "Nothing yet: you have not used anything from the fleet or given it anything.".to_string();
    }
    match l.standing.as_str() {
        _ if credits(diff) == "0 CR" => "Even: you have given the fleet as much as you have used from it.".to_string(),
        "black" => format!("In the black: you have given the fleet {} more than you have used from it.", credits(diff)),
        "red" => format!("In the red: you have used {} more from the fleet than you have given it.", credits(diff)),
        _ => "Even: you have given the fleet as much as you have used from it.".to_string(),
    }
}

/// What the server's supply mode means, in a sentence. (It no longer says "nobody goes without":
/// the ledger can say a meal was taken, but whether the player eats is the game's, finding 7.)
pub fn supply_sentence(supply: &str) -> &'static str {
    match supply {
        "stocked" => "This server's fleet is stocked: its stores hold only what is in them, and an empty store means a missed meal.",
        _ => "This server's fleet is unlimited: its stores never run out. This ledger is how you see what you take from it and what you give it.",
    }
}

/// The intro, and how power is counted (finding 18 of the 2026-10-04 review: power is most of a
/// ledger, and nothing said it is counted without the player doing anything).
pub const INTRO: &str = "The ship's fleet supplies everyone aboard: meals from the mess hall's stores and power from \
     the ship's reactor. Its ledger keeps what you used and what you gave back, so you can see whether \
     you are in the red or in the black.";
pub const POWER_NOTE: &str = "Power for your home is counted by itself every minute you are in the shared world, and \
     power your home's own panels send back to the ship counts as given.";

/// How many of a thing: "3 meals", "2.25 kWh", "2 Bread".
pub fn amount_text(quantity: f64, unit: &str, item_name: Option<&str>) -> String {
    let n = if (quantity - quantity.round()).abs() < 1e-6 { format!("{}", quantity.round()) } else { format!("{:.2}", quantity) };
    match (item_name, unit) {
        (Some(name), _) => format!("{n} {name}"),
        (None, "meal") if (quantity - 1.0).abs() < 1e-6 => "1 meal".to_string(),
        (None, "meal") => format!("{n} meals"),
        // A kind counted in items (what was given): its total names no one item.
        (None, "") if (quantity - 1.0).abs() < 1e-6 => "1 item".to_string(),
        (None, "") => format!("{n} items"),
        (None, u) => format!("{n} {u}"),
    }
}

/// When a line was written: the shared world's day, and the real date. The server keeps no
/// finer time than the game day (finding 10), so neither does this.
pub fn when_text(game_time: f64, real_day: i64) -> String {
    let day = (game_time / 86_400.0).floor() as i64 + 1;
    let date = super::game_admin::format_ban_date(real_day * 86_400_000);
    format!("Day {day} ({})", &date[..10.min(date.len())])
}

/// The fleet store nearest the player, and how far it is.
pub fn nearest_store(view: &FleetView) -> Option<(&FleetStore, f32)> {
    let me = view.my_position?;
    view.stores
        .iter()
        .map(|s| (s, ((s.position[0] - me[0]).powi(2) + (s.position[1] - me[1]).powi(2) + (s.position[2] - me[2]).powi(2)).sqrt()))
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

/// How many of `item_id` the backpack has free to give: less what a give asked this frame
/// takes, what is moving to storage, and what is promised to a trade the player confirmed
/// (finding 1). Items held for a give are already out of the backpack.
pub fn free_to_give(state: &GuiState, item_id: &str) -> u32 {
    let carried: u32 = state.inventory_items.iter().flatten().filter(|s| s.item_id == item_id).map(|s| s.quantity).sum();
    let asked: u32 = state.fleet.outbox.iter().filter(|g| g.item_id == item_id).map(|g| g.qty).sum();
    let leaving: u32 = state.pending_inventory_transfers.iter().filter(|o| !o.add && o.item_id == item_id).map(|o| o.qty).sum();
    let promised = super::trade::promised_to_trades(state, item_id);
    carried.saturating_sub(asked + leaving + promised)
}

/// The give this server still has to answer, if any: one give waits at a time (findings 4 and
/// 13: a double click sent two gives within the relay's 200 ms).
pub fn waiting_give(state: &GuiState) -> Option<String> {
    if let Some(g) = state.fleet.outbox.first() {
        return Some(format!("{} {}", g.qty, g.name));
    }
    let server = &state.connected_server_url;
    state.fleet.held.iter().find(|h| &h.server == server).map(|h| format!("{} {}", h.qty, h.name))
}

/// Send a message on the open connection; false when there is none.
fn send(state: &GuiState, v: &serde_json::Value) -> bool {
    match state.ws_client.as_ref() {
        Some(c) if c.is_connected() => {
            c.send(&v.to_string());
            true
        }
        _ => false,
    }
}

/// Ask the server for this player's ledger.
pub fn request_ledger(state: &GuiState) -> bool {
    send(state, &serde_json::json!({ "type": "game_fleet_ledger_request" }))
}

/// Give `qty` of `item_id` from the backpack to the fleet at the nearest store: checked here
/// first (in the world, at a store, connected, no other give waiting, carried and free), then
/// ASKED: engine/fleet.rs takes the items out and holds them on the next frame. Returns the
/// sentence to show.
pub fn start_give(state: &mut GuiState, item_id: &str, qty: u32) -> String {
    if !state.copresence_active {
        return "Join a server's shared world to give to its fleet.".into();
    }
    let Some((store, dist)) = nearest_store(&state.fleet).map(|(s, d)| (s.clone(), d)) else {
        return "No fleet store is known in this world.".into();
    };
    if dist > STORE_REACH_M {
        return format!("Walk within {} m of {} to give (you are {:.0} m away).", STORE_REACH_M, store.name, dist);
    }
    if qty == 0 {
        return "Choose how many to give.".into();
    }
    if let Some(w) = waiting_give(state) {
        return format!("Waiting for the server to record {w}; one give at a time.");
    }
    let free = free_to_give(state, item_id);
    if qty > free {
        return format!("You carry only {free} of that free to give.");
    }
    let Some(slot) = state.inventory_items.iter().flatten().find(|s| s.item_id == item_id).cloned() else {
        return "That is no longer in your backpack.".into();
    };
    if !state.ws_client.as_ref().is_some_and(|c| c.is_connected()) {
        return "Not connected to the server.".into();
    }
    state.fleet.outbox.push(FleetGive {
        give_id: format!("give-{:016x}", rand::random::<u64>()),
        store: store.entity_id,
        item_id: item_id.to_string(),
        name: slot.name.clone(),
        qty,
        wear: slot.wear,
        quality: slot.quality,
    });
    format!("Giving {qty} {} to the fleet...", slot.name)
}

/// The `game_fleet_give` message for a held give, from the home `home` (sent again with the
/// same id after a reconnect; `creative` when it was given in Creative mode).
pub fn give_message(h: &FleetHeld, home: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "game_fleet_give", "give_id": h.give_id, "home": home, "entity_id": h.store,
        "item_id": h.item_id, "quantity": h.qty, "creative": h.creative,
    })
}

/// Inventory > The fleet: the player's own ledger, and taking a meal or giving at a store.
pub fn draw_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::body_hint(ui, theme, INTRO);
    widgets::body_hint(ui, theme, POWER_NOTE);
    ui.add_space(theme.spacing_xs);
    let joined = state.copresence_active;
    if let Some(l) = state.fleet.ledger.clone() {
        if !joined {
            // Kept from the last visit to this server's shared world (a switch to another
            // server forgets it, finding 16): say so, so old numbers never read as current.
            widgets::body_hint(ui, theme, "Your ledger on this server, as of your last visit to its shared world.");
        }
        // The server's settings, when this game has them, say the mode as it is NOW (an admin's
        // change reaches every connected app as `server_settings_state`); the ledger says it as
        // it was when the ledger was sent.
        let supply = state.server_settings.as_ref().map_or_else(|| l.supply.clone(), |s| s.fleet_supply_mode.clone());
        draw_ledger(ui, theme, &l, &supply);
    } else if joined {
        widgets::body_hint(ui, theme, "Asking the server for your ledger...");
    } else {
        widgets::body_hint(ui, theme, "The ledger is kept by the server whose shared world you play in. Join one to see yours.");
    }
    draw_held(ui, theme, state);
    if !joined {
        return;
    }
    ui.add_space(theme.spacing_sm);
    draw_actions(ui, theme, state);
    if !state.fleet.status.is_empty() {
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new(state.fleet.status.clone()).size(theme.font_size_small).color(theme.accent()));
    }
}

/// The ledger itself: the headline, the totals, power's share, the supply mode, each kind, the
/// newest lines.
fn draw_ledger(ui: &mut egui::Ui, theme: &Theme, l: &FleetLedger, supply: &str) {
    let color = match l.standing.as_str() {
        _ if standing_sentence(l).starts_with("Even") => theme.text_primary(),
        "black" => theme.success(),
        "red" => theme.danger(),
        _ => theme.text_primary(),
    };
    ui.label(RichText::new(standing_sentence(l)).size(theme.font_size_heading).strong().color(color));
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(format!("Used from the fleet: {}", credits(l.used))).color(theme.text_primary()));
        ui.add_space(theme.spacing_md);
        ui.label(RichText::new(format!("Given to the fleet: {}", credits(l.contributed))).color(theme.text_primary()));
    });
    let (power_used, power_given) = l.power_values();
    if power_used > 0.0 || power_given > 0.0 {
        ui.label(
            RichText::new(format!("Of which power: {} used, {} given back.", credits(power_used), credits(power_given)))
                .size(theme.font_size_small)
                .color(theme.text_secondary()),
        );
    }
    widgets::body_hint(ui, theme, supply_sentence(supply));
    if !l.kinds.is_empty() {
        ui.add_space(theme.spacing_sm);
        widgets::subsection_label(ui, theme, "By kind");
        egui::Grid::new("fleet_kinds").num_columns(3).spacing([theme.spacing_md, theme.spacing_xs]).show(ui, |ui| {
            for k in &l.kinds {
                ui.label(RichText::new(&k.label).color(theme.text_primary()));
                ui.label(RichText::new(amount_text(k.quantity, &k.unit, None)).color(theme.text_secondary()));
                let used = k.direction == "used";
                let sign = if used { "used" } else { "given" };
                ui.label(RichText::new(format!("{} {sign}", credits(k.value))).color(if used { theme.danger() } else { theme.success() }));
                ui.end_row();
            }
        });
    }
    if !l.recent.is_empty() {
        ui.add_space(theme.spacing_sm);
        widgets::subsection_label(ui, theme, "Newest lines");
        egui::Grid::new("fleet_recent").num_columns(4).spacing([theme.spacing_md, theme.spacing_xs]).show(ui, |ui| {
            for line in &l.recent {
                let used = line.direction == "used";
                ui.label(RichText::new(when_text(line.game_time, line.real_day)).size(theme.font_size_small).color(theme.text_muted()));
                ui.label(RichText::new(&line.label).size(theme.font_size_small).color(theme.text_primary()));
                ui.label(RichText::new(amount_text(line.quantity, &line.unit, line.item_name.as_deref())).size(theme.font_size_small).color(theme.text_secondary()));
                let v = if used { format!("-{}", credits(line.value)) } else { format!("+{}", credits(line.value)) };
                ui.label(RichText::new(v).size(theme.font_size_small).color(if used { theme.danger() } else { theme.success() }));
                ui.end_row();
            }
        });
    }
}

/// The gives whose items are held for a server's answer: where those items are now.
fn draw_held(ui: &mut egui::Ui, theme: &Theme, state: &GuiState) {
    if state.fleet.held.is_empty() {
        return;
    }
    ui.add_space(theme.spacing_xs);
    let here = &state.connected_server_url;
    for h in &state.fleet.held {
        let text = if &h.server == here {
            format!("Waiting for the server to record {} {}. They are out of your backpack; if the fleet refuses them, they come back.", h.qty, h.name)
        } else {
            format!(
                "{} {} are held for the fleet of the server at {}. They are sent there when you next join its shared world, and come back to your backpack if it refuses them.",
                h.qty, h.name, h.server
            )
        };
        ui.label(RichText::new(text).size(theme.font_size_small).color(theme.text_muted()));
    }
}

/// Taking a meal and giving, at the nearest fleet store.
fn draw_actions(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::subsection_label(ui, theme, "At a fleet store");
    let near = nearest_store(&state.fleet).map(|(s, d)| (s.clone(), d));
    let in_reach = match &near {
        Some((s, d)) if *d <= STORE_REACH_M => {
            widgets::body_hint(ui, theme, &format!("You are at {} ({:.1} m away).", s.name, d));
            Some(s.clone())
        }
        Some((s, d)) => {
            widgets::body_hint(ui, theme, &format!("The nearest fleet store is {}, {:.0} m away. Walk within {} m of it to take a meal or give.", s.name, d, STORE_REACH_M));
            None
        }
        None => {
            widgets::body_hint(ui, theme, "No fleet store is known in this world yet.");
            None
        }
    };
    ui.horizontal_wrapped(|ui| {
        if widgets::Button::primary("Take a meal").disabled(in_reach.is_none()).tooltip("Eat one crew meal from the ship's stores, one a meal time. It is a line in your ledger.").show(ui, theme) {
            if let Some(s) = &in_reach {
                state.fleet.status = if send(state, &serde_json::json!({ "type": "game_interact", "entity_id": s.entity_id, "action": "take_meal" })) {
                    "Asking for a meal...".into()
                } else {
                    "Not connected to the server.".into()
                };
            }
        }
        if widgets::Button::secondary("Refresh").tooltip("Ask the server for your ledger again.").show(ui, theme) {
            state.fleet.status = if request_ledger(state) { String::new() } else { "Not connected to the server.".into() };
        }
    });

    // The give form: only what the backpack holds and the fleet has a price for.
    ui.add_space(theme.spacing_sm);
    let mut givable: Vec<(String, String, f64)> = Vec::new();
    for s in state.inventory_items.iter().flatten() {
        if let Some(p) = state.fleet.prices.get(&s.item_id) {
            if !givable.iter().any(|(id, _, _)| *id == s.item_id) {
                givable.push((s.item_id.clone(), s.name.clone(), *p));
            }
        }
    }
    if givable.is_empty() {
        widgets::body_hint(ui, theme, "Nothing in your backpack has a price the fleet knows, so there is nothing to give yet.");
        return;
    }
    if !givable.iter().any(|(id, _, _)| *id == state.fleet.give_item) {
        state.fleet.give_item = givable[0].0.clone();
    }
    let free = free_to_give(state, &state.fleet.give_item.clone());
    state.fleet.give_qty = state.fleet.give_qty.clamp(1, free.max(1));
    let picked = givable.iter().find(|(id, _, _)| *id == state.fleet.give_item).cloned().unwrap_or_default();
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Give").color(theme.text_primary()));
        egui::ComboBox::from_id_salt("fleet_give_item").selected_text(picked.1.clone()).show_ui(ui, |ui| {
            for (id, name, _) in &givable {
                ui.selectable_value(&mut state.fleet.give_item, id.clone(), name);
            }
        });
        ui.add(egui::DragValue::new(&mut state.fleet.give_qty).range(1..=free.max(1)));
        ui.label(RichText::new(format!("of {free}")).color(theme.text_secondary()));
    });
    widgets::body_hint(ui, theme, &format!("The fleet values {} at {} each: {} for {}.", picked.1, credits(picked.2), credits(picked.2 * f64::from(state.fleet.give_qty)), state.fleet.give_qty));
    if state.creative_mode {
        widgets::body_hint(
            ui,
            theme,
            "Creative mode is on: the fleet records what you give but does not count it, because Creative mode makes things \
             from nothing. Turn it off at the top of this page for your gifts to count.",
        );
    }
    let waiting = waiting_give(state);
    if widgets::Button::primary("Give to the fleet")
        .disabled(in_reach.is_none() || free == 0 || waiting.is_some())
        .tooltip("Hand these to the fleet's store. They leave your backpack at once and are given when the server records them; if it refuses, they come back.")
        .show(ui, theme)
    {
        let (item, qty) = (state.fleet.give_item.clone(), state.fleet.give_qty);
        state.fleet.status = start_give(state, &item, qty);
    }
}

/// Server Settings > ADMIN > Fleet supply: the server setting `fleet_supply_mode`, and the
/// whole fleet's totals (called from `game_admin::draw_section`, inside the admin-gated
/// section; the relay checks the role again).
pub fn draw_admin(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::section_header(ui, theme, "Fleet supply");
    widgets::body_hint(
        ui,
        theme,
        "Whether the fleet's stores can run out in this server's shared world. Unlimited (the \
         default during early development): they never run out, no meal is ever refused, and each \
         player's ledger shows what they used and gave. Stocked (the realistic mode): the stores \
         hold only what the ship's farms put in, and an empty store means a missed meal, crew \
         included. A change applies at once and never touches anyone's ledger.",
    );
    ui.add_space(theme.spacing_sm);
    let current = state.server_settings.as_ref().map(|s| s.fleet_supply_mode.clone());
    if let (Some(d), Some(c)) = (state.fleet.admin_draft.as_ref(), current.as_ref()) {
        if d == c {
            state.fleet.admin_draft = None;
        }
    }
    match &current {
        Some(c) => ui.label(RichText::new(format!("Now: {}", mode_name(c))).size(theme.font_size_small).color(theme.text_primary())),
        None => ui.label(RichText::new("Connect to a server to see its fleet's supply.").size(theme.font_size_small).color(theme.text_secondary())),
    };
    let chosen = state.fleet.admin_draft.clone().or(current.clone()).unwrap_or_else(|| crate::relay::storage::DEFAULT_FLEET_SUPPLY_MODE.to_string());
    ui.horizontal_wrapped(|ui| {
        for mode in ["unlimited", "stocked"] {
            if ui.radio(chosen == mode, RichText::new(mode_name(mode)).color(theme.text_primary())).clicked() && chosen != mode {
                state.fleet.admin_draft = Some(mode.to_string());
            }
        }
    });
    let differs = current.as_deref() != Some(chosen.as_str());
    ui.add_enabled_ui(differs, |ui| {
        if widgets::Button::primary("Apply to the fleet").tooltip("Run the fleet's stores this way, for every player, from now on.").show(ui, theme) {
            state.game_admin_status = if send(state, &serde_json::json!({ "type": "server_settings_update", "fleet_supply_mode": chosen })) {
                format!("Asked the server to make the fleet {}.", chosen)
            } else {
                "Not connected to the server.".into()
            };
        }
    });
    ui.add_space(theme.spacing_sm);
    widgets::body_hint(ui, theme, TOTALS_NOTE);
    ui.horizontal_wrapped(|ui| {
        if widgets::Button::secondary("Show the fleet's totals").tooltip("Everything every player has used from the fleet and given it, summed. Names no one.").show(ui, theme) {
            if !send(state, &serde_json::json!({ "type": "game_fleet_totals_request" })) {
                state.game_admin_status = "Not connected to the server.".into();
            }
        }
        if let Some(t) = state.fleet.totals {
            ui.label(RichText::new(totals_sentence(&t)).size(theme.font_size_small).color(theme.text_primary()));
        }
    });
}

/// What the admin's totals do and do not hide (finding 9 of the 2026-10-04 review).
pub const TOTALS_NOTE: &str = "The fleet's totals are sums over every player with a ledger. They are shown only once at \
     least three players other than you have one, because with fewer, the totals minus your own lines would \
     be someone's own ledger. Even then, watching them change while you know who is online can hint at who did \
     what.";

/// "Unlimited" / "Stocked", as the control names them.
pub fn mode_name(mode: &str) -> &'static str {
    match mode {
        "stocked" => "Stocked (realistic: stores can run empty)",
        _ => "Unlimited (never runs out)",
    }
}

/// The fleet's totals in a sentence (or why they are held back).
pub fn totals_sentence(t: &FleetTotals) -> String {
    let who = if t.players == 1 { "1 player has".to_string() } else { format!("{} players have", t.players) };
    if t.withheld {
        return format!("{who} a ledger. The totals are shown once at least {} players other than you have one.", t.others_needed);
    }
    format!("{who} a ledger: {} used from the fleet, {} given to it.", credits(t.used), credits(t.contributed))
}

#[cfg(test)]
#[path = "fleet_ledger_tests.rs"]
pub(crate) mod tests;
