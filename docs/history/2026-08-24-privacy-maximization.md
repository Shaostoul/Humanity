# 2026-08-24: privacy maximization (follows graph removed + the last leaks closed)

The operator asked to push all the way to full privacy maximization before
writing anything public, rather than piecemeal it. This arc finishes the job
the sealed-sender cutover and the hardening sweep started.

## The follows graph is gone

`follows` was the last server-side social graph, and it's the interesting one
because it powered a real feature (the friend gate on DMs) so it couldn't just
be dropped. The redesign moves the whole notion client-side:

- **Following** is a sealed control message. When you follow someone, your
  client sends a `[[hum:follow]]` DM (recipient copy + self copy, like any
  message) over the mailbox. Each client keeps its own following/followers
  sets in its local encrypted store; the self-copy syncs your own state across
  your devices for free. Unfollow is `[[hum:unfollow]]`.
- **Friendship** is a client-held certificate. When a follow becomes mutual,
  each side issues the other a Dilithium certificate authorizing them to DM
  freely: `Dilithium_issuer("hum/friend/v1\n{issuer}\n{grantee}")`. The relay
  verifies it statelessly at `dm_put` and stores nothing. A subpoena of the
  server finds no who-follows-whom and no who-is-friends-with-whom, because
  neither is recorded.
- **Strangers can still reach out.** A DM without a certificate is a "knock":
  still sealed, still delivered, but capped at 20 per sender per day across all
  recipients. Cold outreach works; flooding doesn't. The budget is
  sender-scoped, never per-pair, so it never reconstructs a graph.
- **Friend codes** now hand the redeemer the owner's key and let the clients
  complete the friendship over control messages, instead of the relay creating
  edges.

All three clients implement it: the relay verifies certs and meters knocks;
native carries the control-message brain in `engine/dm.rs` with the social sets
in `dm_store.rs`; web mirrors it in `chat-social.js` and `chat-dm-store.js`.
The existing follow badges and friend indicators keep working, now fed from the
local store instead of a server push.

## The last length/transport/replication leaks

- **DM size padding.** Ciphertext length was leaking message length ("ok" vs a
  paragraph is visible to anyone holding the mailbox). Sealed plaintext is now
  padded up to buckets (256 / 1024 / 4096 / 16384 bytes) before encryption; a
  test proves a two-character message and a full sentence produce
  identical-length ciphertext.
- **Message retention.** A new `message_retention_days` server setting (default
  0 = keep forever) auto-expires public channel messages past the window on the
  same maintenance sweep as the DM-mailbox TTL. Pinned messages are always
  kept. It bounds how long even public history lingers.
- **Federation gossip respects unlisted.** A user on the Private or Balanced
  tier (directory-unlisted) no longer has their profile replicated across
  federated servers; the gossip and the signed-profile cache are gated on the
  directory choice, and switching to unlisted retracts the local replicated
  copy. Gossip only ever carried user-authored public profile fields anyway,
  never presence, IP, or DM metadata, but propagating an unlisted user's
  profile still contradicted their choice.
- **Tor onion service.** The one exposure the application layer can't fix is
  the IP address of a live socket. An optional Tor v3 onion service
  (`scripts/tor-onion-setup.sh`, `docs/admin/tor-onion-service.md`) lets users
  reach the relay without revealing their location at all. Opt-in and additive;
  the clearnet endpoint is unchanged.

## Verification

Full lib suite 1606/0. New tests: friend-cert roundtrip with a pinned preimage
(the web client builds the same string, so the format is frozen against silent
drift), the knock-budget path (certless mail caps at the daily budget, a forged
cert counts as no cert, a valid cert bypasses an exhausted budget), padding
equal-length, and message retention (window respected, pins preserved, 0 =
forever). All four standalone GUI lints green.

## Honest limits after this arc

v1 certificates don't expire and can't be server-side revoked, so unfriending
is a client-side action (your client stops showing them; their mail lands under
the knock budget). Live traffic analysis by an active wire observer on the
clearnet endpoint remains a mixnet problem that the onion service sidesteps for
opt-in users but doesn't universally solve. Both are documented, not papered
over.
