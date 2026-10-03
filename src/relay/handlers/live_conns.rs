//! One person, several sockets (2026-10-02, BUG-112).
//!
//! The desktop app and a web tab, or web Chat and the Tasks board, can sign in
//! with the same identity at once. `RelayState::peers` keeps ONE registration
//! per identity, owned by the newest socket, so on its own it cannot say
//! whether the person is still here when a socket closes. `RelayState::
//! live_conns` counts every signed-in socket per identity, and these two
//! functions are the only writers: one when a socket signs in, one when it
//! closes. The relay's teardown runs the departure (peer removed, voice left,
//! the game's link-dead grace) only when the LAST socket closes.

use crate::relay::relay::RelayState;

/// Count a socket that just signed in among its identity's live ones. Call it
/// BEFORE the registration names the socket, so whatever `peers` points at is
/// always in the set. True when the identity already had a live socket: the
/// same person on another device or tab, not a new arrival.
pub async fn note_signed_in(state: &RelayState, key: &str, conn_id: u64) -> bool {
    let mut live = state.live_conns.write().await;
    let set = live.entry(key.to_string()).or_default();
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
        match live.get_mut(key) {
            Some(set) => {
                set.remove(&conn_id);
                if set.is_empty() {
                    live.remove(key);
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
