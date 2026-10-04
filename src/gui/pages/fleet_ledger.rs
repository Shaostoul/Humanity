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
//! A GIVE: the backpack lives in this game (the server holds no inventories yet), so the items
//! leave it only once the server says it recorded the give. `start_give` sends it with an id of
//! its own and keeps it "in flight"; engine/fleet.rs takes the answer, and on a success takes the
//! items out of the backpack ONCE (recorded by the give's id with the backpack, so it is saved
//! with it and never done twice); on a refusal nothing leaves.
//!
//! THE WEB does not mirror the player's ledger: the website has no game world and shows no
//! player's game state (no inventory, no quest, no position), so there is nothing there to give
//! from or to measure. The admin's Fleet supply control and the fleet's totals ARE mirrored, in
//! the chat's Game Admin window (web/chat/chat-game-admin.js), because an admin may run a server
//! from a browser.

use egui::RichText;

use crate::gui::theme::Theme;
use crate::gui::{widgets, GuiState};

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
}

/// A give sent to the server and not yet answered, or answered yes this session.
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
}

/// Everything the fleet panel and the admin section hold (GuiState::fleet).
#[derive(Debug, Clone, Default)]
pub struct FleetView {
    /// The player's ledger as last received.
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
    /// Gives sent and not answered yet.
    pub in_flight: Vec<FleetGive>,
    /// Gives the server recorded this session: each one's items leave the backpack once
    /// (engine/fleet.rs settles them, and settles again after a save load put them back).
    pub confirmed: Vec<FleetGive>,
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

/// Credits as people write them: whole numbers bare, else one decimal.
pub fn credits(v: f64) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{} CR", v.round())
    } else {
        format!("{:.1} CR", v)
    }
}

/// The headline: in the red or in the black, in plain words.
pub fn standing_sentence(l: &FleetLedger) -> String {
    let diff = (l.contributed - l.used).abs();
    match l.standing.as_str() {
        "black" => format!("In the black: you have given the fleet {} more than you have used from it.", credits(diff)),
        "red" => format!("In the red: you have used {} more from the fleet than you have given it.", credits(diff)),
        _ if l.used == 0.0 && l.contributed == 0.0 => "Nothing yet: you have not used anything from the fleet or given it anything.".to_string(),
        _ => "Even: you have given the fleet as much as you have used from it.".to_string(),
    }
}

/// What the server's supply mode means, in a sentence.
pub fn supply_sentence(supply: &str) -> &'static str {
    match supply {
        "stocked" => "This server's fleet is stocked: its stores hold only what is in them, and an empty store means a missed meal.",
        _ => "This server's fleet is unlimited: its stores never run out, so nobody goes without. This ledger is how you see what you take and what you give.",
    }
}

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

/// When a line was written: the shared world's day and time, and the real date.
pub fn when_text(game_time: f64, real_day: i64) -> String {
    let day = (game_time / 86_400.0).floor() as i64 + 1;
    let secs = game_time.rem_euclid(86_400.0) as i64;
    let date = super::game_admin::format_ban_date(real_day * 86_400_000);
    format!("Day {day}, {:02}:{:02} ({})", secs / 3600, (secs % 3600) / 60, &date[..10.min(date.len())])
}

/// The fleet store nearest the player, and how far it is.
pub fn nearest_store(view: &FleetView) -> Option<(&FleetStore, f32)> {
    let me = view.my_position?;
    view.stores
        .iter()
        .map(|s| (s, ((s.position[0] - me[0]).powi(2) + (s.position[1] - me[1]).powi(2) + (s.position[2] - me[2]).powi(2)).sqrt()))
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

/// How many of `item_id` the backpack has that are not already on their way out (a give in
/// flight, a move to storage this frame).
pub fn free_to_give(state: &GuiState, item_id: &str) -> u32 {
    let carried: u32 = state.inventory_items.iter().flatten().filter(|s| s.item_id == item_id).map(|s| s.quantity).sum();
    let in_flight: u32 = state.fleet.in_flight.iter().filter(|g| g.item_id == item_id).map(|g| g.qty).sum();
    let leaving: u32 = state.pending_inventory_transfers.iter().filter(|o| !o.add && o.item_id == item_id).map(|o| o.qty).sum();
    carried.saturating_sub(in_flight + leaving)
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
/// first (in the world, at a store, carried), then sent with a new give id and kept in flight.
/// Returns the sentence to show. Nothing leaves the backpack here.
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
    let free = free_to_give(state, item_id);
    if qty > free {
        return format!("You carry only {free} of that to give.");
    }
    let Some(slot) = state.inventory_items.iter().flatten().find(|s| s.item_id == item_id).cloned() else {
        return "That is no longer in your backpack.".into();
    };
    let give = FleetGive {
        give_id: format!("give-{:016x}", rand::random::<u64>()),
        store: store.entity_id,
        item_id: item_id.to_string(),
        name: slot.name.clone(),
        qty,
        wear: slot.wear,
        quality: slot.quality,
    };
    if !send(state, &give_message(&give)) {
        return "Not connected to the server.".into();
    }
    let s = format!("Giving {qty} {} to the fleet...", give.name);
    state.fleet.in_flight.push(give);
    s
}

/// The `game_fleet_give` message for a give (sent again with the same id after a reconnect).
pub fn give_message(g: &FleetGive) -> serde_json::Value {
    serde_json::json!({ "type": "game_fleet_give", "give_id": g.give_id, "entity_id": g.store, "item_id": g.item_id, "quantity": g.qty })
}

/// Inventory > The fleet: the player's own ledger, and taking a meal or giving at a store.
pub fn draw_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    widgets::body_hint(
        ui,
        theme,
        "The ship's fleet supplies everyone aboard: meals from the mess hall's stores and power \
         from the ship's reactor. Its ledger keeps what you used and what you gave back, so you \
         can see whether you are in the red or in the black.",
    );
    ui.add_space(theme.spacing_xs);
    let joined = state.copresence_active;
    if let Some(l) = state.fleet.ledger.clone() {
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

/// The ledger itself: the headline, the totals, the supply mode, each kind, the newest lines.
fn draw_ledger(ui: &mut egui::Ui, theme: &Theme, l: &FleetLedger, supply: &str) {
    let color = match l.standing.as_str() {
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
        if widgets::Button::primary("Take a meal").disabled(in_reach.is_none()).tooltip("One crew meal from the ship's stores, one a meal time.").show(ui, theme) {
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
    if widgets::Button::primary("Give to the fleet").disabled(in_reach.is_none() || free == 0).tooltip("Hand these to the fleet's store. They leave your backpack once the server has recorded the give.").show(ui, theme) {
        let (item, qty) = (state.fleet.give_item.clone(), state.fleet.give_qty);
        state.fleet.status = start_give(state, &item, qty);
    }
    for g in &state.fleet.in_flight {
        ui.label(RichText::new(format!("Waiting for the server to record {} {}...", g.qty, g.name)).size(theme.font_size_small).color(theme.text_muted()));
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
         default during early development): they never run out, nobody misses a meal, and each \
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

/// "Unlimited" / "Stocked", as the control names them.
pub fn mode_name(mode: &str) -> &'static str {
    match mode {
        "stocked" => "Stocked (realistic: stores can run empty)",
        _ => "Unlimited (never runs out)",
    }
}

/// The fleet's totals in a sentence.
pub fn totals_sentence(t: &FleetTotals) -> String {
    let who = if t.players == 1 { "1 player has".to_string() } else { format!("{} players have", t.players) };
    format!("{who} a ledger: {} used from the fleet, {} given to it.", credits(t.used), credits(t.contributed))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::gui::screen_surface::find_text_in_shapes;

    fn ledger(standing: &str, used: f64, contributed: f64) -> FleetLedger {
        FleetLedger { supply: "unlimited".into(), used, contributed, standing: standing.into(), ..Default::default() }
    }

    /// THE HEADLINE SAYS RED OR BLACK IN PLAIN WORDS, and by how much.
    ///
    /// Seen red 2026-10-04 with the difference not made positive (given minus used): "left:
    /// \"In the red: you have used -18 CR more from the fleet than you have given it.\" / right:
    /// \"In the red: you have used 18 CR more from the fleet than you have given it.\"".
    #[test]
    fn the_headline_says_red_or_black_in_plain_words() {
        assert_eq!(standing_sentence(&ledger("black", 10.0, 22.0)), "In the black: you have given the fleet 12 CR more than you have used from it.");
        assert_eq!(standing_sentence(&ledger("red", 30.0, 12.0)), "In the red: you have used 18 CR more from the fleet than you have given it.");
        assert_eq!(standing_sentence(&ledger("even", 0.0, 0.0)), "Nothing yet: you have not used anything from the fleet or given it anything.");
        assert_eq!(standing_sentence(&ledger("even", 5.0, 5.0)), "Even: you have given the fleet as much as you have used from it.");
        assert_eq!(credits(4.5), "4.5 CR");
        assert_eq!(amount_text(3.0, "meal", None), "3 meals");
        assert_eq!(amount_text(1.0, "meal", None), "1 meal");
        assert_eq!(amount_text(2.25, "kWh", None), "2.25 kWh");
        assert_eq!(amount_text(2.0, "", Some("Bread")), "2 Bread");
        assert_eq!(amount_text(2.0, "", None), "2 items");
        assert_eq!(when_text(86_400.0 * 2.0 + 3600.0 * 14.0 + 60.0 * 20.0, 20_730), "Day 3, 14:20 (2026-10-04)");
    }

    /// The server's message reads into the ledger the panel draws.
    ///
    /// Seen red 2026-10-04 with the used total read from "used" (the message says
    /// "used_value"): "left: (0.0, 6.0, \"red\") / right: (10.0, 6.0, \"red\")".
    #[test]
    fn a_ledger_message_reads() {
        let v = serde_json::json!({
            "type": "game_fleet_ledger", "supply": "unlimited", "used_value": 10.0, "contributed_value": 6.0,
            "standing": "red", "kinds": [{"kind": "meal", "label": "Meal from the ship's stores", "unit": "meal", "direction": "used", "quantity": 1.0, "value": 10.0}],
            "recent": [{"label": "Given to the fleet", "unit": "", "direction": "contributed", "item_name": "Bread", "quantity": 2.0, "value": 6.0, "game_time": 100.0, "real_day": 20730}],
        });
        let l = FleetLedger::from_json(&v).unwrap();
        assert_eq!((l.used, l.contributed, l.standing.as_str()), (10.0, 6.0, "red"));
        assert_eq!(l.kinds[0].label, "Meal from the ship's stores");
        assert_eq!(l.recent[0].item_name.as_deref(), Some("Bread"));
        assert!(FleetLedger::from_json(&serde_json::json!({"type": "game_fleet_ledger", "error": "failed"})).is_none());
    }

    /// A give is checked before it goes: not in the world, too far from a store, more than the
    /// backpack holds (counting what is already on its way out), and no connection, all say why
    /// and send nothing and keep nothing in flight.
    ///
    /// Seen red 2026-10-04 with `free_to_give` not counting gives in flight: "a second give of
    /// the same 3 loaves was let through / left: \"Not connected to the server.\" / right: \"You
    /// carry only 0 of that to give.\"".
    #[test]
    fn a_give_is_checked_before_it_goes() {
        let mut gs = GuiState::default();
        gs.fleet.stores = vec![FleetStore { entity_id: 12, name: "The mess hall's stores".into(), position: [67.0, 1.0, 22.0] }];
        gs.inventory_items = vec![Some(crate::gui::GuiItemSlot { item_id: "bread_0".into(), name: "Bread".into(), quantity: 3, wear: 0, quality: 0 })];
        assert_eq!(start_give(&mut gs, "bread_0", 1), "Join a server's shared world to give to its fleet.");
        gs.copresence_active = true;
        gs.fleet.my_position = Some([67.0, 1.7, 40.0]);
        assert!(start_give(&mut gs, "bread_0", 1).starts_with("Walk within 5 m of The mess hall's stores"), "too far");
        gs.fleet.my_position = Some([68.0, 1.7, 22.0]);
        assert_eq!(start_give(&mut gs, "bread_0", 4), "You carry only 3 of that to give.");
        assert_eq!(start_give(&mut gs, "bread_0", 3), "Not connected to the server.");
        assert!(gs.fleet.in_flight.is_empty(), "nothing in flight without a connection");
        // A give already on its way counts against the backpack.
        gs.fleet.in_flight.push(FleetGive { give_id: "give-1".into(), store: 12, item_id: "bread_0".into(), name: "Bread".into(), qty: 3, wear: 0, quality: 0 });
        let second = start_give(&mut gs, "bread_0", 3);
        assert_eq!(second, "You carry only 0 of that to give.", "a second give of the same 3 loaves was let through / left: {second:?} / right: \"You carry only 0 of that to give.\"");
    }

    /// THE WEB'S GAME ADMIN WINDOW OFFERS THE SAME FLEET SUPPLY CONTROL: the same two modes under
    /// the same names, the same message, and app.js keeps the mode and the totals it shows.
    ///
    /// Seen red 2026-10-04 with the web's stocked label written "Stocked (stores can run out)":
    /// "the web window offers Stocked (realistic: stores can run empty) too: ['stocked',
    /// 'Stocked (realistic: stores can run empty)'] in FLEET_MODES".
    #[test]
    fn the_web_window_offers_the_same_fleet_supply_control() {
        let js = std::fs::read_to_string("web/chat/chat-game-admin.js").expect("web/chat/chat-game-admin.js");
        for mode in ["unlimited", "stocked"] {
            let entry = format!("['{mode}', '{}']", mode_name(mode));
            assert!(js.contains(&entry), "the web window offers {} too: {entry} in FLEET_MODES", mode_name(mode));
        }
        assert!(js.contains("type: 'server_settings_update', fleet_supply_mode: chosen"), "the web sends the same update");
        assert!(js.contains("type: 'game_fleet_totals_request'"), "and asks for the same totals");
        let app = std::fs::read_to_string("web/chat/app.js").expect("web/chat/app.js");
        assert!(app.contains("msg.settings.fleet_supply_mode") && app.contains("case 'game_fleet_totals':"), "app.js keeps the mode and the totals");
    }

    /// One headless frame of the fleet section in a plain panel.
    fn frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) -> egui::FullOutput {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 900.0))), ..Default::default() };
        ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| draw_section(ui, theme, state));
        })
    }

    /// THE PANEL SHOWS THE LEDGER IN WORDS: the headline, both totals, the supply mode, a line,
    /// and, at a store, the Take a meal and Give buttons; out of the world it says to join one.
    ///
    /// Seen red 2026-10-04 with `draw_ledger` not called when a ledger is in hand: "the panel
    /// shows the headline" (no shape with that text was drawn).
    #[test]
    fn the_panel_shows_the_ledger_in_words() {
        let ctx = egui::Context::default();
        crate::gui::fonts::install_font_fallbacks(&ctx);
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        let mut gs = GuiState::default();
        let out = frame(&ctx, &theme, &mut gs);
        assert!(find_text_in_shapes(&out.shapes, "Join one to see yours").is_some(), "out of the world it says to join one");
        gs.copresence_active = true;
        gs.fleet.ledger = Some(demo_ledger());
        gs.fleet.stores = vec![FleetStore { entity_id: 12, name: "The mess hall's stores".into(), position: [67.0, 1.0, 22.0] }];
        gs.fleet.my_position = Some([68.0, 1.7, 22.0]);
        gs.fleet.prices.insert("bread_0".into(), 3.0);
        gs.inventory_items = vec![Some(crate::gui::GuiItemSlot { item_id: "bread_0".into(), name: "Bread".into(), quantity: 3, wear: 0, quality: 0 })];
        let out = frame(&ctx, &theme, &mut gs);
        for text in ["In the red: you have used 4 CR more", "Used from the fleet: 10 CR", "Given to the fleet: 6 CR", "fleet is unlimited", "2 Bread", "Take a meal", "Give to the fleet"] {
            assert!(find_text_in_shapes(&out.shapes, text).is_some(), "the panel shows {text:?}");
        }
        // An admin switched the server to stocked after the ledger came: the server's settings,
        // which every connected app is sent, say so at once. Seen red 2026-10-04 with the
        // sentence read from the ledger only: "after the switch the panel says the fleet is
        // stocked".
        let mut s = crate::relay::storage::ServerSettings::default();
        s.fleet_supply_mode = "stocked".into();
        gs.server_settings = Some(s);
        let out = frame(&ctx, &theme, &mut gs);
        assert!(find_text_in_shapes(&out.shapes, "fleet is stocked").is_some(), "after the switch the panel says the fleet is stocked");
    }

    /// One headless frame of the admin's Fleet supply section, with `events`.
    fn admin_frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, events: Vec<egui::Event>) -> egui::FullOutput {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 700.0))), events, ..Default::default() };
        ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| draw_admin(ui, theme, state));
        })
    }

    fn admin_click(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, text: &str) {
        let out = admin_frame(ctx, theme, state, Vec::new());
        let pos = find_text_in_shapes(&out.shapes, text).unwrap_or_else(|| panic!("{text} is drawn")).rect.center();
        let m = egui::Modifiers::default();
        admin_frame(ctx, theme, state, vec![egui::Event::PointerMoved(pos)]);
        admin_frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
        admin_frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
    }

    /// THE ADMIN PICKS A MODE AND APPLIES IT, the way Server Settings > ADMIN > Fleet supply is
    /// used: the server says unlimited, the admin picks Stocked, Apply with the link down says
    /// the server was NOT asked and keeps the pick.
    ///
    /// Seen red 2026-10-04 with the status set whether or not anything was sent: "with no
    /// connection the section says: Asked the server to make the fleet stocked.".
    #[test]
    fn picking_a_fleet_mode_and_applying_with_no_connection_says_so() {
        let ctx = egui::Context::default();
        crate::gui::fonts::install_font_fallbacks(&ctx);
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        let mut state = GuiState::default();
        state.server_settings = Some(crate::relay::storage::ServerSettings::default());
        let out = admin_frame(&ctx, &theme, &mut state, Vec::new());
        assert!(find_text_in_shapes(&out.shapes, "Now: Unlimited (never runs out)").is_some(), "the section says the server's mode");
        admin_click(&ctx, &theme, &mut state, "Stocked (realistic: stores can run empty)");
        assert_eq!(state.fleet.admin_draft.as_deref(), Some("stocked"), "the admin's pick");
        admin_click(&ctx, &theme, &mut state, "Apply to the fleet");
        assert_eq!(state.game_admin_status, "Not connected to the server.", "with no connection the section says: {}", state.game_admin_status);
        assert_eq!(state.fleet.admin_draft.as_deref(), Some("stocked"), "and keeps the pick");
    }

    /// The ledger the snapshot and the panel test draw: a meal used, bread given.
    pub(crate) fn demo_ledger() -> FleetLedger {
        FleetLedger::from_json(&serde_json::json!({
            "supply": "unlimited", "used_value": 10.0, "contributed_value": 6.0, "standing": "red",
            "kinds": [
                {"kind": "meal", "label": "Meal from the ship's stores", "unit": "meal", "direction": "used", "quantity": 1.0, "value": 10.0},
                {"kind": "item", "label": "Given to the fleet", "unit": "", "direction": "contributed", "quantity": 2.0, "value": 6.0},
            ],
            "recent": [
                {"label": "Given to the fleet", "unit": "", "direction": "contributed", "item_name": "Bread", "quantity": 2.0, "value": 6.0, "game_time": 216_600.0, "real_day": 20730},
                {"label": "Meal from the ship's stores", "unit": "meal", "direction": "used", "quantity": 1.0, "value": 10.0, "game_time": 216_000.0, "real_day": 20730},
            ],
        }))
        .unwrap()
    }
}
