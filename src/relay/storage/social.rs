//! Social storage — deliberately (almost) empty since 2026-08-24.
//!
//! The `follows` table was the LAST server-side social graph, and it is
//! gone (dropped by migration in storage/mod.rs). Following is a
//! client-side notion now: follow/unfollow notices travel as sealed
//! control messages over the DM mailbox, each client keeps its own
//! following/followers sets in its local encrypted store, and FRIENDSHIP
//! is a client-held Dilithium certificate the relay verifies statelessly
//! at dm_put (`relay/core/pq_crypto.rs::verify_friend_cert`).
//!
//! What a subpoena of this server yields about relationships: nothing,
//! because nothing is recorded. Do NOT re-add follow storage here — if a
//! feature seems to need a server-side social edge, the certificate
//! pattern (client-held, statelessly verified) is the house answer.
//!
//! (The legacy plaintext group storage that used to live here was removed
//! 2026-08-23; groups are the E2EE P2P signed-object system in
//! groups_p2p.rs.)
