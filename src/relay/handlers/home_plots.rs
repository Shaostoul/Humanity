//! Homes on the ship, the relay's side of the plot rules that are not the claim itself
//! (increment 1b of docs/design/ship-homes-and-logistics.md; the claim and the spawn are
//! game_state.rs `assign_home`, the table is storage/plots.rs):
//!   - a join naming ANOTHER ship is refused before anything is spawned (`refused_other_ship`);
//!   - a game whose home cannot stand on the plot it was given gives it back as it leaves
//!     (`give_up_plot_if_asked`);
//!   - an admin gives back a player's plot from Server Settings (`handle_game_release_plot`,
//!     the in-app control the GUI-first rule asks for);
//!   - the one dispatch relay.rs makes for every game-admin message (`handle_game_admin`).
//!
//! Its own file because msg_handlers.rs is held to a line budget (tests/file_size_ratchet.rs).

use crate::relay::handlers::msg_handlers::{
    despawn_player_now, handle_game_ban, handle_game_banned_list, handle_game_unban, is_game_admin, send_game_private,
};
use crate::relay::handlers::game_state::JoinHome;
use crate::relay::relay::RelayState;
use std::sync::Arc;

/// True when this join names a ship that is not this relay's, and was refused (the second
/// review of 1b): `game_join_denied` with reason "other_ship", the one plain sentence and this
/// relay's ship, sent privately. Nothing is spawned and nobody hears of it (the first build
/// spawned it, welcomed it, and everyone saw a player join and leave at once): its positions
/// could never agree with ours. If an earlier join of theirs is still in the world (a reconnect
/// after their ship changed), it leaves.
pub async fn refused_other_ship(state: &Arc<RelayState>, my_key: &str, join: &JoinHome) -> bool {
    let (ship, present) = {
        let world = state.game_world.read().await;
        if !world.ship_plots.is_other_ship(join) {
            return false;
        }
        let ship = serde_json::json!({ "id": world.ship_plots.ship_id, "hash": world.ship_plots.ship_hash });
        (ship, world.find_player_entity(my_key).is_some())
    };
    tracing::info!("Game: {} draws another ship ({:?}); join refused, nothing spawned", my_key, join.ship_hash);
    if present {
        despawn_player_now(state, my_key).await;
    }
    let denied = serde_json::json!({
        "type": "game_join_denied",
        "reason": "other_ship",
        "message": crate::ship::ship_structure::OTHER_SHIP_SENTENCE,
        "chat_unaffected": true,
        "ship": ship,
    });
    send_game_private(state, my_key, &denied).await;
    true
}

/// `game_leave` with `"give_up_plot": true`: the game is leaving because its home cannot stand
/// on the plot this relay gave it (engine/home_plot.rs, "does not fit"), so the plot goes back
/// for the next player instead of being held for good by someone who never lives there. Only
/// ever the leaver's own plot. Called BEFORE they leave the world (msg_handlers.rs
/// `handle_game_leave`), so once anyone sees them gone the plot is free.
pub async fn give_up_plot_if_asked(state: &Arc<RelayState>, player_key: &str, raw: &serde_json::Value) {
    if raw.get("give_up_plot").and_then(|v| v.as_bool()) != Some(true) {
        return;
    }
    match state.game_world.read().await.release_home(&state.db, player_key) {
        Ok(Some(plot)) => tracing::info!("Game: {} gave up plot {} (their home does not fit it)", player_key, plot),
        Ok(None) => {}
        Err(e) => tracing::warn!("Game: could not give back {}'s plot: {e}", player_key),
    }
}

/// The game-admin messages, one dispatch for relay.rs (each handler checks the admin
/// role itself): game bans (v0.474) and releasing a player's plot (increment 1b).
pub async fn handle_game_admin(state: &Arc<RelayState>, my_key: &str, kind: &str, raw: &serde_json::Value) {
    match kind {
        "game_ban" => handle_game_ban(state, my_key, raw).await,
        "game_unban" => handle_game_unban(state, my_key, raw).await,
        "game_banned_list_request" => handle_game_banned_list(state, my_key).await,
        "game_release_plot" => handle_game_release_plot(state, my_key, raw).await,
        _ => {}
    }
}

/// Admin gives back a player's plot on the ship (increment 1b, the second review): the
/// in-app control for `Storage::release_plot` (Server Settings > ADMIN > Homes on the ship,
/// src/gui/pages/game_admin.rs; GUI-first, CLAUDE.md). `target` is the player's public key,
/// as for a game ban. The next player who joins without a plot claims it.
///
/// Refused while the target is in the world: their game draws their home on that plot, and
/// handing it to the next joiner would put two homes on one plot. Release it once they have
/// left. There is no automatic release of idle plots: when a plot should go back by itself
/// is the operator's policy call (docs/design/ship-homes-and-logistics.md, question 19).
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
        send_game_private(state, my_key, &reply("No player public key given.".into(), false)).await;
        return;
    }
    // Shortened for the reply by characters, not bytes: the field is typed by a person.
    let short = if target.chars().count() > 16 { format!("{}...", target.chars().take(16).collect::<String>()) } else { target.clone() };
    let msg = {
        let world = state.game_world.read().await;
        if world.find_player_entity(&target).is_some() {
            reply(format!("{short} is in the world now, and their home stands on that plot. Release it once they have left."), false)
        } else {
            match world.release_home(&state.db, &target) {
                Ok(Some(plot)) => {
                    tracing::info!("Game: admin {} released plot {} held by {}", my_key, plot, target);
                    reply(format!("Released plot {plot}, held by {short}. The next player to join without a plot gets it."), true)
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
