//! Homes on the ship, the relay's side of the plot rules that are not the claim itself
//! (increment 1b of docs/design/ship-homes-and-logistics.md; the claim and the spawn are
//! game_state.rs `assign_home`, the table is storage/plots.rs):
//!   - a join naming ANOTHER ship, or naming one when this relay has none, is refused before
//!     anything is spawned (`refused_join`);
//!   - a game whose home cannot stand on the plot it was given gives it back as it leaves
//!     (`give_up_plot_if_asked`);
//!   - an admin gives back a plot from Server Settings, naming the player's public key or the
//!     plot's id (`handle_game_release_plot`, the in-app control the GUI-first rule asks for);
//!   - an account being erased leaves the world and frees its plot in one step, the stored
//!     world forgets its figure, and its own game is told it left (`leave_world_for_erase`);
//!   - the one dispatch relay.rs makes for every game-admin message (`handle_game_admin`);
//!   - where you live on this ship, and choosing a home while you have none (`handle_home`,
//!     ship homes increment 6: `game_home_request` and `game_choose_home`, answered `game_home`).
//!
//! Its own file because msg_handlers.rs is held to a line budget (tests/file_size_ratchet.rs).

use crate::relay::handlers::game_state::{plot_owner_id, start_of_kind, GameWorld, JoinHome, JoinRefusal};
use crate::relay::handlers::msg_handlers::{
    despawn_player_now, handle_game_ban, handle_game_banned_list, handle_game_unban, is_game_admin, perception_rate_allows,
    send_game_private,
};
use crate::relay::handlers::shared_build::{came_down_note, take_down_plot};
use crate::relay::relay::RelayState;
use crate::relay::storage::Storage;
use crate::ship::starts::{refusal_words, HeldHome, HomeAnswer, HomeRefusal, Start, StartOffer, CHOOSE_HOME};
use std::sync::Arc;

/// True when this join was refused before anything is spawned, sent privately as
/// `game_join_denied` with this relay's ship:
///   - reason "other_ship": it names a ship that is not this relay's (the second review of
///     1b: the first build spawned it, welcomed it, and everyone saw a player join and leave
///     at once), its positions could never agree with ours;
///   - reason "no_ship": it names a ship and this relay has none (the third review: it was
///     told "a different ship from yours", which was not true);
///   - reason "no_ship_named": it names an empty ship, its game's own ship did not load (round
///     4 of the review: it was told "a different ship from yours", or, on a relay with no
///     ship, taken as this ship's).
///
/// Nobody else hears of it. If an earlier join of theirs is still in the world (a reconnect
/// after their ship changed), it leaves.
///
/// First of all, a join from a key whose account was erased here, while this relay remembers
/// that (BUG-135; sign_ups.rs `refused_erased_join`): refused with reason "account_erased",
/// the sentence the erase itself sends, so a device still connected from before the erase
/// cannot claim a plot for the erased account.
pub async fn refused_join(state: &Arc<RelayState>, my_key: &str, join: &JoinHome) -> bool {
    if crate::relay::handlers::sign_ups::refused_erased_join(state, my_key).await {
        return true;
    }
    let (why, ship, present) = {
        let world = state.game_world.read().await;
        let Some(why) = world.ship_plots.join_refusal(join) else { return false };
        let ship = serde_json::json!({ "id": world.ship_plots.ship_id, "hash": world.ship_plots.ship_hash });
        (why, ship, world.find_player_entity(my_key).is_some())
    };
    let (reason, message) = match why {
        JoinRefusal::OtherShip => ("other_ship", crate::ship::ship_structure::OTHER_SHIP_SENTENCE),
        JoinRefusal::NoShip => ("no_ship", crate::ship::ship_structure::NO_SHIP_SENTENCE),
        JoinRefusal::NoShipNamed => ("no_ship_named", crate::ship::ship_structure::OWN_SHIP_SENTENCE),
    };
    tracing::info!("Game: {} join refused ({reason}, theirs {:?}); nothing spawned", my_key, join.ship_hash);
    if present {
        despawn_player_now(state, my_key).await;
    }
    let denied = serde_json::json!({
        "type": "game_join_denied",
        "reason": reason,
        "message": message,
        "chat_unaffected": true,
        "ship": ship,
    });
    send_game_private(state, my_key, &denied).await;
    true
}

/// An account is being erased (account_erase.rs `carry_out`, by the person or an admin): take
/// its figure out of the shared world and free its plot on this ship in ONE step, under the
/// game world's write lock, which every join holds while it claims a plot (game_state.rs
/// `assign_home`). Round 4 of the 1b review: the erase freed the plot while the figure still stood on it and its game
/// still drew its home there, so the next joiner was handed a plot someone visibly lived on.
/// Their progress is NOT saved on the way out (`despawn_player_now` would write a fresh
/// `player_progress` row for an account being erased; the erase deletes any row they had).
///
/// Round 5 of the review:
///   - the stored world (`GameWorld::save_to_db`, written every 30 s) is written again in the
///     same step, so it no longer holds the figure (nor one that left in the last 30 s): a
///     crash before the next save restored it,
///     and the restore's ghost reap wrote the erased account's progress back, for good;
///   - the erasing game is told privately that it left (`game_join_denied`, reason
///     "account_erased", `ERASED_SENTENCE`): the game_player_left everyone gets reads, to it,
///     as somebody else leaving, so it went on showing the shared world with every update
///     dropped, and Respawn joined it again, claiming a new plot for the erased account.
///
/// Ship homes increment 5: in the same step every piece standing on the freed plot comes down,
/// whoever built it (rows and all), and every piece the account built anywhere else leaves the
/// relay's memory; those rows go with the rest of the account (`Storage::delete_account`).
///
/// What was freed here, for the erase's receipt ([`ErasedFromWorld::add_to`]).
pub async fn leave_world_for_erase(state: &Arc<RelayState>, key: &str) -> ErasedFromWorld {
    state.link_dead.write().await.remove(key);
    crate::relay::handlers::live_conns::release_game_seat(state, key).await;
    let (left, freed, pieces) = {
        let mut world = state.game_world.write().await;
        let left = world.despawn_player(key);
        // Also when no figure was taken out: one that left within the last 30 s can still
        // stand in the stored world. Cheap, and an erase is rare.
        if state.features.enabled(crate::relay::features::Feature::Game) {
            if let Err(e) = world.save_to_db(&state.db) {
                tracing::warn!("Game: could not store the world without an erased account's figure: {e}");
            }
        }
        let freed = world.release_home(&state.db, key);
        let plot = freed.as_ref().ok().and_then(|p| p.as_deref());
        let pieces = crate::relay::handlers::shared_build::erase_account(state, &mut world, &state.db, key, plot);
        (left, freed, pieces)
    };
    if let Some(entity_id) = left {
        let gone = serde_json::json!({ "type": "game_player_left", "player_id": entity_id });
        let _ = state.broadcast_tx.send(crate::relay::relay::RelayMessage::System { message: format!("__game__:{gone}") });
        // No key (sign_ups.rs `sign_up_logs_never_name_the_key`): the log outlives the window
        // the person was promised.
        tracing::info!("Game: a player left (entity {entity_id}): their account is being erased");
        let told = serde_json::json!({
            "type": "game_join_denied",
            "reason": "account_erased",
            "message": crate::ship::ship_structure::ERASED_SENTENCE,
            "chat_unaffected": true,
        });
        send_game_private(state, key, &told).await;
    }
    let plot_freed = match freed {
        Ok(freed) => freed.is_some(),
        Err(e) => {
            tracing::warn!("Game: could not give back the plot of an account being erased: {e}");
            false
        }
    };
    ErasedFromWorld { plot_freed, pieces }
}

/// What taking an erased account out of the shared world freed ([`leave_world_for_erase`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ErasedFromWorld {
    /// Its plot on this ship was given back.
    pub plot_freed: bool,
    /// How many building pieces stood on that plot and came down with it, whoever built them.
    pub pieces: usize,
}

impl ErasedFromWorld {
    /// Count what was freed in the erase's receipt (`Storage::delete_account`'s), beside what the
    /// erase itself deleted: the plot under `ship_plots`, the pieces that stood on it under
    /// `world_pieces` (with the account's own pieces elsewhere, which the erase deletes).
    pub fn add_to(&self, receipt: &mut Vec<(String, usize)>) {
        for (label, n) in [("ship_plots", usize::from(self.plot_freed)), ("world_pieces", self.pieces)] {
            if n == 0 {
                continue;
            }
            match receipt.iter_mut().find(|(l, _)| l == label) {
                Some(entry) => entry.1 += n,
                None => receipt.push((label.to_string(), n)),
            }
        }
    }
}

/// `game_leave` with `"give_up_plot": true`: the game is leaving because its home cannot stand
/// on the plot this relay gave it (engine/home_plot.rs, "does not fit"), so the plot goes back
/// for the next player instead of being held for good by someone who never lives there. Only
/// ever the leaver's own plot. Called BEFORE they leave the world (msg_handlers.rs
/// `handle_game_leave`), so once anyone sees them gone the plot is free. The building pieces on
/// it come down with it (increment 5, shared_build.rs `take_down_frame`).
pub async fn give_up_plot_if_asked(state: &Arc<RelayState>, player_key: &str, raw: &serde_json::Value) {
    if raw.get("give_up_plot").and_then(|v| v.as_bool()) != Some(true) {
        return;
    }
    let mut world = state.game_world.write().await;
    match world.release_home(&state.db, player_key) {
        Ok(Some(plot)) => {
            take_down_plot(state, &mut world, &state.db, &plot);
            tracing::info!("Game: {} gave up plot {} (their home does not fit it)", player_key, plot);
        }
        Ok(None) => {}
        Err(e) => tracing::warn!("Game: could not give back {}'s plot: {e}", player_key),
    }
}

/// The game-admin messages, one dispatch for relay.rs (each handler checks the admin
/// role itself): game bans (v0.474) and releasing a plot (increment 1b).
pub async fn handle_game_admin(state: &Arc<RelayState>, my_key: &str, kind: &str, raw: &serde_json::Value) {
    match kind {
        "game_ban" => handle_game_ban(state, my_key, raw).await,
        "game_unban" => handle_game_unban(state, my_key, raw).await,
        "game_banned_list_request" => handle_game_banned_list(state, my_key).await,
        "game_release_plot" => handle_game_release_plot(state, my_key, raw).await,
        _ => {}
    }
}

/// Admin gives back a plot on the ship (increment 1b): the in-app control for the plot table
/// (Server Settings > ADMIN > Homes on the ship, src/gui/pages/game_admin.rs; GUI-first,
/// CLAUDE.md). `target` is either the id of a plot of this ship ("p1") or the public key of
/// the player who holds one, as for a game ban. A plot id is what an admin can still name
/// when the holder's key is gone from every list (the third review: an erased account, whose
/// plot nothing in the app could free). The next player who joins without a plot claims it.
///
/// Refused while the holder is in the world: their game draws their home on that plot, and
/// handing it to the next joiner would put two homes on one plot. The check is by the id the
/// plot is held under (`plot_owner_id`), the same one the release deletes by, so a key pasted
/// in upper case cannot slip past it (the third review). There is no automatic release of
/// idle plots: when a plot should go back by itself is the operator's policy call
/// (docs/design/ship-homes-and-logistics.md, question 19).
///
/// The building pieces on a released plot come down with it, whoever built them (decision 2 of
/// the increment 5 plan: otherwise the next household moves in among a stranger's walls), and
/// the notice says how many (shared_build.rs `take_down_frame`, `came_down_note`).
pub async fn handle_game_release_plot(state: &Arc<RelayState>, my_key: &str, raw: &serde_json::Value) {
    let reply = |message: String, ok: bool| {
        serde_json::json!({ "type": if ok { "game_admin_notice" } else { "game_admin_error" }, "message": message })
    };
    if !is_game_admin(state, my_key) {
        let msg = reply("Not authorized: releasing a plot requires an admin or owner role.".into(), false);
        send_game_private(state, my_key, &msg).await;
        return;
    }
    let target = raw.get("target").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if target.is_empty() {
        send_game_private(state, my_key, &reply("No plot id or player public key given.".into(), false)).await;
        return;
    }
    // Shortened for the reply by characters, not bytes: the field is typed by a person.
    let short = if target.chars().count() > 16 { format!("{}...", target.chars().take(16).collect::<String>()) } else { target.clone() };
    let msg = {
        let mut world = state.game_world.write().await;
        let ship = world.ship_plots.ship_id.clone();
        if world.ship_plots.plot(&target).is_some() {
            // A plot of this ship, by its id: whoever holds it.
            match state.db.plot_holder(&ship, &target) {
                Ok(None) => reply(format!("Nobody holds plot {target}."), true),
                Ok(Some(holder)) if world.plot_holder_in_world(&holder) => reply(
                    format!("The player who holds plot {target} is in the world now, and their home stands on it. Release it once they have left."),
                    false,
                ),
                Ok(Some(_)) => match state.db.release_plot_by_id(&ship, &target) {
                    Ok(_) => {
                        tracing::info!("Game: admin {} released plot {}", crate::relay::handlers::federation::key_in_log(my_key), target);
                        let note = came_down_note(take_down_plot(state, &mut world, &state.db, &target));
                        reply(format!("Released plot {target}. The next player to join without a plot gets it.{note}"), true)
                    }
                    Err(e) => {
                        tracing::error!("Game: releasing plot {}: {e}", target);
                        reply("Could not release the plot (a storage error; see the relay log).".into(), false)
                    }
                },
                Err(e) => {
                    tracing::error!("Game: reading who holds plot {}: {e}", target);
                    reply("Could not release the plot (a storage error; see the relay log).".into(), false)
                }
            }
        } else if world.plot_holder_in_world(&plot_owner_id(&target)) {
            reply(format!("{short} is in the world now, and their home stands on that plot. Release it once they have left."), false)
        } else {
            match world.release_home(&state.db, &target) {
                Ok(Some(plot)) => {
                    tracing::info!("Game: admin {} released plot {} held by {}", crate::relay::handlers::federation::key_in_log(my_key), plot, crate::relay::handlers::federation::key_in_log(&target));
                    let note = came_down_note(take_down_plot(state, &mut world, &state.db, &plot));
                    reply(format!("Released plot {plot}, held by {short}. The next player to join without a plot gets it.{note}"), true)
                }
                Ok(None) => reply(format!("{short} holds no plot on this ship."), true),
                Err(e) => {
                    tracing::error!("Game: releasing {}'s plot failed: {e}", target);
                    reply("Could not release the plot (a storage error; see the relay log).".into(), false)
                }
            }
        }
    };
    send_game_private(state, my_key, &msg).await;
}

// ── Where you live (ship homes increment 6) ──────────────────────────────────────────────────

/// `game_home_request` (where do I live on this ship, and what is free?) and `game_choose_home`
/// {start} (give me a home of this kind, while I hold none here), both answered with `game_home`
/// (src/ship/starts.rs `HomeAnswer`) to the asker alone (docs/design/ship-homes-increment-6-plan.md
/// section 3.5). The answer names only the asker's own plot and counts of free plots per start:
/// nobody is told who lives where (plan 3.11). Nothing new is stored: a choice is the claim the
/// join makes, the existing `game_plots` row. A guest in the world who chooses is answered
/// `chose: true`; their game steps out and joins again, and that welcome gives the plot.
///
/// A choice from an account erased here, or from someone banned from the game world, is refused
/// the way their join would be (`game_join_denied`), so neither can hold a plot from outside the
/// world that the join would never have given them.
pub async fn handle_home(state: &Arc<RelayState>, my_key: &str, kind: &str, raw: &serde_json::Value) {
    if let Some(answer) = home_answer(state, my_key, kind, raw).await {
        send_game_private(state, my_key, &answer.to_message()).await;
    }
}

/// What a person asks about their home (`handle_home`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HomeAsk<'a> {
    /// `game_home_request`: where do I live, and what is free?
    Where,
    /// `game_choose_home` with its `start` (None: none, or not a start id).
    Choose(Option<&'a str>),
}

/// `handle_home`'s answer, None when another message went instead (an erased or banned asker's
/// `game_join_denied`). Cheapest first (plan 3.5): the 200 ms limit (`again`, the same answer as
/// a storage error, with no plot and no starts so a client keeps what it showed), then the rest
/// under the game world's lock: a choice takes the WRITE lock, the one every join claims under
/// (msg_handlers.rs `handle_game_join`) and an erase or an admin's release frees under, so a plot
/// is never chosen while it is being given to a joiner or given back.
pub(crate) async fn home_answer(state: &Arc<RelayState>, my_key: &str, kind: &str, raw: &serde_json::Value) -> Option<HomeAnswer> {
    let ask = if kind == CHOOSE_HOME {
        HomeAsk::Choose(raw.get("start").and_then(|v| v.as_str()).filter(|s| crate::ship::starts::is_start_id(s)))
    } else {
        HomeAsk::Where
    };
    if !perception_rate_allows(state, my_key, "home") {
        // A client asking faster than the limit: not worth a log line at the usual level.
        tracing::debug!("Game: home asked again within the limit (refused/again)");
        let world = state.game_world.read().await;
        return Some(refused_alone(HomeRefusal::Again, world.starts.default_start()));
    }
    let answer = match ask {
        HomeAsk::Where => {
            let world = state.game_world.read().await;
            answer_home(&world, &state.db, my_key, ask)
        }
        HomeAsk::Choose(_) => {
            let world = state.game_world.write().await;
            // An erase records before it takes this lock (sign_ups.rs), as for the join.
            if !my_key.starts_with("bot_") && state.db.erased_account_remembered(my_key) {
                drop(world);
                tracing::info!("Game: home chosen (refused/account_erased)");
                crate::relay::handlers::sign_ups::refused_erased_join(state, my_key).await;
                return None;
            }
            if let Some(denied) = game_ban_denial(state, my_key) {
                drop(world);
                tracing::info!("Game: home chosen (refused/game_banned)");
                send_game_private(state, my_key, &denied).await;
                return None;
            }
            answer_home(&world, &state.db, my_key, ask)
        }
    };
    if matches!(ask, HomeAsk::Choose(_)) {
        // Never a key, never a plot id: which kind of home was chosen, or why not (plan 3.11).
        match (answer.chose, answer.refused) {
            (true, _) => tracing::info!("Game: home chosen ({})", answer.plot.as_ref().map(|p| p.start.as_str()).unwrap_or("?")),
            (false, Some(r)) => tracing::info!("Game: home chosen (refused/{})", refusal_name(r)),
            (false, None) => {}
        }
    }
    Some(answer)
}

/// The words a refusal goes by on the wire ("holds_one"), for the log line.
fn refusal_name(r: HomeRefusal) -> String {
    serde_json::to_value(r).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

/// The `game_join_denied` the join sends someone banned from the game world (msg_handlers.rs
/// `handle_game_join`, the same words: a ban there and here read alike), or None when they are
/// not. FAIL CLOSED like the join: a storage error refuses too. Bots are exempt, as at the join.
fn game_ban_denial(state: &Arc<RelayState>, my_key: &str) -> Option<serde_json::Value> {
    if my_key.starts_with("bot_") {
        return None;
    }
    match state.db.is_game_banned(my_key) {
        Ok(None) => None,
        Ok(Some(ban)) => Some(serde_json::json!({
            "type": "game_join_denied",
            "reason": ban.reason,
            "chat_unaffected": true,
            "message": "You are banned from the game world. Chat is unaffected.",
        })),
        Err(e) => {
            tracing::error!("Game: could not read the game bans before a home was chosen: {e}; refused (fail closed)");
            Some(serde_json::json!({
                "type": "game_join_denied",
                "reason": "server error",
                "chat_unaffected": true,
                "message": "Could not verify game access right now. Chat is unaffected; try again shortly.",
            }))
        }
    }
}

/// A refusal that says nothing of a home or the starts (`again`, `no_ship`): only why, in
/// `refusal_words`' sentence. `any` is any start (neither sentence reads it).
fn refused_alone(r: HomeRefusal, any: &Start) -> HomeAnswer {
    HomeAnswer { plot: None, starts: Vec::new(), chose: false, refused: Some(r), message: refusal_words(r, any, None, &[]) }
}

/// The answer, from the world and the plot table (`home_answer` holds the lock and has checked
/// the limit and who is asking). Only ever the asker's own row and counts.
pub(crate) fn answer_home(world: &GameWorld, db: &Storage, my_key: &str, ask: HomeAsk<'_>) -> HomeAnswer {
    let starts = &world.starts;
    let sp = &world.ship_plots;
    if sp.ship_hash.is_empty() {
        return refused_alone(HomeRefusal::NoShip, starts.default_start());
    }
    let owner = plot_owner_id(my_key);
    // Their own plot, when it is still a plot of this ship (a row whose plot left the ship file
    // is no home: the next claim lets it go, `Storage::claim_plot`).
    let held = match db.held_by(&sp.ship_id, &owner) {
        Ok(h) => h.and_then(|id| sp.plot(&id)).map(|p| HeldHome { id: p.id.clone(), kind: p.kind.clone(), start: start_of_kind(starts, &p.kind) }),
        Err(e) => {
            tracing::warn!("Game: could not read a home on the ship: {e}");
            return refused_alone(HomeRefusal::Again, starts.default_start());
        }
    };
    // Every start in the file's order, with how many plots of its kind nobody holds (none for a
    // start not offered: it cannot be chosen).
    let offers = |world: &GameWorld| -> Vec<StartOffer> {
        starts.starts.iter().map(|s| StartOffer::of(s, if s.offered { world.free_of_kind(db, &s.plot_kind) } else { 0 })).collect()
    };
    let answer = |plot: Option<HeldHome>, starts: Vec<StartOffer>, refused: Option<HomeRefusal>, message: String| HomeAnswer {
        chose: false,
        plot,
        starts,
        refused,
        message,
    };
    let HomeAsk::Choose(asked) = ask else { return answer(held, offers(world), None, String::new()) };

    // The start asked for. One that is unknown, or not offered (the Apartment, until there are
    // apartment plots), is refused in its own words, or for an id no row has, a plain sentence.
    let unknown;
    let start = match asked.and_then(|id| starts.get(id)) {
        Some(s) => s,
        None => {
            unknown = unknown_start(starts, asked);
            &unknown
        }
    };
    if !start.offered {
        let message = refusal_words(HomeRefusal::NotOffered, start, held.as_ref(), &[]);
        return answer(held, offers(world), Some(HomeRefusal::NotOffered), message);
    }
    // Moving to another kind of home is not built (plan question 2): one home per ship.
    if held.is_some() {
        let message = refusal_words(HomeRefusal::HoldsOne, start, held.as_ref(), &[]);
        return answer(held, offers(world), Some(HomeRefusal::HoldsOne), message);
    }
    let all: Vec<&str> = sp.plots.iter().map(|p| p.id.as_str()).collect();
    match db.claim_plot(&sp.ship_id, &owner, &all, &world.plot_ids_of_kind(&start.plot_kind)) {
        Ok(Some(id)) => {
            let plot = sp.plot(&id).map(|p| HeldHome { id: p.id.clone(), kind: p.kind.clone(), start: start_of_kind(starts, &p.kind) });
            let chose = plot.is_some();
            HomeAnswer { chose, ..answer(plot, offers(world), None, String::new()) }
        }
        Ok(None) => {
            let offered = offers(world);
            let free_names: Vec<&str> = offered.iter().filter(|o| o.offered && o.free > 0 && o.id != start.id).map(|o| o.name.as_str()).collect();
            let message = refusal_words(HomeRefusal::Full, start, None, &free_names);
            answer(None, offered, Some(HomeRefusal::Full), message)
        }
        Err(e) => {
            tracing::warn!("Game: could not claim a chosen home: {e}");
            refused_alone(HomeRefusal::Again, starts.default_start())
        }
    }
}

/// A start the file has no row for, as `refusal_words` reads it: not offered, named after the id
/// asked for ("Palace homes are not offered on this ship."), or "Those" when none was given.
fn unknown_start(starts: &crate::ship::starts::Starts, asked: Option<&str>) -> Start {
    let mut s = starts.default_start().clone();
    let name = asked.map(|a| a.replace(['_', '-'], " ")).unwrap_or_else(|| "those".into());
    let mut chars = name.chars();
    s.name = chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default();
    s.id = asked.unwrap_or("").to_string();
    s.offered = false;
    s.not_offered_why = String::new();
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_clock::ManualClock;
    use std::time::Duration;

    /// A relay on a fresh database, its 200 ms limit on a clock the test moves (`ask` moves it past
    /// the limit before each ask). The tests give its ship the cabin row (`with_cabin_plots`).
    fn relay(tag: &str) -> (Arc<RelayState>, Arc<ManualClock>) {
        let state = Arc::new(RelayState::new(Storage::open_temp(&format!("home_answer_{tag}"))));
        let clock = ManualClock::new();
        assert!(state.perception_clock.set(clock.clone()).is_ok(), "a new relay has no test clock yet");
        (state, clock)
    }

    async fn ask(state: &Arc<RelayState>, clock: &ManualClock, key: &str, kind: &str, start: Option<&str>) -> HomeAnswer {
        clock.advance(Duration::from_millis(250)); // past the 200 ms limit, unless a test says otherwise
        let raw = match start {
            Some(s) => serde_json::json!({ "type": kind, "start": s }),
            None => serde_json::json!({ "type": kind }),
        };
        home_answer(state, key, kind, &raw).await.expect("a game_home answer")
    }

    fn free_of(a: &HomeAnswer, id: &str) -> Option<u32> {
        a.starts.iter().find(|s| s.id == id).map(|s| s.free)
    }

    /// CHOOSING A HOME CLAIMS ONE ONLY WHEN YOU HOLD NONE (plan 3.5, step 4), and every refusal of
    /// the plan in its own words: holding none, a cabin is claimed (the first, `chose: true`, the
    /// free count one less); holding one, `holds_one` naming it; every cabin held, `full`, with
    /// what is free instead; the Apartment (not offered) and a start no row has, `not_offered`;
    /// asked twice within 200 ms, `again`; a relay whose ship did not load, `no_ship`. The answer
    /// to a request names the asker's own plot only.
    ///
    /// Seen red 2026-10-12 with the claim allowed while holding one (the holds-one check taken
    /// out: the claim then kept the held plot and answered it as chosen): "assertion `left ==
    /// right` failed: holding c1 and asking for another cabin is refused holds_one / left: None /
    /// right: Some(HoldsOne)".
    #[tokio::test]
    async fn choosing_a_home_claims_one_only_when_you_hold_none() {
        let (state, clock) = relay("choose");
        crate::relay::handlers::game_state::with_cabin_plots(&mut *state.game_world.write().await);
        let (cabins, homesteads) = {
            let w = state.game_world.read().await;
            (w.plot_ids_of_kind("cabin").len() as u32, w.plot_ids_of_kind("homestead").len() as u32)
        };
        assert!(cabins >= 2 && homesteads >= 2, "a ship with cabins and homesteads: {cabins} and {homesteads}");
        let (ann, bo) = ("a11ce0", "b0b0b0");

        // Holding none: where she lives is nowhere, every kind's count is whole.
        let a = ask(&state, &clock, ann, "game_home_request", None).await;
        assert_eq!(a.plot, None);
        assert_eq!((free_of(&a, "cabin"), free_of(&a, "homestead"), free_of(&a, "apartment")), (Some(cabins), Some(homesteads), Some(0)));
        assert!(a.refused.is_none() && !a.chose && a.message.is_empty(), "{a:?}");

        // None held: a cabin is claimed, the first one.
        let a = ask(&state, &clock, ann, "game_choose_home", Some("cabin")).await;
        assert!(a.chose, "a choice while holding none is made: {a:?}");
        let mine = a.plot.clone().expect("the plot chosen");
        assert_eq!((mine.kind.as_str(), mine.start.as_str()), ("cabin", "cabin"));
        assert_eq!(mine.id, state.game_world.read().await.plot_ids_of_kind("cabin")[0], "the first cabin");
        assert_eq!(free_of(&a, "cabin"), Some(cabins - 1), "one cabin fewer free");
        // Asked where she lives: that cabin.
        assert_eq!(ask(&state, &clock, ann, "game_home_request", None).await.plot, Some(mine.clone()));

        // Holding one: refused, naming hers, nothing claimed.
        let a = ask(&state, &clock, ann, "game_choose_home", Some("cabin")).await;
        assert_eq!(a.refused, Some(HomeRefusal::HoldsOne), "holding {} and asking for another cabin is refused holds_one", mine.id);
        assert!(!a.chose);
        assert_eq!(a.plot, Some(mine.clone()));
        assert_eq!(a.message, format!("You already have a home on this ship: a cabin, {}. Moving to another kind of home comes in a later update.", mine.id));
        let a = ask(&state, &clock, ann, "game_choose_home", Some("homestead")).await;
        assert_eq!(a.refused, Some(HomeRefusal::HoldsOne), "nor another kind");
        assert_eq!(free_of(&a, "cabin"), Some(cabins - 1), "nothing more was claimed");
        assert_eq!(free_of(&a, "homestead"), Some(homesteads));

        // Not offered: the Apartment in its own words; an id no row has, plainly.
        let apartment_why = state.game_world.read().await.starts.get("apartment").unwrap().not_offered_why.clone();
        let a = ask(&state, &clock, bo, "game_choose_home", Some("apartment")).await;
        assert_eq!((a.refused, a.message.as_str()), (Some(HomeRefusal::NotOffered), apartment_why.as_str()));
        assert_eq!(a.plot, None);
        let a = ask(&state, &clock, bo, "game_choose_home", Some("palace")).await;
        assert_eq!((a.refused, a.message.as_str()), (Some(HomeRefusal::NotOffered), "Palace homes are not offered on this ship."));
        let a = ask(&state, &clock, bo, "game_choose_home", None).await;
        assert_eq!((a.refused, a.message.as_str()), (Some(HomeRefusal::NotOffered), "Those homes are not offered on this ship."));

        // Full: every cabin held by someone; Bo is refused and told the homesteads are free.
        {
            let w = state.game_world.read().await;
            for (n, id) in w.plot_ids_of_kind("cabin").iter().enumerate() {
                if *id != mine.id {
                    let all: Vec<&str> = w.ship_plots.plots.iter().map(|p| p.id.as_str()).collect();
                    state.db.claim_plot(&w.ship_plots.ship_id, &format!("did:hum:filler{n}"), &all, &[*id]).unwrap();
                }
            }
        }
        let a = ask(&state, &clock, bo, "game_choose_home", Some("cabin")).await;
        assert_eq!(a.refused, Some(HomeRefusal::Full), "every cabin held: full");
        assert_eq!(a.message, "No cabin is free on this ship right now. You can have a homestead instead.");
        assert_eq!((a.plot.as_ref(), free_of(&a, "cabin")), (None, Some(0)));
        // The homestead is still his to choose.
        let a = ask(&state, &clock, bo, "game_choose_home", Some("homestead")).await;
        assert!(a.chose && a.plot.as_ref().is_some_and(|p| p.kind == "homestead"), "{a:?}");

        // Twice within 200 ms: again, and nothing about any home.
        let a = ask(&state, &clock, bo, "game_home_request", None).await;
        assert!(a.refused.is_none());
        let raw = serde_json::json!({ "type": "game_home_request" });
        let again = home_answer(&state, bo, "game_home_request", &raw).await.expect("an answer");
        assert_eq!(again.refused, Some(HomeRefusal::Again), "a second ask in the same instant");
        assert_eq!((again.plot, again.starts.len(), again.message.as_str()), (None, 0, "The server is busy; try again in a moment."));
        clock.advance(Duration::from_millis(199));
        let raw = serde_json::json!({ "type": "game_choose_home", "start": "cabin" });
        assert_eq!(home_answer(&state, bo, "game_choose_home", &raw).await.unwrap().refused, Some(HomeRefusal::Again), "199 ms later");

        // A relay whose ship did not load: no_ship, and nothing else.
        state.game_world.write().await.ship_plots = Default::default();
        let a = ask(&state, &clock, "c0ffee", "game_choose_home", Some("cabin")).await;
        assert_eq!(a.refused, Some(HomeRefusal::NoShip));
        assert_eq!((a.plot, a.starts.len(), a.message.as_str()), (None, 0, "This server's ship did not load, so there is no home to choose."));
        assert_eq!(ask(&state, &clock, ann, "game_home_request", None).await.refused, Some(HomeRefusal::NoShip));
    }

    /// The answer about a home is the asker's alone: Bo, holding a homestead, asks where he lives
    /// and is told his own plot and counts; nothing in the message names Ann's cabin or her key.
    #[tokio::test]
    async fn the_answer_names_only_the_askers_own_home() {
        let (state, clock) = relay("only_mine");
        crate::relay::handlers::game_state::with_cabin_plots(&mut *state.game_world.write().await);
        let (ann, bo) = ("a11ce0", "b0b0b0");
        let annes = ask(&state, &clock, ann, "game_choose_home", Some("cabin")).await.plot.expect("Ann's cabin");
        let bos = ask(&state, &clock, bo, "game_choose_home", Some("homestead")).await.plot.expect("Bo's homestead");
        let a = ask(&state, &clock, bo, "game_home_request", None).await;
        assert_eq!(a.plot, Some(bos));
        let text = a.to_message().to_string();
        assert!(!text.contains(&format!("\"{}\"", annes.id)), "Bo's answer names Ann's plot: {text}");
        assert!(!text.contains(ann) && !text.contains(&plot_owner_id(ann)), "Bo's answer names Ann: {text}");
        let msg = a.to_message();
        let mut fields: Vec<&str> = msg.as_object().unwrap().keys().map(|k| k.as_str()).collect();
        fields.sort_unstable();
        assert_eq!(fields, ["plot", "starts", "type"], "only the asker's plot and the starts");
    }
}
