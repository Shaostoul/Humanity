//! One person, several sockets (2026-10-02, BUG-112).
//!
//! The desktop app and a web tab, or web Chat and the Tasks board, can sign in
//! with the same identity at once. `RelayState::peers` keeps ONE registration
//! per identity, owned by the newest socket, so on its own it cannot say
//! whether the person is still here when a socket closes. `RelayState::
//! live_conns` counts every signed-in socket per identity, and `note_signed_in`
//! and `release_closed` are its only writers for that count: one when a socket
//! signs in, one when it closes. The relay's teardown runs the departure (peer
//! removed, status cleared, PeerLeft) only when the LAST socket closes.
//!
//! Seats (2026-10-02, review of BUG-112). Two things a person does belong to
//! ONE socket rather than to the whole identity: being in the shared game world
//! (the socket that sent `game_join`) and being in a voice room (the socket
//! that joined it). Waiting for the last socket was wrong for those: quitting
//! the desktop game while a web tab stayed open left the avatar standing in the
//! world and the person listed in voice for as long as the tab lived. So each
//! seat records the socket that took it, and that socket's close runs the
//! seat's departure whether or not other sockets stay open
//! (`depart_owned_seats`).
//!
//! Names (2026-10-02). A socket that signs in without a usable name (the Tasks
//! board, or a second device before its name has loaded) is registered under
//! the name the identity already has (`settle_name`), so the registration it
//! takes over never turns a known person into "Anonymous".

use crate::relay::relay::RelayState;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Who is signed in, and which socket holds each seat. One lock over all
/// three maps, so each change reads them together. The lock alone does not
/// keep a seat from naming a closed socket; what does is the order of the
/// work: a seat is only taken by a message arriving on a signed-in socket,
/// and each socket's teardown gives up the seats it holds
/// (`depart_owned_seats`) before it leaves the live set (`release_closed`).
#[derive(Debug, Default)]
pub struct LiveConns {
    /// Every signed-in socket, per identity.
    pub sockets: HashMap<String, HashSet<u64>>,
    /// The socket that joined the shared game world, per identity. Set by
    /// `game_join`, cleared by `game_leave`, by a despawn, and by that
    /// socket's close.
    pub game_seat: HashMap<String, u64>,
    /// The socket that holds the identity's place in voice: the last one to
    /// join a voice room. Cleared by a voice `leave` and by that socket's close.
    /// A client that reconnects re-sends its join, which moves the seat to the
    /// new socket (web `chat-voice-rooms.js`, native `frame_ws_poll.rs`).
    pub voice_seat: HashMap<String, u64>,
}

/// Count a socket that just signed in among its identity's live ones. Call it
/// BEFORE the registration names the socket, so whatever `peers` points at is
/// always in the set. True when the identity already had a live socket: the
/// same person on another device or tab, not a new arrival.
pub async fn note_signed_in(state: &RelayState, key: &str, conn_id: u64) -> bool {
    let mut live = state.live_conns.write().await;
    let set = live.sockets.entry(key.to_string()).or_default();
    let already_here = !set.is_empty();
    set.insert(conn_id);
    already_here
}

/// Take a closing socket out of its identity's live set. When another socket
/// for the identity is still open, hand the registration to it if this one
/// owned it and return that socket's id: the caller then tidies nothing else.
/// None when this was the last socket, so the person really has left.
pub async fn release_closed(state: &RelayState, key: &str, conn_id: u64) -> Option<u64> {
    let still_open = {
        let mut live = state.live_conns.write().await;
        match live.sockets.get_mut(key) {
            Some(set) => {
                set.remove(&conn_id);
                if set.is_empty() {
                    live.sockets.remove(key);
                    None
                } else {
                    set.iter().next().copied()
                }
            }
            None => None,
        }
    }?;
    if let Some(p) = state.peers.write().await.get_mut(key) {
        if p.conn_id == conn_id {
            p.conn_id = still_open;
        }
    }
    Some(still_open)
}

/// This socket joined the game world: its close is now the identity's game
/// departure. A later join from another socket (the game reconnecting on a
/// fresh socket) takes the seat over, so a dying old socket leaves it alone.
/// Returns the socket that held the seat before (None: none did), which tells
/// a reconnect from a join repeated on the same socket (ship homes increment 4
/// review, M4: move_check.rs `JoinKind`).
pub async fn take_game_seat(state: &RelayState, key: &str, conn_id: u64) -> Option<u64> {
    state.live_conns.write().await.game_seat.insert(key.to_string(), conn_id)
}

/// The identity left the game world some other way (`game_leave`, a despawn),
/// so no socket's close should start a game departure for it any more.
pub async fn release_game_seat(state: &RelayState, key: &str) {
    state.live_conns.write().await.game_seat.remove(key);
}

/// This socket joined a voice room: its close takes the person out of voice.
pub async fn take_voice_seat(state: &RelayState, key: &str, conn_id: u64) {
    state.live_conns.write().await.voice_seat.insert(key.to_string(), conn_id);
}

/// The identity left voice on purpose.
pub async fn release_voice_seat(state: &RelayState, key: &str) {
    state.live_conns.write().await.voice_seat.remove(key);
}

/// Which departures `depart_owned_seats` already ran, so the last-socket path
/// does not run them a second time (a second game departure would restart the
/// link-dead grace clock).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SeatsLeft {
    pub game: bool,
    pub voice: bool,
}

/// A socket is closing: run the departure for every seat it holds, whether or
/// not another socket for the identity stays open. Call it in the teardown
/// BEFORE `release_closed` decides whether the person has left.
///
/// A seat another socket has since taken is not ours, so this does nothing
/// for it: that is the zombie guard for seats (the client reconnected on a
/// fresh socket and joined again before the old one's close was noticed).
///
/// Voice after a network blip (2026-10-02). The new socket usually signs in
/// before the relay notices the old one closed, and the clients now re-send
/// their voice `join` once the new socket's identify is accepted (web
/// `chat-voice-rooms.js`, native `frame_ws_poll.rs`). That join moves the seat
/// to the new socket, so the old socket's close leaves the person in the room.
/// If the old close is noticed first, the person drops off the roster until
/// the re-sent join puts them back.
pub async fn depart_owned_seats(state: &Arc<RelayState>, key: &str, conn_id: u64) -> SeatsLeft {
    let game = give_up_seat(&mut state.live_conns.write().await.game_seat, key, conn_id);
    if game {
        depart_game_seat(state, key).await;
    }
    let voice = depart_voice_seat(state, key, conn_id).await;
    SeatsLeft { game, voice }
}

/// Remove `key`'s seat when `conn_id` holds it; true when it did.
fn give_up_seat(seats: &mut HashMap<String, u64>, key: &str, conn_id: u64) -> bool {
    let ours = seats.get(key) == Some(&conn_id);
    if ours {
        seats.remove(key);
    }
    ours
}

/// The game departure for a seat a closing socket just gave up: the same
/// link-dead grace a lost connection gets, so the place is held and a rejoin
/// inside the window resumes it.
///
/// A rejoin can land between the seat being given up and the departure
/// (review nit, 2026-10-02): the player would then be marked link-dead while
/// a live socket holds them, and swept 90 s later. `game_join` clears
/// link-dead and takes the seat under the world write lock, so after the
/// departure this looks again under the world lock and takes back a link-dead
/// mark when a socket holds the seat. The check before it only saves work.
/// Known limit: with the grace set to 0 the departure despawns at once, and a
/// rejoin landing in that same instant is despawned with it.
pub(crate) async fn depart_game_seat(state: &Arc<RelayState>, key: &str) {
    if holds_game_seat(state, key).await {
        return;
    }
    crate::relay::handlers::msg_handlers::handle_game_disconnect(state, key).await;
    let _world = state.game_world.read().await;
    if holds_game_seat(state, key).await {
        state.link_dead.write().await.remove(key);
    }
}

async fn holds_game_seat(state: &RelayState, key: &str) -> bool {
    state.live_conns.read().await.game_seat.contains_key(key)
}

/// The voice departure, decided and done under the `voice_rooms` lock. A
/// voice `join` holds that lock while it takes the seat, so a join from the
/// person's new socket lands wholly before this (the seat is no longer ours:
/// nothing happens) or wholly after (the person is gone and the join puts
/// them back). Deciding first and removing later would let a join slip in
/// between and leave the new socket holding a seat in a room that no longer
/// lists the person. The removal matches `leave_voice_room`.
async fn depart_voice_seat(state: &Arc<RelayState>, key: &str, conn_id: u64) -> bool {
    let mut rooms = state.voice_rooms.write().await;
    let ours = give_up_seat(&mut state.live_conns.write().await.voice_seat, key, conn_id);
    if !ours {
        return false;
    }
    rooms.retain(|_, room| {
        room.participants.retain(|(k, _)| k != key);
        !room.participants.is_empty()
    });
    drop(rooms);
    crate::relay::handlers::broadcast::broadcast_voice_channel_list(state).await;
    true
}

/// Placeholder / throwaway names (empty, "Anonymous", "Player",
/// "DesktopUser_NNNN"). The relay never registers them or joins them as
/// members, and a socket offering one is registered under the identity's
/// real name when it has one (`settle_name`). Moved here from the identify
/// path in relay.rs (2026-10-02) so both read one definition.
pub fn is_placeholder_name(name: &str) -> bool {
    let n = name.trim();
    n.is_empty()
        || n.eq_ignore_ascii_case("anonymous")
        || n.eq_ignore_ascii_case("player")
        || (n.len() > 12
            && n.starts_with("DesktopUser_")
            && n[12..].bytes().all(|b| b.is_ascii_digit()))
}

/// The name a signing-in socket is registered under. A real offered name is
/// used as is (identify has already validated and registered it). With no
/// usable name, the identity keeps the one it is known by: first the live
/// registration (another socket of theirs is open; a bot's name lives only
/// there), then the key's CURRENT stored name. Only when neither exists does
/// the offered value (nothing, or a placeholder) stand.
///
/// Why (2026-10-02): the Tasks board signs in without a name, and its socket
/// took over the registration with `display_name: None`, so other clients
/// showed the person as "Anonymous" (and offline in the full user list, which
/// matches online people by name) while their Chat tab was still open.
///
/// The stored name is `current_name_for_key`, not `name_for_key` (review,
/// 2026-10-02): the latter returns any name the key ever registered, after a
/// rename usually the old one, and the sign-in path writes whatever this
/// returns into the member row, so a nameless sign-in undid the rename.
pub async fn settle_name(state: &RelayState, key: &str, offered: Option<String>) -> Option<String> {
    if offered.as_deref().is_some_and(|n| !is_placeholder_name(n)) {
        return offered;
    }
    let live = state.peers.read().await.get(key).and_then(|p| p.display_name.clone());
    live.filter(|n| !is_placeholder_name(n))
        .or_else(|| state.db.current_name_for_key(key).ok().flatten().filter(|n| !is_placeholder_name(n)))
        .or(offered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::relay::Peer;

    fn fresh_state(tag: &str) -> Arc<RelayState> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir()
            .join(format!("hum_liveconns_{tag}_{}_{nanos}.db", std::process::id()));
        let db = crate::relay::storage::Storage::open(&path).expect("open test db");
        Arc::new(RelayState::new(db))
    }

    fn peer(key: &str, name: Option<&str>, conn_id: u64) -> Peer {
        Peer {
            public_key_hex: key.to_string(),
            display_name: name.map(str::to_string),
            upload_token: None,
            kyber_public: None,
            conn_id,
        }
    }

    /// The live registration is the source when the database has no name for
    /// the key (a bot, or a name that never got registered). Seen red
    /// 2026-10-02 by deleting the `live.filter(..)` branch so only the
    /// database was asked: the nameless socket came back None.
    #[tokio::test]
    async fn a_nameless_socket_keeps_the_name_on_the_live_registration() {
        let state = fresh_state("live_name");
        let key = "settle_live_key";
        state.peers.write().await.insert(key.to_string(), peer(key, Some("Ada"), 1));
        assert_eq!(settle_name(&state, key, None).await.as_deref(), Some("Ada"));
        assert_eq!(
            settle_name(&state, key, Some("Anonymous".into())).await.as_deref(),
            Some("Ada"),
            "a placeholder is no better than no name"
        );
        assert_eq!(
            settle_name(&state, key, Some("Grace".into())).await.as_deref(),
            Some("Grace"),
            "a real offered name is used as is"
        );
    }

    /// With no other socket open, the name registered to the key is used.
    /// Seen red 2026-10-02 by making `settle_name` return `offered`.
    #[tokio::test]
    async fn a_nameless_socket_takes_the_name_registered_to_its_key() {
        let state = fresh_state("db_name");
        let key = "settle_db_key";
        state.db.register_name("Lovelace", key).expect("register");
        assert_eq!(settle_name(&state, key, None).await.as_deref(), Some("Lovelace"));
        // And someone with no name anywhere stays nameless.
        assert_eq!(settle_name(&state, "nobody_key", None).await, None);
    }

    /// After a rename the stored name to fall back on is the CURRENT one.
    /// `registered_names` keeps every name a key ever registered; the member
    /// row holds the one in use (it is rewritten on each named sign-in).
    /// Seen red 2026-10-02 by putting `name_for_key` back in `settle_name`:
    /// the first assertion got "Aold".
    #[tokio::test]
    async fn a_renamed_identity_settles_on_its_current_name() {
        let state = fresh_state("renamed");
        let key = "renamed_key";
        state.db.register_name("Aold", key).expect("register");
        state.db.register_name("Znew", key).expect("register");
        assert_eq!(
            settle_name(&state, key, None).await.as_deref(),
            Some("Znew"),
            "not a member: the newest registered name"
        );
        // A member who went back to their first name: the member row wins
        // over the newest registration.
        state.db.join_server(key, "Znew").expect("join");
        state.db.update_member_name(key, "Aold").expect("rename back");
        assert_eq!(settle_name(&state, key, None).await.as_deref(), Some("Aold"));
    }

    /// A REVOKED DEVICE DOES NOT GET ITS NAME BACK (review of round 2,
    /// 2026-10-02). Bob's lost laptop is revoked: its `registered_names` row
    /// goes, its member row stays. Signing in nameless, it must not be settled
    /// as "Bob". Red check, run: with the member row trusted alone (the
    /// still-registered check removed from `current_name_for_key`), this got
    /// "Bob".
    #[tokio::test]
    async fn a_revoked_device_does_not_settle_on_its_old_name() {
        let state = fresh_state("revoked");
        let laptop = "laptop_key_0001";
        state.db.register_name("Bob", laptop).expect("register");
        state.db.join_server(laptop, "Bob").expect("join");
        assert_eq!(settle_name(&state, laptop, None).await.as_deref(), Some("Bob"), "before: Bob");
        let revoked = state.db.revoke_device("Bob", "laptop_key").expect("revoke");
        assert!(!revoked.is_empty(), "the device was revoked");
        assert_eq!(settle_name(&state, laptop, None).await, None, "a revoked device has no name to settle on");
    }

    fn graced_state(tag: &str) -> Arc<RelayState> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir()
            .join(format!("hum_liveconns_{tag}_{}_{nanos}.db", std::process::id()));
        let mut state = RelayState::new(crate::relay::storage::Storage::open(&path).expect("open test db"));
        // This machine's server-config.json may set the grace to 0.
        state.reconnect_grace = std::time::Duration::from_secs(90);
        Arc::new(state)
    }

    /// The game rejoin race (review nit, 2026-10-02). The closing socket gives
    /// up its seat; before its departure reaches the world, the client's new
    /// socket rejoins (`game_join` takes the seat under the world write lock,
    /// which the test holds to stand in for it). The departure must not leave
    /// the player marked link-dead, or the sweep despawns a player who is
    /// connected. Seen red 2026-10-02 by deleting the link-dead removal at the
    /// end of `depart_game_seat`: "not link-dead" failed.
    #[tokio::test]
    async fn a_rejoin_during_the_game_departure_is_not_marked_link_dead() {
        let state = graced_state("game_race");
        let key = "racer";
        state.game_world.write().await.spawn_player(key, [0.0, 1.0, 0.0]);
        take_game_seat(&state, key, 7).await;

        let world = state.game_world.write().await; // the rejoin's critical section
        let closing = tokio::spawn({
            let state = state.clone();
            async move { depart_owned_seats(&state, key, 7).await }
        });
        // Let the departure run up to the world lock it now waits on.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(!holds_game_seat(&state, key).await, "socket 7 gave its seat up");
        take_game_seat(&state, key, 8).await; // the rejoin takes the seat
        drop(world);
        assert_eq!(closing.await.unwrap(), SeatsLeft { game: true, voice: false });

        assert!(!state.link_dead.read().await.contains_key(key), "not link-dead");
        assert_eq!(state.live_conns.read().await.game_seat.get(key), Some(&8));
        assert!(state.game_world.read().await.find_player_entity(key).is_some(), "still in the world");
    }

    /// The voice rejoin race (2026-10-02): the person's new socket re-sends
    /// its voice join while the old socket's close is being handled. A join
    /// takes the seat while holding the `voice_rooms` lock (the test holds it
    /// to stand in for the join). The old socket's departure must leave the
    /// person in the room, on the new socket's seat. Seen red 2026-10-02 with
    /// the previous order put back (seat given up under its own lock, then
    /// `leave_voice_room`): socket 7 still held the seat when it decided, so
    /// the departure reported `voice: true` and took the person out of the
    /// room the join had just kept them in.
    #[tokio::test]
    async fn a_voice_join_during_the_old_sockets_close_keeps_the_person_in_the_room() {
        use crate::relay::relay::VoiceRoom;
        let state = fresh_state("voice_race");
        let key = "caller";
        state.voice_rooms.write().await.insert(
            "lounge".into(),
            VoiceRoom { name: "Lounge".into(), participants: vec![(key.into(), "Caller".into())] },
        );
        take_voice_seat(&state, key, 7).await;

        let rooms = state.voice_rooms.write().await; // the join's critical section
        let closing = tokio::spawn({
            let state = state.clone();
            async move { depart_owned_seats(&state, key, 7).await }
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        take_voice_seat(&state, key, 8).await; // the join takes the seat
        drop(rooms);
        assert_eq!(closing.await.unwrap(), SeatsLeft::default(), "the seat was no longer socket 7's");

        let rooms = state.voice_rooms.read().await;
        let listed = rooms.get("lounge").map_or(0, |r| r.participants.iter().filter(|(k, _)| k == key).count());
        assert_eq!(listed, 1, "still in the room, once");
        drop(rooms);
        assert_eq!(state.live_conns.read().await.voice_seat.get(key), Some(&8));
    }

    /// A seat is departed only by the socket that holds it, and only once.
    /// Seen red 2026-10-02 by dropping the `ours` check in
    /// `depart_owned_seats`: the other socket's close took the game seat.
    #[tokio::test]
    async fn only_the_seat_holder_departs_the_seat() {
        let state = fresh_state("seats");
        let key = "seat_key";
        take_game_seat(&state, key, 7).await;
        take_voice_seat(&state, key, 9).await;
        assert_eq!(depart_owned_seats(&state, key, 8).await, SeatsLeft::default());
        assert_eq!(
            depart_owned_seats(&state, key, 7).await,
            SeatsLeft { game: true, voice: false }
        );
        assert_eq!(
            depart_owned_seats(&state, key, 7).await,
            SeatsLeft::default(),
            "a seat departs once"
        );
        assert_eq!(
            depart_owned_seats(&state, key, 9).await,
            SeatsLeft { game: false, voice: true }
        );
    }

    /// Leaving the world without closing the socket gives the seat up, so the
    /// socket's later close does not run a second game departure (which would
    /// restart the link-dead clock on someone already leaving). Seen red
    /// 2026-10-02 by deleting the `release_game_seat` calls from
    /// `handle_game_leave` and `despawn_player_now`: both seats stayed.
    #[tokio::test]
    async fn leaving_the_world_gives_up_the_game_seat() {
        use crate::relay::handlers::msg_handlers::{despawn_player_now, handle_game_leave};
        let state = fresh_state("seat_release");
        state.game_world.write().await.spawn_player("leaver", [0.0, 1.0, 0.0]);
        take_game_seat(&state, "leaver", 3).await;
        handle_game_leave(&state, "leaver", &serde_json::json!({})).await;
        assert!(state.live_conns.read().await.game_seat.get("leaver").is_none(), "game_leave frees the seat");

        state.game_world.write().await.spawn_player("despawned", [0.0, 1.0, 0.0]);
        take_game_seat(&state, "despawned", 4).await;
        despawn_player_now(&state, "despawned").await;
        assert!(state.live_conns.read().await.game_seat.get("despawned").is_none(), "a despawn frees the seat");
    }
}
