# Federation Activation: design

> **Status (2026-10-22):** Phase 1 native UI shipped (v0.722.0); its web mirror is the admin console's federation panel (the data it needs is in the admin API, see "What the admins see" below). **Phase 2 built 2026-10-22**: every kind of federated frame has a per-peer allowance, and a peer over it is slowed, then paused, never dropped (`src/relay/handlers/federation_flow.rs`). **Phase 3's load-bearing test is automated** (`src/relay/handlers/federation_phase3_tests.rs`, in the relay battery): two relays on loopback, a message, an edit, a delete, a profile change and a ban on one reach the other within 10 seconds, the ban is honoured there (what "honoured" means is decided below), and a pause loses nothing and repeats nothing. The live check on the operator's two servers is `docs/admin/federation-live-check.md`. Phase 4 (third-party peers) is still to come.
>
> **Original status (2026-05-20):** design. Operator chose "plan activation" for TIER 1 #4. Federation code was fully implemented but dormant (zero peers). This doc is the vetting + abuse model + phased plan to take it live SAFELY.

## Why federation exists

HumanityOS's mission is civilization-scale + no-single-point-of-control. Federation lets independent relays (run by different people/communities) interconnect so a user on one server can chat in shared channels + have their signed profile/objects replicate across servers. No home server; latest-timestamp-wins; any server caches. This is the multi-server future the architecture was built for, it just hasn't been switched on because one relay + zero peers needs no federation.

## Current state (what's already built)

**Storage** (`federated_servers` table): `server_id, name, url, public_key, trust_tier (default 0), accord_compliant, status, last_seen, added_at`. Managed via `add_federated_server` / `set_server_trust_tier` / `list_federated_servers`.

**Trust tiers** gate everything:
- `trust_tier 0` (default), untrusted. Not connected to; inbound hellos rejected.
- `trust_tier < 2`, skipped by `start_federation_connections`.
- `trust_tier >= 2`, trusted: we make outbound connections to it AND accept its inbound `FederationHello`.

**Outbound** (`federation.rs::start_federation_connections` + `federation_connect_loop`): for each peer with trust_tier >= 2, open a persistent WS with exponential backoff. On connect, send a `FederationHello` signed with our server's Ed25519 key.

**Inbound** (`handle_federation_hello`): accept a hello ONLY from a server already in `federated_servers` at trust_tier >= 2, with a fresh timestamp (±5 min, replay-guarded) and a valid Ed25519 signature over that timestamp. Reply with `FederationWelcome` listing our federated channels.

**What flows once connected:**
- `FederatedChat`, chat in *federated channels only* (`is_channel_federated`), rate-limited 10 msg/s per peer.
- `ProfileGossip`, signed profiles; **Dilithium3-verified** since v0.276.0 (`verify_profile_signature`). Bad sig from a signed client = rejected.
- `SignedObjectGossip`, generic PQ-signed objects; `put_signed_object` verifies the Dilithium signature; multi-hop re-gossip to other peers (excluding source), rate-limited 50/s per peer; `INSERT OR IGNORE` breaks cycles.

**Key fact:** federation is **fail-closed today.** An unknown server cannot federate without the operator explicitly adding it AND raising its trust_tier to 2. So "dormant" is already safe, the risk is only realized when the operator deliberately trusts a peer. Good foundation.

## The trust model: how a peer earns trust_tier 2

This is the human process the code assumes but doesn't enforce. Proposed:

1. **Out-of-band introduction.** A prospective peer operator contacts you (not via the relay). You learn who they are + their server URL + their server public key.
2. **Accord agreement.** They affirm the Humanity Accord (esp. the altruistic-benevolence + no-prompt-injection articles). The `accord_compliant` flag records this. A peer that won't agree doesn't get trusted.
3. **Reciprocity.** Federation is bidirectional, both servers must add each other at trust_tier 2. Document the exact key exchange (each operator gives the other their server_id + public_key + url).
4. **Staged trust.** Add at trust_tier 1 first (present but not connected) to record the relationship, observe, then raise to 2 to activate. (Today only >=2 is meaningful; tier 1 is a documented "known but not yet activated" holding state.)
5. **Revocation is instant.** `set_server_trust_tier(server_id, 0)` + restart drops them (see INCIDENT-PLAYBOOK "A federated peer turns hostile"). This must stay a one-command kill switch.

## Abuse model: what a trusted peer can do, and our defenses

A peer is only as trustworthy as its operator; assume any peer *could* turn hostile or be compromised. Per-vector:

| Vector | What a hostile peer could do | Current defense | Gap |
|--------|------------------------------|-----------------|-----|
| Chat flood | Spam federated channels | Per-peer allowance, slowed then paused (Phase 2, 2026-10-22) | OK |
| Profile gossip flood | Spam profile updates → DB write storm | Dilithium sig verify + per-peer allowance (Phase 2) | OK since 2026-10-22 |
| Federation switched off | A peer the owner once listed dials in anyway and is taken in | The hello is refused (`run_inbound_peer`) and `federation_log::ingest` takes nothing in; a peer's frames never pass the socket's feature gate | OK since 2026-10-24 |
| Signed-object flood | Spam objects + amplify via multi-hop | Per-peer allowance (Phase 2) + Dilithium verify + INSERT-OR-IGNORE dedup | OK |
| Resume flood | Ask for the catch-up again and again, so this server reads and sends its outbox | Per-peer allowance for requests (2 a second, 10 at once), batches of 100 | Bounded; a hostile tier-2 peer could still cost about 2 batches a second of bandwidth until it is removed |
| Forged identity | Impersonate a user from their server | Profiles/objects are Dilithium-signed by the USER's key; a peer can't forge a signature it doesn't hold | OK (the user's PQ key is the root of trust, not the peer's) |
| Malicious content | Push illegal/abusive content into shared channels | The origin's deletes reach every copy; bans are honoured in the shared rooms (decided below, tested in Phase 3) | OK since 2026-10-22 |
| Moderation takeover | A hostile tier-2 peer bans our people from the rooms we share with it | Its bans count only in the shared rooms, only while it is linked at tier 2, and are listed per peer for the admins | Accepted for operator-controlled peers; see "Before Phase 4" below |
| Hello replay | Replay an old hello to spoof presence | ±5 min freshness window | OK |
| Metadata leak | A peer learns who's in shared channels, message timing | Inherent to federation; DMs are NOT federated (E2EE, server-mediated only) | Acceptable, document it |

**The biggest unknowns** (must resolve before trusting a real third-party peer): (a) does moderation (ban/mute/delete) propagate to or apply against federated content? (b) profile-gossip per-peer rate limit. Both resolved 2026-10-22: (a) decided and tested in Phase 3 below, (b) built in Phase 2 for every kind of frame.

## Activation phases

### Phase 1: federated-server admin UI + channel-federation toggle
Today, adding/trusting a peer is a raw storage call (no UI) and there's no surfaced way to mark a channel federated. Build:
- Server Settings → Federation panel: list peers (name, url, trust_tier, status, accord_compliant), add a peer, raise/lower trust_tier, **one-click defederate** (set tier 0 + drop connection without a full restart).
- A per-channel "federated" toggle (writes `is_channel_federated`).
- Native (`src/gui/pages/server_settings.rs`) + web parity.

### Phase 2: a per-peer allowance for every kind of frame (BUILT 2026-10-22)

The plan was a profile-gossip rate limit beside the existing ones. What was built covers every kind of frame a linked server sends, and replaces the old limits, which DROPPED what came over them (chat over 10 a second was lost for good):

| Kind | Allowance per peer (at once, then a second) |
|------|------|
| chat line | 100, then 20 |
| edit | 50, then 10 |
| delete or erase | 100, then 20 |
| ban or unban | 20, then 2 |
| profile copy | 500, then 100 (a new link's catch-up of up to 1,000 copies waits about 5 s) |
| key card (`docs/design/key-cards.md`) | 500, then 100: its own allowance, not shared with profile copies, because the same catch-up also sends up to 1,000 cards; shared, the two would wait about 15 s together, the time that pauses a link, and apart they wait about 5 s each (2026-10-24) |
| signed object | 200, then 50 |
| request to catch up | 10, then 2 |
| bookkeeping (skips, gaps, batch ends) | 1,000, then 200 |

Over the allowance a frame **waits for its turn** (the read pump sleeps, so the peer's socket backs up: TCP slows it). A peer that has spent more than **15 seconds of the last minute** waiting is flooding, not bursting, and is **paused**: for 5 s, then 5, 10, 15, 25, 40, 65, 105 s for each pause in a row (the Fibonacci delays the chat limiter uses), and ten calm minutes start it again from 5 s. What it sends while paused is set aside unprocessed, and when the pause ends this server asks for it again from the numbered log (Phase 3 below), so a pause loses nothing and repeats nothing. What this server asked for (a catch-up) is paced by its own asking and never counts as a flood. The allowances are in `FlowLimits::default` (`src/relay/handlers/federation_flow.rs`).

### Phase 3: everything on one server reaches the other (BUILT AND TESTED 2026-10-22)

**The numbered log.** Before this, a federated chat line went once, live, to whichever links were up, and a receiver refused it after five minutes as a possible replay; nothing but chat crossed. Now everything a server sends its links goes into its **outbox** first, numbered (`federation_outbox`; `src/relay/handlers/federation_log.rs`, storage in `src/relay/storage/federation_log.rs`): chat lines, edits, deletes, account erases, bans and unbans, profile copies, key cards, signed objects. A receiver applies numbers strictly in order and stores its place in each peer's log (`federation_cursors`): a repeat is ignored, a gap makes it ask for everything after its place (`fed_resume`), answered in batches of 100. A link that comes up asks the same way. So a peer that was down, restarted, or paused catches up exactly. The outbox keeps 10,000 rows and at most 7 days (fewer when the server keeps its messages for less); a peer further behind is told the gap (counted as `missed` for the admins) and sent the current ban list and profile copies so that state still agrees. A deleted message's words are scrubbed from the outbox at once, so a link catching up later is never sent words their author or a moderator took back; an edited line is signed again with its new words.

**What "honoured" means (decided 2026-10-22; the design had left it open):**

- **A message belongs to the server it was first posted on (its origin).** The origin's edit or delete, by the author or by a moderator there, is applied to every linked server's copy. An account erased there has its messages removed from every copy (only when it had posted in a shared room, so nothing is said about anyone else). A linked server's own moderators can still remove a copy from their own server; that stays local, because it is not their message to remove from anyone else's.
- **A ban is honoured in the rooms the servers share.** Every linked server (tier 2 or above) refuses the banned key's posts in its federated rooms, saying "This room is shared with <server>, which has banned you, so you cannot post in it. This server's own rooms are still open to you." The banning server also refuses that key's words arriving from any link, in case a peer does not honour it. Each server's own rooms, and whether the person may sign in there at all, stay that server's call. Unbanning lifts it everywhere; removing a link, or lowering it below tier 2, lifts that server's bans at once.
- **Kicks and mutes stay local.** They are brief by nature, and sharing them would let one server's moment of moderation silence someone everywhere.

**The test** (`src/relay/handlers/federation_phase3_tests.rs`, run by `just verify-relay`): two complete relays on 127.0.0.1, linked both ways at tier 2 the way `/server-add` links them, people signing in through the real `/ws` handshake and posting PQ-signed chat, a moderator banning through the real `mod_action`. A message, an edit, a delete, a profile change and a ban on A are on B within 10 seconds (the test prints the slowest: `PHASE 3: the slowest change took ...`). B refuses the banned key in the shared room and lets it post in its own; A refuses that key's words from a peer that did not honour the ban; the unban reaches B. B paused for three seconds while A posts five lines, edits one and deletes another: and A keeps a new key card of Ann's: nothing arrives during the pause, then B holds exactly the four remaining lines, the edited one edited, each once, and Ann's card, and asking again from three numbers back changes nothing. A flood of 40 lines against small allowances is slowed, paused (shown by the admin API) and still arrives complete, each line once.

**Still to do in Phase 3:** the same check on the operator's live pair, in a test channel (`docs/admin/federation-live-check.md`). Defederating through the Phase 1 UI and the kill switch (tier 0, the peer cannot reconnect) were already covered by `a_link_to_a_removed_peer_closes` and the hello checks.

### What the admins see

Each peer row of `POST /api/admin/federation` (every action) and of the admin stats' `federation` list carries a read-only `link` object, for the web admin page's federation panel and the native Server Settings panel:

```json
"link": {
  "flow": {
    "state": "ok | slowed | paused",
    "paused_until": 1760000000000,
    "pauses": 0, "strikes": 0, "set_aside": 0, "missed": 0,
    "slowed_ms_last_minute": 0, "slowed_ms_total": 0,
    "received": { "chat": 0, "edit": 0, "delete": 0, "moderation": 0, "profile": 0, "object": 0, "resume": 0, "control": 0 },
    "limits": { "chat": { "per_sec": 20, "burst": 100 } }
  },
  "cursor": { "log": "<its outbox id>", "seq": 0 },
  "sending": { "mode": "waiting | catching_up | live", "queued": 0 },
  "bans": [ { "public_key": "...", "name": "...", "banned_at": 0 } ]
}
```

`paused_until` is Unix milliseconds (null when not paused); `received` counts frames since this server started; `bans` are the bans that peer told this server about (at most 100 listed). The admin stats also carry `ip_limits`, the per-address limits' summary (see `docs/admin/SELF-HOSTING.md`).

### Before Phase 4 (third-party peers)

A tier-2 peer's bans count in the rooms shared with it. For servers one operator runs, that is the point. For a third party it is a way to silence our people in a shared room, visible in its `bans` list and undone by removing the link. Before the first third-party peer, decide whether honouring a peer's bans should be a per-peer switch (default off for third parties), and whether an admin may lift one peer's ban here without removing the link.

### Phase 4: open to vetted third-party peers
Only after Phases 1–3. Document the operator-facing "how to federate with HumanityOS" process (key exchange, accord agreement, reciprocity). Start with ONE trusted third party; expand slowly.

## Open questions for the operator

1. **Server-to-server hello: keep Ed25519 or move to PQ?** The federation HELLO handshake is signed with the server's Ed25519 key (the last Ed25519 path, deliberately untouched by the user-identity PQ cutover). Federated CONTENT (profiles, objects) is already Dilithium. Moving the hello to PQ is low-priority (it's a server key, not a user identity, and the freshness window limits replay) but worth a decision for full-PQ consistency.
2. **Channel-federation granularity:** federate specific channels only (e.g., a shared #network channel) vs all public channels? Per-channel is safer (limits blast radius); recommend starting there.
3. **Accord enforcement:** is `accord_compliant` a self-attestation (peer says yes) or is there a verification step? For Phase 4, probably self-attestation + revocation-on-violation.
4. **Default trust for the second-VPS test:** the operator's own second box can go straight to tier 2 (Phase 3); third parties start at tier 1 (observed) before tier 2.

## Relationship to current code

Nothing here requires undoing existing code, the dormant implementation is the correct foundation. Phase 1 adds management UI on top of the existing storage API; Phase 2 adds one rate-limit map; Phase 3 is testing; Phase 4 is process + docs. The fail-closed default (trust_tier 0) means we can build + test all of this without exposing the live relay to any untrusted peer until the operator deliberately flips a peer to tier 2.
