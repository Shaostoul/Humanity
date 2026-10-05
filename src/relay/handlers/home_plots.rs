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
//!   - the one dispatch relay.rs makes for every game-admin message (`handle_game_admin`).
//!
//! Its own file because msg_handlers.rs is held to a line budget (tests/file_size_ratchet.rs).

use crate::relay::handlers::game_state::{plot_owner_id, JoinHome, JoinRefusal};
use crate::relay::handlers::msg_handlers::{
    despawn_player_now, handle_game_ban, handle_game_banned_list, handle_game_unban, is_game_admin, send_game_private,
};
use crate::relay::handlers::shared_build::{came_down_note, take_down_plot};
use crate::relay::relay::RelayState;
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

/// An account is being erased (msg_handlers.rs `handle_account_delete`): take its figure out
/// of the shared world and free its plot on this ship in ONE step, under the game world's write
/// lock, which every join holds while it claims a plot (game_state.rs `assign_home`). Round 4
/// of the 1b review: the erase freed the plot while the figure still stood on it and its game
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
                        tracing::info!("Game: admin {} released plot {}", my_key, target);
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
                    tracing::info!("Game: admin {} released plot {} held by {}", my_key, plot, target);
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
