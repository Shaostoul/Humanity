# First Playable: multiplayer co-presence gap list

**Status: SCOPING (2026-06-16).** The operator's near-term success criterion is "a playable
multiplayer experience on the VPS." This doc records what is actually wired today (verified by hand,
not from STATUS.md, whose multiplayer checkmarks are self-flagged unverified pre-v0.132) and the
concrete gap to two humans co-occupying the 3D world. It is a backlog, not a design rewrite.

## What works today (verified)
- **The relay has an authoritative game world.** `src/relay/handlers/msg_handlers.rs:3276`
  `handle_game_join(...)` plus `game_join`-first guards (msg_handlers.rs:3670/3733/3938/3980) gate a
  set of game endpoints. The server is ready to track joined players.
- **The client has a sync layer, fully written.** `src/net/protocol.rs:8` `enum NetMessage` defines
  `Join`, `PlayerJoined`, `PositionUpdate` (and more). `src/net/sync.rs:26` `NetSyncSystem` has
  `new`/`queue_messages`/`set_player_id`. `NetClient` exists. None of it is broken; it is just dark.

## The gap (verified)
1. **`NetSyncSystem` / `NetClient` are instantiated NOWHERE.** `grep "NetSyncSystem\|NetClient\|
   net::sync" src/lib.rs` returns 0 hits. The engine main loop never creates the sync system, never
   sends a join, never applies remote player positions. The 3D world is single-player in practice.
2. **The client WebSocket handler is chat-only.** `src/lib.rs:4264` filters `__game__:` and
   `__sync_data__` OUT of the chat display (correct -- game traffic should not appear in #general),
   but there is no sibling branch that routes those same messages INTO the game world. Game messages
   the relay sends are received and then dropped on the floor.
3. **Protocol drift.** The client speaks `net::protocol::NetMessage` (e.g. `{type: "Join", ...}`)
   while the relay speaks a `game_*` envelope (e.g. `{type: "game_join", ...}`). The two were never
   reconciled, so even if the client sent a join today the relay would not recognize it (and vice
   versa for position updates). Pick ONE wire format and make both ends agree (the relay is live and
   harder to change safely, so prefer matching the relay's `game_*` envelope from the client).

## The measurement spike (do this FIRST, before any wiring)
Do not assume the gap size -- measure it. Boot two clients (two identities) against the live VPS
relay, both Enter World, and instrument:
- Does either client send anything game-shaped on connect? (It will not, today -- NetSyncSystem is
  unwired -- so this confirms gap #1.)
- Hand-send a `game_join` envelope over the existing WS and watch the relay logs (`just logs`) for
  `handle_game_join` firing. Confirms the relay half end-to-end.
- This tells us whether co-presence is "80% there, just wire the client" or "the relay endpoints need
  position broadcast too." The survey believes the relay half is largely done; verify it.

## The smallest first slice (after the spike)
A minimal, honest co-presence loop, in order:
1. On Enter World, instantiate `NetSyncSystem`, send a `game_join` (reconciled format) over the
   existing chat WS (reuse the connection -- do not open a second socket).
2. Each frame, send the local player's `PositionUpdate` (throttled, e.g. 10 Hz) as a `__game__:`-
   tagged message so it stays off the chat display.
3. Add the missing client branch at `lib.rs:4264`: when a received system message is `__game__:`-
   tagged AND decodes to a remote `PositionUpdate`/`PlayerJoined`, route it to `NetSyncSystem` ->
   spawn/update a remote-player ECS entity (a capsule + nameplate) instead of `continue`-ing.
4. Relay: confirm `handle_game_join` registers the player and that position updates fan out to other
   joined players in the same world (add the broadcast if it is missing -- the spike says which).
5. Verify with two real clients on the VPS: each sees the other move. That is "first playable."

## The character dimension MUST be designed into the join envelope (do not retrofit)
The very first `game_join` the client ever sends is also where character-select threads in (operator
requirement, 2026-06-16): a player joins with a character that belongs to the server, from a LOCAL
self-custodial section or a SERVER-held section, with servers declaring open/closed/hybrid policy
(Diablo II open vs closed Battle.net). Full design + the grounded data model:
[characters-and-servers.md](characters-and-servers.md). The good news: most of it already exists (the
relay's `player_progress` row keyed by pubkey, which `handle_game_join` re-seeds from and never trusts
client stats). **Three things must be decided BEFORE step 1 above, because they shape the wire and
cannot be cleanly bolted on later:**
1. The `game_join` envelope must carry `character_mode` + (`character` bytes | `character_id`) so the
   relay can branch open vs closed on day one. (Step 1's "reconciled format" IS this envelope.)
2. Identity at join derives from the identify-bound socket key, NOT the client-supplied `player_name`
   (`msg_handlers.rs:3281` currently trusts the name).
3. `character_policy` (default "open") must exist on `ServerInfoResponse` before the client can render
   the right selector for a server.
Everything else (the `server_characters` table, multi-character, intent-based inventory) defers until
after first co-presence -- the single `player_progress` row suffices for the first slice.

## Watchouts
- Reuse the existing chat WebSocket for game traffic (the `__game__:` tag already separates it). Do
  NOT open a second socket or duplicate auth.
- Throttle position updates and never echo a player their own update.
- Relay changes touch `src/relay/` -- they must pass `cargo check --features relay
  --no-default-features` and deploy to the live VPS (operator-authorized).
- This is the operator's #1 goal but it is NOT trivial; treat the spike's findings as the real plan.
