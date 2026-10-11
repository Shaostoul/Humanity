//! Tickets, round 4 (2026-10-11; docs/design/media-showcase.md, "Round 4: fixes from the round 3
//! review", R4-1): at most 16 per person, a short fingerprint of the key instead of the key, and a
//! pool of 500,000 that drops its oldest ticket when full instead of refusing anyone. A child of
//! media_round3_tests.rs, so its relay, folders and file helpers are at hand. Each test names the
//! break it was seen failing against ("Seen red").

use super::*;
use crate::relay::media_ticket::{Tickets, BYTES_PER_TICKET, TICKETS_MAX, TICKETS_PER_PERSON};

/// A relay whose ticket pool holds `room` tickets, so a test can fill it.
fn relay_with_pool(tag: &str, room: usize) -> Arc<RelayState> {
    let mut state = RelayState::new(Storage::open_temp(tag));
    state.media_tickets = Tickets::with_room(room);
    Arc::new(state)
}

/// A ticket for `key` to open `item`, now.
fn ticket_for(state: &RelayState, key: &str, item: &MediaItem) -> Result<String, (StatusCode, String)> {
    media_ticket::ticket_core(state, key, item.id, &item.upload_filename, Instant::now()).map(|a| ticket_of(&a.0))
}

/// R4-1 (a). A FULL POOL NEVER LOCKS ANYONE OUT: eight members fill a pool of 64 tickets, eight
/// each (under the 16 one person may hold). A ninth member who has none still gets a ticket, and it
/// opens the file; the pool stays at 64 because the OLDEST ticket was dropped to make room (it no
/// longer opens anything). One person still holds at most 16: the 17th is refused with 429, and
/// nobody else is affected by that.
///
/// Seen red 2026-10-11 with a full pool refusing again (`issue` answering 429 when `by_value.len()
/// >= room`, as round 3 did, instead of dropping the oldest): "a new person still gets a ticket
/// from a full pool: (429, \"Too many members-only files ...\")".
#[tokio::test]
async fn a_full_ticket_pool_drops_its_oldest_and_never_locks_anyone_out() {
    let state = relay_with_pool("mr4_r41a", 64);
    let d = dirs("mr4_r41a");
    let item = member_with_item(&state, &d, "ann", "tok_ann", "1_clip.mp4");
    let mut first = None;
    for n in 0..8 {
        let key = format!("member_{n}");
        state.db.join_server(&key, &key).unwrap();
        for _ in 0..8 {
            let t = ticket_for(&state, &key, &item).unwrap();
            first.get_or_insert(t);
        }
    }
    assert_eq!(state.media_tickets.len(), 64, "test setup: the pool is full");
    let first = first.unwrap();
    assert_eq!(open(&state, &d, item.id, &first, None).await.0, 200, "test setup: the oldest ticket works while it is held");

    state.db.join_server("zed", "Zed").unwrap();
    let fresh = ticket_for(&state, "zed", &item);
    let fresh = match fresh {
        Ok(t) => t,
        Err(refused) => panic!("a new person still gets a ticket from a full pool: {refused:?}"),
    };
    assert_eq!(state.media_tickets.len(), 64, "the pool stays at its size");
    assert!(!state.media_tickets.holds(&first), "the oldest ticket was dropped to make room");
    assert_eq!(open(&state, &d, item.id, &first, None).await.0, 403, "and opens nothing now");
    assert_eq!(open(&state, &d, item.id, &fresh, None).await, (200, b"0123456789".to_vec()), "the new one opens the file");

    for _ in 1..TICKETS_PER_PERSON {
        ticket_for(&state, "zed", &item).expect("up to 16 for one person");
    }
    let over = ticket_for(&state, "zed", &item).expect_err("a 17th for the same person");
    assert_eq!(over.0, StatusCode::TOO_MANY_REQUESTS);
    assert!(ticket_for(&state, "member_3", &item).is_ok(), "someone else is not affected by it");
    assert_eq!(TICKETS_PER_PERSON, 16);
    assert_eq!(TICKETS_MAX, 500_000);
}

/// R4-1 (b). A TICKET KEEPS A FINGERPRINT OF THE KEY, NOT THE KEY: one ticket takes at most 128
/// bytes (the ticket record is `Copy`, so nothing of it is on the heap, checked when the relay
/// compiles), whatever the key's length; a member with a real Dilithium3 key, 3,904 hex characters,
/// gets a ticket that opens the file. The person is found again by their member row, and the key
/// there must match the fingerprint: when the member leaves and someone else joins and is given
/// the same row number, the old ticket opens nothing.
///
/// Seen red 2026-10-11 with the fingerprint check taken out of `admitted` (the row's member let in
/// whoever they are): "a member row given to someone else later does not inherit the ticket /
/// left: 200 / right: 403".
#[tokio::test]
async fn a_ticket_keeps_a_fingerprint_not_the_key() {
    assert!(BYTES_PER_TICKET <= 128, "one ticket takes {BYTES_PER_TICKET} bytes");
    let state = relay("mr4_r41b");
    let d = dirs("mr4_r41b");
    let long_key = "ab".repeat(1952);
    assert_eq!(long_key.len(), 3904, "test setup: the size of a Dilithium3 public key in hex");
    let item = member_with_item(&state, &d, &long_key, "tok_long", "1_clip.mp4");
    let t = ticket_for(&state, &long_key, &item).unwrap();
    assert_eq!(open(&state, &d, item.id, &t, None).await.0, 200, "a member with a full-size key opens the file");

    let row = state.db.member_row(&long_key).unwrap();
    assert!(state.db.leave_server(&long_key).unwrap());
    state.db.join_server("bea", "Bea").unwrap();
    assert_eq!(state.db.member_row("bea"), Some(row), "test setup: the next member is given the same row number");
    assert_eq!(open(&state, &d, item.id, &t, None).await.0, 403, "a member row given to someone else later does not inherit the ticket");
}
